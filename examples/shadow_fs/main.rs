use std::path::PathBuf;

pub mod fs;

#[tokio::main]
async fn main() {
    let path = std::env::args().nth(1).expect("must supply directory to mirror");
    let path = PathBuf::from(path);

    let fs = fs::ShadowFS::new(path);
    println!("ShadowFS rooted at {:?}", fs.root_path());
}
