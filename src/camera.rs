//! Original D3D8 view/projection arithmetic. Camera controls and host viewport
//! selection remain separate. Matrices use native row-vector storage, which is
//! also column-major storage for the equivalent WGSL column-vector transform.

/// Mode12's desired camera state. Host input sampling and camera interpolation
/// are separate from this original 00482620 callback arithmetic.
#[derive(Clone, Copy, Debug)]
pub struct BlockCamera {
    pub center: [f32; 3],
    pub angle: f32,
    pub height: f32,
    pub radius: f32,
}
impl BlockCamera {
    /// Original right-button-only orbit/zoom, 00482946 then 004823d0.
    /// Apply after `tick`: horizontal pose changes appear on the next callback;
    /// zoom changes the emitted height/look-at height immediately.
    pub fn right_drag(
        &mut self,
        pose: &mut ([f32; 3], [f32; 3]),
        delta: [i32; 2],
        sensitivity: i32,
        invert_zoom: bool,
        legacy_rotation: bool,
        low_height: bool,
    ) -> Result<(), String> {
        if !(0..=100).contains(&sensitivity) || delta[1] == i32::MIN {
            return Err("unsupported camera input range".into());
        }
        let dx = delta[0].clamp(-14, 14);
        if dx != 0 {
            self.angle = if legacy_rotation {
                (f64::from(self.angle) + f64::from((dx * 2) / (sensitivity + 50))) as f32
            } else {
                (f64::from(self.angle) - f64::from(dx * 2) / f64::from(200 - sensitivity)) as f32
            };
        }
        let dy = if invert_zoom { -delta[1] } else { delta[1] };
        let amount = f64::from(dy.unsigned_abs());
        let speed = (f64::from(sensitivity) * f64::from(0.01f32) + 0.5) as f32;
        let scaled = amount * f64::from(speed);
        if dy > 0 && (if low_height { 17. } else { 25. }) + scaled < f64::from(self.height) {
            self.height = (f64::from(self.height) - scaled) as f32;
            pose.1[1] = (f64::from(pose.1[1]) - scaled * 0.5) as f32;
            self.radius = (f64::from(self.radius) - amount * f64::from(-0.1f32)) as f32;
        } else if dy < 0 && 200. - scaled > f64::from(self.height) {
            self.height = (f64::from(self.height) + scaled) as f32;
            pose.1[1] = (f64::from(pose.1[1]) + scaled * 0.5) as f32;
            self.radius = (f64::from(self.radius) - amount * f64::from(0.1f32)) as f32;
        }
        pose.0[1] = self.height;
        Ok(())
    }
    /// Emit the pose before applying keys, as the original callback does.
    /// Key order is right, left, down, up (DirectInput CD, CB, D0, C8).
    pub fn tick(&mut self, keys: [bool; 4]) -> ([f32; 3], [f32; 3]) {
        let angle = f64::from(self.angle);
        let sin = f64::from(angle.sin() as f32);
        let cos = f64::from(angle.cos() as f32);
        let target_x = f64::from(self.center[0]) + sin * 25.;
        let target_z = (f64::from(self.center[2]) - cos * 25.) as f32;
        let target = [target_x as f32, self.center[1], target_z];
        let eye = [
            (angle.cos() * f64::from(self.radius) + target_x) as f32,
            self.height,
            (angle.sin() * f64::from(self.radius) + f64::from(target_z)) as f32,
        ];
        for (enabled, dx, dz) in [
            (keys[0], -sin * 2., cos * 2.),
            (keys[1], sin * 2., -cos * 2.),
            (keys[2], cos * 2., sin * 2.),
            (keys[3], -cos * 2., -sin * 2.),
        ] {
            if enabled {
                self.center[0] = (f64::from(self.center[0]) + dx) as f32;
                self.center[2] = (f64::from(self.center[2]) + dz) as f32;
            }
        }
        self.center[0] = self.center[0].clamp(-500., 500.);
        self.center[2] = self.center[2].clamp(-500., 500.);
        (eye, target)
    }
}

