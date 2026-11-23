//! Headless original-save restoration into the retained Session. The host
//! supplies bytes; this module never opens the original profile or save path.
use super::{CommandAssets, Inputs, Policy, Session};

/// 450a30 checks packet+0xc before its command-kind switch. Its unsigned JA
/// readmits any future packet on the total-clock list. A value of -1 reaches
/// 480900's immediate sentinel and recursively re-enters 450a30 instead.
fn native_future_packet_due(packet_due: u32, total_clock: u32) -> bool {
    packet_due > total_clock && packet_due != u32::MAX
}

/// 4bbe30 writes the business pool before the 0x4000 queue, then only the
/// auction, objective, catalogs, history and mode tables. A command18 branch
/// without tenant callbacks touches only its business row, so these trailing
/// records commute with it. Unknown or reordered tails remain explicit.
fn immediate_terms_after_business_pool(save: &SaveFile<'_>, queue_offset: usize) -> bool {
    save.sections
        .iter()
        .find(|section| section.tag == 0x20)
        .is_some_and(|section| section.offset < queue_offset)
        && save
            .sections
            .iter()
            .filter(|section| section.offset > queue_offset)
            .all(|section| {
                matches!(
                    section.tag,
                    0x4000 | 0x8000 | 0x10000 | 0x40000 | 0x200000 | 0x20000 | 0x400000
                )
            })
}

/// A due-now business open also needs the restored building links, resident
/// accounts and clock before its original queue position. The later writer
/// tags are the same independent tail admitted for row-only terms.
fn immediate_open_after_required_records(save: &SaveFile<'_>, queue_offset: usize) -> bool {
    immediate_terms_after_business_pool(save, queue_offset)
        && [4, 0x200, 0x400, 0x1000].into_iter().all(|tag| {
            save.sections
                .iter()
                .find(|section| section.tag == tag)
                .is_some_and(|section| section.offset < queue_offset)
        })
        && save
            .sections
            .iter()
            .filter(|section| matches!(section.tag, 0x10 | 0x40))
            .all(|section| section.offset < queue_offset)
}

/// Command19 changes category fields and schedules a normal-clock expiry.
/// Those source records must precede the queue; original trailing tables do
/// not touch the wait fields. Retain source order with immediate kind1 rows.
fn immediate_wait_after_required_records(save: &SaveFile<'_>, queue_offset: usize) -> bool {
    [4, 0x1000].into_iter().all(|tag| {
        save.sections.iter().find(|section| section.tag == tag)
            .is_some_and(|section| section.offset < queue_offset)
    }) && save.sections.iter().filter(|section| section.offset > queue_offset)
        .all(|section| matches!(section.tag,
            0x4000 | 0x8000 | 0x10000 | 0x40000 | 0x200000 | 0x20000 | 0x400000))
}
/// Transfer helpers consume the player/account, business, linked building and
/// category pools synchronously. Catalog-dependent AI callbacks are excluded
/// by the bounded human-only consumer below, so original singleton tails commute.
fn immediate_transfer_after_required_records(save: &SaveFile<'_>, queue_offset: usize) -> bool {
    immediate_terms_after_business_pool(save, queue_offset)
        && [4, 0x400, 0x1000].into_iter().all(|tag| {
            save.sections.iter().find(|section| section.tag == tag)
                .is_some_and(|section| section.offset < queue_offset)
        })
        && save.sections.iter().filter(|section| section.tag == 0x40)
            .all(|section| section.offset < queue_offset)
}
fn immediate_sale_after_required_records(save: &SaveFile<'_>, queue_offset: usize) -> bool {
    immediate_transfer_after_required_records(save, queue_offset)
        && [0x200, 0x800].into_iter().all(|tag| save.sections.iter()
            .find(|section| section.tag == tag)
            .is_some_and(|section| section.offset < queue_offset))
}

/// The 15 supplied SAVE_ONE command13 cases refresh these six active detail
/// classes. V2 owns their final draws during scene assembly; unknown native
/// constructors still require a synchronous load-stage renderer owner.
fn checked_positive_sale_appearance(
    model: &openmt2001::save::appearance::ModelIdentity,
    height: u8,
) -> bool {
    let expected_count = match (model.descriptor, height) {
        // Exact selected model/height identities and 485200 row counts from
        // native-sale-positive-appearance.bin, derived from SAVE_ONE.
        (0x8000_05e8, 8) => 3,
        (0x827, 7) => 2,
        (0x8000_08eb, 9) => 5,
        (0x7a9, 9) => 3,
        (0x7e3, 9) => 6,
        (0x8000_066b, 9) => 9,
        (0x7a8, 8) => 3,
        (0x92c, 7) => 2,
        (0x8000_0653, 9) => 3,
        (0x2e3, 3) => 4,
        (0x8000_0132, 2) => 4,
        (0x402, 3) => 3,
        (0x32e, 2) => 4,
        (0x32d, 2) => 5,
        (0x2df, 1) => 3,
        _ => return false,
    };
    if model.renderer_templates.len() != expected_count
        || model.renderer_candidates.len() != expected_count {
        return false;
    }
    let mut active = 0;
    for template in &model.renderer_templates {
        let state = u32::from_le_bytes(template.row[..4].try_into().unwrap());
        if state != 1 {
            continue;
        }
        active += 1;
        if !matches!(template.descriptor,
            0x0101_0201 | 0x0102_0201 | 0x0104_0201 |
            0x0104_0202 | 0x0108_0202 | 0x0200_0001) {
            return false;
        }
    }
    active != 0
}
/// Original45bac0 and adjacent45b3b0 reset runtime resident storage after
/// level Lua initialization, without rewinding creation RNG streams.
fn reset_original_resident_pool(
    creation: &mut openmt2001::economy::ResidentCreationState,
    residents: &mut openmt2001::economy::ResidentAccounts,
) {
    creation.auxiliary = openmt2001::resident::AuxiliaryPool::for_residents();
    *residents = openmt2001::economy::ResidentAccounts::new_pool();
}
use openmt2001::{
    building_geometry::BuildingGeometry,
    building_pool, category_reset, economy, housing, level_startup, player_constructor,
    save::{SaveFile, restore},
    startup_rng,
};

pub(crate) fn saved_command(packet: &[u8]) -> Result<openmt2001::live_commands::Command, String> {
    use openmt2001::live_commands::Command;
    let word = |at: usize| -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            packet
                .get(at..at + 4)
                .ok_or("saved command word truncated")?
                .try_into()
                .unwrap(),
        ))
    };
    let kind = word(4)?;
    match kind {
        0x0f => Ok(Command::Build(
            openmt2001::construction::Command::from_packet(packet)?,
        )),
        0x10 => Ok(Command::Demolish(
            openmt2001::demolition::Command::from_packet(packet)?,
        )),
        0x11 => Ok(Command::Open(
            openmt2001::business_open::Command::from_packet(packet)?,
        )),
        0x12 if packet.len() == 32 => {
            Ok(Command::Convert(openmt2001::business_conversion::Command {
                kind: word(20)?,
                building: word(24)?,
                owner: word(28)?,
            }))
        }
        0x13 if packet.len() == 28 => {
            Ok(Command::SellBusiness(openmt2001::business_sale::Command {
                business: word(20)?,
                player: packet[24],
            }))
        }
        0x15 if packet.len() == 24 => {
            Ok(Command::Takeover(openmt2001::business_takeover::Command {
                building: word(20)?,
            }))
        }
        0x18 if packet.len() == 64 => {
            Ok(Command::Terms(openmt2001::economy::PendingBusinessTerms {
                business_index: word(20)?,
                terms: openmt2001::economy::BusinessTerms {
                    stock_targets: std::array::from_fn(|i| {
                        i16::from_le_bytes(packet[24 + i * 2..26 + i * 2].try_into().unwrap())
                    }),
                    prices: std::array::from_fn(|i| {
                        f32::from_bits(u32::from_le_bytes(
                            packet[36 + i * 4..40 + i * 4].try_into().unwrap(),
                        ))
                    }),
                    rent: f32::from_bits(word(60)?),
                },
            }))
        }
        0x16 if packet.len() == 52 => Ok(Command::Auction {
            descriptor: packet.try_into().unwrap(),
        }),
        0x16 if packet.len() >= 27 => Ok(Command::PropertyTransfer {
            amount: word(20)? as i32,
            category: i16::from_le_bytes(packet[24..26].try_into().unwrap()),
            owner: packet[26],
        }),
        0x19 | 0x1d | 0x1e | 0x1f | 0x20 if packet.len() == 52 => Ok(Command::Auction {
            descriptor: packet.try_into().unwrap(),
        }),
        _ => Err(format!(
            "saved command {kind:02x} has no retained packet consumer"
        )),
    }
}

