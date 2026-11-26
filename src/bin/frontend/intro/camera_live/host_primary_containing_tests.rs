//! Install-delayed CPU component fixture. Actual Assets, Host,
//! Endpoint and UiContext; no native startup/category-state equality claim.
//! PreparedScene::default is only an inert render carrier: no renderer, model,
//! category, source, callback, or Session authority is supplied by it.
use super::{deferred_primary::{DeferredPrimary,PrimaryProducts},
    endpoint::{Endpoint,PropertyInstall},owner::Owner,
    primitive_factory::{DescriptorIdentity,MainPhase,Phase,Step},
    primary_d_construction::PrimaryPhase,ui_graph::PanelId,
    Globals,OriginalGraphicsPolicy,State,Cursor,PhysicalInput,UiContext,UiJoins,SoundBoundary};
use crate::{intro::{Assets,Paths,Inspection,SceneInputs,SnapshotCache,DetailVisibility},
    live_host::{self,Host,CameraSupplierPolicy,camera_bridge as bridge}};
use openmt2001::{camera::bank::{Bank,Capture},render::PreparedScene};
use std::{fs,path::PathBuf,sync::{Arc,mpsc,atomic::{AtomicBool,Ordering}},thread,time::{Duration,Instant}};
const WAIT:Duration=Duration::from_secs(120);
const D:u32=0x574488;const A:u32=0x58e3c0;const F:u32=0x58cdb0;const G:u32=0x57e710;
// Independent retained43cc60 order, not registration::PRIMARY or phase enum.
const POOL:[PanelId;16]=[PanelId::F,PanelId::OtherPrimary(15),PanelId::C,PanelId::Comparison,
    PanelId::A,PanelId::Clock,PanelId::E,PanelId::OtherPrimary(9),PanelId::OtherPrimary(8),
    PanelId::OtherPrimary(7),PanelId::B,PanelId::H,PanelId::G,PanelId::OtherPrimary(3),
    PanelId::OtherPrimary(2),PanelId::OtherPrimary(1)];
