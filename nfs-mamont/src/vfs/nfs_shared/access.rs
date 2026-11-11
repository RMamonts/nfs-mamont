/// Mask of access rights.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Mask(u32);

impl Mask {
    pub const READ: u32 = 0x0001;
    pub const LOOKUP: u32 = 0x0002;
    pub const MODIFY: u32 = 0x0004;
    pub const EXTEND: u32 = 0x0008;
    pub const DELETE: u32 = 0x0010;
    pub const EXECUTE: u32 = 0x0020;

    pub const ALL: u32 =
        Self::READ | Self::LOOKUP | Self::MODIFY | Self::EXTEND | Self::DELETE | Self::EXECUTE;

    pub fn from_wire(raw: u32) -> Self {
        Self(raw & Self::ALL)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, flag: u32) -> bool {
        self.0 & flag == flag
    }
}
