
//! fn geobacter_suicide(why: &str) -> !, but allows overriding by the cross platform codegen.

use std::marker::PhantomData;

use super::*;

/// Kill (ie `abort()`) the current workitem/thread only.
pub struct Suicide<T>(PhantomData<T>)
    where T: PlatformImplDetail;
impl<T> Suicide<T>
    where T: PlatformImplDetail,
{ }
impl<T> Default for Suicide<T>
    where T: PlatformImplDetail,
{
    fn default() -> Self {
        Suicide(PhantomData)
    }
}

impl<T> CustomIntrinsicMirGen for Suicide<T>
    where T: PlatformImplDetail,
{
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     _instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);
        tcx.redirect_or_panic(mir, || {
            Operand::Move(Local::new(1).into())
        }, move || {
            let id = T::kernel_instance()?;
            let instance = tcx.convert_kernel_instance(id)
                .expect("failed to convert kernel id to def id");
            Some((instance, vec![]))
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        0
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        let why = tcx.types.str_;
        let region = tcx.mk_region(ty::ReLateBound(ty::INNERMOST, ty::BrAnon(0)));
        tcx.intern_type_list(&[tcx.mk_imm_ref(region, why)])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.types.never
    }
}
impl<T> fmt::Debug for Suicide<T>
    where T: PlatformImplDetail,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "geobacter_suicide<{:?}>", T::platform())
    }
}
impl<T> fmt::Display for Suicide<T>
    where T: PlatformImplDetail,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "geobacter_suicide")
    }
}

pub struct PanicSuicide;
impl PlatformImplDetail for PanicSuicide {
    fn platform() -> &'static str { "host" }
    fn kernel_instance() -> Option<KernelInstanceRef<'static>> {
        None
    }
}
impl IntrinsicName for Suicide<PanicSuicide> {
    const NAME: &'static str = "geobacter_suicide";
}
