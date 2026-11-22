//! Default building texture selection from original v1.4 tables and 00488050.
//! Runtime custom-texture overrides are not represented here.
use crate::pe::PeImage;

/// Main facade state 6/7 selected by 004b7d90. Texture indexes refer to
/// BuildingTextures, not the procedural BuildTextures collection.
#[derive(Clone, Copy, Debug)]
pub struct FacadeMaterial {
    pub texture: usize,
    pub diffuse_alpha: u8,
    pub camera_coordinates: bool,
}

impl FacadeMaterial {
    /// Feature gates correspond to the original 0045a160/0045a190 checks.
    /// None selects the opaque state-2 fallback.
    pub fn select(diffuse_sun: u32, night_enabled: bool, day_enabled: bool) -> Option<Self> {
        let factor = (diffuse_sun & 255) * 2;
        if factor < 127 {
            night_enabled.then_some(Self {
                texture: 26,
                diffuse_alpha: (255 - factor) as u8,
                camera_coordinates: false,
            })
        } else {
            day_enabled.then_some(Self {
                texture: 0,
                diffuse_alpha: (factor / 6) as u8,
                camera_coordinates: true,
            })
        }
    }

    /// 004f2810/004e3770: daylight uses the mesh's stored positions with the
    /// CPU camera's contiguous 3x3 rows, with no translation term. Night copies UV1.
    pub fn coordinates(&self, camera: &[f32; 12], position: [f32; 3], uv1: [f32; 2]) -> [f32; 2] {
        if !self.camera_coordinates {
            return uv1;
        }
        let [x, y, z] = position.map(f64::from);
        std::array::from_fn(|i| {
            let at = i * 3;
            let projected = ((x * f64::from(camera[at]) + y * f64::from(camera[at + 1]))
                + z * f64::from(camera[at + 2])) as f32;
            (f64::from(projected) * f64::from(0.02f32)) as f32
        })
    }
}

pub struct BuildingMaterials {
    slots: [Option<usize>; 720],
}

#[derive(Debug, PartialEq, Eq)]
pub struct MaterialSelection {
    /// Missing slots use an engine-global fallback handle in the original.
    /// Resolving that handle requires the complete texture registry.
    pub facade_base: Option<usize>,
    pub flat_roof: usize,
    pub pitched_roof: usize,
    pub ground: usize,
    pub random_state: u32,
}

impl BuildingMaterials {
    pub fn from_v14(executable: &[u8]) -> Result<Self, String> {
        let image = PeImage::parse(executable)?;
        let available = image.at(0x54605c, 45)?;
        let mut slots = [None; 720];
        let mut texture = 0;
        for (group, &present) in available.iter().enumerate() {
            if present != 0 {
                for slot in &mut slots[group * 16..group * 16 + 16] {
                    *slot = Some(texture);
                    texture += 1;
                }
            }
        }
        if texture != 480 {
            return Err("unexpected v1.4 building texture availability table".into());
        }
        Ok(Self { slots })
    }

    pub fn select(&self, variation: u32) -> MaterialSelection {
        // 00488230 supplies only the low sixteen bits to 00488050.
        let seed = variation & 0xffff;
        let first = seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        let second = first.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        MaterialSelection {
            facade_base: self.slots[(seed as usize % 720).min(704)],
            flat_roof: ((first >> 16) & 0x7fff) as usize % 6,
            pitched_roof: 6 + ((second >> 16) & 1) as usize,
            ground: 8,
            random_state: second,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original two-texture facade vertex observations"]
    fn matches_native_facade_coordinates() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_FACADE_VERTICES").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(&bytes[..8], b"M4FV003\0");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let mode = word(at);
            assert!(mode == 0 || mode == 2);
            let count = word(at + 4) as usize;
            let camera = std::array::from_fn(|i| float(at + 8 + i * 4));
            let material = FacadeMaterial {
                texture: if mode == 0 { 26 } else { 0 },
                diffuse_alpha: word(at + 68) as u8,
                camera_coordinates: mode == 2,
            };
            let local = at + 72;
            let world = local + count * 12;
            let uv0 = world + count * 24;
            let uv1 = uv0 + count * 8;
            let output = uv1 + count * 8 + 4;
            assert_eq!(word(output - 4) as usize, count * 36);
            for i in 0..count {
                let position = std::array::from_fn(|c| float(local + i * 12 + c * 4));
                let baked_uv = std::array::from_fn(|c| float(uv1 + i * 8 + c * 4));
                let actual = material.coordinates(&camera, position, baked_uv);
                for (c, value) in actual.iter().enumerate() {
                    assert_eq!(
                        value.to_bits(),
                        word(output + i * 36 + 28 + c * 4),
                        "case{case} vertex{i} UV1.{c}"
                    );
                }
                // The native output distinguishes pretransformed positions from
                // the mesh-local position used exclusively for generated UV1.
                for c in 0..3 {
                    assert_eq!(word(output + i * 36 + c * 4), word(world + i * 12 + c * 4));
                }
                for c in 0..2 {
                    assert_eq!(
                        word(output + i * 36 + 20 + c * 4),
                        word(uv0 + i * 8 + c * 4)
                    );
                }
                assert_eq!(
                    word(output + i * 36 + 12) >> 24,
                    u32::from(material.diffuse_alpha)
                );
            }
            at = output + count * 36;
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original facade state-selection observations"]
    fn matches_native_facade_materials() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_FACADE_MATERIALS").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTFMAT01");
        assert_eq!(bytes.len(), 12 + word(8) as usize * 20);
        for case in 0..word(8) as usize {
            let at = 12 + case * 20;
            let gates = word(at + 4);
            let material = FacadeMaterial::select(word(at), gates & 1 != 0, gates & 2 != 0);
            if let Some(material) = material {
                assert_eq!(
                    if material.camera_coordinates { 7 } else { 6 },
                    word(at + 8),
                    "case{case} state"
                );
                assert_eq!(material.texture as u32, word(at + 12), "case{case} texture");
                assert_eq!(
                    u32::from(material.diffuse_alpha),
                    word(at + 16),
                    "case{case} alpha"
                );
            } else {
                assert_eq!(word(at + 8), 2, "case{case} fallback");
                assert_eq!(word(at + 12), u32::MAX, "case{case} texture unbound");
            }
        }
    }
    #[test]
    #[ignore = "requires original v1.4 executable and native material observations"]
    fn matches_native_selection() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let file = read("OPENMT2001_EXE");
        let native = read("OPENMT2001_NATIVE_MATERIALS");
        let materials = BuildingMaterials::from_v14(&file).unwrap();
        assert_eq!(&native[..8], b"MTMATER1");
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(native.len(), 12 + word(8) as usize * 24);
        for case in 0..word(8) as usize {
            let at = 12 + case * 24;
            let seed = word(at);
            let selected = materials.select(seed);
            assert_eq!(
                selected.facade_base.map(|i| i as u32 + 1000).unwrap_or(240),
                word(at + 4),
                "seed {seed}"
            );
            assert_eq!(selected.flat_roof as u32 + 2000, word(at + 8));
            assert_eq!(selected.pitched_roof as u32 + 2000, word(at + 12));
            assert_eq!(selected.ground as u32 + 2000, word(at + 16));
            assert_eq!(selected.random_state, word(at + 20));
        }
    }
}
