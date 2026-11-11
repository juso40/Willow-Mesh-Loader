//! Bounded engine-storage writes, object lookup and retained native ownership.
//!
//! SDK constructors and reviewed layouts define fresh storage. Replacement
//! retains existing objects and installs fresh storage after render release.

use super::{
    abi::{self, WORD_BYTES, vertex_impl},
    discovery::Table,
    execution,
    memory::{Reader, word},
    platform,
};
use crate::{Error, Result};
use unrealsdk_rs::{
    flags::object_flags::*,
    objects::{
        UBuffer, class_default_object, find_class, find_object, object_class, object_flags,
        object_path_name, set_object_flags,
    },
    types::{UClass, UObject},
};

/// Flags that mark an object unusable as a new reference target.
pub(crate) const DEAD: u64 =
    RF_PENDING_KILL | RF_UNREACHABLE | RF_BEGIN_DESTROYED | RF_FINISH_DESTROYED;

/// Bounded write view over one retained engine allocation or validated object.
#[derive(Clone, Copy)]
pub(crate) struct Region {
    /// Start address of the owned storage.
    pub(crate) address: usize,
    /// Owned byte length. Every write is bounds-checked against it.
    pub(crate) length: usize,
}

impl Region {
    /// Copies bytes into the region, refusing writes past its owned length.
    pub(crate) fn write(self, offset: usize, bytes: &[u8]) -> Result<()> {
        if offset
            .checked_add(bytes.len())
            .is_none_or(|end| end > self.length)
        {
            return Err(Error(
                "native construction write exceeds owned region".into(),
            ));
        }
        // SAFETY: private constructor region/engine allocation, bounded above.
        // Only this synchronous game-thread operation writes it. Existing mesh
        // objects are validated before use. Cooked buffer contents are never written.
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                (self.address + offset) as *mut u8,
                bytes.len(),
            );
        }
        Ok(())
    }

    /// Writes one little-endian u32.
    pub(crate) fn u32(self, offset: usize, value: u32) -> Result<()> {
        self.write(offset, &value.to_le_bytes())
    }

    /// Writes one f32 in its little-endian bit pattern.
    pub(crate) fn f32(self, offset: usize, value: f32) -> Result<()> {
        self.u32(offset, value.to_bits())
    }

    /// Stamps a three-word UE3 array header (data, count, capacity).
    ///
    /// No allocation or callback-producing work may happen between these
    /// stores. Every pointee is filled and retained before attachment.
    pub(crate) fn header(self, offset: usize, data: usize, count: u32) -> Result<()> {
        self.u32(offset, data as u32)?;
        self.u32(offset + abi::ARRAY_COUNT, count)?;
        self.u32(offset + abi::ARRAY_CAPACITY, count)
    }
}

/// Allocates fresh GMalloc-backed storage and hands ownership to the engine.
///
/// Detached or partial allocations stay retained on error instead of being
/// freed from Rust. Engine OOM/SEH is not recoverable here.
pub(crate) fn retain_allocation(size: usize) -> Result<Region> {
    let buffer =
        UBuffer::alloc(size).ok_or_else(|| Error("engine buffer allocation failed".into()))?;
    let address = buffer.as_mut_ptr() as usize;
    // Explicit native ownership handoff. The SDK allocator is GMalloc-backed.
    // Rust never owns these bytes again.
    std::mem::forget(buffer);
    Ok(Region {
        address,
        length: size,
    })
}

/// Allocates fresh engine-owned storage and fills it in one step.
pub(crate) fn retain_bytes(data: &[u8]) -> Result<Region> {
    let region = retain_allocation(data.len())?;
    region.write(0, data)?;
    Ok(region)
}

/// Copies a bounded process-memory snapshot into owned Rust bytes.
pub(crate) fn read(address: usize, size: usize) -> Result<Vec<u8>> {
    platform::Memory.read(address, size)
}

/// Loads one little-endian word from process memory.
pub(crate) fn load(address: usize) -> Result<u32> {
    word(&read(address, WORD_BYTES)?, 0)
}

