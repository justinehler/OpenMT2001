//! UI owner for v1.4 card/ledger cues and the auction ambient handle.
//! Original 4ad760 accepts a high-sound filename and five playback words.
use openmt2001::pe::PeImage;
use rodio::Source;
use std::{collections::HashMap, path::{Path, PathBuf}, sync::mpsc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Native 4af140 calls the backend's +24 method before releasing retained
    /// city handles. Auction closure releases its murmur handle afterward.
    Reset,
    Targeted { actor_is_current: bool },
    AuctionAmbientCreate,
    AuctionAmbientRelease(u32),
    /// Native card-panel callback 41b430 chooses one of two cues from the
    /// original 20-byte card row's fourth word.
    CardOpen { positive: bool },
    /// Original result particle 4549a9/453c1c cue, after a 50 Hz firework tick.
    ResultFirework(crate::fresh_city::result::fireworks::Cue),
    /// 465980 dispatches these only when 4ecc20 completes an auction actor
    /// animation, after the producer has stored its participant state.
    AuctionActor(crate::auction_scene::ActorCue),
    Script { name: String },
    /// Native 48a5a0 plays a 60-point buy/sell cue for the local ledger.
    Ledger { sale: bool },
    /// Original 42f33e/42ca30/4433e8 widget activations.
    UiLightClick,
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

/// Script sound requests can arise while the UI is waiting for a worker reply.
/// Queue them without a reciprocal wait so a status/snapshot request cannot
/// deadlock the simulation and renderer threads.
pub fn enqueue(sender: &Option<mpsc::Sender<Request>>, operation: Operation) -> Result<(), String> {
    let Some(sender) = sender else { return Ok(()) };
    let (reply, _answer) = mpsc::channel();
    let _ = sender.send(Request { operation, reply });
    Ok(())
}

#[derive(Clone)]
pub struct Assets {
    weapon: Vec<f32>,
    spell: Vec<f32>,
    auction_murmur: Vec<f32>,
    positive_pluck: Vec<f32>,
    negative_boing: Vec<f32>,
    firework_launch1: Vec<f32>,
    firework_launch2: Vec<f32>,
    firework_echo_crackle: Vec<f32>,
    firework_echo_single: Vec<f32>,
    firework_whistle: Vec<f32>,
    auction_voices: Vec<[Vec<f32>; 3]>,
    auction_applause: Vec<f32>,
    buy: Vec<f32>,
    sell: Vec<f32>,
    ui_light_click: Vec<f32>,
    script_directory: PathBuf,
}

fn checked_name(image: &PeImage<'_>, address: u32, expected: &str) -> Result<(), String> {
    if image.at(address, expected.len() + 1)? != [expected.as_bytes(), &[0]].concat() {
        return Err(format!(
            "v1.4 gameplay sound string changed at {address:08x}"
        ));
    }
    Ok(())
}

fn auction_voice_name(image: &PeImage<'_>, character: u32, kind: usize) -> Result<String, String> {
    let suffix = ["_bid.wav", "_win.wav", "_withdraw.wav"]
        .get(kind).ok_or("Auction voice kind outside original table")?;
    let at = 0x53c298 + character * 12 + kind as u32 * 4;
    let address = u32::from_le_bytes(image.at(at, 4)?.try_into().unwrap());
    let bytes = image.at(address, 64)?;
    let end = bytes.iter().position(|byte| *byte == 0).ok_or("Auction voice name terminator")?;
    let name = std::str::from_utf8(&bytes[..end]).map_err(|e| e.to_string())?;
    if !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.'))
        || !name.to_ascii_lowercase().ends_with(*suffix) {
        return Err("Original auction voice name is outside its high-sound catalog".into());
    }
    // The v1.4 PE stores `WheelBarrow` while the supplied installer spells
    // its filename lowercase. Normalize ASCII case for Linux/macOS paths.
    Ok(name.to_ascii_lowercase())
}

pub fn card_open_positive(image: &PeImage<'_>, card: u32) -> Result<bool, String> {
    let row = openmt2001::event_cards::lifecycle::card_row(image, card)?;
    Ok(i32::from_le_bytes(row[12..16].try_into().unwrap()) <= 6)
}

pub(crate) fn pcm16(path: &Path) -> Result<Vec<f32>, String> {
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
            "{}: unsupported original gameplay PCM format",
            path.display()
        ));
    }
    let length = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    if length == 0 || !length.is_multiple_of(2) || bytes.len() < 44 + length {
        return Err(format!(
            "{}: invalid original gameplay PCM length",
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
        checked_name(image, 0x5349ec, "weaponfired.wav")?;
        checked_name(image, 0x5349fc, "spell.wav")?;
        checked_name(image, 0x53c518, "auction\\auctionmurmer.wav")?;
        checked_name(image, 0x53c50c, "auction\\%s")?;
        checked_name(image, 0x53c6ec, "auction\\ambi_any_applausehigh.wav")?;
        checked_name(image, 0x5348e4, "Positivepluck.wav")?;
        checked_name(image, 0x5348f8, "Negativeboing.wav")?;
        for (address, name) in [
            (0x53a184, "launch1.wav"),
            (0x53a190, "echosingle.wav"),
            (0x53a1a0, "echocrackle.wav"),
            (0x53a1b0, "launch2.wav"),
            (0x53a1bc, "launchwhistle.wav"),
        ] {
            checked_name(image, address, name)?;
        }
        checked_name(image, 0x541cb4, "spot_any_buy.wav")?;
        checked_name(image, 0x541cc8, "spot_any_sell.wav")?;
        checked_name(image, 0x535870, "spot_any_ui_lightclick.wav")?;
        let directory = installer.join("Standard/sound/high");
        let mut auction_voices = Vec::with_capacity(10);
        for character in 0..10 {
            auction_voices.push([
                pcm16(&directory.join("auction").join(auction_voice_name(image, character, 0)?))?,
                pcm16(&directory.join("auction").join(auction_voice_name(image, character, 1)?))?,
                pcm16(&directory.join("auction").join(auction_voice_name(image, character, 2)?))?,
            ]);
        }
        Ok(Self {
            weapon: pcm16(&directory.join("weaponfired.wav"))?,
            spell: pcm16(&directory.join("spell.wav"))?,
            auction_murmur: pcm16(&directory.join("auction/auctionmurmer.wav"))?,
            positive_pluck: pcm16(&directory.join("Positivepluck.wav"))?,
            negative_boing: pcm16(&directory.join("Negativeboing.wav"))?,
            firework_launch1: pcm16(&directory.join("launch1.wav"))?,
            firework_launch2: pcm16(&directory.join("launch2.wav"))?,
            firework_echo_crackle: pcm16(&directory.join("echocrackle.wav"))?,
            firework_echo_single: pcm16(&directory.join("echosingle.wav"))?,
            firework_whistle: pcm16(&directory.join("launchwhistle.wav"))?,
            auction_voices,
            auction_applause: pcm16(&directory.join("auction/ambi_any_applausehigh.wav"))?,
            buy: pcm16(&directory.join("spot_any_buy.wav"))?,
            sell: pcm16(&directory.join("spot_any_sell.wav"))?,
            ui_light_click: pcm16(&directory.join("spot_any_ui_lightclick.wav"))?,
            script_directory: directory,
        })
    }
}

pub struct Player {
    assets: Assets,
    bus: crate::audio_bus::Bus,
    one_shots: Vec<rodio::Sink>,
    ledger_cues: Vec<LedgerCue>,
    ui_click_cues: Vec<UiClickCue>,
    audio: crate::audio_bus::focus::SharedAudio,
    scripted: HashMap<String, Vec<f32>>,
    ambient: Option<(u32, rodio::Sink)>,
    firework_bursts: Vec<rodio::Sink>,
    firework_whistles: Vec<rodio::Sink>,
    auction_bid_sinks: [Option<rodio::Sink>; 6],
    auction_withdraw_sinks: [Option<rodio::Sink>; 6],
    next_handle: u32,
}

/// Raw original +438 source word remains separate from applied Sink gain.
struct LedgerCue {
    source_level: i32,
    sink: rodio::Sink,
}

/// Raw UI source word, independent of the applied gain just like +438.
struct UiClickCue {
    source_level: i32,
    sink: rodio::Sink,
}

/// 4ab420/4abad0: factor was already stored binary32 by 4aa750.
/// Only the fixed q60 ledger and q100 UI/card-open callers are qualified here.
/// No binary32 spill of q*c is allowed before the final combined store.
/// Each q*c needs at most 27 significant bits; multiplying a binary32 factor
/// needs at most 51, exact in f64. This is not a general raw32/x87 API.
fn fixed_cue_normalized(factor: f32, source_level: i32) -> f32 {
    crate::music_gain::source_normalized(factor, source_level)
}

fn play_bounded(
    mixer: &rodio::mixer::Mixer,
    pool: &mut Vec<rodio::Sink>,
    capacity: usize,
    samples: Vec<f32>,
    gain: f32,
) -> bool {
    pool.retain(|sink| !sink.empty());
    if pool.len() >= capacity { return false }
    let sink = rodio::Sink::connect_new(mixer);
    sink.set_volume(gain);
    sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
    pool.push(sink);
    true
}

fn replace_auction_voice(
    slot: &mut Option<rodio::Sink>,
    mixer: Option<&rodio::mixer::Mixer>,
    samples: Vec<f32>,
    suppressed: bool,
    gain: f32,
) {
    // Native 465a8b/4647a5 release the previous retained handle before
    // testing panel+858. A muted callback still stops the earlier voice.
    if let Some(prior) = slot.take() { prior.stop(); }
    if suppressed { return; }
    if let Some(mixer) = mixer {
        let sink = rodio::Sink::connect_new(mixer);
        sink.set_volume(gain);
        sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
        *slot = Some(sink);
    }
}

impl Player {
    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            bus: crate::audio_bus::Bus::new(),
            one_shots: Vec::new(),
            ledger_cues: Vec::new(),
            ui_click_cues: Vec::new(),
            // Preserve the existing Player/Bus initial 100-percent policy.
            audio: crate::audio_bus::focus::SharedAudio::new(100, 100),
            scripted: HashMap::new(),
            ambient: None,
            firework_bursts: Vec::new(),
            firework_whistles: Vec::new(),
            auction_bid_sinks: std::array::from_fn(|_| None),
            auction_withdraw_sinks: std::array::from_fn(|_| None),
            next_handle: 1,
        }
    }

    fn mixer(&mut self) -> Option<&rodio::mixer::Mixer> {
        self.bus.combined_mixer()
    }
    pub fn bind_audio(&mut self, audio: crate::audio_bus::focus::SharedAudio) {
        self.audio = audio;
    }
    pub fn set_level(&mut self, effects: u8) {
        self.audio.set_effects(effects);
    }

    pub fn apply(&mut self, operation: Operation) -> Result<u32, String> {
        self.one_shots.retain(|sink| !sink.empty());
        self.ledger_cues.retain(|cue| !cue.sink.empty());
        self.ui_click_cues.retain(|cue| !cue.sink.empty());
        match operation {
            Operation::Reset => {
                for cue in self.ui_click_cues.drain(..) {
                    cue.sink.stop();
                }
                for cue in self.ledger_cues.drain(..) {
                    cue.sink.stop();
                }
                for sink in self.one_shots.drain(..) {
                    sink.stop();
                }
                if let Some((_, sink)) = &self.ambient {
                    sink.stop();
                }
                for sink in self.firework_bursts.drain(..).chain(self.firework_whistles.drain(..)) {
                    sink.stop();
                }
                for sink in self.auction_bid_sinks.iter_mut().filter_map(Option::take) {
                    sink.stop();
                }
                for sink in self.auction_withdraw_sinks.iter_mut().filter_map(Option::take) {
                    sink.stop();
                }
                Ok(0)
            }
            Operation::Targeted { actor_is_current } => {
                let samples = if actor_is_current {
                    self.assets.weapon.clone()
                } else {
                    self.assets.spell.clone()
                };
                let gain = self.audio.source_gain(100);
                if let Some(mixer) = self.mixer() {
                    let sink = rodio::Sink::connect_new(mixer);
                    sink.set_volume(gain);
                    sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                    self.one_shots.push(sink);
                }
                // The 41bf6c caller discards the returned 4ad760 handle.
                Ok(0)
            }
            Operation::AuctionAmbientCreate => {
                if self.ambient.is_some() {
                    return Err("auction ambient handle already active".into());
                }
                let samples = self.assets.auction_murmur.clone();
                let gain = self.audio.source_gain(100);
                let Some(mixer) = self.mixer() else {
                    return Ok(0);
                };
                let sink = rodio::Sink::connect_new(mixer);
                sink.set_volume(gain);
                // 464aa0 supplies playback words (1,1,0,1,100); the retained
                // ambient handle is released by admission or closure.
                sink.append(
                    rodio::buffer::SamplesBuffer::new(1, 22_050, samples).repeat_infinite(),
                );
                let handle = self.next_handle;
                self.next_handle = handle
                    .checked_add(1)
                    .ok_or("auction ambient handle overflow")?;
                self.ambient = Some((handle, sink));
                Ok(handle)
            }
            Operation::AuctionAmbientRelease(handle) => {
                if handle == 0 {
                    return Ok(0);
                }
                let Some((stored, sink)) = self.ambient.take() else {
                    return Err("auction ambient handle missing".into());
                };
                if stored != handle {
                    self.ambient = Some((stored, sink));
                    return Err(format!(
                        "auction ambient handle {handle} does not match {stored}"
                    ));
                }
                sink.stop();
                Ok(0)
            }
            Operation::CardOpen { positive } => {
                let samples = if positive {
                    self.assets.positive_pluck.clone()
                } else {
                    self.assets.negative_boing.clone()
                };
                let factor = self.audio.factor();
                if let Some(mixer) = self.bus.combined_mixer() {
                    let sink = rodio::Sink::connect_new(mixer);
                    // 41b436..41b467 supplies (1,0,0,0,100), with no retained handle.
                    // The source consumes the current factor through 4ab420/4abad0
                    // before play. Later preference stores do not rewrite it.
                    sink.set_volume(crate::music_gain::normalized_amplitude(
                        fixed_cue_normalized(factor, 100)));
                    sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                    self.one_shots.push(sink);
                }
                Ok(0)
            }
            Operation::ResultFirework(cue) => {
                use crate::fresh_city::result::fireworks::Cue;
                let (samples, source_level, whistle) = match cue {
                    Cue::Launch1 => (&self.assets.firework_launch1, 100, false),
                    Cue::Launch2 => (&self.assets.firework_launch2, 100, false),
                    Cue::EchoCrackle => (&self.assets.firework_echo_crackle, 100, false),
                    Cue::EchoSingle => (&self.assets.firework_echo_single, 100, false),
                    Cue::LaunchWhistle => (&self.assets.firework_whistle, 80, true),
                };
                let samples = samples.clone();
                let gain = self.audio.source_gain(source_level);
                let Some(mixer) = self.mixer().cloned() else { return Ok(0) };
                let pool = if whistle { &mut self.firework_whistles } else { &mut self.firework_bursts };
                // 453290 scans six burst handles, 454965 scans four whistles;
                // the original reuses a slot when handle+42c reports inactive.
                // Sink::empty() is the available backend end-of-playback signal.
                play_bounded(&mixer, pool, if whistle { 4 } else { 6 }, samples, gain);
                Ok(0)
            }
            Operation::AuctionActor(cue) => {
                use crate::auction_scene::ActorCue;
                match cue {
                    ActorCue::Bid { participant, character, suppressed } => {
                        let samples = self.assets.auction_voices.get(character as usize)
                            .ok_or("Auction bid character outside original roster")?[0].clone();
                        let gain = self.audio.source_gain(100);
                        let mixer = self.mixer().cloned();
                        let slot = self.auction_bid_sinks.get_mut(participant as usize)
                            .ok_or("Auction bid participant outside original roster")?;
                        replace_auction_voice(slot, mixer.as_ref(), samples, suppressed, gain);
                    }
                    ActorCue::Withdraw { participant, character, suppressed } => {
                        let samples = self.assets.auction_voices.get(character as usize)
                            .ok_or("Auction withdraw character outside original roster")?[2].clone();
                        let gain = self.audio.source_gain(100);
                        let mixer = self.mixer().cloned();
                        let slot = self.auction_withdraw_sinks.get_mut(participant as usize)
                            .ok_or("Auction withdraw participant outside original roster")?;
                        replace_auction_voice(slot, mixer.as_ref(), samples, suppressed, gain);
                    }
                    ActorCue::Win { character } => {
                        let samples = self.assets.auction_voices.get(character as usize)
                            .ok_or("Auction win character outside original roster")?[1].clone();
                        let gain = self.audio.source_gain(100);
                        if let Some(mixer) = self.mixer().cloned() {
                            let sink = rodio::Sink::connect_new(&mixer);
                            sink.set_volume(gain);
                            sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                            self.one_shots.push(sink);
                        }
                    }
                    ActorCue::Applause => {
                        let samples = self.assets.auction_applause.clone();
                        let gain = self.audio.source_gain(100);
                        if let Some(mixer) = self.mixer().cloned() {
                            let sink = rodio::Sink::connect_new(&mixer);
                            sink.set_volume(gain);
                            sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                            self.one_shots.push(sink);
                        }
                    }
                }
                Ok(0)
            }
            Operation::Script { name } => {
                if name.len() > 128 || !name.ends_with(".wav")
                    || !name.bytes().all(|byte| byte.is_ascii_alphanumeric()
                        || matches!(byte, b'_' | b'-' | b'.'))
                    || name.contains("..")
                {
                    return Err("script sound name is outside original high-sound catalog".into());
                }
                if !self.scripted.contains_key(&name) {
                    let samples = pcm16(&self.assets.script_directory.join(&name))?;
                    self.scripted.insert(name.clone(), samples);
                }
                let samples = self.scripted.get(&name).unwrap().clone();
                let gain = self.audio.source_gain(100);
                if let Some(mixer) = self.mixer() {
                    let sink = rodio::Sink::connect_new(mixer);
                    sink.set_volume(gain);
                    sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                    self.one_shots.push(sink);
                }
                Ok(0)
            }
            Operation::Ledger { sale } => {
                let samples = if sale { self.assets.sell.clone() } else { self.assets.buy.clone() };
                let factor = self.audio.factor();
                if let Some(mixer) = self.bus.combined_mixer() {
                    let cue = LedgerCue {
                        source_level: 60, // Both native 48a5e6 and 48a61d push 0x3c.
                        sink: rodio::Sink::connect_new(mixer),
                    };
                    cue.sink.set_volume(crate::music_gain::normalized_amplitude(
                        fixed_cue_normalized(factor, cue.source_level)));
                    // Gain precedes append: initial mute cannot leak full gain.
                    cue.sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                    self.ledger_cues.push(cue);
                }
                Ok(0)
            }
            Operation::UiLightClick => {
                let samples = self.assets.ui_light_click.clone();
                let factor = self.audio.factor();
                if let Some(mixer) = self.bus.combined_mixer() {
                    let cue = UiClickCue {
                        // All three widget sites push 0x64 before 4ad760.
                        source_level: 100,
                        sink: rodio::Sink::connect_new(mixer),
                    };
                    cue.sink.set_volume(crate::music_gain::normalized_amplitude(
                        fixed_cue_normalized(factor, cue.source_level)));
                    // Native +1c precedes play +10; never append at full gain.
                    cue.sink.append(rodio::buffer::SamplesBuffer::new(1, 22_050, samples));
                    self.ui_click_cues.push(cue);
                }
                Ok(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_firework_voice_caps_and_whistle_gain_without_device() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 22_050);
        mixer.add(rodio::source::Zero::new(1, 22_050));
        let mut bursts = Vec::new();
        for _ in 0..6 {
            assert!(play_bounded(&mixer, &mut bursts, 6, vec![0.05; 64], 1.));
        }
        assert!(!play_bounded(&mixer, &mut bursts, 6, vec![0.05; 64], 1.));
        assert_eq!(bursts.len(), 6);
        assert!(bursts.iter().all(|sink| sink.volume() == 1.));
        assert!((0..512).filter_map(|_| output.next()).any(|sample| sample > 0.));
        for _ in 0..512 { output.next(); }
        assert!(play_bounded(&mixer, &mut bursts, 6, vec![0.05; 64], 1.));
        assert_eq!(bursts.len(), 1, "finished retained handles must recycle");

        let (mixer, mut output) = rodio::mixer::mixer(1, 22_050);
        mixer.add(rodio::source::Zero::new(1, 22_050));
        let mut whistles = Vec::new();
        for _ in 0..4 {
            assert!(play_bounded(&mixer, &mut whistles, 4, vec![0.125; 64], 0.8));
        }
        assert!(!play_bounded(&mixer, &mut whistles, 4, vec![0.125; 64], 0.8));
        assert!(whistles.iter().all(|sink| sink.volume() == 0.8));
        assert!((0..512).filter_map(|_| output.next()).any(|sample| sample > 0.));
    }

    #[test]
    fn reset_silences_sound_and_preserves_auction_release_order() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 22_050);
        mixer.add(rodio::source::Zero::new(1, 22_050));
        for _ in 0..512 {
            assert_eq!(output.next(), Some(0.));
        }
        let samples = vec![0.5; 64];
        let make_sink = || {
            let sink = rodio::Sink::connect_new(&mixer);
            sink.append(
                rodio::buffer::SamplesBuffer::new(1, 22_050, samples.clone()).repeat_infinite(),
            );
            sink
        };
        let assets = Assets {
            weapon: samples.clone(),
            spell: samples.clone(),
            auction_murmur: samples.clone(),
            positive_pluck: samples.clone(),
            negative_boing: samples.clone(),
            firework_launch1: samples.clone(),
            firework_launch2: samples.clone(),
            firework_echo_crackle: samples.clone(),
            firework_echo_single: samples.clone(),
            firework_whistle: samples.clone(),
            auction_voices: vec![[samples.clone(), samples.clone(), samples.clone()]; 10],
            auction_applause: samples.clone(),
            buy: samples.clone(),
            sell: samples.clone(),
            ui_light_click: samples.clone(),
            script_directory: PathBuf::new(),
        };
        let mut player = Player::new(assets);
        player.one_shots.push(make_sink());
        player.ambient = Some((7, make_sink()));
        player.firework_bursts.push(make_sink());
        player.firework_whistles.push(make_sink());
        player.auction_bid_sinks[0] = Some(make_sink());
        player.auction_withdraw_sinks[0] = Some(make_sink());
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
        assert!(player.one_shots.is_empty());
        assert!(player.firework_bursts.is_empty());
        assert!(player.firework_whistles.is_empty());
        assert!(player.auction_bid_sinks.iter().all(Option::is_none));
        assert!(player.auction_withdraw_sinks.iter().all(Option::is_none));
        assert_eq!(player.ambient.as_ref().map(|(handle, _)| *handle), Some(7));
        assert_eq!(
            player.apply(Operation::AuctionAmbientRelease(7)).unwrap(),
            0
        );
        assert!(player.ambient.is_none());
    }

    #[test]
    fn auction_withdraw_releases_prior_voice_before_sound_gate() {
        let (mixer, mut output) = rodio::mixer::mixer(1, 22_050);
        mixer.add(rodio::source::Zero::new(1, 22_050));
        let previous = rodio::Sink::connect_new(&mixer);
        previous.append(rodio::buffer::SamplesBuffer::new(1, 22_050,
            vec![0.5; 64]).repeat_infinite());
        let mut slot = Some(previous);
        assert!((0..512).filter_map(|_| output.next()).any(|sample| sample > 0.));
        replace_auction_voice(&mut slot, Some(&mixer), vec![0.25; 64], true, 1.);
        assert!(slot.is_none(), "native 4647a5 releases even when +858 suppresses the new voice");
        let silent: Vec<_> = (0..8192).map(|_| output.next()).collect();
        assert!(silent[silent.len()-1024..].iter().all(|sample| *sample == Some(0.)));
        replace_auction_voice(&mut slot, Some(&mixer), vec![0.25; 64], false, 1.);
        assert!(slot.is_some());
        assert!((0..512).filter_map(|_| output.next()).any(|sample| sample > 0.));
    }

    #[test]
    fn original_targeted_and_auction_sound_assets() {
        let (Some(exe), Some(suncols)) = (
            std::env::var_os("OPENMT2001_EXE"),
            std::env::var_os("OPENMT2001_SUNCOLS"),
        ) else {
            return;
        };
        let exe = std::fs::read(exe).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let installer = Path::new(&suncols).parent().unwrap().parent().unwrap();
        let assets = Assets::load(&image, installer).unwrap();
        assert_eq!(assets.weapon.len(), 66_338);
        assert_eq!(assets.spell.len(), 58_768);
        assert_eq!(assets.auction_murmur.len(), 77_543);
        assert_eq!(assets.positive_pluck.len(), 22_435);
        assert_eq!(assets.negative_boing.len(), 12_436);
        assert_eq!(assets.firework_launch1.len(), 33_460);
        assert_eq!(assets.firework_launch2.len(), 18_859);
        assert_eq!(assets.firework_echo_crackle.len(), 71_417);
        assert_eq!(assets.firework_echo_single.len(), 41_307);
        assert_eq!(assets.firework_whistle.len(), 39_388);
        assert_eq!(assets.auction_voices.len(), 10);
        assert!(assets.auction_voices.iter().all(|pair| pair.iter().all(|samples| !samples.is_empty())));
        assert!(!assets.auction_applause.is_empty());
        // Branch-specific filename pushes converge on 453c1c; 4549a9 is
        // the kind-3 emitter whistle. All are original high-sound assets.
        for (address, name) in [
            (0x453546, 0x53a1b0u32),
            (0x453754, 0x53a1a0),
            (0x45395e, 0x53a190),
            (0x453c12, 0x53a184),
            (0x45499f, 0x53a1bc),
        ] {
            let push = image.at(address, 5).unwrap();
            assert_eq!(push[0], 0x68);
            assert_eq!(u32::from_le_bytes(push[1..5].try_into().unwrap()), name);
        }
        for call_address in [0x453c1c, 0x4549a9] {
            let call = image.at(call_address, 5).unwrap();
            assert_eq!(call[0], 0xe8);
            assert_eq!(
                (call_address + 5).wrapping_add_signed(i32::from_le_bytes(call[1..5].try_into().unwrap())),
                0x4ad760
            );
        }
        assert!(card_open_positive(&image, 1).unwrap());
        assert!(card_open_positive(&image, 65).unwrap());
        assert!(!card_open_positive(&image, 10).unwrap());
        // 41b443 compares the fourth 32-bit word of 20-byte card rows.
        assert_eq!(image.at(0x41b436, 6).unwrap(), &[0x6a, 0x64, 0x6a, 0, 0x6a, 0]);
        assert_eq!(image.at(0x41b443, 8).unwrap(), &[0x83, 0x3c, 0x85, 0x14, 0x42, 0x53, 0x00, 6]);
        for (push, name) in [(0x41b44d, 0x5348f8u32), (0x41b45d, 0x5348e4)] {
            let instruction = image.at(push, 5).unwrap();
            assert_eq!(instruction[0], 0x68);
            assert_eq!(u32::from_le_bytes(instruction[1..5].try_into().unwrap()), name);
        }
        for call_address in [0x41b457, 0x41b467] {
            let call = image.at(call_address, 5).unwrap();
            assert_eq!(call[0], 0xe8);
            assert_eq!(
                (call_address + 5).wrapping_add_signed(i32::from_le_bytes(call[1..5].try_into().unwrap())),
                0x4ad760
            );
        }
        assert!(!assets.buy.is_empty() && !assets.sell.is_empty());
        // The supplied RIFF has 562 PCM data bytes (281 samples) followed by
        // a separate LIST chunk; playback must stop at the data chunk.
        assert_eq!(assets.ui_light_click.len(), 281);
        for address in [0x42f33e, 0x42ca30, 0x4433e8] {
            let push = image.at(address, 5).unwrap();
            assert_eq!(push[0], 0x68);
            assert_eq!(u32::from_le_bytes(push[1..5].try_into().unwrap()), 0x535870);
        }
        for (address, name_address) in [
            (0x48a5f0, 0x541cc8u32), (0x48a627, 0x541cb4),
        ] {
            let push = image.at(address, 5).unwrap();
            assert_eq!(push[0], 0x68);
            assert_eq!(u32::from_le_bytes(push[1..5].try_into().unwrap()), name_address);
        }
        assert!(pcm16(&assets.script_directory.join("harpstrum.wav")).unwrap().len() > 20_000);
        // 464abb names auction murmur and 464ac5 calls 4ad760.
        assert_eq!(image.at(0x464abb, 1).unwrap(), &[0x68]);
        assert_eq!(
            u32::from_le_bytes(image.at(0x464abc, 4).unwrap().try_into().unwrap()),
            0x53c518
        );
        let call = image.at(0x464ac5, 5).unwrap();
        assert_eq!(call[0], 0xe8);
        assert_eq!(
            0x464aca_u32.wrapping_add_signed(i32::from_le_bytes(call[1..5].try_into().unwrap())),
            0x4ad760
        );
    }
}

