//! Original players.lua execution and RegisterPlayer callback values.
//! The host applies these ordered registrations to its player/model manager;
//! this does not replace the later character-selection or constructor paths.
use crate::lua4::{Runtime, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registration {
    pub index: i32,
    pub kind: i32,
    pub character: i32,
    pub difficulty: i32,
}

pub struct PendingRoster {
    pub records: [Registration; 6],
    /// Original Frontend+227c mirror retains script choices across human edits.
    pub script_characters: [i32; 6],
}

/// Original48cca0 globals updated by its normal roster path.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct RosterStatus {
    pub current_player: u32,
    pub ai_count: u32,
    pub has_ai: bool,
}

pub struct RosterInputs<'a> {
    pub names: [&'a [u8]; 6],
    /// Live InitialValue.playermoney after the level initializer has run.
    /// Original48c712 reads mutable541c78, not the executable's default bytes.
    pub initial_cash: f32,
    pub default_ai: &'a [u8],
    pub scenario_ai: Option<(&'a str, &'a [u8])>,
}

impl Default for PendingRoster {
    fn default() -> Self {
        Self {
            records: std::array::from_fn(|index| Registration {
                index: index as i32,
                kind: -1,
                character: -1,
                difficulty: 0,
            }),
            script_characters: [0; 6],
        }
    }
}

impl PendingRoster {
    /// Join the normal48cca0 roster stage to original construction and4d7a80.
    /// The required world callback performs467a30 ownership maintenance and
    /// conditional477be0 after each kind1 player's466870 reset. This stage
    /// follows the level initializer (map, preferences and population) and does
    /// not start a game clock.
    pub fn initialize_created<const N: usize>(
        &self,
        inputs: RosterInputs<'_>,
        construction: &mut crate::player_constructor::RosterConstruction<'_>,
        state: &mut crate::ai_script::Initialization<'_, N>,
        scripts: &mut crate::ai_script::AiScript,
        status: &mut RosterStatus,
        after_reset: &mut impl FnMut(&mut crate::player_manager::Manager, u32) -> Result<(), String>,
    ) -> Result<(), String> {
        use crate::player_manager::{Creation, word};
        if word(&state.manager.bytes, 8) != 0 {
            status.current_player = word(&state.manager.bytes, 0x10);
            return Ok(());
        }
        status.ai_count = 0;
        status.has_ai = false;
        for (slot, record) in self.records.iter().enumerate() {
            state.manager.register(
                state.profiles,
                Creation {
                    kind: record.kind,
                    character: record.character,
                    name: inputs.names[slot],
                    initial_cash: inputs.initial_cash,
                },
                &mut |manager, kind| construction.construct(manager, kind),
            )?;
            if record.kind == 1 {
                status.ai_count = status.ai_count.wrapping_add(1);
                status.has_ai = true;
            }
        }
        status.current_player = word(&state.manager.bytes, 0x10);
        for (slot, record) in self.records.iter().enumerate() {
            if record.kind != 1 {
                continue;
            }
            let reference = word(&state.manager.bytes, 0x10 + slot * 4);
            let player = state
                .manager
                .players
                .iter_mut()
                .find(|player| player.reference == reference)
                .ok_or("AI registration slot has no player")?;
            player.reset_ai(construction.image, construction.global_reset, &mut |_| {
                Err("new player owns unexpected prior list allocation".into())
            })?;
            after_reset(state.manager, reference)?;
        }
        if status.has_ai {
            scripts.initialize(state, inputs.default_ai, inputs.scenario_ai)?;
        }
        //48cdc8: the per-roster difficulty overrides the global difficulty only
        //after4d7a80's events and initial count have used the global setting.
        for (slot, record) in self.records.iter().enumerate() {
            if record.kind == 1 {
                let reference = word(&state.manager.bytes, 0x10 + slot * 4);
                let player = state
                    .manager
                    .players
                    .iter_mut()
                    .find(|player| player.reference == reference)
                    .ok_or("AI difficulty slot has no player")?;
                crate::player_manager::put(&mut player.bytes, 0x13f4, record.difficulty as u32);
            }
        }
        Ok(())
    }

