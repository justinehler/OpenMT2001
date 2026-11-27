//! Intro camera context and finite UI integration boundaries.
//! Raw records/kind/capture/restore remain exclusively in Owner.
use super::super::{Assets, Inspection};
use super::{
    cursor::Cursor,
    owner::{Input, Owner, Pose},
};
use crate::live_host::camera_bridge::{Admission, NativeOperation, Operation, TaskEffect};
use openmt2001::camera::bank::Capture;
use openmt2001::{
    camera::{
        bank::StrategicTransition,
        primary::{CursorRequest, Mode12Input, Mode13Input},
    },
    flyby::CameraSample,
};
use std::{collections::VecDeque, sync::mpsc};

/// These are process globals, not serialized camera fields or gameplay menu
/// choices. Caller must supply their actual values; no Default or mode guess.
#[derive(Clone, Copy)]
pub struct Globals {
    pub legacy_yaw: bool,           // 8b23c3
    pub low_height: bool,           // standalone fixture snapshot only; production reads process
    pub force_default_cursor: bool, // 8b22d8 && 8b22dc && !86d675
}

/// Original compatibility policy selected by the caller. This is unrelated
/// to whether this host happens to render through Vulkan/Metal/software WGPU.
pub use crate::live_host::process_runtime::OriginalGraphicsPolicy;
impl Globals {
    /// Legacy standalone fixture projection; this does not construct/reset
    /// ProcessRuntime. Production ignores low_height here and reads Assets.
    /// 4b91a5 initializes legacy0; 4b91c3 initializes height-kind2.
    /// D3D retains2; the original software policy writes0.
    pub fn after_normal_initialization(
        policy: OriginalGraphicsPolicy,
        force_default_cursor: bool,
    ) -> Self {
        Self {
            legacy_yaw: false,
            low_height: matches!(policy, OriginalGraphicsPolicy::NormalD3d),
            force_default_cursor,
        }
    }
}

pub struct State {
    pub globals: Globals,
    cursor: Cursor,
    admission: Admission,
    upper_input_error: Option<String>,
    // This is UI selection history only. There is deliberately no pose tuple.
    detail_selection: Option<crate::selection::Hit>,
    published_selection: Option<crate::selection::Hit>,
    staged: Vec<TaskEffect>,
    intents: VecDeque<Intent>,
    captured_saves: Vec<(mpsc::Sender<Result<Capture, String>>, Capture)>,
}

/// Only this FIFO or an ordered Operation::Apply may mutate Owner. GUI event
/// handlers enqueue intents; they never borrow Owner while Host logic runs.
pub enum Intent {
    Navigation,
    /// Original selection/script prelude must already be completed at its
    /// proper producer boundary. This is the explicit camera suffix only.
    BlockCameraSuffix {
        category_id: u32,
        caller: super::selection::BlockCaller,
    },
    /// No dynamic record is smuggled in from an earlier render snapshot.
    SelectBusiness {
        business_slot: u32,
    },
    /// A by-value raw camera cut, not a camera/world atomic save barrier.
    CaptureSave {
        reply: mpsc::Sender<Result<Capture, String>>,
    },
}
impl State {
    /// A supplied strategic cold load can have no retained selected object;
    /// None is valid UI state, never a reason to replace either raw record.
    pub fn new(
        globals: Globals,
        cursor: Cursor,
        detail_selection: Option<crate::selection::Hit>,
        published_selection: Option<crate::selection::Hit>,
    ) -> Self {
        Self {
            globals,
            cursor,
            admission: Admission::default(),
            upper_input_error: None,
            detail_selection,
            published_selection,
            staged: Vec::new(),
            intents: VecDeque::new(),
            captured_saves: Vec::new(),
        }
    }
    /// A stopped Host cannot issue another CentralTail. Complete all queued
    /// save replies here without dispatch, sample, or raw-record fallback.
    pub fn reject_save_captures(&mut self, reason: &str) {
        self.intents.retain(|intent| {
            if let Intent::CaptureSave { reply } = intent {
                let _ = reply.send(Err(reason.to_owned()));
                false
            } else {
                true
            }
        });
        for (reply, _) in self.captured_saves.drain(..) {
            let _ = reply.send(Err(reason.to_owned()));
        }
    }
    pub fn admission(&self) -> &Admission {
        &self.admission
    }
    pub(super) fn prospective_cursor(&self) -> Cursor {
        self.cursor
    }
    pub(super) fn publish_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
    }
    pub fn ui_cursor(&mut self, request: CursorRequest) -> Result<(), String> {
        self.cursor
            .select(request, self.globals.force_default_cursor)
    }
    pub fn reject_upper_input(&mut self, error: String) {
        if self.upper_input_error.is_none() {
            self.upper_input_error = Some(error);
        }
    }
    pub fn cursor_index(&self) -> usize {
        self.cursor.host_index()
    }
    pub fn cursor_presented(&mut self) {
        self.cursor.presented();
    }
    pub fn enqueue(&mut self, intent: Intent) {
        self.intents.push_back(intent);
    }
    pub fn queue_save_capture(&mut self) -> mpsc::Receiver<Result<Capture, String>> {
        let (reply, result) = mpsc::channel();
        self.enqueue(Intent::CaptureSave { reply });
        result
    }
}

