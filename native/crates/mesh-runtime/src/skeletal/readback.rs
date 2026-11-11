//! Checked LOD0 GPU readback for extending or updating the current Gestalt master.

use super::{
    PreparedSkeletalMesh, SkinChunk, SkinPayload, SkinnedMesh,
    bindings::Reload,
    gestalt,
    layout::{chunk, index_resource, lod, lod_info, mesh},
    records,
};
use crate::{
    Error, Result, encoding,
    engine::{
        abi,
        construct::{self, load, read},
        reflection, session,
    },
    sections,
};

fn short(bytes: &[u8], offset: usize) -> Result<u16> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| Error("truncated skeletal record".into()))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}
fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    crate::engine::memory::word(bytes, offset)
}

pub(super) fn read_current(path: &str) -> Result<(PreparedSkeletalMesh, Vec<String>)> {
    let _operation = session::reserve()?;
    read_current_in_session(path)
}

/// The caller holds the native operation session across master and definition staging.
pub(super) fn read_current_in_session(path: &str) -> Result<(PreparedSkeletalMesh, Vec<String>)> {
    let base = construct::admitted_base()?;
    let object = reflection::skeletal_mesh(path)?;
    let address = object as usize;
    construct::verify_live(
        "Gestalt master",
        construct::flags_of("Gestalt master", object)?,
    )?;
    construct::verify_lods(address, mesh::LOD_MODELS, mesh::LOD_INFO, lod::SIZE)?;
    if load(address + mesh::LOD_MODELS + abi::ARRAY_COUNT)? != 1 {
        return Err(Error(
            "Gestalt readback requires one LOD0 render model".into(),
        ));
    }
    let info = load(address + mesh::LOD_INFO)? as usize;
    let material_map =
        construct::read_array(info + lod_info::MATERIAL_MAP, 4, usize::from(u16::MAX) + 1)?;
    for (index, value) in material_map.chunks_exact(4).enumerate() {
        if word(value, 0)? as usize != index {
            return Err(Error(
                "Gestalt readback refuses a nonidentity LOD material remap".into(),
            ));
        }
    }
    let lod_address = load(load(address + mesh::LOD_MODELS)? as usize)? as usize;
    let index_stride = read(lod_address + lod::INDEX_TYPE_SIZE, 1)?[0] as usize;
    let api = Reload::resolve(base, index_stride)?;
    construct::verify_vtable("Gestalt master", load(address)?, &api.mesh_vtable)?;
    if load(address + mesh::SKINNING_FLAGS)? & mesh::FORCE_CPU_SKINNING != 0 {
        return Err(Error("Gestalt readback requires GPU skinning".into()));
    }
    let skeleton = super::reference::read(address)?.0;
    let vertices = load(lod_address + lod::GPU_VERTICES)? as usize;
    let tex_coords = load(lod_address + lod::GPU_TEX_COORDS)? as usize;
    let stride = load(lod_address + lod::GPU_STRIDE)? as usize;
    let full_uvs = load(lod_address + lod::FULL_PRECISION_UVS)? != 0;
    if vertices == 0
        || vertices != load(lod_address + lod::VERTICES)? as usize
        || !(1..=4).contains(&tex_coords)
        || tex_coords != load(lod_address + lod::TEX_COORDS)? as usize
        || stride != 28 + tex_coords * if full_uvs { 8 } else { 4 }
    {
        return Err(Error("Gestalt master requires readable full-XYZ GPU vertices and one through four UV channels".into()));
    }
    let bytes = vertices
        .checked_mul(stride)
        .filter(|&n| n <= i32::MAX as usize)
        .ok_or_else(|| Error("Gestalt master GPU byte count overflow".into()))?;
    let implementation = construct::vertex_storage(lod_address + lod::GPU_IMPLEMENTATION)?;
    let gpu = construct::read_array(
        implementation.address + abi::vertex_impl::ARRAY,
        stride,
        vertices,
    )?;
    if gpu.len() != bytes
        || load(implementation.address + abi::vertex_impl::ARRAY)?
            != load(lod_address + lod::GPU_DATA)?
    {
        return Err(Error(
            "Gestalt master GPU vertices are not CPU-readable".into(),
        ));
    }
    let normalized_bytes = vertices
        .checked_mul(28 + tex_coords * 8)
        .filter(|&n| n <= i32::MAX as usize)
        .ok_or_else(|| Error("normalized Gestalt GPU size overflow".into()))?;
    let mut gpu_vertices = Vec::with_capacity(normalized_bytes);
    for record in gpu.chunks_exact(stride) {
        gpu_vertices.extend_from_slice(&record[..28]);
        if full_uvs {
            if record[28..].chunks_exact(4).any(|bytes| {
                !f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).is_finite()
            }) {
                return Err(Error("Gestalt master contains nonfinite UVs".into()));
            }
            gpu_vertices.extend_from_slice(&record[28..]);
        } else {
            for value in record[28..].chunks_exact(2) {
                let uv = gestalt::half(u16::from_le_bytes([value[0], value[1]]));
                if !uv.is_finite() {
                    return Err(Error("Gestalt master contains nonfinite UVs".into()));
                }
                gpu_vertices.extend_from_slice(&uv.to_le_bytes());
            }
        }
    }
    let index_object = load(lod_address + lod::INDEX_RESOURCE)? as usize;
    construct::verify_vtable(
        "Gestalt index resource",
        load(index_object)?,
        &api.storage.geometry.index_vtable,
    )?;
    let indices = construct::read_array(
        index_object + index_resource::INDICES,
        index_stride,
        i32::MAX as usize,
    )?;
    if indices.is_empty() || indices.len() % (index_stride * encoding::TRIANGLE_INDICES) != 0 {
        return Err(Error(
            "Gestalt master requires a readable triangle-list index buffer".into(),
        ));
    }
    let triangles = indices.len() / index_stride / encoding::TRIANGLE_INDICES;
    let material_bytes =
        construct::read_array(address + mesh::MATERIALS, 4, usize::from(u16::MAX) + 1)?;
    let materials = material_bytes
        .chunks_exact(4)
        .map(|bytes| {
            let pointer = word(bytes, 0)? as usize;
            if pointer == 0 {
                return Ok("EngineMaterials.DefaultMaterial".to_owned());
            }
            construct::object_path(
                "Gestalt material",
                pointer as *mut unrealsdk_rs::types::UObject,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let chunk_bytes = construct::read_array(
        lod_address + lod::CHUNKS,
        records::CHUNK_SIZE,
        usize::from(u16::MAX) + 1,
    )?;
    let mut chunks = Vec::new();
    let chunk_address = load(lod_address + lod::CHUNKS)? as usize;
    let mut vertex_cursor = 0;
    for (index, bytes) in chunk_bytes.chunks_exact(records::CHUNK_SIZE).enumerate() {
        let base_vertex = word(bytes, chunk::BASE_VERTEX)? as usize;
        let count = (word(bytes, chunk::RIGID_COUNT)? as usize)
            .checked_add(word(bytes, chunk::SOFT_COUNT)? as usize)
            .ok_or_else(|| Error("Gestalt chunk count overflow".into()))?;
        if base_vertex != vertex_cursor || count == 0 || count > vertices - vertex_cursor {
            return Err(Error(
                "Gestalt master chunk vertices must form a contiguous partition".into(),
            ));
        }
        let palette = construct::read_array(
            chunk_address + index * records::CHUNK_SIZE + chunk::BONE_MAP,
            2,
            super::MAX_PALETTE_BONES,
        )?
        .chunks_exact(2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
        .collect::<Vec<_>>();
        if palette.is_empty() || palette.iter().any(|&bone| bone as usize >= skeleton.len()) {
            return Err(Error(
                "Gestalt master contains an invalid bone palette".into(),
            ));
        }
        chunks.push(SkinChunk {
            base_vertex,
            vertices: count,
            first_index: 0,
            triangles: 0,
            palette,
            max_bone_influences: gpu_vertices[base_vertex * (28 + tex_coords * 8)
                ..(base_vertex + count) * (28 + tex_coords * 8)]
                .chunks_exact(28 + tex_coords * 8)
                .map(|record| record[12..16].iter().filter(|&&weight| weight != 0).count() as u32)
                .max()
                .unwrap_or(0),
        });
        vertex_cursor += count;
    }
    if vertex_cursor != vertices {
        return Err(Error(
            "Gestalt chunks do not cover the vertex stream".into(),
        ));
    }
    let section_bytes = construct::read_array(
        lod_address + lod::SECTIONS,
        records::SECTION_SIZE,
        i32::MAX as usize,
    )?;
    let mut sections = Vec::new();
    let mut cursor = 0;
    let mut previous_chunk = 0;
    for bytes in section_bytes.chunks_exact(records::SECTION_SIZE) {
        let chunk_index = short(bytes, 2)? as usize;
        let first_index = word(bytes, 4)? as usize;
        let count = word(bytes, 8)? as usize;
        if first_index != cursor
            || count == 0
            || chunk_index < previous_chunk
            || chunk_index > previous_chunk + 1
        {
            return Err(Error(
                "Gestalt sections must preserve contiguous chunk and triangle order".into(),
            ));
        }
        let chunk = chunks
            .get_mut(chunk_index)
            .ok_or_else(|| Error("Gestalt section references an absent chunk".into()))?;
        if chunk.triangles == 0 {
            chunk.first_index = first_index;
        }
        chunk.triangles = chunk
            .triangles
            .checked_add(count)
            .ok_or_else(|| Error("Gestalt chunk triangle count overflow".into()))?;
        previous_chunk = chunk_index;
        cursor = count
            .checked_mul(3)
            .and_then(|n| cursor.checked_add(n))
            .ok_or_else(|| Error("Gestalt section count overflow".into()))?;
        sections.push(sections::Section {
            first_index,
            triangles: count,
            material_index: short(bytes, 0)? as usize,
        });
    }
    if cursor != triangles * 3 || chunks.iter().any(|chunk| chunk.triangles == 0) {
        return Err(Error(
            "Gestalt sections do not cover the index stream".into(),
        ));
    }
    let has_colors = load(address + mesh::COLOR_FLAGS)? & mesh::HAS_VERTEX_COLORS != 0;
    let colors = if has_colors {
        crate::engine::colors::read(lod_address + lod::COLORS, vertices)?
    } else {
        Vec::new()
    };
    if has_colors && colors.is_empty() {
        return Err(Error(
            "Gestalt master vertex colors are not CPU-readable".into(),
        ));
    }
    let bounds =
        encoding::bounds_from_positions(gpu_vertices.chunks_exact(28 + tex_coords * 8).map(
            |record| {
                std::array::from_fn(|axis| {
                    f32::from_le_bytes(std::array::from_fn(|byte| record[16 + axis * 4 + byte]))
                })
            },
        ))?;
    let used = chunks
        .iter()
        .flat_map(|chunk| chunk.palette.iter().copied())
        .collect::<Vec<_>>();
    let (active_bones, required_bones) = super::bone_closure(&skeleton, &used)?;
    let max_bone_influences = chunks
        .iter()
        .map(|chunk| chunk.max_bone_influences)
        .max()
        .unwrap_or(0);
    let skin = SkinPayload {
        gpu_vertices,
        colors,
        tex_coords,
        indices,
        index_stride,
        palette: active_bones.clone(),
        chunks,
        vertices,
        triangles,
        max_bone_influences,
        bounds,
    };
    let decoded = gestalt::indices(&skin)?;
    let checked = sections::validate(&decoded, vertices, &sections, materials.len())?;
    validate_influences(&skin, &decoded)?;
    Ok((
        PreparedSkeletalMesh::encoded(
            SkinnedMesh {
                skin,
                skeleton,
                active_bones,
                required_bones,
            },
            checked,
            materials.len(),
        ),
        materials,
    ))
}

fn validate_influences(skin: &SkinPayload, indices: &[u32]) -> Result<()> {
    for chunk in &skin.chunks {
        for &index in &indices[chunk.first_index..chunk.first_index + chunk.triangles * 3] {
            if !(chunk.base_vertex..chunk.base_vertex + chunk.vertices).contains(&(index as usize))
            {
                return Err(Error(
                    "Gestalt section index escapes its bone-palette chunk".into(),
                ));
            }
        }
        for vertex in chunk.base_vertex..chunk.base_vertex + chunk.vertices {
            let record = &skin.gpu_vertices
                [vertex * skin.vertex_stride()..(vertex + 1) * skin.vertex_stride()];
            if record[12..16]
                .iter()
                .map(|&weight| u16::from(weight))
                .sum::<u16>()
                != 255
                || (0..4).any(|slot| {
                    record[12 + slot] != 0 && record[8 + slot] as usize >= chunk.palette.len()
                })
            {
                return Err(Error(
                    "Gestalt master GPU influences do not match their palette".into(),
                ));
            }
        }
    }
    Ok(())
}
