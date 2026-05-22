use monoio::io::Splitable;
use monoio::net::TcpStream;
use async_channel;
use tracing::error;

use crate::allocator::Allocator;
use crate::context::ServerContext;
use crate::task::global::mount::MountCommand;
use crate::task::global::nlm::NlmCommand;
use crate::task::ProcReply;
use crate::vfs::Vfs;

mod read;
mod write;

pub async fn new<A, V>(
    socket: TcpStream,
    mount_sender: async_channel::Sender<MountCommand>,
    nlm_sender: async_channel::Sender<NlmCommand>,
    context: &ServerContext<A, V>,
) where
    A: Allocator + Send + Sync + 'static,
    V: Vfs + Send + Sync + 'static,
{
    let peer_addr = match socket.peer_addr() {
        Ok(addr) => addr,
        Err(err) => {
            error!(error=%err, "failed to determine peer address");
            return;
        }
    };

    let (result_sender, result_receiver) = async_channel::unbounded::<ProcReply>();

    let (read_socket, write_socket) = socket.into_split();

    read::ReadTask::new(
        read_socket,
        peer_addr,
        mount_sender,
        nlm_sender,
        result_sender.clone(),
        context.get_write_allocator(),
        context.get_vfs_pool().sender(),
    )
    .spawn();

    write::WriteTask::new(write_socket, result_receiver).spawn();
}
