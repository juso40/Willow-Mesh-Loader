//! Named and tagged Gestalt preparation with final reflected publication ranges.

use std::collections::{BTreeMap, BTreeSet};

use mesh_runtime::skeletal::PreparedSkeletalMesh;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

use crate::{
    error,
    inspection::SourceSection,
    inspection::SourceSelection,
    layout::SourceFragmentRange,
    materials::{MaterialMapping, section_material_paths},
    prepared::{MeshData, PreparedMesh},
    results::FragmentRange,
    skeletal::ImportedSkin,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    source: &mesh_import::Source,
    template: &str,
    fragment_names: Vec<String>,
    materials: Vec<String>,
    material_slots: Vec<usize>,
    uv_channels: usize,
    scale: f32,
    selection: SourceSelection,
) -> PyResult<PreparedMesh> {
    let imported = ImportedSkin::load(source, scale)?;
    if imported.geometry.tex_coords != uv_channels {
        return Err(PyRuntimeError::new_err(
            "Gestalt UV count does not match the export metadata",
        ));
    }
    let expected: BTreeSet<_> = fragment_names
        .into_iter()
        .map(|name| name.to_lowercase())
        .collect();
    let actual: BTreeSet<_> = imported
        .geometry
        .fragments
        .iter()
        .map(|part| part.name.to_lowercase())
        .collect();
    let mut cursor = 0;
    for part in &imported.geometry.fragments {
        if part.first_index != cursor || part.triangles == 0 {
            return Err(PyRuntimeError::new_err(
                "Gestalt tags must cover every triangle exactly once",
            ));
        }
        cursor += part.triangles * mesh_runtime::encoding::TRIANGLE_INDICES;
    }
    if cursor != imported.geometry.indices.len() || actual != expected || expected.is_empty() {
        return Err(PyRuntimeError::new_err(
            "Gestalt source does not contain exactly the declared fragment identities",
        ));
    }
    let paths = materials;
    let slots = material_slots;
    // Gestalt draws match fragment MaterialIndex against section MaterialIndex.
    // Aliased source slots would create duplicate sections drawing the same fragments.
    if slots.iter().copied().collect::<BTreeSet<_>>().len() != slots.len() {
        return Err(PyRuntimeError::new_err(
            "Gestalt material-slot mappings must be distinct",
        ));
    }
    let mut sections = Vec::new();
    for section in &imported.geometry.sections {
        let material_index = section
            .source_material
            .and_then(|slot| slots.get(slot))
            .copied()
            .filter(|&slot| slot < paths.len())
            .ok_or_else(|| {
                PyRuntimeError::new_err(
                    "every Gestalt source material needs an explicit template material slot",
                )
            })?;
        sections.push(mesh_runtime::sections::Section {
            first_index: section.first_index,
            triangles: section.triangles,
            material_index,
        });
    }
    let parts: Vec<_> = imported
        .fragment_ranges()?
        .into_iter()
        .map(|part| {
            let section = sections
                .get(part.source_section)
                .ok_or_else(|| PyRuntimeError::new_err("invalid final fragment section"))?;
            Ok(FragmentRange {
                name: part.name,
                material_slot: section.material_index,
                first_index: part.first_index,
                triangle_count: part.triangle_count,
                bounds: part.bounds,
            })
        })
        .collect::<PyResult<_>>()?;
    let materials: Vec<_> = paths.iter().map(String::as_str).collect();
    let input = mesh_runtime::skeletal::SkinnedGeometryInput {
        sections: &sections,
        ..imported.input()
    };
    let mut prepared = PreparedSkeletalMesh::new(&input, materials.len()).map_err(error)?;
    prepared.bind_template(template).map_err(error)?;
    let info = imported.description(selection, Vec::new());
    Ok(PreparedMesh::new(
        MeshData::Gestalt {
            mesh: prepared,
            parts,
        },
        info,
        paths,
    ))
}

