//! Concrete factories from the SAME already-initialized native owner cells.
//! No Scene/defaults. Missing layout/text/list/child producer is an error.
use std::collections::{BTreeMap,BTreeSet};
use super::{process_image::Cells,registration,bindings::ModeValues,
    navigation::Group,property_child::{Widget,Selector,ListRange},ui_graph::{Panel,PanelId,WidgetId},
    ui_selection::{Selected,SelectionState,SelectionKey,RelatedKey}};

/// Actual typed content produced by the native text/list model ports. These
/// are source owners, never a saved raw-memory snapshot. Removed on transfer
/// into the graph so aliases cannot become independent cloned authorities.
pub struct Content {
    pub captions:BTreeMap<u32,(super::property_child::Caption,u32)>,
    pub text_list_counts:BTreeMap<u32,u32>, //actual42e420 producer result
    pub identities:BTreeMap<u32,WidgetId>,
    pub selected:BTreeMap<u32,Selected>,
}
impl Content {
    pub fn id(&self,p:u32)->Result<WidgetId,String>{self.identities.get(&p).copied()
        .ok_or_else(||format!("actual child{p:08x} has no stable dispatch binding"))}
    pub fn selection(&self,p:u32)->Result<Option<Selected>,String>{
        if p==0{return Ok(None)}
        let selected = *self.selected.get(&p).ok_or_else(||format!("actual selection{p:08x} lacks live owner resolver"))?;
        if selected.key != SelectionKey::NativeObject(p) {
            return Err("raw process selection lacks proven native-object key binding; modern pool handles cannot be encoded as native pointers".into());
        }
        Ok(Some(selected))
    }
}
pub fn navigation(c:&Cells)->Result<ModeValues,String>{Ok(ModeValues{
    mode:c.word(0x8b1050)? as i32,shadow:c.word(0x8b1078)? as i32,
    previous:c.word(0x8b2264)? as i32,
    saved:[c.byte(0x8b1028)?,c.byte(0x8b1029)?,c.byte(0x8b102a)?],
})}
pub fn selection(c:&Cells,content:&Content,sequence_index:Option<i32>) ->Result<SelectionState,String>{
    //Sequence null vs index must come from the actual4c39c0 link producer.
    let building=c.word(0x590f60)?;
    Ok(SelectionState::installed(content.selection(c.word(0x822d50)?)?,
        content.selection(c.word(0x590f58)?)?,
        content.selection(c.word(0x823168)?)?,RelatedKey::from_native_cell(c.word(0x6175f8)?),
        if building==0{None}else{Some(building)},
        sequence_index,c.word(0x54557c)? as i32,c.word(0x545580)? as i32))
}
fn actual_children(c:&Cells,p:u32,content:&Content)->Result<Vec<WidgetId>,String>{
    let mut node=c.word(p+0x28)?;let mut seen=BTreeSet::new();let mut children=Vec::new();
    while node!=0{
        if !seen.insert(node){return Err("cycle in actual UI child sibling chain".into())}
        children.push(content.id(node)?);node=c.word(node+0x2c)?;
    }Ok(children)
}
///All48 actual post-layout/post-reset descriptors, in native registration
///order. Children come from actual+28/+2c links; no supplied empty list switch.
pub fn panels(c:&Cells,content:&Content)->Result<BTreeMap<PanelId,Panel>,String>{
    let mut out=BTreeMap::new();
    for group in [Group::Primary,Group::Secondary,Group::Tertiary]{
        for entry in registration::entries(group){let p=entry.object;let d=c.word(p+0x37c)?;
            if c.word(p+0x138)?!=registration::group_address(group){return Err("actual panel group owner mismatch".into())}
            let independent=c.byte(p+0x34e)?;
            if d==0{return Err(format!("actual descriptor absent for {:?}",entry.id))}
            let main=if entry.id==PanelId::F{c.word(p+0x33c)?}else{c.word(p+0x358)?};
            let count=c.word(p+0x178)? as i32;
            let panel=Panel{group,requested:c.byte(p+0x345)?,moving:c.byte(p+0x344)?,
                seeking:c.byte(p+0x347)?,draw_enabled:c.byte(p+0x348)?,force_show:c.byte(p+0x185)?,
                independent,block34c:c.byte(p+0x34c)?,disabled1d:c.byte(p+0x1d)?,node_shown1c:c.byte(p+0x1c)?,
                promote34b:c.byte(p+0x34b)?,block34d:c.byte(p+0x34d)?,
                f_pointer:if entry.id==PanelId::F{Some(super::f_generic::PanelCells::capture(c,p)?)}else{None},
                children:actual_children(c,p,content)?,frame_count:count,target:c.word(p+0x17c)? as i32,
                transition_end:c.word(p+0x180)? as i32,frame:c.half(d+0x2c)?,
                position:[c.float(d+4)?,c.float(d+8)?],velocity:[c.float(p+0x364)?,c.float(p+0x368)?],
                main_region:if main==u32::MAX{None}else{Some(main)},
                alpha:c.byte(d+0x3f)?,fading:c.byte(p+0x349)?,fade_step:c.byte(p+0x34a)? as i8};
            if count<1||count>u16::MAX as i32{return Err("actual panel count outside finite descriptor domain".into())}
            out.insert(entry.id,panel);
        }
    }Ok(out)
}
///Visual native conditional policy is+338;442ab0 writes CAPTION mode+13c.
///Those two cells must not be conflated when populating Widget.native_mode.
pub fn visual(c:&Cells,p:u32,caption:super::property_child::Caption,font:u32)->Result<Widget,String>{
    if p==0{return Err("mandatory actual visual is null".into())}
    Ok(Widget{shown:c.byte(p+0x1c)?,native_mode:c.word(p+0x338)?,selected:c.byte(p+0x336)?,
        disabled:c.byte(p+0x333)?,down:c.byte(p+0x335)?,node_disabled:c.byte(p+0x1d)?,
        bounds:c.rect(p+4)?,background:c.extent(p+0x318)?,image:c.word(p+0x32c)?,
        animation:c.word(p+0x34)?,completion:c.byte(p+0x38)?,caption,font})
}
///A text/list control has no Visual+338/+336 fields. Requiring invented
///visual bytes from every subtype is incorrect; its displayed text/list owner
///is transferred by its actual subtype factory. See CONTRACT's blocking join.
pub fn selector(c:&Cells,p:u32,previous:Widget,next:Widget,content:&Content)->Result<Selector,String>{
    if p==0{return Err("mandatory actual selector is null".into())}
    let simple=c.word(p+0x158)?;let first=c.word(p+0x15c)?;let last=c.word(p+0x160)?;let text=c.word(p+0x164)?;
    fn range(c:&Cells,p:u32)->Result<Option<ListRange>,String>{if p==0{return Ok(None)}
        Ok(Some(ListRange{limit480:c.word(p+0x480)? as i32,limit2cc:c.word(p+0x2cc)? as i32,first284:c.word(p+0x284)? as i32}))}
    Ok(Selector{shown:c.byte(p+0x1c)?,admitted191:c.byte(p+0x191)?,node_disabled:c.byte(p+0x1d)?,
        extent:c.float(p+0x154)?,simple:if simple==0{None}else{Some((c.word(simple+0x190)? as i32,c.word(simple+0x148)? as i32))},
        first_list:range(c,first)?,last_list:range(c,last)?,
        text_list:if text==0{None}else{Some((*content.text_list_counts.get(&text)
            .ok_or("selector42e420 count producer absent")?,c.word(text+0x2394)?))},
        previous,next,zero:c.float(0x525040)?})
}

