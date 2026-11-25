// Saved-preview composition of the production map, Locator and HUD producers.
// This exercises the loader and mouse controls, not an actual Intro window draw.
use super::*;

#[test]
#[ignore = "requires supplied original saves/Stats assets and GPU; saved-preview composition"]
fn capture_saved_preview_manager_stats() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let read=|key|fs::read(std::env::var(key).unwrap()).unwrap();
        let executable=read("OPENMT2001_EXE");
        let exe_path=PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let parameter=PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let language=exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let inputs=SceneInputs {
            process_runtime: None, //legacy fixture, not a process-owner assertion
            compatibility: crate::compatibility::Mode::StrictV14,style:test_board_style(&executable),time_runtime:Default::default(),
            profile_root:PathBuf::from("work/frontend/stats-capture-profile"),
            character_names:crate::fresh_city::character_names(&openmt2001::pe::PeImage::parse(&executable).unwrap(),&language).unwrap(),
            installer:parameter.parent().unwrap().parent().unwrap().to_path_buf(),language,
            maps:PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap()).parent().unwrap().to_path_buf(),
            executable,build_file:read("OPENMT2001_BUILD_FILE"),directory:read("OPENMT2001_ARCHIVE_DIR"),archive:read("OPENMT2001_ARCHIVE_BIN"),suncols:read("OPENMT2001_SUNCOLS")};
        let index=openmt2001::archive::ArchiveIndex::parse(&inputs.directory,inputs.archive.len() as u64).unwrap();
        let navigation=crate::navigation::Navigation::load(&index,&inputs.archive,&inputs.executable).unwrap();
        let offsets=fs::read(inputs.language.join("tycoon.off")).unwrap();let text=fs::read(inputs.language.join("tycoon.bin")).unwrap();
        let mut at=0;let strings=offsets.chunks_exact(2).map(|row|{let end=at+usize::from(u16::from_le_bytes(row.try_into().unwrap()));let row=&text[at..end];at=end;row}).collect::<Vec<_>>();
        let localized=crate::tooltip::localized_strings(&inputs.installer,&strings).unwrap();let strings=localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let assets=crate::stats_panel::Assets::load(&index,&inputs.archive,&inputs.installer,&strings,&inputs.executable).unwrap();
        let mut descriptor=wgpu::InstanceDescriptor::new_without_display_handle();if cfg!(windows){descriptor.backends=wgpu::Backends::DX12;}
        let renderer=pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        for (key,suffix) in [("OPENMT2001_SAVE_ONE","one"),("OPENMT2001_SAVE_TWO","two")] {
            let (_,mut session)=inputs.saved_preview(std::path::Path::new(&std::env::var(key).unwrap()),navigation.camera,&mut SnapshotCache::default()).unwrap();
            let mut host=crate::live_host::Host::new(navigation.camera.eye);host.begin_presentation(&session).unwrap();
            let original_pause=session.world.pause.bytes;
            host.stats_action(&mut session,crate::live_host::StatsAction::Open).unwrap();
            let status=host.status(&session);let snapshot=status.stats.as_ref().unwrap();
            assert!(!status.running,"player pause must remain independent of active Manager session");
            assert_eq!(session.world.pause.bytes[0],original_pause[0].wrapping_add(1));
            assert_eq!(snapshot.history,session.history.as_bytes());
            let mut view=crate::stats_panel::View::default();view.update(&assets,&renderer,Some(snapshot)).unwrap();
            crate::capture_test::capture_standalone(&renderer,view.prepared.as_ref().unwrap(),&format!("work/frontend/stats-saved-preview-{suffix}.png"));
            let point=Some([433.,86.]); //actual STATS_RIGHT_CHANGER interior
            assert!(view.pointer(&assets,point,true,snapshot).is_none());
            let action=view.pointer(&assets,point,false,snapshot).unwrap();
            assert!(matches!(action,crate::live_host::StatsAction::Cycle(true)));
            host.stats_action(&mut session,action).unwrap();
            let status=host.status(&session);let snapshot=status.stats.as_ref().unwrap();
            view.update(&assets,&renderer,Some(snapshot)).unwrap();
            crate::capture_test::capture_standalone(&renderer,view.prepared.as_ref().unwrap(),&format!("work/frontend/stats-saved-preview-{suffix}-next.png"));
            //A press outside cannot acquire a widget by moving into it while held.
            assert!(view.pointer(&assets,Some([0.,0.]),true,snapshot).is_none());
            assert!(view.pointer(&assets,point,true,snapshot).is_none());
            assert!(view.pointer(&assets,point,false,snapshot).is_none());
            host.stats_action(&mut session,crate::live_host::StatsAction::Close).unwrap();
            assert_eq!(session.world.pause.bytes,original_pause);
            assert!(host.status(&session).stats.is_none());view.update(&assets,&renderer,None).unwrap();assert!(view.prepared.is_none());
        }
    }).unwrap().join().unwrap();
}

