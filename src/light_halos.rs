//! City lamp glows from 00449360/004b69f0 and road lights from 004b5930.
use crate::{board::Block, mesh::CachedMesh};

pub const TEXTURES: &str = "gamedata2/billboardrepeattextures/textures.gtd";
pub const ROAD_LIGHTS: &str = "gamedata2/road_lights/road_lights.tm";
pub const WATERFRONT_LIGHTS: &str =
    "gamedata2/New_building stuff/surrounds/river_bank/conelight_locators.tm";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Halo {
    /// City world coordinates, after the collector's input Z conversion.
    pub position: [f32; 3],
    pub threshold: i16,
    pub radius: f32,
    pub depth_offset: f32,
    pub argb: u32,
}
impl Halo {
    pub fn visible(self, night_threshold: u8, enabled: bool) -> bool {
        enabled && night_threshold != 255 && i16::from(night_threshold) <= self.threshold
    }
}

/// 00449360 initializes this local RNG independently of the simulation streams.
pub fn lamps(blocks: &[Block]) -> Vec<Halo> {
    let mut random = 0x016569bf_u32;
    crate::street_furniture::lamps(blocks)
        .into_iter()
        .map(|pose| {
            random = random.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
            let mut position = pose.position;
            // Glow centers are at +/-38; the model bases are at +/-41.
            match pose.yaw as u32 {
                270 => position[2] -= 3.,
                180 => position[0] -= 3.,
                90 => position[2] += 3.,
                _ => position[0] += 3.,
            }
            position[1] = 17.;
            Halo {
                position,
                threshold: (((random >> 16) & 0x7fff) % 257) as i16,
                radius: 5.,
                depth_offset: -1.,
                argb: 0xffffffff,
            }
        })
        .collect()
}

/// 004b5860 admits locator i only below executable table545568[i&15].
/// The comparison is strict here, unlike the street lamps' inclusive threshold.
pub fn road_lights(mesh: &CachedMesh<'_>, thresholds: &[u8; 16]) -> Result<Vec<Halo>, String> {
    mesh.opaque_records_28
        .as_chunks::<28>()
        .0
        .iter()
        .enumerate()
        .map(|(i, record)| {
            let mut position = std::array::from_fn(|axis| {
                f32::from_le_bytes(record[axis * 4..axis * 4 + 4].try_into().unwrap())
            });
            if !position.into_iter().all(f32::is_finite) {
                return Err("non-finite light locator".into());
            }
            // 004b5860 sends model-local Z straight to 004ec0b0, which
            // negates it. 004b69f0's lamp caller cancels that negation first.
            position[2] = -position[2];
            Ok(Halo {
                position,
                threshold: i16::from(thresholds[i & 15]) - 1,
                radius: 5.,
                depth_offset: -2.,
                argb: 0xafffffaf,
            })
        })
        .collect()
}

#[cfg(feature = "graphics")]
#[derive(Clone)]
pub(crate) struct Layer {
    pub groups: Vec<(usize, Vec<Halo>)>,
    pub threshold: u8,
    pub enabled: bool,
    pub detail: u8,
    road_draw: Option<usize>,
    pools: Option<crate::light_pools::Layer>,
    pub camera: crate::flyby::CameraSample,
    pub viewport: [u32; 2],
    signals: Option<SignalGlows>,
}

/// 0x44dbe0's junction traffic-light glow points. Unlike the lamps they have
/// no night threshold and no Halos option: they follow the live crossing
/// masks and the camera every city frame.
#[cfg(feature = "graphics")]
#[derive(Clone)]
struct SignalGlows {
    draw: usize,
    posts: Vec<crate::traffic_lights::Post>,
    lanes: Vec<[u8; 32]>,
}

