//! Original469210 inbox consumer. World/offer operations stay synchronous;
//! message, profile and controller stores use the retained native allocations.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    player_messages,
};
pub mod offers;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub object: u32,
    pub arguments: [u32; 3],
    pub count: usize,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub value: u32,
    ///4789b0's output offer reference, separate from its boolean return.
    pub offer: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Auction {
    pub participant: u32,
    pub winner: i32,
    pub price: i32,
}
pub struct Context<'a, 'data> {
    pub image: &'a PeImage<'data>,
    pub profiles: &'a mut [[u8; 0x650]],
    pub buildings: &'a [[u8; 0x394]],
    pub auction: Option<Auction>,
}
fn packet(player: &Player) -> Result<&[u8; 48], String> {
    let reference = word(&player.bytes, 0x20c);
    player
        .auxiliaries
        .iter()
        .find(|a| a.reference == reference)
        .map(|a| &a.bytes)
        .ok_or("AI current message unresolved".into())
}
fn offset(player: &Player, reference: u32, size: usize) -> Result<usize, String> {
    let at = reference
        .checked_sub(player.reference)
        .ok_or("AI offer outside player allocation")? as usize;
    if at
        .checked_add(size)
        .is_none_or(|end| end > player.bytes.len())
    {
        return Err("AI offer outside player allocation".into());
    }
    Ok(at)
}
fn remove_pending(player: &mut Player, category: u32) {
    if let Some(index) = (0..10).find(|&i| word(&player.bytes, 0x1408 + i * 4) == category) {
        put(&mut player.bytes, 0x1408 + index * 4, u32::MAX);
        player.bytes[0x1406] = player.bytes[0x1406].wrapping_sub(1);
    }
}
fn debit_offer(player: &mut Player, reference: u32) -> Result<usize, String> {
    let at = offset(player, reference, 0x68)?;
    let amount =
        crate::x87::Sum::from_f32(f32::from_bits(word(&player.bytes, at + 0x60)))?.ftol_low_word();
    let mut remaining = crate::x87::Sum::from_f32(f32::from_bits(word(&player.bytes, 0x1430)))?;
    remaining.add(crate::x87::Sum::from_i32(amount).negated());
    put(&mut player.bytes, 0x1430, remaining.to_f32().to_bits());
    Ok(at)
}
fn finish_offer(player: &mut Player, at: usize) {
    if word(&player.bytes, at + 8) != 3 {
        put(&mut player.bytes, at + 8, 3);
        put(&mut player.bytes, at + 0x10, 0);
    }
}

