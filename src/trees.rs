//! Original city tree pool (0047e370) and foliage point producer (004b6370).
//! Geometry/material submission is separate. Cached locator positions are
//! copied verbatim into the pool; the rendering consumers negate Z once.

use crate::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage, x87::Sum};

pub mod building;

pub const CAPACITY: usize = 500;
pub const LOCATOR_PATH: &str = "gamedata2/trees/tree_locators.tm";
pub const TRUNK_PATH: &str = "gamedata2/trees/tree_trunk.tm";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub position: [f32; 3],
    pub scale: [f32; 3],
}

impl Slot {
    fn validate(&self) -> Result<(), String> {
        if self
            .position
            .iter()
            .chain(&self.scale)
            .all(|v| v.is_finite())
        {
            Ok(())
        } else {
            Err("non-finite tree slot pose".into())
        }
    }
}

#[derive(Clone)]
pub struct Pool {
    slots: [Option<Slot>; CAPACITY],
    count: usize,
}

impl Default for Pool {
    fn default() -> Self {
        Self {
            slots: [None; CAPACITY],
            count: 0,
        }
    }
}

impl Pool {
    /// 0047e370 initializes ambient rows independently of saved buildings.
    pub fn ambient(locators: &[[f32; 3]]) -> Result<Self, String> {
        if locators.len() > CAPACITY {
            return Err("tree locators exceed shared pool".into());
        }
        let mut pool = Self::default();
        for &position in locators {
            pool.allocate(Slot {
                position,
                scale: [1.; 3],
            })?;
        }
        Ok(pool)
    }

    pub fn count(&self) -> usize {
        self.count
    }
    pub fn slot(&self, index: usize) -> Option<&Slot> {
        self.slots.get(index)?.as_ref()
    }

    /// First-free allocation shared with building-owned trees. Exhaustion is
    /// reported rather than reproducing the original out-of-bounds write.
    pub fn allocate(&mut self, slot: Slot) -> Result<usize, String> {
        slot.validate()?;
        let index = self
            .slots
            .iter()
            .position(Option::is_none)
            .ok_or("tree pool exhausted")?;
        self.slots[index] = Some(slot);
        self.count += 1;
        Ok(index)
    }

    pub fn set_pose(&mut self, index: usize, slot: Slot) -> Result<(), String> {
        slot.validate()?;
        let target = self
            .slots
            .get_mut(index)
            .filter(|s| s.is_some())
            .ok_or("inactive tree slot")?;
        *target = Some(slot);
        Ok(())
    }

    pub fn release(&mut self, index: usize) -> Result<(), String> {
        let target = self
            .slots
            .get_mut(index)
            .filter(|s| s.is_some())
            .ok_or("inactive tree slot")?;
        *target = None;
        self.count -= 1;
        Ok(())
    }

    /// The original pass scans below captured occupancy, checking activation;
    /// occupancy is not the highest live index after releases. Preserve that
    /// distinction instead of silently replacing the native walk with all500.
    pub fn submission_slots(&self) -> impl Iterator<Item = (usize, &Slot)> {
        self.slots[..self.count]
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|s| (i, s)))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Attachment {
    pub position: [f32; 3],
    pub kind: u32,
}

pub struct Tables {
    angles: [f32; 8],
    multipliers: [f32; 8],
    sine: [f32; 8],
    cosine: [f32; 8],
    colors: [u32; 9],
}

#[derive(Clone, Copy, Debug)]
pub struct Variation {
    pub yaw_degrees: f32,
    pub scale: [f32; 3],
    pub sine: f32,
    pub cosine: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    /// Foliage texture collection row is5+phase; phase is slot index modulo3.
    pub phase: usize,
    pub attachment: usize,
    pub position: [f32; 3],
    /// Collector scale2 makes diameter4*effective slot width.
    pub radius: f32,
    pub color: u32,
}

impl Tables {
    pub fn from_v14(image: &PeImage<'_>) -> Result<Self, String> {
        let mut angles = [0.; 8];
        let mut multipliers = [0.; 8];
        for i in 0..8 {
            angles[i] = image.f32_at(0x545614 + i as u32 * 4)?;
            multipliers[i] = image.f32_at(0x545634 + i as u32 * 4)?;
        }
        if !angles.iter().chain(&multipliers).all(|v| v.is_finite()) {
            return Err("non-finite original tree variation table".into());
        }
        let mut colors = [0; 9];
        for (i, color) in colors.iter_mut().enumerate() {
            *color = u32::from_le_bytes(image.at(0x5455f0 + i as u32 * 4, 4)?.try_into().unwrap());
        }
        Ok(Self {
            angles,
            multipliers,
            sine: angles.map(|v| f64::from(v).sin() as f32),
            cosine: angles.map(|v| f64::from(v).cos() as f32),
            colors,
        })
    }

