//! Lightweight source metadata without geometry or animation sampling.

use crate::Source;
use serde::Serialize;

/// Stable source-local identity and optional authored name.
#[derive(Debug, Clone, Serialize)]
pub struct Identity {
    /// Zero-based document index.
    pub id: usize,
    /// Authored label, which may be absent or duplicated.
    pub name: Option<String>,
}

/// One scene and its root nodes.
#[derive(Debug, Serialize)]
pub struct SceneInfo {
    /// Document identity.
    #[serde(flatten)]
    pub identity: Identity,
    /// Root node IDs in authored order.
    pub roots: Vec<usize>,
}

/// One mesh instance, including the scene memberships used for transform evaluation.
#[derive(Debug, Serialize)]
pub struct MeshInfo {
    /// Mesh-node identity rather than the shared geometry definition's identity.
    #[serde(flatten)]
    pub identity: Identity,
    /// Shared glTF mesh definition ID.
    pub mesh_id: usize,
    /// Optional geometry-definition label.
    pub mesh_name: Option<String>,
    /// Skin ID, or no skin for a static instance.
    pub skin_id: Option<usize>,
    /// Scenes containing this mesh node.
    pub scene_ids: Vec<usize>,
    /// Source material IDs in primitive order, including unassigned materials.
    pub materials: Vec<Option<usize>>,
}

/// One skin and the node IDs defining its skeleton.
#[derive(Debug, Serialize)]
pub struct SkinInfo {
    /// Document identity.
    #[serde(flatten)]
    pub identity: Identity,
    /// Joint node IDs in authored skin order.
    pub joints: Vec<usize>,
    /// Authored joint labels in the same order as the joint IDs.
    pub joint_names: Vec<Option<String>>,
    /// Presence of the inverse-bind accessor required by preparation.
    pub has_inverse_binds: bool,
}

/// One unsampled animation and its possible skeleton associations.
#[derive(Debug, Serialize)]
pub struct AnimationInfo {
    /// Document identity.
    #[serde(flatten)]
    pub identity: Identity,
    /// Distinct targeted node IDs.
    pub target_nodes: Vec<usize>,
    /// Skins whose joints or ancestors receive at least one channel.
    /// Membership is not a compatibility or channel-validation guarantee.
    pub skin_ids: Vec<usize>,
}

/// Document catalog available before buffer loading or full asset validation.
#[derive(Debug, Serialize)]
pub struct Catalog {
    /// Authored default scene, when present.
    pub default_scene: Option<usize>,
    /// Available scenes.
    pub scenes: Vec<SceneInfo>,
    /// Mesh instances, selected by node ID.
    pub meshes: Vec<MeshInfo>,
    /// Available reference skeletons.
    pub skins: Vec<SkinInfo>,
    /// Available animation clips, selected by animation ID.
    pub animations: Vec<AnimationInfo>,
}

fn identity(id: usize, name: Option<&str>) -> Identity {
    Identity {
        id,
        name: name.map(str::to_owned),
    }
}

/// Lightweight mesh-node identity used without detailed catalog discovery.
#[derive(Debug)]
pub struct MeshIdentity {
    /// Node identity rather than shared geometry identity.
    pub identity: Identity,
    /// Associated skin, or none for static geometry.
    pub skin_id: Option<usize>,
}

/// Selector identities without scene traversal or animation association analysis.
#[derive(Debug)]
pub struct Identities {
    /// Scene identities in document order.
    pub scenes: Vec<Identity>,
    /// Mesh-node identities in document order.
    pub meshes: Vec<MeshIdentity>,
    /// Skin identities in document order.
    pub skins: Vec<Identity>,
    /// Animation identities in document order.
    pub animations: Vec<Identity>,
}

impl Source {
    /// Enumerates selector identities without buffer loading or detailed discovery.
    pub fn identities(&self) -> Identities {
        let doc = self.document();
        Identities {
            scenes: doc
                .scenes()
                .map(|item| identity(item.index(), item.name()))
                .collect(),
            meshes: doc
                .nodes()
                .filter(|node| node.mesh().is_some())
                .map(|node| MeshIdentity {
                    identity: identity(node.index(), node.name()),
                    skin_id: node.skin().map(|skin| skin.index()),
                })
                .collect(),
            skins: doc
                .skins()
                .map(|item| identity(item.index(), item.name()))
                .collect(),
            animations: doc
                .animations()
                .map(|item| identity(item.index(), item.name()))
                .collect(),
        }
    }

