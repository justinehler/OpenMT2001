//! Retained event auxiliary lifecycle (41bb20/41bc90/41bd30/41bd70).
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub card: u32,
    pub player: u32,
    pub target: i32,
}

pub fn auxiliary(player: &Player, reference: u32) -> Result<usize, String> {
    let index = player
        .auxiliaries
        .iter()
        .position(|a| a.reference == reference)
        .ok_or("event auxiliary reference unresolved")?;
    if word(&player.auxiliaries[index].bytes, 0x14) != player.reference {
        return Err("event auxiliary owner differs from player".into());
    }
    if player.bytes.len() < 0x1bd {
        return Err("event player record truncated".into());
    }
    Ok(index)
}

pub fn card_row<'a>(image: &'a PeImage<'_>, card: u32) -> Result<&'a [u8], String> {
    let address = card
        .checked_mul(20)
        .and_then(|n| n.checked_add(0x534208))
        .ok_or("event card address overflow")?;
    image.at(address, 20)
}

///41bb20 publishes a dealt card to both owner and auxiliary.
pub fn receive(player: &mut Player, reference: u32, card: u32) -> Result<(), String> {
    let index = auxiliary(player, reference)?;
    put(&mut player.auxiliaries[index].bytes, 4, card);
    put(&mut player.bytes, 0x1b8, card);
    put(&mut player.auxiliaries[index].bytes, 0x18, 1);
    Ok(())
}

///41bd30 only modes1/2 copy the card; mode0 clears it; all other signed
/// values preserve the player's card while replacing the auxiliary mode.
pub fn set_mode(player: &mut Player, reference: u32, mode: u32) -> Result<(), String> {
    let index = auxiliary(player, reference)?;
    match mode {
        0 => put(&mut player.bytes, 0x1b8, 0),
        1 | 2 => put(
            &mut player.bytes,
            0x1b8,
            word(&player.auxiliaries[index].bytes, 4),
        ),
        _ => (),
    }
    put(&mut player.auxiliaries[index].bytes, 0x18, mode);
    Ok(())
}

fn reset(player: &mut Player, reference: u32) -> Result<(), String> {
    let index = auxiliary(player, reference)?;
    put(&mut player.bytes, 0x1b8, 0);
    let aux = &mut player.auxiliaries[index].bytes;
    put(aux, 0x18, 0);
    let state = word(aux, 0x20)
        .wrapping_mul(word(aux, 0x1c))
        .wrapping_add(word(aux, 0x24));
    put(aux, 0x1c, state);
    aux[8] = 0;
    put(aux, 0, 20 + ((state >> 16) & 0x7fff) % 9);
    Ok(())
}

///41bc90 snapshots card/player BEFORE validation and posts command1a BEFORE
/// resetting the auxiliary. Validation and submission may synchronously mutate
/// retained state; the reset rereads the original auxiliary's RNG afterward.
pub fn retire(
    player: &mut Player,
    reference: u32,
    mut validate: impl FnMut(&mut Player, u32) -> Result<i32, String>,
    mut submit: impl FnMut(Command, &mut Player) -> Result<(), String>,
) -> Result<bool, String> {
    let index = auxiliary(player, reference)?;
    let card = word(&player.auxiliaries[index].bytes, 4);
    let slot = u32::from(player.bytes[0x1bc]);
    let target = validate(player, reference)?;
    if target == -1 {
        return Ok(false);
    }
    submit(
        Command {
            card,
            player: slot,
            target,
        },
        player,
    )?;
    reset(player, reference)?;
    Ok(true)
}

///41bd70 releases only type2 cards through message34 before the same reset.
pub fn discard(
    player: &mut Player,
    reference: u32,
    image: &PeImage<'_>,
    mut send: impl FnMut([i32; 8], &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    let index = auxiliary(player, reference)?;
    let card = word(&player.auxiliaries[index].bytes, 4);
    if word(card_row(image, card)?, 0) != 2 {
        return Ok(());
    }
    send(
        [
            -1,
            i32::from(player.bytes[0x1bc]),
            0x34,
            card as i32,
            0,
            0,
            0,
            -999,
        ],
        player,
    )?;
    reset(player, reference)
}

/// The retained geometry node selected by4dfba0. Its kind/category words are
/// native +30/+38; allocation identity remains owned by the scene.
#[derive(Clone, Copy)]
pub struct Node {
    pub reference: u32,
    pub kind: u16,
    pub category: u32,
}

pub struct Targets<'a> {
    pub buildings: &'a [[u8; 0x394]],
    pub businesses: &'a [[u8; 544]],
    pub categories: &'a [[u8; 0x17c]],
    pub players: &'a [u32],
    pub nodes: &'a [Node],
}

