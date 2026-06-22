//! Utilities for computing the serialized size of values in the
//! [XDR (External Data Representation)](https://datatracker.ietf.org/doc/html/rfc4506) format.
//!
//! This module defines the [`XDRSize`] trait together with implementations for
//! the primitive and container types that are used by the project.
//!
//! `XDRSize::xdr_size()` returns the number of bytes that a value will occupy
//! **after XDR serialization**.
//!
//! In this module, all XDR values are assumed to be laid out according to the
//! XDR 4-byte alignment rule:
//!
//! - every serialized value occupies a number of bytes that is a multiple of 4;
//! - fixed-size scalar values already satisfy this naturally (`u32`, `bool`, `u64`, etc.);
//! - opaque/string-like values are padded with zero bytes up to the next 4-byte boundary;
//! - compound values are built from components whose serialized sizes also follow
//!   this alignment rule.
//!
//! The returned size therefore includes:
//! - the actual payload size;
//! - XDR-required padding up to a 4-byte boundary;
//! - length prefixes for variable-length values such as `String` and `Vec<u8>`;
//! - discriminants for `Option<T>` / `Result<T, E>` represented as XDR booleans.
//!
//! # Encoding rules used in this module
//!
//! All serialized values are assumed to satisfy the XDR alignment rule:
//! **their serialized size must be a multiple of 4 bytes**.
//!
//! For fixed-size scalar types this is already true by construction.
//! For fixed and variable opaque values (`[u8; N]`, `Vec<u8>`, `String`, `PathBuf`),
//! the payload is padded to the next multiple of 4 bytes.
//!
//! The implementations in this module follow these conventions:
//!
//! - `u32`, `i32`, `usize`, `bool` are treated as XDR **4-byte integers**;
//! - `u64` is treated as an XDR **8-byte hyper integer**;
//! - `[u8; N]` is treated as a fixed opaque byte array padded to a multiple of 4 bytes;
//! - `Vec<u8>` is treated as an XDR variable-length opaque value:
//!   4-byte length prefix + padded byte payload;
//! - `String` and `PathBuf` are treated as XDR strings/opaque byte sequences:
//!   4-byte length prefix + padded UTF-8 payload;
//! - `Vec<T>` for `T != u8` is treated as a variable-length XDR array:
//!   4-byte length prefix + sum of serialized element sizes;
//! - `Option<T>` is encoded as an XDR boolean flag followed by the payload for `Some(T)`;
//! - `Result<T, E>` is encoded as an XDR boolean flag followed by either the `Ok` payload
//!   or the `Err` payload.
//!
//! # Important note about `usize`
//!
//! `usize` is encoded here as a **4-byte XDR integer**. This is convenient in code that
//! conceptually uses `usize` as a size/count field but still serializes it as an XDR `u32`.
//! Note that this is a project-level convention rather than a property of Rust `usize` itself.
//!
//! # Derive support
//!
//! The trait can be derived with [`XDRSize`](nfs_mamont_derive::XDRSize) for structs and enums.
//!
//! For structs, the derived implementation sums the XDR sizes of all fields in declaration order.
//! For enums, the derived implementation adds a 4-byte discriminant and then the size of the
//! payload of the active variant.

#[cfg(test)]
mod tests;

use std::path::PathBuf;

pub use nfs_mamont_derive::XDRSize;

/// Returns the size of a value in its XDR-serialized representation.
///
/// This trait is used to compute how many bytes a value will occupy when
/// encoded using the XDR rules adopted by this project.
///
/// # Alignment invariant
///
/// All serialized values are assumed to obey the XDR 4-byte alignment rule:
/// the total serialized size of a value must be a multiple of 4 bytes.
///
/// For fixed-size scalar types this is naturally true. For variable-size and
/// opaque/string-like values, the serialized representation includes enough
/// padding bytes to round the payload size up to the next multiple of 4.
///
/// As a consequence, every correct `XDRSize` implementation should return
/// a size that is aligned to [`XDRSize::ALIGNMENT`].
///
/// # Constants
///
/// The trait exposes several associated constants that correspond to common XDR sizes:
///
/// - [`ALIGNMENT`](XDRSize::ALIGNMENT) — XDR alignment boundary in bytes (always 4);
/// - [`INTEGER`](XDRSize::INTEGER) — size of an XDR integer (`u32`/`i32`/`bool`) in bytes;
/// - [`HYPER_INTEGER`](XDRSize::HYPER_INTEGER) — size of an XDR hyper integer (`u64`) in bytes.
pub trait XDRSize {
    /// XDR alignment boundary in bytes.
    ///
    /// XDR requires values of variable-length opaque/string-like types to be padded
    /// so that the total payload length is aligned to a 4-byte boundary.
    const ALIGNMENT: usize = 4;

    /// Size of an XDR integer in bytes.
    ///
    /// Used for values encoded as XDR 32-bit integers, including `u32`, `i32`,
    /// booleans, and length/discriminant fields.
    const INTEGER: usize = 4;

