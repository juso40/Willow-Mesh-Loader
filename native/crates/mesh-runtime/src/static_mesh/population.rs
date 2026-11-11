//! Fresh static mesh admission and native resource initialization.

use crate::encoding::Payload;
use crate::engine::{construct, native, session};
use crate::static_mesh::{bindings::Storage, layout::mesh as mesh_layout, storage};
use crate::{Error, Result};

pub(super) fn populate_new(
    path: &str,
    material_paths: &[&str],
    payload: &Payload,
    sections: &[crate::sections::CheckedSection],
) -> Result<String> {
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let api = Storage::resolve(base)?;
    let mesh = construct::fresh_object(
        path,
        "Engine.StaticMesh",
        mesh_layout::SIZE,
        &[
            mesh_layout::LOD_MODELS,
            mesh_layout::LOD_INFO,
            mesh_layout::COLLISION_NODES,
            mesh_layout::COLLISION_TRIANGLES,
        ],
    )?;
    construct::verify_vtable(
        "fresh static mesh",
        construct::load(mesh.address)?,
        &crate::engine::signatures::STATIC_MESH_VTABLE.resolve(base)?,
    )?;
    if construct::load(mesh.address + mesh_layout::BODY_SETUP)? != 0 {
        return Err(Error(
            "fresh static population refuses existing collision".into(),
        ));
    }
    let materials = construct::resolve_materials(material_paths)?;
    let section_bytes = crate::sections::encode(
        sections,
        &materials
            .iter()
            .map(|material| *material as u32)
            .collect::<Vec<_>>(),
    )?;
    construct::retain_materials(&materials)?;
    construct::retain_object(
        "fresh static mesh",
        mesh.address as *mut unrealsdk_rs::types::UObject,
    )?;
    let staged = storage::stage(&api.geometry, payload, &section_bytes, sections.len())?;
    storage::install(mesh, &staged, payload)?;
    mesh.f32(mesh_layout::LOD_DISTANCE_RATIO, 1.)?;
    mesh.f32(
        mesh_layout::LOD_MAX_RANGE,
        mesh_layout::DEFAULT_LOD_MAX_RANGE,
    )?;
    mesh.u32(mesh_layout::LIGHT_MAP_RESOLUTION, 0)?;
    mesh.u32(mesh_layout::LIGHT_MAP_COORDINATE_INDEX, 0)?;
    mesh.u32(mesh_layout::FULL_PRECISION_UVS, 1)?;
    mesh.u32(mesh_layout::USED_FOR_INSTANCING, 0)?;
    mesh.u32(mesh_layout::CAN_BECOME_DYNAMIC, 1)?;
    mesh.f32(mesh_layout::STREAMING_DISTANCE_MULTIPLIER, 1.)?;
    // SAFETY: the SDK-created mesh has its complete reviewed static storage installed.
    unsafe { (api.init_resources)(mesh.address as *mut native::StaticMesh) };
    crate::static_mesh::replacement::register(mesh.address)?;
    construct::object_path(
        "static mesh",
        mesh.address as *mut unrealsdk_rs::types::UObject,
    )
}
