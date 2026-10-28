//! Test helpers that encode NLM procedure arguments in XDR format.
//!
//! Wraps the production [`serializer`](crate::serializer) to build test data.

use std::io::Write;

use byteorder::{BigEndian, WriteBytesExt};

use crate::consts::nfsv3::NFS3_FHSIZE;
use crate::serializer;

pub fn string(s: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    serializer::string(&mut buf, s).unwrap();
    buf
}

pub fn handle(bytes: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    serializer::u32(&mut buf, NFS3_FHSIZE as u32).unwrap();
    buf.write_all(bytes).unwrap();
    serializer::padding(&mut buf, bytes.len()).unwrap();
    buf
}

pub fn opaque(bytes: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    serializer::vector(&mut buf, bytes).unwrap();
    buf
}

pub fn i32_val(v: i32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.write_i32::<BigEndian>(v).unwrap();
    buf
}

pub fn u32_val(v: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    serializer::u32(&mut buf, v).unwrap();
    buf
}

pub fn u64_val(v: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    serializer::u64(&mut buf, v).unwrap();
    buf
}

pub fn bool_val(v: bool) -> Vec<u8> {
    let mut buf = Vec::new();
    serializer::bool(&mut buf, v).unwrap();
    buf
}
