//! `game headless`: run a city with no window, renderer or audio.
//!
//! The session starts the way the live session worker starts it
//! (`intro/scene_inputs.rs`): same process runtime, dispatch custody, styled
//! board and `live_host::Host::frame` boundary. Presentation sinks stay
//! detached. Auctions get a renderer stand-in that returns the live renderer's
//! transition values. Human input is replaced by a fixed policy, and the run
//! prints one JSON line per checkpoint so two runs can be diffed.
use std::io::Write;
use std::path::{Path, PathBuf};

const USAGE: &str = "\
usage: game headless (--scenario TOKEN | --save FILE) [options]

start (exactly one):
  --scenario TOKEN      fresh city, e.g. SCENARIO3, sandbox
  --save FILE           original v1.4 or V2 save

original files (either --fixtures, or --assets/--scene-root/--executable):
  --fixtures            resolve files through OPENMT2001_FIXTURES
  --assets DIR          extracted installer (parameters/, Standard/)
  --scene-root DIR      directory holding archive.bin, archive.DIR
  --executable FILE     v1.4 mc.exe
  --scripts DIR         default: <executable dir>/scripts
  --language DIR        default: <executable dir>/../US/language/usa

run:
  --ticks N             logic ticks to advance (default 250)
  --every N             report every N ticks (default: on each new hour)
  --difficulty 0|1|2    fresh city only (default 1)
  --character N         fresh city only (default 4)
  --seed N              fresh city host seed, decimal or 0x hex (default 0)
  --frame-ms N          host milliseconds per frame (default 40)
  --dialogs first|last|stop   reply to modal dialogs (default last)
  --auctions pass|wait|stop   human auction turn (default pass)
  --write-save FILE     write a V2 save when the run ends
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogPolicy {
    First,
    Last,
    Stop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuctionPolicy {
    Pass,
    Wait,
    Stop,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    Scenario {
        token: String,
        difficulty: i32,
        character: usize,
        seed: u32,
    },
    Save(PathBuf),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub installer: PathBuf,
    pub scene: PathBuf,
    pub executable: PathBuf,
    pub scripts: PathBuf,
    pub language: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub start: Start,
    pub paths: Paths,
    pub ticks: u32,
    pub every: Option<u32>,
    pub frame_ms: i32,
    pub dialogs: DialogPolicy,
    pub auctions: AuctionPolicy,
    pub write_save: Option<PathBuf>,
}

impl Paths {
    fn derive(
        installer: PathBuf,
        scene: PathBuf,
        executable: PathBuf,
        scripts: Option<PathBuf>,
        language: Option<PathBuf>,
    ) -> Result<Self, String> {
        let dir = executable
            .parent()
            .ok_or("--executable has no parent directory")?;
        let scripts = scripts.unwrap_or_else(|| dir.join("scripts"));
        let language = match language {
            Some(language) => language,
            None => dir
                .parent()
                .ok_or("--executable has no payload directory")?
                .join("US/language/usa"),
        };
        Ok(Self {
            installer,
            scene,
            executable,
            scripts,
            language,
        })
    }
    /// The private fixture manifest keeps the original install layout.
    fn from_fixtures(scripts: Option<PathBuf>, language: Option<PathBuf>) -> Result<Self, String> {
        let fixture = |name: &str| {
            openmt2001::fixtures::optional(name).ok_or(format!(
                "--fixtures: {name} is not configured (OPENMT2001_FIXTURES)"
            ))
        };
        let parameters = fixture("PARAMETERS")?;
        let installer = parameters
            .parent()
            .and_then(Path::parent)
            .ok_or("PARAMETERS fixture is not under an installer")?
            .to_path_buf();
        let scene = fixture("ARCHIVE_DIR")?
            .parent()
            .ok_or("ARCHIVE_DIR fixture has no directory")?
            .to_path_buf();
        Self::derive(installer, scene, fixture("EXE")?, scripts, language)
    }
}

fn number(text: &str) -> Result<u32, String> {
    match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => u32::from_str_radix(hex, 16),
        None => text.parse(),
    }
    .map_err(|_| format!("not a number: {text}"))
}

impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut scenario = None;
        let mut save = None;
        let mut fixtures = false;
        let (mut installer, mut scene, mut executable) = (None, None, None);
        let (mut scripts, mut language) = (None, None);
        let (mut difficulty, mut character, mut seed) = (1, 4, 0);
        let mut options = Self {
            start: Start::Save(PathBuf::new()),
            paths: Paths {
                installer: PathBuf::new(),
                scene: PathBuf::new(),
                executable: PathBuf::new(),
                scripts: PathBuf::new(),
                language: PathBuf::new(),
            },
            ticks: 250,
            every: None,
            frame_ms: 40,
            dialogs: DialogPolicy::Last,
            auctions: AuctionPolicy::Pass,
            write_save: None,
        };
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or(format!("{arg} requires a value"));
            match arg.as_str() {
                "--scenario" => scenario = Some(value()?),
                "--save" => save = Some(PathBuf::from(value()?)),
                "--fixtures" => fixtures = true,
                "--assets" => installer = Some(PathBuf::from(value()?)),
                "--scene-root" => scene = Some(PathBuf::from(value()?)),
                "--executable" => executable = Some(PathBuf::from(value()?)),
                "--scripts" => scripts = Some(PathBuf::from(value()?)),
                "--language" => language = Some(PathBuf::from(value()?)),
                "--ticks" => options.ticks = number(&value()?)?,
                "--every" => {
                    let every = number(&value()?)?;
                    if every == 0 {
                        return Err("--every must be positive".into());
                    }
                    options.every = Some(every);
                }
                "--difficulty" => {
                    difficulty = match value()?.as_str() {
                        "0" => 0,
                        "1" => 1,
                        "2" => 2,
                        other => return Err(format!("--difficulty must be 0, 1 or 2: {other}")),
                    }
                }
                "--character" => character = number(&value()?)? as usize,
                "--seed" => seed = number(&value()?)?,
                "--frame-ms" => {
                    options.frame_ms = i32::try_from(number(&value()?)?)
                        .ok()
                        .filter(|&ms| ms > 0)
                        .ok_or("--frame-ms must be a positive number")?
                }
                "--dialogs" => {
                    options.dialogs = match value()?.as_str() {
                        "first" => DialogPolicy::First,
                        "last" => DialogPolicy::Last,
                        "stop" => DialogPolicy::Stop,
                        other => {
                            return Err(format!("--dialogs must be first, last or stop: {other}"));
                        }
                    }
                }
                "--auctions" => {
                    options.auctions = match value()?.as_str() {
                        "pass" => AuctionPolicy::Pass,
                        "wait" => AuctionPolicy::Wait,
                        "stop" => AuctionPolicy::Stop,
                        other => {
                            return Err(format!("--auctions must be pass, wait or stop: {other}"));
                        }
                    }
                }
                "--write-save" => options.write_save = Some(PathBuf::from(value()?)),
                "--help" | "-h" => return Err(USAGE.into()),
                other => return Err(format!("unknown option {other}\n{USAGE}")),
            }
        }
        options.start = match (scenario, save) {
            (Some(token), None) => Start::Scenario {
                token,
                difficulty,
                character,
                seed,
            },
            (None, Some(path)) => Start::Save(path),
            _ => return Err(format!("give exactly one of --scenario or --save\n{USAGE}")),
        };
        if options.write_save.is_some() && !matches!(options.start, Start::Save(_)) {
            // A fresh city has no camera records to save; no camera owner runs headless.
            return Err("--write-save needs a run started with --save".into());
        }
        options.paths = match (fixtures, installer, scene, executable) {
            (true, None, None, None) => Paths::from_fixtures(scripts, language)?,
            (false, Some(installer), Some(scene), Some(executable)) => {
                Paths::derive(installer, scene, executable, scripts, language)?
            }
            _ => {
                return Err(format!(
                    "give --fixtures, or all of --assets, --scene-root and --executable\n{USAGE}"
                ));
            }
        };
        Ok(options)
    }
}

/// Why a run ended. Everything except `Ticks` stops before the tick budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    Ticks,
    /// The original result modal (48b490) opened; `context` is the winner slot.
    Result {
        context: u32,
    },
    Dialog {
        callback: u32,
    },
    Auction,
    /// The logic clock did not move for `frames` consecutive frames.
    Stalled {
        frames: u32,
    },
    Failure(String),
}

