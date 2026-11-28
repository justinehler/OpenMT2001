//! Original4665f0 initialization after the common player constructor.
use crate::player_manager::{Player, RandomRegistry, put, word};

/// Original construction dependencies, owned by the surrounding world arena.
/// Allocation returns uninitialized bytes; no allocator pattern is invented here.
pub struct Construction<'a> {
    pub image: &'a crate::pe::PeImage<'a>,
    pub registry: &'a mut RandomRegistry,
    pub global_reset: &'a mut u8,
    pub allocate: &'a mut dyn FnMut() -> Result<Option<crate::player_manager::Auxiliary>, String>,
    pub link_tail: &'a mut dyn FnMut(u32, u32) -> Result<(), String>,
}

/// Fresh allocations supplied by the world's allocator. References are logical
/// byte addresses, not host pointers; allocator byte contents remain explicit.
pub struct Allocation {
    pub reference: u32,
    pub bytes: Vec<u8>,
}

pub struct RosterConstruction<'a> {
    pub image: &'a crate::pe::PeImage<'a>,
    pub registry: &'a mut RandomRegistry,
    pub global_reset: &'a mut u8,
    pub allocate: &'a mut dyn FnMut(usize) -> Result<Option<Allocation>, String>,
    pub external_rng_link: &'a mut dyn FnMut(u32, u32) -> Result<(), String>,
}

impl RosterConstruction<'_> {
    pub fn construct(
        &mut self,
        manager: &mut crate::player_manager::Manager,
        kind: i32,
    ) -> Result<Player, String> {
        let size = match kind {
            0 => 0x224,
            1 | 2 => 0x1e50,
            _ => return Err("invalid player constructor kind".into()),
        };
        let allocation = (self.allocate)(size)?.ok_or("player allocation failed")?;
        let mut player = Player {
            reference: allocation.reference,
            bytes: allocation.bytes,
            auxiliaries: vec![],
        };
        Construction {
            image: self.image,
            registry: self.registry,
            global_reset: self.global_reset,
            allocate: &mut || {
                (self.allocate)(0x30)?
                    .map(|allocation| {
                        Ok(crate::player_manager::Auxiliary {
                            reference: allocation.reference,
                            bytes: allocation
                                .bytes
                                .try_into()
                                .map_err(|_| "auxiliary allocation must contain 48 bytes")?,
                        })
                    })
                    .transpose()
            },
            link_tail: &mut |tail, next| {
                if manager.link_rng_tail(tail, next) {
                    Ok(())
                } else {
                    (self.external_rng_link)(tail, next)
                }
            },
        }
        .initialize(&mut player, kind)?;
        Ok(player)
    }
}

impl Construction<'_> {
    /// Run the common constructor and, for kinds1/2, the full AI extension.
    /// Manager::register applies character/name/cash only after this returns.
    pub fn initialize(&mut self, player: &mut Player, kind: i32) -> Result<(), String> {
        let size = match kind {
            0 => 0x224,
            1 | 2 => 0x1e50,
            _ => return Err("invalid player constructor kind".into()),
        };
        if player.bytes.len() < size || !player.auxiliaries.is_empty() {
            return Err("player constructor requires fresh valid storage".into());
        }
        let allocator = std::cell::RefCell::new(&mut self.allocate);
        player.initialize_common(
            self.image.at(0x541c80, 32)?,
            &mut || {
                let mut account = (allocator.borrow_mut())()?.ok_or("account allocation failed")?;
                self.registry
                    .initialize_account(&mut account, &mut |tail, next| {
                        (self.link_tail)(tail, next)
                    })?;
                Ok(account)
            },
            &mut || (allocator.borrow_mut())(),
        )?;
        if kind == 0 {
            put(&mut player.bytes, 0, 0x525e04);
            put(&mut player.bytes, 0x220, 0);
        } else {
            player.initialize_ai_extension(
                self.image,
                self.registry,
                self.global_reset,
                &mut |tail, next| (self.link_tail)(tail, next),
            )?;
        }
        Ok(())
    }
}

