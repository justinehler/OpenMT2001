use super::*;
use openmt2001::network::{
    Row,
    lobby::{Settings, Slot, Snapshot, worker::View},
};

fn ticket() -> Ticket {
    let mut snapshot = Snapshot::host(
        Settings {
            name: b"Shared city".to_vec(),
            level: 300001,
            parameter_bits: 3500f32.to_bits(),
            difficulty: 1,
            map: b"empty.lua".to_vec(),
        },
        b"Host".to_vec(),
        4,
    )
    .unwrap();
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
    // Capture the full portable gameplay records; local current-player selection
    // and camera pose are deliberately outside the shared gameplay contract.
    let current = session.roster.status.current_player;
    // Player+1c0 is the local cash-arrow display cache (player_tick), not cash.
    let cash_display: Vec<_> = session
        .roster
        .manager
        .players
        .iter_mut()
        .map(|p| {
            let old: [u8; 4] = p.bytes[0x1c0..0x1c4].try_into().unwrap();
            p.bytes[0x1c0..0x1c4].fill(0);
            old
        })
        .collect();
    session.roster.status.current_player = session.roster.manager.players[0].reference;
    let (camera, _) = fresh_city_camera();
    let cameras = crate::save_menu::cameras(None, camera).unwrap();
    let bytes = session.save_v2(image, cameras, 1).unwrap();
    session.roster.status.current_player = current;
    for (player, cache) in session.roster.manager.players.iter_mut().zip(cash_display) {
        player.bytes[0x1c0..0x1c4].copy_from_slice(&cache);
    }
    let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
    let extension = openmt2001::save::extension::Extension::parse(&save)
        .unwrap()
        .unwrap();
    (2..=13)
        .chain([16, 17, 18])
        .map(|tag| extension.required(tag).unwrap().to_vec())
        .collect()
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
