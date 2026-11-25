//! Concrete4024b0/402c30/402d10/4021a0 child1 lifecycle.
//! Finite admission: None or ordinary kind4 selected property summary/offer;
//! selected rent/business/special report effects remain explicit preflight errors.
//! Reads actual Owner selection at each native resample. No Scene/Host/FreshRequest.
use super::{owner::{Owner,ResolvedSelected},ui_selection::Selected};
use super::property_inputs::{Inputs,Category};
use super::main_panel::Prospective;
#[derive(Clone,Debug,PartialEq)]
pub enum Caption { Empty,Id(u32),Bytes(Vec<u8>),Glyphs(Vec<u8>) }
#[derive(Clone,Debug)]
pub struct Widget {
    pub shown:u8,pub native_mode:u32,pub selected:u8,pub disabled:u8,pub down:u8,pub node_disabled:u8,
    pub bounds:[f32;4],pub background:[f32;5],pub image:u32,
    pub animation:u32,pub completion:u8,pub caption:Caption,pub font:u32,
}
impl Widget {
    pub fn set_selected(&mut self,value:u8){if self.native_mode==1{self.selected=value;}}
    fn set_image(&mut self,extent:[f32;5],image:u32){self.background=extent;self.image=image;}
    fn animate(&mut self,animation:u32){self.completion=0;self.animation=animation;}
    fn reset_text(&mut self){self.caption=Caption::Empty;}
}
/// Actual4326b0/4326d0 range sources, in native overwrite order. These are
/// retained live attached-list fields; root must transfer their actual writes,
/// including row-count/range changes, with the list owner (not a Scene).
#[derive(Clone,Debug)]
pub struct ListRange {pub limit480:i32,pub limit2cc:i32,pub first284:i32}
impl ListRange {fn extent(&self)->f32{
    self.limit480.max(self.limit2cc).wrapping_sub(self.first284).wrapping_add(1) as f32
}}
#[derive(Clone,Debug)]
pub struct Selector {
    pub shown:u8,pub admitted191:u8,pub node_disabled:u8,pub extent:f32,
    pub simple:Option<(i32,i32)>, //attached+190,+148
    pub first_list:Option<ListRange>, //+15c
    pub text_list:Option<(u32,u32)>, //42e420 result, attached+2394
    pub last_list:Option<ListRange>, //+160
    pub previous:Widget,pub next:Widget,pub zero:f32,
}
impl Selector {
    pub(crate) fn show(&mut self,value:u8){
        self.shown=value;self.admitted191=value;self.recalculate();
    }
    pub(crate) fn recalculate(&mut self){
        let (extent,effect)=selector_range_controls(self.extent,self.admitted191,self.zero,
            self.simple,self.first_list.as_ref(),self.text_list,self.last_list.as_ref());
        self.extent=extent;
        match effect{
            ControlEffect::Disable=>self.disable(),
            ControlEffect::DisableThenEnable=>{self.disable();self.node_disabled=0;self.previous.node_disabled=0;self.next.node_disabled=0;
                self.previous.disabled=0;self.next.disabled=0;},
            ControlEffect::Enable=>{self.node_disabled=0;self.previous.node_disabled=0;self.next.node_disabled=0;
                self.previous.disabled=0;self.next.disabled=0;},
            ControlEffect::Retain=>(),
        }
    }

