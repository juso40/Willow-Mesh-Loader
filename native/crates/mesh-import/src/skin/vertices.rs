//! Skeletal influence decoding and assembly around shared primitive conversion.

use super::{Fragment, SkinnedGeometry, SkinnedVertex};
use crate::math::Matrix;
use crate::{Result, Section, fail, primitive, warn};
use gltf::mesh::Semantic;

struct Influence {
    joints: [u16; 4],
    weights: [f32; 4],
}

pub(super) fn append_skinned(
    primitive: gltf::Primitive<'_>,
    conversion: &Matrix,
    buffers: &crate::document::Buffers<'_>,
    to_bone: &[u16],
    out: &mut SkinnedGeometry,
) -> Result<()> {
    let converted = primitive::convert(
        primitive.clone(),
        *conversion,
        buffers,
        out.vertices.len(),
        &mut out.primitives,
        &mut out.warnings,
        true,
    )?;
    let influences = influences(
        &primitive,
        buffers,
        to_bone,
        converted.source_vertices,
        &mut out.warnings,
    )?;
    let first_index = out.indices.len();
    for channel in 0..4 {
        if primitive.get(&Semantic::TexCoords(channel)).is_some() {
            out.tex_coords = out.tex_coords.max(channel as usize + 1);
        }
    }
    if let Some(extras) = primitive.extras() {
        let extras: serde_json::Value =
            serde_json::from_str(extras.get()).map_err(|_| fail("invalid primitive extras"))?;
        if let Some(name) = extras.get("gestalt_fragment_name") {
            let name = name
                .as_str()
                .filter(|name| {
                    super::bind::valid_bone_name(name) && !name.eq_ignore_ascii_case("None")
                })
                .ok_or_else(|| fail("Gestalt fragment name must be a nonempty native FName"))?;
            out.fragments.push(Fragment {
                name: name.into(),
                first_index,
                triangles: converted.vertices.len() / 3,
                material_index: out.sections.len(),
            });
        }
    }
    let vertices = converted.vertices.len();
    out.vertices
        .try_reserve(vertices)
        .map_err(|_| fail("host vertex allocation failed"))?;
    out.indices
        .try_reserve(vertices)
        .map_err(|_| fail("host index allocation failed"))?;
    out.indices
        .extend((out.vertices.len()..out.vertices.len() + vertices).map(|index| index as u32));
    out.vertices.extend(
        converted
            .vertices
            .into_iter()
            .zip(converted.source_indices)
            .map(|(vertex, source)| {
                let influence = &influences[source as usize];
                SkinnedVertex {
                    vertex,
                    joints: influence.joints,
                    weights: influence.weights,
                }
            }),
    );
    out.sections.push(Section {
        first_index,
        triangles: vertices / 3,
        source_material: primitive.material().index(),
    });
    Ok(())
}

fn influences(
    primitive: &gltf::Primitive<'_>,
    buffers: &crate::document::Buffers<'_>,
    to_bone: &[u16],
    count: usize,
    warnings: &mut Vec<String>,
) -> Result<Vec<Influence>> {
    if primitive.get(&Semantic::Joints(0)).is_none()
        || primitive.get(&Semantic::Weights(0)).is_none()
    {
        return Err(fail("skinned primitive needs JOINTS_0 and WEIGHTS_0"));
    }
    let second_set = primitive.get(&Semantic::Joints(1)).is_some();
    if second_set != primitive.get(&Semantic::Weights(1)).is_some() {
        return Err(fail("JOINTS_1 and WEIGHTS_1 must appear together"));
    }
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()));
    let joints0: Vec<[u16; 4]> = reader
        .read_joints(0)
        .ok_or_else(|| fail("cannot read JOINTS_0"))?
        .into_u16()
        .collect();
    let weights0: Vec<[f32; 4]> = reader
        .read_weights(0)
        .ok_or_else(|| fail("cannot read WEIGHTS_0"))?
        .into_f32()
        .collect();
    let joints1: Option<Vec<[u16; 4]>> = reader
        .read_joints(1)
        .map(|values| values.into_u16().collect());
    let weights1: Option<Vec<[f32; 4]>> = reader
        .read_weights(1)
        .map(|values| values.into_f32().collect());
    if second_set && (joints1.is_none() || weights1.is_none()) {
        return Err(fail("present skin attributes could not be decoded"));
    }
    if joints0.len() != count
        || weights0.len() != count
        || joints1.as_ref().is_some_and(|values| values.len() != count)
        || weights1
            .as_ref()
            .is_some_and(|values| values.len() != count)
    {
        return Err(fail("short skin attribute data"));
    }
    if weights0
        .iter()
        .flatten()
        .chain(weights1.iter().flatten().flatten())
        .any(|value| !value.is_finite())
    {
        return Err(fail("nonfinite skin weight"));
    }
    if weights0
        .iter()
        .flatten()
        .chain(weights1.iter().flatten().flatten())
        .any(|value| *value < 0.)
    {
        return Err(fail("negative skin weight"));
    }

    let mut influences = Vec::with_capacity(count);
    let mut reduced = false;
    for i in 0..count {
        if joints0[i]
            .iter()
            .chain(
                joints1
                    .as_ref()
                    .into_iter()
                    .flat_map(|joints| joints[i].iter()),
            )
            .any(|&joint| usize::from(joint) >= to_bone.len())
        {
            return Err(fail("joint attribute references a joint outside the skin"));
        }
        // Duplicate joints are merged before the strongest-four reduction so
        // repeated joints cannot consume influence slots or hide malformed data.
        let mut merged: Vec<(u16, f64)> = Vec::new();
        let sets = std::iter::once((&joints0[i], &weights0[i])).chain(
            joints1
                .as_ref()
                .zip(weights1.as_ref())
                .map(|(joints, weights)| (&joints[i], &weights[i])),
        );
        for (joints, weights) in sets {
            for (&joint, &weight) in joints.iter().zip(weights) {
                if weight > 0. {
                    let joint = to_bone[usize::from(joint)];
                    if let Some(entry) = merged.iter_mut().find(|entry| entry.0 == joint) {
                        entry.1 += f64::from(weight);
                    } else {
                        merged.push((joint, f64::from(weight)));
                    }
                }
            }
        }
        if merged.is_empty() {
            return Err(fail("skinned vertex has no nonzero weight"));
        }
        if merged.len() > 4 {
            reduced = true;
            merged.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            merged.truncate(4);
        }
        let mut influence = Influence {
            joints: [0; 4],
            weights: [0.; 4],
        };
        let sum: f64 = merged.iter().map(|entry| entry.1).sum();
        for (slot, (joint, weight)) in merged.iter().enumerate() {
            influence.joints[slot] = *joint;
            influence.weights[slot] = (*weight / sum) as f32;
        }
        influences.push(influence);
    }
    if reduced {
        warn(
            warnings,
            "vertex influences reduced to the four strongest. Runtime renormalizes",
        );
    }
    Ok(influences)
}
