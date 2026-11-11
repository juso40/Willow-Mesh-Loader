//! Fresh skeletal mesh admission, reference caches and native resource initialization.

use crate::engine::{
    construct::{self, load, retain_bytes},
    native, session,
};
use crate::skeletal::{
    bindings::Create,
    layout::mesh as mesh_layout,
    records::{self as skeletal_native, NameWords},
    storage,
};
use crate::{
    Error, Result,
    sections::CheckedSection,
    skeletal::{ReferenceBone, SkinnedMesh},
};
use unrealsdk_rs::objects::fname_from_str;

fn stage_bone_names(skeleton: &[ReferenceBone]) -> Result<Vec<NameWords>> {
    let mut names = Vec::with_capacity(skeleton.len());
    for bone in skeleton {
        let name = fname_from_str(&bone.name, 0)
            .ok_or_else(|| Error("reference bone FName creation failed".into()))?;
        names.push(NameWords {
            index: name.index,
            number: name.number,
        });
    }
    Ok(names)
}

pub(super) fn populate_new(
    path: &str,
    material_paths: &[&str],
    fixture: &SkinnedMesh,
    sections: &[CheckedSection],
) -> Result<String> {
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let api = Create::resolve(base, fixture.skin.index_stride)?;
    let mesh = construct::fresh_object(
        path,
        "Engine.SkeletalMesh",
        mesh_layout::SIZE,
        &[
            mesh_layout::LOD_MODELS,
            mesh_layout::LOD_INFO,
            mesh_layout::REFERENCE_SKELETON,
            mesh_layout::MATERIALS,
            mesh_layout::REFERENCE_ATOMS,
        ],
    )?;
    construct::verify_vtable(
        "fresh skeletal mesh",
        load(mesh.address)?,
        &crate::engine::signatures::SKELETAL_MESH_VTABLE.resolve(base)?,
    )?;
    let materials = construct::resolve_materials(material_paths)?;
    let names = stage_bone_names(&fixture.skeleton)?;
    let records = skeletal_native::stage(fixture, &names, sections)?;
    let skin = &fixture.skin;
    construct::retain_materials(&materials)?;
    construct::retain_object(
        "fresh skeletal mesh",
        mesh.address as *mut unrealsdk_rs::types::UObject,
    )?;
    let mut flag_word = load(mesh.address + mesh_layout::SKINNING_FLAGS)?;
    if flag_word & mesh_layout::FORCE_CPU_SKINNING != 0 {
        return Err(Error(
            "fresh SkeletalMesh forces unsupported CPU skinning".into(),
        ));
    }
    flag_word |= mesh_layout::FULL_PRECISION_UVS;
    let bones = retain_bytes(&records.bones)?;
    let staged = storage::stage(&api.storage.geometry, &materials, fixture, &records)?;
    storage::install(mesh, &staged, skin, &skin.bounds)?;
    mesh.u32(mesh_layout::SKELETON_DEPTH, records.skeleton_depth)?;
    mesh.u32(mesh_layout::SKINNING_FLAGS, flag_word)?;
    mesh.header(
        mesh_layout::REFERENCE_SKELETON,
        bones.address,
        (records.bones.len() / skeletal_native::BONE_SIZE) as u32,
    )?;
    // SAFETY: the SDK-created mesh has its validated reference skeleton installed.
    // These count-gated reference caches are initialized once, not copied on reload.
    unsafe {
        let receiver = mesh.address as *mut native::SkeletalMesh;
        (api.init_name_index_map)(receiver);
        (api.calculate_inv_ref_matrices)(receiver);
        (api.build_ref_bone_atom_cache)(receiver);
    }
    // SAFETY: the library-owned skeletal mesh and reference caches are complete.
    unsafe { (api.storage.init_resources)(mesh.address as *mut native::SkeletalMesh) };
    crate::skeletal::replacement::register(mesh.address)?;
    construct::object_path(
        "skeletal mesh",
        mesh.address as *mut unrealsdk_rs::types::UObject,
    )
}
