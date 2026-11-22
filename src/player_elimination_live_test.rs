use crate::{
    ai_messages::offers::auction::{admission, closure},
    economy,
    player_elimination::*,
    player_manager::{Manager, Player},
};
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
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
#[test]
#[ignore = "requires whole original elimination/accounting/auction-cleanup observations"]
fn matches_native_live_elimination() {
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_ELIMINATION_LIVE").unwrap()).unwrap();
    let mut r = R(&data);
    assert_eq!(r.take(8), b"MTELIV01");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 8] = std::array::from_fn(|_| r.word());
        let state: [u32; 8] = std::array::from_fn(|_| r.word());
        let lengths = [
            40,
            544,
            544,
            544,
            544,
            544,
            544,
            52 * 380,
            4 * 544,
            8 * 916,
            344,
            60,
        ];
        let mut before = vec![];
        let mut after = vec![];
        for n in lengths {
            before.push(r.take(n));
            after.push(r.take(n));
        }
        let expected: Vec<Vec<u32>> = (0..h[7])
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut manager = Manager {
            bytes: before[0].try_into().unwrap(),
            players: (0..6)
                .map(|i| Player {
                    reference: 0x10010000 + i as u32 * 0x400,
                    bytes: before[i + 1].to_vec(),
                    auxiliaries: vec![],
                })
                .collect(),
        };
        let mut categories: Vec<[u8; 380]> = before[7]
            .chunks_exact(380)
            .map(|b| b.try_into().unwrap())
            .collect();
        let mut businesses = vec![[0; 544]; 600];
        for (i, b) in before[8].chunks_exact(544).enumerate() {
            businesses[i] = b.try_into().unwrap();
        }
        let mut buildings = vec![[0; 916]; 600];
        for (i, b) in before[9].chunks_exact(916).enumerate() {
            buildings[i] = b.try_into().unwrap();
        }
        let mut controller = admission::Controller {
            record: before[10].try_into().unwrap(),
            reference: state[0],
            active_count: state[1],
        };
        let mut tasks: [[u8; 20]; 3] =
            std::array::from_fn(|i| before[11][i * 20..i * 20 + 20].try_into().unwrap());
        let mut controls = Controls {
            network_enabled: h[3] != 0,
            current_player: manager.players[h[1] as usize].reference,
            auction: (state[0] != 0).then_some(Auction {
                reference: state[0],
                category: word(&controller.record, 4) as i32,
                player: word(&controller.record, 0x5c),
            }),
            refresh: [state[2] as u8, state[3] as u8],
        };
        let mut trace = vec![];
        advance(
            h[0] as usize,
            &mut manager,
            Pools {
                categories: &mut categories,
                businesses: &mut businesses,
                buildings: &mut buildings,
            },
            &mut controls,
            |call, manager, pools, _| {
                match call {
                    Call::BusinessOwner { business } => {
                        let building = i16::from_le_bytes(
                            pools.businesses[business][8..10].try_into().unwrap(),
                        ) as usize;
                        let prefix = pools.buildings[building][..64].try_into().unwrap();
                        let mut accounts = manager
                            .players
                            .iter()
                            .map(|p| economy::PlayerAccount::from_runtime(&p.bytes))
                            .collect::<Result<Vec<_>, _>>()?;
                        economy::transfer_business_owner(
                            &mut pools.businesses[business],
                            &prefix,
                            0,
                            h[2] != 0,
                            &mut accounts,
                        )?;
                        for (p, a) in manager.players.iter_mut().zip(accounts) {
                            a.write_runtime(&mut p.bytes)?;
                        }
                    }
                    Call::BuildingOwner { building } | Call::BuildingRefresh { building } => {
                        let row = &mut pools.buildings[building];
                        if matches!(call, Call::BuildingOwner { .. }) {
                            row[0x2d] = 0;
                            row[0x30..0x34].copy_from_slice(&h[6].to_le_bytes());
                        }
                        if word(row, 0x4c) == 1
                            && (word(row, 0x1c) == u32::MAX || matches!(word(row, 0x24), 0 | 1))
                        {
                            trace.push(vec![0x485520, 0x591318 + building as u32 * 916]);
                        }
                    }
                    Call::Auction => {
                        let mut presentation = admission::Presentation::default();
                        let mut kinds = manager
                            .players
                            .iter()
                            .map(|p| word(&p.bytes, 0x154))
                            .collect();
                        let mut world = closure::World {
                            controller: &mut controller,
                            categories: pools.categories,
                            presentation: &mut presentation,
                            player_kinds: &mut kinds,
                            network: [h[3] != 0, h[4] != 0],
                            command_prior: [0; 28],
                            row_text: &mut [0; 7],
                            bid_text: &mut [0; 7],
                        };
                        closure::cleanup(&mut world, &mut |call, _| {
                            trace.push(match call {
                                closure::Call::Message(args) => {
                                    let mut row = vec![0x48cab0];
                                    row.extend(args.map(|x| x as u32));
                                    row
                                }
                                closure::Call::CategoryView(c) => vec![0x405260, c],
                                closure::Call::CategoryStatus(c) => vec![0x41ed10, c],
                                closure::Call::CloseAnnouncement => vec![0x43bed0, 0x10022000],
                                other => return Err(format!("unexpected cleanup{other:?}")),
                            });
                            Ok(())
                        })?;
                    }
                    Call::PlayerUi { object } => {
                        trace.push(vec![0x43bed0, 0x10020000 + object as u32 * 0x1000])
                    }
                    Call::NetworkFinalize => trace.push(vec![0x4e1db0, 0]),
                    Call::FinishLocal => {
                        finish_local((h[5] != u32::MAX).then_some(h[5] as usize), &mut tasks)?
                    }
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(trace, expected, "case{case} calls");
        assert_eq!(
            [
                controller.reference,
                controller.active_count,
                u32::from(controls.refresh[0]),
                u32::from(controls.refresh[1])
            ],
            state[4..],
            "case{case} globals"
        );
        assert_eq!(manager.bytes.as_slice(), after[0], "case{case} manager");
        for (i, p) in manager.players.iter().enumerate() {
            assert_eq!(p.bytes, after[i + 1], "case{case} player{i}");
        }
        assert_eq!(categories.as_flattened(), after[7], "case{case} categories");
        assert_eq!(
            businesses[..4].as_flattened(),
            after[8],
            "case{case} businesses"
        );
        assert_eq!(
            buildings[..8].as_flattened(),
            after[9],
            "case{case} buildings"
        );
        assert_eq!(
            controller.record.as_slice(),
            after[10],
            "case{case} controller"
        );
        assert_eq!(tasks.as_flattened(), after[11], "case{case} timers");
    }
    assert!(r.0.is_empty());
}
