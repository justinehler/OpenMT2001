//! Top-right Time panel state: the shared animated cash amount (native
//! 560a1c/560a20), the per-instance widget caches and pass counts. The
//! binary's `time_panel` module draws it.

/// App-owned native560a1c/560a20 globals, retained across city loads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Runtime {
    pub amount: f32,
    pub remaining: i32,
    /// Normal58fb40 and auction565bb0 retain independent show/update caches.
    /// Their model-bind initializer resets those caches on each city load.
    pub widgets: [Widget; 2],
    pub chance_visuals: [crate::card_panel::EntryVisual; 2],
}
/// Original565bb0 widget state, initialized independently of app globals.
#[derive(Clone, Copy, Debug)]
pub struct Widget {
    pub last_amount: f32,
    pub cash_icon_phase: i32,
    pub visible: bool,
    /// The cash text this widget last published (None until the first change).
    pub text_amount: Option<f32>,
}
impl Default for Widget {
    fn default() -> Self {
        Self {
            last_amount: -1.,
            cash_icon_phase: 0,
            visible: false,
            text_amount: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DisplayState {
    pub cash: Option<f32>,
    pub cash_icon_phase: i32,
    pub visible: bool,
}
impl Widget {
    pub fn display(&self) -> DisplayState {
        DisplayState {
            cash: self.text_amount,
            visible: self.visible,
            cash_icon_phase: self.cash_icon_phase,
        }
    }
}
impl Runtime {
    ///43b770 bind tail invokes4281e0 separately for both Time instances.
    /// It clears text, comparison9ac, phase9a0, visibility345 and reset9a4;
    /// shared560a1c/560a20 and Chance blink9a8 are not written.
    pub fn bind_panels(&mut self) {
        for (widget, chance) in self.widgets.iter_mut().zip(&mut self.chance_visuals) {
            widget.last_amount = 0.;
            widget.cash_icon_phase = 0;
            widget.visible = false;
            widget.text_amount = None;
            chance.textures = [285; 4];
            chance.reset = false;
        }
    }
    ///43be20→vtable+34=4285f0 on a newly shown instance. The blink phase
    /// survives; only the cached cash comparison resets before428060.
    pub fn activate(&mut self, actual: f32, instance: usize) -> Result<(), String> {
        let mut widget = *self
            .widgets
            .get(instance)
            .ok_or("Time instance outside two widgets")?;
        widget.last_amount = -1.;
        self.initialize(actual, &mut widget)?;
        self.widgets[instance] = widget;
        Ok(())
    }
    pub fn show(&mut self, actual: f32, instance: usize) -> Result<(), String> {
        let widget = self
            .widgets
            .get_mut(instance)
            .ok_or("Time instance outside two widgets")?;
        if widget.visible {
            return Ok(());
        }
        widget.visible = true;
        self.activate(actual, instance)
    }
    pub fn hide(&mut self, instance: usize) -> Result<(), String> {
        self.widgets
            .get_mut(instance)
            .ok_or("Time instance outside two widgets")?
            .visible = false;
        Ok(())
    }
    pub fn tick(&mut self, actual: f32, debt_status: bool, instance: usize) -> Result<(), String> {
        let mut widget = *self
            .widgets
            .get(instance)
            .ok_or("Time instance outside two widgets")?;
        self.step(actual, debt_status, &mut widget)?;
        self.widgets[instance] = widget;
        Ok(())
    }
    /// Native428099..42813f globals560a1c/560a20 and widget9ac.
    pub fn initialize(&mut self, actual: f32, widget: &mut Widget) -> Result<Option<u32>, String> {
        if self.amount != actual {
            if self.remaining == 0 {
                self.remaining = 50;
            } else {
                let mut amount = crate::x87::Sum::from_f32(actual)?;
                amount.add_f32(-self.amount)?;
                amount.div(crate::x87::Sum::from_i32(self.remaining))?;
                self.remaining = self.remaining.wrapping_sub(1);
                amount.add_f32(self.amount)?;
                self.amount = amount.to_f32();
            }
        }
        if widget.last_amount == self.amount {
            Ok(None)
        } else {
            widget.last_amount = self.amount;
            widget.text_amount = Some(self.amount);
            Ok(Some(if self.amount < 0. { 6 } else { 0 }))
        }
    }
    /// Native428faf/428fbd uses the retained midnight debt-status byte.
    /// Caller supplies the already-proven native widget dispatch gate.
    pub fn step(
        &mut self,
        actual: f32,
        debt_status: bool,
        widget: &mut Widget,
    ) -> Result<Option<u32>, String> {
        widget.cash_icon_phase = if debt_status {
            widget.cash_icon_phase.wrapping_sub(1)
        } else {
            0
        };
        self.initialize(actual, widget)
    }
}

/// Native4b5370 manager activation, then4ba580 dispatch. The caller applies
/// suppression and signed pause gates before these per-instance pass counts.
pub fn instance_passes(gui_mode: u32, iterations: u32) -> [u32; 2] {
    match gui_mode {
        1 => [0, 1],
        4 => [0, 0],
        _ => [iterations, 0],
    }
}
