//! Retained-session save encoding and V2 runtime restoration; no file I/O.
//! Native records use the checked 4bbe30 writer. The V2 extension preserves
//! runtime allocations/RNG that the native format deliberately omits.
use crate::session::Session;
pub mod auction;
#[cfg(test)]
mod queued_command_tests;
pub mod transport;
use crate::{
    live_commands::Command,
    save::{SaveFile, extension, writer},
};
pub use auction::{AuctionContinuation, AuctionHostState};

pub fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
pub fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn validate_dust_bindings(
    manager: &crate::demolition_effect::Manager,
    pending: &crate::pending_world::State,
    buildings: &[[u8; 916]],
) -> Result<(), String> {
    for row in &pending.records {
        if row[2] != 2 || !matches!(row[3], 1 | 2) {
            continue;
        }
        let building = buildings
            .get(row[1] as usize)
            .ok_or("dust building outside pool")?;
        let effect = manager
            .slots
            .get(word(building, 0x7c) as usize)
            .ok_or("demolition dust binding outside pool")?;
        if effect.state != 0 {
            return Err("demolition binding does not own an emitting dust slot".into());
        }
    }
    Ok(())
}
pub fn words(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}
pub fn flat<const N: usize>(rows: &[[u8; N]]) -> Vec<u8> {
    rows.as_flattened().to_vec()
}
pub struct Output(pub Vec<u8>);
impl Output {
    pub fn word(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    pub fn blob(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.word(u32::try_from(bytes.len()).map_err(|_| "V2 blob exceeds DWORD")?);
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}
pub struct Input<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Input<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("V2 length overflow")?;
        let row = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated V2 runtime state")?;
        self.at = end;
        Ok(row)
    }
    pub fn word(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    pub fn blob(&mut self) -> Result<&'a [u8], String> {
        let n = self.word()? as usize;
        self.take(n)
    }
    /// Bytes consumed so far.
    pub fn position(&self) -> usize {
        self.at
    }
    pub fn end(self) -> Result<(), String> {
        if self.at != self.bytes.len() {
            Err("trailing V2 runtime bytes".into())
        } else {
            Ok(())
        }
    }
}

/// Older extensions store process-owned pause bytes, Calendar.SetFrequency
/// state/IDs and complete timer records here. Consume their unchanged layout,
/// but keep the registrations and pause effects established by this load.
/// Native 0x200/0x400 clock words are restored separately by the native reader;
/// World reset clears timer accumulators rather than loading old timer records.
pub fn skip_saved_dispatch(input: &mut Input<'_>) -> Result<(), String> {
    input.take(9)?;
    input.word()?; // Calendar.SetFrequency value
    input.word()?; // calendar timer ID
    input.word()?; // logic timer ID
    let timers = input.blob()?;
    if timers.len() % 20 != 0 || timers.len() > 1000 * 20 {
        return Err("V2 timer pool size invalid".into());
    }
    Ok(())
}

/// Recover runtime data absent from tag10 without rewinding 45be80's movement
/// fixup or retiring the actor allocations it just created. Saved model/frame/
/// controller state is not recoverable from the native resident identity alone.
pub fn restore_resident_runtime(
    residents: &mut crate::economy::ResidentAccounts,
    auxiliary: &mut crate::resident::AuxiliaryPool,
    bytes: &[u8],
) -> Result<Vec<u32>, String> {
    use crate::resident::Resident;
    let mut input = Input::new(bytes);
    let count = input.word()?;
    let n = input.word()? as usize;
    if n != 2000
        || count > 2000
        || residents.nodes.len() != n
        || auxiliary.slots().len() != n
        || count != residents.active_count
    {
        return Err("V2/native resident pool counts disagree".into());
    }
    let mut saved = Vec::with_capacity(n);
    for _ in 0..n {
        let bytes = input
            .take(330)?
            .try_into()
            .map_err(|_| "V2 resident size invalid")?;
        let active = u8::try_from(input.word()?).map_err(|_| "V2 resident active byte invalid")?;
        saved.push(Resident::from_runtime_bytes(bytes, active, input.word()?));
    }
    let used = input.word()? as usize;
    let slots = input.blob()?;
    if slots.len() != 2000 * 284 || used > 2000 {
        return Err("V2 resident auxiliary pool size invalid".into());
    }
    input.end()?;
    let mut references = vec![0; used];
    let mut native_slots = vec![false; auxiliary.used()];
    for (native, saved) in residents.nodes.iter().zip(&saved) {
        if native.is_active() != saved.is_active() {
            return Err("V2/native resident occupancy disagrees".into());
        }
        if !native.is_active() {
            continue;
        }
        let old = saved
            .auxiliary_reference()
            .checked_sub(1)
            .and_then(|slot| references.get_mut(slot as usize))
            .ok_or("V2 resident auxiliary is not allocated")?;
        let new = native.auxiliary_reference();
        let occupied = new
            .checked_sub(1)
            .and_then(|slot| native_slots.get_mut(slot as usize))
            .ok_or("native resident auxiliary is not allocated")?;
        if *old != 0 || *occupied {
            return Err("resident auxiliary shared by multiple residents".into());
        }
        *old = new;
        *occupied = true;
    }
    // Keep any saved allocations not owned by live residents too. Native tag10
    // omits those, inactive resident bytes and the unused auxiliary tail.
    for (index, reference) in references.iter_mut().enumerate() {
        let mut row: [u8; 284] = slots[index * 284..(index + 1) * 284]
            .try_into()
            .map_err(|_| "V2 resident auxiliary size invalid")?;
        if *reference == 0 {
            let slot = auxiliary
                .allocate(&row)
                .ok_or("V2 resident auxiliary pool exhausted")?;
            *reference = slot as u32 + 1;
        } else {
            let dst = auxiliary
                .get_mut((*reference - 1) as usize)
                .ok_or("native resident auxiliary disappeared")?;
            // Native load owns world position, movement yaw and visibility.
            // Retain the saved visual model, animation and other runtime bytes.
            row[4..16].copy_from_slice(&dst[4..16]);
            row[0x14..0x18].copy_from_slice(&dst[0x14..0x18]);
            row[0xb8] = (row[0xb8] & !1) | (dst[0xb8] & 1);
            *dst = row;
        }
    }
    let tail = auxiliary.used() * 284;
    auxiliary.slots_mut().as_flattened_mut()[tail..].copy_from_slice(&slots[tail..]);
    for (native, saved) in residents.nodes.iter_mut().zip(saved) {
        if !native.is_active() {
            let reference = saved.auxiliary_reference();
            let reference = if reference == 0 {
                0
            } else {
                remap_resident_auxiliary(reference, &references)?
            };
            *native =
                Resident::from_runtime_bytes(*saved.as_bytes(), saved.active_byte(), reference);
        }
    }
    Ok(references)
}

fn remap_resident_auxiliary(reference: u32, references: &[u32]) -> Result<u32, String> {
    reference
        .checked_sub(1)
        .and_then(|slot| references.get(slot as usize))
        .copied()
        .ok_or_else(|| "V2 resident reference leaves allocated auxiliary pool".into())
}

