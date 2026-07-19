//! RPC message parser for NFS, MOUNT and NLM protocols.
//!
//! This module provides the [`RpcParser`] struct, which parses XDR-encoded RPC messages
//! according to RFC 5531 (RPC) and RFC 1813 (NFSv3). It handles:
//!
//! - RPC message framing and headers
//! - Authentication (currently only AUTH_NONE)
//! - NFSv3 procedure parsing (all 22 procedures)
//! - MOUNT protocol procedure parsing
//! - NLM procedure parsing
//! - Error handling and message discarding on protocol errors
//!
//! The parser uses a [`FrameReader`] which buffers the head window of every
//! RMS frame, so headers and procedure arguments are parsed synchronously
//! without retries. Only opaque `WRITE` payloads that do not fit into the
//! buffered window are streamed from the socket directly into the allocated
//! [`Buffer`].

use std::cmp::min;
use std::io::{self, ErrorKind};
use std::num::NonZeroUsize;
use std::sync::Arc;

use tokio::io::AsyncRead;
use tracing::{debug, error, warn};

use crate::allocator::{Allocator, Buffer};
use crate::consts::mount::{
    MOUNT_DUMP, MOUNT_EXPORT, MOUNT_MNT, MOUNT_NULL, MOUNT_PROGRAM, MOUNT_UMNT, MOUNT_UMNTALL,
    MOUNT_VERSION,
};
use crate::consts::nfsv3::{
    ACCESS, COMMIT, CREATE, FSINFO, FSSTAT, GETATTR, LINK, LOOKUP, MKDIR, MKNOD, NFS_PROGRAM,
    NFS_VERSION, NULL, PATHCONF, READ, READDIR, READDIRPLUS, READLINK, REMOVE, RENAME, RMDIR,
    SETATTR, SYMLINK, WRITE,
};
use crate::consts::nlm::{
    NLMPROC4_CANCEL, NLMPROC4_LOCK, NLMPROC4_NULL, NLMPROC4_TEST, NLMPROC4_UNLOCK, NLM_PROGRAM,
    NLM_VERSION,
};
use crate::parser::mount::mnt::mount;
use crate::parser::mount::umnt::unmount;
use crate::parser::nfsv3::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};
use crate::parser::nlm::{cancel::cancel, lock::lock, test::test, unlock::unlock};
use crate::parser::primitive::{u32, u32_as_usize, ALIGNMENT};
use crate::parser::read_buffer::FrameReader;
use crate::parser::rpc::{auth, RpcMessage};
use crate::parser::{
    proc_nested_errors, ArgWrapper, Error, ErrorWrapper, MountArgWrapper, MountArguments,
    NfsArgWrapper, NfsArguments, NlmArguments, ProcArguments, Result, RpcHeader,
};
use crate::rpc::{AuthFlavor, AuthStat, OpaqueAuth, RpcBody, VersionMismatch, RPC_VERSION};
use crate::vfs;

/// Minimum buffer size, that could hold complete RPC message
/// with NFSv3 or Mount protocol arguments, except for NFSv3 `WRITE` procedure -
/// this size is enough to hold only arguments without opaque data ([`Buffer`] in [`vfs::write::Args`]).
///
/// The arguments of every procedure except the opaque `WRITE` data MUST fit
/// into the parser buffer: the parser reads at most `capacity` bytes of a
/// frame into memory before parsing and fails with `InvalidData` if the
/// arguments extend beyond that window.
pub const DEFAULT_SIZE: usize = 2500;

/// Maps an `UnexpectedEof` from synchronous argument parsing to `InvalidData`.
///
/// After [`FrameReader::begin_body`] the head window of the frame is fully
/// buffered, so running out of data during synchronous parsing means either
/// the frame is shorter than its arguments (truncated message) or the
/// arguments do not fit into the parser buffer (legal only for `WRITE`
/// payloads, which are streamed separately). Both cases are protocol
/// violations rather than transient I/O conditions.
fn map_eof(error: Error) -> Error {
    match error {
        Error::IO(err) if err.kind() == ErrorKind::UnexpectedEof => Error::IO(io::Error::new(
            ErrorKind::InvalidData,
            "RPC message arguments exceed frame or buffer bounds",
        )),
        other => other,
    }
}

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
pub struct RpcParser<A: Allocator, S: AsyncRead + Unpin> {
    allocator: Arc<A>,
    reader: FrameReader<S>,
}

