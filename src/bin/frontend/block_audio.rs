//! Original v1.4 450080 block sound positions and 4aec00 dispatch.
//! Playback stays with the UI audio owner; the host supplies live category
//! centers, camera, clock and the original 8af3ec startup RNG stream.

pub const PARKS: [usize; 7] = [14, 19, 47, 48, 49, 50, 51];

#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pub centers: [Option<[f32; 2]>; 52],
    pub camera: [f32; 3],
    pub hour: u32,
    pub tick: u32,
    pub previous_tick: u32,
    pub initial_random_state: u32,
}

impl Snapshot {
    pub fn from_categories(
        categories: &[[u8; 0x17c]; 52],
        camera: [f32; 3],
        hour: u32,
        tick: u32,
        previous_tick: u32,
        initial_random_state: u32,
    ) -> Self {
        Self {
            centers: std::array::from_fn(|index| {
                let row = &categories[index];
                (row[4] != 0).then(|| {
                    [
                        f32::from_le_bytes(row[0xe0..0xe4].try_into().unwrap()),
                        f32::from_le_bytes(row[0xe4..0xe8].try_into().unwrap()),
                    ]
                })
            }),
            camera,
            hour,
            tick,
            previous_tick,
            initial_random_state,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Source {
    pub position: [f32; 3],
    pub distance: f32,
}

fn distance(position: [f32; 3], camera: [f32; 3]) -> f64 {
    let delta: [f64; 3] =
        std::array::from_fn(|axis| f64::from(position[axis]) - f64::from(camera[axis]));
    ((delta[2] * delta[2] + delta[1] * delta[1]) + delta[0] * delta[0]).sqrt()
}

impl Source {
    /// Nonspatial 4ae7a0 recomputes distance on the x87 stack, clips to
    /// 1..300, then uses the truncating 50e928 conversion twice.
    pub fn level(self, camera: [f32; 3]) -> i32 {
        let rounded = distance(self.position, camera).clamp(1., 300.).trunc() + 1.;
        let scale = f64::from(f32::from_bits(0x3b11a2b4)); // native 5265c0
        (100. - rounded * scale * 100.).trunc().clamp(0., 100.) as i32
    }
    #[cfg(test)]
    pub fn gain(self, camera: [f32; 3]) -> f32 {
        self.level(camera) as f32 / 100.
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Factory,
    School,
    ParkOne,
    ParkTwo,
    Owl,
}

impl Kind {
    pub fn looping(self) -> bool {
        matches!(self, Self::Factory | Self::School)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Play { category: usize, kind: Kind },
    Stop(usize),
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Controller {
    pub sources: [Source; 52],
    random_state: u32,
    previous_tick: u32,
    day_timer: u32,
    night_timer: u32,
}

impl Controller {
    pub fn new(random_state: u32, previous_tick: u32) -> Self {
        Self {
            sources: [Source::default(); 52],
            random_state,
            previous_tick,
            day_timer: 0,
            night_timer: 0,
        }
    }

    fn random(&mut self, modulus: u32) -> u32 {
        self.random_state = self
            .random_state
            .wrapping_mul(0x343fd)
            .wrapping_add(0x269ec3);
        ((self.random_state >> 16) & 0x7fff) % modulus
    }

    /// Nearby 4addd0 ship calls consume this same stream once even though
    /// their subsequent `(tick & 31) > 160` re-trigger gate cannot succeed.
    pub fn ship_random(&mut self) {
        self.random(101);
    }

    pub fn advance(
        &mut self,
        snapshot: Snapshot,
        interface_mode: u32,
        mut playing: [bool; 52],
    ) -> Vec<Action> {
        // 44faf0 stores each pavement instance's x/z from category+e0/+e4.
        // 450080 retains inactive rows and fixes active source Y to 40.
        for (source, center) in self.sources.iter_mut().zip(snapshot.centers) {
            if let Some([x, z]) = center {
                let position = [x, 40., z];
                *source = Source {
                    position,
                    distance: distance(position, snapshot.camera) as f32,
                };
            }
        }
        let delta = snapshot.tick.wrapping_sub(self.previous_tick);
        let mut actions = Vec::new();
        if matches!(interface_mode, 1 | 3) {
            self.previous_tick = snapshot.tick; // 4afd77 also runs in modes 1/3.
            return actions;
        }
        for category in 0..52 {
            let kind = match category {
                2 | 35 | 39 | 42 => Some(Kind::Factory),
                41 if (8..16).contains(&snapshot.hour) => Some(Kind::School),
                41 => {
                    actions.push(Action::Stop(category));
                    None
                }
                _ => None,
            };
            if let Some(kind) = kind {
                actions.push(Action::Play { category, kind });
            }
            if !PARKS.contains(&category) {
                continue;
            }
            if (7..20).contains(&snapshot.hour) {
                // Native accumulates the same elapsed ticks once per park,
                // preserving its shared static timer and category order.
                self.day_timer = self.day_timer.wrapping_add(delta);
                if self.day_timer >= 1500 && self.random(101) <= 70 && !playing[category] {
                    let kind = if self.random(11) < 5 {
                        Kind::ParkOne
                    } else {
                        Kind::ParkTwo
                    };
                    actions.push(Action::Play { category, kind });
                    playing[category] = self.sources[category].distance <= 300.;
                    self.day_timer = 0;
                }
            }
            if ((21..24).contains(&snapshot.hour) || snapshot.hour <= 5)
                && self.sources[category].distance <= 150.
            {
                self.night_timer = self.night_timer.wrapping_add(delta);
                if self.night_timer >= 1500 {
                    // Native consumes rand%101 before testing <=100.
                    self.random(101);
                    if !playing[category] {
                        actions.push(Action::Play {
                            category,
                            kind: Kind::Owl,
                        });
                        playing[category] = true;
                    }
                    self.night_timer = 0;
                }
            }
            // 4aee4f releases all seven park handles at hours 5 and 20.
            if matches!(snapshot.hour, 5 | 20) {
                for park in PARKS {
                    actions.push(Action::Stop(park));
                    playing[park] = false;
                }
            }
        }
        self.previous_tick = snapshot.tick;
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(hour: u32, delta: u32, seed: u32, distance: f32) -> Snapshot {
        Snapshot {
            centers: [Some([distance, 0.]); 52],
            camera: [0., 40., 0.],
            hour,
            tick: 100 + delta,
            previous_tick: 100,
            initial_random_state: seed,
        }
    }
    fn plays(actions: Vec<Action>) -> Vec<(usize, Kind)> {
        actions
            .into_iter()
            .filter_map(|action| match action {
                Action::Play { category, kind } => Some((category, kind)),
                Action::Stop(_) => None,
            })
            .collect()
    }

    #[test]
    fn native_block_geometry_and_nonspatial_gain() {
        let mut input = snapshot(8, 0, 205, 100.);
        input.centers[0] = Some([150.5, -90.25]);
        input.camera = [100., 20., -20.];
        let mut controller = Controller::new(205, 100);
        assert!(controller.advance(input, 3, [false; 52]).is_empty());
        // Observed by running original 450080 with a retained instance seam.
        assert_eq!(
            controller.sources[0],
            Source {
                position: [150.5, 40., -90.25],
                distance: f32::from_bits(0x42b1993c),
            }
        );
        let retained = controller.sources[0];
        input.centers[0] = None;
        input.camera = [0.; 3];
        controller.advance(input, 3, [false; 52]);
        assert_eq!(controller.sources[0], retained);
        // Original 4ae7a0 replay under FPCW037f: volume77/33, not rounded78.
        assert_eq!(
            Source {
                position: [100., 40., 0.],
                distance: 100.
            }
            .gain([0., 40., 0.]),
            0.77
        );
        assert_eq!(
            Source {
                position: [299., 40., 0.],
                distance: 299.
            }
            .gain([0., 40., 0.]),
            0.33
        );
        // Native 4addd0 nonspatial replay, using the same truncating helper:
        // ship engine/horn gains are99/87/49 at distances0/100/400.
        assert_eq!(super::super::positional_gain(0., 800.), 0.99);
        assert_eq!(super::super::positional_gain(100., 800.), 0.87);
        assert_eq!(super::super::positional_gain(400., 800.), 0.49);
    }

    #[test]
    fn native_block_order_timers_and_park_random_gate() {
        use Kind::*;
        let mut controller = Controller::new(205, 100);
        assert_eq!(
            plays(controller.advance(snapshot(8, 1500, 205, 100.), 0, [false; 52])),
            [
                (2, Factory),
                (14, ParkOne),
                (19, ParkTwo),
                (35, Factory),
                (39, Factory),
                (41, School),
                (42, Factory),
                (47, ParkTwo),
                (49, ParkOne),
                (51, ParkTwo)
            ]
        );
        // Original replay roll70 at category14 is accepted; roll71 is rejected.
        for (seed, expected) in [(288, true), (10, false)] {
            let mut controller = Controller::new(seed, 100);
            let result = plays(controller.advance(snapshot(8, 1500, seed, 100.), 0, [false; 52]));
            assert_eq!(result.iter().any(|(category, _)| *category == 14), expected);
        }
        // Native 4aec00 replay after a nearby ship consumed one 4addd0 draw:
        // seed288 becomes64166755, and all seven parks choose park2.
        let mut controller = Controller::new(288, 100);
        controller.ship_random();
        let result = plays(controller.advance(snapshot(8, 1500, 288, 100.), 0, [false; 52]));
        let parks: Vec<_> = result
            .into_iter()
            .filter(|(category, _)| PARKS.contains(category))
            .collect();
        assert_eq!(parks, PARKS.map(|category| (category, ParkTwo)));
        // 1499 is below threshold at the first park; the second park adds the
        // same delta to the shared accumulator and crosses1500.
        let mut controller = Controller::new(205, 100);
        let result = plays(controller.advance(snapshot(8, 1499, 205, 100.), 0, [false; 52]));
        assert!(!result.iter().any(|(category, _)| *category == 14));
        assert!(result.iter().any(|(category, _)| *category == 19));
        for (distance, expected) in [(150., true), (151., false)] {
            let mut controller = Controller::new(205, 100);
            let result =
                plays(controller.advance(snapshot(23, 1500, 205, distance), 0, [false; 52]));
            assert_eq!(result.iter().any(|(_, kind)| *kind == Owl), expected);
            assert!(!result.iter().any(|(_, kind)| *kind == School));
        }
        let mut controller = Controller::new(205, 100);
        let result = controller.advance(snapshot(5, 1500, 205, 100.), 0, [false; 52]);
        assert_eq!(
            &result[result.len() - PARKS.len()..],
            &PARKS.map(Action::Stop)
        );
        let mut controller = Controller::new(205, 100);
        assert!(
            controller
                .advance(snapshot(8, 1500, 205, 100.), 3, [false; 52])
                .is_empty()
        );
        let result = plays(controller.advance(snapshot(8, 1501, 205, 100.), 0, [false; 52]));
        assert!(
            !result
                .iter()
                .any(|(_, kind)| matches!(kind, ParkOne | ParkTwo))
        );
    }

    #[test]
    fn v14_block_audio_provenance_and_source_assets() {
        let (Some(exe), Some(installer)) = (
            openmt2001::fixtures::optional("EXE"),
            openmt2001::fixtures::optional("INSTALLER"),
        ) else {
            return;
        };
        let exe = std::fs::read(exe).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        // 44faf0 uses category+e0/+e4 as instance+4/+c; 450080 fixes Y40.
        assert_eq!(
            image.at(0x44fbd0, 6).unwrap(),
            &[0x8b, 0x45, 0x4c, 0x8b, 0x4d, 0x50]
        );
        assert_eq!(
            image.at(0x4500ba, 7).unwrap(),
            &[0xc7, 0x46, 0xfc, 0, 0, 0x20, 0x42]
        );
        assert_eq!(image.f32_at(0x525ba4).unwrap(), 300.);
        assert_eq!(image.f32_at(0x525bac).unwrap(), 800.);
        assert_eq!(image.f32_at(0x5265b0).unwrap(), 150.);
        assert_eq!(image.f32_at(0x5265c0).unwrap().to_bits(), 0x3b11a2b4);
        assert_eq!(image.f32_at(0x544ff0).unwrap(), 70.);
        assert_eq!(image.f32_at(0x544fdc).unwrap(), 100.);
        assert_eq!(
            image.at(0x4e55d4, 7).unwrap(),
            &[0xc7, 0x40, 4, 0xfd, 0x43, 3, 0]
        );
        assert_eq!(
            image.at(0x4e55db, 7).unwrap(),
            &[0xc7, 0x40, 8, 0xc3, 0x9e, 0x26, 0]
        );
        for address in [0x544fec, 0x544fd8] {
            assert_eq!(
                u32::from_le_bytes(image.at(address, 4).unwrap().try_into().unwrap()),
                1500
            );
        }
        let call = image.at(0x4afd65, 5).unwrap();
        assert_eq!(call[0], 0xe8);
        assert_eq!(
            0x4afd6au32.wrapping_add_signed(i32::from_le_bytes(call[1..].try_into().unwrap())),
            0x4aec00
        );
        let assets = super::super::Assets::load(&image, std::path::Path::new(&installer)).unwrap();
        assert_eq!(assets.block_factory.len(), 51_011);
        assert_eq!(assets.block_school.len(), 150_157);
        assert_eq!(assets.block_park_one.len(), 61_038);
        assert_eq!(assets.block_park_two.len(), 66_078);
        assert_eq!(assets.block_owl.len(), 48_545);
    }
}
