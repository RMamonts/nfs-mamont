use std::io;
use std::rc::Rc;

use async_trait::async_trait;
use tokio_uring::net::TcpStream;

#[async_trait(?Send)]
pub trait RpcRead {
    async fn read_some(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    async fn read_exact_into(&mut self, buf: &mut [u8]) -> io::Result<()>;
}

#[async_trait(?Send)]
pub trait RpcWrite {
    async fn write_all_buf(&mut self, buf: &[u8]) -> io::Result<()>;
}

pub struct UringReadHalf {
    stream: Rc<TcpStream>,
}

impl UringReadHalf {
    pub fn new(stream: Rc<TcpStream>) -> Self {
        Self { stream }
    }
}

pub struct UringWriteHalf {
    stream: Rc<TcpStream>,
}

impl UringWriteHalf {
    pub fn new(stream: Rc<TcpStream>) -> Self {
        Self { stream }
    }
}

#[async_trait(?Send)]
impl RpcRead for UringReadHalf {
    async fn read_some(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let (result, owned) = self.stream.read(vec![0u8; buf.len()]).await;
        let n = result?;
        buf[..n].copy_from_slice(&owned[..n]);
        Ok(n)
    }

    async fn read_exact_into(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let mut offset = 0usize;

        while offset < buf.len() {
            let n = self.read_some(&mut buf[offset..]).await?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Connection closed",
                ));
            }
            offset += n;
        }

        Ok(())
    }
}

#[async_trait(?Send)]
impl RpcWrite for UringWriteHalf {
    async fn write_all_buf(&mut self, buf: &[u8]) -> io::Result<()> {
        let (result, _) = self.stream.write_all(buf.to_vec()).await;
        result
    }
}
