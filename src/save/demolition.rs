//! Portable original dust pool continuation. Renderer handles are not stored.
use crate::demolition_effect::{Effect, Manager};

fn validate(manager: &Manager) -> Result<(), String> {
    if manager.count != manager.slots.iter().filter(|s| s.state == 0).count() as u32 {
        return Err("dust allocation count disagrees with slots".into());
    }
    for slot in &manager.slots {
        let capacity = slot.positions.len();
        if slot.model > 1 || slot.state > 2 || capacity > 500
            || capacity != slot.particles.len() || slot.live as usize > capacity {
            return Err("unsupported dust slot continuation".into());
        }
        // Retired buffers retain capacity but their zeroed ages are not live.
        if slot.state != 1 && slot.live as usize != slot.particles.iter()
            .filter(|particle| particle[3] != u32::MAX).count() {
            return Err("dust live count disagrees with particles".into());
        }
    }
    Ok(())
}

pub fn encode(manager: &Manager) -> Result<Vec<u8>, String> {
    validate(manager)?;
    let mut bytes = Vec::new();
    let mut word = |value: u32| bytes.extend_from_slice(&value.to_le_bytes());
    word(manager.count); word(manager.random);
    for slot in &manager.slots {
        for value in [slot.model, slot.state, slot.origin[0].to_bits(), slot.origin[1].to_bits(),
            slot.extent[0].to_bits(), slot.extent[1].to_bits(), slot.positions.len() as u32, slot.live] {
            word(value);
        }
        for position in &slot.positions { for value in position { word(value.to_bits()); } }
        for particle in &slot.particles { for value in particle { word(*value); } }
    }
    Ok(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<Manager, String> {
    let mut at = 0;
    let mut word = || -> Result<u32, String> {
        let row = bytes.get(at..at + 4).ok_or("truncated dust continuation")?;
        at += 4;
        Ok(u32::from_le_bytes(row.try_into().unwrap()))
    };
    let count = word()?;
    let mut manager = Manager::new(word()?);
    manager.count = count;
    for slot in &mut manager.slots {
        let model = word()?; let state = word()?;
        let origin = [f32::from_bits(word()?), f32::from_bits(word()?)];
        let extent = [f32::from_bits(word()?), f32::from_bits(word()?)];
        let capacity = word()? as usize; let live = word()?;
        if capacity > 500 { return Err("dust capacity exceeds original pool limit".into()); }
        let mut positions = Vec::with_capacity(capacity);
        let mut particles = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            positions.push([f32::from_bits(word()?), f32::from_bits(word()?), f32::from_bits(word()?)]);
        }
        for _ in 0..capacity { particles.push([word()?, word()?, word()?, word()?, word()?]); }
        *slot = Effect { model, state, origin, extent, live, positions, particles };
    }
    if at != bytes.len() { return Err("trailing dust continuation bytes".into()); }
    validate(&manager)?;
    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_models_and_inconsistent_slots() {
        let mut manager = Manager::new(17);
        manager.create([1, 0, 0, 1f32.to_bits(), 2f32.to_bits(), 3]).unwrap();
        let bytes = encode(&manager).unwrap();
        assert_eq!(encode(&decode(&bytes).unwrap()).unwrap(), bytes);
        let mut truncated = bytes.clone(); truncated.pop();
        assert!(decode(&truncated).is_err());
        manager.slots[0].model = 2;
        assert!(encode(&manager).is_err());
        manager.slots[0].model = 1; manager.count = 0;
        assert!(encode(&manager).is_err());
    }

    #[test]
    #[ignore = "requires original v1.4 particle constants"]
    fn live_and_retired_dust_codec_continues_original_updates() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut uninterrupted = Manager::new(0x12345678);
        let index = uninterrupted.create([1, 3f32.to_bits(), 4f32.to_bits(),
            5f32.to_bits(), 6f32.to_bits(), 27]).unwrap();
        for _ in 0..19 { uninterrupted.advance(&image).unwrap(); }
        let mut resumed = decode(&encode(&uninterrupted).unwrap()).unwrap();
        assert_eq!(uninterrupted.points(&image).unwrap(), resumed.points(&image).unwrap());
        for tick in 0..160 {
            if tick == 12 { uninterrupted.release(index).unwrap(); resumed.release(index).unwrap(); }
            uninterrupted.advance(&image).unwrap(); resumed.advance(&image).unwrap();
            assert_eq!(encode(&uninterrupted).unwrap(), encode(&resumed).unwrap());
            assert_eq!(uninterrupted.points(&image).unwrap(), resumed.points(&image).unwrap());
        }
        assert!(!uninterrupted.allocated());
        assert_eq!(uninterrupted.slots[0].positions.len(), 27);
        let retired = decode(&encode(&uninterrupted).unwrap()).unwrap();
        assert_eq!(retired.random, uninterrupted.random);
        assert_eq!(encode(&retired).unwrap(), encode(&uninterrupted).unwrap());
    }
}
