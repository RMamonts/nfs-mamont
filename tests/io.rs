use std::{collections::HashSet, fs::OpenOptions, os::unix::fs::FileExt, path::PathBuf};

const EXPORT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/", "exports");

fn path(name: &str) -> PathBuf {
    let mut path = PathBuf::new();

    path.push(EXPORT_DIR);
    path.push(name);

    path
}

#[test]
fn create_dir() {
    std::fs::create_dir(path("create_dir")).unwrap();
}

#[test]
fn create_new_file() {
    OpenOptions::new()
        .create_new(true)
        .truncate(true)
        .write(true)
        .open(path("create_new_file"))
        .unwrap();
}

#[test]
fn read_dir() {
    const FILES: &[&str] = &["first_file", "second_file", "third_file"];

    let dir_path = path("read_dir");
    std::fs::create_dir(&dir_path).unwrap();

    let mut dir_iter = std::fs::read_dir(&dir_path).unwrap();
    assert!(dir_iter.next().is_none());

    for &file in FILES {
        let file_path = {
            let mut path = dir_path.clone();
            path.push(file);
            path
        };
        OpenOptions::new().create_new(true).write(true).truncate(true).open(&file_path).unwrap();
    }

    let dir_entries: std::io::Result<Vec<_>> = std::fs::read_dir(&dir_path).unwrap().collect();
    let dir_entries: HashSet<_> =
        dir_entries.unwrap().into_iter().map(|entry| entry.file_name()).collect();

    assert_eq!(dir_entries.len(), FILES.len());
    for &file in FILES {
        assert!(dir_entries.contains(std::ffi::OsStr::new(file)));
    }
}

struct Random(u32);

impl Default for Random {
    fn default() -> Self {
        Self(1)
    }
}

impl Random {
    fn next(&mut self) -> u32 {
        let next = self.0;

        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;

        next
    }
}

#[test]
fn create_read_write() {
    const ITERATIONS: u32 = 100;
    const BUFFERS_CAPACITY: usize = 1024;

    let dir_path = path("create_read_write");
    match std::fs::create_dir(&dir_path) {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        err => err.expect("to create dir"),
    };

    let file_path = {
        let mut path = dir_path.clone();
        path.push("file");
        path
    };
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(&file_path)
        .unwrap();

    let mut random = Random::default();
    let mut actual_buffer = vec![0u8; BUFFERS_CAPACITY];
    let mut expected_buffer = vec![0u8; BUFFERS_CAPACITY];

    for _ in 0..ITERATIONS {
        for expected in expected_buffer.iter_mut() {
            *expected = random.next() as u8;
        }

        let offset = random.next() as u64;

        file.write_all_at(&expected_buffer, offset).unwrap();
        file.read_exact_at(&mut actual_buffer, offset).unwrap();

        assert_eq!(expected_buffer, actual_buffer);
    }
}
