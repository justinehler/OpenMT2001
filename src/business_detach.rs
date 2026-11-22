//! Original463960. Clear the building link before evicting residents: the
//! original45cd10 then skips4618c0, preserving their housing chain links.
use crate::business_binding::{Player, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    RemoveOwnerCount { category: u32, business_type: u32 },
    RefreshBuilding { building: usize },
    EvictResident { resident: usize },
    NotifyBusiness { business: usize },
    RefreshCategory { category: usize },
    TransferOwner { business: usize, owner: u8 },
}
fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}
fn building(world: &World<'_>, business: usize) -> Result<usize, String> {
    let row = world
        .businesses
        .get(business)
        .ok_or("detach business leaves pool")?;
    let index = i16::from_le_bytes(row[8..10].try_into().unwrap()) as usize;
    world
        .buildings
        .get(index)
        .ok_or("detach building leaves pool")?;
    Ok(index)
}
fn category(world: &World<'_>, building: usize) -> Result<usize, String> {
    let offset = word(&world.buildings[building], 0x40)
        .checked_sub(world.category_base)
        .ok_or("detach category precedes arena")?;
    if !offset.is_multiple_of(0x17c) || offset / 0x17c >= 52 {
        return Err("detach category leaves arena".into());
    }
    Ok(offset as usize / 0x17c)
}
pub fn apply(
    index: i32,
    world: &mut World<'_>,
    residents: &mut [crate::resident::Resident],
    notifications: bool,
    landscape_value: f32,
    mut callback: impl FnMut(
        Call,
        &mut World<'_>,
        &mut [crate::resident::Resident],
    ) -> Result<(), String>,
) -> Result<(), String> {
    if index == -1 {
        return Ok(());
    }
    let index = usize::try_from(index).map_err(|_| "negative detach business")?;
    let linked = building(world, index)?;
    let cat = category(world, linked)?;
    let owner = world.businesses[index][0xa] as i8;
    if owner != 0
        && let Some(Some(Player { kind, .. })) = world.players.get((i32::from(owner) - 1) as usize)
        && *kind != 1
    {
        callback(
            Call::RemoveOwnerCount {
                category: word(&world.categories[cat], 0),
                business_type: word(&world.businesses[index], 4),
            },
            world,
            residents,
        )?;
    }
    if word(&world.businesses[index], 0) == 2 {
        let identity = i16::from_le_bytes(world.categories[cat][..2].try_into().unwrap()) as usize;
        let target = world
            .categories
            .get(identity)
            .ok_or("landscape category identity leaves table")?;
        let target = word(target, 0) as usize;
        let row = &world.buildings[linked];
        let volume = (i32::from(row[7]) - i32::from(row[5]) + 1)
            .wrapping_mul(i32::from(row[6]) - i32::from(row[4]) + 1)
            .wrapping_mul(i32::from(row[8]));
        let target = world
            .categories
            .get_mut(target)
            .ok_or("landscape value category leaves table")?;
        let value = f32::from_bits(word(target, 0x84));
        let next = (f64::from(value) - f64::from(volume) * f64::from(landscape_value)) as f32;
        target[0x84..0x88].copy_from_slice(&next.to_le_bytes());
    }
    world.buildings[linked][0x1c..0x20].copy_from_slice(&u32::MAX.to_le_bytes());
    callback(Call::RefreshBuilding { building: linked }, world, residents)?;
    if word(&world.businesses[index], 0) == 1 {
        let row = &world.businesses[index];
        let occupancy = u16::from_le_bytes(row[0x10..0x12].try_into().unwrap());
        let capacity = u16::from_le_bytes(row[0xe..0x10].try_into().unwrap());
        if occupancy < capacity
            && let Some(at) = world.available.iter().position(|&v| v == index)
        {
            world.available.remove(at);
            //4633c0 leaves a deleted iterator pointer dangling. The host
            // invalidates that pointer; surviving node identities are retained.
            if let Some(cursor) = *world.availability_cursor {
                *world.availability_cursor = if cursor > at {
                    Some(cursor - 1)
                } else if cursor == at {
                    None
                } else {
                    Some(cursor)
                };
            }
        }
        let mut next = word(&world.businesses[index], 0x20c);
        let mut visited = 0;
        while next != u32::MAX {
            if next as usize >= residents.len() || visited >= residents.len() {
                return Err("invalid or cyclic detached resident chain".into());
            }
            callback(
                Call::EvictResident {
                    resident: next as usize,
                },
                world,
                residents,
            )?;
            next = residents[next as usize].next_in_housing() as u32;
            visited += 1;
        }
    }
    if notifications {
        callback(Call::NotifyBusiness { business: index }, world, residents)?;
    }
    let cat = category(world, building(world, index)?)?;
    let identity = i16::from_le_bytes(world.categories[cat][..2].try_into().unwrap()) as usize;
    callback(
        Call::RefreshCategory { category: identity },
        world,
        residents,
    )?;
    callback(
        Call::TransferOwner {
            business: index,
            owner: 0xff,
        },
        world,
        residents,
    )?;
    crate::business_pool::reset_record(&mut world.businesses[index], index as u32);
    Ok(())
}
