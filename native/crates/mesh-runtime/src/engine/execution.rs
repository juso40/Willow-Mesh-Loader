//! Engine-state admission outside collection, saving and destruction.
//!
//! Game-thread execution is a caller contract rather than a runtime check.
//! State reads do not exclude reentrant engine calls or establish a lifetime
//! lease. Native operations still require a safe synchronous caller.

use super::signatures::*;
use crate::{Error, Result};

pub(crate) fn require_safe_context(base: usize) -> Result<()> {
    for state in [
        &GARBAGE_COLLECTING,
        &SAVING_PACKAGE,
        &INCREMENTAL_PURGING,
        &PURGE_REQUIRED,
        &PURGING_OBJECT,
    ] {
        if state.resolve(base)?.read()? != 0 {
            return Err(Error(
                "engine collection, saving or destruction is active".into(),
            ));
        }
    }
    Ok(())
}
