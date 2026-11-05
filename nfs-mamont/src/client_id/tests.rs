use super::*;

fn request(xid: u32) -> RequestId {
    RequestId { scope: 1, xid }
}

fn manager() -> ClientIdManager {
    ClientIdManager::new()
}

fn registration(verifier: u8) -> Registration {
    Registration {
        identity: ClientIdentity::Handshake(vec![0, 255, 0]),
        verifier: ClientVerifier::new([verifier; 8]),
        callback: Callback::Universal {
            netid: NetId::new("tcp".into()),
            address: "127.0.0.1:2049".parse().unwrap(),
            program: 123,
            ident: 1,
        },
    }
}

fn confirmed(m: &mut ClientIdManager) -> PendingClient {
    let pair = m.register(request(1), registration(1)).unwrap();
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));
    let transition = m.confirm(request(2), pair).unwrap();
    assert!(transition.retired.is_none());
    pair
}

#[test]
fn callback_update_preserves_clientid() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let mut update = registration(1);
    if let Callback::Universal { ident, .. } = &mut update.callback {
        *ident = 99;
    }
    let next = m.register(request(3), update.clone()).unwrap();
    assert_eq!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(&m.get(first.clientid).unwrap().callback, &registration(1).callback);
    assert!(m.confirm(request(4), next).unwrap().retired.is_none());
    let record = m.get(first.clientid).unwrap();
    assert_eq!(record, &update);
}

#[test]
fn new_registration_can_restore_a_previous_callback() {
    let mut m = manager();
    let mut previous: Option<PendingClient> = None;
    for (index, callback_ident) in [1, 2, 1, 1].into_iter().enumerate() {
        let mut input = registration(1);
        if let Callback::Universal { ident, .. } = &mut input.callback {
            *ident = callback_ident;
        }
        let pair = m.register(request(10 + 2 * index as u32), input.clone()).unwrap();
        if let Some(old) = previous {
            assert_eq!(pair.clientid, old.clientid);
            assert_ne!(pair.confirmation, old.confirmation);
        }
        assert!(m.confirm(request(11 + 2 * index as u32), pair).unwrap().retired.is_none());
        assert_eq!(m.get(pair.clientid).unwrap(), &input);
        previous = Some(pair);
    }
}

#[test]
fn callback_updates_preserve_pending_reboot() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let reboot = m.register(request(3), registration(2)).unwrap();
    let mut update = registration(1);
    if let Callback::Universal { ident, .. } = &mut update.callback {
        *ident = 99;
    }
    let replaced = m.register(request(4), update.clone()).unwrap();
    if let Callback::Universal { ident, .. } = &mut update.callback {
        *ident = 100;
    }
    let next = m.register(request(5), update.clone()).unwrap();
    assert_eq!(next.clientid, first.clientid);
    assert_ne!(next.confirmation, replaced.confirmation);
    assert_eq!(m.confirm(request(6), replaced), Err(Error::StaleClientId));
    assert!(m.confirm(request(7), next).unwrap().retired.is_none());
    assert_eq!(m.get(first.clientid).unwrap(), &update);

    let transition = m.confirm(request(8), reboot).unwrap();
    assert_eq!(transition.retired, Some(first.clientid));
    assert_eq!(m.get(reboot.clientid).unwrap(), &registration(2));
    assert_eq!(m.get(first.clientid), Err(Error::StaleClientId));
}

#[test]
fn new_incarnation_replaces_all_pending_records_for_its_identity() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let reboot = m.register(request(3), registration(2)).unwrap();
    let mut update = registration(1);
    if let Callback::Universal { ident, .. } = &mut update.callback {
        *ident = 99;
    }
    let callback = m.register(request(4), update).unwrap();
    let mut other = registration(1);
    other.identity = ClientIdentity::Handshake(b"other-client".to_vec());
    let unrelated = m.register(request(5), other.clone()).unwrap();

    let replacement = m.register(request(6), registration(3)).unwrap();
    assert_eq!(m.confirm(request(7), reboot), Err(Error::StaleClientId));
    assert_eq!(m.confirm(request(8), callback), Err(Error::StaleClientId));
    assert_eq!(m.get(first.clientid).unwrap(), &registration(1));
    assert!(m.confirm(request(9), unrelated).unwrap().retired.is_none());
    assert_eq!(m.get(unrelated.clientid).unwrap(), &other);
    assert_eq!(m.confirm(request(10), replacement).unwrap().retired, Some(first.clientid));
    assert_eq!(m.get(replacement.clientid).unwrap(), &registration(3));
}

#[test]
fn reboot_retires_only_at_confirmation() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let next = m.register(request(3), registration(2)).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert!(m.get(first.clientid).is_ok());
    let transition = m.confirm(request(4), next).unwrap();
    let retired = transition.retired.unwrap();
    assert_eq!(retired, first.clientid);
    assert_eq!(m.get(first.clientid), Err(Error::StaleClientId));
    assert!(m.confirm(request(4), next).unwrap().retired.is_none());
    assert!(m.confirm(request(5), next).unwrap().retired.is_none());
}