    /// Size of an XDR hyper integer in bytes.
    ///
    /// Used for values encoded as XDR 64-bit integers such as `u64`.
    const HYPER_INTEGER: usize = 8;

    /// Returns the number of bytes this value occupies when serialized as XDR.
    ///
    /// The returned size is expected to satisfy the XDR alignment rule, i.e.
    /// it should be a multiple of [`XDRSize::ALIGNMENT`].
    ///
    /// For variable-size values this includes any required trailing padding.
    fn xdr_size(&self) -> usize;
}

/// `u32` is encoded as a 4-byte XDR integer.
impl XDRSize for u32 {
    fn xdr_size(&self) -> usize {
        Self::INTEGER
    }
}

/// `i32` is encoded as a 4-byte XDR integer.
impl XDRSize for i32 {
    fn xdr_size(&self) -> usize {
        Self::INTEGER
    }
}

/// `u64` is encoded as an 8-byte XDR hyper integer.
impl XDRSize for u64 {
    fn xdr_size(&self) -> usize {
        Self::HYPER_INTEGER
    }
}

/// `usize` is encoded as a 4-byte XDR integer.
///
/// # Note
///
/// This is a project-level convention: although Rust `usize` is platform-dependent,
/// it is treated here as an XDR integer and therefore always occupies 4 bytes in the
/// serialized representation.
impl XDRSize for usize {
    fn xdr_size(&self) -> usize {
        Self::INTEGER
    }
}

/// A fixed-size byte array is treated as XDR fixed opaque data.
///
/// Its serialized size is the array length rounded up to the next multiple of 4.
impl<const N: usize> XDRSize for [u8; N] {
    fn xdr_size(&self) -> usize {
        (N + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1)
    }
}

/// `Vec<u8>` is treated as XDR variable-length opaque data.
///
/// The serialized size is:
///
/// `4-byte length prefix + padded byte payload`.
impl XDRSize for Vec<u8> {
    fn xdr_size(&self) -> usize {
        Self::INTEGER + ((self.len() + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1))
    }
}

/// `Vec<T>` is treated as an XDR variable-length array.
///
/// The serialized size is:
///
/// `4-byte length prefix + sum of serialized element sizes`.
///
/// This implementation is intended for general XDR arrays. Note that `Vec<u8>`
/// has a dedicated implementation and is treated as XDR opaque data instead.
impl<T: XDRSize> XDRSize for Vec<T> {
    fn xdr_size(&self) -> usize {
        Self::INTEGER + self.iter().map(|item| item.xdr_size()).sum::<usize>()
    }
}

/// `bool` is encoded as a 4-byte XDR boolean/integer.
impl XDRSize for bool {
    fn xdr_size(&self) -> usize {
        Self::INTEGER
    }
}

/// `Option<T>` is encoded as:
///
/// - an XDR boolean discriminant;
/// - followed by the payload only for `Some(T)`.
///
/// Therefore:
///
/// - `None` occupies 4 bytes;
/// - `Some(x)` occupies `4 + x.xdr_size()`.
impl<T: XDRSize> XDRSize for Option<T> {
    fn xdr_size(&self) -> usize {
        match self {
            Some(x) => true.xdr_size() + x.xdr_size(),
            None => false.xdr_size(),
        }
    }
}

/// `String` is encoded as a variable-length XDR string.
///
/// The serialized size is:
///
/// `4-byte length prefix + padded UTF-8 byte payload`.
impl XDRSize for String {
    fn xdr_size(&self) -> usize {
        Self::INTEGER + ((self.len() + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1))
    }
}

/// `PathBuf` is encoded as a variable-length XDR string-like value.
///
/// The serialized size is:
///
/// `4-byte length prefix + padded UTF-8 byte payload`.
impl XDRSize for PathBuf {
    fn xdr_size(&self) -> usize {
        let path_str = self.to_string_lossy();
        Self::INTEGER + ((path_str.len() + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1))
    }
}

/// `Result<T, E>` is encoded as:
///
/// - an XDR boolean discriminant;
/// - followed by either the `Ok(T)` payload or the `Err(E)` payload.
///
/// Therefore:
///
/// - `Ok(v)` occupies `4 + v.xdr_size()`
/// - `Err(e)` occupies `4 + e.xdr_size()`
impl<T: XDRSize, E: XDRSize> XDRSize for Result<T, E> {
    fn xdr_size(&self) -> usize {
        match self {
            Ok(value) => true.xdr_size() + value.xdr_size(),
            Err(err) => false.xdr_size() + err.xdr_size(),
        }
    }
}

/// `Box<T>` has the same serialized size as `T`.
///
/// Boxing only changes ownership/allocation strategy in Rust and does not affect
/// the XDR wire representation.
impl<T: XDRSize> XDRSize for Box<T> {
    fn xdr_size(&self) -> usize {
        (**self).xdr_size()
    }
}
