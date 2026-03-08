//! RPC message parser for NFS and MOUNT protocols.
//!
//! This module provides the [`RpcParser`] struct, which parses XDR-encoded RPC messages
//! according to RFC 5531 (RPC) and RFC 1813 (NFSv3). It handles:
//!
//! - RPC message framing and headers
//! - Authentication (`AUTH_NONE` and `AUTH_SYS` credentials)
//! - NFSv3 procedure parsing (all 22 procedures)
//! - MOUNT protocol procedure parsing
//! - Error handling and message discarding on protocol errors
//!
//! The parser uses a [`CountBuffer`] to efficiently read from async streams while
//! supporting retry logic for parsing operations that may need additional data.

use std::cmp::min;
use std::io::{self, ErrorKind};
use std::num::NonZeroUsize;

use tokio::io::AsyncRead;

use crate::allocator::{Allocator, Slice};
use crate::mount::{MOUNT_PROGRAM, MOUNT_VERSION};
use crate::nfsv3::{
    ACCESS, COMMIT, CREATE, FSINFO, FSSTAT, GETATTR, LINK, LOOKUP, MKDIR, MKNOD, NFS_PROGRAM,
    NFS_VERSION, NULL, PATHCONF, READ, READDIR, READDIRPLUS, READLINK, REMOVE, RENAME, RMDIR,
    SETATTR, SYMLINK, WRITE,
};
use crate::parser::mount::mnt::mount;
use crate::parser::mount::umnt::unmount;
use crate::parser::nfsv3::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};
use crate::parser::primitive::{u32, u32_as_usize, ALIGNMENT};
use crate::parser::read_buffer::CountBuffer;
use crate::parser::rpc::{auth, RpcMessage};
use crate::parser::{proc_nested_errors, Arguments, Error, Result};
use crate::rpc::{
    AuthFlavor, AuthStat, ParsedRpcCall, RpcBody, RpcCallHeader, VersionMismatch, RPC_VERSION,
};
use crate::vfs;

const RMS_HEADER_SIZE: usize = size_of::<u32>();

/// Minimum buffer size, that could hold complete RPC message
/// with NFSv3 or Mount protocol arguments, except for NFSv3 `WRITE` procedure -
/// this size is enough to hold only arguments without opaque data ([`Slice`] in [`vfs::write::Args`])
const DEFAULT_SIZE: usize = 128 * 1024;

/// Parser for RPC messages over async streams.
///
/// `RpcParser` parses complete RPC messages from an async stream, handling
/// message framing, RPC headers, authentication, and procedure-specific arguments.
/// It supports both NFSv3 and MOUNT protocols.
///
/// The parser uses an allocator for operations that require dynamic memory
/// allocation (such as WRITE operations with variable-length data).
///
/// # Type Parameters
///
/// * `A` - An allocator type that implements [`Allocator`] for dynamic memory allocation
/// * `S` - An async stream type that implements [`AsyncRead`] and [`Unpin`]
///
/// # Example
///
/// ```ignore
/// use std::io;
///
/// use nfs_mamont::allocator::Allocator;
/// use nfs_mamont::parser::parser_struct::RpcParser;
/// use tokio::io::AsyncRead;
///
/// # async fn example<A: Allocator, S: AsyncRead + Unpin>(socket: S, alloc: A) -> io::Result<()> {
/// let mut parser = RpcParser::new(socket, alloc);
/// let args = parser.parse_message().await?;
/// # let _ = args;
/// # Ok(())
/// # }
/// ```
pub struct RpcParser<A: Allocator, S: AsyncRead + Unpin> {
    allocator: A,
    buffer: CountBuffer<S>,
    last: bool,
    current_frame_size: usize,
}

pub struct ParseFailure {
    pub xid: Option<u32>,
    pub error: Error,
}

#[allow(dead_code)]
impl<A: Allocator, S: AsyncRead + Unpin> RpcParser<A, S> {
    /// Creates a new `RpcParser` with the default buffer size.
    ///
    /// # Arguments
    ///
    /// * `socket` - The async stream to read RPC messages from
    /// * `allocator` - The allocator to use for dynamic memory allocation
    ///
    /// # Returns
    ///
    /// A new `RpcParser` instance ready to parse messages.
    pub fn new(socket: S, allocator: A) -> Self {
        Self {
            allocator,
            buffer: CountBuffer::new(DEFAULT_SIZE, socket),
            last: false,
            current_frame_size: 0,
        }
    }