    /// Normal single-player4a8420: reset kind/character, evaluate scenario,
    /// override human selection, then resolve duplicates. No defaultai fallback.
    pub fn load_normal(
        &mut self,
        name: &str,
        source: &[u8],
        difficulty: i32,
        selected_character: i32,
    ) -> Result<(), String> {
        for record in &mut self.records {
            record.kind = -1;
            record.character = -1;
        }
        for registration in evaluate(name, source, difficulty)? {
            let index =
                usize::try_from(registration.index).map_err(|_| "negative registration slot")?;
            *self
                .records
                .get_mut(index)
                .ok_or("registration exceeds six slots")? = registration;
            self.script_characters[index] = registration.character;
        }
        self.records[0].kind = 0;
        self.records[0].character = selected_character;
        self.resolve_tokens(&mut |_, _| {})
    }

    /// Original4a8610 restores each active nonhuman-slot script choice first.
    /// A collision advances modulo10 until no current record has that token,
    /// including records later in the scan and inactive records' retained values.
    pub fn resolve_tokens(&mut self, changed: &mut impl FnMut(usize, i32)) -> Result<(), String> {
        let mut used = [false; 10];
        let index = |value: i32| {
            usize::try_from(value)
                .ok()
                .filter(|i| *i < 10)
                .ok_or("character exceeds ten tokens")
        };
        used[index(self.records[0].character)?] = true;
        for slot in 1..6 {
            if self.records[slot].kind == -1 {
                continue;
            }
            let original = self.script_characters[slot];
            index(original)?;
            self.records[slot].character = original;
            changed(slot, original);
            if used[index(original)?] {
                let mut candidate = original;
                loop {
                    candidate = (candidate + 1) % 10;
                    if !self
                        .records
                        .iter()
                        .any(|record| record.character == candidate)
                    {
                        break;
                    }
                }
                self.records[slot].character = candidate;
                changed(slot, candidate);
            }
            used[index(self.records[slot].character)?] = true;
        }
        Ok(())
    }
}

impl Registration {
    /// Original4dbbc0: FTOL conversion and optional fourth numeric argument.
    pub fn from_numbers(arguments: &[f64]) -> Self {
        let integer = |index| {
            arguments
                .get(index)
                .map_or(0, |value| crate::economy::ftol_low_word(*value))
        };
        Self {
            index: integer(0),
            kind: integer(1),
            character: integer(2),
            difficulty: integer(3),
        }
    }
}

