
use crate::alloc::{Layout, LayoutErr};
use crate::cell::UnsafeCell;
use crate::convert::{AsRef, AsMut};
use crate::default::Default;
use crate::geobacter::platform::platform;
use crate::iter::ExactSizeIterator;
use crate::marker::Copy;
use crate::mem::MaybeUninit;
use crate::ops::{Deref, DerefMut, Drop};
use crate::ptr::{slice_from_raw_parts, slice_from_raw_parts_mut, drop_in_place};
use crate::{fmt, ptr};

pub mod builtin;
pub mod matrix;
pub mod pipeline_layout;
pub mod shader_interface;
pub mod workitem;

#[repr(simd)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct V2<T>(pub T, pub T);
#[repr(simd)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct V3<T>(pub T, pub T, pub T);
#[repr(simd)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct V4<T>(pub T, pub T, pub T, pub T);

pub trait SetBinding {
    const SET: u32;
    const BINDING: u32;

    #[inline(always)]
    fn set(&self) -> u32 { Self::SET }
    #[inline(always)]
    fn binding(&self) -> u32 { Self::BINDING }
}
pub trait ShaderInterface {
    const LOCATION: u32;

    #[inline(always)]
    fn location(&self) -> u32 { Self::LOCATION }
}

/// This type *must* be used for statics only.
#[cfg_attr(not(stage0), lang = "spirv_shader_input")]
#[repr(transparent)]
pub struct ShaderInput<T, const LOC: u32>(MaybeUninit<T>)
    where T: Copy;
impl<T, const LOC: u32> ShaderInput<T, {LOC}>
    where T: Copy,
{
    #[inline(always)]
    pub const fn new() -> Self {
        ShaderInput(MaybeUninit::uninit())
    }
}
impl<T, const LOC: u32> Deref for ShaderInput<T, {LOC}>
    where T: Copy,
{
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        assert!(platform().is_spirv());
        unsafe {
            // We're initialized by the platform (Vulkan, OpenGL, or OpenCL).
            self.0.assume_init_ref()
        }
    }
}
impl<T, const LOC: u32> fmt::Debug for ShaderInput<T, {LOC}>
    where T: Copy + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if platform().is_spirv() {
            f.debug_struct("Input")
                .field("location", &LOC)
                .field("value", &self.0)
                .finish()
        } else {
            f.debug_struct("Input")
                .field("location", &LOC)
                .finish()
        }
    }
}
impl<T, const LOC: u32> ShaderInterface for ShaderInput<T, {LOC}>
    where T: Copy,
{
    const LOCATION: u32 = LOC;
}

/// This type *must* be used for statics only.
#[cfg_attr(not(stage0), lang = "spirv_shader_output")]
#[repr(transparent)]
pub struct ShaderOutput<T, const LOC: u32>(UnsafeCell<T>)
  where T: Copy;
impl<T, const LOC: u32> ShaderOutput<T, {LOC}>
    where T: Copy,
{
    #[inline(always)]
    pub const fn new(v: T) -> Self {
        ShaderOutput(UnsafeCell::new(v))
    }

    #[inline(always)]
    pub fn write(&self, v: T) {
        assert!(platform().is_spirv());
        unsafe {
            *self.0.get() = v;
        }
    }
}
impl<T, const LOC: u32> Deref for ShaderOutput<T, {LOC}>
    where T: Copy,
{
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        assert!(platform().is_spirv());
        unsafe { &*self.0.get() }
    }
}
impl<T, const LOC: u32> DerefMut for ShaderOutput<T, {LOC}>
    where T: Copy,
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        assert!(platform().is_spirv());
        unsafe { &mut *self.0.get() }
    }
}
impl<T, const LOC: u32> fmt::Debug for ShaderOutput<T, {LOC}>
    where T: Copy + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if platform().is_spirv() {
            f.debug_struct("Output")
                .field("location", &LOC)
                .field("value", unsafe { &*self.0.get() })
                .finish()
        } else {
            f.debug_struct("Output")
                .field("location", &LOC)
                .finish()
        }
    }
}
impl<T, const LOC: u32> ShaderInterface for ShaderOutput<T, {LOC}>
    where T: Copy,
{
    const LOCATION: u32 = LOC;
}

/// This type *must* be used for statics only.
#[cfg_attr(not(stage0), lang = "spirv_uniform_object")]
#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct Uniform<T, const SET: u32, const BINDING: u32>(T);
impl<T, const SET: u32, const BINDING: u32> Uniform<T, {SET}, {BINDING}> {
    #[inline(always)]
    pub const fn new(v: T) -> Self {
        Uniform(v)
    }
}
impl<T, const SET: u32, const BINDING: u32> Deref for Uniform<T, {SET}, {BINDING}> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T { &self.0 }
}
impl<T, const SET: u32, const BINDING: u32> fmt::Debug for Uniform<T, {SET}, {BINDING}>
    where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if platform().is_spirv() {
            f.debug_struct("Uniform")
                .field("descriptor_set", &(SET, BINDING))
                .field("value", &self.0)
                .finish()
        } else {
            f.debug_struct("Uniform")
                .field("descriptor_set", &(SET, BINDING))
                .finish()
        }
    }
}
impl<T, const SET: u32, const BINDING: u32> SetBinding for Uniform<T, {SET}, {BINDING}> {
    const SET: u32 = SET;
    const BINDING: u32 = BINDING;
}

