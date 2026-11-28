/// Original-input regression. Serialization is entirely
/// in memory; this never launches the original executable or writes a save.
#[test]
#[ignore = "requires supplied original saves/scripts/assets; retirement world/clock regression"]
fn retired_camera_host_keeps_world_clock_and_committed_timers() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let map = openmt2001::fixtures::path("MAP");
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let executable = read("OPENMT2001_EXE");
            let exe_path = openmt2001::fixtures::path("EXE");
            let inputs = SceneInputs {
                process_runtime: None,
                compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                // This test never creates or writes this path.
                profile_root: PathBuf::from("unused-camera-retirement-test-profiles"),
                character_names: crate::fresh_city::character_names(
                    &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                    &exe_path
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("US/language/usa"),
                )
                .unwrap(),
                installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                language: exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa"),
                maps: map.parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"),
                executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            };
            let camera = CameraSample {
                eye: [0., 180., 220.],
                target: [0.; 3],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            };
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let board = inputs.styled_board(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
            let assets = crate::fresh_city::CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
                let (loaded, mut session) = inputs
                    .saved_preview(
                        &openmt2001::fixtures::path(key),
                        camera,
                        &mut SnapshotCache::default(),
                    )
                    .unwrap();
                let capture = loaded.imported_cameras.unwrap();
                let index = openmt2001::archive::ArchiveIndex::parse(
                    &inputs.directory,
                    inputs.archive.len() as u64,
                )
                .unwrap();
                let selection =
                    crate::selection::Assets::load(&inputs.executable, &index, &inputs.archive)
                        .unwrap();
                let pavement = selection.pavement_resource().binding().unwrap().handle();
                let mut host = crate::live_host::Host::new(camera.eye);
                let (worker, inbox) = crate::live_host::camera_bridge::channel(
                    session.world.topology.epoch().serial(),
                );
                let install =
                    camera_live::endpoint::PropertyInstall::capture(&selection, &image).unwrap();
                let ui = std::thread::spawn(move || {
                    let owner = camera_live::owner::Owner::new(
                        openmt2001::camera::bank::Bank::loader_zero_storage(),
                        1,
                    );
                    let mut endpoint = camera_live::endpoint::Endpoint::with_property_install(
                        owner, inbox, install,
                    );
                    let mut context = camera_live::endpoint::tests::Ui::default();
                    assert_eq!(
                        endpoint
                            .wait_service(&mut context, std::time::Duration::from_secs(2))
                            .unwrap(),
                        crate::live_host::camera_bridge::Service::Completed
                    );
                    assert_eq!(
                        endpoint
                            .owner()
                            .property_descriptors()
                            .unwrap()
                            .count()
                            .unwrap(),
                        52
                    );
                    assert!(
                        endpoint.camera_only_admission(),
                        "CameraOnly must not capture C/F suppliers"
                    );
                    let owner = endpoint.into_owner();
                    assert!(owner.property_lifecycle_error().is_none());
                });
                host.install_camera_bridge(
                    &mut session,
                    &image,
                    crate::live_host::CameraSupplierPolicy::CameraOnly,
                    worker,
                    capture,
                    pavement,
                )
                .unwrap();
                ui.join().unwrap();
                assert!(host.camera_retired());
                let before = session.save_v2(&image, capture.into_records(), 1).unwrap();
                let timing = session.world.clock.timing;
                let calendar = session.world.clock.calendar;
                let timers = session.hooks.timers.clone();
                let interface_ticks = host.interface_ticks();
                // Even a stale running bit and catch-up-sized delta cannot
                // dispatch a retired session's interface or world callbacks.
                host.running = true;
                host.frame(&mut session, &assets, 1000);
                host.tick(&mut session, &assets);
                assert!(
                    host.options(&mut session.world.pause, &mut session.hooks.timers, false)
                        .is_err()
                );
                host.set_running(true);
                assert!(!host.running && host.failure.is_none());
                assert_eq!(session.world.clock.timing, timing);
                assert_eq!(session.world.clock.calendar, calendar);
                assert_eq!(session.hooks.timers, timers);
                assert_eq!(host.interface_ticks(), interface_ticks);
                assert_eq!(
                    session.save_v2(&image, capture.into_records(), 1).unwrap(),
                    before
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[cfg(feature = "capture")]
#[test]
#[ignore = "requires original Time assets and offscreen GPU; release performance measurement"]
fn original_time_panel_texture_preparation_performance() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameters = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameters.parent().unwrap().parent().unwrap();
    let language = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let assets =
        crate::time_panel::Assets::load(&index, &archive, installer, &language, &executable)
            .unwrap();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let prepare = |fraction| {
        assets
            .prepare(
                &renderer,
                crate::time_panel::Display {
                    year: 1930,
                    hour: 9,
                    speed: 1,
                    name: "JUSTIN",
                    cash: Some(15000.),
                    fraction,
                    character: Some(4),
                    cash_icon_phase: 0,
                },
            )
            .unwrap()
    };
    renderer.bypass_texture_cache_for_capture(true);
    let started = Instant::now();
    for fraction in 0..100 {
        std::hint::black_box(prepare(fraction));
    }
    let uncached = started.elapsed();
    renderer.bypass_texture_cache_for_capture(false);
    prepare(0);
    let started = Instant::now();
    for fraction in 0..100 {
        std::hint::black_box(prepare(fraction));
    }
    let cached = started.elapsed();
    let mut retained = prepare(0);
    let started = Instant::now();
    for fraction in 0..100 {
        assets
            .refresh_for_test(
                &renderer,
                &mut retained,
                crate::time_panel::Display {
                    year: 1930,
                    hour: 9,
                    speed: 1,
                    name: "JUSTIN",
                    cash: Some(15000.),
                    fraction,
                    character: Some(4),
                    cash_icon_phase: 0,
                },
            )
            .unwrap();
    }
    eprintln!(
        "Original Time panel prepare (100 changing clock fractions): uncached {uncached:?}, resident {cached:?}, retained {:?}",
        started.elapsed()
    );
}

fn test_board_style(executable: &[u8]) -> openmt2001::board_style::Style {
    let parameters = openmt2001::fixtures::path("PARAMETERS");
    crate::block_sidewalk_labels::load_style(
        &openmt2001::pe::PeImage::parse(executable).unwrap(),
        parameters.parent().unwrap().parent().unwrap(),
    )
    .unwrap()
}

mod pause_save_test {
    include!("../intro_pause_save_test.rs");
}

#[cfg(feature = "capture")]
mod rights_worker_test {
    include!("../intro_rights_test.rs");
}
#[cfg(feature = "capture")]
mod auction_save_test {
    include!("../intro_auction_save_test.rs");
}
#[cfg(feature = "capture")]
mod strategic_capture_test {
    include!("../intro_strategic_capture_test.rs");
}
use super::*;

#[cfg(target_os = "linux")]
mod toolbar_right_dispatch_test {
    include!("../toolbar_right_dispatch_test.rs");
}

#[cfg(feature = "capture")]
mod menu_identity_test {
    include!("../intro_menu_identity_test.rs");
}

#[test]
fn normal_city_entry_keeps_native_pose_with_53_degree_projection() {
    let (sample, controls) = fresh_city_camera();
    assert_eq!(sample.eye, [26., 586., -695.]);
    assert_eq!(sample.target, [26., 463., -595.]);
    assert_eq!(sample.up, [0., 1., 0.]);
    assert_eq!(sample.vertical_fov, 53.);
    assert_eq!(controls.center, [51., 463., -595.]);
    assert_eq!(controls.height, 586.);
    assert_eq!(controls.radius, 100.);
    assert_eq!(controls.angle, -std::f32::consts::FRAC_PI_2);
}

#[test]
fn normal_saved_city_selects_primary_pose_and_owns_fov() {
    let mut records = [[0u8; 196]; 2];
    let poses = [
        ([82., 120., -454.5], [82., 0.25, -374.5], [0., 1., 0.]),
        ([410., 80., -60.], [10., 2., 20.], [0., 0., 1.]),
    ];
    for (record, (eye, target, up)) in records.iter_mut().zip(poses) {
        for (offset, values) in [(0x4c, eye), (0x58, target), (0x64, up)] {
            for (axis, value) in values.into_iter().enumerate() {
                let at = offset + axis * 4;
                record[at..at + 4].copy_from_slice(&f32::to_le_bytes(value));
            }
        }
    }
    let (sample, controls) = normal_saved_city_viewpoint(&records).unwrap();
    assert_eq!((sample.eye, sample.target, sample.up), poses[0]);
    assert_eq!(sample.vertical_fov, 53.);
    assert_eq!(controls.radius, 80.);
    assert_eq!(controls.height, 120.);
    // Other owners retain the caller-selected FOV and record.
    let generic = saved_viewpoint(&records[1], 61.).unwrap().0;
    assert_eq!((generic.eye, generic.target, generic.up), poses[1]);
    assert_eq!(generic.vertical_fov, 61.);
    // An invalid primary must not silently select the usable backup.
    records[0][0x4c..0x50].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(normal_saved_city_viewpoint(&records).is_err());
}

#[test]
#[ignore = "requires supplied v1.4 PE and both original saves; read-only camera contract"]
fn supplied_normal_city_camera_matches_pe_fov_and_projection() {
    let executable = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let word = |address| u32::from_le_bytes(image.at(address, 4).unwrap().try_into().unwrap());
    assert_eq!(word(0x545acc), 53f32.to_bits(), "normal reset FOV data");
    assert_eq!(
        word(0x52af24),
        0x4f1550,
        "renderer slot14 projection setter"
    );
    assert_eq!(word(0x52b0bc), 0x3c0efa1e, "original half-degree factor");
    assert_eq!(fresh_city_camera().0.vertical_fov.to_bits(), word(0x545acc));
    for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
        let bytes = fs::read(openmt2001::fixtures::path(key)).unwrap();
        let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
        let records = save.cameras().unwrap().map(|row| *row);
        let sample = normal_saved_city_viewpoint(&records).unwrap().0;
        let primary =
            |offset| f32::from_le_bytes(records[0][offset..offset + 4].try_into().unwrap());
        assert_eq!(
            sample.eye,
            [primary(0x4c), primary(0x50), primary(0x54)],
            "{key}"
        );
        assert_eq!(
            sample.target,
            [primary(0x58), primary(0x5c), primary(0x60)],
            "{key}"
        );
        assert_eq!(
            sample.up,
            [primary(0x64), primary(0x68), primary(0x6c)],
            "{key}"
        );
        assert_eq!(sample.vertical_fov.to_bits(), word(0x545acc), "{key}");
        let [near, far] = openmt2001::camera::city_clip(sample.eye[1]).unwrap();
        let matrix = openmt2001::camera::projection(
            sample.vertical_fov,
            near,
            far,
            1280,
            960,
            [0., 1., 0., 1.],
        )
        .unwrap();
        // Original004f1480 full-clip focal scales at53 degrees and4:3.
        // Specified viewport only; not a claim about the recording's aspect.
        assert!(
            (matrix[5] - 2.0056954286f32).abs() < 0.000001,
            "{key} vertical focal scale"
        );
        assert!(
            (matrix[0] - 1.5042715714f32).abs() < 0.000001,
            "{key} aspect scaling"
        );
        assert_eq!(
            (matrix[8], matrix[9], matrix[11], matrix[15]),
            (0., 0., 1., 0.)
        );
    }
}

#[test]
#[ignore = "requires original board and supplied saved business memberships"]
fn original_category_hyperlinks_keep_native_identity() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let exe = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    // Real board hyperlinks above both holes. Building ordinals are
    // controlled inputs here; they do not claim a natural high-ID business.
    let mut high_ids = 0;
    for block in board.blocks.iter().filter(|block| block.id >= 46) {
        let hit = inspection_hit(&board.blocks, block.id, Some(7)).unwrap();
        assert_eq!(board.blocks[hit.category].id, block.id);
        assert_eq!(hit.category + 2, block.id);
        assert_eq!(hit.building, Some(7));
        high_ids += 1;
    }
    assert_eq!(high_ids, 6);
    assert!(inspection_hit(&board.blocks, 40, None).is_none());
    assert!(inspection_hit(&board.blocks, 45, None).is_none());
    let save = read("OPENMT2001_SAVE_ONE");
    let saved = openmt2001::save::SaveFile::parse(&save).unwrap();
    let buildings = saved.buildings().unwrap();
    assert!(!buildings.is_empty());
    // These original retained memberships establish the scene ID domain.
    for (ordinal, building) in buildings.iter().enumerate() {
        let hit = inspection_hit(&board.blocks, building.category, Some(ordinal)).unwrap();
        assert_eq!(board.blocks[hit.category].id, building.category);
        assert_eq!(hit.building, Some(ordinal));
    }
}

#[test]
fn construction_snapshot_cadence_is_bounded_to_active_growth() {
    assert_eq!(
        city_snapshot_interval(true),
        std::time::Duration::from_millis(250)
    );
    assert_eq!(
        city_snapshot_interval(false),
        std::time::Duration::from_secs(1)
    );
}

#[test]
fn menu_camera_retains_flyby_and_uses_native_tenth_step() {
    let flyby = CameraSample {
        eye: [-40., 20., 20.],
        target: [-45., 10., 25.],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let mut camera = MenuCamera::new(flyby);
    assert_eq!(camera.sample(0, 0.), flyby);
    let first = camera.sample(1, 0.);
    assert_eq!(first.eye[1], 21.);
    assert_eq!(first.eye[2], 25.4);
    assert_eq!(first.target, [-45.8, 9.5, 26.9]);
    let before = camera.eye_start;
    let presented = camera.sample(1, 1.);
    assert!((presented.eye[1] - 21.9).abs() < 0.00001);
    assert_eq!(
        camera.eye_start, before,
        "presentation must not advance the tick"
    );
    let second = camera.sample(2, 0.);
    assert!((second.eye[1] - 21.9).abs() < 0.00001);
    assert_eq!(camera.tick, 2);
}

#[test]
#[ignore = "requires original top-right panel mesh"]
fn time_panel_hitboxes_at_default_window_size() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let executable = read("OPENMT2001_EXE");
    let language = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let panel =
        crate::time_panel::Assets::load(&index, &archive, installer, &language, &executable)
            .unwrap();
    let hit = |screen| {
        let point = internal_pointer(screen, [960, 720]).unwrap();
        panel.button(point.map(|v| v as f32 * 0.5))
    };
    assert_eq!(hit([880., 26.]), Some(crate::time_panel::Button::Options));
    let options = internal_pointer([910., 26.], [960, 720]).unwrap();
    assert!(panel.contains(options.map(|v| v as f32 * 0.5)));
    assert_eq!(hit([930., 692.]), Some(crate::time_panel::Button::Speed));
    assert_eq!(hit([875., 692.]), Some(crate::time_panel::Button::Manager));
    let manager = internal_pointer([875., 692.], [960, 720]).unwrap();
    assert!(panel.contains(manager.map(|v| v as f32 * 0.5)));
    let compass = internal_pointer([818., 692.], [960, 720]).unwrap();
    assert!(
        !panel.contains(compass.map(|v| v as f32 * 0.5)),
        "compass retains its own navigation hitbox"
    );
    let outside = internal_pointer([730., 26.], [960, 720]).unwrap();
    assert!(!panel.contains(outside.map(|v| v as f32 * 0.5)));
    let letterboxed = internal_pointer([1090., 692.], [1280, 720]).unwrap();
    assert_eq!(
        panel.button(letterboxed.map(|v| v as f32 * 0.5)),
        Some(crate::time_panel::Button::Speed)
    );
    let manager_letterboxed = internal_pointer([1035., 692.], [1280, 720]).unwrap();
    assert_eq!(
        panel.button(manager_letterboxed.map(|v| v as f32 * 0.5)),
        Some(crate::time_panel::Button::Manager)
    );
    assert!(internal_pointer([50., 692.], [1280, 720]).is_none());
}

#[test]
#[ignore = "requires original menu mesh and language assets"]
fn game_options_hitboxes_at_default_and_letterboxed_window_size() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let executable = openmt2001::fixtures::path("EXE");
    let language = executable
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let assets = crate::game_options::Assets::load(&index, &archive, installer, &language).unwrap();
    let labels = assets
        .labels()
        .iter()
        .map(|row| row.iter().copied().map(|b| b + 32).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        [
            b"CONTINUE".to_vec(),
            b"SAVE GAME".to_vec(),
            b"SETTINGS".to_vec(),
            b"QUIT".to_vec()
        ]
    );
    let bounds = assets.bounds();
    assert_eq!(bounds[0], [79., 161.77893, 568., 195.77893]);
    let executable_bytes = fs::read(&executable).unwrap();
    let image = openmt2001::pe::PeImage::parse(&executable_bytes).unwrap();
    for (i, expected) in [
        b"MAIN_BUTT02",
        b"MAIN_BUTT03",
        b"MAIN_BUTT04",
        b"MAIN_BUTT05",
    ]
    .into_iter()
    .enumerate()
    {
        let native = image.at(0x543a68 + i as u32 * 20, 20).unwrap();
        let name = u32::from_le_bytes(native[..4].try_into().unwrap());
        assert_eq!(image.at(name, expected.len()).unwrap(), expected);
    }
    let actions = [
        crate::game_options::Button::Continue,
        crate::game_options::Button::Save,
        crate::game_options::Button::Settings,
        crate::game_options::Button::Quit,
    ];
    let mut state = crate::game_options::View::default();
    state.open(true);
    assert!(state.open && state.resume_running);
    for (i, [l, t, r, b]) in bounds.into_iter().enumerate() {
        let point = [(l + r) / 2., (t + b) / 2.];
        for (size, screen) in [
            ([960, 720], [point[0] * 1.5, point[1] * 1.5]),
            ([1280, 720], [160. + point[0] * 1.5, point[1] * 1.5]),
        ] {
            let internal = internal_pointer(screen.map(f64::from), size).unwrap();
            assert_eq!(
                assets.button(internal.map(|v| v as f32 * 0.5)),
                Some(actions[i])
            );
        }
        assert_eq!(assets.button([l - 1., (t + b) / 2.]), None);
        assert_eq!(state.mouse(&assets, Some(point), true), None);
        assert_eq!(
            state.mouse(&assets, Some([l - 1., (t + b) / 2.]), false),
            None,
            "release outside the pressed row must cancel"
        );
        assert_eq!(state.mouse(&assets, Some(point), true), None);
        assert_eq!(state.mouse(&assets, Some(point), false), Some(actions[i]));
    }
    state.quit_prompt.dialog_button(1);
    assert_eq!(state.quit_prompt.take_dialog_replies()[0].button, 1);
    state.quit_prompt.finish_dismiss(vec![]);
    state.set_save_disabled(true);
    let [l, t, r, b] = bounds[1];
    let save = [(l + r) * 0.5, (t + b) * 0.5];
    assert_eq!(state.mouse(&assets, Some(save), true), None);
    assert_eq!(
        state.mouse(&assets, Some(save), false),
        None,
        "native tutorial Save row is disabled before activation"
    );
    assert!(state.close(), "Continue restores the prior running state");
    state.open(false);
    assert!(!state.close(), "a previously paused game remains paused");
    assert!(internal_pointer([50., 200.], [1280, 720]).is_none());
}

