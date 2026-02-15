//! RPC message parser for NFS and MOUNT protocols.
//!
//! This module provides the [`RpcParser`] struct, which parses XDR-encoded RPC messages
//! according to RFC 5531 (RPC) and RFC 1813 (NFSv3). It handles:
//!
//! - RPC message framing and headers
//! - Authentication (currently only AUTH_NONE)
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
use crate::nfsv3::{NFS_PROGRAM, NFS_VERSION};
use crate::parser::mount::{mount, unmount};
use crate::parser::nfsv3::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};
use crate::parser::primitive::{u32, u32_as_usize, ALIGNMENT};
use crate::parser::read_buffer::CountBuffer;
use crate::parser::rpc::{auth, AuthFlavor, AuthStat, RpcMessage};
use crate::parser::{
    proc_nested_errors, Arguments, Error, ProgramVersionMismatch, RPCVersionMismatch, Result,
};
use crate::rpc::{rpc_message_type, RPC_VERSION};
use crate::vfs;

#[allow(dead_code)]
const MAX_MESSAGE_LEN: usize = 2500;

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
/// ```no_run
/// use tokio::io::AsyncRead;
/// use crate::parser::parser_struct::RpcParser;
/// use crate::allocator::Allocator;
///
/// # async fn example<A: Allocator, S: AsyncRead + Unpin>(socket: S, alloc: A) {
/// let mut parser = RpcParser::new(socket, alloc, 4096);
/// let args = parser.parse_message().await?;
/// # }
/// ```
pub struct RpcParser<A: Allocator, S: AsyncRead + Unpin> {
    allocator: A,
    buffer: CountBuffer<S>,
    last: bool,
    current_frame_size: usize,
}

