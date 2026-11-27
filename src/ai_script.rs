//! Original AI scripts evaluated by Lua4, with game state owned by Rust.
//!
//! This implements the recovered profile bindings, not the complete scenario
//! initializer or original userdata interface. Unrecovered methods fail in Lua.
use crate::lua4::{Runtime, Value};

pub struct AiScript {
    runtime: Runtime,
    pub scores: crate::ai_parameters::Scores,
    twiddle_thumbs: [bool; 10],
}

/// State touched by original4d7a80 after the player roster is constructed.
pub struct Initialization<'a, const N: usize = 64> {
    pub profiles: &'a mut [[u8; 0x650]],
    pub manager: &'a mut crate::player_manager::Manager,
    pub category_counts: &'a mut [u32; 52 * 45],
    pub chains: &'a [Vec<usize>; 52],
    pub buildings: &'a [[u8; N]],
    pub businesses: &'a [[u8; 544]],
    pub difficulty: i32,
    pub current_hour: i32,
    pub count_tick: Option<u32>,
}

const METHODS: [(&str, usize); 10] = [
    ("SetFloors", 10),
    ("SetFootprint", 4),
    ("ResetTimeTable", 1),
    ("LoadTimeTable", 4),
    ("SetDifficultyVars", 16),
    ("SetPrestigeBias", 1),
    ("SetStrategySelectionBias", 4),
    ("SetBuildTimes", 10),
    ("SetBaseValMultiplier", 1),
    ("SetTwiddleThumbs", 0),
];

