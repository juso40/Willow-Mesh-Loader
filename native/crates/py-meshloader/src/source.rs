//! Reusable parsed-source handles and selected preparation entry points.

use crate::{
    catalog::{SourceCatalog, SourceIdentities, SourceIdentity},
    error,
    inspection::SourceSelection,
    materials::MaterialMapping,
    prepared::{PreparedAnimationSet, PreparedMesh},
};
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::sync::OnceLock;

/// Shared glTF/GLB document and lazily loaded, validated source buffers.
#[gen_stub_pyclass]
#[pyclass(frozen, module = "meshloader._meshloader")]
pub struct Source {
    source: mesh_import::Source,
    identities: SourceIdentities,
    catalog: OnceLock<SourceCatalog>,
}

impl Source {
    fn mesh_selection(
        &self,
        node: usize,
        scene: usize,
        requested: &str,
    ) -> PyResult<(mesh_import::Source, mesh_import::kind::Kind)> {
        use mesh_import::kind::Kind;
        let source = self.source.select_mesh(node, scene).map_err(error)?;
        let kind = source.kind().map_err(error)?;
        match (requested, kind) {
            ("auto", _) | ("static", Kind::Static) | ("skeletal", Kind::Skeletal) => {
                Ok((source, kind))
            }
            ("static" | "skeletal", _) => Err(error(
                "selected node does not match the requested mesh kind",
            )),
            _ => Err(error("kind must be auto, static or skeletal")),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn selection(
        &self,
        node: Option<usize>,
        skin: Option<usize>,
        scene: usize,
        scale: f32,
        bone_frames: Option<&str>,
        template_mesh: Option<String>,
    ) -> SourceSelection {
        SourceSelection {
            path: self.source.path().to_string_lossy().into_owned(),
            scene: self.identities.scenes[scene].clone(),
            node: node
                .and_then(|id| self.identities.meshes.iter().find(|mesh| mesh.id == id))
                .map(|mesh| SourceIdentity {
                    id: mesh.id,
                    name: mesh.name.clone(),
                }),
            skin: skin.and_then(|id| self.identities.skins.get(id)).cloned(),
            scale,
            bone_frames: bone_frames.map(str::to_owned),
            template_mesh,
        }
    }

    fn mesh_skin(&self, node: usize) -> Option<usize> {
        self.identities
            .meshes
            .iter()
            .find(|mesh| mesh.id == node)
            .and_then(|mesh| mesh.skin_id)
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Source {
    /// Parses one source document without constructing engine objects.
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let source = mesh_import::Source::open(std::path::Path::new(path)).map_err(error)?;
        let identities = source.identities().into();
        Ok(Self {
            source,
            identities,
            catalog: OnceLock::new(),
        })
    }

    /// Returns descriptive source-local identities without decoding asset payloads.
    fn catalog(&self) -> SourceCatalog {
        self.catalog
            .get_or_init(|| self.source.catalog().into())
            .clone()
    }

    /// Returns lightweight identities without scene traversal or channel discovery.
    #[getter]
    fn identities(&self) -> SourceIdentities {
        self.identities.clone()
    }

    /// Returns selected source material slots in first-use order without decoding buffers.
    #[pyo3(signature = (*, node, scene, kind="auto"))]
    fn material_slots(
        &self,
        node: usize,
        scene: usize,
        kind: &str,
    ) -> PyResult<Vec<Option<usize>>> {
        self.mesh_selection(node, scene, kind)?
            .0
            .material_slots()
            .map_err(error)
    }

    /// Prepares a selected mesh node in its selected scene.
    /// `umodel` defaults to PSK-workflow bone frames. False preserves glTF frames.
    #[pyo3(signature = (*, node, scene, kind="auto", material=crate::DEFAULT_MATERIAL_PATH, scale=mesh_import::DEFAULT_SCALE, materials=None, umodel=true))]
    #[allow(clippy::too_many_arguments)]
    fn prepare_mesh(
        &self,
        node: usize,
        scene: usize,
        kind: &str,
        material: &str,
        scale: f32,
        materials: Option<MaterialMapping>,
        umodel: bool,
    ) -> PyResult<PreparedMesh> {
        let (source, kind) = self.mesh_selection(node, scene, kind)?;
        let source = source.with_umodel(umodel);
        use mesh_import::kind::Kind;
        match kind {
            Kind::Static => crate::static_mesh::prepare_source(
                &source,
                material,
                scale,
                materials.as_ref(),
                self.selection(Some(node), None, scene, scale, None, None),
            ),
            Kind::Skeletal => crate::skeletal::prepare_source(
                &source,
                material,
                scale,
                materials.as_ref(),
                self.selection(
                    Some(node),
                    self.mesh_skin(node),
                    scene,
                    scale,
                    Some(if umodel { "psk" } else { "gltf" }),
                    None,
                ),
            ),
        }
    }

    /// Prepares selected clips for a selected skin and scene without decoding geometry.
    /// `umodel` defaults to PSK-workflow bind and sampled-pose conversion.
    #[pyo3(signature = (*, skin, clips, scene, scale=mesh_import::DEFAULT_SCALE, sample_rate=30.0, umodel=true))]
    #[allow(clippy::too_many_arguments)]
    fn prepare_animations(
        &self,
        skin: usize,
        clips: Vec<usize>,
        scene: usize,
        scale: f32,
        sample_rate: f32,
        umodel: bool,
    ) -> PyResult<PreparedAnimationSet> {
        let source = self
            .source
            .select_skin(skin, scene)
            .map_err(error)?
            .with_umodel(umodel);
        crate::animation::prepare_source(
            &source,
            scale,
            sample_rate,
            &clips,
            self.selection(
                None,
                Some(skin),
                scene,
                scale,
                Some(if umodel { "psk" } else { "gltf" }),
                None,
            ),
        )
    }

    /// Prepares selected Gestalt nodes, using authored node names for untagged fragments.
    /// Independent sources retain their skeleton when no template is supplied.
    #[pyo3(signature = (*, nodes, scene, template=None, material=crate::DEFAULT_MATERIAL_PATH, materials=None, scale=mesh_import::DEFAULT_SCALE, umodel=true))]
    #[allow(clippy::too_many_arguments)]
    fn prepare_gestalt_parts(
        &self,
        nodes: Vec<usize>,
        scene: usize,
        template: Option<&str>,
        material: &str,
        materials: Option<MaterialMapping>,
        scale: f32,
        umodel: bool,
    ) -> PyResult<PreparedMesh> {
        if nodes.is_empty()
            || nodes
                .iter()
                .enumerate()
                .any(|(i, node)| nodes[..i].contains(node))
        {
            return Err(error("Gestalt nodes must be nonempty and distinct"));
        }
        let mut sources = Vec::new();
        for &node in &nodes {
            let identity = self
                .identities
                .meshes
                .iter()
                .find(|mesh| mesh.id == node)
                .ok_or_else(|| error("unknown Gestalt mesh node"))?;
            let name = identity
                .name
                .clone()
                .ok_or_else(|| error("Gestalt mesh nodes require authored names"))?;
            sources.push((
                self.source
                    .select_mesh(node, scene)
                    .map_err(error)?
                    .with_umodel(umodel),
                name,
            ));
        }
        let skin = self
            .mesh_skin(nodes[0])
            .filter(|&skin| nodes.iter().all(|&node| self.mesh_skin(node) == Some(skin)));
        crate::gestalt::prepare_parts(
            sources,
            material,
            materials.as_ref(),
            template,
            scale,
            self.selection(
                if nodes.len() == 1 {
                    Some(nodes[0])
                } else {
                    None
                },
                skin,
                scene,
                scale,
                Some(if umodel { "psk" } else { "gltf" }),
                template.map(str::to_owned),
            ),
        )
    }

    /// Prepares and template-binds a selected fully tagged Gestalt mesh node.
    /// `umodel` defaults to PSK-workflow bone frames. False preserves glTF frames.
    #[pyo3(signature = (template, fragment_names, materials, material_slots, uv_channels, *, node, scene, scale=mesh_import::DEFAULT_SCALE, umodel=true))]
    #[allow(clippy::too_many_arguments)]
    fn prepare_gestalt(
        &self,
        template: &str,
        fragment_names: Vec<String>,
        materials: Vec<String>,
        material_slots: Vec<usize>,
        uv_channels: usize,
        node: usize,
        scene: usize,
        scale: f32,
        umodel: bool,
    ) -> PyResult<PreparedMesh> {
        let source = self
            .source
            .select_mesh(node, scene)
            .map_err(error)?
            .with_umodel(umodel);
        crate::gestalt::prepare(
            &source,
            template,
            fragment_names,
            materials,
            material_slots,
            uv_channels,
            scale,
            self.selection(
                Some(node),
                self.mesh_skin(node),
                scene,
                scale,
                Some(if umodel { "psk" } else { "gltf" }),
                Some(template.to_owned()),
            ),
        )
    }
}
