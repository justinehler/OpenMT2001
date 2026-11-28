//! Retained native frontend model handles for fresh-building preparation.
//! CPU geometry and instance submissions survive until shared scene assembly;
//! these handles are local registry identities, never original process pointers.
use std::{cell::RefCell, rc::Rc};

pub type TreeState = Rc<RefCell<crate::trees::building::OwnedPool>>;

use crate::{
    building_geometry::{ModelRegistrar, RegisteredModel},
    scene::SceneBuilding,
};
#[derive(Default)]
pub struct Models {
    pub models: Vec<Model>,
    pub tree_state: Option<TreeState>,
    pub bindings: std::collections::BTreeMap<usize, Binding>,
    /// Original saved 485200 detail identities, in renderer request order.
    /// They are not GPU handles; 4846f0 must resolve each candidate before
    /// the positive-count 485520 refresh can run.
    pub appearance_candidates:
        std::collections::BTreeMap<usize, crate::save::appearance::ModelIdentity>,
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
    release_record_with_trees(record, None)
}

pub fn release_record_with_trees(
    record: &mut [u8; 0x394],
    trees: Option<&TreeState>,
) -> Result<(), String> {
    use crate::trees::building::{OWNER_COUNT_OFFSET, OWNER_INDICES_OFFSET};
    let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    let owner = word(0) as usize;
    if word(0x4c) == 1
        && let Some(trees) = trees
    {
        let count = word(OWNER_COUNT_OFFSET) as usize;
        if count > 8 {
            return Err("building tree cleanup count exceeds owner storage".into());
        }
        let listed: Vec<_> = (0..count)
            .map(|i| word(OWNER_INDICES_OFFSET + i * 4) as usize)
            .collect();
        let pool = trees.borrow();
        if pool.indices(owner).unwrap_or(&[]) != listed {
            return Err("building tree cleanup does not match owned identities".into());
        }
    }

    use crate::building_pool::{AuxiliaryCleanupCallbacks, cleanup_auxiliary_record};
    let cleaned = cleanup_auxiliary_record(
        (&mut record[0x4c..0x38c]).try_into().unwrap(),
        &mut AuxiliaryCleanupCallbacks {
            release_array: &mut |_, _, handle| {
                Err(format!("foreign building model registration {handle}"))
            },
            release_renderer: &mut |index| Err(format!("foreign renderer registration {index}")),
            release_aux_registry: &mut |index| {
                trees
                    .ok_or_else(|| format!("foreign auxiliary registration {index}"))?
                    .borrow_mut()
                    .release_next(owner, index)
            },
            //485880 retires the original dynamic draw entries. This host's
            // counterpart is removal from the retained binding map below.
            refresh: &mut |_| Ok(()),
        },
    )?;
    // An owned zero-tree building has no per-row cleanup callback.
    if cleaned && let Some(trees) = trees {
        trees.borrow_mut().release(owner)?;
    }
    Ok(())
}
impl Models {
    pub fn with_tree_locators(locators: Option<&[[f32; 3]]>) -> Result<Self, String> {
        Ok(Self {
            tree_state: locators
                .map(|locators| {
                    crate::trees::building::OwnedPool::ambient(locators)
                        .map(|pool| Rc::new(RefCell::new(pool)))
                })
                .transpose()?,
            ..Self::default()
        })
    }
    pub fn with_tree_state(tree_state: Option<TreeState>) -> Self {
        Self {
            tree_state,
            ..Self::default()
        }
    }
}
impl ModelRegistrar for Models {
    fn register(&mut self, building: &SceneBuilding) -> Result<RegisteredModel, String> {
        let prepared = crate::geometry::prepare(&building.layers);
        let positions: Vec<_> = crate::geometry::facade_positions(&prepared)
            .into_iter()
            .chain(crate::roof::mesh(&prepared).positions)
            .map(|p| building.local_position(p))
            .collect();
        let bytes: Vec<_> = positions
            .iter()
            .flat_map(|p| p.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        let definition = crate::mesh::model_definition(&bytes)?;
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
    fn finish_tree_binding(
        &mut self,
        building: &SceneBuilding,
        file: &crate::build_file::BuildFile<'_>,
        record: &mut [u8; 0x394],
    ) -> Result<(), String> {
        let Some(trees) = &self.tree_state else {
            return Ok(());
        };
        use crate::trees::building::{self, OWNER_COUNT_OFFSET, OWNER_INDICES_OFFSET};
        let locals = building::locals(file, building.selected, building.spec.height)?;
        let pose = building::Pose::from_scene(building);
        let poses = locals
            .into_iter()
            .map(|local| pose.place(local))
            .collect::<Result<Vec<_>, _>>()?;
        let owner = u32::from_le_bytes(record[..4].try_into().unwrap()) as usize;
        let mut pool = trees.borrow_mut();
        let indices = pool.bind(owner, &poses)?;
        record[OWNER_COUNT_OFFSET..OWNER_COUNT_OFFSET + 4]
            .copy_from_slice(&(indices.len() as u32).to_le_bytes());
        record[OWNER_INDICES_OFFSET..OWNER_INDICES_OFFSET + 32].fill(0);
        for (i, &index) in indices.iter().enumerate() {
            record[OWNER_INDICES_OFFSET + i * 4..OWNER_INDICES_OFFSET + i * 4 + 4]
                .copy_from_slice(&(index as u32).to_le_bytes());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        board::Board,
        build_file::BuildFile,
        building_geometry::{BuildingGeometry, V14_CATEGORY_BASE},
        building_pool,
        pe::PeImage,
        render_scene::{self, OriginalAssets, Request, Source},
        scene,
    };
    #[test]
    #[ignore = "requires player-supplied original model/board/tree data"]
    fn original_building_tree_binding_release_and_slot_reuse() {
        use super::*;
        use crate::{
            archive::ArchiveIndex,
            board::Board,
            build_file::BuildFile,
            building_geometry::BuildingGeometry,
            pe::PeImage,
            scene::BuildingSpec,
            trees::{
                self,
                building::{OWNER_COUNT_OFFSET, OWNER_INDICES_OFFSET},
            },
        };
        let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let image = PeImage::parse(&executable).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let assets = trees::Assets::load(&image, &index, &archive).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let file = BuildFile::parse(&build).unwrap();
        let mut geometry = BuildingGeometry::new(
            &board,
            &file,
            Models::with_tree_locators(Some(&assets.locators)).unwrap(),
        );
        let state = geometry.registrar().tree_state.clone().unwrap();
        let make = |x| {
            SceneBuilding::resolve(
                BuildingSpec {
                    block: board
                        .blocks
                        .iter()
                        .find(|b| b.id == 9)
                        .unwrap()
                        .name
                        .clone(),
                    lot: [x, 0, 1, 2],
                    height: 1,
                    kind: 2,
                    variation: 0,
                    quality: 0,
                    businesses: Vec::new(),
                },
                &board,
                &file,
            )
            .unwrap()
        };
        let a = make(1);
        let b = make(3);
        assert_eq!(a.selected.index, 236);
        assert_eq!(b.selected.index, 236);
        let record = |slot: u32| {
            let mut r = [0u8; 916];
            r[..4].copy_from_slice(&slot.to_le_bytes());
            r[0x24..0x28].copy_from_slice(&2u32.to_le_bytes());
            r[0x3f] = 1;
            r
        };
        let mut ra = record(5);
        let mut rb = record(1);
        geometry.prepare_scene(&mut ra, &a).unwrap();
        geometry.prepare_scene(&mut rb, &b).unwrap();
        let word = |r: &[u8], at| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
        assert_eq!(word(&ra, OWNER_COUNT_OFFSET), 1);
        assert_eq!(word(&ra, OWNER_INDICES_OFFSET), 137);
        assert_eq!(word(&rb, OWNER_INDICES_OFFSET), 138);
        let b_indices = state.borrow().indices(1).unwrap().to_vec();
        let expected = trees::building::Pose::from_scene(&a)
            .place(trees::building::locals(&file, a.selected, 1).unwrap()[0])
            .unwrap();
        assert_eq!(state.borrow().pool().slot(137).unwrap(), &expected);
        let revision = state.borrow().revision();
        let mut foreign = rb;
        foreign[OWNER_INDICES_OFFSET..OWNER_INDICES_OFFSET + 4]
            .copy_from_slice(&137u32.to_le_bytes());
        assert!(release_record_with_trees(&mut foreign, Some(&state)).is_err());
        assert_eq!(state.borrow().revision(), revision);
        release_record_with_trees(&mut ra, Some(&state)).unwrap();
        assert_eq!(word(&ra, OWNER_COUNT_OFFSET), 0);
        assert_eq!(state.borrow().pool().count(), 138);
        assert_eq!(state.borrow().indices(1).unwrap(), b_indices);
        assert!(
            !state
                .borrow()
                .pool()
                .submission_slots()
                .any(|(i, _)| i == 138)
        );
        let revision = state.borrow().revision();
        release_record_with_trees(&mut ra, Some(&state)).unwrap();
        assert_eq!(state.borrow().revision(), revision);
        geometry.prepare_scene(&mut ra, &a).unwrap();
        assert_eq!(word(&ra, OWNER_INDICES_OFFSET), 137);
        assert_eq!(state.borrow().indices(1).unwrap(), b_indices);
        assert!(
            state
                .borrow()
                .pool()
                .submission_slots()
                .any(|(i, _)| i == 138)
        );
        assert!(Rc::ptr_eq(
            geometry.registrar().tree_state.as_ref().unwrap(),
            &state
        ));
        release_record_with_trees(&mut rb, Some(&state)).unwrap();
        release_record_with_trees(&mut ra, Some(&state)).unwrap();
        assert_eq!(state.borrow().pool().count(), 137);
        let registry = Models::with_tree_state(Some(state.clone()));
        assert!(Rc::ptr_eq(registry.tree_state.as_ref().unwrap(), &state));
        eprintln!(
            "Original landscape236 tree binding, offsets, shared ownership, ordered cleanup and first-hole reuse verified; no renderer/native acceptance"
        );
    }
}
