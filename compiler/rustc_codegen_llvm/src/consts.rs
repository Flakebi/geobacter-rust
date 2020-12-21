use crate::base;
use crate::common::{CodegenCx, val_addr_space, val_addr_space_opt};
use crate::debuginfo;
use crate::llvm::{self, True};
use crate::type_::Type;
use crate::type_of::LayoutLlvmExt;
use crate::value::Value;
use libc::c_uint;
use rustc_codegen_ssa::traits::*;
use rustc_data_structures::const_cstr;
use rustc_hir as hir;
use rustc_hir::def_id::DefId;
use rustc_hir::Node;
use rustc_middle::middle::codegen_fn_attrs::{CodegenFnAttrFlags, CodegenFnAttrs, SpirVImageTypeSpec,
                                             SpirVAttrNode, SpirVTypeSpec};
use rustc_middle::mir::interpret::{
    read_target_uint, Allocation, ErrorHandled, Pointer,
};
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::{self, Instance, Ty};
use rustc_middle::{bug, span_bug};
use rustc_span::symbol::sym;
use rustc_span::Span;
use rustc_target::abi::{Align, HasDataLayout, LayoutOf, Primitive, Scalar, Size};
use rustc_target::spec::AddrSpaceIdx;
use tracing::debug;

use std::ffi::CStr;

pub fn const_alloc_to_llvm(cx: &CodegenCx<'ll, '_>, alloc: &Allocation) -> &'ll Value {
    let mut llvals = Vec::with_capacity(alloc.relocations().len() + 1);
    let dl = cx.data_layout();
    let pointer_size = dl.pointer_size.bytes() as usize;

    let mut next_offset = 0;
    for &(offset, ((), alloc_id)) in alloc.relocations().iter() {
        let offset = offset.bytes();
        assert_eq!(offset as usize as u64, offset);
        let offset = offset as usize;
        if offset > next_offset {
            // This `inspect` is okay since we have checked that it is not within a relocation, it
            // is within the bounds of the allocation, and it doesn't affect interpreter execution
            // (we inspect the result after interpreter execution). Any undef byte is replaced with
            // some arbitrary byte value.
            //
            // FIXME: relay undef bytes to codegen as undef const bytes
            let bytes = alloc.inspect_with_uninit_and_ptr_outside_interpreter(next_offset..offset);
            llvals.push(cx.const_bytes(bytes));
        }
        let ptr_offset = read_target_uint(
            dl.endian,
            // This `inspect` is okay since it is within the bounds of the allocation, it doesn't
            // affect interpreter execution (we inspect the result after interpreter execution),
            // and we properly interpret the relocation as a relocation pointer offset.
            alloc.inspect_with_uninit_and_ptr_outside_interpreter(offset..(offset + pointer_size)),
        )
        .expect("const_alloc_to_llvm: could not read relocation pointer")
            as u64;
        llvals.push(cx.scalar_to_backend(
            Pointer::new(alloc_id, Size::from_bytes(ptr_offset)).into(),
            &Scalar { value: Primitive::Pointer, valid_range: 0..=!0 },
            cx.type_i8p(),
        ));
        next_offset = offset + pointer_size;
    }
    if alloc.len() >= next_offset {
        let range = next_offset..alloc.len();
        // This `inspect` is okay since we have check that it is after all relocations, it is
        // within the bounds of the allocation, and it doesn't affect interpreter execution (we
        // inspect the result after interpreter execution). Any undef byte is replaced with some
        // arbitrary byte value.
        //
        // FIXME: relay undef bytes to codegen as undef const bytes
        let bytes = alloc.inspect_with_uninit_and_ptr_outside_interpreter(range);
        llvals.push(cx.const_bytes(bytes));
    }

    let out = cx.const_struct(&llvals, true);

    if alloc.relocations().len() == 0 {
        let imask = alloc.init_mask();
        if (0..alloc.len()).all(|i| !imask.get(Size::from_bytes(i)) ) {
            // use a real undef value:
            let ty = crate::common::val_ty(out);
            return cx.const_undef(ty);
        }
    }

    out
}

