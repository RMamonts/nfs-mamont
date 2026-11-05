use super::*;

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
    let pair = m.register(registration(1)).unwrap();
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));
    let transition = m.confirm(pair).unwrap();
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
    let next = m.register(update.clone()).unwrap();
    assert_eq!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(&m.get(first.clientid).unwrap().callback, &registration(1).callback);
    assert!(m.confirm(next).unwrap().retired.is_none());
    let record = m.get(first.clientid).unwrap();
    assert_eq!(record, &update);
}

#[test]
fn reboot_retires_only_at_confirmation() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let next = m.register(registration(2)).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert!(m.get(first.clientid).is_ok());
    let transition = m.confirm(next).unwrap();
    let retired = transition.retired.unwrap();
    assert_eq!(retired, first.clientid);
    assert_eq!(m.get(first.clientid), Err(Error::StaleClientId));
    assert!(m.confirm(next).unwrap().retired.is_none());
}

#[test]
fn repeated_registration_returns_cached_pair() {
    let mut m = manager();
    let first = m.register(registration(1)).unwrap();
    let second = m.register(registration(1)).unwrap();
    assert_eq!(first, second);
    let _ = m.confirm(first).unwrap();
    assert!(m.confirm(second).unwrap().retired.is_none());
}

#[test]
fn pending_reboot_is_replaced_when_callback_changes() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let a = m.register(registration(2)).unwrap();
    assert_eq!(m.register(registration(2)).unwrap(), a);
    let mut changed = registration(2);
    if let Callback::Universal { ident, .. } = &mut changed.callback {
        *ident = 99;
    }
    let b = m.register(changed).unwrap();
    assert_ne!(a.clientid, b.clientid);
    assert!(m.get(first.clientid).is_ok());
    assert_eq!(m.confirm(a), Err(Error::StaleClientId));
    assert!(m.confirm(b).unwrap().retired.is_some());
}

#[test]
fn invalidated_pending_cannot_be_confirmed() {
    let mut m = manager();
    let pair = m.register(registration(1)).unwrap();
    assert_eq!(m.invalidate(pair.clientid), None);
    assert_eq!(m.confirm(pair), Err(Error::StaleClientId));
}

#[test]
fn wrong_pair_and_old_confirmation_cannot_change_callback() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let mut update = registration(1);
    if let Callback::Universal { ident, .. } = &mut update.callback {
        *ident = 99;
    }
    let next = m.register(update).unwrap();
    m.drc.clear(); // Exercise the confirmed/pending checks after a cache miss.
    assert_eq!(m.confirm(first), Err(Error::StaleClientId));
    let wrong = PendingClient { confirmation: ConfirmationVerifier::new([0; 8]), ..next };
    assert_eq!(m.confirm(wrong), Err(Error::StaleClientId));
    let _ = m.confirm(next).unwrap();
}

#[test]
fn invalidate_removes_shared_id_pending_but_not_new_incarnation() {
    for verifier in [1, 2] {
        let mut m = manager();
        let first = confirmed(&mut m);
        let pair = m.register(registration(verifier)).unwrap();
        assert!(m.invalidate(first.clientid).is_some());
        assert!(m.invalidate(first.clientid).is_none());
        m.drc.clear(); // Check record removal rather than a historical cached reply.
        let result = m.confirm(pair);
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
    assert!(m.register(input.clone()).is_ok());
    input.identity = ClientIdentity::Handshake(vec![0; 1025]);
    assert_eq!(m.register(input.clone()), Err(Error::InvalidIdentity));
    input.identity = ClientIdentity::Implicit {
        address: "192.0.2.1".parse().unwrap(),
        host_name: HostName::new("host-one".into()).unwrap(),
    };
    assert_eq!(m.register(input.clone()), Err(Error::InvalidIdentity));
    let _ = m.new_unchecked(input).unwrap();
    assert_eq!(m.new_unchecked(registration(1)), Err(Error::InvalidIdentity));
}

#[test]
fn server_restart_does_not_accept_old_clientid() {
    let pair = confirmed(&mut manager());
    let mut restarted = manager();
    assert_eq!(restarted.get(pair.clientid), Err(Error::StaleClientId));
    assert_eq!(restarted.confirm(pair), Err(Error::StaleClientId));
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
fn registration_moves_between_tables_without_cloning_identity() {
    let mut m = manager();
    let input = registration(1);
    let ClientIdentity::Handshake(bytes) = &input.identity else {
        panic!("expected handshake identity");
    };
    let original_buffer = bytes.as_ptr();
    let pair = m.register(input).unwrap();
    let ClientIdentity::Handshake(bytes) = &m.pending[&pair.clientid].registration.identity else {
        panic!("expected handshake identity");
    };
    assert_eq!(bytes.as_ptr(), original_buffer);
    let _ = m.confirm(pair).unwrap();
    let ClientIdentity::Handshake(bytes) = &m.get(pair.clientid).unwrap().identity else {
        panic!("expected handshake identity");
    };
    assert_eq!(bytes.as_ptr(), original_buffer);
}

#[test]
fn drc_replays_historical_results_without_restoring_registration() {
    let mut m = manager();
    let pair = confirmed(&mut m);
    assert_eq!(m.invalidate(pair.clientid), Some(pair.clientid));
    assert!(m.confirm(pair).unwrap().retired.is_none());
    assert_eq!(m.register(registration(1)), Ok(pair));
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));

    m.drc.clear();
    assert_eq!(m.confirm(pair), Err(Error::StaleClientId));
    let replacement = m.register(registration(1)).unwrap();
    assert_ne!(pair.clientid, replacement.clientid);
    let _ = m.confirm(replacement).unwrap();
}

#[test]
fn drc_eviction_reprocesses_identical_arguments() {
    let mut m = manager();
    let first = m.register(registration(1)).unwrap();
    for ident in 2..=DRC_CAPACITY as u32 + 1 {
        let mut input = registration(1);
        if let Callback::Universal { ident: callback_ident, .. } = &mut input.callback {
            *callback_ident = ident;
        }
        m.register(input).unwrap();
    }
    let next = m.register(registration(1)).unwrap();
    assert_ne!(first.clientid, next.clientid);
    assert_ne!(first.confirmation, next.confirmation);
    assert_eq!(m.confirm(first), Err(Error::StaleClientId));
    let _ = m.confirm(next).unwrap();
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
