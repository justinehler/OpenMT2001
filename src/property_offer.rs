//! Original404d90 property offer controls and405120/4052d0/405580 actions.
use crate::{
    pe::PeImage,
    player_manager::{Player, word},
};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub visible: u8,
    pub amount: f32,
    pub step: f32,
    pub minimum: f32,
    pub maximum: f32,
    pub amount_visible: u8,
    pub action_visible: u8,
    pub commit_selected: u8,
    pub commit_hover: u8,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            visible: 0,
            amount: 1000.,
            step: 100.,
            minimum: 1000.,
            maximum: 100000.,
            amount_visible: 0,
            action_visible: 0,
            commit_selected: 0,
            commit_hover: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// Widget methods retain their argument order; their target is presentation.
    Ui {
        address: u32,
        arguments: [u32; 3],
        count: u8,
    },
    Producer {
        address: u32,
        arguments: [u32; 3],
    },
    RefreshProperty,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Confirm,
    Buy,
    Sell,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dispatch {
    Queue {
        descriptor: [u8; 52],
        size: usize,
    },
    External {
        route: crate::player_messages::Route,
        packet: crate::player_messages::Packet,
    },
}
///48cab0 routing followed synchronously by48c1c0 broadcast handling. The
/// callback queues real local commands and handles external presentation only.
pub fn route(
    args: [i32; 8],
    remaining: u32,
    players: &mut impl crate::player_messages::Players,
    context: &mut crate::player_messages::Context,
    mut call: impl FnMut(Dispatch) -> Result<(), String>,
) -> Result<bool, String> {
    use crate::{
        ai_messages::offers::auction::broadcast,
        player_messages::{self, Route, Routing},
    };
    let routing = Routing {
        current_player: context.current_player,
        network: context.network,
        prior: context.prior,
    };
    let mut external = None;
    let admitted = player_messages::produce_in(
        args,
        remaining,
        players,
        &mut context.arena,
        routing,
        |route, packet| {
            external = Some((route, *packet));
            Ok(())
        },
    )?;
    if let Some((route, mut packet)) = external {
        if route == Route::Broadcast && matches!(word(&packet, 4), 0x22 | 0x23) {
            broadcast::dispatch(
                &mut packet,
                remaining as i32,
                [0; 52],
                |event, packet| match event {
                    broadcast::Call::Queue { descriptor, size } => {
                        call(Dispatch::Queue { descriptor, size })
                    }
                    broadcast::Call::Reply => player_messages::reply_in(
                        packet,
                        0x24,
                        [0x5f5e0ff; 4],
                        players,
                        &mut context.arena,
                        routing,
                        |route, packet| {
                            call(Dispatch::External {
                                route,
                                packet: *packet,
                            })
                        },
                    )
                    .map(|_| ()),
                },
            )?;
        } else {
            call(Dispatch::External { route, packet })?;
        }
    }
    Ok(admitted)
}
pub struct World<'a> {
    pub categories: &'a mut [[u8; 380]; 52],
    pub current: &'a mut Player,
    pub selected: Option<u32>,
    pub allowance: i32,
    pub affordability_override: bool,
}
/// Original4021f8..402214 initializes the amount only when the selected
/// property changes. Report/offer toggles retain the numeric widget state.
pub fn selected_amount(category: &[u8; 380], image: &PeImage<'_>) -> Result<f32, String> {
    Ok(
        crate::ai_messages::offers::execution::category_value(category, image)?.ftol_low_word()
            as f32,
    )
}
/// Original4013c5..40157d display phase updates controls after404d90.
/// The active bidding panel hides both children. An owned property's amount
/// is computed rather than edited; native rounds through FST32 before FTOL.
pub fn display(
    panel: &mut Panel,
    category: &[u8; 380],
    current_owner_index: u8,
    auction_visible: bool,
    image: &PeImage<'_>,
) -> Result<(), String> {
    let value = crate::ai_messages::offers::execution::category_value(category, image)?;
    panel.step = 100.;
    panel.minimum = value.to_f32();
    panel.maximum = 100000.;
    if panel.visible != 1 || auction_visible {
        panel.amount_visible = 0;
        panel.action_visible = 0;
    } else if i32::from(category[5] as i8) - 1 == i32::from(current_owner_index) {
        let mut sale = value;
        sale.mul_f32(image.f32_at(0x52502c)?)?;
        panel.amount = crate::x87::Sum::from_f32(sale.to_f32())?.ftol_low_word() as f32;
        panel.action_visible = 1;
    } else {
        panel.amount_visible = 1;
        panel.action_visible = 1;
    }
    Ok(())
}

