//! Skeletal glTF clips sampled into rigid engine-space local bone tracks.

mod pose;
mod sampling;

use crate::{
    Result, Source, fail,
    skin::{self, ImportBone},
};
use pose::PoseSampler;
use sampling::Channel;

/// One uniformly sampled local bone track in engine coordinates.
#[derive(Debug, Clone)]
pub struct BoneTrack {
    /// Local translations, including the source-to-engine unit scale.
    pub translations: Vec<[f32; 3]>,
    /// Unit XYZW rotations in the same convention as reference bones.
    pub rotations: Vec<[f32; 4]>,
}

/// One absolute, non-additive skeletal animation clip.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    /// Source animation index.
    pub index: usize,
    /// Authored nonempty source name.
    pub name: String,
    /// Playback duration in seconds. A single pose lasts one requested sample interval.
    pub duration: f32,
    /// Uniform sample count, including both endpoints for clips with positive duration.
    pub frames: usize,
    /// Bone tracks in reference-skeleton order.
    pub tracks: Vec<BoneTrack>,
}

/// Reference skeleton and imported clips without engine objects or geometry payloads.
#[derive(Debug)]
pub struct AnimationSet {
    /// Bind-derived source skeleton in parent-before-child order.
    pub bones: Vec<ImportBone>,
    /// Selected clips in document order, retaining their source IDs.
    pub clips: Vec<AnimationClip>,
    /// Bind/animation normalization and source rest-pose mismatch diagnostics.
    pub warnings: Vec<String>,
}

pub(crate) fn from_source(
    source: &Source,
    scale: f32,
    sample_rate: f32,
    selection: &[usize],
) -> Result<AnimationSet> {
    if !sample_rate.is_finite() || sample_rate <= 0. {
        return Err(fail("animation sample rate must be finite and positive"));
    }
    let unique: std::collections::BTreeSet<_> = selection.iter().copied().collect();
    if selection.is_empty()
        || unique.len() != selection.len()
        || selection
            .iter()
            .any(|&id| id >= source.document().animations().len())
    {
        return Err(fail(
            "clip selection must contain distinct existing source animation IDs",
        ));
    }
    let bind_buffers = source.buffers(source.skin()?.inverse_bind_matrices())?;
    let reference = skin::reference_skeleton(source, scale, &bind_buffers)?;
    let bones = reference.bones;
    let sampler = PoseSampler::new(source, &bones, scale)?;
    let mut accessors = Vec::new();
    for animation in source
        .document()
        .animations()
        .filter(|animation| selection.contains(&animation.index()))
    {
        source.check_extensions(&format!("/animations/{}", animation.index()))?;
        for (index, channel) in animation.channels().enumerate() {
            let path = format!("/animations/{}/channels/{index}", animation.index());
            source.check_extensions(&path)?;
            source.check_extensions(&format!("{path}/target"))?;
            source.check_extensions(&format!(
                "/animations/{}/samplers/{}",
                animation.index(),
                channel.sampler().index()
            ))?;
            accessors.push(channel.sampler().input());
            if sampler.includes(channel.target().node().index()) {
                accessors.push(channel.sampler().output());
            }
        }
    }
    let buffers = source.buffers(accessors)?;
    let mut clips = Vec::new();
    let mut warnings = reference.warnings;
    let mut names = std::collections::BTreeSet::new();
    for animation in source
        .document()
        .animations()
        .filter(|animation| selection.contains(&animation.index()))
    {
        let (clip, corrected, excluded) = sample_clip(animation, &buffers, &sampler, sample_rate)?;
        if excluded {
            crate::warn(
                &mut warnings,
                &format!(
                    "animation '{}': channels outside the selected skin and its ancestors were excluded",
                    clip.name
                ),
            );
        }
        if corrected {
            crate::warn(
                &mut warnings,
                &format!(
                    "animation '{}': near-rigid sampled transforms orthonormalized within 1e-4 tolerance. Joint positions preserved",
                    clip.name,
                ),
            );
        }
        if !names.insert(clip.name.to_ascii_lowercase()) {
            return Err(fail(
                "animation names must be distinct non-None native FNames",
            ));
        }
        clips.push(clip);
    }
    Ok(AnimationSet {
        bones,
        clips,
        warnings,
    })
}

fn sample_clip(
    animation: gltf::Animation<'_>,
    buffers: &crate::document::Buffers<'_>,
    sampler: &PoseSampler<'_>,
    sample_rate: f32,
) -> Result<(AnimationClip, bool, bool)> {
    let name = animation
        .name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| fail("animation clips require authored names"))?
        .to_owned();
    if !skin::valid_bone_name(&name) || name.eq_ignore_ascii_case("None") {
        return Err(fail("animation names must be non-None native FNames"));
    }
    let mut channels = Vec::new();
    let mut targets = std::collections::BTreeSet::new();
    let mut duration = 0_f64;
    let mut excluded = false;
    for channel in animation.channels() {
        if !sampler.includes(channel.target().node().index()) {
            let times = Channel::read_times(&channel, buffers)?;
            duration = duration.max(times[times.len() - 1]);
            excluded = true;
            continue;
        }
        let channel = Channel::read(channel, buffers)?;
        sampler.validate_target(channel.node)?;
        if !targets.insert((channel.node, channel.property as u8)) {
            return Err(fail("animation contains duplicate node/property channels"));
        }
        duration = duration.max(channel.end());
        channels.push(channel);
    }
    if channels.is_empty() {
        return Err(fail(
            "animation clip has no channels for the selected skeleton",
        ));
    }
    let intervals = (duration * f64::from(sample_rate)).ceil();
    if !intervals.is_finite() || intervals >= i32::MAX as f64 {
        return Err(fail("animation frame count exceeds native limits"));
    }
    let frames = if duration == 0. {
        1
    } else {
        intervals as usize + 1
    };
    if frames
        .checked_mul(sampler.track_count())
        .and_then(|n| n.checked_mul(28))
        .is_none_or(|n| n > i32::MAX as usize)
    {
        return Err(fail(
            "animation samples exceed the signed 32-bit payload size",
        ));
    }
    let mut tracks: Vec<_> = (0..sampler.track_count())
        .map(|_| BoneTrack {
            translations: Vec::new(),
            rotations: Vec::new(),
        })
        .collect();
    for track in &mut tracks {
        track
            .translations
            .try_reserve_exact(frames)
            .map_err(|_| fail("animation sample allocation failed"))?;
        track
            .rotations
            .try_reserve_exact(frames)
            .map_err(|_| fail("animation sample allocation failed"))?;
    }
    let mut corrected = false;
    for frame in 0..frames {
        let time = if frames == 1 {
            0.
        } else {
            duration * frame as f64 / (frames - 1) as f64
        };
        corrected |= sampler.sample(&channels, time, &mut tracks)?;
    }
    let duration = if duration == 0. {
        1. / sample_rate
    } else {
        duration as f32
    };
    if !duration.is_finite() || duration <= 0. {
        return Err(fail(
            "animation duration must remain finite and positive in f32",
        ));
    }
    Ok((
        AnimationClip {
            index: animation.index(),
            name,
            duration,
            frames,
            tracks,
        },
        corrected,
        excluded,
    ))
}