    /// Creates a new `RpcParser` with the specified buffer size.
    ///
    /// # Arguments
    ///
    /// * `socket` - The async stream to read RPC messages from
    /// * `allocator` - The allocator to use for dynamic memory allocation
    /// * `size` - The size of the internal read buffer (used for each of the two buffers)
    ///
    /// # Returns
    ///
    /// A new `RpcParser` instance ready to parse messages.
    pub fn with_capacity(socket: S, allocator: A, size: usize) -> Self {
        Self {
            allocator,
            buffer: CountBuffer::new(size, socket),
            last: false,
            current_frame_size: 0,
        }
    }

    /// Reads and parses the RPC message header.
    ///
    /// The message header contains:
    /// - A 32-bit word with the most significant bit indicating if this is the last fragment
    /// - The remaining 31 bits containing the fragment size
    /// - The transaction ID (XID)
    ///
    /// Currently, fragmented messages are not supported and will return an error.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the header was successfully parsed, or an error if:
    /// - The message is fragmented (not supported)
    /// - An I/O error occurs
    async fn read_message_header(&mut self) -> Result<u32> {
        let header = self.buffer.parse_with_retry(u32).await?;
        self.last = header & 0x8000_0000 != 0;
        self.current_frame_size = (header & 0x7FFF_FFFF) as usize;

        if self.current_frame_size < std::mem::size_of::<u32>() {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Frame size must include XID",
            )));
        }
        if self.current_frame_size > DEFAULT_SIZE {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Frame exceeds maximum supported length",
            )));
        }

        // this is temporal check, apparently this will go to separate object Validator
        if !self.last {
            return Err(Error::IO(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            )));
        }
        let xid = self.buffer.parse_with_retry(u32).await?;
        Ok(xid)
    }

    /// Parses the RPC call header.
    ///
    /// The RPC header contains:
    /// - Message type (must be CALL, not REPLY)
    /// - RPC version (must match the expected version)
    /// - Program number (NFS or MOUNT)
    /// - Program version
    /// - Procedure number
    /// - Authentication information
    ///
    /// # Returns
    ///
    /// Returns a [`RpcMessage`] containing the program, version, and procedure,
    /// or an error if:
    /// - The message type is REPLY (not expected for incoming calls)
    /// - The RPC version doesn't match
    /// - Authentication fails
    /// - An I/O error occurs
    async fn parse_rpc_header(&mut self, xid: u32) -> Result<RpcMessage> {
        let msg_type = self.buffer.parse_with_retry(u32).await?;
        if msg_type != RpcBody::Call as u32 {
            return Err(Error::MessageTypeMismatch);
        }

        let rpc_version = self.buffer.parse_with_retry(u32).await?;
        if rpc_version != RPC_VERSION {
            return Err(Error::RpcVersionMismatch(VersionMismatch {
                low: RPC_VERSION,
                high: RPC_VERSION,
            }));
        }

        let program = self.buffer.parse_with_retry(u32).await?;
        let version = self.buffer.parse_with_retry(u32).await?;
        let procedure = self.buffer.parse_with_retry(u32).await?;

        let auth = self.parse_authentication().await?;
        self.buffer.parse_with_retry(crate::parser::rpc::auth).await?;

        Ok(RpcMessage { xid, program, procedure, version, auth_flavor: auth.flavor })
    }

    /// Parses and validates RPC credentials.
    ///
    /// Currently `AUTH_NONE` and `AUTH_SYS` are accepted.
    ///
    /// # Returns
    ///
    /// Returns parsed opaque authentication metadata if authentication succeeds,
    /// or an error if authentication fails or an I/O error occurs.
    async fn parse_authentication(&mut self) -> Result<crate::rpc::OpaqueAuth> {
        let auth = self.buffer.parse_with_retry(auth).await?;

        match auth.flavor {
            AuthFlavor::None | AuthFlavor::Sys => Ok(auth),
            _ => Err(Error::AuthError(AuthStat::BadCred)),
        }
    }

    /// Parses procedure-specific arguments based on the RPC message header.
    ///
    /// This method dispatches to the appropriate parser based on the program
    /// (NFS or MOUNT) and procedure number. It supports all NFSv3 procedures
    /// (0-21) and MOUNT procedures (0-5).
    ///
    /// For the WRITE procedure (NFS procedure 7), this uses a special adapter
    /// that allocates memory for the write data.
    ///
    /// # Arguments
    ///
    /// * `head` - The parsed RPC message header containing program, version, and procedure
    ///
    /// # Returns
    ///
    /// Returns a boxed [`Arguments`] enum variant containing the parsed procedure arguments,
    /// or an error if:
    /// - The program is not recognized (NFS or MOUNT)
    /// - The program version doesn't match
    /// - The procedure number is invalid
    /// - Parsing the procedure arguments fails
    async fn parse_proc(&mut self, head: RpcMessage) -> Result<Box<Arguments>> {
        match head.program {
            NFS_PROGRAM => self.parse_nfs_proc(head).await,
            MOUNT_PROGRAM => self.parse_mount_proc(head).await,
            _ => Err(Error::ProgramMismatch),
        }
    }

    async fn parse_nfs_proc(&mut self, head: RpcMessage) -> Result<Box<Arguments>> {
        if head.version != NFS_VERSION {
            return Err(Error::ProgramVersionMismatch(VersionMismatch {
                low: NFS_VERSION,
                high: NFS_VERSION,
            }));
        }

        macro_rules! parse_arg {
            ($variant:ident, $parser:path) => {
                Arguments::$variant(self.buffer.parse_with_retry($parser).await?)
            };
        }

        Ok(Box::new(match head.procedure {
            NULL => Arguments::Null,
            GETATTR => parse_arg!(GetAttr, get_attr::args),
            SETATTR => parse_arg!(SetAttr, set_attr::args),
            LOOKUP => parse_arg!(LookUp, lookup::args),
            ACCESS => parse_arg!(Access, access::args),
            READLINK => parse_arg!(ReadLink, read_link::args),
            READ => parse_arg!(Read, read::args),
            WRITE => {
                Arguments::Write(adapter_for_write(&mut self.allocator, &mut self.buffer).await?)
            }
            CREATE => parse_arg!(Create, create::args),
            MKDIR => parse_arg!(MkDir, mk_dir::args),
            SYMLINK => parse_arg!(SymLink, symlink::args),
            MKNOD => parse_arg!(MkNod, mk_node::args),
            REMOVE => parse_arg!(Remove, remove::args),
            RMDIR => parse_arg!(RmDir, rm_dir::args),
            RENAME => parse_arg!(Rename, rename::args),
            LINK => parse_arg!(Link, link::args),
            READDIR => parse_arg!(ReadDir, read_dir::args),
            READDIRPLUS => parse_arg!(ReadDirPlus, read_dir_plus::args),
            FSSTAT => parse_arg!(FsStat, fs_stat::args),
            FSINFO => parse_arg!(FsInfo, fs_info::args),
            PATHCONF => parse_arg!(PathConf, path_conf::args),
            COMMIT => parse_arg!(Commit, commit::args),
            _ => return Err(Error::ProcedureMismatch),
        }))
    }

    async fn parse_mount_proc(&mut self, head: RpcMessage) -> Result<Box<Arguments>> {
        if head.version != MOUNT_VERSION {
            return Err(Error::ProgramVersionMismatch(VersionMismatch {
                low: MOUNT_VERSION,
                high: MOUNT_VERSION,
            }));
        }

        Ok(Box::new(match head.procedure {
            0 => Arguments::Null,
            1 => Arguments::Mount(self.buffer.parse_with_retry(mount).await?),
            2 => Arguments::Dump,
            3 => Arguments::Unmount(self.buffer.parse_with_retry(unmount).await?),
            4 => Arguments::UnmountAll,
            5 => Arguments::Export,
            _ => return Err(Error::ProcedureMismatch),
        }))
    }

    pub async fn parse_request_full(&mut self) -> std::result::Result<ParsedRpcCall, ParseFailure> {
        let xid = match self.read_message_header().await {
            Ok(xid) => xid,
            Err(error) => return Err(ParseFailure { xid: None, error }),
        };
        let rpc_header = match self.parse_rpc_header(xid).await {
            Ok(arg) => arg,
            Err(err) => {
                return Err(ParseFailure { xid: Some(xid), error: self.match_errors(err).await });
            }
        };
        let proc = match self.parse_proc(rpc_header).await {
            Ok(arg) => arg,
            Err(err) => {
                return Err(ParseFailure { xid: Some(xid), error: self.match_errors(err).await });
            }
        };

        if let Err(error) = self.finalize_parsing() {
            return Err(ParseFailure { xid: Some(xid), error });
        }

        Ok(ParsedRpcCall {
            header: RpcCallHeader {
                xid: rpc_header.xid,
                program: rpc_header.program,
                version: rpc_header.version,
                procedure: rpc_header.procedure,
                auth_flavor: rpc_header.auth_flavor,
            },
            arguments: proc,
        })
    }

    pub async fn parse_request(&mut self) -> Result<ParsedRpcCall> {
        self.parse_request_full().await.map_err(|failure| failure.error)
    }

    /// Parses a complete RPC message from the stream.
    ///
    /// This is the main entry point for parsing. It performs the following steps:
    /// 1. Reads the message header (framing)
    /// 2. Parses the RPC call header
    /// 3. Parses procedure-specific arguments
    /// 4. Validates that all data in the frame was consumed
    /// 5. Cleans up internal state for the next message
    ///
    /// If a protocol error occurs (version mismatch, auth error, etc.), the parser
    /// will attempt to discard the remaining message data to maintain stream alignment.
    ///
    /// # Returns
    ///
    /// Returns a boxed [`Arguments`] enum variant containing the parsed procedure arguments,
    /// or an error if parsing fails at any stage.
    pub async fn parse_message(&mut self) -> Result<Box<Arguments>> {
        Ok(self.parse_request().await?.arguments)
    }

    /// Finalizes parsing by validating that all frame data was consumed.
    ///
    /// This method is called after successful parsing to ensure that:
    /// - All bytes in the message frame were consumed
    /// - Internal buffer state is reset for the next message
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if validation passes, or an error if unparsed data
    /// remains in the frame (indicating a parsing bug or malformed message).
    fn finalize_parsing(&mut self) -> Result<()> {
        // CountBuffer keep count of bytes, read from it,
        // but first u32 of message - header that shouldn't be counted
        // https://datatracker.ietf.org/doc/html/rfc5531#section-11
        let bytes_consumed = self.buffer.total_bytes().checked_sub(RMS_HEADER_SIZE).ok_or(
            Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Consumed bytes are less than RMS header size",
            )),
        )?;
        if bytes_consumed != self.current_frame_size {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Unparsed data remaining in frame",
            )));
        }

        self.buffer.clean();
        self.current_frame_size = 0;
        self.last = false;
        Ok(())
    }

    /// Handles errors by potentially discarding the current message.
    ///
    /// For certain protocol-level errors (version mismatches, auth errors, etc.),
    /// this method discards the remaining message data to maintain stream alignment
    /// for subsequent messages. For other errors, it returns them as-is.
    ///
    /// # Arguments
    ///
    /// * `error` - The error that occurred during parsing
    ///
    /// # Returns
    ///
    /// Returns the error, potentially after attempting to discard the message.
    async fn match_errors(&mut self, error: Error) -> Error {
        if let Error::RpcVersionMismatch(_)
        | Error::ProgramMismatch
        | Error::ProcedureMismatch
        | Error::AuthError(_)
        | Error::MessageTypeMismatch
        | Error::ProgramVersionMismatch(_) = &error
        {
            proc_nested_errors(error, self.discard_current_message()).await
        } else {
            error
        }
    }

    /// Discards the remaining data in the current message frame.
    ///
    /// This method is called after protocol-level errors to skip over the
    /// remaining bytes in the current message, ensuring the stream is aligned
    /// for parsing the next message.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the message was successfully discarded, or an error
    /// if an I/O error occurs while discarding.
    async fn discard_current_message(&mut self) -> Result<()> {
        // CountBuffer keep count of bytes, read from it,
        // but first u32 of message - header that shouldn't be counted
        // https://datatracker.ietf.org/doc/html/rfc5531#section-11
        let remaining = (self.current_frame_size + RMS_HEADER_SIZE)
            .checked_sub(self.buffer.total_bytes())
            .ok_or(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Consumed more bytes than RMS header suggests",
            )))?;
        self.buffer.discard_bytes(remaining).await.map_err(Error::IO)?;
        self.finalize_parsing()?;
        Ok(())
    }
}

