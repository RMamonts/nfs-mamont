use num_derive::{FromPrimitive, ToPrimitive};

pub const NLM_PROGRAM: u32 = 100021;
pub const NLM_VERSION: u32 = 4;

/// NLMv4 procedures (RFC 1813 §6.2, Appendix II) as a type-safe enum.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ToPrimitive, FromPrimitive)]
pub enum NlmProc {
    Null = 0,
    Test = 1,
    Lock = 2,
    Cancel = 3,
    Unlock = 4,
}

pub const LM_MAXSTRLEN: usize = 1024;
pub const OPAQUE_HANDLE_SIZE: usize = 1024;
