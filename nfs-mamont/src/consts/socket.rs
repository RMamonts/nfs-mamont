/// Pause after a non-fatal `accept` error (e.g. out of file descriptors) before retrying.
pub const ACCEPT_ERROR_BACKOFF: std::time::Duration = std::time::Duration::from_millis(50);

/// `accept` on a closed listening socket.
pub const ACCEPT_ERROR_EBADF: i32 = 9;

/// `accept` on an invalid socket.
pub const ACCEPT_ERROR_EINVAL: i32 = 22;

/// `accept` on something that is not a socket.
pub const ACCEPT_ERROR_ENOTSOCK: i32 = 88;
