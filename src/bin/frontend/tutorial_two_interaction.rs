//! Script-driven prompt position and 3D pick admission for Tutorial 2.
//!
//! The supplied TUTORIAL2/hub.lua requests these operations. The script does
//! not specify the renderer's hit-test details, so callers must pass only a
//! committed navigation pick from the existing scene selection path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// A single block selection in Strategic View or on block pavement.
    Block { category_id: u32 },
    /// A double-click that enters the selected block from Strategic View.
    BlockEntry { category_id: u32 },
    Building { slot: usize },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct State {
    offset: [f32; 2],
    gate: Gate,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Gate {
    #[default]
    Disabled,
    Block(Option<u32>),
    Building(Option<usize>),
}

impl State {
    /// 4db740 independently stores numeric x/y values on the prompt widget.
    /// Missing or nonnumeric arguments leave that axis unchanged.
    pub fn position_prompt_parts(&mut self, x: Option<f32>, y: Option<f32>) {
        if let Some(x) = x {
            self.offset[0] = x;
        }
        if let Some(y) = y {
            self.offset[1] = y;
        }
    }

    pub fn prompt_offset(&self) -> [f32; 2] {
        self.offset
    }

    pub fn has_pick_gate(&self) -> bool {
        !matches!(self.gate, Gate::Disabled)
    }

    pub fn positioned_point(&self, point: [f32; 2]) -> [f32; 2] {
        [point[0] + self.offset[0], point[1] + self.offset[1]]
    }

    pub fn disable_all_picking(&mut self) {
        self.gate = Gate::Disabled;
    }

    pub fn enable_block_picking(&mut self, category_id: Option<u32>) {
        // 4db970 clears the building gate when it enables block picks.
        self.gate = Gate::Block(category_id);
    }

    /// The original script also uses an optional building slot in later
    /// tutorials. Tutorial 2 passes no argument, meaning any building.
    pub fn enable_building_picking(&mut self, slot: Option<usize>) {
        // 4db9e0 clears the block gate. Missing argument stores -1 (any).
        self.gate = Gate::Building(slot);
    }

    /// Native 4c3929/4c3979 dispatch Picked3D(0) for a single block
    /// selection, 4c3beb/4c3cf6 dispatch 1 for block entry, and 4c3958
    /// dispatches 2 for a single building selection. Those calls target
    /// 4d5fb0; the nearby 41f290 values instead select the main-panel mode.
    pub fn callback(&self, pick: Pick) -> Option<i32> {
        match (&self.gate, pick) {
            // 43deeb compares the selected category directly with the stored
            // block id; a missing argument stores -1 and admits no category.
            (Gate::Block(None), Pick::Block { .. }) => None,
            (Gate::Block(Some(expected)), Pick::Block { category_id })
                if *expected == category_id =>
            {
                Some(0)
            }
            (Gate::Block(Some(expected)), Pick::BlockEntry { category_id })
                if *expected == category_id =>
            {
                Some(1)
            }
            (Gate::Building(None), Pick::Building { .. }) => Some(2),
            (Gate::Building(Some(expected)), Pick::Building { slot }) if *expected == slot => {
                Some(2)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_two_position_and_pick_sequence() {
        let mut state = State::default();
        assert_eq!(state.prompt_offset(), [0., 0.]);
        assert_eq!(state.callback(Pick::Block { category_id: 8 }), None);
        state.position_prompt_parts(Some(0.), Some(150.));
        state.enable_block_picking(Some(8)); // Indiana Avenue in the original board.
        assert_eq!(state.positioned_point([320., 240.]), [320., 390.]);
        assert_eq!(state.callback(Pick::Block { category_id: 8 }), Some(0));
        assert_eq!(state.callback(Pick::BlockEntry { category_id: 8 }), Some(1));
        assert_eq!(state.callback(Pick::Block { category_id: 7 }), None);
        assert_eq!(state.callback(Pick::Building { slot: 0 }), None);
        state.disable_all_picking();
        state.position_prompt_parts(Some(0.), Some(0.));
        state.position_prompt_parts(Some(0.), Some(-100.));
        state.enable_building_picking(None);
        assert_eq!(state.positioned_point([320., 240.]), [320., 140.]);
        assert_eq!(state.callback(Pick::Building { slot: 0 }), Some(2));
        assert_eq!(state.callback(Pick::Building { slot: 8 }), Some(2));
        assert_eq!(state.callback(Pick::Block { category_id: 8 }), None);
        state.disable_all_picking();
        state.enable_block_picking(Some(8));
        assert_eq!(state.callback(Pick::Block { category_id: 8 }), Some(0));
        assert_eq!(state.callback(Pick::BlockEntry { category_id: 8 }), Some(1));
        assert_eq!(state.callback(Pick::Building { slot: 0 }), None);
        state.disable_all_picking();
        state.enable_building_picking(Some(8));
        assert_eq!(state.callback(Pick::Building { slot: 8 }), Some(2));
        assert_eq!(state.callback(Pick::Building { slot: 7 }), None);
        state.enable_block_picking(None);
        assert_eq!(state.callback(Pick::Block { category_id: 8 }), None);
    }

    #[test]
    #[ignore = "requires the supplied original Tutorial 2 script"]
    fn original_tutorial_two_script_requests_match_policy() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Original Distribution/Monopoly Tycoon ISO 2001 Archive.org/Extracted Installer/scripts/TUTORIAL2/hub.lua");
        let script = std::fs::read_to_string(path).expect("supplied Tutorial 2 hub.lua");
        for call in [
            "Interface.PositionPrompt(0, 150)",
            "Interface.PositionPrompt(0, 0)",
            "Interface.PositionPrompt(0,-100)",
            "Interface.EnableBlockPicking(INDIANA_AVENUE)",
            "Interface.EnableBuildingPicking()",
            "if a == 1 and g_currentprompt == 5 then",
        ] {
            assert!(script.contains(call), "original Tutorial 2 changed: {call}");
        }
        assert_eq!(
            script
                .matches("Interface.EnableBlockPicking(INDIANA_AVENUE)")
                .count(),
            2
        );
    }

    #[test]
    #[ignore = "requires original v1.4 mc.exe"]
    fn original_native_picked_3d_callback_codes() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = openmt2001::pe::PeImage::parse(&bytes).unwrap();
        assert_eq!(image.at(0x54cd98, 9).unwrap(), b"Picked3D\0");
        let table = |address: u32, count: usize| image.at(address, count * 4).unwrap()
            .chunks_exact(4).map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect::<Vec<_>>();
        // Native class4 categories select 4c391f on a single click and
        // 4c3bdd on a double click; each tests strategic interface mode3.
        assert_eq!(table(0x4c39ac, 4),
            vec![0x4c38f5, 0x4c38f5, 0x4c38f5, 0x4c391f]);
        assert_eq!(table(0x4c3d28, 4),
            vec![0x4c3bbd, 0x4c3bdd, 0x4c3d25, 0x4c3d25]);
        assert_eq!(image.at(0x4c3d38, 9).unwrap(), &[0, 0, 3, 1, 3, 3, 3, 3, 2]);
        assert_eq!(image.at(0x4c3924, 3).unwrap(), &[0x83, 0xf8, 3]);
        assert_eq!(image.at(0x4c3be2, 3).unwrap(), &[0x83, 0xf8, 3]);
        // Verify the event name and invoker as well as the argument; adjacent
        // 41f290 panel-mode arguments are not Picked3D callback evidence.
        for (address, code) in [(0x4c3929, 0), (0x4c3979, 0),
            (0x4c3beb, 1), (0x4c3cf6, 1), (0x4c3958, 2), (0x4c3c82, 3)] {
            assert_eq!(image.at(address, 2).unwrap(), &[0x6a, code]);
            assert_eq!(image.at(address + 2, 1).unwrap(), &[0x68]);
            assert_eq!(u32::from_le_bytes(image.at(address + 3, 4).unwrap()
                .try_into().unwrap()), 0x54cd98);
            assert_eq!(image.at(address + 12, 1).unwrap(), &[0xe8]);
            let displacement = i32::from_le_bytes(image.at(address + 13, 4).unwrap()
                .try_into().unwrap());
            assert_eq!(address.wrapping_add(17).wrapping_add(displacement as u32), 0x4d5fb0);
        }
    }
}
