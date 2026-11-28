//! Original4eb520 sixteen-slot dust manager and4eb5b0 update order.
//! Buffers are owned arrays; original renderer/allocation pointers are opaque.
use crate::{pe::PeImage, x87::Sum};

#[derive(Clone, Debug)]
pub struct Effect {
    pub model: u32,
    pub state: u32,
    pub origin: [f32; 2],
    pub extent: [f32; 2],
    pub live: u32,
    pub positions: Vec<[f32; 3]>,
    pub particles: Vec<[u32; 5]>,
}
impl Default for Effect {
    fn default() -> Self {
        Self {
            model: 0,
            state: 1,
            origin: [0.; 2],
            extent: [0.; 2],
            live: 0,
            positions: vec![],
            particles: vec![],
        }
    }
}
#[derive(Clone, Debug)]
pub struct Manager {
    pub count: u32,
    pub random: u32,
    pub slots: [Effect; 16],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub position: [f32; 3],
    pub argb: u32,
    pub size: f32,
    pub model: u32,
    pub slot: u8,
}
impl Manager {
    pub fn new(seed: u32) -> Self {
        Self {
            count: 0,
            random: seed,
            slots: std::array::from_fn(|_| Effect::default()),
        }
    }
    pub fn allocated(&self) -> bool {
        self.slots.iter().any(|slot| slot.state != 1)
    }
    pub fn create(&mut self, arguments: [u32; 6]) -> Result<u32, String> {
        if self.count > 16 {
            return Ok(u32::MAX);
        }
        // Original count==16 scans outside its array if no free slot exists.
        // Preserve an explicit capacity error instead of a native overrun.
        let index = self
            .slots
            .iter()
            .position(|slot| slot.state == 1)
            .ok_or("original dust pool exhausted")?;
        let capacity = (arguments[5] as i32).min(500);
        let capacity = usize::try_from(capacity).map_err(|_| "negative dust capacity")?;
        let origin = [arguments[1], arguments[2]].map(f32::from_bits);
        let extent = [arguments[3], arguments[4]].map(f32::from_bits);
        if !origin.into_iter().chain(extent).all(f32::is_finite) {
            return Err("nonfinite dust emitter bounds".into());
        }
        self.slots[index] = Effect {
            model: arguments[0],
            state: 0,
            origin,
            extent,
            live: 0,
            positions: vec![[0.; 3]; capacity],
            particles: vec![[0, 0, 0, u32::MAX, 0]; capacity],
        };
        self.count = self.count.wrapping_add(1);
        Ok(index as u32)
    }
    pub fn release(&mut self, index: u32) -> Result<(), String> {
        if index == u32::MAX {
            return Ok(());
        }
        let effect = self
            .slots
            .get_mut(index as usize)
            .ok_or("dust release outside pool")?;
        effect.state = 2;
        self.count = self.count.wrapping_sub(1);
        Ok(())
    }
    pub fn advance(&mut self, image: &PeImage<'_>) -> Result<(), String> {
        let multiplier = image.f32_at(0x52aef0)?;
        let half = image.f32_at(0x52502c)?;
        let drift = image.f32_at(0x52508c)?;
        let rise = image.f32_at(0x525b60)?;
        let random = &mut self.random;
        let mut sample = || -> Result<Sum, String> {
            *random = random.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            let mut value = Sum::from_i32(((*random >> 16) & 0x7fff) as i32);
            value.mul_f32(multiplier)?;
            Ok(value)
        };
        for effect in &mut self.slots {
            if effect.state == 1 {
                continue;
            }
            if effect.state == 2 && effect.live == 0 {
                effect.state = 1;
                // Native frees buffers while retaining the other emitter words.
                effect.positions.fill([0.; 3]);
                effect.particles.fill([0; 5]);
                continue;
            }
            for (position, particle) in effect.positions.iter_mut().zip(&mut effect.particles) {
                if particle[3] == u32::MAX {
                    continue;
                }
                for axis in 0..3 {
                    position[axis] = (f64::from(position[axis])
                        + f64::from(f32::from_bits(particle[axis])))
                        as f32;
                }
                particle[3] = particle[3].wrapping_add(1);
                if particle[3] == particle[4] {
                    particle[3] = u32::MAX;
                    effect.live = effect.live.wrapping_sub(1);
                }
            }
            if effect.state != 0 || effect.live == effect.particles.len() as u32 {
                continue;
            }
            let slot = effect
                .particles
                .iter()
                .position(|particle| particle[3] == u32::MAX)
                .ok_or("dust live count disagrees with particles")?;
            let mut x = sample()?;
            x.mul_f32(effect.extent[0])?;
            x.add_f32(effect.origin[0])?;
            let mut z = sample()?;
            z.mul_f32(effect.extent[1])?;
            z.add_f32(effect.origin[1])?;
            effect.positions[slot] = [x.to_f32(), 1., z.to_f32()];
            let mut vx = sample()?;
            vx.add_f32(-half)?;
            vx.mul_f32(drift)?;
            let mut vy = sample()?;
            vy.mul_f32(rise)?;
            vy.add_f32(rise)?;
            let mut vz = sample()?;
            vz.add_f32(-half)?;
            vz.mul_f32(drift)?;
            effect.particles[slot] = [
                vx.to_f32().to_bits(),
                vy.to_f32().to_bits(),
                vz.to_f32().to_bits(),
                0,
                128,
            ];
            effect.live = effect.live.wrapping_add(1);
        }
        Ok(())
    }
    /// Logical native positions.4eb450 negates Z at the renderer boundary.
    pub fn points(&self, image: &PeImage<'_>) -> Result<Vec<Point>, String> {
        let size = image.f32_at(0x525210)?;
        let mut points = vec![];
        for (slot, effect) in self.slots.iter().enumerate() {
            if effect.state == 1 {
                continue;
            }
            for (&position, particle) in effect.positions.iter().zip(&effect.particles) {
                if particle[3] != u32::MAX {
                    points.push(Point {
                        position,
                        argb: 0x00ff_ffffu32.wrapping_sub(particle[3].wrapping_shl(25)),
                        size: (f64::from(particle[3] as i32) * f64::from(size)) as f32,
                        model: effect.model,
                        slot: slot as u8,
                    });
                }
            }
        }
        Ok(points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires pinned v1.4 native dust allocator/update/draw observation"]
    fn matches_native_dust_lifetimes() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_DEMOLITION_EFFECT")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut at = 8;
        let read = |at: &mut usize| {
            let x = u32::from_le_bytes(data[*at..*at + 4].try_into().unwrap());
            *at += 4;
            x
        };
        assert_eq!(&data[..8], b"MTDUST01");
        assert_eq!(read(&mut at), 0x037f);
        for case in 0..read(&mut at) {
            let capacity = read(&mut at);
            let release = read(&mut at);
            let seed = read(&mut at);
            let mut args = [0; 6];
            for argument in &mut args[1..5] {
                *argument = read(&mut at);
            }
            args[5] = capacity;
            let frames = read(&mut at);
            let mut manager = Manager::new(seed);
            assert_eq!(manager.create(args).unwrap(), 0);
            if capacity == 60 && release == 2 {
                assert_eq!(manager.create(args).unwrap(), 1);
            }
            for tick in 0..frames {
                if tick == release {
                    manager.release(0).unwrap();
                }
                manager.advance(&image).unwrap();
                // Re-enter through the portable save boundary at every
                // observed native phase, including released and retired slots.
                manager = crate::save::demolition::decode(
                    &crate::save::demolition::encode(&manager).unwrap(),
                )
                .unwrap();
                assert_eq!(manager.count, read(&mut at), "count case{case} tick{tick}");
                assert_eq!(manager.random, read(&mut at), "RNG case{case} tick{tick}");
                for (index, effect) in manager.slots.iter().enumerate() {
                    let row: [u32; 10] = std::array::from_fn(|_| read(&mut at));
                    assert_eq!(
                        row,
                        [
                            effect.model,
                            effect.state,
                            effect.origin[0].to_bits(),
                            effect.origin[1].to_bits(),
                            effect.extent[0].to_bits(),
                            effect.extent[1].to_bits(),
                            effect.positions.len() as u32,
                            effect.live,
                            0,
                            0
                        ],
                        "row case{case} tick{tick} slot{index}"
                    );
                    for position in &effect.positions {
                        assert_eq!(
                            position.map(f32::to_bits),
                            std::array::from_fn(|_| read(&mut at)),
                            "position case{case} tick{tick}"
                        );
                    }
                    for particle in &effect.particles {
                        assert_eq!(
                            *particle,
                            std::array::from_fn(|_| read(&mut at)),
                            "particle case{case} tick{tick}"
                        );
                    }
                    let native_count = read(&mut at);
                    let mut one = Manager::new(0);
                    one.slots[0] = effect.clone();
                    let points = one.points(&image).unwrap();
                    assert_eq!(native_count, points.len() as u32);
                    for point in points {
                        assert_eq!(
                            [
                                point.position[0].to_bits(),
                                point.position[1].to_bits(),
                                (-point.position[2]).to_bits(),
                                point.argb,
                                point.size.to_bits()
                            ],
                            std::array::from_fn(|_| read(&mut at)),
                            "draw case{case} tick{tick}"
                        );
                    }
                }
            }
        }
        assert_eq!(at, data.len());
    }
}