/// Native load owns the saved account prefix and the AI reset/bootstrap.
/// The extension still supplies common runtime auxiliaries and embedded RNG
/// state absent from native records. Live auction continuation has its own
/// validated activation snapshot and retains that existing policy.
pub fn restore_player_runtime(
    player: &mut crate::player_manager::Player,
    saved: &[u8],
    auction: bool,
) -> Result<(), String> {
    if saved.len() != player.bytes.len() || saved.len() < 0x224 {
        return Err("V2 player runtime size invalid".into());
    }
    if auction {
        player.bytes.copy_from_slice(saved);
    } else {
        player.bytes[0x1bc..0x220].copy_from_slice(&saved[0x1bc..0x220]);
        if word(&player.bytes, 0x154) == 1 {
            if saved.len() < 0x238 {
                return Err("V2 AI runtime size invalid".into());
            }
            player.bytes[0x224..0x238].copy_from_slice(&saved[0x224..0x238]);
        } else {
            player.bytes[0x220..].copy_from_slice(&saved[0x220..]);
        }
    }
    Ok(())
}

/// Read the legacy cache payload even when native load has invalidated it.
pub fn restore_market_runtime(
    input: &mut Input<'_>,
    market: &mut crate::market::Cache,
    auction: bool,
) -> Result<(), String> {
    let dirty = match input.word()? {
        0 => false,
        1 => true,
        _ => return Err("V2 market flag invalid".into()),
    };
    let mut prices = [0.; 32];
    for value in &mut prices {
        *value = f32::from_bits(input.word()?);
    }
    let rent = f32::from_bits(input.word()?);
    if auction {
        *market = crate::market::Cache {
            dirty,
            prices,
            rent,
        };
    } else {
        market.invalidate();
    }
    Ok(())
}

/// One tag-4000 record as 0x481010 writes it: the 24-byte queue header
/// (+0 kind, +4 clock-list selector, +8 outer due) and, for kind-zero
/// packet nodes, the complete command object.
pub fn scheduled_record(
    domain: crate::scheduler::ClockDomain,
    outer: u32,
    admission: u32,
    direct: bool,
    command: &Command,
) -> Result<([u8; 24], Vec<u8>), String> {
    let mut header = [0; 24];
    let selector = match domain {
        crate::scheduler::ClockDomain::Total => 0,
        crate::scheduler::ClockDomain::Normal => 1,
    };
    put(&mut header, 4, selector);
    put(&mut header, 8, outer);
    let packet = if let Command::CategoryReady { category } = command {
        if !direct {
            return Err("non-direct category queue encoding unsupported".into());
        }
        put(&mut header, 0, 1);
        put(&mut header, 12, *category);
        vec![]
    } else {
        if direct {
            return Err("direct packet queue encoding unsupported".into());
        }
        packet(command, admission)?
    };
    Ok((header, packet))
}

/// Size of the original object behind an auction-family command kind, as
/// returned by its class's size method (vtable slot 0). 481010 saves exactly
/// this many bytes and 4810e0 restores the stated size.
pub fn auction_command_size(kind: u32) -> Option<usize> {
    Some(match kind {
        // Property transfer: 464bf0/48c1c0, vtable 525c60, size 464e20.
        0x16 => 28,
        // Category wait: 445cc0, vtable 525b14, size 445d00.
        0x19 => 24,
        // Auction admission request: 48b350/48c1c0, vtable 525cb8, size 477da0.
        0x1d => 52,
        // Room start: 465330, vtable 525c70, size 465480.
        0x1e => 20,
        // Auction action: 4644a0..464580, vtable 525c58, size 4644e0.
        0x1f => 32,
        // Auction finish: 464f60, vtable 525c68, size 465220.
        0x20 => 20,
        // Auction inbox notifications: vtable 525dc0, size 48a350.
        0x27..=0x2a => 48,
        _ => return None,
    })
}

pub fn packet(command: &Command, due: u32) -> Result<Vec<u8>, String> {
    let mut bytes = match command {
        // 4896e0 and 489df0 build the result command (kind 9, vtable
        // 525db8, size 0x1c from 489560): winner at +0x14, reason at +0x18.
        // 48cc50 builds the removal command (kind 10, vtable 525e10, size
        // 0x18 from 48cc90) with the player slot at +0x14. Both clear the
        // relative delay at +0xc and set flag +0x10 before 450490 routes
        // them; 481010 saves the whole object, vtable word included.
        Command::OutcomeNotice { player, reason } => {
            let mut b = vec![0; 28];
            put(&mut b, 0, 0x525db8);
            put(&mut b, 4, 9);
            b[16] = 1;
            put(&mut b, 20, *player as u32);
            put(&mut b, 24, *reason);
            b
        }
        Command::Eliminate { slot } => {
            let mut b = vec![0; 24];
            put(&mut b, 0, 0x525e10);
            put(&mut b, 4, 10);
            b[16] = 1;
            put(
                &mut b,
                20,
                u32::try_from(*slot).map_err(|_| "elimination slot exceeds DWORD")?,
            );
            b
        }
        Command::Build(c) => c.packet().to_vec(),
        Command::Demolish(c) => c.packet().to_vec(),
        Command::Open(c) => {
            let mut b = [0; 36];
            c.write_packet(&mut b);
            b.to_vec()
        }
        Command::Convert(c) => {
            let mut b = [0; 32];
            c.write_packet(&mut b);
            b.to_vec()
        }
        Command::SellBusiness(c) => {
            let mut b = [0; 28];
            c.write_packet(&mut b);
            b.to_vec()
        }
        Command::Takeover(c) => {
            let mut b = [0; 24];
            c.write_packet(&mut b);
            b.to_vec()
        }
        Command::Terms(c) => {
            let mut b = vec![0; 64];
            put(&mut b, 4, 0x18);
            put(&mut b, 20, c.business_index);
            for (i, v) in c.terms.stock_targets.iter().enumerate() {
                b[24 + i * 2..26 + i * 2].copy_from_slice(&v.to_le_bytes());
            }
            for (i, v) in c.terms.prices.iter().enumerate() {
                put(&mut b, 36 + i * 4, v.to_bits());
            }
            put(&mut b, 60, c.terms.rent.to_bits());
            b
        }
        Command::EventCard(c) => {
            let mut b = vec![0; 32];
            put(&mut b, 0, 0x52515c);
            put(&mut b, 4, 0x1a);
            b[16] = 1;
            put(&mut b, 20, c.card);
            put(&mut b, 24, c.player);
            put(&mut b, 28, c.target as u32);
            b
        }
        // The live auction adapter carries every auction-family command as
        // a zero-padded 52-byte descriptor, but 481010 saves only the
        // original object, whose size is its class's own size method. A
        // descriptor with nonzero bytes past that size keeps the padded form
        // rather than losing them.
        Command::Auction { descriptor } => match auction_command_size(word(descriptor, 4)) {
            Some(size) if descriptor[size..].iter().all(|&b| b == 0) => descriptor[..size].to_vec(),
            _ => descriptor.to_vec(),
        },
        // Restored from a native 28-byte transfer; written as 464bf0 and
        // 48c1c0 build it (vtable 525c60, scheduling flag set).
        Command::PropertyTransfer {
            category,
            owner,
            amount,
        } => {
            let mut b = vec![0; 28];
            put(&mut b, 0, 0x525c60);
            put(&mut b, 4, 0x16);
            b[16] = 1;
            put(&mut b, 20, *amount as u32);
            b[24..26].copy_from_slice(&category.to_le_bytes());
            b[26] = *owner;
            b
        }
        _ => {
            return Err(
                "save encountered a queued command without a checked native encoding".into(),
            );
        }
    };
    let n = u32::try_from(bytes.len()).map_err(|_| "packet exceeds DWORD")?;
    put(&mut bytes, 8, n);
    put(&mut bytes, 12, due);
    Ok(bytes)
}

