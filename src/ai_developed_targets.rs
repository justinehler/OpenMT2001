//! Original47aa90/46cce0 developed-property selection and474290 scoring.
use crate::{
    ai_strategy_queries as q,
    business_pool::BusinessCatalog,
    pe::PeImage,
    player_manager::{Player, put, word},
};
pub struct World<'a> {
    pub image: &'a PeImage<'a>,
    pub profiles: &'a [[u8; 0x650]],
    pub hour: i32,
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub catalog: &'a BusinessCatalog,
    pub ownership: &'a crate::ai_ownership::Storage,
    pub residents: &'a crate::economy::ResidentAccounts,
    pub commodity_active: &'a [bool; 32],
}
fn catalog<'a>(kind: u32, w: &'a World<'_>) -> Result<&'a [u8; 116], String> {
    w.catalog
        .row(kind as usize)
        .ok_or_else(|| "AI developed-target business kind missing".into())
}
fn owned_rows<'a>(
    player: &Player,
    w: &'a World<'_>,
    allocated: bool,
) -> Result<&'a [[u8; 28]], String> {
    let at = if allocated { 0xb46 } else { 0xb44 };
    let count = u16::from_le_bytes(player.bytes[at..at + 2].try_into().unwrap()) as usize;
    if count == 0 {
        return Ok(&[]);
    }
    w.ownership
        .lists
        .get(&word(&player.bytes, 0xb3c))
        .and_then(|v| v.get(..count))
        .ok_or_else(|| "AI developed-target ownership allocation missing".into())
}
fn group_owned(player: &Player, id: u32, w: &World<'_>) -> Result<i32, String> {
    let group = w.image.at(0x5385a4 + id * 88, 1)?[0];
    q::owned_in_group(player, &q::group(w.image, u32::from(group))?, |c| {
        q::category_owner(c, w.categories, w.buildings)
    })
}
///46bef0 changes the target temporarily; the probed business kind remains.
pub fn probe(
    player: &mut Player,
    category: u32,
    kind: u32,
    mut plan: impl FnMut(&mut Player, &mut [u32; 2]) -> Result<bool, String>,
) -> Result<bool, String> {
    let old = word(&player.bytes, 0xb68);
    put(&mut player.bytes, 0xb68, category);
    put(&mut player.bytes, 0xb90, kind);
    let result = plan(player, &mut [u32::MAX; 2])?;
    put(&mut player.bytes, 0xb68, old);
    Ok(result)
}
///474210 scans active-count rows, without inspecting each row's active byte.
pub fn owned_kind(player: &Player, kind: u32, w: &World<'_>) -> Result<i32, String> {
    let mut n = 0i32;
    for row in owned_rows(player, w, false)? {
        let id = word(row, 4);
        // 474210 scans physical rows without checking their active byte.
        // A removed -1 row reads the zero-initialized kind word just before
        // the business pool, so it counts as kind 0.
        let row_kind = if id == u32::MAX {
            0
        } else {
            let business = w
                .businesses
                .get(id as usize)
                .ok_or("AI owned-kind business missing")?;
            word(business, 4)
        };
        if row_kind == kind {
            n = n.wrapping_add(1);
        }
    }
    Ok(n)
}
///46b2e0 scans the allocated-count prefix and filters its active bytes.
pub fn category_businesses(
    player: &Player,
    id: u32,
    w: &World<'_>,
    rows: &mut [i32; 24],
    count: &mut i32,
) -> Result<(), String> {
    *count = 0;
    for row in owned_rows(player, w, true)? {
        if row[0] == 0 {
            continue;
        }
        let business = word(row, 4);
        let b = w
            .businesses
            .get(business as usize)
            .ok_or("AI target business missing")?;
        let building = i16::from_le_bytes(b[8..10].try_into().unwrap());
        let category = if building == -1 {
            -1
        } else {
            let b = w
                .buildings
                .get(building as usize)
                .ok_or("AI target building missing")?;
            let reference = word(b, 0x40);
            let offset = reference
                .checked_sub(0x617628)
                .ok_or("AI target category reference before pool")?;
            if offset % 380 != 0 {
                return Err("AI target category reference unaligned".into());
            }
            let c = w
                .categories
                .get(offset as usize / 380)
                .ok_or("AI target category reference leaves pool")?;
            i16::from_le_bytes(c[..2].try_into().unwrap())
        };
        if i32::from(category) == id as i32 {
            *rows
                .get_mut(*count as usize)
                .ok_or("AI target native24row output overflow")? = business as i32;
            *count += 1;
        }
    }
    Ok(())
}
///474610 ground-area contribution for same-owner businesses in the time class.
pub fn owned_area(player: &Player, id: usize, mode: i32, w: &World<'_>) -> Result<i32, String> {
    let mut area = 0i32;
    q::visit_chain(&w.categories[id], w.buildings, |b| {
        let index = word(b, 0x1c) as i32;
        if index < 0 {
            return Ok(());
        }
        let business = w
            .businesses
            .get(index as usize)
            .ok_or("AI owned area business missing")?;
        if i32::from(business[10] as i8) != i32::from(player.bytes[0x1bc]) + 1 {
            return Ok(());
        }
        let day = catalog(word(business, 4), w)?[0x5c] < 18;
        if (mode == 0 && day) || (mode == 1 && !day) || mode == 2 {
            area = area.wrapping_add(
                (i32::from(b[7]) - i32::from(b[5]) + 1)
                    .wrapping_mul(i32::from(b[6]) - i32::from(b[4]) + 1),
            );
        }
        Ok(())
    })?;
    Ok(area)
}
///474290 includes planning side effects and the original final poll of45.
pub fn score(
    player: &mut Player,
    id: usize,
    mode: i32,
    w: &World<'_>,
    mut plan: impl FnMut(&mut Player, &mut [u32; 2]) -> Result<bool, String>,
) -> Result<i32, String> {
    let category = w
        .categories
        .get(id)
        .ok_or("AI developed score category missing")?;
    if category[5] != 0 && player.bytes[0x178 + id] == 0 {
        return Ok(0);
    }
    let state = word(category, 0x78);
    if state != 2 && (state != 1 || player.bytes[0x178 + id] == 0) {
        return Ok(0);
    }
    let (mut result, mut possible) = (0i32, 0i32);
    for kind in 0..45u32 {
        if kind == 44 || catalog(kind, w)?[0x30] == 0 || !probe(player, id as u32, kind, &mut plan)?
        {
            continue;
        }
        possible += 1;
        let (mut count, mut last) = (0, -1i32);
        q::visit_chain(category, w.buildings, |b| {
            let index = word(b, 0x1c) as i32;
            if index >= 0
                && word(
                    w.businesses
                        .get(index as usize)
                        .ok_or("AI score business missing")?,
                    4,
                ) == kind
            {
                count += 1;
                last = index;
            }
            Ok(())
        })?;
        let owned = owned_kind(player, kind, w)?;
        if count == 0 {
            result = result
                .wrapping_add((i32::from(catalog(kind, w)?[0x4c]) - owned + 10).wrapping_mul(10));
        } else if count == 1 {
            let b = &w.businesses[last as usize];
            let kind = word(b, 4);
            let owned = owned_kind(player, kind, w)?;
            let owner = b[10] as i8;
            let factor = if owner == 0 {
                5
            } else if i32::from(owner) != i32::from(player.bytes[0x1bc]) + 1 {
                3
            } else {
                0
            };
            result = result.wrapping_add(
                (i32::from(catalog(kind, w)?[0x4c]) - owned + 10).wrapping_mul(factor),
            );
        }
    }
    if possible <= 0 {
        return Ok(result);
    }
    let group = group_owned(player, id as u32, w)?;
    if player.bytes[0x178 + id] != 0 {
        result = result.wrapping_add(250);
        if group > 1 {
            result = result.wrapping_add(group.wrapping_mul(50).wrapping_sub(50));
        }
    } else if group > 0 {
        result = result.wrapping_add(group.wrapping_mul(50));
    }
    let (mut matching, mut opposite) = (0i32, 0i32);
    q::visit_chain(category, w.buildings, |b| {
        let index = word(b, 0x1c) as i32;
        if index >= 0 {
            let row = w
                .businesses
                .get(index as usize)
                .ok_or("AI score time-class business missing")?;
            let day = catalog(word(row, 4), w)?[0x5c] < 18;
            if (mode == 0 && day) || (mode == 1 && !day) {
                matching += 1;
            } else {
                opposite += 1;
            }
        }
        Ok(())
    })?;
    result = result.wrapping_add(
        matching
            .wrapping_mul(6)
            .wrapping_add(opposite)
            .wrapping_mul(25),
    );
    crate::consumer_poll::start(
        player,
        45,
        w.categories,
        w.buildings,
        w.businesses,
        w.residents,
        w.commodity_active,
    )?;
    crate::consumer_poll::stop(player);
    let polled = u16::from_le_bytes(player.bytes[0x20a..0x20c].try_into().unwrap());
    crate::consumer_poll::clear(player);
    Ok(result.wrapping_add(i32::from(polled)))
}
///46cce0. The maximum score is calculated but never used to filter the output.
pub fn fill(
    player: &mut Player,
    w: &World<'_>,
    rows: &mut [i32; 52],
    count: &mut i32,
    mut plan: impl FnMut(&mut Player, &mut [u32; 2]) -> Result<bool, String>,
) -> Result<bool, String> {
    *count = 0;
    let windows = crate::ai_selection::windows(player, w.profiles, w.hour)?;
    let profile = w
        .profiles
        .get(word(&player.bytes, 0x150) as usize)
        .ok_or("AI developed profile missing")?;
    let threshold = i32::from(
        *profile
            .get(0x5c8 + word(&player.bytes, 0x13f4) as usize * 36)
            .ok_or("AI developed difficulty missing")?,
    );
    let mode = if windows.0 {
        0
    } else if windows.1 {
        1
    } else {
        threshold
    };
    let mut best = 0i32;
    let mut businesses = [-1i32; 24];
    let mut business_count = 0;
    for id in 0..52 {
        if crate::ai_targets::undevelopable(w.image, id as u32)?
            || crate::ai_targets::allowed(player, id as u32)
        {
            continue;
        }
        if matches!(mode, 0 | 1) {
            let gate = crate::ai_candidate_queries::window_gate(
                true,
                &w.categories[id],
                w.buildings,
                w.businesses,
                w.catalog,
                w.image,
            )?;
            if (mode == 0 && !gate) || (mode == 1 && gate) {
                continue;
            }
        }
        let row = &w.categories[id];
        if word(row, 0x78) != 2 && player.bytes[0x178 + id] == 0 {
            continue;
        }
        category_businesses(player, id as u32, w, &mut businesses, &mut business_count)?;
        if score(player, id, mode, w, &mut plan)? > 0 {
            if player.bytes[0x178 + id] == 0 && business_count <= 0 {
                continue;
            }
            let mut value = business_count.wrapping_mul(10);
            let area = owned_area(player, id, mode, w)?;
            if area > 0 {
                value = value.wrapping_add(area.wrapping_mul(10));
            }
            if player.bytes[0x178 + id] != 0 {
                value = value.wrapping_add(20);
            } else if row[5] != 0 {
                value = value.wrapping_sub(20);
            }
            let group = group_owned(player, id as u32, w)?;
            if group > 0 {
                value = value.wrapping_add(group.wrapping_mul(10));
            }
            if value > threshold && value > best {
                best = value;
            }
        }
        if business_count > 0 {
            rows[*count as usize] = id as i32;
            *count += 1;
        }
    }
    Ok(*count > 0)
}
pub fn select(
    player: &mut Player,
    w: &World<'_>,
    plan: impl FnMut(&mut Player, &mut [u32; 2]) -> Result<bool, String>,
) -> Result<bool, String> {
    let mut rows = [-1; 52];
    let mut count = 0;
    if !fill(player, w, &mut rows, &mut count, plan)? {
        return Ok(false);
    }
    for &id in rows[..count as usize].iter().rev() {
        if id <= -1 {
            continue;
        }
        let c = &w.categories[id as usize];
        let day = crate::ai_candidate_queries::window_gate(
            true,
            c,
            w.buildings,
            w.businesses,
            w.catalog,
            w.image,
        )? && crate::ai_selection::windows(player, w.profiles, w.hour)?.0;
        if day
            || (crate::ai_candidate_queries::window_gate(
                false,
                c,
                w.buildings,
                w.businesses,
                w.catalog,
                w.image,
            )? && crate::ai_selection::windows(player, w.profiles, w.hour)?.1)
        {
            put(&mut player.bytes, 0xb54, 0);
            put(&mut player.bytes, 0xb68, id as u32);
            put(&mut player.bytes, 0xb64, 4);
            return Ok(true);
        }
    }
    Ok(false)
}

