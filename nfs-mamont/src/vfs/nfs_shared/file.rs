use std::io;

/// A validated wrapper around a `String` representing a name.
///
/// [`Name`] ensures that the inner string does not exceed `N`.
/// It provides safe construction, accessors, and conversion back into the
/// owned inner `String`.
#[derive(Debug, PartialEq, Clone)]
pub struct Name<const N: usize>(String);

impl<const N: usize> Name<N> {
    /// Creates a new [`Name`] after validating its length.
    ///
    /// # Errors
    ///
    /// Returns an error if `name.len() > N` or empty name.
    pub fn new(name: String) -> io::Result<Self> {
        if name.len() > N {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name too long"));
        }
        if name.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "name is empty"));
        }
        if name.contains('/') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "name contains path separator",
            ));
        }
        Ok(Name(name))
    }

    /// Consumes the wrapper and returns the inner [`String`].
    #[inline]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Returns the inner name as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
