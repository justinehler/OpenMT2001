//! City inspection selection using original pavement bounds and box narrow phase.
use openmt2001::{
    archive::ArchiveIndex,
    board::{Block, Board},
    mesh::CachedMesh,
    pe::PeImage,
};

pub use openmt2001::pavement_resource;

pub struct Assets {
    pub blocks: Vec<Block>,
    center: [f32; 3],
    pavement: pavement_resource::Directory,
    marker: crate::ui::Surface,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub category: usize,
    pub building: Option<usize>,
}

/// Geometry is independent of camera frames; only the world pulse phase moves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkerState {
    selected: Option<Hit>,
    hovered: Option<Hit>,
    pulse_phase: u32,
}

/// Original 4b880c updates each category's dynamic +0xb8 bit1 from 4e0a10.
/// Picking already uses the matching 4e07d0/4e0910 classifier; share its
/// transformed box rather than substituting a category-center depth test.
pub struct CategoryFrustum {
    view: [f32; 12],
    projection: openmt2001::picking::QueryProjection,
}

impl CategoryFrustum {
    pub fn new(camera: openmt2001::flyby::CameraSample) -> Result<Self, String> {
        let [near, far] = openmt2001::camera::city_clip(camera.eye[1])?;
        Ok(Self {
            view: openmt2001::camera::scene_view_with_up(camera.eye, camera.target, camera.up),
            projection: openmt2001::picking::QueryProjection::new(
                1280.,
                960.,
                camera.vertical_fov,
                near,
                far,
            )?,
        })
    }

    pub fn category(
        &self,
        id: usize,
        center: [f32; 2],
        model_height: Option<f32>,
    ) -> Result<(openmt2001::picking::QueryBox, bool), String> {
        use openmt2001::{camera, picking};
        let bounds = picking::category_bounds(model_height)?;
        let height = bounds[2][1];
        let radius = ((f64::from(height) * 0.5).powi(2) + 3362.).sqrt() as f32;
        let transform = camera::translated_scene_view(self.view, [center[0], 0., center[1]]);
        let object_center = camera::transform_scene_point(transform, [0., height * 0.5, 0.]);
        let bounds = bounds.map(|point| camera::transform_scene_point(transform, point));
        let visible = self.projection.visible(&bounds, object_center, radius);
        Ok((
            picking::QueryBox {
                id: id as u32,
                bounds,
                center: object_center,
                radius,
            },
            visible,
        ))
    }