impl Player {
    /// Complete the AI-specific portion of4665f0 on common-constructed storage.
    /// The global801dec byte and external RNG links belong to the host arena.
    pub fn initialize_ai_extension(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        registry: &mut RandomRegistry,
        global_reset: &mut u8,
        link_tail: &mut impl FnMut(u32, u32) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.bytes.len() < 0x1e50 || self.reference == 0 {
            return Err("full AI constructor requires common-constructed storage".into());
        }
        let reference = self
            .reference
            .checked_add(0x224)
            .ok_or("AI RNG reference overflow")?;
        let auxiliaries = &mut self.auxiliaries;
        registry.initialize_rng(
            reference,
            &mut self.bytes[0x224..0x238],
            &mut |tail, next| {
                if let Some(record) = auxiliaries
                    .iter_mut()
                    .find(|a| a.reference.checked_add(0x1c) == Some(tail))
                {
                    put(&mut record.bytes, 0x28, next);
                    Ok(())
                } else {
                    link_tail(tail, next)
                }
            },
        )?;
        let b = &mut self.bytes;
        //478ca0: constructor for both ten-record arrays, preserving padding.
        for base in [0x248, 0x658] {
            for i in 0..10 {
                let at = base + i * 0x68;
                for field in [
                    0x48, 0x50, 0x40, 0x3c, 0x38, 0x34, 0x4c, 0, 8, 12, 16, 0x24, 0x20, 0x60,
                ] {
                    put(b, at + field, 0);
                }
                for field in [0x58, 0x5c, 0x18, 0x28] {
                    put(b, at + field, u32::MAX);
                }
                b[at + 0x44..at + 0x46].fill(255);
                b[at + 4] = 99;
            }
        }
        reset_business_state(b);
        //466710's six records are intentionally untouched.
        b[0x13c4..0x13c8].fill(0);
        b[0x13c8..0x13dc].fill(255);
        put(b, 0, 0x525c88);
        put(b, 0xb3c, 0);
        put(b, 0xb40, 0);
        self.reset_ai(image, global_reset, &mut |_| {
            Err("fresh AI lists must be empty".into())
        })
    }

    /// Original466870, called again after character assignment by478ea0.
    /// Retains the embedded RNG; releases previous ownership-list allocations.
    pub fn reset_ai(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        global_reset: &mut u8,
        release: &mut impl FnMut(u32) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.bytes.len() < 0x1e50 {
            return Err("truncated AI reset storage".into());
        }
        let b = &mut self.bytes;
        b[0x1400] = 0;
        put(b, 0xb50, 10000);
        for at in [12, 0x1e38, 0x13fc, 0xa68] {
            put(b, at, u32::MAX);
        }
        let character = word(b, 0x150);
        put(b, 0x1e3c, character);
        put(b, 0x1e40, character);
        reset_business_state(b);
        //47a540 copies a terminated string and two immutable264-byte records.
        let name = image.at(0x53edb8, 0x104)?;
        let length = name
            .iter()
            .position(|v| *v == 0)
            .ok_or("unterminated AI default string")?
            + 1;
        b[0xe3c..0xe3c + length].copy_from_slice(&name[..length]);
        b[0xf40..0x1048].copy_from_slice(image.at(0x53fa18, 0x108)?);
        b[0x1048..0x1150].copy_from_slice(image.at(0x540570, 0x108)?);
        for at in [
            0x1e00, 0x1e20, 0x1e18, 0x1e1c, 0x13f4, 0x13e8, 0x13ec, 0x13f0, 0x220, 0x1e08, 0x23c,
            0x1430, 0x1dfc, 0x244, 0x1e0c, 0x13e0, 0x13e4, 0xa6c, 0xa70, 0xa74, 0x1e28, 0x1e2c,
        ] {
            put(b, at, 0);
        }
        for at in [
            0x13f8, 0xaa8, 0x1402, 0x1404, 0x1e04, 0x1df9, 0x1df8, 0x1405, 0x1403, 0x1e13, 0x1e10,
            0x1e11, 0x1e12, 0x13dc, 0x13dd, 0x1e34, 0x1e45, 0x1e44, 0x1406, 0x13c0, 0x13c1, 0x13c2,
        ] {
            b[at] = 0;
        }
        b[0x1401] = 1;
        b[0x238..0x23b].fill(1);
        *global_reset = 0;
        put(b, 8, 15);
        put(b, 0x240, 2);
        put(b, 0x1e14, 20);
        put(b, 0x1e48, 20);
        b[0xb4c..0xb50].fill(0);
        let mut ownership = crate::ai_ownership::State {
            completed_groups: 0,
            groups: std::array::from_fn(|i| {
                b[0xaac + i * 12..0xaac + (i + 1) * 12].try_into().unwrap()
            }),
            lists: std::array::from_fn(|_| crate::ai_ownership::BusinessList {
                entries: vec![],
                active_count: 0,
            }),
        };
        ownership.reset_from_v14(image)?;
        for (i, group) in ownership.groups.iter().enumerate() {
            b[0xaac + i * 12..0xaac + (i + 1) * 12].copy_from_slice(group);
        }
        //478bc0 links the first ten records.478c60 resets the second array.
        for base in [0x248, 0x658] {
            for i in 0..10 {
                let at = base + i * 0x68;
                for field in [0, 8, 16, 12, 0x60, 0x1c] {
                    put(b, at + field, 0);
                }
                for field in [0x14, 0x18] {
                    put(b, at + field, u32::MAX);
                }
                b[at + 4] = i as u8;
                if base == 0x248 {
                    put(
                        b,
                        at + 0x20,
                        if i == 9 {
                            0
                        } else {
                            self.reference.wrapping_add((at + 0x68) as u32)
                        },
                    );
                    put(
                        b,
                        at + 0x24,
                        if i == 0 {
                            0
                        } else {
                            self.reference.wrapping_add((at - 0x68) as u32)
                        },
                    );
                } else {
                    put(b, at + 12, 1);
                }
            }
        }
        for i in 0..6 {
            b[0xa78 + i * 8] = 0;
            put(b, 0xa7c + i * 8, 0);
        }
        let at = 0xc98;
        for field in [0, 4, 8, 12, 16, 0x14, 0x18, 0x20, 0x2c, 0x30, 0x34] {
            put(b, at + field, 0);
        }
        put(b, at + 0x24, u32::MAX);
        for field in [0x1c, 0x28, 0x38, 0x39] {
            b[at + field] = 0;
        }
        b[0x1408..0x1430].fill(255);
        b[0x1150..0x13c0].fill(0);
        //Original reset clears each pointer before releasing its old allocation.
        for pointer_at in [0xb3c, 0xb40] {
            let old = word(b, pointer_at);
            put(b, pointer_at, 0);
            if old != 0 {
                release(old)?;
            }
            let counters = if pointer_at == 0xb3c { 0xb44 } else { 0xb48 };
            b[counters..counters + 4].fill(0);
        }
        //Six groups of52 pairs of dwords, not six52-byte records.
        b[0x1438..0x1df8].fill(0);
        Ok(())
    }
}

