//! CPU Host/protocol acceptance; native UI is exercised separately.
//! Natural gameplay only: no assignment to clocks, economy, ownership, RNG,
//! pause, success state, queues, card pools, hourly gates or mode.
use super::*;
use crate::{
    fresh_city::{CommandAssets, Session},
    live_host::{Host, camera_bridge as bridge},
    session_save::natural_observer as probe,
};
use camera_live::{
    endpoint::{Context, Endpoint, PropertyInstall},
    owner::{Input, Owner, Pose},
};
use openmt2001::camera::{
    bank::{Bank, Capture},
    primary::CursorRequest,
};
use std::{
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
const FRAMES: usize = 30_000;
const WALL: Duration = Duration::from_secs(240);
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
struct Budget {
    start: Instant,
    used: usize,
}
impl Budget {
    fn tick(&mut self, h: &mut Host, s: &mut Session<'_>, a: &CommandAssets<'_, '_>) {
        assert!(
            self.used < FRAMES && self.start.elapsed() < WALL,
            "single30k/240s budget exhausted at {} {:?}",
            self.used,
            s.world.clock
        );
        self.used += 1;
        assert_eq!(h.policy.scheduler.iterations, 1);
        assert!(!h.policy.hourly.suppressed);
        assert!(!s.world.pause.has_reason(), "unexpected real pause");
        h.tick(s, a);
        assert!(h.failure.is_none(), "actual Host failure: {:?}", h.failure);
        assert!(h.running && !h.camera_retired());
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let parent = PathBuf::from(
            std::env::var_os("OPENMT2001_TEST_SCRATCH")
                .expect("explicit disk scratch parent required"),
        );
        assert!(parent.is_absolute());
        fs::create_dir_all(&parent).unwrap();
        let root = parent.join(format!(
            "autosave-natural-host-v1-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[derive(Default, Debug)]
struct Audit {
    install: usize,
    current: usize,
    raw: usize,
    mode_reads: usize,
    pairs: Vec<[[u8; 196]; 2]>,
    dirty: Vec<u8>,
}
/// Finite capture-only context. Install stores no UI input/provider state;
/// admission and final writeback validate the real graph-absent contract.
/// Its view comes from genuine restored metadata, not a constant normal view.
struct ReadOnlyContext {
    mode: u32,
    installing: bool,
    audit: Arc<Mutex<Audit>>,
}
fn unsupported<T>(name: &str) -> Result<T, String> {
    Err(format!("capture-only context cannot service {name}"))
}
impl Context for ReadOnlyContext {
    fn save_interface_mode(&self) -> Result<u32, String> {
        self.audit.lock().unwrap().mode_reads += 1;
        Ok(self.mode)
    }
    fn validate(&self, op: &bridge::Operation, _: &Owner) -> Result<(), String> {
        let mut audit = self.audit.lock().unwrap();
        match op {
            bridge::Operation::Install { .. } => audit.install += 1,
            bridge::Operation::CurrentPose => audit.current += 1,
            bridge::Operation::RawCapture => audit.raw += 1,
            other => return unsupported(&format!("{other:?}")),
        };
        Ok(())
    }
    fn admission(&mut self, a: &bridge::Admission) -> Result<(), String> {
        if self.installing || a.upper.is_some() || a.strategic.is_some() {
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
    fn pre_camera(&mut self, _: &mut Owner, _: &bridge::Admission) -> Result<(), String> {
        unsupported("pre_camera")
    }
    fn input(&mut self, _: &bridge::Admission) -> Result<Input, String> {
        unsupported("input")
    }
    fn consumed_input(&mut self) {
        panic!("capture-only consumed_input")
    }
    fn cursor(&mut self, _: Vec<CursorRequest>) -> Result<(), String> {
        unsupported("cursor")
    }
    fn navigate(&mut self, _: &mut Owner) -> Result<(), String> {
        unsupported("navigate")
    }
    fn tutorial(&mut self, _: bridge::NativeOperation, _: &mut Owner) -> Result<(), String> {
        unsupported("tutorial")
    }
    fn pending_sound(&mut self, _: Pose) -> Result<(), String> {
        unsupported("pending_sound")
    }
    fn present(&mut self, _: Pose) -> Result<(), String> {
        unsupported("present")
    }
    fn take_task_effects(&mut self) -> Vec<bridge::TaskEffect> {
        panic!("capture-only task effects")
    }
    fn publish_save_captures(&mut self) {
        panic!("capture-only manual-save publication")
    }
    fn discard_task_effects(&mut self) {
        panic!("unsupported endpoint operation; no successful cleanup substitute")
    }
}
struct CameraThread {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<(), String>>>,
    audit: Arc<Mutex<Audit>>,
}
impl CameraThread {
    fn start(inbox: bridge::Inbox, install: PropertyInstall, mode: u32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let audit = Arc::new(Mutex::new(Audit::default()));
        let observed = audit.clone();
        let join = thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(move || {
                let owner = Owner::new(Bank::loader_zero_storage(), 1);
                let mut endpoint = Endpoint::with_property_install(owner, inbox, install);
                let mut context = ReadOnlyContext {
                    mode,
                    installing: false,
                    audit: observed.clone(),
                };
                let started = Instant::now();
                let mut error = None;
                while !stopped.load(Ordering::Acquire) && started.elapsed() < WALL {
                    let raw_before = observed.lock().unwrap().raw;
                    let before = endpoint.capture();
                    let dirty = endpoint.owner().kind_dirty();
                    match endpoint.wait_service(&mut context, Duration::from_millis(20)) {
                        Ok(bridge::Service::Idle) => {}
                        Ok(bridge::Service::Completed) => {
                            if observed.lock().unwrap().install == 1 {
                                assert_eq!(
                                    endpoint
                                        .owner()
                                        .property_descriptors()
                                        .unwrap()
                                        .count()
                                        .unwrap(),
                                    52
                                );
                                assert!(endpoint.owner().resident_descriptors().is_ok());
                            }
                            if observed.lock().unwrap().raw != raw_before {
                                assert_eq!(endpoint.capture(), before);
                                assert_eq!(endpoint.owner().kind_dirty(), dirty);
                                let mut audit = observed.lock().unwrap();
                                audit.pairs.push(before.into_records());
                                audit.dirty.push(dirty);
                            }
                        }
                        Err(e) => {
                            error = Some(e);
                            break;
                        }
                    }
                }
                let owner = endpoint.into_owner();
                assert!(owner.property_lifecycle_error().is_none());
                error.map_or(Ok(()), Err)
            })
            .unwrap();
        Self {
            stop,
            join: Some(join),
            audit,
        }
    }
    fn finish(mut self) {
        self.stop.store(true, Ordering::Release);
        self.join.take().unwrap().join().unwrap().unwrap();
    }
}
impl Drop for CameraThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn install_host(
    s: &mut Session<'_>,
    loaded: &mut LoadedInspection,
    root: &Path,
    selection: &crate::selection::Assets,
    image: &openmt2001::pe::PeImage<'_>,
) -> (Host, CameraThread, [[u8; 196]; 2]) {
    let capture = loaded
        .imported_cameras
        .expect("actual imported pair required");
    assert!(matches!(loaded.saved_prior_mode, None | Some(0 | 3)));
    // Exact Inspection constructor policy (intro.rs5599): absent normal-view
    // extension is the saved normal view; Some(3) is strategic.
    let mode = if loaded.saved_prior_mode == Some(3) {
        3
    } else {
        0
    };
    let eye = loaded
        .saved_viewpoint
        .expect("genuine saved viewpoint required")
        .0
        .eye;
    let (worker, inbox) = bridge::channel(s.world.topology.epoch().serial());
    let camera = CameraThread::start(
        inbox,
        PropertyInstall::capture(selection, image).unwrap(),
        mode,
    );
    let mut h = Host::new(eye);
    h.autosave_profile_root = Some(root.to_path_buf());
    h.install_camera_bridge(
        s,
        image,
        crate::live_host::CameraSupplierPolicy::CameraOnly,
        worker,
        capture,
        selection.pavement_resource().binding().unwrap().handle(),
    )
    .unwrap();
    assert!(h.camera_bridge_installed());
    h.begin_presentation(s).unwrap();
    h.set_running(true);
    h.apply_restored_presentation(std::mem::take(&mut loaded.restored_presentation))
        .unwrap();
    // Unopposed Sandbox cannot require a successful fake auction renderer.
    assert_eq!(
        s.roster.manager.players.len(),
        1,
        "supply genuine UnopposedSandbox; do not remove opponents"
    );
    assert_eq!(s.catalog.selected_level().unwrap().id(), 400002);
    assert!(loaded.saved_auction.is_none());
    assert_eq!(s.world.auction.controller.reference, 0);
    assert_eq!(camera.audit.lock().unwrap().install, 1);
    (h, camera, capture.into_records())
}
fn accounts(s: &Session<'_>) -> Vec<Vec<u8>> {
    s.world
        .ledgers
        .players
        .iter()
        .map(|a| {
            let mut row = vec![0; 0x1b8];
            a.write_runtime(&mut row).unwrap();
            row
        })
        .collect()
}
fn pending_card(s: &Session<'_>) -> usize {
    s.live_commands.snapshot().unwrap().iter().filter(|(_,_,_,_,c)|
    matches!(c,openmt2001::live_commands::Command::EventCard(c) if c.card==1 && c.player==0)).count()
}
fn no_prompt(h: &mut Host, s: &Session<'_>) {
    assert!(
        h.take_prompts(s).is_empty(),
        "unexpected real prompt: inspect; never clear pause or omit it"
    );
}

#[test]
#[ignore = "supplied genuine ordinary UnopposedSandbox; natural CPU Host/protocol only"]
fn natural_host_two_fixed_slots_and_future_human_card() {
    run("OPENMT2001_AUTOSAVE_NATURAL_SAVE", false);
}
#[test]
#[ignore = "supplied genuine late-era UnopposedSandbox; natural CPU Host/protocol only"]
fn natural_host_era_queue_replaced_once_and_future_human_card() {
    run("OPENMT2001_AUTOSAVE_ERA_SAVE", true);
}
fn run(fixture_key: &'static str, expect_era: bool) {
    thread::Builder::new().stack_size(32*1024*1024).spawn(move||{
        let budget=Instant::now();let scratch=Scratch::new();let observer=probe::Observe::start();
        let read=|key|fs::read(std::env::var(key).unwrap()).unwrap();
        let original=PathBuf::from(std::env::var(fixture_key).unwrap());let original_bytes=fs::read(&original).unwrap();
        let parameter=PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let exe_path=PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());let executable=read("OPENMT2001_EXE");
        let image=openmt2001::pe::PeImage::parse(&executable).unwrap();
        let language=exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let style=crate::block_sidewalk_labels::load_style(&image,parameter.parent().unwrap().parent().unwrap()).unwrap();
        let inputs=SceneInputs {process_runtime:None,compatibility:crate::compatibility::Mode::StrictV14,
            style,time_runtime:Default::default(),profile_root:scratch.0.join("profiles"),
            character_names:crate::fresh_city::character_names(&image,&language).unwrap(),
            installer:parameter.parent().unwrap().parent().unwrap().to_path_buf(),language,
            maps:PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap()).parent().unwrap().to_path_buf(),
            build_file:read("OPENMT2001_BUILD_FILE"),executable:executable.clone(),
            directory:read("OPENMT2001_ARCHIVE_DIR"),archive:read("OPENMT2001_ARCHIVE_BIN"),suncols:read("OPENMT2001_SUNCOLS")};
        let index=openmt2001::archive::ArchiveIndex::parse(&inputs.directory,inputs.archive.len() as u64).unwrap();
        let selection=crate::selection::Assets::load(&inputs.executable,&index,&inputs.archive).unwrap();
        let board=inputs.styled_board(&image).unwrap();
        let file=openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
        let assets=CommandAssets{image:&image,board:&board,file:&file};
        let camera=super::fresh_city_camera().0;
        let (mut loaded,mut source)=inputs.saved_preview(&original,camera,&mut SnapshotCache::default()).unwrap();
        observer.discard_initial_restore();
        let root=crate::save_menu::selected_profile_root(&inputs.profile_root,b"NaturalHostV1");
        let other=crate::save_menu::selected_profile_root(&inputs.profile_root,b"IsolatedV1");
        fs::create_dir_all(other.join("saves")).unwrap();fs::write(other.join("saves/keep.sav"),&original_bytes).unwrap();
        let (mut host,ui,cameras)=install_host(&mut source,&mut loaded,&root,&selection,&image);
        let slot=root.join("saves/autosave.sav");assert!(!slot.exists());
        assert!(source.world.clock.hour()>0 && source.world.clock.hour()<24);
        assert_eq!(source.world.clock.timing[5],1);assert_eq!(source.tutorial_state,0);
        let mut budget=Budget{start:budget,used:0};let mut saves=Vec::new();let mut cuts=Vec::new();
        let mut card_opened=false;let mut requested=false;let mut due=0;let mut expected_packet=None;
        while saves.len()<2 {
            if saves.len()==1 && !requested {
                assert_eq!(source.event_cards.used[1],0);
                host.human_card(&mut source,&assets,crate::fresh_city::cards::Action::Request(Some(1))).unwrap();requested=true;
            }
            if saves.len()==1 && !card_opened && source.world.clock.hour()==23 && source.world.clock.timing[2]==610 {
                let row=openmt2001::event_cards::lifecycle::card_row(&image,1).unwrap();
                assert_eq!(word(row,8),200);assert_eq!(word(row,12),0);assert_eq!(row[16],0);
                assert_eq!(image.f32_at(0x534200).unwrap(),1.);
                let p=&source.roster.manager.players[0];let a=p.auxiliaries.iter().find(|a|a.reference==word(&p.bytes,0x21c)).unwrap();
                assert_eq!(word(&a.bytes,4),1);assert_ne!(word(&a.bytes,0x18),0);
                let before=accounts(&source);due=source.world.clock.timing[0]+25;
                host.human_card(&mut source,&assets,crate::fresh_city::cards::Action::Open).unwrap();
                assert_eq!(accounts(&source),before);assert_eq!(pending_card(&source),1);card_opened=true;
                let queue=source.live_commands.snapshot().unwrap();let (_,outer,inner,direct,command)=queue.iter().find(|(_,_,_,_,c)|
                    matches!(c,openmt2001::live_commands::Command::EventCard(c) if c.card==1 && c.player==0)).unwrap();
                assert_eq!((*outer,*inner,*direct),(due,due,false));
                let openmt2001::live_commands::Command::EventCard(command)=command else {unreachable!()};
                expected_packet=Some([0x52515cu32,0x1a,32,due,1,1,0,command.target as u32]
                    .into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>());
            }
            let before=source.world.clock;budget.tick(&mut host,&mut source,&assets);no_prompt(&mut host,&source);
            let observed=observer.take();if observed.is_empty(){continue;}
            assert_eq!(observed.len(),1);let cut=observed.into_iter().next().unwrap();
            assert_eq!(before.calendar[0],23);assert_eq!(before.timing[2],624);
            assert_eq!(cut.clock.calendar[0],0);assert_eq!(cut.clock.calendar[3],before.calendar[3]);
            assert_eq!(cut.clock.timing[0],before.timing[0]+1);assert_eq!(cut.clock.timing[2],0);
            assert_eq!(cut.clock.timing[3],0);
            let final_absolute=(cut.clock.calendar[1]+cut.clock.calendar[2]*10)*24;
            assert_eq!(source.world.clock.calendar[3],final_absolute);
            assert_ne!(cut.clock.calendar[3],source.world.clock.calendar[3],"transient word must remain distinguishable");
            assert!(matches!(&host.autosave_status,Some(crate::save_menu::AutosaveStatus::Saved(p)) if p==&slot),"{:?}",host.autosave_status);
            let bytes=fs::read(&slot).unwrap();probe::assert_file(&cut,&bytes,cameras);
            for (i,row) in source.world.categories.iter().enumerate() {
                for j in 0..24 {assert_eq!(word(row,12+4*j),cut.category_after.as_ref().unwrap()[i][j]);}
            }
            assert_eq!(fs::read_dir(slot.parent().unwrap()).unwrap().count(),1,"no numbered/staging slots");
            assert_eq!(fs::read(other.join("saves/keep.sav")).unwrap(),original_bytes);
            if let Some(previous)=saves.last(){assert_ne!(previous,&bytes,"second rollover must replace bytes");}
            // Infer the era boundary from the real before/callback clocks,
            // including Sandbox's five-years-per-day transition at the second cut.
            let enters_1940=before.calendar[2]==0 && cut.clock.calendar[2]==1;
            if saves.is_empty(){assert_eq!(enters_1940,expect_era,"first-cut fixture eligibility");}
            let intros=cut.scripts.iter().filter(|(name,_,_,_)|name=="EVENT_BusinessIntro1940").collect::<Vec<_>>();
            if enters_1940 {
                assert_eq!(intros.len(),1);assert_eq!((intros[0].2,intros[0].3),(1250,0));
                let mut later=Vec::new();
                for e in source.scripts.events.pending() {
                    let (name,_)=source.scripts.runtime.event_reference_identity(e.reference).unwrap().unwrap();
                    if name=="EVENT_BusinessIntro1940" {later.push((e.remaining,e.argument));}
                }
                assert_eq!(later,vec![(1249,0)],"same tick script tail must not enter frozen record13");
            } else if saves.is_empty(){assert!(intros.is_empty());}
            if !saves.is_empty() {
                assert!(card_opened);assert!(cut.clock.timing[0]<due);
                let q=cut.queue.iter().filter(|(_,_,_,_,p)|p.len()==32 && word(p,4)==0x1a).collect::<Vec<_>>();
                assert_eq!(q.len(),1);assert_eq!((q[0].0,q[0].1,q[0].2,q[0].3),(0,due,due,false));
                assert_eq!(word(&q[0].4,0),0x52515c);assert_eq!(word(&q[0].4,8),32);
                assert_eq!(word(&q[0].4,12),due);assert_eq!(q[0].4[16],1);
                assert_eq!(word(&q[0].4,20),1);assert_eq!(word(&q[0].4,24),0);
                assert_eq!(&q[0].4,expected_packet.as_ref().unwrap(),"literal32-byte future1a packet");
            }
            eprintln!("natural-host cut={} frame={} clock={:?} later={:?} allocation={:#x} queue={:?}",saves.len()+1,budget.used,cut.clock,source.world.clock,cut.rng[8],cut.queue);
            saves.push(bytes);cuts.push(cut);
        }
        // Load the two REAL files through the selected-profile preview routine
        // shared by saved_worker. This CPU adapter does not run session_worker.
        let continuation=root.join("saves/first-cut.sav");fs::write(&continuation,&saves[0]).unwrap();
        let (first_view,mut first)=inputs.saved_preview_with_selection(&continuation,camera,&mut SnapshotCache::default(),Some(Some(&root))).unwrap();
        probe::assert_loaded(&cuts[0],&mut first);drop(first_view);drop(first);
        let (mut reloaded,mut resumed)=inputs.saved_preview_with_selection(&slot,camera,&mut SnapshotCache::default(),Some(Some(&root))).unwrap();
        probe::assert_loaded(&cuts[1],&mut resumed);
        let (mut resumed_host,resumed_ui,_)=install_host(&mut resumed,&mut reloaded,&root,&selection,&image);
        // Modern pair starts from the callback snapshot, not source's later
        // interrupted tail. Original oracle and native cold process are separate.
        let (mut paired_view,mut paired)=inputs.saved_preview_with_selection(&slot,camera,&mut SnapshotCache::default(),Some(Some(&root))).unwrap();
        probe::assert_loaded(&cuts[1],&mut paired);
        let (mut paired_host,paired_ui,_)=install_host(&mut paired,&mut paired_view,&root,&selection,&image);
        let mut delivered=false;let mut posting=None;
        for _ in 0..48 {
            let before=accounts(&resumed);budget.tick(&mut resumed_host,&mut resumed,&assets);
            budget.tick(&mut paired_host,&mut paired,&assets);
            probe::assert_pair(&mut resumed,&mut paired);assert_eq!(accounts(&resumed),accounts(&paired));
            assert_eq!(resumed.event_cards.used,paired.event_cards.used);assert_eq!(resumed.random_registry.process,paired.random_registry.process);
            assert_eq!(resumed.allocation_next,paired.allocation_next);no_prompt(&mut resumed_host,&resumed);no_prompt(&mut paired_host,&paired);
            if resumed.world.clock.timing[0]<due {assert_eq!(pending_card(&resumed),1);assert_eq!(resumed.event_cards.used[1],1);}
            else if !delivered {
                assert_eq!(pending_card(&resumed),0);assert_eq!(resumed.event_cards.used[1],0);
                let after=accounts(&resumed);let bank=0x14+usize::from(after[0][0x144])*38*4;
                assert_eq!(f32::from_bits(word(&after[0],4))-f32::from_bits(word(&before[0],4)),200.);
                assert_eq!(f32::from_bits(word(&after[0],bank+0x24*4))-f32::from_bits(word(&before[0],bank+0x24*4)),200.);
                posting=Some((bank,word(&after[0],bank+0x24*4)));delivered=true;
            } else {
                assert_eq!(pending_card(&resumed),0);assert_eq!(resumed.event_cards.used[1],0);
                let (bank,value)=posting.unwrap();assert_eq!(word(&accounts(&resumed)[0],bank+0x24*4),value,"no repeat event24 posting");
            }
        }
        assert!(delivered);assert!(observer.take().is_empty());
        observer.assert_no_pending_restores();
        let source_audit=ui.audit.clone();
        // Retire the real endpoints before dropping their still-live Workers.
        // Dropping a Worker first is an unexpected disconnect, not normal quit.
        ui.finish();resumed_ui.finish();paired_ui.finish();drop(host);drop(resumed_host);drop(paired_host);
        let audit=source_audit.lock().unwrap();assert_eq!((audit.raw,audit.mode_reads,audit.pairs.len()),(2,2,2));
        assert!(audit.pairs.iter().all(|pair|*pair==cameras));drop(audit);
        assert_eq!(fs::read(&original).unwrap(),original_bytes);
        eprintln!("natural-host CPU/protocol PASS within {} /30000 ticks; fresh-process/UI NOT tested",budget.used);
        let own=scratch.0.clone();drop(scratch);assert!(!own.exists(),"Drop must remove only owned scratch");
    }).unwrap().join().unwrap();
}
