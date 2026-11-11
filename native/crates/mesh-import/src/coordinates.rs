//! Shared geometry axes and explicit source-workflow bone frames.

/// Source-to-engine geometry axis mapping used by every import operation.
pub const DESCRIPTION: &str = "(glTF X, glTF Z, glTF Y) * scale";

/// Local frame correction after source-to-engine conversion.
///
/// Blender's Y-up export adds a bone-specific quarter-turn to PSK importer
/// frames. Post-multiplying every global joint by engine X -90 degrees cancels
/// it without moving joint heads or changing geometry coordinates.
pub(crate) const PSK_BONE_BASIS: crate::math::Matrix = [
    [1., 0., 0., 0.],
    [0., 0., -1., 0.],
    [0., 1., 0., 0.],
    [0., 0., 0., 1.],
];

pub(crate) fn matrix(scale: f64) -> crate::math::Matrix {
    [
        [scale, 0., 0., 0.],
        [0., 0., scale, 0.],
        [0., scale, 0., 0.],
        [0., 0., 0., 1.],
    ]
}
