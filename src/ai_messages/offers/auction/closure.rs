//! Original464bf0 settlement and464e30 cleanup. Scene restoration is an
//! explicit presentation boundary; ownership transfer remains a delayed command.
use super::admission::{Controller, Presentation};
use crate::player_manager::{put, word};
pub struct World<'a> {
    pub controller: &'a mut Controller,
    pub categories: &'a mut [[u8; 380]],
    pub presentation: &'a mut Presentation,
    pub player_kinds: &'a mut Vec<u32>,
    pub network: [bool; 2],
    pub command_prior: [u8; 28],
    pub row_text: &'a mut [u32; 7],
    pub bid_text: &'a mut [u32; 7],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    Message([i32; 8]),
    QueueTransfer([u8; 28]),
    /// Original464cca..464dba restores camera and interface mode, removes scene
    /// models and resets participant animations. The host must implement it.
    RestorePresentation,
    CategoryView(u32),
    CategoryStatus(u32),
    CloseAnnouncement,
    AudioReset4af140,
    Tutorial(u32),
    ReleaseAmbient(u32),
}
fn local(w: &World<'_>) -> bool {
    !w.network[0] || w.network[1]
}
pub fn cleanup(
    w: &mut World<'_>,
    call: &mut impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let category = word(&w.controller.record, 4);
    w.categories
        .get_mut(category as usize)
        .ok_or("auction cleanup category outside pool")?[0x6c] = 0;
    call(Call::CategoryView(word(&w.controller.record, 4)), w)?;
    if w.controller.reference == 0 {
        return Err("auction cleanup global controller is null".into());
    }
    w.controller.record[0x7c] = 0;
    w.controller.record[0x7d] = 0;
    call(Call::CategoryStatus(word(&w.controller.record, 4)), w)?;
    w.controller.active_count = 0;
    call(Call::CloseAnnouncement, w)?;
    if local(w) {
        let mut i = 0;
        while i < w.player_kinds.len() {
            call(
                Call::Message([
                    i as i32,
                    -1,
                    0x2a,
                    word(&w.controller.record, 4) as i32,
                    0,
                    0,
                    0,
                    -999,
                ]),
                w,
            )?;
            i += 1;
        }
    }
    Ok(())
}
pub fn finish(
    w: &mut World<'_>,
    mut call: impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    if local(w) {
        let mut i = 0;
        while i < w.player_kinds.len() {
            if w.controller.record[0x70] == 0 && w.player_kinds[i] == 1 {
                call(
                    Call::Message([
                        i as i32,
                        -1,
                        0x29,
                        word(&w.controller.record, 4) as i32,
                        word(&w.controller.record, 0x60) as i32,
                        word(&w.controller.record, 0x6c) as i32,
                        0,
                        -999,
                    ]),
                    w,
                )?;
            }
            call(
                Call::Message([
                    i as i32,
                    -1,
                    0x28,
                    word(&w.controller.record, 4) as i32,
                    0,
                    0,
                    0,
                    -999,
                ]),
                w,
            )?;
            i += 1;
        }
        let mut d = w.command_prior;
        put(&mut d, 0, 0x525c60);
        put(&mut d, 4, 0x16);
        put(&mut d, 12, 0);
        d[16] = 1;
        put(&mut d, 20, word(&w.controller.record, 0x6c));
        d[24..26].copy_from_slice(&w.controller.record[4..6]);
        d[26] = if w.controller.record[0x70] == 0 {
            w.controller.record[0x60].wrapping_add(1)
        } else {
            0
        };
        call(Call::QueueTransfer(d), w)?;
    }
    w.controller.record[0x70] = 0;
    call(Call::RestorePresentation, w)?;
    cleanup(w, &mut call)?;
    call(Call::AudioReset4af140, w)?;
    *w.row_text = [0; 7];
    *w.bid_text = [0; 7];
    call(Call::Tutorial(0x40), w)?;
    if w.presentation.ambient_handle != 0 {
        call(Call::ReleaseAmbient(w.presentation.ambient_handle), w)?;
        w.presentation.ambient_handle = 0;
    }
    Ok(())
}

#[test]
#[ignore = "requires original auction settlement capture"]
fn matches_native_closure() {
    let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_CLOSURE").unwrap()).unwrap();
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
    assert_eq!(r.take(8), b"MTAUCL01");
    assert_eq!(r.word(), 0x037f);
    let count = r.word();
    for case in 0..count {
        let kind = r.word();
        let network = [r.word() != 0, r.word() != 0];
        let mutate = r.word() != 0;
        let mut kinds = (0..6).map(|_| r.word()).collect();
        let record = r.take(344).try_into().unwrap();
        let mut categories: Vec<[u8; 380]> =
            (0..52).map(|_| r.take(380).try_into().unwrap()).collect();
        let mut controller = Controller {
            record,
            reference: r.word(),
            active_count: r.word(),
        };
        let mut presentation = Presentation {
            ambient_handle: r.word(),
            ..Default::default()
        };
        let mut row_text = std::array::from_fn(|_| r.word());
        let mut bid_text = std::array::from_fn(|_| r.word());
        let expected = r.take(344 + 52 * 380 + 12 + 56);
        let n = r.word();
        let trace: Vec<Vec<u32>> = (0..n)
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut actual = vec![];
        let mut world = World {
            controller: &mut controller,
            categories: &mut categories,
            presentation: &mut presentation,
            player_kinds: &mut kinds,
            network,
            command_prior: [0xcc; 28],
            row_text: &mut row_text,
            bid_text: &mut bid_text,
        };
        let mut call = |call, w: &mut World<'_>| {
            actual.push(match call {
                Call::Message(args) => {
                    if mutate {
                        let category = (word(&w.controller.record, 4) + 1) % 52;
                        let amount = word(&w.controller.record, 0x6c) + 17;
                        put(&mut w.controller.record, 4, category);
                        put(&mut w.controller.record, 0x6c, amount);
                    }
                    let mut v = vec![0];
                    v.extend(args.map(|v| v as u32));
                    v
                }
                Call::QueueTransfer(d) => {
                    let mut v = vec![1];
                    v.extend(
                        d.chunks_exact(4)
                            .map(|b| u32::from_le_bytes(b.try_into().unwrap())),
                    );
                    v
                }
                Call::RestorePresentation => vec![2],
                Call::CategoryView(v) => vec![3, v],
                Call::CategoryStatus(v) => vec![4, v],
                Call::CloseAnnouncement => vec![5],
                Call::AudioReset4af140 => vec![6],
                Call::Tutorial(v) => vec![7, v],
                Call::ReleaseAmbient(v) => vec![8, v],
            });
            Ok(())
        };
        if kind == 0 {
            cleanup(&mut world, &mut call).unwrap();
        } else {
            finish(&mut world, call).unwrap();
        }
        assert_eq!(actual, trace, "case{case}");
        let mut actual = controller.record.to_vec();
        actual.extend(categories.iter().flatten());
        for v in [
            controller.reference,
            controller.active_count,
            presentation.ambient_handle,
        ]
        .into_iter()
        .chain(row_text)
        .chain(bid_text)
        {
            actual.extend(v.to_le_bytes());
        }
        assert_eq!(actual, expected, "case{case}");
    }
    assert!(r.0.is_empty());
}
