//! Original 0044d0a0 camera-distance state; sound submission belongs to the host.
use super::{difference, extended, float, record_index, word};

#[derive(Default)]
pub struct Proximity {
    /// Distance bits, retained zero word, and vehicle speed bits (+28).
    pub nearest: [[u32; 3]; 3],
    pub reciprocal_sum: f32,
    pub count: usize,
}

impl Proximity {
    pub fn update(
        &mut self,
        vehicles: &[[u8; 208]; 200],
        auxiliaries: &[[u8; 284]],
        auxiliary_base: u32,
        camera: [f32; 3],
    ) -> Result<(), String> {
        self.count = 0;
        self.reciprocal_sum = 0.;
        // The original only resets the distance words; unused payloads survive.
        for row in &mut self.nearest {
            row[0] = 101_f32.to_bits();
        }
        for vehicle in vehicles {
            if word(vehicle, 0x1c) != 9 {
                continue;
            }
            let index = record_index(auxiliaries, auxiliary_base, word(vehicle, 0xc4))?;
            let auxiliary = &auxiliaries[index];
            let mut squares = [extended(0.)?; 3];
            for axis in 0..3 {
                let delta = difference(float(auxiliary, 4 + axis * 4), camera[axis])?;
                squares[axis] = delta;
                squares[axis].mul(delta);
            }
            let mut distance = squares[2];
            distance.add(squares[1]);
            distance.add(squares[0]);
            distance = distance.sqrt()?;
            if !distance.compare(extended(100.)?).is_lt() {
                continue;
            }
            if distance.compare(extended(10.)?).is_lt() {
                distance = extended(10.)?;
            }
            let mut weight = extended(1.)?;
            weight
                .div(distance)
                .map_err(|e| format!("road sound distance attenuation: {e}"))?;
            weight.add_f32(self.reciprocal_sum)?;
            self.reciprocal_sum = weight.to_f32();
            let index = (0..self.count)
                .find(|&i| {
                    distance
                        .compare(extended(f32::from_bits(self.nearest[i][0])).unwrap())
                        .is_lt()
                })
                .unwrap_or(self.count);
            if index == 3 {
                continue;
            }
            for i in (index + 1..3).rev() {
                self.nearest[i] = self.nearest[i - 1];
            }
            self.nearest[index] = [distance.to_f32().to_bits(), 0, word(vehicle, 0x28)];
            self.count = (self.count + 1).min(3);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::put;
    #[test]
    #[ignore = "requires original vehicle proximity corpus"]
    fn matches_original_proximity() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_VEHICLE_AUDIO")).unwrap();
        assert_eq!(&data[..8], b"MTVAUD01");
        let mut at = 16;
        for _ in 0..word(&data, 12) {
            let header = &data[at..at + 32];
            at += 32;
            let mut state = Proximity::default();
            for row in &mut state.nearest {
                for value in row {
                    *value = word(&data, at);
                    at += 4;
                }
            }
            let expected = &data[at..at + 36];
            at += 36;
            let writes = word(&data, at + 8) as usize;
            at += 12;
            let mut vehicles = [[0; 208]; 200];
            let mut auxiliaries = [[0; 284]; 200];
            for _ in 0..word(header, 4) {
                let slot = word(&data, at) as usize;
                put(&mut vehicles[slot], 0x1c, word(&data, at + 4));
                put(&mut vehicles[slot], 0x28, word(&data, at + 8));
                put(&mut vehicles[slot], 0xc4, 0x10020000 + slot as u32 * 284);
                auxiliaries[slot][4..16].copy_from_slice(&data[at + 12..at + 24]);
                at += 24;
            }
            at += writes * 12;
            state
                .update(
                    &vehicles,
                    &auxiliaries,
                    0x10020000,
                    std::array::from_fn(|i| float(header, 8 + i * 4)),
                )
                .unwrap();
            assert_eq!(state.count, word(header, 20) as usize);
            assert_eq!(state.reciprocal_sum.to_bits(), word(header, 24));
            for i in 0..9 {
                assert_eq!(state.nearest[i / 3][i % 3], word(expected, i * 4));
            }
        }
        assert_eq!(at, data.len());
    }
}
