//! Packed geometry concatenation for named Gestalt additions without changing old ranges.

use super::{SkinnedMesh, skeleton};
use crate::{Error, Result, encoding, sections};

/// Joins compatible encoded meshes while retaining both triangle orders and palettes.
/// Incoming materials occupy fresh slots even when their UObject paths are equal.
/// Missing UV channels and colors receive zero UVs and opaque white, respectively.
pub fn concatenate(
    left: SkinnedMesh,
    right: &SkinnedMesh,
    left_sections: &[sections::Section],
    right_sections: &[sections::Section],
    left_materials: usize,
    right_materials: usize,
) -> Result<(SkinnedMesh, Vec<sections::CheckedSection>)> {
    let materials = left_materials
        .checked_add(right_materials)
        .ok_or_else(|| Error("Gestalt material count overflow".into()))?;
    join(
        left,
        right,
        left_sections,
        right_sections,
        left_materials,
        materials,
    )
}

pub(super) fn join(
    mut left: SkinnedMesh,
    right: &SkinnedMesh,
    left_sections: &[sections::Section],
    right_sections: &[sections::Section],
    material_offset: usize,
    materials: usize,
) -> Result<(SkinnedMesh, Vec<sections::CheckedSection>)> {
    skeleton::compatible(&left.skeleton, &right.skeleton)?;
    let vertices = left
        .skin
        .vertices
        .checked_add(right.skin.vertices)
        .filter(|&n| n <= i32::MAX as usize)
        .ok_or_else(|| Error("Gestalt vertex count overflow".into()))?;
    let first_index = left
        .skin
        .triangles
        .checked_mul(encoding::TRIANGLE_INDICES)
        .ok_or_else(|| Error("Gestalt index count overflow".into()))?;
    let triangles = left
        .skin
        .triangles
        .checked_add(right.skin.triangles)
        .filter(|&n| n <= i32::MAX as usize / encoding::TRIANGLE_INDICES)
        .ok_or_else(|| Error("Gestalt triangle count overflow".into()))?;
    if materials == 0 || materials > usize::from(u16::MAX) + 1 {
        return Err(Error(
            "Gestalt material count exceeds native ordinals".into(),
        ));
    }
    let mut indices = indices(&left.skin)?;
    let vertex_offset = u32::try_from(left.skin.vertices)
        .map_err(|_| Error("Gestalt vertex offset overflow".into()))?;
    for index in self::indices(&right.skin)? {
        indices.push(
            index
                .checked_add(vertex_offset)
                .ok_or_else(|| Error("Gestalt index overflow".into()))?,
        );
    }
    if ![left.skin.tex_coords, right.skin.tex_coords]
        .iter()
        .all(|n| (1..=4).contains(n))
    {
        return Err(Error(
            "Gestalt GPU skinning requires one through four UV channels".into(),
        ));
    }
    let tex_coords = left.skin.tex_coords.max(right.skin.tex_coords);
    let stride = 28 + tex_coords * 8;
    let mut gpu = Vec::new();
    gpu.try_reserve_exact(
        vertices
            .checked_mul(stride)
            .ok_or_else(|| Error("Gestalt GPU byte size overflow".into()))?,
    )
    .map_err(|_| Error("Gestalt GPU allocation failed".into()))?;
    for skin in [&left.skin, &right.skin] {
        append_vertices(&mut gpu, skin, tex_coords)?;
    }
    let mut colors = Vec::new();
    if !left.skin.colors.is_empty() || !right.skin.colors.is_empty() {
        for skin in [&left.skin, &right.skin] {
            if skin.colors.is_empty() {
                colors.resize(colors.len() + skin.vertices * 4, 255);
            } else if skin.colors.len() == skin.vertices * 4 {
                colors.extend_from_slice(&skin.colors);
            } else {
                return Err(Error(
                    "Gestalt color stream differs from its vertex count".into(),
                ));
            }
        }
    }
    let mut sections = left_sections.to_vec();
    for section in right_sections {
        sections.push(sections::Section {
            first_index: section
                .first_index
                .checked_add(first_index)
                .ok_or_else(|| Error("Gestalt section offset overflow".into()))?,
            triangles: section.triangles,
            material_index: section
                .material_index
                .checked_add(material_offset)
                .ok_or_else(|| Error("Gestalt material offset overflow".into()))?,
        });
    }
    let sections = sections::validate(&indices, vertices, &sections, materials)?;
    let old_vertices = left.skin.vertices;
    for chunk in &right.skin.chunks {
        let mut chunk = chunk.clone();
        chunk.base_vertex += old_vertices;
        chunk.first_index += first_index;
        left.skin.chunks.push(chunk);
    }
    let mut palette = left.active_bones.clone();
    palette.extend_from_slice(&right.active_bones);
    let (active, required) = super::bone_closure(&left.skeleton, &palette)?;
    let bounds = encoding::bounds_from_positions(gpu.chunks_exact(stride).map(|record| {
        std::array::from_fn(|axis| {
            f32::from_le_bytes(std::array::from_fn(|byte| record[16 + axis * 4 + byte]))
        })
    }))?;
    left.skin.gpu_vertices = gpu;
    left.skin.colors = colors;
    left.skin.tex_coords = tex_coords;
    left.skin.vertices = vertices;
    left.skin.triangles = triangles;
    left.skin.bounds = bounds;
    left.skin.max_bone_influences = left
        .skin
        .max_bone_influences
        .max(right.skin.max_bone_influences);
    left.skin.palette = active.clone();
    left.active_bones = active;
    left.required_bones = required;
    left.skin.index_stride = if vertices > encoding::MAX_COMPACT_VERTICES {
        4
    } else {
        2
    };
    let index_bytes = indices
        .len()
        .checked_mul(left.skin.index_stride)
        .ok_or_else(|| Error("Gestalt index byte size overflow".into()))?;
    let mut packed_indices = Vec::new();
    packed_indices
        .try_reserve_exact(index_bytes)
        .map_err(|_| Error("Gestalt index allocation failed".into()))?;
    for index in indices {
        packed_indices.extend_from_slice(&index.to_le_bytes()[..left.skin.index_stride]);
    }
    left.skin.indices = packed_indices;
    Ok((left, sections))
}

