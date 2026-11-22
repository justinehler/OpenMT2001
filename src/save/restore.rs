//! Original record consumers. Initialization, script replay and final world
//! activation are separate ordered stages; copying records alone is not resume.
use crate::{
    building_pool,
    economy::BusinessTerms,
    player_manager::{Creation, Manager, Player},
};

/// 463f70's branch with no tenant callback during immediate saved command18.
/// The tag20 business row is already restored when 4810e0 admits the packet.
/// Linked tenants are untouched for ordered, nonincreasing rent; an increase
/// or unordered comparison synchronously enters 45ca80/45a770 and needs the
/// resident topology at this exact load point.
pub fn apply_business_terms_without_tenant_callback(
    businesses: &mut [[u8; 0x220]],
    index: u32,
    terms: &BusinessTerms,
) -> Result<(), String> {
    let row = businesses
        .get_mut(index as usize)
        .ok_or("saved immediate terms business leaves pool")?;
    let old_rent = f32::from_le_bytes(row[0x1dc..0x1e0].try_into().unwrap());
    if i32::from_le_bytes(row[0x20c..0x210].try_into().unwrap()) != -1
        && old_rent.partial_cmp(&terms.rent).is_none_or(|order| order.is_lt())
    {
        return Err("saved immediate terms require synchronous tenant callbacks".into());
    }
    for (i, price) in terms.prices.iter().enumerate() {
        row[0x1c4 + i * 4..0x1c8 + i * 4].copy_from_slice(&price.to_bits().to_le_bytes());
        row[0x1ec + i * 4..0x1f0 + i * 4].copy_from_slice(&price.to_bits().to_le_bytes());
    }
    for (i, target) in terms.stock_targets.iter().enumerate() {
        row[0x1b8 + i * 2..0x1ba + i * 2].copy_from_slice(&target.to_le_bytes());
        row[0x1e0 + i * 2..0x1e2 + i * 2].copy_from_slice(&target.to_le_bytes());
    }
    row[0x1dc..0x1e0].copy_from_slice(&terms.rent.to_bits().to_le_bytes());
    row[0x204..0x208].copy_from_slice(&terms.rent.to_bits().to_le_bytes());
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppearanceAlias {
    Building(usize),
    Business(usize),
}

/// The completed 485520 path when the record has no native renderer entries.
/// Original v1.4 485520 calls its local name lookup and optional business
/// name resolver, then returns without changing either record arena. A
/// positive +2e4 count continues through 4d4a00 and the renderer registry;
/// its synchronous effects need a separate retained scene owner.
pub fn activate_zero_model_appearance(
    building: usize,
    _alias: AppearanceAlias,
    row: &[u8; 916],
) -> Result<(), String> {
    let count = u32::from_le_bytes(row[0x2e4..0x2e8].try_into().unwrap());
    if count != 0 {
        return Err(format!(
            "saved building {building} has {count} original renderer entries requiring synchronous 485520"
        ));
    }
    Ok(())
}

/// Original 4bbd20 tail and 4450a0 callback gate. The callback represents
/// 485520, whose world/appearance effects are not part of this walker. Native
/// code reloads each node's +44 next pointer AFTER the callback; keep the
/// category and building arenas borrowed through the callback for that reason.
pub fn post_load_appearance_walk(
    categories: &mut [[u8; 380]],
    buildings: &mut [[u8; 916]],
    building_base: u32,
    mut callback: impl FnMut(
        usize,
        AppearanceAlias,
        &mut [[u8; 380]],
        &mut [[u8; 916]],
    ) -> Result<(), String>,
) -> Result<(), String> {
    let word = |row: &[u8], at: usize| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
    for category in 0..categories.len() {
        let mut reference = word(&categories[category], 0x15c);
        let mut visited = 0usize;
        while reference != 0 {
            let offset = reference
                .checked_sub(building_base)
                .ok_or("saved appearance list reference precedes building arena")?
                as usize;
            if !offset.is_multiple_of(916) || offset / 916 >= buildings.len() {
                return Err("saved appearance list reference leaves building arena".into());
            }
            let building = offset / 916;
            if visited >= buildings.len() {
                return Err("saved appearance category list is cyclic".into());
            }
            visited += 1;
            let row = &buildings[building];
            if word(row, 0x4c) == 1 {
                let linked = word(row, 0x1c);
                let alias = if linked == u32::MAX {
                    Some(AppearanceAlias::Building(building))
                } else if matches!(word(row, 0x24), 0 | 1) {
                    let business = linked as usize;
                    if business >= 600 {
                        return Err("saved appearance business link leaves pool".into());
                    }
                    Some(AppearanceAlias::Business(business))
                } else {
                    None
                };
                if let Some(alias) = alias {
                    callback(building, alias, categories, buildings)?;
                }
            }
            reference = word(&buildings[building], 0x44);
        }
    }
    Ok(())
}

/// Check the references needed by 4b0810 before the first resumed tick. The
/// 0x800 chunk stores indices, not pointers; its unused records may contain
/// stale words and must not be interpreted. The transition and command
/// consumers remain responsible for the actual completion effects.
pub fn pending_references(
    pending: &crate::pending_world::State,
    buildings: &[[u8; 916]],
    categories: &[[u8; 380]],
    player_count: usize,
) -> Result<(), String> {
    let mut active = 0u32;
    for (slot, record) in pending.records.iter().enumerate() {
        match record[2] {
            0 => continue,
            1 | 2 => {}
            kind => {
                return Err(format!(
                    "saved pending slot {slot} has unsupported kind {kind}"
                ));
            }
        }
        active += 1;
        let category = record[0] as usize;
        let building = record[1] as usize;
        if category >= categories.len() || building >= buildings.len() {
            return Err(format!(
                "saved pending slot {slot} references a missing category or building"
            ));
        }
        if u32::from_le_bytes(buildings[building][..4].try_into().unwrap()) != record[1] {
            return Err(format!(
                "saved pending slot {slot} building ID disagrees with its pool slot"
            ));
        }
        if record[2] == 1 && record[6] != 0 && record[6] as usize > player_count {
            return Err(format!(
                "saved pending slot {slot} completion owner leaves roster"
            ));
        }
    }
    if active != pending.count {
        return Err(format!(
            "saved pending count {} disagrees with {active} active records",
            pending.count
        ));
    }
    Ok(())
}

/// Original447d70. The caller has already removed existing category lists;
/// unspecified runtime tails retain the reset state supplied by that caller.
pub fn buildings(
    payload: &[u8; 0xb270],
    buildings: &mut [[u8; 916]],
    categories: &mut [[u8; 380]; 52],
    bases: [u32; 2],
    selection: &mut [u32; 2],
    geometry: &mut impl building_pool::GeometryBinding,
) -> Result<(), String> {
    if buildings.len() != 600 {
        return Err("loaded building pool requires600 slots".into());
    }
    for (record, prefix) in buildings.iter_mut().zip(payload[..0x9600].chunks_exact(64)) {
        record[..64].copy_from_slice(prefix);
    }
    for (record, prefix) in categories
        .iter_mut()
        .zip(payload[0x9600..].chunks_exact(140))
    {
        record[..140].copy_from_slice(prefix);
    }
    for (category, record) in categories.iter_mut().enumerate() {
        let count = u32::from_le_bytes(record[8..12].try_into().unwrap()) as i32;
        record[8..12].fill(0);
        if count > 24 {
            return Err("saved category exceeds original24 membership slots".into());
        }
        for i in 0..count.max(0) as usize {
            let slot = u32::from_le_bytes(record[12 + i * 4..16 + i * 4].try_into().unwrap());
            if slot == u32::MAX {
                continue;
            }
            building_pool::bind_category(
                record,
                buildings,
                slot as usize,
                bases[0],
                bases[1]
                    .checked_add(category as u32 * 380)
                    .ok_or("category reference overflow")?,
                geometry,
            )?;
        }
    }
    // Actual590f60 and6175f8 selected/current building references.
    *selection = [0; 2];
    Ok(())
}

pub struct PlayerContext<'a, 'image> {
    pub profiles: &'a mut [[u8; 0x650]],
    pub construction: &'a mut crate::player_constructor::RosterConstruction<'image>,
    pub metadata: &'a mut [[u8; 48]; 6],
    pub current_player: &'a mut u32,
    pub initial_cash: f32,
}

/// Original48c7d0 followed by48c800 and the real48c5d0 constructor path.
/// A held card invokes41bc30 after prefix copy/current-player selection. The
/// caller routes its actual32 broadcast synchronously through the live manager.
pub fn player(
    saved: &super::SavedPlayer<'_>,
    manager: &mut Manager,
    context: &mut PlayerContext<'_, '_>,
    mut held_card: impl FnMut(u32, u32, &mut Manager) -> Result<(), String>,
) -> Result<u32, String> {
    let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let before = word(&manager.bytes, 8) as usize;
    *context
        .metadata
        .get_mut(before)
        .ok_or("saved metadata exceeds six slots")? = *saved.metadata;
    let kind = word(saved.bytes, 0x150) as i32;
    let character = word(saved.bytes, 0x14c) as i32;
    let registered = manager.register(
        context.profiles,
        Creation {
            kind,
            character,
            name: &saved.bytes[0x154..],
            initial_cash: context.initial_cash,
        },
        &mut |manager, kind| context.construction.construct(manager, kind),
    )?;
    if !registered {
        return Err("saved player could not allocate its original slot".into());
    }
    let count = word(&manager.bytes, 8) as usize;
    let reference = word(&manager.bytes, 12 + count * 4);
    let row: &mut Player = manager
        .players
        .iter_mut()
        .find(|p| p.reference == reference)
        .ok_or("saved player manager tail is unresolved")?;
    row.bytes[4..0x1bc].copy_from_slice(saved.bytes);
    if *context.current_player == 0 {
        *context.current_player = reference;
    }
    let card = word(&row.bytes, 0x1b8);
    if card != 0 {
        held_card(reference, card, manager)?;
    }
    Ok(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original immediate command18 no-callback fixture"]
    fn native_immediate_terms_without_tenant_callbacks_match_business_row() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_SAVE_TERMS_APPLY").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTTERMS1");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x037f);
        let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 16 + count * (64 + 0x220 * 2));
        for case in 0..count {
            let at = 16 + case * (64 + 0x220 * 2);
            let packet = &bytes[at..at + 64];
            let mut row: [u8; 0x220] = bytes[at + 64..at + 64 + 0x220].try_into().unwrap();
            let expected = &bytes[at + 64 + 0x220..at + 64 + 0x440];
            let terms = BusinessTerms {
                stock_targets: std::array::from_fn(|i| {
                    i16::from_le_bytes(packet[24 + i * 2..26 + i * 2].try_into().unwrap())
                }),
                prices: std::array::from_fn(|i| {
                    f32::from_bits(u32::from_le_bytes(
                        packet[36 + i * 4..40 + i * 4].try_into().unwrap(),
                    ))
                }),
                rent: f32::from_bits(u32::from_le_bytes(packet[60..64].try_into().unwrap())),
            };
            apply_business_terms_without_tenant_callback(
                std::slice::from_mut(&mut row), 0, &terms,
            )
            .unwrap();
            assert_eq!(row.as_slice(), expected, "native 463f70 case {case}");
        }
    }
    #[test]
    #[ignore = "requires original 4bbd20/4450a0 list-walk fixture"]
    fn matches_native_post_load_appearance_walk() {
        const BASE: u32 = 0x591318;
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_SAVE_LIST_WALK").unwrap()).unwrap();
        let mut at = 0usize;
        let mut take = |len: usize| {
            let part = &bytes[at..at + len];
            at += len;
            part
        };
        let word = |bytes: &[u8]| u32::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(take(8), b"MTLSTW01");
        assert_eq!(word(take(4)), 1);
        assert_eq!(word(take(4)), 0x037f);
        assert_eq!(word(take(4)), 3);
        for case in 0..3 {
            assert_eq!(word(take(4)), case);
            let count = word(take(4));
            let expected: Vec<_> = (0..count)
                .map(|_| {
                    let building = word(take(4)) as usize;
                    let alias = match word(take(4)) {
                        0 => AppearanceAlias::Building(word(take(4)) as usize),
                        1 => AppearanceAlias::Business(word(take(4)) as usize),
                        _ => panic!("native alias kind"),
                    };
                    (building, alias)
                })
                .collect();
            let mut categories = vec![[0u8; 380]; 52];
            let mut buildings = vec![[0u8; 916]; 5];
            categories[0][0x15c..0x160].copy_from_slice(&BASE.to_le_bytes());
            categories[1][0x15c..0x160]
                .copy_from_slice(&(BASE + (if case == 2 { 3 } else { 4 }) * 916).to_le_bytes());
            for (i, building) in buildings.iter_mut().enumerate().take(3) {
                building[0x44..0x48].copy_from_slice(&(BASE + (i as u32 + 1) * 916).to_le_bytes());
            }
            for i in [0, 2, 3, 4] {
                buildings[i][0x4c..0x50].copy_from_slice(&1u32.to_le_bytes());
            }
            for (i, business, kind) in [
                (0, u32::MAX, 0u32),
                (2, 5, if case == 1 { 0 } else { 2 }),
                (3, 4, 1),
                (4, 3, 0),
            ] {
                buildings[i][0x1c..0x20].copy_from_slice(&business.to_le_bytes());
                buildings[i][0x24..0x28].copy_from_slice(&kind.to_le_bytes());
            }
            let mut actual = vec![];
            post_load_appearance_walk(
                &mut categories,
                &mut buildings,
                BASE,
                |building, alias, _, rows| {
                    actual.push((building, alias));
                    if case == 1 && building == 0 {
                        rows[0][0x44..0x48].copy_from_slice(&(BASE + 3 * 916).to_le_bytes());
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(actual, expected, "native case {case}");
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    fn post_load_appearance_walk_preserves_list_order_aliases_and_callback_relink() {
        const BASE: u32 = 0x591318;
        let mut categories = vec![[0u8; 380]; 2];
        let mut buildings = vec![[0u8; 916]; 5];
        categories[0][0x15c..0x160].copy_from_slice(&BASE.to_le_bytes());
        categories[1][0x15c..0x160].copy_from_slice(&(BASE + 4 * 916).to_le_bytes());
        for (i, building) in buildings.iter_mut().enumerate().take(3) {
            building[0x44..0x48].copy_from_slice(&(BASE + (i as u32 + 1) * 916).to_le_bytes());
        }
        buildings[0][0x4c..0x50].copy_from_slice(&1u32.to_le_bytes());
        buildings[0][0x1c..0x20].copy_from_slice(&u32::MAX.to_le_bytes());
        buildings[2][0x4c..0x50].copy_from_slice(&1u32.to_le_bytes());
        buildings[2][0x1c..0x20].copy_from_slice(&5u32.to_le_bytes());
        buildings[2][0x24..0x28].copy_from_slice(&2u32.to_le_bytes());
        buildings[3][0x4c..0x50].copy_from_slice(&1u32.to_le_bytes());
        buildings[3][0x1c..0x20].copy_from_slice(&4u32.to_le_bytes());
        buildings[3][0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
        buildings[4][0x4c..0x50].copy_from_slice(&1u32.to_le_bytes());
        buildings[4][0x1c..0x20].copy_from_slice(&3u32.to_le_bytes());
        buildings[4][0x24..0x28].copy_from_slice(&0u32.to_le_bytes());
        let mut calls = vec![];
        post_load_appearance_walk(
            &mut categories,
            &mut buildings,
            BASE,
            |building, alias, _, _| {
                calls.push((building, alias));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            calls,
            [
                (0, AppearanceAlias::Building(0)),
                (3, AppearanceAlias::Business(4)),
                (4, AppearanceAlias::Business(3))
            ]
        );
        // Make node2 callback-eligible, then redirect node0's next link from
        // inside 485520. The native tail must reread +44 and skip node2.
        buildings[2][0x24..0x28].copy_from_slice(&0u32.to_le_bytes());
        let mut calls = vec![];
        post_load_appearance_walk(
            &mut categories,
            &mut buildings,
            BASE,
            |building, alias, _, rows| {
                calls.push((building, alias));
                if building == 0 {
                    rows[0][0x44..0x48].copy_from_slice(&(BASE + 3 * 916).to_le_bytes());
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            calls,
            [
                (0, AppearanceAlias::Building(0)),
                (3, AppearanceAlias::Business(4)),
                (4, AppearanceAlias::Business(3))
            ]
        );
        buildings[0][0x44..0x48].copy_from_slice(&(BASE + 916).to_le_bytes());
        let mut calls = vec![];
        post_load_appearance_walk(
            &mut categories,
            &mut buildings,
            BASE,
            |building, alias, _, _| {
                calls.push((building, alias));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            calls,
            [
                (0, AppearanceAlias::Building(0)),
                (2, AppearanceAlias::Business(5)),
                (3, AppearanceAlias::Business(4)),
                (4, AppearanceAlias::Business(3))
            ]
        );
    }
    #[test]
    fn zero_model_appearance_preserves_record_and_rejects_renderer_entry() {
        let mut record = [0u8; 916];
        record[0x2d] = 3;
        record[0x1c..0x20].copy_from_slice(&5u32.to_le_bytes());
        let before = record;
        activate_zero_model_appearance(17, AppearanceAlias::Business(5), &record).unwrap();
        assert_eq!(record, before);
        record[0x2e4..0x2e8].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            activate_zero_model_appearance(17, AppearanceAlias::Business(5), &record)
                .unwrap_err()
                .contains("synchronous 485520")
        );
    }
    #[test]
    fn saved_pending_references_check_active_records_only() {
        let mut pending = crate::pending_world::State::default();
        let mut buildings = vec![[0; 916]; 2];
        buildings[1][..4].copy_from_slice(&1u32.to_le_bytes());
        let categories = vec![[0; 380]; 3];
        pending.records[0] = [2, 1, 1, 1, 0, 4, 1];
        pending.count = 1;
        pending.records[1] = [u32::MAX; 7];
        pending.records[1][2] = 0; // Stale free-slot words are serialized.
        pending_references(&pending, &buildings, &categories, 1).unwrap();
        pending.records[0][6] = 2;
        assert!(
            pending_references(&pending, &buildings, &categories, 1)
                .unwrap_err()
                .contains("owner")
        );
        pending.records[0][6] = 1;
        pending.records[0][1] = 3;
        assert!(
            pending_references(&pending, &buildings, &categories, 1)
                .unwrap_err()
                .contains("missing")
        );
        pending.records[0][1] = 1;
        pending.count = 0;
        assert!(
            pending_references(&pending, &buildings, &categories, 1)
                .unwrap_err()
                .contains("count")
        );
    }
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
        fn array<const N: usize>(&mut self) -> &'a [u8; N] {
            self.take(N).try_into().unwrap()
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires original complete saved-building reconstruction observations"]
    fn matches_native_saved_building_reconstruction() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_SAVE_RESTORE_BUILDINGS").unwrap())
                .unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTBLDL01");
        assert_eq!(r.word(), 0x37f);
        let count = r.word();
        struct Geometry {
            height: f32,
            prepared: Vec<u32>,
        }
        impl building_pool::GeometryBinding for Geometry {
            fn prepare(&mut self, slot: usize, row: &mut [u8; 916]) -> Result<(), String> {
                self.prepared.push(slot as u32);
                row[0x170..0x174].copy_from_slice(&1u32.to_le_bytes());
                row[0x220..0x224].copy_from_slice(&2u32.to_le_bytes());
                for (i, value) in [2f32, 3., 4., 8., 9., 10.].into_iter().enumerate() {
                    row[0x224 + i * 4..0x228 + i * 4].copy_from_slice(&value.to_le_bytes());
                }
                Ok(())
            }
            fn model_height(&mut self, handle: u32) -> Result<f32, String> {
                assert_eq!(handle, 1);
                Ok(self.height)
            }
        }
        for case in 0..count {
            let height = f32::from_bits(r.word());
            let payload = r.array();
            let mut rows: Vec<[u8; 916]> = (0..600).map(|_| *r.array()).collect();
            let mut categories: [_; 52] = std::array::from_fn(|_| *r.array());
            let expected_buildings = r.take(600 * 916);
            let expected_categories = r.take(52 * 380);
            let expected_selection = [r.word(), r.word()];
            let n = r.word();
            let expected_trace: Vec<_> = (0..n).map(|_| r.word()).collect();
            let mut geometry = Geometry {
                height,
                prepared: vec![],
            };
            let mut selection = [0x591318, 0x5916ac];
            buildings(
                payload,
                &mut rows,
                &mut categories,
                [0x591318, 0x617628],
                &mut selection,
                &mut geometry,
            )
            .unwrap();
            assert_eq!(
                geometry.prepared, expected_trace,
                "case{case} geometry order"
            );
            assert_eq!(selection, expected_selection);
            assert_eq!(
                rows.as_flattened(),
                expected_buildings,
                "case{case} all600 runtime buildings"
            );
            assert_eq!(
                categories.as_flattened(),
                expected_categories,
                "case{case} all52 runtime categories"
            );
        }
        assert!(r.0.is_empty());
        println!(
            "{count} whole saved-building reconstruction passes matched full pools, geometry order and selection cleanup"
        );
    }
    #[test]
    #[ignore = "requires complete original saved-player reconstruction observations"]
    fn matches_native_saved_player_reconstruction() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_SAVE_RESTORE_PLAYERS").unwrap())
            .unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTPLDL01");
        assert_eq!(r.word(), 0x37f);
        let count = r.word();
        let mut loaded = 0;
        for case in 0..count {
            let fill = r.word() as u8;
            let process = std::array::from_fn(|_| r.word());
            let n = r.word();
            let mut manager = Manager {
                bytes: *r.array(),
                players: vec![],
            };
            let mut metadata: [_; 6] = std::array::from_fn(|_| *r.array());
            let mut profiles: Vec<[u8; 0x650]> = (0..10).map(|_| *r.array()).collect();
            let saved: Vec<_> = (0..n)
                .map(|_| super::super::SavedPlayer {
                    metadata: r.array(),
                    bytes: r.array(),
                })
                .collect();
            let expected_manager = r.take(40);
            let expected_metadata = r.take(288);
            let expected_profiles = r.take(10 * 0x650);
            let expected_current = r.word();
            let expected_process = r.word();
            let expected_head = r.word();
            let expected_tail = r.word();
            let expected_reset = r.word();
            let allocations = r.word();
            let expected: Vec<_> = (0..allocations)
                .map(|_| {
                    let a = r.word();
                    (a, r.blob())
                })
                .collect();
            let traces = r.word();
            let expected_trace: Vec<[u32; 8]> = (0..traces)
                .map(|_| std::array::from_fn(|_| r.word()))
                .collect();
            let mut registry = crate::player_manager::RandomRegistry {
                process,
                head: 0,
                tail: 0,
            };
            let mut reset = 127;
            let mut next = 0x10050000u32;
            let mut allocated = vec![];
            let mut current = 0;
            let mut trace = vec![];
            let mut allocate = |size: usize| {
                let reference = next;
                next += (size as u32 + 4095) & !4095;
                allocated.push((reference, size));
                Ok(Some(crate::player_constructor::Allocation {
                    reference,
                    bytes: vec![fill; size],
                }))
            };
            let mut construction = crate::player_constructor::RosterConstruction {
                image: &image,
                registry: &mut registry,
                global_reset: &mut reset,
                allocate: &mut allocate,
                external_rng_link: &mut |_, _| Err("fixture unexpected external RNG link".into()),
            };
            for saved in saved {
                player(
                    &saved,
                    &mut manager,
                    &mut PlayerContext {
                        profiles: &mut profiles,
                        construction: &mut construction,
                        metadata: &mut metadata,
                        current_player: &mut current,
                        initial_cash: 10000.,
                    },
                    |reference, card, manager| {
                        let slot = manager
                            .players
                            .iter()
                            .find(|p| p.reference == reference)
                            .unwrap()
                            .bytes[0x1bc];
                        trace.push([
                            u32::MAX,
                            u32::from(slot),
                            0x32,
                            card,
                            0,
                            0,
                            0,
                            (-999i32) as u32,
                        ]);
                        Ok(())
                    },
                )
                .unwrap();
                loaded += 1;
            }
            assert_eq!(manager.bytes, expected_manager, "case{case} manager");
            assert_eq!(
                metadata.as_flattened(),
                expected_metadata,
                "case{case} metadata"
            );
            assert_eq!(
                profiles.as_flattened(),
                expected_profiles,
                "case{case} profiles"
            );
            assert_eq!(
                [
                    current,
                    registry.process[0],
                    registry.head,
                    registry.tail,
                    u32::from(reset)
                ],
                [
                    expected_current,
                    expected_process,
                    expected_head,
                    expected_tail,
                    expected_reset
                ],
                "case{case} globals"
            );
            assert_eq!(
                trace, expected_trace,
                "case{case} outgoing held-card request"
            );
            assert_eq!(allocated.len(), expected.len());
            for ((reference, size), (expected_reference, expected_bytes)) in
                allocated.iter().zip(expected)
            {
                assert_eq!(*reference, expected_reference);
                assert_eq!(*size, expected_bytes.len());
                let actual = manager
                    .players
                    .iter()
                    .find(|p| p.reference == *reference)
                    .map(|p| p.bytes.as_slice())
                    .or_else(|| {
                        manager
                            .players
                            .iter()
                            .flat_map(|p| &p.auxiliaries)
                            .find(|a| a.reference == *reference)
                            .map(|a| a.bytes.as_slice())
                    })
                    .expect("every original allocation retained");
                assert_eq!(actual, expected_bytes, "case{case} allocation{reference:x}");
            }
        }
        assert!(r.0.is_empty());
        println!(
            "{count} original saved-player sequences/{loaded} complete constructors, RNG links and card requests matched"
        );
    }
}
