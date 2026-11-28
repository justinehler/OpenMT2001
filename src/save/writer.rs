//! Original4bbe30 record production. This is encoding, not load orchestration.
//! Host file I/O and the unwritten final allocation word remain explicit.

pub struct Player<'a> {
    pub metadata: &'a [u8; 48],
    /// Original runtime+4..+1bc; excludes the vtable and controller tail.
    pub prefix: &'a [u8; 440],
}
pub struct Resident<'a> {
    pub prefix: &'a [u8; 330],
    pub active: u8,
}
pub struct Scheduled<'a> {
    pub header: &'a [u8; 24],
    /// Kind0 owns the packet whose DWORD+8 gives its complete size.
    pub packet: Option<&'a [u8]>,
}
pub struct Auction<'a> {
    pub record: &'a mut [u8; 344],
    pub countdown: u32,
}
pub struct State<'a> {
    pub compatibility: &'a [u8; 40],
    pub level: u32,
    pub difficulty: u32,
    pub current_cash: f32,
    pub players: &'a [Player<'a>],
    pub businesses: &'a [[u8; 544]],
    pub buildings: &'a [[u8; 916]],
    pub building_base: u32,
    pub building_sentinel_next: u32,
    pub categories: &'a mut [[u8; 380]; 52],
    pub residents: &'a [Resident<'a>],
    pub cameras: &'a [[u8; 196]; 2],
    pub calendar: [u32; 5],
    pub timing: [u32; 6],
    pub pending: &'a [u8; 3588],
    pub resident_accounts: &'a [[u8; 144]],
    /// Original total list, then normal list, each in retained traversal order.
    pub scheduled: &'a [Scheduled<'a>],
    pub auction: Option<Auction<'a>>,
    pub objective: &'a [u8; 60],
    pub business_info: &'a [[u8; 116]; 45],
    pub commodities: &'a [[u8; 28]; 32],
    pub history: &'a [u8; 0x10f44],
    /// Raw original 0x8fca9c, independent of the clock's logic mode.
    pub session_option_word: u32,
}

struct Encoder(Vec<u8>);
impl Encoder {
    fn append(&mut self, tag: u32, payload: &[u8]) -> Result<(), String> {
        let size = u32::try_from(payload.len()).map_err(|_| "save record exceeds DWORD length")?;
        if self
            .0
            .len()
            .checked_add(payload.len() + 8)
            .is_none_or(|n| n > 0x200000)
        {
            return Err("original save staging capacity exceeded".into());
        }
        self.0.extend_from_slice(&tag.to_le_bytes());
        self.0.extend_from_slice(&size.to_le_bytes());
        self.0.extend_from_slice(payload);
        Ok(())
    }
    fn words(&mut self, tag: u32, words: &[u32]) -> Result<(), String> {
        self.append(
            tag,
            &words
                .iter()
                .flat_map(|n| n.to_le_bytes())
                .collect::<Vec<_>>(),
        )
    }
}
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn flatten<const N: usize>(rows: &[[u8; N]]) -> Vec<u8> {
    rows.iter().flat_map(|row| row.iter().copied()).collect()
}

