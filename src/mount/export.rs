//! Defines Mount version 3 Export interface (Procedure 5).
//!
//! as defined in RFC 1813 section 5.2.5.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.5>.

use super::ExportEntry;

/// Success result.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Debug))]
pub struct Success {
    /// Vector of export entries, each containing an exported
    /// directory and a vector of clients that are allowed
    /// to mount the specified directory.
    pub exports: Vec<ExportEntry>,
}
