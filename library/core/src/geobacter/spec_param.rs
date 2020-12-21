
//! Support for programmatic "kernel" parameters for affecting codegen. These functions
//! are *constant*, though they may not be used in types (ie as a configurable array
//! length). Instead these are intended to allow LLVM to specialize via constant
//! propagation.
//!

use crate::ops::Fn;
use crate::option::Option;

/// F is just a marker here. It won't be called. Currently, f can't be a closure, but that
/// restriction isn't strictly required here.
/// THIS ASSUMES IDENTICAL HOST/DEVICE ENDIANNESS. Endianness swapping will be handled
/// automatically Later(TM), but that will almost certainly be a breaking change.

#[cfg(not(bootstrap))]
pub fn get<F, R>(_: &F) -> Option<&'static R>
  where F: Fn() -> R,
{
  use crate::geobacter::intrinsics::geobacter_specialization_param;
  unsafe {
    geobacter_specialization_param::<F, R>()
      .get(0)
  }
}
#[cfg(bootstrap)]
pub fn get<F, R>(_: &F) -> Option<&'static R>
  where F: Fn() -> R,
{
  crate::option::Option::None
}