    /// Returns selected mesh material IDs in first-use order without decoding geometry.
    /// These are source slots, not final native draw sections or an admission guarantee.
    pub fn material_slots(&self) -> crate::Result<Vec<Option<usize>>> {
        let node = self.node()?;
        let mesh = node
            .mesh()
            .ok_or_else(|| crate::fail("selected node has no mesh"))?;
        let mut slots = Vec::new();
        for primitive in mesh.primitives() {
            let slot = primitive.material().index();
            if !slots.contains(&slot) {
                slots.push(slot);
            }
        }
        Ok(slots)
    }

    /// Enumerates source identities without loading buffers or decoding geometry.
    /// Catalog entries are descriptive. Preparation performs full validation.
    pub fn catalog(&self) -> Catalog {
        let doc = self.document();
        let mut scene_ids = vec![Vec::new(); doc.nodes().len()];
        for scene in doc.scenes() {
            let mut visited = vec![false; doc.nodes().len()];
            let mut stack: Vec<_> = scene.nodes().collect();
            while let Some(node) = stack.pop() {
                if std::mem::replace(&mut visited[node.index()], true) {
                    continue;
                }
                scene_ids[node.index()].push(scene.index());
                stack.extend(node.children());
            }
        }
        let mut parents = vec![Vec::new(); doc.nodes().len()];
        for node in doc.nodes() {
            for child in node.children() {
                parents[child.index()].push(node.index());
            }
        }
        let relevant: Vec<_> = doc
            .skins()
            .map(|skin| {
                let mut included = vec![false; doc.nodes().len()];
                let mut stack: Vec<_> = skin.joints().map(|joint| joint.index()).collect();
                while let Some(node) = stack.pop() {
                    if std::mem::replace(&mut included[node], true) {
                        continue;
                    }
                    stack.extend(&parents[node]);
                }
                included
            })
            .collect();
        Catalog {
            default_scene: doc.default_scene().map(|scene| scene.index()),
            scenes: doc
                .scenes()
                .map(|scene| SceneInfo {
                    identity: identity(scene.index(), scene.name()),
                    roots: scene.nodes().map(|node| node.index()).collect(),
                })
                .collect(),
            meshes: doc
                .nodes()
                .filter_map(|node| {
                    node.mesh().map(|mesh| MeshInfo {
                        identity: identity(node.index(), node.name()),
                        mesh_id: mesh.index(),
                        mesh_name: mesh.name().map(str::to_owned),
                        skin_id: node.skin().map(|skin| skin.index()),
                        scene_ids: scene_ids[node.index()].clone(),
                        materials: mesh
                            .primitives()
                            .map(|primitive| primitive.material().index())
                            .collect(),
                    })
                })
                .collect(),
            skins: doc
                .skins()
                .map(|skin| SkinInfo {
                    identity: identity(skin.index(), skin.name()),
                    joints: skin.joints().map(|node| node.index()).collect(),
                    joint_names: skin
                        .joints()
                        .map(|node| node.name().map(str::to_owned))
                        .collect(),
                    has_inverse_binds: skin.inverse_bind_matrices().is_some(),
                })
                .collect(),
            animations: doc
                .animations()
                .map(|animation| {
                    let targets: std::collections::BTreeSet<_> = animation
                        .channels()
                        .map(|channel| channel.target().node().index())
                        .collect();
                    AnimationInfo {
                        identity: identity(animation.index(), animation.name()),
                        skin_ids: relevant
                            .iter()
                            .enumerate()
                            .filter_map(|(id, nodes)| {
                                targets.iter().any(|&node| nodes[node]).then_some(id)
                            })
                            .collect(),
                        target_nodes: targets.into_iter().collect(),
                    }
                })
                .collect(),
        }
    }
}
