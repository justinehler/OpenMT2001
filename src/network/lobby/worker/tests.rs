use super::*;
use crate::network::lobby::{
    Settings,
    tests::{names, settings},
};
// This bound catches hangs without imposing production response times on CI.
const WAIT: Duration = Duration::from_secs(60);

fn spawn_worker(owner: &'static Owner, open: Open) -> Result<Handle, String> {
    Handle::spawn_with_timeout(owner, open, WAIT)
}
fn owner() -> &'static Owner {
    Box::leak(Box::new(Owner::new().unwrap()))
}
fn wait(handle: &Handle, mut accept: impl FnMut(&Update) -> bool) -> Update {
    let deadline = Instant::now() + WAIT;
    loop {
        let update = handle
            .updates
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("worker result timed out or channel closed");
        if accept(&update) {
            return update;
        }
        if let Update::Failed(error) = update {
            panic!("{error}");
        }
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
    let host = spawn_worker(owner(), host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = spawn_worker(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    assert_eq!(selection.snapshot.settings, settings());
    assert_eq!(selection.snapshot.capacity(), (1, 6));
    drop(probe);
    let join = spawn_worker(
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
    let Update::Prepare(ticket) = wait(&host, |u| matches!(u, Update::Prepare(_))) else {
        panic!()
    };
    assert_eq!(ticket.view.snapshot, ready.snapshot);
    let content = network::Content {
        level: ticket.view.snapshot.settings.level,
        parameter_bits: ticket.view.snapshot.settings.parameter_bits,
        difficulty: ticket.view.snapshot.settings.difficulty,
        fingerprint: [4; 32],
    };
    host.prepared(Ok(content)).unwrap();
    let Update::Prepare(client_ticket) = wait(&join, |u| matches!(u, Update::Prepare(_))) else {
        panic!()
    };
    assert_eq!(client_ticket.seed, ticket.seed);
    assert_eq!(client_ticket.expected, Some(content));
    assert_eq!(client_ticket.view.snapshot, ticket.view.snapshot);
    assert_eq!(host.owner.phase(), Ok(network::Phase::Lobby));
    join.prepared(Ok(content)).unwrap();
    wait(&host, |u| matches!(u, Update::Validated));
    wait(&join, |u| matches!(u, Update::Validated));
    assert_eq!(host.owner.phase(), Ok(network::Phase::Starting));
    let binding = host.bind_world(10).unwrap();
    let other = join.bind_world(11).unwrap();
    assert_eq!(host.validate_binding(binding), Ok(()));
    assert_eq!(join.validate_binding(other), Ok(()));
    drop(join);
    drop(host);
}
#[test]
fn stale_worker_retirement_cannot_close_replacement() {
    let owner = owner();
    let old = spawn_worker(owner, host_open(settings())).unwrap();
    wait(&old, |u| matches!(u, Update::Lobby(_)));
    let attempt = old.attempt;
    drop(old);
    let next = spawn_worker(owner, host_open(settings())).unwrap();
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
    let handle = spawn_worker(owner, Open::Probe("127.0.0.1:1".parse().unwrap())).unwrap();
    let start = Instant::now();
    for _ in 0..100 {
        let _ = handle.poll();
    }
    assert!(start.elapsed() < Duration::from_millis(100));
}

#[test]
fn full_six_human_session_still_answers_read_only_probes() {
    let host = spawn_worker(owner(), host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let mut clients = Vec::new();
    for i in 1..6 {
        let probe = spawn_worker(owner(), Open::Probe(initial.address)).unwrap();
        let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
            panic!()
        };
        drop(probe);
        let join = spawn_worker(
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
    let probe = spawn_worker(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    assert_eq!(selection.snapshot.capacity(), (6, 6));
    assert_eq!(clients.len(), 5);
}

#[test]
fn join_refuses_a_rehosted_session_selected_by_an_earlier_probe() {
    let host_owner = owner();
    let host = spawn_worker(host_owner, host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = spawn_worker(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    drop(probe);
    drop(host);
    let replacement = spawn_worker(
        host_owner,
        Open::Host {
            address: initial.address,
            snapshot: Snapshot::host(settings(), b"Host".to_vec(), 0).unwrap(),
            token_names: names(),
        },
    )
    .unwrap();
    wait(&replacement, |u| matches!(u, Update::Lobby(_)));
    let join = spawn_worker(
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
    let host = spawn_worker(owner(), host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = spawn_worker(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    drop(probe);
    let client = spawn_worker(
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
    extra.connect(attempt, initial.address, WAIT).unwrap();
    let deadline = Instant::now() + WAIT;
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

#[test]
fn mismatched_content_never_releases_construction() {
    let host = spawn_worker(owner(), host_open(settings())).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = spawn_worker(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    drop(probe);
    let join = spawn_worker(
        owner(),
        Open::Join {
            selection,
            name: b"Other".to_vec(),
            token: 2,
        },
    )
    .unwrap();
    wait(&join, |u| matches!(u, Update::Lobby(_)));
    join.action(Action::Ready).unwrap();
    wait(
        &host,
        |u| matches!(u, Update::Lobby(v) if v.snapshot.start_enabled()),
    );
    host.action(Action::Start).unwrap();
    let Update::Prepare(ticket) = wait(&host, |u| matches!(u, Update::Prepare(_))) else {
        panic!()
    };
    let mut content = network::Content {
        level: ticket.view.snapshot.settings.level,
        parameter_bits: ticket.view.snapshot.settings.parameter_bits,
        difficulty: ticket.view.snapshot.settings.difficulty,
        fingerprint: [3; 32],
    };
    host.prepared(Ok(content)).unwrap();
    wait(&join, |u| matches!(u, Update::Prepare(_)));
    content.fingerprint[0] ^= 1;
    join.prepared(Ok(content)).unwrap();
    let Update::Failed(error) = wait(&join, |u| matches!(u, Update::Failed(_))) else {
        panic!()
    };
    assert!(error.contains("digest mismatch"));
    let Update::Failed(error) = wait(&host, |u| matches!(u, Update::Failed(_))) else {
        panic!()
    };
    assert!(error.contains("could not validate") || error.contains("connection ended"));
    assert!(host.bind_world(1).is_err());
    assert!(join.bind_world(2).is_err());
}

#[test]
fn cancelling_content_staging_cannot_release_or_retire_a_replacement() {
    let owner = owner();
    let old = spawn_worker(owner, host_open(settings())).unwrap();
    wait(&old, |u| matches!(u, Update::Lobby(_)));
    old.action(Action::Kind(1)).unwrap();
    wait(
        &old,
        |u| matches!(u, Update::Lobby(v) if v.snapshot.start_enabled()),
    );
    old.action(Action::Start).unwrap();
    let Update::Prepare(ticket) = wait(&old, |u| matches!(u, Update::Prepare(_))) else {
        panic!()
    };
    let (entered, staged) = mpsc::sync_channel(1);
    let (release, resume) = mpsc::sync_channel(1);
    let ui_thread = thread::current().id();
    old.stage(move || {
        assert_ne!(thread::current().id(), ui_thread);
        entered.send(()).unwrap();
        resume.recv_timeout(WAIT).unwrap();
        Ok(network::Content {
            level: ticket.view.snapshot.settings.level,
            parameter_bits: ticket.view.snapshot.settings.parameter_bits,
            difficulty: ticket.view.snapshot.settings.difficulty,
            fingerprint: [8; 32],
        })
    })
    .unwrap();
    staged.recv_timeout(WAIT).unwrap();
    old.cancellation().cancel();
    assert!(old.validate().is_err());
    let next = spawn_worker(owner, host_open(settings())).unwrap();
    wait(&next, |u| matches!(u, Update::Lobby(_)));
    release.send(()).unwrap();
    drop(old);
    assert_eq!(owner.phase(), Ok(network::Phase::Lobby));
    assert!(next.validate().is_ok());
}
