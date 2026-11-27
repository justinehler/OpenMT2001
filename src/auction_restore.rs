//! Original 465810 restoration of the retained 0x158-byte auction object.
//! The saved bytes live in the existing auction controller at 0x7f4758.
use crate::ai_messages::offers::auction::{State, admission};
use crate::player_manager::word;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    ReleaseAmbient(u32),
    /// 4b5370(record+150), before 465490's scene/panel restoration.
    SetGameMode(u32),
    PrepareAuctionScene,
    ShowAnnouncement,
    CategoryStatus(u32),
}

/// `active` is the original per-player active byte used by 464970. `mode`
/// is global 822d30 at the instant tag 0x8000 is consumed. Host presentation
/// calls remain synchronous boundaries; the authoritative auction record and
/// category lock are changed here in original instruction order.
pub fn restore(
    saved: &[u8; 0x158],
    state: &mut State,
    categories: &mut [[u8; 380]],
    active: &[u8],
    current_slot: u8,
    mode: u32,
    mut call: impl FnMut(
        Call,
        &mut admission::Controller,
        &mut admission::Presentation,
        &mut [[u8; 380]],
    ) -> Result<(), String>,
) -> Result<(), String> {
    state.controller.reference = 0x7f4758;
    state.controller.record.copy_from_slice(saved);
    let category = word(saved, 4);
    let bidder = word(saved, 0x84);
    let amount = word(saved, 0x88);
    if state.controller.active_count == 0 {
        let mut world = admission::World {
            controller: &mut state.controller,
            categories,
            active,
            presentation: &mut state.presentation,
            current_slot,
            tutorial: false,
            network: [false; 2],
            command_prior: [0; 20],
        };
        admission::initialize(category, bidder, amount, &mut world, &mut |event, world| {
            match event {
                admission::Call::ReleaseAmbient(handle) => call(
                    Call::ReleaseAmbient(handle),
                    world.controller,
                    world.presentation,
                    world.categories,
                )?,
                other => return Err(format!("465810 initializer emitted {other:?}")),
            }
            Ok(())
        })?;
        state.controller.active_count = state.controller.active_count.wrapping_add(1);
        state.controller.record[0x7d] = 1;
    } else if bidder == u32::from(current_slot) {
        categories
            .get_mut(category as usize)
            .ok_or("restored auction category outside pool")?[0x6c] = 0;
    }
    if mode == 2 {
        call(
            Call::SetGameMode(word(&state.controller.record, 0x150)),
            &mut state.controller,
            &mut state.presentation,
            categories,
        )?;
        call(
            Call::PrepareAuctionScene,
            &mut state.controller,
            &mut state.presentation,
            categories,
        )?;
        return Ok(());
    }
    let category = word(&state.controller.record, 4);
    state.presentation.announcement = [
        category,
        word(&state.controller.record, 0x5c),
        word(&state.controller.record, 0x88),
    ];
    let countdown =
        i32::from_le_bytes(state.controller.record[0x80..0x84].try_into().unwrap()) / 50;
    call(
        Call::ShowAnnouncement,
        &mut state.controller,
        &mut state.presentation,
        categories,
    )?;
    // 41db40 stores seconds*50 at the presentation+0xb00 counter. The
    // signed divide above discards a saved sub-second remainder.
    state.presentation.countdown = countdown.wrapping_mul(50) as u32;
    // 465810 reloads the global pointer after 41db40.
    let category = word(&state.controller.record, 4);
    categories
        .get_mut(category as usize)
        .ok_or("restored auction category outside pool")?[0x6c] = 1;
    call(
        Call::CategoryStatus(category),
        &mut state.controller,
        &mut state.presentation,
        categories,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original v1.4 auction restore capture"]
    fn matches_native_465810() {
        struct Reader<'a>(&'a [u8]);
        impl<'a> Reader<'a> {
            fn take(&mut self, n: usize) -> &'a [u8] {
                let (front, rest) = self.0.split_at(n);
                self.0 = rest;
                front
            }
            fn word(&mut self) -> u32 {
                u32::from_le_bytes(self.take(4).try_into().unwrap())
            }
        }
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_RESTORE").unwrap()).unwrap();
        let mut reader = Reader(&bytes);
        assert_eq!(reader.take(8), b"MTAURST2");
        assert_eq!(reader.word(), 2);
        assert_eq!(reader.word(), 0x037f);
        let count = reader.word();
        assert!(count >= 7);
        for case in 0..count {
            let [active_count, mode, current_slot, owner, category] =
                std::array::from_fn(|_| reader.word());
            let saved: &[u8; 0x158] = reader.take(0x158).try_into().unwrap();
            let expected_record = reader.take(0x158);
            let expected_reference = reader.word();
            let expected_count = reader.word();
            let expected_mode = reader.word();
            let expected_categories = reader.take(52);
            let expected_view: [u32; 4] = std::array::from_fn(|_| reader.word());
            let native_calls = (0..reader.word())
                .map(|_| {
                    (0..reader.word())
                        .map(|_| reader.word())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut state = State::default();
            state.controller.active_count = active_count;
            let mut categories = vec![[0xa5u8; 380]; 52];
            for row in &mut categories {
                row[5..9].fill(0);
            }
            categories[category as usize][5] = owner as u8;
            let active = [1, 1, 1, 1, 0, 0];
            let mut calls = Vec::new();
            restore(
                saved,
                &mut state,
                &mut categories,
                &active,
                current_slot as u8,
                mode,
                |call, controller, _, _| {
                    let entry = match call {
                        Call::ReleaseAmbient(_) => panic!("unexpected ambient callback"),
                        Call::SetGameMode(value) => (0x4b5370, Some(value)),
                        Call::PrepareAuctionScene => {
                            if case == 6 {
                                controller.record[4..8].copy_from_slice(&15u32.to_le_bytes());
                            }
                            (0x465490, None)
                        }
                        Call::ShowAnnouncement => {
                            if case == 4 {
                                controller.record[4..8].copy_from_slice(&15u32.to_le_bytes());
                            }
                            (0x43be20, None)
                        }
                        Call::CategoryStatus(category) => (0x41ed10, Some(category)),
                    };
                    calls.push(entry);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                state.controller.record.as_slice(),
                expected_record,
                "case {case} record"
            );
            assert_eq!(
                state.controller.reference, expected_reference,
                "case {case} pointer"
            );
            assert_eq!(
                state.controller.active_count, expected_count,
                "case {case} count"
            );
            assert_eq!(mode, expected_mode, "case {case} mode");
            let statuses: Vec<_> = categories.iter().map(|row| row[0x6c]).collect();
            assert_eq!(statuses, expected_categories, "case {case} categories");
            assert_eq!(
                [
                    state.presentation.countdown,
                    state.presentation.announcement[0],
                    state.presentation.announcement[1],
                    state.presentation.announcement[2],
                ],
                expected_view,
                "case {case} view"
            );
            if mode == 2 {
                assert_eq!(native_calls[0][0], 0x4b5370, "case {case} mode call");
                assert_eq!(native_calls[0][2], word(saved, 0x150));
                assert_eq!(
                    native_calls.iter().any(|row| row[0] == 0x464aa0),
                    state.controller.record[0x7d] != 0,
                    "case {case} 465490 active gate"
                );
                assert_eq!(
                    calls,
                    vec![(0x4b5370, Some(word(saved, 0x150))), (0x465490, None)]
                );
            } else {
                assert_eq!(
                    native_calls.iter().map(|row| row[0]).collect::<Vec<_>>(),
                    vec![0x43be20, 0x41ed10],
                    "case {case} normal call order"
                );
                assert_eq!(
                    calls,
                    vec![(0x43be20, None), (0x41ed10, Some(native_calls[1][2]))],
                    "case {case} callbacks"
                );
            }
        }
        assert!(reader.0.is_empty());
    }
}
