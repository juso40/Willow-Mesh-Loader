//! Opaque prepared payloads with lightweight operations and explicit typed inspection.

use crate::{
    error,
    inspection::{
        AnimationClipInfo, AnimationInspection, MeshStatistics, ReferenceBone, SkeletalInspection,
        SourceSection, SourceSelection, StaticInspection,
    },
    layout::{
        SectionVertexRange, SkeletalChunkInfo, SkeletalLayout, SourceFragmentRange, StaticLayout,
    },
    results::{AnimationOutcome, FragmentRange, GestaltAppendOutcome, MeshOutcome},
};
use mesh_runtime::{
    ReplacementPolicy, skeletal::PreparedSkeletalMesh, static_mesh::PreparedStaticMesh,
};
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

pub(crate) enum MeshData {
    Static(PreparedStaticMesh),
    Skeletal(PreparedSkeletalMesh),
    Gestalt {
        mesh: PreparedSkeletalMesh,
        parts: Vec<FragmentRange>,
    },
}

impl MeshData {
    fn populate_new(&self, path: &str, materials: &[&str]) -> mesh_runtime::Result<String> {
        match self {
            Self::Static(mesh) => mesh.populate_new(path, materials),
            Self::Skeletal(mesh) => mesh.populate_new(path, materials),
            Self::Gestalt { mesh, .. } => mesh.populate_gestalt(path, materials),
        }
    }

    fn replace(
        &self,
        path: &str,
        materials: &[&str],
        policy: ReplacementPolicy,
    ) -> mesh_runtime::Result<String> {
        match self {
            Self::Static(mesh) => mesh.replace(path, materials, policy),
            Self::Skeletal(mesh) => mesh.replace(path, materials, policy),
            Self::Gestalt { .. } => Err(mesh_runtime::Error(
                "Gestalt geometry updates require dedicated Gestalt replacement, append or named update".into(),
            )),
        }
    }
}

pub(crate) struct MeshDescription {
    pub source: SourceSelection,
    pub imported_vertex_count: usize,
    pub primitive_count: usize,
    pub source_sections: Vec<SourceSection>,
    pub source_fragments: Vec<SourceFragmentRange>,
    pub warnings: Vec<String>,
}

/// Validated mesh payload with separate construction hints and inspection snapshots.
#[gen_stub_pyclass]
#[pyclass(frozen, module = "meshloader._meshloader")]
pub struct PreparedMesh {
    data: MeshData,
    info: MeshDescription,
    materials: Vec<String>,
}

impl PreparedMesh {
    pub(crate) fn new(data: MeshData, info: MeshDescription, materials: Vec<String>) -> Self {
        Self {
            data,
            info,
            materials,
        }
    }

    fn statistics(&self, vertices: usize, triangles: usize, bytes: usize) -> MeshStatistics {
        MeshStatistics {
            imported_vertex_count: self.info.imported_vertex_count,
            vertex_count: vertices,
            triangle_count: triangles,
            primitive_count: self.info.primitive_count,
            encoded_render_bytes: bytes,
        }
    }

    fn replace(&self, path: &str, policy: ReplacementPolicy) -> PyResult<MeshOutcome> {
        let materials: Vec<_> = self.materials.iter().map(String::as_str).collect();
        let path = self.data.replace(path, &materials, policy).map_err(error)?;
        Ok(MeshOutcome {
            path,
            warnings: self.info.warnings.clone(),
        })
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PreparedMesh {
    /// Returns the exact engine class without materializing an inspection snapshot.
    #[getter]
    #[gen_stub(override_return_type(type_repr="typing.Literal[\"StaticMesh\", \"SkeletalMesh\"]", imports=("typing")))]
    fn class_name(&self) -> &'static str {
        match self.data {
            MeshData::Static(_) => "StaticMesh",
            _ => "SkeletalMesh",
        }
    }

    /// Returns source preparation warnings without copying geometry metadata.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.info.warnings.clone()
    }

