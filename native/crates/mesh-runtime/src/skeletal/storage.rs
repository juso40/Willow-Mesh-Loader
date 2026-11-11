//! Shared native skeletal LOD staging and publication for population and replacement.

use crate::engine::{
    abi::{WORD_BYTES, vertex_impl},
    colors,
    construct::{self, Region, load, retain_allocation, retain_bytes},
    native,
};
use crate::skeletal::layout::{
    chunk as chunk_layout, index_resource, lod as lod_layout, lod_info as info_layout,
    mesh as mesh_layout,
};
use crate::{
    Error, Result,
    skeletal::{SkinnedMesh, bindings::Geometry, records::NativeRecords},
};

pub(crate) struct StagedGeometry {
    material_slots: Region,
    indirect: Region,
    lod_info: Region,
    material_count: u32,
}

pub(crate) fn stage(
    api: &Geometry,
    materials: &[*mut unrealsdk_rs::types::UObject],
    fixture: &SkinnedMesh,
    records: &NativeRecords,
) -> Result<StagedGeometry> {
    let skin = &fixture.skin;
    let active = retain_bytes(&records.active_bones)?;
    let required = retain_bytes(&records.required_bones)?;
    let section_bytes = retain_bytes(&records.section_records)?;
    let chunks = retain_bytes(&records.chunks)?;
    let mut bone_offset = 0;
    for (index, chunk) in skin.chunks.iter().enumerate() {
        let offset = index * crate::skeletal::records::CHUNK_SIZE;
        let first = chunk.base_vertex * crate::skeletal::records::SOFT_SKIN_VERTEX_SIZE;
        let end = first + chunk.vertices * crate::skeletal::records::SOFT_SKIN_VERTEX_SIZE;
        let bone_end = bone_offset + chunk.palette.len() * size_of::<u16>();
        // Each nested TArray owns its allocation, not an interior pointer into
        // another chunk's storage, so native destruction remains well-formed.
        let soft = retain_bytes(&records.soft_vertices[first..end])?;
        let bone_map = retain_bytes(&records.bone_map[bone_offset..bone_end])?;
        chunks.header(
            offset + chunk_layout::SOFT_VERTICES,
            soft.address,
            chunk.vertices as u32,
        )?;
        chunks.header(
            offset + chunk_layout::BONE_MAP,
            bone_map.address,
            chunk.palette.len() as u32,
        )?;
        bone_offset = bone_end;
    }
    let lod_material_map = retain_bytes(&records.lod_material_map)?;
    let shadow = retain_bytes(&records.shadow_elements)?;
    let lod_info = retain_bytes(&records.lod_info)?;
    lod_info.header(
        info_layout::MATERIAL_MAP,
        lod_material_map.address,
        (records.lod_material_map.len() / WORD_BYTES) as u32,
    )?;
    lod_info.header(
        info_layout::SHADOW_ELEMENTS,
        shadow.address,
        records.section_count,
    )?;
    let material_slots = retain_allocation(materials.len() * WORD_BYTES)?;
    for (index, material) in materials.iter().enumerate() {
        material_slots.u32(index * WORD_BYTES, *material as u32)?;
    }
    let indirect = retain_allocation(WORD_BYTES)?;
    let indices = retain_bytes(&skin.indices)?;
    // FMultiSizeIndexContainer uses its own resource and array vtables, not the static index class.
    let index_object = retain_allocation(index_resource::SIZE)?;
    index_object.u32(0, api.index_vtable.address() as u32)?;
    index_object.u32(
        index_resource::ARRAY_VTABLE,
        api.index_array_vtable.address() as u32,
    )?;
    index_object.header(
        index_resource::INDICES,
        indices.address,
        (skin.indices.len() / skin.index_stride) as u32,
    )?;
    index_object.u32(index_resource::CPU_ACCESS, 1)?;
    let lod = retain_allocation(lod_layout::SIZE)?;
    // SAFETY: the retained allocation has the reviewed SkeletalLod size and alignment.
    unsafe { (api.lod_constructor)(lod.address as *mut native::SkeletalLod) };
    // The full-XYZ/full-UV payload overrides the constructor's packed-position default.
    lod.u32(lod_layout::FULL_PRECISION_UVS, 1)?;
    lod.u32(lod_layout::PACKED_POSITION, 0)?;
    lod.u32(lod_layout::PROCESSED_PACKED_POSITION, 0)?;
    lod.u32(lod_layout::GPU_TEX_COORDS, skin.tex_coords as u32)?;
    // SAFETY: the constructed LOD owns this correctly sized GPU skin-buffer region.
    unsafe {
        (api.allocate_data)((lod.address + lod_layout::GPU_RESOURCE) as *mut native::SkinBuffer)
    };
    let implementation = construct::vertex_storage(lod.address + lod_layout::GPU_IMPLEMENTATION)?;
    let gpu = retain_bytes(&skin.gpu_vertices)?;
    implementation.header(vertex_impl::ARRAY, gpu.address, skin.vertices as u32)?;
    lod.u32(lod_layout::GPU_DATA, gpu.address as u32)?;
    lod.u32(lod_layout::GPU_STRIDE, skin.vertex_stride() as u32)?;
    lod.u32(lod_layout::GPU_VERTICES, skin.vertices as u32)?;
    colors::fill(
        api.color_allocate,
        Region {
            address: lod.address + lod_layout::COLORS,
            length: colors::SIZE,
        },
        &skin.colors,
        skin.vertices,
    )?;
    lod.header(
        lod_layout::SECTIONS,
        section_bytes.address,
        records.section_count,
    )?;
    lod.header(lod_layout::CHUNKS, chunks.address, skin.chunks.len() as u32)?;
    lod.header(
        lod_layout::ACTIVE_BONES,
        active.address,
        (records.active_bones.len() / size_of::<u16>()) as u32,
    )?;
    lod.header(
        lod_layout::REQUIRED_BONES,
        required.address,
        records.required_bones.len() as u32,
    )?;
    lod.u32(lod_layout::INDEX_CPU_ACCESS, 1)?;
    lod.write(lod_layout::INDEX_TYPE_SIZE, &[skin.index_stride as u8])?;
    lod.u32(lod_layout::INDEX_RESOURCE, index_object.address as u32)?;
    let render_bytes = skin
        .gpu_vertices
        .len()
        .checked_add(skin.indices.len())
        .and_then(|count| count.checked_add(skin.colors.len()))
        .ok_or_else(|| Error("skeletal render size overflow".into()))?;
    lod.u32(lod_layout::RENDER_BYTES, render_bytes as u32)?;
    lod.u32(lod_layout::VERTICES, skin.vertices as u32)?;
    lod.u32(lod_layout::TEX_COORDS, skin.tex_coords as u32)?;
    indirect.u32(0, lod.address as u32)?;
    Ok(StagedGeometry {
        material_slots,
        indirect,
        lod_info,
        material_count: materials.len() as u32,
    })
}

