//! Existing static mesh admission and ordered render-resource replacement.

use crate::engine::{
    abi::{self, ReattachContext, WORD_BYTES},
    native,
};
use crate::engine::{
    construct::{self, Region, load, read},
    reflection, session,
};
use crate::static_mesh::layout::{component as component_layout, mesh as mesh_layout};
use crate::static_mesh::{bindings::Reload, storage};
use crate::{Error, Result};
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};
use unrealsdk_rs::objects::{
    is_kind_of, object_class, object_name_text, objects, property_array_dim, property_class_name,
    property_element_size, property_offset, struct_fields,
};

static CREATED: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashSet<usize>> {
    CREATED.get_or_init(Mutex::default)
}

fn snapshot(mesh: usize) -> Result<Vec<u8>> {
    let mut bytes = read(
        mesh + mesh_layout::LOD_MODELS,
        mesh_layout::LIGHT_MAP_COORDINATE_INDEX + WORD_BYTES - mesh_layout::LOD_MODELS,
    )?;
    bytes.extend(read(
        mesh + mesh_layout::COLLISION_NODES,
        mesh_layout::COLLISION_TRIANGLES + crate::engine::abi::ARRAY_HEADER_BYTES
            - mesh_layout::COLLISION_NODES,
    )?);
    bytes.extend(read(
        mesh + mesh_layout::BODY_SETUP,
        mesh_layout::BOUNDS_RADIUS + WORD_BYTES - mesh_layout::BODY_SETUP,
    )?);
    bytes.extend(read(
        mesh + mesh_layout::FULL_PRECISION_UVS,
        mesh_layout::STREAMING_DISTANCE_MULTIPLIER + WORD_BYTES - mesh_layout::FULL_PRECISION_UVS,
    )?);
    Ok(bytes)
}

pub(crate) fn register(mesh: usize) -> Result<()> {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(mesh);
    Ok(())
}

fn component_users(api: &Reload, mesh: usize) -> Result<Vec<usize>> {
    let class = construct::require_class("Engine.StaticMeshComponent")?;
    let property = struct_fields(class.cast())
        .into_iter()
        .find(|p| object_name_text(p.cast()) == "StaticMesh")
        .ok_or_else(|| Error("StaticMeshComponent.StaticMesh unavailable".into()))?;
    if property_class_name(property).as_deref() != Some("ObjectProperty")
        || property_offset(property) != Some(component_layout::MESH as i32)
        || property_element_size(property) != Some(WORD_BYTES as i32)
        || property_array_dim(property) != Some(1)
    {
        return Err(Error(
            "StaticMeshComponent reflection layout differs".into(),
        ));
    }
    for (offset, expected) in [
        (component_layout::ATTACH_DISPATCH, api.component_attach),
        (component_layout::DETACH_DISPATCH, api.component_detach),
        (component_layout::VALIDITY_DISPATCH, api.component_valid),
    ] {
        construct::verify_dispatch(
            "component dispatch",
            api.component_vtable.slot(offset)?,
            expected,
        )?;
    }
    let mut users = Vec::new();
    for object in objects().ok_or_else(|| Error("GObjects unavailable".into()))? {
        if object.is_null() {
            continue;
        }
        match is_kind_of(object, class) {
            Some(false) => continue,
            Some(true) => {}
            None => {
                return Err(Error(
                    "object classification unavailable during reload".into(),
                ));
            }
        }
        let address = object as usize;
        if load(address + component_layout::MESH)? as usize != mesh {
            continue;
        }
        construct::verify_live("mesh component", construct::flags_of("component", object)?)?;
        if object_class(object) != Some(class)
            || load(address)? as usize != api.component_vtable.address()
        {
            return Err(Error(
                "reload refuses specialized StaticMeshComponent users".into(),
            ));
        }
        if load(address + component_layout::LOD_DATA_COUNT)? != 0 {
            return Err(Error(
                "reload refuses per-component LOD/lighting/vertex-color data".into(),
            ));
        }
        users.push(address);
    }
    Ok(users)
}

