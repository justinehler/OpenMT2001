//! Original AI scripts evaluated by Lua4, with game state owned by Rust.
//!
//! This implements the recovered profile bindings, not the complete scenario
//! initializer or original userdata interface. Unrecovered methods fail in Lua.
use crate::lua4::{Runtime, Value};
use crate::player_messages::Players;

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

const METHODS: [(&str, usize); 14] = [
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
    ("GetCash", 0),
    ("SetMoney", 1),
    ("IncreaseMoney", 1),
    ("DecreaseMoney", 1),
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
            if id >= 10 {
                // Keep lua_tonumber coercion (including strings/nil) in Lua.
                runtime.register_coerced(&host, id as u32, &"n".repeat(count + 1))?;
            } else {
                runtime.register(&host, id as u32)?;
            }
            let arguments = (0..*count)
                .map(|i| format!("a{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let separator = if *count == 0 { "" } else { "," };
            bindings.push_str(&format!("for i=0,9 do AiChars[i].{name}=function(self{separator}{arguments}) return {host}(self.index{separator}{arguments}) end end;\n"));
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
        self.load(
            state.profiles,
            &mut state.manager.players,
            default,
            scenario,
            state.current_hour,
        )?;
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
            &mut state.manager.players,
            "EVENT_LoadScoreData",
            &[],
            state.current_hour,
        )
    }

    /// 4bbd20 passes false to 4d7a80 after resetting each loaded AI. Keep the
    /// reset strategy deadline so its first controller pass can select a
    /// strategy; fresh-game build-time randomization would delay that selection.
    pub fn initialize_loaded<const N: usize>(
        &mut self,
        state: &mut Initialization<'_, N>,
        default: &[u8],
        scenario: Option<(&str, &[u8])>,
    ) -> Result<(), String> {
        state.count_tick = None;
        self.initialize(state, default, scenario)
    }

    /// Evaluate complete DEFAULT first, then the selected scenario's override.
    /// The host supplies buffers; this shared layer performs no filesystem I/O.
    pub fn load(
        &mut self,
        profiles: &mut [[u8; 0x650]],
        players: &mut impl Players,
        default: &[u8],
        scenario: Option<(&str, &[u8])>,
        current_hour: i32,
    ) -> Result<(), String> {
        self.runtime
            .execute("DEFAULT/ai.lua", default, &mut |id, args| {
                dispatch(
                    profiles,
                    players,
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
                    players,
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
        players: &mut impl Players,
        event: &str,
        arguments: &[f64],
        current_hour: i32,
    ) -> Result<(), String> {
        self.runtime.call(event, arguments, &mut |id, args| {
            dispatch(
                profiles,
                players,
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
        players: &mut impl Players,
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
                            players,
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
                    players,
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
        players: &mut impl Players,
        event: &str,
        argument: i32,
        current_hour: i32,
    ) -> Result<(), String> {
        if let Some(pending) = self.trigger(profiles, players, event, argument, current_hour)? {
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
            &mut |manager, profiles, index, event| {
                self.call(
                    profiles,
                    &mut manager.players,
                    EVENTS[event],
                    &[index as f64],
                    current_hour,
                )
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
    players: &mut impl Players,
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
    if id >= 10 {
        // AiChars are character-indexed; the profile's first word links to the
        // live player, whose roster slot need not equal the character index.
        let reference = u32::from_le_bytes(profile[..4].try_into().unwrap());
        let mut slot = 0;
        let slot = loop {
            let player = players
                .get(slot)
                .ok_or("AI cash player link is unresolved")?;
            if reference != 0 && player.reference == reference {
                break slot;
            }
            slot += 1;
        };
        let player = players.get_mut(slot).unwrap();
        let mut account = crate::economy::PlayerAccount::from_runtime(&player.bytes)?;
        if id == 10 {
            // 4d7180 returns the signed low word of _ftol(cash), as a number.
            return Ok(Some(f64::from(crate::economy::ftol_low_word(f64::from(
                account.cash,
            )))));
        }
        use crate::economy::ScriptMoneyOperation;
        let operation = match id {
            11 => ScriptMoneyOperation::Set,
            12 => ScriptMoneyOperation::Increase,
            13 => ScriptMoneyOperation::Decrease,
            _ => return Err("unknown AI cash binding".into()),
        };
        // 4d6f10/4d6f40/4d6f80 update cash directly, without ledger posts.
        account.apply_script_money(values[0], operation);
        account.write_runtime(&mut player.bytes)?;
        return Ok(None);
    }
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
    fn cash_player(reference: u32, cash: f32) -> crate::player_manager::Player {
        let mut bytes = vec![0xa5; 0x1c0];
        bytes[4..8].copy_from_slice(&cash.to_le_bytes());
        for entry in 0..76 {
            let at = 0x14 + entry * 4;
            bytes[at..at + 4].copy_from_slice(&(entry as f32 + 0.25).to_le_bytes());
        }
        bytes[0x144] = 1;
        crate::player_manager::Player {
            reference,
            bytes,
            auxiliaries: vec![],
        }
    }

    #[test]
    fn cash_methods_use_linked_live_accounts_and_native_results() {
        let mut script = AiScript::new().unwrap();
        let mut profiles = vec![[0; 0x650]; 10];
        let mut players = vec![cash_player(0x2000, 100.75), cash_player(0x1000, -20.75)];
        profiles[7][..4].copy_from_slice(&players[0].reference.to_le_bytes());
        profiles[2][..4].copy_from_slice(&players[1].reference.to_le_bytes());
        script
            .load(
                &mut profiles,
                &mut players,
                br#"
                function capture(...) result_count=arg.n; result=arg[1] end
                function read(a) capture(AiChars[a]:GetCash()) end
                function set(a,v) capture(AiChars[a]:SetMoney(v)) end
                function increase(a,v) capture(AiChars[a]:IncreaseMoney(v)) end
                function decrease(a,v) capture(AiChars[a]:DecreaseMoney(v)) end
                "#,
                None,
                0,
            )
            .unwrap();
        for (character, expected) in [(7., 100.), (2., -20.)] {
            script
                .call(&mut profiles, &mut players, "read", &[character], 0)
                .unwrap();
            assert_eq!(script.runtime.number("result_count").unwrap(), Some(1.));
            assert_eq!(script.runtime.number("result").unwrap(), Some(expected));
        }
        // Each case starts with fractional cash and distinctive values in both
        // ledger periods. Only the linked player's four cash bytes may change.
        for (event, amount, expected) in [
            ("set", 12.9, 12.),
            ("set", -12.9, -12.),
            ("increase", 12.9, 112.75),
            ("increase", -12.9, 88.75),
            ("decrease", 12.9, 112.75),
            ("decrease", -12.9, 88.75),
            // _ftol returns i64, but these callbacks consume only its low word.
            ("set", 4294967297.9, 1.),
        ] {
            players[0].bytes[4..8].copy_from_slice(&100.75f32.to_le_bytes());
            let before: Vec<_> = players.iter().map(|player| player.bytes.clone()).collect();
            script
                .call(&mut profiles, &mut players, event, &[7., amount], 0)
                .unwrap();
            assert_eq!(
                script.runtime.number("result_count").unwrap(),
                Some(0.),
                "{event}"
            );
            let account = crate::economy::PlayerAccount::from_runtime(&players[0].bytes).unwrap();
            assert_eq!(account.cash, expected, "{event}({amount})");
            let mut expected_bytes = before[0].clone();
            expected_bytes[4..8].copy_from_slice(&expected.to_le_bytes());
            assert_eq!(
                players[0].bytes, expected_bytes,
                "{event} preserves ledgers and player state"
            );
            assert_eq!(players[1].bytes, before[1]);
        }
        // GetCash follows _ftol's low-word behavior as well as truncation.
        players[1].bytes[4..8].copy_from_slice(&2147483648.0f32.to_le_bytes());
        script
            .call(&mut profiles, &mut players, "read", &[2.], 0)
            .unwrap();
        assert_eq!(script.runtime.number("result").unwrap(), Some(-2147483648.));
    }

    #[test]
    fn cash_arguments_use_lua_number_coercion() {
        let mut script = AiScript::new().unwrap();
        let mut profiles = vec![[0; 0x650]; 10];
        let mut players = vec![cash_player(0x1234, 50.5)];
        profiles[9][..4].copy_from_slice(&players[0].reference.to_le_bytes());
        // These are newly authored snippets; no supplied game scripts are needed.
        for (expression, expected) in [
            ("AiChars[9]:SetMoney('14.9')", 14.),
            ("AiChars[9]:IncreaseMoney('-3.9')", 11.),
            ("AiChars[9]:DecreaseMoney('-2.9')", 9.),
            ("AiChars[9]:SetMoney(nil)", 0.),
            ("AiChars[9]:IncreaseMoney({})", 0.),
            ("AiChars[9]:SetMoney()", 0.),
            ("AiChars[9]:SetMoney('not a number')", 0.),
            ("AiChars[9]:SetMoney(7.9,99)", 7.),
        ] {
            script
                .load(&mut profiles, &mut players, expression.as_bytes(), None, 0)
                .unwrap();
            assert_eq!(
                crate::economy::PlayerAccount::from_runtime(&players[0].bytes)
                    .unwrap()
                    .cash,
                expected,
                "{expression}"
            );
        }
        profiles[9][..4].fill(0);
        assert!(
            script
                .load(
                    &mut profiles,
                    &mut players,
                    b"AiChars[9]:SetMoney(5)",
                    None,
                    0
                )
                .unwrap_err()
                .contains("player link is unresolved")
        );
        assert_eq!(
            crate::economy::PlayerAccount::from_runtime(&players[0].bytes)
                .unwrap()
                .cash,
            7.
        );
    }

    #[test]
    fn cash_events_share_live_split_players_and_prefix_order() {
        let mut script = AiScript::new().unwrap();
        let mut profiles = vec![[0; 0x650]; 10];
        let mut players = vec![
            cash_player(0x2000, 1.),
            cash_player(0x1000, 2.),
            cash_player(0x3000, 3.),
        ];
        for (character, slot) in [(7, 0), (2, 1), (9, 2)] {
            profiles[character][..4].copy_from_slice(&players[slot].reference.to_le_bytes());
        }
        script
            .load(
                &mut profiles,
                &mut players,
                br#"
            function EVENTINITIALISE_Cash(a) AiChars[a]:SetMoney(10.9) end
            function EVENT_Cash(a)
                AiChars[a]:IncreaseMoney(-3.9)
                AiChars[7]:DecreaseMoney(-2.9)
                AiChars[9]:SetMoney(AiChars[a]:GetCash()+0.9)
            end
        "#,
                None,
                0,
            )
            .unwrap();
        let (before, rest) = players.split_at_mut(1);
        let (current, after) = rest.split_first_mut().unwrap();
        script
            .trigger_timetable(
                &mut profiles,
                &mut crate::player_messages::SplitPlayers {
                    before,
                    current,
                    after,
                },
                "Cash",
                2,
                0,
            )
            .unwrap();
        let cash: Vec<_> = players
            .iter()
            .map(|player| {
                crate::economy::PlayerAccount::from_runtime(&player.bytes)
                    .unwrap()
                    .cash
            })
            .collect();
        assert_eq!(cash, [-1., 7., 7.]);
    }

    #[test]
    #[ignore = "requires tools/native_tutorial_owner.py original twiddle fixture"]
    fn original_twiddle_changes_linked_player_once() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_TUTORIAL_TWIDDLE")).unwrap();
        assert_eq!(&bytes[..8], b"MTTWIDD0");
        let mut player = crate::player_manager::Player {
            reference: 1,
            bytes: bytes[8..8 + 0x2400].to_vec(),
            auxiliaries: vec![],
        };
        let mut script = AiScript::new().unwrap();
        let mut profiles = [[0; 0x650]; 10];
        let original =
            std::fs::read(crate::fixtures::path("SCRIPTS").join("TUTORIAL3/ai.lua")).unwrap();
        script
            .load(
                &mut profiles,
                &mut vec![],
                b"",
                Some(("TUTORIAL3/ai.lua", &original)),
                0,
            )
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
                &mut profiles, &mut vec![],
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
            .trigger(&mut profiles, &mut vec![], "Choice", 2, 0)
            .unwrap()
            .unwrap();
        assert_eq!((pending.remaining, pending.argument), (2, 2));
        assert_eq!(script.runtime.number("trace").unwrap(), Some(12.));
        let expected = profiles[2];
        script
            .call(&mut profiles, &mut vec![], "EVENT_Choice", &[2.], 0)
            .unwrap();
        assert_eq!(profiles[2], expected);
        script.runtime.release_reference(pending.reference).unwrap();
        assert!(
            script
                .trigger(&mut profiles, &mut vec![], "InitializerOnly", 2, 0)
                .unwrap()
                .is_none()
        );
        let expected = profiles[2];
        script
            .call(
                &mut profiles,
                &mut vec![],
                "EVENTINITIALISE_InitializerOnly",
                &[2.],
                0,
            )
            .unwrap();
        assert_eq!(profiles[2], expected);
        script
            .trigger_timetable(&mut profiles, &mut vec![], "Stop", 2, 0)
            .unwrap();
        script
            .trigger_timetable(&mut profiles, &mut vec![], "Absent", 2, 0)
            .unwrap();
    }
    #[test]
    #[ignore = "requires original AI timetable script"]
    fn original_timetable_triggers_use_native_event_prefix() {
        let source = std::fs::read(crate::fixtures::path("AI_SCRIPT")).unwrap();
        let mut script = AiScript::new().unwrap();
        let mut profiles = vec![[0; 0x650]; 10];
        script
            .load(&mut profiles, &mut vec![], &source, None, 10)
            .unwrap();
        for event in ["LoadLandscapeBlockTT", "Wait4CashTT", "Kill_TAC_Timetable"] {
            script
                .trigger_timetable(&mut profiles, &mut vec![], event, 2, 10)
                .unwrap();
            let expected = profiles.clone();
            script
                .call(
                    &mut profiles,
                    &mut vec![],
                    &format!("EVENT_{event}"),
                    &[2.],
                    10,
                )
                .unwrap();
            assert_eq!(profiles, expected, "{event}");
        }
    }
    #[test]
    #[ignore = "requires original score property dispatch observations"]
    fn matches_native_score_properties_through_lua() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_SCORE_PROPERTIES")).unwrap();
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
                .load(&mut profiles, &mut vec![], source.as_bytes(), None, 0)
                .unwrap();
            for step in 0..steps {
                let property = word(at);
                let number = f64::from_le_bytes(data[at + 4..at + 12].try_into().unwrap());
                at += 12;
                script
                    .call(
                        &mut profiles,
                        &mut vec![],
                        &format!("set{property}"),
                        &[number],
                        0,
                    )
                    .unwrap();
                assert_eq!(
                    script.scores.bytes.as_slice(),
                    &data[at..at + 29],
                    "case{case}/step{step}"
                );
                at += 29;
                script
                    .call(&mut profiles, &mut vec![], "reads", &[], 0)
                    .unwrap();
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
        let source = std::fs::read(crate::fixtures::path("AI_SCRIPT")).unwrap();
        let fixture = std::fs::read(crate::fixtures::path("NATIVE_AI_PREFERENCES")).unwrap();
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
        scripts.load(&mut profiles, &mut vec![],&source,Some(("scenario/ai.lua",b"function EVENT_LoadBuyBlockTimetable(a) AiChars[a]:ResetTimeTable(BLK); AiChars[a]:LoadTimeTable(BLK,9,5,TT_BUY_BLOCK); end")), 0).unwrap();
        scripts
            .call(
                &mut profiles,
                &mut vec![],
                "EVENT_LoadBuyBlockTimetable",
                &[0.],
                0,
            )
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
        scripts
            .call(
                &mut profiles,
                &mut manager.players,
                "EVENT_SetPlayerMoney",
                &[0., 100.],
                0,
            )
            .unwrap();
        assert_eq!(
            crate::economy::PlayerAccount::from_runtime(&manager.players[0].bytes)
                .unwrap()
                .cash,
            100.
        );
    }
}

#[cfg(test)]
#[path = "ai_script_loaded_retail_test.rs"]
mod loaded_retail_test;
