//! Kani harnesses for RPC credential parsing.

use crate::parser::rpc::authsys_parms;
use crate::parser::Error;
use crate::rpc::{AUTH_SYS_MAX_GIDS, AUTH_SYS_MAX_MACHINE_NAME};
use crate::serializer;

/// Upper bound on an arbitrary credential body, in bytes.
const MAX_BODY: usize = 28;
/// Upper bound on a machine name that the parser has to validate as UTF-8.
///
/// UTF-8 validation over a symbolic length is expensive; longer names are
/// covered by the `string_max_size` harnesses.
const MAX_VALIDATED_NAME: u32 = 4;
/// Length of the machine name in the round-trip harness, in bytes.
const NAME_LEN: usize = 3;
/// Upper bound on the auxiliary groups in the round-trip harness.
const MAX_GIDS: usize = 3;

/// Every credential body up to [`MAX_BODY`] bytes is either rejected or
/// parsed into a credential within the RFC 5531 limits, without panicking.
///
/// The declared machine name is either short enough to validate or above
/// the limit; every other byte, and the body length, is arbitrary.
#[kani::proof]
#[kani::unwind(18)]
fn authsys_parms_arbitrary_body() {
    let len: usize = kani::any_where(|len| *len <= MAX_BODY);
    let mut body: [u8; MAX_BODY] = kani::any();
    let name_len: u32 = kani::any_where(|len| {
        *len <= MAX_VALIDATED_NAME || *len as usize > AUTH_SYS_MAX_MACHINE_NAME
    });
    body[4..8].copy_from_slice(&name_len.to_be_bytes());
    let mut src = &body[..len];

    if let Ok(params) = authsys_parms(&mut src) {
        assert!(params.machine_name.len() <= AUTH_SYS_MAX_MACHINE_NAME);
        assert!(params.gids.len() <= AUTH_SYS_MAX_GIDS);
    }
}

/// A group count above [`AUTH_SYS_MAX_GIDS`] is rejected from the count word
/// alone, before the parser reserves memory or reads the groups.
#[kani::proof]
fn authsys_parms_rejects_gid_count_by_header() {
    let count: u32 = kani::any();
    kani::assume(count as usize > AUTH_SYS_MAX_GIDS);

    let mut wire = Vec::new();
    serializer::u32(&mut wire, kani::any()).unwrap();
    serializer::u32(&mut wire, 0).unwrap();
    serializer::u32(&mut wire, kani::any()).unwrap();
    serializer::u32(&mut wire, kani::any()).unwrap();
    serializer::u32(&mut wire, count).unwrap();

    assert!(matches!(authsys_parms(&mut wire.as_slice()), Err(Error::MaxElemLimit)));
}

/// Small well-formed credentials round-trip and consume the whole body.
#[kani::proof]
#[kani::unwind(9)]
fn authsys_parms_round_trip() {
    let stamp: u32 = kani::any();
    let uid: u32 = kani::any();
    let gid: u32 = kani::any();

    let name: [u8; NAME_LEN] = kani::any();
    kani::assume(name.is_ascii());
    let name = std::str::from_utf8(&name).unwrap();

    let gid_count: usize = kani::any_where(|count| *count <= MAX_GIDS);
    let gids: [u32; MAX_GIDS] = kani::any();
    let gids = &gids[..gid_count];

    let mut wire = Vec::new();
    serializer::u32(&mut wire, stamp).unwrap();
    serializer::string_max_size(&mut wire, name, AUTH_SYS_MAX_MACHINE_NAME).unwrap();
    serializer::u32(&mut wire, uid).unwrap();
    serializer::u32(&mut wire, gid).unwrap();
    serializer::usize_as_u32(&mut wire, gid_count).unwrap();
    for aux in gids {
        serializer::u32(&mut wire, *aux).unwrap();
    }

    let mut src = wire.as_slice();
    let params = authsys_parms(&mut src).unwrap();
    assert!(src.is_empty());
    assert_eq!(params.stamp, stamp);
    assert_eq!(params.machine_name, name);
    assert_eq!(params.uid, uid);
    assert_eq!(params.gid, gid);
    assert_eq!(params.gids.as_slice(), gids);
}
