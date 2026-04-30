use std::num::NonZeroUsize;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path of directory to mirror
    #[arg(short, long)]
    pub path: String,

    /// Address with port to listen
    #[arg(short, long)]
    pub bind: String,

    /// Size of read buffer in bytes
    #[arg(long, default_value_t = NonZeroUsize::new(1048576).unwrap())]
    pub read_buffer_size: NonZeroUsize,

    /// Number of read buffers
    #[arg(long, default_value_t = NonZeroUsize::new(512).unwrap())]
    pub read_buffer_count: NonZeroUsize,

    /// Size of write buffer in bytes
    #[arg(short, long, default_value_t = NonZeroUsize::new(1048576).unwrap())]
    pub write_buffer_size: NonZeroUsize,

    /// Number of write buffers
    #[arg(long, default_value_t = NonZeroUsize::new(512).unwrap())]
    pub write_buffer_count: NonZeroUsize,

    /// Size of VFS pool
    #[arg(long, default_value_t = NonZeroUsize::new(10).unwrap())]
    pub vfs_pool_size: NonZeroUsize,
}
