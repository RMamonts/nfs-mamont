//! NFSv3 requests and replies (RFC 1813).

use nfs_mamont::consts::nfsv3::{
    ACCESS, COMMIT, CREATE, FSINFO, GETATTR, LOOKUP, NFS_PROGRAM, NFS_VERSION, NULL, READ, READDIR,
    READDIRPLUS, REMOVE, WRITE,
};

use super::xdr::{opaque_size, put_bool, put_opaque, put_str, put_u32, put_u64, Reader};
use super::{Connection, Error};

/// Result of a client call.
pub type Result<T> = std::result::Result<T, Error>;

/// `stable_how`: the server may keep the data in memory.
pub const UNSTABLE: u32 = 0;
/// `stable_how`: data must reach stable storage.
pub const DATA_SYNC: u32 = 1;
/// `stable_how`: data and metadata must reach stable storage.
pub const FILE_SYNC: u32 = 2;
/// Every `ACCESS` right.
pub const ACCESS_ALL: u32 = 0x3f;
/// `ftype3` of a regular file.
pub const TYPE_REGULAR: u32 = 1;
/// `ftype3` of a directory.
pub const TYPE_DIRECTORY: u32 = 2;
/// Size of `nfstime3`.
const TIME_SIZE: usize = 8;
/// Size of `wcc_attr`: size, mtime, ctime.
const WCC_ATTR_SIZE: usize = 8 + 2 * TIME_SIZE;

/// Argument encoders. Each returns the XDR arguments of one procedure.
pub mod args {
    use super::*;

    /// `GETATTR3args`.
    pub fn getattr(fh: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(opaque_size(fh.len()));
        put_opaque(&mut buf, fh);
        buf
    }

    /// `LOOKUP3args`.
    pub fn lookup(dir: &[u8], name: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        put_opaque(&mut buf, dir);
        put_str(&mut buf, name);
        buf
    }

    /// `ACCESS3args`.
    pub fn access(fh: &[u8], mask: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        put_opaque(&mut buf, fh);
        put_u32(&mut buf, mask);
        buf
    }

    /// `READ3args`; the offset lives at [`offset_position`] of the result.
    pub fn read(fh: &[u8], offset: u64, count: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        put_opaque(&mut buf, fh);
        put_u64(&mut buf, offset);
        put_u32(&mut buf, count);
        buf
    }

    /// `WRITE3args`; the offset lives at [`offset_position`] of the result.
    pub fn write(fh: &[u8], offset: u64, data: &[u8], stable: u32) -> Vec<u8> {
        let mut buf = Vec::with_capacity(opaque_size(fh.len()) + 16 + opaque_size(data.len()));
        put_opaque(&mut buf, fh);
        put_u64(&mut buf, offset);
        put_u32(&mut buf, data.len() as u32);
        put_u32(&mut buf, stable);
        put_opaque(&mut buf, data);
        buf
    }

    /// `CREATE3args` with `UNCHECKED` mode and no attributes.
    pub fn create(dir: &[u8], name: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        put_opaque(&mut buf, dir);
        put_str(&mut buf, name);
        put_u32(&mut buf, 0); // createmode3::UNCHECKED
        sattr3_unset(&mut buf);
        buf
    }

    /// `REMOVE3args`.
    pub fn remove(dir: &[u8], name: &str) -> Vec<u8> {
        lookup(dir, name)
    }

    /// `READDIR3args`.
    pub fn readdir(dir: &[u8], cookie: u64, verifier: &[u8; 8], count: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        put_opaque(&mut buf, dir);
        put_u64(&mut buf, cookie);
        buf.extend_from_slice(verifier);
        put_u32(&mut buf, count);
        buf
    }

    /// `READDIRPLUS3args`.
    pub fn readdirplus(
        dir: &[u8],
        cookie: u64,
        verifier: &[u8; 8],
        dir_count: u32,
        max_count: u32,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        put_opaque(&mut buf, dir);
        put_u64(&mut buf, cookie);
        buf.extend_from_slice(verifier);
        put_u32(&mut buf, dir_count);
        put_u32(&mut buf, max_count);
        buf
    }

