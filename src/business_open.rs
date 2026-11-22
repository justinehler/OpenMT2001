//! Original48ad80 producer and command11 consumer45110e..451263.
#[derive(Clone, Debug)]
pub struct Command {
    pub kind: u32,
    pub building: u32,
    pub owner: u32,
    pub stock_argument: u8,
}
impl Command {
    pub fn from_completion(
        player: &crate::player_manager::Player,
        kind: u32,
        building: u32,
        stock_argument: u8,
    ) -> Result<Self, String> {
        let owner = u32::from(
            *player
                .bytes
                .get(0x1bc)
                .ok_or("truncated completion player")?,
        )
        .wrapping_add(1);
        Ok(Self {
            kind,
            building,
            owner,
            stock_argument,
        })
    }
    pub fn from_packet(b: &[u8]) -> Result<Self, String> {
        if b.len() != 0x24 || u32::from_le_bytes(b[4..8].try_into().unwrap()) != 0x11 {
            return Err("business open requires command11".into());
        }
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        Ok(Self {
            kind: word(0x14),
            building: word(0x18),
            owner: word(0x1c),
            stock_argument: b[0x20],
        })
    }
    /// Preserve bytes not written by the original stack-object constructor.
    pub fn write_packet(&self, b: &mut [u8; 0x24]) {
        for (at, value) in [
            (0, 0x525de4),
            (4, 0x11),
            (0xc, 0),
            (0x14, self.kind),
            (0x18, self.building),
            (0x1c, self.owner),
        ] {
            b[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        b[0x10] = 1;
        b[0x20] = self.stock_argument;
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub arguments: [u32; 8],
    pub count: usize,
}
pub struct Controls {
    pub network_enabled: bool,
    pub network_override: bool,
}
fn call(address: u32, args: &[u32]) -> Call {
    let mut arguments = [0; 8];
    arguments[..args.len()].copy_from_slice(args);
    Call {
        address,
        arguments,
        count: args.len(),
    }
}
pub fn apply(
    command: &mut Command,
    age: &mut u8,
    controls: &mut Controls,
    mut callback: impl FnMut(Call, &mut Command, &mut u8, &mut Controls) -> Result<u32, String>,
) -> Result<u32, String> {
    let business = callback(
        call(0x4634c0, &[command.kind, command.owner, command.building]),
        command,
        age,
        controls,
    )?;
    if business == u32::MAX {
        callback(call(0x4e6960, &[1, 0, 0x536678]), command, age, controls)?;
    }
    callback(
        call(
            0x463560,
            &[
                business,
                command.building,
                u32::from(command.stock_argument),
            ],
        ),
        command,
        age,
        controls,
    )?;
    *age = age.wrapping_add(100);
    if !controls.network_enabled || controls.network_override {
        callback(
            call(
                0x48cab0,
                &[
                    command.owner.wrapping_sub(1),
                    u32::MAX,
                    0x2f,
                    business,
                    0,
                    0,
                    0,
                    (-999i32) as u32,
                ],
            ),
            command,
            age,
            controls,
        )?;
    }
    // The descriptor is an explicit typed command argument, never a host pointer.
    callback(call(0x4504e0, &[1]), command, age, controls)?;
    callback(call(0x41f410, &[]), command, age, controls)?;
    callback(call(0x407520, &[command.building]), command, age, controls)?;
    callback(call(0x48dc40, &[0x63]), command, age, controls)?;
    Ok(business)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires original construction completion observations"]
    fn matches_native_construction_completion() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_CONSTRUCTION_COMPLETE").unwrap())
            .unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTCCMP01");
        assert_eq!(r.word(), 2);
        let count = r.word();
        for _ in 0..count {
            let n = u16::from_le_bytes(r.take(2).try_into().unwrap()) as usize;
            let name = std::str::from_utf8(r.take(n)).unwrap();
            r.blob();
            let mode = r.word();
            let input: [u32; 10] = std::array::from_fn(|_| r.word());
            let before: [&[u8]; 4] = std::array::from_fn(|_| r.blob());
            let before_age = r.word();
            let before_current = r.word();
            let after: [&[u8]; 4] = std::array::from_fn(|_| r.blob());
            let after_age = r.word();
            let after_current = r.word();
            let packet = r.blob();
            let n = r.word();
            let mut expected = Vec::new();
            for _ in 0..n {
                let target = r.word();
                r.word();
                r.word();
                let n = r.word();
                let mut args: Vec<_> = (0..n).map(|_| r.word()).collect();
                if target == 0x4504e0 {
                    assert_eq!(args.pop(), Some(0x10021000));
                }
                expected.push((target, args));
            }
            assert_ne!(r.word(), 0);
            assert_eq!(r.word(), 0, "{name}");
            assert_eq!(
                before, after,
                "native downstream seams preserve records: {name}"
            );
            assert_eq!(before_current, after_current, "{name}");
            if mode == 0 {
                let player = crate::player_manager::Player {
                    reference: 0x10020000,
                    bytes: before[0].to_vec(),
                    auxiliaries: vec![],
                };
                let command =
                    Command::from_completion(&player, input[0], input[1], input[2] as u8).unwrap();
                let mut actual: [u8; 36] = packet.try_into().unwrap();
                for range in [0..8, 12..17, 20..33] {
                    actual[range].fill(0xff);
                }
                command.write_packet(&mut actual);
                assert_eq!(actual, packet, "{name}");
                assert_eq!(expected, [(0x450490, vec![0])], "{name}");
                assert_eq!(before_age, after_age, "{name}");
            } else {
                let mut command = Command::from_packet(packet).unwrap();
                let mut age = before_age as u8;
                let mut trace = Vec::new();
                let result = apply(
                    &mut command,
                    &mut age,
                    &mut Controls {
                        network_enabled: input[6] != 0,
                        network_override: input[7] != 0,
                    },
                    |call, _, _, _| {
                        trace.push((call.address, call.arguments[..call.count].to_vec()));
                        Ok(if call.address == 0x4634c0 {
                            input[9]
                        } else {
                            0
                        })
                    },
                )
                .unwrap();
                assert_eq!(result, input[9], "{name}");
                assert_eq!(trace, expected, "{name}");
                assert_eq!(
                    (before_age & 0xffffff00) | u32::from(age),
                    after_age,
                    "{name}"
                );
            }
        }
        assert!(r.0.is_empty());
    }
}