pub fn codegen_static_initializer(
    cx: &CodegenCx<'ll, 'tcx>,
    def_id: DefId,
) -> Result<(&'ll Value, &'tcx Allocation), ErrorHandled> {
    let alloc = cx.tcx.eval_static_initializer(def_id)?;
    Ok((const_alloc_to_llvm(cx, alloc), alloc))
}

fn set_global_alignment(cx: &CodegenCx<'ll, '_>, gv: &'ll Value, mut align: Align) {
    // The target may require greater alignment for globals than the type does.
    // Note: GCC and Clang also allow `__attribute__((aligned))` on variables,
    // which can force it to be smaller.  Rust doesn't support this yet.
    if let Some(min) = cx.sess().target.target.options.min_global_align {
        match Align::from_bits(min) {
            Ok(min) => align = align.max(min),
            Err(err) => {
                cx.sess().err(&format!("invalid minimum global alignment: {}", err));
            }
        }
    }
    unsafe {
        llvm::LLVMSetAlignment(gv, align.bytes() as u32);
    }
}

fn check_and_apply_linkage(
    cx: &CodegenCx<'ll, 'tcx>,
    attrs: &CodegenFnAttrs,
    ty: Ty<'tcx>,
    sym: &str,
    span: Span,
) -> &'ll Value {
    let llty = cx.layout_of(ty).llvm_type(cx);
    let addr_space = attrs.addr_space
      .unwrap_or(cx.flat_addr_space());
    if let Some(linkage) = attrs.linkage {
        debug!("get_static: sym={} linkage={:?}", sym, linkage);

        // If this is a static with a linkage specified, then we need to handle
        // it a little specially. The typesystem prevents things like &T and
        // extern "C" fn() from being non-null, so we can't just declare a
        // static and call it a day. Some linkages (like weak) will make it such
        // that the static actually has a null value.
        let llty2 = if let ty::RawPtr(ref mt) = ty.kind() {
            cx.layout_of(mt.ty).llvm_type(cx)
        } else {
            cx.sess().span_fatal(
                span,
                "must have type `*const T` or `*mut T` due to `#[linkage]` attribute",
            )
        };
        unsafe {
            // Declare a symbol `foo` with the desired linkage.
            let g1 = cx.declare_global(&sym, llty2, addr_space);
            llvm::LLVMRustSetLinkage(g1, base::linkage_to_llvm(linkage));

            // Declare an internal global `extern_with_linkage_foo` which
            // is initialized with the address of `foo`.  If `foo` is
            // discarded during linking (for example, if `foo` has weak
            // linkage and there are no definitions), then
            // `extern_with_linkage_foo` will instead be initialized to
            // zero.
            let mut real_name = "_rust_extern_with_linkage_".to_string();
            real_name.push_str(&sym);
            let g2 = cx.define_global(&real_name, llty, addr_space)
                .unwrap_or_else(|| {
                    cx.sess().span_fatal(span, &format!("symbol `{}` is already defined", &sym))
                });
            llvm::LLVMRustSetLinkage(g2, llvm::Linkage::InternalLinkage);
            llvm::LLVMSetInitializer(g2, g1);
            g2
        }
    } else {
        // Generate an external declaration.
        // FIXME(nagisa): investigate whether it can be changed into define_global
        cx.declare_global(&sym, llty, addr_space)
    }
}

/// Won't change address spaces
pub fn ptrcast(val: &'ll Value, ty: &'ll Type) -> &'ll Value {
    let ty = ty.copy_addr_space(val_addr_space(val));
    unsafe { llvm::LLVMConstPointerCast(val, ty) }
}

