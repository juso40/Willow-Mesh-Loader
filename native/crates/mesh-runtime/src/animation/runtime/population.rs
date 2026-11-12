//! Fresh AnimSet admission, retained storage and native finalization.

use super::{
    bindings::{CODEC_LINKS, SET_POST_LOAD},
    layout::{anim_set, sequence},
    objects, storage,
};
use crate::{
    Error, Result,
    animation::{PreparedAnimations, encoding::encode},
    engine::{
        construct::{self, load, retain_allocation},
        native, session,
    },
};
use unrealsdk_rs::{
    objects::{fname_from_str, object_outer},
    types::UObject,
};

pub(in crate::animation) fn populate_new(
    prepared: &PreparedAnimations,
    path: &str,
    sequence_paths: &[&str],
    mesh: Option<&str>,
) -> Result<String> {
    let target = mesh
        .map(crate::skeletal::get_reference_skeleton)
        .transpose()?;
    let (bones, order) = prepared.track_layout(target.as_deref())?;
    let encoded = prepared
        .clips
        .iter()
        .map(|clip| encode(clip, &order))
        .collect::<Result<Vec<_>>>()?;
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let links = CODEC_LINKS.resolve(base)?;
    let post_load = SET_POST_LOAD.resolve(base)?;
    let set = construct::fresh_object(
        path,
        anim_set::CLASS,
        anim_set::SIZE,
        anim_set::FRESH_ARRAYS,
    )?;
    let set_object = set.address as *mut UObject;
    objects::anim_set(set_object)?;
    let mut destinations = Vec::new();
    let mut identities = std::collections::BTreeSet::new();
    for path in sequence_paths {
        let target = construct::fresh_object(
            path,
            sequence::CLASS,
            sequence::SIZE,
            sequence::FRESH_ARRAYS,
        )?;
        let object = target.address as *mut UObject;
        objects::sequence(object)?;
        if load(target.address + sequence::FLAGS)? & sequence::ADDITIVE != 0 {
            return Err(Error(
                "fresh animation population requires non-additive sequences".into(),
            ));
        }
        if object_outer(object) != Some(set_object) || !identities.insert(target.address) {
            return Err(Error(
                "animation destinations must be distinct children of the destination AnimSet"
                    .into(),
            ));
        }
        destinations.push(target);
    }
    construct::retain_object("animation set", set_object)?;
    for target in &destinations {
        construct::retain_object("animation sequence", target.address as *mut UObject)?;
    }
    let track_names = retain_allocation(bones.len() * 8)?;
    for (index, bone) in bones.iter().enumerate() {
        let name = fname_from_str(&bone.name, 0)
            .ok_or_else(|| Error("animation bone FName unavailable".into()))?;
        track_names.u32(index * 8, name.index)?;
        track_names.u32(index * 8 + 4, name.number)?;
    }
    let storage = encoded
        .iter()
        .map(storage::stage)
        .collect::<Result<Vec<_>>>()?;
    let clip_names = prepared
        .clips
        .iter()
        .map(|clip| {
            fname_from_str(&clip.name, 0)
                .ok_or_else(|| Error("animation sequence FName unavailable".into()))
        })
        .collect::<Result<Vec<_>>>()?;
    set.u32(
        anim_set::FLAGS,
        load(set.address + anim_set::FLAGS)? & !anim_set::ROTATION_ONLY,
    )?;
    set.header(
        anim_set::TRACK_NAMES,
        track_names.address,
        bones.len() as u32,
    )?;
    let sequences = retain_allocation(prepared.clips.len() * 4)?;
    for (index, (((clip, storage), name), destination)) in prepared
        .clips
        .iter()
        .zip(&storage)
        .zip(&clip_names)
        .zip(destinations)
        .enumerate()
    {
        destination.u32(sequence::NAME, name.index)?;
        destination.u32(sequence::NAME + 4, name.number)?;
        destination.f32(sequence::RATE_SCALE, 1.)?;
        storage::install(destination, clip, storage, links)?;
        sequences.u32(index * 4, destination.address as u32)?;
    }
    set.header(
        anim_set::SEQUENCES,
        sequences.address,
        prepared.clips.len() as u32,
    )?;
    // SAFETY: the fresh exact AnimSet owns complete sequences and track names.
    // Its reviewed PostLoad populates SequenceCache without changing the track layout.
    unsafe { post_load(set.address as *mut native::AnimSet) };
    construct::object_path("animation set", set_object)
}
