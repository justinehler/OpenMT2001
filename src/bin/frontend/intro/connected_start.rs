//! Connected construction stops at the initial city. Command/deadline exchange,
//! auction barriers, chat and peer-loss handling remain #35–#38. The scheduler
//! holds at its initial deadline (25); no local command transport is substituted.
use super::*;
use openmt2001::network::{
    self, Content, RowKind, SessionBinding,
    lobby::worker::{Handle, Ticket, Update},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub struct Request {
    pub ticket: Ticket,
    pub worker: Handle,
}
pub(super) struct Prepared {
    request: Request,
    stage: Staged,
}
pub(super) struct Pending {
    request: Option<Request>,
    staged: mpsc::Receiver<Staged>,
    ready: Option<Staged>,
    validated: bool,
}
impl Pending {
    pub(super) fn start(request: Request, inputs: Arc<SceneInputs>) -> Result<Self, String> {
        let (tx, staged) = mpsc::sync_channel(1);
        let ticket = request.ticket.clone();
        request.worker.stage(move || {
            let stage = inputs.stage_connected(&ticket)?;
            let content = stage.content;
            tx.try_send(stage)
                .map_err(|_| "Connected staging was cancelled".to_string())?;
            Ok(content)
        })?;
        Ok(Self {
            request: Some(request),
            staged,
            ready: None,
            validated: false,
        })
    }
    pub(super) fn poll(&mut self) -> Result<(), String> {
        let worker = &self
            .request
            .as_ref()
            .ok_or("Connected startup already consumed")?
            .worker;
        while let Some(update) = worker.poll() {
            match update {
                Update::Validated => self.validated = true,
                Update::Failed(error) => return Err(error),
                _ => (),
            }
        }
        worker.validate()?;
        if self.ready.is_none() {
            self.ready = self.staged.try_recv().ok();
        }
        Ok(())
    }
    pub(super) fn take_ready(&mut self) -> Option<Prepared> {
        if !self.validated {
            return None;
        }
        Some(Prepared {
            stage: self.ready.take()?,
            request: self.request.take()?,
        })
    }
}
pub struct Connection {
    pub binding: SessionBinding,
    pub worker: Handle,
}

pub(crate) struct Staged {
    pub catalog: openmt2001::level_catalog::Catalog,
    pub launch: crate::fresh_city::SkirmishLaunch,
    pub scripts: BTreeMap<String, Vec<u8>>,
    pub map: crate::skirmish_maps::ResolvedMap,
    pub peers: Vec<Option<u8>>,
    pub local_slot: usize,
    parameters: String,
    timetable: String,
    era_styles: [openmt2001::building_styles::Styles; 8],
    road_routes: Vec<u8>,
    pub content: Content,
}
impl Staged {
    pub fn read(&self, relative: &str) -> Result<Vec<u8>, String> {
        self.scripts
            .get(&relative.to_ascii_lowercase())
            .cloned()
            .ok_or_else(|| format!("Missing staged connected content: {relative}"))
    }
}
fn hash_part(hash: &mut Sha256, name: &str, bytes: &[u8]) {
    hash.update((name.len() as u64).to_le_bytes());
    hash.update(name);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
impl SceneInputs {
    fn stage_connected(&self, ticket: &Ticket) -> Result<Staged, String> {
        let snapshot = &ticket.view.snapshot;
        if !snapshot.start_enabled() {
            return Err("Invalid admitted connected roster".into());
        }
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let scripts_root = self
            .maps
            .parent()
            .ok_or("Missing connected script directory")?;
        let mut catalog = crate::campaign_store::load_catalog(&image, scripts_root)?;
        catalog.reset_progress();
        let selected = catalog
            .find(snapshot.settings.level)
            .ok_or("Missing connected level")?;
        let level = &catalog.levels[selected];
        let directory = std::str::from_utf8(level.directory())
            .map_err(|_| "Invalid connected level directory")?
            .to_owned();
        if !(300000..=300006).contains(&level.id())
            || level.single_player()
            || !directory.eq_ignore_ascii_case(&format!("net{}", level.id() - 300000))
        {
            return Err("Invalid connected level identity".into());
        }
        let mut scripts = BTreeMap::new();
        for relative in [
            "auctioncamera",
            "sound",
            "general",
            "commoditysettings",
            "blocksettings",
            "businesssettings",
            "buildingsettings",
            "default",
            "hub",
            "ai",
        ]
        .map(|n| format!("DEFAULT/{n}.lua"))
        .into_iter()
        .chain(["hub", "initialpref", "ai"].map(|n| format!("{directory}/{n}.lua")))
        {
            match crate::campaign_store::original_path(scripts_root, &relative)? {
                Some(path) => {
                    let bytes = fs::read(path)
                        .map_err(|e| format!("Missing connected content {relative}: {e}"))?;
                    scripts.insert(relative.to_ascii_lowercase(), bytes);
                }
                None if relative == format!("{directory}/ai.lua") => (),
                None => return Err(format!("Missing connected content: {relative}")),
            }
        }
        let objective = openmt2001::level_script::objective_selection::evaluate_setup(
            &image,
            level,
            &scripts[&format!("{directory}/hub.lua").to_ascii_lowercase()],
        )?
        .kind;
        let target = f32::from_bits(snapshot.settings.parameter_bits);
        let options = openmt2001::skirmish::Options::from_pe(&image)?;
        let rule = options
            .rules()
            .iter()
            .find(|r| r.kind == objective)
            .ok_or("Unknown connected objective")?;
        if !target.is_finite()
            || target.fract() != 0.
            || target < rule.minimum as f32
            || target > rule.maximum as f32
        {
            return Err("Connected parameter bits outside original objective bounds".into());
        }
        let map_name = std::str::from_utf8(&snapshot.settings.map)
            .map_err(|_| "Invalid connected map name")?;
        let map = crate::skirmish_maps::Catalog::discover(scripts_root)?
            .resolve(scripts_root, map_name)
            .map_err(|e| format!("Missing or invalid connected map content: {e}"))?;
        let rows: Vec<_> = snapshot
            .rows
            .iter()
            .filter(|r| matches!(r.player.kind, RowKind::Human | RowKind::Ai))
            .collect();
        let peers: Vec<_> = rows.iter().map(|r| r.peer).collect();
        if peers.first() != Some(&Some(0)) {
            return Err("Connected host must occupy compact slot zero".into());
        }
        let local_slot = peers
            .iter()
            .position(|&p| p == Some(ticket.view.local_peer))
            .ok_or("Local peer absent from compact connected roster")?;
        let launch = crate::fresh_city::SkirmishLaunch {
            level_id: level.id(),
            directory: directory.clone(),
            map: map_name.into(),
            objective_kind: objective,
            target,
            difficulty: snapshot.settings.difficulty as i32,
            participants: rows
                .iter()
                .map(|r| openmt2001::skirmish::roster::Participant {
                    kind: if r.player.kind == RowKind::Human {
                        openmt2001::skirmish::roster::Kind::Human
                    } else {
                        openmt2001::skirmish::roster::Kind::Ai
                    },
                    token: Some(r.player.token as usize),
                    name: r.player.name.clone(),
                    ready: r.player.ready,
                    difficulty: r.difficulty as i32,
                })
                .collect(),
        };
        launch.validate_roster(true)?;
        let parameters_root = self.installer.join("parameters");
        let read_text = |name: &str| {
            fs::read_to_string(parameters_root.join(name))
                .map_err(|e| format!("Missing connected content {name}: {e}"))
        };
        let parameters = read_text("parameter.prm")?;
        let timetable = read_text("timetable.prm")?;
        let era_text = (1930..=2000)
            .step_by(10)
            .map(|year| read_text(&format!("{year}.prm")))
            .collect::<Result<Vec<_>, _>>()?;
        let era_styles = era_text
            .iter()
            .map(|text| openmt2001::building_styles::Styles::from_parameters(text))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Connected era bank size")?;
        let road_routes = fs::read(self.installer.join("Standard/route_smalltable.bin"))
            .map_err(|e| format!("Missing connected road routes: {e}"))?;
        let mut hash = Sha256::new();
        hash_part(
            &mut hash,
            "startup-version",
            concat!(env!("CARGO_PKG_VERSION"), "/connected-city-1").as_bytes(),
        );
        for (name, bytes) in [
            ("original-executable", self.executable.as_slice()),
            ("archive-directory", &self.directory),
            ("archive", &self.archive),
            ("build-file", &self.build_file),
            ("suncols", &self.suncols),
            ("parameters", parameters.as_bytes()),
            ("timetable", timetable.as_bytes()),
            ("road-routes", &road_routes),
            ("map", &map.bytes),
        ] {
            hash_part(&mut hash, name, bytes);
        }
        let palette: Vec<_> = self
            .style
            .pavement
            .into_iter()
            .chain(self.style.text)
            .flat_map(u32::to_le_bytes)
            .collect();
        hash_part(&mut hash, "district-palette", &palette);
        for (name, bytes) in &scripts {
            hash_part(&mut hash, name, bytes);
        }
        for (i, text) in era_text.iter().enumerate() {
            hash_part(&mut hash, &format!("era-{i}"), text.as_bytes());
        }
        // The exact admitted six-row snapshot pins compaction, peers and owner order.
        let roster = network::codec::encode(&network::Frame {
            session: [1; 16],
            sequence: 0,
            message: network::Message::Lobby(snapshot.clone()),
        })
        .map_err(|e| e.to_string())?;
        hash_part(&mut hash, "registrations", &roster);
        hash_part(&mut hash, "level", &level.bytes);
        let content = Content {
            level: level.id(),
            parameter_bits: snapshot.settings.parameter_bits,
            difficulty: snapshot.settings.difficulty,
            fingerprint: hash.finalize().into(),
        };
        if ticket.expected.is_some_and(|expected| expected != content) {
            return Err("Connected startup version/content digest mismatch".into());
        }
        Ok(Staged {
            catalog,
            launch,
            scripts,
            map,
            peers,
            local_slot,
            parameters,
            timetable,
            era_styles,
            road_routes,
            content,
        })
    }
    pub(super) fn fresh_connected_joined(
        &self,
        prepared: Prepared,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        let Prepared { request, stage } = prepared;
        request.worker.validate()?;
        let mut session = self.construct_connected(&request.ticket, &stage, dispatch)?;
        let binding = request
            .worker
            .bind_world(session.world.topology.epoch().serial())?;
        session.connected = Some(Connection {
            binding,
            worker: request.worker,
        });
        let loaded = self.inspect_fresh(&mut session, camera, cache)?;
        eprintln!(
            "Connected city initialized: level {}, local compact slot {}, players {}, seed {}, initial deadline 25. Command/deadline exchange remains unavailable (#35).",
            stage.launch.level_id,
            stage.local_slot,
            session.roster.manager.players.len(),
            request.ticket.seed
        );
        Ok((loaded, session))
    }
    fn construct_connected<'s>(
        &'s self,
        ticket: &Ticket,
        stage: &Staged,
        dispatch: Option<crate::process_dispatch::Seed>,
    ) -> Result<crate::fresh_city::Session<'s>, String> {
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let process = self
            .process_runtime
            .clone()
            .ok_or("Connected startup requires the process seed boundary")?;
        let session = crate::fresh_city::Session::initialize_connected_joined(
            crate::fresh_city::Inputs {
                tree_locators: Some(
                    openmt2001::trees::Assets::load(&image, &index, &self.archive)?.locators,
                ),
                image: &image,
                board: &board,
                file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &self.archive)?,
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                road_routes: stage.road_routes.clone(),
                scripts: self.maps.parent().ok_or("Missing scripts")?,
                parameters: &stage.parameters,
                timetable: &stage.timetable,
                era_styles: stage.era_styles.clone(),
                scenario: &stage.launch.directory,
                difficulty: stage.launch.difficulty,
                selected_character: stage.launch.participants[stage.local_slot].token.unwrap(),
                profile_name: &stage.launch.participants[stage.local_slot].name,
                character_names: &self.character_names,
            },
            crate::fresh_city::Policy {
                seed: ticket.seed,
                auxiliary_prior_bytes: [0; 0x11c],
            },
            stage,
            Some(&self.style),
            process,
            dispatch,
        )?;
        Ok(session)
    }
}

#[cfg(test)]
#[path = "connected_start_tests.rs"]
mod tests;
