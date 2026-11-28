//! Camera-facing result points for the original 4ec120/4ec270 draw stream.
use super::fireworks::{POINT_CAPACITY as RESULT_POINT_CAPACITY, Point};
use openmt2001::{flyby::CameraSample, render::SurfaceMode, render_scene::OwnedDraw};
use std::sync::Arc;

// Retained shared dust/pulse/generic admission. Result mode owns750 separately.
const CAPACITY: usize = 512;
const CORNERS: [[f32; 2]; 4] = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]];
const UV: [[f32; 2]; 4] = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
const TRIANGLES: [usize; 6] = [0, 1, 2, 0, 2, 3];

const FIREWORK_BANK: &str = "gamedata2/billboardrepeattextures/textures.gtd";
const FIREWORK_RECORD: usize = 1; // 4bd0c6 -> 687c78+4, selected by4b1775.

pub struct FireworkTexture {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}
impl FireworkTexture {
    pub fn load(index: &openmt2001::archive::ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let entry = index
            .find(FIREWORK_BANK)
            .ok_or("original result firework texture bank missing")?;
        let end = entry
            .offset
            .checked_add(entry.length)
            .ok_or("result firework texture range overflow")?;
        let bytes = archive
            .get(
                usize::try_from(entry.offset).map_err(|_| "result firework texture offset")?
                    ..usize::try_from(end).map_err(|_| "result firework texture end")?,
            )
            .ok_or("result firework texture range")?;
        let set = openmt2001::texture::TextureSet::parse(bytes).map_err(|e| e.to_string())?;
        let image = set
            .textures
            .get(FIREWORK_RECORD)
            .ok_or("original result firework texture record missing")?;
        Ok(Self {
            width: image.width,
            height: image.height,
            rgba: openmt2001::texture::loader_pixels(image).into(),
        })
    }
    pub fn empty_draw(&self) -> OwnedDraw {
        let mut draw = empty_draw();
        draw.mode = SurfaceMode::ResultFireworks;
        // The result path prepares once, then update_surface requires enough
        // retained GPU capacity for the complete admitted collector prefix.
        draw.vertices.resize(RESULT_POINT_CAPACITY * 6, [0.; 8]);
        draw.specular
            .as_mut()
            .unwrap()
            .resize(RESULT_POINT_CAPACITY * 6, [0.; 3]);
        draw.diffuse_alpha
            .as_mut()
            .unwrap()
            .resize(RESULT_POINT_CAPACITY * 6, 0);
        draw.width = self.width;
        draw.height = self.height;
        draw.rgba = self.rgba.clone();
        // Original budget upload builds the resolved page's mip chain.
        // Mode12's linear min/mag, point mip and wrap use the existing sampler.
        draw.mipmaps = true;
        draw
    }
}

