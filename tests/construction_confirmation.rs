//! Synthetic planner admission with the real quote arithmetic and local queue.
use openmt2001::{building_binding, construction, local_commands, pe::PeImage};

#[path = "support/building_image.rs"]
mod building_image;

fn command() -> construction::Command {
    construction::Command {
        category: 3,
        rectangle: [0, 0, 1, 0],
        height: 1,
        kind: 0,
        business: 0,
        variation: 0,
        quality: 0,
        owner: 1,
        components: [0.; 4],
    }
}

fn reprice(command: &mut construction::Command, hour: i32) -> f32 {
    let bytes = building_image::fixture();
    let image = PeImage::parse(&bytes).unwrap();
    let mut business = [0; 0x74];
    business[0x4c] = 1;
    business[0x58..0x5c].copy_from_slice(&1f32.to_le_bytes());
    for offset in [0x60, 0x64, 0x68, 0x6c] {
        business[offset..offset + 4].copy_from_slice(&1f32.to_le_bytes());
    }
    let cost = building_binding::construction_cost(
        &building_binding::ConstructionInputs {
            volume: 2,
            kind: 0,
            quality: 0,
            category_value: 0.,
            hour,
            discount: 0.,
            owner: 1,
            utility_owners: [0; 4],
            business: &business,
            products: &[building_binding::StockProduct {
                value: 3.,
                fraction: 1.,
            }],
        },
        &image,
    )
    .unwrap();
    command.components = cost.components;
    cost.total as f32
}

#[test]
fn increased_live_quote_is_queued_after_displayed_price_admission() {
    let mut command = command();
    let displayed = reprice(&mut command, 5);
    assert_eq!(displayed, 20.);
    let cash = -9_970.;
    let (total, admitted) = construction::confirmation_quote(cash, 10_000, Some(displayed), || {
        Ok(reprice(&mut command, 6))
    })
    .unwrap();
    assert!(admitted);
    assert_eq!(total, 54.);
    assert!(!construction::affordable(cash, 10_000, total, false));
    assert_eq!(command.components, [20., 6., 20., 8.]);
    let mut categories = [[0; 380]; 52];
    let mut players: Vec<openmt2001::player_manager::Player> = Vec::new();
    let mut arena = openmt2001::player_messages::Arena { next: 0x20000000 };
    assert!(
        construction::admit(
            &command,
            &mut categories,
            &mut players,
            &mut arena,
            0,
            openmt2001::player_messages::Routing {
                current_player: 1,
                network: [false; 2],
                prior: [0; 48],
            },
            |_, _| panic!("accepted lot emitted a rejection"),
        )
        .unwrap()
    );
    assert_eq!(&categories[3][0x9c..0xa4], &[1, 0, 0, 0, 0, 0, 0, 1]);
    let mut queue = local_commands::Queue::default();
    queue
        .submit_with_delay(command, 100, true, 0, |_| panic!("premature execution"))
        .unwrap();
    let snapshot = queue.snapshot().unwrap();
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].1, 125);
    assert_eq!(snapshot[0].4.components, [20., 6., 20., 8.]);
    queue
        .drain(&mut [124, 124], |_| panic!("premature execution"))
        .unwrap();
    assert_eq!(cash, -9_970., "admission does not debit cash");
}

#[test]
fn decreased_live_quote_cannot_replace_failed_displayed_price_gate() {
    let mut command = command();
    let displayed = reprice(&mut command, 6);
    let cash = -9_970.;
    assert!(construction::affordable(cash, 10_000, 20., false));
    let before = command.components;
    let mut queue = local_commands::Queue::default();
    let (total, admitted) = construction::confirmation_quote(cash, 10_000, Some(displayed), || {
        panic!("rejected confirmation must not reprice")
    })
    .unwrap();
    if admitted {
        queue
            .submit_with_delay(command.clone(), 100, true, 0, |_| Ok(()))
            .unwrap();
    }
    assert!(!admitted);
    assert_eq!(total, 54.);
    assert!(queue.snapshot().unwrap().is_empty());
    assert_eq!(command.components, before);
    assert_eq!(cash, -9_970.);
}

#[test]
fn undisplayed_producers_keep_the_current_quote_gate() {
    for (hour, expected) in [(5, true), (6, false)] {
        let mut command = command();
        let (total, admitted) = construction::confirmation_quote(-9_970., 10_000, None, || {
            Ok(reprice(&mut command, hour))
        })
        .unwrap();
        assert_eq!(admitted, expected);
        assert_eq!(total, if hour == 5 { 20. } else { 54. });
    }
    assert_eq!(
        construction::confirmation_quote(-10_000., 10_000, Some(0.), || Ok(54.)).unwrap(),
        (54., true),
        "a displayed zero is a valid retained quote"
    );
    assert!(
        !construction::confirmation_quote(100., 10_000, Some(f32::NAN), || {
            panic!("unordered retained price must fail admission")
        })
        .unwrap()
        .1
    );
}
