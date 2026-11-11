//! Scene hierarchy traversal and inverse-bind-derived reference skeletons.

use gltf::accessor::DataType;

use super::{
    ImportBone, canonical_bind_global, inverse_orthogonal_scaled, rigid, rotation_quaternion,
    valid_bone_name,
};
use crate::{Result, fail, math::*, warn};

pub(super) const IDENTITY: Matrix = [
    [1., 0., 0., 0.],
    [0., 1., 0., 0.],
    [0., 0., 1., 0.],
    [0., 0., 0., 1.],
];

pub(super) struct SceneTransforms {
    pub worlds: Vec<Option<Matrix>>,
    parents: Vec<Option<usize>>,
}

impl SceneTransforms {
    pub fn read(source: &crate::Source, mesh_node: Option<usize>) -> Result<Self> {
        let hierarchy = crate::hierarchy::Hierarchy::read(source)?;
        if let Some(node) = mesh_node {
            hierarchy.relevant([node])?;
        }
        let worlds = hierarchy.worlds(source.skin()?.joints().map(|joint| joint.index()))?;
        Ok(Self {
            worlds,
            parents: hierarchy.parents,
        })
    }
}

pub(super) struct Skeleton {
    pub bones: Vec<ImportBone>,
    pub joint_to_bone: Vec<u16>,
    pub conversion: Matrix,
}

pub(super) fn decode(
    skin: gltf::Skin<'_>,
    scene: &SceneTransforms,
    buffers: &crate::document::Buffers<'_>,
    scale: f32,
    bone_basis: Option<Matrix>,
    warnings: &mut Vec<String>,
) -> Result<Skeleton> {
    let joints: Vec<gltf::Node<'_>> = skin.joints().collect();
    if joints.is_empty() || joints.len() > crate::policy::MAX_JOINTS {
        return Err(fail("skin requires 1..=256 joints"));
    }
    let mut joint_set = vec![false; scene.worlds.len()];
    let mut names = std::collections::BTreeSet::new();
    for joint in &joints {
        let name = joint
            .name()
            .filter(|name| valid_bone_name(name))
            .ok_or_else(|| {
                fail("joint names must fit the native FName format (1..=1023 UTF-16 units, no NUL)")
            })?;
        if joint_set[joint.index()]
            || !names.insert(name.to_ascii_lowercase())
            || name.eq_ignore_ascii_case("None")
        {
            return Err(fail("duplicate/None joint names or duplicate skin joints"));
        }
        joint_set[joint.index()] = true;
    }
    let bind_matrices = inverse_binds(&skin, buffers, joints.len())?;
    let mut bind_globals = Vec::with_capacity(joints.len());
    for (index, matrix) in bind_matrices.iter().enumerate() {
        if matrix.iter().flatten().any(|value| !value.is_finite())
            || matrix[0][3].abs() > crate::policy::AFFINE_TOLERANCE
            || matrix[1][3].abs() > crate::policy::AFFINE_TOLERANCE
            || matrix[2][3].abs() > crate::policy::AFFINE_TOLERANCE
            || (matrix[3][3] - 1.).abs() > crate::policy::AFFINE_TOLERANCE
        {
            return Err(fail("inverse bind matrix must be finite affine"));
        }
        let world = scene.worlds[joints[index].index()]
            .ok_or_else(|| fail("skin joint is not reachable from the selected scene"))?;
        let (bind, _) = canonical_bind_global(*matrix).map_err(|error| {
            fail(&format!(
                "joint '{}': {error}",
                joints[index].name().unwrap_or("<unnamed>")
            ))
        })?;
        if (0..4).any(|column| {
            (0..4).any(|row| {
                (world[column][row] - bind[column][row]).abs()
                    > crate::policy::RIGID_BASIS_TOLERANCE * bind[column][row].abs().max(1.)
            })
        }) {
            warn(
                warnings,
                "joint node rest transforms differ from inverse binds. Using the bind-derived reference pose",
            );
        }
        bind_globals.push(bind);
    }
    let parents: Vec<_> = joints
        .iter()
        .map(|joint| {
            let mut cursor = scene.parents[joint.index()];
            while let Some(index) = cursor {
                if joint_set[index] {
                    return Some(index);
                }
                cursor = scene.parents[index];
            }
            None
        })
        .collect();
    if parents.iter().filter(|parent| parent.is_none()).count() != 1 {
        return Err(fail("skin must have a single joint root"));
    }
    let order = parent_first_order(&joints, &parents)?;
    let mut joint_to_bone = vec![0u16; joints.len()];
    for (bone, &joint) in order.iter().enumerate() {
        joint_to_bone[joint] = bone as u16;
    }
    let conversion = crate::coordinates::matrix(f64::from(scale));
    let inverse_conversion = inverse_orthogonal_scaled(conversion)?;
    let mut engine_globals = Vec::with_capacity(joints.len());
    for &joint in &order {
        let global = matrix_mul(
            conversion,
            matrix_mul(bind_globals[joint], inverse_conversion),
        );
        let global = bone_basis.map_or(global, |basis| matrix_mul(global, basis));
        engine_globals.push(checked_matrix(global)?);
    }
    let mut bones = Vec::with_capacity(joints.len());
    for (bone, &joint) in order.iter().enumerate() {
        let name = joints[joint]
            .name()
            .filter(|name| valid_bone_name(name))
            .ok_or_else(|| fail("skin joint nodes need nonempty native FName identifiers"))?
            .to_owned();
        let (local, parent) = match parents[joint] {
            None => (engine_globals[bone], None),
            Some(parent_node) => {
                let parent_joint = joints
                    .iter()
                    .position(|node| node.index() == parent_node)
                    .ok_or_else(|| fail("joint parent was not in the skin"))?;
                let parent_bone = joint_to_bone[parent_joint];
                (
                    matrix_mul(
                        inverse_orthogonal_scaled(engine_globals[parent_bone as usize])?,
                        engine_globals[bone],
                    ),
                    Some(parent_bone),
                )
            }
        };
        rigid(local)?;
        let rotation = rotation_quaternion(local)?;
        let translation = f32v([local[3][0], local[3][1], local[3][2]])?;
        bones.push(ImportBone {
            name,
            parent,
            translation,
            rotation_xyzw: rotation,
        });
    }
    Ok(Skeleton {
        bones,
        joint_to_bone,
        conversion,
    })
}

