//! Original465330 admission and464970/4648f0 retained auction initialization.

#[derive(Clone, Debug)]
pub struct Controller {
    pub record: [u8; 0x158],
    pub reference: u32,
    pub active_count: u32,
}
#[derive(Clone, Debug, Default)]
pub struct Presentation {
    /// Original7f8d40 retained ambient audio allocation.
    pub ambient_handle: u32,
    /// Original announcement panel+345.
    pub visible: bool,
    pub selected: [u32; 2],
    pub announcement: [u32; 3],
    /// Original announcement+b00, decremented by its 50 Hz message.
    pub countdown: u32,
    pub countdown_sound: u32,
    pub countdown_last: i32,
}
pub struct World<'a> {
    pub controller: &'a mut Controller,
    pub categories: &'a mut [[u8; 380]],
    pub active: &'a [u8],
    pub presentation: &'a mut Presentation,
    pub current_slot: u8,
    pub tutorial: bool,
    pub network: [bool; 2],
    pub command_prior: [u8; 20],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    ReleaseAmbient(u32),
    QueueStart {
        additional_delay: u32,
        descriptor: [u8; 20],
    },
    ShowAnnouncement,
    CategoryStatus(u32),
}
use crate::player_manager::{put, word};
pub fn initialize(
    category: u32,
    bidder: u32,
    amount: u32,
    world: &mut World<'_>,
    call: &mut impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let owner = *world
        .categories
        .get(category as usize)
        .ok_or("auction category outside pool")?
        .get(5)
        .unwrap() as i8;
    let r = &mut world.controller.record;
    put(r, 0x50, (i32::from(owner) - 1) as u32);
    put(r, 4, category);
    put(r, 0x78, 0);
    put(r, 0, world.controller.active_count);
    for off in [0x60, 0x54, 0x58, 0x5c] {
        put(r, off, u32::MAX);
    }
    for off in [0x74, 0x64, 0x68, 0x6c] {
        put(r, off, 0);
    }
    for off in [0x70, 0x7c, 0x7d] {
        r[off] = 0;
    }
    for i in 0..6 {
        put(r, i * 12 + 12, 0);
        for off in [8, 16, 17, 18] {
            r[i * 12 + off] = 0;
        }
    }
    world.presentation.selected = [0; 2];
    if world.presentation.ambient_handle != 0 {
        call(
            Call::ReleaseAmbient(world.presentation.ambient_handle),
            world,
        )?;
        world.presentation.ambient_handle = 0;
    }
    if world.active.len() > 6 {
        return Err("auction has over six players".into());
    }
    for i in 0..world.active.len() {
        if word(&world.controller.record, 0x50) != i as u32 && world.active[i] != 0 {
            world.controller.record[i * 12 + 8] = 1;
        }
    }
    let r = &mut world.controller.record;
    if bidder != u32::MAX {
        if bidder >= 6 {
            return Err("auction initial bidder outside pool".into());
        }
        for off in [0x54, 0x5c, 0x58, 0x84] {
            put(r, off, bidder);
        }
        for off in [0x68, 0x6c, 0x88, bidder as usize * 12 + 12] {
            put(r, off, amount);
        }
    } else {
        let owner = word(r, 0x50);
        if owner == u32::MAX {
            put(r, 0x58, 0);
            put(r, 0x5c, 0);
        } else {
            put(r, 0x58, owner.wrapping_sub(1));
            put(r, 0x5c, owner);
        }
        put(r, 0x84, bidder);
        put(r, 0x6c, amount);
        put(r, 0x88, amount);
    }
    Ok(())
}
/// The authority's answer to a refused kind-29 admission (0x450a30): inbox
/// kind 38 to the requesting bidder, carrying the request's retained words
/// +0x20..+0x30 (property, amount, offer identity and context) unchanged.
pub fn busy_reply(descriptor: &[u8]) -> [i32; 8] {
    [
        word(descriptor, 24) as i32,
        -1,
        0x26,
        word(descriptor, 32) as i32,
        word(descriptor, 36) as i32,
        word(descriptor, 40) as i32,
        word(descriptor, 44) as i32,
        word(descriptor, 48) as i32,
    ]
}
pub fn admit(
    category: u32,
    bidder: u32,
    amount: u32,
    world: &mut World<'_>,
    mut call: impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<bool, String> {
    if world.controller.active_count != 0 {
        if bidder == u32::from(world.current_slot) {
            world
                .categories
                .get_mut(category as usize)
                .ok_or("busy auction category outside pool")?[0x6c] = 0;
        }
        return Ok(false);
    }
    world.controller.reference = 0x7f4758;
    initialize(category, bidder, amount, world, &mut call)?;
    world.controller.active_count = world.controller.active_count.wrapping_add(1);
    world.controller.record[0x7d] = 1;
    let duration = if world.tutorial { 5 } else { 120 };
    if !world.network[0] || world.network[1] {
        let mut descriptor = world.command_prior;
        put(&mut descriptor, 0, 0x525c70);
        put(&mut descriptor, 4, 0x1e);
        put(&mut descriptor, 12, 0);
        descriptor[16] = 1;
        call(
            Call::QueueStart {
                additional_delay: duration * 25,
                descriptor,
            },
            world,
        )?;
    }
    let r = &world.controller.record;
    world.presentation.announcement = [word(r, 4), word(r, 0x5c), word(r, 0x88)];
    call(Call::ShowAnnouncement, world)?;
    world.presentation.countdown = duration.wrapping_mul(50);
    let category = word(&world.controller.record, 4);
    world
        .categories
        .get_mut(category as usize)
        .ok_or("auction announcement category outside pool")?[0x6c] = 1;
    let category = word(&world.controller.record, 4);
    call(Call::CategoryStatus(category), world)?;
    Ok(true)
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            record: [0; 0x158],
            reference: 0,
            active_count: 0,
        }
    }
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
    }
    #[test]
    #[ignore = "requires original auction admission capture"]
    fn matches_native_admission() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_ADMISSION")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTACAD02");
        assert_eq!(r.word(), 0x037f);
        let count = r.word();
        for case in 0..count {
            let input: [u32; 10] = std::array::from_fn(|_| r.word());
            let record = r.take(0x158).try_into().unwrap();
            let mut categories: Vec<[u8; 380]> =
                (0..52).map(|_| r.take(380).try_into().unwrap()).collect();
            let mut controller = Controller {
                record,
                reference: r.word(),
                active_count: r.word(),
            };
            let mut presentation = Presentation {
                selected: [r.word(), r.word()],
                ambient_handle: r.word(),
                announcement: [r.word(), r.word(), r.word()],
                countdown: r.word(),
                countdown_sound: 0,
                countdown_last: 0,
                visible: false,
            };
            let expected = r.take(0x158 + 52 * 380 + 36);
            let expected_return = r.word();
            let n = r.word();
            let expected_trace: Vec<Vec<u32>> = (0..n)
                .map(|_| {
                    let n = r.word();
                    (0..n).map(|_| r.word()).collect()
                })
                .collect();
            let active: Vec<u8> = (0..6).map(|j| u8::from(j != input[9])).collect();
            let mut world = World {
                controller: &mut controller,
                categories: &mut categories,
                active: &active,
                presentation: &mut presentation,
                current_slot: input[7] as u8,
                tutorial: input[4] != 0,
                network: [input[5] != 0, input[6] != 0],
                command_prior: [0xcc; 20],
            };
            let mut trace = vec![];
            let result = admit(input[0], input[1], input[2], &mut world, |call, _| {
                trace.push(match call {
                    Call::ReleaseAmbient(v) => vec![0, v],
                    Call::QueueStart {
                        additional_delay,
                        descriptor,
                    } => {
                        let mut v = vec![1, additional_delay];
                        v.extend(
                            descriptor
                                .chunks_exact(4)
                                .map(|b| u32::from_le_bytes(b.try_into().unwrap())),
                        );
                        v
                    }
                    Call::ShowAnnouncement => vec![2],
                    Call::CategoryStatus(v) => vec![4, v],
                });
                Ok(())
            })
            .unwrap();
            assert_eq!(u32::from(result), expected_return, "case{case}");
            assert_eq!(trace, expected_trace, "case{case}");
            let mut actual = controller.record.to_vec();
            actual.extend(categories.iter().flatten());
            for v in [
                controller.reference,
                controller.active_count,
                presentation.selected[0],
                presentation.selected[1],
                presentation.ambient_handle,
                presentation.announcement[0],
                presentation.announcement[1],
                presentation.announcement[2],
                presentation.countdown,
            ] {
                actual.extend(v.to_le_bytes());
            }
            if let Some(at) = actual.iter().zip(expected).position(|(a, b)| a != b) {
                panic!("case{case} byte{at:x} {} != {}", actual[at], expected[at]);
            }
        }
        assert!(r.0.is_empty());
    }
}
