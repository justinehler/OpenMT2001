//! Building-owned tree descriptors and persistent first-free slot ownership.
//! Rendering consumes this state; snapshots must never allocate from it.
use super::{CAPACITY, Pool, Slot};
use crate::{
    build_file::{BuildFile, ModelMatch},
    x87::Sum,
};
use std::collections::BTreeMap;

pub const OWNER_LIMIT: usize = 600;
pub const TREES_PER_BUILDING: usize = 8;
/// Full-building offsets: native485200/47e450 receive the B+4c subobject.
pub const OWNER_COUNT_OFFSET: usize = 0x368;
pub const OWNER_INDICES_OFFSET: usize = 0x36c;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalTree {
    pub detail_slot: u8,
    pub descriptor: u32,
    /// Pool-local coordinates before488230's tree tail, not mesh coordinates.
    pub position: [f32; 3],
}

pub fn is_tree_descriptor(descriptor: u32) -> bool {
    matches!(
        descriptor,
        0x0200_0004 | 0x0200_0046 | 0x46 | 0x0200_0047 | 0x47 | 0x0200_0048 | 0x48
    )
}

///485200 visits32 expanded rows in order, admitting the first8 tree rows.
/// Bare04 is intentionally not part of the recovered tree branch.
pub fn locals(
    file: &BuildFile<'_>,
    selected: ModelMatch,
    height: u8,
) -> Result<Vec<LocalTree>, String> {
    let layers = file.profile(selected.index, height)?;
    let model = file
        .models
        .chunks_exact(28)
        .nth(selected.index)
        .ok_or("tree model leaves BuildFile")?;
    let word = |bytes: &[u8], at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let first = word(model, 8) as usize;
    let count = (word(model, 12) as usize).min(32);
    let mut result = Vec::new();
    for detail_slot in 0..count {
        let index = first
            .checked_add(detail_slot)
            .ok_or("tree detail index overflow")?;
        let row = file
            .details
            .chunks_exact(32)
            .nth(index)
            .ok_or("tree detail leaves BuildFile")?;
        let descriptor = word(row, 24);
        if !is_tree_descriptor(descriptor) || result.len() == TREES_PER_BUILDING {
            continue;
        }
        let layer = layers
            .get(word(row, 28) as usize)
            .ok_or("tree level leaves expanded profile")?;
        let f = |at| f32::from_bits(word(row, at));
        let position = [
            (f64::from(f(0)) * 10.) as f32,
            (f64::from(layer.base) + f64::from(f(4))) as f32,
            (f64::from(f(8)) * -10.) as f32,
        ];
        Slot {
            position,
            scale: [0.75; 3],
        }
        .validate()?;
        result.push(LocalTree {
            detail_slot: detail_slot as u8,
            descriptor,
            position,
        });
    }
    Ok(result)
}

/// Inputs selected by488230. Canonical size follows SceneBuilding [width,depth];
/// native stack+30/+24 hold those dimensions in the opposite order.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub canonical_size: [u8; 2],
    pub transposed: bool,
    pub mirror: bool,
    pub angle: f32,
    pub center: [f32; 2],
}

impl Pose {
    pub fn from_scene(building: &crate::scene::SceneBuilding) -> Self {
        Self {
            canonical_size: building.canonical_size,
            transposed: building.selected.transposed,
            mirror: building.mirror,
            angle: building.angle,
            center: building.center,
        }
    }
    ///488230 tree tail: cosine spills toF32, sine remains wide; X spills
    /// before center addition, while Z's wide value survives its local store.
    /// Host libm approximates native FSIN/FCOS; universal bit parity is unproved.
    pub fn place(self, local: LocalTree) -> Result<Slot, String> {
        if !self.angle.is_finite()
            || !self.center.into_iter().all(f32::is_finite)
            || !self
                .canonical_size
                .into_iter()
                .all(|n| (1..=7).contains(&n))
        {
            return Err("invalid building tree pose".into());
        }
        Slot {
            position: local.position,
            scale: [0.75; 3],
        }
        .validate()?;
        let [mut width, mut depth] = self.canonical_size.map(f32::from);
        if self.transposed {
            std::mem::swap(&mut width, &mut depth);
        }
        let u = (f64::from(local.position[0]) - f64::from(width) * 5.) as f32;
        let mut v = (f64::from(local.position[2])
            + (7. - f64::from(depth)) * 10.
            + f64::from(depth) * 5.) as f32;
        if self.mirror {
            v = -v;
        }
        let radians = f64::from(self.angle) * f64::from(f32::from_bits(0x3c8efa35));
        let sine = Sum::from_f64(radians.sin())?;
        let cosine = radians.cos() as f32;
        let mut x = Sum::from_f32(u)?;
        x.mul_f32(cosine)?;
        let mut vs = sine;
        vs.mul_f32(v)?;
        x.add(vs);
        let x = (f64::from(x.to_f32()) + f64::from(self.center[0])) as f32;
        let mut z = Sum::from_f32(v)?;
        z.mul_f32(cosine)?;
        let mut us = sine;
        us.mul_f32(u)?;
        z.add(us.negated());
        z.add(Sum::from_f32(self.center[1])?);
        let result = Slot {
            position: [
                x,
                (f64::from(local.position[1]) + 0.5) as f32,
                z.negated().to_f32(),
            ],
            scale: [0.75; 3],
        };
        result.validate()?;
        Ok(result)
    }
}

