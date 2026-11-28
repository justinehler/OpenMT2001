//! Frontend FLYBY1 level event, evaluated by the same Lua4 runtime as city hubs.
use crate::lua4::{Runtime, Value};

#[derive(Debug, PartialEq, Eq)]
pub struct Selection {
    pub hour: u32,
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

/// The map has already been assembled by the frontend. Only EVENT_Level runs
/// here: EVENTINITIALISE_Level would load that map again and enable logos.
/// The CLI quality caps the live process performance without overwriting it.
/// `random_state` is the caller-owned 93aee8 stream, seeded through startup_rng.
pub fn select(
    source: &[u8],
    quality: &str,
    random_state: &mut u32,
    mut performance: impl FnMut() -> Result<i32, String>,
) -> Result<Selection, String> {
    let cap = performance_cap(quality)?;
    let mut vm = Runtime::new()?;
    vm.register_number_properties("Game", 1)?;
    vm.register_number_properties("Calendar", 3)?;
    vm.register_coerced("C_FlybyRandom", 5, "n")?;
    vm.register_coerced("C_FlybyTrack", 6, "S")?;
    vm.execute("flyby tables", b"Random={}; Camera={}", &mut |_, _| {
        Err("unexpected flyby initialization callback".into())
    })?;
    vm.alias_function("Random", "ZeroTo", "C_FlybyRandom")?;
    vm.alias_function("Camera", "SetFlybyMode", "C_FlybyTrack")?;
    for hour in 0..24 {
        vm.set_number(&format!("HOUR_{hour}"), f64::from(hour))?;
    }
    let mut hour = None;
    let mut track = None;
    let mut handler = |id, args: &[Value]| match (id, args) {
        (1, [Value::Bytes(key)]) if key == b"performance" => {
            Ok(Some(f64::from(performance()?.min(cap))))
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
        _ => Err(format!("unsupported frontend flyby callback {id}")),
    };
    vm.execute("FLYBY1/hub.lua", source, &mut handler)?;
    vm.execute("FLYBY1 level event", b"EVENT_Level()", &mut handler)?;
    Ok(Selection {
        hour: hour.ok_or("flyby level event did not set an hour")?,
        track: track.ok_or("flyby level event did not select a track")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Authored fixture: independent table dispatch exercises the host API in CI.
    const HUB: &[u8] = br#"
        function EVENTINITIALISE_Level() unavailable_map_loader() end
        function EVENT_Level()
            local index = Random.ZeroTo(Game.performance) + 1
            local hours = {HOUR_12, HOUR_20, HOUR_8}
            local tracks = {"low", "medium", "high"}
            Calendar.hour = hours[index]
            Camera.SetFlybyMode("gamedata/cut_scenes/intro_" .. tracks[index] .. ".tm")
        end
    "#;

    fn check_selections(source: &[u8]) {
        for (quality, cap) in [("low", 0), ("medium", 1), ("high", 2)] {
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
                let selected = select(source, quality, &mut state, || Ok(2)).unwrap();
                assert_eq!(selected.hour, [12, 20, 8][draw]);
                assert_eq!(
                    selected.track,
                    ["intro_low.tm", "intro_medium.tm", "intro_high.tm"][draw]
                );
                assert_eq!(state, expected_state, "exactly one RNG draw");
                seen[draw] = true;
            }
            for (draw, seen) in seen.into_iter().enumerate() {
                assert_eq!(seen, draw <= cap as usize, "{quality}, draw {draw}");
            }
        }
    }

    #[test]
    fn lua_event_selects_each_hour_and_track_within_quality_cap() {
        check_selections(HUB);
        assert!(performance_cap("ultra").is_err());
    }

    #[test]
    fn live_performance_can_lower_the_cli_cap() {
        for seed in 0..16 {
            let mut state = crate::startup_rng::Snapshot::from_seed(seed).state(0x93aee8);
            let selected = select(HUB, "high", &mut state, || Ok(0)).unwrap();
            assert_eq!(selected.hour, 12);
            assert_eq!(selected.track, "intro_low.tm");
        }
        assert!(select(HUB, "high", &mut 0, || Err("owner absent".into())).is_err());
    }

    #[test]
    fn script_controls_selection_instead_of_a_native_lookup() {
        let source = br#"function EVENT_Level()
            Calendar.hour=HOUR_3
            Camera.SetFlybyMode("gamedata\\cut_scenes\\custom.tm")
        end"#;
        let mut state = 123;
        let selected = select(source, "low", &mut state, || Ok(2)).unwrap();
        assert_eq!(selected.hour, 3);
        assert_eq!(selected.track, "custom.tm");
        assert_eq!(state, 123);
    }

    #[test]
    #[ignore = "requires supplied original FLYBY1 hub script"]
    fn original_level_event_selects_each_hour_and_track_within_quality_cap() {
        let source =
            std::fs::read(crate::fixtures::path("SCRIPTS").join("FLYBY1/hub.lua")).unwrap();
        check_selections(&source);
    }
}
