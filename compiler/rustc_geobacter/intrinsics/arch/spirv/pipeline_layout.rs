
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::convert::TryInto;
use std::geobacter::spirv::pipeline_layout::*;
use std::str::FromStr;

use smallvec::SmallVec;

use rustc_data_structures::fx::FxHashSet;
use rustc_errors::DiagnosticBuilder;
use rustc_hir::LangItem;
use rustc_middle::mir::mono::MonoItem;
use rustc_middle::ty::*;

use super::*;
use crate::collector::collect_items_rec;

/// `geobacter_spirv_pipeline_layout_desc{}`, where `{}` is the number of
/// entry point type params.
#[derive(Clone, Copy)]
pub struct PipelineLayoutDesc(u32);

const PREFIX: &'static str = "geobacter_spirv_pipeline_layout_desc";

impl PipelineLayoutDesc {
    pub fn insert_into_map<F>(mut map: F)
        where Self: Sized,
              F: for<'a> FnMut(&'a str, Lrc<dyn CustomIntrinsicMirGen>),
    {
        for suffix in 0u32..8 {
            let name = format!("{}{}", PREFIX, suffix);
            map(&name, Lrc::new(PipelineLayoutDesc(suffix)))
        }
    }
    pub fn check(name: &str) -> Result<(), Lrc<dyn CustomIntrinsicMirGen>> {
        if !name.starts_with(PREFIX) {
            return Ok(());
        }

        let suffix = &name[PREFIX.len()..];
        match u32::from_str(suffix) {
            Ok(c) => Err(Lrc::new(PipelineLayoutDesc(c))),
            Err(_) => Ok(()),
        }
    }
}

