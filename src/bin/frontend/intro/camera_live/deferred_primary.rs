//! Fresh Install-delayed43cc60 prefix only. No full native startup claim.
use super::{
    main_construction::{CompletedMainLayout, ResourceIdentity},
    navigation::Group,
    panel_models::CpuModel,
    primary_d_construction::{
        Currency, DProduct, InstallDisposition, PrimaryConstruction, PrimaryPhase,
    },
    primitive_factory::Primitives,
    process_images::Images,
    registration::{self, Descriptor},
    ui_text_resources::UiTextResources,
};
use crate::live_host::camera_bridge::{ResidentSource, SessionServices};
use openmt2001::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage};

/// Neither this enum nor its completed arm converts to DeferredPrimary.
/// Previously executed F/Main effects remain explicitly reordered.
pub enum PrimaryProducts {
    Reordered(CompletedMainLayout),
    Deferred(DeferredPrimary),
}
impl PrimaryProducts {
    pub fn reordered(&self) -> Result<&CompletedMainLayout, String> {
        match self {
            Self::Reordered(main) => Ok(main),
            Self::Deferred(_) => Err("Fresh Main has not reached41dcf0".into()),
        }
    }
    pub fn require_ordered_main(&self) -> Result<(), String> {
        match self {
            Self::Reordered(_) => {
                Err("Reordered eager F/Main cannot convert into a fresh ordered prefix".into())
            }
            Self::Deferred(primary) => primary.require_ordered_main(),
        }
    }
}
/// Retained original preimages and aliases for SAME statically born A/child0.
/// No typed Main layout tree exists before its actual body is reached. There
/// is deliberately no method to manufacture a completed/empty Main here.
pub struct MainBirth {
    pub source: u32,
    pub caption: u32,
    pub child0_source: u32,
    pub static_preimage: Vec<u8>,
    pub child0_preimage: Vec<u8>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainBirthPhase {
    StaticPending,
    Unbound,
    FailedBeforeLayout,
}
pub struct MainSlot {
    birth: Box<MainBirth>,
    phase: MainBirthPhase,
}
impl MainSlot {
    pub fn phase(&self) -> MainBirthPhase {
        self.phase
    }
    pub fn retained_birth(&self) -> &MainBirth {
        &self.birth
    }
    pub fn unbound_birth(&self) -> Result<&MainBirth, String> {
        if self.phase == MainBirthPhase::Unbound {
            Ok(&self.birth)
        } else {
            Err(
                "Main static birth incomplete; retained preimages are not completed constructors"
                    .into(),
            )
        }
    }
}
pub struct DeferredPrimary {
    d: PrimaryConstruction,
    main: MainSlot,
    voting: Box<super::voting_construction::VotingProduct>,
    eventlist: Box<super::eventlist_construction::EventListProduct>,
    chat: Box<super::chat_construction::ChatProduct>,
    process: ResourceIdentity,
    text: ResourceIdentity,
    images: ResourceIdentity,
}
impl DeferredPrimary {
    /// Only the cold loader can supply an unclaimed deferred allocation.
    /// Parsing and source validation precede claim. Reached births retain
    /// their partial owner and return Failed, never an invisible local drop.
    pub(crate) fn allocate(
        executable: &[u8],
        index: &ArchiveIndex,
        archive: &[u8],
        p: &mut Primitives,
        text: &UiTextResources,
        images: &Images,
    ) -> Result<Self, String> {
        p.validate_deferred_unclaimed()?;
        if images.ui_dimensions != [640, 480] || text.board_stem.is_none() {
            return Err(
                "Fresh Primary requires actual640 resources and installed board language".into(),
            );
        }
        let currency = Currency::from_language(text)?;
        let pe = PeImage::parse(executable)?;
        for (at, n) in [
            (0x439840, 0x78),
            (0x439ae0, 0x80),
            (0x42a370, 0x285),
            (0x4df9c0, 0x7f),
            (0x43a770, 0x83),
            (0x41a510, 0x40a),
            (0x5258c0, 0x3c),
            (0x43f340, 0x1c5),
            (0x5259f4, 0x28),
            (0x41a190, 0x380),
            (0x442960, 0x16d),
            (0x439ce0, 0x77),
            (0x41cdc0, 0x1e2),
            (0x5256e0, 0x3c),
            (0x534a08, 0x50),
            (0x439df0, 0xf0),
            (0x42c000, 0x421),
            (0x43ed20, 0x144),
            (0x442600, 0xce),
            (0x52571c, 0x3c),
            (0x5259a4, 0x34),
            (0x43cc60, 0x17d),
            (0x43b770, 0x310),
            (0x43e9a0, 0x197),
            (0x5255f0, 0x3c),
            (0x525908, 0x34),
            (0x559730, 48),
        ] {
            if pe.at(at, n)? != p.cells().read(at, n)? {
                return Err(format!(
                    "Fresh Primary source{at:x} differs from same PE allocation"
                ));
            }
        }
        if p.cells().word(0x5255f4)? != 0x42a370 || p.cells().word(0x52590c)? != 0x42f140 {
            return Err("D/Text actual slot04 differs".into());
        }
        let mut models = Vec::new();
        //F is already parsed once by Navigator's cold loader. Every other
        //source is loaded independently, including the distinct A/G payloads.
        for ordinal in 1..16u8 {
            let entry = registration::PRIMARY
                .iter()
                .find(|e| matches!(e.descriptor,Descriptor::Model(i) if i==ordinal))
                .ok_or("actual Primary slot absent")?;
            let name = registration::asset(p.cells(), Group::Primary, ordinal)?;
            let source = index
                .find(&name)
                .ok_or_else(|| format!("Fresh Primary{ordinal}: {name} absent"))?;
            let end = source
                .offset
                .checked_add(source.length)
                .ok_or("Primary source range overflow")?;
            let bytes = archive
                .get(
                    usize::try_from(source.offset).map_err(|_| "Primary offset")?
                        ..usize::try_from(end).map_err(|_| "Primary end")?,
                )
                .ok_or("Primary source leaves archive")?;
            models.push(CpuModel::parsed(
                entry.id,
                source,
                &CachedMesh::parse(bytes)?,
            )?);
        }
        let main = MainSlot {
            phase: MainBirthPhase::StaticPending,
            birth: Box::new(MainBirth {
                source: 0x58e3c0,
                caption: 0x58e548,
                child0_source: 0x560920,
                static_preimage: p.cells().read(0x58e3c0, 0x50c)?.to_vec(),
                child0_preimage: p.cells().read(0x560920, 16)?.to_vec(),
            }),
        };
        let voting = super::voting_construction::VotingProduct::allocated(p)?;
        let eventlist = super::eventlist_construction::EventListProduct::allocated(p)?;
        let chat = super::chat_construction::ChatProduct::allocated(p)?;
        let d = PrimaryConstruction::deferred_owner(currency, p, text, images)?;
        let mut this = Self {
            d,
            main,
            voting,
            eventlist,
            chat,
            process: p.construction_identity.clone(),
            text: text.construction_identity.clone(),
            images: images.construction_identity.clone(),
        };
        p.claim_deferred()?;
        match p.deferred_static_births(models) {
            Ok(()) => {
                this.main.phase = MainBirthPhase::Unbound;
                this.voting.phase = super::voting_construction::Phase::Unbound;
                this.eventlist.phase = super::eventlist_construction::Phase::Unbound;
                this.chat.phase = super::chat_construction::Phase::Unbound;
            }
            Err(error) => {
                p.primary_fail_d();
                this.main.phase = MainBirthPhase::FailedBeforeLayout;
                this.voting.fail(error.clone());
                this.eventlist.fail(error.clone());
                this.chat.fail(error.clone());
                this.d.fail(error);
            }
        }
        Ok(this)
    }
    pub fn phase(&self) -> PrimaryPhase {
        self.d.phase()
    }
    pub fn d(&self) -> &DProduct {
        self.d.d()
    }
    pub fn main(&self) -> &MainSlot {
        &self.main
    }
    pub fn voting(&self) -> &super::voting_construction::VotingProduct {
        &self.voting
    }
    pub fn eventlist(&self) -> &super::eventlist_construction::EventListProduct {
        &self.eventlist
    }
    pub fn chat(&self) -> &super::chat_construction::ChatProduct {
        &self.chat
    }
    pub fn currency(&self) -> &Currency {
        self.d.currency()
    }
    pub fn require_ordered_main(&self) -> Result<(), String> {
        Err(format!(
            "{:?}: real G through Clock and later Main are not supplied; A/F remain unbound",
            self.phase()
        ))
    }
    pub fn require_current_d(&self, epoch: u64) -> Result<(), String> {
        self.d.require_current_d(epoch)
    }
    pub fn require_supplier(&self) -> Result<(), String> {
        Err("Fresh prefix stops before G/Main/F; Graph/Source suppliers remain closed".into())
    }
    pub fn retire_for_camera(&mut self, p: &mut Primitives) -> Result<(), String> {
        self.d.retire_for_camera(p)
    }
    /// Existing protected Install is the sole producer boundary. This is
    /// internally ordered Primary work against this real Session, delayed
    /// relative to native pre-menu topology/category initialization.
    pub fn install(
        &mut self,
        source: &ResidentSource,
        generation: u64,
        services: &mut SessionServices<'_>,
        p: &mut Primitives,
        text: &mut UiTextResources,
        images: &Images,
    ) -> Result<InstallDisposition, String> {
        if !self.process.same(&p.construction_identity)
            || !self.text.same(&text.construction_identity)
            || !self.images.same(&images.construction_identity)
            || images.ui_dimensions != [640, 480]
            || source.epoch().serial() != generation
            || services.generation() != generation
        {
            return Err(
                "Fresh prefix requires SAME process/Fonts/images and actual Install epoch".into(),
            );
        }
        source.validate_live()?;
        let starting = self.phase() == PrimaryPhase::Allocated;
        if starting {
            self.d.begin_deferred(source, services, p, text)?;
        }
        //The same real D body/rows/Fonts and actual bind suffix/register are
        //reused. Its late failures remain sticky and retain completed effects.
        //On World2 it only parks; this never recreates D or replays the prelude.
        let result = self
            .d
            .install(source, generation, services, p, text, images)
            .and_then(|outcome| {
                match outcome {
                    InstallDisposition::CompletedD { .. } => {
                        self.d.install_voting(
                            &mut self.voting,
                            source,
                            generation,
                            p,
                            text,
                            images,
                        )?;
                        self.d.install_eventlist(
                            &mut self.eventlist,
                            source,
                            generation,
                            p,
                            text,
                            images,
                        )?;
                        self.d.install_chat(
                            &mut self.chat,
                            &self.eventlist,
                            source,
                            generation,
                            p,
                            text,
                            images,
                        )
                    }
                    other => Ok(other), //warm park, never Voting replay
                }
            });
        //Once this containing operation wrote currency/copied descriptors,
        //even a later pre-layout source/protocol failure is a reached failure.
        //It cannot leave an apparently retryable intermediate prefix behind.
        if starting {
            if let Err(error) = &result {
                p.primary_fail_d();
                if self.voting.phase() != super::voting_construction::Phase::Registered {
                    self.voting.fail(error.clone());
                }
                if self.eventlist.phase() != super::eventlist_construction::Phase::Registered {
                    self.eventlist.fail(error.clone());
                }
                if self.chat.phase() != super::chat_construction::Phase::Registered {
                    self.chat.fail(error.clone());
                }
                self.d.fail(error.clone());
            }
        }
        result
    }
}
#[cfg(test)]
#[path = "deferred_primary_tests.rs"]
mod tests;