    pub fn visible(
        &self,
        id: usize,
        center: [f32; 2],
        model_height: Option<f32>,
    ) -> Result<bool, String> {
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
        // Install the SAME computed CPU definition and exact archive source.
        // This finite resource is not the native mesh/material/render manager.
        let pavement = pavement_resource::Directory::install(
            pavement_resource::LoadedPavement::from_loaded(directory, archive, entry, definition)?,
        )?;
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
            pavement,
        })
    }
    /// Sole retained occupied definition owner; callers resolve typed bindings.
    pub fn pavement_resource(&self) -> &pavement_resource::Directory {
        &self.pavement
    }
    pub fn pavement_definition(&self) -> Result<[u8; 116], String> {
        self.pavement.binding()?.definition()
    }
    /// Actual asset removal only. Root must retire dependent Pool allocations
    /// BEFORE calling this, replacing Assets, or dropping the process owner.
    pub fn retire_pavement_resource(&mut self) -> Result<(), String> {
        self.pavement.retire()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn descriptor_test_assets(blocks: Vec<Block>, definition: [u8; 116]) -> Self {
        // Authored test fixture only; never used as a live/default producer.
        let center = [24, 25, 26].map(|i| {
            f32::from_bits(u32::from_le_bytes(
                definition[i * 4..i * 4 + 4].try_into().unwrap(),
            ))
        });
        Self {
            blocks,
            center,
            marker: crate::ui::Surface::solid(),
            pavement: pavement_resource::Directory::synthetic(definition),
        }
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn replace_descriptor_test_resource(
        &mut self,
        definition: [u8; 116],
    ) -> Result<(), String> {
        self.pavement.replace_synthetic(definition)?;
        self.center = [24, 25, 26].map(|i| {
            f32::from_bits(u32::from_le_bytes(
                definition[i * 4..i * 4 + 4].try_into().unwrap(),
            ))
        });
        Ok(())
    }
    /// Refresh the retained marker geometry only on identity/scene invalidation
    /// or a new selected pulse phase. A paused world keeps the same tick.
    pub fn refresh_marker_surface(
        &self,
        state: &mut Option<MarkerState>,
        hits: [Option<Hit>; 2],
        scene: &openmt2001::render_scene::Scene,
        world_tick: u32,
        invalidated: bool,
    ) -> Result<Option<crate::ui::Surface>, String> {
        let [selected, hovered] = hits;
        let next = MarkerState {
            selected,
            hovered,
            pulse_phase: if selected.is_some() {
                world_tick & 31
            } else {
                0
            },
        };
        if !invalidated && *state == Some(next) {
            return Ok(None);
        }
        let surface = self.marker_surface(selected, hovered, scene, world_tick)?;
        *state = Some(next);
        Ok(Some(surface))
    }

    /// Retained allocation has room for selected and hovered boxes.
    pub fn marker_surface(
        &self,
        selected: Option<Hit>,
        hovered: Option<Hit>,
        scene: &openmt2001::render_scene::Scene,
        world_tick: u32,
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
                world_tick,
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
        let category = self
            .blocks
            .iter()
            .position(|block| block.id == scene.inspection_buildings[0].category)
            .ok_or("Planning preview category missing")?;
        self.marker_surface(
            Some(Hit {
                category,
                building: Some(0),
            }),
            None,
            scene,
            0,
        )
    }
    /// 482580 consumes actor translation and model center SEPARATELY, with
    /// retained x87 addition. Do not feed already-rounded center(index).
    /// Original44faf0 creates pavement descriptors at [blockX,0,blockZ].
    pub fn camera_block_initialization(
        &self,
        category_id: u32,
        height_word: u32,
        radius_word: u32,
    ) -> Result<openmt2001::camera::primary::BlockInitialization, String> {
        let block = self
            .blocks
            .iter()
            .find(|block| block.id == category_id as usize)
            .filter(|block| block.flags & 1 == 0)
            .ok_or("Camera category has no active immutable pavement descriptor")?;
        Ok(openmt2001::camera::primary::BlockInitialization {
            actor_translation: [block.center[0].to_bits(), 0, block.center[1].to_bits()],
            model_offsets: self.center.map(f32::to_bits),
            height_word,
            radius_word,
        })
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
            let (bounds, visible) = frustum
                .category(
                    block.id,
                    block.center,
                    scene.category_model_heights.get(&block.id).copied(),
                )
                .ok()?;
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
    fn marker_refresh_tracks_world_phase_without_animating_hover_or_render_frames() {
        // A synthetic square model supplies a real Scene without original assets/GPU.
        let mut build = [0u8; 204];
        for (index, word) in [
            (0, 0x4c6d4266u32),
            (1, 9),
            (2, 204),
            (11, 4),
            (12, 100), // points
            (13, 4),
            (14, 132), // point attributes
            (15, 1),
            (16, 136), // polygon
            (17, 1),
            (18, 140), // attribute range
            (19, 1),
            (20, 144), // layer
            (21, 1),
            (22, 176), // model
        ] {
            build[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        for (i, point) in [[-1f32, -1.], [1., -1.], [1., 1.], [-1., 1.]]
            .into_iter()
            .enumerate()
        {
            for (axis, value) in point.into_iter().enumerate() {
                let at = 100 + i * 8 + axis * 4;
                build[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        build[136..140].copy_from_slice(&(4u32 << 24).to_le_bytes());
        build[156..160].copy_from_slice(&10f32.to_le_bytes());
        build[160] = 1; // flat roof
        build[180..184].copy_from_slice(&1u32.to_le_bytes()); // one model layer
        let scene = openmt2001::render_scene::assemble(openmt2001::render_scene::Request {
            build_file: &build,
            source: openmt2001::render_scene::Source::Model {
                index: 0,
                height: 4,
                variation: 0,
            },
            original: None,
            suncols: None,
            phase: None,
            camera: None,
            width: 1280,
            height: 960,
            animated: false,
        })
        .unwrap();
        let blocks = [0., 200.]
            .into_iter()
            .enumerate()
            .map(|(id, x)| Block {
                id,
                name: format!("Block {id}"),
                text_id: 0,
                kind: 0,
                center: [x, 0.],
                size: [82.; 2],
                pavement_color: 0,
                flags: 0,
                street_sides: 0,
            })
            .collect();
        let mut assets = Assets::descriptor_test_assets(blocks, [0; 116]);
        // One synthetic triangle per corner lets us inspect final draw vertices.
        assets.marker.vertices = vec![
            [0., 0., 0., 1., 1., 1., 0., 0.],
            [1., 0., 0., 1., 1., 1., 1., 0.],
            [0., 1., 0., 1., 1., 1., 0., 1.],
        ];
        assets.marker.alpha = vec![255; 3];
        let selected = Some(Hit {
            category: 0,
            building: None,
        });
        let hovered = Some(Hit {
            category: 1,
            building: None,
        });
        let mut state = None;
        let mut refresh = |tick, hits, invalidated| {
            assets
                .refresh_marker_surface(&mut state, hits, &scene, tick, invalidated)
                .unwrap()
        };
        let first = refresh(0, [selected, hovered], false).unwrap();
        for _ in 0..100 {
            assert!(refresh(0, [selected, hovered], false).is_none());
        }
        let second = refresh(15, [selected, hovered], false).unwrap();
        let split = 8 * assets.marker.vertices.len();
        assert_eq!(first.vertices.len(), split * 2);
        assert_eq!(second.vertices.len(), split * 2);
        assert_ne!(&first.vertices[..split], &second.vertices[..split]);
        for (a, b) in first.vertices[..split]
            .iter()
            .zip(&second.vertices[..split])
        {
            assert_ne!(&a[..3], &b[..3], "selected bounds must expand with phase");
            assert_eq!(&a[3..6], &[0., 246. / 255., 0.]);
            assert_eq!(&b[3..6], &[0., 127. / 255., 0.]);
        }
        assert_eq!(&first.vertices[split..], &second.vertices[split..]);
        assert!(
            second.vertices[split..]
                .iter()
                .all(|v| v[3..6] == [95. / 255., 95. / 255., 254. / 255.])
        );
        assert!(second.alpha[..split].iter().all(|&a| a == 255));
        assert!(second.alpha[split..].iter().all(|&a| a == 159));
        assert!(refresh(15, [selected, hovered], false).is_none());
        assert!(
            refresh(47, [selected, hovered], false).is_none(),
            "same repeating phase"
        );
        assert!(
            refresh(47, [selected, hovered], true).is_some(),
            "scene invalidation still refreshes"
        );

        let duplicate = refresh(15, [selected, selected], false).unwrap();
        assert_eq!(duplicate.vertices, second.vertices[..split]);
        let hover_only = refresh(15, [None, hovered], false).unwrap();
        assert_eq!(hover_only.vertices, second.vertices[split..]);
        assert!(refresh(16, [None, hovered], false).is_none());
    }

    #[test]
    fn strategic_categories_pick_at_their_rendered_positions() {
        use glam::{Mat4, Vec3};
        use openmt2001::{camera, picking};
        let sample = crate::navigation::strategic_camera(0.1);
        let frustum = CategoryFrustum::new(sample).unwrap();
        // Separated synthetic categories exercise the edges as well as center.
        let centers = [[-400., -350.], [400., -350.], [-400., 350.], [400., 350.]];
        let categories = centers
            .into_iter()
            .enumerate()
            .map(|(id, center)| {
                let (bounds, visible) = frustum.category(id, center, None).unwrap();
                picking::QueryCategory {
                    bounds,
                    visible,
                    buildings: vec![],
                }
            })
            .collect::<Vec<_>>();
        let [near, far] = camera::city_clip(sample.eye[1]).unwrap();
        for [width, height] in [[800, 600], [1024, 768]] {
            let projection = camera::projection(
                sample.vertical_fov,
                near,
                far,
                width,
                height,
                [0., 1., 0., 1.],
            )
            .unwrap();
            let matrix = Mat4::from_cols_array(&projection)
                * Mat4::from_cols_array(&camera::view_with_up(
                    sample.eye,
                    sample.target,
                    sample.up,
                ));
            for (id, center) in centers.into_iter().enumerate() {
                let clip = matrix * Vec3::new(center[0], 0., center[1]).extend(1.);
                let pixel = [
                    (clip.x / clip.w + 1.) * width as f32 * 0.5,
                    (1. - clip.y / clip.w) * height as f32 * 0.5,
                ];
                let point = [
                    (pixel[0] * 1280. / width as f32) as i32,
                    (pixel[1] * 960. / height as f32) as i32,
                ];
                let hit = picking::hover(&frustum.projection, &categories, None, point)
                    .unwrap()
                    .unwrap();
                assert_eq!(hit.category, id, "category {id} at {width}x{height}");
            }
        }
    }

    #[test]
    #[ignore = "requires original v1.4 category-visibility oracle"]
    fn category_frustum_matches_native_bit_one() {
        let fixture =
            std::fs::read(openmt2001::fixtures::path("NATIVE_CATEGORY_VISIBILITY")).unwrap();
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
                parameters[0],
                parameters[1],
                parameters[2],
                parameters[3],
                parameters[4],
            )
            .unwrap(),
        };
        for i in 0..count {
            let at = 12 + 7 * 4 + 0x74 + i * 20;
            let translation = [float(at), float(at + 4), float(at + 8)];
            assert_eq!(translation[1], 0.);
            let classification = word(at + 12);
            let flags = word(at + 16);
            let visible = frustum
                .visible(i, [translation[0], translation[2]], None)
                .unwrap();
            assert_eq!(
                visible,
                flags & 2 != 0,
                "native class {classification:#x} at {translation:?}"
            );
            assert_eq!(visible, classification != 0x40);
        }
    }
}
