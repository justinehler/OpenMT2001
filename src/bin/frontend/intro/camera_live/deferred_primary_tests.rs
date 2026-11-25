//! Supplied-file original-order/component regressions.
//! These are not connected native startup-placement or platform acceptance.
use super::*;
use super::super::{primitive_factory::{Step,Phase,MainPhase,DescriptorIdentity},ui_graph::PanelId};
use crate::live_host::camera_bridge::{self as bridge,Operation,Admission,Observation,Pose,SessionCall};
use std::{fs,path::PathBuf,thread,time::Duration};
const D:u32=0x574488;const A:u32=0x58e3c0;const F:u32=0x58cdb0;const G:u32=0x57e710;
fn path(key:&str)->PathBuf{std::env::var_os(key).map(PathBuf::from).unwrap_or_else(||panic!("required {key}"))}
fn paths()->crate::intro::Paths{crate::intro::Paths{compatibility:crate::compatibility::Mode::StrictV14,
    installer:path("OPENMT2001_INSTALLER"),ffmpeg:path("OPENMT2001_FFMPEG"),scene:path("OPENMT2001_SCENE"),
    executable:path("OPENMT2001_EXE"),map:path("OPENMT2001_INTRO_MAP"),quality:"low".into(),language:path("OPENMT2001_LANGUAGE"),
    profile_root:path("OPENMT2001_TEXT_PROFILE"),save_root:path("OPENMT2001_TEXT_SAVES")}}
struct Inputs{exe:Vec<u8>,archive:Vec<u8>,index:ArchiveIndex}
impl Inputs{
    fn read()->Self{
        let exe=fs::read(path("OPENMT2001_EXE")).unwrap();let archive=fs::read(path("OPENMT2001_ARCHIVE_BIN")).unwrap();
        let index=ArchiveIndex::parse(&fs::read(path("OPENMT2001_ARCHIVE_DIR")).unwrap(),archive.len() as u64).unwrap();Self{exe,archive,index}
    }
    fn text(&self)->UiTextResources{
        let mut text=UiTextResources::load(&self.exe,&path("OPENMT2001_INSTALLER"),&path("OPENMT2001_LANGUAGE"),&self.index,&self.archive).unwrap();
        text.install_usa_board(&self.exe,&path("OPENMT2001_INSTALLER")).unwrap();text
    }
    fn images(&self)->Images{Images::load(&self.exe,&self.index,&self.archive,[640,480]).unwrap()}
    fn model_f(&self)->CpuModel{
        //Independent original pool0 literal; do not get the expected slot/path
        //from the production registration array or controller phase enum.
        let entry=self.index.find("gamedata2/brand_new_panels/new_nav_top.tm").unwrap();
        CpuModel::parsed(PanelId::F,entry,&CachedMesh::parse(&self.archive[entry.offset as usize..(entry.offset+entry.length)as usize]).unwrap()).unwrap()
    }
    fn fresh(&self)->(DeferredPrimary,Primitives,UiTextResources,Images){
        let mut p=Primitives::deferred_allocation(&self.exe,self.model_f()).unwrap();let text=self.text();let images=self.images();
        let primary=DeferredPrimary::allocate(&self.exe,&self.index,&self.archive,&mut p,&text,&images).unwrap();
        assert_eq!(primary.phase(),PrimaryPhase::Allocated);(primary,p,text,images)
    }
    fn world(&self)->openmt2001::level_startup::World{
        let pe=PeImage::parse(&self.exe).unwrap();let board=openmt2001::board::Board::from_v14(&pe).unwrap();
        let configuration=openmt2001::resident::Configuration::before_defaults(
            &fs::read_to_string(path("OPENMT2001_PARAMETERS")).unwrap(),&fs::read_to_string(path("OPENMT2001_TIMETABLE")).unwrap(),&pe).unwrap();
        //Explicit controlled component fixture only. Never a production
        //throwaway World to claim native pre-menu category placement.
        let creation=openmt2001::economy::ResidentCreationState{auxiliary:openmt2001::resident::AuxiliaryPool::for_residents(),
            initialization_rng:openmt2001::resident::InitializationRng{constructor:7,movement:11,objective:13},identity_rng:17,preference_rng:19};
        openmt2001::level_startup::World::fresh(&pe,&board,configuration,creation).unwrap()
    }
}
fn row(w:&mut openmt2001::level_startup::World,i:usize,active:u8,excluded:u8,title:i32){
    w.categories[i][4]=active;w.categories[i][0x90]=excluded;w.categories[i][0x8c..0x90].copy_from_slice(&title.to_le_bytes());
}
fn run(primary:DeferredPrimary,p:Primitives,text:UiTextResources,images:Images,w:&openmt2001::level_startup::World)
    ->(DeferredPrimary,Primitives,UiTextResources,Images,Result<InstallDisposition,String>){
    run_with_validation_rejection(primary,p,text,images,w,None)
}
fn run_with_validation_rejection(mut primary:DeferredPrimary,mut p:Primitives,mut text:UiTextResources,images:Images,w:&openmt2001::level_startup::World,reject_validation:Option<usize>)
    ->(DeferredPrimary,Primitives,UiTextResources,Images,Result<InstallDisposition,String>){
    let source=ResidentSource::from_world(w);let generation=source.epoch().serial();
    let (mut worker,mut inbox)=bridge::channel(generation);
    let ui=thread::spawn(move||{
        let mut outcome=None;
        let operation=inbox.wait_service_with_handler(Duration::from_secs(30),|_,port|{
            let result=primary.install(&source,generation,port,&mut p,&mut text,&images);outcome=Some(result.clone());result?;
            Ok(Observation{raw_capture:None,current:Pose{eye:[0;3],target:[0;3],up:[0;3]},sampled:None,strategic_writeback:None,finalization_fault:None,task_effects:vec![]})
        });
        let outcome=outcome.unwrap();assert_eq!(operation.is_ok(),outcome.is_ok());inbox.retire();(primary,p,text,images,outcome)
    });
    let mut validations=0;
    let wire=worker.request_with_handler(Operation::Install{property:None,capture:openmt2001::camera::bank::Capture::from_records([[0;196];2]),admission:Admission::default()},|call|{
        let SessionCall::PrimaryCategory{source,row}=call else{panic!("Fresh prefix emitted a mutating/general callback")};
        if row.is_none(){validations+=1;if reject_validation==Some(validations){return Err("injected actual category validation failure".into())}}
        crate::live_host::test_primary_category_read(w,&source,row)
    });
    let result=ui.join().unwrap();assert_eq!(wire.is_ok(),result.4.is_ok());result
}
fn assert_unbound(p:&Primitives){
    for address in [A,F]{assert_eq!(p.panels[&address].phase,Phase::Constructed);assert_eq!(p.panels[&address].descriptor,None);assert_eq!(p.cells().byte(address+0x184).unwrap(),0);}
    assert_eq!(p.main_phase(),MainPhase::Unstarted);
    assert!(!p.steps.iter().any(|step|matches!(step,Step::Bind(x)|Step::Layout(x)|Step::Bound(x) if *x==A||*x==F)));
    assert!(p.require_process_graph().is_err());
}