#[test]
#[ignore = "requires original Settings meshes and language assets"]
fn settings_menu_hitboxes_at_default_and_letterboxed_window_size() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let executable = openmt2001::fixtures::path("EXE");
    let language = executable
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let assets =
        crate::settings_menu::Assets::load(&index, &archive, installer, &language).unwrap();
    for (id, expected) in [
        (1679, b"SETTINGS".as_slice()),
        (1607, b"Sound"),
        (1608, b"Graphics"),
        (1609, b"Gameplay"),
        (1610, b"Board"),
        (1770, b"Credits"),
        (1139, b"Reset to default"),
        (1127, b"OK"),
    ] {
        let decoded = assets
            .label_for_test(id)
            .iter()
            .map(|&b| b + 32)
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected, "original Settings text ID {id}");
    }
    let page = crate::settings_menu::Page::Main;
    assert_eq!(
        assets
            .bounds(page, crate::settings_menu::Button::Sound)
            .unwrap(),
        [79., 111.52376, 568., 145.52376]
    );
    let rows = [
        crate::settings_menu::Button::Sound,
        crate::settings_menu::Button::Graphics,
        crate::settings_menu::Button::Gameplay,
        crate::settings_menu::Button::Back,
    ];
    let mut state = crate::settings_menu::View::default();
    let mut levels = crate::settings_menu::Levels::default();
    state.open();
    for button in rows {
        let [l, t, r, b] = assets.bounds(page, button).unwrap();
        let point = [(l + r) / 2., (t + b) / 2.];
        for (size, screen) in [
            ([960, 720], [point[0] * 1.5, point[1] * 1.5]),
            ([1280, 720], [160. + point[0] * 1.5, point[1] * 1.5]),
        ] {
            let internal = internal_pointer(screen.map(f64::from), size).unwrap();
            assert_eq!(
                assets.button(page, internal.map(|v| v as f32 * 0.5)),
                Some(button)
            );
        }
        state.mouse(&assets, Some(point), true, &mut levels);
        state.mouse(&assets, Some([l - 1., (t + b) / 2.]), false, &mut levels);
        assert_eq!(state.page, Some(page), "different release target cancels");
        state.mouse(&assets, Some(point), true, &mut levels);
        state.mouse(&assets, Some(point), false, &mut levels);
        if button == crate::settings_menu::Button::Back {
            assert_eq!(state.page, None);
        } else {
            assert_ne!(state.page, Some(page));
            let subpage = state.page.unwrap();
            let back = assets
                .bounds(subpage, crate::settings_menu::Button::Back)
                .unwrap();
            let center = [(back[0] + back[2]) / 2., (back[1] + back[3]) / 2.];
            state.mouse(&assets, Some(center), true, &mut levels);
            state.mouse(&assets, Some(center), false, &mut levels);
            assert_eq!(state.page, Some(page));
        }
    }
    assert!(internal_pointer([50., 200.], [1280, 720]).is_none());
}

#[test]
#[ignore = "requires original Sound mesh and v1.4 executable"]
fn sound_settings_original_slider_input_and_dispatch() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    // 49bd0f/49bd8c create slider groups 4/5, each with max 100.
    assert_eq!(image.at(0x49bd0f, 2).unwrap(), &[0x6a, 4]);
    assert_eq!(image.at(0x49bd88, 2).unwrap(), &[0x6a, 5]);
    assert_eq!(image.at(0x49bd18, 5).unwrap(), &[0xbe, 0, 0, 0xc8, 0x42]);
    assert_eq!(
        image.at(0x4c1902, 10).unwrap(),
        &[0xc7, 0x05, 0xd8, 0x06, 0x92, 0x00, 0x3f, 0x00, 0x00, 0x00]
    );
    assert_eq!(
        image.at(0x4c190c, 10).unwrap(),
        &[0xc7, 0x05, 0xdc, 0x06, 0x92, 0x00, 0x53, 0x00, 0x00, 0x00]
    );
    let call = |at: u32| {
        let bytes = image.at(at, 5).unwrap();
        assert_eq!(bytes[0], 0xe8);
        (at + 5).wrapping_add_signed(i32::from_le_bytes(bytes[1..5].try_into().unwrap()))
    };
    assert_eq!(call(0x49c211), 0x4af3f0);
    assert_eq!(call(0x49c22d), 0x4af410);
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe_path = openmt2001::fixtures::path("EXE");
    let language = exe_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let assets =
        crate::settings_menu::Assets::load(&index, &archive, installer, &language).unwrap();
    use crate::settings_menu::{Button, Levels, Page, View};
    let mut state = View::default();
    let mut levels = Levels::default();
    state.open();
    state.mouse(&assets, Some([320., 128.]), true, &mut levels);
    state.mouse(&assets, Some([320., 128.]), false, &mut levels);
    assert_eq!(state.page, Some(Page::Sound));
    for button in [
        Button::MusicTrack,
        Button::EffectsTrack,
        Button::MusicLeft,
        Button::MusicRight,
        Button::EffectsLeft,
        Button::EffectsRight,
    ] {
        let [l, t, r, b] = assets.bounds(Page::Sound, button).unwrap();
        let center = [(l + r) / 2., (t + b) / 2.];
        for (size, screen) in [
            ([960, 720], [center[0] * 1.5, center[1] * 1.5]),
            ([1280, 720], [160. + center[0] * 1.5, center[1] * 1.5]),
        ] {
            let internal = internal_pointer(screen.map(f64::from), size).unwrap();
            assert_eq!(
                assets.button(Page::Sound, internal.map(|v| v as f32 * 0.5)),
                Some(button)
            );
        }
    }
    let track = assets.bounds(Page::Sound, Button::MusicTrack).unwrap();
    let y = (track[1] + track[3]) / 2.;
    let native_track = crate::ui::slider_track(track.map(f32::trunc), false);
    assert!(state.mouse(&assets, Some([native_track[0] + 1., y]), true, &mut levels));
    assert_eq!(levels.music, 0);
    assert!(state.drag(
        &assets,
        Some([
            // 432260 truncates the pointer to an integer pixel before
            // selecting one of maximum+1 bins. Pick the first pixel in
            // value 37's bin, rather than assuming 37% of any track width.
            (native_track[0] + (native_track[2] - native_track[0]) * 37. / 101.).ceil(),
            y
        ]),
        &mut levels
    ));
    assert_eq!(levels.music, 37);
    state.mouse(&assets, Some([track[2], y]), false, &mut levels);
    let arrow = assets.bounds(Page::Sound, Button::MusicRight).unwrap();
    let center = [(arrow[0] + arrow[2]) / 2., (arrow[1] + arrow[3]) / 2.];
    state.mouse(&assets, Some(center), true, &mut levels);
    assert!(!state.mouse(
        &assets,
        Some([arrow[0] - 1., center[1]]),
        false,
        &mut levels
    ));
    assert_eq!(levels.music, 37);
    state.mouse(&assets, Some(center), true, &mut levels);
    assert!(state.mouse(&assets, Some(center), false, &mut levels));
    assert_eq!(levels.music, 38);
    // The original Sound panel has no Reset child; only the native
    // slider and arrow controls may change these levels.
    assert_eq!(levels.effects, Levels::default().effects);
}