pub fn empty_draw() -> OwnedDraw {
    // Retained shared untextured helper for dust/pulse and generic callers.
    // Authored result fireworks use FireworkTexture's dedicated material.
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
    let capacity = match draw.mode {
        SurfaceMode::ResultParticles | SurfaceMode::ChimneySmoke => CAPACITY,
        SurfaceMode::ResultFireworks => RESULT_POINT_CAPACITY,
        _ => return Err("result particle material changed".into()),
    };
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
    for point in points.iter().take(capacity) {
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
        .ok_or("result particle specular missing")?
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
    fn result_750_and_shared_512_keep_ordered_point_fields() {
        // Controlled no-asset field oracle: the original collector takes the
        // first750, generic callers retain512. Geometry uses the established
        // axis-aligned camera and radius=size*.5, not a mirrored fill helper.
        let texture = FireworkTexture {
            width: 1,
            height: 1,
            rgba: Arc::from([255, 255, 255, 255]),
        };
        let mut result = texture.empty_draw();
        let mut shared = empty_draw();
        for (draw, count) in [(&result, 4500), (&shared, 3072)] {
            assert_eq!(draw.vertices.len(), count);
            assert_eq!(draw.specular.as_ref().unwrap().len(), count);
            assert_eq!(draw.diffuse_alpha.as_ref().unwrap().len(), count);
            assert_eq!(draw.borrowed().vertices.len(), count);
        }
        let points = (0..900)
            .map(|id| Point {
                position: [id as f32 * 4., 8., -16.],
                size: 2.,
                argb: (((64 + id % 128) as u32) << 24)
                    | ((id as u8 as u32) << 16)
                    | (((id >> 8) as u8 as u32) << 8)
                    | 0x33,
            })
            .collect::<Vec<_>>();
        let camera = CameraSample {
            eye: [0.; 3],
            target: [0., 0., -1.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        for count in [0, 1, 511, 512, 513, 749, 750, 751, 900, 1, 750] {
            fill(&mut result, &points[..count], camera).unwrap();
            fill(&mut shared, &points[..count], camera).unwrap();
            for (draw, limit) in [(&result, 750), (&shared, 512)] {
                let admitted = count.min(limit);
                assert_eq!(draw.vertices.len(), admitted * 6);
                assert_eq!(draw.specular.as_ref().unwrap().len(), admitted * 6);
                assert_eq!(draw.diffuse_alpha.as_ref().unwrap().len(), admitted * 6);
                assert_eq!(draw.borrowed().vertices.len(), admitted * 6);
                for id in 0..admitted {
                    let x = id as f32 * 4.;
                    let expected = [
                        [x - 1., 7., 16.],
                        [x + 1., 7., 16.],
                        [x + 1., 9., 16.],
                        [x - 1., 7., 16.],
                        [x + 1., 9., 16.],
                        [x - 1., 9., 16.],
                    ];
                    let rgb = [
                        (id as u8) as f32 / 255.,
                        ((id >> 8) as u8) as f32 / 255.,
                        51. / 255.,
                    ];
                    for (offset, position) in expected.into_iter().enumerate() {
                        let vertex = &draw.vertices[id * 6 + offset];
                        assert_eq!(vertex[..3], position, "ordered geometry point{id}");
                        assert_eq!(vertex[3..6], rgb);
                        assert_eq!(
                            vertex[6..8],
                            [[0., 1.], [1., 1.], [1., 0.], [0., 1.], [1., 0.], [0., 0.]][offset]
                        );
                        assert_eq!(
                            draw.diffuse_alpha.as_ref().unwrap()[id * 6 + offset],
                            (64 + id % 128) as u8
                        );
                        assert_eq!(draw.specular.as_ref().unwrap()[id * 6 + offset], [0.; 3]);
                    }
                }
            }
        }
        assert_eq!(result.mode, SurfaceMode::ResultFireworks);
        assert_eq!(shared.mode, SurfaceMode::ResultParticles);
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires GPU adapter; retained result750 allocation/update boundary"]
    fn result_750_updates_prepared_surface_without_reallocation_api() {
        let texture = FireworkTexture {
            width: 1,
            height: 1,
            rgba: Arc::from([255, 255, 255, 255]),
        };
        let mut draw = texture.empty_draw();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let matrix = Mat4::IDENTITY.to_cols_array();
        let mut prepared = renderer.prepare(&[draw.borrowed()], matrix, None).unwrap();
        let camera = CameraSample {
            eye: [0.; 3],
            target: [0., 0., -1.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let points = vec![
            Point {
                position: [0., 0., -1.],
                argb: 0x804080ff,
                size: 2.
            };
            751
        ];
        // update_surface is the actual result caller's bounded API, not the
        // dynamic-growth API which could conceal an undersized initial draw.
        for count in [513, 750, 1, 749, 751, 0, 750] {
            fill(&mut draw, &points[..count], camera).unwrap();
            assert_eq!(draw.borrowed().vertices.len(), count.min(750) * 6);
            renderer
                .update_surface(&mut prepared, 0, &draw.borrowed(), matrix, None)
                .unwrap();
        }
    }

    #[test]
    #[ignore = "requires supplied original archive; no GPU"]
    fn supplied_fireworks_texture_identity_and_point_fields() {
        use openmt2001::{
            archive::ArchiveIndex,
            texture::{TextureSet, loader_pixels},
        };
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find("gamedata2/billboardrepeattextures/textures.gtd")
            .unwrap();
        assert_eq!(
            (entry.offset, entry.length),
            (41063112, 132268),
            "pinned supplied corpus"
        );
        let bytes = &archive[entry.offset as usize..(entry.offset + entry.length) as usize];
        let set = TextureSet::parse(bytes).unwrap();
        assert_eq!(set.textures.len(), 3);
        let image = &set.textures[1];
        assert_eq!(
            (image.width, image.height, image.header[0]),
            (128, 128, 0x8000)
        );
        assert!(image.texels.chunks_exact(4).all(|p| p[3] == 255));
        for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127)] {
            assert_eq!(
                &image.texels[(y * 128 + x) * 4..(y * 128 + x + 1) * 4],
                &[0, 0, 0, 255]
            );
        }
        assert_eq!(
            &image.texels[(64 * 128 + 64) * 4..(64 * 128 + 65) * 4],
            &[255, 255, 252, 255]
        );
        let texture = FireworkTexture::load(&index, &archive).unwrap();
        let mut actual = texture.empty_draw();
        assert_eq!(actual.mode, SurfaceMode::ResultFireworks);
        assert_eq!((actual.width, actual.height), (128, 128));
        assert_eq!(actual.rgba.as_ref(), loader_pixels(image));
        assert!(actual.mipmaps);
        assert!(
            FireworkTexture::load(
                &index,
                &archive[..(entry.offset + entry.length - 1) as usize]
            )
            .is_err()
        );
        let mut missing = index.clone();
        missing
            .entries
            .retain(|e| e.path.replace('\\', "/").to_ascii_lowercase() != FIREWORK_BANK);
        assert!(FireworkTexture::load(&missing, &archive).is_err());
        let mut shared = empty_draw();
        assert_eq!(shared.mode, SurfaceMode::ResultParticles);
        assert_eq!(
            (shared.width, shared.height, shared.rgba.as_ref()),
            (1, 1, &[255, 255, 255, 255][..])
        );
        assert!(!shared.mipmaps);
        let points = [Point {
            position: [10., 20., -30.],
            argb: 0x804080ff,
            size: 2.,
        }];
        let camera = CameraSample {
            eye: [10., 20., 50.],
            target: [10., 20., 0.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        fill(&mut shared, &points, camera).unwrap();
        fill(&mut actual, &points, camera).unwrap();
        assert_eq!(
            actual.vertices, shared.vertices,
            "material-only: preserve geometry/UV/RGB"
        );
        assert_eq!(actual.diffuse_alpha, shared.diffuse_alpha);
        assert_eq!(actual.diffuse_alpha.as_ref().unwrap().as_slice(), &[128; 6]);
        assert_eq!(actual.vertices[0][3..6], [64. / 255., 128. / 255., 1.]);
    }

    // Controlled material fixture: no original payload or simulation changes.
    #[cfg(feature = "capture")]
    fn material_quad(mode: SurfaceMode, bounds: [f32; 4], z: f32, color: [u8; 4]) -> OwnedDraw {
        let mut draw = OwnedDraw::texture_page(mode, 1, 1, Arc::<[u8]>::from(color));
        // No per-vertex overrides: zero specular, opaque diffuse alpha.
        draw.specular = None;
        draw.diffuse_alpha = None;
        draw.mipmaps = false;
        let [l, b, r, t] = bounds;
        let positions = [[l, b, z], [r, b, z], [r, t, z], [l, t, z]];
        draw.vertices = TRIANGLES
            .into_iter()
            .map(|i| {
                let [x, y, z] = positions[i];
                [x, y, z, 1., 1., 1., UV[i][0], UV[i][1]]
            })
            .collect();
        draw
    }

    #[cfg(feature = "capture")]
    fn material_readback(renderer: &Renderer, draws: &[OwnedDraw]) -> Vec<u8> {
        let size = wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        };
        let texture = |format, usage| {
            renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("firework material regression"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = texture(
            openmt2001::render::COLOR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = texture(
            openmt2001::render::DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let prepared = renderer
            .prepare(
                &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
                Mat4::IDENTITY.to_cols_array(),
                None,
            )
            .unwrap();
        renderer.draw_prepared(
            &color.create_view(&Default::default()),
            &depth.create_view(&Default::default()),
            &prepared,
        );
        let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("firework material readback"),
            size: 64 * 64 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = renderer.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            size,
        );
        renderer.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        renderer
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let bytes = buffer.slice(..).get_mapped_range().unwrap().to_vec();
        buffer.unmap();
        bytes
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires a GPU adapter; controlled offscreen material fixture"]
    fn result_fireworks_additive_depth_and_shared_material_boundary() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let camera = CameraSample {
            eye: [0.; 3],
            target: [0., 0., -1.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let point = Point {
            position: [0., 0., -0.5],
            argb: 0x80ff8040,
            size: 1.5,
        };
        let texture = FireworkTexture {
            width: 1,
            height: 1,
            rgba: Arc::from([128, 128, 128, 255]),
        };
        let background = || {
            material_quad(
                SurfaceMode::UiDepth,
                [-1., -1., 1., 1.],
                0.9,
                [20, 30, 40, 255],
            )
        };
        let mut one = texture.empty_draw();
        fill(&mut one, &[point], camera).unwrap();
        let mut two = texture.empty_draw();
        fill(&mut two, &[point, point], camera).unwrap();
        let pixel = |bytes: &[u8], x: usize| {
            <[u8; 3]>::try_from(&bytes[(32 * 64 + x) * 4..(32 * 64 + x) * 4 + 3]).unwrap()
        };
        let close = |actual: [u8; 3], expected: [u8; 3]| {
            for (a, e) in actual.into_iter().zip(expected) {
                assert!(a.abs_diff(e) <= 2, "{actual:?} != {expected:?}");
            }
        };
        close(
            pixel(
                &material_readback(&renderer, &[background(), one.clone()]),
                32,
            ),
            [84, 62, 56],
        );
        close(
            pixel(&material_readback(&renderer, &[background(), two]), 32),
            [148, 94, 72],
        );
        let front = material_quad(
            SurfaceMode::UiDepth,
            [-1., -1., 0., 1.],
            0.25,
            [20, 30, 40, 255],
        );
        let equal = material_quad(
            SurfaceMode::UiDepth,
            [0., -1., 1., 1.],
            0.5,
            [20, 30, 40, 255],
        );
        let depth = material_readback(&renderer, &[background(), front, equal, one.clone()]);
        close(pixel(&depth, 24), [20, 30, 40]); // Occluded.
        close(pixel(&depth, 40), [84, 62, 56]); // Equal depth is admitted.
        let behind = material_quad(
            SurfaceMode::UiDepth,
            [-1., -1., 1., 1.],
            0.6,
            [7, 11, 17, 255],
        );
        close(
            pixel(
                &material_readback(&renderer, &[background(), one, behind]),
                32,
            ),
            [7, 11, 17],
        );
        // Black opaque texels contribute zero with mode12; no fabricated alpha.
        let black = FireworkTexture {
            width: 1,
            height: 1,
            rgba: Arc::from([0, 0, 0, 255]),
        };
        let mut firework = black.empty_draw();
        fill(&mut firework, &[point], camera).unwrap();
        close(
            pixel(&material_readback(&renderer, &[background(), firework]), 32),
            [20, 30, 40],
        );
        let mut shared = empty_draw();
        shared.rgba = black.rgba.clone();
        fill(&mut shared, &[point], camera).unwrap();
        close(
            pixel(&material_readback(&renderer, &[background(), shared]), 32),
            [10, 15, 20],
        );
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires GPU; controlled original mode19 smoke material"]
    fn chimney_smoke_alpha_depth_and_no_depth_write() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let background = || {
            material_quad(
                SurfaceMode::UiDepth,
                [-1., -1., 1., 1.],
                0.9,
                [80, 120, 160, 255],
            )
        };
        let mut smoke = material_quad(SurfaceMode::ChimneySmoke, [-1., -1., 1., 1.], 0.5, [255; 4]);
        smoke.vertices.iter_mut().for_each(|v| v[3..6].fill(0.));
        smoke.diffuse_alpha = Some(vec![128; 6]);
        let pixel = |draws: &[OwnedDraw], x: usize| {
            let bytes = material_readback(&renderer, draws);
            <[u8; 3]>::try_from(&bytes[(32 * 64 + x) * 4..(32 * 64 + x) * 4 + 3]).unwrap()
        };
        assert_eq!(pixel(&[background(), smoke.clone()], 32), [40, 60, 80]);
        let front = material_quad(
            SurfaceMode::UiDepth,
            [-1., -1., 0., 1.],
            0.25,
            [80, 120, 160, 255],
        );
        let equal = material_quad(
            SurfaceMode::UiDepth,
            [0., -1., 1., 1.],
            0.5,
            [80, 120, 160, 255],
        );
        assert_eq!(
            pixel(
                &[background(), front.clone(), equal.clone(), smoke.clone()],
                24
            ),
            [80, 120, 160]
        );
        assert_eq!(
            pixel(&[background(), front, equal, smoke.clone()], 40),
            [40, 60, 80]
        );
        let behind = material_quad(
            SurfaceMode::UiDepth,
            [-1., -1., 1., 1.],
            0.6,
            [7, 11, 17, 255],
        );
        assert_eq!(pixel(&[background(), smoke, behind], 32), [7, 11, 17]);
    }

    #[test]
    #[ignore = "original renderer178 hardware/software vertex oracle /037f"]
    fn original_point_radius_vertices() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_POINT_VERTICES")).unwrap();
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
    #[ignore = "requires original v1.4 executable/archive and GPU result capture path"]
    fn captures_native_launch_particles() {
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
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
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let texture = FireworkTexture::load(&index, &archive).unwrap();
        let mut draw = texture.empty_draw();
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
