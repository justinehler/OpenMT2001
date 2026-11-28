//! Retained-session save encoding and V2 runtime restoration; no file I/O.
//! Native records use the checked 4bbe30 writer. The V2 extension preserves
//! runtime allocations/RNG that the native format deliberately omits.
use crate::fresh_city::Session;
#[path = "session_save_auction.rs"]
mod auction;
#[path = "session_save_transport.rs"]
mod transport;
pub use auction::{AuctionContinuation, AuctionHostState};
use openmt2001::{
    live_commands::Command,
    save::{SaveFile, extension, writer},
};

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn validate_dust_bindings(
    manager: &openmt2001::demolition_effect::Manager,
    pending: &openmt2001::pending_world::State,
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
fn words(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}
fn flat<const N: usize>(rows: &[[u8; N]]) -> Vec<u8> {
    rows.as_flattened().to_vec()
}
struct Output(Vec<u8>);
impl Output {
    fn word(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn blob(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.word(u32::try_from(bytes.len()).map_err(|_| "V2 blob exceeds DWORD")?);
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}
struct Input<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Input<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("V2 length overflow")?;
        let row = self
            .bytes
            .get(self.at..end)
            .ok_or("truncated V2 runtime state")?;
        self.at = end;
        Ok(row)
    }
    fn word(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn blob(&mut self) -> Result<&'a [u8], String> {
        let n = self.word()? as usize;
        self.take(n)
    }
    fn end(self) -> Result<(), String> {
        if self.at != self.bytes.len() {
            Err("trailing V2 runtime bytes".into())
        } else {
            Ok(())
        }
    }
}

/// Recover runtime data absent from tag10 without rewinding 45be80's movement
/// fixup or retiring the actor allocations it just created. Saved model/frame/
/// controller state is not recoverable from the native resident identity alone.
fn restore_resident_runtime(
    residents: &mut openmt2001::economy::ResidentAccounts,
    auxiliary: &mut openmt2001::resident::AuxiliaryPool,
    bytes: &[u8],
) -> Result<Vec<u32>, String> {
    use openmt2001::resident::Resident;
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

fn packet(command: &Command, due: u32) -> Result<Vec<u8>, String> {
    let mut bytes = match command {
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
        Command::Auction { descriptor } => descriptor.to_vec(),
        Command::PropertyTransfer {
            category,
            owner,
            amount,
        } => {
            let mut b = vec![0; 28];
            put(&mut b, 4, 0x16);
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
pub(crate) fn verified_modern_presentation_queue(save: &SaveFile<'_>) -> Result<Vec<bool>, String> {
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
                match crate::fresh_city::restore::saved_command(bytes)? {
                    Command::SellBusiness(_) => true,
                    Command::PropertyTransfer { .. } => true,
                    Command::Terms(_) => true,
                    Command::Takeover(command) => {
                        // 450a30's unsigned future gate admits every non-sentinel
                        // deadline at or before the saved total clock. The native
                        // load stream would dispatch an overdue packet here too.
                        if outer_due && packet_due {
                            if save.game_mode()? != 1 || save.auction()?.is_some() {
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
    /// V2 host view, independent of the original saved game-mode global.
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
        image: &openmt2001::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
    ) -> Result<Vec<u8>, String> {
        self.save_v2_with_auction(image, cameras, game_mode, None)
    }
    pub fn save_v2_with_auction(
        &mut self,
        image: &openmt2001::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
    ) -> Result<Vec<u8>, String> {
        let interface_mode = word(&self.world.auction.controller.record, 0x150);
        self.save_v2_with_auction_view(image, cameras, game_mode, host, interface_mode)
    }
    pub fn save_v2_with_auction_view(
        &mut self,
        image: &openmt2001::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String> {
        self.save_v2_with_auction_view_inner(image, cameras, game_mode, host, interface_mode, None)
    }
    /// UI SaveSelected owns the actual worker Host. Serialize its cold-load
    /// continuation without dropping the live Options hold before the cut.
    pub(crate) fn save_v2_with_options_view(
        &mut self,
        image: &openmt2001::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: &crate::live_host::Host,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String> {
        let continuation = host
            .options_save_projection(&self.world.pause, &self.hooks.timers)?
            .map(|(pause, timers)| OptionsContinuation { pause, timers });
        self.save_v2_with_auction_view_inner(
            image,
            cameras,
            game_mode,
            host.auction_save_state(),
            interface_mode,
            continuation,
        )
    }
    fn save_v2_with_auction_view_inner(
        &mut self,
        image: &openmt2001::pe::PeImage<'_>,
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
        image: &openmt2001::pe::PeImage<'_>,
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
            Some(openmt2001::save::demolition::decode(ext.required(17)?)?)
        } else {
            None
        };
        let interface_mode = if has_view {
            if ext.records.contains_key(&14) || save.game_mode()? != 1 || save.auction()?.is_some()
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
            player.bytes.copy_from_slice(bytes);
            let n = p.word()? as usize;
            if n > 4096 {
                return Err("V2 player auxiliary count exceeds retained bound".into());
            }
            player.auxiliaries.clear();
            for _ in 0..n {
                player
                    .auxiliaries
                    .push(openmt2001::player_manager::Auxiliary {
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
        self.allocation_next = r.word()?;
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
        self.pulse_lights = openmt2001::pulse_lights::PulseLights::from_bytes(r.take(96)?)?;
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
            dst.copy_from_slice(src);
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
            *dst = word(src, 0);
        }
        self.roster.proposal_identity = a.word()?;
        let n = a.word()?;
        if n > 12 {
            return Err("V2 ownership list count exceeds roster".into());
        }
        self.roster.ai_ownership.lists.clear();
        for _ in 0..n {
            let reference = a.word()?;
            let b = a.blob()?;
            if b.len() % 28 != 0 || b.len() > 600 * 28 {
                return Err("V2 AI ownership list invalid".into());
            }
            if self
                .roster
                .ai_ownership
                .lists
                .insert(
                    reference,
                    b.chunks_exact(28).map(|b| b.try_into().unwrap()).collect(),
                )
                .is_some()
            {
                return Err("duplicate V2 ownership reference".into());
            }
        }
        let n = a.word()?;
        if n > 6 {
            return Err("V2 liquidation target count invalid".into());
        }
        self.roster.liquidation_targets.clear();
        for _ in 0..n {
            let reference = a.word()?;
            let bytes = a.take(12)?.try_into().unwrap();
            self.roster.liquidation_targets.insert(reference, bytes);
        }
        self.world
            .ai_demand
            .rows
            .as_flattened_mut()
            .copy_from_slice(a.take(33 * 264)?);
        self.world.ai_demand.ready =
            u8::try_from(a.word()?).map_err(|_| "V2 demand ready byte invalid")?;
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
        let mut queue = openmt2001::live_commands::Queue::default();
        for _ in 0..n {
            let domain = match q.word()? {
                0 => openmt2001::scheduler::ClockDomain::Total,
                1 => openmt2001::scheduler::ClockDomain::Normal,
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
                crate::fresh_city::restore::saved_command(b)?
            };
            queue.restore_queued(command, domain, outer, admission, direct);
        }
        q.end()?;
        self.live_commands = queue;
        let mut runtime = Input::new(ext.required(10)?);
        self.world
            .ai_neighbors
            .rows
            .as_flattened_mut()
            .copy_from_slice(runtime.take(52 * 56)?);
        for value in &mut self.world.ai_neighbors.owned_properties {
            *value = runtime.word()? as i32;
        }
        for value in &mut self.world.ai_neighbors.completed_groups {
            *value = runtime.word()? as i32;
        }
        self.world.ownership_refresh_counter =
            u8::try_from(runtime.word()?).map_err(|_| "V2 ownership counter invalid")?;
        self.population.business_counts.age =
            u8::try_from(runtime.word()?).map_err(|_| "V2 business counter age invalid")?;
        for value in &mut self.population.business_counts.values {
            *value = runtime.word()? as i32;
        }
        self.population.market.dirty = match runtime.word()? {
            0 => false,
            1 => true,
            _ => return Err("V2 market flag invalid".into()),
        };
        for value in &mut self.population.market.prices {
            *value = f32::from_bits(runtime.word()?);
        }
        self.population.market.rent = f32::from_bits(runtime.word()?);
        self.roster
            .scripts
            .scores
            .bytes
            .copy_from_slice(runtime.take(29)?);
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
            openmt2001::resident::ResidentAnimationPool::from_records(animations)?;
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
                .push(openmt2001::shopping::ResidentBinding {
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
        self.world.pause.bytes.copy_from_slice(economy.take(9)?);
        self.hooks.frequency.frequency = f32::from_bits(economy.word()?);
        let calendar = economy.word()?;
        let logic = economy.word()?;
        self.hooks.frequency.calendar_timer = (calendar != u32::MAX).then_some(calendar as usize);
        self.hooks.frequency.logic_timer = (logic != u32::MAX).then_some(logic as usize);
        let timers = economy.blob()?;
        if timers.len() % 20 != 0 || timers.len() > 1000 * 20 {
            return Err("V2 timer pool size invalid".into());
        }
        self.hooks.timers = timers
            .chunks_exact(20)
            .map(|b| b.try_into().unwrap())
            .collect();
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
            restored.push(openmt2001::script_events::Event {
                reference,
                remaining,
                argument,
            });
        }
        events.end()?;
        for event in self.scripts.events.pending() {
            self.scripts.runtime.release_reference(event.reference)?;
        }
        self.scripts.events = openmt2001::script_events::Events::default();
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

#[cfg(all(test, feature = "capture"))]
#[path = "session_save_mixed_boundary_test.rs"]
mod mixed_boundary_test;

#[cfg(test)]
#[path = "session_save_ai_test.rs"]
mod ai_test;
#[cfg(all(test, feature = "capture"))]
#[path = "session_save_auction_test.rs"]
pub(crate) mod auction_test;

#[cfg(test)]
#[path = "session_save_terms_test.rs"]
mod terms_test;

#[cfg(test)]
#[path = "session_save_resident_test.rs"]
mod resident_test;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original game assets; owned Options save and cold Clock continuation"]
    fn options_save_cold_load_preserves_independent_pause_owners() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                use crate::fresh_city::{CommandAssets, Inputs, Policy};
                let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
                let exe = read("OPENMT2001_EXE");
                let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
                let board = openmt2001::board::Board::from_v14(&image).unwrap();
                let build = read("OPENMT2001_BUILD_FILE");
                let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
                let directory = read("OPENMT2001_ARCHIVE_DIR");
                let archive = read("OPENMT2001_ARCHIVE_BIN");
                let index =
                    openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
                        .unwrap();
                let parameters = read("OPENMT2001_PARAMETERS");
                let timetable = read("OPENMT2001_TIMETABLE");
                let default = openmt2001::fixtures::path("DEFAULT_SETTINGS");
                let scripts = default.parent().unwrap().parent().unwrap();
                let styles = crate::fresh_city::load_era_styles(
                    std::path::Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                        .parent()
                        .unwrap(),
                )
                .unwrap();
                let names = std::array::from_fn(|_| Vec::new());
                let inputs = || Inputs {
                    tree_locators: None,
                    image: &image,
                    board: &board,
                    file: &file,
                    people: openmt2001::people::PeopleModels::load(&image, &index, &archive)
                        .unwrap(),
                    vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                        &image, &index, &archive,
                    )
                    .unwrap(),
                    transport_models: openmt2001::traffic::models::Models::from_original(
                        &image, &index, &archive,
                    )
                    .unwrap(),
                    road_routes: read("OPENMT2001_ROAD_ROUTE_TABLE"),
                    scripts,
                    parameters: std::str::from_utf8(&parameters).unwrap(),
                    era_styles: styles.clone(),
                    timetable: std::str::from_utf8(&timetable).unwrap(),
                    scenario: "SCENARIO3",
                    difficulty: 0,
                    selected_character: 4,
                    profile_name: b"Save fixture",
                    character_names: &names,
                };
                let policy = || Policy {
                    seed: 0x12345678,
                    auxiliary_prior_bytes: [0; 284],
                };
                let mut session = Session::initialize(inputs(), policy()).unwrap();

                let assets = CommandAssets {
                    image: &image,
                    board: &board,
                    file: &file,
                };
                let camera = openmt2001::flyby::CameraSample {
                    eye: [100., 120., 100.],
                    target: [0., 0., 0.],
                    up: [0., 1., 0.],
                    vertical_fov: 0.7,
                };
                let cameras = crate::save_menu::cameras(None, camera).unwrap();
                // These real Session saves exercise serialization/restore and a fresh
                // Host's actual registered logic/Clock. Camera/UI admission stays a
                // separate native containing regression, not a synthetic capture.
                for other in [None, Some(0usize), Some(1usize)] {
                    if let Some(reason) = other {
                        session
                            .world
                            .pause
                            .pause(reason, |c, _| {
                                c.apply(&mut session.hooks.timers);
                                Ok(())
                            })
                            .unwrap();
                    }
                    let expected_pause = session.world.pause.clone();
                    let expected_timers = session.hooks.timers.clone();
                    let mut host = crate::live_host::Host::new(camera.eye);
                    host.set_running(true);
                    host.options(&mut session.world.pause, &mut session.hooks.timers, true)
                        .unwrap();
                    assert!(host.options_save_allowed(&session.world.pause, &session.hooks.timers));
                    let live_pause = session.world.pause.clone();
                    let live_timers = session.hooks.timers.clone();
                    let bytes = session
                        .save_v2_with_options_view(&image, cameras, 1, &host, 0)
                        .unwrap();
                    assert_eq!(session.world.pause, live_pause);
                    assert_eq!(session.hooks.timers, live_timers);
                    assert!(!host.running);
                    assert!(host.options_save_allowed(&session.world.pause, &session.hooks.timers));
                    let save = SaveFile::parse(&bytes).unwrap();
                    let mut cold = Session::load_original(inputs(), policy(), &bytes).unwrap();
                    cold.restore_v2_extension_with_image(&save, &image).unwrap();
                    assert_eq!(cold.saved_cameras, Some(cameras));
                    assert_eq!(cold.world.pause, expected_pause);
                    assert_eq!(cold.hooks.timers, expected_timers);
                    let mut fresh = crate::live_host::Host::new(camera.eye);
                    fresh.set_running(true);
                    assert!(!fresh.options_save_allowed(&cold.world.pause, &cold.hooks.timers));
                    let tick = cold.world.clock.timing[0];
                    fresh.frame(&mut cold, &assets, 40);
                    assert!(fresh.failure.is_none(), "{:?}", fresh.failure);
                    if let Some(reason) = other {
                        assert_eq!(cold.world.clock.timing[0], tick);
                        assert_eq!(cold.world.pause, expected_pause);
                        cold.world
                            .pause
                            .unpause(reason, |c, _| {
                                c.apply(&mut cold.hooks.timers);
                                Ok(())
                            })
                            .unwrap();
                        fresh.frame(&mut cold, &assets, 40);
                        assert!(fresh.failure.is_none(), "{:?}", fresh.failure);
                    }
                    assert!(
                        cold.world.clock.timing[0] > tick,
                        "cold logic must advance without opening/closing a new Options owner"
                    );
                    host.options(&mut session.world.pause, &mut session.hooks.timers, false)
                        .unwrap();
                    assert_eq!(session.world.pause, expected_pause);
                    assert_eq!(session.hooks.timers, expected_timers);
                    if let Some(reason) = other {
                        session
                            .world
                            .pause
                            .unpause(reason, |c, _| {
                                c.apply(&mut session.hooks.timers);
                                Ok(())
                            })
                            .unwrap();
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
    #[test]
    #[ignore = "requires original v1.4 game assets and audited scripts"]
    fn retained_save_host_roundtrip_and_delayed_terms() {
        std::thread::Builder::new().stack_size(32*1024*1024).spawn(||{
            use crate::fresh_city::{Inputs,Policy,CommandAssets};
            let read=|key|std::fs::read(openmt2001::fixtures::path(key)).unwrap();
            let exe=read("OPENMT2001_EXE");let image=openmt2001::pe::PeImage::parse(&exe).unwrap();
            let board=openmt2001::board::Board::from_v14(&image).unwrap();let build=read("OPENMT2001_BUILD_FILE");let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let directory=read("OPENMT2001_ARCHIVE_DIR");let archive=read("OPENMT2001_ARCHIVE_BIN");let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
            let parameters=read("OPENMT2001_PARAMETERS");let timetable=read("OPENMT2001_TIMETABLE");
            let default=openmt2001::fixtures::path("DEFAULT_SETTINGS");let scripts=default.parent().unwrap().parent().unwrap();
            let styles=crate::fresh_city::load_era_styles(std::path::Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap();
            let names=std::array::from_fn(|_|Vec::new());
            let inputs=||Inputs{tree_locators: None, image:&image,board:&board,file:&file,people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts,parameters:std::str::from_utf8(&parameters).unwrap(),era_styles:styles.clone(),timetable:std::str::from_utf8(&timetable).unwrap(),scenario:"SCENARIO3",difficulty:0,selected_character:4,profile_name:b"Save fixture",character_names:&names};
            let policy=||Policy{seed:0x12345678,auxiliary_prior_bytes:[0;284]};
            let mut session=Session::initialize(inputs(),policy()).unwrap();
            // The supplied bank must reach this actual Session before map
            // production. Preserve an old save-owned rate10 across cold load.
            assert_eq!([session.world.building_parameters.row(0).unwrap(),session.world.building_parameters.row(1).unwrap(),session.world.building_parameters.row(2).unwrap()],[[3,15,4],[3,15,4],[0,15,4]]);
            let old_rate_slot=session.world.buildings.iter().position(|row|row[0x3f]!=0 && word(row,0x24)<2).expect("Scenario3 materialized retail/residential building");
            assert_eq!(word(&session.world.buildings[old_rate_slot],0x28),3,"map/scenario producer must receive configured rent");
            session.world.buildings[old_rate_slot][0x28..0x2c].copy_from_slice(&10u32.to_le_bytes());
            // Reach the human quote adapter with exact, independent operands:
            // volume2, prestige0, quality0, landscape before06 => base30.
            let prior_prestige: [u8;4]=session.world.categories[0][0x84..0x88].try_into().unwrap();
            let prior_components=session.world.construction_components;
            session.world.categories[0][0x84..0x88].copy_from_slice(&0f32.to_le_bytes());
            let mut quote_command=openmt2001::construction::Command { category:0,rectangle:[0,0,1,0],height:1,kind:2,business:0,variation:0,quality:0,owner:1,components:[0.;4] };
            assert_eq!(openmt2001::live_commands::quote(&mut quote_command,&mut session.world.city,&session.roster,&image,5).unwrap(),30);
            assert_eq!(quote_command.components.map(f32::to_bits),[30f32,0.,0.,0.].map(f32::to_bits));
            session.world.categories[0][0x84..0x88].copy_from_slice(&prior_prestige);
            session.world.construction_components=prior_components;
            let cold_build_state=session.build_random_state;
            let cold_dust_state=session.demolition_effects.random;
            let style_counts=session.era_styles[session.world.clock.calendar[2] as usize].qualities.each_ref().map(Vec::len);
            crate::build_panel::placement_choices(&mut session.build_random_state,style_counts).unwrap();
            assert_ne!(session.build_random_state,cold_build_state);
            session.campaign_profile=Some(std::path::PathBuf::from("work/save/campaign/536176652066697874757265.lua"));
            let camera=openmt2001::flyby::CameraSample{eye:[100.,120.,100.],target:[0.,0.,0.],up:[0.,1.,0.],vertical_fov:0.7};
            let cameras=crate::save_menu::cameras(None,camera).unwrap();
            let assets=CommandAssets{image:&image,board:&board,file:&file};
            let business=session.world.ledgers.businesses.iter().position(|b|b[0x21c]!=0).unwrap();
            let terms=openmt2001::economy::PendingBusinessTerms{business_index:word(&session.world.ledgers.businesses[business],0x210),terms:openmt2001::economy::BusinessTerms{stock_targets:[7;6],prices:[2.;6],rent:123.}};
            session.live_commands.submit(Command::Terms(terms),session.world.clock.timing[0],true,|_|Err("terms dispatched early".into())).unwrap();
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let root=std::path::PathBuf::from("work/save/v2-roundtrip-profile");let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(&path).unwrap();let save=SaveFile::parse(&loaded).unwrap();
            let mut resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            let metadata=resumed.restore_v2_extension(&save).unwrap().unwrap();assert_eq!(metadata.profile_filename.as_deref(),Some("536176652066697874757265.lua"));
            assert_eq!(resumed.world.clock,session.world.clock);
            assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
            assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
            assert_eq!(resumed.saved_cameras,Some(cameras));
            assert_eq!(resumed.world.building_parameters.row(0).unwrap(),[3,15,4]);
            assert_eq!(word(&resumed.world.buildings[old_rate_slot],0x28),10,"cold resource reinit must not rewrite saved rent");

            assert_eq!(resumed.world.creation.auxiliary.slots(),session.world.creation.auxiliary.slots());
            assert_eq!(resumed.roster.profiles,session.roster.profiles);
            assert_eq!(resumed.script_random_state,session.script_random_state);
            assert_eq!(resumed.build_random_state,session.build_random_state);
            let ext=extension::Extension::parse(&save).unwrap().unwrap();
            assert_eq!(ext.version,4);
            // Rebuild actual legacy record sets, rather than claiming the new
            // placement stream can be derived from their other RNG records.
            for legacy_version in [1,2,3] {
                let mut legacy=bytes[..save.sections.iter().find(|s|s.tag==extension::TAG).unwrap().offset].to_vec();
                legacy.extend_from_slice(&u32::MAX.to_le_bytes());legacy.extend_from_slice(&save.terminator_padding);
                let mut records:Vec<_>=ext.records.iter().filter(|(tag,_)|**tag<=13).map(|(tag,row)|(*tag,row.to_vec())).collect();
                if legacy_version==2 { records.push((15,words([3]))); }
                if legacy_version==3 { records.push((16,words([session.build_random_state]))); }
                extension::append_version(&mut legacy,&records,legacy_version).unwrap();
                let parsed=SaveFile::parse(&legacy).unwrap();
                assert_eq!(verified_modern_presentation_queue(&parsed).unwrap(), [legacy_version != 1],
                    "VERSION1 alone keeps synchronous native admission");
                if legacy_version == 1 {
                    let queue=extension::Extension::parse(&parsed).unwrap().unwrap().required(9).unwrap();
                    let at=queue.as_ptr() as usize-legacy.as_ptr() as usize;
                    let mut disagree=legacy.clone();
                    put(&mut disagree,at+8,word(&legacy,at+8).wrapping_add(1));
                    let disagree=SaveFile::parse(&disagree).unwrap();
                    assert!(verified_modern_presentation_queue(&disagree).unwrap_err()
                        .contains("queue identity"),
                        "VERSION1 must not replace a different native queue");
                    let mut direct=Session::load_original(inputs(),policy(),&legacy).unwrap();
                    assert!(direct.restore_v2_extension(&disagree).err().expect("tampered VERSION1 queue accepted")
                        .contains("queue identity"),
                        "direct VERSION1 restoration must reject a different native queue");
                }
                let mut old=Session::load_original(inputs(),policy(),&legacy).unwrap();
                assert_eq!(old.build_random_state,cold_build_state);
                old.restore_v2_extension(&parsed).unwrap();
                assert_eq!(old.build_random_state,if legacy_version==3 {session.build_random_state} else {cold_build_state});
                assert_eq!(old.demolition_effects.count,0);
                assert_eq!(old.demolition_effects.random,cold_dust_state);
            }
            let mut host=crate::live_host::Host::new(camera.eye);let mut resumed_host=crate::live_host::Host::new(camera.eye);
            host.running=true;resumed_host.running=true;
            for _ in 0..30 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
            }
            assert_eq!(resumed.world.clock,session.world.clock);
            assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
            assert_eq!(resumed.world.clock.timing[0],30);
            assert_eq!(word(&resumed.world.ledgers.businesses[business],0x1dc),123f32.to_bits());
            println!("V2 host saved {}: complete city/accounting/camera/profile/RNG state and delayed terms resume through 30 ticks",path.display());
            // Find a genuinely admitted original form, then save while native
            // pending-world growth is active rather than manufacturing a pool.
            let mut selected=None;
            'search: for category in 0..52 {
                if !matches!(crate::build_panel::Context::from_session(&session,category).unwrap().entry(),crate::build_panel::Entry::Open){continue;}
                for x in 0..=5 {for y in 0..=5 {
                    let request=crate::build_panel::Request{category:category as i32,origin:[x,y],size:[2,2],height:2,packed_model:0,quality:0,kind:0,displayed_quote:0.,business:0};
                    if session.inspect_build(&request,&assets).is_ok_and(|quote|quote.allowed()){selected=Some(request);break 'search;}
                }}
            }
            let request=selected.expect("Scenario3 has a legally quoted bakery lot");
            let slot=openmt2001::building_pool::first_free(&session.world.buildings).unwrap().unwrap();
            assert!(session.submit_build(&request,&assets,|event|Err(format!("build prematurely dispatched:{event:?}"))).unwrap().submitted);
            // A normal Save Game request can arrive during the 25-tick delay
            // after a legal Build submission. The original queue packet is
            // future-due here; compare its uninterrupted and cold V4 paths.
            let build_due=session.world.clock.timing[0]+25;
            let queue_image=|s:&Session<'_>|s.live_commands.snapshot().unwrap().into_iter()
                .map(|(domain,outer,inner,direct,command)|{
                    let domain=match domain {openmt2001::scheduler::ClockDomain::Total=>0u8,
                        openmt2001::scheduler::ClockDomain::Normal=>1u8};
                    let bytes=match command {Command::CategoryReady{category}=>category.to_le_bytes().to_vec(),
                        _=>packet(command,inner).unwrap()};
                    (domain,outer,inner,direct,bytes)
                }).collect::<Vec<_>>();
            let accounts=|s:&Session<'_>|s.world.ledgers.players.iter().map(|account|{
                let mut row=[0u8;0x1bc];account.write_runtime(&mut row).unwrap();row
            }).collect::<Vec<_>>();
            assert!(session.live_commands.snapshot().unwrap().iter().any(|(_,outer,inner,_,command)|
                *outer==build_due && *inner==build_due && matches!(command,Command::Build(_))));
            let pending_bytes=session.save_v2(&image,cameras,1).unwrap();
            let pending_save=SaveFile::parse(&pending_bytes).unwrap();
            assert!(verified_modern_presentation_queue(&pending_save).unwrap().iter().all(|deferred| !deferred));
            resumed=Session::load_original(inputs(),policy(),&pending_bytes).unwrap();
            resumed.restore_v2_extension(&pending_save).unwrap();
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            assert_eq!(resumed.world.clock,session.world.clock);
            assert_eq!(queue_image(&resumed),queue_image(&session));
            assert_eq!(accounts(&resumed),accounts(&session));
            while session.world.clock.timing[0]<80 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(resumed.world.clock,session.world.clock);
                assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
                assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
                assert_eq!(queue_image(&resumed),queue_image(&session));
                assert_eq!(accounts(&resumed),accounts(&session));
                assert_eq!(&resumed.world.buildings[slot][..64],&session.world.buildings[slot][..64]);
            }
            assert!(session.world.clock.timing[0]>build_due);
            assert_eq!(word(&session.world.buildings[slot],0x28),3,"due human Build initializer uses configured rent");
            assert_eq!(word(&resumed.world.buildings[slot],0x28),3,"cold queued Build uses configured rent");
            assert_eq!(word(&resumed.world.buildings[old_rate_slot],0x28),10,"new construction must preserve old saved rate");

            assert_ne!(session.world.pending_world.count,0);
            assert_ne!(word(&session.world.buildings[slot],0x70),1f32.to_bits());
            assert!(!crate::intro::detail_model_at_unit_scale(&session.world.buildings[slot]),
                "native pass3 must not draw a legally growing bakery");
            let bytes=session.save_v2(&image,cameras,1).unwrap();let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let bytes=std::fs::read(path).unwrap();let save=SaveFile::parse(&bytes).unwrap();
            resumed=Session::load_original(inputs(),policy(),&bytes).unwrap();resumed.restore_v2_extension(&save).unwrap();
            assert!(!crate::intro::detail_model_at_unit_scale(&resumed.world.buildings[slot]),
                "restored growth must retain the native pass3 admission gate");
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            let mut prompt_deadlines=Vec::new();
            let mut open_due=None;
            while session.world.clock.timing[0]<700 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(resumed.world.clock,session.world.clock);
                let left_prompts=host.take_prompts(&session);let right_prompts=resumed_host.take_prompts(&resumed);
                assert_eq!(left_prompts,right_prompts,"named original scenario callback deadline changed");
                for prompt in left_prompts{prompt_deadlines.push(session.world.clock.timing[0]);session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();resumed.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();}
                let differing=session.world.ledgers.businesses.iter().zip(&resumed.world.ledgers.businesses).position(|(a,b)|a!=b);
                assert!(differing.is_none(),"business state diverged at tick{} row{:?}",session.world.clock.timing[0],differing);
                assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
                assert_eq!(resumed.road_traffic.admission.random,session.road_traffic.admission.random);
                assert_eq!(resumed.world.creation.initialization_rng.constructor,session.world.creation.initialization_rng.constructor);
                if open_due.is_none() {
                    let queued=session.live_commands.snapshot().unwrap().into_iter()
                        .find_map(|(_,outer,inner,_,command)|match command {
                            Command::Open(open) if open.building==slot as u32=>Some((outer,inner)),
                            _=>None,
                        });
                    if let Some((outer,inner))=queued {
                        assert_eq!(outer,inner,"completion's Open packet keeps its inner deadline");
                        assert!(outer>session.world.clock.timing[0],"Open must be future-due at Save Game");
                        let open_bytes=session.save_v2(&image,cameras,1).unwrap();
                        let open_save=SaveFile::parse(&open_bytes).unwrap();
                        assert!(verified_modern_presentation_queue(&open_save).unwrap().iter().all(|deferred| !deferred));
                        resumed=Session::load_original(inputs(),policy(),&open_bytes).unwrap();
                        resumed.restore_v2_extension(&open_save).unwrap();
                        resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
                        assert_eq!(queue_image(&resumed),queue_image(&session));
                        assert_eq!(accounts(&resumed),accounts(&session));
                        assert_eq!(resumed.world.pending_world.to_bytes(),session.world.pending_world.to_bytes());
                        open_due=Some(outer);
                    }
                }
                if open_due.is_some_and(|due|session.world.clock.timing[0]<=due+2) {
                    assert_eq!(queue_image(&resumed),queue_image(&session),"Open queue continuation at tick{}",session.world.clock.timing[0]);
                    assert_eq!(accounts(&resumed),accounts(&session),"Open accounting at tick{}",session.world.clock.timing[0]);
                    assert_eq!(&resumed.world.buildings[slot][..64],&session.world.buildings[slot][..64]);
                }
            }
            assert!(open_due.is_some(),"legal construction never queued its delayed Open");
            assert!(!prompt_deadlines.is_empty(),"original CreatePrompt callback did not fire");
            assert_eq!(word(&session.world.buildings[slot],0x14),4);
            assert_eq!(session.world.pending_world.count,0);
            for completed in [&session, &resumed] {
                assert!(crate::intro::detail_model_at_unit_scale(&completed.world.buildings[slot]),
                    "completed bakery must admit its native descriptor rows");
            }
            let opened=word(&session.world.buildings[slot],0x1c) as usize;
            assert_ne!(session.world.ledgers.businesses[opened][0x21c],0);
            for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players){assert_eq!(left.bytes,right.bytes);}
            for (left,right) in session.world.ledgers.residents.nodes.iter().zip(&resumed.world.ledgers.residents.nodes){assert!(left.as_bytes()==right.as_bytes(),"resident state diverged after active-construction save");}
            // Controlled queue boundary, using its public restoration API:
            // preserve the legally built bakery and clocks without mutation.
            // The ordinary UI producer delays25 ticks; this represents a
            // pending due-now record that the modern serializer can retain.
            let seller=session.roster.manager.players.iter().position(|p|
                p.reference==session.roster.status.current_player).unwrap();
            assert_eq!(session.world.ledgers.businesses[opened][10],seller as u8+1);
            assert!(openmt2001::ai_actions::liquidation_score::business_value(
                &session.world.ledgers.businesses[opened],&session.world.buildings,&image,
            ).unwrap().to_f32()>0.,"legal bakery must exercise actual cash presentation");
            // A real ordinary completed model still needs synchronous485520.
            // Reject its due-now takeover before returning publishable bytes.
            let retained_queue=std::mem::take(&mut session.live_commands);
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(Command::Takeover(openmt2001::business_takeover::Command {
                building:slot as u32,
            }),openmt2001::scheduler::ClockDomain::Total,due,due,false);
            assert!(session.save_v2(&image,cameras,1).unwrap_err()
                .contains("unverified synchronous appearance"));
            session.live_commands=retained_queue;
            let sale=Command::SellBusiness(openmt2001::business_sale::Command {
                business:opened as u32,player:seller as u8,
            });
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(sale,
                openmt2001::scheduler::ClockDomain::Total,due,due,false);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            // Native and extension identities must agree before dispatch is
            // deferred; changing only the redundant extension deadline fails.
            let queue=extension::Extension::parse(&save).unwrap().unwrap().required(9).unwrap();
            let queue_offset=queue.as_ptr() as usize-bytes.as_ptr() as usize;
            let mut disagree=bytes.clone();
            put(&mut disagree,queue_offset+4+8,due.wrapping_add(1));
            assert!(verified_modern_presentation_queue(&SaveFile::parse(&disagree).unwrap()).is_err());
            let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(path).unwrap();
            let save=SaveFile::parse(&loaded).unwrap();
            resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            resumed.restore_v2_extension(&save).unwrap();
            assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses,
                "loading must not execute the modern sale early");
            assert_eq!(resumed.live_commands.snapshot().unwrap().len(),1);
            let age=session.population.business_counts.age;
            let (left_sound,left_requests)=std::sync::mpsc::channel();
            let (right_sound,right_requests)=std::sync::mpsc::channel();
            host=crate::live_host::Host::new(camera.eye);host.running=true;host.gameplay_sound=Some(left_sound);
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;resumed_host.gameplay_sound=Some(right_sound);
            for turn in 0..2 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(resumed.world.clock,session.world.clock);
                assert_eq!(resumed.world.ledgers.businesses,session.world.ledgers.businesses);
                for (left,right) in session.world.buildings.iter().zip(&resumed.world.buildings) {
                    assert_eq!(&left[..64],&right[..64]);
                    assert_eq!(word(left,0x4c),word(right,0x4c));
                    assert_eq!(&left[0x58..0x80],&right[0x58..0x80]);
                }
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players){assert_eq!(left.bytes,right.bytes);}
                assert_eq!(session.population.business_counts.age,age.wrapping_add(100),"sale ran more than once at turn{turn}");
                assert_eq!(resumed.population.business_counts.age,session.population.business_counts.age);
            }
            assert_eq!(session.world.ledgers.businesses[opened][10],0);
            assert_eq!(resumed.live_commands.snapshot().unwrap().len(),0);
            let ledger=|receiver:std::sync::mpsc::Receiver<crate::gameplay_sound::Request>|receiver.try_iter()
                .filter_map(|r|match r.operation {crate::gameplay_sound::Operation::Ledger{sale}=>Some(sale),_=>None}).collect::<Vec<_>>();
            assert_eq!(ledger(left_requests),[true]);assert_eq!(ledger(right_requests),[true]);
            assert!(host.cash_arrow_state().active);assert!(resumed_host.cash_arrow_state().active);
            let buyer=session.roster.manager.players[seller].bytes[0x1bc]+1;
            let mut acquired=None;
            for (amount,same_player) in [(50000i32,false),(-50000,false),(50000,true)] {
                let chains=openmt2001::building_pool::category_chains(
                    &session.world.categories,&session.world.buildings,0x591318).unwrap();
                let category=if same_player { acquired.unwrap() } else {
                    chains.iter().enumerate().find(|(index,chain)|
                        chain.is_empty() && session.world.categories[*index][5]==0
                            && ![2,35,39,42].contains(index)).unwrap().0
                };
                if acquired.is_none() { acquired=Some(category); }
                let due=session.world.clock.timing[0];
                session.live_commands.restore_queued(Command::PropertyTransfer {
                    category:category as i16,owner:buyer,amount,
                },openmt2001::scheduler::ClockDomain::Total,due,due,false);
                let bytes=session.save_v2(&image,cameras,1).unwrap();
                let save=SaveFile::parse(&bytes).unwrap();
                assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
                let path=crate::save_menu::write_new(&root,&bytes).unwrap();
                let loaded=std::fs::read(path).unwrap();
                let save=SaveFile::parse(&loaded).unwrap();
                resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
                resumed.restore_v2_extension(&save).unwrap();
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                    assert_eq!(left.bytes,right.bytes,"load must not post cash early");
                }
                assert_eq!(session.world.categories[category][5],resumed.world.categories[category][5]);
                assert_eq!(resumed.live_commands.snapshot().unwrap().len(),1);
                let (left_sound,left_requests)=std::sync::mpsc::channel();
                let (right_sound,right_requests)=std::sync::mpsc::channel();
                host=crate::live_host::Host::new(camera.eye);host.running=true;host.gameplay_sound=Some(left_sound);
                resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;resumed_host.gameplay_sound=Some(right_sound);
                for _ in 0..2 {
                    host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                    assert!(host.failure.is_none(),"{:?}",host.failure);
                    assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                    assert_eq!(session.world.clock,resumed.world.clock);
                    assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
                    for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                        assert_eq!(left.bytes,right.bytes);
                    }
                    assert_eq!(session.world.city.ownership_refresh_counter,resumed.world.city.ownership_refresh_counter);
                    for (left,right) in session.world.categories.iter().zip(&resumed.world.categories) {
                        assert_eq!(left[5],right[5]);
                        assert_eq!(&left[0x6c..0x84],&right[0x6c..0x84]);
                    }
                    assert_eq!(openmt2001::building_pool::category_chains(&session.world.categories,&session.world.buildings,0x591318).unwrap(),
                        openmt2001::building_pool::category_chains(&resumed.world.categories,&resumed.world.buildings,0x591318).unwrap());
                    assert_eq!(host.cash_arrow_state(),resumed_host.cash_arrow_state());
                }
                assert_eq!(session.world.categories[category][5],buyer);
                assert!(session.live_commands.snapshot().unwrap().is_empty());
                assert!(resumed.live_commands.snapshot().unwrap().is_empty());
                let expected=if same_player {vec![true,false]} else {vec![amount<0]};
                assert_eq!(ledger(left_requests),expected);assert_eq!(ledger(right_requests),expected);
                assert!(host.cash_arrow_state().active);assert!(resumed_host.cash_arrow_state().active);
            }
            // The already-due zero-price release and direct category expiry
            // each pass through original4810e0 on load. A V4 save must leave
            // their effects pending until its authoritative worker resumes;
            // otherwise record9 reinstalls a command already consumed once.
            let released_category=acquired.unwrap();
            assert_eq!(session.world.categories[released_category][5],buyer);
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(Command::PropertyTransfer {
                category:released_category as i16,owner:0,amount:0,
            },openmt2001::scheduler::ClockDomain::Total,due,due,false);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            resumed=Session::load_original(inputs(),policy(),&bytes).unwrap();
            resumed.restore_v2_extension(&save).unwrap();
            assert_eq!(resumed.world.categories[released_category][5],buyer,
                "zero-price release must remain pending at load");
            host=crate::live_host::Host::new(camera.eye);host.running=true;
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
            assert!(host.failure.is_none(),"{:?}",host.failure);
            assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
            assert_eq!(session.world.categories[released_category][5],0);
            assert_eq!(resumed.world.categories[released_category][5],0);
            assert_eq!(resumed.world.city.ownership_refresh_counter,
                session.world.city.ownership_refresh_counter);
            assert!(session.live_commands.snapshot().unwrap().is_empty());
            assert!(resumed.live_commands.snapshot().unwrap().is_empty());

            let ready_category=released_category;
            let normal=session.world.clock.timing[1];
            openmt2001::category_ownership::begin_wait(
                &mut session.world.categories[ready_category],normal);
            session.live_commands.restore_queued(Command::CategoryReady {
                category:ready_category as u32,
            },openmt2001::scheduler::ClockDomain::Normal,normal,normal,true);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            resumed=Session::load_original(inputs(),policy(),&bytes).unwrap();
            resumed.restore_v2_extension(&save).unwrap();
            assert_eq!(word(&resumed.world.categories[ready_category],0x78),1,
                "direct category expiry must remain pending at load");
            host=crate::live_host::Host::new(camera.eye);host.running=true;
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
            assert!(host.failure.is_none(),"{:?}",host.failure);
            assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
            assert_eq!(word(&session.world.categories[ready_category],0x78),2);
            assert_eq!(session.world.categories[ready_category],resumed.world.categories[ready_category]);
            assert!(session.live_commands.snapshot().unwrap().is_empty());
            assert!(resumed.live_commands.snapshot().unwrap().is_empty());
            // Continue the legally built bakery through the real demolition
            // producer, dust allocator, scheduler and geometry retirement.
            let category = ((word(&session.world.buildings[slot],0x40)-0x617628)/380) as usize;
            let amount = openmt2001::demolition::quote_with_parameters(&session.world.buildings[slot],
                &session.world.categories,&session.world.city.building_parameters,&image).unwrap().to_f32();
            session.live_commands.submit(Command::Demolish(openmt2001::demolition::Command {
                category:category as i8,building:slot as u16,amount,
            }),session.world.clock.timing[0],true,|_|Err("demolition dispatched early".into())).unwrap();
            let demolition=session.live_commands.snapshot().unwrap().into_iter()
                .find_map(|(_,_,_,_,command)| match command {
                    Command::Demolish(command)=>Some(command.clone()),_=>None,
                }).expect("legal demolition producer queued its command");
            let retained=std::mem::take(&mut session.live_commands);
            let due=session.world.clock.timing[0];
            session.live_commands.restore_queued(Command::Demolish(demolition),
                openmt2001::scheduler::ClockDomain::Total,due,due,false);
            assert!(session.save_v2(&image,cameras,1).unwrap_err()
                .contains("Cannot save due-now command 10"));
            session.live_commands=retained;
            for _ in 0..150 {
                host.tick(&mut session,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                if session.demolition_points.len()>=8 {break;}
            }
            assert!(session.demolition_points.len()>=8,"legal demolition must emit live dust");
            let effect = word(&session.world.buildings[slot],0x7c) as usize;
            assert!(effect<16);
            assert_eq!(session.demolition_effects.slots[effect].model,1);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(path).unwrap();let save=SaveFile::parse(&loaded).unwrap();
            resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            resumed.restore_v2_extension_with_image(&save,&image).unwrap();
            assert_eq!(word(&resumed.world.buildings[slot],0x7c),effect as u32);
            assert_eq!(session.demolition_points,resumed.demolition_points);
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;
            for _ in 0..350 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(openmt2001::save::demolition::encode(&session.demolition_effects).unwrap(),
                    openmt2001::save::demolition::encode(&resumed.demolition_effects).unwrap());
                assert_eq!(session.demolition_points,resumed.demolition_points);
                assert_eq!(session.world.pending_world.to_bytes(),resumed.world.pending_world.to_bytes());
                assert_eq!(session.world.clock,resumed.world.clock);
                assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
                for offset in [0x14,0x5c,0x70,0x74,0x78,0x7c] {
                    assert_eq!(word(&session.world.buildings[slot],offset),
                        word(&resumed.world.buildings[slot],offset),"demolition field{offset:x}");
                }
                assert_eq!(openmt2001::building_pool::category_chains(
                    &session.world.categories,&session.world.buildings,0x591318).unwrap(),
                    openmt2001::building_pool::category_chains(
                    &resumed.world.categories,&resumed.world.buildings,0x591318).unwrap());
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                    assert_eq!(left.bytes,right.bytes);
                }
                if !session.demolition_effects.allocated() && session.world.pending_world.count==0 {break;}
            }
            assert!(!session.demolition_effects.allocated(),"dust must naturally retire");
            assert_eq!(session.world.pending_world.count,0);
            assert!(!session.demolition_effects.slots[effect].positions.is_empty());
            let retired= openmt2001::save::demolition::encode(&session.demolition_effects).unwrap();
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let save=SaveFile::parse(&bytes).unwrap();
            let mut after=Session::load_original(inputs(),policy(),&bytes).unwrap();
            after.restore_v2_extension_with_image(&save,&image).unwrap();
            assert_eq!(openmt2001::save::demolition::encode(&after.demolition_effects).unwrap(),retired);
            assert!(after.demolition_points.is_empty());
            // Original planning type2/business44 reaches the genuine linked
            // structure branch that does not refresh485520. No flags/status
            // are manufactured to admit the saved takeover.
            let (park_sound, _park_requests) = std::sync::mpsc::channel();
            host.gameplay_sound = Some(park_sound);
            let mut park = None;
            'park_search: for category in 0..52 {
                if !matches!(crate::build_panel::Context::from_session(&session,category).unwrap().entry(),
                    crate::build_panel::Entry::Open) {continue;}
                for x in 0..=5 {for y in 0..=5 {
                    let request=crate::build_panel::Request {category:category as i32,origin:[x,y],
                        size:[2,2],height:1,packed_model:0,quality:0,kind:2,displayed_quote:0.,business:44};
                    if session.inspect_build(&request,&assets).is_ok_and(|q|q.allowed()) {
                        park=Some(request);break 'park_search;
                    }
                }}
            }
            let park=park.expect("actual Scenario3 catalog must admit a legal type2 structure");
            let park_slot=openmt2001::building_pool::first_free(&session.world.buildings).unwrap().unwrap();
            assert!(session.submit_build(&park,&assets,|_|Ok(())).unwrap().submitted);
            // Submit queues the build command; the scene row appears only
            // after the Host consumes it on a simulation tick.
            for _ in 0..350 {
                host.tick(&mut session,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                if word(&session.world.buildings[park_slot],0x24)==2 {break;}
            }
            assert_eq!(word(&session.world.buildings[park_slot],0x24),2);
            assert_ne!(word(&session.world.buildings[park_slot],0x14),4);
            let suncols=read("OPENMT2001_SUNCOLS");
            let before_open=session.scene(&build,openmt2001::render_scene::OriginalAssets {
                executable:&exe,directory:&directory,archive:&archive,
            },&suncols,Some(camera),false).unwrap();
            let pending_park=before_open.inspection_buildings.iter()
                .find(|building|building.runtime_slot==Some(park_slot)).unwrap();
            assert_eq!(pending_park.kind,2);
            assert!(!crate::intro::building_has_management_link(pending_park),
                "original 4c39ed must present Block Information before the Landscape Open link");
            for _ in 0..350 {
                host.tick(&mut session,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                for prompt in host.take_prompts(&session) {
                    session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();
                }
                if word(&session.world.buildings[park_slot],0x14)==4
                    && word(&session.world.buildings[park_slot],0x1c)!=u32::MAX {break;}
            }
            assert_eq!(word(&session.world.buildings[park_slot],0x14),4);
            assert_eq!(word(&session.world.buildings[park_slot],0x24),2);
            assert_ne!(word(&session.world.buildings[park_slot],0x1c),u32::MAX,
                "legal landscape Open must finish: tick{}, pause{:?}, queued{}",
                session.world.clock.timing[0],session.world.pause.bytes,session.live_commands.snapshot().unwrap().len());
            let park_business=word(&session.world.buildings[park_slot],0x1c) as usize;
            assert_eq!(word(&session.world.ledgers.businesses[park_business],4),44);
            // The original 407970 type2 building-panel path receives the
            // linked landscape business after Open, with its action tabs
            // gated by class2. Retain that link in the frontend snapshot.
            let scene=session.scene(&build,openmt2001::render_scene::OriginalAssets {
                executable:&exe,directory:&directory,archive:&archive,
            },&suncols,Some(camera),false).unwrap();
            let park_view=scene.inspection_buildings.iter()
                .find(|building|building.runtime_slot==Some(park_slot)).unwrap();
            assert!(crate::intro::building_has_management_link(park_view),
                "original 4c39f3 must present Building Information after Landscape Open");
            let park_link=park_view.business().expect("opened Landscape must reach the original building panel");
            assert_eq!(park_link.slot,park_business);
            assert_eq!(word(&park_link.record,0),2);
            let park_panel=session.inspect_business_operations(park_slot,[30.,29.,190.,300.],&image).unwrap();
            assert_eq!(park_panel.report.summary.len(),3);
            assert!(park_panel.permissions.modes[0]);
            assert!(!park_panel.permissions.modes[1]);
            assert!(park_panel.trade_price.is_none());
            let owner=session.roster.manager.players[seller].bytes[0x1bc]+1;
            session.live_commands.submit(Command::PropertyTransfer {category:park.category as i16,
                owner,amount:50000},session.world.clock.timing[0],true,
                |_|Err("land acquisition dispatched early".into())).unwrap();
            for _ in 0..30 {host.tick(&mut session,&assets);assert!(host.failure.is_none(),"{:?}",host.failure);}
            assert_eq!(session.world.categories[park.category as usize][5],owner);
            assert_eq!(session.world.ledgers.businesses[park_business][10],owner);
            let references:Vec<_>=session.roster.manager.players.iter().map(|p|p.reference).collect();
            assert!(openmt2001::business_takeover::price(park_slot,&openmt2001::business_takeover::PriceWorld {
                categories:&session.world.categories,buildings:&session.world.buildings,
                businesses:&session.world.ledgers.businesses,player_references:&references,
            },session.world.clock.timing[1],&image).unwrap().to_f32()>0.,
                "actual park must exercise both current-player cash postings");
            assert!(session.world.clock.timing[0] > 0);
            session.live_commands.restore_queued(Command::Takeover(openmt2001::business_takeover::Command {
                building:park_slot as u32,
            }),openmt2001::scheduler::ClockDomain::Total,session.world.clock.timing[0],
                session.world.clock.timing[0] - 1,false);
            let bytes=session.save_v2(&image,cameras,1).unwrap();
            let path=crate::save_menu::write_new(&root,&bytes).unwrap();
            let loaded=std::fs::read(path).unwrap();let save=SaveFile::parse(&loaded).unwrap();
            assert_eq!(verified_modern_presentation_queue(&save).unwrap(),[true]);
            resumed=Session::load_original(inputs(),policy(),&loaded).unwrap();
            resumed.restore_v2_extension_with_image(&save,&image).unwrap();
            assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
            for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                assert_eq!(left.bytes,right.bytes,"takeover must not post during loading");
            }
            let (left_sound,left_requests)=std::sync::mpsc::channel();
            let (right_sound,right_requests)=std::sync::mpsc::channel();
            host=crate::live_host::Host::new(camera.eye);host.running=true;host.gameplay_sound=Some(left_sound);
            resumed_host=crate::live_host::Host::new(camera.eye);resumed_host.running=true;resumed_host.gameplay_sound=Some(right_sound);
            for _ in 0..30 {
                host.tick(&mut session,&assets);resumed_host.tick(&mut resumed,&assets);
                assert!(host.failure.is_none(),"{:?}",host.failure);
                assert!(resumed_host.failure.is_none(),"{:?}",resumed_host.failure);
                assert_eq!(session.world.clock,resumed.world.clock);
                assert_eq!(session.world.ledgers.businesses,resumed.world.ledgers.businesses);
                assert_eq!(session.population.business_counts.age,resumed.population.business_counts.age);
                assert_eq!(session.population.business_counts.values,resumed.population.business_counts.values);
                assert_eq!(session.world.pending_world.to_bytes(),resumed.world.pending_world.to_bytes());
                for (left,right) in session.world.buildings.iter().zip(&resumed.world.buildings) {
                    assert_eq!(&left[..64],&right[..64]);
                    assert_eq!(word(left,0x4c),word(right,0x4c));
                }
                for (left,right) in session.world.categories.iter().zip(&resumed.world.categories) {
                    assert_eq!(left[5],right[5]);assert_eq!(&left[0x6c..0x8c],&right[0x6c..0x8c]);
                }
                assert_eq!(openmt2001::building_pool::category_chains(
                    &session.world.categories,&session.world.buildings,0x591318).unwrap(),
                    openmt2001::building_pool::category_chains(
                    &resumed.world.categories,&resumed.world.buildings,0x591318).unwrap());
                assert_eq!(session.roster.profiles,resumed.roster.profiles);
                for (left,right) in session.roster.manager.players.iter().zip(&resumed.roster.manager.players) {
                    assert_eq!(left.bytes,right.bytes);
                    assert_eq!(left.auxiliaries.len(),right.auxiliaries.len());
                    for (left,right) in left.auxiliaries.iter().zip(&right.auxiliaries) {
                        assert_eq!(left.reference,right.reference);assert_eq!(left.bytes,right.bytes);
                    }
                }
                assert_eq!(host.cash_arrow_state(),resumed_host.cash_arrow_state());
            }
            assert!(session.live_commands.snapshot().unwrap().is_empty());
            assert!(resumed.live_commands.snapshot().unwrap().is_empty());
            assert_eq!(ledger(left_requests),[true,false]);assert_eq!(ledger(right_requests),[true,false]);
            println!("V4 host save acceptance: legal bakery construction/open/hour plus live demolition dust and retired pool/RNG continuation");
        }).unwrap().join().unwrap();
    }
}

/// Disjoint live save owners. Autosave supplies callback Clock/City/Population,
/// moved hourly utilities/roads and the live message allocator. Models are not
/// serialized; their registrar remains with the running geometry transaction.
pub(crate) struct SaveProjection<'a> {
    pub live_commands: &'a openmt2001::live_commands::Queue,
    pub city: &'a mut openmt2001::level_startup::City,
    pub clock: &'a openmt2001::clock::Clock,
    pub utilities: &'a openmt2001::economy::Utilities,
    pub frequency: &'a openmt2001::timers::CalendarFrequency,
    pub timers: &'a [openmt2001::timers::Task],
    pub scripts: &'a mut openmt2001::level_startup::Scripts,
    pub roster: &'a mut openmt2001::level_startup::Roster,
    pub population: &'a openmt2001::level_runtime::Population,
    pub road_traffic: &'a openmt2001::traffic::road::runtime::RoadTraffic,
    pub transport: &'a openmt2001::traffic::scene::Scene,
    pub catalog: &'a openmt2001::level_catalog::Catalog,
    pub campaign_profile: &'a Option<std::path::PathBuf>,
    pub skirmish_map: &'a Option<crate::skirmish_maps::MapIdentity>,
    pub saved_player_metadata: &'a [[u8; 48]; 6],
    pub history: &'a openmt2001::player_history::State,
    pub random_registry: &'a openmt2001::player_manager::RandomRegistry,
    pub event_cards: &'a openmt2001::event_cards::Deck,
    pub pulse_lights: &'a openmt2001::pulse_lights::PulseLights,
    pub demolition_effects: &'a openmt2001::demolition_effect::Manager,
    pub policy: &'a crate::fresh_city::Policy,
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
struct OptionsContinuation {
    pause: openmt2001::pause::State,
    timers: Vec<openmt2001::timers::Task>,
}
impl SaveProjection<'_> {
    pub(crate) fn encode(
        self,
        image: &openmt2001::pe::PeImage<'_>,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String> {
        self.encode_continuation(image, cameras, game_mode, host, interface_mode, None)
    }
    fn encode_continuation(
        self,
        image: &openmt2001::pe::PeImage<'_>,
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
            let domain = match domain {
                openmt2001::scheduler::ClockDomain::Total => 0,
                openmt2001::scheduler::ClockDomain::Normal => 1,
            };
            let mut h = [0; 24];
            put(&mut h, 4, domain);
            put(&mut h, 8, outer);
            let p = if let Command::CategoryReady { category } = command {
                if !direct {
                    return Err("non-direct category queue encoding unsupported".into());
                }
                put(&mut h, 0, 1);
                put(&mut h, 12, *category);
                vec![]
            } else {
                if direct {
                    return Err("direct packet queue encoding unsupported".into());
                }
                packet(command, admission)?
            };
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
            game_mode,
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
            &self.demolition_effects,
            &self.city.pending_world,
            &self.city.buildings,
        )?;
        records.push((
            17,
            openmt2001::save::demolition::encode(&self.demolition_effects)?,
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
pub(crate) trait AutosaveRequest: std::fmt::Debug {
    fn encode_once(
        &mut self,
        cameras: [[u8; 196]; 2],
        game_mode: u32,
        host: Option<AuctionHostState>,
        interface_mode: u32,
    ) -> Result<Vec<u8>, String>;
}
pub(crate) struct SaveRequest<'a, 'data> {
    pub projection: Option<SaveProjection<'a>>,
    pub image: &'a openmt2001::pe::PeImage<'data>,
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

#[cfg(test)]
#[path = "session_autosave_live_test.rs"]
mod autosave_live_tests;
