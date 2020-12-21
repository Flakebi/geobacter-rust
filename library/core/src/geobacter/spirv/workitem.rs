
#![allow(improper_ctypes)]

use super::*;

#[inline(always)]
pub fn local_invocation_id() -> [u32; 3] {
    assert!(platform().is_spirv());
    extern "C" {
        #[link_name = "llvm.spirv.local.invocation.id"]
        fn liid() -> V3<u32>;
    }
    let v = unsafe { liid() };
    [v.0, v.1, v.2]
}
#[inline(always)]
pub fn global_invocation_id() -> [u32; 3] {
    assert!(platform().is_spirv());
    extern "C" {
        #[link_name = "llvm.spirv.global.invocation.id"]
        fn giid() -> V3<u32>;
    }
    let v = unsafe { giid() };
    [v.0, v.1, v.2]
}
