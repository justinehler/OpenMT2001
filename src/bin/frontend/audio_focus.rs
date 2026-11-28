//! UI-owned effective/saved audio state. Settings remain with the caller.
//! Factor stores never visit WAV sinks; source-level application reads factor.
//! S4 owns the whole-prefix queue; this adapter performs audio only on the UI.
use std::{
    cell::{Cell, RefCell},
    ops::Deref,
    rc::{Rc, Weak},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Suspend,
    Resume,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Applied,
    GuardedInactive,
}
/// Authoritative frontend activation and original mode flags, supplied by UI.
/// An unopened/failed lazy Bus is not evidence for any of these flags.
#[derive(Clone, Copy, Debug)]
pub struct Guards {
    pub active: bool,
    pub sound_enabled: bool,
    pub music_enabled: bool,
}

trait MusicControl {
    fn pause(&self);
    fn run(&self);
    fn volume(&self, level: u8);
    fn stop(&self);
}
struct MusicSource {
    sink: rodio::Sink,
    level: Cell<u8>,
}
impl MusicControl for MusicSource {
    fn pause(&self) {
        self.sink.pause();
    }
    fn run(&self) {
        self.sink.play();
    }
    fn volume(&self, level: u8) {
        self.level.set(level);
        self.sink.set_volume(crate::music_gain::amplitude(level));
    }
    fn stop(&self) {
        self.sink.stop();
    }
}

/// One playback owner. Cancelling/replacing invalidates every older token.
#[derive(Clone)]
pub struct MusicOwner(Rc<Cell<Option<u64>>>);
#[derive(Clone)]
pub struct MusicToken {
    owner: Weak<Cell<Option<u64>>>,
    generation: u64,
}
impl MusicOwner {
    pub fn new() -> Self {
        Self(Rc::new(Cell::new(Some(0))))
    }
    pub fn token(&self) -> Result<MusicToken, String> {
        Ok(MusicToken {
            owner: Rc::downgrade(&self.0),
            generation: self.0.get().ok_or("Music owner retired")?,
        })
    }
    pub fn cancel(&self) -> Result<(), String> {
        let next = self.0.get().and_then(|n| n.checked_add(1));
        self.0.set(next);
        next.map(|_| ())
            .ok_or_else(|| "Music generation exhausted or retired".into())
    }
    pub fn retire(&self) {
        self.0.set(None);
    }
}
impl MusicToken {
    pub fn is_current(&self) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|owner| owner.get() == Some(self.generation))
    }
}

struct RegisteredMusic {
    sink: Weak<dyn MusicControl>,
    token: MusicToken,
}
struct Mix {
    factor: f32,
    music: u8,
    saved_factor: f32,
    saved_music: f32,
    paused: bool,
    music_sinks: Vec<RegisteredMusic>,
}

