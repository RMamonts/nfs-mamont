pub struct Random(u32);

impl Default for Random {
    fn default() -> Self {
        Self(1)
    }
}

impl Random {
    pub fn next(&mut self) -> u32 {
        let next = self.0;

        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;

        next
    }
}
