//! Original v1.4 result particle manager (455080, 4549e0, 4530c0).
//!
//! The launch particle at emitter+1c shares storage with the emitter's
//! activity flag, random counter, and vertical-velocity burst threshold.
use openmt2001::pe::PeImage;

const EMITTER_LIMIT: usize = 30;
const PARTICLES_PER_KIND: usize = 90;
// Original454f10 ->4ebf40: capacity750, independent of flags0x200.
// Shared with the dedicated result GPU owner; generic particles keep512.
pub(super) const POINT_CAPACITY: usize = 750;
const ANGLE_STEP: f64 = 0.017_453_292_5; // Original double at 525bb8.
const DT: f32 = f32::from_bits(0x3dcccccd); // 52508c.
const GRAVITY: f32 = f32::from_bits(0x3ca3d70b); // 525bc4.

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub position: [f32; 3],
    pub argb: u32,
    pub size: f32,
}

/// Original 4532c0 branch names and the kind-3 4549a9 launch cue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Launch1,
    Launch2,
    EchoCrackle,
    EchoSingle,
    LaunchWhistle,
}

#[derive(Clone, Copy, Debug)]
struct Particle {
    active: i32,
    life: i32,
    decrement: i32,
    position: [f32; 3],
    velocity: [f32; 3],
    size: f32,
    rgb: [u8; 3],
    alpha_table: u8,
}

#[derive(Clone, Debug)]
struct Emitter {
    kind: u8,
    size: f32,
    phase: u8,
    launch: Vec<Option<Particle>>,
    burst: Vec<Option<Particle>>,
}

impl Emitter {
    fn new(kind: u8, size: f32, launch: Particle) -> Self {
        let mut particles = vec![None; PARTICLES_PER_KIND];
        particles[0] = Some(launch);
        Self {
            kind,
            size,
            phase: 1,
            launch: particles,
            burst: vec![None; PARTICLES_PER_KIND],
        }
    }
    fn insert_launch(&mut self, particle: Particle) {
        if let Some(slot) = self
            .launch
            .iter_mut()
            .find(|slot| slot.as_ref().is_none_or(|p| p.active == 0))
        {
            *slot = Some(particle);
        }
    }
    fn insert_trail(&mut self, particle: Particle) {
        if let Some(slot) = self
            .burst
            .iter_mut()
            .find(|slot| slot.as_ref().is_none_or(|p| p.active == 0))
        {
            *slot = Some(particle);
        }
    }
}

/// A simulation tick is the original 455080 callback, distinct from the
/// result controller's 50 Hz title clock.
pub struct Fireworks {
    emitter_count: usize,
    spawn_width: [f32; 2],
    origin: [f32; 3],
    rng: u32,
    emitters: Vec<Option<Emitter>>,
    alpha: [[u8; 256]; 2],
    burst_countdown: i32,
    tick: usize,
    submitted_count: usize,
    points: Vec<Point>,
    sounds: Vec<Cue>,
}

impl Fireworks {
    pub fn new(image: &PeImage<'_>, setting: u32, rng: u32) -> Result<Self, String> {
        let (emitter_count, origin, spawn_width) = match setting {
            // 4b16d0 initializes type 2, with 20 or 30 concurrent emitters.
            0 => (20, [-170., 0., -100.], [80., 9.]),
            1 => (30, [-172., 0., 201.], [310., 9.]),
            _ => return Err("unsupported original result fireworks setting".into()),
        };
        let mut alpha = [[0; 256]; 2];
        for (table, address) in alpha.iter_mut().zip([0x53a004, 0x53a104]) {
            table.copy_from_slice(image.at(address, 256)?);
        }
        Ok(Self {
            emitter_count,
            spawn_width,
            origin,
            rng,
            emitters: vec![None; EMITTER_LIMIT],
            alpha,
            burst_countdown: -1,
            tick: 0,
            submitted_count: 0,
            points: Vec::new(),
            sounds: Vec::new(),
        })
    }

    fn next(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        (self.rng >> 16) & 0x7fff
    }

