//! UI-owned v1.4 era music and the before-six-o'clock sound handle.
//! 4952f0 registers seven music files; 4ad6f0 forwards (era, repeat) to
//! the sound manager. Music decoding is deferred until an era is selected.
use openmt2001::pe::PeImage;
use rodio::Source;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
};

pub use openmt2001::audio_requests::era::{Operation, Request, request};

const SONGS: [(u32, &str); 7] = [
    (0x542c90, "music_30s.wma"),
    (0x542c80, "music_40s.wma"),
    (0x542c70, "music_50s.wma"),
    (0x542c60, "music_60s.wma"),
    (0x542c50, "music_70s.wma"),
    (0x542c40, "music_80s.wma"),
    (0x542c30, "music_90s.wma"),
];

fn checked_name(image: &PeImage<'_>, address: u32, expected: &str) -> Result<(), String> {
    if image.at(address, expected.len() + 1)? != [expected.as_bytes(), &[0]].concat() {
        return Err(format!("v1.4 era sound string changed at {address:08x}"));
    }
    Ok(())
}

#[derive(Clone)]
pub struct Assets {
    songs: [PathBuf; 7],
    before_six: Vec<f32>,
    ffmpeg: PathBuf,
}

fn pcm16(path: &Path) -> Result<Vec<f32>, String> {
    let path = crate::transport_audio::original_case_path(path)?;
    let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
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
            "{}: unsupported original pre-six PCM format",
            path.display()
        ));
    }
    let length = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    if length == 0 || !length.is_multiple_of(2) || bytes.len() < 44 + length {
        return Err(format!(
            "{}: invalid original pre-six PCM length",
            path.display()
        ));
    }
    Ok(bytes[44..44 + length]
        .chunks_exact(2)
        .map(|sample| f32::from(i16::from_le_bytes(sample.try_into().unwrap())) / 32768.)
        .collect())
}

