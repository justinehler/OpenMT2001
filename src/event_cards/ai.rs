//! Original46f130 event evaluation and46c080 time gates.
use super::lifecycle::{auxiliary, card_row};
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Choose(u32),
    Select { category: i32, building: i32 },
    Retire(u32),
    Discard(u32),
    Diagnostic { kind: u32, used: bool },
}
#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub target: i32,
    pub score: f32,
    pub count: u32,
}
impl Default for Choice {
    fn default() -> Self {
        Self {
            target: -1,
            score: 0.,
            count: 0,
        }
    }
}

///470ad0 tests the calendar's third DWORD and signed hour.
pub fn allowed(kind: u32, hour: i32, period: u32) -> bool {
    let initial = period == 0 && hour <= 23;
    match kind {
        0 | 4 => matches!(hour,6..=9|15..=18),
        1 | 7 => !initial,
        2 => (hour == 6 && !initial) || matches!(hour,7..=9|15..=18),
        3 => (hour == 10 && !initial) || matches!(hour,11..=14|19..=21),
        5 | 6 => !initial && matches!(hour, 4..=7),
        8 => (hour == 6 && !initial) || matches!(hour, 7..=9),
        _ => false,
    }
}

/// Callbacks remain synchronous: selection validates the retained auxiliary,
/// then its resulting flag determines retirement. Retirement/discard precede
/// clearing the AI warning and age fields, even if retirement rejects a target.
pub fn evaluate(
    player: &mut Player,
    hour: i32,
    period: u32,
    image: &PeImage<'_>,
    mut call: impl FnMut(Call, &mut Player) -> Result<Choice, String>,
) -> Result<(), String> {
    let reference = word(&player.bytes, 0x21c);
    let index = auxiliary(player, reference)?;
    let kind = word(
        card_row(image, word(&player.auxiliaries[index].bytes, 4))?,
        8,
    );
    let mut discard = false;
    if allowed(kind, hour, period) {
        let choice = call(Call::Choose(kind), player)?;
        let threshold = |address| -> Result<f32, String> {
            Ok(f32::from_le_bytes(
                image.at(address, 4)?.try_into().unwrap(),
            ))
        };
        let mut select = false;
        match kind {
            5 | 6 => {
                select = choice.target > -1 && choice.score > threshold(0x525bb0)?;
                if !select {
                    discard = choice.score < 0. || choice.score.is_nan();
                }
            }
            0..=4 => {
                discard = (word(&player.bytes, 0x13e0) as i32) > 48 || choice.count == 0;
                let limit = match kind {
                    0 | 4 => 0x525ba4,
                    1 => 0x525bb0,
                    _ => 0x525178,
                };
                select = !discard
                    && choice.target > -1
                    && (player.bytes[0x13dd] != 0 || choice.score > threshold(limit)?);
            }
            7 => {
                discard = choice.count == 0;
                select = !discard && choice.target > -1 && choice.score > 0.;
            }
            8 => {
                discard = choice.count == 0;
                select = !discard && choice.target > -1;
            }
            _ => (),
        }
        if select {
            let category = matches!(kind, 1 | 5 | 6 | 7);
            call(
                Call::Select {
                    category: if category { choice.target } else { -1 },
                    building: if category { -1 } else { choice.target },
                },
                player,
            )?;
        }
    }
    let used = player.auxiliaries[index].bytes[8] != 0;
    if used || discard {
        if player.bytes[0x13c1] != 0 {
            call(Call::Diagnostic { kind, used }, player)?;
        }
        call(
            if used {
                Call::Retire(reference)
            } else {
                Call::Discard(reference)
            },
            player,
        )?;
        player.bytes[0x13dd] = 0;
        put(&mut player.bytes, 0x13e0, 0);
    }
    Ok(())
}

///41ca00 stores native references, then immediately invokes41ca60 validation.
/// Category=-1 deliberately preserves the stale geometry reference at+c.
pub fn select(
    player: &mut Player,
    reference: u32,
    category: i32,
    building: i32,
    mut geometry: impl FnMut(i32) -> Result<u32, String>,
    mut validate: impl FnMut(&mut Player, u32) -> Result<i32, String>,
) -> Result<i32, String> {
    let index = auxiliary(player, reference)?;
    if category != -1 {
        put(
            &mut player.auxiliaries[index].bytes,
            0xc,
            geometry(category)?,
        );
    }
    let reference_building = if building == -1 {
        0
    } else {
        0x591318u32.wrapping_add((building as u32).wrapping_mul(0x394))
    };
    put(
        &mut player.auxiliaries[index].bytes,
        0x10,
        reference_building,
    );
    validate(player, reference)
}

