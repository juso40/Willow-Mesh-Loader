//! Packed fragment replacement with shared-range preservation and vertex compaction.

use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
};

use super::PartRange;
use crate::skeletal::{self, PreparedSkeletalMesh, SkinChunk, SkinPayload, SkinnedMesh, gestalt};
use crate::{Error, Result, encoding, sections};

pub(super) struct Updated {
    pub mesh: PreparedSkeletalMesh,
    pub parts: Vec<PartRange>,
    pub bounds: BTreeMap<String, encoding::Bounds>,
}

fn range(part: &PartRange, indices: usize, materials: usize) -> Result<Range<usize>> {
    let end = part
        .triangles
        .checked_mul(3)
        .and_then(|n| part.first_index.checked_add(n));
    match end {
        Some(end)
            if part.first_index.is_multiple_of(3)
                && part.material_slot < materials
                && end <= indices =>
        {
            Ok(part.first_index..end)
        }
        _ => Err(Error(format!("invalid Gestalt range for {}", part.name))),
    }
}

fn slots(parts: &[PartRange], name: &str) -> Vec<usize> {
    let mut slots = Vec::new();
    for part in parts.iter().filter(|part| part.name == name) {
        if !slots.contains(&part.material_slot) {
            slots.push(part.material_slot);
        }
    }
    slots
}

pub(super) fn update(
    current: &PreparedSkeletalMesh,
    existing: &[PartRange],
    incoming: &PreparedSkeletalMesh,
    replacements: &[PartRange],
    preserve_materials: bool,
) -> Result<Updated> {
    if existing.is_empty() || replacements.is_empty() {
        return Err(Error(
            "Gestalt updates require existing and incoming fragments".into(),
        ));
    }
    let old = current.mesh();
    let mut incoming = incoming.clone();
    incoming.bind_skeleton(&old.skeleton)?;
    let source = incoming.mesh();
    let old_indices = gestalt::indices(&old.skin)?;
    let source_indices = gestalt::indices(&source.skin)?;
    let names: BTreeSet<_> = replacements.iter().map(|part| part.name.as_str()).collect();
    for name in &names {
        if name.eq_ignore_ascii_case("None") || !existing.iter().any(|part| part.name == *name) {
            return Err(Error(format!(
                "Gestalt update has no existing fragment named {name}"
            )));
        }
    }
    let mut keep = vec![true; old.skin.triangles];
    for part in existing {
        let range = range(part, old_indices.len(), current.material_count())?;
        if names.contains(part.name.as_str()) {
            keep[range.start / 3..range.end / 3].fill(false);
        }
    }
    // Different identities may alias the same triangles. Untouched identities win.
    for part in existing
        .iter()
        .filter(|part| !names.contains(part.name.as_str()))
    {
        let range = range(part, old_indices.len(), current.material_count())?;
        keep[range.start / 3..range.end / 3].fill(true);
    }
    let mut prefix = Vec::with_capacity(keep.len() + 1);
    prefix.push(0usize);
    for &keep in &keep {
        prefix.push(prefix[prefix.len() - 1] + usize::from(keep));
    }
    let first_index = prefix[keep.len()] * 3;
    let materials = if preserve_materials {
        current.material_count()
    } else {
        current
            .material_count()
            .checked_add(incoming.material_count())
            .ok_or_else(|| Error("Gestalt material count overflow".into()))?
    };
    if materials > usize::from(u16::MAX) + 1 {
        return Err(Error(
            "Gestalt material count exceeds native ordinals".into(),
        ));
    }
    let mut parts: Vec<_> = existing
        .iter()
        .filter(|part| !names.contains(part.name.as_str()))
        .map(|part| PartRange {
            first_index: prefix[part.first_index / 3] * 3,
            ..part.clone()
        })
        .collect();
    let mut source_sections = Vec::new();
    for part in replacements {
        range(part, source_indices.len(), incoming.material_count())?;
        let material_slot = if preserve_materials {
            let previous = slots(existing, &part.name);
            let selected = slots(replacements, &part.name);
            if previous.len() != 1 && previous.len() != selected.len() {
                return Err(Error(format!(
                    "fragment {} requires matching material-slot groups or explicit material assignments",
                    part.name,
                )));
            }
            if previous.len() == 1 {
                previous[0]
            } else {
                let slot = selected
                    .iter()
                    .position(|&slot| slot == part.material_slot)
                    .ok_or_else(|| Error("Gestalt source material mapping is incomplete".into()))?;
                previous[slot]
            }
        } else {
            current.material_count() + part.material_slot
        };
        source_sections.push(sections::Section {
            first_index: part.first_index,
            triangles: part.triangles,
            material_index: material_slot,
        });
        parts.push(PartRange {
            first_index: first_index + part.first_index,
            material_slot,
            ..part.clone()
        });
    }
    source_sections.sort_by_key(|section| section.first_index);
    let source_sections_checked = sections::validate(
        &source_indices,
        source.skin.vertices,
        &source_sections,
        materials,
    )?;
    let (mesh, checked) = if let Some((left, left_sections)) = filter(current, &old_indices, &keep)?
    {
        gestalt::join(left, source, &left_sections, &source_sections, 0, materials)?
    } else {
        let mut mesh = source.clone();
        if old.skin.tex_coords > source.skin.tex_coords {
            let mut vertices = Vec::new();
            gestalt::append_vertices(&mut vertices, &source.skin, old.skin.tex_coords)?;
            mesh.skin.gpu_vertices = vertices;
            mesh.skin.tex_coords = old.skin.tex_coords;
        }
        if !old.skin.colors.is_empty() && mesh.skin.colors.is_empty() {
            let bytes = mesh
                .skin
                .vertices
                .checked_mul(4)
                .ok_or_else(|| Error("Gestalt color byte size overflow".into()))?;
            mesh.skin.colors = encoding::buffer(bytes)?;
            mesh.skin.colors.resize(bytes, 255);
        }
        (mesh, source_sections_checked)
    };
    let mut bounds = BTreeMap::new();
    for name in names {
        let positions = replacements
            .iter()
            .filter(|part| part.name == name)
            .flat_map(|part| {
                &source_indices[part.first_index..part.first_index + part.triangles * 3]
            })
            .map(|&index| position(&source.skin, index as usize));
        bounds.insert(name.to_owned(), encoding::bounds_from_positions(positions)?);
    }
    parts.sort_by_key(|part| part.first_index);
    Ok(Updated {
        mesh: PreparedSkeletalMesh::encoded(mesh, checked, materials),
        parts,
        bounds,
    })
}

