//! Original47ef20 neighbor cache and46c2b0/46c470 expansion selection.
use crate::{
    pe::PeImage,
    player_manager::{Player, put, word},
    x87::Sum,
};

#[derive(Clone, Debug)]
pub struct Cache {
    pub rows: [[u8; 56]; 52],
    pub owned_properties: [i32; 6],
    pub completed_groups: [i32; 6],
}
impl Default for Cache {
    fn default() -> Self {
        let mut rows = [[0; 56]; 52];
        for row in &mut rows {
            row[20..52].fill(255);
        }
        Self {
            rows,
            owned_properties: [0; 6],
            completed_groups: [0; 6],
        }
    }
}
impl Cache {
    ///47eee0 counts residents' current category byte7c, storing a wrapping WORD.
    pub fn refresh_population(
        &mut self,
        residents: &crate::economy::ResidentAccounts,
    ) -> Result<(), String> {
        let active = if residents.active_count as i32 > 0 {
            residents
                .nodes
                .get(..residents.active_count as usize)
                .ok_or("neighbor resident count exceeds pool")?
        } else {
            &[]
        };
        for row in &mut self.rows {
            let count = active
                .iter()
                .filter(|resident| resident.as_bytes()[0x7c] == row[0])
                .count() as u16;
            row[14..16].copy_from_slice(&count.to_le_bytes());
        }
        Ok(())
    }
    ///47fec0/47f990. Age is the authoritative City ownership refresh counter.
    pub fn groups(
        &mut self,
        age: &mut u8,
        slot: usize,
        players: &[&Player],
        categories: &[[u8; 380]; 52],
        residents: &crate::economy::ResidentAccounts,
        image: &PeImage<'_>,
    ) -> Result<i32, String> {
        if slot >= 6 || players.len() > 6 {
            return Err("group metrics player leaves native six slots".into());
        }
        if *age > 10 {
            *age = 0;
            self.refresh_owned(players, categories, residents, image)?;
        }
        Ok(self.completed_groups[slot])
    }
    pub fn refresh_owned(
        &mut self,
        players: &[&Player],
        categories: &[[u8; 380]; 52],
        residents: &crate::economy::ResidentAccounts,
        image: &PeImage<'_>,
    ) -> Result<(), String> {
        if players.len() > 6 {
            return Err("group metrics roster exceeds six slots".into());
        }
        self.owned_properties[..players.len()].fill(0);
        self.completed_groups[..players.len()].fill(0);
        for id in 0..52 {
            if image.at(0x5385a4 + id as u32 * 88, 1)?[0] == 0 {
                continue;
            }
            //47f2d0 uses the retained row ID, not its position in the array.
            let row = &mut self.rows[id];
            let category = usize::from(row[0]);
            let source = categories
                .get(category)
                .ok_or("group metrics category missing")?;
            row[1] = source[5];
            put(
                row,
                4,
                crate::ai_messages::offers::execution::category_value(source, image)?
                    .to_f32()
                    .to_bits(),
            );
            row[9..12].fill(0);
            if residents.active_count as i32 > 0 {
                for resident in residents
                    .nodes
                    .get(..residents.active_count as usize)
                    .ok_or("group metrics resident count exceeds pool")?
                {
                    let b = resident.as_bytes();
                    let value = (b[0x2c + category / 4] >> ((category % 4) * 2)) & 3;
                    row[9] = row[9].wrapping_add(value);
                }
            }
            if let Some(slot) = players.iter().position(|p| p.bytes[0x178 + id] != 0) {
                self.owned_properties[slot] = self.owned_properties[slot].wrapping_add(1);
            }
        }
        for (slot, player) in players.iter().enumerate() {
            for group in 0..12 {
                let ids = crate::ai_strategy_queries::group(image, group)?;
                if ids[0] == -1 || ids[1] == -1 {
                    continue;
                }
                let mut complete = true;
                for id in ids[..3].iter().copied().filter(|id| *id != -1) {
                    if *player
                        .bytes
                        .get(0x178 + id as usize)
                        .ok_or("native group metrics ownership index invalid")?
                        == 0
                    {
                        complete = false;
                        break;
                    }
                }
                if complete {
                    self.completed_groups[slot] = self.completed_groups[slot].wrapping_add(1);
                }
            }
        }
        Ok(())
    }
    /// Original47ef20 retains inactive rows, and preserves scan order and duplicates.
    pub fn refresh(
        &mut self,
        categories: &[[u8; 380]; 52],
        image: &PeImage<'_>,
    ) -> Result<(), String> {
        fn close(a: f32, b: f32, size: f32, padding: f32) -> Result<bool, String> {
            let mut distance = Sum::from_f32(a)?;
            distance.add(Sum::from_f32(b)?.negated());
            if distance.compare(Sum::default()).is_lt() {
                distance = distance.negated();
            }
            let mut bound = Sum::from_f32(size)?;
            bound.add_f32(padding)?;
            Ok(!bound.compare(distance).is_lt())
        }
        let value = |row: &[u8; 380], at| f32::from_bits(word(row, at));
        let padding = image.f32_at(0x5251c0)?;
        for (i, row) in categories.iter().enumerate() {
            self.rows[i][0] = i as u8;
            if row[4] == 0 {
                continue;
            }
            self.rows[i][12] = image.at(
                0x5385a4u32.wrapping_add(word(row, 0x8c).wrapping_mul(88)),
                1,
            )?[0];
            self.rows[i][20..52].fill(255);
            self.rows[i][52] = 0;
            let mut append = |id: u32| -> Result<(), String> {
                if id > image.f32_at(0x525d70)? as u32 {
                    return Err("AI neighbor category outside original bounds".into());
                }
                let n = usize::from(self.rows[i][52]);
                // The original writes beyond the eight entries after its debug
                // warning. Reject corrupt geometry instead of emulating heap damage.
                if n >= 8 {
                    return Err("AI neighbor cache exceeds eight entries".into());
                }
                put(&mut self.rows[i], 20 + n * 4, id);
                self.rows[i][52] += 1;
                Ok(())
            };
            for (j, other) in categories.iter().enumerate() {
                if i == j || other[4] == 0 {
                    continue;
                }
                if value(row, 0xe0) == value(other, 0xe0) {
                    if !close(
                        value(row, 0xe4),
                        value(other, 0xe4),
                        value(row, 0xd4),
                        padding,
                    )? {
                        continue;
                    }
                    append(word(other, 0))?;
                    for (k, diagonal) in categories.iter().enumerate() {
                        if k != j
                            && diagonal[4] != 0
                            && value(other, 0xe4) == value(diagonal, 0xe4)
                            && close(
                                value(other, 0xe0),
                                value(diagonal, 0xe0),
                                value(other, 0xd0),
                                padding,
                            )?
                        {
                            append(word(diagonal, 0))?;
                        }
                    }
                } else if value(row, 0xe4) == value(other, 0xe4)
                    && close(
                        value(row, 0xe0),
                        value(other, 0xe0),
                        value(row, 0xd0),
                        padding,
                    )?
                {
                    append(word(other, 0))?;
                }
            }
        }
        Ok(())
    }
}

