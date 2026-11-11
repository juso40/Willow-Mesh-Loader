//! Existing-identity Gestalt updates and detached component-data publication.

mod geometry;
pub(crate) mod live;
#[cfg(test)]
mod tests;

use crate::{
    Result,
    engine::{construct, session},
    skeletal::{PreparedSkeletalMesh, readback, replacement},
};

/// One named Gestalt index range using encoded mesh material-slot ordinals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartRange {
    /// Exact authored fragment identity.
    pub name: String,
    /// Material-slot ordinal in the associated mesh.
    pub material_slot: usize,
    /// First triangle-list index element.
    pub first_index: usize,
    /// Number of triangles in this range.
    pub triangles: usize,
}

pub(super) fn update(
    incoming: &PreparedSkeletalMesh,
    path: &str,
    parts: &[PartRange],
    materials: &[&str],
    preserve_materials: bool,
) -> Result<String> {
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let target = live::Definition::read(path)?;
    let (current, mut material_paths) = readback::read_current_in_session(&target.mesh_path)?;
    let updated = geometry::update(&current, &target.parts, incoming, parts, preserve_materials)?;
    if !preserve_materials {
        material_paths.extend(materials.iter().map(|path| (*path).to_owned()));
    }
    let mut publication = live::Publication::stage(base, target, &updated.parts, &updated.bounds)?;
    let references: Vec<_> = material_paths.iter().map(String::as_str).collect();
    replacement::update_gestalt(&updated.mesh, &references, &mut publication)
}
