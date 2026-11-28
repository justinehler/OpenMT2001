//! Road vehicles from the retained 44d340 projection and group-4 model catalogue.
use crate::{object_scene::Model, people_scene::DrawingDistances};
use openmt2001::{camera, picking::QueryProjection, render_scene::OwnedDraw};
use std::{ops::Range, sync::Arc};

struct Policy {
    distances: DrawingDistances,
}

impl Policy {
    /// Road-vehicle setup 00452630, separate from the resident distances.
    fn from_detail(detail: u32) -> Result<Self, String> {
        let factor = f64::from(crate::people_scene::Policy::distance_factor(detail)?);
        // Both arguments are stored as f32 before 004e0b20 converts them.
        // The far expression uses the first store before its integer conversion.
        let first = (200. * factor + 200.) as f32;
        let far = (500. * factor + f64::from(first) + 300.) as f32;
        Ok(Self {
            distances: DrawingDistances {
                lod_distance: first.trunc(),
                far_distance: far.trunc(),
            },
        })
    }
}

/// 0044d4b0 requires height + 2 to be strictly positive; NaN is excluded.
fn visible_height(height: f32) -> bool {
    f64::from(height) + 2. > 0.
}

/// One session's original vehicle textures and both catalogue LODs.
pub struct Resources {
    surfaces: Vec<OwnedDraw>,
    models: Arc<Vec<Option<Model>>>,
}

impl Resources {
    pub fn new(
        session: &openmt2001::session::Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
    ) -> Result<Self, String> {
        use openmt2001::{render::SurfaceMode, scene_assets};
        let textures = scene_assets::vehicle_textures(image, index, archive, 1280)?;
        let (pages, regions) = scene_assets::scenery_pages(&textures)?;
        let surfaces = pages
            .into_iter()
            .map(|page| {
                OwnedDraw::texture_page(
                    if openmt2001::texture::uploaded_page_flags(page.flags) & 0x8000 != 0 {
                        SurfaceMode::StaticOpaque
                    } else {
                        SurfaceMode::StaticLit
                    },
                    page.side,
                    page.side,
                    page.rgba.into(),
                )
            })
            .collect();
        let mut models = Vec::with_capacity(87 * 2);
        for slot in 0..87 {
            for lod in [false, true] {
                let Some(source) = session.vehicle_models.source(slot, lod) else {
                    models.push(None);
                    continue;
                };
                models.push(Some(Model::from_mesh(source, &textures, &regions, None)?));
            }
        }
        Ok(Self {
            surfaces,
            models: Arc::new(models),
        })
    }
}

pub struct Layer {
    records: Box<[[u8; 208]; 200]>,
    auxiliaries: Vec<[u8; 284]>,
    definitions: Vec<Option<[u8; 116]>>,
    models: Arc<Vec<Option<Model>>>,
    suncolors: openmt2001::lighting::SunColors,
    surfaces: Vec<OwnedDraw>,
    range: Range<usize>,
    /// Crossing green/amber masks from the same road tick, for the junction
    /// traffic-light glows (0x44dbe0).
    signal_lanes: Vec<[u8; 32]>,
}

impl Layer {
    #[cfg(test)]
    pub fn active_count(&self) -> usize {
        self.auxiliaries
            .iter()
            .filter(|auxiliary| {
                let index = u32::from_le_bytes(auxiliary[0x38..0x3c].try_into().unwrap()) as usize;
                self.records[index][0x14] != 0
            })
            .count()
    }

    #[cfg(test)]
    pub fn first_active_position(&self) -> Option<[f32; 3]> {
        self.auxiliaries.iter().find_map(|auxiliary| {
            let index = u32::from_le_bytes(auxiliary[0x38..0x3c].try_into().unwrap()) as usize;
            let y = f32::from_le_bytes(auxiliary[8..12].try_into().unwrap());
            (self.records[index][0x14] != 0 && visible_height(y) && auxiliary[0xb8] & 1 != 0).then(
                || {
                    std::array::from_fn(|axis| {
                        f32::from_le_bytes(
                            auxiliary[4 + axis * 4..8 + axis * 4].try_into().unwrap(),
                        )
                    })
                },
            )
        })
    }

    pub fn from_resources(
        session: &openmt2001::session::Session<'_>,
        suncols: &[u8],
        resources: &Resources,
    ) -> Result<Self, String> {
        let suncolors = openmt2001::lighting::SunColors::parse(suncols)?;
        suncolors.sample(session.world.clock.hour(), 0)?;
        let definitions = (0..87)
            .map(|slot| session.vehicle_models.definition(slot).copied())
            .collect();
        Ok(Self {
            records: session.road_traffic.records.clone(),
            auxiliaries: session.road_traffic.auxiliaries.slots()
                [..session.road_traffic.auxiliaries.used()]
                .to_vec(),
            definitions,
            models: Arc::clone(&resources.models),
            suncolors,
            surfaces: resources.surfaces.clone(),
            range: 0..0,
            signal_lanes: session.world.road_network.lanes.clone(),
        })
    }