impl AiScript {
    pub fn new() -> Result<Self, String> {
        let mut runtime = Runtime::new()?;
        // Original4d72c0 numeric global registrations.
        for (index, name) in crate::ai_parameters::CHARACTER_NAMES.iter().enumerate() {
            runtime.set_number(name, index as f64)?;
        }
        for (name, value) in [
            ("TAC", 0),
            ("HSE", 1),
            ("BLK", 2),
            ("PP_HIGH", 0),
            ("PP_MED", 1),
            ("PP_LOW", 2),
            ("PP_ANY", 3),
            ("TT_POLL", 2),
            ("TT_RENT", 1),
            ("TT_SPOILAGE", 4),
            ("TT_DEMOLISH", 5),
            ("TT_BUY_BLOCK", 7),
            ("TT_SELL_BLOCK", 8),
            ("TT_CREATE_BUS", 6),
            ("TT_TAKEOVER_BUS", 11),
            ("TT_REFURBISH_BUS", 12),
            ("TT_EOD_STOCK_AND_PRICE", 13),
            ("TT_BUSINESS_STATES", 14),
            ("FORCE_AUCTION", 0),
            ("STOP_STRAT_BLOCKS", 1),
            ("SHOW_SPELL_CARDS", 2),
        ] {
            runtime.set_number(name, f64::from(value))?;
        }
        let mut bindings = String::from("AiChars={}; for i=0,9 do AiChars[i]={index=i} end;\n");
        for (id, (name, count)) in METHODS.iter().enumerate() {
            let host = format!("openmt_ai_{id}");
            runtime.register(&host, id as u32)?;
            let arguments = (0..*count)
                .map(|i| format!("a{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let separator = if *count == 0 { "" } else { "," };
            bindings.push_str(&format!("for i=0,9 do AiChars[i].{name}=function(self{separator}{arguments}) {host}(self.index{separator}{arguments}) end end;\n"));
        }
        runtime.execute("AI profile bindings", bindings.as_bytes(), &mut |_, _| {
            Err("unexpected binding initialization call".into())
        })?;
        runtime.register_number_properties("AiScores", 100)?;
        Ok(Self {
            runtime,
            scores: Default::default(),
            twiddle_thumbs: [false; 10],
        })
    }

    /// Publish4d6ef0's one byte store to its linked runtime player.
    /// Requests are consumed once; the flag can subsequently change in native AI logic.
    pub fn apply_twiddle_thumbs(
        &mut self,
        player: &mut crate::player_manager::Player,
    ) -> Result<(), String> {
        let character = u32::from_le_bytes(
            player
                .bytes
                .get(0x150..0x154)
                .ok_or("twiddle player truncated")?
                .try_into()
                .unwrap(),
        ) as usize;
        let pending = self
            .twiddle_thumbs
            .get_mut(character)
            .ok_or("twiddle character leaves profile table")?;
        if *pending {
            *player
                .bytes
                .get_mut(0x1403)
                .ok_or("twiddle linked player is not AI storage")? = 1;
            *pending = false;
        }
        Ok(())
    }

    /// Original loader ordering: scripts, four events per active profile,
    /// linked-player setup, category counts, then the score event.470260 is RET.
    /// Buffers and the already-created city/player state are supplied by host.
    pub fn initialize<const N: usize>(
        &mut self,
        state: &mut Initialization<'_, N>,
        default: &[u8],
        scenario: Option<(&str, &[u8])>,
    ) -> Result<(), String> {
        self.load(state.profiles, default, scenario, state.current_hour)?;
        for player in &mut state.manager.players {
            self.apply_twiddle_thumbs(player)?;
        }
        self.initialize_players(
            state.profiles,
            state.current_hour,
            state.difficulty,
            state.manager,
            state.count_tick,
        )?;
        for player in &mut state.manager.players {
            self.apply_twiddle_thumbs(player)?;
        }
        crate::ai_business::rebuild_category_counts(
            state.category_counts,
            state.chains,
            state.buildings,
            state.businesses,
        )?;
        self.call(
            state.profiles,
            "EVENT_LoadScoreData",
            &[],
            state.current_hour,
        )
    }

    /// Evaluate complete DEFAULT first, then the selected scenario's override.
    /// The host supplies buffers; this shared layer performs no filesystem I/O.
    pub fn load(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        default: &[u8],
        scenario: Option<(&str, &[u8])>,
        current_hour: i32,
    ) -> Result<(), String> {
        self.runtime
            .execute("DEFAULT/ai.lua", default, &mut |id, args| {
                dispatch(
                    profiles,
                    &mut self.scores,
                    &mut self.twiddle_thumbs,
                    id,
                    args,
                    current_hour,
                )
            })?;
        if let Some((name, bytes)) = scenario {
            self.runtime.execute(name, bytes, &mut |id, args| {
                dispatch(
                    profiles,
                    &mut self.scores,
                    &mut self.twiddle_thumbs,
                    id,
                    args,
                    current_hour,
                )
            })?;
        }
        Ok(())
    }

    /// Invoke one recovered script event. The host retains initialization order
    /// and active-profile selection; this does not start a world simulation.
    pub fn call(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        event: &str,
        arguments: &[f64],
        current_hour: i32,
    ) -> Result<(), String> {
        self.runtime.call(event, arguments, &mut |id, args| {
            dispatch(
                profiles,
                &mut self.scores,
                &mut self.twiddle_thumbs,
                id,
                args,
                current_hour,
            )
        })
    }

    /// Original4d5d00 zero-delay trigger: optional initializer first, then the
    /// optional EVENT function. A nonzero return retains a weak reference for
    /// the shared event scheduler; the caller must admit it to the proper VM.
    pub fn trigger(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        event: &str,
        argument: i32,
        current_hour: i32,
    ) -> Result<Option<crate::script_events::Event>, String> {
        let initializer = format!("EVENTINITIALISE_{event}");
        if let Some(reference) = self.runtime.reference_function(&initializer)? {
            let result =
                self.runtime
                    .call(&initializer, &[f64::from(argument)], &mut |id, args| {
                        dispatch(
                            profiles,
                            &mut self.scores,
                            &mut self.twiddle_thumbs,
                            id,
                            args,
                            current_hour,
                        )
                    });
            let release = self.runtime.release_reference(reference);
            result?;
            release?;
        }
        let Some(reference) = self.runtime.reference_function(&format!("EVENT_{event}"))? else {
            return Ok(None);
        };
        let result = self
            .runtime
            .call_reference(reference, argument, &mut |id, args| {
                dispatch(
                    profiles,
                    &mut self.scores,
                    &mut self.twiddle_thumbs,
                    id,
                    args,
                    current_hour,
                )
            });
        let remaining = match result {
            Ok(Some(delay)) => crate::economy::ftol_low_word(delay) as u32,
            Ok(None) => {
                self.runtime.release_reference(reference)?;
                return Err("AI event function disappeared before synchronous invocation".into());
            }
            Err(error) => {
                let _ = self.runtime.release_reference(reference);
                return Err(format!("AI event {event}: {error}"));
            }
        };
        if remaining == 0 {
            self.runtime.release_reference(reference)?;
            Ok(None)
        } else {
            Ok(Some(crate::script_events::Event {
                reference,
                remaining,
                argument,
            }))
        }
    }

    /// Native AI timetable events return nil/zero. Preserve both prefix calls;
    /// refuse a delayed result until cross-VM shared-queue ownership is bound.
    pub fn trigger_timetable(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        event: &str,
        argument: i32,
        current_hour: i32,
    ) -> Result<(), String> {
        if let Some(pending) = self.trigger(profiles, event, argument, current_hour)? {
            self.runtime.release_reference(pending.reference)?;
            return Err(format!(
                "AI event {event} requested unresolved shared-VM delay {}",
                pending.remaining
            ));
        }
        Ok(())
    }

    /// Original active-profile loop including linked-player difficulty and
    /// optional47d5f0 result. Global postludes remain the host's responsibility.
    pub fn initialize_players(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        current_hour: i32,
        difficulty: i32,
        manager: &mut crate::player_manager::Manager,
        count_tick: Option<u32>,
    ) -> Result<(), String> {
        const EVENTS: [&str; 4] = [
            "EVENT_LoadHseKeepingTimetable",
            "EVENT_LoadBuyBlockTimetable",
            "EVENT_LoadDifficultyVariables",
            "EVENT_LoadBuildTimes",
        ];
        manager.initialize_ai(
            profiles,
            difficulty,
            count_tick.is_some(),
            &mut |_, profiles, index, event| {
                self.call(profiles, EVENTS[event], &[index as f64], current_hour)
            },
            &mut |player, profiles| {
                Ok(
                    player.initial_count(profiles, count_tick.ok_or("missing count clock")?)?
                        as u16,
                )
            },
        )
    }
}

fn dispatch(
    profiles: &mut [[u8; 0x650]],
    scores: &mut crate::ai_parameters::Scores,
    twiddle_thumbs: &mut [bool; 10],
    id: u32,
    args: &[Value],
    current_hour: i32,
) -> Result<Option<f64>, String> {
    if id == 100 || id == 101 {
        let Some(Value::Bytes(key)) = args.first() else {
            return Err("AI score property requires a string key".into());
        };
        let property = crate::ai_parameters::SCORE_PROPERTIES
            .iter()
            .position(|(name, _)| name.as_bytes() == key)
            .ok_or("unknown AI score property")?;
        if id == 100 {
            return Ok(Some(f64::from(scores.get(property)?)));
        }
        let number = match args.get(1) {
            Some(Value::Number(number)) => *number,
            Some(Value::Nil) => 0.,
            _ => return Err("AI score numeric-string conversion is not implemented".into()),
        };
        scores.set(property, number)?;
        return Ok(None);
    }
    let (name, count) = METHODS.get(id as usize).ok_or("unknown AI binding")?;
    if args.len() != count + 1 {
        return Err(format!("{name}: wrong argument count"));
    }
    let values = args
        .iter()
        .map(|value| match value {
            Value::Number(n) => Ok(*n),
            _ => Err(format!("{name}: expected numeric argument")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let index = crate::economy::ftol_low_word(values[0]);
    let profile = usize::try_from(index)
        .ok()
        .and_then(|index| profiles.get_mut(index))
        .ok_or("AI character leaves profile table")?;
    let values = &values[1..];
    match id {
        0 => crate::ai_parameters::set_floors(profile, values.try_into().unwrap()),
        1 => crate::ai_parameters::set_footprint(profile, values.try_into().unwrap())?,
        2 => crate::ai_events::configure_timetable(profile, true, [values[0], 0., 0., 0.])?,
        3 => crate::ai_events::configure_timetable(profile, false, values.try_into().unwrap())?,
        4 => crate::ai_parameters::set_difficulty(profile, values.try_into().unwrap())?,
        5 => crate::ai_parameters::set_prestige_bias(profile, values[0]),
        6 => crate::ai_parameters::set_strategy_bias(profile, values.try_into().unwrap()),
        7 => crate::ai_parameters::set_build_times(
            profile,
            values.try_into().unwrap(),
            current_hour,
        )?,
        8 => crate::ai_parameters::set_base_value_multiplier(profile, values[0]),
        //4d6ef0 sets the AiChars linked runtime player, not its profile.
        9 => twiddle_thumbs[index as usize] = true,
        _ => return Err("unknown AI binding".into()),
    }
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires tools/native_tutorial_owner.py original twiddle fixture"]
    fn original_twiddle_changes_linked_player_once() {
        let bytes = std::fs::read("work/economy/native-tutorial-twiddle.bin").unwrap();
        assert_eq!(&bytes[..8], b"MTTWIDD0");
        let mut player = crate::player_manager::Player {
            reference: 1,
            bytes: bytes[8..8 + 0x2400].to_vec(),
            auxiliaries: vec![],
        };
        let mut script = AiScript::new().unwrap();
        let mut profiles = [[0; 0x650]; 10];
        let original =
            std::fs::read("work/patch14/payload/Essential/scripts/TUTORIAL3/ai.lua").unwrap();
        script
            .load(&mut profiles, b"", Some(("TUTORIAL3/ai.lua", &original)), 0)
            .unwrap();
        script.apply_twiddle_thumbs(&mut player).unwrap();
        assert_eq!(player.bytes, bytes[8 + 0x2400..]);
        player.bytes[0x1403] = 0;
        script.apply_twiddle_thumbs(&mut player).unwrap();
        assert_eq!(player.bytes[0x1403], 0, "the request is consumed once");
    }

    #[test]
    fn native_trigger_prefix_order_and_optional_functions() {
        let mut script = AiScript::new().unwrap();
        let mut profiles = vec![[0; 0x650]; 10];
        script
            .load(
                &mut profiles,
                br#"
            trace=0; function EVENTINITIALISE_Choice(a) trace=trace*10+1; AiChars[a]:SetBaseValMultiplier(3) end
            function EVENT_Choice(a) trace=trace*10+2; AiChars[a]:SetBaseValMultiplier(7); return '2.9' end
            function EVENTINITIALISE_InitializerOnly(a) AiChars[a]:SetBaseValMultiplier(11) end
            function EVENT_Stop(a) AiChars[a]:SetBaseValMultiplier(13) end
        "#,
                None,
                0,
            )
            .unwrap();
        let pending = script
            .trigger(&mut profiles, "Choice", 2, 0)
            .unwrap()
            .unwrap();
        assert_eq!((pending.remaining, pending.argument), (2, 2));
        assert_eq!(script.runtime.number("trace").unwrap(), Some(12.));
        let expected = profiles[2];
        script
            .call(&mut profiles, "EVENT_Choice", &[2.], 0)
            .unwrap();
        assert_eq!(profiles[2], expected);
        script.runtime.release_reference(pending.reference).unwrap();
        assert!(
            script
                .trigger(&mut profiles, "InitializerOnly", 2, 0)
                .unwrap()
                .is_none()
        );
        let expected = profiles[2];
        script
            .call(&mut profiles, "EVENTINITIALISE_InitializerOnly", &[2.], 0)
            .unwrap();
        assert_eq!(profiles[2], expected);
        script
            .trigger_timetable(&mut profiles, "Stop", 2, 0)
            .unwrap();
        script
            .trigger_timetable(&mut profiles, "Absent", 2, 0)
            .unwrap();
    }
    #[test]
    #[ignore = "requires original AI timetable script"]
    fn original_timetable_triggers_use_native_event_prefix() {
        let source = std::fs::read(std::env::var("OPENMT2001_AI_SCRIPT").unwrap()).unwrap();
        let mut script = AiScript::new().unwrap();
        let mut profiles = vec![[0; 0x650]; 10];
        script.load(&mut profiles, &source, None, 10).unwrap();
        for event in ["LoadLandscapeBlockTT", "Wait4CashTT", "Kill_TAC_Timetable"] {
            script
                .trigger_timetable(&mut profiles, event, 2, 10)
                .unwrap();
            let expected = profiles.clone();
            script
                .call(&mut profiles, &format!("EVENT_{event}"), &[2.], 10)
                .unwrap();
            assert_eq!(profiles, expected, "{event}");
        }
    }
    #[test]
    #[ignore = "requires original score property dispatch observations"]
    fn matches_native_score_properties_through_lua() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_SCORE_PROPERTIES").unwrap()).unwrap();
        let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(&data[..8], b"MTAISCP3");
        assert_eq!(word(8), 1);
        let mut source = String::new();
        for (index, (name, _)) in crate::ai_parameters::SCORE_PROPERTIES.iter().enumerate() {
            source.push_str(&format!("function set{index}(v) AiScores.{name}=v end;\n"));
        }
        source.push_str("function reads() ");
        for (index, (name, _)) in crate::ai_parameters::SCORE_PROPERTIES.iter().enumerate() {
            source.push_str(&format!("r{index}=AiScores.{name}; "));
        }
        source.push_str("end");
        let mut at = 16;
        for case in 0..word(12) {
            let mut script = AiScript::new().unwrap();
            script.scores.bytes.copy_from_slice(&data[at..at + 29]);
            at += 29;
            let steps = word(at);
            at += 4;
            let mut profiles = vec![[0; 0x650]; 10];
            script
                .load(&mut profiles, source.as_bytes(), None, 0)
                .unwrap();
            for step in 0..steps {
                let property = word(at);
                let number = f64::from_le_bytes(data[at + 4..at + 12].try_into().unwrap());
                at += 12;
                script
                    .call(&mut profiles, &format!("set{property}"), &[number], 0)
                    .unwrap();
                assert_eq!(
                    script.scores.bytes.as_slice(),
                    &data[at..at + 29],
                    "case{case}/step{step}"
                );
                at += 29;
                script.call(&mut profiles, "reads", &[], 0).unwrap();
                for property in 0..25 {
                    let expected = f64::from_le_bytes(data[at..at + 8].try_into().unwrap());
                    at += 8;
                    assert_eq!(
                        script.runtime.number(&format!("r{property}")).unwrap(),
                        Some(expected)
                    );
                }
            }
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires supplied DEFAULT script and native preference observations"]
    fn loads_complete_default_and_runs_recovered_events() {
        let source = std::fs::read(std::env::var("OPENMT2001_AI_SCRIPT").unwrap()).unwrap();
        let fixture =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_PREFERENCES").unwrap()).unwrap();
        assert_eq!(&fixture[..8], b"MTAIPRF1");
        let mut pattern = std::array::from_fn(|i| (i as u8).wrapping_mul(17).wrapping_add(3));
        pattern[4] = 0;
        let mut profiles = vec![pattern; 10];
        for profile in &mut profiles[..10] {
            profile[4] = 1;
        }
        let mut manager = crate::player_manager::Manager {
            bytes: [0; 0x28],
            players: (0..10)
                .map(|i| crate::player_manager::Player {
                    reference: i + 1,
                    bytes: vec![0; 0x1e50],
                    auxiliaries: vec![],
                })
                .collect(),
        };
        for (index, profile) in profiles.iter_mut().enumerate() {
            profile[..4].copy_from_slice(&(index as u32 + 1).to_le_bytes());
        }
        let mut expected = profiles.clone();
        let floor_order = [0, 4, 3, 2, 6, 7, 8, 5, 1, 9];
        let footprint_order = [4, 3, 7, 2, 6, 0, 9, 5, 8, 1];
        let mut at = 20;
        for case in 0..60 {
            // Combine only native changes from each independent literal call.
            let index = if case < 10 {
                floor_order[case]
            } else {
                footprint_order[(case - 10) / 5]
            };
            at += 12 + 128;
            let before = &fixture[at..at + 0x650];
            let after = &fixture[at + 0x650..at + 0x650 * 2];
            for byte in 0..0x650 {
                if before[byte] != after[byte] {
                    expected[index][byte] = after[byte];
                }
            }
            at += 0x650 * 2 + 4;
        }
        let mut scripts = AiScript::new().unwrap();
        let mut counts = [u32::MAX; 52 * 45];
        scripts
            .initialize(
                &mut Initialization::<64> {
                    profiles: &mut profiles,
                    manager: &mut manager,
                    category_counts: &mut counts,
                    chains: &std::array::from_fn(|_| vec![]),
                    buildings: &[],
                    businesses: &[],
                    difficulty: 2,
                    current_hour: 0,
                    count_tick: None,
                },
                &source,
                None,
            )
            .unwrap();
        assert_eq!(counts, [0; 52 * 45]);
        // Supplied assignments run in source order, including overlapping DWORDs.
        assert_eq!(
            scripts.scores.bytes,
            [
                75, 25, 60, 40, 75, 50, 10, 25, 60, 20, 75, 40, 35, 22, 10, 3, 20, 25, 100, 110, 0,
                30, 0, 0, 10, 75, 0, 0, 0
            ]
        );
        assert!(profiles.iter().all(|profile| profile[0x5a0] == 10));
        for index in 0..10 {
            assert_eq!(
                &profiles[index][0x36..0xef],
                &expected[index][0x36..0xef],
                "profile {index}"
            );
            let secondary =
                crate::ai_events::Timetable::secondary_from_profile(&profiles[index], 1);
            assert_eq!(secondary.count, 3);
            assert_eq!(
                secondary.records[0],
                crate::ai_events::Record {
                    timestamp: 5000,
                    code: 7
                }
            );
            for difficulty in 0..3 {
                assert_eq!(
                    crate::ai_parameters::rent_increase(&profiles[index], difficulty).unwrap(),
                    1
                );
            }
        }
        profiles[9][4] = 0;
        let inactive = profiles[9];
        scripts
            .initialize_players(&mut profiles, 0, 2, &mut manager, None)
            .unwrap();
        assert_eq!(profiles[9], inactive);
        // Scenario definitions must supersede DEFAULT before events are called.
        scripts.load(&mut profiles,&source,Some(("scenario/ai.lua",b"function EVENT_LoadBuyBlockTimetable(a) AiChars[a]:ResetTimeTable(BLK); AiChars[a]:LoadTimeTable(BLK,9,5,TT_BUY_BLOCK); end")), 0).unwrap();
        scripts
            .call(&mut profiles, "EVENT_LoadBuyBlockTimetable", &[0.], 0)
            .unwrap();
        let queue = crate::ai_events::Timetable::secondary_from_profile(&profiles[0], 1);
        assert_eq!(queue.count, 1);
        assert_eq!(
            queue.records[0],
            crate::ai_events::Record {
                timestamp: 5675,
                code: 7
            }
        );
        assert!(
            scripts
                .call(&mut profiles, "EVENT_SetPlayerMoney", &[0., 100.], 0)
                .is_err()
        );
    }
}
