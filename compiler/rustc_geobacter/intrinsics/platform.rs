
use std::mem::{size_of, transmute, align_of};

use super::*;

/// This intrinsic has to be manually inserted by the drivers
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug)]
pub struct PlatformIntrinsic(pub Platform);
impl PlatformIntrinsic {
  pub const fn host_platform() -> Self { PlatformIntrinsic(Platform::Host) }

  fn data(self) -> [u8; size_of::<Platform>()] {
    unsafe {
      transmute(self.0)
    }
  }
}
impl IntrinsicName for PlatformIntrinsic {
  const NAME: &'static str = "geobacter_platform";
}
impl fmt::Display for PlatformIntrinsic {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "geobacter_platform")
  }
}
impl mir::CustomIntrinsicMirGen for PlatformIntrinsic {
  fn mirgen_simple_intrinsic<'tcx>(&self,
                                   tcx: TyCtxt<'tcx>,
                                   _instance: Instance<'tcx>,
                                   mir: &mut mir::Body<'tcx>) {
    let align = Align::from_bits(align_of::<Platform>() as _).unwrap();
    let data = &self.data()[..];
    let alloc = Allocation::from_bytes(data, align);
    let alloc = tcx.intern_const_alloc(alloc);
    let alloc_id = tcx.create_memory_alloc(alloc);

    let ret = Place::return_place();

    let source_info = mir::SourceInfo {
      span: DUMMY_SP,
      scope: mir::OUTERMOST_SOURCE_SCOPE,
    };

    let mut bb = mir::BasicBlockData {
      statements: Vec::new(),
      terminator: Some(mir::Terminator {
        source_info: source_info.clone(),
        kind: mir::TerminatorKind::Return,
      }),

      is_cleanup: false,
    };

    let ptr = Pointer::from(alloc_id);
    let const_val = ConstValue::Scalar(ptr.into());
    let constant = tcx.mk_const_op(&source_info, Const {
      ty: self.output(tcx),
      val: ConstKind::Value(const_val),
    });
    let rvalue = Rvalue::Use(constant);

    let stmt_kind = StatementKind::Assign(Box::new((ret, rvalue)));
    let stmt = Statement {
      source_info: source_info.clone(),
      kind: stmt_kind,
    };
    bb.statements.push(stmt);
    mir.basic_blocks_mut().push(bb);
  }

  fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
    0
  }
  /// The types of the input args.
  fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>) -> &'tcx ty::List<Ty<'tcx>> {
    tcx.intern_type_list(&[])
  }
  /// The return type.
  fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let arr = tcx.mk_array(tcx.types.u8, size_of::<Platform>() as _);
    tcx.mk_imm_ref(tcx.lifetimes.re_static, arr)
  }
}
