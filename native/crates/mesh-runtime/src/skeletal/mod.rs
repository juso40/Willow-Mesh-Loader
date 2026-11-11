//! Skinned geometry preparation, native record staging and engine mesh ownership.

pub(crate) mod bindings;
mod chunks;
/// Packed Gestalt concatenation and native half-precision UV conversion.
pub mod gestalt;
mod gestalt_update;
pub(crate) mod layout;
mod payload;
mod population;
mod prepared;
mod readback;
/// Native bone, vertex, chunk and LOD record encoding.
pub mod records;
mod reference;
mod replacement;
pub(crate) mod skeleton;
mod storage;
mod types;
mod weights;

pub use gestalt_update::PartRange as GestaltPartRange;
pub use payload::{encode, skinned, skinned_with_uvs};
pub use prepared::{PreparedSkeletalMesh, SkinnedGeometryInput};
pub use reference::get_reference_skeleton;
pub use skeleton::{bind_to_template, bone_closure};
pub use types::{ReferenceBone, SkinChunk, SkinPayload, SkinWeights, SkinnedMesh};
pub use weights::Influences;

/// Full-XYZ, full-precision single-UV GPU vertex stride in bytes.
pub const GPU_VERTEX_STRIDE: usize = 36;
/// Maximum shader bone-palette size per GPU chunk.
pub const MAX_PALETTE_BONES: usize = 75;
/// Maximum number of reference bones supported by byte-sized bone ordinals.
pub const MAX_REFERENCE_BONES: usize = u8::MAX as usize + 1;
/// Maximum reference-bone FName length in UTF-16 code units.
pub const MAX_BONE_NAME_UNITS: usize = crate::MAX_NAME_UTF16_UNITS;

fn failure(message: &str) -> crate::Error {
    crate::Error(message.into())
}
