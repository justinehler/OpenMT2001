//! Original464610 auction command consumer and464820 bidder rotation.
pub mod activation;
pub mod admission;
pub mod announcement;
pub mod broadcast;
pub mod budget;
pub mod closure;
pub mod inbox;
pub mod progress;
pub mod valuation;
use crate::player_manager::{put, word};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub action: u32,
    pub participant: u32,
    pub amount: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    ParticipantEffect { participant: u32, kind: u32 },
    Tutorial(u32),
    ShowPanel(u32),
    RefreshTurn(u32),
    WithdrawNotice(u32),
}
fn row(participant: u32) -> Result<usize, String> {
    if participant >= 6 {
        return Err("auction participant outside six rows".into());
    }
    Ok(participant as usize * 12)
}

/// UI selection is a synchronous boundary enabled by original68596c.
pub fn rotate(
    state: &mut [u8; 128],
    count: i32,
    presentation: bool,
    call: &mut impl FnMut(Call, &mut [u8; 128]) -> Result<(), String>,
) -> Result<(), String> {
    if !(1..=6).contains(&count) {
        return Err("auction bidder count outside retained pool".into());
    }
    let previous = word(state, 0x58);
    let owner = word(state, 0x50);
    put(state, 0x78, 0);
    let mut candidate = previous;
    let mut seen = [false; 6];
    loop {
        candidate = candidate.wrapping_add(1);
        put(state, 0x58, candidate);
        if candidate == owner {
            candidate = candidate.wrapping_add(1);
            put(state, 0x58, candidate);
        }
        if candidate as i32 >= count {
            candidate = 0;
            put(state, 0x58, 0);
        }
        let at = row(candidate)?;
        if candidate == previous || state[at + 8] != 0 {
            break;
        }
        if seen[candidate as usize] {
            return Err("original auction rotation would not terminate".into());
        }
        seen[candidate as usize] = true;
    }
    let active = (0..count as usize)
        .filter(|&i| state[i * 12 + 8] != 0)
        .count() as u32;
    let remaining = active.wrapping_sub(word(state, 0x64));
    if word(state, 0x5c) == candidate && remaining == 1 {
        put(state, 0x60, candidate);
    } else if remaining == 0 {
        state[0x70] = 1;
    }
    if state[0x70] == 0 && state[row(candidate)? + 0x10] != 0 {
        state[row(candidate)? + 0x10] = 0;
    }
    if presentation {
        call(Call::RefreshTurn(candidate), state)?;
    }
    Ok(())
}

pub fn apply(
    command: Command,
    state: &mut [u8; 128],
    count: i32,
    presentation: bool,
    mut call: impl FnMut(Call, &mut [u8; 128]) -> Result<(), String>,
) -> Result<(), String> {
    let Command {
        action,
        participant,
        amount,
    } = command;
    match action {
        0 => {
            if word(state, 0x6c) == amount {
                return Ok(());
            }
            call(
                Call::ParticipantEffect {
                    participant,
                    kind: 2,
                },
                state,
            )?;
            put(state, 0x54, participant);
            put(state, 0x6c, amount);
            let at = row(participant)?;
            state[at + 0x10] = 0;
            put(state, at + 12, amount);
            put(state, 0x5c, participant);
            put(state, 0x64, 0);
            rotate(state, count, presentation, &mut call)?;
        }
        1 => {
            let passed = word(state, 0x64).wrapping_add(1);
            put(state, 0x64, passed);
            state[row(participant)? + 0x10] = 1;
            put(state, 0x54, participant);
            rotate(state, count, presentation, &mut call)?;
        }
        2 => {
            let at = row(participant)?;
            state[at + 8] = 0;
            state[at + 0x12] = 1;
            rotate(state, count, presentation, &mut call)?;
            if word(state, 0x60) == u32::MAX {
                call(Call::WithdrawNotice(participant), state)?;
            }
        }
        3 => {
            state[row(word(state, 0x58))? + 8] = 0;
        }
        4 => {
            if participant == 3 {
                call(Call::Tutorial(0x44), state)?;
                call(
                    Call::ParticipantEffect {
                        participant: word(state, 0x60),
                        kind: 5,
                    },
                    state,
                )?;
            } else if participant == 2 {
                for address in [0x565b9c, 0x565ba4, 0x565ba0, 0x565b94, 0x565ba8] {
                    call(Call::ShowPanel(address), state)?;
                }
                call(Call::Tutorial(0x3f), state)?;
            }
            put(state, 0x74, participant);
        }
        _ => (),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original v1.4 auction capture"]
    fn matches_native_auction() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_AI_AUCTION")).unwrap();
        assert_eq!(&bytes[..8], b"MTAOAU01");
        let mut data = &bytes[8..];
        fn take<'a>(data: &mut &'a [u8], n: usize) -> &'a [u8] {
            let (value, rest) = data.split_at(n);
            *data = rest;
            value
        }
        fn next(data: &mut &[u8]) -> u32 {
            u32::from_le_bytes(take(data, 4).try_into().unwrap())
        }
        assert_eq!(next(&mut data), 0x037f);
        let count = next(&mut data);
        for case in 0..count {
            let command = Command {
                action: next(&mut data),
                participant: next(&mut data),
                amount: next(&mut data),
            };
            let presentation = next(&mut data) != 0;
            let mutation = next(&mut data) != 0;
            let mut state: [u8; 128] = take(&mut data, 128).try_into().unwrap();
            let expected = take(&mut data, 128);
            let traces = next(&mut data);
            let expected_trace: Vec<[u32; 3]> = (0..traces)
                .map(|_| std::array::from_fn(|_| next(&mut data)))
                .collect();
            let mut trace = Vec::new();
            apply(command, &mut state, 6, presentation, |call, state| {
                trace.push(match call {
                    Call::ParticipantEffect { participant, kind } => {
                        if mutation {
                            put(state, 0x58, 4);
                            put(state, 0x60, 5);
                        }
                        [0, participant, kind]
                    }
                    Call::Tutorial(value) => [1, value, 0],
                    Call::ShowPanel(value) => [2, value, 0],
                    Call::RefreshTurn(value) => [3, value, 0],
                    Call::WithdrawNotice(value) => [4, value, 0],
                });
                Ok(())
            })
            .unwrap();
            assert_eq!(state.as_slice(), expected, "case {case}, {command:?}");
            assert_eq!(trace, expected_trace, "case {case}, {command:?}");
        }
        assert!(data.is_empty());
    }
}

