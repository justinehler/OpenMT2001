//! Retained fresh-city host adapter for level startup, roster, local commands
//! and session dispatch. Unbound gameplay and presentation work stays explicit.
use openmt2001::{
    business_binding, business_pool, economy, level_script, level_startup, lua4::Value, map_script,
};
use std::{fs, path::Path};
#[cfg(test)]
#[path = "fresh_city/ai_resume_probe.rs"]
mod ai_resume_probe;
#[path = "session_cards.rs"]
pub mod cards;
#[path = "custom_session.rs"]
pub mod custom;
#[cfg(test)]
#[path = "fresh_city/objective_tests.rs"]
mod objective_tests;
#[cfg(all(test, feature = "capture"))]
#[path = "outcome_capture.rs"]
mod outcome_capture;
#[path = "session_property.rs"]
pub mod property;
#[path = "session_restore.rs"]
pub mod restore;
#[path = "fresh_city/result.rs"]
pub mod result;
#[path = "session_scenarios.rs"]
pub mod scenarios;
#[path = "session_tick.rs"]
pub mod tick;
#[path = "tutorial_announcement.rs"]
mod tutorial_announcement;

/// Original4a6830 AI names resolve the descriptor's localized string ID.
pub fn character_names(
    image: &openmt2001::pe::PeImage<'_>,
    language: &Path,
) -> Result<[Vec<u8>; 10], String> {
    let offsets = fs::read(language.join("tycoon.off")).map_err(|e| e.to_string())?;
    let text = fs::read(language.join("tycoon.bin")).map_err(|e| e.to_string())?;
    if !offsets.len().is_multiple_of(2) {
        return Err("partial language length".into());
    }
    let mut strings = Vec::new();
    let mut at = 0;
    for bytes in offsets.chunks_exact(2) {
        let end = at + usize::from(u16::from_le_bytes(bytes.try_into().unwrap()));
        strings.push(text.get(at..end).ok_or("language entry outside text")?);
        at = end;
    }
    if at != text.len() {
        return Err("language lengths do not cover text".into());
    }
    let mut names: [Vec<u8>; 10] = std::array::from_fn(|_| Vec::new());
    for (index, name) in names.iter_mut().enumerate() {
        let id = u32::from_le_bytes(
            image
                .at(0x541ae8 + index as u32 * 40, 4)?
                .try_into()
                .unwrap(),
        ) as usize;
        let glyphs = strings.get(id).ok_or("character name outside language")?;
        // Original4e78e0 reverse-searches the first language entry, then adds
        //32 to its first matching index. Glyph0 is a space, not a terminator.
        let dictionary = strings.first().ok_or("language dictionary missing")?;
        *name = glyphs
            .iter()
            .map(|glyph| {
                dictionary
                    .iter()
                    .position(|candidate| candidate == glyph)
                    .map(|index| (index as u8).wrapping_add(32))
                    .ok_or_else(|| "character name contains an unmapped language glyph".to_string())
            })
            .collect::<Result<_, _>>()?;
    }
    Ok(names)
}

/// Host policy is explicit because unspecified original stack bytes and seed
/// selection must not be presented as recovered deterministic defaults.
pub struct Policy {
    pub seed: u32,
    pub auxiliary_prior_bytes: [u8; 0x11c],
}
pub struct Inputs<'a, 'data> {
    /// Native hosts provide original ambient locators before script binding.
    /// Explicitly headless hosts may omit renderer tree state.
    pub tree_locators: Option<Vec<[f32; 3]>>,
    pub image: &'a openmt2001::pe::PeImage<'data>,
    pub board: &'a openmt2001::board::Board,
    pub file: &'a openmt2001::build_file::BuildFile<'data>,
    pub people: openmt2001::people::PeopleModels<'data>,
    pub vehicle_models: openmt2001::traffic::road::models::Models<'data>,
    pub transport_models: openmt2001::traffic::models::Models<'data>,
    pub road_routes: Vec<u8>,
    pub scripts: &'a Path,
    pub parameters: &'a str,
    pub era_styles: [openmt2001::building_styles::Styles; 8],
    pub timetable: &'a str,
    pub scenario: &'a str,
    pub difficulty: i32,
    pub selected_character: usize,
    pub profile_name: &'a [u8],
    pub character_names: &'a [Vec<u8>; 10],
}
///Load the eight original era files; parameter.prm contains no texture ranks.
pub fn load_era_styles(
    directory: &Path,
) -> Result<[openmt2001::building_styles::Styles; 8], String> {
    (1930..=2000)
        .step_by(10)
        .map(|year| {
            let path = directory.join(format!("{year}.prm"));
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            openmt2001::building_styles::Styles::from_parameters(&text)
        })
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "original era style bank size mismatch".into())
}
/// Confirmed setup and full compact participants, separate from ordinary Inputs.
/// Offline local peer0 is an adapter identity, not a recovered native peer ID.
#[derive(Clone)]
pub struct SkirmishLaunch {
    pub level_id: u32,
    pub directory: String,
    pub map: String,
    pub objective_kind: i32,
    pub target: f32,
    pub difficulty: i32,
    pub participants: Vec<openmt2001::skirmish::roster::Participant>,
}
impl SkirmishLaunch {
    fn validate(&self) -> Result<(), String> {
        self.validate_roster(false)
    }
    pub(crate) fn validate_roster(&self, connected: bool) -> Result<(), String> {
        use openmt2001::skirmish::roster::Kind;
        // Bounded original supplied map support; never join a caller path.
        if !(0..=2).contains(&self.difficulty)
            || !self.target.is_finite()
            || !(2..=6).contains(&self.participants.len())
            || self.participants[0].kind != Kind::Human
        {
            return Err("invalid offline Skirmish launch".into());
        }
        if self.directory.is_empty() || !self.directory.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("invalid selected level directory".into());
        }
        let mut used = [false; 10];
        for (slot, p) in self.participants.iter().enumerate() {
            let token = p
                .token
                .filter(|&t| t < 10)
                .ok_or("invalid participant token")?;
            if used[token]
                || !p.ready
                || p.name.is_empty()
                || p.name.len() >= 32
                || p.name.contains(&0)
                || (slot > 0 && p.kind != Kind::Ai && !(connected && p.kind == Kind::Human))
                || !(0..=2).contains(&p.difficulty)
            {
                return Err("invalid compact offline participant".into());
            }
            used[token] = true;
        }
        Ok(())
    }
}

pub struct Session<'data> {
    pub connected: Option<crate::intro::connected_start::Connection>,
    pub building_styles: openmt2001::building_styles::Styles,
    pub era_styles: [openmt2001::building_styles::Styles; 8],
    pub player_style_indices: [i32; 6],
    pub difficulty: usize,
    /// Original level object+85c.48d439 leaves this zero for mode2 normal
    /// scenarios; tutorial modes1/7 set it to one. Retain the byte because
    ///48dbf0 tests nonzero while48dc40 tests equality with one.
    pub tutorial_state: u8,
    /// Original level manager and selected record, from GAMESTART registrations.
    pub catalog: openmt2001::level_catalog::Catalog,
    /// Retained source provenance; cold loads restore materialized state, never replay a map.
    pub skirmish_map: Option<crate::skirmish_maps::MapIdentity>,
    /// V2-owned profile path, attached by the host after profile selection.
    pub campaign_profile: Option<std::path::PathBuf>,
    /// Original68b478=-1; offline startup does not register450410.
    pub network_command_timer: Option<usize>,
    pub setup: Option<crate::singleplayer::Setup>,
    pub world: level_startup::World,
    pub scripts: level_startup::Scripts,
    pub models: crate::building_models::Models,
    pub people: openmt2001::people::PeopleModels<'data>,
    pub resident_models: [economy::ResidentModelBinding; 32],
    pub hooks: Hooks,
    pub policy: Policy,
    pub roster: level_startup::Roster,
    pub random_registry: openmt2001::player_manager::RandomRegistry,
    /// Original 0x93aee8 script Random.ZeroTo stream, cold-seeded because
    /// original saves do not serialize the process RNG state.
    pub script_random_state: u32,
    /// Original 0x8b06e8 result-credit shuffle stream, separate from scripts.
    pub credit_random_state: u32,
    /// Original 0x68c028 particle stream, initialized with other process RNG nodes.
    pub particle_random_state: u32,
    /// Original940cf8 stream and4eb500 dust pool, independent of fireworks.
    pub demolition_effects: openmt2001::demolition_effect::Manager,
    pub demolition_points: Vec<openmt2001::demolition_effect::Point>,
    /// Original builder560a08 stream, independent of the script/process RNG.
    pub build_random_state: u32,
    pub window_texture: openmt2001::window_texture::WindowTexture,
    pub pulse_lights: openmt2001::pulse_lights::PulseLights,
    pub allocation_next: u32,
    pub static_rng_tail_next: u32,
    pub event_cards: openmt2001::event_cards::Deck,
    pub population: openmt2001::level_runtime::Population,
    pub history: openmt2001::player_history::State,
    pub road_traffic: openmt2001::traffic::road::runtime::RoadTraffic,
    pub vehicle_models: openmt2001::traffic::road::models::Models<'data>,
    pub transport_models: openmt2001::traffic::models::Models<'data>,
    pub transport: openmt2001::traffic::scene::Scene,
    pub train_routes: [openmt2001::traffic::RouteGeometry; 8],
    pub road_routes: Vec<u8>,
    pub live_commands: openmt2001::live_commands::Queue,
    pub building_selection: [u32; 2],
    /// Exact original presentation records, retained while headless.
    pub saved_cameras: Option<[[u8; 196]; 2]>,
    pub saved_player_metadata: [[u8; 48]; 6],
    pub saved_game_mode: Option<u32>,
    /// Host script directory retained for the original load-mode AI replay.
    pub script_directory: std::path::PathBuf,
    pub result_presentation: Option<result::ResultPresentation>,
}

