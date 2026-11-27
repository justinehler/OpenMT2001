//! Train, aircraft and scene-effect submission from the retained 44f0b0 frame.
use crate::{object_scene::Model, people_scene::DrawingDistances};
use openmt2001::{camera, picking::QueryProjection, render_scene::OwnedDraw};
use std::{collections::BTreeMap, ops::Range, sync::Arc};

struct Policy {
    distances: DrawingDistances,
}

impl Policy {
    /// Constant setup 004524b0 immediately precedes transport pass 0044f0b0.
    const fn new() -> Self {
        Self {
            distances: DrawingDistances {
                lod_distance: 999_999.,
                far_distance: 999_999.,
            },
        }
    }

    fn projection(
        &self,
        sample: openmt2001::flyby::CameraSample,
    ) -> Result<Option<QueryProjection>, String> {
        let [near, _] = camera::city_clip(sample.eye[1])?;
        if near >= self.distances.far_distance {
            return Ok(None);
        }
        QueryProjection::new(
            1280.,
            960.,
            sample.vertical_fov,
            near,
            self.distances.far_distance,
        )
        .map(Some)
    }

    fn pose(
        &self,
        auxiliary: &[u8; 284],
        view: [f32; 12],
        projection: &QueryProjection,
    ) -> Result<Option<[[f32; 3]; 4]>, String> {
        if auxiliary[0xb8] & 8 != 0 {
            return Err("transport callback lacks producer matrix".into());
        }
        let definition: [u8; 116] = auxiliary[0x44..0xb8].try_into().unwrap();
        let matrix = std::array::from_fn(|row| {
            std::array::from_fn(|axis| {
                let at = 0xbc + row * 12 + axis * 4;
                f32::from_le_bytes(auxiliary[at..at + 4].try_into().unwrap())
            })
        });
        Ok(self
            .distances
            .pose_with_matrix(auxiliary, &definition, view, projection, matrix)?
            .map(|_| matrix))
    }
}

pub struct Resources {
    surfaces: Vec<OwnedDraw>,
    models: Arc<BTreeMap<u32, Model>>,
}

fn submitted_model(reference: u32) -> bool {
    matches!(
        reference,
        0x6887e0
            | 0x6887f0
            | 0x688800
            | 0x688880
            | 0x688890
            | 0x6888a0
            | 0x688db0
            | 0x688dc0
            | 0x688dd0
            | 0x688de0
            | 0x689970
            | 0x688b98
            | 0x6896e8
            | 0x687830
    )
}

impl Resources {
    pub fn new(
        catalogue: &openmt2001::traffic::models::Models<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
    ) -> Result<Self, String> {
        use openmt2001::{render::SurfaceMode, scene_assets};
        let textures = scene_assets::transport_textures(image, index, archive, 1280)?;
        let (pages, regions) = scene_assets::scenery_pages(&textures)?;
        let mut surfaces: Vec<OwnedDraw> = pages
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
        let untextured_page = surfaces.len();
        surfaces.push(OwnedDraw::texture_page(
            SurfaceMode::StaticLit,
            4,
            4,
            vec![255; 4 * 4 * 4].into(),
        ));
        let mut models = BTreeMap::new();
        for (reference, mesh) in catalogue.registered_meshes() {
            // 44f0b0 submits planes, trains, four static boats and the four
            // hourly ships selected from original539d50. The other
            // registrations retain route/ship/aircraft locators, the separate
            // helicopter auxiliary and crane base. This callback never submits
            // those identities to4e0d00 as draw models.
            if !submitted_model(reference) {
                continue;
            }
            models.insert(
                reference,
                Model::from_mesh(mesh, &textures, &regions, Some(untextured_page))
                    .map_err(|error| format!("transport model {reference:08x}: {error}"))?,
            );
        }
        Ok(Self {
            surfaces,
            models: Arc::new(models),
        })
    }
}

pub struct Layer {
    auxiliaries: Vec<[u8; 284]>,
    models: Arc<BTreeMap<u32, Model>>,
    suncolors: openmt2001::lighting::SunColors,
    surfaces: Vec<OwnedDraw>,
    range: Range<usize>,
}

