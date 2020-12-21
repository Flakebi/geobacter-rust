
use super::*;
use crate::codec::GeobacterEncoder;

use rustc_middle::ty::print::with_no_trimmed_paths;

#[derive(Default)]
pub struct KernelInstance;
impl KernelInstance {
    fn inner_ret_ty<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.mk_tup([
            tcx.mk_static_str(),
            tcx.mk_imm_ref(tcx.lifetimes.re_static,
                           tcx.mk_slice(tcx.types.u8))
        ].iter())
    }
}
impl CustomIntrinsicMirGen for KernelInstance {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        let source_info = dummy_source_info();

        let mut bb = mir::BasicBlockData {
            statements: Vec::new(),
            terminator: Some(mir::Terminator {
                source_info: source_info.clone(),
                kind: mir::TerminatorKind::Return,
            }),

            is_cleanup: false,
        };

        let ret = mir::Place::return_place();
        let local_ty = instance.substs
            .types()
            .next()
            .unwrap();

        let instance = tcx.extract_opt_fn_instance(instance, local_ty);

        let slice = tcx.mk_optional(instance, |tcx, instance| {
            let name = with_no_trimmed_paths(|| {
                tcx.def_path_str(instance.def_id())
            });
            let name = tcx.mk_static_str_cv(&*name.as_str());

            let instance = GeobacterEncoder::with(tcx, |encoder| {
                instance.encode(encoder).expect("actual encode kernel instance");
                Ok(())
            }).expect("encode kernel instance");

            let instance_len = instance.len();
            let alloc = Allocation::from_byte_aligned_bytes(instance);
            let alloc = tcx.intern_const_alloc(alloc);
            tcx.create_memory_alloc(alloc);
            let instance = ConstValue::Slice {
                data: alloc,
                start: 0,
                end: instance_len,
            };

            tcx.mk_static_tuple_cv("kernel_instance",
                                   vec![name, instance].into_iter(),
                                   self.inner_ret_ty(tcx))
        });
        let rvalue = tcx.const_value_rvalue(&source_info, slice, self.output(tcx));

        let stmt_kind = StatementKind::Assign(Box::new((ret, rvalue)));
        let stmt = Statement {
            source_info: source_info.clone(),
            kind: stmt_kind,
        };
        bb.statements.push(stmt);
        mir.basic_blocks_mut().push(bb);
    }

    fn generic_parameter_count<'tcx>(&self, _tcx: TyCtxt<'tcx>) -> usize {
        3
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>) -> &'tcx ty::List<Ty<'tcx>> {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        return tcx.mk_static_slice(self.inner_ret_ty(tcx));
    }
}
impl IntrinsicName for KernelInstance {
    const NAME: &'static str = "geobacter_kernel_instance";
}

/// Creates a static variable which can be used (atomically!) to store
/// platform handles for various accelerators. This means the function doesn't
/// need to be looked up in a map.
#[derive(Default)]
pub struct KernelContextDataId;
impl CustomIntrinsicMirGen for KernelContextDataId {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     _instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>) {
        let ptr_size = tcx.pointer_size();
        let data = vec![0; ptr_size.bytes() as usize];
        let min_global_align = tcx.sess.target.target
            .options
            .min_global_align
            .unwrap_or(1);
        // XXX arch dependent. Is this info stored anywhere?
        let align = min_global_align.max(128);
        let align = Align::from_bits(align).unwrap();
        let mut alloc = Allocation::from_bytes(&data[..], align);
        alloc.mutability = ast::Mutability::Mut;
        let alloc = tcx.intern_const_alloc(alloc);
        let alloc_id = tcx.create_memory_alloc(alloc);

        let ret = Place::return_place();

        let source_info = dummy_source_info();

        let mut bb = mir::BasicBlockData {
            statements: Vec::new(),
            terminator: Some(mir::Terminator {
                source_info: source_info.clone(),
                kind: mir::TerminatorKind::Return,
            }),

            is_cleanup: false,
        };

        let ptr = Pointer::from(alloc_id);
        let scalar = Scalar::Ptr(ptr);
        let rvalue = tcx.const_value_rvalue(&source_info,
                                            ConstValue::Scalar(scalar),
                                            self.output(tcx));


        let stmt_kind = StatementKind::Assign(Box::new((ret, rvalue)));
        let stmt = Statement {
            source_info: source_info.clone(),
            kind: stmt_kind,
        };
        bb.statements.push(stmt);
        mir.basic_blocks_mut().push(bb);
    }

    fn generic_parameter_count<'tcx>(&self, _tcx: TyCtxt<'tcx>) -> usize {
        3
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>) -> &'tcx ty::List<Ty<'tcx>> {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.mk_imm_ref(tcx.lifetimes.re_static, tcx.types.usize)
    }
}
impl IntrinsicName for KernelContextDataId {
    const NAME: &'static str = "geobacter_kernel_codegen_stash";
}
impl fmt::Display for KernelContextDataId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("geobacter_kernel_codegen_stash")
    }
}
