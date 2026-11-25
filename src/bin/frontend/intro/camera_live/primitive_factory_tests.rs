//! Source-backed synthetic regressions.
//! Fixtures construct PE/TM wire formats; production has no fake providers.
use super::*;
use super::super::panel_models::CpuModel;
use openmt2001::{archive::ArchiveEntry,mesh::CachedMesh};
const F:u32=0x58cdb0;const CAP:u32=F+0x188;
fn put(b:&mut [u8],i:usize,v:u32){b[i..i+4].copy_from_slice(&v.to_le_bytes())}
fn half(b:&mut [u8],i:usize,v:u16){b[i..i+2].copy_from_slice(&v.to_le_bytes())}
fn pe()->Vec<u8>{
    //One synthetic file-backed section and its known virtual zero tail. No
    //original executable bytes are embedded. Heap test storage is separate.
    let mut b=vec![0;0x24200];b[..2].copy_from_slice(b"MZ");put(&mut b,0x3c,0x80);
    b[0x80..0x84].copy_from_slice(b"PE\0\0");half(&mut b,0x84,0x14c);half(&mut b,0x86,1);
    half(&mut b,0x94,64);half(&mut b,0x98,0x10b);put(&mut b,0xb4,0x400000);
    put(&mut b,0xe0,0x100000);put(&mut b,0xe4,0x125000);put(&mut b,0xe8,0x24000);put(&mut b,0xec,0x200);
    let at=|va:u32|0x200+(va-0x525000)as usize;
    put(&mut b,at(0x525934),0x43b370);put(&mut b,at(0x52557c),0x43d010);
    put(&mut b,at(0x525264),(-1f32).to_bits());put(&mut b,at(0x525034),1f32.to_bits());
    for (va,s) in [(0x5473b8,b"gamedata2/brand_new_panels/new_nav_top.tm\0".as_slice()),
        (0x5364ac,b"WINDOW\0".as_slice()),(0x534ed0,b"BACK\0".as_slice())]{b[at(va)..at(va)+s.len()].copy_from_slice(s)}b
}
fn wire(frames:u16)->Vec<u8>{
    let mut b=vec![0;164];half(&mut b,0,4);half(&mut b,2,2);half(&mut b,4,frames);put(&mut b,8,4);
    b[48..164].fill(0x5a);
    for f in 0..frames {for [x,y,z] in [[0f32,0f32,2f32],[10.,0.,3.],[0.,10.,4.],[30.,40.,5.]]{
        for v in [x+2.*f as f32,y+3.*f as f32,z+4.*f as f32]{b.extend(v.to_le_bytes())}
    }}
    b.extend([0;4*8+4*12]);
    for (name,part) in [(b"main".as_slice(),0),(b"caption".as_slice(),1),(b"window".as_slice(),0),(b"back".as_slice(),1)]{
        let mut r=[0;35];r[..name.len()].copy_from_slice(name);r[33]=part;b.extend(r);
    }
    for _ in 0..2{let mut p=[0;20];half(&mut p,6,3);put(&mut p,16,3);b.extend(p)}
    b.extend(6u32.to_le_bytes());for i in [0u16,1,2,1,2,3]{b.extend(i.to_le_bytes())}b
}
fn model(frames:u16)->CpuModel{
    let bytes=wire(frames);let mesh=CachedMesh::parse(&bytes).unwrap();
    let mut metadata=[0;13];metadata[1]=123;metadata[2]=bytes.len() as u32;
    let entry=ArchiveEntry{path:"gamedata2/brand_new_panels/new_nav_top.tm".into(),offset:123,length:bytes.len()as u64,metadata};
    CpuModel::parsed(PanelId::F,&entry,&mesh).unwrap()
}
fn builder(frames:u16)->Builder{
    let mut b=Builder::new(Cells::process_allocation(&pe()).unwrap());b.add_primary_model(model(frames)).unwrap();b.navigator().unwrap();b
}
#[test]
fn unknown_heap_preimage_rejects_and_nondefault_constructor_retains_unwritten_cells(){
    let mut c=Cells::process_allocation(&pe()).unwrap();let p=0x700000;c.heap_allocation(p,0x388).unwrap();
    let mut b=Builder::new(c);assert!(b.panel(p).unwrap_err().contains("producer"));assert!(b.cells.word(p).is_err());
    b.cells.set_word(p+0x188+0x140,37).unwrap();b.cells.set_word(p+0x24,0xdeadbeef).unwrap();
    b.cells.set_byte(p+0x1e,0x59).unwrap();b.cells.set_byte(p+0x34a,0x98).unwrap();b.cells.set_byte(p+0x34b,0xa7).unwrap();
    b.panel(p).unwrap();
    assert_eq!(b.cells.word(p+0x24).unwrap(),0xdeadbeef);assert_eq!(b.cells.byte(p+0x1e).unwrap(),0x59);
    assert_eq!(b.cells.byte(p+0x34a).unwrap(),0x98);assert_eq!(b.cells.byte(p+0x34b).unwrap(),0xa7);
    for o in [0x178,0x358,0x35c,0x33c,0x340,0x37c]{assert!(b.cells.word(p+o).is_err(),"unwritten+{o:x}")}
    assert!(b.cells.byte(p+0x185).is_err());assert_eq!(b.cells.word(p+0x188+0x140).unwrap(),10);
    assert!(b.node(p+4).is_err());assert!(b.panel(p).is_err());
    let owner=b.finish().unwrap();assert_eq!(owner.texts.len(),1);assert_eq!(owner.texts[&(p+0x188)].pitch,10);
}
#[test]
fn whole_node_preserves_context_and_unknown_padding_and_rejects_reconstruction(){
    let mut c=Cells::process_allocation(&pe()).unwrap();let p=0x700000;c.heap_allocation(p,0x138).unwrap();
    c.set_word(p+0x24,0x12345678).unwrap();let mut b=Builder::new(c);b.node(p).unwrap();
    assert_eq!(b.cells.word(p+0x24).unwrap(),0x12345678);assert!(b.cells.half(p+0x1e).is_err());
    assert_eq!(b.cells.read(p+0x39,255).unwrap(),&[0;255]);assert!(b.node(p).is_err());
}
#[test]
fn real_model_named_regions_frames_and_exact_constructor_bind_layout_order(){
    let mut b=builder(2);b.cells.set_word(F+0x37c,0xaabbccdd).unwrap();
    b.bind_prefix(F,Some(PanelId::F),Some([Some(b"MAIN"),Some(b"CAPTION")]),Group::Primary).unwrap();
    assert_eq!(b.cells.rect(F+0x150).unwrap(),[0.,-10.,10.,-0.]);
    assert_eq!(b.cells.rect(F+0x13c).unwrap(),[2.,-43.,32.,-3.]);
    assert_eq!(b.cells.rect(F+0x164).unwrap(),[2.,-13.,12.,-3.]);
    assert_eq!(b.cells.rect(F+0x18c).unwrap(),[2.,-43.,32.,-3.]);
    assert_eq!(b.cells.word(F+0x178).unwrap(),2);assert_eq!(b.cells.word(F+0x17c).unwrap(),2);
    assert_eq!(b.cells.byte(F+0x347).unwrap(),0);assert_eq!(b.cells.byte(F+0x184).unwrap(),0);
    assert_eq!(b.cells.word(F+0x37c).unwrap(),0xaabbccdd,"typed resource link must not forge native pointer");
    let m=b.resources.model(PanelId::F).unwrap();assert_eq!(m.frames[1][0][2],-5.);assert_eq!(m.definition,[0x5a;116]);
    assert_eq!(m.source.offset,123);assert_eq!(m.records35[0][..4],*b"MAIN");
    b.layout(F,Layout::Navigator).unwrap();assert_eq!(b.cells.byte(F+0x184).unwrap(),1);
    assert_eq!(b.cells.extent(F+0x15e4).unwrap(),[2.,-13.,12.,-3.,0.5]);
    assert_eq!(b.steps,vec![Step::Node(F),Step::PanelBounds(F),Step::CaptionNode(CAP),Step::CaptionDefaults(CAP),
        Step::PanelSuffix(F),Step::NavigatorSuffix(F),Step::Bind(F),Step::Names(F),Step::ModelZ(F),Step::CaptionMode(F),
        Step::Frames(F),Step::Prepare(F),Step::BindFlags(F),Step::Layout(F),Step::Bound(F)]);
    assert!(b.bind_prefix(F,Some(PanelId::F),None,Group::Primary).is_err());
    let owner=b.finish().unwrap();assert_eq!(owner.panels[&F].phase,Phase::Bound);assert_eq!(owner.texts[&CAP].style,4);
    assert!(owner.require_process_graph().is_err());assert!(owner.require_text_layout(CAP).is_err());
}
#[test]
fn null_names_and_missing_name_query_retain_bounds_and_single_frame_skips_closed_write(){
    let mut b=builder(1);
    for o in [0x150,0x164,0x18c]{rect(&mut b.cells,F+o,[71.,72.,73.,74.]).unwrap()}
    b.bind_prefix(F,Some(PanelId::F),None,Group::Primary).unwrap();
    assert_eq!(b.cells.word(F+0x33c).unwrap(),u32::MAX);assert_eq!(b.cells.word(F+0x340).unwrap(),u32::MAX);
    for o in [0x150,0x164,0x18c]{assert_eq!(b.cells.rect(F+o).unwrap(),[71.,72.,73.,74.])}
    b.layout(F,Layout::Navigator).unwrap();let mut owner=b.finish().unwrap();
    assert_eq!(owner.texts[&CAP].bounds,[71.,72.,73.,74.]);
    owner.text_mut(CAP).unwrap().pitch=23;assert_eq!(owner.text_mut(CAP).unwrap().pitch,23,"same alias has one Text owner");
    let mut m=model(2);assert_eq!(m.find(Some(b"ABSENT")).unwrap(),u32::MAX);
    assert_eq!(m.region(u32::MAX,usize::MAX).unwrap(),None,"-1 writes nothing before frame access");
    assert!(m.region(0,2).is_err());assert!(m.region(99,0).is_err());
    assert_eq!(m.find(Some(b"main")).unwrap(),u32::MAX,"expected bytes are not uppercased");
    assert_eq!(m.find(Some(b"")).unwrap(),u32::MAX);
}
#[test]
fn missing_descriptor_wrong_alias_resource_and_definition_reject_without_starting_bind(){
    let mut b=builder(2);
    assert!(b.bind_prefix(F,Some(PanelId::A),None,Group::Primary).is_err());assert_eq!(b.panels[&F].phase,Phase::Constructed);
    b.resources.descriptors.get_mut(&PanelId::F).unwrap().reference=Some(PanelId::A);
    assert!(b.bind_prefix(F,Some(PanelId::F),None,Group::Primary).unwrap_err().contains("alias"));
    b.resources.descriptors.get_mut(&PanelId::F).unwrap().reference=Some(PanelId::F);
    b.resources.descriptors.get_mut(&PanelId::F).unwrap().definition[0]^=1;
    assert!(b.bind_prefix(F,Some(PanelId::F),None,Group::Primary).unwrap_err().contains("definition"));
    assert_eq!(b.panels[&F].phase,Phase::Constructed);
    let mut b=builder(2);let before=b.resources.models[&PanelId::F].frames[0][0][2];
    assert!(b.add_primary_model(model(2)).unwrap_err().contains("alias"));assert_eq!(b.resources.models[&PanelId::F].frames[0][0][2],before);
    let mut wrong=model(2);wrong.source.path="different.tm".into();
    let mut b=Builder::new(Cells::process_allocation(&pe()).unwrap());assert!(b.add_primary_model(wrong).unwrap_err().contains("path"));
    assert!(b.resources.model(PanelId::F).is_err());
}
#[test]
fn explicit_null_definition_and_null_argument_take_independent_branch_without_fake_zero_regions(){
    for null_argument in [false,true]{
        let mut b=builder(2);b.cells.set_word(F+0x33c,17).unwrap();b.cells.set_word(F+0x340,19).unwrap();
        b.resources.descriptors.get_mut(&PanelId::F).unwrap().reference=None;
        b.bind_prefix(F,if null_argument{None}else{Some(PanelId::F)},None,Group::Primary).unwrap();
        assert_eq!(b.cells.byte(F+0x34e).unwrap(),1);assert_eq!(b.cells.word(F+0x178).unwrap(),1);
        assert_eq!(b.cells.word(F+0x33c).unwrap(),17);assert_eq!(b.cells.word(F+0x340).unwrap(),19);
        assert_eq!(b.resources.model(PanelId::F).unwrap().frames[0][0][2],2.,"no Z bind without reference");
        assert!(b.layout(F,Layout::Navigator).is_err());assert_eq!(b.cells.byte(F+0x184).unwrap(),0);
    }
}
#[test]
fn unsupplied_layout_never_marks_bound_and_retained_fields_are_not_defaults(){
    let mut b=builder(2);b.cells.set_byte(F+0x34b,7).unwrap();b.cells.set_byte(F+0x34a,9).unwrap();
    b.cells.set_word(F+0x15c0,0x12345678).unwrap();b.cells.set_word(F+0x15c4,0x87654321).unwrap();
    b.resources.descriptors.get_mut(&PanelId::F).unwrap().frame=1;
    b.resources.descriptors.get_mut(&PanelId::F).unwrap().position=[3.,4.,5.];
    b.bind_prefix(F,Some(PanelId::F),None,Group::Primary).unwrap();
    assert!(b.layout(F,Layout::Unsupplied).unwrap_err().contains("unsupplied"));
    assert_eq!(b.cells.byte(F+0x184).unwrap(),0);assert_eq!(b.cells.byte(F+0x34b).unwrap(),7);assert_eq!(b.cells.byte(F+0x34a).unwrap(),9);
    b.layout(F,Layout::Navigator).unwrap();assert_eq!(b.cells.word(F+0x15c0).unwrap(),0x12345678);
    assert_eq!(b.cells.word(F+0x15c4).unwrap(),0x87654321);
    assert_eq!(b.resources.descriptor(PanelId::F).unwrap().frame,1);assert_eq!(b.resources.descriptor(PanelId::F).unwrap().position,[3.,4.,5.]);
}
#[test]
fn parsed_source_rejects_range_mismatch_and_keeps_opaque_record_part_identity(){
    let bytes=wire(2);let mesh=CachedMesh::parse(&bytes).unwrap();
    let entry=ArchiveEntry{path:"wrong.tm".into(),offset:1,length:bytes.len()as u64,metadata:[0;13]};
    assert!(CpuModel::parsed(PanelId::F,&entry,&mesh).is_err());
    let mut m=model(2);m.records35.swap(0,1);assert_eq!(m.find(Some(b"MAIN")).unwrap(),0,"part id is not name-record ordinal");
    let mut duplicate=m.records35[0];duplicate[33]=0;m.records35.insert(0,duplicate);
    assert_eq!(m.find(Some(b"CAPTION")).unwrap(),0,"first matching record wins");
}

