//! UI-owned moving transport audio from the original v1.4 4aefc0 dispatcher.
//! The worker supplies the retained route sound call; no audio handle enters the
//! shared simulation. Source WAVs are read in place from the user's installer.
pub mod block_audio;
use openmt2001::{
    pe::PeImage,
    traffic::{RouteSound, scene::EnvironmentCall},
};
use rodio::Source;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

#[derive(Clone, Debug)]
pub enum Operation {
    /// Original 4af140 clears the retained city sound handles when leaving
    /// gameplay for a result or an auction transition.
    Reset,
    Route(RouteSound),
    /// Original 4af860 consumes 650330 and three ranked 63f9c0 rows,
    /// produced together by road::audio::Proximity.
    RoadProximity {
        reciprocal_sum: f32,
        nearest: [[u32; 3]; 3],
    },
    /// Original 4afa40 weights the 52 category resident counts by camera
    /// distance before starting or stopping the busy-people city bed.
    ResidentCrowd(f32),
    /// Original 44ec50 calls 4addd0/4ae2b0/4ae1f0 with the current sound
    /// position. The Host computes distance from its retained camera.
    Environment {
        call: EnvironmentCall,
        distance: f32,
    },
    /// Original 4b0750 keeps the two nearest construction positions per
    /// stage; 4adb10/4afcde present their four sound beds.
    Construction([[Option<ConstructionSource>; 2]; 4]),
    /// Original 4afd65 presents block ambience after the construction beds.
    Blocks(Box<block_audio::Snapshot>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstructionSource {
    pub distance: f32,
    pub position: [f32; 3],
}

/// The native 4b0750 queue inserts after equal distances and discards any
/// candidate beyond the two nearest entries. The camera is 44d060's global.
pub fn insert_construction_source(
    groups: &mut [[Option<ConstructionSource>; 2]; 4],
    group: usize,
    position: [f32; 3],
    camera: [f32; 3],
) -> Result<(), String> {
    let entries = groups
        .get_mut(group)
        .ok_or("construction sound group outside native pool")?;
    // 44d060 keeps the component differences and square root on the x87
    // stack until 4b0750 stores the winning distance as f32.
    let delta: [f64; 3] =
        std::array::from_fn(|axis| f64::from(position[axis]) - f64::from(camera[axis]));
    let distance = ((delta[2] * delta[2] + delta[1] * delta[1]) + delta[0] * delta[0]).sqrt();
    if !distance.is_finite() {
        return Err("construction sound distance is nonfinite".into());
    }
    let source = ConstructionSource {
        distance: distance as f32,
        position,
    };
    if entries[0].is_none_or(|entry| distance < f64::from(entry.distance)) {
        entries[1] = entries[0];
        entries[0] = Some(source);
    } else if entries[1].is_none_or(|entry| distance < f64::from(entry.distance)) {
        entries[1] = Some(source);
    }
    Ok(())
}

/// Original 4afa73..4afb28. Native f32 stores occur once per category for
/// both the clipped distance and the accumulated weight. This uses the
/// authoritative +e0/+e4 centers and +88 signed count in each live row.
pub fn resident_crowd_weight(categories: &[[u8; 0x17c]; 52], camera: [f32; 3]) -> f32 {
    let mut weight = 0f32;
    for category in categories {
        let x = f32::from_le_bytes(category[0xe0..0xe4].try_into().unwrap());
        let z = f32::from_le_bytes(category[0xe4..0xe8].try_into().unwrap());
        let count = i32::from_le_bytes(category[0x88..0x8c].try_into().unwrap());
        let dx = f64::from(camera[0]) - f64::from(x);
        let dz = f64::from(camera[2]) - f64::from(z);
        let y = f64::from(camera[1]);
        let distance = ((dz * dz + dx * dx) + y * y).sqrt() + 1.;
        let clipped = if distance > 500. {
            5000f32
        } else if distance < 100. || distance.is_nan() {
            100f32
        } else {
            distance as f32
        };
        weight = (f64::from(weight) + 50. / f64::from(clipped) * f64::from(count)) as f32;
    }
    weight
}

fn crowd_level(weight: f32) -> Option<i32> {
    (weight > 30.).then(|| (f64::from(weight) - 30.).trunc().clamp(0., 65.) as i32)
}
#[cfg(test)]
fn crowd_gain(weight: f32) -> Option<f32> { crowd_level(weight).map(|q| q as f32 / 100.) }

pub struct Request {
    pub operation: Operation,
}

pub fn submit(sender: &Option<mpsc::Sender<Request>>, operation: Operation) -> Result<(), String> {
    // Headless Host validation runs the same simulation without a UI owner.
    let Some(sender) = sender else { return Ok(()) };
    // Inspection retirement drops its audio receiver before the worker's
    // 16 ms loop notices its session channel closed. Sound presentation must
    // not turn that ordinary teardown frame into a simulation failure.
    let _ = sender.send(Request { operation });
    Ok(())
}

#[derive(Clone)]
pub struct Assets {
    train_motion: Vec<f32>,
    train_toot: Vec<f32>,
    road_ambient: Vec<f32>,
    road_motor: Vec<f32>,
    resident_crowd: Vec<f32>,
    near_aircraft: Vec<f32>,
    ship_engine: Vec<f32>,
    ship_horn: Vec<f32>,
    ocean_liner: Vec<f32>,
    crane_start: Vec<f32>,
    crane_end: Vec<f32>,
    crane_lift: Vec<f32>,
    crane_loop: Vec<f32>,
    construction_scaffold: Vec<f32>,
    construction_saw: Vec<f32>,
    construction_hammer: Vec<f32>,
    block_factory: Vec<f32>,
    block_school: Vec<f32>,
    block_park_one: Vec<f32>,
    block_park_two: Vec<f32>,
    block_owl: Vec<f32>,
}

fn checked_name(
    image: &PeImage<'_>,
    address: u32,
    expected: &'static str,
) -> Result<&'static str, String> {
    let actual = image.at(address, expected.len() + 1)?;
    if actual != [expected.as_bytes(), &[0]].concat() {
        return Err(format!(
            "v1.4 transport sound string changed at {address:08x}"
        ));
    }
    Ok(expected)
}

fn pcm16(path: &Path) -> Result<Vec<f32>, String> {
    let path = original_case_path(path)?;
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
            "{}: unsupported original transport PCM format",
            path.display()
        ));
    }
    let length = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    if length == 0 || !length.is_multiple_of(2) || bytes.len() < 44 + length {
        return Err(format!(
            "{}: invalid original transport PCM length",
            path.display()
        ));
    }
    Ok(bytes[44..44 + length]
        .chunks_exact(2)
        .map(|sample| f32::from(i16::from_le_bytes(sample.try_into().unwrap())) / 32768.)
        .collect())
}