pub struct World<'a> {
    pub image: &'a PeImage<'a>,
    pub profiles: &'a [[u8; 0x650]],
    pub hour: i32,
    pub categories: &'a [[u8; 380]; 52],
    pub buildings: &'a [[u8; 916]],
    pub businesses: &'a [[u8; 544]],
    pub catalog: &'a crate::business_pool::BusinessCatalog,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub residents: &'a crate::economy::ResidentAccounts,
    pub commodity_active: &'a [bool; 32],
    pub footprints: &'a [[i32; 2]; 52],
    pub cache: &'a Cache,
}
///46c640 includes the real consumer-poll lifecycle and native weighted scores.
pub fn score(player: &mut Player, id: u32, w: &World<'_>) -> Result<i32, String> {
    use crate::{ai_candidate_queries as q, ai_candidate_scores as s};
    let c = w
        .categories
        .get(id as usize)
        .ok_or("AI neighbor score category missing")?;
    let windows = crate::ai_selection::windows(player, w.profiles, w.hour)?;
    if !q::feasible(true, windows, c, w.buildings, w.businesses, w.catalog)? {
        return Ok(0);
    }
    crate::consumer_poll::start(
        player,
        id as i32,
        w.categories,
        w.buildings,
        w.businesses,
        w.residents,
        w.commodity_active,
    )?;
    crate::consumer_poll::stop(player);
    let mut total = i32::from(u16::from_le_bytes(
        player.bytes[0x20a..0x20c].try_into().unwrap(),
    ));
    crate::consumer_poll::clear(player);
    let perimeter = q::vacant_perimeter(c);
    if perimeter == 0 {
        return Ok(0);
    }
    let free = 49i32
        .wrapping_sub(perimeter)
        .wrapping_sub(crate::ai_strategy_queries::occupied_area(c, w.buildings)?);
    if free == 0 {
        return Ok(0);
    }
    total = total
        .wrapping_add(free)
        .wrapping_add(perimeter.wrapping_mul(2));
    let context = s::World {
        catalog: w.catalog,
        products: w.products,
        commodities: w.commodities,
        category: c,
        buildings: w.buildings,
        businesses: w.businesses,
        image: w.image,
    };
    for (own, multiplier) in [(true, 25), (false, 5)] {
        let value = s::ownership(own, player, windows, &context)?;
        if value > 0 {
            total = total.wrapping_add(value.wrapping_mul(multiplier));
        }
    }
    if player.bytes[0x178 + id as usize] != 0 {
        let group = w.image.at(0x5385a4 + id * 88, 1)?[0];
        let owned = crate::ai_strategy_queries::owned_in_group(
            player,
            &crate::ai_strategy_queries::group(w.image, u32::from(group))?,
            |id| crate::ai_strategy_queries::category_owner(id, w.categories, w.buildings),
        )?;
        total = total.wrapping_add(owned.wrapping_add(4).wrapping_mul(5));
    }
    Ok(total)
}