pub fn consume(
    player: &mut Player,
    context: &mut Context<'_, '_>,
    mut callback: impl FnMut(Call, &mut Player, &mut Context<'_, '_>) -> Result<Reply, String>,
) -> Result<(), String> {
    if player.bytes.len() < 0x1e50 {
        return Err("AI message player truncated".into());
    }
    let Some(reference) = player_messages::pop(player, |_, _| Ok(()))? else {
        return Ok(());
    };
    put(&mut player.bytes, 0x20c, reference);
    let mut call =
        |address, object, arguments: &[u32], player: &mut Player, context: &mut Context<'_, '_>| {
            let mut args = [0; 3];
            args[..arguments.len()].copy_from_slice(arguments);
            callback(
                Call {
                    address,
                    object,
                    arguments: args,
                    count: arguments.len(),
                },
                player,
                context,
            )
        };
    let kind = word(packet(player)?, 4);
    match kind {
        0x33 => {
            let card = word(packet(player)?, 0x18);
            let aux_ref = word(&player.bytes, 0x21c);
            let aux = player
                .auxiliaries
                .iter_mut()
                .find(|a| a.reference == aux_ref)
                .ok_or("AI event auxiliary unresolved")?;
            if word(&aux.bytes, 0x14) != player.reference {
                return Err("AI event auxiliary owner unresolved".into());
            }
            put(&mut aux.bytes, 4, card);
            put(&mut player.bytes, 0x1b8, card);
            put(&mut aux.bytes, 0x18, 1);
        }
        0x2b => player.bytes[0x1402] = 1,
        0x2c => player.bytes[0x1402] = 0,
        0x2d => {
            player.bytes[0x1400] = 1;
            let character = word(&player.bytes, 0x150) as usize;
            let profile = context
                .profiles
                .get_mut(character)
                .ok_or("AI message character leaves profiles")?;
            crate::ai_parameters::reset_profile(profile, &mut player.bytes[0x1400]);
            //470880(9) is an actual ret4 in v1.4; it has no state writes.
        }
        0x24 => {
            player.bytes[0xb74] = 1;
            player.bytes[0xb86] = 0;
            put(&mut player.bytes, 12, u32::MAX);
        }
        0x23 | 0x26 => {
            if kind == 0x26 {
                player.bytes[0xb74] = 1;
            }
            call(
                0x4779b0,
                player.reference,
                &[if kind == 0x26 { 2 } else { 1 }, reference],
                player,
                context,
            )?;
            if kind == 0x26 {
                let category = word(packet(player)?, 0x18);
                remove_pending(player, category);
                put(&mut player.bytes, 12, u32::MAX);
            }
        }
        0x25 => return Err("original4693d3 dereferences its null local offer pointer".into()),
        0x27 => {
            let category = word(packet(player)?, 0x18);
            let offer = call(0x478e00, player.reference, &[category], player, context)?.value;
            let current = word(&player.bytes, 0x20c);
            player.bytes[0x1df8] = 1;
            call(
                0x4699c0,
                player.reference,
                &[offer, current],
                player,
                context,
            )?;
        }
        0x28 => {
            call(0x478950, player.reference, &[], player, context)?;
            for at in [0x1df8, 0x1df9, 0xb85, 0xb86, 0x13c5] {
                player.bytes[at] = 0;
            }
            put(&mut player.bytes, 0xb64, 0);
            player.bytes[0xb74] = 1;
            put(&mut player.bytes, 0xb70, 10);
            put(&mut player.bytes, 0xcb8, 0);
            let state = word(&player.bytes, 0x224)
                .wrapping_mul(word(&player.bytes, 0x228))
                .wrapping_add(word(&player.bytes, 0x22c));
            put(&mut player.bytes, 0x224, state);
            put(&mut player.bytes, 0x1e48, ((state >> 16) & 0x7fff) % 11);
            let mode = word(&player.bytes, 0x244);
            put(&mut player.bytes, 0x240, mode);
            player.bytes[0xcc0] = 0;
            put(&mut player.bytes, 12, u32::MAX);
            let auction = context
                .auction
                .ok_or("AI auction completion has no auction record")?;
            if auction.participant == u32::from(player.bytes[0x1bc]) {
                player.bytes[0x220] |= 0x10;
            }
            let character = word(&player.bytes, 0x1e3c);
            put(&mut player.bytes, 0x1e38, u32::MAX);
            put(&mut player.bytes, 0x1e40, character);
        }
        0x2a => {
            let category = word(packet(player)?, 0x18);
            let offer = call(0x478e00, player.reference, &[category], player, context)?.value;
            if offer != 0 {
                let at = debit_offer(player, offer)?;
                finish_offer(player, at);
                call(0x478950, player.reference, &[], player, context)?;
            }
            let mode = word(&player.bytes, 0x244);
            put(&mut player.bytes, 0x240, mode);
        }
        0x29 => {
            // The first argument is a host-neutral out-reference, returned in Reply.
            let result = call(0x4789b0, player.reference, &[0, reference], player, context)?;
            if result.value as u8 != 0 {
                let at = debit_offer(player, result.offer)?;
                let category = word(packet(player)?, 0x18);
                if call(0x476920, 0, &[category], player, context)?.value as u8 == 0 {
                    let category = word(packet(player)?, 0x18);
                    let object = category.wrapping_mul(0x17c).wrapping_add(0x617628);
                    call(0x445cc0, object, &[], player, context)?;
                }
                let category = word(packet(player)?, 0x18);
                if (category as i32) < 0 {
                    call(0x4e6960, 0, &[0, 0, 0x53cb1c], player, context)?;
                }
                let table = category
                    .checked_mul(88)
                    .and_then(|n| n.checked_add(0x5385a4))
                    .ok_or("AI category table address overflow")?;
                let group = usize::from(context.image.at(table, 1)?[0]);
                let group_at = 0xaac + group * 12;
                if group_at + 12 > player.bytes.len() {
                    return Err("AI ownership group leaves player".into());
                }
                let mask = 1_u32 << (category & 31);
                let bits = word(&player.bytes, group_at + 8);
                if bits & mask == 0 {
                    put(&mut player.bytes, group_at + 8, bits | mask);
                    player.bytes[group_at] = player.bytes[group_at].wrapping_add(1);
                    player.bytes[group_at + 6] = player.bytes[group_at + 6].wrapping_sub(1);
                }
                if player.bytes[group_at + 6] == 0 {
                    player.bytes[0xaa8] = player.bytes[0xaa8].wrapping_add(1);
                }
                let message = packet(player)?;
                let price = word(message, 0x20) as i32;
                let category = word(message, 0x18);
                let row = (category as usize)
                    .checked_mul(12)
                    .and_then(|n| n.checked_add(0x1150))
                    .ok_or("AI category history address overflow")?;
                if row + 12 > player.bytes.len() {
                    return Err("AI category history leaves player".into());
                }
                player.bytes[row] = 1;
                put(&mut player.bytes, row + 4, (price as f32).to_bits());
                put(&mut player.bytes, row + 8, 5);
                finish_offer(player, at);
                let category = word(packet(player)?, 0x18);
                remove_pending(player, category);
            } else {
                let auction = context
                    .auction
                    .ok_or("AI auction observation has no auction record")?;
                let category = word(packet(player)?, 0x18) as i32;
                if !(0..=52).contains(&category) {
                    call(0x4e6960, 0, &[0, 0, 0x53cc80], player, context)?;
                }
                if !(-1..=6).contains(&auction.winner) {
                    call(0x4e6960, 0, &[0, 0, 0x53cc4c], player, context)?;
                }
                // The original permits -1/52/6 before doing pointer arithmetic;
                // retain any resulting in-allocation alias, reject host overrun.
                let at = 0x1438_i64 + (i64::from(category) + 52 * i64::from(auction.winner)) * 8;
                let at = usize::try_from(at).map_err(|_| "AI auction history underflow")?;
                if at + 8 > player.bytes.len() {
                    return Err("AI auction history leaves player".into());
                }
                put(&mut player.bytes, at, (auction.price as f32).to_bits());
                let age = call(0x47d730, 0, &[player.reference], player, context)?.value;
                put(&mut player.bytes, at + 4, age);
                player.bytes[0xb84] = 1;
            }
        }
        0x2e => {
            let building = word(packet(player)?, 0x18) as usize;
            let business = word(
                context
                    .buildings
                    .get(building)
                    .ok_or("AI message building outside pool")?,
                0x1c,
            );
            call(0x46ae20, player.reference, &[business, 0], player, context)?;
        }
        0x2f => {
            let business = word(packet(player)?, 0x18);
            call(0x46aa40, player.reference, &[business, 0], player, context)?;
        }
        0x30 => {
            let p = packet(player)?;
            let args = [word(p, 0x18), word(p, 0x1c)];
            call(0x472620, 0, &args, player, context)?;
        }
        _ => {
            let message = match kind {
                0x22 => 0x53cc10,
                0x31 => 0x53cbd4,
                0x34 => 0x53cb98,
                _ => 0x53cb7c,
            };
            call(0x4e6960, 0, &[1, 0, message], player, context)?;
        }
    }
    Ok(())
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
    #[ignore = "requires original469210 message observations"]
    fn matches_native_messages() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_MESSAGES").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTAMSG01");
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let config: [u32; 8] = std::array::from_fn(|_| r.word());
            let before: [&[u8]; 5] = std::array::from_fn(|_| r.blob());
            let after: [&[u8]; 5] = std::array::from_fn(|_| r.blob());
            let mut expected: Vec<Vec<u32>> = (0..r.word())
                .map(|_| (0..r.word()).map(|_| r.word()).collect())
                .collect();
            let count = word(before[0], 0x214);
            if count != 0 {
                let free = expected.remove(0);
                assert_eq!(free[0], 0x4e6060);
                assert_eq!(free[3], 0x10015000);
            }
            let mut player = Player {
                reference: 0x10010000,
                bytes: before[0].to_vec(),
                auxiliaries: vec![],
            };
            for (reference, index) in [(0x10013000, 2), (0x10014000, 3), (0x10015000, 4)] {
                player.auxiliaries.push(crate::player_manager::Auxiliary {
                    reference,
                    bytes: before[index].try_into().unwrap(),
                });
            }
            let mut profiles = vec![before[1].try_into().unwrap()];
            let mut buildings = vec![[0; 0x394]; 600];
            put(&mut buildings[2], 0x1c, 7);
            let mut context = Context {
                image: &image,
                profiles: &mut profiles,
                buildings: &buildings,
                auction: Some(Auction {
                    participant: config[4],
                    winner: config[5] as i32,
                    price: config[6] as i32,
                }),
            };
            let mut trace = vec![];
            let result = consume(&mut player, &mut context, |call, p, _| {
                let mut row = vec![call.address, call.object, call.count as u32];
                row.extend_from_slice(&call.arguments[..call.count]);
                trace.push(row);
                if call.address == 0x4779b0 && config[3] != 0 {
                    let scratch = p
                        .auxiliaries
                        .iter_mut()
                        .find(|a| a.reference == 0x10013000)
                        .unwrap();
                    put(&mut scratch.bytes, 0x18, 99);
                    put(&mut p.bytes, 0x140c, 99);
                }
                Ok(Reply {
                    value: match call.address {
                        0x478e00 => {
                            if config[0] != 0 {
                                0x10010248
                            } else {
                                0
                            }
                        }
                        0x4789b0 => config[1],
                        0x476920 => config[2],
                        0x47d730 => 77,
                        _ => 0,
                    },
                    offer: 0x10010658,
                })
            });
            assert_eq!(result.is_err(), config[7] != 0, "{name}: {result:?}");
            assert_eq!(trace, expected, "calls {name}");
            assert_eq!(player.bytes, after[0], "player {name}");
            assert_eq!(profiles[0], after[1], "profile {name}");
            for (reference, index) in [(0x10013000, 2), (0x10014000, 3)] {
                assert_eq!(
                    player
                        .auxiliaries
                        .iter()
                        .find(|a| a.reference == reference)
                        .unwrap()
                        .bytes,
                    after[index],
                    "auxiliary {name}/{index}"
                );
            }
            assert_eq!(player.auxiliaries.len(), if count == 0 { 3 } else { 2 });
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires whole original ownership/count inbox executions"]
    fn matches_native_business_inbox() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_BUSINESS_INBOX").unwrap()).unwrap();
        let exe = std::fs::read("work/patch14/payload/Essential/mc.exe").unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIBIN1");
        assert_eq!(r.word(), 0x37f);
        for case in 0..r.word() {
            let _kind = r.word();
            let b: Vec<_> = (0..9).map(|_| r.blob()).collect();
            let a: Vec<_> = (0..4).map(|_| r.blob()).collect();
            let expected_alloc: Vec<_> = (0..r.word()).map(|_| r.word()).collect();
            let frees: Vec<_> = (0..r.word()).map(|_| r.word()).collect();
            assert_eq!(frees[0], 0x10016000);
            let mut player = Player {
                reference: 0x10010000,
                bytes: b[0].to_vec(),
                auxiliaries: vec![
                    crate::player_manager::Auxiliary {
                        reference: 0x10015000,
                        bytes: b[1].try_into().unwrap(),
                    },
                    crate::player_manager::Auxiliary {
                        reference: 0x10016000,
                        bytes: b[2].try_into().unwrap(),
                    },
                ],
            };
            let mut storage = crate::ai_ownership::Storage::default();
            storage.lists.insert(
                0x10020000,
                b[3].chunks_exact(28)
                    .map(|v| v.try_into().unwrap())
                    .collect(),
            );
            let buildings: Vec<[u8; 916]> = b[4]
                .chunks_exact(916)
                .map(|v| v.try_into().unwrap())
                .collect();
            let businesses: Vec<[u8; 544]> = b[5]
                .chunks_exact(544)
                .map(|v| v.try_into().unwrap())
                .collect();
            let mut categories = [[0; 380]; 52];
            categories[0] = b[6].try_into().unwrap();
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                b[7].chunks_exact(116)
                    .map(|v| v.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            );
            let mut counts: [u32; 52 * 45] = std::array::from_fn(|i| word(b[8], i * 4));
            let mut profiles = [[0; 0x650]; 10];
            let mut alloc = vec![];
            let mut context = Context {
                image: &image,
                profiles: &mut profiles,
                buildings: &buildings,
                auction: None,
            };
            consume(&mut player, &mut context, |call, player, _| {
                match call.address {
                    0x46aa40 => storage.mutate(
                        player,
                        |size| {
                            alloc.push(size as u32);
                            Ok(0x10040000)
                        },
                        |state| {
                            state.insert(
                                call.arguments[1] as usize,
                                call.arguments[0] as i32,
                                |id| {
                                    crate::ai_ownership::live_entry_value_from_catalog(
                                        id,
                                        &businesses,
                                        &buildings,
                                        &categories,
                                        &catalog,
                                    )
                                },
                            )
                        },
                    )?,
                    0x46ae20 => storage.mutate(
                        player,
                        |_| panic!("remove allocated"),
                        |state| {
                            state.lists[call.arguments[1] as usize]
                                .remove(call.arguments[0] as i32);
                            Ok(())
                        },
                    )?,
                    0x472620 => crate::ai_business::remove_category_count(
                        &mut counts,
                        call.arguments[0],
                        call.arguments[1],
                    )?,
                    _ => panic!("unexpected native business inbox callback {call:?}"),
                }
                Ok(Reply::default())
            })
            .unwrap();
            assert_eq!(player.bytes, a[0], "case{case} player");
            assert_eq!(player.auxiliaries.len(), 1);
            assert_eq!(player.auxiliaries[0].bytes, a[1], "case{case} scratch");
            let state = storage.state(&player).unwrap();
            assert_eq!(
                state.lists[0].entries.as_flattened(),
                a[2],
                "case{case} ownership rows"
            );
            for (i, value) in counts.iter().enumerate() {
                assert_eq!(*value, word(a[3], i * 4), "case{case} count{i}");
            }
            assert_eq!(alloc, expected_alloc, "case{case} allocation");
        }
        assert!(r.0.is_empty());
    }
}
