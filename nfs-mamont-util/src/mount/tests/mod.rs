use nfs_mamont::vfs::file;

use super::MountService;

fn handle(byte: u8) -> file::Handle {
    file::Handle::new(file::BackendId::new(0, 0), [byte; file::PAYLOAD_SIZE])
}

fn path(name: &str) -> file::Path {
    file::Path::new(name.to_string()).unwrap()
}

async fn service() -> MountService {
    let service = MountService::new();
    service.add_export(path("/initial"), handle(1)).await;
    service
}

#[tokio::test]
async fn new_has_no_exports() {
    let service = MountService::new();

    assert!(service.export_entry(&path("/initial")).await.is_none());
    assert!(service.exports.read().await.export_list().is_empty());
}

#[tokio::test]
async fn add_export_keeps_initial_entry() {
    let service = service().await;

    let entry = service.export_entry(&path("/initial")).await.unwrap();
    assert_eq!(entry.root_handle, handle(1));
    assert_eq!(service.exports.read().await.export_list().len(), 1);
}

#[tokio::test]
async fn add_export_makes_entry_visible() {
    let service = service().await;

    service.add_export(path("/added"), handle(2)).await;

    let entry = service.export_entry(&path("/added")).await.unwrap();
    assert_eq!(entry.root_handle, handle(2));
    let directories = service
        .exports
        .read()
        .await
        .export_list()
        .into_iter()
        .map(|entry| entry.directory)
        .collect::<Vec<_>>();
    assert_eq!(directories.len(), 2);
    assert!(directories.contains(&path("/added")));
}

#[tokio::test]
async fn add_export_replaces_existing_entry() {
    let service = service().await;

    service.add_export(path("/initial"), handle(9)).await;

    let entry = service.export_entry(&path("/initial")).await.unwrap();
    assert_eq!(entry.root_handle, handle(9));
    assert_eq!(service.exports.read().await.export_list().len(), 1);
}

#[tokio::test]
async fn remove_export_removes_entry_and_reports_whether_it_existed() {
    let service = service().await;

    assert!(service.remove_export(&path("/initial")).await);
    assert!(!service.remove_export(&path("/initial")).await);
    assert!(service.exports.read().await.export_list().is_empty());
}
