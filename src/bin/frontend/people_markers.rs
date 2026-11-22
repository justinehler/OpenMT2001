//! Saved City View resident markers; original4b7380 and load fixup45bf31.
use openmt2001::{board::Board, pe::PeImage, save::SaveFile};
pub const LABELS: [u32; 4] = [1799, 1454, 1458, 1464];
pub const MODES: [u32; 4] = [4, 3, 1, 2];
#[derive(Clone, Debug)]
pub struct Marker {
    pub position: [f32; 3],
    pub visible: bool,
    pub colors: [u32; 4],
}
pub fn from_save(save: &SaveFile<'_>, image: &PeImage<'_>) -> Result<Vec<Marker>, String> {
    let centers = Board::from_v14(image)?.category_geometry().centers;
    let nodes = save.residents()?.nodes;
    let players = save.players()?;
    let economy = save.economy()?;
    let businesses: Vec<_> = economy.businesses().collect();
    let buildings = save
        .sections
        .iter()
        .find(|s| s.tag == 4)
        .ok_or("marker building table missing")?
        .payload;
    if buildings.len() != 0xb270 {
        return Err("marker building table length".into());
    }
    let palette: [u32; 3] = std::array::from_fn(|i| {
        u32::from_le_bytes(
            image
                .at(0x5336b0 + i as u32 * 4, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        )
    });
    nodes
        .into_iter()
        .enumerate()
        .map(|(slot, node)| {
            if usize::from(u16::from_le_bytes(node[0x148..0x14a].try_into().unwrap())) != slot {
                return Err("marker saved pool requires dense original slots".into());
            }
            let mut shopping = 0xffffffff;
            if node[0x7e] & 1 == 0 && node[0x54] != 0xff {
                let slot = i16::from_le_bytes(node[0x56..0x58].try_into().unwrap());
                let at = usize::try_from(slot)
                    .ok()
                    .filter(|&i| i < 600)
                    .ok_or("marker shopping building outside pool")?
                    * 64
                    + 0x1c;
                let business = i32::from_le_bytes(buildings[at..at + 4].try_into().unwrap());
                if business != -1 {
                    let business = businesses
                        .get(usize::try_from(business).map_err(|_| "negative marker business")?)
                        .ok_or("marker business outside pool")?;
                    let owner = business.bytes[10] as i8;
                    shopping = if owner == 0 {
                        0xff888888
                    } else {
                        let player = players
                            .get(usize::try_from(owner - 1).map_err(|_| "negative marker owner")?)
                            .ok_or("marker owner outside roster")?;
                        let c = u32::from_le_bytes(player.bytes[0x14c..0x150].try_into().unwrap());
                        if c >= 10 {
                            return Err("marker character outside palette".into());
                        }
                        u32::from_le_bytes(image.at(0x541af0 + c * 40, 4)?.try_into().unwrap())
                    };
                }
            }
            let colors = colors(node, shopping, palette);
            let mut resident = openmt2001::resident::Resident::from_bytes(*node);
            let mut auxiliary = [0; 284];
            // Only fields written by this verified fixup are read; scratch is not a model template.
            resident.restore_saved_movement(1, &mut auxiliary, &centers)?;
            let position = std::array::from_fn(|i| {
                f32::from_le_bytes(auxiliary[4 + i * 4..8 + i * 4].try_into().unwrap())
            });
            if auxiliary[0xb8] & 1 != 0 && position.iter().any(|v| !v.is_finite()) {
                return Err("nonfinite saved marker position".into());
            }
            Ok(Marker {
                position,
                visible: auxiliary[0xb8] & 1 != 0,
                colors,
            })
        })
        .collect()
}
fn colors(node: &[u8; 0x14a], shopping: u32, palette: [u32; 3]) -> [u32; 4] {
    let word = |at| u32::from_le_bytes(node[at..at + 4].try_into().unwrap());
    let kind = word(0);
    let home = i16::from_le_bytes(node[14..16].try_into().unwrap());
    let income = word(60);
    [
        shopping,
        palette[if kind == 4 {
            2
        } else if home == -1 {
            1
        } else {
            0
        }],
        palette[if kind == 0 {
            0
        } else if kind == 3 {
            2
        } else {
            1
        }],
        palette[if income == 0 {
            2
        } else if income == 1 {
            1
        } else {
            0
        }],
    ]
}
/// Snapshot actual initialized auxiliaries; never apply the saved-load fixup to a fresh city.
pub fn from_world(
    world: &openmt2001::level_startup::World,
    image: &PeImage<'_>,
    players: &openmt2001::player_manager::Manager,
) -> Result<Vec<Marker>, String> {
    from_runtime_pools(&world.ledgers.residents, &world.creation.auxiliary,
        &world.buildings, &world.ledgers.businesses, image, players)
}
/// Original4b7407/4b7486/4b7665 pair each used auxiliary with the resident at
/// the same ordinal, independent of active_count, +148 identity or +f0 reference.
/// Preserve all retained auxiliary rows, including older V2 imported pools.
fn from_runtime_pools(
    residents: &openmt2001::economy::ResidentAccounts,
    auxiliaries: &openmt2001::resident::AuxiliaryPool,
    buildings: &[[u8; 0x394]],
    businesses: &[[u8; 0x220]],
    image: &PeImage<'_>,
    players: &openmt2001::player_manager::Manager,
) -> Result<Vec<Marker>, String> {
    let count = auxiliaries.used();
    let nodes = residents.nodes.get(..count)
        .ok_or("People auxiliary iteration leaves resident pool")?;
    let mut palette = [0; 3];
    for (i, c) in palette.iter_mut().enumerate() {
        *c = u32::from_le_bytes(image.at(0x5336b0 + i as u32 * 4, 4)?.try_into().unwrap());
    }
    nodes.iter()
        .zip(&auxiliaries.slots()[..count])
        .map(|(resident, auxiliary)| {
            let node = resident.as_bytes();
            let visible = auxiliary[0xb8] & 1 != 0;
            let mut shopping = 0xffffffff;
            if visible && node[0x54] != 0xff {
                let building = i16::from_le_bytes(node[0x56..0x58].try_into().unwrap());
                shopping = runtime_shopping_color(building, buildings, businesses, players, image)?;
            }
            let position = std::array::from_fn(|i| {
                f32::from_le_bytes(auxiliary[4 + i * 4..8 + i * 4].try_into().unwrap())
            });
            if visible && position.iter().any(|v| !v.is_finite()) {
                return Err("fresh marker nonfinite position".into());
            }
            Ok(Marker {
                position,
                visible,
                colors: colors(node, shopping, palette),
            })
        })
        .collect()
}
/// Original460390 ->4b761b: ownership comes from the targeted building's
/// business, and its character is read from the current runtime player+150.
/// Player order and character order are independent (including human choice).
fn runtime_shopping_color(
    building: i16,
    buildings: &[[u8; 0x394]],
    businesses: &[[u8; 0x220]],
    players: &openmt2001::player_manager::Manager,
    image: &PeImage<'_>,
) -> Result<u32, String> {
    let row = buildings
        .get(usize::try_from(building).map_err(|_| "fresh marker negative building")?)
        .ok_or("fresh marker building outside pool")?;
    let business = i32::from_le_bytes(row[0x1c..0x20].try_into().unwrap());
    if business == -1 { return Ok(0xffffffff); }
    let row = businesses
        .get(usize::try_from(business).map_err(|_| "fresh marker negative business")?)
        .ok_or("fresh marker business outside pool")?;
    let owner = row[10] as i8;
    if owner == -1 { return Ok(0xffffffff); }
    if owner == 0 { return Ok(0xff888888); }
    let player = players.players
        .get(usize::try_from(i16::from(owner) - 1).map_err(|_| "fresh marker negative owner")?)
        .ok_or("fresh marker owner outside roster")?;
    let character = u32::from_le_bytes(player.bytes.get(0x150..0x154)
        .ok_or("fresh marker player character missing")?.try_into().unwrap());
    if character >= 10 { return Err("fresh marker character outside palette".into()); }
    Ok(u32::from_le_bytes(image.at(0x541af0 + character * 40, 4)?.try_into().unwrap()))
}
pub fn quad(point: [f32; 2]) -> [[f32; 2]; 4] {
    let left = f64::from(point[0]) - 2.5;
    let bottom = (f64::from(point[1]) + 2.5) as f32;
    let right = (left + 4.5) as f32;
    let top = (f64::from(bottom) - 4.5) as f32;
    [
        [left as f32, bottom],
        [right, bottom],
        [right, top],
        [left as f32, top],
    ]
}
pub fn step(current: usize, right: bool) -> usize {
    (current + if right { 1 } else { 3 }) % 4
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original divergent marker iteration fixture"]
    fn matches_native_positional_marker_iteration() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_LIVE_PEOPLE_ITERATION")
            .unwrap()).unwrap();
        let mut offset = 0;
        let mut take = |n: usize| { let start = offset; offset += n; &bytes[start..offset] };
        assert_eq!(take(8), b"MTLVIT01");
        let count = u32::from_le_bytes(take(4).try_into().unwrap());
        assert_eq!(u32::from_le_bytes(take(4).try_into().unwrap()), 0x037f);
        let players = openmt2001::player_manager::Manager {
            bytes: [0; 0x28], players: vec![],
        };
        for _ in 0..count {
            let mut residents = openmt2001::economy::ResidentAccounts::new_pool();
            residents.active_count = u32::from_le_bytes(take(4).try_into().unwrap());
            let used = u32::from_le_bytes(take(4).try_into().unwrap()) as usize;
            let mut auxiliaries = openmt2001::resident::AuxiliaryPool::for_residents();
            for node in residents.nodes.iter_mut().take(used) {
                let prefix: [u8; 330] = take(330).try_into().unwrap();
                let reference = u32::from_le_bytes(prefix[0xf0..0xf4].try_into().unwrap());
                *node = openmt2001::resident::Resident::from_runtime_bytes(prefix, 1, reference);
                auxiliaries.allocate(take(284).try_into().unwrap()).unwrap();
            }
            let markers = from_runtime_pools(&residents, &auxiliaries, &[], &[], &image,
                &players).unwrap();
            assert_eq!(markers.len(), used);
            // Native report4112a0 counts active residents separately from draw4b7380.
            assert_eq!(crate::city_people::Report::from_nodes(
                residents.nodes[..residents.active_count as usize].iter().map(|n| n.as_bytes()),
                &image).unwrap().rows[0].value, residents.active_count as i32);
            for (slot, marker) in markers.iter().enumerate() {
                assert_eq!(marker.position, [10. + 100. * slot as f32, 2., 30.]);
            }
            for expected_mode in [0, 4, 3, 1, 2] {
                let mode = u32::from_le_bytes(take(4).try_into().unwrap());
                assert_eq!(mode, expected_mode);
                let drawn = u32::from_le_bytes(take(4).try_into().unwrap()) as usize;
                let expected: Vec<_> = (0..drawn).map(|_| u32::from_le_bytes(
                    take(4).try_into().unwrap())).collect();
                let actual: Vec<_> = markers.iter().filter(|m| m.visible).map(|m| {
                    if mode == 0 { 0xffff_ffff }
                    else { m.colors[MODES.iter().position(|&m| m == mode).unwrap()] }
                }).collect();
                assert_eq!(actual, expected, "active{} aux{used} mode{mode}", residents.active_count);
            }
        }
        assert_eq!(offset, bytes.len());
    }
    #[test]
    #[ignore = "requires original live resident marker ownership fixture"]
    fn matches_native_live_shopping_markers() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let fixture = std::fs::read(std::env::var("OPENMT2001_NATIVE_LIVE_PEOPLE_MARKERS")
            .unwrap()).unwrap();
        assert_eq!(&fixture[..8], b"MTLVMK01");
        let word = |at| u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
        assert_eq!(word(12), 0x037f);
        assert_eq!(fixture.len(), 16 + word(8) as usize * 16);
        let mut buildings = vec![[0; 0x394]; 600];
        let mut businesses = vec![[0; 0x220]; 600];
        let mut players = openmt2001::player_manager::Manager {
            bytes: [0; 0x28],
            players: (0..6).map(|slot| openmt2001::player_manager::Player {
                reference: 0x100c0000 + slot * 0x300,
                bytes: vec![0; 0x1e50], auxiliaries: vec![],
            }).collect(),
        };
        for at in (16..fixture.len()).step_by(16) {
            let owner = word(at) as i32;
            let character = word(at + 4);
            let business = word(at + 8) as i32;
            for (slot, player) in players.players.iter_mut().enumerate() {
                let selected = if slot as i32 == owner - 1 { character }
                    else { (character + 1) % 10 };
                player.bytes[0x150..0x154].copy_from_slice(&selected.to_le_bytes());
            }
            buildings[17][0x1c..0x20].copy_from_slice(&business.to_le_bytes());
            businesses[23][10] = owner as u8;
            assert_eq!(runtime_shopping_color(17, &buildings, &businesses, &players,
                &image).unwrap(), word(at + 12),
                "owner{owner} character{character} business{business}");
        }
    }
    #[test]
    #[ignore = "private original strategic resident fixture"]
    fn matches_native_people_markers() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_PEOPLE_MARKERS").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTPEOMK1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        assert_eq!(word(&mut at), 2);
        for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let raw = std::fs::read(std::env::var(key).unwrap()).unwrap();
            let rows = from_save(&SaveFile::parse(&raw).unwrap(), &image).unwrap();
            at += 32;
            assert_eq!(word(&mut at) as usize, rows.len());
            for row in &rows {
                for v in row.position {
                    assert_eq!(word(&mut at), v.to_bits());
                }
                assert_eq!(b[at] != 0, row.visible);
                at += 1;
            }
            for (i, mode) in MODES.into_iter().enumerate() {
                assert_eq!(word(&mut at), mode);
                assert_eq!(
                    word(&mut at) as usize,
                    rows.iter().filter(|r| r.visible).count()
                );
                for row in rows.iter().filter(|r| r.visible) {
                    let p = [
                        (row.position[0] + 640.) * 0.5,
                        (row.position[2] + 480.) * 0.5,
                    ];
                    for q in quad(p) {
                        for v in q {
                            assert_eq!(word(&mut at), v.to_bits());
                        }
                        assert_eq!(word(&mut at), 0.5f32.to_bits());
                    }
                    assert_eq!(word(&mut at), row.colors[i]);
                    for v in [0f32, 1., 0., 0., 1., 0., 1., 1.] {
                        assert_eq!(word(&mut at), v.to_bits());
                    }
                }
            }
        }
        assert_eq!(word(&mut at), 8);
        for _ in 0..8 {
            let prior = word(&mut at) as usize;
            let right = word(&mut at) != 0;
            let selected = step(prior, right);
            assert_eq!(word(&mut at) as usize, selected);
            assert_eq!(word(&mut at), MODES[selected]);
            assert_eq!(word(&mut at), LABELS[selected]);
        }
        assert_eq!(at, b.len());
    }
}