    fn rotate(velocity: &mut [f32; 3], a: usize, b: usize, angle: f32) {
        let (sin, cos) = f64::from(angle).sin_cos();
        let x = f64::from(velocity[a]);
        let y = f64::from(velocity[b]);
        velocity[a] = (x * cos - y * sin) as f32;
        velocity[b] = (y * cos + x * sin) as f32;
    }

    fn initial_launch(&mut self, kind: u8, origin: [f32; 3]) -> Particle {
        // 454650 stores color 6 = white before drawing a launch spark.
        let random = self.next() & 0x3ff;
        let multiplier = match kind {
            0 | 2 => 0.7,
            1 => 0.8,
            3 => 0.85,
            4 => 1.,
            5 => 0.9,
            _ => unreachable!(),
        };
        let speed = (((random as f32 * (1. / 1024.)) * 0.1 + 1.) * 6.) * multiplier;
        let mut velocity = [0., speed, 0.];
        for (a, b) in [(1, 2), (0, 2), (0, 1)] {
            let angle = ((self.next() % 6) as i32 - 3) as f32 * ANGLE_STEP as f32;
            Self::rotate(&mut velocity, a, b, angle);
        }
        Particle {
            active: 1,
            life: 127,
            decrement: 1,
            position: origin,
            velocity,
            size: 1.,
            rgb: [255; 3],
            alpha_table: 0,
        }
    }

    fn spawn_emitter(&mut self, index: usize) {
        let kind = (self.next() % 6) as u8; // 455260 for manager mode 2.
        let z = self.next() & 0x3ff;
        let x = self.next() & 0x3ff;
        let origin = [
            (x as f32 * (1. / 1024.)) * self.spawn_width[0] + self.origin[0],
            self.origin[1],
            (z as f32 * (1. / 1024.)) * self.spawn_width[1] + self.origin[2],
        ];
        let launch = self.initial_launch(kind, origin);
        self.emitters[index] = Some(Emitter::new(kind, 6., launch));
        if kind == 3 {
            self.sounds.push(Cue::LaunchWhistle);
        }
    }

    fn color(&mut self, kind: u8) -> [u8; 3] {
        let mut random = |mask: u32, offset: u8| (self.next() & mask) as u8 + offset;
        match kind {
            0 => [random(0x1f, 60), random(0x1f, 60), 255],
            1 => [255, random(0x1f, 60), random(0x1f, 60)],
            2 => [random(0x1f, 60), 255, random(0x1f, 60)],
            3 => [255, 170, random(0x1f, 40)],
            4 => [255, 255, random(0x1f, 50)],
            5 => [255, random(0x3f, 60), 255],
            6 => [255; 3],
            _ => unreachable!(),
        }
    }

    fn launch_trail(&mut self, emitter: &mut Emitter) {
        // 453d60 reads the launch particle's life byte and position. Its
        // local descriptor is copied into the first free 52-byte slot.
        let life = self.next()
            & if emitter.kind == 2 {
                0x1f
            } else if emitter.kind >= 4 {
                3
            } else {
                0xf
            };
        let palette = if emitter.kind >= 4 {
            if self.next() & 1 == 0 { 6 } else { 4 }
        } else if self.next() & 1 == 0 {
            6
        } else {
            4
        };
        let rgb = self.color(palette);
        let x = self.next() & 0x3ff;
        let z = self.next() & 0x3ff;
        let launch = emitter.launch[0].as_ref().unwrap();
        let offset = |value: u32| value as f32 * (1. / 1024.) * 0.1 - 0.05;
        let particle = Particle {
            active: 1,
            life: launch.life & 0xff,
            decrement: life as i32 + 2,
            position: [
                launch.position[0] + offset(x),
                launch.position[1],
                launch.position[2] + offset(z),
            ],
            velocity: [0.; 3],
            size: 1.,
            rgb,
            alpha_table: 0,
        };
        emitter.insert_trail(particle);
    }