#[test]
#[ignore="supplied production Assets loader/ffmpeg; run in a fresh process"]
fn actual_cold_assets_defers_effects_and_has_no_completed_main(){
    let mut assets=Box::new(crate::intro::Assets::load_deferred_primary(paths(),"").unwrap());
    let PrimaryProducts::Deferred(primary)=&assets.primary_products else{panic!("fresh loader returned eager product")};
    assert_eq!(primary.phase(),PrimaryPhase::Allocated);assert!(assets.reordered_main().is_err());assert!(assets.primary_products.require_ordered_main().is_err());
    let fonts=assets.ui_text.fonts() as *const _;let main=primary.main().unbound_birth().unwrap();let main_ptr=main as *const MainBirth;
    let p=assets.navigator.process_primitives_mut();assert_unbound(p);assert!(p.resources.descriptors.is_empty());assert_eq!(p.resources.models.len(),16);
    assert_eq!(p.cells().word(G+0x24).unwrap(),0);assert!(!p.steps.iter().any(|s|matches!(s,Step::Currency|Step::DescriptorCopy(_)|Step::Bind(_)|Step::Layout(_))));
    let moved=assets;let PrimaryProducts::Deferred(primary)=&moved.primary_products else{unreachable!()};let main=primary.main().unbound_birth().unwrap();
    assert_eq!(main as *const MainBirth,main_ptr);assert_eq!(moved.ui_text.fonts() as *const _,fonts);
}