#[test]
fn repeated_registration_returns_cached_pair() {
    let mut m = manager();
    let first = m.register(request(3), registration(1)).unwrap();
    let second = m.register(request(3), registration(1)).unwrap();
    assert_eq!(first, second);
    let _ = m.confirm(request(4), first).unwrap();
    assert!(m.confirm(request(4), second).unwrap().retired.is_none());
}

#[test]
fn new_request_replaces_pending_even_with_identical_arguments() {
    let mut m = manager();
    let first = m.register(request(1), registration(1)).unwrap();
    let next = m.register(request(2), registration(1)).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(m.confirm(request(3), first), Err(Error::StaleClientId));
    assert!(m.confirm(request(4), next).unwrap().retired.is_none());
}

#[test]
fn replay_scope_separates_identical_xids_and_arguments() {
    let mut m = manager();
    let first = m.register(request(1), registration(1)).unwrap();
    let other_scope = RequestId { scope: 2, xid: 1 };
    let next = m.register(other_scope, registration(1)).unwrap();
    assert_ne!(first, next);
    assert_eq!(m.register(request(1), registration(1)).unwrap(), first);
    assert_eq!(m.register(other_scope, registration(1)).unwrap(), next);
    assert_eq!(m.confirm(request(2), first), Err(Error::StaleClientId));
    assert!(m.confirm(request(3), next).unwrap().retired.is_none());
}

#[test]
fn replay_checks_operation_and_arguments() {
    let mut m = manager();
    let first = m.register(request(1), registration(1)).unwrap();
    let next = m.register(request(1), registration(2)).unwrap();
    assert_ne!(first, next);
    assert_eq!(m.confirm(request(1), first), Err(Error::StaleClientId));
    assert!(m.confirm(request(1), next).unwrap().retired.is_none());
    assert_eq!(m.get(next.clientid).unwrap(), &registration(2));
}

#[test]
fn pending_reboot_is_replaced_when_callback_changes() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let a = m.register(request(3), registration(2)).unwrap();
    assert_eq!(m.register(request(3), registration(2)).unwrap(), a);
    let mut changed = registration(2);
    if let Callback::Universal { ident, .. } = &mut changed.callback {
        *ident = 99;
    }
    let b = m.register(request(4), changed).unwrap();
    assert_ne!(a.clientid, b.clientid);
    assert!(m.get(first.clientid).is_ok());
    assert_eq!(m.confirm(request(5), a), Err(Error::StaleClientId));
    assert!(m.confirm(request(6), b).unwrap().retired.is_some());
}

#[test]
fn invalidated_pending_cannot_be_confirmed() {
    let mut m = manager();
    let pair = m.register(request(3), registration(1)).unwrap();
    assert_eq!(m.invalidate(pair.clientid), None);
    assert_eq!(m.confirm(request(4), pair), Err(Error::StaleClientId));
}

#[test]
fn wrong_pair_and_old_confirmation_cannot_change_callback() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let mut update = registration(1);
    if let Callback::Universal { ident, .. } = &mut update.callback {
        *ident = 99;
    }
    let next = m.register(request(3), update).unwrap();
    assert_eq!(m.confirm(request(4), first), Err(Error::StaleClientId));
    let wrong = PendingClient { confirmation: ConfirmationVerifier::new([0; 8]), ..next };
    assert_eq!(m.confirm(request(5), wrong), Err(Error::StaleClientId));
    let _ = m.confirm(request(6), next).unwrap();
}

