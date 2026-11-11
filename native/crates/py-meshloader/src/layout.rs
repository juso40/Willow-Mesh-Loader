//! Explicit encoded layout and source fragment diagnostics.

use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;

use crate::results::MeshBounds;

/// Checked static section vertex bounds for explicit diagnostics.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SectionVertexRange {
    /// Minimum referenced encoded vertex ordinal.
    pub min_vertex_index: u32,
    /// Maximum referenced encoded vertex ordinal.
    pub max_vertex_index: u32,
}

/// Explicit static index layout diagnostics.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct StaticLayout {
    /// Encoded index width, currently two bytes.
    pub index_size_bytes: usize,
    /// Checked vertex bounds in source-section order.
    pub section_vertex_ranges: Vec<SectionVertexRange>,
}

/// Contiguous encoded skin range with a GPU-local bone map.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SkeletalChunkInfo {
    /// First vertex in the shared encoded vertex buffer.
    pub base_vertex: usize,
    /// Vertices in this chunk.
    pub vertex_count: usize,
    /// First triangle-list index element in this chunk.
    pub first_index: usize,
    /// Consecutive triangles in this chunk.
    pub triangle_count: usize,
    /// Local influence byte to containing inspection reference-bone ordinal.
    pub palette: Vec<u16>,
    /// Largest nonzero quantized influence count in this chunk.
    pub max_bone_influences: u32,
}

/// Explicit GPU skinning layout diagnostics, not construction input.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SkeletalLayout {
    /// Encoded index width, either two or four bytes.
    pub index_size_bytes: usize,
    /// Stable contiguous GPU partitions.
    pub chunks: Vec<SkeletalChunkInfo>,
    /// Sorted unique weighted reference-bone ordinals, not a GPU-local palette.
    pub weighted_bones: Vec<u16>,
}

/// Tagged source range before live-template material binding.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceFragmentRange {
    /// Authored fragment identity.
    pub name: String,
    /// Ordinal into the containing source_sections collection.
    pub source_section: usize,
    /// First triangle-list index element.
    pub first_index: usize,
    /// Consecutive triangles in this range.
    pub triangle_count: usize,
    /// Bounds of the entire named fragment across all of its ranges.
    pub bounds: MeshBounds,
}
