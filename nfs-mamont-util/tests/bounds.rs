//! Compile-time check that the ready-made implementations satisfy the bounds
//! [`nfs_mamont::handle_forever`] puts on its MOUNT, NLM and allocator parameters.

use nfs_mamont::mount::Mount;
use nfs_mamont::nlm::Nlm;
use nfs_mamont::{Allocator, Buffer};
use nfs_mamont_util::{MountService, NlmService, PoolAllocator, Slice};

fn mount<M: Mount + Send + Sync + 'static>() {}

fn nlm<N: Nlm + Send + Sync + 'static>() {}

fn allocator<A, B>()
where
    A: Allocator<Buffer = B> + Send + Sync + 'static,
    B: Buffer + 'static,
{
}

#[test]
fn utils_types_fit_handle_forever() {
    mount::<MountService>();
    nlm::<NlmService>();
    allocator::<PoolAllocator, Slice>();
}