#[test]
#[ignore="supplied actual World/Fonts/protected wire, independent original order"]
fn original_prefix_order_all_sixteen_copies_d_only_and_same_owners(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();assert_unbound(&p);
    let main=primary.main().unbound_birth().unwrap();let main_ptr=main as *const MainBirth;let d_ptr=primary.d() as *const DProduct;
    let fonts=text.fonts() as *const _;let a=p.resources.model(PanelId::A).unwrap().frames.as_ptr();let f=p.resources.model(PanelId::F).unwrap().frames.as_ptr();
    let mut w=input.world();for i in 0..52{row(&mut w,i,0,1,46);}row(&mut w,1,1,0,45);row(&mut w,3,1,0,46);row(&mut w,41,0,1,0);
    let (primary,p,text,_,result)=run(primary,p,text,images,&w);result.unwrap();assert_unbound(&p);
    //Independent retained43cc60 caller/store oracle, not registration::PRIMARY.
    let observed=p.steps.iter().filter(|s|matches!(s,Step::Currency|Step::DescriptorCopy(_)|Step::PrimaryReferences|Step::Bind(_)|Step::Layout(_)|Step::LayoutReturned(_)|Step::Bound(_)|Step::Registered(_)|Step::PrimaryCount(_))).copied().collect::<Vec<_>>();
    assert_eq!(observed,vec![Step::Currency,Step::DescriptorCopy(0),Step::DescriptorCopy(1),Step::DescriptorCopy(2),Step::DescriptorCopy(3),Step::DescriptorCopy(4),Step::DescriptorCopy(5),Step::DescriptorCopy(6),Step::DescriptorCopy(7),Step::DescriptorCopy(8),Step::DescriptorCopy(9),Step::DescriptorCopy(10),Step::DescriptorCopy(11),Step::DescriptorCopy(12),Step::DescriptorCopy(13),Step::DescriptorCopy(14),Step::DescriptorCopy(15),Step::PrimaryReferences,Step::Bind(D),Step::Layout(D),Step::LayoutReturned(D),Step::Bound(D),Step::Registered(D),Step::PrimaryCount(17)]);
    assert_eq!(p.primary_pool().unwrap(),&[PanelId::F,PanelId::OtherPrimary(15),PanelId::C,PanelId::Comparison,PanelId::A,PanelId::Clock,PanelId::E,PanelId::OtherPrimary(9),PanelId::OtherPrimary(8),PanelId::OtherPrimary(7),PanelId::B,PanelId::H,PanelId::G,PanelId::OtherPrimary(3),PanelId::OtherPrimary(2),PanelId::OtherPrimary(1)]);
    for &id in p.primary_pool().unwrap(){let descriptor=p.resources.descriptor(id).unwrap();assert_eq!(descriptor.kind,11);assert_eq!(descriptor.angle_x,180.);assert_eq!(descriptor.definition,p.resources.model(id).unwrap().definition);}
    assert_ne!(a,p.resources.model(PanelId::G).unwrap().frames.as_ptr());assert_eq!(p.resources.model(PanelId::A).unwrap().frames.as_ptr(),a);assert_eq!(p.resources.model(PanelId::F).unwrap().frames.as_ptr(),f);
    assert_eq!((p.cells().word(G+0x2c).unwrap(),p.cells().word(G+0x30).unwrap(),p.cells().byte(G+0x34).unwrap()),(16,16,1));
    assert_eq!(p.panels[&D].descriptor,Some(DescriptorIdentity::EmbeddedD));assert_eq!(p.cells().byte(D+0x184).unwrap(),1);
    assert_eq!((p.cells().word(G+8).unwrap(),p.cells().word(G+12).unwrap(),p.cells().word(G+0x24).unwrap()),(D,D,17));
    assert_eq!((p.cells().word(D+0x380).unwrap(),p.cells().word(D+0x384).unwrap()),(0,0));
    assert_eq!(primary.d().rows.iter().map(|r|(r.category,r.title,r.title_record)).collect::<Vec<_>>(),vec![(Some(1),Some(45),Some(0x539518)),(Some(41),Some(0),Some(0x5385a0))]);
    let pe=PeImage::parse(&input.exe).unwrap();for r in &primary.d().rows{let id=u32::from_le_bytes(pe.at(r.title_record.unwrap(),4).unwrap().try_into().unwrap()).wrapping_add(1);assert!(!text.fonts().language.row(id).unwrap().is_empty());assert!(r.text.group.count>0);}
    let main=primary.main().unbound_birth().unwrap();assert_eq!(main as *const MainBirth,main_ptr);assert_eq!(primary.d() as *const DProduct,d_ptr);assert_eq!(text.fonts() as *const _,fonts);
    assert_eq!(primary.phase(),PrimaryPhase::AwaitingVoting);assert!(primary.require_ordered_main().is_err());assert!(primary.require_supplier().is_err());
}

