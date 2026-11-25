//! Original command0f (450ffe) over the authoritative building/category pools.
//! The quote is snapshotted in the packet; execution must not recompute prices.
use crate::{building_pool, economy::PlayerAccount, pe::PeImage, pending_world};

/// Original48a6d0 compares an exact FILD integer plus cash before any f32
/// store. The v1.4 initial value of541c7c is10000; callers retain it explicitly.
pub fn affordable(cash: f32, allowance: i32, requested: f32, override_gate: bool) -> bool {
    if override_gate {
        return true;
    }
    if cash.is_nan() || requested.is_nan() {
        return false;
    }
    if cash.is_infinite() || requested.is_infinite() {
        return f64::from(cash) + f64::from(allowance) >= f64::from(requested);
    }
    let mut available = crate::x87::Sum::from_i32(allowance);
    available.add_f32(cash).expect("finite cash");
    !available
        .compare(crate::x87::Sum::from_f32(requested).expect("finite request"))
        .is_lt()
}

/// Ordered original failure UI calls. The shared predicate never displays UI.
pub const INSUFFICIENT_CASH_CALLS: [(u32, &[u32]); 6] = [
    (0x43be20, &[]),
    (0x429af0, &[0x12e]),
    (0x429820, &[0, 0x6b6, 0]),
    (0x429820, &[1, 0x6c2, 0]),
    (0x429a60, &[1]),
    (0x4298b0, &[0, 0x59b]),
];

