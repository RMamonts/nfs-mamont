use std::io;

pub fn io_other<T, E>(message: E) -> io::Result<T>
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    Err(io::Error::other(message))
}
