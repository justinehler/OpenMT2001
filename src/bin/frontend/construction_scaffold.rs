//! Original 4b0cb0 construction scaffold draw, separate from the growing facade.
use crate::object_scene::Model;
use openmt2001::{
    archive::ArchiveIndex,
    lighting::{Palette, SunColors},
    mesh::CachedMesh,
    pe::PeImage,
    render::SurfaceMode,
    render_scene::{OwnedDraw, Scene},
    scene_assets,
};
use std::{collections::BTreeMap, sync::Arc};

// 4bdc2e..4bdd05 registers these twelve references. Two paths deliberately
// occur twice in the native table, once for a different edge orientation.
const MODEL_PATHS: [u32; 12] = [
    0x547be8, 0x547ba4, 0x547b60, 0x547b28, 0x547af0, 0x547aac, 0x547be8, 0x547a68, 0x547a2c,
    0x5479f0, 0x547af0, 0x5479b4,
];
const MAX_FRAME: usize = 24;

pub struct Resources {
    surfaces: Vec<OwnedDraw>,
    models: Vec<Arc<Vec<Model>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Instance {
    model: usize,
    frame: usize,
    position: [f32; 3],
    yaw: f32,
}

fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}

fn submitted(record: &[u32; 7]) -> bool {
    matches!((record[2], record[3]), (1, 0..=1 | 3) | (2, 0..=2))
}

pub fn has_active(pending: &openmt2001::pending_world::State) -> bool {
    pending.records.iter().any(submitted)
}

impl Resources {
    pub fn new(image: &PeImage<'_>, index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        let textures = scene_assets::scenery_textures(image, index, archive, 1280)?;
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
        let mut distinct = BTreeMap::<u32, Arc<Vec<Model>>>::new();
        let mut models = Vec::with_capacity(MODEL_PATHS.len());
        for address in MODEL_PATHS {
            if let Some(existing) = distinct.get(&address) {
                models.push(Arc::clone(existing));
                continue;
            }
            let path = image.at(address, 260)?;
            let end = path
                .iter()
                .position(|&byte| byte == 0)
                .ok_or("unterminated scaffold model path")?;
            let path =
                std::str::from_utf8(&path[..end]).map_err(|_| "non-UTF8 scaffold model path")?;
            let entry = index.find(path).ok_or("original scaffold model absent")?;
            let data = archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("original scaffold model exceeds archive")?;
            let mesh = CachedMesh::parse(data)?;
            if mesh.frames <= MAX_FRAME {
                return Err(
                    "original scaffold animation shorter than native 25-frame range".into(),
                );
            }
            let frames = (0..=MAX_FRAME)
                .map(|frame| Model::from_mesh_frame(&mesh, &textures, &regions, None, frame))
                .collect::<Result<Vec<_>, _>>()?;
            let frames = Arc::new(frames);
            distinct.insert(address, Arc::clone(&frames));
            models.push(frames);
        }
        Ok(Self { surfaces, models })
    }

