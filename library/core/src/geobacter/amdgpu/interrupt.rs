

/// Send an interrupt to the host. This is unsafe because there are details not documented here
/// you must adhere to.
#[inline(always)]
pub unsafe fn send_interrupt(arg0: i32, arg1: u32) {
    unsafe {
        crate::geobacter::intrinsics::geobacter_amdgpu_sendmsg(arg0, arg1);
    }
}
