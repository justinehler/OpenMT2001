//! Requires supplied immutable originals and the exact source composition.
//! Fixture hooks replaced factories; fixture geometry is not ownership proof.
use super::*;
use super::super::super::{eventlist_construction::{self as eventlist,Effect,Label},case_zero_text::{Text,ErrorWord}};
const E:u32=0x571378;const V:u32=0x573b38;
fn controlled(input:&Inputs)->openmt2001::level_startup::World{
    let mut w=input.world();for i in 0..52{row(&mut w,i,0,1,46);}row(&mut w,41,0,1,0);w
}
#[test]
#[ignore="supplied actual static loader birth/nonwriters"]
fn static_birth_retains_alias_preimages_caption_and_unique_movable_owner(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let event=primary.eventlist();
    assert_eq!(event.phase(),eventlist::Phase::Unbound);assert!(event.children().is_empty());
    assert_eq!(event.nodes().count(),0);assert_eq!(event.alias(0x464),None);
    let loader=super::super::super::process_image::Cells::process_allocation(&input.exe).unwrap();
    for offset in (0x464..0x490).step_by(4){assert_eq!(p.cells().read(E+offset,4).unwrap(),loader.read(E+offset,4).unwrap());}
    for offset in [0x388,0x39c,0x3b0,0x3c4,0x3d8,0x3ec,0x400,0x414,0x428,0x43c,0x450]{
        let actual:[u32;5]=std::array::from_fn(|i|p.cells().word(E+offset+4*i as u32).unwrap());
        assert_eq!(actual,[0,0,0,0,0x3f000000]);
    }
    let owner=event as *const _;let caption=&p.texts[&(E+0x188)] as *const _;let fonts=text.fonts() as *const _;
    let moved=Box::new((primary,p,text,images));
    assert_eq!(moved.0.eventlist() as *const _,owner);assert_eq!(&moved.1.texts[&(E+0x188)] as *const _,caption);
    assert_eq!(moved.2.fonts() as *const _,fonts);assert_eq!(moved.1.panels[&E].caption,E+0x188);assert_unbound(&moved.1);
}
#[test]
#[ignore="actual protected Install + retained native eleven-call fixture"]
fn concrete_original_eleven_texts_return_before_suffix_and_registration(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let w=controlled(&input);
    let owner=primary.eventlist() as *const _;let caption=&p.texts[&(E+0x188)] as *const _;
    let fonts=text.fonts() as *const _;let model=p.resources.model(PanelId::OtherPrimary(2)).unwrap().frames.as_ptr();
    let scratch=text.fonts().scratch.clone();let error=text.fonts().error;
    let (primary,p,text,_,result)=run(primary,p,text,images,&w);
    assert_eq!(result.unwrap(),InstallDisposition::CompletedChat{epoch:w.topology.epoch().serial()});
    assert_eq!(primary.phase(),PrimaryPhase::AwaitingG);assert_unbound(&p);
    let e=primary.eventlist();assert_eq!(e as *const _,owner);assert_eq!(e.phase(),eventlist::Phase::Registered);
    assert_eq!(&p.texts[&(E+0x188)] as *const _,caption);assert_eq!(p.texts[&(E+0x188)].style,4);
    assert_eq!(text.fonts() as *const _,fonts);assert_eq!(p.resources.model(PanelId::OtherPrimary(2)).unwrap().frames.as_ptr(),model);
    //Voting and Chat literals legitimately write scratch around EventList; the full
    //operation preserves error, but is not a scratch no-write oracle.
    let _pre_voting_scratch=scratch;assert_eq!(text.fonts().error,error);
    assert_eq!(e.children().len(),11);assert_eq!(e.nodes().count(),11);
    //Retained native hook output is independent of this candidate/model
    //algorithm. Root preflights its hash; no embedded asset/English oracle.
    let fixture=fs::read(path("OPENMT2001_EVENTLIST_LAYOUT")).unwrap();
    assert_eq!(&fixture[..8],b"MTEVLST1");
    let word=|at:usize|u32::from_le_bytes(fixture[at..at+4].try_into().unwrap());
    assert_eq!(word(8),0x037f);assert_eq!(word(12),11);
    for (number,id) in e.children().iter().copied().enumerate(){
        let n=e.node(id).unwrap();let t=n.text();let base=16+44*number;assert_eq!(word(base),number as u32);
        assert_eq!(n.number,Some(number as u32));assert_eq!(n.parent,E);assert_eq!(n.root,E);
        assert_eq!(n.next,e.children().get(number+1).copied());assert_eq!(n.allocation_size,0x1b4);
        for axis in 0..4{assert_eq!(t.bounds[axis].to_bits(),word(base+4+4*axis));}
        assert_eq!(t.depth.to_bits(),word(base+20));assert_eq!(t.font,word(base+24));assert_eq!(t.style,word(base+32));
        assert_eq!(word(base+28),if number==10{1501}else if number%2==0{1491+number as u32/2}else{1496+number as u32/2});
        assert_eq!((word(base+36)as i32,word(base+40)as i32),(-999,-999));
        assert_eq!((t.pitch,t.visible,t.y,t.inset,t.scroll,t.dirty,t.geometry_dirty),(10,64,0,0,0,0,0));
        assert_eq!(t.extent,[0.;4]);assert!(!t.group.row(0).unwrap().is_empty());
        assert_eq!(t.colors.len(),t.x.len());assert_eq!(t.blink_colors.len(),t.x.len());assert_eq!(t.blink.len(),t.x.len());
        assert!(t.x.len()>=t.group.count as usize);assert!(t.x.iter().all(Option::is_none));
        assert!(t.colors.iter().chain(t.blink_colors).chain(t.blink).all(|b|*b==0));
    }
    for i in 0..5u32{
        assert_eq!(e.alias(0x468+4*i),Some(e.children()[2*i as usize]));
        assert_eq!(e.alias(0x47c+4*i),Some(e.children()[2*i as usize+1]));
        assert_eq!(e.node(e.children()[2*i as usize]).unwrap().label,Label::Green(i as usize));
        assert_eq!(e.node(e.children()[2*i as usize+1]).unwrap().label,Label::Body(i as usize));
    }
    assert_eq!(e.alias(0x464),Some(e.children()[10]));
    let actual=e.effects().iter().filter_map(|x|if let Effect::Query{name,..}=x{Some(p.cells().ntbs(*name,256).unwrap())}else{None}).collect::<Vec<_>>();
    assert_eq!(actual,vec![b"DAY_TITLE_AREA".to_vec(),b"DAY_GREEN_AREA1".to_vec(),b"DAY_TEXTBOX1".to_vec(),b"DAY_GREEN_AREA2".to_vec(),b"DAY_TEXTBOX2".to_vec(),b"DAY_GREEN_AREA3".to_vec(),b"DAY_TEXTBOX3".to_vec(),b"DAY_GREEN_AREA4".to_vec(),b"DAY_TEXTBOX4".to_vec(),b"DAY_GREEN_AREA5".to_vec(),b"DAY_TEXTBOX5".to_vec()]);
    assert_eq!(e.effects().iter().filter(|x|matches!(x,Effect::TextCall{target:0x440b70|0x441220,..})).count(),22);
    assert!(!e.effects().iter().any(|x|matches!(x,Effect::TextCall{target:0x440900|0x4406a0|0x4409c0,..})));
    assert_eq!((p.cells().word(G+8).unwrap(),p.cells().word(G+12).unwrap(),p.cells().word(G+0x24).unwrap()),(0x57ef28,D,17));
    assert_eq!((p.cells().word(E+0x380).unwrap(),p.cells().word(E+0x384).unwrap(),p.cells().word(V+0x384).unwrap()),(V,0x57ef28,E));
    assert_eq!(p.cells().word(V+0x380).unwrap(),D);assert_eq!(p.cells().word(D+0x384).unwrap(),V);
    let cut=p.steps.iter().position(|x|*x==Step::LayoutReturned(E)).unwrap();
    assert_eq!(&p.steps[cut..cut+3],&[Step::LayoutReturned(E),Step::Bound(E),Step::Registered(E)]);
    assert_eq!(p.cells().byte(E+0x184).unwrap(),1);assert!(primary.require_supplier().is_err());assert!(primary.require_ordered_main().is_err());
}
#[test]
#[ignore="invalid EventList model part retains preceding Voting registration"]
fn failure_is_retained_without_losing_voting_or_allowing_replay(){
    let input=Inputs::read();let (primary,mut p,text,images)=input.fresh();
    let m=p.resources.models.get_mut(&PanelId::OtherPrimary(2)).unwrap();
    let record=m.records35.iter_mut().find(|r|{let end=r[..33].iter().position(|b|*b==0).unwrap();r[..end].eq_ignore_ascii_case(b"DAY_GREEN_AREA1")}).unwrap();record[33]=255;
    let w=controlled(&input);let (primary,p,text,images,result)=run(primary,p,text,images,&w);assert!(result.is_err());
    assert_eq!(primary.phase(),PrimaryPhase::Failed);assert_eq!(primary.eventlist().phase(),eventlist::Phase::Failed);
    assert_eq!(primary.voting().phase(),super::super::super::voting_construction::Phase::Registered);
    assert!(primary.eventlist().children().is_empty());assert_eq!(p.cells().byte(E+0x184).unwrap(),0);
    assert_eq!((p.cells().word(G+8).unwrap(),p.cells().word(G+0x24).unwrap()),(V,17));
    assert!(!p.steps.contains(&Step::Registered(E)));assert!(!p.steps.contains(&Step::LayoutReturned(E)));
    let steps=p.steps.clone();let effects=primary.eventlist().effects().to_vec();let model=p.resources.model(PanelId::OtherPrimary(2)).unwrap().records35.clone();
    let (primary,p,_,_,retry)=run(primary,p,text,images,&w);assert!(retry.is_err());
    assert_eq!(p.steps,steps);assert_eq!(primary.eventlist().effects(),effects);assert_eq!(p.resources.model(PanelId::OtherPrimary(2)).unwrap().records35,model);assert_unbound(&p);
}
#[test]
#[ignore="actual existing parent-root branch"]
fn inherited_actual_root_is_used_for_all_mandatory_children(){
    let input=Inputs::read();let (primary,mut p,text,images)=input.fresh();p.test_eventlist_inherited_root(A).unwrap();
    let w=controlled(&input);let (primary,p,_,_,result)=run(primary,p,text,images,&w);result.unwrap();
    for n in primary.eventlist().nodes(){assert_eq!(n.parent,E);assert_eq!(n.root,A);}
    assert_unbound(&p);
}
#[test]
#[ignore="same supplied Fonts selector notify/cache/current branch"]
fn selector_retains_current_before_notify_and_cache_hits_skip_pending(){
    let input=Inputs::read();let mut text=input.text();let f=text.fonts_mut();
    f.selector.cached=1;f.current=1;f.selector.pending=[0,0];f.selector.notify=7;
    assert!(f.select455420_retained(3).unwrap_err().contains("after retained current store"));
    assert_eq!(f.current,3);assert_eq!(f.selector.cached,1);assert_eq!(f.selector.notify,7);
    f.selector.pending=[4,5];assert!(f.select455420_retained(1).is_ok()); //cache hit never repairs current
    assert_eq!(f.current,3);assert_eq!(f.selector.pending,[4,5]);
    assert!(f.select455420_retained(0).unwrap_err().contains("pending page-batch"));assert_eq!(f.current,3);
}
#[test]
#[ignore="invalid packed ID ordering and reached manager effects"]
fn malformed_identifier_keeps_scale_line_mode_error_and_unknown_capacity(){
    let input=Inputs::read();let mut text=input.text();let f=text.fonts_mut();
    f.selector.cached=3;f.current=3;f.line_mode=0;f.fonts.get_mut(&3).unwrap().scale=7.;
    let formatted=f.formatted.bytes.clone();let scratch=f.scratch.clone();let mut t=Text::normal_fields();
    t.font=3;t.scale=2.;t.bounds=[0.,0.,200.,40.];let mut x=Vec::new();
    assert!(t.identifier441220_unaligned(f,u32::MAX,&mut x).is_err());
    assert_eq!(f.error,ErrorWord::Known(4));assert_eq!(f.current,3);assert_eq!(f.selector.cached,3);
    assert_eq!(f.line_mode,1);assert_eq!(f.fonts[&3].scale,2.);assert_eq!(f.formatted.bytes,formatted);assert_eq!(f.scratch,scratch);
    assert_eq!(t.group.count,0);assert!(x.is_empty());assert!(t.colors.is_empty());
}
#[test]
#[ignore="controlled post-append signed16 failure preserves reached Group/restoration"]
fn post_append_failure_keeps_group_and_restored_shared_state(){
    let input=Inputs::read();let mut text=input.text();let f=text.fonts_mut();
    f.selector.cached=3;f.current=3;f.line_mode=0;
    let mut t=Text::normal_fields();t.font=3;t.bounds=[0.,0.,2000.,40.];t.scale=2.;
    //Controlled large pre-existing Group, not a production native birth.
    t.group.count=i16::MAX as u32;t.group.offsets=vec![0;i16::MAX as usize+2];let mut x=Vec::new();
    assert!(t.identifier441220_unaligned(f,1491,&mut x).unwrap_err().contains("after append/restoration"));
    assert!(t.group.count>i16::MAX as u32);assert_eq!(t.dirty,0);assert_eq!(f.line_mode,0);assert_eq!(f.fonts[&3].scale,1.);
    assert!(x.is_empty());assert!(t.colors.is_empty());
}

