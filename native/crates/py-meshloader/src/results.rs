//! Narrow native outcomes for loading and Gestalt definition assembly.

use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;

/// Created mesh identity and import warnings, without diagnostic geometry snapshots.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct MeshOutcome {
    /// Canonical engine object path.
    pub path: String,
    /// Nonfatal source-import diagnostics.
    pub warnings: Vec<String>,
}

/// Appended master identity and final ranges for the newly added fragments.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct GestaltAppendOutcome {
    /// Canonical master mesh path.
    pub path: String,
    /// New fragment ranges in the extended master index and material spaces.
    pub parts: Vec<FragmentRange>,
    /// Nonfatal source-import diagnostics.
    pub warnings: Vec<String>,
}

/// Created or replaced animation asset identity and source-import warnings.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct AnimationOutcome {
    /// Canonical AnimSet or AnimSequence object path.
    pub path: String,
    /// Nonfatal reference-pose and sampled-animation diagnostics.
    pub warnings: Vec<String>,
}

/// Local-space box and sphere bounds.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct MeshBounds {
    /// Box and sphere center.
    pub origin: (f32, f32, f32),
    /// Box half extents.
    pub box_extent: (f32, f32, f32),
    /// Bounding sphere radius.
    pub sphere_radius: f32,
}

impl From<mesh_runtime::encoding::Bounds> for MeshBounds {
    fn from(bounds: mesh_runtime::encoding::Bounds) -> Self {
        Self {
            origin: bounds.origin.into(),
            box_extent: bounds.box_extent.into(),
            sphere_radius: bounds.sphere_radius,
        }
    }
}

/// Final native index range and complete reference-pose bounds for a Gestalt fragment.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct FragmentRange {
    /// Original fragment identity.
    pub name: String,
    /// Destination material-slot ordinal.
    pub material_slot: usize,
    /// Offset into the final index buffer.
    pub first_index: usize,
    /// Triangle count in this range.
    pub triangle_count: usize,
    /// Bounds of the entire named fragment, including its other ranges.
    pub bounds: MeshBounds,
}