    /// Returns an independent descriptive snapshot, excluding detailed GPU layout.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "StaticInspection | SkeletalInspection"))]
    fn metadata(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match &self.data {
            MeshData::Static(mesh) => {
                let payload = mesh.payload();
                let bytes = payload.positions.len()
                    + payload.tangent_uv.len()
                    + payload.colors.len()
                    + payload.indices.len();
                Ok(Py::new(
                    py,
                    StaticInspection {
                        source: self.info.source.clone(),
                        stats: self.statistics(payload.vertices, payload.triangles, bytes),
                        bounds: payload.bounds.into(),
                        source_sections: self.info.source_sections.clone(),
                        warnings: self.info.warnings.clone(),
                    },
                )?
                .into_any())
            }
            MeshData::Skeletal(mesh) | MeshData::Gestalt { mesh, .. } => {
                let fixture = mesh.mesh();
                let skin = &fixture.skin;
                Ok(Py::new(
                    py,
                    SkeletalInspection {
                        source: self.info.source.clone(),
                        stats: self.statistics(
                            skin.vertices,
                            skin.triangles,
                            skin.gpu_vertices.len() + skin.colors.len() + skin.indices.len(),
                        ),
                        bounds: skin.bounds.into(),
                        source_sections: self.info.source_sections.clone(),
                        bones: fixture.skeleton.iter().map(ReferenceBone::from).collect(),
                        uv_channels: skin.tex_coords,
                        max_bone_influences: skin.max_bone_influences,
                        warnings: self.info.warnings.clone(),
                    },
                )?
                .into_any())
            }
        }
    }

    /// Returns explicit encoded layout diagnostics without exposing packed buffers.
    #[getter]
    #[gen_stub(override_return_type(type_repr = "StaticLayout | SkeletalLayout"))]
    fn layout(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match &self.data {
            MeshData::Static(mesh) => Ok(Py::new(
                py,
                StaticLayout {
                    index_size_bytes: mesh.payload().index_stride,
                    section_vertex_ranges: mesh
                        .sections()
                        .iter()
                        .map(|section| SectionVertexRange {
                            min_vertex_index: section.min_vertex_index,
                            max_vertex_index: section.max_vertex_index,
                        })
                        .collect(),
                },
            )?
            .into_any()),
            MeshData::Skeletal(mesh) | MeshData::Gestalt { mesh, .. } => {
                let fixture = mesh.mesh();
                let mut weighted_bones = fixture.active_bones.clone();
                weighted_bones.sort_unstable();
                weighted_bones.dedup();
                Ok(Py::new(
                    py,
                    SkeletalLayout {
                        index_size_bytes: fixture.skin.index_stride,
                        weighted_bones,
                        chunks: fixture
                            .skin
                            .chunks
                            .iter()
                            .map(|chunk| SkeletalChunkInfo {
                                base_vertex: chunk.base_vertex,
                                vertex_count: chunk.vertices,
                                first_index: chunk.first_index,
                                triangle_count: chunk.triangles,
                                palette: chunk.palette.clone(),
                                max_bone_influences: chunk.max_bone_influences,
                            })
                            .collect(),
                    },
                )?
                .into_any())
            }
        }
    }

    /// Returns ordinary source ranges with source-section ordinals, not template slots.
    /// Bound Gestalt payloads expose final ranges through parts instead.
    #[getter]
    fn source_fragments(&self) -> Vec<SourceFragmentRange> {
        self.info.source_fragments.clone()
    }

    /// Returns final template-bound Gestalt ranges, or an empty list for ordinary payloads.
    #[getter]
    fn parts(&self) -> Vec<FragmentRange> {
        match &self.data {
            MeshData::Gestalt { parts, .. } => parts.clone(),
            _ => Vec::new(),
        }
    }

    /// Installs fresh storage into an empty exact-class destination without rollback.
    pub fn populate_new(&self, path: &str) -> PyResult<MeshOutcome> {
        let materials: Vec<_> = self.materials.iter().map(String::as_str).collect();
        let path = self.data.populate_new(path, &materials).map_err(error)?;
        Ok(MeshOutcome {
            path,
            warnings: self.info.warnings.clone(),
        })
    }

    /// Replaces a Gestalt master through the explicit paired-definition operation.
    pub fn load_into_gestalt(&self, path: &str) -> PyResult<MeshOutcome> {
        let MeshData::Gestalt { mesh, .. } = &self.data else {
            return Err(error(
                "Gestalt replacement requires a Gestalt prepared payload",
            ));
        };
        let materials = self
            .materials
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        Ok(MeshOutcome {
            path: mesh.replace_gestalt(path, &materials).map_err(error)?,
            warnings: self.info.warnings.clone(),
        })
    }

