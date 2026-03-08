//! Defines Mount version 3 UMNT procedure data types (Procedure 3).
//!
//! as defined in RFC 1813 section 5.2.3.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.3>.

use crate::vfs::file;

/// Arguments for the Unmount operation, containing the path to be unmounted.
#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Debug)]
pub struct UnmountArgs(pub file::Path);
