use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use log::*;

use smallvec::SmallVec;

use rustc_ast::ast;
use rustc_data_structures::fx::FxHashSet;
use rustc_errors::DiagnosticBuilder;
use rustc_hir::LangItem;
use rustc_index::vec::IndexVec;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::*;

use super::*;
use crate::collector::collect_items_rec;
use rustc_target::abi::VariantIdx;

#[derive(Clone, Copy)]
pub struct CheckShaderInterfaces;
#[derive(Clone, Copy)]
pub struct InputShaderInterface;
#[derive(Clone, Copy)]
pub struct OutputShaderInterface;

impl IntrinsicName for CheckShaderInterfaces {
    const NAME: &'static str = "geobacter_spirv_input_shader_interface";
}
impl fmt::Display for CheckShaderInterfaces {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, Self::NAME)
    }
}
impl IntrinsicName for InputShaderInterface {
    const NAME: &'static str = "geobacter_spirv_input_shader_interface";
}
impl fmt::Display for InputShaderInterface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, Self::NAME)
    }
}
impl IntrinsicName for OutputShaderInterface {
    const NAME: &'static str = "geobacter_spirv_output_shader_interface";
}
impl fmt::Display for OutputShaderInterface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, Self::NAME)
    }
}

impl InputShaderInterface {}


rustc_index::newtype_index! {
    struct IFaceIdx {
      DEBUG_FORMAT = "Interface({})",
    }
}

struct PathSegment<'tcx> {
    variant: VariantIdx,
    field: usize,
    /// Some types must be split over two locations (ie i64 vectors with 3 or more elements).
    split: Option<usize>,
    ty: Ty<'tcx>,
}

struct IFace<'tcx> {
    /// The root static instance
    inst: ty::Instance<'tcx>,
    path: Vec<PathSegment<'tcx>>,
}

struct LocationIdx {
    loc: u32,
    /// `[0, 4]` range.
    component: u8,
}

struct LocationAssignments<'tcx> {
    interfaces: IndexVec<IFaceIdx, IFace<'tcx>>,
    assignments: IndexVec<LocactionIdx, IFaceIdx>,
}

impl<'tcx> LocationAssignments<'tcx> {}


fn num_locations<'tcx>(tcx: TyCtxt<'tcx>, ty: ty::Ty<'tcx>) -> u32 {
    match ty.kind {
        Bool => 1,
        Char => 1,
        Int(_) => 1,
        Uint(_) => 1,
        Float(_) => 1,
        Adt(def, substs) if def.is_simd() => {

        }
        Adt(def, substs) if def.is_struct() => {
            let mut sum = 0u32;

            for field in def.variants[0].fields.iter() {

            }
        }
        Adt(def, substs) if def.is_union() => {

        }
        Adt(def, substs) if def.is_enum() => {

        }
        Adt(..) => unreachable!(),

        Foreign(_) => {
            // XXX ?
            0
        }
    }
}

fn extract_location<'tcx>(tcx: TyCtxt<'tcx>, io_did: DefId, from: ty::Instance<'tcx>)
                          -> Option<Range<u32>>
{
    let ty = from.monomorphic_ty(tcx);
    let (inner_ty, substs) = match ty.kind {
        Adt(def, substs) if def.did == io_did => {
            let ty = def.variants[0].fields[0].ty(tcx, substs);
            (ty, substs)
        }
        _ => { return None; }
    };

    let mut consts = substs.consts()
        .map(|c| {
            c.eval_bits(tcx, ParamEnv::reveal_all(), tcx.types.u32) as u32
        });

    let start = consts.next().unwrap_or_else(|| {
        bug!("expected constant param; got none: {:?}", from);
    });


    Some(start..end)
}
impl CustomIntrinsicMirGen for OutputShaderInterface {
    fn mirgen_simple_intrinsic<'tcx>(&self, tcx: TyCtxt<'tcx>,
                                     instance: ty::Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        // Create an empty function:
        let source_info = mir::SourceInfo {
            span: DUMMY_SP,
            scope: mir::OUTERMOST_SOURCE_SCOPE,
        };

        let mut bb = mir::BasicBlockData {
            statements: vec![],
            terminator: Some(mir::Terminator {
                source_info,
                kind: mir::TerminatorKind::Return,
            }),

            is_cleanup: false,
        };


        let mut visited: FxHashSet<_> = Default::default();
        let entries = instance.substs.types()
            .filter_map(|root| tcx.extract_opt_fn_instance(instance, root));
        for entry in entries {
            // collect all referenced mono items upfront:
            let mono_root = MonoItem::Fn(entry);
            collect_items_rec(tcx, mono_root, &mut visited);
        }

        let lang_items = LangItems::new(tcx);

        let mut sets: BTreeMap<u32, BTreeMap<u32, DescriptorDesc<'_>>> = Default::default();

        for mono in visited.into_iter() {
            let instance = match mono {
                MonoItem::Fn(_) => { continue; }
                MonoItem::Static(mono_did) => Instance::mono(tcx, mono_did),
                MonoItem::GlobalAsm(..) => {
                    bug!("unexpected mono item `{:?}`", mono);
                }
            };

            let desc = lang_items.extract_descriptor(tcx, instance);
            if desc.is_none() { continue; }
            let (desc, slot) = desc.unwrap();

            let set = match sets.entry(slot.0) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => {
                    v.insert(BTreeMap::default())
                }
            };

