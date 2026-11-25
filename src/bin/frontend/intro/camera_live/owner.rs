//! Event-thread camera authority. No timer, wall clock, worker or render cache
//! owns another evolving numeric record. Installation/refresh move this value.
use openmt2001::camera::{bank::{Bank,Capture,RawRecord,SelectedActorInput,StrategicTransition},
    primary::{Primary,Mode12Input,Mode13Input,BlockInitialization,CursorRequest}};
use openmt2001::x87::portable_trig::PortableTrig;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Pose {pub eye:[u32;3],pub target:[u32;3],pub up:[u32;3]}
pub struct Input {pub mode12:Mode12Input,pub mode13:Mode13Input}
pub struct Owner {bank:Bank,kind_dirty:u8,
    ui_selection:Option<super::ui_selection::SelectionState>,
    resident_descriptors:Option<super::resident_descriptors::Registry>,
    property_descriptors:Option<super::property_descriptors::Pool>,
    property_epoch:Option<crate::live_host::camera_bridge::PropertyEpoch>,
    last_property_epoch:Option<crate::live_host::camera_bridge::PropertyEpoch>,
    property_failure:Option<String>}

/// A field-readable selection borrowed from the owning allocation validator.
/// Private construction and the Owner borrow prohibit pool retirement while a
/// consumer holds this facade. source() copies the actual identity unchanged.
/// Resident fields require a live Host service loan and cannot enter this
/// UI-only field-readable representation.
#[derive(Clone,Copy)]
pub(super) struct ResolvedSelected<'a> {
    value:super::ui_selection::Selected,
    _owner:&'a Owner,
}
impl ResolvedSelected<'_> {
    pub fn kind(self)->i16{self.value.kind}
    pub fn category(self)->u32{self.value.category}
    pub fn source(self)->super::ui_selection::Selected{self.value}
}

fn pose(raw:&RawRecord,eye:usize,target:usize)->Pose {
    let triple=|at|std::array::from_fn(|j|u32::from_le_bytes(raw[at+j*4..at+j*4+4].try_into().unwrap()));
    Pose{eye:triple(eye),target:triple(target),up:triple(0x64)}
}

