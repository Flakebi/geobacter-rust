
use super::*;

use rustc_middle::ty::ParamEnv;
use rustc_span::symbol::Symbol;

pub mod amdgpu;
pub mod spirv;

pub fn insert_all_intrinsics<F>(mut map: F)
    where F: for<'a> FnMut(&'a str, Lrc<dyn CustomIntrinsicMirGen>),
{
    amdgpu::insert_all_intrinsics(&mut map);
    spirv::insert_all_intrinsics(&mut map);
}

pub fn find_intrinsic(tcx: TyCtxt<'_>, name: &str)
    -> Result<(), Lrc<dyn CustomIntrinsicMirGen>>
{
    amdgpu::find_intrinsic(tcx, name)?;
    spirv::find_intrinsic(tcx, name)?;

    Ok(())
}
