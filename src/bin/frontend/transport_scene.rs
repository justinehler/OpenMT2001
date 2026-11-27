//! Train, aircraft and scene-effect submission from the retained 44f0b0 frame.
use crate::object_scene::Model;
use openmt2001::{camera, picking::QueryProjection, render_scene::OwnedDraw};
use std::{collections::BTreeMap, ops::Range, sync::Arc};

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
            .ok_or("transport layer is not attached")?;
        for draw in draws.iter_mut() {
            draw.vertices.clear();
            draw.specular.as_mut().unwrap().clear();
        }
        let policy = crate::people_scene::Policy::from_detail(detail)?;
        let view = camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        let [near, _] = camera::city_clip(sample.eye[1])?;
        if near >= policy.far_distance {
            return Ok(self.range.clone());
        }
        let projection =
            QueryProjection::new(1280., 960., sample.vertical_fov, near, policy.far_distance)?;
        let color = |value: u32| [16, 8, 0].map(|shift| ((value >> shift) & 255) as f32 / 255.);
        for auxiliary in &self.auxiliaries {
            if auxiliary[0xb8] & 8 != 0 {
                return Err("transport callback lacks producer matrix".into());
            }
            let reference = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
            let model = self
                .models
                .get(&reference)
                .ok_or_else(|| format!("transport model {reference:08x} absent from group 10"))?;
            let definition: [u8; 116] = auxiliary[0x44..0xb8].try_into().unwrap();
            let matrix = std::array::from_fn(|row| {
                std::array::from_fn(|axis| {
                    let at = 0xbc + row * 12 + axis * 4;
                    f32::from_le_bytes(auxiliary[at..at + 4].try_into().unwrap())
                })
            });
            if policy
                .pose_with_matrix(auxiliary, &definition, view, &projection, matrix)?
                .is_none()
            {
                continue;
            }
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
