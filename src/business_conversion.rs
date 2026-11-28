//! Original48adf0 command12: charge4453b0, then the shared business-open tail.
use crate::{
    business_pool::BusinessCatalog,
    pe::PeImage,
    player_manager::{put, word},
    x87::Sum,
};
#[derive(Clone, Debug)]
pub struct Command {
    pub kind: u32,
    pub building: u32,
    pub owner: u32,
}
impl Command {
    pub fn write_packet(&self, b: &mut [u8; 32]) {
        for (at, value) in [
            (0, 0x525dec),
            (4, 0x12),
            (12, 0),
            (20, self.kind),
            (24, self.building),
            (28, self.owner),
        ] {
            put(b, at, value);
        }
        b[16] = 1;
    }
    pub fn opening(&self) -> crate::business_open::Command {
        crate::business_open::Command {
            kind: self.kind,
            building: self.building,
            owner: self.owner,
            stock_argument: 0,
        }
    }
}
///4453b0 reads the category's DWORD identity and mutable BusinessInfo cost.
pub fn quote(
    id: u32,
    kind: u32,
    categories: &[[u8; 380]; 52],
    buildings: &[[u8; 916]],
    catalog: &BusinessCatalog,
    image: &PeImage<'_>,
) -> Result<Sum, String> {
    let b = buildings
        .get(id as usize)
        .ok_or("conversion building outside pool")?;
    let offset = word(b, 0x40)
        .checked_sub(0x617628)
        .ok_or("conversion category precedes pool")?;
    if offset % 380 != 0 {
        return Err("conversion category unaligned".into());
    }
    let physical = categories
        .get(offset as usize / 380)
        .ok_or("conversion category outside pool")?;
    let c = categories
        .get(word(physical, 0) as usize)
        .ok_or("conversion category identity outside pool")?;
    let width = i32::from(b[6]) - i32::from(b[4]) + 1;
    let depth = i32::from(b[7]) - i32::from(b[5]) + 1;
    let mut value = Sum::from_i32(width.wrapping_mul(depth).wrapping_mul(i32::from(b[8])));
    value.mul(Sum::from_f64(f64::from_le_bytes(
        image.at(0x525b08, 8)?.try_into().unwrap(),
    ))?);
    value.add(Sum::from_f64(f64::from_le_bytes(
        image.at(0x525b00, 8)?.try_into().unwrap(),
    ))?);
    let mut location = Sum::from_f32(f32::from_bits(word(c, 0x84)))?;
    location.mul_f32(image.f32_at(0x52502c)?)?;
    location.add_f32(image.f32_at(0x525034)?)?;
    value.mul(location);
    value.mul(Sum::from_i32(word(
        catalog
            .row(kind as usize)
            .ok_or("conversion catalog missing")?,
        0x54,
    ) as i32));
    value.mul_f32(image.f32_at(match b[0x2c] as i8 {
        0 => 0x525034,
        1 => 0x525b10,
        _ => 0x5251e8,
    })?)?;
    Ok(value)
}
///451197 negates the extended quote and stores single precision before posting.
pub fn debit(
    command: &Command,
    quote: Sum,
    players: &mut [crate::economy::PlayerAccount],
) -> Result<crate::economy::AccountPosting, String> {
    let player = command.owner.wrapping_sub(1) as usize;
    let amount = -quote.to_f32();
    players
        .get_mut(player)
        .ok_or("conversion owner unresolved")?
        .post(amount, 0x14)?;
    Ok(crate::economy::AccountPosting { player, amount })
}
///463bb0 checks the live catalog flag, footprint, height and longest side.
/// The side length is a wrapped BYTE; area uses signed arithmetic followed by
/// unsigned comparisons. A zero-height enabled building faults before limits.
pub fn fits(building: &[u8; 916], info: &[u8; 116]) -> Result<bool, String> {
    if info[0x30] == 0 {
        return Ok(false);
    }
    let height = i32::from(building[8]);
    let width = i32::from(building[6]) - i32::from(building[4]) + 1;
    let depth = i32::from(building[7]) - i32::from(building[5]) + 1;
    let area = width
        .wrapping_mul(depth)
        .wrapping_mul(height)
        .checked_div(height)
        .ok_or("original463bb0 division by zero")? as u32;
    let side = u32::from((width as u8).max(depth as u8));
    Ok(area >= word(info, 0x38)
        && area <= word(info, 0x3c)
        && height as u32 <= word(info, 0x44)
        && height as u32 >= word(info, 0x48)
        && side <= word(info, 0x40))
}