/// The only external UI callbacks this finite adapter cannot substitute with
/// existing Inspection fields. The caller supplies concrete nonblocking
/// functions or the route errors. They MUST NOT send a FreshRequest, wait on
/// a worker, mutate a clock, or stage scheduler effects during Apply.
/// No function is installed here under an invented successful default.
pub struct UiJoins {
    pub primary_clear: Option<fn(&mut Inspection, &mut Assets, &mut Owner) -> Result<(), String>>,
    pub navigation: Option<
        fn(
            &mut Inspection,
            &mut super::ui_text_resources::UiTextResources,
            NavigationUi,
            &mut Owner,
        ) -> Result<(), String>,
    >,
    pub tutorial:
        Option<fn(&mut Inspection, &mut Assets, NativeOperation, &mut Owner) -> Result<(), String>>,
}
pub enum NavigationUi {
    /// Hide G/H/B, enable A and save visibility before the camera setters.
    EnterMode3BeforeCamera,
    /// Reset groups, disable A/F and enable D/C after the camera setters.
    EnterMode3AfterCamera,
    ReturnMode0ThenToolbar1,
    RestorePickedPanel,
    RefreshThenShowGroup73,
}

#[derive(Clone, Copy)]
pub enum SoundBoundary {
    /// Explicitly conditional on the existing mono/2D backend. The current
    /// backend has no listener setter; this does not implement native v40/58.
    Existing2d,
    RequireOriginalListener,
}

pub struct UiContext<'a> {
    view: &'a mut Inspection,
    physical: &'a mut super::input::PhysicalInput,
    assets: &'a mut Assets,
    joins: &'a UiJoins,
    sound: SoundBoundary,
}

impl<'a> UiContext<'a> {
    pub fn new(
        view: &'a mut Inspection,
        physical: &'a mut super::input::PhysicalInput,
        assets: &'a mut Assets,
        joins: &'a UiJoins,
        sound: SoundBoundary,
    ) -> Result<Self, String> {
        let state = view
            .camera_ui
            .as_ref()
            .ok_or("Camera UI state not installed")?;
        if !state.staged.is_empty() || !state.captured_saves.is_empty() {
            return Err("Camera task effects leaked across endpoint boundaries".into());
        }
        Ok(Self {
            view,
            physical,
            assets,
            joins,
            sound,
        })
    }
    fn state(&mut self) -> &mut State {
        self.view.camera_ui.as_mut().unwrap()
    }
    fn navigation_ui(&mut self, owner: &mut Owner, phase: NavigationUi) -> Result<(), String> {
        self.joins
            .navigation
            .ok_or("Original navigation UI producer is unjoined")?(
            self.view,
            &mut self.assets.ui_text,
            phase,
            owner,
        )
    }

