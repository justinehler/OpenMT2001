//! City smoke: 004b2ac0/004b5cb0 initialize, 004b5db0 updates,
//! and 004b60d0 submits camera-facing points. Emitters are world locators,
//! not BuildFile details or flags on individual building models.
use crate::{archive::ArchiveIndex, mesh::CachedMesh};

pub const LOCATOR_MODEL: &str = "gamedata2/NEW_building stuff/smoke_stack_locators.tm";
pub const RNG_REFERENCE: u32 = 0x8b2250;
pub const SLOTS: usize = 20;

/// 004b5d05's additional emitter precedes every locator from the TM model.
const FIXED_POSITION: [f32; 3] = [
    f32::from_bits(0x43922666),
    f32::from_bits(0x42780000),
    f32::from_bits(0xc323e666),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Native point coordinates; the billboard submission negates Z.
    pub position: [f32; 3],
    pub size: f32,
    pub argb: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Puff {
    active: bool,
    fading: bool,
    position: [f32; 3],
    drift: f32,
    rise: f32,
    size: f32,
    alpha: i32,
}

impl Default for Puff {
    fn default() -> Self {
        Self {
            active: false,
            fading: false,
            position: [0.; 3],
            drift: 0.,
            rise: 0.7,
            size: 4.,
            alpha: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Emitter {
    origin: [f32; 3],
    puffs: [Puff; SLOTS],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Smoke {
    emitters: Vec<Emitter>,
    random_state: u32,
}

/// The loader at 004c01a0 selects this dedicated locator-only TM. Its rows
/// already contain world coordinates, including those over fixed utilities.
pub fn locators(index: &ArchiveIndex, archive: &[u8]) -> Result<Vec<[f32; 3]>, String> {
    let entry = index
        .find(LOCATOR_MODEL)
        .ok_or("smoke locator model missing")?;
    let end = entry
        .offset
        .checked_add(entry.length)
        .ok_or("smoke locator range overflow")?;
    let bytes = archive
        .get(
            usize::try_from(entry.offset).map_err(|_| "smoke locator offset")?
                ..usize::try_from(end).map_err(|_| "smoke locator end")?,
        )
        .ok_or("smoke locator range")?;
    let model = CachedMesh::parse(bytes)?;
    if model.vertex_count != 0 || !model.parts.is_empty() || model.frames != 1 {
        return Err("smoke locator model must contain only static locators".into());
    }
    model
        .opaque_records_28
        .as_chunks::<28>()
        .0
        .iter()
        .map(|row| {
            let mut position = [0.; 3];
            for (axis, value) in position.iter_mut().enumerate() {
                *value = f32::from_le_bytes(
                    row[axis * 4..axis * 4 + 4]
                        .try_into()
                        .map_err(|_| "smoke locator coordinate")?,
                );
            }
            if position.iter().any(|value| !value.is_finite()) {
                return Err("nonfinite smoke locator".into());
            }
            Ok(position)
        })
        .collect()
}

fn random(state: &mut u32) -> i32 {
    *state = state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
    ((*state >> 16) & 0x7fff) as i32
}

impl Smoke {
    /// Seed semantics from 004b5c10: lift two units and reverse locator Z.
    pub fn new(random_state: u32, locators: &[[f32; 3]]) -> Self {
        Self {
            emitters: std::iter::once(FIXED_POSITION)
                .chain(locators.iter().copied())
                .map(|[x, y, z]| Emitter {
                    origin: [x, (f64::from(y) + 2.) as f32, -z],
                    puffs: [Puff::default(); SLOTS],
                })
                .collect(),
            random_state,
        }
    }

    /// Bind original locators before the host starts its retained logic loop.
    /// Native save loading also initializes the presentation pool afresh.
    pub fn bind_archive(&mut self, index: &ArchiveIndex, archive: &[u8]) -> Result<(), String> {
        *self = Self::new(self.random_state, &locators(index, archive)?);
        Ok(())
    }

    /// Exactly one 00481680 mode-1 scheduler dispatch. Rendering never ticks
    /// this state, so pause and camera motion cannot age or spawn puffs.
    pub fn tick(&mut self, logic_tick: u32) {
        for emitter in &mut self.emitters {
            if logic_tick.is_multiple_of(10)
                && let Some(puff) = emitter.puffs.iter_mut().find(|puff| !puff.active)
            {
                let initial_size = random(&mut self.random_state) % 101;
                let direction = random(&mut self.random_state) % 101;
                *puff = Puff {
                    active: true,
                    fading: false,
                    position: [emitter.origin[0], emitter.origin[1], -emitter.origin[2]],
                    drift: (f64::from(if direction < 50 {
                        -direction
                    } else {
                        direction
                    }) * f64::from(0.001f32)) as f32,
                    rise: 0.7,
                    size: (f64::from(initial_size) * f64::from(0.05f32) + 3.) as f32,
                    alpha: 0,
                };
            }
            // Newly spawned particles participate in the same update.
            for puff in emitter.puffs.iter_mut().filter(|puff| puff.active) {
                let step = random(&mut self.random_state) % 6;
                let z_delta = (f64::from(1.1f32) - f64::from(puff.rise)) * 2. * -0.25;
                puff.position[0] = (f64::from(puff.position[0]) + f64::from(puff.drift)) as f32;
                puff.position[1] = ((f64::from(puff.rise)
                    - (f64::from(step) + 0.1) * f64::from(0.0125f32))
                    + f64::from(puff.position[1])) as f32;
                puff.position[2] = (f64::from(puff.position[2]) + z_delta) as f32;
                if puff.fading {
                    let (divisor, scale) = if puff.size >= 15. {
                        (puff.size as i32 + 1, 0.1f32)
                    } else {
                        (5, 0.4f32) // The original width stays at 4.
                    };
                    let decrement = random(&mut self.random_state) % divisor + 1;
                    puff.alpha =
                        (f64::from(puff.alpha) - f64::from(decrement) * f64::from(scale)) as i32;
                } else {
                    puff.alpha += 10;
                    if puff.alpha >= 255 {
                        puff.alpha = 255;
                        puff.fading = true;
                    }
                }
                if puff.rise > 0.1f32 {
                    puff.rise = (f64::from(puff.rise) - f64::from(0.01f32)) as f32;
                }
                puff.size = (f64::from(puff.size) + 4. * 0.05) as f32;
                if puff.alpha <= 0 && puff.fading {
                    puff.active = false;
                    puff.position = [emitter.origin[0], emitter.origin[1], -emitter.origin[2]];
                }
            }
        }
    }

    /// 004b6123 uses graphics detail 0 to draw only emitter zero. There are
    /// no weather, hour or camera-distance gates.
    ///
    /// `lit_white` is the RGB the original reads from 008b2248: the ninth
    /// entry of the foliage palette that 004b6370 rebuilds earlier in the same
    /// city pass (white 0xffffffff lit by sun and ambient colour, see
    /// [`crate::trees::Tables::palette`]). Mode 19 modulates the texture by
    /// this diffuse colour, so the puffs are a lit light grey. The palette
    /// stays zero only until 004b6370 first runs.
    pub fn points(&self, detail: u8, lit_white: u32) -> Vec<Point> {
        let rgb = lit_white & 0x00ff_ffff;
        self.emitters
            .iter()
            .take(if detail == 0 { 1 } else { self.emitters.len() })
            .flat_map(|emitter| emitter.puffs.iter().filter(|puff| puff.active))
            .map(|puff| Point {
                position: puff.position,
                size: puff.size,
                argb: ((puff.alpha as u32) << 24).wrapping_add(rgb),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chimney_smoke_locator_carrier_builds_every_instance() {
        let mut archive = vec![0; 164 + 2 * 28];
        archive[4..6].copy_from_slice(&1u16.to_le_bytes());
        archive[12..16].copy_from_slice(&2u32.to_le_bytes());
        for (row, position) in archive[164..]
            .as_chunks_mut::<28>()
            .0
            .iter_mut()
            .zip([[10f32, 20., 30.], [40., 50., 60.]])
        {
            for (axis, value) in position.into_iter().enumerate() {
                row[axis * 4..axis * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        let index = ArchiveIndex {
            header_words: [0; 8],
            version: 3,
            header_flags: 0,
            entries: vec![crate::archive::ArchiveEntry {
                path: LOCATOR_MODEL.to_owned(),
                offset: 0,
                length: archive.len() as u64,
                metadata: [0; 13],
            }],
        };
        let mut smoke = Smoke::new(0, &[]);
        smoke.bind_archive(&index, &archive).unwrap();
        assert_eq!(
            smoke
                .emitters
                .iter()
                .map(|emitter| emitter.origin)
                .collect::<Vec<_>>(),
            [
                [FIXED_POSITION[0], 64., -FIXED_POSITION[2]],
                [10., 22., -30.],
                [40., 52., -60.]
            ]
        );
        smoke.tick(10);
        assert_eq!(smoke.points(2, 0).len(), 3);
        assert_eq!(smoke.points(0, 0).len(), 1);
        assert!(locators(&index, &archive[..archive.len() - 1]).is_err());
    }

    #[test]
    fn chimney_smoke_spawn_motion_growth_fade_and_retirement() {
        let mut smoke = Smoke::new(0, &[[10., 20., 30.]]);
        for tick in 1..10 {
            smoke.tick(tick);
        }
        assert!(smoke.points(2, 0).is_empty());
        smoke.tick(10);
        assert_eq!(smoke.points(0, 0).len(), 1);
        assert_eq!(smoke.points(2, 0).len(), 2);
        let first = smoke.emitters[0].puffs[0];
        // Controlled seed0/two-emitter execution of original 004b5db0.
        assert_eq!(
            first.position.map(f32::to_bits),
            [0x439220e5, 0x42814c29, 0xc3241999]
        );
        assert_eq!(first.drift.to_bits(), 0xbd3020c5);
        assert_eq!(first.size.to_bits(), 0x40a33333);
        assert_eq!(first.alpha, 10);
        assert_eq!(first.rise, (f64::from(0.7f32) - f64::from(0.01f32)) as f32);
        assert!(first.position[1] > 62. && first.position[2] < FIXED_POSITION[2]);
        assert!((first.size - 5.1).abs() < 0.00001);
        let frozen = smoke.clone();
        for _ in 0..5 {
            let _ = smoke.points(2, 0);
        }
        assert_eq!(
            smoke, frozen,
            "rendering while paused must not update state"
        );
        for tick in 11..20 {
            smoke.tick(tick);
        }
        assert_eq!(smoke.points(2, 0).len(), 2);
        let puff = smoke.emitters[0].puffs[0];
        assert_eq!(
            puff.position.map(f32::to_bits),
            [0x4391ef5c, 0x428c6000, 0xc3262000]
        );
        assert_eq!(puff.alpha, 100);
        assert!((puff.position[0] - (FIXED_POSITION[0] + first.drift * 10.)).abs() < 0.001);
        assert!(puff.position[1] > first.position[1]);
        smoke.tick(20);
        assert_eq!(smoke.points(2, 0).len(), 4);
        for tick in 21..=35 {
            smoke.tick(tick);
        }
        assert_eq!(smoke.emitters[0].puffs[0].alpha, 255);
        assert!(smoke.emitters[0].puffs[0].fading);
        // Suppress further births while exercising the exact lifetime path.
        let mut age = 26;
        while smoke.emitters[0].puffs[0].active && age < 500 {
            smoke.tick(1);
            age += 1;
        }
        assert_eq!(
            age, 160,
            "original seed0 first-puff lifetime in logic updates"
        );
        assert!(!smoke.emitters[0].puffs[0].active);
        smoke.tick(40);
        assert_eq!(
            smoke.emitters[0].puffs[0].alpha, 10,
            "first free slot is reused"
        );
    }

    #[test]
    fn chimney_smoke_points_take_lit_white_and_puff_alpha() {
        let mut smoke = Smoke::new(0, &[]);
        for tick in 10..=20 {
            smoke.tick(tick);
        }
        let alphas = smoke.emitters[0]
            .puffs
            .iter()
            .filter(|puff| puff.active)
            .map(|puff| puff.alpha as u32)
            .collect::<Vec<_>>();
        assert_eq!(alphas, [110, 10]);
        // 004b60d0 masks the palette entry's alpha and adds the puff alpha.
        let points = smoke.points(2, 0xffd8d0c8);
        assert_eq!(
            points.iter().map(|point| point.argb).collect::<Vec<_>>(),
            [0x6ed8d0c8, 0x0ad8d0c8]
        );
        // Before 004b6370 first fills the palette the colour is black.
        assert!(
            smoke
                .points(2, 0)
                .iter()
                .all(|point| point.argb & 0x00ff_ffff == 0)
        );
    }

    #[test]
    #[ignore = "requires original v1.4 smoke locator TM, executable and BuildFile"]
    fn original_chimney_smoke_carrier_models() {
        let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
        let archive = read("ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&read("ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let positions = locators(&index, &archive).unwrap();
        assert_eq!(positions.len(), 3);
        let smoke = Smoke::new(0, &positions);
        assert_eq!(smoke.emitters.len(), 4);
        let executable = read("EXE");
        let image = crate::pe::PeImage::parse(&executable).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let file_bytes = read("BUILD_FILE");
        let file = crate::build_file::BuildFile::parse(&file_bytes).unwrap();
        let buildings = crate::city_structures::buildings(&board, &file).unwrap();
        let electric = buildings
            .iter()
            .find(|building| building.spec.block == "ELECTRIC_COMPANY")
            .unwrap();
        assert_eq!(electric.selected.index, 296);
        let fixed = &smoke.emitters[0];
        assert!((fixed.origin[0] - electric.center[0]).abs() < 35.);
        assert!((fixed.origin[2] - electric.center[1]).abs() < 35.);
        eprintln!("fixed smoke emitter: Electric Company, BuildFile model296");
        // These names are spatial associations, not attachment keys: the
        // original initializer walks LOCATOR_MODEL, never these factory TMs.
        // Each locator lies within four units of its factory chimney mesh.
        for (position, name) in
            positions
                .iter()
                .zip(["bando_fact1.tm", "shortline_fact6.tm", "pensyl_fact2.tm"])
        {
            let path = format!("gamedata2/New_building stuff/train_stations/{name}");
            let entry = index.find(&path).unwrap();
            let model = CachedMesh::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap();
            let vertices = model.positions(0).unwrap();
            let distance = vertices
                .iter()
                .map(|vertex| {
                    (position[0] - vertex[0])
                        .hypot(position[1] - vertex[1])
                        .hypot(position[2] - vertex[2])
                })
                .min_by(f32::total_cmp)
                .unwrap();
            assert!(
                distance < 4.,
                "{name} locator is {distance} units from its mesh"
            );
            eprintln!("{LOCATOR_MODEL}: {name}");
        }
    }
}
