//! Original hourly category countdown and callout ordering (00447e20).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub owner: u8,
    pub ownership_state: u32,
    pub countdowns: [u32; 2],
    /// Value loaded from the original name-table row, not its pointer/index.
    pub notification_value: u32,
}

pub struct Controls {
    /// Zero-based byte read from the current-player object+1bc.
    pub current_owner_index: u8,
    pub selected_category: Option<u32>,
    pub selection_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub address: u32,
    pub arguments: [u32; 3],
    pub argument_count: u8,
}

pub fn from_saved(
    save: &crate::save::SaveFile<'_>,
    notification_values: &[u32; 52],
) -> Result<[State; 52], String> {
    let mut states = [State {
        owner: 0,
        ownership_state: 0,
        countdowns: [0; 2],
        notification_value: 0,
    }; 52];
    for (index, state) in states.iter_mut().enumerate() {
        *state = State {
            owner: save.category_owner(index)?,
            ownership_state: save.category_ownership_state(index)?,
            countdowns: save.category_countdowns(index)?,
            notification_value: notification_values[index],
        };
    }
    Ok(states)
}

pub fn advance_hour(
    states: &mut [State; 52],
    controls: &mut Controls,
    mut callback: impl FnMut(Call, &mut [State; 52], &mut Controls) -> Result<(), String>,
) -> Result<(), String> {
    let matches_owner = |state: &State, controls: &Controls| {
        i32::from(state.owner as i8) == i32::from(controls.current_owner_index) + 1
    };
    for index in 0..52 {
        if states[index].owner == 0 || states[index].countdowns[0] == 0 {
            continue;
        }
        states[index].countdowns = states[index].countdowns.map(|v| v.wrapping_sub(1));
        if states[index].countdowns[0] == 24 && matches_owner(&states[index], controls) {
            notify(index, 0x6c0, states, controls, &mut callback)?;
        }
        // Native reloads the first countdown after the notification callbacks.
        if states[index].countdowns[0] != 0 {
            continue;
        }
        if matches_owner(&states[index], controls) {
            notify(index, 0x6c1, states, controls, &mut callback)?;
        }
        callback(
            Call {
                address: 0x48c450,
                arguments: [index as u32, 0, 0],
                argument_count: 2,
            },
            states,
            controls,
        )?;
        if controls.selection_active && controls.selected_category == Some(index as u32) {
            callback(
                Call {
                    address: 0x464e30,
                    arguments: [0; 3],
                    argument_count: 0,
                },
                states,
                controls,
            )?;
        }
    }
    Ok(())
}

