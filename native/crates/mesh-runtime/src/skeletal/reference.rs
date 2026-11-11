//! Read-only inspection of loaded native reference skeletons.

use super::{MAX_REFERENCE_BONES, ReferenceBone, layout::mesh as mesh_layout, records::NameWords};
use crate::{
    Error, Result,
    engine::{
        abi::WORD_BYTES, construct, memory::word, reflection, session,
        signatures::SKELETAL_MESH_VTABLE,
    },
};
use unrealsdk_rs::{objects::fname_to_string, types::FName};

/// Returns a validated snapshot of a loaded mesh's reference bones in native order.
/// The mesh and its skeleton remain unchanged. No source file or reload ownership
/// is required. Native bone names, parents and local reference frames are returned.
pub fn get_reference_skeleton(path: &str) -> Result<Vec<ReferenceBone>> {
    let _operation = session::reserve()?;
    let base = construct::admitted_base()?;
    let object = reflection::skeletal_mesh(path)?;
    let mesh = object as usize;
    construct::verify_vtable(
        "reference skeleton mesh",
        construct::load(mesh)?,
        &SKELETAL_MESH_VTABLE.resolve(base)?,
    )?;
    construct::verify_live(
        "reference skeleton mesh",
        construct::flags_of("reference skeleton mesh", object)?,
    )?;
    Ok(read(mesh)?.0)
}

pub(super) fn read(mesh: usize) -> Result<(Vec<ReferenceBone>, Vec<NameWords>)> {
    let bytes = construct::read_array(
        mesh + mesh_layout::REFERENCE_SKELETON,
        super::records::BONE_SIZE,
        MAX_REFERENCE_BONES,
    )?;
    let mut skeleton = Vec::new();
    let mut names = Vec::new();
    for (index, bone) in bytes.chunks_exact(super::records::BONE_SIZE).enumerate() {
        let name = NameWords {
            index: word(bone, 0)?,
            number: word(bone, WORD_BYTES)?,
        };
        let text = fname_to_string(&FName {
            index: name.index,
            number: name.number,
        })
        .ok_or_else(|| Error("existing reference bone name unavailable".into()))?
        .text;
        // FMeshBone contains VJointPos at +0x10 and ParentIndex at +0x40.
        let parent = word(bone, 0x40)?;
        let parent = if index == 0 && (parent == 0 || parent == u32::MAX) {
            None
        } else {
            Some(u16::try_from(parent).map_err(|_| Error("invalid existing bone parent".into()))?)
        };
        skeleton.push(ReferenceBone {
            name: text,
            parent,
            translation: [
                f32::from_bits(word(bone, 0x20)?),
                f32::from_bits(word(bone, 0x24)?),
                f32::from_bits(word(bone, 0x28)?),
            ],
            rotation_xyzw: [
                f32::from_bits(word(bone, 0x10)?),
                f32::from_bits(word(bone, 0x14)?),
                f32::from_bits(word(bone, 0x18)?),
                f32::from_bits(word(bone, 0x1c)?),
            ],
        });
        names.push(name);
    }
    super::skeleton::validate(&skeleton)?;
    Ok((skeleton, names))
}
