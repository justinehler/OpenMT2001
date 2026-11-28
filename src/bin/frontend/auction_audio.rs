//! UI-owned v1.4 auction sounds. The retained worker receives the sound handle
//! synchronously, just as original 4ad760 returns a handle to each controller.
use openmt2001::pe::PeImage;
use std::{collections::BTreeMap, path::Path};

pub use openmt2001::audio_requests::auction::{Kind, Operation, Request, enqueue, request};

#[derive(Clone)]
pub struct Assets {
    announcement: Vec<f32>,
    turn_timer: Vec<f32>,
}

fn native_name(image: &PeImage<'_>, address: u32) -> Result<&'static str, String> {
    let expected = match address {
        0x534c50 => "ambi_any_build_hospitalbeep.wav",
        0x533a88 => "auction\\dynam_auctioncounter.wav",
        _ => return Err(format!("unsupported auction sound string {address:08x}")),
    };
    let bytes = image.at(address, expected.len() + 1)?;
    if bytes != [expected.as_bytes(), &[0]].concat() {
        return Err(format!(
            "v1.4 auction sound string changed at {address:08x}"
        ));
    }
    Ok(expected)
}

fn pcm16(path: &Path) -> Result<Vec<f32>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if bytes.len() < 44
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || u32::from_le_bytes(bytes[16..20].try_into().unwrap()) != 16
        || u16::from_le_bytes(bytes[20..22].try_into().unwrap()) != 1
        || u16::from_le_bytes(bytes[22..24].try_into().unwrap()) != 1
        || u32::from_le_bytes(bytes[24..28].try_into().unwrap()) != 22_050
        || u16::from_le_bytes(bytes[34..36].try_into().unwrap()) != 16
        || &bytes[36..40] != b"data"
    {
        return Err(format!(
            "{}: unsupported original auction PCM format",
            path.display()
        ));
    }
    let length = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    let riff_length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8;
    if length == 0
        || !length.is_multiple_of(2)
        || bytes.len() != riff_length
        || bytes.len() < 44 + length
    {
        return Err(format!(
            "{}: invalid original auction PCM length",
            path.display()
        ));
    }
    Ok(bytes[44..44 + length]
        .chunks_exact(2)
        .map(|sample| f32::from(i16::from_le_bytes(sample.try_into().unwrap())) / 32768.)
        .collect())
}

impl Assets {
    pub fn load(image: &PeImage<'_>, installer: &Path) -> Result<Self, String> {
        let directory = installer.join("Standard/sound/high");
        let filename = |address| {
            native_name(image, address).map(|name| directory.join(name.replace('\\', "/")))
        };
        Ok(Self {
            announcement: pcm16(&filename(0x534c50)?)?,
            turn_timer: pcm16(&filename(0x533a88)?)?,
        })
    }
    fn samples(&self, kind: Kind) -> &[f32] {
        match kind {
            Kind::Announcement => &self.announcement,
            Kind::TurnTimer => &self.turn_timer,
        }
    }
}

pub struct Player {
    assets: Assets,
    bus: crate::audio_bus::Bus,
    audio: crate::audio_bus::focus::SharedAudio,
    sinks: BTreeMap<u32, crate::audio_bus::effects::Handle>,
    next_handle: u32,
}
impl Player {
    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            bus: crate::audio_bus::Bus::new(),
            audio: crate::audio_bus::focus::SharedAudio::new(100, 100),
            sinks: BTreeMap::new(),
            next_handle: 1,
        }
    }
    pub fn bind_audio(&mut self, audio: crate::audio_bus::focus::SharedAudio) {
        self.audio = audio;
    }
    pub fn set_level(&mut self, effects: u8) {
        self.audio.set_effects(effects);
    }
    pub fn apply(&mut self, operation: Operation) -> Result<u32, String> {
        match operation {
            Operation::Reset => {
                for sink in self.sinks.values() {
                    sink.stop();
                }
                Ok(0)
            }
            Operation::Create(kind) => {
                // Original auction callers accept a zero 4ad760 result and skip
                // release while the stored handle is zero.
                let samples = self.assets.samples(kind).to_vec();
                let gain = self.audio.source_gain(100);
                let Some(mixer) = self.bus.combined_mixer() else {
                    return Ok(0);
                };
                let handle = self.next_handle;
                if handle == 0 || self.sinks.contains_key(&handle) {
                    return Err("auction audio handle space exhausted".into());
                }
                self.next_handle = handle.checked_add(1).unwrap_or(0);
                let name = match kind {
                    Kind::Announcement => "ambi_any_build_hospitalbeep.wav",
                    Kind::TurnTimer => "auction\\dynam_auctioncounter.wav",
                };
                let Some(sink) = self.audio.effect(
                    mixer,
                    crate::audio_bus::effects::Request {
                        name,
                        streamed: false,
                        looping: false,
                        unique: true,
                    },
                    &samples,
                    gain,
                ) else {
                    return Ok(0);
                };
                self.sinks.insert(handle, sink);
                Ok(handle)
            }
            Operation::Release(handle) => {
                let sink = self
                    .sinks
                    .remove(&handle)
                    .ok_or_else(|| format!("auction audio release of unknown handle {handle}"))?;
                sink.stop();
                Ok(0)
            }
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        for sink in self.sinks.values() {
            sink.stop();
        }
    }
}

