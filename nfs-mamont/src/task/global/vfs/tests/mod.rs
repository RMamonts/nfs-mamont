use crate::allocator::Buffer;
use crate::parser::NfsArguments;
use crate::vfs::file::{BackendId, Handle, PAYLOAD_SIZE};
use crate::vfs::{self, get_attr, link, read, rename, write, DirOpArgs, NfsRes};

use super::{cost, failed_response, target, Target};

mod drr;

/// Routing never touches the payload, so the buffer holds no data.
struct MockBuffer;

impl Buffer for MockBuffer {
    fn chunks(&self) -> impl Iterator<Item = &[u8]> + Send + '_ {
        std::iter::empty()
    }

    fn chunks_mut(&mut self) -> impl Iterator<Item = &mut [u8]> + Send + '_ {
        std::iter::empty()
    }

    fn len(&self) -> usize {
        0
    }

    fn is_empty(&self) -> bool {
        true
    }

    fn empty() -> Self {
        MockBuffer
    }
}

fn handle(backend: u8) -> Handle {
    Handle::new(BackendId::new(backend, 0), [0x01; PAYLOAD_SIZE])
}

fn name() -> vfs::file::Name {
    vfs::file::Name::new("file".to_string()).unwrap()
}

#[test]
fn null_needs_no_backend() {
    let proc = NfsArguments::<MockBuffer>::Null;

    assert!(matches!(target(&proc), Target::None));
}

#[test]
fn backend_is_taken_from_the_handle() {
    let proc = NfsArguments::<MockBuffer>::GetAttr(get_attr::Args { file: handle(3) });

    assert!(matches!(target(&proc), Target::Single(id) if id == BackendId::new(3, 0)));
}

#[test]
fn rename_within_one_backend_is_routed_to_it() {
    let proc = NfsArguments::<MockBuffer>::Rename(rename::Args {
        from: DirOpArgs { dir: handle(2), name: name() },
        to: DirOpArgs { dir: handle(2), name: name() },
    });

    assert!(matches!(target(&proc), Target::Single(id) if id == BackendId::new(2, 0)));
}

#[test]
fn rename_across_backends_is_crossing() {
    let proc = NfsArguments::<MockBuffer>::Rename(rename::Args {
        from: DirOpArgs { dir: handle(2), name: name() },
        to: DirOpArgs { dir: handle(5), name: name() },
    });

    assert!(matches!(target(&proc), Target::Crossing));
}

#[test]
fn link_across_backends_is_crossing() {
    let proc = NfsArguments::<MockBuffer>::Link(link::Args {
        file: handle(0),
        link: DirOpArgs { dir: handle(1), name: name() },
    });

    assert!(matches!(target(&proc), Target::Crossing));
}

#[test]
fn failed_response_keeps_the_procedure_variant() {
    let proc = NfsArguments::<MockBuffer>::GetAttr(get_attr::Args { file: handle(0) });

    let NfsRes::GetAttr(Err(fail)) = failed_response(&proc, vfs::Error::StaleFile) else {
        panic!("expected a failed GETATTR response");
    };
    assert_eq!(fail.error, vfs::Error::StaleFile);
}

fn read_of(count: u32) -> NfsArguments<MockBuffer> {
    NfsArguments::Read(read::Args { file: handle(0), offset: 0, count }, MockBuffer)
}

fn write_of(size: u32) -> NfsArguments<MockBuffer> {
    let stable = write::StableHow::Unstable;
    NfsArguments::Write(write::Args { file: handle(0), offset: 0, size, stable, data: MockBuffer })
}

#[test]
fn cost_grows_with_the_data() {
    let getattr = NfsArguments::<MockBuffer>::GetAttr(get_attr::Args { file: handle(0) });

    assert_eq!(cost(&getattr), 1);
    assert_eq!(cost(&read_of(0)), 1);
    assert_eq!(cost(&read_of(4096)), 2);
    assert_eq!(cost(&read_of(64 * 1024)), 2);
    assert_eq!(cost(&read_of(64 * 1024 + 1)), 3);
    assert_eq!(cost(&read_of(1 << 20)), 17);
    assert_eq!(cost(&write_of(64 * 1024)), 2);
}
