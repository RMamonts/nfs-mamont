//! Defines the NFSv4 COMMIT operation types, RFC 7530 §16.3.

/// COMMIT arguments (`COMMIT4args`).
pub struct Args {
    /// Position within the file at which the flush is to begin. A value of
    /// `0` means to flush starting at the beginning of the file.
    pub offset: u64,
    /// Number of bytes of data to flush. If `count` is `0`, a flush from
    /// the offset to the end of the file is done.
    pub count: u32,
}

/// Success result (`COMMIT4resok`).
pub struct Success {
    /// Write verifier provided by the server.
    pub writeverf: super::Verifier,
}

/// Fail result (`COMMIT4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