///Moves actual subtype factory products exactly once, preserving identity.
pub struct Controls {pub widgets:BTreeMap<u32,Widget>,pub selectors:BTreeMap<u32,Selector>}
impl Controls {
    fn widget(&mut self,c:&Cells,cell:u32)->Result<Widget,String>{let p=c.word(cell)?;
        if p==0{return Err(format!("mandatory actual control cell{cell:08x} is null"))}
        self.widgets.remove(&p).ok_or_else(||format!("actual subtype factory{p:08x} absent or multiply moved"))}
    fn optional(&mut self,c:&Cells,cell:u32)->Result<Option<Widget>,String>{
        if c.word(cell)?==0{Ok(None)}else{Ok(Some(self.widget(c,cell)?))}
    }
    fn selector(&mut self,c:&Cells,cell:u32)->Result<Option<Selector>,String>{let p=c.word(cell)?;
        if p==0{return Ok(None)}Ok(Some(self.selectors.remove(&p).ok_or_else(||format!("actual selector factory{p:08x} absent or multiply moved"))?))}
}
///Literal field binding for property child1 after4055f0/4063f0. No report,
///mode, range, pending or control state is reconstructed from selection.
pub fn property(c:&Cells,controls:&mut Controls,inputs:super::property_inputs::Inputs,
    actual_title_caption:super::property_child::Caption)
    ->Result<super::property_child::State,String>{
    use super::property_child::{State,Widgets};let p=0x560820;let a=c.word(p+0x28)?;
    if a!=0x58e3c0{return Err("property child1 actual parent binding mismatch".into())}
    if c.word(0x822d50)?!=0||c.word(p+0x20)?!=0||c.word(p+0x24)?!=0{
        return Err("finite first-process property factory requires actual no-selection report0/mode0 after native layout; retained warm state must be reused".into())
    }
    let offer=c.word(p+0xb0)?;
    if offer==0{return Err("actual property offer allocation missing".into())}
    let infos=(0..5).map(|i|controls.widget(c,a+0x4a4+i*4)).collect::<Result<Vec<_>,_>>()?
        .try_into().map_err(|_|"actual five info controls")?;
    let offer_labels=(0..6).map(|i|controls.widget(c,p+0x94+i*4)).collect::<Result<Vec<_>,_>>()?
        .try_into().map_err(|_|"actual six offer labels")?;
    Ok(State{reset_flag:c.byte(p+0x10)?,other_group:c.byte(p+0x11)?,utility_group:c.byte(p+0x12)?,
        utility_index:c.word(p+0x14)?,other_index:c.word(p+0x18)?,active:c.byte(p+0x1c)?,
        mode:c.word(p+0x20)?,report:c.word(p+0x24)?,profit_period:c.word(p+0x7c)?,efficiency_period:c.word(p+0x88)?,
        infos,title:super::case_zero_factories::property_title(c,actual_title_caption)?,notice:controls.optional(c,p+0x34)?,
        widgets:Widgets{previous:controls.optional(c,p+0x2c)?,next:controls.optional(c,p+0x30)?,
            report_one:controls.optional(c,p+0x64)?,selector_one:controls.selector(c,p+0x68)?,
            report_three:controls.optional(c,p+0x74)?,selector_three:controls.selector(c,p+0x78)?,
            report_four:controls.optional(c,p+0x80)?,selector_four:controls.selector(c,p+0x84)?,
            report_five:controls.optional(c,p+0x8c)?,selector_five:controls.selector(c,p+0x90)?,
            offer:controls.optional(c,p+0xb0)?,rights:controls.optional(c,p+0xac)?,auxiliary:controls.optional(c,p+0xbc)?,
            marker:controls.optional(c,p+0x6c)?,text:controls.optional(c,p+0x38)?},
        //Requires property-owner-alias.patch: offer explanation IS labels[4].
        rights_explanation:controls.widget(c,p+0xb8)?,offer_labels,
        amount:c.float(offer+0x140)?,amount_dirty:c.byte(offer+0x13c)?,step:c.float(offer+0x144)?,
        minimum:c.float(offer+0x148)?,maximum:c.float(offer+0x14c)?,action_key:c.word(c.word(p+0xac)?+0x2f4)?,
        //4063f0 report0/None never populates these typed report projections.
        //Their actual text/list controls ABOVE remain allocated and retained.
        summary:Vec::new(),summary_text:Vec::new(),land:Vec::new(),land_links:Vec::new(),
        selected_category:c.word(a+0x4f4)?,colors:[c.word(a+0x40c)?,c.word(a+0x410)?,c.word(a+0x414)?,c.word(a+0x418)?],inputs})
}