#[derive(Debug)]
struct LangItems {
    buffer: DefId,
    uniform: DefId,
}
impl LangItems {
    fn new<'tcx>(tcx: TyCtxt<'tcx>) -> Self {
        LangItems {
            buffer: tcx.require_lang_item(LangItem::SpirvBufferObject, None),
            uniform: tcx.require_lang_item(LangItem::SpirvUniformObject, None),
        }
    }

    fn has_set_binding(&self, did: DefId) -> bool {
        did == self.buffer || did == self.uniform
    }

    fn extract_descriptor<'tcx>(&self, tcx: TyCtxt<'tcx>, from: ty::Instance<'tcx>)
        -> Option<(DescriptorDesc<'tcx>, (u32, u32))>
    {
        let ty = from.ty(tcx, ParamEnv::reveal_all());
        let (array_count, def, substs) = match *ty.kind() {
            Array(inner, count) => {
                match *inner.kind() {
                    Adt(def, substs) if self.has_set_binding(def.did) => {
                        let c = count.eval_bits(tcx, ParamEnv::reveal_all(),
                                                tcx.types.u32)
                            .try_into()
                            .unwrap_or_else(|_| {
                                let sp = tcx.def_span(from.def_id());
                                let msg = format!("array size > u32::max_value()");
                                tcx.sess.span_err(sp, &msg);
                                1
                            });
                        (c as u32, def, substs)
                    },
                    _ => { return None; },
                }
            },
            Adt(def, substs) if self.has_set_binding(def.did) => {
                (1, def, substs)
            },
            _ => { return None; },
        };

        let mut consts = substs.consts()
            .map(|c| {
                c.eval_bits(tcx, ParamEnv::reveal_all(), tcx.types.u32) as u32
            });

        let set = consts.next().unwrap_or_else(|| {
            bug!("expected constant param; got none: {:?}", from);
        });
        let binding = consts.next().unwrap_or_else(|| {
            bug!("expected constant param; got none: {:?}", from);
        });

        let desc = if def.did == self.buffer {
            DescriptorBufferDesc {
                dynamic: Some(false),
                storage: true,
            }
        } else if def.did == self.uniform {
            DescriptorBufferDesc {
                dynamic: Some(false),
                storage: false,
            }
        } else {
            unreachable!();
        };

        let ty = DescriptorDescTy::Buffer(desc);

        let desc = DescriptorDesc {
            insts: SmallVec::new(),
            ty,
            array_count,
            stages: ShaderStages {
                // TO DO
                vertex: true,
                fragment: true,
                compute: true,

                tessellation_control: false,
                tessellation_evaluation: false,
                geometry: false,
            },
            // TO DO
            readonly: false,
        };

        Some((desc, (set, binding)))
    }
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DescriptorImageDesc {
    sampled: bool,
    dimensions: CompilerDescriptorImageDims,
    format: Option<CompilerImgFormat>,
    multisampled: bool,
    array_layers: DescriptorImageDescArray,
}
#[allow(dead_code)] // TO DO
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DescriptorImageDescArray {
    NonArrayed,
    Arrayed {
        max_layers: Option<u32>,
    },
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DescriptorBufferDesc {
    dynamic: Option<bool>,
    storage: bool,
}
#[allow(dead_code)] // TO DO
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DescriptorDescTy {
    Sampler,
    CombinedImageSampler(DescriptorImageDesc),
    Image(DescriptorImageDesc),
    TexelBuffer {
        storage: bool,
        format: Option<CompilerImgFormat>,
    },
    InputAttachment {
        multisampled: bool,
        array_layers: DescriptorImageDescArray,
    },
    Buffer(DescriptorBufferDesc),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ShaderStages {
    vertex: bool,
    tessellation_control: bool,
    tessellation_evaluation: bool,
    geometry: bool,
    fragment: bool,
    compute: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DescriptorDesc<'tcx> {
    /// If `len()` > 1, then this spot has multiple assignments; these instances
    /// will be used for reporting an error to the user.
    insts: SmallVec<[ty::Instance<'tcx>; 1]>,
    ty: DescriptorDescTy,
    array_count: u32,
    stages: ShaderStages,
    readonly: bool,
}

impl CustomIntrinsicMirGen for PipelineLayoutDesc {
    fn mirgen_simple_intrinsic<'tcx>(&self, tcx: TyCtxt<'tcx>,
                                     instance: ty::Instance<'tcx>,
                                     mir: &mut mir::Body<'tcx>)
    {
        // Create an empty function:
        let source_info = mir::SourceInfo {
            span: DUMMY_SP,
            scope: mir::OUTERMOST_SOURCE_SCOPE,
        };

        let mut bb = mir::BasicBlockData {
            statements: vec![],
            terminator: Some(mir::Terminator {
                source_info,
                kind: mir::TerminatorKind::Return,
            }),

            is_cleanup: false,
        };


        let mut visited: FxHashSet<_> = Default::default();
        let entries = instance.substs.types()
            .filter_map(|root| tcx.extract_opt_fn_instance(instance, root) );
        for entry in entries {
            // collect all referenced mono items upfront:
            let mono_root = MonoItem::Fn(entry);
            collect_items_rec(tcx, mono_root, &mut visited);
        }

        let lang_items = LangItems::new(tcx);

        let mut sets: BTreeMap<u32, BTreeMap<u32, DescriptorDesc<'_>>> = Default::default();

        for mono in visited.into_iter() {
            let instance = match mono {
                MonoItem::Fn(_) => { continue; },
                MonoItem::Static(mono_did) => Instance::mono(tcx, mono_did),
                MonoItem::GlobalAsm(..) => {
                    bug!("unexpected mono item `{:?}`", mono);
                },
            };

            let desc = lang_items.extract_descriptor(tcx, instance);
            if desc.is_none() { continue; }
            let (desc, slot) = desc.unwrap();

            let set = match sets.entry(slot.0) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => {
                    v.insert(BTreeMap::default())
                },
            };

            let desc = match set.entry(slot.1) {
                Entry::Vacant(v) => {
                    // the happy path
                    v.insert(desc)
                },
                Entry::Occupied(o) => {
                    // There are two or more statics which are assigned to the same set+binding.
                    o.into_mut()
                },
            };
            desc.insts.push(instance);
        }

        info!("desc set bindings: {:#?}", sets);
        let sets = sets;

        let desc_set_ty = tcx.mk_array(tcx.mk_tup([
            tcx.types.u32,
            desc_bindings_desc_ty(tcx),
        ].iter()), sets.len() as _);

        let desc_bindings_ty = desc_desc_ty(tcx);
        let mut c_sets: SmallVec<[_; 32]> = SmallVec::new();
        c_sets.reserve(sets.len() * 2);
        for (&set_id, set) in sets.iter() {
            let set_bindings_ty = tcx.mk_array(desc_bindings_ty, set.len() as _);

            let mut c_set = <SmallVec<[_; 32]>>::with_capacity(set.len() * 8);
            for (&binding_id, desc) in set.iter() {
                // check that all the spots have only one instance, otherwise report an error.
                if desc.insts.len() > 1 {
                    let hir = tcx.hir();

                    let msg = format!("duplicate descriptor: set = {}, binding = {}",
                                      set_id, binding_id);

                    let mut diag: Option<DiagnosticBuilder<'_>> = None;
                    for inst in desc.insts.iter() {
                        let local_did = inst.def_id().as_local()
                            .expect("TODO: non-local DefIds");
                        let hir_id = hir.local_def_id_to_hir_id(local_did);
                        let span = hir.span(hir_id);
                        match diag {
                            Some(ref mut diag) => {
                                diag.span_label(span, &msg);
                            },
                            None => {
                                diag = Some(tcx.sess.struct_span_err(span, &msg));
                            },
                        }
                    }
                }

                c_set.extend(build_compiler_descriptor_desc(tcx, binding_id, desc));
            }

            let slice = tcx.mk_static_slice_cv("desc set bindings",
                                               c_set.into_iter(),
                                               set_bindings_ty,
                                               set.len());
            c_sets.push(tcx.mk_u32_cv(set_id));
            c_sets.push(slice);
        }

        let slice = tcx.mk_static_slice_cv("desc sets",
                                           c_sets.into_iter(),
                                           desc_set_ty,
                                           sets.len());
        let ret_ty = self.output(tcx);
        let slice = tcx.const_value_rvalue(&source_info, slice, ret_ty);

        let ret = mir::Place::return_place();
        let stmt_kind = StatementKind::Assign(Box::new((ret, slice)));
        let stmt = Statement {
            source_info,
            kind: stmt_kind,
        };
        bb.statements.push(stmt);
        mir.basic_blocks_mut().push(bb);
    }

    fn generic_parameter_count<'tcx>(&self, _tcx: TyCtxt<'tcx>) -> usize {
        self.0 as _
    }
    /// The types of the input args.
    fn inputs<'tcx>(&self, tcx: TyCtxt<'tcx>)
                    -> &'tcx ty::List<Ty<'tcx>>
    {
        tcx.intern_type_list(&[])
    }
    /// The return type.
    fn output<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
        desc_set_bindings_desc_ty(tcx)
    }
}

