//! Checked inverse bind reconstruction and reference-pose transforms.

use crate::math::*;
use crate::{Result, fail, policy::*};

/// Inverts an affine matrix whose rotation columns are equal-length orthogonal.
/// Handles both rigid transforms and the scaled axis conversion.
pub(crate) fn inverse_orthogonal_scaled(m: Matrix) -> Result<Matrix> {
    let c = [m[0], m[1], m[2]].map(|col| [col[0], col[1], col[2]]);
    let lens = c.map(|v| v.iter().map(|x| x * x).sum::<f64>());
    let scale2 = lens[0];
    if !scale2.is_finite()
        || scale2 <= 0.
        || (lens[1] - scale2).abs() > scale2 * RIGID_BASIS_TOLERANCE
        || (lens[2] - scale2).abs() > scale2 * RIGID_BASIS_TOLERANCE
    {
        return Err(fail("joint bind transform has nonuniform scale"));
    }
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        let d = c[a][0] * c[b][0] + c[a][1] * c[b][1] + c[a][2] * c[b][2];
        // Composed/exported f32 bind matrices can have a few parts-per-million
        // orthogonality residue even when their lengths/determinant are rigid.
        // Tolerate that numerical noise, not authored shear. This inverse uses
        // the transpose approximation. Keep its tolerance below the 1e-4 rigid
        // and rest/bind checks rather than allowing general affine transforms.
        if d.abs() > scale2 * RIGID_ORTHOGONALITY_TOLERANCE {
            return Err(fail("joint bind transform has shear"));
        }
    }
    let t = [m[3][0], m[3][1], m[3][2]];
    let mut out = [[0.; 4]; 4];
    for r in 0..3 {
        // Columns of R^-1 are rows of R, divided by the squared column length.
        out[r][0] = c[0][r] / scale2;
        out[r][1] = c[1][r] / scale2;
        out[r][2] = c[2][r] / scale2;
        out[3][r] = -(c[r][0] * t[0] + c[r][1] * t[1] + c[r][2] * t[2]) / scale2;
    }
    out[3][3] = 1.;
    Ok(out)
}

/// Inverts a near-rigid bind accurately and removes only bounded rotation
/// residue. Unlike a transpose approximation, the full inverse preserves joint
/// positions even when export roundoff leaves slightly skewed axes. The 1e-4
/// envelope matches the existing rigid length/determinant checks. This is not
/// support for arbitrary affine binds, and any meaningful cleanup is reported.
pub(crate) fn canonical_bind_global(m: Matrix) -> Result<(Matrix, bool)> {
    const TOLERANCE: f64 = RIGID_BASIS_TOLERANCE;
    let c = [m[0], m[1], m[2]].map(|v| [v[0], v[1], v[2]]);
    let det = determinant(m);
    if (det - 1.).abs() > TOLERANCE || c.iter().any(|v| (dot(*v, *v) - 1.).abs() > TOLERANCE) {
        return Err(fail(
            "joint binds must be rigid: scale/shear/mirror is unsupported",
        ));
    }
    let shear = [(0, 1), (0, 2), (1, 2)]
        .map(|(a, b)| dot(c[a], c[b]).abs())
        .into_iter()
        .fold(0., f64::max);
    if shear > TOLERANCE {
        return Err(fail(&format!(
            "joint bind transform has shear (axis dot {shear:.6e} exceeds {TOLERANCE:.1e})"
        )));
    }
    // Inverse rows are the column cofactors divided by the determinant.
    let rows = [cross(c[1], c[2]), cross(c[2], c[0]), cross(c[0], c[1])].map(|v| mul(v, 1. / det));
    let columns: [V; 3] = std::array::from_fn(|col| std::array::from_fn(|row| rows[row][col]));
    let x = unit(columns[0])?;
    let y = unit(sub(columns[1], mul(x, dot(x, columns[1]))))?;
    let z = cross(x, y); // Reflected input is refused above.
    let rotation = [x, y, z];
    let correction = (0..3)
        .flat_map(|col| (0..3).map(move |row| (rotation[col][row] - columns[col][row]).abs()))
        .fold(0., f64::max);
    if correction > TOLERANCE {
        return Err(fail(
            "joint bind rotation needs correction beyond near-rigid tolerance",
        ));
    }
    let mut out = [[0.; 4]; 4];
    for col in 0..3 {
        out[col][..3].copy_from_slice(&rotation[col]);
    }
    let t = [m[3][0], m[3][1], m[3][2]];
    for row in 0..3 {
        out[3][row] = -dot(rows[row], t);
    }
    out[3][3] = 1.;
    Ok((checked_matrix(out)?, correction > RIGID_CORRECTION_WARNING))
}

/// Extracts an XYZW unit quaternion from rotation columns (Shepperd's method).
pub(crate) fn rotation_quaternion(m: Matrix) -> Result<[f32; 4]> {
    let (a, b, c) = (
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    );
    let (m00, m11, m22) = (a[0], b[1], c[2]);
    let trace = m00 + m11 + m22;
    let q = if trace > 0. {
        let s = (trace + 1.).sqrt() * 2.;
        [
            (b[2] - c[1]) / s,
            (c[0] - a[2]) / s,
            (a[1] - b[0]) / s,
            s / 4.,
        ]
    } else if m00 > m11 && m00 > m22 {
        let s = (1. + m00 - m11 - m22).sqrt() * 2.;
        [
            s / 4.,
            (a[1] + b[0]) / s,
            (a[2] + c[0]) / s,
            (b[2] - c[1]) / s,
        ]
    } else if m11 > m22 {
        let s = (1. + m11 - m00 - m22).sqrt() * 2.;
        [
            (a[1] + b[0]) / s,
            s / 4.,
            (b[2] + c[1]) / s,
            (c[0] - a[2]) / s,
        ]
    } else {
        let s = (1. + m22 - m00 - m11).sqrt() * 2.;
        [
            (a[2] + c[0]) / s,
            (b[2] + c[1]) / s,
            s / 4.,
            (a[1] - b[0]) / s,
        ]
    };
    let norm: f64 = q.iter().map(|v| v.powi(2)).sum::<f64>().sqrt();
    if !norm.is_finite() || norm < MIN_QUATERNION_NORM {
        return Err(fail("joint rotation is not representable"));
    }
    let q = q.map(|v| v / norm);
    let out: [f32; 4] = std::array::from_fn(|i| q[i] as f32);
    if out.iter().any(|v| !v.is_finite()) {
        return Err(fail("joint rotation exceeds finite f32 range"));
    }
    Ok(out)
}

pub(crate) fn valid_bone_name(name: &str) -> bool {
    !name.is_empty() && name.encode_utf16().count() <= MAX_JOINT_NAME_UNITS && !name.contains('\0')
}

pub(crate) fn rigid(m: Matrix) -> Result<()> {
    if (determinant(m) - 1.).abs() > RIGID_BASIS_TOLERANCE
        || (0..3).any(|c| {
            (dot([m[c][0], m[c][1], m[c][2]], [m[c][0], m[c][1], m[c][2]]) - 1.).abs()
                > RIGID_BASIS_TOLERANCE
        })
    {
        return Err(fail(
            "joint binds must be rigid: scale/shear/mirror is unsupported",
        ));
    }
    inverse_orthogonal_scaled(m)?;
    Ok(())
}
