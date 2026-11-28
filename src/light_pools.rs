//! Streetlamp ground pools from 004b69f0.
//!
//! For each street lamp of 00449360 that passes the night gate, the first
//! loop (state 8, "Combined volumetric lights") submits a cone from the lamp
//! head to the ground twice with 0047e640: back faces multiply the
//! destination down, front faces double it, so only ground inside the cone
//! brightens. The second loop (state 4, additive) adds a small textured cone
//! under the lamp head with 0047ec30. Both loops run with fog disabled
//! (renderer method 0xc8, 004ef590).
use crate::light_halos::Halo;

/// 004c10f0 registers this set into 00689620[]; both cone loops pass
/// 00689628, its third record.
pub const TEXTURES: &str = "gamedata2/BuildingTextures/textures.gtd";
/// 00449360 stores 8.5 at record +0x18 (00449f8e), used both as the
/// culling sphere radius and as the cone radius.
pub const RADIUS: f32 = 8.5;
/// 0047ec30 scales the inner cone radius by 00525d6c and its rim height by
/// 00525d68.
const INNER_RADIUS_SCALE: f32 = 0.333;
const INNER_HEIGHT_SCALE: f32 = 0.667;
/// 004e28b0 fills a 320-entry sine cache by repeatedly adding 0052ae70;
/// the cosine table starts 64 entries in.
const TRIG_CACHE_STEP: u32 = 0x3cc8f5c3;

#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub argb: u32,
    pub uv: [f32; 2],
}

#[derive(Debug)]
pub struct Ring {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

/// 004b69f0 derives the tessellation from the binary32 exponent and high
/// mantissa bits of the ground center's eye-space Z (not its range).
pub fn segments(depth: f32) -> usize {
    (0x440_i32 - (depth.to_bits() >> 20) as i32).clamp(7, 64) as usize
}

/// 004b69f0: full strength to eye Z 300, linear fade to zero at 500.
pub fn fade(depth: f32) -> f32 {
    if depth <= 300. {
        1.
    } else if depth <= 500. {
        (1. - f64::from(depth - 300.) * f64::from(0.005_f32)) as f32
    } else {
        0.
    }
}

/// The three fans 004b69f0 submits for one lamp, in draw order: the dim and
/// bright volume passes, then the additive inner cone when its colour is
/// nonzero. Each fan shares the lamp head as apex.
pub fn rings(lamp: Halo, depth: f32, threshold: u8, enabled: bool) -> Vec<Ring> {
    if !depth.is_finite() || !lamp.visible(threshold, enabled) || fade(depth) <= 0. {
        return Vec::new();
    }
    let count = segments(depth);
    let intensity = fade(depth);
    // 004b69f0 passes fade+1 as a binary32; 0047e640 subtracts 1, scales by
    // 255 and truncates.
    let alpha = (f64::from((intensity + 1.) - 1.) * 255.) as u32;
    let grey = 255 - (alpha >> 1);
    let dim = grey * 0x010101;
    let bright = (alpha << 24) | 0x00ffffff;
    // 004b6ca4 stores fade*255 as binary32 before its rounding fistp.
    let glow = (intensity * 255.).round_ties_even() as u32;
    // Under state 8's D3DCULL_CW, 0047e640's first index order shows only the
    // cone's far inner walls (dim, multiplying the destination by grey); the
    // reversed second order shows its near outer walls (bright, adding
    // destination*alpha). Rays crossing both walls are left unchanged.
    let mut result = vec![
        ring(lamp, count, [RADIUS, 0.], dim, dim, false),
        ring(lamp, count, [RADIUS, 0.], bright, bright, true),
    ];
    if glow > 0 {
        let color = 0xff000000 | (glow * 0x010101);
        let apex = ((color as i32 >> 1) as u32) & 0xff7f7f7f;
        result.push(ring(
            lamp,
            count,
            [
                (f64::from(RADIUS) * f64::from(INNER_RADIUS_SCALE)) as f32,
                (f64::from(lamp.position[1]) * f64::from(INNER_HEIGHT_SCALE)) as f32,
            ],
            apex,
            0xff000000,
            true,
        ));
    }
    result
}

fn ring(
    lamp: Halo,
    count: usize,
    [radius, height]: [f32; 2],
    apex: u32,
    rim: u32,
    reverse: bool,
) -> Ring {
    let mut vertices = vec![Vertex {
        position: lamp.position,
        argb: apex,
        uv: [0.5, 0.],
    }];
    let mut indices = Vec::with_capacity(count * 3);
    // The original steps a 16.16 accumulator by trunc(2^24 / count) and
    // indexes the 256-step trig cache with its integer part. Keeping this
    // quantization matters at the low segment counts of distant lamps.
    let increment = 0x1000000 / count;
    let step = f64::from(f32::from_bits(TRIG_CACHE_STEP));
    for i in 0..count {
        let lookup = (i * increment) >> 16;
        let sine = (lookup as f64 * step).sin() as f32;
        let cosine = ((lookup + 64) as f64 * step).sin() as f32;
        vertices.push(Vertex {
            position: [
                (f64::from(lamp.position[0]) + f64::from(radius) * f64::from(cosine)) as f32,
                height,
                (f64::from(lamp.position[2]) - f64::from(radius) * f64::from(sine)) as f32,
            ],
            argb: rim,
            uv: [(i as f64 * (3. / count as f64)) as f32, 1.],
        });
        let a = i as u16 + 1;
        let b = ((i + 1) % count) as u16 + 1;
        indices.extend(if reverse { [0, b, a] } else { [0, a, b] });
    }
    Ring { vertices, indices }
}

#[cfg(feature = "graphics")]
#[derive(Clone)]
pub(crate) struct Layer {
    volume: usize,
    glow: usize,
    lamps: Vec<Halo>,
}

#[cfg(feature = "graphics")]
impl Layer {
    pub fn load(
        payload: &[u8],
        lamps: Vec<Halo>,
        draws: &mut Vec<crate::render_scene::OwnedDraw>,
    ) -> Result<Self, String> {
        use crate::{render::SurfaceMode, render_scene::OwnedDraw, texture::TextureSet};
        let textures = TextureSet::parse(payload).map_err(|e| e.to_string())?;
        let texture = textures
            .textures
            .get(2)
            .ok_or("lamp pool texture missing")?;
        let volume = draws.len();
        let mut outer =
            OwnedDraw::texture_page(SurfaceMode::LightPoolVolume, 1, 1, vec![255; 4].into());
        outer.diffuse_alpha = Some(Vec::new());
        draws.push(outer);
        let glow = draws.len();
        let mut inner = OwnedDraw::texture_page(
            SurfaceMode::LightPoolGlow,
            texture.width,
            texture.height,
            crate::texture::loader_pixels(texture).into(),
        );
        inner.diffuse_alpha = Some(Vec::new());
        draws.push(inner);
        Ok(Self {
            volume,
            glow,
            lamps,
        })
    }