// Whole original-data city assemblies plus visible retained-resource probes.
#[cfg(feature = "capture")]
#[test]
#[ignore = "requires supplied originals and GPU"]
fn original_city_refresh_matches_independent_prepare() {
    // Parent must supply a unique on-disk directory; never use shared /tmp.
    let parent = openmt2001::fixtures::scratch("EQ_SCRATCH");
    assert!(parent.is_absolute() && parent.is_dir());
    let owned = parent.join(format!("city-refresh-{}", std::process::id()));
    fs::create_dir(&owned).expect("scratch must be uniquely owned");
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let _cleanup = Cleanup(owned.clone());
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build = read("OPENMT2001_BUILD_FILE");
    let suncols = read("OPENMT2001_SUNCOLS");
    let save = read("OPENMT2001_SAVE_ONE");
    let map = read("OPENMT2001_INTRO_MAP");
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    eprintln!("equivalence adapter: {}", renderer.adapter_name);
    let compare = |fresh: &openmt2001::render::PreparedScene,
                   retained: &openmt2001::render::PreparedScene,
                   label: &str| {
        let a = owned.join("fresh.png");
        let b = owned.join("retained.png");
        crate::capture_test::capture_standalone(&renderer, fresh, a.to_str().unwrap());
        crate::capture_test::capture_standalone(&renderer, retained, b.to_str().unwrap());
        assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap(), "{label}");
    };
    for saved in [true, false] {
        let mut retained = None;
        let mut previous_materials = None;
        let mut material_transition = false;
        for hour in [9, 10, 11, 21, 9] {
            // Complete independent assembly, original order, no worker/world step.
            let scene = render_scene::assemble(Request {
                build_file: &build,
                source: if saved {
                    Source::Save(&save)
                } else {
                    Source::Map(std::str::from_utf8(&map).unwrap())
                },
                original: Some(OriginalAssets {
                    executable: &executable,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: Some(&suncols),
                phase: Some(hour),
                camera: None,
                width: 1280,
                height: 960,
                animated: true,
            })
            .unwrap();
            let materials = scene
                .draws
                .iter()
                .map(|d| {
                    (
                        d.mode,
                        d.secondary.as_ref().map(|t| {
                            use std::hash::Hasher;
                            let mut hash = std::collections::hash_map::DefaultHasher::new();
                            hash.write(t.rgba.as_ref());
                            (t.width, t.height, t.mipmaps, hash.finish())
                        }),
                    )
                })
                .collect::<Vec<_>>();
            if let Some(old) = &previous_materials {
                material_transition |= old != &materials;
            }
            previous_materials = Some(materials);
            let draws = scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>();
            let fresh = renderer.prepare(&draws, scene.camera, scene.fog).unwrap();
            if let Some(old) = &mut retained {
                renderer
                    .refresh_prepared(old, &draws, scene.camera, scene.fog)
                    .unwrap();
            } else {
                retained = Some(renderer.prepare(&draws, scene.camera, scene.fog).unwrap());
            }
            compare(
                &fresh,
                retained.as_ref().unwrap(),
                &format!("complete city hour {hour}"),
            );
        }
        assert!(
            material_transition,
            "fixture must exercise day/night material or secondary transition"
        );
    }
    // Visible diagnostic contract probes supplement potentially occluded city data.
    // No invented city assembly/lighting rules: these are explicitly synthetic.
    use openmt2001::render::{SecondaryTexture, SurfaceMode, TexturedDraw};
    let identity = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let triangle = [
        [-0.8, -0.8, 0.2, 1., 1., 1., 0., 0.],
        [0.8, -0.8, 0.2, 1., 1., 1., 1., 0.],
        [0., 0.8, 0.2, 1., 1., 1., 0.5, 1.],
    ];
    let mut retained = None;
    let mut previous_pixels = None;
    for case in 0..10 {
        let count = [3, 6, 3, 0, 3, 3, 3, 3, 3, 3][case];
        let mut vertices = triangle.repeat(count / 3);
        if count == 6 {
            for v in &mut vertices[3..] {
                v[0] *= 0.5;
                v[1] += 0.1;
            }
        }
        let uv = vec![[0.3, 0.7]; count];
        let specular = vec![[0.1, 0.0, 0.0]; count];
        let alpha = vec![128; count];
        let primary = [255, if case < 5 { 80 } else { 180 }, 20, 255];
        let secondary = [20, 40, if case == 7 { 100 } else { 240 }, 255];
        let blend = case >= 6;
        let draw = TexturedDraw {
            view_projection: (case >= 5).then_some(identity),
            vertices: &vertices,
            specular: Some(&specular),
            diffuse_alpha: Some(&alpha),
            mode: if blend {
                SurfaceMode::FacadeBlend
            } else {
                SurfaceMode::StaticLit
            },
            width: 1,
            height: 1,
            rgba: &primary,
            mipmaps: case >= 7,
            secondary: blend.then_some(SecondaryTexture {
                uv: &uv,
                diffuse_alpha: 120,
                width: 1,
                height: 1,
                rgba: &secondary,
                mipmaps: case >= 7,
            }),
        };
        let mut camera = identity;
        if case >= 5 {
            camera[12] = 0.2;
        } // override must win
        let draws = if case == 8 { Vec::new() } else { vec![draw] };
        let fog = (case >= 5).then_some(openmt2001::render::EyeFog {
            color: 0x406080,
            start: 0.0,
            end: 2.0,
        });
        let fresh = renderer.prepare(&draws, camera, fog).unwrap();
        if let Some(old) = &mut retained {
            renderer.refresh_prepared(old, &draws, camera, fog).unwrap();
        } else {
            retained = Some(renderer.prepare(&draws, camera, fog).unwrap());
        }
        compare(&fresh, retained.as_ref().unwrap(), &format!("probe {case}"));
        let pixels = fs::read(owned.join("fresh.png")).unwrap();
        if matches!(case, 3 | 4 | 5 | 7) {
            assert_ne!(
                previous_pixels.as_ref().unwrap(),
                &pixels,
                "geometry/texture controls must visibly change"
            );
        }
        previous_pixels = Some(pixels);
    }
    // Replace only one of two surfaces, leaving a shared camera allocation
    // beside a standalone replacement buffer. Then update both geometries,
    // global camera/fog and a per-draw override across further refreshes.
    let mut mixed = None;
    let mut previous = None;
    for case in 0..5 {
        let left = triangle
            .iter()
            .map(|v| {
                let mut v = *v;
                v[0] = v[0] * 0.4 - 0.5;
                v
            })
            .collect::<Vec<_>>();
        let right = triangle
            .iter()
            .map(|v| {
                let mut v = *v;
                v[0] = v[0] * 0.4 + 0.5;
                v
            })
            .collect::<Vec<_>>();
        let left_pixels = [255, if case == 0 { 20 } else { 180 }, 20, 255];
        let right_pixels = [20, 120, if case < 3 { 255 } else { 80 }, 255];
        let mut camera = identity;
        camera[13] = case as f32 * 0.07;
        let fog = Some(openmt2001::render::EyeFog {
            color: 0x406080,
            start: 0.0,
            end: 2.0,
        });
        let draws = [
            TexturedDraw {
                view_projection: None,
                vertices: if case == 4 { &[] } else { &left },
                specular: None,
                diffuse_alpha: None,
                mode: SurfaceMode::StaticLit,
                width: 1,
                height: 1,
                rgba: &left_pixels,
                mipmaps: false,
                secondary: None,
            },
            TexturedDraw {
                view_projection: Some(identity),
                vertices: &right,
                specular: None,
                diffuse_alpha: None,
                mode: SurfaceMode::StaticLit,
                width: 1,
                height: 1,
                rgba: &right_pixels,
                mipmaps: false,
                secondary: None,
            },
        ];
        let fresh = renderer.prepare(&draws, camera, fog).unwrap();
        if let Some(old) = &mut mixed {
            renderer.refresh_prepared(old, &draws, camera, fog).unwrap();
        } else {
            mixed = Some(renderer.prepare(&draws, camera, fog).unwrap());
        }
        compare(
            &fresh,
            mixed.as_ref().unwrap(),
            &format!("mixed buffers {case}"),
        );
        let pixels = fs::read(owned.join("fresh.png")).unwrap();
        if let Some(previous) = &previous {
            assert_ne!(previous, &pixels, "mixed controls must visibly change");
        }
        previous = Some(pixels);
    }
}

#[cfg(feature = "capture")]
#[test]
#[ignore = "requires original intro assets and GPU"]
fn menu_vertex_upload_batch_matches_individual_updates() {
    for phase in [0, 12] {
        verify_menu_vertex_upload_phase(phase);
    }
}

#[cfg(feature = "capture")]
fn verify_menu_vertex_upload_phase(phase: u32) {
    eprintln!("Vertex batch original-city phase{phase}");
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build_file = read("OPENMT2001_BUILD_FILE");
    let suncols = read("OPENMT2001_SUNCOLS");
    let map = read("OPENMT2001_INTRO_MAP");
    let camera = CameraSample {
        eye: [-53., 30., 74.],
        target: [-53., 5., 44.],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let mut scene = render_scene::assemble(Request {
        build_file: &build_file,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &executable,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(phase),
        camera: Some(camera),
        width: 1280,
        height: 960,
        animated: true,
    })
    .unwrap();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let borrowed = scene
        .draws
        .iter()
        .map(|draw| draw.borrowed())
        .collect::<Vec<_>>();
    let mut individual = renderer
        .prepare(&borrowed, scene.camera, scene.fog)
        .unwrap();
    let mut batch = renderer
        .prepare(&borrowed, scene.camera, scene.fog)
        .unwrap();
    renderer
        .update_dynamic_vertices_batch(&mut batch, &[])
        .unwrap();
    let mut menu = MenuCamera::new(camera);
    let mut old_time = std::time::Duration::ZERO;
    let mut batch_time = std::time::Duration::ZERO;
    let mut reframe_time = std::time::Duration::ZERO;
    for tick in 0..100 {
        let started = Instant::now();
        let changed = scene.reframe(menu.sample(tick, 0.25)).unwrap();
        reframe_time += started.elapsed();
        let started = Instant::now();
        for (index, draw) in scene.draws.iter().enumerate() {
            if changed.contains(&index) {
                renderer
                    .update_dynamic_surface(
                        &mut individual,
                        index,
                        &draw.borrowed(),
                        scene.camera,
                        scene.fog,
                    )
                    .unwrap();
            } else {
                renderer
                    .update_camera(
                        &individual,
                        index,
                        draw.view_projection.unwrap_or(scene.camera),
                        scene.fog,
                    )
                    .unwrap();
            }
        }
        old_time += started.elapsed();
        let started = Instant::now();
        let updates = changed
            .iter()
            .map(|&index| (index, scene.draws[index].borrowed()))
            .collect::<Vec<_>>();
        renderer
            .update_dynamic_vertices_batch(&mut batch, &updates)
            .unwrap();
        let cameras = scene
            .draws
            .iter()
            .map(|draw| draw.view_projection.unwrap_or(scene.camera))
            .collect::<Vec<_>>();
        renderer
            .update_cameras(&batch, &cameras, scene.fog)
            .unwrap();
        batch_time += started.elapsed();
        if [0, 50, 99].contains(&tick) {
            let reference = format!("frontend/vertex-individual-{tick}.png");
            let actual = format!("frontend/vertex-batch-{tick}.png");
            crate::capture_test::capture_standalone(&renderer, &individual, &reference);
            crate::capture_test::capture_standalone(&renderer, &batch, &actual);
            assert_eq!(
                fs::read(crate::capture_test::output_path(&reference)).unwrap(),
                fs::read(crate::capture_test::output_path(&actual)).unwrap()
            );
            let fresh = format!("frontend/vertex-fresh-{tick}.png");
            crate::capture_test::capture_scene_world_overlays(&renderer, &scene, &[], &[], &fresh);
            assert_eq!(
                fs::read(crate::capture_test::output_path(&fresh)).unwrap(),
                fs::read(crate::capture_test::output_path(&actual)).unwrap()
            );
        } else {
            // Flush staging uploads equally without adding offscreen rendering
            // to the CPU preparation measurements.
            renderer.queue.submit([]);
            renderer
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
        }
    }
    // Individual APIs must still isolate each draw's camera and fog slots.
    let alternate = scene.fog.map(|mut fog| {
        fog.color ^= 0x00ff0000;
        fog
    });
    renderer
        .update_camera(&individual, 0, scene.camera, alternate)
        .unwrap();
    renderer
        .update_camera(&batch, 0, scene.camera, alternate)
        .unwrap();
    crate::capture_test::capture_standalone(
        &renderer,
        &individual,
        "frontend/vertex-individual-fog.png",
    );
    crate::capture_test::capture_standalone(&renderer, &batch, "frontend/vertex-batch-fog.png");
    assert_eq!(
        fs::read(crate::capture_test::output_path(
            "frontend/vertex-individual-fog.png"
        ))
        .unwrap(),
        fs::read(crate::capture_test::output_path(
            "frontend/vertex-batch-fog.png"
        ))
        .unwrap()
    );
    // A material replacement introduces a standalone uniform buffer. The
    // batch API must update that live buffer as well as the retained arena.
    let mut pixels = scene.draws[0].rgba.to_vec();
    pixels[0] ^= 255;
    scene.draws[0].rgba = pixels.into();
    let extra = scene.draws[0].vertices.clone();
    scene.draws[0].vertices.extend(extra);
    if let Some(values) = &mut scene.draws[0].specular {
        values.extend(values.clone());
    }
    if let Some(values) = &mut scene.draws[0].diffuse_alpha {
        values.extend(values.clone());
    }
    if let Some(secondary) = &mut scene.draws[0].secondary {
        secondary.uv.extend(secondary.uv.clone());
    }
    renderer
        .update_dynamic_surface(
            &mut individual,
            0,
            &scene.draws[0].borrowed(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    renderer
        .update_dynamic_vertices_batch(&mut batch, &[(0, scene.draws[0].borrowed())])
        .unwrap();
    let borrowed = scene
        .draws
        .iter()
        .map(|draw| draw.borrowed())
        .collect::<Vec<_>>();
    renderer
        .refresh_prepared(&mut individual, &borrowed, scene.camera, scene.fog)
        .unwrap();
    renderer
        .refresh_prepared(&mut batch, &borrowed, scene.camera, scene.fog)
        .unwrap();
    let changed = scene.reframe(menu.sample(125, 0.5)).unwrap();
    for (index, draw) in scene.draws.iter().enumerate() {
        if changed.contains(&index) {
            renderer
                .update_dynamic_surface(
                    &mut individual,
                    index,
                    &draw.borrowed(),
                    scene.camera,
                    scene.fog,
                )
                .unwrap();
        } else {
            renderer
                .update_camera(
                    &individual,
                    index,
                    draw.view_projection.unwrap_or(scene.camera),
                    scene.fog,
                )
                .unwrap();
        }
    }
    let updates = changed
        .iter()
        .map(|&index| (index, scene.draws[index].borrowed()))
        .collect::<Vec<_>>();
    renderer
        .update_dynamic_vertices_batch(&mut batch, &updates)
        .unwrap();
    let cameras = scene
        .draws
        .iter()
        .map(|draw| draw.view_projection.unwrap_or(scene.camera))
        .collect::<Vec<_>>();
    renderer
        .update_cameras(&batch, &cameras, scene.fog)
        .unwrap();
    crate::capture_test::capture_standalone(
        &renderer,
        &individual,
        "frontend/vertex-individual-replaced.png",
    );
    crate::capture_test::capture_standalone(
        &renderer,
        &batch,
        "frontend/vertex-batch-replaced.png",
    );
    assert_eq!(
        fs::read(crate::capture_test::output_path(
            "frontend/vertex-individual-replaced.png"
        ))
        .unwrap(),
        fs::read(crate::capture_test::output_path(
            "frontend/vertex-batch-replaced.png"
        ))
        .unwrap()
    );
    eprintln!(
        "Original menu {} draws, 100 moving frames: reframe {reframe_time:?}, individual uploads {old_time:?}, batch uploads {batch_time:?}",
        scene.draws.len()
    );
}

#[cfg(feature = "capture")]
#[test]
#[ignore = "requires original intro assets and GPU"]
fn menu_batch_camera_upload_matches_individual_updates() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build_file = read("OPENMT2001_BUILD_FILE");
    let suncols = read("OPENMT2001_SUNCOLS");
    let map = read("OPENMT2001_INTRO_MAP");
    let camera = CameraSample {
        eye: [-53., 30., 74.],
        target: [-53., 5., 44.],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let mut scene = render_scene::assemble(Request {
        build_file: &build_file,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &executable,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(12),
        camera: Some(camera),
        width: 1280,
        height: 960,
        animated: true,
    })
    .unwrap();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let borrowed = scene
        .draws
        .iter()
        .map(|draw| draw.borrowed())
        .collect::<Vec<_>>();
    let mut individual = renderer
        .prepare(&borrowed, scene.camera, scene.fog)
        .unwrap();
    let mut batch = renderer
        .prepare(&borrowed, scene.camera, scene.fog)
        .unwrap();
    let mut menu = MenuCamera::new(camera);
    let mut old_time = std::time::Duration::ZERO;
    let mut batch_time = std::time::Duration::ZERO;
    let mut reframe_time = std::time::Duration::ZERO;
    for tick in 0..100 {
        let started = Instant::now();
        let changed = scene.reframe(menu.sample(tick, 0.25)).unwrap();
        reframe_time += started.elapsed();
        let started = Instant::now();
        for (index, draw) in scene.draws.iter().enumerate() {
            if changed.contains(&index) {
                renderer
                    .update_dynamic_surface(
                        &mut individual,
                        index,
                        &draw.borrowed(),
                        scene.camera,
                        scene.fog,
                    )
                    .unwrap();
            } else {
                renderer
                    .update_camera(
                        &individual,
                        index,
                        draw.view_projection.unwrap_or(scene.camera),
                        scene.fog,
                    )
                    .unwrap();
            }
        }
        old_time += started.elapsed();
        let started = Instant::now();
        for &index in &changed {
            renderer
                .update_dynamic_vertices(&mut batch, index, &scene.draws[index].borrowed())
                .unwrap();
        }
        let cameras = scene
            .draws
            .iter()
            .map(|draw| draw.view_projection.unwrap_or(scene.camera))
            .collect::<Vec<_>>();
        renderer
            .update_cameras(&batch, &cameras, scene.fog)
            .unwrap();
        batch_time += started.elapsed();
        if [0, 50, 99].contains(&tick) {
            let reference = format!("frontend/menu-individual-{tick}.png");
            let actual = format!("frontend/menu-batch-{tick}.png");
            crate::capture_test::capture_standalone(&renderer, &individual, &reference);
            crate::capture_test::capture_standalone(&renderer, &batch, &actual);
            assert_eq!(
                fs::read(crate::capture_test::output_path(&reference)).unwrap(),
                fs::read(crate::capture_test::output_path(&actual)).unwrap()
            );
            let fresh = format!("frontend/menu-fresh-{tick}.png");
            crate::capture_test::capture_scene_world_overlays(&renderer, &scene, &[], &[], &fresh);
            assert_eq!(
                fs::read(crate::capture_test::output_path(&fresh)).unwrap(),
                fs::read(crate::capture_test::output_path(&actual)).unwrap()
            );
        } else {
            // Flush staging uploads equally without adding offscreen rendering
            // to the CPU preparation measurements.
            renderer.queue.submit([]);
            renderer
                .device
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
        }
    }
    // Individual APIs must still isolate each draw's camera and fog slots.
    let alternate = scene.fog.map(|mut fog| {
        fog.color ^= 0x00ff0000;
        fog
    });
    renderer
        .update_camera(&individual, 0, scene.camera, alternate)
        .unwrap();
    renderer
        .update_camera(&batch, 0, scene.camera, alternate)
        .unwrap();
    crate::capture_test::capture_standalone(
        &renderer,
        &individual,
        "frontend/menu-individual-fog.png",
    );
    crate::capture_test::capture_standalone(&renderer, &batch, "frontend/menu-batch-fog.png");
    assert_eq!(
        fs::read(openmt2001::fixtures::path("MENU_INDIVIDUAL_FOG")).unwrap(),
        fs::read(openmt2001::fixtures::path("MENU_BATCH_FOG")).unwrap()
    );
    // A material replacement introduces a standalone uniform buffer. The
    // batch API must update that live buffer as well as the retained arena.
    let mut pixels = scene.draws[0].rgba.to_vec();
    pixels[0] ^= 255;
    scene.draws[0].rgba = pixels.into();
    let extra = scene.draws[0].vertices.clone();
    scene.draws[0].vertices.extend(extra);
    if let Some(values) = &mut scene.draws[0].specular {
        values.extend(values.clone());
    }
    if let Some(values) = &mut scene.draws[0].diffuse_alpha {
        values.extend(values.clone());
    }
    if let Some(secondary) = &mut scene.draws[0].secondary {
        secondary.uv.extend(secondary.uv.clone());
    }
    renderer
        .update_dynamic_surface(
            &mut individual,
            0,
            &scene.draws[0].borrowed(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    renderer
        .update_dynamic_vertices(&mut batch, 0, &scene.draws[0].borrowed())
        .unwrap();
    let borrowed = scene
        .draws
        .iter()
        .map(|draw| draw.borrowed())
        .collect::<Vec<_>>();
    renderer
        .refresh_prepared(&mut individual, &borrowed, scene.camera, scene.fog)
        .unwrap();
    renderer
        .refresh_prepared(&mut batch, &borrowed, scene.camera, scene.fog)
        .unwrap();
    let changed = scene.reframe(menu.sample(125, 0.5)).unwrap();
    for (index, draw) in scene.draws.iter().enumerate() {
        if changed.contains(&index) {
            renderer
                .update_dynamic_surface(
                    &mut individual,
                    index,
                    &draw.borrowed(),
                    scene.camera,
                    scene.fog,
                )
                .unwrap();
            renderer
                .update_dynamic_vertices(&mut batch, index, &draw.borrowed())
                .unwrap();
        } else {
            renderer
                .update_camera(
                    &individual,
                    index,
                    draw.view_projection.unwrap_or(scene.camera),
                    scene.fog,
                )
                .unwrap();
        }
    }
    let cameras = scene
        .draws
        .iter()
        .map(|draw| draw.view_projection.unwrap_or(scene.camera))
        .collect::<Vec<_>>();
    renderer
        .update_cameras(&batch, &cameras, scene.fog)
        .unwrap();
    crate::capture_test::capture_standalone(
        &renderer,
        &individual,
        "frontend/menu-individual-replaced.png",
    );
    crate::capture_test::capture_standalone(&renderer, &batch, "frontend/menu-batch-replaced.png");
    assert_eq!(
        fs::read(openmt2001::fixtures::path("MENU_INDIVIDUAL_REPLACED")).unwrap(),
        fs::read(openmt2001::fixtures::path("MENU_BATCH_REPLACED")).unwrap()
    );
    eprintln!(
        "Original menu {} draws, 100 moving frames: reframe {reframe_time:?}, individual uploads {old_time:?}, batch uploads {batch_time:?}",
        scene.draws.len()
    );
}

#[cfg(feature = "capture")]
#[test]
#[ignore = "requires original intro assets and GPU"]
fn capture_mr_monopoly_in_intro_city() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build_file = read("OPENMT2001_BUILD_FILE");
    let suncols = read("OPENMT2001_SUNCOLS");
    let map = read("OPENMT2001_INTRO_MAP");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let actors = crate::auction_scene::Assets::load_result(&image, &index, &archive).unwrap();
    let actor_draws = actors
        .draws(&crate::auction_scene::result_layout((10, 10)).unwrap())
        .unwrap();
    let camera = CameraSample {
        eye: [-53., 30., 74.],
        target: [-53., 5., 44.],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let mut scene = render_scene::assemble(Request {
        build_file: &build_file,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &executable,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(12),
        camera: Some(camera),
        width: 1280,
        height: 960,
        animated: true,
    })
    .unwrap();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let mut city = renderer
        .prepare(
            &scene
                .draws
                .iter()
                .map(|draw| draw.borrowed())
                .collect::<Vec<_>>(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    let actor = renderer
        .prepare(
            &actor_draws
                .iter()
                .map(|draw| draw.borrowed())
                .collect::<Vec<_>>(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    let path = std::env::var("OPENMT2001_MR_MONOPOLY_CAPTURE")
        .unwrap_or_else(|_| "frontend/mr-monopoly-menu-capture.png".into());
    crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor], &[], &path);
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let language = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let time_assets =
        crate::time_panel::Assets::load(&index, &archive, installer, &language, &executable)
            .unwrap();
    let time = time_assets
        .prepare(
            &renderer,
            crate::time_panel::Display {
                year: 1930,
                hour: 12,
                speed: 1,
                name: "Player",
                cash: Some(8000.),
                fraction: 312,
                character: Some(4),
                cash_icon_phase: 0,
            },
        )
        .unwrap();
    let navigation = crate::navigation::Navigation::load(&index, &archive, &executable)
        .unwrap()
        .prepare(&renderer)
        .unwrap();
    crate::capture_test::capture_prepared_world_overlays(
        &renderer,
        &city,
        &[&actor],
        &[&navigation[0], &time],
        "frontend/time-panel-city-corrected.png",
    );
    let language = openmt2001::fixtures::path("EXE")
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let options_assets =
        crate::game_options::Assets::load(&index, &archive, installer, &language).unwrap();
    let options = options_assets
        .prepare(&renderer, Some(crate::game_options::Button::Continue), None)
        .unwrap();
    crate::capture_test::capture_prepared_world_overlays(
        &renderer,
        &city,
        &[&actor],
        &[&options],
        "frontend/game-options-city.png",
    );
    let mut tutorial_options = crate::game_options::View::default();
    tutorial_options.open(false);
    tutorial_options.set_save_disabled(true);
    tutorial_options
        .update(&options_assets, &renderer, None, 255)
        .unwrap();
    crate::capture_test::capture_prepared_world_overlays(
        &renderer,
        &city,
        &[&actor],
        &[tutorial_options.prepared.as_ref().unwrap()],
        "frontend/game-options-tutorial-save-disabled.png",
    );
    let settings_assets =
        crate::settings_menu::Assets::load(&index, &archive, installer, &language).unwrap();
    let settings = settings_assets
        .prepare(
            &renderer,
            crate::settings_menu::Page::Main,
            None,
            None,
            crate::settings_menu::Levels::default(),
        )
        .unwrap();
    crate::capture_test::capture_prepared_world_overlays(
        &renderer,
        &city,
        &[&actor],
        &[&settings],
        "frontend/settings-menu-city.png",
    );
    let sound = settings_assets
        .prepare(
            &renderer,
            crate::settings_menu::Page::Sound,
            None,
            None,
            crate::settings_menu::Levels {
                music: 60,
                effects: 25,
            },
        )
        .unwrap();
    crate::capture_test::capture_prepared_world_overlays(
        &renderer,
        &city,
        &[&actor],
        &[&sound],
        "frontend/sound-settings-city.png",
    );
    for (name, page) in [
        ("graphics", crate::settings_menu::Page::Graphics),
        ("gameplay", crate::settings_menu::Page::Gameplay),
    ] {
        let panel = settings_assets
            .prepare(
                &renderer,
                page,
                None,
                None,
                crate::settings_menu::Levels::default(),
            )
            .unwrap();
        crate::capture_test::capture_prepared_world_overlays(
            &renderer,
            &city,
            &[&actor],
            &[&panel],
            &format!("frontend/{name}-settings-city.png"),
        );
    }
    // Controlled flyby-to-menu frames. The original recording has splash
    // skips, so these are native-rule checkpoints rather than synchronized
    // movie timestamps.
    let track = fs::read(installer.join("Standard/Cut_scenes/intro_low.tm")).unwrap();
    let track = Flyby::parse(&track).unwrap();
    for frame in [1, 100] {
        scene.reframe(track.sample(frame).unwrap()).unwrap();
        for index in 0..actor_draws.len() {
            renderer
                .update_camera(&actor, index, scene.camera, scene.fog)
                .unwrap();
        }
        crate::capture_test::capture_scene_world_overlays(
            &renderer,
            &scene,
            &[&actor],
            &[],
            &format!("frontend/mr-monopoly-intro-frame-{frame:04}.png"),
        );
    }
    city = renderer
        .prepare(
            &scene
                .draws
                .iter()
                .map(|draw| draw.borrowed())
                .collect::<Vec<_>>(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    let final_flyby = track.sample(track.frame_count() - 1).unwrap();
    let mut menu_camera = MenuCamera::new(final_flyby);
    for tick in [0, 10, 50] {
        let camera = menu_camera.sample(tick, 0.);
        let changed = scene.reframe(camera).unwrap();
        for (index, draw) in scene.draws.iter().enumerate() {
            if changed.contains(&index) {
                renderer
                    .update_dynamic_surface(
                        &mut city,
                        index,
                        &draw.borrowed(),
                        scene.camera,
                        scene.fog,
                    )
                    .unwrap();
            } else {
                renderer
                    .update_camera(
                        &city,
                        index,
                        draw.view_projection.unwrap_or(scene.camera),
                        scene.fog,
                    )
                    .unwrap();
            }
        }
        for index in 0..actor_draws.len() {
            renderer
                .update_camera(&actor, index, scene.camera, scene.fog)
                .unwrap();
        }
        crate::capture_test::capture_prepared_world_overlays(
            &renderer,
            &city,
            &[&actor],
            &[],
            &format!("frontend/menu-camera-transition-{tick:02}.png"),
        );
    }
}

#[test]
#[ignore = "requires original intro assets and a GPU adapter"]
fn normal_city_star_uploads_apply_detail_before_borrowing() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build_file = read("OPENMT2001_BUILD_FILE");
    let suncols = read("OPENMT2001_SUNCOLS");
    let map = read("OPENMT2001_INTRO_MAP");
    let camera = CameraSample {
        eye: [0., 50., 0.],
        target: [0., 50., 1.],
        up: [0., 1., 0.],
        vertical_fov: NORMAL_CITY_VERTICAL_FOV,
    };
    let mut scene = render_scene::assemble(Request {
        build_file: &build_file,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &executable,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(20),
        camera: Some(camera),
        width: 1280,
        height: 960,
        animated: true,
    })
    .unwrap();
    let stars = scene
        .draws
        .iter()
        .position(|d| d.mode == openmt2001::render::SurfaceMode::Stars)
        .unwrap();
    let capacity = scene.draws[stars].vertices.len();
    assert!(capacity > 0);
    let changed = scene.advance_daylight(20, 0, camera).unwrap();
    assert_eq!(
        scene.draws[stars].borrowed().vertices.len(),
        capacity,
        "night premise"
    );
    // The actual snapshot owner uses this seam before prepare/refresh/take.
    normal_city_scene_updates(&mut scene, camera, Some(0), changed).unwrap();
    assert!(scene.draws[stars].borrowed().vertices.is_empty());
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let mut prepared = renderer
        .prepare(
            &scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    // Observe the actual arguments supplied to the renderer, not just a
    // policy boolean. Masked updates must never expand/write full stars.
    let upload = |scene: &Scene, changed: &[usize], prepared: &mut PreparedScene| {
        let mut star_vertices = 0;
        for &index in changed {
            let draw = scene.draws[index].borrowed();
            if index == stars {
                star_vertices += draw.vertices.len();
            }
            renderer
                .update_dynamic_vertices(prepared, index, &draw)
                .unwrap();
        }
        star_vertices
    };
    for detail in [0, 1, 2, 1, 0, 2] {
        for event in 0..2 {
            let changed = if event == 0 {
                scene.reframe(camera).unwrap()
            } else {
                let mut changed = scene.advance_environment(37, camera.eye).unwrap();
                changed.extend(scene.advance_daylight(20, 45, camera).unwrap());
                changed
            };
            // Same ordering function used by camera and environment owners.
            let changed =
                normal_city_scene_updates(&mut scene, camera, Some(detail), changed).unwrap();
            assert_eq!(scene.draws[stars].vertices.len(), capacity);
            let expected = if detail == 2 { capacity } else { 0 };
            assert_eq!(scene.draws[stars].borrowed().vertices.len(), expected);
            let sent = upload(&scene, &changed, &mut prepared);
            assert_eq!(
                sent,
                if changed.contains(&stars) {
                    expected
                } else {
                    0
                },
                "Detail{detail} event{event}: exact star payload to renderer"
            );
            if detail != 2 {
                assert_eq!(sent, 0, "no masked full-star upload");
            }
        }
    }
    // Same-hour replacement, then the separate post-preparation camera seam.
    let mut replacement = scene.clone();
    replacement.retain_environment(&scene).unwrap();
    let changed = replacement.advance_daylight(20, 45, camera).unwrap();
    normal_city_scene_updates(&mut replacement, camera, Some(1), changed).unwrap();
    assert!(replacement.draws[stars].borrowed().vertices.is_empty());
    let mut replacement_prepared = renderer
        .prepare(
            &replacement
                .draws
                .iter()
                .map(|d| d.borrowed())
                .collect::<Vec<_>>(),
            replacement.camera,
            replacement.fog,
        )
        .unwrap();
    let changed = replacement.reframe(camera).unwrap();
    let changed = normal_city_scene_updates(&mut replacement, camera, Some(1), changed).unwrap();
    assert_eq!(upload(&replacement, &changed, &mut replacement_prepared), 0);
    let changed = replacement.reframe(camera).unwrap();
    let changed = normal_city_scene_updates(&mut replacement, camera, Some(2), changed).unwrap();
    assert_eq!(
        upload(&replacement, &changed, &mut replacement_prepared),
        capacity,
        "High grows/restores the same previously masked prepared surface"
    );
    // Explicitly unowned/result callers retain generic night admission.
    replacement.apply_normal_star_detail(0, camera).unwrap();
    let changed = replacement.reframe(camera).unwrap();
    let changed = normal_city_scene_updates(&mut replacement, camera, None, changed).unwrap();
    assert_eq!(
        upload(&replacement, &changed, &mut replacement_prepared),
        capacity
    );
}

#[test]
#[ignore = "requires original intro assets and a GPU adapter"]
fn right_drag_mode12_reframes_retained_gpu_city() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let build_file = read("OPENMT2001_BUILD_FILE");
    let suncols = read("OPENMT2001_SUNCOLS");
    let map = read("OPENMT2001_INTRO_MAP");
    let mut controls = openmt2001::camera::BlockCamera {
        center: [0.; 3],
        angle: -std::f32::consts::FRAC_PI_2,
        height: 180.,
        radius: 220.,
    };
    let (eye, target) =
        mode12_input_tick(&mut controls, false, [false; 4], [0; 2], 50, false).unwrap();
    let mut camera = CameraSample {
        eye,
        target,
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let mut scene = render_scene::assemble(Request {
        build_file: &build_file,
        source: Source::Map(std::str::from_utf8(&map).unwrap()),
        original: Some(OriginalAssets {
            executable: &executable,
            directory: &directory,
            archive: &archive,
        }),
        suncols: Some(&suncols),
        phase: Some(12),
        camera: Some(camera),
        width: 1280,
        height: 960,
        animated: true,
    })
    .unwrap();
    let layout = scene
        .draws
        .iter()
        .map(|draw| draw.vertices.len())
        .collect::<Vec<_>>();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let mut prepared = renderer
        .prepare(
            &scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    let mut batch = renderer
        .prepare(
            &scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
            scene.camera,
            scene.fog,
        )
        .unwrap();
    let original_matrix = scene.camera;
    let first = mode12_input_tick(&mut controls, false, [false; 4], [8, -12], 50, false).unwrap();
    assert_eq!(first.0[0], eye[0], "horizontal orbit applies next callback");
    assert_ne!(first.0[1], eye[1], "zoom applies in this callback");
    let second = mode12_input_tick(&mut controls, false, [false; 4], [0; 2], 50, false).unwrap();
    assert_ne!(second.0[0], first.0[0]);
    camera.eye = second.0;
    camera.target = second.1;
    let changed = scene.reframe(camera).unwrap();
    assert_ne!(scene.camera, original_matrix);
    for (index, draw) in scene.draws.iter().enumerate() {
        if changed.contains(&index) {
            renderer
                .update_dynamic_surface(
                    &mut prepared,
                    index,
                    &draw.borrowed(),
                    draw.view_projection.unwrap_or(scene.camera),
                    scene.fog,
                )
                .unwrap();
        } else {
            renderer
                .update_camera(
                    &prepared,
                    index,
                    draw.view_projection.unwrap_or(scene.camera),
                    scene.fog,
                )
                .unwrap();
        }
    }
    // Since #518 the junction traffic-light glows share the light-halo draws
    // and are rebuilt for the new camera (0x44dbe0 emits a point only where
    // its post faces the eye), so their vertex counts follow the reframe.
    // Every other draw keeps its assembled topology, and every rebuilt halo
    // draw is reported as changed so the renderer re-uploads it.
    for (index, draw) in scene.draws.iter().enumerate() {
        if draw.mode == openmt2001::render::SurfaceMode::LightHalos {
            if draw.vertices.len() != layout[index] {
                assert!(
                    changed.contains(&index),
                    "halo draw {index} not re-uploaded"
                );
            }
        } else {
            assert_eq!(draw.vertices.len(), layout[index], "draw {index}");
        }
    }
    for phase in 0..2 {
        let changed = if phase == 0 {
            changed.clone()
        } else {
            let mut changed = scene.advance_environment(37, camera.eye).unwrap();
            changed.extend(scene.advance_daylight(13, 45, camera).unwrap());
            changed.sort_unstable();
            changed.dedup();
            for &index in &changed {
                renderer
                    .update_dynamic_surface(
                        &mut prepared,
                        index,
                        &scene.draws[index].borrowed(),
                        scene.camera,
                        scene.fog,
                    )
                    .unwrap();
            }
            for (index, draw) in scene.draws.iter().enumerate() {
                renderer
                    .update_camera(
                        &prepared,
                        index,
                        draw.view_projection.unwrap_or(scene.camera),
                        scene.fog,
                    )
                    .unwrap();
            }
            changed
        };
        for index in changed {
            renderer
                .update_dynamic_vertices(&mut batch, index, &scene.draws[index].borrowed())
                .unwrap();
        }
        let cameras = scene
            .draws
            .iter()
            .map(|draw| draw.view_projection.unwrap_or(scene.camera))
            .collect::<Vec<_>>();
        renderer
            .update_cameras(&batch, &cameras, scene.fog)
            .unwrap();
        #[cfg(feature = "capture")]
        {
            let reference = format!("frontend/city-individual-phase-{phase}.png");
            let actual = format!("frontend/city-batch-phase-{phase}.png");
            let fresh = format!("frontend/city-fresh-phase-{phase}.png");
            crate::capture_test::capture_standalone(&renderer, &prepared, &reference);
            crate::capture_test::capture_standalone(&renderer, &batch, &actual);
            crate::capture_test::capture_scene_world_overlays(&renderer, &scene, &[], &[], &fresh);
            let actual = crate::capture_test::output_path(&actual);
            assert_eq!(
                fs::read(crate::capture_test::output_path(&reference)).unwrap(),
                fs::read(&actual).unwrap()
            );
            assert_eq!(
                fs::read(crate::capture_test::output_path(&fresh)).unwrap(),
                fs::read(actual).unwrap()
            );
        }
    }
}
#[test]
#[ignore = "requires supplied v1.4 assets"]
fn sandbox_and_bonus_launch_use_retained_hubs() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let map = openmt2001::fixtures::path("MAP");
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let executable = read("OPENMT2001_EXE");
            let exe_path = openmt2001::fixtures::path("EXE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let inputs = SceneInputs {
                process_runtime: None,
                compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
                character_names: crate::fresh_city::character_names(
                    &image,
                    &exe_path
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("US/language/usa"),
                )
                .unwrap(),
                installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                language: exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa"),
                maps: map.parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"),
                executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            };
            let catalog = crate::campaign_store::load_catalog(
                &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                inputs.maps.parent().unwrap(),
            )
            .unwrap();
            let camera = CameraSample {
                eye: [0., 180., 220.],
                target: [0.; 3],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            };
            use crate::singleplayer::{Difficulty, Preview};
            for (preview, id, difficulty, expected_cash, expected_people) in [
                (
                    Preview::Sandbox,
                    400002,
                    Difficulty::Easy,
                    Some(20000.),
                    500,
                ),
                (
                    Preview::Scenario(400001),
                    400001,
                    Difficulty::Easy,
                    Some(40000000.),
                    500,
                ),
                (
                    Preview::Scenario(400001),
                    400001,
                    Difficulty::Medium,
                    Some(30000000.),
                    500,
                ),
                (
                    Preview::Scenario(400001),
                    400001,
                    Difficulty::Hard,
                    Some(20000000.),
                    500,
                ),
                (
                    Preview::Scenario(200016),
                    200016,
                    Difficulty::Easy,
                    Some(8000.),
                    410,
                ),
                (Preview::Scenario(100001), 100001, Difficulty::Easy, None, 0),
            ] {
                let setup = crate::singleplayer::Setup {
                    preview,
                    difficulty,
                    character_index: 4,
                    profile_name: b"Launch test".to_vec(),
                    catalog: catalog.clone(),
                    profile_path: openmt2001::fixtures::scratch("FRONTEND")
                        .join("launch-test-profile.lua"),
                };
                let mut cache = SnapshotCache::default();
                let (loaded, session) = inputs.fresh_preview(setup, camera, &mut cache).unwrap();
                assert_eq!(session.catalog.selected_level().unwrap().id(), id);
                assert_eq!(
                    session.world.ledgers.residents.active_count,
                    expected_people
                );
                if let Some(cash) = expected_cash {
                    assert_eq!(session.world.player_money, cash);
                    assert_eq!(session.world.ledgers.players[0].cash, cash);
                }
                if matches!(id, 400001 | 400002) {
                    assert_eq!(session.roster.manager.players.len(), 1);
                }
                if id == 400001 {
                    let row = session.world.catalog.info.row(42).unwrap();
                    for (at, expected) in [(0x38, 1), (0x3c, 30), (0x48, 1), (0x44, 20), (0x40, 6)]
                    {
                        assert_eq!(
                            u32::from_le_bytes(row[at..at + 4].try_into().unwrap()),
                            expected
                        );
                    }
                }
                println!(
                    "Launch contract id{id} cash{} people{} roster{}",
                    session.world.ledgers.players[0].cash,
                    session.world.ledgers.residents.active_count,
                    session.roster.manager.players.len()
                );
                if id != 100001 {
                    assert!(session.world.ledgers.residents.active_count > 0);
                }
                assert!(loaded.player_info.is_some());
                assert!(!loaded.scene.draws.is_empty());
                if id == 100001 {
                    let plain = session
                        .scene(
                            &inputs.build_file,
                            OriginalAssets {
                                executable: &inputs.executable,
                                directory: &inputs.directory,
                                archive: &inputs.archive,
                            },
                            &inputs.suncols,
                            Some(camera),
                            true,
                        )
                        .unwrap();
                    assert!(
                        cache.city.as_ref().unwrap().draws.len() > plain.draws.len(),
                        "original Tutorial2 map signs and banners must reach the live city"
                    );
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
#[ignore = "requires supplied original saves, scripts and assets"]
fn building_tree_saved_snapshot_retention() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let map = openmt2001::fixtures::path("MAP");
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let executable = read("OPENMT2001_EXE");
            let exe_path = openmt2001::fixtures::path("EXE");
            let inputs = SceneInputs {
                process_runtime:None,
                compatibility: crate::compatibility::Mode::Modern,
                style: test_board_style(&executable), time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("tree-snapshot-profiles"),
                character_names: crate::fresh_city::character_names(
                    &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                    &exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                ).unwrap(),
                installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                maps: map.parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"), executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"), archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            };
            let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
                up: [0., 1., 0.], vertical_fov: 45. };
            for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
                let mut cache = SnapshotCache::default();
                let (_, mut session) = inputs.saved_preview(
                    &openmt2001::fixtures::path(key), camera, &mut cache).unwrap();
                let state = session.models.tree_state.as_ref().unwrap().clone();
                let revision = state.borrow().revision();
                let mut seen = std::collections::BTreeSet::new();
                for (&slot, _) in &session.models.bindings {
                    use openmt2001::trees::building::{OWNER_COUNT_OFFSET, OWNER_INDICES_OFFSET};
                    let record = &session.world.buildings[slot];
                    let word = |at| u32::from_le_bytes(record[at..at+4].try_into().unwrap()) as usize;
                    let pool = state.borrow();
                    let indices = pool.indices(slot).expect("bound model must own its tree list, including empty lists");
                    assert_eq!(word(OWNER_COUNT_OFFSET), indices.len());
                    for (n, &index) in indices.iter().enumerate() {
                        assert_eq!(word(OWNER_INDICES_OFFSET + n*4), index);
                        assert!(index >= 137 && seen.insert(index));
                        assert!(pool.pool().slot(index).is_some());
                    }
                }
                assert_eq!(state.borrow().pool().count(), 137 + seen.len());
                let retained = cache.tree_instances.as_ref().unwrap().1.clone();
                let _ = inputs.inspect_fresh(&mut session, camera, &mut cache).unwrap();
                assert!(std::sync::Arc::ptr_eq(&retained, &cache.tree_instances.as_ref().unwrap().1));
                assert_eq!(state.borrow().revision(), revision, "snapshot cannot allocate tree IDs");
                assert!(std::rc::Rc::ptr_eq(&state, session.models.tree_state.as_ref().unwrap()));
                eprintln!("{key}: {} building trees, stable pool/revision across repeated scene snapshots", seen.len());
                if key == "OPENMT2001_SAVE_TWO" {
                    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
                    let board = inputs.styled_board(&image).unwrap();
                    let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
                    let assets = crate::fresh_city::CommandAssets { image: &image, board: &board, file: &file };
                    let mut selected = None;
                    'search: for category in 0..52 {
                        if !matches!(crate::build_panel::Context::from_session(&session,category).unwrap().entry(),
                            crate::build_panel::Entry::Open) { continue; }
                        for x in 0..=6 { for y in 0..=5 { for variant in 0..32 {
                            let request = crate::build_panel::Request { category: category as i32,
                                origin: [x,y], size: [1,2], height: 1, packed_model: variant << 16,
                                quality: 0, kind: 2, displayed_quote: 0., business: 44 };
                            if let Ok(quote) = session.inspect_build(&request,&assets)
                                && quote.allowed()
                                && let Some(preview) = &quote.preview
                                && !openmt2001::trees::building::locals(&file,preview.selected,1).unwrap().is_empty() {
                                eprintln!("Tree-bearing legal preview model{} category{category} lot{x},{y} variant{variant}",preview.selected.index);
                                selected = Some(request); break 'search;
                            }
                        }}}
                    }
                    let request = selected.expect("supplied skirmish admits a legally quoted 1x2 landscape");
                    let slot = openmt2001::building_pool::first_free(&session.world.buildings).unwrap().unwrap();
                    assert!(session.submit_build(&request,&assets,|_|Ok(())).unwrap().submitted);
                    let mut host = crate::live_host::Host::new(camera.eye);
                    host.running = true;
                    for _ in 0..350 {
                        host.tick(&mut session,&assets);
                        assert!(host.failure.is_none(),"{:?}",host.failure);
                        for prompt in host.take_prompts(&session) {
                            session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();
                        }
                        if state.borrow().indices(slot).is_some_and(|ids| !ids.is_empty()) { break; }
                    }
                    let ids = state.borrow().indices(slot).expect("real queued Build must bind tree ownership").to_vec();
                    assert!(!ids.is_empty());
                    let record = &session.world.buildings[slot];
                    assert_eq!(&record[4..8],&[request.origin[0] as u8,request.origin[1] as u8,
                        (request.origin[0]+request.size[0]-1) as u8,(request.origin[1]+request.size[1]-1) as u8]);
                    assert_eq!(record[8],request.height as u8);
                    assert_eq!(u32::from_le_bytes(record[0x24..0x28].try_into().unwrap()),request.kind);
                    let poses: Vec<_> = ids.iter().map(|&i| *state.borrow().pool().slot(i).unwrap()).collect();
                    let _ = inputs.inspect_fresh(&mut session,camera,&mut cache).unwrap();
                    assert!(!std::sync::Arc::ptr_eq(&retained,&cache.tree_instances.as_ref().unwrap().1));
                    let bytes = session.save_v2(&image,session.saved_cameras.unwrap(),1).unwrap();
                    let path = std::env::temp_dir().join(format!("building-tree-roundtrip-{}-{}.sav",
                        std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
                    struct SavedFixture(PathBuf);
                    impl Drop for SavedFixture { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }
                    let mut output = fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
                    let fixture = SavedFixture(path.clone());
                    std::io::Write::write_all(&mut output,&bytes).unwrap();
                    drop(output);
                    let (_, mut resumed) = inputs.saved_preview(&path,camera,&mut SnapshotCache::default()).unwrap();
                    let resumed_state = resumed.models.tree_state.as_ref().unwrap().clone();
                    let resumed_ids = resumed_state.borrow().indices(slot).unwrap().to_vec();
                    let restored: Vec<_> = resumed_ids.iter().map(|&i| *resumed_state.borrow().pool().slot(i).unwrap()).collect();
                    assert_eq!(restored,poses,"cold native/model rebind must retain tree poses");
                    assert_eq!(resumed_state.borrow().pool().count(),137+seen.len()+poses.len());
                    let mut resumed_host = crate::live_host::Host::new(camera.eye);
                    resumed_host.running = true;
                    let before = session.world.clock.timing[0];
                    assert_eq!(resumed.world.clock.timing[0],before);
                    for _ in 0..30 {
                        assert!(host.running && resumed_host.running);
                        assert!(!session.world.pause.has_reason() && !resumed.world.pause.has_reason());
                        host.tick(&mut session,&assets); resumed_host.tick(&mut resumed,&assets);
                        assert!(host.failure.is_none() && resumed_host.failure.is_none());
                        assert_eq!(session.world.clock,resumed.world.clock);
                    }
                    assert_eq!(session.world.clock.timing[0],before+30,"continuation must advance30 real logic ticks");
                    assert_eq!(state.borrow().indices(slot).unwrap(),ids);
                    assert_eq!(resumed_state.borrow().indices(slot).unwrap(),resumed_ids);
                    eprintln!("Legal landscape on category{} slot{slot}: {} trees through queued construction, scene invalidation, export/cold reload and30 continuation ticks",request.category,poses.len());
                    drop(fixture);
                    assert!(!path.exists(),"owned roundtrip fixture removed");
                }
            }
        }).unwrap().join().unwrap();
}
#[test]
#[ignore = "requires both supplied original saves and v1.4 assets"]
fn load_menu_uses_retained_original_session() {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let map = openmt2001::fixtures::path("MAP");
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let executable = read("OPENMT2001_EXE");
        let exe_path = openmt2001::fixtures::path("EXE");
        let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
            character_names: crate::fresh_city::character_names(
                &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                &exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
            ).unwrap(),
            installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
            language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
            maps: map.parent().unwrap().to_path_buf(),
            build_file: read("OPENMT2001_BUILD_FILE"), executable,
            directory: read("OPENMT2001_ARCHIVE_DIR"), archive: read("OPENMT2001_ARCHIVE_BIN"),
            suncols: read("OPENMT2001_SUNCOLS"),
        };
        let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
            up: [0., 1., 0.], vertical_fov: 45.,
                };
        for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let mut cache = SnapshotCache::default();
            let (loaded, mut session) = inputs.saved_preview(
                std::path::Path::new(&openmt2001::fixtures::path(key)), camera,
                &mut cache,
                        ).unwrap();
            if key == "OPENMT2001_SAVE_ONE" {
                assert!(loaded.scene.diagnostics.iter().any(|line|
                    line.contains("45 native-order detail replacements")));
                let baseline = session.scene(&inputs.build_file,
                    OriginalAssets { executable: &inputs.executable,
                        directory: &inputs.directory, archive: &inputs.archive,
                                },
                    &inputs.suncols, Some(loaded.saved_viewpoint.unwrap().0), true,
                            ).unwrap();
                let mut current_models = baseline.clone();
                let mut current_cache = SnapshotCache::default();
                let mut current_visibility = DetailVisibility::default();
                inputs.append_current_appearance(
                    &session, &mut current_models, &mut current_visibility,
                    &mut current_cache, false,
                ).unwrap();
                assert_eq!(current_models.draws.len(), cache.city.as_ref().unwrap().draws.len(),
                    "live model appearance must submit the same original details");
                let expected_draws = session.models.appearance_candidates.values()
                    .flat_map(|model| &model.renderer_templates)
                    .filter(|row| u32::from_le_bytes(row.row[..4].try_into().unwrap()) == 1)
                    .map(|row| if row.descriptor == 0x0108_0202 { 3 } else { 2 })
                    .sum::<usize>();
                assert_eq!(expected_draws, 105, "supplied active-row dynamic detail contract");
                // Static inactive rows are batched into shared atlas pages, not
                // counted by the active-row dynamic renderer-template formula.
                let static_categories = baseline.inspection_buildings.iter().filter_map(|building| {
                    let slot = building.runtime_slot?;
                    let record = &session.world.buildings[slot];
                    (record[0x3f] != 0 && detail_model_at_unit_scale(record)
                        && session.models.appearance_candidates.get(&slot)?.renderer_templates.iter()
                            .any(|row| crate::static_detail::supports(row.descriptor)
                                && u32::from_le_bytes(row.row[..4].try_into().unwrap()) == 0))
                        .then_some(building.category)
                }).collect::<std::collections::BTreeSet<_>>();
                assert!(!static_categories.is_empty(), "supplied save must exercise static details");
                let check_details = |scene: &Scene, detail_cache: &SnapshotCache,
                                     visibility: &DetailVisibility, base_draws: usize| {
                    let mut end = base_draws;
                    let mut static_draws = 0;
                    let mut categories = std::collections::BTreeSet::new();
                    for range in &detail_cache.static_detail_ranges {
                        assert!(range.start >= end && range.end <= scene.draws.len(),
                            "static ranges must be ordered, disjoint, and appended to the base");
                        end = range.end;
                        static_draws += range.len();
                        for index in range.clone() {
                            let draw = &scene.draws[index];
                            assert!(!draw.vertices.is_empty(), "static page must contain geometry");
                            // Category spans are distance gated; fixed utility/marina
                            // details are submitted directly (004b7bbf, #376).
                            let mut spans = visibility.spans.iter().filter(|span| span.draw == index)
                                .map(|span| (span.start, span.positions.len(), Some(span.category)))
                                .chain(detail_cache.fixed_detail_spans.iter()
                                    .filter(|(draw, _)| *draw == index)
                                    .map(|(_, vertices)| (vertices.start, vertices.len(), None)))
                                .collect::<Vec<_>>();
                            spans.sort_by_key(|span| span.0);
                            let mut vertex_end = 0;
                            for (start, len, category) in spans {
                                assert_eq!(start, vertex_end, "static spans must cover each page once");
                                vertex_end += len;
                                if let Some(category) = category {
                                    assert!(static_categories.contains(&category),
                                        "static category must own an eligible supported inactive row");
                                    categories.insert(category);
                                }
                            }
                            assert_eq!(vertex_end, draw.vertices.len(), "static page has untracked vertices");
                        }
                    }
                    assert!(static_draws > 0, "supplied static rows must produce atlas pages");
                    assert_eq!(categories, static_categories, "static category coverage");
                    assert_eq!(scene.draws.len() - base_draws - static_draws, expected_draws,
                        "saved dynamic details must retain the active-row count");
                    static_draws
                };
                let retained = cache.city.as_ref().unwrap();
                let static_draws = check_details(retained, &cache, &cache.detail_visibility,
                    baseline.draws.len());
                assert_eq!(check_details(&current_models, &current_cache, &current_visibility,
                    baseline.draws.len()), static_draws, "saved/current static page count");
                // Same immutable Session, pose, time and append helpers; only
                // the scene projection input changes. No host tick or replay.
                let control_camera = CameraSample { vertical_fov: 45.,
                    ..loaded.saved_viewpoint.unwrap().0 };
                let mut control = session.scene(&inputs.build_file,
                    OriginalAssets { executable: &inputs.executable,
                        directory: &inputs.directory, archive: &inputs.archive },
                    &inputs.suncols, Some(control_camera), true).unwrap();
                let control_base = control.draws.len();
                assert_eq!(control_base, baseline.draws.len(), "FOV must not change base draw admission");
                let mut control_cache = SnapshotCache::default();
                let mut control_visibility = DetailVisibility::default();
                inputs.append_saved_appearance(&session, &mut control, &mut control_visibility,
                    &mut control_cache).unwrap();
                inputs.append_current_appearance(&session, &mut control, &mut control_visibility,
                    &mut control_cache, true).unwrap();
                assert_eq!(check_details(&control, &control_cache, &control_visibility, control_base),
                    static_draws, "45/53 static page count");
                assert_eq!(control_cache.static_detail_ranges, cache.static_detail_ranges,
                    "45/53 static page placement");
                for (control_draw, saved_draw) in control.draws[control_base..].iter()
                    .zip(&retained.draws[baseline.draws.len()..]) {
                    assert_eq!(control_draw.vertices, saved_draw.vertices, "45/53 detail geometry");
                    assert_eq!(control_draw.mode, saved_draw.mode, "45/53 detail material mode");
                    assert_eq!(control_draw.specular, saved_draw.specular, "45/53 detail lighting");
                    assert_eq!((control_draw.width, control_draw.height),
                        (saved_draw.width, saved_draw.height), "45/53 detail atlas dimensions");
                    assert_eq!(control_draw.rgba, saved_draw.rgba, "45/53 detail atlas pixels");
                }
                eprintln!("SAVE_ONE appearance: {expected_draws} dynamic draws + {static_draws} static pages; \
                    static categories {static_categories:?}; identical45/53 detail submission");
                // Controlled ownership-only mutation: the same-hour snapshot must
                // rebuild appearance, then restore the original material submission.
                let original_record = session.world.buildings[12];
                assert_eq!(original_record[0x2d], 1);
                let original_materials = cache.city.as_ref().unwrap().draws.iter()
                    .map(|draw| (draw.width, draw.height, draw.rgba.clone())).collect::<Vec<_>>();
                let mut absent_business = original_record;
                absent_business[0x1c..0x20].copy_from_slice(&u32::MAX.to_le_bytes());
                let payload = |record: &[u8; openmt2001::building_pool::RECORD_SIZE]| {
                    let (class, labels) = SceneInputs::appearance_payload(&session, record,
                        cache.appearance_assets.as_ref().unwrap()).unwrap();
                    (class, crate::save_appearance::banner_material_indices(
                        &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                        labels.banner.unwrap()).unwrap())
                };
                assert_eq!(payload(&original_record), (3, (48, 3, Some(5))));
                assert_eq!(payload(&absent_business), (3, (48, 3, None)));
                session.world.buildings[12][0x2d] = 0;
                let city = inputs.inspect_fresh(&mut session, loaded.saved_viewpoint.unwrap().0,
                    &mut cache).unwrap();
                assert!(!city.reuse_city, "owner change must rebuild cached appearance");
                let (class, labels) = SceneInputs::appearance_payload(&session,
                    &session.world.buildings[12], cache.appearance_assets.as_ref().unwrap()).unwrap();
                assert_eq!(class, 10);
                assert_eq!(crate::save_appearance::banner_material_indices(
                    &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                    labels.banner.unwrap()).unwrap(), (55, 286, Some(5)));
                assert_ne!(cache.city.as_ref().unwrap().draws.iter()
                    .map(|draw| (draw.width, draw.height, draw.rgba.clone())).collect::<Vec<_>>(),
                    original_materials, "owner change must reach actual scene materials");
                session.world.buildings[12] = original_record;
                let restored = inputs.inspect_fresh(&mut session, loaded.saved_viewpoint.unwrap().0,
                    &mut cache).unwrap();
                assert!(!restored.reuse_city);
                assert_eq!(cache.city.as_ref().unwrap().draws.iter()
                    .map(|draw| (draw.width, draw.height, draw.rgba.clone())).collect::<Vec<_>>(),
                    original_materials, "restored owner must restore original scene materials");
                let mut unchanged = cache.city.as_ref().unwrap().clone();
                inputs.append_current_appearance(
                    &session, &mut unchanged, &mut DetailVisibility::default(),
                    &mut SnapshotCache::default(), true,
                ).unwrap();
                assert_eq!(unchanged.draws.len(), cache.city.as_ref().unwrap().draws.len(),
                    "load-time identities must not submit duplicate live details");
                let (&slot, original_identity) = session.models.appearance_candidates
                    .iter().next().unwrap();
                let original_identity = original_identity.clone();
                session.models.appearance_candidates.remove(&slot);
                let mut rebound = baseline.clone();
                let mut rebound_cache = SnapshotCache::default();
                inputs.append_saved_appearance(&session, &mut rebound,
                    &mut DetailVisibility::default(), &mut rebound_cache,
                            ).unwrap();
                inputs.append_current_appearance(
                    &session, &mut rebound, &mut DetailVisibility::default(),
                    &mut rebound_cache, true,
                ).unwrap();
                assert_eq!(rebound.draws.len(), cache.city.as_ref().unwrap().draws.len(),
                    "a model without its saved identity must acquire current details");
                session.models.appearance_candidates.insert(slot, original_identity);
            }
            let saved = session.saved_cameras.as_ref().unwrap();
            let (visible, controls) = loaded.saved_viewpoint.unwrap();
            assert_eq!(visible.vertical_fov, 53., "ordinary saved owner retained menu FOV");
            let at = |offset: usize| {
                        f32::from_le_bytes(saved[0][offset..offset + 4].try_into().unwrap())
                    };
            assert_eq!(visible.eye, [at(0x4c), at(0x50), at(0x54)]);
            assert_eq!(visible.target, [at(0x58), at(0x5c), at(0x60)]);
            assert_eq!(visible.up, [at(0x64), at(0x68), at(0x6c)]);
            assert_ne!(visible.eye, camera.eye, "save still uses menu camera");
            assert!(controls.radius > 0.);
            let mut pan = controls;
            let (first_eye, first_target) = saved_camera_tick(&mut pan, [true, false, false, false]);
            for (actual, saved) in first_eye.into_iter().zip(visible.eye) {
                assert!((actual - saved).abs() < 0.01, "saved camera jumps at first pan callback");
            }
            for (actual, saved) in first_target.into_iter().zip(visible.target) {
                assert!((actual - saved).abs() < 0.01, "saved target jumps at first pan callback");
            }
            let (next_eye, next_target) = saved_camera_tick(&mut pan, [false; 4]);
            assert!((next_eye[0] - first_eye[0]).hypot(next_eye[2] - first_eye[2]) <= 2.01);
            assert!((next_target[0] - first_target[0]).hypot(next_target[2] - first_target[2]) <= 2.01);
            let page_up_height = page_height(controls.height, true);
            assert!((page_up_height - visible.eye[1]).abs() <= 10.);
            assert!(loaded.fresh_session.is_none());
            assert!(session.saved_logic_mode.is_some(), "{key} did not retain saved mode");
            assert!(!loaded.scene.draws.is_empty(), "{key} did not assemble a live city");
            assert!(session.campaign_profile.is_none(), "arbitrary save attached a V2 profile");
            if key == "OPENMT2001_SAVE_ONE" {
                // A renderer becomes available only after the load result is
                // delivered. Exercise the deferred mode-2 Prepare/Activate
                // handoff without requiring a GPU in this loader test.
                let category = session.world.city.categories.iter()
                    .position(|row| row[5] == 0).unwrap();
                session.saved_logic_mode = Some(2);
                let controller = &mut session.world.city.auction.controller;
                controller.reference = 0x7f4758;
                controller.record[4..8].copy_from_slice(&(category as u32).to_le_bytes());
                controller.record[0x7d] = 1;
                let (sender, receiver) = mpsc::channel::<crate::auction_bridge::Request>();
                let responder = std::thread::spawn(move || {
                    let first = receiver.recv().unwrap();
                    assert!(matches!(first.operation, crate::auction_bridge::Operation::Prepare { .. }));
                    first.reply.send(Ok(0)).unwrap();
                    let second = receiver.recv().unwrap();
                    assert!(matches!(second.operation, crate::auction_bridge::Operation::Activate));
                    second.reply.send(Ok(0)).unwrap();
                });
                let mut host = crate::live_host::Host::new(camera.eye);
                host.auction_renderer = Some(sender);
                host.activate_saved_auction(&mut session).unwrap();
                responder.join().unwrap();
                assert_eq!(session.world.city.auction.controller.record[0x7c..0x7e], [1, 0]);
            }
        }
        }).unwrap().join().unwrap();
}
#[cfg(feature = "capture")]
#[test]
#[ignore = "requires original supplied save, original assets and GPU"]
fn capture_saved_appearance_against_plain_city() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
            let language = installer.join("US/language/usa");
            let executable = read("OPENMT2001_EXE");
            let inputs = SceneInputs {
                process_runtime: None,
                compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
                character_names: crate::fresh_city::character_names(
                    &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                    &language,
                )
                .unwrap(),
                installer,
                language,
                maps: openmt2001::fixtures::path("MAP")
                    .parent()
                    .unwrap()
                    .to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"),
                executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            };
            let camera = CameraSample {
                eye: [0., 180., 220.],
                target: [0.; 3],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            };
            let mut cache = SnapshotCache::default();
            let (loaded, session) = inputs
                .saved_preview(
                    std::path::Path::new(&openmt2001::fixtures::path("SAVE_ONE")),
                    camera,
                    &mut cache,
                )
                .unwrap();
            let plain = session
                .scene(
                    &inputs.build_file,
                    OriginalAssets {
                        executable: &inputs.executable,
                        directory: &inputs.directory,
                        archive: &inputs.archive,
                    },
                    &inputs.suncols,
                    Some(loaded.saved_viewpoint.unwrap().0),
                    true,
                )
                .unwrap();
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            crate::capture_test::capture(&renderer, None, &plain, "save/plain-saved-city.png");
            crate::capture_test::capture(
                &renderer,
                None,
                cache.city.as_ref().unwrap(),
                "save/appearance-saved-city.png",
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
#[cfg(feature = "capture")]
#[test]
#[ignore = "requires original Scenario1 assets and an offscreen GPU"]
fn capture_static_details_against_fresh_city() {
    std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
            let language = installer.join("US/language/usa");
            let executable = read("OPENMT2001_EXE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
                character_names: crate::fresh_city::character_names(&image, &language).unwrap(),
                installer, language,
                maps: openmt2001::fixtures::path("MAP")
                    .parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"), executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"), suncols: read("OPENMT2001_SUNCOLS"),
            };
            let catalog = crate::campaign_store::load_catalog(
                &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                inputs.maps.parent().unwrap(),
            ).unwrap();
            let setup = crate::singleplayer::Setup {
                preview: crate::singleplayer::Preview::Scenario(200001),
                difficulty: crate::singleplayer::Difficulty::Easy,
                character_index: 4, profile_name: b"Static detail capture".to_vec(), catalog,
                profile_path: openmt2001::fixtures::scratch("FRONTEND").join("static-capture-profile.lua"),
            };
            let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
                up: [0., 1., 0.], vertical_fov: 45. };
            let mut cache = SnapshotCache::default();
            let (_loaded, _session) = inputs.fresh_preview(setup, camera, &mut cache).unwrap();
            let mut after = cache.city.as_ref().unwrap().clone();
            let first = cache.static_detail_ranges.first().unwrap();
            let vertex = after.draws[first.clone()].iter()
                .flat_map(|draw| &draw.vertices).next().unwrap();
            let focus = [vertex[0], vertex[1], vertex[2]];
            let close = CameraSample { eye: [focus[0] + 28., focus[1] + 20., focus[2] + 30.],
                target: focus, ..camera };
            let mut before = after.clone();
            let mut removed_vertices = 0;
            for range in cache.static_detail_ranges.iter().rev() {
                removed_vertices += before.draws[range.clone()].iter()
                    .map(|draw| draw.vertices.len()).sum::<usize>();
                before.draws.drain(range.clone());
            }
            assert!(removed_vertices > 0, "original Scenario1 must submit static meshes");
            before.vertex_count -= removed_vertices;
            before.reframe(close).unwrap();
            after.reframe(close).unwrap();
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let before_path = "frontend/static-city-before.png";
            let after_path = "frontend/static-city-after.png";
            crate::capture_test::capture(&renderer, None, &before, before_path);
            crate::capture_test::capture(&renderer, None, &after, after_path);
            let pixels = |path| {
                let mut reader = png::Decoder::new(std::io::BufReader::new(
                    fs::File::open(crate::capture_test::output_path(path)).unwrap())).read_info().unwrap();
                let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
                let frame = reader.next_frame(&mut rgba).unwrap();
                rgba.truncate(frame.buffer_size());
                rgba
            };
            let before_pixels = pixels(before_path);
            let after_pixels = pixels(after_path);
            let changed = before_pixels.chunks_exact(4).zip(after_pixels.chunks_exact(4))
                .filter(|(before, after)| before[..3] != after[..3]).count();
            assert!(changed > 16, "static meshes changed only {changed} GPU pixels");
            eprintln!("Static capture: {removed_vertices} submitted vertices, {changed} changed pixels; focus {focus:?}");
        }).unwrap().join().unwrap();
}
#[test]
#[ignore = "requires supplied Custom originals; fresh/save/selected-profile cold ownership"]
fn custom_fresh_save_and_cold_selected_profile_ownership() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            struct Scratch(PathBuf);
            impl Drop for Scratch {
                fn drop(&mut self) {
                    fs::remove_dir_all(&self.0).unwrap();
                }
            }
            let root = Scratch(std::env::temp_dir().join(format!("custom-profile-{}-{}",
                std::process::id(), std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
            fs::create_dir_all(&root.0).unwrap();
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let map = openmt2001::fixtures::path("MAP");
            let parameter = openmt2001::fixtures::path("PARAMETERS");
            let exe_path = openmt2001::fixtures::path("EXE");
            let executable = read("OPENMT2001_EXE");
            let language = exe_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa");
            let inputs = SceneInputs {
                process_runtime: None,
                compatibility: crate::compatibility::Mode::Modern,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: root.0.clone(),
                character_names: crate::fresh_city::character_names(
                    &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                    &language,
                )
                .unwrap(),
                installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                language,
                maps: map.parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"),
                executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            };
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let catalog =
                crate::campaign_store::load_catalog(&image, inputs.maps.parent().unwrap()).unwrap();
            let alice = crate::campaign_store::profile_path(&root.0, b"Alice");
            let bob = crate::campaign_store::profile_path(&root.0, b"Bob");
            // Distinct persistence fixtures; these are not gameplay victories.
            for (path, attempts) in [(&alice, 2), (&bob, 7)] {
                let mut profile = catalog.clone();
                profile.reset_progress();
                profile.set_level_info(500000, attempts, [0; 3]);
                crate::campaign_store::save_progress(&profile, path).unwrap();
            }
            let before = [fs::read(&alice).unwrap(), fs::read(&bob).unwrap()];
            let mut selected = catalog.clone();
            crate::campaign_store::load_progress(&mut selected, &alice).unwrap();
            let setup = crate::singleplayer::Setup {
                preview: crate::singleplayer::Preview::Scenario(500000),
                difficulty: crate::singleplayer::Difficulty::Medium,
                character_index: 4,
                profile_name: b"Alice".to_vec(),
                catalog: selected,
                profile_path: alice.clone(),
            };
            let camera = CameraSample {
                eye: [0., 180., 220.],
                target: [0.; 3],
                up: [0., 1., 0.],
                vertical_fov: 0.7,
            };
            let (inspection, mut session) = inputs
                .fresh_preview(setup, camera, &mut SnapshotCache::default())
                .unwrap();
            assert_eq!(session.campaign_profile, Some(alice.clone()));
            assert_eq!(session.catalog.selected_level().unwrap().attempts(), 2);
            let bytes = session
                .save_v2(&image, crate::save_menu::cameras(None, camera).unwrap(), 1)
                .unwrap();
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let extension = openmt2001::save::extension::Extension::parse(&save)
                .unwrap()
                .unwrap();
            assert!(extension.version >= 4);
            let path = root.0.join("custom.sav");
            fs::write(&path, &bytes).unwrap();
            drop(session);
            drop(inspection);
            let (inspection, restored) = inputs
                .saved_preview(&path, camera, &mut SnapshotCache::default())
                .unwrap();
            assert_eq!(
                restored.campaign_profile,
                Some(alice.clone()),
                "the explicit runtime extension must retain the fresh profile identity"
            );
            drop(restored);
            drop(inspection);
            for (name, expected, attempts) in [
                (Some(b"Alice".as_slice()), Some(alice.clone()), 2),
                (Some(b"Bob".as_slice()), Some(bob.clone()), 7),
                (None, None, 0),
            ] {
                let selected_root =
                    name.map(|name| crate::save_menu::selected_profile_root(&root.0, name));
                let (inspection, restored) = inputs
                    .saved_preview_with_selection(
                        &path,
                        camera,
                        &mut SnapshotCache::default(),
                        Some(selected_root.as_deref()),
                    )
                    .unwrap();
                assert_eq!(restored.campaign_profile, expected);
                assert_eq!(restored.catalog.selected_level().unwrap().id(), 500000);
                assert_eq!(
                    restored.catalog.selected_level().unwrap().attempts(),
                    attempts
                );
                assert_eq!(restored.difficulty, 1);
                drop(restored);
                drop(inspection);
            }
            assert_eq!(
                [fs::read(&alice).unwrap(), fs::read(&bob).unwrap()],
                before,
                "startup/save/reload must not count a completed result"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
#[ignore = "requires original fresh scenario and graphics assets"]
fn fresh_preview_worker_retains_session_until_inspection_closes() {
    // The retained city records exceed the default debug test-thread stack.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(fresh_preview_worker_retains_session_until_inspection_closes_body)
        .unwrap()
        .join()
        .unwrap();
}

fn fresh_preview_worker_retains_session_until_inspection_closes_body() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let map = openmt2001::fixtures::path("MAP");
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let executable = read("OPENMT2001_EXE");
    let exe_path = openmt2001::fixtures::path("EXE");
    let inputs = SceneInputs {
        process_runtime: None,
        compatibility: crate::compatibility::Mode::StrictV14,
        style: test_board_style(&executable),
        time_runtime: Default::default(),
        profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
        character_names: crate::fresh_city::character_names(
            &openmt2001::pe::PeImage::parse(&executable).unwrap(),
            &exe_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa"),
        )
        .unwrap(),
        installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
        language: exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa"),
        maps: map.parent().unwrap().to_path_buf(),
        build_file: read("OPENMT2001_BUILD_FILE"),
        executable,
        directory: read("OPENMT2001_ARCHIVE_DIR"),
        archive: read("OPENMT2001_ARCHIVE_BIN"),
        suncols: read("OPENMT2001_SUNCOLS"),
    };
    let inputs = Arc::new(inputs);
    let scripts = inputs.maps.parent().unwrap();
    let catalog = crate::campaign_store::load_catalog(
        &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
        scripts,
    )
    .unwrap();
    let setup = crate::singleplayer::Setup {
        preview: crate::singleplayer::Preview::Scenario(200003),
        difficulty: crate::singleplayer::Difficulty::Easy,
        character_index: 2,
        profile_name: b"Preview test".to_vec(),
        catalog,
        profile_path: crate::campaign_store::profile_path(&inputs.profile_root, b"Preview test"),
    };
    let mut controls = openmt2001::camera::BlockCamera {
        center: [0.; 3],
        angle: -std::f32::consts::FRAC_PI_2,
        height: 180.,
        radius: 220.,
    };
    let (eye, target) = controls.tick([false; 4]);
    let camera = CameraSample {
        eye,
        target,
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let (tx, rx) = mpsc::channel();
    let worker_inputs = inputs.clone();
    let worker = std::thread::Builder::new()
        .name("city-simulation-test".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || worker_inputs.preview_worker(setup, camera, tx))
        .unwrap();
    let mut loaded = rx.recv().unwrap().unwrap();
    assert!(loaded.fresh_session.is_some());
    assert_eq!(loaded.music_era, Some(0));
    assert!(
        loaded.vehicles.is_some(),
        "retained city must bind road model materials"
    );
    assert!(
        loaded.transport.is_some(),
        "retained city must bind transport materials"
    );
    assert!(loaded.status.is_none());
    assert_eq!(loaded.scene.building_count, 9);
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_SIDEWALK_CAPTURE") {
        crate::block_sidewalk_labels::capture_marvin(
            &loaded.scene,
            loaded.block_information.as_ref().unwrap(),
            &path,
        );
    }
    let sender = loaded.fresh_session.as_ref().unwrap();
    let (reply, initial_status) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let initial_status = initial_status.recv().unwrap();
    assert!(
        initial_status.running,
        "native worker startup enables gameplay"
    );
    assert!(
        initial_status.failure.is_none(),
        "{:?}",
        initial_status.failure
    );
    // Exercise the same asynchronous Manager requests as the window.
    // Two opens must add one modal reason, and GUI4 freezes Time while
    // the registered interface timer continues to dispatch.
    sender
        .send(FreshRequest::Stats(openmt2001::stats_panel::Action::Open))
        .unwrap();
    sender
        .send(FreshRequest::Stats(openmt2001::stats_panel::Action::Open))
        .unwrap();
    let (reply, opened) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let opened = opened.recv().unwrap();
    assert!(opened.running && opened.script_paused);
    let snapshot = opened.stats.as_ref().unwrap();
    assert_eq!(snapshot.players[0].name, b"Preview test");
    assert_eq!(snapshot.players[0].character, 2);
    std::thread::sleep(std::time::Duration::from_millis(120));
    let (reply, held) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let held = held.recv().unwrap();
    assert_eq!(
        held.ticks, opened.ticks,
        "Stats modal suspends the actual logic timer group"
    );
    assert!(held.interface_ticks > opened.interface_ticks);
    assert_eq!(
        held.time_display, opened.time_display,
        "GUI4 suppresses native Time callbacks"
    );
    sender
        .send(FreshRequest::Stats(openmt2001::stats_panel::Action::Close))
        .unwrap();
    let (reply, closed) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let closed = closed.recv().unwrap();
    assert!(
        closed.running && !closed.script_paused && closed.stats.is_none(),
        "closing a duplicate-opened graph releases exactly one reason"
    );
    sender.send(FreshRequest::Run(false)).unwrap();
    sender
        .send(FreshRequest::Stats(openmt2001::stats_panel::Action::Open))
        .unwrap();
    let (reply, paused_graph) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let paused_graph = paused_graph.recv().unwrap();
    assert!(
        !paused_graph.running && paused_graph.script_paused && paused_graph.stats.is_some(),
        "original session-active flag remains set during a player's pause"
    );
    sender
        .send(FreshRequest::Stats(openmt2001::stats_panel::Action::Close))
        .unwrap();
    let (reply, paused_status) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let paused_tick = paused_status.recv().unwrap().ticks;
    let (reply, saved_result) = mpsc::channel();
    let save_name = format!(
        "worker-{:012x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
            & 0xffff_ffff_ffff
    );
    sender
        .send(FreshRequest::SaveSelected {
            target: crate::save_menu::Target::NewName(save_name.clone()),
            capture: None,
            camera,
            game_mode: 1,
            interface_mode: 0,
            reply,
        })
        .unwrap();
    let saved_path = saved_result.recv().unwrap().unwrap();
    assert_eq!(saved_path.file_stem().unwrap().to_string_lossy(), save_name);
    let scoped_root =
        crate::save_menu::selected_profile_root(&inputs.profile_root, b"Preview test");
    assert!(saved_path.starts_with(fs::canonicalize(scoped_root.join("saves")).unwrap()));
    let saved_bytes = fs::read(&saved_path).unwrap();
    let saved_file = openmt2001::save::SaveFile::parse(&saved_bytes).unwrap();
    let saved_players = saved_file.players().unwrap();
    assert_eq!(saved_players.len(), 1);
    let mut expected_metadata = [0u8; 48];
    expected_metadata[..4].copy_from_slice(&2_i32.to_le_bytes());
    expected_metadata[8..20].copy_from_slice(b"Preview test");
    assert_eq!(
        *saved_players[0].metadata, expected_metadata,
        "fresh export retains actual frontend character, kind, name and difficulty"
    );
    let reload_inputs = inputs.clone();
    let reload_path = saved_path.clone();
    let restored = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || {
            let (inspection, session) = reload_inputs
                .saved_preview(&reload_path, camera, &mut SnapshotCache::default())
                .unwrap();
            assert_eq!(
                session.campaign_profile,
                Some(crate::campaign_store::profile_path(
                    &reload_inputs.profile_root,
                    b"Preview test"
                ))
            );
            assert_eq!(session.world.clock.timing[0], paused_tick);
            assert_eq!(session.saved_player_metadata[0], expected_metadata);
            inspection
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(restored.scene.building_count, loaded.scene.building_count);
    assert_eq!(restored.saved_viewpoint.unwrap().0.eye, camera.eye);
    let (delete_reply, delete_result) = mpsc::channel();
    sender
        .send(FreshRequest::DeleteSave {
            id: saved_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            reply: delete_reply,
        })
        .unwrap();
    delete_result.recv().unwrap().unwrap();
    assert!(
        !saved_path.exists(),
        "confirmed Delete must remove only the selected V2 slot"
    );
    for expected in [4, 1] {
        sender.send(FreshRequest::ToggleSpeed).unwrap();
        let (reply, result) = mpsc::channel();
        sender
            .send(FreshRequest::Status {
                camera: camera.eye,
                reply,
            })
            .unwrap();
        let status = result.recv().unwrap();
        assert_eq!(status.speed, expected);
        assert!(!status.running, "speed control does not unpause the city");
    }
    crate::prompt::verify_card_panel(
        &loaded.scene,
        &loaded.player_info.as_ref().unwrap().records[0],
    );
    crate::auction_notice::verify_panel(
        &loaded.scene,
        &loaded.player_info.as_ref().unwrap().records[0],
    );
    crate::business_adjustment::verify_live_panels(&loaded.scene);
    assert!(!worker.is_finished());
    let (reply, answer) = mpsc::channel();
    loaded
        .fresh_session
        .as_ref()
        .unwrap()
        .send(FreshRequest::Planning {
            category: 33,
            reply,
        })
        .unwrap();
    let planning = answer.recv().unwrap().unwrap();
    assert_eq!(planning.category, 33);
    assert_eq!(planning.current_owner, 1);
    assert_eq!(planning.era, 0);
    assert_eq!(planning.players.len(), 1);
    assert_eq!(planning.catalog.rows().len(), 45);
    assert_eq!(
        planning.category_record[5],
        loaded.block_information.as_ref().unwrap().owners[33]
    );
    assert_eq!(planning.entry(), crate::build_panel::Entry::Open);
    let initial_floor = u32::from_le_bytes(
        planning.catalog.row(0).unwrap()[0x48..0x4c]
            .try_into()
            .unwrap(),
    ) as i32;
    let preview_worker = loaded.fresh_session.as_ref().unwrap().clone();
    crate::build_view::verify_live_context(
        planning,
        &mut loaded.scene,
        |form| {
            let (reply, answer) = mpsc::channel();
            preview_worker
                .send(FreshRequest::Build {
                    form,
                    submit: false,
                    reply,
                })
                .unwrap();
            answer.recv().unwrap().unwrap().unwrap()
        },
        || {
            let (reply, answer) = mpsc::channel();
            preview_worker
                .send(FreshRequest::BuildDrag {
                    category: 33,
                    reply,
                })
                .unwrap();
            answer.recv().unwrap().unwrap().unwrap()
        },
    );
    drop(preview_worker);
    let mut form = crate::build_panel::Request {
        category: 33,
        origin: [0, 0],
        size: [2, 2],
        height: initial_floor,
        packed_model: 0,
        kind: 0,
        quality: 0,
        displayed_quote: f32::NAN,
        business: 0,
    };
    let mut accepted = None;
    let mut construction_camera = None;
    for origin in [[0, 0], [0, 5], [5, 0], [5, 5]] {
        form.origin = origin;
        let (reply, answer) = mpsc::channel();
        loaded
            .fresh_session
            .as_ref()
            .unwrap()
            .send(FreshRequest::Build {
                form: form.clone(),
                submit: false,
                reply,
            })
            .unwrap();
        let quote = answer.recv().unwrap().unwrap().unwrap();
        assert!(!quote.submitted);
        assert!(quote.total.is_finite());
        if quote.allowed() {
            let center = quote.preview.as_ref().unwrap().center;
            construction_camera = Some(CameraSample {
                eye: [center[0], 120., center[1] + 100.],
                target: [center[0], 0., center[1]],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            });
            accepted = Some(quote.total);
            break;
        }
    }
    let total = accepted.expect("original fresh city has a valid construction preview");
    form.displayed_quote = total;
    let construction_camera = construction_camera.unwrap();
    let (reply, answer) = mpsc::channel();
    loaded
        .fresh_session
        .as_ref()
        .unwrap()
        .send(FreshRequest::Build {
            form,
            submit: true,
            reply,
        })
        .unwrap();
    let submitted = answer.recv().unwrap().unwrap().unwrap();
    assert!(submitted.submitted);
    assert_eq!(
        submitted.total, total,
        "confirmation carries the displayed quote"
    );
    let (reply, answer) = mpsc::channel();
    loaded
        .fresh_session
        .as_ref()
        .unwrap()
        .send(FreshRequest::Inspect { camera, reply })
        .unwrap();
    let refreshed = answer.recv().unwrap().unwrap();
    assert!(
        refreshed.reuse_city,
        "paused snapshot reuses exact city geometry"
    );
    assert_eq!(
        refreshed.scene.building_count, loaded.scene.building_count,
        "paused worker must not secretly execute queued construction"
    );
    assert_eq!(
        refreshed.detail_visibility.spans.len(),
        loaded.detail_visibility.spans.len(),
        "snapshot reuse must retain registry-detail ownership"
    );
    drop(refreshed);
    assert!(
        !loaded.detail_visibility.spans.is_empty(),
        "original fresh city should contain registry details"
    );
    let first_detail = loaded
        .detail_visibility
        .spans
        .iter()
        .find(|span| {
            span.positions
                .iter()
                .any(|position| position[1] != span.pivot_y)
        })
        .expect("original registry details have no vertical geometry")
        .clone();
    let detail_camera = CameraSample {
        eye: [first_detail.center[0], 80., first_detail.center[1] + 300.],
        target: [first_detail.center[0], 0., first_detail.center[1]],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let depth = openmt2001::camera::translated_scene_view(
        openmt2001::camera::scene_view_with_up(
            detail_camera.eye,
            detail_camera.target,
            detail_camera.up,
        ),
        [first_detail.center[0], 0., first_detail.center[1]],
    )[11];
    assert!(
        depth > 200. && depth <= 500.,
        "fixture camera should cross the native detail0/100 far bounds: {depth}"
    );
    let base_vertices = loaded.scene.draws[0].vertices.clone();
    let started = Instant::now();
    loaded
        .detail_visibility
        .reframe(&mut loaded.scene, detail_camera, 0)
        .unwrap();
    let hidden = loaded.scene.draws[first_detail.draw].vertices
        [first_detail.start..first_detail.start + first_detail.positions.len()]
        .iter()
        .map(|v| [v[0], v[1], v[2]])
        .collect::<Vec<_>>();
    assert!(
        hidden.iter().all(|position| *position == hidden[0]),
        "native pass3 far gate should hide category registry details"
    );
    let band_camera = CameraSample {
        eye: [first_detail.center[0], 80., first_detail.center[1] + 235.],
        target: [first_detail.center[0], 0., first_detail.center[1]],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let band_depth = openmt2001::camera::translated_scene_view(
        openmt2001::camera::scene_view_with_up(band_camera.eye, band_camera.target, band_camera.up),
        [first_detail.center[0], 0., first_detail.center[1]],
    )[11];
    assert!(
        band_depth > 220. && band_depth < 270.,
        "fixture camera should enter original detail fade band: {band_depth}"
    );
    loaded
        .detail_visibility
        .reframe(&mut loaded.scene, band_camera, 35)
        .unwrap();
    let faded = loaded.scene.draws[first_detail.draw].vertices
        [first_detail.start..first_detail.start + first_detail.positions.len()]
        .iter()
        .map(|v| [v[0], v[1], v[2]])
        .collect::<Vec<_>>();
    assert!(
        faded
            .iter()
            .zip(&first_detail.positions)
            .any(|(now, original)| now[1] != original[1] && now[1] != first_detail.pivot_y),
        "original registry detail never entered intermediate vertical scale"
    );
    assert!(
        faded
            .iter()
            .zip(&first_detail.positions)
            .all(|(now, original)| now[0] == original[0] && now[2] == original[2]),
        "native detail band scales Y only"
    );
    loaded
        .detail_visibility
        .reframe(&mut loaded.scene, detail_camera, 100)
        .unwrap();
    let restored = loaded.scene.draws[first_detail.draw].vertices
        [first_detail.start..first_detail.start + first_detail.positions.len()]
        .iter()
        .map(|v| [v[0], v[1], v[2]])
        .collect::<Vec<_>>();
    assert_eq!(
        restored, first_detail.positions,
        "raising distance restores the original detail triangles"
    );
    assert_eq!(
        loaded.scene.draws[0].vertices, base_vertices,
        "the mode1 city base must remain untouched"
    );
    eprintln!(
        "[detail distance] spans={} three reframes={:?}",
        loaded.detail_visibility.spans.len(),
        started.elapsed()
    );
    assert!(loaded.people.is_some());
    let focus = loaded.people.as_ref().unwrap().first_position();
    let near_camera = CameraSample {
        eye: [focus[0], focus[1] + 4., focus[2] - 12.],
        target: focus,
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };

    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_FRESH_HOST_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(target_os = "windows") {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut prepared = renderer
            .prepare(
                &loaded
                    .scene
                    .draws
                    .iter()
                    .map(|d| d.borrowed())
                    .collect::<Vec<_>>(),
                loaded.scene.camera,
                loaded.scene.fog,
            )
            .unwrap();
        for (name, sample) in [
            ("near", near_camera),
            (
                "lod",
                CameraSample {
                    eye: [focus[0], focus[1] + 15., focus[2] + 120.],
                    ..near_camera
                },
            ),
            (
                "empty",
                CameraSample {
                    eye: [focus[0], 3500., focus[2] + 10.],
                    ..near_camera
                },
            ),
            ("return", near_camera),
        ] {
            let mut changed = loaded.scene.reframe(sample).unwrap();
            let residents = loaded
                .people
                .as_ref()
                .unwrap()
                .reframe(&mut loaded.scene, sample, 35)
                .unwrap();
            let count: usize = loaded.scene.draws[residents.clone()]
                .iter()
                .map(|d| d.vertices.len())
                .sum();
            if name == "empty" {
                assert_eq!(count, 0);
            } else {
                assert!(count > 0);
            }
            let dynamic = residents.clone();
            changed.extend(residents);
            for (i, draw) in loaded.scene.draws.iter().enumerate() {
                if dynamic.contains(&i) {
                    renderer
                        .update_dynamic_surface(
                            &mut prepared,
                            i,
                            &draw.borrowed(),
                            loaded.scene.camera,
                            loaded.scene.fog,
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "reframe {name} dynamic surface {i} {:?} vertices {}: {error}",
                                draw.mode,
                                draw.vertices.len()
                            )
                        });
                } else if changed.contains(&i) {
                    renderer
                        .update_dynamic_surface(
                            &mut prepared,
                            i,
                            &draw.borrowed(),
                            loaded.scene.camera,
                            loaded.scene.fog,
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "reframe {name} retained surface {i} {:?} vertices {}: {error}",
                                draw.mode,
                                draw.vertices.len()
                            )
                        });
                } else {
                    renderer
                        .update_camera(
                            &prepared,
                            i,
                            draw.view_projection.unwrap_or(loaded.scene.camera),
                            loaded.scene.fog,
                        )
                        .unwrap();
                }
            }
            crate::capture_test::capture_prepared(
                &renderer,
                &loaded.scene,
                &prepared,
                &format!("{path}-{name}.png"),
            );
        }
        // 452470/4524d0 map the Graphics Draw Distance percentage to
        // moving-object LOD and far visibility. Hold the camera fixed so
        // these captures isolate that original control's visible effect.
        let distance_camera = CameraSample {
            eye: [focus[0], focus[1] + 15., focus[2] + 120.],
            ..near_camera
        };
        let mut distance_counts = Vec::new();
        for detail in [0, 35, 100] {
            loaded.scene.reframe(distance_camera).unwrap();
            loaded
                .detail_visibility
                .reframe(&mut loaded.scene, distance_camera, detail as u8)
                .unwrap();
            let residents = loaded
                .people
                .as_ref()
                .unwrap()
                .reframe(&mut loaded.scene, distance_camera, detail)
                .unwrap();
            distance_counts.push(
                loaded.scene.draws[residents]
                    .iter()
                    .map(|draw| draw.vertices.len())
                    .sum::<usize>(),
            );
            let draws = loaded
                .scene
                .draws
                .iter()
                .map(|draw| draw.borrowed())
                .collect::<Vec<_>>();
            let distance_prepared = renderer
                .prepare(&draws, loaded.scene.camera, loaded.scene.fog)
                .unwrap();
            crate::capture_test::capture_prepared(
                &renderer,
                &loaded.scene,
                &distance_prepared,
                &format!("{path}-distance-{detail}.png"),
            );
        }
        assert!(
            distance_counts[0] < distance_counts[2],
            "original draw-distance range should reveal residents at a fixed camera"
        );
        // The same native control gates category registry details in pass3.
        // This camera holds the first original detail category in frame.
        for detail in [0, 35, 100] {
            loaded.scene.reframe(detail_camera).unwrap();
            loaded
                .detail_visibility
                .reframe(&mut loaded.scene, detail_camera, detail as u8)
                .unwrap();
            loaded
                .people
                .as_ref()
                .unwrap()
                .reframe(&mut loaded.scene, detail_camera, detail)
                .unwrap();
            let draws = loaded
                .scene
                .draws
                .iter()
                .map(|draw| draw.borrowed())
                .collect::<Vec<_>>();
            let captured = renderer
                .prepare(&draws, loaded.scene.camera, loaded.scene.fog)
                .unwrap();
            crate::capture_test::capture_prepared(
                &renderer,
                &loaded.scene,
                &captured,
                &format!("{path}-category-detail-{detail}.png"),
            );
        }
    }
    let residents = loaded
        .people
        .as_ref()
        .unwrap()
        .reframe(&mut loaded.scene, near_camera, 35)
        .unwrap();
    assert!(
        loaded.scene.draws[residents]
            .iter()
            .any(|d| !d.vertices.is_empty())
    );
    let sender = loaded.fresh_session.as_ref().unwrap();
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::Status {
            camera: camera.eye,
            reply,
        })
        .unwrap();
    let paused = answer.recv().unwrap();
    assert_eq!(
        paused.ticks, paused_tick,
        "inspection and build submission do not advance a paused session"
    );
    if std::env::var_os("OPENMT2001_PROFILE_PACE").is_some() {
        for sample in 0..3 {
            let started = Instant::now();
            let (reply, answer) = mpsc::channel();
            sender.send(FreshRequest::Residents { reply }).unwrap();
            answer.recv().unwrap().unwrap();
            let residents = started.elapsed();
            let started = Instant::now();
            let (reply, answer) = mpsc::channel();
            sender.send(FreshRequest::Vehicles { reply }).unwrap();
            answer.recv().unwrap().unwrap();
            let vehicles = started.elapsed();
            let started = Instant::now();
            let (reply, answer) = mpsc::channel();
            sender.send(FreshRequest::Transport { reply }).unwrap();
            answer.recv().unwrap().unwrap();
            eprintln!(
                "PACE worker request sample={sample} residents={residents:?} vehicles={vehicles:?} transport={:?}",
                started.elapsed()
            );
        }
    }
    sender.send(FreshRequest::Run(true)).unwrap();
    let pace_started = Instant::now();
    let profile_pace = std::env::var_os("OPENMT2001_PROFILE_PACE").is_some();
    let mut pace_bucket = 0;
    let mut paused_total = std::time::Duration::ZERO;
    let mut status_samples = 0u32;
    let mut status_total = std::time::Duration::ZERO;
    let mut status_max = std::time::Duration::ZERO;
    let deadline = Instant::now() + std::time::Duration::from_secs(45);
    let mut foundation_seen = false;
    let mut partial_seen = false;
    let mut partial_tick = None;
    let mut patched_seen = false;
    loop {
        let status_started = Instant::now();
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Status {
                camera: camera.eye,
                reply,
            })
            .unwrap();
        let status = answer.recv().unwrap();
        if profile_pace {
            let latency = status_started.elapsed();
            status_samples += 1;
            status_total += latency;
            status_max = status_max.max(latency);
        }
        if profile_pace && status.ticks / 25 > pace_bucket {
            pace_bucket = status.ticks / 25;
            eprintln!(
                "PACE 1x logic={} interface={} active={:.3}s wall={:.3}s status_mean={:.2}ms status_max={:.2}ms",
                status.ticks,
                status.interface_ticks,
                (pace_started.elapsed() - paused_total).as_secs_f64(),
                pace_started.elapsed().as_secs_f64(),
                status_total.as_secs_f64() * 1000. / f64::from(status_samples),
                status_max.as_secs_f64() * 1000.
            );
        }
        assert!(
            status.failure.is_none(),
            "real worker tick {} stopped: {:?}",
            status.ticks,
            status.failure
        );
        if !foundation_seen && (45..75).contains(&status.ticks) {
            let pause_started = Instant::now();
            sender.send(FreshRequest::Run(false)).unwrap();
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::Inspect {
                    camera: construction_camera,
                    reply,
                })
                .unwrap();
            let foundation = answer.recv().unwrap().unwrap();
            assert!(foundation.construction_active);
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_CONSTRUCTION_LIVE_CAPTURE") {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                crate::capture_test::capture(
                    &renderer,
                    None,
                    &foundation.scene,
                    &format!("{prefix}-foundation-{}.png", status.ticks),
                );
            }
            foundation_seen = true;
            sender.send(FreshRequest::Run(true)).unwrap();
            paused_total += pause_started.elapsed();
        }
        if !partial_seen && (125..175).contains(&status.ticks) {
            let pause_started = Instant::now();
            // Hold the original 25 Hz worker while the debug GPU capture
            // runs, so a later scripted prompt is still observed below.
            sender.send(FreshRequest::Run(false)).unwrap();
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::Inspect {
                    camera: construction_camera,
                    reply,
                })
                .unwrap();
            let partial = answer.recv().unwrap().unwrap();
            assert!(partial.construction_active);
            assert!(
                !partial.reuse_city,
                "growing pose invalidates the cached city"
            );
            assert_eq!(
                partial.scene.building_count,
                loaded.scene.building_count + 1,
                "live snapshot must include the actively growing building"
            );
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_CONSTRUCTION_LIVE_CAPTURE") {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                crate::capture_test::capture(
                    &renderer,
                    None,
                    &partial.scene,
                    &format!("{prefix}-partial-{}.png", status.ticks),
                );
            }
            partial_seen = true;
            partial_tick = Some(status.ticks);
            sender.send(FreshRequest::Run(true)).unwrap();
            paused_total += pause_started.elapsed();
        }
        if partial_tick.is_some_and(|tick| status.ticks >= tick + 10)
            && !patched_seen
            && status.ticks < 225
        {
            let pause_started = Instant::now();
            sender.send(FreshRequest::Run(false)).unwrap();
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::Inspect {
                    camera: construction_camera,
                    reply,
                })
                .unwrap();
            let growing = answer.recv().unwrap().unwrap();
            assert!(growing.construction_active);
            assert!(
                growing.reuse_city,
                "the second active-growth snapshot must reuse the city layout"
            );
            assert!(
                !growing.updated_city_draws.is_empty(),
                "active-growth scale must patch existing material draws"
            );
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_CONSTRUCTION_LIVE_CAPTURE") {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                crate::capture_test::capture(
                    &renderer,
                    None,
                    &growing.scene,
                    &format!("{prefix}-growing-{}.png", status.ticks),
                );
            }
            patched_seen = true;
            sender.send(FreshRequest::Run(true)).unwrap();
            paused_total += pause_started.elapsed();
        }
        if status.ticks >= 275 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "live worker did not reach construction completion: {status:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    assert!(
        foundation_seen,
        "worker skipped the scaffold foundation interval"
    );
    assert!(
        partial_seen,
        "worker skipped the visible construction interval"
    );
    assert!(
        patched_seen,
        "worker skipped the retained growth-pose update"
    );
    sender.send(FreshRequest::Run(false)).unwrap();
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::Inspect { camera, reply })
        .unwrap();
    let built = answer.recv().unwrap().unwrap();
    assert!(!built.construction_active);
    assert!(!built.reuse_city, "construction invalidates city geometry");
    assert_eq!(
        built.scene.building_count,
        loaded.scene.building_count + 1,
        "live command creates a real scene building"
    );
    assert!(
        built
            .scene
            .inspection_buildings
            .iter()
            .any(|b| b.initialized_business.is_some()),
        "construction opens its authoritative business after its separate delayed open command"
    );
    crate::business_adjustment::verify_live_panels(&built.scene);
    let shop = built
        .scene
        .inspection_buildings
        .iter()
        .find(|b| b.initialized_business.is_some())
        .unwrap();
    let business = shop.initialized_business.as_ref().unwrap();
    let target = crate::business_edit::Target {
        building: shop.runtime_slot.unwrap(),
        business: business.slot,
    };
    let edit = |action| {
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::EditBusiness {
                target,
                action,
                reply,
            })
            .unwrap();
        answer.recv().unwrap().unwrap()
    };
    let old = edit(crate::business_edit::Action::Refresh);
    let price = edit(crate::business_edit::Action::Price {
        slot: 0,
        increase: true,
    });
    assert_eq!(
        price[0x1c4..0x1c8],
        old[0x1c4..0x1c8],
        "editing only changes pending price"
    );
    assert_ne!(price[0x1ec..0x1f0], old[0x1ec..0x1f0]);
    let stock = edit(crate::business_edit::Action::Stock {
        slot: 0,
        increase: false,
    });
    assert_eq!(
        stock[0x1b8..0x1ba],
        old[0x1b8..0x1ba],
        "editing only changes pending target"
    );
    let submitted = edit(crate::business_edit::Action::Apply);
    assert_eq!(submitted[0x208], 0);
    assert_eq!(
        submitted[0x1c4..0x1c8],
        old[0x1c4..0x1c8],
        "Apply preserves the real command delay"
    );
    sender.send(FreshRequest::Run(true)).unwrap();
    let deadline = Instant::now() + std::time::Duration::from_secs(15);
    let mut saw_prompt = false;
    loop {
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Status {
                camera: camera.eye,
                reply,
            })
            .unwrap();
        let status = answer.recv().unwrap();
        assert!(
            status.failure.is_none(),
            "post-build worker stopped: {status:?}"
        );
        if let Some(prompt) = status.prompts.first() {
            assert!(status.script_paused);
            std::thread::sleep(std::time::Duration::from_millis(80));
            let (reply, paused_answer) = mpsc::channel();
            sender
                .send(FreshRequest::Status {
                    camera: camera.eye,
                    reply,
                })
                .unwrap();
            let still_paused = paused_answer.recv().unwrap();
            assert_eq!(
                still_paused.ticks, status.ticks,
                "script pause suspends25Hz logic"
            );
            assert!(
                still_paused.interface_ticks > status.interface_ticks,
                "registered50Hz interface task continues during script pause"
            );
            assert!(still_paused.prompts.is_empty());
            assert_eq!(
                prompt.dismissal_id, 1,
                "original scenario3 callback identity"
            );
            let mut ui = crate::prompt::State::default();
            ui.submit(prompt.clone());
            // Script messages share the modal queue. Drive the physical Return
            // path used by the window, including the first event that arms it.
            let mut keys = dialog_keys::Input::default();
            keys.event1(Some(&mut ui));
            keys.physical(process_frontend::PhysicalSample::Key {
                key: winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Enter),
                down: true,
                repeat: false,
            });
            keys.event1(Some(&mut ui));
            let responses = ui.take_dialog_replies();
            assert_eq!(
                responses,
                vec![crate::human_dialog::Reply {
                    callback: 0,
                    context: 1,
                    button: 0,
                }]
            );
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::HumanDialog(responses[0], reply))
                .unwrap();
            assert!(
                ui.request().is_some(),
                "keep slot active during worker callback"
            );
            ui.finish_dismiss(answer.recv().unwrap());
            assert!(ui.request().is_none());
            saw_prompt = true;
        }
        if status.ticks >= 350 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "scripted prompt did not release live worker: {status:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    assert!(saw_prompt, "original9:30 scenario tip reached the host");
    sender.send(FreshRequest::Run(false)).unwrap();
    let applied = edit(crate::business_edit::Action::Refresh);
    assert_eq!(applied[0x1c4..0x1c8], price[0x1ec..0x1f0]);
    assert_eq!(applied[0x1b8..0x1ba], stock[0x1e0..0x1e2]);
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::Inspect {
            camera: near_camera,
            reply,
        })
        .unwrap();
    let final_snapshot = answer.recv().unwrap().unwrap();
    assert!(
        final_snapshot.reuse_city,
        "business terms preserve city geometry"
    );
    let final_shop = final_snapshot
        .scene
        .inspection_buildings
        .iter()
        .find(|b| b.runtime_slot == Some(target.building))
        .unwrap();
    assert_eq!(
        final_shop.initialized_business.as_ref().unwrap().record,
        applied,
        "cached geometry must expose the latest authoritative business record"
    );
    let (reply, answer) = mpsc::channel();
    sender.send(FreshRequest::Residents { reply }).unwrap();
    let mut moving = answer.recv().unwrap().unwrap();
    let (reply, answer) = mpsc::channel();
    sender.send(FreshRequest::Vehicles { reply }).unwrap();
    let mut traffic = answer.recv().unwrap().unwrap();
    assert!(
        traffic.active_count() > 0,
        "live original admission should activate road vehicle records"
    );
    traffic
        .replace_attachment(final_snapshot.vehicles.as_ref().unwrap())
        .unwrap();
    let focus = traffic.first_active_position().unwrap();
    let near_road = CameraSample {
        eye: [focus[0], focus[1] + 4., focus[2] - 12.],
        target: focus,
        ..camera
    };
    let mut road_scene = final_snapshot.scene.clone();
    road_scene.reframe(near_road).unwrap();
    // The worker's road tick carries the shared crossing masks that pick
    // the junction traffic-light glows (0x44dbe0). Every crossing row gets
    // the same green/amber words; amber suppresses the green and red points.
    let lanes = traffic.signal_lanes();
    assert!(!lanes.is_empty());
    assert!(lanes.iter().all(|row| row[..8] == lanes[0][..8]));
    let green = u32::from_le_bytes(lanes[0][..4].try_into().unwrap());
    let amber = u32::from_le_bytes(lanes[0][4..8].try_into().unwrap());
    assert!(matches!((green, amber), (5 | 10, 0) | (0, 0 | 5 | 10)));
    let mut changed_lanes = lanes.to_vec();
    for row in &mut changed_lanes {
        row[..4].copy_from_slice(&(15 - green.min(15)).to_le_bytes());
    }
    assert_eq!(
        road_scene.set_signal_lanes(&changed_lanes).unwrap().len(),
        1
    );
    assert_eq!(road_scene.set_signal_lanes(lanes).unwrap().len(), 1);
    assert!(road_scene.set_signal_lanes(lanes).unwrap().is_empty());
    let road_draws = traffic.reframe(&mut road_scene, near_road, 35).unwrap();
    assert!(
        road_scene.draws[road_draws]
            .iter()
            .any(|draw| !draw.vertices.is_empty()),
        "projected original road vehicles must submit visible mesh geometry"
    );
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_LIVE_VEHICLE_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let prepared = renderer
            .prepare(
                &road_scene
                    .draws
                    .iter()
                    .map(|draw| draw.borrowed())
                    .collect::<Vec<_>>(),
                road_scene.camera,
                road_scene.fog,
            )
            .unwrap();
        crate::capture_test::capture_prepared(&renderer, &road_scene, &prepared, &path);
    }
    // #490: the marina boats belong to the frontend backdrop city, so this
    // morning game submits none. Starting Out's noon ship checks visible
    // transport meshes near their pose.
    let marina_boats = [0x688db0, 0x688dc0, 0x688dd0, 0x688de0];
    assert!(
        final_snapshot
            .transport
            .as_ref()
            .unwrap()
            .submitted_references()
            .all(|reference| !marina_boats.contains(&reference)),
        "a started game must not submit the marina boats"
    );
    assert!(
        moving.differs_from(loaded.people.as_ref().unwrap()),
        "resident-only transport must contain advanced live poses/frames"
    );
    moving
        .replace_attachment(final_snapshot.people.as_ref().unwrap())
        .unwrap();
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_LIVE_RESIDENT_CAPTURE") {
        let scene = &mut final_snapshot.scene.clone();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        // Upload zero resident vertices, then prove an incoming population
        // snapshot can grow those buffers without replacing its textures.
        let empty = CameraSample {
            eye: [0., 3500., 10.],
            ..camera
        };
        moving.reframe(scene, empty, 35).unwrap();
        let mut prepared = renderer
            .prepare(
                &scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
                scene.camera,
                scene.fog,
            )
            .unwrap();
        let focus = moving.first_position();
        let near = CameraSample {
            eye: [focus[0], focus[1] + 4., focus[2] - 12.],
            target: focus,
            ..camera
        };
        let mut changed = scene.reframe(near).unwrap();
        let residents = moving.reframe(scene, near, 35).unwrap();
        assert!(
            scene.draws[residents.clone()]
                .iter()
                .any(|d| !d.vertices.is_empty())
        );
        changed.extend(residents.clone());
        for (i, draw) in scene.draws.iter().enumerate() {
            if changed.contains(&i) {
                renderer
                    .update_dynamic_surface(
                        &mut prepared,
                        i,
                        &draw.borrowed(),
                        scene.camera,
                        scene.fog,
                    )
                    .unwrap();
            } else {
                renderer
                    .update_camera(
                        &prepared,
                        i,
                        draw.view_projection.unwrap_or(scene.camera),
                        scene.fog,
                    )
                    .unwrap();
            }
        }
        crate::capture_test::capture_prepared(&renderer, scene, &prepared, &path);
    }
    let inspect_operations = |building| {
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::InspectOperations {
                building,
                bounds: [30., 29., 190., 300.],
                reply,
            })
            .unwrap();
        answer.recv().unwrap().unwrap()
    };
    let shop_controls = inspect_operations(target.building);
    assert_eq!(
        shop_controls.permissions.trade,
        Some(openmt2001::business_permissions::Trade::Sell)
    );
    assert!(shop_controls.permissions.convert && shop_controls.permissions.demolish);
    assert!(!shop_controls.report.summary.is_empty());
    #[cfg(feature = "capture")]
    crate::business_view::capture_inspection(&shop_controls);
    let apartment = final_snapshot
        .scene
        .inspection_buildings
        .iter()
        .find(|b| b.initialized_apartment.is_some())
        .unwrap()
        .runtime_slot
        .unwrap();
    let apartment_controls = inspect_operations(apartment);
    assert!(!apartment_controls.permissions.convert);
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::BusinessOperation {
            building: apartment,
            action: openmt2001::business_controls::Action::Convert {
                selected_label: shop_controls.business_label,
            },
            reply,
        })
        .unwrap();
    assert!(
        answer
            .recv()
            .unwrap()
            .unwrap_err()
            .contains("original panel rules")
    );
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::BusinessOperation {
            building: target.building,
            action: openmt2001::business_controls::Action::Trade,
            reply,
        })
        .unwrap();
    assert!(answer.recv().unwrap().unwrap());
    assert!(
        inspect_operations(target.building)
            .permissions
            .trade
            .is_none(),
        "pending sale disables repeated submission before dispatch"
    );
    sender.send(FreshRequest::Run(true)).unwrap();
    let deadline = Instant::now() + std::time::Duration::from_secs(6);
    loop {
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Status {
                camera: camera.eye,
                reply,
            })
            .unwrap();
        let status = answer.recv().unwrap();
        assert!(status.failure.is_none(), "worker sale failed: {status:?}");
        if status.ticks >= 380 {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    sender.send(FreshRequest::Run(false)).unwrap();
    assert_ne!(
        inspect_operations(target.building).permissions.trade,
        Some(openmt2001::business_permissions::Trade::Sell),
        "actual delayed sale removes ownership"
    );
    eprintln!(
        "Real worker building operations: native report, owned retail permissions, apartment conversion rejection, pending sale gate, delayed ownership transfer"
    );
    drop(final_snapshot);
    drop(built);
    drop(loaded);
    worker.join().unwrap();
}
#[test]
#[ignore = "requires original Starting Out graphics/script inputs"]
fn starting_out_early_controls_and_options_remain_live() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let map = openmt2001::fixtures::path("MAP");
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let executable = read("OPENMT2001_EXE");
    let exe_path = openmt2001::fixtures::path("EXE");
    let inputs = SceneInputs {
        process_runtime: None,
        compatibility: crate::compatibility::Mode::StrictV14,
        style: test_board_style(&executable),
        time_runtime: Default::default(),
        profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
        character_names: crate::fresh_city::character_names(
            &openmt2001::pe::PeImage::parse(&executable).unwrap(),
            &exe_path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("US/language/usa"),
        )
        .unwrap(),
        installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
        language: exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa"),
        maps: map.parent().unwrap().to_path_buf(),
        build_file: read("OPENMT2001_BUILD_FILE"),
        executable,
        directory: read("OPENMT2001_ARCHIVE_DIR"),
        archive: read("OPENMT2001_ARCHIVE_BIN"),
        suncols: read("OPENMT2001_SUNCOLS"),
    };
    let inputs = Arc::new(inputs);
    let scripts = inputs.maps.parent().unwrap();
    let catalog = crate::campaign_store::load_catalog(
        &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
        scripts,
    )
    .unwrap();
    let setup = crate::singleplayer::Setup {
        preview: crate::singleplayer::Preview::Scenario(200003),
        difficulty: crate::singleplayer::Difficulty::Easy,
        character_index: 2,
        profile_name: b"Justin".to_vec(),
        catalog,
        profile_path: crate::campaign_store::profile_path(&inputs.profile_root, b"Justin"),
    };
    let mut controls = openmt2001::camera::BlockCamera {
        center: [0.; 3],
        angle: -std::f32::consts::FRAC_PI_2,
        height: 180.,
        radius: 220.,
    };
    let (eye, target) = controls.tick([false; 4]);
    let camera = CameraSample {
        eye,
        target,
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    let (tx, rx) = mpsc::channel();
    let worker_inputs = inputs.clone();
    let worker = std::thread::Builder::new()
        .name("city-simulation-test".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || worker_inputs.preview_worker(setup, camera, tx))
        .unwrap();

    let loaded = rx.recv().unwrap().unwrap();
    let sender = loaded.fresh_session.as_ref().unwrap().clone();
    let status_raw = || {
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Status {
                camera: camera.eye,
                reply,
            })
            .unwrap();
        answer
            .recv_timeout(std::time::Duration::from_secs(15))
            .unwrap()
    };
    // Every Status consumes its actual one-shot prompt batch, even when
    // this test asks only about running/failure. Retain all such batches.
    let retained_prompts = std::cell::RefCell::new(std::collections::VecDeque::new());
    let status = || {
        let mut observed = status_raw();
        retained_prompts
            .borrow_mut()
            .extend(std::mem::take(&mut observed.prompts));
        observed
    };
    sender.send(FreshRequest::Run(false)).unwrap();
    let initial = status();
    assert!(initial.failure.is_none(), "{initial:?}");
    assert!(
        initial.ticks < 312,
        "construction test must precede9:30 tip"
    );
    for id in [11, 12, 13, 15, 16, 17, 18, 19, 20, 88] {
        sender.send(FreshRequest::TutorialControl(id)).unwrap();
    }
    sender
        .send(FreshRequest::TutorialProducer {
            control: 40,
            event: 7,
        })
        .unwrap();
    sender.send(FreshRequest::AuctionButton(0)).unwrap();
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::Planning {
            category: 33,
            reply,
        })
        .unwrap();
    let planning = answer.recv().unwrap().unwrap();
    assert_eq!(planning.entry(), crate::build_panel::Entry::Open);
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::BuildDrag {
            category: 33,
            reply,
        })
        .unwrap();
    answer.recv().unwrap().unwrap().unwrap();
    let floor = u32::from_le_bytes(
        planning.catalog.row(0).unwrap()[0x48..0x4c]
            .try_into()
            .unwrap(),
    ) as i32;
    let mut form = crate::build_panel::Request {
        category: 33,
        origin: [0, 0],
        size: [2, 2],
        height: floor,
        packed_model: 0,
        kind: 0,
        quality: 0,
        displayed_quote: f32::NAN,
        business: 0,
    };
    let mut allowed = false;
    for origin in [[0, 0], [0, 5], [5, 0], [5, 5]] {
        form.origin = origin;
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Build {
                form: form.clone(),
                submit: false,
                reply,
            })
            .unwrap();
        let quote = answer.recv().unwrap().unwrap().unwrap();
        if quote.allowed() {
            form.displayed_quote = quote.total;
            allowed = true;
            break;
        }
    }
    assert!(allowed);
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::Build {
            form,
            submit: true,
            reply,
        })
        .unwrap();
    assert!(answer.recv().unwrap().unwrap().unwrap().submitted);
    assert!(
        status().failure.is_none(),
        "building before first tip must remain legal"
    );
    let options = |open| {
        let (reply, ack) = mpsc::channel();
        sender.send(FreshRequest::Options { open, reply }).unwrap();
        ack.recv_timeout(std::time::Duration::from_secs(15))
            .expect("actual Options owner reply")
            .unwrap();
    };
    for running in [false, true] {
        sender.send(FreshRequest::Run(running)).unwrap();
        options(true);
        options(true);
        sender.send(FreshRequest::Run(true)).unwrap();
        assert!(!status().running);
        options(false);
        assert_eq!(status().running, running);
    }
    sender.send(FreshRequest::ToggleSpeed).unwrap();
    sender.send(FreshRequest::Run(true)).unwrap();
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    let paused = loop {
        // Leave a Status reply unread while opening Options. The worker
        // drains prompt events into that reply exactly once; the UI must
        // retain its receiver through the transition to display the tip.
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Status {
                camera: camera.eye,
                reply,
            })
            .unwrap();
        options(true);
        let mut barrier = status_raw();
        assert!(!barrier.running && barrier.failure.is_none(), "{barrier:?}");
        let mut snapshot = answer
            .recv_timeout(std::time::Duration::from_secs(15))
            .unwrap();
        options(false);
        assert!(snapshot.failure.is_none(), "{snapshot:?}");
        // Worker FIFO produced A before B, regardless of which receiver
        // the test read first. Neither actual returned batch may be lost.
        if !snapshot.prompts.is_empty() || !barrier.prompts.is_empty() {
            eprintln!(
                "retained tutorial Status batches A={} B={}",
                snapshot.prompts.len(),
                barrier.prompts.len()
            );
        }
        retained_prompts
            .borrow_mut()
            .extend(std::mem::take(&mut snapshot.prompts));
        retained_prompts
            .borrow_mut()
            .extend(std::mem::take(&mut barrier.prompts));
        let has_tip = !retained_prompts.borrow().is_empty();
        if has_tip {
            break status();
        } // actual post-close pause observation
        assert!(
            Instant::now() < deadline,
            "first original tip did not arrive"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(
        paused.script_paused,
        "original prompt owns its own pause reason"
    );
    assert_eq!(retained_prompts.borrow().front().unwrap().dismissal_id, 1);
    options(true);
    options(false);
    assert!(
        status().script_paused,
        "Options must not dismiss the scenario tip"
    );
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::DismissPrompt(1, Some(reply)))
        .unwrap();
    answer.recv().unwrap();
    assert_eq!(
        retained_prompts
            .borrow_mut()
            .pop_front()
            .unwrap()
            .dismissal_id,
        1
    );
    let after = status();
    assert!(
        after.running && !after.script_paused && after.failure.is_none(),
        "{after:?}"
    );
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let next = status();
        assert!(next.failure.is_none(), "{next:?}");
        if next.ticks > after.ticks {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "dismissed tip did not resume logic"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    // The user crash occurred after construction and subsequent original
    // tips, when the hourly ship reached the actual rendering adapter.
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    let ships = std::array::from_fn::<_, 4, _>(|i| {
        u32::from_le_bytes(
            image
                .at(0x539d50 + i as u32 * 4, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        )
    });
    let deadline = Instant::now() + std::time::Duration::from_secs(45);
    loop {
        let next = status();
        assert!(next.failure.is_none(), "{next:?}");
        let prompts: Vec<_> = retained_prompts.borrow_mut().drain(..).collect();
        for prompt in prompts {
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::DismissPrompt(
                    prompt.dismissal_id,
                    Some(reply),
                ))
                .unwrap();
            answer.recv().unwrap();
        }
        if next.ticks >= 2000 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Starting Out failed to reach its first hourly ship"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    sender.send(FreshRequest::Run(false)).unwrap();
    let (reply, answer) = mpsc::channel();
    sender
        .send(FreshRequest::Inspect { camera, reply })
        .unwrap();
    let mut snapshot = answer.recv().unwrap().unwrap();
    let transport = snapshot.transport.as_ref().unwrap();
    let references = transport.submitted_references().collect::<Vec<_>>();
    assert!(
        references.iter().any(|reference| ships.contains(reference)),
        "actual noon frame did not submit an hourly ship: {references:x?}"
    );
    transport.reframe(&mut snapshot.scene, camera, 35).unwrap();
    let focus = transport.first_position().unwrap();
    let near_transport = CameraSample {
        eye: [focus[0], focus[1] + 8., focus[2] - 24.],
        target: focus,
        ..camera
    };
    let mut transport_scene = snapshot.scene.clone();
    transport_scene.reframe(near_transport).unwrap();
    let transport_draws = transport
        .reframe(&mut transport_scene, near_transport, 35)
        .unwrap();
    assert!(
        transport_scene.draws[transport_draws]
            .iter()
            .any(|draw| !draw.vertices.is_empty()),
        "native transport callbacks must submit visible meshes near their pose"
    );
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_LIVE_TRANSPORT_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let prepared = renderer
            .prepare(
                &transport_scene
                    .draws
                    .iter()
                    .map(|draw| draw.borrowed())
                    .collect::<Vec<_>>(),
                transport_scene.camera,
                transport_scene.fog,
            )
            .unwrap();
        crate::capture_test::capture_prepared(&renderer, &transport_scene, &prepared, &path);
    }
    assert!(status().failure.is_none());
    eprintln!(
        "Starting Out: early build, stale controls, Options, queued tips and noon ship rendering pass ({references:x?})"
    );
    drop(snapshot);
    drop(sender);
    drop(loaded);
    worker.join().unwrap();
}

mod announcement_save_test {
    include!("../intro_announcement_save_test.rs");
}
