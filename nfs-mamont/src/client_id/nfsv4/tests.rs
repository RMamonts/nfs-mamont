use super::*;
use crate::auth::{AuthSysParams, Credential};
use crate::client_id::{Callback, ClientVerifier, NetId};

fn manager() -> ClientIdManager {
    ClientIdManager::new()
}

fn registration(verifier: u8) -> Registration {
    Registration {
        identity: identity(vec![0, 255, 0]),
        verifier: ClientVerifier::new([verifier; 8]),
        callback: Callback {
            netid: NetId::new("tcp".into()),
            address: "127.0.0.1:2049".parse().unwrap(),
            program: 123,
            ident: 1,
        },
    }
}

fn confirmed(m: &mut ClientIdManager) -> PendingClient {
    let pair = m.register(registration(1), &Credential::None).unwrap();
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));
    m.confirm(pair, &Credential::None).unwrap();
    pair
}

#[test]
fn callback_update_preserves_clientid() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let mut update = registration(1);
    update.callback.ident = 99;
    let next = m.register(update.clone(), &Credential::None).unwrap();
    assert_eq!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(&m.get(first.clientid).unwrap().callback, &registration(1).callback);
    assert_eq!(m.confirm(next, &Credential::None), Ok(()));
    let record = m.get(first.clientid).unwrap();
    assert_eq!(record, &update);
}

#[test]
fn new_registration_can_restore_a_previous_callback() {
    let mut m = manager();
    let mut previous: Option<PendingClient> = None;
    for callback_ident in [1, 2, 1, 1] {
        let mut input = registration(1);
        input.callback.ident = callback_ident;
        let pair = m.register(input.clone(), &Credential::None).unwrap();
        if let Some(old) = previous {
            assert_eq!(pair.clientid, old.clientid);
            assert_ne!(pair.confirmation, old.confirmation);
        }
        assert_eq!(m.confirm(pair, &Credential::None), Ok(()));
        assert_eq!(m.get(pair.clientid).unwrap(), &input);
        previous = Some(pair);
    }
}

#[test]
fn callback_updates_preserve_pending_reboot() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let reboot = m.register(registration(2), &Credential::None).unwrap();
    let mut update = registration(1);
    update.callback.ident = 99;
    let replaced = m.register(update.clone(), &Credential::None).unwrap();
    update.callback.ident = 100;
    let next = m.register(update.clone(), &Credential::None).unwrap();
    assert_eq!(next.clientid, first.clientid);
    assert_ne!(next.confirmation, replaced.confirmation);
    assert_eq!(m.confirm(replaced, &Credential::None), Err(Error::StaleClientId));
    assert_eq!(m.confirm(next, &Credential::None), Ok(()));
    assert_eq!(m.get(first.clientid).unwrap(), &update);

    m.confirm(reboot, &Credential::None).unwrap();
    assert_eq!(m.get(reboot.clientid).unwrap(), &registration(2));
    assert_eq!(m.get(first.clientid), Err(Error::StaleClientId));
}

#[test]
fn new_incarnation_replaces_all_pending_records_for_its_identity() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let reboot = m.register(registration(2), &Credential::None).unwrap();
    let mut update = registration(1);
    update.callback.ident = 99;
    let callback = m.register(update, &Credential::None).unwrap();
    let mut other = registration(1);
    other.identity = identity(b"other-client".to_vec());
    let unrelated = m.register(other.clone(), &Credential::None).unwrap();

    let replacement = m.register(registration(3), &Credential::None).unwrap();
    assert_eq!(m.confirm(reboot, &Credential::None), Err(Error::StaleClientId));
    assert_eq!(m.confirm(callback, &Credential::None), Err(Error::StaleClientId));
    assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
    assert_eq!(m.confirm(unrelated, &Credential::None), Ok(()));
    assert_eq!(m.get(unrelated.clientid).unwrap(), &other);
    assert_eq!(m.confirm(replacement, &Credential::None), Ok(()));
    assert_eq!(m.get(replacement.clientid).unwrap(), &registration(3));
}