    pub fn signal_lanes(&self) -> &[[u8; 32]] {
        &self.signal_lanes
    }

    pub fn replace_attachment(&mut self, previous: &Self) -> Result<(), String> {
        if self.surfaces.len() != previous.range.len() {
            return Err("road material registry changed within a session".into());
        }
        self.range = previous.range.clone();
        self.surfaces.clear();
        Ok(())
    }

    pub fn attach(&mut self, scene: &mut openmt2001::render_scene::Scene) {
        // Empty material pages still need an uploadable initial vertex buffer.
        // The first reframe below replaces these placeholders before drawing.
        for surface in &mut self.surfaces {
            surface.vertices.resize(3, [0.; 8]);
            surface.specular.as_mut().unwrap().resize(3, [0.; 3]);
        }
        self.range = scene.draws.len()..scene.draws.len() + self.surfaces.len();
        scene.draws.append(&mut self.surfaces);
    }

    pub fn reframe(
        &self,
        scene: &mut openmt2001::render_scene::Scene,
        sample: openmt2001::flyby::CameraSample,
        detail: u32,
    ) -> Result<Range<usize>, String> {
        let time = scene
            .daylight_time()
            .ok_or("moving layer needs original scene daylight")?;
        let light = self.suncolors.sample(time.0, time.1)?;
        let palette = openmt2001::lighting::Palette::new(
            light.diffuse,
            light.ambient,
            light.diffuse,
            0,
            light.scale,
        );
        let draws = scene
            .draws
            .get_mut(self.range.clone())
            .ok_or("road layer is not attached")?;
        for draw in draws.iter_mut() {
            draw.vertices.clear();
            draw.specular.as_mut().unwrap().clear();
        }
        let policy = Policy::from_detail(detail)?.distances;
        let view = camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        let [near, _] = camera::city_clip(sample.eye[1])?;
        if near >= policy.far_distance {
            return Ok(self.range.clone());
        }
        let projection =
            QueryProjection::new(1280., 960., sample.vertical_fov, near, policy.far_distance)?;
        let color = |value: u32| [16, 8, 0].map(|shift| ((value >> shift) & 255) as f32 / 255.);
        for auxiliary in &self.auxiliaries {
            let vehicle = u32::from_le_bytes(auxiliary[0x38..0x3c].try_into().unwrap()) as usize;
            if self
                .records
                .get(vehicle)
                .ok_or("road auxiliary vehicle outside pool")?[0x14]
                == 0
            {
                continue;
            }
            let y = f32::from_le_bytes(auxiliary[8..12].try_into().unwrap());
            if !visible_height(y) {
                continue;
            }
            let handle = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
            let offset = handle
                .checked_sub(0x689080)
                .ok_or("road model below catalogue")?;
            if offset % 16 != 0 || offset / 16 >= 87 {
                return Err("road model outside catalogue".into());
            }
            let slot = (offset / 16) as usize;
            let definition = self.definitions[slot]
                .as_ref()
                .ok_or("active road model absent")?;
            let Some(lod) = policy.initial_pose(auxiliary, definition, view, &projection)? else {
                continue;
            };
            let model = self.models[slot * 2 + lod]
                .as_ref()
                .ok_or("visible road model LOD absent")?;
            let matrix = crate::people_scene::object_matrix(auxiliary)?;
            let transform = crate::people_scene::transform(auxiliary)?;
            let light = crate::people_scene::sunlight(matrix, light.direction);
            for part in &model.parts {
                let draw = &mut draws[part.page];
                for (vertex, &normal) in part.vertices.iter().zip(&part.normals) {
                    let mut vertex = *vertex;
                    let position = transform.position([vertex[0], vertex[1], vertex[2]]);
                    vertex[..3].copy_from_slice(&position);
                    let shades = palette.shade_static(normal, light)?;
                    vertex[3..6].copy_from_slice(&color(shades[0]));
                    draw.vertices.push(vertex);
                    draw.specular.as_mut().unwrap().push(color(shades[1]));
                }
            }
        }
        Ok(self.range.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn road_vehicle_drawing_distances() {
        for (detail, expected) in [(0, [200., 500.]), (50, [300., 850.]), (100, [500., 1550.])] {
            let distances = Policy::from_detail(detail).unwrap().distances;
            assert_eq!([distances.lod_distance, distances.far_distance], expected);
        }
    }

    #[test]
    fn road_vehicle_distances_store_floats_before_conversion() {
        // At 1%, truncating the extended expressions before their f32 stores
        // would produce 201 and 506 rather than 202 and 507.
        let distances = Policy::from_detail(1).unwrap().distances;
        assert_eq!(
            [distances.lod_distance, distances.far_distance],
            [202., 507.]
        );
    }

    #[test]
    fn hidden_height_boundary_is_strict() {
        assert!(!visible_height(f32::from_bits((-2_f32).to_bits() + 1)));
        assert!(!visible_height(-2.));
        assert!(visible_height(f32::from_bits((-2_f32).to_bits() - 1)));
        assert!(!visible_height(f32::NAN));
    }
}
