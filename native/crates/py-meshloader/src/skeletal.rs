//! Skeletal conversion and source descriptions without eager inspection snapshots.

use mesh_runtime::skeletal::{PreparedSkeletalMesh, SkinnedGeometryInput};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use std::{collections::BTreeMap, ops::Range};

use crate::{
    convert, error,
    inspection::{SourceSection, SourceSelection},
    layout::SourceFragmentRange,
    materials::{MaterialMapping, section_material_paths},
    prepared::{MeshData, MeshDescription, PreparedMesh},
};

pub(crate) struct ImportedSkin {
    pub(crate) geometry: mesh_import::skin::SkinnedGeometry,
    vertices: Vec<mesh_runtime::encoding::Vertex>,
    weights: Vec<mesh_runtime::skeletal::SkinWeights>,
    extra_uvs: Vec<Vec<[f32; 2]>>,
    skeleton: Vec<mesh_runtime::skeletal::ReferenceBone>,
    sections: Vec<mesh_runtime::sections::Section>,
}

impl ImportedSkin {
    pub(crate) fn load(source: &mesh_import::Source, scale: f32) -> PyResult<Self> {
        let geometry = source.skeletal_geometry(scale).map_err(error)?;
        let vertices = geometry
            .vertices
            .iter()
            .map(|vertex| convert::vertex(&vertex.vertex))
            .collect();
        let weights = geometry
            .vertices
            .iter()
            .map(|vertex| mesh_runtime::skeletal::SkinWeights {
                joints: vertex.joints,
                weights: vertex.weights,
            })
            .collect();
        let skeleton = geometry.bones.iter().cloned().map(convert::bone).collect();
        let sections = convert::sections(&geometry.sections);
        let extra_uvs = (1..geometry.tex_coords)
            .map(|channel| {
                geometry
                    .vertices
                    .iter()
                    .map(|vertex| vertex.vertex.extra_uvs[channel - 1])
                    .collect()
            })
            .collect();
        Ok(Self {
            geometry,
            vertices,
            weights,
            extra_uvs,
            skeleton,
            sections,
        })
    }

    pub(crate) fn input(&self) -> SkinnedGeometryInput<'_> {
        SkinnedGeometryInput {
            vertices: &self.vertices,
            indices: &self.geometry.indices,
            weights: &self.weights,
            extra_uvs: &self.extra_uvs,
            skeleton: &self.skeleton,
            sections: &self.sections,
        }
    }

    pub(crate) fn fragment_ranges(&self) -> PyResult<Vec<SourceFragmentRange>> {
        let mut ranges: BTreeMap<String, Vec<Range<usize>>> = BTreeMap::new();
        for part in &self.geometry.fragments {
            ranges.entry(part.name.to_lowercase()).or_default().push(
                part.first_index
                    ..part.first_index + part.triangles * mesh_runtime::encoding::TRIANGLE_INDICES,
            );
        }
        let mut bounds = BTreeMap::new();
        for (name, ranges) in ranges {
            let positions = ranges
                .iter()
                .flat_map(|range| self.geometry.indices[range.clone()].iter())
                .map(|&index| self.vertices[index as usize].position);
            bounds.insert(
                name,
                mesh_runtime::encoding::bounds_from_positions(positions).map_err(error)?,
            );
        }
        Ok(self
            .geometry
            .fragments
            .iter()
            .map(|part| SourceFragmentRange {
                name: part.name.clone(),
                source_section: part.material_index,
                first_index: part.first_index,
                triangle_count: part.triangles,
                bounds: bounds[&part.name.to_lowercase()].into(),
            })
            .collect())
    }

    pub(crate) fn description(
        self,
        source: SourceSelection,
        source_fragments: Vec<SourceFragmentRange>,
    ) -> MeshDescription {
        MeshDescription {
            source,
            imported_vertex_count: self.geometry.vertices.len(),
            primitive_count: self.geometry.primitives,
            source_sections: self
                .geometry
                .sections
                .iter()
                .map(SourceSection::from)
                .collect(),
            source_fragments,
            warnings: self.geometry.warnings,
        }
    }
}

pub(crate) fn prepare_source(
    source: &mesh_import::Source,
    material: &str,
    scale: f32,
    mapping: Option<&MaterialMapping>,
    selection: SourceSelection,
) -> PyResult<PreparedMesh> {
    let imported = ImportedSkin::load(source, scale)?;
    let materials = section_material_paths(&imported.geometry.sections, material, mapping)
        .map_err(PyRuntimeError::new_err)?;
    let prepared = PreparedSkeletalMesh::new(&imported.input(), materials.len()).map_err(error)?;
    let fragments = imported.fragment_ranges()?;
    let info = imported.description(selection, fragments);
    Ok(PreparedMesh::new(
        MeshData::Skeletal(prepared),
        info,
        materials,
    ))
}
