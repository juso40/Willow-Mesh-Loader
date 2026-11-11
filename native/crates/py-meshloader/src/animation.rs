//! Animation preparation with lightweight clip descriptors and source provenance.

use crate::{
    convert, error,
    inspection::{AnimationClipInfo, SourceSelection},
    prepared::PreparedAnimationSet,
};
use mesh_import::{Source, animation::AnimationSet};
use mesh_runtime::animation::PreparedAnimations;
use pyo3::prelude::*;

pub(crate) fn prepare_source(
    source: &Source,
    scale: f32,
    sample_rate: f32,
    clips: &[usize],
    selection: SourceSelection,
) -> PyResult<PreparedAnimationSet> {
    let source = source
        .animation_clips(scale, sample_rate, clips)
        .map_err(error)?;
    let clips_info = source
        .clips
        .iter()
        .map(|clip| AnimationClipInfo {
            source_animation_id: clip.index,
            name: clip.name.clone(),
            duration: clip.duration,
            frame_count: clip.frames,
        })
        .collect();
    let AnimationSet {
        bones,
        clips,
        warnings,
    } = source;
    let data = PreparedAnimations::new(
        bones.into_iter().map(convert::bone).collect(),
        clips.into_iter().map(convert::clip).collect(),
    )
    .map_err(error)?;
    Ok(PreparedAnimationSet::new(
        data,
        selection,
        sample_rate,
        clips_info,
        warnings,
    ))
}
