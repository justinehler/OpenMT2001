//! Original00468670 consumes pending AI decisions in a fixed, live-flag order.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Ownership,
    BusinessSettings,
    RentSettings,
    /// Original47c630 receives (1, &choices[1], &choices[0], 0, 0).
    ChooseBusiness,
    Helper(u32),
}

/// Flags are the full original player+220 dword. Only the original tested bits
/// are consumed; callbacks may set later decisions during this same pass.
/// blocked is whether global7f8d38 resolves to an object with nonzero byte7d.
/// Opaque decision helpers remain explicit synchronous callouts.
pub fn advance(
    flags: &mut u32,
    blocked: &mut bool,
    mut callback: impl FnMut(Event, &mut u32, &mut bool, &mut [i32; 2]) -> Result<u32, String>,
) -> Result<(), String> {
    if *flags == 0 {
        return Ok(());
    }
    let mut choices = [-1; 2];
    for (bit, event) in [
        (0x10, Event::Ownership),
        (0x20, Event::Helper(0x46eda0)),
        (0x08, Event::Helper(0x467dc0)),
        (0x2000, Event::Helper(0x467c30)),
        (0x0200, Event::Helper(0x468fc0)),
    ] {
        if *flags & bit != 0 {
            callback(event, flags, blocked, &mut choices)?;
            *flags &= !bit;
        }
    }
    if *flags & 0x40 != 0 {
        if !*blocked {
            callback(Event::Helper(0x47c4f0), flags, blocked, &mut choices)?;
        }
        *flags &= !0x40;
    }
    if *flags & 0x1000 != 0 {
        callback(Event::Helper(0x471d00), flags, blocked, &mut choices)?;
        if callback(Event::Helper(0x468de0), flags, blocked, &mut choices)? as u8 != 0 {
            callback(Event::ChooseBusiness, flags, blocked, &mut choices)?;
        }
        *flags &= !0x1000;
    }
    for (bit, event) in [
        (0x04, Event::Helper(0x4683f0)),
        (0x0100, Event::BusinessSettings),
        (0x01, Event::RentSettings),
        (0x0800, Event::Helper(0x4687e0)),
        (0x0400, Event::Helper(0x468920)),
    ] {
        if *flags & bit != 0 {
            callback(event, flags, blocked, &mut choices)?;
            *flags &= !bit;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original AI decision-dispatch observations"]
    fn matches_native_dispatch() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_DISPATCH").unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTAID468");
        assert_eq!(word(8), 1);
        let mut at = 16;
        for _ in 0..word(12) {
            let len = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&bytes[at..at + len]).unwrap();
            at += len;
            let config: [u32; 8] = std::array::from_fn(|i| word(at + i * 4));
            at += 32;
            let expected_flags = word(at) | (word(at + 4) << 8);
            let count = word(at + 8);
            at += 12;
            let mut expected = Vec::new();
            for _ in 0..count {
                let event: [u32; 10] = std::array::from_fn(|i| word(at + i * 4));
                at += 40;
                if event[0] == 0x47c630 {
                    assert_eq!([event[3], event[4]], [5, 1]);
                }
                expected.push([
                    event[0],
                    event[6] | (event[7] << 8),
                    event[8] | (event[9] << 8),
                ]);
            }
            let mut flags = config[6] | (config[7] << 8);
            let mut blocked = config[3] != 0;
            let mut actual = Vec::new();
            advance(&mut flags, &mut blocked, |event, flags, _, choices| {
                let target = match event {
                    Event::Ownership => 0x467a30,
                    Event::BusinessSettings => 0x467f40,
                    Event::RentSettings => 0x47cc60,
                    Event::ChooseBusiness => {
                        assert_eq!(*choices, [-1; 2]);
                        0x47c630
                    }
                    Event::Helper(address) => address,
                };
                let before = *flags;
                if (target == 0x467a30 && config[4] != 0) || (target == 0x47cc60 && config[5] != 0)
                {
                    *flags |= 0x100;
                }
                actual.push([target, before, *flags]);
                Ok(if target == 0x468de0 { config[2] } else { 0 })
            })
            .unwrap();
            assert_eq!(actual, expected, "AI calls {name}");
            assert_eq!(flags, expected_flags, "AI flags {name}");
        }
        assert_eq!(at, bytes.len());
    }
}
