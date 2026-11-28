//! Host controls for the retained session. A failed simulation boundary latches
//! a stop: resuming must never replay a partially completed original tick.
pub mod auction_audio;
#[path = "live_host/camera_bridge.rs"]
pub mod camera_bridge;
#[path = "live_host/camera_session_service.rs"]
mod camera_session_service;
#[path = "live_host/process_runtime.rs"]
pub mod process_runtime;
use crate::{
    fresh_city::{self, CommandAssets, Session},
    prompt,
};
use openmt2001::{hourly, level_script, lua4::Value, scheduler};

fn property_refresh_broadcast(packet: &openmt2001::player_messages::Packet) -> bool {
    u32::from_le_bytes(packet[4..8].try_into().unwrap()) == 0x26
}

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub autosave: Option<crate::save_menu::AutosaveStatus>,
    pub time_display: crate::time_panel::DisplayState,
    pub cash_arrow: crate::cash_arrow::State,
    pub window_texture: Option<openmt2001::window_texture::Publication>,
    pub pulse_lights: openmt2001::pulse_lights::PulseLights,
    pub demolition_points: Vec<openmt2001::demolition_effect::Point>,
    pub stats: Option<crate::stats_panel::Snapshot>,
    pub comparison: Option<crate::comparison::Snapshot>,
    pub market_share: Option<crate::market_share_panel::Snapshot>,
    pub result_request: Option<ResultRequest>,
    pub result_review: Option<ResultReview>,
    /// Original normal main57e760 and player-statistics57e778 panel visibility.
    pub player_panels: [bool; 2],
    /// Current original runtime+4 ledgers for the 412810 player report.
    pub financial: Option<crate::player_info::Snapshot>,
    /// Native current player's one-based ownership byte, not character ID.
    pub business_owner: Option<u8>,
    /// Original current-player +10 activity byte used by construction input.
    pub current_player_active: bool,
    /// Current original 4112a0 resident counts and ratios for the City tab.
    pub city_people_rows: Option<Vec<crate::city_people::Row>>,
    pub tutorial_controls: TutorialControls,
    pub source_picking: Option<process_runtime::PickingProjection>,
    pub tutorial_arrows: Vec<[i32; 2]>,
    /// Diagnostic view of the active zero-button tutorial record.
    pub tutorial_message: Option<prompt::Request>,
    /// Close operations, indexed between the requests in `prompts`.
    pub prompt_closes: Vec<usize>,
    pub tutorial_enabled_controls: Vec<u32>,
    pub tutorial_interaction: crate::tutorial_two_interaction::State,
    /// Ordered native Camera callbacks from the retained tutorial VM.
    pub tutorial_camera_events: Vec<TutorialCameraAction>,
    /// Effective native builder+788 gate; tutorial byte1 activates restriction,
    /// while -1 permits either building type.
    pub tutorial_build_type: i32,
    /// Original builder+78c use-list index gate; 999 permits every row.
    pub tutorial_build_use: i32,
    pub tutorial_completed: bool,
    /// Authoritative active-level tutorial byte for original disabled menus.
    pub tutorial_session: bool,
    /// Normal static EventList571378 shown by original Interface.CreateEventList.
    pub event_list_visible: bool,
    ///43bce0 clears the one-frame window's opening state at its first UI callback.
    pub event_list_settled: bool,
    pub card: Option<crate::card_panel::Panel>,
    pub card_entry: crate::card_panel::Entry,
    pub card_visual: crate::card_panel::EntryVisual,
    /// Original822b0c/822b0e/822af4, retained independently of clock arithmetic.
    pub day_night_flags: [u8; 3],
    pub notices: Vec<crate::notifications::Notice>,
    pub auction: Option<Auction>,
    pub running: bool,
    pub speed: u32,
    pub network_enabled: bool,
    pub script_paused: bool,
    pub ticks: u32,
    pub interface_ticks: u64,
    /// Delivered updates of the normal modal UI group, after the central gate.
    pub prompt_updates: u64,
    pub year: u32,
    pub hour: u32,
    /// Original822d48 fraction within the625-tick hour.
    pub hour_fraction: u32,
    pub failure: Option<String>,
    /// Recoverable AI script diagnostics since this host was created.
    pub ai_warning_count: u64,
    /// Most recent warning, formatted like the retained stderr diagnostic.
    pub last_ai_warning: Option<String>,
    pub pending_presentation: Vec<u32>,
    pub prompts: Vec<prompt::Request>,
    pub auction_seconds: Option<i32>,
    pub auction_notice: Option<crate::auction_notice::Notice>,
    pub auction_locked_categories: Vec<usize>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TutorialControls {
    pub all_disabled: bool,
    pub picking_disabled: bool,
    pub camera_disabled: bool,
}
/// Terminal diagnostics replace the intrusive city overlay. Emit only a state,
/// speed or hour transition; clock polling must not flood the console.
pub(crate) fn report_status(status: &Status, previous: Option<&Status>) {
    if previous.is_none() {
        eprintln!(
            "[controls] F1/P pauses/resumes; Options opens the game menu. Diagnostics stay in this terminal."
        );
    }
    if !status.pending_presentation.is_empty()
        && previous
            .is_none_or(|previous| previous.pending_presentation != status.pending_presentation)
    {
        eprintln!(
            "[presentation] Pending callbacks: {:x?}",
            status.pending_presentation
        );
    }
    let key = |s: &Status| {
        (
            s.running,
            s.script_paused,
            s.failure.is_some(),
            s.speed,
            s.year,
            s.hour,
            s.stats.is_some(),
            s.tutorial_completed,
        )
    };
    if previous.is_some_and(|previous| key(previous) == key(status)) {
        return;
    }
    let state = if status.failure.is_some() {
        "STOPPED"
    } else if status.tutorial_completed {
        "COMPLETED"
    } else if status.script_paused {
        "PROMPT PAUSED"
    } else if !status.running {
        "PAUSED"
    } else {
        "RUNNING"
    };
    eprintln!(
        "[session] {state}: year {}, {:02}:{:02}, tick {}, speed {}x{}",
        status.year,
        status.hour,
        status.hour_fraction * 60 / 625,
        status.ticks,
        status.speed,
        if status.stats.is_some() {
            ", statistics open"
        } else {
            ""
        }
    );
}
#[derive(Default)]
struct AiWarnings {
    count: u64,
    last_message: Option<String>,
    #[cfg(test)]
    expected_count: u64,
}

#[cfg(test)]
impl Drop for AiWarnings {
    fn drop(&mut self) {
        // Preserve an existing test failure instead of panicking during unwind.
        if !std::thread::panicking() {
            assert_eq!(
                self.count, self.expected_count,
                "unexpected AI warning count; last warning: {:?}",
                self.last_message
            );
        }
    }
}

/// Native 4e6960 kind 1 returns after acknowledgement; kind 0 terminates.
/// The frontend's game prompts need resource IDs and gameplay dismissal events,
/// so native debug dialogs are recorded in the terminal instead.
fn ai_diagnostic(
    diagnostic: openmt2001::ai_diagnostic::Diagnostic,
    warnings: &mut AiWarnings,
) -> Result<u32, String> {
    use openmt2001::ai_diagnostic::Kind;
    match diagnostic.kind {
        Kind::RecoverableWarning => {
            warnings.count = warnings.count.saturating_add(1);
            warnings.last_message = Some(diagnostic.to_string());
            eprintln!("[AI warning] {diagnostic}");
            Ok(0)
        }
        Kind::Fatal => Err(format!("AI fatal diagnostic: {diagnostic}")),
    }
}

