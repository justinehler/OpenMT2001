//! Supplied-source/resource construction regressions.
use super::*;
use crate::intro::camera_live::{case_zero_text::ResetTarget,primitive_factory::Step,process_image::Cells};
use std::{fs,path::PathBuf};
fn path(key:&str)->PathBuf{std::env::var_os(key).map(PathBuf::from).unwrap_or_else(||panic!("required {key}"))}
struct Supplied{exe:Vec<u8>,archive:Vec<u8>,index:ArchiveIndex,installer:PathBuf,language:PathBuf}
impl Supplied{
    fn read()->Self{
        let exe=fs::read(path("OPENMT2001_EXE")).unwrap();let directory=fs::read(path("OPENMT2001_ARCHIVE_DIR")).unwrap();
        let archive=fs::read(path("OPENMT2001_ARCHIVE_BIN")).unwrap();let index=ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
        Self{exe,archive,index,installer:path("OPENMT2001_INSTALLER"),language:path("OPENMT2001_LANGUAGE")}
    }
    fn text(&self)->UiTextResources{let mut t=UiTextResources::load(&self.exe,&self.installer,&self.language,&self.index,&self.archive).unwrap();t.install_usa_board(&self.exe,&self.installer).unwrap();t}
    fn images(&self)->Images{Images::load(&self.exe,&self.index,&self.archive,[640,480]).unwrap()}
    fn navigator(&self)->crate::navigator::Navigator{
        let board=openmt2001::board::Board::from_v14(&PeImage::parse(&self.exe).unwrap()).unwrap();
        crate::navigator::Navigator::load(&self.index,&self.archive,&board.blocks,&self.exe).unwrap()
    }
    fn child1(&self,t:&mut UiTextResources,n:&mut crate::navigator::Navigator,i:&mut Images)->AwaitingChild5{
        let mut slot=Some(Prefix::load(&self.exe,&self.index,&self.archive,[640,480],t,n.process_primitives_mut(),i).unwrap());
        AwaitingChild5::continue_child1(&mut slot,&self.exe,t,n.process_primitives_mut(),i).unwrap()
    }
    fn child5(&self,t:&mut UiTextResources,n:&mut crate::navigator::Navigator,i:&mut Images)->AwaitingChild2{
        let mut slot=Some(self.child1(t,n,i));AwaitingChild2::continue_child5(&mut slot,&self.exe,t,n.process_primitives_mut(),i).unwrap()
    }
}
fn paths()->crate::intro::Paths{crate::intro::Paths{compatibility:crate::compatibility::Mode::StrictV14,
    installer:path("OPENMT2001_INSTALLER"),ffmpeg:path("OPENMT2001_FFMPEG"),scene:path("OPENMT2001_SCENE"),
    executable:path("OPENMT2001_EXE"),map:path("OPENMT2001_INTRO_MAP"),quality:"low".into(),language:path("OPENMT2001_LANGUAGE"),
    profile_root:path("OPENMT2001_TEXT_PROFILE"),save_root:path("OPENMT2001_TEXT_SAVES")}}