/// Special adapter for parsing WRITE procedure arguments.
///
/// The WRITE procedure requires special handling because it includes variable-length
/// data that must be allocated. This function:
/// 1. Parses the fixed portion of the WRITE arguments
/// 2. Allocates memory for the write data
/// 3. Reads the data from the buffer (handling both sync and async portions)
/// 4. Discards any padding bytes
///
/// # Arguments
///
/// * `alloc` - The allocator to use for allocating the write data buffer
/// * `buffer` - The buffer to read from
///
/// # Returns
///
/// Returns the parsed [`vfs::write::Args`] with allocated data, or an error if:
/// - Parsing fails
/// - Memory allocation fails
/// - Reading the data fails
async fn adapter_for_write<S: AsyncRead + Unpin>(
    alloc: &mut impl Allocator,
    buffer: &mut CountBuffer<S>,
) -> Result<vfs::write::Args> {
    // Parse arguments for WRITE procedure.
    let part_arg = buffer.parse_with_retry(write::args).await?;
    let size = buffer.parse_with_retry(u32_as_usize).await?;

    // Attempt allocation with the given size, or fallback to NonZeroUsize::MIN.
    let non_zero_size = NonZeroUsize::new(size).unwrap_or(NonZeroUsize::MIN);
    let mut slice = alloc.allocate(non_zero_size).await.ok_or_else(|| {
        Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory"))
    })?;

    // Calculate necessary padding to maintain ALIGNMENT
    let padding = (ALIGNMENT - (size % ALIGNMENT)) % ALIGNMENT;

    // Read synchronously what is available, then finish asynchronously if needed.
    let bytes_read_sync = read_in_slice_sync(buffer, &mut slice, size)?;
    if bytes_read_sync < size {
        read_in_slice_async(buffer, &mut slice, bytes_read_sync, size - bytes_read_sync).await?;
    }

    // Discard any trailing padding bytes after the data.
    buffer.discard_bytes(padding).await.map_err(Error::IO)?;
    Ok(vfs::write::Args {
        file: part_arg.file,
        offset: part_arg.offset,
        size: part_arg.size,
        stable: part_arg.stable,
        data: slice,
    })
}