    /// Append a stable page layout on every live snapshot after first use.
    /// Cached city draws remain untouched while pending records change.
    pub fn append(
        &self,
        scene: &mut Scene,
        pending: &openmt2001::pending_world::State,
        buildings: &[[u8; 0x394]],
        suncols: &[u8],
        hour: u32,
    ) -> Result<std::ops::Range<usize>, String> {
        let light = SunColors::parse(suncols)?.sample(hour, 0)?;
        let palette = Palette::new(light.diffuse, light.ambient, light.diffuse, 0, light.scale);
        let mut draws = self.surfaces.clone();
        let color = |value: u32| [16, 8, 0].map(|shift| ((value >> shift) & 255) as f32 / 255.);
        for record in pending.records.iter().filter(|record| submitted(record)) {
            let building = buildings
                .get(record[1] as usize)
                .ok_or("scaffold pending building leaves runtime pool")?;
            for instance in instances(building, record[4])? {
                let model = &self.models[instance.model][instance.frame];
                let angle =
                    f64::from((f64::from(instance.yaw) * f64::from(0.017453292_f32)) as f32);
                let (sine, cosine) = angle.sin_cos();
                let matrix = [
                    [cosine as f32, 0., sine as f32],
                    [0., 1., 0.],
                    [-(sine as f32), 0., cosine as f32],
                    instance.position,
                ];
                let transform = crate::people_scene::transform_from_matrix(matrix);
                let sunlight = crate::people_scene::sunlight(matrix, light.direction);
                for part in &model.parts {
                    let draw = draws
                        .get_mut(part.page)
                        .ok_or("scaffold texture page absent")?;
                    for (vertex, &normal) in part.vertices.iter().zip(&part.normals) {
                        let mut vertex = *vertex;
                        let local = [vertex[0], vertex[1], vertex[2]];
                        vertex[..3].copy_from_slice(&transform.position(local));
                        let shades = palette.shade_static(normal, sunlight)?;
                        vertex[3..6].copy_from_slice(&color(shades[0]));
                        draw.vertices.push(vertex);
                        draw.specular.as_mut().unwrap().push(color(shades[1]));
                    }
                }
            }
        }
        // The retained renderer keeps one GPU buffer per material page. Empty
        // pages use a degenerate triangle, as other dynamic scene layers do.
        for draw in &mut draws {
            if draw.vertices.is_empty() {
                draw.vertices.resize(3, [0.; 8]);
                draw.specular.as_mut().unwrap().resize(3, [0.; 3]);
            }
        }
        scene.vertex_count += draws.iter().map(|draw| draw.vertices.len()).sum::<usize>();
        let start = scene.draws.len();
        scene.draws.extend(draws);
        Ok(start..scene.draws.len())
    }
}

