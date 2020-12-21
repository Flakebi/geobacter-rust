use std::borrow::Cow;
use std::iter::repeat;

use rustc_middle::bug;
use rustc_middle::mir::{Constant, Operand};
use rustc_middle::mir;
use rustc_middle::mir::interpret::{Allocation, AllocId, ConstValue, Pointer, Scalar, ScalarMaybeUninit, GlobalAlloc};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_middle::ty::{Array, Const, ConstKind, ParamEnv, Tuple};
use rustc_middle::ty::layout::HasTyCtxt;
use rustc_target::abi::{Align, FieldsShape, HasDataLayout, Size};

use tracing::info;

pub trait TyCtxtConstBuilder<'tcx>: HasTyCtxt<'tcx> {
    fn mk_const_op(&self,
                   src: &mir::SourceInfo,
                   c: ty::Const<'tcx>) -> Operand<'tcx> {
        let v = Constant {
            span: src.span,
            literal: self.tcx().mk_const(c),
            user_ty: None,
        };
        let v = Box::new(v);
        Operand::Constant(v)
    }

    fn mk_bool_cv(&self, v: bool) -> ConstValue<'tcx> {
        let v = Scalar::from_bool(v);
        ConstValue::Scalar(v)
    }
    fn mk_u32_cv(&self, v: u32) -> ConstValue<'tcx> {
        let v = Scalar::from_uint(v, Size::from_bytes(4));
        ConstValue::Scalar(v)
    }
    fn mk_u64_cv(&self, v: u64) -> ConstValue<'tcx> {
        let v = Scalar::from_uint(v, Size::from_bytes(8));
        ConstValue::Scalar(v)
    }
    fn mk_usize_cv(&self, v: impl Into<u128>) -> ConstValue<'tcx> {
        let size = self.tcx().data_layout().pointer_size;
        let v = Scalar::from_uint(v, size);
        ConstValue::Scalar(v)
    }
    fn mk_usize_c(&self, v: impl Into<u128>) -> &'tcx ty::Const<'tcx> {
        self.tcx().mk_const(ty::Const {
            ty: self.tcx().types.usize,
            val: ConstKind::Value(self.mk_usize_cv(v)),
        })
    }

    fn unwrap_global_memory(&self, id: AllocId) -> &'tcx Allocation {
        match self.tcx().global_alloc(id) {
            GlobalAlloc::Memory(alloc) => alloc,
            v => bug!("{} is not GlobalAlloc: {:?}", id, v),
        }
    }

    fn mk_static_str_operand(&self,
                             src: &mir::SourceInfo,
                             v: &str)
                             -> Operand<'tcx>
    {
        let tcx = self.tcx();
        let v = self.mk_static_str_cv(v);
        let v = tcx.mk_const(Const {
            ty: tcx.mk_static_str(),
            val: ConstKind::Value(v),
        });
        let v = Constant {
            span: src.span,
            literal: v,
            user_ty: None,
        };
        Operand::Constant(Box::new(v))
    }

    fn mk_u64_operand(&self,
                      src: &mir::SourceInfo,
                      v: u64)
                      -> Operand<'tcx>
    {
        let tcx = self.tcx();
        let v = self.mk_u64_cv(v);
        let v = tcx.mk_const(Const {
            ty: tcx.types.u64,
            val: ConstKind::Value(v),
        });
        let v = Constant {
            span: src.span,
            literal: v,
            user_ty: None,
        };
        let v = Box::new(v);
        Operand::Constant(v)
    }

    fn mk_optional<F, T>(&self,
                         val: Option<T>,
                         some_val: F) -> ConstValue<'tcx>
        where F: FnOnce(TyCtxt<'tcx>, T) -> ConstValue<'tcx>,
    {
        let tcx = self.tcx();
        if let Some(val) = val {
            let val = some_val(tcx, val);
            let alloc = match val {
                ConstValue::Scalar(Scalar::Ptr(ptr)) => {
                    self.unwrap_global_memory(ptr.alloc_id)
                }
                ConstValue::Scalar(Scalar::Raw { size, .. }) => {
                    // create an allocation for this

                    let scalar = match val {
                        ConstValue::Scalar(s) => s,
                        _ => unreachable!(),
                    };

                    let size = Size::from_bytes(size);
                    let align = Align::from_bytes(16).unwrap();
                    let mut alloc = Allocation::uninit(size, align);
                    let alloc_id = tcx.reserve_alloc_id();

                    let ptr = Pointer::from(alloc_id);
                    alloc.write_scalar(&tcx, ptr,
                                       ScalarMaybeUninit::Scalar(scalar),
                                       size)
                        .expect("allocation write failed");

                    let alloc = tcx.intern_const_alloc(alloc);
                    tcx.set_alloc_id_memory(alloc_id, alloc);

                    alloc
                }
                val => unimplemented!("scalar type {:?}", val),
            };
            ConstValue::Slice {
                data: alloc,
                start: 0,
                end: 1,
            }
        } else {
            // Create an empty slice to represent a None value:
            const C: &'static [u8] = &[];
            let alloc = Allocation::from_byte_aligned_bytes(Cow::Borrowed(C));
            let alloc = tcx.intern_const_alloc(alloc);
            ConstValue::Slice {
                data: alloc,
                start: 0,
                end: 0,
            }
        }
    }

    fn mk_static_str_cv(&self, s: &str) -> ConstValue<'tcx> {
        let tcx = self.tcx();
        let align = Align::from_bytes(16).unwrap();
        let alloc = Allocation::from_bytes(Cow::Borrowed(s.as_bytes()), align);
        let alloc = tcx.intern_const_alloc(alloc);
        ConstValue::Slice {
            data: alloc,
            start: 0,
            end: s.len(),
        }
    }

    fn mk_static_slice_cv<I>(&self, what: &str,
                             elems: I, ty: Ty<'tcx>,
                             len: usize) -> ConstValue<'tcx>
        where I: ExactSizeIterator<Item = ConstValue<'tcx>>,
    {
        let (_, alloc, _) = self.static_tuple_alloc(what, elems, ty);
        ConstValue::Slice {
            data: alloc,
            start: 0,
            end: len,
        }
    }

    fn mk_static_tuple_cv<I>(&self, what: &str,
                             tuple: I, ty: Ty<'tcx>) -> ConstValue<'tcx>
        where I: ExactSizeIterator<Item=ConstValue<'tcx>>,
    {
        let (alloc_id, ..) = self.static_tuple_alloc(what, tuple, ty);
        let ptr = Pointer::from(alloc_id);
        let scalar = Scalar::Ptr(ptr);
        ConstValue::Scalar(scalar)
    }

    fn static_tuple_alloc<I>(&self, what: &str,
                             tuple: I, ty: Ty<'tcx>)
                             -> (AllocId, &'tcx Allocation, Size)
        where I: ExactSizeIterator<Item=ConstValue<'tcx>>,
    {
        let tcx = self.tcx();

        let env = ParamEnv::reveal_all()
            .and(ty);
        let layout = tcx.layout_of(env)
            .expect("layout failure");
        let size = layout.size;
        let align = layout.align.pref;

        let data = vec![0; size.bytes() as usize];
        let mut alloc = Allocation::from_bytes(&data, align);
        let alloc_id = tcx.reserve_alloc_id();

        let mut tuple = tuple.enumerate();

        self.write_static_tuple(what, &mut tuple, alloc_id, &mut alloc,
                                Size::ZERO, ty);

        assert_eq!(tuple.next(), None);

        let alloc = tcx.intern_const_alloc(alloc);
        tcx.set_alloc_id_memory(alloc_id, alloc);
        (alloc_id, alloc, size)
    }

    fn write_static_tuple<I>(&self, what: &str, tuple: &mut I,
                             alloc_id: AllocId, alloc: &mut Allocation,
                             base: Size, ty: Ty<'tcx>)
        where I: ExactSizeIterator<Item=(usize, ConstValue<'tcx>)>,
    {
        let tcx = self.tcx();

        let env = ParamEnv::reveal_all()
            .and(ty);
        let layout = tcx.layout_of(env)
            .expect("layout failure");

        let fields = match layout.fields {
            FieldsShape::Arbitrary {
                ref offsets,
                ..
            } => {
                offsets.clone()
            }
            FieldsShape::Array {
                stride, count,
            } => {
                let offsets: Vec<_> = (0..count)
                    .map(|idx| stride * idx)
                    .collect();
                offsets
            }
            _ => unimplemented!("layout offsets {:?}", layout),
        };

        let ty_fields: Box<dyn Iterator<Item=Ty<'tcx>>> = match ty.kind() {
            Tuple(tuple_fields) => {
                assert_eq!(tuple_fields.len(), fields.len());
                Box::new(tuple_fields.types()) as Box<_>
            }
            &Array(element, _count) => {
                Box::new(repeat(element)) as Box<_>
            }
            _ => unimplemented!("non tuple type: {:?}", ty),
        };

        for (mut offset, field_ty) in fields.into_iter().zip(ty_fields) {
            info!("{}: offset = {}, field_ty = {:?}", what, (base + offset).bytes(), field_ty);
            match field_ty.kind() {
                Tuple(_) => {
                    self.write_static_tuple(what, tuple, alloc_id, alloc,
                                            base + offset, field_ty);
                    continue;
                }
                Array(..) => {
                    self.write_static_tuple(what, tuple, alloc_id, alloc,
                                            base + offset, field_ty);
                    continue;
                }
                _ => {}
            }

            let (index, element) = tuple.next()
                .expect("missing tuple field value");

            info!("{}: index = {}, element = {:?}", what, index, element);

            let mut write_scalar = |scalar| {
                let ptr = Pointer::new(alloc_id, base + offset);
                let size = match scalar {
                    Scalar::Raw { size, .. } => {
                        Size::from_bytes(size)
                    }
                    Scalar::Ptr(_) => {
                        tcx.data_layout().pointer_size
                    }
                };
                offset += size;

                let scalar = ScalarMaybeUninit::Scalar(scalar);
                alloc.write_scalar(&tcx, ptr, scalar, size)
                    .expect("allocation write failed");
            };

            match element {
                ConstValue::Scalar(scalar) => {
                    write_scalar(scalar);
                }
                ConstValue::Slice { data, start, end, } => {
                    // this process follows the same procedure as in rustc_codegen_ssa
                    let id = tcx.create_memory_alloc(data);
                    let offset = Size::from_bytes(start as u64);
                    let ptr = Pointer::new(id, offset);
                    write_scalar(ptr.into());
                    let size = Scalar::from_uint((end - start) as u128,
                                                 tcx.data_layout().pointer_size);
                    write_scalar(size);
                }
                _ => {
                    bug!("unhandled ConstValue: {:?}", element);
                }
            }
        }
    }
}

impl<'tcx, T> TyCtxtConstBuilder<'tcx> for T
    where T: HasTyCtxt<'tcx>,
{}
