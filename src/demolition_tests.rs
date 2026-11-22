//! Native comparison for the complete player demolition path.
//!
//! The fixture is produced by `tools/native_demolition_command.py` and keeps
//! the selected business, building, category, resident pool, both account
//! records, and pending-world blob.  This test intentionally uses the same
//! host seams as the production command path: presentation, availability,
//! resident objective, and resident intent callbacks are no-ops.  Accounting
//! and the pending-world producer remain the Rust implementations under test.

use crate::business_binding;
use crate::business_pool::BusinessCatalog;
use crate::demolition;
use crate::economy::{self, PlayerAccount};
use crate::pending_world::State as PendingState;
use crate::resident::Resident;

const MAGIC: &[u8; 8] = b"MTDEM01\0";
const VERSION: u32 = 3;
const CATEGORY_BASE: u32 = 0x617628;
// Original 0052508c (the fixture's 9.1 result is 10.0 - 9 cells * 0.1).
const LANDSCAPE_VALUE: f32 = 0.1;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let end = self.at.checked_add(N).expect("fixture offset overflow");
        let value = self.bytes[self.at..end]
            .try_into()
            .expect("truncated fixture");
        self.at = end;
        value
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take())
    }

    fn blob<const N: usize>(&mut self) -> Vec<u8> {
        self.take::<N>().to_vec()
    }
}

#[derive(Clone)]
struct Snapshot {
    business: Vec<u8>,
    building: Vec<u8>,
    category: Vec<u8>,
    residents: Vec<u8>,
    player: Vec<u8>,
    other: Vec<u8>,
    pending: Vec<u8>,
    queue_count: u32,
    owner_counter: u32,
}

struct NativeCase {
    name: String,
    input: [u32; 11],
    /// The native trace is parsed and retained so a malformed/truncated call
    /// stream cannot make a state comparison appear valid.
    calls: Vec<(u32, u32, Vec<u32>)>,
    before: Snapshot,
    after: Snapshot,
}

fn read_pair<const N: usize>(reader: &mut Reader<'_>) -> (Vec<u8>, Vec<u8>) {
    (reader.blob::<N>(), reader.blob::<N>())
}

fn read_fixture() -> Vec<NativeCase> {
    let path = std::env::var("OPENMT2001_NATIVE_DEMOLITION_COMMAND")
        .expect("set OPENMT2001_NATIVE_DEMOLITION_COMMAND to MTDEM01 v3 fixture");
    let bytes = std::fs::read(path).expect("read MTDEM01 v3 fixture");
    let mut reader = Reader {
        bytes: &bytes,
        at: 0,
    };
    assert_eq!(&reader.take::<8>(), MAGIC);
    assert_eq!(reader.u32(), VERSION);
    let count = reader.u32() as usize;
    let mut cases = Vec::with_capacity(count);
    for _ in 0..count {
        let input = std::array::from_fn(|_| reader.u32());
        let call_count = reader.u32() as usize;
        let mut calls = Vec::with_capacity(call_count);
        for _ in 0..call_count {
            let address = reader.u32();
            let ecx = reader.u32();
            let argc = reader.u32() as usize;
            let args = (0..argc).map(|_| reader.u32()).collect();
            calls.push((address, ecx, args));
        }
        // The binary writer stores each field as before/after consecutively,
        // rather than writing two complete snapshots back-to-back.
        let (business_before, business_after) = read_pair::<0x220>(&mut reader);
        let (building_before, building_after) = read_pair::<0x394>(&mut reader);
        let (category_before, category_after) = read_pair::<0x17c>(&mut reader);
        let (residents_before, residents_after) = read_pair::<{ 3 * 0x150 }>(&mut reader);
        let (player_before, player_after) = read_pair::<0x220>(&mut reader);
        let (other_before, other_after) = read_pair::<0x220>(&mut reader);
        let (pending_before, pending_after) = read_pair::<0xe04>(&mut reader);
        let before_queue_count = reader.u32();
        let after_queue_count = reader.u32();
        let after_owner_counter = reader.u32();
        let before = Snapshot {
            business: business_before,
            building: building_before,
            category: category_before,
            residents: residents_before,
            player: player_before,
            other: other_before,
            pending: pending_before,
            queue_count: before_queue_count,
            owner_counter: 0,
        };
        let after = Snapshot {
            business: business_after,
            building: building_after,
            category: category_after,
            residents: residents_after,
            player: player_after,
            other: other_after,
            pending: pending_after,
            queue_count: after_queue_count,
            owner_counter: after_owner_counter,
        };
        cases.push(NativeCase {
            name: format!("case-{}/{}", cases.len(), count),
            input,
            calls,
            before,
            after,
        });
    }
    assert_eq!(reader.at, bytes.len(), "trailing bytes in MTDEM01 fixture");
    cases
}

