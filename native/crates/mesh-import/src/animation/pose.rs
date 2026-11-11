//! Scene hierarchy sampling with non-joint ancestors collapsed into local bone tracks.

use super::{BoneTrack, sampling::Channel};
use crate::{
    Result, Source, fail,
    math::*,
    skin::{self, ImportBone},
};
use gltf::animation::Property;

type Trs = ([f32; 3], [f32; 4], [f32; 3]);
const IDENTITY: Matrix = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

pub(super) struct PoseSampler<'a> {
    bones: &'a [ImportBone],
    joint_nodes: Vec<usize>,
    node_parents: Vec<Option<usize>>,
    relevant: Vec<bool>,
    matrix_nodes: Vec<bool>,
    traversal: Vec<usize>,
    locals: Vec<Matrix>,
    trs: Vec<Trs>,
    conversion: Matrix,
    inverse_conversion: Matrix,
    bone_basis: Option<Matrix>,
}

impl<'a> PoseSampler<'a> {
    pub fn new(source: &Source, bones: &'a [ImportBone], scale: f32) -> Result<Self> {
        let doc = source.document();
        let joints: Vec<_> = source.skin()?.joints().collect();
        let joint_nodes: Vec<_> = bones
            .iter()
            .map(|bone| {
                joints
                    .iter()
                    .find(|node| node.name() == Some(bone.name.as_str()))
                    .map(|node| node.index())
                    .ok_or_else(|| fail("animation joint mapping is incomplete"))
            })
            .collect::<Result<_>>()?;
        let hierarchy = crate::hierarchy::Hierarchy::read(source)?;
        let relevant = hierarchy.relevant(joint_nodes.iter().copied())?;
        let traversal = hierarchy
            .order
            .iter()
            .copied()
            .filter(|&node| relevant[node])
            .collect();
        let node_parents = hierarchy.parents;
        let locals = doc
            .nodes()
            .map(|node| {
                if relevant[node.index()] {
                    source.check_extensions(&format!("/nodes/{}", node.index()))?;
                    node_transform(node.transform())
                } else {
                    Ok(IDENTITY)
                }
            })
            .collect::<Result<_>>()?;
        let trs = doc
            .nodes()
            .map(|node| {
                if relevant[node.index()] {
                    node.transform().decomposed()
                } else {
                    ([0.; 3], [0., 0., 0., 1.], [1.; 3])
                }
            })
            .collect();
        let matrix_nodes = doc
            .nodes()
            .map(|node| matches!(node.transform(), gltf::scene::Transform::Matrix { .. }))
            .collect();
        let conversion = crate::coordinates::matrix(f64::from(scale));
        let inverse_conversion = skin::inverse_orthogonal_scaled(conversion)?;
        Ok(Self {
            bones,
            joint_nodes,
            node_parents,
            relevant,
            matrix_nodes,
            traversal,
            locals,
            trs,
            conversion,
            inverse_conversion,
            bone_basis: source.bone_basis(),
        })
    }

    pub fn includes(&self, node: usize) -> bool {
        self.relevant[node]
    }

    pub fn validate_target(&self, node: usize) -> Result<()> {
        if !self.relevant[node] {
            return Err(fail(
                "animation channels must target joints or their ancestors",
            ));
        }
        if self.matrix_nodes[node] {
            return Err(fail("animated nodes must use TRS transforms, not matrices"));
        }
        Ok(())
    }

    pub fn track_count(&self) -> usize {
        self.bones.len()
    }

    pub fn sample(
        &self,
        channels: &[Channel],
        time: f64,
        tracks: &mut [BoneTrack],
    ) -> Result<bool> {
        let mut locals = self.locals.clone();
        let mut trs = std::collections::BTreeMap::new();
        for channel in channels {
            let (translation, rotation, scale) =
                trs.entry(channel.node).or_insert(self.trs[channel.node]);
            let value = channel.sample(time)?.map(|v| v as f32);
            match channel.property {
                Property::Translation => translation.copy_from_slice(&value[..3]),
                Property::Rotation => *rotation = value,
                Property::Scale => scale.copy_from_slice(&value[..3]),
                _ => return Err(fail("unsupported animation target")),
            }
        }
        for (node, (translation, rotation, scale)) in trs {
            locals[node] = node_transform(gltf::scene::Transform::Decomposed {
                translation,
                rotation,
                scale,
            })?;
        }
        let mut worlds = vec![IDENTITY; locals.len()];
        for &node in &self.traversal {
            let parent = self.node_parents[node]
                .map(|index| worlds[index])
                .unwrap_or(IDENTITY);
            worlds[node] = checked_matrix(matrix_mul(parent, locals[node]))?;
        }
        let mut corrected = false;
        let globals = self
            .joint_nodes
            .iter()
            .enumerate()
            .map(|(index, &node)| {
                let global = checked_matrix(matrix_mul(
                    self.conversion,
                    matrix_mul(worlds[node], self.inverse_conversion),
                ))?;
                let (inverse, changed) = skin::canonical_bind_global(global).map_err(|error| {
                    fail(&format!(
                        "animation bone '{}' at {time:.6}s must be rigid: {error}",
                        self.bones[index].name,
                    ))
                })?;
                corrected |= changed;
                // The bind helper validates and inverts near-rigid input. Inverting
                // its rigid result removes bake residue while retaining joint positions.
                let mut rigid = skin::inverse_orthogonal_scaled(inverse)?;
                rigid[3] = global[3];
                Ok(self
                    .bone_basis
                    .map_or(rigid, |basis| matrix_mul(rigid, basis)))
            })
            .collect::<Result<Vec<_>>>()?;
        for (index, bone) in self.bones.iter().enumerate() {
            let local = match bone.parent {
                None => globals[index],
                Some(parent) => matrix_mul(
                    skin::inverse_orthogonal_scaled(globals[parent as usize])?,
                    globals[index],
                ),
            };
            skin::rigid(local).map_err(|error| {
                let axes = [local[0], local[1], local[2]].map(|v| [v[0], v[1], v[2]]);
                let lengths = axes.map(|axis| dot(axis, axis).sqrt());
                let dots = [(0, 1), (0, 2), (1, 2)].map(|(a, b)| dot(axes[a], axes[b]));
                fail(&format!(
                    "animation bone '{}' at {time:.6}s must be rigid (axis lengths {lengths:?}, axis dots {dots:?}): {error}",
                    bone.name,
                ))
            })?;
            tracks[index]
                .translations
                .push(f32v([local[3][0], local[3][1], local[3][2]])?);
            tracks[index]
                .rotations
                .push(skin::rotation_quaternion(local)?);
        }
        Ok(corrected)
    }
}