    /// `COMMIT3args`; the offset lives at [`offset_position`] of the result.
    pub fn commit(fh: &[u8], offset: u64, count: u32) -> Vec<u8> {
        read(fh, offset, count)
    }

    /// `FSINFO3args`.
    pub fn fsinfo(fh: &[u8]) -> Vec<u8> {
        getattr(fh)
    }

    /// Position of the `offset` field inside READ/WRITE/COMMIT arguments.
    pub fn offset_position(fh_len: usize) -> usize {
        opaque_size(fh_len)
    }

    /// `sattr3` leaving every attribute unchanged.
    fn sattr3_unset(buf: &mut Vec<u8>) {
        put_bool(buf, false); // mode
        put_bool(buf, false); // uid
        put_bool(buf, false); // gid
        put_bool(buf, false); // size
        put_u32(buf, 0); // set_atime::DONT_CHANGE
        put_u32(buf, 0); // set_mtime::DONT_CHANGE
    }
}

/// Subset of `fattr3` the tools look at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attr {
    pub file_type: u32,
    pub mode: u32,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub used: u64,
    pub fsid: u64,
    pub fileid: u64,
}

fn attr(reader: &mut Reader<'_>) -> Result<Attr> {
    let attr = Attr {
        file_type: reader.u32()?,
        mode: reader.u32()?,
        nlink: reader.u32()?,
        uid: reader.u32()?,
        gid: reader.u32()?,
        size: reader.u64()?,
        used: reader.u64()?,
        fsid: {
            reader.skip(8)?; // rdev
            reader.u64()?
        },
        fileid: reader.u64()?,
    };
    reader.skip(3 * TIME_SIZE)?;

    Ok(attr)
}

fn post_op_attr(reader: &mut Reader<'_>) -> Result<Option<Attr>> {
    if reader.bool()? {
        attr(reader).map(Some)
    } else {
        Ok(None)
    }
}

fn wcc_data(reader: &mut Reader<'_>) -> Result<()> {
    if reader.bool()? {
        reader.skip(WCC_ATTR_SIZE)?;
    }
    post_op_attr(reader).map(|_| ())
}

fn verifier(reader: &mut Reader<'_>) -> Result<[u8; 8]> {
    let bytes = reader.bytes(8)?;
    let mut verifier = [0u8; 8];
    verifier.copy_from_slice(bytes);

    Ok(verifier)
}

/// Checks the `nfsstat3` at the start of `body` and returns a reader positioned after it.
pub fn status(body: &[u8]) -> Result<Reader<'_>> {
    let mut reader = Reader::new(body);
    match reader.u32()? {
        0 => Ok(reader),
        code => Err(Error::Status(code)),
    }
}

/// `GETATTR3resok`.
pub fn parse_getattr(body: &[u8]) -> Result<Attr> {
    attr(&mut status(body)?)
}

/// `LOOKUP3resok`.
#[derive(Clone, Debug)]
pub struct Lookup {
    pub handle: Vec<u8>,
    pub attr: Option<Attr>,
}

/// Decodes `LOOKUP3resok`.
pub fn parse_lookup(body: &[u8]) -> Result<Lookup> {
    let mut reader = status(body)?;
    let handle = reader.opaque()?.to_vec();
    let attr = post_op_attr(&mut reader)?;

    Ok(Lookup { handle, attr })
}

/// `ACCESS3resok`: the granted rights.
pub fn parse_access(body: &[u8]) -> Result<u32> {
    let mut reader = status(body)?;
    post_op_attr(&mut reader)?;
    reader.u32()
}

/// `READ3resok`, borrowing the data from the record.
#[derive(Debug)]
pub struct Read<'a> {
    pub attr: Option<Attr>,
    pub count: u32,
    pub eof: bool,
    pub data: &'a [u8],
}