#[test]
#[ignore = "requires supplied original save/assets and GPU"]
fn capture_saved_preview_strategic_composition() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let inputs = SceneInputs {
            process_runtime: None, //legacy fixture, not a process-owner assertion
            compatibility: crate::compatibility::Mode::StrictV14,
            style: test_board_style(&executable),
            time_runtime: Default::default(),
            profile_root: PathBuf::from("work/frontend/strategic-capture-profile"),
            character_names: crate::fresh_city::character_names(
                &openmt2001::pe::PeImage::parse(&executable).unwrap(), &language).unwrap(),
            installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
            language,
            maps: PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap())
                .parent().unwrap().to_path_buf(),
            executable, build_file: read("OPENMT2001_BUILD_FILE"),
            directory: read("OPENMT2001_ARCHIVE_DIR"),
            archive: read("OPENMT2001_ARCHIVE_BIN"), suncols: read("OPENMT2001_SUNCOLS"),
        };
        let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
        let index = openmt2001::archive::ArchiveIndex::parse(&inputs.directory,
            inputs.archive.len() as u64).unwrap();
        let navigation = crate::navigation::Navigation::load(&index, &inputs.archive,
            &inputs.executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let map_assets = crate::strategic_scene::Assets::load(&image, &index,
            &inputs.archive, &board.blocks).unwrap();
        let offsets = fs::read(inputs.language.join("tycoon.off")).unwrap();
        let text = fs::read(inputs.language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets.chunks_exact(2).map(|row| {
            let end = at + usize::from(u16::from_le_bytes(row.try_into().unwrap()));
            let string = &text[at..end]; at = end; string
        }).collect::<Vec<_>>();
        let comparison_assets = crate::comparison::Assets::load(&index, &inputs.archive,
            &inputs.installer, &strings, &inputs.executable).unwrap();
        let locator_assets = crate::locator::Assets::load(&index, &inputs.archive,
            &inputs.installer, &strings, &inputs.executable).unwrap();
        let card_assets = crate::card_panel::Assets::load(&index, &inputs.archive,
            &inputs.installer, &strings).unwrap();
        let (_, regions) = crate::ui::panel_frame_base(&index, &inputs.archive,
            "gamedata2/brand_new_panels/main_panel_v2.tm", &(0..12).collect::<Vec<_>>(),
            9).unwrap();
        let generic_entry = index.find("gamedata2/GenericTextures640/textures.gtd").unwrap();
        let generic = openmt2001::texture::TextureSet::parse(&inputs.archive[
            generic_entry.offset as usize..(generic_entry.offset + generic_entry.length) as usize])
            .unwrap();
        let button = &generic.textures[13];
        let city_buttons = crate::ui::mpp_modes(regions[9], [button.width, button.height])
            .unwrap().buttons;
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let time_assets = crate::time_panel::Assets::load(&index, &inputs.archive,
            &inputs.installer, &inputs.language, &inputs.executable).unwrap();
        let navigator = crate::navigator::Navigator::load(&index, &inputs.archive,
            &board.blocks, &inputs.executable).unwrap();
        let compass = navigation.prepare(&renderer).unwrap();
        for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let (mut loaded, session) = inputs.saved_preview(
                std::path::Path::new(&std::env::var(key).unwrap()), navigation.camera,
                &mut SnapshotCache::default()).unwrap();
            let direct = crate::city_people::Report::from_world(
                &session.world, &image, &session.roster.manager);
            eprintln!("{key}: restored residents {}, auxiliaries {}, direct runtime adapter {:?}",
                session.world.ledgers.residents.active_count, session.world.creation.auxiliary.used(),
                direct.as_ref().map(|report| (report.markers.len(),
                    report.markers.iter().filter(|marker| marker.visible).count())));
            // Use the verified native overview pose, as the actual compass transition does.
            loaded.scene.reframe(navigation.camera).unwrap();
            let mut state = if let Some(inputs) = loaded.runtime_locator.as_ref() {
                locator_assets.runtime_state(inputs).unwrap()
            } else { locator_assets.state(&loaded.scene).unwrap().unwrap() };
            let markers = loaded.city_people.as_ref().unwrap().markers.len();
            assert_eq!(markers, session.world.creation.auxiliary.used());
            assert!(loaded.city_people.as_ref().unwrap().markers.iter().any(|m| m.visible));
            state.attach_people(loaded.city_people.take().unwrap());
            state.attach_players(loaded.player_info.take().unwrap());
            state.refresh_scene(&loaded.scene);
            state.attach_polls(loaded.consumer_polls);
            state.attach_ownership(loaded.city_ownership.take());
            state.attach_rights(loaded.city_rights.take());
            state.attach_prestige(loaded.city_prestige.take());
            let mut comparison = crate::comparison::View::new(loaded.comparison.take().unwrap());
            comparison.prepare(&comparison_assets, &renderer, state.financial_records()).unwrap();
            let map = map_assets.prepare(&renderer, loaded.scene.camera,
                loaded.block_information.as_ref().map(|report| &report.title_colors)).unwrap();
            let mini = navigator.prepare_with_ownership(&renderer, &loaded.navigator_owners).unwrap();
            let mut host = crate::live_host::Host::new(navigation.camera.eye);
            host.time_runtime = Arc::clone(&inputs.time_runtime);
            host.begin_presentation(&session).unwrap();
            let status = host.status(&session);
            assert!(status.failure.is_none(), "saved HUD snapshot failed: {:?}", status.failure);
            let mut time = crate::time_panel::View::default();
            time.update(&time_assets, &renderer, &status).unwrap();
            let chance = card_assets.entry_scene(&renderer, status.card_entry,
                status.card_visual, &crate::card_panel::Input::default(), None).unwrap();
            let suffix = if key == "OPENMT2001_SAVE_ONE" { "one" } else { "two" };
            for (mode, name) in [(0, "people"), (1, "prestige"), (2, "rights"), (3, "ownership")] {
                let [l, t, r, b] = city_buttons[mode];
                let point = Some([(l + r) as i32, (t + b) as i32]);
                state.mouse(&locator_assets, &loaded.scene, point, true);
                state.mouse(&locator_assets, &loaded.scene, point, false);
                assert_eq!(state.take_tutorial_control(), Some(24 + mode as u32));
                let panel = locator_assets.prepare(&renderer, &state, &loaded.scene).unwrap();
                crate::capture_test::capture_prepared_world_overlays(&renderer, &map, &[],
                    &[&mini, &panel, comparison.prepared.as_ref().unwrap(), &compass[0],
                        time.prepared.as_ref().unwrap(), &chance],
                    &format!("work/frontend/strategic-preview-{suffix}-{name}.png"));
            }
            // Actual Locator toggle exposes the existing business footprint producer.
            let point = Some([34, 226]);
            state.mouse(&locator_assets, &loaded.scene, point, true);
            state.mouse(&locator_assets, &loaded.scene, point, false);
            assert_eq!(state.take_tutorial_control(), Some(22));
            let panel = locator_assets.prepare(&renderer, &state, &loaded.scene).unwrap();
            crate::capture_test::capture_prepared_world_overlays(&renderer, &map, &[],
                &[&mini, &panel, comparison.prepared.as_ref().unwrap(), &compass[0],
                        time.prepared.as_ref().unwrap(), &chance],
                &format!("work/frontend/strategic-preview-{suffix}-businesses.png"));
            eprintln!("{key}: actual saved-preview {} buildings, {markers} People markers",
                loaded.scene.inspection_buildings.len());
        }
    }).unwrap().join().unwrap();
}
