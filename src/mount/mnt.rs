//! Defines Mount version 3 [`Mnt`] interface (Procedure 1).
//!
//! as defined in RFC 1813 section 5.2.1.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.1>.

#![allow(dead_code)]

use async_trait::async_trait;

use super::{DirPath, Error, FileHandle};

#[derive(Debug)]
pub enum AuthFlavor {
    None,
    Unix,
    Short,
    Des,
    Kerb,
}

pub struct Success {
    pub file_handle: FileHandle,
    pub auth_flavors: Vec<AuthFlavor>,
}

pub type Result = std::result::Result<Success, Error>;

#[async_trait]
pub trait Promise {
    async fn keep(result: Result);
}

#[async_trait]
pub trait Mnt {
    async fn mnt(&self, dirpath: DirPath, promise: impl Promise);
}