#[test]
#[ignore="first real child/alias survives reached invalid packed ID"]
fn invalid_first_identifier_retains_child_alias_fonts_and_previous_registration(){
    let input=Inputs::read();let (primary,mut p,text,images)=input.fresh();
    p.test_eventlist_first_identifier(u32::MAX).unwrap();let w=controlled(&input);
    let (primary,p,text,images,result)=run(primary,p,text,images,&w);assert!(result.is_err());
    let e=primary.eventlist();assert_eq!(e.phase(),eventlist::Phase::Failed);assert_eq!(e.children().len(),1);
    let id=e.alias(0x468).unwrap();assert_eq!(id,e.children()[0]);let n=e.node(id).unwrap();let t=n.text();
    assert_eq!(n.number,Some(0));assert_eq!(n.parent,E);assert_eq!(n.root,E);assert_eq!(n.next,None);
    assert_eq!(t.font,3);assert_eq!(t.style,0);assert_eq!(t.group.count,0);assert!(t.x.is_empty());assert_eq!(e.alias(0x47c),None);
    assert_eq!(text.fonts().error,ErrorWord::Known(4));assert_eq!(text.fonts().current,3);assert_eq!(text.fonts().selector.cached,3);
    assert_eq!(primary.voting().phase(),super::super::super::voting_construction::Phase::Registered);
    assert_eq!((p.cells().word(G+8).unwrap(),p.cells().word(G+0x24).unwrap()),(V,17));assert_eq!(p.cells().byte(E+0x184).unwrap(),0);
    assert!(!p.steps.contains(&Step::LayoutReturned(E)));assert!(!p.steps.contains(&Step::Registered(E)));
    let steps=p.steps.clone();let effects=e.effects().to_vec();
    let (primary,p,_,_,retry)=run(primary,p,text,images,&w);assert!(retry.is_err());
    assert_eq!(primary.eventlist().alias(0x468),Some(id));assert_eq!(primary.eventlist().effects(),effects);assert_eq!(p.steps,steps);assert_unbound(&p);
}