/// Keep this cloneable UI handle on the process/frontend owner, not a World.
/// Players bind it before source creation; warm transfers keep the same handle.
#[derive(Clone)]
pub struct SharedAudio(Rc<RefCell<Mix>>);
impl SharedAudio {
    pub fn new(music: u8, effects: u8) -> Self {
        let music = music.min(100);
        let factor = crate::music_gain::normalized_percentage(effects);
        Self(Rc::new(RefCell::new(Mix {
            factor,
            music,
            // 4a9500 constructor sets saved+8=1 and saved+c=75. Profile/
            // activation preferences change the current fields only; an
            // admitted restore before first loss must retain this oddity.
            saved_factor: 1.,
            saved_music: 75.,
            paused: false,
            music_sinks: Vec::new(),
        })))
    }
    pub fn same_owner(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn factor(&self) -> f32 {
        self.0.borrow().factor
    }
    pub fn music_level(&self) -> u8 {
        self.0.borrow().music
    }
    pub fn music_paused(&self) -> bool {
        self.0.borrow().paused
    }
    pub fn set_effects(&self, level: u8) {
        self.0.borrow_mut().factor = crate::music_gain::normalized_percentage(level);
    }
    pub fn set_music(&self, level: u8) {
        let level = level.min(100);
        self.0.borrow_mut().music = level;
        for sink in self.targets() {
            sink.volume(level);
        }
    }
    pub fn source_gain(&self, level: i32) -> f32 {
        crate::music_gain::source_amplitude(self.factor(), level)
    }
    fn targets(&self) -> Vec<Rc<dyn MusicControl>> {
        let mut state = self.0.borrow_mut();
        let mut result = Vec::new();
        let mut cancelled = Vec::new();
        state.music_sinks.retain(|entry| {
            let Some(sink) = entry.sink.upgrade() else {
                return false;
            };
            if !entry.token.is_current() {
                cancelled.push(sink);
                return false;
            }
            result.push(sink);
            true
        });
        drop(state);
        for sink in cancelled {
            sink.stop();
        }
        result
    }
    fn transport(&self, paused: bool) {
        for sink in self.targets() {
            if paused {
                sink.pause();
            } else {
                sink.run();
            }
            // Original member action precedes a fresh read for volume.
            sink.volume(self.music_level());
        }
    }
    pub fn apply(&self, action: Action, guards: Guards) -> Outcome {
        if !guards.active || !guards.sound_enabled {
            return Outcome::GuardedInactive;
        }
        {
            let mut state = self.0.borrow_mut();
            match action {
                Action::Suspend => {
                    state.saved_factor = state.factor;
                    state.factor = 0.;
                    state.saved_music = f32::from(state.music);
                    state.music = 0;
                }
                Action::Resume => {
                    state.factor = state.saved_factor;
                    // Ordinary supported 0..100 values are exact in binary32.
                    state.music = state.saved_music.trunc() as u8;
                }
            }
            if guards.music_enabled {
                state.paused = action == Action::Suspend;
            }
        }
        if guards.music_enabled {
            self.transport(action == Action::Suspend);
        }
        Outcome::Applied
    }
    fn register(&self, token: MusicToken, sink: Rc<dyn MusicControl>) -> Result<(), String> {
        if !token.is_current() {
            return Err("Stale music generation cannot attach".into());
        }
        self.targets(); // Prune dead/cancelled owners before retaining a new weak entry.
        // Apply CURRENT effective state before any source is appended.
        if self.music_paused() {
            sink.pause();
        } else {
            sink.run();
        }
        sink.volume(self.music_level());
        self.0.borrow_mut().music_sinks.push(RegisteredMusic {
            sink: Rc::downgrade(&sink),
            token,
        });
        Ok(())
    }
    pub fn music_sink(
        &self,
        mixer: &rodio::mixer::Mixer,
        token: MusicToken,
    ) -> Result<Music, String> {
        let sink = Rc::new(MusicSource {
            sink: rodio::Sink::connect_new(mixer),
            level: Cell::new(self.music_level()),
        });
        let owner = token.owner.upgrade().ok_or("Music owner retired")?;
        self.register(token.clone(), sink.clone())?;
        Ok(Music {
            sink,
            token,
            _owner: owner,
        })
    }
}

/// Retains one existing Sink and its playback position. Drop stops, never detaches.
pub struct Music {
    sink: Rc<MusicSource>,
    token: MusicToken,
    _owner: Rc<Cell<Option<u64>>>,
}
impl Music {
    /// Member level (+0x438), separate from the shared target at 0x544f24.
    pub fn source_level(&self) -> u8 {
        self.sink.level.get()
    }
    pub fn set_source_level(&self, level: u8) {
        self.sink.volume(level);
    }
    pub fn is_current(&self) -> bool {
        self.token.is_current()
    }
    pub fn append(
        &self,
        source: impl rodio::Source<Item = f32> + Send + 'static,
    ) -> Result<(), String> {
        if !self.is_current() {
            return Err("Stale music generation cannot append".into());
        }
        self.sink.sink.append(source);
        Ok(())
    }
}
impl Deref for Music {
    type Target = rodio::Sink;
    fn deref(&self) -> &Self::Target {
        &self.sink.sink
    }
}
impl Drop for Music {
    fn drop(&mut self) {
        self.sink.stop();
    }
}

/// S4's UI-owned whole-prefix service calls this adapter synchronously.
/// Its queue owns worker wakeup/retirement; no reciprocal worker query occurs.
/// All calls require the same exclusive UI borrow, so generation attachment
/// cannot overtake admitted source/member effects.
pub struct Frontend {
    audio: SharedAudio,
    guards: Guards,
    active_generation: Option<u64>,
    last_generation: Option<u64>,
}
impl Frontend {
    pub fn new(audio: SharedAudio, guards: Guards) -> Self {
        Self {
            audio,
            guards,
            active_generation: None,
            last_generation: None,
        }
    }
    pub fn set_guards(&mut self, guards: Guards) {
        self.guards = guards;
    }
    pub fn activate(&mut self, generation: u64) -> Result<(), String> {
        if self.active_generation.is_some() {
            return Err("Audio frontend generation still attached".into());
        }
        if self.last_generation.is_some_and(|last| generation <= last) {
            return Err("Audio frontend generation is stale".into());
        }
        self.last_generation = Some(generation);
        self.active_generation = Some(generation);
        Ok(())
    }
    pub fn retire(&mut self, generation: u64) -> Result<(), String> {
        if self.active_generation != Some(generation) {
            return Err("Audio frontend generation retired".into());
        }
        self.active_generation = None;
        Ok(())
    }
    pub fn perform(&mut self, generation: u64, action: Action) -> Result<Outcome, String> {
        if self.active_generation != Some(generation) {
            return Err("Audio frontend generation retired".into());
        }
        Ok(self.audio.apply(action, self.guards))
    }
}

#[cfg(test)]
#[path = "audio_focus_tests.rs"]
mod tests;
