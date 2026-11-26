//! Original48a8f0 ownership writes -> real47fbb0 metric ->4896e0/489df0.
//! Synthetic component state, with original PE constants read as data. This
//! neither invents an auction success nor exercises result UI/profile award.
use openmt2001::{
    category_ownership,
    objective_tick::{self, Controls, Event},
    pe::PeImage,
    player_manager::{Manager, Player},
    player_metrics,
    scenario_objective::State,
    x87::Sum,
};

struct Fixture {
    manager: Manager,
    categories: [[u8; 380]; 52],
}

fn put_float(bytes: &mut [u8], at: usize, value: f32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn float(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn original_bytes() -> Vec<u8> {
    std::fs::read(std::env::var_os("OPENMT2001_EXE").expect("original v1.4 PE data path")).unwrap()
}

fn check_literals(image: &PeImage<'_>) {
    // Source-backed arithmetic below uses these original binary32 constants.
    assert_eq!(image.at(0x5385a4, 1).unwrap(), [1]);
    for (address, bits) in [
        (0x52502c, 0.5f32.to_bits()),
        (0x525034, 1f32.to_bits()),
        (0x525b20, 375f32.to_bits()),
        (0x525b1c, 0x3ca3d70a),
        (0x525248, 50f32.to_bits()),
    ] {
        assert_eq!(image.f32_at(address).unwrap().to_bits(), bits);
    }
}

impl Fixture {
    fn new(opponent_cash: f32) -> Self {
        let mut manager = Manager::initialize([0; 40]);
        manager.bytes[12..16].copy_from_slice(&2u32.to_le_bytes());
        for (slot, cash) in [5000., opponent_cash].into_iter().enumerate() {
            let mut bytes = vec![0; 0x224];
            put_float(&mut bytes, 4, cash);
            bytes[0x10] = 1;
            bytes[0x1bc] = slot as u8;
            manager.players.push(Player {
                reference: 0x10010000 + slot as u32 * 0x400,
                bytes,
                auxiliaries: vec![],
            });
        }
        let mut categories = [[0; 380]; 52];
        put_float(&mut categories[0], 0x84, 10.);
        Self {
            manager,
            categories,
        }
    }

    fn acquire(&mut self) {
        let chains = std::array::from_fn(|_| Vec::new());
        let mut world = category_ownership::World {
            categories: &mut self.categories,
            buildings: &mut [],
            businesses: &mut [],
            players: &mut self.manager.players,
            chains: &chains,
            normal_tick: 100,
            category_tick: 15000,
            refresh_counter: 0,
            accounting_enabled: true,
            current_player: None, // No presentation is part of this component.
            notification_threshold: 0.,
        };
        world
            .apply(0, 0, 1000., &mut |_, seam| {
                panic!("empty-property human acquisition reached unrelated seam {seam:?}")
            })
            .unwrap();
        assert_eq!(world.refresh_counter, 100);
        assert_eq!(world.categories[0][5], 1);
        assert_eq!(&world.categories[0][0x78..0x7c], &[0; 4]);
        assert_eq!(&world.categories[0][0x7c..0x80], &15000u32.to_le_bytes());
        assert_eq!(world.players[0].bytes[0x178], 1);
        assert_eq!(float(&world.players[0].bytes, 4), 4000.);
        assert_eq!(float(&world.players[0].bytes, 0x14), -1000.);
        assert_eq!(float(&world.players[1].bytes, 0x14), 0.);
    }

    fn metric(&self, slot: usize, image: &PeImage<'_>) -> Sum {
        metric(&self.categories, &self.manager.players[slot], slot, image)
    }

    fn evaluate(
        &mut self,
        state: &mut State,
        year: u32,
        day: u32,
        image: &PeImage<'_>,
    ) -> Vec<Event> {
        let mut controls = Controls {
            network_enabled: false,
            network_override: false,
            mode: 2,
            year,
            day,
        };
        let mut events = vec![];
        objective_tick::advance(
            state,
            &mut self.manager,
            &mut controls,
            &mut |event, _, manager, _| {
                events.push(event);
                match event {
                    Event::Metric {
                        address: 0x47fbb0,
                        player,
                    } => Ok(metric(
                        &self.categories,
                        &manager.players[player],
                        player,
                        image,
                    )),
                    Event::Metric { .. } => panic!("wrong objective metric"),
                    _ => Ok(Sum::from_i32(0)),
                }
            },
        )
        .unwrap();
        events
    }
}

fn metric(categories: &[[u8; 380]; 52], player: &Player, slot: usize, image: &PeImage<'_>) -> Sum {
    let categories: Vec<_> = categories
        .iter()
        .map(|row| (row[5], float(row, 0x84)))
        .collect();
    // Empty chains contribute no business capital; property ownership and cash
    // still flow through the actual metric, never a configured metric return.
    player_metrics::empire_value(
        slot,
        player.bytes[4..444].try_into().unwrap(),
        &[],
        &[],
        &categories,
        image,
    )
    .unwrap()
}

fn threshold(target: f32) -> State {
    State {
        kind: 5,
        target,
        flag: 0,
        ..State::default()
    }
}

#[test]
#[ignore = "requires original v1.4 PE as read-only data"]
fn acquired_property_reaches_real_empire_threshold_despite_lower_cash() {
    let bytes = original_bytes();
    let image = PeImage::parse(&bytes).unwrap();
    check_literals(&image);
    let mut fixture = Fixture::new(6000.);
    assert_eq!(
        fixture.metric(0, &image).compare(Sum::from_i32(5000)),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        fixture.metric(1, &image).compare(Sum::from_i32(6000)),
        std::cmp::Ordering::Equal
    );
    let mut before = threshold(6250.);
    assert_eq!(
        fixture.evaluate(&mut before, 0, 0, &image),
        [
            Event::Metric {
                address: 0x47fbb0,
                player: 0
            },
            Event::Metric {
                address: 0x47fbb0,
                player: 1
            },
        ]
    );
    assert_eq!(before.reached, [0; 6]);
    fixture.acquire();
    // Native4477e0: FTOL((10*.5+1)*375*f32(0.02)+.5)*50 =2250.
    // Actual player cash4000 + property2250 = literal6250; not a cash stub.
    assert_eq!(
        fixture.metric(0, &image).compare(Sum::from_i32(6250)),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        fixture.metric(1, &image).compare(Sum::from_i32(6000)),
        std::cmp::Ordering::Equal
    );
    let mut above = threshold(6250.5);
    let events = fixture.evaluate(&mut above, 0, 0, &image);
    assert!(events.iter().all(|e| matches!(e, Event::Metric { .. })));
    assert_eq!(above.reached, [0; 6]);
    let mut exact = threshold(6250.);
    assert_eq!(
        fixture.evaluate(&mut exact, 0, 0, &image),
        [
            Event::Metric {
                address: 0x47fbb0,
                player: 0
            },
            Event::Metric {
                address: 0x47fbb0,
                player: 1
            },
            Event::Outcome {
                winner: 0,
                reason: 2
            },
            Event::RemovalCommand(1),
            Event::RemovePlayer(1),
        ]
    );
    assert_eq!(exact.reached, [1, 0, 0, 0, 0, 0]);
    assert_eq!(exact.excess, [0; 6]);
}

#[test]
#[ignore = "requires original v1.4 PE as read-only data"]
fn acquired_property_and_different_cash_composition_produce_native_date_tie() {
    let bytes = original_bytes();
    let image = PeImage::parse(&bytes).unwrap();
    check_literals(&image);
    let mut fixture = Fixture::new(6250.);
    fixture.acquire();
    assert_ne!(
        float(&fixture.manager.players[0].bytes, 4),
        float(&fixture.manager.players[1].bytes, 4)
    );
    for slot in 0..2 {
        assert_eq!(
            fixture.metric(slot, &image).compare(Sum::from_i32(6250)),
            std::cmp::Ordering::Equal
        );
    }
    let mut state = State {
        kind: 4,
        target: 1940.,
        flag: 0,
        ..State::default()
    };
    assert!(fixture.evaluate(&mut state, 0, 9, &image).is_empty());
    assert_eq!(state.reached, [0; 6]);
    let events = fixture.evaluate(&mut state, 1, 0, &image);
    let outcomes: Vec<_> = events
        .into_iter()
        .filter(|e| !matches!(e, Event::Metric { .. }))
        .collect();
    assert_eq!(
        outcomes,
        [Event::Outcome {
            winner: -1,
            reason: 2
        }]
    );
    assert_eq!(state.reached, [1, 1, 0, 0, 0, 0]);
    assert_eq!(state.excess, [0; 6]);
}

#[test]
#[ignore = "requires original v1.4 PE as read-only data"]
fn unspilled_final_cash_addition_does_not_round_into_threshold() {
    let bytes = original_bytes();
    let image = PeImage::parse(&bytes).unwrap();
    check_literals(&image);
    let mut fixture = Fixture::new(6000.);
    fixture.acquire();
    // Deliberately controlled metric-prefix input after acquisition; this is
    // not claimed to result from subtracting1000 from a5000f32 cash value.
    put_float(&mut fixture.manager.players[0].bytes, 4, 4000.000732421875);
    let expected = Sum::from_f64(6250.000732421875).unwrap();
    let actual = fixture.metric(0, &image);
    assert_eq!(actual.compare(expected), std::cmp::Ordering::Equal);
    let mut state = threshold(6250.0009765625);
    // Independent negative control: premature f32 narrowing would equal target.
    assert_eq!(actual.to_f32().to_bits(), state.target.to_bits());
    assert!(
        fixture
            .evaluate(&mut state, 0, 0, &image)
            .iter()
            .all(|e| matches!(e, Event::Metric { .. }))
    );
    assert_eq!(state.reached, [0; 6]);
}