/// The supplied installer retains the original mixed-case WAV filenames,
/// while v1.4 requests some of them through lowercase PE strings. Resolve the
/// actual directory entry so the same private assets load on case-sensitive
/// Linux/macOS filesystems as well as Windows.
pub(crate) fn original_case_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() { return Ok(path.to_path_buf()); }
    let parent = path.parent().ok_or("original sound path has no parent")?;
    let requested = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("original sound filename is not UTF-8")?;
    let mut folded = None;
    for entry in
        std::fs::read_dir(parent).map_err(|error| format!("{}: {error}", parent.display()))?
    {
        let entry = entry.map_err(|error| format!("{}: {error}", parent.display()))?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name == requested {
            return Ok(entry.path());
        }
        if name.eq_ignore_ascii_case(requested) {
            if folded.is_some() {
                return Err(format!(
                    "{}: ambiguous case-folded original sound",
                    path.display()
                ));
            }
            folded = Some(entry.path());
        }
    }
    folded.ok_or_else(|| format!("{}: original sound missing", path.display()))
}

impl Assets {
    pub fn load(image: &PeImage<'_>, installer: &Path) -> Result<Self, String> {
        let directory = installer.join("Standard/sound/high");
        for (address, name) in [
            (0x545220, "Block_Sounds\\ambi_factory_1.wav"),
            (0x5451fc, "Block_Sounds\\SchoolPlayground.wav"),
            (0x544ff8, "block_sounds//ambi_park_1.wav"),
            (0x54508c, "block_sounds//ambi_park_2.wav"),
            (0x545018, "block_sounds//owl6.wav"),
        ] {
            checked_name(image, address, name)?;
        }
        let blocks = directory.join("Block_Sounds");
        Ok(Self {
            train_motion: pcm16(&directory.join(checked_name(
                image,
                0x54513c,
                "ambi_any_traff_trainmotion30.wav",
            )?))?,
            train_toot: pcm16(&directory.join(checked_name(
                image,
                0x545240,
                "spot_any_train_toot30.wav",
            )?))?,
            road_ambient: pcm16(&directory.join(checked_name(
                image,
                0x5450f8,
                "ambi_day_traff_busycity.wav",
            )?))?,
            road_motor: pcm16(&directory.join(checked_name(image, 0x5450ec, "motor2.wav")?))?,
            resident_crowd: pcm16(&directory.join(checked_name(
                image,
                0x5450d0,
                "ambi_day_peop_busycity.wav",
            )?))?,
            near_aircraft: pcm16(&directory.join(checked_name(
                image,
                0x54511c,
                "ambi_any_traff_carengine006.wav",
            )?))?,
            ship_engine: pcm16(&directory.join(checked_name(image, 0x5451a0, "shipengine.wav")?))?,
            ship_horn: pcm16(&directory.join(checked_name(image, 0x5451ec, "shiphorn.wav")?))?,
            ocean_liner: pcm16(&directory.join(checked_name(image, 0x5451dc, "oceanlin.wav")?))?,
            crane_start: pcm16(&directory.join(checked_name(image, 0x545190, "cranestart.wav")?))?,
            crane_end: pcm16(&directory.join(checked_name(image, 0x545180, "craneend.wav")?))?,
            crane_lift: pcm16(&directory.join(checked_name(image, 0x545170, "cranelift.wav")?))?,
            crane_loop: pcm16(&directory.join(checked_name(image, 0x545160, "craneloop.wav")?))?,
            construction_scaffold: pcm16(&directory.join(checked_name(
                image,
                0x545050,
                "ambi_any_altbuild_scaffold.wav",
            )?))?,
            construction_saw: pcm16(&directory.join(checked_name(
                image,
                0x545070,
                "rand_any_altbuild_saw.wav",
            )?))?,
            construction_hammer: pcm16(&directory.join(checked_name(
                image,
                0x545030,
                "rand_any_altbuild_hammer002.wav",
            )?))?,
            block_factory: pcm16(&blocks.join("ambi_factory_1.wav"))?,
            block_school: pcm16(&blocks.join("SchoolPlayground.wav"))?,
            block_park_one: pcm16(&blocks.join("ambi_park_1.wav"))?,
            block_park_two: pcm16(&blocks.join("ambi_park_2.wav"))?,
            block_owl: pcm16(&blocks.join("owl6.wav"))?,
        })
    }
}

pub struct Player {
    assets: Assets,
    bus: crate::audio_bus::Bus,
    audio: crate::audio_bus::focus::SharedAudio,
    train_motion: Option<rodio::Sink>,
    train_toot: Option<rodio::Sink>,
    road_ambient: Option<rodio::Sink>,
    road_motors: [Option<rodio::Sink>; 3],
    resident_crowd: Option<rodio::Sink>,
    near_aircraft: Option<rodio::Sink>,
    ship_engine: Option<rodio::Sink>,
    ship_horn: Option<rodio::Sink>,
    crane_start: Option<rodio::Sink>,
    crane_end: Option<rodio::Sink>,
    crane_lift: Option<rodio::Sink>,
    crane_loop: Option<rodio::Sink>,
    construction: [Option<rodio::Sink>; 8],
    block_sinks: [Option<rodio::Sink>; 52],
    block_controller: Option<block_audio::Controller>,
    pending_ship_random: u32,
    interface_mode: u32,
}