fn notify(
    index: usize,
    event: u32,
    states: &mut [State; 52],
    controls: &mut Controls,
    callback: &mut impl FnMut(Call, &mut [State; 52], &mut Controls) -> Result<(), String>,
) -> Result<(), String> {
    for (address, arguments, argument_count) in [
        (0x43be20, [0; 3], 0),
        (0x429af0, [0x12e, 0, 0], 1),
        (0x429820, [0, 0x6bf, 0], 3),
        (0x441940, [0; 3], 0),
    ] {
        callback(
            Call {
                address,
                arguments,
                argument_count,
            },
            states,
            controls,
        )?;
    }
    callback(
        Call {
            address: 0x441080,
            arguments: [event, states[index].notification_value, 0],
            argument_count: 2,
        },
        states,
        controls,
    )?;
    callback(
        Call {
            address: 0x429a60,
            arguments: [1, 0, 0],
            argument_count: 1,
        },
        states,
        controls,
    )?;
    callback(
        Call {
            address: 0x4298b0,
            arguments: [0, 0x59b, 0],
            argument_count: 2,
        },
        states,
        controls,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpiryEvent {
    /// Original00485520 appearance update for this building slot.
    BuildingAppearance(usize),
    /// Original00467a30. Bind to ai_ownership::State::maintain once its
    /// runtime AI state is initialized.
    OwnerLists(usize),
    /// Original0041f410 manager refresh, still a callback.
    Refresh,
}

/// Borrow the same business/player ledgers used by housing and shopping.
/// Building prefixes and chain order come from the saved scene; +4c render
/// kinds are runtime inputs and must not be guessed from the saved active byte.
pub struct Expiry<'a> {
    pub buildings: &'a mut [[u8; 64]],
    pub render_kinds: &'a [u32],
    pub chains: &'a [Vec<usize>; 52],
    pub ledgers: &'a mut crate::economy::Ledgers,
    pub normal_tick: u32,
    pub accounting_enabled: bool,
    /// Original byte82296e; expiry adds100 with byte wrapping.
    pub refresh_counter: u8,
}

impl Expiry<'_> {
    /// Run the countdown pass and its recovered zero-compensation expiry on
    /// shared ledgers. Notification and remaining AI/appearance callbacks stay
    /// synchronous, preserving their position among simulation mutations.
    pub fn advance_hour(
        &mut self,
        categories: &mut [State; 52],
        controls: &mut Controls,
        mut callout: impl FnMut(Call, &mut [State; 52], &mut Controls, &mut Self) -> Result<(), String>,
        mut expiry_event: impl FnMut(ExpiryEvent, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        advance_hour(categories, controls, |call, categories, controls| {
            if call.address == 0x48c450 {
                self.release(
                    call.arguments[0] as usize,
                    &mut categories[call.arguments[0] as usize],
                    &mut expiry_event,
                )
            } else {
                callout(call, categories, controls, self)
            }
        })
    }

    /// Original0048c450 with zero compensation, as called by00447e20.
    /// Other compensated ownership operations are not covered by this method.
    pub fn release(
        &mut self,
        index: usize,
        category: &mut State,
        mut callback: impl FnMut(ExpiryEvent, &mut Self) -> Result<(), String>,
    ) -> Result<(), String> {
        if index >= 52 {
            return Err("expired category exceeds the original table".into());
        }
        let owner = category.owner.checked_sub(1).map(usize::from);
        if let Some(owner) = owner {
            let player = self
                .ledgers
                .players
                .get_mut(owner)
                .ok_or("expired category owner does not resolve to a player")?;
            player.owned_categories[index] = 0;
            // Native performs both floating stores even for zero compensation.
            player.post(0., 0)?;
        }
        category.owner = 0;
        category.ownership_state = 2;
        for position in 0..self.chains[index].len() {
            let slot = self.chains[index][position];
            let building = self
                .buildings
                .get_mut(slot)
                .ok_or("expired category building leaves the pool")?;
            if u32::from_le_bytes(building[0x24..0x28].try_into().unwrap()) == 2 {
                building[0x2d] = 0;
                building[0x30..0x34].copy_from_slice(&self.normal_tick.to_le_bytes());
                if self.needs_appearance(slot)? {
                    callback(ExpiryEvent::BuildingAppearance(slot), self)?;
                }
                // The original resolver is called after the appearance callback.
                let business =
                    i32::from_le_bytes(self.buildings[slot][0x1c..0x20].try_into().unwrap());
                if business != -1 {
                    let record = self
                        .ledgers
                        .businesses
                        .get_mut(business as usize)
                        .ok_or("expired building business leaves the pool")?;
                    let building = if self.accounting_enabled {
                        i16::from_le_bytes(record[8..10].try_into().unwrap()) as usize
                    } else {
                        // Native computes the address but does not dereference
                        // the linked building when accounting is disabled.
                        slot
                    };
                    let prefix = self
                        .buildings
                        .get(building)
                        .ok_or("expired business building leaves the pool")?;
                    crate::economy::transfer_business_owner(
                        record,
                        prefix,
                        0,
                        self.accounting_enabled,
                        &mut self.ledgers.players,
                    )?;
                }
            }
            if self.needs_appearance(slot)? {
                callback(ExpiryEvent::BuildingAppearance(slot), self)?;
            }
        }
        self.refresh_counter = self.refresh_counter.wrapping_add(100);
        if let Some(owner) = owner
            && self.ledgers.players[owner].controller_kind == 1
        {
            callback(ExpiryEvent::OwnerLists(owner), self)?;
        }
        callback(ExpiryEvent::Refresh, self)
    }

    fn needs_appearance(&self, slot: usize) -> Result<bool, String> {
        let kind = *self
            .render_kinds
            .get(slot)
            .ok_or("building render kind leaves the pool")?;
        let prefix = &self.buildings[slot];
        let business = i32::from_le_bytes(prefix[0x1c..0x20].try_into().unwrap());
        let category = u32::from_le_bytes(prefix[0x24..0x28].try_into().unwrap());
        Ok(kind == 1 && (business == -1 || category <= 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original expiry observations and saved shopping bindings"]
    fn matches_native_expiry_and_hourly_caller() {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let bytes = read("OPENMT2001_NATIVE_CATEGORY_EXPIRY");
        let exe = read("OPENMT2001_EXE");
        let saved = read("OPENMT2001_SAVE_ONE");
        let save = crate::save::SaveFile::parse(&saved).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let mut topology =
            crate::shopping::Topology::from_saved(&save, &board, &[[0.; 3]; 600]).unwrap();
        let mut ledgers = crate::economy::Ledgers::from_saved(&save).unwrap();
        let field = |raw: &[u8], at| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        let word = |at| field(&bytes, at);
        assert_eq!(&bytes[..8], b"MTEXP002");
        assert_eq!(word(8), 9);
        let mut at = 12;
        for _ in 0..word(8) {
            let length = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&bytes[at..at + length]).unwrap();
            at += length;
            let config: [u32; 7] = std::array::from_fn(|i| word(at + i * 4));
            at += 28;
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            for length in [52 * 380, 600 * 916, 600 * 544, 0x500] {
                assert_eq!(word(at) as usize, length);
                at += 4;
                actual.push(bytes[at..at + length].to_vec());
                at += length;
                assert_eq!(word(at) as usize, length);
                at += 4;
                expected.push(bytes[at..at + length].to_vec());
                at += length;
            }
            topology.categories = std::array::from_fn(|i| State {
                owner: actual[0][i * 380 + 5],
                ownership_state: field(&actual[0], i * 380 + 0x78),
                countdowns: [
                    field(&actual[0], i * 380 + 0x7c),
                    field(&actual[0], i * 380 + 0x80),
                ],
                notification_value: u32::from_le_bytes(
                    image
                        .at(0x5385a0 + field(&actual[0], i * 380 + 0x8c) * 88, 4)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                ),
            });
            let mut buildings: Vec<[u8; 64]> = actual[1]
                .chunks_exact(916)
                .map(|b| b[..64].try_into().unwrap())
                .collect();
            let render_kinds: Vec<u32> = actual[1]
                .chunks_exact(916)
                .map(|b| field(b, 0x4c))
                .collect();
            let chains: [Vec<usize>; 52] = std::array::from_fn(|i| {
                let mut chain = Vec::new();
                let mut pointer = field(&actual[0], i * 380 + 0x15c);
                while pointer != 0 {
                    let delta = pointer.checked_sub(0x591318).unwrap();
                    assert_eq!(delta % 916, 0);
                    let slot = (delta / 916) as usize;
                    assert!(slot < 600 && !chain.contains(&slot));
                    chain.push(slot);
                    pointer = field(&actual[1], slot * 916 + 0x44);
                }
                chain
            });
            ledgers.businesses = actual[2]
                .chunks_exact(544)
                .map(|b| b.try_into().unwrap())
                .collect();
            ledgers.players = vec![
                crate::economy::PlayerAccount::from_saved(&crate::save::SavedPlayer {
                    metadata: &[0; 48],
                    bytes: actual[3][4..444].try_into().unwrap(),
                })
                .unwrap(),
            ];
            let mut trace = Vec::new();
            let mut event = |event, _: &mut Expiry<'_>| {
                trace.push(match event {
                    ExpiryEvent::BuildingAppearance(slot) => {
                        [0x485520, 0x591318 + slot as u32 * 916]
                    }
                    ExpiryEvent::OwnerLists(owner) => {
                        assert_eq!(owner, 0);
                        [0x467a30, 0x10020000]
                    }
                    ExpiryEvent::Refresh => [0x41f410, 0x10090000],
                });
                Ok(())
            };
            let counter = {
                let mut expiry = Expiry {
                    buildings: &mut buildings,
                    render_kinds: &render_kinds,
                    chains: &chains,
                    ledgers: &mut ledgers,
                    normal_tick: config[2],
                    accounting_enabled: config[3] != 0,
                    refresh_counter: config[4] as u8,
                };
                if config[0] == 0x447e20 {
                    expiry
                        .advance_hour(
                            &mut topology.categories,
                            &mut Controls {
                                current_owner_index: config[6] as u8,
                                selected_category: None,
                                selection_active: false,
                            },
                            |call, _, _, _| panic!("unexpected notification {call:?}"),
                            &mut event,
                        )
                        .unwrap();
                } else {
                    assert_eq!(config[0], 0x48c450);
                    expiry
                        .release(
                            config[1] as usize,
                            &mut topology.categories[config[1] as usize],
                            &mut event,
                        )
                        .unwrap();
                }
                expiry.refresh_counter
            };
            assert_eq!(u32::from(counter), config[5], "{name} counter");
            for (i, state) in topology.categories.iter().enumerate() {
                actual[0][i * 380 + 5] = state.owner;
                for (offset, value) in [
                    (0x78, state.ownership_state),
                    (0x7c, state.countdowns[0]),
                    (0x80, state.countdowns[1]),
                ] {
                    actual[0][i * 380 + offset..i * 380 + offset + 4]
                        .copy_from_slice(&value.to_le_bytes());
                }
                assert_eq!(
                    topology.category_owners()[i],
                    expected[0][i * 380 + 5].checked_sub(1).map(usize::from),
                    "{name} shopping owner{i}"
                );
            }
            for (i, prefix) in buildings.iter().enumerate() {
                actual[1][i * 916..i * 916 + 64].copy_from_slice(prefix);
            }
            for (i, business) in ledgers.businesses.iter().enumerate() {
                actual[2][i * 544..(i + 1) * 544].copy_from_slice(business);
            }
            let player = &ledgers.players[0];
            actual[3][4..8].copy_from_slice(&player.cash.to_le_bytes());
            let period = usize::from(actual[3][0x144]);
            let offset = 0x14 + period * 152;
            actual[3][offset..offset + 4].copy_from_slice(&player.ledger(0).unwrap().to_le_bytes());
            actual[3][0x178..0x1ac].copy_from_slice(&player.owned_categories);
            actual[3][0x1b0..0x1b4].copy_from_slice(&player.housing_counter.to_le_bytes());
            for (i, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                assert_eq!(actual, expected, "{name} complete region{i}");
            }
            assert_eq!(word(at) as usize, trace.len(), "{name} callback count");
            at += 4;
            for call in trace {
                assert_eq!(call, [word(at), word(at + 4)], "{name} callback order");
                at += 8;
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original category hourly observations and supplied saves"]
    fn matches_native_category_hour() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let bytes = read("OPENMT2001_NATIVE_CATEGORY_HOUR");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTCATH02");
        assert_eq!(word(8), 12);
        let mut at = 12;
        for case in 0..word(8) {
            let length = word(at) as usize;
            at += 4;
            let name = std::str::from_utf8(&bytes[at..at + length]).unwrap();
            at += length;
            let config: [u32; 6] = std::array::from_fn(|i| word(at + i * 4));
            at += 24;
            let notifications: [u32; 52] = std::array::from_fn(|i| word(at + i * 4));
            at += 208;
            assert_eq!(word(at), 52 * 380);
            at += 4;
            let mut actual = bytes[at..at + 52 * 380].to_vec();
            let mut states: [State; 52] = std::array::from_fn(|i| State {
                owner: bytes[at + i * 380 + 5],
                ownership_state: word(at + i * 380 + 0x78),
                countdowns: [word(at + i * 380 + 0x7c), word(at + i * 380 + 0x80)],
                notification_value: notifications[i],
            });
            at += 52 * 380;
            assert_eq!(word(at), 52 * 380);
            at += 4;
            if case >= 10 {
                let variable = if case == 10 {
                    "OPENMT2001_SAVE_ONE"
                } else {
                    "OPENMT2001_SAVE_TWO"
                };
                let saved = read(variable);
                let save = crate::save::SaveFile::parse(&saved).unwrap();
                let exe = read("OPENMT2001_EXE");
                let board =
                    crate::board::Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap())
                        .unwrap();
                let loaded = from_saved(&save, &board.category_notification_values()).unwrap();
                assert_eq!(loaded, states, "{name} saved inputs");
            }
            let mut controls = Controls {
                current_owner_index: config[0] as u8,
                selected_category: (config[1] != 0).then_some(config[2]),
                selection_active: config[3] != 0,
            };
            let mut trace = Vec::new();
            advance_hour(&mut states, &mut controls, |call, states, controls| {
                trace.push(call);
                if call.address == 0x441080 && call.arguments[0] == 0x6c0 && config[4] != u32::MAX {
                    states[config[4] as usize].countdowns[0] = 0;
                }
                if call.address == 0x48c450 && config[5] != 0 {
                    controls.selected_category = Some(config[2]);
                    controls.selection_active = true;
                }
                Ok(())
            })
            .unwrap();
            for (i, state) in states.iter().enumerate() {
                actual[i * 380 + 5] = state.owner;
                for (j, value) in state.countdowns.iter().enumerate() {
                    let offset = i * 380 + 0x7c + j * 4;
                    actual[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
            assert_eq!(actual, bytes[at..at + 52 * 380], "{name} table");
            at += 52 * 380;
            assert_eq!(word(at) as usize, trace.len(), "{name} call count");
            at += 4;
            for actual in trace {
                let expected = Call {
                    address: word(at),
                    argument_count: word(at + 4) as u8,
                    arguments: [word(at + 8), word(at + 12), word(at + 16)],
                };
                assert_eq!(actual, expected, "{name} call");
                at += 20;
            }
        }
        assert_eq!(at, bytes.len());
    }
}
