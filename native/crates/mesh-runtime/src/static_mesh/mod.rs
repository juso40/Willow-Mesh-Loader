//! Prepared static geometry, native population and in-place replacement.

pub(crate) mod bindings;
pub(crate) mod layout;
mod population;
mod prepared;
mod replacement;

pub use prepared::PreparedStaticMesh;
mod storage;