impl Player {
    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            bus: crate::audio_bus::Bus::new(),
            audio: crate::audio_bus::focus::SharedAudio::new(100, 100),
            train_motion: None,
            train_toot: None,
            road_ambient: None,
            road_motors: std::array::from_fn(|_| None),
            resident_crowd: None,
            near_aircraft: None,
            ship_engine: None,
            ship_horn: None,
            crane_start: None,
            crane_end: None,
            crane_lift: None,
            crane_loop: None,
            construction: std::array::from_fn(|_| None),
            block_sinks: std::array::from_fn(|_| None),
            block_controller: None,
            pending_ship_random: 0,
            interface_mode: 0,
        }
    }

    fn mixer(&mut self) -> Option<&rodio::mixer::Mixer> {
        self.bus.combined_mixer()
    }
    pub fn bind_audio(&mut self, audio: crate::audio_bus::focus::SharedAudio) { self.audio = audio; }
    pub fn set_level(&mut self, effects: u8) {
        self.audio.set_effects(effects);
    }
    pub fn set_interface_mode(&mut self, mode: u32) {
        self.interface_mode = mode;
    }

    fn loop_sink(mixer: &rodio::mixer::Mixer, samples: &[f32], gain: f32) -> rodio::Sink {
        let sink = rodio::Sink::connect_new(mixer);
        sink.set_volume(gain);
        sink.append(
            rodio::buffer::SamplesBuffer::new(1, 22_050, samples.to_vec()).repeat_infinite(),
        );
        sink
    }

    fn one_shot(mixer: &rodio::mixer::Mixer, samples: &[f32], gain: f32) -> rodio::Sink {
        let sink = rodio::Sink::connect_new(mixer);
        sink.set_volume(gain);
        sink.append(rodio::buffer::SamplesBuffer::new(
            1,
            22_050,
            samples.to_vec(),
        ));
        sink
    }

    pub fn apply(&mut self, operation: Operation) {
        match operation {
            Operation::Reset => self.reset(),
            Operation::Route(call) => self.route(call),
            Operation::RoadProximity {
                reciprocal_sum,
                nearest,
            } => self.road(reciprocal_sum, nearest),
            Operation::ResidentCrowd(weight) => self.crowd(weight),
            Operation::Environment { call, distance } => self.environment(call, distance),
            Operation::Construction(groups) => self.construction(groups),
            Operation::Blocks(snapshot) => self.blocks(*snapshot),
        }
    }

    fn reset(&mut self) {
        for handle in [
            &mut self.train_motion,
            &mut self.train_toot,
            &mut self.road_ambient,
            &mut self.resident_crowd,
            &mut self.near_aircraft,
            &mut self.ship_engine,
            &mut self.ship_horn,
            &mut self.crane_start,
            &mut self.crane_end,
            &mut self.crane_lift,
            &mut self.crane_loop,
        ] {
            stop(handle);
        }
        for handle in &mut self.construction {
            stop(handle);
        }
        for handle in &mut self.road_motors {
            stop(handle);
        }
        for handle in &mut self.block_sinks {
            stop(handle);
        }
    }

    fn blocks(&mut self, snapshot: block_audio::Snapshot) {
        let playing = std::array::from_fn(|index| {
            self.block_sinks[index]
                .as_ref()
                .is_some_and(|sink| !sink.empty())
        });
        let controller = self.block_controller.get_or_insert_with(|| {
            block_audio::Controller::new(snapshot.initial_random_state, snapshot.previous_tick)
        });
        for _ in 0..std::mem::take(&mut self.pending_ship_random) {
            controller.ship_random();
        }
        let actions = controller.advance(snapshot, self.interface_mode, playing);
        let sources = controller.sources;
        for action in actions {
            match action {
                block_audio::Action::Stop(category) => stop(&mut self.block_sinks[category]),
                block_audio::Action::Play { category, kind } => {
                    let source = sources[category];
                    if !source.distance.is_finite() || source.distance > 300. {
                        stop(&mut self.block_sinks[category]);
                        continue;
                    }
                    if !kind.looping() {
                        stop(&mut self.block_sinks[category]);
                    }
                    if self.block_sinks[category].is_none() {
                        let samples = match kind {
                            block_audio::Kind::Factory => self.assets.block_factory.clone(),
                            block_audio::Kind::School => self.assets.block_school.clone(),
                            block_audio::Kind::ParkOne => self.assets.block_park_one.clone(),
                            block_audio::Kind::ParkTwo => self.assets.block_park_two.clone(),
                            block_audio::Kind::Owl => self.assets.block_owl.clone(),
                        };
                        if let Some(mixer) = self.mixer() {
                            self.block_sinks[category] = Some(if kind.looping() {
                                Self::loop_sink(mixer, &samples, 0.)
                            } else {
                                Self::one_shot(mixer, &samples, 0.)
                            });
                        }
                    }
                    if let Some(sink) = &self.block_sinks[category] {
                        sink.set_volume(self.audio.source_gain(source.level(snapshot.camera)));
                    }
                }
            }
        }
    }

    fn construction(&mut self, groups: [[Option<ConstructionSource>; 2]; 4]) {
        for (group, candidates) in groups.into_iter().enumerate() {
            for (slot, candidate) in candidates.into_iter().enumerate() {
                let index = group * 2 + slot;
                let Some(candidate) = candidate else {
                    if let Some(sink) = self.construction[index].take() {
                        sink.stop();
                    }
                    continue;
                };
                if !candidate.distance.is_finite() || candidate.distance >= 250. {
                    // 4ad8b0 retains an existing handle and silences it while
                    // its ranked source is beyond the audible radius.
                    if let Some(sink) = &self.construction[index] {
                        sink.set_volume(0.);
                    }
                    continue;
                }
                if self.construction[index].is_none() {
                    let samples = match group {
                        0 | 1 => self.assets.construction_scaffold.clone(),
                        2 => self.assets.construction_saw.clone(),
                        _ => self.assets.construction_hammer.clone(),
                    };
                    if let Some(mixer) = self.mixer() {
                        self.construction[index] = Some(Self::loop_sink(mixer, &samples, 0.));
                    }
                }
                if let Some(sink) = &self.construction[index] {
                    sink.set_volume(self.audio.source_gain(construction_fallback_level(candidate.distance)));
                }
            }
        }
    }

    fn route(&mut self, call: RouteSound) {
        // 4aefc0 index 0 dispatches to 4af6e0. The tier-0 aircraft bed uses
        // the original car-engine WAV; its distance is refreshed every 5 ticks.
        if call.id == 0 {
            if !(call.distance.is_finite() && call.distance < 500.) {
                if let Some(sink) = self.near_aircraft.take() {
                    sink.stop();
                }
                return;
            }
            if self.near_aircraft.is_none() {
                let samples = self.assets.near_aircraft.clone();
                if let Some(mixer) = self.mixer() {
                    self.near_aircraft = Some(Self::loop_sink(mixer, &samples, 0.));
                }
                return;
            }
            if let Some(sink) = &self.near_aircraft {
                // 4af6e0 sends a signed, truncated gain to its backend;
                // Rodio's nonnegative volume uses the audible portion.
                sink.set_volume(self.audio.source_gain(aircraft_volume(call.distance)));
            }
            return;
        }
        // Existing route coverage is 7/8/11. The id11 motion stop remains a
        // separate lifetime limitation (native silences that retained handle).
        // Toot volume is refreshed on each supported non-id8 in-range call.
        if !matches!(call.id, 7 | 8 | 11) {
            return;
        }
        if !(call.distance.is_finite() && call.distance <= 500.) {
            if let Some(sink) = self.train_motion.take() {
                sink.stop();
            }
            // 4af5e0..4af5eb silences the existing toot without restarting,
            // releasing, or rewinding it. Factor stores alone do not do this.
            if let Some(sink) = &self.train_toot {
                sink.set_volume(self.audio.source_gain(0));
            }
            return;
        }
        if call.id == 8 {
            let toot_gain = self.audio.source_gain(train_level(call.distance, 0.05, false));
            let samples = self.assets.train_toot.clone();
            if let Some(mixer) = self.mixer() {
                let sink = Self::one_shot(mixer, &samples, toot_gain);
                // 4af430 truncates 70 - (distance + 1) * binary32(0.05).
                sink.set_volume(toot_gain);
                self.train_toot = Some(sink);
            }
        } else if let Some(sink) = &self.train_toot {
            // 4af502..4af50c precedes id7 motion work and consumes the live
            // effects factor with the freshly computed toot source word.
            sink.set_volume(self.audio.source_gain(train_level(call.distance, 0.05, false)));
        }
        if call.id == 11 {
            if let Some(sink) = self.train_motion.take() {
                sink.stop();
            }
            return;
        }
        if call.id == 7 && self.train_motion.is_none() {
            let samples = self.assets.train_motion.clone();
            if let Some(mixer) = self.mixer() {
                self.train_motion = Some(Self::loop_sink(mixer, &samples, 0.));
            }
        }
        if let Some(sink) = &self.train_motion {
            // 4af430 stores distance + 1 as f32 before this second gain.
            sink.set_volume(self.audio.source_gain(train_level(call.distance, 0.1, true)));
            if call.id == 7 {
                let speed = f32::from_bits(call.arguments[0]);
                // Original frequency: speed * 0.25 * 22050 + 11025 Hz.
                if speed.is_finite() {
                    sink.set_speed((0.5 + speed * 0.25).max(0.01));
                }
            }
        }
    }

    fn road(&mut self, reciprocal_sum: f32, nearest: [[u32; 3]; 3]) {
        // With the live manager's +160 flag set, 4af87e retains the road bed
        // unchanged in strategic mode 3; both modes release ranked motors.
        if self.interface_mode == 3 {
            for handle in &mut self.road_motors {
                stop(handle);
            }
            return;
        }
        if self.interface_mode == 1 {
            stop(&mut self.road_ambient);
            for handle in &mut self.road_motors {
                stop(handle);
            }
            return;
        }
        // 4af860 keeps the busy-city traffic bed and updates its gain from
        // the reciprocal road-vehicle proximity sum (650330).
        if !reciprocal_sum.is_finite() {
            return;
        }
        if self.road_ambient.is_none() {
            let samples = self.assets.road_ambient.clone();
            if let Some(mixer) = self.mixer() {
                self.road_ambient = Some(Self::loop_sink(mixer, &samples, 0.));
            }
        }
        if let Some(sink) = &self.road_ambient {
            sink.set_volume(self.audio.source_gain(road_level(reciprocal_sum)));
        }
        let needs_mixer = nearest.iter().enumerate().any(|(slot, row)| {
            f32::from_bits(row[0]).is_finite()
                && f32::from_bits(row[0]) < 100.
                && self.road_motors[slot].is_none()
        });
        let mixer = if needs_mixer {
            self.mixer().cloned()
        } else {
            None
        };
        self.update_road_motors(nearest, mixer.as_ref());
    }

    fn update_road_motors(&mut self, nearest: [[u32; 3]; 3], mixer: Option<&rodio::mixer::Mixer>) {
        for (slot, row) in nearest.into_iter().enumerate() {
            let distance = f32::from_bits(row[0]);
            if !distance.is_finite() || distance >= 100. {
                stop(&mut self.road_motors[slot]);
                continue;
            }
            if self.road_motors[slot].is_none() {
                if let Some(mixer) = mixer {
                    self.road_motors[slot] = Some(Self::loop_sink(mixer, &self.assets.road_motor, 0.));
                }
                // 4af9b5 jumps past both setters on the creation frame.
                continue;
            }
            if let Some(sink) = &self.road_motors[slot] {
                sink.set_volume(self.audio.source_gain(road_motor_volume(distance)));
                let speed = f32::from_bits(row[2]);
                if speed.is_finite() {
                    sink.set_speed((road_motor_frequency(speed) as f32 / 22_050.).max(0.01));
                }
            }
        }
    }

    fn crowd(&mut self, weight: f32) {
        // 4afa4f/4afa5d release this bed in strategic and inspection modes.
        if matches!(self.interface_mode, 1 | 3) {
            stop(&mut self.resident_crowd);
            return;
        }
        // 4afb28 compares the stored total to 30, then retains a looping
        // ambi_day_peop_busycity.wav handle with integer volume capped at 65.
        let Some(level) = crowd_level(weight) else {
            stop(&mut self.resident_crowd);
            return;
        };
        let gain = self.audio.source_gain(level);
        if self.resident_crowd.is_none() {
            let samples = self.assets.resident_crowd.clone();
            if let Some(mixer) = self.mixer() {
                self.resident_crowd = Some(Self::loop_sink(mixer, &samples, 0.));
            }
        }
        if let Some(sink) = &self.resident_crowd {
            sink.set_volume(gain);
        }
    }

    fn environment(&mut self, call: EnvironmentCall, distance: f32) {
        if !distance.is_finite() {
            return;
        }
        match call {
            EnvironmentCall::Ship(model_choice) => {
                if distance > 800. || matches!(self.interface_mode, 1 | 3) {
                    stop(&mut self.ship_engine);
                    stop(&mut self.ship_horn);
                    return;
                }
                // 4adee2/4ae072 share the park sound manager RNG, including
                // ship events presented before the first block snapshot.
                if let Some(controller) = &mut self.block_controller {
                    controller.ship_random();
                } else {
                    self.pending_ship_random = self.pending_ship_random.wrapping_add(1);
                }
                if self.ship_engine.is_none() {
                    let samples = self.assets.ship_engine.clone();
                    if let Some(mixer) = self.mixer() {
                        self.ship_engine = Some(Self::loop_sink(mixer, &samples, 0.));
                    }
                }
                let gain = self.audio.source_gain(positional_level(distance, 800.));
                if let Some(sink) = &self.ship_engine {
                    sink.set_volume(gain);
                }
                // 4addd0 chooses oceanlin for model 0/2 and shiphorn for 1.
                // Its conditional random re-trigger tests (tick & 31)>160,
                // which is unreachable, so one horn handle is retained.
                if self.ship_horn.is_none() {
                    let samples = if matches!(model_choice, 0 | 2) {
                        self.assets.ocean_liner.clone()
                    } else {
                        self.assets.ship_horn.clone()
                    };
                    if let Some(mixer) = self.mixer() {
                        self.ship_horn = Some(Self::one_shot(mixer, &samples, 0.));
                    }
                }
                if let Some(sink) = &self.ship_horn {
                    sink.set_volume(gain);
                }
            }
            EnvironmentCall::Cue(id) => {
                if id == 30 {
                    stop(&mut self.crane_lift);
                    return;
                }
                if id == 32 {
                    stop(&mut self.crane_loop);
                    return;
                }
                if distance >= 300. {
                    return;
                }
                let (has_handle, samples, looping) = match id {
                    27 => (
                        self.crane_start.is_some(),
                        self.assets.crane_start.clone(),
                        false,
                    ),
                    28 => (
                        self.crane_end.is_some(),
                        self.assets.crane_end.clone(),
                        false,
                    ),
                    29 => (
                        self.crane_lift.is_some(),
                        self.assets.crane_lift.clone(),
                        true,
                    ),
                    31 => (
                        self.crane_loop.is_some(),
                        self.assets.crane_loop.clone(),
                        true,
                    ),
                    _ => return,
                };
                if has_handle {
                    return;
                }
                let Some(mixer) = self.mixer() else { return };
                let sink = if looping {
                    Self::loop_sink(mixer, &samples, 0.)
                } else {
                    Self::one_shot(mixer, &samples, 0.)
                };
                // Nonspatial 4ae2b0 creates at source0 and returns. Only the
                // separate 4ae1f0 Refresh path applies positional level/factor.
                match id {
                    27 => self.crane_start = Some(sink),
                    28 => self.crane_end = Some(sink),
                    29 => self.crane_lift = Some(sink),
                    31 => self.crane_loop = Some(sink),
                    _ => unreachable!(),
                }
            }
            EnvironmentCall::Refresh => {
                if distance >= 300. {
                    // Native 4ae73d releases +140/+144/+148; +14c is not
                    // visited on this branch.
                    stop(&mut self.crane_start);
                    stop(&mut self.crane_lift);
                    stop(&mut self.crane_loop);
                    return;
                }
                let gain = self.audio.source_gain(positional_level(distance, 300.));
                for sink in [
                    &self.crane_start,
                    &self.crane_end,
                    &self.crane_lift,
                    &self.crane_loop,
                ]
                .into_iter()
                .flatten()
                {
                    sink.set_volume(gain);
                }
            }
        }
    }
}

