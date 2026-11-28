//! Portable stereo presentation of the DirectSound 3D state. The original
//! delegates speaker/HRTF panning to the driver (4aa7b0), not gameplay code.
use rodio::Source;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub position: [f32; 3],
    pub forward: [f32; 3],
}
impl Default for Listener {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            forward: [0., 0., 1.],
        }
    }
}
impl Listener {
    /// 4afc11..4afca3 uses normalized look-at minus eye and world up.
    pub fn camera(position: [f32; 3], target: [f32; 3]) -> Self {
        let delta = std::array::from_fn(|i| target[i] - position[i]);
        let length = dot(delta, delta).sqrt();
        let forward = if length.is_finite() && length > 0. {
            delta.map(|v| v / length)
        } else {
            [0., 0., 1.]
        };
        Self { position, forward }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Emitter {
    pub position: [f32; 3],
    pub min_distance: f32,
    pub max_distance: f32,
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
impl Emitter {
    pub fn gains(self, listener: Listener) -> [f32; 2] {
        let delta = std::array::from_fn(|i| self.position[i] - listener.position[i]);
        let distance = dot(delta, delta).sqrt();
        // Spatial buffers use MUTE3DATMAXDISTANCE (4aaf19/4abe06); the listener
        // rolloff is 3 and doppler is zero (4a97b0). Within min: unity.
        if !distance.is_finite() || distance > self.max_distance {
            return [0.; 2];
        }
        // DirectSound distance rolloff: https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee418662(v=vs.85)
        let attenuation =
            self.min_distance / (self.min_distance + 3. * (distance - self.min_distance).max(0.));
        // DirectSound's left-handed coordinates: world-up cross forward.
        let right = [listener.forward[2], 0., -listener.forward[0]];
        let length = dot(right, right).sqrt();
        let pan = if distance > 0. && length > 0. {
            (dot(delta, right) / (distance * length)).clamp(-1., 1.)
        } else {
            0.
        };
        // Stereo balance keeps the near speaker at unity and attenuates the
        // opposite speaker. This is a portable speaker renderer, not HRTF.
        [
            attenuation * (1. - pan.max(0.)),
            attenuation * (1. + pan.min(0.)),
        ]
    }
}

pub struct Gains(AtomicU64);
impl Gains {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(pack([1.; 2]))))
    }
    pub fn store(&self, gains: [f32; 2]) {
        self.0.store(pack(gains), Ordering::Relaxed);
    }
    pub fn load(&self) -> [f32; 2] {
        let bits = self.0.load(Ordering::Relaxed);
        [
            f32::from_bits(bits as u32),
            f32::from_bits((bits >> 32) as u32),
        ]
    }
}
fn pack(gains: [f32; 2]) -> u64 {
    u64::from(gains[0].to_bits()) | (u64::from(gains[1].to_bits()) << 32)
}
/// Mono WAV -> stereo, sharing one gain snapshot across each sample pair.
/// Source-level/preference gain remains on Sink, outside this adapter.
#[derive(Clone)]
pub struct Stereo<S> {
    source: S,
    gains: Arc<Gains>,
    right: Option<f32>,
}
impl<S> Stereo<S> {
    pub fn new(source: S, gains: Arc<Gains>) -> Self {
        Self {
            source,
            gains,
            right: None,
        }
    }
}
impl<S: Source> Iterator for Stereo<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let sample = self.source.next()?;
        let gains = self.gains.load();
        self.right = Some(sample * gains[1]);
        Some(sample * gains[0])
    }
}
impl<S: Source> Source for Stereo<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.source
            .current_span_len()
            .map(|n| n * 2 + usize::from(self.right.is_some()))
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}