#[test]
fn reboot_retires_only_at_confirmation() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let next = m.register(registration(2), &Credential::None).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert!(m.get(first.clientid).is_ok());
    m.confirm(next, &Credential::None).unwrap();
    assert_eq!(m.get(first.clientid), Err(Error::StaleClientId));
    assert_eq!(m.confirm(next, &Credential::None), Ok(()));
}

#[test]
fn new_request_replaces_pending_even_with_identical_arguments() {
    let mut m = manager();
    let first = m.register(registration(1), &Credential::None).unwrap();
    m.drc = Cache::default(); // A DRC miss must execute the RFC replacement branch.
    let next = m.register(registration(1), &Credential::None).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(m.confirm(first, &Credential::None), Err(Error::StaleClientId));
    assert_eq!(m.confirm(next, &Credential::None), Ok(()));
}

#[test]
fn pending_reboot_is_replaced_when_callback_changes() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let a = m.register(registration(2), &Credential::None).unwrap();
    let mut changed = registration(2);
    changed.callback.ident = 99;
    let b = m.register(changed, &Credential::None).unwrap();
    assert_ne!(a.clientid, b.clientid);
    assert!(m.get(first.clientid).is_ok());
    assert_eq!(m.confirm(a, &Credential::None), Err(Error::StaleClientId));
    assert_eq!(m.confirm(b, &Credential::None), Ok(()));
}

#[test]
fn invalidated_pending_cannot_be_confirmed() {
    let mut m = manager();
    let pair = m.register(registration(1), &Credential::None).unwrap();
    assert_eq!(m.invalidate(pair.clientid), None);
    assert_eq!(m.confirm(pair, &Credential::None), Err(Error::StaleClientId));
}

#[test]
fn wrong_pair_and_old_confirmation_cannot_change_callback() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let mut update = registration(1);
    update.callback.ident = 99;
    let next = m.register(update, &Credential::None).unwrap();
    assert_eq!(m.confirm(first, &Credential::None), Err(Error::StaleClientId));
    let wrong = PendingClient { confirmation: ConfirmationVerifier::new(0), ..next };
    assert_eq!(m.confirm(wrong, &Credential::None), Err(Error::StaleClientId));
    m.confirm(next, &Credential::None).unwrap();
}

