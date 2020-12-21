// Note regarding the metadata: many of the slices used here are used inplace of `Option`.
// If the array doesn't have any elements, then the array can be interpreted as `None`.
// Otherwise, the array should only have a single element.

use crate::convert::{Into, TryFrom};
use crate::result::Result;

pub type CompilerRawImgFormat = u32;

#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
#[allow(missing_docs)]
#[allow(non_camel_case_types)]
pub enum CompilerImgFormat {
    R4G4UnormPack8,
    R4G4B4A4UnormPack16,
    B4G4R4A4UnormPack16,
    R5G6B5UnormPack16,
    B5G6R5UnormPack16,
    R5G5B5A1UnormPack16,
    B5G5R5A1UnormPack16,
    A1R5G5B5UnormPack16,
    R8Unorm,
    R8Snorm,
    R8Uscaled,
    R8Sscaled,
    R8Uint,
    R8Sint,
    R8Srgb,
    R8G8Unorm,
    R8G8Snorm,
    R8G8Uscaled,
    R8G8Sscaled,
    R8G8Uint,
    R8G8Sint,
    R8G8Srgb,
    R8G8B8Unorm,
    R8G8B8Snorm,
    R8G8B8Uscaled,
    R8G8B8Sscaled,
    R8G8B8Uint,
    R8G8B8Sint,
    R8G8B8Srgb,
    B8G8R8Unorm,
    B8G8R8Snorm,
    B8G8R8Uscaled,
    B8G8R8Sscaled,
    B8G8R8Uint,
    B8G8R8Sint,
    B8G8R8Srgb,
    R8G8B8A8Unorm,
    R8G8B8A8Snorm,
    R8G8B8A8Uscaled,
    R8G8B8A8Sscaled,
    R8G8B8A8Uint,
    R8G8B8A8Sint,
    R8G8B8A8Srgb,
    B8G8R8A8Unorm,
    B8G8R8A8Snorm,
    B8G8R8A8Uscaled,
    B8G8R8A8Sscaled,
    B8G8R8A8Uint,
    B8G8R8A8Sint,
    B8G8R8A8Srgb,
    A8B8G8R8UnormPack32,
    A8B8G8R8SnormPack32,
    A8B8G8R8UscaledPack32,
    A8B8G8R8SscaledPack32,
    A8B8G8R8UintPack32,
    A8B8G8R8SintPack32,
    A8B8G8R8SrgbPack32,
    A2R10G10B10UnormPack32,
    A2R10G10B10SnormPack32,
    A2R10G10B10UscaledPack32,
    A2R10G10B10SscaledPack32,
    A2R10G10B10UintPack32,
    A2R10G10B10SintPack32,
    A2B10G10R10UnormPack32,
    A2B10G10R10SnormPack32,
    A2B10G10R10UscaledPack32,
    A2B10G10R10SscaledPack32,
    A2B10G10R10UintPack32,
    A2B10G10R10SintPack32,
    R16Unorm,
    R16Snorm,
    R16Uscaled,
    R16Sscaled,
    R16Uint,
    R16Sint,
    R16Sfloat,
    R16G16Unorm,
    R16G16Snorm,
    R16G16Uscaled,
    R16G16Sscaled,
    R16G16Uint,
    R16G16Sint,
    R16G16Sfloat,
    R16G16B16Unorm,
    R16G16B16Snorm,
    R16G16B16Uscaled,
    R16G16B16Sscaled,
    R16G16B16Uint,
    R16G16B16Sint,
    R16G16B16Sfloat,
    R16G16B16A16Unorm,
    R16G16B16A16Snorm,
    R16G16B16A16Uscaled,
    R16G16B16A16Sscaled,
    R16G16B16A16Uint,
    R16G16B16A16Sint,
    R16G16B16A16Sfloat,
    R32Uint,
    R32Sint,
    R32Sfloat,
    R32G32Uint,
    R32G32Sint,
    R32G32Sfloat,
    R32G32B32Uint,
    R32G32B32Sint,
    R32G32B32Sfloat,
    R32G32B32A32Uint,
    R32G32B32A32Sint,
    R32G32B32A32Sfloat,
    R64Uint,
    R64Sint,
    R64Sfloat,
    R64G64Uint,
    R64G64Sint,
    R64G64Sfloat,
    R64G64B64Uint,
    R64G64B64Sint,
    R64G64B64Sfloat,
    R64G64B64A64Uint,
    R64G64B64A64Sint,
    R64G64B64A64Sfloat,
    B10G11R11UfloatPack32,
    E5B9G9R9UfloatPack32,
    D16Unorm,
    X8_D24UnormPack32,
    D32Sfloat,
    S8Uint,
    D16Unorm_S8Uint,
    D24Unorm_S8Uint,
    D32Sfloat_S8Uint,
    BC1_RGBUnormBlock,
    BC1_RGBSrgbBlock,
    BC1_RGBAUnormBlock,
    BC1_RGBASrgbBlock,
    BC2UnormBlock,
    BC2SrgbBlock,
    BC3UnormBlock,
    BC3SrgbBlock,
    BC4UnormBlock,
    BC4SnormBlock,
    BC5UnormBlock,
    BC5SnormBlock,
    BC6HUfloatBlock,
    BC6HSfloatBlock,
    BC7UnormBlock,
    BC7SrgbBlock,
    ETC2_R8G8B8UnormBlock,
    ETC2_R8G8B8SrgbBlock,
    ETC2_R8G8B8A1UnormBlock,
    ETC2_R8G8B8A1SrgbBlock,
    ETC2_R8G8B8A8UnormBlock,
    ETC2_R8G8B8A8SrgbBlock,
    EAC_R11UnormBlock,
    EAC_R11SnormBlock,
    EAC_R11G11UnormBlock,
    EAC_R11G11SnormBlock,
    ASTC_4x4UnormBlock,
    ASTC_4x4SrgbBlock,
    ASTC_5x4UnormBlock,
    ASTC_5x4SrgbBlock,
    ASTC_5x5UnormBlock,
    ASTC_5x5SrgbBlock,
    ASTC_6x5UnormBlock,
    ASTC_6x5SrgbBlock,
    ASTC_6x6UnormBlock,
    ASTC_6x6SrgbBlock,
    ASTC_8x5UnormBlock,
    ASTC_8x5SrgbBlock,
    ASTC_8x6UnormBlock,
    ASTC_8x6SrgbBlock,
    ASTC_8x8UnormBlock,
    ASTC_8x8SrgbBlock,
    ASTC_10x5UnormBlock,
    ASTC_10x5SrgbBlock,
    ASTC_10x6UnormBlock,
    ASTC_10x6SrgbBlock,
    ASTC_10x8UnormBlock,
    ASTC_10x8SrgbBlock,
    ASTC_10x10UnormBlock,
    ASTC_10x10SrgbBlock,
    ASTC_12x10UnormBlock,
    ASTC_12x10SrgbBlock,
    ASTC_12x12UnormBlock,
    ASTC_12x12SrgbBlock,
}