impl<A: Allocator, S: AsyncRead + Unpin> RpcParser<A, S> {
    /// Creates a new `RpcParser` with [`DEFAULT_SIZE`] buffer size.
    ///
    /// # Arguments
    ///
    /// * `socket` - The async stream to read RPC messages from
    /// * `allocator` - The allocator to use for dynamic memory allocation
    ///
    /// # Returns
    ///
    /// A new `RpcParser` instance ready to parse messages.
    pub fn new(socket: S, allocator: Arc<A>) -> Self {
        Self { allocator, reader: FrameReader::new(DEFAULT_SIZE, socket) }
    }

    /// Creates a new `RpcParser` with the specified buffer size.
    ///
    /// # Arguments
    ///
    /// * `socket` - The async stream to read RPC messages from
    /// * `allocator` - The allocator to use for dynamic memory allocation
    /// * `size` - The size of the internal frame buffer; arguments of any
    ///   procedure (except opaque `WRITE` data) must fit into it
    ///
    /// # Returns
    ///
    /// A new `RpcParser` instance ready to parse messages.
    #[allow(dead_code)]
    pub fn with_capacity(socket: S, allocator: Arc<A>, size: usize) -> Self {
        Self { allocator, reader: FrameReader::new(size, socket) }
    }

    /// Reads and parses the RPC message header.
    ///
    /// The message header contains:
    /// - A 32-bit word with the most significant bit indicating if this is the last fragment
    /// - The remaining 31 bits containing the fragment size
    /// - The transaction ID (XID)
    ///
    /// After validating the header, the head window of the frame body
    /// (`min(frame_size, capacity)` bytes) is buffered so that subsequent
    /// header and argument parsing is fully synchronous.
    ///
    /// Currently, fragmented messages are not supported and will return an error.
    ///
    /// # Returns
    ///
    /// Returns the XID if the header was successfully parsed, or an error if:
    /// - The message is fragmented (not supported)
    /// - An I/O error occurs
    async fn read_message_header(&mut self) -> Result<u32> {
        let header = self.reader.read_frame_header().await.map_err(Error::IO)?;
        let last = header & 0x8000_0000 != 0;
        let frame_size = (header & 0x7FFF_FFFF) as usize;

        if frame_size < std::mem::size_of::<u32>() {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Frame size must include XID",
            )));
        }

        //TODO("https://github.com/RMamonts/nfs-mamont/issues/124")

        // this is temporal check, apparently this will go to separate object Validator
        if !last {
            return Err(Error::IO(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            )));
        }

        self.reader.begin_body(frame_size).await.map_err(Error::IO)?;
        u32(&mut self.reader).map_err(map_eof)
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
    /// - The frame or the parser buffer is too small for the header
    fn parse_rpc_header(&mut self) -> Result<RpcMessage> {
        self.parse_rpc_header_inner().map_err(map_eof)
    }

    fn parse_rpc_header_inner(&mut self) -> Result<RpcMessage> {
        let msg_type = u32(&mut self.reader)?;
        if msg_type != RpcBody::Call as u32 {
            error!(msg_type, "rpc parse reject: unexpected msg_type");
            return Err(Error::MessageTypeMismatch);
        }

        let rpc_version = u32(&mut self.reader)?;
        if rpc_version != RPC_VERSION {
            error!(rpc_version, expected = RPC_VERSION, "rpc parse reject: rpc_version mismatch");
            return Err(Error::RpcVersionMismatch(VersionMismatch {
                low: RPC_VERSION,
                high: RPC_VERSION,
            }));
        }

        let program = u32(&mut self.reader)?;
        let version = u32(&mut self.reader)?;
        let procedure = u32(&mut self.reader)?;
        debug!(program, version, procedure, "rpc header parsed");

        //TODO(https://github.com/RMamonts/nfs-mamont/issues/156)
        let (cred, verf) = self.parse_authentication()?;

        Ok(RpcMessage { program, procedure, version, cred, verf })
    }

    /// Parses and validates RPC authentication.
    ///
    /// NFS clients commonly send AUTH_SYS credentials, so both AUTH_NONE and
    /// AUTH_SYS are accepted for credentials. Verifier must remain AUTH_NONE.
    ///
    /// # Returns
    ///
    /// Returns a pair of [`OpaqueAuth`] if authentication succeeds, or an error
    /// if authentication fails or an I/O error occurs.
    fn parse_authentication(&mut self) -> Result<(OpaqueAuth, OpaqueAuth)> {
        let cred = auth(&mut self.reader)?;
        let verf = auth(&mut self.reader)?;
        let cred_ok = match cred.flavor {
            AuthFlavor::None => cred.body.is_empty(),
            AuthFlavor::Sys => true,
            _ => false,
        };
        if !cred_ok {
            error!(
                cred_flavor=?cred.flavor,
                cred_len=%cred.body.len(),
                "rpc auth reject: unsupported credential flavor",
            );
            return Err(Error::Auth(AuthStat::BadCred));
        }
        if !matches!(verf.flavor, AuthFlavor::None) || !verf.body.is_empty() {
            error!(
                verf_flavor=?verf.flavor,
                verf_len=%verf.body.len(),
                "rpc auth reject: invalid verifier",
            );
            return Err(Error::Auth(AuthStat::BadVerf));
        }
        debug!(
            cred_flavor=?cred.flavor,
            cred_len=%cred.body.len(),
            verf_flavor=?verf.flavor,
            verf_len=%verf.body.len(),
            "rpc auth accepted",
        );
        Ok((cred, verf))
    }

    /// Parses NFSv3 procedure arguments from the current frame.
    ///
    /// All arguments are parsed synchronously from the buffered head window,
    /// except the opaque `WRITE` payload which is streamed via
    /// [`adapter_for_write`].
    async fn parse_nfs_proc(&mut self, procedure: u32) -> Result<NfsArguments<A::Buffer>> {
        if procedure == WRITE {
            return Ok(NfsArguments::Write(
                adapter_for_write(&self.allocator, &mut self.reader).await?,
            ));
        }
        let src = &mut self.reader;
        let args = match procedure {
            NULL => NfsArguments::Null,
            GETATTR => NfsArguments::GetAttr(get_attr::args(src).map_err(map_eof)?),
            SETATTR => NfsArguments::SetAttr(set_attr::args(src).map_err(map_eof)?),
            LOOKUP => NfsArguments::LookUp(lookup::args(src).map_err(map_eof)?),
            ACCESS => NfsArguments::Access(access::args(src).map_err(map_eof)?),
            READLINK => NfsArguments::ReadLink(read_link::args(src).map_err(map_eof)?),
            READ => NfsArguments::Read(read::args(src).map_err(map_eof)?),
            CREATE => NfsArguments::Create(create::args(src).map_err(map_eof)?),
            MKDIR => NfsArguments::MkDir(mk_dir::args(src).map_err(map_eof)?),
            SYMLINK => NfsArguments::SymLink(symlink::args(src).map_err(map_eof)?),
            MKNOD => NfsArguments::MkNod(mk_node::args(src).map_err(map_eof)?),
            REMOVE => NfsArguments::Remove(remove::args(src).map_err(map_eof)?),
            RMDIR => NfsArguments::RmDir(rm_dir::args(src).map_err(map_eof)?),
            RENAME => NfsArguments::Rename(rename::args(src).map_err(map_eof)?),
            LINK => NfsArguments::Link(link::args(src).map_err(map_eof)?),
            READDIR => NfsArguments::ReadDir(read_dir::args(src).map_err(map_eof)?),
            READDIRPLUS => NfsArguments::ReadDirPlus(read_dir_plus::args(src).map_err(map_eof)?),
            FSSTAT => NfsArguments::FsStat(fs_stat::args(src).map_err(map_eof)?),
            FSINFO => NfsArguments::FsInfo(fs_info::args(src).map_err(map_eof)?),
            PATHCONF => NfsArguments::PathConf(path_conf::args(src).map_err(map_eof)?),
            COMMIT => NfsArguments::Commit(commit::args(src).map_err(map_eof)?),
            _ => return Err(Error::ProcedureMismatch),
        };
        Ok(args)
    }

    /// Parses MOUNT procedure arguments from the current frame.
    fn parse_mount_proc(&mut self, procedure: u32) -> Result<MountArguments> {
        let args = match procedure {
            MOUNT_NULL => MountArguments::Null,
            MOUNT_MNT => MountArguments::Mount(mount(&mut self.reader).map_err(map_eof)?),
            MOUNT_DUMP => MountArguments::Dump,
            MOUNT_UMNT => MountArguments::Unmount(unmount(&mut self.reader).map_err(map_eof)?),
            MOUNT_UMNTALL => MountArguments::UnmountAll,
            MOUNT_EXPORT => MountArguments::Export,
            _ => return Err(Error::ProcedureMismatch),
        };
        Ok(args)
    }

    /// Parses NLM procedure arguments from the current frame.
    fn parse_nlm_proc(&mut self, procedure: u32) -> Result<NlmArguments> {
        let args = match procedure {
            NLMPROC4_NULL => NlmArguments::Null,
            NLMPROC4_LOCK => NlmArguments::Lock(lock(&mut self.reader).map_err(map_eof)?),
            NLMPROC4_UNLOCK => NlmArguments::Unlock(unlock(&mut self.reader).map_err(map_eof)?),
            NLMPROC4_TEST => NlmArguments::Test(test(&mut self.reader).map_err(map_eof)?),
            NLMPROC4_CANCEL => NlmArguments::Cancel(cancel(&mut self.reader).map_err(map_eof)?),
            _ => return Err(Error::ProcedureMismatch),
        };
        Ok(args)
    }

    /// Parses a complete NFSv3 RPC message from the stream.
    ///
    /// This is the main entry point for parsing. It performs the following steps:
    /// 1. Reads the message header (framing) and buffers the frame head window
    /// 2. Parses the RPC call header
    /// 3. Parses NFSv3 procedure-specific arguments
    /// 4. Validates that all data in the frame was consumed
    ///
    /// If a protocol error occurs (version mismatch, auth error, etc.), the parser
    /// will attempt to discard the remaining message data to maintain stream alignment.
    ///
    /// # Returns
    ///
    /// Returns parsed NFSv3 procedure arguments,
    /// or an error if parsing fails at any stage.
    #[allow(dead_code)]
    pub async fn parse_nfs_message(&mut self) -> Result<NfsArgWrapper<A::Buffer>> {
        let xid = self.read_message_header().await?;
        let rpc_header = match self.parse_rpc_header() {
            Ok(arg) => arg,
            Err(err) => return Err(self.match_errors(err).await),
        };
        let proc = match self.parse_nfs_message_with_header(&rpc_header).await {
            Ok(arg) => Box::new(arg),
            Err(err) => return Err(self.match_errors(err).await),
        };

        // finalize_parsing() is only called after successful header and procedure parsing; it is not run on error paths
        self.finalize_parsing()?;
        Ok(NfsArgWrapper {
            header: RpcHeader { xid, cred: rpc_header.cred, verf: rpc_header.verf },
            proc,
        })
    }

    /// Parses the next RPC message and returns typed arguments for its program.
    ///
    /// This is the generic entry point for call sites that do not know in advance
    /// whether the next frame contains NFSv3 or MOUNT data.
    pub async fn next_message(
        &mut self,
    ) -> core::result::Result<ArgWrapper<A::Buffer>, ErrorWrapper> {
        let xid = match self.read_message_header().await {
            Ok(xid) => xid,
            Err(error) => return Err(ErrorWrapper { xid: None, error }),
        };
        let rpc_header = match self.parse_rpc_header() {
            Ok(arg) => arg,
            Err(err) => {
                return Err(ErrorWrapper { xid: Some(xid), error: self.match_errors(err).await })
            }
        };
        let proc = match self.parse_next_message_with_header(&rpc_header).await {
            Ok(arg) => arg,
            Err(err) => {
                return Err(ErrorWrapper { xid: Some(xid), error: self.match_errors(err).await })
            }
        };

        // finalize_parsing() is only called after successful header and procedure parsing; it is not run on error paths
        match self.finalize_parsing() {
            Ok(_) => Ok(ArgWrapper {
                header: RpcHeader { xid, cred: rpc_header.cred, verf: rpc_header.verf },
                proc,
            }),
            Err(error) => Err(ErrorWrapper { xid: Some(xid), error }),
        }
    }

    /// Parses a complete MOUNT RPC message from the stream.
    #[allow(dead_code)]
    pub async fn parse_mount_message(&mut self) -> Result<MountArgWrapper> {
        let xid = self.read_message_header().await?;
        let rpc_header = match self.parse_rpc_header() {
            Ok(arg) => arg,
            Err(err) => return Err(self.match_errors(err).await),
        };
        let proc = match self.parse_mount_message_with_header(&rpc_header) {
            Ok(arg) => Box::new(arg),
            Err(err) => return Err(self.match_errors(err).await),
        };

        // finalize_parsing() is only called after successful header and procedure parsing; it is not run on error paths
        self.finalize_parsing()?;
        Ok(MountArgWrapper {
            header: RpcHeader { xid, cred: rpc_header.cred, verf: rpc_header.verf },
            proc,
        })
    }

    async fn parse_next_message_with_header(
        &mut self,
        head: &RpcMessage,
    ) -> Result<ProcArguments<A::Buffer>> {
        match head.program {
            NFS_PROGRAM => {
                let args = self.parse_nfs_message_with_header(head).await?;
                Ok(ProcArguments::Nfs3(Box::new(args)))
            }
            MOUNT_PROGRAM => {
                let args = self.parse_mount_message_with_header(head)?;
                Ok(ProcArguments::Mount(Box::new(args)))
            }
            NLM_PROGRAM => {
                let args = self.parse_nlm_message_with_header(head)?;
                Ok(ProcArguments::Nlm4(Box::new(args)))
            }
            _ => {
                warn!(program = head.program, "rpc parse reject: unknown program");
                Err(Error::ProgramMismatch)
            }
        }
    }

    async fn parse_nfs_message_with_header(
        &mut self,
        head: &RpcMessage,
    ) -> Result<NfsArguments<A::Buffer>> {
        if head.program != NFS_PROGRAM {
            error!(
                got = head.program,
                expected = NFS_PROGRAM,
                "rpc parse reject: nfs parser got unexpected program",
            );
            return Err(Error::ProgramMismatch);
        }
        if head.version != NFS_VERSION {
            error!(
                got = head.version,
                expected = NFS_VERSION,
                "rpc parse reject: nfs version mismatch",
            );
            return Err(Error::ProgramVersionMismatch(VersionMismatch {
                low: NFS_VERSION,
                high: NFS_VERSION,
            }));
        }
        self.parse_nfs_proc(head.procedure).await
    }

    fn parse_mount_message_with_header(&mut self, head: &RpcMessage) -> Result<MountArguments> {
        if head.program != MOUNT_PROGRAM {
            error!(
                got = head.program,
                expected = MOUNT_PROGRAM,
                "rpc parse reject: mount parser got unexpected program",
            );
            return Err(Error::ProgramMismatch);
        }
        if head.version != MOUNT_VERSION {
            error!(
                got = head.version,
                expected = MOUNT_VERSION,
                "rpc parse reject: mount version mismatch",
            );
            return Err(Error::ProgramVersionMismatch(VersionMismatch {
                low: MOUNT_VERSION,
                high: MOUNT_VERSION,
            }));
        }
        self.parse_mount_proc(head.procedure)
    }

    fn parse_nlm_message_with_header(&mut self, head: &RpcMessage) -> Result<NlmArguments> {
        if head.program != NLM_PROGRAM {
            error!(
                got = head.program,
                expected = NLM_PROGRAM,
                "rpc parse reject: NLM parser got unexpected program",
            );
            return Err(Error::ProgramMismatch);
        }
        if head.version != NLM_VERSION {
            error!(
                got = head.version,
                expected = NLM_VERSION,
                "rpc parse reject: NLM version mismatch",
            );
            return Err(Error::ProgramVersionMismatch(VersionMismatch {
                low: NLM_VERSION,
                high: NLM_VERSION,
            }));
        }
        self.parse_nlm_proc(head.procedure)
    }

    /// Finalizes parsing by validating that all frame data was consumed.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if validation passes, or an error if unparsed data
    /// remains in the frame (indicating a parsing bug or malformed message).
    fn finalize_parsing(&mut self) -> Result<()> {
        if self.reader.frame_remaining() != 0 {
            return Err(Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Unparsed data remaining in frame",
            )));
        }
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
        | Error::Auth(_)
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
        self.reader.discard_rest_of_frame().await.map_err(Error::IO)
    }
}

