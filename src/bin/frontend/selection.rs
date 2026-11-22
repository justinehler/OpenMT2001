//! Paused inspection selection using original pavement bounds and box narrow phase.
use openmt2001::{
    archive::ArchiveIndex,
    board::{Block, Board},
    mesh::CachedMesh,
    pe::PeImage,
};

pub struct Assets {
    pub blocks: Vec<Block>,
    center: [f32; 3],
    marker: crate::ui::Surface,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub category: usize,
    pub building: Option<usize>,
}

/// Original 4b880c updates each category's dynamic +0xb8 bit1 from 4e0a10.
/// Picking already uses the matching 4e07d0/4e0910 classifier; share its
/// transformed box rather than substituting a category-center depth test.
pub(crate) struct CategoryFrustum {
    view: [f32; 12],
    projection: openmt2001::picking::QueryProjection,
}

impl CategoryFrustum {
    pub fn new(camera: openmt2001::flyby::CameraSample) -> Result<Self, String> {
        let [near, far] = openmt2001::camera::city_clip(camera.eye[1])?;
        Ok(Self {
            view: openmt2001::camera::scene_view_with_up(camera.eye, camera.target, camera.up),
            projection: openmt2001::picking::QueryProjection::new(
                1280., 960., 45., near, far)?,
        })
    }

    pub fn category(&self, id: usize, center: [f32; 2], model_height: Option<f32>)
        -> Result<(openmt2001::picking::QueryBox, bool), String> {
        use openmt2001::{camera, picking};
        let bounds = picking::category_bounds(model_height)?;
        let height = bounds[2][1];
        let radius = ((f64::from(height) * 0.5).powi(2) + 3362.).sqrt() as f32;
        let transform = camera::translated_scene_view(self.view, [center[0], 0., center[1]]);
        let object_center = camera::transform_scene_point(transform, [0., height * 0.5, 0.]);
        let bounds = bounds.map(|point| camera::transform_scene_point(transform, point));
        let visible = self.projection.visible(&bounds, object_center, radius);
        Ok((picking::QueryBox {
            id: id as u32, bounds, center: object_center, radius,
        }, visible))
    }

    pub fn visible(&self, id: usize, center: [f32; 2], model_height: Option<f32>)
        -> Result<bool, String> {
        Ok(self.category(id, center, model_height)?.1)
    }
}

