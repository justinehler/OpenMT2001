//! Supplied-resource regressions. No synthetic success
//! maps or world/category/business State.
use super::*;
use super::super::super::{primitive_factory::{Phase,Step},process_image::Cells};
use std::{fs,path::PathBuf};
fn path(key:&str)->PathBuf{std::env::var_os(key).map(PathBuf::from).unwrap_or_else(||panic!("required {key}"))}
struct Supplied {exe:Vec<u8>,archive:Vec<u8>,index:ArchiveIndex,installer:PathBuf,language:PathBuf}
impl Supplied {
    fn read()->Self{
        let exe=fs::read(path("OPENMT2001_EXE")).unwrap();let directory=fs::read(path("OPENMT2001_ARCHIVE_DIR")).unwrap();
        let archive=fs::read(path("OPENMT2001_ARCHIVE_BIN")).unwrap();let index=ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
        Self{exe,archive,index,installer:path("OPENMT2001_INSTALLER"),language:path("OPENMT2001_LANGUAGE")}
    }
    fn text(&self)->UiTextResources{let mut t=UiTextResources::load(&self.exe,&self.installer,&self.language,&self.index,&self.archive).unwrap();t.install_usa_board(&self.exe,&self.installer).unwrap();t}
    fn navigator(&self)->crate::navigator::Navigator{
        let board=openmt2001::board::Board::from_v14(&PeImage::parse(&self.exe).unwrap()).unwrap();
        crate::navigator::Navigator::load(&self.index,&self.archive,&board.blocks,&self.exe).unwrap()
    }
    fn images(&self)->Images{Images::load(&self.exe,&self.index,&self.archive,[640,480]).unwrap()}
    fn prefix(&self,t:&mut UiTextResources,n:&mut crate::navigator::Navigator,i:&mut Images)->AwaitingOtherMainChildren{
        Prefix::load(&self.exe,&self.index,&self.archive,[640,480],t,n.process_primitives_mut(),i).unwrap()
    }
}
#[test]
#[ignore="requires supplied originals"]
fn child1_every_allocation_number_append_native_order_and_real_report_pairs(){
    let s=Supplied::read();let mut text=s.text();let mut n=s.navigator();let mut images=s.images();
    let prefix=s.prefix(&mut text,&mut n,&mut images);assert_eq!(prefix.phase(),MainPhase::AwaitingOtherMainChildren);
    assert_eq!((prefix.counter(),prefix.nodes().count()),(23,25));assert!(prefix.inner.child1.is_none());
    let root=prefix.root();let child0=prefix.child0().list;
    let old_nodes=prefix.nodes().map(|node|(node.id,node as *const Node)).collect::<Vec<_>>();
    let a=n.process_primitives_mut().resources.model(PanelId::A).unwrap().frames[0].as_ptr();
    let f=n.process_primitives_mut().resources.model(PanelId::F).unwrap().frames[0].as_ptr();
    let fonts=text.fonts() as *const Fonts;let records=images.records.as_ptr();let mut slot=Some(prefix);
    let done=AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut text,n.process_primitives_mut(),&mut images).unwrap();
    assert!(slot.is_none());assert_eq!(done.phase(),MainPhase::AwaitingChild5);assert_eq!(done.prefix.phase(),MainPhase::AwaitingChild5);
    assert_eq!(done.root(),root);assert_eq!(done.child0().list,child0);assert_eq!((done.counter(),done.nodes().count()),(48,60));
    assert_eq!(done.child1().parent,root);assert_eq!(done.child1().source,CHILD);assert_eq!(done.child1().native_return,-1);
    let offsets=[0x6c,0x38,0x64,0x68,0x2c,0x30,0x74,0x78,0x34,0x80,0x84,0x94,0x98,0x9c,0xa0,0xa4,0xa8,0xb0,0xb4,0xac,0xc4,0xb8,0xbc,0x8c,0x90];
    assert_eq!(done.child1().fields.len(),offsets.len());
    let new=&done.node(root).unwrap().children[20..];assert_eq!(new.len(),offsets.len());
    for (i,(&id,offset)) in new.iter().zip(offsets).enumerate(){
        assert_eq!(id,done.child1().field(offset).unwrap());let node=done.node(id).unwrap();
        assert_eq!(node.label,Label::Property(offset));assert_eq!(node.parent,Some(root));assert_eq!(node.root,Some(root));assert_eq!(node.number,Some(23+i as u32));
        let events=done.journal().iter().filter(|e|e.node==id).collect::<Vec<_>>();
        let alloc=events.iter().position(|e|matches!(e.event,Event::Allocate(_))).unwrap();
        let slot04=events.iter().position(|e|matches!(e.event,Event::Slot04(_))).unwrap();
        let number=events.iter().position(|e|matches!(e.event,Event::Number(_))).unwrap();
        let append=events.iter().position(|e|matches!(e.event,Event::Append(_))).unwrap();assert!(alloc<slot04&&slot04<number&&number<append);
    }
    for node in done.nodes().skip(25){
        let events=done.journal().iter().filter(|e|e.node==node.id).collect::<Vec<_>>();
        let alloc=events.iter().position(|e|matches!(e.event,Event::Allocate(_))).unwrap();
        let slot04=events.iter().position(|e|matches!(e.event,Event::Slot04(_))).unwrap();
        let number=events.iter().position(|e|matches!(e.event,Event::Number(_))).unwrap();
        let append=events.iter().position(|e|matches!(e.event,Event::Append(_))).unwrap();
        assert!(alloc<slot04&&slot04<number&&number<append,"all nested arrows/amount children follow their native wrapper order");
        if matches!(node.subtype,Subtype::SpecialList3Texts(_)|Subtype::List3Texts(_)){
            assert_eq!(node.field18,0);assert_eq!(node.embedded_text_contexts24.len(),3);
            assert!(node.embedded_text_contexts24.iter().all(|c|matches!(c,Field::Unknown)));
            assert_eq!(events.iter().filter(|e|matches!(e.event,Event::Constructor(0x440160))).count(),3);
        }
    }
    for (id,pointer) in old_nodes{assert_eq!(done.node(id).unwrap() as *const Node,pointer,"same prior boxed owner survives Vec growth");}
    for (list,selector) in [(0x64,0x68),(0x74,0x78),(0x80,0x84),(0x8c,0x90)]{
        let l=done.child1().field(list).unwrap();let sel=done.child1().field(selector).unwrap();
        let Subtype::SpecialList3Texts(owner)=&done.node(l).unwrap().subtype else{panic!("mandatory special report list")};
        assert_eq!(owner.indicator,Some(sel));assert_eq!((owner.selectable680,owner.flag681),(1,0));assert!(owner.selection658.is_empty());
        assert_eq!((owner.list.key,owner.list.chosen,owner.list.value),(l,-1,0));assert!(owner.list.entries.is_empty());
        let sn=done.node(sel).unwrap();let Subtype::Selector2Arrows(so)=&sn.subtype else{panic!()};
        assert_eq!(so.first_list,None);assert_eq!(so.last_list,Some(l),"actual431ea0 binds160, not ordinary15c");assert_eq!((sn.shown,sn.disabled,so.admitted191),(0,1,0));
        assert_eq!(done.node(so.previous.unwrap()).unwrap().parent,Some(sel));assert_eq!(done.node(so.next.unwrap()).unwrap().parent,Some(sel));
    }
    assert_eq!(done.child1().global(0x86d3c0),Some(done.child1().field(0x64).unwrap()));
    for (offset,address) in [(0x2c,0x86d3c4),(0x30,0x86d3c8),(0xac,0x86d3cc),(0xbc,0x86d4ac)]{assert_eq!(done.child1().global(address),Some(done.child1().field(offset).unwrap()));}
    assert_eq!(fonts,text.fonts() as *const Fonts);assert_eq!(records,images.records.as_ptr());
    let p=n.process_primitives_mut();assert_eq!(p.main_phase(),MainPhase::AwaitingChild5);assert_eq!(p.cells().word(A+0x388).unwrap(),48);
    assert_eq!(p.resources.model(PanelId::A).unwrap().frames[0].as_ptr(),a);assert_eq!(p.resources.model(PanelId::F).unwrap().frames[0].as_ptr(),f);
    assert_eq!(p.steps.iter().filter(|s|**s==Step::Bind(A)).count(),1);assert_eq!(p.panels[&A].phase,Phase::AwaitingLayout);
    assert_eq!(p.cells().byte(A+0x184).unwrap(),0);assert_eq!(done.journal().last().unwrap().source,0x41e725);
    assert!(done.alias(0x4bc).is_err());assert!(done.require_full_main().unwrap_err().contains("child5"));assert!(done.require_input_or_render().is_err());assert!(p.require_process_graph().is_err());
}
#[test]
#[ignore="supplied originals; real synchronous glyph and title alias effects"]
fn rights442960_and_shared_title_are_real_text_owners_and_palette16(){
    let s=Supplied::read();let mut text=s.text();let mut n=s.navigator();let mut images=s.images();let mut prefix=Some(s.prefix(&mut text,&mut n,&mut images));
    let shared=prefix.as_ref().unwrap().alias(0x4b8).unwrap();let pointer=prefix.as_ref().unwrap().node(shared).unwrap() as *const Node;
    let done=AwaitingChild5::continue_child1(&mut prefix,&s.exe,&mut text,n.process_primitives_mut(),&mut images).unwrap();
    assert_eq!(done.node(shared).unwrap() as *const Node,pointer);
    let title=done.node(shared).unwrap();let Subtype::Text(t)=&title.subtype else{panic!("one shared parent Text")};
    assert_eq!(title.shown,1);assert_eq!((t.font,t.pitch),(3,10));assert!(t.group.count>0);assert!(t.colors[..t.group.count as usize].iter().all(|&c|c==16));
    assert_eq!(done.nodes().filter(|node|node.label==Label::SharedTitle).count(),1);
    for (offset,id,font) in [(0xac,47,2),(0xbc,53,0)]{
        let node=done.node(done.child1().field(offset).unwrap()).unwrap();let Subtype::Visual(v)=&node.subtype else{panic!("actual caption-bearing Visual")};
        assert_eq!((node.shown,v.caption_mode,v.caption.font,v.caption.style),(0,3,font,4));assert_eq!(v.caption_identifier2f4,Field::Known(id));
        assert!(v.caption.group.count>0);assert!(!v.caption.group.bytes.is_empty());assert!(v.caption.colors[..v.caption.group.count as usize].iter().all(|&c|c==0));
        assert!(!text.fonts().language.row(id).unwrap().is_empty());
        let calls=done.journal().iter().filter(|e|e.node==node.id).collect::<Vec<_>>();
        let reset=calls.iter().position(|e|matches!(e.event,Event::TextCall(0x441940))).unwrap();
        let append=calls.iter().position(|e|matches!(e.event,Event::TextCall(0x441220))).unwrap();
        let flags=calls.iter().position(|e|matches!(e.event,Event::TextCall(0x440330))).unwrap();
        let publish=calls.iter().position(|e|matches!(e.event,Event::CaptionStore(_))).unwrap();
        assert!(reset<append&&append<flags&&flags<publish);
        assert!(done.journal().iter().any(|e|e.node==node.id&&matches!(e.event,Event::TextCall(0x442960))));
    }
    //Title34 is actually reset in the tail; labels+a4 are a different owner.
    let Subtype::Text(t)=&done.node(done.child1().field(0x34).unwrap()).unwrap().subtype else{panic!()};assert_eq!((t.group.count,t.font,t.style),(0,3,4));
    let a4=done.child1().field(0xa4).unwrap();assert_ne!(a4,done.child1().field(0xb8).unwrap());
    let Subtype::Text(label)=&done.node(a4).unwrap().subtype else{panic!()};assert_eq!((label.style,label.font),(1,3));assert!(label.group.count>0);
}
#[test]
#[ignore="supplied originals; concrete amount children, range stores and image identity"]
fn amount_and_special_list_use_native_subtypes_source_images_and_true_setters(){
    let s=Supplied::read();let mut text=s.text();let mut n=s.navigator();let mut images=s.images();let mut prefix=Some(s.prefix(&mut text,&mut n,&mut images));
    let mut done=AwaitingChild5::continue_child1(&mut prefix,&s.exe,&mut text,n.process_primitives_mut(),&mut images).unwrap();
    let amount=done.child1().field(0xb0).unwrap();let outer=done.node(amount).unwrap();assert_eq!(outer.family,Family::Amount2Visuals);
    let Subtype::Amount2Visuals(a)=&outer.subtype else{panic!()};assert_eq!((a.dirty13c,a.value140,a.step144,a.minimum148,a.maximum14c),(1,1000.,100.,1000.,100000.));
    let first=match &a.first150{Field::Known(id)=>*id,Field::Unknown=>panic!("42ec42 first writer absent")};let second=match &a.second154{Field::Known(id)=>*id,Field::Unknown=>panic!("42ecb0 first writer absent")};assert_eq!(outer.children,[first,second]);
    for (id,number,label) in [(first,0,0x68b010),(second,1,0x68b000)]{let node=done.node(id).unwrap();assert_eq!(node.parent,Some(amount));assert_eq!(node.root,Some(done.root()));assert_eq!(node.number,Some(number));let Subtype::Visual(v)=&node.subtype else{panic!()};assert_eq!(v.current,Some(images.id(label).unwrap()));assert_eq!(v.caption_mode,4);}
    assert!(done.node(first).unwrap().bounds[0]>done.node(second).unwrap().bounds[0],"source arg order places+150 on the right");
    let append=done.journal().iter().position(|e|e.node==amount&&matches!(e.event,Event::Append(_))).unwrap();
    let child=done.journal().iter().position(|e|e.node==first&&matches!(e.event,Event::Allocate(_))).unwrap();assert!(append<child);
    for label in [0x68afac,0x68af58,0x68b000]{assert_eq!(images.record(images.id(label).unwrap()).unwrap().scale,Some(1.));}
    let Subtype::Amount2Visuals(a)=&mut done.prefix.inner.node_mut(amount).unwrap().subtype else{panic!()};
    a.dirty13c=0;a.range42efb0(3.,9.,1.);assert_eq!(a.dirty13c,0);assert_eq!((a.step144,a.minimum148,a.maximum14c),(3.,9.,1.));
    a.amount42ee70(-4.);assert_eq!((a.dirty13c,a.value140),(1,-4.),"setter does not clamp to the range");
    let special=done.child1().field(0x6c).unwrap();let normal=done.child1().field(0x38).unwrap();
    assert!(matches!(done.node(special).unwrap().subtype,Subtype::SpecialList3Texts(_)));assert!(matches!(done.node(normal).unwrap().subtype,Subtype::List3Texts(_)));
    let Subtype::SpecialList3Texts(l)=&done.node(special).unwrap().subtype else{panic!()};assert_eq!((l.list.primary.font,l.list.secondary.font,l.list.scratch.font),(3,3,0));assert_eq!(l.list.secondary.pitch,10,"RET reset retains pitch");assert_eq!((l.list.chosen,l.list.value),(-1,0));
    //Final41f510 follows auxiliary set_font0. Direct4406a0 must use the
    //active font0 while primary Text.font is still0, then set_font3 later.
    let final_list=done.child1().field(0x8c).unwrap();let Subtype::SpecialList3Texts(last)=&done.node(final_list).unwrap().subtype else{panic!()};
    let glyph=usize::from(text.fonts().language.bytes()[usize::from(b'W'-32)]);let at=4+glyph*44+36;
    let table=&text.fonts().fonts[&0].table;let height=i32::from_le_bytes(table[at..at+4].try_into().unwrap());
    assert_eq!((last.list.primary.font,last.list.primary.pitch,last.list.primary.geometry_dirty),(3,height,1));
    assert_eq!(last.list.primary.extent,[0.;4],"no4409c0 measure tail");assert_eq!(last.list.secondary.pitch,10);
}
#[test]
#[ignore="supplied originals; typed owner/phase rejection must precede all moves"]
fn failed_duplicate_and_foreign_continuations_keep_same_owners_and_do_not_replay(){
    let s=Supplied::read();let mut text=s.text();let mut n=s.navigator();let mut images=s.images();let prefix=s.prefix(&mut text,&mut n,&mut images);
    let root=prefix.root();let counter=prefix.counter();let mut slot=Some(prefix);let scratch=text.fonts().scratch.clone();
    let mut other_images=s.images();let mut other_text=s.text();let mut other_nav=s.navigator();
    assert!(AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut text,n.process_primitives_mut(),&mut other_images).err().unwrap().contains("SAME"));
    assert!(AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut other_text,n.process_primitives_mut(),&mut images).err().unwrap().contains("SAME"));
    assert!(AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut text,other_nav.process_primitives_mut(),&mut images).is_err());
    assert_eq!(slot.as_ref().unwrap().root(),root);assert_eq!(slot.as_ref().unwrap().counter(),counter);assert_eq!(text.fonts().scratch,scratch);
    assert!(AwaitingChild5::continue_child1(&mut slot,b"missing PE",&mut text,n.process_primitives_mut(),&mut images).is_err());
    assert_eq!(n.process_primitives_mut().main_phase(),MainPhase::AwaitingOtherMainChildren);
    let done=AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut text,n.process_primitives_mut(),&mut images).unwrap();
    let scratch=text.fonts().scratch.clone();let counter=done.counter();
    assert!(AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut text,n.process_primitives_mut(),&mut images).is_err());
    assert_eq!(done.counter(),counter);assert_eq!(text.fonts().scratch,scratch);
    //A real missing image in a separate actual product fails mid-construction.
    let mut t=s.text();let mut nav=s.navigator();let mut i=s.images();let mut failed=Some(s.prefix(&mut t,&mut nav,&mut i));let r=failed.as_ref().unwrap().root();
    let ordinal=i.record(i.id(0x68afac).unwrap()).unwrap().ordinal;i.records.truncate(ordinal);
    assert!(AwaitingChild5::continue_child1(&mut failed,&s.exe,&mut t,nav.process_primitives_mut(),&mut i).is_err());
    assert_eq!(nav.process_primitives_mut().main_phase(),MainPhase::Failed);assert_eq!(failed.as_ref().unwrap().phase(),MainPhase::Failed);assert_eq!(failed.as_ref().unwrap().root(),r);
    let journal=failed.as_ref().unwrap().journal().len();let scratch=t.fonts().scratch.clone();
    assert!(AwaitingChild5::continue_child1(&mut failed,&s.exe,&mut t,nav.process_primitives_mut(),&mut i).is_err());assert_eq!(failed.as_ref().unwrap().journal().len(),journal);assert_eq!(t.fonts().scratch,scratch);
    let mut t=s.text();let mut nav=s.navigator();let mut i=s.images();let mut no_font=Some(s.prefix(&mut t,&mut nav,&mut i));
    t.fonts_mut().fonts.remove(&0); //negative resource failure, never a successful fixture
    assert!(AwaitingChild5::continue_child1(&mut no_font,&s.exe,&mut t,nav.process_primitives_mut(),&mut i).is_err());
    assert_eq!(nav.process_primitives_mut().main_phase(),MainPhase::Failed);
    assert_eq!(done.root(),root,"failed candidate cannot replace already retained successful product");
}
#[test]
#[ignore="supplied originals; source birth/guards rather than caller no-selection flag"]
fn actual_process_birth_and_source_guards_control_no_selection_domain(){
    let s=Supplied::read();let raw=Cells::process_allocation(&s.exe).unwrap();assert_eq!(raw.word(0x822d50).unwrap(),0);
    let mut text=s.text();let mut n=s.navigator();let mut images=s.images();let mut slot=Some(s.prefix(&mut text,&mut n,&mut images));
    //A nonnull source word is rejected without resolving a fabricated object.
    //This mutation is negative fault injection only, never successful input.
    let saved=n.process_primitives_mut().cells().word(0x822d50).unwrap();
    n.process_primitives_mut().test_property_selection_word(u32::MAX).unwrap();
    let root=slot.as_ref().unwrap().root();let scratch=text.fonts().scratch.clone();
    assert!(AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut text,n.process_primitives_mut(),&mut images).err().unwrap().contains("selected822d50"));
    assert_eq!(slot.as_ref().unwrap().root(),root);assert_eq!(text.fonts().scratch,scratch);
    assert_eq!(n.process_primitives_mut().main_phase(),MainPhase::AwaitingOtherMainChildren);
    n.process_primitives_mut().test_property_selection_word(saved).unwrap();
    assert_eq!(saved,0);
    let done=AwaitingChild5::continue_child1(&mut slot,&s.exe,&mut text,n.process_primitives_mut(),&mut images).unwrap();
    let p=n.process_primitives_mut();assert_eq!(p.cells().word(0x822d50).unwrap(),0);assert_eq!(p.cells().word(A+0x4f4).unwrap(),u32::MAX);
    assert_eq!(p.cells().word(CHILD+0x14).unwrap(),u32::MAX);assert_eq!(p.cells().word(CHILD+0x18).unwrap(),u32::MAX);
    assert_eq!(p.cells().byte(CHILD+0x11).unwrap(),0);assert_eq!(p.cells().byte(CHILD+0x12).unwrap(),0);
    assert_eq!(p.cells().word(CHILD+0x70).unwrap(),0);assert_eq!(p.cells().word(CHILD+0x20).unwrap(),0);assert_eq!(p.cells().word(CHILD+0x24).unwrap(),0);
    for i in 0..6{assert_eq!(done.node(done.alias(0x48c+i*4).unwrap()).unwrap().shown,0);}
    for i in 0..5{let node=done.node(done.alias(0x4a4+i*4).unwrap()).unwrap();assert_eq!(node.shown,1);
        let Subtype::Visual(v)=&node.subtype else{panic!()};assert_eq!(v.selected336,u8::from(i==0));
        assert_eq!((v.caption_mode,v.policy338),(3,1),"caption mode13c differs from selection policy338");}
    assert!(done.child1().require_selected_reports().is_err());
}
#[test]
#[ignore="requires supplied Assets::load inputs and ffmpeg"]
fn actual_assets_entry_advances_once_and_failed_load_cannot_replace_published_assets(){
    let paths=||crate::intro::Paths{compatibility:crate::compatibility::Mode::StrictV14,
        installer:path("OPENMT2001_INSTALLER"),ffmpeg:path("OPENMT2001_FFMPEG"),scene:path("OPENMT2001_SCENE"),
        executable:path("OPENMT2001_EXE"),map:path("OPENMT2001_INTRO_MAP"),quality:"low".into(),language:path("OPENMT2001_LANGUAGE"),
        profile_root:path("OPENMT2001_TEXT_PROFILE"),save_root:path("OPENMT2001_TEXT_SAVES")};
    let mut published=Box::new(crate::intro::Assets::load(paths()).unwrap());
    assert_eq!(published.main_prefix.phase(),MainPhase::AwaitingParentTail);assert_eq!(published.main_prefix.child1().fields.len(),25);
    let root=published.main_prefix.root();let fonts=published.ui_text.fonts() as *const Fonts;let child=published.main_prefix.child1().field(0xac).unwrap();let child_ptr=published.main_prefix.node(child).unwrap() as *const Node;
    let mut missing=paths();missing.executable=path("OPENMT2001_TEXT_PROFILE").join("intentionally-missing-original.exe");
    if let Ok(replacement)=crate::intro::Assets::load(missing){published=Box::new(replacement);panic!("missing supplied PE unexpectedly published");}
    assert_eq!(published.main_prefix.root(),root);assert_eq!(published.ui_text.fonts() as *const Fonts,fonts);assert_eq!(published.main_prefix.node(child).unwrap() as *const Node,child_ptr);
    assert_eq!(published.navigator.process_primitives_mut().main_phase(),MainPhase::AwaitingParentTail);
    let moved=published;assert_eq!(moved.main_prefix.node(child).unwrap() as *const Node,child_ptr);assert!(moved.main_prefix.require_full_main().is_err());
}
