//! Original48b1d0/48b240 player inbox. Opaque allocation identities are host supplied.
use crate::player_manager::{Auxiliary, Player, put, word};
pub type Packet = [u8; 0x30];
fn auxiliary(player: &Player, reference: u32) -> Result<usize, String> {
    player
        .auxiliaries
        .iter()
        .position(|a| a.reference == reference)
        .ok_or_else(|| format!("player inbox reference {reference:08x} unresolved"))
}
/// Append a complete descriptor, preserving its prior/padding bytes.
pub fn enqueue(
    player: &mut Player,
    packet: &Packet,
    mut allocate: impl FnMut() -> Result<Auxiliary, String>,
) -> Result<bool, String> {
    if player.bytes.len() < 0x21c {
        return Err("player inbox record truncated".into());
    }
    if player.bytes[0x10] == 0 {
        return Ok(false);
    }
    let mut node = allocate()?;
    if node.reference == 0
        || player
            .auxiliaries
            .iter()
            .any(|a| a.reference == node.reference)
    {
        return Err("player inbox allocation identity collides".into());
    }
    node.bytes = *packet;
    let count = word(&player.bytes, 0x214).wrapping_add(1);
    put(&mut player.bytes, 0x214, count);
    put(&mut node.bytes, 0x2c, 0);
    let reference = node.reference;
    player.auxiliaries.push(node);
    let mut tail = word(&player.bytes, 0x218);
    if tail == 0 {
        put(&mut player.bytes, 0x218, reference);
        return Ok(true);
    }
    let mut visited = 0;
    loop {
        if visited >= player.auxiliaries.len() {
            return Err("player inbox chain is cyclic".into());
        }
        visited += 1;
        let index = auxiliary(player, tail)?;
        let next = word(&player.auxiliaries[index].bytes, 0x2c);
        if next == 0 {
            put(&mut player.auxiliaries[index].bytes, 0x2c, reference);
            return Ok(true);
        }
        tail = next;
    }
}
/// Copy into the constructor-owned scratch record before releasing the head.
/// The native scratch link word is copied too, even though it points onward.
pub fn pop(
    player: &mut Player,
    mut release: impl FnMut(u32, &mut Player) -> Result<(), String>,
) -> Result<Option<u32>, String> {
    if player.bytes.len() < 0x21c {
        return Err("player inbox record truncated".into());
    }
    let count = word(&player.bytes, 0x214);
    if count == 0 {
        return Ok(None);
    }
    let head = word(&player.bytes, 0x218);
    let scratch = word(&player.bytes, 0x210);
    put(&mut player.bytes, 0x214, count.wrapping_sub(1));
    let bytes = player.auxiliaries[auxiliary(player, head)?].bytes;
    let scratch_index = auxiliary(player, scratch)?;
    player.auxiliaries[scratch_index].bytes = bytes;
    let next = word(&bytes, 0x2c);
    release(head, player)?;
    let index = auxiliary(player, head)?;
    player.auxiliaries.remove(index);
    let result = word(&player.bytes, 0x210);
    put(&mut player.bytes, 0x218, next);
    Ok(Some(result))
}
/// Original48cab0 packet construction. Unwritten stack bytes are an explicit input.
/// Admission and451720 routing belong to the caller; no host address is dereferenced.
pub fn descriptor(args: [i32; 8], sender_context: i32, mut prior: Packet) -> Packet {
    put(&mut prior, 0, 0x525dc0);
    put(&mut prior, 4, args[2] as u32);
    put(&mut prior, 8, 0x30);
    put(&mut prior, 12, 0);
    prior[0x10] = 0;
    prior[0x14] = args[1] as u8;
    prior[0x15] = args[0] as u8;
    for (index, value) in args[3..7].iter().enumerate() {
        put(&mut prior, 0x18 + index * 4, *value as u32);
    }
    put(
        &mut prior,
        0x28,
        if args[7] == -999 {
            sender_context as u32
        } else {
            args[7] as u32
        },
    );
    prior
}
/// Monotonic logical allocation policy; freed native heap addresses need not be reproduced.
pub struct Arena {
    pub next: u32,
}
impl Arena {
    pub fn allocate(&mut self) -> Result<Auxiliary, String> {
        let reference = self.next;
        self.next = self
            .next
            .checked_add(0x30)
            .ok_or("player inbox arena exhausted")?;
        if reference == 0 {
            return Err("player inbox arena begins at null".into());
        }
        Ok(Auxiliary {
            reference,
            bytes: [0; 0x30],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Broadcast,
    ///451732:405260(category,1), before any recipient routing.
    PropertyRefresh {
        category: u32,
    },
    NetworkSend,
    NetworkRecipient {
        slot: usize,
    },
}
/// Original48cab0 ->450490 ->451720 admission and player routing.
pub fn produce(
    args: [i32; 8],
    manager: &mut crate::player_manager::Manager,
    arena: &mut Arena,
    current_player: u32,
    network: [bool; 2],
    prior: Packet,
    external: impl FnMut(Route, &Packet) -> Result<(), String>,
) -> Result<bool, String> {
    produce_in(
        args,
        word(&manager.bytes, 12),
        &mut manager.players,
        arena,
        Routing {
            current_player,
            network,
            prior,
        },
        external,
    )
}

/// A borrowed split keeps synchronous producers on the authoritative players
/// while a normal controller holds the current player separately.
pub struct SplitPlayers<'a> {
    pub before: &'a mut [Player],
    pub current: &'a mut Player,
    pub after: &'a mut [Player],
}
pub trait Players {
    fn get(&self, slot: usize) -> Option<&Player>;
    fn get_mut(&mut self, slot: usize) -> Option<&mut Player>;
}
impl Players for Vec<Player> {
    fn get(&self, slot: usize) -> Option<&Player> {
        self.as_slice().get(slot)
    }
    fn get_mut(&mut self, slot: usize) -> Option<&mut Player> {
        self.as_mut_slice().get_mut(slot)
    }
}
impl Players for &mut [Player] {
    fn get(&self, slot: usize) -> Option<&Player> {
        (**self).get(slot)
    }
    fn get_mut(&mut self, slot: usize) -> Option<&mut Player> {
        (**self).get_mut(slot)
    }
}
impl Players for SplitPlayers<'_> {
    fn get(&self, slot: usize) -> Option<&Player> {
        let pivot = self.before.len();
        if slot < pivot {
            self.before.get(slot)
        } else if slot == pivot {
            Some(self.current)
        } else {
            self.after.get(slot - pivot - 1)
        }
    }
    fn get_mut(&mut self, slot: usize) -> Option<&mut Player> {
        let pivot = self.before.len();
        if slot < pivot {
            self.before.get_mut(slot)
        } else if slot == pivot {
            Some(self.current)
        } else {
            self.after.get_mut(slot - pivot - 1)
        }
    }
}
#[derive(Clone, Copy)]
pub struct Routing {
    pub current_player: u32,
    pub network: [bool; 2],
    pub prior: Packet,
}
pub fn produce_in(
    args: [i32; 8],
    broadcast_enabled: u32,
    players: &mut impl Players,
    arena: &mut Arena,
    routing: Routing,
    mut external: impl FnMut(Route, &Packet) -> Result<(), String>,
) -> Result<bool, String> {
    let Routing {
        current_player,
        network,
        prior,
    } = routing;
    if args[0] == -1 {
        if broadcast_enabled == 0 {
            return Ok(false);
        }
    } else {
        let player = players
            .get(args[0] as usize)
            .ok_or("message recipient leaves roster")?;
        if player
            .bytes
            .get(0x10)
            .copied()
            .ok_or("message recipient truncated")?
            == 0
        {
            return Ok(false);
        }
    }
    let context = if args[7] == -999 && args[1] > -1 {
        let sender = players
            .get(args[1] as usize)
            .ok_or("message sender leaves roster")?;
        word(&sender.bytes, 12) as i32
    } else {
        args[1]
    };
    let packet = descriptor(args, context, prior);
    route(
        &packet,
        players,
        arena,
        current_player,
        network,
        |r, p, _| external(r, p),
    )
}

///450490/451720 routing after a producer has checked recipient admission.
fn route<P: Players>(
    packet: &Packet,
    players: &mut P,
    arena: &mut Arena,
    current_player: u32,
    network: [bool; 2],
    mut external: impl FnMut(Route, &Packet, &mut P) -> Result<(), String>,
) -> Result<bool, String> {
    if network[0] && !network[1] {
        external(Route::NetworkSend, packet, players)?;
        return Ok(true);
    }
    if (word(packet, 4) as i32) < 0x21 {
        return Err("player inbox producer requires a player-message kind".into());
    }
    if word(packet, 4) == 0x26 {
        external(
            Route::PropertyRefresh {
                category: word(packet, 0x18),
            },
            packet,
            players,
        )?;
    }
    if packet[0x15] == 0xff {
        external(Route::Broadcast, packet, players)?;
        return Ok(true);
    }
    let slot = packet[0x15] as i8 as usize;
    let player = players
        .get_mut(slot)
        .ok_or("transported recipient leaves roster")?;
    if word(&player.bytes, 0x154) == 1 || player.reference == current_player {
        enqueue(player, packet, || arena.allocate())?;
    } else {
        external(Route::NetworkRecipient { slot }, packet, players)?;
    }
    Ok(true)
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
    fn decode(bytes: &[u8]) -> crate::player_manager::Manager {
        let mut r = Reader(bytes);
        let header = r.take(40).try_into().unwrap();
        let players = (0..2)
            .map(|i| {
                let bytes = r.take(0x224).to_vec();
                let n = r.word();
                let auxiliaries = (0..n)
                    .map(|_| Auxiliary {
                        reference: r.word(),
                        bytes: r.take(48).try_into().unwrap(),
                    })
                    .collect();
                Player {
                    reference: 0x10010000 + i * 0x1000,
                    bytes,
                    auxiliaries,
                }
            })
            .collect();
        assert!(r.0.is_empty());
        crate::player_manager::Manager {
            bytes: header,
            players,
        }
    }
    fn snapshot(manager: &crate::player_manager::Manager) -> Vec<u8> {
        let mut out = manager.bytes.to_vec();
        for p in &manager.players {
            out.extend_from_slice(&p.bytes);
            out.extend_from_slice(&(p.auxiliaries.len() as u32).to_le_bytes());
            let mut aux: Vec<_> = p.auxiliaries.iter().collect();
            aux.sort_by_key(|a| a.reference);
            for a in aux {
                out.extend_from_slice(&a.reference.to_le_bytes());
                out.extend_from_slice(&a.bytes);
            }
        }
        out
    }
    #[test]
    #[ignore = "requires original player producer/inbox fixture"]
    fn matches_native_inbox() {
        for split in [None, Some(0), Some(1)] {
            check_native_inbox(split);
        }
    }
    fn check_native_inbox(split: Option<usize>) {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PLAYER_MESSAGES").unwrap()).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTINBOX1");
        let count = r.word();
        for _ in 0..count {
            let name = std::str::from_utf8(r.blob()).unwrap();
            let config: [u32; 6] = std::array::from_fn(|_| r.word());
            let mut manager = decode(r.blob());
            let mut arena = Arena { next: 0x10020000 };
            let n = r.word();
            for step in 0..n {
                let opcode = r.word();
                let slot = r.word() as usize;
                let args = std::array::from_fn(|_| r.word() as i32);
                let expected_result = r.word();
                let expected_next = r.word();
                let expected = r.blob();
                let calls = r.word();
                let expected_calls: Vec<Vec<u32>> = (0..calls)
                    .map(|_| {
                        let n = r.word();
                        (0..n).map(|_| r.word()).collect()
                    })
                    .collect();
                let calls = std::cell::RefCell::new(vec![]);
                let prior = std::array::from_fn(|i| (i * 9 + 17) as u8);
                let result = match opcode {
                    0 => {
                        let mut packet = prior;
                        put(&mut packet, 4, args[2] as u32);
                        enqueue(&mut manager.players[slot], &packet, || {
                            let a = arena.allocate()?;
                            calls.borrow_mut().push(vec![1, a.reference]);
                            Ok(a)
                        })
                        .unwrap();
                        0
                    }
                    1 => pop(&mut manager.players[slot], |reference, p| {
                        calls.borrow_mut().push(vec![2, reference]);
                        if config[5] != 0 {
                            put(&mut p.bytes, 0x218, 0xdeadbeef);
                            let i = auxiliary(p, reference)?;
                            put(&mut p.auxiliaries[i].bytes, 0x2c, 0xdeadbeef);
                        }
                        Ok(())
                    })
                    .unwrap()
                    .unwrap_or(0),
                    3 => {
                        let old = word(&manager.players[slot].bytes, 0x214);
                        let head = word(&manager.players[slot].bytes, 0x218);
                        if old != 0 {
                            calls.borrow_mut().push(vec![2, head]);
                        }
                        consume_human(&mut manager.players[slot], |call, _| {
                            let mut trace = match call {
                                HumanCall::Ui {
                                    address,
                                    arguments,
                                    count,
                                } => {
                                    let mut v = vec![address];
                                    v.extend_from_slice(&arguments[..count]);
                                    v
                                }
                                HumanCall::Diagnostic => vec![0x4e6960, 1, 0, 0x541d34],
                                other => panic!("unhandled human fixture {other:?}"),
                            };
                            calls.borrow_mut().push(std::mem::take(&mut trace));
                            Ok(())
                        })
                        .unwrap();
                        0
                    }
                    2 | 4 => {
                        let before = arena.next;
                        let mut external = |route, packet: &Packet| {
                            let mut call = vec![match route {
                                Route::Broadcast => 0x48c1a0,
                                Route::NetworkSend => 0x4e2410,
                                Route::NetworkRecipient { .. } => 0x4e2590,
                                Route::PropertyRefresh { .. } => 0x405260,
                            }];
                            call.extend(
                                packet
                                    .chunks_exact(4)
                                    .map(|b| u32::from_le_bytes(b.try_into().unwrap())),
                            );
                            calls.borrow_mut().push(call);
                            Ok(())
                        };
                        let result = if opcode == 2
                            && let Some(split) = split
                        {
                            let enabled = word(&manager.bytes, 12);
                            let (before, rest) = manager.players.split_at_mut(split);
                            let (current, after) = rest.split_first_mut().unwrap();
                            produce_in(
                                args,
                                enabled,
                                &mut SplitPlayers {
                                    before,
                                    current,
                                    after,
                                },
                                &mut arena,
                                Routing {
                                    current_player: 0x10010000 + config[2] * 0x1000,
                                    network: [config[3] != 0, config[4] != 0],
                                    prior,
                                },
                                &mut external,
                            )
                            .unwrap()
                        } else if opcode == 2 {
                            produce(
                                args,
                                &mut manager,
                                &mut arena,
                                0x10010000 + config[2] * 0x1000,
                                [config[3] != 0, config[4] != 0],
                                prior,
                                &mut external,
                            )
                            .unwrap()
                        } else {
                            let source =
                                descriptor([args[1], args[0], 0x31, 11, 22, 33, 44, 55], 0, prior);
                            let mut context = Context {
                                arena: Arena { next: arena.next },
                                current_player: 0x10010000 + config[2] * 0x1000,
                                network: [config[3] != 0, config[4] != 0],
                                prior,
                            };
                            let value = context
                                .reply(
                                    &source,
                                    args[2] as u32,
                                    args[3..7]
                                        .try_into()
                                        .map(|a: [i32; 4]| a.map(|v| v as u32))
                                        .unwrap(),
                                    &mut manager,
                                    &mut external,
                                )
                                .unwrap();
                            arena.next = context.arena.next;
                            value
                        };
                        if arena.next != before {
                            calls.borrow_mut().push(vec![1, before]);
                        }
                        u32::from(result)
                    }
                    _ => panic!("unknown fixture opcode"),
                };
                assert_eq!(result, expected_result, "{name}/{step} return");
                assert_eq!(arena.next, expected_next, "{name}/{step} allocation");
                assert_eq!(*calls.borrow(), expected_calls, "{name}/{step} calls");
                let actual = snapshot(&manager);
                if let Some(at) = actual.iter().zip(expected).position(|(a, b)| a != b) {
                    panic!(
                        "{name}/{step} byte {at:x}: {} != {}",
                        actual[at], expected[at]
                    );
                }
                assert_eq!(actual.len(), expected.len());
            }
        }
        assert!(r.0.is_empty());
    }
}

