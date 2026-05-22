use std::cmp::min;

use monoio::buf::IoBufMut;
use monoio::io::AsyncReadRent;
use monoio::BufResult;

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

impl AsyncReadRent for MockSocket {
    fn read<T: IoBufMut>(&mut self, mut buf: T) -> impl std::future::Future<Output = BufResult<usize, T>> {
        let to_read = if self.position >= self.data.len() {
            0
        } else {
            let remaining_data = self.data.len() - self.position;
            let to_read = min(SEPARATE, min(buf.bytes_total(), remaining_data));
            if to_read > 0 {
                unsafe {
                    let dst = buf.write_ptr();
                    std::ptr::copy_nonoverlapping(
                        self.data[self.position..].as_ptr(),
                        dst,
                        to_read,
                    );
                    buf.set_init(to_read);
                }
                self.position += to_read;
            }
            to_read
        };
        async move { (Ok(to_read), buf) }
    }

    fn readv<T: monoio::buf::IoVecBufMut>(&mut self, buf: T) -> impl std::future::Future<Output = BufResult<usize, T>> {
        async move { (Ok(0), buf) }
    }
}
