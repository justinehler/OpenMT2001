//! Actual ordered helper and installed graph read routes.
use super::*;
use std::cell::RefCell;
#[test]
fn timing400_stores_first_four_then_synchronous_mode_then_sixth(){
    struct State{words:[u32;6],human:bool,trace:RefCell<Vec<i32>>,fail:bool}
    fn store(s:&mut State,index:usize,value:u32)->Result<(),String>{s.trace.borrow_mut().push(index as i32);s.words[index]=value;Ok(())}
    fn human(s:&State)->bool{s.trace.borrow_mut().push(40);s.human}
    fn mode(s:&mut State,value:i32)->Result<(),String>{
        assert_eq!(s.words,[11,12,13,14,55,66]);assert_eq!(value,17);
        s.trace.borrow_mut().push(50);if s.fail{Err("mode failed".into())}else{Ok(())}
    }
    let payload=[11u32,12,13,14,17,99].into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>();
    let mut s=State{words:[0,0,0,0,55,66],human:true,trace:RefCell::new(vec![]),fail:false};
    process_lifetime::restore_timing400(&payload,&mut s,store,human,mode).unwrap();
    assert_eq!(*s.trace.borrow(),[0,1,2,3,40,50,5]);assert_eq!(s.words,[11,12,13,14,55,99]);
    s.words=[0,0,0,0,55,66];s.trace.borrow_mut().clear();s.fail=true;
    assert!(process_lifetime::restore_timing400(&payload,&mut s,store,human,mode).is_err());
    assert_eq!(s.words,[11,12,13,14,55,66]);assert_eq!(*s.trace.borrow(),[0,1,2,3,40,50]);
    s.words=[0,0,0,0,55,66];s.trace.borrow_mut().clear();s.human=false;
    process_lifetime::restore_timing400(&payload,&mut s,store,human,mode).unwrap();
    assert_eq!(*s.trace.borrow(),[0,1,2,3,40,5]);assert_eq!(s.words[5],99);
}
#[test]
fn actual_chat_identity_guard_rereads_requested_not_voting_g_body_or_moving(){
    use ui_graph::PanelId;
    let process=crate::live_host::process_runtime::test_fixture();
    let mut g=ui_graph::refresh_fixture([0;600]);
    assert!(g.chat_guard57e794_plus345().is_err());
    //Actual installed pointer + registration identity, independently of the
    //synthetic panel animation fixture. No constructor Cells after transfer.
    //The test-only allocation is explicitly initialized by this source writer.
    //It does not claim native constructor/factory coverage.
    let mut c=loader_cells();c.heap_allocation(0x57e710,0x88).unwrap();
    c.set_word(0x57e710+0x84,0x57ef28).unwrap();
    g.bind_process_source(process,&c).unwrap();
    g.panel_mut(PanelId::G).unwrap().requested=1;
    g.panel_mut(PanelId::OtherPrimary(1)).unwrap().requested=1;
    let chat=g.panel_mut(PanelId::OtherPrimary(3)).unwrap();
    chat.requested=0;chat.moving=1;chat.draw_enabled=0;
    assert_eq!(g.chat_guard57e794_plus345().unwrap(),0);
    g.panel_mut(PanelId::OtherPrimary(3)).unwrap().requested=2;
    assert_eq!(g.chat_guard57e794_plus345().unwrap(),2);
    g.panel_mut(PanelId::OtherPrimary(3)).unwrap().requested=0;
    assert_eq!(g.chat_guard57e794_plus345().unwrap(),0);
    let mut g=ui_graph::refresh_fixture([0;600]);c.set_word(0x57e710+0x84,0x573b38).unwrap();
    assert!(g.bind_process_source(crate::live_host::process_runtime::test_fixture(),&c).is_err());
}
fn loader_cells()->process_image::Cells{
    let mut pe=vec![0;0x200];pe[..2].copy_from_slice(b"MZ");
    let put=|pe:&mut[u8],at:usize,value:u32|pe[at..at+4].copy_from_slice(&value.to_le_bytes());
    put(&mut pe,0x3c,0x80);pe[0x80..0x84].copy_from_slice(b"PE\0\0");
    pe[0x84..0x86].copy_from_slice(&0x14cu16.to_le_bytes());pe[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    pe[0x94..0x96].copy_from_slice(&0xe0u16.to_le_bytes());pe[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
    put(&mut pe,0xb4,0x400000);put(&mut pe,0x180,0x150000);put(&mut pe,0x184,0x400000);
    process_image::Cells::process_allocation(&pe).unwrap()
}
