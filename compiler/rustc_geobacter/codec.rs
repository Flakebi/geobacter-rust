//! A Encoder and Decoder useful for on-demand serialization and
//! deserialization of Rust compiler structures.
//!
//! This code is based on the incremental compilation cache.
//!
//! TODO Figure out how we can share as much of the data we encode here
//! with the crate metadata.
//!

use std::mem;

use rustc_data_structures::fingerprint::{Fingerprint, FingerprintDecoder, FingerprintEncoder};
use rustc_data_structures::fx::{FxHashMap, FxHashSet, FxIndexSet};
use rustc_data_structures::sync::HashMapExt;
use rustc_hir::def_id::{CrateNum, DefId, DefIndex, LOCAL_CRATE,
                        LocalDefId};
use rustc_hir::definitions::DefPathHash;
use rustc_index::vec::*;
use rustc_middle::mir;
use rustc_middle::mir::interpret::{self, AllocDecodingSession, AllocDecodingState,
                                   AllocId, specialized_encode_alloc_id};
use rustc_middle::ty::{self, Ty, TyCtxt};
use rustc_middle::ty::codec as ty_codec;
use rustc_middle::ty::codec::{RefDecodable, TyDecoder, TyEncoder, OpaqueEncoder};
use rustc_serialize::{Decodable, Decoder, Encodable, Encoder, opaque};
use rustc_session::CrateDisambiguator;
use rustc_span::Span;

const TAG_FILE_FOOTER: u128 = 0xC0FFEE_C0FFEE_C0FFEE_C0FFEE_C0FFEE;

#[derive(Encodable, Decodable)]
struct Footer {
    /// This is dense, ie a crate num won't necessarily be at it's
    /// corresponding index.
    prev_cnums: Vec<(u32, String, CrateDisambiguator)>,
    interpret_alloc_index: Vec<u32>,
}

pub struct GeobacterDecoder<'a, 'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub opaque: opaque::Decoder<'a>,

    cnum_map: IndexVec<CrateNum, Option<CrateNum>>,

    alloc_decoding_session: AllocDecodingSession<'a>,
}

impl<'a, 'tcx> GeobacterDecoder<'a, 'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>, data: &'a [u8],
               alloc_decoding_state: &'a mut Option<AllocDecodingState>)
               -> Self
    {
        let mut decoder = opaque::Decoder::new(&data[..], 0);
        let footer: Footer = {
            // Decode the *position* of the footer which can be found in the
            // last 8 bytes of the file.
            decoder.set_position(data.len() - IntEncodedWithFixedSize::ENCODED_SIZE);
            let query_result_index_pos = IntEncodedWithFixedSize::decode(&mut decoder)
                .expect("Error while trying to decode query result index position.")
                .0 as usize;

            // Decoder the file footer which contains all the lookup tables, etc.
            decoder.set_position(query_result_index_pos);
            decode_tagged(&mut decoder, TAG_FILE_FOOTER)
                .expect("Error while trying to decode query result index position.")
        };

        decoder.set_position(0);

        *alloc_decoding_state = Some(AllocDecodingState::new(footer.interpret_alloc_index));

        GeobacterDecoder {
            tcx,
            opaque: decoder,
            cnum_map: Self::compute_cnum_map(tcx, footer.prev_cnums),
            alloc_decoding_session: alloc_decoding_state.as_ref()
                .unwrap()
                .new_decoding_session(),
        }
    }

    // This function builds mapping from previous-session-CrateNum to
    // current-session-CrateNum. There might be CrateNums from the previous
    // Session that don't occur in the current one. For these, the mapping
    // maps to None.
    fn compute_cnum_map(tcx: TyCtxt<'tcx>,
                        prev_cnums: Vec<(u32, String, CrateDisambiguator)>)
                        -> IndexVec<CrateNum, Option<CrateNum>>
    {
        tcx.dep_graph.with_ignore(|| {
            let current_cnums = tcx.all_crate_nums(LOCAL_CRATE).iter().map(|&cnum| {
                let crate_name = tcx.original_crate_name(cnum)
                    .to_string();
                let crate_disambiguator = tcx.crate_disambiguator(cnum);
                ((crate_name, crate_disambiguator), cnum)
            }).collect::<FxHashMap<_, _>>();

            let map_size = prev_cnums.iter()
                .map(|&(cnum, ..)| cnum)
                .max()
                .unwrap_or(0) + 1;
            let mut map = IndexVec::from_elem_n(None, map_size as usize);

            for (prev_cnum, crate_name, crate_disambiguator) in prev_cnums.into_iter() {
                let key = (crate_name, crate_disambiguator);
                map[CrateNum::from_u32(prev_cnum)] = current_cnums.get(&key).cloned();
            }

            // XXX Nothing in the local crate should ever be loaded by Geobacter.
            map[LOCAL_CRATE] = Some(LOCAL_CRATE);
            map
        })
    }
}

impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>> for CrateNum {
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<Self, String> {
        let cnum = CrateNum::from_u32(u32::decode(d)?);
        Ok(d.map_encoded_cnum_to_current(cnum))
    }
}

// This impl makes sure that we get a runtime error when we try decode a
// `DefIndex` that is not contained in a `DefId`. Such a case would be problematic
// because we would not know how to transform the `DefIndex` to the current
// context.
impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>> for DefIndex {
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<DefIndex, String> {
        Err(d.error("trying to decode `DefIndex` outside the context of a `DefId`"))
    }
}

// Both the `CrateNum` and the `DefIndex` of a `DefId` can change in between two
// compilation sessions. We use the `DefPathHash`, which is stable across
// sessions, to map the old `DefId` to the new one.
impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>> for DefId {
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<Self, String> {
        // Load the `DefPathHash` which is was we encoded the `DefId` as.
        let def_path_hash = DefPathHash::decode(d)?;

        // Using the `DefPathHash`, we can lookup the new `DefId`.
        Ok(d.tcx().def_path_hash_to_def_id.as_ref().unwrap()[&def_path_hash])
    }
}

impl<'a, 'tcx> FingerprintDecoder for GeobacterDecoder<'a, 'tcx> {
    fn decode_fingerprint(&mut self) -> Result<Fingerprint, Self::Error> {
        Fingerprint::decode_opaque(&mut self.opaque)
    }
}

impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>> for &'tcx FxHashSet<LocalDefId> {
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<Self, String> {
        RefDecodable::decode(d)
    }
}

impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>>
for &'tcx IndexVec<mir::Promoted, mir::Body<'tcx>>
{
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<Self, String> {
        RefDecodable::decode(d)
    }
}

impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>> for &'tcx [(ty::Predicate<'tcx>, Span)] {
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<Self, String> {
        RefDecodable::decode(d)
    }
}

impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>> for &'tcx [rustc_ast::InlineAsmTemplatePiece] {
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<Self, String> {
        RefDecodable::decode(d)
    }
}

impl<'a, 'tcx> Decodable<GeobacterDecoder<'a, 'tcx>> for &'tcx [Span] {
    fn decode(d: &mut GeobacterDecoder<'a, 'tcx>) -> Result<Self, String> {
        RefDecodable::decode(d)
    }
}

implement_ty_decoder!(GeobacterDecoder<'a, 'tcx>);

