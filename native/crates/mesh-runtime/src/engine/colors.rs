//! Native static and skeletal BGRA8 color-stream staging.

use super::{
    abi::vertex_impl,
    construct::{self, Region, retain_bytes},
    native::{ColorBuffer, Method},
};
use crate::{Error, Result};

pub(crate) const SIZE: usize = 0x28;
pub(crate) const IMPLEMENTATION: usize = 0x18;
const DATA: usize = 0x1c;
const STRIDE: usize = 0x20;
const VERTICES: usize = 0x24;
const COLOR_BYTES: usize = size_of::<[u8; 4]>();

pub(crate) fn read(buffer: usize, vertices: usize) -> Result<Vec<u8>> {
    let count = construct::load(buffer + VERTICES)? as usize;
    if count == 0 {
        return Ok(Vec::new());
    }
    if count != vertices || construct::load(buffer + STRIDE)? as usize != COLOR_BYTES {
        return Err(Error("skeletal color stream layout differs".into()));
    }
    let bytes = count
        .checked_mul(COLOR_BYTES)
        .ok_or_else(|| Error("skeletal color size overflow".into()))?;
    let implementation = construct::vertex_storage(buffer + IMPLEMENTATION)?;
    let colors = construct::read_array(
        implementation.address + vertex_impl::ARRAY,
        COLOR_BYTES,
        vertices,
    )?;
    if colors.len() != bytes
        || construct::load(implementation.address + vertex_impl::ARRAY)?
            != construct::load(buffer + DATA)?
    {
        return Err(Error("skeletal color stream is not CPU-readable".into()));
    }
    Ok(colors)
}

/// Installs retained BGRA8 bytes into a constructed, unused native color buffer.
pub(crate) fn fill(
    allocate: Method<ColorBuffer>,
    buffer: Region,
    payload: &[u8],
    vertices: usize,
) -> Result<()> {
    if payload.is_empty() {
        return Ok(());
    }
    if vertices > i32::MAX as usize || vertices.checked_mul(COLOR_BYTES) != Some(payload.len()) {
        return Err(Error(
            "color stream must match the native vertex count".into(),
        ));
    }
    // SAFETY: the caller supplies the constructed static or skeletal color buffer
    // and its matching native allocator. Their implementation vtables differ.
    unsafe { allocate(buffer.address as *mut ColorBuffer) };
    let implementation = construct::vertex_storage(buffer.address + IMPLEMENTATION)?;
    let storage = retain_bytes(payload)?;
    implementation.header(vertex_impl::ARRAY, storage.address, vertices as u32)?;
    buffer.u32(DATA, storage.address as u32)?;
    buffer.u32(STRIDE, COLOR_BYTES as u32)?;
    buffer.u32(VERTICES, vertices as u32)
}
