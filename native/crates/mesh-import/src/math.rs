//! Checked affine transforms and vector operations for importers.

use crate::{
    Result, fail,
    policy::{
        AFFINE_TOLERANCE, MIN_RELATIVE_DETERMINANT, MIN_VECTOR_LENGTH, QUATERNION_NORM_TOLERANCE,
    },
};
pub(crate) type V = [f64; 3];
pub(crate) type Matrix = [[f64; 4]; 4];

pub(crate) fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
pub(crate) fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
pub(crate) fn mul(a: V, s: f64) -> V {
    a.map(|v| v * s)
}
pub(crate) fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn unit(v: V) -> Result<V> {
    let l = dot(v, v).sqrt();
    if !l.is_finite() || l < MIN_VECTOR_LENGTH {
        return Err(fail(
            "zero/nonfinite normal, tangent or degenerate triangle",
        ));
    }
    Ok(mul(v, 1. / l))
}
pub(crate) fn f32v(v: V) -> Result<[f32; 3]> {
    let f = v.map(|v| v as f32);
    if f.iter().any(|v| !v.is_finite()) {
        return Err(fail("transformed vertex/basis exceeds finite f32 range"));
    }
    Ok(f)
}
pub(crate) fn matrix_mul(a: Matrix, b: Matrix) -> Matrix {
    std::array::from_fn(|c| std::array::from_fn(|r| (0..4).map(|k| a[k][r] * b[c][k]).sum()))
}
pub(crate) fn direction(m: Matrix, v: V) -> V {
    std::array::from_fn(|r| (0..3).map(|c| m[c][r] * v[c]).sum())
}
pub(crate) fn position(m: Matrix, v: [f32; 3]) -> V {
    add(direction(m, v.map(f64::from)), [m[3][0], m[3][1], m[3][2]])
}
fn columns(m: Matrix) -> [V; 3] {
    std::array::from_fn(|i| [m[i][0], m[i][1], m[i][2]])
}
pub(crate) fn determinant(m: Matrix) -> f64 {
    let c = columns(m);
    dot(c[0], cross(c[1], c[2]))
}
pub(crate) fn normal(m: Matrix, n: V) -> Result<V> {
    let c = columns(m);
    let co = [cross(c[1], c[2]), cross(c[2], c[0]), cross(c[0], c[1])];
    unit(mul(
        add(add(mul(co[0], n[0]), mul(co[1], n[1])), mul(co[2], n[2])),
        1. / determinant(m),
    ))
}
pub(crate) fn node_transform(transform: gltf::scene::Transform) -> Result<Matrix> {
    if let gltf::scene::Transform::Decomposed { rotation, .. } = &transform {
        let norm: f64 = rotation.iter().map(|v| f64::from(*v).powi(2)).sum();
        if !norm.is_finite() || (norm - 1.).abs() > QUATERNION_NORM_TOLERANCE {
            return Err(fail("node rotation quaternion must be finite/unit"));
        }
    }
    checked_matrix(transform.matrix().map(|c| c.map(f64::from)))
}

pub(crate) fn checked_matrix(m: Matrix) -> Result<Matrix> {
    if m.iter().flatten().any(|v| !v.is_finite())
        || (0..3).any(|i| m[i][3].abs() > AFFINE_TOLERANCE)
        || (m[3][3] - 1.).abs() > AFFINE_TOLERANCE
    {
        return Err(fail("node transform must be finite affine"));
    }
    let c = columns(m);
    let scale = (dot(c[0], c[0]) * dot(c[1], c[1]) * dot(c[2], c[2])).sqrt();
    if !scale.is_finite() || scale == 0. || determinant(m).abs() < scale * MIN_RELATIVE_DETERMINANT
    {
        return Err(fail("singular or ill-conditioned node transform"));
    }
    Ok(m)
}
