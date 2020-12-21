use super::*;

/// fn __geobacter_update_dpp<T, const DPP_CTRL: i32, const ROW_MASK: i32, const BANK_MASK: i32,
///                           const BOUND_CTRL: bool>(old: T, src: T) -> T;
#[derive(Default)]
pub struct UpdateDpp;
impl UpdateDpp {
    fn kernel_instance_i32() -> KernelInstanceRef<'static> {
        amdgcn_update_dpp_i32.kernel_instance()
    }
    fn intrinsic<'tcx>(tcx: TyCtxt<'tcx>, t: Ty<'tcx>,
                       instance: ty::Instance<'tcx>)
                       -> Option<KernelInstanceRef<'static>>
    {
        let intrinsic = if t == tcx.types.i32 || t == tcx.types.u32 {
            Self::kernel_instance_i32()
        } else {
            tcx.sess.span_err(tcx.def_span(instance.def_id()),
                              "expected a 32-bit integer type");
            return None;
        };
        Some(intrinsic)
    }
}
impl CustomIntrinsicMirGen for UpdateDpp {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);

        let t = instance.substs
            .types()
            .next()
            .unwrap();

        let intrinsic = Self::intrinsic(tcx, t, instance);

        let param_env = ParamEnv::reveal_all();
        let consts = instance.substs.consts()
            .map(|const_arg| {
                let c = const_arg.eval(tcx, param_env);
                mir::Constant {
                    span: DUMMY_SP,
                    user_ty: None,
                    literal: c,
                }
            })
            .map(Box::new)
            .map(Operand::Constant);

        let args = mir.args_iter()
            .map(Place::from)
            .map(Operand::Move)
            .chain(consts)
            .collect::<Vec<_>>();

        if args.len() != 6 {
            // param types are checked by rustc. we just need to check that the consts
            // are present.
            tcx.sess.span_err(tcx.def_span(instance.def_id()),
                              "expected 4 constant parameters");
            return;
        }

        tcx.call_device_inst_args(mir, move || {
            target_check(tcx)?;
            Some((intrinsic?, args))
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        1
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>) -> &'tcx ty::List<Ty<'tcx>> {
        let n = 0;
        let p = Symbol::intern("T");
        let p = tcx.mk_ty_param(n, p);
        tcx.intern_type_list(&[p, p, ])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        let n = 0;
        let p = Symbol::intern("T");
        tcx.mk_ty_param(n, p)
    }
}
impl IntrinsicName for UpdateDpp {
    const NAME: &'static str = "geobacter_amdgpu_update_dpp_v2";
}
impl fmt::Display for UpdateDpp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::NAME)
    }
}

/// Use of constant generics in ^ crashes the current version of the compiler.
#[derive(Default)]
pub struct UpdateDppWorkaround;
impl CustomIntrinsicMirGen for UpdateDppWorkaround {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     instance: ty::Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);

        let t = instance.substs
            .types()
            .next()
            .unwrap();

        let intrinsic = UpdateDpp::intrinsic(tcx, t, instance);

        let args = mir.args_iter()
            .map(Place::from)
            .map(Operand::Move)
            .collect::<Vec<_>>();

        tcx.call_device_inst_args(mir, move || {
            target_check(tcx)?;
            Some((intrinsic?, args))
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        1
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        let n = 0;
        let p = Symbol::intern("T");
        let p = tcx.mk_ty_param(n, p);
        tcx.intern_type_list(&[p, p, tcx.types.i32, tcx.types.i32,
            tcx.types.i32, tcx.types.bool])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        let n = 0;
        let p = Symbol::intern("T");
        tcx.mk_ty_param(n, p)
    }
}
impl IntrinsicName for UpdateDppWorkaround {
    const NAME: &'static str = "geobacter_amdgpu_update_dpp_v1";
}
impl fmt::Display for UpdateDppWorkaround {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::NAME)
    }
}