fn path(key:&str)->PathBuf{std::env::var_os(key).map(PathBuf::from).unwrap_or_else(||panic!("required {key}"))}
struct Scratch(PathBuf);
impl Scratch {
    fn new()->Self{
        let parent=path("OPENMT2001_TEST_SCRATCH");assert!(parent.is_absolute());
        fs::create_dir_all(&parent).unwrap();
        let root=parent.join(format!("host-primary-containing-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();fs::create_dir(root.join("profiles")).unwrap();
        fs::create_dir(root.join("saves")).unwrap();Self(root)
    }
    fn paths(&self)->Paths{Paths{compatibility:crate::compatibility::Mode::StrictV14,
        installer:path("OPENMT2001_INSTALLER"),ffmpeg:path("OPENMT2001_FFMPEG"),scene:path("OPENMT2001_SCENE"),
        executable:path("OPENMT2001_EXE"),map:path("OPENMT2001_INTRO_MAP"),quality:"low".into(),
        language:path("OPENMT2001_LANGUAGE"),profile_root:self.0.join("profiles"),save_root:self.0.join("saves")}}
}
impl Drop for Scratch{fn drop(&mut self){fs::remove_dir_all(&self.0).expect("remove owned fixture scratch");}}
fn primary(a:&Assets)->&DeferredPrimary{match &a.primary_products{PrimaryProducts::Deferred(p)=>p,_=>panic!("actual deferred loader required")}}
fn row(w:&mut openmt2001::level_startup::World,i:usize,active:u8,excluded:u8,title:i32){
    w.categories[i][4]=active;w.categories[i][0x90]=excluded;w.categories[i][0x8c..0x90].copy_from_slice(&title.to_le_bytes());
}
fn controlled_rows(w:&mut openmt2001::level_startup::World,world:usize){
    // Explicit test writes to the real Session World after genuine restore.
    // This is not inferred native pre-menu state or a production World.
    for i in 0..52{row(w,i,0,1,46)}
    row(w,0,1,1,0);row(w,1,1,0,45);row(w,2,0,0,0);row(w,3,1,0,46);
    row(w,41,0,1,if world==0{0}else{45});
}
fn expected_words(i:u8)->[u32;3]{match i{0=>[1,1,0],1=>[1,0,45],2=>[0,0,0],3=>[1,0,46],41=>[0,1,0],_=>[0,1,46]}}
fn assert_first_reads(audit:&live_host::test_primary_category::Observe,epoch:u64){
    let calls=audit.records();
    // Two domain checks,52 row predicates, and three real title reloads for
    // each of the independently selected rows1/41 (42a413/46d/479).
    let mut expected=vec![(epoch,None,None),(epoch,None,None)];
    for i in 0..52u8{for _ in 0..(if i==1||i==41{4}else{1}){expected.push((epoch,Some(i),Some(expected_words(i))))}}
    assert_eq!(calls,expected,"actual Host-stack category reads/delivery");
}

// All render products here are empty and never exercised. Scene/models,
// Fonts/images, pavement, raw pair and Session come from actual resource owners.
// Missing navigation/tutorial/primary-clear joins stay None and fail if reached.
fn inspection(a:&Assets)->Inspection{
    let (camera,controls)=crate::intro::fresh_city_camera();let now=Instant::now();
    Inspection{
        ui_graph:None,camera_navigation:None,camera_focus:Default::default(),navigation_click_latched:false,
        camera_endpoint:None,camera_capture_pending:None,
        camera_ui:Some(State::new(Globals::after_normal_initialization(OriginalGraphicsPolicy::NormalD3d,false),
            Cursor::new(0,0).unwrap(),None,None)),
        save_profile_root:None,saved_camera:true,result_pending:None,result:None,
        auction_requests:None,auction_audio_requests:None,
        auction_audio_player:crate::live_host::auction_audio::Player::new(a.auction_audio.clone()),
        transport_audio_requests:None,transport_audio_player:crate::transport_audio::Player::new(a.transport_audio.clone()),
        era_audio_requests:None,era_audio_player:crate::era_audio::Player::new(a.era_audio.clone()),
        gameplay_sound_requests:None,gameplay_sound_player:crate::gameplay_sound::Player::new(a.gameplay_sound.clone()),
        auction:Default::default(),auction_panel:Default::default(),notifications:Default::default(),
        planning:None,planning_pending:None,planning_pressed:false,live_status:None,time_panel:Default::default(),
        cash_arrow:Default::default(),dust:Default::default(),time_pressed:None,game_options:Default::default(),
        save_dialog:Default::default(),save_pending:None,options_ack:Default::default(),save_delete_pending:None,settings_menu:Default::default(),
        pulse_draw:None,pulse_state:None,status_pending:None,environment_pending:None,environment_tick:0,
        environment_time:None,last_environment_query:now,last_status_query:now,last_snapshot:now,
        construction_active:false,snapshot_tick:0,card_input:Default::default(),card_selection:None,
        prompts:Default::default(),tutorial_message:Default::default(),event_list:Default::default(),
        stats_panel:Default::default(),stats_requested:false,tutorial_camera_seen:0,auction_notice:Default::default(),
        prompt_reply:None,block_information:None,utility_report:Default::default(),property_pending:None,
        rights_pending:None,comparison:None,trees:None,people:None,people_pending:None,last_people_query:now,
        vehicles:None,vehicles_pending:None,last_vehicles_query:now,transport:None,transport_pending:None,
        last_transport_query:now,_fresh_session:None,adjustment:None,operations:Default::default(),
        locator:None,locator_blocked:false,scene:a.scene.clone(),detail_visibility:DetailVisibility::default(),
        prepared:PreparedScene::default(),markers:PreparedScene::default(),navigator:PreparedScene::default(),
        navigator_owners:Vec::new(),navigator_bounds:a.navigator.bounds,navigator_pressed:false,
        tutorial_go_to_block_pressed:false,tooltip:None,tooltip_state:None,icon_tooltip_dwell:Default::default(),
        navigation:Default::default(),navigation_bounds:a.navigation.bounds,navigation_pressed:false,
        double_click:Default::default(),strategic:false,return_camera:None,reframe_pending:false,information:None,
        details_visible:true,details_pressed:false,toolbar_input:Default::default(),toolbar_right_held:false,
        toolbar_bounds:[[0.;4];3],toolbar_regions:[None;3],detail_icons:Default::default(),detail_bounds:a.navigation.detail_bounds,
        camera,controls,building_angle:0.,building_camera:false,
        inertia:openmt2001::camera::BlockCameraInertia::new(camera.eye,camera.target),keys:[false;4],taps:[false;4],
        cursor:None,mouse_buttons:[false;2],drag:[0;2],pointer:None,hovered:None,selected:None,title_state:None,started:now,tick:0,
    }
}
fn service(view:&mut Inspection,a:&mut Assets,physical:&mut PhysicalInput)->Result<bridge::Service,String>{
    let mut endpoint=view.camera_endpoint.take().expect("actual retained endpoint");
    let joins=UiJoins{primary_clear:None,navigation:None,tutorial:None};
    let result={let mut context=UiContext::new(view,physical,a,&joins,SoundBoundary::Existing2d).unwrap();
        endpoint.wait_service(&mut context,WAIT)};
    view.camera_endpoint=Some(endpoint);result
}
fn retire(view:&mut Inspection,a:&mut Assets,flag:&AtomicBool,epoch:u64)->Owner{
    // Same production city-exit order as Intro::retire_inspection: D first,
    // then refresh::retire -> Endpoint::into_owner -> source retirement/wake.
    assert!(!flag.load(Ordering::Acquire));
    let PrimaryProducts::Deferred(p)=&mut a.primary_products else{unreachable!()};
    p.retire_for_camera(a.navigator.process_primitives_mut()).unwrap();
    assert!(p.require_current_d(epoch).is_err());
    assert!(!flag.load(Ordering::Acquire),"D must invalidate before Inbox retirement wakes Host");
    let mut retry=crate::intro::refresh::Retry::default();let mut refreshing=false;
    let mut loading=None;let mut parked=None;
    crate::intro::refresh::retire(&mut retry,&mut refreshing,&mut loading,
        Some(&mut view.camera_endpoint),&mut parked).unwrap();
    assert!(view.camera_endpoint.is_none());assert!(flag.load(Ordering::Acquire));let owner=parked.unwrap();
    assert!(owner.property_lifecycle_error().is_none());assert!(owner.property_descriptors().unwrap().count().is_err());
    assert!(owner.resident_descriptors().unwrap().source().is_err());owner
}
fn unbound(a:&mut Assets){
    assert!(a.reordered_main().is_err());assert!(a.primary_products.require_ordered_main().is_err());
    assert!(primary(a).require_supplier().is_err());
    let p=a.navigator.process_primitives_mut();assert_eq!(p.main_phase(),MainPhase::Unstarted);
    for at in [A,F]{assert_eq!(p.panels[&at].phase,Phase::Constructed);
        assert_eq!(p.panels[&at].descriptor,None);assert_eq!(p.cells().byte(at+0x184).unwrap(),0);}
    assert!(!p.steps.iter().any(|s|matches!(s,Step::Bind(at)|Step::Layout(at)|Step::Bound(at) if *at==A||*at==F)));
    assert!(p.require_process_graph().is_err());
}
#[derive(Debug,PartialEq,Eq)]
struct Footprint{
    identities:Vec<usize>,model_allocations:Vec<usize>,steps:Vec<Step>,cells:Vec<Vec<u8>>,fonts:String,images:String,texts:Vec<String>,
    models:Vec<Vec<u8>>,descriptors:Vec<Vec<u8>>,rows:Vec<(usize,Option<usize>,Option<i32>,Option<u32>,bool)>,
}
fn footprint(a:&mut Assets)->Footprint{
    let d=primary(a);let mut identities=vec![d.d() as *const _ as usize,d.main().retained_birth() as *const _ as usize,
        a.ui_text.fonts() as *const _ as usize,a.ui_images.records.as_ptr() as usize];
    let rows=d.d().rows.iter().map(|r|{identities.push(&**r as *const _ as usize);
        (r.number as usize,r.category,r.title,r.title_record,r.initialized)}).collect();
    let texts=d.d().rows.iter().map(|r|format!("{:?}",r.text)).collect();
    let fonts=format!("{:?}",a.ui_text.fonts());let images=format!("{:?}",a.ui_images.records);
    let p=a.navigator.process_primitives_mut();let mut models=Vec::new();let mut descriptors=Vec::new();let mut model_allocations=Vec::new();
    for id in POOL{
        let m=p.resources.model(id).unwrap();model_allocations.push(m.frames.as_ptr() as usize);
        let mut bytes=m.definition.to_vec();bytes.extend_from_slice(&m.header);
        for frame in &m.frames{model_allocations.push(frame.as_ptr() as usize);
            for xyz in frame{for v in xyz{bytes.extend_from_slice(&v.to_bits().to_le_bytes());}}}
        models.push(bytes);
        if let Ok(d)=p.resources.descriptor(id){let mut b=d.definition.to_vec();
            for v in d.position{b.extend_from_slice(&v.to_bits().to_le_bytes());}
            b.extend_from_slice(&d.frame.to_le_bytes());b.push(d.alpha);b.extend_from_slice(&d.angle_x.to_bits().to_le_bytes());
            b.extend_from_slice(&d.kind.to_le_bytes());b.extend_from_slice(format!("{:?}{:?}",d.slot,d.reference).as_bytes());descriptors.push(b);}
    }
    let cells=[(D,0xdf8),(A,0x50c),(F,0x160c),(G,0x88),(0x57e6ec,0x24),(0x59050c,4),
        (0x560a48,0xc8),(0x560b14,4)].into_iter().map(|(at,n)|p.cells().read(at,n).unwrap().to_vec()).collect();
    Footprint{identities,model_allocations,steps:p.steps.clone(),cells,fonts,images,texts,models,descriptors,rows}
}
// Original43b95a..43b996: two separate f32 stores, read from the
// allocation snapshot before any bind/layout. No production bind is the oracle.
fn assert_success_model_bytes(before:&Footprint,after:&Footprint,executable:&[u8]){
    use openmt2001::{pe::PeImage,x87::Sum};
    assert_eq!(before.models.len(),16);assert_eq!(after.models.len(),16);
    let pe=PeImage::parse(executable).unwrap();
    let negative=f32::from_le_bytes(pe.at(0x525264,4).unwrap().try_into().unwrap());
    let offset=f32::from_le_bytes(pe.at(0x525034,4).unwrap().try_into().unwrap());
    assert!(negative.is_finite()&&offset.is_finite());
    // Fixed43cc60 pool ordinals13=Chat,14=EventList,15=Voting.
    for ordinal in 0..16{
        let raw=&before.models[ordinal];let actual=&after.models[ordinal];
        if ordinal<13{assert_eq!(actual,raw,"unbound model ordinal{ordinal}");continue}
        assert_eq!(actual.len(),raw.len());assert!(raw.len()>=280);
        assert_eq!(&actual[..280],&raw[..280],"serialized definition/header preserved");
        let vertices=usize::from(u16::from_le_bytes(raw[116..118].try_into().unwrap()));
        let frames=usize::from(u16::from_le_bytes(raw[120..122].try_into().unwrap()));
        assert_eq!((vertices,frames),match ordinal{13=>(72,1),14=>(80,1),15=>(88,1),_=>unreachable!()});
        assert_eq!(raw.len(),280+12*vertices*frames);
        for (old,new) in raw[280..].chunks_exact(12).zip(actual[280..].chunks_exact(12)){
            assert_eq!(&new[..8],&old[..8],"XY preserved at bound ordinal{ordinal}");
            let mut z=Sum::from_f32(f32::from_le_bytes(old[8..12].try_into().unwrap())).unwrap();
            z.mul_f32(negative).unwrap();let stored=z.to_f32();
            let mut z=Sum::from_f32(stored).unwrap();z.add_f32(offset).unwrap();
            assert_eq!(u32::from_le_bytes(new[8..12].try_into().unwrap()),z.to_f32().to_bits(),
                "two original43b770 f32 stores at bound ordinal{ordinal}");
        }
    }
}
struct Start{inbox:bridge::Inbox,epoch:u64,capture:Capture,retirement:Arc<AtomicBool>}
fn host_run(inputs:Arc<SceneInputs>,pavement:crate::selection::pavement_resource::LocalPavementHandle,
    starts:mpsc::Sender<Start>,retired:mpsc::Receiver<()>,fault:bool){
    let image=openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
    let observe=live_host::test_primary_category::Observe::start();
    for world in 0..(if fault{1}else{2}){
        let (loaded,mut s)=inputs.saved_preview(&path(if world==0{"OPENMT2001_SAVE_ONE"}else{"OPENMT2001_SAVE_TWO"}),
            crate::intro::fresh_city_camera().0,&mut SnapshotCache::default()).unwrap();
        controlled_rows(&mut s.world,world);let epoch=s.world.topology.epoch().serial();
        let capture=loaded.imported_cameras.expect("genuine saved raw pair required");
        let (worker,inbox)=bridge::channel(epoch);let retirement=worker.test_retirement_flag();
        starts.send(Start{inbox,epoch,capture,retirement}).unwrap();
        let mut host=Host::new(loaded.saved_viewpoint.expect("actual saved camera").0.eye);
        let install=host.install_camera_bridge(&mut s,&image,CameraSupplierPolicy::CameraOnly,worker,capture,pavement);
        if fault{
            assert!(install.is_err());assert!(host.failure.is_some());assert!(!host.camera_bridge_installed());
            assert_eq!(observe.records(),vec![(epoch,None,None)],"actual prelude check before ordinal6 fault");
            // Same failed Host cannot issue even a read, much less replay Install.
            assert!(host.exchange_camera_with_session(&mut s,bridge::Operation::CurrentPose).is_err());
            let (replacement,rejected)=bridge::channel(epoch);
            assert!(host.install_camera_bridge(&mut s,&image,CameraSupplierPolicy::CameraOnly,replacement,capture,pavement).is_err());
            drop(rejected);assert_eq!(observe.records(),vec![(epoch,None,None)]);
        }else{
            install.unwrap();assert!(host.camera_bridge_installed());assert!(host.failure.is_none());
            if world==0{assert_first_reads(&observe,epoch)}else{assert_eq!(observe.records(),vec![(epoch,None,None)]);}
            let reads=observe.records();
            // Real Host transport, read-only CameraOnly operations after Install.
            // No CentralTail/gameplay advancement or numerical camera claim.
            let current=host.exchange_camera_with_session(&mut s,bridge::Operation::CurrentPose).unwrap().unwrap();
            assert!(current.raw_capture.is_none());assert!(current.sampled.is_none());
            let raw=host.exchange_camera_with_session(&mut s,bridge::Operation::RawCapture).unwrap().unwrap();
            let raw=raw.raw_capture.expect("real RawCapture output");assert_eq!(raw.cameras,capture);
            assert_eq!(raw.interface_mode.unwrap(),0);assert_eq!(observe.records(),reads,"reads cannot enter category service");
        }
        retired.recv_timeout(WAIT).expect("UI must actually retire after final operation");
        assert!(host.camera_retired());
        if !fault{assert!(matches!(host.exchange_camera_with_session(&mut s,bridge::Operation::CurrentPose),Err(bridge::RequestError::Retired)));}
        observe.clear();
    }
}
/// On any assertion failure, retire the real endpoint before joining Host.
/// Closing the fixture channel releases a Host waiting between operations;
/// dropping the receiving Start channel releases a not-yet-installed worker.
struct Fixture{
    a:Assets,view:Inspection,physical:PhysicalInput,
    retired:Option<mpsc::Sender<()>>,started:Option<mpsc::Receiver<Start>>,host:Option<thread::JoinHandle<()>>,
}
impl Fixture{
    fn close(&mut self)->thread::Result<()>{
        if let PrimaryProducts::Deferred(p)=&mut self.a.primary_products{
            let _=p.retire_for_camera(self.a.navigator.process_primitives_mut());
        }
        if let Some(endpoint)=self.view.camera_endpoint.take(){let _=endpoint.into_owner();}
        self.retired.take();self.started.take();
        self.host.take().map_or(Ok(()),|host|host.join())
    }
}
impl Drop for Fixture{fn drop(&mut self){let _=self.close();}}
fn run(fault:bool){
    thread::Builder::new().stack_size(32*1024*1024).spawn(move||{
        let scratch=Scratch::new();let mut a=Assets::load_deferred_primary(scratch.paths(),"").unwrap();
        let view=inspection(&a);let physical=PhysicalInput::default();unbound(&mut a);
        assert_eq!(primary(&a).phase(),PrimaryPhase::Allocated);let birth=footprint(&mut a);
        assert!(birth.steps.iter().all(|s|!matches!(s,Step::Currency|Step::DescriptorCopy(_)|Step::Bind(_)|Step::Layout(_))));
        if fault{a.navigator.process_primitives_mut().resources.models.get_mut(&PanelId::E).unwrap().source.panel=PanelId::OtherPrimary(255);}
        let inputs=a.inputs.clone();let pavement=a.selection.pavement_resource().binding().unwrap().handle();
        let (starts,started)=mpsc::channel();let (retired,retirement)=mpsc::channel();
        // All Lua/Session owners are created and retained on this Host stack.
        let host=thread::Builder::new().stack_size(32*1024*1024).spawn(move||host_run(inputs,pavement,starts,retirement,fault)).unwrap();
        let executable=a.inputs.executable.clone();let image=openmt2001::pe::PeImage::parse(&executable).unwrap();
        let mut fixture=Fixture{a,view,physical,retired:Some(retired),started:Some(started),host:Some(host)};
        let Fixture{a,view,physical,retired,started,..}=&mut fixture;
        let mut parked=Some(Owner::new(Bank::loader_zero_storage(),1));let mut retained=None;let mut epoch_one=None;
        for world in 0..(if fault{1}else{2}){
            let start=started.as_ref().unwrap().recv_timeout(WAIT).expect("real Session startup must supply actual epoch/inbox");
            if let Some(one)=epoch_one{assert_ne!(one,start.epoch)}else{epoch_one=Some(start.epoch)}
            let owner=parked.take().unwrap();let before_raw=owner.capture();let before_dirty=owner.kind_dirty();
            view.camera_endpoint=Some(Endpoint::with_property_install(owner,start.inbox,PropertyInstall::capture(&a.selection,&image).unwrap()));
            let result=service(view,a,physical);
            if fault{
                assert!(result.is_err());assert_eq!(primary(&a).phase(),PrimaryPhase::Failed);
                let p=a.navigator.process_primitives_mut();assert_eq!((p.cells().word(G+0x2c).unwrap(),p.cells().word(G+0x30).unwrap(),p.cells().byte(G+0x34).unwrap()),(6,16,1));
                assert_eq!(p.resources.descriptors.len(),6);assert!(p.primary_pool().is_none());
                assert_eq!(p.steps.iter().filter_map(|s|if let Step::DescriptorCopy(i)=s{Some(*i)}else{None}).collect::<Vec<_>>(),vec![0,1,2,3,4,5]);
                assert!(!p.steps.contains(&Step::Bind(D)));assert_eq!(p.cells().word(G+0x24).unwrap(),16);
                assert_eq!(view.camera_endpoint.as_ref().unwrap().capture(),before_raw);
                assert_eq!(view.camera_endpoint.as_ref().unwrap().owner().kind_dirty(),before_dirty);
                unbound(a);
                // D failure is sticky. Teardown is still real Endpoint retirement.
                let owner=view.camera_endpoint.take().unwrap().into_owner();assert!(owner.property_lifecycle_error().is_none());
                retired.as_ref().unwrap().send(()).unwrap();continue
            }
            assert_eq!(result.unwrap(),bridge::Service::Completed);
            assert!(view.camera_endpoint.as_ref().unwrap().camera_only_admission());
            assert!(view.ui_graph.is_none());assert!(view.camera_navigation.is_none());unbound(a);
            let endpoint=view.camera_endpoint.as_ref().unwrap();assert_eq!(endpoint.capture(),start.capture);
            assert_eq!(endpoint.owner().property_descriptors().unwrap().count().unwrap(),52);assert!(endpoint.owner().resident_descriptors().is_ok());
            if world==0{
                assert_eq!(primary(&a).phase(),PrimaryPhase::AwaitingG);primary(&a).require_current_d(start.epoch).unwrap();
                let d=primary(&a).d();assert_eq!(d.rows.iter().map(|r|(r.category,r.title,r.title_record,r.initialized)).collect::<Vec<_>>(),
                    vec![(Some(1),Some(45),Some(0x539518),true),(Some(41),Some(0),Some(0x5385a0),true)]);
                assert!(d.rows.iter().all(|r|r.text.group.count>0));
                let p=a.navigator.process_primitives_mut();assert_eq!(p.primary_pool().unwrap(),&POOL);
                assert_eq!((p.cells().word(G+0x2c).unwrap(),p.cells().word(G+0x30).unwrap(),p.cells().byte(G+0x34).unwrap()),(16,16,1));
                assert_eq!(p.resources.descriptors.len(),16);assert_eq!(p.panels[&D].descriptor,Some(DescriptorIdentity::EmbeddedD));
                assert_eq!(p.panels[&D].phase,Phase::Bound);assert_eq!(p.cells().byte(D+0x184).unwrap(),1);
                assert_eq!((p.cells().word(G+8).unwrap(),p.cells().word(G+12).unwrap(),p.cells().word(G+0x24).unwrap()),(0x57ef28,D,17));
                let expected=vec![Step::Currency,Step::DescriptorCopy(0),Step::DescriptorCopy(1),Step::DescriptorCopy(2),Step::DescriptorCopy(3),Step::DescriptorCopy(4),Step::DescriptorCopy(5),Step::DescriptorCopy(6),Step::DescriptorCopy(7),Step::DescriptorCopy(8),Step::DescriptorCopy(9),Step::DescriptorCopy(10),Step::DescriptorCopy(11),Step::DescriptorCopy(12),Step::DescriptorCopy(13),Step::DescriptorCopy(14),Step::DescriptorCopy(15),Step::PrimaryReferences,Step::Bind(D),Step::Layout(D),Step::LayoutReturned(D),Step::Bound(D),Step::Registered(D),Step::PrimaryCount(17),Step::Bind(0x573b38),Step::Layout(0x573b38),Step::LayoutReturned(0x573b38),Step::Bound(0x573b38),Step::Registered(0x573b38),Step::Bind(0x571378),Step::Layout(0x571378),Step::LayoutReturned(0x571378),Step::Bound(0x571378),Step::Registered(0x571378),Step::Bind(0x57ef28),Step::Layout(0x57ef28),Step::LayoutReturned(0x57ef28),Step::Bound(0x57ef28),Step::Registered(0x57ef28)];
                assert_eq!(p.steps.iter().filter(|s|matches!(s,Step::Currency|Step::DescriptorCopy(_)|Step::PrimaryReferences|Step::Bind(_)|Step::Layout(_)|Step::LayoutReturned(_)|Step::Bound(_)|Step::Registered(_)|Step::PrimaryCount(_))).copied().collect::<Vec<_>>(),expected);
                let after=footprint(a);assert_eq!(&after.identities[..4],&birth.identities[..4]);
                assert_eq!(after.model_allocations,birth.model_allocations);assert_success_model_bytes(&birth,&after,&executable);retained=Some(after);
            }else{
                assert_eq!(primary(&a).phase(),PrimaryPhase::AwaitingDRebind);assert_eq!(primary(&a).d().epoch,epoch_one);
                assert_eq!(primary(&a).d().parked_for,Some(start.epoch));assert!(primary(&a).require_current_d(start.epoch).is_err());
                assert_eq!(footprint(a),*retained.as_ref().unwrap(),"World2 cannot replay any retained construction work");
            }
            let before=footprint(a);let raw=view.camera_endpoint.as_ref().unwrap().capture();
            let dirty=view.camera_endpoint.as_ref().unwrap().owner().kind_dirty();
            for _ in 0..2{
                assert_eq!(service(view,a,physical).unwrap(),bridge::Service::Completed);
                assert!(view.camera_endpoint.as_ref().unwrap().camera_only_admission());
                assert_eq!(view.camera_endpoint.as_ref().unwrap().capture(),raw);
                assert_eq!(view.camera_endpoint.as_ref().unwrap().owner().kind_dirty(),dirty);
                assert_eq!(footprint(a),before,"CameraOnly reads cannot mutate D/Text/Fonts/models/pool");
            }
            parked=Some(retire(view,a,&start.retirement,start.epoch));assert_eq!(parked.as_ref().unwrap().capture(),raw);
            assert_eq!(parked.as_ref().unwrap().kind_dirty(),dirty);
            assert_eq!(primary(&a).phase(),PrimaryPhase::AwaitingDRebind);assert!(primary(&a).require_current_d(start.epoch).is_err());
            assert_eq!(footprint(a),before,"actual retirement retains construction products");
            retired.as_ref().unwrap().send(()).unwrap();
        }
        fixture.close().unwrap();
    }).unwrap().join().unwrap();
}
#[test]
#[ignore="supplied actual Assets and genuine saves; fresh-process validation"]
fn actual_host_install_deferred_assets_uicontext_d_reads_retirement_and_world2(){run(false)}
#[test]
#[ignore="supplied actual Assets/Session ordinal6 resource fault; fresh-process validation"]
fn actual_host_install_descriptor_fault_retains_six_completed_copies(){run(true)}
