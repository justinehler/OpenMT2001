//! Legacy display producer using the SAME process Owner as native endpoints.
//! Captures are transaction cuts; there is no second evolving Bank. Native
//! bridge sessions keep their existing capture path and never install this source.
use super::{CameraSample, Inspection, NORMAL_CITY_VERTICAL_FOV, camera_live};
use crate::save_menu::camera_source::Publisher;
use openmt2001::camera::{
    BlockCamera, BlockCameraInertia,
    bank::{Bank, Capture, RawRecord, StrategicTransition},
};

/// Saved raw camera records do not store the retained gameplay renderer FOV.
pub(super) const STRATEGIC_FOV: f32 = crate::navigation::STRATEGIC_VERTICAL_FOV;

pub(super) fn new_process_owner() -> Result<camera_live::owner::Owner, String> {
    // Actual once-only modern UI process raw-storage allocation. The existing
    // frontend explicitly selects NormalD3d, so4b98xx passes width50/parameter0.
    // These are the proved4e0700 stores, not a full CRT/graph constructor claim.
    let mut bank = Bank::loader_zero_storage();
    let mut dirty = 0;
    bank.initialize_process(50, 0, &mut dirty)
        .map_err(str::to_owned)?;
    Ok(camera_live::owner::Owner::new(bank, dirty))
}
fn valid_pose(camera: CameraSample) -> Result<(), String> {
    if camera
        .eye
        .into_iter()
        .chain(camera.target)
        .chain(camera.up)
        .any(|v| !v.is_finite())
        || camera.up == [0.; 3]
    {
        return Err("Unsupported legacy camera pose".into());
    }
    Ok(())
}
fn coherent(capture: Capture, mode: u32) -> Result<(), String> {
    let kind = u32::from_le_bytes(capture.records()[0][..4].try_into().unwrap());
    if !matches!(mode, 0 | 3) || (mode == 3 && kind != 4) {
        return Err(
            "Legacy camera raw kind disagrees with host mode; normalization refused".into(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct Controls {
    pub camera: CameraSample,
    pub controls: BlockCamera,
    pub inertia: BlockCameraInertia,
    pub saved: bool,
    pub building: bool,
    pub building_angle: f32,
    pub selected: Option<crate::selection::Hit>,
}
impl Controls {
    pub(super) fn read(view: &Inspection) -> Self {
        Self {
            camera: view.camera,
            controls: view.controls,
            inertia: view.inertia,
            saved: view.saved_camera,
            building: view.building_camera,
            building_angle: view.building_angle,
            selected: view.selected,
        }
    }
    pub(super) fn put(self, view: &mut Inspection) {
        view.camera = self.camera;
        view.controls = self.controls;
        view.inertia = self.inertia;
        view.saved_camera = self.saved;
        view.building_camera = self.building;
        view.building_angle = self.building_angle;
        view.selected = self.selected;
    }
    pub(super) fn from_raw(raw: &RawRecord, strategic: bool) -> Result<Self, String> {
        let kind = u32::from_le_bytes(raw[..4].try_into().unwrap());
        if (strategic && kind != 4) || (!strategic && !matches!(kind, 6 | 12 | 13)) {
            return Err(format!(
                "Raw camera kind{kind} has no supported legacy return/control projection"
            ));
        }
        let (camera, mut controls) = super::saved_viewpoint(
            raw,
            if strategic {
                STRATEGIC_FOV
            } else {
                NORMAL_CITY_VERTICAL_FOV
            },
        )?;
        let building = kind == 13;
        if building {
            controls.center = camera.target;
        }
        Ok(Self {
            camera,
            controls,
            inertia: BlockCameraInertia::new(camera.eye, camera.target),
            saved: true,
            building,
            building_angle: controls.angle,
            selected: None,
        })
    }
    fn same_pose(&self, raw: &RawRecord) -> bool {
        [
            (0x4c, self.camera.eye),
            (0x58, self.camera.target),
            (0x64, self.camera.up),
        ]
        .into_iter()
        .all(|(at, values)| {
            values
                .into_iter()
                .enumerate()
                .all(|(axis, value)| raw[at + 4 * axis..at + 4 * axis + 4] == value.to_le_bytes())
        })
    }
}

/// Source provenance is retained separately from this modern host attachment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Attachment {
    Fresh,
    Explicit(u32),
    LegacyOrdinary,
    LegacyRawKind4,
}
pub(super) struct Initial {
    pub active: Controls,
    pub normal_return: Option<Controls>,
    imported: Option<Capture>,
    mode: u32,
    pub provenance: Attachment,
    pub metadata: Option<u32>,
}
impl Initial {
    pub(super) fn prepare(
        imported: Option<Capture>,
        metadata: Option<u32>,
        fresh: Controls,
    ) -> Result<Self, String> {
        valid_pose(fresh.camera)?;
        if let Some(capture) = imported {
            let raw_kind = u32::from_le_bytes(capture.records()[0][..4].try_into().unwrap());
            let (mode, provenance) = match metadata {
                Some(mode) => {
                    coherent(capture, mode)?;
                    (mode, Attachment::Explicit(mode))
                }
                None if raw_kind == 4 => (3, Attachment::LegacyRawKind4),
                None => (0, Attachment::LegacyOrdinary),
            };
            // LegacyRawKind4 is an explicit modern compatibility attachment.
            // It does not rewrite the absent input metadata or either raw row.
            coherent(capture, mode)?;
            let mut active = Controls::from_raw(&capture.records()[0], raw_kind == 4)?;
            active.camera.vertical_fov = if mode == 3 {
                STRATEGIC_FOV
            } else {
                NORMAL_CITY_VERTICAL_FOV
            };
            // Opaque/unsupported secondary remains retained and savable. The
            // actual return action must preflight it before any raw/UI mutation.
            let normal_return = (raw_kind == 4)
                .then(|| Controls::from_raw(&capture.records()[1], false))
                .transpose();
            Ok(Self {
                active,
                normal_return: normal_return.ok().flatten(),
                imported,
                mode,
                provenance,
                metadata,
            })
        } else {
            if !matches!(metadata, None | Some(0)) {
                return Err("Fresh world cannot invent strategic return storage".into());
            }
            Ok(Self {
                active: fresh,
                normal_return: None,
                imported: None,
                mode: 0,
                provenance: Attachment::Fresh,
                metadata,
            })
        }
    }
    /// Called by actual Intro adoption and supplied-data cold-load tests.
    pub(super) fn from_loaded(
        loaded: &super::LoadedInspection,
        fresh: Controls,
    ) -> Result<Self, String> {
        Self::prepare(loaded.imported_cameras, loaded.saved_prior_mode, fresh)
    }
    pub(super) fn strategic(&self) -> bool {
        self.mode == 3
    }
}

pub(super) struct Navigation {
    pub active: Controls,
    pub normal_return: Option<Controls>,
    pub strategic: bool,
}

/// Only this value owns the process Owner while the legacy UI is active.
pub(super) struct Core {
    owner: camera_live::owner::Owner,
}
impl Core {
    fn legacy_callback_enabled(&self, ui_strategic: bool) -> bool {
        // 004e02d0 skips callbacks for kind 6, selected by 004b9e57.
        // Running mode12 here clamps the start center Z=-595 to -500,
        // moving the otherwise passive overview on its second idle tick.
        !ui_strategic && !matches!(self.kind(), 4 | 6)
    }
    pub(super) fn new(owner: camera_live::owner::Owner) -> Self {
        Self { owner }
    }
    pub(super) fn capture(&self) -> Capture {
        self.owner.capture()
    }
    pub(super) fn kind(&self) -> u32 {
        self.owner.kind()
    }
    pub(super) fn project(&mut self, camera: CameraSample) -> Result<(), String> {
        valid_pose(camera)?; // validate EVERY operand before touching any word
        let mut rows = self.owner.capture().into_records();
        for (at, values) in [(0x4c, camera.eye), (0x58, camera.target), (0x64, camera.up)] {
            for (axis, value) in values.into_iter().enumerate() {
                rows[0][at + 4 * axis..at + 4 * axis + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        self.owner.restore(Capture::from_records(rows));
        Ok(())
    }
    pub(super) fn preflight_return(&self, cached: Option<Controls>) -> Result<Controls, String> {
        let raw = self.owner.capture();
        let secondary = &raw.records()[1];
        let decoded = Controls::from_raw(secondary, false)?;
        if let Some(cached) = cached {
            if !cached.same_pose(secondary) {
                return Err("Legacy return controls disagree with raw secondary".into());
            }
            Ok(cached)
        } else {
            Ok(decoded)
        }
    }
    pub(super) fn navigate(
        &mut self,
        current: Controls,
        cached: Option<Controls>,
        strategic: CameraSample,
    ) -> Result<Navigation, String> {
        if self.kind() == 4 {
            // All required display/control operands preflight before the raw copy.
            let active = self.preflight_return(cached)?;
            self.owner.strategic_after_ui_clear();
            Ok(Navigation {
                active,
                normal_return: None,
                strategic: false,
            })
        } else {
            valid_pose(strategic)?;
            self.project(current.camera)?;
            self.owner.strategic_after_ui_clear();
            let active = Controls {
                camera: strategic,
                selected: None,
                ..current
            };
            Ok(Navigation {
                active,
                normal_return: Some(current),
                strategic: true,
            })
        }
    }
    #[cfg(test)]
    pub(super) fn strategic_after_ui_clear(&mut self) -> StrategicTransition {
        self.owner.strategic_after_ui_clear()
    }
    pub(super) fn into_owner(self) -> camera_live::owner::Owner {
        self.owner
    }
}

pub(super) struct Producer {
    attachment: Attachment,
    input_metadata: Option<u32>,
    core: Core,
    publisher: Publisher,
    failure: Option<String>,
}
impl Producer {
    pub(super) fn attach(
        slot: &mut Option<camera_live::owner::Owner>,
        publisher: Publisher,
        initial: &Initial,
    ) -> Result<Self, String> {
        let mut owner = slot
            .take()
            .ok_or("Process camera Owner is already in use")?;
        // Separate proved reset boundaries, then transport imported rows. No
        // process constructor replay; descriptor/registry custody never changes.
        owner.reset_yaw();
        owner.world_reset_final_fields();
        if let Some(capture) = initial.imported {
            owner.restore(capture);
        }
        let mut value = Self {
            attachment: initial.provenance,
            input_metadata: initial.metadata,
            core: Core::new(owner),
            publisher,
            failure: None,
        };
        if let Err(error) = value.commit(initial.active.camera, initial.mode) {
            let (owner, _) = value.retire();
            *slot = Some(owner);
            return Err(error);
        }
        Ok(value)
    }
    pub(super) fn capture(&self) -> Capture {
        self.core.capture()
    }
    pub(super) fn attachment(&self) -> Attachment {
        self.attachment
    }
    pub(super) fn input_metadata(&self) -> Option<u32> {
        self.input_metadata
    }
    pub(super) fn legacy_callback_enabled(&self, ui_strategic: bool) -> bool {
        self.core.legacy_callback_enabled(ui_strategic)
    }
    pub(super) fn kind(&self) -> u32 {
        self.core.kind()
    }
    pub(super) fn error(&self) -> Option<&str> {
        self.failure.as_deref()
    }
    pub(super) fn invalidate(&mut self, error: String) {
        let _ = self.publisher.invalidate();
        if self.failure.is_none() {
            self.failure = Some(error);
        }
    }
    pub(super) fn commit(&mut self, camera: CameraSample, mode: u32) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        let result = (|| {
            coherent(self.capture(), mode)?;
            self.core.project(camera)?;
            self.publisher.publish(self.capture(), mode)
        })();
        if let Err(error) = &result {
            self.invalidate(error.clone());
        }
        result
    }
    pub(super) fn navigate(
        &mut self,
        current: Controls,
        cached: Option<Controls>,
        strategic: CameraSample,
    ) -> Result<Navigation, String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.core.navigate(current, cached, strategic)
    }
    pub(super) fn retire(mut self) -> (camera_live::owner::Owner, Result<(), String>) {
        let result = self.publisher.retire();
        (self.core.into_owner(), result)
    }
}

/// SAME process/source ownership is moved, never cloned or regenerated.
pub(super) fn transfer(
    old: &mut Option<Producer>,
    new: &mut Option<Producer>,
) -> Result<(), String> {
    if new.is_some() {
        return Err("Refresh cannot replace camera producer".into());
    }
    *new = old.take();
    Ok(())
}

#[cfg(test)]
#[path = "camera_producer_tests.rs"]
mod tests;

/// Identity only. Dynamic XZ/kind/entrance words always come from the admitted
/// Session, never from geometry/Status. None is the genuine unlinked building.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ExpectedBuilding {
    pub building_slot: u32,
    pub business_slot: Option<u32>,
    pub category_id: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Turn {
    pub epoch: openmt2001::level_startup::SourceEpoch,
    pub building: Option<ExpectedBuilding>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Resolved {
    Local,
    Actor(camera_live::selection::SelectedBuilding),
    UnlinkedBlock(u32),
}
/// Revalidate associations at the real FIFO boundary, before its due frame.
/// This function returns owned data; no Session reference reaches reply wait.
pub(super) fn admit(
    session: &crate::fresh_city::Session<'_>,
    turn: Turn,
) -> Result<Resolved, String> {
    if session.world.topology.epoch() != turn.epoch {
        return Err("Camera selection belongs to another World".into());
    }
    let Some(expected) = turn.building else {
        return Ok(Resolved::Local);
    };
    if let Some(business) = expected.business_slot {
        let selected = camera_live::selected_business(session, business)?;
        if selected.building_slot != expected.building_slot
            || selected.category_id != expected.category_id
        {
            return Err("Camera selection live building/category identity changed".into());
        }
        let row = &session.world.buildings[selected.building_slot as usize];
        if row[0x3f] == 0 {
            return Err("Camera selection building retired".into());
        }
        return Ok(Resolved::Actor(selected));
    }
    let row = session
        .world
        .buildings
        .get(expected.building_slot as usize)
        .ok_or("Camera selection building leaves pool")?;
    let word = |at| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
    if row[0x3f] == 0 || word(0x1c) != u32::MAX {
        return Err("Camera selection unlinked building identity changed".into());
    }
    let offset = word(0x40)
        .checked_sub(openmt2001::building_geometry::V14_CATEGORY_BASE)
        .ok_or("Camera selection category precedes arena")?;
    if offset % 0x17c != 0 || offset / 0x17c >= 52 {
        return Err("Camera selection category leaves arena".into());
    }
    let category = (offset / 0x17c) as usize;
    let id = u32::from_le_bytes(session.world.categories[category][..4].try_into().unwrap());
    let chains = openmt2001::building_pool::category_chains(
        &session.world.categories,
        &session.world.buildings,
        0x591318,
    )?;
    if id != expected.category_id || !chains[category].contains(&(expected.building_slot as usize))
    {
        return Err("Camera selection unlinked building/category identity changed".into());
    }
    //4c39ed/4c39f3 distinguishes block information from linked management.
    Ok(Resolved::UnlinkedBlock(id))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Post {
    TutorialControl(u32),
    TutorialPick(crate::tutorial_two_interaction::Pick),
}
pub(super) struct Receipt(Option<std::sync::mpsc::Sender<Result<Vec<Post>, String>>>);
impl Receipt {
    pub(super) fn complete(mut self, result: Result<Vec<Post>, String>) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(result);
        }
    }
}
impl Drop for Receipt {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(Err("Camera turn cancelled before commit".into()));
        }
    }
}
pub(super) struct Payload {
    pub turn: Turn,
    pub input: Resolved,
    pub receipt: Receipt,
}
/// Only transport is retained while waiting. No UI lock, Host or Session loan.
pub(super) fn deliver(
    turn: Turn,
    admitted: Result<Resolved, String>,
    reply: std::sync::mpsc::Sender<Result<Payload, String>>,
) -> Result<Vec<Post>, String> {
    match admitted {
        Err(error) => {
            let _ = reply.send(Err(error.clone()));
            Err(error)
        }
        Ok(input) => {
            let (sender, answer) = std::sync::mpsc::channel();
            reply
                .send(Ok(Payload {
                    turn,
                    input,
                    receipt: Receipt(Some(sender)),
                }))
                .map_err(|_| "Camera UI cancelled before admission".to_owned())?;
            answer
                .recv()
                .map_err(|_| "Camera UI receipt closed".to_owned())?
        }
    }
}
/// Keep the loading frame until the first authored tutorial camera turn has
/// committed. Its delayed event must run before either the city or prompt draws.
#[derive(Default)]
pub(super) struct TutorialStart {
    waiting: bool,
}
impl TutorialStart {
    pub(super) fn new(tutorial: bool) -> Self {
        Self { waiting: tutorial }
    }
    pub(super) fn holds_loading(
        &mut self,
        events_seen: usize,
        turns_pending: bool,
        failed: bool,
    ) -> bool {
        self.waiting &= !failed && (events_seen == 0 || turns_pending);
        self.waiting
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BlockOrigin {
    Selection,
    Navigator,
    Locator,
    StrategicDoubleClick,
    Tutorial,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActorOrigin {
    Scene,
    Locator,
    Adjustment,
    Hyperlink,
    Tutorial,
}
/// The real event callers and CPU witnesses use this same transaction enum.
#[derive(Clone, Copy)]
pub(super) enum Event {
    Deselect,
    Block {
        category: u32,
        selected: crate::selection::Hit,
        origin: BlockOrigin,
    },
    Actor {
        selected: crate::selection::Hit,
        auto_rotate: bool,
        origin: ActorOrigin,
    },
    Compass {
        strategic: CameraSample,
    },
    ChooseView {
        mode: u32,
        strategic: CameraSample,
    },
}
impl Event {
    pub(super) fn script_block(
        target: u32,
        selection: &crate::selection::Assets,
    ) -> Result<Self, String> {
        let category = selection
            .blocks
            .iter()
            .position(|block| block.id == target as usize)
            .ok_or("Script camera category identity missing")?;
        Ok(Self::Block {
            category: target,
            selected: crate::selection::Hit {
                category,
                building: None,
            },
            origin: BlockOrigin::Tutorial,
        })
    }
}
pub(super) struct Pending {
    pub posts: Vec<Post>,
    pub turn: Turn,
    pub event: Event,
    pub reply: std::sync::mpsc::Receiver<Result<Payload, String>>,
    pub adjustment: Option<crate::business_adjustment::State>,
    pub open_adjustment: bool,
}
#[derive(Clone, Copy)]
pub(super) struct View {
    pub active: Controls,
    pub strategic: bool,
    pub normal_return: Option<Controls>,
}
impl View {
    pub(super) fn read(view: &Inspection) -> Self {
        Self {
            active: Controls::read(view),
            strategic: view.strategic,
            normal_return: view.return_camera,
        }
    }
    pub(super) fn put(self, view: &mut Inspection) {
        self.active.put(view);
        view.strategic = self.strategic;
        view.return_camera = self.normal_return;
    }
}
enum RawCommit {
    Prefix,
    None,
    ReturnSecondary,
    EnterCompass(CameraSample),
    EnterPlan,
}
struct Prepared {
    prefix: Capture,
    kind: Option<u32>,
    view: View,
    raw: RawCommit,
}
fn scalar(raw: &RawRecord, at: usize) -> Result<f32, String> {
    let v = f32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
    if !v.is_finite() {
        Err("Camera initializer produced nonfinite legacy controls".into())
    } else {
        Ok(v)
    }
}
fn initialized_controls(
    raw: &RawRecord,
    mut current: Controls,
    building: bool,
    selected: crate::selection::Hit,
    reset: bool,
) -> Result<Controls, String> {
    current.controls = BlockCamera {
        center: [scalar(raw, 0xb4)?, scalar(raw, 0xb8)?, scalar(raw, 0xbc)?],
        angle: scalar(raw, 0x7c)?,
        height: scalar(raw, 0x38)?,
        radius: scalar(raw, 0xb0)?,
    };
    if current.controls.radius <= 0. {
        return Err("Camera initializer has invalid legacy radius".into());
    }
    current.building = building;
    current.building_angle = if building {
        scalar(raw, 0x98)?
    } else {
        current.controls.angle
    };
    current.selected = Some(selected);
    current.camera.vertical_fov = NORMAL_CITY_VERTICAL_FOV;
    current.camera.up = [0., 1., 0.];
    if reset {
        current.inertia = BlockCameraInertia::new(current.camera.eye, current.camera.target);
    }
    valid_pose(current.camera)?;
    Ok(current)
}
fn stage_block(
    category: u32,
    selected: crate::selection::Hit,
    origin: BlockOrigin,
    view: &mut View,
    bank: &mut Bank,
    assets: &crate::selection::Assets,
) -> Result<(), String> {
    if assets.blocks.get(selected.category).map(|b| b.id as u32) != Some(category) {
        return Err("Camera block UI/category identity changed".into());
    }
    let input = camera_live::selection::block(assets, category, 120f32.to_bits(), 80f32.to_bits())?;
    if origin == BlockOrigin::StrategicDoubleClick {
        bank.reset_yaw();
    }
    bank.try_update_primary(|raw| {
        *raw = openmt2001::camera::primary::Primary::initialize_block_words(
            *raw,
            input,
            &mut openmt2001::x87::portable_trig::PortableTrig,
        )
        .map_err(str::to_owned)?;
        Ok::<(), String>(())
    })?;
    view.active = initialized_controls(bank.primary(), view.active, false, selected, true)?;
    view.strategic = false;
    view.normal_return = None;
    Ok(())
}
impl Core {
    fn prepare_event(
        &self,
        mut view: View,
        event: Event,
        input: Resolved,
        assets: &crate::selection::Assets,
    ) -> Result<Prepared, String> {
        let mut bank = Bank::from_capture(self.capture()); // ephemeral transaction only
        let mut kind = None;
        let mut raw = RawCommit::Prefix;
        match event {
            Event::Deselect => {
                view.active.selected = None;
                raw = RawCommit::None;
            }
            Event::Block {
                category,
                selected,
                origin,
            } => {
                stage_block(category, selected, origin, &mut view, &mut bank, assets)?;
                kind = Some(12);
            }
            Event::Actor {
                selected,
                auto_rotate,
                origin,
            } => match input {
                Resolved::Actor(live) => {
                    if assets.blocks.get(selected.category).map(|b| b.id as u32)
                        != Some(live.category_id)
                    {
                        return Err("Camera UI category identity changed".into());
                    }
                    let operand =
                        camera_live::selection::actor(&self.owner, assets, live, auto_rotate)?;
                    bank.initialize_selected_actor(
                        operand,
                        &mut openmt2001::x87::portable_trig::PortableTrig,
                    )
                    .map_err(str::to_owned)?;
                    view.active = initialized_controls(
                        bank.primary(),
                        view.active,
                        true,
                        selected,
                        matches!(
                            origin,
                            ActorOrigin::Locator | ActorOrigin::Hyperlink | ActorOrigin::Tutorial
                        ) || view.strategic,
                    )?;
                    view.strategic = false;
                    view.normal_return = None;
                    kind = Some(13);
                }
                Resolved::UnlinkedBlock(category) => {
                    stage_block(
                        category,
                        crate::selection::Hit {
                            category: selected.category,
                            building: None,
                        },
                        BlockOrigin::Selection,
                        &mut view,
                        &mut bank,
                        assets,
                    )?;
                    kind = Some(12);
                }
                Resolved::Local => {
                    return Err(
                        "Actor camera turn omitted actual live selected-building input".into(),
                    );
                }
            },
            Event::Compass { strategic } => {
                if bank.primary_kind() == 4 {
                    // Unsupported return preflight is recoverable: current P/S
                    // and its savable snapshot remain committed and unchanged.
                    let active = self.preflight_return(view.normal_return)?;
                    raw = RawCommit::ReturnSecondary;
                    bank.restore_primary(*bank.secondary());
                    view = View {
                        active,
                        strategic: false,
                        normal_return: None,
                    };
                } else {
                    valid_pose(strategic)?;
                    raw = RawCommit::EnterCompass(view.active.camera);
                    let mut rows = bank.capture().into_records();
                    for (at, values) in [
                        (0x4c, view.active.camera.eye),
                        (0x58, view.active.camera.target),
                        (0x64, view.active.camera.up),
                    ] {
                        for (axis, value) in values.into_iter().enumerate() {
                            rows[0][at + axis * 4..at + axis * 4 + 4]
                                .copy_from_slice(&value.to_le_bytes());
                        }
                    }
                    bank.restore(Capture::from_records(rows));
                    bank.restore_secondary(*bank.primary());
                    bank.set_immediate_words([-220f32, 1050., -30.].map(f32::to_bits))
                        .map_err(str::to_owned)?;
                    kind = Some(4);
                    view.normal_return = Some(view.active);
                    view.active.camera = strategic;
                    view.active.selected = None;
                    view.strategic = true;
                }
            }
            Event::ChooseView { mode: 3, strategic } => {
                valid_pose(strategic)?;
                //4da030->4b5370(3): no primary->secondary copy.
                raw = RawCommit::EnterPlan;
                bank.set_immediate_words([-220f32, 1050., -30.].map(f32::to_bits))
                    .map_err(str::to_owned)?;
                kind = Some(4);
                view.active.camera = strategic;
                view.active.selected = None;
                view.strategic = true;
                view.normal_return = None;
            }
            Event::ChooseView { mode: 0, .. } => {
                //4da060->4b5370(0): NO raw camera setter/copy. UI mode and
                // raw callback identity are distinct authorities.
                view.strategic = false;
                view.active.camera.vertical_fov = NORMAL_CITY_VERTICAL_FOV;
                raw = RawCommit::None;
            }
            Event::ChooseView { mode, .. } => {
                return Err(format!("Unsupported choose-view mode{mode}"));
            }
        }
        let prefix = bank.capture();
        let mut final_check = Bank::from_capture(prefix);
        if let Some(k) = kind {
            final_check.set_kind(k, &mut 0);
        }
        coherent(final_check.capture(), if view.strategic { 3 } else { 0 })?;
        valid_pose(view.active.camera)?;
        if matches!(raw, RawCommit::None) && !view.active.same_pose(&self.capture().records()[0]) {
            return Err("Camera UI pose changed before no-raw view operation".into());
        }
        Ok(Prepared {
            prefix,
            kind,
            view,
            raw,
        })
    }
    /// Shared completed event transaction, used by the real UI service and its
    /// CPU witnesses. The UI applies returned controls BEFORE sending receipt.
    pub(super) fn event(
        &mut self,
        view: &mut View,
        event: Event,
        input: Resolved,
        assets: &crate::selection::Assets,
    ) -> Result<(), String> {
        let prepared = self.prepare_event(*view, event, input, assets)?;
        match prepared.raw {
            RawCommit::None => {}
            RawCommit::ReturnSecondary => self.owner.copy_secondary_to_primary(),
            RawCommit::EnterCompass(camera) => {
                self.project(camera).expect("preflighted normal pose");
                self.owner.copy_primary_to_secondary();
                self.owner
                    .enter_strategic_fields()
                    .expect("proved finite fixed strategic operands");
            }
            RawCommit::EnterPlan => {
                self.owner
                    .enter_strategic_fields()
                    .expect("proved finite fixed strategic operands");
            }
            RawCommit::Prefix => {
                self.owner.restore(prepared.prefix);
                if let Some(kind) = prepared.kind {
                    self.owner.set_kind(kind);
                }
            }
        }
        if !prepared.view.active.same_pose(&self.capture().records()[0]) {
            self.project(prepared.view.active.camera)
                .expect("all pose operands preflighted");
        }
        *view = prepared.view;
        Ok(())
    }
}

impl Producer {
    pub(super) fn event(
        &mut self,
        view: &mut View,
        event: Event,
        input: Resolved,
        assets: &crate::selection::Assets,
    ) -> Result<(), String> {
        if let Some(error) = self.error() {
            return Err(error.to_owned());
        }
        self.core.event(view, event, input, assets)?;
        self.publisher
            .publish(self.capture(), if view.strategic { 3 } else { 0 })
    }
}

pub(super) fn enqueue(
    queue: &mut std::collections::VecDeque<Pending>,
    sender: &std::sync::mpsc::Sender<super::FreshRequest>,
    turn: Turn,
    event: Event,
    adjustment: Option<crate::business_adjustment::State>,
    open_adjustment: bool,
) -> Result<(), String> {
    let (reply, result) = std::sync::mpsc::channel();
    sender
        .send(super::FreshRequest::CameraTurn { turn, reply })
        .map_err(|_| "Camera selection Session closed".to_owned())?;
    queue.push_back(Pending {
        posts: Vec::new(),
        turn,
        event,
        reply: result,
        adjustment,
        open_adjustment,
    });
    Ok(())
}
/// Actual UI completion seam. All fallible preparation precedes raw/UI writes;
/// closure is an infallible local UI commit, never a reciprocal worker call.
/// Receipt follows BOTH the published raw cut and the real UI fields.
pub(super) fn complete_event(
    producer: &mut Producer,
    current: &mut View,
    turn: Turn,
    event: Event,
    payload: Payload,
    assets: &crate::selection::Assets,
    publish_ui: impl FnOnce(View),
) -> Result<(), String> {
    complete_with_posts(
        producer,
        current,
        turn,
        event,
        payload,
        assets,
        Vec::new(),
        publish_ui,
    )
}
pub(super) fn complete_with_posts(
    producer: &mut Producer,
    current: &mut View,
    turn: Turn,
    event: Event,
    payload: Payload,
    assets: &crate::selection::Assets,
    posts: Vec<Post>,
    publish_ui: impl FnOnce(View),
) -> Result<(), String> {
    let Payload {
        turn: actual,
        input,
        receipt,
    } = payload;
    let result = if turn != actual {
        Err("Camera UI turn identity changed".into())
    } else {
        producer.event(current, event, input, assets)
    };
    match result {
        Ok(()) => {
            publish_ui(*current);
            receipt.complete(Ok(posts));
            Ok(())
        }
        Err(error) => {
            receipt.complete(Err(error.clone()));
            Err(error)
        }
    }
}

/// Actual ordinary UI identity port, shared by all actor callers and tests.
/// Scene contributes identifiers only; not one numeric camera operand is read.
pub(super) fn identity_for_hit(
    scene: &openmt2001::render_scene::Scene,
    assets: &crate::selection::Assets,
    selected: crate::selection::Hit,
) -> Result<ExpectedBuilding, String> {
    let index = selected
        .building
        .ok_or("Actor event requires building identity")?;
    let row = scene
        .inspection_buildings
        .get(index)
        .ok_or("Camera UI building identity missing")?;
    let slot = row
        .runtime_slot
        .or(row.saved_slot)
        .ok_or("Camera UI building has no pool identity")?;
    let category = assets
        .blocks
        .get(selected.category)
        .ok_or("Camera UI category missing")?
        .id as u32;
    if row.category as u32 != category {
        return Err("Camera UI building/category identity disagrees".into());
    }
    let business = row
        .business()
        .map(|b| b.slot)
        .or_else(|| row.initialized_apartment.as_ref().map(|a| a.slot));
    Ok(ExpectedBuilding {
        building_slot: slot as u32,
        business_slot: business.map(|v| v as u32),
        category_id: category,
    })
}
/// Dropping queued payload/receipt owners deterministically cancels each turn.
/// Only true retirement uses this; warm refresh keeps the durable suffix.
pub(super) fn cancel_pending(queue: &mut std::collections::VecDeque<Pending>) {
    queue.clear();
}

/// All game/shared drops finish on the worker; a tiny joiner observes the
/// actual OS-thread join off the UI thread before reporting terminal state.
/// No new World/reset may start while this receipt is pending or failed.
pub(super) type ExitFailures = std::sync::Arc<std::sync::Mutex<Vec<String>>>;
pub(super) fn record_exit(failures: &ExitFailures, error: String) {
    if let Ok(mut errors) = failures.lock() {
        errors.push(error);
    }
}
pub(super) struct WorkerExit {
    receiver: Option<
        std::sync::mpsc::Receiver<Result<Option<crate::process_dispatch::DispatchCustody>, String>>,
    >,
    custody: Option<crate::process_dispatch::DispatchCustody>,
}
impl WorkerExit {
    pub(super) fn take_custody(&mut self) -> Option<crate::process_dispatch::DispatchCustody> {
        self.custody.take()
    }

    pub(super) fn poll(&mut self) -> Option<Result<(), String>> {
        let receiver = self.receiver.as_ref()?;
        let result = match receiver.try_recv() {
            Ok(Ok(custody)) => {
                self.custody = custody;
                Some(Ok(()))
            }
            Ok(Err(error)) => Some(Err(error)),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(
                "Worker terminal channel disconnected without qualified completion".into(),
            )),
        };
        if result.is_some() {
            self.receiver = None;
        }
        result
    }
}
pub(super) fn startup_ready(worker: &Option<WorkerExit>, failure: &Option<String>) -> bool {
    worker.is_none() && failure.is_none()
}
/// This join runs off the UI thread and includes all TLS/shared destructors.
/// A failed joiner spawn leaves a disconnected receipt: it never reports clean.
pub(super) fn supervise(
    worker: std::thread::JoinHandle<()>,
    failures: ExitFailures,
    name: &str,
) -> WorkerExit {
    supervise_worker(worker, failures, name, |()| Ok(None))
}
fn supervise_worker<T: Send + 'static>(
    worker: std::thread::JoinHandle<T>,
    failures: ExitFailures,
    name: &str,
    finish: fn(T) -> Result<Option<crate::process_dispatch::DispatchCustody>, String>,
) -> WorkerExit {
    let (reply, receiver) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new()
        .name(format!("{name}-join"))
        .stack_size(256 * 1024)
        .spawn(move || {
            let joined = worker.join();
            let result = match joined {
                Err(_) => {
                    Err("City worker panicked; terminal success is not established".to_owned())
                }
                Ok(value) => match failures.lock() {
                    Ok(errors) if errors.is_empty() => finish(value),
                    Ok(errors) => Err(errors.join("; ")),
                    Err(_) => Err("Worker completion status poisoned".into()),
                },
            };
            let _ = reply.send(result);
        });
    WorkerExit {
        receiver: Some(receiver),
        custody: None,
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_session(
    name: &str,
    inputs: std::sync::Arc<super::SceneInputs>,
    start: super::WorkerStart,
    camera: CameraSample,
    tx: std::sync::mpsc::Sender<Result<super::LoadedInspection, crate::load::Failure>>,
    pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
    focus: bool,
) -> Result<WorkerExit, String> {
    let failures = ExitFailures::default();
    let status = std::sync::Arc::clone(&failures);
    let worker = std::thread::Builder::new()
        .name(name.into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            inputs.session_worker_with_terminal(start, camera, tx, pavement, focus, true, status);
            // inputs and every Session/OfflineLease drop before the joiner replies.
        })
        .map_err(|e| e.to_string())?;
    Ok(supervise(worker, failures, name))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_session_joined(
    name: &str,
    inputs: std::sync::Arc<super::SceneInputs>,
    start: super::WorkerStart,
    camera: CameraSample,
    tx: std::sync::mpsc::Sender<Result<super::LoadedInspection, crate::load::Failure>>,
    pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
    focus: bool,
    custody: Option<crate::process_dispatch::DispatchCustody>,
) -> Result<WorkerExit, String> {
    let failures = ExitFailures::default();
    let status = std::sync::Arc::clone(&failures);
    let worker = std::thread::Builder::new()
        .name(name.into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            inputs.session_worker_joined(start, camera, tx, pavement, focus, true, status, custody)
            // inputs and every Session/OfflineLease drop before the joiner replies.
        })
        .map_err(|e| e.to_string())?;
    Ok(supervise_worker(worker, failures, name, |custody| {
        custody
            .map(Some)
            .ok_or_else(|| "Worker failed to return actual dispatch custody".into())
    }))
}

pub(super) struct OfflineLease {
    pub process: Option<crate::live_host::process_runtime::Handle>,
    pub failures: ExitFailures,
}
impl Drop for OfflineLease {
    fn drop(&mut self) {
        if let Some(process) = &self.process {
            if let Err(error) = process.network_teardown4e1c50() {
                record_exit(
                    &self.failures,
                    format!("Shared offline teardown failed: {error}"),
                );
            }
        }
    }
}
/// Typed, same-Session continuation only. No UI queue is consulted or replayed.
pub(super) fn run_posts(
    host: &mut crate::live_host::Host,
    session: &mut crate::fresh_city::Session<'_>,
    assets: &crate::fresh_city::CommandAssets<'_, '_>,
    posts: Vec<Post>,
    failures: &ExitFailures,
) -> Result<(), String> {
    for post in posts {
        let result = match post {
            Post::TutorialControl(id) => host.tutorial_event(session, assets, id),
            Post::TutorialPick(pick) => host.tutorial_pick(session, assets, pick).map(|_| ()),
        };
        if let Err(error) = result {
            let partial =
                format!("Camera committed; completion post-action {post:?} failed: {error}");
            record_exit(failures, partial.clone());
            host.fail(partial.clone());
            return Err(partial);
        }
    }
    Ok(())
}

/// Actual Intro refresh request boundary. Pending camera turns are durable:
/// Inspect follows them on the same FIFO, while this UI continues servicing.
#[allow(clippy::too_many_arguments)]
pub(super) fn request_refresh(
    mut view: Option<&mut Inspection>,
    selection: &crate::selection::Assets,
    adjustment: Option<&crate::business_adjustment::Assets>,
    loading: &mut Option<
        std::sync::mpsc::Receiver<Result<super::LoadedInspection, crate::load::Failure>>,
    >,
    refreshing: &mut bool,
    retry: &mut super::refresh::Retry,
) -> Result<(), String> {
    if let Some(view) = view.as_deref_mut() {
        view.service_camera_turns(selection, adjustment);
    }
    if loading.is_some() {
        return Err("a city snapshot is already pending".into());
    }
    let view = view.ok_or("no city is open")?;
    let sender = view
        ._fresh_session
        .as_ref()
        .ok_or("city has no retained fresh session")?;
    *loading = Some(retry.request(sender, view.camera)?);
    *refreshing = true;
    Ok(())
}
/// Concrete camera owners/cursor in the actual successful warm commit. Other
/// overlay owners remain in Intro's existing refresh transaction. The mapped
/// presentation selection/return tuple has already been prepared by its caller.
pub(super) fn commit_refresh(previous: &mut Inspection, next: &mut Inspection) {
    let mapped_selection = next.selected;
    Controls::read(previous).put(next);
    next.selected = mapped_selection;
    next.strategic = previous.strategic;
    transfer(&mut previous.camera_producer, &mut next.camera_producer)
        .expect("preflighted empty refresh producer destination");
    next.camera_ready = previous.camera_ready.take();
    next.camera_epoch = previous.camera_epoch;
    next.camera_turns = std::mem::take(&mut previous.camera_turns);
    next.tutorial_camera_seen = previous.tutorial_camera_seen;
    next.tutorial_start = std::mem::take(&mut previous.tutorial_start);
}