    fn random_velocity(&mut self, speed: f32) -> [f32; 3] {
        let mut velocity = [speed, 0., 0.];
        for (a, b) in [(1, 2), (0, 2), (0, 1)] {
            let angle = (self.next() & 0x3ff) as f32 * ANGLE_STEP as f32;
            Self::rotate(&mut velocity, a, b, angle);
        }
        velocity
    }

    fn insert_burst(emitter: &mut Emitter, particle: Particle) {
        emitter.insert_launch(particle);
    }

    fn burst(&mut self, emitter: &mut Emitter) {
        // 4532c0 places the first burst in the launch array, reusing its now
        // inactive first slot. 454230 emits the shorter secondary sparks.
        let origin = emitter.launch[0].as_ref().unwrap().position;
        let kind = emitter.kind;
        self.sounds.push(match kind {
            0 => Cue::Launch2,
            1 => Cue::EchoCrackle,
            2 => Cue::EchoSingle,
            3..=5 => Cue::Launch1,
            _ => unreachable!(),
        });
        if kind == 4 {
            let palette = [1, 0, 2, 5][(self.next() & 3) as usize];
            for _ in 0..50 {
                let speed_random = self.next() & 0x3ff;
                let speed = ((20. * 0.01 * speed_random as f32 * (1. / 1024.)) + 1.)
                    * (emitter.size * 0.25);
                let velocity = self.random_velocity(speed);
                let rgb = self.color(palette);
                let life = 127 - (self.next() & 7) as i32;
                let size_random = self.next() & 0x3ff;
                let size = ((50. * 0.01 * size_random as f32 * (1. / 1024.)) + 1.) * 2.5;
                Self::insert_burst(
                    emitter,
                    Particle {
                        active: 1,
                        life,
                        decrement: 1,
                        position: origin,
                        velocity,
                        size,
                        rgb,
                        alpha_table: 1,
                    },
                );
            }
            return;
        }
        let palette = if matches!(kind, 2 | 3 | 5) {
            Some([1, 0, 2, 5][(self.next() & 3) as usize])
        } else {
            None
        };
        let count = match kind {
            0 | 3 | 5 => 30,
            1 => 60,
            2 => 20,
            _ => unreachable!(),
        };
        for _ in 0..count {
            let speed_random = self.next() & 0x3ff;
            let speed_scale = if kind == 2 { 0.2 } else { 50. };
            let speed =
                ((speed_random as f32 * (1. / 1024.)) * 0.15 + 1.) * emitter.size * speed_scale;
            let velocity = self.random_velocity(speed);
            let color_kind = if let Some(palette) = palette {
                palette
            } else if kind == 1 {
                [4, 3, 6][(self.next() % 3) as usize]
            } else if self.next() & 1 == 0 {
                4
            } else {
                3
            };
            let rgb = self.color(color_kind);
            let size = (self.next() & 0x3ff) as f32 * (1. / 1024.) + 2.;
            Self::insert_burst(
                emitter,
                Particle {
                    active: 1,
                    life: if kind == 1 { 100 } else { 127 },
                    decrement: 2,
                    position: origin,
                    velocity,
                    size,
                    rgb,
                    alpha_table: if matches!(kind, 2 | 3 | 5) { 1 } else { 0 },
                },
            );
        }
    }

    fn secondary(&mut self, emitter: &mut Emitter) {
        // 454230 skips the launch particle and tests the remaining 89 sparks.
        for slot in 1..PARTICLES_PER_KIND {
            let Some(source) = emitter.launch[slot] else {
                continue;
            };
            if source.active == 0 {
                continue;
            }
            if self.next() & 0xf == 0 {
                let life = if matches!(emitter.kind, 1 | 5) {
                    127
                } else {
                    source.life & 0xff
                };
                let decrement =
                    (self.next() & 0xf) as i32 + if matches!(emitter.kind, 1 | 5) { 5 } else { 1 };
                let x = self.next() & 0x3ff;
                let z = self.next() & 0x3ff;
                let offset = |value: u32| value as f32 * (1. / 1024.) * 0.1 - 0.05;
                let particle = Particle {
                    active: 1,
                    life,
                    decrement,
                    position: [
                        source.position[0] + offset(x),
                        source.position[1],
                        source.position[2] + offset(z),
                    ],
                    velocity: [0.; 3],
                    size: 1.,
                    rgb: source.rgb,
                    alpha_table: 0,
                };
                if let Some(free) = emitter
                    .burst
                    .iter_mut()
                    .find(|slot| slot.as_ref().is_none_or(|particle| particle.active == 0))
                {
                    *free = Some(particle);
                }
            }
        }
    }