#[derive(Clone, Debug)]
pub struct Command {
    pub category: i8,
    pub rectangle: [i8; 4],
    pub height: i8,
    pub kind: u32,
    pub business: u32,
    pub variation: u32,
    pub quality: i8,
    /// One-based player owner, read as a signed byte by the consumer.
    pub owner: i8,
    /// Base, initial stock, land rent, utilities (6175fc..617608).
    pub components: [f32; 4],
}
impl Command {
    pub fn from_packet(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 0x3c || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) != 0xf {
            return Err("construction requires original command0f descriptor".into());
        }
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        Ok(Self {
            category: bytes[0x14] as i8,
            rectangle: [0x15, 0x17, 0x16, 0x18].map(|at| bytes[at] as i8),
            height: bytes[0x19] as i8,
            kind: word(0x24),
            business: word(0x20),
            variation: word(0x1c),
            quality: bytes[0x28] as i8,
            owner: bytes[0x29] as i8,
            //48ac20 permutes the quote globals into the descriptor.
            components: [0x2c, 0x38, 0x34, 0x30].map(|at| f32::from_bits(word(at))),
        })
    }
    pub fn packet(&self) -> [u8; 0x3c] {
        let mut bytes = [0; 0x3c];
        bytes[..4].copy_from_slice(&0x525dd4u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&0xfu32.to_le_bytes());
        bytes[0x10] = 1;
        bytes[0x14] = self.category as u8;
        for (at, v) in [0x15, 0x17, 0x16, 0x18].into_iter().zip(self.rectangle) {
            bytes[at] = v as u8;
        }
        bytes[0x19] = self.height as u8;
        for (at, v) in [
            (0x1c, self.variation),
            (0x20, self.business),
            (0x24, self.kind),
        ] {
            bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes[0x28] = self.quality as u8;
        bytes[0x29] = self.owner as u8;
        for (at, v) in [0x2c, 0x38, 0x34, 0x30].into_iter().zip(self.components) {
            bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes
    }
    pub fn building_value(&self) -> Result<f32, String> {
        let mut sum = crate::x87::Sum::from_f32(self.components[2])?;
        for i in [1, 3, 0] {
            sum.add_f32(self.components[i])?;
        }
        Ok(sum.to_f32())
    }
}

/// Original4517f2..4518ca local Build0f admission, before delay/enqueue.
/// `false` is native rejection with sender success: callers must not enqueue.
/// Due application and cold queue restoration must bypass this producer gate.
pub fn admit(
    command: &Command,
    categories: &mut [[u8; 0x17c]; 52],
    players: &mut impl crate::player_messages::Players,
    arena: &mut crate::player_messages::Arena,
    broadcast_enabled: u32,
    routing: crate::player_messages::Routing,
    external: impl FnMut(crate::player_messages::Route, &crate::player_messages::Packet) -> Result<(), String>,
) -> Result<bool, String> {
    if routing.network[0] && !routing.network[1] {
        return Err("construction requires unresolved network command transport".into());
    }
    let category = categories.get_mut(command.category as usize)
        .ok_or("construction admission category leaves pool")?;
    let occupancy: &mut [u8; 49] = (&mut category[0x9c..0xcd]).try_into().unwrap();
    let rectangle = command.rectangle.map(i32::from);
    if building_pool::can_place(occupancy, rectangle) {
        //45184e precedes450a30, including its allocator/error behavior. Do not
        //roll back this store if the subsequent queue consumer fails.
        building_pool::fill_occupancy(occupancy, rectangle, command.height as u8)?;
        return Ok(true);
    }
    //451894 sign-extends packet+29;86d1fc+4*owner is the zero-based
    //86d200 player table at owner-1, NOT player table[owner]. Resolve only
    //on rejection, matching the native accepted path's lack of owner lookup.
    let recipient = i32::from(command.owner) - 1;
    let owner = usize::try_from(recipient).ok().and_then(|slot| players.get(slot))
        .ok_or("construction rejection owner leaves roster")?;
    let kind = owner.bytes.get(0x154..0x158)
        .ok_or("construction rejection player truncated")?;
    if u32::from_le_bytes(kind.try_into().unwrap()) == 1 {
        //4518af..4518c5 ->48cab0(owner-1,-1,30,category,business,0,0,-999).
        //The message owner's activity gate, allocation, inbox and prior bytes
        //are shared with all real player messages, including failure ordering.
        crate::player_messages::produce_in(
            [recipient, -1, 0x30, i32::from(command.category), command.business as i32, 0, 0, -999],
            broadcast_enabled, players, arena, routing, external,
        )?;
    }
    Ok(false)
}

#[cfg(test)]
#[path = "construction/admission_tests.rs"]
mod admission_tests;

pub struct World<'a> {
    pub buildings: &'a mut [[u8; 0x394]],
    pub categories: &'a mut [[u8; 0x17c]; 52],
    pub pending: &'a mut pending_world::State,
    pub players: &'a mut [PlayerAccount],
}
pub struct Context<'a, 'image> {
    pub image: &'a PeImage<'image>,
    pub building_base: u32,
    pub category_base: u32,
    pub date: [u8; 2],
    pub ownership_timestamp: u32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Appearance(usize),
    Created(usize),
    /// Ledger/cash stores already happened; original48a5a0's host effects follow.
    Posted {
        player: usize,
        amount: f32,
        ledger: usize,
    },
    RefreshInterface,
}
fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}
fn appearance(row: &[u8]) -> bool {
    word(row, 0x4c) == 1 && (word(row, 0x1c) == u32::MAX || matches!(word(row, 0x24), 0 | 1))
}

