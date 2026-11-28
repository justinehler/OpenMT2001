//! Original48ae60 command13 and451366 sale-to-city consumer.
use crate::{
    pe::PeImage,
    player_manager::{put, word},
};
#[derive(Clone, Debug)]
pub struct Command {
    pub business: u32,
    pub player: u8,
}
impl Command {
    pub fn write_packet(&self, row: &mut [u8; 28]) {
        for (at, v) in [(0, 0x525df4), (4, 0x13), (12, 0), (20, self.business)] {
            put(row, at, v);
        }
        row[16] = 1;
        row[24] = self.player;
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    BuildingAppearance(usize),
    RefreshInterface,
    BuildingPanel(usize),
    AccountPosted(crate::economy::AccountPosting),
}
pub struct World<'a> {
    pub businesses: &'a mut [[u8; 544]],
    pub buildings: &'a mut [[u8; 916]],
    pub players: &'a mut [crate::economy::PlayerAccount],
    pub catalog: &'a crate::business_pool::BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub age: &'a mut u8,
}
pub struct Controls {
    /// Account index corresponding to original86cfc0, when resolved.
    pub current_player: Option<usize>,
    pub accounting: bool,
    pub ownership_tick: u32,
    pub timestamp: u32,
    pub hour: i32,
}
pub fn apply(
    command: &Command,
    w: &mut World<'_>,
    controls: &Controls,
    image: &PeImage<'_>,
    mut callback: impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let id = command.business as usize;
    let business = w.businesses.get(id).ok_or("sale business outside pool")?;
    let owner = business[10] as i8;
    let value = crate::ai_actions::liquidation_score::business_value(business, w.buildings, image)?
        .to_f32();
    if owner != 0 {
        w.players
            .get_mut((i32::from(owner) - 1) as usize)
            .ok_or("sale owner unresolved")?
            .post(value, 0x18)?;
        // Keep feedback before ownership transfer and initialization.
        callback(
            Call::AccountPosted(crate::economy::AccountPosting {
                player: (i32::from(owner) - 1) as usize,
                amount: value,
            }),
            w,
        )?;
    }
    let building = i16::from_le_bytes(w.businesses[id][8..10].try_into().unwrap()) as usize;
    let prefix = w.buildings.get(building).ok_or("sale building missing")?[..64]
        .try_into()
        .unwrap();
    crate::economy::transfer_business_owner(
        &mut w.businesses[id],
        prefix,
        0,
        controls.accounting,
        w.players,
    )?;
    let building = i16::from_le_bytes(w.businesses[id][8..10].try_into().unwrap()) as usize;
    let b = w
        .buildings
        .get_mut(building)
        .ok_or("sale building missing after transfer")?;
    b[0x2d] = 0;
    put(b, 0x30, controls.ownership_tick);
    if word(b, 0x4c) == 1 && (word(b, 0x1c) == u32::MAX || matches!(word(b, 0x24), 0 | 1)) {
        callback(Call::BuildingAppearance(building), w)?;
    }
    let business = w
        .businesses
        .get_mut(id)
        .ok_or("sale business missing after appearance")?;
    let building = i16::from_le_bytes(business[8..10].try_into().unwrap()) as usize;
    let kind = word(business, 4) as usize;
    let info = w.catalog.row(kind).ok_or("sale catalog kind missing")?;
    let n = if word(business, 0) == 0 {
        usize::from(info[0x4c])
    } else {
        0
    };
    let products = w.products[kind]
        .get(..n)
        .ok_or("sale product count exceeds six")?
        .iter()
        .map(|p| {
            let c = w
                .commodities
                .get(p[0] as usize)
                .ok_or("sale commodity missing")?;
            Ok(crate::business_initialize::Product {
                value: f32::from_bits(word(c, 20)),
                markup: f32::from_bits(word(c, 24)),
                fraction: f32::from_bits(p[1]),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let input = crate::business_initialize::Inputs {
        hour: controls.hour,
        timestamp: controls.timestamp,
        initializer: 0,
        info,
        products: &products,
    };
    let initialization = crate::business_initialize::initialize_before_charge(
        business,
        w.buildings
            .get_mut(building)
            .ok_or("sale initialization building missing")?,
        &input,
        image,
    )?;
    if let Some(amount) = initialization.charge {
        let b = &mut w.businesses[id];
        let mut value = crate::x87::Sum::from_f32(f32::from_bits(word(b, 0x1c)))?;
        value.add_f32(amount)?;
        put(b, 0x1c, value.to_f32().to_bits());
        if b[10] != 0 {
            let player = (i32::from(b[10] as i8) - 1) as usize;
            w.players
                .get_mut(player)
                .ok_or("sale stock owner missing")?
                .post(amount, 0x11)?;
            callback(
                Call::AccountPosted(crate::economy::AccountPosting { player, amount }),
                w,
            )?;
        }
    }
    crate::business_initialize::initialize_after_charge(
        &mut w.businesses[id],
        &input,
        image,
        initialization,
    )?;
    *w.age = w.age.wrapping_add(100);
    callback(Call::RefreshInterface, w)?;
    let building = i16::from_le_bytes(w.businesses[id][8..10].try_into().unwrap()) as usize;
    // Original407660 clears the pending-building flag before checking whether
    // this is the selected interface object.
    w.buildings
        .get_mut(building)
        .ok_or("sale panel building missing")?[0x3e] = 0;
    callback(Call::BuildingPanel(building), w)
}

#[test]
#[ignore = "requires original48ae60 packet observations"]
fn matches_native_sale_producer() {
    let data = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_SALE_PRODUCER")).unwrap();
    assert_eq!(&data[..8], b"MTBSPR01");
    assert_eq!(word(&data, 8), 0x37f);
    for row in data[16..].chunks_exact(64) {
        let mut packet = row[8..36].try_into().unwrap();
        Command {
            business: word(row, 0),
            player: word(row, 4) as u8,
        }
        .write_packet(&mut packet);
        assert_eq!(packet.as_slice(), &row[36..]);
    }
    assert_eq!(data.len(), 16 + word(&data, 12) as usize * 64);
}

#[test]
#[ignore = "requires complete original command13 observations"]
fn matches_native_sales() {
    struct R<'a>(&'a [u8]);
    impl<'a> R<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    fn rows<const N: usize>(b: &[u8]) -> Vec<[u8; N]> {
        b.chunks_exact(N).map(|r| r.try_into().unwrap()).collect()
    }
    let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let data = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_SALE")).unwrap();
    let mut r = R(&data);
    assert_eq!(r.take(8), b"MTBSALE1");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 4] = std::array::from_fn(|_| r.word());
        let b: Vec<_> = (0..16).map(|_| r.blob()).collect();
        let mut trace = vec![];
        for _ in 0..h[3] {
            let address = r.word();
            let n = r.word();
            trace.push((address, (0..n).map(|_| r.word()).collect::<Vec<_>>()));
        }
        let mut businesses = rows::<544>(b[0]);
        let mut buildings = rows::<916>(b[1]);
        let catalog =
            crate::business_pool::BusinessCatalog::from_rows(rows::<116>(b[2]).try_into().unwrap());
        let products = std::array::from_fn(|k| {
            std::array::from_fn(|i| [word(b[3], k * 48 + i * 8), word(b[3], k * 48 + i * 8 + 4)])
        });
        let commodities = rows::<28>(b[4]);
        let mut players = (5..8)
            .map(|i| crate::economy::PlayerAccount::from_runtime(b[i]).unwrap())
            .collect::<Vec<_>>();
        let mut age = (h[0] * 17) as u8;
        let mut world = World {
            businesses: &mut businesses,
            buildings: &mut buildings,
            players: &mut players,
            catalog: &catalog,
            products: &products,
            commodities: commodities.as_slice().try_into().unwrap(),
            age: &mut age,
        };
        let mut actual = vec![];
        apply(
            &Command {
                business: 0,
                player: (h[0] % 6) as u8,
            },
            &mut world,
            &Controls {
                current_player: None,
                accounting: !(h[0] / 4).is_multiple_of(2),
                ownership_tick: 271 + h[0],
                timestamp: 1200 + h[0],
                hour: h[2] as i32,
            },
            &image,
            |call, _| {
                if let Call::AccountPosted(posting) = call {
                    actual.extend(crate::account_posting_test_support::cash_calls(
                        posting.amount,
                        false,
                        0.,
                    ));
                    return Ok(());
                }
                actual.push(match call {
                    Call::BuildingAppearance(id) => (0x485520, vec![0x591318 + id as u32 * 916]),
                    Call::RefreshInterface => (0x41f410, vec![]),
                    Call::BuildingPanel(id) => (0x407660, vec![id as u32]),
                    Call::AccountPosted(_) => unreachable!(),
                });
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(actual, trace, "case{case} presentation order");
        assert_eq!(u32::from(age), h[1]);
        for (at, (a, z)) in businesses[0].iter().zip(b[8]).enumerate() {
            assert_eq!(a, z, "case{case} business{at:x}");
        }
        for (at, (a, z)) in buildings[0].iter().zip(b[9]).enumerate() {
            assert_eq!(a, z, "case{case} building{at:x}");
        }
        for (i, p) in players.iter().enumerate() {
            let mut actual = b[5 + i].to_vec();
            p.write_runtime(&mut actual).unwrap();
            for (at, (a, z)) in actual.iter().zip(b[13 + i]).enumerate() {
                assert_eq!(a, z, "case{case} player{i} byte{at:x}");
            }
        }
    }
    assert!(r.0.is_empty());
}