impl CodegenCx<'ll, 'tcx> {
    crate fn const_bitcast(&self, val: &'ll Value, ty: &'ll Type) -> &'ll Value {
        let ty = if let Some(addr_space) = val_addr_space_opt(val) {
            ty.copy_addr_space(addr_space)
        } else {
            ty
        };
        unsafe { llvm::LLVMConstBitCast(val, ty) }
    }

    crate fn const_addrcast(&self, val: &'ll Value, addr_space: AddrSpaceIdx) -> &'ll Value {
        let src_ty = self.val_ty(val);
        if src_ty.is_ptr() && src_ty.address_space() != addr_space {
            let dest_ty = src_ty.copy_addr_space(addr_space);
            self.check_addr_space_cast(val, dest_ty);
            unsafe {
                llvm::LLVMConstAddrSpaceCast(val, dest_ty)
            }
        } else {
            val
        }
    }

    crate fn static_addr_of_mut(
        &self,
        cv: &'ll Value,
        align: Align,
        kind: Option<&str>,
        addr_space: AddrSpaceIdx,
    ) -> &'ll Value {
        unsafe {
            let gv = match kind {
                Some(kind) if !self.tcx.sess.fewer_names() => {
                    let name = self.generate_local_symbol_name(kind);
                    let gv = self.define_global(&name[..],
                        self.val_ty(cv), addr_space).unwrap_or_else(|| {
                            bug!("symbol `{}` is already defined", name);
                    });
                    llvm::LLVMRustSetLinkage(gv, llvm::Linkage::PrivateLinkage);
                    gv
                }
                _ => self.define_private_global(self.val_ty(cv), addr_space),
            };
            llvm::LLVMSetInitializer(gv, cv);
            set_global_alignment(&self, gv, align);
            llvm::SetUnnamedAddress(gv, llvm::UnnamedAddr::Global);
            gv
        }
    }

    crate fn get_static(&self, def_id: DefId) -> &'ll Value {
        let instance = Instance::mono(self.tcx, def_id);
        if let Some(&g) = self.instances.borrow().get(&instance) {
            return g;
        }

        let defined_in_current_codegen_unit =
            self.codegen_unit.items().contains_key(&MonoItem::Static(def_id));
        assert!(
            !defined_in_current_codegen_unit,
            "consts::get_static() should always hit the cache for \
                 statics defined in the same CGU, but did not for `{:?}`",
            def_id
        );

        let ty = instance.ty(self.tcx, ty::ParamEnv::reveal_all());
        let sym = self.tcx.symbol_name(instance).name;
        let cg_attrs = self.tcx.codegen_fn_attrs(def_id);

        debug!("get_static: sym={} instance={:?}", sym, instance);

        let g = if let Some(def_id) = def_id.as_local() {
            let id = self.tcx.hir().local_def_id_to_hir_id(def_id);
            let llty = self.layout_of(ty).llvm_type(self);
            // FIXME: refactor this to work without accessing the HIR
            let (g, attrs) = match self.tcx.hir().get(id) {
                Node::Item(&hir::Item { attrs, span, kind: hir::ItemKind::Static(_, m, _), .. }) => {
                    if let Some(g) = self.get_declared_value(sym) {
                        if self.val_ty(g) != self.type_ptr_to(llty) {
                            span_bug!(span, "Conflicting types for static");
                        }
                    }
                    let freeze = self.type_is_freeze(ty);
                    let addr_space = if m == hir::Mutability::Mut || !freeze {
                        self.mutable_addr_space()
                    } else {
                        self.const_addr_space()
                    };
                    let addr_space = cg_attrs.addr_space
                      .unwrap_or(addr_space);

                    let g = self.declare_global(sym, llty, addr_space);

                    if !self.tcx.is_reachable_non_generic(def_id) {
                        unsafe {
                            llvm::LLVMRustSetVisibility(g, llvm::Visibility::Hidden);
                        }
                    }

                    (g, attrs)
                }

                Node::ForeignItem(&hir::ForeignItem {
                    attrs,
                    span,
                    kind: hir::ForeignItemKind::Static(..),
                    ..
                }) => {
                    (check_and_apply_linkage(&self, &cg_attrs, ty, sym, span), attrs)
                }

                item => bug!("get_static: expected static, found {:?}", item),
            };

            debug!("get_static: sym={} attrs={:?}", sym, attrs);

            for attr in attrs {
                if self.tcx.sess.check_name(attr, sym::thread_local) {
                    llvm::set_thread_local_mode(g, self.tls_model);
                }
            }

            g
        } else {
            // FIXME(nagisa): perhaps the map of externs could be offloaded to llvm somehow?
            debug!("get_static: sym={} item_attr={:?}", sym, self.tcx.item_attrs(def_id));

            let span = self.tcx.def_span(def_id);
            let g = check_and_apply_linkage(&self, &cg_attrs, ty, sym, span);

            // Thread-local statics in some other crate need to *always* be linked
            // against in a thread-local fashion, so we need to be sure to apply the
            // thread-local attribute locally if it was present remotely. If we
            // don't do this then linker errors can be generated where the linker
            // complains that one object files has a thread local version of the
            // symbol and another one doesn't.
            if cg_attrs.flags.contains(CodegenFnAttrFlags::THREAD_LOCAL) {
                llvm::set_thread_local_mode(g, self.tls_model);
            }

            let needs_dll_storage_attr = self.use_dll_storage_attrs && !self.tcx.is_foreign_item(def_id) &&
                // ThinLTO can't handle this workaround in all cases, so we don't
                // emit the attrs. Instead we make them unnecessary by disallowing
                // dynamic linking when linker plugin based LTO is enabled.
                !self.tcx.sess.opts.cg.linker_plugin_lto.enabled();

            // If this assertion triggers, there's something wrong with commandline
            // argument validation.
            debug_assert!(
                !(self.tcx.sess.opts.cg.linker_plugin_lto.enabled()
                    && self.tcx.sess.target.target.options.is_like_windows
                    && self.tcx.sess.opts.cg.prefer_dynamic)
            );

            if needs_dll_storage_attr {
                // This item is external but not foreign, i.e., it originates from an external Rust
                // crate. Since we don't know whether this crate will be linked dynamically or
                // statically in the final application, we always mark such symbols as 'dllimport'.
                // If final linkage happens to be static, we rely on compiler-emitted __imp_ stubs
                // to make things work.
                //
                // However, in some scenarios we defer emission of statics to downstream
                // crates, so there are cases where a static with an upstream DefId
                // is actually present in the current crate. We can find out via the
                // is_codegened_item query.
                if !self.tcx.is_codegened_item(def_id) {
                    unsafe {
                        llvm::LLVMSetDLLStorageClass(g, llvm::DLLStorageClass::DllImport);
                    }
                }
            }
            g
        };

        if self.use_dll_storage_attrs && self.tcx.is_dllimport_foreign_item(def_id) {
            // For foreign (native) libs we know the exact storage type to use.
            unsafe {
                llvm::LLVMSetDLLStorageClass(g, llvm::DLLStorageClass::DllImport);
            }
        }

        self.instances.borrow_mut().insert(instance, g);
        g
    }

    fn md_string(&self, s: &str) -> &'ll Value {
        unsafe {
            llvm::LLVMMDStringInContext(self.llcx, s.as_ptr() as *const _,
                                        s.len() as _)
        }
    }
    fn md_node(&self, values: &[&'ll Value]) -> &'ll Value {
        unsafe {
            llvm::LLVMMDNodeInContext(self.llcx,
                                      values.as_ptr() as *const _,
                                      values.len() as _)
        }
    }
    fn set_metadata(&self, value: &'ll Value, kind: &'static str,
                    node: &'ll Value)
    {
        assert!(kind.ends_with("\0"));

        unsafe {
            let kind_id =
                llvm::LLVMRustGetMDKindID(self.llcx,
                                          kind.as_ptr() as *const _);
            llvm::LLVMRustGlobalObjectSetMetadata(value, kind_id, node);
        }
    }

    pub fn add_amdgpu_attributes(&self, g: &'ll Value, attrs: &CodegenFnAttrs) {
        const NUM_VGPRS_KIND: &'static CStr = unsafe {
            CStr::from_bytes_with_nul_unchecked(b"amdgpu-num-vgpr\0")
        };
        const UNIFORM_WG_SIZE_KIND: &'static CStr = unsafe {
            CStr::from_bytes_with_nul_unchecked(b"uniform-work-group-size\0")
        };
        const FLAT_WG_SIZE_KIND: &'static CStr = unsafe {
            CStr::from_bytes_with_nul_unchecked(b"amdgpu-flat-work-group-size\0")
        };

        if self.tcx.sess.target.target.arch != "amdgpu" {
            return;
        }

        let idx = llvm::AttributePlace::Function;

        if let Some(num_vgprs) = attrs.amdgpu_num_vgpr {
            let num_vgprs = format!("{}\0", num_vgprs);
            let attr = unsafe {
                CStr::from_bytes_with_nul_unchecked(num_vgprs.as_ref())
            };
            llvm::AddFunctionAttrStringValue(g, idx, NUM_VGPRS_KIND,
                                             attr);
        }
        if let Some(uniform_wg_size) = attrs.amdgpu_uniform_workgroup_size {
            let attr = if uniform_wg_size { "true\0" } else { "false\0" };
            let attr = unsafe {
                CStr::from_bytes_with_nul_unchecked(attr.as_ref())
            };
            llvm::AddFunctionAttrStringValue(g, idx, UNIFORM_WG_SIZE_KIND,
                                             attr);
        }
        if let Some((start, end)) = attrs.amdgpu_flat_workgroup_size {
            let s = format!("{},{}\0", start, end);
            let attr = unsafe {
                CStr::from_bytes_with_nul_unchecked(s.as_ref())
            };
            llvm::AddFunctionAttrStringValue(g, idx, FLAT_WG_SIZE_KIND,
                                             attr);
        }
    }

    pub fn add_spirv_metadata(&self, g: &'ll Value, attrs: &CodegenFnAttrs) {
        const TYPE_SPEC_KIND: &'static str = "spirv.TypeSpec\0";
        const STORAGE_CLASS_KIND: &'static str = "spirv.StorageClass\0";
        const EXE_MODEL_KIND: &'static str = "spirv.ExecutionModel\0";
        const EXE_MODE_KIND: &'static str = "spirv.ExecutionMode\0";
        const BINDING_KIND: &'static str = "spirv.PipelineBinding\0";
        const SET_KIND: &'static str = "spirv.PipelineDescSet\0";

        let attrs = attrs.spirv.as_ref();
        if attrs.is_none() { return; }
        let attrs = attrs.unwrap();

        if let Some(class) = attrs.storage_class.as_ref() {
            let class_md = self.md_string(class);
            self.set_metadata(g, STORAGE_CLASS_KIND, class_md);
        }

        if let Some(ref metadata) = attrs.metadata {
            let spec_md = self.encode_spirv_attr_node(metadata);
            self.set_metadata(g, TYPE_SPEC_KIND, spec_md);
        }

        if let Some(ref exe_model) = attrs.exe_model {
            let md = self.md_string(exe_model);
            self.set_metadata(g, EXE_MODEL_KIND, md);
        }

        if let Some(ref exe_mode) = attrs.exe_mode {
            let md = self.encode_spirv_exe_mode_metadata(exe_mode);
            self.set_metadata(g, EXE_MODE_KIND, md);
        }

        if let Some(id) = attrs.pipeline_binding {
            let v = self.const_u32(id);
            self.set_metadata(g, BINDING_KIND, v);
        }
        if let Some(id) = attrs.pipeline_descriptor_set {
            let v = self.const_u32(id);
            self.set_metadata(g, SET_KIND, v);
        }
    }
    fn encode_decorations(&self, decorations: &[(String, Vec<u32>)]) -> &'ll Value {
        let mut md_dec = Vec::with_capacity(decorations.len());
        for &(ref decoration, ref literals) in decorations.iter() {
            let mut tuple = Vec::with_capacity(1 + literals.len());
            tuple.push(self.md_string(decoration));

            for &literal in literals.iter() {
                tuple.push(self.const_u32(literal));
            }

            md_dec.push(self.md_node(&tuple));
        }

        self.md_node(&md_dec)
    }
    fn encode_spirv_attr_node(&self, node: &SpirVAttrNode) -> &'ll Value {
        let type_spec = self.encode_spirv_type_spec(&node.type_spec);
        let decorations = self.encode_decorations(&node.decorations);
        self.md_node(&[type_spec, decorations])
    }
    fn encode_spirv_type_spec(&self, spec: &SpirVTypeSpec) -> &'ll Value {
        match spec {
            &SpirVTypeSpec::Image(ref img) => {
                self.encode_spirv_image_metadata(img)
            },
            &SpirVTypeSpec::SampledImage(ref img) => {
                const KIND: &'static str = "SampledImage";
                let kind = self.md_string(KIND);
                let img = self.encode_spirv_image_metadata(img);
                let values = [kind, img];
                self.md_node(&values)
            },
            &SpirVTypeSpec::Struct(ref members) => {
                let members = members.iter()
                    .map(|m| {
                        let node = self.encode_spirv_attr_node(&m.node);
                        let decorations = self.encode_decorations(&m.decorations);
                        self.md_node(&[node, decorations])
                    });

                let members: Vec<_> = Some(self.md_string("Struct")).into_iter()
                    .chain(members)
                    .collect();

                self.md_node(&members)
            },
            &SpirVTypeSpec::Array(ref element) => {
                self.md_node(&[
                    self.md_string("Array"),
                    self.encode_spirv_attr_node(&*element),
                ])
            },
            &SpirVTypeSpec::Matrix { columns, rows, ref decorations, } => {
                let type_spec = self.md_node(&[
                    self.const_u32(columns),
                    self.const_u32(rows),
                ]);

                let mut md_decorations = Vec::with_capacity(decorations.len());
                for &(ref decoration, ref literals) in decorations.iter() {
                    let decoration = self.md_string(decoration);
                    if literals.len() == 0 {
                        md_decorations.push(self.md_node(&[decoration]));
                    } else {
                        let mut tuple = Vec::with_capacity(1 + literals.len());
                        tuple.push(decoration);

                        for &literal in literals.iter() {
                            tuple.push(self.const_u32(literal));
                        }

                        md_decorations.push(self.md_node(&tuple));
                    }
                }
                let md_decorations = self.md_node(&md_decorations);
                self.md_node(&[
                    self.md_string("Matrix"),
                    self.md_node(&[type_spec, md_decorations]),
                ])
            },
        }
    }
    fn encode_spirv_image_metadata(&self, desc: &SpirVImageTypeSpec) -> &'ll Value {
        const KIND: &'static str = "Image";

        let arrayed = desc.arrayed as u32;
        let multisampled = desc.multisampled as u32;

        let kind = self.md_string(KIND);
        let dim = self.md_string(&desc.dim);
        let depth = self.const_u32(desc.depth);
        let arrayed = self.const_u32(arrayed);
        let multisampled = self.const_u32(multisampled);
        let sampled = self.const_u32(desc.sampled);
        let format = self.md_string(&desc.format);

        let values = [kind, dim, depth, arrayed, multisampled, sampled, format];

        self.md_node(&values)
    }
    fn encode_spirv_exe_mode_metadata(&self, mode: &[(String, Vec<u64>)]) -> &'ll Value {
        let modes: Vec<_> = mode.iter()
            .map(|&(ref kind, ref args)| {
                let mut values = vec![self.md_string(kind)];

                let args = args.iter()
                    .map(|&v| self.const_u64(v) );
                values.extend(args);

                self.md_node(&values)
            })
            .collect();

        self.md_node(&modes)
    }
}