/// Checkpoint digest. Reading it never mutates the session.
pub fn digest(session: &crate::fresh_city::Session<'_>) -> String {
    use sha2::Digest;
    let world = &session.world;
    let clock = &world.clock;
    let mut hash = sha2::Sha256::new();
    for word in clock.timing.iter().chain(clock.calendar.iter()) {
        hash.update(word.to_le_bytes());
    }
    for row in &world.categories {
        hash.update(row);
    }
    for record in &world.buildings {
        hash.update(record);
    }
    for record in &world.ledgers.businesses {
        hash.update(record);
    }
    let residents = &world.ledgers.residents;
    hash.update(residents.active_count.to_le_bytes());
    for node in &residents.nodes {
        hash.update(node.as_bytes());
    }
    for account in &residents.accounts {
        hash.update(account.as_bytes());
    }
    for player in &world.ledgers.players {
        hash.update(player.cash.to_bits().to_le_bytes());
        hash.update(player.owned_categories);
    }
    hash.update(world.objective.reached);
    let state: String = hash.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let cash: Vec<String> = world
        .ledgers
        .players
        .iter()
        .map(|p| format!("{}", p.cash))
        .collect();
    let owners: String = world
        .categories
        .iter()
        .map(|row| char::from_digit(u32::from(row[5]), 36).unwrap_or('?'))
        .collect();
    format!(
        "{{\"tick\":{},\"year\":{},\"day\":{},\"hour\":{},\"cash\":[{}],\"residents\":{},\"buildings\":{},\"owners\":\"{owners}\",\"reached\":{:?},\"state\":\"{state}\"}}",
        clock.timing[0],
        1930 + clock.calendar[2] * 10 + clock.calendar[1],
        clock.calendar[4],
        clock.hour(),
        cash.join(","),
        residents.active_count,
        world.buildings.len(),
        world.objective.reached,
    )
}

/// Stand-in for the auction renderer thread (`auction_bridge::View`). It keeps
/// the scene/transition state machine so the session sees the values the live
/// renderer returns, and drains actor events without drawing them.
fn auction_stand_in(
    requests: std::sync::mpsc::Receiver<crate::auction_bridge::Request>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use crate::auction_bridge::Operation;
        let mut actors = None;
        let mut prepared = false;
        let mut transition = 0u32;
        let mut saved_transition = None;
        while let Ok(request) = requests.recv() {
            if let Some(events) = &actors {
                let _ = std::sync::mpsc::Receiver::try_iter(events).count();
            }
            let result = match request.operation {
                Operation::Prepare { actor_events, .. } if !prepared => {
                    prepared = true;
                    actors = Some(actor_events);
                    // The live UI passes 0 for the city view (3 for strategic).
                    Ok(0)
                }
                Operation::Prepare { .. } => {
                    Err("Auction renderer already owns an active scene".into())
                }
                Operation::Activate if !prepared => Err("Auction scene was not prepared".into()),
                Operation::Activate if saved_transition.is_some() => {
                    Err("Auction camera is already active".into())
                }
                Operation::Activate => {
                    let previous = std::mem::replace(&mut transition, 1);
                    saved_transition = Some(previous);
                    Ok(previous)
                }
                Operation::Restore => match saved_transition.take() {
                    Some(previous) if prepared => {
                        prepared = false;
                        actors = None;
                        transition = previous;
                        Ok(0)
                    }
                    _ => Err("Auction restore has no retained scene".into()),
                },
            };
            let _ = request.reply.send(result);
        }
    })
}