#[cfg(feature = "graphics")]
impl Layer {
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        original: crate::render_scene::OriginalAssets<'_>,
        lamps: Vec<Halo>,
        signals: (Vec<crate::traffic_lights::Post>, Vec<[u8; 32]>),
        draws: &mut Vec<crate::render_scene::OwnedDraw>,
        threshold: u8,
        camera: crate::flyby::CameraSample,
        viewport: [u32; 2],
    ) -> Result<Self, String> {
        use crate::{
            archive::ArchiveIndex, pe::PeImage, render::SurfaceMode, render_scene::OwnedDraw,
            texture::TextureSet,
        };
        let index = ArchiveIndex::parse(original.directory, original.archive.len() as u64)
            .map_err(|e| e.to_string())?;
        let payload = |path: &str| {
            let e = index
                .find(path)
                .ok_or_else(|| format!("missing halo asset {path}"))?;
            original
                .archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("halo asset outside archive".to_string())
        };
        let image = PeImage::parse(original.executable)?;
        let thresholds = image
            .at(0x545568, 16)?
            .try_into()
            .map_err(|_| "halo threshold table")?;
        let mut roads = Vec::new();
        for path in [WATERFRONT_LIGHTS, ROAD_LIGHTS] {
            roads.extend(road_lights(
                &CachedMesh::parse(payload(path)?)?,
                thresholds,
            )?);
        }
        let textures = TextureSet::parse(payload(TEXTURES)?).map_err(|e| e.to_string())?;
        let pools = Some(crate::light_pools::Layer::load(
            payload(crate::light_pools::TEXTURES)?,
            lamps.clone(),
            draws,
        )?);
        let mut page = |record: usize| -> Result<usize, String> {
            let texture = textures
                .textures
                .get(record)
                .ok_or("halo texture record missing")?;
            let mut draw = OwnedDraw::texture_page(
                SurfaceMode::LightHalos,
                texture.width,
                texture.height,
                crate::texture::loader_pixels(texture).into(),
            );
            draw.diffuse_alpha = Some(Vec::new());
            draws.push(draw);
            Ok(draws.len() - 1)
        };
        let mut groups = Vec::new();
        for (record, instances) in [(1, roads), (2, lamps)] {
            groups.push((page(record)?, instances));
        }
        // 0x44dbe0 selects texture 0x687c80, the lamps' record.
        let (posts, lanes) = signals;
        let signals = Some(SignalGlows {
            draw: page(2)?,
            posts,
            lanes,
        });
        let road_draw = groups.first().map(|(draw, _)| *draw);
        Ok(Self {
            groups,
            threshold,
            enabled: true,
            detail: 2,
            road_draw,
            pools,
            camera,
            viewport,
            signals,
        })
    }

    /// The draw holding the junction traffic-light glow points.
    #[cfg(test)]
    pub fn signal_draw(&self) -> Option<usize> {
        self.signals.as_ref().map(|signals| signals.draw)
    }

    /// Replace the crossing masks the signal glows follow. Returns the
    /// refilled draw when they changed.
    pub fn set_signal_lanes(
        &mut self,
        lanes: &[[u8; 32]],
        draws: &mut [crate::render_scene::OwnedDraw],
    ) -> Result<Vec<usize>, String> {
        let Some(signals) = &mut self.signals else {
            return Ok(Vec::new());
        };
        if signals.lanes == lanes {
            return Ok(Vec::new());
        }
        signals.lanes = lanes.to_vec();
        let view = self.view();
        let [near, far] = crate::camera::city_clip(self.camera.eye[1])?;
        let projection = self.projection(near, far)?;
        self.fill_signals(draws, view, projection, near, far)
    }

    fn view(&self) -> glam::Mat4 {
        glam::Mat4::from_cols_array(&crate::camera::view_with_up(
            self.camera.eye,
            self.camera.target,
            self.camera.up,
        ))
    }

    fn projection(&self, near: f32, far: f32) -> Result<[f32; 16], String> {
        crate::camera::projection(
            self.camera.vertical_fov,
            near,
            far,
            self.viewport[0] as i32,
            self.viewport[1] as i32,
            [0., 1., 0., 1.],
        )
    }

    fn fill_signals(
        &self,
        draws: &mut [crate::render_scene::OwnedDraw],
        view: glam::Mat4,
        projection: [f32; 16],
        near: f32,
        far: f32,
    ) -> Result<Vec<usize>, String> {
        let Some(signals) = &self.signals else {
            return Ok(Vec::new());
        };
        let scene_view =
            crate::camera::scene_view_with_up(self.camera.eye, self.camera.target, self.camera.up);
        let draw = draws
            .get_mut(signals.draw)
            .ok_or("signal glow surface missing")?;
        draw.view_projection = Some(projection);
        draw.vertices.clear();
        draw.diffuse_alpha
            .as_mut()
            .ok_or("signal glow alpha missing")?
            .clear();
        for post in &signals.posts {
            let Some(glow) = post.glow(&signals.lanes)? else {
                continue;
            };
            let alpha = post.glow_alpha(&scene_view);
            if alpha <= 0 {
                continue;
            }
            let argb = ((alpha as u32) << 24).wrapping_add(glow.rgb());
            for position in post.glow_points(glow) {
                push_billboard(
                    draw,
                    view,
                    near,
                    far,
                    Halo {
                        position,
                        threshold: 0,
                        radius: crate::traffic_lights::GLOW_RADIUS,
                        depth_offset: crate::traffic_lights::GLOW_DEPTH_OFFSET,
                        argb,
                    },
                )?;
            }
        }
        draw.specular
            .as_mut()
            .ok_or("signal glow specular missing")?
            .resize(draw.vertices.len(), [0.; 3]);
        Ok(vec![signals.draw])
    }
    pub fn fill(&self, draws: &mut [crate::render_scene::OwnedDraw]) -> Result<Vec<usize>, String> {
        let [near, far] = crate::camera::city_clip(self.camera.eye[1])?;
        let projection = self.projection(near, far)?;
        let view = self.view();
        let mut changed = if let Some(pools) = &self.pools {
            pools.fill(draws, view, projection, near, self.threshold, self.enabled)?
        } else {
            Vec::new()
        };
        for (index, halos) in &self.groups {
            let draw = draws.get_mut(*index).ok_or("halo surface missing")?;
            draw.view_projection = Some(projection);
            draw.vertices.clear();
            draw.diffuse_alpha
                .as_mut()
                .ok_or("halo alpha missing")?
                .clear();
            let enabled = self.enabled && (self.road_draw != Some(*index) || self.detail != 0);
            for halo in halos.iter().filter(|h| h.visible(self.threshold, enabled)) {
                push_billboard(draw, view, near, far, *halo)?;
            }
            draw.specular
                .as_mut()
                .ok_or("halo specular missing")?
                .resize(draw.vertices.len(), [0.; 3]);
            changed.push(*index);
        }
        changed.extend(self.fill_signals(draws, view, projection, near, far)?);
        Ok(changed)
    }
}

