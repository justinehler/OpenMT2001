use super::*;

#[test]
#[ignore = "requires original v1.4 assets/scripts for the menu save/load route"]
fn menu_loaded_announcement_keeps_whole_seconds_and_room_deadline() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            use openmt2001::{
                ai_messages::offers::auction::announcement::Call, live_commands::Command,
            };
            struct Scratch(PathBuf);
            impl Drop for Scratch {
                fn drop(&mut self) {
                    let _ = fs::remove_dir_all(&self.0);
                }
            }
            let root = Scratch(
                openmt2001::fixtures::scratch("ANNOUNCEMENT_SAVE").join(format!(
                    "announcement-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )),
            );
            let inputs = camera_producer_live_tests::inputs(root.0.clone());
            let camera = fresh_city_camera().0;
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let (_, mut source) = inputs
                .fresh_preview(
                    camera_producer_live_tests::setup(&inputs),
                    camera,
                    &mut SnapshotCache::default(),
                )
                .unwrap();
            let mut cameras = crate::save_menu::cameras(None, camera).unwrap();
            for row in &mut cameras {
                row[..4].copy_from_slice(&6_u32.to_le_bytes());
            }
            let now = source.world.clock.timing[0];
            let deadline = now + 3000;
            source.live_commands = Default::default();
            let mut start = [0; 52];
            start[4..8].copy_from_slice(&0x1e_u32.to_le_bytes());
            start[16] = 1;
            source
                .live_commands
                .submit_with_delay(
                    Command::Auction { descriptor: start },
                    now,
                    true,
                    2975,
                    |_| panic!("future start"),
                )
                .unwrap();
            let deadlines = |s: &crate::fresh_city::Session<'_>| {
                s.live_commands
                    .snapshot()
                    .unwrap()
                    .into_iter()
                    .map(|(domain, outer, admission, direct, command)| {
                        let kind = match command {
                            Command::Auction { descriptor } => {
                                u32::from_le_bytes(descriptor[4..8].try_into().unwrap())
                            }
                            _ => panic!("unexpected queued command"),
                        };
                        let domain = match domain {
                            openmt2001::scheduler::ClockDomain::Total => 0,
                            openmt2001::scheduler::ClockDomain::Normal => 1,
                        };
                        (domain, outer, admission, direct, kind)
                    })
                    .collect::<Vec<_>>()
            };
            for prior in [0, 3] {
                let mut cameras = cameras;
                if prior == 3 {
                    let strategic = CameraSample {
                        eye: [-220., 1050., -30.],
                        target: [-300., 5., -30.],
                        ..camera
                    };
                    cameras[0] = crate::save_menu::cameras(Some(cameras), strategic).unwrap()[0];
                    cameras[0][..4].copy_from_slice(&4_u32.to_le_bytes());
                }
                for (ticks, expected, seconds) in [
                    (149_u32, 100, vec![1, 0]),
                    (50, 50, vec![0]),
                    (49, 0, vec![]),
                ] {
                    let auction = &mut source.world.auction;
                    auction.controller.reference = 0x7f4758;
                    auction.controller.active_count = 1;
                    auction.controller.record[0x7c] = 0;
                    auction.controller.record[0x7d] = 1;
                    for (at, value) in [(4, 36_u32), (0x5c, 0), (0x84, 0), (0x88, 100)] {
                        auction.controller.record[at..at + 4].copy_from_slice(&value.to_le_bytes());
                    }
                    auction.presentation.visible = true;
                    auction.presentation.countdown = ticks;
                    auction.presentation.countdown_last = (ticks as i32 + 49) / 50;
                    auction.presentation.announcement = [36, 0, 100];
                    let host_state = crate::session_save::AuctionHostState {
                        controls: Default::default(),
                        visible: false,
                        interface_ticks: 123,
                        // An old host label must not supersede the native setter.
                        announcement_seconds: Some((ticks as i32 + 49) / 50),
                    };
                    let bytes = source
                        .save_v2_with_auction_view(&image, cameras, 1, Some(host_state), prior)
                        .unwrap();
                    let path = crate::save_menu::write_new(&root.0, &bytes).unwrap();
                    let (loaded, mut resumed) = inputs
                        .saved_preview(&path, camera, &mut SnapshotCache::default())
                        .unwrap();
                    assert_eq!(loaded.saved_prior_mode, Some(prior));
                    let initial = camera_producer::Initial::from_loaded(
                        &loaded,
                        camera_producer::Controls {
                            camera,
                            controls: fresh_city_camera().1,
                            inertia: openmt2001::camera::BlockCameraInertia::new(
                                camera.eye,
                                camera.target,
                            ),
                            saved: false,
                            building: false,
                            building_angle: 0.,
                            selected: None,
                        },
                    )
                    .unwrap();
                    assert_eq!(initial.strategic(), prior == 3);
                    let continuation = loaded.saved_auction.unwrap();
                    let mut host = crate::live_host::Host::new(camera.eye);
                    host.activate_saved_auction(&mut resumed).unwrap();
                    continuation.apply(&mut resumed).unwrap();
                    host.restore_auction_widgets(continuation.host.as_ref(), &resumed)
                        .unwrap();
                    assert_eq!(resumed.world.auction.presentation.countdown, expected);
                    assert_eq!(host.status(&resumed).auction_seconds, None);
                    let queue = deadlines(&resumed);
                    assert_eq!(queue, deadlines(&source));
                    assert_eq!(queue[0].1, deadline);
                    let mut text = Vec::new();
                    let mut beeps = 0;
                    for _ in 0..150 {
                        resumed
                            .advance_interface_tick(1, false, |call| {
                                match call {
                                    Call::CreateSound => beeps += 1,
                                    Call::WriteSeconds(s) => text.push(s),
                                    _ => {}
                                }
                                Ok(0)
                            })
                            .unwrap();
                    }
                    assert_eq!(text, seconds);
                    assert_eq!(beeps, seconds.len());
                    assert_eq!(resumed.world.auction.presentation.countdown, 0);
                    assert_eq!(resumed.world.auction.controller.record[0x7c], 0);
                    assert_eq!(
                        deadlines(&resumed),
                        queue,
                        "UI zero must not change room-start deadline"
                    );
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
