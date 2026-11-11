//! Serialization of synchronous native mesh operations.

use crate::{Error, Result};
use std::sync::{Mutex, MutexGuard, TryLockError};

static OPERATION: Mutex<()> = Mutex::new(());

/// Serializes native operations without retaining failure state.
pub(crate) fn reserve() -> Result<MutexGuard<'static, ()>> {
    match OPERATION.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::Poisoned(error)) => Ok(error.into_inner()),
        Err(TryLockError::WouldBlock) => Err(Error("native mesh operation is busy".into())),
    }
}