            let desc = match set.entry(slot.1) {
                Entry::Vacant(v) => {
                    // the happy path
                    v.insert(desc)
                }
                Entry::Occupied(o) => {
                    // There are two or more statics which are assigned to the same set+binding.
                    o.into_mut()
                }
            };
            desc.insts.push(instance);
        }

        info!("desc set bindings: {:#?}", sets);
        let sets = sets;

        let desc_set_ty = tcx.mk_array(tcx.mk_tup([
            tcx.types.u32,
            desc_bindings_desc_ty(tcx),
        ].iter()), sets.len() as _);

        let desc_bindings_ty = desc_desc_ty(tcx);
        let mut c_sets: SmallVec<[_; 32]> = SmallVec::new();
        c_sets.reserve(sets.len() * 2);
        for (&set_id, set) in sets.iter() {
            let set_bindings_ty = tcx.mk_array(desc_bindings_ty, set.len() as _);

            let mut c_set = <SmallVec<[_; 32]>>::with_capacity(set.len() * 8);
            for (&binding_id, desc) in set.iter() {
                // check that all the spots have only one instance, otherwise report an error.
                if desc.insts.len() > 1 {
                    let hir = tcx.hir();

                    let msg = format!("duplicate descriptor: set = {}, binding = {}",
                                      set_id, binding_id);

                    let mut diag: Option<DiagnosticBuilder<'_>> = None;
                    for inst in desc.insts.iter() {
                        let local_did = inst.def_id().as_local()
                            .expect("TODO: non-local DefIds");
                        let hir_id = hir.local_def_id_to_hir_id(local_did);
                        let span = hir.span(hir_id);
                        match diag {
                            Some(ref mut diag) => {
                                diag.span_label(span, &msg);
                            }
                            None => {
                                diag = Some(tcx.sess.struct_span_err(span, &msg));
                            }
                        }
                    }
                }

                c_set.extend(build_compiler_descriptor_desc(tcx, binding_id, desc));
            }

            let slice = tcx.mk_static_slice_cv("desc set bindings",
                                               c_set.into_iter(),
                                               set_bindings_ty,
                                               set.len());
            c_sets.push(tcx.mk_u32_cv(set_id));
            c_sets.push(slice);
        }

        let slice = tcx.mk_static_slice_cv("desc sets",
                                           c_sets.into_iter(),
                                           desc_set_ty,
                                           sets.len());
        let ret_ty = self.output(tcx);
        let slice = tcx.const_value_rvalue(&source_info, slice, ret_ty);

        let ret = mir::Place::return_place();
        let stmt_kind = StatementKind::Assign(Box::new((ret, slice)));
        let stmt = Statement {
            source_info,
            kind: stmt_kind,
        };
        bb.statements.push(stmt);
        mir.basic_blocks_mut().push(bb);
    }

    fn generic_parameter_count<'tcx>(&self, _tcx: TyCtxt<'tcx>) -> usize {
        self.0 as _
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        desc_set_bindings_desc_ty(tcx)
    }
}

impl CustomIntrinsicMirGen for InputShaderInterface {
    fn mirgen_simple_intrinsic<'tcx>(&self, tcx: TyCtxt<'tcx>,
                                     instance: ty::Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        // Create an empty function:
        let source_info = mir::SourceInfo {
            span: DUMMY_SP,
            scope: mir::OUTERMOST_SOURCE_SCOPE,
        };

        let mut bb = mir::BasicBlockData {
            statements: vec![],
            terminator: Some(mir::Terminator {
                source_info,
                kind: mir::TerminatorKind::Return,
            }),

            is_cleanup: false,
        };

        let mut visited: FxHashSet<_> = Default::default();
        let entries = instance.substs.types()
            .filter_map(|root| tcx.extract_opt_fn_instance(instance, root));
        for entry in entries {
            // collect all referenced mono items upfront:
            let mono_root = MonoItem::Fn(entry);
            collect_items_rec(tcx, mono_root, &mut visited);
        }

        let builtin_did = tcx.require_lang_item(LangItem::SpirvShaderInput);

        for mono in visited.into_iter() {
            let instance = match mono {
                MonoItem::Fn(_) => { continue; }
                MonoItem::Static(mono_did) => Instance::mono(tcx, mono_did),
                MonoItem::GlobalAsm(..) => {
                    bug!("unexpected mono item `{:?}`", mono);
                }
            };

            if let Some(location) = extract_location(tcx, builtin_did, instance) {

            }
        }

        let ret_ty = self.output(tcx);
        let slice = tcx.const_value_rvalue(&source_info, slice, ret_ty);

        let ret = mir::Place::return_place();
        let stmt_kind = StatementKind::Assign(Box::new((ret, slice)));
        let stmt = Statement {
            source_info,
            kind: stmt_kind,
        };
        bb.statements.push(stmt);
        mir.basic_blocks_mut().push(bb);
    }

    fn generic_parameter_count<'tcx>(&self, _tcx: TyCtxt<'tcx>) -> usize {
        self.0 as _
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        desc_set_bindings_desc_ty(tcx)
    }
}
