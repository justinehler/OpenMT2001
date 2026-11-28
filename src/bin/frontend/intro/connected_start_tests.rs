use super::*;
use openmt2001::network::{
    Row,
    lobby::{Settings, Slot, Snapshot, worker::View},
};

fn settings() -> Settings {
    Settings {
        name: b"Shared city".to_vec(),
        level: 300001,
        parameter_bits: 3500f32.to_bits(),
        difficulty: 1,
        map: b"empty.lua".to_vec(),
    }
}
fn ticket() -> Ticket {
    let mut snapshot = Snapshot::host(settings(), b"Host".to_vec(), 4).unwrap();
    // Gaps, transport peers and compact player slots are independent identities.
    snapshot.rows[2] = Slot {
        player: Row {
            kind: RowKind::Ai,
            token: 6,
            ready: true,
            name: b"Native AI".to_vec(),
        },
        peer: None,
        difficulty: 2,
    };
    snapshot.rows[4] = Slot {
        player: Row {
            kind: RowKind::Human,
            token: 3,
            ready: true,
            name: b"Client".to_vec(),
        },
        peer: Some(4),
        difficulty: 0,
    };
    Ticket {
        view: View {
            snapshot,
            local_peer: 0,
            host: true,
            address: "127.0.0.1:2300".parse().unwrap(),
        },
        seed: 0x12345678,
        expected: None,
    }
}
fn inputs() -> Arc<SceneInputs> {
    let mut inputs = super::super::camera_producer_live_tests::inputs(
        openmt2001::fixtures::scratch("TEST_SCRATCH").join("connected-34"),
    );
    let (process, _) = crate::live_host::process_runtime::startup(&inputs.executable, "").unwrap();
    Arc::get_mut(&mut inputs).unwrap().process_runtime = Some(process);
    inputs
}
fn shared_state(
    session: &mut crate::fresh_city::Session<'_>,
    image: &openmt2001::pe::PeImage<'_>,
) -> Vec<Vec<u8>> {
    super::shared_state(session, image).unwrap()
}
#[test]
#[ignore = "requires original assets; real two-Session connected constructor and frame schedules"]
fn two_sessions_share_gameplay_and_live_rng_with_native_ai_and_distinct_local_humans() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let host_inputs = inputs();
            let mut client_inputs = inputs();
            let host_ticket = ticket();
            let host_stage = host_inputs.stage_connected(&host_ticket).unwrap();
            let mut client_ticket = host_ticket.clone();
            client_ticket.view.local_peer = 4;
            client_ticket.view.host = false;
            client_ticket.expected = Some(host_stage.content);
            let client_stage = client_inputs.stage_connected(&client_ticket).unwrap();
            assert_eq!(host_stage.content, client_stage.content);
            assert_eq!(host_stage.peers, [Some(0), None, Some(4)]);
            assert_eq!((host_stage.local_slot, client_stage.local_slot), (0, 2));
            // Execution must use the staged bytes even after the source paths
            // are unavailable. This changes no supplied file on disk.
            Arc::get_mut(&mut client_inputs).unwrap().maps =
                openmt2001::fixtures::scratch("TEST_SCRATCH").join("connected-34-no-reopen/MAPS");
            let mut host = host_inputs
                .construct_connected(&host_ticket, &host_stage, None)
                .unwrap();
            let mut client = client_inputs
                .construct_connected(&client_ticket, &client_stage, None)
                .unwrap();
            for inputs in [&host_inputs, &client_inputs] {
                let process = inputs.process_runtime.as_ref().unwrap();
                assert_eq!(process.last_city_seed().unwrap(), host_ticket.seed);
                assert!(
                    process.reseed_connected_city().is_err(),
                    "constructor must consume the process seed boundary exactly once"
                );
            }
            assert_eq!(host.roster.manager.players.len(), 3);
            assert_eq!(client.roster.manager.players.len(), 3);
            assert_eq!(host.roster.status.ai_count, 1);
            assert_eq!(
                host.roster.status.current_player,
                host.roster.manager.players[0].reference
            );
            assert_eq!(
                client.roster.status.current_player,
                client.roster.manager.players[2].reference
            );
            assert_eq!(host.saved_player_metadata, client.saved_player_metadata);
            let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
            for (slot, (kind, peer)) in [(0, 0), (1, u32::MAX), (0, 4)].into_iter().enumerate() {
                assert_eq!(word(&host.saved_player_metadata[slot], 4), kind);
                assert_eq!(word(&host.saved_player_metadata[slot], 40), peer);
                assert_eq!(word(&host.roster.manager.players[slot].bytes, 0x154), kind);
            }
            let image = openmt2001::pe::PeImage::parse(&host_inputs.executable).unwrap();
            let board = host_inputs.styled_board(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&host_inputs.build_file).unwrap();
            let assets = crate::fresh_city::CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            for (tag, (left, right)) in (2..=13).chain([16, 17, 18]).zip(
                shared_state(&mut host, &image)
                    .into_iter()
                    .zip(shared_state(&mut client, &image)),
            ) {
                assert!(
                    left == right,
                    "Shared gameplay tag {tag} differs at {:?}",
                    left.iter().zip(&right).position(|(a, b)| a != b)
                );
            }
            let mut host_frame = crate::live_host::Host::new([100., 120., 100.]);
            let mut client_frame = crate::live_host::Host::new([120., 140., 120.]);
            for frame in [&mut host_frame, &mut client_frame] {
                frame.running = true;
                frame.policy.scheduler.network_enabled = true;
                frame.policy.scheduler.deadline = 25;
            }
            // Same 800 ms in forty short frames vs twenty longer frames.
            for _ in 0..20 {
                host_frame.frame(&mut host, &assets, 40);
            }
            for _ in 0..40 {
                client_frame.frame(&mut client, &assets, 20);
            }
            assert!(host_frame.failure.is_none(), "{:?}", host_frame.failure);
            assert!(client_frame.failure.is_none(), "{:?}", client_frame.failure);
            assert!(host.world.clock.timing[0] > 0);
            assert_eq!(host.world.clock, client.world.clock);
            for (tag, (left, right)) in (2..=13).chain([16, 17, 18]).zip(
                shared_state(&mut host, &image)
                    .into_iter()
                    .zip(shared_state(&mut client, &image)),
            ) {
                assert!(
                    left == right,
                    "Shared gameplay tag {tag} differs at {:?}",
                    left.iter().zip(&right).position(|(a, b)| a != b)
                );
            }
            assert_eq!(host.world.categories, client.world.categories);
            assert_eq!(
                host.world.ledgers.businesses,
                client.world.ledgers.businesses
            );
            assert_eq!(host.random_registry.process, client.random_registry.process);
            assert_eq!(
                (
                    host.random_registry.head,
                    host.random_registry.tail,
                    host.static_rng_tail_next
                ),
                (
                    client.random_registry.head,
                    client.random_registry.tail,
                    client.static_rng_tail_next
                )
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
#[ignore = "requires original connected content validation"]
fn rejects_missing_map_bad_parameters_version_and_compact_registration_mismatch() {
    let inputs = inputs();
    let original = ticket();
    let stage = inputs.stage_connected(&original).unwrap();
    let mut bad = original.clone();
    bad.view.snapshot.settings.map = b"does-not-exist.lua".to_vec();
    assert!(
        inputs
            .stage_connected(&bad)
            .err()
            .unwrap()
            .contains("map content")
    );
    let mut bad = original.clone();
    bad.view.snapshot.settings.parameter_bits = f32::NAN.to_bits();
    assert!(inputs.stage_connected(&bad).is_err());
    let mut bad = original.clone();
    bad.view.snapshot.settings.parameter_bits = 3500.5f32.to_bits();
    assert!(
        inputs
            .stage_connected(&bad)
            .err()
            .unwrap()
            .contains("parameter bits")
    );
    let mut bad = original.clone();
    bad.view.snapshot.settings.level = 200003;
    assert!(inputs.stage_connected(&bad).is_err());
    let mut bad = original.clone();
    bad.expected = Some(Content {
        fingerprint: [42; 32],
        ..stage.content
    });
    assert!(
        inputs
            .stage_connected(&bad)
            .err()
            .unwrap()
            .contains("digest mismatch")
    );
    let mut bad = original.clone();
    bad.expected = Some(stage.content);
    bad.view.snapshot.rows[4].player.name = b"Different admitted name".to_vec();
    assert!(
        inputs
            .stage_connected(&bad)
            .err()
            .unwrap()
            .contains("digest mismatch")
    );
    let mut bad = original;
    bad.view.snapshot.rows[4].peer = Some(0);
    assert!(inputs.stage_connected(&bad).is_err());
}

#[test]
fn connected_roster_does_not_weaken_offline_skirmish_validation() {
    use openmt2001::skirmish::roster::{Kind, Participant};
    let launch = crate::fresh_city::SkirmishLaunch {
        level_id: 300001,
        directory: "net1".into(),
        map: "empty.lua".into(),
        objective_kind: 6,
        target: 3500.,
        difficulty: 1,
        participants: [4, 3]
            .map(|token| Participant {
                kind: Kind::Human,
                token: Some(token),
                name: b"Human".to_vec(),
                ready: true,
                difficulty: 0,
            })
            .to_vec(),
    };
    assert!(launch.validate_roster(true).is_ok());
    assert!(launch.validate_roster(false).is_err());
}
/// Construct the staged host and client cities of `ticket()` and hand them,
/// with the host's command assets, to `body`.
fn with_connected_pair(
    body: impl for<'s> FnOnce(
        &mut crate::fresh_city::Session<'s>,
        &mut crate::fresh_city::Session<'s>,
        &crate::fresh_city::CommandAssets<'_, '_>,
        &openmt2001::pe::PeImage<'_>,
    ),
) {
    let host_inputs = inputs();
    let client_inputs = inputs();
    let host_ticket = ticket();
    let host_stage = host_inputs.stage_connected(&host_ticket).unwrap();
    let mut client_ticket = host_ticket.clone();
    client_ticket.view.local_peer = 4;
    client_ticket.view.host = false;
    client_ticket.expected = Some(host_stage.content);
    let client_stage = client_inputs.stage_connected(&client_ticket).unwrap();
    let mut host = host_inputs
        .construct_connected(&host_ticket, &host_stage, None)
        .unwrap();
    let mut client = client_inputs
        .construct_connected(&client_ticket, &client_stage, None)
        .unwrap();
    let image = openmt2001::pe::PeImage::parse(&host_inputs.executable).unwrap();
    let board = host_inputs.styled_board(&image).unwrap();
    let file = openmt2001::build_file::BuildFile::parse(&host_inputs.build_file).unwrap();
    let assets = crate::fresh_city::CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    body(&mut host, &mut client, &assets, &image);
}
#[test]
#[ignore = "requires original assets; real two-Session connected frames under an open Options menu"]
fn open_options_keeps_both_connected_cities_ticking_and_never_saves() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            with_connected_pair(|host, client, assets, image| {
                let saves = openmt2001::fixtures::scratch("TEST_SCRATCH").join("connected-37");
                let _ = std::fs::remove_dir_all(&saves);
                std::fs::create_dir_all(saves.join("saves")).unwrap();
                let mut host_frame = crate::live_host::Host::new([100., 120., 100.]);
                let mut client_frame = crate::live_host::Host::new([120., 140., 120.]);
                for frame in [&mut host_frame, &mut client_frame] {
                    frame.running = true;
                    frame.policy.scheduler.network_enabled = true;
                    // No worker grants tick-ceiling windows here, so both
                    // peers start inside one wide window.
                    frame.policy.scheduler.deadline = 100_000;
                    frame.autosave_profile_root = Some(saves.clone());
                }
                for _ in 0..10 {
                    host_frame.frame(host, assets, 40);
                    client_frame.frame(client, assets, 40);
                }
                let opened_at = host.world.clock.timing[0];
                assert!(opened_at > 0);
                // Both peers open Options through the same Host operation the
                // menu's acknowledgement waits on.
                for (frame, session) in [
                    (&mut host_frame, &mut *host),
                    (&mut client_frame, &mut *client),
                ] {
                    frame
                        .options(&mut session.world.pause, &mut session.hooks.timers, true)
                        .unwrap();
                    assert!(frame.running);
                    assert_eq!(session.world.pause.bytes[0], 0);
                    assert!(frame.status(session).network_enabled);
                }
                // Fast speed stays refused while the menu is up.
                for frame in [&mut host_frame, &mut client_frame] {
                    frame.toggle_speed();
                    frame.set_fast(true);
                    assert_eq!(frame.policy.scheduler.iterations, 1);
                }
                // Both cities keep ticking in step under the open menu.
                for _ in 0..10 {
                    host_frame.frame(host, assets, 40);
                    client_frame.frame(client, assets, 40);
                }
                assert!(host.world.clock.timing[0] > opened_at);
                assert_eq!(host.world.clock, client.world.clock);
                for (tag, (left, right)) in (2..=13).chain([16, 17, 18]).zip(
                    shared_state(host, image)
                        .into_iter()
                        .zip(shared_state(client, image)),
                ) {
                    assert!(left == right, "Shared gameplay tag {tag} differs");
                }
                // Run both cities across a midnight with the menu open.
                for session in [&mut *host, &mut *client] {
                    let clock = &mut session.world.clock;
                    clock.calendar[0] = 23;
                    clock.calendar[3] = 23 + (clock.calendar[1] + clock.calendar[2] * 10) * 24;
                    // AdvanceHour follows the normal-tick word (timing 1).
                    clock.timing[1] = 620;
                    clock.timing[2] = 620;
                    clock.timing[3] = 14995;
                }
                let day = host.world.clock.calendar[4];
                for _ in 0..40 {
                    host_frame.frame(host, assets, 40);
                    client_frame.frame(client, assets, 40);
                }
                for frame in [&host_frame, &client_frame] {
                    assert!(frame.failure.is_none(), "{:?}", frame.failure);
                    assert!(frame.autosave_status.is_none());
                }
                assert_eq!(host.world.clock, client.world.clock);
                assert_ne!(host.world.clock.calendar[4], day, "no midnight passed");
                assert_eq!(std::fs::read_dir(saves.join("saves")).unwrap().count(), 0);
                // A manual save reaching the write is refused on both peers,
                // with Options open or closed.
                for (frame, session) in [
                    (&mut host_frame, &mut *host),
                    (&mut client_frame, &mut *client),
                ] {
                    assert!(
                        !frame.options_save_allowed(&session.world.pause, &session.hooks.timers)
                    );
                    assert!(frame.save_view(session).is_err());
                    frame
                        .options(&mut session.world.pause, &mut session.hooks.timers, false)
                        .unwrap();
                    assert!(frame.running);
                    assert!(frame.save_view(session).is_err());
                    assert_eq!(session.world.pause.bytes[0], 0);
                }
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
/// Two peers' tag-2 rosters that differ only in what the original keeps per
/// peer (inbox words, held card, auxiliary descriptors) normalize to the same
/// bytes; any other player byte still differs.
#[test]
fn per_peer_player_state_is_blanked_and_shared_state_is_kept() {
    let roster = |held: u8, inbox: u8, auxiliaries: u32, cash: u8| {
        let mut record = 1u32.to_le_bytes().to_vec();
        record.extend_from_slice(&[0; 44]);
        let mut player = vec![0u8; 0x238];
        player[0x1b8] = held;
        player[0x214] = inbox;
        player[0x10] = cash;
        record.extend_from_slice(&7u32.to_le_bytes());
        record.extend_from_slice(&(player.len() as u32).to_le_bytes());
        record.extend_from_slice(&player);
        record.extend_from_slice(&auxiliaries.to_le_bytes());
        for index in 0..auxiliaries {
            record.extend_from_slice(&(0x2000_5000 + index).to_le_bytes());
            record.extend_from_slice(&[0x33; 48]);
        }
        record
    };
    let mut host = roster(0, 0, 1, 5);
    let mut client = roster(0x21, 2, 3, 5);
    per_peer_players(&mut host).unwrap();
    per_peer_players(&mut client).unwrap();
    assert_eq!(host, client);
    let mut other = roster(0, 0, 1, 6);
    per_peer_players(&mut other).unwrap();
    assert_ne!(host, other);
    let mut truncated = roster(0, 0, 1, 5);
    truncated.pop();
    assert!(per_peer_players(&mut truncated).is_err());
}

/// Construct one staged city and link it to its lobby worker exactly as
/// `fresh_connected_joined` does.
fn link<'s>(
    inputs: &'s SceneInputs,
    ticket: &Ticket,
    stage: &Staged,
    worker: Handle,
) -> crate::fresh_city::Session<'s> {
    let mut session = inputs.construct_connected(ticket, stage, None).unwrap();
    let binding = worker
        .bind_world(session.world.topology.epoch().serial())
        .unwrap();
    let commands = network::commands::Adapter::new(
        binding,
        ticket.view.host,
        ticket.view.local_peer,
        stage.peers.clone(),
    )
    .unwrap();
    let mut synchronization = network::synchronization::Synchronization::new(
        binding,
        ticket.view.host,
        ticket.view.local_peer,
        stage.peers.clone(),
    )
    .unwrap();
    session.network_command_timer =
        Some(synchronization.enable(&mut session.hooks.timers).unwrap());
    synchronization
        .barrier
        .open(&mut session.world.city.pause, &mut session.hooks.timers)
        .unwrap();
    session.connected = Some(Connection {
        synchronization,
        commands,
        binding,
        worker,
    });
    session
}

