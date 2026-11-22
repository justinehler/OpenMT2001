//! Original0044ee90..0044f0a6 transport proximity sounds, after train motion.
use super::{RouteSound, scene::Scene, word};
use crate::x87::Sum;

pub(super) fn distance(point: [f32; 3], camera: [f32; 3]) -> Result<Sum, String> {
    let mut total = Sum::default();
    // The native x87 stack adds Z squared, then Y squared, then X squared.
    for axis in (0..3).rev() {
        let mut difference = Sum::from_f32(point[axis])?;
        difference.add(Sum::from_f32(camera[axis])?.negated());
        difference.mul(difference);
        total.add(difference);
    }
    total.sqrt().map_err(str::to_owned)
}

impl Scene {
    pub fn proximity_sounds(
        &mut self,
        tick: &mut u32,
        camera: &mut [f32; 3],
        mut sound: impl FnMut(RouteSound, &mut Self, &mut u32, &mut [f32; 3]) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut nearest = f32::from_bits(0x4cbebc20);
        match *tick % 5 {
            1 => {
                let mut speed = 0;
                for row in &self.trains {
                    if word(row, 0) == 0 {
                        continue;
                    }
                    let point = [
                        f32::from_bits(word(row, 0x18)),
                        f32::from_bits(word(row, 0x1c)),
                        -f32::from_bits(word(row, 0x20)),
                    ];
                    let value = distance(point, *camera)?;
                    if value.compare(Sum::from_f32(nearest)?) == std::cmp::Ordering::Less {
                        nearest = value.to_f32();
                        speed = word(row, 0x30);
                    }
                }
                if nearest < 10_000. {
                    sound(
                        RouteSound {
                            id: 7,
                            distance: nearest,
                            arguments: [speed, 0],
                        },
                        self,
                        tick,
                        camera,
                    )?;
                }
            }
            0 => {
                let mut tier = 0;
                for index in 0..self.aircraft.len() {
                    let row = &self.aircraft[index];
                    if word(row, 0) == 0 {
                        continue;
                    }
                    tier = word(row, 4);
                    let point =
                        std::array::from_fn(|axis| f32::from_bits(word(row, 0x18 + axis * 4)));
                    let extended = distance(point, *camera)?;
                    let value = extended.to_f32();
                    if extended.compare(Sum::from_f32(nearest)?) == std::cmp::Ordering::Less {
                        nearest = value;
                    }
                    for (offset, id) in [
                        (1000, if tier == 0 { 2 } else { 6 }),
                        (600, if tier == 0 { 1 } else { 4 }),
                        (850, 5),
                    ] {
                        // Each native test reloads tick and the retained start;
                        // synchronous sound callbacks can change either.
                        let elapsed = tick
                            .wrapping_sub(word(&self.aircraft[index], 16))
                            .wrapping_sub(offset) as i32;
                        if (0..=5).contains(&elapsed) && (offset != 850 || tier != 0) {
                            sound(
                                RouteSound {
                                    id,
                                    distance: value,
                                    arguments: [0; 2],
                                },
                                self,
                                tick,
                                camera,
                            )?;
                        }
                    }
                }
                // EBP retains the LAST active tier, not the nearest aircraft's.
                if nearest < 10_000. {
                    sound(
                        RouteSound {
                            id: if tier == 0 { 0 } else { 3 },
                            distance: nearest,
                            arguments: [0; 2],
                        },
                        self,
                        tick,
                        camera,
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