impl Targets<'_> {
    fn owner(&self, byte: u8) -> Result<u32, String> {
        if byte == 0 {
            return Ok(0);
        }
        self.players
            .get((i32::from(byte as i8) - 1) as usize)
            .copied()
            .ok_or("event target owner leaves roster".into())
    }
    /// Actual41ca60 with4455a0/462ae0/445ca0 and41ccd0/41cd10 reads.
    pub fn validate(
        &self,
        player: &mut Player,
        reference: u32,
        image: &PeImage<'_>,
    ) -> Result<i32, String> {
        let index = auxiliary(player, reference)?;
        let aux = &mut player.auxiliaries[index].bytes;
        if word(aux, 0x18) == 0 {
            return Ok(-1);
        }
        aux[8] = 0;
        let row = card_row(image, word(aux, 4))?;
        if word(row, 0) != 2 {
            aux[8] = 1;
            return Ok(0);
        }
        let kind = word(row, 8);
        let mut target = -1;
        if matches!(kind, 0 | 2 | 3 | 4 | 8) {
            let reference = word(aux, 0x10);
            if reference == 0 {
                return Ok(-1);
            }
            let offset = reference.wrapping_sub(0x591318);
            if !offset.is_multiple_of(0x394) {
                return Err("event building reference unaligned".into());
            }
            let building = self
                .buildings
                .get((offset / 0x394) as usize)
                .ok_or("event building outside pool")?;
            if word(building, 0x24) != 0 {
                return Ok(-1);
            }
            let business = word(building, 0x1c) as i32;
            if business < 0 {
                return Ok(-1);
            }
            let business = self
                .businesses
                .get(business as usize)
                .ok_or("event business outside pool")?;
            let same = self.owner(business[0xa])? == word(aux, 0x14);
            if same == matches!(kind, 2 | 3) {
                target = word(building, 0) as i32;
            }
        } else if matches!(kind, 1 | 5 | 6 | 7) {
            let reference = word(aux, 0xc);
            if reference == 0 {
                return Ok(-1);
            }
            let node = self
                .nodes
                .iter()
                .find(|n| n.reference == reference)
                .ok_or("event geometry node unresolved")?;
            if node.kind != 4 || word(aux, 0x10) != 0 {
                return Ok(-1);
            }
            let category = node.category;
            let at = category
                .checked_mul(88)
                .and_then(|v| v.checked_add(0x5385a4))
                .ok_or("event category definition address overflow")?;
            if matches!(image.at(at, 1)?[0], 0 | 10 | 11) {
                return Ok(-1);
            }
            let accepted = if matches!(kind, 6 | 7) {
                let category = self
                    .categories
                    .get(category as usize)
                    .ok_or("event category outside pool")?;
                (self.owner(category[5])? == word(aux, 0x14)) == (kind == 7)
            } else {
                true
            };
            if accepted {
                target = category as i32;
            }
        }
        if target != -1 {
            aux[8] = 1;
        }
        Ok(target)
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
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires original event lifecycle observations"]
    fn matches_native_lifecycle() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_EVENT_LIFECYCLE").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTECLF01");
        assert_eq!(r.word(), 2);
        assert_eq!(r.word(),0x037f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let [op, arg, mutation, expected_return] = std::array::from_fn::<_, 4, _>(|_| r.word());
            let bytes = r.blob().to_vec();
            let aux = r.blob().try_into().unwrap();
            let buildings = vec![r.blob().try_into().unwrap()];
            let businesses = vec![r.blob().try_into().unwrap()];
            let categories: Vec<[u8; 0x17c]> = r
                .blob()
                .chunks_exact(0x17c)
                .map(|v| v.try_into().unwrap())
                .collect();
            let node = r.blob();
            let nodes = [Node {
                reference: 0x10018000,
                kind: u16::from_le_bytes(node[0x30..0x32].try_into().unwrap()),
                category: word(node, 0x38),
            }];
            let expected_player = r.blob();
            let expected_aux = r.blob();
            let expected_trace: Vec<[u32; 9]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let targets = Targets {
                buildings: &buildings,
                businesses: &businesses,
                categories: &categories,
                players: &[0x10010000, 0x10018000],
                nodes: &nodes,
            };
            let mut player = Player {
                reference: 0x10010000,
                bytes,
                auxiliaries: vec![crate::player_manager::Auxiliary {
                    reference: 0x10014000,
                    bytes: aux,
                }],
            };
            let mut trace = vec![];
            let mutate = |p: &mut Player| {
                if mutation != 0 {
                    let a = &mut p.auxiliaries[0].bytes;
                    put(a, 0x1c, u32::MAX);
                    put(a, 0x20, 3);
                    put(a, 0x24, 7);
                }
            };
            match op {
                0 => assert_eq!(
                    targets.validate(&mut player, 0x10014000, &image).unwrap() as u32,
                    expected_return,
                    "return {name}"
                ),
                1 => {
                    let returned = retire(
                        &mut player,
                        0x10014000,
                        |p, a| targets.validate(p, a, &image),
                        |c, p| {
                            trace.push([
                                0x450490,
                                c.card,
                                c.player,
                                c.target as u32,
                                0,
                                0,
                                0,
                                0,
                                0,
                            ]);
                            mutate(p);
                            Ok(())
                        },
                    )
                    .unwrap();
                    assert_eq!(u8::from(returned), expected_return as u8, "return {name}");
                }
                2 => discard(&mut player, 0x10014000, &image, |args, p| {
                    let mut row = [0; 9];
                    row[0] = 0x48cab0;
                    row[1..].copy_from_slice(&args.map(|v| v as u32));
                    trace.push(row);
                    mutate(p);
                    Ok(())
                })
                .unwrap(),
                3 => set_mode(&mut player, 0x10014000, arg).unwrap(),
                4 => receive(&mut player, 0x10014000, arg).unwrap(),
                _ => panic!("bad operation"),
            }
            assert_eq!(player.bytes, expected_player, "player {name}");
            assert_eq!(
                player.auxiliaries[0].bytes, expected_aux,
                "auxiliary {name}"
            );
            assert_eq!(trace, expected_trace, "trace {name}");
        }
        assert!(r.0.is_empty());
    }
}
