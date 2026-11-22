//! One adjustable master gain above each UI-owned rodio player. Child sinks
//! retain original relative/positional gains; the master changes live output.
use rodio::{mixer::Mixer, source::Zero};

#[derive(Default)]
pub struct Bus {
    stream: Option<rodio::OutputStream>,
    mixer: Option<Mixer>,
    master: Option<rodio::Sink>,
    level: u8,
}
impl Bus {
    pub fn new() -> Self { Self { level: 100, ..Self::default() } }
    fn connect_to(&mut self, output: &Mixer, channels: u16, sample_rate: u32) {
        let (mixer, source) = rodio::mixer::mixer(channels, sample_rate);
        // A mixer with no source detaches from its parent and would not pick
        // up sounds submitted after the menu closes.
        mixer.add(Zero::new(channels, sample_rate));
        let master = rodio::Sink::connect_new(output);
        master.set_volume(f32::from(self.level) / 100.);
        master.append(source);
        self.mixer = Some(mixer);
        self.master = Some(master);
    }
    pub fn mixer(&mut self) -> Option<&Mixer> {
        if self.mixer.is_none() {
            let stream = rodio::OutputStreamBuilder::open_default_stream().ok()?;
            let channels = stream.config().channel_count();
            let sample_rate = stream.config().sample_rate();
            self.connect_to(stream.mixer(), channels, sample_rate);
            self.stream = Some(stream);
        }
        self.mixer.as_ref()
    }
    pub fn set_level(&mut self, level: u8) {
        self.level = level.min(100);
        if let Some(master) = &self.master {
            master.set_volume(f32::from(self.level) / 100.);
        }
    }
    #[cfg(test)]
    pub fn level(&self) -> u8 { self.level }
}

#[cfg(test)]
mod tests {
    use super::*;
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
