//! GPU skin encoding with per-chunk palette mapping and shared vertex streams.

use super::{
    GPU_VERTEX_STRIDE, Influences, MAX_PALETTE_BONES, ReferenceBone, SkinPayload, SkinWeights,
    SkinnedMesh, bone_closure, failure,
};
use crate::{
    Result,
    encoding::{self, Vertex},
};

/// Encodes engine-space geometry into palette-bounded GPU chunks.
/// Every reference bone remains required for evaluation, including unweighted helpers.
pub fn skinned(
    vertices: &[Vertex],
    indices: &[u32],
    weights: &[SkinWeights],
    skeleton: &[ReferenceBone],
) -> Result<SkinnedMesh> {
    skinned_with_uvs(vertices, indices, weights, skeleton, &[])
}

fn weighted_bones(
    vertices: usize,
    weights: &[SkinWeights],
    skeleton: &[ReferenceBone],
) -> Result<(Vec<u16>, Vec<u8>)> {
    if weights.len() != vertices {
        return Err(failure("skin influence count must match vertices"));
    }
    let mut used = std::collections::BTreeSet::new();
    for influence in weights {
        if influence.weights.iter().any(|w| !w.is_finite() || *w < 0.) {
            return Err(failure("skin weights must be finite and nonnegative"));
        }
        for slot in 0..influence.weights.len() {
            if influence.weights[slot] > 0. {
                let joint = usize::from(influence.joints[slot]);
                if joint >= skeleton.len() {
                    return Err(failure(
                        "skin influence joint exceeds the reference skeleton",
                    ));
                }
                used.insert(influence.joints[slot]);
            }
        }
    }
    bone_closure(skeleton, &used.into_iter().collect::<Vec<_>>())
}

/// Encodes a skinned mesh with zero through three additional full-precision UV channels.
/// The first UV pair comes from each vertex. Additional channels match the vertex count.
pub fn skinned_with_uvs(
    vertices: &[Vertex],
    indices: &[u32],
    weights: &[SkinWeights],
    skeleton: &[ReferenceBone],
    extra_uvs: &[Vec<[f32; 2]>],
) -> Result<SkinnedMesh> {
    if extra_uvs.len() > 3
        || extra_uvs.iter().any(|channel| {
            channel.len() != vertices.len() || channel.iter().flatten().any(|v| !v.is_finite())
        })
    {
        return Err(failure(
            "GPU skinning requires one through four finite UV channels matching the vertices",
        ));
    }
    let (active_bones, required_bones) = weighted_bones(vertices.len(), weights, skeleton)?;
    if active_bones.len() > MAX_PALETTE_BONES {
        return super::chunks::skinned(vertices, indices, weights, skeleton, extra_uvs);
    }
    let quantized = Influences::quantize_global(weights, &active_bones)?;
    let skin = encode(vertices, indices, &quantized, &active_bones)?;
    let mut fixture = SkinnedMesh {
        skin,
        skeleton: skeleton.to_vec(),
        active_bones,
        required_bones,
    };
    if extra_uvs.is_empty() {
        return Ok(fixture);
    }
    let skin = &mut fixture.skin;
    skin.tex_coords = 1 + extra_uvs.len();
    let byte_len = vertices
        .len()
        .checked_mul(skin.vertex_stride())
        .ok_or_else(|| failure("multi-UV skin size overflow"))?;
    let mut gpu = encoding::buffer(byte_len)?;
    for (index, record) in skin
        .gpu_vertices
        .chunks_exact(GPU_VERTEX_STRIDE)
        .enumerate()
    {
        gpu.extend_from_slice(record);
        for channel in extra_uvs {
            for value in channel[index] {
                gpu.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    skin.gpu_vertices = gpu;
    Ok(fixture)
}

/// Encodes already-converted geometry with one small chunk's local influences.
/// The shared geometry encoder validates bases, multi-size indices and bounds first.
pub fn encode(
    vertices: &[Vertex],
    indices: &[u32],
    influences: &[Influences],
    palette: &[u16],
) -> Result<SkinPayload> {
    if palette.is_empty() || palette.len() > MAX_PALETTE_BONES {
        return Err(failure("chunk palette exceeds the GPU bone budget"));
    }
    for a in 0..palette.len() {
        for b in a + 1..palette.len() {
            if palette[a] == palette[b] {
                return Err(failure("chunk palette contains duplicate bones"));
            }
        }
    }
    if influences.len() != vertices.len() {
        return Err(failure("skin influence count must match vertices"));
    }
    if influences.iter().any(|inf| {
        inf.bones().iter().any(|&b| usize::from(b) >= palette.len())
            || inf.weights().iter().map(|&w| u16::from(w)).sum::<u16>() != u16::from(u8::MAX)
    }) {
        return Err(failure("skin influences do not match the selected palette"));
    }
    let byte_len = vertices
        .len()
        .checked_mul(GPU_VERTEX_STRIDE)
        .ok_or_else(|| failure("skin vertex byte size overflow"))?;
    let index_stride = if vertices.len() > encoding::MAX_COMPACT_VERTICES {
        size_of::<u32>()
    } else {
        encoding::INDEX_STRIDE
    };
    let geometry = encoding::encode_with_index_width(vertices, indices, index_stride)?;
    let mut gpu_vertices = encoding::buffer(byte_len)?;
    let mut max_bone_influences = 0;
    for (i, inf) in influences.iter().enumerate() {
        // BL2 SetVertex: tangent X/Z at 0/4, local bones/weights at 8/12,
        // XYZ at 16, full UV1 at 28. NOT the static mesh's split streams.
        let tangent_start = i * encoding::TANGENT_UV_STRIDE;
        gpu_vertices.extend_from_slice(
            &geometry.tangent_uv[tangent_start..tangent_start + encoding::PACKED_TANGENTS_BYTES],
        );
        gpu_vertices.extend_from_slice(&inf.bones());
        gpu_vertices.extend_from_slice(&inf.weights());
        let position_start = i * encoding::POSITION_STRIDE;
        gpu_vertices.extend_from_slice(
            &geometry.positions[position_start..position_start + encoding::POSITION_STRIDE],
        );
        gpu_vertices.extend_from_slice(
            &geometry.tangent_uv[tangent_start + encoding::PACKED_TANGENTS_BYTES
                ..tangent_start + encoding::TANGENT_UV_STRIDE],
        );
        max_bone_influences =
            max_bone_influences.max(inf.weights().iter().filter(|&&w| w != 0).count());
    }
    Ok(SkinPayload {
        gpu_vertices,
        colors: geometry.colors,
        tex_coords: 1,
        indices: geometry.indices,
        index_stride: geometry.index_stride,
        palette: palette.to_vec(),
        chunks: vec![super::SkinChunk {
            base_vertex: 0,
            vertices: geometry.vertices,
            first_index: 0,
            triangles: geometry.triangles,
            palette: palette.to_vec(),
            max_bone_influences: max_bone_influences as u32,
        }],
        vertices: geometry.vertices,
        triangles: geometry.triangles,
        max_bone_influences: max_bone_influences as u32,
        bounds: geometry.bounds,
    })
}
