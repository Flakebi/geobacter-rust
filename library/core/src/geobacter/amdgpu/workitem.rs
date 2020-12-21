use crate::geobacter::intrinsics::*;
use crate::intrinsics::transmute;
use crate::marker::Copy;
use crate::mem::size_of;
use super::{DispatchPacket, ensure_amdgpu};
use crate::raw::TraitObject;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct XAxis;
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct YAxis;
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ZAxis;

pub trait WorkItemAxis {
    fn workitem_id(&self) -> u32;
}
impl WorkItemAxis for Axis {
    #[inline(always)]
    fn workitem_id(&self) -> u32 {
        match self {
            &Axis::X => XAxis.workitem_id(),
            &Axis::Y => YAxis.workitem_id(),
            &Axis::Z => ZAxis.workitem_id(),
        }
    }
}
impl WorkItemAxis for XAxis {
    #[inline(always)]
    fn workitem_id(&self) -> u32 {
        ensure_amdgpu("workitem_x_id");
        unsafe { geobacter_amdgpu_workitem_x_id() as _ }
    }
}
impl WorkItemAxis for YAxis {
    #[inline(always)]
    fn workitem_id(&self) -> u32 {
        ensure_amdgpu("workitem_y_id");
        unsafe { geobacter_amdgpu_workitem_y_id() as _ }
    }
}
impl WorkItemAxis for ZAxis {
    #[inline(always)]
    fn workitem_id(&self) -> u32 {
        ensure_amdgpu("workitem_z_id");
        unsafe { geobacter_amdgpu_workitem_z_id() as _ }
    }
}

pub trait WorkGroupAxis {
    fn workgroup_id(&self) -> u32;
    fn workgroup_size(&self, p: &DispatchPacket) -> u32;
}
impl WorkGroupAxis for Axis {
    #[inline(always)]
    fn workgroup_id(&self) -> u32 {
        match self {
            &Axis::X => XAxis.workgroup_id(),
            &Axis::Y => YAxis.workgroup_id(),
            &Axis::Z => ZAxis.workgroup_id(),
        }
    }
    #[inline(always)]
    fn workgroup_size(&self, p: &DispatchPacket) -> u32 {
        match self {
            &Axis::X => XAxis.workgroup_size(p),
            &Axis::Y => YAxis.workgroup_size(p),
            &Axis::Z => ZAxis.workgroup_size(p),
        }
    }
}
impl WorkGroupAxis for XAxis {
    #[inline(always)]
    fn workgroup_id(&self) -> u32 {
        ensure_amdgpu("workgroup_x_id");
        unsafe { geobacter_amdgpu_workgroup_x_id() as _ }
    }
    #[inline(always)]
    fn workgroup_size(&self, p: &DispatchPacket) -> u32 {
        p.workgroup_size_x as _
    }
}
impl WorkGroupAxis for YAxis {
    #[inline(always)]
    fn workgroup_id(&self) -> u32 {
        ensure_amdgpu("workgroup_y_id");
        unsafe { geobacter_amdgpu_workgroup_y_id() as _ }
    }
    #[inline(always)]
    fn workgroup_size(&self, p: &DispatchPacket) -> u32 {
        p.workgroup_size_y as _
    }
}
impl WorkGroupAxis for ZAxis {
    #[inline(always)]
    fn workgroup_id(&self) -> u32 {
        ensure_amdgpu("workgroup_z_id");
        unsafe { geobacter_amdgpu_workgroup_z_id() as _ }
    }
    #[inline(always)]
    fn workgroup_size(&self, p: &DispatchPacket) -> u32 {
        p.workgroup_size_z as _
    }
}
pub trait GridAxis {
    fn grid_size(&self, p: &DispatchPacket) -> u32;
}
impl GridAxis for Axis {
    #[inline(always)]
    fn grid_size(&self, p: &DispatchPacket) -> u32 {
        match self {
            &Axis::X => XAxis.grid_size(p),
            &Axis::Y => YAxis.grid_size(p),
            &Axis::Z => ZAxis.grid_size(p),
        }
    }
}
impl GridAxis for XAxis {
    #[inline(always)]
    fn grid_size(&self, p: &DispatchPacket) -> u32 {
        p.grid_size_x
    }
}
impl GridAxis for YAxis {
    #[inline(always)]
    fn grid_size(&self, p: &DispatchPacket) -> u32 {
        p.grid_size_y
    }
}
impl GridAxis for ZAxis {
    #[inline(always)]
    fn grid_size(&self, p: &DispatchPacket) -> u32 {
        p.grid_size_z
    }
}

#[inline(always)]
pub fn workitem_ids() -> [u32; 3] {
    [
        XAxis.workitem_id(),
        YAxis.workitem_id(),
        ZAxis.workitem_id(),
    ]
}
#[inline(always)]
pub fn workgroup_ids() -> [u32; 3] {
    [
        XAxis.workgroup_id(),
        YAxis.workgroup_id(),
        ZAxis.workgroup_id(),
    ]
}