fn position(skin: &SkinPayload, index: usize) -> [f32; 3] {
    let record = &skin.gpu_vertices[index * skin.vertex_stride()..];
    std::array::from_fn(|axis| {
        f32::from_le_bytes(std::array::from_fn(|byte| record[16 + axis * 4 + byte]))
    })
}

fn filter(
    current: &PreparedSkeletalMesh,
    indices: &[u32],
    keep: &[bool],
) -> Result<Option<(SkinnedMesh, Vec<sections::Section>)>> {
    if !keep.iter().any(|&keep| keep) {
        return Ok(None);
    }
    let source = current.mesh();
    let skin = &source.skin;
    let stride = skin.vertex_stride();
    let sections = current.section_inputs();
    let mut gpu_vertices = Vec::new();
    let mut colors = Vec::new();
    let mut output_indices = Vec::new();
    let mut output_sections: Vec<sections::Section> = Vec::new();
    let mut chunks = Vec::new();
    let mut section_index = 0;
    for chunk in &skin.chunks {
        let base_vertex = gpu_vertices.len() / stride;
        let first_index = output_indices.len();
        let mut remap = vec![u32::MAX; chunk.vertices];
        let mut max_influences = 0;
        for triangle in chunk.first_index / 3..chunk.first_index / 3 + chunk.triangles {
            while triangle * 3
                >= sections[section_index].first_index + sections[section_index].triangles * 3
            {
                section_index += 1;
            }
            if !keep[triangle] {
                continue;
            }
            let material_index = sections[section_index].material_index;
            match output_sections.last_mut() {
                Some(last) if last.material_index == material_index => last.triangles += 1,
                _ => output_sections.push(sections::Section {
                    first_index: output_indices.len(),
                    triangles: 1,
                    material_index,
                }),
            }
            output_indices
                .try_reserve(3)
                .map_err(|_| Error("Gestalt index allocation failed".into()))?;
            for &index in &indices[triangle * 3..triangle * 3 + 3] {
                let index = index as usize;
                if !(chunk.base_vertex..chunk.base_vertex + chunk.vertices).contains(&index) {
                    return Err(Error(
                        "Gestalt index escapes its source palette chunk".into(),
                    ));
                }
                let destination = &mut remap[index - chunk.base_vertex];
                if *destination == u32::MAX {
                    *destination = (gpu_vertices.len() / stride) as u32;
                    let record = &skin.gpu_vertices[index * stride..(index + 1) * stride];
                    gpu_vertices
                        .try_reserve(stride)
                        .map_err(|_| Error("Gestalt GPU allocation failed".into()))?;
                    gpu_vertices.extend_from_slice(record);
                    max_influences = max_influences
                        .max(record[12..16].iter().filter(|&&w| w != 0).count() as u32);
                    if !skin.colors.is_empty() {
                        colors
                            .try_reserve(4)
                            .map_err(|_| Error("Gestalt color allocation failed".into()))?;
                        colors.extend_from_slice(&skin.colors[index * 4..index * 4 + 4]);
                    }
                }
                output_indices.push(*destination);
            }
        }
        if output_indices.len() != first_index {
            chunks.push(SkinChunk {
                base_vertex,
                vertices: gpu_vertices.len() / stride - base_vertex,
                first_index,
                triangles: (output_indices.len() - first_index) / 3,
                palette: chunk.palette.clone(),
                max_bone_influences: max_influences,
            });
        }
    }
    let vertices = gpu_vertices.len() / stride;
    let index_stride = if vertices > encoding::MAX_COMPACT_VERTICES {
        4
    } else {
        2
    };
    let mut packed = encoding::buffer(
        output_indices
            .len()
            .checked_mul(index_stride)
            .ok_or_else(|| Error("Gestalt index size overflow".into()))?,
    )?;
    for index in &output_indices {
        packed.extend_from_slice(&index.to_le_bytes()[..index_stride]);
    }
    let used: Vec<_> = chunks
        .iter()
        .flat_map(|chunk| chunk.palette.iter().copied())
        .collect();
    let (active_bones, required_bones) = skeletal::bone_closure(&source.skeleton, &used)?;
    let max_bone_influences = chunks
        .iter()
        .map(|chunk| chunk.max_bone_influences)
        .max()
        .unwrap_or(0);
    let mut output = SkinnedMesh {
        skin: SkinPayload {
            gpu_vertices,
            colors,
            tex_coords: skin.tex_coords,
            indices: packed,
            index_stride,
            palette: active_bones.clone(),
            chunks,
            vertices,
            triangles: output_indices.len() / 3,
            max_bone_influences,
            bounds: skin.bounds,
        },
        skeleton: source.skeleton.clone(),
        active_bones,
        required_bones,
    };
    output.skin.bounds =
        encoding::bounds_from_positions((0..vertices).map(|index| position(&output.skin, index)))?;
    Ok(Some((output, output_sections)))
}
