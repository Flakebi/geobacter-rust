use super::*;
use crate::intrinsics::suicide::Suicide;

use tracing::info;

pub use pipeline_layout::PipelineLayoutDesc;

pub mod pipeline_layout;
// WIP
//pub mod shader_interface;

#[inline(always)]
pub fn insert_all_intrinsics<F>(map: F)
    where F: for<'a> FnMut(&'a str, Lrc<dyn CustomIntrinsicMirGen>),
{
    PipelineLayoutDesc::insert_into_map(map);
}

pub fn find_intrinsic(tcx: TyCtxt<'_>, name: &str)
                      -> Result<(), Lrc<dyn CustomIntrinsicMirGen>>
{
    match &tcx.sess.target.target.arch[..] {
        "spirv" => {
            SpirvSuicide::check(name)?;
        },
        _ => { },
    };

    PipelineLayoutDesc::check(name)?;

    Ok(())
}

def_id_intrinsic!(fn spirv_kill() -> ! => "llvm.spirv.kill");

pub struct SuicideDetail;
impl PlatformImplDetail for SuicideDetail {
    fn platform() -> &'static str { "spirv" }
    fn kernel_instance() -> Option<KernelInstanceRef<'static>> {
        Some(spirv_kill.kernel_instance())
    }
}
impl IntrinsicName for SpirvSuicide {
    const NAME: &'static str = "geobacter_suicide";
}
pub type SpirvSuicide = Suicide<SuicideDetail>;
