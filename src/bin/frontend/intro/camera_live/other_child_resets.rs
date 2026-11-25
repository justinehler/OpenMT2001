//! Required suffixes of41eb40, independent of selected mode0:
//!409d20 child2 with actual590f60 null,409f00 child5 +24=0.
//!This does NOT install child2/5 enter/activate/full business operations.
use std::collections::BTreeMap;
use super::{owner::Owner,ui_selection::Selected};
use super::property_child::{Widget,Selector,Caption,Title};
use super::case_zero::{List};
use super::case_zero_text::{Text,Fonts};

#[derive(Clone,Debug)]
pub enum Node {Visual(Widget),List(List),Text{shown:u8,text:Text}}
impl Node {
    fn hide(&mut self){match self{Self::Visual(w)=>w.shown=0,Self::List(l)=>l.shown=0,Self::Text{shown,..}=>*shown=0}}
    fn show(&mut self){match self{Self::Visual(w)=>w.shown=1,Self::List(l)=>l.shown=1,Self::Text{shown,..}=>*shown=1}}
    fn block(&mut self)->Result<(),String>{match self{
        Self::Visual(w)=>{w.disabled=1;w.down=0;Ok(())},
        _=>Err("business native+333/+335 target is not actual visual subtype".into())
    }}
}
#[derive(Clone,Debug)]
pub struct LinkedSelector {pub owner:Selector,pub first:u32,pub last:u32}
#[derive(Clone,Debug)]
pub struct Currency {
    ///Actual57e6ec raw NTBS (not glyphs), actual59050c suffix selector and
    ///actual57f734 format-result buffer shared with other currency producers.
    pub raw_symbol:Vec<u8>,pub suffix:u32,pub buffer:Vec<u8>,
    pub prefix_format:Vec<u8>,pub suffix_format:Vec<u8>,
}
impl Currency {
    pub(super) fn zero(&mut self)->Result<Vec<u8>,String>{
        //43a8c0's selected positive-value source sprintf format. Read actual
        //formats from supplied PE536400/536410; do not substitute a locale.
        let format=if self.suffix==0{&self.prefix_format}else{&self.suffix_format};
        let mut out=Vec::new();let mut at=0;
        while at<format.len()&&format[at]!=0{
            let c=format[at];at+=1;
            if c!=b'%'{out.push(c);continue}
            let c=*format.get(at).ok_or("business currency format truncated")?;at+=1;
            match c{b's'=>out.extend_from_slice(&self.raw_symbol),b'i'|b'd'=>out.push(b'0'),b'%'=>out.push(b'%'),
                _=>return Err("unsupported supplied zero currency sprintf token".into())}
        }
        self.buffer=out.clone();self.buffer.push(0);Ok(out)
    }
}
#[derive(Clone,Debug)]
pub struct Business {
    ///Actual pointer fields retained by offset; multiple offsets may name
    ///the SAME node key. Each subtype appears in nodes exactly once.
    pub pointers:BTreeMap<u32,u32>,pub nodes:BTreeMap<u32,Node>,
    pub selector:Option<LinkedSelector>,
    pub category28:u32,pub selection_mode2c:u32,pub option34:u32,
    pub action48:u32,pub active4c:u8,pub cacheb4:u32,pub currency:Currency,
}
struct Parts<'a>{
    controls:&'a mut[Widget;6],infos:&'a mut[Widget;5],title:&'a mut Title,
    info_text:&'a mut Text,font:&'a mut Fonts,expanded:u8,pending:u8,
}
impl Business {
    fn pointer(&self,offset:u32)->Result<u32,String>{self.pointers.get(&offset).copied().ok_or_else(||format!("business actual pointer field+{offset:x} not supplied"))}
    fn node(&mut self,offset:u32)->Result<&mut Node,String>{
        let p=self.pointer(offset)?;if p==0{return Err(format!("business mandatory field+{offset:x} is null"))}
        self.nodes.get_mut(&p).ok_or_else(||format!("business actual subtype{p:08x} not bound"))
    }
    fn hide(&mut self,offset:u32,optional:bool)->Result<(),String>{
        if optional&&self.pointer(offset)?==0{return Ok(())}self.node(offset)?.hide();Ok(())
    }
    fn selector_hide(&mut self)->Result<(),String>{
        let Some(mut selector)=self.selector.take()else{return Ok(())};
        let result=(||{
            //These live list sources overwrite range snapshots immediately
            //before native4326d0, in the same order as the common selector.
            if selector.owner.simple.is_some()||selector.owner.text_list.is_some(){return Err("business selector requires unsupported external simple/text source".into())}
            let range=|key:u32|->Result<_,String>{
                match self.nodes.get(&key){Some(Node::List(list))=>Ok(list.range()),_=>Err("business selector actual attached list owner absent".into())}
            };
            if selector.first!=0{selector.owner.first_list=Some(range(selector.first)?)}else if selector.owner.first_list.is_some(){return Err("business selector first source mismatch".into())}
            if selector.last!=0{selector.owner.last_list=Some(range(selector.last)?)}else if selector.owner.last_list.is_some(){return Err("business selector last source mismatch".into())}
            selector.owner.show(0);Ok(())
        })();self.selector=Some(selector);result
    }
    fn zero_notice(&mut self,parts:&mut Parts<'_>)->Result<(),String>{
        if self.pointer(0xa4)?==0{return Ok(())}
        match self.node(0xa4)?{Node::Text{text,..}=>text.reset(parts.font)?,
            _=>return Err("business+a4 must be the actual text subtype".into())}
        let raw=self.currency.zero()?;
        //Native forwards this result as a FORMAT with zero extra arguments.
        //Text::literal preserves % semantics and rejects any missing operand.
        match self.node(0xa4)?{Node::Text{text,..}=>text.literal(parts.font,&raw,&[]),
            _=>Err("business+a4 actual text subtype changed".into())}
    }
    fn prepare(parts:&mut Parts<'_>){
        for info in parts.infos.iter_mut(){info.shown=0;info.set_selected(0);info.disabled=0;}
    }
    ///408b50 finite mode0/null590f60, on the existing shared Text owner.
    fn mode_zero(&mut self,parts:&mut Parts<'_>)->Result<(),String>{
        parts.info_text.reset(parts.font)?;parts.title.caption=Caption::Empty;
        if parts.pending!=1&&parts.expanded!=0{parts.infos[0].shown=1;parts.infos[1].shown=1;}
        parts.info_text.identifier(parts.font,0x1e)?;parts.title.caption=Caption::Id(0x1e);
        self.node(0x64)?.show();
        parts.info_text.resize()?;
        for color in &mut parts.info_text.colors[..parts.info_text.group.count as usize]{*color=16;}
        Ok(())
    }
    fn reset(&mut self,parts:&mut Parts<'_>)->Result<(),String>{
        null_reset(&mut LegacyNull{business:self,parts})
    }

}
fn check_null(owner:&Owner)->Result<(),String>{
    if owner.ui_selection()?.selected_building.is_some(){return Err("business409d20 selected590f60 branch unsupported".into())}Ok(())
}
pub(super) fn business_owned(view:&mut super::main_panel::Prospective,owner:&Owner)->Result<(),String>{
    let m=&mut view.main;
    let Some(b)=&mut m.business_reset else{return Ok(())}; //actual optional pointer
    check_null(owner)?;
    b.reset(&mut Parts{controls:&mut m.controls,infos:&mut m.property.infos,title:&mut m.property.title,
        info_text:&mut m.info_text,font:&mut view.fonts,expanded:m.expanded,pending:m.transition_pending})
}
pub fn business_check(view:&impl super::main_panel::Access,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    let mut plan=super::main_panel::Prospective::capture(view,current)?;business_owned(&mut plan,owner)
}
pub fn business_reset(view:&mut impl super::main_panel::Access,owner:&mut Owner)->Result<(),String>{
    let current=owner.ui_selection()?.current;
    super::main_panel::transact(view,current,|p|business_owned(p,owner))
}
#[derive(Clone,Debug)]
pub struct Five {pub state24:u32}
pub fn five_check(view:&impl super::main_panel::Access,_:&Owner,_:Option<Selected>)->Result<(),String>{let _=view.main()?;Ok(())}
pub fn five_reset(view:&mut impl super::main_panel::Access,_:&mut Owner)->Result<(),String>{
    if let Some(five)=&mut view.main_mut()?.five_reset{five.state24=0;}Ok(())
}
pub fn business_step()->super::main_panel::Step{super::main_panel::Step{check:process_business_check,apply:process_business_reset}}
pub fn five_step()->super::main_panel::Step{super::main_panel::Step{check:process_five_check,apply:process_five_reset}}