    fn disable(&mut self){
        self.node_disabled=1;self.previous.node_disabled=1;self.next.node_disabled=1;
        self.previous.disabled=1;self.previous.down=0;self.next.disabled=1;self.next.down=0;
    }
}
#[derive(Clone,Copy,Debug)]
pub(super) enum ControlEffect {Disable,Enable,DisableThenEnable,Retain}
///Single4326d0 range/control algorithm. Typed production nodes and legacy
///finite property snapshots share it; neither copies selector-show logic.
pub(super) fn selector_range_controls(mut extent:f32,admitted:u8,zero:f32,
    simple:Option<(i32,i32)>,first:Option<&ListRange>,text:Option<(u32,u32)>,last:Option<&ListRange>)
    ->(f32,ControlEffect){
    if let Some((last,first))=simple{extent=last.wrapping_sub(first).wrapping_add(1) as f32;}
    if let Some(list)=first{extent=list.extent();}
    if let Some((count,offset))=text{if count>=offset{extent=count.wrapping_sub(offset) as f32;}}
    if let Some(list)=last{extent=list.extent();}
    if admitted==0{return (extent,ControlEffect::Disable)}
    if extent.partial_cmp(&zero)!=Some(std::cmp::Ordering::Greater){return (0.,if 0.!=zero{ControlEffect::DisableThenEnable}else{ControlEffect::Disable})}
    (extent,if extent!=zero{ControlEffect::Enable}else{ControlEffect::Retain})
}
#[derive(Clone,Debug)]
pub struct Widgets {
    pub previous:Option<Widget>,pub next:Option<Widget>,
    pub report_one:Option<Widget>,pub selector_one:Option<Selector>,
    pub report_three:Option<Widget>,pub selector_three:Option<Selector>,
    pub report_four:Option<Widget>,pub selector_four:Option<Selector>,
    pub report_five:Option<Widget>,pub selector_five:Option<Selector>,
    pub offer:Option<Widget>,pub rights:Option<Widget>,pub auxiliary:Option<Widget>,
    pub marker:Option<Widget>,pub text:Option<Widget>,
}
///Actual parent+4b8 text node control facet. It is not a Visual+338 subtype.
///Caption is a compatibility projection of main.info_text's retained glyphs.
#[derive(Clone,Debug)]
pub struct Title {pub shown:u8,pub caption:Caption}
impl Title {fn reset_text(&mut self){self.caption=Caption::Empty;}}
#[derive(Clone,Debug)]
pub struct State {
    pub reset_flag:u8,pub other_group:u8,pub utility_group:u8,
    pub utility_index:u32,pub other_index:u32,pub active:u8,
    pub mode:u32,pub report:u32,pub profit_period:u32,pub efficiency_period:u32,
    pub infos:[Widget;5],pub title:Title,pub notice:Option<Widget>,
    pub widgets:Widgets,pub rights_explanation:Widget,
    pub offer_labels:[Widget;6],pub amount:f32,pub amount_dirty:u8,pub step:f32,pub minimum:f32,pub maximum:f32,
    pub action_key:u32,pub summary:Vec<crate::block_information::Row>,
    pub summary_text:Vec<(u32,Caption)>,
    pub land:Vec<crate::block_information::land_rent::Row>,
    /// Native report-link metadata retains actual selected object and slot,
    /// never display Hit indices or freshly inferred Scene properties.
    pub land_links:Vec<(Selected,usize)>,pub selected_category:u32,
    pub colors:[u32;4],pub inputs:Inputs,
}
// Construction has no guessed Default. Root installs actual constructor/layout
// state once, then replaces only inputs at the acknowledged live boundary.
fn state(view:&Prospective)->Result<&State,String>{Ok(&view.main.property)}
fn state_mut(view:&mut Prospective)->Result<&mut State,String>{Ok(&mut view.main.property)}
fn main(view:&Prospective)->Result<&super::main_panel::State,String>{Ok(&view.main)}
fn main_mut(view:&mut Prospective)->Result<&mut super::main_panel::State,String>{Ok(&mut view.main)}
fn current<'a>(view:&Prospective,owner:&'a Owner)->Result<Option<ResolvedSelected<'a>>,String>{
    view.resolved_current(owner)
}
fn category<'v>(view:&'v Prospective,selected:ResolvedSelected<'_>)->Result<&'v Category,String>{
    state(view)?.inputs.categories.get(selected.category() as usize).ok_or_else(||"live property category outside captured52 records".into())
}
fn current_category(view:&Prospective,owner:&Owner)->Result<Option<Category>,String>{
    current(view,owner)?.map(|selected|category(view,selected).cloned()).transpose()
}
fn hide(widget:&mut Option<Widget>){if let Some(widget)=widget{widget.shown=0;}}
fn selector_hide(selector:&mut Option<Selector>){if let Some(selector)=selector{selector.show(0);}}
fn mandatory(view:&Prospective)->Result<(),String>{
    let s=state(view)?;
    if s.mode>2{return Err(format!("unsupported retained property mode{}",s.mode));}
    if s.report>5{return Err(format!("property report{} leaves actual info controls",s.report));}
    Ok(())
}
fn classify(view:&mut Prospective,owner:&Owner)->Result<u8,String>{
    let Some(selected)=current(view,owner)? else{return Ok(0)}; //retains flags/indices
    let label=category(view,selected)?.label;
    let utility_labels=state(view)?.inputs.utility_labels;let other_labels=state(view)?.inputs.other_labels;
    let s=state_mut(view)?;s.other_group=0;s.utility_group=0;
    s.utility_index=u32::MAX;s.other_index=u32::MAX;
    for i in 0..4 {
        if utility_labels[i]==label{s.utility_group=1;s.utility_index=i as u32;}
        if other_labels[i]==label{s.other_group=1;s.other_index=i as u32;}
    }
    Ok(u8::from(s.utility_group==1||s.other_group==1))
}
///402c30 ->41f150 reset order. Retains all report rows and widget values.
pub(super) fn reset_owned(view:&mut Prospective)->Result<(),String>{
    state_mut(view)?.reset_flag=0;
    for info in &mut state_mut(view)?.infos {
        info.shown=0;info.set_selected(0);info.disabled=0;
    }
    let s=state_mut(view)?;s.title.shown=0;
    hide(&mut s.widgets.previous);hide(&mut s.widgets.next);
    hide(&mut s.widgets.report_one);selector_hide(&mut s.widgets.selector_one);
    hide(&mut s.widgets.report_three);selector_hide(&mut s.widgets.selector_three);
    hide(&mut s.widgets.report_four);selector_hide(&mut s.widgets.selector_four);
    hide(&mut s.widgets.offer);s.active=0;hide(&mut s.widgets.rights);
    hide(&mut s.widgets.auxiliary);hide(&mut s.widgets.report_five);
    selector_hide(&mut s.widgets.selector_five);hide(&mut s.widgets.marker);hide(&mut s.widgets.text);
    Ok(())
}
pub(super) fn exit_owned(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    current(view,owner)?;
    reset_owned(view)?;super::main_panel::clear_notice(view)?;
    let s=state_mut(view)?;
    hide(&mut s.widgets.previous);hide(&mut s.widgets.next);
    hide(&mut s.widgets.report_four);selector_hide(&mut s.widgets.selector_four);
    hide(&mut s.widgets.offer);s.active=0;hide(&mut s.widgets.rights);
    hide(&mut s.widgets.auxiliary);hide(&mut s.widgets.report_five);
    selector_hide(&mut s.widgets.selector_five);hide(&mut s.widgets.marker);Ok(())
}
fn clear_child_notice(view:&mut Prospective)->Result<(),String>{
    if let Some(notice)=&mut state_mut(view)?.notice{notice.reset_text();}Ok(())
}
fn info_visibility(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    if main(view)?.transition_pending==1{return Ok(())}
    if let Some(selected)=current(view,owner)? {
        if selected.kind()==4&&category(view,selected)?.metadata_group==0 {
            state_mut(view)?.infos[0].shown=1;return Ok(())
        }
    }
    let available=classify(view,owner)?;
    for i in 0..5 {if available==0||available==1&&i<2{state_mut(view)?.infos[i].shown=1;}}
    Ok(())
}
fn select_info_flag(view:&mut Prospective,index:u32)->Result<(),String>{
    state_mut(view)?.infos.get_mut(index as usize).ok_or("reached property info flag outside5")?.set_selected(1);Ok(())
}
fn select_mode_flag(view:&mut Prospective,index:u32,value:u8)->Result<(),String>{
    main_mut(view)?.controls.get_mut(index as usize).ok_or("reached property mode flag outside6")?.set_selected(value);Ok(())
}
fn update_title(view:&mut Prospective)->Result<(),String>{
    super::main_panel::reset_info_text(view)?;
    {let s=state_mut(view)?;s.title.reset_text();s.title.shown=1;}
    let (report,other,utility,index)={let s=state(view)?;(s.report,s.other_group,s.utility_group,s.utility_index)};
    let mut append=|view:&mut Prospective,id:u32|->Result<(),String>{
        super::main_panel::append_info_identifier(view,id)?;state_mut(view)?.title.caption=Caption::Id(id);Ok(())
    };
    match report{
        0=>append(view,28)?,
        1=>{append(view,1402)?;let s=state_mut(view)?;
            s.widgets.report_one.as_mut().ok_or("Land Rent list absent")?.shown=1;
            s.widgets.selector_one.as_mut().ok_or("Land Rent selector absent")?.show(1);},
        2..=4=>return Err("property business reports require synchronous model effect port".into()),
        5=>{if other==1{append(view,1812)?;}if utility==1&&index<=3{append(view,1806+index)?;}},
        _=>(),
    }
    //Native440ad0 fills row palette indices16;440b70 sets font index.
    super::main_panel::color_info_text(view,16)
}
fn dispatch_report(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    let report=state(view)?.report;
    //401a80 clears list BEFORE checking null, unlike the other report bodies.
    if report==1 {let s=state_mut(view)?;s.land.clear();s.land_links.clear();}
    let Some(selected)=current(view,owner)? else{return Ok(())};
    let row=category(view,selected)?.clone();
    match report {
        0=>{if state(view)?.other_group==1||state(view)?.utility_group==1{
                return Err("special utility/railroad403b10 report unsupported".into());
            }
            let materialized=row.summary.iter().map(|item|{
                use crate::block_information::Value;
                let value=match &item.value {
                    None=>Caption::Empty,Some(Value::Label(id))=>Caption::Id(*id),
                    Some(Value::Number(value))=>Caption::Bytes(value.to_string().into_bytes()),
                    Some(Value::Owner(_))=>Caption::Bytes(row.owner_name.clone()),
                };(item.heading,value)
            }).collect();
            let s=state_mut(view)?;s.summary=row.summary;s.summary_text=materialized;},
        1=>{let s=state_mut(view)?;s.land=row.land;
            s.land_links=s.land.iter().filter_map(|r|match r{
                crate::block_information::land_rent::Row::Business(b)=>Some((selected.source(),b.slot)),_=>None}).collect();},
        2..=4=>return Err("property business report model effects unsupported".into()),
        5=>return Err("special utility/railroad403b10 report unsupported".into()),
        _=>(),
    }Ok(())
}
//404e4b compares extendedST0 directly with retainedbinary32 amount. Range
//and replacement have their separatef32stores; a binary64rounding here can
//lose a greater-than result and its observable amount-dirty setter effect.
fn value_exceeds_amount(value:openmt2001::x87::Sum,amount:f32)->bool {
    if amount.is_nan() || amount==f32::INFINITY {return false;}
    if amount==f32::NEG_INFINITY {return true;}
    value.compare(openmt2001::x87::Sum::from_f32(amount).expect("finite retained amount"))
        ==std::cmp::Ordering::Greater
}
fn offer_refresh(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    let Some(row)=current_category(view,owner)? else{return Ok(())};
    if row.offer_pending!=0{state_mut(view)?.active=0;return Ok(())}
    if row.owner_slot==i32::from(state(view)?.inputs.current_slot){
        let s=state_mut(view)?;s.action_key=48;s.active=1;
        s.offer_labels[4].reset_text();s.offer_labels[4].caption=Caption::Id(57);return Ok(())
    }
    {let s=state_mut(view)?;s.step=100.;s.minimum=row.value.to_f32();s.maximum=100000.;}
    //Range setter42efb0 only stores the range, so it cannot change selection.
    let amount=state(view)?.amount;
    if value_exceeds_amount(row.value,amount){let s=state_mut(view)?;s.amount=row.value.to_f32();s.amount_dirty=1;}
    state_mut(view)?.action_key=47;
    if current_category(view,owner)?.is_some_and(|r|r.available!=0){state_mut(view)?.active=1;}
    Ok(())
}
fn offer_display(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    for label in &mut state_mut(view)?.offer_labels{label.reset_text();}
    if let Some(row)=current_category(view,owner)? {
        if row.available!=0 {
            let input=&state(view)?.inputs;
            let money=crate::ui::formatted_float_money(row.value.to_f32(),&input.currency_symbol,input.currency_suffix)?;
            let s=state_mut(view)?;
            for (i,id) in [54,55,56].into_iter().enumerate(){s.offer_labels[i*2].caption=Caption::Id(id);}
            s.offer_labels[1].caption=if row.owner_reference==0{Caption::Id(1116)}else{Caption::Bytes(row.owner_name)};
            s.offer_labels[3].caption=Caption::Glyphs(money);
        }else{state_mut(view)?.offer_labels[0].caption=Caption::Id(58);}
    }offer_refresh(view,owner)
}
fn rights_caption(view:&Prospective,row:&Category)->Result<Option<u32>,String>{
    if row.rights_pending==1{return Ok(Some(1406))}
    if row.owner_reference!=state(view)?.inputs.current_reference {
        if row.rights==0||row.rights==1{return Ok(Some(if row.rights==0{1201}else{1203}))}
    }else if row.rights==0{return Ok(Some(1202))}
    Ok(None) //actual close/clear/republish fallback, explicitly unsupported
}
fn rights_refresh(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    {let s=state_mut(view)?;s.widgets.auxiliary.as_mut().ok_or("rights auxiliary absent")?.shown=0;
        s.rights_explanation.reset_text();}
    let Some(row)=current_category(view,owner)? else{return Ok(())};
    let caption=rights_caption(view,&row)?.ok_or("unsupported404eb0 close/descriptor/republication fallback")?;
    state_mut(view)?.rights_explanation.caption=Caption::Id(caption);
    super::main_model::ensure_expanded(view,owner)?;
    if caption==1202&&main(view)?.transition_pending==0{
        state_mut(view)?.widgets.auxiliary.as_mut().ok_or("rights auxiliary absent")?.shown=1;
    }Ok(())
}
fn activate_admission(view:&Prospective,_:&Owner,selection:Option<ResolvedSelected<'_>>)->Result<(),String>{
    let _=state(view)?;
    let Some(selected)=selection else{return Ok(())};
    if selected.kind()!=4{return Ok(())} //4021a0 source-suppressed branch
    mandatory(view)?;
    let row=category(view,selected)?;
    super::main_panel::check_notice_identifier(view,row.title)?;
    let s=state(view)?;
    if s.mode==1&&(s.widgets.offer.is_none()||s.widgets.rights.is_none()){
        return Err("actual property offer/rights child absent".into());
    }
    if s.mode==1&&row.available!=0 {
        crate::ui::formatted_float_money(row.value.to_f32(),&s.inputs.currency_symbol,s.inputs.currency_suffix)?;
    }
    if s.mode==2 {
        if s.widgets.auxiliary.is_none(){return Err("actual rights auxiliary absent".into());}
        if rights_caption(view,row)?.is_none(){return Err("unsupported404eb0 close/descriptor/republication fallback".into());}
        if main(view)?.expanded==0{super::main_model::check_prepare(view,10)?;}
    }
    if s.mode==0 {
        if s.other_group==1||s.utility_group==1||s.inputs.utility_labels.contains(&row.label)||s.inputs.other_labels.contains(&row.label){
            return Err("unsupported selected utility/railroad403b10 report".into());
        }
        if s.report>=1{return Err(format!("unsupported selected property report{} list/model effects",s.report));}
    }Ok(())
}
pub(super) fn activate_owned(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    activate_admission(view,owner,current(view,owner)?)?;
    let Some(selected)=current(view,owner)? else{state_mut(view)?.selected_category=u32::MAX;return Ok(())};
    if selected.kind()!=4{return Ok(())}
    if state(view)?.selected_category!=selected.category() {
        state_mut(view)?.selected_category=selected.category();
        if state(view)?.widgets.offer.is_some(){
            let live=current(view,owner)?.ok_or("property selection disappeared at amount initialization")?;
            let amount=category(view,live)?.selected_amount;let s=state_mut(view)?;s.amount=amount;s.amount_dirty=1;
        }
    }
    let Some(selected)=current(view,owner)? else{return Err("property selection disappeared after amount control".into())};
    if selected.kind()!=4{return Ok(())}
    super::main_panel::clear_notice(view)?;
    let selected_category=state(view)?.selected_category;
    let row=state(view)?.inputs.categories.get(selected_category as usize).ok_or("cached selected property category")?.clone();
    super::main_panel::set_notice_identifier(view,row.title)?;
    let first=row.color;let faded=(first&0x00ffffff)|(((first>>24)/3)<<24);
    state_mut(view)?.colors=[first,row.color,faded,faded];
    match state(view)?.mode {2=>rights_refresh(view,owner),1=>offer_display(view,owner),0=>dispatch_report(view,owner),_=>Ok(())}
}
fn info_mode(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    let report=state(view)?.report;select_info_flag(view,report)?;info_visibility(view,owner)?;
    update_title(view)?;activate_owned(view,owner)
}
fn rights_mode(view:&mut Prospective,owner:&Owner,requested:u32)->Result<(),String>{
    exit_owned(view,owner)?;clear_child_notice(view)?;state_mut(view)?.mode=requested;
    match requested {2=>rights_refresh(view,owner),1=>offer_refresh(view,owner),0=>info_mode(view,owner),_=>Ok(())}
}
fn lock_state(view:&mut Prospective,owner:&Owner,index:u32)->Result<(),String>{
    let row=state(view)?.inputs.categories.get(index as usize).ok_or("offer lock category outside52")?.clone();
    if row.offer_pending==1 {
        select_mode_flag(view,1,0)?;select_mode_flag(view,2,0)?;
        {let m=main_mut(view)?;m.controls[2].disabled=1;m.controls[2].down=0;}
        select_mode_flag(view,0,1)?;exit_owned(view,owner)?;clear_child_notice(view)?;
        state_mut(view)?.mode=0;info_mode(view,owner)?;
    }else if row.rights==2||row.rights==1&&row.available!=0{
        select_mode_flag(view,2,0)?;main_mut(view)?.controls[2].disabled=0;
    }Ok(())
}
fn enter_admission(view:&Prospective,owner:&Owner,selection:Option<ResolvedSelected<'_>>)->Result<(),String>{
    enter_check_pending(view,owner,selection,main(view)?.transition_pending)
}
fn enter_check_pending(view:&Prospective,owner:&Owner,selection:Option<ResolvedSelected<'_>>,pending:u8)->Result<(),String>{
    mandatory(view)?;
    if let Some(selected)=selection {
        if selected.kind()!=4{return Err(format!("unsupported property entry kind{} (actor/nonproperty)",selected.kind()));}
        let row=category(view,selected)?;let s=state(view)?;
        if s.inputs.utility_labels.contains(&row.label)||s.inputs.other_labels.contains(&row.label){
            return Err("unsupported utility/railroad property entry403b10".into());
        }
        //Metadata-group0 forces report0 first. A pending offer keeps the
        //retained report and can reach unported list/model effects.
        if s.report>=1&&s.report!=5&&row.metadata_group!=0&&(s.mode==0||row.offer_pending==1){
            return Err(format!("unsupported property entry retained report{} model effects",s.report));
        }
        if s.report==5&&row.offer_pending==1&&row.metadata_group!=0{
            return Err("pending property retained report5 indexes absent info5".into());
        }
        if s.mode==1&&row.offer_pending!=1&&row.metadata_group!=0&&(s.widgets.offer.is_none()||s.widgets.rights.is_none()){
            return Err("actual property offer/rights child absent".into());
        }
        if s.mode==1&&row.available!=0&&row.offer_pending!=1&&row.metadata_group!=0 {
            crate::ui::formatted_float_money(row.value.to_f32(),&s.inputs.currency_symbol,s.inputs.currency_suffix)?;
        }
        if s.mode==2&&row.offer_pending!=1&&row.metadata_group!=0&&s.widgets.auxiliary.is_none(){
            return Err("actual rights auxiliary absent".into());
        }
        if s.mode==2&&row.offer_pending!=1&&row.metadata_group!=0&&rights_caption(view,row)?.is_none(){
            return Err("unsupported rights close/descriptor/republication fallback".into());
        }
    }
    if state(view)?.mode==2&&pending!=1&&main(view)?.expanded!=0&&state(view)?.widgets.auxiliary.is_none(){
        return Err("actual rights auxiliary absent".into());
    }
    //None still reaches title and retained-mode work; do not bypass it.
    if pending!=1&&main(view)?.expanded!=0 {
        if state(view)?.report==1&&state(view)?.mode==0&&(state(view)?.widgets.report_one.is_none()||state(view)?.widgets.selector_one.is_none()){
            return Err("actual property report1 title controls absent".into());
        }
        if state(view)?.mode==0&&state(view)?.report>=2&&state(view)?.report<=4 {
            return Err("unsupported property entry report2/3/4 title/list controls".into());
        }
    }
    let _=owner;Ok(())
}
pub(super) fn enter_owned(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    enter_admission(view,owner,current(view,owner)?)?;
    for i in 0..3 {
        let image=state(view)?.inputs.mode_images[i];let animation=state(view)?.inputs.mode_titles[i];
        let button=&mut main_mut(view)?.controls[i];button.shown=1;
        let [l,t,r,b]=button.bounds;button.set_image([l,t,r,b,0.5],image);
        button.animate(animation);
    }
    let special=classify(view,owner)?;
    if special==1{main_mut(view)?.controls[2].shown=0;}
    let group_zero=if let Some(selected)=current(view,owner)? {
        selected.kind()==4&&category(view,selected)?.metadata_group==0
    }else{false};
    if group_zero {
        {let m=main_mut(view)?;m.controls[1].shown=0;m.controls[2].shown=0;}
        reset_owned(view)?;info_visibility(view,owner)?;state_mut(view)?.report=0;
        update_title(view)?;dispatch_report(view,owner)?;let report=state(view)?.report;select_info_flag(view,report)?;
        exit_owned(view,owner)?;clear_child_notice(view)?;state_mut(view)?.mode=0;info_mode(view,owner)?;
        let report=state(view)?.report;select_mode_flag(view,report,1)?;
    }else{
        let available=current_category(view,owner)?.is_some_and(|r|r.available!=0);
        {let button=&mut main_mut(view)?.controls[1];button.disabled=u8::from(!available);if !available{button.down=0;}}
        let enable=if let Some(row)=current_category(view,owner)? {
            row.rights==2||row.rights==1&&row.available!=0||row.rights==0&&row.owner_reference==state(view)?.inputs.current_reference
        }else{false};
        let button=&mut main_mut(view)?.controls[2];button.disabled=u8::from(!enable);if !enable{button.down=0;}
    }
    if let Some(selected)=current(view,owner)?{lock_state(view,owner,selected.category())?;}
    if main(view)?.transition_pending==1||main(view)?.expanded==0{return Ok(())}
    for i in 0..5 {
        let input=&state(view)?.inputs;
        let (inset,special_image,image,animation)=(input.inset,input.special_image,input.info_images[i],input.info_titles[i]);
        let s=state_mut(view)?;let button=&mut s.infos[i];
        button.shown=u8::from(special==0||special==1&&i<1);
        let [l,t,r,b]=button.bounds;
        let extent=[(f64::from(l)*inset+f64::from(l))as f32,
            (f64::from(t)*inset+f64::from(t))as f32,
            (f64::from(r)-f64::from(r)*inset)as f32,
            (f64::from(b)-f64::from(b)*inset)as f32,0.5];
        if special==1&&i==1 {
            button.set_image(extent,special_image);
            if s.other_group==1{button.animate(1813);}
            if s.utility_group==1&&s.utility_index<=3{button.animate(1806+s.utility_index);}
        }else{button.set_image(extent,image);button.animate(animation);}
    }
    let now_special=classify(view,owner)?;let report=state(view)?.report;
    if now_special!=0&&(report as i32)>=1&&report==5 {
        state_mut(view)?.report=5;select_live_mode(view,owner)?;restore_live_mode(view,owner)?;
        select_info_flag(view,1)?;return Ok(())
    }
    if report==5||now_special!=0&&(report as i32)>=1 {
        state_mut(view)?.report=0;super::main_model::ensure_expanded(view,owner)?;
        rights_mode(view,owner,0)?;select_mode_flag(view,0,1)?;activate_owned(view,owner)?;
        exit_owned(view,owner)?;clear_child_notice(view)?;state_mut(view)?.mode=0;return info_mode(view,owner)
    }
    select_live_mode(view,owner)?;restore_live_mode(view,owner)
}
fn select_live_mode(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    let retained=state(view)?.mode;
    match retained {
        2=>rights_mode(view,owner,2)?,
        1=>{super::main_model::ensure_expanded(view,owner)?;rights_mode(view,owner,1)?;},
        0=>{super::main_model::ensure_expanded(view,owner)?;exit_owned(view,owner)?;
            clear_child_notice(view)?;state_mut(view)?.mode=0;info_mode(view,owner)?;},_=>(),
    }
    select_mode_flag(view,retained,1)?;activate_owned(view,owner)
}
fn restore_live_mode(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    let restored=state(view)?.mode;exit_owned(view,owner)?;clear_child_notice(view)?;
    state_mut(view)?.mode=restored;
    match restored {2=>rights_refresh(view,owner),1=>offer_refresh(view,owner),0=>info_mode(view,owner),_=>Ok(())}
}
/// Source40463/401d10 input helpers for admitted modes/reports. Parent's whole
/// event12/28 dispatcher remains unported and MUST NOT silently route here.
pub(super) fn select_mode_owned(view:&mut Prospective,owner:&Owner,requested:u32)->Result<(),String>{
    current(view,owner)?;
    if requested>2{return Err("unsupported requested property mode".into())}
    let mut prospective=state(view)?.clone();prospective.mode=requested;
    //Validate prospective mode before prepare/exit; checks below use explicit
    //current input, not temporary writes to the live mode.
    if requested==2 {
        if let Some(row)=current_category(view,owner)? {
            if rights_caption(view,&row)?.is_none(){return Err("unsupported requested rights republish fallback".into());}
        }
    }
    if requested==0&&prospective.report>0{return Err("unsupported requested report mode model effects".into())}
    mandatory(view)?;
    if let Some(selected)=current(view,owner)? {
        if selected.kind()!=4{return Err("unsupported nonproperty mode input".into());}
        let row=category(view,selected)?;let input=&state(view)?.inputs;
        if input.utility_labels.contains(&row.label)||input.other_labels.contains(&row.label){return Err("unsupported special property mode input".into());}
        if requested==1&&row.available!=0{crate::ui::formatted_float_money(row.value.to_f32(),&input.currency_symbol,input.currency_suffix)?;}
    }
    if requested==1&&(state(view)?.widgets.offer.is_none()||state(view)?.widgets.rights.is_none()){
        return Err("actual offer/rights mode controls absent".into());
    }
    if requested==2&&state(view)?.widgets.auxiliary.is_none(){return Err("actual rights mode auxiliary absent".into());}
    if requested<=1{
        if main(view)?.expanded==0{super::main_model::check_prepare(view,10)?;}
        super::main_model::ensure_expanded(view,owner)?;
    }
    rights_mode(view,owner,requested)?;select_mode_flag(view,requested,1)?;activate_owned(view,owner)
}
pub(super) fn select_report_owned(view:&mut Prospective,owner:&Owner,requested:u32)->Result<(),String>{
    current(view,owner)?;
    if requested>0{return Err("unsupported report input requires exact retained list/model effects".into())}
    if let Some(selected)=current(view,owner)? {
        if selected.kind()!=4{return Err("unsupported nonproperty report input".into());}
        let row=category(view,selected)?;let input=&state(view)?.inputs;
        if input.utility_labels.contains(&row.label)||input.other_labels.contains(&row.label){return Err("unsupported403b10 special report input".into());}
    }
    if requested==1&&(state(view)?.widgets.report_one.is_none()||state(view)?.widgets.selector_one.is_none()){
        return Err("actual Land Rent control absent".into())
    }
    classify(view,owner)?;reset_owned(view)?;info_visibility(view,owner)?;
    state_mut(view)?.report=requested;update_title(view)?;dispatch_report(view,owner)?;
    let report=state(view)?.report;select_info_flag(view,report)
}