fn stop(handle: &mut Option<rodio::Sink>) {
    if let Some(sink) = handle.take() {
        sink.stop();
    }
}

fn positional_level(distance: f32, radius: f32) -> i32 {
    // 4addd0 / 4ae1f0 call the truncating 50e928 conversion twice.
    // Their reciprocal constants are stored as f32 before x87 multiplication.
    let units = f64::from(distance).trunc() + 1.;
    let reciprocal = f64::from(1. / radius);
    (100. - units * reciprocal * 100.).trunc().clamp(0., 100.) as i32
}

fn construction_fallback_level(distance: f32) -> i32 {
    // 4ad9be/4ad9ee call 50e928, which temporarily selects x87 truncation.
    // The intermediate division and 90-point scale stay on the x87 stack.
    let units = f64::from(distance).trunc() + 1.;
    (90. - units / 250. * 90.).trunc().clamp(0., 90.) as i32
}

fn train_level(distance: f32, scale: f32, stored_units: bool) -> i32 {
    // 4af479 keeps the first toot calculation on the x87 stack; 4af58b
    // reloads a stored f32 for train motion. 50e928 truncates the result.
    let units = f64::from(distance) + 1.;
    let units = if stored_units {
        f64::from(units as f32)
    } else {
        units
    };
    (70. - units * f64::from(scale)).trunc().clamp(0., 100.) as i32
}

