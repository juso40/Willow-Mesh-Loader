//! Animation sample admission and source-to-target track layout matching.

use super::AnimationClip;
use crate::{Error, Result, skeletal::ReferenceBone};

pub(super) fn validate(bones: &[ReferenceBone], clips: &[AnimationClip]) -> Result<()> {
    crate::skeletal::skeleton::validate(bones)?;
    if clips.is_empty() || clips.len() > i32::MAX as usize {
        return Err(Error(
            "animations require a nonempty native clip array".into(),
        ));
    }
    let mut names = std::collections::BTreeSet::new();
    for clip in clips {
        if clip.name.is_empty()
            || clip.name.encode_utf16().count() > crate::MAX_NAME_UTF16_UNITS
            || clip.name.contains('\0')
            || clip.name.eq_ignore_ascii_case("None")
            || !names.insert(clip.name.to_ascii_lowercase())
            || !clip.duration.is_finite()
            || clip.duration <= 0.
            || clip.frames == 0
            || clip.frames > i32::MAX as usize
            || clip.tracks.len() != bones.len()
        {
            return Err(Error("invalid or duplicate animation clip metadata".into()));
        }
        if clip
            .frames
            .checked_mul(bones.len())
            .and_then(|count| count.checked_mul(28))
            .is_none_or(|count| count > i32::MAX as usize)
        {
            return Err(Error(
                "animation sample payload exceeds native limits".into(),
            ));
        }
        for track in &clip.tracks {
            if track.translations.len() != clip.frames
                || track.rotations.len() != clip.frames
                || track
                    .translations
                    .iter()
                    .flatten()
                    .any(|value| !value.is_finite())
                || track.rotations.iter().any(|quaternion| {
                    quaternion.iter().any(|value| !value.is_finite())
                        || (quaternion
                            .iter()
                            .map(|value| f64::from(*value).powi(2))
                            .sum::<f64>()
                            - 1.)
                            .abs()
                            > 0.001
                })
            {
                return Err(Error("animation tracks require finite translations and unit rotations for every frame".into()));
            }
        }
    }
    Ok(())
}

pub(super) fn track_order(
    source: &[ReferenceBone],
    target: &[ReferenceBone],
) -> Result<Vec<usize>> {
    if source.len() != target.len() {
        return Err(Error(
            "animation source and target require the same bone count".into(),
        ));
    }
    crate::skeletal::skeleton::validate(target)?;
    let mut target_order = Vec::new();
    for bone in target {
        target_order.push(
            source
                .iter()
                .position(|source| source.name == bone.name)
                .ok_or_else(|| {
                    Error(format!("animation source has no bone named {}", bone.name))
                })?,
        );
    }
    for (&index, bone) in target_order.iter().zip(target) {
        let parent = source[index]
            .parent
            .map(|parent| {
                target_order
                    .iter()
                    .position(|&index| index == parent as usize)
                    .map(|index| index as u16)
                    .ok_or_else(|| Error("animation parent mapping is incomplete".into()))
            })
            .transpose()?;
        if parent != bone.parent {
            return Err(Error(format!(
                "animation source and target require the same direct parent for bone {}",
                bone.name,
            )));
        }
    }
    Ok(target_order)
}