impl Assets {
    pub fn load(
        executable: &[u8],
        directory: &ArchiveIndex,
        archive: &[u8],
    ) -> Result<Self, String> {
        let blocks = Board::from_v14(&PeImage::parse(executable)?)?.blocks;
        let entry = directory
            .find("gamedata2/New_building stuff/pavement.tm")
            .ok_or("missing pavement selection bounds")?;
        let mesh = CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )?;
        let definition = mesh.model_definition()?;
        let f = |i: usize| f32::from_le_bytes(definition[i * 4..i * 4 + 4].try_into().unwrap());
        let entry = directory
            .find("gamedata2/bounding_box/bound_tubed.tm")
            .ok_or("missing selection marker mesh")?;
        let marker_mesh = CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )?;
        if marker_mesh.parts.len() != 1 {
            return Err("unsupported selection marker parts".into());
        }
        let part = &marker_mesh.parts[0];
        let name = marker_mesh.texture_names[part.texture as usize];
        let entry = directory
            .find(&format!("gamedata2/bounding_box/{name}"))
            .or_else(|| directory.find(name))
            .ok_or("missing selection marker texture")?;
        let textures = openmt2001::texture::TextureSet::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .map_err(|e| e.to_string())?;
        let mut marker = crate::ui::Surface::new(&textures.textures[0]);
        let positions = marker_mesh.positions(0)?;
        let uvs = marker_mesh.uvs();
        for i in part.vertex_indices() {
            marker.vertices.push([
                positions[i][0],
                positions[i][1],
                -positions[i][2],
                1.,
                1.,
                1.,
                uvs[i][0],
                uvs[i][1],
            ]);
            marker.alpha.push(255);
        }
        Ok(Self {
            marker,
            blocks,
            center: [f(24), f(25), f(26)],
        })
    }
    /// Retained allocation has room for selected and hovered boxes. Paused
    /// inspection holds the shared logic counter at zero.
    pub fn marker_surface(
        &self,
        selected: Option<Hit>,
        hovered: Option<Hit>,
        scene: &openmt2001::render_scene::Scene,
    ) -> Result<crate::ui::Surface, String> {
        let mut surface = self.marker.clone();
        surface.vertices.clear();
        surface.alpha.clear();
        for (index, selected) in [
            (selected, true),
            (hovered.filter(|i| Some(*i) != selected), false),
        ] {
            let Some(index) = index else { continue };
            let block = &self.blocks[index.category];
            let (bounds, translation) = if let Some(building) = index.building {
                let building = &scene.inspection_buildings[building];
                (building.bounds, building.translation)
            } else {
                (
                    openmt2001::picking::category_bounds(
                        scene.category_model_heights.get(&block.id).copied(),
                    )?,
                    [block.center[0], 0., block.center[1]],
                )
            };
            let markers = openmt2001::picking::block_markers(
                &bounds,
                translation,
                if selected { 0xff00fe00 } else { 0x9f5f5ffe },
                selected,
                0,
            )?;
            for marker in markers {
                // 004e0b50 -> 004e3380, specialized to the original corner
                // rotations (Z=0). Degree conversion is stored as float first.
                let radians = |v: f32| f64::from((f64::from(v) * f64::from(0.017453292f32)) as f32);
                let (sx, cx) = radians(marker.rotation[0]).sin_cos();
                let (sy, cy) = radians(marker.rotation[1]).sin_cos();
                let sy = f64::from(sy as f32);
                let scale = f64::from(marker.scale[0]);
                let matrix = [
                    cy * scale,
                    0.,
                    sy * scale,
                    sy * sx * scale,
                    cx * scale,
                    -cy * sx * scale,
                    -sy * cx * scale,
                    sx * scale,
                    cy * cx * scale,
                ]
                .map(|v| v as f32);
                for vertex in &self.marker.vertices {
                    let mut v = *vertex;
                    for j in 0..3 {
                        v[j] = (f64::from(vertex[2]) * f64::from(matrix[6 + j])
                            + f64::from(vertex[1]) * f64::from(matrix[3 + j])
                            + f64::from(vertex[0]) * f64::from(matrix[j])
                            + f64::from(marker.position[j])) as f32;
                    }
                    for (j, shift) in [16, 8, 0].into_iter().enumerate() {
                        v[3 + j] = ((marker.color >> shift) & 255) as f32 / 255.;
                    }
                    surface.vertices.push(v);
                    surface.alpha.push((marker.color >> 24) as u8);
                }
            }
        }
        Ok(surface)
    }
    /// The quoted SceneBuilding is assembled alone, so its exact model bounds
    /// can use the same original bound_tubed marker as a selected city building.
    pub fn preview_marker_surface(
        &self,
        scene: &openmt2001::render_scene::Scene,
    ) -> Result<crate::ui::Surface, String> {
        if scene.inspection_buildings.len() != 1 {
            return Err("Planning preview must contain one building".into());
        }
        let category = self.blocks.iter().position(|block| {
            block.id == scene.inspection_buildings[0].category
        }).ok_or("Planning preview category missing")?;
        self.marker_surface(Some(Hit { category, building: Some(0) }), None, scene)
    }
    pub fn center(&self, index: usize) -> [f32; 3] {
        let block = &self.blocks[index];
        [
            block.center[0] + self.center[0],
            self.center[1],
            block.center[1] + self.center[2],
        ]
    }
    pub fn pick(
        &self,
        camera: openmt2001::flyby::CameraSample,
        scene: &openmt2001::render_scene::Scene,
        point: [i32; 2],
        previous: Option<Hit>,
    ) -> Option<Hit> {
        use openmt2001::{camera as cpu, picking};
        let frustum = CategoryFrustum::new(camera).ok()?;
        let view = frustum.view;
        let projection = &frustum.projection;
        let mut categories = Vec::new();
        let mut category_indices = Vec::new();
        let mut building_indices = Vec::new();
        for (index, block) in self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.flags & 1 == 0)
        {
            let (bounds, visible) = frustum.category(block.id, block.center,
                scene.category_model_heights.get(&block.id).copied()).ok()?;
            let mut indices = Vec::new();
            let mut buildings = Vec::new();
            for (i, b) in scene
                .inspection_buildings
                .iter()
                .enumerate()
                .filter(|(_, b)| b.category == block.id)
            {
                let transform = cpu::translated_scene_view(view, b.translation);
                buildings.push(picking::QueryBox {
                    id: b.saved_slot.unwrap_or(i) as u32,
                    bounds: b.bounds.map(|p| cpu::transform_scene_point(transform, p)),
                    center: cpu::transform_scene_point(transform, b.center),
                    radius: b.radius,
                });
                indices.push(i);
            }
            categories.push(picking::QueryCategory {
                bounds,
                visible,
                buildings,
            });
            category_indices.push(index);
            building_indices.push(indices);
        }
        let previous =
            previous.and_then(|hit| category_indices.iter().position(|i| *i == hit.category));
        let hit = picking::hover(projection, &categories, previous, point).ok()??;
        Some(Hit {
            category: category_indices[hit.category],
            building: hit.building.map(|i| building_indices[hit.category][i]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original v1.4 category-visibility oracle"]
    fn category_frustum_matches_native_bit_one() {
        let fixture = std::fs::read("work/frontend/native-category-visibility.bin").unwrap();
        assert_eq!(&fixture[..8], b"MTCVIS01");
        let word = |at: usize| u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
        let float = |at: usize| f32::from_bits(word(at));
        let count = word(8) as usize;
        assert_eq!(fixture.len(), 12 + 7 * 4 + 0x74 + count * 20);
        let parameters = std::array::from_fn::<_, 7, _>(|i| float(12 + i * 4));
        assert_eq!(&parameters, &[640., 480., 45., 4., 3000., 1., 1.]);
        let frustum = CategoryFrustum {
            view: [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.],
            projection: openmt2001::picking::QueryProjection::new(
                parameters[0], parameters[1], parameters[2],
                parameters[3], parameters[4]).unwrap(),
        };
        for i in 0..count {
            let at = 12 + 7 * 4 + 0x74 + i * 20;
            let translation = [float(at), float(at + 4), float(at + 8)];
            assert_eq!(translation[1], 0.);
            let classification = word(at + 12);
            let flags = word(at + 16);
            let visible = frustum.visible(i, [translation[0], translation[2]], None).unwrap();
            assert_eq!(visible, flags & 2 != 0,
                "native class {classification:#x} at {translation:?}");
            assert_eq!(visible, classification != 0x40);
        }
    }
}