#[test]
#[ignore="supplied resources: eager admission and wrong-resource read-only rejection"]
fn eager_owners_cannot_convert_and_preclaim_resource_failure_does_not_mutate(){
    let input=Inputs::read();let mut eager=Primitives::navigator(&input.exe,input.model_f()).unwrap();let text=input.text();let images=input.images();
    let bytes=eager.cells().read(F,0x160c).unwrap().to_vec();let steps=eager.steps.clone();
    assert!(DeferredPrimary::allocate(&input.exe,&input.index,&input.archive,&mut eager,&text,&images).is_err());assert_eq!(eager.steps,steps);assert_eq!(eager.cells().read(F,0x160c).unwrap(),bytes);
    let (primary,p,text,_)=input.fresh();let foreign=input.images();let before=p.steps.clone();let d=primary.d() as *const DProduct;let scratch=text.fonts().scratch.clone();let w=input.world();
    let (primary,p,text,_,result)=run(primary,p,text,foreign,&w);assert!(result.is_err());assert_eq!(primary.phase(),PrimaryPhase::Allocated);assert_eq!(p.steps,before);assert_eq!(primary.d() as *const DProduct,d);assert_eq!(text.fonts().scratch,scratch);assert!(p.resources.descriptors.is_empty());
}

#[test]
#[ignore="supplied World negative-title reached failure/no replay"]
fn late_d_failure_retains_real_prefix_child_and_rejects_repair_retry(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let mut w=input.world();for i in 0..52{row(&mut w,i,0,1,46);}row(&mut w,0,1,0,-1);
    let (primary,p,text,images,result)=run(primary,p,text,images,&w);assert!(result.unwrap_err().contains("signed before-table"));
    assert_eq!(primary.phase(),PrimaryPhase::Failed);assert_eq!(p.primary_phase,Some(PrimaryPhase::Failed));assert_eq!(primary.d().rows.len(),1);assert!(!primary.d().rows[0].initialized);assert_unbound(&p);
    assert_eq!(p.cells().byte(D+0x184).unwrap(),0);assert_eq!(p.cells().word(G+0x24).unwrap(),16);assert_eq!(p.cells().word(G+8).unwrap(),0);assert_eq!(p.resources.descriptors.len(),16);
    let child=&*primary.d().rows[0] as *const _;let steps=p.steps.clone();row(&mut w,0,1,0,0);
    let (primary,p,_,_,result)=run(primary,p,text,images,&w);assert!(result.is_err());assert_eq!(&*primary.d().rows[0] as *const _,child);assert_eq!(p.steps,steps);
}

#[test]
#[ignore="supplied fresh Main gate and reached real Font failure/no replay"]
fn fresh_main_rejects_eager_prefix_and_font_failure_preserves_partial_owner(){
    let input=Inputs::read();let (primary,mut p,mut text,mut images)=input.fresh();let before=p.steps.clone();
    assert!(super::super::main_construction::Prefix::load(&input.exe,&input.index,&input.archive,[640,480],&mut text,&mut p,&mut images).is_err());
    assert_eq!(p.main_phase(),MainPhase::Unstarted);assert_eq!(p.steps,before);assert_unbound(&p);
    let font=text.fonts_mut().fonts.remove(&0).unwrap();let mut w=input.world();for i in 0..52{row(&mut w,i,0,1,46);}row(&mut w,0,1,0,0);
    let (primary,p,mut text,images,result)=run(primary,p,text,images,&w);assert!(result.is_err());assert_eq!(primary.phase(),PrimaryPhase::Failed);
    assert_eq!(primary.d().rows.len(),1);assert!(!primary.d().rows[0].initialized);let child=&*primary.d().rows[0] as *const _;let steps=p.steps.clone();
    assert_eq!(p.resources.descriptors.len(),16);assert_eq!(p.cells().word(G+0x24).unwrap(),16);assert_eq!(p.cells().byte(D+0x184).unwrap(),0);
    text.fonts_mut().fonts.insert(0,font);
    let (primary,p,_,_,result)=run(primary,p,text,images,&w);assert!(result.is_err());assert_eq!(&*primary.d().rows[0] as *const _,child);assert_eq!(p.steps,steps);assert_unbound(&p);
}