fn build_compiler_descriptor_desc<'tcx>(tcx: TyCtxt<'tcx>,
                                        id: u32, ty: &DescriptorDesc<'tcx>)
                                        -> impl Iterator<Item = ConstValue<'tcx>>
{
    let first = build_compiler_descriptor_desc_ty(tcx, ty.ty);
    let second = Some(tcx.mk_u32_cv(ty.array_count)).into_iter();
    let third = build_compiler_shader_stages(tcx, ty.stages);
    let forth = Some(tcx.mk_bool_cv(ty.readonly)).into_iter();

    let mut values: SmallVec<[_; 8]> = Default::default();
    values.push(tcx.mk_u32_cv(id));
    values.extend(first.iter().cloned());
    values.extend(second);
    values.extend(third.into_iter());
    values.extend(forth);

    values.into_iter()
}
fn build_compiler_shader_stages<'tcx>(tcx: TyCtxt<'tcx>,
                                      ty: ShaderStages)
                                      -> SmallVec<[ConstValue<'tcx>; 6]>
{
    let tuple = [
        ty.vertex,
        ty.tessellation_control,
        ty.tessellation_evaluation,
        ty.geometry,
        ty.fragment,
        ty.compute,
    ];
    tuple.iter().map(|&b| tcx.mk_bool_cv(b) ).collect()
}
fn build_compiler_descriptor_desc_ty<'tcx>(tcx: TyCtxt<'tcx>,
                                           ty: DescriptorDescTy)
                                           -> [ConstValue<'tcx>; 6]
{
    let kind = match ty {
        DescriptorDescTy::Sampler => CompilerDescriptorDescTyKind::Sampler,
        DescriptorDescTy::CombinedImageSampler(..) => CompilerDescriptorDescTyKind::CombinedImageSampler,
        DescriptorDescTy::Image(..) => CompilerDescriptorDescTyKind::Image,
        DescriptorDescTy::TexelBuffer { .. } => CompilerDescriptorDescTyKind::TexelBuffer,
        DescriptorDescTy::InputAttachment { .. } => CompilerDescriptorDescTyKind::InputAttachment,
        DescriptorDescTy::Buffer(_) => CompilerDescriptorDescTyKind::Buffer,
    };
    let combined_image_sampler = match ty {
        DescriptorDescTy::CombinedImageSampler(desc) => Some(desc),
        _ => None,
    };
    let image = match ty {
        DescriptorDescTy::Image(desc) => Some(desc),
        _ => None,
    };
    let texel_buffer = match ty {
        DescriptorDescTy::TexelBuffer {
            storage,
            format,
        } => Some((storage, format)),
        _ => None,
    };
    let input_attachment = match ty {
        DescriptorDescTy::InputAttachment {
            multisampled,
            array_layers,
        } => Some((multisampled, array_layers)),
        _ => None,
    };
    let buffer = match ty {
        DescriptorDescTy::Buffer(desc) => Some(desc),
        _ => None,
    };

    let kind = tcx.mk_u32_cv(kind.into());
    let combined_image_sampler = tcx.mk_optional(combined_image_sampler,
                                                 build_compiler_descriptor_img_desc);
    let image = tcx.mk_optional(image,
                                build_compiler_descriptor_img_desc);
    let texel_buffer = tcx.mk_optional(texel_buffer,
                                       build_compiler_descriptor_texel_buffer);
    let input_attachment = tcx.mk_optional(input_attachment,
                                           build_compiler_descriptor_input_attachment);
    let buffer = tcx.mk_optional(buffer,
                                 build_compiler_descriptor_buffer);

    [kind, combined_image_sampler, image,
        texel_buffer, input_attachment, buffer, ]
}
fn build_compiler_descriptor_img_desc<'tcx>(tcx: TyCtxt<'tcx>,
                                            desc: DescriptorImageDesc)
                                            -> ConstValue<'tcx>
{
    let ty = tcx.mk_static_slice(desc_img_desc_ty(tcx));

    let sampled = tcx.mk_bool_cv(desc.sampled);
    let dims = tcx.mk_u32_cv(desc.dimensions as _);
    let format = tcx.mk_optional(desc.format, |_, v| tcx.mk_u32_cv(v as _) );
    let multisampled = tcx.mk_bool_cv(desc.multisampled);
    let array_layout = build_compiler_descriptor_img_array(tcx, desc.array_layers);

    let mut tuple: SmallVec<[_; 8]> = SmallVec::new();
    tuple.push(sampled);
    tuple.push(dims);
    tuple.push(format);
    tuple.push(multisampled);
    tuple.extend(array_layout.iter().cloned());

    tcx.mk_static_tuple_cv("compiler_descriptor_img_desc", tuple.into_iter(), ty)
}
fn build_compiler_descriptor_img_array<'tcx>(tcx: TyCtxt<'tcx>,
                                             desc: DescriptorImageDescArray)
                                             -> [ConstValue<'tcx>; 2]
{
    let (first, second) = match desc {
        DescriptorImageDescArray::NonArrayed => (true, None),
        DescriptorImageDescArray::Arrayed {
            max_layers,
        } => (false, max_layers),
    };

    [
        tcx.mk_bool_cv(first),
        tcx.mk_optional(second, |_, v| tcx.mk_u32_cv(v) ),
    ]
}
fn build_compiler_descriptor_texel_buffer<'tcx>(tcx: TyCtxt<'tcx>,
                                                desc: (bool, Option<CompilerImgFormat>))
                                                -> ConstValue<'tcx>
{
    let storage = tcx.mk_bool_cv(desc.0);
    let format = tcx.mk_optional(desc.1, |_, v| tcx.mk_u32_cv(v as _) );

    let tup = [
        tcx.types.bool,
        tcx.mk_static_slice(vk_format_ty(tcx)),
    ];
    let ty = tcx.mk_tup(tup.iter());

    tcx.mk_static_tuple_cv("compiler_descriptor_texel_buffer",
                           [storage, format].iter().cloned(),
                           ty)
}
fn build_compiler_descriptor_input_attachment<'tcx>(tcx: TyCtxt<'tcx>,
                                                    desc: (bool, DescriptorImageDescArray))
                                                    -> ConstValue<'tcx>
{
    let multisampled = tcx.mk_bool_cv(desc.0);
    let array_layers = build_compiler_descriptor_img_array(tcx, desc.1);

    let tup = [
        tcx.types.bool,
        desc_img_array_ty(tcx),
    ];
    let ty = tcx.mk_tup(tup.iter());

    let mut values: SmallVec<[_; 3]> = Default::default();
    values.push(multisampled);
    values.extend(array_layers.iter().cloned());

    tcx.mk_static_tuple_cv("compiler_descriptor_input_attachment",
                           values.into_iter(), ty)
}
fn build_compiler_descriptor_buffer<'tcx>(tcx: TyCtxt<'tcx>,
                                          desc: DescriptorBufferDesc)
                                          -> ConstValue<'tcx>
{
    let dynamic = tcx.mk_optional(desc.dynamic, |_, v| tcx.mk_bool_cv(v) );
    let storage = tcx.mk_bool_cv(desc.storage);

    let ty = desc_buffer_desc_ty(tcx);

    tcx.mk_static_tuple_cv("compiler_descriptor_buffer",
                           [dynamic, storage].iter().cloned(),
                           ty)
}