/// Evaluate the supplied complete script, including its difficulty branches.
/// Original4dac20 returns signed game difficulty86d33c as a Lua number.
pub fn evaluate(name: &str, source: &[u8], difficulty: i32) -> Result<Vec<Registration>, String> {
    let mut runtime = Runtime::new()?;
    for (index, name) in crate::ai_parameters::CHARACTER_NAMES.iter().enumerate() {
        runtime.set_number(name, index as f64)?;
    }
    for (name, value) in [
        ("HUMAN_PLAYER", 0),
        ("AI_PLAYER", 1),
        ("DIFFICULTY_EASY", 0),
        ("DIFFICULTY_MEDIUM", 1),
        ("DIFFICULTY_HARD", 2),
    ] {
        runtime.set_number(name, f64::from(value))?;
    }
    runtime.register("openmt_register_player", 0)?;
    runtime.register("openmt_game_difficulty", 1)?;
    runtime.execute("player bindings",b"Frontend={RegisterPlayer=openmt_register_player}; Game={GetDifficulty=openmt_game_difficulty}",&mut |_,_|Err("unexpected player binding initialization".into()))?;
    let mut registrations = Vec::new();
    runtime.execute(name, source, &mut |id, args| match id {
        0 => {
            let numbers = args
                .iter()
                .map(|value| match value {
                    Value::Number(n) => Ok(*n),
                    Value::Nil => Ok(0.),
                    _ => Err("RegisterPlayer string conversion is not implemented"),
                })
                .collect::<Result<Vec<_>, _>>()?;
            registrations.push(Registration::from_numbers(&numbers));
            Ok(None)
        }
        1 => Ok(Some(f64::from(difficulty))),
        _ => Err("unknown player binding".into()),
    })?;
    Ok(registrations)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires supplied v1.4 image and complete scenario scripts"]
    fn constructs_original_rosters_and_initializes_ai() {
        use crate::player_constructor::{Allocation, RosterConstruction};
        use crate::player_manager::{Manager, RandomRegistry, word};
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let ai_path = std::path::PathBuf::from(std::env::var("OPENMT2001_AI_SCRIPT").unwrap());
        let root = ai_path.parent().unwrap().parent().unwrap();
        let default_ai = std::fs::read(&ai_path).unwrap();
        let mapping = crate::ai_ownership::group_mapping(&image).unwrap();
        for scenario in ["SCENARIO3", "SCENARIO1", "defaultai", "sandbox"] {
            for difficulty in 0..3 {
                // Explicit host input; this test does not execute initialpref.
                // These values also match SCENARIO3's difficulty branches.
                let initial_cash = [15000., 12000., 10000.][difficulty as usize];
                let mut roster = PendingRoster::default();
                roster
                    .load_normal(
                        scenario,
                        &std::fs::read(root.join(scenario).join("players.lua")).unwrap(),
                        difficulty,
                        6,
                    )
                    .unwrap();
                let mut manager = Manager::initialize([0xa5; 0x28]);
                let mut profiles = vec![[0; 0x650]; 10];
                let mut rng = RandomRegistry {
                    process: [0x12345678, 0x343fd, 0x269ec3],
                    head: 0,
                    tail: 0,
                };
                let mut reset = 255;
                let mut next = 0x10010000_u32;
                let mut allocations = 0;
                let mut allocate = |size: usize| {
                    let reference = next;
                    next += (size as u32 + 0xfff) & !0xfff;
                    allocations += 1;
                    Ok(Some(Allocation {
                        reference,
                        bytes: vec![0xa5; size],
                    }))
                };
                let mut construction = RosterConstruction {
                    image: &image,
                    registry: &mut rng,
                    global_reset: &mut reset,
                    allocate: &mut allocate,
                    external_rng_link: &mut |_, _| {
                        Err("unexpected external RNG in fresh fixture".into())
                    },
                };
                let mut counts = [u32::MAX; 52 * 45];
                let chains = std::array::from_fn(|_| vec![]);
                let businesses = vec![[0; 544]; 600];
                let mut scripts = crate::ai_script::AiScript::new().unwrap();
                let mut status = RosterStatus::default();
                let mut started = vec![];
                let scenario_ai = std::fs::read(root.join(scenario).join("ai.lua")).ok();
                let mut state: crate::ai_script::Initialization<'_> =
                    crate::ai_script::Initialization {
                        profiles: &mut profiles,
                        manager: &mut manager,
                        category_counts: &mut counts,
                        chains: &chains,
                        buildings: &[],
                        businesses: &businesses,
                        difficulty,
                        current_hour: 0,
                        count_tick: Some(0),
                    };
                roster
                    .initialize_created(
                        RosterInputs {
                            names: [b"Fixture Player"; 6],
                            initial_cash,
                            default_ai: &default_ai,
                            scenario_ai: scenario_ai
                                .as_deref()
                                .map(|bytes| ("scenario/ai.lua", bytes)),
                        },
                        &mut construction,
                        &mut state,
                        &mut scripts,
                        &mut status,
                        &mut |manager, reference| {
                            let player = manager
                                .players
                                .iter_mut()
                                .find(|p| p.reference == reference)
                                .unwrap();
                            // Real ownership pass over an explicitly empty city fixture;
                            // auction state is absent in this integration case.
                            let mut ownership = crate::ai_ownership::State {
                                completed_groups: player.bytes[0xaa8],
                                groups: std::array::from_fn(|i| {
                                    player.bytes[0xaac + i * 12..0xaac + (i + 1) * 12]
                                        .try_into()
                                        .unwrap()
                                }),
                                lists: std::array::from_fn(|_| crate::ai_ownership::BusinessList {
                                    entries: vec![],
                                    active_count: 0,
                                }),
                            };
                            ownership.maintain(
                                player.bytes[0x1bc],
                                player.bytes[0x178..0x1ac].try_into().unwrap(),
                                &mapping,
                                &businesses,
                                &[0; 600],
                                |_| Err("empty fixture requested business valuation".into()),
                            )?;
                            assert!(ownership.lists.iter().all(|list| list.entries.is_empty()));
                            player.bytes[0xaa8] = ownership.completed_groups;
                            for (i, group) in ownership.groups.iter().enumerate() {
                                player.bytes[0xaac + i * 12..0xaac + (i + 1) * 12]
                                    .copy_from_slice(group);
                            }
                            started.push(reference);
                            Ok(())
                        },
                    )
                    .unwrap();
                let expected_players = roster.records.iter().filter(|r| r.kind != -1).count();
                let expected_ai = roster.records.iter().filter(|r| r.kind == 1).count();
                assert_eq!(status.ai_count as usize, expected_ai);
                assert_eq!(status.has_ai, expected_ai != 0);
                assert_eq!(status.current_player, manager.players[0].reference);
                assert_eq!(manager.players.len(), expected_players);
                assert_eq!(word(&manager.bytes, 8) as usize, expected_players);
                assert_eq!(allocations, expected_players * 3);
                assert_eq!(started.len(), expected_ai);
                let mut nodes = vec![];
                for (slot, player) in manager.players.iter().enumerate() {
                    assert_eq!(
                        word(&player.bytes, 0x150),
                        roster.records[slot].character as u32
                    );
                    let account =
                        crate::economy::PlayerAccount::from_runtime(&player.bytes).unwrap();
                    assert_eq!(account.cash, initial_cash);
                    nodes.push((
                        player.auxiliaries[0].reference + 0x1c,
                        word(&player.auxiliaries[0].bytes, 0x28),
                        word(&player.auxiliaries[0].bytes, 0x2c),
                    ));
                    if roster.records[slot].kind == 1 {
                        assert_eq!(
                            word(&player.bytes, 0x1e3c),
                            roster.records[slot].character as u32
                        );
                        assert_eq!(
                            word(&player.bytes, 0x13f4),
                            roster.records[slot].difficulty as u32
                        );
                        nodes.push((
                            player.reference + 0x224,
                            word(&player.bytes, 0x230),
                            word(&player.bytes, 0x234),
                        ));
                    }
                }
                assert_eq!(rng.head, nodes[0].0);
                assert_eq!(rng.tail, nodes.last().unwrap().0);
                for (i, node) in nodes.iter().enumerate() {
                    assert_eq!(node.1, nodes.get(i + 1).map_or(0, |n| n.0));
                    assert_eq!(node.2, i.checked_sub(1).map_or(0, |p| nodes[p].0));
                }
                if status.has_ai {
                    assert_eq!(counts, [0; 52 * 45]);
                    assert_eq!(scripts.scores.bytes[25], 75);
                } else {
                    assert_eq!(counts, [u32::MAX; 52 * 45]);
                    assert_eq!(scripts.scores.bytes, [0; 29]);
                }
            }
        }
    }

    #[test]
    #[ignore = "requires original4a8610 token-resolution observations"]
    fn matches_native_token_resolution() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PLAYER_TOKENS").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTPLTOK1");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let mut table = bytes[at..at + 0x120].to_vec();
            let mut pending = PendingRoster::default();
            for slot in 0..6 {
                pending.records[slot].character = word(at + slot * 48) as i32;
                pending.records[slot].kind = word(at + slot * 48 + 4) as i32;
            }
            at += 0x120;
            pending.script_characters = std::array::from_fn(|i| word(at + i * 4) as i32);
            at += 24;
            let expected = &bytes[at..at + 0x120];
            at += 0x120;
            let count = word(at);
            at += 4;
            let mut expected_calls = vec![];
            for _ in 0..count {
                expected_calls.push((word(at) as usize, word(at + 4) as i32));
                at += 8;
            }
            let mut calls = vec![];
            pending
                .resolve_tokens(&mut |slot, character| calls.push((slot, character)))
                .unwrap();
            assert_eq!(calls, expected_calls, "token calls {case}");
            for slot in 0..6 {
                table[slot * 48..slot * 48 + 4]
                    .copy_from_slice(&pending.records[slot].character.to_le_bytes());
            }
            assert_eq!(table, expected, "token table {case}");
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original RegisterPlayer callback observations"]
    fn matches_native_registration() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PLAYER_REGISTRATION").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTPLR001");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 1);
        let mut at = 16;
        for case in 0..word(12) {
            let count = word(at) as usize;
            at += 4;
            let values: [f64; 4] = std::array::from_fn(|i| {
                f64::from_le_bytes(bytes[at + i * 8..at + i * 8 + 8].try_into().unwrap())
            });
            at += 32;
            let registration = Registration::from_numbers(&values[..count]);
            // Direct model/UI side effects are outside this returned command.
            at += 0x120 * 2 + 0x30 * 2;
            assert_eq!(word(at), 4);
            at += 4;
            for (kind, value) in [
                (1, registration.kind),
                (2, registration.character),
                (3, registration.character),
                (4, registration.difficulty),
            ] {
                assert_eq!(
                    (word(at), word(at + 4) as i32, word(at + 8) as i32),
                    (kind, registration.index, value),
                    "registration {case}"
                );
                at += 12;
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires supplied original players.lua scripts"]
    fn evaluates_original_rosters_and_difficulty_branches() {
        let ai = std::path::PathBuf::from(std::env::var("OPENMT2001_AI_SCRIPT").unwrap());
        let root = ai.parent().unwrap().parent().unwrap();
        let load = |name: &str, difficulty| {
            evaluate(
                name,
                &std::fs::read(root.join(name).join("players.lua")).unwrap(),
                difficulty,
            )
            .unwrap()
        };
        for difficulty in 0..3 {
            assert_eq!(
                load("SCENARIO3", difficulty),
                [Registration {
                    index: 0,
                    kind: 0,
                    character: 4,
                    difficulty: 0
                }]
            );
            let roster = load("SCENARIO1", difficulty);
            assert_eq!(roster.len(), 4 + difficulty as usize);
            assert_eq!(
                roster[1],
                Registration {
                    index: 1,
                    kind: 1,
                    character: 2,
                    difficulty: 0
                }
            );
            assert_eq!(
                roster[2],
                Registration {
                    index: 2,
                    kind: 1,
                    character: 0,
                    difficulty: i32::from(difficulty != 0)
                }
            );
        }
        let roster = load("defaultai", 2);
        assert_eq!(
            roster.iter().map(|p| p.character).collect::<Vec<_>>(),
            [4, 6, 0, 2, 9, 5]
        );
        assert!(roster.iter().all(|p| p.difficulty == 0));
        assert_eq!(load("sandbox", 0).len(), 1);
        let mut pending = PendingRoster::default();
        pending
            .load_normal(
                "SCENARIO3",
                &std::fs::read(root.join("SCENARIO3/players.lua")).unwrap(),
                2,
                6,
            )
            .unwrap();
        assert_eq!(pending.records[0].character, 6);
        assert!(pending.records[1..].iter().all(|record| record.kind == -1));
        pending
            .load_normal(
                "defaultai",
                &std::fs::read(root.join("defaultai/players.lua")).unwrap(),
                2,
                6,
            )
            .unwrap();
        assert_eq!(pending.records[0].character, 6);
        assert_eq!(pending.records[1].character, 7);
        // Exercise every supplied player's script through the interpreter.
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path().join("players.lua");
            if path.is_file() {
                let bytes = std::fs::read(&path).unwrap();
                for difficulty in 0..3 {
                    evaluate(&path.to_string_lossy(), &bytes, difficulty).unwrap();
                }
            }
        }
    }
}
