//! Builtin input and output variable types

use super::*;

pub trait SpirvBuiltinDetail {
    type Ty: Copy;
    const ID: u32;
    const NAME: &'static str;

    type RawTy: Copy;
    fn raw_ref(raw: &Self::RawTy) -> &Self::Ty;
    fn raw_mut(raw: &mut Self::RawTy) -> &mut Self::Ty;
}
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct PositionBuiltin;
impl SpirvBuiltinDetail for PositionBuiltin {
    type Ty = V4<f32>;
    const ID: u32 = 0;
    const NAME: &'static str = "Position";

    type RawTy = RawBuiltin<Self::Ty, 0>;
    #[inline(always)]
    fn raw_ref(raw: &Self::RawTy) -> &Self::Ty {
        &raw.0
    }
    #[inline(always)]
    fn raw_mut(raw: &mut Self::RawTy) -> &mut Self::Ty {
        &mut raw.0
    }
}
pub type PositionIn = BuiltinInput<PositionBuiltin>;
pub type PositionOutput = BuiltinOutput<PositionBuiltin>;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct FragDepthBuiltin;
impl SpirvBuiltinDetail for FragDepthBuiltin {
    type Ty = f32;
    const ID: u32 = 22;
    const NAME: &'static str = "FragDepth";

    type RawTy = RawBuiltin<Self::Ty, 22>;
    #[inline(always)]
    fn raw_ref(raw: &Self::RawTy) -> &Self::Ty {
        &raw.0
    }
    #[inline(always)]
    fn raw_mut(raw: &mut Self::RawTy) -> &mut Self::Ty {
        &mut raw.0
    }
}
pub type FragDepth = BuiltinOutput<FragDepthBuiltin>;

#[cfg_attr(not(stage0), lang = "spirv_builtin")]
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RawBuiltin<T, const ID: u32>(T);

/// Like `ShaderInput`, but doesn't require the location and component layout annotations.
/// This type *must* be used for statics only.
#[cfg_attr(not(stage0), lang = "spirv_input")]
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct BuiltinInput<B>(MaybeUninit<B::RawTy>)
    where B: SpirvBuiltinDetail;

/// Like `ShaderOutput`, but doesn't require the location and component layout annotations.
/// This type *must* be used for statics only.
#[cfg_attr(not(stage0), lang = "spirv_output")]
#[repr(transparent)]
pub struct BuiltinOutput<B>(UnsafeCell<B::RawTy>)
    where B: SpirvBuiltinDetail;

impl<B> BuiltinInput<B>
    where B: SpirvBuiltinDetail,
{
    #[inline(always)]
    pub const fn new() -> Self {
        BuiltinInput(MaybeUninit::uninit())
    }
}
impl<B> Deref for BuiltinInput<B>
    where B: SpirvBuiltinDetail,
{
    type Target = B::Ty;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        assert!(platform().is_spirv());
        // We're initialized by the platform
        unsafe { B::raw_ref(self.0.assume_init_ref()) }
    }
}
impl<B> fmt::Debug for BuiltinInput<B>
    where B: SpirvBuiltinDetail,
          B::Ty: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if platform().is_spirv() {
            f.debug_tuple(B::NAME)
                .field(&**self)
                .finish()
        } else {
            f.debug_tuple(B::NAME)
                .field(&"..")
                .finish()
        }
    }
}
unsafe impl<B> Sync for BuiltinInput<B>
    where B: SpirvBuiltinDetail,
{ }

impl<B> BuiltinOutput<B>
    where B: SpirvBuiltinDetail,
{
    #[inline(always)]
    pub const fn new<BTy, const BID: u32>(v: BTy) -> Self
        where B: SpirvBuiltinDetail<Ty = BTy, RawTy = RawBuiltin<BTy, {BID}>>,
              BTy: Copy,
    {
        BuiltinOutput(UnsafeCell::new(RawBuiltin(v)))
    }

    pub fn write(&self, v: B::Ty) {
        assert!(platform().is_spirv());
        unsafe {
            // These static variables are unique to the specific shader invocation.
            *B::raw_mut(&mut *self.0.get()) = v;
        }
    }
}
impl<B> Deref for BuiltinOutput<B>
    where B: SpirvBuiltinDetail,
{
    type Target = B::Ty;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        assert!(platform().is_spirv());
        // We're initialized by the platform
        unsafe { B::raw_ref(&*self.0.get()) }
    }
}
impl<B> DerefMut for BuiltinOutput<B>
    where B: SpirvBuiltinDetail,
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(platform().is_spirv());
        // We're initialized by the platform
        unsafe { B::raw_mut(&mut *self.0.get()) }
    }
}
impl<B> fmt::Debug for BuiltinOutput<B>
    where B: SpirvBuiltinDetail,
          B::Ty: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if platform().is_spirv() {
            f.debug_tuple(B::NAME)
                .field(&**self)
                .finish()
        } else {
            f.debug_tuple(B::NAME)
                .field(&"..")
                .finish()
        }
    }
}
unsafe impl<B> Sync for BuiltinOutput<B>
    where B: SpirvBuiltinDetail,
{ }
