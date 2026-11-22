//! Original469bf0 retained offer execution. Producers are synchronous boundaries;
//! the offer is changed/cancelled only after its corresponding producer returns.
use crate::{
    player_manager::{Player, put, word},
    x87::Sum,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    CategoryValue(u32),
    Placement {
        category: u32,
        rectangle: [u32; 4],
    },
    RemoveOwnedBusiness(u32),
    Producer {
        address: u32,
        arguments: [u32; 11],
        count: usize,
    },
    AuctionBudget,
    Diagnostic(u32),
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Reply {
    pub value: u32,
    pub extended: Sum,
}
#[derive(Clone, Copy)]
pub struct Colors<'a> {
    pub banks: [&'a [u32]; 3],
    pub players: &'a [i32],
}
pub struct World<'a> {
    /// Owner bytes are read before execution issues any producer callback.
    pub category_owners: Vec<i8>,
    pub buildings: &'a [[u8; 916]],
    pub auction: Option<&'a mut [u8; 128]>,
    /// Original866fb8/8680d8/82cfb8 variable-length colour-index banks.
    /// Live53c9dc slot-index array and colour banks are required only on construction.
    pub colors: Option<Colors<'a>>,
    pub event_gate: bool,
}
fn producer(address: u32, args: &[u32]) -> Call {
    let mut arguments = [0; 11];
    arguments[..args.len()].copy_from_slice(args);
    Call::Producer {
        address,
        arguments,
        count: args.len(),
    }
}
fn release_reservation(p: &mut Player, at: usize) -> Result<(), String> {
    let amount =
        crate::economy::ftol_low_word(f64::from(f32::from_bits(word(&p.bytes, at + 0x60))));
    let mut reserved = Sum::from_f32(f32::from_bits(word(&p.bytes, 0x1430)))?;
    reserved.add(Sum::from_i32(amount).negated());
    put(&mut p.bytes, 0x1430, reserved.to_f32().to_bits());
    Ok(())
}
fn completed(p: &mut Player, at: usize) {
    if word(&p.bytes, at + 8) != 3 {
        put(&mut p.bytes, at + 8, 3);
        put(&mut p.bytes, at + 0x10, 0);
    }
}
fn next(p: &mut Player) -> u32 {
    let state = word(&p.bytes, 0x224)
        .wrapping_mul(word(&p.bytes, 0x228))
        .wrapping_add(word(&p.bytes, 0x22c));
    put(&mut p.bytes, 0x224, state);
    (state >> 16) & 0x7fff
}
fn auction<'a>(world: &'a World<'_>) -> Result<&'a [u8; 128], String> {
    world
        .auction
        .as_deref()
        .ok_or("offer requires active auction allocation".into())
}

///4477e0 retains445a20's x87 prestige factor through both multiplications,
/// rounds through original FTOL, then returns the scaled integer in ST0.
pub fn category_value(category: &[u8; 380], image: &crate::pe::PeImage<'_>) -> Result<Sum, String> {
    let mut value = Sum::from_f32(f32::from_bits(word(category, 0x84)))?;
    value.mul_f32(image.f32_at(0x52502c)?)?;
    value.add_f32(image.f32_at(0x525034)?)?;
    value.mul_f32(image.f32_at(0x525b20)?)?;
    value.mul_f32(image.f32_at(0x525b1c)?)?;
    value.add(Sum::from_f64(f64::from_le_bytes(
        image.at(0x5250e8, 8)?.try_into().unwrap(),
    ))?);
    let mut result = Sum::from_i32(value.ftol_low_word());
    result.mul_f32(image.f32_at(0x525248)?)?;
    Ok(result)
}
///445ef0 accepts x,y,width,depth and converts to445b10's inclusive endpoints.
pub fn placement(category: &[u8; 380], rectangle: [u32; 4]) -> bool {
    let [x, y, width, depth] = rectangle;
    crate::building_pool::can_place(
        category[0x9c..0xcd].try_into().unwrap(),
        [
            x as i32,
            y as i32,
            x.wrapping_add(width).wrapping_sub(1) as i32,
            y.wrapping_add(depth).wrapping_sub(1) as i32,
        ],
    )
}