fn aircraft_volume(distance: f32) -> i32 {
    // 4af6fe..4af717 keeps the calculation on x87 until 50e928 truncates.
    (60. - (f64::from(distance) + 1.) * f64::from(0.125f32)).trunc() as i32
}

fn road_level(sum: f32) -> i32 {
    // 4af91d..4af934 retains the f32 reciprocal sum on x87, then 50e928
    // truncates 20 + 200 * sum before the native upper cap of 80.
    (20. + f64::from(sum) * f64::from(200f32))
        .trunc()
        .clamp(0., 80.) as i32
}

fn road_motor_volume(distance: f32) -> i32 {
    // 4af963..4af977: 55 - (distance + 1) * 0.5, then 50e928 truncates.
    (55. - (f64::from(distance) + 1.) * f64::from(0.5f32)).trunc() as i32
}

fn road_motor_frequency(speed: f32) -> i32 {
    // 4af9bd..4af9d6 keeps each f32 multiplier on x87 until truncation.
    (f64::from(speed) * f64::from(f32::from_bits(0x3f8e38e4)) * 11_025. + 22_050.).trunc() as i32
}

#[cfg(test)]
fn positional_gain(d: f32, r: f32) -> f32 { positional_level(d, r) as f32 / 100. }
#[cfg(test)]
fn construction_fallback_gain(d: f32) -> f32 { construction_fallback_level(d) as f32 / 100. }
#[cfg(test)]
fn train_gain(d: f32, scale: f32, stored: bool) -> f32 { train_level(d, scale, stored) as f32 / 100. }
#[cfg(test)]
fn road_gain(sum: f32) -> f32 { road_level(sum) as f32 / 100. }

