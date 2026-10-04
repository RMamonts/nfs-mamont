//! Kani harnesses for XDR primitive parsers and their serializer counterparts.
//!
//! Fixed-size primitives are proved for every value; variable-length opaques
//! are checked up to [`MAX_LEN`] bytes. Strings are checked for a few concrete
//! lengths only: UTF-8 validation over a symbolic length is too expensive.

use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::consts::xdr::ALIGNMENT;
use crate::parser::nfsv3::file;
use crate::parser::primitive;
use crate::parser::Error;
use crate::serializer;

/// Upper bound on the length of variable-length opaques in bounded harnesses.
///
/// Covers every padding length and a value longer than one XDR unit.
const MAX_LEN: usize = 5;

/// Serializer padding completes any length to the XDR alignment with zero bytes.
#[kani::proof]
#[kani::unwind(5)]
fn serializer_padding_aligns() {
    let n: usize = kani::any();
    let mut dest = Vec::new();
    serializer::padding(&mut dest, n).unwrap();

    assert!(dest.len() < ALIGNMENT);
    assert_eq!((n % ALIGNMENT + dest.len()) % ALIGNMENT, 0);
    assert!(dest.iter().all(|byte| *byte == 0));
}

/// Parser padding consumes exactly as many bytes as the serializer emits.
#[kani::proof]
#[kani::unwind(5)]
fn parser_padding_matches_serializer() {
    let n: usize = kani::any();
    let wire: [u8; ALIGNMENT] = kani::any();
    let mut src = &wire[..];
    primitive::padding(&mut src, n).unwrap();

    let mut emitted = Vec::new();
    serializer::padding(&mut emitted, n).unwrap();
    assert_eq!(ALIGNMENT - src.len(), emitted.len());
}

#[kani::proof]
fn u32_round_trip() {
    let value: u32 = kani::any();
    let mut wire = Vec::new();
    serializer::u32(&mut wire, value).unwrap();

    let mut src = wire.as_slice();
    assert_eq!(primitive::u32(&mut src).unwrap(), value);
    assert!(src.is_empty());
}

#[kani::proof]
fn u64_round_trip() {
    let value: u64 = kani::any();
    let mut wire = Vec::new();
    serializer::u64(&mut wire, value).unwrap();

    let mut src = wire.as_slice();
    assert_eq!(primitive::u64(&mut src).unwrap(), value);
    assert!(src.is_empty());
}

/// Signed values (NLM `svid`) are sent as their `u32` bit pattern.
#[kani::proof]
fn i32_round_trip() {
    let value: i32 = kani::any();
    let mut wire = Vec::new();
    serializer::u32(&mut wire, value as u32).unwrap();

    let mut src = wire.as_slice();
    assert_eq!(primitive::i32(&mut src).unwrap(), value);
    assert!(src.is_empty());
}

#[kani::proof]
fn bool_round_trip() {
    let value: bool = kani::any();
    let mut wire = Vec::new();
    serializer::bool(&mut wire, value).unwrap();

    let mut src = wire.as_slice();
    assert_eq!(primitive::bool(&mut src).unwrap(), value);
    assert!(src.is_empty());
}

/// An XDR boolean is accepted only for the discriminants `0` and `1`.
#[kani::proof]
fn bool_rejects_other_discriminants() {
    let raw: u32 = kani::any();
    let wire = raw.to_be_bytes();
    let parsed = primitive::bool(&mut &wire[..]);

    match raw {
        0 => {
            assert!(matches!(parsed, Ok(false)));
        }
        1 => {
            assert!(matches!(parsed, Ok(true)));
        }
        _ => {
            assert!(matches!(parsed, Err(Error::EnumDiscMismatch)));
        }
    }
}

#[kani::proof]
#[kani::unwind(13)]
fn array_round_trip() {
    let value: [u8; NFS3_FHSIZE] = kani::any();
    let mut wire = Vec::new();
    serializer::array(&mut wire, value).unwrap();
    assert_eq!(wire.len() % ALIGNMENT, 0);

    let mut src = wire.as_slice();
    assert_eq!(primitive::array::<NFS3_FHSIZE>(&mut src).unwrap(), value);
    assert!(src.is_empty());
}

/// `nfs_fh3` round-trips, and any declared length other than
/// [`NFS3_FHSIZE`] is rejected from the length word alone.
#[kani::proof]
#[kani::unwind(13)]
fn file_handle_round_trip() {
    let declared: u32 = kani::any();
    let payload: [u8; NFS3_FHSIZE] = kani::any();
    let mut wire = Vec::new();
    serializer::u32(&mut wire, declared).unwrap();
    serializer::array(&mut wire, payload).unwrap();

    let mut src = wire.as_slice();
    let parsed = file::handle(&mut src);
    if declared as usize == NFS3_FHSIZE {
        assert_eq!(parsed.unwrap().0, payload);
        assert!(src.is_empty());
    } else {
        assert!(matches!(parsed, Err(Error::BadFileHandle)));
    }
}

/// Bounded opaques round-trip; an oversized value is rejected by the
/// serializer without writing anything.
#[kani::proof]
#[kani::unwind(10)]
fn vec_max_size_round_trip() {
    let len: usize = kani::any_where(|len| *len <= MAX_LEN);
    let max_size: usize = kani::any_where(|max| *max <= MAX_LEN);
    let data: [u8; MAX_LEN] = kani::any();
    let data = &data[..len];

    let mut wire = Vec::new();
    let serialized = serializer::vec_max_size(&mut wire, data, max_size);
    if len > max_size {
        assert!(serialized.is_err());
        assert!(wire.is_empty());
        return;
    }
    serialized.unwrap();
    assert_eq!(wire.len() % ALIGNMENT, 0);

    let mut src = wire.as_slice();
    assert_eq!(primitive::vec_max_size(&mut src, max_size).unwrap(), data);
    assert!(src.is_empty());
}

/// A declared length above `max_size` is rejected from the length word alone,
/// before the parser allocates or reads the body.
#[kani::proof]
fn vec_max_size_rejects_by_header() {
    let declared: u32 = kani::any();
    let max_size: usize = kani::any();
    kani::assume(declared as usize > max_size);
    let wire = declared.to_be_bytes();

    let parsed = primitive::vec_max_size(&mut &wire[..], max_size);
    assert!(matches!(parsed, Err(Error::MaxElemLimit)));
}

/// A string of `LEN` arbitrary bytes is either accepted and round-trips
/// through the serializer, or rejected as non-UTF-8; ASCII is always accepted.
fn check_string_round_trip<const LEN: usize>() {
    let bytes: [u8; LEN] = kani::any();
    let mut wire = Vec::new();
    serializer::vec_max_size(&mut wire, &bytes, LEN).unwrap();

    match primitive::string_max_size(&mut wire.as_slice(), LEN) {
        Ok(parsed) => {
            assert_eq!(parsed.as_bytes(), &bytes);

            let mut reserialized = Vec::new();
            serializer::string_max_size(&mut reserialized, &parsed, LEN).unwrap();
            assert_eq!(reserialized, wire);
        }
        Err(err) => {
            assert!(matches!(err, Error::IncorrectString(_)));
            assert!(!bytes.is_ascii());
        }
    }
}

/// One byte: the longest padding.
#[kani::proof]
#[kani::unwind(9)]
fn string_max_size_round_trip_one_byte() {
    check_string_round_trip::<1>();
}

/// One full XDR unit: no padding, room for a multi-byte character.
#[kani::proof]
#[kani::unwind(9)]
fn string_max_size_round_trip_four_bytes() {
    check_string_round_trip::<4>();
}
