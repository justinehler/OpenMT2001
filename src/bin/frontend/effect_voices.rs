//! Frontend WAV admission (4a9f60/4aa880), independent of device ownership.
use rodio::{Source, mixer::Mixer};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ops::Deref,
    rc::Rc,
};

pub struct Request<'a> {
    pub name: &'a str,
    pub streamed: bool,
    pub looping: bool,
    pub unique: bool,
}
impl Request<'_> {
    fn path(&self) -> String {
        // Preserve the caller's spelling, including case and separators.
        format!("gamedata\\sound\\high\\{}", self.name)
    }
}

use super::spatial::{Emitter, Gains, Listener, Stereo};

struct Voice {
    spatial: RefCell<Option<Emitter>>,
    listener: Rc<Cell<Listener>>,
    gains: std::sync::Arc<Gains>,
    sink: rodio::Sink,
    stopped: Cell<bool>,
}
/// A returned handle does not own playback. The frontend retains voices even
/// when a caller discards/overwrites its handle (notably 4ade96's ship loop).
#[derive(Clone)]
pub struct Handle(Rc<Voice>);
impl Handle {
    pub fn set_spatial(&self, emitter: Emitter) {
        *self.0.spatial.borrow_mut() = Some(emitter);
        self.refresh_spatial();
    }
    fn refresh_spatial(&self) {
        if let Some(emitter) = *self.0.spatial.borrow() {
            self.0.gains.store(emitter.gains(self.0.listener.get()));
        }
    }
    #[cfg(test)]
    pub fn spatial_gains(&self) -> [f32; 2] {
        self.0.gains.load()
    }
    pub fn empty(&self) -> bool {
        self.0.stopped.get() || self.0.sink.empty()
    }
    pub fn stop(&self) {
        // Native release is immediate; Sink processes its stop on the next
        // mixer poll. Admission must not suppress a request in that interval.
        self.0.stopped.set(true);
        self.0.sink.stop();
    }
}
impl Deref for Handle {
    type Target = rodio::Sink;
    fn deref(&self) -> &Self::Target {
        &self.0.sink
    }
}
#[cfg(test)]
impl From<rodio::Sink> for Handle {
    fn from(sink: rodio::Sink) -> Self {
        Self(Rc::new(Voice {
            spatial: RefCell::new(None),
            listener: Rc::default(),
            gains: Gains::shared(),
            sink,
            stopped: Cell::new(false),
        }))
    }
}
struct Entry {
    path: String,
    handle: Handle,
    age: i32,
}
#[derive(Default)]
pub struct Owner {
    listener: Rc<Cell<Listener>>,
    // Cached voices compete only within their case-sensitive sample group.
    cached: HashMap<String, [Option<Entry>; 4]>,
    streamed: [Option<Entry>; 10],
}
impl Owner {
    pub fn set_listener(&mut self, listener: Listener) {
        self.listener.set(listener);
        for entry in self
            .cached
            .values()
            .flatten()
            .chain(self.streamed.iter())
            .flatten()
        {
            entry.handle.refresh_spatial();
        }
    }
    /// Once per frontend audio frame, independently of simulation ticks.
    pub fn service(&mut self) {
        for slot in self
            .cached
            .values_mut()
            .flatten()
            .chain(self.streamed.iter_mut())
        {
            if let Some(entry) = slot {
                if entry.handle.empty() {
                    *slot = None;
                } else {
                    entry.age = entry.age.wrapping_add(1);
                }
            }
        }
    }
    pub fn reset(&mut self) {
        for entry in self
            .cached
            .values()
            .flatten()
            .chain(self.streamed.iter())
            .flatten()
        {
            entry.handle.stop();
        }
        self.cached.clear();
        self.streamed = Default::default();
    }
    pub fn request(
        &mut self,
        mixer: &Mixer,
        request: Request<'_>,
        samples: &[f32],
        gain: f32,
    ) -> Option<Handle> {
        let path = request.path();
        let slots: &mut [Option<Entry>] = if request.streamed {
            &mut self.streamed
        } else {
            self.cached.entry(path.clone()).or_default()
        };
        if request.unique
            && slots
                .iter()
                .flatten()
                .any(|entry| entry.path == path && !entry.handle.empty())
        {
            return None;
        }
        let index = slots
            .iter()
            .position(|slot| slot.as_ref().is_none_or(|entry| entry.handle.empty()))
            .or_else(|| {
                let mut best = 0;
                // Cached falls back to cell zero without a positive age;
                // streamed has no fallback if all signed ages are negative.
                let mut selected = (!request.streamed).then_some(0);
                for (index, entry) in slots.iter().enumerate() {
                    if let Some(entry) = entry
                        && (entry.age > best || (request.streamed && entry.age == best))
                    {
                        best = entry.age;
                        selected = Some(index);
                    }
                }
                selected
            })?;
        if let Some(previous) = slots[index].take() {
            previous.handle.stop();
        }
        let sink = rodio::Sink::connect_new(mixer);
        sink.set_volume(gain);
        let source = rodio::buffer::SamplesBuffer::new(1, 22_050, samples.to_vec());
        let gains = Gains::shared();
        // Repeat raw PCM, not rendered stereo: rodio buffers repeat input,
        // and caching spatial gains there would freeze them after one loop.
        if request.looping {
            sink.append(Stereo::new(source.repeat_infinite(), gains.clone()));
        } else {
            sink.append(Stereo::new(source, gains.clone()));
        }
        let voice = Rc::new(Voice {
            spatial: RefCell::new(None),
            listener: self.listener.clone(),
            gains,
            sink,
            stopped: Cell::new(false),
        });
        slots[index] = Some(Entry {
            path,
            handle: Handle(voice.clone()),
            age: 0,
        });
        Some(Handle(voice))
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(name: &str, streamed: bool, unique: bool) -> Request<'_> {
        Request {
            name,
            streamed,
            looping: true,
            unique,
        }
    }
    #[test]
    fn unique_full_paths_are_case_sensitive_and_released_handles_readmit_immediately() {
        for streamed in [false, true] {
            let (mixer, _output) = rodio::mixer::mixer(1, 22_050);
            let mut owner = Owner::default();
            let first = owner
                .request(
                    &mixer,
                    request("dir\\voice.wav", streamed, false),
                    &[0.5; 64],
                    1.,
                )
                .unwrap();
            // A unique request must see earlier nonunique requests too.
            assert!(
                owner
                    .request(
                        &mixer,
                        request("dir\\voice.wav", streamed, true),
                        &[0.5; 64],
                        1.
                    )
                    .is_none()
            );
            assert!(
                owner
                    .request(
                        &mixer,
                        request("dir\\Voice.wav", streamed, true),
                        &[0.5; 64],
                        1.
                    )
                    .is_some()
            );
            assert!(
                owner
                    .request(
                        &mixer,
                        request("elsewhere\\voice.wav", streamed, true),
                        &[0.5; 64],
                        1.
                    )
                    .is_some()
            );
            first.stop();
            assert!(
                owner
                    .request(
                        &mixer,
                        request("dir\\voice.wav", streamed, true),
                        &[0.5; 64],
                        1.
                    )
                    .is_some()
            );
            owner.reset();
            assert!(first.empty());
        }
    }
    #[test]
    fn signed_age_selection_uses_native_zero_and_negative_boundaries() {
        for streamed in [false, true] {
            let (mixer, _output) = rodio::mixer::mixer(1, 22_050);
            let mut owner = Owner::default();
            let count = if streamed { 10 } else { 4 };
            let handles: Vec<_> = (0..count)
                .map(|_| {
                    owner
                        .request(
                            &mixer,
                            request("voice.wav", streamed, false),
                            &[0.5; 64],
                            1.,
                        )
                        .unwrap()
                })
                .collect();
            let slots: &mut [Option<Entry>] = if streamed {
                &mut owner.streamed
            } else {
                owner
                    .cached
                    .get_mut("gamedata\\sound\\high\\voice.wav")
                    .unwrap()
            };
            for entry in slots.iter_mut().flatten() {
                entry.age = -1;
            }
            let result = owner.request(
                &mixer,
                request("voice.wav", streamed, false),
                &[0.5; 64],
                1.,
            );
            if streamed {
                assert!(result.is_none(), "no nonnegative streamed candidate");
                assert!(handles.iter().all(|handle| !handle.empty()));
            } else {
                assert!(result.is_some());
                assert!(handles[0].empty(), "cached falls back to slot zero");
                assert!(handles[1..].iter().all(|handle| !handle.empty()));
            }
        }
    }
    #[test]
    fn first_inactive_precedes_oldest_and_non_equal_ages_replace_oldest() {
        for streamed in [false, true] {
            let (mixer, _output) = rodio::mixer::mixer(1, 22_050);
            let mut owner = Owner::default();
            let count = if streamed { 10 } else { 4 };
            let handles: Vec<_> = (0..count)
                .map(|_| {
                    let handle = owner
                        .request(
                            &mixer,
                            request("voice.wav", streamed, false),
                            &[0.5; 64],
                            1.,
                        )
                        .unwrap();
                    owner.service();
                    handle
                })
                .collect();
            handles[2].stop();
            owner
                .request(
                    &mixer,
                    request("voice.wav", streamed, false),
                    &[0.5; 64],
                    1.,
                )
                .unwrap();
            assert!(!handles[0].empty(), "inactive slot wins over oldest");
            owner
                .request(
                    &mixer,
                    request("voice.wav", streamed, false),
                    &[0.5; 64],
                    1.,
                )
                .unwrap();
            assert!(handles[0].empty());
            assert!(!handles[1].empty());
        }
    }
}