    /// Run inside CentralTail processing before input, camera callback and
    /// common completion. Intents retain their event order.
    pub fn pre_camera(&mut self, owner: &mut Owner, admission: &Admission) -> Result<(), String> {
        use super::endpoint::Context;
        if let Some(error) = &self.state().upper_input_error {
            return Err(error.clone());
        }
        // Endpoint already validated and refreshed BOTH suppliers atomically.
        if self.view.ui_graph.is_some() {
            if admission.upper.is_none() {
                return Err("F current CentralTail supplier absent".into());
            }
            //The staged GUI owner is read here; physical camera packets and
            //display hover cannot overwrite GUI logical/current-right cells.
            super::f_generic::pointer(self.view)?;
        }
        while let Some(intent) = self.state().intents.pop_front() {
            match intent {
                Intent::Navigation => {
                    self.navigate(owner)?;
                }
                Intent::BlockCameraSuffix {
                    category_id,
                    caller,
                } => {
                    super::selection::initialize_block(
                        owner,
                        &self.assets.selection,
                        category_id,
                        caller,
                    )?;
                    self.view.strategic = false;
                    self.view.return_camera = None;
                    self.view.title_state = None;
                    self.view.reframe_pending = true;
                }
                Intent::SelectBusiness { business_slot } => {
                    return Err(format!(
                        "Queued business {business_slot} requires an ordered current producer payload; worker query/Scene fallback forbidden"
                    ));
                }
                Intent::CaptureSave { reply } => {
                    // No sampling, view projection or worker command. Delay
                    // publication until the entire CentralTail succeeds.
                    self.state().captured_saves.push((reply, owner.capture()));
                }
            }
        }
        if self.view.ui_graph.is_some() {
            super::upper_f::central_focus_tail(self.view);
        }
        Ok(())
    }
}

/// No mutation, cloning of authority, callbacks or worker query. Graph absence
/// remains a valid camera-only route. On protected graph boundaries neither
/// packet nor actual process owner may be omitted.
fn validate_suppliers(
    view: &Inspection,
    value: &Admission,
    protected: bool,
    owner: Option<&Owner>,
) -> Result<(), String> {
    use super::ui_graph::PanelId;
    let owners = view.ui_graph.as_ref().map(|g| SupplierOwners {
        locator: view.locator.as_ref(),
        colors: &g.world.colors,
        c_children: g.panels.get(&PanelId::C).map(|p| p.children.as_slice()),
        upper: g.upper.as_ref(),
        f_children: g.panels.get(&PanelId::F).map(|p| p.children.as_slice()),
        input_panel: g.panels.contains_key(&PanelId::G),
    });
    validate_owner_packets(owners, value, protected, owner)
}
struct SupplierOwners<'a> {
    locator: Option<&'a crate::locator::Locator>,
    colors: &'a [u32; 600],
    c_children: Option<&'a [super::ui_graph::WidgetId]>,
    upper: Option<&'a crate::navigator::upper::State>,
    f_children: Option<&'a [super::ui_graph::WidgetId]>,
    input_panel: bool,
}
fn validate_owner_packets(
    owners: Option<SupplierOwners<'_>>,
    value: &Admission,
    protected: bool,
    owner: Option<&Owner>,
) -> Result<(), String> {
    let Some(owners) = owners else {
        return Ok(());
    };
    if protected && !owners.input_panel {
        return Err("Protected graph input G panel absent".into());
    }
    if protected && (value.strategic.is_none() || value.upper.is_none()) {
        return Err("Protected UI graph boundary requires current C and F suppliers".into());
    }
    if protected || value.strategic.is_some() {
        super::strategic_panel::check_supplier_owner(
            owners.locator,
            owners.colors,
            owners.c_children,
            value.strategic.as_deref(),
            owner,
        )?;
    }
    if protected || value.upper.is_some() {
        super::upper_f::check_supplier_owner(
            owners.upper,
            owners.f_children,
            value.upper.as_deref(),
        )?;
    }
    if let (Some(c), Some(f)) = (&value.strategic, &value.upper) {
        if c.categories != f.rows || c.tables.property_labels != f.property_labels {
            return Err("C/F supplier category/table identities disagree".into());
        }
    }
    Ok(())
}

