
use super::*;

#[derive(Clone, Copy, Debug, Default, Hash)]
pub struct SpecializationParam;

impl SpecializationParam { }

impl CustomIntrinsicMirGen for SpecializationParam {
  fn mirgen_simple_intrinsic<'tcx>(&self,
                                   tcx: TyCtxt<'tcx>,
                                   instance: ty::Instance<'tcx>,
                                   mir: &mut Body<'tcx>)
  {
    let source_info = SourceInfo {
      span: DUMMY_SP,
      scope: mir::OUTERMOST_SOURCE_SCOPE,
    };

    let mut bb = BasicBlockData {
      statements: Vec::new(),
      terminator: Some(mir::Terminator {
        source_info: source_info.clone(),
        kind: TerminatorKind::Return,
      }),

      is_cleanup: false,
    };

    let ret = Place::return_place();
    let local_ty = instance.substs
        .types()
        .next()
        .unwrap();

    let instance = tcx.extract_fn_instance(instance, local_ty);

    let param_data = tcx.specialization_data(instance);
    // TODO: endianness

    let slice = match param_data {
      Some(param_data) => {
        let alloc = Allocation::from_byte_aligned_bytes(&*param_data);
        let alloc = tcx.intern_const_alloc(alloc);
        tcx.create_memory_alloc(alloc);
        ConstValue::Slice {
          data: alloc,
          start: 0,
          end: 1,
        }
      },
      None => {
        let alloc = Allocation::from_byte_aligned_bytes(&([0u8; 0])[..]);
        let alloc = tcx.intern_const_alloc(alloc);
        tcx.create_memory_alloc(alloc);
        ConstValue::Slice {
          data: alloc,
          start: 0,
          end: 0,
        }
      },
    };

    let rvalue = tcx.const_value_rvalue(&source_info, slice,
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
    2
  }
  /// The types of the input args.
  fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>) -> &'tcx ty::List<Ty<'tcx>> {
    tcx.intern_type_list(&[])
  }
  /// The return type.
  fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let n = 1;
    let p = Symbol::intern("R");
    let f = tcx.mk_ty_param(n, p);
    return tcx.mk_static_slice(f);
  }
}
impl IntrinsicName for SpecializationParam {
  const NAME: &'static str = "geobacter_specialization_param";
}
impl fmt::Display for SpecializationParam {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "geobacter_specialization_param")
  }
}
