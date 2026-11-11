//! Skeletal geometry and bind-derived reference poses without engine access.

use crate::{Result, Source, fail, warn};

mod bind;
mod skeleton;
mod types;
mod vertices;
pub(crate) use bind::*;
use skeleton::SceneTransforms;
pub use types::{Fragment, ImportBone, SkinnedGeometry, SkinnedVertex};
use vertices::append_skinned;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportMode {
    Geometry,
    Skeleton,
}

pub(crate) struct ImportedSkeleton {
    pub bones: Vec<ImportBone>,
    pub warnings: Vec<String>,
}

pub(crate) fn from_source(source: &Source, scale: f32) -> Result<SkinnedGeometry> {
    decode(source, scale, &source.mesh_buffers()?, ImportMode::Geometry)
}

pub(crate) fn reference_skeleton(
    source: &Source,
    scale: f32,
    buffers: &crate::document::Buffers<'_>,
) -> Result<ImportedSkeleton> {
    let decoded = decode(source, scale, buffers, ImportMode::Skeleton)?;
    Ok(ImportedSkeleton {
        bones: decoded.bones,
        warnings: decoded.warnings,
    })
}

fn decode(
    source: &Source,
    scale: f32,
    buffers: &crate::document::Buffers<'_>,
    mode: ImportMode,
) -> Result<SkinnedGeometry> {
    crate::policy::validate_scale(scale)?;
    let skin = source.skin()?;
    let scene = source.scene()?;
    let mut geometry = SkinnedGeometry {
        vertices: Vec::new(),
        indices: Vec::new(),
        bones: Vec::new(),
        scene: scene.index(),
        primitives: 0,
        sections: Vec::new(),
        fragments: Vec::new(),
        tex_coords: 1,
        warnings: Vec::new(),
    };
    let mesh_node = if mode == ImportMode::Geometry {
        Some(source.node()?.index())
    } else {
        None
    };
    let transforms = SceneTransforms::read(source, mesh_node)?;
    let mesh = match mode {
        ImportMode::Geometry => {
            let node = source.node()?;
            if node.skin().map(|skin| skin.index()) != Some(skin.index()) {
                return Err(fail("selected mesh node does not use the selected skin"));
            }
            if node
                .transform()
                .matrix()
                .map(|column| column.map(f64::from))
                != skeleton::IDENTITY
            {
                warn(
                    &mut geometry.warnings,
                    "skinned mesh node transform is ignored per glTF skinning semantics",
                );
            }
            node.mesh()
        }
        ImportMode::Skeleton => None,
    };
    let skeleton = skeleton::decode(
        skin,
        &transforms,
        buffers,
        scale,
        source.bone_basis(),
        &mut geometry.warnings,
    )?;
    geometry.bones = skeleton.bones;
    if mode == ImportMode::Skeleton {
        return Ok(geometry);
    }
    let mesh = mesh.ok_or_else(|| fail("selected skinned node has no mesh"))?;
    for primitive in mesh.primitives() {
        append_skinned(
            primitive,
            &skeleton.conversion,
            buffers,
            &skeleton.joint_to_bone,
            &mut geometry,
        )?;
    }
    finish_geometry(geometry)
}

fn finish_geometry(mut geometry: SkinnedGeometry) -> Result<SkinnedGeometry> {
    if geometry.indices.is_empty() {
        return Err(fail("skinned mesh contains no nonempty triangle geometry"));
    }
    crate::indexing::deduplicate(&mut geometry.vertices, &mut geometry.indices, |vertex| {
        (
            crate::indexing::vertex_key(&vertex.vertex),
            vertex.joints,
            vertex.weights.map(f32::to_bits),
        )
    })?;
    let (indices, sections, offsets) =
        crate::sections::group_with_offsets(&geometry.indices, &geometry.sections)?;
    for fragment in &mut geometry.fragments {
        let old = &geometry.sections[fragment.material_index];
        fragment.first_index = offsets[fragment.material_index];
        fragment.material_index = sections
            .iter()
            .position(|section| section.source_material == old.source_material)
            .ok_or_else(|| fail("missing fragment material"))?;
    }
    geometry
        .fragments
        .sort_by_key(|fragment| fragment.first_index);
    geometry.indices = indices;
    geometry.sections = sections;
    Ok(geometry)
}