#[test]
fn invalidate_removes_shared_id_pending_but_not_new_incarnation() {
    for verifier in [1, 2] {
        let mut m = manager();
        let first = confirmed(&mut m);
        let pair = m.register(registration(verifier), &Credential::None).unwrap();
        assert!(m.invalidate(first.clientid).is_some());
        assert!(m.invalidate(first.clientid).is_none());
        let result = m.confirm(pair, &Credential::None);
        if verifier == 1 {
            assert_eq!(result, Err(Error::StaleClientId));
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn server_restart_does_not_accept_old_clientid() {
    let pair = confirmed(&mut manager());
    let mut restarted = manager();
    assert_eq!(restarted.get(pair.clientid), Err(Error::StaleClientId));
    assert_eq!(restarted.confirm(pair, &Credential::None), Err(Error::StaleClientId));
}

fn identity(bytes: Vec<u8>) -> ClientIdentity {
    ClientIdentity::new(bytes).unwrap()
}

fn credential(uid: u32, stamp: u32) -> Credential {
    Credential::Sys(AuthSysParams {
        stamp,
        machine_name: "client-host".into(),
        uid,
        gid: 10,
        gids: vec![20, 30],
    })
}

#[test]
fn opaque_identity_accepts_binary_empty_and_limit() {
    assert!(ClientIdentity::new(vec![]).is_ok());
    assert_eq!(identity(vec![0, 255]).into_inner(), vec![0, 255]);
    assert!(ClientIdentity::new(vec![255; 1024]).is_ok());
    assert_eq!(ClientIdentity::new(vec![0; 1025]), Err(Error::InvalidIdentity));
}

#[test]
fn registration_principal_conflict_preserves_confirmed_and_pending() {
    let mut m = manager();
    let owner = credential(100, 1);
    let first = m.register(registration(1), &owner).unwrap();
    m.confirm(first, &owner).unwrap();
    let pending = m.register(registration(2), &owner).unwrap();
    for verifier in [1, 2, 3] {
        assert_eq!(
            m.register(registration(verifier), &credential(200, 1)),
            Err(Error::IdentityInUse)
        );
        assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
    }
    assert_eq!(m.confirm(pending, &owner), Ok(()));
}

#[test]
fn pending_without_confirmed_can_be_replaced_by_another_principal() {
    let mut m = manager();
    let first = m.register(registration(1), &credential(100, 1)).unwrap();
    let next = m.register(registration(1), &credential(200, 2)).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(m.confirm(first, &credential(100, 1)), Err(Error::StaleClientId));
    assert_eq!(m.confirm(next, &credential(100, 1)), Err(Error::IdentityInUse));
    assert_eq!(m.confirm(next, &credential(200, 2)), Ok(()));
}

#[test]
fn credentials_use_full_equality() {
    let mut m = manager();
    let owner = credential(100, 1);
    let changed_stamp = credential(100, 2);
    assert_ne!(owner, changed_stamp);
    assert_eq!(owner, owner.clone());
    let first = m.register(registration(1), &owner).unwrap();
    assert_eq!(m.confirm(first, &changed_stamp), Err(Error::IdentityInUse));
    m.confirm(first, &owner).unwrap();
    assert_eq!(m.register(registration(1), &changed_stamp), Err(Error::IdentityInUse));
}

#[test]
fn confirmation_rejects_foreign_credentials_in_each_branch() {
    for callback_update in [false, true] {
        let mut m = manager();
        let owner = credential(100, 1);
        let foreign = credential(200, 1);
        let first = m.register(registration(1), &owner).unwrap();
        assert_eq!(m.confirm(first, &foreign), Err(Error::IdentityInUse));
        m.confirm(first, &owner).unwrap();
        assert_eq!(m.confirm(first, &foreign), Err(Error::IdentityInUse));
        let input = registration(if callback_update { 1 } else { 2 });
        let next = m.register(input.clone(), &owner).unwrap();
        assert_eq!(m.confirm(next, &foreign), Err(Error::IdentityInUse));
        assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
        m.confirm(next, &owner).unwrap();
        assert_eq!(m.get(next.clientid).unwrap(), &input);
        assert_eq!(m.confirm(next, &owner), Ok(()));
    }
}

#[test]
fn new_incarnation_replaces_pending_even_when_verifier_is_unchanged() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let a = m.register(registration(2), &Credential::None).unwrap();
    m.drc = Cache::default(); // The algorithm's replacement rules apply on a DRC miss.
    let b = m.register(registration(2), &Credential::None).unwrap();
    assert_ne!(a.clientid, b.clientid);
    assert_ne!(a.confirmation, b.confirmation);
    assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
    assert_eq!(m.confirm(a, &Credential::None), Err(Error::StaleClientId));
    assert_eq!(m.confirm(b, &Credential::None), Ok(()));
}

#[test]
fn anonymous_and_sys_credentials_are_distinct() {
    let mut m = manager();
    let _ = confirmed(&mut m);
    assert_eq!(m.register(registration(1), &credential(0, 0)), Err(Error::IdentityInUse));
}

#[test]
fn drc_hit_preserves_pending_and_confirmed_state() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let input = registration(2);
    let owner = credential(100, 1);
    // A cached registration is checked before subsequent principal/branch checks.
    let mut other = input.clone();
    other.identity = identity(b"other-client".to_vec());
    let other_pair = m.register(other.clone(), &owner).unwrap();
    let pair = m.register(input.clone(), &Credential::None).unwrap();
    assert_eq!(m.register(input, &Credential::None).unwrap(), pair);
    assert_eq!(m.register(other, &owner).unwrap(), other_pair);
    assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
    assert_eq!(m.confirm(pair, &Credential::None), Ok(()));
}

#[test]
fn drc_checks_credentials_and_evicts_invalidated_registration() {
    let mut m = manager();
    let first = m.register(registration(1), &credential(100, 1)).unwrap();
    let next = m.register(registration(1), &credential(200, 1)).unwrap();
    assert_ne!(first, next);
    assert_eq!(m.confirm(first, &credential(100, 1)), Err(Error::StaleClientId));
    assert_eq!(m.invalidate(next.clientid), None);
    let recreated = m.register(registration(1), &credential(200, 1)).unwrap();
    assert_ne!(next, recreated);
    assert_eq!(m.confirm(recreated, &credential(200, 1)), Ok(()));
}

#[test]
fn confirmation_drc_replays_success_without_restoring_records() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let pair = m.register(registration(2), &Credential::None).unwrap();
    assert_eq!(m.confirm(pair, &Credential::None), Ok(()));
    assert_eq!(m.get(first.clientid), Err(Error::StaleClientId));
    assert_eq!(m.invalidate(pair.clientid), Some(pair.clientid));
    // A DRC hit precedes the table lookup and must not recreate the record.
    assert_eq!(m.confirm(pair, &Credential::None), Ok(()));
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));
    m.confirmation_drc = Cache::default();
    assert_eq!(m.confirm(pair, &Credential::None), Err(Error::StaleClientId));
}