impl<'data> Session<'data> {
    /// The 45110e command11 branch for one isolated due-now 0x4000 record.
    /// This bounded case uses the original zero-detail model1744, so
    /// 4450a0 -> 485520 returns without renderer replacement. Other models
    /// still need a synchronous original appearance registry.
    fn restore_immediate_open(
        &mut self,
        command: openmt2001::business_open::Command,
        game_mode: u32,
        canonical_queue_position: bool,
        scripted_catalog_matches_saved: bool,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        use openmt2001::{business_binding::Call, live_commands::{self, Event}};
        let building = command.building as usize;
        let row = self.world.buildings.get(building)
            .ok_or("saved immediate open building leaves pool")?;
        let linked = u32::from_le_bytes(row[0x1c..0x20].try_into().unwrap());
        let model = self.models.appearance_candidates.get(&building)
            .ok_or("saved immediate open lacks prepared model identity")?;
        let category = &self.world.categories[13];
        let building_reference = 0x591318 + (building * 916) as u32;
        let category_word = |at: usize| {
            u32::from_le_bytes(category[at..at + 4].try_into().unwrap())
        };
        if canonical_queue_position && !scripted_catalog_matches_saved {
            return Err("saved immediate open pre-queue BusinessInfo differs from restored row42".into());
        }
        if game_mode != 0 || command.kind != 42 || command.owner != 1
            || command.stock_argument != 1
            || row[0x3f] == 0 || linked != u32::MAX
            || row[4..9] != [0, 0, 6, 0, 4]
            || u32::from_le_bytes(row[0x24..0x28].try_into().unwrap()) != 1
            || row[0x2c] != 2
            || u32::from_le_bytes(row[0x40..0x44].try_into().unwrap())
                != 0x617628 + 13 * 380
            || model.descriptor != 1744 || !model.renderer_candidates.is_empty()
            || !model.renderer_templates.is_empty()
            || category_word(8) != 1
            || category_word(0x15c) != building_reference
            || category_word(0x160) != building_reference
            || u32::from_le_bytes(row[0x44..0x48].try_into().unwrap()) != 0
            || self.world.catalog.info.row(42).is_none_or(|row| row[0x4c] != 0)
        {
            return Err("saved immediate open requires native zero-detail model1744 case".into());
        }
        let context = live_commands::Context {
            image: assets.image,
            clock: &self.world.clock,
            // Original 8b22dc is still zero during 4bc290's queue stream.
            accounting_enabled: false,
            // 801dec is still clear before 4b9f00 rebuilds demand.
            notifications: false,
            tutorial_state: 0,
            network_enabled: false,
            network_override: false,
            utility_factor: self.world.settings.rail_income(self.difficulty)
                .ok_or("saved open utility factor difficulty leaves settings")?,
        };
        let mut messages = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena { next: self.allocation_next },
            current_player: self.roster.status.current_player,
            network: [false, false],
            prior: [0; 48],
        };
        let mut appearance_refreshes = 0;
        let result = live_commands::open(
            &command, &mut self.world.city, &mut self.roster,
            &mut self.population.business_counts, &mut self.hooks.utilities,
            &mut messages, &context,
            |event| match event {
                Event::Business(Call::RefreshBuilding { building: refreshed })
                    if refreshed == building => {
                        appearance_refreshes += 1;
                        Ok(())
                    },
                Event::RefreshInterface => Ok(()),
                other => Err(format!("saved immediate open callback {other:?} has no load-stage owner")),
            },
        );
        self.allocation_next = messages.arena.next;
        result?;
        if appearance_refreshes != 1 {
            return Err("saved immediate open missed synchronous 4450a0 appearance refresh".into());
        }
        restore::activate_zero_model_appearance(
            building,
            restore::AppearanceAlias::Business(
                u32::from_le_bytes(self.world.buildings[building][0x1c..0x20].try_into().unwrap()) as usize,
            ),
            &self.world.buildings[building],
        )?;
        Ok(())
    }

    /// Modern imports recover the visual model from the saved resident identity.
    /// Native 45be80 retains a freshly randomized model while overwriting age.
    /// Keep that low-level contract and strict loads unchanged; modern runtime
    /// extensions already serialize their actual model/controller state.
    pub(crate) fn repair_imported_resident_models(
        &mut self,
        mode: crate::compatibility::Mode,
        has_retained_runtime: bool,
    ) -> Result<usize, String> {
        if mode != crate::compatibility::Mode::Modern || has_retained_runtime {
            return Ok(0);
        }
        let mut repairs = Vec::new();
        for resident in self.world.ledgers.residents.nodes.iter().filter(|node| node.is_active()) {
            if resident.age() > 4 {
                return Err("imported resident age leaves model registry".into());
            }
            let row = usize::from(resident.model_variant());
            let model = &self.resident_models[row];
            let reference = resident.auxiliary_reference();
            let slot = reference.checked_sub(1).ok_or("imported resident has no auxiliary")? as usize;
            if slot >= self.world.creation.auxiliary.used() {
                return Err("imported resident auxiliary is not allocated".into());
            }
            let auxiliary = &self.world.creation.auxiliary.slots()[slot];
            let actual = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
            if actual == model.reference {
                continue;
            }
            let binding_index = self.population.bindings.iter()
                .position(|binding| binding.auxiliary_reference == reference)
                .ok_or("imported resident has no animation binding")?;
            if self.population.bindings[binding_index].auxiliary_slot != slot {
                return Err("imported resident animation binding selects another auxiliary".into());
            }
            if i16::from_le_bytes(auxiliary[0x2e..0x30].try_into().unwrap()) != -1 {
                return Err("imported resident model repair requires an idle load boundary".into());
            }
            let frame = i16::from_le_bytes(auxiliary[0x2c..0x2e].try_into().unwrap());
            let geometry = self.people.geometry_source(row, false).ok_or("imported resident geometry missing")?;
            for lod in [false, true] {
                let source = self.people.geometry_source(row, lod).ok_or("imported resident LOD missing")?;
                if usize::try_from(frame).map_or(true, |frame| frame >= source.frames) {
                    return Err("imported resident frame cannot be retained by identity model".into());
                }
            }
            let frames = u16::try_from(geometry.frames).map_err(|_| "imported resident frame count overflow")?;
            repairs.push((slot, binding_index, row, frames));
        }
        // Validate every repair before changing any retained state.
        for &(slot, binding, row, frames) in &repairs {
            let model = &self.resident_models[row];
            openmt2001::resident::bind_auxiliary_model(
                self.world.creation.auxiliary.get_mut(slot).unwrap(), model.reference, &model.definition,
            );
            self.population.bindings[binding].model_frame_count = frames;
        }
        Ok(repairs.len())
    }

    /// Initialize the matching original level and consume a complete saved
    /// state. `input.scenario` and `input.difficulty` must designate the saved
    /// level; callers can derive them from the original GAMESTART catalog and
    /// tag 2000 before constructing asset adapters.
    pub fn load_original(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
    ) -> Result<Self, String> {
        Self::load_original_with_catalog(input, policy, saved_bytes, None, None, None, None)
    }

    /// Presentation-aware loading returns original synchronous UI calls for
    /// one-shot delivery after the Host has attached its scene/audio channels.
    /// The caller must own every returned call before advancing the Session.
    pub fn load_original_with_presentation(
        input: Inputs<'_, 'data>, policy: Policy, saved_bytes: &[u8],
    ) -> Result<(Self, Vec<openmt2001::category_ownership::Seam>), String> {
        Self::load_original_with_presentation_style(input, policy, saved_bytes, None)
    }

    pub fn load_original_with_presentation_style(
        input: Inputs<'_, 'data>, policy: Policy, saved_bytes: &[u8],
        style: Option<&openmt2001::board_style::Style>,
    ) -> Result<(Self, Vec<openmt2001::category_ownership::Seam>), String> {
        let mut calls = Vec::new();
        let session = Self::load_original_with_catalog(
            input, policy, saved_bytes, None, None, Some(&mut calls), style,
        )?;
        Ok((session, calls))
    }

    pub fn load_original_campaign(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: std::path::PathBuf,
    ) -> Result<Self, String> {
        Self::load_original_with_catalog(
            input,
            policy,
            saved_bytes,
            Some(catalog),
            Some(profile_path),
            None,
            None,
        )
    }

    fn load_original_with_catalog(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
        catalog: Option<openmt2001::level_catalog::Catalog>,
        profile_path: Option<std::path::PathBuf>,
        presentation: Option<&mut Vec<openmt2001::category_ownership::Seam>>,
        style: Option<&openmt2001::board_style::Style>,
    ) -> Result<Self, String> {
        let save = SaveFile::parse(saved_bytes)?;
        save.validate_compatibility(input.image)?;
        let metadata = save.level_metadata()?;
        if metadata[3] as i32 != input.difficulty {
            return Err("saved difficulty differs from selected level input".into());
        }
        let mut catalog = catalog;
        if let Some(ext) = openmt2001::save::extension::Extension::parse(&save)? {
            ext.validate_runtime_section_set()?;
            if ext.version == 5 {
                let row = ext.required(18)?;
                if row.len() != 16 { return Err("invalid Skirmish selection record".into()); }
                let word = |at| u32::from_le_bytes(row[at..at+4].try_into().unwrap());
                let target = f32::from_bits(word(4));
                if word(0) != metadata[0] || !(300000..=300006).contains(&word(0))
                    || word(8) != 0 || word(12) != 1 || !target.is_finite() {
                    return Err("unsupported Skirmish save capability".into());
                }
                let retained = if let Some(catalog) = catalog.as_mut() { catalog } else {
                    catalog = Some(crate::campaign_store::load_catalog(input.image, input.scripts)?);
                    catalog.as_mut().unwrap()
                };
                let index = retained.find(word(0)).ok_or("saved Skirmish missing from catalog")?;
                retained.levels[index].set_objective_target(target);
            }
        }
        let assets = CommandAssets {
            image: input.image,
            board: input.board,
            file: input.file,
        };
        let mut session =
            Self::initialize_with_catalog(input, policy, catalog, profile_path, true, style, crate::compatibility::Mode::StrictV14, None)?;
        if session.catalog.selected_level().map(|level| level.id()) != Some(metadata[0]) {
            return Err("saved level differs from selected original level".into());
        }
        if let Some(calls) = presentation {
            session.restore_original_state_with_presentation(&save, &assets, Some(calls))?;
        } else {
            session.restore_original_state(&save, &assets)?;
        }
        Ok(session)
    }

    /// Current-player transfers use the original self-notice filter. Priced
    /// transfers require the presentation companion to own their cash cues;
    /// appearance and AI callbacks remain outside this bounded path.
    fn restore_immediate_property_transfer(
        &mut self, category: i16, owner: u8, amount: i32, assets: &CommandAssets<'_, '_>,
        mut presentation: Option<&mut Vec<openmt2001::category_ownership::Seam>>,
    ) -> Result<(), String> {
        use openmt2001::live_commands::{self, Event};
        let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let self_buyer = self.roster.manager.players.iter()
            .find(|player| player.reference == self.roster.status.current_player)
            .map(|player| player.bytes[0x1bc].wrapping_add(1));
        if (owner != 0 && Some(owner) != self_buyer)
            || (amount != 0 && (owner == 0 || presentation.is_none()))
            || !(0..52).contains(&category)
            || self.roster.manager.players.iter().any(|player| word(&player.bytes, 0x154) != 0)
            || assets.image.f32_at(0x525040)? != 0.
        {
            return Err("saved immediate property release requires native zero-price City/self-buyer human-only branch or priced self-buyer presentation owner".into());
        }
        let chains = openmt2001::building_pool::category_chains(
            &self.world.categories, &self.world.buildings, 0x591318,
        )?;
        if chains[category as usize].iter().any(|&building| {
            let row = &self.world.buildings[building];
            word(row, 0x4c) == 1
                && (word(row, 0x1c) == u32::MAX || matches!(word(row, 0x24), 0 | 1))
        }) {
            return Err("saved immediate property release needs synchronous building appearance owner".into());
        }
        let clock = self.world.clock;
        let context = live_commands::Context {
            image: assets.image, clock: &clock, accounting_enabled: false,
            notifications: false, tutorial_state: 0, network_enabled: false,
            network_override: false, utility_factor: 0.,
        };
        let mut messages = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena { next: self.allocation_next },
            current_player: self.roster.status.current_player,
            network: [false, false], prior: [0; 48],
        };
        live_commands::property_transfer(
            category, owner, amount, &mut self.world.city, &mut self.roster,
            &mut messages, &context, |event| match event {
                Event::RefreshInterface | Event::PropertyView(_) => Ok(()),
                Event::PropertyOwnership(openmt2001::category_ownership::Seam::NativeUiCall {
                    address, args, argument_count,
                }) if (address == 0x4ad760 && argument_count == 6
                    && matches!(args[0], 0x541cc8 | 0x541cb4)
                    && args[1..] == [0, 0, 0, 1, 0x3c])
                    || (matches!(address, 0x4c6290 | 0x4c62d0)
                        && argument_count == 0 && args == [0; 6]) => {
                    presentation.as_mut().ok_or("saved immediate property cash cue lacks presentation owner")?
                        .push(openmt2001::category_ownership::Seam::NativeUiCall {
                            address, args, argument_count,
                        });
                    Ok(())
                }
                // Actual 4504e0 -> 420500 returns before inserting a notice
                // when owner-1 equals current slot (420509..42051b).
                Event::PropertyTransferNotice { owner: notice_owner, .. }
                    if owner != 0 && Some(notice_owner) == self_buyer => Ok(()),
                other => Err(format!("saved immediate property release callback {other:?} has no load-stage owner")),
            },
        )?;
        self.allocation_next = messages.arena.next;
        self.hooks.utilities.owners = [2,35,39,42].map(|i| self.world.categories[i][5]);
        Ok(())
    }

    fn restore_immediate_sale(
        &mut self, command: openmt2001::business_sale::Command,
        assets: &CommandAssets<'_, '_>, calls: &mut Vec<openmt2001::category_ownership::Seam>,
    ) -> Result<(), String> {
        use openmt2001::{business_sale::{self, Call}, category_ownership::Seam};
        let business = self.world.ledgers.businesses.get(command.business as usize)
            .ok_or("saved immediate sale business leaves pool")?;
        if business[0x21c] == 0 || self.building_selection != [0; 2] {
            return Err("saved immediate sale requires active business and unselected property panel".into());
        }
        let building = i16::from_le_bytes(business[8..10].try_into().unwrap()) as usize;
        let row = self.world.buildings.get(building).ok_or("saved immediate sale building leaves pool")?;
        let word = |at| u32::from_le_bytes(row[at..at+4].try_into().unwrap());
        if row[0x3f] == 0 || word(0x1c) != command.business {
            return Err("saved immediate sale requires matching live building/business links".into());
        }
        let appearance = word(0x4c) == 1 && (word(0x1c) == u32::MAX || matches!(word(0x24),0|1));
        let mut positive_appearance = false;
        if appearance {
            let model = self.models.appearance_candidates.get(&building)
                .ok_or("saved immediate sale lacks prepared appearance model")?;
            let zero = model.descriptor == 1744 && model.renderer_candidates.is_empty()
                && model.renderer_templates.is_empty();
            positive_appearance = checked_positive_sale_appearance(model, row[8]);
            if !zero && !positive_appearance {
                return Err("saved immediate sale appearance has unverified renderer identity".into());
            }
        }
        let current_player = self.roster.manager.players.iter()
            .position(|player| player.reference == self.roster.status.current_player);
        let controls = business_sale::Controls {
            current_player, accounting: false,
            ownership_tick: self.world.clock.timing[1],
            timestamp: self.world.clock.timing[0], hour: self.world.clock.hour() as i32,
        };
        let mut appearance_count = 0;
        let city = &mut self.world.city;
        business_sale::apply(&command, &mut business_sale::World {
            businesses: &mut city.ledgers.businesses,
            buildings: &mut city.buildings,
            players: &mut city.ledgers.players,
            catalog: &city.catalog.info,
            products: &city.catalog.products,
            commodities: &city.commodities.rows,
            age: &mut self.population.business_counts.age,
        }, &controls, assets.image, |call, world| match call {
            Call::NativeUiCall { address, args, argument_count }
                if (address == 0x4ad760 && argument_count == 6
                    && matches!(args[0],0x541cc8|0x541cb4)
                    && args[1..] == [0,0,0,1,0x3c])
                    || (matches!(address,0x4c6290|0x4c62d0)
                        && argument_count == 0 && args == [0;6]) => {
                calls.push(Seam::NativeUiCall { address, args, argument_count }); Ok(())
            }
            Call::BuildingAppearance(id) if appearance && id == building => {
                appearance_count += 1;
                if positive_appearance {
                    // Native command13 reaches 485520 after changing +2d to
                    // City owner. Its 15-case trace changes only the opaque
                    // renderer handles within the 52-byte registry rows;
                    // V2 materializes the City-owned final draws when the
                    // restored scene is assembled after the load stream.
                    if world.buildings[building][0x2d] != 0 {
                        return Err("saved immediate sale positive appearance did not become City-owned".into());
                    }
                    Ok(())
                } else {
                    restore::activate_zero_model_appearance(building,
                        restore::AppearanceAlias::Business(command.business as usize),
                        &world.buildings[building])
                }
            }
            Call::RefreshInterface => Ok(()),
            // Original407660 clears +3e but returns for an unselected object.
            Call::BuildingPanel(id) if id == building => Ok(()),
            other => Err(format!("saved immediate sale callback {other:?} lacks synchronous owner")),
        })?;
        if appearance_count != usize::from(appearance) {
            return Err("saved immediate sale appearance callback count differs".into());
        }
        self.roster.publish_accounts(&self.world.city)?;
        Ok(())
    }

    /// The visible load entry point keeps unsupported native queue and auction
    /// scene tails explicit. Both supplied ordinary saves have neither record.
    pub(crate) fn restore_original_state(
        &mut self,
        save: &SaveFile<'_>,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        self.restore_original_state_with_presentation(save, assets, None)
    }

    fn restore_original_state_with_presentation(
        &mut self, save: &SaveFile<'_>, assets: &CommandAssets<'_, '_>,
        mut presentation: Option<&mut Vec<openmt2001::category_ownership::Seam>>,
    ) -> Result<(), String> {
        if self.saved_game_mode.is_some() {
            return Err("save records require a newly prepared load-mode Session".into());
        }
        save.validate_compatibility(assets.image)?;
        let metadata = save.level_metadata()?;
        if self.catalog.selected_level().map(|level| level.id()) != Some(metadata[0])
            || self.difficulty as u32 != metadata[3]
        {
            return Err("saved level or difficulty differs from retained Session".into());
        }
        let saved_clock = save.clock()?;
        // BusinessInfo+42 is set by native businesssettings.lua before
        // 4bc290 reads queue tags. Tag 0x40000 then restores it afterward.
        // A canonical due-now open is allowed only when this exact scripted
        // pre-queue row equals the saved post-queue row it would otherwise use.
        let scripted_catalog_matches_saved = self.world.catalog.info.row(42)
            == Some(&save.business_catalog()?[42]);
        let scheduled = save.scheduled()?;
        let modern_presentation = crate::session_save::verified_modern_presentation_queue(save)?;
        let last_non_queue_offset = save.sections.iter()
            .filter(|section| section.tag != 0x4000)
            .map(|section| section.offset)
            .max().unwrap_or(0);
        let mut restored_commands = Vec::with_capacity(scheduled.len());
        let mut immediate_terms = Vec::new();
        let mut immediate_open = None;
        let mut immediate_sale = None;
        for (queue_index, (event, section)) in scheduled
            .iter()
            .zip(save.sections.iter().filter(|section| section.tag == 0x4000))
            .enumerate()
        {
            let word = |at: usize| u32::from_le_bytes(event.header[at..at + 4].try_into().unwrap());
            let kind = word(0);
            let domain = match word(4) {
                0 => openmt2001::scheduler::ClockDomain::Total,
                1 => openmt2001::scheduler::ClockDomain::Normal,
                _ => return Err("saved queue record has invalid clock domain".into()),
            };
            let due = word(8);
            // Both 480ec0 clock-list loops dispatch only kind 0 (packet)
            // and kind 1 (category ready). Other kinds are removed through
            // 480e50 without a world callback when their deadline arrives.
            // The original node's heap lifetime has no retained owner here.
            if kind > 1 {
                continue;
            }
            // 450a30's dispatch table sends packet kind14 straight to its
            // return epilogue (45168e), with no world or host callback.
            // The original queue may still allocate/free a node, but it has
            // no retained simulation effect at either admission deadline.
            if kind == 0
                && event
                    .packet
                    .is_some_and(|packet| packet[4..8] == 0x14u32.to_le_bytes())
            {
                continue;
            }
            // 4810e0 treats -1 as an immediate sentinel. A kind-zero node
            // with the total clock equal to due, or a normal-clock node at
            // or before its clock, also calls 450a30 during load. Kind one
            // stores category+78=2 through 480d70/480b70 instead.
            let immediate = due == u32::MAX
                || match domain {
                    openmt2001::scheduler::ClockDomain::Total => due == saved_clock.timing[0],
                    openmt2001::scheduler::ClockDomain::Normal => {
                        (due as i32) <= (saved_clock.timing[1] as i32)
                    }
                };
            if kind == 1 && immediate && modern_presentation[queue_index] {
                continue;
            }
            let admission_due = if let Some(packet) = event.packet {
                u32::from_le_bytes(packet[12..16].try_into().unwrap())
            } else {
                due
            };
            // 450a30's unsigned deadline gate precedes its command switch.
            // Any immediate outer packet with a future inner deadline is
            // copied into 480900's total-clock queue before world dispatch.
            // An inner -1 sentinel recursively enters 450a30 and remains
            // outside this bounded path.
            let readmitted_packet = kind == 0
                && immediate
                && event.packet.is_some()
                && native_future_packet_due(admission_due, saved_clock.timing[0]);
            if kind == 0 && immediate && !readmitted_packet {
                // A verified modern runtime queue restores this exact packet
                // after native record preparation. Its live consumer must run
                // once, after Host has attached scene and presentation channels.
                if modern_presentation[queue_index]
                    && admission_due <= saved_clock.timing[0] {
                    continue;
                }
                if event.packet.is_some_and(|packet| packet[4..8] == 0x13u32.to_le_bytes())
                    && (admission_due == 0 || admission_due == saved_clock.timing[0]) {
                    if save.sections.iter().any(|s|s.tag==openmt2001::save::extension::TAG) {
                        return Err("V2 immediate sale requires authoritative extension queue continuation, not original load-stage presentation".into());
                    }
                    if presentation.is_none() || scheduled.len() != 1
                        || !immediate_sale_after_required_records(save, section.offset) {
                        return Err("saved immediate sale needs isolated original record order and presentation owner".into());
                    }
                    let openmt2001::live_commands::Command::SellBusiness(command) =
                        saved_command(event.packet.unwrap())? else { unreachable!() };
                    // Native queue replay precedes saved catalogs. Check only
                    // rows the original stock initializer actually consumes.
                    let economy = save.economy()?;
                    let business = economy.businesses().nth(command.business as usize)
                        .ok_or("saved immediate sale business leaves pool")?;
                    let kind = business.business_type() as usize;
                    let saved_info = save.business_catalog()?;
                    let info = saved_info.get(kind).ok_or("saved immediate sale catalog kind leaves pool")?;
                    if self.world.catalog.info.row(kind) != Some(info) {
                        return Err("saved immediate sale pre-queue BusinessInfo differs from saved trailing row".into());
                    }
                    let count = if business.discriminator() == 0 { usize::from(info[0x4c]) } else { 0 };
                    let products = self.world.catalog.products.get(kind)
                        .and_then(|rows| rows.get(..count))
                        .ok_or("saved immediate sale product count exceeds six")?;
                    let saved_commodities = save.commodity_rows()?;
                    for product in products {
                        let slot = product[0] as usize;
                        if self.world.commodities.rows.get(slot) != saved_commodities.get(slot)
                            || saved_commodities.get(slot).is_none() {
                            return Err("saved immediate sale pre-queue commodity differs from saved trailing row".into());
                        }
                    }
                    immediate_sale = Some(command);
                    continue;
                }
                if event.packet.is_some_and(|packet| packet[4..8] == 0x11u32.to_le_bytes())
                    && admission_due <= saved_clock.timing[0]
                {
                    // A nonzero overdue inner deadline also reaches 45110e
                    // after 450a9e's diagnostic, before 4634c0 allocates.
                    // For the original writer position, only independent
                    // tail tables may follow. The exact scripted catalog
                    // gate and one-building category gate are checked when
                    // the restored model and list identities are available.
                    let canonical_queue_position = section.offset <= last_non_queue_offset;
                    if scheduled.len() != 1
                        || (canonical_queue_position
                            && !immediate_open_after_required_records(save, section.offset))
                    {
                        return Err("saved immediate open needs isolated original queue order".into());
                    }
                    let openmt2001::live_commands::Command::Open(command) =
                        saved_command(event.packet.unwrap())?
                    else { unreachable!() };
                    immediate_open = Some((command, canonical_queue_position));
                    continue;
                }
                // 450a30 enters 463f70 at the load stream's 0x4000 tag.
                // Its branch without tenant callbacks changes only the
                // restored business row; preserve packet order until the
                // later queue stage after resident/account records load.
                if event
                    .packet
                    .is_some_and(|packet| packet[4..8] == 0x18u32.to_le_bytes())
                    && admission_due <= saved_clock.timing[0]
                {
                    // A nonzero overdue inner deadline visits 450a9e's
                    // diagnostic, then continues through 451522/463f70.
                    // Its no-tenant-callback stores match the due-now path.
                    // The retained loader reconstructs trailing singleton
                    // tables first. Their original writer order leaves the
                    // business row untouched, so this row-only branch has
                    // the same effect at the canonical queue position.
                    if !immediate_terms_after_business_pool(save, section.offset) {
                        return Err("saved immediate terms require original business-before-queue order".into());
                    }
                    let openmt2001::live_commands::Command::Terms(terms) =
                        saved_command(event.packet.unwrap())?
                    else {
                        unreachable!()
                    };
                    immediate_terms.push(terms);
                    continue;
                }
                let wait = event.packet.is_some_and(|packet|
                    packet[4..8] == 0x19u32.to_le_bytes())
                    && (admission_due == 0 || admission_due == saved_clock.timing[0]);
                if wait {
                    if !immediate_wait_after_required_records(save, section.offset) {
                        return Err("saved immediate category wait requires category and clock before queue".into());
                    }
                } else if event.packet.is_some_and(|packet| {
                    packet[4..8] == 0x16u32.to_le_bytes()
                        && packet.len() == 28
                        && (packet[20..24] == [0; 4]
                            || (presentation.is_some() && packet[26] != 0
                                && !save.sections.iter().any(|section|
                                    section.tag == openmt2001::save::extension::TAG)))
                }) && (admission_due == 0 || admission_due == saved_clock.timing[0]) {
                    if scheduled.len() != 1 || !immediate_transfer_after_required_records(save, section.offset) {
                        return Err("saved immediate property release requires original pools-before-queue order".into());
                    }
                    if event.packet.is_some_and(|packet| packet[20..24] != [0; 4])
                        && (save.auction()?.is_some() || save.game_mode()? == 2) {
                        return Err("priced saved self-transfer requires normal original load without auction activation".into());
                    }
                } else {
                    // Other packets still require a synchronous native owner.
                    return Err(
                        "saved immediate packet needs native 4810e0/450a30 synchronous dispatch".into(),
                    );
                }
            }
            let command = match kind {
                0 => saved_command(
                    event
                        .packet
                        .ok_or("saved packet queue entry lacks packet")?,
                )?,
                1 => openmt2001::live_commands::Command::CategoryReady { category: word(12) },
                _ => return Err("saved queue entry has invalid native kind".into()),
            };
            let (queue_domain, queue_due) = if readmitted_packet {
                (openmt2001::scheduler::ClockDomain::Total, admission_due)
            } else {
                (domain, due)
            };
            restored_commands.push((
                command,
                queue_domain,
                queue_due,
                admission_due,
                kind == 1,
                immediate && !readmitted_packet,
            ));
        }
        let saved_auction = save.auction()?;
        let saved_players = save.players()?;
        if saved_players.len() > 6 {
            return Err("saved player roster exceeds six slots".into());
        }
        let saved_residents = save.residents()?;
        let saved_ledgers = economy::Ledgers::from_saved(save)?;
        let saved_pending = save.pending_world()?;
        let saved_objective = save.scenario_objective()?;
        let saved_history = save.player_history()?;
        let cameras = save.cameras()?.map(|row| *row);
        let game_mode = save.game_mode()?;

        // 4bcd34..4bcd57 reset before the full 4bc290 stream. The category
        // list disposal has no renderer here; all linked records are released
        // with the owned building arena before 447bf0's reset pass.
        //447bf0 calls445d10 on category-list heads before4456b0. Tree
        // identities must be retired before their owning tails are zeroed.
        let tree_state = self.models.tree_state.clone();
        if tree_state.is_some() {
            let mut visited = [false; 600];
            let city = &mut self.world.city;
            for category in &city.categories {
                let mut reference = u32::from_le_bytes(category[0x15c..0x160].try_into().unwrap());
                while reference != 0 {
                    let offset = reference.checked_sub(0x591318).ok_or("tree restore owner precedes building pool")?;
                    if !offset.is_multiple_of(916) { return Err("tree restore owner is not record-aligned".into()); }
                    let slot = (offset / 916) as usize;
                    if slot >= 600 || visited[slot] { return Err("tree restore category list is invalid/cyclic".into()); }
                    visited[slot] = true;
                    let record = &mut city.buildings[slot];
                    reference = u32::from_le_bytes(record[0x44..0x48].try_into().unwrap());
                    crate::building_models::release_record_with_trees(record, tree_state.as_ref())?;
                }
            }
        }
        let mut temporary = [0; building_pool::RECORD_SIZE];
        building_pool::reset(&mut self.world.buildings, &mut temporary)?;
        for row in &mut self.world.categories {
            row[0x9c..0xcd].fill(0);
            row[0x15c..0x164].fill(0);
        }
        let reset_values = self.world.definitions.reset_values();
        category_reset::reset_after_lists_removed(&mut self.world.categories, &reset_values);
        // Original 4bcd3e -> 45bac0 frees the script-created auxiliary list
        // (4dfb20), then allocates a fresh 2000-row pool (4dfad0). Preserve
        // creation RNG contexts while dropping those prior render members.
        {
            let city = &mut self.world.city;
            reset_original_resident_pool(&mut city.creation, &mut city.ledgers.residents);
        }
        // 48c7d0/48c800 allocate each manager player, then copy the saved
        // prefix. No AI strategy is decoded from bytes the writer omitted.
        self.roster = level_startup::Roster::new([0; 0x28])?;
        // Player constructors continue the caller-owned process registry.
        self.allocation_next = 0x20000000;
        self.static_rng_tail_next = 0;
        let mut ready = self.world.ai_demand.ready;
        let mut current = 0;
        let deck = &mut self.event_cards;
        for player in &saved_players {
            let allocation_next = &mut self.allocation_next;
            let static_tail = &mut self.static_rng_tail_next;
            let mut constructor = player_constructor::RosterConstruction {
                image: assets.image,
                registry: &mut self.random_registry,
                global_reset: &mut ready,
                allocate: &mut |size| {
                    let reference = *allocation_next;
                    let span = u32::try_from(size)
                        .map_err(|_| "player allocation exceeds DWORD")?
                        .checked_add(0xfff)
                        .ok_or("player allocation alignment overflow")?
                        & !0xfff;
                    *allocation_next = allocation_next
                        .checked_add(span)
                        .ok_or("player allocation arena overflow")?;
                    Ok(Some(player_constructor::Allocation {
                        reference,
                        bytes: vec![0; size],
                    }))
                },
                external_rng_link: &mut |reference, next| {
                    if reference != startup_rng::TAIL || *static_tail != 0 {
                        return Err("saved roster RNG tail mismatch".into());
                    }
                    *static_tail = next;
                    Ok(())
                },
            };
            restore::player(
                player,
                &mut self.roster.manager,
                &mut restore::PlayerContext {
                    profiles: &mut self.roster.profiles,
                    construction: &mut constructor,
                    metadata: &mut self.saved_player_metadata,
                    current_player: &mut current,
                    initial_cash: self.world.player_money,
                },
                |reference, card, manager| {
                    // 48c800 calls 41bc30 after copying the prefix. Its 32
                    // broadcast marks the card used, then 33 delivers it to
                    // this player's newly constructed event auxiliary.
                    deck.draw_specific(card, |dealt, _| {
                        let row = manager
                            .players
                            .iter_mut()
                            .find(|row| row.reference == reference)
                            .ok_or("held-card player disappeared")?;
                        let auxiliary =
                            u32::from_le_bytes(row.bytes[0x21c..0x220].try_into().unwrap());
                        openmt2001::event_cards::lifecycle::receive(row, auxiliary, dealt)
                    })
                },
            )?;
        }
        self.roster.status.current_player = current;
        self.roster.status.ai_count = self
            .roster
            .manager
            .players
            .iter()
            .filter(|player| {
                u32::from_le_bytes(player.bytes[0x154..0x158].try_into().unwrap()) == 1
            })
            .count() as u32;
        self.roster.status.has_ai = self.roster.status.ai_count != 0;
        // Native tag 0x40 player accounts precede tag 0x20 business rows and
        // the 0x4000 queue. Command11 may post to both during queue replay.
        self.world.ledgers.players = saved_ledgers.players;
        // Tag 0x20 precedes tag 4 in the original stream.
        self.world.ledgers.businesses = saved_ledgers.businesses;
        let mut geometry = BuildingGeometry::new(
            assets.board,
            assets.file,
            crate::building_models::Models::with_tree_state(tree_state),
        );
        let city = &mut self.world.city;
        restore::buildings(
            save.building_record()?,
            &mut city.buildings,
            &mut city.categories,
            [0x591318, 0x617628],
            &mut self.building_selection,
            &mut geometry,
        )?;
        self.models = std::mem::take(geometry.registrar_mut());
        // 444ff0/47e5d0 prepares renderer candidates while tag4 buildings
        // are rebound, before the later 4bbd20 appearance walk. Recover the
        // ordered 485200 model identities from the same BuildFile selection
        // used by the retained geometry. Renderer handles remain unresolved.
        for saved in save.buildings()? {
            if self.world.buildings[saved.slot][0x3f] == 0 {
                continue;
            }
            if !self.models.bindings.contains_key(&saved.slot) {
                return Err("saved appearance model has no prepared building binding".into());
            }
            let selected =
                openmt2001::scene::SceneBuilding::from_saved(&saved, assets.board, assets.file)?
                    .selected;
            let identity = openmt2001::save::appearance::model_identity(
                assets.file, selected, saved.height(),
            )?;
            self.models
                .appearance_candidates
                .insert(saved.slot, identity);
        }
        self.world.catalog.info =
            openmt2001::business_pool::BusinessCatalog::from_rows(save.business_catalog()?);
        self.world.commodities.rows = save.commodity_rows()?;
        // 464180 resets business records without clearing the separate
        // 7f3da8 availability list. Scenario replay can populate that list
        // before the second reset; retain its order rather than deriving a
        // sorted list from the saved business bytes.

        // 45be80 reconstructs runtime auxiliary/model state for every saved
        // resident before copying its 14a-byte prefix. 45b3b0 later replaces
        // the entire separate account table, including inactive rows.
        // Resident storage was reset at original4bcd3e/43 before the stream.
        // The original save does not contain RNG state. Keep the stream left
        // by the caller's 4b9f20 preparation and script replay; rebuilding it
        // from the host seed here would rewind resident creation.
        let mut housing = housing::Topology::from_runtime(
            &self.world.buildings,
            &self.world.categories,
            0x591318,
        )?;
        let centers = assets.board.category_geometry().centers;
        let commodities = self.world.commodities.resident_inputs();
        let creation_hour = self.world.clock.hour() as usize;
        let city = &mut self.world.city;
        let personal =
            city.configuration
                .inputs(&commodities, city.commodities.memberships, creation_hour);
        let inputs = economy::ResidentCreationInputs {
            personal: &personal,
            preference_groups: &city.configuration.preference_groups,
            models: &self.resident_models,
            countdown_period: city.settings.itchy_counter,
        };
        for node in saved_residents.nodes {
            city.ledgers
                .residents
                .load_resident(
                    *node,
                    self.policy.auxiliary_prior_bytes,
                    &inputs,
                    &mut city.creation,
                    &mut housing.categories,
                    &centers,
                )
                .map_err(|error| format!("original resident restoration: {error:?}"))?;
        }
        for (row, bytes) in self
            .world
            .ledgers
            .residents
            .accounts
            .iter_mut()
            .zip(saved_residents.accounts.chunks_exact(0x90))
        {
            *row = openmt2001::resident::SpendingAccount::from_bytes(bytes.try_into().unwrap());
        }
        for (row, category) in self.world.categories.iter_mut().zip(&housing.categories) {
            row[0x88..0x8c].copy_from_slice(&category.resident_counter.to_le_bytes());
        }

        self.world.clock = saved_clock;
        self.building_styles = self
            .era_styles
            .get(saved_clock.calendar[2] as usize)
            .ok_or("saved era leaves original style bank")?
            .clone();
        restore::pending_references(
            &saved_pending,
            &self.world.buildings,
            &self.world.categories,
            self.roster.manager.players.len(),
        )?;
        self.world.pending_world = saved_pending;
        self.live_commands = openmt2001::live_commands::Queue::default();
        // The stream's 0x4000 records follow the restored business,
        // building, resident and account records. 463f70 sees those rows
        // synchronously, before 4b9f00/4bbd20 rebuild derived state.
        for pending in &immediate_terms {
            restore::apply_business_terms_without_tenant_callback(
                &mut self.world.ledgers.businesses,
                pending.business_index,
                &pending.terms,
            )?;
        }
        if let Some((command, canonical_queue_position)) = immediate_open {
            self.restore_immediate_open(
                command,
                game_mode,
                canonical_queue_position,
                scripted_catalog_matches_saved,
                assets,
            )?;
        }
        if let Some(command) = immediate_sale {
            self.restore_immediate_sale(command, assets, presentation.take()
                .ok_or("saved immediate sale lost presentation owner")?)?;
        }
        for (command, domain, queue_due, admission_due, direct, immediate) in restored_commands {
            if immediate {
                match command {
                    openmt2001::live_commands::Command::CategoryReady { category } => {
                        openmt2001::category_ownership::end_wait(
                            self.world.categories.get_mut(category as usize)
                                .ok_or("saved immediate category leaves pool")?,
                        );
                    }
                    openmt2001::live_commands::Command::Auction { descriptor }
                        if descriptor[4..8] == 0x19u32.to_le_bytes() => {
                        let category = u32::from_le_bytes(descriptor[20..24].try_into().unwrap());
                        let normal = self.world.clock.timing[1];
                        let due = normal.wrapping_add(15000);
                        if (due as i32) <= (normal as i32) {
                            return Err("saved category wait expiry reaches native immediate normal-event fallthrough".into());
                        }
                        openmt2001::category_ownership::begin_wait(
                            self.world.categories.get_mut(category as usize)
                                .ok_or("saved immediate category wait leaves pool")?, normal,
                        );
                        self.live_commands.restore_queued(
                            openmt2001::live_commands::Command::CategoryReady { category },
                            openmt2001::scheduler::ClockDomain::Normal, due, due, true,
                        );
                    }
                    openmt2001::live_commands::Command::PropertyTransfer { category, owner, amount } => {
                        self.restore_immediate_property_transfer(category, owner, amount, assets,
                            presentation.as_deref_mut())?;
                    }
                    _ => return Err("saved immediate queue entry is not a category event".into()),
                }
            } else {
                self.live_commands.restore_queued(
                    command,
                    domain,
                    queue_due,
                    admission_due,
                    direct,
                );
            }
        }
        if let Some(record) = saved_auction {
            let active = self
                .roster
                .manager
                .players
                .iter()
                .map(|player| player.bytes[0x10])
                .collect::<Vec<_>>();
            let current_slot = self
                .roster
                .manager
                .players
                .iter()
                .find(|player| player.reference == self.roster.status.current_player)
                .ok_or("restored auction current player missing")?
                .bytes[0x1bc];
            let city = &mut self.world.city;
            openmt2001::auction_restore::restore(
                record,
                &mut city.auction,
                &mut city.categories,
                &active,
                current_slot,
                game_mode,
                |call, _, presentation, _| match call {
                    openmt2001::auction_restore::Call::ReleaseAmbient(_)
                    | openmt2001::auction_restore::Call::CategoryStatus(_) => Ok(()),
                    openmt2001::auction_restore::Call::ShowAnnouncement => {
                        presentation.visible = true;
                        Ok(())
                    }
                    // 465810 invokes 4b5370 then 465490 here. The UI cannot
                    // prepare assets until the loaded inspection is delivered;
                    // the retained worker performs 464aa0 via Host afterwards.
                    openmt2001::auction_restore::Call::SetGameMode(_)
                    | openmt2001::auction_restore::Call::PrepareAuctionScene => Ok(()),
                },
            )?;
        }
        self.world.objective = saved_objective;
        self.history = saved_history;
        self.saved_cameras = Some(cameras);
        self.saved_game_mode = Some(game_mode);

        self.world.ai_demand.ready = ready;
        // 4b9f00 runs before 4bbd20 in the original load caller.
        {
            let city = &mut self.world.city;
            let players: Vec<_> = self.roster.manager.players.iter().collect();
            openmt2001::player_metrics::reset(
                &mut city.ai_neighbors,
                &mut city.ownership_refresh_counter,
                &mut self.population.business_counts,
                &mut self.population.market,
                &players,
                &city.categories,
                &city.ledgers.businesses,
                &city.ledgers.residents,
                assets.image,
            )?;
            city.ai_demand.rebuild(
                &city.ledgers.residents,
                &city.ledgers.businesses,
                &city.buildings,
                &city.catalog.info,
                &city.catalog.products,
                &city.commodities.rows,
                assets.image,
            )?;
            openmt2001::ai_space::refresh_all(
                &mut city.category_footprints,
                &city.categories,
                assets.image,
            )?;
            // Native 470d10 also clears a 45-entry transient AI ranking
            // memo at 7ff858 and its 801de8 valid count. The retained
            // ranking query recomputes from this rebuilt city/catalog and
            // has no persistent counterpart to clear.
        }
        // 4bbd20 first refreshes the 51 ordinary category commodity masks.
        // Its loop intentionally stops before the final sentinel category.
        {
            let city = &mut self.world.city;
            let resident_count = city.ledgers.residents.active_count as usize;
            let products = city.catalog.products;
            let commodities = city.commodities.rows;
            let mut world = openmt2001::business_binding::World {
                buildings: &mut city.buildings,
                businesses: &mut city.ledgers.businesses,
                categories: &mut city.categories,
                category_base: 0x617628,
                catalog: &city.catalog.info,
                available: &mut city.available,
                availability_cursor: &mut city.availability_cursor,
                players: &[],
                resident_count,
            };
            for category in 0..51 {
                openmt2001::business_binding::refresh_category(
                    &mut world,
                    category,
                    0x591318,
                    &products,
                    &commodities,
                )?;
            }
        }
        // 4bbd20 invokes 478ea0 only when the restored roster includes AI.
        // The constructor has made the AI allocation and profile link; the
        // later reset consumes the saved player prefix and live city ownership.
        if self.roster.status.has_ai {
            for player in &mut self.roster.manager.players {
                if u32::from_le_bytes(player.bytes[0x154..0x158].try_into().unwrap()) != 1 {
                    continue;
                }
                player.reset_ai(
                    assets.image,
                    &mut self.world.ai_demand.ready,
                    &mut |reference| {
                        self.roster.ai_ownership.lists.remove(&reference);
                        Ok(())
                    },
                )?;
                openmt2001::ai_ownership::maintain_player(
                    &mut self.roster.ai_ownership,
                    player,
                    &self.world.categories,
                    &self.world.buildings,
                    &self.world.ledgers.businesses,
                    assets.image,
                    &self.world.catalog.info,
                    |size| {
                        let reference = self.allocation_next;
                        self.allocation_next = self
                            .allocation_next
                            .checked_add(
                                (size as u32)
                                    .checked_add(0xfff)
                                    .ok_or("AI list allocation overflow")?
                                    & !0xfff,
                            )
                            .ok_or("AI list arena overflow")?;
                        Ok(reference)
                    },
                )?;
            }
            let default_ai = std::fs::read(self.script_directory.join("DEFAULT/ai.lua"))
                .map_err(|error| format!("DEFAULT/ai.lua: {error}"))?;
            let selected = self
                .catalog
                .selected_level()
                .ok_or("loaded catalog level missing")?;
            let directory = std::str::from_utf8(selected.directory())
                .map_err(|_| "loaded catalog directory is not UTF-8")?;
            let scenario_ai =
                match std::fs::read(self.script_directory.join(directory).join("ai.lua")) {
                    Ok(bytes) => Some(bytes),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.to_string()),
                };
            let chains = building_pool::category_chains(
                &self.world.categories,
                &self.world.buildings,
                0x591318,
            )?;
            self.roster.scripts.initialize(
                &mut openmt2001::ai_script::Initialization {
                    profiles: &mut self.roster.profiles,
                    manager: &mut self.roster.manager,
                    category_counts: &mut self.roster.category_counts,
                    chains: &chains,
                    buildings: &self.world.buildings,
                    businesses: &self.world.ledgers.businesses,
                    difficulty: self.difficulty as i32,
                    current_hour: self.world.clock.hour() as i32,
                    count_tick: Some(self.world.clock.timing[1]),
                },
                &default_ai,
                scenario_ai
                    .as_deref()
                    .map(|bytes| ("scenario/ai.lua", bytes)),
            )?;
        }
        for (slot, player) in self.roster.manager.players.iter_mut().enumerate() {
            let metadata = self
                .saved_player_metadata
                .get(slot)
                .ok_or("loaded player metadata exceeds six slots")?;
            // 48cdd2 tests kind at metadata+4, then copies difficulty+2c.
            // The character/style at+0 must not decide whether this is AI.
            if u32::from_le_bytes(metadata[4..8].try_into().unwrap()) == 1 {
                player.bytes[0x13f4..0x13f8].copy_from_slice(&metadata[44..48]);
            }
        }
        // 4bbd20 invokes 485520 in category-list order. The exact walker and
        // zero-model callback are recovered in restore.rs. The saved model
        // selection and ordered 485200 detail identities now live in
        // self.models.appearance_candidates; all 15 supplied SAVE_ONE callback
        // buildings have positive native counts. Retained BuildingGeometry
        // does not own 4846f0 per-detail renderer handles or replacements, so
        // positive 485520 remains unexecuted. Runtime +2e4 stays zero and
        // cannot be used as evidence for the native zero-model branch.
        self.population
            .refresh_city(&self.world.city, assets.board)?;
        self.population
            .refresh_bindings(&self.world.city, &self.resident_models, &self.people)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};
    #[test]
    fn immediate_terms_future_gate_matches_native_unsigned_boundaries() {
        // Native 450a30 uses unsigned JA, not signed or modular distance;
        // 480900 recursively dispatches a future -1 packet deadline.
        for (clock, due, queued) in [
            (50, 51, true),
            (50, 50, false),
            (50, 0, false),
            (50, u32::MAX, false),
            (0x7fff_ffff, 0x8000_0000, true),
            (0x8000_0000, 0x7fff_ffff, false),
            (0x8000_0000, 0x8000_0001, true),
            (0xffff_fffe, 1, false),
            (0xffff_fffe, u32::MAX, false),
            (u32::MAX, u32::MAX, false),
        ] {
            assert_eq!(
                native_future_packet_due(due, clock),
                queued,
                "clock {clock:08x}, packet deadline {due:08x}"
            );
        }
    }
    #[test]
    fn decodes_native_command18_saved_packet() {
        // 464060 submits 20-byte transport header plus the 44-byte
        // business-settings payload; preserve signed targets and float bits.
        let mut packet = [0u8; 64];
        packet[4..8].copy_from_slice(&0x18u32.to_le_bytes());
        packet[8..12].copy_from_slice(&64u32.to_le_bytes());
        packet[20..24].copy_from_slice(&17u32.to_le_bytes());
        for (i, target) in [-1i16, 2, i16::MAX, i16::MIN, 0, 100].iter().enumerate() {
            packet[24 + i * 2..26 + i * 2].copy_from_slice(&target.to_le_bytes());
        }
        let prices: [u32; 6] = [
            0x7fc01234, 0x80000000, 0x3f800000, 0x40400000, 0x40800000, 0x40a00000,
        ];
        for (i, bits) in prices.iter().enumerate() {
            packet[36 + i * 4..40 + i * 4].copy_from_slice(&bits.to_le_bytes());
        }
        packet[60..64].copy_from_slice(&23.5f32.to_bits().to_le_bytes());
        let openmt2001::live_commands::Command::Terms(terms) = saved_command(&packet).unwrap()
        else {
            panic!("command18 did not decode as terms")
        };
        assert_eq!(terms.business_index, 17);
        assert_eq!(
            terms.terms.stock_targets,
            [-1, 2, i16::MAX, i16::MIN, 0, 100]
        );
        assert_eq!(terms.terms.prices.map(f32::to_bits), prices);
        assert_eq!(terms.terms.rent.to_bits(), 23.5f32.to_bits());
        assert!(saved_command(&packet[..63]).is_err());
    }
    #[test]
    #[ignore = "requires original v1.4 inputs and both supplied original saves"]
    fn loads_supplied_original_saves_into_sessions() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(loads_supplied_original_saves_into_sessions_inner)
            .unwrap()
            .join()
            .unwrap();
    }

    fn loads_supplied_original_saves_into_sessions_inner() {
        let read = |name| fs::read(std::env::var(name).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let built = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&built).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let scripts = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent()
            .unwrap()
            .join("scripts");
        let era_styles = super::super::load_era_styles(
            Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
                .parent()
                .unwrap(),
        )
        .unwrap();
        let names = std::array::from_fn(|_| Vec::new());
        for (key, scenario, difficulty, count, businesses) in [
            ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, 340, 15),
            ("OPENMT2001_SAVE_TWO", "sandbox", 0, 500, 0),
        ] {
            let saved = read(key);
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
                        scripts: &scripts,
                        parameters: std::str::from_utf8(&parameters).unwrap(),
                        era_styles: era_styles.clone(),
                        timetable: std::str::from_utf8(&timetable).unwrap(),
                        scenario,
                        difficulty,
                        selected_character: 4,
                        profile_name: b"Saved session",
                        character_names: &names,
                    };
            let policy = || Policy {
                        seed: 0x12345678,
                        auxiliary_prior_bytes: [0; 0x11c],
                    };
            let load = |saved_bytes: &[u8]| {
                Session::load_original(inputs(), policy(), saved_bytes)
            };
            let session = load(&saved).unwrap();
            assert_eq!(session.world.creation.auxiliary.used(), count as usize);
            let mut references = std::collections::BTreeSet::new();
            for node in session.world.ledgers.residents.nodes.iter().filter(|node| node.is_active()) {
                let reference = node.auxiliary_reference();
                assert!((1..=count).contains(&reference));
                assert!(references.insert(reference), "loaded residents share auxiliary slot");
            }
            assert_eq!(references.len(), count as usize);
            let mut appearance_categories = session.world.categories;
            let mut appearance_buildings = session.world.buildings.clone();
            let mut appearance_calls = Vec::new();
            restore::post_load_appearance_walk(
                &mut appearance_categories,
                &mut appearance_buildings,
                0x591318,
                |building, alias, _, _| {
                    appearance_calls.push((building, alias));
                    Ok(())
                },
            )
            .unwrap();
            println!(
                "{key}: {} original 485520 post-load callbacks",
                appearance_calls.len()
            );
            assert_eq!(
                appearance_calls.len(),
                businesses,
                "supplied save post-load appearance callback count"
            );
            if key == "OPENMT2001_SAVE_ONE" {
                let counts: Vec<_> = appearance_calls
                    .iter()
                    .map(|(building, _)| {
                        session.models.appearance_candidates[building]
                            .renderer_candidates
                            .len()
                    })
                    .collect();
                // Native 488230 -> 4d4a00 -> 485200, v1.4 SHA pinned by
                // tools/native_save_appearance_counts.py at FPCW 0x037f.
                assert_eq!(counts, [3, 2, 5, 3, 6, 9, 3, 2, 3, 4, 4, 3, 4, 5, 3]);
                let active = appearance_calls.iter().flat_map(|(building, _)| {
                    &session.models.appearance_candidates[building].renderer_templates
                }).filter(|template| u32::from_le_bytes(template.row[..4].try_into().unwrap()) == 1)
                    .count();
                assert_eq!(active, 45, "native 4846f0 active decoration rows");
            }
            // These are retained V2 counts; original scene registration may
            // populate the native +2e4 count differently before 485520.
            println!(
                "{key}: retained 485520 model counts {:?}",
                appearance_calls
                    .iter()
                    .map(|(building, _)| u32::from_le_bytes(
                        session.world.buildings[*building][0x2e4..0x2e8]
                            .try_into()
                            .unwrap()
                    ))
                    .collect::<Vec<_>>()
            );
            assert_eq!(session.world.ledgers.residents.active_count, count);
            assert_eq!(
                session
                    .world
                    .ledgers
                    .businesses
                    .iter()
                    .filter(|r| r[0x21c] != 0)
                    .count(),
                businesses
            );
            assert_eq!(session.roster.manager.players.len(), 1);
            assert!(session.saved_cameras.is_some());
            let mut expected_demand = openmt2001::ai_demand::State::default();
            let city = &session.world.city;
            expected_demand
                .rebuild(
                    &city.ledgers.residents,
                    &city.ledgers.businesses,
                    &city.buildings,
                    &city.catalog.info,
                    &city.catalog.products,
                    &city.commodities.rows,
                    &image,
                )
                .unwrap();
            let demands = |rows: &[[u8; 264]; 33]| {
                let mut values: Vec<_> = rows
                    .iter()
                    .map(|r| {
                        (
                            u32::from_le_bytes(r[0..4].try_into().unwrap()),
                            u32::from_le_bytes(r[4..8].try_into().unwrap()),
                        )
                    })
                    .collect();
                values.sort_unstable();
                values
            };
            assert_eq!(
                demands(&city.ai_demand.rows),
                demands(&expected_demand.rows),
                "post-load demand counts must reflect saved residents and businesses"
            );
            assert_eq!(city.ai_demand.ready, 1);
            let mut expected_footprints = [[0; 2]; 52];
            openmt2001::ai_space::refresh_all(&mut expected_footprints, &city.categories, &image)
                .unwrap();
            assert_eq!(
                city.category_footprints, expected_footprints,
                "post-load footprints must reflect reconstructed geometry"
            );
            if key == "OPENMT2001_SAVE_ONE" {
                assert_eq!(city.category_footprints[36], [1, 6]);
            }
            let source = SaveFile::parse(&saved).unwrap();
            if key == "OPENMT2001_SAVE_ONE" {
                let current=session.roster.manager.players.iter().position(|p|
                    p.reference==session.roster.status.current_player).unwrap();
                let id=session.world.ledgers.businesses.iter().position(|business|
                    business[0x21c]!=0 && business[10] as usize==current+1).unwrap();
                let mut packet=[0;28];openmt2001::business_sale::Command{business:id as u32,player:current as u8}.write_packet(&mut packet);
                packet[8..12].copy_from_slice(&28u32.to_le_bytes());
                let mut payload=[0;24].to_vec();payload[8..12].copy_from_slice(&session.world.clock.timing[0].to_le_bytes());payload.extend_from_slice(&packet);
                let at=source.sections.iter().find(|s|s.tag==0x10000).unwrap().offset;
                let mut sale_bytes=saved[..at].to_vec();sale_bytes.extend_from_slice(&0x4000u32.to_le_bytes());sale_bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());sale_bytes.extend_from_slice(&payload);sale_bytes.extend_from_slice(&saved[at..]);
                assert!(load(&sale_bytes).err().unwrap().contains("presentation owner"));
                let (sold, _) = Session::load_original_with_presentation(inputs(),policy(),&sale_bytes).unwrap();
                let building = i16::from_le_bytes(session.world.ledgers.businesses[id][8..10]
                    .try_into().unwrap()) as usize;
                assert_eq!(sold.world.buildings[building][0x2d],0);
                assert_eq!(sold.world.ledgers.businesses[id][10],0);
                assert_eq!(sold.models.appearance_candidates[&building],
                    session.models.appearance_candidates[&building]);
                let mut categories=sold.world.categories;
                let mut buildings=sold.world.buildings.clone();
                let mut callbacks=Vec::new();
                restore::post_load_appearance_walk(&mut categories,&mut buildings,0x591318,
                    |slot,alias,_,_| {if slot==building {callbacks.push(alias);} Ok(())}).unwrap();
                assert_eq!(callbacks,[restore::AppearanceAlias::Business(id)]);
                for slot in 0..15 {
                    let identity=&session.models.appearance_candidates[&slot];
                    assert!(checked_positive_sale_appearance(identity,session.world.buildings[slot][8]),
                        "original positive sale identity {slot}");
                }
                let mut unknown_model=session.models.appearance_candidates[&building].clone();
                unknown_model.renderer_templates.iter_mut()
                    .find(|row|u32::from_le_bytes(row.row[..4].try_into().unwrap())==1)
                    .unwrap().descriptor=0x0300_0001;
                assert!(!checked_positive_sale_appearance(&unknown_model,
                    session.world.buildings[building][8]));
                assert!(!checked_positive_sale_appearance(&session.models.appearance_candidates[&building],
                    session.world.buildings[building][8].wrapping_add(1)));
            }
            let section = |tag| {
                source
                    .sections
                    .iter()
                    .find(|record| record.tag == tag)
                    .unwrap()
                    .payload
            };
            assert_eq!(session.world.pending_world.to_bytes(), section(0x800));
            assert_eq!(session.world.objective.to_bytes(), section(0x10000));
            assert_eq!(session.history.as_bytes(), section(0x20000));
            assert_eq!(session.saved_cameras.unwrap()[0], section(0x100));
            assert_eq!(session.saved_cameras.unwrap()[1], section(0x100000));
            let saved_players = source.players().unwrap();
            assert_eq!(
                session.roster.manager.players[0].bytes[4..0x1bc],
                *saved_players[0].bytes
            );
            assert_eq!(session.saved_player_metadata[0], *saved_players[0].metadata);
            for (record, bytes) in session
                .world
                .ledgers
                .businesses
                .iter()
                .zip(section(0x20).chunks_exact(0x220))
            {
                assert_eq!(record.as_slice(), bytes);
            }
            for (record, bytes) in session
                .world
                .ledgers
                .residents
                .accounts
                .iter()
                .zip(section(0x1000).chunks_exact(0x90))
            {
                assert_eq!(record.as_bytes(), bytes);
            }
            for category in 0..52 {
                assert_eq!(
                    session.world.categories[category][5],
                    source.category_owner(category).unwrap()
                );
            }
            if key == "OPENMT2001_SAVE_ONE" {
                let assets = CommandAssets {
                    image: &image,
                    board: &board,
                    file: &file,
                };
                let original_clock = session.world.clock;
                let original_count = session.world.ledgers.residents.active_count;
                assert!(openmt2001::building_pool::can_place(
                    session.world.categories[13][0x9c..0xcd].try_into().unwrap(),
                    [0, 0, 2, 2],
                ));
                let mut with_pending = saved.clone();
                let building_at = source
                    .sections
                    .iter()
                    .find(|record| record.tag == 4)
                    .unwrap()
                    .offset
                    + 8;
                let mut new_building = [0u8; 64];
                new_building.copy_from_slice(&with_pending[building_at..building_at + 64]);
                new_building[..4].copy_from_slice(&15u32.to_le_bytes());
                new_building[0x14..0x18].copy_from_slice(&2u32.to_le_bytes());
                new_building[0x1c..0x20].fill(0xff);
                with_pending[building_at + 15 * 64..building_at + 16 * 64]
                    .copy_from_slice(&new_building);
                // Scenario3's category13 has no saved buildings. Its first
                // slot accepts the copied 0..2 footprint without overlap.
                let empty_category = building_at + 0x9600 + 13 * 140;
                with_pending[empty_category + 8..empty_category + 12]
                    .copy_from_slice(&1u32.to_le_bytes());
                with_pending[empty_category + 12..empty_category + 16]
                    .copy_from_slice(&15u32.to_le_bytes());
                let pending_at = source
                    .sections
                    .iter()
                    .find(|record| record.tag == 0x800)
                    .unwrap()
                    .offset
                    + 8;
                with_pending[pending_at..pending_at + 4].copy_from_slice(&1u32.to_le_bytes());
                for (offset, value) in [
                    (0, 13u32),
                    (4, 15),
                    (8, 1),
                    (12, 1),
                    (16, 0),
                    (20, 42),
                    (24, 1),
                ] {
                    with_pending[pending_at + 4 + offset..pending_at + 8 + offset]
                        .copy_from_slice(&value.to_le_bytes());
                }
                let pending_save = SaveFile::parse(&with_pending).unwrap();
                let mut pending_session = load(&with_pending).unwrap();
                assert!(!openmt2001::building_pool::can_place(
                    pending_session.world.categories[13][0x9c..0xcd]
                        .try_into()
                        .unwrap(),
                    [0, 0, 2, 2],
                ));
                assert_eq!(pending_session.world.pending_world.count, 1);
                assert_eq!(
                    pending_session.world.pending_world.to_bytes(),
                    pending_save.pending_world().unwrap().to_bytes()
                );
                // Resume the saved construction just before its completion
                // callback. 4b0810 decrements progress through signed -1,
                // then 48ad80 queues command11 for the original 25-tick
                // local admission. This verifies the restored record reaches
                // the production pending path with its original player ID.
                for step in 0..2 {
                    pending_session.world.clock.timing[0] = original_clock.timing[0] + step;
                    pending_session
                        .advance_pending(
                            &board,
                            &mut openmt2001::pending_world::Controls {
                                network_enabled: false,
                                network_override: false,
                                effect_model_reference: 0,
                            },
                            |event, _| match event {
                                openmt2001::live_commands::PendingCall::World(
                                    openmt2001::pending_world::Event::Progress { .. },
                                ) => Ok(0),
                                other => {
                                    Err(format!("unexpected restored pending effect {other:?}"))
                                }
                            },
                        )
                        .unwrap();
                }
                assert_eq!(pending_session.world.pending_world.count, 0);
                assert_eq!(
                    u32::from_le_bytes(
                        pending_session.world.buildings[15][0x14..0x18]
                            .try_into()
                            .unwrap()
                    ),
                    4
                );
                pending_session.world.clock.timing[0] = original_clock.timing[0] + 25;
                pending_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(&pending_session.world.buildings[15][0x1c..0x20], &[0xff; 4]);
                pending_session.world.clock.timing[0] = original_clock.timing[0] + 26;
                pending_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_ne!(&pending_session.world.buildings[15][0x1c..0x20], &[0xff; 4]);
                assert_eq!(
                    pending_session
                        .world
                        .ledgers
                        .businesses
                        .iter()
                        .filter(|row| row[0x21c] != 0)
                        .count(),
                    businesses + 1
                );
                let opened = u32::from_le_bytes(
                    pending_session.world.buildings[15][0x1c..0x20]
                        .try_into()
                        .unwrap(),
                ) as usize;
                let row = &pending_session.world.ledgers.businesses[opened];
                assert_eq!(u16::from_le_bytes(row[8..10].try_into().unwrap()), 15);
                assert_eq!(row[0xa], 1);
                assert_eq!(u32::from_le_bytes(row[4..8].try_into().unwrap()), 42);
                let append = |tag: u32, payload: &[u8]| {
                    let mut bytes = saved[..saved.len() - 8].to_vec();
                    bytes.extend_from_slice(&tag.to_le_bytes());
                    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                    bytes.extend_from_slice(payload);
                    bytes.extend_from_slice(&saved[saved.len() - 8..]);
                    bytes
                };
                // A real supplied human-only save admits the native zero-price
                // City release at its source queue position. Category3 has no
                // buildings, so it needs no renderer appearance callbacks.
                assert_ne!(u32::from_le_bytes(session.world.categories[3][0x78..0x7c].try_into().unwrap()), 2, "release must change saved category state");
                let mut release_packet = [0u8; 28];
                release_packet[4..8].copy_from_slice(&0x16u32.to_le_bytes());
                release_packet[8..12].copy_from_slice(&28u32.to_le_bytes());
                release_packet[12..16].copy_from_slice(&original_clock.timing[0].to_le_bytes());
                release_packet[24..26].copy_from_slice(&3i16.to_le_bytes());
                let mut release_header = [0u8; 24];
                release_header[8..12].copy_from_slice(&original_clock.timing[0].to_le_bytes());
                let mut release_payload = release_header.to_vec();
                release_payload.extend_from_slice(&release_packet);
                let mut released = load(&append(0x4000, &release_payload)).unwrap();
                assert_eq!(released.world.categories[3][5], 0);
                assert_eq!(released.world.categories[3][0x6c], 0);
                assert_eq!(u32::from_le_bytes(released.world.categories[3][0x78..0x7c].try_into().unwrap()), 2);
                assert!(released.live_commands.snapshot().unwrap().is_empty());
                let cash_before: Vec<_> = released.world.ledgers.players.iter().map(|p| p.cash).collect();
                released.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(released.world.ledgers.players.iter().map(|p| p.cash).collect::<Vec<_>>(), cash_before);
                // Native 420500 suppresses a current-player buyer notice. A
                // real City-owned property becomes this human's owned property.
                let current_slot = session.roster.manager.players.iter()
                    .find(|player| player.reference == session.roster.status.current_player)
                    .unwrap().bytes[0x1bc];
                let mut purchase_payload = release_payload.clone();
                purchase_payload[50] = current_slot + 1;
                let purchased = load(&append(0x4000, &purchase_payload)).unwrap();
                assert_eq!(purchased.world.categories[3][5], current_slot + 1);
                let purchaser = purchased.roster.manager.players.iter()
                    .find(|player| player.reference == purchased.roster.status.current_player).unwrap();
                assert_eq!(purchaser.bytes[0x178 + 3], 1);
                assert_eq!(purchased.world.ledgers.players.iter().map(|p| p.cash).collect::<Vec<_>>(), cash_before);
                assert!(purchased.live_commands.snapshot().unwrap().is_empty());
                let mut rival_payload = release_payload.clone();
                rival_payload[50] = current_slot + 2;
                assert!(load(&append(0x4000, &rival_payload)).err().unwrap().contains("City/self-buyer"));
                // Price-bearing transfer remains guarded until cash/audio and
                // property-notice callbacks have an actual load-stage owner.
                let mut priced_release = release_payload.clone();
                priced_release[44..48].copy_from_slice(&1i32.to_le_bytes());
                assert!(load(&append(0x4000, &priced_release)).is_err());
                assert!(Session::load_original_with_presentation(inputs(), policy(),
                    &append(0x4000, &priced_release)).is_err(), "priced City release stays gated");
                for amount in [50000i32, -50000] {
                    let mut priced = purchase_payload.clone();
                    priced[44..48].copy_from_slice(&amount.to_le_bytes());
                    let priced_save = append(0x4000, &priced);
                    assert!(load(&priced_save).is_err(), "default loader cannot own priced cash cues");
                    let mode_at = SaveFile::parse(&priced_save).unwrap().sections.iter()
                        .find(|section| section.tag == 0x400000).unwrap().offset + 8;
                    let mut modal = priced_save.clone();
                    modal[mode_at..mode_at + 4].copy_from_slice(&2u32.to_le_bytes());
                    assert!(Session::load_original_with_presentation(inputs(), policy(), &modal)
                        .err().unwrap().contains("without auction activation"));
                    let (paid, calls) = Session::load_original_with_presentation(inputs(), policy(), &priced_save).unwrap();
                    let mut expected = load(&saved).unwrap();
                    let clock = expected.world.clock;
                    let context = openmt2001::live_commands::Context {
                        image: &image, clock: &clock, accounting_enabled: false,
                        notifications: false, tutorial_state: 0, network_enabled: false,
                        network_override: false, utility_factor: 0.,
                    };
                    let mut messages = openmt2001::player_messages::Context {
                        arena: openmt2001::player_messages::Arena { next: expected.allocation_next },
                        current_player: expected.roster.status.current_player,
                        network: [false, false], prior: [0; 48],
                    };
                    let mut expected_calls = Vec::new();
                    openmt2001::live_commands::property_transfer(3, current_slot + 1, amount,
                        &mut expected.world.city, &mut expected.roster, &mut messages, &context,
                        |event| {
                            if let openmt2001::live_commands::Event::PropertyOwnership(call) = event {
                                expected_calls.push(call);
                            }
                            Ok(())
                        }).unwrap();
                    assert_eq!(calls, expected_calls);
                    assert_eq!(paid.world.categories, expected.world.categories);
                    assert_eq!(paid.world.buildings, expected.world.buildings);
                    assert_eq!(paid.world.ledgers.businesses, expected.world.ledgers.businesses);
                    for (actual, expected) in paid.roster.manager.players.iter().zip(&expected.roster.manager.players) {
                        assert_eq!(actual.bytes, expected.bytes);
                    }
                    assert!(paid.live_commands.snapshot().unwrap().is_empty());
                    let mut host = crate::live_host::Host::new([0.; 3]);
                    let (sound, requests) = std::sync::mpsc::channel();
                    host.gameplay_sound = Some(sound);
                    host.apply_restored_presentation(calls).unwrap();
                    assert_eq!(requests.try_iter().map(|request| request.operation).collect::<Vec<_>>(),
                        vec![crate::gameplay_sound::Operation::Ledger { sale: amount < 0 }]);
                    assert!(host.cash_arrow_state().active);
                    assert_eq!(host.cash_arrow_state().frame, if amount < 0 { 41 } else { 0 });
                }
                // A controlled saved tag4 record uses the same v1.4
                // residential corner that native 488230 selects as model
                // 1744. Its 485200 renderer count is zero, so the full
                // 45110e -> 463560 -> 4450a0 -> 485520 load callback can
                // finish without a renderer-owned replacement handle.
                let mut zero_model_save = saved.clone();
                let mut zero_model_building = [0u8; 64];
                zero_model_building[..4].copy_from_slice(&15u32.to_le_bytes());
                zero_model_building[4..9].copy_from_slice(&[0, 0, 6, 0, 4]);
                zero_model_building[0x1c..0x20].fill(0xff);
                zero_model_building[0x24..0x28].copy_from_slice(&1u32.to_le_bytes());
                zero_model_building[0x2c] = 2;
                zero_model_building[0x3f] = 1;
                zero_model_save[building_at + 15 * 64..building_at + 16 * 64]
                    .copy_from_slice(&zero_model_building);
                let category_at = building_at + 0x9600 + 13 * 140;
                zero_model_save[category_at + 8..category_at + 12]
                    .copy_from_slice(&1u32.to_le_bytes());
                zero_model_save[category_at + 12..category_at + 16]
                    .copy_from_slice(&15u32.to_le_bytes());
                let mut zero_packet = [0u8; 36];
                zero_packet[..4].copy_from_slice(&0x525de4u32.to_le_bytes());
                zero_packet[4..8].copy_from_slice(&0x11u32.to_le_bytes());
                zero_packet[8..12].copy_from_slice(&36u32.to_le_bytes());
                zero_packet[20..24].copy_from_slice(&42u32.to_le_bytes());
                zero_packet[24..28].copy_from_slice(&15u32.to_le_bytes());
                zero_packet[28..32].copy_from_slice(&1u32.to_le_bytes());
                zero_packet[32] = 1;
                let mut zero_payload = [0u8; 60];
                zero_payload[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
                zero_payload[24..].copy_from_slice(&zero_packet);
                let mut zero_open_save = zero_model_save[..zero_model_save.len() - 8].to_vec();
                zero_open_save.extend_from_slice(&0x4000u32.to_le_bytes());
                zero_open_save.extend_from_slice(&(zero_payload.len() as u32).to_le_bytes());
                zero_open_save.extend_from_slice(&zero_payload);
                zero_open_save.extend_from_slice(&zero_model_save[zero_model_save.len() - 8..]);
                let mut zero_open = load(&zero_open_save).unwrap();
                assert_eq!(zero_open.models.appearance_candidates[&15].descriptor, 1744);
                assert!(zero_open.models.appearance_candidates[&15].renderer_templates.is_empty());
                let opened = u32::from_le_bytes(zero_open.world.buildings[15][0x1c..0x20].try_into().unwrap()) as usize;
                assert_ne!(opened, u32::MAX as usize);
                let opened_row = &zero_open.world.ledgers.businesses[opened];
                assert_eq!(u32::from_le_bytes(opened_row[4..8].try_into().unwrap()), 42);
                assert_eq!(u16::from_le_bytes(opened_row[8..10].try_into().unwrap()), 15);
                assert_eq!(opened_row[0xa], 1);
                // Native zero-profile command11 writes zero to +0x18 and
                // +0x28 at 46369c and 4629e1, under FPCW 0x037f.
                assert_eq!(&opened_row[0x18..0x1a], &[0, 0]);
                assert_eq!(&opened_row[0x28..0x2c], &[0, 0, 0, 0]);
                assert_eq!(zero_open.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count(), businesses+1);
                // Native businesssettings.lua property setters finish before
                // 4bc290 begins the save stream. For this single-building
                // kind42 case, their complete row matches the later saved
                // BusinessInfo row; no product/commodity entries are read.
                let native_properties = read("OPENMT2001_NATIVE_BUSINESS_PROPERTIES");
                assert_eq!(&native_properties[..8], b"MTBIP001");
                let row42_at = 16 + 42 * 120 + 4;
                assert_eq!(
                    &native_properties[row42_at..row42_at + 116],
                    source.business_catalog().unwrap()[42].as_slice(),
                );
                let queue_at = source.sections.iter()
                    .find(|section| section.tag == 0x10000).unwrap().offset;
                let mut canonical_open_save = zero_model_save[..queue_at].to_vec();
                canonical_open_save.extend_from_slice(&0x4000u32.to_le_bytes());
                canonical_open_save.extend_from_slice(&(zero_payload.len() as u32).to_le_bytes());
                canonical_open_save.extend_from_slice(&zero_payload);
                canonical_open_save.extend_from_slice(&zero_model_save[queue_at..]);
                let canonical_open = load(&canonical_open_save).unwrap();
                assert_eq!(
                    canonical_open.world.ledgers.businesses[opened],
                    zero_open.world.ledgers.businesses[opened],
                    "original writer queue position should preserve the complete opened row",
                );
                assert_eq!(
                    canonical_open.world.buildings[15][..64],
                    zero_open.world.buildings[15][..64],
                );
                // Re-export the native-backed opened fixture, then admit a
                // single sale at the original writer's queue position. This
                // source corner has zero capital: ownership/account semantics
                // are tested here; nonzero cash cues use the128 native cases.
                let cameras=zero_open.saved_cameras.unwrap();
                let exported=zero_open.save_v2(&image,cameras,1).unwrap();
                let exported_save=SaveFile::parse(&exported).unwrap();
                let extension_at=exported_save.sections.iter().find(|s|s.tag==openmt2001::save::extension::TAG).unwrap().offset;
                let mut native=exported[..extension_at].to_vec();native.extend_from_slice(&exported[exported.len()-8..]);
                let native_save=SaveFile::parse(&native).unwrap();
                let queue_at=native_save.sections.iter().find(|s|s.tag==0x10000).unwrap().offset;
                let mut packet=[0;28];openmt2001::business_sale::Command{business:opened as u32,player:0}.write_packet(&mut packet);
                packet[8..12].copy_from_slice(&28u32.to_le_bytes());
                let mut payload=[0;24].to_vec();payload[8..12].copy_from_slice(&zero_open.world.clock.timing[0].to_le_bytes());payload.extend_from_slice(&packet);
                let mut sale=native[..queue_at].to_vec();sale.extend_from_slice(&0x4000u32.to_le_bytes());sale.extend_from_slice(&(payload.len() as u32).to_le_bytes());sale.extend_from_slice(&payload);sale.extend_from_slice(&native[queue_at..]);
                assert!(load(&sale).err().unwrap().contains("presentation owner"));
                let (sold,calls)=Session::load_original_with_presentation(inputs(),policy(),&sale).unwrap();
                let value=openmt2001::ai_actions::liquidation_score::business_value(
                    &zero_open.world.ledgers.businesses[opened],&zero_open.world.buildings,&image).unwrap().to_f32();
                assert_eq!(value,0.);assert!(calls.is_empty());
                assert_eq!(sold.world.ledgers.businesses[opened][10],0);
                assert_eq!(sold.world.buildings[15][0x2d],0);
                assert_eq!(sold.models.appearance_candidates[&15].descriptor,1744);
                assert!(sold.live_commands.snapshot().unwrap().is_empty());
                assert_eq!(sold.world.ledgers.players.iter().map(|p|p.cash.to_bits()).collect::<Vec<_>>(),
                    zero_open.world.ledgers.players.iter().map(|p|p.cash.to_bits()).collect::<Vec<_>>());
                let sale_save=SaveFile::parse(&sale).unwrap();
                let info_at=sale_save.sections.iter().find(|s|s.tag==0x40000).unwrap().offset+8+42*116;
                let mut changed=sale.clone();changed[info_at]^=1;
                assert!(Session::load_original_with_presentation(inputs(),policy(),&changed).err().unwrap()
                    .contains("pre-queue BusinessInfo"));
                println!("zero-detail1744 original due-now sale: City owner, unchanged zero-capital cash, empty queue");
                let mut changed_catalog = canonical_open_save.clone();
                let catalog_at = source.sections.iter()
                    .find(|section| section.tag == 0x40000).unwrap().offset
                    + 8 + (8 + zero_payload.len()) + 42 * 0x74 + 0x60;
                changed_catalog[catalog_at] ^= 1;
                assert!(
                    load(&changed_catalog).err().unwrap()
                        .contains("pre-queue BusinessInfo differs"),
                    "a later saved catalog change must not be used at the queue's earlier position",
                );
                // 450a30's future-inner gate is shared by command11. The
                // outer -1 event may be immediate while its open waits on
                // the total clock; opening at load would bind too early.
                let mut deferred_open_save = zero_open_save.clone();
                let deferred_due = original_clock.timing[0] + 2;
                let packet_due_at = deferred_open_save.len() - 8 - 36 + 12;
                deferred_open_save[packet_due_at..packet_due_at + 4]
                    .copy_from_slice(&deferred_due.to_le_bytes());
                let mut deferred_open = load(&deferred_open_save).unwrap();
                assert_eq!(
                    deferred_open.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count(),
                    businesses,
                );
                deferred_open.world.clock.timing[0] = deferred_due - 1;
                deferred_open.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    deferred_open.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count(),
                    businesses,
                );
                deferred_open.world.clock.timing[0] = deferred_due;
                deferred_open.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    deferred_open.world.ledgers.businesses.iter().filter(|row|row[0x21c]!=0).count(),
                    businesses + 1,
                );
                let mut overdue_open_save = zero_open_save.clone();
                overdue_open_save[packet_due_at..packet_due_at + 4]
                    .copy_from_slice(&(original_clock.timing[0] - 1).to_le_bytes());
                let overdue_open = load(&overdue_open_save).unwrap();
                assert_eq!(
                    overdue_open.world.ledgers.businesses[opened],
                    zero_open.world.ledgers.businesses[opened],
                    "overdue command11 should bind the same zero-detail business",
                );
                let mut positive = zero_open_save.clone();
                let mut positive_building = [0u8; 64];
                positive_building.copy_from_slice(&saved[building_at..building_at + 64]);
                positive_building[..4].copy_from_slice(&15u32.to_le_bytes());
                positive_building[0x1c..0x20].fill(0xff);
                positive[building_at + 15 * 64..building_at + 16 * 64]
                    .copy_from_slice(&positive_building);
                let positive_error = load(&positive).err().unwrap();
                assert!(positive_error.contains("native zero-detail model1744"), "{positive_error}");
                let mut pending = [0u8; 24];
                pending[..4].copy_from_slice(&1u32.to_le_bytes());
                pending[4..8].copy_from_slice(&1u32.to_le_bytes());
                let due = original_clock.timing[1].wrapping_add(2);
                pending[8..12].copy_from_slice(&due.to_le_bytes());
                pending[12..16].copy_from_slice(&36u32.to_le_bytes());
                let queued = append(0x4000, &pending);
                let queued = SaveFile::parse(&queued).unwrap();
                assert_eq!(queued.scheduled().unwrap().len(), 1);
                let mut queued_session = load(&append(0x4000, &pending)).unwrap();
                queued_session.world.categories[36][0x78..0x7c].fill(0);
                queued_session.world.clock.timing[1] = due.wrapping_sub(1);
                queued_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(&queued_session.world.categories[36][0x78..0x7c], &[0; 4]);
                queued_session.world.clock.timing[1] = due;
                queued_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    u32::from_le_bytes(
                        queued_session.world.categories[36][0x78..0x7c]
                            .try_into()
                            .unwrap()
                    ),
                    2
                );
                // Native 4810e0 dispatches kind one synchronously for the
                // -1 sentinel and for a normal-clock deadline already due.
                // The restored category state must be visible before any
                // post-load Session tick.
                for (domain, immediate_due) in [
                    (0u32, u32::MAX),
                    (0, original_clock.timing[0]),
                    (1, u32::MAX),
                    (1, original_clock.timing[1]),
                    (1, original_clock.timing[1].wrapping_sub(1)),
                ] {
                    let mut immediate = pending;
                    immediate[4..8].copy_from_slice(&domain.to_le_bytes());
                    immediate[8..12].copy_from_slice(&immediate_due.to_le_bytes());
                    let ready_session = load(&append(0x4000, &immediate)).unwrap();
                    assert_eq!(
                        u32::from_le_bytes(
                            ready_session.world.categories[36][0x78..0x7c]
                                .try_into()
                                .unwrap()
                        ),
                        2,
                        "domain {domain}, due {immediate_due}"
                    );
                }

                // Native 4810e0 enters 450a30 command11 immediately for
                // the sentinel/equal outer deadline. The retained narrow
                // owner requires an active, prepared zero-detail building.
                let mut open_packet = [0u8; 0x24];
                open_packet[4..8].copy_from_slice(&0x11u32.to_le_bytes());
                open_packet[8..12].copy_from_slice(&0x24u32.to_le_bytes());
                open_packet[20..24].copy_from_slice(&0u32.to_le_bytes());
                open_packet[24..28].copy_from_slice(&15u32.to_le_bytes());
                open_packet[28..32].copy_from_slice(&1u32.to_le_bytes());
                let mut open_header = [0u8; 24];
                open_header[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
                let mut open_payload = open_header.to_vec();
                open_payload.extend_from_slice(&open_packet);
                assert!(
                    load(&append(0x4000, &open_payload))
                        .err()
                        .unwrap()
                        .contains("lacks prepared model identity")
                );
                open_header[8..12]
                    .copy_from_slice(&original_clock.timing[0].wrapping_add(2).to_le_bytes());
                open_payload[..24].copy_from_slice(&open_header);
                let _future_open = load(&append(0x4000, &open_payload)).unwrap();

                // Kind-zero saved packets have two clocks: the outer node is
                // eligible on the normal list now, but 450a30 readmits this
                // command to the total list at packet+0xc. Command19 begins
                // the real category wait only at that inner deadline.
                let mut packet = [0u8; 52];
                packet[4..8].copy_from_slice(&0x19u32.to_le_bytes());
                packet[8..12].copy_from_slice(&52u32.to_le_bytes());
                let packet_due = original_clock.timing[0].wrapping_add(2);
                packet[12..16].copy_from_slice(&packet_due.to_le_bytes());
                packet[20..24].copy_from_slice(&36u32.to_le_bytes());
                let mut header = [0u8; 24];
                header[4..8].copy_from_slice(&1u32.to_le_bytes());
                let outer_due = original_clock.timing[1].wrapping_add(1);
                header[8..12].copy_from_slice(&outer_due.to_le_bytes());
                let mut payload = header.to_vec();
                payload.extend_from_slice(&packet);
                // A due inner command19 executes at the source queue position,
                // preserving its order with synchronous category-ready rows.
                let mut immediate_wait = payload.clone();
                immediate_wait[8..12].copy_from_slice(&original_clock.timing[1].to_le_bytes());
                immediate_wait[36..40].copy_from_slice(&original_clock.timing[0].to_le_bytes());
                for ready_after in [false, true] {
                    let mut waited_bytes = append(0x4000, &immediate_wait);
                    let mut ready = [0u8; 24];
                    ready[..4].copy_from_slice(&1u32.to_le_bytes());
                    ready[4..8].copy_from_slice(&1u32.to_le_bytes());
                    ready[8..12].copy_from_slice(&original_clock.timing[1].to_le_bytes());
                    ready[12..16].copy_from_slice(&36u32.to_le_bytes());
                    let queue_offset = waited_bytes.len() - 8 - immediate_wait.len() - 8;
                    let mut record = 0x4000u32.to_le_bytes().to_vec();
                    record.extend_from_slice(&24u32.to_le_bytes());
                    record.extend_from_slice(&ready);
                    let insert = if ready_after { waited_bytes.len() - 8 } else { queue_offset };
                    waited_bytes.splice(insert..insert, record);
                    let mut waited = load(&waited_bytes).unwrap();
                    let mut expected = session.world.categories[36];
                    openmt2001::category_ownership::begin_wait(&mut expected, original_clock.timing[1]);
                    if ready_after { openmt2001::category_ownership::end_wait(&mut expected); }
                    assert_eq!(waited.world.categories[36], expected, "saved wait source order {ready_after}");
                    waited.world.clock.timing[1] = original_clock.timing[1] + 14999;
                    waited.drain_commands(&assets, |_| Ok(())).unwrap();
                    assert_eq!(waited.world.categories[36], expected);
                    waited.world.clock.timing[1] += 1;
                    waited.drain_commands(&assets, |_| Ok(())).unwrap();
                    openmt2001::category_ownership::end_wait(&mut expected);
                    assert_eq!(waited.world.categories[36], expected);
                }
                let mut immediate_packet = payload.clone();
                immediate_packet[8..12].copy_from_slice(&original_clock.timing[1].to_le_bytes());
                // A due outer normal-clock command19 is readmitted on the
                // total clock at packet+0xc. The category wait must begin
                // only when that inner deadline arrives.
                let mut readmitted_category =
                    load(&append(0x4000, &immediate_packet)).unwrap();
                readmitted_category.world.categories[36][0x78..0x7c].fill(0);
                readmitted_category.world.clock.timing[0] = packet_due - 1;
                readmitted_category.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(&readmitted_category.world.categories[36][0x78..0x7c], &[0; 4]);
                readmitted_category.world.clock.timing[0] = packet_due;
                readmitted_category.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    &readmitted_category.world.categories[36][0x78..0x7c],
                    &1u32.to_le_bytes(),
                );
                let mut noop = [0u8; 24];
                noop[4..8].copy_from_slice(&0x14u32.to_le_bytes());
                noop[8..12].copy_from_slice(&24u32.to_le_bytes());
                for outer_due in [original_clock.timing[1], original_clock.timing[1] + 1] {
                    let mut noop_header = header;
                    noop_header[8..12].copy_from_slice(&outer_due.to_le_bytes());
                    let mut noop_payload = noop_header.to_vec();
                    noop_payload.extend_from_slice(&noop);
                    let mut noop_session = load(&append(0x4000, &noop_payload)).unwrap();
                    let before = noop_session.world.categories[36];
                    noop_session.world.clock.timing[1] = outer_due + 1;
                    noop_session.drain_commands(&assets, |_| Ok(())).unwrap();
                    assert_eq!(noop_session.world.categories[36], before);
                }
                // 480ec0 removes nonpacket scheduled kinds above one without
                // command or category dispatch. This kind-two record is a
                // valid writer-sized queue node on the normal clock list.
                let mut inert = [0u8; 24];
                inert[..4].copy_from_slice(&2u32.to_le_bytes());
                inert[4..8].copy_from_slice(&1u32.to_le_bytes());
                let inert_due = original_clock.timing[1] + 1;
                inert[8..12].copy_from_slice(&inert_due.to_le_bytes());
                inert[12..16].copy_from_slice(&36u32.to_le_bytes());
                let mut inert_session = load(&append(0x4000, &inert)).unwrap();
                let before = inert_session.world.categories[36];
                inert_session.world.clock.timing[1] = inert_due;
                inert_session
                    .drain_commands(&assets, |_| Err("inert saved queue node dispatched".into()))
                    .unwrap();
                assert_eq!(inert_session.world.categories[36], before);
                let mut packet_session = load(&append(0x4000, &payload)).unwrap();
                packet_session.world.categories[36][0x78..0x7c].fill(0);
                packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(&packet_session.world.categories[36][0x78..0x7c], &[0; 4]);
                packet_session.world.clock.timing[1] = outer_due;
                packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(&packet_session.world.categories[36][0x78..0x7c], &[0; 4]);
                packet_session.world.clock.timing[0] = packet_due.wrapping_sub(1);
                packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(&packet_session.world.categories[36][0x78..0x7c], &[0; 4]);
                packet_session.world.clock.timing[0] = packet_due;
                packet_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    u32::from_le_bytes(
                        packet_session.world.categories[36][0x78..0x7c]
                            .try_into()
                            .unwrap()
                    ),
                    1
                );
                let mut terms_packet = [0u8; 64];
                terms_packet[4..8].copy_from_slice(&0x18u32.to_le_bytes());
                terms_packet[8..12].copy_from_slice(&64u32.to_le_bytes());
                let terms_due = original_clock.timing[0].wrapping_add(3);
                terms_packet[12..16].copy_from_slice(&terms_due.to_le_bytes());
                let business = &session.world.ledgers.businesses[0];
                terms_packet[20..24].copy_from_slice(&business[0x210..0x214]);
                for i in 0..6 {
                    terms_packet[24 + i * 2..26 + i * 2]
                        .copy_from_slice(&(10 + i as i16).to_le_bytes());
                    terms_packet[36 + i * 4..40 + i * 4]
                        .copy_from_slice(&(4.0 + i as f32).to_le_bytes());
                }
                terms_packet[60..64].copy_from_slice(&20.0f32.to_le_bytes());
                let mut terms_payload = header.to_vec();
                terms_payload.extend_from_slice(&terms_packet);
                let mut terms_session = load(&append(0x4000, &terms_payload)).unwrap();
                terms_session.world.clock.timing[1] = outer_due;
                terms_session.world.clock.timing[0] = terms_due.wrapping_sub(1);
                terms_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_ne!(
                    &terms_session.world.ledgers.businesses[0][0x1e0..0x1e2],
                    &10i16.to_le_bytes()
                );
                terms_session.world.clock.timing[0] = terms_due;
                terms_session.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    &terms_session.world.ledgers.businesses[0][0x1e0..0x1e2],
                    &10i16.to_le_bytes()
                );
                assert_eq!(
                    &terms_session.world.ledgers.businesses[0][0x1c4..0x1c8],
                    &4.0f32.to_le_bytes()
                );
                // 4810e0 immediately admits the outer -1 event, and
                // 450a30 requeues command18 on total time terms_due.
                let mut readmitted_terms = terms_payload.clone();
                readmitted_terms[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
                let mut readmitted = load(&append(0x4000, &readmitted_terms)).unwrap();
                assert_ne!(
                    &readmitted.world.ledgers.businesses[0][0x1e0..0x1e2],
                    &10i16.to_le_bytes()
                );
                readmitted.world.clock.timing[0] = terms_due.wrapping_sub(1);
                readmitted.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_ne!(
                    &readmitted.world.ledgers.businesses[0][0x1e0..0x1e2],
                    &10i16.to_le_bytes()
                );
                readmitted.world.clock.timing[0] = terms_due;
                readmitted.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    &readmitted.world.ledgers.businesses[0][0x1e0..0x1e2],
                    &10i16.to_le_bytes()
                );
                assert_eq!(
                    &readmitted.world.ledgers.businesses[0][0x1c4..0x1c8],
                    &4.0f32.to_le_bytes()
                );
                for inner_due in [original_clock.timing[0], u32::MAX] {
                    readmitted_terms[24 + 12..24 + 16].copy_from_slice(&inner_due.to_le_bytes());
                    let error = load(&append(0x4000, &readmitted_terms)).err().unwrap();
                    assert!(error.contains(if inner_due == u32::MAX {
                        "450a30 synchronous dispatch"
                    } else {
                        "synchronous tenant callbacks"
                    }), "{error}");
                }
                // 463f70 completes synchronously for an unoccupied saved
                // business. Slot 9 has no tenant chain in this supplied save;
                // slot 0 still requires the original tenant callback path.
                let unoccupied = &session.world.ledgers.businesses[9];
                assert_eq!(&unoccupied[0x20c..0x210], &(-1i32).to_le_bytes());
                let mut now = terms_payload.clone();
                now[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
                now[24 + 12..24 + 16].copy_from_slice(&0u32.to_le_bytes());
                now[24 + 20..24 + 24].copy_from_slice(&unoccupied[0x210..0x214]);
                let loaded_now = load(&append(0x4000, &now)).unwrap();
                let changed = &loaded_now.world.ledgers.businesses[9];
                assert_eq!(&changed[0x1e0..0x1e2], &10i16.to_le_bytes());
                assert_eq!(&changed[0x1c4..0x1c8], &4.0f32.to_le_bytes());
                assert_eq!(&changed[0x204..0x208], &20.0f32.to_le_bytes());
                assert_ne!(&business[0x20c..0x210], &(-1i32).to_le_bytes());
                let old_rent = f32::from_le_bytes(business[0x1dc..0x1e0].try_into().unwrap());
                assert!(old_rent.is_finite());
                let lower_rent = old_rent - 1.0;
                let mut occupied_decrease = now.clone();
                occupied_decrease[24 + 20..24 + 24]
                    .copy_from_slice(&business[0x210..0x214]);
                occupied_decrease[24 + 60..24 + 64]
                    .copy_from_slice(&lower_rent.to_le_bytes());
                let occupied = load(&append(0x4000, &occupied_decrease)).unwrap();
                let occupied_row = &occupied.world.ledgers.businesses[0];
                assert_eq!(&occupied_row[0x20c..0x210], &business[0x20c..0x210]);
                assert_eq!(&occupied_row[0x1e0..0x1e2], &10i16.to_le_bytes());
                assert_eq!(&occupied_row[0x1dc..0x1e0], &lower_rent.to_le_bytes());
                let mut overdue_now = now.clone();
                overdue_now[24 + 12..24 + 16]
                    .copy_from_slice(&(original_clock.timing[0] - 1).to_le_bytes());
                let overdue = load(&append(0x4000, &overdue_now)).unwrap();
                assert_eq!(
                    overdue.world.ledgers.businesses[9],
                    loaded_now.world.ledgers.businesses[9],
                    "overdue command18 should finish the same unoccupied branch",
                );
                let queue_at = source.sections.iter()
                    .find(|section| section.tag == 0x10000).unwrap().offset;
                let mut canonical = saved[..queue_at].to_vec();
                canonical.extend_from_slice(&0x4000u32.to_le_bytes());
                canonical.extend_from_slice(&(now.len() as u32).to_le_bytes());
                canonical.extend_from_slice(&now);
                canonical.extend_from_slice(&saved[queue_at..]);
                let canonical_loaded = load(&canonical).unwrap();
                assert_eq!(
                    canonical_loaded.world.ledgers.businesses[9],
                    loaded_now.world.ledgers.businesses[9],
                    "original writer queue position should retain command18 row effects",
                );
                let business_at = source.sections.iter()
                    .find(|section| section.tag == 0x20).unwrap().offset;
                let mut before_business = saved[..business_at].to_vec();
                before_business.extend_from_slice(&0x4000u32.to_le_bytes());
                before_business.extend_from_slice(&(now.len() as u32).to_le_bytes());
                before_business.extend_from_slice(&now);
                before_business.extend_from_slice(&saved[business_at..]);
                assert!(load(&before_business).err().unwrap()
                    .contains("business-before-queue order"));
                now[24 + 20..24 + 24].copy_from_slice(&business[0x210..0x214]);
                assert!(
                    load(&append(0x4000, &now))
                        .err()
                        .unwrap()
                        .contains("synchronous tenant callbacks")
                );
                let mut auction = [0u8; 344];
                auction[0x7d] = 1;
                auction[4..8].copy_from_slice(&36u32.to_le_bytes());
                auction[0x80..0x84].copy_from_slice(&100u32.to_le_bytes());
                let auction_session = load(&append(0x8000, &auction)).unwrap();
                assert_eq!(
                    auction_session.world.city.auction.controller.reference,
                    0x7f4758
                );
                assert_eq!(
                    auction_session.world.city.auction.controller.record[0x7d],
                    1
                );
                assert_eq!(auction_session.world.city.categories[36][0x6c], 1);
                assert_eq!(
                    auction_session.world.city.auction.presentation.countdown,
                    100
                );
                assert!(auction_session.world.city.auction.presentation.visible);
                let mut mode2 = append(0x8000, &auction);
                let mode_record = SaveFile::parse(&mode2)
                    .unwrap()
                    .sections
                    .iter()
                    .find(|section| section.tag == 0x400000)
                    .unwrap()
                    .offset
                    + 8;
                mode2[mode_record..mode_record + 4].copy_from_slice(&2u32.to_le_bytes());
                let mode2_session = load(&mode2).unwrap();
                assert_eq!(mode2_session.saved_game_mode, Some(2));
                assert_eq!(
                    mode2_session.world.city.auction.controller.reference,
                    0x7f4758
                );
                assert_eq!(mode2_session.world.city.auction.controller.record[0x7d], 1);
                assert!(!mode2_session.world.city.auction.presentation.visible);
                // The UI receives the retained Session before Host invokes
                // 464aa0 through its synchronous renderer bridge.

                // Both supplied files have only a human. Insert a second
                // native metadata/player pair immediately after the existing
                // pair to exercise the 48c800 AI constructor and 4bbd20
                // post-load strategy activation on a retained opponent.
                let player_section = source
                    .sections
                    .iter()
                    .find(|section| section.tag == 0x40)
                    .unwrap();
                let insert_at = player_section.offset + 8 + player_section.payload.len();
                let mut metadata = *saved_players[0].metadata;
                metadata[..4].copy_from_slice(&5u32.to_le_bytes());
                metadata[4..8].copy_from_slice(&1u32.to_le_bytes());
                metadata[44..48].copy_from_slice(&2u32.to_le_bytes());
                let mut opponent = *saved_players[0].bytes;
                opponent[0x14c..0x150].copy_from_slice(&5u32.to_le_bytes());
                opponent[0x150..0x154].copy_from_slice(&1u32.to_le_bytes());
                opponent[0x1b4..0x1b8].fill(0); // no held event card
                let mut pair = Vec::new();
                for (tag, payload) in [
                    (0x80000u32, metadata.as_slice()),
                    (0x40, opponent.as_slice()),
                ] {
                    pair.extend_from_slice(&tag.to_le_bytes());
                    pair.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                    pair.extend_from_slice(payload);
                }
                let mut with_opponent = saved[..insert_at].to_vec();
                with_opponent.extend_from_slice(&pair);
                with_opponent.extend_from_slice(&saved[insert_at..]);
                let mut opponent_session = load(&with_opponent).unwrap();
                assert_eq!(opponent_session.roster.manager.players.len(), 2);
                assert_eq!(opponent_session.roster.status.ai_count, 1);
                assert!(opponent_session.roster.status.has_ai);
                assert_eq!(
                    opponent_session.roster.manager.players[1].bytes.len(),
                    0x1e50
                );
                assert_eq!(
                    &opponent_session.roster.manager.players[1].bytes[0x13f4..0x13f8],
                    &2u32.to_le_bytes()
                );
                assert!(
                    opponent_session.roster.profiles[5]
                        .iter()
                        .any(|byte| *byte != 0),
                    "4d7a80 must initialize the restored AI's script profile"
                );
                // One retained AI controller pass consumes the reconstructed
                // player/profile/city rather than merely inspecting bytes.
                {
                    let player = &mut opponent_session.roster.manager.players[1];
                    player.bytes[0x1403] = 0;
                    player.bytes[0x1df8] = 1;
                    player.bytes[8..12].copy_from_slice(&4u32.to_le_bytes());
                }
                let tick = opponent_session.world.clock.timing[0];
                opponent_session
                    .roster
                    .advance_ai(
                        &mut opponent_session.world.city,
                        1,
                        &image,
                        &mut openmt2001::ai_tick::Controls {
                            normal_tick: tick,
                            day_tick: opponent_session.world.clock.timing[3],
                            event_gate: false,
                        },
                        &mut opponent_session.allocation_next,
                        &opponent_session.hooks.utilities,
                        &opponent_session.population.properties,
                        None,
                        |call, _, _| Err(format!("loaded AI escaped guarded controller: {call:?}")),
                    )
                    .unwrap();
                opponent_session.world.clock.timing[1] =
                    opponent_session.world.clock.timing[1].wrapping_add(1);
                opponent_session
                    .drain_commands(&assets, |_| Ok(()))
                    .unwrap();
                let mut malformed = vec![0u8; 36];
                malformed[32..36].copy_from_slice(&16u32.to_le_bytes());
                let invalid = append(0x4000, &malformed);
                let invalid = SaveFile::parse(&invalid).unwrap();
                assert!(invalid.scheduled().is_err());
                assert_eq!(session.world.clock, original_clock);
                assert_eq!(session.world.ledgers.residents.active_count, original_count);
            }
            println!(
                "{key}: loaded {} residents, {businesses} businesses, hour {}",
                count,
                session.world.clock.hour()
            );
            let mut session = session;
            assert_import_model_policy(&mut session);
        }
    }

    fn assert_import_model_policy(session: &mut Session<'_>) {
        use crate::compatibility::Mode;
        let nodes = session.world.ledgers.residents.nodes.iter()
            .map(|n| (*n.as_bytes(), n.active_byte(), n.auxiliary_reference())).collect::<Vec<_>>();
        let accounts = session.world.ledgers.residents.accounts.iter().map(|a| *a.as_bytes()).collect::<Vec<_>>();
        let auxiliary = session.world.creation.auxiliary.slots().to_vec();
        let bindings = |s: &Session<'_>| s.population.bindings.iter().map(|b|
            (b.resident_reference, b.auxiliary_reference, b.auxiliary_slot, b.model_frame_count)).collect::<Vec<_>>();
        let before_bindings = bindings(session);
        let controllers = session.population.animations.records().to_vec();
        let clock = session.world.clock.clone();
        let rng = |s: &Session<'_>| [s.world.creation.identity_rng, s.world.creation.preference_rng,
            s.world.creation.initialization_rng.constructor, s.world.creation.initialization_rng.movement,
            s.world.creation.initialization_rng.objective, s.script_random_state, s.credit_random_state,
            s.particle_random_state, s.policy.seed];
        let before_rng = rng(session);
        let mut expected = auxiliary.clone();
        let mut expected_bindings = before_bindings.clone();
        let mut changed = Vec::new();
        for node in session.world.ledgers.residents.nodes.iter().filter(|node| node.is_active()) {
            let slot = (node.auxiliary_reference() - 1) as usize;
            let row = usize::from(node.model_variant());
            let model = &session.resident_models[row];
            if auxiliary[slot][..4] != model.reference.to_le_bytes() {
                expected[slot][..4].copy_from_slice(&model.reference.to_le_bytes());
                expected[slot][0x44..0xb8].copy_from_slice(&model.definition);
                let binding = expected_bindings.iter_mut().find(|b| b.1 == node.auxiliary_reference()).unwrap();
                assert_eq!(binding.2, slot);
                binding.3 = session.people.geometry_source(row, false).unwrap().frames as u16;
                changed.push(slot);
            }
        }
        assert!(!changed.is_empty(), "supplied import must exercise a saved/fresh identity mismatch");
        for (mode, retained) in [(Mode::StrictV14, false), (Mode::StrictV14, true), (Mode::Modern, true)] {
            assert_eq!(session.repair_imported_resident_models(mode, retained).unwrap(), 0);
            assert_eq!(session.world.creation.auxiliary.slots(), auxiliary);
            assert_eq!(bindings(session), before_bindings);
        }
        // A late invalid candidate must not install any earlier valid repair.
        let last = *changed.last().unwrap();
        session.world.creation.auxiliary.get_mut(last).unwrap()[0x2c..0x2e].copy_from_slice(&i16::MAX.to_le_bytes());
        let invalid = session.world.creation.auxiliary.slots().to_vec();
        assert!(session.repair_imported_resident_models(Mode::Modern, false).is_err());
        assert_eq!(session.world.creation.auxiliary.slots(), invalid);
        assert_eq!(bindings(session), before_bindings);
        session.world.creation.auxiliary.get_mut(last).unwrap()[0x2c..0x2e].copy_from_slice(&auxiliary[last][0x2c..0x2e]);
        assert_eq!(session.repair_imported_resident_models(Mode::Modern, false).unwrap(), changed.len());
        assert_eq!(session.world.creation.auxiliary.slots(), expected);
        assert_eq!(bindings(session), expected_bindings);
        assert_eq!(session.repair_imported_resident_models(Mode::Modern, false).unwrap(), 0);
        assert_eq!(session.world.creation.auxiliary.slots(), expected);
        assert_eq!(bindings(session), expected_bindings);
        assert_eq!(session.world.ledgers.residents.nodes.iter().map(|n|
            (*n.as_bytes(), n.active_byte(), n.auxiliary_reference())).collect::<Vec<_>>(), nodes);
        assert_eq!(session.world.ledgers.residents.accounts.iter().map(|a| *a.as_bytes()).collect::<Vec<_>>(), accounts);
        assert_eq!(session.population.animations.records(), controllers);
        assert_eq!(session.world.clock, clock);
        assert_eq!(rng(session), before_rng);
        println!("Modern import repaired {} bindings; exact byte policy, strict/runtime exclusion, failure atomicity and idempotence passed", changed.len());
    }

    #[test]
    #[ignore = "requires supplied saves and original headless scheduler assets"]
    fn advances_loaded_saves_to_next_natural_hour() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(advances_loaded_saves_to_next_natural_hour_inner)
            .unwrap()
            .join()
            .unwrap();
    }

    fn advances_loaded_saves_to_next_natural_hour_inner() {
        let read = |name| fs::read(std::env::var(name).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let built = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&built).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let scripts = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent()
            .unwrap()
            .join("scripts");
        let era_styles = super::super::load_era_styles(
            Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
                .parent()
                .unwrap(),
        )
        .unwrap();
        let names = std::array::from_fn(|_| Vec::new());
        for (key, scenario, difficulty, release, buyer) in [
            ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, false, 0),
            ("OPENMT2001_SAVE_TWO", "sandbox", 0, false, 0),
            ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, true, 0),
            ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1, true, 1),
        ] {
            let mut saved = read(key);
            if release {
                let total = SaveFile::parse(&saved).unwrap().clock().unwrap().timing[0];
                let mut event = [0u8; 52];
                event[8..12].copy_from_slice(&total.to_le_bytes());
                event[28..32].copy_from_slice(&0x16u32.to_le_bytes());
                event[32..36].copy_from_slice(&28u32.to_le_bytes());
                event[36..40].copy_from_slice(&total.to_le_bytes());
                event[48..50].copy_from_slice(&3i16.to_le_bytes());
                event[50] = buyer;
                let at = saved.len() - 8;
                let mut record = 0x4000u32.to_le_bytes().to_vec();
                record.extend_from_slice(&52u32.to_le_bytes());
                record.extend_from_slice(&event);
                saved.splice(at..at, record);
            }
            let mut session = Session::load_original(
                Inputs {
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
                    scripts: &scripts,
                    parameters: std::str::from_utf8(&parameters).unwrap(),
                    era_styles: era_styles.clone(),
                    timetable: std::str::from_utf8(&timetable).unwrap(),
                    scenario,
                    difficulty,
                    selected_character: 4,
                    profile_name: b"Saved session",
                    character_names: &names,
                },
                Policy {
                    seed: 0x12345678,
                    auxiliary_prior_bytes: [0; 0x11c],
                },
                &saved,
            )
            .unwrap();
            if release { assert_eq!(session.world.categories[3][5], buyer); }
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let mut policy = super::super::tick::Policy {
                scheduler: openmt2001::scheduler::Controls {
                    iterations: 1,
                    abort: false,
                    alternate_script_active: false,
                    network_enabled: false,
                    network_override: false,
                    deadline: u32::MAX,
                    gate_32: false,
                    gate_32_offset: false,
                    before_six_sound: 0,
                },
                calendar: openmt2001::clock::Controls {
                    years_per_day: 0,
                    suppress_rollover_query: false,
                    rollover_events_enabled: false,
                    autosave_disabled: true,
                },
                hourly: openmt2001::hourly::Controls {
                    suppressed: false,
                    world_mode: 1,
                    before_six_sound: 0,
                    network_enabled: false,
                    network_override: false,
                },
                camera: [0., 20., 0.],
                distance_disabled: false,
                notifications: false,
                effect_model: 0,
                market_share_prior: [0.; 6],
                player_message_prior: [0; 48],
                ai_event_gate: false,
            };
            let before = session.world.clock.timing[0];
            let before_accounts: Vec<_> = session
                .world
                .ledgers
                .residents
                .accounts
                .iter()
                .map(|a| *a.as_bytes())
                .collect();
            let before_residents: Vec<_> = session
                .world
                .ledgers
                .residents
                .nodes
                .iter()
                .take(session.world.ledgers.residents.active_count as usize)
                .map(|r| *r.as_bytes())
                .collect();
            let before_businesses = session.world.ledgers.businesses.clone();
            let before_owners: Vec<_> = session.world.categories.iter().map(|r| r[5]).collect();
            let mut result = Ok(());
            let mut completed = 0;
            let start_hour = session.world.clock.hour();
            for _ in 0..700 {
                result = session.advance_session(&assets, &mut policy, |event, _| match event {
                    super::super::tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                        0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0 | 0x4b5d50,
                    )) => Ok(0),
                    super::super::tick::Event::Script { commands: [], .. } => Ok(0),
                    super::super::tick::Event::ScriptCommands([]) => Ok(0),
                    super::super::tick::Event::TransportSound(_) => Ok(0),
                    _ => Err(format!("unbound post-load event {event:?}")),
                });
                if result.is_err() {
                    break;
                }
                completed += 1;
                if completed == 25 {
                    assert_eq!(session.world.clock.timing[0], before + 25);
                }
                if completed >= 25 && session.world.clock.hour() != start_hour {
                    break;
                }
            }
            let account_changes = session
                .world
                .ledgers
                .residents
                .accounts
                .iter()
                .zip(&before_accounts)
                .filter(|(a, before)| a.as_bytes() != *before)
                .count();
            let resident_changes = session
                .world
                .ledgers
                .residents
                .nodes
                .iter()
                .zip(&before_residents)
                .filter(|(r, before)| r.as_bytes() != *before)
                .count();
            let business_changes = session
                .world
                .ledgers
                .businesses
                .iter()
                .zip(&before_businesses)
                .filter(|(r, before)| *r != *before)
                .count();
            let owner_changes = session
                .world
                .categories
                .iter()
                .zip(&before_owners)
                .filter(|(r, before)| r[5] != **before)
                .count();
            println!(
                "{key}: {completed} normal ticks, {} -> {}, hour {} -> {}, account changes {account_changes}, resident changes {resident_changes}, business changes {business_changes}, owner changes {owner_changes}, result {result:?}",
                before,
                session.world.clock.timing[0],
                start_hour,
                session.world.clock.hour()
            );
            assert!(result.is_ok(), "{key}: {result:?}");
            assert!(completed >= 25 && session.world.clock.hour() != start_hour);
            assert!(
                resident_changes > 0,
                "normal ticks should mutate retained residents"
            );
            if key == "OPENMT2001_SAVE_ONE" {
                assert!(
                    account_changes > 0,
                    "next hourly boundary should update resident accounts"
                );
                assert!(
                    business_changes > 0,
                    "next hourly boundary should update businesses"
                );
            }
            assert_eq!(
                owner_changes, 0,
                "unrequested property transfer during loaded hour"
            );
        }
    }
    #[test]
    #[ignore = "requires supplied saves and original headless scheduler assets"]
    fn supplied_save_advance_export_reload_continuation() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(supplied_save_advance_export_reload_continuation_inner)
            .unwrap()
            .join()
            .unwrap();
    }

    fn supplied_save_advance_export_reload_continuation_inner() {
        let read = |name| fs::read(std::env::var(name).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let built = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&built).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let scripts = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent()
            .unwrap()
            .join("scripts");
        let era_styles = super::super::load_era_styles(
            Path::new(&std::env::var("OPENMT2001_PARAMETERS").unwrap())
                .parent()
                .unwrap(),
        )
        .unwrap();
        let names = std::array::from_fn(|_| Vec::new());
        let (key, scenario, difficulty) = ("OPENMT2001_SAVE_ONE", "SCENARIO3", 1);
        {
            let saved = read(key);
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
                    scripts: &scripts,
                    parameters: std::str::from_utf8(&parameters).unwrap(),
                    era_styles: era_styles.clone(),
                    timetable: std::str::from_utf8(&timetable).unwrap(),
                    scenario,
                    difficulty,
                    selected_character: 4,
                    profile_name: b"Saved session",
                    character_names: &names,
                };
            let load_policy = || Policy {
                    seed: 0x12345678,
                    auxiliary_prior_bytes: [0; 0x11c],
                };
            let mut session = Session::load_original(inputs(), load_policy(), &saved).unwrap();
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            let tick_policy = || super::super::tick::Policy {
                scheduler: openmt2001::scheduler::Controls {
                    iterations: 1,
                    abort: false,
                    alternate_script_active: false,
                    network_enabled: false,
                    network_override: false,
                    deadline: u32::MAX,
                    gate_32: false,
                    gate_32_offset: false,
                    before_six_sound: 0,
                },
                calendar: openmt2001::clock::Controls {
                    years_per_day: 0,
                    suppress_rollover_query: false,
                    rollover_events_enabled: false,
                    autosave_disabled: true,
                },
                hourly: openmt2001::hourly::Controls {
                    suppressed: false,
                    world_mode: 1,
                    before_six_sound: 0,
                    network_enabled: false,
                    network_override: false,
                },
                camera: [0., 20., 0.],
                distance_disabled: false,
                notifications: false,
                effect_model: 0,
                market_share_prior: [0.; 6],
                player_message_prior: [0; 48],
                ai_event_gate: false,
            };
            let mut policy = tick_policy();
            let before = session.world.clock.timing[0];
            let before_accounts: Vec<_> = session
                .world
                .ledgers
                .residents
                .accounts
                .iter()
                .map(|a| *a.as_bytes())
                .collect();
            let before_residents: Vec<_> = session
                .world
                .ledgers
                .residents
                .nodes
                .iter()
                .take(session.world.ledgers.residents.active_count as usize)
                .map(|r| *r.as_bytes())
                .collect();
            let before_businesses = session.world.ledgers.businesses.clone();
            let before_owners: Vec<_> = session.world.categories.iter().map(|r| r[5]).collect();
            let mut result = Ok(());
            let mut completed = 0;
            let start_hour = session.world.clock.hour();
            for _ in 0..700 {
                result = session.advance_session(&assets, &mut policy, |event, _| match event {
                    super::super::tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                        0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0 | 0x4b5d50,
                    )) => Ok(0),
                    super::super::tick::Event::Script { commands: [], .. } => Ok(0),
                    super::super::tick::Event::ScriptCommands([]) => Ok(0),
                    super::super::tick::Event::TransportSound(_) => Ok(0),
                    _ => Err(format!("unbound post-load event {event:?}")),
                });
                if result.is_err() {
                    break;
                }
                completed += 1;
                if completed == 25 {
                    assert_eq!(session.world.clock.timing[0], before + 25);
                }
                if completed >= 25 && session.world.clock.hour() != start_hour {
                    break;
                }
            }
            let account_changes = session
                .world
                .ledgers
                .residents
                .accounts
                .iter()
                .zip(&before_accounts)
                .filter(|(a, before)| a.as_bytes() != *before)
                .count();
            let resident_changes = session
                .world
                .ledgers
                .residents
                .nodes
                .iter()
                .zip(&before_residents)
                .filter(|(r, before)| r.as_bytes() != *before)
                .count();
            let business_changes = session
                .world
                .ledgers
                .businesses
                .iter()
                .zip(&before_businesses)
                .filter(|(r, before)| *r != *before)
                .count();
            let owner_changes = session
                .world
                .categories
                .iter()
                .zip(&before_owners)
                .filter(|(r, before)| r[5] != **before)
                .count();
            println!(
                "{key}: {completed} normal ticks, {} -> {}, hour {} -> {}, account changes {account_changes}, resident changes {resident_changes}, business changes {business_changes}, owner changes {owner_changes}, result {result:?}",
                before,
                session.world.clock.timing[0],
                start_hour,
                session.world.clock.hour()
            );
            assert!(result.is_ok(), "{key}: {result:?}");
            assert!(completed >= 25 && session.world.clock.hour() != start_hour);
            assert!(
                resident_changes > 0,
                "normal ticks should mutate retained residents"
            );
            if key == "OPENMT2001_SAVE_ONE" {
                assert!(
                    account_changes > 0,
                    "next hourly boundary should update resident accounts"
                );
                assert!(
                    business_changes > 0,
                    "next hourly boundary should update businesses"
                );
            }
            assert_eq!(
                owner_changes, 0,
                "unrequested property transfer during loaded hour"
            );

            // Same explicit bounded host abstractions as the natural-hour test:
            // named scheduler helpers, empty script dispatch and transport audio.
            // Every other event fails; no new gameplay callback is stubbed.
            let camera = openmt2001::flyby::CameraSample {
                eye: [100.,120.,100.], target: [0.,0.,0.], up: [0.,1.,0.], vertical_fov: 0.7,
            };
            let cameras = crate::save_menu::cameras(None, camera).unwrap();
            let bytes = session.save_v2(&image, cameras, 1).expect("supplied-save modern export");
            let parent = std::env::var_os("OPENMT2001_TEST_SCRATCH")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("work/test-scratch"));
            fs::create_dir_all(&parent).unwrap();
            let root = parent.join(format!("supplied-save-{}-{}", std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            // Exclusive creation establishes ownership before installing cleanup.
            fs::create_dir(&root).unwrap();
            struct Scratch(std::path::PathBuf);
            impl Drop for Scratch {
                fn drop(&mut self) {
                    if let Err(error) = fs::remove_dir_all(&self.0) {
                        eprintln!("scratch cleanup {}: {error}", self.0.display());
                    }
                }
            }
            let _scratch = Scratch(root.clone());
            let path = crate::save_menu::write_new(&root, &bytes).unwrap();
            let loaded = fs::read(&path).unwrap();
            let save = SaveFile::parse(&loaded).unwrap();
            let mut resumed = Session::load_original(inputs(), load_policy(), &loaded).unwrap();
            assert!(resumed.restore_v2_extension_with_image(&save, &image).unwrap().is_some());
            assert_eq!(resumed.saved_cameras, Some(cameras));
            let mut resumed_policy = tick_policy();
            for step in 0..=30 {
                supplied_save_state_equal(&mut session, &mut resumed, &image, cameras);
                if step == 30 { break; }
                for (branch, controls) in [(&mut session, &mut policy), (&mut resumed, &mut resumed_policy)] {
                    branch.advance_session(&assets, controls, |event, _| match event {
                        super::super::tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                            0x4805b0 | 0x450080 | 0x4eb5b0 | 0x4b5db0 | 0x4b5d50,
                        )) => Ok(0),
                        super::super::tick::Event::Script { commands: [], .. } => Ok(0),
                        super::super::tick::Event::ScriptCommands([]) => Ok(0),
                        super::super::tick::Event::TransportSound(_) => Ok(0),
                        _ => Err(format!("unbound supplied-save continuation event {event:?}")),
                    }).unwrap();
                }
            }
            println!("supplied SAVE_ONE: export tick {}, final tick {}, 30 identical continuation ticks; artifact {}",
                save.clock().unwrap().timing[0], session.world.clock.timing[0], path.display());
        }
    }

    fn supplied_save_state_equal(a: &mut Session<'_>, b: &mut Session<'_>, image: &openmt2001::pe::PeImage<'_>, cameras: [[u8;196];2]) {
        assert_eq!(a.world.clock, b.world.clock);
        assert_eq!(a.world.ledgers.businesses, b.world.ledgers.businesses);
        assert_eq!(a.world.buildings, b.world.buildings);
        assert_eq!(a.world.categories, b.world.categories);
        assert_eq!(a.world.pending_world.to_bytes(), b.world.pending_world.to_bytes());
        assert_eq!(a.world.objective.to_bytes(), b.world.objective.to_bytes());
        assert_eq!(a.history.as_bytes(), b.history.as_bytes());
        assert_eq!(a.world.ledgers.residents.active_count, b.world.ledgers.residents.active_count);
        assert_eq!(a.world.ledgers.residents.nodes.len(), b.world.ledgers.residents.nodes.len());
        for (x,y) in a.world.ledgers.residents.nodes.iter().zip(&b.world.ledgers.residents.nodes) {
            assert_eq!(x.as_bytes(), y.as_bytes());
        }
        assert_eq!(a.world.ledgers.residents.accounts.len(), b.world.ledgers.residents.accounts.len());
        for (x,y) in a.world.ledgers.residents.accounts.iter().zip(&b.world.ledgers.residents.accounts) {
            assert_eq!(x.as_bytes(), y.as_bytes());
        }
        for (x,y) in a.world.ledgers.players.iter().zip(&b.world.ledgers.players) {
            let mut left=[0;0x1bc]; let mut right=[0;0x1bc];
            x.write_runtime(&mut left).unwrap(); y.write_runtime(&mut right).unwrap();
            assert_eq!(left,right);
        }
        assert_eq!(a.roster.profiles,b.roster.profiles);
        let queue = |s: &mut Session<'_>| {
            let bytes = s.save_v2(image, cameras, 1).unwrap();
            let save = SaveFile::parse(&bytes).unwrap();
            openmt2001::save::extension::Extension::parse(&save).unwrap().unwrap().required(9).unwrap().to_vec()
        };
        assert_eq!(queue(a),queue(b), "delayed command domains/deadlines/payloads");
        let rng = |s: &Session<'_>| (s.random_registry.process,s.random_registry.head,s.random_registry.tail,
            s.script_random_state,s.credit_random_state,s.particle_random_state,s.build_random_state,
            s.world.creation.initialization_rng.constructor,s.world.creation.initialization_rng.movement,
            s.world.creation.initialization_rng.objective,s.world.creation.identity_rng,s.world.creation.preference_rng);
        assert_eq!(rng(a),rng(b));
        assert_eq!(a.road_traffic.admission.random,b.road_traffic.admission.random);
        assert_eq!(a.demolition_effects.random,b.demolition_effects.random);
    }

}

#[cfg(test)]
mod transfer_stream_test {
    include!("session_restore_transfer_test.rs");
}

#[cfg(test)]
mod resident_reset_test {
    include!("session_restore_resident_reset_test.rs");
}

#[cfg(test)]
mod sale_stream_test { include!("session_restore_sale_test.rs"); }