///46c470 build /46c2b0 buy. Only record new global maxima, as the original does.
pub fn candidate(build: bool, player: &mut Player, w: &World<'_>) -> Result<i32, String> {
    let windows = crate::ai_selection::windows(player, w.profiles, w.hour)?;
    if if build { !windows.0 } else { !windows.1 } {
        return Ok(-1);
    }
    let old = word(&player.bytes, 0xb68);
    put(&mut player.bytes, 0xb68, u32::MAX);
    let mut scores = [0i32; 52];
    let mut best = 0;
    let mut amount = 0;
    for id in 0..52 {
        if crate::ai_targets::undevelopable(w.image, id as u32)? {
            continue;
        }
        let mut owner = u32::MAX;
        let pending =
            crate::ai_actions::queries::pending_offer(player, id as u32, &mut owner, &mut amount)?;
        if if build {
            f32::from_bits(amount) > 0.
        } else {
            pending
        } {
            continue;
        }
        if player.bytes[0x178 + id] == 0
            && crate::ai_targets::owner_count(
                u32::from(player.bytes[0x1bc]),
                &w.categories[id],
                w.buildings,
                w.businesses,
            )? <= 0
        {
            continue;
        }
        let group = w.image.at(0x5385a4 + id as u32 * 88, 1)?[0];
        for slot in 0..8 {
            let next = word(&w.cache.rows[id], 20 + slot * 4);
            if next == u32::MAX {
                continue;
            }
            let c = w
                .categories
                .get(next as usize)
                .ok_or("AI adjacent category leaves pool")?;
            if crate::ai_targets::allowed(player, next)
                || !crate::ai_candidate_queries::window_gate(
                    build,
                    c,
                    w.buildings,
                    w.businesses,
                    w.catalog,
                    w.image,
                )?
                || !(word(c, 0x78) == 2
                    || (word(c, 0x78) == 1 && player.bytes[0x178 + next as usize] != 0))
                || group != w.image.at(0x5385a4 + next * 88, 1)?[0]
                || w.footprints[next as usize][1] < 6
            {
                continue;
            }
            put(&mut player.bytes, 0xb68, next);
            let value = score(player, next, w)?;
            if value > best {
                best = value;
                scores[next as usize] = value;
            }
        }
    }
    put(&mut player.bytes, 0xb68, old);
    let mut selected = -1;
    best = 0;
    for (i, value) in scores.into_iter().enumerate() {
        if value > best {
            best = value;
            selected = i as i32;
        }
    }
    Ok(selected)
}
///47aea0 prefers the build window, then the buy window, retaining output on failure.
pub fn select(player: &mut Player, mode: &mut u32, w: &World<'_>) -> Result<bool, String> {
    let windows = crate::ai_selection::windows(player, w.profiles, w.hour)?;
    let mut target = -1;
    if windows.0 {
        target = candidate(true, player, w)?;
        if target >= 0 {
            *mode = 0;
        }
    }
    if target == -1 && windows.1 {
        target = candidate(false, player, w)?;
        if target >= 0 {
            *mode = 1;
        }
    }
    if target < 0 {
        return Ok(false);
    }
    put(&mut player.bytes, 0xb64, 4);
    put(&mut player.bytes, 0xb68, target as u32);
    put(&mut player.bytes, 0xb54, 0);
    Ok(true)
}

