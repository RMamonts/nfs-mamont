/// Identifies a point in the directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cookie(u64);

impl Cookie {
    /// Creates a new `Cookie` instance.
    ///
    /// ### Arguments
    /// * `val` - A 64-bit unsigned integer representing a specific point
    ///   in the directory, as returned by the server in a directory entry.
    ///
    /// ### Returns
    /// * A `Cookie` wrapping the provided value.
    pub fn new(val: u64) -> Self {
        Self(val)
    }

    /// Retrieves the raw 64-bit value of the cookie.
    ///
    /// ### Returns
    /// * The internal `u64` value.
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Checks if the cookie is the initial zero value.
    ///
    /// ### Returns
    /// * `true` if the value is 0. In the first `READDIR` request for a directory,
    ///   this should be set to 0 to start reading from the first entry.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}