/// 4b0270: each floor has a perimeter of scaffold pieces. 4b0cb0 admits
/// construction stages 0/1/3 and demolition stages 0/1/2, then passes pending
/// progress (not stage) as the frame source. The native frame is
/// min(24, progress + edge index - floor*25); a negative value suppresses
/// that piece until its floor begins growing.
fn instances(building: &[u8; 0x394], progress: u32) -> Result<Vec<Instance>, String> {
    let [x0, y0, x1, y1] = [building[4], building[5], building[6], building[7]];
    if x1 < x0 || y1 < y0 || x1 > 6 || y1 > 6 {
        return Err("scaffold lot outside original seven-cell grid".into());
    }
    let width = usize::from(x1 - x0 + 1);
    let depth = usize::from(y1 - y0 + 1);
    let center = [
        f32::from_bits(word(building, 0xc)),
        f32::from_bits(word(building, 0x10)),
    ];
    if !center.iter().all(|v| v.is_finite()) {
        return Err("nonfinite scaffold building center".into());
    }
    let hx = (width as f32 - 1.) * 5.;
    let hz = (depth as f32 - 1.) * 5.;
    let floors = usize::from(building[8]);
    let mut output = Vec::with_capacity((width + depth) * 2 * floors);
    for floor in 0..floors {
        let elevation = 0.5 + floor as f32 * 7.;
        let model_base = usize::from(floor != 0) * 6;
        let mut push = |model: usize, segment: usize, position: [f32; 3], yaw: f32| {
            let frame = i64::from(progress as i32) + segment as i64 - floor as i64 * 25;
            if frame >= 0 {
                output.push(Instance {
                    model: model_base + model,
                    frame: (frame as usize).min(MAX_FRAME),
                    position,
                    yaw,
                });
            }
        };
        for i in 0..width {
            let front = if i == 0 {
                if x0 == 0 {
                    1 + usize::from(y0 == 0)
                } else if y0 == 0 {
                    3
                } else {
                    0
                }
            } else if i + 1 == width && depth > 1 {
                if x1 == 6 {
                    3 - usize::from(y0 == 0)
                } else {
                    usize::from(y0 == 0)
                }
            } else {
                4 + usize::from(y0 == 0)
            };
            push(
                front,
                i,
                [center[0] - hx + i as f32 * 10., elevation, center[1] - hz],
                if i + 1 == width && width > 1 && depth > 1 {
                    0.
                } else {
                    -90.
                },
            );
            let back = if i == 0 {
                if x1 == 6 {
                    1 + usize::from(y1 == 6)
                } else if y1 == 6 {
                    3
                } else {
                    0
                }
            } else if i + 1 == width && depth > 1 {
                if x0 == 0 {
                    3 - usize::from(y1 == 6)
                } else {
                    usize::from(y1 == 6)
                }
            } else {
                4 + usize::from(y1 == 6)
            };
            push(
                back,
                i,
                [center[0] + hx - i as f32 * 10., elevation, center[1] + hz],
                if i + 1 == width && width > 1 && depth > 1 {
                    180.
                } else {
                    90.
                },
            );
        }
        for j in 1..depth {
            if j + 1 == depth && width > 1 {
                continue;
            }
            push(
                4 + usize::from(x0 == 0),
                j,
                [center[0] - hx, elevation, center[1] - hz + j as f32 * 10.],
                180.,
            );
            push(
                4 + usize::from(x1 == 6),
                j,
                [center[0] + hx, elevation, center[1] + hz - j as f32 * 10.],
                0.,
            );
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_scaffold_progress_selects_frame_and_floor() {
        let mut building = [0u8; 0x394];
        building[4..8].copy_from_slice(&[1, 1, 3, 2]);
        building[0xc..0x10].copy_from_slice(&100f32.to_le_bytes());
        building[0x10..0x14].copy_from_slice(&200f32.to_le_bytes());
        building[8] = 3;
        let foundation = instances(&building, 0).unwrap();
        let growing = instances(&building, 25).unwrap();
        assert_eq!(foundation.len(), 6);
        assert_eq!(foundation[0].position, [90., 0.5, 195.]);
        assert_eq!(foundation[1].position, [110., 0.5, 205.]);
        assert_eq!(foundation[0].frame, 0);
        assert_eq!(growing[0].frame, 24);
        assert_eq!(foundation[0].model, 0);
        assert_eq!(growing.len(), 12);
        assert_eq!(growing[6].model, 6);
        assert_eq!(growing[6].frame, 0);
        assert_eq!(growing[6].position, [90., 7.5, 195.]);
        building[4..8].copy_from_slice(&[0, 0, 6, 0]);
        let full_edge = instances(&building, 0).unwrap();
        assert_eq!(full_edge.len(), 14);
        assert_eq!(full_edge[12].frame, 6);
    }

    #[test]
    fn native_corner_models_and_one_cell_lot() {
        let mut building = [0u8; 0x394];
        building[4..8].copy_from_slice(&[0, 0, 1, 1]);
        building[8] = 2;
        building[0xc..0x10].copy_from_slice(&100f32.to_le_bytes());
        building[0x10..0x14].copy_from_slice(&200f32.to_le_bytes());
        let pieces = instances(&building, 25).unwrap();
        assert_eq!(pieces.len(), 8);
        assert_eq!(
            pieces[..4].iter().map(|p| p.model).collect::<Vec<_>>(),
            [2, 0, 1, 3]
        );
        assert_eq!(
            pieces[4..].iter().map(|p| p.model).collect::<Vec<_>>(),
            [8, 6, 7, 9]
        );
        assert_eq!(pieces[4].position, [95., 7.5, 195.]);
        assert_eq!(pieces[6].yaw, 0.);
        building[4..8].copy_from_slice(&[1, 1, 1, 3]);
        let narrow = instances(&building, 0).unwrap();
        assert_eq!(narrow.len(), 6);
        assert_eq!(narrow[4].position, [100., 0.5, 210.]);
        assert_eq!(narrow[5].position, [100., 0.5, 190.]);
    }
}
