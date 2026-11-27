//! Original445300 command15 and451264 compulsory business acquisition.
use crate::{
    pe::PeImage,
    player_manager::{put, word},
    x87::Sum,
};
#[derive(Clone, Debug)]
pub struct Command {
    pub building: u32,
}
impl Command {
    pub fn write_packet(&self, packet: &mut [u8; 24]) {
        for (at, value) in [(0, 0x525af8), (4, 0x15), (12, 0), (20, self.building)] {
            put(packet, at, value);
        }
        packet[16] = 1;
    }
}
pub struct World<'a> {
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a mut [[u8; 916]],
    pub businesses: &'a mut [[u8; 544]],
    pub players: &'a mut [crate::economy::PlayerAccount],
    pub player_references: &'a [u32],
    pub player_slots: &'a [u8],
    pub age: &'a mut u8,
}
pub struct PriceWorld<'a> {
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub player_references: &'a [u32],
}
pub struct Controls {
    pub accounting: bool,
    pub ownership_tick: u32,
    pub network: [bool; 2],
    pub current_player: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    NativeUiCall {
        address: u32,
        args: [u32; 6],
        argument_count: u8,
    },
    BuildingAppearance(usize),
    Message([i32; 8]),
    RefreshInterface,
    BuildingPanel(usize),
}
fn row_from_pointer<'a>(
    building: &[u8; 916],
    categories: &'a [[u8; 380]; 52],
) -> Result<&'a [u8; 380], String> {
    let offset = word(building, 0x40)
        .checked_sub(0x617628)
        .ok_or("takeover category pointer precedes pool")?;
    if offset % 380 != 0 {
        return Err("takeover category pointer unaligned".into());
    }
    categories
        .get(offset as usize / 380)
        .ok_or_else(|| "takeover category pointer leaves pool".into())
}
fn owner_reference(owner: i8, references: &[u32]) -> Result<u32, String> {
    if owner == 0 {
        Ok(0)
    } else {
        references
            .get((i32::from(owner) - 1) as usize)
            .copied()
            .ok_or_else(|| "takeover category owner unresolved".into())
    }
}
fn monopoly(
    business: u32,
    group: u32,
    w: &PriceWorld<'_>,
    image: &PeImage<'_>,
) -> Result<bool, String> {
    let category = crate::ai_actions::liquidation::category(
        business,
        w.businesses,
        w.buildings,
        w.categories,
    )?;
    let owner = owner_reference(
        crate::ai_strategy_queries::category_owner(i32::from(category), w.categories, w.buildings)?,
        w.player_references,
    )?;
    if owner == 0 {
        return Ok(false);
    }
    crate::ai_strategy_queries::owns_group(
        owner,
        &crate::ai_strategy_queries::group(image, group)?,
        |id| {
            owner_reference(
                crate::ai_strategy_queries::category_owner(id, w.categories, w.buildings)?,
                w.player_references,
            )
        },
    )
}
///445180 re-reads the business/category identity at each original query. The
///FST32 intermediates and unsigned elapsed-tick division are observable.
pub fn price(
    building: usize,
    w: &PriceWorld<'_>,
    tick: u32,
    image: &PeImage<'_>,
) -> Result<Sum, String> {
    let b = w
        .buildings
        .get(building)
        .ok_or("takeover price building outside pool")?;
    let business = word(b, 0x1c);
    if business == u32::MAX {
        return Ok(Sum::from_f32(image.f32_at(0x525040)?)?);
    }
    let record = w
        .businesses
        .get(business as usize)
        .ok_or("takeover price business outside pool")?;
    let identity = word(row_from_pointer(b, w.categories)?, 0);
    let category = w
        .categories
        .get(identity as usize)
        .ok_or("takeover price category identity outside pool")?;
    let mut prestige = Sum::from_f32(f32::from_bits(word(category, 0x84)))?;
    prestige.mul_f32(image.f32_at(0x52502c)?)?;
    prestige.add_f32(image.f32_at(0x525034)?)?;
    let mut scaled =
        crate::ai_actions::liquidation_score::business_base_value(record, w.buildings, image)?;
    scaled.mul_f32(prestige.to_f32())?;
    let scaled = scaled.to_f32();
    let group = u32::from(image.at(0x5385a4u32.wrapping_add(identity.wrapping_mul(88)), 1)?[0]);
    let percent = if monopoly(business, group, w, image)? {
        0
    } else {
        //Original unsigned MUL D1B71759 followed by SHR EDX,9 is /625.
        let elapsed = tick.wrapping_sub(word(b, 0x30)) / 625;
        100i32.wrapping_sub(elapsed as i32).max(0)
    };
    let mut fraction = Sum::from_i32(percent);
    fraction.mul_f32(image.f32_at(0x5250e0)?)?;
    let fraction = fraction.to_f32();
    let mut extra = Sum::from_f32(fraction)?;
    extra.mul_f32(fraction)?;
    extra.mul_f32(scaled)?;
    let mut total =
        crate::ai_actions::liquidation_score::business_base_value(record, w.buildings, image)?;
    total.add(extra);
    let mut total = Sum::from_f32(total.to_f32())?;
    if monopoly(business, group, w, image)? {
        total.mul_f32(image.f32_at(0x525af4)?)?;
    }
    Ok(total)
}
pub fn apply(
    command: &Command,
    w: &mut World<'_>,
    controls: &Controls,
    image: &PeImage<'_>,
    mut callback: impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let id = command.building as usize;
    let building = w
        .buildings
        .get(id)
        .ok_or("takeover building outside pool")?;
    let business = word(building, 0x1c);
    if business == u32::MAX {
        return Ok(());
    }
    //The consumer derives its buyer from the property's current owner.
    let buyer_owner = row_from_pointer(building, w.categories)?[5] as i8;
    if owner_reference(buyer_owner, w.player_references)? == 0 {
        return Ok(());
    }
    let buyer = (i32::from(buyer_owner) - 1) as usize;
    let old_owner = w
        .businesses
        .get(business as usize)
        .ok_or("takeover business outside pool")?[10] as i8;
    let seller_ref = owner_reference(old_owner, w.player_references)?;
    let amount = price(
        id,
        &PriceWorld {
            categories: w.categories,
            buildings: w.buildings,
            businesses: w.businesses,
            player_references: w.player_references,
        },
        controls.ownership_tick,
        image,
    )?
    .to_f32();
    if seller_ref != 0 {
        let seller = (i32::from(old_owner) - 1) as usize;
        w.players
            .get_mut(seller)
            .ok_or("takeover seller missing")?
            .post(amount, 0x16)?;
        if controls.current_player == Some(seller) {
            notify_post(amount, w, image, &mut callback)?;
        }
    }
    w.players
        .get_mut(buyer)
        .ok_or("takeover buyer missing")?
        .post(-amount, 3)?;
    if controls.current_player == Some(buyer) {
        notify_post(-amount, w, image, &mut callback)?;
    }
    let buyer_slot = *w
        .player_slots
        .get(buyer)
        .ok_or("takeover buyer slot missing")?;
    let buyer_id = buyer_slot.wrapping_add(1);
    let business = word(&w.buildings[id], 0x1c) as usize;
    let actual_building =
        i16::from_le_bytes(w.businesses[business][8..10].try_into().unwrap()) as usize;
    let prefix = w
        .buildings
        .get(actual_building)
        .ok_or("takeover business building missing")?[..64]
        .try_into()
        .unwrap();
    crate::economy::transfer_business_owner(
        &mut w.businesses[business],
        prefix,
        buyer_id,
        controls.accounting,
        w.players,
    )?;
    let b = &mut w.buildings[id];
    b[0x2d] = buyer_id;
    put(b, 0x30, controls.ownership_tick);
    if word(b, 0x4c) == 1 && (word(b, 0x1c) == u32::MAX || matches!(word(b, 0x24), 0 | 1)) {
        callback(Call::BuildingAppearance(id), w)?;
    }
    *w.age = w.age.wrapping_add(100);
    if (!controls.network[0] || controls.network[1]) && seller_ref != 0 {
        let seller_slot = *w
            .player_slots
            .get((i32::from(old_owner) - 1) as usize)
            .ok_or("takeover seller slot missing")?;
        callback(
            Call::Message([
                i32::from(seller_slot),
                i32::from(buyer_slot),
                0x2e,
                command.building as i32,
                0,
                0,
                0,
                -999,
            ]),
            w,
        )?;
    }
    callback(Call::RefreshInterface, w)?;
    w.buildings[id][0x3e] = 0;
    callback(Call::BuildingPanel(id), w)
}

