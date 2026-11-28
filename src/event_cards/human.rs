//! Human card activation, dialog controls and request producers.
use super::lifecycle::{self, Command};
use crate::{
    pe::PeImage,
    player_manager::{Player, word},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    ///41b800; the presentation is populated before activation.
    Present(u32),
    ///43be20.
    Activate,
    ///43bed0.
    Close,
    ///450490, command1a with delay byte1 and admission argument0.
    Submit(Command),
    ///48cab0; recipient-1 requests belong to the broadcast deck.
    Send([i32; 8]),
}

///41bb40 opens the card and immediately applies non-targeted cards. Presentation
/// callbacks run synchronously; the card is reloaded after activation.
pub fn open(
    player: &mut Player,
    reference: u32,
    image: &PeImage<'_>,
    validate: impl FnMut(&mut Player, u32) -> Result<i32, String>,
    mut call: impl FnMut(Call, &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    let index = lifecycle::auxiliary(player, reference)?;
    call(
        Call::Present(word(&player.auxiliaries[index].bytes, 4)),
        player,
    )?;
    call(Call::Activate, player)?;
    let index = lifecycle::auxiliary(player, reference)?;
    let card = word(&player.auxiliaries[index].bytes, 4);
    if word(lifecycle::card_row(image, card)?, 0) != 2 {
        lifecycle::retire(player, reference, validate, |command, p| {
            call(Call::Submit(command), p)
        })?;
    }
    Ok(())
}

///41b680. Button identities are original control indices, not displayed labels.
pub fn button(
    player: &mut Player,
    reference: u32,
    image: &PeImage<'_>,
    event: u32,
    button: u32,
    validate: impl FnMut(&mut Player, u32) -> Result<i32, String>,
    mut call: impl FnMut(Call, &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    if event != 0xc || button > 3 {
        return Ok(());
    }
    match button {
        1 => {
            if !lifecycle::retire(player, reference, validate, |command, p| {
                call(Call::Submit(command), p)
            })? {
                return Ok(());
            }
        }
        2 => lifecycle::set_mode(player, reference, 2)?,
        3 => lifecycle::discard(player, reference, image, |args, p| {
            call(Call::Send(args), p)
        })?,
        _ => (),
    }
    call(Call::Close, player)
}

///41bc00/41bc30. These requests are broadcasts; card delivery is the separate33 reply.
pub fn request(
    player: &mut Player,
    reference: u32,
    card: Option<u32>,
    mut call: impl FnMut(Call, &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    lifecycle::auxiliary(player, reference)?;
    let args = [
        -1,
        i32::from(player.bytes[0x1bc]),
        if card.is_some() { 0x32 } else { 0x31 },
        card.unwrap_or(0) as i32,
        0,
        0,
        0,
        -999,
    ];
    call(Call::Send(args), player)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_manager::{Auxiliary, put};
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
    #[ignore = "requires original human-card observations"]
    fn matches_native_human() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_EVENT_HUMAN")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTECHUM1");
        assert_eq!(r.word(), 1);
        assert_eq!(r.word(), 0x037f);
        for _ in 0..r.word() {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let [op, event, button_index, card, mutation] =
                std::array::from_fn::<_, 5, _>(|_| r.word());
            let mut player = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![Auxiliary {
                    reference: 0x10014000,
                    bytes: r.blob().try_into().unwrap(),
                }],
            };
            let buildings = [r.blob().try_into().unwrap()];
            let businesses = [r.blob().try_into().unwrap()];
            let categories: Vec<[u8; 0x17c]> = r
                .blob()
                .chunks_exact(0x17c)
                .map(|v| v.try_into().unwrap())
                .collect();
            let node = r.blob();
            let nodes = [lifecycle::Node {
                reference: 0x10018000,
                kind: u16::from_le_bytes(node[0x30..0x32].try_into().unwrap()),
                category: word(node, 0x38),
            }];
            let targets = lifecycle::Targets {
                buildings: &buildings,
                businesses: &businesses,
                categories: &categories,
                players: &[0x10010000, 0x10018000],
                nodes: &nodes,
            };
            let expected_player = r.blob();
            let expected_aux = r.blob();
            let expected_trace: Vec<[u32; 9]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut trace = vec![];
            let mut call = |call, p: &mut Player| {
                let mut row = [0; 9];
                match call {
                    Call::Present(card) => {
                        row[0] = 0x41b800;
                        row[1] = card;
                    }
                    Call::Activate => row[0] = 0x43be20,
                    Call::Close => row[0] = 0x43bed0,
                    Call::Submit(c) => {
                        row[0] = 0x450490;
                        row[1] = c.card;
                        row[2] = c.player;
                        row[3] = c.target as u32;
                    }
                    Call::Send(args) => {
                        row[0] = 0x48cab0;
                        row[1..].copy_from_slice(&args.map(|v| v as u32));
                    }
                }
                trace.push(row);
                let a = &mut p.auxiliaries[0].bytes;
                match (mutation, call) {
                    (1, Call::Present(_)) => put(a, 4, 65),
                    (2, Call::Activate) => put(a, 4, 1),
                    (3, Call::Activate) => put(a, 0x18, 0),
                    (4, Call::Submit(_) | Call::Send(_)) => {
                        put(a, 0x1c, u32::MAX);
                        put(a, 0x20, 3);
                        put(a, 0x24, 7);
                    }
                    _ => (),
                }
                Ok(())
            };
            let validate = |p: &mut Player, a| targets.validate(p, a, &image);
            match op {
                0 => open(&mut player, 0x10014000, &image, validate, &mut call),
                1 => button(
                    &mut player,
                    0x10014000,
                    &image,
                    event,
                    button_index,
                    validate,
                    &mut call,
                ),
                2 | 3 => request(
                    &mut player,
                    0x10014000,
                    (op == 3).then_some(card),
                    &mut call,
                ),
                _ => panic!("invalid fixture operation"),
            }
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(player.bytes, expected_player, "player {name}");
            assert_eq!(
                player.auxiliaries[0].bytes, expected_aux,
                "auxiliary {name}"
            );
            assert_eq!(trace, expected_trace, "callbacks {name}");
        }
        assert!(r.0.is_empty());
    }
}