/// Fields initialized by original449360 before47ef20. Geometry is immutable
/// during normal simulation and lies outside the saved category prefix.
pub fn bind_geometry(categories: &mut [[u8; 380]; 52], board: &crate::board::Board) {
    let geometry = board.category_geometry();
    for (i, row) in categories.iter_mut().enumerate() {
        put(row, 0, i as u32);
        row[4] = board.category_enabled()[i];
        for axis in 0..2 {
            put(row, 0xe0 + axis * 4, geometry.centers[i][axis].to_bits());
            put(row, 0xd0 + axis * 4, geometry.sizes[i][axis].to_bits());
        }
    }
    for block in &board.blocks {
        put(&mut categories[block.id], 0x8c, block.id as u32);
        put(&mut categories[block.id], 0x94, block.pavement_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    #[test]
    #[ignore = "requires complete original neighbor-selection observations"]
    fn matches_native_neighbors() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_NEIGHBORS")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTNEIG01");
        assert_eq!(r.word(), 0x37f);
        for case in 0..r.word() {
            let h: [u32; 5] = std::array::from_fn(|_| r.word());
            let b: Vec<_> = (0..8).map(|_| r.blob()).collect();
            let cb = r.blob();
            let ca = r.blob();
            let after = r.blob();
            let mut player = Player {
                reference: 0x10010000,
                bytes: b[0].to_vec(),
                auxiliaries: vec![],
            };
            let profiles = rows::<0x650>(b[1]);
            let categories: [[u8; 380]; 52] = rows::<380>(b[2]).try_into().unwrap();
            let buildings = rows::<916>(b[3]);
            let businesses = rows::<544>(b[4]);
            let catalog = crate::business_pool::BusinessCatalog::from_rows(
                rows::<116>(b[5]).try_into().unwrap(),
            );
            let mut cache = Cache {
                rows: rows::<56>(cb).try_into().unwrap(),
                ..Default::default()
            };
            cache.refresh(&categories, &image).unwrap();
            assert_eq!(cache.rows.as_flattened(), ca, "case{case} cache");
            let footprints =
                std::array::from_fn(|i| [word(b[7], i * 8) as i32, word(b[7], i * 8 + 4) as i32]);
            let residents = crate::economy::ResidentAccounts {
                active_count: 0,
                nodes: vec![],
                accounts: vec![],
            };
            let w = World {
                image: &image,
                profiles: &profiles,
                hour: h[2] as i32,
                categories: &categories,
                buildings: &buildings,
                businesses: &businesses,
                catalog: &catalog,
                products: &[[[0; 2]; 6]; 45],
                commodities: &[[0; 28]; 32],
                residents: &residents,
                commodity_active: &[false; 32],
                footprints: &footprints,
                cache: &cache,
            };
            let mut mode = 17;
            let result = match h[1] {
                0x47aea0 => u32::from(select(&mut player, &mut mode, &w).unwrap()),
                0x46c2b0 | 0x46c470 => candidate(h[1] == 0x46c470, &mut player, &w).unwrap() as u32,
                0x46c640 => score(&mut player, 1 + h[0] % 4, &w).unwrap() as u32,
                _ => panic!("unknown native neighbor entry"),
            };
            assert_eq!(result, h[3], "case{case} entry{:x}", h[1]);
            assert_eq!(mode, h[4], "case{case} mode");
            assert_eq!(player.bytes, after, "case{case} full player");
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires original board neighbor cache"]
    fn matches_native_board_neighbors() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_NEIGHBOR_BOARD")).unwrap();
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTNEIB01");
        let categories = rows::<380>(r.take(52 * 380)).try_into().unwrap();
        let mut cache = Cache {
            rows: rows::<56>(r.take(52 * 56)).try_into().unwrap(),
            ..Default::default()
        };
        cache.refresh(&categories, &image).unwrap();
        assert_eq!(cache.rows.as_flattened(), r.take(52 * 56));
        assert!(r.0.is_empty());
        // The live fresh-city geometry must supply every input this native cache consumes.
        let board = crate::board::Board::from_v14(&image).unwrap();
        let mut live = [[0u8; 380]; 52];
        bind_geometry(&mut live, &board);
        for block in &board.blocks {
            assert_eq!(
                word(&live[block.id], 0x94),
                word(&categories[block.id], 0x94),
                "native fresh category tint {}",
                block.name
            );
        }
        let mut actual = Cache::default();
        actual.refresh(&live, &image).unwrap();
        for (a, b) in actual.rows.iter().zip(cache.rows) {
            assert_eq!(&a[20..53], &b[20..53]);
            assert_eq!(a[12], b[12]);
        }
    }
}
#[cfg(test)]
mod metric_tests {
    use super::*;
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
    #[test]
    #[ignore = "requires original whole group metric captures"]
    fn matches_native_group_metrics() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_AI_GROUP_METRICS")).unwrap();
        let mut r = Reader(&data);
        assert_eq!(r.take(8), b"MTAIGRP1");
        assert_eq!(r.word(), 0x37f);
        let exe = std::fs::read("work/patch14/payload/Essential/mc.exe").unwrap();
        let image = PeImage::parse(&exe).unwrap();
        for i in 0..r.word() {
            let count = r.word() as usize;
            let slot = r.word() as usize;
            let n = r.word();
            let expected = r.word();
            let b: Vec<_> = (0..5).map(|_| r.blob()).collect();
            let after = r.blob();
            let expected_rows = r.blob();
            let mut object = b[0].to_vec();
            let mut cache = Cache {
                rows: b[1]
                    .chunks_exact(56)
                    .map(|b| b.try_into().unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
                owned_properties: std::array::from_fn(|i| word(&object, 0x8c + i * 16) as i32),
                completed_groups: std::array::from_fn(|i| word(&object, 0x94 + i * 16) as i32),
            };
            let categories = b[2]
                .chunks_exact(380)
                .map(|b| b.try_into().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let players: Vec<_> = b[3]
                .chunks_exact(0x1e50)
                .enumerate()
                .map(|(i, b)| Player {
                    reference: 0x10010000 + i as u32 * 0x1e50,
                    bytes: b.to_vec(),
                    auxiliaries: vec![],
                })
                .collect();
            let players: Vec<_> = players[..count].iter().collect();
            let residents = crate::economy::ResidentAccounts {
                active_count: n,
                nodes: b[4]
                    .chunks_exact(336)
                    .map(|b| crate::resident::Resident::from_bytes(b[..330].try_into().unwrap()))
                    .collect(),
                accounts: vec![],
            };
            assert_eq!(
                cache
                    .groups(
                        &mut object[6],
                        slot,
                        &players,
                        &categories,
                        &residents,
                        &image
                    )
                    .unwrap() as u32,
                expected,
                "case{i} result"
            );
            for j in 0..6 {
                put(&mut object, 0x8c + j * 16, cache.owned_properties[j] as u32);
                put(&mut object, 0x94 + j * 16, cache.completed_groups[j] as u32);
            }
            assert_eq!(object, after, "case{i} cache");
            assert_eq!(
                cache.rows.as_flattened(),
                expected_rows,
                "case{i} category rows"
            );
        }
        assert!(r.0.is_empty());
    }
}
