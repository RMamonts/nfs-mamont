use crate::consts::nlm::OPAQUE_HANDLE_SIZE;

use super::OpaqueHandle;

#[test]
fn opaque_handle_bytes() {
    let bytes = [0x01; OPAQUE_HANDLE_SIZE].to_vec();
    let verifier = bytes.clone();
    let oh = OpaqueHandle::new(bytes).unwrap();
    assert_eq!(oh.as_bytes(), verifier.as_slice());
}