fn resident_from_runtime(row: &[u8]) -> Resident {
    let payload: [u8; 0x14a] = row[..0x14a].try_into().unwrap();
    let active = row[0x14a];
    let auxiliary = u32::from_le_bytes(row[0x14c..0x150].try_into().unwrap());
    Resident::from_runtime_bytes(payload, active, auxiliary)
}

fn resident_blob(residents: &[Resident; 3], initial: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(3 * 0x150);
    for (index, resident) in residents.iter().enumerate() {
        let runtime_at = index * 0x150;
        bytes.extend_from_slice(resident.as_bytes());
        bytes.push(resident.active_byte());
        // +14b is padding and remains untouched by the native path.
        bytes.push(initial[runtime_at + 0x14b]);
        bytes.extend_from_slice(&initial[runtime_at + 0x14c..runtime_at + 0x150]);
    }
    bytes
}

fn set_resident_detached(resident: &mut Resident) {
    // 45cd10 clears the housing category/building and the resident objective
    // byte before the intent seam.  Keep +14a/+14c runtime fields intact.
    let mut bytes = *resident.as_bytes();
    bytes[0x0c] = 0xff;
    bytes[0x0e..0x10].copy_from_slice(&u16::MAX.to_le_bytes());
    bytes[0x40] = 0;
    *resident = Resident::from_runtime_bytes(
        bytes,
        resident.active_byte(),
        resident.auxiliary_reference(),
    );
}

