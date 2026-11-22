//! Retained original ambient trunks and state10 foliage, independent of buildings.
use crate::{compatibility::Mode, object_scene, people_scene};
use openmt2001::{
    archive::ArchiveIndex,
    camera,
    flyby::CameraSample,
    lighting::{Palette, SunColors},
    mesh::CachedMesh,
    pe::PeImage,
    picking::QueryProjection,
    render::SurfaceMode,
    render_scene::{OwnedDraw, Scene},
    scene_assets,
    texture::TextureSet,
    trees,
};
use std::{collections::BTreeMap, ops::Range, sync::Arc};

struct Instance {
    matrix: [[f32; 3]; 4],
    positions: Vec<Vec<[f32; 3]>>,
    near: Vec<trees::Point>,
    far: Vec<trees::Point>,
}

pub struct Resources {
    assets: trees::Assets,
    pool: trees::Pool,
    definition: [u8; 116],
    model: object_scene::Model,
    instances: Vec<Instance>,
    surfaces: Vec<OwnedDraw>,
    trunk_pages: usize,
    sun: SunColors,
}

impl Resources {
    pub fn load(
        image: &PeImage<'_>,
        index: &ArchiveIndex,
        archive: &[u8],
        suncols: &[u8],
    ) -> Result<Self, String> {
        let payload = |path| -> Result<&[u8], String> {
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing tree resource {path}"))?;
            let end = entry
                .offset
                .checked_add(entry.length)
                .ok_or("tree resource range overflow")?;
            archive
                .get(
                    entry.offset as usize
                        ..usize::try_from(end).map_err(|_| "tree resource range overflow")?,
                )
                .ok_or_else(|| "tree resource outside archive".into())
        };
        let assets = trees::Assets::load(image, index, archive)?;
        let pool = trees::Pool::ambient(&assets.locators)?;
        let trunk = CachedMesh::parse(payload(trees::TRUNK_PATH)?)?;
        if trunk.flags & 8 != 0 {
            return Err("tree model uses unsupported early distance gate".into());
        }
        let definition = trunk.model_definition()?;
        let textures = scene_assets::scenery_textures(image, index, archive, 1280)?;
        // These authentic archive members are native cache-hit records. Their
        // pixels/flags already passed the original conversion before caching.
        let (pages, regions) = scene_assets::cached_scenery_pages(&textures)?;
        let mut model = object_scene::Model::from_mesh(&trunk, &textures, &regions, None)?;
        // Cached loader retains manager+50 normals; registration only generates
        // normals when that pointer is absent. Preserve the authentic cache.
        for (part, source) in model.parts.iter_mut().zip(&trunk.parts) {
            part.normals = source
                .vertex_indices()
                .map(|i| {
                    std::array::from_fn(|axis| {
                        f32::from_le_bytes(
                            trunk.normal_bytes[i * 12 + axis * 4..i * 12 + axis * 4 + 4]
                                .try_into()
                                .unwrap(),
                        )
                    })
                })
                .collect();
            if !part.normals.iter().flatten().all(|v| v.is_finite()) {
                return Err("non-finite cached tree normal".into());
            }
        }
        let mut used = BTreeMap::new();
        for part in &mut model.parts {
            let next = used.len();
            part.page = *used.entry(part.page).or_insert(next);
        }
        let trunk_pages = used.len();
        let mut surfaces: Vec<Option<OwnedDraw>> = (0..trunk_pages).map(|_| None).collect();
        for (original, remapped) in used {
            let page = &pages[original];
            surfaces[remapped] = Some(OwnedDraw::texture_page(
                if openmt2001::texture::uploaded_page_flags(page.flags) & 0x8000 != 0 {
                    SurfaceMode::StaticOpaque
                } else {
                    SurfaceMode::StaticLit
                },
                page.side,
                page.side,
                Arc::from(page.rgba.as_slice()),
            ));
        }
        let mut surfaces: Vec<_> = surfaces.into_iter().map(Option::unwrap).collect();
        let foliage = TextureSet::parse(payload("gamedata2/billboardtextures/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        for row in 5..8 {
            let texture = foliage
                .textures
                .get(row)
                .ok_or("tree foliage collection lacks rows5..7")?;
            if (texture.width, texture.height) != (32, 32) {
                return Err("original tree foliage dimensions changed".into());
            }
            let mut surface = OwnedDraw::texture_page(
                SurfaceMode::TreeFoliage,
                texture.width,
                texture.height,
                Arc::from(texture.texels),
            );
            surface.diffuse_alpha = Some(Vec::new());
            surfaces.push(surface);
        }
        let mut instances = Vec::with_capacity(pool.count());
        for (i, slot) in pool.submission_slots() {
            let variation = assets.tables.variation(i, slot)?;
            let mut auxiliary = [0; 284];
            for (at, value) in [
                (4, slot.position[0]),
                (8, slot.position[1]),
                (12, -slot.position[2]),
                (0x14, variation.yaw_degrees),
                (0x1c, variation.scale[0]),
                (0x20, variation.scale[1]),
                (0x24, variation.scale[2]),
            ] {
                auxiliary[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
            let matrix = people_scene::object_matrix(&auxiliary)?;
            let transform = people_scene::transform_from_matrix(matrix);
            let positions = model
                .parts
                .iter()
                .map(|part| {
                    part.vertices
                        .iter()
                        .map(|v| transform.position([v[0], v[1], v[2]]))
                        .collect()
                })
                .collect();
            let mut near = Vec::new();
            let mut far = Vec::new();
            // Placement is immutable for these ambient slots. Only selection,
            // camera projection and palette modulation change on later frames.
            assets
                .tables
                .append_points(i, slot, 100, &assets.attachments, &[0; 9], &mut near)?;
            assets
                .tables
                .append_points(i, slot, 300, &assets.attachments, &[0; 9], &mut far)?;
            instances.push(Instance {
                matrix,
                positions,
                near,
                far,
            });
        }
        Ok(Self {
            assets,
            pool,
            definition,
            model,
            instances,
            surfaces,
            trunk_pages,
            sun: SunColors::parse(suncols)?,
        })
    }
}

pub struct Layer {
    resources: Arc<Resources>,
    compatibility: Mode,
    range: Range<usize>,
    previous_visible: Vec<bool>,
    previous_time: Option<(u32, u32)>,
    pub submissions: Vec<i32>,
    pub foliage_counts: [usize; 3],
}

impl Layer {
    pub fn new(resources: Arc<Resources>, compatibility: Mode) -> Self {
        Self {
            resources,
            compatibility,
            range: 0..0,
            previous_visible: Vec::new(),
            previous_time: None,
            submissions: Vec::new(),
            foliage_counts: [0; 3],
        }
    }
    pub fn attach(&mut self, scene: &mut Scene) {
        let r = &self.resources;
        let mut surfaces = r.surfaces.clone();
        let mut capacity = vec![0; surfaces.len()];
        for instance in &r.instances {
            for (part, positions) in r.model.parts.iter().zip(&instance.positions) {
                capacity[part.page] += positions.len();
            }
        }
        for (i, instance) in r.instances.iter().enumerate() {
            capacity[r.trunk_pages + i % 3] += instance.near.len().max(instance.far.len()) * 6;
        }
        for (surface, count) in surfaces.iter_mut().zip(capacity) {
            surface.vertices.resize(count, [0.; 8]);
            surface.specular.as_mut().unwrap().resize(count, [0.; 3]);
            if let Some(alpha) = &mut surface.diffuse_alpha {
                alpha.resize(count, 0);
            }
        }
        self.range = scene.draws.len()..scene.draws.len() + surfaces.len();
        scene.draws.append(&mut surfaces);
    }

    pub fn reframe(
        &mut self,
        scene: &mut Scene,
        sample: CameraSample,
    ) -> Result<Vec<usize>, String> {
        let r = &self.resources;
        let size = scene
            .viewport_size()
            .ok_or("tree layer needs animated scene viewport")?;
        let time = scene
            .daylight_time()
            .ok_or("tree layer needs original scene daylight")?;
        let light = r.sun.sample(time.0, time.1)?;
        let palette = r.assets.tables.palette(
            sample.eye,
            sample.target,
            light.direction,
            light.diffuse,
            light.ambient,
        )?;
        let view = camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        let gpu_view = camera::view_with_up(sample.eye, sample.target, sample.up);
        let gpu_view = [
            gpu_view[0],
            gpu_view[1],
            gpu_view[2],
            gpu_view[4],
            gpu_view[5],
            gpu_view[6],
            gpu_view[8],
            gpu_view[9],
            gpu_view[10],
            gpu_view[12],
            gpu_view[13],
            gpu_view[14],
        ];
        let [near, far] = camera::city_clip(sample.eye[1])?;
        let projection = camera::projection(
            sample.vertical_fov,
            near,
            far,
            size[0] as i32,
            size[1] as i32,
            [0., 1., 0., 1.],
        )?;
        let query = QueryProjection::new(
            size[0] as f32,
            size[1] as f32,
            sample.vertical_fov,
            near,
            far,
        )?;
        self.submissions.clear();
        let d = |at| f32::from_le_bytes(r.definition[at..at + 4].try_into().unwrap());
        let mut visible = Vec::with_capacity(r.pool.count());
        for instance in &r.instances {
            let object_view = people_scene::object_view(view, instance.matrix);
            let bounds = std::array::from_fn(|i| {
                camera::transform_scene_point(
                    object_view,
                    [d(i * 12), d(i * 12 + 4), d(i * 12 + 8)],
                )
            });
            let center = camera::transform_scene_point(object_view, [d(96), d(100), d(104)]);
            let accepted = query.visible(&bounds, center, d(112));
            visible.push(accepted);
            self.submissions
                .push(trees::submission_depth(accepted, center[2])?);
        }
        let changed_trunks = self.previous_visible != visible || self.previous_time != Some(time);
        let draws = scene
            .draws
            .get_mut(self.range.clone())
            .ok_or("tree layer not attached")?;
        let rgb = |color: u32| [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.);
        let mut changed = Vec::with_capacity(draws.len());
        if changed_trunks {
            for draw in &mut draws[..r.trunk_pages] {
                draw.vertices.clear();
                draw.specular.as_mut().unwrap().clear();
            }
            let lighting =
                Palette::new(light.diffuse, light.ambient, light.diffuse, 0, light.scale);
            for (_, instance) in r.instances.iter().enumerate().filter(|(i, _)| visible[*i]) {
                let sun = people_scene::sunlight(instance.matrix, light.direction);
                for (part, positions) in r.model.parts.iter().zip(&instance.positions) {
                    let draw = &mut draws[part.page];
                    for ((vertex, normal), position) in
                        part.vertices.iter().zip(&part.normals).zip(positions)
                    {
                        let colors = trunk_shade(&lighting, *normal, sun, self.compatibility)?;
                        let diffuse = rgb(colors[0]);
                        draw.vertices.push([
                            position[0],
                            position[1],
                            position[2],
                            diffuse[0],
                            diffuse[1],
                            diffuse[2],
                            vertex[6],
                            vertex[7],
                        ]);
                        draw.specular.as_mut().unwrap().push(rgb(colors[1]));
                    }
                }
            }
            changed.extend(self.range.start..self.range.start + r.trunk_pages);
        }
        self.previous_visible = visible;
        self.previous_time = Some(time);
        self.foliage_counts = [0; 3];
        for phase in 0..3 {
            let draw = &mut draws[r.trunk_pages + phase];
            draw.vertices.clear();
            draw.specular.as_mut().unwrap().clear();
            let alpha = draw.diffuse_alpha.as_mut().unwrap();
            alpha.clear();
            draw.view_projection = Some(projection);
            for i in (phase..r.pool.count()).step_by(3) {
                let submission = self.submissions[i];
                if submission == 0 {
                    continue;
                }
                let instance = &r.instances[i];
                let points = if submission < 300 {
                    &instance.near
                } else {
                    &instance.far
                };
                for point in points {
                    let center = camera::transform_scene_point(gpu_view, point.position);
                    let color = palette[(i + point.attachment) & 7];
                    let diffuse = rgb(color);
                    for corner in [0, 1, 2, 0, 2, 3] {
                        let [x, y] = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]][corner];
                        let [u, v] = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]][corner];
                        draw.vertices.push([
                            (f64::from(center[0]) + f64::from(x) * f64::from(point.radius)) as f32,
                            (f64::from(center[1]) + f64::from(y) * f64::from(point.radius)) as f32,
                            center[2],
                            diffuse[0],
                            diffuse[1],
                            diffuse[2],
                            u,
                            v,
                        ]);
                        alpha.push((color >> 24) as u8);
                    }
                    self.foliage_counts[phase] += 1;
                }
            }
            draw.specular
                .as_mut()
                .unwrap()
                .resize(draw.vertices.len(), [0.; 3]);
            changed.push(self.range.start + r.trunk_pages + phase);
        }
        Ok(changed)
    }
}