#[test]
fn confirmation_drc_separates_credentials_and_caches_failures() {
    let mut m = manager();
    let owner = credential(100, 1);
    let foreign = credential(200, 1);
    let pair = m.register(registration(1), &owner).unwrap();
    assert_eq!(m.confirm(pair, &foreign), Err(Error::IdentityInUse));
    m.confirm(pair, &owner).unwrap();
    assert_eq!(m.invalidate(pair.clientid), Some(pair.clientid));
    assert_eq!(m.confirm(pair, &foreign), Err(Error::IdentityInUse));
    assert_eq!(m.confirm(pair, &owner), Ok(()));
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));
}

#[test]
fn confirmation_verifier_encodes_counter_in_network_byte_order() {
    assert_eq!(
        ConfirmationVerifier::new(0x0102030405060708),
        ConfirmationVerifier::from_bytes([1, 2, 3, 4, 5, 6, 7, 8]),
    );
}

#[test]
fn callback_confirmation_requires_distinct_verifiers_before_credentials() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let mut update = registration(1);
    update.callback.ident = 99;
    m.register(update.clone(), &Credential::None).unwrap();
    // Model a pair of records violating the RFC's s != t precondition.
    m.pending.get_mut(&first.clientid).unwrap().confirmation = first.confirmation;
    assert_eq!(m.confirm(first, &credential(200, 1)), Err(Error::StaleClientId));
    assert_eq!(m.confirm(first, &Credential::None), Err(Error::StaleClientId));
    assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
    assert_eq!(m.pending[&first.clientid].registration, update);
    assert_eq!(m.pending[&first.clientid].confirmation, first.confirmation);
}

#[test]
fn callback_confirmation_checks_credentials_of_both_records() {
    for change_confirmed in [false, true] {
        let mut m = manager();
        let first = confirmed(&mut m);
        let mut update = registration(1);
        update.callback.ident = 99;
        let pair = m.register(update.clone(), &Credential::None).unwrap();
        if change_confirmed {
            m.confirmed.get_mut(&first.clientid).unwrap().credentials = credential(200, 1);
        } else {
            m.pending.get_mut(&first.clientid).unwrap().credentials = credential(200, 1);
        }
        assert_ne!(first.confirmation, pair.confirmation);
        assert_eq!(m.confirm(pair, &Credential::None), Err(Error::IdentityInUse));
        assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
        assert_eq!(m.pending[&first.clientid].registration, update);
        assert_eq!(m.pending[&first.clientid].confirmation, pair.confirmation);
    }
}
