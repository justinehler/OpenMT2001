use super::*;
use crate::network::lobby::{
    Settings,
    tests::{names, settings},
};
fn owner() -> &'static Owner {
    Box::leak(Box::new(Owner::new().unwrap()))
}
fn wait(handle: &Handle, mut accept: impl FnMut(&Update) -> bool) -> Update {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(update) = handle.poll() {
            if accept(&update) {
                return update;
            }
            if let Update::Failed(error) = update {
                panic!("{error}");
            }
        }
        assert!(Instant::now() < deadline, "worker result timed out");
        thread::sleep(Duration::from_millis(5));
    }
}
fn host_open(settings: Settings) -> Open {
    Open::Host {
        address: "127.0.0.1:0".parse().unwrap(),
        snapshot: Snapshot::host(settings, b"Host".to_vec(), 0).unwrap(),
        token_names: names(),
    }
}
#[test]
fn loopback_probe_join_edit_revision_start_and_leave_over_real_transport() {
    let host = Handle::spawn(owner(), host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = Handle::spawn(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    assert_eq!(selection.snapshot.settings, settings());
    assert_eq!(selection.snapshot.capacity(), (1, 6));
    drop(probe);
    let join = Handle::spawn(
        owner(),
        Open::Join {
            selection,
            name: b"Client".to_vec(),
            token: 3,
        },
    )
    .unwrap();
    let Update::Lobby(client) = wait(&join, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    assert!(!client.host);
    assert_eq!(
        client.snapshot.rows[client.snapshot.local_row(client.local_peer).unwrap()]
            .player
            .name,
        b"Client"
    );
    let revision = client.snapshot.revision;
    assert!(!client.snapshot.start_enabled());
    join.action(Action::Ready).unwrap();
    let Update::Lobby(ready) = wait(
        &join,
        |u| matches!(u, Update::Lobby(v) if v.snapshot.revision > revision),
    ) else {
        panic!()
    };
    assert!(ready.snapshot.start_enabled());
    wait(
        &host,
        |u| matches!(u, Update::Lobby(v) if v.snapshot.revision == ready.snapshot.revision),
    );
    host.action(Action::Start).unwrap();
    let Update::Notice(message) = wait(&host, |u| matches!(u, Update::Notice(_))) else {
        panic!()
    };
    assert_eq!(message, crate::network::lobby::START_BOUNDARY);
    drop(join);
    let Update::Lobby(left) = wait(
        &host,
        |u| matches!(u, Update::Lobby(v) if v.snapshot.capacity().0 == 1),
    ) else {
        panic!()
    };
    assert!(left.snapshot.revision > ready.snapshot.revision);
}
#[test]
fn stale_worker_retirement_cannot_close_replacement() {
    let owner = owner();
    let old = Handle::spawn(owner, host_open(settings())).unwrap();
    wait(&old, |u| matches!(u, Update::Lobby(_)));
    let attempt = old.attempt;
    drop(old);
    let next = Handle::spawn(owner, host_open(settings())).unwrap();
    wait(&next, |u| matches!(u, Update::Lobby(_)));
    assert_eq!(owner.retire(attempt), Err(network::Error::StaleAttempt));
    assert_eq!(owner.phase(), Ok(network::Phase::Lobby));
}
#[test]
fn blocking_connect_is_structurally_inside_worker_and_poll_is_nonblocking() {
    let source: String = include_str!("../worker.rs")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let spawn = source.find(".spawn(move||").unwrap();
    let run_call = source[spawn..].find("run(owner,attempt").unwrap() + spawn;
    let run_fn = source.find("fnrun(").unwrap();
    let connect = source.find("owner.connect(").unwrap();
    assert!(spawn < run_call && run_call < run_fn && run_fn < connect);
    assert_eq!(source.matches("owner.connect(").count(), 1);
    let owner = owner();
    let handle = Handle::spawn(owner, Open::Probe("127.0.0.1:1".parse().unwrap())).unwrap();
    let start = Instant::now();
    for _ in 0..100 {
        let _ = handle.poll();
    }
    assert!(start.elapsed() < Duration::from_millis(100));
}

#[test]
fn full_six_human_session_still_answers_read_only_probes() {
    let host = Handle::spawn(owner(), host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let mut clients = Vec::new();
    for i in 1..6 {
        let probe = Handle::spawn(owner(), Open::Probe(initial.address)).unwrap();
        let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
            panic!()
        };
        drop(probe);
        let join = Handle::spawn(
            owner(),
            Open::Join {
                selection,
                name: format!("Client {i}").into_bytes(),
                token: i,
            },
        )
        .unwrap();
        wait(&join, |u| matches!(u, Update::Lobby(_)));
        clients.push(join);
    }
    let probe = Handle::spawn(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    assert_eq!(selection.snapshot.capacity(), (6, 6));
    assert_eq!(clients.len(), 5);
}

#[test]
fn join_refuses_a_rehosted_session_selected_by_an_earlier_probe() {
    let host_owner = owner();
    let host = Handle::spawn(host_owner, host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = Handle::spawn(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    drop(probe);
    drop(host);
    let replacement = Handle::spawn(
        host_owner,
        Open::Host {
            address: initial.address,
            snapshot: Snapshot::host(settings(), b"Host".to_vec(), 0).unwrap(),
            token_names: names(),
        },
    )
    .unwrap();
    wait(&replacement, |u| matches!(u, Update::Lobby(_)));
    let join = Handle::spawn(
        owner(),
        Open::Join {
            selection,
            name: b"Client".to_vec(),
            token: 1,
        },
    )
    .unwrap();
    let Update::Failed(message) = wait(&join, |u| matches!(u, Update::Failed(_))) else {
        panic!()
    };
    assert!(message.contains("selected session changed"));
    assert_eq!(host_owner.phase(), Ok(network::Phase::Lobby));
}

#[test]
fn cancelling_an_unjoined_connection_keeps_existing_lobby_peers() {
    let host = Handle::spawn(owner(), host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = Handle::spawn(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    drop(probe);
    let client = Handle::spawn(
        owner(),
        Open::Join {
            selection,
            name: b"Client".to_vec(),
            token: 1,
        },
    )
    .unwrap();
    let Update::Lobby(joined) = wait(&client, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let extra = Owner::new().unwrap();
    let attempt = extra.begin().unwrap();
    extra
        .connect(attempt, initial.address, Duration::from_secs(1))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while extra.phase().unwrap() != network::Phase::Lobby {
        extra.pump(attempt, None).unwrap();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    extra.retire(attempt).unwrap();
    host.action(Action::Ready).unwrap();
    let Update::Lobby(next) = wait(
        &client,
        |u| matches!(u, Update::Lobby(v) if v.snapshot.revision > joined.snapshot.revision),
    ) else {
        panic!()
    };
    assert_eq!(next.snapshot.capacity().0, 2);
    assert_eq!(next.snapshot.revision, joined.snapshot.revision + 1);
}