pub fn execute(
    player: &mut Player,
    reference: u32,
    world: &mut World<'_>,
    mut call: impl FnMut(Call, &mut Player, &mut World<'_>) -> Result<Reply, String>,
) -> Result<(), String> {
    let at = super::offset(player, reference)?;
    match word(&player.bytes, at + 0x10) {
        0 | 9 | 12 => (),
        1 => {
            let category = word(&player.bytes, at + 0x14);
            let owner = *world.category_owners.get(category as usize).ok_or("offer property outside pool")?;
            let amount = f32::from_bits(word(&player.bytes, at + 0x60));
            let cash = f32::from_bits(word(&player.bytes, 4));
            let reserved = f32::from_bits(word(&player.bytes, 0x1430));
            let reject = if i32::from(owner) == i32::from(player.bytes[0x1bc]) + 1
                || amount.is_nan()
                || cash.is_nan()
                || reserved.is_nan()
            {
                true
            } else {
                let mut available = Sum::from_f32(cash)?;
                available.add(Sum::from_f32(reserved)?.negated());
                available.add(Sum::from_i32(word(&player.bytes, 0xb50) as i32));
                available.compare(Sum::from_f32(amount)?).is_lt()
            };
            if reject {
                release_reservation(player, at)?;
                super::cancel(player, reference)?;
            } else {
                let identity = word(&player.bytes, at);
                put(&mut player.bytes, 12, identity);
                call(
                    producer(
                        0x48a640,
                        &[
                            category,
                            word(&player.bytes, at + 0x60),
                            word(&player.bytes, at),
                        ],
                    ),
                    player,
                    world,
                )?;
                put(&mut player.bytes, at + 0x10, 9);
                put(&mut player.bytes, at + 8, 2);
                put(&mut player.bytes, 0xb64, 0);
                let count = usize::from(player.bytes[0x1406]);
                if count < 5 {
                    put(&mut player.bytes, 0x1408 + count * 4, category);
                    player.bytes[0x1406] = player.bytes[0x1406].wrapping_add(1);
                }
            }
        }
        2 => {
            let value = call(
                Call::CategoryValue(word(&player.bytes, at + 0x14)),
                player,
                world,
            )?
            .extended
            .to_f32();
            call(
                producer(
                    0x48a690,
                    &[word(&player.bytes, at + 0x14), value.to_bits(), u32::MAX],
                ),
                player,
                world,
            )?;
            let category = word(&player.bytes, at + 0x14) as usize;
            let row = 0x1150 + category * 12;
            if row + 12 > player.bytes.len() {
                return Err("offer category state outside player".into());
            }
            player.bytes[row] = 0;
            put(&mut player.bytes, row + 4, 0);
            put(&mut player.bytes, row + 8, 0);
            super::cancel(player, reference)?;
        }
        4 => {
            call(
                Call::RemoveOwnedBusiness(word(&player.bytes, at + 0x2c)),
                player,
                world,
            )?;
            call(
                producer(0x48ae60, &[word(&player.bytes, at + 0x2c)]),
                player,
                world,
            )?;
            super::cancel(player, reference)?;
        }
        6 => {
            call(
                Call::RemoveOwnedBusiness(word(&player.bytes, at + 0x2c)),
                player,
                world,
            )?;
            call(
                producer(
                    0x48ad80,
                    &[
                        word(&player.bytes, at + 0x28),
                        u32::from(u16::from_le_bytes(
                            player.bytes[at + 0x44..at + 0x46].try_into().unwrap(),
                        )),
                        0,
                    ],
                ),
                player,
                world,
            )?;
            super::cancel(player, reference)?;
        }
        7 => {
            put(&mut player.bytes, 0xb5c, u32::MAX);
            call(
                producer(
                    0x48ad20,
                    &[
                        word(&player.bytes, at + 0x14),
                        u32::from(u16::from_le_bytes(
                            player.bytes[at + 0x44..at + 0x46].try_into().unwrap(),
                        )),
                        word(&player.bytes, at + 0x4c),
                    ],
                ),
                player,
                world,
            )?;
            let building = usize::from(u16::from_le_bytes(
                player.bytes[at + 0x44..at + 0x46].try_into().unwrap(),
            ));
            let business = word(
                world
                    .buildings
                    .get(building)
                    .ok_or("offer demolition building outside pool")?,
                0x1c,
            );
            call(Call::RemoveOwnedBusiness(business), player, world)?;
            //50fc80 reads only CL; values>=64 produce zero.
            let shift = word(&player.bytes, at + 0x14) as u8;
            let mask = if shift < 64 { 1u64 << shift } else { 0 };
            let bits = u64::from(word(&player.bytes, 0x1e28))
                | (u64::from(word(&player.bytes, 0x1e2c)) << 32);
            if bits & mask != 0 {
                put(&mut player.bytes, 0x1e28, (bits & !mask) as u32);
                put(&mut player.bytes, 0x1e2c, ((bits & !mask) >> 32) as u32);
            }
            super::cancel(player, reference)?;
        }
        8 => {
            let category = word(&player.bytes, at + 0x14);
            let rectangle = [0x34, 0x38, 0x3c, 0x40].map(|i| word(&player.bytes, at + i));
            if call(
                Call::Placement {
                    category,
                    rectangle,
                },
                player,
                world,
            )?
            .value as u8
                != 0
            {
                let colors = world
                    .colors
                    .as_ref()
                    .ok_or("offer construction color tables unresolved")?;
                let color = *colors
                    .players
                    .get(usize::from(player.bytes[0x1bc]))
                    .ok_or("offer player color unresolved")?;
                let bank = colors.banks[(next(player) % 3) as usize];
                let count = i32::try_from(bank.len()).map_err(|_| "offer color bank oversized")?;
                if count == 0 {
                    return Err("original offer color division by zero".into());
                }
                let index = color % count;
                let value = *bank
                    .get(index as usize)
                    .ok_or("offer color index outside bank")?;
                let variation = value.wrapping_add(((color / count) as u32) << 16);
                let args = [
                    word(&player.bytes, at + 0x14),
                    word(&player.bytes, at + 0x34),
                    word(&player.bytes, at + 0x38),
                    word(&player.bytes, at + 0x3c),
                    word(&player.bytes, at + 0x40),
                    word(&player.bytes, at + 0x50),
                    variation,
                    word(&player.bytes, at + 0x58),
                    (word(&player.bytes, at + 0x28) & !255) | u32::from(player.bytes[at + 0x54]),
                    word(&player.bytes, at + 0x4c),
                    word(&player.bytes, at + 0x28),
                ];
                call(producer(0x48ac20, &args), player, world)?;
            }
            release_reservation(player, at)?;
            super::cancel(player, reference)?;
        }
        10 => completed(player, at),
        11 => {
            if word(&player.bytes, 8) as i32 > 0 {
                return Ok(());
            }
            if word(auction(world)?, 0x74) == 3 {
                completed(player, at);
                return Ok(());
            }
            let slot = u32::from(player.bytes[0x1bc]);
            if player.bytes[0x1e04] != 0 {
                player.bytes[0x1e04] = 0;
                call(producer(0x4644a0, &[slot]), player, world)?;
                put(&mut player.bytes, at + 0x10, 13);
            } else if word(&player.bytes, 0x1e00) as i32 > 0 {
                call(
                    producer(0x464530, &[slot, word(&player.bytes, 0x1e00)]),
                    player,
                    world,
                )?;
                put(&mut player.bytes, at + 0x10, 13);
            } else {
                call(producer(0x4644f0, &[slot]), player, world)?;
                put(&mut player.bytes, at + 0x10, 10);
                player.bytes[0xcd0] = 0;
            }
        }
        13 => {
            let a = auction(world)?;
            let state = word(a, 0x74);
            if state == 5 {
                completed(player, at);
                return Ok(());
            }
            if word(a, 0x58) != u32::from(player.bytes[0x1bc]) {
                return Ok(());
            }
            if state == 3 {
                completed(player, at);
                return Ok(());
            }
            let delay = if world.event_gate {
                1
            } else {
                next(player) % 21 + 5
            };
            put(&mut player.bytes, 8, delay);
            let amount = call(Call::AuctionBudget, player, world)?.value;
            put(&mut player.bytes, 0x1e00, amount);
            let previous = word(&player.bytes, 0x1e38) as i32;
            if previous > -1
                && *auction(world)?
                    .get(0x12 + previous as usize * 12)
                    .ok_or("offer prior bidder outside auction")?
                    != 0
            {
                put(&mut player.bytes, 0x1e00, 0);
            }
            put(&mut player.bytes, at + 0x10, 11);
        }
        _ => {
            call(Call::Diagnostic(0x53ccdc), player, world)?;
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
    #[ignore = "requires original469bf0 offer-execution observations"]
    fn matches_native_execution() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_OFFER_EXECUTION").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAOEX01");
        assert_eq!(r.word(), 2);
        assert_eq!(r.word(), 0x037f);
        for case in 0..r.word() {
            let [kind, variant, placement, budget, mutation, gate, color] =
                std::array::from_fn::<_, 7, _>(|_| r.word());
            let mut p = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![],
            };
            let mut a: [u8; 128] = r.blob().try_into().unwrap();
            let ownership = r
                .blob()
                .chunks_exact(28)
                .map(|b| b.try_into().unwrap())
                .collect();
            let categories: Vec<[u8; 380]> = r
                .blob()
                .chunks_exact(380)
                .map(|b| b.try_into().unwrap())
                .collect();
            let buildings: Vec<[u8; 916]> = r
                .blob()
                .chunks_exact(916)
                .map(|b| b.try_into().unwrap())
                .collect();
            let expected_p = r.blob();
            let expected_a = r.blob();
            let expected_list = r.blob();
            let expected_trace: Vec<[u32; 12]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut trace = vec![];
            let mut storage = crate::ai_ownership::Storage::default();
            storage.lists.insert(0x10016000, ownership);
            let mut world = World {
                category_owners: categories.iter().map(|c|c[5] as i8).collect(),
                buildings: &buildings,
                auction: Some(&mut a),
                colors: Some(Colors {
                    banks: [&[10, 11, 12], &[20, 21, 22, 23], &[30, 31, 32, 33, 34]],
                    players: &[color as i32],
                }),
                event_gate: gate != 0,
            };
            execute(&mut p, 0x10010248, &mut world, |call, p, w| {
                let mut reply = Reply::default();
                let (address, args): (u32, Vec<u32>) = match call {
                    Call::CategoryValue(c) => {
                        reply.extended = Sum::from_f32(987.125)?;
                        (0x4477e0, vec![c])
                    }
                    Call::Placement {
                        category,
                        rectangle,
                    } => {
                        reply.value = placement;
                        (0x445ef0, [vec![category], rectangle.to_vec()].concat())
                    }
                    Call::RemoveOwnedBusiness(b) => {
                        storage.mutate(
                            p,
                            |_| Err("removal unexpectedly allocated".into()),
                            |state| {
                                state.lists[0].remove(b as i32);
                                Ok(())
                            },
                        )?;
                        (0x46ae20, vec![b])
                    }
                    Call::Producer {
                        address,
                        arguments,
                        count,
                    } => {
                        if mutation != 0
                            && matches!(
                                address,
                                0x48a640 | 0x48a690 | 0x48ad20 | 0x48ad80 | 0x48ae60 | 0x48ac20
                            )
                        {
                            put(&mut p.bytes, 0x248 + 0x60, 17.75f32.to_bits());
                        }
                        (address, arguments[..count].to_vec())
                    }
                    Call::AuctionBudget => {
                        reply.value = budget;
                        if mutation != 0 {
                            put(&mut p.bytes, 0x1e38, 2);
                            w.auction.as_deref_mut().unwrap()[0x12 + 24] = 1;
                        }
                        (0x46a200, vec![])
                    }
                    Call::Diagnostic(text) => (0x4e6960, vec![text]),
                };
                let mut row = [0; 12];
                row[0] = address;
                row[1..1 + args.len()].copy_from_slice(&args);
                trace.push(row);
                Ok(reply)
            })
            .unwrap_or_else(|e| panic!("case{case} kind{kind} variant{variant}: {e}"));
            let diffs: Vec<_> = p
                .bytes
                .iter()
                .zip(expected_p)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| (i, *a, *b))
                .collect();
            assert!(
                diffs.is_empty(),
                "player case{case} kind{kind} variant{variant}: {diffs:?}"
            );
            assert_eq!(a, expected_a, "auction case{case}");
            let list: Vec<u8> = storage.lists[&0x10016000]
                .iter()
                .flatten()
                .copied()
                .collect();
            assert_eq!(list, expected_list, "ownership case{case}");
            assert_eq!(trace, expected_trace, "trace case{case}");
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires original4477e0/445ef0 observations"]
    fn matches_native_world_queries() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_OFFER_WORLD").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAOWR01");
        assert_eq!(r.word(), 0x037f);
        for case in 0..r.word() {
            let op = r.word();
            let category = r.take(380).try_into().unwrap();
            let args = std::array::from_fn(|_| r.word());
            let expected = r.word();
            let actual = if op == 0 {
                category_value(category, &image).unwrap().to_f32().to_bits()
            } else {
                u32::from(placement(category, args))
            };
            assert_eq!(actual, expected, "case{case}");
        }
        assert!(r.0.is_empty());
    }
}
