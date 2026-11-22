//! Retained native frontend model handles for fresh-building preparation.
//! CPU geometry and instance submissions survive until shared scene assembly;
//! these handles are local registry identities, never original process pointers.
use openmt2001::{
    building_geometry::{ModelRegistrar, RegisteredModel},
    scene::SceneBuilding,
};
#[derive(Default)]
pub struct Models {
    pub models: Vec<Model>,
    pub bindings: std::collections::BTreeMap<usize, Binding>,
    /// Original saved 485200 detail identities, in renderer request order.
    /// They are not GPU handles; 4846f0 must resolve each candidate before
    /// the positive-count 485520 refresh can run.
    pub appearance_candidates:
        std::collections::BTreeMap<usize, openmt2001::save::appearance::ModelIdentity>,
}
pub struct Model {
    pub positions: Vec<[f32; 3]>,
    pub definition: [u8; 116],
}
pub struct Binding {
    pub handle: u32,
    pub translation: [f32; 3],
}

/// Release the record-owned auxiliary state of a retained building. This
/// frontend submits geometry from `Models::bindings`; it does not allocate the
/// original renderer's inline arrays or global instance tables. Reject such
/// foreign registrations rather than treating them as our own handles.
pub fn release_record(record: &mut [u8; 0x394]) -> Result<(), String> {
    use openmt2001::building_pool::{AuxiliaryCleanupCallbacks, cleanup_auxiliary_record};
    cleanup_auxiliary_record(
        (&mut record[0x4c..0x38c]).try_into().unwrap(),
        &mut AuxiliaryCleanupCallbacks {
            release_array: &mut |_, _, handle| {
                Err(format!("foreign building model registration {handle}"))
            },
            release_renderer: &mut |index| Err(format!("foreign renderer registration {index}")),
            release_aux_registry: &mut |index| {
                Err(format!("foreign auxiliary registration {index}"))
            },
            //485880 retires the original dynamic draw entries. This host's
            // counterpart is removal from the retained binding map below.
            refresh: &mut |_| Ok(()),
        },
    )?;
    Ok(())
}
impl ModelRegistrar for Models {
    fn register(&mut self, building: &SceneBuilding) -> Result<RegisteredModel, String> {
        let prepared = openmt2001::geometry::prepare(&building.layers);
        let positions: Vec<_> = openmt2001::geometry::facade_positions(&prepared)
            .into_iter()
            .chain(openmt2001::roof::mesh(&prepared).positions)
            .map(|p| building.local_position(p))
            .collect();
        let bytes: Vec<_> = positions
            .iter()
            .flat_map(|p| p.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        let definition = openmt2001::mesh::model_definition(&bytes)?;
        let handle = u32::try_from(self.models.len() + 1)
            .map_err(|_| "frontend building model handles exhausted")?;
        let registered = RegisteredModel::from_definition(handle, &definition)?;
        self.models.push(Model {
            positions,
            definition,
        });
        Ok(registered)
    }
    fn finish_binding(
        &mut self,
        _: &SceneBuilding,
        record: &mut [u8; 0x394],
    ) -> Result<(), String> {
        let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let slot = word(0) as usize;
        let handle = word(0x170);
        if slot >= 600 || handle == 0 || handle as usize > self.models.len() {
            return Err("frontend building submission identity".into());
        }
        let translation = std::array::from_fn(|i| f32::from_bits(word(0x58 + i * 4)));
        if !translation.into_iter().all(f32::is_finite) {
            return Err("frontend building submission translation".into());
        }
        self.bindings.insert(
            slot,
            Binding {
                handle,
                translation,
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::{
        board::Board,
        build_file::BuildFile,
        building_geometry::{BuildingGeometry, V14_CATEGORY_BASE},
        building_pool,
        pe::PeImage,
        render_scene::{self, OriginalAssets, Request, Source},
        scene,
    };
    #[test]
    #[ignore = "requires original map, executable, BuildFile and scenery assets"]
    fn live_building_records_match_map_scene() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_bytes = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_MAP");
        let map = std::str::from_utf8(&map).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let file = BuildFile::parse(&build_bytes).unwrap();
        let mut records = vec![[0; building_pool::RECORD_SIZE]; 600];
        building_pool::reset(&mut records, &mut [0; building_pool::RECORD_SIZE]).unwrap();
        let mut categories = [[0; 0x17c]; 52];
        let definitions = openmt2001::initial_conditions::Definitions::from_v14(&image).unwrap();
        openmt2001::category_reset::reset_after_lists_removed(
            &mut categories,
            &definitions.reset_values(),
        );
        for (row, center) in categories.iter_mut().zip(board.category_geometry().centers) {
            row[0xe0..0xe4].copy_from_slice(&center[0].to_le_bytes());
            row[0xe4..0xe8].copy_from_slice(&center[1].to_le_bytes());
        }
        let mut geometry = BuildingGeometry::new(&board, &file, Models::default());
        for (slot, spec) in scene::parse_static_map(map)
            .unwrap()
            .into_iter()
            .enumerate()
        {
            let building = SceneBuilding::resolve(spec, &board, &file).unwrap();
            let [x, y, w, d] = building.spec.lot.map(i32::from);
            building_pool::initialize(
                &mut records[slot],
                &building_pool::Initialization {
                    rectangle: [x, y, x + w - 1, y + d - 1],
                    height: u32::from(building.spec.height),
                    center: building.center,
                    variation: building.spec.variation,
                    kind: u32::from(building.spec.kind),
                    quality: building.spec.quality as i8,
                    // Geometry-only fixture: no cost/date or full scenario initialization claim.
                    total_cost: 0,
                    date: [0; 2],
                },
                &image,
            )
            .unwrap();
            building_pool::bind_category(
                &mut categories[building.block_id],
                &mut records,
                slot,
                0x10000000,
                V14_CATEGORY_BASE + building.block_id as u32 * 0x17c,
                &mut geometry,
            )
            .unwrap();
        }
        let request = |source| Request {
            build_file: &build_bytes,
            source,
            original: Some(OriginalAssets {
                executable: &executable,
                directory: &directory,
                archive: &archive,
            }),
            suncols: Some(&suncols),
            phase: Some(9),
            camera: None,
            width: 1280,
            height: 960,
            animated: false,
        };
        let expected = render_scene::assemble(request(Source::Map(map))).unwrap();
        let actual = render_scene::assemble(request(Source::RuntimeBuildings {
            records: &records,
            category_base: V14_CATEGORY_BASE,
        }))
        .unwrap();
        assert_eq!(actual.building_count, expected.building_count);
        assert_eq!(geometry.registrar().bindings.len(), actual.building_count);
        assert_eq!(
            actual.category_model_heights,
            expected.category_model_heights
        );
        assert_eq!(actual.draws.len(), expected.draws.len());
        for (a, b) in actual.draws.iter().zip(&expected.draws) {
            let (a, b) = (a.borrowed(), b.borrowed());
            assert_eq!(a.mode, b.mode);
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(a.vertices),
                bytemuck::cast_slice::<_, u8>(b.vertices)
            );
            assert_eq!(a.rgba, b.rgba);
            assert_eq!(a.view_projection, b.view_projection);
        }
        for (slot, (a, b)) in actual
            .inspection_buildings
            .iter()
            .zip(&expected.inspection_buildings)
            .enumerate()
        {
            assert_eq!(a.camera_entrance, b.camera_entrance);
            assert!(a.saved_slot.is_none() && a.saved_business.is_none());
            assert_eq!(a.runtime_slot, Some(slot));
            assert!(b.runtime_slot.is_none());
            let binding = &geometry.registrar().bindings[&slot];
            let model = &geometry.registrar().models[binding.handle as usize - 1];
            assert!(!model.positions.is_empty());
            assert_eq!(binding.translation[1], 0.5);
            assert_eq!(
                RegisteredModel::from_definition(binding.handle, &model.definition)
                    .unwrap()
                    .max_y,
                geometry.model_height_for_handle(binding.handle).unwrap()
            );
        }
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_RUNTIME_BUILDING_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(target_os = "windows") {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            crate::capture_test::capture(&renderer, None, &actual, &path);
        }
        println!(
            "{} live bound buildings match original map scene; no save provenance",
            actual.building_count
        );
    }
}
