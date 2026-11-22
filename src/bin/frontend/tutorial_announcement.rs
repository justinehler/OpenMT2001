//! Synchronous native48dc40 delivery within auction and business completion.
use openmt2001::{building_pool::GeometryBinding, level_script, level_startup, live_commands};
use std::cell::RefCell;

pub(super) struct Bridge<'a, 'hooks, 'context, 'image> {
    pub scripts: &'a mut level_startup::Scripts,
    pub hooks: &'a RefCell<&'hooks mut super::Hooks>,
    pub context: &'a mut level_script::Context<'context, 'image>,
}
pub(super) struct PartsBridge<'a,'hooks,'context,'image>{
    pub runtime:&'a mut openmt2001::lua4::Runtime,
    pub bindings:&'a mut level_script::Bindings,
    pub events:&'a mut openmt2001::script_events::Events,
    pub hooks:&'a RefCell<&'hooks mut super::Hooks>,
    pub context:&'a mut level_script::Context<'context,'image>,
}

struct Geometry<'a>(&'a mut dyn GeometryBinding);
impl GeometryBinding for Geometry<'_> {
    fn prepare(&mut self, slot: usize, record: &mut [u8; 0x394]) -> Result<(), String> {
        self.0.prepare(slot, record)
    }
    fn model_height(&mut self, handle: u32) -> Result<f32, String> {
        self.0.model_height(handle)
    }
}

/// Hooks retains every original CreatePrompt/CreateMessage/DismissPrompt call.
/// Explicit user dismissal executes CreatePrompt(next), so the replacement is
/// recorded here as well. Match the Host's message.or(prompt) precedence.
fn active_dismissal(hooks: &super::Hooks) -> Result<Option<i32>, String> {
    let mut prompt = None;
    let mut message = None;
    for (command, args) in &hooks.commands {
        match command {
            level_script::Command::Prompt => {
                prompt = Some(crate::live_host::prompt_request(args)?.dismissal_id as i32)
            }
            level_script::Command::TutorialMessage => {
                message = Some(crate::live_host::prompt_request(args)?.dismissal_id as i32);
                prompt = None;
            }
            level_script::Command::TutorialDismissPrompt => {
                prompt = None;
                message = None;
            }
            _ => {}
        }
    }
    Ok(message.or(prompt))
}

impl Bridge<'_, '_, '_, '_> {
    fn dispatch_control(
        &mut self,
        control: u32,
        state: &mut level_script::State<'_>,
        clock: &openmt2001::clock::Clock,
        geometry: &mut dyn GeometryBinding,
        utilities: &mut openmt2001::economy::Utilities,
    ) -> Result<(), String> {
        PartsBridge{runtime:&mut self.scripts.runtime,bindings:&mut self.scripts.bindings,
            events:&mut self.scripts.events,hooks:self.hooks,context:self.context}
            .dispatch_control(control,state,clock,geometry,utilities)
    }
}
impl PartsBridge<'_,'_,'_,'_>{
    fn dispatch_control(&mut self,control:u32,state:&mut level_script::State<'_>,
        clock:&openmt2001::clock::Clock,geometry:&mut dyn GeometryBinding,
        utilities:&mut openmt2001::economy::Utilities)->Result<(),String>{
        let mut hooks = self.hooks.borrow_mut();
        let active = active_dismissal(&hooks)?;
        let first = hooks.commands.len();
        self.context.update_clock(clock);
        let mut geometry = Geometry(geometry);
        std::mem::swap(utilities, &mut hooks.utilities);
        let result = (|| {
            level_startup::Scripts::dispatch_catch_event_parts(self.runtime,self.bindings,self.events,
                ("TutorialEvent", control as i32),
                state,
                self.context,
                &mut geometry,
                &mut **hooks,
            )?;
            if hooks.commands[first..]
                .iter()
                .any(|(command, _)| matches!(command, level_script::Command::TutorialDismissPrompt))
            {
                // Original Interface.DismissPrompt binding4db6b0 calls43bed0.
                // Its +345 active-byte check branches directly to the tail
                // when no prompt is shown. Tutorial12 does this after prompt4
                // closes, so no InterfacePromptDismiss callback is emitted.
                if let Some(dismissal) = active {
                    level_startup::Scripts::dispatch_catch_event_parts(self.runtime,self.bindings,self.events,
                        ("InterfacePromptDismiss", dismissal),
                        state,
                        self.context,
                        &mut geometry,
                        &mut **hooks,
                    )?;
                }
            }
            Ok(())
        })();
        std::mem::swap(utilities, &mut hooks.utilities);
        result
    }
}

impl live_commands::TutorialAnnouncement for PartsBridge<'_,'_,'_,'_>{
    fn start(&mut self,state:&mut level_script::State<'_>,clock:&openmt2001::clock::Clock,
        _presentation:&openmt2001::ai_messages::offers::auction::admission::Presentation,
        geometry:&mut dyn GeometryBinding,utilities:&mut openmt2001::economy::Utilities)->Result<(),String>{
        self.dispatch_control(61,state,clock,geometry,utilities)
    }
    fn control(&mut self,control:u32,state:&mut level_script::State<'_>,clock:&openmt2001::clock::Clock,
        geometry:&mut dyn GeometryBinding,utilities:&mut openmt2001::economy::Utilities)->Result<(),String>{
        self.dispatch_control(control,state,clock,geometry,utilities)
    }
}

impl live_commands::TutorialAnnouncement for Bridge<'_, '_, '_, '_> {
    fn start(
        &mut self,
        state: &mut level_script::State<'_>,
        clock: &openmt2001::clock::Clock,
        _presentation: &openmt2001::ai_messages::offers::auction::admission::Presentation,
        geometry: &mut dyn GeometryBinding,
        utilities: &mut openmt2001::economy::Utilities,
    ) -> Result<(), String> {
        self.dispatch_control(61, state, clock, geometry, utilities)
    }
    fn control(
        &mut self,
        control: u32,
        state: &mut level_script::State<'_>,
        clock: &openmt2001::clock::Clock,
        geometry: &mut dyn GeometryBinding,
        utilities: &mut openmt2001::economy::Utilities,
    ) -> Result<(), String> {
        self.dispatch_control(control, state, clock, geometry, utilities)
    }
}
