//! Defines Mount version 3 [`Export`] interface (Procedure 5).
//!
//! as defined in RFC 1813 section 5.2.5.
//! <https://datatracker.ietf.org/doc/html/rfc1813#section-5.2.5>.

use async_trait::async_trait;

use super::ExportEntry;

/// Success result.
pub struct Success {
    /// Vector of export entries, each containing an exported
    /// directory and a vector of clients that are allowed
    /// to mount the specified directory.
    pub exports: Vec<ExportEntry>,
}

pub type Result = std::result::Result<Success, ()>;

/// Defines callback to pass [`Export::export`] result into.
#[async_trait]
pub trait Promise {
    async fn keep(result: Result);
}

#[async_trait]
pub trait Export {
    /// Retrieves a vector of all the exported file systems and which clients
    /// are allowed to mount each one.
    ///
    /// There are no MOUNT protocol errors which can be returned from this procedure.
    async fn export(&self, promise: impl Promise);
}
