//! Imported skinned vertices and bind-derived reference bones.

use crate::{Section, Vertex};

/// One expanded skinned vertex: engine-space attributes plus global joints.
#[derive(Debug, Clone, Copy)]
pub struct SkinnedVertex {
    /// Engine-space position/basis/UV, same conversion as static import.
    pub vertex: Vertex,
    /// Up to four global bone indices in output bone order (zero-weight slots 0).
    pub joints: [u16; 4],
    /// Raw nonnegative weights with nonzero sum. Runtime normalizes/quantizes.
    pub weights: [f32; 4],
}

/// Reference bone in engine space, parent-before-child. Names come from glTF
/// node names and must satisfy the engine's bounded identifier rules.
#[derive(Debug, Clone)]
pub struct ImportBone {
    /// Node name used for the engine FName at runtime.
    pub name: String,
    /// Nearest ancestor joint. Root is None.
    pub parent: Option<u16>,
    /// Parent-relative translation in engine units.
    pub translation: [f32; 3],
    /// Parent-relative XYZW unit quaternion.
    pub rotation_xyzw: [f32; 4],
}

/// One named primitive range after final material grouping.
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Engine fragment identity supplied by primitive extras.
    pub name: String,
    /// Offset in the final index buffer, measured in index elements.
    pub first_index: usize,
    /// Nonempty triangle count.
    pub triangles: usize,
    /// Final native material section ordinal.
    pub material_index: usize,
}

/// One skinned mesh with its own skeleton, flattened to one native LOD.
#[derive(Debug)]
pub struct SkinnedGeometry {
    /// Indexed vertices with exact tangent, UV and influence seams retained.
    pub vertices: Vec<SkinnedVertex>,
    /// Clockwise triangle list in the static-import engine convention.
    pub indices: Vec<u32>,
    /// Parent-before-child bones derived from inverse bind matrices.
    pub bones: Vec<ImportBone>,
    /// Explicitly selected scene index.
    pub scene: usize,
    /// Flattened primitive count before grouping by material.
    pub primitives: usize,
    /// Grouped index ranges by source material in first-use order.
    pub sections: Vec<Section>,
    /// Named ranges after final index-buffer assembly. Empty for ordinary glTF.
    pub fragments: Vec<Fragment>,
    /// Number of UV channels required by any primitive, between one and four.
    pub tex_coords: usize,
    /// Nonfatal import diagnostics.
    pub warnings: Vec<String>,
}
