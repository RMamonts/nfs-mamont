use crate::allocator::Slice;
use crate::parser::NfsArguments;
use crate::task::global::vfs::{failed_response, target, Target};
use crate::vfs::file::{Handle, PAYLOAD_SIZE};
use crate::vfs::{self, get_attr, link, rename, DirOpArgs, NfsRes};

fn handle(backend: u8) -> Handle {
    Handle::new(backend, [0x01; PAYLOAD_SIZE])
}

fn name() -> vfs::file::Name {
    vfs::file::Name::new("file".to_string()).unwrap()
}

#[test]
fn null_needs_no_backend() {
    let proc = NfsArguments::<Slice>::Null;

    assert!(matches!(target(&proc), Target::None));
}

#[test]
fn backend_is_taken_from_the_handle() {
    let proc = NfsArguments::<Slice>::GetAttr(get_attr::Args { file: handle(3) });

    assert!(matches!(target(&proc), Target::Single(3)));
}

#[test]
fn rename_within_one_backend_is_routed_to_it() {
    let proc = NfsArguments::<Slice>::Rename(rename::Args {
        from: DirOpArgs { dir: handle(2), name: name() },
        to: DirOpArgs { dir: handle(2), name: name() },
    });

    assert!(matches!(target(&proc), Target::Single(2)));
}

#[test]
fn rename_across_backends_is_crossing() {
    let proc = NfsArguments::<Slice>::Rename(rename::Args {
        from: DirOpArgs { dir: handle(2), name: name() },
        to: DirOpArgs { dir: handle(5), name: name() },
    });

    assert!(matches!(target(&proc), Target::Crossing));
}

#[test]
fn link_across_backends_is_crossing() {
    let proc = NfsArguments::<Slice>::Link(link::Args {
        file: handle(0),
        link: DirOpArgs { dir: handle(1), name: name() },
    });

    assert!(matches!(target(&proc), Target::Crossing));
}

#[test]
fn failed_response_keeps_the_procedure_variant() {
    let proc = NfsArguments::<Slice>::GetAttr(get_attr::Args { file: handle(0) });

    let NfsRes::GetAttr(Err(fail)) = failed_response(&proc, vfs::Error::StaleFile) else {
        panic!("expected a failed GETATTR response");
    };
    assert_eq!(fail.error, vfs::Error::StaleFile);
}