pub(crate) fn reset_business_state(b: &mut [u8]) {
    //4707e0 and470830 write identical fields; the latter repeats them.
    let at = 0xb54;
    put(b, at, 6);
    for field in [8, 0x3c, 0x14, 0x2c] {
        put(b, at + field, u32::MAX);
    }
    for field in [0x28, 12, 16, 0x18, 0x1c] {
        put(b, at + field, 0);
    }
    for field in [
        0x20, 0x21, 0x22, 0x24, 0x25, 0x23, 4, 0x31, 0x32, 0x30, 0x143,
    ] {
        b[at + field] = 0;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original combined roster observations and v1.4 executable"]
    fn matches_native_constructed_roster() {
        use crate::player_manager::{Creation, Manager, RandomRegistry, word};
        fn take<'a>(bytes: &'a [u8], at: &mut usize, size: usize) -> &'a [u8] {
            let result = &bytes[*at..*at + size];
            *at += size;
            result
        }
        fn next(bytes: &[u8], at: &mut usize) -> u32 {
            word(take(bytes, at, 4), 0)
        }
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_PLAYER_ROSTER")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        assert_eq!(&bytes[..8], b"MTPROL02");
        assert_eq!(word(&bytes, 8), 2);
        assert_eq!(word(&bytes, 16), 0x28);
        let mut at = 20;
        for _ in 0..word(&bytes, 12) {
            let name_len = next(&bytes, &mut at) as usize;
            let call_count = next(&bytes, &mut at);
            let fill = next(&bytes, &mut at) as u8;
            let seed = next(&bytes, &mut at);
            let mut reset = next(&bytes, &mut at) as u8;
            let case = std::str::from_utf8(take(&bytes, &mut at, name_len)).unwrap();
            let size = next(&bytes, &mut at) as usize;
            let mut profiles: Vec<[u8; 0x650]> = take(&bytes, &mut at, size)
                .chunks_exact(0x650)
                .map(|p| p.try_into().unwrap())
                .collect();
            assert_eq!(profiles.len(), 10);
            let size = next(&bytes, &mut at) as usize;
            let expected_profiles = take(&bytes, &mut at, size);
            let mut manager = Manager::initialize([fill; 0x28]);
            assert_eq!(
                manager.bytes,
                take(&bytes, &mut at, 0x28),
                "initial manager {case}"
            );
            let expected_manager = take(&bytes, &mut at, 0x28);
            let mut calls = vec![];
            for _ in 0..call_count {
                let kind = next(&bytes, &mut at) as i32;
                let character = next(&bytes, &mut at) as i32;
                let result = next(&bytes, &mut at);
                assert_eq!(next(&bytes, &mut at), 1);
                let size = next(&bytes, &mut at) as usize;
                let name = take(&bytes, &mut at, size);
                let after = take(&bytes, &mut at, 0x28);
                assert_eq!(&after[0x10..], take(&bytes, &mut at, 24));
                calls.push((kind, character, result, name, after));
            }
            let expected_rng: [u32; 5] = std::array::from_fn(|_| next(&bytes, &mut at));
            let expected_reset = next(&bytes, &mut at) as u8;
            assert_eq!(next(&bytes, &mut at), 1);
            let player_count = next(&bytes, &mut at);
            let mut expected_players = vec![];
            for _ in 0..player_count {
                let slot = next(&bytes, &mut at) as usize;
                let reference = next(&bytes, &mut at);
                let size = next(&bytes, &mut at) as usize;
                expected_players.push((slot, reference, take(&bytes, &mut at, size)));
            }
            let allocation_count = next(&bytes, &mut at) as usize;
            let mut allocations = vec![];
            for _ in 0..allocation_count {
                let reference = next(&bytes, &mut at);
                let size = next(&bytes, &mut at) as usize;
                let _return_site = next(&bytes, &mut at);
                let before = take(&bytes, &mut at, size);
                let after = take(&bytes, &mut at, size);
                allocations.push((reference, before, after));
            }
            let mut used = 0;
            let mut registry = RandomRegistry {
                process: [seed, 0x343fd, 0x269ec3],
                head: 0,
                tail: 0,
            };
            let mut allocate = |size| {
                let (reference, before, _) = allocations[used];
                used += 1;
                assert_eq!(size, before.len(), "allocation order {case}");
                Ok(Some(super::Allocation {
                    reference,
                    bytes: before.to_vec(),
                }))
            };
            let mut construction = super::RosterConstruction {
                image: &image,
                registry: &mut registry,
                global_reset: &mut reset,
                allocate: &mut allocate,
                external_rng_link: &mut |_, _| Err("unexpected external RNG node".into()),
            };
            for (kind, character, result, name, after) in calls {
                let actual = manager
                    .register(
                        &mut profiles,
                        Creation {
                            kind,
                            character,
                            name,
                            initial_cash: Manager::initial_cash(&image).unwrap(),
                        },
                        &mut |manager, kind| construction.construct(manager, kind),
                    )
                    .unwrap();
                assert_eq!(u32::from(actual), result & 255, "return AL {case}");
                assert_eq!(manager.bytes, after, "registered manager {case}");
            }
            assert_eq!(used, allocation_count);
            assert_eq!(manager.bytes, expected_manager);
            assert_eq!(profiles.concat(), expected_profiles, "profiles {case}");
            for (slot, reference, expected) in expected_players {
                let actual = &manager.players[slot];
                assert_eq!(actual.reference, reference);
                assert_eq!(actual.bytes, expected, "full player {case}/{slot}");
            }
            for (reference, _, expected) in allocations {
                let actual = manager
                    .players
                    .iter()
                    .find(|p| p.reference == reference)
                    .map(|p| p.bytes.as_slice())
                    .or_else(|| {
                        manager
                            .players
                            .iter()
                            .flat_map(|p| &p.auxiliaries)
                            .find(|a| a.reference == reference)
                            .map(|a| a.bytes.as_slice())
                    })
                    .unwrap();
                assert_eq!(actual, expected, "allocation {case}/{reference:x}");
            }
            assert_eq!(
                [
                    registry.head,
                    registry.tail,
                    registry.process[0],
                    registry.process[1],
                    registry.process[2]
                ],
                expected_rng
            );
            assert_eq!(reset, expected_reset);
        }
        assert_eq!(at, bytes.len());
    }
    use super::*;
    use crate::player_manager::Auxiliary;
    #[test]
    #[ignore = "requires original466870 reset observations"]
    fn matches_native_reset() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_PLAYER_RESET")).unwrap();
        assert_eq!(&data[..8], b"MTAI6870");
        assert_eq!(word(&data, 8), 1);
        let mut at = 20;
        for case in 0..word(&data, 12) {
            let frees = word(&data, at + 16);
            let size = word(&data, at + 20) as usize;
            at += 24;
            let mut player = Player {
                reference: 0x10020000,
                bytes: data[at..at + size].to_vec(),
                auxiliaries: vec![],
            };
            at += size;
            let expected = &data[at..at + size];
            at += size;
            let mut expected_frees = vec![];
            for _ in 0..frees {
                expected_frees.push(word(&data, at));
                at += 16;
            }
            assert_eq!(&data[at..at + 20], &data[at + 20..at + 40]);
            at += 40;
            let expected_reset = word(&data, at + 12) as u8;
            at += 16;
            let mut reset = 255;
            let mut released = vec![];
            player
                .reset_ai(&image, &mut reset, &mut |pointer| {
                    released.push(pointer);
                    Ok(())
                })
                .unwrap();
            let differences = player
                .bytes
                .iter()
                .zip(expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| format!("{i:x}:{a:02x}!={b:02x}"))
                .collect::<Vec<_>>();
            assert!(
                differences.is_empty(),
                "case {case}: {}",
                differences.join(" ")
            );
            assert_eq!(released, expected_frees);
            assert_eq!(reset, expected_reset);
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original4665f0 constructor observations"]
    fn matches_native_full_constructor() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let fixture = std::fs::read(crate::fixtures::path("NATIVE_AI_PLAYER_CONSTRUCTOR")).unwrap();
        assert_eq!(&fixture[..8], b"MTAI4665");
        assert_eq!(word(&fixture, 8), 2);
        let mut at = 20;
        for case in 0..word(&fixture, 12) {
            let fill = word(&fixture, at) as u8;
            let state = word(&fixture, at + 4);
            assert_eq!(word(&fixture, at + 8), 0x100f0000);
            let reference = word(&fixture, at + 12);
            let size = word(&fixture, at + 16) as usize;
            assert_eq!(word(&fixture, at + 20) as usize, size);
            at += 24;
            let mut player = Player {
                reference,
                bytes: fixture[at..at + size].to_vec(),
                auxiliaries: vec![],
            };
            at += size;
            let expected = &fixture[at..at + size];
            at += size;
            let globals = [
                word(&fixture, at),
                word(&fixture, at + 4),
                word(&fixture, at + 8),
            ];
            at += 12;
            let events = word(&fixture, at) as usize;
            at += 4 + events * 8;
            let reset = word(&fixture, at) as u8;
            let count = word(&fixture, at + 4);
            at += 8;
            let mut expected_aux = vec![];
            for _ in 0..count {
                let pointer = word(&fixture, at);
                let size = word(&fixture, at + 4) as usize;
                at += 8;
                assert_eq!(&fixture[at..at + size], vec![fill ^ 0x5a; size]);
                at += size;
                expected_aux.push((pointer, fixture[at..at + size].to_vec()));
                at += size;
            }
            let mut registry = RandomRegistry {
                process: [state, 0x343fd, 0x269ec3],
                head: 0,
                tail: 0,
            };
            let mut global_reset = 0xa5;
            let mut allocation_index = 0;
            Construction {
                image: &image,
                registry: &mut registry,
                global_reset: &mut global_reset,
                allocate: &mut || {
                    let reference = 0x10060000 + allocation_index * 0x1000;
                    allocation_index += 1;
                    Ok(Some(Auxiliary {
                        reference,
                        bytes: [fill ^ 0x5a; 0x30],
                    }))
                },
                link_tail: &mut |_, _| Err("unexpected external RNG".into()),
            }
            .initialize(&mut player, 1)
            .unwrap();
            assert_eq!(allocation_index, 2);
            let differences = player
                .bytes
                .iter()
                .zip(expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| format!("{i:x}:{a:02x}!={b:02x}"))
                .collect::<Vec<_>>();
            assert!(
                differences.is_empty(),
                "case {case}: {}",
                differences.join(" ")
            );
            assert_eq!([registry.head, registry.tail, registry.process[0]], globals);
            assert_eq!(global_reset, reset);
            assert_eq!(player.auxiliaries.len(), expected_aux.len());
            for (actual, (reference, bytes)) in player.auxiliaries.iter().zip(expected_aux) {
                assert_eq!(actual.reference, reference);
                assert_eq!(actual.bytes.as_slice(), bytes);
            }
        }
        assert_eq!(at, fixture.len());
    }
}
