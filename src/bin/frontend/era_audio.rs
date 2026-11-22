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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Select {
        era: u32,
        repeat: bool,
    },
    /// Original result constructor 4b1881 calls 4af140 before result music.
    Stop,
    BeforeSixCreate,
    BeforeSixRelease(u32),
}

pub struct Request {
    pub operation: Operation,
    pub reply: mpsc::Sender<Result<u32, String>>,
}

pub fn request(
    sender: &Option<mpsc::Sender<Request>>,
    operation: Operation,
) -> Result<u32, String> {
    let Some(sender) = sender else { return Ok(0) };
    let (reply, answer) = mpsc::channel();
    if sender.send(Request { operation, reply }).is_err() {
        return Ok(0);
    }
    answer.recv().unwrap_or(Ok(0))
}

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
    song: Option<rodio::Sink>,
    pending: Option<(bool, TrackDecode)>,
    before_six: Option<(u32, rodio::Sink)>,
    next_handle: u32,
}

impl Player {
    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            music_bus: crate::audio_bus::Bus::new(),
            effects_bus: crate::audio_bus::Bus::new(),
            song: None,
            pending: None,
            before_six: None,
            next_handle: 1,
        }
    }

    pub fn set_levels(&mut self, music: u8, effects: u8) {
        self.music_bus.set_level(music);
        self.effects_bus.set_level(effects);
    }

    fn start_song(&mut self, source: rodio::buffer::SamplesBuffer, repeat: bool) {
        if let Some(mixer) = self.music_bus.mixer() {
            let sink = rodio::Sink::connect_new(mixer);
            if repeat {
                sink.append(source.repeat_infinite());
            } else {
                sink.append(source);
            }
            self.song = Some(sink);
        }
    }

    /// Finish a selected song without blocking the UI frame on WMA decoding.
    /// A later Select or Stop drops the pending receiver, so an older decode
    /// cannot revive music after the city changes era or enters a result view.
    pub fn poll(&mut self) -> Result<(), String> {
        let Some((_, receiver)) = &self.pending else {
            return Ok(());
        };
        match receiver.try_recv() {
            Ok(result) => {
                let (repeat, _) = self.pending.take().unwrap();
                let samples = result?;
                let source = rodio::buffer::SamplesBuffer::new(2, 44_100, samples);
                self.start_song(source, repeat);
                Ok(())
            }
            Err(mpsc::TryRecvError::Empty) => Ok(()),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.pending = None;
                Err("era music decoder disconnected".into())
            }
        }
    }

    pub fn apply(&mut self, operation: Operation) -> Result<u32, String> {
        match operation {
            Operation::Stop => {
                self.pending = None;
                if let Some(song) = self.song.take() {
                    song.stop();
                }
                if let Some((_, cue)) = self.before_six.take() {
                    cue.stop();
                }
                Ok(0)
            }
            Operation::Select { era, repeat } => {
                let index = usize::try_from(era).map_err(|_| "era song index exceeds usize")?;
                let path = self
                    .assets
                    .songs
                    .get(index)
                    .ok_or("era song index outside original catalog")?
                    .clone();
                self.pending = None;
                if let Some(old) = self.song.take() {
                    old.stop();
                }
                let ffmpeg = self.assets.ffmpeg.clone();
                let (sender, receiver) = mpsc::channel();
                std::thread::Builder::new()
                    .name(format!("openmt-era-music-{index}"))
                    .spawn(move || {
                        let _ = sender.send(decode(&ffmpeg, &path));
                    })
                    .map_err(|error| format!("era music decoder thread: {error}"))?;
                self.pending = Some((repeat, receiver));
                Ok(0)
            }
            Operation::BeforeSixCreate => {
                if self.before_six.is_some() {
                    return Err("pre-six sound already active".into());
                }
                let samples = self.assets.before_six.clone();
                let Some(mixer) = self.effects_bus.mixer() else {
                    return Ok(0);
                };
                let sink = rodio::Sink::connect_new(mixer);
                sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                let handle = self.next_handle;
                self.next_handle = handle
                    .checked_add(1)
                    .ok_or("pre-six sound handle overflow")?;
                self.before_six = Some((handle, sink));
                Ok(handle)
            }
            Operation::BeforeSixRelease(handle) => {
                if handle == 0 {
                    return Ok(0);
                }
                let Some((stored, sink)) = self.before_six.take() else {
                    return Err("pre-six sound handle missing".into());
                };
                if stored != handle {
                    self.before_six = Some((stored, sink));
                    return Err(format!(
                        "pre-six sound handle {handle} does not match {stored}"
                    ));
                }
                sink.stop();
                Ok(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_cancels_pending_decode_and_new_selection_replaces_it() {
        let assets = Assets {
            songs: std::array::from_fn(|_| PathBuf::new()),
            before_six: Vec::new(),
            ffmpeg: PathBuf::new(),
        };
        let mut player = Player::new(assets);
        let (stale_sender, stale_receiver) = mpsc::channel();
        player.pending = Some((true, stale_receiver));
        player.apply(Operation::Stop).unwrap();
        assert!(player.pending.is_none());
        assert!(stale_sender.send(Ok(vec![0.; 8])).is_err());

        let (older_sender, older_receiver) = mpsc::channel();
        let (newer_sender, newer_receiver) = mpsc::channel();
        player.pending = Some((true, older_receiver));
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
            std::env::var_os("OPENMT2001_EXE"),
            std::env::var_os("OPENMT2001_SUNCOLS"),
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
        let ffmpeg = std::env::var_os("OPENMT2001_FFMPEG").map(PathBuf::from)
            .unwrap_or_else(|| if cfg!(windows) {
                "work/python/Lib/site-packages/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe".into()
            } else { "ffmpeg".into() });
        let assets = Assets::load(&image, installer, &ffmpeg).unwrap();
        assert_eq!(assets.before_six.len(), 95_385);
        assert!(assets.songs.iter().all(|song| song.is_file()));
        assert!(decode(&assets.ffmpeg, &assets.songs[0]).unwrap().len() > 88_200);
    }
}
