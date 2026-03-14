use std::path::PathBuf;
use std::sync::Arc;

pub mod fs;
pub mod fs_map;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);
    let export_root = std::fs::canonicalize(&path).unwrap_or_else(|error| {
        panic!("failed to resolve export root {}: {error}", path.display())
    });
    let metadata = std::fs::metadata(&export_root).unwrap_or_else(|error| {
        panic!("failed to stat export root {}: {error}", export_root.display())
    });
    assert!(metadata.is_dir(), "export root {} must be a directory", export_root.display());

    let _fs = Arc::new(fs::MirrorFS::new(export_root.clone()));
}