#[cfg(test)]
mod focus_tests {
    use super::*;
    use crate::audio_bus::focus::{Action, Guards, SharedAudio};
    #[test]
    fn announcement_and_timer_keep_creation_gain_through_focus_and_settings() {
        for kind in [Kind::Announcement, Kind::TurnTimer] {
            for (initial, expected) in [(0, 0.), (25, 0.017336842)] {
                let audio = SharedAudio::new(75, initial);
                let mut player = Player::new(Assets {
                    announcement: vec![0.5; 65536],
                    turn_timer: vec![0.5; 65536],
                });
                player.bind_audio(audio.clone());
                let (mixer, mut output) = rodio::mixer::mixer(1, 22050);
                mixer.add(rodio::source::Zero::new(1, 22050));
                player.bus.connect_test_output(&mixer, 1, 22050);
                let handle = player.apply(Operation::Create(kind)).unwrap();
                assert_ne!(handle, 0);
                for stage in 0..4 {
                    match stage {
                        1 => {
                            player.set_level(100);
                        }
                        2 => {
                            audio.apply(
                                Action::Suspend,
                                Guards {
                                    active: true,
                                    sound_enabled: true,
                                    music_enabled: true,
                                },
                            );
                        }
                        3 => {
                            audio.apply(
                                Action::Resume,
                                Guards {
                                    active: true,
                                    sound_enabled: true,
                                    music_enabled: true,
                                },
                            );
                        }
                        _ => (),
                    }
                    let samples: Vec<_> = output.by_ref().take(4096).collect();
                    assert!(
                        samples
                            .iter()
                            .all(|sample| *sample == 0. || (*sample - expected).abs() < 1e-6)
                    );
                    assert!(
                        samples[2048..]
                            .iter()
                            .all(|sample| (*sample - expected).abs() < 1e-6)
                    );
                }
                player.apply(Operation::Reset).unwrap();
                player.apply(Operation::Release(handle)).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_reset_silences_auction_sink_before_handle_release() {
        use rodio::Source;
        let (mixer, mut output) = rodio::mixer::mixer(1, 22_050);
        mixer.add(rodio::source::Zero::new(1, 22_050));
        for _ in 0..512 {
            assert_eq!(output.next(), Some(0.));
        }
        let sink = rodio::Sink::connect_new(&mixer);
        sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, vec![0.5; 64]).repeat_infinite());
        let mut player = Player::new(Assets {
            announcement: vec![],
            turn_timer: vec![],
        });
        player.sinks.insert(7, sink.into());
        assert!(
            (0..512)
                .filter_map(|_| output.next())
                .any(|sample| sample > 0.)
        );
        assert_eq!(player.apply(Operation::Reset).unwrap(), 0);
        let tail: Vec<_> = (0..8192).map(|_| output.next()).collect();
        assert!(
            tail[tail.len() - 1024..]
                .iter()
                .all(|sample| *sample == Some(0.))
        );
        assert!(player.sinks.contains_key(&7));
        assert_eq!(player.apply(Operation::Release(7)).unwrap(), 0);
        assert!(player.sinks.is_empty());
    }

    #[test]
    fn original_auction_sound_strings_and_pcm() {
        let Some(exe) = openmt2001::fixtures::optional("EXE") else {
            return;
        };
        let Some(suncols) = openmt2001::fixtures::optional("SUNCOLS") else {
            return;
        };
        let exe = std::fs::read(exe).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let installer = Path::new(&suncols).parent().unwrap().parent().unwrap();
        let assets = Assets::load(&image, installer).unwrap();
        assert_eq!(assets.announcement.len(), 4099);
        assert_eq!(assets.turn_timer.len(), 19456);
        assert!(
            native_name(&image, 0x534c50)
                .unwrap()
                .ends_with("hospitalbeep.wav")
        );
        assert!(
            native_name(&image, 0x533a88)
                .unwrap()
                .ends_with("auctioncounter.wav")
        );
    }
}