    pub fn fill(
        &self,
        draws: &mut [crate::render_scene::OwnedDraw],
        view: glam::Mat4,
        projection: [f32; 16],
        near: f32,
        threshold: u8,
        enabled: bool,
    ) -> Result<Vec<usize>, String> {
        for index in [self.volume, self.glow] {
            let draw = draws.get_mut(index).ok_or("lamp pool surface missing")?;
            draw.view_projection = Some(projection);
            draw.vertices.clear();
            draw.diffuse_alpha
                .as_mut()
                .ok_or("lamp pool alpha missing")?
                .clear();
        }
        // 004b69f0 culls each lamp's ground center with 004e07d0. Its far
        // limit (0055904c) is set near 1e6 by 004524b0, so the fade, not the
        // far plane, bounds the drawn range.
        for &lamp in &self.lamps {
            let center =
                view.transform_point3(glam::Vec3::new(lamp.position[0], 0., lamp.position[2]));
            let outside_side = [center.x, center.y]
                .into_iter()
                .zip([projection[0], projection[5]])
                .any(|(position, scale)| {
                    let extent = (f64::from(scale).hypot(1.) * f64::from(RADIUS)) as f32;
                    let distance = (f64::from(position) * f64::from(scale)).abs() as f32;
                    f64::from(distance) - f64::from(extent) >= f64::from(center.z)
                });
            if center.z + RADIUS < near || outside_side {
                continue;
            }
            for (part, ring) in rings(lamp, center.z, threshold, enabled)
                .into_iter()
                .enumerate()
            {
                let index = if part < 2 { self.volume } else { self.glow };
                let draw = &mut draws[index];
                let alpha = draw
                    .diffuse_alpha
                    .as_mut()
                    .ok_or("lamp pool alpha missing")?;
                for index in ring.indices {
                    let vertex = ring.vertices[usize::from(index)];
                    let position = view.transform_point3(glam::Vec3::from_array(vertex.position));
                    let rgb = [16, 8, 0].map(|shift| ((vertex.argb >> shift) & 255) as f32 / 255.);
                    draw.vertices.push([
                        position.x,
                        position.y,
                        position.z,
                        rgb[0],
                        rgb[1],
                        rgb[2],
                        vertex.uv[0],
                        vertex.uv[1],
                    ]);
                    alpha.push((vertex.argb >> 24) as u8);
                }
            }
        }
        for index in [self.volume, self.glow] {
            let draw = &mut draws[index];
            draw.specular
                .as_mut()
                .ok_or("lamp pool specular missing")?
                .resize(draw.vertices.len(), [0.; 3]);
        }
        Ok(vec![self.volume, self.glow])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lamp() -> Halo {
        Halo {
            position: [10., 17., -100.],
            threshold: 100,
            radius: 5.,
            depth_offset: -1.,
            argb: 0xffffffff,
        }
    }
    #[cfg(feature = "graphics")]
    #[test]
    fn synthetic_production_pool_surfaces_rebuild_with_camera_and_gate() {
        use crate::{render::SurfaceMode, render_scene::OwnedDraw};
        let mut draws = [SurfaceMode::LightPoolVolume, SurfaceMode::LightPoolGlow].map(|mode| {
            let mut draw = OwnedDraw::texture_page(mode, 1, 1, vec![255; 4].into());
            draw.diffuse_alpha = Some(Vec::new());
            draw
        });
        let layer = Layer {
            volume: 0,
            glow: 1,
            lamps: vec![lamp()],
        };
        let view = glam::Mat4::from_cols_array(&crate::camera::view([0.; 3], [0., 0., -1.]));
        let projection = glam::Mat4::IDENTITY.to_cols_array();
        layer
            .fill(&mut draws, view, projection, 1., 100, true)
            .unwrap();
        assert_eq!(draws[0].borrowed().vertices.len(), 120);
        assert_eq!(draws[1].borrowed().vertices.len(), 60);
        assert_eq!(draws[0].vertices[0][..3], [-10., 17., 100.]);
        assert_eq!(draws[0].diffuse_alpha.as_ref().unwrap()[0], 0);
        assert_eq!(draws[0].diffuse_alpha.as_ref().unwrap()[60], 255);
        for (threshold, enabled) in [(101, true), (255, true), (0, false)] {
            layer
                .fill(&mut draws, view, projection, 1., threshold, enabled)
                .unwrap();
            assert!(draws.iter().all(|d| d.borrowed().vertices.is_empty()));
        }
        let distant = glam::Mat4::from_translation(glam::Vec3::new(0., 0., 501.)) * view;
        layer
            .fill(&mut draws, distant, projection, 1., 0, true)
            .unwrap();
        assert!(draws.iter().all(|d| d.vertices.is_empty()));
        layer
            .fill(&mut draws, view, projection, 1., 0, true)
            .unwrap();
        assert_eq!(draws[1].vertices.len(), 60);
    }

    #[test]
    fn synthetic_pool_tessellation_radii_winding_and_colours() {
        for depth in [1., 10., 100., 300., 400.] {
            let rings = rings(lamp(), depth, 100, true);
            assert_eq!(rings.len(), 3);
            for ring in &rings {
                assert_eq!(ring.vertices.len(), segments(depth) + 1);
                assert_eq!(ring.indices.len(), segments(depth) * 3);
                assert!(
                    ring.indices
                        .iter()
                        .all(|&i| usize::from(i) < ring.vertices.len())
                );
                assert_eq!(ring.vertices[0].position, lamp().position);
            }
            assert_eq!(rings[0].indices[..3], [0, 1, 2]);
            assert_eq!(rings[1].indices[..3], [0, 2, 1]);
            for (ring, radius, height) in [(&rings[0], 8.5, 0.), (&rings[2], 2.8305, 11.339)] {
                for v in &ring.vertices[1..] {
                    let r = ((v.position[0] - 10.).powi(2) + (v.position[2] + 100.).powi(2)).sqrt();
                    // The original approximate quarter-turn cache slightly distorts radii.
                    assert!((r - radius).abs() < 0.005, "{r} vs {radius}");
                    assert!((v.position[1] - height).abs() < 0.0001);
                    assert_eq!(v.uv[1], 1.);
                }
            }
        }
        assert_eq!(segments(1.), 64);
        assert_eq!(segments(100.), 20);
        assert_eq!(segments(300.), 7);
        let close = rings(lamp(), 100., 0, true);
        assert_eq!(close[0].vertices[0].argb, 0x00808080);
        assert_eq!(close[1].vertices[0].argb, 0xffffffff);
        assert_eq!(close[2].vertices[0].argb, 0xff7f7f7f);
        assert_eq!(close[2].vertices[1].argb, 0xff000000);
    }
    #[test]
    fn synthetic_pool_fade_and_shared_night_option_gate() {
        assert_eq!(fade(299.), 1.);
        assert_eq!(fade(300.), 1.);
        assert!((fade(400.) - 0.5).abs() < 0.00001);
        // At exactly 500 the binary32 0.005 leaves a tiny positive fade: the
        // volume passes are submitted with alpha 0 and the glow is skipped.
        assert!(fade(500.) > 0.);
        assert_eq!(rings(lamp(), 500., 0, true).len(), 2);
        assert_eq!(fade(501.), 0.);
        // Glow rounds the binary32 product, ties to even: 0.5*255 -> 128.
        assert_eq!(fade(400.) * 255., 127.5);
        assert!(rings(lamp(), 501., 0, true).is_empty());
        assert!(rings(lamp(), 100., 0, false).is_empty());
        assert!(rings(lamp(), 100., 255, true).is_empty());
        assert!(rings(lamp(), 100., 101, true).is_empty());
        assert!(!rings(lamp(), 100., 100, true).is_empty());
        assert!(!rings(lamp(), 100., 0, true).is_empty());
        let halfway = rings(lamp(), 400., 0, true);
        assert_eq!(halfway[1].vertices[0].argb >> 24, 127);
        assert_eq!(halfway[2].vertices[0].argb & 255, 64);
    }
}
