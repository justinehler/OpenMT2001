//! The bounded allocation/defer seam. No whole CRT/Clock claim is made here.
use super::super::{
    primary_d_construction::{Currency, PrimaryPhase},
    ui_text_resources::UiTextResources,
};
use super::*;
const A: u32 = 0x58e3c0;
const D: u32 = 0x574488;
const F: u32 = 0x58cdb0;
const G: u32 = 0x57e710;

impl Primitives {
    /// Fresh loader only. Immutable model parse owns F, but no static birth,
    /// descriptor copy,43b770,43d010, font/image layout or Main body runs.
    pub(crate) fn deferred_allocation(executable: &[u8], model: CpuModel) -> Result<Self, String> {
        if model.source.panel != PanelId::F {
            return Err("Fresh Navigator requires actual F source".into());
        }
        let mut p = Builder::new(Cells::process_allocation(executable)?).finish()?;
        p.resources
            .insert_unbound_model(p.cells.as_ref().unwrap(), model)?;
        p.provenance = Provenance::DeferredUnclaimed;
        Ok(p)
    }
    pub(in crate::intro::camera_live) fn validate_deferred_unclaimed(&self) -> Result<(), String> {
        if self.provenance != Provenance::DeferredUnclaimed
            || self.primary_claimed
            || self.primary_currency_identity.is_some()
            || self.main_phase != MainPhase::Unstarted
            || !self.panels.is_empty()
            || !self.texts.is_empty()
            || !self.caption_keys.is_empty()
            || !self.spans.is_empty()
            || !self.steps.is_empty()
            || self.resources.models.len() != 1
            || !self.resources.models.contains_key(&PanelId::F)
            || !self.resources.descriptors.is_empty()
        {
            return Err("Fresh Primary requires its unclaimed cold allocation; eager/previously executed owners cannot convert".into());
        }
        Ok(())
    }
    pub(in crate::intro::camera_live) fn claim_deferred(&mut self) -> Result<(), String> {
        self.validate_deferred_unclaimed()?;
        self.provenance = Provenance::DeferredClaimed;
        self.primary_claimed = true;
        self.primary_phase = Some(PrimaryPhase::Allocated);
        Ok(())
    }
    /// Loan every existing-allocation registry with the SAME Cells and owners.
    /// The containing controller already owns its D and Main slots. A reached
    /// failure returns all partial primitive effects to that retained owner.
    pub(in crate::intro::camera_live) fn deferred_static_births(
        &mut self,
        models: Vec<CpuModel>,
    ) -> Result<(), String> {
        if self.provenance != Provenance::DeferredClaimed
            || self.primary_phase != Some(PrimaryPhase::Allocated)
        {
            return Err("Fresh static birth outside initial claimed allocation".into());
        }
        let mut b = Builder {
            cells: self.cells.take().ok_or("process cells borrowed")?,
            panels: std::mem::take(&mut self.panels),
            caption_keys: std::mem::take(&mut self.caption_keys),
            spans: std::mem::take(&mut self.spans),
            resources: std::mem::replace(&mut self.resources, CpuResources::new()),
            steps: std::mem::take(&mut self.steps),
        };
        let result = (|| {
            for model in models {
                b.resources.insert_unbound_model(&b.cells, model)?;
            }
            //This is a finite projection of actual supplied births. It does
            //not execute the global CRT list or claim its omitted owners/RNG.
            init::group43c570(&mut b.cells, G)?;
            b.admit_span(D, 0xdf8)?;
            b.panel(D)?;
            b.spans.insert(D, 0xdf8);
            primary_d_primitives::reset_descriptor(&mut b.cells, D + 0x388)?;
            for offset in [0x4ac, 0xa7c, 0xc34] {
                caption(&mut b.cells, D + offset, &mut b.steps)?;
                self.texts.insert(
                    D + offset,
                    super::super::case_zero_factories::text_owner(&b.cells, D + offset)?,
                );
                b.caption_keys.insert(D + offset);
            }
            b.cells.set_word(D, 0x5255f0)?;
            self.texts.insert(
                D + 0x188,
                super::super::case_zero_factories::text_owner(&b.cells, D + 0x188)?,
            );
            b.voting_static()?;
            self.texts.insert(
                0x573b38 + 0x188,
                super::super::case_zero_factories::text_owner(&b.cells, 0x573b38 + 0x188)?,
            );
            b.eventlist_static()?;
            self.texts.insert(
                0x571378 + 0x188,
                super::super::case_zero_factories::text_owner(&b.cells, 0x571378 + 0x188)?,
            );
            b.chat_static43a770(&mut self.texts)?;
            b.main_static()?;
            self.texts.insert(
                A + 0x188,
                super::super::case_zero_factories::text_owner(&b.cells, A + 0x188)?,
            );
            b.navigator()?;
            self.texts.insert(
                F + 0x188,
                super::super::case_zero_factories::text_owner(&b.cells, F + 0x188)?,
            );
            Ok(())
        })();
        self.cells = Some(b.cells);
        self.panels = b.panels;
        self.caption_keys = b.caption_keys;
        self.spans = b.spans;
        self.resources = b.resources;
        self.steps = b.steps;
        if result.is_err() {
            self.primary_phase = Some(PrimaryPhase::Failed);
        }
        result
    }
    pub(in crate::intro::camera_live) fn check_deferred_ready(
        &self,
        text: &UiTextResources,
    ) -> Result<(), String> {
        if self.provenance != Provenance::DeferredClaimed
            || self.primary_phase != Some(PrimaryPhase::Allocated)
            || self.primary_currency_identity.is_some()
            || self.primary_pool.is_some()
            || self.main_phase != MainPhase::Unstarted
            || self.resources.models.len() != 16
            || !self.resources.descriptors.is_empty()
        {
            return Err(
                "Fresh prefix already started or owner/resource phases differ; no replay".into(),
            );
        }
        for address in [D, 0x573b38, 0x571378, 0x57ef28, A, F] {
            let panel = self
                .panels
                .get(&address)
                .ok_or("Fresh required actual static Panel absent")?;
            if panel.phase != Phase::Constructed
                || panel.descriptor.is_some()
                || self.cells().byte(address + 0x184)? != 0
            {
                return Err("Fresh panel was already bound or laid out".into());
            }
        }
        if text.board_stem.is_none() {
            return Err("Fresh currency requires actual selected board language producer".into());
        }
        Ok(())
    }
    /// Concrete43cc60 prefix: currency ->16 copies in native ordinal order ->
    ///18 references -> D bind-prefix. Never call legacy primary_prepare here.
    pub(in crate::intro::camera_live) fn deferred_prefix(
        &mut self,
        currency: &Currency,
        text: &UiTextResources,
    ) -> Result<(), String> {
        self.check_deferred_ready(text)?;
        self.primary_phase = Some(PrimaryPhase::Preparing);
        let result = (|| {
            let c = self.cells.as_mut().ok_or("actual process cells borrowed")?;
            c.write(0x57e6ec, &currency.symbol)?;
            c.set_word(0x59050c, currency.suffix)?;
            self.primary_currency_identity = Some(text.construction_identity.clone());
            self.steps.push(Step::Currency);
            c.set_word(G + 0x24, 16)?;
            let mut pool = Vec::new();
            pool.try_reserve_exact(16)
                .map_err(|_| "Primary descriptor pool allocation failed")?;
            // Successful typed reserve: capacity/enabled, not completed count.
            c.set_word(G + 0x30, 16)?;
            c.set_byte(G + 0x34, 1)?;
            for ordinal in 0..16u8 {
                let id = self.resources.prepare_primary_descriptor(ordinal)?;
                pool.push(id);
                c.set_word(G + 0x2c, u32::from(ordinal) + 1)?;
                self.steps.push(Step::DescriptorCopy(ordinal));
            }
            self.primary_pool = Some(
                pool.try_into()
                    .map_err(|_| "actual16 descriptor pool length differs")?,
            );
            for (offset, p) in [
                (0x38, F),
                (0x3c, 0x56cdc8),
                (0x44, 0x57f838),
                (0x48, D),
                (0x4c, 0x58e8d0),
                (0x50, A),
                (0x58, 0x57e798),
                (0x5c, 0x571378),
                (0x68, 0x56f3f0),
                (0x6c, 0x58fb40),
                (0x54, 0x58ca08),
                (0x70, 0x575d98),
                (0x74, 0x575280),
                (0x78, 0x58f438),
                (0x7c, 0x577048),
                (0x80, 0x571808),
                (0x84, 0x57ef28),
                (0x60, 0x573b38),
            ] {
                c.set_word(G + offset, p)?;
            }
            self.steps.push(Step::PrimaryReferences);
            Ok(())
        })();
        if let Err(e) = result {
            self.primary_phase = Some(PrimaryPhase::Failed);
            return Err(e);
        }
        let mut b = Builder {
            cells: self.cells.take().ok_or("actual process cells borrowed")?,
            panels: std::mem::take(&mut self.panels),
            caption_keys: std::mem::take(&mut self.caption_keys),
            spans: std::mem::take(&mut self.spans),
            resources: std::mem::replace(&mut self.resources, CpuResources::new()),
            steps: std::mem::take(&mut self.steps),
        };
        let result =
            b.bind_descriptor_prefix(D, DescriptorIdentity::EmbeddedD, None, Group::Primary);
        self.cells = Some(b.cells);
        self.panels = b.panels;
        self.caption_keys = b.caption_keys;
        self.spans = b.spans;
        self.resources = b.resources;
        self.steps = b.steps;
        self.primary_phase = Some(if result.is_ok() {
            PrimaryPhase::AwaitingDLayout
        } else {
            PrimaryPhase::Failed
        });
        result
    }
}
