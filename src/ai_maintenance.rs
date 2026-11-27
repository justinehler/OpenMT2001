//! Original467dc0/467c30: convert, sell, or demolish failing businesses.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};
pub struct World<'a> {
    pub building_parameters: crate::building_parameters::BuildingKinds,
    pub image: &'a PeImage<'a>,
    pub profiles: &'a [[u8; 0x650]],
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub catalog: &'a crate::business_pool::BusinessCatalog,
    pub entries: &'a [[u8; 28]],
    pub counts: &'a mut [u32; 52 * 45],
    pub identity: &'a mut u32,
    pub prior: [u8; 104],
}
/// Original4683f0. Its product pointer is initialized to zero at468411 and
/// tested at4684cd without ever becoming nonzero. The per-product adjustment
/// branch is unreachable in v1.4; the final464060 dirty-settings submission
/// still runs, including for businesses outside this player's ownership list.
/// Complete native passes verify both unchanged prices and emitted commands.
pub fn late_pricing(
    player: &Player,
    businesses: &mut [[u8; 544]],
    submit: impl FnMut(crate::economy::PendingBusinessTerms, &mut [[u8; 544]]) -> Result<(), String>,
) -> Result<(), String> {
    crate::economy::submit_business_terms(businesses, i32::from(player.bytes[0x1bc]), submit)
}

