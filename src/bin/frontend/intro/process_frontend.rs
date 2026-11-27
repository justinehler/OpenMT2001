//! One UI process owner. Winit foreground admission is an event backend, not
//! COM Acquire/HRESULT, buffered polling, scan code or native pointer equivalence.
use super::focus::{self, AttachmentId, Effects, Prefix};
use crate::{audio_bus::focus as audio, process_dispatch::DispatchCustody};
use openmt2001::native_input::{self, focus as native};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};
use winit::{
    event::{ElementState, MouseButton, WindowEvent},
    keyboard::PhysicalKey,
    window::WindowId,
};

pub(crate) type Owner = Rc<RefCell<ProcessFrontend>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Menu,
    Loading,
    World,
    Draining,
    Failed,
    Stopped,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum PhysicalSample {
    Key {
        key: PhysicalKey,
        down: bool,
        repeat: bool,
    },
    /// Winit focus reconciliation updates held state without a physical press.
    SyntheticKey {
        key: PhysicalKey,
        down: bool,
    },
    Button {
        button: MouseButton,
        down: bool,
    },
    Pointer([f64; 2]),
    Unfocus,
}
impl PhysicalSample {
    pub(crate) fn from_event(event: &WindowEvent) -> Option<Self> {
        Some(match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic: true,
                ..
            } => Self::SyntheticKey {
                key: event.physical_key,
                down: event.state == ElementState::Pressed,
            },
            WindowEvent::KeyboardInput { event, .. } => Self::Key {
                key: event.physical_key,
                down: event.state == ElementState::Pressed,
                repeat: event.repeat,
            },
            WindowEvent::MouseInput { button, state, .. } => Self::Button {
                button: *button,
                down: *state == ElementState::Pressed,
            },
            WindowEvent::CursorMoved { position, .. } => Self::Pointer([position.x, position.y]),
            WindowEvent::Focused(false) => Self::Unfocus,
            _ => return None,
        })
    }
}
struct Receiver {
    identity: u64,
    window: WindowId,
    admitted: [bool; 2],
}
struct Activation {
    disable_sound: u8,
    disable_music: u8,
    sound: u8,
    born: bool,
    active: u8,
    music: u8,
}
impl Activation {
    fn birth(&mut self) -> Result<(), String> {
        if self.born {
            return Err("Process frontend born twice".into());
        }
        self.born = true;
        self.active = u8::from(self.disable_sound == 0);
        self.music = u8::from(self.disable_music == 0);
        Ok(())
    }
    fn guards(&self) -> audio::Guards {
        audio::Guards {
            active: self.born && self.active != 0,
            sound_enabled: self.sound != 0,
            music_enabled: self.music != 0,
        }
    }
}
struct Deferred {
    fence: u64,
    sample: PhysicalSample,
}
pub(crate) struct ProcessFrontend {
    receiver: Option<Rc<RefCell<Receiver>>>,
    pub(crate) input: native_input::StagedInput,
    wrapper: native::Frontend,
    activation: Activation,
    pub(crate) audio: audio::SharedAudio,
    frontend: audio::Frontend,
    pub(crate) phase: Phase,
    custody: Option<DispatchCustody>,
    process: crate::live_host::process_runtime::Handle,
    generation: u64,
    attachment: Option<u64>,
    observed: Option<bool>,
    observation: u64,
    applied: u64,
    drain_cut: u64,
    physical: VecDeque<Deferred>,
    ready: VecDeque<PhysicalSample>,
    observations: VecDeque<u64>,
    last_sent: u64,
    pub(crate) cancelled_samples: usize,
    failure: Option<String>,
    #[cfg(test)]
    after_mouse: Option<Box<dyn FnOnce(&mut Self)>>,
    menu_owner: audio::MusicOwner,
    menu_token: Option<audio::MusicToken>,
    menu_music: Option<audio::Music>,
}
impl ProcessFrontend {
    pub(crate) fn new(
        receipt: crate::live_host::process_runtime::FrontendStartup,
        process: crate::live_host::process_runtime::Handle,
        window: WindowId,
        levels: crate::settings_menu::Levels,
    ) -> Result<Owner, String> {
        let audio = audio::SharedAudio::new(levels.music, levels.effects);
        let mut activation = Activation {
            disable_sound: receipt.disable_sound,
            disable_music: receipt.disable_music,
            sound: receipt.sound,
            born: false,
            active: 0,
            music: 1,
        };
        activation.birth()?; // Only called after the actual successful Intro constructor.
        let mut frontend = audio::Frontend::new(audio.clone(), activation.guards());
        frontend.activate(1)?;
        Ok(Rc::new(RefCell::new(Self {
            receiver: Some(Rc::new(RefCell::new(Receiver {
                identity: 1,
                window,
                admitted: [false; 2],
            }))),
            input: native_input::StagedInput::from_process_cells(
                receipt.input,
                native_input::CharacterQueue::initialize([0; 16]),
            ),
            wrapper: native::Frontend::from_process_byte(receipt.suspended),
            activation,
            audio,
            frontend,
            phase: Phase::Menu,
            custody: Some(DispatchCustody::startup(receipt.pause)?),
            process,
            generation: 1,
            attachment: None,
            observed: None,
            observation: 0,
            applied: 0,
            drain_cut: 0,
            observations: VecDeque::new(),
            last_sent: 0,
            physical: VecDeque::new(),
            ready: VecDeque::new(),
            cancelled_samples: 0,
            failure: None,
            #[cfg(test)]
            after_mouse: None,
            menu_owner: audio::MusicOwner::new(),
            menu_token: None,
            menu_music: None,
        })))
    }
    fn live(&self) -> Result<(), String> {
        self.failure.clone().map_or(Ok(()), Err)
    }
    pub(crate) fn fail(&mut self, error: String) {
        self.cancelled_samples += self.physical.len() + self.ready.len();
        self.physical.clear();
        self.ready.clear();
        self.phase = Phase::Failed;
        self.failure = Some(error);
    }
    /// Every device call ends its receiver borrow; then lookup the LIVE receiver.
    fn prefix(&mut self, focused: bool, reason3: i8) -> Result<Prefix, String> {
        self.live()?;
        let Some(mouse) = self.receiver.clone() else {
            return Ok(Prefix::ReceiverAbsent);
        };
        let identity = mouse.borrow().identity;
        mouse.borrow_mut().admitted[0] = focused;
        #[cfg(test)]
        if let Some(callback) = self.after_mouse.take() {
            callback(self);
        }
        let keyboard = self
            .receiver
            .clone()
            .ok_or("Receiver disappeared after mouse0")?;
        if keyboard.borrow().identity != identity {
            return Err("Receiver changed after completed mouse0".into());
        }
        keyboard.borrow_mut().admitted[1] = focused;
        if focused {
            self.input.focus_gain_cells_after_device_calls();
        } else {
            self.input.focus_loss_cells_after_device_calls();
        }
        self.audio_pause(focused, reason3)
    }
    /// 43e5f0 reaches the same 4ad610/4ad640 audio wrapper as focus,
    /// without acquiring devices or changing input admission.
    fn audio_pause(&mut self, focused: bool, reason3: i8) -> Result<Prefix, String> {
        self.frontend.set_guards(self.activation.guards());
        let generation = self.generation;
        let frontend = &mut self.frontend;
        let outcome = self
            .wrapper
            .apply(focused, self.activation.born, reason3, |action| {
                let action = match action {
                    native::Action::Suspend => audio::Action::Suspend,
                    native::Action::Resume => audio::Action::Resume,
                };
                frontend.perform(generation, action).map(|o| match o {
                    audio::Outcome::Applied => native::AudioOutcome::Applied,
                    audio::Outcome::GuardedInactive => native::AudioOutcome::GuardedInactive,
                })
            })?;
        Ok(Prefix::Complete(outcome))
    }
    fn menu_focus(&mut self, focused: bool) -> Result<(), String> {
        let reason3 = self
            .custody
            .as_ref()
            .ok_or("Menu has no dispatch custody")?
            .pause
            .bytes[3] as i8;
        if self.prefix(focused, reason3)? != Prefix::ReceiverAbsent {
            // Authoritative network value read AFTER the actual input/audio prefix.
            let blocked = self
                .process
                .byte(crate::live_host::process_runtime::Byte::Network93bbb9)
                .map_err(|e| e.to_string())?
                != 0;
            let custody = self
                .custody
                .as_mut()
                .ok_or("Menu custody lost during prefix")?;
            let mut change = |change: openmt2001::pause::Timers,
                              _: &mut openmt2001::pause::State| {
                change.apply(&mut custody.tasks);
                Ok(())
            };
            if focused {
                custody.pause.focus_gained(blocked, &mut change)?;
            } else {
                custody.pause.focus_lost(blocked, &mut change)?;
            }
        }
        self.applied = self.observation;
        Ok(())
    }
    pub(crate) fn note_sent(&mut self, sent: u64) {
        if sent > self.last_sent {
            self.observations.push_back(self.observation);
            self.last_sent = sent;
        }
    }
    pub(crate) fn observe(&mut self, focused: bool) -> Result<(), String> {
        self.live()?;
        self.observation = self
            .observation
            .checked_add(1)
            .ok_or("OS observation sequence exhausted")?;
        self.observed = Some(focused);
        if self.phase == Phase::Menu {
            if let Err(e) = self.menu_focus(focused) {
                self.fail(e.clone());
                return Err(e);
            }
        }
        Ok(())
    }
    pub(crate) fn take_custody(&mut self, fresh: bool) -> Result<DispatchCustody, String> {
        self.live()?;
        if self.phase != Phase::Menu || !self.physical.is_empty() {
            return Err("Process start cut is not ready".into());
        }
        // Reject before destructive worker reset while custody is still available.
        if fresh
            && (self
                .custody
                .as_ref()
                .ok_or("Dispatch custody absent")?
                .pause
                .bytes[0] as i8)
                < 0
        {
            return Err("Unsupported negative reason0 before fresh/reset admission".into());
        }
        self.frontend.retire(self.generation)?;
        self.phase = Phase::Loading;
        self.drain_cut = self.applied;
        self.cancel_physical();
        self.custody
            .take()
            .ok_or_else(|| "Dispatch custody already moved".into())
    }
    fn cancel_physical(&mut self) {
        self.cancelled_samples += self.physical.len() + self.ready.len();
        self.physical.clear();
        self.ready.clear();
    }
    pub(crate) fn draining(&mut self) {
        if self.phase == Phase::World || self.phase == Phase::Loading {
            self.phase = Phase::Draining;
            self.drain_cut = self.applied;
            self.observations.clear();
            self.cancel_physical();
        }
    }
    pub(crate) fn return_custody(&mut self, custody: DispatchCustody) -> Result<(), String> {
        self.live()?;
        if self.custody.is_some() {
            return Err("Duplicate dispatch custody return".into());
        }
        if !matches!(self.phase, Phase::Draining | Phase::Loading) {
            return Err("Custody returned before retirement".into());
        }
        self.custody = Some(custody);
        self.phase = Phase::Menu;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("Audio generation exhausted")?;
        self.frontend.activate(self.generation)?;
        if self.observation != self.drain_cut {
            self.menu_focus(self.observed.ok_or("No OS observation on return")?)?;
        }
        Ok(())
    }
    /// Only typed physical samples wait for a terminal focus receipt. No UI command replay.
    pub(crate) fn sample(
        &mut self,
        window: WindowId,
        sample: PhysicalSample,
        fence: u64,
        completed: u64,
    ) -> Result<(), String> {
        self.live()?;
        if self
            .receiver
            .as_ref()
            .is_none_or(|r| r.borrow().window != window)
        {
            return Err("Physical sample belongs to another window/receiver".into());
        }
        if matches!(sample, PhysicalSample::Unfocus) {
            // Event-local cleanup wins over every older receipt-delayed press.
            // A later Gain completion cannot resurrect those physical histories.
            self.cancel_physical();
            self.ready.push_back(sample);
            return Ok(());
        }
        if matches!(self.phase, Phase::Loading | Phase::Draining) {
            self.cancelled_samples += 1;
            return Ok(());
        }
        if fence > completed {
            if self.physical.len() >= 256 {
                let error =
                    "Physical suffix overflow; attachment failed and suffix cancelled".to_owned();
                self.fail(error.clone());
                return Err(error);
            }
            self.physical.push_back(Deferred { fence, sample });
        } else {
            self.admit(sample);
        }
        Ok(())
    }
    fn admit(&mut self, sample: PhysicalSample) {
        let index = if matches!(
            sample,
            PhysicalSample::Key { .. } | PhysicalSample::SyntheticKey { .. }
        ) {
            1
        } else {
            0
        };
        if self
            .receiver
            .as_ref()
            .is_some_and(|r| r.borrow().admitted[index])
        {
            self.ready.push_back(sample);
        }
    }
    pub(crate) fn complete_physical(&mut self, completed: u64) {
        while self.physical.front().is_some_and(|d| d.fence <= completed) {
            let d = self.physical.pop_front().unwrap();
            self.admit(d.sample);
        }
    }
    pub(crate) fn take_samples(&mut self) -> Vec<PhysicalSample> {
        self.ready.drain(..).collect()
    }
    pub(crate) fn commit_levels(&mut self, levels: crate::settings_menu::Levels) {
        if self.activation.guards().music_enabled {
            self.audio.set_music(levels.music);
        }
        self.audio.set_effects(levels.effects);
    }
    pub(crate) fn stop_menu(&mut self) -> Result<(), String> {
        self.menu_owner.cancel()?;
        self.menu_token = None;
        self.menu_music = None;
        Ok(())
    }
    pub(crate) fn menu_token(&mut self) -> Result<audio::MusicToken, String> {
        self.live()?;
        if self.phase != Phase::Menu {
            return Err("Menu decode is outside menu custody".into());
        }
        let token = self.menu_owner.token()?;
        self.menu_token = Some(token.clone());
        Ok(token)
    }
    pub(crate) fn attach_menu(
        &mut self,
        mixer: &rodio::mixer::Mixer,
        token: audio::MusicToken,
        samples: Vec<f32>,
    ) -> Result<(), String> {
        use rodio::Source;
        self.live()?;
        if self.phase != Phase::Menu || !token.is_current() {
            return Err("Retired menu decode completion".into());
        }
        let music = self.audio.music_sink(mixer, token)?;
        music.append(rodio::buffer::SamplesBuffer::new(2, 44100, samples).repeat_infinite())?;
        self.menu_music = Some(music);
        Ok(())
    }
}
pub(super) struct Provider(pub Owner);
impl Effects for Provider {
    fn player_pause(
        &mut self,
        id: AttachmentId,
        focused: bool,
        reason3: i8,
    ) -> Result<Prefix, String> {
        let mut p = self.0.borrow_mut();
        p.live()?;
        if p.attachment != Some(id.value()) || p.phase != Phase::World {
            return Err("Player pause attachment retired".into());
        }
        p.audio_pause(focused, reason3)
    }
    fn activate(&mut self, id: AttachmentId) -> Result<(), String> {
        let mut p = self.0.borrow_mut();
        p.live()?;
        if p.phase != Phase::Loading {
            return Err("Focus attach outside cold loading".into());
        }
        p.generation = p
            .generation
            .checked_add(1)
            .ok_or("Audio generation exhausted")?;
        let generation = p.generation;
        p.frontend.activate(generation)?;
        p.attachment = Some(id.value());
        p.phase = Phase::World;
        Ok(())
    }
    fn retire(&mut self, id: AttachmentId) -> Result<(), String> {
        let mut p = self.0.borrow_mut();
        if p.attachment != Some(id.value()) {
            return Err("Retired process attachment".into());
        }
        let generation = p.generation;
        p.frontend.retire(generation)?;
        p.attachment = None;
        p.draining();
        Ok(())
    }
    fn before_pause(
        &mut self,
        id: AttachmentId,
        focused: bool,
        reason3: i8,
    ) -> Result<Prefix, String> {
        let mut p = self.0.borrow_mut();
        if p.phase != Phase::World || p.attachment != Some(id.value()) {
            return Err("Stale process prefix".into());
        }
        let observed = p
            .observations
            .pop_front()
            .ok_or("Focus prefix has no admitted OS observation")?;
        let result = p.prefix(focused, reason3);
        if result.is_ok() {
            p.applied = observed;
        }
        if let Err(error) = &result {
            p.fail(error.clone());
        }
        result
    }
}
impl Drop for ProcessFrontend {
    fn drop(&mut self) {
        self.cancel_physical();
        self.menu_owner.retire();
        self.menu_music = None;
        self.receiver = None;
        self.activation.active = 0;
        self.phase = Phase::Stopped;
    }
}

