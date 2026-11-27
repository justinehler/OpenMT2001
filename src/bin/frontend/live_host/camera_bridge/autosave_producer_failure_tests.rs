//! Real Scenario1 producers and Host callbacks, with a controlled
//! one-tick midnight alignment. These are component regressions, not natural
//! rollover, full UI, original-executable, or portable GPU acceptance.
use crate::intro::camera_live::{
    self as live,
    endpoint::{Context, Endpoint, PropertyInstall},
    owner::{Input, Owner, Pose},
};
use crate::{
    fresh_city::{CommandAssets, Session},
    live_host::{CameraSupplierPolicy, Host, camera_bridge, ledger_ui_call},
};
use openmt2001::{
    camera::{
        bank::{Bank, Capture},
        primary::CursorRequest,
    },
    live_commands::Command,
    save::SaveFile,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
const WAIT: Duration = Duration::from_secs(10);
const LIMIT: usize = 12_000;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = PathBuf::from(
            std::env::var_os("OPENMT2001_TEST_SCRATCH")
                .expect("explicit disk scratch parent required"),
        );
        assert!(parent.is_absolute());
        fs::create_dir_all(&parent).unwrap();
        let path = parent.join(format!(
            "autosave-producer-failure-v1-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default, Debug)]
struct CameraAudit {
    completed: Vec<camera_bridge::Boundary>,
    retired_queued: bool,
    before_retire: Option<Capture>,
    after_retire: Option<Capture>,
    raw_pairs: Vec<Capture>,
}
/// Deliberately has no provider or process D product. Only the graph-absent
/// Install, CurrentPose and RawCapture branches are admitted. All actual raw
/// reads/install/retirement remain in the production Endpoint and Owner.
struct CaptureContext {
    expected: camera_bridge::Boundary,
    installing: bool,
}
fn unsupported<T>(name: &str) -> Result<T, String> {
    Err(format!("capture-only fixture cannot service {name}"))
}
impl Context for CaptureContext {
    fn save_interface_mode(&self) -> Result<u32, String> {
        // Explicit controlled normal-view component input, not a UI observation.
        if self.expected != camera_bridge::Boundary::RawCapture {
            return unsupported("view outside RawCapture");
        }
        Ok(0)
    }
    fn validate(&self, operation: &camera_bridge::Operation, _: &Owner) -> Result<(), String> {
        let actual = match operation {
            camera_bridge::Operation::Install { .. } => camera_bridge::Boundary::Install,
            camera_bridge::Operation::CurrentPose => camera_bridge::Boundary::CurrentPose,
            camera_bridge::Operation::RawCapture => camera_bridge::Boundary::RawCapture,
            other => return unsupported(&format!("{other:?}")),
        };
        if actual != self.expected {
            return unsupported("unexpected operation order");
        }
        Ok(())
    }
    fn admission(&mut self, a: &camera_bridge::Admission) -> Result<(), String> {
        if self.expected != camera_bridge::Boundary::Install
            || self.installing
            || a.upper.is_some()
            || a.strategic.is_some()
        {
            return unsupported("noninitial/graph admission");
        }
        self.installing = true;
        Ok(())
    }
    fn take_strategic_writeback(
        &mut self,
    ) -> Result<Option<crate::locator::strategic::Writeback>, String> {
        if !self.installing {
            return unsupported("noninstall writeback");
        }
        self.installing = false;
        Ok(None)
    }
    fn pre_camera(&mut self, _: &mut Owner, _: &camera_bridge::Admission) -> Result<(), String> {
        unsupported("pre_camera")
    }
    fn input(&mut self, _: &camera_bridge::Admission) -> Result<Input, String> {
        unsupported("input")
    }
    fn consumed_input(&mut self) {
        panic!("capture-only consumed_input");
    }
    fn cursor(&mut self, _: Vec<CursorRequest>) -> Result<(), String> {
        unsupported("cursor")
    }
    fn navigate(&mut self, _: &mut Owner) -> Result<(), String> {
        unsupported("navigate")
    }
    fn tutorial(&mut self, _: camera_bridge::NativeOperation, _: &mut Owner) -> Result<(), String> {
        unsupported("tutorial")
    }
    fn pending_sound(&mut self, _: Pose) -> Result<(), String> {
        unsupported("pending_sound")
    }
    fn present(&mut self, _: Pose) -> Result<(), String> {
        unsupported("present")
    }
    fn take_task_effects(&mut self) -> Vec<camera_bridge::TaskEffect> {
        panic!("capture-only task effects");
    }
    fn publish_save_captures(&mut self) {
        panic!("capture-only manual-save publication");
    }
    fn discard_task_effects(&mut self) {
        panic!("unsupported endpoint operation reached cleanup");
    }
}
enum CameraStep {
    Service(camera_bridge::Boundary),
    RetireQueued(mpsc::Receiver<()>),
    Fence(mpsc::Sender<()>),
    Stop,
}
struct CameraThread {
    commands: mpsc::Sender<CameraStep>,
    join: Option<JoinHandle<Result<(), String>>>,
    audit: Arc<Mutex<CameraAudit>>,
    _selection: crate::selection::Assets,
}
impl CameraThread {
    fn start(
        inbox: camera_bridge::Inbox,
        install: PropertyInstall,
        selection: crate::selection::Assets,
    ) -> Self {
        let (commands, receive) = mpsc::channel();
        let audit = Arc::new(Mutex::new(CameraAudit::default()));
        let seen = audit.clone();
        let join = thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(move || {
                let mut endpoint = Endpoint::with_property_install(
                    Owner::new(Bank::loader_zero_storage(), 1),
                    inbox,
                    install,
                );
                let mut context = CaptureContext {
                    expected: camera_bridge::Boundary::Install,
                    installing: false,
                };
                let result = (|| -> Result<bool, String> {
                    loop {
                        match receive.recv_timeout(WAIT).map_err(|e| e.to_string())? {
                            CameraStep::Service(expected) => {
                                context.expected = expected;
                                let before = endpoint.capture();
                                if endpoint.wait_service(&mut context, WAIT)?
                                    != camera_bridge::Service::Completed
                                {
                                    return Err(
                                        "expected actual queued camera request did not arrive"
                                            .into(),
                                    );
                                }
                                if expected != camera_bridge::Boundary::Install {
                                    assert_eq!(
                                        endpoint.capture(),
                                        before,
                                        "read-only operation changed raw pair"
                                    );
                                }
                                let mut a = seen.lock().unwrap();
                                a.completed.push(expected);
                                if expected == camera_bridge::Boundary::RawCapture {
                                    a.raw_pairs.push(endpoint.capture());
                                }
                            }
                            CameraStep::RetireQueued(queued) => {
                                // Notification fires AFTER the real RawCapture Request
                                // send into this Endpoint's Inbox. No wait/sleep is proof.
                                queued.recv_timeout(WAIT).map_err(|e| e.to_string())?;
                                seen.lock().unwrap().before_retire = Some(endpoint.capture());
                                return Ok(true);
                            }
                            CameraStep::Fence(done) => {
                                done.send(()).map_err(|e| e.to_string())?;
                            }
                            CameraStep::Stop => return Ok(false),
                        }
                    }
                })();
                // Sole production retirement path; it invalidates live sources and
                // calls Inbox::retire while the genuine Request is still queued.
                let owner = endpoint.into_owner();
                let mut a = seen.lock().unwrap();
                a.after_retire = Some(owner.capture());
                a.retired_queued = *result.as_ref().unwrap_or(&false);
                result.map(|_| ())
            })
            .unwrap();
        Self {
            commands,
            join: Some(join),
            audit,
            _selection: selection,
        }
    }
    fn service(&self, boundary: camera_bridge::Boundary) {
        self.commands.send(CameraStep::Service(boundary)).unwrap();
    }
    fn retire_queued(&self, queued: mpsc::Receiver<()>) {
        self.commands
            .send(CameraStep::RetireQueued(queued))
            .unwrap();
    }
    fn fence(&self) {
        let (done, wait) = mpsc::channel();
        self.commands.send(CameraStep::Fence(done)).unwrap();
        wait.recv_timeout(WAIT).unwrap();
    }
    fn finish(&mut self) {
        let _ = self.commands.send(CameraStep::Stop);
        if let Some(join) = self.join.take() {
            join.join().unwrap().unwrap();
        }
    }
}
impl Drop for CameraThread {
    fn drop(&mut self) {
        let _ = self.commands.send(CameraStep::Stop);
        if let Some(join) = self.join.take() {
            let result = join.join();
            if !thread::panicking() {
                result.unwrap().unwrap();
            }
        }
    }
}

#[derive(Default, Debug)]
struct AuctionAudit {
    prepare: usize,
    activate: usize,
    restore: usize,
}
struct AuctionThread {
    sender: mpsc::Sender<crate::auction_bridge::Request>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    audit: Arc<Mutex<AuctionAudit>>,
}
impl AuctionThread {
    fn start(exe: Arc<Vec<u8>>, directory: Arc<Vec<u8>>, archive: Arc<Vec<u8>>) -> Self {
        let (sender, receive) = mpsc::channel::<crate::auction_bridge::Request>();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let audit = Arc::new(Mutex::new(AuctionAudit::default()));
        let seen = audit.clone();
        let join = thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(move || {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer =
                    pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                let mut view = crate::auction_bridge::View::default();
                while !stopped.load(Ordering::Acquire) {
                    let request = match receive.recv_timeout(Duration::from_millis(100)) {
                        Ok(request) => request,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    view.service_actors(&renderer).unwrap();
                    let which = match &request.operation {
                        crate::auction_bridge::Operation::Prepare { .. } => 0,
                        crate::auction_bridge::Operation::Activate => 1,
                        crate::auction_bridge::Operation::Restore => 2,
                    };
                    // Actual prepared scene, actor owner, transition state and Restore.
                    // No constant Ok reply substitutes for any operation.
                    let result = view.present(
                        request.operation,
                        &renderer,
                        (&exe, &directory, &archive),
                        0,
                    );
                    if result.is_ok() {
                        let mut a = seen.lock().unwrap();
                        match which {
                            0 => a.prepare += 1,
                            1 => a.activate += 1,
                            2 => a.restore += 1,
                            _ => unreachable!(),
                        }
                    }
                    request.reply.send(result).unwrap();
                }
            })
            .unwrap();
        Self {
            sender,
            stop,
            join: Some(join),
            audit,
        }
    }
}
impl Drop for AuctionThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let result = join.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}

fn prompts(host: &mut Host, session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) {
    for prompt in host.take_prompts(session) {
        if let Some(dialog) = prompt.dialog {
            let button = match (dialog.callback, dialog.button_count) {
                // Actual purchase-offer refusal and takeover acknowledgement.
                (0x48b350, 2) => 1,
                (0x48b480, 1) | (0, 1) => 0,
                other => panic!(
                    "unbound actual UI callback {other:?}; inspect fixture, never admit by default"
                ),
            };
            host.human_dialog_reply(
                session,
                assets,
                crate::human_dialog::Reply {
                    callback: dialog.callback,
                    context: dialog.context,
                    button,
                },
            )
            .unwrap();
        } else {
            // Genuine InterfacePrompt producer; preserve its actual dismissal ID.
            session
                .dispatch_session_event(
                    "InterfacePromptDismiss",
                    prompt.dismissal_id as i32,
                    assets,
                    true,
                )
                .unwrap();
        }
    }
}
fn tick(host: &mut Host, session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) {
    assert!(host.running);
    assert!(!session.world.pause.has_reason());
    assert_eq!(host.policy.scheduler.iterations, 1);
    host.tick(session, assets);
    assert!(host.failure.is_none(), "{:?}", host.failure);
    prompts(host, session, assets);
}
fn frame(host: &mut Host, session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) {
    host.frame(session, assets, 40);
    assert!(host.failure.is_none(), "{:?}", host.failure);
    prompts(host, session, assets);
}
fn closure_due(session: &Session<'_>) -> Option<u32> {
    session.live_commands.snapshot().unwrap().iter().find_map(
        |(domain, outer, inner, direct, c)| {
            if let Command::Auction { descriptor } = c
                && word(descriptor, 4) == 0x20
            {
                assert!(matches!(domain, openmt2001::scheduler::ClockDomain::Total));
                assert_eq!(outer, inner);
                assert!(!direct);
                assert_eq!(word(descriptor, 0), 0x525c68);
                assert_eq!(descriptor[16], 1);
                Some(*outer)
            } else {
                None
            }
        },
    )
}
/// Only these controlled writes align the independently produced queue event
/// with a midnight calendar callback. Total tick, simulation mode, auction owner,
/// controller and queue deadlines remain untouched.
fn align_midnight(session: &mut Session<'_>, host: &mut Host) {
    let next_normal_hour = (session.world.clock.timing[1] / 625 + 1) * 625;
    session.world.clock.timing[1] = next_normal_hour - 1;
    session.world.clock.timing[2] = 624;
    session.world.clock.timing[3] = 14999;
    session.world.clock.calendar = [23, 0, 0, 23, 0];
    session.world.settings.tweaks[4..8].copy_from_slice(&1u32.to_le_bytes());
    host.policy.hourly.suppressed = true;
    assert_eq!(host.policy.scheduler.iterations, 1);
    assert!(!host.policy.scheduler.network_enabled);
    assert!(!session.world.pause.has_reason());
}
fn install_camera(
    session: &mut Session<'_>,
    host: &mut Host,
    assets: &CommandAssets<'_, '_>,
    executable: &[u8],
    directory: &[u8],
    archive: &[u8],
    capture: Capture,
) -> CameraThread {
    let index = openmt2001::archive::ArchiveIndex::parse(directory, archive.len() as u64).unwrap();
    let selection = crate::selection::Assets::load(executable, &index, archive).unwrap();
    let (worker, inbox) = camera_bridge::channel(session.world.topology.epoch().serial());
    let property = PropertyInstall::capture(&selection, assets.image).unwrap();
    let handle = selection.pavement_resource().binding().unwrap().handle();
    let ui = CameraThread::start(inbox, property, selection);
    ui.service(camera_bridge::Boundary::Install);
    host.install_camera_bridge(
        session,
        assets.image,
        CameraSupplierPolicy::CameraOnly,
        worker,
        capture,
        handle,
    )
    .unwrap();
    assert!(host.camera_bridge_installed());
    ui
}

#[test]
#[ignore = "supplied original assets and working wgpu adapter; controlled real auction closure/Host cut"]
fn completed_auction_closure_gate_survives_same_tick_queued_raw_capture_retirement() {
    run(true);
}
#[test]
#[ignore = "supplied original assets; actual rejected auction request allocates before Host autosave"]
fn rejected_busy_auction_producer_saves_live_message_allocator_not_stale_session_cursor() {
    run(false);
}
fn run(retirement: bool) {
    thread::Builder::new().stack_size(32*1024*1024).spawn(move || {
        let started=Instant::now(); let scratch=Scratch::new();
        let read=|key|fs::read(std::env::var(key).unwrap()).unwrap();
        let exe=Arc::new(read("OPENMT2001_EXE")); let image=openmt2001::pe::PeImage::parse(&exe).unwrap();
        let board=openmt2001::board::Board::from_v14(&image).unwrap();
        let build=read("OPENMT2001_BUILD_FILE"); let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let directory=Arc::new(read("OPENMT2001_ARCHIVE_DIR")); let archive=Arc::new(read("OPENMT2001_ARCHIVE_BIN"));
        let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
        let parameters=read("OPENMT2001_PARAMETERS"); let timetable=read("OPENMT2001_TIMETABLE");
        let default=PathBuf::from(std::env::var("OPENMT2001_DEFAULT_SETTINGS").unwrap());
        let scripts=default.parent().unwrap().parent().unwrap();
        let styles=crate::fresh_city::load_era_styles(Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap()).parent().unwrap()).unwrap();
        let exe_path=PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let names=crate::fresh_city::character_names(&image,&exe_path.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
        let inputs=crate::fresh_city::Inputs { tree_locators:None,image:&image,board:&board,file:&file,
            people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
            vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
            transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
            road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts,parameters:std::str::from_utf8(&parameters).unwrap(),
            era_styles:styles,timetable:std::str::from_utf8(&timetable).unwrap(),scenario:"SCENARIO1",difficulty:0,
            selected_character:4,profile_name:b"Autosave producer cut",character_names:&names };
        let mut session=Session::initialize(inputs,crate::fresh_city::Policy{seed:0x12345678,auxiliary_prior_bytes:[0;284]}).unwrap();
        assert_eq!(session.roster.manager.players.len(),4);
        let sample=openmt2001::flyby::CameraSample {eye:[100.,120.,100.],target:[0.;3],up:[0.,1.,0.],vertical_fov:0.7};
        // Test-created valid raw pair, explicitly not an imported-save/UI owner claim.
        let capture=Capture::from_records(crate::save_menu::cameras(None,sample).unwrap());
        // Declare renderer guard first so the Host sender drops before it on unwind.
        let renderer=retirement.then(||AuctionThread::start(exe.clone(),directory.clone(),archive.clone()));
        let mut host=Host::new(sample.eye); host.begin_presentation(&session).unwrap(); host.running=true;
        if let Some(renderer)=&renderer {host.auction_renderer=Some(renderer.sender.clone());}
        if retirement {
            let mut bid=false; let mut due=None;
            for _ in 0..LIMIT {
                assert!(started.elapsed()<Duration::from_secs(240),"bounded fixture budget exhausted");
                frame(&mut host,&mut session,&assets(&image,&board,&file));
                let controls=host.auction.as_ref().map(|a|&a.controls);
                if !bid && controls.is_some_and(|c|c.timer_visible && c.buttons_enabled&1!=0) {
                    host.auction_button(&mut session,&assets(&image,&board,&file),0).unwrap(); bid=true;
                } else if bid && controls.is_some_and(|c|c.buttons_enabled&(1<<3)!=0) {
                    host.auction_button(&mut session,&assets(&image,&board,&file),3).unwrap();
                }
                if let Some(at)=closure_due(&session) {due=Some(at);break;}
            }
            assert!(bid,"fixture never reached the real local bidder");
            let due=due.expect("real progress producer never queued closure20; do not manufacture it");
            assert!(due>session.world.clock.timing[0]);
            while session.world.clock.timing[0]+1<due {tick(&mut host,&mut session,&assets(&image,&board,&file));}
            assert_eq!(session.world.clock.timing[5],2);
            assert!(host.auction.as_ref().is_some_and(|a|a.visible));
            assert!(host.auction_actor_events.is_some());
            // Real title-right callback5 writes the true gate and actor event.
            if !host.policy.ai_event_gate {host.auction_button(&mut session,&assets(&image,&board,&file),5).unwrap();}
            assert!(host.policy.ai_event_gate);
            let category=word(&session.world.auction.controller.record,4) as usize;
            assert_eq!(session.world.auction.controller.active_count,1);
            assert_ne!(session.world.categories[category][0x6c],0);
            let root=crate::save_menu::selected_profile_root(&scratch.0,b"Retirement");
            fs::create_dir_all(root.join("saves")).unwrap();
            let slot=root.join("saves/autosave.sav");fs::write(&slot,b"retained prior slot").unwrap();
            host.autosave_profile_root=Some(root);
            let mut ui=install_camera(&mut session,&mut host,&assets(&image,&board,&file),&exe,&directory,&archive,capture);
            align_midnight(&mut session,&mut host);
            // Seed a genuine production money UI pulse to detect later Animations.
            ledger_ui_call(&mut host.cash_arrow,&host.gameplay_sound,
                openmt2001::category_ownership::Seam::NativeUiCall{address:0x4c6290,args:[0;6],argument_count:0}).unwrap();
            let cash=host.cash_arrow; assert!(cash.active);
            let (notify,queued)=mpsc::channel();
            host.camera_bridge.as_mut().unwrap().test_notify_enqueue_at(camera_bridge::Boundary::RawCapture,notify);
            ui.service(camera_bridge::Boundary::CurrentPose); ui.retire_queued(queued);
            let normal=session.world.clock.timing[1];
            host.tick(&mut session,&assets(&image,&board,&file)); ui.finish();
            assert!(host.failure.is_none(),"{:?}",host.failure);
            assert!(!host.running); assert!(host.camera_retired());
            assert!(!host.policy.ai_event_gate,"completed RestorePresentation must commit its local bid_gate");
            assert!(host.auction.is_none()); assert!(host.auction_actor_events.is_none());
            assert_eq!(session.world.auction.controller.active_count,0);
            assert_eq!(&session.world.auction.controller.record[0x7c..0x7e],&[0,0]);
            assert_eq!(session.world.categories[category][0x6c],0);
            assert_eq!(session.world.auction.participant_ui,[0;7]);assert_eq!(session.world.auction.bid_ui,[0;7]);
            let rendered=renderer.as_ref().unwrap().audit.lock().unwrap();
            assert_eq!((rendered.prepare,rendered.activate,rendered.restore),(1,1,1));drop(rendered);
            assert_eq!(session.world.clock.timing[0],due);
            assert_eq!(session.world.clock.timing[1],normal+1);
            assert_eq!(session.world.clock.calendar,[0,1,0,23,1],"post-Autosave absolute-hour store must not execute");
            assert_eq!(host.cash_arrow,cash,"later animation tail must stop at retirement");
            assert!(host.autosave_status.is_none(),"typed retirement is neither a saved slot nor recoverable save failure");
            assert_eq!(fs::read(&slot).unwrap(),b"retained prior slot");
            assert_eq!(fs::read_dir(slot.parent().unwrap()).unwrap().count(),1,"staged temp must be removed");
            assert!(closure_due(&session).is_none(),"completed closure queue node must stay consumed");
            assert!(session.live_commands.snapshot().unwrap().iter().any(|(_,outer,_,_,c)|
                matches!(c,Command::Auction{descriptor} if word(descriptor,4)==0x16) && *outer>due),
                "real closure's delayed ownership transfer must survive");
            let a=ui.audit.lock().unwrap();assert!(a.retired_queued);
            assert_eq!(a.completed,[camera_bridge::Boundary::Install,camera_bridge::Boundary::CurrentPose]);
            assert_eq!(a.before_retire,Some(capture));assert_eq!(a.after_retire,Some(capture));drop(a);
            let stopped=session.world.clock;
            host.tick(&mut session,&assets(&image,&board,&file));
            assert_eq!(session.world.clock,stopped);assert!(host.failure.is_none());assert!(!host.running);
        } else {
            for _ in 0..3000 {
                frame(&mut host,&mut session,&assets(&image,&board,&file));
                if session.world.auction.controller.active_count!=0 {break;}
            }
            assert_eq!(session.world.auction.controller.active_count,1,"natural Scenario1 did not admit an auction");
            assert_eq!(session.world.clock.timing[5],1,"require announcement before scene activation");
            let category=word(&session.world.auction.controller.record,4);
            let local=session.roster.manager.players.iter().position(|p|p.reference==session.roster.status.current_player).unwrap();
            assert_ne!(session.roster.manager.players[local].bytes[0x10],0);
            assert_ne!(word(&session.roster.manager.players[local].bytes,0x154),1);
            // Use the actual accepted-offer descriptor producer, with an actual
            // category/local recipient. Admission will reject it as busy and
            // Context::produce will allocate its literal48-byte kind26 packet.
            let descriptor=openmt2001::ai_messages::offers::negotiation::request([
                category,local as u32,100,category,0,0,0,0],[0;52]);
            let due=session.world.clock.timing[0]+25;
            session.submit_command(Command::Auction{descriptor},&assets(&image,&board,&file),
                |_|Err("production request unexpectedly dispatched synchronously".into())).unwrap();
            while session.world.clock.timing[0]+1<due {tick(&mut host,&mut session,&assets(&image,&board,&file));}
            assert!(session.live_commands.snapshot().unwrap().iter().any(|(_,outer,inner,direct,c)|
                *outer==due && *inner==due && !direct && matches!(c,Command::Auction{descriptor:d} if *d==descriptor)));
            let root=crate::save_menu::selected_profile_root(&scratch.0,b"Producer");
            let slot=root.join("saves/autosave.sav");host.autosave_profile_root=Some(root);
            let mut ui=install_camera(&mut session,&mut host,&assets(&image,&board,&file),&exe,&directory,&archive,capture);
            align_midnight(&mut session,&mut host);
            let stale=session.allocation_next;
            assert!(session.roster.manager.players.iter().all(|p|p.auxiliaries.iter().all(|a|a.reference!=stale)));
            ui.service(camera_bridge::Boundary::CurrentPose);ui.service(camera_bridge::Boundary::RawCapture);
            host.tick(&mut session,&assets(&image,&board,&file));
            assert!(host.failure.is_none(),"{:?}",host.failure);assert!(host.running);
            assert!(matches!(&host.autosave_status,Some(crate::save_menu::AutosaveStatus::Saved(path)) if *path==slot));
            let bytes=fs::read(&slot).unwrap();let save=SaveFile::parse(&bytes).unwrap();
            assert_eq!(save.clock().unwrap().timing[0],due,"producer and capture must share this actual Host tick");
            assert_eq!(save.cameras().unwrap().map(|c|*c),capture.into_records());
            let extension=openmt2001::save::extension::Extension::parse(&save).unwrap().unwrap();
            assert_eq!(word(extension.required(3).unwrap(),32),stale+0x30,
                "one actual pre-cut allocation must replace the stale Session cursor");
            assert_ne!(word(extension.required(3).unwrap(),32),stale);
            // Independently locate the actual allocation in the saved roster,
            // with literal descriptor/recipient/category checks. This cannot be
            // satisfied by a helper that simply changes allocation_next.
            let runtime=saved_players(extension.required(2).unwrap());
            let packet=runtime[local].iter().find(|(reference,_)|*reference==stale)
                .expect("actual busy-request inbox allocation absent at snapshot").1;
            assert_eq!(word(&packet,0),0x525dc0);assert_eq!(word(&packet,4),0x26);assert_eq!(word(&packet,8),0x30);
            assert_eq!(packet[0x14],0xff);assert_eq!(usize::from(packet[0x15]),local);
            assert_eq!(word(&packet,0x18),category);
            assert_eq!(save.clock().unwrap().calendar,[0,1,0,23,1]);
            assert_eq!(session.world.clock.calendar,[0,1,0,24,1]);
            assert!(session.allocation_next>=stale+0x30,"completed allocation must survive later tick work");
            assert_eq!(session.world.auction.controller.active_count,1,"busy reply cannot replace the actual auction");
            ui.fence(); // real service completion, not an ack/scheduler race.
            let a=ui.audit.lock().unwrap();assert_eq!(a.raw_pairs,[capture]);drop(a);
            ui.finish();
        }
    }).unwrap().join().unwrap();
}
fn assets<'a, 'b>(
    image: &'a openmt2001::pe::PeImage<'b>,
    board: &'a openmt2001::board::Board,
    file: &'a openmt2001::build_file::BuildFile<'b>,
) -> CommandAssets<'a, 'b> {
    CommandAssets { image, board, file }
}

/// Read only the documented record2 envelope; use literal48-byte auxiliary rows
/// rather than invoking a second encoder or manufacturing a live Arena cursor.
fn saved_players(mut bytes: &[u8]) -> Vec<Vec<(u32, [u8; 48])>> {
    fn take<'a>(bytes: &mut &'a [u8], n: usize) -> &'a [u8] {
        let (head, tail) = bytes.split_at(n);
        *bytes = tail;
        head
    }
    fn next(bytes: &mut &[u8]) -> u32 {
        u32::from_le_bytes(take(bytes, 4).try_into().unwrap())
    }
    let count = next(&mut bytes);
    assert_eq!(count, 4);
    take(&mut bytes, 40);
    next(&mut bytes); // manager header and current player.
    let result = (0..count)
        .map(|_| {
            next(&mut bytes); // actual player identity.
            let size = next(&mut bytes) as usize;
            take(&mut bytes, size);
            let count = next(&mut bytes);
            (0..count)
                .map(|_| {
                    let reference = next(&mut bytes);
                    (reference, take(&mut bytes, 48).try_into().unwrap())
                })
                .collect()
        })
        .collect();
    assert!(bytes.is_empty());
    result
}
