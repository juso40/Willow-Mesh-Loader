//! Shared primitive decoding, face-corner expansion and engine-space conversion.

mod tangents;

use crate::document::typed;
use crate::math::*;
use crate::{Result, Vertex, fail, warn};
use gltf::{
    accessor::{DataType, Dimensions},
    mesh::{Mode, Semantic},
};
use tangents::Corner;

pub(crate) struct Converted {
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) source_indices: Vec<u32>,
    pub(crate) source_vertices: usize,
}

pub(crate) fn convert(
    primitive: gltf::Primitive<'_>,
    transform: Matrix,
    buffers: &crate::document::Buffers<'_>,
    existing_vertices: usize,
    primitives: &mut usize,
    warnings: &mut Vec<String>,
    skinned: bool,
) -> Result<Converted> {
    if primitive.mode() != Mode::Triangles {
        return Err(fail("only TRIANGLES primitives are supported"));
    }
    if primitive.morph_targets().next().is_some() {
        return Err(fail("morph targets are not supported"));
    }
    *primitives += 1;
    let positions = primitive
        .get(&Semantic::Positions)
        .ok_or_else(|| fail("primitive needs POSITION"))?;
    typed(&positions, DataType::F32, Dimensions::Vec3)?;
    let count = positions.count();
    for (semantic, accessor) in primitive.attributes() {
        if matches!(semantic, Semantic::TexCoords(1..) if !skinned) {
            warn(warnings, "static UV channels beyond TEXCOORD_0 are ignored");
            continue;
        }
        if matches!(semantic, Semantic::Colors(1..)) {
            warn(warnings, "color sets beyond COLOR_0 are ignored");
            continue;
        }
        if accessor.count() != count {
            return Err(fail("primitive attribute counts differ"));
        }
        match semantic {
            Semantic::Positions => {}
            Semantic::Normals => typed(&accessor, DataType::F32, Dimensions::Vec3)?,
            Semantic::Tangents => typed(&accessor, DataType::F32, Dimensions::Vec4)?,
            Semantic::TexCoords(0..=3) => {
                if accessor.dimensions() != Dimensions::Vec2
                    || !matches!(
                        (accessor.data_type(), accessor.normalized()),
                        (DataType::F32, false) | (DataType::U8, true) | (DataType::U16, true)
                    )
                {
                    return Err(fail(
                        "texture coordinates need float or normalized u8/u16 VEC2",
                    ));
                }
            }
            Semantic::Colors(0) => {
                if !matches!(accessor.dimensions(), Dimensions::Vec3 | Dimensions::Vec4)
                    || !matches!(
                        (accessor.data_type(), accessor.normalized()),
                        (DataType::F32, false) | (DataType::U8, true) | (DataType::U16, true)
                    )
                {
                    return Err(fail("COLOR_0 needs float or normalized u8/u16 VEC3/VEC4"));
                }
            }
            Semantic::Joints(_) | Semantic::Weights(_) if !skinned => {
                return Err(fail("skin attributes are not supported"));
            }
            Semantic::Joints(0 | 1) => {
                if accessor.dimensions() != Dimensions::Vec4
                    || accessor.normalized()
                    || !matches!(accessor.data_type(), DataType::U8 | DataType::U16)
                {
                    return Err(fail("JOINTS_0/1 need unnormalized u8/u16 VEC4"));
                }
            }
            Semantic::Weights(0 | 1) => {
                if accessor.dimensions() != Dimensions::Vec4
                    || !matches!(
                        (accessor.data_type(), accessor.normalized()),
                        (DataType::F32, false) | (DataType::U8, true) | (DataType::U16, true)
                    )
                {
                    return Err(fail("WEIGHTS_0/1 need float or normalized u8/u16 VEC4"));
                }
            }
            Semantic::Joints(_) | Semantic::Weights(_) => {
                return Err(fail("only influence sets 0 and 1 are supported"));
            }
            Semantic::TexCoords(_) if skinned => {
                return Err(fail(
                    "the native GPU skin format supports at most four UV sets",
                ));
            }
            _ => {}
        }
    }
    if let Some(accessor) = primitive.indices()
        && (accessor.dimensions() != Dimensions::Scalar
            || accessor.normalized()
            || !matches!(
                accessor.data_type(),
                DataType::U8 | DataType::U16 | DataType::U32
            ))
    {
        return Err(fail("indices need unsigned unnormalized SCALAR"));
    }
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()));
    let positions: Vec<_> = reader
        .read_positions()
        .ok_or_else(|| fail("cannot read positions"))?
        .collect();
    let normals = reader
        .read_normals()
        .map(|values| values.collect::<Vec<_>>());
    let authored = reader
        .read_tangents()
        .map(|values| values.collect::<Vec<_>>());
    let uvs = reader
        .read_tex_coords(0)
        .map(|values| values.into_f32().collect::<Vec<_>>());
    let colors = reader
        .read_colors(0)
        .map(|values| values.into_rgba_f32().collect::<Vec<_>>());
    if (primitive.get(&Semantic::Colors(0)).is_some() && colors.is_none())
        || (primitive.get(&Semantic::Normals).is_some() && normals.is_none())
        || (primitive.get(&Semantic::Tangents).is_some() && authored.is_none())
        || (primitive.get(&Semantic::TexCoords(0)).is_some() && uvs.is_none())
    {
        return Err(fail("present attribute could not be decoded"));
    }
    if positions.len() != count
        || normals.as_ref().is_some_and(|values| values.len() != count)
        || authored
            .as_ref()
            .is_some_and(|values| values.len() != count)
        || uvs.as_ref().is_some_and(|values| values.len() != count)
        || colors.as_ref().is_some_and(|values| values.len() != count)
    {
        return Err(fail("short attribute data"));
    }
    if positions
        .iter()
        .flatten()
        .chain(normals.iter().flatten().flatten())
        .chain(authored.iter().flatten().flatten())
        .chain(uvs.iter().flatten().flatten())
        .any(|value| !value.is_finite())
    {
        return Err(fail("nonfinite vertex attribute"));
    }
    if colors.as_ref().is_some_and(|values| {
        values
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    }) {
        return Err(fail("COLOR_0 components must be finite and within [0, 1]"));
    }
    if authored.is_some() && normals.is_none() {
        return Err(fail("TANGENT requires NORMAL"));
    }
    let mut indices: Vec<u32> = reader
        .read_indices()
        .map(|values| values.into_u32().collect())
        .unwrap_or_else(|| (0..count as u32).collect());
    if indices.is_empty()
        || !indices.len().is_multiple_of(3)
        || indices.iter().any(|&index| index as usize >= count)
    {
        return Err(fail("invalid triangle index list"));
    }
    if indices.len() > (i32::MAX as usize).saturating_sub(existing_vertices) {
        return Err(fail(
            "triangle corners exceed the signed native array count",
        ));
    }
    let mut extra_uvs = [None, None, None];
    if skinned {
        for (channel, values) in extra_uvs.iter_mut().enumerate() {
            *values = reader
                .read_tex_coords(channel as u32 + 1)
                .map(|values| values.into_f32().collect::<Vec<_>>());
            if values.as_ref().is_some_and(|values| {
                values.len() != count || values.iter().flatten().any(|v| !v.is_finite())
            }) {
                return Err(fail("invalid additional UV channel"));
            }
        }
    }
    if normals.is_none() {
        warn(warnings, "missing normals: generated flat face normals");
    }
    if uvs.is_none() {
        warn(warnings, "missing TEXCOORD_0: using zero UVs");
    }

    // MikkTSpace operates in authored glTF space and returns one tangent per
    // face corner. Axis reflection, nonuniform scale and winding are applied
    // afterwards, exactly as for authored tangents. Seams remain split.
    let mut corners = Vec::new();
    corners
        .try_reserve(indices.len())
        .map_err(|_| fail("host vertex allocation failed"))?;
    for triangle in indices.chunks_exact(3) {
        let points = [triangle[0], triangle[1], triangle[2]]
            .map(|index| positions[index as usize].map(f64::from));
        let flat = if normals.is_some() {
            [0., 0., 1.]
        } else {
            unit(cross(sub(points[1], points[0]), sub(points[2], points[0]))).unwrap_or_else(|_| {
                warn(
                    warnings,
                    "degenerate triangle without normals: using a deterministic normal",
                );
                [0., 0., 1.]
            })
        };
        for &index in triangle {
            let index = index as usize;
            let normal = normals
                .as_ref()
                .map_or(flat, |values| values[index].map(f64::from));
            corners.push(Corner {
                position: positions[index],
                normal: f32v(unit(normal)?)?,
                uv: uvs.as_ref().map_or([0.; 2], |values| values[index]),
            });
        }
    }
    let generated = if authored.is_none() {
        warn(
            warnings,
            "missing tangents: generated MikkTSpace frames where UVs permit",
        );
        Some(tangents::generate(&corners, uvs.is_some(), warnings))
    } else {
        None
    };
    let mut vertices = Vec::new();
    vertices
        .try_reserve(indices.len())
        .map_err(|_| fail("host vertex allocation failed"))?;
    let reflected_winding = determinant(transform) > 0.;
    for triangle in (0..indices.len()).step_by(3) {
        let order = if reflected_winding {
            [0, 2, 1]
        } else {
            [0, 1, 2]
        };
        for offset in order {
            let corner_index = triangle + offset;
            let source = indices[corner_index];
            let corner = &corners[corner_index];
            let tangent = match &authored {
                Some(values) => Some(values[source as usize]),
                None => generated.as_ref().and_then(|values| values[corner_index]),
            };
            let (tangent_x, tangent_y, tangent_z) =
                tangents::basis(transform, corner.normal, tangent, warnings)?;
            vertices.push(Vertex {
                position: f32v(position(transform, corner.position))?,
                tangent_x,
                tangent_y,
                tangent_z,
                uv: corner.uv,
                color: colors
                    .as_ref()
                    .map(|values| values[source as usize].map(|v| (v * 255.0).round() as u8)),
                extra_uvs: std::array::from_fn(|channel| {
                    extra_uvs[channel]
                        .as_ref()
                        .map_or([0.; 2], |values| values[source as usize])
                }),
            });
        }
    }
    if reflected_winding {
        for triangle in indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    Ok(Converted {
        vertices,
        source_indices: indices,
        source_vertices: count,
    })
}