#[cfg(test)]
mod focus_tests {
    include!("transport_audio_focus_tests.rs");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_audio_ui_does_not_fault_transport_submission() {
        let (sender, receiver) = mpsc::channel::<Request>();
        submit(&Some(sender.clone()), Operation::ResidentCrowd(40.)).unwrap();
        assert!(matches!(receiver.try_recv().unwrap().operation,
            Operation::ResidentCrowd(weight) if weight == 40.));
        drop(receiver);
        submit(&Some(sender), Operation::ResidentCrowd(41.)).unwrap();
        submit(&None, Operation::Reset).unwrap();
    }

    #[test]
    fn detached_audio_ui_preserves_live_backend_errors() {
        let (auction, receiver) = mpsc::channel();
        drop(receiver);
        assert_eq!(crate::live_host::auction_audio::request(&Some(auction.clone()),
            crate::live_host::auction_audio::Operation::Reset).unwrap(), 0);
        crate::live_host::auction_audio::enqueue(&Some(auction),
            crate::live_host::auction_audio::Operation::Reset).unwrap();

        let (era, receiver) = mpsc::channel();
        drop(receiver);
        assert_eq!(crate::era_audio::request(&Some(era),
            crate::era_audio::Operation::Stop).unwrap(), 0);

        let (gameplay, receiver) = mpsc::channel();
        drop(receiver);
        assert_eq!(crate::gameplay_sound::request(&Some(gameplay.clone()),
            crate::gameplay_sound::Operation::UiLightClick).unwrap(), 0);
        crate::gameplay_sound::enqueue(&Some(gameplay),
            crate::gameplay_sound::Operation::UiLightClick).unwrap();

        let (sender, receiver) = mpsc::channel::<crate::era_audio::Request>();
        let ui = std::thread::spawn(move || receiver.recv().unwrap().reply
            .send(Err("original audio backend failure".to_owned())).unwrap());
        assert_eq!(crate::era_audio::request(&Some(sender),
            crate::era_audio::Operation::Stop).unwrap_err(),
            "original audio backend failure");
        ui.join().unwrap();
    }

    #[test]
    fn original_audio_reset_silences_retained_city_beds() {
        let samples = vec![0.5; 64];
        let assets = Assets {
            train_motion: samples.clone(),
            train_toot: samples.clone(),
            road_ambient: samples.clone(),
            road_motor: samples.clone(),
            resident_crowd: samples.clone(),
            near_aircraft: samples.clone(),
            ship_engine: samples.clone(),
            ship_horn: samples.clone(),
            ocean_liner: samples.clone(),
            crane_start: samples.clone(),
            crane_end: samples.clone(),
            crane_lift: samples.clone(),
            crane_loop: samples.clone(),
            construction_scaffold: samples.clone(),
            construction_saw: samples.clone(),
            construction_hammer: samples.clone(),
            block_factory: samples.clone(),
            block_school: samples.clone(),
            block_park_one: samples.clone(),
            block_park_two: samples.clone(),
            block_owl: samples.clone(),
        };
        let (mixer, mut output) = rodio::mixer::mixer(1, 22_050);
        mixer.add(rodio::source::Zero::new(1, 22_050));
        for _ in 0..512 {
            assert_eq!(output.next(), Some(0.));
        }
        let mut player = Player::new(assets);
        player.road_ambient = Some(Player::loop_sink(&mixer, &samples, 1.));
        player.resident_crowd = Some(Player::loop_sink(&mixer, &samples, 1.));
        player.construction[0] = Some(Player::loop_sink(&mixer, &samples, 1.));
        assert!(
            (0..512)
                .filter_map(|_| output.next())
                .any(|sample| sample > 0.)
        );
        let retained = player.construction[0].as_ref().unwrap() as *const rodio::Sink;
        let mut construction = [[None; 2]; 4];
        construction[0][0] = Some(ConstructionSource {
            distance: 300.,
            position: [300., 0., 0.],
        });
        player.construction(construction);
        let sink = player.construction[0].as_ref().unwrap();
        assert_eq!(sink as *const rodio::Sink, retained);
        assert_eq!(sink.volume(), 0.);
        construction[0][0].as_mut().unwrap().distance = 100.;
        player.construction(construction);
        let sink = player.construction[0].as_ref().unwrap();
        assert_eq!(sink as *const rodio::Sink, retained);
        assert!((sink.volume() - 0.14723125).abs() < 1e-7);
        player.road_ambient = Some(Player::loop_sink(&mixer, &samples, 1.));
        let road = player.road_ambient.as_ref().unwrap() as *const rodio::Sink;
        player.set_interface_mode(3);
        player.road(0.1, [[101f32.to_bits(), 0, 0]; 3]);
        assert_eq!(
            player.road_ambient.as_ref().unwrap() as *const rodio::Sink,
            road
        );
        assert_eq!(player.road_ambient.as_ref().unwrap().volume(), 1.);
        player.set_interface_mode(1);
        player.road(0.1, [[101f32.to_bits(), 0, 0]; 3]);
        assert!(player.road_ambient.is_none());
        player.apply(Operation::Reset);
        let tail: Vec<_> = (0..8192).map(|_| output.next()).collect();
        assert!(
            tail[tail.len() - 1024..]
                .iter()
                .all(|sample| *sample == Some(0.))
        );
        assert!(player.road_ambient.is_none());
        assert!(player.road_motors.iter().all(Option::is_none));
        assert!(player.resident_crowd.is_none());
        assert!(player.construction[0].is_none());
    }

