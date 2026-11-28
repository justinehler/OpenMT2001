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

#[cfg(feature = "lua4")]
pub(crate) fn playing_pair(with_ai: bool) -> (Handle, Handle, u8) {
    let host_owner = owner();
    let client_owner = owner();
    let mut open = host_open(settings());
    if let Open::Host { snapshot, .. } = &mut open
        && with_ai
    {
        snapshot.action(Action::Kind(2), &names()).unwrap();
    }
    let host = spawn_worker(host_owner, open).unwrap();
    let Update::Lobby(initial) = wait(&host, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    let probe = spawn_worker(owner(), Open::Probe(initial.address)).unwrap();
    let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
        panic!()
    };
    drop(probe);
    let client = spawn_worker(
        client_owner,
        Open::Join {
            selection,
            name: b"Client".to_vec(),
            token: if with_ai { 2 } else { 1 },
        },
    )
    .unwrap();
    let Update::Lobby(view) = wait(&client, |u| matches!(u, Update::Lobby(_))) else {
        panic!()
    };
    client.action(Action::Ready).unwrap();
    wait(
        &host,
        |u| matches!(u,Update::Lobby(v) if v.snapshot.start_enabled()),
    );
    host.action(Action::Start).unwrap();
    let Update::Prepare(ticket) = wait(&host, |u| matches!(u, Update::Prepare(_))) else {
        panic!()
    };
    let content = network::Content {
        level: ticket.view.snapshot.settings.level,
        parameter_bits: ticket.view.snapshot.settings.parameter_bits,
        difficulty: ticket.view.snapshot.settings.difficulty,
        fingerprint: [5; 32],
    };
    host.prepared(Ok(content)).unwrap();
    wait(&client, |u| matches!(u, Update::Prepare(_)));
    client.prepared(Ok(content)).unwrap();
    wait(&host, |u| matches!(u, Update::Validated));
    wait(&client, |u| matches!(u, Update::Validated));
    (host, client, view.local_peer)
}

