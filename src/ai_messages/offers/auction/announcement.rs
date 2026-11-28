//! Original41d900 message1, including the8b22d8 pause latch.
use super::admission::Presentation;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    ReleaseSound(u32),
    ///4ad760("ambi_any_build_hospitalbeep.wav",0,0,0,1,100).
    CreateSound,
    Diagnostic(u32),
    ClearSeconds,
    WriteSeconds(i32),
}
pub fn advance(
    state: &mut Presentation,
    controller_exists: bool,
    paused: bool,
    mut call: impl FnMut(Call, &mut Presentation) -> Result<u32, String>,
) -> Result<(), String> {
    if state.countdown == 0 || paused {
        return Ok(());
    }
    state.countdown = state.countdown.wrapping_sub(1);
    let count = state.countdown as i32;
    if count % 50 != 0 || !controller_exists {
        return Ok(());
    }
    let seconds = count / 50;
    if seconds <= 5 {
        if state.countdown_last != seconds {
            if state.countdown_sound != 0 {
                call(Call::ReleaseSound(state.countdown_sound), state)?;
                state.countdown_sound = 0;
            }
            state.countdown_sound = call(Call::CreateSound, state)?;
        }
        state.countdown_last = seconds;
    }
    if !(0..52).contains(&(state.announcement[0] as i32)) {
        call(Call::Diagnostic(0x534c24), state)?;
    }
    if state.announcement[1] as i32 <= -1 {
        call(Call::Diagnostic(0x534bf8), state)?;
    }
    if seconds <= -1 {
        call(Call::Diagnostic(0x534bc8), state)?;
    }
    if state.announcement[2] as i32 <= 0 {
        call(Call::Diagnostic(0x534b9c), state)?;
    }
    call(Call::ClearSeconds, state)?;
    call(Call::WriteSeconds(seconds), state)?;
    Ok(())
}

#[test]
#[ignore = "requires original auction announcement capture"]
fn matches_native_announcement() {
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_AUCTION_ANNOUNCEMENT")).unwrap();
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
    let mut r = Reader(&bytes);
    assert_eq!(r.take(8), b"MTAUAN01");
    assert_eq!(r.word(), 0x037f);
    let count = r.word();
    for case in 0..count {
        let exists = r.take(1)[0] != 0;
        let paused = r.take(1)[0] != 0;
        let mut state = Presentation {
            countdown: r.word(),
            announcement: [r.word(), r.word(), r.word()],
            countdown_sound: r.word(),
            countdown_last: r.word() as i32,
            ..Default::default()
        };
        let expected = r.take(24);
        let n = r.word();
        let trace: Vec<Vec<u32>> = (0..n)
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut actual = vec![];
        advance(&mut state, exists, paused, |call, _| {
            actual.push(match call {
                Call::ReleaseSound(v) => vec![0, v],
                Call::CreateSound => vec![1],
                Call::Diagnostic(v) => vec![2, v],
                Call::ClearSeconds => vec![3],
                Call::WriteSeconds(v) => vec![4, v as u32],
            });
            Ok(0x12345)
        })
        .unwrap();
        assert_eq!(actual, trace, "case{case}");
        let actual: Vec<u8> = [
            state.countdown,
            state.announcement[0],
            state.announcement[1],
            state.announcement[2],
            state.countdown_sound,
            state.countdown_last as u32,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
        assert_eq!(actual, expected, "case{case}");
    }
    assert!(r.0.is_empty());
}
