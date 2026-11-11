//! Copy-only process-memory access. Never create borrowed slices over native buffers.

use super::memory::{MIN_PROCESS_ADDRESS, Reader};
use crate::{Error, Result};

pub(crate) struct Memory;

impl Reader for Memory {
    fn read(&self, address: usize, length: usize) -> Result<Vec<u8>> {
        if address < MIN_PROCESS_ADDRESS
            || length > isize::MAX as usize
            || address.checked_add(length).is_none()
        {
            return Err(Error("invalid or excessive process-memory read".into()));
        }
        read_memory(address, length)
    }
}

#[cfg(all(windows, target_arch = "x86"))]
mod os {
    use super::*;
    use std::ffi::c_void;
    const MEM_COMMIT: u32 = 0x1000;
    const PAGE_GUARD: u32 = 0x100;
    const PAGE_PROTECTION_MASK: u32 = 0xff;
    const PAGE_READONLY: u32 = 0x02;
    const PAGE_READWRITE: u32 = 0x04;
    const PAGE_WRITECOPY: u32 = 0x08;
    const PAGE_EXECUTE: u32 = 0x10;
    const PAGE_EXECUTE_READ: u32 = 0x20;
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GetCurrentProcess() -> *mut c_void;
        fn VirtualQuery(address: *const c_void, info: *mut MemoryInfo, length: usize) -> usize;
        fn ReadProcessMemory(
            process: *mut c_void,
            address: *const c_void,
            buffer: *mut c_void,
            size: usize,
            read: *mut usize,
        ) -> i32;
    }

    #[repr(C)]
    struct MemoryInfo {
        base: *mut c_void,
        allocation_base: *mut c_void,
        allocation_protect: u32,
        region_size: usize,
        state: u32,
        protect: u32,
        kind: u32,
    }

    pub(crate) fn require_region(
        base: usize,
        address: usize,
        length: usize,
        executable: bool,
    ) -> Result<()> {
        let mut info = MemoryInfo {
            base: std::ptr::null_mut(),
            allocation_base: std::ptr::null_mut(),
            allocation_protect: 0,
            region_size: 0,
            state: 0,
            protect: 0,
            kind: 0,
        };
        // SAFETY: VirtualQuery writes the correctly sized PE32 MemoryInfo output.
        // it does not dereference the query address as a Rust pointer.
        let count = unsafe {
            VirtualQuery(
                address as *const c_void,
                &mut info,
                std::mem::size_of::<MemoryInfo>(),
            )
        };
        let end = address
            .checked_add(length)
            .ok_or_else(|| Error("memory region overflow".into()))?;
        let region_end = (info.base as usize)
            .checked_add(info.region_size)
            .ok_or_else(|| Error("memory region overflow".into()))?;
        let protection = info.protect & PAGE_PROTECTION_MASK;
        let permitted = if executable {
            matches!(
                protection,
                PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY
            )
        } else {
            matches!(
                protection,
                PAGE_READONLY
                    | PAGE_READWRITE
                    | PAGE_WRITECOPY
                    | PAGE_EXECUTE_READ
                    | PAGE_EXECUTE_READWRITE
                    | PAGE_EXECUTE_WRITECOPY
            )
        };
        if count != std::mem::size_of::<MemoryInfo>()
            || info.allocation_base as usize != base
            || info.state != MEM_COMMIT
            || info.protect & PAGE_GUARD != 0
            || !permitted
            || address < info.base as usize
            || end > region_end
        {
            return Err(Error("discovered image memory protection mismatch".into()));
        }
        Ok(())
    }

    pub(crate) fn read_memory(address: usize, length: usize) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Error("process-memory copy allocation failed".into()))?;
        bytes.resize(length, 0);
        let mut count = 0;
        // SAFETY: only the OS reads the supplied foreign address. Our output buffer
        // is valid for `length`. No Rust reference is made to game-owned memory.
        let success = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                address as *const c_void,
                bytes.as_mut_ptr().cast(),
                length,
                &mut count,
            )
        };
        if success == 0 || count != length {
            return Err(Error(format!(
                "cannot copy {length} bytes at 0x{address:08x}"
            )));
        }
        Ok(bytes)
    }

    pub(crate) fn image_base() -> Result<usize> {
        // SAFETY: null module requests the process executable, already mapped by Windows.
        let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
        if base == 0 {
            return Err(Error("executable module unavailable".into()));
        }
        Ok(base)
    }
}

#[cfg(not(all(windows, target_arch = "x86")))]
mod os {
    use super::*;

    fn unsupported<T>() -> Result<T> {
        Err(Error(
            "native mesh operations require 32-bit Windows".into(),
        ))
    }

    pub(crate) fn image_base() -> Result<usize> {
        unsupported()
    }

    pub(crate) fn read_memory(_address: usize, _length: usize) -> Result<Vec<u8>> {
        unsupported()
    }

    pub(crate) fn require_region(
        _base: usize,
        _address: usize,
        _length: usize,
        _executable: bool,
    ) -> Result<()> {
        unsupported()
    }
}

pub(crate) use os::{image_base, read_memory, require_region};
