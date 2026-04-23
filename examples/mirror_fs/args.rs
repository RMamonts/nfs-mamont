use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "mirrorfs", about = "NFS mirror filesystem server")]
pub struct Args {
    /// Path to TOML configuration file
    #[clap(short = 'c', long = "config", value_hint = clap::ValueHint::AnyPath)]
    pub config_path: Option<PathBuf>,

    /// IP address and TCP port to listen to.
    #[arg(short, long, default_value = "0.0.0.0:2049")]
    pub addr: SocketAddr,
}
