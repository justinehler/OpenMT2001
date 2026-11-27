//! Original resident submission eligibility, retained animation frames and poses.
use openmt2001::{camera, picking::QueryProjection};

/// 004e0be4 -> 004e3380 shared object matrix. The mixed pitch/yaw/roll
/// branch preserves its asymmetric native arithmetic; the yaw-only path
/// retains the original resident float-store order.
pub(crate) fn object_matrix(auxiliary: &[u8; 284]) -> Result<[[f32; 3]; 4], String> {
    let f = |at: usize| f32::from_le_bytes(auxiliary[at..at + 4].try_into().unwrap());
    if !(1..10).all(|i| f(i * 4).is_finite()) {
        return Err("non-finite resident pose".into());
    }
    if f(0x10) != 0. || f(0x18) != 0. {
        // 004e3380 first forms the native yaw/roll rows, then mixes its
        // second and third rows by pitch. Its yaw/roll row-2 Y term has the
        // same negative sign as row-2 X; keep that native asymmetry.
        let angle = |at| {
            let radians = (f64::from(f(at)) * f64::from(0.017453292_f32)) as f32;
            let (sine, cosine) = f64::from(radians).sin_cos();
            (f64::from(sine as f32), f64::from(cosine as f32))
        };
        let (sp, cp) = angle(0x10);
        let (sy, cy) = angle(0x14);
        let (sr, cr) = angle(0x18);
        let first = [cy * cr, -cy * sr, sy];
        let second = [sr, cr, 0.];
        let third = [-sy * cr, -sy * sr, cy];
        let mixed_second = std::array::from_fn::<_, 3, _>(|i| cp * second[i] - sp * third[i]);
        let mixed_third = std::array::from_fn::<_, 3, _>(|i| sp * second[i] + cp * third[i]);
        let scale = [f(0x1c), f(0x20), f(0x24)];
        return Ok([
            first.map(|v| (v * f64::from(scale[0])) as f32),
            mixed_second.map(|v| (v * f64::from(scale[1])) as f32),
            mixed_third.map(|v| (v * f64::from(scale[2])) as f32),
            [f(4), f(8), f(12)],
        ]);
    }
    let radians = f64::from((f64::from(f(0x14)) * f64::from(0.017453292_f32)) as f32);
    let (sine, cosine) = radians.sin_cos();
    let sine = f64::from(sine as f32);
    Ok([
        [
            (cosine * f64::from(f(0x1c))) as f32,
            0.,
            (sine * f64::from(f(0x1c))) as f32,
        ],
        [0., f(0x20), 0.],
        [
            (-sine * f64::from(f(0x24))) as f32,
            0.,
            (cosine * f64::from(f(0x24))) as f32,
        ],
        [f(4), f(8), f(12)],
    ])
}

pub(crate) fn transform(
    auxiliary: &[u8; 284],
) -> Result<openmt2001::people::PeopleTransform, String> {
    Ok(transform_from_matrix(object_matrix(auxiliary)?))
}

pub(crate) fn transform_from_matrix(
    mut matrix: [[f32; 3]; 4],
) -> openmt2001::people::PeopleTransform {
    // Original 004f2350 tests the unflipped diagonal only.
    let translation_only = (0..3).all(|i| matrix[i][i].to_bits() == 1_f32.to_bits());
    matrix[2] = matrix[2].map(|v| -v);
    openmt2001::people::PeopleTransform {
        matrix,
        translation_only,
    }
}

pub(crate) fn object_view(view: [f32; 12], object: [[f32; 3]; 4]) -> [f32; 12] {
    let mut result = camera::translated_scene_view(view, object[3]);
    // Original 004e2fc0, including the first product pair's grouping.
    for row in 0..3 {
        for column in 0..3 {
            result[row * 3 + column] = ((f64::from(object[row][0]) * f64::from(view[column])
                + f64::from(object[row][1]) * f64::from(view[3 + column]))
                + f64::from(object[row][2]) * f64::from(view[6 + column]))
                as f32;
        }
    }
    result
}

