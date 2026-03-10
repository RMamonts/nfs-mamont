//! Defines Mount version 3 Umnt interface (Procedure 3).
//!
//! as defined in RFC 1813 section 5.2.3.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3>.

use crate::vfs::file;

/// Arguments for the Unmount operation, containing the path to be unmounted.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary, Clone))]
pub struct UnmountArgs(pub file::Path);
