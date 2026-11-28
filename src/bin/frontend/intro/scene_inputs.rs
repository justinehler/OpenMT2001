//! `impl SceneInputs`, split out of `intro.rs` (#99). Behaviour lives here unchanged.
use super::*;

impl SceneInputs {
    pub(super) fn styled_board(
        &self,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<openmt2001::board::Board, String> {
        let mut board = openmt2001::board::Board::from_v14(image)?;
        self.style.apply_board(&mut board)?;
        Ok(board)
    }

    pub(super) fn preview_worker(
        &self,
        setup: crate::singleplayer::Setup,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
    ) {
        self.session_worker(WorkerStart::Fresh(setup), camera, tx, None);
    }
    pub(super) fn saved_worker(
        &self,
        path: PathBuf,
        save_profile_root: Option<PathBuf>,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
        pavement: crate::selection::pavement_resource::LocalPavementHandle,
    ) {
        self.session_worker(
            WorkerStart::Saved(path, save_profile_root),
            camera,
            tx,
            Some(pavement),
        );
    }
    pub(super) fn session_worker(
        &self,
        start: WorkerStart,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
        pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
    ) {
        self.session_worker_with_focus(start, camera, tx, pavement, false);
    }
    pub(super) fn session_worker_with_focus(
        &self,
        start: WorkerStart,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
        pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment: bool,
    ) {
        self.session_worker_with_sources(
            start,
            camera,
            tx,
            pavement,
            require_focus_attachment,
            false,
        );
    }
    pub(super) fn session_worker_with_sources(
        &self,
        start: WorkerStart,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
        pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment: bool,
        produce_camera: bool,
    ) {
        self.session_worker_with_terminal(
            start,
            camera,
            tx,
            pavement,
            require_focus_attachment,
            produce_camera,
            camera_producer::ExitFailures::default(),
        );
    }
    pub(super) fn session_worker_with_terminal(
        &self,
        start: WorkerStart,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
        pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment: bool,
        produce_camera: bool,
        exit_failures: camera_producer::ExitFailures,
    ) {
        let _ = self.session_worker_joined(
            start,
            camera,
            tx,
            pavement,
            require_focus_attachment,
            produce_camera,
            exit_failures,
            None,
        );
    }
    pub(super) fn session_worker_joined(
        &self,
        start: WorkerStart,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
        pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment: bool,
        produce_camera: bool,
        exit_failures: camera_producer::ExitFailures,
        custody: Option<crate::process_dispatch::DispatchCustody>,
    ) -> Option<crate::process_dispatch::DispatchCustody> {
        let joined = custody.is_some();
        let fresh_completion = matches!(
            &start,
            WorkerStart::Fresh(_) | WorkerStart::Skirmish(_) | WorkerStart::Connected(_)
        );
        let (mut dispatch, mut timing) = if let Some(custody) = custody {
            match custody.prepare_start(matches!(
                &start,
                WorkerStart::Fresh(_) | WorkerStart::Skirmish(_) | WorkerStart::Connected(_)
            )) {
                Ok((seed, timing)) => (Some(seed), timing),
                Err(error) => {
                    let _ = tx.send(Err(error.into()));
                    return None;
                }
            }
        } else {
            // Explicit isolated caller path. Ordinary App ALWAYS supplies custody.
            let mut frame = openmt2001::timers::Frame::default();
            frame.initialize(0);
            (
                None,
                crate::process_dispatch::Timing {
                    frame,
                    epoch: Instant::now(),
                },
            )
        };

        if let Some(process) = &self.process_runtime {
            if let Err(error) = process.world_controller_reset4c2b80() {
                let _ = tx.send(Err(error.to_string().into()));
                return None;
            }
        }
        let _offline_lease = camera_producer::OfflineLease {
            process: self.process_runtime.clone(),
            failures: Arc::clone(&exit_failures),
        };
        if matches!(
            &start,
            WorkerStart::Fresh(_) | WorkerStart::Skirmish(_) | WorkerStart::Connected(_)
        ) && let Some(process) = &self.process_runtime
            && let Err(error) = process.source_fresh_world()
        {
            let _ = tx.send(Err(error.to_string().into()));
            return None;
        }
        let mut cache = SnapshotCache::default();
        let selected_save_root = match &start {
            WorkerStart::Connected(_) => None,
            WorkerStart::Saved(_, root) => root.clone(),
            WorkerStart::Skirmish(request) => Some(request.save_root.clone()),
            WorkerStart::Fresh(setup) => {
                crate::save_menu::profile_root_for_campaign(&self.profile_root, &setup.profile_path)
            }
        };
        // Private native-review activation. It admits only imported ordinary
        // routes with separately inspected first-Level effects. Fresh/menu and
        // generic tutorial producer ordering remain outside this experiment.
        let review_camera = matches!(&start, WorkerStart::Saved(..))
            && std::env::var_os("OPENMT2001_CAMERA_OWNER_NATIVE_REVIEW").is_some();
        let result = match start {
            WorkerStart::Connected(request) => self
                .fresh_connected_joined(*request, camera, &mut cache, dispatch.take())
                .map_err(Into::into),
            WorkerStart::Skirmish(request) => self
                .fresh_skirmish_joined(request, camera, &mut cache, dispatch.take())
                .map_err(Into::into),
            WorkerStart::Fresh(setup) => {
                let retained = match setup.preview {
                    crate::singleplayer::Preview::Scenario(id) => {
                        crate::scenarios::live_tutorial(id)
                            || matches!(id, 200001..=200016 | 400001)
                            || setup.catalog.find(id).is_some_and(|index| {
                                crate::fresh_city::custom::valid_level(&setup.catalog.levels[index])
                            })
                    }
                    crate::singleplayer::Preview::Sandbox => true,
                };
                if retained {
                    self.fresh_preview_joined(setup, camera, &mut cache, dispatch.take())
                        .map_err(Into::into)
                } else {
                    let _ = tx.send(self.preview_scene(setup, camera).map_err(Into::into));
                    return None;
                }
            }
            WorkerStart::Saved(path, _) => self.saved_preview_with_selection_joined(
                &path,
                camera,
                &mut cache,
                Some(selected_save_root.as_deref()),
                dispatch.take(),
            ),
        };
        match result {
            Ok((mut loaded, mut session)) => {
                let binding = (|| -> Result<(), String> {
                    let index = openmt2001::archive::ArchiveIndex::parse(
                        &self.directory,
                        self.archive.len() as u64,
                    )
                    .map_err(|error| error.to_string())?;
                    let entry = index
                        .find("gamedata2/BuildingTextures/textures.gtd")
                        .ok_or("window texture collection missing")?;
                    let set = openmt2001::texture::TextureSet::parse(
                        &self.archive
                            [entry.offset as usize..(entry.offset + entry.length) as usize],
                    )
                    .map_err(|error| error.to_string())?;
                    let image = set.textures.get(26).ok_or("window texture missing")?;
                    session.window_texture.bind(
                        image.width,
                        image.height,
                        openmt2001::texture::loader_pixels(image).into(),
                    );
                    Ok(())
                })();
                if let Err(error) = binding {
                    let _ = tx.send(Err(error.into()));
                    return None;
                }

                // Ordinary saves belong to the selected profile independently
                // of whether this session may persist campaign progression.
                let save_profile_root = selected_save_root;
                loaded.save_profile_root = save_profile_root.clone();
                let saved_auction = loaded.saved_auction.take();
                let restored_presentation = std::mem::take(&mut loaded.restored_presentation);
                let host_eye = loaded
                    .saved_viewpoint
                    .map_or(camera.eye, |(sample, _)| sample.eye);
                let (lease, released) = mpsc::channel();
                loaded.fresh_session = Some(lease);
                let (auction_renderer, auction_requests) = mpsc::channel();
                loaded.auction_requests = Some(auction_requests);
                let (auction_audio, auction_audio_requests) = mpsc::channel();
                loaded.auction_audio_requests = Some(auction_audio_requests);
                let (transport_audio, transport_audio_requests) = mpsc::channel();
                loaded.transport_audio_requests = Some(transport_audio_requests);
                let (era_audio, era_audio_requests) = mpsc::channel();
                loaded.era_audio_requests = Some(era_audio_requests);
                let (gameplay_sound, gameplay_sound_requests) = mpsc::channel();
                loaded.gameplay_sound_requests = Some(gameplay_sound_requests);
                let camera_connection = if review_camera {
                    let id = session.catalog.selected_level().map(|level| level.id());
                    if !matches!(id, Some(200003 | 400002)) {
                        let _ = tx.send(Err(
                            "Private camera review has no first-Level contract for this route"
                                .into(),
                        ));
                        return None;
                    }
                    let Some(capture) = loaded.imported_cameras else {
                        let _ = tx.send(Err(
                            "Private camera review requires both imported raw records".into(),
                        ));
                        return None;
                    };
                    let generation = session.world.topology.epoch().serial();
                    let (worker, inbox) = crate::live_host::camera_bridge::channel(generation);
                    loaded.camera_inbox = Some(inbox);
                    let Some(pavement) = pavement else {
                        let _=tx.send(Err("Private camera review requires originating selection pavement registration".into()));
                        return None;
                    };
                    Some((worker, capture, pavement))
                } else {
                    None
                };
                let ui_camera_reader = if produce_camera && !review_camera {
                    let (publisher, reader) =
                        crate::save_menu::camera_source::channel(session.world.topology.epoch());
                    loaded.camera_epoch = Some(session.world.topology.epoch());
                    loaded.camera_publisher = Some(publisher);
                    Some(reader)
                } else {
                    None
                };
                if fresh_completion {
                    timing
                        .frame
                        .synchronize(timing.epoch.elapsed().as_millis() as u32);
                    for task in &mut session.hooks.timers {
                        task[12..16].copy_from_slice(&0f32.to_le_bytes());
                    }
                }
                if tx.send(Ok(loaded)).is_ok() {
                    let mut host = crate::live_host::Host::new(host_eye);
                    macro_rules! cancelled_session {
                        () => {{
                            if let Some(error) = &host.failure {
                                camera_producer::record_exit(&exit_failures, error.clone());
                            }
                            if joined {
                                match crate::process_dispatch::custody_from_session(
                                    &mut session,
                                    timing,
                                ) {
                                    Ok(custody) => return Some(custody),
                                    Err(error) => {
                                        camera_producer::record_exit(&exit_failures, error)
                                    }
                                }
                            }
                            return None;
                        }};
                    }
                    if let Some(reader) = &ui_camera_reader {
                        if let Err(error) = host.install_ui_camera_source(&session, reader.clone())
                        {
                            host.fail(error);
                            return None;
                        }
                    }
                    if session.connected.is_some() {
                        host.policy.scheduler.network_enabled = true;
                        host.policy.scheduler.deadline = 25;
                    }
                    host.autosave_profile_root = save_profile_root.clone();
                    host.process_runtime = self.process_runtime.clone();
                    // SAME process Frame/epoch; fresh completion above uses
                    //4ebaa0 only. Saved adoption never initializes the manager.
                    if let Some((worker, capture, pavement)) = camera_connection {
                        // Host already classifies the typed install result;
                        // explicit retirement is not an incomplete simulation.
                        match openmt2001::pe::PeImage::parse(&self.executable)
                            .map_err(crate::live_host::camera_bridge::RequestError::from)
                            .and_then(|image| {
                                host.install_camera_bridge(
                                    &mut session,
                                    &image,
                                    crate::live_host::CameraSupplierPolicy::CameraOnly,
                                    worker,
                                    capture,
                                    pavement,
                                )
                            }) {
                            Ok(()) => {}
                            Err(crate::live_host::camera_bridge::RequestError::Retired) => {
                                cancelled_session!()
                            }
                            Err(crate::live_host::camera_bridge::RequestError::Failed(error)) => {
                                host.fail(error)
                            }
                        }
                        if host.camera_retired() {
                            cancelled_session!();
                        }
                    }
                    host.time_runtime = Arc::clone(&self.time_runtime);
                    host.auction_renderer = Some(auction_renderer);
                    host.auction_audio = Some(auction_audio);
                    host.transport_audio = Some(transport_audio);
                    host.era_audio = Some(era_audio);
                    host.gameplay_sound = Some(gameplay_sound);
                    if let Err(error) = host.begin_presentation(&session) {
                        host.fail(error);
                    }
                    if host.camera_retired() {
                        cancelled_session!();
                    }
                    //4ba167 enables gameplay after the startup pause counter
                    //has drained; retained script/modal pause reasons still
                    //gate each logic dispatch independently.
                    host.set_running(true);
                    if host.camera_retired() {
                        cancelled_session!();
                    }
                    if let Err(error) = host.apply_restored_presentation(restored_presentation) {
                        host.fail(error);
                    }
                    if host.camera_retired() {
                        cancelled_session!();
                    }
                    if let Err(error) = host.activate_saved_auction(&mut session) {
                        host.fail(error);
                    }
                    if host.camera_retired() {
                        cancelled_session!();
                    }
                    if let Some(continuation) = saved_auction
                        && let Err(error) = continuation.apply(&mut session).and_then(|()| {
                            host.restore_auction_widgets(continuation.host.as_ref(), &session)
                        })
                    {
                        host.fail(error);
                    }
                    let image = match openmt2001::pe::PeImage::parse(&self.executable) {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("{error}");
                            return None;
                        }
                    };
                    let board = match self.styled_board(&image) {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("{error}");
                            return None;
                        }
                    };
                    let file = match openmt2001::build_file::BuildFile::parse(&self.build_file) {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("{error}");
                            return None;
                        }
                    };
                    let assets = crate::fresh_city::CommandAssets {
                        image: &image,
                        board: &board,
                        file: &file,
                    };
                    if host.camera_retired() {
                        cancelled_session!();
                    }
                    if let Some(reader) = &ui_camera_reader {
                        // The UI publishes its complete cold transaction before
                        // this barrier. No frame/save observes a partial owner.
                        match released.recv() {
                            Ok(FreshRequest::UiCameraReady { reply }) => {
                                let result =
                                    reader.read(session.world.topology.epoch()).map(|_| ());
                                let failed = result.is_err();
                                let _ = reply.send(result);
                                if failed {
                                    return None;
                                }
                            }
                            Err(_) => cancelled_session!(),
                            Ok(_) => {
                                camera_producer::record_exit(
                                    &exit_failures,
                                    "Expected real camera Ready barrier".into(),
                                );
                                return None;
                            }
                        }
                    }
                    if require_focus_attachment {
                        // Explicit modern cold policy: all restore/startup and
                        // presentation effects above precede ONE live OS-focus
                        // snapshot. No frame or queued save may pass this barrier.
                        match released.recv() {
                            Ok(FreshRequest::Focus(event)) if event.is_attachment() => {
                                if event
                                    .apply(
                                        &mut host,
                                        &mut session.world.pause,
                                        &mut session.hooks.timers,
                                    )
                                    .is_err()
                                {
                                    return None;
                                }
                            }
                            Ok(_) => {
                                host.fail(
                                    "First focus-enabled request must attach current OS focus"
                                        .into(),
                                );
                                return None;
                            }
                            Err(_) => cancelled_session!(),
                        }
                    }
                    let mut last_tick = Instant::now();
                    loop {
                        if host.camera_retired()
                            || ui_camera_reader.as_ref().is_some_and(|reader| {
                                reader.read(session.world.topology.epoch()).is_err()
                            })
                        {
                            break;
                        }
                        // Poll original interface/logic timers from monotonic host
                        // milliseconds. Rendering and worker requests are separate.
                        let period = std::time::Duration::from_millis(16);
                        let request = match released
                            .recv_timeout(period.saturating_sub(last_tick.elapsed()))
                        {
                            Ok(request) => Some(request),
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        if host.camera_retired()
                            || ui_camera_reader.as_ref().is_some_and(|reader| {
                                reader.read(session.world.topology.epoch()).is_err()
                            })
                        {
                            break;
                        }
                        // Camera selection is an admitted FIFO prefix too. All
                        // Session borrows end before its by-value reply wait.
                        let request = match request {
                            Some(FreshRequest::CameraTurn { turn, reply }) => {
                                let admitted = camera_producer::admit(&session, turn);
                                if let Ok(posts) = camera_producer::deliver(turn, admitted, reply) {
                                    // Consumed once, on this SAME validated Session, after
                                    // camera/UI publication and before next frame/request.
                                    if camera_producer::run_posts(
                                        &mut host,
                                        &mut session,
                                        &assets,
                                        posts,
                                        &exit_failures,
                                    )
                                    .is_err()
                                    {
                                        return None;
                                    }
                                }
                                if ui_camera_reader.as_ref().is_some_and(|reader| {
                                    reader.read(session.world.topology.epoch()).is_err()
                                }) {
                                    break;
                                }
                                continue;
                            }
                            other => other,
                        };
                        // Received focus takes effect before any next due frame.
                        // No Options-style preflush and no completed-frame replay.
                        let request = match focus::before_frame(
                            request,
                            &mut host,
                            &mut session.world.pause,
                            &mut session.hooks.timers,
                        ) {
                            Ok(focus::BeforeFrame::FocusApplied) => continue,
                            Ok(focus::BeforeFrame::Request(other)) => other,
                            Err(_) => break,
                        };
                        if last_tick.elapsed() >= period {
                            let elapsed = timing
                                .frame
                                .elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                            last_tick = Instant::now();
                            host.frame(&mut session, &assets, elapsed);
                            if std::mem::take(&mut host.synchronize_time) {
                                timing
                                    .frame
                                    .synchronize(timing.epoch.elapsed().as_millis() as u32);
                            }
                        }
                        if host.camera_retired() {
                            break;
                        }
                        let Some(request) = request else {
                            continue;
                        };
                        match request {
                            FreshRequest::Focus(_) => unreachable!("focus handled before frame"),
                            FreshRequest::CameraTurn { .. } => {
                                unreachable!("camera turn handled before frame")
                            }
                            FreshRequest::UiCameraReady { reply } => {
                                let _ =
                                    reply.send(Err("Camera attachment already completed".into()));
                            }
                            FreshRequest::PropertyRights {
                                category,
                                panel_mode,
                                activate,
                                reply,
                            } => {
                                let _ = reply.send(host.property_rights(
                                    &mut session,
                                    &assets,
                                    category,
                                    panel_mode,
                                    activate,
                                ));
                            }
                            FreshRequest::PropertyOffer { request, reply } => {
                                let result = if request.submit {
                                    host.property_offer(
                                        &mut session,
                                        &assets,
                                        request.category,
                                        request.panel,
                                    )
                                } else {
                                    host.inspect_property_offer(
                                        &mut session,
                                        &assets,
                                        request.category,
                                        request.panel,
                                        request.initialize_amount,
                                    )
                                };
                                let _ = reply.send(result);
                            }
                            FreshRequest::SaveSelected {
                                target,
                                camera,
                                capture,
                                game_mode,
                                interface_mode,
                                reply,
                            } => {
                                let result = (|| {
                                    let root = save_profile_root
                                        .as_deref()
                                        .ok_or("Save Game requires a selected V2 profile")?;
                                    if host.has_ui_camera_source() {
                                        return host.save_selected_from_ui_snapshot(
                                            &mut session,
                                            &image,
                                            root,
                                            &target,
                                            game_mode,
                                        );
                                    }
                                    if capture.is_some()
                                        && !host.options_save_allowed(
                                            &session.world.pause,
                                            &session.hooks.timers,
                                        )
                                    {
                                        return Err(
                                            "Save requires the active Options hold".to_owned()
                                        );
                                    }
                                    //The by-value pair retains Owner's raw bytes. Do not
                                    //recompute a sample or overlay either record here.
                                    let cameras = match capture {
                                        Some(capture) => {
                                            if review_camera {
                                                eprintln!("[camera] serializing captured raw pair");
                                            }
                                            capture.into_records()
                                        }
                                        None => crate::save_menu::cameras(
                                            session.saved_cameras,
                                            camera,
                                        )?,
                                    };
                                    let view = host.save_view(&session)?;
                                    let bytes = session.save_v2_with_options_view(
                                        &image,
                                        cameras,
                                        game_mode,
                                        view,
                                        interface_mode,
                                    )?;
                                    crate::save_menu::write_target(root, &target, &bytes)
                                })();
                                let _ = reply.send(result);
                            }
                            FreshRequest::DeleteSave { id, reply } => {
                                let result = save_profile_root
                                    .as_deref()
                                    .ok_or("Save Game requires a selected V2 profile".to_owned())
                                    .and_then(|root| crate::save_menu::delete_slot(root, &id));
                                let _ = reply.send(result);
                            }
                            FreshRequest::ResultState { reply } => {
                                let _ = reply.send(
                                    session
                                        .result_presentation()
                                        .cloned()
                                        .map(|presentation| ResultSnapshot {
                                            presentation,
                                            tick: session.world.clock.timing[0],
                                            hour: session.world.clock.hour(),
                                            counter: session.world.clock.timing[2],
                                        })
                                        .ok_or(
                                            "Result request has no retained script state".into(),
                                        ),
                                );
                            }
                            FreshRequest::ResultCredits { reply } => {
                                let result =
                                    session.construct_result_credits(&image).and_then(|_| {
                                        session.result_presentation().cloned().ok_or(
                                            "Result credits lost retained script state".into(),
                                        )
                                    });
                                let _ = reply.send(result);
                            }
                            FreshRequest::ResultEvent { name, reply } => {
                                let result =
                                    host.dispatch_result_event(&mut session, &assets, name)
                                        .and_then(|_| {
                                            session.result_presentation().cloned().ok_or(
                                                "Result event lost retained script state".into(),
                                            )
                                        });
                                let _ = reply.send(result);
                            }
                            FreshRequest::ResultComplete { reply } => {
                                let completed = host
                                    .complete_result_sequence(&mut session)
                                    .map(|_| host.status(&session));
                                let _ = reply.send(completed);
                            }
                            FreshRequest::InspectOperations {
                                building,
                                bounds,
                                reply,
                            } => {
                                let _ = reply.send(
                                    session.inspect_business_operations(building, bounds, &image),
                                );
                            }
                            FreshRequest::BusinessOperation {
                                building,
                                action,
                                reply,
                            } => {
                                let _ = reply.send(host.business_operation(
                                    &mut session,
                                    &assets,
                                    building,
                                    action,
                                ));
                            }
                            FreshRequest::Card(action) => {
                                if let Err(error) = host.human_card(&mut session, &assets, action) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::HumanDialog(response, reply) => {
                                if let Err(error) =
                                    host.human_dialog_reply(&mut session, &assets, response)
                                {
                                    host.fail(error);
                                }
                                let _ = reply.send(host.take_prompts(&session));
                            }
                            FreshRequest::AuctionButton(button) => {
                                if let Err(error) =
                                    host.auction_button(&mut session, &assets, button)
                                {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::Residents { reply } => {
                                let result = crate::people_scene::Layer::from_resources(
                                    &session,
                                    &self.suncols,
                                    cache.people.as_ref().unwrap(),
                                );
                                let _ = reply.send(result);
                            }
                            FreshRequest::Vehicles { reply } => {
                                let result = session
                                    .road_traffic
                                    .project(&mut session.world.road_network, 0.)
                                    .and_then(|()| {
                                        crate::vehicle_scene::Layer::from_resources(
                                            &session,
                                            &self.suncols,
                                            cache.vehicles.as_ref().unwrap(),
                                        )
                                    });
                                let _ = reply.send(result);
                            }
                            FreshRequest::Transport { reply } => {
                                let mut auxiliaries = Vec::new();
                                let result = session
                                    .advance_transport_frame(|auxiliary| {
                                        auxiliaries.push(*auxiliary);
                                        Ok(())
                                    })
                                    .and_then(|()| {
                                        crate::transport_scene::Layer::from_resources(
                                            &session,
                                            &self.suncols,
                                            cache.transport.as_ref().unwrap(),
                                            auxiliaries,
                                        )
                                    });
                                let _ = reply.send(result);
                            }
                            FreshRequest::EditBusiness {
                                target,
                                action,
                                reply,
                            } => {
                                let result = if host.failure.is_some() {
                                    Err("Stopped session cannot edit businesses".into())
                                } else {
                                    crate::business_edit::edit(
                                        &mut session,
                                        &assets,
                                        target,
                                        action,
                                    )
                                };
                                let _ = reply.send(result);
                            }
                            FreshRequest::Run(running) => {
                                let elapsed = timing
                                    .frame
                                    .elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                                host.frame(&mut session, &assets, elapsed);
                                if std::mem::take(&mut host.synchronize_time) {
                                    timing
                                        .frame
                                        .synchronize(timing.epoch.elapsed().as_millis() as u32);
                                }
                                if host.camera_retired() {
                                    break;
                                }
                                host.set_running(running);
                                last_tick = Instant::now();
                            }
                            FreshRequest::Options { open, reply } => {
                                let elapsed = timing
                                    .frame
                                    .elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                                host.frame(&mut session, &assets, elapsed);
                                if std::mem::take(&mut host.synchronize_time) {
                                    timing
                                        .frame
                                        .synchronize(timing.epoch.elapsed().as_millis() as u32);
                                }
                                if host.camera_retired() {
                                    break;
                                }
                                let result = host.options(
                                    &mut session.world.pause,
                                    &mut session.hooks.timers,
                                    open,
                                );
                                if let Err(error) = &result {
                                    host.fail(error.clone());
                                }
                                let _ = reply.send(result);
                                last_tick = Instant::now();
                            }
                            FreshRequest::DismissMarketShare => host.dismiss_market_share(),
                            FreshRequest::ToggleSpeed => host.toggle_speed(),
                            FreshRequest::ComparisonMode(mode) => {
                                if let Err(error) = host.select_comparison(&session, &assets, mode)
                                {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::ScenarioReminder => {
                                if let Some(level) = session.catalog.selected_level() {
                                    host.scenario_reminder(level, session.world.objective.kind);
                                }
                            }
                            FreshRequest::Stats(action) => {
                                if let Err(error) = host.stats_action(&mut session, action) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::DismissPrompt(id, reply) => {
                                let elapsed = timing
                                    .frame
                                    .elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                                host.frame(&mut session, &assets, elapsed);
                                if std::mem::take(&mut host.synchronize_time) {
                                    timing
                                        .frame
                                        .synchronize(timing.epoch.elapsed().as_millis() as u32);
                                }
                                if host.camera_retired() {
                                    break;
                                }
                                if host.failure.is_none()
                                    && let Err(error) = session.dispatch_session_event(
                                        "InterfacePromptDismiss",
                                        id as i32,
                                        &assets,
                                        true,
                                    )
                                {
                                    host.fail(error);
                                }
                                last_tick = Instant::now();
                                if let Some(reply) = reply {
                                    let _ = reply.send(host.take_prompts(&session));
                                }
                            }
                            FreshRequest::TutorialControl(id) => {
                                if let Err(error) = host.tutorial_event(&mut session, &assets, id) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::TutorialProducer { control, event } => {
                                if let Err(error) = host.tutorial_producer_event(
                                    &mut session,
                                    &assets,
                                    control,
                                    event,
                                ) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::TutorialPick(pick) => {
                                if let Err(error) = host.tutorial_pick(&mut session, &assets, pick)
                                {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::WindowTexturePublished(revision) => {
                                session.window_texture.published(revision);
                            }
                            FreshRequest::Status { camera, reply } => {
                                host.observe_presentation_camera(camera);
                                let _ = reply.send(host.status(&session));
                            }
                            #[cfg(test)]
                            FreshRequest::DispatchProbe(reply) => {
                                let _ = reply.send(DispatchProbe {
                                    pause: session.world.pause.bytes,
                                    tasks: session.hooks.timers.clone(),
                                    frequency: (
                                        session.hooks.frequency.frequency,
                                        session.hooks.frequency.calendar_timer,
                                        session.hooks.frequency.logic_timer,
                                    ),
                                    frame: timing.frame.clone(),
                                    epoch: timing.epoch,
                                });
                            }
                            FreshRequest::LogicTick(reply) => {
                                let clock = &session.world.clock;
                                let _ = reply.send(crate::time_panel::ClockSample {
                                    ticks: clock.timing[0],
                                    interface_ticks: host.interface_ticks(),
                                    prompt_updates: host.prompt_updates(),
                                    year: 1930 + clock.calendar[2] * 10 + clock.calendar[1],
                                    hour: clock.hour(),
                                    fraction: clock.timing[2],
                                    speed: host.policy.scheduler.iterations,
                                });
                            }
                            FreshRequest::Planning { category, reply } => {
                                let _ = reply.send(crate::build_panel::Context::from_session(
                                    &session, category,
                                ));
                            }
                            FreshRequest::BuildDrag { category, reply } => {
                                let result = (|| {
                                    if session.planning_exits(category)? {
                                        return Ok(None);
                                    }
                                    host.complete_build_drag(&mut session, &assets).map(Some)
                                })();
                                if let Err(error) = &result {
                                    host.fail(error.clone());
                                }
                                let _ = reply.send(result);
                            }
                            FreshRequest::Build {
                                form,
                                submit,
                                reply,
                            } => {
                                let result = (|| {
                                    if submit && host.failure.is_some() {
                                        return Err(
                                            "Stopped session cannot submit construction".into()
                                        );
                                    }
                                    let image = openmt2001::pe::PeImage::parse(&self.executable)?;
                                    let board = self.styled_board(&image)?;
                                    let file =
                                        openmt2001::build_file::BuildFile::parse(&self.build_file)?;
                                    let assets = crate::fresh_city::CommandAssets {
                                        image: &image,
                                        board: &board,
                                        file: &file,
                                    };
                                    session.plan_build(&form, &assets, submit, |event|
                                        Err(format!("paused worker received immediate command effect {event:?}")))
                                })();
                                let _ = reply.send(result);
                            }
                            FreshRequest::Inspect { camera, reply } => {
                                let _ = reply.send(
                                    self.inspect_fresh(&mut session, camera, &mut cache)
                                        .map_err(Into::into),
                                );
                            }
                        }
                    }
                    if let Some(error) = &host.failure {
                        camera_producer::record_exit(&exit_failures, error.clone());
                    }
                }
                if joined {
                    match crate::process_dispatch::custody_from_session(&mut session, timing) {
                        Ok(custody) => {
                            drop(session);
                            return Some(custody);
                        }
                        Err(error) => camera_producer::record_exit(&exit_failures, error),
                    }
                }
                drop(session);
            }
            Err(error) => {
                let _ = tx.send(Err(error));
            }
        }
        None
    }
    pub(super) fn fresh_skirmish(
        &self,
        request: crate::menu::skirmish_route::Request,
        camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.fresh_skirmish_joined(request, camera, cache, None)
    }
    pub(super) fn fresh_skirmish_joined(
        &self,
        request: crate::menu::skirmish_route::Request,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        let directory = request.launch.directory.clone();
        let profile_name = request.launch.participants[0].name.clone();
        let difficulty = request.launch.difficulty;
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let people = openmt2001::people::PeopleModels::load(&image, &index, &self.archive)?;
        let parameters = fs::read_to_string(self.installer.join("parameters/parameter.prm"))
            .map_err(|e| e.to_string())?;
        let timetable = fs::read_to_string(self.installer.join("parameters/timetable.prm"))
            .map_err(|e| e.to_string())?;
        let scripts = self.maps.parent().ok_or("script directory missing")?;
        // Catalog directory spelling is logical Windows data. Resolve its
        // actual supplied filesystem spelling before the Session's direct reads.
        let hub_path =
            crate::campaign_store::original_path(scripts, &format!("{directory}/hub.lua"))?
                .ok_or("selected Skirmish hub missing")?;
        let directory = hub_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .ok_or("selected Skirmish directory is not text")?
            .to_owned();
        // Sample once immediately before actual offline fresh Session construction.
        // Tests may call initialize_skirmish with an injected Policy instead.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as u32;
        let mut session = crate::fresh_city::Session::initialize_joined(
            crate::fresh_city::Inputs {
                tree_locators: Some(
                    openmt2001::trees::Assets::load(&image, &index, &self.archive)?.locators,
                ),
                image: &image,
                board: &board,
                file: &file,
                people,
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                road_routes: fs::read(self.installer.join("Standard/route_smalltable.bin"))
                    .map_err(|e| format!("road route table: {e}"))?,
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                scripts,
                parameters: &parameters,
                era_styles: crate::fresh_city::load_era_styles(&self.installer.join("parameters"))?,
                timetable: &timetable,
                scenario: &directory,
                difficulty,
                selected_character: request.launch.participants[0]
                    .token
                    .ok_or("human token missing")?,
                profile_name: &profile_name,
                character_names: &self.character_names,
            },
            crate::fresh_city::Policy {
                seed,
                auxiliary_prior_bytes: [0; 0x11c],
            },
            Some(request.catalog),
            None,
            false,
            Some(&self.style),
            crate::compatibility::Mode::StrictV14,
            Some(request.launch),
            self.process_runtime.clone(),
            dispatch,
        )?;
        session.chimney_smoke.bind_archive(&index, &self.archive)?;
        let loaded = self.inspect_fresh(&mut session, camera, cache)?;
        Ok((loaded, session))
    }

    pub(super) fn fresh_preview(
        &self,
        setup: crate::singleplayer::Setup,
        camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.fresh_preview_joined(setup, camera, cache, None)
    }
    pub(super) fn fresh_preview_joined(
        &self,
        setup: crate::singleplayer::Setup,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        let (scenario_id, scenario_token) = match setup.preview {
            crate::singleplayer::Preview::Scenario(id) => {
                if let Some(level) = setup.catalog.find(id).map(|i| &setup.catalog.levels[i])
                    && crate::fresh_city::custom::valid_level(level)
                {
                    (
                        id,
                        std::str::from_utf8(level.directory())
                            .map_err(|_| "custom scenario directory is not UTF-8")?
                            .to_owned(),
                    )
                } else {
                    let scenario = crate::scenarios::find(id)?;
                    (id, scenario.token.to_owned())
                }
            }
            // Supplied v1.4 GAMESTART registers ID 400002 as "sandbox";
            // its hub creates the resident city and must run through Session.
            crate::singleplayer::Preview::Sandbox => (400002, "sandbox".to_owned()),
        };
        if setup.catalog.find(scenario_id).is_none() {
            return Err(format!(
                "level {scenario_id} is absent from the selected profile catalog"
            ));
        }
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let people = openmt2001::people::PeopleModels::load(&image, &index, &self.archive)?;
        let parameters = fs::read_to_string(self.installer.join("parameters/parameter.prm"))
            .map_err(|e| e.to_string())?;
        let timetable = fs::read_to_string(self.installer.join("parameters/timetable.prm"))
            .map_err(|e| e.to_string())?;
        let scripts = self.maps.parent().ok_or("script directory missing")?;
        let difficulty = match setup.difficulty {
            crate::singleplayer::Difficulty::Easy => 0,
            crate::singleplayer::Difficulty::Medium => 1,
            crate::singleplayer::Difficulty::Hard => 2,
        };
        let is_custom = setup.catalog.find(scenario_id).is_some_and(|index| {
            crate::fresh_city::custom::valid_level(&setup.catalog.levels[index])
        });
        // Reproducible preview policy. These unspecified prior bytes are not
        // original captured stack data, nor is seed0 the original host seed.
        let input = crate::fresh_city::Inputs {
            tree_locators: Some(
                openmt2001::trees::Assets::load(&image, &index, &self.archive)?.locators,
            ),
            image: &image,
            board: &board,
            file: &file,
            people,
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                &image,
                &index,
                &self.archive,
            )?,
            road_routes: fs::read(self.installer.join("Standard/route_smalltable.bin"))
                .map_err(|e| format!("road route table: {e}"))?,
            transport_models: openmt2001::traffic::models::Models::from_original(
                &image,
                &index,
                &self.archive,
            )?,
            scripts,
            parameters: &parameters,
            era_styles: crate::fresh_city::load_era_styles(&self.installer.join("parameters"))?,
            timetable: &timetable,
            scenario: &scenario_token,
            difficulty,
            selected_character: setup.character_index,
            profile_name: &setup.profile_name,
            character_names: &self.character_names,
        };
        let seed = if is_custom {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "offline launch clock predates Unix epoch")?
                .as_secs() as u32
        } else {
            0
        };
        let policy = crate::fresh_city::Policy {
            seed,
            auxiliary_prior_bytes: [0; 0x11c],
        };
        let mut session = crate::fresh_city::Session::initialize_joined(
            input,
            policy,
            Some(setup.catalog.clone()),
            Some(setup.profile_path.clone()),
            false,
            Some(&self.style),
            self.compatibility,
            None,
            self.process_runtime.clone(),
            dispatch,
        )?;
        session.chimney_smoke.bind_archive(&index, &self.archive)?;
        eprintln!(
            "Fresh city preview: {}, difficulty {difficulty}, {} residents, {} buildings; host seed{seed} / auxiliary prior bytes zero; player roster initialized; live worker enabled",
            scenario_token,
            session.world.ledgers.residents.active_count,
            session.models.bindings.len()
        );
        if is_custom {
            eprintln!(
                "Authored custom offline launch used host seed{seed}; named profile progress retained"
            );
        }
        session.setup = Some(setup);
        let loaded = self.inspect_fresh(&mut session, camera, cache)?;
        Ok((loaded, session))
    }

    #[cfg(test)]
    pub(super) fn saved_preview(
        &self,
        path: &std::path::Path,
        camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.saved_preview_with_selection(path, camera, cache, None)
    }
    /// `Some(None)` represents a menu with no active profile. Tests invoking
    /// saved_preview directly use the unrestricted read-only preview path.
    pub(super) fn saved_preview_with_selection(
        &self,
        path: &std::path::Path,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        selected_root: Option<Option<&std::path::Path>>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.saved_preview_with_selection_joined(path, camera, cache, selected_root, None)
            .map_err(|error| error.to_string())
    }
    pub(super) fn saved_preview_with_selection_joined(
        &self,
        path: &std::path::Path,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        selected_root: Option<Option<&std::path::Path>>,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), crate::load::Failure> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let scripts = self.maps.parent().ok_or("script directory missing")?;
        let catalog = crate::campaign_store::load_catalog(&image, scripts)?;
        let (save, index, [id, _, _, difficulty]) =
            crate::load::refusal::validate(&bytes, image.at(0x545d1c, 40)?, &catalog)?;
        if let Some(process) = &self.process_runtime {
            process.source_load_unjoined().map_err(|e| e.to_string())?;
        }
        let level = &catalog.levels[index];
        let scenario = std::str::from_utf8(level.directory())
            .map_err(|_| "saved scenario directory is not UTF-8")?;
        // Catalog names retain Windows casing; use the supplied directory's
        // spelling for the session's hub, player and AI reads on Linux.
        let scenario_path = crate::campaign_store::original_path(scripts, scenario)?
            .ok_or("saved scenario script directory is missing")?;
        let scenario = scenario_path
            .strip_prefix(scripts)
            .map_err(|_| "saved scenario directory is outside scripts")?
            .to_str()
            .ok_or("saved scenario directory is not UTF-8")?;
        let difficulty =
            i32::try_from(difficulty).map_err(|_| "saved difficulty exceeds signed range")?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let parameters = fs::read_to_string(self.installer.join("parameters/parameter.prm"))
            .map_err(|e| e.to_string())?;
        let timetable = fs::read_to_string(self.installer.join("parameters/timetable.prm"))
            .map_err(|e| e.to_string())?;
        // An arbitrary original save has no authenticated V2 profile selection.
        let mut restored_presentation = Vec::new();
        let mut session = crate::fresh_city::Session::load_original_joined(
            crate::fresh_city::Inputs {
                tree_locators: Some(
                    openmt2001::trees::Assets::load(&image, &index, &self.archive)?.locators,
                ),
                image: &image,
                board: &board,
                file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &self.archive)?,
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                road_routes: fs::read(self.installer.join("Standard/route_smalltable.bin"))
                    .map_err(|e| format!("road route table: {e}"))?,
                scripts,
                parameters: &parameters,
                era_styles: crate::fresh_city::load_era_styles(&self.installer.join("parameters"))?,
                timetable: &timetable,
                scenario,
                difficulty,
                selected_character: 0,
                profile_name: b"Loaded original game",
                character_names: &self.character_names,
            },
            crate::fresh_city::Policy {
                seed: 0,
                auxiliary_prior_bytes: [0; 0x11c],
            },
            &bytes,
            None,
            None,
            Some(&mut restored_presentation),
            Some(&self.style),
            self.process_runtime.clone(),
            dispatch,
        )?;
        session.chimney_smoke.bind_archive(&index, &self.archive)?;
        // Native restore success4bcd74/7b precedes the explicit modern extension.
        // Input/wrapper transition is published only by the later real focus
        // prefix; no reset or synthesized acquisition occurs on this worker.
        crate::process_dispatch::native_load_completed(&mut session)?;
        let metadata = session.restore_v2_extension_with_image(&save, &image)?;
        session.repair_imported_resident_models(self.compatibility, metadata.is_some())?;
        let saved_auction = metadata
            .as_ref()
            .and_then(|metadata| metadata.auction.clone());
        let saved_interface_mode = metadata
            .as_ref()
            .and_then(|metadata| metadata.interface_mode);
        let is_custom = session
            .catalog
            .selected_level()
            .is_some_and(crate::fresh_city::custom::valid_level);
        let filename = metadata.and_then(|metadata| metadata.profile_filename);
        let progress = if is_custom {
            // Custom progress belongs to the actively selected named profile.
            // An imported save's filename cannot choose another profile's writer.
            match selected_root {
                Some(Some(root)) => {
                    crate::save_menu::progress_for_profile_root(&self.profile_root, root)
                }
                Some(None) => None,
                None => filename
                    .as_ref()
                    .map(|name| self.profile_root.join("campaign").join(name)),
            }
        } else {
            filename.and_then(|filename| {
                let profile = self.profile_root.join("campaign").join(filename);
                let owner =
                    crate::save_menu::profile_root_for_campaign(&self.profile_root, &profile);
                selected_root
                    .is_none_or(|selected| selected == owner.as_deref())
                    .then_some(profile)
            })
        };
        if let Some(profile) = progress {
            crate::campaign_store::load_progress(&mut session.catalog, &profile)?;
            if !session.catalog.select(id, false) {
                return Err(crate::load::Failure::UnavailableLevel(id));
            }
            session.campaign_profile = Some(profile);
        }
        let saved_viewpoint = session
            .saved_cameras
            .as_ref()
            .map(normal_saved_city_viewpoint)
            .transpose()?;
        let saved_prior_mode = saved_interface_mode
            .or_else(|| saved_auction.as_ref().map(|auction| auction.prior_mode))
            .or_else(|| {
                (session.saved_logic_mode == Some(2)
                    && session.world.city.auction.controller.reference != 0)
                    .then(|| {
                        u32::from_le_bytes(
                            session.world.city.auction.controller.record[0x150..0x154]
                                .try_into()
                                .unwrap(),
                        )
                    })
            });
        let camera = saved_viewpoint.map_or(camera, |(sample, _)| sample);
        let mut loaded = self.inspect_fresh(&mut session, camera, cache)?;
        loaded.saved_viewpoint = saved_viewpoint;
        loaded.saved_prior_mode = saved_prior_mode;
        loaded.saved_auction = saved_auction;
        loaded.restored_presentation = restored_presentation;
        Ok((loaded, session))
    }

    pub(super) fn ensure_appearance_assets(&self, cache: &mut SnapshotCache) -> Result<(), String> {
        if cache.appearance_assets.is_none() {
            let tab = fs::read(self.installer.join("Standard/fonts/coppgoth23p.tab"))
                .map_err(|e| format!("saved appearance font4: {e}"))?;
            let offsets = fs::read(self.language.join("tycoon.off"))
                .map_err(|e| format!("saved appearance language offsets: {e}"))?;
            let glyphs = fs::read(self.language.join("tycoon.bin"))
                .map_err(|e| format!("saved appearance language glyphs: {e}"))?;
            cache.appearance_assets = Some(
                crate::save_appearance::Assets::from_original(&self.directory, &self.archive)?
                    .with_language(&tab, &offsets, &glyphs)?,
            );
        }
        Ok(())
    }

    pub(super) fn append_static_appearance(
        &self,
        prepared: &std::collections::BTreeMap<
            usize,
            (
                openmt2001::scene::SceneBuilding,
                openmt2001::save::appearance::ModelIdentity,
                u32,
            ),
        >,
        scene: &mut Scene,
        detail_visibility: &mut DetailVisibility,
        cache: &mut SnapshotCache,
        context: StaticAppearanceContext<'_, '_>,
    ) -> Result<(), String> {
        let started = Instant::now();
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let fixed_templates = scene
            .city_structures
            .iter()
            .map(|building| {
                openmt2001::save::appearance::renderer_templates(&file, building.selected, 1)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let instances = prepared
            .values()
            .flat_map(|(building, identity, _)| {
                identity
                    .renderer_templates
                    .iter()
                    .filter(|row| crate::static_detail::supports(row.descriptor))
                    .map(move |row| (building, row))
            })
            .chain(scene.city_structures.iter().zip(&fixed_templates).flat_map(
                |(building, rows)| {
                    rows.iter()
                        .filter(|row| crate::static_detail::supports(row.descriptor))
                        .map(move |row| (building, row))
                },
            ))
            .collect::<Vec<_>>();
        let instance_count = instances.len();
        if instance_count == 0 {
            return Ok(());
        }
        if cache.static_details.is_none() {
            let index = openmt2001::archive::ArchiveIndex::parse(
                &self.directory,
                self.archive.len() as u64,
            )
            .map_err(|error| error.to_string())?;
            cache.static_details = Some(crate::static_detail::Resources::load(
                context.image,
                &index,
                &self.archive,
            )?);
        }
        let load_elapsed = started.elapsed();
        let (draws, spans) = cache.static_details.as_ref().unwrap().draws_with_spans(
            &instances,
            &self.suncols,
            context.hour,
        )?;
        if std::env::var_os("OPENMT2001_PROFILE_SNAPSHOTS").is_some() {
            eprintln!(
                "Live static details: instances={} load={load_elapsed:?} draws={:?} total={:?}",
                instance_count,
                started.elapsed() - load_elapsed,
                started.elapsed()
            );
        }
        scene.vertex_count += draws.iter().map(|draw| draw.vertices.len()).sum::<usize>();
        let first = scene.draws.len();
        scene.draws.extend(draws);
        for span in spans {
            if scene
                .city_structures
                .iter()
                .any(|building| building.block_id == span.category)
            {
                continue;
            }
            let center = context
                .board
                .blocks
                .iter()
                .find(|block| block.id == span.category)
                .ok_or("static detail category missing from board")?
                .center;
            detail_visibility.add(
                scene,
                span.category,
                center,
                first + span.draw,
                span.vertices,
                span.pivot_y,
            )?;
        }
        #[cfg(test)]
        cache.static_detail_ranges.push(first..scene.draws.len());
        scene.diagnostics.push(format!(
            "Static appearance: {} original TM detail instances",
            instance_count,
        ));
        Ok(())
    }

    pub(super) fn appearance_payload(
        session: &crate::fresh_city::Session<'_>,
        record: &[u8; openmt2001::building_pool::RECORD_SIZE],
        assets: &crate::save_appearance::Assets,
    ) -> Result<(u32, crate::save_appearance::RefreshLabels), String> {
        //485520 reads metadata character/name, independently of roster slot.
        let owner = usize::from(record[0x2d]);
        if owner > 6 {
            return Err("appearance owner leaves roster".into());
        }
        let (class, name) = if owner == 0 {
            (10, None)
        } else {
            let metadata = &session.saved_player_metadata[owner - 1];
            let name = &metadata[8..];
            let len = name
                .iter()
                .position(|&byte| byte == 0)
                .ok_or("appearance owner metadata name unterminated")?;
            (
                u32::from_le_bytes(metadata[..4].try_into().unwrap()),
                Some(&name[..len]),
            )
        };
        let business = u32::from_le_bytes(record[0x1c..0x20].try_into().unwrap());
        let kind = if business == u32::MAX {
            -1
        } else {
            let row = session
                .world
                .ledgers
                .businesses
                .get(business as usize)
                .ok_or("appearance building business index outside pool")?;
            i32::from_le_bytes(row[4..8].try_into().unwrap())
        };
        let name_id = if kind == -1 {
            0x35e
        } else {
            let info = session
                .world
                .catalog
                .info
                .rows()
                .get(usize::try_from(kind).map_err(|_| "appearance business subtype is negative")?)
                .ok_or("appearance business subtype outside catalog")?;
            u32::from_le_bytes(info[4..8].try_into().unwrap())
        };
        let mut payload = assets.refresh_labels(name, name_id)?;
        payload.banner = Some(crate::save_appearance::BannerSelection::refreshed(
            class, kind,
        )?);
        Ok((class, payload))
    }

    pub(super) fn append_saved_appearance(
        &self,
        session: &crate::fresh_city::Session<'_>,
        scene: &mut Scene,
        detail_visibility: &mut DetailVisibility,
        cache: &mut SnapshotCache,
    ) -> Result<(), String> {
        use openmt2001::{
            save::{SavedBuilding, appearance, restore},
            scene::SceneBuilding,
        };
        self.ensure_appearance_assets(cache)?;
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let mut prepared = std::collections::BTreeMap::new();
        let mut labels = std::collections::BTreeMap::new();
        for inspection in &scene.inspection_buildings {
            let Some(slot) = inspection.runtime_slot else {
                continue;
            };
            let Some(identity) = session.models.appearance_candidates.get(&slot) else {
                continue;
            };
            let record = &session.world.buildings[slot];
            if record[0x3f] == 0 || !detail_model_at_unit_scale(record) {
                continue;
            }
            let source = SavedBuilding {
                slot,
                category: inspection.category,
                bytes: record[..0x40].try_into().unwrap(),
            };
            let building = SceneBuilding::from_saved(&source, &board, &file)?;
            let current =
                match appearance::model_identity(&file, building.selected, source.height()) {
                    Ok(identity) => identity,
                    Err(error) if error.contains("has no native 4846f0 class") => continue,
                    Err(error) => return Err(error),
                };
            if current != *identity {
                // A later in-game remodel needs a new 485200 binding. The
                // initial saved registry must not be drawn at its stale pose.
                continue;
            }
            let (class, text) = Self::appearance_payload(
                session,
                record,
                cache.appearance_assets.as_ref().unwrap(),
            )?;
            labels.insert(slot, text);
            prepared.insert(slot, (building, identity.clone(), class));
        }
        let mut categories = session.world.categories;
        let mut buildings = session.world.buildings.clone();
        let mut callbacks = Vec::new();
        restore::post_load_appearance_walk(
            &mut categories,
            &mut buildings,
            0x591318,
            |slot, _, _, _| {
                if prepared.contains_key(&slot) {
                    callbacks.push(slot);
                }
                Ok(())
            },
        )?;
        let (details, replacements, spans) = crate::save_appearance::restored_draws_with_spans(
            &prepared,
            &callbacks,
            &image,
            cache.appearance_assets.as_ref().unwrap(),
            &labels,
        )?;
        scene.vertex_count += details
            .iter()
            .map(|draw| draw.vertices.len())
            .sum::<usize>();
        let first = scene.draws.len();
        scene.draws.extend(details);
        for (slot, range, pivot_y) in spans {
            let building = &prepared.get(&slot).ok_or("appearance span slot missing")?.0;
            let center = board
                .blocks
                .iter()
                .find(|block| block.id == building.block_id)
                .ok_or("appearance category missing from board")?
                .center;
            for draw in range {
                let index = first + draw;
                detail_visibility.add(
                    scene,
                    building.block_id,
                    center,
                    index,
                    0..scene.draws[index].vertices.len(),
                    pivot_y,
                )?;
            }
        }
        self.append_static_appearance(
            &prepared,
            scene,
            detail_visibility,
            cache,
            StaticAppearanceContext {
                board: &board,
                image: &image,
                hour: session.world.clock.hour(),
            },
        )?;
        scene.diagnostics.push(format!(
            "Saved appearance: {} prepared buildings, {replacements} native-order detail replacements",
            prepared.len(),
        ));
        Ok(())
    }

    /// Fresh map and newly built models also pass through native 485200 when
    /// the building binds. Draw the already checked constructor classes from
    /// each current model identity; keep unknown classes visible in diagnostics
    /// until their original renderer path is recovered. Saved models that still
    /// match their load-time identity already used the ordered 485520 walk.
    pub(super) fn append_current_appearance(
        &self,
        session: &crate::fresh_city::Session<'_>,
        scene: &mut Scene,
        detail_visibility: &mut DetailVisibility,
        cache: &mut SnapshotCache,
        after_saved_refresh: bool,
    ) -> Result<(), String> {
        use openmt2001::{
            save::{SavedBuilding, appearance},
            scene::SceneBuilding,
        };
        if !scene.inspection_buildings.iter().any(|inspection| {
            inspection.runtime_slot.is_some_and(|slot| {
                session.world.buildings[slot][0x3f] != 0
                    && session.models.bindings.contains_key(&slot)
            })
        }) {
            return Ok(());
        }
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let mut prepared = std::collections::BTreeMap::new();
        let mut callbacks = Vec::new();
        let mut labels = std::collections::BTreeMap::new();
        let mut unsupported = std::collections::BTreeMap::<String, usize>::new();
        for inspection in &scene.inspection_buildings {
            let Some(slot) = inspection.runtime_slot else {
                continue;
            };
            let record = &session.world.buildings[slot];
            if record[0x3f] == 0 || !session.models.bindings.contains_key(&slot) {
                continue;
            }
            // The detail pose is for the completed model. Native 444ff0
            // gives status-4 buildings a unit model matrix even when their
            // auxiliary scale bytes are zero. For pending-world growth wait
            // until the auxiliary scale reaches one, so signs do not float
            // over a small construction model.
            if !detail_model_at_unit_scale(record) {
                continue;
            }
            let source = SavedBuilding {
                slot,
                category: inspection.category,
                bytes: record[..0x40].try_into().unwrap(),
            };
            let building = SceneBuilding::from_saved(&source, &board, &file)?;
            let identity =
                match appearance::model_identity(&file, building.selected, source.height()) {
                    Ok(identity) => identity,
                    Err(error) if error.contains("has no native 4846f0 class") => {
                        *unsupported.entry(error).or_default() += 1;
                        continue;
                    }
                    Err(error) => return Err(error),
                };
            if after_saved_refresh
                && session.models.appearance_candidates.get(&slot) == Some(&identity)
            {
                continue;
            }
            let class = if openmt2001::building_geometry::refresh_appearance_on_bind(record) {
                self.ensure_appearance_assets(cache)?;
                let (class, text) = Self::appearance_payload(
                    session,
                    record,
                    cache.appearance_assets.as_ref().unwrap(),
                )?;
                callbacks.push(slot);
                labels.insert(slot, text);
                class
            } else {
                10
            };
            prepared.insert(slot, (building, identity, class));
        }
        let (details, spans) = if prepared.is_empty() {
            (Vec::new(), Vec::new())
        } else {
            self.ensure_appearance_assets(cache)?;
            let (details, _, spans) = crate::save_appearance::restored_draws_with_spans(
                &prepared,
                &callbacks,
                &image,
                cache.appearance_assets.as_ref().unwrap(),
                &labels,
            )?;
            (details, spans)
        };
        scene.vertex_count += details
            .iter()
            .map(|draw| draw.vertices.len())
            .sum::<usize>();
        let first = scene.draws.len();
        scene.draws.extend(details);
        for (slot, range, pivot_y) in spans {
            let building = &prepared.get(&slot).ok_or("appearance span slot missing")?.0;
            let center = board
                .blocks
                .iter()
                .find(|block| block.id == building.block_id)
                .ok_or("appearance category missing from board")?
                .center;
            for draw in range {
                let index = first + draw;
                detail_visibility.add(
                    scene,
                    building.block_id,
                    center,
                    index,
                    0..scene.draws[index].vertices.len(),
                    pivot_y,
                )?;
            }
        }
        self.append_static_appearance(
            &prepared,
            scene,
            detail_visibility,
            cache,
            StaticAppearanceContext {
                board: &board,
                image: &image,
                hour: session.world.clock.hour(),
            },
        )?;
        scene.diagnostics.push(format!(
            "Current appearance: {} checked models, {} unsupported model classes",
            prepared.len(),
            unsupported.values().sum::<usize>(),
        ));
        for (class, count) in unsupported {
            scene
                .diagnostics
                .push(format!("{count} current models: {class}"));
        }
        Ok(())
    }

    pub(super) fn inspect_fresh(
        &self,
        session: &mut crate::fresh_city::Session<'_>,
        camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<LoadedInspection, String> {
        let started = Instant::now();
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let input_elapsed = started.elapsed();
        let hour = session.world.clock.hour();
        let profile_snapshot = std::env::var_os("OPENMT2001_PROFILE_SNAPSHOTS").is_some();
        let cache_observation = profile_snapshot.then(|| {
            (
                cache.hour,
                cache.city.is_some(),
                cache.buildings.as_slice() == session.world.buildings.as_slice(),
                cache.scaffolds_active,
            )
        });
        let mut reuse_city = false;
        let mut updated_city_draws = Vec::new();
        let mut assembly_elapsed = std::time::Duration::ZERO;
        let mut appearance_elapsed = std::time::Duration::ZERO;
        let mut retained = None;
        if cache.hour == hour
            && let Some(mut city) = cache.city.take()
        {
            if cache.buildings.as_slice() == session.world.buildings.as_slice() {
                reuse_city = true;
            } else if let Some(draws) =
                city.patch_runtime_poses(&cache.buildings, &session.world.buildings)?
            {
                reuse_city = true;
                updated_city_draws = draws;
                cache.buildings = session.world.buildings.to_vec();
            }
            if reuse_city {
                // Move the base out while patching: a pose/metadata error can
                // leave partial writes, and must discard that base. Successful
                // snapshots clone only once for their actor layers.
                updated_city_draws
                    .extend(city.set_street_furniture_era(session.world.clock.calendar[2])?);
                session.refresh_scene_records(&mut city)?;
                if let Some(image) = session.window_texture.current() {
                    updated_city_draws.extend(city.set_window_pixels(image.pixels)?);
                }
                retained = Some(city.clone());
                cache.city = Some(city);
            }
        }
        let mut detail_visibility = if retained.is_some() {
            cache.detail_visibility.clone()
        } else {
            DetailVisibility::default()
        };
        let mut scene = if let Some(scene) = retained {
            scene
        } else {
            #[cfg(test)]
            cache.static_detail_ranges.clear();
            let assembly_started = Instant::now();
            let mut scene = session.scene_with_style_cached(
                &self.build_file,
                OriginalAssets {
                    executable: &self.executable,
                    directory: &self.directory,
                    archive: &self.archive,
                },
                &self.suncols,
                Some(camera),
                true,
                Some(&self.style),
                Some(&mut cache.converted_textures),
            )?;
            assembly_elapsed = assembly_started.elapsed();
            let appearance_started = Instant::now();
            if session.saved_logic_mode.is_some() {
                self.append_saved_appearance(session, &mut scene, &mut detail_visibility, cache)?;
                self.append_current_appearance(
                    session,
                    &mut scene,
                    &mut detail_visibility,
                    cache,
                    true,
                )?;
            } else {
                self.append_current_appearance(
                    session,
                    &mut scene,
                    &mut detail_visibility,
                    cache,
                    false,
                )?;
            }
            appearance_elapsed = appearance_started.elapsed();
            cache.hour = hour;
            cache.buildings = session.world.buildings.to_vec();
            cache.detail_visibility = detail_visibility.clone();
            cache.city = Some(scene.clone());
            scene
        };
        let scaffold_started = Instant::now();
        let has_scaffolds = construction_scaffold::has_active(&session.world.pending_world);
        if has_scaffolds != cache.scaffolds_active {
            // Adding or clearing scaffold geometry changes the retained GPU
            // page layout independently of the cached building geometry.
            reuse_city = false;
        }
        cache.scaffolds_active = has_scaffolds;
        if has_scaffolds && cache.scaffolds.is_none() {
            cache.scaffolds = Some(construction_scaffold::Resources::new(
                &image,
                &index,
                &self.archive,
            )?);
        }
        if let Some(scaffolds) = &cache.scaffolds {
            let range = scaffolds.append(
                &mut scene,
                &session.world.pending_world,
                &session.world.buildings,
                &self.suncols,
                hour,
            )?;
            if has_scaffolds {
                updated_city_draws.extend(range);
            }
        }
        let scaffold_elapsed = scaffold_started.elapsed();
        let city_elapsed = started.elapsed();
        if cache.trees.is_none() {
            cache.trees = Some(std::sync::Arc::new(crate::tree_scene::Resources::load(
                &image,
                &index,
                &self.archive,
                &self.suncols,
            )?));
        }
        let resources = cache.trees.as_ref().unwrap().clone();
        let mut trees = if let Some(state) = &session.models.tree_state {
            let pool = state.borrow();
            if !cache
                .tree_instances
                .as_ref()
                .is_some_and(|(revision, instances)| {
                    *revision == pool.revision() && instances.matches(pool.pool())
                })
            {
                cache.tree_instances =
                    Some((pool.revision(), resources.instances(pool.pool().clone())?));
            }
            crate::tree_scene::Layer::with_instances(
                resources,
                cache.tree_instances.as_ref().unwrap().1.clone(),
                self.compatibility,
            )
        } else {
            crate::tree_scene::Layer::new(resources, self.compatibility)
        };
        trees.attach(&mut scene);
        if cache.people.is_none() {
            cache.people = Some(crate::people_scene::Resources::new(
                session,
                &image,
                &index,
                &self.archive,
            )?);
        }
        let mut people = crate::people_scene::Layer::from_resources(
            session,
            &self.suncols,
            cache.people.as_ref().unwrap(),
        )?;
        people.attach(&mut scene);
        if cache.vehicles.is_none() {
            cache.vehicles = Some(crate::vehicle_scene::Resources::new(
                session,
                &image,
                &index,
                &self.archive,
            )?);
        }
        session
            .road_traffic
            .project(&mut session.world.road_network, 0.)?;
        let mut vehicles = crate::vehicle_scene::Layer::from_resources(
            session,
            &self.suncols,
            cache.vehicles.as_ref().unwrap(),
        )?;
        vehicles.attach(&mut scene);
        if cache.transport.is_none() {
            cache.transport = Some(crate::transport_scene::Resources::new(
                &session.transport_models,
                &image,
                &index,
                &self.archive,
            )?);
        }
        let mut transport_auxiliaries = Vec::new();
        session.advance_transport_frame(|auxiliary| {
            transport_auxiliaries.push(*auxiliary);
            Ok(())
        })?;
        let mut transport = crate::transport_scene::Layer::from_resources(
            session,
            &self.suncols,
            cache.transport.as_ref().unwrap(),
            transport_auxiliaries,
        )?;
        transport.attach(&mut scene);
        let render_elapsed = started.elapsed();
        let city_people = Some(crate::city_people::Report::from_world(
            &session.world,
            &image,
            &session.roster.manager,
        )?);
        let player_info = Some(crate::player_info::State::from_runtime(
            &session.roster.manager,
            &session.world.ledgers.players,
            session.roster.status.current_player,
        )?);
        let comparison = Some(crate::comparison::Data::from_runtime(
            &session.world,
            &player_info.as_ref().unwrap().records,
            &image,
            0x591318,
        )?);
        let mut block_information = crate::block_information::Report::from_runtime(
            &session.world,
            player_info.as_ref().unwrap(),
            &image,
        )?;
        if let Some(report) = &mut block_information {
            report.attach_railroads(
                &session.population.properties,
                session.world.settings.selected_income().daily_freight,
            )?;
        }
        let board = self.styled_board(&image)?;
        let runtime_locator = (session.saved_logic_mode.is_none())
            .then(|| crate::locator::RuntimeInputs::from_world(&session.world));
        let consumer_polls = Some(crate::consumer_polls::Poll::from_runtime(&session.world)?);
        let city_ownership = Some(crate::city_ownership::Ownership::from_runtime(
            &session.world,
            player_info.as_ref().unwrap(),
        )?);
        let city_rights = crate::city_rights::Rights::from_runtime(
            &session.world,
            player_info.as_ref().unwrap(),
        )?;
        let city_prestige = Some(crate::city_prestige::Prestige::from_runtime(
            &session.world,
            &board,
        )?);
        if let Some((previous_hour, had_city, buildings_equal, previous_scaffolds)) =
            cache_observation
        {
            eprintln!(
                "Live snapshot: inputs {input_elapsed:?}, assembly {assembly_elapsed:?}, appearance {appearance_elapsed:?}, scaffold {scaffold_elapsed:?}, city cumulative {:?}, actors/resources host elapsed {:?}, reports host elapsed {:?}, total host elapsed {:?}",
                city_elapsed,
                render_elapsed.saturating_sub(city_elapsed),
                started.elapsed().saturating_sub(render_elapsed),
                started.elapsed()
            );
            eprintln!(
                "Snapshot cache tick{} previous_hour{previous_hour} hour{hour} had_city{had_city} buildings_equal{buildings_equal} previous_scaffolds{previous_scaffolds} scaffolds{has_scaffolds} reuse_city{reuse_city} draws{}; host elapsed durations include waits; not thread CPU time or GPU execution",
                session.world.clock.timing[0],
                scene.draws.len(),
            );
        }
        Ok(LoadedInspection {
            tutorial_session: session.tutorial_state == 1,
            camera_epoch: None,
            camera_publisher: None,
            camera_inbox: None,
            save_profile_root: None,
            logic_tick: Some(session.world.clock.timing[0]),
            construction_active: session
                .world
                .pending_world
                .records
                .iter()
                .any(|row| matches!(row[2], 1 | 2)),
            music_era: Some(session.world.clock.calendar[2]),
            environment_time: Some((session.world.clock.hour(), session.world.clock.timing[2])),
            imported_cameras: session
                .saved_cameras
                .map(openmt2001::camera::bank::Capture::from_records),
            saved_viewpoint: None,
            saved_prior_mode: None,
            saved_auction: None,
            restored_presentation: Vec::new(),
            reuse_city,
            updated_city_draws,
            auction_requests: None,
            auction_audio_requests: None,
            transport_audio_requests: None,
            era_audio_requests: None,
            gameplay_sound_requests: None,
            player_info,
            comparison,
            city_people,
            runtime_locator,
            consumer_polls,
            city_ownership,
            city_rights,
            navigator_owners: crate::navigator::runtime_owners(
                &session.world.categories,
                &session.roster.manager,
                session.roster.status.current_player,
                &image,
            )?,
            block_information,
            city_prestige,
            scene,
            detail_visibility,
            status: None,
            fresh_session: None,
            trees: Some(trees),
            people: Some(people),
            vehicles: Some(vehicles),
            transport: Some(transport),
        })
    }
    pub(super) fn preview_scene(
        &self,
        setup: crate::singleplayer::Setup,
        camera: CameraSample,
    ) -> Result<LoadedInspection, String> {
        eprintln!(
            "Preview setup: {:?}, character {}, profile {}",
            setup.difficulty,
            setup.character_index,
            String::from_utf8_lossy(&setup.profile_name)
        );
        let map = match setup.preview {
            crate::singleplayer::Preview::Scenario(id) => {
                let scenario = crate::scenarios::find(id)?;
                let map = scenario
                    .map
                    .ok_or("Preview unavailable: scenario initialization required")?;
                let path = self.maps.join(map);
                eprintln!(
                    "Static preview source: level {id}, {}, scripts/maps/{map}; hub not executed",
                    scenario.token
                );
                fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?
            }
            crate::singleplayer::Preview::Sandbox => String::new(),
        };
        // Diagnostic phase12 only: this is not a recovered initialization clock.
        render_scene::assemble_with_style(
            Request {
                build_file: &self.build_file,
                source: Source::Map(&map),
                original: Some(OriginalAssets {
                    executable: &self.executable,
                    directory: &self.directory,
                    archive: &self.archive,
                }),
                suncols: Some(&self.suncols),
                phase: Some(12),
                camera: Some(camera),
                width: 1280,
                height: 960,
                animated: true,
            },
            Some(&self.style),
        )
        .map(|scene| LoadedInspection {
            tutorial_session: false,
            camera_epoch: None,
            camera_publisher: None,
            camera_inbox: None,
            save_profile_root: None,
            logic_tick: None,
            construction_active: false,
            music_era: None,
            environment_time: None,
            imported_cameras: None,
            saved_viewpoint: None,
            saved_prior_mode: None,
            saved_auction: None,
            restored_presentation: Vec::new(),
            reuse_city: false,
            updated_city_draws: Vec::new(),
            auction_requests: None,
            auction_audio_requests: None,
            transport_audio_requests: None,
            era_audio_requests: None,
            gameplay_sound_requests: None,
            comparison: None,
            city_people: None,
            runtime_locator: None,
            consumer_polls: None,
            city_ownership: None,
            city_rights: None,
            navigator_owners: Vec::new(),
            player_info: None,
            block_information: None,
            city_prestige: None,
            fresh_session: None,
            trees: None,
            people: None,
            vehicles: None,
            transport: None,
            scene,
            detail_visibility: DetailVisibility::default(),
            status: None,
        })
    }
}
