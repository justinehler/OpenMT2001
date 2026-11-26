//! Pure proposed Game.End transaction model. No Host/intro wiring, teardown,
//! UI, filesystem or save-schema integration. Acknowledgments below are caller
//! claims; this model does not implement or prove the original suppliers.
use crate::{level_catalog::Catalog, outcome_notice::Sequence};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    session: u64,
    registration: usize,
    level: u32,
    difficulty: u32,
    profile: Option<PathBuf>,
}

impl Identity {
    /// Capture the session's owned identity, never the current menu selection.
    /// Registration index matters because native catalogs permit duplicate IDs.
    pub fn capture(session: u64, catalog: &Catalog, profile: Option<&Path>) -> Result<Self, Error> {
        let registration = catalog.selected.ok_or(Error::Selection)?;
        let level = catalog.levels.get(registration).ok_or(Error::Selection)?;
        if catalog.difficulty >= 3 {
            return Err(Error::Difficulty);
        }
        Ok(Self {
            session,
            registration,
            level: level.id(),
            difficulty: catalog.difficulty,
            profile: profile.map(Path::to_path_buf),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair {
    /// Already converted by the native Lua boundary; not a raw Lua value.
    pub completed: bool,
    /// Native signed FTOL low dword; only exactly1 awards a non-tutorial win.
    pub secondary: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Direct,
    Tutorial,
    Result(Sequence),
}

/// Host evidence required for an existing actual result path. Merely creating
/// this value does not prove that an authored scene has run.
pub struct ResultGate {
    pub request: Sequence,
    pub presentation_finished: bool,
    pub authored_end: Option<Pair>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnTarget {
    /// Original43be20 receiver; names/resource labels require UI evidence.
    Panel(u32),
    Background491720,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReturnContext {
    pub sequence_active_8b0700: bool,
    pub transition_8fca9c: u32,
    pub network_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Captured,
    PreRoutingAcknowledged,
    TutorialAwardApplied,
    RoutePresented,
    NonTutorialAwardApplied,
    TailAcknowledged,
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Selection,
    Difficulty,
    IdentityChanged,
    TutorialOrigin,
    ResultNotReady,
    ResultEndMismatch,
    WrongPhase,
    WrongAwardBranch,
    ReturnChanged,
    Presentation(String),
    Progress(String),
    Persistence(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub identity: Identity,
    pub pair: Pair,
    pub origin: Origin,
    pub target: ReturnTarget,
}

pub struct Transaction {
    identity: Identity,
    pair: Pair,
    origin: Origin,
    phase: Phase,
    target: Option<ReturnTarget>,
}

impl Transaction {
    pub fn new(identity: Identity, pair: Pair, origin: Origin) -> Self {
        Self {
            identity,
            pair,
            origin,
            phase: Phase::Captured,
            target: None,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    fn validate(&self, live: &Identity, catalog: &Catalog) -> Result<(), Error> {
        if live != &self.identity
            || Identity::capture(live.session, catalog, live.profile.as_deref())? != self.identity
        {
            return Err(Error::IdentityChanged);
        }
        Ok(())
    }

    /// Literal4ba2a4..4ba38a branching. This returns a target only; it never
    /// shows a panel or acknowledges an unimplemented UI/teardown operation.
    pub fn return_target(
        &self,
        live: &Identity,
        catalog: &Catalog,
        context: ReturnContext,
    ) -> Result<ReturnTarget, Error> {
        self.validate(live, catalog)?;
        let level = &catalog.levels[self.identity.registration];
        if self.pair.completed {
            return Ok(ReturnTarget::Panel(if level.tutorial() {
                0x88d384
            } else {
                0x8898e4
            }));
        }
        Ok(
            if context.sequence_active_8b0700 || context.transition_8fca9c != 0 {
                ReturnTarget::Background491720
            } else if level.tutorial() {
                ReturnTarget::Panel(0x88d384)
            } else if level.single_player() && level.bytes[2] == 0 {
                ReturnTarget::Panel(0x885e7c)
            } else if level.single_player() && level.bytes[2] == 1 {
                ReturnTarget::Panel(0x886338)
            } else if context.network_enabled {
                ReturnTarget::Panel(0x86da48)
            } else {
                ReturnTarget::Background491720
            },
        )
    }

    /// Caller acknowledges only4ba1d0..4ba2a4 effects, before either the
    /// tutorial award or return presentation. No panel has been shown here.
    pub fn acknowledge_pre_routing(
        &mut self,
        live: &Identity,
        catalog: &Catalog,
        gate: Option<&ResultGate>,
    ) -> Result<(), Error> {
        self.validate(live, catalog)?;
        if self.origin == Origin::Tutorial && !catalog.levels[self.identity.registration].tutorial()
        {
            return Err(Error::TutorialOrigin);
        }
        if let Origin::Result(sequence) = self.origin {
            let gate = gate.ok_or(Error::ResultNotReady)?;
            if gate.request != sequence || !gate.presentation_finished {
                return Err(Error::ResultNotReady);
            }
            let secondary = if sequence == Sequence::Lose { 2 } else { 1 };
            if self.pair
                != (Pair {
                    completed: true,
                    secondary,
                })
                || gate.authored_end != Some(self.pair)
            {
                return Err(Error::ResultEndMismatch);
            }
        }
        if self.phase == Phase::Captured {
            self.phase = Phase::PreRoutingAcknowledged;
        }
        Ok(())
    }

    /// Native4ba2c0 publishes tutorial completion before4ba2ca opens the
    /// Tutorial picker, whose49c890 selection reads those completion bytes.
    pub fn apply_tutorial_award(
        &mut self,
        live: &Identity,
        catalog: &mut Catalog,
    ) -> Result<(), Error> {
        self.validate(live, catalog)?;
        if !self.pair.completed || !catalog.levels[self.identity.registration].tutorial() {
            return Err(Error::WrongAwardBranch);
        }
        match self.phase {
            Phase::PreRoutingAcknowledged => (),
            Phase::TutorialAwardApplied
            | Phase::RoutePresented
            | Phase::TailAcknowledged
            | Phase::Complete => return Ok(()),
            _ => return Err(Error::WrongPhase),
        }
        self.stage_progress(catalog)?;
        self.phase = Phase::TutorialAwardApplied;
        Ok(())
    }

    /// Explicit route-presentation boundary. The supplied observer/provider
    /// sees published tutorial completion, or pre-award non-tutorial Graph
    /// state. No live provider is implemented by this dormant core model.
    pub fn acknowledge_route_presentation(
        &mut self,
        live: &Identity,
        catalog: &Catalog,
        context: ReturnContext,
        mut present: impl FnMut(ReturnTarget, &Catalog) -> Result<(), String>,
    ) -> Result<(), Error> {
        self.validate(live, catalog)?;
        let target = self.return_target(live, catalog, context)?;
        if matches!(
            self.phase,
            Phase::RoutePresented
                | Phase::NonTutorialAwardApplied
                | Phase::TailAcknowledged
                | Phase::Complete
        ) {
            return if self.target == Some(target) {
                Ok(())
            } else {
                Err(Error::ReturnChanged)
            };
        }
        let ready = if self.pair.completed && catalog.levels[self.identity.registration].tutorial()
        {
            Phase::TutorialAwardApplied
        } else {
            Phase::PreRoutingAcknowledged
        };
        if self.phase != ready {
            return Err(Error::WrongPhase);
        }
        present(target, catalog).map_err(Error::Presentation)?;
        self.target = Some(target);
        self.phase = Phase::RoutePresented;
        Ok(())
    }

    /// Native4ba2d9 opens Graph before4ba2de..4ba30c mutates non-tutorial
    /// attempts/cup/successor. Abort and tutorial branches have no award here.
    pub fn apply_non_tutorial_award(
        &mut self,
        live: &Identity,
        catalog: &mut Catalog,
    ) -> Result<(), Error> {
        self.validate(live, catalog)?;
        if !self.pair.completed || catalog.levels[self.identity.registration].tutorial() {
            return Err(Error::WrongAwardBranch);
        }
        match self.phase {
            Phase::RoutePresented => (),
            Phase::NonTutorialAwardApplied | Phase::TailAcknowledged | Phase::Complete => {
                return Ok(());
            }
            _ => return Err(Error::WrongPhase),
        }
        self.stage_progress(catalog)?;
        self.phase = Phase::NonTutorialAwardApplied;
        Ok(())
    }

    fn stage_progress(&self, catalog: &mut Catalog) -> Result<(), Error> {
        // Stage an isolated candidate so a Catalog error cannot partially
        // increment attempts. This is modern retry protection, not native I/O.
        let mut candidate = catalog.clone();
        candidate
            .finish(self.pair.completed, self.pair.secondary == 1)
            .map_err(Error::Progress)?;
        *catalog = candidate;
        Ok(())
    }

    /// Caller completed original reset-world/queue/script-event-clear tail.
    pub fn acknowledge_tail(&mut self, live: &Identity, catalog: &Catalog) -> Result<(), Error> {
        self.validate(live, catalog)?;
        if matches!(self.phase, Phase::TailAcknowledged | Phase::Complete) {
            return Ok(());
        }
        let ready = if self.pair.completed && !catalog.levels[self.identity.registration].tutorial()
        {
            Phase::NonTutorialAwardApplied
        } else {
            Phase::RoutePresented
        };
        if self.phase != ready {
            return Err(Error::WrongPhase);
        }
        self.phase = Phase::TailAcknowledged;
        Ok(())
    }

    /// Injected owned-profile persistence, never filesystem access here. A
    /// failed write leaves TailAcknowledged, so retry repeats only persistence.
    /// No campaign profile means no campaign-file creation on Skirmish return.
    pub fn persist(
        &mut self,
        live: &Identity,
        catalog: &Catalog,
        mut write: impl FnMut(&Catalog, &Path) -> Result<(), String>,
    ) -> Result<Completion, Error> {
        self.validate(live, catalog)?;
        if !matches!(self.phase, Phase::TailAcknowledged | Phase::Complete) {
            return Err(Error::WrongPhase);
        }
        if self.phase != Phase::Complete {
            if let Some(profile) = self.identity.profile.as_deref() {
                write(catalog, profile).map_err(Error::Persistence)?;
            }
            self.phase = Phase::Complete;
        }
        Ok(Completion {
            identity: self.identity.clone(),
            pair: self.pair,
            origin: self.origin,
            target: self.target.ok_or(Error::WrongPhase)?,
        })
    }
}