    pub fn variation(&self, index: usize, slot: &Slot) -> Result<Variation, String> {
        slot.validate()?;
        let i = index & 7;
        Ok(Variation {
            yaw_degrees: (f64::from(self.angles[i]) * f64::from(57.29578_f32)) as f32,
            scale: std::array::from_fn(|axis| {
                (f64::from(self.multipliers[(index + axis) & 7]) * f64::from(slot.scale[axis]))
                    as f32
            }),
            sine: self.sine[i],
            cosine: self.cosine[i],
        })
    }

    /// Original camera target-minus-eye normalization, dot order and palette
    /// channel spills. Inputs use the native camera/sun coordinate convention.
    pub fn palette(
        &self,
        eye: [f32; 3],
        target: [f32; 3],
        sun: [f32; 3],
        high: u32,
        mid: u32,
    ) -> Result<[u32; 9], String> {
        let delta =
            std::array::from_fn::<_, 3, _>(|i| (f64::from(target[i]) - f64::from(eye[i])) as f32);
        let product = |a, b| -> Result<Sum, String> {
            let mut v = Sum::from_f32(a)?;
            v.mul_f32(b)?;
            Ok(v)
        };
        let mut square = product(delta[0], delta[0])?;
        square.add(product(delta[1], delta[1])?);
        square.add(product(delta[2], delta[2])?);
        let mut reciprocal = Sum::from_f32(1.)?;
        reciprocal.div(square.sqrt()?)?;
        let mut direction = [0.; 3];
        for i in 0..3 {
            let mut v = reciprocal;
            v.mul_f32(delta[i])?;
            direction[i] = v.to_f32();
        }
        let mut dot = product(direction[2], sun[2])?;
        dot.add(product(direction[1], sun[1])?);
        dot.add(product(direction[0], sun[0])?);
        let mut factor = dot.negated();
        factor.mul_f32(0.5)?;
        factor.add_f32(0.5)?;
        let mut coefficients = [Sum::default(); 3];
        for (i, shift) in [16, 8, 0].into_iter().enumerate() {
            let mut v = factor;
            v.mul(Sum::from_i32(((high >> shift) & 255) as i32));
            v.add(Sum::from_i32(((mid >> shift) & 255) as i32));
            v.mul_f32(1. / 256.)?;
            coefficients[i] = if i < 2 { Sum::from_f32(v.to_f32())? } else { v };
        }
        Ok(self.colors.map(|color| {
            let channels = std::array::from_fn::<_, 3, _>(|i| {
                let mut v = coefficients[i];
                v.mul(Sum::from_i32(((color >> [16, 8, 0][i]) & 255) as i32));
                v.ftol_low_word().min(255) as u32
            });
            channels[0]
                .wrapping_sub(256)
                .wrapping_shl(16)
                .wrapping_add(channels[1].wrapping_shl(8))
                .wrapping_add(channels[2])
        }))
    }

    pub fn append_points(
        &self,
        index: usize,
        slot: &Slot,
        submission: i32,
        attachments: &[Attachment],
        palette: &[u32; 9],
        points: &mut Vec<Point>,
    ) -> Result<(), String> {
        if submission == 0 {
            return Ok(());
        }
        let variation = self.variation(index, slot)?;
        let kind = if submission < 300 { 1 } else { 2 };
        for (j, attachment) in attachments
            .iter()
            .enumerate()
            .filter(|(_, a)| a.kind == kind)
        {
            let [x, y, z] = attachment.position;
            let multiply = |a, b| -> Result<Sum, String> {
                let mut v = Sum::from_f32(a)?;
                v.mul_f32(b)?;
                Ok(v)
            };
            let mut px = multiply(x, variation.cosine)?;
            px.add(multiply(z, variation.sine)?.negated());
            px.mul_f32(variation.scale[0])?;
            px.add_f32(slot.position[0])?;
            let mut py = multiply(y, variation.scale[1])?;
            py.add_f32(slot.position[1])?;
            let mut pz = multiply(z, variation.cosine)?;
            pz.add(multiply(x, variation.sine)?);
            pz.mul_f32(variation.scale[2])?;
            pz.add_f32(slot.position[2])?;
            points.push(Point {
                phase: index % 3,
                attachment: j,
                position: [px.to_f32(), py.to_f32(), -pz.to_f32()],
                radius: (f64::from(variation.scale[0]) * 2.) as f32,
                color: palette[(index + j) & 7],
            });
        }
        Ok(())
    }
}