impl<'a, 'tcx> TyDecoder<'tcx> for GeobacterDecoder<'a, 'tcx> {
    const CLEAR_CROSS_CRATE: bool = false;

    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn peek_byte(&self) -> u8 {
        self.opaque.data[self.opaque.position()]
    }

    fn position(&self) -> usize {
        self.opaque.position()
    }

    fn cached_ty_for_shorthand<F>(
        &mut self,
        shorthand: usize,
        or_insert_with: F,
    ) -> Result<Ty<'tcx>, Self::Error>
        where
            F: FnOnce(&mut Self) -> Result<Ty<'tcx>, Self::Error>,
    {
        let tcx = self.tcx();

        let cache_key =
            ty::CReaderCacheKey { cnum: CrateNum::ReservedForIncrCompCache, pos: shorthand };

        if let Some(&ty) = tcx.ty_rcache.borrow().get(&cache_key) {
            return Ok(ty);
        }

        let ty = or_insert_with(self)?;
        // This may overwrite the entry, but it should overwrite with the same value.
        tcx.ty_rcache.borrow_mut().insert_same(cache_key, ty);
        Ok(ty)
    }
    fn cached_predicate_for_shorthand<F>(
        &mut self,
        shorthand: usize,
        or_insert_with: F,
    ) -> Result<ty::Predicate<'tcx>, Self::Error>
        where
            F: FnOnce(&mut Self) -> Result<ty::Predicate<'tcx>, Self::Error>,
    {
        let tcx = self.tcx();

        let cache_key =
            ty::CReaderCacheKey { cnum: CrateNum::ReservedForIncrCompCache, pos: shorthand };

        if let Some(&pred) = tcx.pred_rcache.borrow().get(&cache_key) {
            return Ok(pred);
        }

        let pred = or_insert_with(self)?;
        // This may overwrite the entry, but it should overwrite with the same value.
        tcx.pred_rcache.borrow_mut().insert_same(cache_key, pred);
        Ok(pred)
    }

    fn with_position<F, R>(&mut self, pos: usize, f: F) -> R
        where F: FnOnce(&mut Self) -> R
    {
        let new_opaque = opaque::Decoder::new(self.opaque.data, pos);
        let old_opaque = mem::replace(&mut self.opaque, new_opaque);
        let r = f(self);
        self.opaque = old_opaque;
        r
    }

    fn map_encoded_cnum_to_current(&self, cnum: CrateNum) -> CrateNum {
        self.cnum_map[cnum].unwrap_or_else(|| {
            bug!("Could not find new CrateNum for {:?}", cnum)
        })
    }

    fn decode_alloc_id(&mut self) -> Result<interpret::AllocId, Self::Error> {
        let alloc_decoding_session = self.alloc_decoding_session;
        alloc_decoding_session.decode_alloc_id(self)
    }
}


pub struct GeobacterEncoder<'a, 'tcx, E>
    where E: OpaqueEncoder,
{
    pub tcx: TyCtxt<'tcx>,
    pub encoder: &'a mut E,
    // If `Some()`, then we need to add this crate num to the footer data.
    // We do this because we oftentimes don't need to encode every cnum.
    crate_nums: Vec<Option<CrateNum>>,

    type_shorthands: FxHashMap<Ty<'tcx>, usize>,
    predicate_shorthands: FxHashMap<ty::Predicate<'tcx>, usize>,
    interpret_allocs: FxIndexSet<AllocId>,
    interpret_allocs_inverse: Vec<AllocId>,
}

