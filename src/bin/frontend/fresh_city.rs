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
#[cfg(test)]
#[path = "fresh_city/test_observers.rs"]
mod test_observers;
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
// The Skirmish launch moved into the library (#99).
pub use openmt2001::skirmish::launch::SkirmishLaunch;

/// Original 499160 selects the single-player option; 4956c0 resets it
/// before tutorials and 4a5c50 resets it on entering network setup.
fn fresh_session_option_word(level: u32, custom: bool, skirmish: bool, network: bool) -> u32 {
    if network {
        0
    } else if custom {
        3
    } else if skirmish {
        1
    } else if level == 400002 {
        2
    } else {
        0
    }
}

#[cfg(test)]
#[path = "session_option_word_test.rs"]
mod session_option_word_tests;

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
    /// Raw original 0x8fca9c, retained independently of logic/UI modes.
    pub session_option_word: u32,
    /// Loaded clock mode (0x822d30), also marks a restored session.
    pub saved_logic_mode: Option<u32>,
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
            self.submit_build_with_quote(request, assets, Some(request.displayed_quote), event)
                .map(Some)
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
        self.inspect_build_with_quote(request, assets, None)
    }

    fn inspect_build_with_quote(
        &mut self,
        request: &crate::build_panel::Request,
        assets: &CommandAssets<'_, '_>,
        displayed_quote: Option<f32>,
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
        let allowance = i32::from_le_bytes(assets.image.at(0x541c7c, 4)?.try_into().unwrap());
        let hour = self.world.clock.hour();
        let (total, affordable) = openmt2001::construction::confirmation_quote(
            self.world.ledgers.players[player].cash,
            allowance,
            displayed_quote,
            || {
                openmt2001::live_commands::quote(
                    &mut command,
                    &mut self.world.city,
                    &self.roster,
                    assets.image,
                    hour,
                )
                .map(|total| total as f32)
            },
        )?;
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
        // Scripted producers do not retain a displayed planner quote.
        self.submit_build_with_quote(request, assets, None, event)
    }

    fn submit_build_with_quote(
        &mut self,
        request: &crate::build_panel::Request,
        assets: &CommandAssets<'_, '_>,
        displayed_quote: Option<f32>,
        event: impl FnMut(openmt2001::live_commands::Event) -> Result<(), String>,
    ) -> Result<BuildInspection, String> {
        let mut result = self.inspect_build_with_quote(request, assets, displayed_quote)?;
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
        if let Some(connection) = &mut self.connected {
            let targets = openmt2001::network::commands::Targets::new(
                &self.world.city.buildings,
                &self.world.city.ledgers.businesses,
                &self.world.city.categories,
            );
            return connection.commands.submit(
                &connection.worker,
                command,
                &mut self.live_commands,
                &mut self.world.city.categories,
                targets,
                self.world.clock.timing[0],
            );
        }
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
        if let Some(connection) = &mut self.connected {
            if connection.synchronization.barrier.active {
                return Ok(());
            }
            return connection.commands.submit_auction(
                &connection.worker,
                descriptor,
                0,
                &mut self.live_commands,
                &mut self.world.city.categories,
                self.world.clock.timing[0],
            );
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
        scene.set_street_furniture_era(self.world.clock.calendar[2])?;
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
            session_option_word: fresh_session_option_word(
                id,
                is_custom,
                skirmish.is_some(),
                connected.is_some(),
            ),
            saved_logic_mode: None,
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
#[path = "fresh_city/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "fresh_city/skirmish_session_acceptance.rs"]
mod skirmish_session_acceptance;

#[cfg(test)]
#[path = "selected_income_tests.rs"]
mod selected_income_tests;
