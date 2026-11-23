//! XDR encoding helpers (RFC 4506): big-endian words, 4-byte alignment.

use nfs_mamont::consts::xdr::ALIGNMENT;

use super::Error;

/// Rounds `len` up to the XDR alignment.
pub fn padded(len: usize) -> usize {
    len.div_ceil(ALIGNMENT) * ALIGNMENT
}

/// Encoded size of a variable-length opaque of `len` bytes.
pub fn opaque_size(len: usize) -> usize {
    4 + padded(len)
}

/// Appends an `unsigned int`.
pub fn put_u32(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_be_bytes());
}

/// Appends an `unsigned hyper`.
pub fn put_u64(buf: &mut Vec<u8>, value: u64) {
    buf.extend_from_slice(&value.to_be_bytes());
}

/// Appends a `bool`.
pub fn put_bool(buf: &mut Vec<u8>, value: bool) {
    put_u32(buf, u32::from(value));
}

/// Appends a variable-length opaque: length, bytes, padding.
pub fn put_opaque(buf: &mut Vec<u8>, bytes: &[u8]) {
    put_u32(buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
    buf.resize(buf.len() + padded(bytes.len()) - bytes.len(), 0);
}

/// Appends a `string`.
pub fn put_str(buf: &mut Vec<u8>, value: &str) {
    put_opaque(buf, value.as_bytes());
}

/// Cursor decoding XDR items from a byte slice.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Starts reading at the beginning of `buf`.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Returns the number of bytes consumed so far.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Returns the bytes not consumed yet.
    pub fn remaining(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }

    /// Takes `n` raw bytes without padding.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Malformed("length overflow"))?;
        if end > self.buf.len() {
            return Err(Error::Malformed("reply shorter than expected"));
        }
        let bytes = &self.buf[self.pos..end];
        self.pos = end;

        Ok(bytes)
    }

    /// Skips `n` raw bytes.
    pub fn skip(&mut self, n: usize) -> Result<(), Error> {
        self.bytes(n).map(|_| ())
    }

    /// Reads an `unsigned int`.
    pub fn u32(&mut self) -> Result<u32, Error> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    /// Reads an `unsigned hyper`.
    pub fn u64(&mut self) -> Result<u64, Error> {
        let high = u64::from(self.u32()?);
        let low = u64::from(self.u32()?);

        Ok((high << 32) | low)
    }

    /// Reads a `bool`.
    pub fn bool(&mut self) -> Result<bool, Error> {
        match self.u32()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Malformed("invalid boolean")),
        }
    }

    /// Reads a variable-length opaque and skips its padding.
    pub fn opaque(&mut self) -> Result<&'a [u8], Error> {
        let len = self.u32()? as usize;
        let bytes = self.bytes(len)?;
        self.skip(padded(len) - len)?;

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::{opaque_size, padded, put_opaque, put_u64, Reader};

    #[test]
    fn opaque_is_padded_to_four_bytes() {
        assert_eq!(padded(0), 0);
        assert_eq!(padded(9), 12);
        assert_eq!(opaque_size(9), 16);

        let mut buf = Vec::new();
        put_opaque(&mut buf, &[1, 2, 3, 4, 5]);
        assert_eq!(buf, [0, 0, 0, 5, 1, 2, 3, 4, 5, 0, 0, 0]);
    }

    #[test]
    fn reader_round_trips() {
        let mut buf = Vec::new();
        put_u64(&mut buf, 0x0102_0304_0506_0708);
        put_opaque(&mut buf, b"abc");

        let mut reader = Reader::new(&buf);
        assert_eq!(reader.u64().unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(reader.opaque().unwrap(), b"abc");
        assert_eq!(reader.position(), buf.len());
        assert!(reader.u32().is_err());
    }
}
