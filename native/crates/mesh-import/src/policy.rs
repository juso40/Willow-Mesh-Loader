//! Native skeleton limits and numerical validation tolerances.

pub(crate) const MAX_JOINTS: usize = u8::MAX as usize + 1;
pub(crate) const MAX_JOINT_NAME_UNITS: usize = 1023;
pub(crate) const GLB_PADDING_BYTES: usize = 3;
pub(crate) const MIN_VECTOR_LENGTH: f64 = 1e-20;
pub(crate) const MIN_PROJECTED_TANGENT_NORM2: f64 = 1e-20;
pub(crate) const MIN_UV_DETERMINANT: f64 = 1e-12;
pub(crate) const MIN_QUATERNION_NORM: f64 = 1e-12;
pub(crate) const AFFINE_TOLERANCE: f64 = 1e-6;
pub(crate) const MIN_RELATIVE_DETERMINANT: f64 = 1e-10;
pub(crate) const RIGID_BASIS_TOLERANCE: f64 = 1e-4;
pub(crate) const RIGID_ORTHOGONALITY_TOLERANCE: f64 = 1e-5;
pub(crate) const RIGID_CORRECTION_WARNING: f64 = 1e-6;
pub(crate) const QUATERNION_NORM_TOLERANCE: f64 = 0.001;
pub(crate) const TANGENT_SIGN_TOLERANCE: f32 = 0.001;
pub(crate) const FALLBACK_AXIS_MAX_ALIGNMENT: f64 = 0.8;

pub(crate) fn validate_scale(scale: f32) -> crate::Result<()> {
    if !scale.is_finite() || scale <= 0. {
        return Err(crate::fail(
            "scale must be finite, positive and representable as f32",
        ));
    }
    Ok(())
}