/// 004e3770 sunlight in object coordinates, before the local-Z flip applied
/// by Palette::shade_static. Every component is stored before scaling by127.
pub(crate) fn sunlight(matrix: [[f32; 3]; 4], sun: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| {
        (f64::from(sun[2]) * f64::from(matrix[i][2])
            + (f64::from(sun[1]) * f64::from(matrix[i][1])
                + f64::from(sun[0]) * f64::from(matrix[i][0]))) as f32
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub(crate) distances: DrawingDistances,
}

/// Shared visibility and LOD calculation; each submission layer supplies its
/// own distance setup before entering 004e0d00/004e0b50.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DrawingDistances {
    pub lod_distance: f32,
    pub far_distance: f32,
}
impl Policy {
    /// 004c1920 default, mapped by00452470 and resident policy004524d0.
    pub fn from_detail(detail: u32) -> Result<Self, String> {
        let factor = Self::distance_factor(detail)?;
        // Each policy argument is stored as f32 before004e0b20 FTOL.
        let threshold =
            |scale: f64, offset: f64| ((f64::from(factor) * scale + offset) as f32).trunc();
        Ok(Self {
            distances: DrawingDistances {
                lod_distance: threshold(100., 50.),
                far_distance: threshold(300., 80.),
            },
        })
    }

    /// Shared 452470 percentage mapping, also read by the category-detail
    /// distance pass. Values above 50 use the original expanded second half.
    pub fn distance_factor(detail: u32) -> Result<f32, String> {
        if detail > 100 {
            return Err("unsupported resident detail percentage".into());
        }
        let initial = f64::from(detail) * f64::from(0.01_f32);
        let mut factor = initial as f32;
        if initial > 0.5 {
            factor = ((f64::from(factor) - 0.5) * 2. + 0.5) as f32;
        }
        Ok(factor)
    }
}

impl DrawingDistances {
    /// 004e0d00 enable gate and004e0b50 visibility/LOD. The caller supplies the
    /// normal model's original116-byte definition and current CPU view.
    pub fn initial_pose(
        &self,
        auxiliary: &[u8; 284],
        definition: &[u8; 116],
        view: [f32; 12],
        projection: &QueryProjection,
    ) -> Result<Option<usize>, String> {
        if auxiliary[0xb8] & 1 == 0 {
            return Ok(None);
        }
        self.pose_with_matrix(
            auxiliary,
            definition,
            view,
            projection,
            object_matrix(auxiliary)?,
        )
    }

    /// 4e0b50 also accepts a producer-supplied matrix when auxiliary flag
    /// bit3 is clear. Trains and aircraft enter through that branch.
    pub fn pose_with_matrix(
        &self,
        auxiliary: &[u8; 284],
        definition: &[u8; 116],
        view: [f32; 12],
        projection: &QueryProjection,
        matrix: [[f32; 3]; 4],
    ) -> Result<Option<usize>, String> {
        if auxiliary[0xb8] & 1 == 0 {
            return Ok(None);
        }
        let translation = matrix[3];
        if !matrix
            .iter()
            .flatten()
            .chain(view.iter())
            .all(|v| v.is_finite())
        {
            return Err("non-finite moving-object pose".into());
        }
        let [x, y, z] = translation.map(f64::from);
        // Early depth gate has its own addition order, distinct from4e39c0.
        let depth = (((z * f64::from(view[8]) + y * f64::from(view[5])) + f64::from(view[2]) * x)
            + f64::from(view[11])) as f32;
        if depth.is_sign_negative() || !(depth - self.far_distance).is_sign_negative() {
            return Ok(None);
        }
        let object_view = object_view(view, matrix);
        let d = |at: usize| f32::from_le_bytes(definition[at..at + 4].try_into().unwrap());
        let bounds = std::array::from_fn(|i| {
            camera::transform_scene_point(object_view, [d(i * 12), d(i * 12 + 4), d(i * 12 + 8)])
        });
        let center = camera::transform_scene_point(object_view, [d(96), d(100), d(104)]);
        Ok(projection
            .visible(&bounds, center, d(112))
            .then_some(usize::from(center[2] > self.lod_distance)))
    }
}