fn decode(ffmpeg: &Path, path: &Path) -> Result<Vec<f32>, String> {
    let mut command = Command::new(ffmpeg);
    command
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-vn", "-ac", "2", "-ar", "44100", "-f", "f32le", "pipe:1"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command
        .output()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if !output.status.success() || output.stdout.is_empty() || output.stdout.len() % 8 != 0 {
        return Err(format!(
            "{}: era music decode failed: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
        .collect())
}

impl Assets {
    pub fn load(image: &PeImage<'_>, installer: &Path, ffmpeg: &Path) -> Result<Self, String> {
        let directory = installer.join("Standard/sound/music");
        let mut found = Vec::with_capacity(SONGS.len());
        for (address, name) in SONGS {
            checked_name(image, address, name)?;
            let path = directory.join(name);
            if !path.is_file() {
                return Err(format!("{}: original era song missing", path.display()));
            }
            found.push(path);
        }
        let songs: [PathBuf; 7] = found.try_into().map_err(|_| "era song catalog length")?;
        checked_name(image, 0x5419bc, "presixoclock.wav")?;
        let before_six = pcm16(&installer.join("Standard/sound/high/Presixoclock.wav"))?;
        Ok(Self {
            songs,
            before_six,
            ffmpeg: ffmpeg.to_path_buf(),
        })
    }
}

type TrackDecode = mpsc::Receiver<Result<Vec<f32>, String>>;

pub struct Player {
    assets: Assets,
    music_bus: crate::audio_bus::Bus,
    effects_bus: crate::audio_bus::Bus,
    audio: crate::audio_bus::focus::SharedAudio,
    music_owner: crate::audio_bus::focus::MusicOwner,
    song: Option<crate::audio_bus::focus::Music>,
    pending: Option<(bool, TrackDecode)>,
    pending_token: Option<crate::audio_bus::focus::MusicToken>,
    prepared: Option<(bool, Vec<f32>)>,
    change_queued: bool,
    before_six: Option<(u32, crate::audio_bus::effects::Handle)>,
    next_handle: u32,
    warned_before_six_release: bool,
}

impl Player {
    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            music_bus: crate::audio_bus::Bus::new(),
            effects_bus: crate::audio_bus::Bus::new(),
            audio: crate::audio_bus::focus::SharedAudio::new(100, 100),
            music_owner: crate::audio_bus::focus::MusicOwner::new(),
            song: None,
            pending: None,
            pending_token: None,
            prepared: None,
            change_queued: false,
            before_six: None,
            next_handle: 1,
            warned_before_six_release: false,
        }
    }

    /// Bind before Select/creation. Warm transfers retain this same Player.
    pub fn bind_audio(
        &mut self,
        audio: crate::audio_bus::focus::SharedAudio,
    ) -> Result<(), String> {
        if self.audio.same_owner(&audio) {
            return Ok(());
        }
        if self.song.is_some() || self.pending.is_some() || self.prepared.is_some() {
            return Err("Cannot replace an active era music owner".into());
        }
        self.audio = audio;
        Ok(())
    }
    pub fn set_levels(&mut self, music: u8, effects: u8) {
        self.audio.set_music(music);
        self.audio.set_effects(effects);
    }

    fn start_song(
        &mut self,
        source: rodio::buffer::SamplesBuffer,
        repeat: bool,
        token: crate::audio_bus::focus::MusicToken,
    ) -> Result<(), String> {
        if !token.is_current() {
            return Err("Stale era decode generation".into());
        }
        if let Some(mixer) = self.music_bus.combined_mixer() {
            let sink = self.audio.music_sink(mixer, token)?;
            if repeat {
                sink.append(source.repeat_infinite())?;
            } else {
                sink.append(source)?;
            }
            self.song = Some(sink);
        }
        Ok(())
    }

    /// Poll decoding without advancing the per-frame fade. Camera request drains
    /// can call this several times in one draw; they are not audio services.
    pub fn poll(&mut self) -> Result<(), String> {
        let Some((_, receiver)) = &self.pending else {
            return Ok(());
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return Ok(()),
            Err(mpsc::TryRecvError::Disconnected) => Err("era music decoder disconnected".into()),
        };
        let (repeat, _) = self.pending.take().ok_or("Era decode missing")?;
        if !self
            .pending_token
            .as_ref()
            .is_some_and(|token| token.is_current())
        {
            self.pending_token = None;
            self.change_queued = false;
            return Err("Stale era decode generation".into());
        }
        match result {
            Ok(samples) => self.prepared = Some((repeat, samples)),
            Err(error) => {
                self.pending_token = None;
                self.change_queued = false;
                return Err(error);
            }
        }
        if !self.change_queued {
            self.replace_song()?;
        }
        Ok(())
    }

    fn replace_song(&mut self) -> Result<(), String> {
        let Some((repeat, samples)) = self.prepared.take() else {
            return Ok(());
        };
        let token = self
            .pending_token
            .take()
            .ok_or("Era decode has no generation token")?;
        if !token.is_current() {
            return Err("Stale era decode generation".into());
        }
        if let Some(old) = self.song.take() {
            old.stop();
        }
        self.change_queued = false;
        // Registration applies the current target before any new PCM is appended.
        self.start_song(
            rodio::buffer::SamplesBuffer::new(2, 44_100, samples),
            repeat,
            token,
        )
    }

    /// Exactly one 0x4acd30 step per frontend audio service, independent of
    /// simulation pause/speed. Keep the outgoing source alive while decoding.
    pub fn service(&mut self) -> Result<(), String> {
        if !self.change_queued {
            return Ok(());
        }
        if let Some(song) = &self.song {
            let level = song.source_level();
            if level >= 30 {
                song.set_source_level(level - 1);
                return Ok(());
            }
        }
        // A slow decoder can delay replacement, but never stop the old song
        // early or consume extra fade steps through request polling.
        self.replace_song()
    }

    fn select_with(
        &mut self,
        era: u32,
        repeat: bool,
        decoder: impl FnOnce(usize, PathBuf, PathBuf) -> Result<TrackDecode, String>,
    ) -> Result<(), String> {
        let index = usize::try_from(era).map_err(|_| "era song index exceeds usize")?;
        let path = self
            .assets
            .songs
            .get(index)
            .ok_or("era song index outside original catalog")?
            .clone();
        let receiver = decoder(index, self.assets.ffmpeg.clone(), path)?;
        // A new decode owns its own generation. The old source retains
        // its owner until replacement, so settings/focus can still reach it.
        self.music_owner = crate::audio_bus::focus::MusicOwner::new();
        self.pending = None;
        self.pending_token = None;
        self.prepared = None;
        self.change_queued = repeat && self.song.as_ref().is_some_and(|song| !song.empty());
        if !self.change_queued
            && let Some(old) = self.song.take()
        {
            old.stop();
        }
        self.pending_token = Some(self.music_owner.token()?);
        self.pending = Some((repeat, receiver));
        Ok(())
    }

    pub fn apply(&mut self, operation: Operation) -> Result<u32, String> {
        match operation {
            Operation::Stop => {
                self.music_owner.cancel()?;
                self.pending = None;
                self.pending_token = None;
                self.prepared = None;
                self.change_queued = false;
                if let Some(song) = self.song.take() {
                    song.stop();
                }
                if let Some((_, cue)) = self.before_six.take() {
                    cue.stop();
                }
                Ok(0)
            }
            Operation::Select { era, repeat } => {
                self.select_with(era, repeat, |index, ffmpeg, path| {
                    let (sender, receiver) = mpsc::channel();
                    std::thread::Builder::new()
                        .name(format!("openmt-era-music-{index}"))
                        .spawn(move || {
                            let _ = sender.send(decode(&ffmpeg, &path));
                        })
                        .map_err(|error| format!("era music decoder thread: {error}"))?;
                    Ok(receiver)
                })?;
                Ok(0)
            }
            Operation::BeforeSixCreate => {
                if let Some((_, stale)) = self.before_six.take() {
                    stale.stop();
                }
                let samples = self.assets.before_six.clone();
                let gain = self.audio.source_gain(100);
                let Some(mixer) = self.effects_bus.combined_mixer() else {
                    return Ok(0);
                };
                let Some(sink) = self.audio.effect(
                    mixer,
                    crate::audio_bus::effects::Request {
                        name: "presixoclock.wav",
                        streamed: true,
                        looping: false,
                        unique: false,
                    },
                    &samples,
                    gain,
                ) else {
                    return Ok(0);
                };
                let handle = self.next_handle;
                // Zero means no cue. Presentation tokens can safely wrap once
                // the previous cue has been released; exhaustion is not fatal.
                self.next_handle = handle.wrapping_add(1).max(1);
                self.before_six = Some((handle, sink));
                Ok(handle)
            }
            Operation::BeforeSixRelease(handle) => {
                if handle == 0 {
                    return Ok(0);
                }
                if self.before_six.as_ref().map(|(stored, _)| *stored) != Some(handle) {
                    if !self.warned_before_six_release {
                        eprintln!(
                            "[era audio] ignoring missing or stale pre-six sound handle {handle}"
                        );
                        self.warned_before_six_release = true;
                    }
                    return Ok(0);
                }
                if let Some((_, sink)) = self.before_six.take() {
                    sink.stop();
                }
                Ok(0)
            }
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.music_owner.retire();
        self.pending = None;
        self.pending_token = None;
        self.prepared = None;
        // Music's Drop stops its non-detached Sink; Bus stream drops afterward.
        self.song = None;
    }
}

#[cfg(test)]
#[path = "era_audio_fade_tests.rs"]
mod fade_tests;

#[cfg(test)]
mod focus_tests {
    include!("era_audio_focus_tests.rs");
}

#[cfg(test)]
mod pre_six_tests {
    include!("era_audio_pre_six_tests.rs");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_gain_reaches_decoded_song_at_start_and_on_retained_bus() {
        // Fixed original-derived command/amplitude cases, not values computed
        // by music_gain or read back from Sink.volume(). PCM input is 0.5.
        for (initial, expected) in [(0, 0.), (25, 0.017336843), (50, 0.0638955)] {
            let assets = Assets {
                songs: std::array::from_fn(|_| PathBuf::new()),
                before_six: Vec::new(),
                ffmpeg: PathBuf::new(),
            };
            let mut player = Player::new(assets);
            player.set_levels(initial, 100);
            let (output, mut rendered) = rodio::mixer::mixer(2, 44_100);
            player.music_bus.connect_test_output(&output, 2, 44_100);
            let (sender, receiver) = mpsc::channel();
            player.pending_token = Some(player.music_owner.token().unwrap());
            player.pending = Some((false, receiver));
            // Prime the connected master while the real decode is still pending.
            // Rodio connects an Empty mono48k queue before append; its initial
            // conversion spans must not be conflated with decoded-song gain.
            player.poll().unwrap();
            assert!(player.pending.is_some());
            assert!(player.song.is_none());
            let pending_silence: Vec<_> = rendered.by_ref().take(4096).collect();
            assert_eq!(pending_silence.len(), 4096);
            assert!(
                pending_silence.iter().all(|sample| *sample == 0.),
                "no decoded source exists during the pending master prime"
            );
            sender.send(Ok(vec![0.5; 65_536])).unwrap();
            player.poll().unwrap();
            assert!(player.pending.is_none());
            let first: Vec<_> = rendered.by_ref().take(4096).collect();
            assert_eq!(first.len(), 4096);
            eprintln!(
                "era startup initial={initial}: first_nonzero={:?}, last_gain_mismatch={:?}",
                first.iter().position(|sample| *sample != 0.),
                first
                    .iter()
                    .rposition(|sample| (*sample - expected).abs() >= 1e-6)
            );
            // Allow queued startup silence, but no samples at the old linear
            // or default full gain before the intended gain takes effect.
            assert!(
                first
                    .iter()
                    .all(|sample| *sample == 0. || (*sample - expected).abs() < 1e-6)
            );
            assert!(
                first[2048..]
                    .iter()
                    .all(|sample| (*sample - expected).abs() < 1e-6)
            );
            for (level, expected) in [
                (0, 0.),
                (25, 0.017336843),
                (50, 0.0638955),
                (100, 0.5),
                (255, 0.5),
            ] {
                let position = player.song.as_ref().unwrap().get_pos();
                player.set_levels(level, 100);
                let changed: Vec<_> = rendered.by_ref().take(4096).collect();
                assert_eq!(changed.len(), 4096);
                assert!(
                    changed[2048..]
                        .iter()
                        .all(|sample| (*sample - expected).abs() < 1e-6),
                    "music percentage {level}, initial {initial}"
                );
                let song = player.song.as_ref().unwrap();
                assert!(
                    song.get_pos() > position,
                    "gain changes keep the same source advancing"
                );
                assert_eq!(song.len(), 1, "gain changes do not queue replacement songs");
            }
            player.apply(Operation::Stop).unwrap();
            assert!(player.song.is_none());
            let stopped: Vec<_> = rendered.by_ref().take(4096).collect();
            assert_eq!(stopped.len(), 4096);
            assert!(stopped[2048..].iter().all(|sample| *sample == 0.));
        }
    }

    #[test]
    fn before_six_effect_consumes_q100_factor_at_creation() {
        let assets = Assets {
            songs: std::array::from_fn(|_| PathBuf::new()),
            before_six: vec![0.5; 32_768],
            ffmpeg: PathBuf::new(),
        };
        let mut player = Player::new(assets);
        player.set_levels(50, 25);
        let (output, mut rendered) = rodio::mixer::mixer(1, 22_050);
        player.effects_bus.connect_test_output(&output, 1, 22_050);
        let handle = player.apply(Operation::BeforeSixCreate).unwrap();
        assert_ne!(handle, 0);
        let samples: Vec<_> = rendered.by_ref().take(4096).collect();
        assert_eq!(samples.len(), 4096);
        assert!(
            samples[2048..]
                .iter()
                .all(|sample| (*sample - 0.017336842).abs() < 1e-6)
        );
        player.apply(Operation::BeforeSixRelease(handle)).unwrap();
    }

    #[test]
    fn stop_cancels_pending_decode_and_new_selection_replaces_it() {
        let assets = Assets {
            songs: std::array::from_fn(|_| PathBuf::new()),
            before_six: Vec::new(),
            ffmpeg: PathBuf::new(),
        };
        let mut player = Player::new(assets);
        let (stale_sender, stale_receiver) = mpsc::channel();
        player.pending_token = Some(player.music_owner.token().unwrap());
        player.pending = Some((true, stale_receiver));
        player.apply(Operation::Stop).unwrap();
        assert!(player.pending.is_none());
        assert!(stale_sender.send(Ok(vec![0.; 8])).is_err());

        let (older_sender, older_receiver) = mpsc::channel();
        let (newer_sender, newer_receiver) = mpsc::channel();
        player.pending_token = Some(player.music_owner.token().unwrap());
        player.pending = Some((true, older_receiver));
        player.pending_token = Some(player.music_owner.token().unwrap());
        player.pending = Some((false, newer_receiver));
        assert!(older_sender.send(Ok(vec![0.; 8])).is_err());
        // Check the nonblocking path while decoding is still in flight.
        player.poll().unwrap();
        assert!(!player.pending.as_ref().unwrap().0);
        newer_sender
            .send(Err("fixture decode failure".into()))
            .unwrap();
        assert_eq!(player.poll().unwrap_err(), "fixture decode failure");
        assert!(player.pending.is_none());
    }

    #[test]
    fn original_era_registration_and_sources() {
        let (Some(exe), Some(suncols)) = (
            openmt2001::fixtures::optional("EXE"),
            openmt2001::fixtures::optional("SUNCOLS"),
        ) else {
            return;
        };
        let exe = std::fs::read(exe).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        // 4952f0 registers indices 0..6 through 4ad740 using these exact
        // string pointers, then starts music_intro through 4ad670.
        for (i, (pointer, name)) in SONGS.iter().enumerate() {
            checked_name(&image, *pointer, name).unwrap();
            let push_name = image.at(0x4952fc + i as u32 * 0x11, 5).unwrap();
            assert_eq!(push_name[0], 0x68);
            assert_eq!(
                u32::from_le_bytes(push_name[1..5].try_into().unwrap()),
                *pointer
            );
        }
        checked_name(&image, 0x542c20, "music_intro.wma").unwrap();
        assert_eq!(image.at(0x495371, 1).unwrap(), &[0x68]);
        assert_eq!(
            u32::from_le_bytes(image.at(0x495372, 4).unwrap().try_into().unwrap()),
            0x542c20,
        );
        let intro_call = image.at(0x49537b, 5).unwrap();
        assert_eq!(intro_call[0], 0xe8);
        assert_eq!(
            0x495380_u32
                .wrapping_add_signed(i32::from_le_bytes(intro_call[1..5].try_into().unwrap())),
            0x4ad670,
        );
        let standard = Path::new(&suncols).parent().unwrap();
        let installer = standard.parent().unwrap();
        let ffmpeg = openmt2001::fixtures::ffmpeg();
        let assets = Assets::load(&image, installer, &ffmpeg).unwrap();
        assert_eq!(assets.before_six.len(), 95_385);
        assert!(assets.songs.iter().all(|song| song.is_file()));
        assert!(decode(&assets.ffmpeg, &assets.songs[0]).unwrap().len() > 88_200);
    }
}

#[cfg(test)]
mod cold_gain_tests {
    use super::*;
    #[test]
    fn cold_nested_music_start_has_only_silence_then_intended_gain() {
        for (level, expected) in [(0, 0.), (25, 0.017336843), (50, 0.0638955)] {
            let assets = Assets {
                songs: std::array::from_fn(|_| PathBuf::new()),
                before_six: Vec::new(),
                ffmpeg: PathBuf::new(),
            };
            let mut player = Player::new(assets);
            player.set_levels(level, 100);
            let (output, mut rendered) = rodio::mixer::mixer(2, 44_100);
            player.music_bus.connect_test_output(&output, 2, 44_100);
            let (sender, receiver) = mpsc::channel();
            player.pending = Some((false, receiver));
            player.pending_token = Some(player.music_owner.token().unwrap());
            sender.send(Ok(vec![0.5; 65_536])).unwrap();
            player.poll().unwrap();
            assert!(player.pending.is_none());
            // Both newly connected queues begin with their own silent/conversion
            // spans. The decoded source may start later than the primed-master
            // case; every nonzero sample must already have the intended gain.
            let samples = rendered.by_ref().take(8192).collect::<Vec<_>>();
            assert_eq!(samples.len(), 8192);
            assert!(
                samples
                    .iter()
                    .all(|s| *s == 0. || (*s - expected).abs() < 1e-6)
            );
            assert!(
                samples[4096..].iter().all(|s| (*s - expected).abs() < 1e-6),
                "cold music level {level}"
            );
            if level != 0 {
                assert!(
                    samples[..4096].iter().any(|s| *s != 0.),
                    "cold source must start within the fixed window"
                );
            }
            let song = player.song.as_ref().unwrap();
            assert!(song.get_pos() > std::time::Duration::ZERO);
            assert_eq!(song.len(), 1);
        }
    }
}

#[cfg(test)]
impl Player {
    pub(crate) fn composition_pending_pcm(
        &mut self,
        audio: crate::audio_bus::focus::SharedAudio,
        mixer: &rodio::mixer::Mixer,
    ) -> mpsc::Sender<Result<Vec<f32>, String>> {
        self.bind_audio(audio).unwrap();
        self.music_bus.connect_test_output(mixer, 2, 44100);
        self.pending_token = Some(self.music_owner.token().unwrap());
        let (sender, receiver) = mpsc::channel();
        self.pending = Some((true, receiver));
        sender
    }
    pub(crate) fn composition_song_state(&self) -> Option<(bool, f32)> {
        self.song
            .as_ref()
            .map(|song| (song.is_paused(), song.volume()))
    }
}
