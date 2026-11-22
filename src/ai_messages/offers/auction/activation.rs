//! Original464aa0 gameplay stores. Presentation setup is an explicit synchronous
//! dependency, including native camera/ambient audio and interface restoration.
use super::{
    Call as ActionCall,
    admission::{Controller, Presentation},
};
use crate::player_manager::{Player, put, word};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    ///464aa0..464aca:4af140 and4ad760(auction/auctionmurmer.wav); return handle.
    Ambient,
    ///464afc..464b52: selected model coordinates, camera snapshot/configuration;
    /// host retains its scene snapshot and returns original4e07a0 previous93b008 transition/control word.
    Camera,
    Action(ActionCall),
    RefreshPanel,
    ///481910(0): original speed flag541918 becomes1 locally, with4254f0 UI.
    NormalSpeed,
}
pub fn activate(
    controller: &mut Controller,
    presentation: &mut Presentation,
    players: &mut [Player],
    categories: &[[u8; 380]],
    network: bool,
    mut call: impl FnMut(Call, &mut Controller, &mut Presentation) -> Result<u32, String>,
) -> Result<(), String> {
    presentation.ambient_handle = call(Call::Ambient, controller, presentation)?;
    for p in players.iter_mut() {
        if word(&p.bytes, 0x154) == 1 {
            p.bytes[0x1df8] = 1;
        }
    }
    let transition = call(Call::Camera, controller, presentation)?;
    put(&mut controller.record, 0x154, transition);
    let category = word(&controller.record, 4) as usize;
    let seller = i32::from(
        categories
            .get(category)
            .ok_or("auction activation category outside pool")?[5] as i8,
    ) - 1;
    put(&mut controller.record, 0x50, seller as u32);
    if players.len() > 6 {
        return Err("auction activation exceeds six participants".into());
    }
    for (i, p) in players.iter().enumerate() {
        if seller == i as i32 || p.bytes[0x10] == 0 {
            controller.record[8 + i * 12] = 0;
        }
    }
    if word(&controller.record, 0x68) as i32 > 0 || seller > -1 {
        // Projection is borrowed directly; visual rotation can update presentation
        // coordinates but does not create a second authoritative auction record.
        let mut calls = vec![];
        super::rotate(
            (&mut controller.record[..128]).try_into().unwrap(),
            players.len() as i32,
            true,
            &mut |c, _| {
                calls.push(c);
                Ok(())
            },
        )?;
        for c in calls {
            call(Call::Action(c), controller, presentation)?;
        }
    }
    call(Call::RefreshPanel, controller, presentation)?;
    controller.record[0x7c] = 1;
    controller.record[0x7d] = 0;
    if !network {
        call(Call::NormalSpeed, controller, presentation)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires original auction activation capture"]
fn matches_native_activation() {
    let bytes =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AUCTION_ACTIVATION").unwrap()).unwrap();
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
    assert_eq!(r.take(8), b"MTAUAC01");
    assert_eq!(r.word(), 0x037f);
    let count = r.word();
    for case in 0..count {
        let network = r.word() != 0;
        let mode = r.word();
        let owner = r.word();
        let mut controller = Controller {
            record: r.take(344).try_into().unwrap(),
            reference: 0x10030000,
            active_count: 1,
        };
        let mut players: Vec<Player> = (0..6)
            .map(|i| Player {
                reference: 0x10010000 + i * 0x2000,
                bytes: r.take(0x1e50).to_vec(),
                auxiliaries: vec![],
            })
            .collect();
        let mut presentation = Presentation {
            ambient_handle: r.word(),
            ..Default::default()
        };
        let mut speed = r.word();
        let expected = r.take(344 + 6 * 0x1e50 + 8);
        let n = r.word();
        let trace: Vec<Vec<u32>> = (0..n)
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut actual = vec![];
        let mut categories = [[0; 380]; 52];
        categories[word(&controller.record, 4) as usize][5] = owner as u8;
        activate(
            &mut controller,
            &mut presentation,
            &mut players,
            &categories,
            network,
            |call, _, _| {
                let (value, row) = match call {
                    Call::Ambient => (0x12345, vec![0]),
                    Call::Camera => (mode, vec![1]),
                    Call::Action(ActionCall::RefreshTurn(v)) => (0, vec![2, v]),
                    Call::RefreshPanel => (0, vec![3]),
                    Call::NormalSpeed => {
                        speed = 1;
                        (0, vec![4])
                    }
                    c => panic!("unexpected{c:?}"),
                };
                actual.push(row);
                Ok(value)
            },
        )
        .unwrap();
        assert_eq!(actual, trace, "case{case}");
        let mut actual = controller.record.to_vec();
        for p in players {
            actual.extend(p.bytes);
        }
        actual.extend(presentation.ambient_handle.to_le_bytes());
        actual.extend(speed.to_le_bytes());
        assert_eq!(actual, expected, "case{case}");
    }
    assert!(r.0.is_empty());
}