#[cfg(test)]
#[path = "process_frontend_tests.rs"]
mod tests;

/// Actual App/Intro physical camera projection; no logical UI command replay.
pub(crate) fn project_camera_sample(
    physical: &mut super::camera_live::PhysicalInput,
    mut view: Option<&mut super::Inspection>,
    sample: PhysicalSample,
) {
    match sample {
        PhysicalSample::Key { key, down, .. } | PhysicalSample::SyntheticKey { key, down } => {
            physical.key(key, down);
            if let winit::keyboard::PhysicalKey::Code(code) = key {
                use winit::keyboard::KeyCode;
                let index = match code {
                    KeyCode::ArrowRight => Some(0),
                    KeyCode::ArrowLeft => Some(1),
                    KeyCode::ArrowDown => Some(2),
                    KeyCode::ArrowUp => Some(3),
                    _ => None,
                };
                if let Some(i) = index
                    && let Some(view) = view.as_deref_mut()
                {
                    view.keys[i] = down;
                    view.taps[i] |= down;
                }
            }
        }
        PhysicalSample::Button { button, down } => physical.button(button, down),
        PhysicalSample::Pointer(point) => {
            let before = physical.snapshot().ok().map(|s| s.2);
            physical.pointer(point);
            if let (Some(before), Ok((_, buttons, after))) = (before, physical.snapshot())
                && let Some(view) = view.as_deref_mut()
                && view.camera_ui.is_none()
                && buttons[1]
                && view.adjustment.is_none()
                && !view.options_visible()
                && view.auction.scene.is_none()
                && view.prompts.request().is_none()
                && !view.stats_requested
                && !view.live_status.as_ref().is_some_and(|s| {
                    s.stats.is_some()
                        || s.tutorial_controls.camera_disabled
                        || s.tutorial_controls.all_disabled
                })
            {
                for i in 0..2 {
                    view.drag[i] = view.drag[i].saturating_add(after[i].saturating_sub(before[i]));
                }
            }
        }
        PhysicalSample::Unfocus => {
            physical.unfocus();
            if let Some(view) = view.as_deref_mut() {
                view.keys = [false; 4];
                view.taps = [false; 4];
            }
        }
    }
}
