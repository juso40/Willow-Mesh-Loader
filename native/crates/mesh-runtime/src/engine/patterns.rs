//! Checked relative-call decoding for reviewed internal dependencies.

use super::memory::word;
use crate::{Error, Result};

pub(super) fn relative_target(anchor: usize, bytes: &[u8], operand: usize) -> Result<usize> {
    let displacement = word(bytes, operand)? as i32;
    anchor
        .checked_add(operand)
        .and_then(|address| address.checked_add(size_of::<i32>()))
        .and_then(|next| next.checked_add_signed(displacement as isize))
        .ok_or_else(|| Error("relative call address overflow".into()))
}