/// Retained resident identity and selected original model/frame for submission.
#[derive(Debug)]
pub struct DrawRequest {
    pub auxiliary_slot: usize,
    pub model_row: usize,
    pub lod: bool,
    pub frame: i16,
    pub transform: openmt2001::people::PeopleTransform,
}

/// Resolve the live auxiliary pool against its retained model registry. This
/// prepares requests only; textured GPU submission remains a separate stage.
pub fn initial_requests(
    session: &crate::fresh_city::Session<'_>,
    camera: openmt2001::flyby::CameraSample,
    detail: u32,
    size: [u32; 2],
) -> Result<Vec<DrawRequest>, String> {
    let policy = Policy::from_detail(detail)?.distances;
    let view = camera::scene_view_with_up(camera.eye, camera.target, camera.up);
    let [near, _] = camera::city_clip(camera.eye[1])?;
    if near >= policy.far_distance {
        return Ok(Vec::new());
    }
    let projection = QueryProjection::new(
        size[0] as f32,
        size[1] as f32,
        camera.vertical_fov,
        near,
        policy.far_distance,
    )?;
    let pool = &session.world.creation.auxiliary;
    let mut requests = Vec::new();
    for (slot, auxiliary) in pool.slots()[..pool.used()].iter().enumerate() {
        if auxiliary[0xb8] & 1 == 0 {
            continue;
        }
        let handle = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
        let row = session
            .resident_models
            .iter()
            .position(|model| model.reference == handle)
            .ok_or("resident auxiliary model is outside retained registry")?;
        let Some(lod) = policy.initial_pose(
            auxiliary,
            &session.resident_models[row].definition,
            view,
            &projection,
        )?
        else {
            continue;
        };
        let frame = i16::from_le_bytes(auxiliary[0x2c..0x2e].try_into().unwrap());
        let source = session
            .people
            .geometry_source(row, lod != 0)
            .ok_or("missing selected resident geometry")?;
        if usize::try_from(frame).map_or(true, |frame| frame >= source.frames) {
            return Err("resident frame leaves retained model".into());
        }
        requests.push(DrawRequest {
            auxiliary_slot: slot,
            model_row: row,
            lod: lod != 0,
            frame,
            transform: transform(auxiliary)?,
        });
    }
    Ok(requests)
}

