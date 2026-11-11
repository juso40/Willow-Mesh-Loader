//! Shared source documents, cached buffers and explicit asset views.

use crate::{Error, Geometry, Result, document::bounded_read, kind::Kind, skin::SkinnedGeometry};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

struct SourceData {
    asset: gltf::Gltf,
    path: PathBuf,
    root: PathBuf,
    buffers: Vec<OnceLock<Result<Vec<u8>>>>,
    extensions: crate::document::Extensions,
}

#[derive(Clone, Copy)]
enum Selection {
    Catalog,
    Mesh {
        node: usize,
        skin: Option<usize>,
        scene: usize,
    },
    Skin {
        skin: usize,
        scene: usize,
    },
}

/// Parsed glTF/GLB source view sharing its document and lazy per-buffer read cache.
#[derive(Clone)]
pub struct Source {
    data: Arc<SourceData>,
    selection: Selection,
    umodel: bool,
}

impl Source {
    /// Reads one source document without loading its external buffers.
    ///
    /// Bone frames default to the PSK/PSA Y-up export workflow. Authored glTF
    /// frames use `with_umodel(false)` before geometry or animation decoding.
    pub fn open(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .map_err(|e| Error(format!("asset path: {e}")))?;
        let root = path
            .parent()
            .ok_or_else(|| crate::fail("asset has no parent directory"))?
            .to_path_buf();
        let bytes = bounded_read(&path, crate::MAX_FILE)?;
        let (asset, extensions) = crate::document::parse(&bytes)?;
        let buffers = (0..asset.buffers().len())
            .map(|_| OnceLock::new())
            .collect();
        Ok(Self {
            data: Arc::new(SourceData {
                asset,
                path,
                root,
                buffers,
                extensions,
            }),
            selection: Selection::Catalog,
            umodel: true,
        })
    }

    /// Returns the canonical source path recorded when the document was opened.
    pub fn path(&self) -> &Path {
        &self.data.path
    }

    /// Returns a view using PSK/PSA Y-up export bone frames when true.
    ///
    /// The default is true. False preserves authored glTF bone frames. Geometry
    /// coordinates and static meshes retain the same conversion in both modes.
    pub fn with_umodel(&self, umodel: bool) -> Self {
        Self {
            data: Arc::clone(&self.data),
            selection: self.selection,
            umodel,
        }
    }

    pub(crate) fn bone_basis(&self) -> Option<crate::math::Matrix> {
        self.umodel.then_some(crate::coordinates::PSK_BONE_BASIS)
    }

    /// Returns a source view selecting one mesh node and the scene containing it.
    pub fn select_mesh(&self, node: usize, scene: usize) -> Result<Self> {
        let selected = self
            .document()
            .nodes()
            .nth(node)
            .filter(|node| node.mesh().is_some())
            .ok_or_else(|| crate::fail("selected node does not reference a mesh"))?;
        let view = Self {
            data: Arc::clone(&self.data),
            selection: Selection::Mesh {
                node,
                skin: selected.skin().map(|skin| skin.index()),
                scene,
            },
            umodel: self.umodel,
        };
        view.scene()?;
        Ok(view)
    }

    /// Returns a source view selecting one skin and the scene containing its joints.
    pub fn select_skin(&self, skin: usize, scene: usize) -> Result<Self> {
        let view = Self {
            data: Arc::clone(&self.data),
            selection: Selection::Skin { skin, scene },
            umodel: self.umodel,
        };
        view.skin()?;
        view.scene()?;
        Ok(view)
    }

    pub(crate) fn document(&self) -> &gltf::Document {
        &self.data.asset.document
    }

    pub(crate) fn check_extensions(&self, path: &str) -> Result<()> {
        self.data.extensions.check(path)
    }

