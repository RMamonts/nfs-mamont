//! Defines the NFSv4 ACCESS operation types, RFC 7530 §16.1.

use super::Error;

/// Mask of access rights used by the ACCESS operation.
///
/// The underlying bit values follow the `ACCESS4_*` constants from RFC 7530
/// §16.1.2.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mask(u32);

impl Mask {
    /// ACCESS4_READ - read data from file or read a directory.
    pub const READ: u32 = 0x00000001;
    /// ACCESS4_LOOKUP - look up a name in a directory.
    pub const LOOKUP: u32 = 0x00000002;
    /// ACCESS4_MODIFY - rewrite existing file data or modify existing
    /// directory entries.
    pub const MODIFY: u32 = 0x00000004;
    /// ACCESS4_EXTEND - write new data or add directory entries.
    pub const EXTEND: u32 = 0x00000008;
    /// ACCESS4_DELETE - delete an existing directory entry.
    pub const DELETE: u32 = 0x00000010;
    /// ACCESS4_EXECUTE - execute file.
    pub const EXECUTE: u32 = 0x00000020;

    /// All access rights that are defined by the specification.
    pub const ALL: u32 =
        Self::READ | Self::LOOKUP | Self::MODIFY | Self::EXTEND | Self::DELETE | Self::EXECUTE;

    /// Builds a mask from the raw on-wire bit value, keeping only the bits
    /// that are defined by the specification.
    pub fn from_wire(raw: u32) -> Self {
        Self(raw & Self::ALL)
    }

    /// Returns the raw bit value of the mask.
    pub fn bits(self) -> u32 {
        self.0
    }

    /// Returns `true` if all the bits of `flag` are set in the mask.
    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}

/// ACCESS arguments (`ACCESS4args`).
pub struct Args {
    /// Bitmask of access permissions to check.
    pub access: Mask,
}

/// Success result (`ACCESS4resok`).
pub struct Success {
    /// Bitmask of access rights for which the server can verify reliably.
    pub supported: Mask,
    /// Bitmask of access rights available to the user for the given file
    /// handle.
    pub access: Mask,
}

/// Fail result (`ACCESS4res`).
pub struct Fail {
    /// Error on failure.
    pub error: Error,
}
