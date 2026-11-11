//! Python marshaling for selected asset preparation, population and replacement.

const DEFAULT_MATERIAL_PATH: &str = "EngineMaterials.DefaultMaterial";

mod animation;
mod catalog;
mod convert;
mod gestalt;
mod inspection;
mod layout;
mod materials;
mod prepared;
mod reference;
mod results;
mod skeletal;
mod source;
mod static_mesh;

pub use prepared::{PreparedAnimationSet, PreparedMesh};
pub use reference::get_reference_skeleton;
pub use results::{AnimationOutcome, FragmentRange, GestaltAppendOutcome, MeshBounds, MeshOutcome};
pub use source::Source;

use pyo3::prelude::*;

/// Registers the typed native boundary records and prepared-handle APIs.
pub fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(reference::get_reference_skeleton, module)?)?;
    module.add_class::<Source>()?;
    module.add_class::<PreparedMesh>()?;
    module.add_class::<PreparedAnimationSet>()?;
    module.add_class::<MeshOutcome>()?;
    module.add_class::<GestaltAppendOutcome>()?;
    module.add_class::<AnimationOutcome>()?;
    module.add_class::<MeshBounds>()?;
    module.add_class::<FragmentRange>()?;
    module.add_class::<catalog::SourceIdentity>()?;
    module.add_class::<catalog::SourceMeshIdentity>()?;
    module.add_class::<catalog::SourceIdentities>()?;
    module.add_class::<catalog::SourceScene>()?;
    module.add_class::<catalog::SourceMesh>()?;
    module.add_class::<catalog::SourceSkin>()?;
    module.add_class::<catalog::SourceAnimation>()?;
    module.add_class::<catalog::SourceCatalog>()?;
    module.add_class::<inspection::SourceSelection>()?;
    module.add_class::<inspection::ReferenceBone>()?;
    module.add_class::<inspection::SourceSection>()?;
    module.add_class::<inspection::MeshStatistics>()?;
    module.add_class::<inspection::StaticInspection>()?;
    module.add_class::<inspection::SkeletalInspection>()?;
    module.add_class::<inspection::AnimationClipInfo>()?;
    module.add_class::<inspection::AnimationInspection>()?;
    module.add_class::<layout::SectionVertexRange>()?;
    module.add_class::<layout::StaticLayout>()?;
    module.add_class::<layout::SkeletalChunkInfo>()?;
    module.add_class::<layout::SkeletalLayout>()?;
    module.add_class::<layout::SourceFragmentRange>()?;
    Ok(())
}

pub(crate) fn error(error: impl std::fmt::Display) -> pyo3::PyErr {
    pyo3::exceptions::PyRuntimeError::new_err(error.to_string())
}