pub(crate) fn replace(
    path: &str,
    material_paths: &[&str],
    payload: &crate::encoding::Payload,
    sections: &[crate::sections::CheckedSection],
    policy: crate::ReplacementPolicy,
) -> Result<String> {
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let api = Reload::resolve(base)?;
    let object = reflection::static_mesh(path)?;
    let mesh = object as usize;
    construct::verify_vtable("reload mesh", load(mesh)?, &api.mesh_vtable)?;
    let flags = construct::flags_of("reload mesh", object)?;
    construct::verify_live("reload mesh", flags)?;
    let created = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&mesh);
    if policy == crate::ReplacementPolicy::LibraryOwned && !created {
        return Err(Error(
            "reload accepts only library-created static meshes".into(),
        ));
    }
    construct::verify_lods(
        mesh,
        mesh_layout::LOD_MODELS,
        mesh_layout::LOD_INFO,
        crate::static_mesh::layout::lod::SIZE,
    )?;
    let previous = snapshot(mesh)?;
    if (policy == crate::ReplacementPolicy::LibraryOwned
        && load(mesh + mesh_layout::BODY_SETUP)? != 0)
        || load(mesh + mesh_layout::USED_FOR_INSTANCING)? != 0
    {
        return Err(Error(
            "replacement refuses instanced meshes. Reload also refuses collision".into(),
        ));
    }
    // Cooked triangle collision may index LOD0 vertices. Retaining its tree while
    // changing render topology is not safe, even when BodySetup is unchanged.
    for offset in [
        mesh_layout::COLLISION_NODES,
        mesh_layout::COLLISION_TRIANGLES,
    ] {
        construct::read_array(mesh + offset, WORD_BYTES, 0)
            .map_err(|_| Error("replacement refuses native triangle collision trees. Simple BodySetup collision is preserved".into()))?;
    }
    api.lifecycle.require_healthy_render_thread()?;
    let users = component_users(&api, mesh)?;
    let materials = construct::resolve_materials(material_paths)?;
    let section_bytes = crate::sections::encode(
        sections,
        &materials.iter().map(|m| *m as u32).collect::<Vec<_>>(),
    )?;
    construct::retain_materials(&materials)?;
    let staged = storage::stage(
        &api.storage.geometry,
        payload,
        &section_bytes,
        sections.len(),
    )?;
    let canonical_path = construct::object_path("reload mesh", object)?;
    let mut contexts = vec![ReattachContext::default(); users.len()].into_boxed_slice();
    let attachment_states = users
        .iter()
        .map(|&component| {
            Ok((
                load(component + abi::COMPONENT_SCENE)?,
                read(component + abi::COMPONENT_FLAGS, size_of::<u8>())?[0]
                    & abi::COMPONENT_ATTACHED,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    construct::retain_object("replacement mesh", object)?;
    let result = (|| {
        for (context, &component) in contexts.iter_mut().zip(&users) {
            api.lifecycle.detach(context, component);
            if load(component + abi::COMPONENT_SCENE)? != 0
                || read(component + abi::COMPONENT_FLAGS, size_of::<u8>())?[0]
                    & abi::COMPONENT_ATTACHED
                    != 0
            {
                return Err(Error("component did not detach completely".into()));
            }
        }
        if component_users(&api, mesh)? != users {
            return Err(Error("mesh users changed during detachment".into()));
        }
        if previous != snapshot(mesh)? {
            return Err(Error(
                "static mesh ownership changed during detachment".into(),
            ));
        }
        api.lifecycle
            .release_and_wait(mesh as *mut native::StaticMesh, mesh_layout::RELEASE_FENCE)?;
        storage::install(
            Region {
                address: mesh,
                length: mesh_layout::SIZE,
            },
            &staged,
            payload,
        )?;
        if policy == crate::ReplacementPolicy::ExistingAsset {
            let region = Region {
                address: mesh,
                length: mesh_layout::SIZE,
            };
            region.u32(mesh_layout::FULL_PRECISION_UVS, 1)?;
            region.u32(mesh_layout::LIGHT_MAP_RESOLUTION, 0)?;
            region.u32(mesh_layout::LIGHT_MAP_COORDINATE_INDEX, 0)?;
        }
        // SAFETY: replacement static storage is installed after the issued release fence.
        unsafe { (api.storage.init_resources)(mesh as *mut native::StaticMesh) };
        for context in contexts.iter_mut().rev() {
            api.lifecycle.reattach(context);
        }
        for (&component, &(scene, attached)) in users.iter().zip(&attachment_states) {
            if load(component + abi::COMPONENT_SCENE)? != scene
                || read(component + abi::COMPONENT_FLAGS, size_of::<u8>())?[0]
                    & abi::COMPONENT_ATTACHED
                    != attached
            {
                return Err(Error("static component attachment was not restored".into()));
            }
        }
        Ok(canonical_path)
    })();
    if result.is_err() {
        std::mem::forget(contexts);
    }
    result
}