/// 004f4230 expands each point in eye space and adds depth before projection.
#[cfg(feature = "graphics")]
fn push_billboard(
    draw: &mut crate::render_scene::OwnedDraw,
    view: glam::Mat4,
    near: f32,
    far: f32,
    halo: Halo,
) -> Result<(), String> {
    let [x, y, z] = halo.position;
    let center = view.transform_point3(glam::Vec3::new(x, y, z));
    if center.z + halo.radius < near || center.z - halo.radius > far {
        return Ok(());
    }
    let rgb = [16, 8, 0].map(|shift| ((halo.argb >> shift) & 255) as f32 / 255.);
    let alpha = draw.diffuse_alpha.as_mut().ok_or("halo alpha missing")?;
    for [u, v] in [[0., 1.], [1., 1.], [1., 0.], [0., 1.], [1., 0.], [0., 0.]] {
        alpha.push((halo.argb >> 24) as u8);
        draw.vertices.push([
            center.x + (u * 2. - 1.) * halo.radius,
            center.y + (1. - v * 2.) * halo.radius,
            center.z + halo.depth_offset,
            rgb[0],
            rgb[1],
            rgb[2],
            u,
            v,
        ]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_lamp_instances_and_threshold_gate() {
        let block = Block {
            id: 0,
            name: String::new(),
            text_id: 0,
            kind: 0,
            center: [100., -200.],
            size: [80.; 2],
            pavement_color: 0,
            flags: 0,
            street_sides: 1 | 4,
        };
        let halos = lamps(&[block]);
        assert_eq!(
            halos.iter().map(|h| h.position).collect::<Vec<_>>(),
            [
                [84., 17., -162.],
                [116., 17., -162.],
                [116., 17., -238.],
                [84., 17., -238.]
            ]
        );
        assert!(
            halos
                .iter()
                .all(|h| h.radius == 5. && h.argb == 0xffffffff && h.depth_offset == -1.)
        );
        for halo in halos {
            assert!(halo.visible(0, true));
            assert!(halo.visible(halo.threshold.min(254) as u8, true));
            assert!(!halo.visible(255, true));
            assert!(!halo.visible(0, false));
            if halo.threshold < 254 {
                assert!(!halo.visible(halo.threshold as u8 + 1, true));
            }
        }
    }
    #[test]
    fn synthetic_sun_gate_is_discrete_and_drives_lamp_list() {
        let mut tga = vec![0; 18 + 384 * 177 * 4];
        tga[2] = 2;
        tga[16] = 32;
        tga[17] = 8;
        tga[12..14].copy_from_slice(&384_u16.to_le_bytes());
        tga[14..16].copy_from_slice(&177_u16.to_le_bytes());
        let markers = 18 + (177 - 97) * 384 * 4;
        tga[markers + 112 * 4] = 64;
        tga[markers + 352 * 4] = 192;
        // Contrasting adjacent samples prove that the gate is not interpolated.
        tga[18 + 96 * 4] = 100;
        tga[18 + 97 * 4] = 255;
        let colors = crate::lighting::SunColors::parse(&tga).unwrap();
        let halo = Halo {
            position: [0., 17., 0.],
            threshold: 100,
            radius: 5.,
            depth_offset: -1.,
            argb: 0xffffffff,
        };
        assert!(halo.visible(colors.sample(6, 0).unwrap().night_threshold, true));
        assert!(halo.visible(colors.sample(6, 38).unwrap().night_threshold, true));
        assert!(!halo.visible(colors.sample(6, 40).unwrap().night_threshold, true));
    }

    #[test]
    fn synthetic_locator_lights_keep_strict_threshold_and_collector_coordinates() {
        let mut bytes = vec![0; 164];
        bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&1_u32.to_le_bytes());
        for value in [12_f32, 10., 30., 0., 0., 0.] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        let mesh = CachedMesh::parse(&bytes).unwrap();
        let halo = road_lights(&mesh, &[100; 16]).unwrap()[0];
        assert_eq!(halo.position, [12., 10., -30.]);
        assert_eq!(
            (halo.radius, halo.depth_offset, halo.argb),
            (5., -2., 0xafffffaf)
        );
        assert!(halo.visible(99, true));
        assert!(!halo.visible(100, true));
        assert!(!road_lights(&mesh, &[0; 16]).unwrap()[0].visible(0, true));
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn synthetic_production_billboards_follow_option_and_night_gate() {
        use crate::{render::SurfaceMode, render_scene::OwnedDraw};
        let mut draw = OwnedDraw::texture_page(SurfaceMode::LightHalos, 1, 1, vec![255; 4].into());
        draw.diffuse_alpha = Some(Vec::new());
        let mut draws = vec![draw];
        let mut layer = Layer {
            groups: vec![(
                0,
                vec![Halo {
                    position: [0., 17., -100.],
                    threshold: 100,
                    radius: 5.,
                    depth_offset: -1.,
                    argb: 0xffffffff,
                }],
            )],
            threshold: 100,
            enabled: true,
            detail: 2,
            road_draw: None,
            pools: None,
            signals: None,
            camera: crate::flyby::CameraSample {
                eye: [0., 17., 0.],
                target: [0., 17., -100.],
                up: [0., 1., 0.],
                vertical_fov: 53.,
            },
            viewport: [640, 480],
        };
        layer.fill(&mut draws).unwrap();
        assert_eq!(draws[0].vertices.len(), 6);
        assert_eq!(draws[0].vertices[0], [-5., -5., 99., 1., 1., 1., 0., 1.]);
        assert_eq!(draws[0].diffuse_alpha.as_ref().unwrap(), &[255; 6]);
        layer.detail = 0;
        layer.fill(&mut draws).unwrap();
        assert_eq!(draws[0].vertices.len(), 6, "lamp halos survive low detail");
        layer.road_draw = Some(0);
        layer.fill(&mut draws).unwrap();
        assert!(
            draws[0].vertices.is_empty(),
            "452770 hides locator lights at low detail"
        );
        layer.detail = 2;
        for (enabled, threshold, count) in
            [(false, 0, 0), (true, 255, 0), (true, 101, 0), (true, 0, 6)]
        {
            layer.enabled = enabled;
            layer.threshold = threshold;
            layer.fill(&mut draws).unwrap();
            assert_eq!(draws[0].borrowed().vertices.len(), count);
        }
    }

    #[test]
    #[ignore = "requires original SUNCOLS image"]
    fn original_requested_hours_use_discrete_night_samples() {
        let colors = crate::lighting::SunColors::parse(
            &std::fs::read(crate::fixtures::path("SUNCOLS")).unwrap(),
        )
        .unwrap();
        for (hour, minute, expected) in [
            (21, 50, 122),
            (2, 35, 0),
            (6, 0, 100),
            (12, 0, 255),
            (18, 14, 255),
            (8, 10, 255),
        ] {
            let sample = colors.sample(hour, minute * 625 / 60).unwrap();
            assert_eq!(sample.night_threshold, expected);
        }
    }
}
