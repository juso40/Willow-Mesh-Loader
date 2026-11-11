//! ABI-bound native functions, distinct from global words and vtables.
//!
//! Each binding fixes its function-pointer type at declaration. Discovery only
//! selects the reviewed code address and does not infer a calling convention.

use super::{
    abi::ReattachContext,
    discovery::{CodeAnchor, CodeEntry, RelativeCall},
};
use crate::Result;
use std::marker::PhantomData;
use unrealsdk_rs::types::UObject;

macro_rules! receivers {
    ($($name:ident),+ $(,)?) => {
        $(
            /// Opaque engine storage seen through this binding's receiver type.
            #[repr(C)]
            pub(crate) struct $name {
                _opaque: [u8; 0],
            }
        )+
    };
}

receivers!(
    StaticMesh,
    SkeletalMesh,
    StaticLod,
    SkeletalLod,
    PositionBuffer,
    TangentBuffer,
    SkinBuffer,
    ColorBuffer,
    RenderFence,
    AnimSequence,
    AnimSet,
);

/// One no-return engine method with a fixed receiver type.
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) type Method<T> = unsafe extern "thiscall" fn(*mut T);
#[cfg(not(all(windows, target_arch = "x86")))]
pub(crate) type Method<T> = unsafe extern "C" fn(*mut T);
/// `FComponentReattachContext` construction from one live component.
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) type ReattachConstructor =
    unsafe extern "thiscall" fn(*mut ReattachContext, *mut UObject);
#[cfg(not(all(windows, target_arch = "x86")))]
pub(crate) type ReattachConstructor = unsafe extern "C" fn(*mut ReattachContext, *mut UObject);
/// One fence wait with a fixed timeout argument.
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) type FenceWait = unsafe extern "thiscall" fn(*mut RenderFence, u32);
#[cfg(not(all(windows, target_arch = "x86")))]
pub(crate) type FenceWait = unsafe extern "C" fn(*mut RenderFence, u32);
/// One fence completion query.
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) type FencePending = unsafe extern "thiscall" fn(*mut RenderFence) -> u32;
#[cfg(not(all(windows, target_arch = "x86")))]
pub(crate) type FencePending = unsafe extern "C" fn(*mut RenderFence) -> u32;

/// Gestalt component-data initialization from an independent three-word name-array view.
#[cfg(all(windows, target_arch = "x86"))]
pub(crate) type GestaltInitialize =
    unsafe extern "thiscall" fn(*mut UObject, *mut UObject, *const [u32; 3]);
#[cfg(not(all(windows, target_arch = "x86")))]
pub(crate) type GestaltInitialize =
    unsafe extern "C" fn(*mut UObject, *mut UObject, *const [u32; 3]);

/// Cdecl animation codec selection from one complete sequence.
pub(crate) type AnimationCodecLinks = unsafe extern "C" fn(*mut AnimSequence);

mod sealed {
    pub trait Sealed {}
}

/// A reviewed native ABI whose exact function-pointer type is known.
pub(crate) trait NativeAbi: sealed::Sealed + Copy + Send + Sync {
    /// The address belongs to code with this exact reviewed ABI.
    unsafe fn from_address(address: usize) -> Self;
}

impl<T> sealed::Sealed for Method<T> {}
impl<T> NativeAbi for Method<T> {
    unsafe fn from_address(address: usize) -> Self {
        // SAFETY: the caller supplies checked code with this exact method ABI.
        unsafe { std::mem::transmute(address) }
    }
}

macro_rules! native_abi {
    ($($abi:ty),+ $(,)?) => {
        $(
            impl sealed::Sealed for $abi {}
            impl NativeAbi for $abi {
                unsafe fn from_address(address: usize) -> Self {
                    // SAFETY: the caller supplies checked code with this exact ABI.
                    unsafe { std::mem::transmute(address) }
                }
            }
        )+
    };
}

native_abi!(
    ReattachConstructor,
    FenceWait,
    FencePending,
    GestaltInitialize
);
#[cfg(all(windows, target_arch = "x86"))]
native_abi!(AnimationCodecLinks);

/// One signature-discovered function with a fixed callable ABI.
pub(crate) struct NativeFunction<F: NativeAbi> {
    anchor: CodeAnchor,
    abi: PhantomData<F>,
}

impl<F: NativeAbi> NativeFunction<F> {
    /// Binds one reviewed code pattern to the fixed ABI `F`.
    pub(crate) const fn new(
        name: &'static str,
        pattern: &'static str,
        calls: &'static [RelativeCall],
    ) -> Self {
        Self {
            anchor: CodeAnchor::new(name, pattern, calls),
            abi: PhantomData,
        }
    }

    /// Resolves the checked typed entry point before the operation mutates native storage.
    pub(crate) fn resolve(&self, base: usize) -> Result<F> {
        let address = self.anchor.code_address(base)?;
        // SAFETY: the binding declaration fixes F to the reviewed native ABI.
        Ok(unsafe { F::from_address(address) })
    }
}

impl<F: NativeAbi> CodeEntry for NativeFunction<F> {
    fn entry_name(&self) -> &'static str {
        self.anchor.entry_name()
    }

    fn code_address(&self, base: usize) -> Result<usize> {
        self.anchor.code_address(base)
    }
}
