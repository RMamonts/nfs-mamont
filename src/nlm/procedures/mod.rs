pub mod test_proc;

/// NLMv4 RPC procedure numbers.
///
/// Corresponds to NLM version 4 protocol as defined in RFC 1813.
pub enum Nlm4Procedures {
    /// NLM4_NULL — no operation, used to test server availability.
    Null = 0,
    /// NLM4_TEST — test for a lock.
    Test = 1,
    /// NLM4_LOCK — request a lock.
    Lock = 2,
    /// NLM4_CANCEL — cancel an outstanding lock request.
    Cancel = 3,
    /// NLM4_UNLOCK — release a lock.
    Unlock = 4,
}
