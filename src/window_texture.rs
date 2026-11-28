//! Shared window lights (480240), driven by the dedicated 822ae0 stream.
//! Like the original, this transient image and RNG are not part of city saves.
use std::sync::Arc;

pub const RNG_REFERENCE: u32 = 0x822ae0;
pub const SIDE: u32 = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication {
    pub revision: u64,
    pub pixels: Arc<[u8]>,
}

pub struct WindowTexture {
    snapshot: Option<Arc<[u8]>>,
    pixels: Arc<[u8]>,
    dimensions: [u32; 2],
    phase: u32,
    rng: u32,
    revision: u64,
    dirty: bool,
    /// Original startup presentation option, not the texture-detail setting.
    /// The normal textured frontend uses zero; mode two suppresses all work.
    pub graphics_mode: u32,
}

impl WindowTexture {
    pub fn new(rng: u32) -> Self {
        Self {
            snapshot: None,
            pixels: Arc::from([]),
            dimensions: [0; 2],
            phase: 0,
            rng,
            revision: 0,
            dirty: false,
            graphics_mode: 0,
        }
    }

    /// Bind the renderer's loader-converted BuildingTextures row 26 once.
    /// The first eligible tick takes the original-image snapshot.
    pub fn bind(&mut self, width: u32, height: u32, pixels: Arc<[u8]>) {
        if self.dimensions == [0; 2] {
            self.dimensions = [width, height];
            self.pixels = pixels;
        }
    }

    fn draw(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        (self.rng >> 16) & 0x7fff
    }

    pub fn tick(&mut self, hour: u32) {
        if self.graphics_mode == 2
            || self.dimensions != [SIDE; 2]
            || self.pixels.len() != (SIDE * SIDE * 4) as usize
        {
            return;
        }
        let snapshot = self
            .snapshot
            .get_or_insert_with(|| self.pixels.clone())
            .clone();
        self.phase = self.phase.wrapping_add(1);
        if self.phase != 3 {
            return;
        }
        self.phase = 0;
        if !self.draw().is_multiple_of(14) {
            return;
        }
        // Even 08:00 and inactive hours consume both coordinate draws.
        let column = (self.draw() % 4) as usize * 16;
        let row = (self.draw() % 4) as usize * 16;
        if hour == 8 {
            Arc::make_mut(&mut self.pixels).fill(0);
        } else if (1..=6).contains(&hour) || (18..=22).contains(&hour) {
            let one_in_five = self.draw().is_multiple_of(5);
            let restore = if hour <= 6 { one_in_five } else { !one_in_five };
            let pixels = Arc::make_mut(&mut self.pixels);
            for y in row..row + 16 {
                let start = (y * SIDE as usize + column) * 4;
                let range = start..start + 16 * 4;
                if restore {
                    pixels[range.clone()].copy_from_slice(&snapshot[range]);
                } else {
                    pixels[range].fill(0);
                }
            }
        } else {
            return;
        }
        // Native marks dirty even when the selected pixels already match.
        self.revision += 1;
        self.dirty = true;
    }

    pub fn pending(&self) -> Option<Publication> {
        if self.dirty { self.current() } else { None }
    }

    /// Also used when rebuilding scene geometry after a successful publication.
    pub fn current(&self) -> Option<Publication> {
        self.snapshot.as_ref().map(|_| Publication {
            revision: self.revision,
            pixels: self.pixels.clone(),
        })
    }

    /// An older in-flight upload must not clear a newer mutation.
    pub fn published(&mut self, revision: u64) {
        if revision == self.revision {
            self.dirty = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(seed: u32) -> WindowTexture {
        let mut state = WindowTexture::new(seed);
        state.bind(64, 64, (0..16384).map(|i| (i % 255 + 1) as u8).collect());
        state
    }

    fn triple(state: &mut WindowTexture, hour: u32) {
        for _ in 0..3 {
            state.tick(hour);
        }
    }

    #[test]
    fn gates_precede_snapshot_pacing_and_random() {
        let mut state = image(7);
        state.graphics_mode = 2;
        triple(&mut state, 20);
        assert_eq!((state.rng, state.phase), (7, 0));
        assert!(state.snapshot.is_none());
        state.graphics_mode = 0;
        state.dimensions = [32, 64];
        triple(&mut state, 20);
        assert_eq!((state.rng, state.phase), (7, 0));
        assert!(state.snapshot.is_none());
        state.dimensions = [64, 64];
        state.tick(20);
        state.tick(20);
        assert_eq!(state.rng, 7);
        assert!(state.snapshot.is_some());
        state.tick(20);
        assert_eq!(state.rng, 0x003d_7aae);
    }

    #[test]
    fn seeded_hour_branches_and_coordinate_draw_order() {
        // Independently calculated MSVCRT outputs for seed 2. First accepted
        // period is number 5: column 19650, row 14590, choice 26431.
        let mut evening = image(2);
        let original = evening.pixels.clone();
        for _ in 0..5 {
            triple(&mut evening, 18);
        }
        assert_eq!(evening.rng, 0x673f_320a);
        assert!(evening.pending().is_some());
        assert_eq!(evening.pixels, original);
        let mut morning = image(2);
        for _ in 0..5 {
            triple(&mut morning, 1);
        }
        for y in 0..64 {
            for x in 0..64 {
                let at = (y * 64 + x) * 4;
                let expected = if (32..48).contains(&x) && (32..48).contains(&y) {
                    &[0; 4][..]
                } else {
                    &original[at..at + 4]
                };
                assert_eq!(&morning.pixels[at..at + 4], expected);
            }
        }
        for hour in [0, 7, 9, 17, 23, u32::MAX, 8] {
            let mut state = image(2);
            for _ in 0..5 {
                triple(&mut state, hour);
            }
            assert_eq!(state.rng, 0x38fe_7d93);
            if hour == 8 {
                assert!(state.pixels.iter().all(|&p| p == 0));
                assert!(state.pending().is_some());
            } else {
                assert_eq!(state.pixels, original);
                assert!(state.pending().is_none());
            }
        }
    }

    #[test]
    fn one_in_five_reverses_morning_and_evening() {
        // Seed 1 qualifies at period 8, then draws column 2, row 0,
        // and choice 5705 (divisible by 5). No unrelated stream is advanced.
        for hour in [1, 6, 18, 22] {
            let mut state = image(1);
            let original = state.pixels.clone();
            for _ in 0..8 {
                triple(&mut state, hour);
            }
            assert_eq!(state.rng, 0x1649_b442);
            for y in 0..64 {
                for x in 0..64 {
                    let at = (y * 64 + x) * 4;
                    let clear = hour >= 18 && (32..48).contains(&x) && y < 16;
                    assert_eq!(
                        &state.pixels[at..at + 4],
                        if clear {
                            &[0; 4][..]
                        } else {
                            &original[at..at + 4]
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn snapshot_restoration_and_publication_acknowledgment() {
        let mut state = image(1);
        let original = state.pixels.clone();
        for _ in 0..625 {
            state.tick(8);
        }
        assert!(state.pixels.iter().all(|&p| p == 0));
        let old = state.pending().unwrap();
        for _ in 0..3125 {
            state.tick(20);
        }
        assert!(state.pixels.iter().any(|&p| p != 0));
        for (actual, source) in state
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .zip(original.as_chunks::<4>().0)
        {
            assert!(*actual == [0; 4] || actual == source);
        }
        state.published(old.revision);
        assert!(state.pending().is_some());
        let revision = state.pending().unwrap().revision;
        state.published(revision);
        assert!(state.pending().is_none());
        assert!(state.current().is_some());
    }
}