#[test]
#[ignore = "pinned original property display/selection fixture"]
fn native_property_display_and_selection() {
    fn take<const N: usize>(bytes: &mut &[u8]) -> [u8; N] {
        let (head, tail) = bytes.split_at(N);
        *bytes = tail;
        head.try_into().unwrap()
    }
    fn word(bytes: &mut &[u8]) -> u32 {
        u32::from_le_bytes(take(bytes))
    }
    fn panel(bytes: &mut &[u8]) -> Panel {
        Panel {
            visible: word(bytes) as u8,
            amount: f32::from_bits(word(bytes)),
            step: f32::from_bits(word(bytes)),
            minimum: f32::from_bits(word(bytes)),
            maximum: f32::from_bits(word(bytes)),
            amount_visible: word(bytes) as u8,
            action_visible: word(bytes) as u8,
            commit_selected: word(bytes) as u8,
            commit_hover: word(bytes) as u8,
        }
    }
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let fixture = std::fs::read(crate::fixtures::path("NATIVE_PROPERTY_DISPLAY")).unwrap();
    let mut bytes = fixture.as_slice();
    assert_eq!(&take::<8>(&mut bytes), b"MTPDIS01");
    assert_eq!(word(&mut bytes), 0x37f);
    for case in 0..word(&mut bytes) {
        let _category = word(&mut bytes);
        let slot = word(&mut bytes) as u8;
        let auction_visible = word(&mut bytes) != 0;
        let selected = word(&mut bytes);
        let row = take::<380>(&mut bytes);
        let mut actual = panel(&mut bytes);
        let expected = panel(&mut bytes);
        display(&mut actual, &row, slot, auction_visible, &image).unwrap();
        assert_eq!(actual, expected, "display case {case}");
        assert_eq!(
            selected_amount(&row, &image).unwrap().to_bits(),
            selected,
            "selection case {case}"
        );
    }
    assert!(bytes.is_empty());
}
fn ui(address: u32, args: &[u32]) -> Call {
    let mut arguments = [0; 3];
    arguments[..args.len()].copy_from_slice(args);
    Call::Ui {
        address,
        arguments,
        count: args.len() as u8,
    }
}
pub fn refresh(
    panel: &mut Panel,
    world: &mut World<'_>,
    image: &PeImage<'_>,
    mut call: impl FnMut(Call, &mut Panel, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let Some(category) = world.selected else {
        return Ok(());
    };
    let row = world
        .categories
        .get(category as usize)
        .ok_or("property controls category outside pool")?;
    if row[0x6c] != 0 {
        panel.visible = 0;
        return Ok(());
    }
    if i32::from(row[5] as i8) - 1 == i32::from(world.current.bytes[0x1bc]) {
        call(ui(0x442960, &[0x30]), panel, world)?;
        panel.visible = 1;
        call(ui(0x441940, &[]), panel, world)?;
        call(ui(0x441220, &[0x39]), panel, world)?;
        return Ok(());
    }
    let value = crate::ai_messages::offers::execution::category_value(row, image)?;
    panel.step = 100.;
    panel.minimum = value.to_f32();
    panel.maximum = 100000.;
    call(
        ui(
            0x42efb0,
            &[
                panel.step.to_bits(),
                panel.minimum.to_bits(),
                panel.maximum.to_bits(),
            ],
        ),
        panel,
        world,
    )?;
    let row = world
        .categories
        .get(category as usize)
        .ok_or("property controls category outside pool")?;
    let value = crate::ai_messages::offers::execution::category_value(row, image)?;
    if !panel.amount.is_nan() && value.to_f64() > f64::from(panel.amount) {
        panel.amount = crate::ai_messages::offers::execution::category_value(row, image)?.to_f32();
        call(ui(0x42ee70, &[panel.amount.to_bits()]), panel, world)?;
    }
    call(ui(0x442960, &[0x2f]), panel, world)?;
    if let Some(category) = world.selected
        && world
            .categories
            .get(category as usize)
            .ok_or("property controls category outside pool")?[0x74]
            != 0
    {
        panel.visible = 1;
    }
    Ok(())
}
pub fn submit(
    action: Action,
    panel: &mut Panel,
    world: &mut World<'_>,
    image: &PeImage<'_>,
    mut call: impl FnMut(Call, &mut Panel, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    if action == Action::Confirm {
        call(ui(0x442af0, &[0]), panel, world)?;
        panel.commit_selected = 1;
        panel.commit_hover = 0;
    }
    let Some(category) = world.selected else {
        return Ok(());
    };
    let own = if action == Action::Confirm {
        let row = world
            .categories
            .get(category as usize)
            .ok_or("property action category outside pool")?;
        i32::from(row[5] as i8) - 1 == i32::from(world.current.bytes[0x1bc])
    } else {
        action == Action::Sell
    };
    if (category as i32) < 0 {
        return Ok(());
    }
    if own {
        call(
            Call::Producer {
                address: 0x48a690,
                arguments: [category, panel.amount.to_bits(), u32::MAX],
            },
            panel,
            world,
        )?;
        panel.amount_visible = 0;
        panel.action_visible = 0;
        let category = world
            .selected
            .ok_or("property selection disappeared after sale producer")?;
        world
            .categories
            .get_mut(category as usize)
            .ok_or("property action category outside pool")?[0x6c] = 1;
    } else {
        let cash = f32::from_bits(word(&world.current.bytes, 4));
        if !crate::construction::affordable(
            cash,
            world.allowance,
            panel.amount,
            world.affordability_override,
        ) {
            for (address, args) in crate::construction::INSUFFICIENT_CASH_CALLS {
                call(ui(address, args), panel, world)?;
            }
            return Ok(());
        }
        let category = world
            .selected
            .ok_or("property selection disappeared after cash check")?;
        world
            .categories
            .get_mut(category as usize)
            .ok_or("property action category outside pool")?[0x6c] = 1;
        call(Call::RefreshProperty, panel, world)?;
        refresh(panel, world, image, &mut call)?;
        let category = world
            .selected
            .ok_or("property selection disappeared after refresh")?;
        call(
            Call::Producer {
                address: 0x48a640,
                arguments: [category, panel.amount.to_bits(), u32::MAX],
            },
            panel,
            world,
        )?;
        return Ok(());
    }
    call(Call::RefreshProperty, panel, world)?;
    refresh(panel, world, image, &mut call)
}

#[test]
#[ignore = "requires original property offer panel/action observations"]
fn matches_native_property_offer() {
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
    fn panel(w: [u32; 9]) -> Panel {
        Panel {
            visible: w[0] as u8,
            amount: f32::from_bits(w[1]),
            step: f32::from_bits(w[2]),
            minimum: f32::from_bits(w[3]),
            maximum: f32::from_bits(w[4]),
            amount_visible: w[5] as u8,
            action_visible: w[6] as u8,
            commit_selected: w[7] as u8,
            commit_hover: w[8] as u8,
        }
    }
    fn words(p: Panel) -> [u32; 9] {
        [
            u32::from(p.visible),
            p.amount.to_bits(),
            p.step.to_bits(),
            p.minimum.to_bits(),
            p.maximum.to_bits(),
            u32::from(p.amount_visible),
            u32::from(p.action_visible),
            u32::from(p.commit_selected),
            u32::from(p.commit_hover),
        ]
    }
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let bytes = std::fs::read(crate::fixtures::path("NATIVE_PROPERTY_OFFER")).unwrap();
    let mut r = R(&bytes);
    assert_eq!(r.take(8), b"MTPROP01");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 6] = std::array::from_fn(|_| r.word());
        let mut categories = Box::new([[0; 380]; 52]);
        categories[h[1] as usize] = r.take(380).try_into().unwrap();
        let category_after = r.take(380);
        let mut player = Player {
            reference: 0x10020000,
            bytes: r.take(548).to_vec(),
            auxiliaries: vec![],
        };
        let player_after = r.take(548);
        let mut p = panel(std::array::from_fn(|_| r.word()));
        let after: [u32; 9] = std::array::from_fn(|_| r.word());
        let expected: Vec<Vec<u32>> = (0..h[5])
            .map(|_| {
                let n = r.word();
                (0..n).map(|_| r.word()).collect()
            })
            .collect();
        let mut world = World {
            categories: &mut categories,
            current: &mut player,
            selected: (h[2] == 0).then_some(h[1]),
            allowance: h[3] as i32,
            affordability_override: h[4] != 0,
        };
        let mut trace = vec![];
        let mut call = |call, _: &mut Panel, world: &mut World<'_>| {
            match call {
                Call::Ui {
                    address,
                    arguments,
                    count,
                } => {
                    let mut v = vec![0, address];
                    v.extend(&arguments[..count as usize]);
                    trace.push(v);
                }
                Call::Producer { address, arguments } => {
                    let mut v = vec![1, address];
                    v.extend(arguments);
                    trace.push(v);
                    let args = crate::ai_messages::offers::producers::property(
                        address,
                        world.current.bytes[0x1bc],
                        arguments[0],
                        arguments[1],
                        arguments[2],
                        world.categories,
                    )?;
                    let mut v = vec![3];
                    v.extend(args.map(|v| v as u32));
                    trace.push(v);
                }
                Call::RefreshProperty => trace.push(vec![2]),
            }
            Ok(())
        };
        if h[0] == 0x404d90 {
            refresh(&mut p, &mut world, &image, &mut call).unwrap();
        } else {
            submit(
                match h[0] {
                    0x405120 => Action::Confirm,
                    0x4052d0 => Action::Buy,
                    0x405580 => Action::Sell,
                    _ => panic!(),
                },
                &mut p,
                &mut world,
                &image,
                &mut call,
            )
            .unwrap();
        }
        assert_eq!(trace, expected, "case{case} trace");
        assert_eq!(words(p), after, "case{case} panel");
        assert_eq!(
            categories[h[1] as usize], category_after,
            "case{case} category"
        );
        assert_eq!(player.bytes, player_after, "case{case} player");
    }
    assert!(r.0.is_empty());
}