impl<'a, 'tcx, E> GeobacterEncoder<'a, 'tcx, E>
    where E: OpaqueEncoder + 'a,
{
    pub fn new(tcx: TyCtxt<'tcx>, encoder: &'a mut E) -> Self {
        GeobacterEncoder {
            tcx,
            encoder,
            crate_nums: Default::default(),
            type_shorthands: Default::default(),
            predicate_shorthands: Default::default(),
            interpret_allocs: Default::default(),
            interpret_allocs_inverse: Default::default(),
        }
    }
    /// Encode something with additional information that allows to do some
    /// sanity checks when decoding the data again. This method will first
    /// encode the specified tag, then the given value, then the number of
    /// bytes taken up by tag and value. On decoding, we can then verify that
    /// we get the expected tag and read the expected number of bytes.
    fn encode_tagged<T: Encodable<Self>, V: Encodable<Self>>(&mut self,
                                                             tag: T,
                                                             value: &V)
                                                             -> Result<(), E::Error>
    {
        let start_pos = self.position();

        tag.encode(self)?;
        value.encode(self)?;

        let end_pos = self.position();
        ((end_pos - start_pos) as u64).encode(self)
    }
    pub fn finish(mut self) -> Result<(), E::Error> {
        let tcx = self.tcx;

        let interpret_alloc_index = {
            let mut interpret_alloc_index = Vec::new();
            let mut n = 0;
            loop {
                let new_n = self.interpret_allocs_inverse.len();
                // if we have found new ids, serialize those, too
                if n == new_n {
                    // otherwise, abort
                    break;
                }
                interpret_alloc_index.reserve(new_n - n);
                for idx in n..new_n {
                    let id = self.interpret_allocs_inverse[idx];
                    let pos = self.position() as u32;
                    interpret_alloc_index.push(pos);
                    specialized_encode_alloc_id(&mut self, tcx, id)?;
                }
                n = new_n;
            }
            interpret_alloc_index
        };

        let prev_cnums: Vec<_> = self.crate_nums
            .iter()
            .filter_map(|&cnum| cnum)
            .map(|cnum| {
                let crate_name = tcx.original_crate_name(cnum).to_string();
                let crate_disambiguator = tcx.crate_disambiguator(cnum);
                (cnum.as_u32(), crate_name, crate_disambiguator)
            })
            .collect();

        // Encode the file footer
        let footer_pos = self.encoder.encoder_position() as u64;
        self.encode_tagged(TAG_FILE_FOOTER, &Footer {
            prev_cnums,
            interpret_alloc_index,
        })?;

        // Encode the position of the footer as the last 8 bytes of the
        // file so we know where to look for it.
        IntEncodedWithFixedSize(footer_pos).encode(self.encoder.opaque())?;

        // DO NOT WRITE ANYTHING TO THE ENCODER AFTER THIS POINT! The address
        // of the footer must be the last thing in the data stream.

        Ok(())
    }
}

impl<'a, 'tcx> GeobacterEncoder<'a, 'tcx, opaque::Encoder> {
    pub fn with<F>(tcx: TyCtxt<'tcx>, f: F) -> Result<Vec<u8>, <opaque::Encoder as Encoder>::Error>
        where F: for<'b> FnOnce(&mut GeobacterEncoder<'b, 'tcx, opaque::Encoder>) -> Result<(), <opaque::Encoder as Encoder>::Error>,
    {
        let mut encoder = opaque::Encoder::new(vec![]);

        {
            let mut this = GeobacterEncoder {
                tcx,
                encoder: &mut encoder,
                crate_nums: Default::default(),
                type_shorthands: Default::default(),
                predicate_shorthands: Default::default(),
                interpret_allocs: Default::default(),
                interpret_allocs_inverse: Default::default(),
            };
            f(&mut this)?;
            this.finish()?;
        }

        Ok(encoder.into_inner())
    }
}


impl<'a, 'tcx, E> ty_codec::TyEncoder<'tcx> for GeobacterEncoder<'a, 'tcx, E>
    where E: OpaqueEncoder + 'a,
{
    const CLEAR_CROSS_CRATE: bool = false;

    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }
    fn position(&self) -> usize {
        self.encoder.encoder_position()
    }
    fn type_shorthands(&mut self) -> &mut FxHashMap<Ty<'tcx>, usize> {
        &mut self.type_shorthands
    }
    fn predicate_shorthands(&mut self) -> &mut FxHashMap<ty::Predicate<'tcx>, usize> {
        &mut self.predicate_shorthands
    }
    fn encode_alloc_id(&mut self, alloc_id: &interpret::AllocId) -> Result<(), Self::Error> {
        let (index, _) = self.interpret_allocs.insert_full(*alloc_id);

        index.encode(self)
    }
}
impl<'a, 'tcx, E> FingerprintEncoder for GeobacterEncoder<'a, 'tcx, E>
    where
        E: 'a + OpaqueEncoder,
{
    fn encode_fingerprint(&mut self, f: &Fingerprint) -> Result<(), <E as Encoder>::Error> {
        f.encode_opaque(self.encoder.opaque())
            .map_err(|_| unreachable!() )
    }
}

impl<'a, 'tcx, E> Encodable<GeobacterEncoder<'a, 'tcx, E>> for DefId
    where
        E: 'a + OpaqueEncoder,
{
    fn encode(&self, s: &mut GeobacterEncoder<'a, 'tcx, E>) -> Result<(), E::Error> {
        let def_path_hash = s.tcx.def_path_hash(*self);
        def_path_hash.encode(s)
    }
}

