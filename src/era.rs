//! Original4806b0 cumulative era parameter reload, music switch and event schedule.
use crate::clock::Clock;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Parameters(i32),
    Music {
        era: u32,
        repeat: bool,
    },
    Script {
        name: u32,
        delay: u32,
        argument: i32,
    },
}
///The parameter-loop bound is captured from the argument. Hour and current era
///are read only after loading all files; callbacks may change those live words.
pub fn advance(
    requested: i32,
    clock: &mut Clock,
    mut callback: impl FnMut(Call, &mut Clock) -> Result<(), String>,
) -> Result<(), String> {
    let mut remaining = requested.wrapping_add(1);
    let mut year = 1930i32;
    while remaining > 0 {
        callback(Call::Parameters(year), clock)?;
        year = year.wrapping_add(10);
        remaining = remaining.wrapping_sub(1);
    }
    //44de20 is a literal RET in v1.4.
    let announce = (clock.calendar[0] as i32) < 2;
    let era = clock.calendar[2];
    if era <= 6 {
        callback(Call::Music { era, repeat: true }, clock)?;
        if announce && (1..=5).contains(&era) {
            let name = [0x5418dc, 0x5418c8, 0x5418b4, 0x5418a0, 0x54188c][era as usize - 1];
            callback(
                Call::Script {
                    name,
                    delay: 1250,
                    argument: 0,
                },
                clock,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original4806b0 transition observations"]
    fn matches_native_transition() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_ERA_TRANSITION")).unwrap();
        assert_eq!(&bytes[..8], b"MTERAT01");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        assert_eq!(word(&mut at), 0x37f);
        for case in 0..word(&mut at) {
            let h: [u32; 9] = std::array::from_fn(|_| word(&mut at));
            let expected: Vec<[u32; 4]> = (0..word(&mut at))
                .map(|_| std::array::from_fn(|_| word(&mut at)))
                .collect();
            let backend: Vec<[u32; 2]> = (0..word(&mut at))
                .map(|_| std::array::from_fn(|_| word(&mut at)))
                .collect();
            let mut clock = Clock {
                calendar: [h[1], 0, h[2], 0, 0],
                timing: [0; 6],
            };
            let mut trace = vec![];
            advance(h[0] as i32, &mut clock, |call, clock| {
                trace.push(match call {
                    Call::Parameters(year) => {
                        if h[3] != 0 {
                            clock.calendar[0] = h[4] % 3;
                            clock.calendar[2] = (h[4] + 1) % 9;
                        }
                        [0, year as u32, 0, 0]
                    }
                    Call::Music { era, repeat } => [1, era, u32::from(repeat), 0],
                    Call::Script {
                        name,
                        delay,
                        argument,
                    } => [2, name, delay, argument as u32],
                });
                Ok(())
            })
            .unwrap();
            assert_eq!(trace, expected, "case{case}");
            assert_eq!([clock.calendar[0], clock.calendar[2]], [h[5], h[6]]);
            let music: Vec<_> = trace
                .iter()
                .filter(|r| r[0] == 1 && h[7] != 0)
                .map(|r| [r[1], r[2]])
                .collect();
            assert_eq!(music, backend);
            assert_eq!(
                h[8],
                if backend.is_empty() {
                    0xaabbccdd
                } else {
                    0x12345678
                }
            );
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[cfg(feature = "lua4")]
    #[ignore = "requires original era Lua source and executable"]
    fn original_delayed_business_introductions() {
        use crate::lua4::{Runtime, Value};
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let path = crate::fixtures::path("DEFAULT_SETTINGS");
        let general = std::fs::read(path.parent().unwrap().join("general.lua")).unwrap();
        let mut vm = Runtime::new().unwrap();
        vm.register_event_triggers(1).unwrap();
        vm.register_coerced("C_PlaySound", 13, "S").unwrap();
        vm.register("CapturePrompt", 14).unwrap();
        vm.set_number("ID_PROMPT_1_OFFSET", 1000.).unwrap();
        vm.set_number("true", 1.).unwrap();
        for (name, index) in crate::shopping::business_names(&image).unwrap() {
            vm.set_number(name, index as f64).unwrap();
        }
        for (name, index) in crate::resident::commodity_names(&image).unwrap() {
            vm.set_number(name, index as f64).unwrap();
        }
        vm.execute("era tables",b"BusinessInfo={}; Commodity={}; Interface={CreatePrompt=CapturePrompt}; for i=0,44 do BusinessInfo[i]={} end; for i=0,31 do Commodity[i]={} end",&mut |_,_|unreachable!()).unwrap();
        vm.execute("original general", &general, &mut |_, _| unreachable!())
            .unwrap();
        let mut events = crate::script_events::Events::default();
        for (era, expected_businesses, expected_commodities) in
            [(1, 4, 3), (2, 8, 6), (3, 12, 7), (4, 16, 8), (5, 20, 10)]
        {
            let mut clock = Clock {
                calendar: [0, 0, era, 0, 0],
                timing: [0; 6],
            };
            advance(era as i32, &mut clock, |call, _| {
                if let Call::Script {
                    name,
                    delay,
                    argument,
                } = call
                {
                    let raw = image.at(name, 32)?;
                    let n = raw.iter().position(|b| *b == 0).unwrap();
                    let name = std::str::from_utf8(&raw[..n]).unwrap();
                    vm.execute(
                        "native era admission",
                        format!("C_TriggerEvent('{name}',{delay},{argument})").as_bytes(),
                        &mut |id, args| match id {
                            1 => Ok(Some(0.)),
                            2 => {
                                let [
                                    Value::Number(reference),
                                    Value::Number(delay),
                                    Value::Number(argument),
                                ] = args
                                else {
                                    panic!()
                                };
                                events.append(crate::script_events::Event {
                                    reference: *reference as u32,
                                    remaining: *delay as u32,
                                    argument: *argument as i32,
                                });
                                Ok(None)
                            }
                            _ => panic!("unexpected initialization callback"),
                        },
                    )?;
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(events.pending().len(), 1);
            assert_eq!(events.pending()[0].remaining, 1250);
            for _ in 0..1249 {
                vm.tick_events(&mut events, &mut |_, _, _| panic!("early event callback"))
                    .unwrap();
            }
            assert_eq!(events.pending()[0].remaining, 1);
            vm.tick_events(&mut events, &mut |_, _, _| {
                panic!("numeric zero bypass must not emit a prompt/sound")
            })
            .unwrap();
            assert!(events.pending().is_empty());
            vm.execute("era census",b"businesses=0; commodities=0; for i=0,44 do if BusinessInfo[i].available then businesses=businesses+1 end end; for i=0,31 do if Commodity[i].active then commodities=commodities+1 end end",&mut |_,_|unreachable!()).unwrap();
            assert_eq!(
                vm.number("businesses").unwrap(),
                Some(f64::from(expected_businesses))
            );
            assert_eq!(
                vm.number("commodities").unwrap(),
                Some(f64::from(expected_commodities))
            );
        }
        let mut calls = vec![];
        vm.execute(
            "original prompt branch",
            b"EVENT_BusinessIntro1940(nil); C_PlaySound('ignored',42); C_PlaySound();",
            &mut |id, args| {
                calls.push((id, args.to_vec()));
                Ok(None)
            },
        )
        .unwrap();
        assert_eq!(calls[0].0, 14);
        assert_eq!(
            calls[1],
            (13, vec![Value::Bytes(b"harpstrum.wav".to_vec())])
        );
        assert_eq!(calls[2], (13, vec![Value::Bytes(b"42".to_vec())]));
        assert_eq!(calls[3], (13, vec![Value::Nil]));
    }
}