#[test]
#[ignore="supplied source rejection after real currency/pool prefix"]
fn source_failure_after_prefix_before_layout_latches_without_replaying_prefix(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let w=input.world();
    let (primary,p,text,images,result)=run_with_validation_rejection(primary,p,text,images,&w,Some(2));assert!(result.is_err());
    assert_eq!(primary.phase(),PrimaryPhase::Failed);assert_eq!(p.primary_phase,Some(PrimaryPhase::Failed));assert_eq!(p.resources.descriptors.len(),16);
    assert_eq!(p.cells().word(G+0x24).unwrap(),16);assert_eq!(p.cells().byte(D+0x184).unwrap(),0);assert!(primary.d().rows.is_empty());
    assert!(p.steps.contains(&Step::Currency));assert!(!p.steps.contains(&Step::Layout(D)));let steps=p.steps.clone();
    let (primary,p,_,_,retry)=run(primary,p,text,images,&w);assert!(retry.is_err());assert_eq!(primary.phase(),PrimaryPhase::Failed);assert_eq!(p.steps,steps);assert_unbound(&p);
}

#[test]
#[ignore="supplied Worlds and source retirement"]
fn second_world_parks_without_currency_pool_birth_layout_or_registration_replay(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let mut one=input.world();let mut two=input.world();
    for i in 0..52{row(&mut one,i,0,1,46);row(&mut two,i,0,1,46);}row(&mut one,41,0,1,0);row(&mut two,41,0,1,45);
    let (mut primary,mut p,text,images,first)=run(primary,p,text,images,&one);first.unwrap();let steps=p.steps.clone();let d=p.cells().read(D,0xdf8).unwrap().to_vec();
    let child=&*primary.d().rows[0] as *const _;let fonts=text.fonts() as *const _;
    primary.retire_for_camera(&mut p).unwrap();assert!(primary.require_current_d(one.topology.epoch().serial()).is_err());
    let (primary,p,text,_,second)=run(primary,p,text,images,&two);assert!(matches!(second.unwrap(),InstallDisposition::ParkedAwaitingDRebind{..}));
    assert_eq!(primary.phase(),PrimaryPhase::AwaitingDRebind);assert!(primary.require_current_d(two.topology.epoch().serial()).is_err());assert_eq!(p.steps,steps);assert_eq!(p.cells().read(D,0xdf8).unwrap(),d);
    assert_eq!(&*primary.d().rows[0] as *const _,child);assert_eq!(text.fonts() as *const _,fonts);assert_unbound(&p);
}


#[test]
#[ignore="supplied actual resources; explicit invalid-resource fault at ordinal6"]
fn descriptor_copy_failure_retains_only_completed_prefix_and_latches(){
    let input=Inputs::read();let (primary,mut p,text,images)=input.fresh();let w=input.world();
    // Deliberate invalid source on the actual seventh resource. This models a
    // modern rejected descriptor, not an original allocator/valid-input fault.
    p.resources.models.get_mut(&PanelId::E).unwrap().source.panel=PanelId::OtherPrimary(255);
    let (primary,p,text,images,result)=run(primary,p,text,images,&w);assert!(result.is_err());
    assert_eq!(primary.phase(),PrimaryPhase::Failed);assert_eq!(p.primary_phase,Some(PrimaryPhase::Failed));
    assert_eq!((p.cells().word(G+0x2c).unwrap(),p.cells().word(G+0x30).unwrap(),p.cells().byte(G+0x34).unwrap()),(6,16,1));
    assert_eq!(p.resources.descriptors.len(),6);assert!(p.primary_pool().is_none());
    let copies=p.steps.iter().filter_map(|s|if let Step::DescriptorCopy(i)=s{Some(*i)}else{None}).collect::<Vec<_>>();
    assert_eq!(copies,vec![0,1,2,3,4,5]);assert!(!p.steps.contains(&Step::Bind(D)));assert_unbound(&p);
    let steps=p.steps.clone();let (primary,p,_,_,retry)=run(primary,p,text,images,&w);
    assert!(retry.is_err());assert_eq!(primary.phase(),PrimaryPhase::Failed);assert_eq!(p.steps,steps);
    assert_eq!(p.cells().word(G+0x2c).unwrap(),6);assert_eq!(p.resources.descriptors.len(),6);
}