/// Borrowed asset tables shared by both start routes.
struct Tables<'a, 'data> {
    files: &'data Files,
    image: &'a openmt2001::pe::PeImage<'data>,
    board: &'a openmt2001::board::Board,
    file: &'a openmt2001::build_file::BuildFile<'data>,
    index: &'a openmt2001::archive::ArchiveIndex,
    paths: &'a Paths,
    names: &'a [Vec<u8>; 10],
}
impl<'a, 'data> Tables<'a, 'data> {
    fn inputs(
        &self,
        scenario: &'a str,
        difficulty: i32,
        selected_character: usize,
        profile_name: &'a [u8],
    ) -> Result<crate::fresh_city::Inputs<'a, 'data>, String> {
        let (image, index, archive) = (self.image, self.index, &self.files.archive[..]);
        Ok(crate::fresh_city::Inputs {
            tree_locators: Some(openmt2001::trees::Assets::load(image, index, archive)?.locators),
            image,
            board: self.board,
            file: self.file,
            people: openmt2001::people::PeopleModels::load(image, index, archive)?,
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                image, index, archive,
            )?,
            transport_models: openmt2001::traffic::models::Models::from_original(
                image, index, archive,
            )?,
            road_routes: std::fs::read(self.paths.installer.join("Standard/route_smalltable.bin"))
                .map_err(|e| format!("road route table: {e}"))?,
            scripts: &self.paths.scripts,
            parameters: &self.files.parameters,
            era_styles: crate::fresh_city::load_era_styles(
                &self.paths.installer.join("parameters"),
            )?,
            timetable: &self.files.timetable,
            scenario,
            difficulty,
            selected_character,
            profile_name,
            character_names: self.names,
        })
    }
}

struct Files {
    executable: Vec<u8>,
    archive: Vec<u8>,
    directory: Vec<u8>,
    build_file: Vec<u8>,
    parameters: String,
    timetable: String,
}
impl Files {
    fn load(paths: &Paths) -> Result<Self, String> {
        let read =
            |path: PathBuf| std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()));
        let text = |path: PathBuf| {
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        Ok(Self {
            executable: read(paths.executable.clone())?,
            archive: read(paths.scene.join("archive.bin"))?,
            directory: read(paths.scene.join("archive.DIR"))?,
            build_file: read(paths.scene.join("BuildEditorModels/BuildFile.dat"))?,
            parameters: text(paths.installer.join("parameters/parameter.prm"))?,
            timetable: text(paths.installer.join("parameters/timetable.prm"))?,
        })
    }
}