fn inverse_binds(
    skin: &gltf::Skin<'_>,
    buffers: &crate::document::Buffers<'_>,
    joint_count: usize,
) -> Result<Vec<Matrix>> {
    let accessor = skin
        .inverse_bind_matrices()
        .ok_or_else(|| fail("skin needs inverse bind matrices"))?;
    if accessor.data_type() != DataType::F32
        || accessor.dimensions() != gltf::accessor::Dimensions::Mat4
        || accessor.normalized()
    {
        return Err(fail("inverse bind matrices need unnormalized float MAT4"));
    }
    let reader = skin.reader(|buffer| buffers.get(buffer.index()));
    let values: Vec<[[f32; 4]; 4]> = reader
        .read_inverse_bind_matrices()
        .ok_or_else(|| fail("skin needs inverse bind matrices"))?
        .collect();
    if values.len() != joint_count {
        return Err(fail("inverse bind matrix count differs from joints"));
    }
    // glTF matrices remain column-major throughout bind reconstruction.
    Ok(values
        .into_iter()
        .map(|matrix| {
            std::array::from_fn(|column| std::array::from_fn(|row| f64::from(matrix[column][row])))
        })
        .collect())
}

fn parent_first_order(joints: &[gltf::Node<'_>], parents: &[Option<usize>]) -> Result<Vec<usize>> {
    let mut order = Vec::new();
    let mut placed = vec![false; joints.len()];
    while order.len() < joints.len() {
        let mut progressed = false;
        for index in 0..joints.len() {
            if placed[index] {
                continue;
            }
            let ready = parents[index].is_none_or(|parent| {
                joints
                    .iter()
                    .position(|joint| joint.index() == parent)
                    .is_some_and(|index| placed[index])
            });
            if ready {
                placed[index] = true;
                order.push(index);
                progressed = true;
                break; // Skin order is reconsidered immediately after each placement.
            }
        }
        if !progressed {
            return Err(fail("joint hierarchy is cyclic or has no root"));
        }
    }
    Ok(order)
}
