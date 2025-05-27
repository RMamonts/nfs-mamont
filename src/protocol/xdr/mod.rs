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
pub trait XDR {
    /// Serializes the implementing type to the provided writer.
    ///
    /// # Arguments
    /// * `dest` - A mutable reference to any type that implements Write.
    ///
    /// # Returns
    /// * `std::io::Result<()>` - Ok(()) on success, or an error if serialization fails.
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()>;

    /// Deserializes data from the provided reader into the implementing type.
    ///
    /// # Arguments
    /// * `src` - A mutable reference to any type that implements Read.
    ///
    /// # Returns
    /// * `std::io::Result<()>` - Ok(()) on success, or an error if deserialization fails.
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()>;
}

/// Macro for implementing XDR serialization and deserialization for enumerations.
///
/// This macro simplifies implementation of the XDR trait for enum types
/// by providing standard serialization and deserialization as 32-bit integers.
#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! XDREnumSerde {
    ($t:ident) => {
        impl XDR for $t {
            fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
                dest.write_u32::<$crate::xdr::XDREndian>(*self as u32)
            }

            fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
                let r: u32 = src.read_u32::<$crate::xdr::XDREndian>()?;
                if let Some(p) = FromPrimitive::from_u32(r) {
                    *self = p;
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Invalid value for {}", stringify!($t)),
                    ));
                }
                Ok(())
            }
        }
    };
}

/// XDR implementation for boolean values.
///
/// Booleans are serialized as 4-byte big endian integers
/// where 0 represents false and any non-zero value represents true.
impl XDR for bool {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        let val: u32 = *self as u32;
        dest.write_u32::<XDREndian>(val)
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let val: u32 = src.read_u32::<XDREndian>()?;
        *self = val > 0;
        Ok(())
    }
}

/// XDR implementation for 32-bit signed integers.
///
/// Integers are serialized as 4-byte big endian values.
impl XDR for i32 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_i32::<XDREndian>(*self)
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_i32::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for 64-bit signed integers.
///
/// 64-bit integers are serialized as 8-byte big endian values.
impl XDR for i64 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_i64::<XDREndian>(*self)
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_i64::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for 32-bit unsigned integers.
///
/// Unsigned 32-bit integers are serialized as 4-byte big endian values.
impl XDR for u32 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_u32::<XDREndian>(*self)
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_u32::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for 64-bit unsigned integers.
///
/// Unsigned 64-bit integers are serialized as 8-byte big endian values.
impl XDR for u64 {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_u64::<XDREndian>(*self)
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        *self = src.read_u64::<XDREndian>()?;
        Ok(())
    }
}

/// XDR implementation for fixed-size byte arrays.
///
/// Fixed-size arrays are serialized as their raw bytes without length prefix.
impl<const N: usize> XDR for [u8; N] {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        dest.write_all(self)
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        src.read_exact(self)
    }
}

/// XDR implementation for variable-length byte vectors.
///
/// Variable-length data is serialized with a 4-byte length prefix,
/// followed by the actual data, and padded to a multiple of 4 bytes.
impl XDR for Vec<u8> {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        assert!(self.len() < u32::MAX as usize);
        let length = self.len() as u32;
        length.serialize(dest)?;
        dest.write_all(self)?;
        // write padding
        let pad = ((4 - length % 4) % 4) as usize;
        let zeros: [u8; 4] = [0, 0, 0, 0];
        if pad > 0 {
            dest.write_all(&zeros[..pad])?;
        }
        Ok(())
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let mut length: u32 = 0;
        length.deserialize(src)?;
        self.resize(length as usize, 0);
        src.read_exact(self)?;
        // read padding
        let pad = ((4 - length % 4) % 4) as usize;
        let mut zeros: [u8; 4] = [0, 0, 0, 0];
        src.read_exact(&mut zeros[..pad])?;
        Ok(())
    }
}

/// XDR implementation for vectors of 32-bit unsigned integers.
///
/// Serialized as a 4-byte length prefix followed by that many 4-byte integers.
impl XDR for Vec<u32> {
    fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
        assert!(self.len() < u32::MAX as usize);
        let length = self.len() as u32;
        length.serialize(dest)?;
        for i in self {
            i.serialize(dest)?;
        }
        Ok(())
    }

    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        let mut length: u32 = 0;
        length.deserialize(src)?;
        self.resize(length as usize, 0);
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
macro_rules! XDRStruct {
    (
        $t:ident,
        $($element:ident),*
    ) => {
        impl XDR for $t {
            fn serialize<R: Write>(&self, dest: &mut R) -> std::io::Result<()> {
                $(self.$element.serialize(dest)?;)*
                Ok(())
            }

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
/// XDRBoolUnion!(pre_op_attr, attributes, wcc_attr)
/// ```
#[allow(non_camel_case_types)]
#[macro_export]
macro_rules! XDRBoolUnion {
    (
        $t:ident, $enumcase:ident, $enumtype:ty
    ) => {
        impl XDR for $t {
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

            fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
                let mut c: bool = false;
                c.deserialize(src)?;
                if c == false {
                    *self = $t::Void;
                } else {
                    let mut r = <$enumtype>::default();
                    r.deserialize(src)?;
                    *self = $t::$enumcase(r);
                }
                Ok(())
            }
        }
    };
}

// Re-export public types for use in other modules
pub use crate::XDRBoolUnion;
pub use crate::XDREnumSerde;
pub use crate::XDRStruct;
