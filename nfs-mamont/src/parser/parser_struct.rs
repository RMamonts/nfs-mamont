//! RPC message parser for NFS, MOUNT and NLM protocols.
//!
//! This module provides the [`RpcParser`] struct, which parses XDR-encoded RPC
//! messages according to RFC 5531 (RPC) and RFC 1813 (NFSv3, MOUNT, NLM). It
//! handles:
//!
//! - RPC message framing and headers
//! - Authentication (AUTH_NONE and AUTH_SYS)
//! - NFSv3 procedure parsing (all 22 procedures)
//! - MOUNT protocol procedure parsing
//! - NLM procedure parsing
//! - Error handling and message discarding on protocol errors
//!
//! The parser uses a [`FrameReader`] which buffers the head window of every
//! RPC frame, so headers and procedure arguments are parsed synchronously
//! without retries. Only opaque `WRITE` payloads that do not fit into the
//! buffered window are streamed from the socket directly into the allocated
//! [`Buffer`].

use std::cmp::min;
use std::io::{self, ErrorKind};
use std::num::NonZeroUsize;
use std::sync::Arc;

use num_traits::FromPrimitive;
use tokio::io::AsyncRead;
use tracing::{debug, error, warn};

use crate::allocator::{Allocator, Buffer};
use crate::consts::mount::{MountProc, MOUNT_PROGRAM, MOUNT_VERSION};
use crate::consts::nfsv3::{NfsProc, NFS_PROGRAM, NFS_VERSION};
use crate::consts::nlm::{NlmProc, NLM_PROGRAM, NLM_VERSION};
use crate::consts::rpc::{HEADER_MASK, MAX_FRAGMENT_SIZE, RPC_VERSION};
use crate::consts::xdr::padding_len;
use crate::parser;
use crate::parser::mount::mnt::mount;
use crate::parser::mount::umnt::unmount;
use crate::parser::nfsv3::{
    access, commit, create, fs_info, fs_stat, get_attr, link, lookup, mk_dir, mk_node, path_conf,
    read, read_dir, read_dir_plus, read_link, remove, rename, rm_dir, set_attr, symlink, write,
};
use crate::parser::nlm::{cancel::cancel, lock::lock, test::test, unlock::unlock};
use crate::parser::primitive::{u32, u32_as_usize};
use crate::parser::read_buffer::FrameReader;
use crate::parser::rpc::{auth, authsys_parms, RpcMessage};
use crate::parser::{
    proc_nested_errors, ArgWrapper, MountArguments, NfsArguments, NlmArguments, ProcArguments,
    RpcHeader,
};
use crate::rpc;
use crate::rpc::auth::{AuthStat, Credential};
use crate::rpc::{AuthFlavor, ErrorWrapper, OpaqueAuth, RpcBody, VersionMismatch};
use crate::vfs;

/// Minimum buffer size, that could hold complete RPC message
/// with NFSv3 or Mount protocol arguments, except for NFSv3 `WRITE` procedure -
/// this size is enough to hold only arguments without opaque data ([`Buffer`] in [`vfs::write::Args`]).
///
/// The arguments of every procedure except for the opaque `WRITE` data MUST fit
/// into the parser buffer: the parser reads at most `capacity` bytes of a
/// frame into memory before parsing and fails with `InvalidData` if the
/// arguments extend beyond that window.
pub const DEFAULT_SIZE: usize = 2500;

/// Maps a structural parser error to the corresponding rpc-level error,
/// converting it at the parser boundary so the rest of the server works with
/// [`crate::rpc::Error`] only.
fn map_parser_to_rpc(error: parser::Error) -> rpc::Error {
    match error {
        parser::Error::MaxElemLimit => rpc::Error::MaxElemLimit,
        parser::Error::EnumDiscMismatch => rpc::Error::EnumDiscMismatch,
        parser::Error::IncorrectString(err) => rpc::Error::IncorrectString(err),
        parser::Error::ImpossibleTypeCast => rpc::Error::ImpossibleTypeCast,
        parser::Error::BadFileHandle => rpc::Error::BadFileHandle,
        parser::Error::InvalidData => rpc::Error::IO(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid data received with parsing failure",
        )),
        parser::Error::FormatMismatch => rpc::Error::IO(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Data does not match expected internal format",
        )),
        parser::Error::IO(err) => rpc::Error::IO(err),
    }
}