#[test]
#[ignore = "requires original developed-target observations"]
fn matches_native_developed_targets() {
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
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    fn rows<const N: usize>(b: &[u8]) -> Vec<[u8; N]> {
        b.chunks_exact(N).map(|r| r.try_into().unwrap()).collect()
    }
    let data =
        std::fs::read(std::env::var("OPENMT2001_NATIVE_AI_DEVELOPED_TARGETS").unwrap()).unwrap();
    let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let image = PeImage::parse(&exe).unwrap();
    let mut r = Reader(&data);
    assert_eq!(r.take(8), b"MTDEVT01");
    assert_eq!(r.word(), 0x37f);
    for case in 0..r.word() {
        let h: [u32; 5] = std::array::from_fn(|_| r.word());
        let b: Vec<_> = (0..8).map(|_| r.blob()).collect();
        let after = r.blob();
        let out = r.blob();
        let mut trace = vec![];
        for _ in 0..h[4] {
            let header: [u32; 3] = std::array::from_fn(|_| r.word());
            trace.push((header, r.blob(), r.blob()));
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
        let catalog = BusinessCatalog::from_rows(rows::<116>(b[5]).try_into().unwrap());
        let mut ownership = crate::ai_ownership::Storage::default();
        ownership.lists.insert(0x10020000, rows::<28>(b[6]));
        let residents = crate::economy::ResidentAccounts {
            active_count: 0,
            nodes: vec![],
            accounts: vec![],
        };
        let w = World {
            image: &image,
            profiles: &profiles,
            hour: h[2] as i32,
            categories: categories.as_slice().try_into().unwrap(),
            buildings: &buildings,
            businesses: &businesses,
            catalog: &catalog,
            ownership: &ownership,
            residents: &residents,
            commodity_active: &[false; 32],
        };
        let mut cursor = 0;
        let mut plan = |p: &mut Player, outputs: &mut [u32; 2]| {
            let (header, before, after) = &trace[cursor];
            cursor += 1;
            assert_eq!(p.bytes, *before, "case{case} planning player");
            assert_eq!(
                *outputs,
                [header[1], header[2]],
                "case{case} planning outputs"
            );
            *outputs = [1, 71];
            p.bytes.copy_from_slice(after);
            Ok(header[0] != 0)
        };
        let mut output_rows: [i32; 52] = std::array::from_fn(|i| word(b[7], i * 4) as i32);
        let mut count = word(b[7], 208) as i32;
        let result = match h[1] {
            0x47aa90 => u32::from(select(&mut player, &w, &mut plan).unwrap()),
            0x46cce0 => {
                u32::from(fill(&mut player, &w, &mut output_rows, &mut count, &mut plan).unwrap())
            }
            0x474290 => score(&mut player, 4, (h[0] % 3) as i32, &w, &mut plan).unwrap() as u32,
            0x474610 => owned_area(&player, 4, (h[0] % 3) as i32, &w).unwrap() as u32,
            0x474210 => owned_kind(&player, h[0] % 5, &w).unwrap() as u32,
            0x46b2e0 => {
                let mut entries = output_rows[..24].try_into().unwrap();
                category_businesses(&player, 4, &w, &mut entries, &mut count).unwrap();
                output_rows[..24].copy_from_slice(&entries);
                0
            }
            0x46bef0 => u32::from(probe(&mut player, 4, h[0] % 5, &mut plan).unwrap()),
            _ => panic!("unknown target entry"),
        };
        assert_eq!(result, h[3], "case{case} seed{} entry{:x}", h[0], h[1]);
        assert_eq!(player.bytes, after, "case{case} fullplayer");
        assert_eq!(cursor, trace.len(), "case{case} trace");
        let mut result_out = Vec::new();
        for v in output_rows.into_iter().chain([count]) {
            result_out.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(result_out, out, "case{case} outputrows");
    }
    assert!(r.0.is_empty());
}