impl StaticMethods for CodegenCx<'ll, 'tcx> {
    fn static_addr_of(&self, cv: &'ll Value, align: Align, kind: Option<&str>) -> &'ll Value {
        if let Some(&gv) = self.const_globals.borrow().get(&cv) {
            unsafe {
                // Upgrade the alignment in cases where the same constant is used with different
                // alignment requirements
                let llalign = align.bytes() as u32;
                if llalign > llvm::LLVMGetAlignment(gv) {
                    llvm::LLVMSetAlignment(gv, llalign);
                }
            }
            return gv;
        }
        let gv = self.static_addr_of_mut(cv, align, kind,
                                         self.const_addr_space());
        unsafe {
            llvm::LLVMSetGlobalConstant(gv, True);
        }
        self.const_globals.borrow_mut().insert(cv, gv);
        gv
    }

    fn codegen_static(&self, def_id: DefId, is_mutable: bool) {
        unsafe {
            let attrs = self.tcx.codegen_fn_attrs(def_id);

            let (v, alloc) = match codegen_static_initializer(&self, def_id) {
                Ok(v) => v,
                // Error has already been reported
                Err(_) => return,
            };

            let g = self.get_static(def_id);

            // boolean SSA values are i1, but they have to be stored in i8 slots,
            // otherwise some LLVM optimization passes don't work as expected
            let mut val_llty = self.val_ty(v);
            let v = if val_llty == self.type_i1() {
                val_llty = self.type_i8();
                llvm::LLVMConstZExt(v, val_llty)
            } else {
                v
            };

            let instance = Instance::mono(self.tcx, def_id);
            let ty = instance.ty(self.tcx, ty::ParamEnv::reveal_all());

            // As an optimization, all shared statics which do not have interior
            // mutability are placed into read-only memory.
            let llvm_mutable = is_mutable || !self.type_is_freeze(ty);

            let llty = self.layout_of(ty).llvm_type(self);
            let g = if val_llty == llty || attrs.spirv.is_some() {
                if attrs.spirv.is_some() {
                    // This global is provided by environment (eg Vulkan driver),
                    // but we can't use `extern "C"`, so it's hacked in here.
                    llvm::LLVMRustSetLinkage(g, llvm::Linkage::ExternalLinkage);
                }
                g
            } else {
                // If we created the global with the wrong type,
                // correct the type.
                let name = llvm::get_value_name(g).to_vec();
                llvm::set_value_name(g, b"");

                let linkage = llvm::LLVMRustGetLinkage(g);
                let visibility = llvm::LLVMRustGetVisibility(g);
                let attrs = self.tcx.codegen_fn_attrs(def_id);

                let addr_space = if llvm_mutable {
                    self.mutable_addr_space()
                } else {
                    self.const_addr_space()
                };
                let addr_space = attrs.addr_space
                  .unwrap_or(addr_space);

                let new_g = llvm::LLVMRustGetOrInsertGlobal(
                    self.llmod,
                    name.as_ptr().cast(),
                    name.len(),
                    val_llty,
                    addr_space.0,
                );

                llvm::LLVMRustSetLinkage(new_g, linkage);
                llvm::LLVMRustSetVisibility(new_g, visibility);

                // To avoid breaking any invariants, we leave around the old
                // global for the moment; we'll replace all references to it
                // with the new global later. (See base::codegen_backend.)
                self.statics_to_rauw.borrow_mut().push((g, new_g));
                new_g
            };
            set_global_alignment(&self, g, self.align_of(ty));
            if attrs.spirv.is_none() {
                llvm::LLVMSetInitializer(g, v);
            }

            // As an optimization, all shared statics which do not have interior
            // mutability are placed into read-only memory.
            if !llvm_mutable {
                llvm::LLVMSetGlobalConstant(g, llvm::True);
            }

            debuginfo::create_global_var_metadata(&self, def_id, g);

            if attrs.flags.contains(CodegenFnAttrFlags::THREAD_LOCAL) {
                llvm::set_thread_local_mode(g, self.tls_model);

                // Do not allow LLVM to change the alignment of a TLS on macOS.
                //
                // By default a global's alignment can be freely increased.
                // This allows LLVM to generate more performant instructions
                // e.g., using load-aligned into a SIMD register.
                //
                // However, on macOS 10.10 or below, the dynamic linker does not
                // respect any alignment given on the TLS (radar 24221680).
                // This will violate the alignment assumption, and causing segfault at runtime.
                //
                // This bug is very easy to trigger. In `println!` and `panic!`,
                // the `LOCAL_STDOUT`/`LOCAL_STDERR` handles are stored in a TLS,
                // which the values would be `mem::replace`d on initialization.
                // The implementation of `mem::replace` will use SIMD
                // whenever the size is 32 bytes or higher. LLVM notices SIMD is used
                // and tries to align `LOCAL_STDOUT`/`LOCAL_STDERR` to a 32-byte boundary,
                // which macOS's dyld disregarded and causing crashes
                // (see issues #51794, #51758, #50867, #48866 and #44056).
                //
                // To workaround the bug, we trick LLVM into not increasing
                // the global's alignment by explicitly assigning a section to it
                // (equivalent to automatically generating a `#[link_section]` attribute).
                // See the comment in the `GlobalValue::canIncreaseAlignment()` function
                // of `lib/IR/Globals.cpp` for why this works.
                //
                // When the alignment is not increased, the optimized `mem::replace`
                // will use load-unaligned instructions instead, and thus avoiding the crash.
                //
                // We could remove this hack whenever we decide to drop macOS 10.10 support.
                if self.tcx.sess.target.target.options.is_like_osx {
                    // The `inspect` method is okay here because we checked relocations, and
                    // because we are doing this access to inspect the final interpreter state
                    // (not as part of the interpreter execution).
                    //
                    // FIXME: This check requires that the (arbitrary) value of undefined bytes
                    // happens to be zero. Instead, we should only check the value of defined bytes
                    // and set all undefined bytes to zero if this allocation is headed for the
                    // BSS.
                    let all_bytes_are_zero = alloc.relocations().is_empty()
                        && alloc
                            .inspect_with_uninit_and_ptr_outside_interpreter(0..alloc.len())
                            .iter()
                            .all(|&byte| byte == 0);

                    let sect_name = if all_bytes_are_zero {
                        const_cstr!("__DATA,__thread_bss")
                    } else {
                        const_cstr!("__DATA,__thread_data")
                    };
                    llvm::LLVMSetSection(g, sect_name.as_ptr());
                }
            }

            // Wasm statics with custom link sections get special treatment as they
            // go into custom sections of the wasm executable.
            if self.tcx.sess.opts.target_triple.triple().starts_with("wasm32") {
                if let Some(section) = attrs.link_section {
                    let section = llvm::LLVMMDStringInContext(
                        self.llcx,
                        section.as_str().as_ptr().cast(),
                        section.as_str().len() as c_uint,
                    );
                    assert!(alloc.relocations().is_empty());

                    // The `inspect` method is okay here because we checked relocations, and
                    // because we are doing this access to inspect the final interpreter state (not
                    // as part of the interpreter execution).
                    let bytes =
                        alloc.inspect_with_uninit_and_ptr_outside_interpreter(0..alloc.len());
                    let alloc = llvm::LLVMMDStringInContext(
                        self.llcx,
                        bytes.as_ptr().cast(),
                        bytes.len() as c_uint,
                    );
                    let data = [section, alloc];
                    let meta = llvm::LLVMMDNodeInContext(self.llcx, data.as_ptr(), 2);
                    llvm::LLVMAddNamedMetadataOperand(
                        self.llmod,
                        "wasm.custom_sections\0".as_ptr().cast(),
                        meta,
                    );
                }
            } else {
                base::set_link_section(g, &attrs);
            }

            if attrs.flags.contains(CodegenFnAttrFlags::USED) {
                self.add_used_global(g);
            }

            self.add_spirv_metadata(g, &attrs);
        }
    }

    /// Add a global value to a list to be stored in the `llvm.used` variable, an array of i8*.
    fn add_used_global(&self, global: &'ll Value) {
        // Note this ignores the address space of `g`, but that's okay here.
        let cast = unsafe { llvm::LLVMConstPointerCast(global, self.type_i8p()) };
        self.used_statics.borrow_mut().push(cast);
    }
}
