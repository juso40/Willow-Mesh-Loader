//! Typed preparation snapshots independent of native payload ownership.

use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

use crate::{catalog::SourceIdentity, results::MeshBounds};

/// Selected-source provenance without retaining its document or buffers.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceSelection {
    /// Canonical source filesystem path at opening.
    pub path: String,
    /// Selected source scene.
    pub scene: SourceIdentity,
    /// Selected mesh node, absent for multi-node Gestalt or geometry-free animations.
    pub node: Option<SourceIdentity>,
    /// Selected source skin when applicable.
    pub skin: Option<SourceIdentity>,
    /// Effective source-to-engine unit multiplier.
    pub scale: f32,
    /// Applied skeletal frame convention: psk or gltf, absent for static data.
    pub bone_frames: Option<String>,
    /// Loaded mesh used for Gestalt binding, if applicable.
    pub template_mesh: Option<String>,
}

/// Reference-pose bone in the containing snapshot skeleton order.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct ReferenceBone {
    /// Native-compatible bone name.
    pub name: String,
    /// Parent bone ordinal, or none for the root.
    pub parent: Option<u16>,
    /// Parent-relative translation in engine units.
    pub translation: (f32, f32, f32),
    /// Parent-relative unit quaternion in XYZW order.
    pub rotation_xyzw: (f32, f32, f32, f32),
}

/// Source material partition before skeletal draw-section splitting.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceSection {
    /// Original glTF material ID, or none for unassigned faces.
    pub source_material: Option<usize>,
    /// Offset measured in index elements, not bytes.
    pub first_index: usize,
    /// Consecutive triangles in this source partition.
    pub triangle_count: usize,
}

/// Portable preparation counts, not total process or GPU memory usage.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct MeshStatistics {
    /// Vertex count after importer seam handling and deduplication.
    pub imported_vertex_count: usize,
    /// Encoded vertex count including GPU chunk-boundary duplication.
    pub vertex_count: usize,
    /// Encoded triangle count.
    pub triangle_count: usize,
    /// Consumed source primitive count before material grouping.
    pub primitive_count: usize,
    /// Primary encoded render streams, excluding native records and retained storage.
    pub encoded_render_bytes: usize,
}

/// Static preparation snapshot without an engine destination.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct StaticInspection {
    /// Selected source and effective conversion options.
    pub source: SourceSelection,
    /// Portable geometry statistics.
    pub stats: MeshStatistics,
    /// Incoming baked geometry bounds, not final expanded replacement bounds.
    pub bounds: MeshBounds,
    /// Source material partitions in engine material-slot order.
    pub source_sections: Vec<SourceSection>,
    /// Nonfatal source preparation diagnostics.
    pub warnings: Vec<String>,
}

/// Skeletal preparation snapshot in prepared or bound-template bone order.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SkeletalInspection {
    /// Selected source and effective bone-frame options.
    pub source: SourceSelection,
    /// Portable geometry statistics.
    pub stats: MeshStatistics,
    /// Reference-pose geometry bounds, not arbitrary animated bounds.
    pub bounds: MeshBounds,
    /// Source partitions, not the final chunk-split draw sections.
    pub source_sections: Vec<SourceSection>,
    /// Reference bones indexed by parents and layout palettes.
    pub bones: Vec<ReferenceBone>,
    /// Encoded UV channel count, between one and four.
    pub uv_channels: usize,
    /// Largest quantized nonzero influence count.
    pub max_bone_influences: u32,
    /// Nonfatal source preparation diagnostics.
    pub warnings: Vec<String>,
}

impl From<&mesh_runtime::skeletal::ReferenceBone> for ReferenceBone {
    fn from(bone: &mesh_runtime::skeletal::ReferenceBone) -> Self {
        Self {
            name: bone.name.clone(),
            parent: bone.parent,
            translation: bone.translation.into(),
            rotation_xyzw: bone.rotation_xyzw.into(),
        }
    }
}

impl From<&mesh_import::Section> for SourceSection {
    fn from(section: &mesh_import::Section) -> Self {
        Self {
            source_material: section.source_material,
            first_index: section.first_index,
            triangle_count: section.triangles,
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl StaticInspection {
    /// Returns the static geometry discriminator.
    #[getter]
    #[gen_stub(override_return_type(type_repr="typing.Literal[\"static\"]", imports=("typing")))]
    fn kind(&self) -> &'static str {
        "static"
    }

    /// Returns the exact engine class used for fresh construction.
    #[getter]
    #[gen_stub(override_return_type(type_repr="typing.Literal[\"StaticMesh\"]", imports=("typing")))]
    fn class_name(&self) -> &'static str {
        "StaticMesh"
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl SkeletalInspection {
    /// Returns the skeletal geometry discriminator.
    #[getter]
    #[gen_stub(override_return_type(type_repr="typing.Literal[\"skeletal\"]", imports=("typing")))]
    fn kind(&self) -> &'static str {
        "skeletal"
    }

    /// Returns the exact engine class used for fresh construction.
    #[getter]
    #[gen_stub(override_return_type(type_repr="typing.Literal[\"SkeletalMesh\"]", imports=("typing")))]
    fn class_name(&self) -> &'static str {
        "SkeletalMesh"
    }
}

/// Prepared absolute clip identity and endpoint-inclusive sample grid.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct AnimationClipInfo {
    /// Source-local animation ID, not a frame or track index.
    pub source_animation_id: usize,
    /// Authored clip name retained as SequenceName.
    pub name: String,
    /// Positive playback duration in seconds.
    pub duration: f32,
    /// Uniform samples, including both endpoints for a nonzero source duration.
    pub frame_count: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl AnimationClipInfo {
    /// Returns the effective endpoint-inclusive rate, or none for a single pose.
    #[getter]
    fn effective_sample_rate(&self) -> Option<f64> {
        (self.frame_count > 1).then(|| (self.frame_count - 1) as f64 / self.duration as f64)
    }
}

/// Prepared source animation metadata without copied track samples.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct AnimationInspection {
    /// Selected skin, scene and effective bone-frame options.
    pub source: SourceSelection,
    /// Requested sampling rate, not necessarily the exact per-clip rate.
    pub requested_sample_rate: f32,
    /// Source sample-track order, before loaded-target track binding.
    pub bones: Vec<ReferenceBone>,
    /// Selected clip descriptors in document order.
    pub clips: Vec<AnimationClipInfo>,
    /// Nonfatal source preparation diagnostics.
    pub warnings: Vec<String>,
}
