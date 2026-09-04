//! SCALE codec implementations, behind the `scale` feature.
//!
//! Bitfields travel inside consensus proofs that cross a Substrate runtime boundary, so they need
//! `parity-scale-codec` alongside SSZ. This is deliberately confined to one module: it is the part
//! of this fork that upstream will never take, since it puts a Substrate dependency into an SSZ
//! library, so keeping it in a single file keeps rebases onto upstream cheap.
//!
//! # Wire compatibility
//!
//! The encoding matches `ssz-rs`, which these types replace: a bitfield encodes as `Vec<bool>`,
//! one byte per bit, **not** as its packed SSZ bytes. That is wasteful but it is what previously
//! encoded proofs contain, so it is not ours to change here.

use crate::{Bitfield, Fixed, ProgressiveBitList, Variable};
use alloc::vec::Vec;
use codec::{Decode, Encode, Error, Input, Output};
use typenum::Unsigned;

/// Encode any bitfield as `Vec<bool>`, matching `ssz-rs`.
fn encode_bits<O: Output + ?Sized, I: Iterator<Item = bool>>(bits: I, dest: &mut O) {
    bits.collect::<Vec<bool>>().encode_to(dest)
}

impl<N: Unsigned + Clone> Encode for Bitfield<Variable<N>> {
    fn encode_to<O: Output + ?Sized>(&self, dest: &mut O) {
        encode_bits(self.iter(), dest)
    }
}

impl<N: Unsigned + Clone> Decode for Bitfield<Variable<N>> {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let bits: Vec<bool> = Decode::decode(input)?;
        let mut out =
            Self::with_capacity(bits.len()).map_err(|_| Error::from("BitList: invalid length"))?;
        for (i, bit) in bits.into_iter().enumerate() {
            out.set(i, bit)
                .map_err(|_| Error::from("BitList: invalid length"))?;
        }
        Ok(out)
    }
}

impl<N: Unsigned + Clone> Encode for Bitfield<Fixed<N>> {
    fn encode_to<O: Output + ?Sized>(&self, dest: &mut O) {
        encode_bits(self.iter(), dest)
    }
}

impl<N: Unsigned + Clone> Decode for Bitfield<Fixed<N>> {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let bits: Vec<bool> = Decode::decode(input)?;
        if bits.len() > N::to_usize() {
            return Err(Error::from("BitVector: encoded data too large"));
        }
        // A fixed bitfield is always `N` bits wide; a short encoding leaves the tail unset, which
        // is how `ssz-rs` behaved.
        let mut out = Self::new();
        for (i, bit) in bits.into_iter().enumerate() {
            out.set(i, bit)
                .map_err(|_| Error::from("BitVector: bit index out of range"))?;
        }
        Ok(out)
    }
}

impl Encode for ProgressiveBitList {
    fn encode_to<O: Output + ?Sized>(&self, dest: &mut O) {
        encode_bits(self.iter(), dest)
    }
}

impl Decode for ProgressiveBitList {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let bits: Vec<bool> = Decode::decode(input)?;
        let mut out = Self::with_capacity(bits.len());
        for (i, bit) in bits.into_iter().enumerate() {
            out.set(i, bit)
                .map_err(|_| Error::from("ProgressiveBitList: bit index out of range"))?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BitList, BitVector};
    use typenum::{U16, U8};

    fn bits(values: &[bool]) -> BitVector<U8> {
        let mut out = BitVector::<U8>::new();
        for (i, v) in values.iter().enumerate() {
            out.set(i, *v).unwrap();
        }
        out
    }

    /// The distinctive part of the `ssz-rs` format: one byte per bit, not packed SSZ bytes.
    /// Existing encoded proofs depend on this, so it is pinned rather than left implicit.
    #[test]
    fn bitfields_encode_as_a_vec_of_bools() {
        let values = [true, false, true, true, false, false, false, true];
        let encoded = bits(&values).encode();

        assert_eq!(encoded, values.to_vec().encode());
        // 8 bits is one byte packed, but 8 bytes plus a compact length prefix here.
        assert_eq!(encoded.len(), 9);
    }

    #[test]
    fn bit_vector_round_trips() {
        let value = bits(&[true, false, true, true, false, false, false, true]);
        assert_eq!(
            BitVector::<U8>::decode(&mut &value.encode()[..]).unwrap(),
            value
        );
    }

    #[test]
    fn bit_list_round_trips() {
        let mut value = BitList::<U16>::with_capacity(12).unwrap();
        value.set(0, true).unwrap();
        value.set(7, true).unwrap();
        value.set(11, true).unwrap();

        let decoded = BitList::<U16>::decode(&mut &value.encode()[..]).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn decoding_rejects_an_over_long_bit_vector() {
        let too_many = alloc::vec![true; 9];
        assert!(BitVector::<U8>::decode(&mut &too_many.encode()[..]).is_err());
    }

    #[test]
    fn progressive_bit_list_round_trips() {
        let mut value = ProgressiveBitList::with_capacity(20);
        value.set(3, true).unwrap();
        value.set(19, true).unwrap();

        let decoded = ProgressiveBitList::decode(&mut &value.encode()[..]).unwrap();
        assert_eq!(decoded, value);
    }
}
