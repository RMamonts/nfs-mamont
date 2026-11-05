use super::*;

fn manager() -> ClientIdManager {
    ClientIdManager::new()
}

fn request(xid: u32) -> RequestId {
    RequestId { scope: 1, xid }
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
fn drc_replays_setclientid_but_miss_replaces_pending() {
    let mut m = manager();
    let first = m.register(request(1), registration(1)).unwrap();
    assert_eq!(m.register(request(1), registration(1)).unwrap(), first);
    let second = m.register(request(2), registration(1)).unwrap();
    assert_ne!(first.clientid, second.clientid);
    assert_ne!(first.confirmation, second.confirmation);
    assert_eq!(m.confirm(request(3), first), Err(Error::StaleClientId));
    let _ = m.confirm(request(4), second).unwrap();
}

#[test]
fn pending_reboot_is_replaced_even_with_identical_verifier() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let a = m.register(request(3), registration(2)).unwrap();
    let b = m.register(request(4), registration(2)).unwrap();
    assert_ne!(a.clientid, b.clientid);
    assert!(m.get(first.clientid).is_ok());
    assert_eq!(m.confirm(request(5), a), Err(Error::StaleClientId));
    assert!(m.confirm(request(6), b).unwrap().retired.is_some());
}

#[test]
fn invalidated_pending_cannot_be_confirmed() {
    let mut m = manager();
    let pair = m.register(request(1), registration(1)).unwrap();
    assert_eq!(m.invalidate(pair.clientid), None);
    assert_eq!(m.confirm(request(2), pair), Err(Error::StaleClientId));
}

#[test]
fn wrong_pair_and_old_confirmation_cannot_change_callback() {
    let mut m = manager();
    let first = confirmed(&mut m);
    let next = m.register(request(3), registration(1)).unwrap();
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
    assert!(m.register(request(1), input.clone()).is_ok());
    input.identity = ClientIdentity::Handshake(vec![0; 1025]);
    assert_eq!(m.register(request(2), input.clone()), Err(Error::InvalidIdentity));
    input.identity = ClientIdentity::Implicit {
        address: "192.0.2.1".parse().unwrap(),
        host_name: HostName::new("host-one".into()).unwrap(),
    };
    assert_eq!(m.register(request(3), input.clone()), Err(Error::InvalidIdentity));
    let _ = m.new_unchecked(input).unwrap();
    assert_eq!(m.new_unchecked(registration(1)), Err(Error::InvalidIdentity));
}

#[test]
fn drc_eviction_and_argument_matching() {
    let mut m = manager();
    let a = m.register(request(1), registration(1)).unwrap();
    let b = m.register(request(1), registration(2)).unwrap();
    assert_ne!(a, b); // same XID with different arguments is not a retransmission
    for xid in 2..=DRC_CAPACITY as u32 {
        m.register(request(xid), registration(2)).unwrap();
    }
    let c = m.register(request(1), registration(1)).unwrap();
    assert_ne!(a, c); // original entry was evicted
}

#[test]
fn server_restart_does_not_accept_old_clientid() {
    let pair = confirmed(&mut manager());
    let mut restarted = manager();
    assert_eq!(restarted.get(pair.clientid), Err(Error::StaleClientId));
    assert_eq!(restarted.confirm(request(1), pair), Err(Error::StaleClientId));
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
fn drc_shares_registration_and_retains_it_after_invalidation() {
    let mut m = manager();
    let pair = m.register(request(1), registration(1)).unwrap();
    let Some(Cached::Register { registration: cached, .. }) = m.drc.front() else {
        panic!("expected cached registration");
    };
    assert!(Arc::ptr_eq(cached, &m.pending[&pair.clientid].registration));

    let _ = m.confirm(request(2), pair).unwrap();
    let Some(Cached::Register { registration: cached, .. }) = m.drc.front() else {
        panic!("expected cached registration");
    };
    assert!(Arc::ptr_eq(cached, &m.confirmed[&pair.clientid].registration));
    assert_eq!(m.invalidate(pair.clientid), Some(pair.clientid));
    assert_eq!(m.register(request(1), registration(1)), Ok(pair));
    assert_eq!(m.get(pair.clientid), Err(Error::StaleClientId));
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
