use super::*;

pub fn insert_all_intrinsics<F>(mut map: F)
    where F: for<'a> FnMut(&'a str, Lrc<dyn CustomIntrinsicMirGen>),
{
    for &(k, v) in AxisId::permutations().iter() {
        map(k, Lrc::new(v));
    }
}

pub fn find_intrinsic(_: TyCtxt<'_>, name: &str)
    -> Result<(), Lrc<dyn CustomIntrinsicMirGen>>
{
    for &(k, v) in AxisId::permutations().iter() {
        if k == name {
            return Err(Lrc::new(v));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum Dim {
    X,
    Y,
    Z,
}
impl Dim {
    fn name(&self) -> &'static str {
        match self {
            &Dim::X => "x",
            &Dim::Y => "y",
            &Dim::Z => "z",
        }
    }
}
impl fmt::Display for Dim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
#[derive(Debug, Clone, Copy)]
enum BlockLevel {
    Item,
    Group,
}
impl BlockLevel {
    fn name(&self) -> &'static str {
        match self {
            &BlockLevel::Item => "workitem",
            &BlockLevel::Group => "workgroup",
        }
    }
}
impl fmt::Display for BlockLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AxisId {
    block: BlockLevel,
    dim: Dim,
}
impl AxisId {
    fn permutations() -> &'static [(&'static str, Self); 6] {
        const C: &'static [(&'static str, AxisId); 6] = &[
            ("geobacter_amdgpu_workgroup_x_id",
             AxisId { block: BlockLevel::Group, dim: Dim::X, }, ),

            ("geobacter_amdgpu_workgroup_y_id",
             AxisId { block: BlockLevel::Group, dim: Dim::Y, }, ),

            ("geobacter_amdgpu_workgroup_z_id",
             AxisId { block: BlockLevel::Group, dim: Dim::Z, }, ),

            ("geobacter_amdgpu_workitem_x_id",
             AxisId { block: BlockLevel::Item, dim: Dim::X, }, ),

            ("geobacter_amdgpu_workitem_y_id",
             AxisId { block: BlockLevel::Item, dim: Dim::Y, }, ),

            ("geobacter_amdgpu_workitem_z_id",
             AxisId { block: BlockLevel::Item, dim: Dim::Z, }, ),
        ];
        C
    }
    fn kernel_instance(&self) -> KernelInstanceRef<'static> {
        match self {
            &AxisId {
                block: BlockLevel::Item,
                dim: Dim::X,
            } => {
                amdgcn_workitem_id_x
                    .kernel_instance()
            },
            &AxisId {
                block: BlockLevel::Item,
                dim: Dim::Y,
            } => {
                amdgcn_workitem_id_y
                    .kernel_instance()
            },
            &AxisId {
                block: BlockLevel::Item,
                dim: Dim::Z,
            } => {
                amdgcn_workitem_id_z
                    .kernel_instance()
            },
            &AxisId {
                block: BlockLevel::Group,
                dim: Dim::X,
            } => {
                amdgcn_workgroup_id_x
                    .kernel_instance()
            },
            &AxisId {
                block: BlockLevel::Group,
                dim: Dim::Y,
            } => {
                amdgcn_workgroup_id_y
                    .kernel_instance()
            },
            &AxisId {
                block: BlockLevel::Group,
                dim: Dim::Z,
            } => {
                amdgcn_workgroup_id_z
                    .kernel_instance()
            },
        }
    }
}
impl mir::CustomIntrinsicMirGen for AxisId {
    fn mirgen_simple_intrinsic<'tcx>(&self,
                                     tcx: TyCtxt<'tcx>,
                                     _instance: ty::Instance<'tcx>,
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
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[])
    }
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        return tcx.types.u32;
    }
}
impl fmt::Display for AxisId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "geobacter_amdgpu_{}_{}_id", self.block, self.dim)
    }
}