#[test]
#[ignore = "requires original48adf0 packets"]
fn matches_native_conversion_producer() {
    let d = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_CONVERSION_PRODUCER")).unwrap();
    assert_eq!(&d[..8], b"MTBCPR01");
    assert_eq!(word(&d, 8), 0x37f);
    for r in d[16..].chunks_exact(76) {
        let mut b = r[12..44].try_into().unwrap();
        Command {
            kind: word(r, 0),
            building: word(r, 4),
            owner: word(r, 8) + 1,
        }
        .write_packet(&mut b);
        assert_eq!(b.as_slice(), &r[44..]);
    }
    assert_eq!(d.len(), 16 + word(&d, 12) as usize * 76);
}
#[test]
#[ignore = "requires complete original463bb0 eligibility observations"]
fn matches_native_conversion_fit() {
    let d = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_CONVERSION_FIT")).unwrap();
    assert_eq!(&d[..8], b"MTBCFIT1");
    assert_eq!(word(&d, 8), 0x37f);
    for (i, r) in d[16..].chunks_exact(1036).enumerate() {
        let actual = match fits(r[4..920].try_into().unwrap(), r[920..].try_into().unwrap()) {
            Ok(v) => u32::from(v),
            Err(_) => 2,
        };
        assert_eq!(actual, word(r, 0), "case{i}");
    }
    assert_eq!(d.len(), 16 + word(&d, 12) as usize * 1036);
}
#[test]
#[ignore = "requires original command12 charge/routing with allocation/binding seams"]
fn matches_native_conversion_entry() {
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
    let data = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_CONVERSION")).unwrap();
    let mut r = R(&data);
    assert_eq!(r.take(8), b"MTBCONV1");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 9] = std::array::from_fn(|_| r.word());
        let prior = r.blob().try_into().unwrap();
        let b: Vec<_> = (0..12).map(|_| r.blob()).collect();
        let mut expected = vec![];
        for _ in 0..h[7] {
            let address = r.word();
            let n = r.word();
            expected.push((address, (0..n).map(|_| r.word()).collect::<Vec<_>>()));
        }
        let nodes: Vec<_> = (0..h[8])
            .map(|_| {
                let id = r.word();
                (id, r.blob())
            })
            .collect();
        let categories: Box<[[u8; 380]; 52]> =
            rows::<380>(b[0]).into_boxed_slice().try_into().unwrap();
        let mut buildings = rows::<916>(b[1]);
        let catalog = BusinessCatalog::from_rows(rows::<116>(b[2]).try_into().unwrap());
        let mut manager = crate::player_manager::Manager {
            bytes: [0; 40],
            players: (0..3)
                .map(|i| crate::player_manager::Player {
                    reference: 0x10030000 + i as u32 * 0x400,
                    bytes: b[3 + i].to_vec(),
                    auxiliaries: vec![],
                })
                .collect(),
        };
        put(&mut manager.bytes, 8, 3);
        put(&mut manager.bytes, 12, 3);
        for (i, p) in manager.players.iter().enumerate() {
            put(&mut manager.bytes, 16 + i * 4, p.reference);
        }
        let mut accounts = manager
            .players
            .iter()
            .map(|p| crate::economy::PlayerAccount::from_runtime(&p.bytes).unwrap())
            .collect::<Vec<_>>();
        let mut messages = crate::player_messages::Context {
            arena: crate::player_messages::Arena { next: 0x10060000 },
            current_player: 0,
            network: [h[4] != 0, h[5] != 0],
            prior,
        };
        let command = Command {
            kind: h[1],
            building: 0,
            owner: h[2],
        };
        let value = quote(0, h[1], &categories, &buildings, &catalog, &image).unwrap();
        let mut trace = vec![
            (0x4453b0, vec![0x591318, h[1]]),
            (
                0x48a5a0,
                vec![
                    0x10030000 + (h[2] - 1) * 0x400,
                    (-value.to_f32()).to_bits(),
                    0x14,
                ],
            ),
        ];
        debit(&command, value, &mut accounts).unwrap();
        for (account, p) in accounts.iter().zip(&mut manager.players) {
            account.write_runtime(&mut p.bytes).unwrap();
        }
        let mut age = (h[0] * 17) as u8;
        let result = crate::business_open::apply(
            &mut command.opening(),
            &mut age,
            &mut crate::business_open::Controls {
                network_enabled: h[4] != 0,
                network_override: h[5] != 0,
            },
            |call, _, _, _| {
                let mut args = call.arguments[..call.count].to_vec();
                if call.address == 0x4504e0 {
                    args.push(0x10020000);
                }
                trace.push((call.address, args));
                if call.address == 0x48cab0 {
                    messages.produce(
                        call.arguments.map(|v| v as i32),
                        &mut manager,
                        |route, _| Err(format!("unexpected conversion route {route:?}")),
                    )?;
                }
                if call.address == 0x407520 {
                    buildings[0][0x3d] = 0;
                }
                Ok(if call.address == 0x4634c0 { h[3] } else { 0 })
            },
        )
        .unwrap();
        assert_eq!(result, h[3]);
        assert_eq!(u32::from(age), h[6]);
        assert_eq!(trace, expected, "case{case} exact callback order");
        assert_eq!(categories.as_slice().as_flattened(), b[6]);
        assert_eq!(buildings.as_flattened(), b[7], "case{case} building");
        assert_eq!(b[2], b[8]);
        for (i, p) in manager.players.iter().enumerate() {
            for (at, (a, z)) in p.bytes.iter().zip(b[9 + i]).enumerate() {
                assert_eq!(a, z, "case{case} player{i} byte{at:x}");
            }
        }
        let actual: Vec<_> = manager
            .players
            .iter()
            .flat_map(|p| {
                p.auxiliaries
                    .iter()
                    .map(|a| (a.reference, a.bytes.as_slice()))
            })
            .collect();
        assert_eq!(actual, nodes, "case{case} real2f inbox");
    }
    assert!(r.0.is_empty());
}