#[test]
#[ignore="supplied originals; actual408da0 containing factory and all hidden owners"]
fn child2_retains_same_allocations_and_all_twenty_root_products_in_source_order(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut images=s.images();let before=s.child5(&mut t,&mut n,&mut images);
    assert_eq!((before.counter(),before.nodes().count()),(49,61));
    let root=before.root();let old=before.nodes().map(|n|(n.id,n as *const Node)).collect::<Vec<_>>();
    let old_children=before.node(root).unwrap().children.clone();let start=before.journal().len();
    let fonts=t.fonts() as *const Fonts;let language=t.fonts().language.bytes().as_ptr();let records=images.records.as_ptr();
    let tables=t.fonts().fonts.iter().map(|(&i,f)|(i,f.table.as_ptr())).collect::<Vec<_>>();
    let a=n.process_primitives_mut().resources.model(PanelId::A).unwrap().frames[0].as_ptr();
    let f=n.process_primitives_mut().resources.model(PanelId::F).unwrap().frames[0].as_ptr();
    let title=before.alias(0x4b8).unwrap();let title_ptr=before.node(title).unwrap() as *const Node;
    let mut slot=Some(before);let done=AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut images).unwrap();
    assert!(slot.is_none());assert_eq!(done.phase(),MainPhase::AwaitingParentTail);assert_eq!((done.counter(),done.nodes().count()),(69,83));
    assert_eq!(done.root(),root);assert_eq!(done.child2().parent,root);assert_eq!(done.child2().source,BUSINESS);assert_eq!(done.child2().native_return,Field::Known(-1));
    let offsets=[0x50,0x54,0x58,0x1c,0x20,0x24,0x64,0xc4,0xbc,0xc0,0xc8,0x9c,0xa0,0xa4,0xac,0x98,0xb8,0x88,0x8c,0x90];
    let expected=offsets.map(|o|done.child2().field(o).unwrap());
    assert_eq!(done.child2().fields.len(),offsets.len());
    let children=&done.node(root).unwrap().children;assert_eq!(&children[..old_children.len()],old_children.as_slice());assert_eq!(&children[old_children.len()..],&expected);
    for (index,id) in expected.iter().enumerate(){let node=done.node(*id).unwrap();assert_eq!(node.label,Label::Business(offsets[index]));assert_eq!(node.parent,Some(root));assert_eq!(node.root,Some(root));assert_eq!(node.number,Some(49+index as u32));}
    for (id,ptr) in old{assert_eq!(done.node(id).unwrap() as *const Node,ptr)}
    assert_eq!(done.alias(0x4b8).unwrap(),title);assert_eq!(done.node(title).unwrap() as *const Node,title_ptr);
    assert_eq!(t.fonts() as *const Fonts,fonts);assert_eq!(t.fonts().language.bytes().as_ptr(),language);assert_eq!(images.records.as_ptr(),records);
    for (i,ptr) in tables{assert_eq!(t.fonts().fonts[&i].table.as_ptr(),ptr)}
    let p=n.process_primitives_mut();assert_eq!(p.resources.model(PanelId::A).unwrap().frames[0].as_ptr(),a);assert_eq!(p.resources.model(PanelId::F).unwrap().frames[0].as_ptr(),f);
    assert_eq!(p.cells().word(A+0x388).unwrap(),69);assert_eq!(p.cells().byte(A+0x184).unwrap(),0);
    assert_eq!(done.child2().static_preimage.as_slice(),Cells::process_allocation(&s.exe).unwrap().read(BUSINESS,0xd0).unwrap());
    for o in offsets{assert_eq!(p.cells().word(BUSINESS+o).unwrap(),0,"modern typed aliases never forge native heap pointers");}
    let amount=done.child1().field(0xb0).unwrap();let node=done.node(amount).unwrap();
    let Subtype::Amount2Visuals(a)=&node.subtype else{panic!("retained actual42ebe0 amount subtype")};
    let (Field::Known(first),Field::Known(second))=(&a.first150,&a.second154) else{panic!()};
    assert_eq!(node.children,vec![*first,*second]);assert_eq!(done.node(*first).unwrap().label,Label::AmountFirst150);assert_eq!(done.node(*second).unwrap().label,Label::AmountSecond154);
    assert!(done.node(*first).unwrap().bounds[0]>done.node(*second).unwrap().bounds[0],"retained first150 is source right extent");
    assert_eq!(p.cells().word(BUSINESS+0x44).unwrap(),A);assert_eq!(p.main_phase(),MainPhase::AwaitingParentTail);
    assert_eq!(p.steps.iter().filter(|s|**s==Step::Bind(A)).count(),1);assert!(p.require_process_graph().is_err());assert!(done.require_full_main().is_err());assert!(done.require_input_or_render().is_err());assert!(done.child2().require_selected_business().is_err());
    let trace=&done.journal()[start..];assert_eq!(trace.iter().filter(|e|matches!(e.event,Event::Allocate(_))).count(),22);
    let reset=trace.iter().position(|e|matches!(e.event,Event::Geometry(0x409d20))).unwrap();assert!(matches!(trace.last().unwrap().event,Event::Stop));assert!(reset<trace.len()-1);
    assert!(!trace.iter().any(|e|matches!(e.event,Event::TextCall(0x4409c0))));
}
#[test]
#[ignore="supplied originals; real subtype/selector slot160 and currency buffer"]
fn child2_subtypes_captions_colors_images_and_aliases_remain_concrete(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut images=s.images();let mut slot=Some(s.child5(&mut t,&mut n,&mut images));
    let done=AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut images).unwrap();let b=done.child2();
    for o in [0x50,0x54,0x58,0x24,0xc4,0xbc,0xc0,0x9c,0xa0,0xa4]{let node=done.node(b.field(o).unwrap()).unwrap();assert_eq!(node.family,Family::Text);let Subtype::Text(text)=&node.subtype else{panic!()};assert_eq!(text.font,3);assert!(matches!(text.reset_target,ResetTarget::Ret43b370));}
    for o in [0x64,0x88,0x8c]{let node=done.node(b.field(o).unwrap()).unwrap();let Subtype::List3Texts(l)=&node.subtype else{panic!("ordinary42f3d0")};assert_eq!((l.list.primary.font,l.list.secondary.font,l.list.scratch.font),(3,3,0));assert_eq!((l.list.secondary.inset,l.list.secondary.scroll),(4,3));assert_eq!(node.embedded_text_contexts24.len(),3);assert!(l.indicator.is_none());}
    let list=b.field(0x98).unwrap();let selector=b.field(0xb8).unwrap();let Subtype::SpecialList3Texts(l)=&done.node(list).unwrap().subtype else{panic!("430790 special")};assert_eq!(l.indicator,Some(selector));assert_eq!(l.selectable680,1);
    let Subtype::Selector2Arrows(sel)=&done.node(selector).unwrap().subtype else{panic!()};assert_eq!(sel.first_list,None);assert_eq!(sel.last_list,Some(list));assert_eq!(sel.admitted191,0);
    for arrow in [sel.previous.unwrap(),sel.next.unwrap()]{let node=done.node(arrow).unwrap();assert_eq!(node.parent,Some(selector));assert!(matches!(node.subtype,Subtype::Visual(_)));assert_eq!(node.disabled,1);}
    for (o,id,global) in [(0xc8,0x58,Some(0x86d3dc)),(0xac,0x55,Some(0x86d3d8)),(0x90,0x587,None)]{
        let key=b.field(o).unwrap();let node=done.node(key).unwrap();let Subtype::Visual(v)=&node.subtype else{panic!()};
        assert_eq!(v.caption_identifier2f4,Field::Known(id));assert_eq!((v.caption.font,v.caption.style,v.caption_mode),(2,4,3));assert!(v.caption.group.count>0);assert!(v.caption.group.bytes.iter().any(|&b|b!=0));
        assert!(v.caption.colors[..v.caption.group.count as usize].iter().all(|&c|c==0));assert_eq!(v.images,[Some(images.id(0x68af58).unwrap()),Some(images.id(0x68af60).unwrap()),Some(images.id(0x68b064).unwrap()),Some(images.id(0x68af5c).unwrap()),None]);
        if let Some(global)=global{assert_eq!(b.global(global),Some(key))}
        let trace=done.journal().iter().filter(|e|e.node==key).collect::<Vec<_>>();let at=|event|trace.iter().position(|e|e.event==event).unwrap();
        assert!(at(Event::TextCall(0x441940))<at(Event::TextCall(0x441220)));assert!(at(Event::TextCall(0x440330))<at(Event::CaptionStore(id)));
    }
    assert_eq!(b.global(0x86d3d0),Some(b.field(0x1c).unwrap()));assert_eq!(b.global(0x86d3d4),Some(b.field(0x20).unwrap()));assert_eq!(b.global(0x86d3e0),Some(list));
    let Subtype::Visual(left)=&done.node(b.field(0x1c).unwrap()).unwrap().subtype else{panic!()};assert_eq!(left.caption_mode,3);
    let Subtype::Visual(right)=&done.node(b.field(0x20).unwrap()).unwrap().subtype else{panic!()};assert_eq!(right.caption_mode,2,"source has no second navigation442ab0");
    let p=n.process_primitives_mut();let mut expected=p.cells().ntbs(0x57e6ec,1024).unwrap();if p.cells().word(0x59050c).unwrap()==0{expected.push(b'0')}else{expected.insert(0,b'0')};expected.push(0);
    assert_eq!(b.currency_buffer,Field::Known(expected.clone()));assert_eq!(p.cells().read(0x57f734,expected.len()).unwrap(),expected);
    for o in [0xc0,0xa4]{let Subtype::Text(text)=&done.node(b.field(o).unwrap()).unwrap().subtype else{panic!()};assert!(text.group.count>0);assert_eq!((text.colors[0],text.blink_colors[0],text.blink[0]),(9,0,0));}
    let Subtype::Text(title)=&done.node(done.alias(0x4b8).unwrap()).unwrap().subtype else{panic!("shared parent4b8 must be Text")};assert_eq!(title.font,3);assert!(title.group.count>0);assert!(title.colors[..title.group.count as usize].iter().all(|&c|c==16));
}
#[test]
#[ignore="supplied originals; mandatory layout reset null guard and scalar/control effects"]
fn child2_layout_reset_retains_hidden_owners_and_uses_independent590f60(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let mut slot=Some(s.child5(&mut t,&mut n,&mut i));
    assert_eq!(n.process_primitives_mut().cells().word(0x822d50).unwrap(),0);assert_eq!(n.process_primitives_mut().cells().word(0x590f60).unwrap(),0);
    let done=AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).unwrap();let b=done.child2();
    for o in [0x90,0x88,0x8c,0xac,0xc8,0x50,0x54,0x58,0x1c,0x20,0x98]{assert_eq!(done.node(b.field(o).unwrap()).unwrap().shown,0)}
    assert_eq!(done.node(b.field(0x64).unwrap()).unwrap().shown,1);
    for o in [0xc4,0xbc,0xc0,0x9c,0xa0]{let id=b.field(o).unwrap();assert!(done.node(done.root()).unwrap().children.contains(&id));assert!(matches!(done.node(id).unwrap().subtype,Subtype::Text(_)));}
    for o in [0xac,0xc8]{let Subtype::Visual(v)=&done.node(b.field(o).unwrap()).unwrap().subtype else{panic!()};assert_eq!((v.disabled333,v.down335),(1,0));}
    let p=n.process_primitives_mut();for o in [0x28,0x2c,0x34,0x48]{assert_eq!(p.cells().word(BUSINESS+o).unwrap(),0)}assert_eq!(p.cells().byte(BUSINESS+0x4c).unwrap(),0);assert_eq!(p.cells().word(BUSINESS+0xb4).unwrap(),u32::MAX);
    assert_eq!(done.journal().iter().filter(|e|matches!(e.event,Event::TextCall(0x43a8c0))).count(),4,"one layout plus three reset/exit currency calls");
    //A different fixture retains actual822d50 NULL but sets590f60 NONNULL.
    //This is a negative guard canary, never invented selected-branch success.
    let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let before=s.child5(&mut t,&mut n,&mut i);let events=before.journal().len();let mut slot=Some(before);
    n.process_primitives_mut().test_business_guard_nonnull().unwrap();assert_eq!(n.process_primitives_mut().cells().word(0x822d50).unwrap(),0);
    let error=AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).err().unwrap();assert!(error.contains("selected590f60"));
    assert_eq!(slot.as_ref().unwrap().journal().len(),events);assert!(slot.as_ref().unwrap().prefix.prefix.inner.child2.is_none());assert_eq!(n.process_primitives_mut().main_phase(),MainPhase::AwaitingChild2);
}
#[test]
#[ignore="supplied originals; source/resource/phase/foreign-owner/replay validation"]
fn child2_rejects_foreign_source_owners_and_duplicate_without_consuming_candidate(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let before=s.child5(&mut t,&mut n,&mut i);let root=before.root();let events=before.journal().len();let mut slot=Some(before);
    let mut foreign_t=s.text();let mut foreign_n=s.navigator();let mut foreign_i=s.images();let scratch=t.fonts().scratch.clone();
    assert!(AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut foreign_t,n.process_primitives_mut(),&mut i).is_err());
    assert!(AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,foreign_n.process_primitives_mut(),&mut i).is_err());
    assert!(AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut foreign_i).is_err());
    assert!(AwaitingParentTail::continue_child2(&mut slot,b"invalid PE",&mut t,n.process_primitives_mut(),&mut i).is_err());
    let mut wrong=s.exe.clone();let original=PeImage::parse(&s.exe).unwrap();let code=original.at(0x408da0,24).unwrap();let at=wrong.windows(code.len()).position(|b|b==code).unwrap();wrong[at]^=1;
    assert!(AwaitingParentTail::continue_child2(&mut slot,&wrong,&mut t,n.process_primitives_mut(),&mut i).err().unwrap().contains("source408da0"));
    assert_eq!((slot.as_ref().unwrap().root(),slot.as_ref().unwrap().journal().len()),(root,events));assert_eq!(t.fonts().scratch,scratch);assert_eq!(n.process_primitives_mut().main_phase(),MainPhase::AwaitingChild2);
    let done=AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).unwrap();assert_eq!(done.root(),root);assert!(slot.is_none());
    assert!(AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).is_err());
}
#[test]
#[ignore="supplied originals; reached failure retains partial boxed products and rejects replay"]
fn child2_failed_font_candidate_retains_original_slot_and_partial_products(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let before=s.child5(&mut t,&mut n,&mut i);let root=before.root();let old=before.nodes().map(|n|(n.id,n as *const Node)).collect::<Vec<_>>();let mut slot=Some(before);
    let removed=t.fonts_mut().fonts.remove(&2).unwrap();
    assert!(AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).is_err());let retained=slot.as_ref().unwrap();
    assert_eq!(retained.root(),root);assert_eq!(retained.phase(),MainPhase::Failed);assert_eq!(n.process_primitives_mut().main_phase(),MainPhase::Failed);
    let child=retained.prefix.prefix.inner.child2.as_ref().unwrap();assert_eq!(child.native_return,Field::Unknown);let c8=child.field(0xc8).unwrap();assert!(retained.node(root).unwrap().children.contains(&c8));
    for (id,ptr) in old{assert_eq!(retained.node(id).unwrap() as *const Node,ptr)}
    let state=(retained.counter(),retained.nodes().count(),retained.journal().len());t.fonts_mut().fonts.insert(2,removed);
    assert!(AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).is_err());let retained=slot.as_ref().unwrap();assert_eq!((retained.counter(),retained.nodes().count(),retained.journal().len()),state);
}
#[test]
#[ignore="real Assets::load supplied resources/ffmpeg; no application/original execution"]
fn actual_assets_calls_child2_once_and_failed_replacement_preserves_live_product(){
    let mut published=Box::new(crate::intro::Assets::load(paths()).unwrap());assert_eq!(published.main_prefix.phase(),MainPhase::AwaitingBinding);assert_eq!((published.main_prefix.counter(),published.main_prefix.nodes().count()),(71,85));
    let root=published.main_prefix.root();let child=published.main_prefix.child2().field(0xa4).unwrap();let ptr=published.main_prefix.node(child).unwrap() as *const Node;let fonts=published.ui_text.fonts() as *const Fonts;
    let mut missing=paths();missing.executable=path("OPENMT2001_TEXT_PROFILE").join("intentionally-missing-original.exe");
    if let Ok(candidate)=crate::intro::Assets::load(missing){published=Box::new(candidate);panic!("failed candidate unexpectedly published")}
    assert_eq!(published.main_prefix.root(),root);assert_eq!(published.main_prefix.node(child).unwrap() as *const Node,ptr);assert_eq!(published.ui_text.fonts() as *const Fonts,fonts);
    assert_eq!(published.navigator.process_primitives_mut().main_phase(),MainPhase::AwaitingBinding);assert_eq!(published.navigator.process_primitives_mut().cells().byte(A+0x184).unwrap(),0);
    assert_eq!(published.navigator.process_primitives_mut().steps.iter().filter(|s|**s==Step::Bind(A)).count(),1);assert!(published.main_prefix.require_full_main().is_err());
}
#[test]
#[ignore="supplied originals; stack-resolved TOP coordinates and retained FST geometry"]
fn child2_geometry_preserves_stack_resolved_top_shared_extents_and_true_pair_y(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let mut slot=Some(s.child5(&mut t,&mut n,&mut i));
    let done=AwaitingParentTail::continue_child2(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).unwrap();let b=done.child2();let m=done.main_window();let pe=PeImage::parse(&s.exe).unwrap();
    let bounds=|o|done.node(b.field(o).unwrap()).unwrap().bounds;
    assert_eq!(bounds(0x50),bounds(0x54));assert_eq!(bounds(0x54),bounds(0x58));
    assert_eq!(bounds(0xc4),bounds(0x9c));assert_eq!(bounds(0xbc),bounds(0xc0));assert_eq!(bounds(0xc0),bounds(0xa0));assert_eq!(bounds(0xa0),bounds(0xa4));assert_eq!(bounds(0xc8),bounds(0xac));
    //Resolve effective addresses from the original stable stack frame:
    //both titles load S+28h after push eax; pair y reloads that row after push0.
    let right=store(sub(m[2],float(&pe,0x52501c).unwrap()).unwrap()).unwrap();
    assert_eq!(bounds(0xc4)[2].to_bits(),right.to_bits());
    let title_top=add(m[1],float(&pe,0x525020).unwrap()).unwrap();
    let expected_bottom=add(title_top,float(&pe,0x525078).unwrap()).unwrap();
    assert_eq!(bounds(0xc4)[1].to_bits(),title_top.to_bits());
    assert_eq!(bounds(0xc4)[3].to_bits(),expected_bottom.to_bits());
    let row_top=add(title_top,float(&pe,0x525098).unwrap()).unwrap();
    let mut y=Sum::from_f32(row_top).unwrap();y.add_f32(m[1]).unwrap();y.add_f32(float(&pe,0x525078).unwrap()).unwrap();
    let y=super::super::super::super::super::case_zero_text::ftol_layout(y).unwrap();
    let expected_top=Sum::from_f64(f64::from(y as u32)).unwrap().to_f32();assert_eq!(bounds(0x98)[1].to_bits(),expected_top.to_bits());
    assert_eq!(bounds(0x88)[1],m[1]);assert_eq!(bounds(0x88)[3].to_bits(),add(m[1],float(&pe,0x525058).unwrap()).unwrap().to_bits());
    let mut top=Sum::from_f32(m[1]).unwrap();top.add_f32(float(&pe,0x525078).unwrap()).unwrap();let stored=top.to_f32();top.add_f32(float(&pe,0x525058).unwrap()).unwrap();
    assert_eq!(bounds(0x8c)[1].to_bits(),stored.to_bits());assert_eq!(bounds(0x8c)[3].to_bits(),top.to_f32().to_bits());
    assert_eq!(i.record(i.id(0x68af58).unwrap()).unwrap().scale,Some(1.));assert_eq!(i.record(i.id(0x68afac).unwrap()).unwrap().scale,Some(1.));
    assert_eq!((t.fonts().current,t.fonts().selector.cached),(3,3));assert!(t.fonts().fonts.values().all(|f|f.scale==1.));
}
