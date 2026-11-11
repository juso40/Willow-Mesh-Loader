//! Bounded source reads and validation of only the consumed accessor ranges.

mod parsing;
pub(crate) use parsing::{Extensions, parse};

/// Borrowed buffers admitted for one selected payload.
pub(crate) struct Buffers<'a> {
    pub(crate) data: Vec<Option<&'a [u8]>>,
}

impl Buffers<'_> {
    pub(crate) fn get(&self, index: usize) -> Option<&[u8]> {
        self.data.get(index).copied().flatten()
    }
}

use crate::{Error, MAX_ACCESSOR, Result, fail, policy::GLB_PADDING_BYTES};
use gltf::accessor::{DataType, Dimensions};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path},
};

pub(crate) fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = std::fs::metadata(path).map_err(|e| Error(e.to_string()))?;
    if !metadata.is_file() {
        return Err(fail("asset/buffer must be a regular file"));
    }
    if metadata.len() > limit as u64 {
        return Err(fail("asset/buffer exceeds the supported byte length"));
    }
    let file =
        File::open(path).map_err(|e| Error(format!("cannot open {}: {e}", path.display())))?;
    let mut bytes = Vec::new();
    file.take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| Error(e.to_string()))?;
    if bytes.len() > limit {
        return Err(fail("asset exceeds the signed 32-bit buffer size"));
    }
    Ok(bytes)
}
pub(crate) fn external(root: &Path, uri: &str) -> Result<std::path::PathBuf> {
    // Deliberately restricted URI subset: no URL/data URI/percent decoding,
    // Windows separators/drives, absolute paths, traversal or symlink escapes.
    if uri.is_empty()
        || uri.contains([':', '\\', '%', '?', '#', '\0'])
        || Path::new(uri)
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(fail(
            "buffer URI must be a plain relative path without traversal/URL/percent encoding",
        ));
    }
    let path = root
        .join(uri)
        .canonicalize()
        .map_err(|e| Error(format!("buffer path: {e}")))?;
    if !path.starts_with(root) {
        return Err(fail("external buffer escapes asset directory"));
    }
    Ok(path)
}

pub(crate) fn buffer(asset: &gltf::Gltf, root: &Path, buffer: gltf::Buffer<'_>) -> Result<Vec<u8>> {
    let data = match buffer.source() {
        gltf::buffer::Source::Bin => asset
            .blob
            .as_ref()
            .ok_or_else(|| fail("GLB BIN chunk missing"))?
            .clone(),
        gltf::buffer::Source::Uri(uri) => bounded_read(&external(root, uri)?, buffer.length())?,
    };
    if data.len() < buffer.length()
        || data.len() > buffer.length().saturating_add(GLB_PADDING_BYTES)
    {
        return Err(fail(
            "buffer length does not match declaration (apart from GLB padding)",
        ));
    }
    Ok(data)
}

pub(crate) fn typed(a: &gltf::Accessor<'_>, data: DataType, dimensions: Dimensions) -> Result<()> {
    if a.data_type() != data || a.dimensions() != dimensions || a.normalized() {
        return Err(fail(
            "unsupported attribute component type/dimension/normalization",
        ));
    }
    Ok(())
}

pub(crate) fn validate_accessor(a: &gltf::Accessor<'_>, data: &[u8]) -> Result<()> {
    if a.sparse().is_some() {
        return Err(fail("sparse accessors are not supported"));
    }
    if a.count() == 0 || a.count() > MAX_ACCESSOR {
        return Err(fail(
            "accessor count exceeds the nonempty signed native array range",
        ));
    }
    let view = a
        .view()
        .ok_or_else(|| fail("accessor has no buffer view"))?;
    let stride = view.stride().unwrap_or(a.size());
    if stride < a.size()
        || !stride.is_multiple_of(a.data_type().size())
        || !a.offset().is_multiple_of(a.data_type().size())
    {
        return Err(fail("invalid accessor stride/alignment"));
    }
    let end = a
        .count()
        .checked_sub(1)
        .and_then(|n| n.checked_mul(stride))
        .and_then(|n| n.checked_add(a.size()))
        .and_then(|n| n.checked_add(a.offset()))
        .ok_or_else(|| fail("accessor range overflow"))?;
    let view_end = view
        .offset()
        .checked_add(view.length())
        .ok_or_else(|| fail("view range overflow"))?;
    if !view.offset().is_multiple_of(a.data_type().size()) {
        return Err(fail("misaligned buffer view"));
    }
    if end > view.length() || view_end > data.len() {
        return Err(fail("accessor/view exceeds actual buffer bytes"));
    }
    Ok(())
}