/// Run one headless city and write JSON lines to `out`.
pub fn run(options: &Options, out: &mut dyn Write) -> Result<End, String> {
    let files = Files::load(&options.paths)?;
    let paths = &options.paths;
    let image = openmt2001::pe::PeImage::parse(&files.executable)?;
    let style = crate::block_sidewalk_labels::load_style(&image, &paths.installer)?;
    let mut board = openmt2001::board::Board::from_v14(&image)?;
    style.apply_board(&mut board)?;
    let file = openmt2001::build_file::BuildFile::parse(&files.build_file)?;
    let index =
        openmt2001::archive::ArchiveIndex::parse(&files.directory, files.archive.len() as u64)
            .map_err(|e| e.to_string())?;
    let names = crate::fresh_city::character_names(&image, &paths.language)?;
    let compatibility = crate::compatibility::Mode::default();
    let (process, mut activity) =
        crate::live_host::process_runtime::startup(&files.executable, "")?;
    let custody =
        crate::process_dispatch::DispatchCustody::startup(activity.take_frontend_startup()?.pause)?;
    process
        .world_controller_reset4c2b80()
        .map_err(|e| e.to_string())?;
    let fresh = matches!(options.start, Start::Scenario { .. });
    let (dispatch, _timing) = custody.prepare_start(fresh)?;
    let tables = Tables {
        files: &files,
        image: &image,
        board: &board,
        file: &file,
        index: &index,
        paths,
        names: &names,
    };
    let mut host = crate::live_host::Host::new([0., 20., 0.]);
    let mut session;
    let mut saved_auction = None;
    let mut restored_presentation = Vec::new();
    match &options.start {
        Start::Scenario {
            token,
            difficulty,
            character,
            seed,
        } => {
            process.source_fresh_world().map_err(|e| e.to_string())?;
            // No profile: progress starts reset with the first level unlocked,
            // and nothing is written back.
            session = crate::fresh_city::Session::initialize_joined(
                tables.inputs(token, *difficulty, *character, b"Headless")?,
                crate::fresh_city::Policy {
                    seed: *seed,
                    auxiliary_prior_bytes: [0; 0x11c],
                },
                None,
                None,
                false,
                Some(&style),
                compatibility,
                None,
                Some(process.clone()),
                Some(dispatch),
            )?;
            // Fresh completion: the live worker zeroes the task accumulators
            // after synchronizing its frame timer.
            for task in &mut session.hooks.timers {
                task[12..16].copy_from_slice(&0f32.to_le_bytes());
            }
        }
        Start::Save(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let save = openmt2001::save::SaveFile::parse(&bytes)?;
            save.validate_compatibility(&image)?;
            process.source_load_unjoined().map_err(|e| e.to_string())?;
            let [id, _, _, difficulty] = save.level_metadata()?;
            let catalog = crate::campaign_store::load_catalog(&image, &paths.scripts)?;
            let level = catalog
                .levels
                .get(
                    catalog
                        .find(id)
                        .ok_or("saved level is absent from original catalog")?,
                )
                .ok_or("saved catalog level is missing")?;
            let scenario = std::str::from_utf8(level.directory())
                .map_err(|_| "saved scenario directory is not UTF-8")?;
            let scenario_path = crate::campaign_store::original_path(&paths.scripts, scenario)?
                .ok_or("saved scenario script directory is missing")?;
            let scenario = scenario_path
                .strip_prefix(&paths.scripts)
                .map_err(|_| "saved scenario directory is outside scripts")?
                .to_str()
                .ok_or("saved scenario directory is not UTF-8")?
                .to_owned();
            let difficulty =
                i32::try_from(difficulty).map_err(|_| "saved difficulty exceeds signed range")?;
            session = crate::fresh_city::Session::load_original_joined(
                tables.inputs(&scenario, difficulty, 0, b"Loaded original game")?,
                crate::fresh_city::Policy {
                    seed: 0,
                    auxiliary_prior_bytes: [0; 0x11c],
                },
                &bytes,
                None,
                None,
                Some(&mut restored_presentation),
                Some(&style),
                Some(process.clone()),
                Some(dispatch),
            )?;
            crate::process_dispatch::native_load_completed(&mut session)?;
            let metadata = session.restore_v2_extension_with_image(&save, &image)?;
            session.repair_imported_resident_models(compatibility, metadata.is_some())?;
            saved_auction = metadata.and_then(|metadata| metadata.auction);
        }
    }
    let assets = crate::fresh_city::CommandAssets {
        image: &image,
        board: &board,
        file: &file,
    };
    let (auction_renderer, auction_requests) = std::sync::mpsc::channel();
    let presenter = auction_stand_in(auction_requests);
    host.process_runtime = Some(process);
    host.auction_renderer = Some(auction_renderer);
    let setup = (|| {
        host.begin_presentation(&session)?;
        host.set_running(true);
        host.apply_restored_presentation(restored_presentation)?;
        host.activate_saved_auction(&mut session)?;
        if let Some(continuation) = saved_auction {
            continuation.apply(&mut session)?;
            host.restore_auction_widgets(continuation.host.as_ref(), &session)?;
        }
        Ok::<_, String>(())
    })();
    let end = match setup {
        Ok(()) => advance(&mut host, &mut session, &assets, options, out),
        Err(error) => Ok(End::Failure(error)),
    };
    let saved = match (&end, &options.write_save) {
        (Ok(_), Some(path)) => write_save(&mut session, &image, path),
        _ => Ok(()),
    };
    drop(host);
    let _ = presenter.join();
    let end = end?;
    saved?;
    Ok(end)
}

