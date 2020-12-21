use crate::mir::mono::Linkage;
use rustc_attr::{InlineAttr, OptimizeAttr};
use rustc_session::config::SanitizerSet;
use rustc_span::symbol::Symbol;
use rustc_target::spec::AddrSpaceIdx;

#[derive(Clone, TyEncodable, TyDecodable, HashStable)]
pub struct CodegenFnAttrs {
    pub flags: CodegenFnAttrFlags,
    /// Parsed representation of the `#[inline]` attribute
    pub inline: InlineAttr,
    /// Parsed representation of the `#[optimize]` attribute
    pub optimize: OptimizeAttr,
    /// The `#[export_name = "..."]` attribute, indicating a custom symbol a
    /// function should be exported under
    pub export_name: Option<Symbol>,
    /// Attributes related to SPIRV
    pub spirv: Option<SpirVAttrs>,
    /// The `#[link_name = "..."]` attribute, indicating a custom symbol an
    /// imported function should be imported as. Note that `export_name`
    /// probably isn't set when this is set, this is for foreign items while
    /// `#[export_name]` is for Rust-defined functions.
    pub link_name: Option<Symbol>,
    /// The `#[link_ordinal = "..."]` attribute, indicating an ordinal an
    /// imported function has in the dynamic library. Note that this must not
    /// be set when `link_name` is set. This is for foreign items with the
    /// "raw-dylib" kind.
    pub link_ordinal: Option<usize>,
    /// AMDGPU specific VGPR constraint
    pub amdgpu_num_vgpr: Option<usize>,
    /// AMDGPU specific flag for when the grid size is guaranteed to be a multiple
    /// of the workgroup size.
    pub amdgpu_uniform_workgroup_size: Option<bool>,
    /// AMDGPU specific flat workgroup size attribute.
    pub amdgpu_flat_workgroup_size: Option<(usize, usize)>,
    /// The `#[target_feature(enable = "...")]` attribute and the enabled
    /// features (only enabled features are supported right now).
    pub target_features: Vec<Symbol>,
    /// The `#[linkage = "..."]` attribute and the value we found.
    pub linkage: Option<Linkage>,
    /// The `#[link_section = "..."]` attribute, or what executable section this
    /// should be placed in.
    pub link_section: Option<Symbol>,
    /// The `#[no_sanitize(...)]` attribute. Indicates sanitizers for which
    /// instrumentation should be disabled inside the annotated function.
    pub no_sanitize: SanitizerSet,
    /// The `#[addr_space = "..."]` attribute, or what address space this global
    /// should be place in. The value names are the same as used in target specs.
    pub addr_space: Option<AddrSpaceIdx>,
}

bitflags! {
    #[derive(TyEncodable, TyDecodable, HashStable)]
    pub struct CodegenFnAttrFlags: u32 {
        /// `#[cold]`: a hint to LLVM that this function, when called, is never on
        /// the hot path.
        const COLD                      = 1 << 0;
        /// `#[rustc_allocator]`: a hint to LLVM that the pointer returned from this
        /// function is never null.
        const ALLOCATOR                 = 1 << 1;
        /// `#[unwind]`: an indicator that this function may unwind despite what
        /// its ABI signature may otherwise imply.
        const UNWIND                    = 1 << 2;
        /// `#[rust_allocator_nounwind]`, an indicator that an imported FFI
        /// function will never unwind. Probably obsolete by recent changes with
        /// #[unwind], but hasn't been removed/migrated yet
        const RUSTC_ALLOCATOR_NOUNWIND  = 1 << 3;
        /// `#[naked]`: an indicator to LLVM that no function prologue/epilogue
        /// should be generated.
        const NAKED                     = 1 << 4;
        /// `#[no_mangle]`: an indicator that the function's name should be the same
        /// as its symbol.
        const NO_MANGLE                 = 1 << 5;
        /// `#[rustc_std_internal_symbol]`: an indicator that this symbol is a
        /// "weird symbol" for the standard library in that it has slightly
        /// different linkage, visibility, and reachability rules.
        const RUSTC_STD_INTERNAL_SYMBOL = 1 << 6;
        /// `#[thread_local]`: indicates a static is actually a thread local
        /// piece of memory
        const THREAD_LOCAL              = 1 << 8;
        /// `#[used]`: indicates that LLVM can't eliminate this function (but the
        /// linker can!).
        const USED                      = 1 << 9;
        /// `#[ffi_returns_twice]`, indicates that an extern function can return
        /// multiple times
        const FFI_RETURNS_TWICE         = 1 << 10;
        /// `#[track_caller]`: allow access to the caller location
        const TRACK_CALLER              = 1 << 11;
        /// #[ffi_pure]: applies clang's `pure` attribute to a foreign function
        /// declaration.
        const FFI_PURE                  = 1 << 12;
        /// #[ffi_const]: applies clang's `const` attribute to a foreign function
        /// declaration.
        const FFI_CONST                 = 1 << 13;
        /// #[cmse_nonsecure_entry]: with a TrustZone-M extension, declare a
        /// function as an entry function from Non-Secure code.
        const CMSE_NONSECURE_ENTRY      = 1 << 14;
    }
}

