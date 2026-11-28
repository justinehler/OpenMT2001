//! Live building geometry binding for [`building_pool::bind_category`].
//!
//! The original `00445fb0 -> 00444ff0 -> 00488230` path has two different
//! responsibilities.  The geometry routine selects and prepares a model,
//! then writes the model-local entrance list.  The category routine later
//! links the building, computes category bounds, and translates that list by
//! the category/building-center delta.  This adapter keeps that ordering.
//!
//! Model registration is deliberately a host seam.  The original handle at
//! `building + 0x170` is a registered model object, not a BuildFile model
//! number.  A host must return the opaque handle and the registered object's
//! bound max-Y (`+0xb4`) through [`ModelRegistrar`].

use std::collections::BTreeMap;

use crate::{
    board::Board,
    build_file::BuildFile,
    building_pool::{self, GeometryBinding},
    scene::{BuildingSpec, SceneBuilding},
};

/// Native category table address in the verified v1.4 image.
pub const V14_CATEGORY_BASE: u32 = 0x0061_7628;
pub const CATEGORY_STRIDE: u32 = 0x17c;
pub const MAX_ENTRANCES: usize = 8;

///444ff0's final dispatch, shared by4450a0 and4454c0. A bound
/// auxiliary model must be present; linked structures above kind1 retain
/// their initial485200 decoration instead of replacing it through485520.
pub fn refresh_appearance_on_bind(record: &[u8; building_pool::RECORD_SIZE]) -> bool {
    let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    word(0x4c) == 1 && (word(0x1c) == u32::MAX || word(0x24) <= 1)
}

/// Result of registering one generated building model with the host.
///
/// `max_y` is the registered model object's post-registration upper-Y bound,
/// corresponding to the native `model + 0xb4` read in `00445fb0`.  It is not
/// the BuildFile model index and must not be substituted with the requested
/// building height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegisteredModel {
    pub handle: u32,
    pub max_y: f32,
}

impl RegisteredModel {
    pub fn new(handle: u32, max_y: f32) -> Result<Self, String> {
        if handle == 0 {
            return Err("registered building model handle is null".into());
        }
        if !max_y.is_finite() {
            return Err("registered building model max-Y is non-finite".into());
        }
        Ok(Self { handle, max_y })
    }

    /// Build the binding from the pointer-free native model definition used
    /// by `mesh::model_definition`.  Native `+0xb4` is definition word 22,
    /// the upper-Y corner, at bytes 88..92.
    pub fn from_definition(handle: u32, definition: &[u8; 116]) -> Result<Self, String> {
        let max_y = f32::from_le_bytes(definition[88..92].try_into().unwrap());
        Self::new(handle, max_y)
    }
}

/// Host operation required to turn a resolved [`SceneBuilding`] into the
/// opaque model object used by the native scene manager.
pub trait ModelRegistrar {
    fn register(&mut self, building: &SceneBuilding) -> Result<RegisteredModel, String>;
    /// Complete the host side of `00444ff0` after the pointer-free fields
    /// above are written.  This is where a renderer may build the scene
    /// matrix, submit the registered model object, and perform any appearance
    /// refresh it owns.  A headless host must provide an explicit no-op.
    fn finish_binding(
        &mut self,
        building: &SceneBuilding,
        record: &mut [u8; building_pool::RECORD_SIZE],
    ) -> Result<(), String>;
    /// Optional retained tree ownership for rendering hosts. Explicitly
    /// headless registrars keep no global tree allocation or visual identity.
    fn finish_tree_binding(
        &mut self,
        _building: &SceneBuilding,
        _file: &BuildFile<'_>,
        _record: &mut [u8; building_pool::RECORD_SIZE],
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Adapter state shared by fresh-map geometry binding and category linking.
///
/// `Board` supplies category names/identity, while `BuildFile` supplies the
/// deterministic model selection and prepared entrance profile.  The host
/// registrar owns GPU/scene resources; this type only keeps the opaque handle
/// to bound-max-Y mapping needed by the native category-height scan.
pub struct BuildingGeometry<'a, R> {
    board: &'a Board,
    file: &'a BuildFile<'a>,
    registrar: R,
    category_base: u32,
    heights: BTreeMap<u32, f32>,
}

impl<'a, R> BuildingGeometry<'a, R> {
    pub fn new(board: &'a Board, file: &'a BuildFile<'a>, registrar: R) -> Self {
        Self::with_category_base(board, file, registrar, V14_CATEGORY_BASE)
    }