// Native48a5a0 presents each posting immediately. A same-player takeover has
// two postings; the Host pulse latch decides whether its second pulse starts.
fn notify_post(
    value: f32,
    w: &mut World<'_>,
    image: &PeImage<'_>,
    callback: &mut impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let threshold = image.f32_at(0x525040)?;
    let sound = |asset| Call::NativeUiCall {
        address: 0x4ad760,
        args: [asset, 0, 0, 0, 1, 0x3c],
        argument_count: 6,
    };
    let pulse = |address| Call::NativeUiCall {
        address,
        args: [0; 6],
        argument_count: 0,
    };
    if value > threshold {
        callback(sound(0x541cc8), w)?;
        callback(pulse(0x4c6290), w)?;
    } else if value < threshold || value.is_nan() {
        callback(pulse(0x4c62d0), w)?;
        callback(sound(0x541cb4), w)?;
    }
    Ok(())
}

#[test]
#[ignore = "requires original445300 packet observations"]
fn matches_native_takeover_producer() {
    let d = std::fs::read(std::env::var("OPENMT2001_NATIVE_BUSINESS_TAKEOVER_PRODUCER").unwrap())
        .unwrap();
    assert_eq!(&d[..8], b"MTBTPR01");
    assert_eq!(word(&d, 8), 0x37f);
    for r in d[16..].chunks_exact(52) {
        let mut packet = r[4..28].try_into().unwrap();
        Command {
            building: word(r, 0),
        }
        .write_packet(&mut packet);
        assert_eq!(packet.as_slice(), &r[28..]);
    }
    assert_eq!(d.len(), 16 + word(&d, 12) as usize * 52);
}
#[test]
#[ignore = "requires complete original command15 observations"]
fn matches_native_takeovers() {
    compare_native_takeovers("OPENMT2001_NATIVE_BUSINESS_TAKEOVER", false, false);
}
#[test]
#[ignore = "requires complete original current-player takeover observations"]
fn matches_native_takeover_cash_callbacks() {
    compare_native_takeovers("OPENMT2001_NATIVE_TAKEOVER_CASH", true, false);
}
#[test]
#[ignore = "requires original4810e0 command15 full stream/deadline observations"]
fn matches_native_saved_takeover_deadlines() {
    compare_native_takeovers("OPENMT2001_NATIVE_SAVED_TAKEOVER", true, true);
}
#[cfg(test)]
fn compare_native_takeovers(variable: &str, cash: bool, stream: bool) {
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
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let data = std::fs::read(std::env::var(variable).unwrap()).unwrap();
    let mut r = R(&data);
    assert_eq!(
        r.take(8),
        if stream {
            b"MTSTAK15"
        } else if cash {
            b"MTTAUI01"
        } else {
            b"MTBTAKE1"
        }
    );
    assert_eq!(r.word(), 0x37f);
    let mut cash_cases = 0;
    let mut both_posts = 0;
    let mut future_copies = 0;
    for case in 0..r.word() {
        let raw: Vec<_> = (0..if stream {
            14
        } else if cash {
            9
        } else {
            7
        })
            .map(|_| r.word())
            .collect();
        let h = [
            raw[0],
            raw[1],
            raw[2],
            raw[3],
            raw[4],
            raw[if cash { 7 } else { 5 }],
            raw[if cash { 8 } else { 6 }],
        ];
        let current = cash.then_some(raw[5] as usize);
        let readmitted = if stream {
            r.take(48); // Original queue's reconstructed host node references.
            Some(r.take(24))
        } else {
            None
        };
        let mut prior = if cash {
            [0; 48]
        } else {
            r.blob().try_into().unwrap()
        };
        let b: Vec<_> = if cash {
            assert_eq!(raw[6], 6);
            let regions: Vec<_> = (0..6)
                .map(|_| {
                    let n = r.word() as usize;
                    (r.take(n), r.take(n))
                })
                .collect();
            regions
                .iter()
                .map(|v| v.0)
                .chain(regions.iter().map(|v| v.1))
                .collect()
        } else {
            (0..12).map(|_| r.blob()).collect()
        };
        let mut trace = vec![];
        for _ in 0..h[5] {
            let address = r.word();
            let n = r.word();
            trace.push((address, (0..n).map(|_| r.word()).collect::<Vec<_>>()));
        }
        let sounds = trace.iter().filter(|call| call.0 == 0x4ad760).count();
        cash_cases += usize::from(sounds != 0);
        both_posts += usize::from(sounds == 2);
        if cash {
            prior = r.take(48).try_into().unwrap();
        }
        let nodes: Vec<_> = (0..h[6])
            .map(|_| {
                let address = r.word();
                (address, if cash { r.take(48) } else { r.blob() })
            })
            .collect();
        if stream {
            let variant = raw[9];
            assert_eq!(raw[10], 1200 + raw[0]);
            assert_eq!(raw[13], u32::from(variant == 2));
            assert_eq!(
                raw[11],
                match variant {
                    1 => u32::MAX,
                    2 => raw[1].saturating_sub(1),
                    _ => raw[10],
                }
            );
            let packet = readmitted.unwrap();
            if variant == 3 {
                assert_eq!(raw[12], raw[10] + 25);
                assert_eq!(trace.len(), 1);
                assert_eq!(trace[0].0, 0x480900);
                assert_eq!(&trace[0].1[..3], &[0, raw[12], 0]);
                assert_eq!(trace[0].1.len(), 4);
                let mut expected = [0; 24];
                Command { building: 0 }.write_packet(&mut expected);
                put(&mut expected, 8, 24);
                put(&mut expected, 12, raw[12]);
                assert_eq!(packet, expected);
                for region in 0..6 {
                    assert_eq!(b[region], b[region + 6]);
                }
                assert!(nodes.is_empty());
                assert_eq!(h[4], raw[0].wrapping_mul(17) as u8 as u32);
                future_copies += 1;
                continue;
            }
            assert_eq!(raw[12], raw[10]);
            assert_eq!(packet, [0; 24]);
        }
        let categories: Box<[[u8; 380]; 52]> =
            rows::<380>(b[0]).into_boxed_slice().try_into().unwrap();
        let mut buildings = rows::<916>(b[1]);
        let mut businesses = rows::<544>(b[2]);
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
        let references: Vec<_> = manager.players.iter().map(|p| p.reference).collect();
        let slots: Vec<_> = manager.players.iter().map(|p| p.bytes[0x1bc]).collect();
        let mut players = manager
            .players
            .iter()
            .map(|p| crate::economy::PlayerAccount::from_runtime(&p.bytes).unwrap())
            .collect::<Vec<_>>();
        let mut age = (h[0] * 17) as u8;
        let mut messages = crate::player_messages::Context {
            arena: crate::player_messages::Arena { next: 0x10060000 },
            current_player: current.map_or(0, |i| 0x10030000 + i as u32 * 0x400),
            network: [h[2] != 0, h[3] != 0],
            prior,
        };
        let mut actual = vec![];
        apply(
            &Command { building: 0 },
            &mut World {
                categories: &categories,
                buildings: &mut buildings,
                businesses: &mut businesses,
                players: &mut players,
                player_references: &references,
                player_slots: &slots,
                age: &mut age,
            },
            &Controls {
                accounting: !(h[0] / 4).is_multiple_of(2),
                ownership_tick: h[1],
                network: [h[2] != 0, h[3] != 0],
                current_player: current,
            },
            &image,
            |call, w| {
                actual.push(match call {
                    Call::NativeUiCall {
                        address,
                        args,
                        argument_count,
                    } => (address, args[..usize::from(argument_count)].to_vec()),
                    Call::BuildingAppearance(id) => (0x485520, vec![0x591318 + id as u32 * 916]),
                    Call::RefreshInterface => (0x41f410, vec![]),
                    Call::BuildingPanel(id) => (0x407660, vec![id as u32]),
                    Call::Message(args) => {
                        for (account, p) in w.players.iter().zip(&mut manager.players) {
                            account.write_runtime(&mut p.bytes)?;
                        }
                        messages.produce(args, &mut manager, |route, _| {
                            Err(format!("unexpected takeover route {route:?}"))
                        })?;
                        (0x48cab0, args.iter().map(|v| *v as u32).collect())
                    }
                });
                Ok(())
            },
        )
        .unwrap_or_else(|e| panic!("case{case}: {e}"));
        assert_eq!(actual, trace, "case{case} callback order");
        assert_eq!(u32::from(age), h[4]);
        assert_eq!(
            categories.as_slice().as_flattened(),
            b[6],
            "case{case} categories"
        );
        assert_eq!(buildings.as_flattened(), b[7], "case{case} buildings");
        assert_eq!(businesses.as_flattened(), b[8], "case{case} businesses");
        for (i, (account, p)) in players.iter().zip(&mut manager.players).enumerate() {
            account.write_runtime(&mut p.bytes).unwrap();
            for (at, (a, z)) in p.bytes.iter().zip(b[9 + i]).enumerate() {
                assert_eq!(a, z, "case{case} player{i} byte{at:x}");
            }
        }
        let actual_nodes: Vec<_> = manager
            .players
            .iter()
            .flat_map(|p| {
                p.auxiliaries
                    .iter()
                    .map(|a| (a.reference, a.bytes.as_slice()))
            })
            .collect();
        assert_eq!(actual_nodes, nodes, "case{case} retained inbox");
    }
    assert!(r.0.is_empty());
    if cash {
        assert_eq!(
            (cash_cases, both_posts),
            if stream { (39, 9) } else { (29, 6) }
        );
    }
    if stream {
        assert_eq!(future_copies, 48);
    }
}
