use std::cmp::min;

use async_trait::async_trait;

use crate::parser::read_buffer::ReadSource;

const SEPARATE: usize = 15;

pub struct MockSocket {
    data: Vec<u8>,
    position: usize,
}

impl MockSocket {
    pub fn new(buf: &[u8]) -> Self {
        MockSocket { data: buf.to_vec(), position: 0 }
    }
}

#[async_trait(?Send)]
impl ReadSource for MockSocket {
    async fn read_into(&mut self, dest: &mut [u8]) -> std::io::Result<usize> {
        if self.position >= self.data.len() {
            return Ok(0);
        }

        let remaining_data = self.data.len() - self.position;
        let to_read = min(SEPARATE, min(dest.len(), remaining_data));

        if to_read > 0 {
            let start = self.position;
            let end = self.position + to_read;
            dest[..to_read].copy_from_slice(&self.data[start..end]);
            self.position += to_read;
        }

        Ok(to_read)
    }

    async fn read_exact_into(&mut self, dest: &mut [u8]) -> std::io::Result<()> {
        let mut total = 0;
        while total < dest.len() {
            let n = self.read_into(&mut dest[total..]).await?;
            if n == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "mock socket EOF",
                ));
            }
            total += n;
        }
        Ok(())
    }
}