pub(crate) fn prepare_parts(
    sources: Vec<(mesh_import::Source, String)>,
    material: &str,
    mapping: Option<&MaterialMapping>,
    template: Option<&str>,
    scale: f32,
    selection: SourceSelection,
) -> PyResult<PreparedMesh> {
    let template_skeleton = template
        .map(mesh_runtime::skeletal::get_reference_skeleton)
        .transpose()
        .map_err(error)?;
    let mut combined: Option<PreparedSkeletalMesh> = None;
    let mut parts = Vec::new();
    let mut paths = Vec::new();
    let mut info = crate::prepared::MeshDescription {
        source: selection,
        imported_vertex_count: 0,
        primitive_count: 0,
        source_sections: Vec::new(),
        source_fragments: Vec::new(),
        warnings: Vec::new(),
    };
    let mut used_materials = BTreeSet::new();
    for (source, name) in sources {
        let imported = ImportedSkin::load(&source, scale)?;
        used_materials.extend(
            imported
                .geometry
                .sections
                .iter()
                .map(|section| section.source_material),
        );
        let selected_mapping = mapping.map(|mapping| {
            mapping
                .iter()
                .filter(|(slot, _)| {
                    imported
                        .geometry
                        .sections
                        .iter()
                        .any(|section| section.source_material == **slot)
                })
                .map(|(&slot, path)| (slot, path.clone()))
                .collect::<MaterialMapping>()
        });
        let materials = section_material_paths(
            &imported.geometry.sections,
            material,
            selected_mapping.as_ref(),
        )
        .map_err(error)?;
        let mut prepared =
            PreparedSkeletalMesh::new(&imported.input(), materials.len()).map_err(error)?;
        if let Some(skeleton) = &template_skeleton {
            prepared.bind_skeleton(skeleton).map_err(error)?;
        }
        if let Some(existing) = &combined {
            prepared
                .bind_skeleton(&existing.mesh().skeleton)
                .map_err(error)?;
        }
        let first_index = combined
            .as_ref()
            .map_or(0, |mesh| mesh.mesh().skin.triangles * 3);
        let material_offset = paths.len();
        let section_offset = info.source_sections.len();
        let fragments = if imported.geometry.fragments.is_empty() {
            imported
                .geometry
                .sections
                .iter()
                .enumerate()
                .map(|(section, range)| SourceFragmentRange {
                    name: name.clone(),
                    source_section: section,
                    first_index: range.first_index,
                    triangle_count: range.triangles,
                    bounds: prepared.mesh().skin.bounds.into(),
                })
                .collect::<Vec<_>>()
        } else {
            imported.fragment_ranges()?
        };
        let mut cursor = 0;
        for fragment in fragments {
            if fragment.first_index != cursor || fragment.triangle_count == 0 {
                return Err(error(
                    "Gestalt fragment tags must cover every triangle exactly once",
                ));
            }
            cursor += fragment.triangle_count * 3;
            parts.push(FragmentRange {
                name: fragment.name.clone(),
                material_slot: material_offset + fragment.source_section,
                first_index: first_index + fragment.first_index,
                triangle_count: fragment.triangle_count,
                bounds: fragment.bounds.clone(),
            });
            info.source_fragments.push(SourceFragmentRange {
                source_section: section_offset + fragment.source_section,
                first_index: first_index + fragment.first_index,
                ..fragment
            });
        }
        if cursor != imported.geometry.indices.len() {
            return Err(error("Gestalt tags do not cover the complete mesh"));
        }
        info.imported_vertex_count += imported.geometry.vertices.len();
        info.primitive_count += imported.geometry.primitives;
        for section in &imported.geometry.sections {
            let mut section = SourceSection::from(section);
            section.first_index += first_index;
            info.source_sections.push(section);
        }
        info.warnings.extend(imported.geometry.warnings);
        combined = Some(match combined {
            None => prepared,
            Some(previous) => previous.concatenate(&prepared).map_err(error)?,
        });
        paths.extend(materials);
    }
    if mapping.is_some_and(|mapping| mapping.keys().any(|slot| !used_materials.contains(slot))) {
        return Err(error(
            "material mapping contains unused/unknown source slot",
        ));
    }
    let mesh =
        combined.ok_or_else(|| error("Gestalt preparation requires at least one mesh node"))?;
    let mut boxes = BTreeMap::<String, ([f32; 3], [f32; 3])>::new();
    for part in &parts {
        let origin: [f32; 3] = part.bounds.origin.into();
        let extent: [f32; 3] = part.bounds.box_extent.into();
        let minimum = std::array::from_fn(|axis| origin[axis] - extent[axis]);
        let maximum = std::array::from_fn(|axis| origin[axis] + extent[axis]);
        boxes
            .entry(part.name.to_lowercase())
            .and_modify(|(min, max)| {
                for axis in 0..3 {
                    min[axis] = min[axis].min(minimum[axis]);
                    max[axis] = max[axis].max(maximum[axis]);
                }
            })
            .or_insert((minimum, maximum));
    }
    let mut bounds = BTreeMap::<_, crate::results::MeshBounds>::new();
    for (name, (min, max)) in boxes {
        let positions = (0..8).map(|corner| {
            std::array::from_fn(|axis| {
                if corner & (1 << axis) == 0 {
                    min[axis]
                } else {
                    max[axis]
                }
            })
        });
        bounds.insert(
            name,
            mesh_runtime::encoding::bounds_from_positions(positions)
                .map_err(error)?
                .into(),
        );
    }
    for part in &mut parts {
        part.bounds = bounds[&part.name.to_lowercase()].clone();
    }
    for fragment in &mut info.source_fragments {
        fragment.bounds = bounds[&fragment.name.to_lowercase()].clone();
    }
    Ok(PreparedMesh::new(
        MeshData::Gestalt { mesh, parts },
        info,
        paths,
    ))
}
