//! Actual session_worker_with_focus boundary, private supplied inputs, CPU only.
//! No GPU/device/original execution. Literal expected no-advance boundary and
//! a saved native clock make moving the real selector below frame observable.
use super::*;
use std::{cell::RefCell, rc::Rc, thread::JoinHandle, time::Duration};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = PathBuf::from(std::env::var_os("OPENMT2001_TEST_SCRATCH").expect("explicit disk scratch parent"));
        assert!(parent.is_absolute()); fs::create_dir_all(&parent).unwrap();
        let root = parent.join(format!("s4-focus-worker-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap(); Self(root)
    }
}
impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

fn inputs(profile_root: PathBuf) -> Arc<SceneInputs> {
    let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
    let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
    let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
    let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
    let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
    let style = crate::block_sidewalk_labels::load_style(&image, &installer).unwrap();
    let names = crate::fresh_city::character_names(&image, &language).unwrap();
    Arc::new(SceneInputs { process_runtime: None, compatibility: crate::compatibility::Mode::StrictV14,
        style, time_runtime: Default::default(), profile_root, character_names: names, installer, language,
        maps: map.parent().unwrap().to_path_buf(), build_file: read("OPENMT2001_BUILD_FILE"),
        executable, directory: read("OPENMT2001_ARCHIVE_DIR"), archive: read("OPENMT2001_ARCHIVE_BIN"),
        suncols: read("OPENMT2001_SUNCOLS") })
}
struct Effects { active: Option<focus::AttachmentId>, calls: Rc<RefCell<Vec<(u64, bool)>>>, fail_loss: bool }
impl focus::Effects for Effects {
    fn activate(&mut self, id: focus::AttachmentId) -> Result<(), String> {
        assert!(self.active.is_none()); self.active = Some(id); Ok(())
    }
    fn retire(&mut self, id: focus::AttachmentId) -> Result<(), String> {
        assert_eq!(self.active, Some(id)); self.active = None; Ok(())
    }
    fn before_pause(&mut self, id: focus::AttachmentId, focused: bool, _: i8) -> Result<focus::Prefix, String> {
        assert_eq!(self.active, Some(id)); self.calls.borrow_mut().push((id.value(), focused));
        if !focused && self.fail_loss { return Err("fixture prefix failed before pause slice".into()); }
        Ok(focus::Prefix::Complete(Some(openmt2001::native_input::focus::AudioOutcome::Applied)))
    }
}
struct Running {
    route: focus::Route,
    sender: Option<mpsc::Sender<FreshRequest>>,
    loaded: Option<LoadedInspection>,
    worker: Option<JoinHandle<()>>,
}
impl Running {
    fn send(&self, request: FreshRequest) { self.sender.as_ref().unwrap().send(request).unwrap(); }
    fn clock(&self) -> crate::time_panel::ClockSample {
        let (reply, answer) = mpsc::channel(); self.send(FreshRequest::LogicTick(reply));
        answer.recv_timeout(Duration::from_secs(15)).unwrap()
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.route.detach(); self.sender.take(); self.loaded.take();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

fn worker_case(fail_loss: bool) {
    let scratch = Scratch::new(); let inputs = inputs(scratch.0.join("profiles"));
    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    let catalog = crate::campaign_store::load_catalog(&image, inputs.maps.parent().unwrap()).unwrap();
    let setup = crate::singleplayer::Setup { preview: crate::singleplayer::Preview::Scenario(200003),
        difficulty: crate::singleplayer::Difficulty::Easy, character_index: 2,
        profile_name: b"Focus worker boundary".to_vec(), catalog,
        profile_path: crate::campaign_store::profile_path(&inputs.profile_root, b"Focus worker boundary") };
    let (camera, _) = fresh_city_camera(); let (tx, rx) = mpsc::channel(); let source = Arc::clone(&inputs);
    let worker = std::thread::Builder::new().stack_size(16 * 1024 * 1024)
        .spawn(move || source.session_worker_with_focus(WorkerStart::Fresh(setup), camera, tx, None, true)).unwrap();
    let loaded = rx.recv_timeout(Duration::from_secs(30)).unwrap().unwrap();
    let initial_tick = loaded.logic_tick.unwrap(); let sender = loaded.fresh_session.as_ref().unwrap().clone();
    let calls = Rc::new(RefCell::new(Vec::new())); let mut route = focus::Route::default();
    route.install(Box::new(Effects { active: None, calls: Rc::clone(&calls), fail_loss })).unwrap();
    route.observe(true).unwrap();
    let mut run = Running { route, sender: Some(sender), loaded: Some(loaded), worker: Some(worker) };
    std::thread::sleep(Duration::from_millis(45)); // first dispatch must stay gated before snapshot arrives
    run.route.attach(run.sender.as_ref().unwrap().clone()).unwrap();
    run.route.wait_prefix();
    let (reply, gated) = mpsc::channel(); run.send(FreshRequest::LogicTick(reply));
    assert!(matches!(gated.try_recv(), Err(mpsc::TryRecvError::Empty)), "no request passes incomplete cold attachment");
    run.route.service_one().unwrap();
    assert_eq!(gated.recv_timeout(Duration::from_secs(15)).unwrap().ticks, initial_tick);
    run.send(FreshRequest::Run(false)); let baseline = run.clock();
    run.send(FreshRequest::Run(true)); run.route.observe(true).unwrap(); run.route.wait_prefix();
    // Worker is blocked in a received ordinary gain. Queue loss then actual
    // SaveSelected/LogicTick in its real FIFO, with a due frame waiting behind.
    run.route.observe(false).unwrap();
    let (reply, saved) = mpsc::channel();
    run.send(FreshRequest::SaveSelected { target: crate::save_menu::Target::NewName("focus-boundary".into()),
        camera, capture: None, game_mode: 1, interface_mode: 0, reply });
    let (reply, after) = mpsc::channel(); run.send(FreshRequest::LogicTick(reply));
    std::thread::sleep(Duration::from_millis(45));
    run.route.service_one().unwrap(); // complete preceding gain
    run.route.wait_prefix(); // loss now admitted at actual worker boundary
    let result = run.route.service_one();
    if fail_loss {
        assert!(result.is_err());
        assert!(matches!(saved.recv_timeout(Duration::from_secs(15)), Err(mpsc::RecvTimeoutError::Disconnected)),
            "failed prefix closes subsequent save reply, never timeout-as-success");
        assert!(matches!(after.recv_timeout(Duration::from_secs(15)), Err(mpsc::RecvTimeoutError::Disconnected)),
            "failed prefix closes subsequent command reply, never timeout-as-success");
    } else {
        result.unwrap();
        let path = saved.recv_timeout(Duration::from_secs(15)).unwrap().unwrap();
        let bytes = fs::read(path).unwrap(); let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
        assert_eq!(save.clock().unwrap().timing[0], baseline.ticks, "received loss precedes already-due frame and actual save");
        let after = after.recv_timeout(Duration::from_secs(15)).unwrap();
        assert_eq!(after.ticks, baseline.ticks, "no stale-focus logic preflush/replay");
        assert!(after.interface_ticks > baseline.interface_ticks, "a real due interface frame ran");
        let id = run.route.attachment_id(); let count = calls.borrow().len();
        let (reply, refreshed) = mpsc::channel(); run.send(FreshRequest::Inspect { camera, reply });
        drop(refreshed.recv_timeout(Duration::from_secs(30)).unwrap().unwrap());
        assert_eq!(run.route.attachment_id(), id); assert_eq!(calls.borrow().len(), count, "warm worker snapshot does not reattach focus");
        run.route.poll().unwrap();
    }
    assert_eq!(&*calls.borrow(), &[(1, true), (1, true), (1, false)]);
    drop(run); // revoke effects, close all leases, join worker before scratch cleanup
}

#[test]
#[ignore = "requires private original assets/scripts and explicit disk scratch; actual CPU worker boundary"]
fn actual_worker_focus_due_frame_save_failure_and_refresh_boundary() {
    worker_case(false);
    worker_case(true);
}