/// Reads data into a slice asynchronously from the `CountBuffer`.
///
/// This function attempts to fill the provided `slice` with `to_write` bytes
/// from the `src` buffer, skipping `to_skip` bytes at the beginning of the slice.
/// It handles situations where data might be split across multiple internal buffers
/// of the `CountBuffer`.
///
/// # Arguments
///
/// * `src` - The `CountBuffer` to read data from.
/// * `slice` - The [`Slice`] to write the read data into.
/// * `to_skip` - The number of bytes to skip in the `slice` before writing.
/// * `to_write` - The number of bytes to write into the `slice`.
///
/// # Returns
///
/// Returns `Ok(usize)` indicating the number of bytes successfully written,
/// or an error if an I/O error occurs or buffer sizes are invalid.
pub async fn read_in_slice_async<S: AsyncRead + Unpin>(
    src: &mut CountBuffer<S>,
    slice: &mut Slice,
    to_skip: usize,
    to_write: usize,
) -> Result<usize> {
    let mut left_skip = to_skip;
    let mut left_write = to_write;
    for buf in slice.iter_mut() {
        let in_cur = min(left_skip, buf.len());
        if left_skip > 0 && in_cur == buf.len() {
            left_skip = left_skip
                .checked_sub(in_cur)
                .ok_or(Error::IO(io::Error::new(ErrorKind::InvalidInput, "invalid buffer size")))?;
            continue;
        }
        let cur_write = min(left_skip + left_write, buf.len() - left_skip);
        src.read_from_async(&mut buf[left_skip..left_skip + cur_write]).await.map_err(Error::IO)?;
        left_write = left_write
            .checked_sub(cur_write)
            .ok_or(Error::IO(io::Error::new(ErrorKind::InvalidInput, "invalid buffer size")))?;
        left_skip = 0;
    }
    Ok(to_write - left_write)
}