/// Verify the native stream's queue against the authoritative modern runtime
/// queue before deferring a due-now command to its live consumer.
/// Original files and legacy VERSION1 retain their original admission gates;
/// VERSION1 still must agree with the native queue before it can replace it.
pub fn verified_modern_presentation_queue(save: &SaveFile<'_>) -> Result<Vec<bool>, String> {
    let native = save.scheduled()?;
    let mut deferred = vec![false; native.len()];
    let Some(ext) = extension::Extension::parse(save)? else {
        return Ok(deferred);
    };
    ext.validate_runtime_section_set()?;
    let modern_presentation = matches!(ext.version, 2..=6);
    let clock = save.clock()?;
    let mut input = Input::new(ext.required(9)?);
    let count = input.word()? as usize;
    if count != native.len() || count > 4096 {
        return Err("Extension queue count differs from native stream".into());
    }
    for (index, event) in native.iter().enumerate() {
        let domain = input.word()?;
        let outer = input.word()?;
        let admission = input.word()?;
        let direct = input.word()?;
        let header = input.blob()?;
        let bytes = input.blob()?;
        if domain > 1
            || direct > 1
            || header != event.header
            || bytes != event.packet.unwrap_or(&[])
            || domain != word(event.header, 4)
            || outer != word(event.header, 8)
        {
            return Err("Extension queue identity differs from native stream".into());
        }
        if direct == 1 {
            if word(event.header, 0) != 1 || !bytes.is_empty() {
                return Err("Extension direct queue kind invalid".into());
            }
            // A modern save is a paused runtime snapshot. Native load would
            // consume an already-due category-ready node; record9 must keep
            // it for the first resumed worker drain instead.
            deferred[index] = modern_presentation;
        } else {
            if word(event.header, 0) != 0 || bytes.len() < 16 || admission != word(bytes, 12) {
                return Err("Extension packet admission differs from native stream".into());
            }
            let outer_due = outer == u32::MAX
                || if domain == 0 {
                    outer == clock.timing[0]
                } else {
                    (outer as i32) <= (clock.timing[1] as i32)
                };
            let packet_due = admission <= clock.timing[0];
            deferred[index] = if modern_presentation {
                match crate::session::restore::saved_command(bytes)? {
                    Command::SellBusiness(_) => true,
                    Command::PropertyTransfer { .. } => true,
                    Command::Terms(_) => true,
                    Command::Takeover(command) => {
                        // 450a30's unsigned future gate admits every non-sentinel
                        // deadline at or before the saved total clock. The native
                        // load stream would dispatch an overdue packet here too.
                        if outer_due && packet_due {
                            if save.clock()?.timing[5] != 1 || save.auction()?.is_some() {
                                return Err(
                                    "V2 due-now takeover requires normal non-auction continuation"
                                        .into(),
                                );
                            }
                            let pool = ext.required(4)?;
                            if pool.len() != 600 * 916 {
                                return Err("V2 takeover building pool size differs".into());
                            }
                            let row = pool
                                .chunks_exact(916)
                                .nth(command.building as usize)
                                .ok_or("V2 takeover target outside building pool")?;
                            let native = save
                                .buildings()?
                                .into_iter()
                                .find(|b| b.slot == command.building as usize)
                                .ok_or("V2 takeover target is not a retained native building")?;
                            if &row[..64] != native.bytes {
                                return Err(
                                    "V2 takeover target differs from native building prefix".into(),
                                );
                            }
                            // Load rebuilds model binding+4c instead of restoring
                            // that source word. Only the linked structure branch
                            // suppresses485520 independently of the new binding.
                            if word(row, 0x24) <= 1 {
                                return Err("V2 due-now takeover requires unverified synchronous appearance".into());
                            }
                            let business = save
                                .economy()?
                                .businesses()
                                .nth(word(row, 0x1c) as usize)
                                .ok_or("V2 takeover requires a live native business")?;
                            if business.business_type() >= 45 {
                                return Err("V2 takeover business leaves retained catalog".into());
                            }
                            true
                        } else {
                            false
                        }
                    }
                    _ if outer_due && packet_due => {
                        return Err(format!(
                            "Cannot save due-now command {:02x}: load-stage continuation is unverified",
                            word(bytes, 4),
                        ));
                    }
                    _ => false,
                }
            } else {
                false
            };
        }
    }
    input.end()?;
    Ok(deferred)
}

