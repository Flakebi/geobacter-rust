use crate::abi::call::{ArgAbi, FnAbi};
use crate::abi::{HasDataLayout, LayoutOf, TyAndLayout, TyAndLayoutMethods};

// X X X: We use globals and no function params, for a variety of reasons, making
// this module dubious at best.

fn classify_ret_ty<'a, Ty, C>(_cx: &C, ret: &mut ArgAbi<'a, Ty>)
    where Ty: TyAndLayoutMethods<'a, C> + Copy,
          C: LayoutOf<Ty = Ty, TyAndLayout = TyAndLayout<'a, Ty>> + HasDataLayout
{
    // Technically, 16bit and smaller is supported if the correct capabilities are
    // requested.
    ret.extend_integer_width_to(32);
}

fn classify_arg_ty<'a, Ty, C>(_cx: &C, arg: &mut ArgAbi<'a, Ty>)
    where Ty: TyAndLayoutMethods<'a, C> + Copy,
          C: LayoutOf<Ty = Ty, TyAndLayout = TyAndLayout<'a, Ty>> + HasDataLayout
{
    arg.extend_integer_width_to(32);
}

pub fn compute_abi_info<'a, Ty, C>(cx: &C, fty: &mut FnAbi<'a, Ty>)
    where Ty: TyAndLayoutMethods<'a, C> + Copy,
          C: LayoutOf<Ty = Ty, TyAndLayout = TyAndLayout<'a, Ty>> + HasDataLayout
{
    if !fty.ret.is_ignore() {
        classify_ret_ty(cx, &mut fty.ret);
    }

    for arg in &mut fty.args {
        if arg.is_ignore() {
            continue;
        }
        classify_arg_ty(cx, arg);
    }
}