/// Mode12 inertia initialized by00482580: both axes enabled, denominator20,
/// zero distance threshold. The host supplies callback endpoints and frame phase.
#[derive(Clone, Copy, Debug)]
pub struct BlockCameraInertia {
    start: [[f32; 3]; 2],
    endpoint: [[f32; 3]; 2],
}
impl BlockCameraInertia {
    pub fn new(eye: [f32; 3], target: [f32; 3]) -> Self {
        Self {
            start: [eye, target],
            endpoint: [eye, target],
        }
    }
    /// Original004e03c9..004e056e, with the mode12 initialization above.
    pub fn tick(&mut self, eye: [f32; 3], target: [f32; 3]) {
        self.endpoint = [eye, target];
        for (start, end) in self
            .start
            .iter_mut()
            .flatten()
            .zip(self.endpoint.iter().flatten())
        {
            *start = ((f64::from(*end) - f64::from(*start)) / 20. + f64::from(*start)) as f32;
        }
    }
    /// Original004e0590. Phase is the fraction of the camera callback interval.
    pub fn sample(&self, phase: f32) -> Result<([f32; 3], [f32; 3]), String> {
        if !phase.is_finite() || !(0. ..=1.).contains(&phase) {
            return Err("camera interpolation phase leaves callback interval".into());
        }
        let value: [[f32; 3]; 2] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                ((f64::from(self.endpoint[i][j]) - f64::from(self.start[i][j])) * f64::from(phase)
                    / 20.
                    + f64::from(self.start[i][j])) as f32
            })
        });
        Ok((value[0], value[1]))
    }
}

/// Normal-city clipping policy, 004b4527..004b457b. Applied after camera setup.
pub fn city_clip(eye_height: f32) -> Result<[f32; 2], String> {
    if !eye_height.is_finite() {
        return Err("non-finite camera height".into());
    }
    let near = if eye_height < 15. {
        1.
    } else {
        let scaled = f64::from(eye_height) * f64::from(0.1f32);
        if scaled < 4. { 4. } else { scaled as f32 }
    };
    Ok([near, 3000.])
}

/// CPU scene camera, 004e2de0 via 004f13f0, with default up Y. Unlike the
/// D3D view builder, X/Y intermediates remain on x87 until the final stores.
/// The first nine cells interleave right/up/forward; translation follows.
pub fn scene_view(eye: [f32; 3], target: [f32; 3]) -> [f32; 12] {
    scene_view_with_up(eye, target, [0., 1., 0.])
}

/// Original004e2fc0, specialized to an object with identity rotation and scale.
pub fn translated_scene_view(mut view: [f32; 12], translation: [f32; 3]) -> [f32; 12] {
    let [x, y, z] = translation.map(f64::from);
    let source = view;
    for row in 0..3 {
        for column in 0..3 {
            let a = if row == 0 { 1. } else { 0. };
            let b = if row == 1 { 1. } else { 0. };
            let c = if row == 2 { 1. } else { 0. };
            view[row * 3 + column] = ((a * f64::from(source[column])
                + b * f64::from(source[3 + column]))
                + c * f64::from(source[6 + column])) as f32;
        }
    }
    for i in 0..3 {
        view[9 + i] = (z * f64::from(source[6 + i])
            + (y * f64::from(source[3 + i])
                + (x * f64::from(source[i]) + f64::from(source[9 + i]))))
            as f32;
    }
    view
}

/// Original004e39c0 product/addition grouping before each float store.
pub fn transform_scene_point(view: [f32; 12], point: [f32; 3]) -> [f32; 3] {
    let m = view.map(f64::from);
    let [x, y, z] = point.map(f64::from);
    [
        ((z * m[6] + m[9]) + (y * m[3] + x * m[0])) as f32,
        ((y * m[4] + x * m[1]) + (z * m[7] + m[10])) as f32,
        (z * m[8] + (y * m[5] + (x * m[2] + m[11]))) as f32,
    ]
}