pub struct Metadata {
    pub profile_filename: Option<String>,
    /// V2 host view, independent of the clock's logic mode.
    pub interface_mode: Option<u32>,
    pub auction: Option<AuctionContinuation>,
}
fn valid_profile(name: &str) -> bool {
    name.is_empty()
        || name.strip_suffix(".lua").is_some_and(|stem| {
            stem.len().is_multiple_of(2)
                && stem
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
}
impl<'data> Session<'data> {
    pub fn save_v2(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
    ) -> Result<Vec<u8>, String> {
        self.save_v2_with_auction(image, cameras, game_mode, None)
    }
    pub fn save_v2_with_auction(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
    ) -> Result<Vec<u8>, String> {
        let interface_mode = word(&self.world.auction.controller.record, 0x150);
        self.save_v2_with_auction_view(image, cameras, game_mode, host, interface_mode)
    }
    pub fn save_v2_with_auction_view(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String> {
        self.save_v2_with_auction_view_inner(image, cameras, game_mode, host, interface_mode, None)
    }
    /// UI SaveSelected owns the actual worker Host. Serialize its cold-load
    /// continuation without dropping the live Options hold before the cut.
    /// `view` is the Host's projection of this Session (`Host::save_view`).
    pub fn save_v2_with_options_view(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        view: HostSaveView,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String> {
        let continuation = view
            .options
            .map(|(pause, timers)| OptionsContinuation { pause, timers });
        self.save_v2_with_auction_view_inner(
            image,
            cameras,
            game_mode,
            view.auction,
            interface_mode,
            continuation,
        )
    }
    fn save_v2_with_auction_view_inner(
        &mut self,
        image: &crate::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
        continuation: Option<OptionsContinuation>,
    ) -> Result<Vec<u8>, String> {
        if self.tutorial_state != 0 {
            return Err("Manual Save is disabled in tutorials".into());
        }
        SaveProjection {
            live_commands: &self.live_commands,
            city: &mut self.world.city,
            clock: &self.world.clock,
            utilities: &self.hooks.utilities,
            frequency: &self.hooks.frequency,
            timers: &self.hooks.timers,
            scripts: &mut self.scripts,
            roster: &mut self.roster,
            population: &self.population,
            road_traffic: &self.road_traffic,
            transport: &self.transport,
            catalog: &self.catalog,
            campaign_profile: &self.campaign_profile,
            skirmish_map: &self.skirmish_map,
            saved_player_metadata: &self.saved_player_metadata,
            history: &self.history,
            random_registry: &self.random_registry,
            event_cards: &self.event_cards,
            pulse_lights: &self.pulse_lights,
            demolition_effects: &self.demolition_effects,
            policy: &self.policy,
            session_option_word: self.session_option_word,
            difficulty: self.difficulty,
            tutorial_state: self.tutorial_state,
            result_present: self.result_presentation.is_some(),
            script_random_state: self.script_random_state,
            credit_random_state: self.credit_random_state,
            particle_random_state: self.particle_random_state,
            allocation_next: self.allocation_next,
            static_rng_tail_next: self.static_rng_tail_next,
            build_random_state: self.build_random_state,
            building_selection: self.building_selection,
        }
        .encode_continuation(
            image,
            cameras,
            game_mode,
            host,
            interface_mode,
            continuation,
        )
    }

    /// Apply only after checked native restoration has reconstructed geometry
    /// and original scripts. Host authenticates profile association separately.
    pub fn restore_v2_extension_with_image(
        &mut self,
        save: &SaveFile<'_>,
        image: &crate::pe::PeImage<'_>,
    ) -> Result<Option<Metadata>, String> {
        let metadata = self.restore_v2_extension(save)?;
        self.demolition_points = self.demolition_effects.points(image)?;
        Ok(metadata)
    }

    pub fn restore_v2_extension(
        &mut self,
        save: &SaveFile<'_>,
    ) -> Result<Option<Metadata>, String> {
        let Some(ext) = extension::Extension::parse(save)? else {
            return Ok(None);
        };
        // This public restoration entry point can also be called separately
        // from load_original. Never install record9 without checking that it
        // names the same native queue and packet deadlines.
        verified_modern_presentation_queue(save)?;
        let restored_map = if matches!(ext.version, 5 | 6) {
            let selection =
                crate::skirmish_maps::Selection::decode(ext.version, ext.required(18)?)?;
            let selected = self
                .catalog
                .selected_level()
                .ok_or("Skirmish selection absent")?;
            if selection.level_id != selected.id()
                || selected.single_player()
                || selection.target.to_bits() != selected.objective_target().to_bits()
                || !selected
                    .directory()
                    .eq_ignore_ascii_case(format!("net{}", selection.level_id - 300000).as_bytes())
            {
                return Err("Skirmish extension differs from prepared selection".into());
            }
            if self.skirmish_map.as_ref().is_some_and(|map| {
                !matches!(map, crate::skirmish_maps::MapIdentity::MaterializedUnknown)
                    && map != &selection.map
            }) {
                return Err("Skirmish map differs from prepared provenance".into());
            }
            Some(selection.map)
        } else {
            None
        };
        let has_auction = ext.records.contains_key(&14);
        let has_view = ext.records.contains_key(&15);
        ext.validate_runtime_section_set()?;
        let build_random_state = if ext.version >= 3 {
            let mut state = Input::new(ext.required(16)?);
            let value = state.word()?;
            state.end()?;
            Some(value)
        } else {
            None
        };
        let demolition_effects = if ext.version >= 4 {
            Some(crate::save::demolition::decode(ext.required(17)?)?)
        } else {
            None
        };
        let interface_mode = if has_view {
            if ext.records.contains_key(&14)
                || save.clock()?.timing[5] != 1
                || save.auction()?.is_some()
            {
                return Err("V2 normal view disagrees with native game/auction mode".into());
            }
            let mut view = Input::new(ext.required(15)?);
            let mode = view.word()?;
            view.end()?;
            if !matches!(mode, 0 | 3) {
                return Err("V2 normal view is unsupported".into());
            }
            Some(mode)
        } else {
            None
        };
        let mut auction = if has_auction {
            let auction = AuctionContinuation::decode(ext.required(14)?)?;
            auction.validate_native(save)?;
            Some(auction)
        } else {
            None
        };
        let profile =
            std::str::from_utf8(ext.required(1)?).map_err(|_| "V2 profile name is not UTF-8")?;
        if !valid_profile(profile) {
            return Err("invalid V2 profile filename".into());
        }
        let profile_filename = (!profile.is_empty()).then(|| profile.to_owned());
        let mut p = Input::new(ext.required(2)?);
        let native_players = save.players()?;
        let count = p.word()? as usize;
        if count != self.roster.manager.players.len() {
            return Err("V2 player roster differs from native records".into());
        }
        self.roster.manager.bytes.copy_from_slice(p.take(40)?);
        self.roster.status.current_player = p.word()?;
        for (player, native) in self.roster.manager.players.iter_mut().zip(&native_players) {
            let reference = p.word()?;
            let bytes = p.blob()?;
            if reference != player.reference || bytes.len() != player.bytes.len() {
                return Err("V2 player allocation differs from checked native constructor".into());
            }
            if bytes.get(4..444) != Some(native.bytes.as_slice()) {
                return Err("V2/native player account records disagree".into());
            }
            restore_player_runtime(player, bytes, has_auction)?;
            let n = p.word()? as usize;
            if n > 4096 {
                return Err("V2 player auxiliary count exceeds retained bound".into());
            }
            player.auxiliaries.clear();
            for _ in 0..n {
                player.auxiliaries.push(crate::player_manager::Auxiliary {
                    reference: p.word()?,
                    bytes: p.take(48)?.try_into().unwrap(),
                });
            }
        }
        p.end()?;
        if !self
            .roster
            .manager
            .players
            .iter()
            .any(|player| player.reference == self.roster.status.current_player)
        {
            return Err("V2 current player is absent from restored roster".into());
        }
        let mut r = Input::new(ext.required(3)?);
        self.random_registry.process = [r.word()?, r.word()?, r.word()?];
        self.random_registry.head = r.word()?;
        self.random_registry.tail = r.word()?;
        self.script_random_state = r.word()?;
        self.credit_random_state = r.word()?;
        self.particle_random_state = r.word()?;
        // Native AI ownership rebuilds can allocate beyond the saved arena.
        self.allocation_next = self.allocation_next.max(r.word()?);
        self.static_rng_tail_next = r.word()?;
        self.world.creation.initialization_rng.constructor = r.word()?;
        self.world.creation.initialization_rng.movement = r.word()?;
        self.world.creation.initialization_rng.objective = r.word()?;
        self.world.creation.identity_rng = r.word()?;
        self.world.creation.preference_rng = r.word()?;
        self.policy.seed = r.word()?;
        self.policy
            .auxiliary_prior_bytes
            .copy_from_slice(r.take(284)?);
        self.pulse_lights = crate::pulse_lights::PulseLights::from_bytes(r.take(96)?)?;
        r.end()?;
        let buildings = ext.required(4)?;
        if buildings.len() != self.world.buildings.len() * 916 {
            return Err("V2 building pool size differs".into());
        }
        // Native load already rebound model geometry and category links. Restore
        // non-pointer runtime state, retaining the new renderer's model handle.
        for (dst, src) in self
            .world
            .buildings
            .iter_mut()
            .zip(buildings.chunks_exact(916))
        {
            if dst[..64] != src[..64] {
                return Err("V2/native building prefixes disagree".into());
            }
            dst[0x58..0x80].copy_from_slice(&src[0x58..0x80]);
        }
        let city = &mut self.world.city;
        let resident_auxiliaries = restore_resident_runtime(
            &mut city.ledgers.residents,
            &mut city.creation.auxiliary,
            ext.required(5)?,
        )?;
        let mut a = Input::new(ext.required(6)?);
        let profiles = a.blob()?;
        if profiles.len() != self.roster.profiles.len() * 0x650 {
            return Err("V2 AI profile pool size invalid".into());
        }
        for (dst, src) in self
            .roster
            .profiles
            .iter_mut()
            .zip(profiles.chunks_exact(0x650))
        {
            if has_auction {
                dst.copy_from_slice(src);
            }
        }
        let counts = a.blob()?;
        if counts.len() != self.roster.category_counts.len() * 4 {
            return Err("V2 category count size invalid".into());
        }
        for (dst, src) in self
            .roster
            .category_counts
            .iter_mut()
            .zip(counts.chunks_exact(4))
        {
            if has_auction {
                *dst = word(src, 0);
            }
        }
        self.roster.proposal_identity = a.word()?;
        let n = a.word()?;
        if n > 12 {
            return Err("V2 ownership list count exceeds roster".into());
        }
        let mut ownership = std::collections::BTreeMap::new();
        for _ in 0..n {
            let reference = a.word()?;
            let b = a.blob()?;
            if b.len() % 28 != 0 || b.len() > 600 * 28 {
                return Err("V2 AI ownership list invalid".into());
            }
            if ownership
                .insert(
                    reference,
                    b.chunks_exact(28).map(|b| b.try_into().unwrap()).collect(),
                )
                .is_some()
            {
                return Err("duplicate V2 ownership reference".into());
            }
        }
        if has_auction {
            self.roster.ai_ownership.lists = ownership;
        }
        let n = a.word()?;
        if n > 6 {
            return Err("V2 liquidation target count invalid".into());
        }
        self.roster.liquidation_targets.clear();
        for _ in 0..n {
            let reference = a.word()?;
            let bytes = a.take(12)?.try_into().unwrap();
            if has_auction {
                self.roster.liquidation_targets.insert(reference, bytes);
            }
        }
        let demand = a.take(33 * 264)?;
        let ready = u8::try_from(a.word()?).map_err(|_| "V2 demand ready byte invalid")?;
        if has_auction {
            self.world
                .ai_demand
                .rows
                .as_flattened_mut()
                .copy_from_slice(demand);
            self.world.ai_demand.ready = ready;
        }
        a.end()?;
        let mut a = Input::new(ext.required(7)?);
        let cursor = a.word()?;
        self.world.available.clear();
        while a.at < a.bytes.len() {
            let slot = a.word()? as usize;
            if slot >= 600 {
                return Err("V2 available business leaves pool".into());
            }
            self.world.available.push(slot);
        }
        self.world.availability_cursor = (cursor != u32::MAX).then_some(cursor as usize);
        a.end()?;
        let mut d = Input::new(ext.required(8)?);
        self.event_cards.rng = [d.word()?, d.word()?, d.word()?];
        for c in &mut self.event_cards.cards {
            *c = d.word()?;
        }
        self.event_cards.remaining = d.word()?;
        self.event_cards.used.copy_from_slice(d.take(83)?);
        d.end()?;
        if self.event_cards.remaining > 83 {
            return Err("V2 event deck count invalid".into());
        }
        let mut q = Input::new(ext.required(9)?);
        let n = q.word()?;
        if n > 4096 {
            return Err("V2 live queue exceeds dispatch bound".into());
        }
        let mut queue = crate::live_commands::Queue::default();
        for _ in 0..n {
            let domain = match q.word()? {
                0 => crate::scheduler::ClockDomain::Total,
                1 => crate::scheduler::ClockDomain::Normal,
                _ => return Err("V2 queue clock domain invalid".into()),
            };
            let outer = q.word()?;
            let admission = q.word()?;
            let direct = match q.word()? {
                0 => false,
                1 => true,
                _ => return Err("V2 queue direct flag invalid".into()),
            };
            let h = q.blob()?;
            let b = q.blob()?;
            if h.len() != 24 {
                return Err("V2 queue header size invalid".into());
            }
            let command = if direct {
                if word(h, 0) != 1 || !b.is_empty() {
                    return Err("V2 direct queue kind invalid".into());
                }
                Command::CategoryReady {
                    category: word(h, 12),
                }
            } else {
                crate::session::restore::saved_command(b)?
            };
            queue.restore_queued(command, domain, outer, admission, direct);
        }
        q.end()?;
        self.live_commands = queue;
        let mut runtime = Input::new(ext.required(10)?);
        // VERSION1+ contains old caches for wire compatibility. Normal loads
        // retain 4b9f00/4bbd20's rebuilt values and script scores instead.
        let neighbors = runtime.take(52 * 56)?;
        let mut owned = [0; 6];
        let mut groups = [0; 6];
        for value in &mut owned {
            *value = runtime.word()? as i32;
        }
        for value in &mut groups {
            *value = runtime.word()? as i32;
        }
        let ownership_age =
            u8::try_from(runtime.word()?).map_err(|_| "V2 ownership counter invalid")?;
        let business_age =
            u8::try_from(runtime.word()?).map_err(|_| "V2 business counter age invalid")?;
        let mut business_counts = [0; 6];
        for value in &mut business_counts {
            *value = runtime.word()? as i32;
        }
        restore_market_runtime(&mut runtime, &mut self.population.market, has_auction)?;
        let scores = runtime.take(29)?;
        if has_auction {
            self.world
                .ai_neighbors
                .rows
                .as_flattened_mut()
                .copy_from_slice(neighbors);
            self.world.ai_neighbors.owned_properties = owned;
            self.world.ai_neighbors.completed_groups = groups;
            self.world.ownership_refresh_counter = ownership_age;
            self.population.business_counts.age = business_age;
            self.population.business_counts.values = business_counts;
            self.roster.scripts.scores.bytes.copy_from_slice(scores);
        }
        let animations = runtime.blob()?;
        if animations.len() % 44 != 0 {
            return Err("V2 animation pool size invalid".into());
        }
        let mut animations = animations.as_chunks::<44>().0.to_vec();
        for record in &mut animations {
            if record[4] != 0 && word(record, 0) <= 3 {
                let reference = remap_resident_auxiliary(word(record, 8), &resident_auxiliaries)?;
                put(record, 8, reference);
            }
        }
        self.population.animations =
            crate::resident::ResidentAnimationPool::from_records(animations)?;
        let n = runtime.word()?;
        if n > 2000 {
            return Err("V2 resident binding count invalid".into());
        }
        self.population.bindings.clear();
        for _ in 0..n {
            let resident_reference = runtime.word()?;
            let saved_reference = runtime.word()?;
            let saved_slot = runtime.word()? as usize;
            let model_frame_count =
                u16::try_from(runtime.word()?).map_err(|_| "V2 model frame count invalid")?;
            if saved_reference.checked_sub(1).map(|slot| slot as usize) != Some(saved_slot) {
                return Err("V2 resident auxiliary binding disagrees with slot".into());
            }
            let auxiliary_reference =
                remap_resident_auxiliary(saved_reference, &resident_auxiliaries)?;
            let auxiliary_slot = (auxiliary_reference - 1) as usize;
            self.population
                .bindings
                .push(crate::shopping::ResidentBinding {
                    resident_reference,
                    auxiliary_reference,
                    auxiliary_slot,
                    model_frame_count,
                });
        }
        runtime.end()?;
        transport::restore(self, ext.required(11)?)?;
        let mut economy = Input::new(ext.required(12)?);
        for value in &mut self.hooks.utilities.owners {
            *value = u8::try_from(economy.word()?).map_err(|_| "V2 utility owner invalid")?;
        }
        for value in &mut self.hooks.utilities.enabled {
            *value = match economy.word()? {
                0 => false,
                1 => true,
                _ => return Err("V2 utility enabled flag invalid".into()),
            };
        }
        for value in &mut self.hooks.utilities.totals {
            *value = f32::from_bits(economy.word()?);
        }
        for value in &mut self.population.properties.owners {
            *value =
                u8::try_from(economy.word()?).map_err(|_| "V2 property income owner invalid")?;
        }
        for value in &mut self.population.properties.totals {
            *value = f32::from_bits(economy.word()?);
        }
        // Native completion has already released reason0 ONCE. Never replace
        // current process/load-script owners with those of the saved process.
        skip_saved_dispatch(&mut economy)?;
        self.building_selection = [economy.word()?, economy.word()?];
        for value in &mut self.world.construction_components {
            *value = f32::from_bits(economy.word()?);
        }
        for value in self.world.category_footprints.as_flattened_mut() {
            *value = economy.word()? as i32;
        }
        economy.end()?;
        let mut events = Input::new(ext.required(13)?);
        let count = events.word()?;
        if count > 4096 {
            return Err("V2 scenario callback count invalid".into());
        }
        let mut restored = Vec::new();
        for _ in 0..count {
            let name = std::str::from_utf8(events.blob()?)
                .map_err(|_| "V2 scenario callback name is not UTF-8")?;
            if !name.starts_with("EVENT_") || name.len() > 4096 {
                return Err("V2 scenario callback is not a named original event".into());
            }
            let expected = events.blob()?;
            let remaining = events.word()?;
            let argument = events.word()? as i32;
            let reference = self
                .scripts
                .runtime
                .reference_function(name)?
                .ok_or("Saved scenario callback is absent from replayed original scripts")?;
            let (bound, body) = self
                .scripts
                .runtime
                .event_reference_identity(reference)?
                .ok_or("Replayed scenario callback has no portable identity")?;
            if bound != name || body != expected {
                return Err(format!(
                    "Saved scenario callback {name} differs from original replayed body"
                ));
            }
            restored.push(crate::script_events::Event {
                reference,
                remaining,
                argument,
            });
        }
        events.end()?;
        for event in self.scripts.events.pending() {
            self.scripts.runtime.release_reference(event.reference)?;
        }
        self.scripts.events = crate::script_events::Events::default();
        for event in restored {
            self.scripts.events.append(event);
        }
        // Native import recreates resident placement and increments category
        // counters. Placement is a constructor side effect, independent of the
        // retained movement fixup; recover saved counters and both derived caches.
        for category in 0..52 {
            let count = save.category_resident_counter(category)?;
            put(&mut self.world.categories[category], 0x88, count);
            self.population.housing.categories[category].resident_counter = count;
            self.population.shopping.category_counts[category] = count;
        }
        self.roster.collect_accounts(&mut self.world.city)?;
        // Native saves and VERSION1/2 did not retain this stream. Their cold
        // startup state remains authoritative rather than inventing attempts.
        if let Some(state) = build_random_state {
            self.build_random_state = state;
        }
        if let Some(manager) = demolition_effects {
            validate_dust_bindings(&manager, &self.world.pending_world, &self.world.buildings)?;
            self.demolition_points.clear();
            self.demolition_effects = manager;
        }
        if let Some(continuation) = &mut auction {
            continuation.retain_activation_players(self);
        }
        if let Some(map) = restored_map {
            self.skirmish_map = Some(map);
        }
        Ok(Some(Metadata {
            profile_filename,
            interface_mode,
            auction,
        }))
    }
}

/// Disjoint live save owners. Autosave supplies callback Clock/City/Population,
/// moved hourly utilities/roads and the live message allocator. Models are not
/// serialized; their registrar remains with the running geometry transaction.
pub struct SaveProjection<'a> {
    pub live_commands: &'a crate::live_commands::Queue,
    pub city: &'a mut crate::level_startup::City,
    pub clock: &'a crate::clock::Clock,
    pub utilities: &'a crate::economy::Utilities,
    pub frequency: &'a crate::timers::CalendarFrequency,
    pub timers: &'a [crate::timers::Task],
    pub scripts: &'a mut crate::level_startup::Scripts,
    pub roster: &'a mut crate::level_startup::Roster,
    pub population: &'a crate::level_runtime::Population,
    pub road_traffic: &'a crate::traffic::road::runtime::RoadTraffic,
    pub transport: &'a crate::traffic::scene::Scene,
    pub catalog: &'a crate::level_catalog::Catalog,
    pub campaign_profile: &'a Option<std::path::PathBuf>,
    pub skirmish_map: &'a Option<crate::skirmish_maps::MapIdentity>,
    pub saved_player_metadata: &'a [[u8; 48]; 6],
    pub history: &'a crate::player_history::State,
    pub random_registry: &'a crate::player_manager::RandomRegistry,
    pub event_cards: &'a crate::event_cards::Deck,
    pub pulse_lights: &'a crate::pulse_lights::PulseLights,
    pub demolition_effects: &'a crate::demolition_effect::Manager,
    pub policy: &'a crate::session::Policy,
    pub session_option_word: u32,
    pub difficulty: usize,
    pub tutorial_state: u8,
    pub result_present: bool,
    pub script_random_state: u32,
    pub credit_random_state: u32,
    pub particle_random_state: u32,
    pub allocation_next: u32,
    pub static_rng_tail_next: u32,
    pub build_random_state: u32,
    pub building_selection: [u32; 2],
}
// Private serialized projection: produced only by the actual worker Host's
// owned Options operation. It is not a native owner, saved token or live state.
/// What a manual save needs from the live Host: the Options owner's
/// cold-load continuation (pause and timers with its one hold released) and
/// the auction presentation state.
pub struct HostSaveView {
    pub options: Option<(crate::pause::State, Vec<crate::timers::Task>)>,
    pub auction: Option<AuctionHostState>,
}
struct OptionsContinuation {
    pause: crate::pause::State,
    timers: Vec<crate::timers::Task>,
}
impl SaveProjection<'_> {
    pub fn encode(
        self,
        image: &crate::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String> {
        self.encode_continuation(image, cameras, game_mode, host, interface_mode, None)
    }
    fn encode_continuation(
        self,
        image: &crate::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
        continuation: Option<OptionsContinuation>,
    ) -> Result<Vec<u8>, String> {
        if self.tutorial_state > 1 {
            return Err("Unsupported tutorial state".into());
        }

        if self.result_present {
            return Err("Saving a result presentation is not yet supported".into());
        }
        if !matches!(interface_mode, 0 | 3) {
            return Err("Saving this interface view is unsupported".into());
        }
        let auction_active = self.city.auction.controller.reference != 0;
        if !auction_active && game_mode != 1 {
            return Err("Saving a modal game mode is not yet supported".into());
        }
        if auction_active {
            AuctionContinuation::capture_state(
                &self.city.auction,
                self.tutorial_state,
                game_mode,
                host.clone(),
                interface_mode,
            )?;
        }
        let mut events = Output(vec![]);
        // 4bcb90 reloads DEFAULT and the tutorial hub, then enters Level.
        // Tutorial prompts/globals restart there; old callbacks must not run
        // alongside that fresh entry. Other runtime extension state is retained.
        let pending_events = if self.tutorial_state == 1 {
            Vec::new()
        } else {
            self.scripts.events.pending()
        };
        if pending_events.len() > 4096 {
            return Err("V2 scenario callback queue exceeds retained bound".into());
        }
        events.word(pending_events.len() as u32);
        for event in pending_events {
            let (name, body) = self
                .scripts
                .runtime
                .event_reference_identity(event.reference)?
                .ok_or("Cannot save an anonymous, collected or replaced scenario callback")?;
            events.blob(name.as_bytes())?;
            events.blob(&body)?;
            events.word(event.remaining);
            events.word(event.argument as u32);
        }
        let level = self
            .catalog
            .selected_level()
            .ok_or("save has no selected level")?
            .id();
        let prefixes = self.roster.account_prefixes(&self.city)?;
        let current = self
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == self.roster.status.current_player)
            .ok_or("save current player missing")?;
        let player_rows: Vec<_> = prefixes
            .iter()
            .enumerate()
            .map(|(i, p)| writer::Player {
                metadata: &self.saved_player_metadata[i],
                prefix: p,
            })
            .collect();
        let residents = &self.city.ledgers.residents;
        let resident_rows: Vec<_> = residents
            .nodes
            .iter()
            .map(|n| writer::Resident {
                prefix: n.as_bytes(),
                active: n.active_byte(),
            })
            .collect();
        let accounts: Vec<_> = residents.accounts.iter().map(|n| *n.as_bytes()).collect();
        let mut headers = Vec::new();
        let mut packets = Vec::new();
        let mut queue = Output(vec![]);
        let snapshots = self.live_commands.snapshot()?;
        queue.word(snapshots.len() as u32);
        for (domain, outer, admission, direct, command) in snapshots {
            let (h, p) = scheduled_record(domain, outer, admission, direct, command)?;
            let domain = word(&h, 4);
            queue.word(domain);
            queue.word(outer);
            queue.word(admission);
            queue.word(u32::from(direct));
            queue.blob(&h)?;
            queue.blob(&p)?;
            headers.push(h);
            packets.push(p);
        }
        let scheduled: Vec<_> = headers
            .iter()
            .zip(&packets)
            .map(|(h, p)| writer::Scheduled {
                header: h,
                packet: (!p.is_empty()).then_some(p.as_slice()),
            })
            .collect();
        let pending: [u8; 3588] = self
            .city
            .pending_world
            .to_bytes()
            .try_into()
            .map_err(|_| "pending pool size mismatch")?;
        let objective = self.city.objective.to_bytes();
        let mut state = writer::State {
            compatibility: image.at(0x545d1c, 40)?.try_into().unwrap(),
            level,
            difficulty: self.difficulty as u32,
            current_cash: self.city.ledgers.players[current].cash,
            players: &player_rows,
            businesses: &self.city.ledgers.businesses,
            buildings: &self.city.buildings,
            building_base: 0x591318,
            building_sentinel_next: 0,
            categories: &mut self.city.categories,
            residents: &resident_rows,
            cameras: &cameras,
            calendar: self.clock.calendar,
            timing: self.clock.timing,
            pending: &pending,
            resident_accounts: &accounts,
            scheduled: &scheduled,
            auction: auction_active.then_some(writer::Auction {
                record: &mut self.city.auction.controller.record,
                countdown: self.city.auction.presentation.countdown,
            }),
            objective: &objective,
            business_info: self.city.catalog.info.rows(),
            commodities: &self.city.commodities.rows,
            history: self
                .history
                .as_bytes()
                .try_into()
                .map_err(|_| "history size mismatch")?,
            session_option_word: self.session_option_word,
        };
        let mut bytes = writer::encode(&mut state, 0x7fffff, [0; 4])?;
        let profile = self
            .campaign_profile
            .as_ref()
            .map(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or("V2 profile filename is not UTF-8")
            })
            .transpose()?;
        if !valid_profile(profile.unwrap_or("")) {
            return Err("V2 profile filename does not match campaign store encoding".into());
        }
        let mut players = Output(vec![]);
        players.word(self.roster.manager.players.len() as u32);
        players.0.extend_from_slice(&self.roster.manager.bytes);
        players.word(self.roster.status.current_player);
        for p in &self.roster.manager.players {
            players.word(p.reference);
            players.blob(&p.bytes)?;
            players.word(p.auxiliaries.len() as u32);
            for a in &p.auxiliaries {
                players.word(a.reference);
                players.0.extend_from_slice(&a.bytes);
            }
        }
        let mut ai = Output(vec![]);
        ai.blob(&flat(&self.roster.profiles))?;
        ai.blob(&words(self.roster.category_counts))?;
        ai.word(self.roster.proposal_identity);
        ai.word(self.roster.ai_ownership.lists.len() as u32);
        for (reference, rows) in &self.roster.ai_ownership.lists {
            ai.word(*reference);
            ai.blob(&flat(rows))?;
        }
        ai.word(self.roster.liquidation_targets.len() as u32);
        for (reference, record) in &self.roster.liquidation_targets {
            ai.word(*reference);
            ai.0.extend_from_slice(record);
        }
        ai.0.extend_from_slice(self.city.ai_demand.rows.as_flattened());
        ai.word(u32::from(self.city.ai_demand.ready));
        let mut rng = words(self.random_registry.process.into_iter().chain([
            self.random_registry.head,
            self.random_registry.tail,
            self.script_random_state,
            self.credit_random_state,
            self.particle_random_state,
            self.allocation_next,
            self.static_rng_tail_next,
            self.city.creation.initialization_rng.constructor,
            self.city.creation.initialization_rng.movement,
            self.city.creation.initialization_rng.objective,
            self.city.creation.identity_rng,
            self.city.creation.preference_rng,
        ]));
        rng.extend_from_slice(&self.policy.seed.to_le_bytes());
        rng.extend_from_slice(&self.policy.auxiliary_prior_bytes);
        rng.extend_from_slice(&self.pulse_lights.to_bytes());
        let mut resident_runtime = Output(vec![]);
        resident_runtime.word(residents.active_count);
        resident_runtime.word(residents.nodes.len() as u32);
        for n in &residents.nodes {
            resident_runtime.0.extend_from_slice(n.as_bytes());
            resident_runtime.word(u32::from(n.active_byte()));
            resident_runtime.word(n.auxiliary_reference());
        }
        resident_runtime.word(self.city.creation.auxiliary.used() as u32);
        resident_runtime.blob(&flat(self.city.creation.auxiliary.slots()))?;
        let mut deck = words(
            self.event_cards
                .rng
                .into_iter()
                .chain(self.event_cards.cards)
                .chain([self.event_cards.remaining]),
        );
        deck.extend_from_slice(&self.event_cards.used);
        let mut availability =
            words([self.city.availability_cursor.map_or(u32::MAX, |v| v as u32)]);
        availability.extend_from_slice(&words(self.city.available.iter().map(|v| *v as u32)));
        let mut runtime = Output(vec![]);
        runtime
            .0
            .extend_from_slice(self.city.ai_neighbors.rows.as_flattened());
        runtime.0.extend_from_slice(&words(
            self.city.ai_neighbors.owned_properties.map(|v| v as u32),
        ));
        runtime.0.extend_from_slice(&words(
            self.city.ai_neighbors.completed_groups.map(|v| v as u32),
        ));
        runtime.word(u32::from(self.city.ownership_refresh_counter));
        runtime.word(u32::from(self.population.business_counts.age));
        runtime.0.extend_from_slice(&words(
            self.population.business_counts.values.map(|v| v as u32),
        ));
        runtime.word(u32::from(self.population.market.dirty));
        runtime
            .0
            .extend_from_slice(&words(self.population.market.prices.map(f32::to_bits)));
        runtime.word(self.population.market.rent.to_bits());
        runtime
            .0
            .extend_from_slice(&self.roster.scripts.scores.bytes);
        runtime.blob(&flat(self.population.animations.records()))?;
        runtime.word(self.population.bindings.len() as u32);
        for binding in &self.population.bindings {
            runtime.word(binding.resident_reference);
            runtime.word(binding.auxiliary_reference);
            runtime.word(binding.auxiliary_slot as u32);
            runtime.word(u32::from(binding.model_frame_count));
        }
        let mut economy = Output(vec![]);
        economy
            .0
            .extend_from_slice(&words(self.utilities.owners.map(u32::from)));
        economy
            .0
            .extend_from_slice(&words(self.utilities.enabled.map(u32::from)));
        economy
            .0
            .extend_from_slice(&words(self.utilities.totals.map(f32::to_bits)));
        economy
            .0
            .extend_from_slice(&words(self.population.properties.owners.map(u32::from)));
        economy
            .0
            .extend_from_slice(&words(self.population.properties.totals.map(f32::to_bits)));
        let (pause, timers) = continuation
            .as_ref()
            .map_or((&self.city.pause, self.timers), |c| {
                (&c.pause, c.timers.as_slice())
            });
        // Keep record12's historical wire layout for existing readers. New
        // loaders consume these process fields without reinstating old owners.
        economy.0.extend_from_slice(&pause.bytes);
        economy.word(self.frequency.frequency.to_bits());
        economy.word(self.frequency.calendar_timer.map_or(u32::MAX, |v| v as u32));
        economy.word(self.frequency.logic_timer.map_or(u32::MAX, |v| v as u32));
        economy.blob(&flat(timers))?;
        economy.0.extend_from_slice(&words(self.building_selection));
        economy
            .0
            .extend_from_slice(&words(self.city.construction_components.map(f32::to_bits)));
        economy.0.extend_from_slice(&words(
            self.city
                .category_footprints
                .as_flattened()
                .iter()
                .map(|v| *v as u32),
        ));
        let auction = if auction_active {
            Some(AuctionContinuation::capture_state(
                &self.city.auction,
                self.tutorial_state,
                game_mode,
                host,
                interface_mode,
            )?)
        } else {
            None
        };
        let mut records = vec![
            (1, profile.unwrap_or("").as_bytes().to_vec()),
            (2, players.0),
            (3, rng),
            (4, flat(&self.city.buildings)),
            (5, resident_runtime.0),
            (6, ai.0),
            (7, availability),
            (8, deck),
            (9, queue.0),
            (10, runtime.0),
            (
                11,
                transport::encode_parts(
                    &self.city.road_network,
                    self.road_traffic,
                    self.transport,
                )?,
            ),
            (12, economy.0),
            (13, events.0),
        ];
        // VERSION3 adds the independent native placement RNG (560a08).
        // Older extension records and native serialization retain their layout.
        records.push((16, words([self.build_random_state])));
        validate_dust_bindings(
            self.demolition_effects,
            &self.city.pending_world,
            &self.city.buildings,
        )?;
        records.push((
            17,
            crate::save::demolition::encode(self.demolition_effects)?,
        ));
        if let Some(auction) = auction {
            records.push((14, auction.encode()));
        } else {
            // Preserve explicit normal as well as strategic UI authority. CityView
            // can leave raw kind4, so absence is not equivalent to explicit mode0.
            // Record15 already admits both values; auction record14 owns its view.
            records.push((15, words([interface_mode])));
        }
        let selected = self
            .catalog
            .selected_level()
            .ok_or("save has no selected level")?;
        let version = if (300000..=300006).contains(&selected.id()) {
            if selected.single_player()
                || !selected.objective_target().is_finite()
                || !selected
                    .directory()
                    .eq_ignore_ascii_case(format!("net{}", selected.id() - 300000).as_bytes())
            {
                return Err("unsupported saved Skirmish identity".into());
            }
            // Independent selected+870 and retained map provenance, never inferred
            // from the active objective or a reconstructed materialized city.
            let map = self
                .skirmish_map
                .clone()
                .ok_or("saved Skirmish provenance absent")?;
            let version = if matches!(map, crate::skirmish_maps::MapIdentity::LegacyEmpty) {
                5
            } else {
                6
            };
            records.push((
                18,
                crate::skirmish_maps::Selection {
                    level_id: selected.id(),
                    target: selected.objective_target(),
                    map,
                }
                .encode(version)?,
            ));
            version
        } else {
            4
        };
        extension::append_version(&mut bytes, &records, version)?;
        // Check new due-now presentation boundaries before host publication.
        verified_modern_presentation_queue(&SaveFile::parse(&bytes)?)?;
        Ok(bytes)
    }
}

/// Object-safe stack request; consuming the projection makes a second encode
/// an explicit error. It cannot outlive the synchronous calendar callback.
pub trait AutosaveRequest: std::fmt::Debug {
    fn encode_once(
        &mut self,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String>;
}
pub struct SaveRequest<'a, 'data> {
    pub projection: Option<SaveProjection<'a>>,
    pub image: &'a crate::pe::PeImage<'data>,
}
impl std::fmt::Debug for SaveRequest<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveRequest")
            .field("available", &self.projection.is_some())
            .finish()
    }
}
impl AutosaveRequest for SaveRequest<'_, '_> {
    fn encode_once(
        &mut self,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String> {
        let projection = self
            .projection
            .take()
            .ok_or("Autosave request already consumed")?;
        projection.encode(self.image, cameras, game_mode, host, interface_mode)
    }
}
