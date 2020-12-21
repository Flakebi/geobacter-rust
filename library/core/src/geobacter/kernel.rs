
use crate::fmt;
use crate::option::{*, Option::None};
use crate::ops::*;
use crate::cmp::{Ordering, PartialEq, Ord, Eq, PartialOrd};
use crate::hash::{Hash, Hasher};

/// roughly corresponds to a `ty::Instance` in `rustc`.
#[derive(Clone, Copy)]
pub struct KernelInstanceRef<'a> {
    /// A debug friendly name.
    pub name: &'a str,
    /// The serialized `ty::Instance<'tcx>`.
    pub instance: &'a [u8],
}
impl<'a> fmt::Debug for KernelInstanceRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("KernelInstanceRef")
            .field(&self.name)
            .finish()
    }
}
impl<'a> Eq for KernelInstanceRef<'a> { }
impl<'a> Ord for KernelInstanceRef<'a> {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.instance.cmp(&rhs.instance)
    }
}
impl<'a, 'b> PartialEq<KernelInstanceRef<'b>> for KernelInstanceRef<'a> {
    fn eq(&self, rhs: &KernelInstanceRef<'b>) -> bool {
        self.instance.eq(rhs.instance)
    }
}
impl<'a, 'b> PartialOrd<KernelInstanceRef<'b>> for KernelInstanceRef<'a> {
    fn partial_cmp(&self, rhs: &KernelInstanceRef<'b>) -> Option<Ordering> {
        self.instance.partial_cmp(&rhs.instance)
    }
}
impl<'a> Hash for KernelInstanceRef<'a> {
    fn hash<H>(&self, hasher: &mut H)
        where H: Hasher,
    {
        self.instance.hash(hasher)
    }
}

pub trait OptionalKernelFn<Args> {
    type Output;
    fn call_optionally(&self, args: Args) -> Self::Output;

    fn has_instance(&self) -> bool;

    fn kernel_instance_opt(&self) -> Option<KernelInstanceRef<'static>>;
    #[inline(always)]
    fn kernel_instance(&self) -> KernelInstanceRef<'static>
        where Self: Fn<Args, Output = <Self as OptionalKernelFn<Args>>::Output>,
    {
        self.kernel_instance_opt().unwrap()
    }
}
impl OptionalKernelFn<()> for () {
    type Output = ();
    fn call_optionally(&self, _: ()) -> Self::Output {
        ()
    }

    fn has_instance(&self) -> bool { false }

    fn kernel_instance_opt(&self) -> Option<KernelInstanceRef<'static>> { None }
}
impl<F, Args> OptionalKernelFn<Args> for F
    where F: Fn<Args>,
{
    type Output = <F as FnOnce<Args>>::Output;
    fn call_optionally(&self, args: Args) -> <F as FnOnce<Args>>::Output {
        self.call(args)
    }

    fn has_instance(&self) -> bool {
        cfg!(not(bootstrap))
    }

    #[cfg(not(bootstrap))]
    fn kernel_instance_opt(&self) -> Option<KernelInstanceRef<'static>> {
        let instance = unsafe {
            super::intrinsics::geobacter_kernel_instance::<F, Args, _>()
        };

        instance.get(0)
            .map(|&(name, instance)| {
                KernelInstanceRef {
                    name,
                    instance,
                }
            })
    }
    #[cfg(bootstrap)]
    fn kernel_instance_opt(&self) -> Option<KernelInstanceRef<'static>> {
        None
    }
}