#[allow(dead_code)]
impl<A: Allocator, S: AsyncRead + Unpin> RpcParser<A, S> {
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
    pub fn new(socket: S, allocator: A, size: usize) -> Self {
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
    async fn read_message_header(&mut self) -> Result<()> {
        let header = self.buffer.parse_with_retry(u32).await?;
        self.last = header & 0x8000_0000 != 0;
        self.current_frame_size = (header & 0x7FFF_FFFF) as usize;

        // this is temporal check, apparently this will go to separate object Validator
        if !self.last {
            return Err(Error::IO(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            )));
        }
        let _xid = self.buffer.parse_with_retry(u32).await?;
        Ok(())
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
    async fn parse_rpc_header(&mut self) -> Result<RpcMessage> {
        let msg_type = self.buffer.parse_with_retry(u32).await?;
        if msg_type != rpc_message_type::CALL as u32 {
            return Err(Error::MessageTypeMismatch);
        }

        let rpc_version = self.buffer.parse_with_retry(u32).await?;
        if rpc_version != RPC_VERSION {
            return Err(Error::RpcVersionMismatch(RPCVersionMismatch(RPC_VERSION, RPC_VERSION)));
        }

        let program = self.buffer.parse_with_retry(u32).await?;
        let version = self.buffer.parse_with_retry(u32).await?;
        let procedure = self.buffer.parse_with_retry(u32).await?;

        let auth_status = self.parse_authentication().await?;
        if auth_status != AuthStat::AuthOk {
            return Err(Error::AuthError(auth_status));
        }

        Ok(RpcMessage { program, procedure, version })
    }

    /// Parses and validates RPC authentication.
    ///
    /// Currently only AUTH_NONE is supported.
    ///
    /// # Returns
    ///
    /// Returns [`AuthStat::AuthOk`] if authentication succeeds, or an error
    /// if authentication fails or an I/O error occurs.
    async fn parse_authentication(&mut self) -> Result<AuthStat> {
        match self.buffer.parse_with_retry(auth).await?.flavor {
            AuthFlavor::AuthNone => Ok(AuthStat::AuthOk),
            _ => {
                unimplemented!()
            }
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
            NFS_PROGRAM => match head.version {
                NFS_VERSION => Ok(Box::new(match head.procedure {
                    0 => Arguments::Null,
                    1 => Arguments::GetAttr(self.buffer.parse_with_retry(get_attr::args).await?),
                    2 => Arguments::SetAttr(self.buffer.parse_with_retry(set_attr::args).await?),
                    3 => Arguments::LookUp(self.buffer.parse_with_retry(lookup::args).await?),
                    4 => Arguments::Access(self.buffer.parse_with_retry(access::args).await?),
                    5 => Arguments::ReadLink(self.buffer.parse_with_retry(read_link::args).await?),
                    6 => Arguments::Read(self.buffer.parse_with_retry(read::args).await?),

                    7 => Arguments::Write(
                        adapter_for_write(&mut self.allocator, &mut self.buffer).await?,
                    ),

                    8 => Arguments::Create(self.buffer.parse_with_retry(create::args).await?),
                    9 => Arguments::MkDir(self.buffer.parse_with_retry(mk_dir::args).await?),
                    10 => Arguments::SymLink(self.buffer.parse_with_retry(symlink::args).await?),
                    11 => Arguments::MkNod(self.buffer.parse_with_retry(mk_node::args).await?),
                    12 => Arguments::Remove(self.buffer.parse_with_retry(remove::args).await?),
                    13 => Arguments::RmDir(self.buffer.parse_with_retry(rm_dir::args).await?),
                    14 => Arguments::Rename(self.buffer.parse_with_retry(rename::args).await?),
                    15 => Arguments::Link(self.buffer.parse_with_retry(link::args).await?),
                    16 => Arguments::ReadDir(self.buffer.parse_with_retry(read_dir::args).await?),
                    17 => Arguments::ReadDirPlus(
                        self.buffer.parse_with_retry(read_dir_plus::args).await?,
                    ),
                    18 => Arguments::FsStat(self.buffer.parse_with_retry(fs_stat::args).await?),
                    19 => Arguments::FsInfo(self.buffer.parse_with_retry(fs_info::args).await?),
                    20 => Arguments::PathConf(self.buffer.parse_with_retry(path_conf::args).await?),
                    21 => Arguments::Commit(self.buffer.parse_with_retry(commit::args).await?),
                    _ => return Err(Error::ProcedureMismatch),
                })),
                _ => Err(Error::ProgramVersionMismatch(ProgramVersionMismatch(
                    NFS_VERSION,
                    NFS_VERSION,
                ))),
            },

            MOUNT_PROGRAM => {
                if head.version != MOUNT_VERSION {
                    return Err(Error::ProgramVersionMismatch(ProgramVersionMismatch(
                        MOUNT_VERSION,
                        MOUNT_VERSION,
                    )));
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
            _ => Err(Error::ProgramMismatch),
        }
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
        self.read_message_header().await?;
        let rpc_header = match self.parse_rpc_header().await {
            Ok(arg) => arg,
            Err(err) => return Err(self.match_errors(err).await),
        };
        let proc = match self.parse_proc(rpc_header).await {
            Ok(arg) => arg,
            Err(err) => return Err(self.match_errors(err).await),
        };

        // that's done after normal parsing without errors
        self.finalize_parsing()?;
        Ok(proc)
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
        if self.buffer.total_bytes() != self.current_frame_size {
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
        let remaining = self.current_frame_size - self.buffer.total_bytes();
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
    let part_arg = buffer.parse_with_retry(write::args).await?;
    let size = buffer.parse_with_retry(u32_as_usize).await?;
    let mut slice = alloc
        .allocate(NonZeroUsize::new(size).unwrap())
        .await
        .ok_or(Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory")))?;
    let padding = (ALIGNMENT - size % ALIGNMENT) % ALIGNMENT;
    let from_sync = read_in_slice_sync(buffer, &mut slice, size)?;
    read_in_slice_async(buffer, &mut slice, from_sync, size - from_sync).await?;
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
