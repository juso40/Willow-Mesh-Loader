//! Exact-class animation views with checked native dispatch and reflected offsets.

use super::layout::{anim_set, sequence};
use crate::{
    Error, Result,
    engine::construct::{self, Region, load},
};
use unrealsdk_rs::{
    objects::{class_default_object, find_object, find_property, object_class, property_offset},
    types::UObject,
};

fn check_object(
    object: *mut UObject,
    class: &str,
    size: usize,
    fields: &[(&str, usize)],
) -> Result<Region> {
    let expected = construct::require_class(class)?;
    if object_class(object) != Some(expected) {
        return Err(Error(format!(
            "animation operation requires the exact {class} class"
        )));
    }
    construct::verify_live(class, construct::flags_of(class, object)?)?;
    let default = class_default_object(expected)
        .ok_or_else(|| Error(format!("{class} default object unavailable")))?;
    if load(object as usize)? != load(default as usize)? {
        return Err(Error(format!(
            "{class} native dispatch differs from its default object"
        )));
    }
    for &(name, offset) in fields {
        if find_property(expected, name).and_then(|property| property_offset(property))
            != Some(offset as i32)
        {
            return Err(Error(format!(
                "{class}.{name} native layout differs from the reviewed runtime"
            )));
        }
    }
    construct::read(object as usize, size)?;
    Ok(Region {
        address: object as usize,
        length: size,
    })
}

pub(super) fn sequence(object: *mut UObject) -> Result<Region> {
    check_object(
        object,
        sequence::CLASS,
        sequence::SIZE,
        &[
            ("SequenceName", sequence::NAME),
            ("SequenceLength", sequence::LENGTH),
            ("NumFrames", sequence::FRAMES),
            ("RawAnimationData", sequence::RAW),
            ("TranslationCompressionFormat", sequence::TRANSLATION_FORMAT),
            ("RotationCompressionFormat", sequence::ROTATION_FORMAT),
            ("KeyEncodingFormat", sequence::KEY_FORMAT),
            ("CompressedTrackOffsets", sequence::OFFSETS),
            ("CompressedByteStream", sequence::STREAM),
            ("DeltaTrackCache", sequence::DELTA_CACHE),
        ],
    )
}

pub(super) fn anim_set(object: *mut UObject) -> Result<Region> {
    check_object(
        object,
        anim_set::CLASS,
        anim_set::SIZE,
        &[
            ("TrackBoneNames", anim_set::TRACK_NAMES),
            ("Sequences", anim_set::SEQUENCES),
            ("SequenceCache", anim_set::SEQUENCE_CACHE),
            ("LinkupCache", anim_set::LINKUPS),
        ],
    )
}

pub(super) fn loaded(path: &str, class: &str) -> Result<*mut UObject> {
    if path.is_empty() || path.contains('\0') || path.len() > crate::MAX_OBJECT_PATH_BYTES {
        return Err(Error(
            "animation object path must be a nonempty full path without NUL".into(),
        ));
    }
    find_object(Some(construct::require_class(class)?), path)
        .ok_or_else(|| Error(format!("loaded animation object not found: {path}")))
}
