//! Constants defined by the Network Lock Manager protocol version 4
//! (X/Open CAE Specification "Protocols for X/Open PC Interworking: XNFS", version 3W).
//!
//! Version 4 changes over version 3 are summarized in
//! [RFC 1813 (appendix II)](https://datatracker.ietf.org/doc/html/rfc1813#section-6.0).

/// RPC program number of the NLM service.
/// ([RFC 1813, section 6.1.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.1.2))
pub const NLM_PROGRAM: u32 = 100021;
/// Version of the NLM protocol implemented by the server.
/// ([RFC 1813, section 6.1.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.1.2))
pub const NLM_VERSION: u32 = 4;

/// Maximum length in bytes of a string such as a caller or host name.
pub const LM_MAXSTRLEN: usize = 1024;
/// Maximum size in bytes of a `netobj` opaque value such as a lock owner or cookie (`MAXNETOBJ_SZ`).
pub const OPAQUE_HANDLE_SIZE: usize = 1024;

/// Do nothing, used to check that the server responds.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_NULL: u32 = 0;
/// Test whether a lock could be granted.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_TEST: u32 = 1;
/// Acquire a lock.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_LOCK: u32 = 2;
/// Cancel a blocked lock request.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_CANCEL: u32 = 3;
/// Release a lock.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_UNLOCK: u32 = 4;
/// Server-to-client callback notifying that a blocked lock was granted.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_GRANTED: u32 = 5;
/// Asynchronous message form of [`NLMPROC4_TEST`]; the result is sent with [`NLMPROC4_TEST_RES`].
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_TEST_MSG: u32 = 6;
/// Asynchronous message form of [`NLMPROC4_LOCK`]; the result is sent with [`NLMPROC4_LOCK_RES`].
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_LOCK_MSG: u32 = 7;
/// Asynchronous message form of [`NLMPROC4_CANCEL`]; the result is sent with [`NLMPROC4_CANCEL_RES`].
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_CANCEL_MSG: u32 = 8;
/// Asynchronous message form of [`NLMPROC4_UNLOCK`]; the result is sent with [`NLMPROC4_UNLOCK_RES`].
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_UNLOCK_MSG: u32 = 9;
/// Asynchronous message form of [`NLMPROC4_GRANTED`]; the result is sent with [`NLMPROC4_GRANTED_RES`].
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_GRANTED_MSG: u32 = 10;
/// Result of a [`NLMPROC4_TEST_MSG`] request.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_TEST_RES: u32 = 11;
/// Result of a [`NLMPROC4_LOCK_MSG`] request.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_LOCK_RES: u32 = 12;
/// Result of a [`NLMPROC4_CANCEL_MSG`] request.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_CANCEL_RES: u32 = 13;
/// Result of a [`NLMPROC4_UNLOCK_MSG`] request.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_UNLOCK_RES: u32 = 14;
/// Result of a [`NLMPROC4_GRANTED_MSG`] request.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_GRANTED_RES: u32 = 15;
/// Create a DOS-style file share reservation.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_SHARE: u32 = 20;
/// Release a DOS-style file share reservation.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_UNSHARE: u32 = 21;
/// Acquire a lock without monitoring the client through the status monitor.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_NM_LOCK: u32 = 22;
/// Release all locks held by a client, sent after the client host reboots.
/// ([RFC 1813, section 6.2](https://datatracker.ietf.org/doc/html/rfc1813#section-6.2))
pub const NLMPROC4_FREE_ALL: u32 = 23;
