//! Original48ad20 descriptor and451458 local demolition consumer.
/// Original445520 quote, retaining the extended result until its caller stores.
pub fn quote(
    building: &[u8; 916],
    categories: &[[u8; 380]; 52],
    image: &crate::pe::PeImage<'_>,
) -> Result<crate::x87::Sum, String> {
    let kind = crate::player_manager::word(building, 0x24);
    let at = 0x5365f4u32.wrapping_add(kind.wrapping_mul(46));
    let cost = u16::from_le_bytes(image.at(at, 2)?.try_into().unwrap());
    quote_word(building, categories, cost, image)
}
/// Quote using the configured Session resource. The command retains its quote.
pub fn quote_with_parameters(
    building: &[u8; 916],
    categories: &[[u8; 380]; 52],
    parameters: &crate::building_parameters::BuildingKinds,
    image: &crate::pe::PeImage<'_>,
) -> Result<crate::x87::Sum, String> {
    let cost = parameters.row(crate::player_manager::word(building, 0x24))?[2];
    quote_word(building, categories, cost, image)
}
fn quote_word(
    building: &[u8; 916],
    categories: &[[u8; 380]; 52],
    cost: u16,
    image: &crate::pe::PeImage<'_>,
) -> Result<crate::x87::Sum, String> {
    use crate::{player_manager::word, x87::Sum};
    let offset = word(building, 0x40)
        .checked_sub(0x617628)
        .ok_or("demolition category pointer before pool")?;
    if !offset.is_multiple_of(380) {
        return Err("demolition category pointer unaligned".into());
    }
    let category = categories
        .get((offset / 380) as usize)
        .ok_or("demolition category pointer outside pool")?;
    let category = categories
        .get(word(category, 0) as usize)
        .ok_or("demolition category identity outside pool")?;
    let volume = (i32::from(building[7]) - i32::from(building[5]) + 1)
        .wrapping_mul(i32::from(building[6]) - i32::from(building[4]) + 1)
        .wrapping_mul(i32::from(building[8]));
    let mut value = Sum::from_f32(f32::from_bits(word(category, 0x84)))?;
    value.mul_f32(image.f32_at(0x52502c)?)?;
    value.add_f32(image.f32_at(0x525034)?)?;
    value.mul(Sum::from_i32(i32::from(cost)));
    value.mul(Sum::from_i32(volume));
    Ok(value)
}
#[derive(Clone, Debug)]
pub struct Command {
    pub category: i8,
    pub building: u16,
    pub amount: f32,
}
impl Command {
    pub fn packet(&self) -> [u8; 0x1c] {
        let mut bytes = [0; 0x1c];
        bytes[..4].copy_from_slice(&0x525ddcu32.to_le_bytes());
        bytes[4..8].copy_from_slice(&0x10u32.to_le_bytes());
        bytes[0x10] = 1;
        bytes[0x14] = self.category as u8;
        bytes[0x16..0x18].copy_from_slice(&self.building.to_le_bytes());
        bytes[0x18..0x1c].copy_from_slice(&self.amount.to_le_bytes());
        bytes
    }
    pub fn from_packet(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 0x1c || bytes[4..8] != 0x10u32.to_le_bytes() {
            return Err("demolition requires command10 descriptor".into());
        }
        Ok(Self {
            category: bytes[0x14] as i8,
            building: u16::from_le_bytes(bytes[0x16..0x18].try_into().unwrap()),
            amount: f32::from_le_bytes(bytes[0x18..0x1c].try_into().unwrap()),
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    Post {
        owner: i8,
        amount: f32,
        ledger: usize,
    },
    Detach {
        business: i32,
    },
    Publish,
    RefreshInterface,
    RefreshBuilding {
        building: u16,
    },
}
pub struct World<'a> {
    pub buildings: &'a mut [[u8; 0x394]],
    pub businesses: &'a mut [[u8; 0x220]],
    pub categories: &'a mut [[u8; 0x17c]; 52],
    pub pending: &'a mut crate::pending_world::State,
}
pub fn apply(
    command: &Command,
    world: &mut World<'_>,
    mut callback: impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let building = usize::from(command.building);
    let row = world
        .buildings
        .get(building)
        .ok_or("demolition building leaves pool")?;
    let business = u32::from_le_bytes(row[0x1c..0x20].try_into().unwrap());
    if business != u32::MAX {
        let row = world
            .businesses
            .get(business as usize)
            .ok_or("demolition business leaves pool")?;
        let owner = row[0xa] as i8;
        let identity = i32::from_le_bytes(row[0x210..0x214].try_into().unwrap());
        if owner > 0 {
            callback(
                Call::Post {
                    owner,
                    amount: -command.amount,
                    ledger: 1,
                },
                world,
            )?;
        }
        callback(Call::Detach { business: identity }, world)?;
    }
    let category = world
        .categories
        .get(command.category as usize)
        .ok_or("demolition category leaves city")?;
    let identity = u32::from_le_bytes(category[..4].try_into().unwrap());
    if world
        .pending
        .demolish(identity, &mut world.buildings[building])?
    {
        callback(Call::Publish, world)?;
    }
    callback(Call::RefreshInterface, world)?;
    callback(
        Call::RefreshBuilding {
            building: command.building,
        },
        world,
    )
}
