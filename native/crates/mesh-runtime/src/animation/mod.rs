//! Validated skeletal clips, portable encoding and native animation population.

mod encoding;
mod runtime;
mod validation;

use crate::{Error, Result, skeletal::ReferenceBone};

/// One local bone track with uniform source samples.
#[derive(Debug, Clone)]
pub struct BoneTrack {
    /// Engine-space local translations.
    pub translations: Vec<[f32; 3]>,
    /// Unit local XYZW rotations, using the reference-bone convention.
    pub rotations: Vec<[f32; 4]>,
}

/// One absolute animation clip with both endpoints included in its sample grid.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    /// Native sequence identifier used for fresh assets.
    pub name: String,
    /// Positive duration in seconds.
    pub duration: f32,
    /// Number of uniformly spaced samples per nonconstant track.
    pub frames: usize,
    /// Local tracks in the associated reference-skeleton order.
    pub tracks: Vec<BoneTrack>,
}

/// Validated absolute animations and their unchanged source reference skeleton.
pub struct PreparedAnimations {
    bones: Vec<ReferenceBone>,
    clips: Vec<AnimationClip>,
}

impl PreparedAnimations {
    /// Validates track counts, finite values, unit rotations and native size limits.
    pub fn new(bones: Vec<ReferenceBone>, clips: Vec<AnimationClip>) -> Result<Self> {
        validation::validate(&bones, &clips)?;
        Ok(Self { bones, clips })
    }

    /// Returns source reference bones in sampled-track order without exposing track samples.
    pub fn reference_skeleton(&self) -> &[ReferenceBone] {
        &self.bones
    }

    /// Validates matching bone counts, names and direct parents against a loaded mesh.
    /// Reference poses may differ. Sampled local poses remain unchanged.
    pub fn check_mesh(&self, mesh: &str) -> Result<()> {
        self.bind(&crate::skeletal::get_reference_skeleton(mesh)?)?;
        Ok(())
    }

    /// Populates an empty Python-constructed AnimSet and its empty child sequences.
    /// Sequence paths follow the prepared clip order and each child belongs to that set.
    pub fn populate_new(&self, path: &str, sequences: &[&str], mesh: &str) -> Result<String> {
        if sequences.len() != self.clips.len() {
            return Err(Error(
                "animation destination count differs from the prepared clip count".into(),
            ));
        }
        runtime::populate_new(self, path, sequences, mesh)
    }

    /// Replaces one loaded non-additive sequence without changing its identity or track layout.
    /// The caller stops all users and reinitializes their playback state after replacement.
    pub fn load_into(&self, path: &str, mesh: &str) -> Result<String> {
        if self.clips.len() != 1 {
            return Err(Error(
                "animation replacement requires exactly one selected clip".into(),
            ));
        }
        runtime::replace(self, path, mesh)
    }

    fn bind(&self, target: &[ReferenceBone]) -> Result<Vec<usize>> {
        validation::track_order(&self.bones, target)
    }
}