impl Layer {
    pub fn from_resources(
        session: &crate::fresh_city::Session<'_>,
        suncols: &[u8],
        resources: &Resources,
        auxiliaries: Vec<[u8; 284]>,
    ) -> Result<Self, String> {
        let suncolors = openmt2001::lighting::SunColors::parse(suncols)?;
        suncolors.sample(session.world.clock.hour(), 0)?;
        Ok(Self {
            auxiliaries,
            models: Arc::clone(&resources.models),
            suncolors,
            surfaces: resources.surfaces.clone(),
            range: 0..0,
        })
    }

    pub fn replace_attachment(&mut self, previous: &Self) -> Result<(), String> {
        if self.surfaces.len() != previous.range.len() {
            return Err("transport material registry changed within a session".into());
        }
        self.range = previous.range.clone();
        self.surfaces.clear();
        Ok(())
    }

    pub fn attach(&mut self, scene: &mut openmt2001::render_scene::Scene) {
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
        _detail: u32,
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
            .ok_or("transport layer is not attached")?;
        for draw in draws.iter_mut() {
            draw.vertices.clear();
            draw.specular.as_mut().unwrap().clear();
        }
        let policy = Policy::new();
        let view = camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        let Some(projection) = policy.projection(sample)? else {
            return Ok(self.range.clone());
        };
        let color = |value: u32| [16, 8, 0].map(|shift| ((value >> shift) & 255) as f32 / 255.);
        for auxiliary in &self.auxiliaries {
            let reference = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
            let model = self
                .models
                .get(&reference)
                .ok_or_else(|| format!("transport model {reference:08x} absent from group 10"))?;
            let Some(matrix) = policy.pose(auxiliary, view, &projection)? else {
                continue;
            };
            let transform = crate::people_scene::transform_from_matrix(matrix);
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

    #[cfg(test)]
    pub fn submitted_references(&self) -> impl Iterator<Item = u32> + '_ {
        self.auxiliaries
            .iter()
            .map(|auxiliary| u32::from_le_bytes(auxiliary[..4].try_into().unwrap()))
    }

    #[cfg(test)]
    pub fn first_position(&self) -> Option<[f32; 3]> {
        self.auxiliaries.first().map(|auxiliary| {
            std::array::from_fn(|axis| {
                let at = 0xbc + 36 + axis * 4;
                f32::from_le_bytes(auxiliary[at..at + 4].try_into().unwrap())
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(reference: u32, position: [f32; 3]) -> [u8; 284] {
        let mut auxiliary = [0_u8; 284];
        auxiliary[..4].copy_from_slice(&reference.to_le_bytes());
        auxiliary[0xb8] = 1;
        // Unit cube definition with its center at the local origin.
        for corner in 0..8 {
            for axis in 0..3 {
                let value = if corner & (1 << axis) == 0 {
                    -1_f32
                } else {
                    1.
                };
                let at = 0x44 + corner * 12 + axis * 4;
                auxiliary[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        auxiliary[0x44 + 112..0x44 + 116].copy_from_slice(&2_f32.to_le_bytes());
        for axis in 0..3 {
            let at = 0xbc + axis * 12 + axis * 4;
            auxiliary[at..at + 4].copy_from_slice(&1_f32.to_le_bytes());
            let at = 0xbc + 36 + axis * 4;
            auxiliary[at..at + 4].copy_from_slice(&position[axis].to_le_bytes());
        }
        auxiliary
    }

    fn sample() -> openmt2001::flyby::CameraSample {
        openmt2001::flyby::CameraSample {
            eye: [0., 6000., 0.],
            target: [0., 6000., 1.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        }
    }

    #[test]
    fn transport_beyond_resident_distance_uses_constant_setup() {
        let sample = sample();
        let policy = Policy::new();
        assert_eq!(
            [policy.distances.lod_distance, policy.distances.far_distance],
            [999_999.; 2]
        );
        let view = camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        let projection = policy.projection(sample).unwrap().unwrap();
        for detail in [0, 100] {
            let resident = crate::people_scene::Policy::from_detail(detail)
                .unwrap()
                .distances;
            // This camera's near plane also exceeds the resident far distance.
            assert!(camera::city_clip(sample.eye[1]).unwrap()[0] >= resident.far_distance);
            for reference in [0x6887e0, 0x688880] {
                // Aircraft and train.
                assert!(submitted_model(reference));
                let auxiliary = descriptor(reference, [0., 6000., 1000.]);
                assert!(
                    resident
                        .pose_with_matrix(
                            &auxiliary,
                            &auxiliary[0x44..0xb8].try_into().unwrap(),
                            view,
                            &projection,
                            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [0., 6000., 1000.]],
                        )
                        .unwrap()
                        .is_none()
                );
                let matrix = policy.pose(&auxiliary, view, &projection).unwrap().unwrap();
                assert_eq!(matrix[3], [0., 6000., 1000.]);
            }
        }
    }

    #[test]
    fn transport_retains_visibility_enable_and_matrix_gates() {
        let sample = sample();
        let policy = Policy::new();
        let view = camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        let projection = policy.projection(sample).unwrap().unwrap();
        for position in [
            [0., 6000., -1000.],
            [0., 6000., 100.],
            [10000., 6000., 1000.],
            [0., 6000., 999_999.],
        ] {
            assert!(
                policy
                    .pose(&descriptor(0x6887e0, position), view, &projection)
                    .unwrap()
                    .is_none()
            );
        }
        let mut auxiliary = descriptor(0x6887e0, [0., 6000., 1000.]);
        auxiliary[0xb8] = 0;
        assert!(
            policy
                .pose(&auxiliary, view, &projection)
                .unwrap()
                .is_none()
        );
        auxiliary[0xb8] = 9;
        assert!(
            policy
                .pose(&auxiliary, view, &projection)
                .unwrap_err()
                .contains("producer matrix")
        );
        auxiliary[0xb8] = 1;
        auxiliary[0xbc..0xc0].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(policy.pose(&auxiliary, view, &projection).is_err());
    }

    #[test]
    #[ignore = "requires original v1.4 executable and archive"]
    fn loads_original_group10_transport_materials() {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let catalogue =
            openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap();
        let animated: Vec<_> = catalogue
            .registered_meshes()
            .filter(|(reference, _)| submitted_model(*reference))
            .filter(|(_, mesh)| mesh.frames != 1)
            .map(|(reference, mesh)| (reference, mesh.frames))
            .collect();
        assert!(
            animated.is_empty(),
            "transport frame selection needs animated meshes: {animated:?}"
        );
        let textures =
            openmt2001::scene_assets::transport_textures(&image, &index, &archive, 1280).unwrap();
        let mut source_groups = std::collections::BTreeSet::new();
        for (_, mesh) in catalogue
            .registered_meshes()
            .filter(|(reference, _)| submitted_model(*reference))
        {
            for name in &mesh.texture_names {
                let key = name.rsplit('\\').next().unwrap();
                let record = textures
                    .iter()
                    .find(|texture| {
                        texture.name.as_ref().is_some_and(|name| {
                            name.as_bytes()
                                .get(..key.len())
                                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key.as_bytes()))
                        })
                    })
                    .unwrap();
                source_groups.insert(record.group);
            }
        }
        assert!(source_groups.is_subset(&[0, 4, 6, 9, 10].into()));
        let resources = Resources::new(&catalogue, &image, &index, &archive).unwrap();
        assert_eq!(resources.models.len(), 14);
        assert!(!resources.surfaces.is_empty());
        // Derive ship coverage from the actual scheduler table, independently
        // of the renderer admission list. Its fourth entry is the crash model.
        let ships = std::array::from_fn::<_, 4, _>(|i| {
            u32::from_le_bytes(
                image
                    .at(0x539d50 + i as u32 * 4, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )
        });
        assert_eq!(ships[3], 0x687830);
        for reference in [0x6887e0, 0x688880, 0x688db0].into_iter().chain(ships) {
            assert!(resources.models.get(&reference).is_some());
            assert!(
                resources.models[&reference]
                    .parts
                    .iter()
                    .any(|part| !part.vertices.is_empty())
            );
        }
    }
}