pub struct Assets {
    pub locators: Vec<[f32; 3]>,
    pub attachments: Vec<Attachment>,
    pub tables: Tables,
}

impl Assets {
    pub fn load(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let mesh = |path| -> Result<CachedMesh<'_>, String> {
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing original tree asset {path}"))?;
            let end = entry
                .offset
                .checked_add(entry.length)
                .ok_or("tree asset range overflow")?;
            let bytes = archive
                .get(
                    entry.offset as usize
                        ..usize::try_from(end).map_err(|_| "tree asset range overflow")?,
                )
                .ok_or("tree asset outside archive")?;
            CachedMesh::parse(bytes)
        };
        let locators = mesh(LOCATOR_PATH)?;
        if locators.vertex_count != 0 || locators.opaque_records_28.len() / 28 > CAPACITY {
            return Err("unsupported original tree locator model".into());
        }
        let position = |record: &[u8]| -> Result<[f32; 3], String> {
            let v = std::array::from_fn(|i| {
                f32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap())
            });
            if v.iter().all(|f| f.is_finite()) {
                Ok(v)
            } else {
                Err("non-finite tree locator".into())
            }
        };
        let locators = locators
            .opaque_records_28
            .chunks_exact(28)
            .map(position)
            .collect::<Result<_, _>>()?;
        let trunk = mesh(TRUNK_PATH)?;
        let attachments = trunk
            .opaque_records_28
            .chunks_exact(28)
            .map(|record| {
                Ok(Attachment {
                    position: position(record)?,
                    kind: u32::from_le_bytes(record[24..28].try_into().unwrap()),
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            locators,
            attachments,
            tables: Tables::from_v14(image)?,
        })
    }
}