impl<'a, 'tcx, E> Encodable<GeobacterEncoder<'a, 'tcx, E>> for DefIndex
    where
        E: 'a + OpaqueEncoder,
{
    fn encode(&self, _: &mut GeobacterEncoder<'a, 'tcx, E>) -> Result<(), E::Error> {
        bug!("encoding `DefIndex` without context");
    }
}

macro_rules! encoder_methods {
    ($($name:ident($ty:ty);)*) => {
        $(fn $name(&mut self, value: $ty) -> Result<(), Self::Error> {
            self.encoder.$name(value)
        })*
    }
}

impl<'a, 'tcx, E> Encoder for GeobacterEncoder<'a, 'tcx, E>
    where E: OpaqueEncoder + 'a,
{
    type Error = E::Error;

    fn emit_unit(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    encoder_methods! {
        emit_usize(usize);
        emit_u128(u128);
        emit_u64(u64);
        emit_u32(u32);
        emit_u16(u16);
        emit_u8(u8);

        emit_isize(isize);
        emit_i128(i128);
        emit_i64(i64);
        emit_i32(i32);
        emit_i16(i16);
        emit_i8(i8);

        emit_bool(bool);
        emit_f64(f64);
        emit_f32(f32);
        emit_char(char);
        emit_str(&str);
    }
}

// An integer that will always encode to 8 bytes.
// Copied from Rust.
struct IntEncodedWithFixedSize(u64);

impl IntEncodedWithFixedSize {
    pub const ENCODED_SIZE: usize = 8;
}

impl Encodable<opaque::Encoder> for IntEncodedWithFixedSize {
    fn encode(&self, e: &mut opaque::Encoder) -> Result<(), !> {
        let start_pos = e.position();
        for i in 0..IntEncodedWithFixedSize::ENCODED_SIZE {
            ((self.0 >> (i * 8)) as u8).encode(e)?;
        }
        let end_pos = e.position();
        assert_eq!((end_pos - start_pos), IntEncodedWithFixedSize::ENCODED_SIZE);
        Ok(())
    }
}

impl<'a> Decodable<opaque::Decoder<'a>> for IntEncodedWithFixedSize {
    fn decode(decoder: &mut opaque::Decoder<'a>) -> Result<IntEncodedWithFixedSize, String> {
        let mut value: u64 = 0;
        let start_pos = decoder.position();

        for i in 0..IntEncodedWithFixedSize::ENCODED_SIZE {
            let byte: u8 = Decodable::decode(decoder)?;
            value |= (byte as u64) << (i * 8);
        }

        let end_pos = decoder.position();
        assert_eq!((end_pos - start_pos), IntEncodedWithFixedSize::ENCODED_SIZE);

        Ok(IntEncodedWithFixedSize(value))
    }
}

pub trait DecoderWithPosition: Decoder {
    fn position(&self) -> usize;
}

impl<'a> DecoderWithPosition for opaque::Decoder<'a> {
    fn position(&self) -> usize {
        self.position()
    }
}

impl<'a, 'tcx> DecoderWithPosition for GeobacterDecoder<'a, 'tcx> {
    fn position(&self) -> usize {
        self.opaque.position()
    }
}

// Decodes something that was encoded with `encode_tagged()` and verify that the
// tag matches and the correct amount of bytes was read.
fn decode_tagged<D, T, V>(decoder: &mut D, expected_tag: T) -> Result<V, D::Error>
    where
        T: Decodable<D> + Eq + ::std::fmt::Debug,
        V: Decodable<D>,
        D: DecoderWithPosition,
{
    let start_pos = decoder.position();

    let actual_tag = T::decode(decoder)?;
    assert_eq!(actual_tag, expected_tag);
    let value = V::decode(decoder)?;
    let end_pos = decoder.position();

    let expected_len: u64 = Decodable::decode(decoder)?;
    assert_eq!((end_pos - start_pos) as u64, expected_len);

    Ok(value)
}
