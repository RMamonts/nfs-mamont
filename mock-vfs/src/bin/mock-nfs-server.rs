//! `mock-nfs-server`: a standalone NFSv3 server backed by the in-memory mock.
//!
//! Use it as the target of `nfs-bench --target` on another machine, or mount it with a
//! real NFS client to look at the server from the outside.

use std::net::SocketAddr;
use std::process::ExitCode;

use mock_vfs::cli::{self, Args};
use mock_vfs::server::{Server, ServerConfig};

const USAGE_HEAD: &str = "\
mock-nfs-server: NFSv3 server backed by an in-memory mock file system

Usage: mock-nfs-server [OPTIONS]

Options:
  --bind <addr>          Address to listen on (default: 0.0.0.0:2049)";

const USAGE_TAIL: &str = "  -h, --help             Show this help

Logging is controlled by RUST_LOG (default: warn).

Mount with the Linux kernel client (the server registers with rpcbind when one runs,
otherwise pass the ports explicitly):
  mount -t nfs -o vers=3,proto=tcp,port=2049,mountport=2049,nolock <host>:/ /mnt";

fn usage() -> String {
    format!("{USAGE_HEAD}\n{}\n{USAGE_TAIL}", cli::SERVER_FLAGS_HELP)
}

enum Command {
    Run(ServerConfig),
    Help,
}

fn parse_args(mut args: Args) -> cli::Result<Command> {
    let mut config =
        ServerConfig { bind: SocketAddr::from(([0, 0, 0, 0], 2049)), ..ServerConfig::default() };

    while let Some(flag) = args.next_flag() {
        match flag.as_str() {
            "--bind" => config.bind = args.parse(&flag)?,
            "-h" | "--help" => return Ok(Command::Help),
            other => {
                if !cli::apply_server_flag(&mut config, other, &mut args)? {
                    return Err(format!("unknown argument '{other}'"));
                }
            }
        }
    }
    config.validate().map_err(|err| err.to_string())?;

    Ok(Command::Run(config))
}

#[tokio::main]
async fn main() -> ExitCode {
    let config = match parse_args(Args::from_env()) {
        Ok(Command::Run(config)) => config,
        Ok(Command::Help) => {
            println!("{}", usage());
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{}", usage());
            return ExitCode::from(2);
        }
    };
    cli::init_tracing("warn");

    let latency = config.vfs.latency;
    let server = match Server::start(config.clone()).await {
        Ok(server) => server,
        Err(err) => {
            eprintln!("error: cannot start the server: {err}");
            return ExitCode::FAILURE;
        }
    };

    eprintln!("mock-nfs-server listening on {}", server.addr());
    eprintln!(
        "export {}: {} files and {} dirs per directory, file size {}, max I/O {}",
        config.export,
        config.vfs.files_per_dir,
        config.vfs.dirs_per_dir,
        cli::format_size(config.vfs.file_size),
        cli::format_size(u64::from(config.vfs.max_io)),
    );
    eprintln!(
        "backend latency {} ± {}, bandwidth {}, {} workers, allocator {} x {}",
        cli::format_duration(latency.base),
        cli::format_duration(latency.jitter),
        latency
            .bandwidth
            .map_or("unlimited".to_string(), |b| format!("{}/s", cli::format_size(b.get()))),
        config.concurrency,
        config.buf_count,
        cli::format_size(config.buf_size.get() as u64),
    );
    eprintln!("press Ctrl+C to stop");

    match server.wait().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: server stopped: {err}");
            ExitCode::FAILURE
        }
    }
}