/// Special adapter for parsing WRITE procedure arguments.
///
/// The WRITE procedure requires special handling because it includes variable-length
/// data that must be allocated. This function:
/// 1. Parses the fixed portion of the WRITE arguments from the buffered head window
/// 2. Validates that the declared data length fits into the frame
/// 3. Allocates memory for the write data
/// 4. Fills the allocated buffer (from the buffered window first, then directly
///    from the socket)
/// 5. Discards any padding bytes
///
/// # Arguments
///
/// * `alloc` - The allocator to use for allocating the write data buffer
/// * `reader` - The frame reader to read from
///
/// # Returns
///
/// Returns the parsed [`vfs::write::Args`] with allocated data, or an error if:
/// - Parsing fails
/// - The declared data length does not fit into the frame
/// - Memory allocation fails
/// - Reading the data fails
async fn adapter_for_write<A, S>(
    alloc: &Arc<A>,
    reader: &mut FrameReader<S>,
) -> Result<vfs::write::Args<A::Buffer>>
where
    A: Allocator,
    S: AsyncRead + Unpin,
{
    // Parse arguments for WRITE procedure.
    let part_arg = write::args(reader).map_err(map_eof)?;
    let size = u32_as_usize(reader).map_err(map_eof)?;

    // Calculate necessary padding to maintain ALIGNMENT
    let padding = (ALIGNMENT - (size % ALIGNMENT)) % ALIGNMENT;

    // The opaque data with its padding must lie within the current frame;
    // otherwise the declared length is bogus and reading it would consume
    // bytes of the next message, misaligning the stream.
    if size + padding > reader.frame_remaining() {
        return Err(Error::IO(io::Error::new(
            ErrorKind::InvalidData,
            "WRITE data length exceeds frame size",
        )));
    }

    // Attempt allocation with the given size, or fallback to NonZeroUsize::MIN.
    let non_zero_size = NonZeroUsize::new(size).unwrap_or(NonZeroUsize::MIN);
    let mut buffer_data = alloc.allocate(non_zero_size).await.ok_or_else(|| {
        Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory"))
    })?;

    // Fill the allocated buffer chunk by chunk; `read_body_exact` consumes the
    // buffered window first and reads the rest directly from the socket.
    let mut left = size;
    for chunk in buffer_data.chunks_mut() {
        if left == 0 {
            break;
        }
        let take = min(chunk.len(), left);
        reader.read_body_exact(&mut chunk[..take]).await.map_err(Error::IO)?;
        left -= take;
    }
    if left != 0 {
        return Err(Error::IO(io::Error::new(
            ErrorKind::InvalidInput,
            "allocated buffer is smaller than WRITE data",
        )));
    }

    // Discard any trailing padding bytes after the data.
    reader.discard_body(padding).await.map_err(Error::IO)?;
    Ok(vfs::write::Args {
        file: part_arg.file,
        offset: part_arg.offset,
        size: part_arg.size,
        stable: part_arg.stable,
        data: buffer_data,
    })
}
