//! Frontend FLYBY1 level event, evaluated by the same Lua4 runtime as city hubs.
use crate::lua4::{Runtime, Value};

#[derive(Debug, PartialEq, Eq)]
pub struct Selection {
    pub hour: u32,
    pub logos: bool,
    /// Filename below the installer's Standard/Cut_scenes directory.
    pub track: String,
}

pub fn performance_cap(quality: &str) -> Result<i32, String> {
    match quality {
        "low" => Ok(0),
        "medium" => Ok(1),
        "high" => Ok(2),
        _ => Err("intro quality must be low, medium or high".into()),
    }
}

/// The map is already assembled by the frontend. The initialization callback
/// acknowledges that load and retains Game.SetLogos for the opening controller.
/// With no CLI quality the live process performance is read unchanged. An
/// explicit quality caps it without overwriting the process value.
/// `random_state` is the caller-owned 93aee8 stream, seeded through startup_rng.
pub fn select(
    source: &[u8],
    quality: Option<&str>,
    random_state: &mut u32,
    mut performance: impl FnMut() -> Result<i32, String>,
) -> Result<Selection, String> {
    let cap = quality.map(performance_cap).transpose()?;
    let mut vm = Runtime::new()?;
    // The ordinary script bootstrap publishes Lua4's numeric true constant.
    vm.set_number("true", 1.)?;
    vm.register_number_properties("Game", 1)?;
    vm.register_number_properties("Calendar", 3)?;
    vm.register_coerced("C_FlybyRandom", 5, "n")?;
    vm.register_coerced("C_FlybyTrack", 6, "S")?;
    vm.register_coerced("C_RunScript", 7, "S")?;
    vm.register_coerced("C_FlybyLogos", 8, "S")?;
    vm.execute("flyby tables", b"Random={}; Camera={}", &mut |_, _| {
        Err("unexpected flyby initialization callback".into())
    })?;
    vm.alias_function("Random", "ZeroTo", "C_FlybyRandom")?;
    vm.alias_function("Camera", "SetFlybyMode", "C_FlybyTrack")?;
    vm.alias_function("Game", "SetLogos", "C_FlybyLogos")?;
    for hour in 0..24 {
        vm.set_number(&format!("HOUR_{hour}"), f64::from(hour))?;
    }
    let mut hour = None;
    let mut track = None;
    let mut logos = false;
    let mut handler = |id, args: &[Value]| match (id, args) {
        (1, [Value::Bytes(key)]) if key == b"performance" => {
            let value = performance()?;
            Ok(Some(f64::from(cap.map_or(value, |cap| value.min(cap)))))
        }
        (4, [Value::Bytes(key), Value::Number(value)]) if key == b"hour" => {
            let value = crate::economy::ftol_low_word(*value) as u32;
            if value >= 24 {
                return Err("flyby hour leaves 0..23".into());
            }
            hour = Some(value);
            Ok(None)
        }
        (5, _) => crate::level_script::random::zero_to(random_state, args).map(Some),
        (6, [Value::Bytes(path)]) => {
            let path = std::str::from_utf8(path)
                .map_err(|e| e.to_string())?
                .replace('\\', "/")
                .to_ascii_lowercase();
            let name = path
                .strip_prefix("gamedata/cut_scenes/")
                .filter(|name| !name.contains('/') && name.ends_with(".tm"))
                .ok_or("unsupported flyby track path")?;
            track = Some(name.to_owned());
            Ok(None)
        }
        (7, [Value::Bytes(path)]) if path.eq_ignore_ascii_case(b"scripts\\maps\\intromap.lua") => {
            Ok(None)
        }
        (8, [value]) => {
            // Original4dad90 tests lua_tostring(-1), including numeric zero.
            logos = matches!(value, Value::Bytes(_));
            Ok(None)
        }
        _ => Err(format!("unsupported frontend flyby callback {id}")),
    };
    vm.execute("FLYBY1/hub.lua", source, &mut handler)?;
    vm.execute(
        "FLYBY1 initialization event",
        b"if EVENTINITIALISE_Level then EVENTINITIALISE_Level() end",
        &mut handler,
    )?;
    vm.execute("FLYBY1 level event", b"EVENT_Level()", &mut handler)?;
    Ok(Selection {
        hour: hour.ok_or("flyby level event did not set an hour")?,
        logos,
        track: track.ok_or("flyby level event did not select a track")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Authored fixture: independent table dispatch exercises the host API in CI.
    const HUB: &[u8] = br#"
        function EVENTINITIALISE_Level()
            C_RunScript("scripts\\maps\\intromap.lua")
            Game.SetLogos(true)
        end
        function EVENT_Level()
            local index = Random.ZeroTo(Game.performance) + 1
            local hours = {HOUR_12, HOUR_20, HOUR_8}
            local tracks = {"low", "medium", "high"}
            Calendar.hour = hours[index]
            Camera.SetFlybyMode("gamedata/cut_scenes/intro_" .. tracks[index] .. ".tm")
        end
    "#;

    fn check_selections(source: &[u8]) {
        for performance in 0..=2 {
            for (quality, cap) in [
                (None, performance),
                (Some("low"), 0),
                (Some("medium"), performance.min(1)),
                (Some("high"), performance),
            ] {
                let mut seen = [false; 3];
                // Fixed startup seeds cover every allowed draw, using the same
                // static-node initialization and RNG as the frontend and results.
                for seed in 0..64 {
                    let mut state = crate::startup_rng::Snapshot::from_seed(seed).state(0x93aee8);
                    let mut expected_state = state;
                    let draw = crate::level_script::random::zero_to(
                        &mut expected_state,
                        &[Value::Number(f64::from(cap))],
                    )
                    .unwrap() as usize;
                    let selected = select(source, quality, &mut state, || Ok(performance)).unwrap();
                    assert_eq!(selected.hour, [12, 20, 8][draw]);
                    assert!(selected.logos);
                    assert_eq!(
                        selected.track,
                        ["intro_low.tm", "intro_medium.tm", "intro_high.tm"][draw]
                    );
                    assert_eq!(state, expected_state, "exactly one RNG draw");
                    seen[draw] = true;
                }
                for (draw, seen) in seen.into_iter().enumerate() {
                    assert_eq!(
                        seen,
                        draw <= cap as usize,
                        "performance {performance}, {quality:?}, draw {draw}"
                    );
                }
            }
        }
    }

    #[test]
    fn default_follows_process_performance_and_explicit_quality_caps_it() {
        check_selections(HUB);
        assert!(performance_cap("ultra").is_err());
    }

    #[test]
    fn uncapped_performance_is_read_live_without_an_implicit_high_cap() {
        let source = br#"function EVENT_Level()
            Calendar.hour=Game.performance
            Camera.SetFlybyMode("gamedata/cut_scenes/custom.tm")
        end"#;
        let mut performance = 3;
        let mut read = || {
            performance += 1;
            Ok(performance)
        };
        let mut state = 123;
        assert_eq!(select(source, None, &mut state, &mut read).unwrap().hour, 4);
        assert_eq!(select(source, None, &mut state, &mut read).unwrap().hour, 5);
        assert_eq!(
            select(source, Some("high"), &mut state, read).unwrap().hour,
            2
        );
        assert_eq!(state, 123);
        assert!(select(HUB, None, &mut 0, || Err("owner absent".into())).is_err());
        assert!(select(HUB, Some("high"), &mut 0, || Err("owner absent".into())).is_err());
    }

    #[test]
    fn script_controls_selection_instead_of_a_native_lookup() {
        let source = br#"function EVENT_Level()
            Calendar.hour=HOUR_3
            Camera.SetFlybyMode("gamedata\\cut_scenes\\custom.tm")
        end"#;
        let mut state = 123;
        let selected = select(source, Some("low"), &mut state, || Ok(2)).unwrap();
        assert_eq!(selected.hour, 3);
        assert_eq!(selected.track, "custom.tm");
        assert_eq!(state, 123);
        assert!(!selected.logos);
    }

    #[test]
    fn initialization_controls_logos_using_native_string_convertibility() {
        for (value, expected) in [("nil", false), ("0", true), ("true", true), ("''", true)] {
            let source = format!(
                "function EVENTINITIALISE_Level() Game.SetLogos({value}) end\n\
                 function EVENT_Level() Calendar.hour=HOUR_3\n\
                 Camera.SetFlybyMode('gamedata/cut_scenes/custom.tm') end"
            );
            let selected = select(source.as_bytes(), None, &mut 0, || Ok(0)).unwrap();
            assert_eq!(selected.logos, expected, "{value}");
        }
    }

    #[test]
    #[ignore = "requires supplied original FLYBY1 hub script"]
    fn original_level_event_follows_process_performance_and_explicit_caps() {
        let source =
            std::fs::read(crate::fixtures::path("SCRIPTS").join("FLYBY1/hub.lua")).unwrap();
        check_selections(&source);
    }
}
