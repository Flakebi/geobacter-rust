pub mod atomic;

use crate::geobacter::intrinsics::*;

pub fn workgroup_barrier() {
    unsafe { geobacter_amdgpu_barrier() }
}
pub fn wavefront_barrier() {
    unsafe { geobacter_amdgpu_wave_barrier() }
}