#[test]
fn invalidate_removes_shared_id_pending_but_not_new_incarnation() {
    for verifier in [1, 2] {
        let mut m = manager();
        let first = confirmed(&mut m);
        let pair = m.register(request(3), registration(verifier)).unwrap();
        assert!(m.invalidate(first.clientid).is_some());
        assert!(m.invalidate(first.clientid).is_none());
        let result = m.confirm(request(4), pair);
        if verifier == 1 {
            assert_eq!(result, Err(Error::StaleClientId));
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn unchecked_get_or_insert_is_scoped_and_preserves_registration() {
    let mut m = manager();
    let explicit = confirmed(&mut m);
    let mut input = registration(1);
    input.identity = ClientIdentity::Implicit {
        address: "192.0.2.1".parse().unwrap(),
        host_name: HostName::new("host-one".into()).unwrap(),
    };
    let first = m.new_unchecked(input.clone()).unwrap();
    assert_ne!(first.clientid, explicit.clientid);
    assert_eq!(m.new_unchecked(input.clone()).unwrap(), first);
    input.verifier = ClientVerifier::new([2; 8]);
    let next = m.new_unchecked(input).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert_eq!(next.retired.unwrap(), first.clientid);
}

#[test]
fn binary_identity_limits_and_domains() {
    let mut m = manager();
    let mut input = registration(1);
    input.identity = ClientIdentity::Handshake(vec![255; 1024]);
    assert!(m.register(request(3), input.clone()).is_ok());
    input.identity = ClientIdentity::Handshake(vec![0; 1025]);
    assert_eq!(m.register(request(4), input.clone()), Err(Error::InvalidIdentity));
    input.identity = ClientIdentity::Implicit {
        address: "192.0.2.1".parse().unwrap(),
        host_name: HostName::new("host-one".into()).unwrap(),
    };
    assert_eq!(m.register(request(5), input.clone()), Err(Error::InvalidIdentity));
    let _ = m.new_unchecked(input).unwrap();
    assert_eq!(m.new_unchecked(registration(1)), Err(Error::InvalidIdentity));
}

#[test]
fn server_restart_does_not_accept_old_clientid() {
    let pair = confirmed(&mut manager());
    let mut restarted = manager();
    assert_eq!(restarted.get(pair.clientid), Err(Error::StaleClientId));
    assert_eq!(restarted.confirm(request(3), pair), Err(Error::StaleClientId));
}

#[test]
fn implicit_identity_is_compared_by_host_address_and_name() {
    let mut m = manager();
    let mut input = registration(1);
    let identity = ClientIdentity::Implicit {
        address: "192.0.2.1".parse().unwrap(),
        host_name: HostName::new("host-one".into()).unwrap(),
    };
    input.identity = identity.clone();
    let first = m.new_unchecked(input.clone()).unwrap();
    assert_eq!(m.lookup(&identity), Some(first.clientid));
    assert_eq!(m.get(first.clientid).unwrap().identity, identity);
    assert_eq!(m.new_unchecked(input.clone()).unwrap().clientid, first.clientid);

    input.identity = ClientIdentity::Implicit {
        address: "192.0.2.2".parse().unwrap(),
        host_name: HostName::new("host-one".into()).unwrap(),
    };
    let different_address = m.new_unchecked(input.clone()).unwrap();
    assert_ne!(different_address.clientid, first.clientid);
    assert!(different_address.retired.is_none());

    input.identity = ClientIdentity::Implicit {
        address: "192.0.2.1".parse().unwrap(),
        host_name: HostName::new("host-two".into()).unwrap(),
    };
    let different_name = m.new_unchecked(input.clone()).unwrap();
    assert_ne!(different_name.clientid, first.clientid);
    assert!(different_name.retired.is_none());

    input.identity = ClientIdentity::Implicit {
        address: "2001:db8::1".parse().unwrap(),
        host_name: HostName::new("host-one".into()).unwrap(),
    };
    let ipv6 = m.new_unchecked(input).unwrap();
    assert_ne!(ipv6.clientid, first.clientid);
    assert!(ipv6.retired.is_none());
    assert_eq!(m.lookup(&identity), Some(first.clientid));
}

#[test]
fn registration_survives_cache_removal_and_confirmation() {
    let mut m = manager();
    let input = registration(1);
    let pair = m.register(request(3), input.clone()).unwrap();
    m.drc.clear();
    let _ = m.confirm(request(4), pair).unwrap();
    assert_eq!(m.get(pair.clientid).unwrap(), &input);
    assert_eq!(m.lookup(&input.identity), Some(pair.clientid));
}

#[test]
fn drc_replays_historical_results_without_restoring_registration() {
    let mut m = manager();
    let pair = confirmed(&mut m);
    assert_eq!(m.invalidate(pair.clientid), Some(pair.clientid));
    assert!(m.confirm(request(2), pair).unwrap().retired.is_none());
    assert_eq!(m.register(request(1), registration(1)), Ok(pair));
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));

    assert_eq!(m.confirm(request(3), pair), Err(Error::StaleClientId));
    let replacement = m.register(request(4), registration(1)).unwrap();
    assert_ne!(pair.clientid, replacement.clientid);
    let _ = m.confirm(request(5), replacement).unwrap();
}

#[test]
fn drc_eviction_reprocesses_identical_arguments() {
    let mut m = manager();
    let first = m.register(request(3), registration(1)).unwrap();
    for ident in 2..=DRC_CAPACITY as u32 + 1 {
        let mut input = registration(1);
        if let Callback::Universal { ident: callback_ident, .. } = &mut input.callback {
            *callback_ident = ident;
        }
        m.register(request(1000 + ident), input).unwrap();
    }
    let next = m.register(request(3), registration(1)).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(m.confirm(request(4), first), Err(Error::StaleClientId));
    let _ = m.confirm(request(5), next).unwrap();
}

#[test]
fn host_name_rejects_empty_or_oversized_input_and_counts_bytes() {
    assert_eq!(HostName::new(String::new()), Err(Error::InvalidHostName));
    assert!(HostName::new("a".repeat(1024)).is_ok());
    assert_eq!(HostName::new("a".repeat(1025)), Err(Error::InvalidHostName));
    let name = "я".repeat(512);
    assert_eq!(HostName::new(name.clone()).unwrap().into_inner(), name);
    assert_eq!(HostName::new("я".repeat(513)), Err(Error::InvalidHostName));
}
