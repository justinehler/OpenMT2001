//! Main-panel lifetime and delivered event1 timeout (4956c0/495860).
use std::time::Duration;

#[derive(Default)]
pub(crate) struct FrontMain {
    shown: bool,
    updates: u16,
}

impl FrontMain {
    pub(crate) fn shown(&mut self, shown: bool) {
        if shown != self.shown {
            self.updates = 0;
            self.shown = shown;
        }
    }

    /// Input events do not write this counter. Only showing the panel resets it.
    pub(crate) fn event1(&mut self) -> bool {
        if !self.shown {
            return false;
        }
        self.updates += 1;
        if self.updates == 6000 {
            self.shown(false);
            return true;
        }
        false
    }
}

/// The frontend group receives one event1 per central 20 ms update, regardless
/// of render rate (4ba580/43c8e0). Disabled intervals never become catch-up work.
#[derive(Default)]
pub(crate) struct Clock {
    remainder: Duration,
}

impl Clock {
    pub(crate) fn delivered(&mut self, elapsed: Duration, enabled: bool) -> u32 {
        if !enabled {
            self.remainder = Duration::ZERO;
            return 0;
        }
        let elapsed = self.remainder.saturating_add(elapsed);
        let count = elapsed.as_millis() / 20;
        self.remainder = Duration::from_nanos((elapsed.as_nanos() % 20_000_000) as u64);
        count.min(u128::from(u32::MAX)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_main_starts_on_the_6000th_delivered_update_and_rearms_on_show() {
        let mut main = FrontMain::default();
        main.shown(true);
        for _ in 0..5999 {
            assert!(!main.event1());
            // Pointer/key events leave this panel shown, including Quit's modal.
            main.shown(true);
        }
        assert!(main.event1());
        assert!(!main.event1());
        main.shown(true);
        assert!(!main.event1());
        assert_eq!(main.updates, 1);
    }

    #[test]
    fn hidden_submenus_and_world_do_not_inherit_the_main_panel_counter() {
        let mut main = FrontMain::default();
        main.shown(true);
        for _ in 0..5999 {
            assert!(!main.event1());
        }
        main.shown(false);
        for _ in 0..7000 {
            assert!(!main.event1());
        }
        main.shown(true);
        assert!(!main.event1());
        assert_eq!(main.updates, 1);
    }

    #[test]
    fn frontend_clock_delivers_50_updates_per_second_at_any_render_rate() {
        for frames in [1, 30, 60, 144] {
            let mut clock = Clock::default();
            let mut delivered = 0;
            let mut previous = Duration::ZERO;
            for frame in 1..=frames {
                let now = Duration::from_secs(120).mul_f64(f64::from(frame) / f64::from(frames));
                delivered += clock.delivered(now - previous, true);
                previous = now;
            }
            assert_eq!(delivered, 6000);
            assert_eq!(clock.delivered(Duration::from_secs(999), false), 0);
            assert_eq!(clock.delivered(Duration::from_millis(19), true), 0);
            assert_eq!(clock.delivered(Duration::from_millis(1), true), 1);
        }
    }
}