#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum CompilerDescriptorDescTyKind {
    Sampler = 0,
    CombinedImageSampler,
    Image,
    TexelBuffer,
    InputAttachment,
    Buffer,
}
impl Into<u32> for CompilerDescriptorDescTyKind {
    #[inline(always)]
    fn into(self) -> u32 {
        self as _
    }
}
impl TryFrom<u32> for CompilerDescriptorDescTyKind {
    type Error = ();
    #[inline(always)]
    fn try_from(v: u32) -> Result<Self, ()> {
        use self::CompilerDescriptorDescTyKind::*;
        Result::Ok(match v {
            0 => Sampler,
            1 => CombinedImageSampler,
            2 => Image,
            3 => TexelBuffer,
            4 => InputAttachment,
            5 => Buffer,
            _ => { return Result::Err(()); },
        })
    }
}
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum CompilerDescriptorImageDims {
    Dim1 = 0,
    Dim2,
    Dim3,
    Cube,
}
impl Into<u32> for CompilerDescriptorImageDims {
    #[inline(always)]
    fn into(self) -> u32 {
        self as _
    }
}
impl TryFrom<u32> for CompilerDescriptorImageDims {
    type Error = ();
    #[inline(always)]
    fn try_from(v: u32) -> Result<Self, ()> {
        use self::CompilerDescriptorImageDims::*;
        Result::Ok(match v {
            0 => Dim1,
            1 => Dim2,
            2 => Dim3,
            3 => Cube,
            _ => { return Result::Err(()); },
        })
    }
}

pub type CompilerDescriptorImageArray = (bool, &'static [u32]);
pub type CompilerDescriptorImageDesc = (bool,
                                        u32 /*CompilerDescriptorImageDims*/,
                                        &'static [CompilerRawImgFormat],
                                        bool,
                                        CompilerDescriptorImageArray,
);
pub type CompilerDescriptorBufferDesc = (&'static [bool], bool);


pub type CompilerDescriptorDescTy = (u32 /*CompilerDescriptorDescTyKind*/,
                                     // If self.0 is CombinedImageSampler
                                     &'static [CompilerDescriptorImageDesc],
                                     // If self.0 is Image
                                     &'static [CompilerDescriptorImageDesc],
                                     // If self.0 is TexelBuffer
                                     &'static [(bool, &'static [CompilerRawImgFormat])],
                                     // If self.0 is InputAttachment
                                     &'static [(bool, CompilerDescriptorImageArray)],
                                     // If self.0 is Buffer
                                     &'static [CompilerDescriptorBufferDesc],
);

/// vertex, tessellation_control, tessellation_evalutation, geometry, fragment, compute
pub type CompilerShaderStages = (bool, bool, bool, bool, bool, bool);
/// binding id, ty, array_count, stages, readonly
pub type CompilerDescriptorDesc = (u32, CompilerDescriptorDescTy, u32, CompilerShaderStages, bool);
/// set id, descriptors
pub type CompilerDescriptorBindingsDesc = (u32, &'static [CompilerDescriptorDesc]);
pub type CompilerDescriptorSetBindingsDesc = &'static [CompilerDescriptorBindingsDesc];