    pub fn with_category_base(
        board: &'a Board,
        file: &'a BuildFile<'a>,
        registrar: R,
        category_base: u32,
    ) -> Self {
        Self {
            board,
            file,
            registrar,
            category_base,
            heights: BTreeMap::new(),
        }
    }

    pub fn registrar(&self) -> &R {
        &self.registrar
    }

    pub fn registrar_mut(&mut self) -> &mut R {
        &mut self.registrar
    }

    pub fn model_height_for_handle(&self, handle: u32) -> Result<f32, String> {
        self.heights
            .get(&handle)
            .copied()
            .ok_or_else(|| format!("building model handle {handle:#x} was not registered"))
    }

    pub fn registered_heights(&self) -> &BTreeMap<u32, f32> {
        &self.heights
    }

    /// Reattach a retained host registry when borrowing the geometry adapter
    /// for a later construction batch. Existing category links retain handles.
    pub fn retain_model(&mut self, model: RegisteredModel) -> Result<(), String> {
        RegisteredModel::new(model.handle, model.max_y)?;
        if let Some(previous) = self.heights.get(&model.handle)
            && previous.to_bits() != model.max_y.to_bits()
        {
            return Err("retained model handle changed its bounds".into());
        }
        self.heights.insert(model.handle, model.max_y);
        Ok(())
    }

    fn category_id(&self, reference: u32) -> Result<usize, String> {
        let offset = reference
            .checked_sub(self.category_base)
            .ok_or("building category reference precedes category table")?;
        if !offset.is_multiple_of(CATEGORY_STRIDE) {
            return Err("building category reference is not table-aligned".into());
        }
        let id = (offset / CATEGORY_STRIDE) as usize;
        if id >= 52 {
            return Err("building category reference exceeds52 categories".into());
        }
        if !self.board.blocks.iter().any(|block| block.id == id) {
            return Err("building category is inactive or missing from Board".into());
        }
        Ok(id)
    }

    fn scene_building(
        &self,
        record: &[u8; building_pool::RECORD_SIZE],
    ) -> Result<SceneBuilding, String> {
        let category_reference = u32::from_le_bytes(record[0x40..0x44].try_into().unwrap());
        let category = self.category_id(category_reference)?;
        let block = self
            .board
            .blocks
            .iter()
            .find(|block| block.id == category)
            .ok_or("building category is missing from Board")?;
        let [x0, y0, x1, y1] = [record[4], record[5], record[6], record[7]];
        if x1 < x0 || y1 < y0 || x1 >= 7 || y1 >= 7 {
            return Err("building footprint has invalid inclusive endpoints".into());
        }
        let center = [
            f32::from_le_bytes(record[0x0c..0x10].try_into().unwrap()),
            f32::from_le_bytes(record[0x10..0x14].try_into().unwrap()),
        ];
        if !center.into_iter().all(f32::is_finite) {
            return Err("building center is non-finite".into());
        }
        let kind = record[0x24] as u32;
        let quality = record[0x2c];
        if kind > 2 || quality > 2 {
            return Err("building kind or quality is unsupported".into());
        }
        let mut building = SceneBuilding::resolve(
            BuildingSpec {
                block: block.name.clone(),
                lot: [x0, y0, x1 - x0 + 1, y1 - y0 + 1],
                height: record[8],
                kind: kind as u8,
                variation: u32::from_le_bytes(record[0x20..0x24].try_into().unwrap()),
                quality,
                businesses: Vec::new(),
            },
            self.board,
            self.file,
        )?;
        // A saved/runtime record is authoritative for center.  Fresh map
        // records carry the same resolved value; retaining the record value
        // also preserves the native saved-building path.
        building.center = center;
        Ok(building)
    }

