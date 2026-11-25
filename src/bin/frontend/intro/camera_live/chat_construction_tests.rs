//! Requires supplied originals and the exact source composition.
use super::*;
use super::super::super::{chat_construction::{self as chat,Effect,Subtype},visual_construction::{Visual,ImageRef},main_construction::Field};
const C:u32=0x57ef28;const E:u32=0x571378;
fn controlled(input:&Inputs)->openmt2001::level_startup::World{
    let mut w=input.world();for i in 0..52{row(&mut w,i,0,1,46);}row(&mut w,41,0,1,0);w
}
fn extent(p:&Primitives,offset:u32)->[u32;5]{std::array::from_fn(|i|p.cells().word(C+offset+4*i as u32).unwrap())}
///Independent mesh vertices/part records. Never calls candidate CpuModel
///find/region, constructor or stored Chat Cells to generate expectations.
fn original_region(input:&Inputs,name:&[u8])->[f32;4]{
    let e=input.index.find("gamedata2/brand_new_panels/chat_panel_v2.tm").unwrap();
    let mesh=CachedMesh::parse(&input.archive[e.offset as usize..(e.offset+e.length)as usize]).unwrap();
    let record=mesh.opaque_records_35.chunks_exact(35).find(|r|{
        let end=r[..33].iter().position(|&b|b==0).unwrap();r[..end].eq_ignore_ascii_case(name)
    }).unwrap();
    let positions=mesh.positions(mesh.frames as usize-1).unwrap();
    let mut xs=Vec::new();let mut ys=Vec::new();
    for index in mesh.parts[usize::from(record[33])].vertex_indices(){xs.push(positions[index][0]);ys.push(-positions[index][1]);}
    xs.sort_by(|a,b|a.partial_cmp(b).unwrap());ys.sort_by(|a,b|a.partial_cmp(b).unwrap());
    [xs[0],ys[0],*xs.last().unwrap(),*ys.last().unwrap()]
}
#[test]
#[ignore="supplied static birth/nonwriters and retained aliases"]
fn chat_static_birth_has_twelve_records_three_same_texts_and_no_dynamic_children(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let c=primary.chat();
    assert_eq!(c.phase(),chat::Phase::Unbound);assert!(c.children().is_empty());assert_eq!(c.nodes().count(),0);
    assert_eq!(p.cells().word(C).unwrap(),0x5258c0);
    for i in 0..12{assert_eq!(extent(&p,0x6f0+20*i),[0,0,0,0,0x3f000000]);}
    for offset in (0x7e0..0x80c).step_by(4){assert_eq!(p.cells().read(C+offset,4).unwrap(),&c.static_preimage[offset as usize..offset as usize+4]);}
    for offset in [0x188,0x388,0x53c]{let t=&p.texts[&(C+offset)];assert_eq!((t.font,t.pitch,t.visible),(0,10,64));assert_eq!(t.group.count,0);}
    let owner=c as *const _;let caption=&p.texts[&(C+0x188)] as *const _;let title=&p.texts[&(C+0x388)] as *const _;let to=&p.texts[&(C+0x53c)] as *const _;
    let moved=Box::new((primary,p,text,images));assert_eq!(moved.0.chat() as *const _,owner);
    assert_eq!(&moved.1.texts[&(C+0x188)] as *const _,caption);assert_eq!(&moved.1.texts[&(C+0x388)] as *const _,title);assert_eq!(&moved.1.texts[&(C+0x53c)] as *const _,to);assert_unbound(&moved.1);
}
#[test]
#[ignore="actual protected Install, original eleven named extents/child order"]
fn real_chat_body_return_suffix_registration_and_duplicate_zero_key(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let w=controlled(&input);
    let owner=primary.chat() as *const _;let fonts=text.fonts() as *const _;let model=p.resources.model(PanelId::OtherPrimary(3)).unwrap().frames.as_ptr();
    let image_scales=images.records.iter().map(|i|i.scale).collect::<Vec<_>>();
    let (primary,p,text,images,result)=run(primary,p,text,images,&w);
    assert_eq!(result.unwrap(),InstallDisposition::CompletedChat{epoch:w.topology.epoch().serial()});
    assert_eq!(primary.phase(),PrimaryPhase::AwaitingG);assert_unbound(&p);
    let c=primary.chat();assert_eq!(c as *const _,owner);assert_eq!(c.phase(),chat::Phase::Registered);
    assert_eq!(text.fonts() as *const _,fonts);assert_eq!(p.resources.model(PanelId::OtherPrimary(3)).unwrap().frames.as_ptr(),model);
    assert_eq!(images.records.iter().map(|i|i.scale).collect::<Vec<_>>(),image_scales);
    let names:[(&[u8],u32);11]=[(b"CHAT_TITLE_AREA",0x6f0),(b"CHAT_BACKING",0x704),(b"CHAT_TO_LABEL",0x718),(b"CHAT_INPUT_AREA",0x72c),
        (b"CHAT_ALL_BUTTON",0x740),(b"CHAT_CANCEL_BUTTON",0x768),(b"CHAT_ICON_SLOT1",0x77c),(b"CHAT_ICON_SLOT2",0x790),
        (b"CHAT_ICON_SLOT3",0x7a4),(b"CHAT_ICON_SLOT4",0x7b8),(b"CHAT_ICON_SLOT5",0x7cc)];
    let queries=c.effects().iter().filter_map(|e|if let Effect::Query{name,..}=e{Some(p.cells().ntbs(*name,256).unwrap())}else{None}).collect::<Vec<_>>();
    assert_eq!(queries,names.iter().map(|(s,_)|s.to_vec()).collect::<Vec<_>>());
    for (name,offset) in names{assert_eq!(&extent(&p,offset)[..4],&original_region(&input,name).map(f32::to_bits));}
    assert_eq!(extent(&p,0x754),[0,0,0,0,0x3f000000]);assert_eq!(c.alias(0x7e4),None);
    assert_eq!(p.cells().read(C+0x7e4,4).unwrap(),&c.static_preimage[0x7e4..0x7e8]);
    assert_eq!(c.children().len(),8);assert_eq!(c.nodes().count(),8);
    for (slot,key) in [0,2,3,4,5,6,7,0].into_iter().enumerate(){let id=c.children()[slot];let n=c.node(id).unwrap();
        assert_eq!(n.number,Some(key));assert_eq!(n.next,c.children().get(slot+1).copied());assert_eq!((n.event_sink,n.frame),(C,C));
        assert_eq!(n.depth,0.5);assert_eq!(n.allocation_size,if slot==7{0x390}else{0x374});
        assert_eq!(n.vtable,if slot==7{0x5259f4}else{0x5259a4});
        if slot<7{assert_eq!(c.alias(0x7e0+4*key),Some(id));}
    }
    assert_ne!(c.alias(0x7e0),c.alias(0x808));assert_eq!(c.alias(0x808),Some(c.children()[7]));
    for (offset,style,id) in [(0x388,4,1481),(0x53c,3,1482)]{let t=&p.texts[&(C+offset)];
        assert_eq!((t.font,t.style,t.line_mode,t.inset,t.scroll),(3,style,1,0,0));assert!(!t.group.row(0).unwrap().is_empty());
        assert!(!text.fonts().language.row(id).unwrap().is_empty());assert!(t.x.is_empty());
    }
    assert!(c.title_x().iter().chain(c.to_x()).all(Option::is_none));
    let reads=c.effects().iter().filter_map(|e|if let Effect::ImageRead(at)=e{Some(*at)}else{None}).collect::<Vec<_>>();
    assert_eq!(reads,[0x68af70,0x68af74,0x68af78,0x68af7c,0x68af80,0x68af84]);
    for (slot,id) in c.children()[..7].iter().copied().enumerate(){let Subtype::Visual(v)=&c.node(id).unwrap().subtype else{panic!("not Visual")};
        assert_eq!(v.caption.font,3);assert_eq!(v.caption.style,4);assert_eq!(v.caption_x.as_ref().unwrap().len(),v.caption.colors.len());
        assert!(v.caption_x.as_ref().unwrap().iter().all(Option::is_none));assert!(v.caption.x.is_empty());
        assert_eq!(v.images[&0x310],ImageRef::Invalid);assert_eq!(v.images[&0x30c],ImageRef::Invalid);
        if slot<2{assert_eq!(v.scalars[&0x2f4],if slot==0{1483}else{1761});
            for (offset,delta) in [(0x300,0),(0x348,0),(0x308,1),(0x304,2)]{let ImageRef::Image(image)=v.images[&offset] else{panic!("missing supplied image")};
                assert_eq!(images.record(image).unwrap().ordinal,0x52+slot*3+delta);}
        }else{assert!(!v.scalars.contains_key(&0x2f4));for offset in [0x300,0x348,0x304,0x308]{assert_eq!(v.images[&offset],ImageRef::Invalid);}}
        let stores=c.effects().iter().filter_map(|e|if let Effect::ImageStore{node,offset}=e{(*node==id).then_some(*offset)}else{None}).collect::<Vec<_>>();
        assert_eq!(stores,[0x300,0x348,0x304,0x308,0x30c,0x310]);
    }
    let i=c.input().unwrap();assert_eq!((i.focused,i.count,i.width_limit,i.char_limit,i.enabled),(0,0,230,215,1));
    assert_eq!((i.flag381,i.ready383,i.color,i.flag388,i.pulse),(1,0,u32::MAX,1,0));assert!(matches!(i.active382,Field::Unknown));
    assert_eq!(i.raw[0],Some(0));assert!(i.raw[1..].iter().all(Option::is_none));assert_eq!(i.text.font,3);
    assert_eq!(i.text.bounds,original_region(&input,b"CHAT_INPUT_AREA"));assert_eq!((i.text.depth,i.text.visible,i.text.group.count),(0.5,64,0));
    assert_eq!((p.cells().word(G+8).unwrap(),p.cells().word(G+12).unwrap(),p.cells().word(G+0x24).unwrap()),(C,D,17));
    assert_eq!((p.cells().word(C+0x380).unwrap(),p.cells().word(C+0x384).unwrap(),p.cells().word(E+0x384).unwrap()),(E,0,C));
    assert_eq!(p.cells().word(0x86d494).unwrap(),C);assert_eq!(p.cells().byte(C+0x184).unwrap(),1);
    let cut=p.steps.iter().position(|s|*s==Step::LayoutReturned(C)).unwrap();assert_eq!(&p.steps[cut..cut+3],&[Step::LayoutReturned(C),Step::Bound(C),Step::Registered(C)]);
    assert!(matches!(&c.effects()[c.effects().len()-3..],[Effect::TextCall{target:0x440b70,..},Effect::ProcessAlias,Effect::Returned]));
    assert!(primary.require_supplier().is_err());assert!(primary.require_ordered_main().is_err());
}
#[test]
#[ignore="reached invalid Visual identifier after real factory/alias, bornFont0"]
fn identifier_failure_keeps_link_alias_native_font_zero_and_blocks_replay(){
    let input=Inputs::read();let (mut primary,p,text,images)=input.fresh();primary.chat.test_first_identifier(u32::MAX);let w=controlled(&input);
    let (primary,p,text,images,result)=run(primary,p,text,images,&w);assert!(result.is_err());let c=primary.chat();
    assert_eq!(c.phase(),chat::Phase::Failed);assert_eq!(c.children().len(),1);let id=c.alias(0x7e0).unwrap();assert_eq!(id,c.children()[0]);
    let Subtype::Visual(v)=&c.node(id).unwrap().subtype else{panic!("not Visual")};assert_eq!(v.caption.font,0);assert_eq!(v.caption.style,0);
    assert!(!v.scalars.contains_key(&0x2f4));assert_eq!(text.fonts().current,0);assert_eq!(text.fonts().selector.cached,0);
    assert_eq!(text.fonts().error,super::super::super::case_zero_text::ErrorWord::Known(4));
    assert_eq!(primary.eventlist().phase(),super::super::super::eventlist_construction::Phase::Registered);
    assert_eq!(p.cells().byte(C+0x184).unwrap(),0);assert_eq!(p.cells().word(G+8).unwrap(),E);assert!(c.alias(0x808).is_none());
    let effects=c.effects().to_vec();let steps=p.steps.clone();let (primary,p,_,_,retry)=run(primary,p,text,images,&w);
    assert!(retry.is_err());assert_eq!(primary.chat().effects(),effects);assert_eq!(p.steps,steps);assert_unbound(&p);
}
#[test]
#[ignore="actual last Font3 notify refusal before process alias"]
fn last_font_failure_keeps_input_limits_and_current_store_before_alias_or_suffix(){
    let input=Inputs::read();let (mut primary,p,text,images)=input.fresh();primary.chat.test_final_font_notify();
    let prior=p.cells().word(0x86d494).unwrap();let w=controlled(&input);let (primary,p,mut text,images,result)=run(primary,p,text,images,&w);
    assert!(result.unwrap_err().contains("after retained current store"));let c=primary.chat();assert_eq!(c.children().len(),8);
    let i=c.input().unwrap();assert_eq!((i.text.font,i.width_limit,i.char_limit,i.enabled),(3,230,215,1));assert!(matches!(i.active382,Field::Unknown));
    assert_eq!(text.fonts().current,3);assert_eq!(text.fonts().selector.cached,0);assert_eq!(text.fonts().selector.notify,7);
    assert_eq!(p.cells().word(0x86d494).unwrap(),prior);assert_eq!(p.cells().byte(C+0x184).unwrap(),0);assert_eq!(p.cells().word(G+8).unwrap(),E);
    assert!(!p.steps.contains(&Step::Registered(C)));assert!(!p.steps.contains(&Step::LayoutReturned(C)));assert_eq!(primary.phase(),PrimaryPhase::Failed);
    let effects=c.effects().to_vec();let steps=p.steps.clone();text.fonts_mut().selector.notify=0;
    let (primary,p,_,_,retry)=run(primary,p,text,images,&w);assert!(retry.is_err());assert_eq!(primary.chat().effects(),effects);assert_eq!(p.steps,steps);
}
#[test]
#[ignore="genuine shared442600 partial pre/post caption store cuts"]
fn shared_visual_extent_failure_retains_pre_caption_writes_without_post_caption_reset(){
    let mut v=Visual::normal_fields();assert!(v.initialize442600([0.,0.,1.,f32::NAN]).is_err());
    assert_eq!(v.scalars[&0x13c],2);assert_eq!(v.scalars[&0x32c],u32::MAX);assert_eq!(v.images[&0x300],ImageRef::Invalid);
    assert!(!v.images.contains_key(&0x348));assert!(!v.images.contains_key(&0x308));assert!(!v.scalars.contains_key(&0x370));assert!(!v.scalars.contains_key(&0x2f4));
    assert!(v.caption.bounds[3].is_nan());assert_eq!(v.caption.visible,64);
}
#[test]
#[ignore="World2 keeps SAME Chat and all aliases/Fonts without reset/show replay"]
fn retirement_and_next_world_park_without_rebuilding_chat(){
    let input=Inputs::read();let (primary,p,text,images)=input.fresh();let one=controlled(&input);let two=controlled(&input);
    let (mut primary,mut p,text,images,result)=run(primary,p,text,images,&one);result.unwrap();let owner=primary.chat() as *const _;
    let effects=primary.chat().effects().to_vec();let ids=primary.chat().children().to_vec();let steps=p.steps.clone();let bytes=p.cells().read(C,0x80c).unwrap().to_vec();
    let input_text=&primary.chat().input().unwrap().text as *const _;let fonts=text.fonts() as *const _;
    primary.retire_for_camera(&mut p).unwrap();assert!(primary.require_current_d(one.topology.epoch().serial()).is_err());
    let (primary,p,text,_,result)=run(primary,p,text,images,&two);assert!(matches!(result.unwrap(),InstallDisposition::ParkedAwaitingDRebind{..}));
    assert_eq!(primary.chat() as *const _,owner);assert_eq!(primary.chat().children(),ids);assert_eq!(primary.chat().effects(),effects);
    assert_eq!(&primary.chat().input().unwrap().text as *const _,input_text);assert_eq!(text.fonts() as *const _,fonts);
    assert_eq!(p.cells().read(C,0x80c).unwrap(),bytes);assert_eq!(p.steps,steps);assert_unbound(&p);
}
#[test]
#[ignore="actual43f340 null/inherited frame branches, supplied non-half depth"]
fn input_factory_direct_copies_rectangle_and_ignores_fifth_word_without_text_reset(){
    let input=Inputs::read();
    for (parent_frame,expected) in [(0,C),(A,A)]{
        let (mut primary,mut p,text,_)=input.fresh();p.test_chat_inherited_frame(parent_frame).unwrap();
        let shared_current=text.fonts().current;let scratch=text.fonts().scratch.clone();let e=[7.,11.,237.,51.,0.75];
        let id=primary.chat.test_input_factory(&p,e).unwrap();let c=primary.chat();let n=c.node(id).unwrap();
        assert_eq!((n.event_sink,n.frame),(C,expected));assert_eq!(n.number,Some(0));assert_eq!(n.vtable,0x5259f4);
        assert_eq!(n.bounds,[7.,11.,237.,51.]);assert_eq!(n.depth,0.5);let Subtype::Input(i)=&n.subtype else{panic!("not Input")};
        assert_eq!(i.text.bounds,n.bounds);assert_eq!((i.text.depth,i.text.visible,i.text.pitch),(0.5,64,10));
        assert_eq!(i.text.font,0);assert_eq!(i.text.group.count,0);assert!(i.text.x.is_empty());
        assert!(matches!(i.embedded_context24,Field::Unknown));assert!(matches!(i.active382,Field::Unknown));
        assert_eq!((i.width_limit,i.char_limit,i.enabled),(-1,-1,0));assert_eq!(c.alias(0x808),None);
        assert_eq!(text.fonts().current,shared_current);assert_eq!(text.fonts().scratch,scratch);
        assert!(!c.effects().iter().any(|e|matches!(e,Effect::TextCall{..})));
        assert!(c.effects().iter().any(|e|matches!(e,Effect::Slot04{target:0x42f140,..})));assert_unbound(&p);
    }
}
#[test]
#[ignore="missing reached hover image rejects before factory despite first read"]
fn missing_image_does_not_allocate_visual_or_change_fonts(){
    let input=Inputs::read();let (mut primary,p,mut text,mut images)=input.fresh();images.records.truncate(0x53);
    let current=text.fonts().current;let scratch=text.fonts().scratch.clone();
    assert!(primary.chat.test_visual_images(&p,&mut text,&images).unwrap_err().contains("ordinal83 missing"));
    assert!(primary.chat().children().is_empty());assert_eq!(primary.chat().nodes().count(),0);assert_eq!(primary.chat().alias(0x7e0),None);
    assert_eq!(primary.chat().effects(),&[Effect::ImageRead(0x68af70),Effect::ImageRead(0x68af74)]);
    assert_eq!(text.fonts().current,current);assert_eq!(text.fonts().scratch,scratch);assert_unbound(&p);
}
#[test]
#[ignore="mandatory completed EventList prerequisite rejects before any Chat mutation"]
fn chat_cannot_complete_from_unbound_eventlist_or_wrong_phase(){
    let input=Inputs::read();let (mut primary,mut p,mut text,images)=input.fresh();let w=controlled(&input);
    let source=ResidentSource::from_world(&w);let epoch=source.epoch().serial();
    let steps=p.steps.clone();let bytes=p.cells().read(C,0x80c).unwrap().to_vec();let scratch=text.fonts().scratch.clone();
    assert!(primary.d.install_chat(&mut primary.chat,&primary.eventlist,&source,epoch,&mut p,&mut text,&images).is_err());
    assert_eq!(p.steps,steps);assert_eq!(p.cells().read(C,0x80c).unwrap(),bytes);assert_eq!(text.fonts().scratch,scratch);
    assert_eq!(primary.chat().phase(),chat::Phase::Unbound);assert!(primary.chat().effects().is_empty());assert_unbound(&p);
}
