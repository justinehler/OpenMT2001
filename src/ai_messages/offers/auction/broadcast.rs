//! Original48c1c0 property broadcast branches22/23.
use crate::{
    player_manager::{put, word},
    player_messages::Packet,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Reply,
    Queue { descriptor: [u8; 52], size: usize },
}
pub fn dispatch(
    packet: &mut Packet,
    remaining: i32,
    prior: [u8; 52],
    mut call: impl FnMut(Call, &mut Packet) -> Result<(), String>,
) -> Result<(), String> {
    let kind = word(packet, 4);
    if !matches!(kind, 0x22 | 0x23) {
        return Err("property broadcast kind unsupported".into());
    }
    let mut descriptor = prior;
    if remaining > 1 {
        put(&mut descriptor, 0, 0x525cb8);
        put(&mut descriptor, 4, 0x1d);
        put(&mut descriptor, 12, 0);
        descriptor[16] = 1;
        put(&mut descriptor, 20, word(packet, 24));
        put(
            &mut descriptor,
            24,
            if kind == 0x22 {
                u32::MAX
            } else {
                i32::from(packet[20] as i8) as u32
            },
        );
        put(&mut descriptor, 28, word(packet, 28));
        if kind == 0x23 {
            descriptor[32..52].copy_from_slice(&packet[24..44]);
        }
        call(
            Call::Queue {
                descriptor,
                size: 52,
            },
            packet,
        )?;
    } else {
        if kind == 0x23 {
            call(Call::Reply, packet)?;
        }
        put(&mut descriptor, 0, 0x525c60);
        put(&mut descriptor, 4, 0x16);
        put(&mut descriptor, 12, 0);
        descriptor[16] = 1;
        descriptor[24..26].copy_from_slice(&packet[24..26]);
        descriptor[26] = if kind == 0x22 {
            0
        } else {
            packet[20].wrapping_add(1)
        };
        put(&mut descriptor, 20, word(packet, 28));
        call(
            Call::Queue {
                descriptor,
                size: 28,
            },
            packet,
        )?;
    }
    Ok(())
}

#[test]
#[ignore = "requires original property broadcast capture"]
fn matches_native_broadcast() {
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_BROADCAST").unwrap()).unwrap();
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
    assert_eq!(r.take(8), b"MTAUBC01");
    assert_eq!(r.word(), 0x037f);
    let count = r.word();
    for case in 0..count {
        let remaining = r.word() as i32;
        let mutate = r.word() != 0;
        let mut packet = r.take(48).try_into().unwrap();
        let expected = r.take(48);
        let n = r.word();
        let trace: Vec<Vec<u32>> = (0..n)
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut actual = vec![];
        dispatch(&mut packet, remaining, [0xcc; 52], |call, packet| {
            actual.push(match call {
                Call::Reply => {
                    if mutate {
                        put(packet, 24, 47);
                        put(packet, 28, 1300);
                        packet[20] = 3;
                    }
                    vec![0]
                }
                Call::Queue { descriptor, size } => {
                    let mut v = vec![1, size as u32];
                    v.extend(
                        descriptor[..size]
                            .chunks_exact(4)
                            .map(|b| u32::from_le_bytes(b.try_into().unwrap())),
                    );
                    v
                }
            });
            Ok(())
        })
        .unwrap();
        assert_eq!(actual, trace, "case{case}");
        assert_eq!(packet, expected, "case{case}");
    }
    assert!(r.0.is_empty());
}
