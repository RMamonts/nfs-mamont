use std::fs::File;
use std::fs::OpenOptions;
use std::os::unix::fs::FileExt;
use std::path::Path;

use crate::random::Random;
use crate::utils;

fn create_file(path: impl AsRef<Path>) -> File {
    OpenOptions::new().create(true).read(true).write(true).truncate(true).open(&path).unwrap()
}

/// Test various file operations:
/// - create file
/// - write file with random data
/// - read written data from file
/// - delete file
pub fn create_write_read_delete(mount_point: impl AsRef<Path>, random: &mut Random) {
    const ITERATIONS: u32 = 100;
    const BUFFERS_CAPACITY: usize = 1024;

    let file_path = utils::join(&[&mount_point, &"test_file"]);

    let file = create_file(&file_path);

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

    std::fs::remove_file(&file_path).unwrap()
}