#[derive(Clone, Copy, Debug)]
pub enum RuntimeCall {
    Submit(super::lifecycle::Command),
    Broadcast([i32; 8]),
    Diagnostic { kind: u32, used: bool },
}
pub struct Bindings<'a, 'image> {
    pub world: super::selection::World<'a>,
    pub targets: super::lifecycle::Targets<'a>,
    pub period: u32,
    pub image: &'a PeImage<'image>,
}
/// Bind46f020's two downstream calls to selection, validation and lifecycle.
/// Commands and broadcast messages are produced before auxiliary reset stores.
pub fn run_update(
    player: &mut Player,
    update: crate::ai_hourly::UpdateCall,
    binding: &Bindings<'_, '_>,
    mut call: impl FnMut(RuntimeCall, &mut Player) -> Result<(), String>,
) -> Result<(), String> {
    let mut dispatch = |event: Call, player: &mut Player| -> Result<Choice, String> {
        match event {
            Call::Choose(kind) => {
                return super::selection::choose(kind, player, &binding.world, binding.image);
            }
            Call::Select { category, building } => {
                let reference = word(&player.bytes, 0x21c);
                select(
                    player,
                    reference,
                    category,
                    building,
                    |category| {
                        Ok(binding
                            .targets
                            .nodes
                            .iter()
                            .find(|n| n.kind == 4 && n.category == category as u32)
                            .map_or(0, |n| n.reference))
                    },
                    |p, a| binding.targets.validate(p, a, binding.image),
                )?;
            }
            Call::Retire(reference) => {
                super::lifecycle::retire(
                    player,
                    reference,
                    |p, a| binding.targets.validate(p, a, binding.image),
                    |command, p| call(RuntimeCall::Submit(command), p),
                )?;
            }
            Call::Discard(reference) => {
                super::lifecycle::discard(player, reference, binding.image, |args, p| {
                    call(RuntimeCall::Broadcast(args), p)
                })?;
            }
            Call::Diagnostic { kind, used } => {
                call(RuntimeCall::Diagnostic { kind, used }, player)?
            }
        }
        Ok(Choice::default())
    };
    match update {
        crate::ai_hourly::UpdateCall::Evaluate => evaluate(
            player,
            binding.world.hour,
            binding.period,
            binding.image,
            dispatch,
        ),
        crate::ai_hourly::UpdateCall::Retire { account } => {
            dispatch(Call::Retire(account), player).map(|_| ())
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
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires original46f130/46c080 observations"]
    fn matches_native_event_ai() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_EVENT_AI")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTECAI01");
        assert_eq!(r.word(), 2);
        assert_eq!(r.word(), 0x037f);
        for _ in 0..r.word() {
            let [k, h, p, result] = std::array::from_fn::<_, 4, _>(|_| r.word());
            assert_eq!(
                u32::from(allowed(k, h as i32, p)),
                result,
                "gate {k} {h} {p}"
            );
        }
        for case in 0..r.word() {
            let [kind, hour, period, target, score, count, accept] =
                std::array::from_fn::<_, 7, _>(|_| r.word());
            let mut p = Player {
                reference: 0x10010000,
                bytes: r.blob().to_vec(),
                auxiliaries: vec![crate::player_manager::Auxiliary {
                    reference: 0x10014000,
                    bytes: r.blob().try_into().unwrap(),
                }],
            };
            let expected_player = r.blob();
            let expected_aux = r.blob();
            let expected_trace: Vec<[u32; 3]> = (0..r.word())
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut trace = vec![];
            evaluate(&mut p, hour as i32, period, &image, |call, p| {
                let row = match call {
                    Call::Choose(k) => {
                        assert_eq!(k, kind);
                        [
                            match k {
                                0 | 4 => 0x46f610,
                                1 => 0x46f8d0,
                                2 => 0x46f990,
                                3 => 0x46fbb0,
                                5 | 6 => 0x46f500,
                                7 => 0x46fe20,
                                8 => 0x46fef0,
                                _ => unreachable!(),
                            },
                            0,
                            0,
                        ]
                    }
                    Call::Select { category, building } => {
                        p.auxiliaries[0].bytes[8] = accept as u8;
                        [0x41ca00, category as u32, building as u32]
                    }
                    Call::Retire(a) | Call::Discard(a) => {
                        p.bytes[0x13dd] = 1;
                        put(&mut p.bytes, 0x13e0, 999);
                        [
                            if matches!(call, Call::Retire(_)) {
                                0x41bc90
                            } else {
                                0x41bd70
                            },
                            a,
                            0,
                        ]
                    }
                    Call::Diagnostic { kind, used } => [0x4e6960, kind, u32::from(used)],
                };
                trace.push(row);
                Ok(Choice {
                    target: target as i32,
                    score: f32::from_bits(score),
                    count,
                })
            })
            .unwrap();
            assert_eq!(p.bytes, expected_player, "player case{case}");
            assert_eq!(p.auxiliaries[0].bytes, expected_aux, "aux case{case}");
            assert_eq!(trace, expected_trace, "trace case{case}");
        }
        assert!(r.0.is_empty());
    }
}