fn desc_img_dims_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    tcx.types.u32
}
fn desc_img_array_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let tup = [tcx.types.bool, tcx.mk_static_slice(tcx.types.u32), ];
    tcx.mk_tup(tup.iter())
}
fn vk_format_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    tcx.types.u32
}
fn desc_img_desc_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let tup = [
        tcx.types.bool,
        desc_img_dims_ty(tcx),
        tcx.mk_static_slice(vk_format_ty(tcx)),
        tcx.types.bool,
        desc_img_array_ty(tcx),
    ];
    tcx.mk_tup(tup.iter())
}
fn desc_buffer_desc_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let tup = [
        tcx.mk_static_slice(tcx.types.bool),
        tcx.types.bool,
    ];
    tcx.mk_tup(tup.iter())
}
fn desc_desc_ty_kind_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    tcx.types.u32
}
fn desc_desc_ty_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let tup = [
        desc_desc_ty_kind_ty(tcx),
        tcx.mk_static_slice(desc_img_desc_ty(tcx)),
        tcx.mk_static_slice(desc_img_desc_ty(tcx)),
        tcx.mk_static_slice({
            let tup = [
                tcx.types.bool,
                tcx.mk_static_slice(vk_format_ty(tcx)),
            ];
            tcx.mk_tup(tup.iter())
        }),
        tcx.mk_static_slice({
            let tup = [
                tcx.types.bool,
                desc_img_array_ty(tcx),
            ];
            tcx.mk_tup(tup.iter())
        }),
        tcx.mk_static_slice(desc_buffer_desc_ty(tcx))
    ];
    tcx.mk_tup(tup.iter())
}
fn shader_stages_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let tup = [tcx.types.bool; 6];
    tcx.mk_tup(tup.iter())
}
fn desc_desc_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let tup = [
        tcx.types.u32,
        desc_desc_ty_ty(tcx),
        tcx.types.u32,
        shader_stages_ty(tcx),
        tcx.types.bool,
    ];
    tcx.mk_tup(tup.iter())
}
fn desc_bindings_desc_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    tcx.mk_static_slice(desc_desc_ty(tcx))
}
fn desc_set_bindings_desc_ty<'tcx>(tcx: TyCtxt<'tcx>) -> Ty<'tcx> {
    let tup = [
        tcx.types.u32,
        desc_bindings_desc_ty(tcx),
    ];
    let tup = tcx.mk_tup(tup.iter());
    tcx.mk_static_slice(tup)
}