impl super::endpoint::Context for UiContext<'_> {
    fn primary_construction(
        &mut self,
    ) -> Option<(
        &mut super::deferred_primary::DeferredPrimary,
        &mut super::primitive_factory::Primitives,
        &mut super::ui_text_resources::UiTextResources,
        &super::process_images::Images,
    )> {
        match &mut self.assets.primary_products {
            super::deferred_primary::PrimaryProducts::Deferred(primary) => Some((
                primary,
                self.assets.navigator.process_primitives_mut(),
                &mut self.assets.ui_text,
                &self.assets.ui_images,
            )),
            super::deferred_primary::PrimaryProducts::Reordered(_) => None,
        }
    }
    fn save_interface_mode(&self) -> Result<u32, String> {
        Ok(if self.view.strategic { 3 } else { 0 })
    }
    fn process_runtime(&self) -> Option<&crate::live_host::process_runtime::Handle> {
        Some(&self.assets.process_runtime)
    }
    fn validate(&self, operation: &Operation, owner: &Owner) -> Result<(), String> {
        let (value, protected) = match operation {
            Operation::Install { admission, .. } | Operation::CentralTail { admission } => {
                (admission, true)
            }
            Operation::Apply { admission, .. } | Operation::Admission(admission) => {
                (admission, false)
            }
            Operation::RawCapture | Operation::CurrentPose | Operation::Sample { .. } => {
                return Ok(());
            }
        };
        if value.upper.is_some() || value.strategic.is_some() {
            if let super::deferred_primary::PrimaryProducts::Deferred(primary) =
                &self.assets.primary_products
            {
                //Read-only rejection on Install/Admission/Apply/CentralTail
                //before packet/physical/UI/owner/source changes (D-01).
                primary.require_supplier()?;
            }
        }
        if matches!(operation, Operation::CentralTail { .. }) {
            if let Some(error) = &self
                .view
                .camera_ui
                .as_ref()
                .ok_or("Camera UI state absent")?
                .upper_input_error
            {
                return Err(error.clone());
            }
            self.physical.snapshot()?;
        }
        validate_suppliers(self.view, value, protected, Some(owner))
    }
    fn admission(&mut self, value: &Admission) -> Result<(), String> {
        // Check both actual owners and all fallible C replacement conditions
        // BEFORE either packet or graph metadata is published. Pure Admission
        // may omit packets; omissions do not refresh or fabricate a producer.
        if value.upper.is_some() || value.strategic.is_some() {
            if let super::deferred_primary::PrimaryProducts::Deferred(primary) =
                &self.assets.primary_products
            {
                primary.require_supplier()?;
            }
        }
        validate_suppliers(self.view, value, false, None)?;
        if self.view.ui_graph.is_some() {
            if let Some(input) = &value.strategic {
                super::strategic_panel::suppliers(self.view, input.clone())?;
            }
            if let Some(input) = &value.upper {
                super::upper_f::suppliers(self.view, input.clone())?;
            }
        }
        self.state().admission = value.clone();
        Ok(()) // presentation mirror only; no clock/worker/UI replay
    }

    fn input(&mut self, admission: &Admission) -> Result<Input, String> {
        let (keys, buttons, delta) = self.physical.snapshot()?;
        let settings = self.view.settings_menu.gameplay;
        let flags = self.state().globals;
        // Assets retains the same synchronized process identity lent to Session
        // hooks. No lock crosses callbacks; one read serves both input modes.
        let low_height = self
            .assets
            .process_runtime
            .performance_nonzero()
            .map_err(|e| e.to_string())?;
        // 920889 has its own producer. all_disabled is a different UI group
        // control and MUST NOT be folded into this byte or raw event capture.
        let enabled = !admission.camera_disabled;
        Ok(Input {
            mode12: Mode12Input {
                build_visible: if let Some(graph) = &self.view.ui_graph {
                    graph.panel(super::ui_graph::PanelId::G)?.requested != 0
                } else {
                    self.view.planning.is_some()
                },
                enable_translation: enabled,
                enable_keyboard: enabled,
                enable_rotation: enabled,
                enable_zoom: enabled,
                keys,
                buttons,
                delta,
                sensitivity: i32::from(settings.mouse_sensitivity),
                legacy_yaw: flags.legacy_yaw,
                invert_vertical: settings.invert_vertical,
                low_height,
            },
            mode13: Mode13Input {
                enable_translation: enabled,
                enable_keyboard: enabled,
                enable_zoom: enabled,
                keys,
                buttons,
                delta,
                sensitivity: i32::from(settings.mouse_sensitivity),
                legacy_yaw: flags.legacy_yaw,
                invert_vertical: settings.invert_vertical,
                low_height,
            },
        })
    }

    fn consumed_input(&mut self) {
        self.physical.consumed();
        // Retire old routed one-shot storage too; it is no longer authority.
        self.view.taps = [false; 4];
        self.view.drag = [0; 2];
    }

    fn cursor(&mut self, requests: Vec<CursorRequest>) -> Result<(), String> {
        let state = self.state();
        for request in requests {
            state
                .cursor
                .select(request, state.globals.force_default_cursor)?;
        }
        Ok(())
    }

    fn pre_camera(&mut self, owner: &mut Owner, admission: &Admission) -> Result<(), String> {
        UiContext::pre_camera(self, owner, admission)
    }

    fn navigate(&mut self, owner: &mut Owner) -> Result<(), String> {
        if self.state().admission.strategic.is_none() {
            return Err("C navigation requires current protected-boundary suppliers".into());
        }
        let clear = self
            .joins
            .primary_clear
            .ok_or("43e370 primary focus clear/refresh producer is unjoined")?;
        if self.joins.navigation.is_none() {
            return Err("Original navigation UI completion producer is unjoined".into());
        }
        clear(self.view, self.assets, owner)?;
        if owner.kind() == 4 {
            owner.copy_secondary_to_primary();
            self.navigation_ui(owner, NavigationUi::ReturnMode0ThenToolbar1)?;
            //C hide has already restored822d50; toolbar callbacks may change it.
            //Publish the live authority AFTER toolbar1; never reseed from view.
            owner.ui_selection_mut()?.publish_current();
            let published = owner.published_ui_selected()?;
            self.state().published_selection = published.and_then(|value| value.hit);
            self.view.selected = published.and_then(|value| value.hit);
            self.view.strategic = false;
            if published.is_some() {
                self.navigation_ui(owner, NavigationUi::RestorePickedPanel)?;
            }
        } else {
            owner.copy_primary_to_secondary();
            self.navigation_ui(owner, NavigationUi::EnterMode3BeforeCamera)?;
            owner.enter_strategic_fields()?;
            self.navigation_ui(owner, NavigationUi::EnterMode3AfterCamera)?;
            //4baf76 retains current AFTER C show and its phase11 callbacks.
            let selected = owner.current_ui_selected()?;
            self.view
                .ui_graph
                .as_mut()
                .ok_or("Graph lost during C show")?
                .retained_c = selected;
            self.state().detail_selection = selected.and_then(|value| value.hit);
            //Do not destroy retained B/H view state on a panel hide.
            self.view.selected = None;
            self.view.strategic = true;
        }
        if let Some(locator) = &mut self.view.locator {
            locator.dirty = true;
        }
        self.view.return_camera = None;
        self.view.hovered = None;
        self.view.title_state = None;
        self.view.reframe_pending = true;
        self.navigation_ui(owner, NavigationUi::RefreshThenShowGroup73)
    }

    fn tutorial(&mut self, operation: NativeOperation, owner: &mut Owner) -> Result<(), String> {
        match operation {
            NativeOperation::TutorialMode { mode: 6, .. } => {
                owner.set_kind(6);
                Ok(())
            }
            operation @ (NativeOperation::TutorialMode { .. }
            | NativeOperation::TutorialCityView
            | NativeOperation::TutorialPlanView) => {
                // Existing {mode,target} cannot carry current selected-building
                // words or the script/selection prelude outcome. No lookup in
                // self.view.scene and no request to the blocked worker occurs.
                self.joins.tutorial.ok_or(
                    "Tutorial selection/view producer payload is unjoined; stale Scene is not authority"
                )?(self.view, self.assets, operation, owner)
            }
            _ => Err("Non-tutorial operation reached tutorial Context".into()),
        }
    }

    fn pending_sound(&mut self, _current: Pose) -> Result<(), String> {
        pending_sound_service(self.view, self.sound)
    }

    fn present(&mut self, sampled: Pose) -> Result<(), String> {
        let previous = self.view.camera;
        let camera = CameraSample {
            eye: sampled.eye.map(f32::from_bits),
            target: sampled.target.map(f32::from_bits),
            up: sampled.up.map(f32::from_bits),
            vertical_fov: previous.vertical_fov,
        };
        // CameraSample is only a projection of the completed raw SAMPLE; no
        // orbit, controls, kind/defaultmode, common or sampling happens here.
        if previous.eye.map(f32::to_bits) != sampled.eye
            || previous.target.map(f32::to_bits) != sampled.target
            || previous.up.map(f32::to_bits) != sampled.up
        {
            self.view.reframe_pending = true;
        }
        self.view.camera = camera;
        Ok(())
    }

    fn take_strategic_writeback(
        &mut self,
    ) -> Result<Option<crate::locator::strategic::Writeback>, String> {
        if self.view.ui_graph.is_none() {
            return Ok(None);
        }
        super::strategic_panel::take_writeback(self.view)
    }
    fn take_task_effects(&mut self) -> Vec<TaskEffect> {
        // Extraction cannot publish a save. Endpoint retains this completed
        // vector even if a later finalizer unexpectedly fails.
        std::mem::take(&mut self.state().staged)
    }
    fn publish_save_captures(&mut self) {
        // Last successful finalization point, after C writeback extraction.
        // Closed receiver cancels an intent; it cannot replay a boundary.
        if !self.state().captured_saves.is_empty()
            && std::env::var_os("OPENMT2001_CAMERA_OWNER_NATIVE_REVIEW").is_some()
        {
            eprintln!("[camera] publishing raw save capture after completed CentralTail");
        }
        for (reply, capture) in std::mem::take(&mut self.state().captured_saves) {
            let _ = reply.send(Ok(capture));
        }
    }
    fn discard_task_effects(&mut self) {
        let state = self.state();
        state.staged.clear();
        for (reply, _) in std::mem::take(&mut state.captured_saves) {
            let _ = reply.send(Err(
                "Camera CentralTail failed before save cut publication".into()
            ));
        }
        for intent in state.intents.drain(..) {
            if let Intent::CaptureSave { reply } = intent {
                let _ = reply.send(Err("Camera boundary failed before queued save cut".into()));
            }
        }
    }
}