///4063f0 finite actual no-selection reset. This is NOT child exit402d10,
///constructor401000 or a blanket report/mode default on every Scene refresh.
fn reset_no_selection_admission(view:&Prospective,_:&Owner,_:Option<Selected>)->Result<(),String>{
    if view.current.is_some(){return Err("property4063f0 selected reset remains unsupported".into())}
    let s=state(view)?;
    super::main_panel::check_info_identifier(view,28)?;
    //Layout4055f0 allocated these unconditionally before reset dereferences.
    if s.widgets.previous.is_none()||s.widgets.next.is_none()||s.widgets.offer.is_none()
        ||s.widgets.rights.is_none()||s.widgets.auxiliary.is_none(){
        return Err("property4063f0 lacks actual mandatory layout controls".into())
    }
    Ok(())
}
pub(super) fn reset_no_selection_owned(view:&mut Prospective,owner:&Owner)->Result<(),String>{
    current(view,owner)?;
    reset_no_selection_admission(view,owner,view.current)?;
    {let s=state_mut(view)?;s.efficiency_period=1;s.profit_period=1;
        hide(&mut s.widgets.auxiliary);hide(&mut s.widgets.rights);hide(&mut s.widgets.offer);
        hide(&mut s.widgets.previous);hide(&mut s.widgets.next);}
    //41f120, actual six controls: preserve down335 and unrelated native cells.
    for control in &mut main_mut(view)?.controls{control.shown=0;control.set_selected(0);control.disabled=0;}
    {let s=state_mut(view)?;s.mode=0;s.report=0;}
    reset_owned(view)?;info_visibility(view,owner)?;
    state_mut(view)?.report=0;update_title(view)?;dispatch_report(view,owner)?;
    let report=state(view)?.report;select_info_flag(view,report)?;
    exit_owned(view,owner)?;clear_child_notice(view)?;
    state_mut(view)?.mode=0;let report=state(view)?.report;select_info_flag(view,report)?;
    info_visibility(view,owner)?;update_title(view)?;activate_owned(view,owner)
}
/// Every public finite property operation executes fully on disposable
/// Main/Text/Fonts/model state. Failed preflight/apply never mutates live UI.
fn check_operation(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>,pending:Option<u8>,
    operation:fn(&mut Prospective,&Owner)->Result<(),String>)->Result<(),String>{
    let selection=owner.validate_ui_selected(selection)?;
    let mut plan=Prospective::capture(view,selection)?;
    if let Some(pending)=pending{plan.main.transition_pending=pending;}
    operation(&mut plan,owner)
}
pub fn reset(view:&mut impl super::main_panel::Access)->Result<(),String>{
    super::main_panel::transact(view,None,reset_owned)
}
pub fn exit_check(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>)->Result<(),String>{check_operation(view,owner,selection,None,exit_owned)}
pub fn activate_check(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>)->Result<(),String>{check_operation(view,owner,selection,None,activate_owned)}
pub fn enter_check(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>)->Result<(),String>{check_operation(view,owner,selection,None,enter_owned)}
pub fn enter_transition_check(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>)->Result<(),String>{check_operation(view,owner,selection,Some(0),enter_owned)}
pub fn after_enter_check(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>)->Result<(),String>{
    check_operation(view,owner,selection,None,|p,o|{enter_owned(p,o)?;activate_owned(p,o)})
}
pub fn after_transition_check(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>)->Result<(),String>{
    check_operation(view,owner,selection,Some(0),|p,o|{enter_owned(p,o)?;activate_owned(p,o)})
}
pub fn reset_no_selection_check(view:&impl super::main_panel::Access,owner:&Owner,selection:Option<Selected>)->Result<(),String>{
    check_operation(view,owner,selection,None,reset_no_selection_owned)
}
pub fn exit(view:&mut impl super::main_panel::Access,owner:&mut Owner)->Result<(),String>{
    let current=owner.current_ui_selected()?;
    super::main_panel::transact(view,current,|p|exit_owned(p,owner))
}
pub fn activate(view:&mut impl super::main_panel::Access,owner:&mut Owner)->Result<(),String>{
    let current=owner.current_ui_selected()?;
    super::main_panel::transact(view,current,|p|activate_owned(p,owner))
}
pub fn enter(view:&mut impl super::main_panel::Access,owner:&mut Owner)->Result<(),String>{
    let current=owner.current_ui_selected()?;
    super::main_panel::transact(view,current,|p|enter_owned(p,owner))
}
pub fn reset_no_selection(view:&mut impl super::main_panel::Access,owner:&mut Owner)->Result<(),String>{
    let current=owner.current_ui_selected()?;
    super::main_panel::transact(view,current,|p|reset_no_selection_owned(p,owner))
}
pub fn select_mode(view:&mut impl super::main_panel::Access,owner:&mut Owner,requested:u32)->Result<(),String>{
    let current=owner.current_ui_selected()?;
    super::main_panel::transact(view,current,|p|select_mode_owned(p,owner,requested))
}
pub fn select_report(view:&mut impl super::main_panel::Access,owner:&mut Owner,requested:u32)->Result<(),String>{
    let current=owner.current_ui_selected()?;
    super::main_panel::transact(view,current,|p|select_report_owned(p,owner,requested))
}
pub fn child()->super::main_panel::Child{
    use super::main_panel::{Child,Step};
    Child{exit:Step{check:process_exit_check,apply:process_exit},enter:Step{check:process_enter_check,apply:process_enter},activate:Step{check:process_activate_check,apply:process_activate}}
}
pub fn reset_step()->super::main_panel::Step{super::main_panel::Step{check:process_reset_no_selection_check,apply:process_reset_no_selection}}

