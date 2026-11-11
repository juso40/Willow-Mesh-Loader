//! Stable triangle partitioning into engine-sized bone palettes.
//!
//! Triangle order and index ranges stay unchanged, preserving material and Gestalt
//! fragment ranges. Vertices shared across chunk boundaries receive separate records.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    MAX_PALETTE_BONES, ReferenceBone, SkinChunk, SkinPayload, SkinWeights, SkinnedMesh,
    bone_closure, failure,
};
use crate::{
    Result,
    encoding::{self, TRIANGLE_INDICES, Vertex},
};

#[derive(Default)]
struct Partition {
    sources: Vec<usize>,
    indices: Vec<u32>,
    remap: BTreeMap<u32, u32>,
    bones: BTreeSet<u16>,
}

fn partition(indices: &[u32], weights: &[SkinWeights]) -> Result<Vec<Partition>> {
    if indices.is_empty()
        || !indices.len().is_multiple_of(TRIANGLE_INDICES)
        || indices.iter().any(|&index| index as usize >= weights.len())
    {
        return Err(failure("invalid chunk triangle index list"));
    }
    let mut partitions = Vec::new();
    let mut current = Partition::default();
    for triangle in indices.chunks_exact(TRIANGLE_INDICES) {
        let bones: BTreeSet<_> = triangle
            .iter()
            .flat_map(|&index| {
                let influence = &weights[index as usize];
                influence
                    .joints
                    .into_iter()
                    .zip(influence.weights)
                    .filter_map(|(joint, weight)| (weight > 0.).then_some(joint))
            })
            .collect();
        if current.bones.union(&bones).count() > MAX_PALETTE_BONES {
            partitions.push(std::mem::take(&mut current));
        }
        current.bones.extend(bones);
        for &source in triangle {
            let destination = *current.remap.entry(source).or_insert_with(|| {
                let index = current.sources.len() as u32;
                current.sources.push(source as usize);
                index
            });
            current.indices.push(destination);
        }
    }
    partitions.push(current);
    if partitions.len() > u16::MAX as usize + 1 {
        return Err(failure("skeletal chunk count exceeds section u16 indices"));
    }
    Ok(partitions)
}

pub(super) fn skinned(
    vertices: &[Vertex],
    indices: &[u32],
    weights: &[SkinWeights],
    skeleton: &[ReferenceBone],
    extra_uvs: &[Vec<[f32; 2]>],
) -> Result<SkinnedMesh> {
    if indices.len() > i32::MAX as usize {
        return Err(failure("skin index count exceeds native limits"));
    }
    let partitions = partition(indices, weights)?;
    let vertex_count = partitions
        .iter()
        .try_fold(0usize, |total, part| total.checked_add(part.sources.len()))
        .ok_or_else(|| failure("chunk vertex count overflow"))?;
    if vertex_count > i32::MAX as usize {
        return Err(failure("chunk vertices exceed native limits"));
    }
    let index_stride = if vertex_count > encoding::MAX_COMPACT_VERTICES {
        4
    } else {
        2
    };
    let tex_coords = 1 + extra_uvs.len();
    let has_colors = vertices.iter().any(|vertex| vertex.color.is_some());
    let mut skin = SkinPayload {
        gpu_vertices: encoding::buffer(
            vertex_count
                .checked_mul(28 + tex_coords * 8)
                .ok_or_else(|| failure("chunk GPU byte size overflow"))?,
        )?,
        colors: encoding::buffer(
            vertex_count
                .checked_mul(usize::from(has_colors) * 4)
                .ok_or_else(|| failure("chunk color byte size overflow"))?,
        )?,
        tex_coords,
        indices: encoding::buffer(
            indices
                .len()
                .checked_mul(index_stride)
                .ok_or_else(|| failure("chunk index byte size overflow"))?,
        )?,
        index_stride,
        palette: Vec::new(),
        chunks: Vec::new(),
        vertices: vertex_count,
        triangles: indices.len() / TRIANGLE_INDICES,
        max_bone_influences: 0,
        bounds: encoding::bounds(vertices)?,
    };
    let mut base_vertex = 0;
    let mut first_index = 0;
    for part in partitions {
        let local_vertices: Vec<_> = part
            .sources
            .iter()
            .map(|&source| vertices[source])
            .collect();
        let local_weights: Vec<_> = part.sources.iter().map(|&source| weights[source]).collect();
        let local_uvs: Vec<_> = extra_uvs
            .iter()
            .map(|channel| part.sources.iter().map(|&source| channel[source]).collect())
            .collect();
        let local = super::skinned_with_uvs(
            &local_vertices,
            &part.indices,
            &local_weights,
            skeleton,
            &local_uvs,
        )?
        .skin;
        for index in part.indices {
            let index = index + base_vertex as u32;
            skin.indices
                .extend_from_slice(&index.to_le_bytes()[..index_stride]);
        }
        skin.gpu_vertices.extend_from_slice(&local.gpu_vertices);
        if has_colors {
            if local.colors.is_empty() {
                skin.colors
                    .resize(skin.colors.len() + local.vertices * 4, 255);
            } else {
                skin.colors.extend_from_slice(&local.colors);
            }
        }
        skin.max_bone_influences = skin.max_bone_influences.max(local.max_bone_influences);
        skin.palette.extend_from_slice(&local.palette);
        skin.chunks.push(SkinChunk {
            base_vertex,
            vertices: local.vertices,
            first_index,
            triangles: local.triangles,
            palette: local.palette,
            max_bone_influences: local.max_bone_influences,
        });
        base_vertex += local.vertices;
        first_index += local.triangles * TRIANGLE_INDICES;
    }
    skin.palette.sort_unstable();
    skin.palette.dedup();
    let (active_bones, required_bones) = bone_closure(skeleton, &skin.palette)?;
    Ok(SkinnedMesh {
        skin,
        skeleton: skeleton.to_vec(),
        active_bones,
        required_bones,
    })
}
