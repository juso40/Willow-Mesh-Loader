//! Native UE3 mesh encoding, fresh-object population and geometry reload.
//!
//! Engine access requires discovered x86 Windows signatures and an initialized
//! game-thread caller outside loading, garbage collection, saving and teardown.
//! Created objects and partial native failures remain retained until process exit.

/// Absolute skeletal animation encoding, population and replacement.
pub mod animation;
/// Portable static vertex and index encoding.
pub mod encoding;
mod engine;
/// Checked material-section partitioning and native section encoding.
pub mod sections;
/// Skinned payload preparation, native records, population and reload.
pub mod skeletal;
/// Static mesh population and geometry reload.
pub mod static_mesh;

/// Eligibility and preservation policy for native geometry replacement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementPolicy {
    /// Replacement requires a library-created mesh and preserves its creation layout.
    LibraryOwned,
    /// Replacement accepts a loaded mesh and applies the existing-asset restrictions.
    ExistingAsset,
}

/// Maximum native FName length excluding the terminator, in UTF-16 code units.
pub const MAX_NAME_UTF16_UNITS: usize = 1023;
/// Maximum byte length of an engine object or material path.
pub const MAX_OBJECT_PATH_BYTES: usize = i32::MAX as usize;

/// A mesh operation failed before completion.
#[derive(Debug)]
pub struct Error(
    /// Failure details, including retention restrictions after native mutation.
    pub String,
);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// The result of mesh validation, encoding or native engine work.
pub type Result<T> = std::result::Result<T, Error>;