/// This type *must* be used for statics only.
#[cfg_attr(not(stage0), lang = "spirv_buffer_object")]
#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct Buffer<T, const SET: u32, const BINDING: u32>(T);
impl<T, const SET: u32, const BINDING: u32> Buffer<T, {SET}, {BINDING}> {
    #[inline(always)]
    pub const fn new(v: T) -> Self {
        Buffer(v)
    }
}
impl<T, const SET: u32, const BINDING: u32> Deref for Buffer<T, {SET}, {BINDING}> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T { &self.0 }
}
impl<T, const SET: u32, const BINDING: u32> DerefMut for Buffer<T, {SET}, {BINDING}> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T { &mut self.0 }
}
impl<T, const SET: u32, const BINDING: u32> fmt::Debug for Buffer<T, {SET}, {BINDING}>
    where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Buffer")
            .field(&self.0)
            .finish()
    }
}
impl<T, const SET: u32, const BINDING: u32> SetBinding for Buffer<T, {SET}, {BINDING}> {
    const SET: u32 = SET;
    const BINDING: u32 = BINDING;
}

// SPIRV has an instruction which returns the runtime length of a runtime array,
// but it's actually pretty weird: the "pointer" argument isn't to the structure member
// which is the runtime array type (presumably the last member), it's to the structure
// itself! So instead of using OpArrayLength, we store the length here.
pub struct RuntimeArray32<T>(u32, [MaybeUninit<T>; 1]);
impl<T> RuntimeArray32<T> {
    #[inline(always)]
    pub const unsafe fn new_raw(len: u32) -> Self {
        RuntimeArray32(len, [MaybeUninit::uninit(); 1])
    }
    #[inline(always)]
    pub const fn new() -> Self {
        unsafe { Self::new_raw(0) }
    }

    #[inline(always)]
    pub fn layout(len: u32) -> Result<Layout, LayoutErr> {
        let slice_l = Layout::array::<T>(len as _)?;
        let len_l = Layout::new::<u32>();
        let padding = len_l.padding_needed_for(slice_l.align());
        let size = len_l.size() + padding + slice_l.size();
        Ok(unsafe {
            Layout::from_size_align_unchecked(size, len_l.align())
        })
    }

    #[inline(always)]
    pub unsafe fn set_len(&mut self, len: u32) {
        self.0 = len;
    }

    #[inline(always)]
    pub unsafe fn initialize_copy_from_slice(&mut self, slice: &[T])
        where T: Copy,
    {
        unsafe { self.set_len(slice.len() as _) };
        self.copy_from_slice(slice);
    }

    #[inline]
    pub unsafe fn initialize(&mut self, iter: impl ExactSizeIterator<Item = T>) {
        unsafe { self.set_len(iter.len() as _) };
        for (v, out) in iter.zip(self.iter_mut()) {
            unsafe { ptr::write(out, v) };
        }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const T {
        self.1.as_ptr() as *const T
    }
    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.1.as_mut_ptr() as *mut T
    }
    #[inline(always)]
    pub fn as_uninit_ptr(&self) -> *const MaybeUninit<T> {
        self.1.as_ptr()
    }
    #[inline(always)]
    pub fn as_uninit_mut_ptr(&mut self) -> *mut MaybeUninit<T> {
        self.1.as_mut_ptr()
    }

    #[inline(always)]
    pub const fn len(&self) -> u32 { self.0 }
}
impl<T> RuntimeArray32<T> {
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        let ptr = self.1.as_ptr() as *const T;
        unsafe {
            &*slice_from_raw_parts(ptr, self.len() as _)
        }
    }
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let ptr = self.1.as_mut_ptr() as *mut T;
        unsafe {
            &mut *slice_from_raw_parts_mut(ptr, self.len() as _)
        }
    }
}
impl<T> AsRef<[T]> for RuntimeArray32<T> {
    #[inline(always)]
    fn as_ref(&self) -> &[T] { self.as_slice() }
}
impl<T> AsMut<[T]> for RuntimeArray32<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] { self.as_mut_slice() }
}
impl<T> fmt::Debug for RuntimeArray32<T>
    where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RuntimeArray")
            .field(&self.as_ref())
            .finish()
    }
}
impl<T> Default for RuntimeArray32<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}
impl<T> Deref for RuntimeArray32<T> {
    type Target = [T];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}
impl<T> DerefMut for RuntimeArray32<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}
impl<T> Drop for RuntimeArray32<T> {
    fn drop(&mut self) {
        unsafe { drop_in_place(self.as_mut_slice()); }
    }
}