    #[test]
    fn original_ranked_road_motor_producer_and_mixer_lifecycle() {
        let mut vehicles = [[0u8; 208]; 200];
        let mut auxiliaries = [[0u8; 284]; 3];
        let base = 0x1002_0000u32;
        for (slot, (distance, speed)) in [(10f32, 1f32), (30., 0.5), (50., 0.25)]
            .into_iter()
            .enumerate()
        {
            vehicles[slot][0x1c..0x20].copy_from_slice(&9u32.to_le_bytes());
            vehicles[slot][0x28..0x2c].copy_from_slice(&speed.to_bits().to_le_bytes());
            vehicles[slot][0xc4..0xc8].copy_from_slice(&(base + slot as u32 * 284).to_le_bytes());
            auxiliaries[slot][4..8].copy_from_slice(&distance.to_le_bytes());
        }
        let mut proximity = openmt2001::traffic::road::audio::Proximity::default();
        proximity
            .update(&vehicles, &auxiliaries, base, [0.; 3])
            .unwrap();
        assert_eq!(proximity.count, 3);
        let (sender, receiver) = mpsc::channel();
        submit(
            &Some(sender),
            Operation::RoadProximity {
                reciprocal_sum: proximity.reciprocal_sum,
                nearest: proximity.nearest,
            },
        )
        .unwrap();
        let Operation::RoadProximity {
            reciprocal_sum,
            mut nearest,
        } = receiver.recv().unwrap().operation
        else {
            panic!("road proximity operation changed");
        };
        assert!(reciprocal_sum > 0.);
        assert_eq!(nearest.map(|row| f32::from_bits(row[0])), [10., 30., 50.]);

        let (mixer, _output) = rodio::mixer::mixer(1, 22_050);
        let samples = vec![0.5; 64];
        let mut player = Player::new(Assets {
            train_motion: samples.clone(),
            train_toot: samples.clone(),
            road_ambient: samples.clone(),
            road_motor: samples.clone(),
            resident_crowd: samples.clone(),
            near_aircraft: samples.clone(),
            ship_engine: samples.clone(),
            ship_horn: samples.clone(),
            ocean_liner: samples.clone(),
            crane_start: samples.clone(),
            crane_end: samples.clone(),
            crane_lift: samples.clone(),
            crane_loop: samples.clone(),
            construction_scaffold: samples.clone(),
            construction_saw: samples.clone(),
            construction_hammer: samples.clone(),
            block_factory: samples.clone(),
            block_school: samples.clone(),
            block_park_one: samples.clone(),
            block_park_two: samples.clone(),
            block_owl: samples,
        });
        player.update_road_motors(nearest, Some(&mixer));
        for handle in &player.road_motors {
            let sink = handle.as_ref().unwrap();
            assert_eq!(sink.volume(), 0., "native creation word is zero");
            assert_eq!(sink.speed(), 1.);
        }
        player.update_road_motors(nearest, Some(&mixer));
        for (slot, (volume, frequency)) in [(0.12175870, 34_300), (0.07413103, 28_175), (0.04345102, 25_112)]
            .into_iter()
            .enumerate()
        {
            let sink = player.road_motors[slot].as_ref().unwrap();
            assert!((sink.volume() - volume).abs() < 1e-7);
            assert_eq!(sink.speed(), frequency as f32 / 22_050.);
        }
        nearest[1][0] = 101f32.to_bits();
        player.update_road_motors(nearest, Some(&mixer));
        assert!(player.road_motors[1].is_none());
        assert!(player.road_motors[0].is_some());
        assert!(player.road_motors[2].is_some());
        player.set_interface_mode(3);
        player.road(reciprocal_sum, nearest);
        assert!(player.road_motors.iter().all(Option::is_none));
    }

    #[test]
    fn original_ranked_road_motor_gain_and_frequency() {
        // v1.4 4af946 with three retained motor2.wav handles, PC64 x87.
        for (distance, gain) in [(10., 49), (30., 39), (50., 29)] {
            assert_eq!(road_motor_volume(distance), gain);
        }
        for (speed, frequency) in [
            (0., 22_050),
            (0.25, 25_112),
            (0.5, 28_175),
            (1., 34_300),
            (2., 46_550),
        ] {
            assert_eq!(road_motor_frequency(speed), frequency);
        }
    }

    #[test]
    fn original_construction_nonspatial_fallback_gain() {
        // Captured from v1.4 4ad8b0 with a retained handle and no spatial
        // backend. 50e928 truncates twice, including the final volume.
        for (distance, volume) in [
            (0., 89),
            (0.5, 89),
            (2., 88),
            (100., 53),
            (124.999, 45),
            (125., 44),
            (199., 18),
            (248., 0),
        ] {
            assert_eq!(construction_fallback_gain(distance), volume as f32 / 100.);
        }
    }

    #[test]
    fn original_train_motion_and_toot_gain() {
        // v1.4 4af430 with retained motion and a toot creation, PC64 x87.
        for (distance, toot, motion) in [(0., 69., 69.), (100., 64., 59.), (499., 44., 19.)] {
            assert_eq!(train_gain(distance, 0.05, false), toot / 100.);
            assert_eq!(train_gain(distance, 0.1, true), motion / 100.);
        }
    }

    #[test]
    fn original_aircraft_engine_gain() {
        // Signed values captured at 4af6e0's retained-handle volume call.
        for (distance, volume) in [
            (0., 59),
            (100., 47),
            (250., 28),
            (470., 1),
            (479., 0),
            (499., -2),
        ] {
            assert_eq!(aircraft_volume(distance), volume);
        }
    }

