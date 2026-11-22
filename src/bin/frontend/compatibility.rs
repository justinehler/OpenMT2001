//! Explicit product policy for documented original defects, separate from the
//! original-backed simulation rules.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Modern,
    StrictV14,
}

impl Mode {
    /// Intentional modern departure for Tutorial5's documented v1.4 blocker:
    /// https://www.gamepressure.com/download/monopoly-tycoon-14-patch/z3135e5
    /// Preserve authored rights only across the fresh ownership reset. This is
    /// not a claim of full native equivalence. Tutorial8 remains excluded until
    /// separately proven; saves and strict evidence flows retain native bytes.
    pub fn repair_startup_rights(
        self,
        id: u32,
        load_mode: bool,
        before: &[(u8, u32); 52],
        after: &mut [[u8; 380]; 52],
    ) -> usize {
        if self != Self::Modern || load_mode || id != 100004 {
            return 0;
        }
        let mut repaired = 0;
        for (&(owner, rights), row) in before.iter().zip(after.iter_mut()) {
            if owner != 0 && row[0x05] == owner && rights != 0
                && row[0x78..0x7c] == [0; 4]
            {
                row[0x78..0x7c].copy_from_slice(&rights.to_le_bytes());
                repaired += 1;
            }
        }
        repaired
    }
}

#[cfg(test)]
mod tests {
    use super::Mode;

    fn fixture() -> ([(u8, u32); 52], [[u8; 380]; 52]) {
        let before = std::array::from_fn(|i| ((i % 6 + 1) as u8, 0x12345600 + i as u32));
        let mut after = std::array::from_fn(|i| std::array::from_fn(|j| (i + j) as u8));
        for (row, &(owner, _)) in after.iter_mut().zip(&before) {
            row[5] = owner;
            row[0x78..0x7c].fill(0);
        }
        (before, after)
    }

    #[test]
    fn restores_exact_authored_values_and_only_rights_bytes() {
        assert_eq!(Mode::default(), Mode::Modern);
        let (before, mut after) = fixture();
        let native = after;
        assert_eq!(Mode::Modern.repair_startup_rights(100004, false, &before, &mut after), 52);
        for i in 0..52 {
            let mut expected = native[i];
            expected[0x78..0x7c].copy_from_slice(&before[i].1.to_le_bytes());
            assert_eq!(after[i], expected);
        }
    }

    #[test]
    fn leaves_native_results_exact_for_strict_load_and_other_levels() {
        let (before, native) = fixture();
        for mode in [Mode::Modern, Mode::StrictV14] {
            for load in [false, true] {
                for id in [0, 100000, 100003, 100004, 100005, 100008, 200001, 400001] {
                    if mode == Mode::Modern && !load && id == 100004 { continue; }
                    let mut after = native;
                    assert_eq!(mode.repair_startup_rights(id, load, &before, &mut after), 0);
                    assert_eq!(after, native);
                }
            }
        }
    }

    #[test]
    fn rejects_changed_or_zero_owners_zero_authored_and_existing_rights() {
        let (mut before, mut after) = fixture();
        after[0][5] = before[0].0 + 1;
        before[1].0 = 0;
        after[1][5] = 0;
        before[2].1 = 0;
        after[3][0x78..0x7c].copy_from_slice(&0xabcdef01u32.to_le_bytes());
        let native = after;
        assert_eq!(Mode::Modern.repair_startup_rights(100004, false, &before, &mut after), 48);
        assert_eq!(&after[..4], &native[..4]);
        // Applying the policy twice cannot overwrite established rights.
        let repaired = after;
        assert_eq!(Mode::Modern.repair_startup_rights(100004, false, &before, &mut after), 0);
        assert_eq!(after, repaired);
    }
}
