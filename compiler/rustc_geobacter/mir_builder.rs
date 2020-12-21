use std::geobacter::kernel::KernelInstanceRef;

use tracing::{event, Level};

use rustc_hir::def_id::DefId;
use rustc_hir::lang_items::LangItem;
use rustc_index::vec::Idx;
use rustc_middle::mir::{Constant, Operand, Place, Rvalue};
use rustc_middle::mir;
use rustc_middle::mir::interpret::ConstValue;
use rustc_middle::ty::{self, Const, ConstKind, Instance, InstanceDef, ParamEnv, Ty, TyCtxt};
use rustc_middle::ty::layout::HasTyCtxt;
use rustc_span::{DUMMY_SP, Span};

use crate::const_builder::TyCtxtConstBuilder;
use crate::TyCtxtKernelInstance;

#[inline(always)]
pub fn dummy_source_info() -> mir::SourceInfo {
    mir::SourceInfo {
        span: DUMMY_SP,
        scope: mir::OUTERMOST_SOURCE_SCOPE,
    }
}

pub trait TyCtxtMirBuilder<'tcx>: HasTyCtxt<'tcx>
    where Self: TyCtxtKernelInstance<'tcx> + TyCtxtConstBuilder<'tcx>,
{
    fn const_value_rvalue(&self, src: &mir::SourceInfo,
                          const_val: ConstValue<'tcx>,
                          ty: Ty<'tcx>) -> Rvalue<'tcx> {
        let constant = self.tcx().mk_const(Const {
            ty,
            val: ConstKind::Value(const_val),
        });
        let constant = Constant {
            span: src.span,
            literal: constant,
            user_ty: None,
        };
        let constant = Box::new(constant);
        let constant = Operand::Constant(constant);

        Rvalue::Use(constant)
    }

    fn call_device_inst<F>(&self, mir: &mut mir::Body<'tcx>, f: F)
        where F: FnOnce() -> Option<KernelInstanceRef<'static>>,
    {
        self.call_device_func(mir, move || {
            let k = f()?;
            let instance = self.convert_kernel_instance(k)
                .expect("failed to convert kernel instance to rustc instance");
            Some(instance)
        })
    }
    fn call_device_func<F>(&self, mir: &mut mir::Body<'tcx>, f: F)
        where F: FnOnce() -> Option<Instance<'tcx>>,
    {
        self.redirect_or_panic(mir, || {
            self.mk_static_str_operand(&dummy_source_info(), "Device function called on unexpected platform")
        },
                               move || Some((f()?, vec![])));
    }
    fn call_device_inst_args<F>(&self, mir: &mut mir::Body<'tcx>, f: F)
        where F: FnOnce() -> Option<(KernelInstanceRef<'static>, Vec<Operand<'tcx>>)>,
    {
        self.call_device_func_args(mir, move || {
            let (k, args) = f()?;
            let instance = self.convert_kernel_instance(k)
                .expect("failed to convert kernel instance to rustc instance");
            Some((instance, args))
        })
    }
    fn call_device_func_args<F>(&self, mir: &mut mir::Body<'tcx>, f: F)
        where F: FnOnce() -> Option<(Instance<'tcx>, Vec<Operand<'tcx>>)>,
    {
        self.redirect_or_panic(mir, || {
            self.mk_static_str_operand(&dummy_source_info(), "Device function called on unexpected platform")
        },
                               f);
    }

    /// Either call the instance returned from `f` or insert code to panic.
    fn redirect_or_panic<F, G>(&self, mir: &mut mir::Body<'tcx>,
                               msg: G, f: F)
        where F: FnOnce() -> Option<(Instance<'tcx>, Vec<Operand<'tcx>>)>,
              G: FnOnce() -> Operand<'tcx>,
    {
        fn lang_item(tcx: TyCtxt<'_>, span: Option<Span>,
                     msg: &str, li: LangItem) -> DefId {
            tcx.lang_items().require(li).unwrap_or_else(|s| {
                let msg = format!("{} {}", msg, s);
                match span {
                    Some(span) => tcx.sess.span_fatal(span, &msg[..]),
                    None => tcx.sess.fatal(&msg[..]),
                }
            })
        }

        let tcx = self.tcx();
        let source_info = dummy_source_info();

        let mut bb = mir::BasicBlockData {
            statements: Vec::new(),
            terminator: Some(mir::Terminator {
                source_info: source_info.clone(),
                kind: mir::TerminatorKind::Return,
            }),

            is_cleanup: false,
        };

        let (callee, args, term_kind) = match f() {
            Some((instance, args)) => {
                (instance, args, mir::TerminatorKind::Return)
            }
            None => {
                // call `panic` from `libcore`
                let item = LangItem::Panic;

                let def_id = lang_item(tcx, None, "", item);
                let mut instance = Instance::mono(tcx, def_id);
                instance.def = InstanceDef::ReifyShim(def_id);

                (instance, vec![msg(), ], mir::TerminatorKind::Unreachable)
            }
        };

        event!(Level::DEBUG, "mirgen intrinsic into {}", callee);

        let success = mir::BasicBlock::new(mir.basic_blocks().next_index().index() + 1);
        let fn_ty = callee.ty(tcx, ParamEnv::reveal_all());
        bb.terminator.as_mut()
            .unwrap()
            .kind = mir::TerminatorKind::Call {
            func: tcx.mk_const_op(&source_info,
                                  *ty::Const::zero_sized(tcx, fn_ty)),
            args,
            destination: Some((Place::return_place(), success)),
            cleanup: None,
            from_hir_call: false,
            fn_span: DUMMY_SP,
        };
        mir.basic_blocks_mut().push(bb);
        let bb = mir::BasicBlockData {
            statements: Vec::new(),
            terminator: Some(mir::Terminator {
                source_info: source_info.clone(),
                kind: term_kind,
            }),

            is_cleanup: false,
        };
        mir.basic_blocks_mut().push(bb);
    }
}

impl<'tcx, T> TyCtxtMirBuilder<'tcx> for T
    where T: HasTyCtxt<'tcx>,
{}
