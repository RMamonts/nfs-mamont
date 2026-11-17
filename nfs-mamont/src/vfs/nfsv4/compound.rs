//! Defines the NFSv4 COMPOUND operation types, RFC 7530 §15.2.

use crate::Buffer;
use std::io;

/// Arguments of operation for `compound`
pub enum OperationArgs<B: Buffer> {
    Access(super::access::Args),
    Close(super::close::Args),
    Commit(super::commit::Args),
    Create(super::create::Args),
    DelegPurge(super::delegpurge::Args),
    DelegReturn(super::delegreturn::Args),
    GetAttr(super::getattr::Args),
    GetFh(super::getfh::Args),
    Link(super::link::Args),
    Lock(super::lock::Args),
    LockT(super::lockt::Args),
    LockU(super::locku::Args),
    Lookup(super::lookup::Args),
    LookupP(super::lookupp::Args),
    NVerify(super::nverify::Args),
    Open(super::open::Args),
    OpenAttr(super::openattr::Args),
    OpenConfirm(super::open_confirm::Args),
    OpenDowngrade(super::open_downgrade::Args),
    PutFh(super::putfh::Args),
    PutPubFh(super::putpubfh::Args),
    PutRootFh(super::putrootfh::Args),
    Read(super::read::Args),
    ReadDir(super::readdir::Args),
    ReadLink(super::readlink::Args),
    Remove(super::remove::Args),
    Rename(super::rename::Args),
    Renew(super::renew::Args),
    RestoreFh(super::restorefh::Args),
    SaveFh(super::savefh::Args),
    Secinfo(super::secinfo::Args),
    SetAttr(super::setattr::Args),
    SetClientId(super::setclientid::Args),
    SetClientIdConfirm(super::setclientid_confirm::Args),
    Verify(super::verify::Args),
    Write(super::write::Args<B>),
    ReleaseLockOwner(super::release_lockowner::Args),
    Illegal,
}

/// Results of operation for `compound`
pub enum OperationRes<B: Buffer> {
    Access(core::result::Result<super::access::Success, super::access::Fail>),
    Close(core::result::Result<super::close::Success, super::close::Fail>),
    Commit(core::result::Result<super::commit::Success, super::commit::Fail>),
    Create(core::result::Result<super::create::Success, super::create::Fail>),
    DelegPurge(core::result::Result<super::delegpurge::Success, super::delegpurge::Fail>),
    DelegReturn(core::result::Result<super::delegreturn::Success, super::delegreturn::Fail>),
    GetAttr(core::result::Result<super::getattr::Success, super::getattr::Fail>),
    GetFh(core::result::Result<super::getfh::Success, super::getfh::Fail>),
    Link(core::result::Result<super::link::Success, super::link::Fail>),
    Lock(core::result::Result<super::lock::Success, super::lock::Fail>),
    LockT(core::result::Result<super::lockt::Success, super::lockt::Fail>),
    LockU(core::result::Result<super::locku::Success, super::locku::Fail>),
    Lookup(core::result::Result<super::lookup::Success, super::lookup::Fail>),
    LookupP(core::result::Result<super::lookupp::Success, super::lookupp::Fail>),
    NVerify(core::result::Result<super::nverify::Success, super::nverify::Fail>),
    Open(core::result::Result<super::open::Success, super::open::Fail>),
    OpenAttr(core::result::Result<super::openattr::Success, super::openattr::Fail>),
    OpenConfirm(core::result::Result<super::open_confirm::Success, super::open_confirm::Fail>),
    OpenDowngrade(
        core::result::Result<super::open_downgrade::Success, super::open_downgrade::Fail>,
    ),
    PutFh(core::result::Result<super::putfh::Success, super::putfh::Fail>),
    PutPubFh(core::result::Result<super::putpubfh::Success, super::putpubfh::Fail>),
    PutRootFh(core::result::Result<super::putrootfh::Success, super::putrootfh::Fail>),
    Read(core::result::Result<super::read::Success<B>, super::read::Fail>),
    ReadDir(core::result::Result<super::readdir::Success, super::readdir::Fail>),
    ReadLink(core::result::Result<super::readlink::Success, super::readlink::Fail>),
    Remove(core::result::Result<super::remove::Success, super::remove::Fail>),
    Rename(core::result::Result<super::rename::Success, super::rename::Fail>),
    Renew(core::result::Result<super::renew::Success, super::renew::Fail>),
    RestoreFh(core::result::Result<super::restorefh::Success, super::restorefh::Fail>),
    SaveFh(core::result::Result<super::savefh::Success, super::savefh::Fail>),
    Secinfo(core::result::Result<super::secinfo::Success, super::secinfo::Fail>),
    SetAttr(core::result::Result<super::setattr::Success, super::setattr::Fail>),
    SetClientId(core::result::Result<super::setclientid::Success, super::setclientid::Fail>),
    SetClientIdConfirm(
        core::result::Result<super::setclientid_confirm::Success, super::setclientid_confirm::Fail>,
    ),
    Verify(core::result::Result<super::verify::Success, super::verify::Fail>),
    Write(core::result::Result<super::write::Success, super::write::Fail>),
    ReleaseLockOwner(
        core::result::Result<super::release_lockowner::Success, super::release_lockowner::Fail>,
    ),
    Illegal,
}

pub struct Tag(String);

impl Tag {
    const MAX_LEN: usize = 255;

    pub fn new(inner: String) -> io::Result<Self> {
        if inner.len() > Self::MAX_LEN {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "tag is too big"));
        }
        Ok(Self(inner))
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// COMPOUND arguments (`COMPOUND4args`).
pub struct Args<B: Buffer> {
    /// Client-supplied tag identifying the request.
    ///
    /// The server SHOULD return the same value in the response.
    pub tag: Tag,
    /// Minor version of the NFSv4 protocol requested.
    pub minorversion: u32,
    /// Array of operations composing the request (`nfs_argop4`).
    pub argarray: Vec<OperationArgs<B>>,
}

/// Success result (`COMPOUND4res`).
pub struct Result<B: Buffer> {
    /// Overall status of the COMPOUND procedure.
    pub status: super::Error,
    /// Tag echoed from the request.
    pub tag: Tag,
    /// Array of per-operation results (`nfs_resop4`).
    pub resarray: Vec<OperationArgs<B>>,
}