/// Copies a checked native array without borrowing engine storage.
pub(crate) fn read_array(address: usize, element_size: usize, maximum: usize) -> Result<Vec<u8>> {
    let data = load(address)? as usize;
    let count = load(address + abi::ARRAY_COUNT)? as usize;
    let capacity = load(address + abi::ARRAY_CAPACITY)? as usize;
    if count > maximum
        || capacity < count
        || capacity > i32::MAX as usize
        || (count != 0 && data == 0)
    {
        return Err(Error("invalid or unsupported native array header".into()));
    }
    if count == 0 {
        return Ok(Vec::new());
    }
    let size = count
        .checked_mul(element_size)
        .ok_or_else(|| Error("native array size overflow".into()))?;
    read(data, size)
}

/// Validates the existing LOD pointer array and matching LOD-info count.
pub(crate) fn verify_lods(mesh: usize, models: usize, info: usize, lod_size: usize) -> Result<()> {
    let pointers = read_array(mesh + models, WORD_BYTES, i32::MAX as usize)?;
    let count = pointers.len() / WORD_BYTES;
    if count == 0
        || load(mesh + info + abi::ARRAY_COUNT)? as usize != count
        || (load(mesh + info + abi::ARRAY_CAPACITY)? as usize) < count
        || load(mesh + info + abi::ARRAY_CAPACITY)? > i32::MAX as u32
        || load(mesh + info)? == 0
    {
        return Err(Error(
            "existing mesh requires matching nonempty native LOD arrays".into(),
        ));
    }
    for pointer in pointers.chunks_exact(WORD_BYTES) {
        let address = u32::from_le_bytes(
            pointer
                .try_into()
                .map_err(|_| Error("invalid LOD pointer".into()))?,
        ) as usize;
        if address == 0 {
            return Err(Error("existing mesh has a null LOD".into()));
        }
        read(address, lod_size)?;
    }
    Ok(())
}

/// Returns an allocated vertex implementation, refusing a failed native allocation.
pub(crate) fn vertex_storage(pointer_field: usize) -> Result<Region> {
    let address = load(pointer_field)? as usize;
    if address == 0 {
        return Err(Error("native vertex buffer allocation failed".into()));
    }
    Ok(Region {
        address,
        length: vertex_impl::SIZE,
    })
}

/// Copies one native box/sphere bounds record without borrowing engine storage.
pub(crate) fn read_bounds(address: usize) -> Result<crate::encoding::Bounds> {
    let mut bounds = crate::encoding::Bounds {
        origin: [0.; 3],
        box_extent: [0.; 3],
        sphere_radius: 0.,
    };
    let count = bounds.origin.len() + bounds.box_extent.len() + 1;
    let bytes = read(address, count * size_of::<f32>())?;
    for (scalar, bytes) in bounds
        .origin
        .iter_mut()
        .chain(bounds.box_extent.iter_mut())
        .chain(std::iter::once(&mut bounds.sphere_radius))
        .zip(bytes.chunks_exact(size_of::<f32>()))
    {
        *scalar = f32::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| Error("invalid bounds scalar".into()))?,
        );
    }
    Ok(bounds)
}

/// Validates the initialized read-only caller context and returns the image base.
pub(crate) fn admitted_base() -> Result<usize> {
    if !unrealsdk_rs::is_initialized() {
        return Err(Error("unrealsdk is not initialized".into()));
    }
    let base = platform::image_base()?;
    execution::require_safe_context(base)?;
    Ok(base)
}

/// Resolves a required engine class by name.
pub(crate) fn require_class(name: &str) -> Result<*mut UClass> {
    find_class(name).ok_or_else(|| Error(format!("{name} unavailable")))
}