    pub fn tick(&mut self) -> Result<&[Point], String> {
        self.tick += 1;
        self.points.clear();
        let mut draw = Vec::new();
        for index in 0..self.emitter_count {
            let Some(mut emitter) = self.emitters[index].take() else {
                continue;
            };
            if emitter.phase == 1 {
                let launch = emitter.launch[0]
                    .as_mut()
                    .ok_or("result launch particle missing")?;
                launch.active = 1;
                if self.next() & 1 != 0 {
                    launch.life += 1;
                }
                let random = self.next() & 0x3ff;
                let threshold = emitter.size * 0.2 * random as f32 * (1. / 1024.);
                if threshold < launch.velocity[1] {
                    self.launch_trail(&mut emitter);
                } else {
                    launch.active = 0;
                    self.burst(&mut emitter);
                    emitter.phase = 2;
                    self.burst_countdown = 30;
                }
            } else if emitter.phase == 2 {
                if emitter.kind == 5 && self.burst_countdown != -1 {
                    self.burst_countdown -= 1;
                    if self.burst_countdown == 0 {
                        self.burst(&mut emitter);
                        self.burst_countdown = -1;
                    }
                }
                self.secondary(&mut emitter);
                if !emitter
                    .launch
                    .iter()
                    .chain(emitter.burst.iter())
                    .flatten()
                    .any(|particle| particle.active != 0)
                {
                    emitter.phase = 0;
                }
            }
            for (slot, particle) in emitter
                .launch
                .iter()
                .chain(emitter.burst.iter())
                .enumerate()
            {
                if let Some(particle) = particle.filter(|particle| particle.active != 0) {
                    let table = &self.alpha[particle.alpha_table as usize];
                    let life = particle.life.clamp(0, 255) as usize;
                    let alpha = table[life];
                    draw.push((index, slot, particle.rgb, alpha, particle.size));
                }
            }
            self.emitters[index] = (emitter.phase != 0).then_some(emitter);
        }
        self.submitted_count = draw.len();
        // 4530c0 advances only particles gathered before a fresh emitter is
        // started; 453030 later submits the advanced positions and prior alpha.
        for (index, slot, rgb, alpha, size) in draw {
            let emitter = self.emitters[index].as_mut().unwrap();
            let particle = if slot < PARTICLES_PER_KIND {
                emitter.launch[slot].as_mut()
            } else {
                emitter.burst[slot - PARTICLES_PER_KIND].as_mut()
            }
            .ok_or("result particle collection changed")?;
            for axis in 0..3 {
                particle.position[axis] = (f64::from(particle.velocity[axis]) * f64::from(DT)
                    + f64::from(particle.position[axis]))
                    as f32;
            }
            particle.velocity[1] = (f64::from(particle.velocity[1]) - f64::from(GRAVITY)) as f32;
            particle.life -= particle.decrement;
            particle.active = particle.life.max(0);
            if self.points.len() < POINT_CAPACITY {
                self.points.push(Point {
                    position: particle.position,
                    argb: (u32::from(alpha) << 24)
                        | (u32::from(rgb[0]) << 16)
                        | (u32::from(rgb[1]) << 8)
                        | u32::from(rgb[2]),
                    size,
                });
            }
        }
        if let Some(index) = self.emitters[..self.emitter_count]
            .iter()
            .position(Option::is_none)
        {
            self.spawn_emitter(index);
        }
        Ok(&self.points)
    }