#[test]
fn name_null_entries_and_independent_byte_preserve_nondefault_caption_fields(){
    let mut b=builder(2);b.cells.set_word(CAP+0x198,17).unwrap();b.cells.set_word(CAP+0x19c,29).unwrap();
    b.cells.set_byte(F+0x34e,1).unwrap();
    b.bind_prefix(F,Some(PanelId::F),Some([None,Some(b"")]),Group::Primary).unwrap();
    assert_eq!(b.cells.word(F+0x33c).unwrap(),u32::MAX);assert_eq!(b.cells.word(F+0x340).unwrap(),u32::MAX);
    assert_eq!(b.cells.word(F+0x178).unwrap(),1,"count1 is native independent branch, not guessed mesh count");
    assert_eq!(b.cells.word(CAP+0x198).unwrap(),17);assert_eq!(b.cells.word(CAP+0x19c).unwrap(),29);
    assert_eq!(b.resources.model(PanelId::F).unwrap().frame_count(),2);
}
#[test]
fn duplicate_asset_paths_still_own_distinct_primary_models(){
    let mut c=Cells::process_allocation(&pe()).unwrap();
    let path=b"gamedata2/brand_new_panels/main_panel.tm\0";c.write(0x547314,path).unwrap();
    let mut b=Builder::new(c);
    for id in [PanelId::A,PanelId::G]{let mut m=model(2);m.source.panel=id;m.source.path="gamedata2/brand_new_panels/main_panel.tm".into();b.add_primary_model(m).unwrap()}
    b.resources.models.get_mut(&PanelId::A).unwrap().frames[0][0][2]=91.;
    assert_eq!(b.resources.model(PanelId::G).unwrap().frames[0][0][2],2.);
    assert_eq!(b.resources.descriptor(PanelId::A).unwrap().reference,Some(PanelId::A));
    assert_eq!(b.resources.descriptor(PanelId::G).unwrap().reference,Some(PanelId::G));
}
#[test]
fn actual_factory_entry_creates_retained_owner_and_rejects_unprovided_text_and_graph(){
    let mut owned=Primitives::navigator(&pe(),model(2)).unwrap();
    assert_eq!(owned.panels[&F].phase,Phase::Bound);assert_eq!(owned.panels[&F].descriptor,Some(PanelId::F));
    assert_eq!(owned.texts.len(),1);assert_eq!(owned.texts[&CAP].font,0);
    assert_eq!(owned.cells().word(CAP+0x24).unwrap(),0,"only synthetic PE virtual allocation proves this zero");
    assert!(owned.require_text_layout(CAP).unwrap_err().contains("shared font"));
    assert!(owned.require_text_layout(0x999999).unwrap_err().contains("alias"));
    assert!(owned.text_mut(0x999999).is_err());assert!(owned.require_process_graph().unwrap_err().contains("18/23/7"));
}