/// One retained auction allocation, including the announcement controller.
/// Cleanup leaves its identity and last result alive for queued player messages.
#[derive(Default)]
pub struct State {
    pub controller: admission::Controller,
    pub presentation: admission::Presentation,
    pub participant_ui: [u32; 7],
    pub bid_ui: [u32; 7],
    pub opening: u8,
}
impl State {
    pub fn message_view(&self) -> Option<crate::ai_messages::Auction> {
        (self.controller.reference != 0).then(|| crate::ai_messages::Auction {
            participant: word(&self.controller.record, 0x54),
            winner: word(&self.controller.record, 0x60) as i32,
            price: word(&self.controller.record, 0x6c) as i32,
        })
    }
    pub fn guard_view(&self) -> Option<crate::ai_guards::Auction> {
        (self.controller.reference != 0).then_some(crate::ai_guards::Auction {
            active: self.controller.record[0x7d] != 0,
            progress: self.presentation.countdown as i32,
        })
    }
    pub fn offer_view(&self) -> Option<crate::ai_messages::offers::Auction> {
        (self.controller.reference != 0).then(|| crate::ai_messages::offers::Auction {
            category: word(&self.controller.record, 4),
            active: self.controller.record[0x7d],
        })
    }
    pub fn elimination_view(&self) -> Option<crate::player_elimination::Auction> {
        (self.controller.reference != 0).then(|| crate::player_elimination::Auction {
            reference: self.controller.reference,
            category: word(&self.controller.record, 4) as i32,
            player: word(&self.controller.record, 0x5c),
        })
    }
}

/// Original4644a0/4644f0/464530 immediate pass/withdraw/bid descriptors.
///450490/451720 routing is performed synchronously by the caller.
///472750 adds the script-supplied increment to the live current bid.
pub fn forced_action_descriptor(
    address: u32,
    participant: u32,
    amount: u32,
    state: &[u8; 128],
    prior: [u8; 32],
) -> Result<[u8; 32], String> {
    let amount = if address == 0x464530 {
        word(state, 0x6c).wrapping_add(amount)
    } else {
        amount
    };
    action_descriptor(address, participant, amount, prior)
}

pub fn action_descriptor(
    address: u32,
    participant: u32,
    amount: u32,
    mut prior: [u8; 32],
) -> Result<[u8; 32], String> {
    let action = match address {
        0x4644a0 => 1,
        0x4644f0 | 0x464580 => 2,
        0x464530 => 0,
        _ => return Err("auction action producer unsupported".into()),
    };
    put(&mut prior, 0, 0x525c58);
    put(&mut prior, 4, 0x1f);
    put(&mut prior, 12, 0);
    prior[16] = 0;
    put(&mut prior, 20, action);
    put(&mut prior, 24, participant);
    if action == 0 {
        put(&mut prior, 28, amount);
    }
    Ok(prior)
}

#[test]
#[ignore = "requires original auction action producer capture"]
fn matches_native_action_producers() {
    let b = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_ACTION_PRODUCERS")).unwrap();
    assert_eq!(&b[..8], b"MTAUAP01");
    assert_eq!(word(&b, 8), 0x037f);
    assert_eq!(b.len(), 16 + word(&b, 12) as usize * 44);
    for (case, r) in b[16..].chunks_exact(44).enumerate() {
        assert_eq!(
            &action_descriptor(word(r, 0), word(r, 4), word(r, 8), [0xcc; 32]).unwrap(),
            &r[12..],
            "case{case}"
        );
    }
}
