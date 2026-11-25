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
}
fn paths()->crate::intro::Paths{crate::intro::Paths{compatibility:crate::compatibility::Mode::StrictV14,
    installer:path("OPENMT2001_INSTALLER"),ffmpeg:path("OPENMT2001_FFMPEG"),scene:path("OPENMT2001_SCENE"),
    executable:path("OPENMT2001_EXE"),map:path("OPENMT2001_INTRO_MAP"),quality:"low".into(),language:path("OPENMT2001_LANGUAGE"),
    profile_root:path("OPENMT2001_TEXT_PROFILE"),save_root:path("OPENMT2001_TEXT_SAVES")}}
#[test]
#[ignore="supplied originals; actual409ec0 producer, tree membership and genuine NULL outputs"]
fn child5_consumes_same_owner_and_only_one_ordinary_list_with_three_texts(){
    let s=Supplied::read();let mut text=s.text();let mut nav=s.navigator();let mut images=s.images();
    let before=s.child1(&mut text,&mut nav,&mut images);let root=before.root();
    assert_eq!((before.counter(),before.nodes().count()),(48,60));
    let old=before.nodes().map(|n|(n.id,n as *const Node)).collect::<Vec<_>>();
    let children=before.node(root).unwrap().children.clone();let events=before.journal().len();
    let fonts=text.fonts() as *const Fonts;let font3=text.fonts().fonts[&3].table.as_ptr();
    let language=text.fonts().language.bytes().as_ptr();let records=images.records.as_ptr();
    let model_a=nav.process_primitives_mut().resources.model(PanelId::A).unwrap().frames[0].as_ptr();
    let model_f=nav.process_primitives_mut().resources.model(PanelId::F).unwrap().frames[0].as_ptr();
    let mut slot=Some(before);
    let done=AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut text,nav.process_primitives_mut(),&mut images).unwrap();
    assert!(slot.is_none());assert_eq!(done.phase(),MainPhase::AwaitingChild2);
    assert_eq!((done.counter(),done.nodes().count()),(49,61));assert_eq!(done.root(),root);
    let five=done.child5();assert_eq!((five.source,five.parent),(FIVE,root));
    assert_eq!(five.state24,Field::Known(0));assert_eq!(five.native_return,Field::Known(-1));
    assert_eq!(five.selector_output,Field::Known(None));assert_eq!(five.title_output,Field::Known(None));assert_eq!(five.title_name,Field::Known(None));
    let list=five.list().unwrap();let node=done.node(list).unwrap();
    assert_eq!(node.label,Label::Child5List);assert_eq!(node.family,Family::List3Texts);
    assert_eq!((node.parent,node.root,node.number,node.field18,node.shown),(Some(root),Some(root),Some(48),0,0));
    assert_eq!(&done.node(root).unwrap().children[..children.len()],children.as_slice());
    assert_eq!(&done.node(root).unwrap().children[children.len()..],&[list]);
    assert_eq!(node.bounds[0],done.main_window()[0]);assert_eq!(node.bounds[2],done.main_window()[2]);
    assert!(node.bounds[2]>done.node(done.child0().list).unwrap().bounds[2],"genuine NULL selector uses the full right edge");
    assert_eq!(node.embedded_text_contexts24.len(),3);assert!(node.embedded_text_contexts24.iter().all(|v|matches!(v,Field::Unknown)));
    let Subtype::List3Texts(l)=&node.subtype else{panic!("ordinary list, never special or Visual")};
    assert!(l.indicator.is_none());assert_eq!((l.list.primary.font,l.list.secondary.font,l.list.scratch.font),(3,3,0));
    for t in [&l.list.primary,&l.list.secondary,&l.list.scratch]{assert_eq!(t.bounds,node.bounds);assert!(matches!(t.reset_target,ResetTarget::Ret43b370));}
    assert_eq!((l.list.chosen,l.list.value,l.list.flag67c),(-1,0,0));assert!(l.list.entries.is_empty());
    assert_eq!((l.list.primary.style,l.list.secondary.style,l.list.primary.padding,l.list.primary.gap,l.list.secondary.gap),(3,2,1,2,2));
    let trace=&done.journal()[events..];
    assert_eq!(trace.iter().filter(|e|matches!(e.event,Event::Allocate(_))).count(),1);
    assert_eq!(trace.iter().filter(|e|matches!(e.event,Event::Constructor(0x440160))).count(),3);
    let index=|f:fn(&Event)->bool|trace.iter().position(|e|f(&e.event)).unwrap();
    assert!(index(|e|matches!(e,Event::Slot04(0x42fdf0)))<index(|e|matches!(e,Event::Number(48))));
    assert!(index(|e|matches!(e,Event::Number(48)))<index(|e|matches!(e,Event::Append(_))));
    assert!(index(|e|matches!(e,Event::Append(_)))<index(|e|matches!(e,Event::Alias(a) if *a==FIVE+0x28)));
    assert!(index(|e|matches!(e,Event::Alias(a) if *a==FIVE+0x28))<index(|e|matches!(e,Event::TextCall(0x4406a0))));
    assert!(!trace.iter().any(|e|matches!(e.event,Event::TextCall(0x4409c0)|Event::Attach(_)|Event::Constructor(0x431dd0))));
    for (id,pointer) in old{assert_eq!(done.node(id).unwrap() as *const Node,pointer)}
    assert_eq!(text.fonts() as *const Fonts,fonts);assert_eq!(text.fonts().fonts[&3].table.as_ptr(),font3);
    assert_eq!(text.fonts().language.bytes().as_ptr(),language);assert_eq!(images.records.as_ptr(),records);
    let p=nav.process_primitives_mut();assert_eq!(p.resources.model(PanelId::A).unwrap().frames[0].as_ptr(),model_a);assert_eq!(p.resources.model(PanelId::F).unwrap().frames[0].as_ptr(),model_f);
    assert_eq!(p.main_phase(),MainPhase::AwaitingChild2);assert_eq!(p.cells().word(A+0x388).unwrap(),49);assert_eq!(p.cells().word(FIVE+0x20).unwrap(),A);assert_eq!(p.cells().word(FIVE+0x24).unwrap(),0);
    assert_eq!(p.cells().read(FIVE+0x28,16).unwrap(),&five.static_preimage[0x28..0x38],"typed alias does not forge a heap pointer");
    assert_eq!(five.static_preimage.as_slice(),Cells::process_allocation(&s.exe).unwrap().read(FIVE,0x38).unwrap());
    assert_eq!(p.cells().byte(A+0x184).unwrap(),0);assert_eq!(p.steps.iter().filter(|s|**s==Step::Bind(A)).count(),1);
    assert!(done.require_full_main().is_err());assert!(done.require_input_or_render().is_err());assert!(p.require_process_graph().is_err());
}
#[test]
#[ignore="supplied originals; real direct-index/cache divergence and direct4406a0"]
fn child5_direct_layout_uses_same_current_font_and_preserves_cached_selector(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut i=s.images();
    let prefix=s.child1(&mut t,&mut n,&mut i);assert_eq!(t.fonts().selector.cached,3);
    t.fonts_mut().direct_index(0).unwrap(); //real4e7470 leaves455420 cache unchanged
    let mut slot=Some(prefix);let done=AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).unwrap();
    let Subtype::List3Texts(l)=&done.node(done.child5().list().unwrap()).unwrap().subtype else{panic!()};
    let glyph=usize::from(t.fonts().language.bytes()[usize::from(b'W'-32)]);let at=4+glyph*44+36;
    let height=i32::from_le_bytes(t.fonts().fonts[&0].table[at..at+4].try_into().unwrap());
    assert_eq!(l.list.primary.pitch,height);assert_eq!(l.list.primary.extent,[0.;4]);
    assert_eq!((l.list.primary.font,l.list.secondary.font,l.list.scratch.font),(3,3,0));
    assert_eq!((l.list.secondary.pitch,l.list.scratch.pitch),(10,10));
    assert_eq!((t.fonts().selector.cached,t.fonts().current),(3,0),"both set_font3 calls take actual cache-hit RET");
}
#[test]
#[ignore="supplied originals; exact409f00 scalar store retains actual list/Texts/rows"]
fn child5_reset_clears_only_state_and_retains_owned_list_rows(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let mut slot=Some(s.child1(&mut t,&mut n,&mut i));
    let mut done=AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).unwrap();
    let id=done.child5().list().unwrap();
    let p=&mut done.prefix.prefix.inner;let Subtype::List3Texts(l)=&mut p.node_mut(id).unwrap().subtype else{panic!()};
    //Actual supplied packed-row append and retained capacity producer, no
    //synthetic world/model/font or fake success callback.
    l.list.primary.identifier(t.fonts_mut(),0x45f).unwrap();l.list.ensure().unwrap();
    let pointer=&l.list as *const List<NodeId>;let row_pointer=l.list.entries.as_ptr();let primary=&l.list.primary as *const Text;
    let bytes=l.list.primary.group.bytes.clone();let offsets=l.list.primary.group.offsets.clone();let count=l.list.primary.group.count;
    let entries=format!("{:?}",l.list.entries);let nodes=p.nodes.len();let counter=p.counter;let children=p.node(p.root).unwrap().children.clone();
    p.child5.as_mut().unwrap().state24=Field::Known(u32::MAX); //reset scalar canary only
    n.process_primitives_mut().test_child5_state_word(u32::MAX).unwrap();
    let before=n.process_primitives_mut().cells().read(FIVE,0x38).unwrap().to_vec();
    done.reset_child5(n.process_primitives_mut()).unwrap();
    assert_eq!(done.child5().state24,Field::Known(0));assert_eq!(done.child5().list().unwrap(),id);
    let Subtype::List3Texts(l)=&done.node(id).unwrap().subtype else{panic!()};
    assert_eq!(&l.list as *const List<NodeId>,pointer);assert_eq!(l.list.entries.as_ptr(),row_pointer);assert_eq!(&l.list.primary as *const Text,primary);
    assert_eq!(l.list.primary.group.bytes,bytes);assert_eq!(l.list.primary.group.offsets,offsets);assert_eq!(l.list.primary.group.count,count);assert!(count>0);
    assert_eq!(format!("{:?}",l.list.entries),entries);assert_eq!((done.nodes().count(),done.counter()),(nodes,counter));assert_eq!(done.node(done.root()).unwrap().children,children);
    let after=n.process_primitives_mut().cells().read(FIVE,0x38).unwrap();assert_eq!(&after[..0x24],&before[..0x24]);assert_eq!(&after[0x28..],&before[0x28..]);assert_eq!(&after[0x24..0x28],&[0;4]);
    let mut foreign=s.navigator();assert!(done.reset_child5(foreign.process_primitives_mut()).is_err());
}
#[test]
#[ignore="supplied originals; invalid/foreign source, resource owners and duplicate continuation"]
fn child5_rejects_before_moves_and_reached_failure_is_retained_without_replay(){
    let s=Supplied::read();let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let prefix=s.child1(&mut t,&mut n,&mut i);
    let root=prefix.root();let counter=prefix.counter();let events=prefix.journal().len();let mut slot=Some(prefix);
    let scratch=t.fonts().scratch.clone();let mut foreign_t=s.text();let mut foreign_i=s.images();let mut foreign_n=s.navigator();
    assert!(AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut foreign_t,n.process_primitives_mut(),&mut i).is_err());
    assert!(AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut foreign_i).is_err());
    assert!(AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut t,foreign_n.process_primitives_mut(),&mut i).is_err());
    assert!(AwaitingChild2::continue_child5(&mut slot,b"invalid PE",&mut t,n.process_primitives_mut(),&mut i).is_err());
    let mut wrong=s.exe.clone();let original=PeImage::parse(&s.exe).unwrap();let code=original.at(0x409ec0,24).unwrap();
    let at=wrong.windows(code.len()).position(|b|b==code).unwrap();wrong[at]^=1;
    assert!(AwaitingChild2::continue_child5(&mut slot,&wrong,&mut t,n.process_primitives_mut(),&mut i).err().unwrap().contains("source409ec0"));
    assert_eq!((slot.as_ref().unwrap().root(),slot.as_ref().unwrap().counter(),slot.as_ref().unwrap().journal().len()),(root,counter,events));
    assert_eq!(t.fonts().scratch,scratch);assert_eq!(n.process_primitives_mut().main_phase(),MainPhase::AwaitingChild5);
    let retained=AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).unwrap();
    assert!(AwaitingChild2::continue_child5(&mut slot,&s.exe,&mut t,n.process_primitives_mut(),&mut i).is_err());assert_eq!(retained.root(),root);
    //Same resource owner, but the reached current metric table is missing.
    let mut t=s.text();let mut n=s.navigator();let mut i=s.images();let mut failed=Some(s.child1(&mut t,&mut n,&mut i));let root=failed.as_ref().unwrap().root();
    let current=t.fonts().current;let removed=t.fonts_mut().fonts.remove(&current).unwrap();
    assert!(AwaitingChild2::continue_child5(&mut failed,&s.exe,&mut t,n.process_primitives_mut(),&mut i).is_err());
    let candidate=failed.as_ref().unwrap();assert_eq!(candidate.phase(),MainPhase::Failed);assert_eq!(candidate.root(),root);assert_eq!(n.process_primitives_mut().main_phase(),MainPhase::Failed);
    let five=candidate.prefix.inner.child5.as_ref().unwrap();let list=five.list().unwrap();assert!(matches!(candidate.node(list).unwrap().subtype,Subtype::List3Texts(_)));
    assert_eq!(five.state24,Field::Unknown);assert_eq!(five.native_return,Field::Unknown);assert!(candidate.node(root).unwrap().children.contains(&list));
    let events=candidate.journal().len();let counter=candidate.counter();let nodes=candidate.nodes().count();let scratch=t.fonts().scratch.clone();
    t.fonts_mut().fonts.insert(current,removed);
    assert!(AwaitingChild2::continue_child5(&mut failed,&s.exe,&mut t,n.process_primitives_mut(),&mut i).is_err());
    assert_eq!((failed.as_ref().unwrap().journal().len(),failed.as_ref().unwrap().counter(),failed.as_ref().unwrap().nodes().count()),(events,counter,nodes));assert_eq!(t.fonts().scratch,scratch);
}
#[test]
#[ignore="requires supplied Assets::load inputs and ffmpeg"]
fn actual_assets_load_keeps_child5_on_same_process_and_rejects_incomplete_main(){
    let assets=crate::intro::Assets::load(paths()).unwrap();
    assert_eq!(assets.main_prefix.phase(),MainPhase::AwaitingParentTail);assert_eq!((assets.main_prefix.counter(),assets.main_prefix.nodes().count()),(69,83));
    let root=assets.main_prefix.root();let list=assets.main_prefix.child5().list().unwrap();let node=assets.main_prefix.node(list).unwrap() as *const Node;
    let fonts=assets.ui_text.fonts() as *const Fonts;let rows=assets.ui_text.fonts().language.bytes().as_ptr();let images=assets.ui_images.records.as_ptr();
    let mut moved=Box::new(assets);
    assert_eq!(moved.main_prefix.root(),root);assert_eq!(moved.main_prefix.node(list).unwrap() as *const Node,node);assert_eq!(moved.ui_text.fonts() as *const Fonts,fonts);
    assert_eq!(moved.ui_text.fonts().language.bytes().as_ptr(),rows);assert_eq!(moved.ui_images.records.as_ptr(),images);
    assert_eq!(moved.navigator.process_primitives_mut().main_phase(),MainPhase::AwaitingParentTail);
    assert!(moved.main_prefix.require_full_main().is_err());assert!(moved.main_prefix.require_input_or_render().is_err());
    let mut missing=paths();missing.executable=path("OPENMT2001_TEXT_PROFILE").join("intentionally-missing-original.exe");assert!(crate::intro::Assets::load(missing).is_err());
    assert_eq!(moved.main_prefix.node(list).unwrap() as *const Node,node);assert_eq!(moved.navigator.process_primitives_mut().cells().byte(A+0x184).unwrap(),0);
}
