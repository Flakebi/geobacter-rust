
use crate::geobacter::intrinsics::geobacter_amdgpu_dispatch_ptr;
use crate::geobacter::platform::platform;

pub mod dpp;
pub mod interrupt;
pub mod sync;
pub mod workitem;

// HSA queue dispatch packet, as defined in the HSA specification.
#[doc = "AQL kernel dispatch packet"]
#[repr(C)]
#[derive(Debug, Copy, Clone, Hash)]
pub struct DispatchPacket {
    #[doc = "Packet header. Used to configure multiple packet parameters such as the"]
    #[doc = "packet type. The parameters are described by ::hsa_packet_header_t."]
    pub header: u16,
    #[doc = "Dispatch setup parameters. Used to configure kernel dispatch parameters"]
    #[doc = "such as the number of dimensions in the grid. The parameters are described"]
    #[doc = "by ::hsa_kernel_dispatch_packet_setup_t."]
    pub setup: u16,
    #[doc = "X dimension of work-group, in work-items. Must be greater than 0."]
    pub workgroup_size_x: u16,
    #[doc = "Y dimension of work-group, in work-items. Must be greater than"]
    #[doc = "0. If the grid has 1 dimension, the only valid value is 1."]
    pub workgroup_size_y: u16,
    #[doc = "Z dimension of work-group, in work-items. Must be greater than"]
    #[doc = "0. If the grid has 1 or 2 dimensions, the only valid value is 1."]
    pub workgroup_size_z: u16,
    #[doc = "Reserved. Must be 0."]
    reserved0: u16,
    #[doc = "X dimension of grid, in work-items. Must be greater than 0. Must"]
    #[doc = "not be smaller than `workgroup_size_x`."]
    pub grid_size_x: u32,
    #[doc = "Y dimension of grid, in work-items. Must be greater than 0. If the grid has"]
    #[doc = "1 dimension, the only valid value is 1. Must not be smaller than"]
    #[doc = "`workgroup_size_y`."]
    pub grid_size_y: u32,
    #[doc = "Z dimension of grid, in work-items. Must be greater than 0. If the grid has"]
    #[doc = "1 or 2 dimensions, the only valid value is 1. Must not be smaller than"]
    #[doc = "`workgroup_size_z`."]
    pub grid_size_z: u32,
    #[doc = "Size in bytes of private memory allocation request (per work-item)."]
    pub private_segment_size: u32,
    #[doc = "Size in bytes of group memory allocation request (per work-group). Must not"]
    #[doc = "be less than the sum of the group memory used by the kernel (and the"]
    #[doc = "functions it calls directly or indirectly) and the dynamically allocated"]
    #[doc = "group segment variables."]
    pub group_segment_size: u32,
    #[doc = "Opaque handle to a code object that includes an implementation-defined"]
    #[doc = "executable code for the kernel."]
    pub kernel_object: u64,
    pub kernarg_address: *mut (),
    #[doc = "Reserved. Must be 0."]
    reserved2: u64,
    #[doc = "Opaque signal handle used to indicate completion of the job. The"]
    #[doc = "application can use the special signal handle 0 to indicate that no signal"]
    #[doc = "is used. Also opaque."]
    pub completion_signal: u64,
}

#[inline(always)]
pub fn dispatch_packet() -> &'static DispatchPacket {
    ensure_amdgpu("amdgpu_dispatch_ptr");

    unsafe {
        let ptr = geobacter_amdgpu_dispatch_ptr();
        let ptr: *const DispatchPacket = ptr as *const _;
        &*ptr
    }
}

#[inline(always)]
fn ensure_amdgpu(what: &str) {
    if !platform().is_amdgcn() {
        panic!("AMDGPU device function `{}` called on non-AMDGPU platform",
               what)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test] #[should_panic]
    fn dispatch_packet_ensure_amdgpu() {
        dispatch_packet();
    }
}