fn ai_callback(
    call: openmt2001::ai_runtime::Call,
    warnings: &mut AiWarnings,
) -> Result<u32, String> {
    match call {
        openmt2001::ai_runtime::Call::Diagnostic(diagnostic) => ai_diagnostic(diagnostic, warnings),
        other => Err(format!("Live session requires Ai({other:?})")),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TutorialCameraAction {
    Mode { mode: u32, target: u32 },
    CityView,
    PlanView,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultRequest {
    pub sequence: openmt2001::outcome_notice::Sequence,
    pub characters: (u32, u32),
    pub music_address: u32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Auction {
    pub controls: crate::auction_controls::State,
    pub visible: bool,
    /// 414160 disables the textureless title-right child in network mode.
    pub hidden_bid_toggle_enabled: bool,
    /// Authoritative details858 voice/AI gate published with the live panel.
    pub bid_voice_gate: bool,
    pub info: Option<crate::auction_panel::Info>,
    pub participants: Vec<crate::auction_labels::Participant>,
}

impl Auction {
    fn refresh_participants(&mut self, session: &Session<'_>) -> Result<(), String> {
        self.participants = crate::auction_labels::participants(session)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsAction {
    Open,
    Close,
    Cycle(bool),
    Toggle(usize),
}

/// The stopped graph owns the session until its Close action reaches a menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultReview {
    Statistics,
    Menu(ResultMenu),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultMenu {
    Scenarios,
    Main,
}
impl ResultMenu {
    fn for_level(level: &openmt2001::level_catalog::Level) -> Self {
        //49e722: the single-player flag without the sandbox flag.
        if level.single_player() && level.bytes[2] == 0 {
            Self::Scenarios
        } else {
            Self::Main
        }
    }
}

/// Caller chooses the actual install path. CameraOnly does not capture C
/// tables/inputs or fabricate F globals for an inactive ordinary UI graph.
pub enum CameraSupplierPolicy {
    CameraOnly,
    Graph {
        upper: crate::navigator::upper::Producer,
    },
}
enum CameraSuppliers {
    CameraOnly,
    Graph {
        tables: std::sync::Arc<crate::locator::strategic::Tables>,
        upper: crate::navigator::upper::Producer,
    },
}

pub struct Host {
    ai_warnings: AiWarnings,
    //Missing process source is explicit for legacy CPU Hosts. Production binds
    //the SAME Assets-born handle before commands/startup are consumed.
    pub process_runtime: Option<process_runtime::Handle>,
    /// None retains existing callers. Installation is an explicit ack barrier.
    camera_bridge: Option<camera_bridge::Worker>,
    /// Explicit legacy UI snapshot source, mutually exclusive with the native
    /// camera bridge. Absence retains the pre-connection supported routes.
    ui_camera_source: Option<crate::save_menu::camera_source::Reader>,
    /// Authority supplied only by Inputs::session_worker selected profile.
    pub(crate) autosave_profile_root: Option<std::path::PathBuf>,
    pub autosave_status: Option<crate::save_menu::AutosaveStatus>,
    camera_suppliers: Option<CameraSuppliers>,
    pub time_runtime: std::sync::Arc<std::sync::Mutex<crate::time_panel::Runtime>>,
    presentation_started: bool,
    /// Native8b22dc denotes an active session, independently of player pause.
    game_active: bool,
    stats: Option<crate::stats_panel::State>,
    comparison: Option<crate::comparison::Runtime>,
    market_share: crate::market_share_panel::State,
    cash_arrow: crate::cash_arrow::State,
    player_panels: [bool; 2],
    card: Option<crate::card_panel::Panel>,
    card_visible: bool,
    day_night_flags: [u8; 3],
    notices: crate::notifications::Queue,
    pub auction_renderer: Option<std::sync::mpsc::Sender<crate::auction_bridge::Request>>,
    auction_actor_events: Option<std::sync::mpsc::Sender<crate::auction_bridge::ActorEvent>>,
    pub auction_audio: Option<std::sync::mpsc::Sender<auction_audio::Request>>,
    pub transport_audio: Option<std::sync::mpsc::Sender<crate::transport_audio::Request>>,
    pub era_audio: Option<std::sync::mpsc::Sender<crate::era_audio::Request>>,
    pub gameplay_sound: Option<std::sync::mpsc::Sender<crate::gameplay_sound::Request>>,
    auction: Option<Auction>,
    pub running: bool,
    player_paused: bool,
    pub failure: Option<String>,
    /// Saved on the worker, rather than inferred from an asynchronous UI poll.
    options_resume: Option<bool>,
    pub policy: fresh_city::tick::Policy,
    commands_seen: usize,
    tutorial_started: bool,
    tutorial_controls: TutorialControls,
    tutorial_arrows: Vec<[i32; 2]>,
    pending_prompt_closes: Vec<usize>,
    script_closes_seen: usize,
    tutorial_enabled_controls: std::collections::BTreeSet<u32>,
    tutorial_interaction: crate::tutorial_two_interaction::State,
    tutorial_camera_events: Vec<TutorialCameraAction>,
    tutorial_build_type: i32,
    tutorial_build_use: i32,
    tutorial_end_requested: Option<bool>,
    tutorial_completed: bool,
    event_list_visible: bool,
    event_list_opening: bool,
    tutorial_applied: bool,
    pending_presentation: std::collections::BTreeSet<u32>,
    pending_prompts: Vec<prompt::Request>,
    interface_ticks: u64,
    prompt_updates: u64,
    auction_seconds: Option<i32>,
    result_request: Option<ResultRequest>,
    result_applied: bool,
    result_completed: bool,
    outcome_callback_seen: bool,
    outcome_context: Option<u32>,
    result_context: Option<u32>,
}
impl Host {
    /// Declare the exact number of deliberately injected warnings in a test.
    /// All other session tests reject warnings when their host is dropped.
    #[cfg(test)]
    pub(crate) fn expect_ai_warnings(&mut self, count: u64) {
        self.ai_warnings.expected_count = count;
    }
    ///4ba17e (fresh) /4bbd6b (loaded) shows the persistent normal Time
    /// instance. Re-showing an already visible widget has no activation call.
    pub fn begin_presentation(&mut self, session: &Session<'_>) -> Result<(), String> {
        if self.presentation_started {
            return Ok(());
        }
        let cash = current_cash(session)?;
        let mut runtime = self
            .time_runtime
            .lock()
            .map_err(|_| "Time runtime poisoned")?;
        runtime.bind_panels();
        runtime.show(cash, 0)?;
        drop(runtime);
        self.presentation_started = true;
        self.game_active = true;
        self.restore_player_pause(&session.world.pause);
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn cash_arrow_state(&self) -> crate::cash_arrow::State {
        self.cash_arrow
    }
    /// Consume original load-stage effects once, after the actual UI channels
    /// exist. These calls already changed the loaded world during restoration.
    pub fn apply_restored_presentation(
        &mut self,
        postings: Vec<openmt2001::economy::AccountPosting>,
    ) -> Result<(), String> {
        for posting in postings {
            // The loader resolved locality at the original posting boundary.
            // Replay that decision after the UI channels have been attached.
            account_ui_post(posting, u8::try_from(posting.player).ok(), |amount| {
                ledger_ui_post(&mut self.cash_arrow, &self.gameplay_sound, true, amount, 0.)
            })?;
        }
        Ok(())
    }
    /// Original424010 completes the independent placement RNG stores, then88.
    /// The visible builder enters state3 only after this callback returns.
    pub fn complete_build_drag(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<[i32; 3], String> {
        if self.failure.is_some() {
            return Err("Stopped session cannot begin build placement".into());
        }
        let styles = session
            .era_styles
            .get(session.world.clock.calendar[2] as usize)
            .ok_or("build era outside original parameter banks")?;
        let choices = crate::build_panel::placement_choices(
            &mut session.build_random_state,
            styles.qualities.each_ref().map(Vec::len),
        )?;
        self.dispatch_tutorial_callback(session, assets, 88)?;
        Ok(choices)
    }
    /// 465490 re-enters 464aa0 after a mode-2 auction record is restored.
    /// The renderer exists only after the loaded inspection reaches the UI,
    /// so the worker performs this ordered presentation step at that seam.
    pub fn activate_saved_auction(&mut self, session: &mut Session<'_>) -> Result<(), String> {
        if session.saved_game_mode != Some(2)
            || session.world.city.auction.controller.reference == 0
            || session.world.city.auction.controller.record[0x7d] == 0
        {
            return Ok(());
        }
        let prior = u32::from_le_bytes(
            session.world.city.auction.controller.record[0x150..0x154]
                .try_into()
                .unwrap(),
        );
        if prior != 0 && prior != 3 {
            return Err(format!(
                "saved auction prior interface mode {prior} has no V2 presenter"
            ));
        }
        let characters = session
            .roster
            .manager
            .players
            .iter()
            .map(|player| u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap()))
            .collect::<Vec<_>>();
        let (actor_sender, actor_events) = std::sync::mpsc::channel();
        crate::auction_bridge::request(
            &self.auction_renderer,
            crate::auction_bridge::Operation::Prepare {
                characters,
                actor_events,
            },
        )?;
        self.auction_actor_events = Some(actor_sender);
        self.policy.ai_event_gate = false;
        self.auction = Some(Auction::default());
        let mut omitted = std::collections::BTreeSet::new();
        let tutorial_state = session.tutorial_state;
        let renderer = &self.auction_renderer;
        let actor_events = &self.auction_actor_events;
        let bid_gate = std::cell::Cell::new(false);
        let gameplay_sound = &self.gameplay_sound;
        let auction = &mut self.auction;
        let cash = current_cash(session)?;
        let time_runtime = &self.time_runtime;
        openmt2001::ai_messages::offers::auction::activation::activate(
            &mut session.world.city.auction.controller,
            &mut session.world.city.auction.presentation,
            &mut session.roster.manager.players,
            &session.world.city.categories,
            false,
            |call, _, _| {
                use openmt2001::ai_messages::offers::auction::activation::Call;
                match call {
                    Call::Ambient => crate::gameplay_sound::request(
                        gameplay_sound,
                        crate::gameplay_sound::Operation::AuctionAmbientCreate,
                    ),
                    Call::Camera => crate::auction_bridge::request(
                        renderer,
                        crate::auction_bridge::Operation::Activate,
                    ),
                    Call::Action(action) => {
                        if action
                            == openmt2001::ai_messages::offers::auction::Call::ShowPanel(0x565ba8)
                        {
                            time_runtime
                                .lock()
                                .map_err(|_| "Time runtime poisoned")?
                                .show(cash, 1)?;
                        } else {
                            auction_action(
                                action,
                                auction,
                                &mut omitted,
                                tutorial_state,
                                actor_events,
                                &bid_gate,
                            )?;
                        }
                        Ok(0)
                    }
                    Call::RefreshPanel => {
                        auction
                            .as_mut()
                            .ok_or("Auction labels have no prepared scene")?
                            .participants
                            .clear();
                        Ok(0)
                    }
                    Call::NormalSpeed => Ok(0),
                }
            },
        )?;
        self.auction
            .as_mut()
            .unwrap()
            .refresh_participants(session)?;
        Ok(())
    }

    pub fn new(camera: [f32; 3]) -> Self {
        Self {
            ai_warnings: AiWarnings::default(),
            process_runtime: None,
            camera_bridge: None,
            ui_camera_source: None,
            autosave_profile_root: None,
            autosave_status: None,
            camera_suppliers: None,
            time_runtime: Default::default(),
            presentation_started: false,
            game_active: false,
            stats: None,
            comparison: None,
            market_share: Default::default(),
            cash_arrow: crate::cash_arrow::State::default(),
            player_panels: [true; 2],
            card: None,
            card_visible: false,
            day_night_flags: [0; 3],
            notices: Default::default(),
            auction_renderer: None,
            auction_actor_events: None,
            auction_audio: None,
            transport_audio: None,
            era_audio: None,
            gameplay_sound: None,
            auction: None,
            running: false,
            player_paused: false,
            failure: None,
            options_resume: None,
            commands_seen: 0,
            tutorial_started: false,
            tutorial_controls: TutorialControls::default(),
            tutorial_arrows: Vec::new(),
            pending_prompt_closes: Vec::new(),
            script_closes_seen: 0,
            tutorial_enabled_controls: Default::default(),
            tutorial_interaction: Default::default(),
            tutorial_camera_events: Vec::new(),
            tutorial_build_type: -1,
            tutorial_build_use: 999,
            tutorial_end_requested: None,
            tutorial_completed: false,
            event_list_visible: false,
            event_list_opening: false,
            tutorial_applied: false,
            pending_presentation: Default::default(),
            pending_prompts: Vec::new(),
            interface_ticks: 0,
            prompt_updates: 0,
            auction_seconds: None,
            result_request: None,
            result_applied: false,
            result_completed: false,
            outcome_callback_seen: false,
            outcome_context: None,
            result_context: None,
            policy: fresh_city::tick::Policy {
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
                    rollover_events_enabled: true,
                    autosave_disabled: false,
                },
                hourly: hourly::Controls {
                    suppressed: false,
                    world_mode: 1,
                    before_six_sound: 0,
                    network_enabled: false,
                    network_override: false,
                },
                camera,
                distance_disabled: false,
                notifications: true,
                // Local portable texture1 corresponds to the first original
                //billboard-repeat registration687c78 selected by4b0c39.
                effect_model: 1,
                market_share_prior: [0.; 6],
                player_message_prior: [0; 48],
                ai_event_gate: false,
            },
        }
    }
    pub fn camera_bridge_installed(&self) -> bool {
        self.camera_bridge
            .as_ref()
            .is_some_and(camera_bridge::Worker::installed)
    }
    pub fn camera_retired(&self) -> bool {
        self.camera_bridge
            .as_ref()
            .is_some_and(camera_bridge::Worker::retired)
    }
    /// Unlike fail(), this records no simulation fault and sends no new work.
    /// The retired generation cannot resume, including through Options/Run.
    fn stop_retired_camera(&mut self) -> bool {
        if !self.camera_retired() {
            return false;
        }
        self.running = false;
        true
    }
    fn camera_error(&mut self, error: camera_bridge::RequestError) {
        match error {
            camera_bridge::RequestError::Retired => {
                self.stop_retired_camera();
            }
            camera_bridge::RequestError::Failed(error) => self.fail(error),
        }
    }
    fn camera_admission(&self, session: &Session<'_>) -> camera_bridge::Admission {
        camera_bridge::Admission {
            strategic: None,
            upper: None,
            tutorial_session: session.tutorial_state == 1,
            all_disabled: self.tutorial_controls.all_disabled,
            camera_disabled: self.tutorial_controls.camera_disabled,
            enabled_controls: self.tutorial_enabled_controls.iter().copied().collect(),
        }
    }
    /// UI must already be servicing the Inbox before this call. Resolve initial
    /// script camera effects at their real pre-pair boundary before installing;
    /// dropping queued legacy events here would silently change startup history.
    pub fn install_camera_bridge(
        &mut self,
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        supplier_policy: CameraSupplierPolicy,
        mut worker: camera_bridge::Worker,
        capture: openmt2001::camera::bank::Capture,
        pavement: crate::selection::pavement_resource::LocalPavementHandle,
    ) -> Result<(), camera_bridge::RequestError> {
        if self.failure.is_some() || self.camera_bridge.is_some() || self.ui_camera_source.is_some()
        {
            return Err("Camera bridge cannot replace an installed/stopped host".into());
        }
        if !self.tutorial_camera_events.is_empty() {
            return Err("Install camera after resolving prior tutorial camera events".into());
        }
        // Retain a failed endpoint too: a partially completed install cannot
        // silently fall back to legacy camera progression or be retried.
        self.camera_suppliers = Some(match supplier_policy {
            CameraSupplierPolicy::CameraOnly => CameraSuppliers::CameraOnly,
            CameraSupplierPolicy::Graph { upper } => CameraSuppliers::Graph {
                tables: std::sync::Arc::new(crate::locator::strategic::Tables::capture(image)?),
                upper,
            },
        });
        let admission = self.supplied_camera_admission(session, true)?;
        let operation = camera_bridge::Operation::Install {
            capture,
            admission,
            property: Some(std::sync::Arc::new(camera_bridge::PropertySource::capture(
                session, pavement,
            ))),
        };
        let result = worker.request_with_handler(operation, |call| {
            camera_session_service::handle(session, call)
        });
        self.camera_bridge = Some(worker);
        match result.and_then(|mut observation| {
            Self::check_camera_finalization(&observation)?;
            if let Some(writeback) = observation.strategic_writeback.take() {
                writeback.commit(session)?;
            }
            self.observe_camera(&observation).map_err(Into::into)
        }) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.camera_error(error.clone());
                Err(error)
            }
        }
    }
    /// Bind only the actual Session topology epoch. Missing initial publication
    /// is retained as an explicit unavailable source, never legacy fallback.
    pub(crate) fn install_ui_camera_source(
        &mut self,
        session: &Session<'_>,
        source: crate::save_menu::camera_source::Reader,
    ) -> Result<(), String> {
        if self.failure.is_some() || self.camera_bridge.is_some() || self.ui_camera_source.is_some()
        {
            return Err("UI camera source cannot replace an installed/stopped host".into());
        }
        source.check_epoch(session.world.topology.epoch())?;
        self.ui_camera_source = Some(source);
        Ok(())
    }
    pub(crate) fn has_ui_camera_source(&self) -> bool {
        self.ui_camera_source.is_some()
    }
    /// Producer-dependent ordinary SaveSelected consumer. The same atomic
    /// pair/mode source is used by autosave below; no cached request pose is
    /// overlaid and Options stays held throughout encoding/publication.
    pub(crate) fn save_selected_from_ui_snapshot(
        &self,
        session: &mut Session<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        root: &std::path::Path,
        target: &crate::save_menu::Target,
        game_mode: u32,
    ) -> Result<std::path::PathBuf, String> {
        if !self.options_save_allowed(&session.world.pause, &session.hooks.timers) {
            return Err("Save requires the active Options hold".into());
        }
        let source = self
            .ui_camera_source
            .as_ref()
            .ok_or("UI camera source is not installed")?;
        let snapshot = source.read(session.world.topology.epoch())?;
        let bytes = session.save_v2_with_options_view(
            image,
            snapshot.capture().into_records(),
            game_mode,
            self,
            snapshot.interface_mode(),
        )?;
        crate::save_menu::write_target_from_snapshot(root, target, &bytes, source, &snapshot)
    }
    /// Legacy Status clients supply their render sample. An installed owner
    /// already supplies current vectors at the acknowledged boundaries; a
    /// sampled Status argument must never replace those world inputs.
    pub fn observe_presentation_camera(&mut self, eye: [f32; 3]) {
        if self.camera_bridge.is_none() {
            self.policy.camera = eye;
        }
    }
    fn check_camera_finalization(observation: &camera_bridge::Observation) -> Result<(), String> {
        match &observation.finalization_fault {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    fn observe_camera(&mut self, observation: &camera_bridge::Observation) -> Result<(), String> {
        Self::check_camera_finalization(observation)?;
        let eye = observation.current.eye.map(f32::from_bits);
        if eye.iter().any(|v| !v.is_finite()) {
            return Err(
                "Acknowledged current camera eye is unsupported by live world consumers".into(),
            );
        }
        self.policy.camera = eye;
        Ok(())
    }
    fn request_camera(
        &mut self,
        operation: camera_bridge::Operation,
    ) -> Result<Option<camera_bridge::Observation>, camera_bridge::RequestError> {
        let observation = self.exchange_camera(operation)?;
        if let Some(observation) = &observation {
            self.observe_camera(observation)?;
        }
        Ok(observation)
    }
    // Exchange without fallible world projection. Callers must commit the
    // completed boundary's scheduler effects before projection can fail.
    fn exchange_camera(
        &mut self,
        operation: camera_bridge::Operation,
    ) -> Result<Option<camera_bridge::Observation>, camera_bridge::RequestError> {
        if let Some(error) = &self.failure {
            return Err(error.clone().into());
        }
        let Some(worker) = &mut self.camera_bridge else {
            return Ok(None);
        };
        worker.request(operation).map(Some)
    }
    /// Actual Session remains on this Host stack throughout the UI operation.
    /// Both Worker and Session are exclusively borrowed, so a callback cannot
    /// re-enter this request or send dependent work to the blocked Host.
    pub(crate) fn exchange_camera_with_session(
        &mut self,
        session: &mut Session<'_>,
        operation: camera_bridge::Operation,
    ) -> Result<Option<camera_bridge::Observation>, camera_bridge::RequestError> {
        if let Some(error) = &self.failure {
            return Err(error.clone().into());
        }
        let Some(worker) = &mut self.camera_bridge else {
            return Ok(None);
        };
        let process = self.process_runtime.as_ref();
        worker
            .request_with_handler(operation, |call| {
                camera_session_service::handle_with_process(session, process, call)
            })
            .map(Some)
    }
    fn tutorial_camera_action(&mut self, session: &Session<'_>, action: TutorialCameraAction) {
        if self.camera_bridge.is_none() {
            self.tutorial_camera_events.push(action);
            return;
        }
        let operation = match action {
            TutorialCameraAction::Mode { mode, target } => {
                camera_bridge::NativeOperation::TutorialMode { mode, target }
            }
            TutorialCameraAction::CityView => camera_bridge::NativeOperation::TutorialCityView,
            TutorialCameraAction::PlanView => camera_bridge::NativeOperation::TutorialPlanView,
        };
        let result = (|| {
            let admission = self.supplied_camera_admission(session, false)?;
            self.request_camera(camera_bridge::Operation::Apply {
                operation,
                admission,
            })
        })();
        if let Err(error) = result {
            self.camera_error(error);
        }
    }
    fn supplied_camera_admission(
        &self,
        session: &Session<'_>,
        protected: bool,
    ) -> Result<camera_bridge::Admission, String> {
        let mut admission = self.camera_admission(session);
        match self
            .camera_suppliers
            .as_ref()
            .ok_or("Camera supplier policy not installed")?
        {
            CameraSuppliers::CameraOnly => {}
            CameraSuppliers::Graph { tables, upper } => {
                if protected {
                    admission.strategic = Some(std::sync::Arc::new(
                        crate::locator::strategic::Inputs::capture(session, tables.clone())?,
                    ));
                }
                admission.upper = Some(upper.capture(session)?);
            }
        }
        Ok(admission)
    }
    ///Bind at the actual presentation policy writer; count belongs to World topology.
    pub fn upper_presentation_mode(&mut self, mode8b2318: u32) -> Result<(), String> {
        let Some(CameraSuppliers::Graph { upper, .. }) = &mut self.camera_suppliers else {
            return Err("F globals before actual graph producer install".into());
        };
        upper.mode8b2318 = mode8b2318;
        Ok(())
    }
    fn camera_central_tail(
        &mut self,
        session: &mut Session<'_>,
    ) -> Result<(), camera_bridge::RequestError> {
        if self.camera_bridge.is_none() {
            return Ok(());
        }
        let admission = self.supplied_camera_admission(session, true)?;
        let central = if let Some(process) = &self.process_runtime {
            let identity = self
                .camera_bridge
                .as_ref()
                .expect("checked camera bridge")
                .central_identity()?;
            Some(process.begin_central(identity).map_err(|e| e.to_string())?)
        } else {
            None
        };
        let exchanged = self.exchange_camera_with_session(
            session,
            camera_bridge::Operation::CentralTail { admission },
        );
        let mut observation = match exchanged {
            Ok(Some(value)) => value,
            Ok(None) => {
                if let (Some(process), Some(token)) = (&self.process_runtime, central) {
                    let _ = process.fail_central(token);
                }
                return Err("Camera bridge disappeared during admitted central".into());
            }
            Err(error) => {
                if let (Some(process), Some(token)) = (&self.process_runtime, central) {
                    let _ = process.fail_central(token);
                }
                return Err(error);
            }
        };
        {
            //All already-committed camera effects precede fallible C/world projection.
            Self::apply_camera_effects(&mut session.hooks.timers, &observation);
            //A completed ack's timers survive even a later process-token/lock
            //failure. Only this operation can satisfy its quit tail obligation.
            if let (Some(process), Some(token)) = (&self.process_runtime, central) {
                process
                    .complete_central_tail(token)
                    .map_err(|e| e.to_string())?;
            }
            Self::check_camera_finalization(&observation)?;
            if let Some(writeback) = observation.strategic_writeback.take() {
                writeback.commit(session)?;
            }
            self.observe_camera(&observation)?;
        }
        Ok(())
    }
    fn complete_camera_central(
        &mut self,
        tasks: &mut [openmt2001::timers::Task],
        observation: camera_bridge::Observation,
    ) -> Result<(), String> {
        Self::apply_camera_effects(tasks, &observation);
        self.observe_camera(&observation)
    }
    fn apply_camera_effects(
        tasks: &mut [openmt2001::timers::Task],
        observation: &camera_bridge::Observation,
    ) {
        for effect in &observation.task_effects {
            match *effect {
                camera_bridge::TaskEffect::SetEnabled { mask, enabled } => {
                    for task in tasks.iter_mut() {
                        if u32::from_le_bytes(task[4..8].try_into().unwrap()) & mask != 0 {
                            task[0] = u8::from(enabled);
                        }
                    }
                }
                camera_bridge::TaskEffect::ClearElapsed => {
                    for task in tasks.iter_mut() {
                        task[12..16].fill(0);
                    }
                }
            }
        }
    }
    pub fn prompt_updates(&self) -> u64 {
        self.prompt_updates
    }
    pub fn interface_ticks(&self) -> u64 {
        self.interface_ticks
    }
    pub fn auction_save_state(&self) -> Option<crate::session_save::AuctionHostState> {
        Some(crate::session_save::AuctionHostState {
            controls: self
                .auction
                .as_ref()
                .map(|auction| auction.controls.clone())
                .unwrap_or_default(),
            visible: self.auction.as_ref().is_some_and(|auction| auction.visible),
            interface_ticks: self.interface_ticks,
            announcement_seconds: self.auction_seconds,
        })
    }
    pub fn restore_auction_widgets(
        &mut self,
        state: Option<&crate::session_save::AuctionHostState>,
        session: &Session<'_>,
    ) -> Result<(), String> {
        if let Some(state) = state {
            self.interface_ticks = state.interface_ticks;
            self.auction_seconds = state.announcement_seconds;
            if session.saved_game_mode == Some(2) {
                let auction = self
                    .auction
                    .as_mut()
                    .ok_or("V2 auction widgets have no rebuilt auction scene")?;
                let rebuilt_sound = auction.controls.timer.sound;
                auction.controls = state.controls.clone();
                auction.controls.timer.sound = rebuilt_sound;
                auction.visible = state.visible;
                auction.info = Some(crate::auction_panel::Info::from_session(session)?);
                auction.refresh_participants(session)?;
                // V2 restores the retained phase-two widget group. Native
                //464ee0 shows565b9c and565ba8 together (464f0f..464f41).
                // Vanilla465490/464aa0 only rebuilds the scene/turn labels;
                // it does not issue this show after the constructor reset.
                // Reconstruct only Time visibility from the saved group,
                // without replaying the phase setter or gameplay callbacks.
                let mut runtime = self
                    .time_runtime
                    .lock()
                    .map_err(|_| "Time runtime poisoned")?;
                if state.visible {
                    runtime.show(current_cash(session)?, 1)?;
                } else {
                    runtime.hide(1)?;
                }
            } else {
                // Announcement has not prepared a scene; settlement releases it.
                self.auction = None;
                if session.world.auction.controller.record[0x7d] != 0 {
                    // 41d700 opens with empty seconds; 41d900 writes the
                    // next whole-second boundary from the restored count.
                    self.auction_seconds = None;
                }
            }
        }
        Ok(())
    }
    pub fn set_running(&mut self, running: bool) {
        if self.stop_retired_camera() {
            return;
        }
        let was_running = self.running;
        self.running = running
            && !self.camera_retired()
            && self.failure.is_none()
            && self.options_resume.is_none()
            && !self.result_completed;
        if self.running != was_running {
            eprintln!(
                "[simulation] {}{}",
                if self.running { "RUNNING" } else { "PAUSED" },
                if self.options_resume.is_some() {
                    " (Options)"
                } else {
                    ""
                }
            );
        }
    }
    /// V2 saves retain the reason bytes. A cold Host must recover the player
    /// latch before its initial focus snapshot, or the first press adds a second hold.
    pub(crate) fn restore_player_pause(&mut self, pause: &openmt2001::pause::State) {
        self.player_paused = pause.bytes[3] != 0;
    }
    pub(crate) fn player_paused(&self) -> bool {
        self.player_paused
    }
    /// 43e5f0 toggles only its own reason3, then calls the process audio
    /// wrapper. Options and script pauses retain their independent counts.
    pub(crate) fn toggle_player_pause(
        &mut self,
        pause: &mut openmt2001::pause::State,
        timers: &mut [openmt2001::timers::Task],
        audio: impl FnOnce(bool, i8) -> Result<(), String>,
    ) -> Result<(), String> {
        let network = if let Some(process) = &self.process_runtime {
            process
                .byte(process_runtime::Byte::Network93bbb9)
                .map_err(|e| e.to_string())?
                != 0
        } else {
            self.policy.scheduler.network_enabled
        };
        if network
            || self.options_resume.is_some()
            || self.stats.is_some()
            || self.result_request.is_some()
            || self.failure.is_some()
            || self.camera_retired()
        {
            return Ok(());
        }
        self.player_paused = !self.player_paused;
        let mut apply = |change: openmt2001::pause::Timers, _: &mut openmt2001::pause::State| {
            change.apply(timers);
            Ok(())
        };
        if self.player_paused {
            pause.pause(3, &mut apply)?;
        } else {
            pause.unpause(3, &mut apply)?;
        }
        let result = audio(!self.player_paused, pause.bytes[3] as i8);
        if let Err(error) = &result {
            self.fail(error.clone());
        }
        result
    }
    /// Apply one received offline host focus operation, before the next frame.
    /// The required provider finishes device/input and guarded frontend-audio
    /// effects first. Player running and all ordinary pause owners stay intact.
    pub(crate) fn focus(
        &mut self,
        pause: &mut openmt2001::pause::State,
        timers: &mut [openmt2001::timers::Task],
        focused: bool,
        prefix: impl FnOnce(i8) -> Result<crate::intro::focus::Prefix, String>,
    ) -> Result<(), String> {
        if self.stop_retired_camera() {
            return Err("Focus Session retired".into());
        }
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = (|| {
            let prefix = prefix(pause.bytes[3] as i8)?;
            if prefix == crate::intro::focus::Prefix::ReceiverAbsent {
                return Ok(());
            }
            // Native tests this gate after the frontend callback, not instead
            // of it. No fabricated success when a required provider is absent.
            let blocked = if let Some(process) = &self.process_runtime {
                process
                    .byte(process_runtime::Byte::Network93bbb9)
                    .map_err(|e| e.to_string())?
                    != 0
            } else {
                self.policy.scheduler.network_enabled
            };
            if focused {
                pause.focus_gained(blocked, |change, _| {
                    change.apply(timers);
                    Ok(())
                })
            } else {
                pause.focus_lost(blocked, |change, _| {
                    change.apply(timers);
                    Ok(())
                })
            }
        })();
        if let Err(error) = &result {
            self.fail(error.clone());
        }
        result
    }
    ///49e860/49e994 local Options owns one reason0 transition. The UI's
    ///oneshot acknowledgement is sent only after this actual owner operation.
    ///Keep the existing player-run latch distinct from native pause bytes.
    pub fn options(
        &mut self,
        pause: &mut openmt2001::pause::State,
        timers: &mut [openmt2001::timers::Task],
        open: bool,
    ) -> Result<(), String> {
        if self.stop_retired_camera() {
            return Err("Options session retired".into());
        }
        if open {
            if let Some(error) = &self.failure {
                return Err(error.clone());
            }
            if self.options_resume.is_none() {
                if !self.policy.scheduler.network_enabled {
                    pause.pause(0, |change, _| {
                        change.apply(timers);
                        Ok(())
                    })?;
                }
                self.options_resume = Some(self.running);
                self.set_running(false);
            }
        } else if let Some(running) = self.options_resume {
            if !self.policy.scheduler.network_enabled {
                pause.unpause(0, |change, _| {
                    change.apply(timers);
                    Ok(())
                })?;
            }
            self.options_resume = None;
            self.set_running(running);
        }
        Ok(())
    }
    ///Recheck the SAME worker/Session at ordinary captured-save consumption.
    ///This is a current modal hold check, not a native construction token.
    pub fn options_save_allowed(
        &self,
        pause: &openmt2001::pause::State,
        timers: &[openmt2001::timers::Task],
    ) -> bool {
        self.options_resume.is_some()
            && !self.running
            && self.failure.is_none()
            && !self.camera_retired()
            && (self.policy.scheduler.network_enabled
                || (pause.bytes[0] != 0
                    && pause.bytes[8] != 0
                    && timers
                        .iter()
                        .filter(|t| u32::from_le_bytes(t[4..8].try_into().unwrap()) & 2 != 0)
                        .all(|t| t[0] == 0)))
    }
    /// Project only this live Options owner's transient pause into a cold-load
    /// continuation. The actual Session stays held until the UI closes Options.
    /// Other reason0 holders, other reasons, timer padding and accumulators stay.
    pub(crate) fn options_save_projection(
        &self,
        pause: &openmt2001::pause::State,
        timers: &[openmt2001::timers::Task],
    ) -> Result<Option<(openmt2001::pause::State, Vec<openmt2001::timers::Task>)>, String> {
        if self.options_resume.is_none() {
            return Ok(None);
        }
        if !self.options_save_allowed(pause, timers) {
            return Err("Options save owner is not held".into());
        }
        if self.policy.scheduler.network_enabled {
            return Ok(None);
        }
        // Native unpause is signed. A wrapped/nonpositive count cannot account
        // for the one owned release: refuse rather than clear or repair it.
        if pause.bytes[0] as i8 <= 0 {
            return Err("Options save pause count cannot release one owner".into());
        }
        let mut continuation = pause.clone();
        let mut tasks = timers.to_vec();
        continuation.unpause(0, |change, _| {
            change.apply(&mut tasks);
            Ok(())
        })?;
        Ok(Some((continuation, tasks)))
    }
    /// Native 481910 chooses one or four logic/interface passes per timer
    /// callback in an offline game; 481940 toggles between those settings.
    pub fn set_fast(&mut self, fast: bool) {
        if !self.policy.scheduler.network_enabled {
            self.policy.scheduler.iterations = if fast { 4 } else { 1 };
        }
    }
    pub fn toggle_speed(&mut self) {
        self.set_fast(self.policy.scheduler.iterations == 1);
    }
    /// Original428dc0 child1 ->428b30. This informational prompt changes
    /// neither GUI/camera mode nor any pause owner;429b50 selects offline1x.
    pub fn scenario_reminder(&mut self, level: &openmt2001::level_catalog::Level, objective: i32) {
        self.pending_prompts.push(prompt::scenario_reminder(
            level,
            self.policy.scheduler.network_enabled,
            objective,
        ));
        if !self.policy.scheduler.network_enabled {
            self.policy.scheduler.iterations = 1;
        }
    }
    /// Original Manager12/2 reaches49dcb0; local games add pause reason0.
    /// Repeated UI polls cannot reopen the modal or add another pause reason.
    pub fn stats_action(
        &mut self,
        session: &mut Session<'_>,
        action: StatsAction,
    ) -> Result<(), String> {
        match action {
            StatsAction::Open => {
                if self.stats.is_some() || self.result_completed {
                    return Ok(());
                }
                let state = crate::stats_panel::State::show(
                    session.world.objective.kind,
                    self.game_active,
                    session.roster.manager.players.len(),
                )?;
                if self.game_active && !self.policy.scheduler.network_enabled {
                    let timers = &mut session.hooks.timers;
                    session.world.pause.pause(0, |change, _| {
                        change.apply(timers);
                        Ok(())
                    })?;
                }
                self.stats = Some(state);
            }
            StatsAction::Close => {
                self.close_statistics(&mut session.world.pause, &mut session.hooks.timers)?;
            }
            StatsAction::Cycle(right) => {
                if let Some(state) = &mut self.stats {
                    state.cycle(right, session.world.objective.kind, self.game_active);
                }
            }
            StatsAction::Toggle(slot) => {
                if let Some(state) = &mut self.stats {
                    state.toggle(slot)?;
                }
            }
        }
        Ok(())
    }
    fn close_statistics(
        &mut self,
        pause: &mut openmt2001::pause::State,
        timers: &mut [openmt2001::timers::Task],
    ) -> Result<(), String> {
        if self.stats.take().is_some() && self.game_active && !self.policy.scheduler.network_enabled
        {
            pause.unpause(0, |change, _| {
                change.apply(timers);
                Ok(())
            })?;
        }
        Ok(())
    }
    pub fn dismiss_market_share(&mut self) {
        self.market_share.dismiss();
    }
    pub fn fail(&mut self, error: String) {
        self.running = false;
        let _ = crate::transport_audio::submit(
            &self.transport_audio,
            crate::transport_audio::Operation::Construction([[None; 2]; 4]),
        );
        if self.failure.is_none() {
            eprintln!("[simulation error] {error}");
            eprintln!(
                "[simulation] Stopped at an incomplete boundary; this session cannot safely resume."
            );
            self.failure = Some(error);
        }
    }
    fn comparison_data(
        session: &Session<'_>,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<crate::comparison::Data, String> {
        let players = crate::player_info::Snapshot::from_runtime(
            &session.roster.manager,
            &session.world.ledgers.players,
            session.roster.status.current_player,
        )?;
        crate::comparison::Data::from_runtime(
            &session.world,
            &players.records,
            assets.image,
            0x591318,
        )
    }
    fn refresh_comparison(
        &mut self,
        session: &Session<'_>,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        let data = Self::comparison_data(session, assets)?;
        if let Some(runtime) = &mut self.comparison {
            runtime.refresh(data);
        } else {
            self.comparison = Some(crate::comparison::Runtime::new(data));
        }
        Ok(())
    }
    pub fn select_comparison(
        &mut self,
        session: &Session<'_>,
        assets: &CommandAssets<'_, '_>,
        mode: usize,
    ) -> Result<(), String> {
        if self.comparison.is_none() {
            self.refresh_comparison(session, assets)?;
        }
        let data = Self::comparison_data(session, assets)?;
        self.comparison.as_mut().unwrap().select(mode, data)
    }
    pub fn status(&mut self, session: &Session<'_>) -> Status {
        self.collect_script_prompts(session);
        let prompts = std::mem::take(&mut self.pending_prompts);
        let auction_notice =
            match crate::auction_notice::Notice::from_session(session, self.auction_seconds) {
                Ok(notice) => notice,
                Err(error) => {
                    self.fail(error);
                    None
                }
            };
        let mut auction = self.auction.clone();
        if let Some(auction) = &mut auction {
            auction.hidden_bid_toggle_enabled = !self.policy.scheduler.network_enabled;
            auction.bid_voice_gate = self.policy.ai_event_gate;
            // Publish names, authoritative cash and the final controller state
            // together, including AI actions and restored auction sessions.
            if let Err(error) = auction.refresh_participants(session) {
                self.fail(error);
            }
            match crate::auction_panel::Info::from_session(session) {
                Ok(info) => auction.info = Some(info),
                Err(error) => self.fail(error),
            }
        }
        let notices = match self.notices.view(session) {
            Ok(notices) => notices,
            Err(error) => {
                self.fail(error);
                Vec::new()
            }
        };
        let card_entry = match session
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == session.roster.status.current_player)
            .ok_or_else(|| "Card entry current player missing".to_string())
            .and_then(|p| {
                crate::card_panel::Entry::player(
                    p,
                    session.world.city.auction.controller.reference != 0
                        && session.world.city.auction.controller.record[0x7c] != 0,
                )
            }) {
            Ok(entry) => entry,
            Err(error) => {
                self.fail(error);
                Default::default()
            }
        };
        let financial = match crate::player_info::Snapshot::from_runtime(
            &session.roster.manager,
            &session.world.ledgers.players,
            session.roster.status.current_player,
        ) {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                self.fail(error);
                None
            }
        };
        let city_people_rows = match crate::city_people::Report::live_rows(&session.world) {
            Ok(rows) => Some(rows),
            Err(error) => {
                self.fail(error);
                None
            }
        };
        let (time_display, card_visual) = {
            let runtime = self.time_runtime.lock().expect("Time runtime poisoned");
            let instance = usize::from(self.auction.is_some());
            (
                runtime.widgets[instance].display(),
                runtime.chance_visuals[instance],
            )
        };
        let stats = match self
            .stats
            .as_ref()
            .map(|state| crate::stats_panel::Snapshot::from_session(session, state))
            .transpose()
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.fail(error);
                None
            }
        };
        Status {
            autosave: self.autosave_status.clone(),
            time_display,
            cash_arrow: self.cash_arrow,
            window_texture: session.window_texture.pending(),
            pulse_lights: session.pulse_lights.clone(),
            demolition_points: session.demolition_points.clone(),
            stats,
            comparison: self.comparison.as_ref().map(|state| state.snapshot.clone()),
            market_share: self.market_share.snapshot.clone(),
            result_request: self.result_request,
            result_review: self.result_review(&session.catalog),
            player_panels: self.player_panels,
            financial,
            business_owner: session
                .roster
                .manager
                .players
                .iter()
                .find(|p| p.reference == session.roster.status.current_player)
                .map(|p| p.bytes[0x1bc].wrapping_add(1)),
            current_player_active: session
                .roster
                .manager
                .players
                .iter()
                .find(|p| p.reference == session.roster.status.current_player)
                .is_some_and(|p| p.bytes[0x10] != 0),
            city_people_rows,
            tutorial_controls: self.tutorial_controls,
            source_picking: self
                .process_runtime
                .as_ref()
                .and_then(|p| p.picking_projection().ok()),
            tutorial_arrows: self.tutorial_arrows.clone(),
            tutorial_message: session
                .hooks
                .script_prompt()
                .filter(|r| r.dialog.is_some_and(|d| d.button_count == 0)),
            prompt_closes: std::mem::take(&mut self.pending_prompt_closes),
            tutorial_enabled_controls: self.tutorial_enabled_controls.iter().copied().collect(),
            tutorial_interaction: self.tutorial_interaction,
            tutorial_camera_events: self.tutorial_camera_events.clone(),
            tutorial_build_type: if session.tutorial_state == 1 {
                self.tutorial_build_type
            } else {
                -1
            },
            tutorial_build_use: if session.tutorial_state == 1 {
                self.tutorial_build_use
            } else {
                999
            },
            tutorial_completed: self.tutorial_completed,
            tutorial_session: session.tutorial_state == 1,
            event_list_visible: self.event_list_visible,
            event_list_settled: self.event_list_visible && !self.event_list_opening,
            card: self.card.filter(|_| self.card_visible),
            card_visual,
            card_entry,
            day_night_flags: self.day_night_flags,
            notices,
            auction,
            running: self.running,
            speed: self.policy.scheduler.iterations,
            network_enabled: self.policy.scheduler.network_enabled,
            script_paused: session.world.pause.has_reason(),
            ticks: session.world.clock.timing[0],
            interface_ticks: self.interface_ticks,
            prompt_updates: self.prompt_updates,
            year: 1930 + session.world.clock.calendar[2] * 10 + session.world.clock.calendar[1],
            hour: session.world.clock.hour(),
            hour_fraction: session.world.clock.timing[2],
            failure: self.failure.clone(),
            ai_warning_count: self.ai_warnings.count,
            last_ai_warning: self.ai_warnings.last_message.clone(),
            pending_presentation: self.pending_presentation.iter().copied().collect(),
            prompts,
            auction_seconds: self.auction_seconds,
            auction_notice,
            auction_locked_categories: session
                .world
                .categories
                .iter()
                .enumerate()
                .filter_map(|(i, row)| (row[0x6c] == 1).then_some(i))
                .collect(),
        }
    }
    pub fn take_prompts(&mut self, session: &Session<'_>) -> Vec<prompt::Request> {
        self.collect_script_prompts(session);
        std::mem::take(&mut self.pending_prompts)
    }
    /// Deliver an enabled tutorial control action through the original Lua
    /// CATCHEVENT, then close the queued message when the script requests it.
    pub fn tutorial_event(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        control: u32,
    ) -> Result<(), String> {
        if self.failure.is_some() || session.tutorial_state != 1 || !self.tutorial_started {
            // Original48dc40 returns without dispatch when level+85c !=1.
            // Campaign builder controls share these producers with tutorials.
            return Ok(());
        }
        self.collect_script_prompts(session);
        if !self.tutorial_enabled_controls.contains(&control) {
            // Admission can change between the UI snapshot and this queued
            // input. A disabled control has no callback or simulation mutation.
            return Ok(());
        }
        self.dispatch_tutorial_callback(session, assets, control)
    }
    /// Native48dc40 receives a callback from an already admitted producer.
    /// Callback7 from numeric/list edits differs from registered controls
    /// 41..44/40. The producer owns input admission; this helper preserves the
    /// original active-level byte gate and synchronous prompt continuation.
    pub fn tutorial_producer_event(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        control: u32,
        callback: u32,
    ) -> Result<(), String> {
        if self.failure.is_some() || session.tutorial_state != 1 || !self.tutorial_started {
            return Ok(());
        }
        self.collect_script_prompts(session);
        if callback != 7 || !matches!(control, 40..=44) {
            return Err(format!(
                "Unsupported tutorial producer {control}/{callback}"
            ));
        }
        if !self.tutorial_enabled_controls.contains(&control) {
            return Ok(());
        }
        self.dispatch_tutorial_callback(session, assets, callback)
    }
    fn dispatch_tutorial_callback(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        callback: u32,
    ) -> Result<(), String> {
        if self.failure.is_some() || session.tutorial_state != 1 {
            return Ok(());
        }
        self.collect_script_prompts(session);
        let active = session.hooks.script_prompt();
        let before = session.hooks.commands.len();
        session.dispatch_session_event("TutorialEvent", callback as i32, assets, false)?;
        if session.hooks.commands[before..]
            .iter()
            .any(|(command, _)| matches!(command, level_script::Command::TutorialDismissPrompt))
        {
            // Native43bed0 skips dismissal when its active byte+345 is zero.
            if let Some(message) = active {
                if message.dismissal_id != 0 {
                    session.dispatch_session_event(
                        "InterfacePromptDismiss",
                        message.dismissal_id as i32,
                        assets,
                        false,
                    )?;
                }
                if message.dismissal_id == 0 {
                    session.hooks.close_script_prompt(false);
                }
                session
                    .hooks
                    .script_prompt_closes
                    .push(session.hooks.commands.len());
            }
        }
        self.collect_script_prompts(session);
        Ok(())
    }
    /// A committed 3D navigation pick is admitted by the original tutorial
    /// whitelist before the retained CATCHEVENT_Picked3D callback. Native
    /// 4c3929/4c3979 dispatch 0 for block selection, 4c3beb/4c3cf6 dispatch
    /// 1 for block entry, and 4c3958 dispatches 2 for building selection.
    /// The script owns dismissal and the following prompt.
    fn tutorial_pick_callback(&self, pick: crate::tutorial_two_interaction::Pick) -> Option<i32> {
        if self.tutorial_controls.picking_disabled {
            None
        } else {
            self.tutorial_interaction.callback(pick)
        }
    }
    pub fn tutorial_pick(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        pick: crate::tutorial_two_interaction::Pick,
    ) -> Result<bool, String> {
        if self.failure.is_some() || session.tutorial_state != 1 || !self.tutorial_started {
            return Ok(false);
        }
        self.collect_script_prompts(session);
        let Some(callback) = self.tutorial_pick_callback(pick) else {
            return Ok(false);
        };
        // Tutorials 10/11 attach pick gates to prompts too. The active queue
        // head supplies the continuation, independently of its button count.
        let message = session
            .hooks
            .script_prompt()
            .ok_or("Tutorial pick has no active prompt")?;
        let before = session.hooks.commands.len();
        session.dispatch_session_event("Picked3D", callback, assets, false)?;
        if session.hooks.commands[before..]
            .iter()
            .any(|(command, _)| matches!(command, level_script::Command::TutorialDismissPrompt))
        {
            if message.dismissal_id != 0 {
                session.dispatch_session_event(
                    "InterfacePromptDismiss",
                    message.dismissal_id as i32,
                    assets,
                    false,
                )?;
            }
            if message.dismissal_id == 0 {
                session.hooks.close_script_prompt(false);
            }
            session
                .hooks
                .script_prompt_closes
                .push(session.hooks.commands.len());
        }
        self.collect_script_prompts(session);
        Ok(true)
    }
    pub fn human_card(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        action: fresh_city::cards::Action,
    ) -> Result<(), String> {
        if self.failure.is_some() {
            return Err("Stopped session cannot accept card controls".into());
        }
        if matches!(action, fresh_city::cards::Action::Open) {
            let player = session
                .roster
                .manager
                .players
                .iter()
                .find(|p| p.reference == session.roster.status.current_player)
                .ok_or("Card current player missing")?;
            if !crate::card_panel::Entry::player(
                player,
                session.world.city.auction.controller.reference != 0
                    && session.world.city.auction.controller.record[0x7c] != 0,
            )?
            .can_open()
            {
                return Ok(());
            }
        }
        if let fresh_city::cards::Action::Button { button, .. } = action
            && (!self.card_visible
                || !self
                    .card
                    .is_some_and(|p| p.buttons.get(button as usize) == Some(&true)))
        {
            return Ok(());
        }
        let network = [
            self.policy.scheduler.network_enabled,
            self.policy.scheduler.network_override,
        ];
        session.human_card(action, assets, network, |call, player| {
            match call {
                openmt2001::event_cards::human::Call::Present(card) => {
                    self.card = Some(crate::card_panel::Panel::present(
                        card,
                        player,
                        assets.image,
                    )?)
                }
                openmt2001::event_cards::human::Call::Activate => {
                    if !self.card_visible {
                        self.pending_presentation.insert(0x41b430);
                        let card = self
                            .card
                            .as_ref()
                            .ok_or("Card activation has no presented panel")?
                            .card;
                        let positive =
                            crate::gameplay_sound::card_open_positive(assets.image, card)?;
                        crate::gameplay_sound::enqueue(
                            &self.gameplay_sound,
                            crate::gameplay_sound::Operation::CardOpen { positive },
                        )?;
                    }
                    self.card_visible = true;
                }
                openmt2001::event_cards::human::Call::Close => self.card_visible = false,
                other => return Err(format!("Unresolved card presentation {other:?}")),
            }
            Ok(())
        })
    }
    pub fn business_operation(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        building: usize,
        action: openmt2001::business_controls::Action,
    ) -> Result<bool, String> {
        if self.failure.is_some() {
            return Err("Stopped session cannot accept building operations".into());
        }
        let network = [
            self.policy.scheduler.network_enabled,
            self.policy.scheduler.network_override,
        ];
        session.business_operation(building, action, assets.image, network, |call| {
            match call {
                openmt2001::business_controls::Call::Ui {
                    address,
                    arguments,
                    count,
                } => {
                    if address == 0x43be20 && !network[0] {
                        self.policy.scheduler.iterations = 1;
                    }
                    human_dialog_ui(
                        &mut self.pending_prompts,
                        address,
                        [arguments[0], arguments[1], arguments[2], 0],
                        count,
                        None,
                    )
                }
                // Building pending bytes were retained by the original producer;
                // the next authoritative inspection determines disabled controls.
                openmt2001::business_controls::Call::Disable(_)
                | openmt2001::business_controls::Call::ClearConversionSelection => Ok(()),
                other => Err(format!(
                    "Unresolved building operation presentation {other:?}"
                )),
            }
        })
    }
    pub fn property_rights(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        category: u32,
        panel_mode: u8,
        activate: bool,
    ) -> Result<openmt2001::property_rights::Inspection, String> {
        if activate {
            if self.failure.is_some() {
                return Err("Stopped session cannot activate building rights".into());
            }
            let inspection = session.activate_property_rights(category, panel_mode)?;
            // Original 41ffef calls 4050d0 before 41fff4 publishes event91.
            // Keep the worker transaction ahead of the tutorial continuation.
            self.tutorial_event(session, assets, 91)?;
            Ok(inspection)
        } else {
            session.inspect_property_rights(category, panel_mode)
        }
    }
    pub fn inspect_property_offer(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        category: u32,
        panel: openmt2001::property_offer::Panel,
        initialize_amount: bool,
    ) -> Result<crate::fresh_city::property::Inspection, String> {
        session.inspect_property_offer_display(
            category,
            panel,
            initialize_amount,
            self.auction.as_ref().is_some_and(|auction| auction.visible),
            assets,
        )
    }
    pub fn property_offer(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        category: u32,
        panel: openmt2001::property_offer::Panel,
    ) -> Result<crate::fresh_city::property::Inspection, String> {
        if self.failure.is_some() {
            return Err("Stopped session cannot accept property offers".into());
        }
        let inspection = self.inspect_property_offer(session, assets, category, panel, false)?;
        if inspection.panel.action_visible == 0 {
            return Ok(inspection);
        }
        session.submit_property_offer_panel(category, inspection.panel, assets, |event| {
            use openmt2001::{live_commands::Event, property_offer::Call};
            match event {
                Event::PropertyOffer(Call::Ui {
                    address,
                    arguments,
                    count,
                }) => {
                    if openmt2001::construction::INSUFFICIENT_CASH_CALLS
                        .iter()
                        .any(|(expected, _)| *expected == address)
                    {
                        if address == 0x43be20 && !self.policy.scheduler.network_enabled {
                            self.policy.scheduler.iterations = 1;
                        }
                        human_dialog_ui(
                            &mut self.pending_prompts,
                            address,
                            [arguments[0], arguments[1], arguments[2], 0],
                            usize::from(count),
                            None,
                        )
                    } else if matches!(
                        address,
                        0x442960 | 0x441940 | 0x441220 | 0x42efb0 | 0x42ee70 | 0x442af0
                    ) {
                        // The returned authoritative panel carries these widget
                        // changes. The UI owns its geometry and pointer state.
                        Ok(())
                    } else {
                        Err(format!("Unresolved property offer widget {address:08x}"))
                    }
                }
                Event::PropertyOffer(Call::RefreshProperty)
                | Event::PlayerMessageRoute {
                    route: openmt2001::player_messages::Route::PropertyRefresh { .. },
                    ..
                } => {
                    self.pending_presentation.insert(0x405260);
                    Ok(())
                }
                other => Err(format!("Unresolved property offer presentation {other:?}")),
            }
        })
    }
    fn collect_script_closes(&mut self, session: &Session<'_>, boundary: usize) {
        while session
            .hooks
            .script_prompt_closes
            .get(self.script_closes_seen)
            == Some(&boundary)
        {
            self.pending_prompt_closes.push(self.pending_prompts.len());
            self.script_closes_seen += 1;
        }
    }
    fn collect_script_prompts(&mut self, session: &Session<'_>) {
        if self.stop_retired_camera() || (self.camera_bridge.is_some() && self.failure.is_some()) {
            return;
        }
        for index in self.commands_seen..session.hooks.commands.len() {
            self.collect_script_closes(session, index);
            let (command, arguments) = &session.hooks.commands[index];
            if let Some(process) = &self.process_runtime {
                if let Err(error) = apply_source_picking_command(process, command, arguments) {
                    self.fail(error);
                    return;
                }
            }
            match command {
                level_script::Command::TutorialEnableAllPicking => {
                    self.tutorial_controls.picking_disabled = false;
                }
                level_script::Command::PlaySound => {
                    let result = match arguments.as_slice() {
                        [Value::Bytes(name)] => std::str::from_utf8(name)
                            .map_err(|_| "script sound filename is not UTF-8".to_owned())
                            .and_then(|name| {
                                crate::gameplay_sound::enqueue(
                                    &self.gameplay_sound,
                                    crate::gameplay_sound::Operation::Script {
                                        name: name.to_owned(),
                                    },
                                )
                            }),
                        _ => Err("script sound requires one filename".to_owned()),
                    };
                    if let Err(error) = result {
                        self.fail(error);
                    }
                }
                level_script::Command::Prompt | level_script::Command::TutorialMessage => {
                    let request = script_request(
                        arguments,
                        session.tutorial_state == 1,
                        matches!(command, level_script::Command::Prompt),
                    );
                    match request {
                        Ok(request) => {
                            self.tutorial_arrows.clear();
                            self.pending_prompts.push(request);
                        }
                        Err(error) => self.fail(error),
                    }
                }
                level_script::Command::TutorialDisableAllControls => {
                    self.tutorial_controls.all_disabled = true;
                    self.tutorial_enabled_controls.clear();
                }
                level_script::Command::TutorialDisableAllPicking => {
                    //Common enable toggles independently of retained block/
                    //building policy. EnableAll must restore the same gate.
                    self.tutorial_controls.picking_disabled = true;
                }
                level_script::Command::TutorialDisableCameraControl => {
                    self.tutorial_controls.camera_disabled = true
                }
                level_script::Command::TutorialArrow => {
                    if let [Value::Number(x), Value::Number(y)] = arguments.as_slice() {
                        self.tutorial_arrows.push([*x as i32, *y as i32]);
                    } else {
                        self.fail("Tutorial arrow coordinates changed".into());
                    }
                }
                level_script::Command::TutorialDismissPrompt => {
                    self.tutorial_arrows.clear();
                }
                level_script::Command::TutorialEnableControl
                | level_script::Command::TutorialEnableControlAndChildren
                | level_script::Command::TutorialDisableControl => {
                    if let [Value::Number(value)] = arguments.as_slice() {
                        let id = *value as u32;
                        if matches!(
                            command,
                            level_script::Command::TutorialEnableControl
                                | level_script::Command::TutorialEnableControlAndChildren
                        ) {
                            self.tutorial_enabled_controls.insert(id);
                            // Native4db910/48dd60 recursively enables descendants.
                            // The retained owned-business group30 contains hyperlink6.
                            if matches!(id, 30 | 32)
                                && matches!(
                                    command,
                                    level_script::Command::TutorialEnableControlAndChildren
                                )
                            {
                                self.tutorial_enabled_controls.insert(6);
                            }
                            if id == 104
                                && matches!(
                                    command,
                                    level_script::Command::TutorialEnableControlAndChildren
                                )
                            {
                                // Native420f20..4228e9 publishes these same-manager
                                // descendants before publishing manager104. Internal
                                // callbacks88/99 and property-rights91 are not nodes.
                                self.tutorial_enabled_controls.extend([
                                    86, 87, 89, 90, 92, 93, 94, 95, 96, 97, 98, 100, 101, 102,
                                ]);
                            }
                        } else {
                            self.tutorial_enabled_controls.remove(&id);
                            if matches!(id, 30 | 32) {
                                self.tutorial_enabled_controls.remove(&6);
                            }
                        }
                    } else {
                        self.fail("Tutorial control identifier changed".into());
                    }
                }
                level_script::Command::TutorialPositionPrompt => {
                    if let [x, y] = arguments.as_slice() {
                        let part = |value: &Value| match value {
                            Value::Number(number) => Some(*number as f32),
                            _ => None,
                        };
                        self.tutorial_interaction
                            .position_prompt_parts(part(x), part(y));
                    } else {
                        self.fail("Tutorial prompt position arguments changed".into());
                    }
                }
                level_script::Command::TutorialEnableBlockPicking => {
                    match tutorial_pick_target(arguments) {
                        Ok(target) => {
                            self.tutorial_interaction.enable_block_picking(target);
                            self.tutorial_controls.picking_disabled = false;
                        }
                        Err(error) => self.fail(error),
                    }
                }
                level_script::Command::TutorialEnableBuildingPicking => {
                    match tutorial_pick_target(arguments) {
                        Ok(target) => {
                            self.tutorial_interaction
                                .enable_building_picking(target.map(|v| v as usize));
                            self.tutorial_controls.picking_disabled = false;
                        }
                        Err(error) => self.fail(error),
                    }
                }
                level_script::Command::TutorialCameraMode => {
                    if let [Value::Number(mode), Value::Number(target)] = arguments.as_slice() {
                        self.tutorial_camera_action(
                            session,
                            TutorialCameraAction::Mode {
                                mode: *mode as u32,
                                target: *target as u32,
                            },
                        );
                    } else {
                        self.fail("Tutorial Camera.SetMode arguments changed".into());
                    }
                }
                level_script::Command::TutorialCityView => {
                    self.tutorial_camera_action(session, TutorialCameraAction::CityView)
                }
                level_script::Command::TutorialPlanView => {
                    self.tutorial_camera_action(session, TutorialCameraAction::PlanView)
                }
                level_script::Command::TutorialBuildType => {
                    if let [Value::Number(value)] = arguments.as_slice() {
                        self.tutorial_build_type = *value as i32;
                    } else {
                        self.fail("Tutorial builder type requires its native integer".into());
                    }
                }
                level_script::Command::TutorialBuildUse => {
                    if let [Value::Number(value)] = arguments.as_slice() {
                        self.tutorial_build_use = *value as i32;
                    } else {
                        self.fail("Tutorial builder use requires its native integer".into());
                    }
                }
                level_script::Command::CreateEventList => {
                    //43be20 returns immediately for an already visible instance.
                    if !self.event_list_visible {
                        self.event_list_visible = true;
                        self.event_list_opening = true;
                    }
                }
                level_script::Command::ResultEnd => {
                    // All Game.End routes reach4ba1d0 and clear8b22dc at4ba291.
                    // Player pause and result-sequence entry leave this byte set.
                    self.game_active = false;
                    self.running = false;
                    //4ba1df/43c690 hides visible widgets in normal manager57e710.
                    let hidden = self
                        .time_runtime
                        .lock()
                        .map_err(|_| "Time runtime poisoned".to_owned())
                        .and_then(|mut runtime| runtime.hide(0));
                    if let Err(error) = hidden {
                        self.fail(error);
                    }
                    if session.tutorial_state == 1 {
                        self.tutorial_end_requested = Some(matches!(arguments.first(),
                            Some(Value::Number(value)) if *value != 0.));
                    }
                }
                _ => (),
            }
            // Send each admission mutation in command order, before a following
            // camera setter. Status continues to carry presentation information;
            // bridged actions are never also appended to its camera event list.
            if self.camera_bridge.is_some()
                && matches!(
                    command,
                    level_script::Command::TutorialDisableAllControls
                        | level_script::Command::TutorialDisableCameraControl
                        | level_script::Command::TutorialEnableControl
                        | level_script::Command::TutorialEnableControlAndChildren
                        | level_script::Command::TutorialDisableControl
                )
            {
                let result = self.request_camera(camera_bridge::Operation::Admission(
                    self.camera_admission(session),
                ));
                if let Err(error) = result {
                    self.camera_error(error);
                }
            }
            if self.stop_retired_camera()
                || (self.camera_bridge.is_some() && self.failure.is_some())
            {
                return;
            }
        }
        self.collect_script_closes(session, session.hooks.commands.len());
        self.commands_seen = session.hooks.commands.len();
    }
    pub fn tick(&mut self, session: &mut Session<'_>, assets: &CommandAssets<'_, '_>) {
        if let Err(error) = session.pump_connected_commands(&mut self.policy) {
            self.fail(error);
            return;
        }
        if self.stop_retired_camera() {
            return;
        }
        self.collect_script_prompts(session);
        if self.stop_retired_camera() {
            return;
        }
        if !self.running || self.failure.is_some() || session.world.pause.has_reason() {
            return;
        }
        if let Err(error) = self.request_camera(camera_bridge::Operation::CurrentPose) {
            self.camera_error(error);
            return;
        }
        if self.stop_retired_camera() {
            return;
        }
        // Original93bbb9 is the network gate, not a new save preference.
        self.policy.calendar.autosave_disabled = self.policy.scheduler.network_enabled;
        let autosave_root = &self.autosave_profile_root;
        let autosave_status = &mut self.autosave_status;
        let ai_warnings = &mut self.ai_warnings;
        let camera_bridge = &mut self.camera_bridge;
        let ui_camera_source = &self.ui_camera_source;
        let ui_camera_epoch = session.world.topology.epoch();
        let interface_ticks = self.interface_ticks;
        let mut autosave_retired = false;
        let mut autosave_fatal = None;
        let omitted = &mut self.pending_presentation;
        let transport_audio = self.transport_audio.clone();
        let auction_audio = self.auction_audio.clone();
        let camera = self.policy.camera;
        let block_previous_tick = session.world.clock.timing[0];
        let block_random_state =
            openmt2001::startup_rng::Snapshot::from_seed(session.policy.seed).state(0x8af3ec);
        let mut construction_sounds = [[None; 2]; 4];
        let era_audio = self.era_audio.clone();
        let gameplay_sound = self.gameplay_sound.clone();
        let day_night_flags = &mut self.day_night_flags;
        let player_panels = &mut self.player_panels;
        let market_share = &mut self.market_share;
        let objective_kind = session.world.objective.kind;
        let prompts = &mut self.pending_prompts;
        let auction_seconds = &mut self.auction_seconds;
        let auction_renderer = &self.auction_renderer;
        let auction_actor_events = std::cell::RefCell::new(self.auction_actor_events.take());
        let bid_gate = std::cell::Cell::new(self.policy.ai_event_gate);
        let auction = &mut self.auction;
        let time_runtime = &self.time_runtime;
        let outcome_callback_seen = &mut self.outcome_callback_seen;
        let outcome_context = &mut self.outcome_context;
        let notices = &mut self.notices;
        let tutorial_state = session.tutorial_state;
        let current_slot = session
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == session.roster.status.current_player)
            .map(|p| p.bytes[0x1bc]);
        let players: Vec<_> = session
            .roster
            .manager
            .players
            .iter()
            .map(|p| p.bytes.clone())
            .collect();
        let cash_arrow = std::cell::RefCell::new(self.cash_arrow);
        let result = session.advance_session_with_animations(assets, &mut self.policy,
            || {
                cash_arrow.borrow_mut().advance();
                if let Some(sender) = auction_actor_events.borrow().as_ref() {
                    sender.send(crate::auction_bridge::ActorEvent::Advance)
                        .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
                }
                Ok(())
            }, |event, clock| {
            use fresh_city::tick::Event;
            match event {
                Event::Autosave(request)=>{
                    use crate::save_menu::{AutosaveFailure as Failure,AutosaveStatus as Status,PreparedAutosave};
                    let destination=autosave_root.as_ref().map(|root|root.join("saves/autosave.sav"));
                    let attempt=(||->Result<std::path::PathBuf,Failure>{
                        let root=autosave_root.as_deref().ok_or(Failure::MissingSelectedProfile)?;
                        let prepared=PreparedAutosave::prepare(root).map_err(Failure::Storage)?;
                        let mut ui_snapshot=None;
                        let (cameras,mode)=if let Some(worker)=camera_bridge.as_mut() {
                            // Native route retains its actual protocol and
                            // fatal owner-error/retirement behavior unchanged.
                            let observation=match worker.request(camera_bridge::Operation::RawCapture) {
                                Ok(observation)=>observation,
                                Err(camera_bridge::RequestError::Retired)=>{
                                    autosave_retired=true;
                                    return Err(Failure::UnsupportedState("Camera generation retired".into()));
                                },
                                Err(camera_bridge::RequestError::Failed(error))=>{
                                    autosave_fatal=Some(error.clone());
                                    return Err(Failure::UnsupportedState(error));
                                },
                            };
                            let raw=observation.raw_capture.ok_or_else(||Failure::UnsupportedState("Raw camera capture absent".into()))?;
                            (raw.cameras,raw.interface_mode.map_err(Failure::UnsupportedState)?)
                        } else {
                            let source=ui_camera_source.as_ref().ok_or(Failure::UnsupportedCamera)?;
                            let snapshot=source.read(ui_camera_epoch).map_err(Failure::UnsupportedState)?;
                            let pair=(snapshot.capture(),snapshot.interface_mode());
                            ui_snapshot=Some((source,snapshot));
                            pair
                        };
                        let host=Some(crate::session_save::AuctionHostState {
                            controls:auction.as_ref().map(|a|a.controls.clone()).unwrap_or_default(),
                            visible:auction.as_ref().is_some_and(|a|a.visible),
                            interface_ticks,announcement_seconds:*auction_seconds,
                        });
                        let bytes=request.encode_once(cameras.into_records(),clock.timing[5],host,mode)
                            .map_err(Failure::Encoding)?;
                        match ui_snapshot {
                            Some((source,snapshot))=>prepared.publish_from_snapshot(bytes,source,&snapshot),
                            None=>prepared.publish(bytes),
                        }.map_err(Failure::Storage)
                    })();
                    // A real protocol/owner failure remains fatal, independently
                    // of ordinary save/storage/unsupported-state failures.
                    if autosave_retired {return Err("Camera generation retired during Autosave".into());}
                    if let Some(error)=autosave_fatal.take() {return Err(error);}
                    *autosave_status=Some(match attempt {
                        Ok(path)=>{eprintln!("Autosave saved: {}",path.display());Status::Saved(path)},
                        Err(error)=>{eprintln!("Autosave failed: {error:?}");Status::Failed{destination,error}},
                    });
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::Elimination(call)) => {
                    use openmt2001::player_elimination::Call;
                    match call {
                        Call::PlayerUi { object } => {
                            *player_panels
                                .get_mut(object)
                                .ok_or("elimination panel outside normal interface")? = false;
                        }
                        // Core ownership, accounting and timestamps precede these
                        // original appearance refreshes. Scene snapshots read them.
                        Call::BuildingOwner { .. } | Call::BuildingRefresh { .. } => {
                            omitted.insert(0x4450a0);
                        }
                        // Session already disabled the optional command timer.
                        Call::FinishLocal => {}
                        other => return Err(format!("Live elimination requires {other:?}")),
                    }
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::LeaseNotification(call)) => {
                    human_dialog_ui(
                        prompts,
                        call.address,
                        [call.arguments[0], call.arguments[1], call.arguments[2], 0],
                        usize::from(call.argument_count),
                        None,
                    )?;
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::OutcomeNotice { .. }) => Ok(0),
                Event::Command(openmt2001::live_commands::Event::Outcome(call)) => {
                    use openmt2001::outcome_notice::Call;
                    match call {
                        Call::Show => human_dialog_ui(prompts, 0x43be20, [0; 4], 0, None)?,
                        Call::Pause | Call::Timers(_) => return Err(
                            "Outcome pause/timers reached presentation before Session application"
                                .into(),
                        ),
                        Call::Handler { callback, context } => {
                            if callback == Some(0x48b490) { *outcome_callback_seen = true; }
                            if let Some(context) = context { *outcome_context = Some(context); }
                            prompts.iter_mut().rev().find_map(|r| r.dialog.as_mut())
                                .ok_or("Outcome modal missing allocated prompt")?.outcome(call)?;
                        }
                        other => prompts
                            .iter_mut()
                            .rev()
                            .find_map(|r| r.dialog.as_mut())
                            .ok_or("Outcome modal missing allocated prompt")?
                            .outcome(other)?,
                    }
                    Ok(0)
                }
                Event::HumanDialog { call, name } => {
                    use openmt2001::human_messages::Call;
                    match call {
                        Call::Ui {
                            address,
                            arguments,
                            count,
                        } => human_dialog_ui(prompts, address, arguments, count, name)?,
                        Call::Handler { callback, context } => {
                            let dialog = prompts
                                .iter_mut()
                                .rev()
                                .find_map(|r| r.dialog.as_mut())
                                .ok_or("Human modal handler has no allocated prompt")?;
                            dialog.callback = callback;
                            dialog.handler_written = true;
                            if let Some(context) = context {
                                dialog.context = context;
                                dialog.context_written = true;
                            }
                        }
                        Call::Diagnostic => eprintln!("Original human inbox diagnostic"),
                    }
                    Ok(0)
                }
                Event::Player(openmt2001::player_tick::Event::Ui {
                    address,
                    arguments,
                    count,
                    ..
                }) => {
                    human_dialog_ui(
                        prompts,
                        address,
                        [arguments[0], arguments[1], arguments[2], 0],
                        count,
                        None,
                    )?;
                    Ok(0)
                }
                Event::MarketShare(openmt2001::player_share::Call::Objective { message, detail }) => {
                    // Native 48a150 creates the standard interface modal,
                    // selects portrait 302, writes title/body language IDs,
                    // and exposes one button 1435. It installs no callback.
                    human_dialog_ui(prompts, 0x43be20, [0; 4], 0, None)?;
                    for (address, arguments, count) in [
                        (0x429af0, [302, 0, 0, 0], 1),
                        (0x429820, [0, message, 0, 0], 3),
                        (0x429820, [1, detail, 0, 0], 3),
                        (0x429a60, [1, 0, 0, 0], 1),
                        (0x4298b0, [0, 0x59b, 0, 0], 2),
                    ] {
                        human_dialog_ui(prompts, address, arguments, count, None)?;
                    }
                    Ok(0)
                }
                Event::MarketShare(call @ (openmt2001::player_share::Call::Refresh
                    | openmt2001::player_share::Call::Kind1ProfileSelect { .. }
                    | openmt2001::player_share::Call::ProfileSelect { .. }
                    | openmt2001::player_share::Call::PlayerShare { .. })) => {
                    market_share.apply(call, objective_kind, &players)?;
                    Ok(0)
                }
                Event::Calendar(openmt2001::level_runtime::CalendarEvent::Clock(
                    openmt2001::clock::Event::Hour19,
                )) => {
                    //4805c0: three byte stores, no calls or simulation writes.
                    *day_night_flags = [1, 1, 1];
                    omitted.insert(0x4805c0);
                    Ok(0)
                }
                Event::Calendar(openmt2001::level_runtime::CalendarEvent::Clock(
                    openmt2001::clock::Event::Hour7,
                )) => {
                    //4805e0:822b0e remains1 while the other two bytes clear.
                    //Night material/light rendering remains a disclosed feature.
                    *day_night_flags = [0, 1, 0];
                    omitted.insert(0x4805e0);
                    Ok(0)
                }
                Event::Calendar(openmt2001::level_runtime::CalendarEvent::Hourly(
                    openmt2001::hourly::Event::StopSound(handle),
                )) => crate::era_audio::request(
                    &era_audio, crate::era_audio::Operation::BeforeSixRelease(handle)),
                Event::Command(openmt2001::live_commands::Event::AuctionScenePrepare) => {
                    let characters = players
                        .iter()
                        .map(|p| {
                            p.get(0x150..0x154)
                                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                                .ok_or_else(|| {
                                    "Auction player character record is truncated".to_owned()
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let (actor_sender, actor_receiver) = std::sync::mpsc::channel();
                    let previous = crate::auction_bridge::request(
                        auction_renderer,
                        crate::auction_bridge::Operation::Prepare { characters,
                            actor_events: actor_receiver },
                    )?;
                    *auction_actor_events.borrow_mut() = Some(actor_sender);
                    bid_gate.set(false);
                    *auction = Some(Auction::default());
                    Ok(previous)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionActivation(
                    openmt2001::ai_messages::offers::auction::activation::Call::Camera,
                )) => crate::auction_bridge::request(
                    auction_renderer,
                    crate::auction_bridge::Operation::Activate,
                ),
                Event::Command(openmt2001::live_commands::Event::AuctionActivation(
                    openmt2001::ai_messages::offers::auction::activation::Call::Ambient,
                )) => crate::gameplay_sound::request(
                    &gameplay_sound, crate::gameplay_sound::Operation::AuctionAmbientCreate),
                Event::Command(openmt2001::live_commands::Event::AuctionTimeShow { cash }) => {
                    time_runtime.lock().map_err(|_| "Time runtime poisoned")?.show(cash, 1)?;
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionActivation(
                    openmt2001::ai_messages::offers::auction::activation::Call::Action(call),
                ))
                | Event::Command(openmt2001::live_commands::Event::AuctionAction(call))
                | Event::Ai(openmt2001::ai_runtime::Call::AuctionAction(call)) => {
                    auction_action(call, auction, omitted, tutorial_state,
                        &auction_actor_events.borrow(), &bid_gate)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionActivation(
                    openmt2001::ai_messages::offers::auction::activation::Call::RefreshPanel,
                )) => {
                    // Refresh from the completed session below: this callback
                    // runs while the simulation owns the controller borrow.
                    auction.as_mut().ok_or("Auction labels have no prepared scene")?.participants.clear();
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionActivation(
                    openmt2001::ai_messages::offers::auction::activation::Call::NormalSpeed,
                )) => Ok(0), // Session already changed scheduler.iterations to1.
                Event::Command(openmt2001::live_commands::Event::AuctionClosure(
                    openmt2001::ai_messages::offers::auction::closure::Call::CloseAnnouncement,
                )) => {
                    *auction_seconds = None;
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionClosure(
                    openmt2001::ai_messages::offers::auction::closure::Call::CategoryView(_),
                )) => {
                    //405260 refreshes the property panel; it does not move the
                    //camera. Closure already cleared category+6c before this.
                    omitted.insert(0x405260);
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionClosure(
                    openmt2001::ai_messages::offers::auction::closure::Call::CategoryStatus(_),
                )) => {
                    omitted.insert(0x41ed10);
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionClosure(
                    openmt2001::ai_messages::offers::auction::closure::Call::AudioReset4af140,
                )) => {
                    omitted.insert(0x4af140);
                    crate::transport_audio::submit(
                        &transport_audio,
                        crate::transport_audio::Operation::Reset,
                    )?;
                    crate::gameplay_sound::enqueue(
                        &gameplay_sound,
                        crate::gameplay_sound::Operation::Reset,
                    )?;
                    auction_audio::enqueue(&auction_audio, auction_audio::Operation::Reset)?;
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionClosure(
                    openmt2001::ai_messages::offers::auction::closure::Call::ReleaseAmbient(handle),
                ))
                | Event::Command(openmt2001::live_commands::Event::AuctionAdmission(
                    openmt2001::ai_messages::offers::auction::admission::Call::ReleaseAmbient(handle),
                )) => crate::gameplay_sound::request(
                    &gameplay_sound, crate::gameplay_sound::Operation::AuctionAmbientRelease(handle)),
                Event::Command(openmt2001::live_commands::Event::AuctionClosure(
                    openmt2001::ai_messages::offers::auction::closure::Call::Tutorial(_),
                )) if tutorial_state != 1 => Ok(0),
                Event::Command(openmt2001::live_commands::Event::PropertyOwnership(
                    openmt2001::category_ownership::Seam::BuildingAppearance { .. },
                )) => {
                    omitted.insert(0x485520);
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::PropertyTransferNotice {
                    category,
                    owner,
                    ..
                }) => {
                    let mut packet = [u32::MAX; 10];
                    packet[1] = u32::from(owner);
                    packet[2] = 0;
                    packet[7] = i32::from(category) as u32;
                    notices.push(
                        packet,
                        current_slot.ok_or("Notification local player unresolved")?,
                    );
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::ConstructionNotice {
                    building,
                    owner,
                    structure_kind,
                    business,
                    business_kind,
                    category,
                }) => {
                    notices.push(
                        [
                            u32::MAX,
                            owner,
                            1,
                            building,
                            structure_kind,
                            business,
                            business_kind,
                            category,
                            u32::MAX,
                            u32::MAX,
                        ],
                        current_slot.ok_or("Notification local player unresolved")?,
                    );
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::PropertyView(_))
                | Event::Command(openmt2001::live_commands::Event::PlayerMessageRoute {
                    route: openmt2001::player_messages::Route::PropertyRefresh { .. }, ..
                })
                | Event::Ai(openmt2001::ai_runtime::Call::MessageRoute {
                    route: openmt2001::player_messages::Route::PropertyRefresh { .. },
                    ..
                }) => {
                    // Core performs the original405260 category lock mutation
                    // before this property-panel refresh boundary.
                    omitted.insert(0x405260);
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::PlayerMessageRoute {
                    route: openmt2001::player_messages::Route::Broadcast, packet,
                })
                | Event::Ai(openmt2001::ai_runtime::Call::MessageRoute {
                    route: openmt2001::player_messages::Route::Broadcast, packet,
                }) if property_refresh_broadcast(&packet) => {
                    // 451720 first calls 405260 for kind 0x26, then sends
                    // the same packet to 48c1a0. Its 48c1c0 kind-0x26
                    // branch only clears the one-shot dispatcher flag.
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::DemolitionNotice {
                    owner, building, structure_kind, category,
                }) => {
                    notices.push([
                        u32::MAX, owner as i32 as u32, 3, u32::from(building),
                        structure_kind, u32::MAX, u32::MAX, category as i32 as u32,
                        u32::MAX, u32::MAX,
                    ], current_slot.ok_or("Demolition notification local player unresolved")?);
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionClosure(
                    openmt2001::ai_messages::offers::auction::closure::Call::RestorePresentation,
                )) => {
                    let result = crate::auction_bridge::request(
                        auction_renderer,
                        crate::auction_bridge::Operation::Restore,
                    )?;
                    time_runtime.lock().map_err(|_| "Time runtime poisoned")?.hide(1)?;
                    *auction = None;
                    *auction_actor_events.borrow_mut() = None;
                    bid_gate.set(false);
                    Ok(result)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionAdmission(
                    openmt2001::ai_messages::offers::auction::admission::Call::ShowAnnouncement,
                )) => {
                    //43be20 visibility is already retained by the command adapter.
                    //41d700 opens compact with empty seconds until its first update.
                    *auction_seconds = None;
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::AuctionAdmission(
                    openmt2001::ai_messages::offers::auction::admission::Call::CategoryStatus(_),
                )) => {
                    //41ed10 updates category text and disables/cancels its open
                    //planning/business panels. Status publishes the authoritative
                    //category+6c locks to the interface; original status label is
                    //still an explicit presentation omission.
                    omitted.insert(0x41ed10);
                    Ok(0)
                }
                // These slices belong to Hooks.commands. The authoritative
                // command cursor below publishes each scripted prompt once.
                Event::Script { .. } | Event::ScriptCommands(_) => Ok(0),
                Event::Command(openmt2001::live_commands::Event::EventCardNotice(notice)) => {
                    if notice.enabled {
                        prompts.push(prompt::card_request(
                            assets.image,
                            players
                                .get(notice.player)
                                .ok_or("card notice player outside roster")?,
                            notice,
                        )?);
                    }
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::EventCardDiagnostic {
                    kind,
                    used,
                }) => {
                    // Original diagnostic output only; the card decision already ran.
                    eprintln!("Event card decision: kind {kind}, used {used}");
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::EventCardTargeted(
                    openmt2001::event_cards::targeted::Call::Sound { actor_is_current },
                )) => crate::gameplay_sound::request(
                    &gameplay_sound,
                    crate::gameplay_sound::Operation::Targeted { actor_is_current }),
                // Reviewed original render-only state: animated texture tiles,
                // category camera-distance cache and two particle systems.
                // They are visible outstanding features, not
                // substitutes for simulation callbacks.
                Event::Tick(scheduler::Event::Helper(
                    address @ (0x450080 | 0x4eb5b0 | 0x4b5db0),
                )) => {
                    omitted.insert(address);
                    Ok(0)
                }
                Event::Tick(scheduler::Event::BeforeSixSound) => {
                    crate::era_audio::request(
                        &era_audio, crate::era_audio::Operation::BeforeSixCreate)
                }
                Event::TransportSound(call) => {
                    if !matches!(call.id, 0 | 7 | 8 | 11) {
                        omitted.insert(0x44f790);
                    }
                    crate::transport_audio::submit(
                        &transport_audio,
                        crate::transport_audio::Operation::Route(call),
                    )?;
                    Ok(0)
                }
                Event::EraMusic { era, repeat } => crate::era_audio::request(
                    &era_audio, crate::era_audio::Operation::Select { era, repeat }),
                Event::TransportEnvironment { call, position } => {
                    let distance = camera.into_iter().zip(position)
                        .map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt();
                    crate::transport_audio::submit(&transport_audio,
                        crate::transport_audio::Operation::Environment { call, distance })?;
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::Construction(
                    openmt2001::construction::Event::Appearance(_),
                ))
                | Event::Command(openmt2001::live_commands::Event::BusinessSale(
                    openmt2001::business_sale::Call::BuildingAppearance(_),
                ))
                | Event::Command(openmt2001::live_commands::Event::BusinessTakeover(
                    openmt2001::business_takeover::Call::BuildingAppearance(_),
                ))
                | Event::Command(openmt2001::live_commands::Event::EventCardTargeted(
                    openmt2001::event_cards::targeted::Call::BuildingAppearance { .. },
                )) => {
                    omitted.insert(0x485520);
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::BusinessSale(
                    openmt2001::business_sale::Call::BuildingPanel(_),
                ))
                | Event::Command(openmt2001::live_commands::Event::BusinessTakeover(
                    openmt2001::business_takeover::Call::BuildingPanel(_),
                )) => {
                    //407660's unconditional building+3e clear is already applied
                    //by the transaction. Selected building controls refresh with
                    //the next authoritative scene snapshot.
                    omitted.insert(0x407660);
                    Ok(0)
                }
                Event::AccountPosted(posting)
                | Event::Command(openmt2001::live_commands::Event::AccountPosted(posting)) =>
                    account_ui_post(posting, current_slot, |amount| {
                        ledger_ui_post(&mut cash_arrow.borrow_mut(), &gameplay_sound, true, amount,
                            assets.image.f32_at(0x525040)?)
                    }),
                Event::Command(openmt2001::live_commands::Event::Construction(
                    openmt2001::construction::Event::RefreshInterface,
                ))
                | Event::Command(openmt2001::live_commands::Event::EventCardTargeted(
                    openmt2001::event_cards::targeted::Call::RefreshInterface,
                ))
                | Event::Command(openmt2001::live_commands::Event::RefreshInterface)
                | Event::Command(openmt2001::live_commands::Event::Demolition(
                    openmt2001::demolition::Call::RefreshInterface,
                )) => Ok(0),
                Event::Command(openmt2001::live_commands::Event::Demolition(
                    openmt2001::demolition::Call::RefreshBuilding { .. },
                )) => {
                    // Core applied4075e0's +3c clear. Selected interface drawing
                    // is refreshed by the authoritative scene snapshot.
                    omitted.insert(0x4075e0);
                    Ok(0)
                }
                Event::Command(openmt2001::live_commands::Event::Business(
                    openmt2001::business_binding::Call::RefreshBuilding { .. },
                ))
                | Event::Command(openmt2001::live_commands::Event::Detach(
                    openmt2001::business_detach::Call::RefreshBuilding { .. },
                )) => {
                    omitted.insert(0x4450a0);
                    Ok(0)
                }
                Event::Pending(
                    openmt2001::live_commands::PendingCall::Refresh, None,
                ) => {
                    omitted.insert(0x41f410);
                    Ok(0)
                }
                Event::Pending(
                    openmt2001::live_commands::PendingCall::World(
                        openmt2001::pending_world::Event::Progress { group, .. },
                    ),
                    Some(record),
                ) => {
                    let position = std::array::from_fn(|axis| {
                        let at = 0x58 + axis * 4;
                        f32::from_le_bytes(record[at..at + 4].try_into().unwrap())
                    });
                    crate::transport_audio::insert_construction_source(
                        &mut construction_sounds, group, position, camera)?;
                    Ok(0)
                }
                Event::Ai(call) => ai_callback(call, ai_warnings),
                // Every other callback must be bound deliberately. In particular,
                // controller/transaction outcomes may not be faked with success.
                other => Err(format!("Live session requires {other:?}")),
            }
        });
        // Logic-scheduled tutorial callbacks can append Camera/Interface
        // commands before the next Status query. Consume them before moving
        // the shared command cursor past this tick's script effects.
        // These local cells include effects completed before a later raw-save
        // retirement. Commit them on both the ordinary and retired paths.
        self.cash_arrow = cash_arrow.into_inner();
        self.auction_actor_events = auction_actor_events.into_inner();
        self.policy.ai_event_gate = bid_gate.get();
        if autosave_retired {
            self.stop_retired_camera();
            return;
        }
        self.collect_script_prompts(session);
        let result = result.and_then(|()| {
            if let Some(auction) = &mut self.auction {
                auction.refresh_participants(session)?;
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                if let Err(error) = crate::transport_audio::submit(
                    &transport_audio,
                    crate::transport_audio::Operation::City(Box::new(
                        crate::transport_audio::CityInputs {
                            reciprocal_sum: session.road_traffic.proximity.reciprocal_sum,
                            nearest: session.road_traffic.proximity.nearest,
                            categories: session.world.city.categories,
                            construction: construction_sounds,
                            hour: session.world.clock.hour(),
                            tick: session.world.clock.timing[0],
                            previous_tick: block_previous_tick,
                            initial_random_state: block_random_state,
                        },
                    )),
                ) {
                    self.fail(error);
                }
            }
            Err(error) => self.fail(error),
        }
    }
    pub fn human_dialog_reply(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        reply: crate::human_dialog::Reply,
    ) -> Result<(), String> {
        if reply.callback == crate::game_options::QUIT_CALLBACK {
            return self.reply_options_quit(
                &mut session.catalog,
                session.campaign_profile.as_deref(),
                reply.button,
            );
        }
        if reply.callback == 0x48b490 {
            // Button and timeout may race for the same modal. The first reply
            // already initialized the original result script and dismissed it.
            if self.result_context == Some(reply.context) {
                return Ok(());
            }
            if !self.outcome_callback_seen || self.outcome_context != Some(reply.context) {
                return Err("Outcome reply lacks matching original modal".into());
            }
            if self.result_request.is_some() || self.result_completed {
                return Err("Outcome sequence was already requested".into());
            }
            let mut sequence = None;
            let mut characters = None;
            let mut music_address = None;
            session.respond_outcome(reply.context, assets, |response| {
                use openmt2001::outcome_notice::Response;
                match response {
                    Response::Characters { winner, loser } => characters = Some((winner, loser)),
                    Response::Sequence(value) => sequence = Some(value),
                    Response::Unlock(_) => (), // Session applied the original unlock.
                    Response::Music { address } => music_address = Some(address),
                }
                Ok(())
            })?;
            self.result_request = Some(ResultRequest {
                sequence: sequence.ok_or("Outcome response omitted result sequence")?,
                characters: characters.ok_or("Outcome response omitted actor characters")?,
                music_address: music_address.ok_or("Outcome response omitted music")?,
            });
            self.running = false;
            // Original result entry 4b1881 clears retained city sound handles
            // through 4af140 before winner/loser music starts.
            let _ = crate::transport_audio::submit(
                &self.transport_audio,
                crate::transport_audio::Operation::Reset,
            );
            let _ = crate::gameplay_sound::enqueue(
                &self.gameplay_sound,
                crate::gameplay_sound::Operation::Reset,
            );
            let _ = auction_audio::enqueue(&self.auction_audio, auction_audio::Operation::Reset);
            self.result_context = Some(reply.context);
            self.outcome_callback_seen = false;
            self.outcome_context = None;
        } else if reply.callback != 0 {
            session.respond_human_dialog(
                reply.callback,
                reply.button,
                reply.context,
                [
                    self.policy.scheduler.network_enabled,
                    self.policy.scheduler.network_override,
                ],
                |event| match event {
                    openmt2001::live_commands::Event::PlayerMessageRoute {
                        route: openmt2001::player_messages::Route::PropertyRefresh { .. },
                        ..
                    } => {
                        self.pending_presentation.insert(0x405260);
                        Ok(())
                    }
                    openmt2001::live_commands::Event::PlayerMessageRoute {
                        route: openmt2001::player_messages::Route::Broadcast,
                        packet,
                    } if property_refresh_broadcast(&packet) => Ok(()),
                    other => Err(format!("Human dialog reply requires {other:?}")),
                },
            )?;
        }
        if !self.policy.scheduler.network_enabled {
            self.policy.scheduler.iterations = 1;
        }
        //429d50 closes only after the button callback; it dispatches nonzero
        //slot+0c through the script VM before unlinking the old prompt.
        let script = session
            .hooks
            .script_prompt()
            .is_some_and(|r| r.dismissal_id == reply.context);
        if reply.context != 0 {
            session.dispatch_session_event(
                "InterfacePromptDismiss",
                reply.context as i32,
                assets,
                false,
            )?;
        }
        if script && reply.context == 0 {
            session.hooks.close_script_prompt(false);
        }
        Ok(())
    }
    /// Original 49e940 releases the modal for either answer. Only Yes calls
    /// 4ba1d0 with completed=0, won=0; it skips results and final statistics.
    pub(crate) fn reply_options_quit(
        &mut self,
        catalog: &mut openmt2001::level_catalog::Catalog,
        profile: Option<&std::path::Path>,
        button: u32,
    ) -> Result<(), String> {
        if button != 0 || self.result_completed {
            return Ok(());
        }
        if self.options_resume.is_none() || !self.game_active {
            return Err("Abandonment requires the active city's Options menu".into());
        }
        catalog.finish(false, 0)?;
        if let Some(path) = profile {
            crate::campaign_store::save_progress(catalog, path)?;
        }
        self.stats = None;
        self.result_request = None;
        self.result_completed = true;
        self.game_active = false;
        self.running = false;
        Ok(())
    }
    #[cfg(test)]
    pub fn result_sequence(&self) -> Option<openmt2001::outcome_notice::Sequence> {
        self.result_request.map(|request| request.sequence)
    }
    /// Consume the result hub's host effects before its event reply is visible.
    pub fn dispatch_result_event(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        name: &str,
    ) -> Result<(), String> {
        session.dispatch_result_event(name, assets)?;
        self.collect_script_prompts(session);
        Ok(())
    }
    fn result_review(&self, catalog: &openmt2001::level_catalog::Catalog) -> Option<ResultReview> {
        self.result_completed.then(|| {
            if self.stats.is_some() {
                ResultReview::Statistics
            } else {
                ResultReview::Menu(ResultMenu::for_level(
                    catalog
                        .selected_level()
                        .expect("completed result selection"),
                ))
            }
        })
    }
    /// Called only after the original result sequence has finished presenting.
    pub fn complete_result_sequence(
        &mut self,
        session: &mut Session<'_>,
    ) -> Result<openmt2001::outcome_notice::Sequence, String> {
        let end_requested = session
            .result_presentation()
            .and_then(|result| result.end_requested);
        self.collect_script_prompts(session);
        let stats = crate::stats_panel::State::show(
            session.world.objective.kind,
            false,
            session.roster.manager.players.len(),
        )?;
        // Validate the retained graph before committing progression.
        crate::stats_panel::Snapshot::from_session(session, &stats)?;
        self.commit_result_review(
            &mut session.catalog,
            session.campaign_profile.as_deref(),
            stats,
            end_requested,
        )
    }
    /// Retain the applied bit across I/O failure so persistence retries cannot
    /// increment attempts or unlock the next scenario twice. Closing the graph
    /// only changes presentation; it never passes through this commit again.
    fn commit_result_review(
        &mut self,
        catalog: &mut openmt2001::level_catalog::Catalog,
        profile: Option<&std::path::Path>,
        stats: crate::stats_panel::State,
        end_requested: Option<u32>,
    ) -> Result<openmt2001::outcome_notice::Sequence, String> {
        if self.failure.is_some() {
            return Err("Stopped result sequence cannot complete campaign".into());
        }
        if self.result_completed {
            return Err("Result sequence already completed".into());
        }
        let sequence = self
            .result_request
            .ok_or("No result sequence is active")?
            .sequence;
        crate::campaign_store::commit_result(
            catalog,
            profile,
            &mut self.result_applied,
            sequence,
            end_requested,
        )?;
        self.result_completed = true;
        self.result_request = None;
        self.game_active = false;
        self.running = false;
        self.stats = Some(stats);
        Ok(sequence)
    }
    fn interface_tick(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
    ) -> Result<(), String> {
        self.begin_presentation(session)?;
        self.event_list_opening = false;
        self.interface_ticks = self.interface_ticks.wrapping_add(1);
        session.advance_interface_tick(
            self.policy.scheduler.iterations,
            self.policy.calendar.suppress_rollover_query,
            |event| {
                use openmt2001::ai_messages::offers::auction::announcement::Call;
                match event {
                    Call::CreateSound => {
                        if self.auction_audio.is_some() {
                            auction_audio::request(
                                &self.auction_audio,
                                auction_audio::Operation::Create(auction_audio::Kind::Announcement),
                            )
                        } else {
                            self.pending_presentation.insert(0x4ad760);
                            Ok(0)
                        }
                    }
                    Call::ReleaseSound(handle) => auction_audio::request(
                        &self.auction_audio,
                        auction_audio::Operation::Release(handle),
                    ),
                    Call::Diagnostic(address) => {
                        eprintln!("Auction announcement diagnostic {address:08x}");
                        Ok(0)
                    }
                    Call::ClearSeconds => {
                        self.auction_seconds = None;
                        Ok(0)
                    }
                    Call::WriteSeconds(seconds) => {
                        self.auction_seconds = Some(seconds);
                        Ok(0)
                    }
                }
            },
        )?;
        if self.policy.calendar.suppress_rollover_query
            || session.world.city.pause.bytes[3] as i8 > 0
        {
            return Ok(());
        }
        let mode = if !self.game_active || self.stats.is_some() {
            4
        } else {
            u32::from(self.auction.is_some())
        };
        self.prompt_updates = self.prompt_updates.wrapping_add(u64::from(
            crate::time_panel::instance_passes(mode, self.policy.scheduler.iterations)[0],
        ));
        //465490 switches GUI8b1050 to1 before any auction widget is shown.
        //Normal GUI0/2/3 share the other manager. Hidden Time widgets do not
        //receive428dc0, and their Chance caches survive subsequent city loads.
        for _ in 0..self.policy.scheduler.iterations {
            self.notices.tick();
            self.market_share.interface_tick();
        }
        let p = session
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == session.roster.status.current_player)
            .ok_or("Time current player missing")?;
        let entry = crate::card_panel::Entry::player(
            p,
            session.world.city.auction.controller.reference != 0
                && session.world.city.auction.controller.record[0x7c] != 0,
        )?;
        let debt = *session
            .world
            .objective
            .cash_statuses
            .get(usize::from(p.bytes[0x1bc]))
            .ok_or("Time debt status outside roster")?
            != 0;
        let cash = current_cash(session)?;
        let mut runtime = self
            .time_runtime
            .lock()
            .map_err(|_| "Time runtime poisoned")?;
        for (instance, passes) in
            crate::time_panel::instance_passes(mode, self.policy.scheduler.iterations)
                .into_iter()
                .enumerate()
        {
            if runtime.widgets[instance].visible {
                for _ in 0..passes {
                    runtime.chance_visuals[instance].tick(entry);
                    runtime.tick(cash, debt, instance)?;
                }
            }
        }
        drop(runtime);
        if !self.auction.as_ref().is_some_and(|a| a.visible) {
            return Ok(());
        }
        let player = local_player(session)?;
        let mut controls = std::mem::take(&mut self.auction.as_mut().unwrap().controls);
        let result = (|| {
            for _ in 0..self.policy.scheduler.iterations {
                let bidder = bidder_view(session, player)?;
                if controls.timer_visible {
                    let no_sale = session.world.city.auction.controller.record[0x70] != 0;
                    controls.timer.tick(
                        bidder.current,
                        player,
                        bidder.winner,
                        no_sale,
                        |call| {
                            use crate::auction_controls::TimerCall;
                            match call {
                                TimerCall::CreateSound => {
                                    if self.auction_audio.is_some() {
                                        auction_audio::request(
                                            &self.auction_audio,
                                            auction_audio::Operation::Create(
                                                auction_audio::Kind::TurnTimer,
                                            ),
                                        )
                                    } else {
                                        self.pending_presentation.insert(0x4ad760);
                                        Ok(0)
                                    }
                                }
                                TimerCall::ReleaseSound(handle) => auction_audio::request(
                                    &self.auction_audio,
                                    auction_audio::Operation::Release(handle),
                                ),
                                TimerCall::ClearText => {
                                    controls.timer_text = None;
                                    Ok(0)
                                }
                                TimerCall::WriteSeconds(seconds) => {
                                    controls.timer_text =
                                        Some(crate::auction_controls::TimerText::Seconds(seconds));
                                    Ok(0)
                                }
                                TimerCall::Label { no_sale } => {
                                    controls.timer_text =
                                        Some(crate::auction_controls::TimerText::Label { no_sale });
                                    Ok(0)
                                }
                            }
                        },
                    )?;
                }
                controls.tick(bidder, player, session.tutorial_state, |call| {
                    use crate::auction_controls::TickCall;
                    match call {
                        TickCall::Withdraw(participant) => {
                            self.apply_auction_action(
                                session,
                                assets,
                                openmt2001::ai_messages::offers::auction::Command {
                                    action: 2,
                                    participant: u32::from(participant),
                                    amount: 0,
                                },
                            )?;
                            Ok(Some(bidder_view(session, player)?))
                        }
                        TickCall::Tutorial(control) if session.tutorial_state == 1 => {
                            self.dispatch_tutorial_callback(session, assets, control)?;
                            Ok(None)
                        }
                        _ => Ok(None),
                    }
                })?;
            }
            Ok(())
        })();
        self.auction.as_mut().unwrap().controls = controls;
        result
    }
    pub(crate) fn apply_auction_action(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        command: openmt2001::ai_messages::offers::auction::Command,
    ) -> Result<(), String> {
        let tutorial_state = session.tutorial_state;
        let bid_gate = std::cell::Cell::new(self.policy.ai_event_gate);
        let result = session.apply_auction_action(command, assets, |event| match event {
            openmt2001::live_commands::Event::AuctionTimeShow { cash } => {
                self.time_runtime
                    .lock()
                    .map_err(|_| "Time runtime poisoned")?
                    .show(cash, 1)?;
                Ok(0)
            }
            openmt2001::live_commands::Event::AuctionAction(call) => auction_action(
                call,
                &mut self.auction,
                &mut self.pending_presentation,
                tutorial_state,
                &self.auction_actor_events,
                &bid_gate,
            ),
            other => Err(format!("Auction action requires {other:?}")),
        });
        self.policy.ai_event_gate = bid_gate.get();
        result?;
        if let Some(auction) = &mut self.auction {
            auction.refresh_participants(session)?;
        }
        Ok(())
    }
    pub fn auction_button(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        button: u32,
    ) -> Result<(), String> {
        if self.failure.is_some() {
            return Err("Session stopped at an incomplete boundary".into());
        }
        // A queued release may outlive the panel it came from. Reject it before
        // reading player/auction state belonging to the former presentation.
        if self.auction.as_ref().is_none_or(|auction| !auction.visible) {
            return Ok(());
        }
        if button == 5 {
            if self.policy.scheduler.network_enabled {
                return Ok(());
            }
            // 414730 message12 index5 enters 414a31 and toggles565b9c+858.
            // Its original input child is a textureless 16px title-right strip.
            let next = !self.policy.ai_event_gate;
            if let Some(sender) = &self.auction_actor_events {
                sender
                    .send(crate::auction_bridge::ActorEvent::BidVoiceGate(next))
                    .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
            }
            self.policy.ai_event_gate = next;
            self.auction.as_mut().unwrap().bid_voice_gate = next;
            return Ok(());
        }
        if self.auction.as_ref().is_none_or(|auction| {
            button > 4 || auction.controls.buttons_enabled & (1 << button) == 0
        }) {
            return Ok(());
        }
        let player = local_player(session)?;
        let view = bidder_view(session, player)?;
        let auction = self.auction.as_mut().unwrap();
        let mut controls = std::mem::take(&mut auction.controls);
        let allowance = i32::from_le_bytes(assets.image.at(0x541c7c, 4)?.try_into().unwrap());
        let result = controls.button(button, view.current, player, view.price, |call| {
            use crate::auction_controls::Call;
            match call {
                Call::Affordable(amount) => Ok(openmt2001::construction::affordable(
                    session.world.ledgers.players[usize::from(player)].cash,
                    allowance,
                    amount,
                    false,
                )),
                Call::Action(command) => self
                    .apply_auction_action(session, assets, command)
                    .map(|_| true),
                Call::Tutorial(control) if session.tutorial_state == 1 => {
                    self.dispatch_tutorial_callback(session, assets, control)?;
                    Ok(true)
                }
                _ => Ok(true),
            }
        });
        self.auction.as_mut().unwrap().controls = controls;
        if let Err(error) = &result {
            self.fail(error.clone());
        }
        result
    }
    /// Dispatch registered activity, interface and logic tasks in native
    /// registration order. Publish records around each callback so script
    /// pause/group changes affect this same dispatch, including the next task.
    pub fn frame(
        &mut self,
        session: &mut Session<'_>,
        assets: &CommandAssets<'_, '_>,
        elapsed: i32,
    ) {
        if self.stop_retired_camera() || self.failure.is_some() || self.result_completed {
            return;
        }
        // Network ingress must remain live while group2 logic timers wait.
        if let Err(error) = session.pump_connected_commands(&mut self.policy) {
            self.fail(error);
            return;
        }
        if session.tutorial_state == 1 {
            self.collect_script_prompts(session);
            if let Some(completed) = self.tutorial_end_requested {
                self.running = false;
                if completed && !self.tutorial_completed {
                    // Native 4ba2bb selects the tutorial-specific mark-complete
                    // branch; Catalog::finish ignores the Win/Lose distinction.
                    if let Err(error) = crate::campaign_store::commit_result(
                        &mut session.catalog,
                        session.campaign_profile.as_deref(),
                        &mut self.tutorial_applied,
                        openmt2001::outcome_notice::Sequence::Win,
                        Some(1),
                    ) {
                        self.fail(error);
                    } else {
                        self.tutorial_completed = true;
                    }
                }
                return;
            }
        }
        if session.tutorial_state == 1 && !self.tutorial_started {
            self.tutorial_started = true;
            if let Err(error) = session.enter_tutorial_level(assets) {
                self.fail(error);
                return;
            }
            self.collect_script_prompts(session);
        }
        if self.stop_retired_camera() || self.failure.is_some() {
            return;
        }
        if self.comparison.is_none()
            && let Err(error) = self.refresh_comparison(session, assets)
        {
            self.fail(error);
            return;
        }
        let indices: Vec<_> = session
            .hooks
            .timers
            .iter()
            .enumerate()
            .filter_map(|(index, task)| {
                matches!(
                    u32::from_le_bytes(task[16..20].try_into().unwrap()),
                    0x4baa10 | 0x4ba580 | 0x481680
                )
                .then_some(index)
            })
            .collect();
        if indices.len() != 3
            || [0x4baa10, 0x4ba580, 0x481680].into_iter().any(|address| {
                indices
                    .iter()
                    .filter(|i| {
                        u32::from_le_bytes(session.hooks.timers[**i][16..20].try_into().unwrap())
                            == address
                    })
                    .count()
                    != 1
            })
        {
            self.fail("Missing unique original activity/interface/logic timer registration".into());
            return;
        }
        // The omitted presentation row does not accumulate or dispatch here.
        // Keep original indices/order and stop on unsupported structural
        // edits rather than silently dropping/reidentifying task registrations.
        let identities: Vec<[u8; 4]> = session
            .hooks
            .timers
            .iter()
            .map(|task| task[16..20].try_into().unwrap())
            .collect();
        // The host's player pause freezes the logic accumulator as before;
        // interface task time continues. Script pauses instead change the
        // actual task enabled byte through the retained native group mask.
        let indices: Vec<_> = indices
            .into_iter()
            .filter(|index| {
                self.running
                    || u32::from_le_bytes(session.hooks.timers[*index][16..20].try_into().unwrap())
                        != 0x481680
            })
            .collect();
        let central = indices
            .iter()
            .position(|index| {
                u32::from_le_bytes(session.hooks.timers[*index][16..20].try_into().unwrap())
                    == 0x4ba580
            })
            .expect("unique central registration was validated");
        let mut tasks = indices
            .iter()
            .map(|index| session.hooks.timers[*index])
            .collect();
        let mut structural_error = false;
        // timers::dispatch has a String-only callback API. This local typed
        // result owns cancellation classification; no returned error text or
        // arbitrary failure is tested/suppressed based on the retirement flag.
        let mut cancelled = false;
        let result = openmt2001::timers::dispatch(&mut tasks, elapsed, |_, address, tasks| {
            for (i, index) in indices.iter().enumerate() {
                session.hooks.timers[*index] = tasks[i];
            }
            let callback_result = if self.stop_retired_camera() {
                Err(camera_bridge::RequestError::Retired)
            } else {
                match address {
                    // 4baa10 delivers event11 every two seconds, independent
                    // of logic speed, rendering, and asynchronous status polls.
                    0x4baa10 => self
                        .refresh_comparison(session, assets)
                        .map_err(camera_bridge::RequestError::Failed),
                    // All successful UI early returns converge here. One tail
                    // per actual central callback, never scheduler.iterations.
                    0x4ba580 => self
                        .interface_tick(session, assets)
                        .map_err(camera_bridge::RequestError::Failed)
                        .and_then(|()| self.camera_central_tail(session)),
                    0x481680 => {
                        self.tick(session, assets);
                        if let Some(error) = &self.failure {
                            Err(camera_bridge::RequestError::Failed(error.clone()))
                        } else if self.stop_retired_camera() {
                            Err(camera_bridge::RequestError::Retired)
                        } else {
                            Ok(())
                        }
                    }
                    _ => Err(camera_bridge::RequestError::Failed(
                        "Live interface/logic timer callback changed".into(),
                    )),
                }
            };
            if session.hooks.timers.len() != identities.len()
                || session
                    .hooks
                    .timers
                    .iter()
                    .zip(&identities)
                    .any(|(task, address)| task[16..20] != address[..])
            {
                structural_error = true;
                return Err(
                    "Live timer scope cannot remove/reorder/replace/append registrations".into(),
                );
            }
            for (i, index) in indices.iter().enumerate() {
                tasks[i] = session.hooks.timers[*index];
            }
            match callback_result {
                Err(camera_bridge::RequestError::Failed(error)) => return Err(error),
                Err(camera_bridge::RequestError::Retired) => {
                    self.stop_retired_camera();
                    cancelled = true;
                    return Err("Explicit camera retirement interrupted timer dispatch".into());
                }
                Ok(()) => {}
            }
            if let Some(error) = &self.failure {
                return Err(error.clone());
            }
            if self.stop_retired_camera() {
                cancelled = true;
                return Err("Explicit camera retirement interrupted timer dispatch".into());
            }
            Ok(())
        });
        if !structural_error {
            for (i, index) in indices.iter().enumerate() {
                session.hooks.timers[*index] = tasks[i];
            }
        }
        match result {
            Err(_) if cancelled => {
                self.stop_retired_camera();
            }
            Err(error) => self.fail(error),
            Ok(ratios) => {
                if self.stop_retired_camera() {
                    return;
                }
                // Existing per-logic audio remains unchanged. This is the end
                // of the currently owned dispatch, not a claim that every
                // original4afbf0 virtual service has been joined here.
                let result = self
                    .request_camera(camera_bridge::Operation::CurrentPose)
                    .and_then(|_| {
                        self.request_camera(camera_bridge::Operation::Sample {
                            ratio_bits: ratios[central].to_bits(),
                        })
                    });
                if let Err(error) = result {
                    self.camera_error(error);
                }
            }
        }
    }
}

/// Test adapter for the same two envelopes consumed by the live match arm.
#[cfg(test)]
fn account_event_ui_post(
    event: &fresh_city::tick::Event<'_>,
    current_slot: Option<u8>,
    post: impl FnMut(f32) -> Result<u32, String>,
) -> Option<Result<u32, String>> {
    use fresh_city::tick::Event;
    use openmt2001::live_commands;
    let posting = match event {
        Event::AccountPosted(posting)
        | Event::Command(live_commands::Event::AccountPosted(posting)) => *posting,
        _ => return None,
    };
    Some(account_ui_post(posting, current_slot, post))
}

/// Route an already-applied posting without touching balances or netting amounts.
fn account_ui_post(
    posting: openmt2001::economy::AccountPosting,
    current_slot: Option<u8>,
    mut post: impl FnMut(f32) -> Result<u32, String>,
) -> Result<u32, String> {
    if current_slot.map(usize::from) == Some(posting.player) {
        post(posting.amount)
    } else {
        Ok(0)
    }
}

/// Original48a5a0 presentation after the account has already been posted.
fn ledger_ui_post(
    cash_arrow: &mut crate::cash_arrow::State,
    sound: &Option<std::sync::mpsc::Sender<crate::gameplay_sound::Request>>,
    current: bool,
    amount: f32,
    threshold: f32,
) -> Result<u32, String> {
    ledger_ui_post_with(current, amount, threshold, |call| {
        ledger_ui_call(cash_arrow, sound, call)
    })
}

fn ledger_ui_post_with(
    current: bool,
    amount: f32,
    threshold: f32,
    mut call: impl FnMut(openmt2001::category_ownership::Seam) -> Result<u32, String>,
) -> Result<u32, String> {
    use openmt2001::category_ownership::Seam;
    if !current {
        return Ok(0);
    }
    let sale = if amount > threshold {
        true
    } else if amount < threshold || amount.is_nan() {
        false
    } else {
        return Ok(0);
    };
    let pulse = Seam::NativeUiCall {
        address: if sale { 0x4c6290 } else { 0x4c62d0 },
        args: [0; 6],
        argument_count: 0,
    };
    let cue = Seam::NativeUiCall {
        address: 0x4ad760,
        args: [if sale { 0x541cc8 } else { 0x541cb4 }, 0, 0, 0, 1, 0x3c],
        argument_count: 6,
    };
    if sale {
        call(cue)?;
        call(pulse)
    } else {
        call(pulse)?;
        call(cue)
    }
}

fn ledger_ui_call(
    cash_arrow: &mut crate::cash_arrow::State,
    sound: &Option<std::sync::mpsc::Sender<crate::gameplay_sound::Request>>,
    call: openmt2001::category_ownership::Seam,
) -> Result<u32, String> {
    use openmt2001::category_ownership::Seam;
    match call {
        Seam::NativeUiCall {
            address: 0x4ad760,
            args,
            argument_count: 6,
        } => {
            let sale = match args {
                [0x541cc8, 0, 0, 0, 1, 0x3c] => true,
                [0x541cb4, 0, 0, 0, 1, 0x3c] => false,
                _ => return Err("property transfer sound call changed".into()),
            };
            crate::gameplay_sound::enqueue(
                sound,
                crate::gameplay_sound::Operation::Ledger { sale },
            )?;
            Ok(0)
        }
        Seam::NativeUiCall {
            address: address @ (0x4c6290 | 0x4c62d0),
            args: [0, 0, 0, 0, 0, 0],
            argument_count: 0,
        } => {
            cash_arrow.start(address == 0x4c6290);
            Ok(0)
        }
        _ => Err("unsupported property transfer presentation call".into()),
    }
}

fn local_player(session: &Session<'_>) -> Result<u8, String> {
    session
        .roster
        .manager
        .players
        .iter()
        .find(|p| p.reference == session.roster.status.current_player)
        .map(|p| p.bytes[0x1bc])
        .ok_or("Auction current player missing".into())
}
fn bidder_view(
    session: &Session<'_>,
    player: u8,
) -> Result<crate::auction_controls::Bidder, String> {
    if player >= 6 {
        return Err("Auction local player exceeds six participants".into());
    }
    let record = &session.world.city.auction.controller.record;
    let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    Ok(crate::auction_controls::Bidder {
        current: word(0x58),
        winner: word(0x60),
        price: word(0x6c),
        passed: record[usize::from(player) * 12 + 0x10],
        withdrawn: record[usize::from(player) * 12 + 0x12],
    })
}
fn current_cash(session: &Session<'_>) -> Result<f32, String> {
    let slot = session
        .roster
        .manager
        .players
        .iter()
        .position(|player| player.reference == session.roster.status.current_player)
        .ok_or("Time current player missing")?;
    session
        .world
        .ledgers
        .players
        .get(slot)
        .map(|player| player.cash)
        .ok_or_else(|| "Time current account missing".into())
}

pub(crate) fn auction_action(
    call: openmt2001::ai_messages::offers::auction::Call,
    auction: &mut Option<Auction>,
    omitted: &mut std::collections::BTreeSet<u32>,
    tutorial_state: u8,
    actor_events: &Option<std::sync::mpsc::Sender<crate::auction_bridge::ActorEvent>>,
    bid_gate: &std::cell::Cell<bool>,
) -> Result<u32, String> {
    use openmt2001::ai_messages::offers::auction::Call;
    let auction = auction
        .as_mut()
        .ok_or("Auction presentation action arrived without a prepared scene")?;
    match call {
        Call::ShowPanel(0x565b9c) => {
            auction.visible = true;
            // 4140b0 resets the hidden bid-voice/AI gate on panel reopen.
            bid_gate.set(false);
            auction.bid_voice_gate = false;
            if let Some(sender) = actor_events {
                sender
                    .send(crate::auction_bridge::ActorEvent::BidVoiceGate(false))
                    .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
            }
        }
        Call::ShowPanel(0x565b94) => (), // Original top panel is part of the renderer.
        Call::ShowPanel(address @ (0x565ba4 | 0x565ba0)) => {
            omitted.insert(address);
        }
        Call::ShowPanel(address) => return Err(format!("Unmapped auction panel{address:08x}")),
        Call::ParticipantEffect { participant, kind } => {
            if let Some(sender) = actor_events {
                sender
                    .send(crate::auction_bridge::ActorEvent::Effect { participant, kind })
                    .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
            } else {
                omitted.insert(0x465980);
            }
        }
        Call::Tutorial(_) if tutorial_state == 1 => {
            return Err("Auction tutorial event needs its tutorial session".into());
        }
        Call::Tutorial(_) => (),
        Call::RefreshTurn(_) => {
            // Invalidate the old text; the host refreshes from the completed
            // controller after the simulation releases its mutable borrow.
            auction.participants.clear();
        }
        Call::WithdrawNotice(participant) => {
            if let Some(sender) = actor_events {
                sender
                    .send(crate::auction_bridge::ActorEvent::WithdrawNotice(
                        participant,
                    ))
                    .map_err(|_| "Auction actor renderer disconnected".to_owned())?;
            } else {
                omitted.insert(0x464769);
            }
        }
    }
    Ok(0)
}

///Same source FISTP-low-word conversion using the public portable arithmetic.
///Nonfinite/indefinite conversions produce low word0; no private economy API.
fn picking_ftol(value: f64) -> i32 {
    openmt2001::x87::Sum::from_f64(value).map_or(0, |number| number.ftol_low_word())
}
fn apply_source_picking_command(
    process: &process_runtime::Handle,
    command: &level_script::Command,
    arguments: &[Value],
) -> Result<(), String> {
    use process_runtime::PickingWrite;
    let target = || match arguments.first() {
        Some(Value::Number(value)) => Some(picking_ftol(*value)),
        _ => None,
    };
    let write = match command {
        level_script::Command::TutorialDisableAllPicking => PickingWrite::DisableAll,
        level_script::Command::TutorialEnableAllPicking => PickingWrite::EnableAll,
        level_script::Command::TutorialDisableCameraControl => PickingWrite::CameraDisable,
        level_script::Command::TutorialEnableBlockPicking => PickingWrite::Block(target()),
        level_script::Command::TutorialEnableBuildingPicking => PickingWrite::Building(target()),
        _ => return Ok(()),
    };
    process.picking_write(write).map_err(|e| e.to_string())
}
fn tutorial_pick_target(arguments: &[Value]) -> Result<Option<u32>, String> {
    Ok(match arguments.first() {
        Some(Value::Number(value)) => match picking_ftol(*value) {
            -1 => None,
            value => Some(value as u32),
        },
        _ => None,
    })
}

pub(crate) fn script_request(
    arguments: &[Value],
    tutorial: bool,
    continue_button: bool,
) -> Result<prompt::Request, String> {
    let number = |at: usize| match arguments.get(at) {
        Some(Value::Number(value)) => openmt2001::x87::Sum::from_f64(*value)
            .map(|v| v.ftol_low_word() as u32)
            .map_err(str::to_owned),
        None | Some(Value::Nil) if at >= 2 => Ok(0),
        _ => Err("Prompt requires C-coerced numeric context/value".to_owned()),
    };
    let field = |at| match arguments.get(at) {
        Some(Value::Number(_)) => Ok((number(at)?, None)),
        Some(Value::Bytes(bytes)) => {
            // Validate safe formatting now; row0 mapping is deferred to rendering.
            prompt::literal_glyphs(bytes, &(0..224).map(|i| i as u8).collect::<Vec<_>>())?;
            Ok((0, Some(std::sync::Arc::<[u8]>::from(bytes.clone()))))
        }
        _ => Err("Prompt title/body must be numeric or convertible to string".to_owned()),
    };
    let (title, literal_title) = field(0)?;
    let (body, literal_body) = field(1)?;
    Ok(prompt::Request {
        script: Some(prompt::ScriptText {
            title: literal_title,
            body: literal_body,
            format_value: number(3)?,
            timeout: !tutorial,
        }),
        dialog: Some(crate::human_dialog::Dialog {
            title,
            body,
            buttons: [1707, 0],
            button_count: usize::from(continue_button),
            context: number(2)?,
            context_written: true,
            timeout: !tutorial,
            timeout_written: tutorial,
            ..Default::default()
        }),
        card: None,
        title,
        body,
        dismissal_id: number(2)?,
    })
}

fn human_dialog_ui(
    prompts: &mut Vec<prompt::Request>,
    address: u32,
    arguments: [u32; 4],
    count: usize,
    name: Option<[u8; 96]>,
) -> Result<(), String> {
    if address == 0x43be20 && count == 0 {
        prompts.push(prompt::Request {
            script: None,
            title: 0,
            body: 0,
            dismissal_id: 0,
            card: None,
            dialog: Some(crate::human_dialog::Dialog::default()),
        });
        return Ok(());
    }
    prompts
        .iter_mut()
        .rev()
        .find_map(|r| r.dialog.as_mut())
        .ok_or("Human modal write has no allocated prompt")?
        .ui(address, arguments, count, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_quit_menu_replies_preserve_pause_or_abandon_once() {
        use crate::{game_options, human_dialog};
        use openmt2001::level_catalog::{Catalog, Labels, Registration};
        // Campaign, tutorial, custom and offline skirmish keep their ordinary
        // end/return route; abandonment never increments attempts or unlocks.
        for (id, target) in [
            (200001, ResultMenu::Scenarios),
            (100001, ResultMenu::Scenarios),
            (900001, ResultMenu::Main),
            (500001, ResultMenu::Main),
        ] {
            for player_paused in [false, true] {
                for button in [0, 1] {
                    let mut catalog = Catalog::default();
                    catalog
                        .register(
                            Registration {
                                id,
                                directory: b"fixture".to_vec(),
                                labels: Labels::Localized {
                                    title: 1,
                                    description: 2,
                                },
                                checksum: 0,
                            },
                            true,
                            true,
                        )
                        .unwrap();
                    let before_levels = catalog.levels.clone();
                    let mut host = Host::new([0.; 3]);
                    host.game_active = true;
                    host.set_running(true);
                    let mut pause = openmt2001::pause::State::default();
                    let mut timers = openmt2001::timers::startup_tasks([0; 20]);
                    if player_paused {
                        host.toggle_player_pause(&mut pause, &mut timers, |_, _| Ok(()))
                            .unwrap();
                    }
                    let before_options = (pause.clone(), timers.clone());
                    host.options(&mut pause, &mut timers, true).unwrap();
                    host.policy.scheduler.iterations = 4;
                    let before_quit = (pause.clone(), timers.clone());
                    let assets = game_options::Assets::test_menu();
                    let mut menu = game_options::View::default();
                    menu.open(true);
                    assert_eq!(menu.mouse(&assets, Some([65., 5.]), true), None);
                    assert_eq!(
                        menu.mouse(&assets, Some([65., 5.]), false),
                        Some(game_options::Button::Quit)
                    );
                    assert!(host.game_active && !host.running && menu.open);
                    let request = menu.quit_prompt.request().unwrap();
                    assert_eq!((request.title, request.body), (1763, 1764));
                    assert_eq!(request.dialog.unwrap().buttons, [1636, 1637]);
                    assert!(!request.dialog.unwrap().timeout);
                    // Past the original normal prompt's 1000-tick timeout.
                    for _ in 0..1050 {
                        assert_eq!(menu.quit_prompt.tick(), None);
                    }
                    assert!(menu.quit_prompt.take_dialog_replies().is_empty());
                    let buttons = [[0., 0., 60., 24.], [80., 0., 140., 24.]];
                    menu.quit_prompt.cursor([buttons[button][0] + 1., 1.]);
                    menu.quit_prompt.dialog_mouse(true, &buttons);
                    menu.quit_prompt.dialog_mouse(false, &buttons);
                    let replies = menu.quit_prompt.take_dialog_replies();
                    assert_eq!(
                        replies,
                        vec![human_dialog::Reply {
                            callback: game_options::QUIT_CALLBACK,
                            context: 0,
                            button: button as u32,
                        }]
                    );
                    host.reply_options_quit(&mut catalog, None, replies[0].button)
                        .unwrap();
                    menu.quit_prompt.finish_dismiss(vec![]);
                    assert!(menu.quit_prompt.request().is_none());
                    assert_eq!(catalog.levels, before_levels);
                    assert_eq!((pause.clone(), timers.clone()), before_quit);
                    assert_eq!(host.policy.scheduler.iterations, 4);
                    if button == 1 {
                        assert!(host.game_active && menu.open);
                        assert_eq!(host.result_review(&catalog), None);
                        // Continue remains usable and restores the earlier hold.
                        assert_eq!(menu.mouse(&assets, Some([5., 5.]), true), None);
                        assert_eq!(
                            menu.mouse(&assets, Some([5., 5.]), false),
                            Some(game_options::Button::Continue)
                        );
                        menu.close();
                        host.options(&mut pause, &mut timers, false).unwrap();
                        assert!(host.running);
                        assert_eq!(host.player_paused(), player_paused);
                        assert_eq!((pause, timers), before_options);
                    } else {
                        assert!(!host.game_active && !host.running);
                        assert_eq!(
                            host.result_review(&catalog),
                            Some(ResultReview::Menu(target))
                        );
                        // A repeated callback cannot persist, award, or end again.
                        host.reply_options_quit(&mut catalog, Some(std::path::Path::new("")), 0)
                            .unwrap();
                        assert_eq!(catalog.levels, before_levels);
                        assert!(host.result_request.is_none());
                        host.set_running(true);
                        assert!(!host.running);
                    }
                }
            }
        }
        let desktop = crate::menu::desktop_quit::dialog();
        assert_eq!(
            (desktop.title, desktop.body, desktop.callback),
            (1765, 1766, 0x495670)
        );
    }
    #[test]
    fn scenario_reminder_open_and_dismiss_preserve_player_pause() {
        let mut level = openmt2001::level_catalog::Level {
            bytes: [0; openmt2001::level_catalog::RECORD_SIZE],
        };
        level.bytes[0x87c] = 1;
        level.bytes[1] = 1;
        level.bytes[0x44..0x48].copy_from_slice(&100u32.to_le_bytes());
        level.bytes[0x854..0x858].copy_from_slice(&101u32.to_le_bytes());
        for paused in [false, true] {
            let mut host = Host::new([0.; 3]);
            host.set_running(true);
            let mut pause = openmt2001::pause::State::default();
            let mut timers = openmt2001::timers::startup_tasks([0; 20]);
            if paused {
                host.toggle_player_pause(&mut pause, &mut timers, |_, _| Ok(()))
                    .unwrap();
            }
            let before = (
                pause.clone(),
                timers.clone(),
                host.running,
                host.player_paused(),
            );
            host.policy.scheduler.iterations = 4;
            host.scenario_reminder(&level, 6);
            let mut ui = prompt::State::default();
            ui.submit(host.pending_prompts.pop().unwrap());
            assert_eq!(host.policy.scheduler.iterations, 1);
            ui.dialog_button(0);
            assert_eq!(
                ui.take_dialog_replies(),
                vec![crate::human_dialog::Reply {
                    callback: 0,
                    context: 0,
                    button: 0
                }]
            );
            ui.finish_dismiss(vec![]);
            assert!(ui.request().is_none());
            assert_eq!((pause, timers, host.running, host.player_paused()), before);
        }
    }

    #[test]
    fn auction_toggle_reopen_resets_indicator_and_voice_gate() {
        use openmt2001::ai_messages::offers::auction::Call;
        let mut auction = Some(Auction {
            bid_voice_gate: true,
            ..Default::default()
        });
        let gate = std::cell::Cell::new(true);
        let (sender, receiver) = std::sync::mpsc::channel();
        auction_action(
            Call::ShowPanel(0x565b9c),
            &mut auction,
            &mut Default::default(),
            0,
            &Some(sender),
            &gate,
        )
        .unwrap();
        let auction = auction.unwrap();
        assert!(auction.visible);
        assert!(!auction.bid_voice_gate);
        assert!(!gate.get());
        assert!(matches!(
            receiver.try_recv().unwrap(),
            crate::auction_bridge::ActorEvent::BidVoiceGate(false)
        ));
    }
    #[test]
    fn account_post_feedback_filters_and_preserves_each_posting_order() {
        use openmt2001::economy::{
            self, BusinessDayParameters, PlayerAccount, ResidentAccounts, Utilities,
        };
        use openmt2001::{category_ownership::Seam, economy::AccountPosting};
        let mut records = vec![[0; 0x220]; 600];
        let mut parameters: Vec<_> = (0..600).map(|_| None).collect();
        for (slot, tenant, landlord, rent) in [
            (0, 1, 2, 75u16),
            (3, 2, 1, 125),
            (5, 1, 2, 0),
            (8, 1, 1, 900),
            (12, 1, 2, 50),
        ] {
            records[slot][..4].copy_from_slice(&2u32.to_le_bytes());
            records[slot][10] = tenant;
            records[slot][0x18..0x1a].copy_from_slice(&rent.to_le_bytes());
            records[slot][0x21c] = 1;
            parameters[slot] = Some(BusinessDayParameters {
                capacity: 0,
                catalog_mask: 0,
                prices: Vec::new(),
                utility_scalar: 0,
                utility_rates: [0.; 4],
                landlord,
            });
        }
        let mut players: Vec<_> = (0..2)
            .map(|_| {
                let mut player = PlayerAccount::from_runtime(&[0; 0x1b8]).unwrap();
                player.cash = 1000.;
                player
            })
            .collect();
        let postings = economy::update_daily_businesses(
            &mut records,
            &parameters,
            &mut ResidentAccounts {
                active_count: 0,
                nodes: Vec::new(),
                accounts: Vec::new(),
            },
            &mut players,
            &mut Utilities::new([0; 4]),
        )
        .unwrap();
        let mut calls = Vec::new();
        for posting in postings.iter().copied() {
            account_ui_post(posting, Some(0), |amount| {
                ledger_ui_post_with(true, amount, 0., |call| {
                    calls.push(call);
                    Ok(0)
                })
            })
            .unwrap();
        }
        let pulse = |credit| Seam::NativeUiCall {
            address: if credit { 0x4c6290 } else { 0x4c62d0 },
            args: [0; 6],
            argument_count: 0,
        };
        let sound = |credit| Seam::NativeUiCall {
            address: 0x4ad760,
            args: [if credit { 0x541cc8 } else { 0x541cb4 }, 0, 0, 0, 1, 0x3c],
            argument_count: 6,
        };
        assert_eq!(
            calls,
            vec![
                pulse(false),
                sound(false),
                sound(true),
                pulse(true),
                pulse(false),
                sound(false),
            ]
        );
        account_ui_post(
            AccountPosting {
                player: 0,
                amount: 100.,
            },
            None,
            |_| panic!("no local account"),
        )
        .unwrap();
        assert_eq!(
            players.iter().map(|p| p.cash).collect::<Vec<_>>(),
            [1000., 1000.]
        );
        ledger_ui_post_with(false, 100., 0., |_| panic!("other player")).unwrap();
    }

    #[test]
    #[ignore = "requires original v1.4 executable"]
    fn original_property_broadcast_dispatcher_is_inert_after_refresh() {
        let bytes = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&bytes).unwrap();
        // 451720 has already called 405260 for kind 0x26. The following
        // 48c1a0 dispatcher selects 48c3c6: clear flag and return.
        let kind = 0x26u32;
        let index = u32::from(image.at(0x48c3ec + kind - 0x22, 1).unwrap()[0]);
        let target = u32::from_le_bytes(
            image
                .at(0x48c3d0 + index * 4, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        assert_eq!(target, 0x48c3c6);
        assert_eq!(
            image.at(target, 9).unwrap(),
            &[0xc6, 0x46, 4, 0, 0x5e, 0x5b, 0x83, 0xc4, 0x34]
        );
        let mut packet = [0u8; 0x30];
        packet[4..8].copy_from_slice(&kind.to_le_bytes());
        assert!(property_refresh_broadcast(&packet));
        packet[4..8].copy_from_slice(&0x31u32.to_le_bytes());
        assert!(!property_refresh_broadcast(&packet));
    }
    #[test]
    fn faulted_tick_cannot_be_replayed_by_resume() {
        let mut host = Host::new([0.; 3]);
        host.set_running(true);
        host.fail("failure after retained mutation".into());
        host.set_running(true);
        assert!(!host.running);
        assert_eq!(
            host.failure.as_deref(),
            Some("failure after retained mutation")
        );
    }
    #[test]
    fn options_restore_worker_pause_state_and_cannot_resume_a_fault() {
        let mut host = Host::new([0.; 3]);
        let mut pause = openmt2001::pause::State::default();
        let mut timers = openmt2001::timers::startup_tasks([0x5a; 20]);
        for was_running in [false, true] {
            host.set_running(was_running);
            host.options(&mut pause, &mut timers, true).unwrap();
            host.options(&mut pause, &mut timers, true).unwrap();
            host.set_running(true);
            assert!(
                !host.running,
                "Options must retain its pause during other queued input"
            );
            host.options(&mut pause, &mut timers, false).unwrap();
            assert_eq!(host.running, was_running);
            host.options(&mut pause, &mut timers, false).unwrap();
            assert_eq!(host.running, was_running);
        }
        host.options(&mut pause, &mut timers, true).unwrap();
        host.fail("failure after retained mutation".into());
        host.options(&mut pause, &mut timers, false).unwrap();
        assert!(!host.running);
        host.fail("later stale action".into());
        assert_eq!(
            host.failure.as_deref(),
            Some("failure after retained mutation")
        );
    }
    #[test]
    fn options_reason_zero_preserves_other_reason_and_interface_task() {
        let mut host = Host::new([0.; 3]);
        host.set_running(true);
        let mut pause = openmt2001::pause::State::default();
        let mut timers = openmt2001::timers::startup_tasks([0x6b; 20]);
        for t in &mut timers {
            t[12..16].copy_from_slice(&37.5f32.to_le_bytes());
        }
        let before = timers.clone();
        host.options(&mut pause, &mut timers, true).unwrap();
        host.options(&mut pause, &mut timers, true).unwrap();
        assert_eq!(&pause.bytes[..4], &[1, 0, 0, 0]);
        assert_eq!(pause.bytes[8], 1);
        assert!(host.options_save_allowed(&pause, &timers));
        for (i, t) in timers.iter().enumerate() {
            let mut wanted = before[i];
            if i == 3 {
                wanted[0] = 0;
            }
            assert_eq!(*t, wanted);
        }
        assert_eq!(timers[2][0], 1, "interface mask1 stays live");
        pause
            .pause(1, |change, _| {
                change.apply(&mut timers);
                Ok(())
            })
            .unwrap();
        host.options(&mut pause, &mut timers, false).unwrap();
        host.options(&mut pause, &mut timers, false).unwrap();
        assert_eq!(&pause.bytes[..4], &[0, 1, 0, 0]);
        assert_eq!(pause.bytes[8], 1);
        assert!(
            !host.options_save_allowed(&pause, &timers),
            "another reason cannot impersonate this Options owner"
        );
        assert_eq!(timers[3][0], 0);
        assert_eq!(timers[2][0], 1);
        assert!(host.running);
        pause
            .unpause(1, |change, _| {
                change.apply(&mut timers);
                Ok(())
            })
            .unwrap();
        assert_eq!(timers, before);
        assert_eq!(pause.bytes, [0; 9]);
    }
    #[test]
    fn options_close_retains_another_reason_zero_and_rechecks_real_timer_state() {
        let mut host = Host::new([0.; 3]);
        host.set_running(true);
        let mut pause = openmt2001::pause::State::default();
        let mut timers = openmt2001::timers::startup_tasks([0x6b; 20]);
        pause
            .pause(0, |change, _| {
                change.apply(&mut timers);
                Ok(())
            })
            .unwrap();
        host.options(&mut pause, &mut timers, true).unwrap();
        assert_eq!(pause.bytes[0], 2);
        assert!(host.options_save_allowed(&pause, &timers));
        timers[3][0] = 1;
        assert!(!host.options_save_allowed(&pause, &timers));
        timers[3][0] = 0;
        host.options(&mut pause, &mut timers, false).unwrap();
        assert_eq!(pause.bytes[0], 1);
        assert_eq!(timers[3][0], 0);
        assert!(!host.options_save_allowed(&pause, &timers));
    }
    #[test]
    fn options_save_projection_releases_only_the_owned_copy() {
        for other in [None, Some(0), Some(1)] {
            let mut host = Host::new([0.; 3]);
            host.set_running(true);
            let mut pause = openmt2001::pause::State::default();
            let mut timers = openmt2001::timers::startup_tasks([0x6b; 20]);
            for (i, t) in timers.iter_mut().enumerate() {
                t[12..16].copy_from_slice(&(13.25 + i as f32).to_le_bytes());
            }
            if let Some(reason) = other {
                pause
                    .pause(reason, |change, _| {
                        change.apply(&mut timers);
                        Ok(())
                    })
                    .unwrap();
            }
            let expected = (pause.clone(), timers.clone());
            host.options(&mut pause, &mut timers, true).unwrap();
            let live = (pause.clone(), timers.clone());
            let projected = host
                .options_save_projection(&pause, &timers)
                .unwrap()
                .unwrap();
            assert_eq!(projected, expected);
            assert_eq!((pause.clone(), timers.clone()), live);
            assert!(host.options_save_allowed(&pause, &timers));
            assert!(!host.running);
            host.options(&mut pause, &mut timers, false).unwrap();
            assert_eq!((pause, timers), expected);
        }
    }
    #[test]
    fn options_save_projection_refuses_unaccountable_owned_counter() {
        let mut host = Host::new([0.; 3]);
        let mut pause = openmt2001::pause::State::default();
        let mut timers = openmt2001::timers::startup_tasks([0x3c; 20]);
        assert!(
            host.options_save_projection(&pause, &timers)
                .unwrap()
                .is_none()
        );
        host.options(&mut pause, &mut timers, true).unwrap();
        pause.bytes[0] = 128;
        let live = (pause.clone(), timers.clone());
        assert!(host.options_save_projection(&pause, &timers).is_err());
        assert_eq!((pause, timers), live);
    }
    #[test]
    fn network_options_does_not_mint_local_pause_reason() {
        let mut host = Host::new([0.; 3]);
        host.policy.scheduler.network_enabled = true;
        let mut pause = openmt2001::pause::State::default();
        let mut timers = openmt2001::timers::startup_tasks([0x6b; 20]);
        let before = timers.clone();
        host.options(&mut pause, &mut timers, true).unwrap();
        host.options(&mut pause, &mut timers, false).unwrap();
        assert_eq!(pause.bytes, [0; 9]);
        assert_eq!(timers, before);
    }
    #[test]
    fn scenario_prompt_preserves_callback_identity() {
        assert_eq!(
            script_request(
                &[
                    Value::Number(1898.),
                    Value::Number(1899.),
                    Value::Number(1.)
                ],
                false,
                true
            )
            .unwrap(),
            prompt::Request {
                script: Some(prompt::ScriptText {
                    title: None,
                    body: None,
                    format_value: 0,
                    timeout: true
                }),
                dialog: Some(crate::human_dialog::Dialog {
                    title: 1898,
                    body: 1899,
                    buttons: [1707, 0],
                    button_count: 1,
                    context: 1,
                    context_written: true,
                    ..Default::default()
                }),
                card: None,
                title: 1898,
                body: 1899,
                dismissal_id: 1
            }
        );
        assert_eq!(
            script_request(&[Value::Number(1898.), Value::Number(1899.)], false, true)
                .unwrap()
                .dismissal_id,
            0
        );
        assert!(
            script_request(
                &[Value::Bytes(b"custom".to_vec()), Value::Number(1899.)],
                false,
                true
            )
            .is_ok()
        );
    }
}

#[cfg(test)]
mod literal_lua_bridge_tests {
    use super::*;
    #[test]
    fn real_lua_to_owned_hooks_to_request() {
        // Same protected adapter signatures as Bindings::register. No PE fixture.
        let mut vm = openmt2001::lua4::Runtime::new().unwrap();
        vm.register_coerced("CreatePrompt", 5, "ppnn").unwrap();
        vm.register_coerced("CreateMessage", 724, "ppnn").unwrap();
        let mut calls = Vec::new();
        vm.execute("synthetic prompts", b"CreatePrompt('Synthetic Notice','A test zone is available.'); CreatePrompt('12','body','-3.9','7'); CreateMessage('title',13.9); CreatePrompt(16.9,'mixed'); CreatePrompt(14,15,nil,{}); CreatePrompt('drop','buffers')", &mut |id,args| {
            calls.push((id,args.to_vec())); Ok(None)
        }).unwrap();
        // Hooks::command is the production owned argument-copy boundary.
        let mut hooks = fresh_city::Hooks::new();
        for (id, args) in calls {
            openmt2001::level_script::Hooks::command(
                &mut hooks,
                if id == 5 {
                    level_script::Command::Prompt
                } else {
                    level_script::Command::TutorialMessage
                },
                &args,
            )
            .unwrap();
        }
        drop(vm);
        let requests = hooks
            .commands
            .iter()
            .map(|(command, args)| {
                script_request(
                    args,
                    false,
                    matches!(command, level_script::Command::Prompt),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            requests[0].script.as_ref().unwrap().title.as_deref(),
            Some(b"Synthetic Notice".as_slice())
        );
        assert_eq!(requests[1].title, 12);
        assert_eq!(requests[1].dismissal_id, (-3i32) as u32);
        assert_eq!(requests[1].script.as_ref().unwrap().format_value, 7);
        assert_eq!(requests[2].body, 13);
        assert_eq!(requests[3].title, 16);
        assert_eq!(
            requests[3].script.as_ref().unwrap().body.as_deref(),
            Some(b"mixed".as_slice())
        );
        assert_eq!(requests[4].dismissal_id, 0);
        assert_eq!(requests[4].script.as_ref().unwrap().format_value, 0);
    }
}

#[cfg(test)]
mod camera_error_effect_tests {
    use super::*;
    #[test]
    fn completed_timer_effects_survive_nonfinite_current_eye_rejection() {
        let mut host = Host::new([1., 2., 3.]);
        let mut tasks = openmt2001::timers::startup_tasks([0; 20]);
        for task in &mut tasks {
            task[12..16].copy_from_slice(&7.5f32.to_le_bytes());
        }
        let observation = camera_bridge::Observation {
            raw_capture: None,
            current: camera_bridge::Pose {
                eye: [f32::NAN.to_bits(), 0, 0],
                target: [0; 3],
                up: [0, 1f32.to_bits(), 0],
            },
            strategic_writeback: None,
            finalization_fault: None,
            sampled: None,
            task_effects: vec![
                camera_bridge::TaskEffect::SetEnabled {
                    mask: 2,
                    enabled: false,
                },
                camera_bridge::TaskEffect::ClearElapsed,
            ],
        };
        assert!(
            host.complete_camera_central(&mut tasks, observation)
                .is_err()
        );
        for task in &tasks {
            let mask = u32::from_le_bytes(task[4..8].try_into().unwrap());
            assert_eq!(task[0], u8::from(mask & 2 == 0));
            assert_eq!(&task[12..16], &[0; 4]);
        }
        assert_eq!(host.policy.camera, [1., 2., 3.]);
    }
}

#[cfg(test)]
mod camera_status_feedback_tests {
    use super::*;
    #[test]
    fn sampled_status_argument_cannot_replace_bridged_current_eye() {
        let mut host = Host::new([1., 2., 3.]);
        host.observe_presentation_camera([4., 5., 6.]);
        assert_eq!(host.policy.camera, [4., 5., 6.]);
        let (worker, _inbox) = camera_bridge::channel(7);
        host.camera_bridge = Some(worker);
        host.observe_presentation_camera([40., 50., 60.]);
        assert_eq!(host.policy.camera, [4., 5., 6.]);
    }
}

#[cfg(test)]
mod camera_retirement_tests {
    use super::*;
    #[test]
    fn retirement_cannot_resume_and_does_not_clear_a_real_failure() {
        let mut host = Host::new([1., 2., 3.]);
        let mut pause = openmt2001::pause::State::default();
        let mut timers = openmt2001::timers::startup_tasks([0x5a; 20]);
        let (worker, inbox) = camera_bridge::channel(401);
        host.camera_bridge = Some(worker);
        host.set_running(true);
        host.options(&mut pause, &mut timers, true).unwrap();
        inbox.retire();
        assert!(host.stop_retired_camera());
        assert!(host.options(&mut pause, &mut timers, false).is_err());
        host.set_running(true);
        assert!(!host.running);
        assert!(host.failure.is_none());
        assert_eq!(host.policy.camera, [1., 2., 3.]);
        host.camera_error(camera_bridge::RequestError::Failed(
            "independent real failure".into(),
        ));
        host.camera_error(camera_bridge::RequestError::Retired);
        assert_eq!(host.failure.as_deref(), Some("independent real failure"));
    }

    #[test]
    fn completed_live_task_effects_commit_even_after_retirement() {
        let mut host = Host::new([1., 2., 3.]);
        let (worker, inbox) = camera_bridge::channel(402);
        host.camera_bridge = Some(worker);
        let mut tasks = openmt2001::timers::startup_tasks([0; 20]);
        for task in &mut tasks {
            task[12..16].copy_from_slice(&7.5f32.to_le_bytes());
        }
        inbox.retire();
        host.complete_camera_central(
            &mut tasks,
            camera_bridge::Observation {
                raw_capture: None,
                strategic_writeback: None,
                finalization_fault: None,
                current: camera_bridge::Pose {
                    eye: [4f32, 5., 6.].map(f32::to_bits),
                    target: [0; 3],
                    up: [0, 1f32.to_bits(), 0],
                },
                sampled: None,
                task_effects: vec![
                    camera_bridge::TaskEffect::ClearElapsed,
                    camera_bridge::TaskEffect::SetEnabled {
                        mask: 2,
                        enabled: false,
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(host.policy.camera, [4., 5., 6.]);
        for task in &tasks {
            assert_eq!(&task[12..16], &[0; 4]);
            if u32::from_le_bytes(task[4..8].try_into().unwrap()) & 2 != 0 {
                assert_eq!(task[0], 0);
            }
        }
        assert!(host.stop_retired_camera());
        assert!(host.failure.is_none());
    }
}

#[cfg(test)]
mod camera_finalization_fault_tests {
    use super::*;
    #[test]
    fn real_host_applies_completed_ack_effects_then_fails_before_projection_and_next_work() {
        let mut host = Host::new([1., 2., 3.]);
        let (worker, mut inbox) = camera_bridge::channel(801);
        host.camera_bridge = Some(worker);
        let ui = std::thread::spawn(move || {
            let pose = camera_bridge::Pose {
                eye: [4f32, 5., 6.].map(f32::to_bits),
                target: [0; 3],
                up: [0, 1f32.to_bits(), 0],
            };
            inbox
                .wait_service(std::time::Duration::from_secs(2), |operation| {
                    assert!(matches!(
                        operation,
                        camera_bridge::Operation::Install { .. }
                    ));
                    Ok(camera_bridge::Observation {
                        raw_capture: None,
                        current: pose,
                        sampled: None,
                        task_effects: vec![],
                        strategic_writeback: None,
                        finalization_fault: None,
                    })
                })
                .unwrap();
            let result = inbox.wait_service(std::time::Duration::from_secs(2), |operation| {
                assert!(matches!(
                    operation,
                    camera_bridge::Operation::CentralTail { .. }
                ));
                Ok(camera_bridge::Observation {
                    raw_capture: None,
                    current: pose,
                    sampled: None,
                    strategic_writeback: None,
                    task_effects: vec![
                        camera_bridge::TaskEffect::ClearElapsed,
                        camera_bridge::TaskEffect::SetEnabled {
                            mask: 2,
                            enabled: false,
                        },
                    ],
                    finalization_fault: Some("C late completed-boundary fault".into()),
                })
            });
            assert_eq!(result, Err("C late completed-boundary fault".into()));
            // A real failure remains Failed even if retirement races its ack.
            inbox.retire();
        });
        host.exchange_camera(camera_bridge::Operation::Install {
            property: None,
            capture: openmt2001::camera::bank::Capture::from_records([[0; 196]; 2]),
            admission: camera_bridge::Admission::default(),
        })
        .unwrap()
        .unwrap();
        let completed = host
            .exchange_camera(camera_bridge::Operation::CentralTail {
                admission: camera_bridge::Admission::default(),
            })
            .unwrap()
            .unwrap();
        let mut tasks = openmt2001::timers::startup_tasks([0; 20]);
        for task in &mut tasks {
            task[12..16].copy_from_slice(&7.5f32.to_le_bytes());
        }
        let error = host
            .complete_camera_central(&mut tasks, completed)
            .unwrap_err();
        assert_eq!(error, "C late completed-boundary fault");
        assert_eq!(host.policy.camera, [1., 2., 3.]);
        for task in &tasks {
            assert_eq!(&task[12..16], &[0; 4]);
            if u32::from_le_bytes(task[4..8].try_into().unwrap()) & 2 != 0 {
                assert_eq!(task[0], 0);
            }
        }
        host.camera_error(camera_bridge::RequestError::Failed(error.clone()));
        assert_eq!(host.failure.as_deref(), Some(error.as_str()));
        assert!(!host.running);
        assert_eq!(
            host.exchange_camera(camera_bridge::Operation::CurrentPose),
            Err(camera_bridge::RequestError::Failed(error))
        );
        ui.join().unwrap();
    }
}

#[cfg(test)]
#[path = "card_account_feedback_test.rs"]
mod card_account_feedback_test;

#[cfg(test)]
#[path = "live_host/final_statistics_tests.rs"]
mod final_statistics_tests;

#[cfg(test)]
#[path = "live_host/ai_warning_tests.rs"]
mod ai_warning_tests;

#[cfg(test)]
#[path = "account_posting_tests.rs"]
mod account_posting_tests;

#[cfg(test)]
#[path = "live_host/modal_tests.rs"]
mod modal_tests;

#[cfg(test)]
#[path = "../../account_posting_test_support.rs"]
pub(super) mod account_posting_test_support;