#[cfg(feature = "lua4")]
#[test]
fn playing_worker_orders_real_commits_before_deadlines_and_resumes_retained_tasks() {
    use crate::{
        live_commands,
        network::{
            Message, Transfer,
            commands::{Adapter, Request},
        },
    };
    let (host, client, local_peer) = playing_pair(false);
    let hb = host.bind_world(100).unwrap();
    let cb = client.bind_world(200).unwrap();
    let mut authority = Adapter::new(hb, true, 0, vec![Some(0), Some(local_peer)]).unwrap();
    let mut replica = Adapter::new(cb, false, local_peer, vec![Some(0), Some(local_peer)]).unwrap();
    // The simulated host is already inside its 100..125 grant window.
    authority.admit_deadline(0, 125).unwrap();
    let (mut left, mut right) = (
        live_commands::Queue::default(),
        live_commands::Queue::default(),
    );
    let (mut lp, mut rp) = (
        crate::pause::State::default(),
        crate::pause::State::default(),
    );
    let (mut lt, mut rt) = (
        crate::timers::startup_tasks([0x5a; 20]),
        crate::timers::startup_tasks([0x5a; 20]),
    );
    for category in [7, 8] {
        replica
            .submit(
                &client,
                live_commands::Command::PropertyTransfer {
                    category,
                    owner: 2,
                    amount: 500,
                },
                &mut right,
                &mut [[0; 0x17c]; 52],
                crate::network::commands::Targets::default(),
                100,
            )
            .unwrap();
    }
    let deadline = Instant::now() + WAIT;
    // Don't publish deadline125 before all requests in this test are admitted.
    // Pump at101: the host stamps126; at125 it grants150.
    while left.snapshot().unwrap().len() < 2 {
        authority
            .pump(
                &host,
                101,
                &mut left,
                &mut [[0; 0x17c]; 52],
                crate::network::commands::Targets::default(),
                &mut lp,
                &mut lt,
            )
            .unwrap();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    authority
        .pump(
            &host,
            125,
            &mut left,
            &mut [[0; 0x17c]; 52],
            crate::network::commands::Targets::default(),
            &mut lp,
            &mut lt,
        )
        .unwrap();
    crate::network::commands::pause_at_limit(&mut rp, &mut rt).unwrap();
    let retained = rt.clone();
    while replica.deadline() < 150 {
        replica
            .pump(
                &client,
                125,
                &mut right,
                &mut [[0; 0x17c]; 52],
                crate::network::commands::Targets::default(),
                &mut rp,
                &mut rt,
            )
            .unwrap();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(right.snapshot().unwrap().len(), 2);
    assert!(
        right
            .snapshot()
            .unwrap()
            .iter()
            .all(|(_, due, inner, _, _)| *due == 126 && *inner == 126)
    );
    assert_eq!(rp.bytes[1], 0);
    for (paused, resumed) in retained.iter().zip(&rt) {
        assert_eq!(&paused[1..], &resumed[1..]);
    }
    // A later frame at the SAME125 Total cannot commit a new request at150:
    // the client already has permission to run through that deadline.
    replica
        .submit(
            &client,
            live_commands::Command::PropertyTransfer {
                category: 9,
                owner: 2,
                amount: 10,
            },
            &mut right,
            &mut [[0; 0x17c]; 52],
            crate::network::commands::Targets::default(),
            125,
        )
        .unwrap();
    authority
        .pump(
            &host,
            125,
            &mut left,
            &mut [[0; 0x17c]; 52],
            crate::network::commands::Targets::default(),
            &mut lp,
            &mut lt,
        )
        .unwrap();
    assert_eq!(left.snapshot().unwrap().len(), 2);
    while left.snapshot().unwrap().len() < 3 {
        authority
            .pump(
                &host,
                126,
                &mut left,
                &mut [[0; 0x17c]; 52],
                crate::network::commands::Targets::default(),
                &mut lp,
                &mut lt,
            )
            .unwrap();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(left.snapshot().unwrap()[2].1, 151);
    // Unsupported family reaches an explicit host rejection, never the queue.
    client
        .send_gameplay(
            cb,
            None,
            Message::CommandRequest(Request {
                serial: 10,
                owner: 2,
                kind: 0x0f,
                payload: vec![],
            }),
        )
        .unwrap();
    let error = loop {
        authority
            .pump(
                &host,
                126,
                &mut left,
                &mut [[0; 0x17c]; 52],
                crate::network::commands::Targets::default(),
                &mut lp,
                &mut lt,
            )
            .unwrap();
        if let Err(error) = replica.pump(
            &client,
            125,
            &mut right,
            &mut [[0; 0x17c]; 52],
            crate::network::commands::Targets::default(),
            &mut rp,
            &mut rt,
        ) {
            break error;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    };
    assert!(error.contains("unsupported family/schema"));
    assert_eq!(left.snapshot().unwrap().len(), 3);
    // Bare legacy commits have no global command order and are not an escape
    // hatch into the qualified command schema.
    assert!(
        client
            .send_gameplay(
                cb,
                None,
                Message::Transfer(Transfer {
                    category: 7,
                    owner: 2,
                    amount: 1,
                    due: 150
                })
            )
            .is_err()
    );
}

fn chat_lines(handle: &Handle, count: usize) -> network::chat::View {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(view) = handle.chat_view()
            && view.lines.len() == count
        {
            return view;
        }
        if let Some(Update::Failed(error)) = handle.poll() {
            panic!("{error}");
        }
        assert!(
            Instant::now() < deadline,
            "chat ring did not reach {count} lines"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
fn chat_join(host: &Handle, name: &[u8], token: i8) -> Handle {
    let initial = host.chat_view().unwrap();
    let view = match wait(host, |u| matches!(u, Update::Lobby(_))) {
        Update::Lobby(view) => view,
        _ => unreachable!(),
    };
    let join = spawn_worker(
        owner(),
        Open::Join {
            selection: Selection {
                address: view.address,
                session: session(host.owner, host.attempt).unwrap(),
                snapshot: initial.snapshot,
            },
            name: name.to_vec(),
            token,
        },
    )
    .unwrap();
    wait(&join, |u| matches!(u, Update::Lobby(_)));
    join
}
#[test]
fn chat_submit_transport_receive_ring_group_selected_and_admitted_identity() {
    let host = spawn_worker(owner(), host_open(settings())).unwrap();
    // Keep the initial update for chat_join, which also waits for admission.
    let deadline = Instant::now() + WAIT;
    while host.chat_view().is_none() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let first = chat_join(&host, b"Blue", 3);
    let second = chat_join(&host, b"Green", 4);
    let deadline = Instant::now() + WAIT;
    while first.chat_view().unwrap().snapshot.capacity().0 != 3 {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    assert!(host.submit_chat(b"lobby".to_vec(), None).unwrap());
    for peer in [&host, &first, &second] {
        let view = chat_lines(peer, 1);
        assert_eq!(view.lines[0].text, b"Host: lobby");
        assert_eq!(view.lines[0].token, 0);
    }
    let first_view = first.chat_view().unwrap();
    let local = first_view
        .snapshot
        .local_row(first_view.local_peer)
        .unwrap();
    let target = first_view
        .snapshot
        .local_row(second.chat_view().unwrap().local_peer)
        .unwrap();
    let mut recipients = network::chat::Recipients::default();
    recipients.all();
    assert_eq!(recipients.0, [true; SLOTS]);
    assert_eq!(recipients.mask(local).count_ones(), 5);
    recipients.clear();
    assert_eq!(recipients.0, [false; SLOTS]);
    assert!(
        !first
            .submit_chat(b"nobody".to_vec(), Some(recipients))
            .unwrap()
    );
    recipients.0[local] = true;
    assert!(
        !first
            .submit_chat(b"self".to_vec(), Some(recipients))
            .unwrap()
    );
    recipients.0[target] = true;
    assert!(
        first
            .submit_chat(b"private".to_vec(), Some(recipients))
            .unwrap()
    );
    for peer in [&first, &second] {
        let view = chat_lines(peer, 2);
        assert_eq!(view.lines[1].text, b"Blue: private");
        assert_eq!((view.lines[1].row, view.lines[1].token), (local, 3));
        let colors = [
            0xffbd0000, 0xff0d00f1, 0xff00bd00, 0xff0000bd, 0xff00bdbd, 0xffbdbd00, 0xffbd00bd,
            0xff7f7f7f, 0xffbd7f00, 0xffbd7f7f,
        ];
        assert_eq!(view.lines[1].color(&colors), 0xff0000bd);
    }
    // The relay host was not selected and did not display the private line.
    assert_eq!(host.chat_view().unwrap().lines.len(), 1);
    assert!(!first.submit_chat(vec![], None).unwrap());
    assert!(first.submit_chat(b"everyone".to_vec(), None).unwrap());
    chat_lines(&host, 2);
    chat_lines(&first, 3);
    chat_lines(&second, 3);
    // Earlier lines follow the sender's current admitted row: a character change
    // recolours them on every peer.
    first
        .action(Action::Character {
            row: local,
            direction: 1,
        })
        .unwrap();
    let deadline = Instant::now() + WAIT;
    let token = loop {
        let token = first.chat_view().unwrap().snapshot.rows[local].player.token;
        if token != 3 {
            break token;
        }
        assert!(Instant::now() < deadline, "character change not admitted");
        thread::sleep(Duration::from_millis(10));
    };
    for peer in [&host, &first, &second] {
        let deadline = Instant::now() + WAIT;
        while peer
            .chat_view()
            .unwrap()
            .lines
            .iter()
            .any(|l| l.row == local && l.token != token)
        {
            assert!(Instant::now() < deadline, "chat line kept stale colour");
            thread::sleep(Duration::from_millis(10));
        }
    }
    // The same worker and ring survive lobby -> validated city binding.
    first.action(Action::Ready).unwrap();
    second.action(Action::Ready).unwrap();
    wait(
        &host,
        |u| matches!(u, Update::Lobby(v) if v.snapshot.start_enabled()),
    );
    host.action(Action::Start).unwrap();
    wait(&host, |u| matches!(u, Update::Prepare(_)));
    let content = network::Content {
        level: settings().level,
        parameter_bits: settings().parameter_bits,
        difficulty: settings().difficulty,
        fingerprint: [7; 32],
    };
    host.prepared(Ok(content)).unwrap();
    for peer in [&first, &second] {
        wait(peer, |u| matches!(u, Update::Prepare(_)));
        peer.prepared(Ok(content)).unwrap();
    }
    for peer in [&host, &first, &second] {
        wait(peer, |u| matches!(u, Update::Validated));
        peer.bind_world(42).unwrap();
    }
    assert_eq!(host.chat_view().unwrap().lines[0].text, b"Host: lobby");
    // Selected submission after actual binding uses the same production route.
    assert!(
        first
            .submit_chat(b"city".to_vec(), Some(recipients))
            .unwrap()
    );
    chat_lines(&first, 4);
    chat_lines(&second, 4);
    // A client cannot choose who a line is from: the host attributes it to the admitted
    // connection it arrived on, whatever sender the payload names, and never relays it
    // back to that connection.
    let connection = {
        let state = first.owner.lock().unwrap();
        Connection {
            attempt: first.attempt,
            slot: 0,
            identity: state.peers[0].as_ref().unwrap().identity,
        }
    };
    for sender in [1, 0xdeadbeef] {
        first
            .owner
            .send(
                connection,
                Message::Chat(network::chat::Packet {
                    sender,
                    recipients: 0,
                    text: b"forged".to_vec(),
                }),
            )
            .unwrap();
    }
    for (peer, count) in [(&host, 4), (&second, 6)] {
        let view = chat_lines(peer, count);
        for line in &view.lines[count - 2..] {
            assert_eq!(line.text, b"Blue: forged");
            assert_eq!((line.row, line.token), (local, token));
        }
    }
    // No relay returns to first: its four submits each echoed exactly once;
    // no-recipient, self-only and empty submits never occupied the ring.
    thread::sleep(Duration::from_millis(200));
    assert_eq!(first.chat_view().unwrap().lines.len(), 4);
}