/// Decodes `READ3resok`.
pub fn parse_read(body: &[u8]) -> Result<Read<'_>> {
    let mut reader = status(body)?;
    let attr = post_op_attr(&mut reader)?;
    let count = reader.u32()?;
    let eof = reader.bool()?;
    let data = reader.opaque()?;
    if data.len() != count as usize {
        return Err(Error::Malformed("READ data length does not match count"));
    }

    Ok(Read { attr, count, eof, data })
}

/// `WRITE3resok`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Write {
    pub count: u32,
    pub committed: u32,
    pub verifier: [u8; 8],
}

/// Decodes `WRITE3resok`.
pub fn parse_write(body: &[u8]) -> Result<Write> {
    let mut reader = status(body)?;
    wcc_data(&mut reader)?;

    Ok(Write { count: reader.u32()?, committed: reader.u32()?, verifier: verifier(&mut reader)? })
}

/// One entry of a `READDIR`/`READDIRPLUS` reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub fileid: u64,
    pub name: String,
    pub cookie: u64,
    /// Attributes, `READDIRPLUS` only.
    pub attr: Option<Attr>,
    /// Handle, `READDIRPLUS` only.
    pub handle: Option<Vec<u8>>,
}

/// `READDIR3resok` / `READDIRPLUS3resok`.
#[derive(Clone, Debug)]
pub struct ReadDir {
    pub entries: Vec<DirEntry>,
    pub eof: bool,
    pub verifier: [u8; 8],
}

fn parse_listing(body: &[u8], plus: bool) -> Result<ReadDir> {
    let mut reader = status(body)?;
    post_op_attr(&mut reader)?;
    let verifier = verifier(&mut reader)?;

    let mut entries = Vec::new();
    while reader.bool()? {
        let fileid = reader.u64()?;
        let name = String::from_utf8(reader.opaque()?.to_vec())
            .map_err(|_| Error::Malformed("entry name is not UTF-8"))?;
        let cookie = reader.u64()?;
        let (attr, handle) = if plus {
            let attr = post_op_attr(&mut reader)?;
            let handle = if reader.bool()? { Some(reader.opaque()?.to_vec()) } else { None };
            (attr, handle)
        } else {
            (None, None)
        };
        entries.push(DirEntry { fileid, name, cookie, attr, handle });
    }
    let eof = reader.bool()?;

    Ok(ReadDir { entries, eof, verifier })
}

/// Decodes `READDIR3resok`.
pub fn parse_readdir(body: &[u8]) -> Result<ReadDir> {
    parse_listing(body, false)
}

/// Decodes `READDIRPLUS3resok`.
pub fn parse_readdirplus(body: &[u8]) -> Result<ReadDir> {
    parse_listing(body, true)
}

/// Decodes `COMMIT3resok`: the write verifier.
pub fn parse_commit(body: &[u8]) -> Result<[u8; 8]> {
    let mut reader = status(body)?;
    wcc_data(&mut reader)?;
    verifier(&mut reader)
}

/// Decodes `CREATE3resok`: the handle of the new file, if the server returned one.
pub fn parse_create(body: &[u8]) -> Result<Option<Vec<u8>>> {
    let mut reader = status(body)?;
    if reader.bool()? {
        Ok(Some(reader.opaque()?.to_vec()))
    } else {
        Ok(None)
    }
}

/// Subset of `FSINFO3resok`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FsInfo {
    pub rtmax: u32,
    pub rtpref: u32,
    pub wtmax: u32,
    pub wtpref: u32,
    pub dtpref: u32,
    pub maxfilesize: u64,
}

