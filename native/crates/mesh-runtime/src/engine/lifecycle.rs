//! Resolved component reattachment and issued render-fence operations.

use super::{
    abi::ReattachContext,
    discovery::Global,
    native::{FencePending, FenceWait, Method, NativeFunction, ReattachConstructor, RenderFence},
    signatures::*,
};
use crate::{Error, Result};
use unrealsdk_rs::types::UObject;

/// Native reload dependencies resolved before allocation or detachment.
pub(crate) struct Lifecycle<T> {
    constructor: ReattachConstructor,
    destructor: Method<ReattachContext>,
    release: Method<T>,
    wait: FenceWait,
    pending: FencePending,
    render_thread_status: Global,
}

impl<T> Lifecycle<T> {
    pub(crate) fn resolve(base: usize, release: &NativeFunction<Method<T>>) -> Result<Self> {
        Ok(Self {
            constructor: REATTACH_CONTEXT_CONSTRUCTOR.resolve(base)?,
            destructor: REATTACH_CONTEXT_DESTRUCTOR.resolve(base)?,
            release: release.resolve(base)?,
            wait: FENCE_WAIT.resolve(base)?,
            pending: FENCE_PENDING.resolve(base)?,
            render_thread_status: RENDER_THREAD_STATUS.resolve(base)?,
        })
    }

    pub(crate) fn require_healthy_render_thread(&self) -> Result<()> {
        if self.render_thread_status.read()? == 0 {
            return Err(Error("render thread is not healthy".into()));
        }
        Ok(())
    }

    // The three-word context stays at a stable address until both dispatches return.
    pub(crate) fn detach(&self, context: &mut ReattachContext, component: usize) {
        // SAFETY: the caller validated the live component and its native dispatch.
        // The correctly sized context remains owned until reattachment completes.
        unsafe { (self.constructor)(context, component as *mut UObject) };
    }

    pub(crate) fn reattach(&self, context: &mut ReattachContext) {
        // SAFETY: the caller supplies the same stable context after resource initialization.
        unsafe { (self.destructor)(context) };
    }

    pub(crate) fn release_and_wait(&self, mesh: *mut T, fence_offset: usize) -> Result<()> {
        let fence = (mesh as usize)
            .checked_add(fence_offset)
            .ok_or_else(|| Error("render fence address overflow".into()))?
            as *mut RenderFence;
        // Component removals precede resource releases on the same render queue.
        // SAFETY: the caller validated the owned mesh, matching release binding and
        // embedded fence layout. Mesh storage and detached contexts remain retained.
        unsafe {
            (self.release)(mesh);
            (self.wait)(fence, 0);
            if (self.pending)(fence) != 0 {
                return Err(Error("issued mesh release fence did not complete".into()));
            }
        }
        self.require_healthy_render_thread()
    }
}
