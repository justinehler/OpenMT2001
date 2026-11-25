//! Uses the real Locator Tree/Handler types.
//! Decode the actual initialized C factory graph, never a convenience subset.
use super::process_image::Cells;
use crate::locator::strategic::tree::{Tree,Node,Handler};
use std::collections::{BTreeMap,BTreeSet};
pub fn tree(c:&Cells)->Result<Tree,String>{
    fn visit(c:&Cells,mut p:u32,nodes:&mut BTreeMap<u32,Node>,seen:&mut BTreeSet<u32>)->Result<(),String>{
        while p!=0{
            if !seen.insert(p){return Err("actual C factory has cyclic/shared child".into())}
            let vtable=c.word(p)?;let call=c.word(vtable+0x14)?;
            let handler=match call{
                0x43fb70=>Handler::Ret43fb70,
                0x430690=>Handler::Report430690,0x431c90=>Handler::Selector431c90,
                0x42c9e0=>Handler::Button{pulse:c.byte(p+0x148)?,previous:c.byte(p+0x149)?},
                0x443360=>Handler::Action{pulse:c.byte(p+0x331)?,previous:c.byte(p+0x332)?},
                0x42f2b0=>Handler::Image{changed:c.byte(p+0x148)?,previous:c.byte(p+0x149)?},
                0x42efe0=>Handler::Slider{changed:c.byte(p+0x13c)?},
                va=>Handler::Unported(va),
            };
            let first=c.word(p+0x28)?;let next=c.word(p+0x2c)?;
            nodes.insert(p,Node{shown:c.byte(p+0x1c)?,first:if first==0{None}else{Some(first)},
                next:if next==0{None}else{Some(next)},handler});
            visit(c,first,nodes,seen)?;p=next;
        }Ok(())
    }
    let first=c.word(0x58e8d0+0x28)?;
    if first==0{return Err("actual C40d080 factory has not produced its required child tree".into())}
    let mut nodes=BTreeMap::new();visit(c,first,&mut nodes,&mut BTreeSet::new())?;
    Tree::installed(Some(first),nodes)
}
///40f53a ->430a70 actual list+65c enabled[-1], before any bounds guard.
///This is intentionally fallible. PE static zero-fill proves no heap byte.
///Only a real owned allocator/header producer may make this read available.
pub fn commodity_negative_mark(c:&Cells)->Result<u8,String>{
    let list=c.word(0x58e8d0+0xb30)?;
    if list==0{return Err("actual C commodity list allocation missing".into())}
    let enabled=c.word(list+0x65c)?;
    let preceding=enabled.checked_sub(1).ok_or("actual C enabled pointer has no predecessor")?;
    c.byte(preceding).map_err(|e|format!("strict C enabled[-1] producer remains unclosed: {e}"))
}