/// A host and a client brought up through the real lobby over loopback and
/// linked as a connected start links them: host slot 0, client slot 1.
fn with_linked_pair(body: impl for<'s> FnOnce(&mut LinkedPair<'s, '_>)) {
    with_linked_peers(|[host, client], assets, image| {
        let mut frames = [
            crate::live_host::Host::new([100., 120., 100.]),
            crate::live_host::Host::new([120., 140., 120.]),
        ];
        for frame in &mut frames {
            frame.running = true;
            frame.policy.scheduler.network_enabled = true;
            frame.policy.scheduler.deadline = 25;
        }
        body(&mut LinkedPair {
            sessions: [host, client],
            frames,
            assets,
            image,
        });
    });
}

/// A host and `PEERS - 1` clients, each on its own transport owner as
/// separate processes would be, brought up through the real lobby over
/// loopback and linked as a connected start links them. Peer `n` is compact
/// player slot `n`; the host is peer 0.
fn with_linked_peers<const PEERS: usize>(
    body: impl for<'s> FnOnce(
        [crate::fresh_city::Session<'s>; PEERS],
        &crate::fresh_city::CommandAssets<'_, '_>,
        &openmt2001::pe::PeImage<'_>,
    ),
) {
    use openmt2001::network::lobby::{
        Action,
        worker::{Open, Update},
    };
    fn wait(worker: &Handle, mut accept: impl FnMut(&Update) -> bool) -> Update {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            while let Some(update) = worker.poll() {
                if let Update::Failed(error) = &update {
                    panic!("{error}");
                }
                if accept(&update) {
                    return update;
                }
            }
            assert!(std::time::Instant::now() < until, "lobby update never came");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let names: [Vec<u8>; 10] = std::array::from_fn(|i| format!("Character {i}").into_bytes());
    let host_worker = Handle::open_isolated(Open::Host {
        address: "127.0.0.1:0".parse().unwrap(),
        snapshot: Snapshot::host(settings(), b"Host".to_vec(), 4).unwrap(),
        token_names: names,
    })
    .unwrap();
    let Update::Lobby(initial) = wait(&host_worker, |u| matches!(u, Update::Lobby(_))) else {
        unreachable!()
    };
    let mut workers = vec![host_worker];
    for client in 1..PEERS {
        let probe = Handle::open_isolated(Open::Probe(initial.address)).unwrap();
        let Update::Probed(selection) = wait(&probe, |u| matches!(u, Update::Probed(_))) else {
            unreachable!()
        };
        drop(probe);
        let worker = Handle::open_isolated(Open::Join {
            selection,
            name: if client == 1 {
                b"Client".to_vec()
            } else {
                format!("Client {client}").into_bytes()
            },
            token: 4 - client as i8,
        })
        .unwrap();
        wait(&worker, |u| matches!(u, Update::Lobby(_)));
        worker.action(Action::Ready).unwrap();
        workers.push(worker);
    }
    wait(&workers[0], |u| {
        matches!(u, Update::Lobby(v)
            if v.snapshot.capacity().0 == PEERS && v.snapshot.start_enabled())
    });
    workers[0].action(Action::Start).unwrap();
    let inputs: [_; PEERS] = std::array::from_fn(|_| inputs());
    let staged: Vec<_> = workers
        .iter()
        .zip(&inputs)
        .map(|(worker, inputs)| {
            let Update::Prepare(ticket) = wait(worker, |u| matches!(u, Update::Prepare(_))) else {
                unreachable!()
            };
            let stage = inputs.stage_connected(&ticket).unwrap();
            worker.prepared(Ok(stage.content)).unwrap();
            (ticket, stage)
        })
        .collect();
    for worker in &workers {
        wait(worker, |u| matches!(u, Update::Validated));
    }
    let mut peers = workers.into_iter().zip(&staged).zip(&inputs);
    let sessions: [_; PEERS] = std::array::from_fn(|_| {
        let ((worker, (ticket, stage)), inputs) = peers.next().unwrap();
        link(inputs, ticket, stage, worker)
    });
    let image = openmt2001::pe::PeImage::parse(&inputs[0].executable).unwrap();
    let board = inputs[0].styled_board(&image).unwrap();
    let file = openmt2001::build_file::BuildFile::parse(&inputs[0].build_file).unwrap();
    let assets = crate::fresh_city::CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    body(sessions, &assets, &image);
}

/// Record 12 without its activity timer list. Those timers accumulate each
/// peer's own frame time, and `LinkedPair::align` runs one peer's frames alone.
/// The list follows the utility and property words (80 bytes), the pause
/// bytes, and three frequency words, as a length-prefixed blob.
fn without_activity_timers(record: &[u8], pause: usize) -> Vec<u8> {
    let at = 80 + pause + 12;
    let len = u32::from_le_bytes(record[at..at + 4].try_into().unwrap()) as usize;
    [&record[..at], &record[at + 4 + len..]].concat()
}

/// Run `body` with each player's last consumed inbox message hidden: the
/// message reference at +0x20c and the +0x210 scratch record it was copied to
/// (0x48bc70). A peer consumes only its own human's messages (0x450a30), so
/// this is local state.
fn without_local_inbox<T>(
    session: &mut crate::fresh_city::Session<'_>,
    body: impl FnOnce(&mut crate::fresh_city::Session<'_>) -> T,
) -> T {
    let word = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let saved: Vec<_> = session
        .roster
        .manager
        .players
        .iter_mut()
        .map(|player| {
            let pending: [u8; 4] = player.bytes[0x20c..0x210].try_into().unwrap();
            player.bytes[0x20c..0x210].fill(0);
            let scratch = word(&player.bytes, 0x210);
            let record = player
                .auxiliaries
                .iter_mut()
                .find(|a| a.reference == scratch)
                .map(|a| std::mem::replace(&mut a.bytes, [0; 0x30]));
            (pending, scratch, record)
        })
        .collect();
    let result = body(session);
    for (player, (pending, scratch, record)) in session.roster.manager.players.iter_mut().zip(saved)
    {
        player.bytes[0x20c..0x210].copy_from_slice(&pending);
        if let Some(record) = record
            && let Some(a) = player
                .auxiliaries
                .iter_mut()
                .find(|a| a.reference == scratch)
        {
            a.bytes = record;
        }
    }
    result
}

/// Peer 0 is the host, peer 1 the client; each human is its peer's slot.
struct LinkedPair<'s, 'a> {
    sessions: [crate::fresh_city::Session<'s>; 2],
    frames: [crate::live_host::Host; 2],
    assets: &'a crate::fresh_city::CommandAssets<'a, 'a>,
    image: &'a openmt2001::pe::PeImage<'a>,
}
impl LinkedPair<'_, '_> {
    /// One frame on each peer. Returns the dialogs each peer raised.
    fn step(&mut self) -> [Vec<(u32, u32)>; 2] {
        for peer in 0..2 {
            self.frames[peer].frame(&mut self.sessions[peer], self.assets, 40);
            assert!(
                self.frames[peer].failure.is_none(),
                "peer {peer}: {:?}",
                self.frames[peer].failure
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
        std::array::from_fn(|peer| {
            self.frames[peer]
                .take_prompts(&self.sessions[peer])
                .into_iter()
                .filter_map(|p| p.dialog)
                .map(|d| (d.callback, d.context))
                .collect()
        })
    }
    fn reply(&mut self, peer: usize, callback: u32, context: u32, button: u32) {
        let (frame, session) = (&mut self.frames[peer], &mut self.sessions[peer]);
        frame
            .human_dialog_reply(
                session,
                self.assets,
                openmt2001::human_dialog::Reply {
                    callback,
                    context,
                    button,
                },
            )
            .unwrap();
    }
    fn auctions(&self) -> [u32; 2] {
        self.sessions
            .each_ref()
            .map(|s| s.world.auction.controller.active_count)
    }
    /// Run only the peer that is behind until both reach the same tick. The
    /// host may run ahead of the client inside its granted window.
    fn align(&mut self, label: &str) {
        let mut frames = 0;
        loop {
            let ticks = self.sessions.each_ref().map(|s| s.world.clock.timing[0]);
            if ticks[0] == ticks[1] {
                return;
            }
            let behind = usize::from(ticks[1] < ticks[0]);
            self.frames[behind].frame(&mut self.sessions[behind], self.assets, 40);
            assert!(
                self.frames[behind].failure.is_none(),
                "peer {behind}: {:?}",
                self.frames[behind].failure
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
            frames += 1;
            assert!(
                frames < 2000,
                "{label}: peers never met at one tick {ticks:?}"
            );
        }
    }
    /// Compare the shared gameplay save sections of both peers at one tick and
    /// print the sample.
    fn sample(&mut self, label: &str) {
        self.align(label);
        let auctions = self.auctions();
        let [host, client] = &mut self.sessions;
        // The save writer refuses an auction whose bidding scene this
        // frame-only harness never builds, so while one is open the sample
        // compares the world it changes directly.
        let saved = auctions == [0, 0];
        if saved {
            let pause = host.world.pause.bytes.len();
            for (tag, (mut left, mut right)) in (2..=13).chain([16, 17, 18]).zip(
                without_local_inbox(host, |s| shared_state(s, self.image))
                    .into_iter()
                    .zip(without_local_inbox(client, |s| shared_state(s, self.image))),
            ) {
                if tag == 12 {
                    left = without_activity_timers(&left, pause);
                    right = without_activity_timers(&right, pause);
                }
                assert!(left == right, "{label}: shared gameplay tag {tag} differs");
            }
        }
        assert_eq!(host.world.clock, client.world.clock, "{label}");
        assert_eq!(host.world.categories, client.world.categories, "{label}");
        assert_eq!(
            host.world.auction.controller.record[..0x8c],
            client.world.auction.controller.record[..0x8c],
            "{label}: auction state differs"
        );
        println!(
            "{label}: tick {} on both peers, auctions {auctions:?}, {} identical",
            host.world.clock.timing[0],
            if saved {
                "shared gameplay sections"
            } else {
                "clock, categories and auction state"
            },
        );
    }
    /// The proposer's human offers, through the property panel, for the
    /// cheapest available property, which the other human owns. The owner's
    /// peer raises the offer modal (0x48bc70) and answers with `button`.
    fn offer(&mut self, proposer: usize, button: u32) -> u32 {
        let owner = 1 - proposer;
        let image = self.image;
        let categories = &self.sessions[0].world.categories;
        let category = (0..52u32)
            .filter(|&c| {
                let row = &categories[c as usize];
                row[0x74] != 0 && row[0x6c] == 0 && row[5] == 0
            })
            .min_by_key(|&c| {
                openmt2001::ai_messages::offers::execution::category_value(
                    &categories[c as usize],
                    image,
                )
                .unwrap()
                .to_f64() as i64
            })
            .expect("an available property");
        for s in &mut self.sessions {
            s.world.categories[category as usize][5] = owner as u8 + 1;
        }
        self.sample("before the offer");
        let side = &mut self.sessions[proposer];
        let panel = side
            .inspect_property_offer(category, None, self.assets)
            .unwrap()
            .panel;
        let submitted = side
            .submit_property_offer(category, panel.minimum, self.assets, |_| Ok(()))
            .unwrap();
        assert!(submitted.submitted, "offer for {category} not submitted");
        let mut frames = 0;
        while !self.step()[owner].contains(&(0x48b350, 0x23)) {
            frames += 1;
            assert!(
                frames < 2000,
                "offer for {category} never reached the owner"
            );
        }
        self.reply(owner, 0x48b350, 0x23, button);
        category
    }
}

#[test]
#[ignore = "requires original assets; two connected Sessions over the real loopback lobby"]
fn human_to_human_offers_accepted_and_rejected_match_on_both_peers() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            with_linked_pair(|pair| {
                let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
                let mut started = 0;
                while pair.sessions.iter().any(|s| s.world.clock.timing[0] < 50) {
                    pair.step();
                    started += 1;
                    assert!(started < 2000, "connected start never ran");
                }
                for (proposer, accept) in [(0, false), (1, false), (1, true), (0, true)] {
                    let label = format!(
                        "{} offer {}",
                        ["host->client", "client->host"][proposer],
                        if accept { "accepted" } else { "rejected" }
                    );
                    let busy = pair.auctions()[0] != 0;
                    let category = pair.offer(proposer, u32::from(!accept));
                    // A refusal, or an admission refused while another auction
                    // runs, is a kind-38 notice for the proposer only: one OK
                    // button and no callback (0x48bc70).
                    let notice = !accept || busy;
                    let mut frames = 0;
                    loop {
                        let dialogs = pair.step();
                        if notice && dialogs[proposer].contains(&(0, 0)) {
                            pair.reply(proposer, 0, 0, 0);
                            break;
                        }
                        if !notice && pair.auctions() == [1, 1] {
                            break;
                        }
                        frames += 1;
                        assert!(frames < 2000, "{label}: answer never arrived");
                    }
                    // Let any scheduled command land on both peers.
                    for _ in 0..40 {
                        pair.step();
                    }
                    for s in &pair.sessions {
                        for player in &s.roster.manager.players {
                            assert_eq!(word(&player.bytes, 0x214), 0, "{label}: inbox left over");
                        }
                    }
                    assert_eq!(pair.auctions(), [u32::from(accept || busy); 2], "{label}");
                    if accept && !busy {
                        let record = &pair.sessions[0].world.auction.controller.record;
                        assert_eq!(word(record, 4), category);
                        assert_eq!(word(record, 0x54), proposer as u32);
                    }
                    pair.sample(&label);
                }
            });
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Frame every peer by the same 40 ms until `done` holds; any simulation
/// failure ends the test.
fn run_connected(
    sessions: &mut [crate::fresh_city::Session<'_>],
    frames: &mut [crate::live_host::Host],
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    label: &str,
    mut done: impl FnMut(&[crate::fresh_city::Session<'_>], &mut [crate::live_host::Host]) -> bool,
) {
    let start = std::time::Instant::now();
    while !done(sessions, frames) {
        let ticks: Vec<_> = sessions.iter().map(|s| s.world.clock.timing[0]).collect();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(600),
            "{label}: never happened, peers at {ticks:?}"
        );
        for (peer, (frame, session)) in frames.iter_mut().zip(sessions.iter_mut()).enumerate() {
            frame.frame(session, assets, 40);
            assert!(frame.failure.is_none(), "peer {peer}: {:?}", frame.failure);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// 0x4e2740 -> 0x450670 during play, through the real lobby, transport and
/// Session drain: a client's connection ends mid-game. The host schedules
/// status (player, 3) and removal for one Total+25 tick; the host and the
/// remaining client both apply them there, show the "left the game" notice
/// (0x743/0x744), release the departed player's property, and play on.
#[test]
#[ignore = "requires original assets; three connected Sessions over the real loopback lobby"]
fn client_lost_mid_game_is_removed_at_the_same_tick_and_the_host_plays_on() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            with_linked_peers(|sessions: [_; 3], assets, image| {
                use openmt2001::live_commands::Command;
                let mut sessions = Vec::from(sessions);
                let mut frames: Vec<_> = (0..3)
                    .map(|_| {
                        let mut frame = crate::live_host::Host::new([100., 120., 100.]);
                        frame.running = true;
                        frame
                    })
                    .collect();
                run_connected(&mut sessions, &mut frames, assets, "start", |sessions, _| {
                    sessions.iter().all(|s| s.world.clock.timing[0] > 200)
                });
                // Peer 1 departs; its human is compact slot 1 on every peer.
                const DEPARTED: usize = 1;
                const CATEGORY: usize = 7;
                let owner = sessions[DEPARTED].roster.manager.players[DEPARTED].bytes[0x1bc] + 1;
                // The departing human first buys a property through the
                // connected command path.
                sessions[DEPARTED]
                    .submit_command(
                        Command::PropertyTransfer {
                            category: CATEGORY as i16,
                            owner,
                            amount: 500,
                        },
                        assets,
                        |_| Ok(()),
                    )
                    .unwrap();
                run_connected(&mut sessions, &mut frames, assets, "purchase", |sessions, _| {
                    sessions
                        .iter()
                        .all(|s| s.world.categories[CATEGORY][5] == owner)
                });
                // The departing client's process ends mid-game.
                drop(sessions.remove(DEPARTED));
                frames.remove(DEPARTED);
                let mut removal = [None; 2];
                let mut notice = [false; 2];
                run_connected(&mut sessions, &mut frames, assets, "removal", |sessions, frames| {
                    for (peer, (session, frame)) in
                        sessions.iter().zip(frames.iter_mut()).enumerate()
                    {
                        for (_, due, _, _, command) in session.live_commands.snapshot().unwrap() {
                            if matches!(command, Command::Eliminate { slot } if *slot == DEPARTED) {
                                removal[peer] = Some(due);
                            }
                        }
                        notice[peer] |= frame.status(session).prompts.iter().any(|request| {
                            request
                                .dialog
                                .as_ref()
                                .is_some_and(|d| (d.title, d.body) == (0x743, 0x744))
                        });
                    }
                    sessions
                        .iter()
                        .all(|s| s.roster.manager.players[DEPARTED].bytes[0x10] == 0)
                });
                // Host and remaining client scheduled the removal for one tick.
                assert!(removal[0].is_some(), "the host never queued the removal");
                assert_eq!(removal[0], removal[1]);
                let due = removal[0].unwrap();
                for session in &sessions {
                    assert!(session.world.clock.timing[0] >= due);
                    let player = &session.roster.manager.players[DEPARTED];
                    assert_eq!(player.bytes[0x10], 0);
                    assert_eq!(
                        u32::from_le_bytes(player.bytes[0x154..0x158].try_into().unwrap()),
                        2
                    );
                    assert!(
                        session
                            .world
                            .categories
                            .iter()
                            .all(|category| category[5] != owner),
                        "the departed player still owns land"
                    );
                }
                assert_eq!(notice, [true; 2], "the left-the-game notice was not shown");
                // The host plays on for at least one more game hour (625 ticks).
                let resumed = sessions[0].world.clock.timing[0];
                run_connected(&mut sessions, &mut frames, assets, "next hour", |sessions, _| {
                    sessions
                        .iter()
                        .all(|s| s.world.clock.timing[0] >= resumed + 625)
                });
                let client = sessions.pop().unwrap();
                let host = sessions.pop().unwrap();
                let client_frame = frames.pop().unwrap();
                let host_frame = frames.pop().unwrap();
                let mut pair = LinkedPair {
                    sessions: [host, client],
                    frames: [host_frame, client_frame],
                    assets,
                    image,
                };
                pair.sample("an hour after the removal");
                assert!(pair.frames.iter().all(|frame| frame.running));
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
