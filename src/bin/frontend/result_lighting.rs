//! Result actor's original 004f2390 camera half-vector and 004f264b highlight.

use openmt2001::lighting::Palette;

fn normalize(vector: [f32; 3]) -> [f32; 3] {
    let square = f64::from(vector[0]) * f64::from(vector[0])
        + f64::from(vector[1]) * f64::from(vector[1])
        + f64::from(vector[2]) * f64::from(vector[2]);
    let length = square.sqrt();
    // 004e2ba0 uses this reciprocal when the half-vector is nearly zero.
    let reciprocal = if length < f64::from(0.001_f32) {
        f64::from(f32::from_bits(0x4479ffff))
    } else {
        1. / length
    };
    vector.map(|v| (f64::from(v) * reciprocal) as f32)
}

/// Native 004f2390 combines normalized camera direction with local sunlight.
/// Both inputs are in the original model coordinate system; the caller flips
/// world Z while forming them, matching the retained mesh transform.
pub fn camera_half(sun_local: [f32; 3], eye_local: [f32; 3], actor_local: [f32; 3]) -> [f32; 3] {
    let delta =
        std::array::from_fn(|i| (f64::from(eye_local[i]) - f64::from(actor_local[i])) as f32);
    let view = normalize(delta);
    let sum = std::array::from_fn(|i| (f64::from(sun_local[i]) + f64::from(view[i])) as f32);
    normalize(sum)
}

/// Original 004f2580 flag-4 vertex path. The diffuse lookup remains the
/// ordinary 004ebbc0 palette; 004f26a0 adds the 16th-power camera highlight
/// to its signed alpha before indexing the secondary palette.
pub fn shade_vertex(
    palette: &Palette,
    normal: [f32; 3],
    sun_local: [f32; 3],
    camera_half: [f32; 3],
) -> Result<[u32; 2], String> {
    let sun = sun_local.map(|v| (f64::from(v) * 127.) as f32);
    let dot = f64::from(normal[2]) * f64::from(sun[2])
        + (f64::from(normal[1]) * f64::from(sun[1]) + f64::from(normal[0]) * f64::from(sun[0]));
    let shade = dot.round_ties_even();
    if !shade.is_finite() || !(-127. ..=128.).contains(&shade) {
        return Err("result actor diffuse index outside native palette".into());
    }
    let primary = palette.primary[(shade as i32 + 127) as usize];
    let spec_dot = f64::from(normal[2]) * f64::from(camera_half[2])
        + (f64::from(normal[1]) * f64::from(camera_half[1])
            + f64::from(normal[0]) * f64::from(camera_half[0]));
    let mut factor = (spec_dot * 0.5 + 0.5).powi(16);
    factor = factor * 254. - 127.;
    let extra = (factor as f32).round_ties_even() as i32;
    // 00945554 points at the middle of the 512-entry secondary table.
    let index = (127 + extra.max(0) + (primary as i32 >> 24)) as usize;
    let secondary = *palette
        .secondary
        .get(index)
        .ok_or("result actor specular index outside native palette")?;
    Ok([primary | 0xff000000, secondary])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_v14_actor_camera_highlight() {
        // tools/native_result_actor_light.py: pinned SHA 1401be713210eec3…
        // FPCW 037f; 004f2390 -> 004f1b40 -> 004f2580.
        let palette = Palette::new(0xffcdbb94, 0xff2d2d2d, 0xffcdbb94, 0, 1.);
        let sun = [0.25, 0.5, -0.75];
        let normal = [0.57735026, 0.57735026, -0.57735026];
        let actor = [-53.7, 4., 47.9];
        for (eye, native_half, native_specular) in [
            (
                [0., 20., 40.],
                [0.7112567, 0.46434164, -0.5277317],
                0xff998b6e,
            ),
            (
                [50., 40., 20.],
                [0.67070186, 0.47060844, -0.57331204],
                0xffac9d7c,
            ),
            (
                [-53.7, 50., 47.9],
                [0.14744195, 0.8846517, -0.44232586],
                0xff000000,
            ),
        ] {
            let half = camera_half(sun, eye, actor);
            for i in 0..3 {
                assert!(
                    (half[i] - native_half[i]).abs() < 0.000001,
                    "half component {i}"
                );
            }
            let shades = shade_vertex(&palette, normal, sun, half).unwrap();
            assert_eq!(shades, [0xffebdab6, native_specular]);
        }
    }
}
