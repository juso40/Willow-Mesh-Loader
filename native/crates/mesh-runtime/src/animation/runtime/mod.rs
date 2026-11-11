//! Engine-backed animation admission, allocation and publication.

mod bindings;
mod layout;
mod objects;
mod population;
mod replacement;
mod storage;

pub(super) use population::populate_new;
pub(super) use replacement::replace;
