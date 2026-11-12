/// Maximum size of a file handle (`nfs_fh4`), in bytes.
///
/// The NFSv4 specification (RFC 7530 §2.1) defines the `nfs_fh4` type as
/// `opaque nfs_fh4<NFS4_FHSIZE>` without fixing a numeric value in the text.
/// Implementations conventionally use 128 bytes; this crate follows suit.
pub const NFS4_FHSIZE: usize = 128;

/// Size of the `other` field of a [`crate::vfs::nfsv4::file::StateId`], in bytes.
pub const NFS4_OTHER_SIZE: usize = 12;