#[cfg(test)]
mod extended_offer_compare_tests {
    use super::value_exceeds_amount;
    use openmt2001::x87::Sum;
    #[test]
    fn unspilled_value_above_retained_amount_still_requires_setter() {
        let mut value=Sum::from_i32(1);
        value.add(Sum::from_f64(2f64.powi(-60)).unwrap());
        assert_eq!(value.to_f64(),1.); //Former comparison incorrectly said equal.
        assert_eq!(value.to_f32(),1.); //Setter still has observable dirty effects.
        assert!(value_exceeds_amount(value,1.));
        assert!(!value_exceeds_amount(Sum::from_i32(1),1.));
        assert!(!value_exceeds_amount(value,f32::NAN));
        assert!(!value_exceeds_amount(value,f32::INFINITY));
        assert!(value_exceeds_amount(value,f32::NEG_INFINITY));
        assert!(!value_exceeds_amount(Sum::default(),-0.));
    }
}

#[cfg(test)]
mod preflight_regressions {
    use super::*;
    use super::super::main_panel::{transact,regression_tests::{fixture,owner,selected,unchanged_on_error}};
    #[test]
    fn ordinary_none_entry_and_report_input_preflight_the_actual_shared_title(){
        let mut p=fixture();let owner=owner(None);p.fonts.language.test_replace(28,vec![5]);
        assert!(unchanged_on_error(&mut p,|p|enter_owned(p,&owner)).contains("formatting"));
        assert!(unchanged_on_error(&mut p,|p|select_report_owned(p,&owner,0)).contains("formatting"));
        p.fonts.language.test_replace(28,vec![1]);
        transact(&mut p,None,|p|select_report_owned(p,&owner,0)).unwrap();
        assert_eq!(p.main.property.report,0);assert_eq!(p.main.info_text.colors[0],16);assert_eq!(p.main.info_text.font,0);
    }
    #[test]
    fn retained_report_one_validates_its_identifier_and_controls_before_commit(){
        let mut p=fixture();let owner=owner(None);p.main.property.report=1;p.fonts.language.test_replace(1402,vec![5]);
        assert!(unchanged_on_error(&mut p,|p|enter_owned(p,&owner)).contains("formatting"));
        p.fonts.language.test_replace(1402,vec![2]);
        transact(&mut p,None,|p|enter_owned(p,&owner)).unwrap();
        assert_eq!(p.main.property.title.caption,Caption::Id(1402));assert_eq!(p.main.info_text.group.row(0).unwrap(),&[2]);
    }
    #[test]
    fn report_five_runs_both_ordered_appends_on_one_prospective_text_and_manager(){
        let mut p=fixture();p.main.property.report=5;p.main.property.other_group=1;p.main.property.utility_group=1;p.main.property.utility_index=0;
        p.fonts.language.test_replace(1812,vec![1]);p.fonts.language.test_replace(1806,vec![2]);
        p.main.info_text.colors=vec![0;2];p.main.info_text.blink_colors=vec![0;1];p.main.info_text.blink=vec![0;2];p.main.info_text.x=vec![0;2];
        assert!(unchanged_on_error(&mut p,update_title).contains("capacities"),"second append reaches count2 after first succeeds");
        p.main.info_text.blink_colors.resize(2,0);
        transact(&mut p,None,update_title).unwrap();
        assert_eq!(p.main.info_text.group.count,2);assert_eq!(p.main.info_text.group.row(0).unwrap(),&[1]);assert_eq!(p.main.info_text.group.row(1).unwrap(),&[2]);
        assert_eq!(&p.main.info_text.colors[..2],&[16,16]);assert_eq!(p.main.property.title.caption,Caption::Id(1806));
        assert_eq!(p.fonts.formatted.row(0).unwrap(),&[2],"same manager retains second append");
    }
    #[test]
    fn direct_mode_input_discards_model_and_control_writes_on_notice_reset_or_identifier_failure(){
        let mut p=fixture();let no_selection=owner(None);p.main.expanded=0;p.main.notice_text.bounds[1]=f32::INFINITY;
        assert!(unchanged_on_error(&mut p,|p|select_mode_owned(p,&no_selection,0)).contains("finite"));
        p.main.notice_text.bounds[1]=0.;p.current=Some(selected());let selected_owner=owner(p.current);
        p.main.property.inputs.categories[0].title=9999;
        assert!(unchanged_on_error(&mut p,|p|select_mode_owned(p,&selected_owner,1)).contains("identifier"));
        p.main.property.inputs.categories[0].title=100;
        let current=p.current;transact(&mut p,current,|p|select_mode_owned(p,&selected_owner,1)).unwrap();
        assert_eq!(p.main.property.mode,1);assert_eq!(p.main.notice_caption,Some(100));
    }
    #[test]
    fn missing_title_glyph_and_zero_retained_pitch_cannot_commit_entry(){
        let mut p=fixture();let owner=owner(None);p.fonts.language.test_replace(28,vec![250]);p.fonts.fonts.get_mut(&0).unwrap().table.truncate(4+100*44);
        assert!(unchanged_on_error(&mut p,|p|enter_owned(p,&owner)).contains("metric"));
        p.fonts=super::super::main_panel::regression_tests::fonts();
        p.main.info_text.pitch=0; //default reset retains pitch; no W-metric dispatch
        assert!(unchanged_on_error(&mut p,|p|enter_owned(p,&owner)).contains("pitch zero"));
    }
}

