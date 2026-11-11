//! SDK-discovered code anchors and typed decoding of image globals and vtables.
//!
//! Resolution checks image permissions and reviewed relative-call destinations.
//! SDK discovery remains the source of truth without ongoing byte-pattern checks.

use super::{memory::Reader, platform};
use super::{memory::word, patterns::relative_target};
use crate::{Error, Result};
use unrealsdk_rs::scan::GameFn;

/// A reviewed code entry with a stable name and checked image address.
pub(crate) trait CodeEntry: Sync {
    fn entry_name(&self) -> &'static str;
    fn code_address(&self, base: usize) -> Result<usize>;
}

/// One reviewed internal relative call that must reach a known code entry.
pub(crate) struct RelativeCall {
    pub(crate) operand: usize,
    pub(crate) target: &'static dyn CodeEntry,
}

pub(crate) struct CodeAnchor {
    entry: GameFn,
    calls: &'static [RelativeCall],
}

impl CodeAnchor {
    /// Binds one IDA-style pattern with its reviewed internal call targets.
    pub(crate) const fn new(
        name: &'static str,
        pattern: &'static str,
        calls: &'static [RelativeCall],
    ) -> Self {
        Self {
            entry: GameFn::new(name, pattern, &[]),
            calls,
        }
    }

    pub(crate) fn address(&self, base: usize) -> Result<usize> {
        let anchor = self.entry.address().ok_or_else(|| {
            Error(format!(
                "{} signature is missing or ambiguous",
                self.entry.name()
            ))
        })?;
        let length = self
            .calls
            .iter()
            .map(|call| call.operand + size_of::<i32>())
            .max()
            .unwrap_or(1);
        platform::require_region(base, anchor, length, true)?;
        let bytes = if self.calls.is_empty() {
            Vec::new()
        } else {
            platform::Memory.read(anchor, length)?
        };
        for call in self.calls {
            let destination = relative_target(anchor, &bytes, call.operand)?;
            if destination != call.target.code_address(base)? {
                return Err(Error(format!(
                    "{} call target differs from {} discovery",
                    self.entry.name(),
                    call.target.entry_name()
                )));
            }
        }
        Ok(anchor)
    }
}

impl CodeEntry for CodeAnchor {
    fn entry_name(&self) -> &'static str {
        self.entry.name()
    }

    fn code_address(&self, base: usize) -> Result<usize> {
        self.address(base)
    }
}

struct ImageOperand {
    anchor: CodeAnchor,
    operand: usize,
}

impl ImageOperand {
    const fn new(name: &'static str, pattern: &'static str, operand: usize) -> Self {
        Self {
            anchor: CodeAnchor::new(name, pattern, &[]),
            operand,
        }
    }

    fn address(&self, base: usize) -> Result<usize> {
        let operand = self
            .anchor
            .address(base)?
            .checked_add(self.operand)
            .ok_or_else(|| Error("image operand address overflow".into()))?;
        let address = word(&platform::Memory.read(operand, size_of::<u32>())?, 0)? as usize;
        platform::require_region(base, address, size_of::<u32>(), false)?;
        Ok(address)
    }
}

/// One read-only engine state word decoded from a checked code operand.
pub(crate) struct GlobalWord(ImageOperand);

impl GlobalWord {
    pub(crate) const fn new(name: &'static str, pattern: &'static str, operand: usize) -> Self {
        Self(ImageOperand::new(name, pattern, operand))
    }

    /// Resolves one checked global address for the current operation.
    pub(crate) fn resolve(&self, base: usize) -> Result<Global> {
        Ok(Global {
            address: self.0.address(base)?,
        })
    }
}

/// A resolved global whose value remains live rather than cached.
pub(crate) struct Global {
    address: usize,
}

impl Global {
    /// Reads the current word through the operation's checked global address.
    pub(crate) fn read(&self) -> Result<u32> {
        word(&platform::Memory.read(self.address, size_of::<u32>())?, 0)
    }
}

/// One engine vtable or implementation-table address decoded from an operand.
pub(crate) struct VTable(ImageOperand);

impl VTable {
    pub(crate) const fn new(name: &'static str, pattern: &'static str, operand: usize) -> Self {
        Self(ImageOperand::new(name, pattern, operand))
    }

    /// Resolves one checked table address for the current operation.
    pub(crate) fn resolve(&self, base: usize) -> Result<Table> {
        Ok(Table {
            base,
            address: self.0.address(base)?,
        })
    }
}

/// A resolved table identity with checked live slot reads.
pub(crate) struct Table {
    base: usize,
    address: usize,
}

impl Table {
    /// Returns the table identity resolved before native mutation.
    pub(crate) fn address(&self) -> usize {
        self.address
    }

    /// Reads one live virtual method word from a byte offset into the table.
    pub(crate) fn slot(&self, byte_offset: usize) -> Result<u32> {
        let address = self
            .address
            .checked_add(byte_offset)
            .ok_or_else(|| Error("vtable slot address overflow".into()))?;
        platform::require_region(self.base, address, size_of::<u32>(), false)?;
        word(&platform::Memory.read(address, size_of::<u32>())?, 0)
    }
}