fn write_save(
    session: &mut crate::fresh_city::Session<'_>,
    image: &openmt2001::pe::PeImage<'_>,
    path: &Path,
) -> Result<(), String> {
    // No camera owner runs headless; keep the cameras the session loaded with.
    let cameras = session
        .saved_cameras
        .ok_or("the loaded save has no camera records to write back")?;
    let mode = session.saved_game_mode.unwrap_or(0);
    let bytes = session.save_v2(image, cameras, mode)?;
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn line(out: &mut dyn Write, text: &str) -> Result<(), String> {
    writeln!(out, "{text}").map_err(|e| e.to_string())
}

fn advance(
    host: &mut crate::live_host::Host,
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    options: &Options,
    out: &mut dyn Write,
) -> Result<End, String> {
    // Long auctions and paused modals stop the clock legitimately; this only
    // catches a city that is waiting for input the policy cannot give.
    const STALL_FRAMES: u32 = 30_000;
    let start = session.world.clock.timing[0];
    let target = start.saturating_add(options.ticks);
    line(out, &digest(session))?;
    let mut last_tick = start;
    let mut last_hour = session.world.clock.hour();
    let mut still = 0;
    let end = loop {
        let tick = session.world.clock.timing[0];
        if tick >= target {
            break End::Ticks;
        }
        host.frame(session, assets, options.frame_ms);
        if let Some(error) = &host.failure {
            break End::Failure(error.clone());
        }
        let tick = session.world.clock.timing[0];
        if let Some(end) = respond(host, session, assets, options, out)? {
            break end;
        }
        if tick == last_tick {
            still += 1;
            if still >= STALL_FRAMES {
                break End::Stalled { frames: still };
            }
            continue;
        }
        still = 0;
        let report = match options.every {
            Some(every) => (tick - start) / every != (last_tick - start) / every,
            None => session.world.clock.hour() != last_hour,
        };
        last_tick = tick;
        last_hour = session.world.clock.hour();
        if report {
            line(out, &digest(session))?;
        }
    };
    line(out, &digest(session))?;
    line(
        out,
        &format!(
            "{{\"end\":\"{}\",\"ticks\":{}}}",
            match &end {
                End::Ticks => "ticks".to_owned(),
                End::Result { context } => format!("result {context}"),
                End::Dialog { callback } => format!("dialog {callback:#x}"),
                End::Auction => "auction".to_owned(),
                End::Stalled { frames } => format!("stalled {frames} frames"),
                End::Failure(error) => format!("failure: {}", error.escape_debug()),
            },
            session.world.clock.timing[0] - start
        ),
    )?;
    Ok(end)
}

/// Apply the fixed human policy to what the host is presenting.
fn respond(
    host: &mut crate::live_host::Host,
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    options: &Options,
    out: &mut dyn Write,
) -> Result<Option<End>, String> {
    let status = host.status(session);
    if let Some(auction) = &status.auction
        && auction.visible
        && auction.controls.buttons_enabled & 8 != 0
    {
        match options.auctions {
            AuctionPolicy::Stop => return Ok(Some(End::Auction)),
            AuctionPolicy::Wait => {}
            AuctionPolicy::Pass => {
                line(
                    out,
                    &format!(
                        "{{\"tick\":{},\"auction\":\"pass\"}}",
                        session.world.clock.timing[0]
                    ),
                )?;
                if let Err(error) = host.auction_button(session, assets, 3) {
                    return Ok(Some(End::Failure(error)));
                }
            }
        }
    }
    for prompt in status.prompts {
        if let Some(dialog) = prompt.dialog {
            if dialog.callback == 0x48b490 {
                return Ok(Some(End::Result {
                    context: dialog.context,
                }));
            }
            let button = match options.dialogs {
                DialogPolicy::Stop => {
                    return Ok(Some(End::Dialog {
                        callback: dialog.callback,
                    }));
                }
                DialogPolicy::First => 0,
                DialogPolicy::Last => dialog.button_count.saturating_sub(1) as u32,
            };
            line(
                out,
                &format!(
                    "{{\"tick\":{},\"dialog\":\"{:#x}\",\"title\":{},\"body\":{},\"button\":{button}}}",
                    session.world.clock.timing[0], dialog.callback, dialog.title, dialog.body
                ),
            )?;
            let reply = crate::human_dialog::Reply {
                callback: dialog.callback,
                context: dialog.context,
                button,
            };
            if let Err(error) = host.human_dialog_reply(session, assets, reply) {
                return Ok(Some(End::Failure(error)));
            }
        } else if prompt.dismissal_id != 0 {
            session.dispatch_session_event(
                "InterfacePromptDismiss",
                prompt.dismissal_id as i32,
                assets,
                false,
            )?;
        }
    }
    Ok(None)
}

/// Entry point for `game headless …`. Returns the process exit status: 0 when
/// the tick budget or a result is reached, 1 on a failure or a policy stop,
/// 2 on bad arguments.
pub fn main(args: impl IntoIterator<Item = String>) -> i32 {
    let options = match Options::parse(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };
    // Session startup recurses deeply through the original scripts.
    let result = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(move || run(&options, &mut std::io::stdout().lock()))
        .map_err(|e| e.to_string())
        .and_then(|worker| {
            worker
                .join()
                .unwrap_or_else(|_| Err("headless runner panicked".into()))
        });
    match result {
        Ok(End::Ticks | End::Result { .. }) => 0,
        Ok(_) => 1,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_install_paths_and_derives_scripts_and_language() {
        let options = parse(&[
            "--scenario",
            "SCENARIO3",
            "--assets",
            "inst",
            "--scene-root",
            "max",
            "--executable",
            "payload/Essential/mc.exe",
            "--ticks",
            "100",
            "--every",
            "25",
            "--seed",
            "0x10",
            "--dialogs",
            "stop",
        ])
        .unwrap();
        assert_eq!(
            options.start,
            Start::Scenario {
                token: "SCENARIO3".into(),
                difficulty: 1,
                character: 4,
                seed: 16
            }
        );
        assert_eq!(
            options.paths.scripts,
            Path::new("payload/Essential/scripts")
        );
        assert_eq!(options.paths.language, Path::new("payload/US/language/usa"));
        assert_eq!((options.ticks, options.every), (100, Some(25)));
        assert_eq!(options.dialogs, DialogPolicy::Stop);
        assert_eq!(options.auctions, AuctionPolicy::Pass);
    }

    #[test]
    fn rejects_ambiguous_starts_and_partial_paths() {
        let paths = ["--assets", "a", "--scene-root", "b", "--executable", "c/d"];
        let with = |extra: &[&str]| {
            let mut args = extra.to_vec();
            args.extend(paths);
            parse(&args)
        };
        assert!(with(&[]).is_err());
        assert!(with(&["--scenario", "x", "--save", "y"]).is_err());
        assert!(with(&["--scenario", "x", "--every", "0"]).is_err());
        assert!(with(&["--scenario", "x", "--difficulty", "3"]).is_err());
        assert!(parse(&["--scenario", "x", "--assets", "a"]).is_err());
        assert!(with(&["--save", "y", "--frame-ms", "0"]).is_err());
        assert!(with(&["--scenario", "x", "--write-save", "z"]).is_err());
        assert!(with(&["--save", "y", "--write-save", "z"]).is_ok());
    }

    fn fixture_options(start: Start, ticks: u32) -> Options {
        Options {
            start,
            paths: Paths::from_fixtures(None, None).unwrap(),
            ticks,
            every: Some(25),
            frame_ms: 40,
            dialogs: DialogPolicy::Last,
            auctions: AuctionPolicy::Pass,
            write_save: None,
        }
    }

    fn twice(options: &Options) -> String {
        let run_once = || {
            let options = options.clone();
            std::thread::Builder::new()
                .stack_size(32 * 1024 * 1024)
                .spawn(move || {
                    let mut out = Vec::new();
                    let end = run(&options, &mut out).unwrap();
                    (end, String::from_utf8(out).unwrap())
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let (end, first) = run_once();
        let (_, second) = run_once();
        assert_eq!(end, End::Ticks, "{first}");
        assert_eq!(first, second, "headless runs are not deterministic");
        first
    }

    #[test]
    #[ignore = "requires original assets"]
    fn fresh_scenario_advances_deterministically() {
        let report = twice(&fixture_options(
            Start::Scenario {
                token: "SCENARIO3".into(),
                difficulty: 1,
                character: 4,
                seed: 0,
            },
            500,
        ));
        assert!(
            report.ends_with("{\"end\":\"ticks\",\"ticks\":500}\n"),
            "{report}"
        );
        // Initial digest, one every 25 ticks, and the final digest.
        assert!(report.lines().count() >= 22, "{report}");
    }

    #[test]
    #[ignore = "requires original assets and the in-progress original save"]
    fn original_save_advances_deterministically() {
        let report = twice(&fixture_options(
            Start::Save(openmt2001::fixtures::path("SAVE_ONE")),
            500,
        ));
        assert!(
            report.ends_with("{\"end\":\"ticks\",\"ticks\":500}\n"),
            "{report}"
        );
    }
}
