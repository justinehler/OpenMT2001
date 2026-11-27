//! Original00463560 orchestration over authoritative live world records.
//! Downstream accounting, resident, stock and presentation calls are required
//! callbacks until their owning runtime is supplied; none are silently omitted.
use crate::{building_pool, business_pool::BusinessCatalog, pe::PeImage};

#[derive(Clone, Copy, Debug)]
pub struct Player {
    /// Opaque identity corresponding to the original player pointer.
    pub identity: u32,
    /// Original player+154;1 suppresses the final owner notification.
    pub kind: u32,
}

pub struct World<'a> {
    pub buildings: &'a mut [[u8; 0x394]],
    pub businesses: &'a mut [[u8; 0x220]],
    pub categories: &'a mut [[u8; 0x17c]; 52],
    pub category_base: u32,
    pub catalog: &'a BusinessCatalog,
    /// Native linked-list order, shared with the housing simulation.
    pub available: &'a mut Vec<usize>,
    /// Logical list position, reset to the head on an eligible append.
    pub availability_cursor: &'a mut Option<usize>,
    /// One-based original owner bytes resolve through these player slots.
    pub players: &'a [Option<Player>],
    pub resident_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    DetachBusiness { business: usize },             //463960
    DebitBusiness { business: usize, amount: f32 }, //4629d0(amount,1,0)
    CreditPlayer { identity: u32, amount: f32 },    //48a5a0(amount,0x22)
    ChargeUtilities { business: usize },            //462070(0)
    PayUtilityOwners,                               //48dfc0
    InvalidateResidentCategory { resident: usize, category: i16 }, //45cb70
    RefreshCategory { category: usize },            //447870
    InitializeStock { business: usize, argument: u32 }, //461180
    RefreshBuilding { building: usize },            //4450a0
    NotifyBusiness { business: usize },             //471a20(business,1)
    NotifyOwner { category: u32, business_type: u32 }, //4725f0
}

pub struct Inputs {
    pub business: usize,
    pub building: usize,
    pub stock_argument: u32,
    pub hour: i32,
    pub discount: f32,
    /// Original801dec gates471a20 only.
    pub notifications: bool,
}

/// Supply the recovered simulation callbacks directly from live state. The
/// remaining ownership/accounting/model/UI operations stay with their owners.
/// This joins initialization and category refresh to the binding in its actual
/// call order without copying either world's records.
pub struct Runtime<'a> {
    pub building_base: u32,
    pub products: &'a [[[u32; 2]; 6]; 45],
    pub commodities: &'a [[u8; 0x1c]; 32],
    pub residents: &'a mut [crate::resident::Resident],
    pub timestamp: u32,
}