pub fn apply(
    command: &Command,
    world: &mut World<'_>,
    context: &Context<'_, '_>,
    geometry: &mut impl building_pool::GeometryBinding,
    mut callback: impl FnMut(Event, &mut World<'_>) -> Result<(), String>,
) -> Result<usize, String> {
    let slot = building_pool::first_free(world.buildings)?
        .ok_or("original construction dereferences an exhausted building pool")?;
    let total = command.building_value()?;
    let category =
        usize::try_from(command.category).map_err(|_| "negative construction category")?;
    let category_row = world
        .categories
        .get(category)
        .ok_or("construction category leaves table")?;
    let rectangle = command.rectangle.map(i32::from);
    let center = building_pool::center(category_row, rectangle, context.image)?;
    building_pool::initialize(
        &mut world.buildings[slot],
        &building_pool::Initialization {
            rectangle,
            height: command.height as i32 as u32,
            center,
            variation: command.variation,
            kind: command.kind,
            quality: command.quality,
            total_cost: total.to_bits(),
            date: context.date,
        },
        context.image,
    )?;
    let reference = context
        .category_base
        .checked_add(category as u32 * 0x17c)
        .ok_or("construction category reference overflow")?;
    building_pool::bind_category(
        &mut world.categories[category],
        world.buildings,
        slot,
        context.building_base,
        reference,
        geometry,
    )?;
    world.buildings[slot][0x2d] = world.categories[category][5];
    world.buildings[slot][0x30..0x34].copy_from_slice(&context.ownership_timestamp.to_le_bytes());
    if appearance(&world.buildings[slot]) {
        callback(Event::Appearance(slot), world)?;
    }
    let identity = word(&world.buildings[slot], 0) as i32;
    if identity > -1 {
        callback(Event::Created(identity as usize), world)?;
    }
    world.buildings[slot][0x2d] = command.owner as u8;
    world.buildings[slot][0x30..0x34].copy_from_slice(&context.ownership_timestamp.to_le_bytes());
    if appearance(&world.buildings[slot]) {
        callback(Event::Appearance(slot), world)?;
    }
    world.pending.construct(
        word(&world.categories[category], 0),
        &mut world.buildings[slot],
        command.business,
        command.owner as i32 as u32,
    )?;
    if appearance(&world.buildings[slot]) {
        callback(Event::Appearance(slot), world)?;
    }
    let player = usize::try_from(i32::from(command.owner) - 1)
        .map_err(|_| "construction owner does not resolve")?;
    for (component, ledger) in [(0, 0xc), (2, 0xe), (1, 0x11), (3, 7)] {
        let amount = -command.components[component];
        world
            .players
            .get_mut(player)
            .ok_or("construction owner leaves roster")?
            .post(amount, ledger)?;
        callback(
            Event::Posted {
                player,
                amount,
                ledger,
            },
            world,
        )?;
    }
    callback(Event::RefreshInterface, world)?;
    Ok(slot)
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
    #[ignore = "requires original48a6d0 observations"]
    fn matches_native_affordability() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_BUILD_ADMISSION").unwrap()).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTBUILD1");
        assert_eq!(r.word(), 1);
        for _ in 0..r.word() {
            let n = u16::from_le_bytes(r.take(2).try_into().unwrap()) as usize;
            let name = std::str::from_utf8(r.take(n)).unwrap();
            r.blob();
            let input: [u32; 8] = std::array::from_fn(|_| r.word());
            let before: [&[u8]; 3] = std::array::from_fn(|_| r.blob());
            let globals = [r.word(), r.word()];
            let after: [&[u8]; 3] = std::array::from_fn(|_| r.blob());
            assert_eq!(before, after, "{name}");
            assert_eq!(globals, [r.word(), r.word()]);
            let mut calls = Vec::new();
            for _ in 0..r.word() {
                let target = r.word();
                r.word();
                r.word();
                let n = r.word();
                calls.push((target, (0..n).map(|_| r.word()).collect::<Vec<_>>()));
            }
            assert_eq!(r.word(), 0, "{name}");
            let accepted = affordable(
                f32::from_bits(input[1]),
                input[0] as i32,
                f32::from_bits(input[2]),
                input[5] as u8 != 0,
            );
            assert_eq!(accepted, u32::from(input[6] as u8) != 0, "{name}");
            let expected = if !accepted && input[4] != 0 && input[3] as u8 != 0 {
                INSUFFICIENT_CASH_CALLS
                    .iter()
                    .map(|(address, args)| (*address, args.to_vec()))
                    .collect()
            } else {
                vec![]
            };
            assert_eq!(calls, expected, "{name}");
        }
        assert!(r.0.is_empty());
    }

    #[test]
    #[ignore = "requires original48ac20 producer observations"]
    fn matches_native_producer() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_CONSTRUCTION_PRODUCER").unwrap())
            .unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTCONP02");
        assert_eq!(r.word(), 2);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            assert_eq!(r.word(), 11);
            let args: [u32; 11] = std::array::from_fn(|_| r.word());
            let owner = r.word();
            let components = std::array::from_fn(|_| f32::from_bits(r.word()));
            assert_eq!(r.word(), 6);
            let quote: [u32; 6] = std::array::from_fn(|_| r.word());
            assert_eq!(
                quote,
                [
                    args[0],
                    args[3].wrapping_mul(args[4]).wrapping_mul(args[5]),
                    args[7],
                    args[10],
                    args[8],
                    0x10020000
                ],
                "{name}"
            );
            assert_eq!(r.word(), 1);
            assert_eq!(r.word(), 0);
            r.blob();
            let expected = r.blob();
            let command = Command {
                category: args[0] as i8,
                rectangle: [
                    args[1] as i8,
                    args[2] as i8,
                    args[1].wrapping_add(args[3]).wrapping_sub(1) as i8,
                    args[2].wrapping_add(args[4]).wrapping_sub(1) as i8,
                ],
                height: args[5] as i8,
                variation: args[6],
                kind: args[7],
                business: args[10],
                quality: args[8] as i8,
                owner: owner.wrapping_add(1) as i8,
                components,
            };
            let actual = command.packet();
            // Native stack padding is unspecified; compare every written byte.
            for range in [0..8, 0xc..0x11, 0x14..0x1a, 0x1c..0x2a, 0x2c..0x3c] {
                assert_eq!(&actual[range.clone()], &expected[range], "{name}");
            }
            let decoded = Command::from_packet(&expected[..0x3c]).unwrap();
            assert_eq!(
                decoded.components.map(f32::to_bits),
                components.map(f32::to_bits),
                "{name}"
            );
        }
        assert!(r.0.is_empty());
    }

    #[test]
    #[ignore = "requires original construction command/geometry seam observations"]
    fn matches_native_consumer() {
        struct Snapshot<'a> {
            building: &'a [u8],
            category: &'a [u8],
            pending: &'a [u8],
            packet: &'a [u8],
            players: [&'a [u8]; 6],
            count: u32,
            tick: u32,
        }
        fn snapshot<'a>(r: &mut Reader<'a>) -> Snapshot<'a> {
            Snapshot {
                building: r.blob(),
                category: r.blob(),
                pending: r.blob(),
                packet: r.blob(),
                players: std::array::from_fn(|_| r.blob()),
                count: r.word(),
                tick: r.word(),
            }
        }
        struct Geometry {
            footprint: [u8; 5],
            center: [u32; 2],
            handle: u32,
            height: f32,
            positions: Vec<[u32; 3]>,
        }
        impl building_pool::GeometryBinding for Geometry {
            fn prepare(&mut self, _: usize, row: &mut [u8; 0x394]) -> Result<(), String> {
                row[4..9].copy_from_slice(&self.footprint);
                for (at, value) in [
                    (0xc, self.center[0]),
                    (0x10, self.center[1]),
                    (0x170, self.handle),
                    (0x220, self.positions.len() as u32),
                ] {
                    row[at..at + 4].copy_from_slice(&value.to_le_bytes());
                }
                for (i, position) in self.positions.iter().enumerate() {
                    for (axis, value) in position.iter().enumerate() {
                        let at = 0x224 + i * 12 + axis * 4;
                        row[at..at + 4].copy_from_slice(&value.to_le_bytes());
                    }
                }
                Ok(())
            }
            fn model_height(&mut self, handle: u32) -> Result<f32, String> {
                assert_eq!(handle, self.handle);
                Ok(self.height)
            }
        }
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_CONSTRUCTION_COMMAND").unwrap())
            .unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTCONS02");
        assert_eq!(r.word(), 2);
        let mut matched = 0;
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            r.blob();
            for _ in 0..r.word() {
                r.word();
                r.word();
                r.word();
            }
            let mut geometry = if r.word() != 0 {
                let footprint = std::array::from_fn(|_| r.word() as u8);
                let center = std::array::from_fn(|_| r.word());
                let handle = r.word();
                let height = f32::from_bits(r.word());
                let positions = (0..r.word())
                    .map(|_| std::array::from_fn(|_| r.word()))
                    .collect();
                Some(Geometry {
                    footprint,
                    center,
                    handle,
                    height,
                    positions,
                })
            } else {
                None
            };
            let before = snapshot(&mut r);
            let after = snapshot(&mut r);
            let returned = r.word();
            let fault = r.word();
            let mut trace = Vec::new();
            for _ in 0..r.word() {
                let kind = r.word();
                let address = r.word();
                let receiver = r.word();
                let args = (0..r.word()).map(|_| r.word()).collect::<Vec<_>>();
                trace.push((kind, address, receiver, args));
            }
            r.blob();
            let command = Command::from_packet(&before.packet[..0x3c]).unwrap();
            let mut buildings = vec![[0; 0x394]; 600];
            buildings[0].copy_from_slice(before.building);
            let mut categories = [[0; 0x17c]; 52];
            categories[command.category as usize].copy_from_slice(before.category);
            let mut pending = pending_world::State::from_bytes(before.pending).unwrap();
            assert_eq!(pending.count, before.count);
            let mut players = before
                .players
                .iter()
                .map(|raw| PlayerAccount::from_runtime(raw).unwrap())
                .collect::<Vec<_>>();
            let context = Context {
                image: &image,
                building_base: 0x591318,
                category_base: 0x617628,
                date: [0, 0],
                ownership_timestamp: before.tick,
            };
            if fault != 0 {
                assert_eq!(returned, 0);
                for row in &mut buildings {
                    row[0x3f] = 1;
                }
                let mut stub = Geometry {
                    footprint: [0; 5],
                    center: [0; 2],
                    handle: 1,
                    height: 0.,
                    positions: vec![],
                };
                assert!(
                    apply(
                        &command,
                        &mut World {
                            buildings: &mut buildings,
                            categories: &mut categories,
                            pending: &mut pending,
                            players: &mut players
                        },
                        &context,
                        &mut stub,
                        |_, _| Ok(())
                    )
                    .is_err()
                );
                continue;
            }
            assert_eq!(returned, 1);
            let init = trace.iter().find(|c| c.1 == 0x4450e0).unwrap();
            assert_eq!(
                init.3[10],
                command.building_value().unwrap().to_bits(),
                "{name}"
            );
            let Some(geometry) = geometry.as_mut() else {
                // Malformed endpoints deliberately bypass native geometry. This
                // proves the signed constructor ABI, not valid world placement.
                assert_eq!(
                    init.3[..5],
                    [
                        command.rectangle[0] as i32 as u32,
                        command.rectangle[1] as i32 as u32,
                        command.rectangle[2] as i32 as u32,
                        command.rectangle[3] as i32 as u32,
                        command.height as i32 as u32
                    ]
                );
                continue;
            };
            let mut posts = Vec::new();
            apply(
                &command,
                &mut World {
                    buildings: &mut buildings,
                    categories: &mut categories,
                    pending: &mut pending,
                    players: &mut players,
                },
                &context,
                geometry,
                |event, _| {
                    if let Event::Posted {
                        player,
                        amount,
                        ledger,
                    } = event
                    {
                        posts.push((
                            0x10030000 + player as u32 * 0x400,
                            vec![amount.to_bits(), ledger as u32],
                        ));
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(buildings[0].as_slice(), after.building, "{name} building");
            assert_eq!(
                categories[command.category as usize].as_slice(),
                after.category,
                "{name} category"
            );
            for (at, (actual, expected)) in pending.to_bytes().iter().zip(after.pending).enumerate()
            {
                assert_eq!(actual, expected, "{name} pending byte{at:#x}");
            }
            assert_eq!(pending.count, after.count);
            for (i, account) in players.iter().enumerate() {
                let mut actual = before.players[i].to_vec();
                account.write_runtime(&mut actual).unwrap();
                assert_eq!(actual, after.players[i], "{name} player{i}");
            }
            assert_eq!(
                posts,
                trace
                    .iter()
                    .filter(|c| c.1 == 0x48a5a0)
                    .map(|c| (c.2, c.3.clone()))
                    .collect::<Vec<_>>(),
                "{name}"
            );
            matched += 1;
        }
        assert_eq!(matched, 4);
        assert!(r.0.is_empty());
    }
}
