//! The XDR (External Data Representation) module defines data structures and methods
//! for serializing/deserializing data according to RFC 1832 standard.
//!
//! XDR provides machine-independent data representation format,
//! which is critical for network protocols like NFS.
//!
//! All data structures that require serialization/deserialization
//! for network transmission must implement the XDR trait.

use std::io::{Read, Write};

use byteorder::BigEndian;
use byteorder::{ReadBytesExt, WriteBytesExt};
use num_traits::ToPrimitive;

pub mod mount;
pub mod nfs3;
pub mod portmap;
pub mod rpc;

/// Type alias for the standard endianness used in XDR serialization (Big Endian).
pub type XDREndian = BigEndian;

/// The XDR trait defines methods for serializing and deserializing data structures
/// according to the External Data Representation (XDR) standard defined in RFC 1832.
///
/// XDR provides a standard way of representing data in a machine-independent format,
/// which is critical for network protocols like NFS.
///
/// All data structures that need to be serialized to or deserialized from
/// the network must implement this trait.
#[allow(clippy::upper_case_acronyms)]
pub trait XDR: Serialize + Deserialize {}

impl<T: Serialize + Deserialize> XDR for T {}

pub trait Serialize {
    /// Serializes the implementing type to the provided writer.
    ///
    /// # Arguments
    /// * `dest` - A mutable reference to any type that implements Write.
    ///
    /// # Returns
    /// * `std::io::Result<()>` - Ok(()) on success, or an error if serialization fails.
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()>;
}

pub trait Deserialize {
    /// Deserializes data from the provided reader into the implementing type.
    ///
    /// # Arguments
    /// * `src` - A mutable reference to any type that implements Read.
    ///
    /// # Returns
    /// * `std::io::Result<()>` - Ok(()) on success, or an error if deserialization fails.
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()>;
}

pub fn deserialize<T>(src: &mut impl Read) -> std::io::Result<T>
where
    T: Deserialize + Default,
{
    let mut val = T::default();
    val.deserialize(src)?;

    Ok(val)
}

/// Macro for implementing XDR serialization and deserialization for enumerations.
///
/// This macro simplifies implementation of the XDR trait for enum types
/// by providing standard serialization and deserialization as 32-bit integers.
#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! SerializeEnum {
    ($t:ident) => {
        impl Serialize for $t {
            fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
                dest.write_u32::<$crate::xdr::XDREndian>(*self as u32)
            }
        }
    };
}

#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! DeserializeEnum {
    ($t:ident) => {
        impl Deserialize for $t {
            fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
                let r: u32 = src.read_u32::<$crate::xdr::XDREndian>()?;
                if let Some(p) = FromPrimitive::from_u32(r) {
                    *self = p;
                    return Ok(());
                }
                Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid value for {}", stringify!($t)),
                ))
            }
        }
    };
}

/// XDR implementation for boolean values.
///
/// Booleans are serialized as 4-byte big endian integers
/// where 0 represents false and any non-zero value represents true.
impl Serialize for bool {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        let val: u32 = *self as u32;
        dest.write_u32::<XDREndian>(val)
    }
}

impl Deserialize for bool {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let val: u32 = src.read_u32::<XDREndian>()?;
        *self = val > 0;
        Ok(())
    }
}

/// XDR implementation for 32-bit signed integers.
///
/// Integers are serialized as 4-byte big endian values.
impl Serialize for i32 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_i32::<XDREndian>(*self)
    }
}

impl Deserialize for i32 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_i32::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for 64-bit signed integers.
///
/// 64-bit integers are serialized as 8-byte big endian values.
impl Serialize for i64 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_i64::<XDREndian>(*self)
    }
}

impl Deserialize for i64 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_i64::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for 32-bit unsigned integers.
///
/// Unsigned 32-bit integers are serialized as 4-byte big endian values.
impl Serialize for u32 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_u32::<XDREndian>(*self)
    }
}

impl Deserialize for u32 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_u32::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for 64-bit unsigned integers.
///
/// Unsigned 64-bit integers are serialized as 8-byte big endian values.
impl Serialize for u64 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_u64::<XDREndian>(*self)
    }
}

impl Deserialize for u64 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_u64::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for fixed-size byte arrays.
///
/// Fixed-size arrays are serialized as their raw bytes without length prefix.
impl<const N: usize> Serialize for [u8; N] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_all(self)
    }
}

impl<const N: usize> Deserialize for [u8; N] {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        src.read_exact(self)
    }
}

#[derive(Default)]
struct UsizeAsU32(usize);

impl Serialize for UsizeAsU32 {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        let Some(val) = self.0.to_u32() else {
            return Err(std::io::Error::other("cannot cast `usize` to `u32`"));
        };

        val.serialize(dest)
    }
}

impl Deserialize for UsizeAsU32 {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let Some(val) = deserialize::<u32>(src)?.to_usize() else {
            return Err(std::io::Error::other("cannot cast `u32` to `usize`"));
        };

