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
/// Startup kind-17 playback. Track requests advance at the camera's 50 Hz
/// cadence; presentation only samples the shared primary camera arithmetic.
pub struct Playback {
    requests: Vec<CameraSample>,
    camera: crate::camera::primary::Primary,
    frame: usize,
}
impl Playback {
    pub fn new(track: &Flyby<'_>) -> Result<Self, String> {
        let requests = (1..track.frame_count())
            .map(|frame| track.sample(frame))
            .collect::<Result<Vec<_>, _>>()?;
        let mut playback = Self {
            requests,
            camera: crate::camera::primary::Primary::flyby([[0.; 3]; 2])?,
            frame: 0,
        };
        // The startup epoch presents the first callback, which preincrements
        // from the basis record to frame 1, just as the track decoder requires.
        playback.advance_to(0)?;
        Ok(playback)
    }
    pub fn duration_ticks(&self) -> usize {
        self.requests.len()
    }
    /// Catch up every callback, including those between two rendered frames.
    /// Completion remains the frontend's separate FlybyEnded/menu transition.
    pub fn advance_to(&mut self, tick: usize) -> Result<(), String> {
        let frame = tick.saturating_add(1).min(self.requests.len());
        while self.frame < frame {
            let request = self.requests[self.frame];
            self.camera
                .request_flyby(request.eye, request.target, request.up)?;
            self.camera.common()?;
            self.frame += 1;
        }
        Ok(())
    }
    /// The menu inherits the simulation pose, rather than a render extrapolation.
    pub fn current(&self) -> CameraSample {
        let view = self.camera.view();
        CameraSample {
            eye: view.current[0],
            target: view.current[1],
            up: view.up,
            vertical_fov: 45.,
        }
    }
    pub fn sample(&self, fraction: f32) -> Result<CameraSample, String> {
        // Primary::sample writes its renderer output words only. Keep even
        // those writes out of playback state so sampling is read-only here.
        let sampled = self.camera.clone().sample(fraction)?;
        Ok(CameraSample {
            eye: sampled[0],
            target: sampled[1],
            ..self.current()
        })
    }
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
    fn authored_track(poses: &[CameraSample]) -> Vec<u8> {
        let frames = poses.len() + 1;
        let mut bytes = vec![0; 16 + frames * 48 + 52];
        bytes[..4].copy_from_slice(&0x10000u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&0x47u32.to_le_bytes());
        bytes[8..10].copy_from_slice(&4u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..14].copy_from_slice(&(frames as u16).to_le_bytes());
        for (frame, pose) in poses.iter().enumerate() {
            let top = std::array::from_fn(|i| pose.eye[i] + pose.up[i]);
            for (vertex, point) in [pose.target, top, pose.eye, [0.; 3]]
                .into_iter()
                .enumerate()
            {
                for (axis, value) in point.into_iter().enumerate() {
                    let value = if axis == 2 { -value } else { value };
                    let at = 16 + (frame + 1) * 48 + vertex * 12 + axis * 4;
                    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
        }
        let end = 16 + frames * 48;
        bytes[end + 32..end + 40].copy_from_slice(&[0, 0, 0, 0, 0, 0, 6, 0]);
        bytes[end + 40..].copy_from_slice(&[2, 0, 0, 0, 1, 0, 1, 0, 3, 0, 2, 0]);
        bytes
    }
    fn pose(eye: [f32; 3], target: [f32; 3]) -> CameraSample {
        CameraSample {
            eye,
            target,
            up: [0., 1., 0.],
            vertical_fov: 45.,
        }
    }
    #[test]
    fn startup_flyby_preincrements_eases_and_copies_at_strict_l1_boundary() {
        let poses = [
            pose([12., 6., -6.], [2., 4., -2.]),
            pose([16., 13., 7.], [3., 4., -3.]),
            pose([100., 100., 100.], [-100., -100., -100.]),
        ];
        let bytes = authored_track(&poses);
        let track = Flyby::parse(&bytes).unwrap();
        let mut playback = Playback::new(&track).unwrap();
        assert_eq!(playback.frame, 1, "basis frame must be skipped");
        assert_eq!(
            playback.camera.view().desired,
            [poses[0].eye, poses[0].target]
        );
        assert_eq!(playback.current(), pose([6., 3., -3.], [1., 2., -1.]));
        assert_eq!(playback.sample(0.).unwrap(), playback.current());
        assert_eq!(
            playback.sample(0.5).unwrap(),
            pose([7.5, 3.75, -3.75], [1.25, 2.5, -1.25])
        );
        assert_eq!(
            playback.sample(1.).unwrap(),
            pose([9., 4.5, -4.5], [1.5, 3., -1.5])
        );
        playback.advance_to(1).unwrap();
        // Eye delta is [10,10,10]: equality copies, while target still eases.
        assert_eq!(playback.current(), pose(poses[1].eye, [2., 3., -2.]));
        assert_eq!(playback.sample(0.5).unwrap().eye, poses[1].eye);
        playback.advance_to(2).unwrap();
        assert_eq!(playback.current(), poses[2]);
        assert_eq!(playback.sample(0.75).unwrap(), poses[2]);
    }
    #[test]
    fn startup_flyby_render_sampling_is_read_only() {
        let bytes = authored_track(&[pose([12., 6., -6.], [2., 4., -2.])]);
        let playback = Playback::new(&Flyby::parse(&bytes).unwrap()).unwrap();
        let before = playback.camera.clone();
        for fraction in [0., 0.25, 0.75, 1., 0.5] {
            playback.sample(fraction).unwrap();
            assert_eq!(playback.camera, before);
            assert_eq!(playback.frame, 1);
        }
    }
    #[test]
    fn startup_flyby_callback_sequence_is_independent_of_render_rate() {
        let poses = (0..30)
            .map(|i| pose([i as f32, 6., -6.], [2., i as f32, -2.]))
            .collect::<Vec<_>>();
        let bytes = authored_track(&poses);
        let track = Flyby::parse(&bytes).unwrap();
        let mut frequent = Playback::new(&track).unwrap();
        let mut sparse = Playback::new(&track).unwrap();
        for tick in 0..30 {
            frequent.advance_to(tick).unwrap();
            for fraction in [0., 0.25, 0.5, 0.75] {
                frequent.sample(fraction).unwrap();
            }
            if tick % 3 == 2 {
                sparse.advance_to(tick).unwrap();
                assert_eq!(sparse.camera, frequent.camera);
                assert_eq!(sparse.sample(0.6).unwrap(), frequent.sample(0.6).unwrap());
            }
        }
        let final_pose = sparse.current();
        sparse.advance_to(usize::MAX).unwrap();
        assert_eq!(
            sparse.current(),
            final_pose,
            "completion must not loop the track"
        );
    }
    #[test]
    #[ignore = "requires the three supplied startup flyby tracks"]
    fn startup_flyby_interpolates_all_supplied_tracks_between_callbacks() {
        let parameter = crate::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        for name in ["intro_low.tm", "intro_medium.tm", "intro_high.tm"] {
            let bytes = std::fs::read(installer.join("Standard/Cut_scenes").join(name)).unwrap();
            let track = Flyby::parse(&bytes).unwrap();
            let mut playback = Playback::new(&track).unwrap();
            let mut current = [[0.; 3]; 2];
            let mut eased = 0;
            let mut interpolated = 0;
            for frame in 1..track.frame_count() {
                let request = track.sample(frame).unwrap();
                let desired = [request.eye, request.target];
                for (current, desired) in current.iter_mut().zip(desired) {
                    let distance = current
                        .iter()
                        .zip(desired)
                        .map(|(c, d)| (f64::from(d) - f64::from(*c)).abs())
                        .sum::<f64>();
                    if distance < 30. {
                        eased += usize::from(distance != 0.);
                        for (c, d) in current.iter_mut().zip(desired) {
                            *c = (f64::from(*c) + (f64::from(d) - f64::from(*c)) / 2.) as f32;
                        }
                    } else {
                        *current = desired;
                    }
                }
                playback.advance_to(frame - 1).unwrap();
                let before = playback.current();
                assert_eq!([before.eye, before.target], current, "{name} frame {frame}");
                for fraction in [0., 0.25, 0.5, 0.75] {
                    let expected: [[f32; 3]; 2] = std::array::from_fn(|vector| {
                        std::array::from_fn(|axis| {
                            (f64::from(current[vector][axis])
                                + (f64::from(desired[vector][axis])
                                    - f64::from(current[vector][axis]))
                                    * f64::from(fraction)
                                    / 2.) as f32
                        })
                    });
                    let rendered = playback.sample(fraction).unwrap();
                    assert_eq!(
                        [rendered.eye, rendered.target],
                        expected,
                        "{name} frame {frame} fraction {fraction}"
                    );
                    assert_eq!(rendered.up, request.up);
                    assert_eq!(rendered.vertical_fov, request.vertical_fov);
                    interpolated += usize::from(rendered != before);
                    assert_eq!(playback.current(), before);
                }
            }
            assert!(eased > 0, "{name} must exercise smoothing");
            assert!(interpolated > 0, "{name} must move between callbacks");
        }
    }

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