/// Decodes `FSINFO3resok`.
pub fn parse_fsinfo(body: &[u8]) -> Result<FsInfo> {
    let mut reader = status(body)?;
    post_op_attr(&mut reader)?;
    let rtmax = reader.u32()?;
    let rtpref = reader.u32()?;
    reader.u32()?; // rtmult
    let wtmax = reader.u32()?;
    let wtpref = reader.u32()?;
    reader.u32()?; // wtmult
    let dtpref = reader.u32()?;
    let maxfilesize = reader.u64()?;

    Ok(FsInfo { rtmax, rtpref, wtmax, wtpref, dtpref, maxfilesize })
}

/// Owned `READ` result.
#[derive(Clone, Debug)]
pub struct ReadReply {
    pub count: u32,
    pub eof: bool,
    pub data: Vec<u8>,
}

async fn call<'a>(conn: &'a mut Connection, procedure: u32, args: &[u8]) -> Result<&'a [u8]> {
    conn.call(NFS_PROGRAM, NFS_VERSION, procedure, args).await
}

/// `NULL`.
pub async fn null(conn: &mut Connection) -> Result<()> {
    call(conn, NULL, &[]).await.map(|_| ())
}

/// `GETATTR`.
pub async fn getattr(conn: &mut Connection, fh: &[u8]) -> Result<Attr> {
    parse_getattr(call(conn, GETATTR, &args::getattr(fh)).await?)
}

/// `LOOKUP`.
pub async fn lookup(conn: &mut Connection, dir: &[u8], name: &str) -> Result<Lookup> {
    parse_lookup(call(conn, LOOKUP, &args::lookup(dir, name)).await?)
}

/// `ACCESS`.
pub async fn access(conn: &mut Connection, fh: &[u8], mask: u32) -> Result<u32> {
    parse_access(call(conn, ACCESS, &args::access(fh, mask)).await?)
}

/// `READ`.
pub async fn read(conn: &mut Connection, fh: &[u8], offset: u64, count: u32) -> Result<ReadReply> {
    let read = parse_read(call(conn, READ, &args::read(fh, offset, count)).await?)?;

    Ok(ReadReply { count: read.count, eof: read.eof, data: read.data.to_vec() })
}

/// `WRITE`.
pub async fn write(
    conn: &mut Connection,
    fh: &[u8],
    offset: u64,
    data: &[u8],
    stable: u32,
) -> Result<Write> {
    parse_write(call(conn, WRITE, &args::write(fh, offset, data, stable)).await?)
}

/// `CREATE` (unchecked).
pub async fn create(conn: &mut Connection, dir: &[u8], name: &str) -> Result<Option<Vec<u8>>> {
    parse_create(call(conn, CREATE, &args::create(dir, name)).await?)
}

/// `REMOVE`.
pub async fn remove(conn: &mut Connection, dir: &[u8], name: &str) -> Result<()> {
    status(call(conn, REMOVE, &args::remove(dir, name)).await?).map(|_| ())
}

/// `READDIR`.
pub async fn readdir(
    conn: &mut Connection,
    dir: &[u8],
    cookie: u64,
    verifier: &[u8; 8],
    count: u32,
) -> Result<ReadDir> {
    parse_readdir(call(conn, READDIR, &args::readdir(dir, cookie, verifier, count)).await?)
}

/// `READDIRPLUS`.
pub async fn readdirplus(
    conn: &mut Connection,
    dir: &[u8],
    cookie: u64,
    verifier: &[u8; 8],
    dir_count: u32,
    max_count: u32,
) -> Result<ReadDir> {
    let args = args::readdirplus(dir, cookie, verifier, dir_count, max_count);
    parse_readdirplus(call(conn, READDIRPLUS, &args).await?)
}

/// `COMMIT`.
pub async fn commit(conn: &mut Connection, fh: &[u8], offset: u64, count: u32) -> Result<[u8; 8]> {
    parse_commit(call(conn, COMMIT, &args::commit(fh, offset, count)).await?)
}

/// `FSINFO`.
pub async fn fsinfo(conn: &mut Connection, fh: &[u8]) -> Result<FsInfo> {
    parse_fsinfo(call(conn, FSINFO, &args::fsinfo(fh)).await?)
}