//Concrete stored callback ABI; forwards once to the same finite owned transaction.
fn process_business_check(view:&super::main_panel::ProcessTextAccess<'_>,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    business_check(view,owner,current)
}
fn process_business_reset(view:&mut super::main_panel::ProcessTextAccess<'_>,owner:&mut Owner)->Result<(),String>{
    business_reset(view,owner)
}
fn process_five_check(view:&super::main_panel::ProcessTextAccess<'_>,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    five_check(view,owner,current)
}
fn process_five_reset(view:&mut super::main_panel::ProcessTextAccess<'_>,owner:&mut Owner)->Result<(),String>{
    five_reset(view,owner)
}

///Common409d20/408a50/407890 finite null-590f60 algorithm. Both adapters
///operate on actual typed owners; no copied Business/world is constructed.
pub(super) trait NullReset {
    fn scalar(&mut self,offset:u32,value:u32,byte:bool)->Result<(),String>;
    fn hide(&mut self,offset:u32,optional:bool)->Result<bool,String>;
    fn block(&mut self,offset:u32)->Result<(),String>;
    fn prepare(&mut self)->Result<(),String>;
    fn selector(&mut self)->Result<(),String>;
    fn zero_notice(&mut self)->Result<(),String>;
    fn mode_zero(&mut self)->Result<(),String>;
    fn select_control_zero(&mut self)->Result<(),String>;
    fn select_info_zero(&mut self)->Result<(),String>;
    fn info_visibility(&mut self)->Result<(),String>;
    fn activation_null(&mut self)->Result<(),String>;
}
fn null_subcontrols(v:&mut impl NullReset)->Result<(),String>{
    v.prepare()?;v.scalar(0xb4,u32::MAX,false)?;
    for at in [0x1c,0x20,0x8c,0x64,0x90,0xc8,0x98,0xac]{v.hide(at,true)?;}
    v.selector()?;v.zero_notice()?;v.hide(0x88,true)?;Ok(())
}
fn null_exit(v:&mut impl NullReset)->Result<(),String>{
    v.scalar(0xb4,u32::MAX,false)?;null_subcontrols(v)?;v.hide(0x64,true)?;
    //4078be intentionally blocks ac under the90 guard.
    if v.hide(0x90,true)?{v.block(0xac)?;}
    if v.hide(0xc8,true)?{v.block(0xc8)?;}
    v.hide(0x98,true)?;if v.hide(0xac,true)?{v.block(0xac)?;}
    v.selector()?;v.zero_notice()?;v.hide(0x88,true)?;Ok(())
}
pub(super) fn null_reset(v:&mut impl NullReset)->Result<(),String>{
    for at in [0x90,0x88,0x8c,0xac,0xc8,0x64,0x50]{v.hide(at,false)?;}
    v.scalar(0x4c,0,true)?;v.scalar(0x34,0,false)?;
    for at in [0x54,0x58]{v.hide(at,false)?;}
    v.scalar(0x28,0,false)?;v.scalar(0x48,0,false)?;v.select_control_zero()?;
    null_subcontrols(v)?;v.scalar(0x2c,0,false)?;v.mode_zero()?;
    v.activation_null()?;v.select_info_zero()?;
    null_exit(v)?;v.prepare()?;v.scalar(0x48,0,false)?;v.select_info_zero()?;
    v.info_visibility()?;v.mode_zero()
}
struct LegacyNull<'a,'b>{business:&'a mut Business,parts:&'a mut Parts<'b>}
impl NullReset for LegacyNull<'_,'_>{
    fn scalar(&mut self,o:u32,v:u32,byte:bool)->Result<(),String>{
        match (o,byte){(0x4c,true)=>self.business.active4c=v as u8,(0x34,false)=>self.business.option34=v,
            (0x28,false)=>self.business.category28=v,(0x48,false)=>self.business.action48=v,
            (0x2c,false)=>self.business.selection_mode2c=v,(0xb4,false)=>self.business.cacheb4=v,
            _=>return Err("business reset scalar subtype mismatch".into())}Ok(())
    }
    fn hide(&mut self,o:u32,optional:bool)->Result<bool,String>{
        if optional&&self.business.pointer(o)?==0{return Ok(false)}self.business.node(o)?.hide();Ok(true)
    }
    fn block(&mut self,o:u32)->Result<(),String>{self.business.node(o)?.block()}
    fn prepare(&mut self)->Result<(),String>{Business::prepare(self.parts);Ok(())}
    fn selector(&mut self)->Result<(),String>{self.business.selector_hide()}
    fn zero_notice(&mut self)->Result<(),String>{self.business.zero_notice(self.parts)}
    fn mode_zero(&mut self)->Result<(),String>{self.business.mode_zero(self.parts)}
    fn select_control_zero(&mut self)->Result<(),String>{self.parts.controls[0].set_selected(1);Ok(())}
    fn select_info_zero(&mut self)->Result<(),String>{self.parts.infos[0].set_selected(1);Ok(())}
    fn info_visibility(&mut self)->Result<(),String>{if self.parts.pending!=1&&self.parts.expanded!=0{self.parts.infos[0].shown=1;self.parts.infos[1].shown=1;}Ok(())}
    fn activation_null(&mut self)->Result<(),String>{Ok(())} //caller already checks authoritative Owner
}
