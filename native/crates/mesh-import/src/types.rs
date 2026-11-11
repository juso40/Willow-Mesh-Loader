//! Imported engine-space geometry and source material ranges.

/// One engine-space vertex with an orthonormal basis, up to four UV sets and optional color.
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    /// Baked native position using the selected coordinate policy and scale.
    pub position: [f32; 3],
    /// Unit tangent.
    pub tangent_x: [f32; 3],
    /// Unit signed bitangent.
    pub tangent_y: [f32; 3],
    /// Unit inverse-transpose-transformed normal.
    pub tangent_z: [f32; 3],
    /// TEXCOORD_0 unchanged, or zero if absent.
    pub uv: [f32; 2],
    /// TEXCOORD_1 through TEXCOORD_3, with zero pairs for absent channels.
    pub extra_uvs: [[f32; 2]; 3],
    /// COLOR_0 quantized to linear RGBA8, or absent for uncolored primitives.
    pub color: Option<[u8; 4]>,
}

/// A contiguous triangle range, grouped by source material in first-use order.
#[derive(Debug, Clone)]
pub struct Section {
    /// Index-element offset, not bytes.
    pub first_index: usize,
    /// Nonempty triangle count.
    pub triangles: usize,
    /// Original glTF material index, None for the default material.
    pub source_material: Option<usize>,
}

/// Static scene flattened into one native LOD with bounded material sections.
#[derive(Debug)]
pub struct Geometry {
    /// Indexed vertices with exact normal, tangent, UV and color seams retained.
    pub vertices: Vec<Vertex>,
    /// Clockwise triangle list in the selected engine-space coordinate convention.
    pub indices: Vec<u32>,
    /// Explicitly selected scene index.
    pub scene: usize,
    /// Mesh-node instances baked into the output.
    pub instances: usize,
    /// Flattened primitive count before grouping by material.
    pub primitives: usize,
    /// Grouped index ranges. Distinct source slots remain distinct override slots.
    pub sections: Vec<Section>,
    /// Nonfatal import diagnostics.
    pub warnings: Vec<String>,
}