impl DispatchPacket {
    #[inline(always)]
    pub fn workgroup_sizes(&self) -> [u32; 3] {
        [
            XAxis.workgroup_size(self),
            YAxis.workgroup_size(self),
            ZAxis.workgroup_size(self),
        ]
    }
    #[inline(always)]
    pub fn grid_sizes(&self) -> [u32; 3] {
        [
            XAxis.grid_size(self),
            YAxis.grid_size(self),
            ZAxis.grid_size(self),
        ]
    }
    #[inline(always)]
    pub fn global_linear_id(&self) -> usize {
        let [l0, l1, l2] = workitem_ids();
        let [g0, g1, g2] = workgroup_ids();
        let [s0, s1, s2] = self.workgroup_sizes();
        let [n0, n1, _n2] = self.grid_sizes();

        let n0 = n0 as usize;
        let n1 = n1 as usize;

        let i0 = (g0 * s0 + l0) as usize;
        let i1 = (g1 * s1 + l1) as usize;
        let i2 = (g2 * s2 + l2) as usize;
        (i2 * n1 + i1) * n0 + i0
    }
    #[inline(always)]
    pub fn global_id_x(&self) -> u32 {
        self.global_id(XAxis)
    }
    #[inline(always)]
    pub fn global_id_y(&self) -> u32 {
        self.global_id(YAxis)
    }
    #[inline(always)]
    pub fn global_id_z(&self) -> u32 {
        self.global_id(ZAxis)
    }
    #[inline(always)]
    pub fn global_id<T>(&self, axis: T) -> u32
        where T: WorkItemAxis + WorkGroupAxis,
    {
        let l = axis.workitem_id();
        let g = axis.workgroup_id();
        let s = axis.workgroup_size(self);
        g * s + l
    }
    #[inline(always)]
    pub fn global_ids(&self) -> (u32, u32, u32) {
        (self.global_id_x(), self.global_id_y(), self.global_id_z())
    }
}

use crate::geobacter::intrinsics::geobacter_amdgpu_readfirstlane as read_first_lane;

/// This trait requires that Drop is not implemented.
pub trait ReadFirstLane {
    unsafe fn read_first_lane(self) -> Self;
}
impl<T> ReadFirstLane for [T; 1]
    where T: ReadFirstLane,
{
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        let [v] = self;
        unsafe {
            [v.read_first_lane(); 1]
        }
    }
}
impl<T> ReadFirstLane for [T; 2]
    where T: ReadFirstLane,
{
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        let [v0, v1] = self;
        unsafe {
            [v0.read_first_lane(), v1.read_first_lane()]
        }
    }
}
impl<T> ReadFirstLane for [T; 3]
    where T: ReadFirstLane,
{
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        let [v0, v1, v2] = self;
        unsafe {
            [
                v0.read_first_lane(),
                v1.read_first_lane(),
                v2.read_first_lane(),
            ]
        }
    }
}
impl<T> ReadFirstLane for [T; 4]
    where T: ReadFirstLane,
{
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        let [v0, v1, v2, v3] = self;
        unsafe {
            [
                v0.read_first_lane(),
                v1.read_first_lane(),
                v2.read_first_lane(),
                v3.read_first_lane(),
            ]
        }
    }
}

impl ReadFirstLane for i8 {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let v: u8 = crate::mem::transmute(self);
            let v: u8 = read_first_lane(v as _) as _;
            crate::mem::transmute(v)
        }
    }
}
impl ReadFirstLane for i16 {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let v: u16 = crate::mem::transmute(self);
            let v: u16 = read_first_lane(v as _) as _;
            crate::mem::transmute(v)
        }
    }
}
impl ReadFirstLane for i32 {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let v = crate::mem::transmute(self);
            let v = read_first_lane(v);
            crate::mem::transmute(v)
        }
    }
}
#[cfg(target_pointer_width = "32")]
impl ReadFirstLane for isize {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let v = crate::mem::transmute(self);
            let v = read_first_lane(v);
            crate::mem::transmute(v)
        }
    }
}
#[cfg(target_pointer_width = "64")]
impl ReadFirstLane for isize {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let v: u64 = crate::mem::transmute(self);
            crate::mem::transmute(v.read_first_lane())
        }
    }
}
impl ReadFirstLane for u8 {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe { read_first_lane(self as _) as _ }
    }
}
impl ReadFirstLane for u16 {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe { read_first_lane(self as _) as _ }
    }
}
impl ReadFirstLane for u32 {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe { read_first_lane(self) }
    }
}
#[cfg(target_pointer_width = "32")]
impl ReadFirstLane for usize {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let v = crate::mem::transmute(self);
            let v = read_first_lane(v);
            crate::mem::transmute(v)
        }
    }
}
#[cfg(target_pointer_width = "64")]
impl ReadFirstLane for usize {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let v: u64 = crate::mem::transmute(self);
            crate::mem::transmute(v.read_first_lane())
        }
    }
}

macro_rules! impl_read_first_lane_u32x {
  ($($ty:ty,)*) => {$(
      impl ReadFirstLane for $ty {
          #[inline(always)]
          unsafe fn read_first_lane(self) -> Self {
              unsafe {
                  let v: [u32; size_of::<$ty>() / size_of::<u32>()]
                      = crate::mem::transmute(self);
                  let v = v.read_first_lane();
                  crate::mem::transmute(v)
              }
          }
      }
  )*};
}
impl_read_first_lane_u32x!(i64, i128, u64, u128, );

impl<T> ReadFirstLane for *const T {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            (self as usize).read_first_lane() as Self
        }
    }
}
impl<T> ReadFirstLane for *mut T {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            (self as usize).read_first_lane() as Self
        }
    }
}

impl<T> ReadFirstLane for *const [T] {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let this: TraitObject = transmute(self);
            let this = TraitObject {
                data: this.data.read_first_lane(),
                vtable: this.vtable.read_first_lane(),
            };
            transmute(this)
        }
    }
}
impl<T> ReadFirstLane for *mut [T] {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe {
            let this: TraitObject = transmute(self);
            let this = TraitObject {
                data: this.data.read_first_lane(),
                vtable: this.vtable.read_first_lane(),
            };
            transmute(this)
        }
    }
}

impl<'a, T> ReadFirstLane for &'a T {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe { &*(self as *const T).read_first_lane() }
    }
}

impl<'a, T> ReadFirstLane for &'a [T] {
    #[inline(always)]
    unsafe fn read_first_lane(self) -> Self {
        unsafe { &*(self as *const [T]).read_first_lane() }
    }
}