/// Original48cb70 routing with either the full roster or a borrowed split.
pub fn reply_in(
    source: &Packet,
    kind: u32,
    args: [u32; 4],
    players: &mut impl Players,
    arena: &mut Arena,
    routing: Routing,
    mut external: impl FnMut(Route, &Packet) -> Result<(), String>,
) -> Result<bool, String> {
    reply_with_players(source, kind, args, players, arena, routing, |r, p, _| {
        external(r, p)
    })
}
/// Keep callbacks synchronous while a split roster owns the current controller.
pub fn reply_with_players<P: Players>(
    source: &Packet,
    kind: u32,
    args: [u32; 4],
    players: &mut P,
    arena: &mut Arena,
    routing: Routing,
    external: impl FnMut(Route, &Packet, &mut P) -> Result<(), String>,
) -> Result<bool, String> {
    let mut packet = routing.prior;
    put(&mut packet, 0, 0x525dc0);
    packet[4..0x11].copy_from_slice(&source[4..0x11]);
    packet[0x18..0x30].copy_from_slice(&source[0x18..0x30]);
    packet[0x14] = source[0x15];
    packet[0x15] = source[0x14];
    if packet[0x15] != 0xff {
        let player = players
            .get(packet[0x15] as i8 as usize)
            .ok_or("reply recipient leaves roster")?;
        if player.bytes[0x10] == 0 {
            return Ok(false);
        }
    }
    put(&mut packet, 4, kind);
    for (index, value) in args.into_iter().enumerate() {
        if value != 0x5f5e0ff {
            put(&mut packet, 0x18 + index * 4, value);
        }
    }
    //450490 overwrites the descriptor size through its virtual size query.
    put(&mut packet, 8, 0x30);
    route(
        &packet,
        players,
        arena,
        routing.current_player,
        routing.network,
        external,
    )
}

