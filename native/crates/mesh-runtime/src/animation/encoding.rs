//! Portable constant-key stream encoding and ActorX quaternion representation.

use super::{AnimationClip, BoneTrack};
use crate::{Error, Result};

pub(super) struct EncodedTrack {
    pub positions: Vec<u8>,
    pub rotations: Vec<u8>,
    pub position_count: u32,
    pub rotation_count: u32,
}

pub(super) struct EncodedClip {
    pub offsets: Vec<u8>,
    pub stream: Vec<u8>,
    pub raw: Vec<EncodedTrack>,
}

struct TrackOffsets {
    translation_offset: usize,
    translation_count: usize,
    rotation_offset: usize,
    rotation_count: usize,
}

impl TrackOffsets {
    fn append_to(self, bytes: &mut Vec<u8>) -> Result<()> {
        for value in [
            self.translation_offset,
            self.translation_count,
            self.rotation_offset,
            self.rotation_count,
        ] {
            if value > i32::MAX as usize {
                return Err(Error("animation stream exceeds native limits".into()));
            }
            bytes.extend_from_slice(&(value as u32).to_le_bytes());
        }
        Ok(())
    }
}

pub(super) fn encode(clip: &AnimationClip, order: &[usize]) -> Result<EncodedClip> {
    let mut encoded = EncodedClip {
        offsets: Vec::new(),
        stream: Vec::new(),
        raw: Vec::new(),
    };
    for &index in order {
        let track = clip
            .tracks
            .get(index)
            .ok_or_else(|| Error("animation track mapping exceeds source tracks".into()))?;
        let position_count = if track
            .translations
            .iter()
            .all(|value| *value == track.translations[0])
        {
            1
        } else {
            clip.frames
        };
        let rotation_count = if constant_rotation(track) {
            1
        } else {
            clip.frames
        };
        let mut positions = Vec::new();
        for value in &track.translations[..position_count] {
            for component in value {
                positions.extend_from_slice(&component.to_le_bytes());
            }
        }
        let translation_offset = encoded.stream.len();
        encoded.stream.extend_from_slice(&positions);
        let rotation_offset = encoded.stream.len();
        let mut rotations = Vec::new();
        for quaternion in &track.rotations[..rotation_count] {
            let norm = quaternion
                .iter()
                .map(|value| f64::from(*value).powi(2))
                .sum::<f64>()
                .sqrt();
            let mut normalized = quaternion.map(|value| (f64::from(value) / norm) as f32);
            // Non-root playback flips W to undo ActorX's exported quaternion convention.
            if index != 0 {
                normalized[3] = -normalized[3];
            }
            for component in normalized {
                rotations.extend_from_slice(&component.to_le_bytes());
            }
            // Float96NoW reconstructs positive W. Whole-quaternion negation preserves rotation.
            let sign = if normalized[3] < 0. { -1. } else { 1. };
            for component in &normalized[..3] {
                encoded
                    .stream
                    .extend_from_slice(&(component * sign).to_le_bytes());
            }
        }
        TrackOffsets {
            translation_offset,
            translation_count: position_count,
            rotation_offset,
            rotation_count,
        }
        .append_to(&mut encoded.offsets)?;
        encoded.raw.push(EncodedTrack {
            positions,
            rotations,
            position_count: position_count as u32,
            rotation_count: rotation_count as u32,
        });
    }
    if encoded.stream.len() > i32::MAX as usize {
        return Err(Error("animation stream exceeds native limits".into()));
    }
    Ok(encoded)
}

fn constant_rotation(track: &BoneTrack) -> bool {
    track.rotations.iter().all(|quaternion| {
        let dot = quaternion
            .iter()
            .zip(track.rotations[0])
            .map(|(a, b)| a * b)
            .sum::<f32>();
        let sign = if dot < 0. { -1. } else { 1. };
        quaternion
            .iter()
            .zip(track.rotations[0])
            .all(|(a, b)| (*a - sign * b).abs() <= f32::EPSILON)
    })
}