pub(super) fn append_vertices(
    gpu: &mut Vec<u8>,
    skin: &super::SkinPayload,
    tex_coords: usize,
) -> Result<()> {
    if !(skin.tex_coords..=4).contains(&tex_coords)
        || !(1..=4).contains(&skin.tex_coords)
        || skin.vertices.checked_mul(skin.vertex_stride()) != Some(skin.gpu_vertices.len())
    {
        return Err(Error("Gestalt GPU stream does not match its layout".into()));
    }
    let bytes = skin
        .vertices
        .checked_mul(28 + tex_coords * 8)
        .ok_or_else(|| Error("Gestalt GPU byte size overflow".into()))?;
    gpu.try_reserve_exact(bytes)
        .map_err(|_| Error("Gestalt GPU allocation failed".into()))?;
    for record in skin.gpu_vertices.chunks_exact(skin.vertex_stride()) {
        gpu.extend_from_slice(record);
        gpu.resize(gpu.len() + (tex_coords - skin.tex_coords) * 8, 0);
    }
    Ok(())
}

/// Decodes a validated-width index stream without truncating u32 indices.
pub fn indices(skin: &super::SkinPayload) -> Result<Vec<u32>> {
    let bytes = skin
        .triangles
        .checked_mul(encoding::TRIANGLE_INDICES)
        .and_then(|n| n.checked_mul(skin.index_stride));
    if ![2, 4].contains(&skin.index_stride) || bytes != Some(skin.indices.len()) {
        return Err(Error(
            "Gestalt index stream does not match its layout".into(),
        ));
    }
    skin.indices
        .chunks_exact(skin.index_stride)
        .map(|bytes| {
            let index = if bytes.len() == 2 {
                u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
            } else {
                u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            };
            if index as usize >= skin.vertices {
                return Err(Error("Gestalt index exceeds the vertex stream".into()));
            }
            Ok(index)
        })
        .collect()
}

/// Converts a native IEEE binary16 UV component to a full-precision float.
pub fn half(value: u16) -> f32 {
    let sign = (u32::from(value) & 0x8000) << 16;
    let exponent = (value >> 10) & 31;
    let mantissa = u32::from(value & 1023);
    if exponent == 0 {
        if mantissa == 0 {
            return f32::from_bits(sign);
        }
        let value = (mantissa as f32) * (2.0f32).powi(-24);
        return if sign == 0 { value } else { -value };
    }
    let exponent = if exponent == 31 {
        255
    } else {
        u32::from(exponent) + 112
    };
    f32::from_bits(sign | (exponent << 23) | (mantissa << 13))
}
