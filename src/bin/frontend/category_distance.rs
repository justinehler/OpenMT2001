//! Original category-depth thresholds shared by pass-3 details and pavement text.
use openmt2001::x87::Sum;

/// 4b8228 adds building scene Y0.5 minus the original 0.25 bias to each
/// descriptor translation after its constructor generated the base mesh.
pub const PASS3_Y_OFFSET: f32 = 0.25;

pub struct Fade {
    near: f32,
    far: f32,
    inverse: f32,
}

impl Fade {
    /// 4b80ae/4b321b retain the unrounded far calculation while subtracting
    /// the stored binary32 near threshold to form the reciprocal.
    pub fn from_factor(factor: f32) -> Self {
        let mut base = Sum::from_f32(factor).expect("bounded Draw Distance factor");
        base.mul_f32(200.).unwrap();
        let mut near_extended = base;
        near_extended.add_f32(150.).unwrap();
        let near = near_extended.to_f32();
        let mut far_extended = base;
        far_extended.add_f32(200.).unwrap();
        let far = far_extended.to_f32();
        far_extended.add(Sum::from_f32(near).unwrap().negated());
        let mut inverse = Sum::from_f32(1.).unwrap();
        inverse.div(far_extended).unwrap();
        Self {
            near,
            far,
            inverse: inverse.to_f32(),
        }
    }

    fn opacity(&self, depth: f32) -> Option<Sum> {
        if !depth.is_finite() || depth > self.far {
            return None;
        }
        if depth <= self.near {
            return Some(Sum::from_f32(1.).unwrap());
        }
        let mut opacity = Sum::from_f32(self.far).unwrap();
        opacity.add(Sum::from_f32(depth).unwrap().negated());
        opacity.mul_f32(self.inverse).unwrap();
        Some(opacity)
    }

    /// Native 4b8155 stores the pass-3 Y-scale as binary32 before multiplying
    /// the descriptor's own Y scale at 4b821e.
    pub fn detail_scale(&self, depth: f32) -> Option<f32> {
        self.opacity(depth).map(Sum::to_f32)
    }

    /// Native 4b3285 instead multiplies the extended opacity by 255 and
    /// truncates it through 50e928; rounding detail_scale first loses bins.
    pub fn pavement_alpha(&self, depth: f32) -> u8 {
        let Some(mut opacity) = self.opacity(depth) else {
            return 0;
        };
        opacity.mul_f32(255.).unwrap();
        opacity.ftol_low_word().clamp(0, 255) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    #[ignore = "requires original v1.4 category detail Y-scale oracle"]
    fn matches_native_detail_scale() {
        let fixture = fs::read("work/frontend/native-category-detail-fade.bin").unwrap();
        assert_eq!(&fixture[..8], b"MTYFAD01");
        let count = u32::from_le_bytes(fixture[8..12].try_into().unwrap()) as usize;
        assert_eq!(fixture.len(), 12 + count * 12);
        for row in fixture[12..].chunks_exact(12) {
            let setting = u32::from_le_bytes(row[..4].try_into().unwrap());
            let depth = f32::from_le_bytes(row[4..8].try_into().unwrap());
            let native = u32::from_le_bytes(row[8..12].try_into().unwrap());
            let factor = crate::people_scene::Policy::distance_factor(setting).unwrap();
            let actual = Fade::from_factor(factor)
                .detail_scale(depth)
                .unwrap_or(0.)
                .to_bits();
            assert_eq!(
                actual, native,
                "Draw Distance {setting}, category depth {depth}"
            );
        }
    }

    #[test]
    #[ignore = "requires original v1.4 category pass-3 row-pose oracle"]
    fn matches_native_detail_row_pose() {
        let fixture = fs::read("work/frontend/native-category-detail-pose.bin").unwrap();
        assert_eq!(&fixture[..8], b"MTDPIV01");
        let count = u32::from_le_bytes(fixture[8..12].try_into().unwrap()) as usize;
        assert_eq!(fixture.len(), 12 + count * 20);
        let mut active = [false; 2];
        for row in fixture[12..].chunks_exact(20) {
            let kind = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
            let setting = u32::from_le_bytes(row[4..8].try_into().unwrap());
            let depth = f32::from_le_bytes(row[8..12].try_into().unwrap());
            let native_y = f32::from_le_bytes(row[12..16].try_into().unwrap());
            let native_scale = f32::from_le_bytes(row[16..20].try_into().unwrap());
            active[kind] = true;
            let factor = crate::people_scene::Policy::distance_factor(setting).unwrap();
            let scale = Fade::from_factor(factor).detail_scale(depth).unwrap();
            assert_eq!(
                (4. + PASS3_Y_OFFSET).to_bits(),
                native_y.to_bits(),
                "native descriptor translation for row kind {kind}"
            );
            assert_eq!(
                (6. * scale).to_bits(),
                native_scale.to_bits(),
                "native descriptor Y scale for row kind {kind}"
            );
        }
        assert_eq!(
            active,
            [true, true],
            "both active and inactive rows were probed"
        );
    }
}