/// 004b6655 FISTP result: zero suppresses leaves, rounded depth300 switches type.
pub fn submission_depth(visible: bool, model_center_depth: f32) -> Result<i32, String> {
    if !visible {
        return Ok(0);
    }
    let depth = f64::from(model_center_depth).round_ties_even();
    if !depth.is_finite() || depth < f64::from(i32::MIN) || depth > f64::from(i32::MAX) {
        return Err("tree submission depth outside finite i32 range".into());
    }
    Ok(depth as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> Tables {
        Tables {
            angles: [0.; 8],
            multipliers: [1.; 8],
            sine: [0.; 8],
            cosine: [1.; 8],
            colors: [0xff806040; 9],
        }
    }

    #[test]
    fn pool_allocation_release_and_native_submission_bound() {
        let mut pool = Pool::ambient(&[[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]]).unwrap();
        assert_eq!(pool.slot(2).unwrap().scale, [1.; 3]);
        pool.release(1).unwrap();
        assert_eq!(pool.count(), 2);
        assert_eq!(
            pool.submission_slots().map(|(i, _)| i).collect::<Vec<_>>(),
            [0]
        );
        assert_eq!(
            pool.allocate(Slot {
                position: [0.; 3],
                scale: [0.75; 3]
            })
            .unwrap(),
            1
        );
        assert_eq!(pool.submission_slots().count(), 3);
        assert!(
            pool.set_pose(
                4,
                Slot {
                    position: [0.; 3],
                    scale: [1.; 3]
                }
            )
            .is_err()
        );
        assert!(Pool::ambient(&[[f32::NAN, 0., 0.]]).is_err());
        let mut full = Pool::ambient(&vec![[0.; 3]; CAPACITY]).unwrap();
        assert!(
            full.allocate(Slot {
                position: [0.; 3],
                scale: [1.; 3]
            })
            .is_err()
        );
        assert_eq!(full.count(), CAPACITY);
    }

    #[test]
    fn foliage_type_radius_color_and_single_z_conversion() {
        let tables = tables();
        let slot = Slot {
            position: [10., 20., 30.],
            scale: [2., 3., 4.],
        };
        let attachments = [
            Attachment {
                position: [1., 2., 3.],
                kind: 1,
            },
            Attachment {
                position: [2., 1., -1.],
                kind: 2,
            },
            Attachment {
                position: [0.; 3],
                kind: 4,
            },
        ];
        let mut points = Vec::new();
        tables
            .append_points(
                1,
                &slot,
                299,
                &attachments,
                &[1, 2, 3, 4, 5, 6, 7, 8, 9],
                &mut points,
            )
            .unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].position, [12., 26., -42.]);
        assert_eq!(
            (
                points[0].phase,
                points[0].attachment,
                points[0].radius,
                points[0].color
            ),
            (1, 0, 4., 2)
        );
        points.clear();
        tables
            .append_points(1, &slot, 300, &attachments, &[0; 9], &mut points)
            .unwrap();
        assert_eq!(points[0].position, [14., 23., -26.]);
        points.clear();
        tables
            .append_points(1, &slot, 0, &attachments, &[0; 9], &mut points)
            .unwrap();
        assert!(points.is_empty());
        assert_eq!(submission_depth(true, 299.5).unwrap(), 300);
        assert_eq!(submission_depth(true, 0.5).unwrap(), 0);
        assert_eq!(submission_depth(false, 500.).unwrap(), 0);
    }

    #[test]
    fn foliage_palette_uses_camera_sun_and_native_packing() {
        let tables = tables();
        assert_eq!(
            tables
                .palette([0.; 3], [0., 0., 1.], [0., 0., 1.], 0xff808080, 0)
                .unwrap(),
            [0xff000000; 9]
        );
        assert_eq!(
            tables
                .palette([0.; 3], [0., 0., 1.], [0., 0., -1.], 0xff808080, 0)
                .unwrap(),
            [0xff403020; 9]
        );
        assert_eq!(
            tables
                .palette([0.; 3], [0., 0., 1.], [1., 0., 0.], 0xff808080, 0)
                .unwrap(),
            [0xff201810; 9]
        );
        assert!(
            tables
                .palette([0.; 3], [0.; 3], [1., 0., 0.], 0, 0)
                .is_err()
        );
    }

    #[test]
    fn foliage_palette_white_entry_is_lit_light_grey_for_chimney_smoke() {
        // Entry 8 of the original 005455f0 table is opaque white; chimney
        // smoke (004b60d0) takes its lit RGB from 008b2248.
        let mut tables = tables();
        tables.colors[8] = 0xffffffff;
        let lit = |sun| {
            tables
                .palette([0.; 3], [0., 0., 1.], sun, 0xff808080, 0xff808080)
                .unwrap()[8]
        };
        assert_eq!(lit([0., 0., -1.]), 0xffffffff);
        assert_eq!(lit([1., 0., 0.]), 0xffbfbfbf);
        assert_eq!(lit([0., 0., 1.]), 0xff7f7f7f);
    }

    #[test]
    #[ignore = "requires supplied original archive and v1.4 executable"]
    fn original_tree_pool_and_foliage_records() {
        let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            ArchiveIndex::parse(&read("OPENMT2001_ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let assets = Assets::load(&PeImage::parse(&exe).unwrap(), &index, &archive).unwrap();
        assert_eq!((assets.locators.len(), assets.attachments.len()), (137, 75));
        for (kind, count) in [(1, 44), (2, 23), (4, 8)] {
            assert_eq!(
                assets.attachments.iter().filter(|a| a.kind == kind).count(),
                count
            );
        }
        assert_eq!(assets.locators[0], [-22.222939, 0., -0.706377]);
        let pool = Pool::ambient(&assets.locators).unwrap();
        let mut points = Vec::new();
        for (i, slot) in pool.submission_slots() {
            assert_eq!(
                slot.position.map(f32::to_bits),
                assets.locators[i].map(f32::to_bits)
            );
            assets
                .tables
                .append_points(
                    i,
                    slot,
                    100,
                    &assets.attachments,
                    &[0xff000000; 9],
                    &mut points,
                )
                .unwrap();
        }
        assert_eq!(points.len(), 137 * 44);
        assert!(
            points
                .iter()
                .all(|p| p.position.iter().all(|v| v.is_finite()) && p.radius.is_finite())
        );
        for (phase, count) in [(0, 46 * 44), (1, 46 * 44), (2, 45 * 44)] {
            assert_eq!(points.iter().filter(|p| p.phase == phase).count(), count);
        }
    }
}
