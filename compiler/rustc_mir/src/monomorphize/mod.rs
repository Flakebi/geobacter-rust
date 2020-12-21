use rustc_index::vec::IndexVec;
use rustc_middle::{traits, mir};
use rustc_middle::ty::adjustment::CustomCoerceUnsized;
use rustc_middle::ty::{self, Ty, TyCtxt, Instance};
use rustc_middle::ty::query::Providers;
use rustc_span::DUMMY_SP;

use rustc_hir::lang_items::LangItem;

pub mod collector;
pub mod partitioning;
pub mod polymorphize;

pub fn provide(providers: &mut Providers) {
    providers.custom_intrinsic_mirgen = |_, _| { None };
    providers.custom_intrinsic_mir = custom_intrinsic_mir;
}

fn custom_intrinsic_mir<'tcx>(tcx: TyCtxt<'tcx>,
                              instance: Instance<'tcx>)
    -> Option<&'tcx mir::Body<'tcx>>
{
    let mirgen = tcx.custom_intrinsic_mirgen(instance.def_id())?;

    let ty = instance.ty(tcx, ty::ParamEnv::reveal_all());
    let sig = ty.fn_sig(tcx);
    let sig = tcx.normalize_erasing_late_bound_regions(
        ty::ParamEnv::reveal_all(),
        &sig,
    );

    // no var arg calls, so we can skip monomorphizing extra arguments.
    assert!(!sig.c_variadic);

    let source_scope_local_data = mir::ClearCrossCrate::Clear;
    let source_scope = mir::SourceScopeData {
        span: DUMMY_SP,
        parent_scope: None,
        local_data: source_scope_local_data,
    };
    let source_info = mir::SourceInfo {
        span: DUMMY_SP,
        scope: mir::OUTERMOST_SOURCE_SCOPE,
    };

    let mut source_scopes = IndexVec::new();
    source_scopes.push(source_scope.clone());

    let ret_decl = mir::LocalDecl::new(sig.output(), DUMMY_SP);
    let mut local_decls = IndexVec::from_elem_n(ret_decl, 1);
    for &arg in sig.inputs().iter() {
        local_decls.push(mir::LocalDecl {
            mutability: mir::Mutability::Mut,
            local_info: None,
            ty: arg,
            source_info,
            internal: false,
            user_ty: None,
            is_block_tail: None,
        });
    }

    let mut gen = mir::Body::new(IndexVec::new(),
                                 source_scopes,
                                 local_decls,
                                 Default::default(),
                                 sig.inputs().len(),
                                 Vec::new(),
                                 source_scope.span,
                                 None);

    mirgen.mirgen_simple_intrinsic(tcx, instance, &mut gen);

    Some(tcx.arena.alloc(gen))
}

pub fn custom_coerce_unsize_info<'tcx>(
    tcx: TyCtxt<'tcx>,
    source_ty: Ty<'tcx>,
    target_ty: Ty<'tcx>,
) -> CustomCoerceUnsized {
    let def_id = tcx.require_lang_item(LangItem::CoerceUnsized, None);

    let trait_ref = ty::Binder::bind(ty::TraitRef {
        def_id,
        substs: tcx.mk_substs_trait(source_ty, &[target_ty.into()]),
    });

    match tcx.codegen_fulfill_obligation((ty::ParamEnv::reveal_all(), trait_ref)) {
        Ok(traits::ImplSource::UserDefined(traits::ImplSourceUserDefinedData {
            impl_def_id,
            ..
        })) => tcx.coerce_unsized_info(impl_def_id).custom_kind.unwrap(),
        impl_source => {
            bug!("invalid `CoerceUnsized` impl_source: {:?}", impl_source);
        }
    }
}
