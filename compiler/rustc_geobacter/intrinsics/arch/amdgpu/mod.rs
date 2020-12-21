
use super::*;
use crate::intrinsics::suicide::Suicide;

pub mod dpp;
pub mod grid;

pub type AmdGpuSuicide = Suicide<SuicideDetail>;

def_id_intrinsic!(fn amdgcn_workitem_id_x() -> u32 => "llvm.amdgcn.workitem.id.x");
def_id_intrinsic!(fn amdgcn_workitem_id_y() -> u32 => "llvm.amdgcn.workitem.id.y");
def_id_intrinsic!(fn amdgcn_workitem_id_z() -> u32 => "llvm.amdgcn.workitem.id.z");
def_id_intrinsic!(fn amdgcn_workgroup_id_x() -> u32 => "llvm.amdgcn.workgroup.id.x");
def_id_intrinsic!(fn amdgcn_workgroup_id_y() -> u32 => "llvm.amdgcn.workgroup.id.y");
def_id_intrinsic!(fn amdgcn_workgroup_id_z() -> u32 => "llvm.amdgcn.workgroup.id.z");
def_id_intrinsic!(fn amdgcn_barrier()      => "llvm.amdgcn.s.barrier");
def_id_intrinsic!(fn amdgcn_wave_barrier() => "llvm.amdgcn.wave.barrier");
def_id_intrinsic!(fn amdgcn_kill(b: bool) -> ! => "llvm.amdgcn.kill");
def_id_intrinsic! {
    fn amdgcn_update_dpp_i32(old: i32, src: i32, dpp_ctrl: i32, row_mask: i32,
                             bank_mask: i32, bound_ctrl: bool) -> i32
        => "llvm.amdgcn.update.dpp.i32"
}
def_id_intrinsic!(fn amdgcn_sendmsg(arg0: i32, arg1: u32) => "llvm.amdgcn.s.sendmsg");
def_id_intrinsic!(fn amdgcn_readfirstlane(arg1: u32) -> u32 => "llvm.amdgcn.readfirstlane");

/// This one is an actual Rust intrinsic; the LLVM intrinsic returns
/// a pointer in the constant address space, which we can't correctly
/// model here in Rust land (the Rust type system has no knowledge of
/// address spaces), so we have to have the compiler help us by inserting
/// a cast to the flat addr space.
fn amdgcn_dispatch_ptr() -> *const u8 {
    extern "rust-intrinsic" {
        fn amdgcn_dispatch_ptr() -> *const u8;
    }
    unsafe { amdgcn_dispatch_ptr() }
}

pub fn insert_all_intrinsics<F>(mut map: F)
    where F: for<'a> FnMut(&'a str, Lrc<dyn CustomIntrinsicMirGen>),
{
    DispatchPtr::insert_into_map(&mut map);
    Barrier::insert_into_map(&mut map);
    WaveBarrier::insert_into_map(&mut map);
    SendMsg::insert_into_map(&mut map);
    ReadFirstLane::insert_into_map(&mut map);
    dpp::UpdateDpp::insert_into_map(&mut map);
    dpp::UpdateDppWorkaround::insert_into_map(&mut map);
    grid::insert_all_intrinsics(&mut map);
}

pub fn find_intrinsic(tcx: TyCtxt<'_>, name: &str)
    -> Result<(), Lrc<dyn CustomIntrinsicMirGen>>
{
    match &tcx.sess.target.target.arch[..] {
        "amdgpu" => {
            AmdGpuSuicide::check(name)?;
        },
        _ => { },
    };

    DispatchPtr::check(name)?;
    Barrier::check(name)?;
    WaveBarrier::check(name)?;
    SendMsg::check(name)?;
    ReadFirstLane::check(name)?;
    dpp::UpdateDpp::check(name)?;
    dpp::UpdateDppWorkaround::check(name)?;
    grid::find_intrinsic(tcx, name)?;

    Ok(())
}

fn target_check(tcx: TyCtxt<'_>) -> Option<()> {
    // panic if not running on an AMDGPU
    match &tcx.sess.target.target.arch[..] {
        "amdgpu" => { },
        _ => { return None; },
    };
    Some(())
}

pub struct SuicideDetail;
impl PlatformImplDetail for SuicideDetail {
    fn platform() -> &'static str { "amdgpu" }
    fn kernel_instance() -> Option<KernelInstanceRef<'static>> {
        #[inline(always)]
        fn kill() -> ! {
            // the real intrinsic needs a single argument.
            amdgcn_kill(false);
        }
        Some(kill.kernel_instance())
    }
}
impl IntrinsicName for Suicide<SuicideDetail> {
    const NAME: &'static str = "geobacter_suicide";
}

