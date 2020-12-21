#![unstable(
    feature = "geobacter",
    reason = "WIP",
    issue = "none"
)]
#![allow(missing_docs)]

#[cfg(stage2)]
pub mod amdgpu;
#[cfg(stage2)]
pub mod cuda;
pub mod spirv;

#[cfg(not(bootstrap))]
pub mod intrinsics;
#[cfg(bootstrap)]
pub mod intrinsics { }

pub mod kernel;
pub mod platform;
pub mod spec_param;
