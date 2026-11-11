//! Bounded, engine-independent glTF and GLB mesh decoding.
//!
//! Importers validate local buffers, bake the UModel-compatible axis conversion and
//! preserve original material slots without SDK access or engine allocations.
//! Authored tangent frames are retained through coordinate conversion. Missing
//! tangents use source-space MikkTSpace with an orthogonal fallback for unusable UVs.

/// Lightweight source catalog with document-local asset IDs.
pub mod catalog;
mod coordinates;
mod document;
mod hierarchy;
mod indexing;
mod math;
mod policy;
mod primitive;
mod sections;
mod source;
mod static_mesh;
mod types;

/// Skeletal animation sampling in engine-space local coordinates.
pub mod animation;
/// Classification of explicitly selected static and skeletal mesh nodes.
pub mod kind;
/// Skin decoding and bind-derived reference skeleton reconstruction.
pub mod skin;

pub use coordinates::DESCRIPTION as COORDINATE_CONVENTION;
pub use source::Source;
pub use types::{Geometry, Section, Vertex};

use std::fmt;

/// Default conversion from source meters to engine-space centimeters.
pub const DEFAULT_SCALE: f32 = 100.;

// The game is a 32-bit process with signed native array counts.
pub(crate) const MAX_FILE: usize = i32::MAX as usize;
pub(crate) const MAX_SECTIONS: usize = i32::MAX as usize;
pub(crate) const MAX_ACCESSOR: usize = i32::MAX as usize;

/// An import format, validation or I/O failure before native construction.
#[derive(Debug)]
pub struct Error(
    /// The format restriction or I/O operation that failed.
    pub String,
);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

/// The result of engine-independent mesh decoding.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn fail(message: &str) -> Error {
    Error(message.into())
}

pub(crate) fn warn(warnings: &mut Vec<String>, message: &str) {
    if !warnings.iter().any(|s| s == message) {
        warnings.push(message.into());
    }
}
