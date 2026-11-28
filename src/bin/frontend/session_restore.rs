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
        save.sections
            .iter()
            .find(|section| section.tag == tag)
            .is_some_and(|section| section.offset < queue_offset)
    }) && save
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
/// Transfer helpers consume the player/account, business, linked building and
/// category pools synchronously. Catalog-dependent AI callbacks are excluded
/// by the bounded human-only consumer below, so original singleton tails commute.
fn immediate_transfer_after_required_records(save: &SaveFile<'_>, queue_offset: usize) -> bool {
    immediate_terms_after_business_pool(save, queue_offset)
        && [4, 0x400, 0x1000].into_iter().all(|tag| {
            save.sections
                .iter()
                .find(|section| section.tag == tag)
                .is_some_and(|section| section.offset < queue_offset)
        })
        && save
            .sections
            .iter()
            .filter(|section| section.tag == 0x40)
            .all(|section| section.offset < queue_offset)
}
fn immediate_sale_after_required_records(save: &SaveFile<'_>, queue_offset: usize) -> bool {
    immediate_transfer_after_required_records(save, queue_offset)
        && [0x200, 0x800].into_iter().all(|tag| {
            save.sections
                .iter()
                .find(|section| section.tag == tag)
                .is_some_and(|section| section.offset < queue_offset)
        })
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
        || model.renderer_candidates.len() != expected_count
    {
        return false;
    }
    let mut active = 0;
    for template in &model.renderer_templates {
        let state = u32::from_le_bytes(template.row[..4].try_into().unwrap());
        if state != 1 {
            continue;
        }
        active += 1;
        if !matches!(
            template.descriptor,
            0x0101_0201 | 0x0102_0201 | 0x0104_0201 | 0x0104_0202 | 0x0108_0202 | 0x0200_0001
        ) {
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
    creation.auxiliary.reset_for_residents();
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
        0x1a if packet.len() == 32 => Ok(Command::EventCard(
            openmt2001::event_cards::lifecycle::Command {
                card: word(20)?,
                player: word(24)?,
                target: word(28)? as i32,
            },
        )),
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
    ///Exact4bb0d0 source helper, when the containing load owns a synchronous
    ///UI-mode callback. The legacy bulk loader below explicitly lacks this
    ///join; it is not admitted by saved mode or auction-presence inference.
    pub(crate) fn restore_source_timing400<F: FnMut(&mut Self, i32) -> Result<(), String>>(
        &mut self,
        payload: &[u8],
        process: &crate::live_host::process_runtime::Handle,
        change_mode: F,
    ) -> Result<(), String> {
        struct Loan<'a, 'data, F> {
            session: &'a mut Session<'data>,
            change: F,
        }
        let result = crate::intro::camera_live::process_lifetime::restore_timing400(
            payload,
            &mut Loan {
                session: self,
                change: change_mode,
            },
            |loan, index, value| {
                loan.session.world.clock.timing[index] = value;
                Ok(())
            },
            |loan| loan.session.roster.status.current_player != 0,
            |loan, mode| (loan.change)(loan.session, mode),
        );
        match result {
            Ok(()) => process.source_timing_completed().map_err(|e| e.to_string()),
            Err(error) => {
                process
                    .source_timing_mode_unjoined()
                    .map_err(|e| e.to_string())?;
                Err(error)
            }
        }
    }

    /// The 45110e command11 branch for one isolated due-now 0x4000 record.
    /// This bounded case uses the original zero-detail model1744, so
    /// 4450a0 -> 485520 returns without renderer replacement. Other models
    /// still need a synchronous original appearance registry.
    fn restore_immediate_open(
        &mut self,
        command: openmt2001::business_open::Command,
        session_option_word: u32,
        canonical_queue_position: bool,
        scripted_catalog_matches_saved: bool,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        use openmt2001::{
            business_binding::Call,
            live_commands::{self, Event},
        };
        let building = command.building as usize;
        let row = self
            .world
            .buildings
            .get(building)
            .ok_or("saved immediate open building leaves pool")?;
        let linked = u32::from_le_bytes(row[0x1c..0x20].try_into().unwrap());
        let model = self
            .models
            .appearance_candidates
            .get(&building)
            .ok_or("saved immediate open lacks prepared model identity")?;
        let category = &self.world.categories[13];
        let building_reference = 0x591318 + (building * 916) as u32;
        let category_word =
            |at: usize| u32::from_le_bytes(category[at..at + 4].try_into().unwrap());
        if canonical_queue_position && !scripted_catalog_matches_saved {
            return Err(
                "saved immediate open pre-queue BusinessInfo differs from restored row42".into(),
            );
        }
        if session_option_word != 0
            || command.kind != 42
            || command.owner != 1
            || command.stock_argument != 1
            || row[0x3f] == 0
            || linked != u32::MAX
            || row[4..9] != [0, 0, 6, 0, 4]
            || u32::from_le_bytes(row[0x24..0x28].try_into().unwrap()) != 1
            || row[0x2c] != 2
            || u32::from_le_bytes(row[0x40..0x44].try_into().unwrap()) != 0x617628 + 13 * 380
            || model.descriptor != 1744
            || !model.renderer_candidates.is_empty()
            || !model.renderer_templates.is_empty()
            || category_word(8) != 1
            || category_word(0x15c) != building_reference
            || category_word(0x160) != building_reference
            || u32::from_le_bytes(row[0x44..0x48].try_into().unwrap()) != 0
            || self
                .world
                .catalog
                .info
                .row(42)
                .is_none_or(|row| row[0x4c] != 0)
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
            utility_factor: self.world.settings.selected_income().multiplier,
        };
        let mut messages = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena {
                next: self.allocation_next,
            },
            current_player: self.roster.status.current_player,
            network: [false, false],
            prior: [0; 48],
        };
        let mut appearance_refreshes = 0;
        let result = live_commands::open(
            &command,
            &mut self.world.city,
            &mut self.roster,
            &mut self.population.business_counts,
            &mut self.hooks.utilities,
            &mut messages,
            &context,
            |event| match event {
                Event::Business(Call::RefreshBuilding {
                    building: refreshed,
                }) if refreshed == building => {
                    appearance_refreshes += 1;
                    Ok(())
                }
                Event::RefreshInterface => Ok(()),
                other => Err(format!(
                    "saved immediate open callback {other:?} has no load-stage owner"
                )),
            },
        );
        self.allocation_next = messages.arena.next;
        result?;
        if appearance_refreshes != 1 {
            return Err("saved immediate open missed synchronous 4450a0 appearance refresh".into());
        }
        restore::activate_zero_model_appearance(
            building,
            restore::AppearanceAlias::Business(u32::from_le_bytes(
                self.world.buildings[building][0x1c..0x20]
                    .try_into()
                    .unwrap(),
            ) as usize),
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
        for resident in self
            .world
            .ledgers
            .residents
            .nodes
            .iter()
            .filter(|node| node.is_active())
        {
            if resident.age() > 4 {
                return Err("imported resident age leaves model registry".into());
            }
            let row = usize::from(resident.model_variant());
            let model = &self.resident_models[row];
            let reference = resident.auxiliary_reference();
            let slot = reference
                .checked_sub(1)
                .ok_or("imported resident has no auxiliary")? as usize;
            if slot >= self.world.creation.auxiliary.used() {
                return Err("imported resident auxiliary is not allocated".into());
            }
            let auxiliary = &self.world.creation.auxiliary.slots()[slot];
            let actual = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
            if actual == model.reference {
                continue;
            }
            let binding_index = self
                .population
                .bindings
                .iter()
                .position(|binding| binding.auxiliary_reference == reference)
                .ok_or("imported resident has no animation binding")?;
            if self.population.bindings[binding_index].auxiliary_slot != slot {
                return Err("imported resident animation binding selects another auxiliary".into());
            }
            if i16::from_le_bytes(auxiliary[0x2e..0x30].try_into().unwrap()) != -1 {
                return Err("imported resident model repair requires an idle load boundary".into());
            }
            let frame = i16::from_le_bytes(auxiliary[0x2c..0x2e].try_into().unwrap());
            let geometry = self
                .people
                .geometry_source(row, false)
                .ok_or("imported resident geometry missing")?;
            for lod in [false, true] {
                let source = self
                    .people
                    .geometry_source(row, lod)
                    .ok_or("imported resident LOD missing")?;
                if usize::try_from(frame).map_or(true, |frame| frame >= source.frames) {
                    return Err(
                        "imported resident frame cannot be retained by identity model".into(),
                    );
                }
            }
            let frames = u16::try_from(geometry.frames)
                .map_err(|_| "imported resident frame count overflow")?;
            repairs.push((slot, binding_index, row, frames));
        }
        // Validate every repair before changing any retained state.
        for &(slot, binding, row, frames) in &repairs {
            let model = &self.resident_models[row];
            openmt2001::resident::bind_auxiliary_model(
                self.world.creation.auxiliary.get_mut(slot).unwrap(),
                model.reference,
                &model.definition,
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
        Self::load_original_with_catalog(input, policy, saved_bytes, None, None, None, None, None)
    }

    /// Presentation-aware loading returns original synchronous UI calls for
    /// one-shot delivery after the Host has attached its scene/audio channels.
    /// The caller must own every returned call before advancing the Session.
    pub fn load_original_with_presentation(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
    ) -> Result<(Self, Vec<openmt2001::economy::AccountPosting>), String> {
        Self::load_original_with_presentation_style(input, policy, saved_bytes, None)
    }

    pub fn load_original_with_presentation_style(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
        style: Option<&openmt2001::board_style::Style>,
    ) -> Result<(Self, Vec<openmt2001::economy::AccountPosting>), String> {
        let mut calls = Vec::new();
        let session = Self::load_original_with_catalog(
            input,
            policy,
            saved_bytes,
            None,
            None,
            Some(&mut calls),
            style,
            None,
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
            None,
        )
    }

    pub fn load_original_with_presentation_style_in_process(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
        style: Option<&openmt2001::board_style::Style>,
        process: Option<crate::live_host::process_runtime::Handle>,
    ) -> Result<(Self, Vec<openmt2001::economy::AccountPosting>), String> {
        let mut calls = Vec::new();
        let session = Self::load_original_with_catalog(
            input,
            policy,
            saved_bytes,
            None,
            None,
            Some(&mut calls),
            style,
            process,
        )?;
        Ok((session, calls))
    }

    fn load_original_with_catalog(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
        catalog: Option<openmt2001::level_catalog::Catalog>,
        profile_path: Option<std::path::PathBuf>,
        presentation: Option<&mut Vec<openmt2001::economy::AccountPosting>>,
        style: Option<&openmt2001::board_style::Style>,
        process: Option<crate::live_host::process_runtime::Handle>,
    ) -> Result<Self, String> {
        Self::load_original_joined(
            input,
            policy,
            saved_bytes,
            catalog,
            profile_path,
            presentation,
            style,
            process,
            None,
        )
    }
    pub(crate) fn load_original_joined(
        input: Inputs<'_, 'data>,
        policy: Policy,
        saved_bytes: &[u8],
        catalog: Option<openmt2001::level_catalog::Catalog>,
        profile_path: Option<std::path::PathBuf>,
        presentation: Option<&mut Vec<openmt2001::economy::AccountPosting>>,
        style: Option<&openmt2001::board_style::Style>,
        process: Option<crate::live_host::process_runtime::Handle>,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<Self, String> {
        let save = SaveFile::parse(saved_bytes)?;
        save.validate_compatibility(input.image)?;
        let metadata = save.level_metadata()?;
        if metadata[3] as i32 != input.difficulty {
            return Err("saved difficulty differs from selected level input".into());
        }
        let mut catalog = catalog;
        let mut restored_map = None;
        if let Some(ext) = openmt2001::save::extension::Extension::parse(&save)? {
            ext.validate_runtime_section_set()?;
            if matches!(ext.version, 5 | 6) {
                let selection =
                    crate::skirmish_maps::Selection::decode(ext.version, ext.required(18)?)?;
                if selection.level_id != metadata[0] {
                    return Err("Skirmish map selection differs from native level metadata".into());
                }
                let retained = if let Some(catalog) = catalog.as_mut() {
                    catalog
                } else {
                    catalog = Some(crate::campaign_store::load_catalog(
                        input.image,
                        input.scripts,
                    )?);
                    catalog.as_mut().unwrap()
                };
                let index = retained
                    .find(selection.level_id)
                    .ok_or("saved Skirmish missing from catalog")?;
                let level = &mut retained.levels[index];
                if level.single_player()
                    || !level.directory().eq_ignore_ascii_case(
                        format!("net{}", selection.level_id - 300000).as_bytes(),
                    )
                {
                    return Err(
                        "saved Skirmish directory differs from native level metadata".into(),
                    );
                }
                level.set_objective_target(selection.target);
                restored_map = Some(selection.map);
            }
        }
        let assets = CommandAssets {
            image: input.image,
            board: input.board,
            file: input.file,
        };
        let mut session = Self::initialize_joined(
            input,
            policy,
            catalog,
            profile_path,
            true,
            style,
            crate::compatibility::Mode::StrictV14,
            None,
            process,
            dispatch,
        )?;
        if session.catalog.selected_level().map(|level| level.id()) != Some(metadata[0]) {
            return Err("saved level differs from selected original level".into());
        }
        if let Some(map) = restored_map {
            session.skirmish_map = Some(map);
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
        &mut self,
        category: i16,
        owner: u8,
        amount: i32,
        assets: &CommandAssets<'_, '_>,
        mut presentation: Option<&mut Vec<openmt2001::economy::AccountPosting>>,
    ) -> Result<(), String> {
        use openmt2001::live_commands::{self, Event};
        let word =
            |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let self_buyer = self
            .roster
            .manager
            .players
            .iter()
            .find(|player| player.reference == self.roster.status.current_player)
            .map(|player| player.bytes[0x1bc].wrapping_add(1));
        if (owner != 0 && Some(owner) != self_buyer)
            || (amount != 0 && (owner == 0 || presentation.is_none()))
            || !(0..52).contains(&category)
            || self
                .roster
                .manager
                .players
                .iter()
                .any(|player| word(&player.bytes, 0x154) != 0)
            || assets.image.f32_at(0x525040)? != 0.
        {
            return Err("saved immediate property release requires native zero-price City/self-buyer human-only branch or priced self-buyer presentation owner".into());
        }
        let chains = openmt2001::building_pool::category_chains(
            &self.world.categories,
            &self.world.buildings,
            0x591318,
        )?;
        if chains[category as usize].iter().any(|&building| {
            let row = &self.world.buildings[building];
            word(row, 0x4c) == 1
                && (word(row, 0x1c) == u32::MAX || matches!(word(row, 0x24), 0 | 1))
        }) {
            return Err(
                "saved immediate property release needs synchronous building appearance owner"
                    .into(),
            );
        }
        let clock = self.world.clock;
        let context = live_commands::Context {
            image: assets.image,
            clock: &clock,
            accounting_enabled: false,
            notifications: false,
            tutorial_state: 0,
            network_enabled: false,
            network_override: false,
            utility_factor: 0.,
        };
        let mut messages = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena {
                next: self.allocation_next,
            },
            current_player: self.roster.status.current_player,
            network: [false, false],
            prior: [0; 48],
        };
        live_commands::property_transfer(
            category,
            owner,
            amount,
            &mut self.world.city,
            &mut self.roster,
            &mut messages,
            &context,
            |event| match event {
                Event::RefreshInterface | Event::PropertyView(_) => Ok(()),
                Event::AccountPosted(posting) => {
                    if posting.amount != 0.
                        && self_buyer.map(|owner| usize::from(owner) - 1) == Some(posting.player)
                    {
                        presentation
                            .as_mut()
                            .ok_or("saved immediate property cash cue lacks presentation owner")?
                            .push(posting);
                    }
                    Ok(())
                }
                // Actual 4504e0 -> 420500 returns before inserting a notice
                // when owner-1 equals current slot (420509..42051b).
                Event::PropertyTransferNotice {
                    owner: notice_owner,
                    ..
                } if owner != 0 && Some(notice_owner) == self_buyer => Ok(()),
                other => Err(format!(
                    "saved immediate property release callback {other:?} has no load-stage owner"
                )),
            },
        )?;
        self.allocation_next = messages.arena.next;
        self.hooks.utilities.owners = [2, 35, 39, 42].map(|i| self.world.categories[i][5]);
        Ok(())
    }

    fn restore_immediate_sale(
        &mut self,
        command: openmt2001::business_sale::Command,
        assets: &CommandAssets<'_, '_>,
        calls: &mut Vec<openmt2001::economy::AccountPosting>,
    ) -> Result<(), String> {
        use openmt2001::business_sale::{self, Call};
        let business = self
            .world
            .ledgers
            .businesses
            .get(command.business as usize)
            .ok_or("saved immediate sale business leaves pool")?;
        if business[0x21c] == 0 || self.building_selection != [0; 2] {
            return Err(
                "saved immediate sale requires active business and unselected property panel"
                    .into(),
            );
        }
        let building = i16::from_le_bytes(business[8..10].try_into().unwrap()) as usize;
        let row = self
            .world
            .buildings
            .get(building)
            .ok_or("saved immediate sale building leaves pool")?;
        let word = |at| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
        if row[0x3f] == 0 || word(0x1c) != command.business {
            return Err(
                "saved immediate sale requires matching live building/business links".into(),
            );
        }
        let appearance = word(0x4c) == 1 && (word(0x1c) == u32::MAX || matches!(word(0x24), 0 | 1));
        let mut positive_appearance = false;
        if appearance {
            let model = self
                .models
                .appearance_candidates
                .get(&building)
                .ok_or("saved immediate sale lacks prepared appearance model")?;
            let zero = model.descriptor == 1744
                && model.renderer_candidates.is_empty()
                && model.renderer_templates.is_empty();
            positive_appearance = checked_positive_sale_appearance(model, row[8]);
            if !zero && !positive_appearance {
                return Err(
                    "saved immediate sale appearance has unverified renderer identity".into(),
                );
            }
        }
        let current_player = self
            .roster
            .manager
            .players
            .iter()
            .position(|player| player.reference == self.roster.status.current_player);
        let controls = business_sale::Controls {
            current_player,
            accounting: false,
            ownership_tick: self.world.clock.timing[1],
            timestamp: self.world.clock.timing[0],
            hour: self.world.clock.hour() as i32,
        };
        let mut appearance_count = 0;
        let city = &mut self.world.city;
        business_sale::apply(
            &command,
            &mut business_sale::World {
                businesses: &mut city.ledgers.businesses,
                buildings: &mut city.buildings,
                players: &mut city.ledgers.players,
                catalog: &city.catalog.info,
                products: &city.catalog.products,
                commodities: &city.commodities.rows,
                age: &mut self.population.business_counts.age,
            },
            &controls,
            assets.image,
            |call, world| match call {
                Call::AccountPosted(posting) => {
                    if posting.amount != 0. && current_player == Some(posting.player) {
                        calls.push(posting);
                    }
                    Ok(())
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
                        restore::activate_zero_model_appearance(
                            building,
                            restore::AppearanceAlias::Business(command.business as usize),
                            &world.buildings[building],
                        )
                    }
                }
                Call::RefreshInterface => Ok(()),
                // Original407660 clears +3e but returns for an unselected object.
                Call::BuildingPanel(id) if id == building => Ok(()),
                other => Err(format!(
                    "saved immediate sale callback {other:?} lacks synchronous owner"
                )),
            },
        )?;
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
        &mut self,
        save: &SaveFile<'_>,
        assets: &CommandAssets<'_, '_>,
        mut presentation: Option<&mut Vec<openmt2001::economy::AccountPosting>>,
    ) -> Result<(), String> {
        if self.saved_logic_mode.is_some() {
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
        let scripted_catalog_matches_saved =
            self.world.catalog.info.row(42) == Some(&save.business_catalog()?[42]);
        let scheduled = save.scheduled()?;
        let modern_presentation = crate::session_save::verified_modern_presentation_queue(save)?;
        let last_non_queue_offset = save
            .sections
            .iter()
            .filter(|section| section.tag != 0x4000)
            .map(|section| section.offset)
            .max()
            .unwrap_or(0);
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
                if modern_presentation[queue_index] && admission_due <= saved_clock.timing[0] {
                    continue;
                }
                if event
                    .packet
                    .is_some_and(|packet| packet[4..8] == 0x13u32.to_le_bytes())
                    && (admission_due == 0 || admission_due == saved_clock.timing[0])
                {
                    if save
                        .sections
                        .iter()
                        .any(|s| s.tag == openmt2001::save::extension::TAG)
                    {
                        return Err("V2 immediate sale requires authoritative extension queue continuation, not original load-stage presentation".into());
                    }
                    if presentation.is_none()
                        || scheduled.len() != 1
                        || !immediate_sale_after_required_records(save, section.offset)
                    {
                        return Err("saved immediate sale needs isolated original record order and presentation owner".into());
                    }
                    let openmt2001::live_commands::Command::SellBusiness(command) =
                        saved_command(event.packet.unwrap())?
                    else {
                        unreachable!()
                    };
                    // Native queue replay precedes saved catalogs. Check only
                    // rows the original stock initializer actually consumes.
                    let economy = save.economy()?;
                    let business = economy
                        .businesses()
                        .nth(command.business as usize)
                        .ok_or("saved immediate sale business leaves pool")?;
                    let kind = business.business_type() as usize;
                    let saved_info = save.business_catalog()?;
                    let info = saved_info
                        .get(kind)
                        .ok_or("saved immediate sale catalog kind leaves pool")?;
                    if self.world.catalog.info.row(kind) != Some(info) {
                        return Err("saved immediate sale pre-queue BusinessInfo differs from saved trailing row".into());
                    }
                    let count = if business.discriminator() == 0 {
                        usize::from(info[0x4c])
                    } else {
                        0
                    };
                    let products = self
                        .world
                        .catalog
                        .products
                        .get(kind)
                        .and_then(|rows| rows.get(..count))
                        .ok_or("saved immediate sale product count exceeds six")?;
                    let saved_commodities = save.commodity_rows()?;
                    for product in products {
                        let slot = product[0] as usize;
                        if self.world.commodities.rows.get(slot) != saved_commodities.get(slot)
                            || saved_commodities.get(slot).is_none()
                        {
                            return Err("saved immediate sale pre-queue commodity differs from saved trailing row".into());
                        }
                    }
                    immediate_sale = Some(command);
                    continue;
                }
                if event
                    .packet
                    .is_some_and(|packet| packet[4..8] == 0x11u32.to_le_bytes())
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
                        return Err(
                            "saved immediate open needs isolated original queue order".into()
                        );
                    }
                    let openmt2001::live_commands::Command::Open(command) =
                        saved_command(event.packet.unwrap())?
                    else {
                        unreachable!()
                    };
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
                        return Err(
                            "saved immediate terms require original business-before-queue order"
                                .into(),
                        );
                    }
                    let openmt2001::live_commands::Command::Terms(terms) =
                        saved_command(event.packet.unwrap())?
                    else {
                        unreachable!()
                    };
                    immediate_terms.push(terms);
                    continue;
                }
                let wait = event
                    .packet
                    .is_some_and(|packet| packet[4..8] == 0x19u32.to_le_bytes())
                    && (admission_due == 0 || admission_due == saved_clock.timing[0]);
                if wait {
                    if !immediate_wait_after_required_records(save, section.offset) {
                        return Err("saved immediate category wait requires category and clock before queue".into());
                    }
                } else if event.packet.is_some_and(|packet| {
                    packet[4..8] == 0x16u32.to_le_bytes()
                        && packet.len() == 28
                        && (packet[20..24] == [0; 4]
                            || (presentation.is_some()
                                && packet[26] != 0
                                && !save.sections.iter().any(|section| {
                                    section.tag == openmt2001::save::extension::TAG
                                })))
                }) && (admission_due == 0 || admission_due == saved_clock.timing[0])
                {
                    if scheduled.len() != 1
                        || !immediate_transfer_after_required_records(save, section.offset)
                    {
                        return Err("saved immediate property release requires original pools-before-queue order".into());
                    }
                    if event.packet.is_some_and(|packet| packet[20..24] != [0; 4])
                        && (save.auction()?.is_some() || saved_clock.timing[5] == 2)
                    {
                        return Err("priced saved self-transfer requires normal original load without auction activation".into());
                    }
                } else {
                    // Other packets still require a synchronous native owner.
                    return Err(
                        "saved immediate packet needs native 4810e0/450a30 synchronous dispatch"
                            .into(),
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
        let session_option_word = save.session_option_word()?;
        let game_mode = saved_clock.timing[5];

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
                    let offset = reference
                        .checked_sub(0x591318)
                        .ok_or("tree restore owner precedes building pool")?;
                    if !offset.is_multiple_of(916) {
                        return Err("tree restore owner is not record-aligned".into());
                    }
                    let slot = (offset / 916) as usize;
                    if slot >= 600 || visited[slot] {
                        return Err("tree restore category list is invalid/cyclic".into());
                    }
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
                assets.file,
                selected,
                saved.height(),
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
                session_option_word,
                canonical_queue_position,
                scripted_catalog_matches_saved,
                assets,
            )?;
        }
        if let Some(command) = immediate_sale {
            self.restore_immediate_sale(
                command,
                assets,
                presentation
                    .take()
                    .ok_or("saved immediate sale lost presentation owner")?,
            )?;
        }
        for (command, domain, queue_due, admission_due, direct, immediate) in restored_commands {
            if immediate {
                match command {
                    openmt2001::live_commands::Command::CategoryReady { category } => {
                        openmt2001::category_ownership::end_wait(
                            self.world
                                .categories
                                .get_mut(category as usize)
                                .ok_or("saved immediate category leaves pool")?,
                        );
                    }
                    openmt2001::live_commands::Command::Auction { descriptor }
                        if descriptor[4..8] == 0x19u32.to_le_bytes() =>
                    {
                        let category = u32::from_le_bytes(descriptor[20..24].try_into().unwrap());
                        let normal = self.world.clock.timing[1];
                        let due = normal.wrapping_add(15000);
                        if (due as i32) <= (normal as i32) {
                            return Err("saved category wait expiry reaches native immediate normal-event fallthrough".into());
                        }
                        openmt2001::category_ownership::begin_wait(
                            self.world
                                .categories
                                .get_mut(category as usize)
                                .ok_or("saved immediate category wait leaves pool")?,
                            normal,
                        );
                        self.live_commands.restore_queued(
                            openmt2001::live_commands::Command::CategoryReady { category },
                            openmt2001::scheduler::ClockDomain::Normal,
                            due,
                            due,
                            true,
                        );
                    }
                    openmt2001::live_commands::Command::PropertyTransfer {
                        category,
                        owner,
                        amount,
                    } => {
                        self.restore_immediate_property_transfer(
                            category,
                            owner,
                            amount,
                            assets,
                            presentation.as_deref_mut(),
                        )?;
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
        self.session_option_word = session_option_word;
        self.saved_logic_mode = Some(game_mode);

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
#[path = "session_restore_tests.rs"]
mod tests;

#[cfg(test)]
mod transfer_stream_test {
    include!("session_restore_transfer_test.rs");
}

#[cfg(test)]
mod resident_reset_test {
    include!("session_restore_resident_reset_test.rs");
}

#[cfg(test)]
mod sale_stream_test {
    include!("session_restore_sale_test.rs");
}