/// The writer's selection bit for the building record is8, while the emitted
/// tag is4. Camera bit100 emits both100 and100000. Compatibility is unconditional.
/// Category ID arrays and auction+80 are synchronously updated as native does.
pub fn encode(
    state: &mut State<'_>,
    mask: u32,
    terminator_prior: [u8; 4],
) -> Result<Vec<u8>, String> {
    let mut out = Encoder(Vec::new());
    out.append(1, state.compatibility)?;
    if mask & 0x2000 != 0 {
        let year = state.calendar[1].wrapping_add(
            state.calendar[2]
                .wrapping_mul(5)
                .wrapping_add(0x3c5)
                .wrapping_mul(2),
        );
        let cash = state.current_cash.to_bits();
        // The emulator does not reproduce x87 signalling-NaN quieting here.
        // Keep that exceptional input explicit until a hardware oracle exists.
        if cash & 0x7fc00000 == 0x7f800000 && cash & 0x003fffff != 0 {
            return Err("save cash signalling-NaN needs verified x87 exception semantics".into());
        }
        out.words(0x2000, &[state.level, year, cash, state.difficulty])?;
    }
    if mask & 0x40 != 0 {
        for player in state.players {
            out.append(0x80000, player.metadata)?;
            out.append(0x40, player.prefix)?;
        }
    }
    if mask & 0x20 != 0 {
        if state.businesses.len() != 600 {
            return Err("save requires the complete600 business pool".into());
        }
        out.append(0x20, &flatten(state.businesses))?;
    }
    if mask & 8 != 0 {
        if state.buildings.len() != 600 {
            return Err("save requires the complete600 building pool".into());
        }
        let mut bytes = Vec::with_capacity(0xb270);
        for row in state.buildings {
            bytes.extend_from_slice(&row[..64]);
        }
        for category in state.categories.iter_mut() {
            let count = word(category, 8) as i32;
            if count > 24 {
                return Err("category membership exceeds its original24 ID slots".into());
            }
            let mut pointer = word(category, 0x15c);
            for i in 0..count.max(0) as usize {
                let (id, next) = if pointer == 0x590f80 {
                    (u32::MAX, state.building_sentinel_next)
                } else {
                    let offset = pointer
                        .checked_sub(state.building_base)
                        .ok_or("saved category points before building pool")?;
                    if !offset.is_multiple_of(916) {
                        return Err("unaligned category building link".into());
                    }
                    let row = state
                        .buildings
                        .get((offset / 916) as usize)
                        .ok_or("category link outside building pool")?;
                    (word(row, 0), word(row, 0x44))
                };
                category[12 + i * 4..16 + i * 4].copy_from_slice(&id.to_le_bytes());
                pointer = next;
            }
            bytes.extend_from_slice(&category[..140]);
        }
        out.append(4, &bytes)?;
    }
    if mask & 0x10 != 0 {
        if state.residents.len() != 2000 {
            return Err("save requires the complete2000 resident pool".into());
        }
        for resident in state.residents {
            if resident.active != 0 {
                out.append(0x10, resident.prefix)?;
            }
        }
    }
    // Original44c080 (mask80) returns without producing a vehicle record.
    if mask & 0x100 != 0 {
        out.append(0x100, &state.cameras[0])?;
        out.append(0x100000, &state.cameras[1])?;
    }
    if mask & 0x200 != 0 {
        out.words(0x200, &state.calendar)?;
    }
    if mask & 0x400 != 0 {
        out.words(0x400, &state.timing)?;
    }
    if mask & 0x800 != 0 {
        out.append(0x800, state.pending)?;
    }
    if mask & 0x1000 != 0 {
        if state.resident_accounts.len() != 2000 {
            return Err("save requires the complete2000 resident accounts".into());
        }
        out.append(0x1000, &flatten(state.resident_accounts))?;
    }
    if mask & 0x4000 != 0 {
        for event in state.scheduled {
            let mut bytes = event.header.to_vec();
            if word(event.header, 0) == 0 {
                let packet = event
                    .packet
                    .ok_or("kind0 scheduled event has no retained packet")?;
                if packet.len() < 12 || word(packet, 8) as usize != packet.len() {
                    return Err("scheduled packet size differs from its retained header".into());
                }
                bytes.extend_from_slice(packet);
            }
            out.append(0x4000, &bytes)?;
        }
    }
    if mask & 0x8000 != 0
        && let Some(auction) = &mut state.auction
        && (auction.record[0x7c] != 0 || auction.record[0x7d] != 0)
    {
        auction.record[0x80..0x84].copy_from_slice(&auction.countdown.to_le_bytes());
        out.append(0x8000, auction.record)?;
    }
    if mask & 0x10000 != 0 {
        out.append(0x10000, state.objective)?;
    }
    if mask & 0x40000 != 0 {
        out.append(0x40000, &flatten(state.business_info))?;
    }
    if mask & 0x200000 != 0 {
        out.append(0x200000, &flatten(state.commodities))?;
    }
    if mask & 0x20000 != 0 {
        out.append(0x20000, state.history)?;
    }
    if mask & 0x400000 != 0 {
        out.words(0x400000, &[state.session_option_word])?;
    }
    // The original counts8 bytes but writes only the terminator DWORD. Preserve
    // explicitly supplied allocation-prior bytes rather than inventing zeros.
    if out.0.len() + 8 > 0x200000 {
        return Err("original save staging capacity exceeded".into());
    }
    out.0.extend_from_slice(&u32::MAX.to_le_bytes());
    out.0.extend_from_slice(&terminator_prior);
    Ok(out.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, size: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(size);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            word(self.take(4), 0)
        }
        fn array<const N: usize>(&mut self) -> &'a [u8; N] {
            self.take(N).try_into().unwrap()
        }
        fn rows<const N: usize>(&mut self, count: usize) -> Vec<[u8; N]> {
            (0..count).map(|_| *self.array()).collect()
        }
        fn blob(&mut self) -> &'a [u8] {
            let n = self.word() as usize;
            self.take(n)
        }
    }
    #[test]
    #[ignore = "requires complete original save-writer observations"]
    fn matches_native_complete_save_writer() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_SAVE_WRITER")).unwrap();
        let image_bytes = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&image_bytes).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSAVE01");
        assert_eq!(r.word(), 0x37f);
        let count = r.word();
        for case in 0..count {
            let mask = r.word();
            let prior = r.word().to_le_bytes();
            let level = r.word();
            let difficulty = r.word();
            let current_cash = f32::from_bits(r.word());
            let building_base = r.word();
            let building_sentinel_next = r.word();
            let calendar = std::array::from_fn(|_| r.word());
            let timing = std::array::from_fn(|_| r.word());
            let countdown = r.word();
            let session_option_word = r.word();
            let players_count = r.word();
            let players: Vec<_> = (0..players_count)
                .map(|_| Player {
                    metadata: r.array(),
                    prefix: r.array(),
                })
                .collect();
            let businesses = r.rows(600);
            let buildings = r.rows(600);
            let mut categories: [_; 52] = r.rows::<380>(52).try_into().unwrap();
            let residents: Vec<_> = (0..2000)
                .map(|_| {
                    let prefix = r.array();
                    let active = r.take(1)[0];
                    Resident { prefix, active }
                })
                .collect();
            let cameras = r.array::<392>();
            let cameras: [&[u8; 196]; 2] = [
                cameras[..196].try_into().unwrap(),
                cameras[196..].try_into().unwrap(),
            ];
            let cameras = [*cameras[0], *cameras[1]];
            let pending = r.array();
            let resident_accounts = r.rows(2000);
            let n = r.word();
            let scheduled: Vec<_> = (0..n)
                .map(|_| {
                    let header = r.array();
                    let packet = r.blob();
                    Scheduled {
                        header,
                        packet: (!packet.is_empty()).then_some(packet),
                    }
                })
                .collect();
            let present = r.word() != 0;
            let mut auction = *r.array();
            let objective = r.array();
            let business_info: [_; 45] = r.rows::<116>(45).try_into().unwrap();
            let commodities: [_; 32] = r.rows::<28>(32).try_into().unwrap();
            let history = r.array();
            let expected = r.blob();
            let expected_categories = r.take(52 * 380);
            let expected_auction = r.take(344);
            let mut state = State {
                compatibility: image.at(0x545d1c, 40).unwrap().try_into().unwrap(),
                level,
                difficulty,
                current_cash,
                players: &players,
                businesses: &businesses,
                buildings: &buildings,
                building_base,
                building_sentinel_next,
                categories: &mut categories,
                residents: &residents,
                cameras: &cameras,
                calendar,
                timing,
                pending,
                resident_accounts: &resident_accounts,
                scheduled: &scheduled,
                auction: present.then_some(Auction {
                    record: &mut auction,
                    countdown,
                }),
                objective,
                business_info: &business_info,
                commodities: &commodities,
                history,
                session_option_word,
            };
            let actual = encode(&mut state, mask, prior).unwrap();
            if actual != expected {
                let offset = actual.iter().zip(expected).position(|(a, b)| a != b);
                panic!(
                    "case{case} mask{mask:x}: save output differs at{offset:?}, lengths{} and{}, association {:x?} vs {:x?}",
                    actual.len(),
                    expected.len(),
                    &actual[56..72],
                    &expected[56..72]
                );
            }
            assert_eq!(
                flatten(&categories),
                expected_categories,
                "case{case} category stores"
            );
            assert_eq!(auction, expected_auction, "case{case} auction stores");
            crate::save::SaveFile::parse(&actual)
                .unwrap()
                .validate_compatibility(&image)
                .unwrap();
        }
        assert!(r.0.is_empty());
        println!(
            "{count} whole original save writers matched, including category and auction side effects"
        );
    }
}