impl CodegenFnAttrs {
    pub fn new() -> CodegenFnAttrs {
        CodegenFnAttrs {
            flags: CodegenFnAttrFlags::empty(),
            inline: InlineAttr::None,
            optimize: OptimizeAttr::None,
            export_name: None,
            spirv: None,
            link_name: None,
            link_ordinal: None,
            amdgpu_num_vgpr: None,
            amdgpu_uniform_workgroup_size: None,
            amdgpu_flat_workgroup_size: None,
            target_features: vec![],
            linkage: None,
            link_section: None,
            no_sanitize: SanitizerSet::empty(),
            addr_space: None,
        }
    }

    /// Returns `true` if `#[inline]` or `#[inline(always)]` is present.
    pub fn requests_inline(&self) -> bool {
        match self.inline {
            InlineAttr::Hint | InlineAttr::Always => true,
            InlineAttr::None | InlineAttr::Never => false,
        }
    }

    /// Returns `true` if it looks like this symbol needs to be exported, for example:
    ///
    /// * `#[no_mangle]` is present
    /// * `#[export_name(...)]` is present
    /// * `#[linkage]` is present
    pub fn contains_extern_indicator(&self) -> bool {
        self.flags.contains(CodegenFnAttrFlags::NO_MANGLE)
            || self.export_name.is_some()
            || match self.linkage {
                // These are private, so make sure we don't try to consider
                // them external.
                None | Some(Linkage::Internal | Linkage::Private) => false,
                Some(_) => true,
            }
    }
}

#[derive(Clone, TyEncodable, TyDecodable, Debug, HashStable)]
pub struct SpirVImageTypeSpec {
    pub dim: String,
    pub depth: u32,
    pub arrayed: bool,
    pub multisampled: bool,
    pub sampled: u32,
    pub format: String,
}

#[derive(Clone, TyEncodable, TyDecodable, Debug, HashStable)]
pub struct SpirVStructMember {
    pub node: SpirVAttrNode,
    pub decorations: Vec<(String, Vec<u32>)>,
}

#[derive(Clone, TyEncodable, TyDecodable, Debug, HashStable)]
pub enum SpirVTypeSpec {
    Image(SpirVImageTypeSpec),
    SampledImage(SpirVImageTypeSpec),
    Struct(Vec<SpirVStructMember>),
    Array(Box<SpirVAttrNode>),
    Matrix {
        columns: u32,
        rows: u32,
        decorations: Vec<(String, Vec<u32>)>,
    },
}

#[derive(Clone, TyEncodable, TyDecodable, Debug, HashStable)]
pub struct SpirVAttrNode {
    pub type_spec: SpirVTypeSpec,
    pub decorations: Vec<(String, Vec<u32>)>,
}

#[derive(Clone, Debug, Default, HashStable, TyEncodable, TyDecodable)]
pub struct SpirVAttrs {
    pub storage_class: Option<String>,
    pub metadata: Option<SpirVAttrNode>,
    pub exe_model: Option<String>,
    pub exe_mode: Option<Vec<(String, Vec<u64>)>>,
    pub pipeline_binding: Option<u32>,
    pub pipeline_descriptor_set: Option<u32>,
}
