//! Owned native-order reference snapshots without dynamic dictionary conversion.

use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyfunction;

use crate::{error, inspection::ReferenceBone};

/// Returns loaded-mesh reference bones in actual native order without mutation.
#[gen_stub_pyfunction]
#[pyfunction]
pub fn get_reference_skeleton(path: &str) -> PyResult<Vec<ReferenceBone>> {
    let skeleton = mesh_runtime::skeletal::get_reference_skeleton(path).map_err(error)?;
    Ok(skeleton.iter().map(ReferenceBone::from).collect())
}
