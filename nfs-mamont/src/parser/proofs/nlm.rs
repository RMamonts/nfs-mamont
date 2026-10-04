//! Kani harnesses for NLMv4 argument parsing.

use crate::consts::nlm::OPAQUE_HANDLE_SIZE;
use crate::parser::nlm::opaque_handle;
use crate::parser::Error;

/// The lock-owner handle is an `opaque<OPAQUE_HANDLE_SIZE>`: a declared length
/// above the cap is rejected from the length word alone, before the parser
/// allocates or reads the body.
#[kani::proof]
fn opaque_handle_rejects_oversized_length_by_header() {
    let declared: u32 = kani::any();
    kani::assume(declared as usize > OPAQUE_HANDLE_SIZE);
    let wire = declared.to_be_bytes();

    assert!(matches!(opaque_handle(&mut &wire[..]), Err(Error::MaxElemLimit)));
}
