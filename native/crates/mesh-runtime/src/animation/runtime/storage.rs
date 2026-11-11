//! Retained engine allocations and publication of complete animation streams.

use super::layout::sequence;
use crate::{
    Error, Result,
    animation::{AnimationClip, encoding::EncodedClip},
    engine::{
        construct::{Region, load, retain_allocation, retain_bytes},
        native::{self, AnimationCodecLinks},
    },
};

pub(super) struct Storage {
    offsets: Region,
    stream: Region,
    raw: Region,
    tracks: u32,
}

pub(super) fn stage(encoded: &EncodedClip) -> Result<Storage> {
    let offsets = retain_bytes(&encoded.offsets)?;
    let stream = retain_bytes(&encoded.stream)?;
    let raw = retain_allocation(encoded.raw.len() * sequence::RAW_TRACK_SIZE)?;
    for (index, track) in encoded.raw.iter().enumerate() {
        let positions = retain_bytes(&track.positions)?;
        let rotations = retain_bytes(&track.rotations)?;
        raw.header(
            index * sequence::RAW_TRACK_SIZE,
            positions.address,
            track.position_count,
        )?;
        raw.header(
            index * sequence::RAW_TRACK_SIZE + sequence::RAW_ROTATIONS,
            rotations.address,
            track.rotation_count,
        )?;
    }
    Ok(Storage {
        offsets,
        stream,
        raw,
        tracks: encoded.raw.len() as u32,
    })
}

pub(super) fn install(
    destination: Region,
    clip: &AnimationClip,
    storage: &Storage,
    links: AnimationCodecLinks,
) -> Result<()> {
    destination.f32(sequence::LENGTH, clip.duration)?;
    destination.u32(sequence::FRAMES, clip.frames as u32)?;
    destination.u32(
        sequence::FLAGS,
        load(destination.address + sequence::FLAGS)? | sequence::NO_LOOPING_INTERPOLATION,
    )?;
    destination.write(
        sequence::TRANSLATION_FORMAT,
        &sequence::CONSTANT_FLOAT96_FORMATS,
    )?;
    destination.header(sequence::RAW, storage.raw.address, storage.tracks)?;
    destination.header(
        sequence::OFFSETS,
        storage.offsets.address,
        storage.tracks * 4,
    )?;
    destination.header(
        sequence::STREAM,
        storage.stream.address,
        storage.stream.length as u32,
    )?;
    for field in [
        sequence::TRANSLATIONS,
        sequence::ROTATIONS,
        sequence::DELTA_CACHE,
    ] {
        destination.header(field, 0, 0)?;
    }
    // SAFETY: complete BL2 constant-key streams and scalar formats are installed on
    // an exact live AnimSequence. This cdecl function only selects its native codecs.
    unsafe { links(destination.address as *mut native::AnimSequence) };
    if load(destination.address + sequence::TRANSLATION_CODEC)? == 0
        || load(destination.address + sequence::ROTATION_CODEC)? == 0
    {
        return Err(Error("animation codec initialization failed".into()));
    }
    Ok(())
}