/// Raw RPC call header fields that follow `rpcvers`, read from the wire
/// before any semantic validation is applied.
///
/// Carries the program, version and procedure words together with the raw
/// credential and verifier opaques. [`RpcParser::validate_header`] turns this
/// into a validated [`RpcMessage`], applying the authentication decisions.
struct RawHeader {
    program: u32,
    version: u32,
    procedure: u32,
    credential: OpaqueAuth,
    verifier: OpaqueAuth,
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
        Self::with_capacity(socket, allocator, DEFAULT_SIZE)
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
    async fn read_message_header(&mut self) -> parser::Result<u32> {
        let header = self.reader.read_frame_header().await.map_err(parser::Error::IO)?;
        let last = header & (HEADER_MASK as u32) != 0;
        let frame_size = header as usize & MAX_FRAGMENT_SIZE;
        self.reader.begin_body(frame_size).await.map_err(parser::Error::IO)?;

        if frame_size < std::mem::size_of::<u32>() {
            return Err(parser::Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Frame size must include XID",
            )));
        }

        //TODO("https://github.com/RMamonts/nfs-mamont/issues/124")
        // Fragmented (multi-fragment) messages are not yet supported.
        if !last {
            return Err(parser::Error::IO(io::Error::new(
                ErrorKind::Unsupported,
                "Fragmented messages not supported",
            )));
        }

        u32(&mut self.reader)
    }

    /// Reads and validates the `msg_type` and `rpcvers` words that open a call.
    ///
    /// Unlike the rest of the header, each of them is validated as soon as it
    /// is read: `msg_type` is the discriminant of the `rpc_msg` union, so the
    /// bytes after a `REPLY` are a `reply_body` rather than a call header, and
    /// `rpcvers` defines the layout of the call that follows (RFC 5531 §9).
    /// Decoding the rest of the header first would report such records as
    /// structural errors, which the server answers, while a
    /// [`rpc::Error::MessageTypeMismatch`] record is dropped by the read task.
    fn read_call_prefix(&mut self) -> rpc::Result<()> {
        let msg_type = u32(&mut self.reader).map_err(map_parser_to_rpc)?;
        if msg_type != RpcBody::Call as u32 {
            error!(msg_type, "rpc parse reject: unexpected msg_type");
            return Err(rpc::Error::MessageTypeMismatch);
        }

        let rpc_version = u32(&mut self.reader).map_err(map_parser_to_rpc)?;
        if rpc_version != RPC_VERSION {
            error!(rpc_version, expected = RPC_VERSION, "rpc parse reject: rpc_version mismatch");
            return Err(rpc::Error::RpcVersionMismatch(VersionMismatch {
                low: RPC_VERSION,
                high: RPC_VERSION,
            }));
        }
        Ok(())
    }

    /// Structurally reads the raw RPC call header fields that follow
    /// [`Self::read_call_prefix`] together with the raw credential and
    /// verifier opaques, without applying any semantic validation.
    ///
    /// Errors returned by this function are parser-level ([`parser::Error`]) — an I/O
    /// or XDR decode failure — as opposed to the semantic rpc errors that the
    /// caller produces from the returned fields via [`Self::validate_header`].
    fn read_rpc_header(&mut self) -> parser::Result<RawHeader> {
        let program = u32(&mut self.reader)?;
        let version = u32(&mut self.reader)?;
        let procedure = u32(&mut self.reader)?;
        let credential = auth(&mut self.reader)?;
        let verifier = auth(&mut self.reader)?;
        Ok(RawHeader { program, version, procedure, credential, verifier })
    }

    /// Semantically validates the raw call header and authentication material
    /// and builds a validated [`RpcMessage`].
    ///
    /// The checks performed here turn raw wire values into the rpc-level errors
    /// the server understands (`msg_type` and `rpcvers` are already validated
    /// by [`Self::read_call_prefix`]):
    /// - The credential/verifier pair must be acceptable ([`rpc::Error::Auth`])
    ///
    /// Both `AUTH_NONE` (anonymous) and `AUTH_SYS` (UNIX uid/gid) credentials
    /// are accepted; the `AUTH_SYS` body is decoded into an [`AuthSysParams`].
    /// Any other credential flavor is rejected with [`AuthStat::BadCred`]. For
    /// both accepted flavors, the verifier must be `AUTH_NONE` with an empty
    /// body (RFC 5531 §8.2); anything else is rejected with [`AuthStat::BadVerf`].
    ///
    /// [`AuthSysParams`]: crate::rpc::auth::AuthSysParams
    fn validate_header(&self, raw: &RawHeader) -> rpc::Result<RpcMessage> {
        let credential = match raw.credential.flavor {
            AuthFlavor::None if raw.credential.body.is_empty() => Credential::None,
            AuthFlavor::Sys => match authsys_parms(&mut raw.credential.body.as_slice()) {
                Ok(params) => Credential::Sys(params),
                Err(err) => {
                    error!(
                        cred_len=%raw.credential.body.len(),
                        error=?err,
                        "rpc auth reject: malformed AUTH_SYS credential",
                    );
                    return Err(rpc::Error::Auth(AuthStat::BadCred));
                }
            },
            _ => {
                error!(
                    cred_flavor=?raw.credential.flavor,
                    cred_len=%raw.credential.body.len(),
                    "rpc auth reject: unsupported credential flavor",
                );
                return Err(rpc::Error::Auth(AuthStat::BadCred));
            }
        };

        if !matches!(raw.verifier.flavor, AuthFlavor::None) || !raw.verifier.body.is_empty() {
            error!(
                verf_flavor=?raw.verifier.flavor,
                verf_len=%raw.verifier.body.len(),
                "rpc auth reject: invalid verifier",
            );
            return Err(rpc::Error::Auth(AuthStat::BadVerf));
        }

        debug!(
            program = raw.program,
            version = raw.version,
            procedure = raw.procedure,
            cred_flavor = ?raw.credential.flavor,
            "rpc header parsed",
        );
        Ok(RpcMessage {
            program: raw.program,
            procedure: raw.procedure,
            version: raw.version,
            cred: credential,
        })
    }

    /// Parses NFSv3 procedure arguments from the current frame.
    ///
    /// All arguments are parsed synchronously from the buffered head window,
    /// except for the opaque `WRITE` payload which is streamed via
    /// [`adapter_for_write`]. Unknown procedures are rejected earlier as an
    /// rpc-level [`rpc::Error::ProcedureMismatch`], so this match is exhaustive.
    async fn parse_nfs_proc(
        &mut self,
        procedure: NfsProc,
    ) -> parser::Result<NfsArguments<A::Buffer>> {
        let src = &mut self.reader;
        let args = match procedure {
            NfsProc::Null => NfsArguments::Null,
            NfsProc::GetAttr => NfsArguments::GetAttr(get_attr::args(src)?),
            NfsProc::SetAttr => NfsArguments::SetAttr(set_attr::args(src)?),
            NfsProc::LookUp => NfsArguments::LookUp(lookup::args(src)?),
            NfsProc::Access => NfsArguments::Access(access::args(src)?),
            NfsProc::ReadLink => NfsArguments::ReadLink(read_link::args(src)?),
            NfsProc::Read => {
                let (args, data) = adapter_for_read(&self.allocator, src).await?;
                NfsArguments::Read(args, data)
            }
            NfsProc::Write => NfsArguments::Write(adapter_for_write(&self.allocator, src).await?),
            NfsProc::Create => NfsArguments::Create(create::args(src)?),
            NfsProc::MkDir => NfsArguments::MkDir(mk_dir::args(src)?),
            NfsProc::SymLink => NfsArguments::SymLink(symlink::args(src)?),
            NfsProc::MkNod => NfsArguments::MkNod(mk_node::args(src)?),
            NfsProc::Remove => NfsArguments::Remove(remove::args(src)?),
            NfsProc::RmDir => NfsArguments::RmDir(rm_dir::args(src)?),
            NfsProc::Rename => NfsArguments::Rename(rename::args(src)?),
            NfsProc::Link => NfsArguments::Link(link::args(src)?),
            NfsProc::ReadDir => NfsArguments::ReadDir(read_dir::args(src)?),
            NfsProc::ReadDirPlus => NfsArguments::ReadDirPlus(read_dir_plus::args(src)?),
            NfsProc::FsStat => NfsArguments::FsStat(fs_stat::args(src)?),
            NfsProc::FsInfo => NfsArguments::FsInfo(fs_info::args(src)?),
            NfsProc::PathConf => NfsArguments::PathConf(path_conf::args(src)?),
            NfsProc::Commit => NfsArguments::Commit(commit::args(src)?),
        };
        Ok(args)
    }

    /// Parses MOUNT procedure arguments from the current frame.
    ///
    /// Unknown procedures are rejected earlier as an rpc-level
    /// [`rpc::Error::ProcedureMismatch`], so this match is exhaustive.
    fn parse_mount_proc(&mut self, procedure: MountProc) -> parser::Result<MountArguments> {
        let args = match procedure {
            MountProc::Null => MountArguments::Null,
            MountProc::Mnt => MountArguments::Mount(mount(&mut self.reader)?),
            MountProc::Dump => MountArguments::Dump,
            MountProc::Umnt => MountArguments::Unmount(unmount(&mut self.reader)?),
            MountProc::UmntAll => MountArguments::UnmountAll,
            MountProc::Export => MountArguments::Export,
        };
        Ok(args)
    }

    /// Parses NLM procedure arguments from the current frame.
    ///
    /// Unknown procedures are rejected earlier as an rpc-level
    /// [`rpc::Error::ProcedureMismatch`], so this match is exhaustive.
    fn parse_nlm_proc(&mut self, procedure: NlmProc) -> parser::Result<NlmArguments> {
        let args = match procedure {
            NlmProc::Null => NlmArguments::Null,
            NlmProc::Lock => NlmArguments::Lock(lock(&mut self.reader)?),
            NlmProc::Unlock => NlmArguments::Unlock(unlock(&mut self.reader)?),
            NlmProc::Test => NlmArguments::Test(test(&mut self.reader)?),
            NlmProc::Cancel => NlmArguments::Cancel(cancel(&mut self.reader)?),
        };
        Ok(args)
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
            Err(error) => {
                let error = map_parser_to_rpc(error);
                return Err(ErrorWrapper { xid: None, error: self.match_errors(error).await });
            }
        };

        if let Err(error) = self.read_call_prefix() {
            return Err(ErrorWrapper { xid: Some(xid), error: self.match_errors(error).await });
        }

        let raw = match self.read_rpc_header() {
            Ok(raw) => raw,
            Err(error) => {
                let error = map_parser_to_rpc(error);
                return Err(ErrorWrapper { xid: Some(xid), error: self.match_errors(error).await });
            }
        };

        let rpc_header = match self.validate_header(&raw) {
            Ok(header) => header,
            Err(error) => {
                return Err(ErrorWrapper { xid: Some(xid), error: self.match_errors(error).await })
            }
        };
        let proc = match self.parse_next_message_with_header(&rpc_header).await {
            Ok(proc) => proc,
            Err(error) => {
                return Err(ErrorWrapper { xid: Some(xid), error: self.match_errors(error).await })
            }
        };

        // finalize_parsing() is only called after successful header and procedure parsing; it is not run on error paths
        match self.finalize_parsing() {
            Ok(_) => Ok(ArgWrapper { header: RpcHeader { xid, cred: rpc_header.cred }, proc }),
            Err(error) => {
                let error = map_parser_to_rpc(error);
                Err(ErrorWrapper { xid: Some(xid), error: self.match_errors(error).await })
            }
        }
    }

    /// Dispatches the parsed header to the matching program parser and applies
    /// the semantic program/version/procedure validation.
    ///
    /// The version is checked before the procedure: procedure numbers are only
    /// defined within a program version (RFC 5531 §8.1), so a call to an
    /// unsupported version gets [`rpc::Error::ProgramVersionMismatch`] with the
    /// supported range rather than [`rpc::Error::ProcedureMismatch`].
    async fn parse_next_message_with_header(
        &mut self,
        head: &RpcMessage,
    ) -> rpc::Result<ProcArguments<A::Buffer>> {
        match head.program {
            NFS_PROGRAM => {
                if head.version != NFS_VERSION {
                    error!(
                        got = head.version,
                        expected = NFS_VERSION,
                        "rpc parse reject: nfs version mismatch",
                    );
                    return Err(rpc::Error::ProgramVersionMismatch(VersionMismatch {
                        low: NFS_VERSION,
                        high: NFS_VERSION,
                    }));
                }
                let procedure =
                    NfsProc::from_u32(head.procedure).ok_or(rpc::Error::ProcedureMismatch)?;
                let args = self.parse_nfs_proc(procedure).await.map_err(map_parser_to_rpc)?;
                Ok(ProcArguments::Nfs3(Box::new(args)))
            }
            MOUNT_PROGRAM => {
                if head.version != MOUNT_VERSION {
                    error!(
                        got = head.version,
                        expected = MOUNT_VERSION,
                        "rpc parse reject: mount version mismatch",
                    );
                    return Err(rpc::Error::ProgramVersionMismatch(VersionMismatch {
                        low: MOUNT_VERSION,
                        high: MOUNT_VERSION,
                    }));
                }
                let procedure =
                    MountProc::from_u32(head.procedure).ok_or(rpc::Error::ProcedureMismatch)?;
                let args = self.parse_mount_proc(procedure).map_err(map_parser_to_rpc)?;
                Ok(ProcArguments::Mount(Box::new(args)))
            }
            NLM_PROGRAM => {
                if head.version != NLM_VERSION {
                    error!(
                        got = head.version,
                        expected = NLM_VERSION,
                        "rpc parse reject: NLM version mismatch",
                    );
                    return Err(rpc::Error::ProgramVersionMismatch(VersionMismatch {
                        low: NLM_VERSION,
                        high: NLM_VERSION,
                    }));
                }
                let procedure =
                    NlmProc::from_u32(head.procedure).ok_or(rpc::Error::ProcedureMismatch)?;
                let args = self.parse_nlm_proc(procedure).map_err(map_parser_to_rpc)?;
                Ok(ProcArguments::Nlm4(Box::new(args)))
            }
            _ => {
                warn!(program = head.program, "rpc parse reject: unknown program");
                Err(rpc::Error::ProgramMismatch)
            }
        }
    }

    /// Finalizes parsing by validating that all frame data was consumed.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if validation passes, or an error if unparsed data
    /// remains in the frame (indicating a parsing bug or malformed message).
    fn finalize_parsing(&mut self) -> parser::Result<()> {
        if self.reader.frame_remaining() != 0 {
            return Err(parser::Error::IO(io::Error::new(
                ErrorKind::InvalidData,
                "Unparsed data remaining in frame",
            )));
        }
        Ok(())
    }

    /// Handles errors by discarding the current message.
    ///
    /// Discards the rest of the current frame after any error so the stream stays
    /// aligned to the next record. If the frame is fully consumed (`frame_remaining`
    /// is 0), discarding is a no-op.
    ///
    /// # Arguments
    ///
    /// * `error` - The error that occurred during parsing
    ///
    /// # Returns
    ///
    /// Returns the error, potentially replaced by an I/O error if discarding fails.
    async fn match_errors(&mut self, error: rpc::Error) -> rpc::Error {
        proc_nested_errors(error, self.discard_current_message()).await
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
    async fn discard_current_message(&mut self) -> rpc::Result<()> {
        self.reader.discard_rest_of_frame().await.map_err(rpc::Error::IO)
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
/// Returns the parsed [`vfs::write::Args`] with the allocated data, or an error if:
/// - Parsing fails
/// - The declared data length does not fit into the frame
/// - Memory allocation fails
/// - Reading the data fails
async fn adapter_for_write<A, S>(
    alloc: &Arc<A>,
    reader: &mut FrameReader<S>,
) -> parser::Result<vfs::write::Args<A::Buffer>>
where
    A: Allocator,
    S: AsyncRead + Unpin,
{
    // Parse arguments for the WRITE procedure.
    let part_arg = write::args(reader)?;
    let size = u32_as_usize(reader)?;

    // Calculate the necessary padding to maintain XDR alignment.
    let padding = padding_len(size);

    // The opaque data with its padding must lie within the current frame;
    // otherwise, the declared length is bogus and reading it would consume
    // bytes of the next message, misaligning the stream.
    let bounds_error = || {
        parser::Error::IO(io::Error::new(
            ErrorKind::InvalidData,
            "WRITE data length exceeds frame size",
        ))
    };
    let padded = size.checked_add(padding).ok_or_else(bounds_error)?;
    if padded > reader.frame_remaining() {
        return Err(bounds_error());
    }

    // Fill the allocated buffer chunk by chunk; `read_body_exact` consumes the
    // buffered window first and reads the rest directly from the socket.
    let mut buffer_data = match NonZeroUsize::new(size) {
        Some(non_zero_size) => alloc.allocate(non_zero_size).await.ok_or_else(|| {
            parser::Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory"))
        })?,
        None => A::Buffer::empty(),
    };

    let mut left = size;
    for chunk in buffer_data.chunks_mut() {
        if left == 0 {
            break;
        }
        let take = min(chunk.len(), left);
        reader.read_body_exact(&mut chunk[..take]).await.map_err(parser::Error::IO)?;
        left -= take;
    }
    if left != 0 {
        return Err(parser::Error::IO(io::Error::new(
            ErrorKind::InvalidInput,
            "allocated buffer is smaller than WRITE data",
        )));
    }

    // Discard any trailing padding bytes after the data.
    reader.discard_body(padding).await.map_err(parser::Error::IO)?;
    Ok(vfs::write::Args {
        file: part_arg.file,
        offset: part_arg.offset,
        size: part_arg.size,
        stable: part_arg.stable,
        data: buffer_data,
    })
}

/// Special adapter for parsing READ procedure arguments.
///
/// Unlike WRITE, the READ request carries no opaque data on the wire; the
/// buffer allocated here is the server-side *output* buffer that the backend
/// fills with the read result. Allocating it on the read side keeps a single
/// allocator serving both READ and WRITE and removes the allocator from the
/// VFS task.
///
/// # Arguments
///
/// * `alloc` - The allocator to use for allocating the read output buffer
/// * `reader` - The frame reader to read the fixed arguments from
///
/// # Returns
///
/// Returns the parsed [`vfs::read::Args`] together with the allocated output
/// buffer, or an error if parsing fails or memory allocation fails. A zero-byte
/// read yields an empty buffer without touching the allocator.
async fn adapter_for_read<A, S>(
    alloc: &Arc<A>,
    reader: &mut FrameReader<S>,
) -> parser::Result<(vfs::read::Args, A::Buffer)>
where
    A: Allocator,
    S: AsyncRead + Unpin,
{
    let args = read::args(reader)?;

    let data = if args.count == 0 {
        A::Buffer::empty()
    } else {
        let size = NonZeroUsize::new(args.count as usize).unwrap();
        alloc.allocate(size).await.ok_or_else(|| {
            parser::Error::IO(io::Error::new(ErrorKind::OutOfMemory, "cannot allocate memory"))
        })?
    };

    Ok((args, data))
}