    fn write_scene(
        &mut self,
        record: &mut [u8; building_pool::RECORD_SIZE],
        building: &SceneBuilding,
        registered: RegisteredModel,
    ) -> Result<(), String>
    where
        R: ModelRegistrar,
    {
        let entrances = building.prepared_entrance_positions()?;
        if entrances.len() > MAX_ENTRANCES {
            return Err("building entrance list exceeds native eight-entry storage".into());
        }
        if entrances.iter().flatten().any(|value| !value.is_finite()) {
            return Err("building entrance list contains non-finite coordinates".into());
        }
        if let Some(previous) = self.heights.insert(registered.handle, registered.max_y)
            && previous.to_bits() != registered.max_y.to_bits()
        {
            return Err("model handle was registered with conflicting max-Y bounds".into());
        }
        record[4..8].copy_from_slice(&[
            building.spec.lot[0],
            building.spec.lot[1],
            building.spec.lot[0] + building.spec.lot[2] - 1,
            building.spec.lot[1] + building.spec.lot[3] - 1,
        ]);
        record[8] = building.spec.height;
        for (axis, value) in building.center.into_iter().enumerate() {
            let at = 0x0c + axis * 4;
            record[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        record[0x170..0x174].copy_from_slice(&registered.handle.to_le_bytes());
        record[0x220..0x224].copy_from_slice(&(entrances.len() as u32).to_le_bytes());
        for (index, point) in entrances.into_iter().enumerate() {
            for (axis, value) in point.into_iter().enumerate() {
                let at = 0x224 + index * 12 + axis * 4;
                record[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        // 00488230's successful common tail, retained by 00444ff0 before
        // appearance refresh: render gate, translation, and cleared Y-axis
        // bookkeeping.  These stores happen only after model preparation and
        // entrance emission succeed.
        record[0x4c..0x50].copy_from_slice(&1_u32.to_le_bytes());
        record[0x58..0x5c].copy_from_slice(&building.center[0].to_le_bytes());
        record[0x5c..0x60].copy_from_slice(&0.5_f32.to_le_bytes());
        record[0x60..0x64].copy_from_slice(&building.center[1].to_le_bytes());
        record[0x68..0x6c].fill(0);
        Ok(())
    }

    /// Prepare a caller-resolved scene building.  This is useful to hosts
    /// that already performed map/save selection and want to keep that exact
    /// `SceneBuilding` while still using this adapter's native field writes.
    pub fn prepare_scene(
        &mut self,
        record: &mut [u8; building_pool::RECORD_SIZE],
        building: &SceneBuilding,
    ) -> Result<(), String>
    where
        R: ModelRegistrar,
    {
        let registered = self.registrar.register(building)?;
        let registered = RegisteredModel::new(registered.handle, registered.max_y)?;
        self.write_scene(record, building, registered)?;
        self.registrar.finish_binding(building, record)?;
        self.registrar
            .finish_tree_binding(building, self.file, record)
    }
}

impl<'a, R: ModelRegistrar> GeometryBinding for BuildingGeometry<'a, R> {
    fn prepare(
        &mut self,
        _slot: usize,
        record: &mut [u8; building_pool::RECORD_SIZE],
    ) -> Result<(), String> {
        let building = self.scene_building(record)?;
        self.prepare_scene(record, &building)
    }

    fn model_height(&mut self, handle: u32) -> Result<f32, String> {
        self.model_height_for_handle(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires pinned original whole building appearance phase oracle"]
    fn matches_native_appearance_phase() {
        let data =
            std::fs::read(crate::fixtures::path("NATIVE_BUILDING_APPEARANCE_PHASE")).unwrap();
        assert_eq!(&data[..8], b"MTAPPH01");
        let count = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        assert_eq!(data.len(), 12 + count * 20);
        for row in data[12..].chunks_exact(20) {
            let words: [u32; 5] = std::array::from_fn(|i| {
                u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap())
            });
            let mut record = [0; building_pool::RECORD_SIZE];
            for (at, value) in [(0x4c, words[1]), (0x1c, words[2]), (0x24, words[3])] {
                record[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
            assert_eq!(
                u32::from(refresh_appearance_on_bind(&record)),
                words[4],
                "native entry {:08x}",
                words[0]
            );
        }
    }
    use crate::build_file::ModelMatch;

    #[test]
    #[ignore = "requires private BuildFile and native entrance fixture"]
    fn matches_native_housing_entrance_emission_and_bound_registration() {
        let build = std::fs::read(crate::fixtures::path("BUILD_FILE")).unwrap();
        let native = std::fs::read(crate::fixtures::path("NATIVE_HOUSING_POSITIONS")).unwrap();
        let executable = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let file = BuildFile::parse(&build).unwrap();
        let image = crate::pe::PeImage::parse(&executable).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let word = |at: usize| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        let float = |at: usize| f32::from_bits(word(at));
        assert_eq!(&native[..8], b"MTHPOS01");
        assert_eq!((word(8), word(12)), (39, 0x027f));

        struct Registrar {
            next_handle: u32,
        }
        impl ModelRegistrar for Registrar {
            fn register(&mut self, building: &SceneBuilding) -> Result<RegisteredModel, String> {
                let prepared = crate::geometry::prepare(&building.layers);
                let mut positions = crate::geometry::facade_positions(&prepared);
                positions.extend(crate::roof::mesh(&prepared).positions);
                let samples: Vec<u8> = positions
                    .into_iter()
                    .flat_map(|point| point.into_iter().flat_map(f32::to_le_bytes))
                    .collect();
                let definition = crate::mesh::model_definition(&samples)?;
                let handle = self.next_handle;
                self.next_handle += 1;
                RegisteredModel::from_definition(handle, &definition)
            }

            fn finish_binding(
                &mut self,
                _building: &SceneBuilding,
                _record: &mut [u8; building_pool::RECORD_SIZE],
            ) -> Result<(), String> {
                Ok(())
            }
        }

        let mut adapter = BuildingGeometry::new(&board, &file, Registrar { next_handle: 1 });
        let mut at = 16;
        for case in 0..word(8) {
            let model = word(at) as usize;
            let height = word(at + 4) as u8;
            let building = SceneBuilding {
                spec: BuildingSpec {
                    block: String::new(),
                    lot: [0, 0, 1, 1],
                    height,
                    kind: 1,
                    variation: 0,
                    quality: 0,
                    businesses: vec![],
                },
                block_id: 0,
                center: [float(at + 28), float(at + 32)],
                selected: ModelMatch {
                    index: model,
                    transposed: word(at + 20) != 0,
                },
                model_variants: 0, // Profile-only geometry oracle.
                angle: float(at + 24),
                mirror: word(at + 16) != 0,
                canonical_size: [word(at + 8) as u8, word(at + 12) as u8],
                layers: file.profile(model, height).unwrap(),
            };
            let category_center = [float(at + 36), float(at + 40)];
            let count = word(at + 44) as usize;
            at += 48;
            let expected_emitted: Vec<[f32; 3]> = (0..count)
                .map(|index| std::array::from_fn(|axis| float(at + index * 12 + axis * 4)))
                .collect();
            at += count * 12;
            let expected_placed: Vec<[f32; 3]> = (0..count)
                .map(|index| std::array::from_fn(|axis| float(at + index * 12 + axis * 4)))
                .collect();
            at += count * 12;

            let mut record = [0_u8; building_pool::RECORD_SIZE];
            adapter.prepare_scene(&mut record, &building).unwrap();
            assert_eq!(word_from(&record, 0x220), count as u32, "case {case} count");
            let expected_transformed = building.prepared_entrance_positions().unwrap();
            assert_eq!(
                expected_transformed.len(),
                count,
                "case {case} transformed count"
            );
            for (index, point) in expected_transformed.into_iter().enumerate() {
                for (axis, value) in point.into_iter().enumerate() {
                    assert_eq!(
                        word_from(&record, 0x224 + index * 12 + axis * 4),
                        value.to_bits(),
                        "case {case} prepared entrance {index} axis {axis}"
                    );
                }
            }
            assert_eq!(
                crate::geometry::entrance_positions(&crate::geometry::prepare(&building.layers))
                    .unwrap(),
                expected_emitted,
                "case {case} native entrance emission"
            );
            assert_eq!(
                building.entrance_positions(category_center).unwrap(),
                expected_placed,
                "case {case} category correction"
            );
            assert_ne!(word_from(&record, 0x170), 0);
            assert_eq!(word_from(&record, 0x4c), 1);
            assert_eq!(word_from(&record, 0x5c), 0x3f000000);
        }
        assert_eq!(at, native.len());
    }

    fn word_from(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }
}