/// Reads data into a slice synchronously from the `CountBuffer`.
///
/// This function attempts to fill the provided `slice` with `left_size` bytes
/// from the `src` buffer. It reads synchronously until `left_size` bytes are read
/// or an I/O error occurs.
///
/// # Arguments
///
/// * `src` - The `CountBuffer` to read data from.
/// * `slice` - The [`Slice`] to write the read data into.
/// * `left_size` - The number of bytes expected to be read into the slice.
///
/// # Returns
///
/// Returns `Ok(usize)` indicating the number of bytes successfully read,
/// or an error if an I/O error occurs or the amount of data read is not as expected.
pub fn read_in_slice_sync<S: AsyncRead + Unpin>(
    src: &mut CountBuffer<S>,
    slice: &mut Slice,
    left_size: usize,
) -> Result<usize> {
    let mut real_size = 0;
    for buf in slice.iter_mut() {
        let block_size = min(buf.len(), left_size - real_size);
        let mut read_count = 0;
        // for my further notice:
        // this is done in maner of cyclic read, because we don't know, when we would fail
        while read_count < block_size {
            let n = match src.read_from_inner(&mut buf[read_count..block_size]) {
                Ok(0) => return Ok(real_size),
                Ok(n) => n,
                Err(e) => return Err(Error::IO(e)),
            };
            read_count += n;
            real_size += n;
        }
    }
    if real_size != left_size {
        return Err(Error::IO(io::Error::new(
            ErrorKind::InvalidInput,
            "invalid amount of data read",
        )));
    }
    Ok(real_size)
}
