//! Camera-facing result points for the original 4ec120/4ec270 draw stream.
use super::fireworks::Point;
use openmt2001::{flyby::CameraSample, render::SurfaceMode, render_scene::OwnedDraw};
use std::sync::Arc;

const CAPACITY: usize = 512; // 454f10 passes 0x200 to 4ebf40.
const CORNERS: [[f32; 2]; 4] = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]];
const UV: [[f32; 2]; 4] = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
const TRIANGLES: [usize; 6] = [0, 1, 2, 0, 2, 3];

pub fn empty_draw() -> OwnedDraw {
    // 4ec270 passes a null texture binding. A white texel preserves diffuse
    // RGB and alpha, while ResultParticles selects depth-tested alpha blend.
    let mut draw = OwnedDraw::texture_page(
        SurfaceMode::ResultParticles,
        1,
        1,
        Arc::<[u8]>::from([255, 255, 255, 255]),
    );
    draw.mipmaps = false;
    draw.vertices = vec![[0.; 8]; CAPACITY * 6];
    draw.specular = Some(vec![[0.; 3]; CAPACITY * 6]);
    draw.diffuse_alpha = Some(vec![0; CAPACITY * 6]);
    draw
}

fn normalize(vector: [f32; 3]) -> Result<[f32; 3], String> {
    let norm = f64::from(vector[0])
        .hypot(f64::from(vector[1]))
        .hypot(f64::from(vector[2]));
    if !norm.is_finite() || norm < 1e-6 {
        return Err("result particle camera basis collapsed".into());
    }
    Ok(vector.map(|value| (f64::from(value) / norm) as f32))
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

///454f10 constructs the result renderer with sizeScale.5.
pub fn fill(draw: &mut OwnedDraw, points: &[Point], camera: CameraSample) -> Result<(), String> {
    fill_with_scale(draw, points, camera, 0.5)
}

///4ec120 stores size*renderer.scale;178 treats that value as the radius.
/// Dust4ba3c0 uses scale1, while results454f10 use.5. The world billboard
/// expansion is portable; original device rasterization remains unverified.
pub fn fill_with_scale(
    draw: &mut OwnedDraw,
    points: &[Point],
    camera: CameraSample,
    scale: f32,
) -> Result<(), String> {
    if draw.mode != SurfaceMode::ResultParticles {
        return Err("result particle material changed".into());
    }
    let forward = normalize(std::array::from_fn(|axis| {
        camera.target[axis] - camera.eye[axis]
    }))?;
    let right = normalize(cross(forward, camera.up))?;
    let up = normalize(cross(right, forward))?;
    draw.vertices.clear();
    let alpha = draw
        .diffuse_alpha
        .as_mut()
        .ok_or("result particle alpha missing")?;
    alpha.clear();
    for point in points.iter().take(CAPACITY) {
        let center = [point.position[0], point.position[1], -point.position[2]];
        let radius = point.size * scale;
        let rgb = [16, 8, 0].map(|shift| ((point.argb >> shift) & 255) as f32 / 255.);
        let opacity = (point.argb >> 24) as u8;
        for corner in TRIANGLES {
            let [u, v] = CORNERS[corner];
            let position: [f32; 3] = std::array::from_fn(|axis| {
                center[axis] + radius * (right[axis] * u + up[axis] * v)
            });
            draw.vertices.push([
                position[0],
                position[1],
                position[2],
                rgb[0],
                rgb[1],
                rgb[2],
                UV[corner][0],
                UV[corner][1],
            ]);
            alpha.push(opacity);
        }
    }
    draw.specular
        .as_mut()
        .unwrap()
        .resize(draw.vertices.len(), [0.; 3]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "capture")]
    use glam::Mat4;
    #[cfg(feature = "capture")]
    use openmt2001::{camera, pe::PeImage, render::Renderer};

    #[test]
    #[ignore = "original renderer178 hardware/software vertex oracle /037f"]
    fn original_point_radius_vertices() {
        let bytes = std::fs::read("work/frontend/native-point-vertices.bin").unwrap();
        let mut at = 0;
        let take = |at: &mut usize, n: usize| {
            let row = &bytes[*at..*at + n];
            *at += n;
            row
        };
        let word = |at: &mut usize| u32::from_le_bytes(take(at, 4).try_into().unwrap());
        assert_eq!(take(&mut at, 8), b"MTPTVX01");
        assert_eq!(word(&mut at), 0x37f);
        assert_eq!(word(&mut at), 16);
        for case in 0..16 {
            let software = word(&mut at) != 0;
            let scale = f32::from_bits(word(&mut at));
            let position: [f32; 3] = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let size = f32::from_bits(word(&mut at));
            let stride = word(&mut at) as usize;
            let native: Vec<Vec<u32>> = (0..4)
                .map(|_| (0..stride).map(|_| word(&mut at)).collect())
                .collect();
            let indices: [usize; 6] = std::array::from_fn(|_| {
                u16::from_le_bytes(take(&mut at, 2).try_into().unwrap()) as usize
            });
            assert_eq!(indices, TRIANGLES);
            let mut draw = empty_draw();
            fill_with_scale(
                &mut draw,
                &[Point {
                    position: [position[0], position[1], -position[2]],
                    argb: 0x7f123456,
                    size,
                }],
                CameraSample {
                    eye: [0.; 3],
                    target: [0., 0., -1.],
                    up: [0., 1., 0.],
                    vertical_fov: 45.,
                },
                scale,
            )
            .unwrap();
            for (vertex, &index) in draw.vertices.iter().zip(&indices) {
                let row = &native[index];
                if software {
                    // The portable path projects the world quad on the GPU;
                    // native software rounds center and radius separately.
                    // Compare within one screen-coordinate f32 ULP here.
                    for (axis, pixels, center) in [(0, 320., 320.), (1, -240., 240.)] {
                        let actual = (f64::from(vertex[axis]) / f64::from(position[2]) * pixels
                            + center) as f32;
                        let expected = f32::from_bits(row[axis]);
                        assert!(
                            (actual - expected).abs() <= f32::EPSILON * expected.abs().max(1.),
                            "software case{case}/axis{axis}: {actual} vs {expected}"
                        );
                    }
                } else {
                    assert_eq!(
                        vertex[..3].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        row[..3],
                        "hardware case{case}"
                    );
                }
                let uv = if software { 6 } else { 5 };
                assert_eq!(
                    vertex[6..8].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    row[uv..uv + 2],
                    "native UV case{case}"
                );
                assert_eq!(draw.diffuse_alpha.as_ref().unwrap()[0], 127);
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    fn flips_native_depth_and_preserves_argb() {
        let mut draw = empty_draw();
        fill(
            &mut draw,
            &[Point {
                position: [10., 20., -30.],
                argb: 0x804080ff,
                size: 2.,
            }],
            CameraSample {
                eye: [10., 20., 50.],
                target: [10., 20., 0.],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            },
        )
        .unwrap();
        assert_eq!(draw.vertices.len(), 6);
        assert!(draw.vertices.iter().all(|vertex| vertex[2] == 30.));
        assert_eq!(draw.diffuse_alpha.as_ref().unwrap(), &vec![128; 6]);
        assert_eq!(draw.vertices[0][3..6], [64. / 255., 128. / 255., 1.]);
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires original v1.4 executable and GPU result capture path"]
    fn captures_native_launch_particles() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        // Native 4e55d0 with the pinned process seed produces 13289; the
        // 40th 455080 frame is covered by the full differential fixture.
        let mut effect = super::super::fireworks::Fireworks::new(&image, 0, 13289).unwrap();
        for _ in 0..39 {
            effect.tick().unwrap();
        }
        let points = effect.tick().unwrap().to_vec();
        assert!(effect.submitted_count() > 300);
        let camera = CameraSample {
            eye: [-110., 35., 185.],
            target: [-110., 20., 95.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let mut draw = empty_draw();
        fill(&mut draw, &points, camera).unwrap();
        let [near, far] = camera::city_clip(camera.eye[1]).unwrap();
        let projection =
            camera::projection(camera.vertical_fov, near, far, 1280, 960, [0., 1., 0., 1.])
                .unwrap();
        let view = camera::view_with_up(camera.eye, camera.target, camera.up);
        let vp =
            (Mat4::from_cols_array(&projection) * Mat4::from_cols_array(&view)).to_cols_array();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let prepared = renderer.prepare(&[draw.borrowed()], vp, None).unwrap();
        let path = std::env::var("OPENMT2001_RESULT_PARTICLE_CAPTURE").unwrap();
        crate::capture_test::capture_standalone(&renderer, &prepared, &path);
        let mut reader =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()))
                .read_info()
                .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut pixels).unwrap();
        assert!(
            pixels[..frame.buffer_size()]
                .chunks_exact(4)
                .any(|pixel| pixel[..3] != [6, 9, 13])
        );
    }
}
