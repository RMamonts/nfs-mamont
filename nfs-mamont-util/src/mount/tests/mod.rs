use std::collections::HashSet;
use std::net::SocketAddr;

use nfs_mamont::auth::{AuthSysParams, Credential, SUPPORTED_AUTH_FLAVORS};
use nfs_mamont::mount::mnt::{Args, Fail, Mnt};
use nfs_mamont::vfs::file;
use nfs_mamont::AuthFlavor;

use super::{ExportPolicy, MountService};

fn handle(byte: u8) -> file::Handle {
    file::Handle::new(file::BackendId::new(0, 0), [byte; file::PAYLOAD_SIZE])
}

fn path(name: &str) -> file::Path {
    file::Path::new(name.to_string()).unwrap()
}

fn sys_cred(uid: u32, gid: u32, gids: &[u32]) -> Credential {
    Credential::Sys(AuthSysParams {
        stamp: 0,
        machine_name: "client".to_string(),
        uid,
        gid,
        gids: gids.to_vec(),
    })
}

fn open_policy() -> ExportPolicy {
    ExportPolicy::new(SUPPORTED_AUTH_FLAVORS.to_vec())
}

fn sys_policy() -> ExportPolicy {
    ExportPolicy::new(vec![AuthFlavor::Sys])
}

async fn mnt(service: &MountService, dir: &str, cred: &Credential) -> Result<file::Handle, Fail> {
    service.mnt(Args { dirpath: path(dir) }, client(), cred).await.map(|ok| ok.file_handle)
}

fn client() -> SocketAddr {
    "127.0.0.1:1234".parse().unwrap()
}

async fn service() -> MountService {
    let service = MountService::new();
    service.add_export(path("/initial"), handle(1), open_policy()).await;
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

    service.add_export(path("/added"), handle(2), open_policy()).await;

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

    service.add_export(path("/initial"), handle(9), open_policy()).await;

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

#[tokio::test]
async fn add_export_keeps_given_policy() {
    let service = service().await;

    let entry = service.export_entry(&path("/initial")).await.unwrap();
    assert_eq!(entry.policy.auth_flavors, SUPPORTED_AUTH_FLAVORS);
    assert!(entry.policy.allowed_uids.is_none());
    assert!(entry.policy.allowed_gids.is_none());

    let policy = ExportPolicy {
        allowed_uids: Some(HashSet::from([1000])),
        allowed_gids: Some(HashSet::from([100])),
        ..sys_policy()
    };
    service.add_export(path("/sys"), handle(3), policy).await;

    let entry = service.export_entry(&path("/sys")).await.unwrap();
    assert_eq!(entry.policy.auth_flavors, [AuthFlavor::Sys]);
    assert_eq!(entry.policy.allowed_uids, Some(HashSet::from([1000])));
    assert_eq!(entry.policy.allowed_gids, Some(HashSet::from([100])));
}

#[tokio::test]
async fn mnt_returns_export_auth_flavors_and_records_mount() {
    let service = service().await;
    service.add_export(path("/sys"), handle(3), sys_policy()).await;

    let success =
        service.mnt(Args { dirpath: path("/sys") }, client(), &sys_cred(0, 0, &[])).await.unwrap();

    assert_eq!(success.file_handle, handle(3));
    assert_eq!(success.auth_flavors, [AuthFlavor::Sys]);
    assert!(service.mounts.read().await.by_client.contains_key(&client()));
}

#[tokio::test]
async fn mnt_without_restrictions_accepts_any_identity() {
    let service = service().await;

    assert_eq!(mnt(&service, "/initial", &Credential::None).await.unwrap(), handle(1));
    assert_eq!(mnt(&service, "/initial", &sys_cred(0, 0, &[])).await.unwrap(), handle(1));
    assert_eq!(mnt(&service, "/initial", &sys_cred(4242, 4242, &[1, 2])).await.unwrap(), handle(1));
}

#[tokio::test]
async fn mnt_denies_flavor_outside_export_policy() {
    let service = service().await;
    service.add_export(path("/sys"), handle(3), sys_policy()).await;

    let result = mnt(&service, "/sys", &Credential::None).await;

    assert!(matches!(result, Err(Fail::Access)));
    assert!(service.mounts.read().await.by_client.is_empty());
}

#[tokio::test]
async fn mnt_none_in_policy_accepts_any_flavor() {
    let service = service().await;
    service.add_export(path("/any"), handle(4), ExportPolicy::new(vec![AuthFlavor::None])).await;

    assert_eq!(mnt(&service, "/any", &sys_cred(0, 0, &[])).await.unwrap(), handle(4));
}

#[tokio::test]
async fn mnt_empty_flavor_list_denies_every_flavor() {
    let service = service().await;
    service.add_export(path("/closed"), handle(5), ExportPolicy::new(Vec::new())).await;

    let none = mnt(&service, "/closed", &Credential::None).await;
    let sys = mnt(&service, "/closed", &sys_cred(0, 0, &[])).await;

    assert!(matches!(none, Err(Fail::Access)));
    assert!(matches!(sys, Err(Fail::Access)));
    assert!(service.mounts.read().await.by_client.is_empty());
}

#[tokio::test]
async fn mnt_allowed_uids_restricts_by_uid_and_denies_anonymous() {
    let service = service().await;
    let policy = ExportPolicy { allowed_uids: Some(HashSet::from([1000])), ..open_policy() };
    service.add_export(path("/uid"), handle(6), policy).await;

    assert_eq!(mnt(&service, "/uid", &sys_cred(1000, 7, &[])).await.unwrap(), handle(6));
    assert!(matches!(
        mnt(&service, "/uid", &sys_cred(1001, 1000, &[1000])).await,
        Err(Fail::Access)
    ));
    assert!(matches!(mnt(&service, "/uid", &Credential::None).await, Err(Fail::Access)));
}

#[tokio::test]
async fn mnt_allowed_gids_matches_primary_or_auxiliary_gid() {
    let service = service().await;
    let policy = ExportPolicy { allowed_gids: Some(HashSet::from([100])), ..open_policy() };
    service.add_export(path("/gid"), handle(7), policy).await;

    assert_eq!(mnt(&service, "/gid", &sys_cred(1, 100, &[])).await.unwrap(), handle(7));
    assert_eq!(mnt(&service, "/gid", &sys_cred(1, 1, &[5, 100])).await.unwrap(), handle(7));
    assert!(matches!(mnt(&service, "/gid", &sys_cred(100, 1, &[5])).await, Err(Fail::Access)));
    assert!(matches!(mnt(&service, "/gid", &Credential::None).await, Err(Fail::Access)));
}

#[tokio::test]
async fn mnt_uid_and_gid_restrictions_must_both_pass() {
    let service = service().await;
    let policy = ExportPolicy {
        allowed_uids: Some(HashSet::from([1000])),
        allowed_gids: Some(HashSet::from([100])),
        ..open_policy()
    };
    service.add_export(path("/both"), handle(8), policy).await;

    assert_eq!(mnt(&service, "/both", &sys_cred(1000, 100, &[])).await.unwrap(), handle(8));
    assert!(matches!(mnt(&service, "/both", &sys_cred(1000, 1, &[])).await, Err(Fail::Access)));
    assert!(matches!(mnt(&service, "/both", &sys_cred(1, 100, &[])).await, Err(Fail::Access)));
    assert!(service.mounts.read().await.by_client.len() == 1);
}
