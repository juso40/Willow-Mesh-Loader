//! Native mesh extension loaded by the meshloader pyunrealsdk library mod.

use pyo3::prelude::*;

/// Provides metadata for Python type-stub generation on the build host.
pub fn stub_info() -> pyo3_stub_gen::Result<pyo3_stub_gen::StubInfo> {
    pyo3_stub_gen::StubInfo::from_project_root(
        "meshloader._meshloader".into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        false,
        Default::default(),
    )
}

#[pymodule]
fn _meshloader(module: &Bound<'_, PyModule>) -> PyResult<()> {
    py_meshloader::register(module)
}
