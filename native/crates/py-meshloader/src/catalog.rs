//! Owned read-only source identities and discovery records.

use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;

/// Source-local identity with an optional authored label.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceIdentity {
    /// Zero-based document index, not a persistent asset identifier.
    pub id: usize,
    /// Authored name, which may be missing or duplicated.
    pub name: Option<String>,
}

/// Lightweight selectable mesh-node identity.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceMeshIdentity {
    /// Document node index, not a mesh-definition index.
    pub id: usize,
    /// Optional authored node name.
    pub name: Option<String>,
    /// Associated source skin, or none for static geometry.
    pub skin_id: Option<usize>,
}

/// Lightweight selection categories without hierarchy or channel discovery.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceIdentities {
    /// Scene identities in document order.
    pub scenes: Vec<SourceIdentity>,
    /// Mesh-node identities in document order.
    pub meshes: Vec<SourceMeshIdentity>,
    /// Skin identities in document order.
    pub skins: Vec<SourceIdentity>,
    /// Animation identities in document order.
    pub animations: Vec<SourceIdentity>,
}

/// Descriptive scene identity and source root nodes.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceScene {
    /// Document scene index.
    pub id: usize,
    /// Optional authored scene name.
    pub name: Option<String>,
    /// Root glTF node IDs, not reference-bone ordinals.
    pub roots: Vec<usize>,
}

/// Descriptive mesh-node identity and shared geometry associations.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceMesh {
    /// Document node index.
    pub id: usize,
    /// Optional selectable node name.
    pub name: Option<String>,
    /// Shared glTF geometry-definition ID.
    pub mesh_id: usize,
    /// Optional geometry label, not a node selector.
    pub mesh_name: Option<String>,
    /// Associated source skin, if present.
    pub skin_id: Option<usize>,
    /// Scenes containing this node, not ambiguity-breaking selectors.
    pub scene_ids: Vec<usize>,
    /// Source material IDs in primitive order, not engine slots.
    pub primitive_material_ids: Vec<Option<usize>>,
}

/// Descriptive skin identities without inverse-bind admission.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceSkin {
    /// Document skin index.
    pub id: usize,
    /// Optional authored skin name.
    pub name: Option<String>,
    /// Source node identities in authored joint order.
    pub joints: Vec<SourceIdentity>,
    /// Accessor presence only, not a validation guarantee.
    pub has_inverse_binds: bool,
}

/// Unsampled animation identity and candidate associations.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceAnimation {
    /// Document animation index.
    pub id: usize,
    /// Optional authored animation name.
    pub name: Option<String>,
    /// Distinct targeted glTF node IDs.
    pub target_nodes: Vec<usize>,
    /// Candidate skins, not proof of playback compatibility.
    pub skin_ids: Vec<usize>,
}

/// Detailed source discovery without geometry or animation decoding.
#[gen_stub_pyclass]
#[pyclass(
    frozen,
    get_all,
    skip_from_py_object,
    module = "meshloader._meshloader"
)]
#[derive(Clone)]
pub struct SourceCatalog {
    /// Authored default scene, never an implicit selection.
    pub default_scene: Option<usize>,
    /// Descriptive scenes.
    pub scenes: Vec<SourceScene>,
    /// Descriptive mesh instances.
    pub meshes: Vec<SourceMesh>,
    /// Descriptive reference skeletons.
    pub skins: Vec<SourceSkin>,
    /// Descriptive unsampled clips.
    pub animations: Vec<SourceAnimation>,
}

impl From<mesh_import::catalog::Identity> for SourceIdentity {
    fn from(value: mesh_import::catalog::Identity) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl From<mesh_import::catalog::Identities> for SourceIdentities {
    fn from(value: mesh_import::catalog::Identities) -> Self {
        Self {
            scenes: value.scenes.into_iter().map(Into::into).collect(),
            meshes: value
                .meshes
                .into_iter()
                .map(|mesh| SourceMeshIdentity {
                    id: mesh.identity.id,
                    name: mesh.identity.name,
                    skin_id: mesh.skin_id,
                })
                .collect(),
            skins: value.skins.into_iter().map(Into::into).collect(),
            animations: value.animations.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<mesh_import::catalog::Catalog> for SourceCatalog {
    fn from(value: mesh_import::catalog::Catalog) -> Self {
        Self {
            default_scene: value.default_scene,
            scenes: value
                .scenes
                .into_iter()
                .map(|scene| SourceScene {
                    id: scene.identity.id,
                    name: scene.identity.name,
                    roots: scene.roots,
                })
                .collect(),
            meshes: value
                .meshes
                .into_iter()
                .map(|mesh| SourceMesh {
                    id: mesh.identity.id,
                    name: mesh.identity.name,
                    mesh_id: mesh.mesh_id,
                    mesh_name: mesh.mesh_name,
                    skin_id: mesh.skin_id,
                    scene_ids: mesh.scene_ids,
                    primitive_material_ids: mesh.materials,
                })
                .collect(),
            skins: value
                .skins
                .into_iter()
                .map(|skin| SourceSkin {
                    id: skin.identity.id,
                    name: skin.identity.name,
                    joints: skin
                        .joints
                        .into_iter()
                        .zip(skin.joint_names)
                        .map(|(id, name)| SourceIdentity { id, name })
                        .collect(),
                    has_inverse_binds: skin.has_inverse_binds,
                })
                .collect(),
            animations: value
                .animations
                .into_iter()
                .map(|animation| SourceAnimation {
                    id: animation.identity.id,
                    name: animation.identity.name,
                    target_nodes: animation.target_nodes,
                    skin_ids: animation.skin_ids,
                })
                .collect(),
        }
    }
}