/// Host allocation and explicit original stack-prior policy for one live session.
pub struct Context {
    pub arena: Arena,
    pub current_player: u32,
    pub network: [bool; 2],
    pub prior: Packet,
}
impl Context {
    ///48cb70 swaps sender/recipient and retains unspecified reply arguments.
    pub fn reply(
        &mut self,
        source: &Packet,
        kind: u32,
        args: [u32; 4],
        manager: &mut crate::player_manager::Manager,
        external: impl FnMut(Route, &Packet) -> Result<(), String>,
    ) -> Result<bool, String> {
        reply_in(
            source,
            kind,
            args,
            &mut manager.players,
            &mut self.arena,
            Routing {
                current_player: self.current_player,
                network: self.network,
                prior: self.prior,
            },
            external,
        )
    }
    pub fn produce(
        &mut self,
        args: [i32; 8],
        manager: &mut crate::player_manager::Manager,
        external: impl FnMut(Route, &Packet) -> Result<(), String>,
    ) -> Result<bool, String> {
        produce(
            args,
            manager,
            &mut self.arena,
            self.current_player,
            self.network,
            self.prior,
            external,
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HumanCall {
    Ui {
        address: u32,
        arguments: [u32; 3],
        count: usize,
    },
    /// Other original48bc70 branches remain synchronous, with the popped packet live.
    Packet {
        kind: u32,
        reference: u32,
    },
    Diagnostic,
}
/// Original48bc70 entry, passive-message cases and bankruptcy warning branch.
/// Complex property/negotiation dialogs are explicit packet callbacks.
pub fn consume_human(
    player: &mut Player,
    mut callback: impl FnMut(HumanCall, &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    let Some(reference) = pop(player, |_, _| Ok(()))? else {
        return Ok(());
    };
    put(&mut player.bytes, 0x20c, reference);
    let kind = word(&player.auxiliaries[auxiliary(player, reference)?].bytes, 4);
    match kind {
        0x25 | 0x27 | 0x28 | 0x2a | 0x2c | 0x2d | 0x2f => Ok(()),
        0x2b => {
            for (address, arguments, count) in [
                (0x43be20, [0, 0, 0], 0),
                (0x429af0, [0x12e, 0, 0], 1),
                (0x429820, [0, 0x6c3, 0], 3),
                (0x429820, [1, 0x6c4, 0], 3),
                (0x429a60, [1, 0, 0], 1),
                (0x4298b0, [0, 0x59b, 0], 2),
            ] {
                callback(
                    HumanCall::Ui {
                        address,
                        arguments,
                        count,
                    },
                    player,
                )?;
            }
            Ok(())
        }
        0x23 | 0x24 | 0x26 | 0x2e | 0x33 => callback(HumanCall::Packet { kind, reference }, player),
        _ => callback(HumanCall::Diagnostic, player),
    }
}