/// World-lifetime ownership. Allocation order belongs to the authoritative
/// building lifecycle, never map-key sorting or the visible/snapshot list.
#[derive(Clone)]
pub struct OwnedPool {
    pool: Pool,
    owners: BTreeMap<usize, Vec<usize>>,
    revision: u64,
}
impl OwnedPool {
    pub fn ambient(locators: &[[f32; 3]]) -> Result<Self, String> {
        Ok(Self {
            pool: Pool::ambient(locators)?,
            owners: BTreeMap::new(),
            revision: 0,
        })
    }
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn indices(&self, owner: usize) -> Option<&[usize]> {
        self.owners.get(&owner).map(Vec::as_slice)
    }
    /// Validate capacity/poses before allocating, so errors cannot leak a
    /// partial owner. Native allocation-failure memory behavior is not emulated.
    pub fn bind(&mut self, owner: usize, poses: &[Slot]) -> Result<&[usize], String> {
        if owner >= OWNER_LIMIT || poses.len() > TREES_PER_BUILDING {
            return Err("building tree ownership exceeds native limits".into());
        }
        if self.owners.contains_key(&owner) {
            return Err("building trees require explicit release before rebind".into());
        }
        for pose in poses {
            pose.validate()?;
        }
        if poses.len() > CAPACITY - self.pool.count() {
            return Err("tree pool exhausted".into());
        }
        let mut indices = Vec::with_capacity(poses.len());
        for &pose in poses {
            indices.push(self.pool.allocate(pose)?);
        }
        self.owners.insert(owner, indices);
        self.revision = self.revision.wrapping_add(1);
        Ok(self.indices(owner).unwrap())
    }
    /// Pose refresh mutates existing rows without changing phase/variation IDs.
    pub fn update(&mut self, owner: usize, poses: &[Slot]) -> Result<(), String> {
        let indices = self
            .owners
            .get(&owner)
            .ok_or("missing building tree owner")?;
        if indices.len() != poses.len() {
            return Err("tree pose refresh changed ownership count".into());
        }
        for pose in poses {
            pose.validate()?;
        }
        if indices.iter().any(|&i| self.pool.slot(i).is_none()) {
            return Err("building tree ownership points to inactive slot".into());
        }
        let changed = indices.iter().zip(poses).any(|(&i, p)| {
            let old = self.pool.slot(i).unwrap();
            old.position.map(f32::to_bits) != p.position.map(f32::to_bits)
                || old.scale.map(f32::to_bits) != p.scale.map(f32::to_bits)
        });
        for (&index, &pose) in indices.iter().zip(poses) {
            self.pool.set_pose(index, pose)?;
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }
    ///47e450 retires the owner's listed rows in order, one callback at a time.
    pub fn release_next(&mut self, owner: usize, index: usize) -> Result<(), String> {
        let indices = self
            .owners
            .get_mut(&owner)
            .ok_or("missing building tree owner")?;
        if indices.first().copied() != Some(index) {
            return Err("building tree release order/identity mismatch".into());
        }
        self.pool.release(index)?;
        indices.remove(0);
        if indices.is_empty() {
            self.owners.remove(&owner);
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Zero-count cleanup is harmless; each owned row is retired exactly once.
    pub fn release(&mut self, owner: usize) -> Result<usize, String> {
        let Some(indices) = self.owners.get(&owner) else {
            return Ok(0);
        };
        if indices.iter().any(|&i| self.pool.slot(i).is_none()) {
            return Err("building tree release points to inactive slot".into());
        }
        let indices = self.owners.remove(&owner).unwrap();
        for &index in &indices {
            self.pool.release(index)?;
        }
        self.revision = self.revision.wrapping_add(1);
        Ok(indices.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree(x: f32) -> Slot {
        Slot {
            position: [x, 2., 3.],
            scale: [0.75; 3],
        }
    }
    #[test]
    fn stable_ownership_holes_pose_and_reuse() {
        let mut state = OwnedPool::ambient(&[[0.; 3]; 137]).unwrap();
        assert_eq!(state.bind(30, &[tree(1.), tree(2.)]).unwrap(), [137, 138]);
        assert_eq!(state.bind(2, &[tree(3.)]).unwrap(), [139]);
        let revision = state.revision();
        state.update(2, &[tree(3.)]).unwrap();
        assert_eq!(state.revision(), revision);
        state.update(2, &[tree(9.)]).unwrap();
        assert_eq!(state.indices(2).unwrap(), [139]);
        assert_eq!(state.pool().slot(139).unwrap().position[0], 9.);
        assert!(state.bind(2, &[tree(5.)]).is_err());
        assert_eq!(state.release(30).unwrap(), 2);
        assert_eq!(state.release(30).unwrap(), 0);
        assert_eq!(state.pool().count(), 138);
        assert!(!state.pool().submission_slots().any(|(i, _)| i == 139));
        assert_eq!(state.bind(7, &[tree(7.)]).unwrap(), [137]);
        assert_eq!(state.indices(2).unwrap(), [139]);
        assert_eq!(state.bind(30, &[tree(8.)]).unwrap(), [138]);
        assert!(state.pool().submission_slots().any(|(i, _)| i == 139));
        assert_eq!(state.pool().slot(0).unwrap().scale, [1.; 3]);
    }
    #[test]
    fn rejection_is_atomic_and_owner_offsets_are_full_record_offsets() {
        let mut state = OwnedPool::ambient(&[[0.; 3]; 499]).unwrap();
        assert!(state.bind(0, &[tree(0.), tree(1.)]).is_err());
        assert_eq!(state.pool().count(), 499);
        assert_eq!(state.revision(), 0);
        assert!(state.bind(0, &[tree(f32::NAN)]).is_err());
        assert!(state.bind(600, &[]).is_err());
        assert!(state.bind(0, &[tree(0.); 9]).is_err());
        assert_eq!(state.bind(0, &[tree(0.)]).unwrap(), [499]);
        let revision = state.revision();
        assert!(state.update(0, &[tree(f32::NAN)]).is_err());
        assert_eq!(state.revision(), revision);
        assert_eq!(state.pool().slot(499).unwrap(), &tree(0.));
        assert_eq!(
            (OWNER_COUNT_OFFSET, OWNER_INDICES_OFFSET),
            (0x4c + 0x31c, 0x4c + 0x320)
        );
    }
    #[test]
    fn native_pose_axis_signs_transpose_mirror_and_quarter_turn() {
        let local = LocalTree {
            detail_slot: 0,
            descriptor: 0x02000004,
            position: [10., 3., -20.],
        };
        let pose = Pose {
            canonical_size: [2, 3],
            transposed: false,
            mirror: false,
            angle: 0.,
            center: [100., 200.],
        };
        assert_eq!(
            pose.place(local).unwrap(),
            Slot {
                position: [100., 3.5, -235.],
                scale: [0.75; 3]
            }
        );
        assert_eq!(
            Pose {
                mirror: true,
                ..pose
            }
            .place(local)
            .unwrap()
            .position,
            [100., 3.5, -165.]
        );
        assert_eq!(
            Pose {
                transposed: true,
                ..pose
            }
            .place(local)
            .unwrap()
            .position,
            [95., 3.5, -240.]
        );
        for (angle, expected) in [
            (90., [135., 3.5, -200.]),
            (180., [100., 3.5, -165.]),
            (270., [65., 3.5, -200.]),
        ] {
            let actual = Pose { angle, ..pose }.place(local).unwrap();
            for (a, b) in actual.position.into_iter().zip(expected) {
                assert!((a - b).abs() < 0.0001, "angle{angle}: {actual:?}");
            }
        }
        assert!(
            Pose {
                angle: f32::NAN,
                ..pose
            }
            .place(local)
            .is_err()
        );
        assert!(!is_tree_descriptor(4));
        assert!(!is_tree_descriptor(0x02000045));
        for d in [
            0x02000004, 0x02000046, 0x46, 0x02000047, 0x47, 0x02000048, 0x48,
        ] {
            assert!(is_tree_descriptor(d));
        }
    }
    #[test]
    #[ignore = "requires player-supplied original BuildFile"]
    fn original_ordered_descriptors_and_local_pose() {
        let bytes = std::fs::read(std::env::var_os("OPENMT2001_BUILD_FILE").unwrap()).unwrap();
        let file = BuildFile::parse(&bytes).unwrap();
        let mut bearing = 0;
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        for (index, model) in file.models.chunks_exact(28).enumerate() {
            let selected = ModelMatch {
                index,
                transposed: false,
            };
            let trees = locals(&file, selected, 4).unwrap();
            let start = word(model, 8) as usize;
            let count = (word(model, 12) as usize).min(32);
            let expected: Vec<_> = (0..count)
                .filter_map(|i| {
                    let r = &file.details[(start + i) * 32..(start + i + 1) * 32];
                    is_tree_descriptor(word(r, 24)).then_some((i, r))
                })
                .take(8)
                .collect();
            assert_eq!(trees.len(), expected.len());
            let layers = file.profile(index, 4).unwrap();
            for (tree, (slot, row)) in trees.iter().zip(expected) {
                assert_eq!(tree.detail_slot as usize, slot);
                assert_eq!(tree.descriptor, word(row, 24));
                assert_eq!(
                    tree.position.map(f32::to_bits),
                    [
                        ((f64::from(f32::from_bits(word(row, 0)))) * 10.) as f32,
                        (f64::from(layers[word(row, 28) as usize].base)
                            + f64::from(f32::from_bits(word(row, 4))))
                            as f32,
                        (f64::from(f32::from_bits(word(row, 8))) * -10.) as f32
                    ]
                    .map(f32::to_bits)
                );
            }
            bearing += usize::from(!trees.is_empty());
        }
        assert_eq!(bearing, 71, "original descriptor-bearing models");
    }
}