    /// Updates existing fragment identities and rebuilds detached component data from saved selections.
    /// The path identifies the loaded Gestalt definition, not its master mesh.
    #[pyo3(signature = (path, *, preserve_materials=true))]
    pub fn update_gestalt(&self, path: &str, preserve_materials: bool) -> PyResult<MeshOutcome> {
        let MeshData::Gestalt { mesh, parts } = &self.data else {
            return Err(error("Gestalt update requires a Gestalt prepared payload"));
        };
        let parts: Vec<_> = parts
            .iter()
            .map(|part| mesh_runtime::skeletal::GestaltPartRange {
                name: part.name.clone(),
                material_slot: part.material_slot,
                first_index: part.first_index,
                triangles: part.triangle_count,
            })
            .collect();
        let materials: Vec<_> = self.materials.iter().map(String::as_str).collect();
        Ok(MeshOutcome {
            path: mesh
                .update_gestalt(path, &parts, &materials, preserve_materials)
                .map_err(error)?,
            warnings: self.info.warnings.clone(),
        })
    }

    /// Appends to current native master geometry without changing existing ranges or slots.
    pub fn append_gestalt(&self, path: &str) -> PyResult<GestaltAppendOutcome> {
        let MeshData::Gestalt { mesh, parts } = &self.data else {
            return Err(error("Gestalt append requires a Gestalt prepared payload"));
        };
        let materials = self
            .materials
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let (path, first_index, material_offset) =
            mesh.append_gestalt(path, &materials).map_err(error)?;
        let mut parts = parts.clone();
        for part in &mut parts {
            part.first_index += first_index;
            part.material_slot += material_offset;
        }
        Ok(GestaltAppendOutcome {
            path,
            parts,
            warnings: self.info.warnings.clone(),
        })
    }

    /// Replaces library-owned geometry while retaining destination identity.
    pub fn reload(&self, path: &str) -> PyResult<MeshOutcome> {
        self.replace(path, ReplacementPolicy::LibraryOwned)
    }

    /// Replaces existing geometry under the skeleton and collision restrictions.
    pub fn load_into(&self, path: &str) -> PyResult<MeshOutcome> {
        self.replace(path, ReplacementPolicy::ExistingAsset)
    }
}

/// Validated source animation samples without destination UObject ownership.
#[gen_stub_pyclass]
#[pyclass(frozen, module = "meshloader._meshloader")]
pub struct PreparedAnimationSet {
    data: mesh_runtime::animation::PreparedAnimations,
    source: SourceSelection,
    requested_sample_rate: f32,
    clips: Vec<AnimationClipInfo>,
    warnings: Vec<String>,
}

impl PreparedAnimationSet {
    pub(crate) fn new(
        data: mesh_runtime::animation::PreparedAnimations,
        source: SourceSelection,
        requested_sample_rate: f32,
        clips: Vec<AnimationClipInfo>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            data,
            source,
            requested_sample_rate,
            clips,
            warnings,
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl PreparedAnimationSet {
    /// Returns ordered clip descriptors without copying reference bones or track samples.
    #[getter]
    fn clips(&self) -> Vec<AnimationClipInfo> {
        self.clips.clone()
    }

    /// Returns nonfatal preparation diagnostics.
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }

    /// Returns source-order reference metadata without exposing sampled tracks.
    #[getter]
    fn metadata(&self) -> AnimationInspection {
        AnimationInspection {
            source: self.source.clone(),
            requested_sample_rate: self.requested_sample_rate,
            bones: self
                .data
                .reference_skeleton()
                .iter()
                .map(ReferenceBone::from)
                .collect(),
            clips: self.clips.clone(),
            warnings: self.warnings.clone(),
        }
    }

    /// Checks target bone counts, names and direct parents before object construction.
    /// Reference poses may differ without correction of sampled local poses.
    pub fn check_mesh(&self, mesh: &str) -> PyResult<()> {
        self.data.check_mesh(mesh).map_err(error)
    }

    /// Populates an empty AnimSet and its empty children in prepared document order.
    pub fn populate_new(
        &self,
        path: &str,
        sequences: Vec<String>,
        mesh: &str,
    ) -> PyResult<AnimationOutcome> {
        let sequences: Vec<_> = sequences.iter().map(String::as_str).collect();
        let path = self
            .data
            .populate_new(path, &sequences, mesh)
            .map_err(error)?;
        Ok(AnimationOutcome {
            path,
            warnings: self.warnings.clone(),
        })
    }

    /// Replaces one existing sequence without managing live playback users.
    pub fn load_into(&self, path: &str, mesh: &str) -> PyResult<AnimationOutcome> {
        let path = self.data.load_into(path, mesh).map_err(error)?;
        Ok(AnimationOutcome {
            path,
            warnings: self.warnings.clone(),
        })
    }
}