impl Runtime<'_> {
    pub fn bind(
        &mut self,
        world: &mut World<'_>,
        input: &Inputs,
        image: &PeImage<'_>,
        external: &mut impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
        stock_charge: &mut impl FnMut(&mut [u8; 0x220], f32) -> Result<(), String>,
    ) -> Result<(), String> {
        self.bind_with_residents(
            world,
            input,
            image,
            &mut |call, world, _| external(call, world),
            stock_charge,
        )
    }
    /// The detach boundary needs the same resident pool as initialization.
    pub fn bind_with_residents(
        &mut self,
        world: &mut World<'_>,
        input: &Inputs,
        image: &PeImage<'_>,
        external: &mut impl FnMut(
            Call,
            &mut World<'_>,
            &mut [crate::resident::Resident],
        ) -> Result<(), String>,
        stock_charge: &mut impl FnMut(&mut [u8; 0x220], f32) -> Result<(), String>,
    ) -> Result<(), String> {
        bind(world, input, image, &mut |call, world| match call {
            Call::RefreshCategory { category } => refresh_category(
                world,
                category,
                self.building_base,
                self.products,
                self.commodities,
            ),
            Call::InvalidateResidentCategory { resident, category } => self
                .residents
                .get_mut(resident)
                .ok_or("binding resident count leaves pool")?
                .invalidate_category(category),
            Call::InitializeStock { business, argument } => {
                let building = world.linked_building(business)?;
                let kind = word(&world.businesses[business], 4) as usize;
                let info = world
                    .catalog
                    .row(kind)
                    .ok_or("initializer business type leaves catalog")?;
                // Residential/landscape initialization sums physical stock
                // using the count but never dereferences product identities.
                let count = if word(&world.businesses[business], 0) == 0 {
                    usize::from(info[0x4c])
                } else {
                    0
                };
                let entries = self
                    .products
                    .get(kind)
                    .and_then(|row| row.get(..count))
                    .ok_or("initializer catalog leaves table")?;
                let products = entries
                    .iter()
                    .map(|entry| {
                        let commodity = self
                            .commodities
                            .get(entry[0] as usize)
                            .ok_or("initializer commodity leaves table")?;
                        Ok(crate::business_initialize::Product {
                            value: f32::from_bits(word(commodity, 0x14)),
                            markup: f32::from_bits(word(commodity, 0x18)),
                            fraction: f32::from_bits(entry[1]),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                crate::business_initialize::initialize(
                    &mut world.businesses[business],
                    &mut world.buildings[building],
                    &crate::business_initialize::Inputs {
                        hour: input.hour,
                        timestamp: self.timestamp,
                        initializer: argument as u8,
                        info,
                        products: &products,
                    },
                    image,
                    stock_charge,
                )
            }
            other => external(other, world, self.residents),
        })
    }
}

/// Original447870/4455a0/462ec0. Every linked business contributes its
/// configured catalog, irrespective of active/open/availability flags. Each
/// stored product identity is used as a commodity-table index, whose DWORD0
/// is then compared to the32 queried mask bits.
pub fn refresh_category(
    world: &mut World<'_>,
    category: usize,
    building_base: u32,
    products: &[[[u32; 2]; 6]; 45],
    commodities: &[[u8; 0x1c]; 32],
) -> Result<(), String> {
    let category_row = world
        .categories
        .get_mut(category)
        .ok_or("category refresh leaves table")?;
    category_row[0x70..0x74].fill(0);
    let mut next = word(category_row, 0x15c);
    let mut seen = vec![false; world.buildings.len()];
    while next != 0 {
        let offset = next
            .checked_sub(building_base)
            .ok_or("category building precedes arena")?;
        if offset % 0x394 != 0 {
            return Err("misaligned category building link".into());
        }
        let index = (offset / 0x394) as usize;
        let building = world
            .buildings
            .get(index)
            .ok_or("category building leaves pool")?;
        if seen[index] {
            return Err("cyclic category building chain".into());
        }
        seen[index] = true;
        let business = word(building, 0x1c);
        if business != u32::MAX {
            let business = world
                .businesses
                .get(business as usize)
                .ok_or("category business leaves pool")?;
            let kind = word(business, 4) as usize;
            let info = world
                .catalog
                .row(kind)
                .ok_or("category business type leaves catalog")?;
            let entries = products[kind]
                .get(..usize::from(info[0x4c]))
                .ok_or("category catalog count exceeds six")?;
            let mut mask = word(category_row, 0x70);
            for entry in entries {
                let commodity = commodities
                    .get(entry[0] as usize)
                    .ok_or("category product leaves commodity table")?;
                let identity = word(commodity, 0);
                if identity < 32 {
                    mask |= 1_u32 << identity;
                }
            }
            category_row[0x70..0x74].copy_from_slice(&mask.to_le_bytes());
        }
        next = word(building, 0x44);
    }
    Ok(())
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn short(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}

impl World<'_> {
    /// Original461590: recompute the stored land rent from the current linked
    /// building and district, using the same PC64 calculation as attachment.
    pub fn refresh_rent(
        &mut self,
        business: usize,
        discount: f32,
        image: &PeImage<'_>,
    ) -> Result<u16, String> {
        self.businesses
            .get(business)
            .ok_or("rent refresh business leaves pool")?;
        let linked = self.linked_building(business)?;
        let category = self.category(linked)?;
        // The category's DWORD identity, rather than its slot, selects rate+84.
        let category_id = word(&self.categories[category], 0) as usize;
        let value = self
            .categories
            .get(category_id)
            .ok_or("category identity leaves table")?;
        let rent = crate::building_binding::initial_rent(
            &self.buildings[linked],
            f32::from_bits(word(value, 0x84)),
            discount,
            image,
        )?;
        self.businesses[business][0x18..0x1a].copy_from_slice(&rent.to_le_bytes());
        Ok(rent)
    }

    /// Binding-time00462070(0): assess utility expenses without debiting cash.
    /// Uses the same live building/catalog records as463560 and the shared
    /// utility accumulator; the following48dfc0 payout is a separate boundary.
    pub fn assess_binding_utilities(
        &mut self,
        business: usize,
        utilities: &mut crate::economy::Utilities,
    ) -> Result<(), String> {
        let linked = self.linked_building(business)?;
        let kind = word(&self.businesses[business], 4) as usize;
        let info = self
            .catalog
            .row(kind)
            .ok_or("utility type leaves catalog")?;
        let building = &self.buildings[linked];
        let scalar = (i32::from(building[6]) - i32::from(building[4]) + 1)
            .wrapping_mul(i32::from(building[7]) - i32::from(building[5]) + 1)
            .wrapping_mul(i32::from(building[8]));
        let rates = [0x60, 0x64, 0x68, 0x6c].map(|at| f32::from_bits(word(info, at)));
        let record = &mut self.businesses[business];
        let amounts = utilities.assess(scalar, rates, record[0xa]);
        let offsets = [0x44, 0x38, 0x40, 0x3c];
        let mut expenses = offsets.map(|at| f32::from_bits(word(record, at)));
        let mut account = crate::economy::BusinessAccount {
            cost_balance: f32::from_bits(word(record, 0x30)),
            balance: f32::from_bits(word(record, 0x1c)),
        };
        account.charge_utilities(&mut expenses, amounts, false, None)?;
        for (at, value) in offsets.into_iter().zip(expenses) {
            record[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        Ok(())
    }

    fn linked_building(&self, business: usize) -> Result<usize, String> {
        let index = short(&self.businesses[business], 8) as i16;
        let index = usize::try_from(index).map_err(|_| "binding has negative building link")?;
        self.buildings
            .get(index)
            .ok_or("binding building link leaves pool")?;
        Ok(index)
    }
    fn category(&self, building: usize) -> Result<usize, String> {
        let offset = word(&self.buildings[building], 0x40)
            .checked_sub(self.category_base)
            .ok_or("binding category precedes arena")?;
        if offset % 0x17c != 0 || offset / 0x17c >= 52 {
            return Err("binding category leaves arena".into());
        }
        Ok((offset / 0x17c) as usize)
    }
    fn player(&self, owner: u8) -> Result<Option<Player>, String> {
        if owner == 0 {
            return Ok(None);
        }
        let index = usize::try_from(i32::from(owner as i8) - 1)
            .map_err(|_| "negative player owner index")?;
        self.players
            .get(index)
            .copied()
            .map(|player| player.filter(|p| p.identity != 0))
            .ok_or_else(|| "player owner leaves table".into())
    }
}

/// Execute binding stores and original downstream call order. The callback can
/// mutate the shared world, so subsequent native reads observe those changes.
pub fn bind(
    world: &mut World<'_>,
    input: &Inputs,
    image: &PeImage<'_>,
    call: &mut impl FnMut(Call, &mut World<'_>) -> Result<(), String>,
) -> Result<(), String> {
    let business = input.business;
    let building = input.building;
    world
        .businesses
        .get(business)
        .ok_or("business binding slot leaves pool")?;
    world
        .buildings
        .get(building)
        .ok_or("building binding slot leaves pool")?;
    if building > i16::MAX as usize {
        return Err("building binding exceeds signed native link".into());
    }
    world.businesses[business][8..10].copy_from_slice(&(building as u16).to_le_bytes());
    let previous = word(&world.buildings[building], 0x1c);
    if previous != u32::MAX {
        call(
            Call::DetachBusiness {
                business: previous as usize,
            },
            world,
        )?;
    }
    world.buildings[building][0x1c..0x20].copy_from_slice(&(business as u32).to_le_bytes());
    let linked = world.linked_building(business)?;
    let kind = word(&world.businesses[business], 4) as usize;
    let info = world
        .catalog
        .row(kind)
        .ok_or("binding business type leaves catalog")?;
    let capacity =
        building_pool::capacity(&world.buildings[linked], f32::from_bits(word(info, 0x34)));
    world.businesses[business][0xe..0x10].copy_from_slice(&(capacity as u16).to_le_bytes());
    let category = world.category(linked)?;
    let rent = world.refresh_rent(business, input.discount, image)?;
    if input.hour >= 6 {
        let owner = world.player(world.categories[category][5])?;
        let tenant = world.player(world.businesses[business][0xa])?;
        if owner.map(|p| p.identity).unwrap_or(0) != tenant.map(|p| p.identity).unwrap_or(0) {
            call(
                Call::DebitBusiness {
                    business,
                    amount: -f32::from(rent),
                },
                world,
            )?;
            if short(&world.businesses[business], 8) != u16::MAX
                && let Some(owner) = owner
            {
                call(
                    Call::CreditPlayer {
                        identity: owner.identity,
                        amount: f32::from(short(&world.businesses[business], 0x18)),
                    },
                    world,
                )?;
            }
        }
        call(Call::ChargeUtilities { business }, world)?;
        call(Call::PayUtilityOwners, world)?;
    }
    if short(&world.businesses[business], 0x10) < short(&world.businesses[business], 0xe)
        && word(&world.businesses[business], 0) == 1
    {
        if let Some(index) = world.available.iter().position(|slot| *slot == business) {
            world.available.remove(index);
        }
        world.available.push(business);
        *world.availability_cursor = Some(0);
    }
    let linked = world.linked_building(business)?;
    let category = world.category(linked)?;
    let category_short = short(&world.categories[category], 0) as i16;
    world.businesses[business][0x78..0x7c].copy_from_slice(&4_u32.to_le_bytes());
    world.businesses[business][0x74] = 0;
    world.businesses[business][0x7c..0x80].fill(0);
    let mut resident = 0;
    while resident < world.resident_count {
        call(
            Call::InvalidateResidentCategory {
                resident,
                category: category_short,
            },
            world,
        )?;
        resident += 1;
    }
    let linked = world.linked_building(business)?;
    let category = world.category(linked)?;
    let category_id = short(&world.categories[category], 0) as i16;
    let category_id = usize::try_from(category_id).map_err(|_| "negative category identity")?;
    if category_id >= 52 {
        return Err("category identity leaves table".into());
    }
    call(
        Call::RefreshCategory {
            category: category_id,
        },
        world,
    )?;
    call(
        Call::InitializeStock {
            business,
            argument: input.stock_argument,
        },
        world,
    )?;
    call(Call::RefreshBuilding { building }, world)?;
    if input.notifications {
        call(Call::NotifyBusiness { business }, world)?;
    }
    let linked = world.linked_building(business)?;
    let category = world.category(linked)?;
    let category_id = short(&world.categories[category], 0) as i16;
    let category_id = usize::try_from(category_id).map_err(|_| "negative category identity")?;
    let identity = word(
        world
            .categories
            .get(category_id)
            .ok_or("category identity leaves table")?,
        0,
    );
    if word(&world.businesses[business], 0) == 2 {
        let b = &world.buildings[building];
        let volume = (i32::from(b[7]) - i32::from(b[5]) + 1)
            .wrapping_mul(i32::from(b[6]) - i32::from(b[4]) + 1)
            .wrapping_mul(i32::from(b[8]));
        let target = world
            .categories
            .get_mut(identity as usize)
            .ok_or("landscape identity leaves table")?;
        let value = (f64::from(volume) * f64::from(image.f32_at(0x52508c)?)
            + f64::from(f32::from_bits(word(target, 0x84)))) as f32;
        target[0x84..0x88].copy_from_slice(&value.to_le_bytes());
    }
    if let Some(player) = world.player(world.businesses[business][0xa])?
        && player.kind != 1
    {
        let category = world.category(building)?;
        call(
            Call::NotifyOwner {
                category: word(&world.categories[category], 0),
                business_type: word(&world.businesses[business], 4),
            },
            world,
        )?;
    }
    world.buildings[building][0x14..0x18].copy_from_slice(&4_u32.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/building_image.rs"
    ));

    #[test]
    fn attached_and_refreshed_rent_drives_daily_tenant_and_landlord_transfers() {
        use crate::economy::{
            self, BusinessDayParameters, PlayerAccount, ResidentAccounts, Utilities,
        };

        let bytes = fixture();
        let image = PeImage::parse(&bytes).unwrap();
        for (rate, expected) in [(10i32, 60u16), (5, 30)] {
            let mut buildings = [[0; 0x394]];
            buildings[0][6..9].copy_from_slice(&[2, 2, 1]);
            buildings[0][0x1c..0x20].copy_from_slice(&u32::MAX.to_le_bytes());
            buildings[0][0x28..0x2c].copy_from_slice(&rate.to_le_bytes());
            buildings[0][0x40..0x44].copy_from_slice(&0x617628u32.to_le_bytes());
            let mut businesses = vec![[0; 0x220]; 600];
            businesses[0][0xa] = 1;
            businesses[0][0x21c] = 1;
            let mut categories = [[0; 0x17c]; 52];
            categories[0][5] = 2;
            let catalog = BusinessCatalog::from_rows([[0; 0x74]; 45]);
            let mut available = vec![];
            let mut cursor = None;
            let identities = [1, 2].map(|identity| Some(Player { identity, kind: 1 }));
            let mut world = World {
                buildings: &mut buildings,
                businesses: &mut businesses,
                categories: &mut categories,
                category_base: 0x617628,
                catalog: &catalog,
                available: &mut available,
                availability_cursor: &mut cursor,
                players: &identities,
                resident_count: 0,
            };
            bind(
                &mut world,
                &Inputs {
                    business: 0,
                    building: 0,
                    stock_argument: 0,
                    hour: 0,
                    discount: 0.5,
                    notifications: false,
                },
                &image,
                &mut |call, _| {
                    assert!(matches!(
                        call,
                        Call::RefreshCategory { .. }
                            | Call::InitializeStock { .. }
                            | Call::RefreshBuilding { .. }
                    ));
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(short(&world.businesses[0], 0x18), expected);

            // Refresh must read the current rate/district and overwrite stale rent.
            world.buildings[0][0x28..0x2c].copy_from_slice(&(rate * 2).to_le_bytes());
            assert_eq!(world.refresh_rent(0, 0.5, &image).unwrap(), expected * 2);
            world.buildings[0][0x28..0x2c].copy_from_slice(&rate.to_le_bytes());
            world.categories[0][0x84..0x88].copy_from_slice(&2f32.to_le_bytes());
            assert_eq!(world.refresh_rent(0, 0.5, &image).unwrap(), expected * 2);
            world.categories[0][0x84..0x88].fill(0);
            world.refresh_rent(0, 0.5, &image).unwrap();
            assert_eq!(short(&world.businesses[0], 0x18), expected);

            let mut players = [0; 2].map(|_| PlayerAccount::from_runtime(&[0; 0x1b8]).unwrap());
            let mut parameters: Vec<_> = (0..600).map(|_| None).collect();
            parameters[0] = Some(BusinessDayParameters {
                capacity: 0,
                catalog_mask: 0,
                prices: vec![],
                utility_scalar: 9,
                utility_rates: [0.; 4],
                landlord: 2,
            });
            economy::update_daily_businesses(
                world.businesses,
                &parameters,
                &mut ResidentAccounts::new_pool(),
                &mut players,
                &mut Utilities::new([0; 4]),
            )
            .unwrap();
            let rent = f32::from(expected);
            assert_eq!(players[0].cash, -rent);
            assert_eq!(players[0].ledger(0x0e), Some(-rent));
            assert_eq!(players[1].cash, rent);
            assert_eq!(players[1].ledger(0x22), Some(rent));
            assert_eq!(f32::from_bits(word(&world.businesses[0], 0x28)), -rent);
            assert_eq!(f32::from_bits(word(&world.businesses[0], 0x1c)), -rent);
        }
    }

    #[test]
    #[ignore = "requires original binding utility observations"]
    fn matches_native_binding_utilities() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BINDING_UTILITIES")).unwrap();
        assert_eq!(&bytes[..8], b"MTBUTIL1");
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let mut at = 12;
        for case in 0..count {
            let mut buildings = vec![[0; 0x394]; 3];
            buildings[2].copy_from_slice(&bytes[at..at + 0x394]);
            at += 0x394;
            let mut rows = [[0; 0x74]; 45];
            rows[7].copy_from_slice(&bytes[at..at + 0x74]);
            at += 0x74;
            let catalog = super::BusinessCatalog::from_rows(rows);
            let mut businesses = [[0; 0x220]; 1];
            businesses[0].copy_from_slice(&bytes[at..at + 0x220]);
            at += 0x220;
            let mut utilities =
                crate::economy::Utilities::new(bytes[at..at + 4].try_into().unwrap());
            utilities.enabled = std::array::from_fn(|i| bytes[at + 4 + i] != 0);
            at += 8;
            utilities.totals = std::array::from_fn(|i| {
                f32::from_le_bytes(bytes[at + i * 4..at + i * 4 + 4].try_into().unwrap())
            });
            at += 16;
            let mut categories = [[0; 0x17c]; 52];
            let mut available = vec![];
            let mut cursor = None;
            let mut world = super::World {
                buildings: &mut buildings,
                businesses: &mut businesses,
                categories: &mut categories,
                category_base: 0x617628,
                catalog: &catalog,
                available: &mut available,
                availability_cursor: &mut cursor,
                players: &[],
                resident_count: 0,
            };
            world.assess_binding_utilities(0, &mut utilities).unwrap();
            assert_eq!(
                businesses[0].as_slice(),
                &bytes[at..at + 0x220],
                "case {case} complete record"
            );
            at += 0x220;
            for value in utilities.totals {
                assert_eq!(
                    value.to_le_bytes(),
                    bytes[at..at + 4],
                    "case {case} accumulator"
                );
                at += 4;
            }
        }
        assert_eq!(at, bytes.len());
    }
    use super::*;
    struct Reader<'a> {
        data: &'a [u8],
        at: usize,
    }
    impl Reader<'_> {
        fn bytes<const N: usize>(&mut self) -> [u8; N] {
            let value = self.data[self.at..self.at + N].try_into().unwrap();
            self.at += N;
            value
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.bytes())
        }
    }

    #[test]
    #[ignore = "requires original live business binding fixture and executable"]
    fn matches_native_live_business_binding() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let data = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_BINDING_LIVE")).unwrap();
        let mut reader = Reader { data: &data, at: 0 };
        assert_eq!(reader.bytes::<8>(), *b"MTBINDL1");
        assert_eq!(reader.word(), 1);
        for case in 0..reader.word() {
            let controls: [u32; 10] = std::array::from_fn(|_| reader.word());
            let trace: Vec<_> = (0..reader.word()).map(|_| reader.word()).collect();
            let before_list: [u32; 4] = std::array::from_fn(|_| reader.word());
            let after_list: [u32; 4] = std::array::from_fn(|_| reader.word());
            assert_eq!(before_list, [0; 4]);
            let catalog = BusinessCatalog::from_rows(std::array::from_fn(|_| reader.bytes()));
            let products =
                std::array::from_fn(|_| std::array::from_fn(|_| [reader.word(), reader.word()]));
            let commodities = std::array::from_fn(|_| reader.bytes());
            let timestamp = reader.word();
            let mut businesses = [reader.bytes::<0x220>()];
            let expected_business = reader.bytes::<0x220>();
            let mut buildings = [reader.bytes::<0x394>()];
            let expected_building = reader.bytes::<0x394>();
            let mut categories = [[0; 0x17c]; 52];
            categories[0] = reader.bytes();
            let expected_category = reader.bytes::<0x17c>();
            let resident_before = reader.bytes::<0x150>();
            let resident_after = reader.bytes::<0x150>();
            assert_eq!(resident_before[0x14a..], resident_after[0x14a..]);
            let mut residents = [crate::resident::Resident::from_bytes(
                resident_before[..0x14a].try_into().unwrap(),
            )];
            let before_nodes: [[u8; 12]; 4] = std::array::from_fn(|_| reader.bytes());
            let after_nodes: [[u8; 12]; 4] = std::array::from_fn(|_| reader.bytes());
            assert_eq!(before_nodes, [[0; 12]; 4]);
            let mut available = vec![];
            let mut cursor = None;
            let mut world = World {
                buildings: &mut buildings,
                businesses: &mut businesses,
                categories: &mut categories,
                category_base: 0x617628,
                catalog: &catalog,
                available: &mut available,
                availability_cursor: &mut cursor,
                players: &[None; 6],
                resident_count: 1,
            };
            let mut runtime = Runtime {
                building_base: 0x591318,
                products: &products,
                commodities: &commodities,
                residents: &mut residents,
                timestamp,
            };
            let mut calls = vec![];
            runtime
                .bind(
                    &mut world,
                    &Inputs {
                        business: 0,
                        building: 0,
                        stock_argument: controls[3],
                        hour: controls[2] as i32,
                        discount: f32::from_bits(controls[8]),
                        notifications: false,
                    },
                    &image,
                    &mut |call, _| {
                        assert_eq!(call, Call::RefreshBuilding { building: 0 });
                        calls.push(0x4450a0);
                        Ok(())
                    },
                    &mut |_, _| panic!("unexpected startup stock charge"),
                )
                .unwrap();
            assert_eq!(
                calls,
                trace
                    .into_iter()
                    .filter(|&a| a == 0x4450a0)
                    .collect::<Vec<_>>()
            );
            for (name, actual, expected) in [
                (
                    "business",
                    world.businesses[0].as_slice(),
                    expected_business.as_slice(),
                ),
                (
                    "building",
                    world.buildings[0].as_slice(),
                    expected_building.as_slice(),
                ),
                (
                    "category",
                    world.categories[0].as_slice(),
                    expected_category.as_slice(),
                ),
                (
                    "resident",
                    residents[0].as_bytes().as_slice(),
                    &resident_after[..0x14a],
                ),
            ] {
                let differences: Vec<_> = actual
                    .iter()
                    .zip(expected)
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .collect();
                assert!(
                    differences.is_empty(),
                    "{name} case {case}: {differences:?}"
                );
            }
            assert_eq!(world.available.len(), after_list[3] as usize);
            if after_list[3] == 0 {
                assert_eq!(*world.availability_cursor, None);
                assert_eq!(after_nodes, [[0; 12]; 4]);
            } else {
                assert_eq!(world.available.as_slice(), &[0]);
                assert_eq!(*world.availability_cursor, Some(0));
                assert_eq!(after_list[0], after_list[1]);
                assert_eq!(after_list[0], after_list[2]);
                assert_eq!(word(&after_nodes[0], 0), 0x7a41a8);
                assert_eq!(&after_nodes[0][4..], &[0; 8]);
            }
        }
        assert_eq!(reader.at, data.len());
    }

    #[test]
    #[ignore = "requires original business binding fixture and executable"]
    fn matches_native_business_building_binding() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let data = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_BINDING")).unwrap();
        let mut reader = Reader { data: &data, at: 0 };
        assert_eq!(reader.bytes::<8>(), *b"MTBIND01");
        assert_eq!(reader.word(), 1);
        for case in 0..reader.word() {
            let controls: [u32; 7] = std::array::from_fn(|_| reader.word());
            let floats: [f32; 3] = std::array::from_fn(|_| f32::from_bits(reader.word()));
            let trace: [u32; 32] = std::array::from_fn(|_| reader.word());
            let before_list: [u32; 4] = std::array::from_fn(|_| reader.word());
            let after_list: [u32; 4] = std::array::from_fn(|_| reader.word());
            assert_eq!(before_list, [0; 4]);
            let mut businesses = [reader.bytes::<0x220>()];
            let expected_business = reader.bytes::<0x220>();
            let mut buildings = [reader.bytes::<0x394>()];
            let expected_building = reader.bytes::<0x394>();
            let mut categories = [[0; 0x17c]; 52];
            categories[0] = reader.bytes();
            let expected_category = reader.bytes::<0x17c>();
            let before_nodes: [[u8; 12]; 4] = std::array::from_fn(|_| reader.bytes());
            let after_nodes: [[u8; 12]; 4] = std::array::from_fn(|_| reader.bytes());
            assert_eq!(before_nodes, [[0; 12]; 4]);
            let mut catalog = BusinessCatalog::from_pe(&image).unwrap();
            catalog.row_mut(0).unwrap()[0x34..0x38].copy_from_slice(&floats[0].to_le_bytes());
            let mut available = vec![];
            let mut cursor = None;
            let mut world = World {
                buildings: &mut buildings,
                businesses: &mut businesses,
                categories: &mut categories,
                category_base: 0x617628,
                catalog: &catalog,
                available: &mut available,
                availability_cursor: &mut cursor,
                players: &[None; 6],
                resident_count: 0,
            };
            let input = Inputs {
                business: controls[0] as usize,
                building: controls[1] as usize,
                stock_argument: controls[2],
                hour: controls[3] as i32,
                discount: floats[1],
                notifications: false,
            };
            let mut calls = vec![];
            bind(&mut world, &input, &image, &mut |call, _| {
                calls.push(match call {
                    Call::RefreshCategory { category: 0 } => 0x447870,
                    Call::InitializeStock {
                        business: 0,
                        argument,
                    } if argument == input.stock_argument => 0x461180,
                    Call::RefreshBuilding { building: 0 } => 0x4450a0,
                    _ => panic!("unexpected fixture callback {call:?}"),
                });
                Ok(())
            })
            .unwrap();
            let expected_calls: Vec<_> = trace
                .into_iter()
                .filter(|address| matches!(address, 0x447870 | 0x461180 | 0x4450a0))
                .collect();
            assert_eq!(calls, expected_calls, "case {case}");
            assert_eq!(
                world.businesses[0], expected_business,
                "business case {case}"
            );
            assert_eq!(
                world.buildings[0], expected_building,
                "building case {case}"
            );
            assert_eq!(
                world.categories[0], expected_category,
                "category case {case}"
            );
            assert_eq!(world.available.len(), after_list[3] as usize);
            if after_list[3] == 0 {
                assert!(world.available.is_empty());
                assert_eq!(*world.availability_cursor, None);
            } else {
                assert_eq!(world.available.as_slice(), &[0]);
                assert_eq!(*world.availability_cursor, Some(0));
                assert_eq!(after_list[0], after_list[1]);
                assert_eq!(after_list[0], after_list[2]);
                assert_eq!(word(&after_nodes[0], 0), 0x7a41a8);
                assert_eq!(&after_nodes[0][4..], &[0; 8]);
            }
        }
        assert_eq!(reader.at, data.len());
    }

    #[test]
    #[ignore = "requires original category catalog fixture"]
    fn matches_native_category_catalog_traversal() {
        let data = std::fs::read(crate::fixtures::path("NATIVE_CATEGORY_CATALOG")).unwrap();
        let mut reader = Reader { data: &data, at: 0 };
        assert_eq!(reader.bytes::<8>(), *b"MTCCAT01");
        assert_eq!(reader.word(), 1);
        for case in 0..reader.word() {
            let mut categories = [[0; 0x17c]; 52];
            categories[0] = reader.bytes();
            let mut buildings: [[u8; 0x394]; 3] = std::array::from_fn(|_| reader.bytes());
            let mut businesses: [[u8; 0x220]; 3] = std::array::from_fn(|_| reader.bytes());
            let catalog = BusinessCatalog::from_rows(std::array::from_fn(|_| reader.bytes()));
            let products =
                std::array::from_fn(|_| std::array::from_fn(|_| [reader.word(), reader.word()]));
            let commodities = std::array::from_fn(|_| reader.bytes());
            let expected: [u8; 0x17c] = reader.bytes();
            let mut available = vec![];
            let mut cursor = None;
            let mut world = World {
                buildings: &mut buildings,
                businesses: &mut businesses,
                categories: &mut categories,
                category_base: 0x617628,
                catalog: &catalog,
                available: &mut available,
                availability_cursor: &mut cursor,
                players: &[],
                resident_count: 0,
            };
            refresh_category(&mut world, 0, 0x591318, &products, &commodities).unwrap();
            assert_eq!(world.categories[0], expected, "case {case}");
        }
        assert_eq!(reader.at, data.len());
    }
}
