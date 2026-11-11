//! Static geometry preparation without eager Python inspection materialization.

use mesh_runtime::static_mesh::PreparedStaticMesh;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

use crate::{
    convert, error,
    inspection::{SourceSection, SourceSelection},
    materials::{MaterialMapping, section_material_paths},
    prepared::{MeshData, MeshDescription, PreparedMesh},
};

pub(crate) fn prepare_source(
    source: &mesh_import::Source,
    material: &str,
    scale: f32,
    mapping: Option<&MaterialMapping>,
    selection: SourceSelection,
) -> PyResult<PreparedMesh> {
    let geometry = source.static_geometry(scale).map_err(error)?;
    let vertices: Vec<_> = geometry.vertices.iter().map(convert::vertex).collect();
    let sections = convert::sections(&geometry.sections);
    let materials = section_material_paths(&geometry.sections, material, mapping)
        .map_err(PyRuntimeError::new_err)?;
    let prepared =
        PreparedStaticMesh::new(&vertices, &geometry.indices, &sections, materials.len())
            .map_err(error)?;
    let info = MeshDescription {
        source: selection,
        imported_vertex_count: geometry.vertices.len(),
        primitive_count: geometry.primitives,
        source_sections: geometry.sections.iter().map(SourceSection::from).collect(),
        source_fragments: Vec::new(),
        warnings: geometry.warnings,
    };
    Ok(PreparedMesh::new(
        MeshData::Static(prepared),
        info,
        materials,
    ))
}
