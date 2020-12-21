
//! Types and functions to determine what platform code is actually running on.
//! Since Geobacter doesn't operate at the syntax level, attributes like `#[cfg()]`
//! don't work. So instead, we have a common enum definition here which includes
//! all supported accelerator devices. You can then query the platform at runtime
//! with the constant function provided. LLVM *should* then use the constant-ness
//! for const propagation and remove branches for other devices.

use crate::default::Default;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Platform {
    Host,

    // GPUs
    Hsa(self::hsa::AmdGpu),
    /// Present, but ATM completely unsupported.
    Cuda,
    Vulkan(self::spirv::ExeModel),
    OpenGl(self::spirv::ExeModel),
    OpenCl,

    /// Berkley Packet Filter; ATM unsupported/incomplete.
    Bpf,
}

pub mod hsa {
    use crate::prelude::v1::*;
    use crate::str::FromStr;

    /// These are taken from the AMDGPU LLVM target machine.
    /// TODO do we care about pre-GFX8 GPUs?
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub enum AmdGcn {
        //===----------------------------------------------------------------------===//
        // GCN GFX8 (Volcanic Islands (VI)).
        //===----------------------------------------------------------------------===//
        Gfx801,
        Carrizo,
        Gfx802,
        Iceland,
        Tonga,
        Gfx803,
        Fiji,
        Polaris10,
        Polaris11,
        Gfx810,
        Stoney,

        //===----------------------------------------------------------------------===//
        // GCN GFX9.
        //===----------------------------------------------------------------------===//

        Gfx900,
        Gfx902,
        Gfx904,
        Gfx906,
        Gfx909,
    }
    impl FromStr for AmdGcn {
        type Err = ();
        fn from_str(s: &str) -> Result<Self, ()> {
            use self::AmdGcn::*;

            let v = match s {
                "gfx801" => Gfx801,
                "carrizo" => Carrizo,
                "gfx802" => Gfx802,
                "iceland" => Iceland,
                "tonga" => Tonga,
                "gfx803" => Gfx803,
                "fiji" => Fiji,
                "polaris10" => Polaris10,
                "polaris11" => Polaris11,
                "gfx810" => Gfx810,
                "stoney" => Stoney,

                "gfx900" => Gfx900,
                "gfx902" => Gfx902,
                "gfx904" => Gfx904,
                "gfx906" => Gfx906,
                "gfx909" => Gfx909,

                _ => { return Err(()); },
            };

            Ok(v)
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    pub enum AmdGpu {
        AmdGcn(AmdGcn)
    }
}
pub mod spirv {
    use crate::prelude::v1::*;
    use crate::str::FromStr;

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
    #[repr(u32)]
    pub enum ExeModel {
        Vertex,
        TessellationControl,
        TessellationEval,
        Geometry,
        Fragment,
        /// Vulkan/OpenGL compute kernel.
        GLCompute,
        /// OpenCL kernel; here for completeness.
        Kernel,
    }
    impl FromStr for ExeModel {
        type Err = ();
        fn from_str(s: &str) -> Result<Self, ()> {
            let r = match s {
                "Vertex" => ExeModel::Vertex,
                "TessellationControl" => ExeModel::TessellationControl,
                "TessellationEval" => ExeModel::TessellationEval,
                "Geometry" => ExeModel::Geometry,
                "Fragment" => ExeModel::Fragment,
                "GLCompute" => ExeModel::GLCompute,
                "Kernel" => ExeModel::Kernel,
                _ => { return Err(()); },
            };

            Ok(r)
        }
    }
}

impl Platform {
    #[inline(always)]
    pub fn is_host(self) -> bool {
        match self {
            Platform::Host => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_hsa(self) -> bool {
        match self {
            Platform::Hsa(_) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_amdgcn(self) -> bool {
        match self {
            Platform::Hsa(self::hsa::AmdGpu::AmdGcn(_)) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_cuda(self) -> bool {
        match self {
            Platform::Cuda => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv(self) -> bool {
        match self {
            Platform::Vulkan(_) |
            Platform::OpenGl(_) |
            Platform::OpenCl => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv_vertex(self) -> bool {
        match self {
            Platform::Vulkan(self::spirv::ExeModel::Vertex) |
            Platform::OpenGl(self::spirv::ExeModel::Vertex) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv_tess_ctl(self) -> bool {
        match self {
            Platform::Vulkan(self::spirv::ExeModel::TessellationControl) |
            Platform::OpenGl(self::spirv::ExeModel::TessellationControl) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv_tess_eval(self) -> bool {
        match self {
            Platform::Vulkan(self::spirv::ExeModel::TessellationEval) |
            Platform::OpenGl(self::spirv::ExeModel::TessellationEval) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv_vert(self) -> bool {
        match self {
            Platform::Vulkan(self::spirv::ExeModel::Vertex) |
            Platform::OpenGl(self::spirv::ExeModel::Vertex) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv_geometry(self) -> bool {
        match self {
            Platform::Vulkan(self::spirv::ExeModel::Geometry) |
            Platform::OpenGl(self::spirv::ExeModel::Geometry) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv_fragment(self) -> bool {
        match self {
            Platform::Vulkan(self::spirv::ExeModel::Fragment) |
            Platform::OpenGl(self::spirv::ExeModel::Fragment) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_spirv_glcompute(self) -> bool {
        match self {
            Platform::Vulkan(self::spirv::ExeModel::GLCompute) |
            Platform::OpenGl(self::spirv::ExeModel::GLCompute) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_vulkan(self) -> bool {
        match self {
            Platform::Vulkan(_) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_opengl(self) -> bool {
        match self {
            Platform::OpenGl(_) => true,
            _ => false,
        }
    }
    #[inline(always)]
    pub fn is_opencl(self) -> bool {
        match self {
            Platform::OpenCl => true,
            _ => false,
        }
    }
}

impl Default for Platform {
    #[inline(always)]
    fn default() -> Platform {
        platform()
    }
}

#[cfg(not(bootstrap))]
#[inline(always)]
pub const fn platform() -> Platform {
    use crate::mem::{size_of, transmute, };

    extern "rust-intrinsic" {
        #[rustc_const_unstable(feature = "geobacter", issue = "none")]
        fn geobacter_platform() -> &'static [u8; size_of::<Platform>()];
    }

    let p: &'static Platform = unsafe {
        let r = geobacter_platform();
        transmute(r)
    };
    *p
}
#[cfg(bootstrap)]
#[inline(always)]
pub const fn platform() -> Platform {
    Platform::Host
}