fn compare_case(case: &NativeCase) {
    assert!(
        !case.calls.is_empty(),
        "{} has no native call trace",
        case.name
    );
    assert_eq!(
        case.input[10], 1,
        "{} is not a native 462b00 case",
        case.name
    );

    let mut businesses = [case.before.business.clone().try_into().unwrap()];
    let mut buildings = [case.before.building.clone().try_into().unwrap()];
    let mut categories = [[0u8; 0x17c]; 52];
    categories[case.input[1] as usize] = case.before.category.clone().try_into().unwrap();
    let mut residents = std::array::from_fn(|index| {
        resident_from_runtime(&case.before.residents[index * 0x150..(index + 1) * 0x150])
    });
    let mut pending = PendingState::from_bytes(&case.before.pending).unwrap();
    let mut players_raw = [case.before.player.clone(), case.before.other.clone()];
    let mut accounts = [
        PlayerAccount::from_runtime(&players_raw[0]).unwrap(),
        PlayerAccount::from_runtime(&players_raw[1]).unwrap(),
    ];
    let identities = [
        Some(business_binding::Player {
            identity: 0x1003_0000,
            kind: 0,
        }),
        Some(business_binding::Player {
            identity: 0x1003_1000,
            kind: 0,
        }),
    ];
    let catalog = BusinessCatalog::from_rows([[0u8; 0x74]; 45]);
    let mut available = vec![0usize];
    let mut availability_cursor = Some(0usize);
    let mut owner_counter = 5u32;

    let command = demolition::Command {
        category: case.input[1] as i8,
        building: case.input[0] as u16,
        amount: f32::from_bits(case.input[7]),
    };
    let notifications = case.input[8] != 0;
    let mut world = demolition::World {
        buildings: &mut buildings,
        businesses: &mut businesses,
        categories: &mut categories,
        pending: &mut pending,
    };
    demolition::apply(&command, &mut world, |call, world| {
        match call {
            demolition::Call::Post {
                owner,
                amount,
                ledger,
            } => {
                let slot = usize::try_from(owner).unwrap().checked_sub(1).unwrap();
                accounts[slot].post(amount, ledger)?;
                accounts[slot].write_runtime(&mut players_raw[slot])?;
            }
            demolition::Call::Detach { business } => {
                let mut detach_world = business_binding::World {
                    buildings: world.buildings,
                    businesses: world.businesses,
                    categories: world.categories,
                    category_base: CATEGORY_BASE,
                    catalog: &catalog,
                    available: &mut available,
                    availability_cursor: &mut availability_cursor,
                    players: &identities,
                    resident_count: residents.len(),
                };
                crate::business_detach::apply(
                    business,
                    &mut detach_world,
                    &mut residents,
                    notifications,
                    LANDSCAPE_VALUE,
                    |detach_call, detach_world, residents| {
                        match detach_call {
                            crate::business_detach::Call::TransferOwner { business, owner } => {
                                let building_index = i16::from_le_bytes(
                                    detach_world.businesses[business][8..10].try_into().unwrap(),
                                ) as usize;
                                let building: [u8; 64] = detach_world.buildings[building_index]
                                    [..64]
                                    .try_into()
                                    .unwrap();
                                economy::transfer_business_owner(
                                    &mut detach_world.businesses[business],
                                    &building,
                                    owner,
                                    true,
                                    &mut accounts,
                                )?;
                                for (slot, account) in accounts.iter().enumerate() {
                                    account.write_runtime(&mut players_raw[slot])?;
                                }
                            }
                            crate::business_detach::Call::EvictResident { resident } => {
                                // Objective selection returns zero in this
                                // fixture and the intent callback is a no-op.
                                set_resident_detached(&mut residents[resident]);
                            }
                            crate::business_detach::Call::RemoveOwnerCount { .. } => {
                                owner_counter = owner_counter.saturating_sub(1);
                            }
                            crate::business_detach::Call::RefreshBuilding { .. }
                            | crate::business_detach::Call::RefreshCategory { .. }
                            | crate::business_detach::Call::NotifyBusiness { .. } => {}
                        }
                        // Keep the callback argument live so future seam
                        // extensions can inspect the same records in order.
                        let _ = (detach_world, residents);
                        Ok(())
                    },
                )?;
            }
            demolition::Call::Publish
            | demolition::Call::RefreshInterface
            | demolition::Call::RefreshBuilding { .. } => {}
        }
        Ok(())
    })
    .unwrap();

    for (slot, account) in accounts.iter().enumerate() {
        account.write_runtime(&mut players_raw[slot]).unwrap();
    }
    let actual_business = businesses[0].to_vec();
    let actual_building = buildings[0].to_vec();
    let actual_category = categories[case.input[1] as usize].to_vec();
    let actual_pending = pending.to_bytes();
    assert_eq!(
        actual_business, case.after.business,
        "{} business",
        case.name
    );
    assert_eq!(
        actual_building, case.after.building,
        "{} building",
        case.name
    );
    assert_eq!(
        actual_category, case.after.category,
        "{} category",
        case.name
    );
    assert_eq!(
        resident_blob(&residents, &case.before.residents),
        case.after.residents,
        "{} residents",
        case.name
    );
    assert_eq!(
        players_raw[0], case.after.player,
        "{} owner player",
        case.name
    );
    assert_eq!(
        players_raw[1], case.after.other,
        "{} other player",
        case.name
    );
    assert_eq!(
        actual_pending, case.after.pending,
        "{} pending blob",
        case.name
    );
    assert_eq!(
        pending.count, case.after.queue_count,
        "{} pending count",
        case.name
    );
    assert_eq!(
        owner_counter, case.after.owner_counter,
        "{} owner counter",
        case.name
    );
}

#[test]
#[ignore = "requires OPENMT2001_NATIVE_DEMOLITION_COMMAND=MTDEM01 v3"]
fn matches_native_demolition_command_full_snapshots() {
    for case in read_fixture() {
        compare_case(&case);
    }
}
