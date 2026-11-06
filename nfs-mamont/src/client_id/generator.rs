/// An internal incrementing 64-bit sequence, independent of registrations.
#[derive(Default)]
pub struct Generator {
    last: u64,
}

impl Generator {
    pub const fn new() -> Self {
        Self { last: 0 }
    }

    pub fn next(&mut self) -> u64 {
        self.last = self.last.wrapping_add(1);
        self.last
    }
}