    #[test]
    fn original_resident_crowd_category_weight_and_gain() {
        let mut categories = [[0u8; 0x17c]; 52];
        let camera = [0., 0., 0.];
        assert_eq!(resident_crowd_weight(&categories, camera), 0.);
        assert_eq!(crowd_gain(30.), None);
        assert_eq!(crowd_gain(30.5), Some(0.));
        assert_eq!(crowd_gain(30.9), Some(0.));
        assert_eq!(crowd_gain(31.), Some(0.01));
        assert_eq!(crowd_gain(31.9), Some(0.01));
        categories[0][0x88..0x8c].copy_from_slice(&80i32.to_le_bytes());
        // Native clips the 1-unit center distance to 100: 50/100 * 80 = 40.
        assert_eq!(resident_crowd_weight(&categories, camera), 40.);
        assert_eq!(crowd_gain(40.), Some(0.1));
        categories[0][0xe0..0xe4].copy_from_slice(&1000f32.to_le_bytes());
        // The original replays a 5000-unit sentinel beyond the 500 cutoff.
        assert_eq!(resident_crowd_weight(&categories, camera), 0.8);
        assert_eq!(crowd_gain(0.8), None);
        categories[0][0xe0..0xe4].copy_from_slice(&499f32.to_le_bytes());
        assert_eq!(resident_crowd_weight(&categories, camera), 8.);
        categories[0][0xe0..0xe4].copy_from_slice(&500f32.to_le_bytes());
        assert_eq!(resident_crowd_weight(&categories, camera), 0.8);
        categories[0][0xe0..0xe4].copy_from_slice(&0f32.to_le_bytes());
        categories[1][0x88..0x8c].copy_from_slice(&200i32.to_le_bytes());
        assert_eq!(resident_crowd_weight(&categories, camera), 140.);
        assert_eq!(crowd_gain(140.), Some(0.65));
        categories[0][0xe0..0xe4].copy_from_slice(&1000f32.to_le_bytes());
        assert_eq!(resident_crowd_weight(&categories, camera), 100.8);
        assert_eq!(crowd_gain(f32::NAN), None);
    }

    #[test]
    fn original_road_ambient_gain() {
        // v1.4 4af860 retained-handle volumes with PC64 x87 conversion.
        for (sum, volume) in [
            (0., 20.),
            (0.003, 20.),
            (0.005, 20.),
            (0.1, 40.),
            (0.299, 79.),
            (0.3, 80.),
            (1., 80.),
        ] {
            assert_eq!(road_gain(sum), volume / 100.);
        }
    }

    #[test]
    fn v14_transport_sound_assets_and_levels() {
        let (Some(exe), Some(installer)) = (
            std::env::var_os("OPENMT2001_EXE"),
            std::env::var_os("OPENMT2001_INSTALLER"),
        ) else {
            return;
        };
        let exe = std::fs::read(exe).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let assets = Assets::load(&image, Path::new(&installer)).unwrap();
        // 4afcde presents road ambience first, then category crowd ambience.
        for (callsite, target) in [(0x4afcb5u32, 0x4af860u32), (0x4afcbc, 0x4afa40)] {
            let code = image.at(callsite, 5).unwrap();
            assert_eq!(code[0], 0xe8);
            assert_eq!(
                (callsite + 5)
                    .wrapping_add_signed(i32::from_le_bytes(code[1..5].try_into().unwrap())),
                target
            );
        }
        assert_eq!(image.f32_at(0x525248).unwrap(), 50.);
        assert_eq!(image.f32_at(0x525094).unwrap(), 30.);
        assert_eq!(image.f32_at(0x525058).unwrap(), 100.);
        assert_eq!(image.f32_at(0x525bb0).unwrap(), 500.);
        let high = Path::new(&installer).join("Standard/sound/high");
        assert_eq!(
            original_case_path(&high.join("shiphorn.wav"))
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            "SHIPHORN.WAV"
        );
        assert_eq!(
            original_case_path(&high.join("oceanlin.wav"))
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            "OCEANLIN.WAV"
        );
        assert_eq!(assets.train_motion.len(), 57_344);
        assert_eq!(assets.train_toot.len(), 111_871);
        assert_eq!(assets.road_ambient.len(), 220_886);
        assert_eq!(assets.road_motor.len(), 31_981);
        assert_eq!(assets.resident_crowd.len(), 84_446);
        assert_eq!(assets.near_aircraft.len(), 33_604);
        assert_eq!(assets.ship_engine.len(), 16_689);
        assert_eq!(assets.ship_horn.len(), 215_882);
        assert_eq!(assets.ocean_liner.len(), 197_866);
        assert_eq!(assets.crane_start.len(), 24_341);
        assert_eq!(assets.crane_end.len(), 30_427);
        assert_eq!(assets.crane_lift.len(), 202_751);
        assert_eq!(assets.crane_loop.len(), 32_363);
        // These WAVs have trailing chunks after `data`; playback reads only
        // the original PCM length declared in the data header.
        assert_eq!(assets.construction_scaffold.len(), 33_793);
        assert_eq!(assets.construction_saw.len(), 50_687);
        assert_eq!(assets.construction_hammer.len(), 17_464);
        // Both native 4adb10 and 4afcde present the four nearby sound lists.
        for (address, expected) in [
            (0x4adb30, 0x8b0260u32),
            (0x4adb4a, 0x8af418),
            (0x4adb64, 0x8b0240),
            (0x4adb7e, 0x8b0280),
            (0x4afcde, 0x8b0260),
            (0x4afcf8, 0x8af418),
            (0x4afd12, 0x8b0240),
            (0x4afd2c, 0x8b0280),
        ] {
            let instruction = image.at(address, 5).unwrap();
            assert_eq!(instruction[0], 0x68);
            assert_eq!(
                u32::from_le_bytes(instruction[1..5].try_into().unwrap()),
                expected
            );
        }
        assert_eq!(road_gain(0.), 0.2);
        assert_eq!(road_gain(1.), 0.8);
        assert_eq!(positional_gain(0., 800.), 0.99);
        assert_eq!(positional_gain(400., 800.), 0.49);
        assert_eq!(positional_gain(299., 300.), 0.);
    }

    #[test]
    fn construction_sources_keep_two_nearest_with_stable_ties() {
        let mut groups = [[None; 2]; 4];
        for x in [9., 4., 7., 4., 2.] {
            insert_construction_source(&mut groups, 2, [x, 0., 0.], [0.; 3]).unwrap();
        }
        assert_eq!(groups[2][0].unwrap().distance, 2.);
        assert_eq!(groups[2][1].unwrap().distance, 4.);
        assert!(groups[0].iter().all(Option::is_none));
    }
}
