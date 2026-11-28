use super::fresh_session_option_word;
use openmt2001::save::{SaveFile, writer};

fn write(session_option_word: u32, logic_mode: u32) -> Vec<u8> {
    let mut categories = [[0; 380]; 52];
    let mut state = writer::State {
        compatibility: &[0; 40],
        level: 200001,
        difficulty: 0,
        current_cash: 0.,
        players: &[],
        businesses: &[],
        buildings: &[],
        building_base: 0,
        building_sentinel_next: 0,
        categories: &mut categories,
        residents: &[],
        cameras: &[[0; 196]; 2],
        calendar: [0; 5],
        timing: [0, 0, 0, 0, 0, logic_mode],
        pending: &[0; 3588],
        resident_accounts: &[],
        scheduled: &[],
        auction: None,
        objective: &[0; 60],
        business_info: &[[0; 116]; 45],
        commodities: &[[0; 28]; 32],
        history: &[0; 0x10f44],
        session_option_word,
    };
    writer::encode(&mut state, 0x400600, [0; 4]).unwrap()
}

#[test]
fn session_save_loaded_option_word_survives_logic_mode_changes() {
    // An independently framed synthetic native save, including unknown raw words.
    for option in [0u32, 1, 2, 3, 0xdeadbeef] {
        let mut original = Vec::new();
        for value in [0x400000u32, 4, option, u32::MAX, 0] {
            original.extend_from_slice(&value.to_le_bytes());
        }
        let loaded = SaveFile::parse(&original)
            .unwrap()
            .session_option_word()
            .unwrap();
        for logic in [0, 1, 2] {
            let bytes = write(loaded, logic);
            let saved = SaveFile::parse(&bytes).unwrap();
            assert_eq!(saved.session_option_word().unwrap(), option);
            assert_eq!(saved.clock().unwrap().timing[5], logic);
        }
    }
}

#[test]
fn session_save_fresh_start_option_words_match_original_routes() {
    for (level, custom, skirmish, network, expected) in [
        (200001, false, false, false, 0), // Campaign: 499189.
        (100000, false, false, false, 0), // Tutorial: frontend reset 4956c5.
        (100012, false, false, false, 0),
        (400002, false, false, false, 2), // Sandbox: 4991d2.
        (300001, false, true, false, 1),  // Offline Skirmish: 4991a5.
        (500001, true, false, false, 3),  // Custom: 4991ee.
        (300001, false, true, true, 0),   // Network setup: 4a5c56.
    ] {
        let option = fresh_session_option_word(level, custom, skirmish, network);
        let bytes = write(option, 1);
        let saved = SaveFile::parse(&bytes).unwrap();
        assert_eq!(
            saved.session_option_word().unwrap(),
            expected,
            "level {level}"
        );
        assert_eq!(saved.clock().unwrap().timing[5], 1);
    }
}