/// Call at the actual UI pre-tail pass, before draining camera endpoints.
/// The final cursor query reads THIS arbiter; native requests are not queued
/// into an ignored field or reconstructed from physical/routed buttons.
pub fn ui_cursor_pass(
    view: &mut Inspection,
    assets: &Assets,
    menu: Option<&crate::menu::Menu>,
) -> Result<(), String> {
    let Some(state) = view.camera_ui.as_ref() else {
        return Ok(());
    };
    let force_default = state.globals.force_default_cursor;
    let requested = if let Some(page) = view.settings_menu.page {
        let blocked = view.toolbar_right_held
            || view.save_dialog.visible
            || view.stats_requested
            || view
                .live_status
                .as_ref()
                .is_some_and(|status| status.stats.is_some());
        let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
        u32::from(!blocked && assets.settings_menu.action_cursor_hover(page, pointer))
    } else {
        u32::from(menu.is_some_and(|menu| {
            view.toolbar_target(menu, &assets.selection.blocks)
                .is_some()
        }))
    };
    view.camera_ui.as_mut().unwrap().cursor.select(
        CursorRequest {
            state: requested,
            priority: 1,
        },
        force_default,
    )
}

#[cfg(test)]
mod stopped_save_tests {
    use super::*;
    #[test]
    fn failed_host_finishes_queued_and_unpublished_captures_without_camera_work() {
        let mut state = State::new(
            Globals::after_normal_initialization(OriginalGraphicsPolicy::NormalD3d, false),
            Cursor::new(0, 0).unwrap(),
            None,
            None,
        );
        let first = state.queue_save_capture();
        state.enqueue(Intent::Navigation);
        let second = state.queue_save_capture();
        let (reply, third) = mpsc::channel();
        state
            .captured_saves
            .push((reply, Capture::from_records([[0; 196]; 2])));
        state.reject_save_captures("stopped host");
        for receiver in [first, second, third] {
            assert_eq!(receiver.try_recv().unwrap().unwrap_err(), "stopped host");
        }
        assert!(state.captured_saves.is_empty());
        assert!(matches!(
            state.intents.pop_front(),
            Some(Intent::Navigation)
        ));
        assert!(state.intents.is_empty());
    }
}