//Concrete stored callback ABI; forwards once to the same finite owned transaction.
fn process_exit_check(view:&super::main_panel::ProcessTextAccess<'_>,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    exit_check(view,owner,current)
}
fn process_exit(view:&mut super::main_panel::ProcessTextAccess<'_>,owner:&mut Owner)->Result<(),String>{
    exit(view,owner)
}
fn process_enter_check(view:&super::main_panel::ProcessTextAccess<'_>,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    enter_check(view,owner,current)
}
fn process_enter(view:&mut super::main_panel::ProcessTextAccess<'_>,owner:&mut Owner)->Result<(),String>{
    enter(view,owner)
}
fn process_activate_check(view:&super::main_panel::ProcessTextAccess<'_>,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    activate_check(view,owner,current)
}
fn process_activate(view:&mut super::main_panel::ProcessTextAccess<'_>,owner:&mut Owner)->Result<(),String>{
    activate(view,owner)
}
fn process_reset_no_selection_check(view:&super::main_panel::ProcessTextAccess<'_>,owner:&Owner,current:Option<Selected>)->Result<(),String>{
    reset_no_selection_check(view,owner,current)
}
fn process_reset_no_selection(view:&mut super::main_panel::ProcessTextAccess<'_>,owner:&mut Owner)->Result<(),String>{
    reset_no_selection(view,owner)
}