// Native scales the local sun by the object basis before its unchecked
// 256-entry palette lookup. Modern mode bounds the resulting index; Strict
// reports unsupported out-of-table reads instead of inventing adjacent memory.
fn trunk_shade(
    palette: &Palette,
    normal: [f32; 3],
    sun: [f32; 3],
    mode: Mode,
) -> Result<[u32; 2], String> {
    if mode == Mode::StrictV14 {
        return palette.shade_static(normal, sun);
    }
    let vector = [sun[0], sun[1], -sun[2]].map(|v| (f64::from(v) * 127.) as f32);
    let products = std::array::from_fn::<_, 3, _>(|i| f64::from(normal[i]) * f64::from(vector[i]));
    let dot = products[2] + (products[1] + products[0]);
    if !dot.is_finite() {
        return Err("non-finite tree lighting normal".into());
    }
    let index = (dot.round_ties_even() + 127.).clamp(0., 255.) as usize;
    let color = palette.primary[index];
    let secondary = palette.secondary[((color as i32 >> 24) + 127) as usize];
    Ok([color | 0xff000000, secondary])
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::render_scene::{self, OriginalAssets, Request, Source};

    #[test]
    fn scaled_trunk_lighting_bounds_only_in_modern_mode() {
        let palette = Palette::new(0xff808080, 0xff202020, 0xff808080, 0, 1.);
        for strength in [-1.15, -1., 0., 1., 1.15] {
            let normal = [1., 0., 0.];
            let sun = [strength, 0., 0.];
            let strict = trunk_shade(&palette, normal, sun, Mode::StrictV14);
            let modern = trunk_shade(&palette, normal, sun, Mode::Modern).unwrap();
            if strength.abs() <= 1. {
                assert_eq!(strict.unwrap(), modern);
            } else {
                assert!(strict.is_err());
            }
        }
        assert!(trunk_shade(&palette, [f32::NAN, 0., 0.], [1., 0., 0.], Mode::Modern).is_err());
    }

    #[test]
    #[ignore = "requires player-supplied original files"]
    fn original_ambient_tree_scene_lod_daylight_and_retention() {
        let read = |key| std::fs::read(std::env::var_os(key).expect(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let suncols = read("OPENMT2001_SUNCOLS");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let save = read("OPENMT2001_SAVE_ONE");
        let image = PeImage::parse(&executable).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let resources = Arc::new(Resources::load(&image, &index, &archive, &suncols).unwrap());
        assert_eq!(resources.pool.count(), 137);
        assert_eq!(resources.assets.attachments.len(), 75);
        assert_eq!(resources.trunk_pages, 1);
        assert!(
            resources
                .instances
                .iter()
                .all(|i| i.near.len() == 44 && i.far.len() == 23)
        );
        let entry = index
            .find("gamedata2/billboardtextures/textures.gtd")
            .unwrap();
        let bytes = &archive[entry.offset as usize..(entry.offset + entry.length) as usize];
        let textures = TextureSet::parse(bytes).unwrap();
        for phase in 0..3 {
            let draw = &resources.surfaces[resources.trunk_pages + phase];
            assert_eq!(draw.mode, SurfaceMode::TreeFoliage);
            assert_eq!(&*draw.rgba, textures.textures[phase + 5].texels);
        }
        // Aim at actual transformed trunk geometry, without inventing a locator.
        let vertices = &resources.instances[0].positions[0];
        let target = std::array::from_fn(|axis| {
            let lo = vertices
                .iter()
                .map(|v| v[axis])
                .fold(f32::INFINITY, f32::min);
            let hi = vertices
                .iter()
                .map(|v| v[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            (lo + hi) * 0.5
        });
        let mut camera = CameraSample {
            eye: [target[0], target[1], target[2] - 100.],
            target,
            up: [0., 1., 0.],
            vertical_fov: 60.,
        };
        let mut scene = render_scene::assemble(Request {
            build_file: &build_file,
            source: Source::Save(&save),
            original: Some(OriginalAssets {
                executable: &executable,
                directory: &directory,
                archive: &archive,
            }),
            suncols: Some(&suncols),
            phase: Some(12),
            camera: Some(camera),
            width: 1280,
            height: 960,
            animated: true,
        })
        .unwrap();
        let mut layer = Layer::new(resources.clone(), Mode::Modern);
        layer.attach(&mut scene);
        let retained: Vec<_> = scene.draws[layer.range.clone()]
            .iter()
            .map(|d| d.rgba.clone())
            .collect();
        let changed = layer.reframe(&mut scene, camera).unwrap();
        assert_eq!(changed.len(), resources.trunk_pages + 3);
        assert!(
            layer.submissions[0] > 0 && layer.submissions[0] < 300,
            "near submission {}",
            layer.submissions[0]
        );
        let expected: usize = layer
            .submissions
            .iter()
            .map(|&d| {
                if d == 0 {
                    0
                } else if d < 300 {
                    44
                } else {
                    23
                }
            })
            .sum();
        assert_eq!(layer.foliage_counts.iter().sum::<usize>(), expected);
        for draw in &scene.draws[layer.range.start + resources.trunk_pages..layer.range.end] {
            assert!(draw.view_projection.is_some());
            assert_eq!(
                draw.vertices.len(),
                draw.diffuse_alpha.as_ref().unwrap().len()
            );
            for quad in draw.vertices.chunks_exact(6) {
                assert!(quad.iter().all(|v| v[2] == quad[0][2]));
                assert_eq!(
                    quad.iter().map(|v| [v[6], v[7]]).collect::<Vec<_>>(),
                    [[0., 1.], [1., 1.], [1., 0.], [0., 1.], [1., 0.], [0., 0.]]
                );
            }
        }
        let unchanged = layer.reframe(&mut scene, camera).unwrap();
        assert_eq!(
            unchanged,
            (layer.range.start + resources.trunk_pages..layer.range.end).collect::<Vec<_>>()
        );
        camera.eye[2] = target[2] - 400.;
        scene.reframe(camera).unwrap();
        layer.reframe(&mut scene, camera).unwrap();
        assert!(
            layer.submissions[0] >= 300,
            "far submission {}",
            layer.submissions[0]
        );
        scene.advance_daylight(15, 10, camera).unwrap();
        let daylight = layer.reframe(&mut scene, camera).unwrap();
        assert!(daylight.contains(&layer.range.start));
        for (draw, texture) in scene.draws[layer.range.clone()].iter().zip(&retained) {
            assert!(Arc::ptr_eq(&draw.rgba, texture));
            assert!(draw.vertices.iter().flatten().all(|x| x.is_finite()));
        }
        assert!(Arc::ptr_eq(&layer.resources, &resources));
        eprintln!(
            "137 ambient trees: raw foliage pixels, near/far selection, palette/daylight, quad layout and retained textures verified; no native visual acceptance"
        );
    }
}