#[cfg(test)]
mod supplier_preflight_tests {
    use super::super::{
        input::PhysicalInput,
        ui_graph::{PanelId, WidgetId, WidgetKind},
    };
    use super::*;
    use crate::{
        locator::{
            Locator,
            strategic::{self, EnabledBufferProvenance},
        },
        navigator::upper,
    };
    use openmt2001::camera::bank::Bank;
    struct Fixture {
        locator: Locator,
        upper: upper::State,
        owner: Owner,
        colors: [u32; 600],
        children: [WidgetId; 1],
        value: Admission,
    }
    fn fixture() -> Fixture {
        let mut s = strategic::selector_boundary_tests::state_fixture(
            crate::compatibility::Mode::Modern,
            EnabledBufferProvenance::Unrepresented,
        );
        s.tab = 2;
        let c = s.input.clone();
        let colors = c.before_colors;
        let mut locator = crate::locator::refresh_ui_tests::fresh();
        locator.install_strategic(s).unwrap();
        let f = std::sync::Arc::new(upper::Inputs {
            source_epoch: crate::live_host::camera_bridge::PropertyEpoch::synthetic(1),
            rows: c.categories,
            owners: [None; 52],
            count: 0,
            phase: 7,
            presentation_mode: 1,
            property_labels: c.tables.property_labels,
        });
        let geometry = upper::Geometry {
            bounds: [0., 0., 1., 1.],
            second: [0., 0., 1., 1.],
            backing: [0., 0., 1., 1.],
            parent_open: [0., 0., 1., 1.],
            parent_closed: [0., 0., 1., 1.],
        };
        let upper = upper::State::installed(
            geometry,
            -1,
            0,
            1,
            upper::Pointer {
                xy: [3, 4],
                blocked: 0,
                restricted: 0,
                allowed: -1,
            },
            f.clone(),
        );
        let mut owner = Owner::new(
            Bank::from_capture(Capture::from_records([[0xa5; 196], [0x5a; 196]])),
            0x91,
        );
        owner
            .install_ui_selection(super::super::ui_selection::SelectionState::installed(
                None, None, None, None, None, None, 42, 19,
            ))
            .unwrap();
        Fixture {
            locator,
            upper,
            owner,
            colors,
            children: [WidgetId {
                panel: PanelId::C,
                kind: WidgetKind::LocatorChild(5),
            }],
            value: Admission {
                strategic: Some(c),
                upper: Some(f),
                ..Admission::default()
            },
        }
    }
    fn owners(f: &Fixture) -> SupplierOwners<'_> {
        SupplierOwners {
            locator: Some(&f.locator),
            colors: &f.colors,
            c_children: Some(&f.children),
            upper: Some(&f.upper),
            f_children: Some(&[]),
            input_panel: true,
        }
    }
    #[test]
    fn real_context_preflight_rejects_missing_packets_and_actual_owners_without_touching_boundary_state()
     {
        use winit::{
            event::MouseButton,
            keyboard::{KeyCode, PhysicalKey},
        };
        let f = fixture();
        let before = f.owner.capture();
        let input = f.locator.strategic().unwrap().input.clone();
        let mut physical = PhysicalInput::default();
        physical.key(PhysicalKey::Code(KeyCode::ArrowRight), true);
        physical.button(MouseButton::Right, true);
        physical.pointer([1., 2.]);
        physical.pointer([4., 8.]);
        let input_before = physical.snapshot().unwrap();
        let mut ui = State::new(
            Globals::after_normal_initialization(OriginalGraphicsPolicy::NormalD3d, false),
            Cursor::new(0, 0).unwrap(),
            None,
            None,
        );
        let reply = ui.queue_save_capture();
        for which in 0..5 {
            let mut value = f.value.clone();
            let mut o = owners(&f);
            let empty = crate::locator::refresh_ui_tests::fresh();
            match which {
                0 => value.strategic = None,
                1 => value.upper = None,
                2 => o.locator = None,
                3 => o.locator = Some(&empty),
                4 => o.upper = None,
                _ => unreachable!(),
            }
            assert!(validate_owner_packets(Some(o), &value, true, Some(&f.owner)).is_err());
            assert_eq!(f.owner.capture(), before);
            assert_eq!(f.owner.kind_dirty(), 0x91);
            assert_eq!(physical.snapshot().unwrap(), input_before);
            assert!(std::sync::Arc::ptr_eq(
                &input,
                &f.locator.strategic().unwrap().input
            ));
            assert!(std::sync::Arc::ptr_eq(
                f.value.upper.as_ref().unwrap(),
                &f.upper.input
            ));
            assert!(matches!(reply.try_recv(), Err(mpsc::TryRecvError::Empty)));
            assert_eq!(ui.intents.len(), 1);
            assert!(ui.captured_saves.is_empty());
        }
        assert!(validate_owner_packets(Some(owners(&f)), &f.value, true, Some(&f.owner)).is_ok());
    }
    #[test]
    fn graph_none_and_supplierless_pure_admission_remain_valid_but_protected_graph_is_not() {
        let f = fixture();
        let none = Admission::default();
        assert!(validate_owner_packets(None, &none, true, Some(&f.owner)).is_ok());
        let o = SupplierOwners {
            locator: None,
            upper: None,
            ..owners(&f)
        };
        assert!(validate_owner_packets(Some(o), &none, false, Some(&f.owner)).is_ok());
        assert!(validate_owner_packets(Some(owners(&f)), &none, true, Some(&f.owner)).is_err());
    }
    #[test]
    fn cross_packet_mismatch_dirty_state_and_wrong_tree_leave_both_suppliers_unchanged() {
        let mut f = fixture();
        let before = f.locator.strategic().unwrap().input.clone();
        let mut value = f.value.clone();
        std::sync::Arc::make_mut(value.upper.as_mut().unwrap()).rows[2][4] = 1;
        assert!(validate_owner_packets(Some(owners(&f)), &value, true, Some(&f.owner)).is_err());
        f.children[0].kind = WidgetKind::LocatorChild(6);
        assert!(validate_owner_packets(Some(owners(&f)), &f.value, true, Some(&f.owner)).is_err());
        f.children[0].kind = WidgetKind::LocatorChild(5);
        f.locator.strategic_mut().unwrap().dirty_writeback = true;
        assert!(validate_owner_packets(Some(owners(&f)), &f.value, true, Some(&f.owner)).is_err());
        assert!(std::sync::Arc::ptr_eq(
            &before,
            &f.locator.strategic().unwrap().input
        ));
        assert!(std::sync::Arc::ptr_eq(
            f.value.upper.as_ref().unwrap(),
            &f.upper.input
        ));
    }
    #[test]
    fn supplier_refresh_keeps_external_colors_and_refuses_uncommitted_local_colors() {
        let mut f = fixture();
        let mut next = (*f.value.strategic.as_ref().unwrap().as_ref()).clone();
        next.before_colors[0] = 0x12345678;
        let next = std::sync::Arc::new(next);
        super::super::strategic_panel::replace_supplier_owner(
            &mut f.locator,
            &mut f.colors,
            next.clone(),
        )
        .unwrap();
        assert_eq!(f.colors[0], 0x12345678);
        assert!(
            f.locator
                .take_strategic_writeback(f.colors)
                .unwrap()
                .is_none()
        );
        f.colors[0] = 0x87654321;
        let pending = f.colors;
        assert!(
            super::super::strategic_panel::replace_supplier_owner(
                &mut f.locator,
                &mut f.colors,
                next.clone()
            )
            .is_err()
        );
        assert_eq!(f.colors, pending);
        assert!(std::sync::Arc::ptr_eq(
            &next,
            &f.locator.strategic().unwrap().input
        ));
    }
}

#[cfg(test)]
#[path = "ui_text_caller_join_tests.rs"]
mod caller_join_tests;

/// Actual Sample/pending_sound consumer, shared with the bounded camera CPU
///witness. It never waits; PCM completion uses the view's SAME Route fence.
pub(crate) fn pending_sound_service(
    view: &mut Inspection,
    sound: SoundBoundary,
) -> Result<(), String> {
    if matches!(sound, SoundBoundary::RequireOriginalListener) {
        return Err("Existing 2D audio has no native current-pose listener consumer".into());
    }
    super::audio::service_available(view)
}
