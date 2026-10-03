use std::io::Cursor;
use crate::nlm::Nlm4Stats;
use crate::parser::nlm::granted::granted;
use super::xdr;

#[test]
fn test_granted() {
    let mut data = Vec::new();
    // Cookie
    data.extend(xdr::u64_val(7));
    // Nlm4Stats::Granted
    data.extend(xdr::u32_val(0));

    let result = granted(&mut Cursor::new(data)).unwrap();

    assert_eq!(result.cookie.raw(), 7);
    assert_eq!(result.stat, Nlm4Stats::Granted);
}