    pub fn rng_state(&self) -> u32 {
        self.rng
    }
    pub fn points(&self) -> &[Point] {
        &self.points
    }
    pub fn submitted_count(&self) -> usize {
        self.submitted_count
    }
    pub fn tick_count(&self) -> usize {
        self.tick
    }
    /// Drain in native tick order, including catch-up ticks in the result UI.
    pub fn take_sounds(&mut self) -> Vec<Cue> {
        std::mem::take(&mut self.sounds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_prefix_750_preserves_order_and_steps_rejected_tail() {
        // Controlled original field contract, not an original capture: legal
        // twenty-emitter phase2 state. Only launch slot0 and burst slots live,
        // so secondary skips all source slots and admission consumes no RNG.
        let fixture = |count: usize| {
            let mut next_id = 0;
            let particle = |id: usize| Particle { active: 1, life: 77, decrement: 3,
                position: [id as f32, 2., -10.], velocity: [0.;3], size: 2.,
                rgb: [id as u8, (id >> 8) as u8, 0x3d], alpha_table: (id & 1) as u8 };
            let mut remaining = count - 20;
            let mut emitters = vec![None; EMITTER_LIMIT];
            for target in emitters.iter_mut().take(20) {
                let launch = particle(next_id); next_id += 1;
                let mut emitter = Emitter::new(0, 6., launch);
                emitter.phase = 2;
                let take = remaining.min(90);
                for slot in 0..take {
                    emitter.burst[slot] = Some(particle(next_id)); next_id += 1;
                }
                remaining -= take;
                *target = Some(emitter);
            }
            assert_eq!((next_id, remaining), (count, 0));
            Fireworks { emitter_count: 20, spawn_width: [80.,9.], origin: [-170.,0.,-100.],
                rng: 0x12345678, emitters,
                alpha: [std::array::from_fn(|i| i as u8), std::array::from_fn(|i| 255-i as u8)],
                burst_countdown: -1, tick: 0, submitted_count: 0, points: Vec::new(), sounds: Vec::new() }
        };
        for count in [511, 512, 513, 749, 750, 751, 1820] {
            let mut effect = fixture(count);
            let points = effect.tick().unwrap().to_vec();
            assert_eq!(effect.submitted_count(), count, "root count is not collector-clamped");
            assert_eq!(points.len(), count.min(750));
            for (id, point) in points.iter().enumerate() {
                assert_eq!(point.position, [id as f32, 2., -10.], "ordered root prefix {id}");
                let alpha = if id & 1 == 0 { 77u32 } else { 178 };
                assert_eq!(point.argb, alpha << 24 | (id as u8 as u32) << 16
                    | ((id >> 8) as u8 as u32) << 8 | 0x3d, "prior-life ARGB {id}");
                assert_eq!(point.size, 2.);
            }
            // Collector refusal must never stop manager stepping. This checks
            // every gathered particle, including ids750..1819 when present.
            let mut stepped = 0;
            for emitter in effect.emitters[..20].iter().flatten() {
                for particle in emitter.launch.iter().chain(&emitter.burst).flatten() {
                    assert_eq!((particle.life, particle.active), (74, 74));
                    assert_eq!(particle.velocity, [0., -GRAVITY, 0.]);
                    stepped += 1;
                }
            }
            assert_eq!(stepped, count);
            assert_eq!(effect.rng_state(), 0x12345678);
            assert_eq!(effect.tick_count(), 1);
            assert!(effect.take_sounds().is_empty());
        }
    }

    #[test]
    #[ignore = "requires original v1.4 executable and 200-frame result particle fixture"]
    fn original_result_sound_producers() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        // Original 455260 LCG produces kind3 from seed2 on the first spawn.
        let mut whistle = Fireworks::new(&image, 0, 2).unwrap();
        whistle.tick().unwrap();
        assert_eq!(whistle.take_sounds(), vec![Cue::LaunchWhistle]);
        assert!(whistle.take_sounds().is_empty(), "a cue is consumed once");

        // The captured full-lifecycle fixture starts setting0 from seed13289.
        // Its existing position/RNG test verifies the same 200 native frames;
        // this gate checks the concrete cues reached within that bounded span.
        let fixture = std::fs::read(std::env::var("OPENMT2001_NATIVE_RESULT_PARTICLE_FULL").unwrap()).unwrap();
        assert_eq!(&fixture[..8], b"MTRSFP01");
        assert_eq!(u32::from_le_bytes(fixture[8..12].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(fixture[12..16].try_into().unwrap()), 13289);
        assert_eq!(u32::from_le_bytes(fixture[16..20].try_into().unwrap()), 200);
        let mut effect = Fireworks::new(&image, 0, 13289).unwrap();
        let mut seen = [false; 5];
        for _ in 0..200 {
            effect.tick().unwrap();
            for cue in effect.take_sounds() {
                seen[match cue {
                    Cue::Launch1 => 0,
                    Cue::Launch2 => 1,
                    Cue::EchoCrackle => 2,
                    Cue::EchoSingle => 3,
                    Cue::LaunchWhistle => 4,
                }] = true;
            }
        }
        assert!(seen[3] && seen[4], "captured 200-frame lifecycle reaches an echo burst and kind-3 whistle");
        for (kind, expected) in [
            (0, Cue::Launch2),
            (1, Cue::EchoCrackle),
            (2, Cue::EchoSingle),
            (3, Cue::Launch1),
            (4, Cue::Launch1),
            (5, Cue::Launch1),
        ] {
            let mut effect = Fireworks::new(&image, 0, 13289).unwrap();
            let launch = effect.initial_launch(kind, [0.; 3]);
            let mut emitter = Emitter::new(kind, 6., launch);
            effect.burst(&mut emitter);
            assert_eq!(effect.take_sounds(), vec![expected], "native 453c30 burst kind {kind}");
        }
    }

    fn compare_fixture(fixture_key: &str, tolerance: f32) {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let fixture = std::fs::read(std::env::var(fixture_key).unwrap()).unwrap();
        assert_eq!(&fixture[..8], b"MTRSFP01");
        let mut at = 8;
        let mut word = || {
            let value = u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
            at += 4;
            value
        };
        for setting in 0..=1 {
            assert_eq!(word(), setting);
            let seed = word();
            let count = word();
            let mut effect = Fireworks::new(&image, setting, seed).unwrap();
            for frame in 1..=count {
                let points = effect.tick().unwrap().to_vec();
                assert_eq!(
                    effect.rng_state(),
                    word(),
                    "setting {setting} frame {frame} RNG"
                );
                let count = word();
                assert_eq!(
                    effect.submitted_count() as u32,
                    count,
                    "setting {setting} frame {frame} submitted count"
                );
                assert_eq!(
                    points.len() as u32,
                    count.min(POINT_CAPACITY as u32),
                    "setting {setting} frame {frame} GPU count"
                );
                let sampled = word();
                for point in &points[..sampled as usize] {
                    let position = [word(), word(), word()];
                    for (axis, bits) in position.into_iter().enumerate() {
                        let expected = f32::from_bits(bits);
                        assert!(
                            (point.position[axis] - expected).abs() < tolerance,
                            "setting {setting} frame {frame} position {axis}: {:?} vs {expected}",
                            point.position
                        );
                    }
                    assert_eq!(point.argb, word(), "setting {setting} frame {frame} color");
                    assert_eq!(
                        point.size.to_bits(),
                        word(),
                        "setting {setting} frame {frame} size"
                    );
                }
            }
        }
        assert_eq!(at, fixture.len());
    }

    #[test]
    #[ignore = "requires v1.4 executable and native 455080 particle fixture"]
    fn matches_native_launch_frames() {
        compare_fixture("OPENMT2001_NATIVE_RESULT_PARTICLE_LIVE", 0.00005);
    }

    #[test]
    #[ignore = "requires 200-frame v1.4 particle lifecycle fixture"]
    fn matches_native_firework_lifecycle() {
        compare_fixture("OPENMT2001_NATIVE_RESULT_PARTICLE_FULL", 0.02);
    }
}
