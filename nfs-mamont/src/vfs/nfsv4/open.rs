//! Defines the NFSv4 OPEN operation types, RFC 7530 §16.16.

use std::io;

use crate::consts::nfsv4::NFS4_MAX_ACE_LEN;

use super::{commit, file, lock};

/// Concept of how the file is to be opened (opentype4), RFC 7530 §16.16.2.
pub enum OpenType {
    /// OPEN4_NOCREATE - open an existing file.
    NoCreate,
    /// OPEN4_CREATE - open a file, creating it if it does not already exist.
    Create,
}

/// Mode of file creation on OPEN (createmode4), RFC 7530 §16.16.2.
pub enum CreateMode {
    /// UNCHECKED4 - create a file without checking its existence.
    Unchecked,
    /// GUARDED4 - create a file only if it does not already exist.
    Guarded,
    /// EXCLUSIVE4 - create a file exclusively, requiring a verifier.
    Exclusive,
}

/// How a file is created on OPEN (createhow4), RFC 7530 §16.16.2.
pub enum CreateHow {
    /// UNCHECKED4 - create with the given attributes.
    Unchecked(file::Attr),
    /// GUARDED4 - create with the given attributes, failing if it exists.
    Guarded(file::Attr),
    /// EXCLUSIVE4 - create exclusively using the given verifier.
    Exclusive(commit::Verifier),
}

/// Whether and how a file is created on open (openflag4), RFC 7530 §16.16.2.
pub enum OpenFlag {
    /// OPEN4_NOCREATE - do not create the file.
    NoCreate,
    /// OPEN4_CREATE - create the file using the given [`CreateHow`].
    Create(CreateHow),
}

/// The owner of open state (`open_owner4`), RFC 7530 §2.2.13.
pub struct OpenOwner {
    /// Client that owns the open state.
    pub clientid: file::ClientId,
    /// Opaque identifier of the open owner.
    pub owner: lock::LockId,
}

/// Type of delegation granted on open (`open_delegation_type4`).
pub enum DelegationType {
    /// OPEN_DELEGATE_NONE - no delegation is granted.
    None,
    /// OPEN_DELEGATE_READ - a read delegation is granted.
    Read,
    /// OPEN_DELEGATE_WRITE - a write delegation is granted.
    Write,
}

/// How the client claims the right to open a file (`open_claim_type4`).
pub enum ClaimType {
    /// CLAIM_NULL - no special rights to the file.
    Null,
    /// CLAIM_PREVIOUS - the right was established by an open before reboot.
    Previous,
    /// CLAIM_DELEGATE_CUR - the right is based on a current delegation.
    DelegateCur,
    /// CLAIM_DELEGATE_PREV - the right is based on a pre-reboot delegation.
    DelegatePrev,
}

/// The delegation-based current claim (`open_claim_delegate_cur4`).
pub struct ClaimDelegateCur {
    /// Stateid of the delegation.
    pub delegate_stateid: file::StateId,
    /// Name of the file being claimed.
    pub file: file::Name,
}

pub struct OwnerName(String);

impl OwnerName {
    /// Creates a new [`OwnerName`] after validating its length.
    pub fn new(name: String) -> io::Result<Self> {
        if name.len() > NFS4_MAX_ACE_LEN {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name too long"));
        }
        if name.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name is empty"));
        }
        Ok(OwnerName(name))
    }

    /// Consumes the wrapper and returns the inner [`String`].
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Returns the inner name as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A unit of access control information (`nfsace4`), RFC 7530 §16.16.3.
pub struct Ace {
    /// Subject of the access control entry.
    pub who: OwnerName,
    /// Bitmask of access permissions.
    pub access_mask: u32,
    /// Type of the access control entry.
    pub ace_type: u32,
    /// Flags of the access control entry.
    pub flag: u32,
}

/// The kind of limit used for a write delegation (`limit_by4`).
pub enum LimitBy {
    /// NFS_LIMIT_SIZE - limit is expressed as a file size.
    Size,
    /// NFS_LIMIT_BLOCKS - limit is expressed as a number of blocks.
    Blocks,
}

/// A block-based space limit (`nfs_modified_limit4`).
pub struct ModifiedLimit {
    /// Number of blocks.
    pub num_blocks: u32,
    /// Bytes per block.
    pub bytes_per_block: u32,
}

/// A space limit for a write delegation (`nfs_space_limit4`).
pub enum SpaceLimit {
    /// NFS_LIMIT_SIZE - limit given as a file size in bytes.
    Size(u64),
    /// NFS_LIMIT_BLOCKS - limit given as a number of blocks.
    Blocks(ModifiedLimit),
}

/// How the file is claimed on open (`open_claim4`), RFC 7530 §16.16.2.
pub enum OpenClaim {
    /// CLAIM_NULL - ordinary open of the file by name.
    Null(file::Name),
    /// CLAIM_PREVIOUS - reclaim an open established before a reboot.
    Previous(DelegationType),
    /// CLAIM_DELEGATE_CUR - open based on a current delegation.
    DelegateCur(ClaimDelegateCur),
    /// CLAIM_DELEGATE_PREV - open based on a pre-reboot delegation.
    DelegatePrev(file::Name),
}

/// OPEN arguments (`OPEN4args`).
pub struct Args {
    /// Sequence id providing for the correct ordering of the operation.
    pub seqid: file::SeqId,
    /// Bitmask of the access operations the client requests.
    pub share_access: u32,
    /// Bitmask of the access operations the client wants to deny.
    pub share_deny: u32,
    /// Owner of the open state.
    pub owner: OpenOwner,
    /// Whether and how the file is to be created and opened.
    pub openhow: OpenFlag,
    /// How the client claims the right to open the file.
    pub claim: OpenClaim,
}

/// A read delegation returned on open (`open_read_delegation4`).
pub struct ReadDelegation {
    /// Stateid for the delegation.
    pub stateid: file::StateId,
    /// Whether the delegation is pre-recalled during reclaim.
    pub recall: bool,
    /// Permissions for which an ACCESS call is not needed to open for read.
    pub permissions: Vec<Ace>,
}

/// A write delegation returned on open (`open_write_delegation4`).
pub struct WriteDelegation {
    /// Stateid for the delegation.
    pub stateid: file::StateId,
    /// Whether the delegation is pre-recalled during reclaim.
    pub recall: bool,
    /// Condition the client must check to decide whether to flush on close.
    pub space_limit: SpaceLimit,
    /// Permissions for which an ACCESS call is not needed as part of a
    /// delegated open.
    pub permissions: Vec<Ace>,
}

/// Delegation granted on open (`open_delegation4`), RFC 7530 §16.16.3.
pub enum OpenDelegation {
    /// OPEN_DELEGATE_NONE - no delegation.
    None,
    /// OPEN_DELEGATE_READ - a read delegation.
    Read(ReadDelegation),
    /// OPEN_DELEGATE_WRITE - a write delegation.
    Write(WriteDelegation),
}

/// Success result (`OPEN4resok`).
pub struct Success {
    /// Stateid for the open.
    pub stateid: file::StateId,
    /// Change information for the directory.
    pub cinfo: file::ChangeInfo,
    /// Result flags.
    pub rflags: u32,
    /// Bitmask of attributes set when creating the file.
    pub attrset: file::Bitmap,
    /// Delegation, if any, granted on the open.
    pub delegation: OpenDelegation,
}

/// Fail result (`OPEN4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
