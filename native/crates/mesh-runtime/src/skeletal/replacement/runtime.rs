//! Existing skeletal mesh inspection, template admission and ordered replacement.

use super::*;
use crate::engine::abi::{self, ARRAY_HEADER_BYTES, ReattachContext, WORD_BYTES};
use crate::engine::{
    construct::{self, Region, load, read},
    native, reflection, session,
};
use crate::skeletal::layout::{
    component as component_layout, lod as lod_layout, mesh as mesh_layout,
};
use crate::skeletal::{
    PreparedSkeletalMesh, ReferenceBone, SkinnedMesh, bind_to_template,
    bindings::Reload,
    gestalt_update::live::Publication,
    records::{self as skeletal_native, NameWords},
    skeleton::compatible,
    storage,
};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use unrealsdk_rs::objects::{
    is_kind_of, object_name_text, objects, property_array_dim, property_class_name,
    property_element_size, property_offset, struct_fields,
};

struct ExistingMesh {
    snapshot: Vec<u8>,
    skeleton: Vec<ReferenceBone>,
    names: Vec<NameWords>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RegisteredMesh {
    Ordinary,
    Gestalt,
}

// Successful creations retain only their identity and Gestalt classification.
static CREATED: OnceLock<Mutex<HashMap<usize, RegisteredMesh>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<usize, RegisteredMesh>> {
    CREATED.get_or_init(Mutex::default)
}

fn snapshot(mesh: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for (offset, size) in [
        (mesh_layout::MATERIALS, ARRAY_HEADER_BYTES),
        (mesh_layout::REFERENCE_SKELETON, ARRAY_HEADER_BYTES),
        (mesh_layout::LOD_MODELS, ARRAY_HEADER_BYTES * 2),
        (mesh_layout::LOD_INFO, ARRAY_HEADER_BYTES),
        (mesh_layout::REFERENCE_ATOMS, ARRAY_HEADER_BYTES),
    ] {
        bytes.extend(read(mesh + offset, size)?);
    }
    bytes.extend(read(mesh + mesh_layout::COLOR_FLAGS, WORD_BYTES)?);
    let bone_count = load(mesh + mesh_layout::REFERENCE_SKELETON + abi::ARRAY_COUNT)? as usize;
    if !(1..=crate::skeletal::MAX_REFERENCE_BONES).contains(&bone_count) {
        return Err(Error("skeletal reload ownership counts differ".into()));
    }
    bytes.extend(read(
        load(mesh + mesh_layout::REFERENCE_SKELETON)? as usize,
        bone_count * skeletal_native::BONE_SIZE,
    )?);
    let pointers = construct::read_array(
        mesh + mesh_layout::LOD_MODELS,
        WORD_BYTES,
        i32::MAX as usize,
    )?;
    bytes.extend(&pointers);
    for pointer in pointers.chunks_exact(WORD_BYTES) {
        let lod = crate::engine::memory::word(pointer, 0)? as usize;
        bytes.extend(read(lod, lod_layout::SIZE)?);
    }
    Ok(bytes)
}

pub(crate) fn mark_gestalt(path: &str) -> Result<()> {
    let _operation = session::reserve()?;
    let object = reflection::skeletal_mesh(path)?;
    let mut created = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let gestalt = created
        .get_mut(&(object as usize))
        .ok_or_else(|| Error("fresh Gestalt mesh ownership unavailable".into()))?;
    *gestalt = RegisteredMesh::Gestalt;
    Ok(())
}

pub(crate) fn register(mesh: usize) -> Result<()> {
    registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(mesh, RegisteredMesh::Ordinary);
    Ok(())
}

fn existing(mesh: usize) -> Result<ExistingMesh> {
    construct::verify_lods(
        mesh,
        mesh_layout::LOD_MODELS,
        mesh_layout::LOD_INFO,
        lod_layout::SIZE,
    )?;
    let (skeleton, names) = crate::skeletal::reference::read(mesh)?;
    Ok(ExistingMesh {
        snapshot: snapshot(mesh)?,
        skeleton,
        names,
    })
}

pub(crate) fn bind_template(path: &str, fixture: &mut SkinnedMesh) -> Result<()> {
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let api = Reload::resolve(base, fixture.skin.index_stride)?;
    let object = reflection::skeletal_mesh(path)?;
    let mesh = object as usize;
    construct::verify_vtable("Gestalt template mesh", load(mesh)?, &api.mesh_vtable)?;
    construct::verify_live(
        "Gestalt template mesh",
        construct::flags_of("Gestalt template mesh", object)?,
    )?;
    let original = existing(mesh)?;
    bind_to_template(fixture, &original.skeleton)?;
    let tex_coords = fixture.skin.tex_coords;
    let first_lod = load(load(mesh + mesh_layout::LOD_MODELS)? as usize)? as usize;
    let required = load(first_lod + lod_layout::TEX_COORDS)? as usize;
    if !(1..=4).contains(&required) || tex_coords < required {
        return Err(Error(format!(
            "Gestalt template requires {required} UV channels, source contains {tex_coords}. Restore missing UV channels before export"
        )));
    }
    if load(mesh + mesh_layout::COLOR_FLAGS)? & mesh_layout::HAS_VERTEX_COLORS != 0
        && fixture.skin.colors.is_empty()
    {
        return Err(Error(
            "Gestalt template requires COLOR_0. Restore vertex colors before export".into(),
        ));
    }
    Ok(())
}

fn reject_gestalt_references(mesh: usize) -> Result<()> {
    let Some(class) =
        unrealsdk_rs::objects::find_class("GearboxFramework.GestaltSkeletalMeshDefinition")
    else {
        return Ok(());
    };
    let property = struct_fields(class.cast())
        .into_iter()
        .find(|p| object_name_text(p.cast()) == "GestaltSkeletalMesh")
        .ok_or_else(|| Error("Gestalt mesh property unavailable".into()))?;
    if property_class_name(property).as_deref() != Some("ObjectProperty")
        || property_element_size(property) != Some(WORD_BYTES as i32)
        || property_array_dim(property) != Some(1)
    {
        return Err(Error("Gestalt mesh property layout differs".into()));
    }
    let offset = property_offset(property)
        .filter(|&offset| offset >= 0)
        .ok_or_else(|| Error("Gestalt mesh property offset unavailable".into()))?
        as usize;
    for object in objects().ok_or_else(|| Error("GObjects unavailable".into()))? {
        if !object.is_null()
            && is_kind_of(object, class)
                .ok_or_else(|| Error("Gestalt definition classification unavailable".into()))?
            && load(object as usize + offset)? as usize == mesh
        {
            return Err(Error("generic geometry replacement cannot update a mesh referenced by a Gestalt definition. Use the dedicated Gestalt operations".into()));
        }
    }
    Ok(())
}

fn component_users(
    api: &Reload,
    mesh: usize,
    publication: Option<&Publication>,
) -> Result<Vec<usize>> {
    let class = construct::require_class("Engine.SkeletalMeshComponent")?;
    let property = struct_fields(class.cast())
        .into_iter()
        .find(|p| object_name_text(p.cast()) == "SkeletalMesh")
        .ok_or_else(|| Error("SkeletalMeshComponent.SkeletalMesh unavailable".into()))?;
    if property_class_name(property).as_deref() != Some("ObjectProperty")
        || property_offset(property) != Some(component_layout::MESH as i32)
        || property_element_size(property) != Some(WORD_BYTES as i32)
        || property_array_dim(property) != Some(1)
    {
        return Err(Error(
            "SkeletalMeshComponent reflection layout differs".into(),
        ));
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
        let component = object as usize;
        let gestalt_reference = publication
            .map(|p| p.references(component))
            .transpose()?
            .unwrap_or(false);
        if load(component + component_layout::MESH)? as usize != mesh && !gestalt_reference {
            continue;
        }
        construct::verify_live(
            "skeletal mesh component",
            construct::flags_of("component", object)?,
        )?;
        let table = load(component)? as usize;
        for (offset, expected) in [
            (component_layout::DETACH_DISPATCH, api.component_detach),
            (component_layout::ATTACH_DISPATCH, api.component_attach),
            (component_layout::VALIDITY_DISPATCH, api.component_valid),
        ] {
            construct::verify_dispatch(
                "skeletal component dispatch",
                load(table + offset)?,
                expected,
            )?;
        }
        let count = load(component + component_layout::LOD_DATA_COUNT)?;
        if count > 1 {
            return Err(Error(
                "skeletal reload refuses multi-LOD component state".into(),
            ));
        }
        if count == 1 {
            let entry = load(component + component_layout::LOD_DATA)? as usize;
            if load(entry + component_layout::LOD_INFLUENCES)? != 0
                || load(entry + component_layout::LOD_FLAGS)?
                    & component_layout::OVERRIDE_INFLUENCES
                    != 0
                || read(entry + component_layout::LOD_SKIN_WEIGHTS, size_of::<u8>())?[0] != 0
                || load(entry + component_layout::LOD_INFLUENCE_INDEX)? != u32::MAX
            {
                return Err(Error(
                    "skeletal reload refuses per-component LOD overrides".into(),
                ));
            }
        }
        users.push(component);
    }
    Ok(users)
}

pub(crate) fn replace(
    path: &str,
    material_paths: &[&str],
    fixture: &SkinnedMesh,
    sections: &[crate::sections::CheckedSection],
    policy: crate::ReplacementPolicy,
) -> Result<String> {
    let _operation = session::reserve()?;
    replace_geometry(path, material_paths, fixture, sections, policy, false, None)
}

pub(crate) fn replace_gestalt(
    path: &str,
    material_paths: &[&str],
    fixture: &SkinnedMesh,
    sections: &[crate::sections::CheckedSection],
) -> Result<String> {
    let _operation = session::reserve()?;
    replace_geometry(
        path,
        material_paths,
        fixture,
        sections,
        crate::ReplacementPolicy::ExistingAsset,
        true,
        None,
    )
}

/// Named updates retain the caller's session across readback, staging and publication.
pub(crate) fn update_gestalt(
    mesh: &PreparedSkeletalMesh,
    materials: &[&str],
    publication: &mut Publication,
) -> Result<String> {
    let path = publication.mesh_path().to_owned();
    replace_geometry(
        &path,
        materials,
        mesh.mesh(),
        mesh.sections(),
        crate::ReplacementPolicy::ExistingAsset,
        true,
        Some(publication),
    )
}

fn replace_geometry(
    path: &str,
    material_paths: &[&str],
    fixture: &SkinnedMesh,
    sections: &[crate::sections::CheckedSection],
    policy: crate::ReplacementPolicy,
    gestalt: bool,
    mut publication: Option<&mut Publication>,
) -> Result<String> {
    let base = construct::admitted_base()?;
    let api = Reload::resolve(base, fixture.skin.index_stride)?;
    let object = reflection::skeletal_mesh(path)?;
    let mesh = object as usize;
    construct::verify_vtable("reload mesh", load(mesh)?, &api.mesh_vtable)?;
    if !gestalt {
        reject_gestalt_references(mesh)?;
    } else if load(mesh + mesh_layout::LOD_MODELS + abi::ARRAY_COUNT)? != 1 {
        return Err(Error(
            "in-place Gestalt operations require one LOD0 render model".into(),
        ));
    }
    let flags = construct::flags_of("reload mesh", object)?;
    construct::verify_live("reload mesh", flags)?;
    let known = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&mesh)
        .copied();
    if !gestalt && known == Some(RegisteredMesh::Gestalt) {
        return Err(Error("generic geometry replacement is disabled for Gestalt meshes. Use the dedicated Gestalt operations".into()));
    }
    if policy == crate::ReplacementPolicy::LibraryOwned && known.is_none() {
        return Err(Error(
            "reload accepts only library-created skeletal meshes".into(),
        ));
    }
    let previous = existing(mesh)?;
    compatible(&previous.skeleton, &fixture.skeleton)?;
    let skinning_flags = load(mesh + mesh_layout::SKINNING_FLAGS)?;
    if skinning_flags & mesh_layout::FORCE_CPU_SKINNING != 0
        || (policy == crate::ReplacementPolicy::LibraryOwned
            && skinning_flags & mesh_layout::FULL_PRECISION_UVS == 0)
    {
        return Err(Error(
            "skeletal reload requires full-UV GPU skinning".into(),
        ));
    }
    api.lifecycle.require_healthy_render_thread()?;
    let old_bounds = construct::read_bounds(mesh + mesh_layout::BOUNDS_ORIGIN)?;
    let bounds = expanded_bounds(old_bounds, fixture.skin.bounds)?;
    let users = component_users(&api, mesh, publication.as_deref())?;
    if let Some(publication) = publication.as_deref_mut() {
        publication.capture(&users)?;
    }
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
    let records = skeletal_native::stage(fixture, &previous.names, sections)?;
    let materials = construct::resolve_materials(material_paths)?;
    construct::retain_materials(&materials)?;
    let staged = storage::stage(&api.storage.geometry, &materials, fixture, &records)?;
    let canonical_path = construct::object_path("reload mesh", object)?;
    let mut contexts = vec![ReattachContext::default(); users.len()].into_boxed_slice();
    construct::retain_object("replacement mesh", object)?;
    let result = (|| {
        for (context, &component) in contexts.iter_mut().zip(&users) {
            api.lifecycle.detach(context, component);
            if load(component + abi::COMPONENT_SCENE)? != 0
                || read(component + abi::COMPONENT_FLAGS, size_of::<u8>())?[0]
                    & abi::COMPONENT_ATTACHED
                    != 0
                || load(component + component_layout::RENDER_OBJECT)? != 0
            {
                return Err(Error(
                    "skeletal component did not release its render object".into(),
                ));
            }
        }
        if component_users(&api, mesh, publication.as_deref())? != users {
            return Err(Error(
                "skeletal mesh users changed during detachment".into(),
            ));
        }
        if snapshot(mesh)? != previous.snapshot {
            return Err(Error("skeletal ownership changed during detachment".into()));
        }
        if let Some(publication) = publication.as_deref() {
            publication.verify()?;
        }
        api.lifecycle.release_and_wait(
            mesh as *mut native::SkeletalMesh,
            mesh_layout::RELEASE_FENCE,
        )?;
        storage::install(
            Region {
                address: mesh,
                length: mesh_layout::SIZE,
            },
            &staged,
            &fixture.skin,
            &bounds,
        )?;
        if policy == crate::ReplacementPolicy::ExistingAsset {
            let region = Region {
                address: mesh,
                length: mesh_layout::SIZE,
            };
            region.u32(
                mesh_layout::SKINNING_FLAGS,
                skinning_flags | mesh_layout::FULL_PRECISION_UVS,
            )?;
        }
        if let Some(publication) = publication.as_deref() {
            publication.install()?;
            publication.refresh()?;
        }
        // SAFETY: replacement skin storage is installed and the original skeleton is retained.
        unsafe { (api.storage.init_resources)(mesh as *mut native::SkeletalMesh) };
        for context in contexts.iter_mut().rev() {
            api.lifecycle.reattach(context);
        }
        for (&component, &(scene, attached)) in users.iter().zip(&attachment_states) {
            if load(component + abi::COMPONENT_SCENE)? != scene
                || read(component + abi::COMPONENT_FLAGS, size_of::<u8>())?[0]
                    & abi::COMPONENT_ATTACHED
                    != attached
            {
                return Err(Error(
                    "skeletal component attachment was not restored".into(),
                ));
            }
        }
        Ok(canonical_path)
    })();
    if result.is_err() {
        std::mem::forget(contexts);
    }
    result
}
