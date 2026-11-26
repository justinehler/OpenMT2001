//! One adjustable master gain above each UI-owned rodio player. Child sinks
//! retain their existing relative/positional gains; the master changes output.
//! Music uses its native nominal transfer. Effects retain the linear mapping
//! until combined preference/source gain and application timing are implemented.
use rodio::{mixer::Mixer, source::Zero};

#[path = "audio_focus.rs"]
pub mod focus;

#[derive(Default)]
pub struct Bus {
    stream: Option<rodio::OutputStream>,
    mixer: Option<Mixer>,
    master: Option<rodio::Sink>,
    // Same device mixer, before this Bus's adjustable master. Combined
    // source gains must not pass through the linear effects preference twice.
    combined_output: Option<Mixer>,
    level: u8,
    gain: f32,
    attempted: bool,
}
impl Bus {
    pub fn new() -> Self { Self { level: 100, gain: 1., ..Self::default() } }
    fn connect_to(&mut self, output: &Mixer, channels: u16, sample_rate: u32) {
        let (mixer, source) = rodio::mixer::mixer(channels, sample_rate);
        // A mixer with no source detaches from its parent and would not pick
        // up sounds submitted after the menu closes.
        mixer.add(Zero::new(channels, sample_rate));
        let master = rodio::Sink::connect_new(output);
        master.set_volume(self.gain);
        master.append(source);
        self.combined_output = Some(output.clone());
        self.mixer = Some(mixer);
        self.master = Some(master);
    }
    pub fn mixer(&mut self) -> Option<&Mixer> {
        self.mixer_with(|| rodio::OutputStreamBuilder::open_default_stream().ok())
    }
    fn mixer_with(&mut self, open: impl FnOnce() -> Option<rodio::OutputStream>) -> Option<&Mixer> {
        if self.mixer.is_none() && !self.attempted {
            // A failed device probe must not repeat for every traffic/UI cue
            // on the render thread. A new bus can try again on next launch.
            self.attempted = true;
            let stream = open()?;
            let channels = stream.config().channel_count();
            let sample_rate = stream.config().sample_rate();
            self.connect_to(stream.mixer(), channels, sample_rate);
            self.stream = Some(stream);
        }
        self.mixer.as_ref()
    }
    /// Already-combined children share the existing stream and its cached
    /// failure. The output mixer adds samples at unity; no extra master gain.
    pub fn combined_mixer(&mut self) -> Option<&Mixer> {
        self.mixer()?;
        self.combined_output.as_ref()
    }
    pub fn set_level(&mut self, level: u8) {
        self.level = level.min(100);
        self.gain = f32::from(self.level) / 100.;
        if let Some(master) = &self.master {
            master.set_volume(self.gain);
        }
    }
    pub fn set_music_level(&mut self, level: u8) {
        self.level = level.min(100);
        self.gain = crate::music_gain::amplitude(self.level);
        if let Some(master) = &self.master {
            master.set_volume(self.gain);
        }
    }
    #[cfg(test)]
    pub(super) fn connect_test_output(&mut self, output: &Mixer, channels: u16, sample_rate: u32) {
        self.connect_to(output, channels, sample_rate);
    }
    #[cfg(test)]
    pub fn level(&self) -> u8 { self.level }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_output_is_probed_once_across_repeated_cues() {
        let mut bus = Bus::new();
        let mut attempts = 0;
        for _ in 0..1000 {
            assert!(bus.mixer_with(|| { attempts += 1; None }).is_none());
        }
        bus.set_level(40);
        assert!(bus.mixer_with(|| { attempts += 1; None }).is_none());
        bus.set_music_level(25);
        for _ in 0..1000 { assert!(bus.combined_mixer().is_none()); }
        assert!(bus.mixer_with(|| { attempts += 1; None }).is_none());
        assert_eq!(attempts, 1);
        let mut next_launch = Bus::new();
        assert!(next_launch.mixer_with(|| { attempts += 1; None }).is_none());
        assert_eq!(attempts, 2);
    }
    #[test]
    fn master_level_changes_without_overwriting_child_positional_gain() {
        let (output, _source) = rodio::mixer::mixer(2, 44_100);
        let mut bus = Bus::new();
        bus.connect_to(&output, 2, 44_100);
        let child = rodio::Sink::connect_new(bus.mixer.as_ref().unwrap());
        child.set_volume(0.37);
        bus.set_level(25);
        assert_eq!(bus.level(), 25);
        assert_eq!(bus.master.as_ref().unwrap().volume(), 0.25);
        assert_eq!(child.volume(), 0.37);
        bus.set_level(0);
        assert_eq!(bus.master.as_ref().unwrap().volume(), 0.);
        bus.set_level(100);
        assert_eq!(bus.master.as_ref().unwrap().volume(), 1.);
    }
    #[test]
    fn child_mixer_accepts_music_after_silent_interval() {
        let (output, mut source) = rodio::mixer::mixer(1, 22_050);
        output.add(Zero::new(1, 22_050));
        let mut bus = Bus::new();
        bus.connect_to(&output, 1, 22_050);
        for _ in 0..512 { assert_eq!(source.next(),Some(0.)); }
        let song = rodio::Sink::connect_new(bus.mixer.as_ref().unwrap());
        song.append(rodio::buffer::SamplesBuffer::new(1,22_050,vec![0.5;128]));
        assert!((0..512).filter_map(|_| source.next()).any(|sample| sample > 0.1));
    }
}
