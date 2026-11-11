//! Existing sequence admission with preserved owning-set layout and event metadata.

use super::{
    bindings::CODEC_LINKS,
    layout::{anim_set, sequence},
    objects, storage,
};
use crate::{
    Error, Result,
    animation::{PreparedAnimations, encoding::encode},
    engine::{
        abi,
        construct::{self, Region, load},
        session,
    },
};
use unrealsdk_rs::{
    objects::{fname_to_string, object_outer},
    types::FName,
};

pub(in crate::animation) fn replace(
    prepared: &PreparedAnimations,
    path: &str,
    mesh: &str,
) -> Result<String> {
    prepared.bind(&crate::skeletal::get_reference_skeleton(mesh)?)?;
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let links = CODEC_LINKS.resolve(base)?;
    let object = objects::loaded(path, sequence::CLASS)?;
    let destination = objects::sequence(object)?;
    if load(destination.address + sequence::FLAGS)? & sequence::ADDITIVE != 0 {
        return Err(Error(
            "animation replacement refuses additive sequences".into(),
        ));
    }
    let parent = object_outer(object)
        .ok_or_else(|| Error("animation sequence has no owning AnimSet".into()))?;
    let set = objects::anim_set(parent)?;
    let sequences = construct::read_array(set.address + anim_set::SEQUENCES, 4, i32::MAX as usize)?;
    if !sequences
        .chunks_exact(4)
        .any(|bytes| bytes == (object as u32).to_le_bytes())
    {
        return Err(Error(
            "animation sequence is absent from its owning AnimSet".into(),
        ));
    }
    let order = existing_track_order(set, prepared)?;
    let clip = &prepared.clips[0];
    if load(destination.address + sequence::CURVES + abi::ARRAY_COUNT)? != 0 {
        return Err(Error(
            "animation replacement refuses existing curve tracks".into(),
        ));
    }
    let old_duration = f32::from_bits(load(destination.address + sequence::LENGTH)?);
    if !old_duration.is_finite() || old_duration <= 0. {
        return Err(Error(
            "animation replacement refuses invalid existing duration".into(),
        ));
    }
    if load(destination.address + sequence::NOTIFIES + abi::ARRAY_COUNT)? != 0
        && (old_duration - clip.duration).abs() > 0.00001
    {
        return Err(Error(
            "animation replacement with existing notifies requires the same duration".into(),
        ));
    }
    let encoded = encode(clip, &order)?;
    let staged = storage::stage(&encoded)?;
    construct::retain_object("animation sequence", object)?;
    construct::retain_object("animation set", parent)?;
    storage::install(destination, clip, &staged, links)?;
    construct::object_path("animation sequence", object)
}

fn existing_track_order(set: Region, prepared: &PreparedAnimations) -> Result<Vec<usize>> {
    let names = construct::read_array(
        set.address + anim_set::TRACK_NAMES,
        8,
        crate::skeletal::MAX_REFERENCE_BONES,
    )?;
    if names.is_empty() {
        return Err(Error("animation set has no bone tracks".into()));
    }
    let mut order = Vec::new();
    for bytes in names.chunks_exact(8) {
        let name = FName {
            index: crate::engine::memory::word(bytes, 0)?,
            number: crate::engine::memory::word(bytes, 4)?,
        };
        let name = fname_to_string(&name)
            .ok_or_else(|| Error("animation track name unavailable".into()))?
            .text;
        let index = prepared
            .bones
            .iter()
            .position(|bone| bone.name.eq_ignore_ascii_case(&name))
            .ok_or_else(|| {
                Error(format!(
                    "source skeleton has no existing animation track {name}"
                ))
            })?;
        if order.contains(&index) {
            return Err(Error("animation set contains duplicate bone tracks".into()));
        }
        order.push(index);
    }
    Ok(order)
}
