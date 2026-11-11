//! Source-space MikkTSpace generation and shared engine-space tangent frames.

use crate::math::*;
use crate::policy::{
    FALLBACK_AXIS_MAX_ALIGNMENT, MIN_PROJECTED_TANGENT_NORM2, MIN_UV_DETERMINANT,
    TANGENT_SIGN_TOLERANCE,
};
use crate::{Result, fail, warn};

pub(super) struct Corner {
    pub(super) position: [f32; 3],
    pub(super) normal: [f32; 3],
    pub(super) uv: [f32; 2],
}

struct Tangents<'a> {
    corners: &'a [Corner],
    values: Vec<Option<[f32; 4]>>,
}

impl mikktspace::Geometry for Tangents<'_> {
    fn num_faces(&self) -> usize {
        self.corners.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.corners[face * 3 + vert].position
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.corners[face * 3 + vert].normal
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.corners[face * 3 + vert].uv
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        self.values[face * 3 + vert] = Some(tangent);
    }
}

pub(super) fn generate(
    corners: &[Corner],
    has_uvs: bool,
    warnings: &mut Vec<String>,
) -> Vec<Option<[f32; 4]>> {
    let mut geometry = Tangents {
        corners,
        values: vec![None; corners.len()],
    };
    if !has_uvs {
        return geometry.values;
    }
    if !mikktspace::generate_tangents(&mut geometry) {
        warn(
            warnings,
            "MikkTSpace tangent generation failed: using orthogonal fallback",
        );
        geometry.values.fill(None);
    }
    for (face, outputs) in corners
        .chunks_exact(3)
        .zip(geometry.values.chunks_exact_mut(3))
    {
        let uv = [0, 1, 2].map(|index| face[index].uv.map(f64::from));
        let determinant = (uv[1][0] - uv[0][0]) * (uv[2][1] - uv[0][1])
            - (uv[1][1] - uv[0][1]) * (uv[2][0] - uv[0][0]);
        for (corner, output) in face.iter().zip(outputs) {
            let usable = output.is_some_and(|tangent| {
                let normal = corner.normal.map(f64::from);
                let vector = [tangent[0], tangent[1], tangent[2]].map(f64::from);
                let projected = sub(vector, mul(normal, dot(normal, vector)));
                tangent.iter().all(|value| value.is_finite())
                    && (tangent[3].abs() - 1.).abs() <= TANGENT_SIGN_TOLERANCE
                    && dot(projected, projected) > MIN_PROJECTED_TANGENT_NORM2
            });
            if determinant.abs() <= MIN_UV_DETERMINANT || !usable {
                *output = None;
            }
        }
    }
    geometry.values
}

pub(super) fn basis(
    transform: Matrix,
    source_normal: [f32; 3],
    tangent: Option<[f32; 4]>,
    warnings: &mut Vec<String>,
) -> Result<([f32; 3], [f32; 3], [f32; 3])> {
    let local_n = unit(source_normal.map(f64::from))?;
    let n = normal(transform, local_n)?;
    let fallback = || {
        let axis = if n[0].abs() < FALLBACK_AXIS_MAX_ALIGNMENT {
            [1., 0., 0.]
        } else {
            [0., 1., 0.]
        };
        sub(axis, mul(n, dot(axis, n)))
    };
    let (t, b) = if let Some(tangent) = tangent {
        if (tangent[3].abs() - 1.).abs() > TANGENT_SIGN_TOLERANCE {
            return Err(fail("glTF tangent W must be +1 or -1"));
        }
        let local_t = unit([tangent[0], tangent[1], tangent[2]].map(f64::from))?;
        (
            direction(transform, local_t),
            direction(
                transform,
                mul(cross(local_n, local_t), f64::from(tangent[3])),
            ),
        )
    } else {
        warn(
            warnings,
            "degenerate/missing UV tangent frame: deterministic orthogonal fallback",
        );
        let t = fallback();
        (t, cross(n, t))
    };
    let projected = sub(t, mul(n, dot(n, t)));
    let t = if dot(projected, projected) < MIN_PROJECTED_TANGENT_NORM2 {
        warn(
            warnings,
            "degenerate tangent: deterministic orthogonal fallback",
        );
        unit(fallback())?
    } else {
        unit(projected)?
    };
    let crossed = cross(n, t);
    let y = mul(crossed, if dot(crossed, b) < 0. { -1. } else { 1. });
    Ok((f32v(t)?, f32v(y)?, f32v(n)?))
}