#[test]
#[ignore = "requires whole original4683f0 observations"]
fn matches_native_late_pricing() {
    let data = std::fs::read(crate::fixtures::path("NATIVE_AI_LATE_PRICING")).unwrap();
    assert_eq!(&data[..8], b"MTALPR01");
    assert_eq!(word(&data, 8), 0x37f);
    let mut at = 16;
    let mut loops = 0;
    for case in 0..word(&data, 12) {
        let commands = word(&data, at + 4) as usize;
        loops += word(&data, at + 8);
        assert_eq!(word(&data, at + 12), 0);
        at += 16;
        let mut blobs = vec![];
        for _ in 0..7 {
            let n = word(&data, at) as usize;
            at += 4;
            blobs.push(&data[at..at + n]);
            at += n;
        }
        let player = Player {
            reference: 0x10010000,
            bytes: blobs[0].to_vec(),
            auxiliaries: vec![],
        };
        let mut businesses: Vec<[u8; 544]> = blobs[2]
            .chunks_exact(544)
            .map(|r| r.try_into().unwrap())
            .collect();
        let mut actual = vec![];
        late_pricing(&player, &mut businesses, |command, _| {
            actual.extend(command.business_index.to_le_bytes());
            for target in command.terms.stock_targets {
                actual.extend(target.to_le_bytes());
            }
            for price in command.terms.prices {
                actual.extend(price.to_bits().to_le_bytes());
            }
            actual.extend(command.terms.rent.to_bits().to_le_bytes());
            Ok(())
        })
        .unwrap();
        assert_eq!(actual, &data[at..at + commands * 44], "case{case} commands");
        at += commands * 44;
        for (i, (a, b)) in businesses.iter().flatten().zip(blobs[6]).enumerate() {
            assert_eq!(a, b, "case{case} byte{i:x}");
        }
    }
    assert!(loops > 100);
    assert_eq!(at, data.len());
}
///4754b0, a read-only portfolio estimate used while choosing a cash strategy.
pub fn portfolio(player: &Player, w: &World<'_>) -> Result<i32, String> {
    let count = usize::from(u16::from_le_bytes(
        player.bytes[0xb44..0xb46].try_into().unwrap(),
    ));
    let mut total = 0f32;
    // The original portfolio loop reads physical ownership rows without checking
    // their active byte. A removed -1 row points into the zero-initialized data
    // before the business pool and still values live building 0 capital.
    let prepool_business = [0u8; 544];
    for entry in w
        .entries
        .get(..count)
        .ok_or("portfolio ownership rows missing")?
    {
        let id = word(entry, 4);
        let row = if id == u32::MAX {
            &prepool_business
        } else {
            business(id, w)?
        };
        let mut value =
            crate::ai_actions::liquidation_score::business_value(row, w.buildings, w.image)?;
        value.add_f32(total)?;
        total = value.to_f32();
    }
    for group in player.bytes[0xaac..0xb3c].chunks_exact(12) {
        if group[0] as i8 <= 0 {
            continue;
        }
        for &id in &group[1..5] {
            if (id as i8) < 0 || word(group, 8) & 1u32.wrapping_shl(u32::from(id)) == 0 {
                continue;
            }
            let mut value = crate::ai_messages::offers::execution::category_value(
                w.categories
                    .get(id as usize)
                    .ok_or("portfolio category missing")?,
                w.image,
            )?;
            value.mul_f32(w.image.f32_at(0x52502c)?)?;
            value.add_f32(total)?;
            total = value.to_f32();
        }
    }
    Ok(Sum::from_f32(total)?.ftol_low_word())
}
fn category<'a>(building: &[u8; 916], w: &'a World<'_>) -> Result<&'a [u8; 380], String> {
    let reference = word(building, 0x40);
    let offset = reference
        .checked_sub(0x617628)
        .ok_or("maintenance category precedes pool")?;
    if offset % 380 != 0 {
        return Err("maintenance category unaligned".into());
    }
    let c = w
        .categories
        .get(offset as usize / 380)
        .ok_or("maintenance category missing")?;
    w.categories
        .get(word(c, 0) as usize)
        .ok_or_else(|| "maintenance category identity missing".into())
}
fn building<'a>(id: u32, w: &'a World<'_>) -> Result<&'a [u8; 916], String> {
    w.buildings
        .get(id as usize)
        .ok_or_else(|| "maintenance building missing".into())
}
///445520, retaining PC64 until the caller's FST32 quote.
pub fn demolition_quote(id: u32, w: &World<'_>) -> Result<Sum, String> {
    crate::demolition::quote_with_parameters(
        building(id, w)?,
        w.categories,
        &w.building_parameters,
        w.image,
    )
}
///4453b0 with the live scripted business catalog and original binary64 constants.
pub fn conversion_quote(id: u32, kind: u32, w: &World<'_>) -> Result<Sum, String> {
    crate::business_conversion::quote(id, kind, w.categories, w.buildings, w.catalog, w.image)
}
///4787e0/4788a0. Prior bytes expose original untouched stack padding.
pub fn proposal(
    convert: bool,
    player: &mut Player,
    args: [u32; 4],
    w: &mut World<'_>,
) -> Result<bool, String> {
    use crate::ai_actions::proposals;
    if player.bytes[0x1404] > 9 {
        eprintln!("Original AI proposal capacity diagnostic 0053da84");
    }
    let mut row = w.prior;
    for (at, v) in [
        (0, 0),
        (8, 1),
        (12, 0),
        (16, if convert { 6 } else { 7 }),
        (24, u32::MAX),
        (32, 0),
        (36, 0),
        (52, 0),
        (56, 0),
        (60, 0),
        (64, 0),
        (72, 0),
        (76, args[3]),
        (80, 0),
        (88, u32::MAX),
        (92, u32::MAX),
        (96, 0),
    ] {
        put(&mut row, at, v);
    }
    row[4] = 99;
    row[68..70].copy_from_slice(&(args[2] as u16).to_le_bytes());
    if convert {
        put(&mut row, 40, args[0]);
        put(&mut row, 44, args[1]);
    } else {
        put(&mut row, 20, args[0]);
        put(&mut row, 40, u32::MAX);
        put(&mut row, 44, word(building(args[2], w)?, 0x1c));
    }
    proposals::admit(player, &mut row, w.identity, |call, _, _| {
        Err(format!("unexpected maintenance admission {call:?}"))
    })
}
fn decrement(category: u32, kind: u32, w: &mut World<'_>) -> Result<(), String> {
    let count = w
        .counts
        .get_mut(category as usize * 45 + kind as usize)
        .ok_or("maintenance category count missing")?;
    if *count as i32 > 0 {
        *count -= 1;
    }
    Ok(())
}
fn business<'a>(id: u32, w: &'a World<'_>) -> Result<&'a [u8; 544], String> {
    w.businesses
        .get(id as usize)
        .ok_or_else(|| "maintenance business missing".into())
}
fn business_building(id: u32, w: &World<'_>) -> Result<u32, String> {
    Ok(i32::from(i16::from_le_bytes(
        business(id, w)?[8..10].try_into().unwrap(),
    )) as u32)
}
fn business_category(id: u32, w: &World<'_>) -> Result<u32, String> {
    let id = business_building(id, w)?;
    if id == u32::MAX {
        Ok(u32::MAX)
    } else {
        Ok(word(category(building(id, w)?, w)?, 0) & 0xffff)
    }
}
fn limit(player: &Player, w: &World<'_>, selling: bool) -> Result<i32, String> {
    let profile = w
        .profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("maintenance profile missing")?;
    Ok(i32::from(
        *profile
            .get(
                0x5bdu32
                    .wrapping_add(word(&player.bytes, 0x13f4).wrapping_mul(36))
                    .wrapping_add(if selling { 2 } else { 0 }) as usize,
            )
            .ok_or("maintenance difficulty missing")?,
    ))
}
///46ed10 delegates to46e6a0 with the existing kind's opening-hour class.
pub fn replacement_mode(id: u32, w: &World<'_>) -> Result<i32, String> {
    let kind = word(business(id, w)?, 4);
    let row = w
        .catalog
        .row(kind as usize)
        .ok_or("maintenance time kind missing")?;
    Ok(i32::from(row[0x5c] >= 18))
}
pub fn maintain(
    selling: bool,
    player: &mut Player,
    w: &mut World<'_>,
    mut rank: impl FnMut(&mut Player, i32, &mut World<'_>) -> Result<i32, String>,
) -> Result<(), String> {
    let mut converted = false;
    let mut index = 0;
    while index
        < usize::from(u16::from_le_bytes(
            player.bytes[0xb46..0xb48].try_into().unwrap(),
        ))
    {
        let entry = *w
            .entries
            .get(index)
            .ok_or("maintenance allocated ownership row missing")?;
        index += 1;
        if entry[0] == 0 || word(&entry, 12) != 5 {
            continue;
        }
        let age = word(&entry, 16) as i32;
        let threshold = limit(player, w, selling)?;
        if if selling {
            age < threshold
        } else {
            age != threshold
        } {
            continue;
        }
        let id = word(&entry, 4);
        if selling {
            let cat = business_category(id, w)?;
            let mut sell = *player
                .bytes
                .get(0x178 + cat as usize)
                .ok_or("maintenance ownership category missing")?
                != 0;
            if !sell {
                let limit = crate::ai_profiles::adjustment(word(&player.bytes, 0x150))
                    .wrapping_add(1000)
                    .wrapping_mul(100);
                let rng = word(&player.bytes, 0x228)
                    .wrapping_mul(word(&player.bytes, 0x224))
                    .wrapping_add(word(&player.bytes, 0x22c));
                put(&mut player.bytes, 0x224, rng);
                let mut chance = Sum::from_i32(limit);
                chance.mul_f32(w.image.f32_at(0x525c94)?)?;
                sell = !Sum::from_i32((((rng >> 16) & 0x7fff) % 101) as i32)
                    .compare(chance)
                    .is_gt();
            }
            if sell {
                crate::ai_actions::proposals::transaction(
                    0x478410,
                    player,
                    [id, cat],
                    w.prior,
                    w.identity,
                    |call, _, _| Err(format!("unexpected maintenance sale {call:?}")),
                )?;
            } else {
                decrement(cat, word(business(id, w)?, 4), w)?;
                let b = business_building(id, w)?;
                let quote = demolition_quote(b, w)?.to_f32();
                proposal(false, player, [cat, id, b, quote.to_bits()], w)?;
            }
        } else {
            let mode = replacement_mode(id, w)?;
            let replacement = rank(player, mode, w)?;
            let mut b = u32::MAX;
            if replacement >= 0 && replacement as u32 != word(business(id, w)?, 4) {
                b = business_building(id, w)?;
                let quote = conversion_quote(b, replacement as u32, w)?.to_f32();
                let mut cash = Sum::from_f32(f32::from_bits(word(&player.bytes, 4)))?;
                cash.add(Sum::from_f32(f32::from_bits(word(&player.bytes, 0x1430)))?.negated());
                cash.add(Sum::from_f32(f32::from_bits(word(&player.bytes, 0x23c)))?.negated());
                if !cash.compare(Sum::from_f32(quote)?).is_lt() {
                    proposal(
                        true,
                        player,
                        [replacement as u32, id, b, quote.to_bits()],
                        w,
                    )?;
                    converted = true;
                    continue;
                }
            }
            if !converted && b != u32::MAX {
                let cat = word(category(building(b, w)?, w)?, 0);
                let quote = demolition_quote(b, w)?.to_f32();
                proposal(false, player, [cat, id, b, quote.to_bits()], w)?;
                decrement(business_category(id, w)?, word(business(id, w)?, 4), w)?;
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "requires original whole failing-business maintenance observations"]
fn matches_native_maintenance() {
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
    let data = std::fs::read(crate::fixtures::path("NATIVE_AI_MAINTENANCE")).unwrap();
    let mut r = R(&data);
    assert_eq!(r.take(8), b"MTMAINT2");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 5] = std::array::from_fn(|_| r.word());
        let b: Vec<_> = (0..11).map(|_| r.blob()).collect();
        let mut trace = vec![];
        for _ in 0..h[3] {
            trace.push((r.word(), r.word(), r.blob(), r.blob()));
        }
        let mut player = Player {
            reference: 0x10010000,
            bytes: b[0].to_vec(),
            auxiliaries: vec![],
        };
        let profiles = rows::<0x650>(b[1]);
        let categories = rows::<380>(b[2]);
        let buildings = rows::<916>(b[3]);
        let businesses = rows::<544>(b[4]);
        let catalog =
            crate::business_pool::BusinessCatalog::from_rows(rows::<116>(b[5]).try_into().unwrap());
        let entries = rows::<28>(b[6]);
        let mut counts = std::array::from_fn(|i| word(b[7], i * 4));
        let mut identity = 77;
        let mut w = World {
            building_parameters: crate::building_parameters::BuildingKinds::from_image(&image)
                .unwrap(),
            image: &image,
            profiles: &profiles,
            categories: categories.as_slice().try_into().unwrap(),
            buildings: &buildings,
            businesses: &businesses,
            catalog: &catalog,
            entries: &entries,
            counts: &mut counts,
            identity: &mut identity,
            prior: b[10].try_into().unwrap(),
        };
        let mut cursor = 0;
        if h[1] == 0x4754b0 {
            assert_eq!(
                portfolio(&player, &w).unwrap() as u32,
                h[4],
                "case{case} portfolio"
            );
        } else {
            maintain(h[1] == 0x467c30, &mut player, &mut w, |player, mode, _| {
                let (m, result, pre, post) = trace[cursor];
                cursor += 1;
                assert_eq!(mode as u32, m, "case{case} mode");
                for (at, (a, b)) in player.bytes.iter().zip(pre).enumerate() {
                    assert_eq!(a, b, "case{case} before rank byte{at:x}");
                }
                player.bytes.copy_from_slice(post);
                Ok(result as i32)
            })
            .unwrap();
        }
        assert_eq!(cursor, trace.len());
        assert_eq!(identity, h[2], "case{case} identity");
        for (at, (a, b)) in player.bytes.iter().zip(b[8]).enumerate() {
            assert_eq!(a, b, "case{case} player byte{at:x}");
        }
        for (i, count) in counts.iter().enumerate() {
            assert_eq!(*count, word(b[9], i * 4), "case{case} count{i}");
        }
    }
    assert!(r.0.is_empty());
}
