//! Original intro flyby camera samples. This is the narrow uncached TM format
//! consumed by483af0/4ed700, then registered by4e96d0 and sampled by483c20.
//! Host timing and FlybyEnded script dispatch remain separate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSample {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub vertical_fov: f32,
}
pub struct Flyby<'a> {
    positions: &'a [u8],
    frames: usize,
}
impl<'a> Flyby<'a> {
    /// Accept the original four-point, one-part, unpacked camera track subset.
    /// Other TM variants require their own recovered loader behavior.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < 16 {
            return Err("truncated flyby header".into());
        }
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let short = |at| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
        if word(0) != 0x10000
            || word(4) != 0x47
            || short(8) != 4
            || short(10) != 1
            || short(14) != 0
        {
            return Err("unsupported uncached flyby TM layout".into());
        }
        let frames = usize::from(short(12));
        if frames < 2 {
            return Err("flyby needs its basis frame and camera samples".into());
        }
        let end = 16 + frames * 48;
        // Four UV records, an8-byte part header and six16-bit indices.
        if bytes.len() != end + 52
            || bytes[end + 32..end + 40] != [0, 0, 0, 0, 0, 0, 6, 0]
            || bytes[end + 40..] != [2, 0, 0, 0, 1, 0, 1, 0, 3, 0, 2, 0]
        {
            return Err("unsupported flyby camera topology".into());
        }
        let positions = &bytes[16..end];
        if positions
            .chunks_exact(4)
            .any(|p| !f32::from_le_bytes(p.try_into().unwrap()).is_finite())
        {
            return Err("non-finite flyby point".into());
        }
        Ok(Self { positions, frames })
    }
    pub fn frame_count(&self) -> usize {
        self.frames
    }
    ///483c20 increments before sampling: the first ordinary sample is frame1.
    /// Frame0 contains source basis points, not an initial world camera pose.
    pub fn sample(&self, frame: usize) -> Result<CameraSample, String> {
        if frame == 0 || frame >= self.frames {
            return Err("flyby sample frame outside1..frame_count".into());
        }
        let point = |vertex: usize| -> [f32; 3] {
            std::array::from_fn(|axis| {
                let at = frame * 48 + vertex * 12 + axis * 4;
                f32::from_le_bytes(self.positions[at..at + 4].try_into().unwrap())
            })
        };
        // Native4e96d0 produces registered source order[2,3,1,0] for this
        // two-triangle animated track. All8904 frames retain that order.
        let eye = point(2);
        let target = point(0);
        let top = point(1);
        let world = |p: [f32; 3]| [p[0], p[1], -p[2]];
        let up = std::array::from_fn(|axis| (f64::from(top[axis]) - f64::from(eye[axis])) as f32);
        Ok(CameraSample {
            eye: world(eye),
            target: world(target),
            up: world(up),
            // 4c9110 installs45 through projection setter4f1550. The30 at
            // 93afd8 written by483c20 is an interpolation threshold, not FOV.
            vertical_fov: 45.,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_truncated_flyby() {
        assert!(Flyby::parse(&[0; 15]).is_err());
    }
    #[test]
    #[ignore = "requires original loader/registration/camera fixture"]
    fn matches_native_flyby_samples() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_FLYBY")).unwrap();
        assert_eq!(&bytes[..8], b"MTFLY001");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for _ in 0..word(8) {
            let length = word(at) as usize;
            let count = word(at + 4);
            at += 8;
            let track = Flyby::parse(&bytes[at..at + length]).unwrap();
            at += length;
            let mut last = None;
            for _ in 0..count {
                let previous = word(at) as usize;
                let current = word(at + 4) as usize;
                let expected = if previous + 1 == track.frame_count() {
                    assert_eq!(current, 0);
                    last.unwrap()
                } else {
                    assert_eq!(current, previous + 1);
                    track.sample(current).unwrap()
                };
                let floats = expected
                    .eye
                    .into_iter()
                    .chain(expected.target)
                    .chain(expected.up);
                for (i, f) in floats.enumerate() {
                    assert_eq!(
                        f.to_bits(),
                        word(at + 8 + i * 4),
                        "frame {previous} component{i}"
                    );
                }
                // Captured separately from the pose: camera inertia threshold.
                assert_eq!(word(at + 44), 30f32.to_bits());
                assert_eq!(expected.vertical_fov, 45.);
                last = Some(expected);
                at += 48;
            }
        }
        assert_eq!(at, bytes.len());
    }
}