pub fn main_model(c:&Cells,mesh:&openmt2001::mesh::CachedMesh<'_>,image:&openmt2001::pe::PeImage<'_>)
    ->Result<super::main_model::State,String>{
    let a=0x58e3c0;let toggle=c.word(a+0x488)?;
    if toggle==0{return Err("actual A overlay control not constructed".into())}
    let part=|at|->Result<Option<usize>,String>{let v=c.word(at)?;Ok(if v==u32::MAX{None}else{Some(v as usize)})};
    let geometry=super::main_model::Geometry::from_mesh(mesh,part(a+0x358)?,part(a+0x35c)?,image)?;
    Ok(super::main_model::State{geometry,closed_bounds:c.rect(a+0x150)?,open_bounds:c.rect(a+0x13c)?,
        caption_bounds:c.rect(a+0x164)?,caption_visual_bounds:c.rect(a+0x18c)?,
        overlay_extent:c.extent(toggle+0x318)?,overlay_image:c.word(toggle+0x32c)?,
        overlay_completion:c.byte(toggle+0x38)?,overlay_animation:c.word(toggle+0x34)?,
        open_icon:c.word(0x68af44)?,closed_icon:c.word(0x68af48)?})
}
pub fn main(c:&Cells,controls:&mut Controls,content:&mut Content,
    property:super::property_child::State,model:super::main_model::State,
    notice_text:super::case_zero_text::Text,case_zero:super::case_zero::State,
    info_text:super::case_zero_text::Text,business_reset:Option<super::other_child_resets::Business>,
    five_reset:Option<super::other_child_resets::Five>,children:[Option<super::main_panel::Child>;6])->Result<super::main_panel::State,String>{
    use super::property_child::Caption;let a=0x58e3c0;
    let modes=(0..6).map(|i|controls.widget(c,a+0x48c+i*4)).collect::<Result<Vec<_>,_>>()?
        .try_into().map_err(|_|"actual six A mode controls")?;
    let notice=c.word(a+0x3a0)?;
    let (notice,notice_caption)=match content.captions.remove(&notice).ok_or("actual A notice text owner absent")?.0{
        Caption::Empty=>(Vec::new(),None),Caption::Id(id)=>(Vec::new(),Some(id)),
        Caption::Bytes(b)|Caption::Glyphs(b)=>(b,None),
    };
    Ok(super::main_panel::State::installed(c.word(a+0x39c)?,c.byte(a+0x508)?,c.byte(a+0x509)?,
        modes,notice,notice_caption,property,model,notice_text,case_zero,info_text,business_reset,five_reset,children))
}
