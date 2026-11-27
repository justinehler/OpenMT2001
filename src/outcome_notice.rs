//! Original48b560 result dialogs and48b490 response selection.
use crate::{
    pe::PeImage,
    player_manager::{Player, word},
    scenario_objective,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Body,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Argument {
    None,
    Name([u8; 96]),
    Integer(i32),
    Currency(i32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Show,
    Timeout(u32),
    Clear(Field),
    Portrait(u32),
    Buttons(u32),
    Button {
        index: u32,
        language: u32,
    },
    Text {
        field: Field,
        language: u32,
        argument: Argument,
    },
    Handler {
        callback: Option<u32>,
        context: Option<u32>,
    },
    Pause,
    Timers(crate::pause::Timers),
}
pub struct World<'a, 'data> {
    pub image: &'a PeImage<'data>,
    pub players: &'a [Player],
    pub current_reference: u32,
    pub objective: &'a scenario_objective::State,
}
/// Preserved unresolved command identity. A native tie is winner=-1/reason2;
/// it is not a draw presentation, winner replacement or profile award.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnresolvedOutcome {
    pub winner: i32,
    pub reason: u32,
    pub roster_len: usize,
    pub objective_kind: i32,
    pub target_bits: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationError {
    Unresolved(UnresolvedOutcome),
    Boundary(String),
}

/// Pure preflight; no modal, queue mutation, winning semantics or recovery.
pub fn preflight(
    winner: i32,
    reason: u32,
    roster_len: usize,
    objective: &scenario_objective::State,
) -> Result<u32, UnresolvedOutcome> {
    if roster_len > 6 || winner < 0 || winner as usize >= roster_len {
        return Err(UnresolvedOutcome {
            winner,
            reason,
            roster_len,
            objective_kind: objective.kind,
            target_bits: objective.target.to_bits(),
        });
    }
    Ok(winner as u32)
}

/// Proposed checked boundary, deliberately not wired into command/Host paths.
/// Returning Unresolved must propagate out of queue dispatch (which retains
/// failed payloads); the host recovery disposition is a separate contract.
pub fn present_checked(
    winner: i32,
    reason: u32,
    world: &World<'_, '_>,
    call: impl FnMut(Call) -> Result<(), String>,
) -> Result<(), PresentationError> {
    let player = preflight(winner, reason, world.players.len(), world.objective)
        .map_err(PresentationError::Unresolved)?;
    present(player, reason, world, call).map_err(PresentationError::Boundary)
}
fn actor<'a>(world: &World<'a, '_>, slot: u32) -> Result<&'a Player, String> {
    world
        .players
        .get(slot as usize)
        .ok_or("outcome player outside roster".into())
}
fn name(world: &World<'_, '_>, slot: u32) -> Result<Argument, String> {
    Ok(Argument::Name(
        actor(world, slot)?.bytes[0x158..0x1b8].try_into().unwrap(),
    ))
}
pub fn present(
    player: u32,
    reason: u32,
    world: &World<'_, '_>,
    mut call: impl FnMut(Call) -> Result<(), String>,
) -> Result<(), String> {
    use Field::{Body, Title};
    call(Call::Show)?;
    call(Call::Timeout(30))?;
    call(Call::Clear(Title))?;
    call(Call::Clear(Body))?;
    let character = word(&actor(world, player)?.bytes, 0x150);
    call(Call::Portrait(word(
        world.image.at(0x541afc + character * 40, 4)?,
        0,
    )))?;
    call(Call::Buttons(1))?;
    call(Call::Button {
        index: 0,
        language: 1435,
    })?;
    let own = if reason <= 2 {
        let current = world
            .players
            .iter()
            .find(|p| p.reference == world.current_reference)
            .ok_or("outcome current player unresolved")?
            .bytes[0x1bc];
        u32::from(current) == player
    } else {
        false
    };
    match reason {
        0 | 1 => {
            let base = 0x73b + reason * 4;
            let argument = if own {
                Argument::None
            } else {
                name(world, player)?
            };
            call(Call::Text {
                field: Title,
                language: base + if own { 0 } else { 2 },
                argument,
            })?;
            call(Call::Text {
                field: Body,
                language: base + if own { 1 } else { 3 },
                argument,
            })?;
            if own {
                call(Call::Pause)?;
                call(Call::Handler {
                    callback: None,
                    context: Some(10),
                })?;
                call(Call::Handler {
                    callback: Some(0x48b490),
                    context: None,
                })?;
            }
        }
        2 => {
            let kind = world.objective.kind.wrapping_sub(1) as u32;
            if kind < 6 {
                // Original jump tables48bc14/48bc2c: objective order is not
                // the language-table order (net worth is the third case).
                let offset = [4, 5, 1, 2, 3, 0][kind as usize];
                let argument = match kind {
                    2 | 4 => Argument::Currency(crate::economy::ftol_low_word(f64::from(
                        world.objective.target,
                    ))),
                    3 | 5 => Argument::Integer(crate::economy::ftol_low_word(f64::from(
                        world.objective.target,
                    ))),
                    _ => Argument::None,
                };
                call(Call::Text {
                    field: Title,
                    language: if own { 0x723 } else { 0x729 } + offset,
                    argument,
                })?;
                call(Call::Text {
                    field: Body,
                    language: if own { 0x72f } else { 0x735 } + offset,
                    argument: if own {
                        Argument::None
                    } else {
                        name(world, player)?
                    },
                })?;
            }
            call(Call::Handler {
                callback: Some(0x48b490),
                context: None,
            })?;
            call(Call::Handler {
                callback: None,
                context: Some(player),
            })?;
            call(Call::Pause)?;
        }
        3 => {
            call(Call::Text {
                field: Title,
                language: 0x743,
                argument: name(world, player)?,
            })?;
            call(Call::Text {
                field: Body,
                language: 0x744,
                argument: name(world, player)?,
            })?;
        }
        4 => {
            call(Call::Text {
                field: Title,
                language: 0x745,
                argument: Argument::None,
            })?;
            call(Call::Text {
                field: Body,
                language: 0x746,
                argument: Argument::None,
            })?;
            call(Call::Handler {
                callback: Some(0x48b490),
                context: None,
            })?;
            call(Call::Handler {
                callback: None,
                context: Some(player),
            })?;
            call(Call::Pause)?;
        }
        _ => (),
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sequence {
    Win,
    Lose,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    ///4b3040 stores both character IDs for the result sequence actors.
    Characters {
        winner: u32,
        loser: u32,
    },
    Sequence(Sequence),
    Unlock(u32),
    Music {
        address: u32,
    },
}
pub fn respond(
    context: u32,
    level_id: u32,
    world: &World<'_, '_>,
    mut call: impl FnMut(Response) -> Result<(), String>,
) -> Result<(), String> {
    let current = world
        .players
        .iter()
        .find(|p| p.reference == world.current_reference)
        .ok_or("outcome current player unresolved")?;
    let character = word(&current.bytes, 0x150);
    if context == u32::from(current.bytes[0x1bc]) {
        call(Response::Characters {
            winner: character,
            loser: 10,
        })?;
        call(Response::Sequence(if level_id == 0x30d50 {
            Sequence::Complete
        } else {
            Sequence::Win
        }))?;
        if level_id == 0x30d50 {
            call(Response::Unlock(400001))?;
        }
        call(Response::Music { address: 0x541d0c })?;
    } else {
        let winner = if context == 10 {
            10
        } else {
            word(&actor(world, context)?.bytes, 0x150)
        };
        call(Response::Characters {
            winner,
            loser: character,
        })?;
        call(Response::Sequence(Sequence::Lose))?;
        call(Response::Music { address: 0x541cdc })?;
    }
    Ok(())
}

#[test]
#[ignore = "requires original outcome dialog and response observations"]
fn matches_native_outcomes() {
    struct R<'a>(&'a [u8]);
    impl<'a> R<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            word(self.take(4), 0)
        }
    }
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let data = std::fs::read(crate::fixtures::path("NATIVE_OUTCOME_LIVE")).unwrap();
    let mut r = R(&data);
    assert_eq!(r.take(8), b"MTOUTL02");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 8] = std::array::from_fn(|_| r.word());
        let mut pause = crate::pause::State {
            bytes: r.take(9).try_into().unwrap(),
        };
        let pause_after = r.take(9);
        let players: Vec<_> = (0..6)
            .map(|i| Player {
                reference: 0x10020000 + i * 0x1000,
                bytes: r.take(548).to_vec(),
                auxiliaries: vec![],
            })
            .collect();
        let after: [u32; 5] = std::array::from_fn(|_| r.word());
        let expected: Vec<Vec<u32>> = (0..h[7])
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let objective = scenario_objective::State {
            kind: h[4] as i32,
            target: f32::from_bits(h[5]),
            ..Default::default()
        };
        let world = World {
            image: &image,
            players: &players,
            current_reference: players[h[2] as usize].reference,
            objective: &objective,
        };
        let mut observed = vec![];
        let mut handler = [0; 2];
        let mut characters = [0xdead, 0xbeef];
        let mut unlocked = 0;
        if h[0] == 0x48b560 {
            present(h[1], h[3], &world, |call| {
                let row = match call {
                    Call::Show => {
                        handler = [0; 2];
                        vec![0]
                    }
                    Call::Timeout(v) => vec![1, v],
                    Call::Clear(f) => vec![2, u32::from(f == Field::Body)],
                    Call::Portrait(v) => vec![3, v],
                    Call::Buttons(v) => vec![4, v],
                    Call::Button { index, language } => vec![5, index, language],
                    Call::Text {
                        field,
                        language,
                        argument,
                    } => {
                        let (t, v) = match argument {
                            Argument::None => (0, 0),
                            Argument::Integer(v) => (2, v as u32),
                            Argument::Currency(v) => (3, v as u32),
                            Argument::Name(n) => (
                                1,
                                players
                                    .iter()
                                    .position(|p| p.bytes[0x158..0x1b8] == n)
                                    .unwrap() as u32,
                            ),
                        };
                        vec![6, u32::from(field == Field::Body), language, t, v]
                    }
                    Call::Handler { callback, context } => {
                        if let Some(v) = callback {
                            handler[0] = v;
                            vec![7, 0, v]
                        } else {
                            let v = context.unwrap();
                            handler[1] = v;
                            vec![7, 1, v]
                        }
                    }
                    Call::Pause => {
                        observed.push(vec![8]);
                        pause.pause(0, |_, _| {
                            observed.push(vec![9]);
                            Ok(())
                        })?;
                        return Ok(());
                    }
                    Call::Timers(_) => unreachable!(),
                };
                observed.push(row);
                Ok(())
            })
            .unwrap();
        } else {
            respond(h[1], h[6], &world, |call| {
                observed.push(match call {
                    Response::Characters { winner, loser } => {
                        characters = [winner, loser];
                        vec![10, winner, loser]
                    }
                    Response::Sequence(s) => vec![
                        11,
                        match s {
                            Sequence::Win => 0,
                            Sequence::Lose => 1,
                            Sequence::Complete => 2,
                        },
                    ],
                    Response::Unlock(v) => {
                        assert_eq!(v, 400001);
                        unlocked = 1;
                        vec![12, v]
                    }
                    Response::Music { address } => vec![13, address],
                });
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(observed, expected, "trace{case}");
        assert_eq!(pause.bytes.as_slice(), pause_after, "pause{case}");
        assert_eq!(
            [
                handler[0],
                handler[1],
                characters[0],
                characters[1],
                unlocked
            ],
            after,
            "stores{case}"
        );
    }
    for case in 0..r.word() {
        let timer = r.word();
        assert_eq!(r.word(), timer, "retained timer identity{case}");
        let mut tasks: [[u8; 20]; 3] = std::array::from_fn(|_| r.take(20).try_into().unwrap());
        let after = r.take(60);
        crate::player_elimination::finish_local(
            (timer != u32::MAX).then_some(timer as usize),
            &mut tasks,
        )
        .unwrap();
        assert_eq!(tasks.as_flattened(), after, "finish-local{case}");
    }
    assert!(r.0.is_empty());
}