/// Resolves an exact-class, live, non-template destination with native default dispatch.
/// Fresh population additionally requires all supplied storage headers to be empty.
pub(crate) fn fresh_object(
    path: &str,
    class_name: &str,
    size: usize,
    arrays: &[usize],
) -> Result<Region> {
    if path.is_empty() || path.contains('\0') || path.len() > crate::MAX_OBJECT_PATH_BYTES {
        return Err(Error(
            "expected a nonempty full object path without NUL".into(),
        ));
    }
    let class = require_class(class_name)?;
    let object = find_object(Some(class), path)
        .ok_or_else(|| Error(format!("destination not loaded: {path}")))?;
    let flags = flags_of("destination", object)?;
    verify_live("destination", flags)?;
    if object_class(object) != Some(class)
        || flags & (RF_CLASS_DEFAULT_OBJECT | RF_ARCHETYPE_OBJECT) != 0
    {
        return Err(Error(format!(
            "fresh population requires a non-template exact {class_name}"
        )));
    }
    let default = class_default_object(class)
        .ok_or_else(|| Error(format!("{class_name} default object unavailable")))?;
    if object == default || load(object as usize)? != load(default as usize)? {
        return Err(Error(
            "destination native dispatch differs from its class default".into(),
        ));
    }
    read(object as usize, size)?;
    let region = Region {
        address: object as usize,
        length: size,
    };
    for &offset in arrays {
        require_empty_array(region.address + offset)?;
    }
    Ok(region)
}

/// Refuses installation over any allocated or counted native array storage.
pub(crate) fn require_empty_array(address: usize) -> Result<()> {
    if read(address, abi::ARRAY_HEADER_BYTES)?
        .iter()
        .any(|&byte| byte != 0)
    {
        return Err(Error("fresh population refuses initialized native storage. Existing assets use replacement instead".into()));
    }
    Ok(())
}

/// Returns the canonical SDK path name of a live object.
pub(crate) fn object_path(subject: &str, object: *mut UObject) -> Result<String> {
    object_path_name(object)
        .and_then(|name| name.to_string())
        .ok_or_else(|| Error(format!("{subject} path unavailable")))
}

/// Reads an object's flags, naming the object on failure.
pub(crate) fn flags_of(subject: &str, object: *mut UObject) -> Result<u64> {
    object_flags(object).ok_or_else(|| Error(format!("{subject} flags unavailable")))
}

/// Validates that an object is not pending destruction or unreachable.
pub(crate) fn verify_live(subject: &str, flags: u64) -> Result<()> {
    if flags & DEAD != 0 {
        return Err(Error(format!(
            "{subject} is being destroyed or unreachable"
        )));
    }
    Ok(())
}

/// Validates a constructor-installed vtable word against its resolved identity.
pub(crate) fn verify_vtable(subject: &str, found: u32, expected: &Table) -> Result<()> {
    if found as usize != expected.address() {
        return Err(Error(format!("{subject} vtable mismatch")));
    }
    Ok(())
}

/// Validates a virtual method slot against a checked code entry.
pub(crate) fn verify_dispatch(subject: &str, found: u32, expected: usize) -> Result<()> {
    if found as usize != expected {
        return Err(Error(format!("{subject} dispatch mismatch")));
    }
    Ok(())
}

/// Resolves material slots in request order without constructing diagnostic paths.
pub(crate) fn resolve_materials(paths: &[&str]) -> Result<Vec<*mut UObject>> {
    let class = require_class("Engine.MaterialInterface")?;
    paths
        .iter()
        .map(|path| {
            find_object(Some(class), path)
                .ok_or_else(|| Error(format!("material not loaded: {path}")))
        })
        .collect()
}

/// Roots each selected material while preserving all other flag bits.
///
/// Flags are re-read per slot, including duplicate references, so stale
/// saved words are never restored when slots share one material object.
pub(crate) fn retain_materials(materials: &[*mut UObject]) -> Result<()> {
    for &material in materials {
        retain_object("material", material)?;
    }
    Ok(())
}

/// Roots a live object while preserving every existing flag bit.
pub(crate) fn retain_object(subject: &str, object: *mut UObject) -> Result<()> {
    let flags = flags_of(subject, object)?;
    verify_live(subject, flags)?;
    if flags & RF_ROOT_SET == 0 && !set_object_flags(object, flags | RF_ROOT_SET) {
        return Err(Error(format!("cannot retain {subject}")));
    }
    Ok(())
}