pub struct CommandAssets<'a, 'data> {
    pub image: &'a openmt2001::pe::PeImage<'data>,
    pub board: &'a openmt2001::board::Board,
    pub file: &'a openmt2001::build_file::BuildFile<'data>,
}
pub struct BuildInspection {
    pub total: f32,
    pub category_locked: bool,
    pub placement_allowed: bool,
    pub affordable: bool,
    pub preview: Option<openmt2001::scene::SceneBuilding>,
    pub submitted: bool,
    command: openmt2001::construction::Command,
}
/// None tells the frontend to leave planning through its ordinary close path.
pub type BuildReply = Result<Option<BuildInspection>, String>;
impl BuildInspection {
    pub fn allowed(&self) -> bool {
        !self.category_locked && self.placement_allowed && self.affordable && self.preview.is_some()
    }
}
impl<'data> Session<'data> {
    pub fn planning_exits(&self, category: usize) -> Result<bool, String> {
        let row = self
            .world
            .categories
            .get(category)
            .ok_or("build category outside city")?;
        let player = self
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == self.roster.status.current_player)
            .ok_or("build current player missing")?;
        Ok(crate::build_panel::controller_exits(
            row[0x6c],
            player.bytes[0x10] != 0,
        ))
    }

    /// Worker-side human planning route. None requests the ordinary interface
    /// transition, before inspecting or submitting a stale frontend form.
    pub fn plan_build(
        &mut self,
        request: &crate::build_panel::Request,
        assets: &CommandAssets<'_, '_>,
        submit: bool,
        event: impl FnMut(openmt2001::live_commands::Event) -> Result<(), String>,
    ) -> BuildReply {
        let category = usize::try_from(request.category).map_err(|_| "negative build category")?;
        if self.planning_exits(category)? {
            return Ok(None);
        }
        if submit {
            self.submit_build(request, assets, event).map(Some)
        } else {
            self.inspect_build(request, assets).map(Some)
        }
    }

    /// Read the authoritative city for the frontend's original construction
    /// form. The UI never supplies financial components or player identity.
    pub fn inspect_build(
        &mut self,
        request: &crate::build_panel::Request,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<BuildInspection, String> {
        let category = usize::try_from(request.category).map_err(|_| "negative build category")?;
        let row = self
            .world
            .categories
            .get(category)
            .ok_or("build category outside city")?;
        let rectangle = [
            request.origin[0],
            request.origin[1],
            request.origin[0]
                .wrapping_add(request.size[0])
                .wrapping_sub(1),
            request.origin[1]
                .wrapping_add(request.size[1])
                .wrapping_sub(1),
        ];
        let property_index = u32::from_le_bytes(row[0x8c..0x90].try_into().unwrap());
        let property_address = property_index
            .checked_mul(88)
            .and_then(|v| v.checked_add(0x5385a4))
            .ok_or("Planning property metadata overflow")?;
        // Original4c35a8..4c35c0 applies the name-table buildable byte
        // after occupancy. Utilities and nonbuilding blocks cannot admit a lot.
        let placement_allowed =
            openmt2001::building_pool::can_place(row[0x9c..0xcd].try_into().unwrap(), rectangle)
                && assets.image.at(property_address, 1)?[0] != 0;
        let category_locked = row[0x6c] == 1;
        let player = self
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == self.roster.status.current_player)
            .ok_or("build current player missing")?;
        let owner = self.roster.manager.players[player].bytes[0x1bc].wrapping_add(1);
        if self.roster.manager.players[player].bytes[0x10] == 0 {
            return Err("Planning current player is inactive".into());
        }
        // This entry point is the UI inspection/submit bridge, not the low-level
        // command consumer. Revalidate the original planning gates against the
        // current worker state so a stale frontend snapshot cannot admit a lot.
        if let crate::build_panel::Entry::Message { text, .. } = crate::build_panel::entry(
            row[0x6d],
            row[5] == owner,
            u32::from_le_bytes(row[0x78..0x7c].try_into().unwrap()),
        ) {
            return Err(format!(
                "Planning permission changed (original message {text})"
            ));
        }
        let catalog = self
            .world
            .catalog
            .info
            .row(request.business as usize)
            .ok_or("Planning business outside catalog")?;
        let catalog_word = |at| u32::from_le_bytes(catalog[at..at + 4].try_into().unwrap());
        let type_allowed = if request.business == 43 {
            let identity = u32::from_le_bytes(row[0x8c..0x90].try_into().unwrap());
            let members = crate::build_panel::special_residential_group(assets.image, identity)?;
            let owners = self.world.categories.each_ref().map(|category| category[5]);
            request.kind == 1 && crate::build_panel::hotel_allowed(&members, row[5], &owners)?
        } else {
            catalog_word(0x0c) == request.kind
        };
        if catalog[0x30] == 0 || !type_allowed {
            return Err("Planning business is unavailable for this type".into());
        }
        let [width, depth] = request.size;
        let area = width
            .checked_mul(depth)
            .ok_or("Planning footprint area overflow")?;
        if width <= 0
            || depth <= 0
            || width >= 7
            || depth >= 7
            || area < catalog_word(0x38) as i32
            || area > catalog_word(0x3c) as i32
            || width.max(depth) > catalog_word(0x40) as i32
            || request.height < catalog_word(0x48) as i32
            || request.height > catalog_word(0x44) as i32
            || request.quality > 2
        {
            return Err("Planning dimensions outside original business limits".into());
        }
        let height =
            u8::try_from(request.height).map_err(|_| "build height outside original byte")?;
        let mut command = openmt2001::construction::Command {
            category: category as i8,
            rectangle: rectangle.map(|v| v as i8),
            height: height as i8,
            kind: request.kind,
            business: request.business,
            variation: request.packed_model,
            quality: request.quality as i8,
            owner: owner as i8,
            components: [0.; 4],
        };
        let preview = if placement_allowed {
            let block = assets
                .board
                .blocks
                .iter()
                .find(|b| b.id == category)
                .ok_or("build block absent from board")?;
            openmt2001::scene::SceneBuilding::resolve(
                openmt2001::scene::BuildingSpec {
                    block: block.name.clone(),
                    lot: [
                        rectangle[0] as u8,
                        rectangle[1] as u8,
                        request.size[0] as u8,
                        request.size[1] as u8,
                    ],
                    height,
                    kind: request.kind as u8,
                    variation: request.packed_model,
                    quality: request.quality,
                    businesses: vec![],
                },
                assets.board,
                assets.file,
            )
            .ok()
        } else {
            None
        };
        let total = openmt2001::live_commands::quote(
            &mut command,
            &mut self.world.city,
            &self.roster,
            assets.image,
            self.world.clock.hour(),
        )? as f32;
        let allowance = i32::from_le_bytes(assets.image.at(0x541c7c, 4)?.try_into().unwrap());
        let affordable = openmt2001::construction::affordable(
            self.world.ledgers.players[player].cash,
            allowance,
            total,
            false,
        );
        Ok(BuildInspection {
            total,
            category_locked,
            placement_allowed,
            affordable,
            preview,
            submitted: false,
            command,
        })
    }

    /// Submission is queued on the worker; pausing does not secretly advance
    /// construction or skip the normal scheduler's command delay.
    pub fn submit_build(
        &mut self,
        request: &crate::build_panel::Request,
        assets: &CommandAssets<'_, '_>,
        event: impl FnMut(openmt2001::live_commands::Event) -> Result<(), String>,
    ) -> Result<BuildInspection, String> {
        let mut result = self.inspect_build(request, assets)?;
        if result.allowed() {
            self.submit_command(
                openmt2001::live_commands::Command::Build(result.command.clone()),
                assets,
                event,
            )?;
            result.submitted = true;
        }
        Ok(result)
    }
    /// Local450490 submission. Build commands must already contain the quote
    /// accepted by the original UI checks; its components survive the delay.
    pub fn submit_command(
        &mut self,
        command: openmt2001::live_commands::Command,
        assets: &CommandAssets<'_, '_>,
        event: impl FnMut(openmt2001::live_commands::Event) -> Result<(), String>,
    ) -> Result<(), String> {
        self.command_batch(Some(command), assets, event)
    }

    /// Bind this at original scheduler helper480ec0, before pending-world work.
    pub fn drain_commands(
        &mut self,
        assets: &CommandAssets<'_, '_>,
        event: impl FnMut(openmt2001::live_commands::Event) -> Result<(), String>,
    ) -> Result<(), String> {
        self.command_batch(None, assets, event)
    }

    ///48b350/48b480 callbacks use the retained current-player scratch packet.
    /// The local host chooses zero for original unwritten stack-prior bytes.
    pub fn respond_human_dialog(
        &mut self,
        callback_address: u32,
        button: u32,
        message_kind: u32,
        network: [bool; 2],
        mut event: impl FnMut(openmt2001::live_commands::Event) -> Result<(), String>,
    ) -> Result<(), String> {
        use openmt2001::{human_reply, live_commands, player_messages};
        let current = self.roster.status.current_player;
        let slot = self
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == current)
            .ok_or("human dialog current player unresolved")?;
        if callback_address == 0x48b480 {
            return human_reply::dismiss_notice(&mut self.roster.manager.players[slot]);
        }
        if callback_address != 0x48b350 {
            return Err(format!(
                "human dialog callback{callback_address:08x} unresolved"
            ));
        }
        let references: Vec<_> = self
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.reference)
            .collect();
        let (before, tail) = self.roster.manager.players.split_at_mut(slot);
        let (player, after) = tail.split_first_mut().unwrap();
        let mut arena = player_messages::Arena {
            next: self.allocation_next,
        };
        let visible = u8::from(self.world.auction.presentation.visible);
        let result = human_reply::offer_response(
            player,
            &mut human_reply::World {
                categories: &mut self.world.city.categories,
                player_references: &references,
            },
            &human_reply::Inputs {
                button,
                message_kind,
                command_prior: [0; 52],
            },
            |call, player, world| {
                match call {
                    human_reply::Call::Auction { descriptor } => {
                        if network[0] && !network[1] {
                            return Err("human accepted offer requires network transport".into());
                        }
                        self.live_commands.submit_with_delay(live_commands::Command::Auction{descriptor},self.world.clock.timing[0],descriptor[16]!=0,0,|_|Err("human auction delay wrapped into synchronous execution".into()))?;
                    }
                    human_reply::Call::Refuse { packet, .. } => {
                        player_messages::reply_with_players(
                            &packet,
                            0x26,
                            [0x5f5e0ff; 4],
                            &mut player_messages::SplitPlayers {
                                before,
                                current: player,
                                after,
                            },
                            &mut arena,
                            player_messages::Routing {
                                current_player: current,
                                network,
                                prior: [0; 48],
                            },
                            |route, packet, _| {
                                if let player_messages::Route::PropertyRefresh { category } = route
                                {
                                    openmt2001::category_ownership::property_view(
                                        world
                                            .categories
                                            .get_mut(category as usize)
                                            .ok_or("human refusal category outside pool")?,
                                        visible,
                                        1,
                                    );
                                }
                                event(live_commands::Event::PlayerMessageRoute {
                                    route,
                                    packet: *packet,
                                })
                            },
                        )?;
                    }
                }
                Ok(())
            },
        );
        self.allocation_next = arena.next;
        result
    }
    /// Immediate local464610 action consumer, used by the original auction UI
    /// producers. Admission and control availability belong to the caller.
    pub fn apply_auction_action(
        &mut self,
        command: openmt2001::ai_messages::offers::auction::Command,
        assets: &CommandAssets<'_, '_>,
        mut event: impl FnMut(openmt2001::live_commands::Event) -> Result<u32, String>,
    ) -> Result<(), String> {
        let mut descriptor = [0; 52];
        descriptor[4..8].copy_from_slice(&0x1fu32.to_le_bytes());
        for (off, value) in [
            (20, command.action),
            (24, command.participant),
            (28, command.amount),
        ] {
            descriptor[off..off + 4].copy_from_slice(&value.to_le_bytes());
        }
        if self.tutorial_state != 1 {
            let mut messages = openmt2001::player_messages::Context {
                arena: openmt2001::player_messages::Arena {
                    next: self.allocation_next,
                },
                current_player: self.roster.status.current_player,
                network: [false, false],
                prior: [0; 48],
            };
            let mut runtime = openmt2001::live_commands::Runtime {
                city: &mut self.world.city,
                roster: &mut self.roster,
                population: &mut self.population,
                utilities: &mut self.hooks.utilities,
                board: assets.board,
                event_cards: &mut self.event_cards,
                message_context: &mut messages,
            };
            let result = runtime.auction_phase(
                &descriptor,
                &mut self.world.clock,
                [false, false],
                |call, _| event(call),
            );
            self.allocation_next = messages.arena.next;
            return result;
        }
        let mut context = self.world.script_context(
            assets.image,
            &self.resident_models,
            level_startup::HostContext {
                accounting_enabled: true,
                difficulty: self.difficulty as i32,
                notifications: self.world.city.ai_demand.ready != 0,
                auxiliary_template: self.policy.auxiliary_prior_bytes,
                category_base: 0x617628,
                building_base: 0x591318,
            },
        )?;
        let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
            assets.board,
            assets.file,
            std::mem::take(&mut self.models),
        );
        let mut utilities =
            std::mem::replace(&mut self.hooks.utilities, economy::Utilities::new([0; 4]));
        let mut messages = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena {
                next: self.allocation_next,
            },
            current_player: self.roster.status.current_player,
            network: [false, false],
            prior: [0; 48],
        };
        let hooks = std::cell::RefCell::new(&mut self.hooks);
        let mut announcement = tutorial_announcement::Bridge {
            scripts: &mut self.scripts,
            hooks: &hooks,
            context: &mut context,
        };
        let mut runtime = openmt2001::live_commands::Runtime {
            city: &mut self.world.city,
            roster: &mut self.roster,
            population: &mut self.population,
            utilities: &mut utilities,
            board: assets.board,
            event_cards: &mut self.event_cards,
            message_context: &mut messages,
        };
        let result = runtime.auction_phase_with_tutorial(
            &descriptor,
            &mut self.world.clock,
            [false, false],
            Some((self.tutorial_state, &mut geometry, &mut announcement)),
            |call, _| event(call),
        );
        self.hooks.utilities = utilities;
        self.models = std::mem::take(geometry.registrar_mut());
        self.allocation_next = messages.arena.next;
        result
    }
    fn command_batch(
        &mut self,
        command: Option<openmt2001::live_commands::Command>,
        assets: &CommandAssets<'_, '_>,
        mut event: impl FnMut(openmt2001::live_commands::Event) -> Result<(), String>,
    ) -> Result<(), String> {
        use openmt2001::{
            building_geometry::{BuildingGeometry, RegisteredModel},
            live_commands,
        };
        let mut script_context = if self.tutorial_state == 1 {
            Some(self.world.script_context(
                assets.image,
                &self.resident_models,
                level_startup::HostContext {
                    accounting_enabled: true,
                    difficulty: self.difficulty as i32,
                    notifications: self.world.city.ai_demand.ready != 0,
                    auxiliary_template: self.policy.auxiliary_prior_bytes,
                    category_base: 0x617628,
                    building_base: 0x591318,
                },
            )?)
        } else {
            None
        };
        let mut geometry =
            BuildingGeometry::new(assets.board, assets.file, std::mem::take(&mut self.models));
        // Direct command submission uses deterministic zero prior packet bytes;
        // scheduler-driven submission uses its explicit Session policy instead.
        let mut command_messages = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena {
                next: self.allocation_next,
            },
            current_player: self.roster.status.current_player,
            network: [false, false],
            prior: [0; 0x30],
        };
        let mut utilities =
            std::mem::replace(&mut self.hooks.utilities, economy::Utilities::new([0; 4]));
        let result = (|| {
            for index in 0..geometry.registrar().models.len() {
                let definition = geometry.registrar().models[index].definition;
                geometry.retain_model(RegisteredModel::from_definition(
                    index as u32 + 1,
                    &definition,
                )?)?;
            }
            let mut context = live_commands::CommandContext {
                image: assets.image,
                clock: &mut self.world.clock,
                accounting_enabled: true,
                notifications: self.world.city.ai_demand.ready != 0,
                tutorial_state: self.tutorial_state,
                network_enabled: false,
                network_override: false,
                utility_factor: self.world.city.settings.selected_income().multiplier,
            };
            let mut runtime = live_commands::Runtime {
                event_cards: &mut self.event_cards,
                message_context: &mut command_messages,
                city: &mut self.world.city,
                roster: &mut self.roster,
                population: &mut self.population,
                utilities: &mut utilities,
                board: assets.board,
            };
            let hooks = std::cell::RefCell::new(&mut self.hooks);
            let mut announcement =
                script_context
                    .as_mut()
                    .map(|context| tutorial_announcement::Bridge {
                        scripts: &mut self.scripts,
                        hooks: &hooks,
                        context,
                    });
            let submission_tick = context.clock.timing[0];
            let mut drain_clocks = context.clock.timing[..2].try_into().unwrap();
            if let Some(live_commands::Command::Build(command)) = &command {
                let admitted = openmt2001::construction::admit(
                    command,
                    &mut runtime.city.categories,
                    &mut runtime.roster.manager.players,
                    &mut runtime.message_context.arena,
                    u32::from_le_bytes(runtime.roster.manager.bytes[12..16].try_into().unwrap()),
                    openmt2001::player_messages::Routing {
                        current_player: runtime.message_context.current_player,
                        network: runtime.message_context.network,
                        prior: runtime.message_context.prior,
                    },
                    |route, packet| {
                        event(live_commands::Event::PlayerMessageRoute {
                            route,
                            packet: *packet,
                        })
                    },
                )?;
                if !admitted {
                    return Ok(());
                }
            }
            let apply = |command: &live_commands::Command| {
                runtime.apply_with_announcement(
                    command,
                    &mut context,
                    &mut geometry,
                    announcement
                        .as_mut()
                        .map(|bridge| bridge as &mut dyn live_commands::TutorialAnnouncement),
                    |call, _clock| event(call),
                )
            };
            if let Some(command) = command {
                self.live_commands
                    .submit(command, submission_tick, true, apply)
            } else {
                drop(apply);
                self.live_commands
                    .drain_scheduling(&mut drain_clocks, |command, enqueue| {
                        runtime.apply_with_announcement(
                            command,
                            &mut context,
                            &mut geometry,
                            announcement.as_mut().map(|bridge| {
                                bridge as &mut dyn live_commands::TutorialAnnouncement
                            }),
                            |call, _clock| {
                                if let live_commands::Event::AuctionQueue {
                                    descriptor,
                                    additional_delay,
                                } = call
                                {
                                    enqueue(
                                        live_commands::Command::Auction { descriptor },
                                        openmt2001::local_commands::Schedule::Local {
                                            delayed: descriptor[16] != 0,
                                            additional: additional_delay,
                                        },
                                    )
                                } else if let live_commands::Event::CategoryReadyAt {
                                    category,
                                    normal_tick,
                                } = call
                                {
                                    enqueue(
                                        live_commands::Command::CategoryReady { category },
                                        openmt2001::local_commands::Schedule::Normal {
                                            due: normal_tick,
                                        },
                                    )
                                } else if let live_commands::Event::Outcome(
                                    openmt2001::outcome_notice::Call::Timers(timers),
                                ) = call
                                {
                                    timers.apply(&mut hooks.borrow_mut().timers);
                                    Ok(())
                                } else if matches!(
                                    call,
                                    live_commands::Event::Elimination(
                                        openmt2001::player_elimination::Call::FinishLocal
                                    )
                                ) {
                                    openmt2001::player_elimination::finish_local(
                                        self.network_command_timer,
                                        &mut hooks.borrow_mut().timers,
                                    )?;
                                    event(call)
                                } else {
                                    event(call)
                                }
                            },
                        )
                    })
            }
        })();
        self.hooks.utilities = utilities;
        self.models = std::mem::take(geometry.registrar_mut());
        self.allocation_next = command_messages.arena.next;
        result
    }

    /// Complete construction/demolition transition pass. The host owns effect
    /// allocation and resource release; shared code owns list/occupancy stores.
    pub fn advance_pending(
        &mut self,
        board: &openmt2001::board::Board,
        controls: &mut openmt2001::pending_world::Controls,
        mut effect: impl FnMut(
            openmt2001::live_commands::PendingCall,
            Option<&mut [u8; 0x394]>,
        ) -> Result<u32, String>,
    ) -> Result<(), String> {
        struct Heights<'a>(&'a [crate::building_models::Model]);
        impl openmt2001::building_pool::GeometryBinding for Heights<'_> {
            fn prepare(&mut self, _: usize, _: &mut [u8; 0x394]) -> Result<(), String> {
                Err("pending transitions cannot create geometry".into())
            }
            fn model_height(&mut self, handle: u32) -> Result<f32, String> {
                let model = self
                    .0
                    .get(handle.wrapping_sub(1) as usize)
                    .ok_or("pending model handle unresolved")?;
                Ok(f32::from_le_bytes(
                    model.definition[88..92].try_into().unwrap(),
                ))
            }
        }
        let tree_state = self.models.tree_state.clone();
        let result = openmt2001::live_commands::PendingRuntime {
            city: &mut self.world.city,
            roster: &self.roster,
            commands: &mut self.live_commands,
            selected: &mut self.building_selection,
        }
        .advance(
            self.world.clock.timing[0],
            controls,
            &mut Heights(&self.models.models),
            |call, mut record| {
                if matches!(
                    call,
                    openmt2001::live_commands::PendingCall::ReleaseBuilding { .. }
                ) {
                    crate::building_models::release_record_with_trees(
                        record
                            .as_deref_mut()
                            .ok_or("missing building release record")?,
                        tree_state.as_ref(),
                    )?;
                }
                let value = effect(call, record)?;
                if let openmt2001::live_commands::PendingCall::ReleaseBuilding {
                    building, ..
                } = call
                {
                    self.models.bindings.remove(&building);
                }
                Ok(value)
            },
        );
        let refreshed = self.population.refresh_city(&self.world.city, board);
        result.and(refreshed)
    }

    pub fn scene(
        &self,
        build_file: &[u8],
        original: openmt2001::render_scene::OriginalAssets<'_>,
        suncols: &[u8],
        camera: Option<openmt2001::flyby::CameraSample>,
        animated: bool,
    ) -> Result<openmt2001::render_scene::Scene, String> {
        self.scene_with_style(build_file, original, suncols, camera, animated, None)
    }

    // Preserve the established scene() fixture inputs while adding one shared
    //host palette; combining those inputs would churn independent callers.
    #[allow(clippy::too_many_arguments)]
    pub fn scene_with_style(
        &self,
        build_file: &[u8],
        original: openmt2001::render_scene::OriginalAssets<'_>,
        suncols: &[u8],
        camera: Option<openmt2001::flyby::CameraSample>,
        animated: bool,
        style: Option<&openmt2001::board_style::Style>,
    ) -> Result<openmt2001::render_scene::Scene, String> {
        self.scene_with_style_cached(build_file, original, suncols, camera, animated, style, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn scene_with_style_cached(
        &self,
        build_file: &[u8],
        original: openmt2001::render_scene::OriginalAssets<'_>,
        suncols: &[u8],
        camera: Option<openmt2001::flyby::CameraSample>,
        animated: bool,
        style: Option<&openmt2001::board_style::Style>,
        textures: Option<&mut openmt2001::texture::LoaderCache>,
    ) -> Result<openmt2001::render_scene::Scene, String> {
        let request = openmt2001::render_scene::Request {
            build_file,
            source: openmt2001::render_scene::Source::RuntimeBuildings {
                records: &self.world.buildings,
                category_base: openmt2001::building_geometry::V14_CATEGORY_BASE,
            },
            original: Some(original),
            suncols: Some(suncols),
            phase: Some(self.world.clock.hour()),
            camera,
            width: 1280,
            height: 960,
            animated,
        };
        let mut scene = if let Some(textures) = textures {
            openmt2001::render_scene::assemble_with_style_cached(request, style, textures)?
        } else {
            openmt2001::render_scene::assemble_with_style(request, style)?
        };
        self.refresh_scene_records(&mut scene)?;
        if let Some(image) = self.window_texture.current() {
            scene.set_window_pixels(image.pixels)?;
        }
        Ok(scene)
    }

    /// Refresh current business/player metadata without rebuilding unchanged meshes.
    pub fn refresh_scene_records(
        &self,
        scene: &mut openmt2001::render_scene::Scene,
    ) -> Result<(), String> {
        scene.saved_business_catalog = Some(self.world.catalog.info.clone());
        scene.saved_commodity_rows = Some(self.world.commodities.rows);
        for building in &mut scene.inspection_buildings {
            building.initialized_apartment = None;
            building.initialized_business = None;
            building.saved_owner_character = None;
            let slot = building
                .runtime_slot
                .ok_or("initialized building identity missing")?;
            let record = &self.world.buildings[slot];
            if record[0x2d] != 0 {
                let player = self
                    .roster
                    .manager
                    .players
                    .get(usize::from(record[0x2d]) - 1)
                    .ok_or("live building owner leaves roster")?;
                let character = u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap());
                if character >= 10 {
                    return Err("live building owner character leaves palette".into());
                }
                building.saved_owner_character = Some(character);
            }
            let business_slot = i32::from_le_bytes(record[0x1c..0x20].try_into().unwrap());
            if business_slot == -1 {
                continue;
            }
            let business_slot =
                usize::try_from(business_slot).map_err(|_| "negative initialized business slot")?;
            let business = self
                .world
                .ledgers
                .businesses
                .get(business_slot)
                .ok_or("initialized business leaves pool")?;
            if business[0x21c] == 0
                || usize::from(u16::from_le_bytes(business[8..10].try_into().unwrap())) != slot
            {
                return Err("initialized business/building linkage disagrees".into());
            }
            // 40c5ae/418ff3 color the adjustment title from business+0xa,
            // which may differ from the building+0x2d land proprietor.
            let owner_character = if business[0xa] == 0 {
                None
            } else {
                let player = self
                    .roster
                    .manager
                    .players
                    .get(usize::from(business[0xa]) - 1)
                    .ok_or("initialized business owner leaves roster")?;
                let character = u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap());
                if character >= 10 {
                    return Err("initialized business owner character leaves palette".into());
                }
                Some(character)
            };
            if business[..4] == [1, 0, 0, 0] {
                let kind = u32::from_le_bytes(business[4..8].try_into().unwrap()) as usize;
                let info = self
                    .world
                    .catalog
                    .info
                    .row(kind)
                    .ok_or("initialized apartment leaves catalog")?;
                if building.kind != 1 || info[0xc..0x10] != [1, 0, 0, 0] {
                    return Err("initialized apartment discriminator disagrees".into());
                }
                building.initialized_apartment =
                    Some(openmt2001::render_scene::InitializedApartmentInspection {
                        slot: business_slot,
                        name_text_id: u32::from_le_bytes(info[4..8].try_into().unwrap()),
                        record: *business,
                        owner_character,
                    });
            } else if business[..4] == [0; 4] || business[..4] == [2, 0, 0, 0] {
                let kind = u32::from_le_bytes(business[4..8].try_into().unwrap());
                let info = self
                    .world
                    .catalog
                    .info
                    .row(kind as usize)
                    .ok_or("initialized business leaves catalog")?;
                if building.kind != business[0] || info[0xc..0x10] != business[..4] {
                    return Err("initialized business discriminator disagrees".into());
                }
                building.initialized_business =
                    Some(openmt2001::render_scene::SavedBusinessInspection {
                        slot: business_slot,
                        record: *business,
                        owner_character,
                        business_type: kind,
                        name_text_id: u32::from_le_bytes(info[4..8].try_into().unwrap()),
                        account: openmt2001::economy::BusinessAccount::from_saved(
                            &openmt2001::save::SavedBusiness { bytes: business },
                        ),
                    });
            }
        }
        Ok(())
    }

    pub fn initialize(input: Inputs<'_, 'data>, policy: Policy) -> Result<Self, String> {
        Self::initialize_with_catalog(
            input,
            policy,
            None,
            None,
            false,
            None,
            crate::compatibility::Mode::StrictV14,
            None,
            None,
        )
    }

    /// Start from the host's loaded campaign profile, preserving its unlocks
    /// and selected level before the level scripts run.
    pub fn initialize_campaign(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: std::path::PathBuf,
    ) -> Result<Self, String> {
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            Some(profile_path),
            false,
            None,
            crate::compatibility::Mode::StrictV14,
            None,
            None,
        )
    }

    pub fn initialize_campaign_with_style(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: std::path::PathBuf,
        style: &openmt2001::board_style::Style,
    ) -> Result<Self, String> {
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            Some(profile_path),
            false,
            Some(style),
            crate::compatibility::Mode::StrictV14,
            None,
            None,
        )
    }

    /// Start with the host-selected compatibility policy. Legacy entry points
    /// above deliberately retain the original v1.4 ownership result.
    pub fn initialize_campaign_compatible(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: std::path::PathBuf,
        compatibility: crate::compatibility::Mode,
    ) -> Result<Self, String> {
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            Some(profile_path),
            false,
            None,
            compatibility,
            None,
            None,
        )
    }

    pub fn initialize_campaign_with_style_and_compatibility(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: std::path::PathBuf,
        style: &openmt2001::board_style::Style,
        compatibility: crate::compatibility::Mode,
    ) -> Result<Self, String> {
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            Some(profile_path),
            false,
            Some(style),
            compatibility,
            None,
            None,
        )
    }

    /// Fresh offline kernel only. Policy.seed is injected by the host; offline
    /// wall-clock sampling belongs at the future launch boundary.
    pub fn initialize_skirmish(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        launch: SkirmishLaunch,
    ) -> Result<Self, String> {
        Self::initialize_skirmish_inner(input, policy, catalog, launch, None, None)
    }

    pub fn initialize_skirmish_with_style(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        launch: SkirmishLaunch,
        style: &openmt2001::board_style::Style,
    ) -> Result<Self, String> {
        Self::initialize_skirmish_inner(input, policy, catalog, launch, Some(style), None)
    }

    fn initialize_skirmish_inner(
        mut input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        launch: SkirmishLaunch,
        style: Option<&openmt2001::board_style::Style>,
        process: Option<crate::live_host::process_runtime::Handle>,
    ) -> Result<Self, String> {
        launch.validate()?;
        let options = openmt2001::skirmish::Options::from_pe(input.image)?;
        let rule = options
            .rules()
            .iter()
            .find(|r| r.kind == launch.objective_kind)
            .ok_or("unknown Skirmish objective")?;
        if launch.target < rule.minimum as f32
            || launch.target > rule.maximum as f32
            || launch.target.fract() != 0.
        {
            return Err("Skirmish target outside original bounds".into());
        }
        input.difficulty = launch.difficulty;
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            None,
            false,
            style,
            crate::compatibility::Mode::StrictV14,
            Some(launch),
            process,
        )
    }

    /// Authored startup without a named profile, for read-only/import callers.
    pub fn initialize_custom_with_style(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        style: &openmt2001::board_style::Style,
        compatibility: crate::compatibility::Mode,
    ) -> Result<Self, String> {
        Self::initialize_custom_with_style_and_profile(
            input,
            policy,
            catalog,
            None,
            style,
            compatibility,
        )
    }

    /// Original Custom completion uses the active named profile and shared
    /// catalog. The caller supplies its progress destination independently of saves.
    pub fn initialize_custom_with_style_and_profile(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: Option<std::path::PathBuf>,
        style: &openmt2001::board_style::Style,
        compatibility: crate::compatibility::Mode,
    ) -> Result<Self, String> {
        let level = catalog
            .levels
            .iter()
            .find(|level| custom::same_directory(level.directory(), input.scenario.as_bytes()))
            .ok_or("custom scenario missing from registered catalog")?;
        if !custom::valid_level(level) {
            return Err("invalid custom scenario identity".into());
        }
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            profile_path,
            false,
            Some(style),
            compatibility,
            None,
            None,
        )
    }

    /// Explicit process identity for standalone callers that execute Game.performance.
    pub fn initialize_in_process(
        input: Inputs<'_, 'data>,
        policy: Policy,
        process: crate::live_host::process_runtime::Handle,
    ) -> Result<Self, String> {
        Self::initialize_with_catalog(
            input,
            policy,
            None,
            None,
            false,
            None,
            crate::compatibility::Mode::StrictV14,
            None,
            Some(process),
        )
    }
    pub fn initialize_campaign_with_style_and_compatibility_in_process(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: std::path::PathBuf,
        style: &openmt2001::board_style::Style,
        compatibility: crate::compatibility::Mode,
        process: Option<crate::live_host::process_runtime::Handle>,
    ) -> Result<Self, String> {
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            Some(profile_path),
            false,
            Some(style),
            compatibility,
            None,
            process,
        )
    }
    pub fn initialize_skirmish_with_style_in_process(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        launch: SkirmishLaunch,
        style: &openmt2001::board_style::Style,
        process: Option<crate::live_host::process_runtime::Handle>,
    ) -> Result<Self, String> {
        Self::initialize_skirmish_inner(input, policy, catalog, launch, Some(style), process)
    }
    pub fn initialize_custom_with_style_and_profile_in_process(
        input: Inputs<'_, 'data>,
        policy: Policy,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: Option<std::path::PathBuf>,
        style: &openmt2001::board_style::Style,
        compatibility: crate::compatibility::Mode,
        process: Option<crate::live_host::process_runtime::Handle>,
    ) -> Result<Self, String> {
        let level = catalog
            .levels
            .iter()
            .find(|level| custom::same_directory(level.directory(), input.scenario.as_bytes()))
            .ok_or("custom scenario missing from registered catalog")?;
        if !custom::valid_level(level) {
            return Err("invalid custom scenario identity".into());
        }
        Self::initialize_with_catalog(
            input,
            policy,
            Some(catalog),
            profile_path,
            false,
            Some(style),
            compatibility,
            None,
            process,
        )
    }

    fn initialize_with_catalog(
        input: Inputs<'_, 'data>,
        policy: Policy,
        supplied_catalog: Option<openmt2001::level_catalog::Catalog>,
        profile_path: Option<std::path::PathBuf>,
        load_mode: bool,
        style: Option<&openmt2001::board_style::Style>,
        compatibility: crate::compatibility::Mode,
        skirmish: Option<SkirmishLaunch>,
        process: Option<crate::live_host::process_runtime::Handle>,
    ) -> Result<Self, String> {
        Self::initialize_joined(
            input,
            policy,
            supplied_catalog,
            profile_path,
            load_mode,
            style,
            compatibility,
            skirmish,
            process,
            None,
        )
    }

    pub(crate) fn initialize_joined(
        input: Inputs<'_, 'data>,
        policy: Policy,
        supplied_catalog: Option<openmt2001::level_catalog::Catalog>,
        profile_path: Option<std::path::PathBuf>,
        load_mode: bool,
        style: Option<&openmt2001::board_style::Style>,
        compatibility: crate::compatibility::Mode,
        skirmish: Option<SkirmishLaunch>,
        process: Option<crate::live_host::process_runtime::Handle>,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<Self, String> {
        Self::initialize_joined_inner(
            input,
            policy,
            supplied_catalog,
            profile_path,
            load_mode,
            style,
            compatibility,
            skirmish,
            process,
            dispatch,
            None,
        )
    }
    pub(crate) fn initialize_connected_joined(
        input: Inputs<'_, 'data>,
        policy: Policy,
        stage: &crate::intro::connected_start::Staged,
        style: Option<&openmt2001::board_style::Style>,
        process: crate::live_host::process_runtime::Handle,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<Self, String> {
        stage.launch.validate_roster(true)?;
        process
            .select_connected_seed(policy.seed)
            .map_err(|e| e.to_string())?;
        Self::initialize_joined_inner(
            input,
            policy,
            Some(stage.catalog.clone()),
            None,
            false,
            style,
            crate::compatibility::Mode::StrictV14,
            Some(stage.launch.clone()),
            Some(process),
            dispatch,
            Some(stage),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn initialize_joined_inner(
        mut input: Inputs<'_, 'data>,
        policy: Policy,
        supplied_catalog: Option<openmt2001::level_catalog::Catalog>,
        profile_path: Option<std::path::PathBuf>,
        load_mode: bool,
        style: Option<&openmt2001::board_style::Style>,
        compatibility: crate::compatibility::Mode,
        skirmish: Option<SkirmishLaunch>,
        process: Option<crate::live_host::process_runtime::Handle>,
        mut dispatch: Option<crate::process_dispatch::Seed>,
        connected: Option<&crate::intro::connected_start::Staged>,
    ) -> Result<Self, String> {
        if !load_mode
            && dispatch
                .as_ref()
                .is_some_and(|seed| (seed.pause.bytes[0] as i8) < 0)
        {
            return Err("Unsupported negative reason0 before World reset".into());
        }
        if let Some(launch) = &skirmish {
            launch.validate_roster(connected.is_some())?;
            let options = openmt2001::skirmish::Options::from_pe(input.image)?;
            let rule = options
                .rules()
                .iter()
                .find(|r| r.kind == launch.objective_kind)
                .ok_or("unknown Skirmish objective")?;
            if launch.target < rule.minimum as f32
                || launch.target > rule.maximum as f32
                || launch.target.fract() != 0.
            {
                return Err("Skirmish target outside original bounds".into());
            }
        }
        if !(0..=2).contains(&input.difficulty) {
            return Err("campaign difficulty outside three levels".into());
        }
        let mut catalog = if let Some(catalog) = supplied_catalog {
            catalog
        } else {
            let mut catalog = crate::campaign_store::load_catalog(input.image, input.scripts)?;
            catalog.reset_progress();
            catalog.unlock_first();
            catalog
        };
        let selected = catalog
            .find_directory(input.scenario.as_bytes())
            .or_else(|| {
                catalog.levels.iter().position(|level| {
                    custom::valid_level(level)
                        && custom::same_directory(level.directory(), input.scenario.as_bytes())
                })
            })
            .ok_or("scenario missing from original level catalog")?;
        let id = catalog.levels[selected].id();
        let is_custom = custom::valid_level(&catalog.levels[selected]);
        let saved_net = load_mode
            && (300000..=300006).contains(&id)
            && !catalog.levels[selected].single_player()
            && catalog.levels[selected]
                .directory()
                .eq_ignore_ascii_case(format!("net{}", id - 300000).as_bytes());
        if skirmish.is_none()
            && !saved_net
            && !is_custom
            && !(200001..=200016).contains(&id)
            && !matches!(id, 100000..=100012 | 400001 | 400002)
        {
            return Err("fresh-city initialization requires a supported scenario, Tutorials 1 through 12, Bonus or Sandbox".into());
        }
        if let Some(launch) = &skirmish {
            if id != launch.level_id
                || catalog.levels[selected].directory() != launch.directory.as_bytes()
                || catalog.levels[selected].single_player()
            {
                return Err(
                    "Skirmish identity does not match selected multiplayer catalog record".into(),
                );
            }
            catalog.levels[selected].set_objective_target(launch.target);
        }
        if !catalog.select(id, skirmish.is_some() || saved_net) {
            return Err("scenario is not a single-player catalog entry".into());
        }
        catalog.difficulty = input.difficulty as u32;
        let resolved_skirmish_map = if let Some(launch) = &skirmish {
            Some(if let Some(stage) = connected {
                stage.map.clone()
            } else {
                crate::skirmish_maps::Catalog::discover(input.scripts)?
                    .resolve(input.scripts, &launch.map)?
            })
        } else {
            None
        };
        let skirmish_map = resolved_skirmish_map
            .as_ref()
            .map(|map| map.identity.clone())
            .or_else(|| {
                saved_net.then_some(crate::skirmish_maps::MapIdentity::MaterializedUnknown)
            });
        let configuration = openmt2001::resident::Configuration::before_defaults(
            input.parameters,
            input.timetable,
            input.image,
        )?;
        let startup = if connected.is_some() {
            process
                .as_ref()
                .ok_or("Connected startup has no process seed owner")?
                .reseed_connected_city()
                .map_err(|e| e.to_string())?
        } else {
            openmt2001::startup_rng::Snapshot::from_seed(policy.seed)
        };
        let creation =
            startup.resident_creation(openmt2001::resident::AuxiliaryPool::for_residents());
        let objective = dispatch
            .as_mut()
            .map(|seed| std::mem::take(&mut seed.objective))
            .unwrap_or_default();
        let mut world = level_startup::World::fresh_with_objective(
            input.image,
            input.board,
            configuration,
            creation,
            objective,
        )?;
        world.city.building_parameters =
            openmt2001::building_parameters::BuildingKinds::from_parameters(
                input.parameters,
                input.image,
            )?;
        if let Some(style) = style {
            style.apply_categories(&mut world.categories);
        }
        let mut scripts = level_startup::Scripts::new(input.image)?;
        let read = |relative: &str| {
            if let Some(stage) = connected {
                stage.read(relative)
            } else if is_custom {
                custom::read(input.scripts, relative)
            } else {
                fs::read(input.scripts.join(relative)).map_err(|e| format!("{relative}: {e}"))
            }
        };
        // Explicit original nested sources. Runtime normalizes their original
        // script names; arbitrary scripts outside this verified stage are rejected.
        for name in [
            "auctioncamera",
            "sound",
            "general",
            "commoditysettings",
            "blocksettings",
            "businesssettings",
            "buildingsettings",
            "default",
        ] {
            scripts.runtime.script_buffer(
                &format!("scripts/default/{name}.lua"),
                &read(&format!("DEFAULT/{name}.lua"))?,
            )?;
        }
        if skirmish.is_some() || saved_net {
            let relative = format!("{}/initialpref.lua", input.scenario);
            scripts
                .runtime
                .script_buffer(&format!("scripts/{relative}"), &read(&relative)?)?;
        } else if is_custom {
            custom::register_sources(&mut scripts.runtime, input.scripts, input.scenario)?;
        } else {
            scenarios::register_sources(&mut scripts.runtime, input.scripts)?;
        }
        scripts
            .bindings
            .bind_selected_level(catalog.selected_level());
        // These opaque identities resolve to retained original people meshes;
        // no resident drawing or native graphics-manager pointer is claimed.
        let handles = std::array::from_fn(|i| [(i * 2 + 1) as u32, (i * 2 + 2) as u32]);
        let resident_models = input.people.resident_bindings(&handles)?;
        let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
            input.board,
            input.file,
            crate::building_models::Models::with_tree_locators(input.tree_locators.as_deref())?,
        );
        if let Some(seed) = dispatch.as_mut() {
            if load_mode {
                seed.world_reset_accumulators();
            } else {
                seed.fresh_reset()?;
            }
        }
        let mut hooks = if let Some(seed) = dispatch.take() {
            world.pause = seed.pause; // SAME process image, before any top-level script.
            Hooks::with_dispatch(seed.tasks, seed.frequency)
        } else {
            Hooks::new()
        };
        hooks.process = process; // Before top-level, initialize and Level dispatch.
        let context = world.script_context(
            input.image,
            &resident_models,
            level_startup::HostContext {
                accounting_enabled: false,
                difficulty: input.difficulty,
                notifications: false,
                auxiliary_template: policy.auxiliary_prior_bytes,
                category_base: openmt2001::building_geometry::V14_CATEGORY_BASE,
                building_base: 0x591318,
            },
        )?;
        let default_hub = read("DEFAULT/hub.lua")?;
        let scenario_hub = read(&format!("{}/hub.lua", input.scenario))?;
        let sources = level_startup::Sources {
            default_hub: &default_hub,
            scenario_name: input.scenario,
            scenario_hub: &scenario_hub,
        };
        if !load_mode && !matches!(id, 100000..=100012) {
            scripts.initialize_active(
                sources,
                &mut world.script_state(&[None; 6]),
                &context,
                &mut geometry,
                &mut hooks,
            )?;
        } else {
            scripts.initialize(
                sources,
                &mut world.script_state(&[None; 6]),
                &context,
                &mut geometry,
                &mut hooks,
            )?;
        }
        if let Some(launch) = &skirmish {
            //4ba07d: Level has executed; the extra multiplayer map precedes48cca0.
            let resolved = resolved_skirmish_map
                .as_ref()
                .ok_or("selected Skirmish map absent")?;
            let mut state = world.script_state(&[None; 6]);
            let bindings = &mut scripts.bindings;
            let events = &mut scripts.events;
            scripts.runtime.execute(
                &format!("scripts/maps/{}", launch.map),
                &resolved.bytes,
                &mut |id, args| {
                    bindings.handle(
                        id,
                        args,
                        &mut state,
                        events,
                        &context,
                        &mut geometry,
                        &mut hooks,
                    )
                },
            )?;
            if world.objective.kind != launch.objective_kind
                || world.objective.target != launch.target
            {
                return Err("authored Skirmish objective differs from confirmed setup".into());
            }
        }
        if matches!(id, 100009 | 100012) {
            let hub = read(&format!("{}/hub.lua", input.scenario))?;
            let compact: String = std::str::from_utf8(&hub)
                .map_err(|e| e.to_string())?
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .collect();
            let bid = if id == 100009 {
                "Interface.ForcePlayerToBid(a,50*a);"
            } else {
                "Interface.ForcePlayerToBid(a,1181+(500*a)+(g_counter*50));"
            };
            for (name, call) in [
                ("EVENT_AIBid", bid),
                ("EVENT_AIPass", "Interface.ForcePlayerToPass(a);"),
                ("EVENT_AIPullout", "Interface.ForcePlayerToPullOut(a);"),
            ] {
                let declaration = format!("function{name}(a)");
                if compact.matches(&declaration).count() != 1
                    || !compact.contains(&format!("{declaration}{call}end;"))
                {
                    return Err(format!(
                        "forced auction handler{name} is not the audited terminal original shape"
                    ));
                }
                let reference = scripts
                    .runtime
                    .reference_function(name)?
                    .ok_or("forced auction original handler absent")?;
                let identity = scripts.runtime.event_reference_identity(reference);
                scripts.runtime.release_reference(reference)?;
                let (actual_name, body) =
                    identity?.ok_or("forced auction original handler identity unresolved")?;
                if actual_name != name {
                    return Err("forced auction original handler name mismatch".into());
                }
                hooks.forced_auction_handlers.insert(actual_name, body);
            }
        }
        let models = std::mem::take(geometry.registrar_mut());
        let mut roster = level_startup::Roster::new([0; 0x28])?;
        let mut random_registry = openmt2001::player_manager::RandomRegistry {
            process: startup.process,
            head: openmt2001::startup_rng::HEAD,
            tail: openmt2001::startup_rng::TAIL,
        };
        let mut allocation_next = 0x20000000_u32;
        let mut static_rng_tail_next = 0;
        let mut saved_player_metadata = [[0u8; 48]; 6];
        if !load_mode {
            let mut pending = openmt2001::player_script::PendingRoster::default();
            if let Some(launch) = &skirmish {
                for (slot, participant) in launch.participants.iter().enumerate() {
                    pending.records[slot] = openmt2001::player_script::Registration {
                        index: slot as i32,
                        kind: if participant.kind == openmt2001::skirmish::roster::Kind::Human {
                            0
                        } else {
                            1
                        },
                        character: participant.token.unwrap() as i32,
                        difficulty: participant.difficulty,
                    };
                    pending.script_characters[slot] = participant.token.unwrap() as i32;
                }
            } else if matches!(id, 100000..=100012) {
                // Original 4a84cd..4a851b: tutorial players.lua is optional. When
                // absent, 4a6a20 registers slot 0 as kind 0, character 0.
                match fs::read(
                    input
                        .scripts
                        .join(format!("{}/players.lua", input.scenario)),
                ) {
                    Ok(source) => {
                        for registration in openmt2001::player_script::evaluate(
                            input.scenario,
                            &source,
                            input.difficulty,
                        )? {
                            let slot = usize::try_from(registration.index)
                                .map_err(|_| "negative tutorial registration slot")?;
                            *pending
                                .records
                                .get_mut(slot)
                                .ok_or("tutorial registration exceeds six slots")? = registration;
                            pending.script_characters[slot] = registration.character;
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        pending.records[0].kind = 0;
                        pending.records[0].character = 0;
                    }
                    Err(error) => return Err(error.to_string()),
                }
            } else {
                pending.load_normal(
                    input.scenario,
                    &read(&format!("{}/players.lua", input.scenario))?,
                    input.difficulty,
                    i32::try_from(input.selected_character)
                        .map_err(|_| "character index overflow")?,
                )?;
            }
            let names: [&[u8]; 6] = std::array::from_fn(|slot| {
                if let Some(launch) = &skirmish {
                    return launch
                        .participants
                        .get(slot)
                        .map_or(&[][..], |p| p.name.as_slice());
                }
                let record = pending.records[slot];
                if record.kind == 0 {
                    input.profile_name
                } else if record.kind == 1 {
                    input.character_names[record.character as usize].as_slice()
                } else {
                    &[]
                }
            });
            // Native86cea0 rows retain the frontend's character/kind/name,
            // offline peer ID(+40) and4a6a00 difficulty(+44). They also feed
            //485520 appearance and80000 save records independently of Players.
            for ((metadata, registration), name) in saved_player_metadata
                .iter_mut()
                .zip(&pending.records)
                .zip(names)
            {
                metadata[..4].copy_from_slice(&registration.character.to_le_bytes());
                metadata[4..8].copy_from_slice(&registration.kind.to_le_bytes());
                let name = &name[..name
                    .iter()
                    .position(|&byte| byte == 0)
                    .unwrap_or(name.len())];
                if name.len() >= 32 {
                    return Err("frontend player metadata name exceeds field".into());
                }
                metadata[8..8 + name.len()].copy_from_slice(name);
                if let Some(launch) = &skirmish {
                    let peer = if launch
                        .participants
                        .get(registration.index as usize)
                        .is_some_and(|p| p.kind == openmt2001::skirmish::roster::Kind::Ai)
                    {
                        -1i32
                    } else {
                        0
                    };
                    let peer = connected.map_or(peer, |stage| {
                        stage
                            .peers
                            .get(registration.index as usize)
                            .copied()
                            .flatten()
                            .map_or(-1, i32::from)
                    });
                    metadata[40..44].copy_from_slice(&peer.to_le_bytes());
                }
                metadata[44..48].copy_from_slice(&registration.difficulty.to_le_bytes());
            }
            // Explicit fresh host arena: zero-filled allocations and opaque,
            // nonoverlapping logical identities, never native host addresses.
            let mut demand_ready = world.city.ai_demand.ready;
            let scenario_ai = if let Some(stage) = connected {
                stage
                    .scripts
                    .get(&format!("{}/ai.lua", input.scenario).to_ascii_lowercase())
                    .cloned()
            } else if is_custom {
                custom::read_optional(input.scripts, &format!("{}/ai.lua", input.scenario))?
            } else {
                match fs::read(input.scripts.join(format!("{}/ai.lua", input.scenario))) {
                    Ok(bytes) => Some(bytes),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.to_string()),
                }
            };
            let cash = world.player_money;
            let roster_allocation_next = std::cell::Cell::new(allocation_next);
            let allocate_roster = |size: usize| -> Result<u32, String> {
                let reference = roster_allocation_next.get();
                let span = u32::try_from(size)
                    .map_err(|_| "player allocation size overflow")?
                    .checked_add(0xfff)
                    .ok_or("player allocation alignment overflow")?
                    & !0xfff;
                roster_allocation_next.set(
                    reference
                        .checked_add(span)
                        .ok_or("player arena exhausted")?,
                );
                Ok(reference)
            };
            roster.initialize_world_with_ownership_allocator(
                &pending,
                openmt2001::player_script::RosterInputs {
                    local_slot: connected.map(|stage| stage.local_slot),
                    names,
                    initial_cash: cash,
                    default_ai: &read("DEFAULT/ai.lua")?,
                    scenario_ai: scenario_ai
                        .as_deref()
                        .map(|bytes| ("scenario/ai.lua", bytes)),
                },
                &mut openmt2001::player_constructor::RosterConstruction {
                    image: input.image,
                    registry: &mut random_registry,
                    global_reset: &mut demand_ready,
                    allocate: &mut |size| {
                        let reference = allocate_roster(size)?;
                        Ok(Some(openmt2001::player_constructor::Allocation {
                            reference,
                            bytes: vec![0; size],
                        }))
                    },
                    external_rng_link: &mut |reference, next| {
                        if reference != openmt2001::startup_rng::TAIL || static_rng_tail_next != 0 {
                            return Err("unexpected external RNG tail during fresh roster".into());
                        }
                        static_rng_tail_next = next;
                        Ok(())
                    },
                },
                &mut world,
                input.difficulty,
                |size| allocate_roster(size),
            )?;
            allocation_next = roster_allocation_next.get();
            world.city.ai_demand.ready = demand_ready;
            let mut ownership_lists = std::mem::take(&mut roster.ai_ownership);
            let ownership_catalog = world.catalog.info.clone();
            // Retain script-authored values before the native ownership reset.
            let authored_rights = std::array::from_fn(|index| {
                let row = &world.categories[index];
                (
                    row[0x05],
                    u32::from_le_bytes(row[0x78..0x7c].try_into().unwrap()),
                )
            });
            let ownership_result =
                roster.initialize_ownership(&mut world, input.image, &mut |ownership, event| {
                    use openmt2001::category_ownership::Seam;
                    match event {
                        Seam::AccountPosted(posting) if posting.amount == 0. => Ok(()),
                        Seam::BuildingAppearance { building } => {
                            // Initial scene submission materializes these retained records.
                            hooks.notifications.push((0x485520, building));
                            Ok(())
                        }
                        Seam::AiOwnershipLists { player } => {
                            openmt2001::ai_ownership::maintain_player(
                                &mut ownership_lists,
                                &mut ownership.players[player],
                                ownership.categories,
                                ownership.buildings,
                                ownership.businesses,
                                input.image,
                                &ownership_catalog,
                                |size| {
                                    let reference = allocation_next;
                                    let span = u32::try_from(size)
                                        .map_err(|_| "ownership allocation size overflow")?
                                        .checked_add(0xfff)
                                        .ok_or("ownership allocation alignment overflow")?
                                        & !0xfff;
                                    allocation_next = allocation_next
                                        .checked_add(span)
                                        .ok_or("ownership allocation arena exhausted")?;
                                    Ok(reference)
                                },
                            )
                        }
                        Seam::NativeUiCall {
                            address: 0x41f410, ..
                        } => {
                            hooks.notifications.push((0x41f410, 0));
                            Ok(())
                        }
                        other => Err(format!(
                            "initial player-owned category needs host boundary {other:?}"
                        )),
                    }
                });
            roster.ai_ownership = ownership_lists;
            ownership_result?;
            compatibility.repair_startup_rights(
                id,
                load_mode,
                &authored_rights,
                &mut world.categories,
            );
        }
        openmt2001::traffic::road::routes::RoadRoutes::from_bytes(
            &input.road_routes,
            world.road_network.road_segment_count as u32,
        )?;
        let mut vehicle_template = policy.auxiliary_prior_bytes;
        let road_traffic = openmt2001::traffic::road::runtime::RoadTraffic::fresh(
            input.image,
            world.road_network.entry_count,
            [startup.state(0x6744c0), 0x343fd, 0x269ec3],
            &mut vehicle_template,
            0x10020000,
            &mut input.vehicle_models,
        )?;
        openmt2001::category_reset::refresh_prestige(
            &mut world.city.categories,
            &world.city.definitions.reset_values(),
        );
        let mut population = openmt2001::level_runtime::Population::new(
            &world,
            input.image,
            input.board,
            input.parameters,
            &resident_models,
            &input.people,
        )?;
        // Original48de60 consumes the values retained by difficulty application.
        world
            .settings
            .selected_income()
            .reset_income(&mut hooks.utilities, &mut population.properties);
        // Original4ba148 resets shared metrics before the demand/space refresh.
        {
            let city = &mut world.city;
            let players: Vec<_> = roster.manager.players.iter().collect();
            openmt2001::player_metrics::reset(
                &mut city.ai_neighbors,
                &mut city.ownership_refresh_counter,
                &mut population.business_counts,
                &mut population.market,
                &players,
                &city.categories,
                &city.ledgers.businesses,
                &city.ledgers.residents,
                input.image,
            )?;
        }
        // Original fresh-start4ba14d rebuilds demand after resident/player
        // setup and income reset, before the post-start player callback loop.
        {
            let city = &mut world.city;
            city.ai_demand.rebuild(
                &city.ledgers.residents,
                &city.ledgers.businesses,
                &city.buildings,
                &city.catalog.info,
                &city.catalog.products,
                &city.commodities.rows,
                input.image,
            )?;
            openmt2001::ai_space::refresh_all(
                &mut city.category_footprints,
                &city.categories,
                input.image,
            )?;
        }
        let mut history = openmt2001::player_history::State::default();
        let transport = openmt2001::traffic::scene::Scene::fresh(
            input.image,
            [startup.state(0x659338), 0x343fd, 0x269ec3],
        )?;
        let train_routes = input.transport_models.routes()?;
        if !load_mode {
            let accounts = roster.account_prefixes(&world.city)?;
            for _ in 0..=world.clock.hour() {
                population.sample_history(&mut history, &world.city, &accounts, input.image)?;
            }
        }
        Ok(Self {
            building_styles: input
                .era_styles
                .get(world.clock.calendar[2] as usize)
                .ok_or("fresh era leaves original style banks")?
                .clone(),
            era_styles: input.era_styles,
            player_style_indices: std::array::from_fn(|i| {
                i32::from_le_bytes(
                    input
                        .image
                        .at(0x53c9dc + i as u32 * 4, 4)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                )
            }),
            difficulty: input.difficulty as usize,
            // The original sixteen campaign registrations use mode2.
            tutorial_state: u8::from(matches!(id, 100000..=100012)),
            catalog,
            skirmish_map,
            campaign_profile: profile_path,
            setup: None,
            connected: None,
            network_command_timer: None,
            world,
            scripts,
            models,
            people: input.people,
            resident_models,
            hooks,
            policy,
            roster,
            random_registry,
            script_random_state: startup.state(0x93aee8),
            credit_random_state: startup.state(0x8b06e8),
            particle_random_state: startup.state(0x68c028),
            demolition_effects: openmt2001::demolition_effect::Manager::new(
                startup.state(0x940cf8),
            ),
            demolition_points: vec![],
            build_random_state: startup.state(0x560a08),
            window_texture: openmt2001::window_texture::WindowTexture::new(
                startup.state(openmt2001::window_texture::RNG_REFERENCE),
            ),
            pulse_lights: openmt2001::pulse_lights::PulseLights::from_original(input.image)?,
            allocation_next,
            static_rng_tail_next,
            event_cards: openmt2001::event_cards::Deck::new([
                startup.state(0x5607f0),
                0x343fd,
                0x269ec3,
            ]),
            population,
            history,
            road_traffic,
            vehicle_models: input.vehicle_models,
            transport_models: input.transport_models,
            transport,
            train_routes,
            road_routes: input.road_routes,
            live_commands: openmt2001::live_commands::Queue::default(),
            building_selection: [0; 2],
            saved_cameras: None,
            saved_player_metadata,
            saved_game_mode: None,
            script_directory: input.scripts.to_path_buf(),
            result_presentation: None,
        })
    }
    /// Tutorial EVENT_Level follows the native optional-roster stage.
    pub fn enter_tutorial_level(&mut self, assets: &CommandAssets<'_, '_>) -> Result<(), String> {
        if self.tutorial_state != 1 {
            return Ok(());
        }
        if self
            .catalog
            .selected_level()
            .is_some_and(|level| matches!(level.id(), 100001..=100012))
        {
            self.scripts.bind_tutorial_camera()?;
        }
        let context = self.world.script_context(
            assets.image,
            &self.resident_models,
            level_startup::HostContext {
                accounting_enabled: true,
                difficulty: self.difficulty as i32,
                notifications: self.world.city.ai_demand.ready != 0,
                auxiliary_template: self.policy.auxiliary_prior_bytes,
                category_base: 0x617628,
                building_base: 0x591318,
            },
        )?;
        let mut geometry = openmt2001::building_geometry::BuildingGeometry::new(
            assets.board,
            assets.file,
            std::mem::take(&mut self.models),
        );
        let result = (|| {
            for index in 0..geometry.registrar().models.len() {
                geometry.retain_model(
                    openmt2001::building_geometry::RegisteredModel::from_definition(
                        index as u32 + 1,
                        &geometry.registrar().models[index].definition,
                    )?,
                )?;
            }
            let players: Vec<_> = self
                .roster
                .manager
                .players
                .iter()
                .map(|player| {
                    Some(openmt2001::business_binding::Player {
                        identity: player.reference,
                        kind: u32::from_le_bytes(player.bytes[0x154..0x158].try_into().unwrap()),
                    })
                })
                .collect();
            self.scripts.enter_level(
                &mut self.world.script_state(&players),
                &context,
                &mut geometry,
                &mut self.hooks,
            )
        })();
        self.models = std::mem::take(geometry.registrar_mut());
        result
    }
}

/// Recorded UI work is retained for a later presenter, never treated as applied.
/// Non-city ownership, new-resident events and cash charges fail explicitly.
pub struct Hooks {
    process: Option<crate::live_host::process_runtime::Handle>,
    /// Only an audited original terminal forced-auction EVENT invocation.
    forced_auction_allowed: bool,
    forced_auction_handlers: std::collections::BTreeMap<String, Vec<u8>>,
    pub notifications: Vec<(u32, usize)>,
    pub commands: Vec<(level_script::Command, Vec<Value>)>,
    // Worker-side metadata uses the same four-record admission as presentation.
    script_prompts: std::cell::RefCell<(usize, crate::prompt::State)>,
    pub(crate) script_prompt_closes: Vec<usize>,
    pub utilities: economy::Utilities,
    pub frequency: openmt2001::timers::CalendarFrequency,
    pub timers: Vec<openmt2001::timers::Task>,
}
impl Hooks {
    pub(crate) fn script_prompt(&self) -> Option<crate::prompt::Request> {
        let mut metadata = self.script_prompts.borrow_mut();
        let (seen, state) = &mut *metadata;
        for (command, arguments) in &self.commands[*seen..] {
            if matches!(
                command,
                level_script::Command::Prompt | level_script::Command::TutorialMessage
            ) && let Ok(request) = crate::live_host::script_request(
                arguments,
                true,
                matches!(command, level_script::Command::Prompt),
            ) {
                state.submit(request);
            }
        }
        *seen = self.commands.len();
        state.request()
    }
    pub(crate) fn close_script_prompt(&mut self, notify_presenter: bool) {
        // Continuations enqueue their requests before the old head is freed.
        if self.script_prompt().is_none() {
            return;
        }
        self.script_prompts.borrow_mut().1.dismiss_with(|_, _| {});
        if notify_presenter {
            self.script_prompt_closes.push(self.commands.len());
        }
    }
    pub(crate) fn new() -> Self {
        Self::with_dispatch(
            openmt2001::timers::startup_tasks([0; 20]),
            openmt2001::timers::CalendarFrequency {
                frequency: 0.,
                calendar_timer: None,
                logic_timer: None,
            },
        )
    }
    fn with_dispatch(
        timers: Vec<openmt2001::timers::Task>,
        frequency: openmt2001::timers::CalendarFrequency,
    ) -> Self {
        Self {
            process: None,
            forced_auction_allowed: false,
            forced_auction_handlers: Default::default(),
            notifications: vec![],
            commands: vec![],
            script_prompts: Default::default(),
            script_prompt_closes: vec![],
            utilities: economy::Utilities::new([0; 4]),
            frequency,
            timers,
        }
    }
}
impl business_pool::BusinessPoolHooks for Hooks {
    fn owner_transfer(
        &mut self,
        slot: usize,
        record: &mut [u8; 0x220],
        owner: u32,
    ) -> Result<(), String> {
        if owner != 0 {
            return Err("fresh-city preview cannot transfer business to player".into());
        }
        record[0xa] = 0;
        self.notifications.push((0x462b00, slot));
        Ok(())
    }
    fn notify(&mut self) -> Result<(), String> {
        self.notifications.push((0x41f410, 0));
        Ok(())
    }
}
impl map_script::Hooks for Hooks {
    fn building_appearance(&mut self, slot: usize, _: &mut [u8; 0x394]) -> Result<(), String> {
        self.notifications.push((0x485520, slot));
        Ok(())
    }
    fn building_created(&mut self, slot: usize) -> Result<(), String> {
        self.notifications.push((0x471d50, slot));
        Ok(())
    }
    fn binding_call(
        &mut self,
        call: business_binding::Call,
        world: &mut business_binding::World<'_>,
    ) -> Result<(), String> {
        match call {
            business_binding::Call::ChargeUtilities { business } => {
                world.assess_binding_utilities(business, &mut self.utilities)?;
            }
            business_binding::Call::PayUtilityOwners => {
                self.utilities.pay_owners(&mut [], 1.)?;
            }
            business_binding::Call::RefreshBuilding { building } => {
                self.notifications.push((0x4450a0, building))
            }
            _ => {
                return Err(format!(
                    "fresh-city preview requires unimplemented binding call {call:?}"
                ));
            }
        }
        Ok(())
    }
    fn stock_charge(&mut self, _: &mut [u8; 0x220], _: f32) -> Result<(), String> {
        Err("fresh-city preview cannot charge a player account".into())
    }
}
impl level_script::Hooks for Hooks {
    fn performance8b26cc(&mut self) -> Result<i32, String> {
        self.process
            .as_ref()
            .ok_or("Game.performance process owner absent")?
            .performance8b26cc()
            .map_err(|e| e.to_string())
    }
    fn set_performance8b26cc(&mut self, value: i32) -> Result<(), String> {
        self.process
            .as_ref()
            .ok_or("Game.performance process owner absent")?
            .set_performance8b26cc(value)
            .map_err(|e| e.to_string())
    }
    fn pause_timers(&mut self, event: openmt2001::pause::Timers) -> Result<(), String> {
        event.apply(&mut self.timers);
        Ok(())
    }
    fn calendar_timers(
        &mut self,
    ) -> (
        &mut openmt2001::timers::CalendarFrequency,
        &mut [openmt2001::timers::Task],
    ) {
        (&mut self.frequency, &mut self.timers)
    }
    fn command(
        &mut self,
        command: level_script::Command,
        arguments: &[Value],
    ) -> Result<(), String> {
        if matches!(command, level_script::Command::ForcedAuction { .. })
            && !self.forced_auction_allowed
        {
            return Err(
                "forced auction requires an audited original terminal event handler".into(),
            );
        }
        self.commands.push((command, arguments.to_vec()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn simulation_test_policy() -> tick::Policy {
        use openmt2001::{hourly, scheduler};
        tick::Policy {
            scheduler: scheduler::Controls {
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
            hourly: hourly::Controls {
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
        }
    }
    fn exercise_ai_hourly(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{hourly, level_runtime::CalendarEvent};
        fn word(bytes: &[u8], at: usize) -> u32 {
            u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
        }
        fn put(bytes: &mut [u8], at: usize, value: u32) {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        if session.world.pause.has_reason() {
            session
                .dispatch_session_event("InterfacePromptDismiss", 0, &assets, false)
                .unwrap();
        }
        assert!(!session.world.pause.has_reason());
        let ai: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| word(&p.bytes, 0x154) == 1)
            .map(|(slot, _)| slot)
            .collect();
        assert!(!ai.is_empty(), "AI integration test must have opponents");
        // An expiring AI countdown must draw a real card and deliver its reply.
        let card_player = &mut session.roster.manager.players[ai[0]];
        let reference = word(&card_player.bytes, 0x21c);
        let card_aux = card_player
            .auxiliaries
            .iter_mut()
            .find(|a| a.reference == reference)
            .unwrap();
        put(&mut card_aux.bytes, 0, 0);
        let accepting = ai[1];
        let p = &mut session.roster.manager.players[accepting];
        let reference = word(&p.bytes, 0x21c);
        let aux = p
            .auxiliaries
            .iter_mut()
            .find(|a| a.reference == reference)
            .unwrap();
        put(&mut aux.bytes, 4, 65);
        put(&mut aux.bytes, 0x18, 1);
        let before: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .map(|p| {
                let aux = p
                    .auxiliaries
                    .iter()
                    .find(|a| a.reference == word(&p.bytes, 0x21c))
                    .unwrap();

                (word(&aux.bytes, 0), word(&aux.bytes, 0x18))
            })
            .collect();
        for &slot in &ai {
            let p = &mut session.roster.manager.players[slot];
            put(&mut p.bytes, 0x143c, 3);
            put(&mut p.bytes, 0x13e0, 24);
            // Isolate hourly delivery: the native normal-controller gate keeps
            // this test's message queued until the explicit inbox check below.
            p.bytes[0x1403] = 1;
        }
        // Position the retained scheduler immediately before a real hour edge.
        session.world.clock.timing[0] = 624;
        session.world.clock.timing[1] = 624;
        session
            .advance_session(&assets, &mut policy, |event, _| {
                if matches!(
                    event,
                    tick::Event::Ai(_)
                        | tick::Event::Calendar(CalendarEvent::Hourly(
                            hourly::Event::CurrentPlayerUpdate
                                | hourly::Event::PlayerUpdate(_)
                                | hourly::Event::Helper(0x470b80)
                        ))
                ) {
                    return Err("connected hourly AI operation escaped to host".into());
                }
                Ok(0)
            })
            .unwrap();
        assert_eq!(session.world.clock.timing[1], 625);
        for (slot, p) in session.roster.manager.players.iter().enumerate() {
            let aux = p
                .auxiliaries
                .iter()
                .find(|a| a.reference == word(&p.bytes, 0x21c))
                .unwrap();
            assert_eq!(
                word(&aux.bytes, 0),
                before[slot].0.wrapping_sub(1),
                "hourly event countdown slot {slot}"
            );
            assert_eq!(
                word(&aux.bytes, 0x18),
                if slot == accepting { 2 } else { before[slot].1 }
            );
        }
        for &slot in &ai {
            let p = &session.roster.manager.players[slot];
            assert_eq!(word(&p.bytes, 0x143c), 2);
            assert_eq!(
                word(&p.bytes, 0x13e0),
                if slot == accepting { 26 } else { 25 }
            );
            if slot == accepting {
                assert_eq!(word(&p.bytes, 0x1b8), 65);
            }
            assert_eq!(p.bytes[0x13dd], 1);
        }
        let card_player = &session.roster.manager.players[ai[0]];
        assert_eq!(word(&card_player.bytes, 0x214), 1);
        let packet = card_player
            .auxiliaries
            .iter()
            .find(|a| a.reference == word(&card_player.bytes, 0x218))
            .unwrap();
        assert_eq!(word(&packet.bytes, 4), 0x33);
        let card = word(&packet.bytes, 0x18) as usize;
        assert_eq!(session.event_cards.used[card], 1);
        assert_eq!(packet.bytes[0x15] as usize, ai[0]);
        assert_eq!(packet.bytes[0x14], 0xff);
        assert_eq!(
            word(&card_player.bytes, 0x1b8),
            0,
            "card remains queued until the AI controller consumes it"
        );
        // Exercise the actual normal-controller inbox tail while its original
        // negotiation gate postpones decision helpers. This is not an assertion
        // that autonomous construction/auction decisions are already connected.
        let slot = ai[0];
        let player = &mut session.roster.manager.players[slot];
        player.bytes[0x1403] = 0;
        player.bytes[0x1df8] = 1;
        put(&mut player.bytes, 8, 4);
        let tick = session.world.clock.timing[0];
        session
            .roster
            .advance_ai(
                &mut session.world.city,
                slot,
                image,
                &mut openmt2001::ai_tick::Controls {
                    normal_tick: tick,
                    day_tick: session.world.clock.timing[3],
                    event_gate: false,
                },
                &mut session.allocation_next,
                &session.hooks.utilities,
                &session.population.properties,
                None,
                |call, _, _| {
                    Err(format!(
                        "connected card/finance controller escaped: {call:?}"
                    ))
                },
            )
            .unwrap();
        let player = &session.roster.manager.players[slot];
        assert_eq!(word(&player.bytes, 0x214), 0);
        assert_eq!(word(&player.bytes, 0x1b8), card as u32);
        assert_eq!(word(&player.bytes, 8), 3);
        assert_eq!(
            word(&player.bytes, 0xc9c),
            session.world.ledgers.players[slot].cash.to_bits()
        );
        assert_eq!(
            u16::from_le_bytes(player.bytes[0xb4e..0xb50].try_into().unwrap()),
            tick.wrapping_add(250) as u16
        );
        let aux = player
            .auxiliaries
            .iter()
            .find(|a| a.reference == word(&player.bytes, 0x21c))
            .unwrap();
        assert_eq!(word(&aux.bytes, 4), card as u32);
        assert_eq!(word(&aux.bytes, 0x18), 1);
        println!(
            "Retained AI hour: {} opponents; countdowns, offer ages, expiry, card delivery and normal-controller consumption/finance advanced",
            ai.len()
        );
    }
    fn exercise_event_retirement(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        let word = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let put = |b: &mut [u8], at: usize, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let slots: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .enumerate()
            .filter(|(_, p)| word(&p.bytes, 0x154) == 1)
            .map(|(i, _)| i)
            .collect();
        assert!(slots.len() >= 2);
        for &slot in &slots {
            let p = &mut session.roster.manager.players[slot];
            p.bytes[0x1403] = 1;
            let reference = word(&p.bytes, 0x21c);
            let aux = p
                .auxiliaries
                .iter_mut()
                .find(|a| a.reference == reference)
                .unwrap();
            put(&mut aux.bytes, 0x18, 0);
        }
        for (slot, card, mode) in [(slots[0], 1, 1), (slots[1], 65, 2)] {
            let p = &mut session.roster.manager.players[slot];
            let reference = word(&p.bytes, 0x21c);
            openmt2001::event_cards::lifecycle::receive(p, reference, card).unwrap();
            openmt2001::event_cards::lifecycle::set_mode(p, reference, mode).unwrap();
            put(&mut p.bytes, 0x13e0, 49);
            p.bytes[0x13dd] = 1;
            p.bytes[0x13c1] = 0;
            session.event_cards.used[card as usize] = 1;
        }
        session.world.clock.calendar[0] = 8;
        session.world.clock.timing[0] = 1249;
        session.world.clock.timing[1] = 1249;
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        let mut accept =
            |event: tick::Event<'_>, _: &openmt2001::clock::Clock| -> Result<u32, String> {
                if matches!(event, tick::Event::Ai(_)) {
                    return Err("event retirement unexpectedly entered normal AI strategy".into());
                }
                Ok(0)
            };
        session
            .advance_session(&assets, &mut policy, &mut accept)
            .unwrap();
        assert_eq!(session.world.clock.timing[0], 1250);
        assert_eq!(
            session.event_cards.used[65], 0,
            "discard broadcast releases targeted card immediately"
        );
        assert_eq!(
            session.event_cards.used[1], 1,
            "financial command is still delayed"
        );
        for &slot in &slots[..2] {
            let p = &session.roster.manager.players[slot];
            let aux = p
                .auxiliaries
                .iter()
                .find(|a| a.reference == word(&p.bytes, 0x21c))
                .unwrap();
            assert_eq!(word(&aux.bytes, 0x18), 0);
            assert_eq!(word(&p.bytes, 0x1b8), 0);
            assert!((20..=28).contains(&word(&aux.bytes, 0)));
            assert_eq!(word(&p.bytes, 0x13e0), 0);
            assert_eq!(p.bytes[0x13dd], 0);
        }
        let ledger = session.world.ledgers.players[slots[0]]
            .ledger(0x24)
            .unwrap();
        while session.world.clock.timing[0] < 1275 {
            session
                .advance_session(&assets, &mut policy, &mut accept)
                .unwrap();
            if session.world.clock.timing[0] < 1275 {
                assert_eq!(session.event_cards.used[1], 1);
            }
        }
        assert_eq!(session.event_cards.used[1], 0);
        assert_eq!(
            session.world.ledgers.players[slots[0]]
                .ledger(0x24)
                .unwrap(),
            ledger + 200.
        );
        assert_eq!(
            word(&session.roster.manager.players[slots[0]].bytes, 4),
            session.world.ledgers.players[slots[0]].cash.to_bits()
        );
        println!(
            "Retained event lifecycle: targeted discard, financial retirement,25-tick delay and200-dollar ledger credit verified"
        );
    }
    fn exercise_ai_business_management(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        let word = |r: &[u8], at| u32::from_le_bytes(r[at..at + 4].try_into().unwrap());
        let put = |r: &mut [u8], at, v: u32| r[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let assets = CommandAssets { image, board, file };
        let slot = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        let business = session
            .world
            .ledgers
            .businesses
            .iter()
            .position(|b| b[0x21c] != 0 && word(b, 0) == 1)
            .unwrap();
        let old_business = session.world.ledgers.businesses[business];
        session.world.ledgers.businesses[business][10] = slot as u8 + 1;
        session.world.ledgers.businesses[business][16..18].copy_from_slice(&0u16.to_le_bytes());
        put(
            &mut session.world.ledgers.businesses[business],
            0x1dc,
            1000f32.to_bits(),
        );
        put(
            &mut session.world.ledgers.businesses[business],
            0x204,
            1000f32.to_bits(),
        );
        // Controlled dispatcher exercise: postpone strategy selection by its
        // original due WORD, while ownership and rent flags run normally.
        for p in &mut session.roster.manager.players {
            if word(&p.bytes, 0x154) == 1 {
                p.bytes[0x1403] = 1;
            }
        }
        let p = &mut session.roster.manager.players[slot];
        p.bytes[0x1400] = 0;
        p.bytes[0x1401] = 1;
        p.bytes[0x1403] = 0;
        p.bytes[0x1df8] = 0;
        p.bytes[0xb4c..0xb4e].copy_from_slice(&u16::MAX.to_le_bytes());
        p.bytes[0x239] = 0;
        p.bytes[0x23a] = 0;
        p.bytes[0x1e11] = 0;
        p.bytes[0x1e12] = 0;
        p.bytes[0xcc0] = 0;
        put(&mut p.bytes, 0xb64, u32::MAX);
        put(&mut p.bytes, 8, 100);
        put(&mut p.bytes, 0x220, 0x11);
        let mut policy = simulation_test_policy();
        let mut accept = |event: tick::Event<'_>, _: &openmt2001::clock::Clock| {
            if matches!(event, tick::Event::Ai(_)) {
                return Err(format!("AI management escaped {event:?}"));
            }
            Ok(0)
        };
        for _ in 0..5 {
            session
                .advance_session(&assets, &mut policy, &mut accept)
                .unwrap();
            if session.world.clock.timing[0].is_multiple_of(5) {
                break;
            }
        }
        let due = session.world.clock.timing[0] + 25;
        let pending = word(&session.world.ledgers.businesses[business], 0x204);
        assert_ne!(
            pending,
            1000f32.to_bits(),
            "flags {:x}, owned {:?}",
            word(&session.roster.manager.players[slot].bytes, 0x220),
            session
                .roster
                .ai_ownership
                .state(&session.roster.manager.players[slot])
                .unwrap()
                .lists[0]
        );
        assert_eq!(
            word(&session.world.ledgers.businesses[business], 0x1dc),
            1000f32.to_bits()
        );
        assert_eq!(session.world.ledgers.businesses[business][0x208], 0);
        assert_eq!(
            word(&session.roster.manager.players[slot].bytes, 0x220) & 0x11,
            0
        );
        while session.world.clock.timing[0] < due {
            session
                .advance_session(&assets, &mut policy, &mut accept)
                .unwrap();
        }
        assert_eq!(
            word(&session.world.ledgers.businesses[business], 0x1dc),
            pending
        );
        session.world.ledgers.businesses[business] = old_business;
        println!(
            "Retained AI rent: ownership refresh, pending rent, dirty clear and25-tick command application verified"
        );
    }
    fn exercise_business_sale(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{business_sale, live_commands};
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let seller = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        let business = session
            .world
            .ledgers
            .businesses
            .iter()
            .position(|b| b[0x21c] != 0 && word(b, 0) != 2)
            .unwrap();
        let building = i16::from_le_bytes(
            session.world.ledgers.businesses[business][8..10]
                .try_into()
                .unwrap(),
        ) as usize;
        let old_business = session.world.ledgers.businesses[business];
        let old_building = session.world.buildings[building];
        let old_accounts = session.world.ledgers.players.clone();
        let old_age = session.population.business_counts.age;
        let old_players: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.bytes.clone())
            .collect();
        for p in &mut session.roster.manager.players {
            if word(&p.bytes, 0x154) == 1 {
                p.bytes[0x1403] = 1;
            }
        }
        // Set up an owned business through the real accounting transfer before
        // testing the independently verified consumer through its live queue.
        {
            let city = &mut session.world.city;
            openmt2001::economy::transfer_business_owner(
                &mut city.ledgers.businesses[business],
                city.buildings[building][..64].try_into().unwrap(),
                seller as u8 + 1,
                true,
                &mut city.ledgers.players,
            )
            .unwrap();
        }
        session.world.buildings[building][0x2d] = seller as u8 + 1;
        session.world.buildings[building][0x3e] = 1;
        let credit = session.world.ledgers.players[seller].ledger(0x18).unwrap();
        let due = session.world.clock.timing[0] + 25;
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        let mut panels = 0;
        session
            .submit_command(
                live_commands::Command::SellBusiness(business_sale::Command {
                    business: business as u32,
                    player: seller as u8,
                }),
                &assets,
                |_| Err("sale applied before25ticks".into()),
            )
            .unwrap();
        while session.world.clock.timing[0] < due {
            session
                .advance_session(
                    &assets,
                    &mut policy,
                    |event: tick::Event<'_>, _: &openmt2001::clock::Clock| {
                        if matches!(event, tick::Event::Ai(_)) {
                            return Err(format!("sale escaped authoritative callbacks: {event:?}"));
                        }
                        if let tick::Event::Command(live_commands::Event::BusinessSale(
                            business_sale::Call::BuildingPanel(id),
                        )) = event
                        {
                            assert_eq!(id, building);
                            panels += 1;
                        }
                        Ok(0)
                    },
                )
                .unwrap();
            if session.world.clock.timing[0] < due {
                assert_eq!(
                    session.world.ledgers.businesses[business][10],
                    seller as u8 + 1
                );
                assert_eq!(
                    session.world.ledgers.players[seller].ledger(0x18).unwrap(),
                    credit
                );
            }
        }
        assert_eq!(session.world.ledgers.businesses[business][10], 0);
        assert_eq!(session.world.buildings[building][0x2d], 0);
        assert_eq!(session.world.buildings[building][0x3e], 0);
        assert_eq!(
            session.world.ledgers.businesses[business][0x21c],
            old_business[0x21c]
        );
        assert!(session.world.ledgers.players[seller].ledger(0x18).unwrap() > credit);
        assert_eq!(panels, 1);
        assert_eq!(
            word(&session.world.buildings[building], 0x1c),
            business as u32
        );
        session.world.ledgers.businesses[business] = old_business;
        session.world.buildings[building] = old_building;
        session.world.ledgers.players = old_accounts;
        session.population.business_counts.age = old_age;
        for (p, bytes) in session.roster.manager.players.iter_mut().zip(old_players) {
            p.bytes = bytes;
        }
        session
            .population
            .refresh_city(&session.world.city, board)
            .unwrap();
        println!(
            "Retained business sale:25-tick delay, ledger18 credit, city ownership, active business retention and panel-flag clear verified"
        );
    }
    fn exercise_business_takeover(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{business_takeover, live_commands};
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let put = |b: &mut [u8], at, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let slots: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .enumerate()
            .filter_map(|(i, p)| (word(&p.bytes, 0x154) == 1).then_some(i))
            .collect();
        let (buyer, seller) = (slots[0], slots[1]);
        let business = session
            .world
            .ledgers
            .businesses
            .iter()
            .position(|b| b[0x21c] != 0)
            .unwrap();
        let building = i16::from_le_bytes(
            session.world.ledgers.businesses[business][8..10]
                .try_into()
                .unwrap(),
        ) as usize;
        let category = ((word(&session.world.buildings[building], 0x40) - 0x617628) / 380) as usize;
        // Controlled ownership fixture; restore it after observing the actual
        // delayed consumer and the seller's scheduled2e inbox consequence.
        let old_business = session.world.ledgers.businesses[business];
        let old_building = session.world.buildings[building];
        let old_category = session.world.categories[category];
        let old_accounts = session.world.ledgers.players.clone();
        let old_lists = session.roster.ai_ownership.lists.clone();
        let old_age = session.population.business_counts.age;
        let old_players: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .map(|p| {
                (
                    p.bytes.clone(),
                    p.auxiliaries
                        .iter()
                        .map(|a| (a.reference, a.bytes))
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        for p in &mut session.roster.manager.players {
            if word(&p.bytes, 0x154) == 1 {
                p.bytes[0x1403] = 1;
            }
        }
        {
            let city = &mut session.world.city;
            openmt2001::economy::transfer_business_owner(
                &mut city.ledgers.businesses[business],
                city.buildings[building][..64].try_into().unwrap(),
                seller as u8 + 1,
                true,
                &mut city.ledgers.players,
            )
            .unwrap();
            city.categories[category][5] = buyer as u8 + 1;
            city.buildings[building][0x2d] = seller as u8 + 1;
            city.buildings[building][0x3e] = 1;
        }
        {
            let p = &mut session.roster.manager.players[seller];
            put(&mut p.bytes, 0x214, 0);
            put(&mut p.bytes, 0x218, 0);
            session
                .roster
                .ai_ownership
                .mutate(
                    p,
                    |size| {
                        let v = session.allocation_next;
                        session.allocation_next += size as u32;
                        Ok(v)
                    },
                    |state| state.insert(0, business as i32, |_| Ok(123)),
                )
                .unwrap();
        }
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        let due = session.world.clock.timing[0] + 25;
        let debit = session.world.ledgers.players[buyer].ledger(3).unwrap();
        let credit = session.world.ledgers.players[seller].ledger(0x16).unwrap();
        session
            .submit_command(
                live_commands::Command::Takeover(business_takeover::Command {
                    building: building as u32,
                }),
                &assets,
                |_| Err("takeover applied before25ticks".into()),
            )
            .unwrap();
        let mut accept = |event: tick::Event<'_>, _: &openmt2001::clock::Clock| {
            if matches!(event, tick::Event::Ai(_)) {
                return Err(format!(
                    "takeover escaped authoritative callbacks: {event:?}"
                ));
            }
            Ok(0)
        };
        while session.world.clock.timing[0] < due {
            session
                .advance_session(&assets, &mut policy, &mut accept)
                .unwrap();
            if session.world.clock.timing[0] < due {
                assert_eq!(
                    session.world.ledgers.businesses[business][10],
                    seller as u8 + 1
                );
            }
        }
        assert_eq!(
            session.world.ledgers.businesses[business][10],
            buyer as u8 + 1
        );
        assert_eq!(session.world.buildings[building][0x2d], buyer as u8 + 1);
        assert_eq!(session.world.buildings[building][0x3e], 0);
        let paid = debit - session.world.ledgers.players[buyer].ledger(3).unwrap();
        assert!(paid > 0.);
        assert_eq!(
            session.world.ledgers.players[seller].ledger(0x16).unwrap() - credit,
            paid
        );
        let p = &mut session.roster.manager.players[seller];
        assert_eq!(word(&p.bytes, 0x214), 1);
        let head = word(&p.bytes, 0x218);
        assert_eq!(
            word(
                &p.auxiliaries
                    .iter()
                    .find(|a| a.reference == head)
                    .unwrap()
                    .bytes,
                4
            ),
            0x2e
        );
        for at in [0x1400, 0x1403, 0x1df8, 0x239, 0x23a, 0x1e11, 0x1e12, 0xcc0] {
            p.bytes[at] = 0;
        }
        p.bytes[0x1401] = 1;
        p.bytes[0xb4c..0xb4e].copy_from_slice(&u16::MAX.to_le_bytes());
        put(&mut p.bytes, 8, 100);
        put(&mut p.bytes, 0x220, 0);
        put(&mut p.bytes, 0xb64, u32::MAX);
        for _ in 0..5 {
            session
                .advance_session(&assets, &mut policy, &mut accept)
                .unwrap();
            if word(&session.roster.manager.players[seller].bytes, 0x214) == 0 {
                break;
            }
        }
        assert_eq!(
            word(&session.roster.manager.players[seller].bytes, 0x214),
            0
        );
        assert!(
            !session
                .roster
                .ai_ownership
                .state(&session.roster.manager.players[seller])
                .unwrap()
                .lists[0]
                .entries
                .iter()
                .any(|e| e[0] != 0 && word(e, 4) == business as u32)
        );
        session.world.ledgers.businesses[business] = old_business;
        session.world.buildings[building] = old_building;
        session.world.categories[category] = old_category;
        session.world.ledgers.players = old_accounts;
        session.roster.ai_ownership.lists = old_lists;
        session.population.business_counts.age = old_age;
        for (p, (bytes, aux)) in session.roster.manager.players.iter_mut().zip(old_players) {
            p.bytes = bytes;
            p.auxiliaries = aux
                .into_iter()
                .map(|(reference, bytes)| openmt2001::player_manager::Auxiliary {
                    reference,
                    bytes,
                })
                .collect();
        }
        session
            .population
            .refresh_city(&session.world.city, board)
            .unwrap();
        println!(
            "Retained business takeover:25-tick delay, live price/payment, owner transfer and scheduled seller2e ownership-list removal verified"
        );
    }
    fn exercise_property_transfer(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::live_commands;
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let buyer = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        for p in &mut session.roster.manager.players {
            if word(&p.bytes, 0x154) == 1 {
                p.bytes[0x1403] = 1;
            }
        }
        let chains = openmt2001::building_pool::category_chains(
            &session.world.categories,
            &session.world.buildings,
            0x591318,
        )
        .unwrap();
        let category = (0..52)
            .find(|&c| session.world.categories[c][5] == 0 && !chains[c].is_empty())
            .unwrap();
        let businesses: Vec<_> = chains[category]
            .iter()
            .filter(|&&b| word(&session.world.buildings[b], 0x24) == 2)
            .map(|&b| word(&session.world.buildings[b], 0x1c) as usize)
            .collect();
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        let account = session.world.ledgers.players[buyer].clone();
        let lease = session.world.settings.lease_length() as u32;
        let mut notices = 0;
        let mut views = 0;
        for (owner, amount) in [(buyer as u8 + 1, 250), (0, 150)] {
            let old_owner = session.world.categories[category][5];
            let due = session.world.clock.timing[0] + 25;
            session
                .submit_command(
                    live_commands::Command::PropertyTransfer {
                        category: category as i16,
                        owner,
                        amount,
                    },
                    &assets,
                    |_| Err("property transfer ran before25ticks".into()),
                )
                .unwrap();
            while session.world.clock.timing[0] < due {
                session
                    .advance_session(
                        &assets,
                        &mut policy,
                        |event: tick::Event<'_>, _: &openmt2001::clock::Clock| {
                            match event {
                                tick::Event::Ai(call) => {
                                    return Err(format!(
                                        "controlled property transfer entered AI decision {call:?}"
                                    ));
                                }
                                tick::Event::Command(
                                    live_commands::Event::PropertyTransferNotice {
                                        category: c,
                                        owner: o,
                                        amount: a,
                                    },
                                ) => {
                                    assert_eq!((c, o, a), (category as i16, owner, amount));
                                    notices += 1;
                                }
                                tick::Event::Command(live_commands::Event::PropertyView(c)) => {
                                    assert_eq!(c, category as i16);
                                    views += 1;
                                }
                                _ => (),
                            }
                            Ok(0)
                        },
                    )
                    .unwrap();
                if session.world.clock.timing[0] < due {
                    assert_eq!(session.world.categories[category][5], old_owner);
                }
            }
            assert_eq!(session.world.categories[category][5], owner);
            assert_eq!(word(&session.world.categories[category], 0x7c), lease);
            assert_eq!(
                word(&session.world.categories[category], 0x78),
                if owner == 0 { 2 } else { 0 }
            );
            for &business in &businesses {
                assert_eq!(session.world.ledgers.businesses[business][10], owner);
            }
            assert_eq!(
                session.world.ledgers.players[buyer].owned_categories[category],
                u8::from(owner != 0)
            );
            assert_eq!(
                word(&session.roster.manager.players[buyer].bytes, 4),
                session.world.ledgers.players[buyer].cash.to_bits()
            );
            if owner != 0 {
                exercise_offer_refusal(session, category, buyer, &assets);
            }
        }
        assert_eq!(
            session.world.ledgers.players[buyer].ledger(0).unwrap(),
            account.ledger(0).unwrap() - 100.
        );
        assert_eq!(
            session.world.ledgers.players[buyer].housing_counter,
            account.housing_counter
        );
        assert_eq!((notices, views), (1, 2));
        println!(
            "Retained property roundtrip:25-tick purchase/city-sale, category ownership, ledger0, lease duration, AI ownership refresh and UI event order verified"
        );
    }
    fn exercise_offer_refusal(
        session: &mut Session<'_>,
        category: usize,
        seller: usize,
        assets: &CommandAssets<'_, '_>,
    ) {
        use openmt2001::{ai_actions::proposals, ai_messages::offers::execution};
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let buyer = session
            .roster
            .manager
            .players
            .iter()
            .enumerate()
            .position(|(i, p)| i != seller && word(&p.bytes, 0x154) == 1)
            .unwrap();
        for slot in [seller, buyer] {
            let p = &mut session.roster.manager.players[slot];
            for at in [0x1400, 0x1403, 0x1df8, 0x239, 0x23a, 0x1e11, 0x1e12, 0xcc0] {
                p.bytes[at] = 0;
            }
            p.bytes[0x1401] = 1;
            p.bytes[0xb4c..0xb4e].copy_from_slice(&u16::MAX.to_le_bytes());
            for (at, v) in [
                (0xb64, u32::MAX),
                (0x220, 0),
                (8, if slot == buyer { 0 } else { 100 }),
                (0xb6c, 0),
            ] {
                p.bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        // The seller has no willingness-to-sell history for this newly bought
        // category. Original47c5f0 must refuse; no external callback decides it.
        session.roster.manager.players[seller].bytes[0x1150 + category * 12] = 0;
        let reserved = word(&session.roster.manager.players[buyer].bytes, 0x1430);
        let value =
            execution::category_value(&session.world.categories[category], assets.image).unwrap();
        assert!(
            proposals::transaction(
                0x4784c0,
                &mut session.roster.manager.players[buyer],
                [category as u32, 0],
                [0; 104],
                &mut session.roster.proposal_identity,
                |call, _, _| match call {
                    proposals::Call::CategoryValue(c) if c == category as u32 => Ok(value),
                    other => Err(format!("unexpected proposal{other:?}")),
                }
            )
            .unwrap()
        );
        let mut policy = simulation_test_policy();
        let mut ticks = 0;
        while session.roster.manager.players[buyer].bytes[0x1404] != 0 {
            session.advance_session(assets,&mut policy,|event:tick::Event<'_>,_:&openmt2001::clock::Clock|{
                if matches!(event,tick::Event::Ai(openmt2001::ai_runtime::Call::MessageRoute{route:openmt2001::player_messages::Route::PropertyRefresh{category:c},..}) if c==category as u32){return Ok(0);}
                if matches!(event,tick::Event::Ai(_)){return Err(format!("offer refusal escaped{event:?}"));}Ok(0)
            }).unwrap();
            ticks += 1;
            assert!(ticks <= 25, "live offer refusal did not finish");
        }
        let p = &session.roster.manager.players[buyer];
        assert_eq!(
            word(&p.bytes, 0x1430),
            reserved,
            "refusal releases the original reservation"
        );
        assert_eq!(p.bytes[0x1406], 0, "refusal removes the pending category");
        let history = 0x1438 + (category + 52 * seller) * 8;
        assert_eq!(word(&p.bytes, history), value.to_f32().to_bits());
        assert!(
            word(&p.bytes, history + 4) >= 20,
            "refusal records actual profile/RNG history age"
        );
        assert_eq!(session.world.categories[category][5], seller as u8 + 1);
        for slot in [seller, buyer] {
            session.roster.manager.players[slot].bytes[0x1403] = 1;
        }
        println!(
            "Retained negotiation: native purchase proposal,23 request,47c5f0 refusal,26 peer reply, reservation/pending release and price history verified"
        );
    }
    fn exercise_live_auction(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{ai_messages::offers::auction, live_commands};
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        // Isolate auction transactions from strategic choices; inbox production
        // remains live and is asserted below. Presentation calls are named seams.
        for p in &mut session.roster.manager.players {
            if word(&p.bytes, 0x154) == 1 {
                p.bytes[0x1403] = 1;
            }
        }
        let category = (0..52)
            .find(|&i| session.world.categories[i][5] == 0)
            .unwrap();
        let bidder = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        let initiator = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == session.roster.status.current_player)
            .unwrap();
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        let mut descriptor = [0; 52];
        for (off, v) in [
            (4, 0x1d),
            (20, category as u32),
            (24, initiator as u32),
            (28, 100),
        ] {
            descriptor[off..off + 4].copy_from_slice(&v.to_le_bytes());
        }
        descriptor[16] = 1;
        let admitted_at = session.world.clock.timing[0] + 25;
        session
            .submit_command(
                live_commands::Command::Auction { descriptor },
                &assets,
                |_| Err("auction admission ran early".into()),
            )
            .unwrap();
        let mut scene_prepares = 0;
        let mut boundary =
            |event: tick::Event<'_>, _: &openmt2001::clock::Clock| -> Result<u32, String> {
                match event {
                    tick::Event::Ai(openmt2001::ai_runtime::Call::AuctionAction(_)) => (),
                    tick::Event::Ai(call) => {
                        return Err(format!("controlled auction entered strategy{call:?}"));
                    }
                    tick::Event::Command(live_commands::Event::AuctionScenePrepare) => {
                        scene_prepares += 1;
                        return Ok(1);
                    }
                    tick::Event::Command(live_commands::Event::AuctionActivation(
                        auction::activation::Call::Camera,
                    )) => return Ok(1),
                    // Named native visual/audio boundaries are intentionally not
                    // evidence of a rendered auction room in this simulation test.
                    tick::Event::Command(
                        live_commands::Event::AuctionAdmission(_)
                        | live_commands::Event::AuctionActivation(_)
                        | live_commands::Event::AuctionAction(_)
                        | live_commands::Event::AuctionClosure(_),
                    ) => (),
                    _ => (),
                }
                Ok(0)
            };
        let mut announcement_host = crate::live_host::Host::new([0.; 3]);
        announcement_host.set_running(true);
        while session.world.clock.timing[0] < admitted_at {
            announcement_host.tick(session, &assets);
            assert!(
                announcement_host.failure.is_none(),
                "real host admission failed: {:?}",
                announcement_host.failure
            );
        }
        assert_eq!(session.world.auction.controller.active_count, 1);
        assert_eq!(session.world.auction.controller.record[0x7d], 1);
        assert_eq!(session.world.auction.presentation.countdown, 6000);
        assert!(session.world.auction.presentation.visible);
        assert_eq!(session.world.categories[category][0x6c], 1);
        let status = announcement_host.status(session);
        let notice = status.auction_notice.unwrap();
        assert_eq!(
            (notice.category, notice.amount, notice.seconds),
            (category as u32, 100, None)
        );
        assert_eq!(
            notice.character,
            word(&session.roster.manager.players[initiator].bytes, 0x150)
        );
        assert_eq!(
            notice.actor.as_slice(),
            &session.roster.manager.players[initiator].bytes[0x158..0x1b8]
        );
        assert!(status.auction_locked_categories.contains(&category));
        // Exercise the real scheduled controller while the announcement is
        // active. Original1040=-1 postpones continuation actions, so this tests
        // valuation without pretending the remaining strategy is complete.
        let prior_category = word(&session.roster.manager.players[bidder].bytes, 0xb68);
        {
            let p = &mut session.roster.manager.players[bidder];
            for at in [
                0x1400, 0x1403, 0x1df8, 0x239, 0x23a, 0x1e11, 0x1e12, 0xcc0, 0x13c5, 0xb86,
            ] {
                p.bytes[at] = 0;
            }
            p.bytes[0x1401] = 1;
            p.bytes[0xb4c..0xb4e].fill(0);
            for (at, v) in [
                (0xb64, u32::MAX),
                (0x1040, u32::MAX),
                (0x240, 7),
                (0x220, 0),
                (8, 100),
                (0x1c4, 0xfeed),
            ] {
                p.bytes[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
            p.bytes[0x1c8..0x20c].fill(0xff);
        }
        let mut valuation_ticks = 0;
        while session.roster.manager.players[bidder].bytes[0x13c5] == 0 {
            session
                .advance_session(&assets, &mut policy, &mut boundary)
                .unwrap();
            valuation_ticks += 1;
            assert!(
                valuation_ticks <= 5,
                "scheduled auction valuation did not run"
            );
        }
        let p = &mut session.roster.manager.players[bidder];
        assert_eq!(
            word(&p.bytes, 0xb68),
            prior_category,
            "auction predecision restores the prior target"
        );
        assert_eq!(
            word(&p.bytes, 0x1c4),
            0,
            "valuation ran the actual poll lifecycle"
        );
        assert!(p.bytes[0x1c8..0x20c].iter().all(|&v| v == 0));
        assert_eq!(p.bytes[0x13c4], 1);
        assert_eq!(p.bytes[0x13c5], 1);
        p.bytes[0x1403] = 1;
        let valued_at = session.world.clock.timing[0];
        announcement_host.set_running(false);
        let mut displayed = Vec::new();
        for _ in 0..6000 {
            announcement_host.frame(session, &assets, 20);
            let status = announcement_host.status(session);
            assert!(
                status.failure.is_none(),
                "real host countdown failed: {:?}",
                status.failure
            );
            if let Some(seconds) = status.auction_seconds
                && displayed.last() != Some(&seconds)
            {
                displayed.push(seconds);
            }
        }
        assert_eq!(
            session.world.clock.timing[0], valued_at,
            "host pause keeps25Hz accumulator frozen while50Hz countdown advances"
        );
        assert_eq!(displayed.len(), 120);
        assert_eq!(displayed.first(), Some(&119));
        assert_eq!(displayed.last(), Some(&0));
        // Advance to the scheduled boundary explicitly; this verifies the queue
        // delay without claiming6000 UI calls established host elapsed time.
        let start = admitted_at + 3025;
        session.world.clock.timing[0] = start - 2;
        session
            .advance_session(&assets, &mut policy, &mut boundary)
            .unwrap();
        assert_eq!(session.world.clock.timing[5], 1);
        assert_eq!(session.world.auction.controller.record[0x7d], 1);
        session
            .advance_session(&assets, &mut policy, &mut boundary)
            .unwrap();
        assert_eq!(session.world.clock.timing[5], 2);
        assert!(!session.world.auction.presentation.visible);
        assert_eq!(word(&session.world.auction.controller.record, 0x74), 1);
        let normal = session.world.clock.timing[1];
        session
            .advance_session(&assets, &mut policy, &mut boundary)
            .unwrap();
        assert_eq!(word(&session.world.auction.controller.record, 0x74), 2);
        // Consume the real27 inbox and let the retained urgent offer budget,
        // wait and submit its own bid. Other participants withdraw through the
        // same immediate command path to isolate this bidder's transaction.
        {
            let p = &mut session.roster.manager.players[bidder];
            p.bytes[0x1403] = 0;
            p.bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
            p.bytes[0xb4c..0xb4e].copy_from_slice(&u16::MAX.to_le_bytes());
        }
        let mut inbox_ticks = 0;
        while session.roster.manager.players[bidder].bytes[0x1405] == 0 {
            session
                .advance_session(&assets, &mut policy, &mut boundary)
                .unwrap();
            inbox_ticks += 1;
            assert!(inbox_ticks <= 10, "auction start inbox not consumed");
        }
        assert!(
            session.roster.manager.players[bidder].bytes[0x1405] > 0,
            "auction activation inserts a retained urgent offer"
        );
        for _ in 0..6 {
            let participant = word(&session.world.auction.controller.record, 0x58);
            if participant == bidder as u32 {
                break;
            }
            session
                .apply_auction_action(
                    auction::Command {
                        action: 2,
                        participant,
                        amount: 0,
                    },
                    &assets,
                    |_| Ok(0),
                )
                .unwrap();
        }
        let winner = bidder as u32;
        assert_eq!(word(&session.world.auction.controller.record, 0x58), winner);
        let ledger = session.world.ledgers.players[bidder].ledger(0).unwrap();
        let mut bid_ticks = 0;
        let opening_price = word(&session.world.auction.controller.record, 0x6c);
        while word(&session.world.auction.controller.record, 0x6c) == opening_price {
            session
                .advance_session(&assets, &mut policy, &mut boundary)
                .unwrap();
            bid_ticks += 1;
            assert!(
                bid_ticks <= 160,
                "retained AI offer did not submit its bid: player={:?}, auction={:?}",
                [
                    8,
                    0xcb8,
                    0xcc4,
                    0xcc8,
                    0x1dfc,
                    0x1e00,
                    0x1e38,
                    0x658 + 8,
                    0x658 + 16
                ]
                .map(|i| (i, word(&session.roster.manager.players[bidder].bytes, i))),
                [0x58, 0x60, 0x68, 0x6c, 0x74]
                    .map(|i| (i, word(&session.world.auction.controller.record, i)))
            );
        }
        let winning_price = word(&session.world.auction.controller.record, 0x6c) as i32;
        assert!(winning_price > 100);
        session.roster.manager.players[bidder].bytes[0x1403] = 1;
        for _ in 0..6 {
            if word(&session.world.auction.controller.record, 0x60) != u32::MAX {
                break;
            }
            let participant = word(&session.world.auction.controller.record, 0x58);
            assert_ne!(participant, winner);
            session
                .apply_auction_action(
                    auction::Command {
                        action: 2,
                        participant,
                        amount: 0,
                    },
                    &assets,
                    |_| Ok(0),
                )
                .unwrap();
        }
        assert_eq!(word(&session.world.auction.controller.record, 0x60), winner);
        let mut steps = 0;
        while session.world.auction.controller.active_count != 0 {
            session
                .advance_session(&assets, &mut policy, &mut boundary)
                .unwrap();
            steps += 1;
            assert!(steps < 200);
            if session.world.clock.timing[5] == 2 {
                assert_eq!(session.world.clock.timing[1], normal);
            }
        }
        assert_eq!(session.world.clock.timing[5], 1);
        assert_eq!(session.world.auction.controller.record[0x7c], 0);
        assert_eq!(session.world.categories[category][0x6c], 0);
        assert_eq!(word(&session.world.auction.controller.record, 0x60), winner);
        assert_ne!(session.world.auction.controller.reference, 0);
        assert_eq!(session.world.categories[category][5], 0);
        let transfer = session.world.clock.timing[0] + 25;
        while session.world.clock.timing[0] < transfer {
            session
                .advance_session(&assets, &mut policy, &mut boundary)
                .unwrap();
            if session.world.clock.timing[0] < transfer {
                assert_eq!(session.world.categories[category][5], 0);
            }
        }
        assert_eq!(session.world.categories[category][5], winner as u8 + 1);
        assert_eq!(
            session.world.ledgers.players[winner as usize]
                .ledger(0)
                .unwrap(),
            ledger - winning_price as f32
        );
        let queued: Vec<u32> = session
            .roster
            .manager
            .players
            .iter()
            .flat_map(|p| p.auxiliaries.iter().map(|a| word(&a.bytes, 4)))
            .collect();
        for kind in [0x27, 0x28, 0x29, 0x2a] {
            assert!(
                queued.contains(&kind),
                "missing retained auction message{kind:x}"
            );
        }
        // Consume winner29 and completion28 through the same scheduled AI;
        // this releases the urgent offer and schedules the real category19.
        session.roster.manager.players[bidder].bytes[0x1403] = 0;
        let mut completion_ticks = 0;
        while session.roster.manager.players[bidder].bytes[0x1df8] != 0 {
            session
                .advance_session(&assets, &mut policy, &mut boundary)
                .unwrap();
            completion_ticks += 1;
            assert!(
                completion_ticks <= 20,
                "auction result inbox did not complete"
            );
        }
        let p = &mut session.roster.manager.players[bidder];
        p.bytes[0x1403] = 1;
        assert_eq!(
            p.bytes[0x1405], 0,
            "completion cancels finished urgent offers"
        );
        assert_eq!(
            p.bytes[0x1150 + category * 12],
            1,
            "winner history comes from result29"
        );
        let wait_at = session.world.clock.timing[0] + 25;
        while session.world.clock.timing[0] < wait_at {
            session
                .advance_session(&assets, &mut policy, &mut boundary)
                .unwrap();
        }
        assert_eq!(word(&session.world.categories[category], 0x78), 1);
        assert_eq!(word(&session.world.categories[category], 0x80), 24);
        // A controlled normal-clock jump verifies retained expiry routing;
        // the native lifecycle oracle separately checks the exact15000 boundary.
        session.world.clock.timing[1] = session.world.clock.timing[1].wrapping_add(15000);
        session
            .advance_session(&assets, &mut policy, &mut boundary)
            .unwrap();
        assert_eq!(word(&session.world.categories[category], 0x78), 2);
        assert_eq!(scene_prepares, 1);
        println!(
            "Retained auction: valuation, actual27 inbox/AI bid,3025-tick start,120 announcement seconds,151+25-tick settlement,25-tick ownership transfer, winner29/completion28 and normal-clock category expiry verified; camera/audio remain named seams"
        );
    }
    fn exercise_business_conversion(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{business_conversion, live_commands};
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let put = |b: &mut [u8], at, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let buyer = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        for p in &mut session.roster.manager.players {
            if word(&p.bytes, 0x154) == 1 {
                p.bytes[0x1403] = 1;
            }
        }
        //4073a0 scans types42/43 for structure1, excluding the current type;
        //463bb0 checks the actual scripted catalog and retained geometry.
        //407970 separately disables this apartment's interactive conversion;
        //this fixture tests the queued consumer, not the UI admission gate.
        let (old, building, kind) = session
            .world
            .ledgers
            .businesses
            .iter()
            .enumerate()
            .find_map(|(i, b)| {
                if b[0x21c] == 0 || word(b, 0) != 1 {
                    return None;
                }
                let building = i16::from_le_bytes(b[8..10].try_into().unwrap()) as usize;
                let row = &session.world.buildings[building];
                let kind = 43u32;
                (word(row, 0x24) == 1
                    && word(b, 4) != kind
                    && business_conversion::fits(
                        row,
                        session.world.catalog.info.row(kind as usize).unwrap(),
                    )
                    .unwrap())
                .then_some((i, building, kind))
            })
            .expect(
                "fresh city has an apartment that satisfies original office-conversion geometry",
            );
        let p = &mut session.roster.manager.players[buyer];
        put(&mut p.bytes, 0x214, 0);
        put(&mut p.bytes, 0x218, 0);
        let quote = business_conversion::quote(
            building as u32,
            kind,
            &session.world.categories,
            &session.world.buildings,
            &session.world.catalog.info,
            image,
        )
        .unwrap()
        .to_f32();
        let prior = session.world.ledgers.players[buyer].ledger(0x14).unwrap();
        let age = session.population.business_counts.age;
        let due = session.world.clock.timing[0] + 25;
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        session
            .submit_command(
                live_commands::Command::Convert(business_conversion::Command {
                    kind,
                    building: building as u32,
                    owner: buyer as u32 + 1,
                }),
                &assets,
                |_| Err("conversion applied before25ticks".into()),
            )
            .unwrap();
        let mut accept = |event: tick::Event<'_>, _: &openmt2001::clock::Clock| {
            match event {
                tick::Event::Ai(_) => {
                    return Err(format!(
                        "conversion escaped authoritative callbacks: {event:?}"
                    ));
                }
                tick::Event::Command(
                    live_commands::Event::AccountPosted(_)
                    | live_commands::Event::RefreshInterface
                    | live_commands::Event::ConstructionNotice { .. }
                    | live_commands::Event::Business(
                        openmt2001::business_binding::Call::RefreshBuilding { .. },
                    )
                    | live_commands::Event::Detach(
                        openmt2001::business_detach::Call::RefreshBuilding { .. },
                    ),
                ) => (),
                tick::Event::Command(other) => {
                    return Err(format!("conversion unresolved command callback: {other:?}"));
                }
                _ => (),
            }
            Ok(0)
        };
        while session.world.clock.timing[0] < due {
            session
                .advance_session(&assets, &mut policy, &mut accept)
                .unwrap();
            if session.world.clock.timing[0] < due {
                assert_eq!(word(&session.world.buildings[building], 0x1c), old as u32);
                assert_eq!(
                    session.world.ledgers.players[buyer].ledger(0x14).unwrap(),
                    prior
                );
            }
        }
        let new = word(&session.world.buildings[building], 0x1c) as usize;
        assert_ne!(new, old);
        assert_eq!(session.world.ledgers.businesses[old][0x21c], 0);
        assert_ne!(session.world.ledgers.businesses[new][0x21c], 0);
        assert_eq!(word(&session.world.ledgers.businesses[new], 4), kind);
        assert_eq!(session.world.ledgers.businesses[new][10], buyer as u8 + 1);
        assert_eq!(session.world.buildings[building][0x3d], 0);
        assert_eq!(
            session.world.ledgers.players[buyer].ledger(0x14).unwrap(),
            prior - quote
        );
        assert_eq!(
            session.population.business_counts.age,
            age.wrapping_add(100)
        );
        let p = &mut session.roster.manager.players[buyer];
        assert_eq!(word(&p.bytes, 0x214), 1);
        let head = word(&p.bytes, 0x218);
        assert_eq!(
            word(
                &p.auxiliaries
                    .iter()
                    .find(|a| a.reference == head)
                    .unwrap()
                    .bytes,
                4
            ),
            0x2f
        );
        for at in [0x1400, 0x1403, 0x1df8, 0x239, 0x23a, 0x1e11, 0x1e12, 0xcc0] {
            p.bytes[at] = 0;
        }
        p.bytes[0x1401] = 1;
        p.bytes[0xb4c..0xb4e].copy_from_slice(&u16::MAX.to_le_bytes());
        put(&mut p.bytes, 8, 100);
        put(&mut p.bytes, 0x220, 0);
        put(&mut p.bytes, 0xb64, u32::MAX);
        for _ in 0..5 {
            session
                .advance_session(&assets, &mut policy, &mut accept)
                .unwrap();
            if word(&session.roster.manager.players[buyer].bytes, 0x214) == 0 {
                break;
            }
        }
        assert_eq!(word(&session.roster.manager.players[buyer].bytes, 0x214), 0);
        assert!(
            session
                .roster
                .ai_ownership
                .state(&session.roster.manager.players[buyer])
                .unwrap()
                .lists[0]
                .entries
                .iter()
                .any(|e| e[0] != 0 && word(e, 4) == new as u32)
        );
        session.roster.manager.players[buyer].bytes[0x1403] = 1;
        println!(
            "Retained conversion consumer: fit-valid apartment-to-office change,25-tick delay, ledger14 charge, actual allocation/detach/rebind and scheduled buyer2f ownership insertion verified; UI admission is separate"
        );
    }
    fn exercise_human_replies(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{human_messages, live_commands, player_messages};
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let put = |b: &mut [u8], at, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let local = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == session.roster.status.current_player)
            .unwrap();
        let buyer = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        for p in &mut session.roster.manager.players {
            if word(&p.bytes, 0x154) == 1 {
                p.bytes[0x1403] = 1;
            }
        }
        for at in [0x214, 0x218, 0x220] {
            put(&mut session.roster.manager.players[local].bytes, at, 0);
        }
        let category = (0..52)
            .find(|&i| session.world.categories[i][5] == 0)
            .unwrap();
        // Controlled category-owner fixtures isolate response revalidation;
        // actual property transfers and settlements are exercised separately.
        session.world.categories[category][5] = local as u8 + 1;
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        fn boundary(event: tick::Event<'_>, _: &openmt2001::clock::Clock) -> Result<u32, String> {
            match event {
                tick::Event::Ai(_) => Err(format!("human reply escaped AI callback:{event:?}")),
                tick::Event::HumanDialog {
                    call: human_messages::Call::Diagnostic,
                    ..
                } => Err("human reply reached native diagnostic".into()),
                tick::Event::Command(live_commands::Event::AuctionAdmission(
                    openmt2001::ai_messages::offers::auction::admission::Call::ShowAnnouncement
                    | openmt2001::ai_messages::offers::auction::admission::Call::CategoryStatus(_),
                )) => Ok(0),
                tick::Event::Command(other) => {
                    Err(format!("human reply unresolved command callback:{other:?}"))
                }
                _ => Ok(0),
            }
        }
        fn deliver(
            session: &mut Session<'_>,
            assets: &CommandAssets<'_, '_>,
            policy: &mut tick::Policy,
            local: usize,
            args: [i32; 8],
        ) -> (u32, u32) {
            let mut messages = player_messages::Context {
                arena: player_messages::Arena {
                    next: session.allocation_next,
                },
                current_player: session.roster.status.current_player,
                network: [false, false],
                prior: [0; 48],
            };
            messages
                .produce(args, &mut session.roster.manager, |route, _| {
                    Err(format!("human message unexpected external route:{route:?}"))
                })
                .unwrap();
            session.allocation_next = messages.arena.next;
            session.roster.manager.players[local].bytes[8..12].copy_from_slice(&0u32.to_le_bytes());
            let mut dialog = (0, 0);
            for _ in 0..5 {
                session
                    .advance_session(
                        assets,
                        policy,
                        |event: tick::Event<'_>, clock: &openmt2001::clock::Clock| {
                            match &event {
                                tick::Event::HumanDialog {
                                    call:
                                        human_messages::Call::Ui {
                                            address: 0x43be20, ..
                                        },
                                    ..
                                } => dialog = (0, 0),
                                tick::Event::HumanDialog {
                                    call: human_messages::Call::Handler { callback, context },
                                    ..
                                } => {
                                    dialog.0 = *callback;
                                    if let Some(context) = context {
                                        dialog.1 = *context;
                                    }
                                }
                                _ => (),
                            }
                            boundary(event, clock)
                        },
                    )
                    .unwrap();
                if u32::from_le_bytes(
                    session.roster.manager.players[local].bytes[0x220..0x224]
                        .try_into()
                        .unwrap(),
                ) == 1
                {
                    break;
                }
            }
            assert_eq!(
                u32::from_le_bytes(
                    session.roster.manager.players[local].bytes[0x220..0x224]
                        .try_into()
                        .unwrap()
                ),
                1
            );
            dialog
        }
        let offer = [
            local as i32,
            buyer as i32,
            0x23,
            category as i32,
            12500,
            73,
            91,
            -999,
        ];
        session.world.categories[category][0x6c] = 1;
        let dialog = deliver(session, &assets, &mut policy, local, offer);
        assert_eq!(dialog, (0x48b350, 0x23));
        let queued = word(&session.roster.manager.players[buyer].bytes, 0x214);
        let mut views = 0;
        session
            .respond_human_dialog(0x48b350, 1, 0x23, [false, false], |event| match event {
                live_commands::Event::PlayerMessageRoute {
                    route: player_messages::Route::PropertyRefresh { category: c },
                    ..
                } => {
                    assert_eq!(c, category as u32);
                    views += 1;
                    Ok(())
                }
                other => Err(format!("refusal external callback:{other:?}")),
            })
            .unwrap();
        assert_eq!(views, 1);
        assert_eq!(session.world.categories[category][0x6c], 0);
        assert_eq!(
            word(&session.roster.manager.players[buyer].bytes, 0x214),
            queued + 1
        );
        assert_eq!(word(&session.roster.manager.players[local].bytes, 0x220), 0);
        deliver(session, &assets, &mut policy, local, offer);
        session.world.categories[category][5] = buyer as u8 + 1;
        session
            .respond_human_dialog(0x48b350, 0, 0x23, [false, false], |_| {
                Err("stale offer emitted a response".into())
            })
            .unwrap();
        assert_eq!(word(&session.roster.manager.players[local].bytes, 0x220), 0);
        let until = session.world.clock.timing[0] + 25;
        while session.world.clock.timing[0] < until {
            session
                .advance_session(&assets, &mut policy, boundary)
                .unwrap();
        }
        assert_eq!(
            session.world.auction.controller.active_count, 0,
            "stale owner must not queue an auction"
        );
        let building = session
            .world
            .buildings
            .iter()
            .position(|b| b[0x3f] != 0 && word(b, 0x1c) != u32::MAX)
            .unwrap();
        let notice = deliver(
            session,
            &assets,
            &mut policy,
            local,
            [
                local as i32,
                buyer as i32,
                0x2e,
                building as i32,
                0,
                0,
                0,
                -999,
            ],
        );
        assert_eq!(notice, (0x48b480, 0));
        session
            .respond_human_dialog(notice.0, 0, notice.1, [false, false], |_| {
                Err("notice dismissal emitted a command".into())
            })
            .unwrap();
        assert_eq!(word(&session.roster.manager.players[local].bytes, 0x220), 0);
        session.world.categories[category][5] = local as u8 + 1;
        deliver(session, &assets, &mut policy, local, offer);
        let due = session.world.clock.timing[0] + 25;
        session
            .respond_human_dialog(0x48b350, 0, 0x23, [false, false], |_| {
                Err("accepted offer unexpectedly routed externally".into())
            })
            .unwrap();
        assert_eq!(word(&session.roster.manager.players[local].bytes, 0x220), 0);
        while session.world.clock.timing[0] < due {
            session
                .advance_session(&assets, &mut policy, boundary)
                .unwrap();
            if session.world.clock.timing[0] < due {
                assert_eq!(session.world.auction.controller.active_count, 0);
            }
        }
        assert_eq!(session.world.auction.controller.active_count, 1);
        assert_eq!(
            word(&session.world.auction.controller.record, 4),
            category as u32
        );
        assert_eq!(word(&session.world.auction.controller.record, 0x68), 12500);
        assert!(session.world.auction.presentation.visible);
        println!(
            "Retained human replies: real inbox/dialog handoff, refusal26/unlock, stale-owner rejection, takeover-notice dismissal and accepted25-tick auction admission verified"
        );
    }
    fn exercise_lease_expiry(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let put = |b: &mut [u8], at, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let local = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == session.roster.status.current_player)
            .unwrap();
        let category = word(&session.world.auction.controller.record, 4) as usize;
        assert_eq!(session.world.auction.controller.active_count, 1);
        assert_eq!(session.world.categories[category][5], local as u8 + 1);
        // Controlled remaining lease values isolate the actual hourly boundary.
        // The accepted auction from the preceding reply exercise stays live.
        for row in &mut session.world.categories {
            put(row, 0x7c, 0);
        }
        put(&mut session.world.categories[category], 0x7c, 25);
        put(&mut session.world.categories[category], 0x80, 25);
        session.world.ledgers.players[local].owned_categories[category] = 1;
        let assets = CommandAssets { image, board, file };
        let mut host = crate::live_host::Host::new([0.; 3]);
        host.set_running(true);
        session.world.clock.calendar[0] = 11;
        session.world.clock.timing[1] = (session.world.clock.timing[1] / 625 + 1) * 625 - 1;
        host.policy.scheduler.iterations = 3;
        host.tick(session, &assets);
        assert!(
            host.failure.is_none(),
            "lease warning Host failed:{:?}",
            host.failure
        );
        assert_eq!(
            host.policy.scheduler.iterations, 1,
            "original modal restores normal speed"
        );
        let warning = host
            .take_prompts(session)
            .into_iter()
            .filter_map(|p| p.dialog)
            .find(|d| d.body == 0x6c0)
            .expect("24-hour lease warning modal");
        assert_eq!(
            (warning.title, warning.button_count, warning.buttons[0]),
            (0x6bf, 1, 0x59b)
        );
        assert_eq!(word(&session.world.categories[category], 0x7c), 24);
        assert_eq!(session.world.auction.controller.active_count, 1);
        put(&mut session.world.categories[category], 0x7c, 1);
        put(&mut session.world.categories[category], 0x80, 1);
        let cash = session.world.ledgers.players[local].cash;
        let age = session.world.ownership_refresh_counter;
        session.world.clock.timing[1] = (session.world.clock.timing[1] / 625 + 1) * 625 - 1;
        host.tick(session, &assets);
        assert!(
            host.failure.is_none(),
            "lease expiry Host failed:{:?}",
            host.failure
        );
        let expired = host
            .take_prompts(session)
            .into_iter()
            .filter_map(|p| p.dialog)
            .find(|d| d.body == 0x6c1)
            .expect("lease expiry modal");
        assert_eq!(expired.title, 0x6bf);
        assert_eq!(session.world.categories[category][5], 0);
        assert_eq!(word(&session.world.categories[category], 0x78), 2);
        assert_eq!(
            session.world.ledgers.players[local].owned_categories[category],
            0
        );
        assert_eq!(session.world.ledgers.players[local].cash, cash);
        assert_eq!(
            session.world.ownership_refresh_counter,
            age.wrapping_add(100)
        );
        assert_eq!(session.world.auction.controller.active_count, 0);
        assert_eq!(session.world.auction.controller.record[0x7d], 0);
        assert!(!session.world.auction.presentation.visible);
        assert_eq!(session.world.categories[category][0x6c], 0);
        for player in &session.roster.manager.players {
            if word(&player.bytes, 0x154) == 1 {
                assert!(
                    player
                        .auxiliaries
                        .iter()
                        .any(|p| word(&p.bytes, 4) == 0x2a
                            && word(&p.bytes, 0x18) == category as u32),
                    "auction cancellation reaches actual AI inbox"
                );
            }
        }
        // A second expiry exercises synchronous467a30 on an AI-owned category.
        let owner = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        session.world.categories[category][5] = owner as u8 + 1;
        session.world.ledgers.players[owner].owned_categories[category] = 1;
        put(&mut session.world.categories[category], 0x7c, 1);
        let age = session.world.ownership_refresh_counter;
        session.world.clock.timing[1] = (session.world.clock.timing[1] / 625 + 1) * 625 - 1;
        host.tick(session, &assets);
        assert!(
            host.failure.is_none(),
            "AI lease expiry Host failed:{:?}",
            host.failure
        );
        assert_eq!(session.world.categories[category][5], 0);
        assert_eq!(
            session.world.ledgers.players[owner].owned_categories[category],
            0
        );
        assert_eq!(
            session.world.ownership_refresh_counter,
            age.wrapping_add(100)
        );
        session
            .roster
            .ai_ownership
            .state(&session.roster.manager.players[owner])
            .unwrap();
        println!(
            "Retained lease expiry: real Host warning/expiry dialogs, zero-compensation release, AI ownership refresh and active-auction cancellation inboxes verified"
        );
    }
    fn exercise_rights_and_outcome(session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) {
        use openmt2001::{
            live_commands,
            outcome_notice::{Call, Response, Sequence},
        };
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let local = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == session.roster.status.current_player)
            .unwrap();
        let category = 0usize;
        // Isolate these controlled command clocks from other deliberately
        // pending transaction exercises, then restore the fixture's prior state.
        let pending = std::mem::take(&mut session.live_commands);
        let original = session.world.categories[category];
        let clocks = session.world.clock.timing;
        session.world.categories[category][5] = local as u8 + 1;
        session.world.categories[category][0x6d] = 0;
        session.world.categories[category][0x78..0x7c].copy_from_slice(&0u32.to_le_bytes());
        assert!(
            session
                .inspect_property_rights(category as u32, 0)
                .unwrap()
                .activate_visible
        );
        assert!(
            !session
                .inspect_property_rights(category as u32, 1)
                .unwrap()
                .activate_visible
        );
        assert_eq!(
            session
                .activate_property_rights(category as u32, 0)
                .unwrap()
                .message,
            Some(1406)
        );
        let due = clocks[0] + 25;
        session.world.clock.timing[0] = due - 1;
        session
            .drain_commands(assets, |call| {
                Err(format!("rights premature effect:{call:?}"))
            })
            .unwrap();
        assert_eq!(word(&session.world.categories[category], 0x78), 0);
        session.world.clock.timing[0] = due;
        session
            .drain_commands(assets, |call| {
                Err(format!("rights unexpected host effect:{call:?}"))
            })
            .unwrap();
        assert_eq!(session.world.categories[category][0x6d], 0);
        assert_eq!(word(&session.world.categories[category], 0x78), 1);
        assert_eq!(word(&session.world.categories[category], 0x80), 24);
        assert!(
            session
                .inspect_property_rights(category as u32, 0)
                .unwrap()
                .open
        );
        session.world.clock.timing[1] = clocks[1] + 14999;
        session
            .drain_commands(assets, |call| Err(format!("rights early unlock:{call:?}")))
            .unwrap();
        assert_eq!(word(&session.world.categories[category], 0x78), 1);
        session.world.clock.timing[1] += 1;
        session
            .drain_commands(assets, |call| {
                Err(format!("rights unexpected unlock effect:{call:?}"))
            })
            .unwrap();
        assert_eq!(word(&session.world.categories[category], 0x78), 2);
        session.world.categories[category] = original;
        session.world.clock.timing = clocks;
        let pause = session.world.pause.clone();
        let timers = session.hooks.timers.clone();
        session.world.pause = openmt2001::pause::State::default();
        for task in &mut session.hooks.timers {
            task[0] = 1;
        }
        let rival = (local + 1) % session.roster.manager.players.len();
        session
            .submit_command(
                live_commands::Command::OutcomeNotice {
                    player: rival as i32,
                    reason: 2,
                },
                assets,
                |_| Err("outcome dispatched immediately".into()),
            )
            .unwrap();
        session.world.clock.timing[0] = clocks[0] + 24;
        session
            .drain_commands(assets, |_| Err("outcome dispatched before25ticks".into()))
            .unwrap();
        session.world.clock.timing[0] += 1;
        let mut calls = vec![];
        session
            .drain_commands(assets, |event| match event {
                live_commands::Event::OutcomeNotice { player, reason } => {
                    assert_eq!((player, reason), (rival as i32, 2));
                    Ok(())
                }
                live_commands::Event::Outcome(call) => {
                    calls.push(call);
                    Ok(())
                }
                other => Err(format!("result command unexpected effect:{other:?}")),
            })
            .unwrap();
        assert!(calls.contains(&Call::Timeout(30)));
        assert!(calls.contains(&Call::Handler {
            callback: Some(0x48b490),
            context: None
        }));
        assert!(calls.contains(&Call::Handler {
            callback: None,
            context: Some(rival as u32)
        }));
        assert_eq!(session.world.pause.bytes[0], 1);
        assert_eq!(session.world.pause.bytes[8], 1);
        for task in &session.hooks.timers {
            assert_eq!(task[0], u8::from(word(task, 4) & 2 == 0));
        }
        let mut response = vec![];
        session
            .respond_outcome(rival as u32, assets, |call| {
                response.push(call);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            response,
            vec![
                Response::Characters {
                    winner: word(&session.roster.manager.players[rival].bytes, 0x150),
                    loser: word(&session.roster.manager.players[local].bytes, 0x150)
                },
                Response::Sequence(Sequence::Lose),
                Response::Music { address: 0x541cdc }
            ]
        );
        session.world.pause = pause;
        session.hooks.timers = timers;
        session.world.clock.timing = clocks;
        session.live_commands = pending;
        println!(
            "Retained rights/result: pending→25tickexclusive→15000normaltickopen;25tickresult→30sectimeout+handler+pause/group2suspension→originallose-sequence request verified (controlled rendering boundary)"
        );
    }
    fn exercise_host_outcome(
        host: &mut crate::live_host::Host,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
    ) {
        use openmt2001::{live_commands, outcome_notice::Response};
        let local = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == session.roster.status.current_player)
            .unwrap();
        let winner = (local + 1) % session.roster.manager.players.len();
        host.set_running(true);
        assert!(!session.world.pause.has_reason());
        let remaining =
            u32::from_le_bytes(session.roster.manager.bytes[12..16].try_into().unwrap());
        assert_eq!(host.status(session).player_panels, [true, true]);
        session
            .submit_command(
                live_commands::Command::OutcomeNotice {
                    player: winner as i32,
                    reason: 2,
                },
                assets,
                |_| Err("result dispatched withoutdelay".into()),
            )
            .unwrap();
        // Original bankruptcy queues result then elimination at the same due
        // tick. The drain completes even though the result pauses later ticks.
        session
            .submit_command(
                live_commands::Command::Eliminate { slot: local },
                assets,
                |_| Err("elimination dispatched without delay".into()),
            )
            .unwrap();
        let due = session.world.clock.timing[0] + 25;
        let mut modal = None;
        for _ in 0..30 {
            host.frame(session, assets, 40);
            assert!(host.failure.is_none(), "result Host:{:?}", host.failure);
            for request in host.take_prompts(session) {
                if request.dialog.is_some_and(|d| d.callback == 0x48b490) {
                    modal = Some(request);
                }
            }
            if modal.is_some() {
                break;
            }
            assert!(session.world.clock.timing[0] < due);
        }
        let request = modal.expect("real Host original result modal");
        let dialog = request.dialog.unwrap();
        #[cfg(feature = "capture")]
        super::outcome_capture::capture(request.clone(), session, assets);
        assert_eq!(session.world.clock.timing[0], due);
        assert_eq!(dialog.context, winner as u32);
        assert_eq!(session.roster.manager.players[local].bytes[0x10], 0);
        assert_eq!(
            u32::from_le_bytes(
                session.roster.manager.players[local].bytes[0x154..0x158]
                    .try_into()
                    .unwrap()
            ),
            2
        );
        assert_eq!(
            u32::from_le_bytes(session.roster.manager.bytes[12..16].try_into().unwrap()),
            remaining - 1
        );
        assert_eq!(host.status(session).player_panels, [false, false]);
        assert_eq!(dialog.timeout_ticks, Some(1500));
        assert_eq!(dialog.button_count, 1);
        assert_eq!(dialog.buttons[0], 1435);
        assert!(dialog.title != 0 && dialog.body != 0);
        assert_eq!(session.world.pause.bytes[0], 1);
        let clock = session.world.clock.timing[0];
        for _ in 0..10 {
            host.frame(session, assets, 40);
            assert!(host.failure.is_none());
        }
        assert_eq!(
            session.world.clock.timing[0], clock,
            "result freezes normal logic"
        );
        let mut ui = crate::prompt::State::default();
        ui.submit(request);
        for _ in 0..1500 {
            ui.tick();
        }
        assert!(ui.take_dialog_replies().is_empty());
        ui.tick();
        let replies = ui.take_dialog_replies();
        assert_eq!(replies.len(), 1);
        assert_eq!(
            (replies[0].callback, replies[0].context, replies[0].button),
            (0x48b490, winner as u32, 0)
        );
        let mut responses = vec![];
        session
            .respond_outcome(replies[0].context, assets, |call| {
                responses.push(call);
                Ok(())
            })
            .unwrap();
        assert!(responses.contains(&Response::Sequence(
            openmt2001::outcome_notice::Sequence::Lose
        )));
        println!(
            "Real Host result:25tick admission, localized modal/portrait,30second automaticOK, actual timer pause, local elimination closes both original player panels, and verified response request; cinematic remains a separate renderer boundary"
        );
    }
    fn exercise_property_offers(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{live_commands, property_offer};
        let word = |b: &[u8], at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let put = |b: &mut [u8], at, v: u32| b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        let local = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| p.reference == session.roster.status.current_player)
            .unwrap();
        let rival = session
            .roster
            .manager
            .players
            .iter()
            .position(|p| word(&p.bytes, 0x154) == 1)
            .unwrap();
        let category = session
            .world
            .categories
            .iter()
            .position(|r| r[5] == 0 && r[0x6c] == 0 && r[0x74] != 0)
            .unwrap() as u32;
        let assets = CommandAssets { image, board, file };
        let initial = session
            .inspect_property_offer(category, None, &assets)
            .unwrap();
        assert_eq!(initial.panel.visible, 1);
        assert!(!initial.owned);
        let amount = initial.panel.amount;
        let cash = session.world.ledgers.players[local].cash;
        session.world.ledgers.players[local].cash = -10000.;
        let mut calls = vec![];
        let result = session
            .submit_property_offer(category, amount, &assets, |event| match event {
                live_commands::Event::PropertyOffer(call) => {
                    calls.push(call);
                    Ok(())
                }
                other => Err(format!("cash denial escaped property UI:{other:?}")),
            })
            .unwrap();
        assert!(!result.submitted);
        assert_eq!(session.world.categories[category as usize][0x6c], 0);
        assert!(
            calls.iter().any(|call| matches!(
                call,
                property_offer::Call::Ui {
                    address: 0x429820,
                    arguments: [1, 0x6c2, 0],
                    ..
                }
            )),
            "original cash failure modal body"
        );
        session.world.ledgers.players[local].cash = cash.max(amount);
        // A controlled rival owner isolates the direct23 inbox route.
        session.world.categories[category as usize][5] = rival as u8 + 1;
        let before = word(&session.roster.manager.players[rival].bytes, 0x214);
        let presentation = |event| match event {
            live_commands::Event::PropertyOffer(_) => Ok(()),
            other => Err(format!(
                "property offer unexpected external route:{other:?}"
            )),
        };
        let result = session
            .submit_property_offer(category, amount, &assets, presentation)
            .unwrap();
        assert!(result.submitted);
        assert_eq!(
            word(&session.roster.manager.players[rival].bytes, 0x214),
            before + 1
        );
        let player = &session.roster.manager.players[rival];
        let mut pointer = word(&player.bytes, 0x218);
        let packet = loop {
            let p = &player
                .auxiliaries
                .iter()
                .find(|a| a.reference == pointer)
                .unwrap()
                .bytes;
            let next = word(p, 0x2c);
            if next == 0 {
                break p;
            }
            pointer = next;
        };
        assert_eq!(word(packet, 4), 0x23);
        assert_eq!(packet[0x14], local as u8);
        assert_eq!(word(packet, 0x18), category);
        assert_eq!(word(packet, 0x1c), amount as u32);
        session.world.categories[category as usize][5] = 0;
        session.world.categories[category as usize][0x6c] = 0;
        let mut host = crate::live_host::Host::new([0.; 3]);
        host.set_running(true);
        let due = session.world.clock.timing[0] + 25;
        assert!(
            session
                .submit_property_offer(category, amount, &assets, presentation)
                .unwrap()
                .submitted
        );
        while session.world.clock.timing[0] < due {
            host.tick(session, &assets);
            assert!(
                host.failure.is_none(),
                "city offer admission:{:?}",
                host.failure
            );
            if session.world.clock.timing[0] < due {
                assert_eq!(session.world.auction.controller.active_count, 0);
            }
        }
        assert_eq!(session.world.auction.controller.active_count, 1);
        assert_eq!(word(&session.world.auction.controller.record, 4), category);
        assert_eq!(
            word(&session.world.auction.controller.record, 0x84),
            local as u32
        );
        // End this controlled admission through the real hourly lease release.
        session.world.categories[category as usize][5] = local as u8 + 1;
        session.world.ledgers.players[local].owned_categories[category as usize] = 1;
        put(&mut session.world.categories[category as usize], 0x7c, 1);
        session.world.clock.timing[1] = (session.world.clock.timing[1] / 625 + 1) * 625 - 1;
        host.tick(session, &assets);
        assert!(
            host.failure.is_none(),
            "property offer cancellation:{:?}",
            host.failure
        );
        assert_eq!(session.world.auction.controller.active_count, 0);
        session.world.categories[category as usize][5] = local as u8 + 1;
        session.world.ledgers.players[local].owned_categories[category as usize] = 1;
        let own = session
            .inspect_property_offer(category, None, &assets)
            .unwrap();
        assert!(own.owned);
        assert_eq!(own.panel.visible, 1);
        let due = session.world.clock.timing[0] + 25;
        assert!(
            session
                .submit_property_offer(category, own.panel.amount, &assets, presentation)
                .unwrap()
                .submitted
        );
        while session.world.clock.timing[0] < due {
            host.tick(session, &assets);
            assert!(
                host.failure.is_none(),
                "owned property sale admission:{:?}",
                host.failure
            );
        }
        assert_eq!(session.world.auction.controller.active_count, 1);
        assert_eq!(
            word(&session.world.auction.controller.record, 0x84),
            u32::MAX
        );
        assert_eq!(
            word(&session.world.auction.controller.record, 0x88),
            own.panel.amount as u32
        );
        session.world.ledgers.players[local].cash = cash;
        println!(
            "Retained human property offers: native controls/cash rejection, rival23 inbox, city purchase and owned sale through25-tick auction admission verified"
        );
    }
    fn exercise_calendar(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) {
        use openmt2001::{hourly, level_runtime::CalendarEvent, scheduler};
        let assets = CommandAssets { image, board, file };
        let mut policy = simulation_test_policy();
        session.hooks.timers = vec![[0; 20]];
        session.hooks.timers[0][4..8].copy_from_slice(&2_u32.to_le_bytes());
        openmt2001::timers::configure(&mut session.hooks.timers[0], 25., 0x481680);
        session.hooks.frequency.logic_timer = Some(0);
        let mut paused = 0;
        let mut before_six_sounds = 0;
        let mut scripts = Vec::new();
        let mut deferred_prompts = 0;
        let mut boundaries = std::collections::BTreeSet::new();
        let mut tick_boundaries = std::collections::BTreeSet::new();
        let mut hours = Vec::new();
        let mut transport_submissions = std::collections::BTreeMap::<u32, usize>::new();
        let command_slot = session
            .world
            .ledgers
            .businesses
            .iter()
            .position(|row| row[0x21c] != 0)
            .unwrap();
        let record = &session.world.ledgers.businesses[command_slot];
        let old_rent = f32::from_le_bytes(record[0x1dc..0x1e0].try_into().unwrap());
        let new_rent = old_rent + 1.;
        let command = economy::PendingBusinessTerms {
            business_index: u32::from_le_bytes(record[0x210..0x214].try_into().unwrap()),
            terms: economy::BusinessTerms {
                stock_targets: std::array::from_fn(|i| {
                    i16::from_le_bytes(record[0x1b8 + i * 2..0x1ba + i * 2].try_into().unwrap())
                }),
                prices: std::array::from_fn(|i| {
                    f32::from_le_bytes(record[0x1c4 + i * 4..0x1c8 + i * 4].try_into().unwrap())
                }),
                rent: new_rent,
            },
        };
        assert_eq!(command.business_index as usize, command_slot);
        session
            .live_commands
            .submit(
                openmt2001::live_commands::Command::Terms(command),
                0,
                true,
                |_| Err("delayed terms applied immediately".into()),
            )
            .unwrap();
        let mut inbox = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena {
                next: session.allocation_next,
            },
            current_player: session.roster.status.current_player,
            network: [false, false],
            prior: [0; 48],
        };
        for kind in [0x2b, 0x2c] {
            inbox
                .produce(
                    [0, -1, kind, 0, 0, 0, 0, -999],
                    &mut session.roster.manager,
                    |_, _| panic!("unexpected inbox external route"),
                )
                .unwrap();
        }
        session.allocation_next = inbox.arena.next;
        let mut human_calls = vec![];
        while session.world.clock.timing[1] < 15000 {
            session
                .advance_transport_frame(|auxiliary| {
                    let reference = u32::from_le_bytes(auxiliary[..4].try_into().unwrap());
                    *transport_submissions.entry(reference).or_default() += 1;
                    Ok(())
                })
                .unwrap();
            if session.world.pause.has_reason() {
                assert_eq!(session.hooks.timers[0][0], 0);
                paused += 1;
                if paused % 3 != 0 {
                    let before = session.world.clock.timing;
                    session
                        .advance_session(&assets, &mut policy, |_, _| panic!("paused dispatch ran"))
                        .unwrap();
                    assert_eq!(session.world.clock.timing, before);
                    continue;
                }
                session
                    .dispatch_session_event("InterfacePromptDismiss", 0, &assets, false)
                    .unwrap();
                assert!(!session.world.pause.has_reason());
            }
            session
                .advance_session(&assets, &mut policy, |event, clock| {
                    match event {
                        tick::Event::ScriptCommands(commands) => {
                            assert!(commands.iter().all(|(command, args)| matches!(
                                command,
                                openmt2001::level_script::Command::Prompt
                            ) && args.len() == 3));
                            deferred_prompts += commands.len();
                        }
                        tick::Event::Script { name, argument, .. } => {
                            if name == "HourlyUpdate" {
                                hours.push(clock.hour());
                            }
                            scripts.push((name, argument));
                        }
                        tick::Event::Tick(scheduler::Event::Helper(address)) => {
                            tick_boundaries.insert(address);
                        }
                        tick::Event::Tick(scheduler::Event::BeforeSixSound) => {
                            before_six_sounds += 1;
                        }
                        tick::Event::Calendar(CalendarEvent::Hourly(hourly::Event::Helper(
                            address,
                        ))) => {
                            boundaries.insert(address);
                        }
                        tick::Event::Calendar(
                            CalendarEvent::Hourly(hourly::Event::Query)
                            | CalendarEvent::Clock(
                                openmt2001::clock::Event::RolloverQuery
                                | openmt2001::clock::Event::Hour19
                                | openmt2001::clock::Event::Hour7,
                            ),
                        ) => {}
                        tick::Event::Player(openmt2001::player_tick::Event::Ui { .. }) => {}
                        tick::Event::TransportSound(sound) => {
                            assert!(sound.id <= 12);
                            assert!(sound.distance.is_finite());
                        }
                        tick::Event::TransportEnvironment { .. } => {}
                        tick::Event::HumanDialog { call, .. } => human_calls.push(call),
                        other => {
                            return Err(format!("unexpected retained full-day boundary {other:?}"));
                        }
                    }
                    Ok(0)
                })
                .unwrap();
            let tick = session.world.clock.timing[0];
            if tick <= 25 {
                let rent = f32::from_le_bytes(
                    session.world.ledgers.businesses[command_slot][0x1dc..0x1e0]
                        .try_into()
                        .unwrap(),
                );
                assert_eq!(rent, if tick < 25 { old_rent } else { new_rent });
            }
        }
        assert!(
            deferred_prompts > 0,
            "scheduled prompt output must reach the retained host boundary"
        );
        assert_eq!(session.world.clock.hour(), 9);
        assert_eq!(human_calls.len(), 6);
        assert!(matches!(
            human_calls[0],
            openmt2001::human_messages::Call::Ui {
                address: 0x43be20,
                ..
            }
        ));
        assert_eq!(
            session.roster.manager.players[0].bytes[0x214..0x21c],
            [0; 8]
        );
        for reference in [0x6887e0, 0x688880, 0x688db0, 0x688dc0, 0x688dd0, 0x688de0] {
            assert!(
                transport_submissions.get(&reference).copied().unwrap_or(0) > 0,
                "missing transport submission {reference:08x}"
            );
        }
        assert!(
            session
                .transport
                .aircraft
                .iter()
                .any(|row| row[..4] == [0; 4]),
            "aircraft expiry must release pool capacity"
        );
        assert_eq!(session.world.clock.calendar[4], 1);
        assert_eq!(before_six_sounds, 1);
        assert_eq!(hours, (10..=24).chain(1..=9).collect::<Vec<_>>());
        assert!(scripts.iter().any(|(name, _)| name == "EndOfDay"));
        assert!(scripts.iter().any(|(name, _)| name == "BlockUnlock"));
        assert!(boundaries.is_empty());
        assert_eq!(session.history.count(0, 0), Some(34));
        assert_eq!(session.history.cursor(0, 0), Some(34));
        assert_eq!(
            tick_boundaries,
            [0x4805b0, 0x4eb5b0, 0x450080, 0x4b5db0, 0x4b5d50]
                .into_iter()
                .collect()
        );
        println!(
            "Full day: 15000 ticks, {} paused host frames, {} explicit tick helper boundaries",
            paused,
            tick_boundaries.len()
        );
    }
    fn exercise_market_outcome(
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile,
    ) {
        use openmt2001::{economy, live_commands, player_share};
        assert_eq!(session.roster.manager.players.len(), 2);
        let assets = CommandAssets { image, board, file };
        let business = session
            .world
            .ledgers
            .businesses
            .iter()
            .position(|r| r[0x21c] != 0)
            .unwrap();
        let building = i16::from_le_bytes(
            session.world.ledgers.businesses[business][8..10]
                .try_into()
                .unwrap(),
        ) as usize;
        let baseline_housing = session.world.ledgers.players[1].housing_counter;
        let prefix = session.world.buildings[building][..64].try_into().unwrap();
        economy::transfer_business_owner(
            &mut session.world.city.ledgers.businesses[business],
            &prefix,
            2,
            true,
            &mut session.world.city.ledgers.players,
        )
        .unwrap();
        session.world.buildings[building][0x2d] = 2;
        session.world.categories[0][5] = 2;
        session.roster.manager.players[1].bytes[0x178] = 1;
        session.world.objective.kind = 1;
        session.world.objective.target = 1.;
        session.world.objective.reached = [0; 6];
        for (slot, account) in session.world.ledgers.players.iter_mut().enumerate() {
            account.retail_counters[0] = if slot == 0 { 1_000_000 } else { 100 };
        }
        for player in &mut session.roster.manager.players {
            player.bytes[0x14c..0x150].copy_from_slice(&0u32.to_le_bytes());
        }
        let tick = session.world.clock.timing[0];
        let current = session.roster.status.current_player;
        let mut messages = openmt2001::player_messages::Context {
            arena: openmt2001::player_messages::Arena {
                next: session.allocation_next,
            },
            current_player: current,
            network: [false, false],
            prior: [0; 48],
        };
        let mut resolved = vec![];
        let result = session
            .roster
            .advance_market_share(
                &mut session.world.city,
                2,
                [false, false],
                [0.; 6],
                |call, _| {
                    assert!(matches!(
                        call,
                        player_share::Call::Refresh
                            | player_share::Call::Kind1ProfileSelect { index: 0 }
                            | player_share::Call::PlayerShare { .. }
                    ));
                    Ok(Default::default())
                },
                |event, _, manager, _| {
                    resolved.push(event);
                    live_commands::queue_outcome(
                        event,
                        &mut session.live_commands,
                        tick,
                        manager,
                        &mut messages,
                        |_| panic!("local loser inbox incorrectly routed externally"),
                    )?;
                    Ok(openmt2001::x87::Sum::from_i32(0))
                },
            )
            .unwrap();
        assert_eq!(result.max_index, 0);
        assert_eq!(session.world.objective.reached[0], 1);
        assert_eq!(
            resolved,
            vec![
                openmt2001::objective_tick::Event::Outcome {
                    winner: 0,
                    reason: 2
                },
                openmt2001::objective_tick::Event::RemovalCommand(1),
                openmt2001::objective_tick::Event::RemovePlayer(1)
            ]
        );
        assert!(
            session
                .world
                .ledgers
                .players
                .iter()
                .all(|p| p.retail_counters[0] == 0)
        );
        assert_eq!(
            u32::from_le_bytes(
                session.roster.manager.players[0].bytes[0x14c..0x150]
                    .try_into()
                    .unwrap()
            ),
            1
        );
        session.world.clock.timing[0] = tick + 24;
        session
            .drain_commands(&assets, |_| panic!("outcome dispatched before25 ticks"))
            .unwrap();
        assert_ne!(session.roster.manager.players[1].bytes[0x10], 0);
        session.world.clock.timing[0] = tick + 25;
        let before_elimination_ages = [
            session.world.ownership_refresh_counter,
            session.population.business_counts.age,
        ];
        let mut notices = vec![];
        session
            .drain_commands(&assets, |event| {
                match event {
                    live_commands::Event::OutcomeNotice { player, reason } => {
                        notices.push((player, reason))
                    }
                    live_commands::Event::Outcome(_) => (),
                    live_commands::Event::Elimination(
                        openmt2001::player_elimination::Call::BuildingOwner { .. }
                        | openmt2001::player_elimination::Call::BuildingRefresh { .. },
                    ) => (),
                    other => panic!("unexpected elimination effect {other:?}"),
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(notices, vec![(0, 2)]);
        assert_eq!(session.roster.manager.players[1].bytes[0x10], 0);
        assert_eq!(
            session.roster.manager.players[1].bytes[0x154..0x158],
            2u32.to_le_bytes()
        );
        assert_eq!(session.roster.manager.bytes[12..16], 1u32.to_le_bytes());
        assert_eq!(session.world.ledgers.businesses[business][0xa], 0);
        assert_ne!(session.world.ledgers.businesses[business][0x21c], 0);
        assert_eq!(session.world.buildings[building][0x2d], 0);
        assert_eq!(session.world.categories[0][5], 0);
        assert_eq!(session.world.categories[0][0x78..0x7c], 2u32.to_le_bytes());
        assert_eq!(session.roster.manager.players[1].bytes[0x178], 0);
        assert_eq!(
            session.world.ledgers.players[1].housing_counter,
            baseline_housing
        );
        assert_eq!(
            session.world.ownership_refresh_counter,
            before_elimination_ages[0].wrapping_add(100)
        );
        assert_eq!(
            session.population.business_counts.age,
            before_elimination_ages[1].wrapping_add(100)
        );
        assert_eq!(session.roster.manager.players.len(), 2);
        println!(
            "Market-share win: callback marker, score, retail reset, queued notice,25-tick elimination, property/business/account transfer and retained slots verified"
        );

        for cash in [-1., 10., -1., -1.] {
            session.world.ledgers.players[0].cash = cash;
            session
                .roster
                .update_cash_status(
                    &mut session.world.city,
                    24,
                    &mut openmt2001::player_manager::CashControls {
                        network_enabled: false,
                        network_override: false,
                    },
                    |call, manager, _, _| {
                        live_commands::queue_cash_status(
                            call,
                            &mut session.live_commands,
                            tick + 25,
                            manager,
                            &mut messages,
                            |_| panic!("local cash inbox incorrectly routed externally"),
                        )
                    },
                )
                .unwrap();
        }
        assert_eq!(
            u32::from_le_bytes(
                session.roster.manager.players[0].bytes[0x214..0x218]
                    .try_into()
                    .unwrap()
            ),
            4
        );
        assert_eq!(
            u32::from_le_bytes(
                session.roster.manager.players[1].bytes[0x214..0x218]
                    .try_into()
                    .unwrap()
            ),
            1
        );
        let mut shown = vec![];
        session.roster.manager.players[0].bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        for _ in 0..8 {
            openmt2001::player_tick::advance_human(
                &mut session.roster.manager.players[0],
                |player| {
                    openmt2001::player_messages::consume_human(player, |call, _| {
                        shown.push(call);
                        Ok(())
                    })
                },
            )
            .unwrap();
        }
        assert_eq!(shown.len(), 12); // Two warnings; recovery/removal are consumed silently.
        assert_eq!(
            session.roster.manager.players[0].bytes[0x214..0x218],
            [0; 4]
        );
        assert_eq!(
            session.roster.manager.players[0].bytes[0x218..0x21c],
            [0; 4]
        );
        session.allocation_next = messages.arena.next;
        assert_ne!(session.roster.manager.players[0].bytes[0x10], 0);
        session.world.clock.timing[0] = tick + 49;
        session
            .drain_commands(&assets, |_| panic!("bankruptcy dispatched early"))
            .unwrap();
        session.world.clock.timing[0] = tick + 50;
        let mut effects = vec![];
        session
            .drain_commands(&assets, |event| {
                if !matches!(event, live_commands::Event::Outcome(_)) {
                    effects.push(event);
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(
            effects,
            vec![
                live_commands::Event::OutcomeNotice {
                    player: 0,
                    reason: 0
                },
                live_commands::Event::Elimination(openmt2001::player_elimination::Call::PlayerUi {
                    object: 0
                }),
                live_commands::Event::Elimination(openmt2001::player_elimination::Call::PlayerUi {
                    object: 1
                }),
                live_commands::Event::Elimination(
                    openmt2001::player_elimination::Call::FinishLocal
                )
            ]
        );
        assert_eq!(session.roster.manager.bytes[12..16], [0; 4]);
        assert!(
            session
                .roster
                .manager
                .players
                .iter()
                .all(|p| p.bytes[0x10] == 0)
        );
        println!(
            "Cash warning/recovery/bankruptcy: ordered inbox boundaries, delayed notice/elimination and current-player effects verified"
        );
    }
    // This exercises the retained population/traffic dispatcher and live timer
    // edits, not the still-incomplete full game: other world helpers are traced
    // explicitly, and crossing an hourly or deadline boundary is an error.
    fn exercise_retained_population(session: &mut Session<'_>) {
        use openmt2001::{
            scheduler::{Controls, Event, TimedEvent},
            timers,
        };
        let initial_day_ticks = session.world.clock.timing[3];
        let positions: Vec<_> = session
            .world
            .creation
            .auxiliary
            .slots()
            .iter()
            .map(|row| row[4..16].to_vec())
            .collect();
        let routes = openmt2001::traffic::road::routes::RoadRoutes::from_bytes(
            &session.road_routes,
            session.world.road_network.road_segment_count as u32,
        )
        .unwrap();
        let mut dispatch = openmt2001::level_runtime::Dispatch {
            population: &mut session.population,
            roads: &mut session.road_traffic,
            routes: &routes,
            models: &mut session.vehicle_models,
            camera: [0., 20., 0.],
            distance_disabled: false,
        };
        let mut controls = Controls {
            iterations: 1,
            abort: false,
            alternate_script_active: false,
            network_enabled: false,
            network_override: false,
            deadline: u32::MAX,
            gate_32: false,
            gate_32_offset: false,
            before_six_sound: 0,
        };
        let mut tasks = vec![[0; 20]];
        timers::configure(&mut tasks[0], 25., 0x481680);
        let mut frame = timers::Frame::default();
        frame.initialize(0);
        let mut boundaries = std::collections::BTreeSet::new();
        for host_frame in 1..=105 {
            if host_frame == 31 {
                tasks[0][0] = 1;
            }
            dispatch
                .advance_timed(
                    &mut session.world,
                    &mut controls,
                    &mut frame,
                    &mut tasks,
                    Some(host_frame * 40),
                    |event, clock, _, _, tasks, _| {
                        if matches!(event, openmt2001::level_runtime::Event::AccountPosted(_)) {
                            return Ok(0);
                        }
                        let openmt2001::level_runtime::Event::Scheduled(TimedEvent::Logic(
                            Event::Helper(address),
                        )) = event
                        else {
                            return Err(format!(
                                "unexpected retained population fixture boundary {event:?}"
                            ));
                        };
                        boundaries.insert(address);
                        if address == 0x4d5b40 && clock.timing[0] == 25 {
                            tasks[0][0] = 0;
                        }
                        Ok(0)
                    },
                )
                .unwrap();
            if (25..=30).contains(&host_frame) {
                assert_eq!(
                    session.world.clock.timing[0], 25,
                    "timer pause must affect live dispatch"
                );
            }
        }
        assert_eq!(
            session.world.clock.timing[..4],
            [100, 100, 100, initial_day_ticks + 100]
        );
        assert_eq!(session.world.clock.hour(), 9);
        assert!(
            session
                .world
                .creation
                .auxiliary
                .slots()
                .iter()
                .zip(&positions)
                .any(|(row, before)| row[4..16] != before[..]),
            "residents must move in retained city records"
        );
        assert_eq!(
            boundaries,
            [
                0x4ed6f0, 0x4ed6e0, 0x480ec0, 0x44ec50, 0x4805b0, 0x4eb5b0, 0x450080, 0x4b5db0,
                0x4b5d50, 0x4b0810, 0x477f20, 0x4d5b40, 0x4896e0, 0x48cea0
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    #[ignore = "requires original level scripts, people meshes and rendering assets"]
    fn original_construction_lifecycle() {
        use openmt2001::{building_pool, construction, live_commands, scene};
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
        let scripts = default_path.parent().unwrap().parent().unwrap();
        let language = openmt2001::fixtures::path("EXE")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let names = character_names(&image, &language).unwrap();
        for case in [0u32, 1, 2, 3] {
            // Case 3 is catalog row 43, the special whole-group Hotel shown
            // under Residential even though its catalog type word is zero.
            let kind = if case == 3 { 1 } else { case };
            let inputs = || Inputs {
                tree_locators: None,
                image: &image,
                board: &board,
                file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
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
                era_styles: load_era_styles(
                    Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                        .parent()
                        .unwrap(),
                )
                .unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario: "SCENARIO3",
                difficulty: 0,
                selected_character: 4,
                profile_name: b"Construction Fixture",
                character_names: &names,
            };
            let policy = || Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 0x11c],
            };
            let mut session = Session::initialize(inputs(), policy()).unwrap();
            session.world.clock.calendar[0] = 9;
            let assets = CommandAssets {
                image: &image,
                board: &board,
                file: &file,
            };
            if kind == 1 {
                // The original planner presents row 43 only when 48a770
                // sees one owner across the complete property-color group.
                let category = 33usize;
                let identity = u32::from_le_bytes(
                    session.world.categories[category][0x8c..0x90]
                        .try_into()
                        .unwrap(),
                );
                let members =
                    crate::build_panel::special_residential_group(&image, identity).unwrap();
                assert_eq!(members, [27, 23, 33, -1]);
                let prior =
                    members.map(|id| (id >= 0).then(|| session.world.categories[id as usize][5]));
                for id in members.into_iter().filter(|&id| id >= 0) {
                    session.world.categories[id as usize][5] = 1;
                }
                let info = *session.world.catalog.info.row(43).unwrap();
                let info_word = |at| u32::from_le_bytes(info[at..at + 4].try_into().unwrap());
                let mut selected = None;
                for width in 1..=6 {
                    for depth in 1..=6 {
                        if width * depth < info_word(0x38)
                            || width * depth > info_word(0x3c)
                            || width.max(depth) > info_word(0x40)
                        {
                            continue;
                        }
                        for x in 0..=7 - width {
                            for y in 0..=7 - depth {
                                let rect = [
                                    x as i32,
                                    y as i32,
                                    (x + width - 1) as i32,
                                    (y + depth - 1) as i32,
                                ];
                                if building_pool::can_place(
                                    session.world.categories[category][0x9c..0xcd]
                                        .try_into()
                                        .unwrap(),
                                    rect,
                                ) {
                                    selected = Some((x, y, width, depth));
                                }
                            }
                        }
                    }
                }
                let (x, y, width, depth) =
                    selected.expect("original owned Hotel group has a buildable lot");
                let request = crate::build_panel::Request {
                    category: category as i32,
                    origin: [x as i32, y as i32],
                    size: [width as i32, depth as i32],
                    height: info_word(0x48) as i32,
                    packed_model: 0,
                    kind: 1,
                    quality: 0,
                    displayed_quote: 0.,
                    business: 43,
                };
                let quote = session.inspect_build(&request, &assets).unwrap();
                assert!(
                    quote.preview.is_some(),
                    "special Hotel has original model preview"
                );
                assert!(quote.allowed(), "owned group permits Hotel construction");
                let cash = session.world.ledgers.players[0].cash.to_bits();
                let pending = session.live_commands.snapshot().unwrap().len();
                let occupancy = session.world.categories[category];
                session.roster.manager.players[0].bytes[0x10] = 0;
                let context =
                    crate::build_panel::Context::from_session(&session, category).unwrap();
                assert_eq!(context.entry(), crate::build_panel::Entry::Exit);
                for submit in [false, true] {
                    assert!(
                        session
                            .plan_build(&request, &assets, submit, |e| panic!(
                                "inactive planner emitted {e:?}"
                            ))
                            .unwrap()
                            .is_none()
                    );
                }
                assert!(session.submit_build(&request, &assets, |_| Ok(())).is_err());
                assert_eq!(session.world.ledgers.players[0].cash.to_bits(), cash);
                assert_eq!(session.live_commands.snapshot().unwrap().len(), pending);
                assert_eq!(session.world.categories[category], occupancy);
                session.roster.manager.players[0].bytes[0x10] = 1;
                for busy in [0, 1, 2] {
                    session.world.categories[category][0x6c] = busy;
                    let context =
                        crate::build_panel::Context::from_session(&session, category).unwrap();
                    assert_eq!(
                        context.entry() == crate::build_panel::Entry::Exit,
                        busy == 1
                    );
                    assert_eq!(
                        session
                            .plan_build(&request, &assets, false, |_| Ok(()))
                            .unwrap()
                            .is_none(),
                        busy == 1
                    );
                }
                session.world.categories[category][0x6c] = occupancy[0x6c];
                for (pending_rights, rights, message) in
                    [(0, 0u32, Some(1202)), (1, 2, Some(1406)), (0, 2, None)]
                {
                    let row = &mut session.world.categories[category];
                    row[0x6d] = pending_rights;
                    row[0x78..0x7c].copy_from_slice(&rights.to_le_bytes());
                    let context =
                        crate::build_panel::Context::from_session(&session, category).unwrap();
                    if let Some(text) = message {
                        assert!(
                            matches!(context.entry(), crate::build_panel::Entry::Message { text: actual, .. } if actual == text)
                        );
                        let error = session
                            .plan_build(&request, &assets, true, |_| Ok(()))
                            .err()
                            .unwrap();
                        assert!(error.contains(&text.to_string()));
                    } else {
                        assert_eq!(context.entry(), crate::build_panel::Entry::Open);
                        assert!(
                            session
                                .plan_build(&request, &assets, false, |_| Ok(()))
                                .unwrap()
                                .unwrap()
                                .allowed()
                        );
                    }
                }
                session.world.categories[category] = occupancy;
                // Keep building rights open so each rejection below exercises
                // hotel admission, including when the city is the landlord.
                session.world.categories[category][0x6d] = 0;
                session.world.categories[category][0x78..0x7c].copy_from_slice(&2u32.to_le_bytes());
                for (landlord, other, expected) in [
                    (0, 0, false),
                    (1, 1, true),
                    (2, 2, true),
                    (1, 0, false),
                    (1, 2, false),
                ] {
                    for id in members.into_iter().filter(|&id| id != -1) {
                        session.world.categories[id as usize][5] = landlord;
                    }
                    session.world.categories[23][5] = other;
                    assert_eq!(
                        session
                            .plan_build(&request, &assets, false, |_| Ok(()))
                            .is_ok(),
                        expected,
                        "worker hotel admission for landlord {landlord}, other owner {other}"
                    );
                    if !expected {
                        assert_eq!(
                            session
                                .plan_build(&request, &assets, true, |_| Ok(()))
                                .err()
                                .unwrap(),
                            "Planning business is unavailable for this type"
                        );
                    }
                }
                assert_eq!(session.world.ledgers.players[0].cash.to_bits(), cash);
                assert_eq!(session.live_commands.snapshot().unwrap().len(), pending);
                for id in members.into_iter().filter(|&id| id != -1) {
                    session.world.categories[id as usize][5] = 1;
                }
                session.world.categories[category] = occupancy;
                session.world.categories[23][5] = 2;
                assert!(
                    session.inspect_build(&request, &assets).is_err(),
                    "stale Hotel quote cannot bypass lost group ownership"
                );
                session.world.categories[23][5] = 1;
                let mut wrong_kind = request.clone();
                wrong_kind.kind = 0;
                assert!(
                    session.inspect_build(&wrong_kind, &assets).is_err(),
                    "special row 43 is not an ordinary retail choice"
                );
                for (id, owner) in members
                    .into_iter()
                    .zip(prior)
                    .filter_map(|(id, owner)| owner.map(|owner| (id, owner)))
                {
                    session.world.categories[id as usize][5] = owner;
                }
            }
            let business = if case == 3 {
                43
            } else {
                session
                    .world
                    .catalog
                    .info
                    .rows()
                    .iter()
                    .position(|r| r[0xc..0x10] == kind.to_le_bytes())
                    .unwrap()
            };
            let height = if case == 3 {
                u32::from_le_bytes(
                    session.world.catalog.info.row(43).unwrap()[0x48..0x4c]
                        .try_into()
                        .unwrap(),
                ) as u8
            } else {
                1
            };
            let mut selected = None;
            let mut blocks: Vec<_> = board.blocks.iter().collect();
            if case == 3 {
                blocks.retain(|block| block.id == 33);
            }
            blocks.sort_by_key(|block| {
                std::cmp::Reverse(u32::from_le_bytes(
                    session.world.categories[block.id][8..12]
                        .try_into()
                        .unwrap(),
                ))
            });
            'blocks: for block in blocks {
                let category = &session.world.categories[block.id];
                if category[0x6c] == 1 {
                    continue;
                }
                for (x, y, width, depth) in (1..=6u32)
                    .flat_map(|width| (1..=6u32).map(move |depth| (width, depth)))
                    .flat_map(|(width, depth)| {
                        (0..=7 - width)
                            .flat_map(move |x| (0..=7 - depth).map(move |y| (x, y, width, depth)))
                    })
                {
                    if case != 3
                        && (width != 2
                            || depth != 2
                            || ![[0, 0], [0, 5], [5, 0], [5, 5]].contains(&[x, y]))
                    {
                        continue;
                    }
                    if case == 3 {
                        let row = session.world.catalog.info.row(43).unwrap();
                        let word = |at| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
                        if width * depth < word(0x38)
                            || width * depth > word(0x3c)
                            || width.max(depth) > word(0x40)
                        {
                            continue;
                        }
                    }
                    let rectangle = [
                        x as i32,
                        y as i32,
                        (x + width - 1) as i32,
                        (y + depth - 1) as i32,
                    ];
                    if !building_pool::can_place(
                        category[0x9c..0xcd].try_into().unwrap(),
                        rectangle,
                    ) {
                        continue;
                    }
                    let spec = scene::BuildingSpec {
                        block: block.name.clone(),
                        lot: [x as u8, y as u8, width as u8, depth as u8],
                        height,
                        kind: kind as u8,
                        variation: 0,
                        quality: 0,
                        businesses: vec![],
                    };
                    if scene::SceneBuilding::resolve(spec, &board, &file).is_ok() {
                        selected = Some((block.id, rectangle));
                        break 'blocks;
                    }
                }
            }
            let (category, rectangle) =
                selected.expect("fresh city has a buildable original model footprint");
            let mut command = construction::Command {
                category: category as i8,
                rectangle: rectangle.map(|v| v as i8),
                height: height as i8,
                kind,
                business: business as u32,
                variation: 0,
                quality: 0,
                owner: 1,
                components: [0.; 4],
            };
            let hour = session.world.clock.hour();
            let quote = live_commands::quote(
                &mut command,
                &mut session.world.city,
                &session.roster,
                &image,
                hour,
            )
            .unwrap();
            assert!(quote > 0);
            if case == 3 {
                // Original445720/461550 with supplied Residential15/rent3,
                // Hotel refurb10, Atlantic prestige4 and six cells at height2:
                // base600 + rent69.588455f32 + utilities24 -> FTOL693.
                // Pin the operands; raw PE defaults10/10 instead quote735.
                assert_eq!((category, business, kind, height, hour), (33, 43, 1, 2, 9));
                assert_eq!(
                    (rectangle[2] - rectangle[0] + 1) * (rectangle[3] - rectangle[1] + 1),
                    6
                );
                assert_eq!(command.quality, 0);
                assert_eq!(
                    session.world.city.building_parameters.row(1).unwrap(),
                    [3, 15, 4]
                );
                assert_eq!(
                    f32::from_le_bytes(
                        session.world.categories[33][0x84..0x88].try_into().unwrap()
                    ),
                    4.
                );
                let hotel = session.world.catalog.info.row(43).unwrap();
                let hotel_word = |at| u32::from_le_bytes(hotel[at..at + 4].try_into().unwrap());
                assert_eq!(
                    (
                        hotel_word(0x38),
                        hotel_word(0x3c),
                        hotel_word(0x48),
                        hotel_word(0x54)
                    ),
                    (6, 16, 2, 10)
                );
                assert_eq!(image.f32_at(0x53be78).unwrap(), 0.5);
                assert_eq!(
                    command.components.map(f32::to_bits),
                    [600_f32, 0., 69.588455, 24.].map(f32::to_bits)
                );
                assert_eq!(quote, 693, "configured Hotel lifecycle quote");
            }
            let slot = building_pool::first_free(&session.world.buildings)
                .unwrap()
                .unwrap();
            let original_slot = session.world.buildings[slot];
            let mut expected_account = session.world.ledgers.players[0].clone();
            for (component, ledger) in [(0, 0xc), (2, 0xe), (1, 0x11), (3, 7)] {
                expected_account
                    .post(-command.components[component], ledger)
                    .unwrap();
            }
            //Native45184e reserves these literal x-major cells before enqueue.
            let mut admitted_grid: [u8; 49] = session.world.categories[category][0x9c..0xcd]
                .try_into()
                .unwrap();
            for x in rectangle[0]..=rectangle[2] {
                for y in rectangle[1]..=rectangle[3] {
                    admitted_grid[(x * 7 + y) as usize] = command.height as u8;
                }
            }
            let cash_before_admission = session.world.ledgers.players[0].cash.to_bits();
            let mut events = Vec::new();
            session
                .submit_command(
                    live_commands::Command::Build(command.clone()),
                    &assets,
                    |e| {
                        events.push(e);
                        Ok(())
                    },
                )
                .unwrap();
            assert!(events.is_empty());
            assert_eq!(
                &session.world.categories[category][0x9c..0xcd],
                &admitted_grid
            );
            assert_eq!(
                session.world.ledgers.players[0].cash.to_bits(),
                cash_before_admission
            );
            let pending_packet = match session.live_commands.snapshot().unwrap()[0].4 {
                live_commands::Command::Build(c) => c.packet(),
                _ => panic!("wrong pending command"),
            };
            //A second direct packet sees the first pending footprint and returns
            //sender success without another node, cash post, or human notice.
            let pending_count = session.live_commands.snapshot().unwrap().len();
            session
                .submit_command(
                    live_commands::Command::Build(command.clone()),
                    &assets,
                    |e| Err(format!("rejected human construction emitted {e:?}")),
                )
                .unwrap();
            assert_eq!(
                session.live_commands.snapshot().unwrap().len(),
                pending_count
            );
            assert_eq!(
                &session.world.categories[category][0x9c..0xcd],
                &admitted_grid
            );
            assert_eq!(
                session.world.ledgers.players[0].cash.to_bits(),
                cash_before_admission
            );
            assert_eq!(
                pending_packet,
                command.packet(),
                "quoted components stay in the packet"
            );
            let start = session.world.clock.timing[0];
            let mut cold_pending = if case == 0 {
                let camera = openmt2001::flyby::CameraSample {
                    eye: [100., 120., 100.],
                    target: [0.; 3],
                    up: [0., 1., 0.],
                    vertical_fov: 0.7,
                };
                let cameras = crate::save_menu::cameras(None, camera).unwrap();
                let bytes = session.save_v2(&image, cameras, 1).unwrap();
                let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
                let mut cold = Session::load_original(inputs(), policy(), &bytes).unwrap();
                cold.restore_v2_extension(&save).unwrap();
                let rows = cold.live_commands.snapshot().unwrap();
                assert_eq!(rows.len(), pending_count);
                assert_eq!(
                    (rows[0].1, rows[0].2, rows[0].3),
                    (start + 25, start + 25, false)
                );
                let live_commands::Command::Build(restored) = rows[0].4 else {
                    panic!("lost saved Build0f")
                };
                assert_eq!(restored.packet(), pending_packet);
                assert_eq!(
                    cold.allocation_next, session.allocation_next,
                    "restore cannot produce message30"
                );
                assert_eq!(
                    cold.world.ledgers.players[0].cash.to_bits(),
                    cash_before_admission
                );
                //Original447d70 rebuilds occupancy from buildings; the saved
                //140-byte category prefix excludes+9c. A future packet enters
                //450a30/480900, not451720. No fresh reservation on cold restore.
                for x in rectangle[0]..=rectangle[2] {
                    for y in rectangle[1]..=rectangle[3] {
                        assert_eq!(
                            cold.world.categories[category][0x9c + (x * 7 + y) as usize],
                            0
                        );
                    }
                }
                Some(cold)
            } else {
                None
            };
            // Accepted delayed construction is independent of planner activity.
            session.roster.manager.players[0].bytes[0x10] = 0;
            for tick in start..start + 25 {
                session.world.clock.timing[0] = tick;
                session
                    .drain_commands(&assets, |_| panic!("construction dispatched early"))
                    .unwrap();
                assert_eq!(session.world.buildings[slot], original_slot);
                assert_eq!(
                    &session.world.categories[category][0x9c..0xcd],
                    &admitted_grid
                );
                assert_eq!(
                    session.world.ledgers.players[0].cash.to_bits(),
                    cash_before_admission
                );
                if let Some(cold) = cold_pending.as_mut() {
                    cold.world.clock.timing[0] = tick;
                    cold.drain_commands(&assets, |e| {
                        Err(format!("saved Build0f dispatched early: {e:?}"))
                    })
                    .unwrap();
                    assert_eq!(
                        cold.world.ledgers.players[0].cash.to_bits(),
                        cash_before_admission
                    );
                    assert_eq!(cold.world.pending_world.count, 0);
                }
            }
            session.world.clock.timing[0] = start + 25;
            session
                .drain_commands(&assets, |e| {
                    events.push(e);
                    Ok(())
                })
                .unwrap();
            assert_eq!(session.world.pending_world.count, 1);
            if let Some(cold) = cold_pending.as_mut() {
                cold.world.clock.timing[0] = start + 25;
                cold.drain_commands(&assets, |_| Ok(())).unwrap();
                assert_eq!(
                    cold.world.pending_world.count, 1,
                    "saved packet must apply once at its original Total deadline"
                );
                assert_eq!(
                    cold.world.ledgers.players[0].cash.to_bits(),
                    expected_account.cash.to_bits()
                );
                assert_eq!(&cold.world.categories[category][0x9c..0xcd], &admitted_grid);
                assert!(cold.live_commands.snapshot().unwrap().is_empty());
            }
            assert_eq!(session.world.buildings[slot][0x14..0x18], [0; 4]);
            assert_eq!(
                session.world.ledgers.players[0].cash.to_bits(),
                expected_account.cash.to_bits()
            );
            for ledger in 0..38 {
                assert_eq!(
                    session.world.ledgers.players[0]
                        .ledger(ledger)
                        .unwrap()
                        .to_bits(),
                    expected_account.ledger(ledger).unwrap().to_bits()
                );
            }
            assert_eq!(
                events
                    .iter()
                    .filter(|e| matches!(e, live_commands::Event::AccountPosted(_)))
                    .count(),
                4
            );
            assert!(session.models.bindings.contains_key(&slot));
            #[cfg(feature = "capture")]
            let capture_stage = |session: &Session<'_>, stage: &str| {
                if let Ok(prefix) = std::env::var("OPENMT2001_CONSTRUCTION_CAPTURE") {
                    crate::capture_test::construction_stage(
                        session,
                        &build,
                        openmt2001::render_scene::OriginalAssets {
                            executable: &executable,
                            directory: &directory,
                            archive: &archive,
                        },
                        slot,
                        &format!("{prefix}-{case}-{stage}.png"),
                    );
                }
            };
            #[cfg(feature = "capture")]
            capture_stage(&session, "foundation");
            assert!(!building_pool::can_place(
                session.world.categories[category][0x9c..0xcd]
                    .try_into()
                    .unwrap(),
                rectangle
            ));
            let mut stages = std::collections::BTreeSet::new();
            let mut steps = 0;
            #[cfg(feature = "capture")]
            let mut halfway_captured = false;
            while session.world.pending_world.count != 0 {
                steps += 1;
                assert!(steps < 500);
                session.world.clock.timing[0] += 1;
                session
                    .drain_commands(&assets, |e| {
                        events.push(e);
                        Ok(())
                    })
                    .unwrap();
                #[cfg(feature = "capture")]
                if !halfway_captured
                    && f32::from_le_bytes(
                        session.world.buildings[slot][0x70..0x74]
                            .try_into()
                            .unwrap(),
                    ) >= 0.5
                {
                    capture_stage(&session, "halfway");
                    halfway_captured = true;
                }
                session
                    .advance_pending(
                        &board,
                        &mut openmt2001::pending_world::Controls {
                            network_enabled: false,
                            network_override: false,
                            effect_model_reference: 0,
                        },
                        |event, _| match event {
                            live_commands::PendingCall::World(
                                openmt2001::pending_world::Event::Progress { .. },
                            ) => Ok(0),
                            other => panic!("unexpected construction host effect {other:?}"),
                        },
                    )
                    .unwrap();
                stages.insert(u32::from_le_bytes(
                    session.world.buildings[slot][0x14..0x18]
                        .try_into()
                        .unwrap(),
                ));
            }
            assert_eq!(stages, [0, 1, 2, 4].into_iter().collect());
            #[cfg(feature = "capture")]
            capture_stage(&session, "completed");
            let completed_record = session.world.buildings[slot];
            let completion = session.world.clock.timing[0];
            for tick in completion..completion + 25 {
                session.world.clock.timing[0] = tick;
                session
                    .drain_commands(&assets, |_| panic!("business opened early"))
                    .unwrap();
                assert_eq!(session.world.buildings[slot][0x1c..0x20], [0xff; 4]);
            }
            session.world.clock.timing[0] = completion + 25;
            session
                .drain_commands(&assets, |e| {
                    events.push(e);
                    Ok(())
                })
                .unwrap();
            // Opening binds the business at +0x1c; it does not replace the
            // completed building's model, auxiliary pose, or growth scale.
            let opened_record = &session.world.buildings[slot];
            assert_eq!(&completed_record[..0x1c], &opened_record[..0x1c]);
            assert_eq!(&completed_record[0x20..], &opened_record[0x20..]);
            let linked = u32::from_le_bytes(
                session.world.buildings[slot][0x1c..0x20]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let row = &session.world.ledgers.businesses[linked];
            #[cfg(feature = "capture")]
            capture_stage(&session, "open");
            assert_eq!(row[0xa], 1);
            assert_ne!(row[0x21c], 0);
            assert_eq!(
                u16::from_le_bytes(row[8..10].try_into().unwrap()) as usize,
                slot
            );
            assert_eq!(
                session.world.ledgers.players[0].cash.to_bits(),
                expected_account.cash.to_bits(),
                "opening prepaid construction must not charge twice"
            );
            if kind == 1 && case != 3 {
                assert!(session.world.available.contains(&linked));
            }
            if case == 3 {
                assert!(
                    !session.world.available.contains(&linked),
                    "special Hotel keeps its original catalog availability class"
                );
            }
            if kind == 0 {
                assert!(
                    row[0x218..0x21c] != [0; 4],
                    "daytime retail opens with stock"
                );
            }
            assert!(
                session.population.shopping.nodes[category]
                    .iter()
                    .any(|node| node.building as usize == slot)
            );
            let before_count = u32::from_le_bytes(
                session.world.categories[category][8..12]
                    .try_into()
                    .unwrap(),
            );
            let before_business_count = session.roster.category_counts[category * 45 + business];
            let removal_cost = 123.5;
            expected_account.post(-removal_cost, 1).unwrap();
            session
                .submit_command(
                    live_commands::Command::Demolish(openmt2001::demolition::Command {
                        category: category as i8,
                        building: slot as u16,
                        amount: removal_cost,
                    }),
                    &assets,
                    |_| panic!("demolition dispatched immediately"),
                )
                .unwrap();
            let demolition_start = session.world.clock.timing[0];
            for tick in demolition_start..demolition_start + 25 {
                session.world.clock.timing[0] = tick;
                session
                    .drain_commands(&assets, |_| panic!("demolition dispatched early"))
                    .unwrap();
                assert_ne!(session.world.ledgers.businesses[linked][0x21c], 0);
            }
            session.world.clock.timing[0] = demolition_start + 25;
            session
                .drain_commands(&assets, |e| {
                    events.push(e);
                    Ok(())
                })
                .unwrap();
            assert_eq!(session.world.ledgers.businesses[linked][0x21c], 0);
            assert_eq!(session.world.buildings[slot][0x1c..0x20], [0xff; 4]);
            assert!(!session.world.available.contains(&linked));
            assert_eq!(session.population.shopping.building_businesses[slot], None);
            assert_eq!(
                session.roster.category_counts[category * 45 + business],
                before_business_count - 1
            );
            assert_eq!(
                session.world.ledgers.players[0].cash.to_bits(),
                expected_account.cash.to_bits()
            );
            session
                .building_selection
                .fill(0x591318 + (slot * 0x394) as u32);
            let mut removal_ticks = 0;
            let mut released = false;
            while session.world.pending_world.count != 0 {
                removal_ticks += 1;
                assert!(removal_ticks < 500);
                session.world.clock.timing[0] += 1;
                session
                    .advance_pending(
                        &board,
                        &mut openmt2001::pending_world::Controls {
                            network_enabled: false,
                            network_override: false,
                            // Diagnostic host deliberately has no particle model allocation.
                            effect_model_reference: 0,
                        },
                        |call, _record| match call {
                            live_commands::PendingCall::World(
                                openmt2001::pending_world::Event::Progress { .. },
                            )
                            | live_commands::PendingCall::World(
                                openmt2001::pending_world::Event::CreateEffect { .. },
                            )
                            | live_commands::PendingCall::ReleaseEffect { .. }
                            | live_commands::PendingCall::Refresh => Ok(0),
                            live_commands::PendingCall::ReleaseBuilding { building, .. } => {
                                assert_eq!(building, slot);
                                released = true;
                                Ok(0)
                            }
                            other => panic!("unexpected demolition host boundary {other:?}"),
                        },
                    )
                    .unwrap();
            }
            assert!(released);
            assert_eq!(session.world.buildings[slot][0x3f], 0);
            assert_eq!(
                session.world.buildings[slot][0x14..0x18],
                5u32.to_le_bytes()
            );
            assert_eq!(session.building_selection, [0; 2]);
            assert!(!session.models.bindings.contains_key(&slot));
            assert_eq!(
                u32::from_le_bytes(
                    session.world.categories[category][8..12]
                        .try_into()
                        .unwrap()
                ),
                before_count - 1
            );
            assert!(building_pool::can_place(
                session.world.categories[category][0x9c..0xcd]
                    .try_into()
                    .unwrap(),
                rectangle
            ));
            assert_eq!(
                building_pool::first_free(&session.world.buildings).unwrap(),
                Some(slot)
            );
            let hour = session.world.clock.hour();
            live_commands::quote(
                &mut command,
                &mut session.world.city,
                &session.roster,
                &image,
                hour,
            )
            .unwrap();
            session
                .submit_command(live_commands::Command::Build(command), &assets, |_| {
                    panic!("rebuild dispatched immediately")
                })
                .unwrap();
            session.world.clock.timing[0] += 25;
            session
                .drain_commands(&assets, |e| {
                    events.push(e);
                    Ok(())
                })
                .unwrap();
            assert_ne!(session.world.buildings[slot][0x3f], 0);
            assert!(session.models.bindings.contains_key(&slot));
            assert_eq!(session.world.pending_world.count, 1);
            assert!(!building_pool::can_place(
                session.world.categories[category][0x9c..0xcd]
                    .try_into()
                    .unwrap(),
                rectangle
            ));
            // Finish the rebuilt business through production dispatch alongside
            // residents, traffic and original scripts, not a second test scheduler.
            let mut policy = tick::Policy {
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
            let mut simulated = 0;
            while session.world.buildings[slot][0x1c..0x20] == [0xff; 4] {
                simulated += 1;
                assert!(simulated < 500);
                if session.world.pause.has_reason() {
                    session
                        .dispatch_session_event("InterfacePromptDismiss", 0, &assets, false)
                        .unwrap();
                }
                session
                    .advance_session(&assets, &mut policy, |event, _| match event {
                        tick::Event::Tick(openmt2001::scheduler::Event::Helper(
                            0x4ed6f0 | 0x4ed6e0 | 0x481340 | 0x44ec50 | 0x4805b0 | 0x4eb5b0
                            | 0x450080 | 0x4b5db0 | 0x4b5d50 | 0x477f20,
                        ))
                        | tick::Event::Tick(openmt2001::scheduler::Event::BeforeSixSound)
                        | tick::Event::Pending(
                            live_commands::PendingCall::World(
                                openmt2001::pending_world::Event::Progress { .. },
                            ),
                            _,
                        )
                        | tick::Event::Command(_)
                        | tick::Event::TransportSound(_)
                        | tick::Event::TransportEnvironment { .. }
                        | tick::Event::Script { .. }
                        | tick::Event::Player(openmt2001::player_tick::Event::Ui { .. }) => Ok(0),
                        other => Err(format!("unbound lifecycle session effect {other:?}")),
                    })
                    .unwrap();
            }
            assert_eq!(session.world.pending_world.count, 0);
            assert_eq!(
                session.world.buildings[slot][0x14..0x18],
                4u32.to_le_bytes()
            );
            assert!(session.population.shopping.building_businesses[slot].is_some());
            println!(
                "rebuilt case={case} kind={kind} opened through production session after {simulated} ticks"
            );
            println!(
                "demolition case={case} kind={kind}: {removal_ticks} pending ticks, owner debit, detach, cleared occupancy/bounds/selection and reused slot verified"
            );
            println!(
                "construction case={case} kind={kind} category={category} building={slot} business={linked}: quote={quote}, {steps} pending ticks, both25-tick command delays, four cash posts and live topology verified"
            );
        }
    }

    #[test]
    #[ignore = "requires original data and GPU; natural Scenario1 reaches its first terminal result"]
    fn natural_ai_reaches_terminal_loss() {
        run_unrestricted_ai(72, true, false, true, 0, false, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; unrestricted opponents through a full day"]
    fn unrestricted_ai_completes_one_day() {
        run_unrestricted_ai(24, true, false, false, 0, false, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; legal human planning through the following trading day"]
    fn human_retail_opens_and_trades() {
        run_unrestricted_ai(48, true, true, false, 0, false, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; measured two-shop Scenario1 loss"]
    fn competitive_two_shop_loss() {
        run_unrestricted_ai(72, true, true, false, 2, false, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; measured three-shop Scenario1 result"]
    fn competitive_diversified_scenario_one() {
        run_unrestricted_ai(72, true, true, false, 3, false, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; legal scaled retail through Scenario1 result"]
    fn competitive_scaled_retail_scenario_one() {
        run_unrestricted_ai(72, true, true, false, 8, false, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; five observed-selling shops through Scenario1 result"]
    fn competitive_five_shop_scenario_one() {
        run_unrestricted_ai(72, true, true, false, 5, false, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; five-shop value-informed visible auction bidding"]
    fn competitive_five_shop_auction_scenario_one() {
        run_unrestricted_ai(72, true, true, false, 5, true, 0);
    }
    #[test]
    #[ignore = "requires original data and GPU; legal enlarged stockout shops through Scenario1 result"]
    fn competitive_five_shop_capacity_scenario_one() {
        run_unrestricted_ai(72, true, true, false, 5, false, 1);
    }
    #[test]
    #[ignore = "requires original data and GPU; legal five-shop Scenario1 human win"]
    fn competitive_five_shop_further_capacity_scenario_one() {
        run_unrestricted_ai(72, true, true, false, 5, false, 2);
    }
    #[test]
    #[ignore = "requires original data and GPU; natural Win through retained result and V2 profile"]
    fn competitive_five_shop_win_profile_scenario_one() {
        run_unrestricted_ai(72, true, true, false, 5, false, 3);
    }
    #[test]
    #[ignore = "requires original data, GPU, and retained Scenario1 profile; genuine three-election mayor win remains open"]
    fn natural_scenario_six_mayor_objective_frontier() {
        run_unrestricted_ai(144, true, true, false, 5, false, 4);
    }
    #[test]
    #[ignore = "requires original data and GPU"]
    fn natural_scenario_eleven_sales_frontier() {
        run_unrestricted_ai(72, false, true, false, 5, false, 5);
    }
    fn try_human_bakery(
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        excluded: &[u32],
        reserve: f32,
        competitive_limit: usize,
        capacity_mode: u8,
    ) -> Result<Option<u32>, String> {
        let competitive = competitive_limit != 0;
        let mut categories = (0..52).collect::<Vec<_>>();
        categories.sort_by_key(|&i| {
            std::cmp::Reverse(u32::from_le_bytes(
                session.world.categories[i][0x88..0x8c].try_into().unwrap(),
            ))
        });
        let businesses = if competitive_limit == 5 {
            let ranked = [20, 21, 11, 13, 26];
            ranked[excluded.len().min(ranked.len())..].to_vec()
        } else if competitive_limit == 8 {
            let ranked = [20, 21, 11, 19, 13, 26, 27, 17];
            ranked[excluded.len().min(ranked.len())..].to_vec()
        } else if competitive {
            match excluded.len() {
                0 => vec![20],
                1 => vec![21],
                2 => vec![13, 26, 11],
                _ => vec![],
            }
        } else {
            vec![0]
        };
        for business in businesses {
            let info = session.world.catalog.info.row(business).unwrap().to_vec();
            if info[0x30] == 0 {
                continue;
            }
            let catalog_word = |at| u32::from_le_bytes(info[at..at + 4].try_into().unwrap()) as i32;
            let minimum_height = catalog_word(0x48);
            let enlarge = capacity_mode != 0 && matches!(business, 20 | 13 | 26);
            let increment = if capacity_mode >= 2 && matches!(business, 13 | 26) {
                2
            } else {
                i32::from(enlarge)
            };
            let height = (minimum_height + increment).min(catalog_word(0x44));
            let minimum_area =
                catalog_word(0x38) + i32::from(enlarge && height < minimum_height + increment);
            for &category in &categories {
                if excluded.contains(&(category as u32)) {
                    continue;
                }
                if !matches!(
                    crate::build_panel::Context::from_session(session, category)?.entry(),
                    crate::build_panel::Entry::Open
                ) {
                    continue;
                }
                for width in 1..=6 {
                    for depth in 1..=6 {
                        if !competitive && (width, depth) != (2, 2) {
                            continue;
                        }
                        if width * depth < minimum_area
                            || width * depth > catalog_word(0x3c)
                            || width.max(depth) > catalog_word(0x40)
                        {
                            continue;
                        }
                        for x in 0..=(7 - width) {
                            for y in 0..=(7 - depth) {
                                let request = crate::build_panel::Request {
                                    category: category as i32,
                                    origin: [x, y],
                                    size: [width, depth],
                                    height,
                                    packed_model: 0,
                                    kind: 0,
                                    quality: 0,
                                    displayed_quote: 0.,
                                    business: business as u32,
                                };
                                let inspected = session.inspect_build(&request, assets)?;
                                if !inspected.allowed() {
                                    continue;
                                }
                                if session.world.ledgers.players[0].cash - inspected.total < reserve
                                {
                                    continue;
                                }
                                let form = crate::build_panel::Form {
                                    category: category as i32,
                                    rectangle: [x, y, x + width - 1, y + depth - 1],
                                    height,
                                    quality: 0,
                                    choices: [0; 3],
                                    variants: vec![0],
                                    kind: 0,
                                    business: business as u32,
                                    displayed_quote: inspected.total,
                                };
                                let request = form
                                    .request(
                                        inspected.category_locked,
                                        inspected.affordable,
                                        inspected.preview.is_some(),
                                    )?
                                    .map_err(|e| {
                                        format!("legal human construction form blocked: {e:?}")
                                    })?;
                                let result = session.submit_build(&request, assets, |event| {
                                    Err(format!("human build dispatched synchronously: {event:?}"))
                                })?;
                                assert!(result.submitted);
                                let rate = f32::from_le_bytes(info[0x58..0x5c].try_into().unwrap());
                                let capacity = openmt2001::economy::stock_capacity(
                                    width * depth,
                                    height as u8,
                                    rate,
                                );
                                if enlarge {
                                    let baseline = openmt2001::economy::stock_capacity(
                                        catalog_word(0x38),
                                        minimum_height as u8,
                                        rate,
                                    );
                                    assert!(
                                        capacity > baseline,
                                        "stockout shop did not gain original stock capacity"
                                    );
                                    if capacity_mode >= 2 && matches!(business, 13 | 26) {
                                        let prior = openmt2001::economy::stock_capacity(
                                            catalog_word(0x38),
                                            (minimum_height + 1) as u8,
                                            rate,
                                        );
                                        assert!(
                                            capacity > prior,
                                            "second legal increase did not exceed prior shop capacity"
                                        );
                                    }
                                }
                                println!(
                                    "Human original construction form submits business{business} on category{category}, lot[{x},{y}], size[{width},{depth}], height{height}, capacity{capacity}, quote{} at tick{}",
                                    result.total, session.world.clock.timing[0]
                                );
                                return Ok(Some(category as u32));
                            }
                        }
                    }
                }
            }
        }
        Ok(None)
    }
    fn run_unrestricted_ai(
        hours: u32,
        require_completion: bool,
        human_build: bool,
        expect_loss: bool,
        competitive_limit: usize,
        auction_value_bid: bool,
        capacity_mode: u8,
    ) {
        let competitive = competitive_limit != 0;
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(move || {
            let read=|key|fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable=read("OPENMT2001_EXE");let directory=read("OPENMT2001_ARCHIVE_DIR");
            let archive=read("OPENMT2001_ARCHIVE_BIN");let build=read("OPENMT2001_BUILD_FILE");
            let parameters=read("OPENMT2001_PARAMETERS");let timetable=read("OPENMT2001_TIMETABLE");
            let image=openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board=openmt2001::board::Board::from_v14(&image).unwrap();
            let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
            let settings=openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let exe_path=openmt2001::fixtures::path("EXE");
            let names=character_names(&image,&exe_path.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
            let scripts=settings.parent().unwrap().parent().unwrap();
            // Scenario6 (mode 4) and Scenario11 (mode 5) continue a campaign whose
            // earlier levels were won; build that profile instead of reading one.
            let natural_win_source=crate::campaign_store::checkpoint_profile(&image,scripts,if capacity_mode==5 {200011} else {200006});
            let profile_path=(capacity_mode>=3).then(|| {
                let nonce=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
                let root=std::path::PathBuf::from("work/validation").join(format!("natural-win-profile-{}-{nonce}",std::process::id()));
                crate::campaign_store::profile_path(&root,b"Natural Win Validation")
            });
            let mut next_level=None;
            let mut baseline_profile=None;
            let mut campaign_catalog=if let Some(path)=profile_path.as_ref() {
                if capacity_mode>=4 {
                    let bytes=fs::read(&natural_win_source).unwrap();
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path,&bytes).unwrap();
                    baseline_profile=Some(bytes.clone());
                    println!("Scenario6 copied natural Scenario1 Win profile source{} bytes{} into {}",natural_win_source.display(),bytes.len(),path.display());
                }
                let mut catalog=crate::campaign_store::load_catalog(&image,scripts).unwrap();
                crate::campaign_store::load_progress(&mut catalog,path).unwrap();
                if capacity_mode==3 {
                    let selected=catalog.find(200001).unwrap();
                    // Original GAMESTART registers Scenario3 first. This isolated
                    // V2 profile represents a player who already unlocked Scenario1.
                    catalog.unlock(200001);
                    crate::campaign_store::save_progress(&catalog,path).unwrap();
                    baseline_profile=Some(fs::read(path).unwrap());
                    let mut loaded=crate::campaign_store::load_catalog(&image,scripts).unwrap();
                    crate::campaign_store::load_progress(&mut loaded,path).unwrap();
                    catalog=loaded;
                    assert!(catalog.levels[selected].unlocked(),"loaded profile did not unlock Starting Out");
                    next_level=Some(catalog.levels[selected+1..].iter().find(|level|!level.tutorial()).unwrap().id());
                }else{
                    let prior=&catalog.levels[catalog.find(200001).unwrap()];
                    assert_eq!(prior.attempts(),1,"source profile lacks natural Scenario1 Win attempt");
                    assert!(prior.completed()[0],"source profile lacks natural Scenario1 completion");
                    assert!(catalog.levels[catalog.find(200006).unwrap()].unlocked(),"source profile lacks Scenario6 entitlement");
                    if capacity_mode==5 {
                        let mayor=&catalog.levels[catalog.find(200006).unwrap()];
                        assert_eq!(mayor.attempts(),1,"source profile lacks natural Scenario6 Win attempt");
                        assert!(mayor.completed()[0],"source profile lacks natural Scenario6 completion");
                        assert!(catalog.levels[catalog.find(200011).unwrap()].unlocked(),"source profile lacks Scenario11 entitlement");
                    }
                }
                Some(catalog)
            }else{None};
            let scenario=match capacity_mode {4=>"SCENARIO6",5=>"SCENARIO11",_=>"SCENARIO1"};
            let input=Inputs{
                tree_locators: None,
                image:&image,board:&board,file:&file,
                people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
                vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
                transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
                road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts,
                parameters:std::str::from_utf8(&parameters).unwrap(),
                era_styles:load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),timetable:std::str::from_utf8(&timetable).unwrap(),
                scenario,difficulty:0,selected_character:4,profile_name:b"AI frontier",character_names:&names,
            };
            let policy=Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]};
            let mut session=if let Some(path)=profile_path.as_ref(){
                Session::initialize_campaign(input,policy,campaign_catalog.take().unwrap(),path.clone()).unwrap()
            }else{Session::initialize(input,policy).unwrap()};
            if capacity_mode==4 {
                assert_eq!(session.world.objective.kind,1,"Scenario6 did not select original mayor objective");
                assert_eq!(session.world.objective.target,3.,"Scenario6 mayor target changed");
                assert_eq!(session.roster.manager.players.len(),4,"Scenario6 easy roster changed");
                println!("Scenario6 starts: objective kind{} target{} roster{} population{} cash{:?}",session.world.objective.kind,session.world.objective.target,session.roster.manager.players.len(),session.world.ledgers.residents.active_count,session.world.ledgers.players.iter().map(|p|p.cash).collect::<Vec<_>>());
            }
            if capacity_mode==5 {
                assert_eq!(session.world.objective.kind,6,"Scenario11 did not select original sales objective");
                assert_eq!(session.world.objective.target,3000.,"Scenario11 sales target changed");
                assert_eq!(session.roster.manager.players.len(),5,"Scenario11 easy roster changed");
                assert_eq!(session.world.ledgers.residents.active_count,440,"Scenario11 initial population changed");
                println!("Scenario11 starts: objective kind{} target{} roster{} population{} active-businesses{} cash{:?}",session.world.objective.kind,session.world.objective.target,session.roster.manager.players.len(),session.world.ledgers.residents.active_count,session.world.ledgers.businesses.iter().filter(|b|b[0x21c]!=0).count(),session.world.ledgers.players.iter().map(|p|p.cash).collect::<Vec<_>>());
            }
            let assets=CommandAssets{image:&image,board:&board,file:&file};
            let (sender,receiver)=std::sync::mpsc::channel::<crate::auction_bridge::Request>();
            let presenter=std::thread::spawn(move || {
                let exe=read("OPENMT2001_EXE");let directory=read("OPENMT2001_ARCHIVE_DIR");let archive=read("OPENMT2001_ARCHIVE_BIN");
                let mut descriptor=wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows){descriptor.backends=wgpu::Backends::DX12;}
                let renderer=pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                let mut view=crate::auction_bridge::View::default();let mut counts=[0;3];
                while let Ok(request)=receiver.recv(){
                    let operation=match &request.operation {
                        crate::auction_bridge::Operation::Prepare{..}=>0,
                        crate::auction_bridge::Operation::Activate=>1,
                        crate::auction_bridge::Operation::Restore=>2,
                    };
                    let result=view.present(request.operation,&renderer,(&exe,&directory,&archive),0);
                    if result.is_ok(){counts[operation]+=1;}
                    request.reply.send(result).unwrap();
                }
                (counts,view.scene.is_some())
            });
            let mut host=crate::live_host::Host::new([0.,20.,0.]);host.auction_renderer=Some(sender);host.set_running(true);
            let idle_prompts=|host:&mut crate::live_host::Host,session:&mut Session<'_>,prompts:Vec<crate::prompt::Request>| -> Option<u32> {
                let mut result_context=None;
                for prompt in prompts {
                    if let Some(dialog)=prompt.dialog {
                        let button=if dialog.button_count==2{1}else{0};
                        println!("Idle human modal response: callback{:x} context{} button{}",dialog.callback,dialog.context,button);
                        host.human_dialog_reply(session,&assets,crate::human_dialog::Reply{callback:dialog.callback,context:dialog.context,button}).unwrap();
                        if dialog.callback==0x48b490 {result_context=Some(dialog.context);}
                    } else if prompt.dismissal_id!=0 {
                        session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,false).unwrap();
                    }
                }
                result_context
            };
            let start_hour=session.world.clock.calendar[4]*24+session.world.clock.hour();
            let mut human_withdrawals=0;let mut previous_hour=session.world.clock.hour();let mut reached_day=false;
            let mut prior_auction=false;let mut auction_rooms=[0;2];
            let mut human_build_category=None;
            let mut competitive_categories=Vec::new();
            let mut human_terms=None;
            let mut human_terms_applied=false;
            let mut human_restocked=false;
            let mut terminal_context=None;
            let mut human_retail_ever_traded=false;
            let mut ai_admissions = std::collections::BTreeSet::new();
            println!("Unrestricted AI with real auction GPU presenter; human acknowledges notices, refuses offers and uses enabled auction controls");
            println!("Initial DELI stockcapacity {}", f32::from_le_bytes(session.world.catalog.info.row(2).unwrap()[0x58..0x5c].try_into().unwrap()));
            for frame in 0..hours.saturating_mul(2500) {
                if human_build&&frame%625==0&&(competitive&&competitive_categories.len()<competitive_limit||!competitive&&human_build_category.is_none()){
                    let built=try_human_bakery(&mut session,&assets,&competitive_categories,if competitive{1600.}else{0.},competitive_limit,capacity_mode).unwrap();
                    if let Some(category)=built {competitive_categories.push(category);human_build_category=Some(category);}
                }
                if human_build&&!competitive&&human_terms.is_none()&&session.world.clock.calendar[4]>=1&&session.world.clock.hour()>=11{
                    let word=|b:&[u8],at|u32::from_le_bytes(b[at..at+4].try_into().unwrap());
                    if let Some(business)=session.world.ledgers.businesses.iter().position(|b|b[0x21c]!=0&&b[10]==1&&word(b,0)==0&&f32::from_bits(word(b,0xa4))>0.){
                        let row=session.world.ledgers.businesses[business];
                        let target=crate::business_edit::Target{business,building:usize::from(u16::from_le_bytes(row[8..10].try_into().unwrap()))};
                        let old_price=word(&row,0x1c4);let old_target=u16::from_le_bytes(row[0x1b8..0x1ba].try_into().unwrap());
                        crate::business_edit::edit(&mut session,&assets,target,crate::business_edit::Action::Price{slot:0,increase:true}).unwrap();
                        let draft=crate::business_edit::edit(&mut session,&assets,target,crate::business_edit::Action::Stock{slot:0,increase:false}).unwrap();
                        let expected_price=word(&draft,0x1ec);let expected_target=u16::from_le_bytes(draft[0x1e0..0x1e2].try_into().unwrap());
                        assert_ne!(expected_price,old_price);assert_ne!(expected_target,old_target);
                        crate::business_edit::edit(&mut session,&assets,target,crate::business_edit::Action::Apply).unwrap();
                        let due=session.world.clock.timing[0]+25;
                        human_terms=Some((target,due,old_price,old_target,expected_price,expected_target));
                        println!("Human submits original price/stock arrows and Apply at tick{}, due{due}",session.world.clock.timing[0]);
                    }
                }
                let prompts=host.take_prompts(&session);
                terminal_context=idle_prompts(&mut host,&mut session,prompts).or(terminal_context);
                if terminal_context.is_some(){break;}
                host.frame(&mut session,&assets,40);
                for (_, due, inner, direct, queued) in session.live_commands.snapshot().unwrap() {
                    let openmt2001::live_commands::Command::Build(command) = queued else { continue };
                    let owner = &session.roster.manager.players[(i32::from(command.owner)-1) as usize];
                    if u32::from_le_bytes(owner.bytes[0x154..0x158].try_into().unwrap()) != 1 { continue; }
                    assert!(!direct); assert_eq!(due,inner);
                    let [x0,y0,x1,y1] = command.rectangle.map(i32::from);
                    //Independent native45184e/445b90 stores, observed before
                    //the actual due consumer. This is not full J2 equality.
                    for x in x0..=x1 { for y in y0..=y1 {
                        assert_eq!(session.world.categories[command.category as usize]
                            [0x9c+(x*7+y) as usize], command.height as u8,
                            "real AI producer reached queue without native footprint reservation");
                    }}
                    if command.height > 0 { ai_admissions.insert((due,command.packet().to_vec())); }
                }
                if let Some((target,due,old_price,old_target,expected_price,expected_target))=human_terms{
                    let row=&session.world.ledgers.businesses[target.business];
                    let observed=(u32::from_le_bytes(row[0x1c4..0x1c8].try_into().unwrap()),u16::from_le_bytes(row[0x1b8..0x1ba].try_into().unwrap()));
                    if session.world.clock.timing[0]<due{assert_eq!(observed,(old_price,old_target),"human business terms applied before native queue deadline");}
                    else if !human_terms_applied{
                        assert_eq!(observed,(expected_price,expected_target),"human business terms did not reach live consumer at native deadline");
                        human_terms_applied=true;
                        println!("Human price/stock terms applied at tick{}: price{} target{}",session.world.clock.timing[0],f32::from_bits(expected_price),expected_target);
                    }
                    if !human_restocked&&session.world.clock.calendar[4]>=2&&session.world.clock.hour()>=6{
                        let stock=u16::from_le_bytes(row[0x80..0x82].try_into().unwrap());
                        let cost=f32::from_le_bytes(row[0x30..0x34].try_into().unwrap());
                        assert_eq!(stock,expected_target,"next real morning replenishment did not use the changed target");
                        assert!(cost<0.,"real replenishment did not charge the human business");
                        human_restocked=true;
                        println!("Human next-morning replenishment reached changed target{stock}, stock expense{cost}, tick{}",session.world.clock.timing[0]);
                    }
                }
                let mut status=host.status(&session);
                terminal_context=idle_prompts(&mut host,&mut session,std::mem::take(&mut status.prompts)).or(terminal_context);
                if terminal_context.is_some(){break;}
                let active_auction=status.auction.as_ref().is_some_and(|a|a.visible);
                if active_auction!=prior_auction {
                    auction_rooms[usize::from(!active_auction)]+=1;
                    println!("Natural auction room {} at tick{}",if active_auction {"opened"}else{"closed"},session.world.clock.timing[0]);
                    if active_auction && auction_value_bid {
                        let category=u32::from_le_bytes(session.world.auction.controller.record[4..8].try_into().unwrap()) as usize;
                        let value=openmt2001::ai_messages::offers::execution::category_value(&session.world.categories[category],&image).unwrap().to_f32();
                        println!("Original auction category{category} paper-wealth value{value} human cash{}",session.world.ledgers.players[0].cash);
                    }
                    prior_auction=active_auction;
                }
                if status.auction.as_ref().is_some_and(|a|a.visible && a.controls.buttons_enabled&8!=0) {
                    let price=u32::from_le_bytes(session.world.auction.controller.record[0x6c..0x70].try_into().unwrap());
                    let bid=if auction_value_bid {
                        let category=u32::from_le_bytes(session.world.auction.controller.record[4..8].try_into().unwrap()) as usize;
                        let value=openmt2001::ai_messages::offers::execution::category_value(&session.world.categories[category],&image).unwrap().to_f32();
                        let next=price.saturating_add(50);
                        value>=next as f32+350. && session.world.ledgers.players[0].cash>next as f32+1600.
                    }else{competitive&&price<=800&&session.world.ledgers.players[0].cash>price as f32+2100.};
                    let button=if bid{0}else{3};
                    host.auction_button(&mut session,&assets,button).unwrap();
                    if !bid{human_withdrawals+=1;}
                    println!("Human auction button{button} at tick{} price{price}",session.world.clock.timing[0]);
                }
                if session.world.clock.hour()!=previous_hour {
                    previous_hour=session.world.clock.hour();
                    crate::session_validation::assert_live_links(&session);
                    if competitive {
                        human_retail_ever_traded|=session.world.ledgers.businesses.iter().any(|b|b[0x21c]!=0&&b[10]==1&&b[..4]==[0;4]&&(0..6).any(|i|{
                            let current=f32::from_le_bytes(b[0xa4+i*0x34..0xa8+i*0x34].try_into().unwrap());
                            let previous=f32::from_le_bytes(b[0xa8+i*0x34..0xac+i*0x34].try_into().unwrap());
                            current>0.||previous>0.
                        }));
                    }
                    let allocations=(1..=session.roster.manager.players.len()).map(|owner|session.world.ledgers.businesses.iter().filter(|b|b[0x21c]!=0&&usize::from(b[10])==owner).count()).collect::<Vec<_>>();
                    let cash=session.world.ledgers.players.iter().map(|p|p.cash).collect::<Vec<_>>();
                    println!("Unrestricted progress: frame{frame} tick{} day{} hour{} allocations-by-owner{allocations:?} cash{cash:?} auction-open/closed{auction_rooms:?}",session.world.clock.timing[0],session.world.clock.calendar[4],previous_hour);
                    if capacity_mode==4&&matches!(previous_hour,0|14|23) {
                        let political=session.roster.manager.players.iter().enumerate().map(|(slot,p)|{
                            let signed=|at|i32::from_le_bytes(p.bytes[at..at+4].try_into().unwrap());
                            let marker=session.roster.manager.bytes[0x0d+slot*8];
                            (signed(0x1b4),signed(0x14c),signed(0x1ac),signed(0x1b0),marker,p.bytes[0x10])
                        }).collect::<Vec<_>>();
                        println!("Scenario6 mayor objective tick{} day{} hour{} kind{} target{} reached{:?} population{} player(score1b4,streak14c,income1ac,income1b0,marker,active){political:?}",session.world.clock.timing[0],session.world.clock.calendar[4],previous_hour,session.world.objective.kind,session.world.objective.target,session.world.objective.reached,session.world.ledgers.residents.active_count);
                    }
                    if capacity_mode==5&&matches!(previous_hour,0|14|23) {
                        let sold=session.roster.manager.players.iter().map(|p|i32::from_le_bytes(p.bytes[0x1b4..0x1b8].try_into().unwrap())).collect::<Vec<_>>();
                        println!("Scenario11 sales objective tick{} day{} hour{} target{} reached{:?} population{} sold-units{sold:?}",session.world.clock.timing[0],session.world.clock.calendar[4],previous_hour,session.world.objective.target,session.world.objective.reached,session.world.ledgers.residents.active_count);
                    }
                    if competitive && previous_hour==0 {
                        let values=(0..session.roster.manager.players.len()).map(|slot|{
                            let prefix:&[u8;440]=session.roster.manager.players[slot].bytes[4..444].try_into().unwrap();
                            openmt2001::player_metrics::runtime_metric(&session.world.city,prefix,slot,0x47fbb0,&image,0x591318).unwrap().to_f32()
                        }).collect::<Vec<_>>();
                        println!("Natural original paper-wealth metric at day{}: {values:?}",session.world.clock.calendar[4]);
                        if competitive_limit==5 {
                            for (slot,b) in session.world.ledgers.businesses.iter().enumerate().filter(|(_,b)|b[0x21c]!=0&&b[10]==1&&b[..4]==[0;4]) {
                                let kind=u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
                                let count=usize::from(session.world.catalog.info.row(kind).unwrap()[0x4c]);
                                let prior=(usize::from(b[0x209])+3)%4;
                                let products=(0..count).map(|i|{
                                    let stock=u16::from_le_bytes(b[0x80+i*0x34..0x82+i*0x34].try_into().unwrap());
                                    let target=i16::from_le_bytes(b[0x1b8+i*2..0x1ba+i*2].try_into().unwrap());
                                    let turnover=u16::from_le_bytes(b[0x8a+i*0x34+prior*2..0x8c+i*0x34+prior*2].try_into().unwrap());
                                    let revenue=f32::from_le_bytes(b[0xa8+i*0x34..0xac+i*0x34].try_into().unwrap());
                                    (stock,target,turnover,revenue)
                                }).collect::<Vec<_>>();
                                let building=usize::from(u16::from_le_bytes(b[8..10].try_into().unwrap()));
                                let model=&session.world.buildings[building];
                                let area=(i32::from(model[6])-i32::from(model[4])+1)*(i32::from(model[7])-i32::from(model[5])+1);
                                let rate=f32::from_le_bytes(session.world.catalog.info.row(kind).unwrap()[0x58..0x5c].try_into().unwrap());
                                let capacity=openmt2001::economy::stock_capacity(area,model[8],rate);
                                println!("Human inventory day{} slot{slot} kind{kind}: capacity{capacity} stock,target,prior-turnover,prior-revenue{products:?} cost-balance{}",session.world.clock.calendar[4],f32::from_le_bytes(b[0x30..0x34].try_into().unwrap()));
                            }
                        }
                    }
                    if human_build&&!competitive{
                        for b in session.world.ledgers.businesses.iter().filter(|b|b[0x21c]!=0&&b[10]==1&&b[..4]==[0;4]){
                            let word=|at|u32::from_le_bytes(b[at..at+4].try_into().unwrap());
                            let bread=session.world.catalog.products[0][0][0];
                            let demand=openmt2001::ai_actions::demand::housed_demand(bread,&session.world.ledgers.residents,&session.world.commodities.rows).unwrap();
                            println!("Human bakery state: closed{} stock{} price{} current-product-revenue{} housed-bread-demand{demand}",word(0x6c),u16::from_le_bytes(b[0x80..0x82].try_into().unwrap()),f32::from_bits(word(0x1c4)),f32::from_bits(word(0xa4)));
                        }
                    }
                }
                if let Some(error)=&host.failure {
                    println!("Unrestricted AI frontier at tick{} hour{}: {error}",session.world.clock.timing[0],session.world.clock.hour());
                    println!("Frontier DELI stockcapacity {}", f32::from_le_bytes(session.world.catalog.info.row(2).unwrap()[0x58..0x5c].try_into().unwrap()));
                    for (slot,p) in session.roster.manager.players.iter().enumerate(){
                        if p.bytes.len()<0x1e50 {continue;}
                        let word=|at|u32::from_le_bytes(p.bytes[at..at+4].try_into().unwrap());
                        let businesses=session.world.ledgers.businesses.iter().filter(|b|b[0x21c]!=0&&usize::from(b[10])==slot+1).count();
                        println!("player{slot}: strategy{} initial{} kind{} category{} business-kind{} flags{:x} cash{} reserve{} active-businesses{businesses}",word(0xb54),p.bytes[0x13dc],word(0xb64),word(0xb68),word(0xb90),word(0x220),f32::from_bits(word(4)),f32::from_bits(word(0xcac)));
                    }
                    break;
                }
                if session.world.clock.calendar[4]*24+session.world.clock.hour()>=start_hour+hours {reached_day=true;break;}
            }
            host.auction_renderer.take();
            let (counts,scene_active)=presenter.join().unwrap();
            println!("Unrestricted {scenario} run ended: tick{} requested-hours{hours} complete{reached_day} terminal-context{terminal_context:?} result{:?} failure{:?} GPU prepare/activate/restore{counts:?} scene-active{scene_active} idle-human-withdrawals{human_withdrawals}",session.world.clock.timing[0],host.result_sequence(),host.failure);
            let word=|bytes:&[u8],at|u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
            let mut traded_retail=0;let mut human_retail_traded=false;
            for (slot,business) in session.world.ledgers.businesses.iter().enumerate().filter(|(_,b)|b[0x21c]!=0) {
                let building=i16::from_le_bytes(business[8..10].try_into().unwrap());
                let record=session.world.buildings.get(building as usize);
                println!("Active allocation{slot}: type{} kind{} owner{} building{building} open-byte{} building-link{:?} constructed{:?} pending{:?}",word(business,0),word(business,4),business[10],business[0x21c],record.map(|r|word(r,0x1c)),record.map(|r|word(r,0x4c)),record.map(|r|r[0x3f]));
                if word(business,0)==0 {
                    let count=usize::from(session.world.catalog.info.row(word(business,4) as usize).unwrap()[0x4c]);
                    let revenue=(0..count).map(|i|[f32::from_bits(word(business,0xa4+i*0x34)),f32::from_bits(word(business,0xa8+i*0x34))]).collect::<Vec<_>>();
                    traded_retail+=usize::from(revenue.iter().flatten().any(|amount|*amount>0.));
                    if business[10]==1&&revenue.iter().flatten().any(|amount|*amount>0.){human_retail_traded=true;}
                    println!("Retail allocation{slot} revenue by product [current,previous]: {revenue:?}");
                }
            }
            for (slot,account) in session.world.ledgers.players.iter().enumerate() {
                let ledgers=(0..38).filter_map(|event|account.ledger(event).filter(|v|*v!=0.).map(|v|(event,v))).collect::<Vec<_>>();
                println!("Final account{slot}: cash{} nonzero-ledgers{ledgers:?}",account.cash);
            }
            if require_completion{
                if hours == 24 && !human_build {
                    assert!(!ai_admissions.is_empty(), "natural AI admission regression needs a real positive-height packet");
                }
                assert!(host.failure.is_none(),"Unrestricted opponents stopped: {:?}",host.failure);
                if expect_loss {
                    assert_eq!(terminal_context,Some(3),"natural outcome callback context changed");
                    assert_eq!(host.result_sequence(),Some(openmt2001::outcome_notice::Sequence::Lose));
                    assert!(!host.running,"normal simulation continued during original result sequence");
                    let stopped=session.world.clock.timing[0];
                    for _ in 0..10 {host.frame(&mut session,&assets,40);}
                    assert_eq!(session.world.clock.timing[0],stopped,"result did not freeze normal logic clock");
                    assert!(host.failure.is_none(),"result presentation failed: {:?}",host.failure);
                } else if competitive {
                    let winner=terminal_context.expect("legal competitive run did not reach the original result callback");
                    if capacity_mode>=2 {
                        assert_eq!(winner,0,"legal five-shop strategy did not produce a human Win");
                        assert_eq!(host.result_sequence(),Some(openmt2001::outcome_notice::Sequence::Win),"human win sequence was not presented");
                        assert_eq!(competitive_categories.len(),5,"five legal original-form builds were not submitted");
                        assert!(human_retail_ever_traded,"human shops did not record revenue before the win");
                    }
                    if competitive_limit==2 {
                        assert_eq!(winner,2,"measured two-shop 1945 winner changed");
                        assert_eq!(session.world.clock.timing[0],40175,"measured two-shop result timing changed");
                    }
                    let expected=if winner==0 {openmt2001::outcome_notice::Sequence::Win}else{openmt2001::outcome_notice::Sequence::Lose};
                    assert_eq!(host.result_sequence(),Some(expected),"result sequence disagreed with winning player");
                    println!("Measured competitive {scenario} result: winner slot{winner}, human victory={}, day{}, tick{}",winner==0,session.world.clock.calendar[4],session.world.clock.timing[0]);
                } else {
                    assert!(reached_day,"Unrestricted opponents did not complete{hours} hours");
                }
                assert!(traded_retail>0,"Full day completed without an observed retail sale");
                assert!(!scene_active,"Auction presentation remained active after completed day");
                if human_build{
                    assert!(human_build_category.is_some(),"No legal human bakery construction was submitted");
                    if capacity_mode!=0 {assert_eq!(competitive_categories.len(),5,"five legal shops were not admitted after capacity changes");}
                    assert!(if competitive{human_retail_ever_traded}else{human_retail_traded},"Human retail never recorded a sale before outcome");
                    if !competitive {
                        assert!(human_terms_applied,"Human price/stock terms were not applied through live command queue");
                        assert!(human_restocked,"Human stock changes were not exercised through next-morning replenishment");
                    }
                }
                if capacity_mode==3 {
                    let path=profile_path.as_ref().unwrap();
                    let selected=session.catalog.selected_level().unwrap();
                    assert_eq!(selected.id(),200001);
                    assert_eq!(selected.attempts(),0,"profile advanced before original result hub completed");
                    assert_eq!(fs::read(path).unwrap(),*baseline_profile.as_ref().unwrap(),"profile changed before original result completion");
                    assert_eq!(session.result_presentation().unwrap().sequence,openmt2001::outcome_notice::Sequence::Win);
                    assert_eq!(session.result_presentation().unwrap().end_requested,None);
                    assert!(host.complete_result_sequence(&mut session).is_err(),"profile advanced before original Game.End");
                    session.dispatch_result_event("FlybyEnded",&assets).unwrap();
                    assert!(host.complete_result_sequence(&mut session).is_err(),"profile advanced before original CutSceneEnded");
                    assert_eq!(fs::read(path).unwrap(),*baseline_profile.as_ref().unwrap(),"profile changed before original CutSceneEnded");
                    session.dispatch_result_event("CutSceneEnded",&assets).unwrap();
                    assert_eq!(session.result_presentation().unwrap().end_requested,Some(1),"original Win hub did not request Game.End(1)");
                    assert_eq!(fs::read(path).unwrap(),*baseline_profile.as_ref().unwrap(),"profile changed before Host committed original Game.End");
                    assert_eq!(host.complete_result_sequence(&mut session).unwrap(),openmt2001::outcome_notice::Sequence::Win);
                    assert!(host.complete_result_sequence(&mut session).is_err(),"profile result committed more than once");
                    assert_eq!(session.catalog.selected_level().unwrap().attempts(),1);
                    assert!(session.catalog.selected_level().unwrap().completed()[0]);
                    let next=next_level.unwrap();
                    assert!(session.catalog.levels[session.catalog.find(next).unwrap()].unlocked());
                    let mut reloaded=crate::campaign_store::load_catalog(&image,scripts).unwrap();
                    crate::campaign_store::load_progress(&mut reloaded,path).unwrap();
                    let level=&reloaded.levels[reloaded.find(200001).unwrap()];
                    assert_eq!(level.attempts(),1,"natural win attempt did not survive V2 profile reload");
                    assert!(level.completed()[0],"natural win completion did not survive V2 profile reload");
                    assert!(reloaded.levels[reloaded.find(next).unwrap()].unlocked(),"next Scenario did not unlock on V2 profile reload");
                    println!("Natural Win result hub and V2 profile reload: end_requested{:?}, attempts{}, completed{:?}, next-level{next} unlocked{}, path{}",session.result_presentation().unwrap().end_requested,level.attempts(),level.completed(),reloaded.levels[reloaded.find(next).unwrap()].unlocked(),path.display());
                }
            }
            if capacity_mode==4 {
                assert_eq!((session.world.objective.kind,session.world.objective.target),(1,3.),
                    "Scenario6 mayor objective changed before result acceptance");
                assert_eq!(word(&session.roster.manager.players[0].bytes,0x14c),3,
                    "Scenario6 ended before the human won three mayor elections");
                assert_eq!(session.world.objective.reached[0],1,
                    "Scenario6 terminal Win is insufficient without the mayor objective reaching its target");
                assert_eq!(terminal_context,Some(0),"natural Scenario6 mayor objective did not produce a human Win");
                assert!(host.failure.is_none(),"Scenario6 stopped at a simulation boundary: {:?}",host.failure);
                assert_eq!(host.result_sequence(),Some(openmt2001::outcome_notice::Sequence::Win));
                assert_eq!(fs::read(&natural_win_source).unwrap(),*baseline_profile.as_ref().unwrap(),"natural Scenario1 Win source profile was modified");
                assert_eq!(fs::read(profile_path.as_ref().unwrap()).unwrap(),*baseline_profile.as_ref().unwrap(),"Scenario6 profile advanced without completed result hub");
                assert_eq!(session.catalog.selected_level().unwrap().id(),200006);
                assert_eq!(session.catalog.selected_level().unwrap().attempts(),0);
                assert!(host.complete_result_sequence(&mut session).is_err(),"Scenario6 profile advanced before original Game.End");
                session.dispatch_result_event("FlybyEnded",&assets).unwrap();
                assert!(host.complete_result_sequence(&mut session).is_err(),"Scenario6 profile advanced before original CutSceneEnded");
                session.dispatch_result_event("CutSceneEnded",&assets).unwrap();
                assert_eq!(session.result_presentation().unwrap().end_requested,Some(1));
                assert_eq!(fs::read(profile_path.as_ref().unwrap()).unwrap(),*baseline_profile.as_ref().unwrap(),"Scenario6 profile advanced before Host committed original Game.End");
                assert_eq!(host.complete_result_sequence(&mut session).unwrap(),openmt2001::outcome_notice::Sequence::Win);
                assert!(host.complete_result_sequence(&mut session).is_err(),"Scenario6 result committed more than once");
                let path=profile_path.as_ref().unwrap();
                let mut reloaded=crate::campaign_store::load_catalog(&image,scripts).unwrap();
                crate::campaign_store::load_progress(&mut reloaded,path).unwrap();
                let first=&reloaded.levels[reloaded.find(200001).unwrap()];
                assert_eq!(first.attempts(),1,"Scenario1 attempt was lost from profile");
                assert!(first.completed()[0],"Scenario1 Win was lost from profile");
                let mayor=&reloaded.levels[reloaded.find(200006).unwrap()];
                assert_eq!(mayor.attempts(),1,"Scenario6 attempt did not survive profile reload");
                assert!(mayor.completed()[0],"Scenario6 Win did not survive profile reload");
                assert_eq!(fs::read(&natural_win_source).unwrap(),*baseline_profile.as_ref().unwrap(),"source profile changed during Scenario6 completion");
                println!("Natural Scenario6 mayor Win and V2 profile reload: tick{}, population{}, mayor-score{:?}, Scenario1 attempts{}, Scenario6 attempts{}, completed{:?}, path{}",session.world.clock.timing[0],session.world.ledgers.residents.active_count,session.roster.manager.players.iter().map(|p|i32::from_le_bytes(p.bytes[0x1b4..0x1b8].try_into().unwrap())).collect::<Vec<_>>(),first.attempts(),mayor.attempts(),mayor.completed(),path.display());
            }
            if capacity_mode==5 {
                assert!(host.failure.is_none(),"Scenario11 stopped at a simulation boundary: {:?}",host.failure);
                assert!(terminal_context.is_some()||reached_day,"Scenario11 stopped before outcome or 72-hour horizon");
                assert_eq!(fs::read(&natural_win_source).unwrap(),*baseline_profile.as_ref().unwrap(),"Scenario1+6 source profile changed");
                assert_eq!(fs::read(profile_path.as_ref().unwrap()).unwrap(),*baseline_profile.as_ref().unwrap(),"Scenario11 profile changed before completed result hub");
                println!("Scenario11 measured frontier: result-context{terminal_context:?}, result{:?}, horizon{reached_day}, builds{}, traded{}, auctions{:?}, population{}, sold-units{:?}",host.result_sequence(),competitive_categories.len(),human_retail_ever_traded,auction_rooms,session.world.ledgers.residents.active_count,session.roster.manager.players.iter().map(|p|i32::from_le_bytes(p.bytes[0x1b4..0x1b8].try_into().unwrap())).collect::<Vec<_>>());
            }
        }).unwrap().join().unwrap();
    }

    #[test]
    #[ignore = "requires original data and GPU; real Host auction controls/settlement"]
    fn original_live_auction_host() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let read=|key|fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable=read("OPENMT2001_EXE");let directory=read("OPENMT2001_ARCHIVE_DIR");
            let archive=read("OPENMT2001_ARCHIVE_BIN");let build=read("OPENMT2001_BUILD_FILE");
            let parameters=read("OPENMT2001_PARAMETERS");let timetable=read("OPENMT2001_TIMETABLE");
            let image=openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board=openmt2001::board::Board::from_v14(&image).unwrap();
            let file=openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index=openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64).unwrap();
            let settings=openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let exe_path=openmt2001::fixtures::path("EXE");
            let names=character_names(&image,&exe_path.parent().unwrap().parent().unwrap().join("US/language/usa")).unwrap();
            let mut session=Session::initialize(Inputs{
                tree_locators: None,
                image:&image,board:&board,file:&file,
                people:openmt2001::people::PeopleModels::load(&image,&index,&archive).unwrap(),
                vehicle_models:openmt2001::traffic::road::models::Models::from_original(&image,&index,&archive).unwrap(),
                transport_models:openmt2001::traffic::models::Models::from_original(&image,&index,&archive).unwrap(),
                road_routes:read("OPENMT2001_ROAD_ROUTE_TABLE"),scripts:settings.parent().unwrap().parent().unwrap(),
                parameters:std::str::from_utf8(&parameters).unwrap(),
                era_styles:load_era_styles(Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap(),timetable:std::str::from_utf8(&timetable).unwrap(),
                scenario:"SCENARIO1",difficulty:0,selected_character:4,profile_name:b"Auction host",character_names:&names,
            },Policy{seed:0x12345678,auxiliary_prior_bytes:[0;0x11c]}).unwrap();
            let word=|b:&[u8],at|u32::from_le_bytes(b[at..at+4].try_into().unwrap());
            // Isolate the player/UI transaction; competing strategy is verified
            // separately. Opponent withdrawals below use the actual consumer.
            for p in &mut session.roster.manager.players { if word(&p.bytes,0x154)==1 {p.bytes[0x1403]=1;} }
            let local=session.roster.manager.players.iter().find(|p|p.reference==session.roster.status.current_player).unwrap().bytes[0x1bc];
            let category=(0..52).find(|&i|session.world.categories[i][5]==0).unwrap();
            let assets=CommandAssets{image:&image,board:&board,file:&file};
            let (sender,receiver)=std::sync::mpsc::channel::<crate::auction_bridge::Request>();
            let presenter=std::thread::spawn(move || {
                let exe=read("OPENMT2001_EXE");let directory=read("OPENMT2001_ARCHIVE_DIR");let archive=read("OPENMT2001_ARCHIVE_BIN");
                let mut descriptor=wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows){descriptor.backends=wgpu::Backends::DX12;}
                let renderer=pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
                let mut view=crate::auction_bridge::View::default();let mut calls=0;
                while let Ok(request)=receiver.recv(){
                    let result=view.present(request.operation,&renderer,(&exe,&directory,&archive),3);
                    assert!(result.is_ok(),"real presenter failed: {result:?}");calls+=1;
                    request.reply.send(result).unwrap();
                }
                assert!(view.scene.is_none(),"settlement must release auction rendering resources");
                assert_eq!(calls,3,"prepare,activate,restore must each run once");
            });
            let mut host=crate::live_host::Host::new([0.,20.,0.]);host.auction_renderer=Some(sender);host.set_running(true);
            // Start from a different participant, leaving the local player a
            // genuine bid turn after controlled opponents before them withdraw.
            let initiator=(usize::from(local)+1)%session.roster.manager.players.len();
            let mut descriptor=[0;52];for(off,value)in[(4,0x1d),(20,category as u32),(24,initiator as u32),(28,100)]{descriptor[off..off+4].copy_from_slice(&value.to_le_bytes());}descriptor[16]=1;
            session.submit_command(openmt2001::live_commands::Command::Auction{descriptor},&assets,|_|Err("early admission".into())).unwrap();
            let frame=|host:&mut crate::live_host::Host,session:&mut Session<'_>| {
                for prompt in host.take_prompts(session) {
                    session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,false).unwrap();
                }
                host.frame(session,&assets,40);
                assert!(host.failure.is_none(),"Host auction stopped at tick{}: {:?}",session.world.clock.timing[0],host.failure);
            };
            for _ in 0..3100 {
                frame(&mut host,&mut session);
                if host.status(&session).auction.is_some_and(|a|a.visible){break;}
            }
            assert_eq!(word(&session.world.auction.controller.record,0x150),3,"actual presenter prior interface");
            assert_eq!(word(&session.world.auction.controller.record,0x154),0,"actual presenter prior transition");
            let started = host.status(&session).auction.unwrap();
            assert!(started.visible);
            assert_eq!(started.participants.len(), session.roster.manager.players.len());
            for (slot, participant) in started.participants.iter().enumerate() {
                let player = &session.roster.manager.players[slot];
                let end = player.bytes[0x158..0x1b8].iter().position(|&b| b == 0).unwrap();
                assert_eq!(participant.name, player.bytes[0x158..0x158 + end]);
                assert!(!participant.name.is_empty());
                assert_eq!(participant.label.cash, session.world.ledgers.players[slot].cash.trunc() as i32);
            }
            use crate::auction_labels::Text;
            let current = word(&session.world.auction.controller.record, 0x58) as usize;
            assert_eq!(started.participants[current].label.status, Text::Language(1558));
            assert_eq!(started.participants[initiator].label.status, Text::Money(100));
            use openmt2001::ai_messages::offers::auction::Command;
            for _ in 0..6 {
                let participant=word(&session.world.auction.controller.record,0x58);
                if participant==u32::from(local){break;}
                host.apply_auction_action(&mut session,&assets,Command{action:2,participant,amount:0}).unwrap();
                assert_eq!(host.status(&session).auction.unwrap().participants[participant as usize].label.status, Text::Language(1555));
            }
            frame(&mut host,&mut session);
            assert_eq!(host.status(&session).auction.unwrap().controls.buttons_enabled,31);
            let price=word(&session.world.auction.controller.record,0x6c);
            let ledger=session.world.ledgers.players[usize::from(local)].ledger(0).unwrap();
            host.auction_button(&mut session,&assets,1).unwrap();
            assert_eq!(word(&session.world.auction.controller.record,0x6c),price+100);
            let bid = host.status(&session).auction.unwrap();
            assert_eq!(bid.controls.buttons_enabled, 0);
            assert_eq!(bid.participants[usize::from(local)].label.status, Text::Money((price + 100) as i32));
            let next = word(&session.world.auction.controller.record, 0x58);
            assert_eq!(bid.participants[next as usize].label.status, Text::Language(1558));
            #[cfg(feature = "capture")]
            if let Ok(path) = std::env::var("OPENMT2001_LIVE_AUCTION_LABEL_CAPTURE") {
                let installer = openmt2001::fixtures::path("PARAMETERS");
                let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa/tycoon");
                let packed = crate::intro::camera_live::ui_text_resources::PackedLanguage::load(&language).unwrap();
                let panel_assets = crate::auction_panel::Assets::load(&index, &archive,
                    installer.parent().unwrap().parent().unwrap(), &packed.rows(), &executable).unwrap();
                let renderer = pollster::block_on(openmt2001::render::Renderer::new(wgpu::InstanceDescriptor::new_without_display_handle())).unwrap();
                let characters = bid.participants.iter().map(|p| p.character).collect::<Vec<_>>();
                let scene = crate::auction_scene::Assets::load(&image, &index, &archive).unwrap().prepare(&renderer, &characters).unwrap();
                let mut view = crate::auction_panel::View::default();
                view.prepare(&panel_assets, &renderer, Some(&bid), None).unwrap();
                crate::capture_test::capture_overlay(&renderer, &scene, view.prepared.as_ref().unwrap(), &path);
            }
            host.apply_auction_action(&mut session, &assets, Command { action: 1, participant: next, amount: 0 }).unwrap();
            let passed = host.status(&session).auction.unwrap();
            assert_eq!(passed.participants[next as usize].label.status, Text::Language(1556));
            assert_eq!(passed.participants[usize::from(local)].label.status, Text::Language(1552));
            assert!(passed.participants[usize::from(local)].winner);
            for _ in 0..6 {
                if word(&session.world.auction.controller.record,0x60)!=u32::MAX{break;}
                let participant=word(&session.world.auction.controller.record,0x58);
                assert_ne!(participant,u32::from(local));
                host.apply_auction_action(&mut session,&assets,Command{action:2,participant,amount:0}).unwrap();
            }
            for _ in 0..500 {
                frame(&mut host,&mut session);
                if session.world.categories[category][5]==local+1 {break;}
            }
            assert_eq!(session.world.categories[category][5],local+1);
            assert_eq!(session.world.auction.controller.active_count,0);
            assert_eq!(session.world.categories[category][0x6c],0);
            assert_eq!(session.world.clock.timing[5],1);
            assert_eq!(session.world.ledgers.players[usize::from(local)].ledger(0).unwrap(),ledger-(price+100) as f32);
            assert!(host.status(&session).auction.is_none());
            // Exercise the real human offer modal after winning the property.
            // Seed a pending offer using the original inbox producer, then choose
            // the visible Refuse button through the modal input state.
            let sender=(usize::from(local)+1)%session.roster.manager.players.len();
            let mut arena=openmt2001::player_messages::Arena{next:session.allocation_next};
            let current=session.roster.status.current_player;
            openmt2001::player_messages::produce([i32::from(local),sender as i32,0x23,category as i32,1750,0,0,0],&mut session.roster.manager,&mut arena,current,[false,false],[0;48],|route,_|Err(format!("offer admission escaped {route:?}"))).unwrap();
            session.allocation_next=arena.next;
            session.world.categories[category][0x6c]=1;
            let mut modal=None;
            for _ in 0..100 {
                host.frame(&mut session,&assets,40);assert!(host.failure.is_none(),"human offer {:?}",host.failure);
                modal=host.take_prompts(&session).into_iter().find(|request|request.dialog.is_some());
                if modal.is_some(){break;}
            }
            let request=modal.unwrap_or_else(|| panic!("real human controller must show offer: countdown{} inbox{} mode{}",word(&session.roster.manager.players[usize::from(local)].bytes,8),word(&session.roster.manager.players[usize::from(local)].bytes,0x214),word(&session.roster.manager.players[usize::from(local)].bytes,0x220)));let dialog=request.dialog.unwrap();
            assert_eq!((dialog.callback,dialog.context,dialog.button_count),(0x48b350,0x23,2));
            assert!(!dialog.timeout);assert_eq!(word(&session.roster.manager.players[usize::from(local)].bytes,0x220),1);
            let mut ui=crate::prompt::State::default();ui.submit(request);
            let buttons=crate::human_dialog::buttons([130.,80.,510.,380.],2);
            ui.cursor([(buttons[1][0]+buttons[1][2])*0.5,(buttons[1][1]+buttons[1][3])*0.5]);
            ui.dialog_mouse(true,&buttons);ui.dialog_mouse(false,&buttons);
            let response=ui.take_dialog_replies();assert_eq!(response.len(),1);
            let inbox_before=word(&session.roster.manager.players[sender].bytes,0x214);
            host.human_dialog_reply(&mut session,&assets,response[0]).unwrap();
            ui.finish_dismiss(host.take_prompts(&session));assert!(ui.request().is_none());
            assert_eq!(word(&session.roster.manager.players[usize::from(local)].bytes,0x220),0);
            assert_eq!(session.world.categories[category][0x6c],0);
            let recipient=&session.roster.manager.players[sender];assert_eq!(word(&recipient.bytes,0x214),inbox_before+1);
            let mut tail=word(&recipient.bytes,0x218);
            loop { let packet=&recipient.auxiliaries.iter().find(|entry|entry.reference==tail).unwrap().bytes;let next=word(packet,0x2c);if next==0 {assert_eq!(word(packet,4),0x26);break;}tail=next; }
            println!("Real Host human offer: typed modal, disabled timeout, Refuse input, native reply inbox, category unlock and latch clear verified");
            // A rival transfer must reach420500's real notification queue. Its
            // timeout is driven by the actual50Hz Host task, independently of
            // the player pause that freezes city logic.
            let rival=((usize::from(local)+1)%session.roster.manager.players.len()) as u8;
            session.submit_command(openmt2001::live_commands::Command::PropertyTransfer{
                category:category as i16,owner:rival+1,amount:0,
            },&assets,|_|Err("early rival transfer".into())).unwrap();
            for _ in 0..26 {frame(&mut host,&mut session);}
            let notices=host.status(&session).notices;
            assert_eq!(notices.len(),1);
            assert_eq!(notices[0].category,category as u32);
            assert_eq!(notices[0].character,word(&session.roster.manager.players[usize::from(rival)].bytes,0x150));
            host.set_running(false);
            for _ in 0..350 {frame(&mut host,&mut session);}
            assert!(host.status(&session).notices.is_empty());
            crate::card_panel::verify_host(&mut host,&mut session,&assets);
            exercise_host_outcome(&mut host,&mut session,&assets);
            host.auction_renderer.take();presenter.join().unwrap();
            println!("Real Host auction: timed admission, original GPU scene handoff, UI eligibility, human bid, settlement, ledger and restored view verified");
        }).unwrap().join().unwrap();
    }

    #[test]
    #[ignore = "requires original level scripts, people meshes and rendering assets"]
    fn original_fresh_city_renders_live_records() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let sun = read("OPENMT2001_SUNCOLS");
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
        let scripts = default_path.parent().unwrap().parent().unwrap();
        let language = openmt2001::fixtures::path("EXE")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let off = fs::read(language.join("tycoon.off")).unwrap();
        let names = character_names(&image, &language).unwrap();
        assert_eq!(
            names.iter().map(|name| name.as_slice()).collect::<Vec<_>>(),
            [
                b"Racecar".as_slice(),
                b"Wheelbarrow",
                b"Dog",
                b"Cannon",
                b"Top Hat",
                b"Horse",
                b"Iron",
                b"Battleship",
                b"Shoe",
                b"Thimble"
            ]
        );
        let native_names = fs::read(openmt2001::fixtures::path("NATIVE_CHARACTER_NAMES")).unwrap();
        assert_eq!(
            names
                .iter()
                .flat_map(|name| name.iter().copied().chain([0]))
                .collect::<Vec<_>>(),
            native_names
        );
        let text = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = off
            .chunks_exact(2)
            .map(|b| {
                let n = usize::from(u16::from_le_bytes(b.try_into().unwrap()));
                let t = &text[at..at + n];
                at += n;
                t
            })
            .collect::<Vec<_>>();
        let installer = openmt2001::fixtures::path("SUNCOLS");
        let adjustment = crate::business_adjustment::Assets::load(
            &index,
            &archive,
            installer.parent().unwrap().parent().unwrap(),
            &strings,
            &executable,
        )
        .unwrap();
        let city_assets = crate::locator::Assets::load(
            &index,
            &archive,
            installer.parent().unwrap().parent().unwrap(),
            &strings,
            &executable,
        )
        .unwrap();
        for scenario in ["SCENARIO1", "SCENARIO3", "SCENARIO5"] {
            for difficulty in 0..3 {
                let mut session = Session::initialize(
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
                        road_routes: std::fs::read(openmt2001::fixtures::path("ROAD_ROUTE_TABLE"))
                            .unwrap(),
                        scripts,
                        parameters: std::str::from_utf8(&parameters).unwrap(),
                        era_styles: load_era_styles(
                            Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                                .parent()
                                .unwrap(),
                        )
                        .unwrap(),
                        timetable: std::str::from_utf8(&timetable).unwrap(),
                        scenario,
                        difficulty,
                        selected_character: 4,
                        profile_name: b"Fixture Player",
                        character_names: &names,
                    },
                    Policy {
                        seed: 0x12345678,
                        auxiliary_prior_bytes: [0; 0x11c],
                    },
                )
                .unwrap();
                let expected_players = match scenario {
                    "SCENARIO1" => 4 + difficulty as usize,
                    "SCENARIO3" => 1,
                    "SCENARIO5" => 2,
                    _ => unreachable!(),
                };
                assert_eq!(session.roster.manager.players.len(), expected_players);
                assert_eq!(session.world.ledgers.players.len(), expected_players);
                let financial = crate::player_info::State::from_runtime(
                    &session.roster.manager,
                    &session.world.ledgers.players,
                    session.roster.status.current_player,
                )
                .unwrap();
                assert_eq!(financial.players.len(), expected_players);
                assert_eq!(financial.current, Some(0));
                let comparison = crate::comparison::Data::verify_runtime(
                    &session.world,
                    &financial.records,
                    &image,
                    scenario,
                    difficulty as usize,
                );
                let mut property = crate::block_information::Report::verify_runtime(
                    &session.world,
                    &financial,
                    &image,
                    scenario,
                    difficulty as usize,
                );
                property
                    .attach_railroads(
                        &session.population.properties,
                        session.world.settings.selected_income().daily_freight,
                    )
                    .unwrap();
                for (record, account) in
                    financial.records.iter().zip(&session.world.ledgers.players)
                {
                    assert_eq!(
                        u32::from_le_bytes(record[..4].try_into().unwrap()),
                        account.cash.to_bits()
                    );
                }
                assert_eq!(
                    session.roster.status.ai_count as usize,
                    expected_players - 1
                );
                for (slot, (player, account)) in session
                    .roster
                    .manager
                    .players
                    .iter()
                    .zip(&session.world.ledgers.players)
                    .enumerate()
                {
                    let character =
                        u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap());
                    let expected_name = if slot == 0 {
                        assert_eq!(character, 4);
                        b"Fixture Player".as_slice()
                    } else {
                        names[character as usize].as_slice()
                    };
                    assert_eq!(
                        &player.bytes[0x158..0x158 + expected_name.len()],
                        expected_name
                    );
                    assert_eq!(player.bytes[0x158 + expected_name.len()], 0);
                    assert_eq!(account.cash.to_bits(), session.world.player_money.to_bits());
                }
                assert_eq!(session.world.clock.timing[5], 1);
                assert_eq!(
                    session.population.bindings.len(),
                    session.world.ledgers.residents.active_count as usize
                );
                if scenario == "SCENARIO3"
                    && difficulty == 0
                    && let Ok(path) = std::env::var("OPENMT2001_INITIALIZED_BUSINESSES_EXPORT")
                {
                    let bytes = session
                        .world
                        .ledgers
                        .businesses
                        .iter()
                        .flat_map(|row| row.iter().copied())
                        .collect::<Vec<_>>();
                    fs::write(path, bytes).unwrap();
                }
                assert_eq!(session.world.clock.hour(), 9);
                assert!(session.world.ledgers.residents.active_count > 0);
                let report = crate::city_people::Report::from_world(
                    &session.world,
                    &image,
                    &session.roster.manager,
                )
                .unwrap();
                assert!(report.initialized);
                assert_eq!(
                    report.rows[0].value,
                    session.world.ledgers.residents.active_count as i32
                );
                assert_eq!(
                    report.markers.len(),
                    session.world.creation.auxiliary.used()
                );
                for (marker, aux) in report
                    .markers
                    .iter()
                    .zip(session.world.creation.auxiliary.slots())
                {
                    assert_eq!(marker.visible, aux[0xb8] & 1 != 0);
                    for (i, value) in marker.position.iter().enumerate() {
                        assert_eq!(
                            value.to_bits(),
                            u32::from_le_bytes(aux[4 + i * 4..8 + i * 4].try_into().unwrap())
                        );
                    }
                }

                let first_auxiliary = &session.world.creation.auxiliary.slots()[0];
                let coordinate =
                    |at| f32::from_le_bytes(first_auxiliary[at..at + 4].try_into().unwrap());
                let focus = [coordinate(4), coordinate(8), coordinate(12)];
                let requests = crate::people_scene::initial_requests(
                    &session,
                    openmt2001::flyby::CameraSample {
                        vertical_fov: 45.,
                        eye: [focus[0], focus[1] + 15., focus[2] + 40.],
                        target: focus,
                        up: [0., 1., 0.],
                    },
                    35,
                    [1280, 960],
                )
                .unwrap();
                assert!(!requests.is_empty());
                assert!(requests.len() < session.world.creation.auxiliary.used());
                for request in &requests {
                    let model = session
                        .people
                        .prepare(request.model_row, request.lod)
                        .unwrap();
                    assert!(
                        !model
                            .world_positions(request.frame, &request.transform)
                            .unwrap()
                            .is_empty()
                    );
                }
                println!(
                    "{scenario} difficulty {difficulty}: {} visible initial resident requests",
                    requests.len()
                );
                if difficulty == 0 {
                    let draws = crate::people_scene::initial_draws(
                        &session, &image, &index, &archive, &sun, &requests,
                    )
                    .unwrap();
                    assert!(!draws.is_empty());
                    assert!(
                        draws.iter().all(
                            |draw| draw.specular.as_ref().unwrap().len() == draw.vertices.len()
                        )
                    );
                    #[cfg(feature = "capture")]
                    if let Ok(prefix) = std::env::var("OPENMT2001_PEOPLE_CAPTURE") {
                        let sample = openmt2001::flyby::CameraSample {
                            vertical_fov: 45.,
                            eye: [focus[0], focus[1] + 4., focus[2] + 12.],
                            target: focus,
                            up: [0., 1., 0.],
                        };
                        let requests = crate::people_scene::initial_requests(
                            &session,
                            sample,
                            35,
                            [1280, 960],
                        )
                        .unwrap();
                        let mut scene = session
                            .scene(
                                &build,
                                openmt2001::render_scene::OriginalAssets {
                                    executable: &executable,
                                    directory: &directory,
                                    archive: &archive,
                                },
                                &sun,
                                Some(sample),
                                false,
                            )
                            .unwrap();
                        scene.draws.extend(
                            crate::people_scene::initial_draws(
                                &session, &image, &index, &archive, &sun, &requests,
                            )
                            .unwrap(),
                        );
                        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                        if cfg!(windows) {
                            descriptor.backends = wgpu::Backends::DX12;
                        }
                        let renderer =
                            pollster::block_on(openmt2001::render::Renderer::new(descriptor))
                                .unwrap();
                        crate::capture_test::capture(
                            &renderer,
                            None,
                            &scene,
                            &format!("{prefix}-{scenario}.png"),
                        );
                    }
                }

                assert_eq!(
                    session.models.bindings.len(),
                    session
                        .world
                        .buildings
                        .iter()
                        .filter(|r| r[0x3f] != 0)
                        .count()
                );
                assert!(
                    session
                        .hooks
                        .commands
                        .iter()
                        .any(|(command, _)| matches!(command, level_script::Command::Objective))
                );
                assert!(
                    session
                        .hooks
                        .commands
                        .iter()
                        .all(|(command, _)| !matches!(command, level_script::Command::Prompt))
                );
                assert_eq!(
                    session
                        .hooks
                        .timers
                        .iter()
                        .map(|t| u32::from_le_bytes(t[16..20].try_into().unwrap()))
                        .collect::<Vec<_>>(),
                    [0x4baa10, 0x4ba4d0, 0x4ba580, 0x481680]
                );
                if difficulty == 0 {
                    let scene = session
                        .scene(
                            &build,
                            openmt2001::render_scene::OriginalAssets {
                                executable: &executable,
                                directory: &directory,
                                archive: &archive,
                            },
                            &sun,
                            None,
                            false,
                        )
                        .unwrap();
                    assert_eq!(scene.building_count, session.models.bindings.len());
                    assert!(scene.saved_players.is_empty());
                    #[cfg(feature = "capture")]
                    if std::env::var_os("OPENMT2001_FRESH_PROPERTY_CAPTURE").is_some() {
                        let category = scene
                            .inspection_buildings
                            .iter()
                            .find(|b| property.categories[b.category].is_some())
                            .unwrap()
                            .category;
                        let center = board
                            .blocks
                            .iter()
                            .find(|b| b.id == category)
                            .unwrap()
                            .center;
                        let mut controls = openmt2001::camera::BlockCamera {
                            center: [center[0], 0., center[1]],
                            angle: -std::f32::consts::FRAC_PI_2,
                            height: 120.,
                            radius: 80.,
                        };
                        let (eye, target) = controls.tick([false; 4]);
                        let property_scene = session
                            .scene(
                                &build,
                                openmt2001::render_scene::OriginalAssets {
                                    executable: &executable,
                                    directory: &directory,
                                    archive: &archive,
                                },
                                &sun,
                                Some(openmt2001::flyby::CameraSample {
                                    eye,
                                    target,
                                    up: [0., 1., 0.],
                                    vertical_fov: 45.,
                                }),
                                false,
                            )
                            .unwrap();
                        city_assets.verify_fresh_property(
                            &property,
                            &property_scene,
                            category,
                            scenario,
                        );
                    }
                    let _ = &property;
                    crate::comparison::Assets::load(
                        &index,
                        &archive,
                        installer.parent().unwrap().parent().unwrap(),
                        &strings,
                        &executable,
                    )
                    .unwrap()
                    .verify_fresh(
                        comparison,
                        &financial.records,
                        &scene,
                        scenario,
                    );
                    city_assets.verify_fresh_players(&scene, financial, scenario);
                    let mut city = city_assets.people_state().unwrap();
                    city.attach_people(report.clone());
                    // Unsupported saved-only controls do not capture input when panel is closed.
                    city.mouse(&city_assets, &scene, Some([32, 125]), true);
                    city.mouse(&city_assets, &scene, Some([32, 125]), false);
                    assert!(!city_assets.contains(&city, [32, 180]));
                    assert!(!city_assets.contains(&city, [32, 225]));
                    city.mouse(&city_assets, &scene, Some([32, 125]), true);
                    city.mouse(&city_assets, &scene, Some([32, 125]), false);
                    #[cfg(feature = "capture")]
                    if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_PEOPLE_CAPTURE") {
                        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                        if cfg!(windows) {
                            descriptor.backends = wgpu::Backends::DX12;
                        }
                        let renderer =
                            pollster::block_on(openmt2001::render::Renderer::new(descriptor))
                                .unwrap();
                        for mode in 0..4 {
                            let prepared = city_assets.prepare(&renderer, &city, &scene).unwrap();
                            crate::capture_test::capture(
                                &renderer,
                                Some(&prepared),
                                &scene,
                                &format!("{prefix}-{scenario}-{mode}.png"),
                            );
                            city.mouse(&city_assets, &scene, Some([362, 108]), true);
                            city.mouse(&city_assets, &scene, Some([362, 108]), false);
                        }
                    }

                    assert!(
                        scene
                            .inspection_buildings
                            .iter()
                            .all(|b| b.saved_business.is_none())
                    );
                    let mut occupied = 0u32;
                    for (i, b) in scene.inspection_buildings.iter().enumerate() {
                        let live = b.initialized_apartment.as_ref().unwrap();
                        assert_eq!(live.record, session.world.ledgers.businesses[live.slot]);
                        let state = adjustment.open(&scene, i).unwrap().unwrap();
                        assert!(state.initialized);
                        let apartment = state.apartment.as_ref().unwrap();
                        occupied += u32::from(apartment.tenants);
                        assert_eq!(
                            apartment.rent.to_bits(),
                            u32::from_le_bytes(live.record[0x204..0x208].try_into().unwrap())
                        );
                        let next = adjustment
                            .navigate(&scene, &state, (i + 1) % scene.building_count)
                            .unwrap();
                        assert!(next.initialized);
                    }
                    println!(
                        "{scenario}: initialized apartment Today counters total {occupied}; population pool {} (later housing counters pending)",
                        session.world.ledgers.residents.active_count
                    );
                    #[cfg(feature = "capture")]
                    if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_CITY_CAPTURE") {
                        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                        if cfg!(target_os = "windows") {
                            descriptor.backends = wgpu::Backends::DX12;
                        }
                        let renderer =
                            pollster::block_on(openmt2001::render::Renderer::new(descriptor))
                                .unwrap();
                        crate::capture_test::capture(
                            &renderer,
                            None,
                            &scene,
                            &format!("{prefix}-{scenario}.png"),
                        );
                        let mut state = adjustment.open(&scene, 0).unwrap().unwrap();
                        state.prepare(&adjustment, &renderer).unwrap();
                        crate::capture_test::capture(
                            &renderer,
                            state.prepared.as_ref(),
                            &scene,
                            &format!("{prefix}-apartment-{scenario}.png"),
                        );
                    }
                }
                if scenario == "SCENARIO1" && difficulty == 0 {
                    exercise_ai_hourly(&mut session, &image, &board, &file);
                    exercise_event_retirement(&mut session, &image, &board, &file);
                    cards::exercise(
                        &mut session,
                        &CommandAssets {
                            image: &image,
                            board: &board,
                            file: &file,
                        },
                        &mut simulation_test_policy(),
                    );
                    exercise_ai_business_management(&mut session, &image, &board, &file);
                    exercise_business_takeover(&mut session, &image, &board, &file);
                    exercise_business_sale(&mut session, &image, &board, &file);
                    exercise_property_transfer(&mut session, &image, &board, &file);
                    exercise_live_auction(&mut session, &image, &board, &file);
                    exercise_business_conversion(&mut session, &image, &board, &file);
                    exercise_human_replies(&mut session, &image, &board, &file);
                    exercise_lease_expiry(&mut session, &image, &board, &file);
                    exercise_property_offers(&mut session, &image, &board, &file);
                    exercise_rights_and_outcome(
                        &mut session,
                        &CommandAssets {
                            image: &image,
                            board: &board,
                            file: &file,
                        },
                    );
                }
                if scenario == "SCENARIO3" && difficulty == 1 {
                    exercise_retained_population(&mut session);
                }
                if scenario == "SCENARIO3" && difficulty == 0 {
                    exercise_calendar(&mut session, &image, &board, &file);
                }
                if scenario == "SCENARIO5" && difficulty == 0 {
                    exercise_market_outcome(&mut session, &image, &board, &file);
                }
                println!(
                    "{scenario}/{difficulty}: {} buildings, {} residents, {} queued events; paused initialization only",
                    session.models.bindings.len(),
                    session.world.ledgers.residents.active_count,
                    session.scripts.events.pending().len()
                );
            }
        }
    }
}

#[cfg(test)]
mod skirmish_session_acceptance {
    use super::*;

    #[test]
    #[ignore = "requires supplied original assets; fresh offline Skirmish kernel"]
    fn original_net1_skirmish_session() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let _sun = read("OPENMT2001_SUNCOLS");
        let parameters = read("OPENMT2001_PARAMETERS");
        let timetable = read("OPENMT2001_TIMETABLE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
        let scripts = default_path.parent().unwrap().parent().unwrap();
        let language = openmt2001::fixtures::path("EXE")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");

        let names = character_names(&image, &language).unwrap();
        use openmt2001::skirmish::roster::{Kind, Participant};
        let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
        let launch = SkirmishLaunch {
            level_id: 300001,
            directory: "net1".into(),
            map: "empty.lua".into(),
            objective_kind: 6,
            target: 3500.,
            difficulty: 1,
            participants: vec![
                Participant {
                    kind: Kind::Human,
                    token: Some(4),
                    name: b"Fixture Player".to_vec(),
                    ready: true,
                    difficulty: 2,
                },
                Participant {
                    kind: Kind::Ai,
                    token: Some(1),
                    name: b"Explicit Opponent".to_vec(),
                    ready: true,
                    difficulty: 0,
                },
            ],
        };
        let session = Session::initialize_skirmish(
            Inputs {
                tree_locators: None,
                image: &image,
                board: &board,
                file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image, &index, &archive,
                )
                .unwrap(),
                road_routes: std::fs::read(openmt2001::fixtures::path("ROAD_ROUTE_TABLE")).unwrap(),
                scripts,
                parameters: std::str::from_utf8(&parameters).unwrap(),
                era_styles: load_era_styles(
                    Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                        .parent()
                        .unwrap(),
                )
                .unwrap(),
                timetable: std::str::from_utf8(&timetable).unwrap(),
                scenario: "NET1",
                difficulty: 1,
                selected_character: 4,
                profile_name: b"Fixture Player",
                character_names: &names,
            },
            Policy {
                seed: 0x12345678,
                auxiliary_prior_bytes: [0; 0x11c],
            },
            catalog,
            launch,
        )
        .unwrap();
        let word =
            |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(session.catalog.selected_level().unwrap().id(), 300001);
        assert!(!session.catalog.selected_level().unwrap().single_player());
        assert_eq!(
            session.catalog.selected_level().unwrap().objective_target(),
            3500.
        );
        assert_eq!(session.world.objective.kind, 6);
        assert_eq!(session.world.objective.target, 3500.);
        assert_eq!(session.difficulty, 1);
        assert_eq!(session.roster.manager.players.len(), 2);
        assert_eq!(session.roster.status.ai_count, 1);
        assert_eq!(session.world.player_money, 6500.);
        assert_eq!(session.world.ledgers.residents.active_count, 251);
        for (slot, (token, kind, difficulty, peer, name)) in [
            (4, 0, 2, 0, b"Fixture Player".as_slice()),
            (1, 1, 0, u32::MAX, b"Explicit Opponent".as_slice()),
        ]
        .into_iter()
        .enumerate()
        {
            let row = &session.saved_player_metadata[slot];
            assert_eq!(word(row, 0), token);
            assert_eq!(word(row, 4), kind);
            assert_eq!(word(row, 40), peer);
            assert_eq!(word(row, 44), difficulty);
            assert_eq!(&row[8..8 + name.len()], name);
            assert_eq!(
                word(&session.roster.manager.players[slot].bytes, 0x150),
                token
            );
            assert_eq!(
                word(&session.roster.manager.players[slot].bytes, 0x154),
                kind
            );
            if kind == 1 {
                assert_eq!(
                    word(&session.roster.manager.players[slot].bytes, 0x13f4),
                    difficulty
                );
            }
            assert_eq!(session.world.ledgers.players[slot].cash, 6500.);
        }
        assert!(session.campaign_profile.is_none());
        assert!(session.network_command_timer.is_none());
    }
    #[test]
    #[ignore = "requires supplied originals; bounded net1 cold reload Host continuation"]
    fn original_net1_skirmish_cold_reload_continuation() {
        skirmish_map_cold_reload_continuation("empty.lua");
    }

    #[test]
    #[ignore = "requires supplied originals; selected-map v6 NET1 cold reload Host continuation"]
    fn original_net1_monopoly_map_cold_reload_continuation() {
        skirmish_map_cold_reload_continuation("monopolymap.lua");
    }

    // Shared source-backed record/model checks; no Host or renderer execution.
    fn check_authored_records(
        state: &Session<'_>,
        specs: &[openmt2001::scene::BuildingSpec],
        board: &openmt2001::board::Board,
        file: &openmt2001::build_file::BuildFile<'_>,
    ) -> Vec<(usize, usize)> {
        let word =
            |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let mut identities = Vec::new();
        for spec in specs {
            let scene =
                openmt2001::scene::SceneBuilding::resolve(spec.clone(), board, file).unwrap();
            let category_reference = openmt2001::building_geometry::V14_CATEGORY_BASE
                + scene.block_id as u32 * openmt2001::building_geometry::CATEGORY_STRIDE;
            let [x, y, w, d] = spec.lot;
            let endpoints = [x, y, x + w - 1, y + d - 1];
            let matches: Vec<_> = state
                .world
                .buildings
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row[0x3f] != 0
                        && word(*row, 0x40) == category_reference
                        && row[4..8] == endpoints
                })
                .collect();
            assert_eq!(
                matches.len(),
                1,
                "authored lot missing or duplicated: {:?}",
                spec.lot
            );
            let (slot, row) = matches[0];
            assert_eq!(word(row, 0), slot as u32);
            assert_eq!(row[8], spec.height);
            assert_eq!(word(row, 0x20), spec.variation);
            assert_eq!(word(row, 0x24), u32::from(spec.kind));
            assert_eq!(row[0x2c], spec.quality);
            assert_eq!(
                [
                    f32::from_bits(word(row, 0xc)),
                    f32::from_bits(word(row, 0x10))
                ],
                scene.center
            );
            assert_eq!(row[0x2d], state.world.categories[scene.block_id][5]);
            let business_slot = word(row, 0x1c) as usize;
            let business = &state.world.ledgers.businesses[business_slot];
            assert_ne!(business[0x21c], 0);
            assert_eq!(word(business, 0x210), business_slot as u32);
            assert_eq!(
                u16::from_le_bytes(business[8..10].try_into().unwrap()) as usize,
                slot
            );
            assert_eq!(spec.businesses.len(), 1);
            let request = openmt2001::map_script::Business::from_arguments(
                &[
                    openmt2001::lua4::Value::Number(slot as f64),
                    openmt2001::lua4::Value::Bytes(spec.businesses[0].as_bytes().to_vec()),
                ],
                &state.world.catalog.info,
            )
            .unwrap()
            .unwrap();
            assert_eq!(word(business, 4), request.kind);
            assert_eq!(
                word(business, 0),
                word(
                    state.world.catalog.info.row(request.kind as usize).unwrap(),
                    0xc
                )
            );
            let binding = state.models.bindings.get(&slot).unwrap();
            assert_eq!(word(row, 0x170), binding.handle);
            assert_ne!(binding.handle, 0);
            let model = &state.models.models[binding.handle as usize - 1];
            let prepared = openmt2001::geometry::prepare(&scene.layers);
            let positions: Vec<_> = openmt2001::geometry::facade_positions(&prepared)
                .into_iter()
                .chain(openmt2001::roof::mesh(&prepared).positions)
                .map(|p| scene.local_position(p))
                .collect();
            assert!(!positions.is_empty());
            assert_eq!(model.positions, positions);
            let bytes: Vec<_> = positions
                .iter()
                .flat_map(|p| p.iter().flat_map(|v| v.to_le_bytes()))
                .collect();
            assert_eq!(
                model.definition,
                openmt2001::mesh::model_definition(&bytes).unwrap()
            );
            assert_eq!(
                binding.translation,
                std::array::from_fn(|i| f32::from_bits(word(row, 0x58 + i * 4)))
            );
            identities.push((slot, business_slot));
        }
        let buildings: std::collections::BTreeSet<_> =
            identities.iter().map(|pair| pair.0).collect();
        let businesses: std::collections::BTreeSet<_> =
            identities.iter().map(|pair| pair.1).collect();
        assert_eq!(
            buildings.len(),
            specs.len(),
            "authored building identities must be unique"
        );
        assert_eq!(
            businesses.len(),
            specs.len(),
            "authored business identities must be unique"
        );
        identities
    }

    #[test]
    #[ignore = "requires supplied originals; bounded sequential25-map NET1 startup only"]
    fn original_25_skirmish_maps_net1_startup_matrix() {
        let started = std::time::Instant::now();
        std::thread::Builder::new().stack_size(64*1024*1024).spawn(move || {
            let budget = std::time::Duration::from_secs(120);
            let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
            let executable = read("OPENMT2001_EXE");
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let build = read("OPENMT2001_BUILD_FILE");
            let parameters = read("OPENMT2001_PARAMETERS");
            let timetable = read("OPENMT2001_TIMETABLE");
            let road_routes = read("OPENMT2001_ROAD_ROUTE_TABLE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let board = openmt2001::board::Board::from_v14(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
            let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
            let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
            let scripts = default_path.parent().unwrap().parent().unwrap();
            let language = openmt2001::fixtures::path("EXE")
                .parent().unwrap().parent().unwrap().join("US/language/usa");
            let names = character_names(&image, &language).unwrap();
            assert_eq!(names[0].as_slice(), b"Racecar");
            assert_eq!(names[1].as_slice(), b"Wheelbarrow");
            let era_styles = load_era_styles(
                Path::new(&openmt2001::fixtures::path("PARAMETERS")).parent().unwrap()).unwrap();
            let maps = crate::skirmish_maps::Catalog::discover(scripts).unwrap();
            assert_eq!(maps.names().len(), 25, "requires actual supplied25-map catalog");
            let mut seen = std::collections::BTreeSet::new();
            let mut completed = 0;
            use crate::skirmish_maps::MapIdentity;
            use openmt2001::skirmish::roster::{Kind, Participant};
            for (case, basename) in maps.names().iter().enumerate() {
                assert!(started.elapsed() < budget, "120s total startup matrix budget before {basename}");
                assert!(seen.insert(basename.to_ascii_lowercase()), "duplicate supplied basename: {basename}");
                let case_started = std::time::Instant::now();
                println!("NET1_MATRIX BEGIN case={}/25 map={basename} total_ms={}", case+1, started.elapsed().as_millis());
                let resolved = maps.resolve(scripts, basename).unwrap();
                let specs = openmt2001::scene::parse_static_map(std::str::from_utf8(&resolved.bytes).unwrap()).unwrap();
                let source_len = u32::try_from(resolved.bytes.len()).unwrap();
                let checksum = resolved.bytes.iter().fold(0u32, |sum,&byte| sum.wrapping_add(u32::from(byte)));
                let expected_identity = if basename == "empty.lua" && resolved.bytes.is_empty() {
                    MapIdentity::LegacyEmpty
                } else {
                    MapIdentity::Named { basename: basename.clone(), checksum, source_len }
                };
                assert_eq!(resolved.identity, expected_identity);
                let expected_buildings = specs.len();
                let expected_businesses: usize = specs.iter().map(|spec| spec.businesses.len()).sum();
                let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
                let launch = SkirmishLaunch {
                    level_id: 300001, directory: "net1".into(), map: basename.clone(),
                    objective_kind: 6, target: 2000., difficulty: 0,
                    participants: vec![
                        Participant { kind: Kind::Human, token: Some(0), name: names[0].clone(), ready: true, difficulty: 0 },
                        Participant { kind: Kind::Ai, token: Some(1), name: names[1].clone(), ready: true, difficulty: 0 },
                    ],
                };
                // Exactly one live Session per case; drop explicitly before the next map.
                let session = Session::initialize_skirmish(Inputs {
                    tree_locators: None, image: &image, board: &board, file: &file,
                    people: openmt2001::people::PeopleModels::load(&image, &index, &archive).unwrap(),
                    vehicle_models: openmt2001::traffic::road::models::Models::from_original(&image, &index, &archive).unwrap(),
                    transport_models: openmt2001::traffic::models::Models::from_original(&image, &index, &archive).unwrap(),
                    road_routes: road_routes.clone(), scripts,
                    parameters: std::str::from_utf8(&parameters).unwrap(), era_styles: era_styles.clone(),
                    timetable: std::str::from_utf8(&timetable).unwrap(), scenario: "NET1",
                    difficulty: 0, selected_character: 0, profile_name: &names[0], character_names: &names,
                }, Policy { seed: 0x12345678, auxiliary_prior_bytes: [0;0x11c] }, catalog, launch).unwrap();
                assert_eq!(session.skirmish_map.as_ref(), Some(&expected_identity), "{basename}");
                let selected = session.catalog.selected_level().unwrap();
                assert_eq!(selected.id(), 300001);
                assert!(!selected.single_player());
                assert!(selected.directory().eq_ignore_ascii_case(b"net1"));
                assert_eq!(selected.objective_target(), 2000.);
                assert_eq!(session.world.objective.kind, 6);
                assert_eq!(session.world.objective.target, 2000.);
                assert_eq!(session.difficulty, 0);
                assert_eq!(session.world.player_money, 8000.);
                assert_eq!(session.roster.manager.players.len(), 2);
                assert_eq!(session.roster.status.ai_count, 1);
                let word = |bytes: &[u8], at: usize| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
                for (slot,token,kind) in [(0,0,0),(1,1,1)] {
                    let saved = &session.saved_player_metadata[slot];
                    assert_eq!(word(saved,0), token);
                    assert_eq!(word(saved,4), kind);
                    assert_eq!(word(saved,44), 0);
                    assert_eq!(&saved[8..8+names[slot].len()], names[slot].as_slice());
                    let player = &session.roster.manager.players[slot].bytes;
                    assert_eq!(word(player,0x150), token);
                    assert_eq!(word(player,0x154), kind);
                }
                assert!(session.campaign_profile.is_none());
                assert!(session.network_command_timer.is_none());
                let buildings = session.world.buildings.iter().filter(|row| row[0x3f] != 0).count();
                let businesses = session.world.ledgers.businesses.iter().filter(|row| row[0x21c] != 0).count();
                assert_eq!(buildings, expected_buildings, "{basename}: active building count");
                assert_eq!(businesses, expected_businesses, "{basename}: active business count");
                let identities = check_authored_records(&session, &specs, &board, &file);
                assert_eq!(identities.len(), expected_buildings);
                assert_eq!(session.models.bindings.len(), expected_buildings, "{basename}: exact model binding count");
                for &(building,business) in &identities {
                    assert_eq!(session.world.buildings[building][0x2d], 0);
                    assert_eq!(session.world.ledgers.businesses[business][0xa], 0);
                }
                drop(session);
                assert!(started.elapsed() < budget, "120s total startup matrix budget after {basename}");
                completed += 1;
                println!("NET1_MATRIX PASS case={}/25 map={basename} bytes={source_len} sum={checksum} specs={expected_buildings} buildings={buildings} businesses={businesses} bindings={} case_ms={} total_ms={}",
                    case+1, identities.len(), case_started.elapsed().as_millis(), started.elapsed().as_millis());
            }
            assert_eq!(seen.len(), 25);
            assert_eq!(completed, 25);
            assert!(started.elapsed() < budget, "120s total startup matrix budget at completion");
            println!("NET1_MATRIX COMPLETE cases=25 level=300001 difficulty=0 objective=6 target=2000 seed=12345678 host_frames=0 total_ms={}", started.elapsed().as_millis());
        }).unwrap().join().unwrap();
    }

    fn skirmish_map_cold_reload_continuation(map: &'static str) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(move || {
                let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
                let executable = read("OPENMT2001_EXE");
                let directory = read("OPENMT2001_ARCHIVE_DIR");
                let archive = read("OPENMT2001_ARCHIVE_BIN");
                let build = read("OPENMT2001_BUILD_FILE");
                let _sun = read("OPENMT2001_SUNCOLS");
                let parameters = read("OPENMT2001_PARAMETERS");
                let timetable = read("OPENMT2001_TIMETABLE");
                let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
                let board = openmt2001::board::Board::from_v14(&image).unwrap();
                let file = openmt2001::build_file::BuildFile::parse(&build).unwrap();
                let index =
                    openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
                        .unwrap();
                let default_path = openmt2001::fixtures::path("DEFAULT_SETTINGS");
                let scripts = default_path.parent().unwrap().parent().unwrap();
                let language = openmt2001::fixtures::path("EXE")
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa");

                use crate::skirmish_maps::{MapIdentity, Selection};
                // Read the actual supplied launch file, never manufacture or patch it.
                let authored = if map == "monopolymap.lua" {
                    let path =
                        crate::campaign_store::original_path(scripts, "MAPS/monopolymap.lua")
                            .unwrap()
                            .unwrap();
                    let bytes = fs::read(path).unwrap();
                    assert_eq!(bytes.len(), 222);
                    let checksum = bytes
                        .iter()
                        .fold(0u32, |sum, &byte| sum.wrapping_add(u32::from(byte)));
                    assert_eq!(checksum, 15_786);
                    let specs =
                        openmt2001::scene::parse_static_map(std::str::from_utf8(&bytes).unwrap())
                            .unwrap();
                    assert_eq!(specs.len(), 2);
                    Some((
                        MapIdentity::Named {
                            basename: map.into(),
                            checksum,
                            source_len: 222,
                        },
                        specs,
                    ))
                } else {
                    assert_eq!(map, "empty.lua");
                    None
                };
                let names = character_names(&image, &language).unwrap();
                use openmt2001::skirmish::roster::{Kind, Participant};
                let catalog = crate::campaign_store::load_catalog(&image, scripts).unwrap();
                let launch = SkirmishLaunch {
                    level_id: 300001,
                    directory: "net1".into(),
                    map: map.into(),
                    objective_kind: 6,
                    target: 3500.,
                    difficulty: 1,
                    participants: vec![
                        Participant {
                            kind: Kind::Human,
                            token: Some(4),
                            name: b"Fixture Player".to_vec(),
                            ready: true,
                            difficulty: 2,
                        },
                        Participant {
                            kind: Kind::Ai,
                            token: Some(1),
                            name: b"Explicit Opponent".to_vec(),
                            ready: true,
                            difficulty: 0,
                        },
                    ],
                };
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
                    road_routes: std::fs::read(openmt2001::fixtures::path("ROAD_ROUTE_TABLE"))
                        .unwrap(),
                    scripts,
                    parameters: std::str::from_utf8(&parameters).unwrap(),
                    era_styles: load_era_styles(
                        Path::new(&openmt2001::fixtures::path("PARAMETERS"))
                            .parent()
                            .unwrap(),
                    )
                    .unwrap(),
                    timetable: std::str::from_utf8(&timetable).unwrap(),
                    scenario: "NET1",
                    difficulty: 1,
                    selected_character: 4,
                    profile_name: b"Fixture Player",
                    character_names: &names,
                };
                let policy = || Policy {
                    seed: 0x12345678,
                    auxiliary_prior_bytes: [0; 0x11c],
                };
                let mut session =
                    Session::initialize_skirmish(inputs(), policy(), catalog, launch).unwrap();
                let word = |bytes: &[u8], at: usize| {
                    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
                };
                // Preserve the cold fixture's two-pair distinctness assertions.
                let check_authored =
                    |state: &Session<'_>, specs: &[openmt2001::scene::BuildingSpec]| {
                        let identities = check_authored_records(state, specs, &board, &file);
                        assert_ne!(identities[0].0, identities[1].0);
                        assert_ne!(identities[0].1, identities[1].1);
                        identities
                    };
                let authored_identities = authored.as_ref().map(|(identity, specs)| {
                    assert_eq!(session.skirmish_map.as_ref(), Some(identity));
                    assert_eq!(
                        session
                            .world
                            .buildings
                            .iter()
                            .filter(|row| row[0x3f] != 0)
                            .count(),
                        2
                    );
                    assert_eq!(
                        session
                            .world
                            .ledgers
                            .businesses
                            .iter()
                            .filter(|row| row[0x21c] != 0)
                            .count(),
                        2
                    );
                    let identities = check_authored(&session, specs);
                    for &(building, business) in &identities {
                        // Original AddBuilding/AddBusiness pass neutral owner0.
                        assert_eq!(session.world.buildings[building][0x2d], 0);
                        assert_eq!(session.world.ledgers.businesses[business][0xa], 0);
                    }
                    identities
                });
                let check_selection = |save: &openmt2001::save::SaveFile<'_>| {
                    let extension = openmt2001::save::extension::Extension::parse(save)
                        .unwrap()
                        .unwrap();
                    let (version, identity) = match &authored {
                        Some((identity, _)) => (6, identity.clone()),
                        None => (5, MapIdentity::LegacyEmpty),
                    };
                    assert_eq!(extension.version, version);
                    let payload = extension.required(18).unwrap();
                    if version == 6 {
                        assert_eq!(
                            payload[..28]
                                .chunks_exact(4)
                                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                                .collect::<Vec<_>>(),
                            [
                                300001,
                                3500f32.to_bits(),
                                1,
                                1,
                                222,
                                15_786,
                                map.len() as u32
                            ]
                        );
                        assert_eq!(&payload[28..], b"monopolymap.lua");
                    }
                    assert_eq!(
                        payload.len(),
                        if version == 6 { 28 + map.len() } else { 16 }
                    );
                    let selection = Selection::decode(extension.version, payload).unwrap();
                    assert_eq!(selection.level_id, 300001);
                    assert_eq!(selection.target.to_bits(), 3500f32.to_bits());
                    assert_eq!(selection.map, identity);
                    assert_eq!(selection.encode(version).unwrap(), payload);
                };
                assert_eq!(session.catalog.selected_level().unwrap().id(), 300001);
                assert!(!session.catalog.selected_level().unwrap().single_player());
                assert_eq!(
                    session.catalog.selected_level().unwrap().objective_target(),
                    3500.
                );
                assert_eq!(session.world.objective.kind, 6);
                assert_eq!(session.world.objective.target, 3500.);
                assert_eq!(session.difficulty, 1);
                assert_eq!(session.roster.manager.players.len(), 2);
                assert_eq!(session.roster.status.ai_count, 1);
                assert_eq!(session.world.player_money, 6500.);
                assert_eq!(session.world.ledgers.residents.active_count, 251);
                for (slot, (token, kind, difficulty, peer, name)) in [
                    (4, 0, 2, 0, b"Fixture Player".as_slice()),
                    (1, 1, 0, u32::MAX, b"Explicit Opponent".as_slice()),
                ]
                .into_iter()
                .enumerate()
                {
                    let row = &session.saved_player_metadata[slot];
                    assert_eq!(word(row, 0), token);
                    assert_eq!(word(row, 4), kind);
                    assert_eq!(word(row, 40), peer);
                    assert_eq!(word(row, 44), difficulty);
                    assert_eq!(&row[8..8 + name.len()], name);
                    assert_eq!(
                        word(&session.roster.manager.players[slot].bytes, 0x150),
                        token
                    );
                    assert_eq!(
                        word(&session.roster.manager.players[slot].bytes, 0x154),
                        kind
                    );
                    if kind == 1 {
                        assert_eq!(
                            word(&session.roster.manager.players[slot].bytes, 0x13f4),
                            difficulty
                        );
                    }
                    assert_eq!(session.world.ledgers.players[slot].cash, 6500.);
                }
                assert!(session.campaign_profile.is_none());
                assert!(session.network_command_timer.is_none());
                // A deliberately distinct active objective must not replace selected+870.
                session.world.objective.target = 3499.;
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
                let mut host = crate::live_host::Host::new(camera.eye);
                host.running = true;
                let initial = session.world.clock.timing[0];
                for _ in 0..30 {
                    host.tick(&mut session, &assets);
                    assert!(host.failure.is_none(), "{:?}", host.failure);
                }
                assert!(session.world.clock.timing[0] > initial);
                struct Scratch(std::path::PathBuf);
                impl Drop for Scratch {
                    fn drop(&mut self) {
                        let _ = fs::remove_dir_all(&self.0);
                    }
                }
                let path = std::path::PathBuf::from("work/save").join(format!(
                    "net1-cold-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::create_dir(&path).unwrap();
                let scratch = Scratch(path);
                let bytes = session.save_v2(&image, cameras, 1).unwrap();
                let saved_path = crate::save_menu::write_new(&scratch.0, &bytes).unwrap();
                let bytes = fs::read(saved_path).unwrap();
                let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
                check_selection(&save);
                assert_eq!(save.level_metadata().unwrap()[0], 300001);
                assert_eq!(save.game_mode().unwrap(), 1);
                let mut resumed = Session::load_original(inputs(), policy(), &bytes).unwrap();
                resumed
                    .restore_v2_extension_with_image(&save, &image)
                    .unwrap();
                assert_eq!(
                    resumed.catalog.selected_level().unwrap().objective_target(),
                    3500.
                );
                assert_eq!(resumed.world.objective.target, 3499.);
                assert_eq!(resumed.world.objective.kind, 6);
                assert_eq!(resumed.difficulty, 1);
                assert_eq!(
                    &resumed.saved_player_metadata[..2],
                    &session.saved_player_metadata[..2]
                );
                assert!(resumed.campaign_profile.is_none());
                assert!(resumed.network_command_timer.is_none());
                assert_eq!(resumed.skirmish_map, session.skirmish_map);
                if let Some((identity, specs)) = &authored {
                    assert_eq!(resumed.skirmish_map.as_ref(), Some(identity));
                    assert_eq!(
                        check_authored(&session, specs),
                        *authored_identities.as_ref().unwrap()
                    );
                    assert_eq!(
                        check_authored(&resumed, specs),
                        *authored_identities.as_ref().unwrap()
                    );
                    assert_eq!(
                        resumed
                            .world
                            .buildings
                            .iter()
                            .filter(|r| r[0x3f] != 0)
                            .count(),
                        session
                            .world
                            .buildings
                            .iter()
                            .filter(|r| r[0x3f] != 0)
                            .count()
                    );
                    assert_eq!(
                        resumed
                            .world
                            .ledgers
                            .businesses
                            .iter()
                            .filter(|r| r[0x21c] != 0)
                            .count(),
                        session
                            .world
                            .ledgers
                            .businesses
                            .iter()
                            .filter(|r| r[0x21c] != 0)
                            .count()
                    );
                    for &(building, business) in authored_identities.as_ref().unwrap() {
                        assert_eq!(
                            resumed.world.buildings[building][0x2d],
                            session.world.buildings[building][0x2d]
                        );
                        assert_eq!(
                            resumed.world.ledgers.businesses[business],
                            session.world.ledgers.businesses[business]
                        );
                    }
                }
                let reexport = resumed.save_v2(&image, cameras, 1).unwrap();
                let reexport_save = openmt2001::save::SaveFile::parse(&reexport).unwrap();
                check_selection(&reexport_save);
                let mut other_host = crate::live_host::Host::new(camera.eye);
                other_host.running = true;
                // Both hosts restart at the same wall-frame boundary, as visible load does.
                host = crate::live_host::Host::new(camera.eye);
                host.running = true;
                let before = resumed.world.clock.timing[0];
                for _ in 0..60 {
                    host.tick(&mut session, &assets);
                    other_host.tick(&mut resumed, &assets);
                    assert!(host.failure.is_none(), "{:?}", host.failure);
                    assert!(other_host.failure.is_none(), "{:?}", other_host.failure);
                    assert_eq!(session.world.clock, resumed.world.clock);
                    assert_eq!(
                        session.world.ledgers.businesses,
                        resumed.world.ledgers.businesses
                    );
                    assert_eq!(
                        session.world.pending_world.to_bytes(),
                        resumed.world.pending_world.to_bytes()
                    );
                    assert_eq!(session.roster.profiles, resumed.roster.profiles);
                    assert_eq!(session.script_random_state, resumed.script_random_state);
                    for (left, right) in session
                        .roster
                        .manager
                        .players
                        .iter()
                        .zip(&resumed.roster.manager.players)
                    {
                        assert_eq!(left.bytes, right.bytes);
                    }
                }
                assert!(resumed.world.clock.timing[0] > before);
                // Complete native+modern serialization compares the entire portable world,
                // retained scripts, participant metadata and scheduler after continuation.
                assert_eq!(
                    session.save_v2(&image, cameras, 1).unwrap(),
                    resumed.save_v2(&image, cameras, 1).unwrap()
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
#[path = "selected_income_tests.rs"]
mod selected_income_tests;
