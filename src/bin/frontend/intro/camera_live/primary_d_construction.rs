//! Finite actual43cc60 prefix through D43cdcd, retaining all later members.
//! Current Assets' pre-existing F/Main effects remain an explicit ordering gap.
use super::{case_zero_text::Text,main_construction::ResourceIdentity,
    panel_models::CpuModel,primitive_factory::{Primitives,DescriptorIdentity,Phase},
    process_images::Images,registration::{self,Descriptor},navigation::Group,
    ui_graph::PanelId,ui_text_resources::UiTextResources};
use crate::live_host::camera_bridge::{ResidentSource,SessionServices,SessionReply,SessionCall};
use openmt2001::{archive::ArchiveIndex,mesh::CachedMesh,pe::PeImage};
use std::sync::atomic::{AtomicU64,Ordering};
const D:u32=0x574488;
const G:u32=0x57e710;
static NEXT:AtomicU64=AtomicU64::new(1);
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Currency {pub symbol:Vec<u8>,pub probe:Vec<u8>,pub suffix:u32}
impl Currency {
    pub(super) fn from_language(text:&UiTextResources)->Result<Self,String>{
        //Same selected packed language/board allocation,4e78e0 first-match.
        let language=&text.fonts().language;let alphabet=language.row(0)?;
        let encode=|id|->Result<Vec<u8>,String>{
            let mut out=Vec::new();
            for glyph in language.row(id)? {
                let at=alphabet.iter().position(|v|v==glyph).ok_or("4e78e0 currency glyph lacks actual alphabet mapping")?;
                let raw=(at as u8).wrapping_add(32);
                if raw==0{return Err("4e78e0 currency encoded embedded terminator outside finite domain".into())}
                out.push(raw);
            }out.push(0);Ok(out)
        };
        let symbol=encode(2243)?;let probe=encode(2244)?;
        if symbol.len()>0x24||probe.len()>4{return Err("actual currency/probe exceeds source output storage".into())}
        let suffix=u32::from(probe[0]!=b'l');Ok(Self{symbol,probe,suffix})
    }
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum PrimaryPhase {Allocated,Preparing,AwaitingDLayout,LayingOutD,AwaitingVoting,AwaitingDRebind,Failed}
///An explicit independent camera-install outcome, never a Graph/D completion
///when the actual warm layout/reset source path has not been joined.
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum InstallDisposition {CompletedD{epoch:u64},ParkedAwaitingDRebind{constructed_epoch:u64,requested_epoch:u64}}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum OrderingBoundary {AwaitingOrderedMain}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct TextId {owner:u64,slot:usize}
pub struct LegendText {
    pub id:TextId,pub parent:u32,pub root:u32,pub number:u32,
    pub text:Text,pub category:Option<usize>,pub title:Option<i32>,pub title_record:Option<u32>,
    pub initialized:bool,
}
pub struct DProduct {
    pub source:u32,pub descriptor:DescriptorIdentity,pub rows:Vec<Box<LegendText>>,
    pub phase:PrimaryPhase,pub failure:Option<String>,pub epoch:Option<u64>,
    pub active_epoch:Option<u64>,pub parked_for:Option<u64>,live_source:Option<ResidentSource>,
    owner:u64,
}
///Stable owner from Assets load through the first actual protected Install.
///No Graph/Source/input conversion or a synthetic completed full group exists.
pub struct PrimaryConstruction {
    d:Box<DProduct>,currency:Currency,process:ResourceIdentity,text:ResourceIdentity,images:ResourceIdentity,
}
impl PrimaryConstruction {
    pub fn phase(&self)->PrimaryPhase{self.d.phase}
    pub fn d(&self)->&DProduct{&self.d}
    pub fn currency(&self)->&Currency{&self.currency}
    pub fn ordering_boundary(&self)->OrderingBoundary{OrderingBoundary::AwaitingOrderedMain}
    pub fn require_ordered_main(&self)->Result<(),String>{Err("AwaitingOrderedMain: retained F already Bound/Main layout already completed before native12-member prefix; SAME products parked, no replay/order completion".into())}
    pub fn initialize_currency(p:&mut Primitives,text:&UiTextResources)->Result<Currency,String>{p.primary_currency(text)}
    pub fn prepare(executable:&[u8],index:&ArchiveIndex,archive:&[u8],currency:Currency,
        p:&mut Primitives,text:&UiTextResources,images:&Images)->Result<Self,String>{
        //No consumed owner or resource change on preflight failure.
        if p.primary_claimed{return Err("Primary prefix already claimed".into())}
        if images.ui_dimensions!=[640,480]{return Err("D requires SAME640 image domain".into())}
        if p.primary_currency_identity.as_ref().is_none_or(|id|!id.same(&text.construction_identity)){return Err("Primary prelude belongs to another actual language owner".into())}
        if Currency::from_language(text)?!=currency{return Err("Primary currency language/board changed before containing prepare".into())}
        if p.cells().ntbs(0x57e6ec,0x24)?!=currency.symbol[..currency.symbol.len()-1]||p.cells().word(0x59050c)?!=currency.suffix{
            return Err("Primary currency real57e6ec/59050c product differs".into())
        }
        let pe=PeImage::parse(executable)?;
        for (at,n) in [(0x439840,0x78),(0x42a370,0x285),(0x4df9c0,0x7f),(0x43cc60,0x17d),
            (0x43b770,0x310),(0x43e9a0,0x197),(0x5255f0,0x3c),(0x525908,0x34),(0x559730,48)]{
            if pe.at(at,n)?!=p.cells().read(at,n)?{return Err(format!("Primary source{at:x} differs from retained actual PE"))}
        }
        if p.cells().word(0x5255f4)?!=0x42a370||p.cells().word(0x52590c)?!=0x42f140{return Err("D/Text actual slot04 differs".into())}
        //All16 true pool sources. Preserve already-owned A/F model allocations.
        //A/G duplicate path denotes two distinct mutable source loads.
        let mut missing=Vec::new();
        for ordinal in 0..16u8 {
            let entry=registration::PRIMARY.iter().find(|entry|matches!(entry.descriptor,Descriptor::Model(i) if i==ordinal)).ok_or("Primary actual pool ordinal absent")?;
            if p.resources.models.contains_key(&entry.id){
                p.resources.resolve(entry.id)?;continue
            }
            let name=registration::asset(p.cells(),Group::Primary,ordinal)?;
            let source=index.find(&name).ok_or_else(||format!("Primary{ordinal}: actual archive entry {name} missing"))?;
            let end=source.offset.checked_add(source.length).ok_or("Primary archive range overflow")?;
            let bytes=archive.get(usize::try_from(source.offset).map_err(|_|"Primary offset")?..usize::try_from(end).map_err(|_|"Primary end")?).ok_or("Primary source range leaves archive")?;
            missing.push(CpuModel::parsed(entry.id,source,&CachedMesh::parse(bytes)?)?);
        }
        //Validate caption constructor consumed preimages BEFORE claiming.
        for o in [0x188,0x4ac,0xa7c,0xc34]{p.cells().word(D+o+0x140)?;}
        let owner=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|n|n.checked_add(1)).map_err(|_|"D typed owner IDs exhausted")?;
        let mut this=Self{d:Box::new(DProduct{source:D,descriptor:DescriptorIdentity::EmbeddedD,rows:Vec::new(),
            phase:PrimaryPhase::AwaitingDLayout,failure:None,epoch:None,active_epoch:None,parked_for:None,live_source:None,owner}),currency,
            process:p.construction_identity.clone(),text:text.construction_identity.clone(),images:images.construction_identity.clone()};
        //Retain the reached partial owner, even if a later static effect faults.
        if let Err(error)=p.primary_prepare(missing){this.fail(error);}
        Ok(this)
    }
    /// The fresh controller creates its retained D Box before any static
    /// constructor or43cc60 effect, with no conversion from eager products.
    pub(super) fn deferred_owner(currency:Currency,p:&Primitives,text:&UiTextResources,images:&Images)->Result<Self,String>{
        let owner=NEXT.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|n|n.checked_add(1)).map_err(|_|"D typed owner IDs exhausted")?;
        Ok(Self{d:Box::new(DProduct{source:D,descriptor:DescriptorIdentity::EmbeddedD,rows:Vec::new(),
            phase:PrimaryPhase::Allocated,failure:None,epoch:None,active_epoch:None,parked_for:None,live_source:None,owner}),currency,
            process:p.construction_identity.clone(),text:text.construction_identity.clone(),images:images.construction_identity.clone()})
    }
    ///Concrete fresh prefix, not a phase-completion/mark-done API.
    pub(super) fn begin_deferred(&mut self,source:&ResidentSource,services:&mut SessionServices<'_>,
        p:&mut Primitives,text:&UiTextResources)->Result<(),String>{
        if self.d.phase!=PrimaryPhase::Allocated||!self.process.same(&p.construction_identity)||!self.text.same(&text.construction_identity){
            return Err("Fresh containing prefix owner/phase differs; no replay".into())
        }
        p.check_deferred_ready(text)?;
        if Currency::from_language(text)?!=self.currency{return Err("Fresh currency source changed before first Install".into())}
        match services.call(SessionCall::PrimaryCategory{source:source.clone(),row:None}).map_err(|e|e.to_string())?{
            SessionReply::PrimaryCategory(None)=>(),_=>return Err("Fresh category domain reply differs".into()),
        }
        match p.deferred_prefix(&self.currency,text){
            Ok(())=>{self.d.phase=PrimaryPhase::AwaitingDLayout;Ok(())},
            Err(error)=>{p.primary_fail_d();self.fail(error.clone());Err(error)},
        }
    }
    pub(super) fn fail(&mut self,error:String){self.d.phase=PrimaryPhase::Failed;self.d.active_epoch=None;self.d.failure=Some(error);}
    pub fn d_current_for(&self,epoch:u64)->bool{
        self.d.phase==PrimaryPhase::AwaitingVoting&&self.d.active_epoch==Some(epoch)
            &&self.d.live_source.as_ref().is_some_and(|s|s.validate_live().is_ok())
    }
    ///Actual Intro city-exit hook invalidates current-world D before the
    ///existing Endpoint retires sources and wakes the waiting worker.
    pub fn retire_for_camera(&mut self,p:&mut Primitives)->Result<(),String>{
        if !self.process.same(&p.construction_identity){return Err("D retirement belongs to another process allocation".into())}
        if self.d.phase==PrimaryPhase::AwaitingVoting {
            p.primary_park_d()?;self.d.phase=PrimaryPhase::AwaitingDRebind;
            self.d.active_epoch=None;self.d.parked_for=self.d.epoch;self.d.live_source=None;
        }Ok(())
    }
    pub fn require_current_d(&self,epoch:u64)->Result<(),String>{
        if self.d_current_for(epoch){Ok(())}else{Err("D has no bound/current layout for this World; AwaitingDRebind/Graph remains closed".into())}
    }
    ///Production Context calls this only during the existing protected Install.
    ///The source has no category copy; each read is of SAME Session World now.
    pub fn install(&mut self,source:&ResidentSource,generation:u64,services:&mut SessionServices<'_>,
        p:&mut Primitives,text:&mut UiTextResources,images:&Images)->Result<InstallDisposition,String>{
        if !self.process.same(&p.construction_identity)||!self.text.same(&text.construction_identity)||!self.images.same(&images.construction_identity){return Err("D continuation requires SAME process/Fonts/images allocations".into())}
        if images.ui_dimensions!=[640,480]||source.epoch().serial()!=generation||generation!=services.generation(){return Err("D source/resource/domain/operation epoch mismatch".into())}
        source.validate_live()?;
        match self.d.phase {
            PrimaryPhase::AwaitingDLayout|PrimaryPhase::AwaitingVoting|PrimaryPhase::AwaitingDRebind=>(),
            _=>return Err(self.d.failure.clone().unwrap_or_else(||"Primary D reached partial phase cannot replay".into())),
        }
        if p.primary_phase!=Some(self.d.phase){return Err("D same primitive/typed phase mismatch".into())}
        //Preclaim live validation on THIS Install. No rows are copied/retained.
        match services.call(SessionCall::PrimaryCategory{source:source.clone(),row:None}).map_err(|e|e.to_string())? {
            SessionReply::PrimaryCategory(None)=>(),_=>return Err("D category validation reply differs".into()),
        }
        if matches!(self.d.phase,PrimaryPhase::AwaitingVoting|PrimaryPhase::AwaitingDRebind){
            let constructed_epoch=self.d.epoch.ok_or("completed D construction epoch absent")?;
            p.primary_park_d()?;
            self.d.phase=PrimaryPhase::AwaitingDRebind;self.d.active_epoch=None;
            self.d.parked_for=Some(generation);self.d.live_source=None;
            //The healthy CameraOnly operation may now continue independently.
            //No D constructor, layout, register, font or raw cell is replayed.
            return Ok(InstallDisposition::ParkedAwaitingDRebind{constructed_epoch,requested_epoch:generation});
        }
        if p.panels.get(&D).is_none_or(|panel|panel.phase!=Phase::AwaitingLayout||panel.descriptor!=Some(DescriptorIdentity::EmbeddedD))||p.cells().byte(D+0x184)?!=0{
            return Err("D continuation requires SAME AwaitingDLayout before actual slot04".into())
        }
        p.primary_claim_d()?;self.d.phase=PrimaryPhase::LayingOutD;self.d.epoch=Some(generation);
        let result=self.layout(source,services,p,text).and_then(|()|{
            p.steps.push(super::primitive_factory::Step::LayoutReturned(D));p.primary_finish_d()
        });
        match result {
            Ok(())=>{self.d.phase=PrimaryPhase::AwaitingVoting;self.d.active_epoch=Some(generation);self.d.live_source=Some(source.clone());Ok(InstallDisposition::CompletedD{epoch:generation})},
            Err(error)=>{p.primary_fail_d();self.fail(error.clone());Err(error)},
        }
    }
    fn layout(&mut self,source:&ResidentSource,services:&mut SessionServices<'_>,p:&mut Primitives,text:&mut UiTextResources)->Result<(),String>{
        p.steps.push(super::primitive_factory::Step::Layout(D));
        //SAME embedded descriptor, not archiveModel or native NULL.
        p.primary_reset_d_descriptor()?;
        {let c=p.primary_cells()?;
            for (o,v) in [(0x13c,0),(0x140,0),(0x144,1f32.to_bits()),(0x148,1f32.to_bits()),(0x4a4,0)]{c.set_word(D+o,v)?;}}
        for category in 0..52usize {
            let words=match services.call(SessionCall::PrimaryCategory{source:source.clone(),row:Some(category as u8)}).map_err(|e|e.to_string())?{
                SessionReply::PrimaryCategory(Some(words))=>words,_=>return Err("D actual category reply absent".into()),
            };
            let title=words[2] as i32;
            if !((words[0]==1&&words[1]&1==0)||category==41)||title>45{continue}
            //Real43e9a0 child allocation/slot04 RET/number/append happens BEFORE
            //the signed title lookup, so an unsupported negative retains it.
            let slot=self.d.rows.len();let id=TextId{owner:self.d.owner,slot};
            let mut child=Text::normal_fields();child.bounds=[0.,0.,200.,40.];
            self.d.rows.push(Box::new(LegendText{id,parent:D,root:D,number:slot as u32,text:child,category:None,title:None,title_record:None,initialized:false}));
            //42a413 reloads the SAME source title after the child birth.
            let title=live_title(source,services,category)?;
            if title<0{return Err(format!("42a41b reached signed before-table title{title}; actual mapping unavailable"))}
            let at=title_address(title)?;
            let identifier=p.cells().word(at)?.wrapping_add(1);
            let row=self.d.rows.last_mut().unwrap();
            row.text.identifier(text.fonts_mut(),identifier)?; //441220 before440ad0/440b70
            row.text.resize()?; //440ad0 ensures actual capacity
            let count=row.text.group.count as usize;
            row.text.colors[..count].fill(0); //440ad0 first row array only
            row.text.set_font(text.fonts_mut(),0)?; //always select, even existing0
            //42a46d/42a479 are two actual reloads, not historical cached input.
            let stored_title=live_title(source,services,category)?;
            p.primary_cells()?.set_word(D+0x664+slot as u32*20,stored_title as u32)?;
            row.category=Some(category);row.title=Some(stored_title);
            let table_title=live_title(source,services,category)?;
            row.title_record=Some(title_address(table_title)?);row.initialized=true;
            //Pointer+660/+670 are typed row/text/table aliases above, never fake host words.
            p.primary_cells()?.set_word(D+0x4a4,(slot+1) as u32)?;
        }
        let c=p.primary_cells()?;
        for (at,value) in GEOMETRY {c.set_word(at,value)?;}
        c.set_word(D+0xdf4,91)
    }
}
fn live_title(source:&ResidentSource,services:&mut SessionServices<'_>,category:usize)->Result<i32,String>{
    match services.call(SessionCall::PrimaryCategory{source:source.clone(),row:Some(category as u8)}).map_err(|e|e.to_string())?{
        SessionReply::PrimaryCategory(Some(words))=>Ok(words[2] as i32),_=>Err("D actual title reload reply absent".into()),
    }
}
fn title_address(title:i32)->Result<u32,String>{
    //The original reload has no second <=45 guard. Resolve only actual
    //mapped PE bytes at its reached consumer; never clamp or fabricate a row.
    if !(0..=45).contains(&title){return Err(format!("D reached unsupported title-table alias {title}; no clamping or substitute"))}
    let address=i64::from(0x5385a0)+i64::from(title)*88;
    u32::try_from(address).map_err(|_|"D title-table address leaves finite mapped domain".into())
}
const GEOMETRY:[(u32,u32);30]=[
    (0x560a48,0),(0x560a4c,0),(0x560a70,0xc1000000),(0x560a74,0),(0x560a78,0x41000000),(0x560a7c,0),
    (0x560a98,0xc1500000),(0x560a9c,0),(0x560aa0,0x41500000),(0x560aa4,0),(0x560aa8,0),(0x560aac,0),
    (0x560ac0,0xc1000000),(0x560ac4,0xc1000000),(0x560ac8,0x41000000),(0x560acc,0xc1000000),
    (0x560ad0,0x41000000),(0x560ad4,0x41000000),(0x560ad8,0xc1000000),(0x560adc,0x41000000),
    (0x560ae8,0xc1000000),(0x560aec,0xc1000000),(0x560af0,0x41000000),(0x560af4,0xc1000000),
    (0x560af8,0xc1500000),(0x560afc,0x41000000),(0x560b00,0x41500000),(0x560b04,0x41000000),
    (0x560b08,0),(0x560b0c,0x41000000),
];
#[cfg(test)]
#[path="primary_d_construction_tests.rs"] mod tests;
