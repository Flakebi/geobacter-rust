
use std::fmt;
use std::geobacter::kernel::KernelInstanceRef;
#[cfg(any(stage1, stage2))]
use std::geobacter::kernel::OptionalKernelFn;
use std::geobacter::platform::Platform;

use rustc_ast::ast;
use rustc_data_structures::sync::Lrc;
use rustc_hir::def_id::DefId;
use rustc_index::vec::Idx;
use rustc_middle::mir::{self, Operand, Rvalue, Place, CustomIntrinsicMirGen,
                        Local, StatementKind, Statement, Body,
                        SourceInfo, BasicBlockData, TerminatorKind};
use rustc_middle::mir::interpret::{ConstValue, Scalar, Pointer, Allocation,
                                   PointerArithmetic};
use rustc_middle::ty::{self, Const, ConstKind, Instance, TyCtxt, Ty};
use rustc_serialize::Encodable;
use rustc_span::{DUMMY_SP, Symbol};
use rustc_target::abi::Align;

use tracing::debug;

use crate::TyCtxtKernelInstance;
use crate::const_builder::*;
use crate::mir_builder::*;

// These are "safe" because unsafe functions don't implement `std::opts::Fn`
// (for good reasons, but we need them to implement Fn here anyway).
#[cfg(any(stage1, stage2))]
macro_rules! def_id_intrinsic {
    (fn $name:ident($($arg:ident: $arg_ty:ty),*) $(-> $ty:ty)? => $llvm_intrinsic:literal) => (
        #[inline(always)]
        fn $name($($arg: $arg_ty),*) $(-> $ty)? {
            extern "C" {
                #[link_name = $llvm_intrinsic]
                fn $name($($arg: $arg_ty),*) $(-> $ty)?;
            }
            unsafe { $name($($arg),*) }
        }
    )
}

// these three need to be supported always.
pub mod kernel;
pub mod platform;
pub mod specialization_param;
pub mod suicide;

#[cfg(any(stage1, stage2))]
pub mod arch;

pub trait PlatformImplDetail: Send + Sync + 'static {
    /// What platform is this detail for? Used for logging/debugging.
    fn platform() -> &'static str;
    /// If this returns None, then the intrinsic will panic if called.
    fn kernel_instance() -> Option<KernelInstanceRef<'static>>;
}
pub trait IntrinsicName: CustomIntrinsicMirGen + Default + 'static {
    const NAME: &'static str;

    fn insert_into_map<F>(mut map: F)
        where Self: Sized,
              F: for<'a> FnMut(&'a str, Lrc<dyn CustomIntrinsicMirGen>),
    {
        map(Self::NAME, Lrc::new(Self::default()))
    }

    fn check(name: &str) -> Result<(), Lrc<dyn CustomIntrinsicMirGen>>
        where Self: Sized,
    {
        if &Self::NAME == &name {
            Err(Lrc::new(Self::default()))
        } else {
            Ok(())
        }
    }
}

pub fn insert_generic_intrinsics<F>(mut map: F)
    where F: for<'a> FnMut(&'a str, Lrc<dyn CustomIntrinsicMirGen>),
{
    kernel::KernelInstance::insert_into_map(&mut map);
    kernel::KernelContextDataId::insert_into_map(&mut map);
    specialization_param::SpecializationParam::insert_into_map(&mut map);

    #[cfg(any(stage1, stage2))] {
        arch::insert_all_intrinsics(&mut map);
    }
}

pub fn provide(providers: &mut ty::query::Providers) {
    providers.custom_intrinsic_mirgen = custom_intrinsic_mirgen;
    providers.specialization_data = |_, _| { None };
    providers.stubbed_instance = |_, inst| { inst };
}

fn custom_intrinsic_mirgen(tcx: TyCtxt<'_>, def_id: DefId)
    -> Option<Lrc<dyn CustomIntrinsicMirGen>>
{
    let name = tcx.item_name(def_id);
    let name_str = name.as_str();

    fn find(tcx: TyCtxt<'_>, name: &str) -> Result<(), Lrc<dyn CustomIntrinsicMirGen>> {
        kernel::KernelInstance::check(name)?;
        kernel::KernelContextDataId::check(name)?;
        platform::PlatformIntrinsic::check(name)?;
        specialization_param::SpecializationParam::check(name)?;

        #[cfg(any(stage1, stage2))] {
            arch::find_intrinsic(tcx, name)?;
        }
        &tcx;

        <suicide::Suicide<suicide::PanicSuicide>>::check(name)?;
        Ok(())
    }

    match find(tcx, &name_str) {
        Ok(()) => None,
        Err(mirgen) => Some(mirgen),
    }
}