        self.0 = val;
        Ok(())
    }
}

/// XDR implementation for variable-length byte vectors.
///
/// Variable-length data is serialized with a 4-byte length prefix,
/// followed by the actual data, and padded to a multiple of 4 bytes.
impl Serialize for [u8] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        UsizeAsU32(self.len()).serialize(dest)?;
        dest.write_all(self)?;

        // write padding
        let pad = 4 - self.len() % 4;
        let zeros: [u8; 4] = [0, 0, 0, 0];
        if pad > 0 {
            dest.write_all(&zeros[..pad])?;
        }
        Ok(())
    }
}

impl Deserialize for Vec<u8> {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let length = deserialize::<UsizeAsU32>(src)?.0;
        self.resize(length, 0);
        src.read_exact(self)?;
        // read padding
        let pad = 4 - length % 4;
        let mut zeros: [u8; 4] = [0, 0, 0, 0];
        src.read_exact(&mut zeros[..pad])?;
        Ok(())
    }
}

/// XDR implementation for vectors of 32-bit unsigned integers.
///
/// Serialized as a 4-byte length prefix followed by that many 4-byte integers.
impl Serialize for [u32] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        UsizeAsU32(self.len()).serialize(dest)?;
        for i in self {
            i.serialize(dest)?;
        }

        Ok(())
    }
}

impl Deserialize for Vec<u32> {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let length = deserialize::<UsizeAsU32>(src)?.0;
        self.resize(length, 0);
        for i in self {
            i.deserialize(src)?;
        }
        Ok(())
    }
}

impl Serialize for str {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        self.as_bytes().serialize(dest)
    }
}

impl Deserialize for String {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        use std::str::from_utf8;

        // Safety: we clear buffer on every step until verification
        unsafe {
            if let err @ Err(_) = self.as_mut_vec().deserialize(src) {
                self.clear();
                return err;
            }

            if from_utf8(self.as_mut_vec()).is_err() {
                self.clear();
                return Err(std::io::Error::other("cannot construct string"));
            }
        };

        Ok(())
    }
}

impl Serialize for char {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        (*self as u32).serialize(dest)
    }
}

impl Deserialize for char {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let u32 = deserialize::<u32>(src)?;
        let Some(char) = std::char::from_u32(u32) else {
            return Err(std::io::Error::other("cannot convert `u32` to `char`"));
        };
        *self = char;
        Ok(())
    }
}

impl Serialize for [char] {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        UsizeAsU32(self.len()).serialize(dest)?;
        for i in self {
            i.serialize(dest)?;
        }
        Ok(())
    }
}

impl Deserialize for Vec<char> {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let length = deserialize::<UsizeAsU32>(src)?.0;
        self.resize(length, Default::default());
        for i in self {
            i.deserialize(src)?;
        }
        Ok(())
    }
}

/// Macro for implementing XDR serialization and deserialization for structs.
///
/// This macro simplifies implementation of the XDR trait for struct types
/// by serializing or deserializing each field in sequence.
#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! SerializeStruct {
    (
        $t:ident,
        $($element:ident),*
    ) => {
        impl Serialize for $t {
            fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
                $(self.$element.serialize(dest)?;)*
                Ok(())
            }
        }
    };
}

#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! DeserializeStruct {
    (
        $t:ident,
        $($element:ident),*
    ) => {
        impl Deserialize for $t {
            fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
                $(self.$element.deserialize(src)?;)*
                Ok(())
            }
        }
    };
}

/// Macro for implementing XDR serialization and deserialization for boolean unions.
///
/// This is specialized for XDR unions where a boolean discriminant selects between
/// two cases: a void (empty) case and a case containing a value of some type.
///
/// # Example
/// ```
/// enum pre_op_attr {
///     Void,
///     attributes(wcc_attr)
/// }
/// DeserializeBoolUnion!(pre_op_attr, attributes, wcc_attr)
/// ```
#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! SerializeBoolUnion {
    (
        $t:ident, $enumcase:ident, $enumtype:ty
    ) => {
        impl Serialize for $t {
            fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
                match self {
                    $t::Void => {
                        false.serialize(dest)?;
                    }
                    $t::$enumcase(v) => {
                        true.serialize(dest)?;
                        v.serialize(dest)?;
                    }
                }
                Ok(())
            }
        }
    };
}

#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! DeserializeBoolUnion {
    (
        $t:ident, $enumcase:ident, $enumtype:ty
    ) => {
        impl Deserialize for $t {
            fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
                if deserialize::<bool>(src)? {
                    *self = $t::$enumcase(deserialize::<$enumtype>(src)?);
                } else {
                    *self = $t::Void;
                }

                Ok(())
            }
        }
    };
}

// Re-export public types for use in other modules
pub use crate::DeserializeBoolUnion;
pub use crate::SerializeBoolUnion;

pub use crate::DeserializeEnum;
pub use crate::SerializeEnum;

pub use crate::DeserializeStruct;
pub use crate::SerializeStruct;
