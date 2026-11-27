//! Event-thread endpoint; the real event loop implements Context.
//! No clock, cloned raw authority, status-derived mode or sample-on-save.
use super::owner::{Input, Owner, Pose};
use crate::live_host::camera_bridge::{
    self as bridge, Admission, Inbox, NativeOperation, Observation, Operation, Service,
    SessionServices, TaskEffect,
};
use openmt2001::camera::{
    bank::{Capture, StrategicTransition},
    primary::CursorRequest,
};

/// Scheduler operations are staged locally, never committed or sent as worker
/// commands by these callbacks. A completed CentralTail releases them to
/// its acknowledgement, including a completed boundary with a finalizer fault. Other operations must not stage scheduler effects.
/// This finite contract does not promise rollback of camera/UI/cursor effects.
pub trait Context {
    /// Read only the actual UI view; no Session, input, provider or sample call.
    fn save_interface_mode(&self) -> Result<u32, String>;
    /// Read-only operation preflight, before suppliers, raw restore, UI intents
    /// or input consumption. A rejected request has no Context side effects.
    fn validate(&self, operation: &Operation, owner: &Owner) -> Result<(), String>;
    fn admission(&mut self, value: &Admission) -> Result<(), String>;
    ///Absent only for legacy fixture Contexts. Production lends Assets source.
    fn process_runtime(&self) -> Option<&crate::live_host::process_runtime::Handle> {
        None
    }
    /// Drain queued UI camera/save intents here, in event order, while the
    /// worker is suspended at the actual central pre-camera boundary. Event
    /// handlers must not mutate the raw authority during a running logic pass.
    fn pre_camera(&mut self, owner: &mut Owner, value: &Admission) -> Result<(), String>;
    fn input(&mut self, value: &Admission) -> Result<Input, String>;
    fn consumed_input(&mut self);
    fn cursor(&mut self, requests: Vec<CursorRequest>) -> Result<(), String>;
    /// Full ordered navigation transaction, including primary focus clear
    /// before kind read and mode3 UI phases around the raw camera setters.
    fn navigate(&mut self, owner: &mut Owner) -> Result<(), String>;
    /// Resolve current actor/model data and perform only the recovered ordered
    /// setter/initializer/UI operations; no legacy pose reconstruction.
    fn tutorial(&mut self, operation: NativeOperation, owner: &mut Owner) -> Result<(), String>;
    /// Actual pending sound/listener work from current vectors, before Sample.
    /// This must not wait on FreshRequest while the worker waits on this ack.
    fn pending_sound(&mut self, current: Pose) -> Result<(), String>;
    fn present(&mut self, sampled: Pose) -> Result<(), String>;
    fn take_strategic_writeback(
        &mut self,
    ) -> Result<Option<crate::locator::strategic::Writeback>, String>;
    fn take_task_effects(&mut self) -> Vec<TaskEffect>;
    /// Separate from staging: only the final successful boundary publishes saves.
    fn publish_save_captures(&mut self);
    fn discard_task_effects(&mut self);
}
/// Required final-join hooks for the service-enabled route. No production
/// implementation/default is installed. The real Context keeps mutable Assets/
/// commonFonts on the UI stack and lends SessionServices only to its Provider.
/// Legacy pre_camera (including its old focus tick) is SKIPPED on this route.
/// Host aggregate/interface effects must be relocated before admitting it.
pub trait SessionContext: Context {
    /// Execute THIS operation's actual prefix and continuation once. Drain
    /// intents only at their recovered boundary. BeforeTail is not an ack.
    fn groups_and_continuation(
        &mut self,
        owner: &mut Owner,
        value: &Admission,
        services: &mut SessionServices<'_>,
    ) -> Result<super::continuation::BeforeTail, String>;
    /// Consume that receipt and run SAME Focus43e3b0 exactly once. Camera mode/
    /// common, completion effects and save publication remain Endpoint's tail.
    fn focus_after_continuation(
        &mut self,
        owner: &mut Owner,
        value: &Admission,
        before: super::continuation::BeforeTail,
        services: &mut SessionServices<'_>,
    ) -> Result<(), String>;
}
pub struct PropertyInstall {
    model: super::property_descriptors::PavementModel,
    reset: super::property_descriptors::ResetTransform,
}
impl PropertyInstall {
    pub fn capture(
        assets: &crate::selection::Assets,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            model: super::property_descriptors::PavementModel::bind(assets)?,
            reset: super::property_descriptors::ResetTransform::capture(image)?,
        })
    }
    #[cfg(test)]
    pub(super) fn synthetic(assets: &crate::selection::Assets) -> Self {
        Self {
            model: super::property_descriptors::PavementModel::bind(assets).unwrap(),
            reset: super::property_descriptors::ResetTransform::synthetic(),
        }
    }
}
pub struct Endpoint {
    owner: Owner,
    inbox: Inbox,
    admission: Admission,
    property_install: Option<PropertyInstall>,
}
impl From<Pose> for bridge::Pose {
    fn from(p: Pose) -> Self {
        Self {
            eye: p.eye,
            target: p.target,
            up: p.up,
        }
    }
}
impl Endpoint {
    /// Owner is the caller's existing process authority, including its external
    /// kind-dirty byte. Install restores the pair; it never fabricates that byte.
    pub fn new(owner: Owner, inbox: Inbox) -> Self {
        Self {
            owner,
            inbox,
            admission: Admission::default(),
            property_install: None,
        }
    }
    pub fn with_property_install(
        owner: Owner,
        inbox: Inbox,
        property_install: PropertyInstall,
    ) -> Self {
        Self {
            owner,
            inbox,
            admission: Admission::default(),
            property_install: Some(property_install),
        }
    }
    pub fn capture(&self) -> Capture {
        self.owner.capture()
    }
    #[cfg(test)]
    pub(crate) fn test_exhaust_property_generation(&mut self) {
        self.owner
            .property_descriptors_mut()
            .unwrap()
            .test_exhaust_generation();
    }
    pub fn owner(&self) -> &Owner {
        &self.owner
    }
    #[cfg(test)]
    pub(crate) fn camera_only_admission(&self) -> bool {
        self.admission.upper.is_none() && self.admission.strategic.is_none()
    }
    /// Explicit normal city retirement; keep every raw byte and external dirty
    /// byte while waking this generation's pending worker request.
    pub fn into_owner(self) -> Owner {
        self.retire_into_owner(|_| {})
    }
    /// Shared production retirement helper. Tests observe the exact cut after
    /// source invalidation and before the Inbox flag/reply senders are released.
    fn retire_into_owner(self, before_wake: impl FnOnce(&Owner)) -> Owner {
        let Self {
            mut owner, inbox, ..
        } = self;
        owner.retire_resident_source();
        owner.retire_property_source();
        owner.trace_property_lifecycle("retired-before-wake");
        before_wake(&owner);
        inbox.retire();
        owner
    }
    pub fn service(&mut self, context: &mut impl Context) -> Result<Service, String> {
        self.service_with(context, None, |context, owner, value, _| {
            context.pre_camera(owner, value)
        })
    }
    pub fn wait_service(
        &mut self,
        context: &mut impl Context,
        timeout: std::time::Duration,
    ) -> Result<Service, String> {
        self.service_with(context, Some(timeout), |context, owner, value, _| {
            context.pre_camera(owner, value)
        })
    }
    /// Inert until a real SessionContext is installed. Same Inbox/generation/
    /// sequence, raw Owner, admission validation, camera/common and completion
    /// finalizer; no alternative queue, snapshot or camera authority.
    pub fn service_session(
        &mut self,
        context: &mut impl SessionContext,
    ) -> Result<Service, String> {
        self.service_session_with(context, None)
    }
    pub fn wait_service_session(
        &mut self,
        context: &mut impl SessionContext,
        timeout: std::time::Duration,
    ) -> Result<Service, String> {
        self.service_session_with(context, Some(timeout))
    }
    fn service_session_with<C: SessionContext>(
        &mut self,
        context: &mut C,
        wait: Option<std::time::Duration>,
    ) -> Result<Service, String> {
        self.service_with(context, wait, |context, owner, value, services| {
            let before = context.groups_and_continuation(owner, value, services)?;
            services.check().map_err(|e| e.to_string())?;
            context.focus_after_continuation(owner, value, before, services)
        })
    }
    fn service_with<C: Context>(
        &mut self,
        context: &mut C,
        wait: Option<std::time::Duration>,
        pre_camera: impl FnOnce(
            &mut C,
            &mut Owner,
            &Admission,
            &mut SessionServices<'_>,
        ) -> Result<(), String>,
    ) -> Result<Service, String> {
        let owner = &mut self.owner;
        let admission = &mut self.admission;
        let property_install = &self.property_install;
        let generation = self.inbox.generation();
        let apply = |operation, services: &mut SessionServices<'_>| {
            // Do not discard queued intents/replies on a read-only rejection.
            if let Operation::Install { property, .. } = &operation {
                let preflight: Result<(), String> = (|| {
                    let source = property
                        .as_deref()
                        .ok_or("Install requires completed Session property source")?;
                    let resources = property_install
                        .as_ref()
                        .ok_or("Install property resource/reset producer absent")?;
                    owner.validate_property_install(source, generation, &resources.model)?;
                    // Synthetic property fixtures have no resident authority.
                    // Real capture always carries SAME Session weak origin.
                    if let Some(resident) = source.resident_for_install()? {
                        owner.validate_resident_install(resident, generation)?;
                    }
                    Ok(())
                })();
                if let Err(error) = preflight {
                    owner.latch_property_failure(error.clone());
                    return Err(error);
                }
            }
            context.validate(&operation, owner)?;
            let result = (|| {
                let mut sampled = None;
                let mut raw_capture = None;
                let mut task_effects = Vec::new();
                let central = matches!(&operation, Operation::CentralTail { .. });
                let installing = matches!(&operation, Operation::Install { .. });
                let protected_writeback = matches!(
                    &operation,
                    Operation::Install { .. } | Operation::CentralTail { .. }
                );
                match operation {
                    Operation::Install {
                        capture,
                        admission: value,
                        property,
                    } => {
                        let source = property
                            .as_deref()
                            .ok_or("Install property source absent")?;
                        let resources = property_install
                            .as_ref()
                            .ok_or("Install property resources absent")?;
                        owner.install_property_source(
                            source,
                            generation,
                            &resources.model,
                            &resources.reset,
                        )?;
                        if let Some(resident) = source.resident_for_install()? {
                            owner.install_resident_source(resident, generation)?;
                        }
                        context.admission(&value)?;
                        *admission = value;
                        // Only after epoch/resource/allocation preflight and install.
                        owner.reset_yaw();
                        owner.world_reset_final_fields();
                        if let Some(process) = context.process_runtime() {
                            process
                                .world_reset_post_camera4b9eb1()
                                .map_err(|e| e.to_string())?;
                        }
                        owner.restore(capture);
                    }
                    Operation::Admission(value) => {
                        context.admission(&value)?;
                        *admission = value;
                    }
                    Operation::Apply {
                        operation,
                        admission: value,
                    } => {
                        context.admission(&value)?;
                        *admission = value;
                        match operation {
                            NativeOperation::SetKind(kind) => owner.set_kind(kind),
                            NativeOperation::SetTargetWords(eye) => owner.target(eye),
                            NativeOperation::SetImmediateWords(eye) => owner.immediate(eye)?,
                            NativeOperation::ResetYaw => owner.reset_yaw(),
                            NativeOperation::WorldResetFinalFields => {
                                owner.world_reset_final_fields();
                                if let Some(process) = context.process_runtime() {
                                    process
                                        .world_reset_post_camera4b9eb1()
                                        .map_err(|e| e.to_string())?;
                                }
                            }
                            NativeOperation::StrategicAfterUiClear => context.navigate(owner)?,
                            tutorial => context.tutorial(tutorial, owner)?,
                        }
                    }
                    Operation::CentralTail { admission: value } => {
                        context.admission(&value)?;
                        *admission = value;
                        pre_camera(context, owner, admission, services)?;
                        // A provider cannot swallow a failed callback then enter
                        // camera/common/save. Earlier UI/Host effects stay committed.
                        services.check().map_err(|e| e.to_string())?;
                        let input = context.input(admission)?;
                        let cursors = owner.central(input)?;
                        context.consumed_input();
                        context.cursor(cursors)?;
                        task_effects = context.take_task_effects();
                    }
                    Operation::RawCapture => {
                        raw_capture = Some(bridge::SaveCapture {
                            cameras: owner.capture(),
                            interface_mode: context.save_interface_mode(),
                        });
                    }
                    Operation::CurrentPose => {}
                    Operation::Sample { ratio_bits } => {
                        // One frame boundary after the manager, not a side effect
                        // of arbitrary current-pose queries inside logic work.
                        context.pending_sound(owner.current())?;
                        let pose = owner.sample(f32::from_bits(ratio_bits))?;
                        context.present(pose)?;
                        sampled = Some(pose.into());
                    }
                }
                // Core work completed. Even an unexpected late extractor failure
                // must cross the direct ack with its completed scheduler effects.
                let (strategic_writeback, finalization_fault) = if protected_writeback {
                    match context.take_strategic_writeback() {
                        Ok(writeback) => (writeback, None),
                        Err(error) => {
                            context.discard_task_effects();
                            (None, Some(error))
                        }
                    }
                } else {
                    (None, None)
                };
                if central && finalization_fault.is_none() {
                    context.publish_save_captures();
                }
                if installing && finalization_fault.is_none() {
                    owner.trace_property_lifecycle("installed-source");
                }
                // Preserve the existing observation shape without a CurrentPose
                // operation/method on the raw-save branch. These redundant MOV
                // words come only from the already captured primary record.
                let current = if let Some(raw) = &raw_capture {
                    let record = &raw.cameras.records()[0];
                    let triple = |at: usize| {
                        std::array::from_fn(|i| {
                            u32::from_le_bytes(
                                record[at + i * 4..at + i * 4 + 4].try_into().unwrap(),
                            )
                        })
                    };
                    bridge::Pose {
                        eye: triple(4),
                        target: triple(0x10),
                        up: triple(0x64),
                    }
                } else {
                    owner.current().into()
                };
                Ok(Observation {
                    raw_capture,
                    current,
                    sampled,
                    task_effects,
                    strategic_writeback,
                    finalization_fault,
                })
            })();
            if result.is_err() {
                context.discard_task_effects();
            }
            result
        };
        if let Some(timeout) = wait {
            self.inbox.wait_service_with_handler(timeout, apply)
        } else {
            self.inbox.try_service_with_handler(apply)
        }
    }
}

#[cfg(test)]
#[path = "endpoint/tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "endpoint/source_lifecycle_tests.rs"]
pub(crate) mod source_lifecycle_tests;

#[cfg(test)]
#[path = "endpoint/resident_lifecycle_tests.rs"]
pub(crate) mod resident_lifecycle_tests;