impl Owner {
    /// The separate process kind-dirty byte is supplied by the caller. It is
    /// not restored from disk or inferred from saved mode/interface flags.
    pub fn new(bank:Bank,kind_dirty:u8)->Self {Self{bank,kind_dirty,ui_selection:None,resident_descriptors:None,property_descriptors:None,property_epoch:None,last_property_epoch:None,property_failure:None}}
    pub fn install_ui_selection(&mut self,value:super::ui_selection::SelectionState)->Result<(),String>{
        if self.ui_selection.is_some(){return Err("Live selection already installed".into());}
        self.ui_selection=Some(value);Ok(())
    }
    pub fn ui_selection(&self)->Result<&super::ui_selection::SelectionState,String>{
        self.ui_selection.as_ref().ok_or_else(||"Live selection has no actual install producer".into())
    }
    pub fn ui_selection_mut(&mut self)->Result<&mut super::ui_selection::SelectionState,String>{
        self.ui_selection.as_mut().ok_or_else(||"Live selection has no actual install producer".into())
    }
    /// Property facades must resolve through the live owned allocation at
    /// each reached read; copied kind/category fields never bypass lifetime.
    pub fn validate_ui_selected(&self,value:Option<super::ui_selection::Selected>)
        ->Result<Option<super::ui_selection::Selected>,String>{
        if let Some(selected)=value{
            if matches!(selected.key,super::ui_selection::SelectionKey::ResidentDescriptor(_)){
                self.resident_descriptors()?.validate_selected(selected)?;
            }
            if matches!(selected.key,super::ui_selection::SelectionKey::PropertyDescriptor(_)){
                self.property_descriptors()?.resolve_selected(selected)?;
            }
        }
        Ok(value)
    }
    pub(super) fn resolve_ui_selected(&self,value:Option<super::ui_selection::Selected>)
        ->Result<Option<ResolvedSelected<'_>>,String>{
        let value=self.validate_ui_selected(value)?;
        if value.is_some_and(|selected|matches!(selected.key,super::ui_selection::SelectionKey::ResidentDescriptor(_))) {
            return Err("resident field consumer requires same-operation Host descriptor reads; UI-only actor/main/follow join unavailable".into());
        }
        Ok(value.map(|value|ResolvedSelected{value,_owner:self}))
    }
    pub fn current_ui_selected(&self)->Result<Option<super::ui_selection::Selected>,String>{
        self.validate_ui_selected(self.ui_selection()?.current)
    }
    pub fn published_ui_selected(&self)->Result<Option<super::ui_selection::Selected>,String>{
        self.validate_ui_selected(self.ui_selection()?.published)
    }
    pub fn transient_ui_selected(&self)->Result<Option<super::ui_selection::Selected>,String>{
        self.validate_ui_selected(self.ui_selection()?.transient)
    }
    pub fn resident_descriptors(&self)->Result<&super::resident_descriptors::Registry,String>{
        self.resident_descriptors.as_ref().ok_or_else(||"actual resident registry not installed".into())
    }
    pub fn resident_descriptors_mut(&mut self)->Result<&mut super::resident_descriptors::Registry,String>{
        self.resident_descriptors.as_mut().ok_or_else(||"actual resident registry not installed".into())
    }
    pub fn validate_resident_install(&self,source:&crate::live_host::camera_bridge::ResidentSource,generation:u64)->Result<(),String>{
        match &self.resident_descriptors {
            Some(registry)=>registry.validate_install(source,generation),
            None=>{
                source.validate_live()?;
                if source.epoch().serial()!=generation{return Err("resident source/channel World epoch mismatch".into());}
                Ok(())
            }
        }
    }
    pub fn install_resident_source(&mut self,source:&crate::live_host::camera_bridge::ResidentSource,generation:u64)->Result<(),String>{
        self.validate_resident_install(source,generation)?;
        if self.resident_descriptors.is_none(){self.resident_descriptors=Some(super::resident_descriptors::Registry::new()?);}
        self.resident_descriptors_mut()?.install(source,generation)
    }
    pub fn retire_resident_source(&mut self){
        if let Some(registry)=&mut self.resident_descriptors{registry.retire();}
    }
    /// Move the ONE actual pool into this SAME process owner once. This does
    /// not construct a pool from defaults or copy Content.selected/Scene.
    pub fn install_property_descriptors(&mut self,pool:super::property_descriptors::Pool)->Result<(),String>{
        if self.property_descriptors.is_some(){return Err("property pool already owned; use actual allocation rebuild".into());}
        self.property_descriptors=Some(pool);Ok(())
    }
    #[cfg(test)]
    pub(crate) fn test_retained_pool_identity(&self)->Option<(u64,u64)>{
        self.property_descriptors.as_ref().map(|pool|pool.test_identity())
    }
    #[cfg(test)]
    pub(crate) fn test_retained_pool_is_retired(&self)->bool{
        self.property_descriptors.as_ref().is_some_and(|pool|pool.test_is_retired())
    }
    pub fn property_lifecycle_error(&self)->Option<&str>{self.property_failure.as_deref()}
    /// Read-only preflight before raw camera/UI writes. Stale frames never
    /// become a retry/rebuild event, and a live World cannot be replaced here.
    pub fn validate_property_install(&self,source:&crate::live_host::camera_bridge::PropertySource,
        generation:u64,model:&super::property_descriptors::PavementModel)->Result<(),String>{
        if let Some(error)=&self.property_failure{return Err(error.clone());}
        if source.epoch().serial()!=generation{return Err("property source/channel World epoch mismatch".into());}
        if self.property_epoch.is_some(){return Err("property source already active; scene refresh must move Endpoint".into());}
        if self.last_property_epoch.is_some_and(|old|!source.epoch().follows(old)){
            return Err("old or duplicate World property source epoch".into());
        }
        model.validate_source(source)?;
        if source.count()!=52{return Err("completed v1.4 World property source count is not52".into());}
        Ok(())
    }
    /// The sole completed-Session install writer. Reuses the same retired Pool;
    /// an allocation/binding failure is fatal and remains latched in this Owner.
    pub fn install_property_source(&mut self,source:&crate::live_host::camera_bridge::PropertySource,
        generation:u64,model:&super::property_descriptors::PavementModel,
        reset:&super::property_descriptors::ResetTransform)->Result<(),String>{
        let result:Result<(),String>=(||{
            self.validate_property_install(source,generation,model)?;
            // Consume the epoch even if allocation fails: no old-frame retry.
            self.last_property_epoch=Some(source.epoch());
            match &mut self.property_descriptors {
                Some(pool)=>pool.rebuild_from_install(source,model,reset)?,
                None=>self.property_descriptors=Some(super::property_descriptors::Pool::build_from_install(source,model,reset)?),
            }
            self.property_epoch=Some(source.epoch());
            Ok(())
        })();
        if let Err(error)=&result{self.property_failure=Some(error.clone());}
        result
    }
    /// Always clears authority before inbox retirement can wake the source
    /// worker. Errors are retained even though cancellation must still proceed.
    pub fn latch_property_failure(&mut self,error:String){
        if self.property_failure.is_none(){self.property_failure=Some(error);}
    }
    /// Opt-in boundary metadata; never enters callbacks or game clocks.
    pub(crate) fn trace_property_lifecycle(&self,phase:&str){
        if std::env::var_os("OPENMT2001_CAMERA_OWNER_NATIVE_REVIEW").is_none(){return;}
        eprintln!("[property-lifecycle] {phase} active={:?} high_water={:?} pool={:?} failure={:?}",
            self.property_epoch,self.last_property_epoch,
            self.property_descriptors.as_ref().map(|p|p.lifecycle_observation()),self.property_failure);
    }
    pub fn retire_property_source(&mut self){
        self.property_epoch=None;
        if let Some(pool)=&mut self.property_descriptors{
            if let Err(error)=pool.retire_source_frame(){
                if self.property_failure.is_none(){self.property_failure=Some(error);}
            }
        }
    }
    pub fn property_descriptors(&self)->Result<&super::property_descriptors::Pool,String>{
        if let Some(error)=&self.property_failure{return Err(error.clone());}
        self.property_descriptors.as_ref().ok_or_else(||"actual property pool not installed".into())
    }
    pub fn property_descriptors_mut(&mut self)->Result<&mut super::property_descriptors::Pool,String>{
        if let Some(error)=&self.property_failure{return Err(error.clone());}
        self.property_descriptors.as_mut().ok_or_else(||"actual property pool not installed".into())
    }
    pub fn property_lookup_current(&mut self,index:i32)->Result<(),String>{
        if let Some(error)=&self.property_failure{return Err(error.clone());}
        let pool=self.property_descriptors.as_ref().ok_or("actual property pool not installed")?;
        let selection=self.ui_selection.as_mut().ok_or("actual selection not installed")?;
        pool.lookup_publish_current(selection,index)
    }
    pub fn property_lookup_transient(&mut self,index:i32)->Result<(),String>{
        if let Some(error)=&self.property_failure{return Err(error.clone());}
        let pool=self.property_descriptors.as_ref().ok_or("actual property pool not installed")?;
        let selection=self.ui_selection.as_mut().ok_or("actual selection not installed")?;
        pool.lookup_publish_transient(selection,index)
    }
    pub fn property_current_camera(&self,input:&crate::navigator::upper::Inputs,
        model:&super::property_descriptors::PavementModel,assets:&crate::selection::Assets,
        height:u32,radius:u32)->Result<BlockInitialization,String>{
        self.property_descriptors()?.current_camera_from_protected(self.ui_selection()?,input,model,assets,height,radius)
    }
    ///43d108 direct logic eye93af64/6c and yaw93afdc (primary base93af60).
    ///Read after actual43d290 focus/material consumers; never infer yaw.
    pub fn upper_logic_words(&self)->([u32;2],u32){
        let raw=self.bank.primary();
        let word=|at|u32::from_le_bytes(raw[at..at+4].try_into().unwrap());
        ([word(4),word(12)],word(0x7c))
    }
    /// Central4ba8f7..902 source target words, primary+34/+38/+3c.
    /// This is a live raw read, distinct from current eye or sampled pose.
    pub(super) fn continuation_target_word(&self,axis:usize)->Result<u32,String>{
        if axis>=3{return Err("central target axis outside source domain".into())}
        let at=0x34+axis*4;
        Ok(u32::from_le_bytes(self.bank.primary()[at..at+4].try_into().unwrap()))
    }
    pub fn capture(&self)->Capture {self.bank.capture()}
    pub fn restore(&mut self,capture:Capture){self.bank.restore(capture);}
    pub fn kind(&self)->u32 {self.bank.primary_kind()}
    pub fn kind_dirty(&self)->u8 {self.kind_dirty}
    pub fn current(&self)->Pose {pose(self.bank.primary(),4,0x10)}
    pub fn sampled(&self)->Pose {pose(self.bank.primary(),0x4c,0x58)}
    pub fn central(&mut self,input:Input)->Result<Vec<CursorRequest>,String> {
        self.bank.try_update_primary(|raw| {
            let mut state=Primary::decode(*raw)?;let mut trig=PortableTrig;
            let effects=match state.view().mode {
                4|6=>Vec::new(),
                12=>state.mode12(input.mode12,&mut trig)?,
                13=>state.mode13(input.mode13,&mut trig)?,
                _=>return Err("unimplemented raw camera callback identity"),
            };
            state.common_with_trig(&mut trig)?;*raw=*state.raw();Ok(effects)
        }).map_err(str::to_owned)
    }
    /// Call only with the actual exported central-task ratio. Sampling never
    /// dispatches a callback, common step, input or save-only recomputation.
    pub fn sample(&mut self,amount:f32)->Result<Pose,String> {
        self.bank.try_update_primary(|raw| {
            let mut state=Primary::decode(*raw)?;state.sample(amount)?;*raw=*state.raw();Ok::<_,&'static str>(())
        }).map_err(str::to_owned)?;Ok(self.sampled())
    }
    pub fn set_kind(&mut self,kind:u32){self.bank.set_kind(kind,&mut self.kind_dirty);}
    pub fn reset_yaw(&mut self){self.bank.reset_yaw();}
    pub fn world_reset_final_fields(&mut self){self.bank.set_world_reset_final_fields(&mut self.kind_dirty);}
    pub fn target(&mut self,eye:[u32;3]){self.bank.set_target_words(eye);}
    pub fn immediate(&mut self,eye:[u32;3])->Result<(),String>{self.bank.set_immediate_words(eye).map_err(str::to_owned)}
    pub fn initialize_block(&mut self,input:BlockInitialization)->Result<(),String>{
        self.bank.try_update_primary(|raw| {
            *raw=Primary::initialize_block_words(*raw,input,&mut PortableTrig)?;Ok::<_,&'static str>(())
        }).map_err(str::to_owned)
    }
    pub fn initialize_actor(&mut self,input:SelectedActorInput)->Result<(),String>{
        self.bank.initialize_selected_actor(input,&mut PortableTrig).map_err(str::to_owned)
    }
    pub fn strategic_after_ui_clear(&mut self)->StrategicTransition {
        self.bank.strategic_after_ui_clear(&mut self.kind_dirty)
    }
    /// Actual UI integration splits these recovered operations so mode3's
    /// UI prelude/tail stay on their original sides of the camera setters.
    pub fn copy_primary_to_secondary(&mut self){let raw=*self.bank.primary();self.bank.restore_secondary(raw);}
    pub fn copy_secondary_to_primary(&mut self){let raw=*self.bank.secondary();self.bank.restore_primary(raw);}
    pub fn enter_strategic_fields(&mut self)->Result<(),String>{
        self.immediate([-220f32,1050.,-30.].map(f32::to_bits))?;self.set_kind(4);Ok(())
    }
}

#[cfg(test)]
#[path="owner/tests.rs"]
mod tests;
