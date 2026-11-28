use super::*;
use openmt2001::ai_messages::offers::auction::{State as Auction, announcement};

#[test]
fn native_auction_validation_uses_clock_mode_independently_of_session_option() {
    for (logic_mode, option) in [(1u32, 2u32), (2, 0), (2, 3)] {
        let mut source = Auction::default();
        source.controller.reference = 0x7f4758;
        source.controller.record[0x7c] = u8::from(logic_mode == 2);
        source.controller.record[0x7d] = u8::from(logic_mode == 1);
        source.controller.active_count = 1;
        let host = AuctionHostState {
            controls: Default::default(),
            visible: logic_mode == 2,
            interface_ticks: 0,
            announcement_seconds: None,
        };
        let continuation =
            AuctionContinuation::capture_state(&source, 0, logic_mode, Some(host), 0).unwrap();
        let mut bytes = Vec::new();
        for (tag, payload) in [
            (0x200u32, vec![0; 20]),
            (
                0x400,
                [0, 0, 0, 0, 0, logic_mode]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            ),
            (0x400000, option.to_le_bytes().to_vec()),
            (0x8000, source.controller.record.to_vec()),
        ] {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&payload);
        }
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
        assert_eq!(save.session_option_word().unwrap(), option);
        continuation.validate_native(&save).unwrap();
        let mut mismatched = continuation;
        mismatched.game_mode = if logic_mode == 1 { 2 } else { 1 };
        assert!(mismatched.validate_native(&save).is_err());
    }
}

#[test]
fn bidding_and_settled_continuations_keep_omitted_countdown() {
    for mode in [1, 2] {
        let mut source = Auction::default();
        source.controller.reference = 0x7f4758;
        source.controller.record[0x7c] = u8::from(mode == 2);
        source.presentation.countdown = 17;
        let host = AuctionHostState {
            controls: Default::default(),
            visible: mode == 2,
            interface_ticks: 0,
            announcement_seconds: None,
        };
        let saved = AuctionContinuation::capture_state(&source, 0, mode, Some(host), 0).unwrap();
        let mut native = Default::default();
        saved.apply_presentation(&mut native);
        assert_eq!(native.countdown, 17);
    }
}

#[test]
fn announcement_continuation_keeps_native_seconds_and_beep_boundaries() {
    for prior in [0, 3] {
        for (ticks, restored, seconds) in
            [(149, 100, vec![1, 0]), (50, 50, vec![0]), (49, 0, vec![])]
        {
            let mut source = Auction::default();
            source.controller.reference = 0x7f4758;
            source.controller.active_count = 1;
            source.controller.record[0x7d] = 1;
            put(&mut source.controller.record, 4, 36);
            put(&mut source.controller.record, 0x5c, 0);
            put(&mut source.controller.record, 0x80, ticks);
            put(&mut source.controller.record, 0x88, 100);
            source.presentation.visible = true;
            source.presentation.announcement = [36, 0, 100];
            source.presentation.countdown = ticks;
            source.presentation.countdown_last = (ticks as i32 + 49) / 50;
            let host = AuctionHostState {
                controls: Default::default(),
                visible: false,
                interface_ticks: 0,
                announcement_seconds: Some(source.presentation.countdown_last),
            };
            let saved =
                AuctionContinuation::capture_state(&source, 0, 1, Some(host), prior).unwrap();
            let saved = AuctionContinuation::decode(&saved.encode()).unwrap();
            let mut native = Auction::default();
            let mut categories = vec![[0; 380]; 52];
            openmt2001::auction_restore::restore(
                &source.controller.record,
                &mut native,
                &mut categories,
                &[1],
                0,
                1,
                |_, _, p, _| {
                    p.visible = true;
                    Ok(())
                },
            )
            .unwrap();
            saved.apply_presentation(&mut native.presentation);
            assert_eq!(native.presentation.countdown, restored);
            let mut text = Vec::new();
            let mut beeps = 0;
            for _ in 0..150 {
                announcement::advance(&mut native.presentation, true, false, |call, _| {
                    match call {
                        announcement::Call::WriteSeconds(s) => text.push(s),
                        announcement::Call::CreateSound => beeps += 1,
                        _ => {}
                    }
                    Ok(0)
                })
                .unwrap();
            }
            assert_eq!(text, seconds);
            assert_eq!(beeps, seconds.len());
            assert_eq!(
                native.controller.record[0x7c], 0,
                "zero must not start bidding"
            );
            assert_eq!(native.controller.record[0x7d], 1);
        }
    }
}