/// Retained local triangle batch for one original model material.
struct Part {
    page: usize,
    vertices: Vec<[f32; 8]>,
    normals: Vec<[f32; 3]>,
}
struct Resident {
    auxiliary: [u8; 284],
    definition: [u8; 116],
    row: usize,
}
struct Model {
    parts: Vec<(openmt2001::people::PreparedPart, usize)>,
    uvs: Vec<[f32; 2]>,
}
/// Original immutable texture pages, material assignments and family normals.
/// Their lifetime is one retained session; actor poses and frames are never cached here.
pub struct Resources {
    surfaces: Vec<openmt2001::render_scene::OwnedDraw>,
    models: Vec<Model>,
}
impl Resources {
    pub fn new(
        session: &crate::fresh_city::Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
    ) -> Result<Self, String> {
        use openmt2001::{render::SurfaceMode, render_scene::OwnedDraw, scene_assets};
        let textures = scene_assets::people_textures(image, index, archive, 1280)?;
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
        let mut models = Vec::with_capacity(64);
        for row in 0..32 {
            for lod in [false, true] {
                let source = session
                    .people
                    .source(row, lod)
                    .ok_or("missing people material source")?;
                let material_regions = source
                    .texture_names
                    .iter()
                    .map(|name| {
                        let key = name.rsplit('\\').next().unwrap();
                        textures
                            .iter()
                            .position(|texture| {
                                texture.name.as_ref().is_some_and(|name| {
                                    name.as_bytes().get(..key.len()).is_some_and(|prefix| {
                                        prefix.eq_ignore_ascii_case(key.as_bytes())
                                    })
                                })
                            })
                            .map(|i| &regions[i])
                            .ok_or_else(|| format!("unresolved people texture {key}"))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let uvs = scene_assets::scenery_mesh_uvs(
                    source,
                    &material_regions.iter().map(|r| r.uv).collect::<Vec<_>>(),
                )?;
                let parts = session
                    .people
                    .prepare(row, lod)?
                    .parts
                    .into_iter()
                    .map(|part| {
                        let material = usize::from(u16::from_le_bytes(
                            part.material_header[4..6].try_into().unwrap(),
                        ));
                        let page = material_regions
                            .get(material)
                            .ok_or("people part material leaves registry")?
                            .page;
                        Ok((part, page))
                    })
                    .collect::<Result<_, String>>()?;
                models.push(Model { parts, uvs });
            }
        }
        Ok(Self { surfaces, models })
    }
}
/// Host-owned immutable initialized records/materials; Lua and simulation stay
/// on their worker. Camera movement only changes the submitted GPU batches.
pub struct Layer {
    residents: Vec<Resident>,
    models: std::collections::BTreeMap<(usize, bool, i16), Vec<Part>>,
    suncolors: openmt2001::lighting::SunColors,
    initial_time: (u32, u32),
    surfaces: Vec<openmt2001::render_scene::OwnedDraw>,
    range: std::ops::Range<usize>,
}
impl Layer {
    /// Keep the existing material slots while replacing all actor poses/frames.
    pub fn replace_attachment(&mut self, previous: &Self) -> Result<(), String> {
        if self.surfaces.len() != previous.range.len() {
            return Err("resident material registry changed within a session".into());
        }
        self.range = previous.range.clone();
        self.surfaces.clear();
        Ok(())
    }

    #[cfg(test)]
    pub fn differs_from(&self, previous: &Self) -> bool {
        self.residents.len() != previous.residents.len()
            || self
                .residents
                .iter()
                .zip(&previous.residents)
                .any(|(a, b)| a.auxiliary != b.auxiliary)
    }

    #[cfg(test)]
    pub fn first_position(&self) -> [f32; 3] {
        let auxiliary = &self.residents[0].auxiliary;
        std::array::from_fn(|i| {
            f32::from_le_bytes(auxiliary[4 + i * 4..8 + i * 4].try_into().unwrap())
        })
    }

    pub fn new(
        session: &crate::fresh_city::Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
        suncols: &[u8],
    ) -> Result<Self, String> {
        let resources = Resources::new(session, image, index, archive)?;
        Self::from_resources(session, suncols, &resources)
    }

    pub fn from_resources(
        session: &crate::fresh_city::Session<'_>,
        suncols: &[u8],
        resources: &Resources,
    ) -> Result<Self, String> {
        let suncolors = openmt2001::lighting::SunColors::parse(suncols)?;
        // Initial-draw snapshots keep their existing explicit phase-zero-counter
        // contract. Retained scene reframes sample the actual scene time below.
        let initial_time = (session.world.clock.hour(), 0);
        suncolors.sample(initial_time.0, initial_time.1)?;
        let surfaces = resources.surfaces.clone();
        let pool = &session.world.creation.auxiliary;
        let residents: Vec<Resident> = pool.slots()[..pool.used()]
            .iter()
            .map(|auxiliary| {
                if auxiliary[0x3c..0x40] != [255; 4] {
                    return Err("resident layer requires normal lighting color".into());
                }
                let handle = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
                let row = session
                    .resident_models
                    .iter()
                    .position(|model| model.reference == handle)
                    .ok_or("resident model outside retained registry")?;
                Ok(Resident {
                    auxiliary: *auxiliary,
                    definition: session.resident_models[row].definition,
                    row,
                })
            })
            .collect::<Result<_, String>>()?;
        let mut models = std::collections::BTreeMap::new();
        for row in 0..32 {
            let frames: std::collections::BTreeSet<i16> = residents
                .iter()
                .filter(|resident| resident.row == row)
                .map(|resident| {
                    i16::from_le_bytes(resident.auxiliary[0x2c..0x2e].try_into().unwrap())
                })
                .collect();
            for lod in [false, true] {
                let model = &resources.models[row * 2 + usize::from(lod)];
                let source = session
                    .people
                    .geometry_source(row, lod)
                    .ok_or("missing people geometry")?;
                for &frame in &frames {
                    let positions = source.positions(
                        usize::try_from(frame).map_err(|_| "negative resident animation frame")?,
                    ).map_err(|error| {
                        let matching = residents.iter().enumerate().filter(|(_, resident)|
                            resident.row == row && i16::from_le_bytes(
                                resident.auxiliary[0x2c..0x2e].try_into().unwrap()) == frame);
                        let count = matching.clone().count();
                        let slots = matching.take(8).map(|(slot, resident)| {
                            let bytes = &resident.auxiliary;
                            (slot, u32::from_le_bytes(bytes[..4].try_into().unwrap()),
                             u16::from_le_bytes(bytes[0x2e..0x30].try_into().unwrap()),
                             u32::from_le_bytes(bytes[0xb8..0xbc].try_into().unwrap()))
                        }).collect::<Vec<_>>();
                        format!("resident model row {row}, lod {lod}, variant {:?}, geometry {:?}, ticks {:?}, matching slots {count}, first8(slot,model,controller,flags) {slots:?}: {error}",
                            session.people.path(row, lod), session.people.path(row & !3, lod),
                            session.world.clock.timing)
                    })?;
                    let mut parts = Vec::new();
                    for (part, page) in &model.parts {
                        let mut output = Part {
                            page: *page,
                            vertices: Vec::new(),
                            normals: Vec::new(),
                        };
                        for triangle in &part.triangles {
                            for &vertex in triangle {
                                let vertex = vertex as usize;
                                let p = positions[part.source_vertices[vertex]];
                                let uv = model.uvs[part.source_vertices[vertex]];
                                let rgb = [1.; 3];
                                output
                                    .vertices
                                    .push([p[0], p[1], p[2], rgb[0], rgb[1], rgb[2], uv[0], uv[1]]);
                                output.normals.push(part.normals[vertex]);
                            }
                        }
                        parts.push(output);
                    }
                    models.insert((row, lod, frame), parts);
                }
            }
        }
        Ok(Self {
            residents,
            models,
            suncolors,
            initial_time,
            surfaces,
            range: 0..0,
        })
    }

    fn fill(
        &self,
        draws: &mut [openmt2001::render_scene::OwnedDraw],
        requests: &[DrawRequest],
        time: (u32, u32),
    ) -> Result<(), String> {
        let light = self.suncolors.sample(time.0, time.1)?;
        let palette = openmt2001::lighting::Palette::new(
            light.diffuse,
            light.ambient,
            light.diffuse,
            0,
            light.scale,
        );
        let color = |value: u32| [16, 8, 0].map(|shift| ((value >> shift) & 255) as f32 / 255.);
        for draw in draws.iter_mut() {
            draw.vertices.clear();
            draw.specular.as_mut().unwrap().clear();
        }
        for request in requests {
            let resident = self
                .residents
                .get(request.auxiliary_slot)
                .ok_or("resident request leaves snapshot")?;
            let light = sunlight(object_matrix(&resident.auxiliary)?, light.direction);
            for part in self
                .models
                .get(&(request.model_row, request.lod, request.frame))
                .ok_or("resident frame absent from snapshot geometry")?
            {
                let draw = &mut draws[part.page];
                for (vertex, &normal) in part.vertices.iter().zip(&part.normals) {
                    let mut vertex = *vertex;
                    let position = request
                        .transform
                        .position([vertex[0], vertex[1], vertex[2]]);
                    vertex[..3].copy_from_slice(&position);
                    let shades = palette.shade_static(normal, light)?;
                    vertex[3..6].copy_from_slice(&color(shades[0]));
                    draw.vertices.push(vertex);
                    draw.specular.as_mut().unwrap().push(color(shades[1]));
                }
            }
        }
        Ok(())
    }

    /// Reserve maximum geometry before the first GPU upload. Reframe must run
    /// immediately after upload and before drawing; placeholder triangles never
    /// represent the scene's visible resident set.
    pub fn attach(&mut self, scene: &mut openmt2001::render_scene::Scene) {
        let mut capacity = vec![0; self.surfaces.len()];
        for resident in &self.residents {
            let frame = i16::from_le_bytes(resident.auxiliary[0x2c..0x2e].try_into().unwrap());
            for lod in [false, true] {
                for part in &self.models[&(resident.row, lod, frame)] {
                    capacity[part.page] += part.vertices.len();
                }
            }
        }
        for (draw, count) in self.surfaces.iter_mut().zip(capacity) {
            draw.vertices.resize(count, [0.; 8]);
            draw.specular.as_mut().unwrap().resize(count, [0.; 3]);
        }
        self.range = scene.draws.len()..scene.draws.len() + self.surfaces.len();
        scene.draws.append(&mut self.surfaces);
    }

    pub fn reframe(
        &self,
        scene: &mut openmt2001::render_scene::Scene,
        sample: openmt2001::flyby::CameraSample,
        detail: u32,
    ) -> Result<std::ops::Range<usize>, String> {
        let policy = Policy::from_detail(detail)?.distances;
        let time = scene
            .daylight_time()
            .ok_or("resident layer needs original scene daylight")?;
        let view = camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        let [near, _] = camera::city_clip(sample.eye[1])?;
        if near >= policy.far_distance {
            self.fill(&mut scene.draws[self.range.clone()], &[], time)?;
            return Ok(self.range.clone());
        }
        let projection =
            QueryProjection::new(1280., 960., sample.vertical_fov, near, policy.far_distance)?;
        let mut requests = Vec::new();
        for (slot, resident) in self.residents.iter().enumerate() {
            let Some(lod) = policy.initial_pose(
                &resident.auxiliary,
                &resident.definition,
                view,
                &projection,
            )?
            else {
                continue;
            };
            requests.push(DrawRequest {
                auxiliary_slot: slot,
                model_row: resident.row,
                lod: lod != 0,
                frame: i16::from_le_bytes(resident.auxiliary[0x2c..0x2e].try_into().unwrap()),
                transform: transform(&resident.auxiliary)?,
            });
        }
        let draws = scene
            .draws
            .get_mut(self.range.clone())
            .ok_or("resident layer is not attached")?;
        self.fill(draws, &requests, time)?;
        Ok(self.range.clone())
    }
}

/// Textured initial pose using retained original pages, geometry and lighting.
pub fn initial_draws(
    session: &crate::fresh_city::Session<'_>,
    image: &openmt2001::pe::PeImage<'_>,
    index: &openmt2001::archive::ArchiveIndex,
    archive: &[u8],
    suncols: &[u8],
    requests: &[DrawRequest],
) -> Result<Vec<openmt2001::render_scene::OwnedDraw>, String> {
    let mut layer = Layer::new(session, image, index, archive, suncols)?;
    let mut draws = std::mem::take(&mut layer.surfaces);
    layer.fill(&mut draws, requests, layer.initial_time)?;
    draws.retain(|draw| !draw.vertices.is_empty());
    Ok(draws)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resident_drawing_distances_are_unchanged() {
        for (detail, expected) in [(0, [50., 80.]), (50, [100., 230.]), (100, [200., 530.])] {
            let distances = Policy::from_detail(detail).unwrap().distances;
            assert_eq!([distances.lod_distance, distances.far_distance], expected);
        }
    }

    #[test]
    fn matches_v14_mixed_object_matrix() {
        // Three mixed cases from six isolated 004e0be4 observations,
        // pinned v1.4/FPCW 0x037f.
        // This catches the original non-orthonormal mixed-angle arithmetic.
        let cases: [([f32; 3], [f32; 3], [f32; 9]); 3] = [
            (
                [30., 60., 10.],
                [1., 1., 1.],
                [
                    0.49240384,
                    -0.08682408,
                    0.86602545,
                    0.576818,
                    0.92806035,
                    -0.25,
                    -0.65178174,
                    0.36216775,
                    0.43301266,
                ],
            ),
            (
                [-17.5, 123.25, 42.75],
                [1., 1., 1.],
                [
                    -0.40262404,
                    0.37218183,
                    0.8362862,
                    0.46271923,
                    0.52963364,
                    -0.16487494,
                    -0.78980047,
                    -0.762_213_2,
                    -0.52291656,
                ],
            ),
            (
                [30., 60., 10.],
                [1., 2., 3.],
                [
                    0.49240384,
                    -0.08682408,
                    0.86602545,
                    1.153636,
                    1.8561207,
                    -0.5,
                    -1.9553453,
                    1.0865033,
                    1.299038,
                ],
            ),
        ];
        for (angles, scales, expected) in cases {
            let mut auxiliary = [0_u8; 284];
            for (i, value) in [10_f32, 20., 30.].into_iter().enumerate() {
                auxiliary[4 + i * 4..8 + i * 4].copy_from_slice(&value.to_le_bytes());
            }
            for (i, value) in angles.into_iter().chain(scales).enumerate() {
                auxiliary[0x10 + i * 4..0x14 + i * 4].copy_from_slice(&value.to_le_bytes());
            }
            let actual = object_matrix(&auxiliary).unwrap();
            for (index, (got, want)) in actual[..3].iter().flatten().zip(expected).enumerate() {
                assert!(
                    (got - want).abs() <= 2e-6,
                    "matrix {index}: {got} != {want}"
                );
            }
            assert_eq!(actual[3], [10., 20., 30.]);
        }
    }
    #[test]
    #[ignore = "requires original resident pose fixture"]
    fn matches_native_moving_resident_pose() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_PEOPLE_POSE").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTPPOSE1");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 12 + count * 452);
        for (case, record) in bytes[12..].chunks_exact(452).enumerate() {
            let aux = record[..284].try_into().unwrap();
            let f = |at| f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
            let view = std::array::from_fn(|i| f(284 + 4 * i));
            let sun = std::array::from_fn(|i| f(332 + 4 * i));
            let matrix = object_matrix(aux).unwrap();
            let composed = object_view(view, matrix);
            let light = sunlight(matrix, sun);
            for (group, actual, start) in [
                (
                    "matrix",
                    matrix.into_iter().flatten().collect::<Vec<_>>(),
                    344,
                ),
                ("view", composed.to_vec(), 392),
                ("sun", light.to_vec(), 440),
            ] {
                for (cell, value) in actual.into_iter().enumerate() {
                    let expected = f(start + cell * 4);
                    // Host sin/cos can differ in its last bit from x87 FSIN;
                    // this comparison bounds the resulting actual matrix values.
                    assert!(
                        (value - expected).abs() <= 2e-6_f32.max(expected.abs() * 2e-7),
                        "case {case} {group}[{cell}]: {value} != {expected}"
                    );
                }
            }
        }
    }
    #[test]
    #[ignore = "requires original resident visibility fixture"]
    fn matches_native_resident_visibility() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PEOPLE_VISIBILITY").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTPEVIS1");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 12 + count * 308);
        let mut definition = [0_u8; 116];
        let mut values = Vec::new();
        for i in 0..8 {
            values.extend([
                if i & 1 != 0 { 1_f32 } else { -1. },
                if i & 2 != 0 { 2. } else { 0. },
                if i & 4 != 0 { 1. } else { -1. },
            ]);
        }
        values.extend([0., 1., 0., 3., 3_f32.sqrt()]);
        for (dst, value) in definition.chunks_exact_mut(4).zip(values) {
            dst.copy_from_slice(&value.to_le_bytes());
        }
        for (index, case) in bytes[12..].chunks_exact(308).enumerate() {
            let detail = u32::from_le_bytes(case[..4].try_into().unwrap());
            let policy = Policy::from_detail(detail).unwrap().distances;
            let f = |at| f32::from_le_bytes(case[at..at + 4].try_into().unwrap());
            assert_eq!([policy.lod_distance, policy.far_distance], [f(16), f(20)]);
            let mut auxiliary: [u8; 284] = case[24..].try_into().unwrap();
            let expected = (auxiliary[0xb8] & 2 != 0)
                .then(|| u32::from_le_bytes(auxiliary[0x34..0x38].try_into().unwrap()) as usize);
            auxiliary[0xb8] = 13;
            let projection =
                QueryProjection::new(640., 480., 45., 1., policy.far_distance).unwrap();
            let actual = policy
                .initial_pose(
                    &auxiliary,
                    &definition,
                    [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.],
                    &projection,
                )
                .unwrap();
            assert_eq!(actual, expected, "case {index}");
        }
    }
}