#[cfg(test)]
mod portable_pcm_tests {
    use super::pcm16;
    use std::path::PathBuf;

    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn original_pcm_loads_despite_filename_case() {
        let base = std::env::var_os("OPENMT2001_TEST_SCRATCH")
            .map(PathBuf::from).unwrap_or_else(|| PathBuf::from("work/test-scratch"));
        std::fs::create_dir_all(&base).unwrap();
        let root = base.join(format!("pcm-case-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let _scratch = Scratch(root.clone());
        let mut wav = vec![0u8; 46];
        wav[..4].copy_from_slice(b"RIFF");
        wav[4..8].copy_from_slice(&38u32.to_le_bytes());
        wav[8..12].copy_from_slice(b"WAVE");
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1u16.to_le_bytes());
        wav[22..24].copy_from_slice(&1u16.to_le_bytes());
        wav[24..28].copy_from_slice(&22050u32.to_le_bytes());
        wav[28..32].copy_from_slice(&44100u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&2u32.to_le_bytes());
        wav[44..46].copy_from_slice(&16384i16.to_le_bytes());
        std::fs::write(root.join("positivepluck.wav"), wav).unwrap();
        assert_eq!(pcm16(&root.join("Positivepluck.wav")).unwrap(), vec![0.5]);
        assert!(pcm16(&root.join("absent.wav")).is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sound_lookup_prefers_exact_name_and_rejects_ambiguous_fold() {
        let base = std::env::var_os("OPENMT2001_TEST_SCRATCH")
            .map(PathBuf::from).unwrap_or_else(|| PathBuf::from("work/test-scratch"));
        std::fs::create_dir_all(&base).unwrap();
        let root = base.join(format!("pcm-fold-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let _scratch = Scratch(root.clone());
        for name in ["cue.wav", "Cue.wav"] { std::fs::write(root.join(name), []).unwrap(); }
        assert_eq!(crate::transport_audio::original_case_path(&root.join("cue.wav")).unwrap(), root.join("cue.wav"));
        assert!(crate::transport_audio::original_case_path(&root.join("CUE.WAV")).unwrap_err().contains("ambiguous"));
    }
}


// Bounded software-mixer regressions; no hardware output is opened.
#[cfg(test)]
mod ledger_gain_tests {
    include!("gameplay_sound_ledger_gain_tests.rs");
}

// UI-click-only actual-owner regressions; no device is opened.
#[cfg(test)]
mod ui_click_gain_tests {
    include!("gameplay_sound_ui_click_gain_tests.rs");
}

#[cfg(test)]
mod card_open_gain_tests {
    include!("gameplay_sound_card_open_gain_tests.rs");
}

#[cfg(test)]
mod focus_tests {
    include!("gameplay_sound_focus_tests.rs");
}