pub(crate) fn install(
    mesh: Region,
    staged: &StagedGeometry,
    skin: &crate::skeletal::SkinPayload,
    bounds: &crate::encoding::Bounds,
) -> Result<()> {
    for index in 0..bounds.origin.len() {
        mesh.f32(
            mesh_layout::BOUNDS_ORIGIN + index * WORD_BYTES,
            bounds.origin[index],
        )?;
        mesh.f32(
            mesh_layout::BOUNDS_EXTENT + index * WORD_BYTES,
            bounds.box_extent[index],
        )?;
        mesh.f32(
            mesh_layout::RENDER_BOUNDS_ORIGIN + index * WORD_BYTES,
            skin.bounds.origin[index],
        )?;
        mesh.f32(
            mesh_layout::RENDER_BOUNDS_EXTENT + index * WORD_BYTES,
            skin.bounds.box_extent[index],
        )?;
    }
    let color_flags = load(mesh.address + mesh_layout::COLOR_FLAGS)?;
    mesh.u32(
        mesh_layout::COLOR_FLAGS,
        (color_flags & !mesh_layout::HAS_VERTEX_COLORS)
            | if skin.colors.is_empty() {
                0
            } else {
                mesh_layout::HAS_VERTEX_COLORS
            },
    )?;
    mesh.f32(mesh_layout::BOUNDS_RADIUS, bounds.sphere_radius)?;
    mesh.f32(mesh_layout::RENDER_BOUNDS_RADIUS, skin.bounds.sphere_radius)?;
    mesh.header(
        mesh_layout::MATERIALS,
        staged.material_slots.address,
        staged.material_count,
    )?;
    mesh.header(mesh_layout::LOD_MODELS, staged.indirect.address, 1)?;
    mesh.header(mesh_layout::LOD_INFO, staged.lod_info.address, 1)?;
    Ok(())
}