/// Original CPU camera builder with an explicit up vector, used by flyby tracks.
pub fn scene_view_with_up(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [f32; 12] {
    let [up_x, up_y, up_z] = up.map(f64::from);
    let mut x = -(f64::from(eye[0]) - f64::from(target[0]));
    let mut y = -(f64::from(eye[1]) - f64::from(target[1]));
    let retained_z = -(f64::from(eye[2]) - f64::from(target[2]));
    let mut z = f64::from(retained_z as f32);
    let length = ((retained_z * z + y * y) + x * x).sqrt();
    if length != 0. {
        x /= length;
        y /= length;
        z = f64::from((z / length) as f32);
    }
    let mut rx = z * up_y - y * up_z;
    let mut ry = f64::from((x * up_z - z * up_x) as f32);
    let retained_rz = y * up_x - x * up_y;
    let mut rz = f64::from(retained_rz as f32);
    let mut ux = retained_rz * y - ry * z;
    let mut uy = rx * z - rz * x;
    let mut uz = ry * x - rx * y;
    let length = ((rz * rz + ry * ry) + rx * rx).sqrt();
    if length != 0. {
        rx /= length;
        ry = f64::from((ry / length) as f32);
        rz = f64::from((rz / length) as f32);
    }
    let length = ((uz * uz + uy * uy) + ux * ux).sqrt();
    if length != 0. {
        ux /= length;
        uy /= length;
        uz /= length;
    }
    let [ex, ey, ez] = eye.map(f64::from);
    [
        rx as f32,
        ux as f32,
        x as f32,
        ry as f32,
        uy as f32,
        y as f32,
        rz as f32,
        uz as f32,
        z as f32,
        ((-(rx * ex) + -(ry * ey)) + -(rz * ez)) as f32,
        ((-(ux * ex) + -(uy * ey)) + -(uz * ez)) as f32,
        ((-(x * ex) + -(y * ey)) + -(z * ez)) as f32,
    ]
}

/// 004f10e0's vertical-view fallback followed by 004f0d60, with default up Y.
pub fn view(eye: [f32; 3], target: [f32; 3]) -> [f32; 16] {
    view_with_up(eye, target, [0., 1., 0.])
}

/// Original D3D camera builder with explicit up, preserving vertical fallback.
pub fn view_with_up(mut eye: [f32; 3], mut target: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let [up_x, up_y, up_z] = up.map(f64::from);
    if (f64::from(eye[0]) - f64::from(target[0])).abs() < 0.01
        && (f64::from(eye[2]) - f64::from(target[2])).abs() < 0.01
    {
        eye = [0.; 3];
        target = [0., 0., 100.];
    }
    let mut forward =
        std::array::from_fn::<_, 3, _>(|i| (-(f64::from(eye[i]) - f64::from(target[i]))) as f32);
    let [x, y, z] = forward.map(f64::from);
    // The final subtraction is FST rather than FSTP: its unrounded value
    // remains on x87 for multiplication by the rounded stored Z.
    let retained_z = -(f64::from(eye[2]) - f64::from(target[2]));
    let length = ((retained_z * z + y * y) + x * x).sqrt();
    if length != 0. {
        forward = forward.map(|v| (f64::from(v) / length) as f32);
    }
    let [x, y, z] = forward.map(f64::from);
    // Keep the original signed zero and store boundaries even with default up.
    let mut right = [
        (z * up_y - y * up_z) as f32,
        (x * up_z - z * up_x) as f32,
        (y * up_x - x * up_y) as f32,
    ];
    let right_z = y * up_x - x * up_y;
    let up = [
        (right_z * y - f64::from(right[1]) * z) as f32,
        (f64::from(right[0]) * z - f64::from(right[2]) * x) as f32,
        (f64::from(right[1]) * x - f64::from(right[0]) * y) as f32,
    ];
    let [rx, ry, rz] = right.map(f64::from);
    // This sum is stored as f32 before sqrt, unlike forward/up lengths.
    let right_length = f64::from((rx * rx + (ry * ry + rz * rz)) as f32).sqrt();
    let right_z = if right_length != 0. {
        right[0] = (rx / right_length) as f32;
        right[1] = (ry / right_length) as f32;
        rz / right_length
    } else {
        rz
    };
    let [ux, uy, uz] = up.map(f64::from);
    let up_length = ((uz * uz + uy * uy) + ux * ux).sqrt();
    let up = if up_length != 0. {
        [ux / up_length, uy / up_length, uz / up_length]
    } else {
        [ux, uy, uz]
    };
    let [ex, ey, ez] = eye.map(f64::from);
    [
        right[0],
        up[0] as f32,
        forward[0],
        0.,
        right[1],
        up[1] as f32,
        forward[1],
        0.,
        right_z as f32,
        up[2] as f32,
        forward[2],
        0.,
        ((-f64::from(right[0]) * ex - f64::from(right[1]) * ey) - right_z * ez) as f32,
        ((-up[0] * ex - up[1] * ey) - up[2] * ez) as f32,
        ((-x * ex - y * ey) - z * ez) as f32,
        1.,
    ]
}

/// 004f1550/004f1480. Clip bounds are original normalized left/right/top/bottom
/// globals, ordinarily [0,1,0,1]. FOV is degrees; depth range is 0..1.
pub fn projection(
    fov: f32,
    near: f32,
    far: f32,
    width: i32,
    height: i32,
    clip: [f32; 4],
) -> Result<[f32; 16], String> {
    if width <= 0
        || height <= 0
        || !(near > 0. && far > near)
        || !(fov > 0. && fov < 180.)
        || !clip.iter().all(|v| v.is_finite())
        || clip[1] <= clip[0]
        || clip[3] <= clip[2]
        || !far.is_finite()
    {
        return Err("invalid camera projection inputs".into());
    }
    let depth = (f64::from(far) / (f64::from(far) - f64::from(near))) as f32;
    let depth_near = (f64::from(depth) * f64::from(near)) as f32;
    let aspect = (f64::from(width) / f64::from(height)) as f32;
    let angle = f64::from(fov) * f64::from(f32::from_bits(0x3c0efa1e));
    let cot = angle.cos() / angle.sin();
    let scale_x = (cot / f64::from(aspect)) as f32;
    let inv_x = (1. / (f64::from(clip[1]) - f64::from(clip[0]))) as f32;
    let inv_y = (1. / (f64::from(clip[3]) - f64::from(clip[2]))) as f32;
    let mut matrix = [0.; 16];
    matrix[0] = (f64::from(inv_x) * f64::from(scale_x)) as f32;
    matrix[5] = (cot * f64::from(inv_y)) as f32;
    matrix[8] = (-1. - (f64::from(clip[0]) - 0.5) * f64::from(inv_x) * 2.) as f32;
    matrix[9] = (-1. - (f64::from(clip[2]) - 0.5) * f64::from(inv_y) * 2.) as f32;
    matrix[10] = depth;
    matrix[11] = 1.;
    matrix[14] = -depth_near;
    Ok(matrix)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original native mode12 inertia observations"]
    fn matches_native_camera_inertia() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_CAMERA_INERTIA")).unwrap();
        assert_eq!(&bytes[..8], b"MTCINR01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(12), word(16)), (3, 60, 5));
        let mut at = 20;
        let read_pose = |at: &mut usize| {
            let result: [[f32; 3]; 2] = std::array::from_fn(|i| {
                std::array::from_fn(|j| f32::from_bits(word(*at + (i * 3 + j) * 4)))
            });
            *at += 24;
            result
        };
        for case in 0..3 {
            let initial = read_pose(&mut at);
            let mut camera = super::BlockCameraInertia::new(initial[0], initial[1]);
            for tick in 0..60 {
                let end = read_pose(&mut at);
                camera.tick(end[0], end[1]);
                let expected = read_pose(&mut at);
                assert_eq!(
                    camera.start.map(|v| v.map(f32::to_bits)),
                    expected.map(|v| v.map(f32::to_bits)),
                    "case{case} tick{tick}"
                );
                for _ in 0..5 {
                    let phase = f32::from_bits(word(at));
                    at += 4;
                    let expected = read_pose(&mut at);
                    let (eye, target) = camera.sample(phase).unwrap();
                    assert_eq!(
                        [eye, target].map(|v| v.map(f32::to_bits)),
                        expected.map(|v| v.map(f32::to_bits)),
                        "case{case} tick{tick} phase{phase}"
                    );
                }
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original scene transform observations"]
    fn matches_native_scene_transforms() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_SCENE_TRANSFORM")).unwrap();
        assert_eq!(&bytes[..8], b"MTSTRANS");
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(bytes.len(), 12 + count * 132);
        for n in 0..count {
            let base = 12 + n * 132;
            let f = |i| f32::from_bits(word(base + i * 4));
            let view = std::array::from_fn(f);
            let matrix = super::translated_scene_view(view, [f(12), f(13), f(14)]);
            let point = super::transform_scene_point(matrix, [f(15), f(16), f(17)]);
            for (i, value) in matrix.into_iter().chain(point).enumerate() {
                assert_eq!(
                    value.to_bits(),
                    word(base + 72 + i * 4),
                    "case {n}, component {i}"
                );
            }
        }
    }
    #[test]
    #[ignore = "requires original native mode12 mouse observations"]
    fn matches_native_block_mouse() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BLOCK_MOUSE")).unwrap();
        assert_eq!(&bytes[..8], b"MTBMOU02");
        let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(word(12), 0x037f);
        assert_eq!(bytes.len(), 16 + count * 76);
        for n in 0..count {
            let base = 16 + n * 76;
            let f = |i| f32::from_bits(word(base + i * 4));
            let mut state = super::BlockCamera {
                center: [f(0), f(1), f(2)],
                angle: f(3),
                height: f(4),
                radius: f(5),
            };
            let flags = word(base + 36);
            let mut pose = state.tick([false; 4]);
            state
                .right_drag(
                    &mut pose,
                    [word(base + 24) as i32, word(base + 28) as i32],
                    word(base + 32) as i32,
                    flags & 1 != 0,
                    flags & 2 != 0,
                    flags & 4 != 0,
                )
                .unwrap();
            for (i, actual) in pose
                .0
                .into_iter()
                .chain(pose.1)
                .chain([state.angle, state.height, state.radius])
                .enumerate()
            {
                let expected = f(10 + i);
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "mouse case {n}, component {i}: {actual} vs {expected}"
                );
            }
        }
    }
    #[test]
    #[ignore = "requires original native mode12 camera observations"]
    fn matches_native_block_camera() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BLOCK_CAMERA")).unwrap();
        assert_eq!(&bytes[..8], b"MTBCAM02");
        let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(word(12), 0x037f);
        assert_eq!(bytes.len(), 16 + count * 60);
        for n in 0..count {
            let base = 16 + n * 60;
            let f = |i| f32::from_bits(word(base + i * 4));
            let mut state = super::BlockCamera {
                center: [f(0), f(1), f(2)],
                angle: f(3),
                height: f(4),
                radius: 80.,
            };
            let mask = word(base + 20);
            let (eye, target) = state.tick(std::array::from_fn(|i| mask & (1 << i) != 0));
            for (i, actual) in eye
                .into_iter()
                .chain(target)
                .chain(state.center)
                .enumerate()
            {
                let expected = f(6 + i);
                assert_eq!(
                    actual.to_bits(),
                    expected.to_bits(),
                    "case {n}, component {i}: {actual} vs {expected}"
                );
            }
        }
    }
    use super::*;
    #[test]
    #[ignore = "requires original city clip-policy observations"]
    fn matches_native_city_clip() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_CITY_CLIP")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTCLIP01");
        assert_eq!(word(8), 1);
        assert_eq!(bytes.len(), 16 + word(12) as usize * 28);
        for case in 0..word(12) as usize {
            let at = 16 + case * 28;
            let actual = city_clip(f32::from_bits(word(at))).unwrap();
            assert_eq!(
                actual.map(f32::to_bits),
                [word(at + 4), word(at + 8)],
                "clip case{case}"
            );
            assert_eq!(word(at + 4), word(at + 12));
            assert_eq!(word(at + 4), word(at + 16));
            assert_eq!((word(at + 20), word(at + 24)), (1, 0x027f));
        }
    }

    #[test]
    #[ignore = "requires original camera instruction observations"]
    fn matches_native_camera_matrices() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_CAMERA")).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(word(at));
        assert_eq!(&bytes[..8], b"MTCAM002");
        let mut at = 16;
        for case in 0..word(8) {
            let eye = std::array::from_fn(|i| float(at + i * 4));
            let target = std::array::from_fn(|i| float(at + 12 + i * 4));
            let actual = view(eye, target);
            for (i, value) in actual.iter().enumerate() {
                assert_eq!(value.to_bits(), word(at + 24 + i * 4), "view{case} cell{i}");
            }
            for (i, value) in scene_view(eye, target).iter().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    word(at + 88 + i * 4),
                    "scene view{case} cell{i}"
                );
            }
            at += 136;
        }
        for case in 0..word(12) {
            let actual = projection(
                float(at),
                float(at + 4),
                float(at + 8),
                word(at + 12) as i32,
                word(at + 16) as i32,
                std::array::from_fn(|i| float(at + 20 + i * 4)),
            )
            .unwrap();
            for (i, value) in actual.iter().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    word(at + 36 + i * 4),
                    "projection{case} cell{i}"
                );
            }
            at += 100;
        }
        assert_eq!(at, bytes.len());
    }
}

#[cfg(test)]
mod flyby_tests {
    #[test]
    #[ignore = "requires original flyby camera matrix fixture"]
    fn matches_native_flyby_camera_matrices() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_FLYBY_CAMERA")).unwrap();
        assert_eq!(&bytes[..8], b"MTFCAM01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(bytes.len(), 12 + count * 148);
        for case in 0..count {
            let at = 12 + case * 148;
            let vector = |off| std::array::from_fn(|i| f32::from_bits(word(at + off + i * 4)));
            let eye = vector(0);
            let target = vector(12);
            let up = vector(24);
            for (i, value) in super::view_with_up(eye, target, up)
                .into_iter()
                .chain(super::scene_view_with_up(eye, target, up))
                .enumerate()
            {
                assert_eq!(
                    value.to_bits(),
                    word(at + 36 + i * 4),
                    "case{case} matrixcell{i}"
                );
            }
        }
    }
}

pub mod bank;
pub mod primary;
