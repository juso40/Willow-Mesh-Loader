//! Bounded process-memory copies and little-endian word decoding.

use crate::{Error, Result};

pub(crate) const MIN_PROCESS_ADDRESS: usize = 0x1_0000;

/// A copy source that returns owned bytes rather than borrowed engine memory.
pub(crate) trait Reader {
    /// Copies exactly the requested byte range or returns a failure.
    fn read(&self, address: usize, length: usize) -> Result<Vec<u8>>;
}

pub(crate) fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset
        .checked_add(size_of::<u32>())
        .ok_or_else(|| Error("offset overflow".into()))?;
    let bytes = bytes
        .get(offset..end)
        .ok_or_else(|| Error("truncated header".into()))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().map_err(|_| Error("bad word".into()))?,
    ))
}
