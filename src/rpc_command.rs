use std::io;
use std::io::Cursor;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;
use tracing::debug;

/// Max size of Record Marking Standard fragment
const MAX_RM_FRAGMENT_SIZE: usize = (1 << 31) - 1;
/// Constant to set last bit in Record Marking Standard
const LAST_FG_MASK: u32 = 1 << 31;

/// RPC command type with context
#[derive(Debug)]
pub struct RpcCommand {
    /// RPC message data
    pub data: Cursor<Vec<u8>>,
}

/// Parses a fragment header into its components.
fn parse_header(arg: u32) -> (bool, usize) {
    ((arg & LAST_FG_MASK) > 0, arg as usize & MAX_RM_FRAGMENT_SIZE)
}

impl RpcCommand {
    /// Reads a complete RPC command from a TCP socket using a Record Marking Protocol.
    ///
    /// This method implements a custom fragmentation protocol where RPC commands can be
    /// split across multiple fragments. Each fragment is preceded by a 4-byte header
    /// that indicates whether it's the last fragment and the length of the fragment data.
    pub async fn read_command_from_socket(&mut self, socket: &mut OwnedReadHalf) -> io::Result<()> {
        let mut header_buf = [0_u8; 4];
        let mut start_offset = 0;
        loop {
            socket.read_exact(&mut header_buf).await?;
            let fragment_header = u32::from_be_bytes(header_buf);
            let (is_last, length) = parse_header(fragment_header);
            debug!("Reading fragment length:{}, last:{}", length, is_last);
            let cur_len = self.data.get_ref().len();
            self.data.get_mut().resize(cur_len + length, 0);
            socket.read_exact(&mut self.data.get_mut()[start_offset..]).await?;
            debug!("Finishing Reading fragment length:{}, last:{}", length, is_last);
            if is_last {
                break;
            }
            start_offset += length;
        }
        Ok(())
    }
}
