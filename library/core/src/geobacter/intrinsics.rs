
//! These intrinsics are not intended to be used directly.

#![unstable(
    feature = "geobacter_intrinsics",
    reason = "these intrinsics are internal to Geobacter",
    issue = "none"
)]
#![allow(missing_docs)]

use crate::geobacter::kernel::*;
use crate::marker::Sized;
use crate::ops::Fn;

extern "rust-intrinsic" {
    /// Kills the current workitem/thread. Panics on the host, behaviour on specific
    /// accelerators is implementation defined. Note: most accelerators *DO NOT SUPPORT
    /// ANY EXCEPTION HANDLING*. This means, when called, function *DOES NOT RUN DROP
    /// CODE* on such accelerators. Eg, on AMDGPU, calling this function causes the
    /// current workitem to just stop: the workitem is "masked off" for the rest of
    /// the program, so no lane cleanup blocks are run as the wavefront returns up the
    /// call stack.
    ///
    /// For the time being, this issue isn't an overly huge deal; these accelerators
    /// don't (yet) manage system/device resources at all, so at least such things
    /// won't leak from device code.
    ///
    /// Again, on host platforms, this will `panic!(why)`.
    ///
    /// XXX Fix this. Should be possible to transform the exceptions into essentially
    /// returning `Result::Err`.
    pub fn geobacter_suicide(why: &str) -> !;

    pub fn geobacter_kernel_instance<F, Args, Ret>()
        -> &'static [(&'static str, &'static [u8])]
        where F: OptionalKernelFn<Args, Output = Ret> + Sized;
    pub fn geobacter_kernel_codegen_stash<F, Args, Ret>()
        -> &'static usize
        where F: Fn<Args, Output = Ret>;
    pub fn geobacter_specialization_param<F, R>() -> &'static [R]
        where F: Fn() -> R;
}

/// AMDGPU intrinsics
#[cfg(stage2)]
extern "rust-intrinsic" {
    pub fn geobacter_amdgpu_dispatch_ptr() -> *const u8;
    pub fn geobacter_amdgpu_update_dpp_v1<T>(old: T, src: T, dpp_ctrl: i32,
                                             row_mask: i32, bank_mask: i32,
                                             bound_ctrl: bool) -> T;
    pub fn geobacter_amdgpu_barrier();
    pub fn geobacter_amdgpu_wave_barrier();
    pub fn geobacter_amdgpu_sendmsg(_: i32, _: u32);
    pub fn geobacter_amdgpu_readfirstlane(_: u32) -> u32;

    pub fn geobacter_amdgpu_workitem_x_id() -> u32;
    pub fn geobacter_amdgpu_workitem_y_id() -> u32;
    pub fn geobacter_amdgpu_workitem_z_id() -> u32;
    pub fn geobacter_amdgpu_workgroup_x_id() -> u32;
    pub fn geobacter_amdgpu_workgroup_y_id() -> u32;
    pub fn geobacter_amdgpu_workgroup_z_id() -> u32;
}

/// Scoped atomic fences. These are slower workarounds because another patch is
/// required for the proper scopes.
#[cfg(stage2)]
mod atomic_scoped_fences_workarounds {
    use crate::sync::atomic::Ordering;

    pub unsafe fn atomic_scoped_fence_singlethread_acq() {
        crate::sync::atomic::fence(Ordering::Acquire);
    }
    pub unsafe fn atomic_scoped_fence_singlethread_rel() {
        crate::sync::atomic::fence(Ordering::Release);
    }
    pub unsafe fn atomic_scoped_fence_singlethread_acqrel() {
        crate::sync::atomic::fence(Ordering::AcqRel);
    }
    pub unsafe fn atomic_scoped_fence_singlethread_seqcst() {
        crate::sync::atomic::fence(Ordering::SeqCst);
    }
    pub unsafe fn atomic_scoped_fence_wavefront_acq() {
        crate::sync::atomic::fence(Ordering::Acquire);
    }
    pub unsafe fn atomic_scoped_fence_wavefront_rel() {
        crate::sync::atomic::fence(Ordering::Release);
    }
    pub unsafe fn atomic_scoped_fence_wavefront_acqrel() {
        crate::sync::atomic::fence(Ordering::AcqRel);
    }
    pub unsafe fn atomic_scoped_fence_wavefront_seqcst() {
        crate::sync::atomic::fence(Ordering::SeqCst);
    }
    pub unsafe fn atomic_scoped_fence_workgroup_acq() {
        crate::sync::atomic::fence(Ordering::Acquire);
    }
    pub unsafe fn atomic_scoped_fence_workgroup_rel() {
        crate::sync::atomic::fence(Ordering::Release);
    }
    pub unsafe fn atomic_scoped_fence_workgroup_acqrel() {
        crate::sync::atomic::fence(Ordering::AcqRel);
    }
    pub unsafe fn atomic_scoped_fence_workgroup_seqcst() {
        crate::sync::atomic::fence(Ordering::SeqCst);
    }
    pub unsafe fn atomic_scoped_fence_agent_acq() {
        crate::sync::atomic::fence(Ordering::Acquire);
    }
    pub unsafe fn atomic_scoped_fence_agent_rel() {
        crate::sync::atomic::fence(Ordering::Release);
    }
    pub unsafe fn atomic_scoped_fence_agent_acqrel() {
        crate::sync::atomic::fence(Ordering::AcqRel);
    }
    pub unsafe fn atomic_scoped_fence_agent_seqcst() {
        crate::sync::atomic::fence(Ordering::SeqCst);
    }
}
#[cfg(stage2)]
pub use self::atomic_scoped_fences_workarounds::*;
