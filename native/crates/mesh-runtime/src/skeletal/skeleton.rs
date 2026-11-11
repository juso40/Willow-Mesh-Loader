//! Reference hierarchy admission, reference-pose compatibility and template binding.

use super::{MAX_BONE_NAME_UNITS, MAX_REFERENCE_BONES, ReferenceBone, failure};
use crate::{Error, Result};

const ROTATION_SQUARED_NORM_TOLERANCE: f64 = 0.001;

pub(crate) fn validate(skeleton: &[ReferenceBone]) -> Result<()> {
    if skeleton.is_empty() || skeleton.len() > MAX_REFERENCE_BONES {
        return Err(failure("skeleton requires 1..=256 reference bones"));
    }
    let mut names = std::collections::BTreeSet::new();
    for (index, bone) in skeleton.iter().enumerate() {
        let name = &bone.name;
        let norm: f64 = bone
            .rotation_xyzw
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum();
        if name.is_empty()
            || name.encode_utf16().count() > MAX_BONE_NAME_UNITS
            || name.contains('\0')
            || name.eq_ignore_ascii_case("None")
            || !names.insert(name.to_ascii_lowercase())
            || !norm.is_finite()
            || (norm - 1.).abs() > ROTATION_SQUARED_NORM_TOLERANCE
            || bone.translation.iter().any(|value| !value.is_finite())
        {
            return Err(failure(
                "reference bones require distinct bounded identifiers, finite unit rotations and finite translations",
            ));
        }
        match bone.parent {
            None if index == 0 => {}
            Some(parent) if usize::from(parent) < index => {}
            _ => {
                return Err(failure(
                    "reference skeleton must be parent-before-child with a single root at index 0",
                ));
            }
        }
    }
    Ok(())
}

/// Validates the hierarchy and returns weighted active bones and all required bones.
/// The single LOD evaluates every reference bone, including unweighted camera and
/// attachment helpers. Only weighted bones enter the GPU palette.
pub fn bone_closure(skeleton: &[ReferenceBone], used: &[u16]) -> Result<(Vec<u16>, Vec<u8>)> {
    validate(skeleton)?;
    let mut active = used.to_vec();
    active.sort_unstable();
    active.dedup();
    if active.is_empty() {
        return Err(failure("skinned geometry references no bones"));
    }
    if active
        .iter()
        .any(|&bone| usize::from(bone) >= skeleton.len())
    {
        return Err(failure(
            "skin influence joint exceeds the reference skeleton",
        ));
    }
    let required = (0..skeleton.len())
        .map(|index| {
            u8::try_from(index).map_err(|_| failure("required bone index exceeds byte indices"))
        })
        .collect::<Result<Vec<u8>>>()?;
    Ok((active, required))
}

/// Restores template bone ordinals after an exporter reorders otherwise identical joints.
/// Local GPU palette indices stay unchanged. Names, parent identities and reference
/// frames must match the template, so this operation does not retarget a skeleton.
pub fn bind_to_template(
    fixture: &mut super::SkinnedMesh,
    template: &[ReferenceBone],
) -> Result<()> {
    validate(template)?;
    validate(&fixture.skeleton)?;
    if template.len() != fixture.skeleton.len() {
        return Err(Error(
            "Gestalt template requires an unchanged bone count".into(),
        ));
    }
    let mut mapping = Vec::new();
    for bone in &fixture.skeleton {
        mapping.push(
            template
                .iter()
                .position(|candidate| candidate.name == bone.name)
                .ok_or_else(|| Error(format!("Gestalt template has no bone named {}", bone.name)))?
                as u16,
        );
    }
    let mut reordered = template.to_vec();
    for (index, bone) in fixture.skeleton.iter().enumerate() {
        let mut bone = bone.clone();
        bone.parent = bone.parent.map(|parent| mapping[parent as usize]);
        reordered[mapping[index] as usize] = bone;
    }
    compatible(template, &reordered)?;
    let palette: Vec<_> = fixture
        .skin
        .palette
        .iter()
        .map(|&joint| {
            mapping
                .get(joint as usize)
                .copied()
                .ok_or_else(|| Error("skin palette exceeds imported reference skeleton".into()))
        })
        .collect::<Result<_>>()?;
    let (active, required) = bone_closure(template, &palette)?;
    fixture.skeleton = template.to_vec();
    fixture.skin.palette = palette;
    for chunk in &mut fixture.skin.chunks {
        for bone in &mut chunk.palette {
            *bone = mapping[*bone as usize];
        }
    }
    fixture.active_bones = active;
    fixture.required_bones = required;
    Ok(())
}

pub(crate) fn compatible(original: &[ReferenceBone], incoming: &[ReferenceBone]) -> Result<()> {
    const TRANSLATION_TOLERANCE: f32 = 0.001;
    const ROTATION_COMPONENT_TOLERANCE: f32 = 0.00001;
    if original.len() != incoming.len() {
        return Err(Error(
            "skeletal reload requires an unchanged bone count".into(),
        ));
    }
    for (index, (old, new)) in original.iter().zip(incoming).enumerate() {
        let sign = if old
            .rotation_xyzw
            .iter()
            .zip(new.rotation_xyzw)
            .map(|(a, b)| a * b)
            .sum::<f32>()
            < 0.0
        {
            -1.0
        } else {
            1.0
        };
        if old.name != new.name
            || old.parent != new.parent
            || old
                .translation
                .iter()
                .zip(new.translation)
                .any(|(a, b)| !b.is_finite() || (a - b).abs() > TRANSLATION_TOLERANCE)
            || old
                .rotation_xyzw
                .iter()
                .zip(new.rotation_xyzw)
                .any(|(a, b)| !b.is_finite() || (a - sign * b).abs() > ROTATION_COMPONENT_TOLERANCE)
        {
            return Err(Error(format!(
                "skeletal reload requires unchanged bone names/order/parents/rest frames. Bone {index} ({}) differs. Restart to change the skeleton",
                old.name
            )));
        }
    }
    Ok(())
}