#[derive(Default)]
pub struct DispatchPtr;
impl DispatchPtr {
    fn kernel_instance(&self) -> KernelInstanceRef<'static> {
        amdgcn_dispatch_ptr.kernel_instance()
    }
}
impl mir::CustomIntrinsicMirGen for DispatchPtr {
    fn mirgen_simple_intrinsic<'tcx>(&self, tcx: TyCtxt<'tcx>,
                                     _instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);
        tcx.call_device_inst(mir, move || {
            target_check(tcx)?;
            Some(self.kernel_instance())
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        0
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.mk_imm_ptr(tcx.types.u8)
    }
}
impl IntrinsicName for DispatchPtr {
    const NAME: &'static str = "geobacter_amdgpu_dispatch_ptr";
}
impl fmt::Display for DispatchPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::NAME)
    }
}

#[derive(Default)]
pub struct Barrier;
impl Barrier {
    fn kernel_instance(&self) -> KernelInstanceRef<'static> {
        amdgcn_barrier.kernel_instance()
    }
}
impl CustomIntrinsicMirGen for Barrier {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     _instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);
        tcx.call_device_inst(mir, move || {
            target_check(tcx)?;
            Some(self.kernel_instance())
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        0
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.types.unit
    }
}
impl IntrinsicName for Barrier {
    const NAME: &'static str = "geobacter_amdgpu_barrier";
}
impl fmt::Display for Barrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::NAME)
    }
}

#[derive(Default)]
pub struct WaveBarrier;
impl WaveBarrier {
    fn kernel_instance(&self) -> KernelInstanceRef<'static> {
        amdgcn_wave_barrier.kernel_instance()
    }
}
impl CustomIntrinsicMirGen for WaveBarrier {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     _instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);
        tcx.call_device_inst(mir, move || {
            target_check(tcx)?;
            Some(self.kernel_instance())
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        0
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.types.unit
    }
}
impl IntrinsicName for WaveBarrier {
    const NAME: &'static str = "geobacter_amdgpu_wave_barrier";
}
impl fmt::Display for WaveBarrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::NAME)
    }
}
/// This exists because we need to selectively not emit the LLVM intrinsic on the host, as
/// it's used from a non-generic location. Otherwise we'll get:
/// "LLVM ERROR: Cannot select: intrinsic %llvm.amdgcn.s.sendmsg".
#[derive(Default)]
pub struct SendMsg;
impl SendMsg {
    fn kernel_instance(&self) -> KernelInstanceRef<'static> {
        amdgcn_sendmsg.kernel_instance()
    }
}
impl CustomIntrinsicMirGen for SendMsg {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     _instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);
        let args = mir.args_iter()
            .map(mir::Place::from)
            .map(Operand::Move)
            .collect();
        tcx.call_device_inst_args(mir, move || {
            target_check(tcx)?;
            Some((self.kernel_instance(), args))
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        0
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[tcx.types.i32, tcx.types.u32])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.types.unit
    }
}
impl IntrinsicName for SendMsg {
    const NAME: &'static str = "geobacter_amdgpu_sendmsg";
}
impl fmt::Display for SendMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::NAME)
    }
}
/// This exists because we need to selectively not emit the LLVM intrinsic on the host, as
/// it's used from a non-generic location. Otherwise we'll get:
/// "LLVM ERROR: Cannot select: intrinsic %llvm.amdgcn.readfirstlane".
#[derive(Default)]
pub struct ReadFirstLane;
impl ReadFirstLane {
    fn kernel_instance(&self) -> KernelInstanceRef<'static> {
        amdgcn_readfirstlane.kernel_instance()
    }
}
impl CustomIntrinsicMirGen for ReadFirstLane {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     _instance: Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        debug!("mirgen intrinsic {}", self);
        let args = mir.args_iter()
            .map(mir::Place::from)
            .map(Operand::Move)
            .collect();
        tcx.call_device_inst_args(mir, move || {
            target_check(tcx)?;
            Some((self.kernel_instance(), args))
        });
    }

    fn generic_parameter_count(&self, _tcx: TyCtxt<'_>) -> usize {
        0
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[tcx.types.u32])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        tcx.types.u32
    }
}
impl IntrinsicName for ReadFirstLane {
    const NAME: &'static str = "geobacter_amdgpu_readfirstlane";
}
impl fmt::Display for ReadFirstLane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::NAME)
    }
}