    pub(crate) fn buffers<'a>(
        &'a self,
        accessors: impl IntoIterator<Item = gltf::Accessor<'a>>,
    ) -> Result<crate::document::Buffers<'a>> {
        let mut data = vec![None; self.data.buffers.len()];
        for accessor in accessors {
            self.check_extensions(&format!("/accessors/{}", accessor.index()))?;
            if accessor.sparse().is_some() {
                return Err(crate::fail("sparse accessors are not supported"));
            }
            let view = accessor
                .view()
                .ok_or_else(|| crate::fail("accessor has no buffer view"))?;
            self.check_extensions(&format!("/bufferViews/{}", view.index()))?;
            let buffer = view.buffer();
            self.check_extensions(&format!("/buffers/{}", buffer.index()))?;
            let bytes = match self.data.buffers[buffer.index()].get_or_init(|| {
                crate::document::buffer(&self.data.asset, &self.data.root, buffer.clone())
            }) {
                Ok(bytes) => bytes.as_slice(),
                Err(error) => return Err(Error(error.0.clone())),
            };
            crate::document::validate_accessor(&accessor, bytes)?;
            data[buffer.index()] = Some(bytes);
        }
        Ok(crate::document::Buffers { data })
    }

    pub(crate) fn mesh_buffers(&self) -> Result<crate::document::Buffers<'_>> {
        use gltf::mesh::Semantic;
        let node = self.node()?;
        self.check_extensions(&format!("/nodes/{}", node.index()))?;
        let mesh = node
            .mesh()
            .ok_or_else(|| crate::fail("selected node has no mesh"))?;
        self.check_extensions(&format!("/meshes/{}", mesh.index()))?;
        let skinned = node.skin().is_some();
        let mut accessors = Vec::new();
        for primitive in mesh.primitives() {
            self.check_extensions(&format!(
                "/meshes/{}/primitives/{}",
                mesh.index(),
                primitive.index()
            ))?;
            accessors.extend(primitive.attributes().filter_map(|(semantic, accessor)| {
                let consumed = match semantic {
                    Semantic::TexCoords(channel) => channel == 0 || (skinned && channel <= 3),
                    _ => matches!(
                        semantic,
                        Semantic::Positions
                            | Semantic::Normals
                            | Semantic::Tangents
                            | Semantic::Colors(0)
                            | Semantic::Joints(0..=1)
                            | Semantic::Weights(0..=1)
                    ),
                };
                consumed.then_some(accessor)
            }));
            accessors.extend(primitive.indices());
        }
        if let Some(skin) = node.skin() {
            self.check_extensions(&format!("/skins/{}", skin.index()))?;
            accessors.extend(skin.inverse_bind_matrices());
        }
        self.buffers(accessors)
    }

    pub(crate) fn scene(&self) -> Result<gltf::Scene<'_>> {
        let index = match self.selection {
            Selection::Mesh { scene, .. } | Selection::Skin { scene, .. } => scene,
            Selection::Catalog => return Err(crate::fail("a scene selection is required")),
        };
        self.check_extensions(&format!("/scenes/{index}"))?;
        self.document()
            .scenes()
            .nth(index)
            .ok_or_else(|| crate::fail("selected scene does not exist"))
    }

    pub(crate) fn node(&self) -> Result<gltf::Node<'_>> {
        let Selection::Mesh { node, .. } = self.selection else {
            return Err(crate::fail("a mesh-node selection is required"));
        };
        self.document()
            .nodes()
            .nth(node)
            .ok_or_else(|| crate::fail("selected mesh node does not exist"))
    }

    pub(crate) fn skin(&self) -> Result<gltf::Skin<'_>> {
        let index = match self.selection {
            Selection::Skin { skin, .. }
            | Selection::Mesh {
                skin: Some(skin), ..
            } => skin,
            _ => return Err(crate::fail("a skin selection is required")),
        };
        self.check_extensions(&format!("/skins/{index}"))?;
        self.document()
            .skins()
            .nth(index)
            .ok_or_else(|| crate::fail("selected skin does not exist"))
    }

    /// Samples selected source clip IDs for this view's skin in document order.
    pub fn animation_clips(
        &self,
        scale: f32,
        sample_rate: f32,
        clips: &[usize],
    ) -> Result<crate::animation::AnimationSet> {
        crate::animation::from_source(self, scale, sample_rate, clips)
    }

    /// Classifies the selected mesh node by its skin reference.
    pub fn kind(&self) -> Result<Kind> {
        Ok(if self.node()?.skin().is_some() {
            Kind::Skeletal
        } else {
            Kind::Static
        })
    }

    /// Decodes the selected static mesh node with its scene transforms.
    pub fn static_geometry(&self, scale: f32) -> Result<Geometry> {
        crate::static_mesh::from_source(self, scale)
    }

    /// Decodes the selected skinned mesh node and its bind-derived skeleton.
    pub fn skeletal_geometry(&self, scale: f32) -> Result<SkinnedGeometry> {
        crate::skin::from_source(self, scale)
    }
}
