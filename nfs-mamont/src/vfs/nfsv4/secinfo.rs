//! Defines the NFSv4 SECINFO operation types, RFC 7530 §16.31.

use crate::vfs::nfsv4::file;

/// The RPCSEC_GSS service (`rpc_gss_svc_t`), RFC 2203.
pub enum RpcGssService {
    /// RPC_GSS_SVC_NONE - no service.
    None,
    /// RPC_GSS_SVC_INTEGRITY - integrity service.
    Integrity,
    /// RPC_GSS_SVC_PRIVACY - privacy service.
    Privacy,
}

/// RPCSEC_GSS security quadruple (`rpcsec_gss_info`).
pub struct GssInfo {
    /// Security mechanism object identifier (`sec_oid4`).
    /// TODO: consider amount limitations
    pub oid: Vec<u8>,
    /// Quality of protection (`qop4`).
    pub qop: u32,
    /// Service type used for the security mechanism.
    pub service: RpcGssService,
}

/// A security flavor and, for RPCSEC_GSS, the associated security quadruple
/// (`secinfo4`).
pub struct SecInfo {
    /// RPC authentication flavor (`AUTH_NONE`, `AUTH_SYS`, or `RPCSEC_GSS`).
    pub flavor: u32,
    /// Security quadruple, present only when `flavor` is `RPCSEC_GSS`.
    pub info: Option<GssInfo>,
}

/// SECINFO arguments (`SECINFO4args`).
pub struct Args {
    /// Name of the object for which the security mechanisms are requested.
    pub name: file::Name,
}

/// Success result (`SECINFO4resok`).
pub struct Success {
    /// List of available security mechanisms, in server preference order.
    /// TODO: consider amount limitations
    pub resok: Vec<SecInfo>,
}

/// Fail result (`SECINFO4res`).
pub struct Fail {
    /// Error on failure.
    pub error: super::Error,
}
