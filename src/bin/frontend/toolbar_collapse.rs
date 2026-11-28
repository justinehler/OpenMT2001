//! Original41fae0 body expansion is independent of modes[0..2] visibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Info,
    Lease,
    Build,
}
impl Action {
    pub const ALL: [Self; 3] = [Self::Info, Self::Lease, Self::Build];
    pub fn index(self) -> usize {
        match self {
            Self::Info => 0,
            Self::Lease => 1,
            Self::Build => 2,
        }
    }
    pub fn tutorial(self) -> u32 {
        9 + self.index() as u32
    }
    pub fn tooltip(self) -> u32 {
        [29, 50, 52][self.index()]
    }
    pub fn reveal(self, body_open: &mut bool) {
        if self != Self::Build {
            *body_open = true;
        } //404630 ->41fc60; Build ->404eb0
    }
}
#[derive(Clone, Copy)]
pub struct Policy {
    pub category: usize,
    pub shown: bool,
    pub nonzero_kind: bool,
    pub lease_available: bool,
    /// Retains the existing host planner session/auction admission. The original
    /// full rights-dependent Build disabled state is not newly reconstructed.
    pub build_allowed: bool,
    pub tutorial_allowed: [bool; 3],
}
impl Policy {
    pub fn visible(self, a: Action) -> bool {
        self.shown && (a == Action::Info || self.nonzero_kind)
    }
    pub fn enabled(self, a: Action) -> bool {
        self.visible(a)
            && self.tutorial_allowed[a.index()]
            && match a {
                Action::Info => true,
                Action::Lease => self.lease_available,
                Action::Build => self.build_allowed,
            }
    }
    pub fn occupied(self, bounds: [[f32; 4]; 3], p: [f32; 2]) -> bool {
        Action::ALL
            .into_iter()
            .any(|a| self.visible(a) && hit(bounds[a.index()], p))
    }
    pub fn target(self, bounds: [[f32; 4]; 3], p: [f32; 2]) -> Option<Action> {
        Action::ALL
            .into_iter()
            .find(|a| self.enabled(*a) && hit(bounds[a.index()], p))
    }
}
pub fn hit(b: [f32; 4], p: [f32; 2]) -> bool {
    p[0] > b[0] && p[0] < b[2] && p[1] >= b[1] && p[1] <= b[3]
}
#[derive(Default)]
pub struct Input {
    pressed: Option<(usize, Action)>,
    pub hover: Option<Action>,
}
impl Input {
    pub fn cancel(&mut self) {
        self.pressed = None;
        self.hover = None;
    }
    pub fn captured(&self) -> bool {
        self.pressed.is_some()
    }
    pub fn pressed(&self, category: usize, a: Action) -> bool {
        self.pressed == Some((category, a))
    }
    pub fn update(
        &mut self,
        policy: Option<Policy>,
        bounds: [[f32; 4]; 3],
        point: Option<[f32; 2]>,
    ) {
        self.hover = policy.and_then(|q| point.and_then(|p| q.target(bounds, p)));
        if self.pressed.is_some_and(|(category, a)| {
            policy.is_none_or(|q| q.category != category || !q.enabled(a))
        }) {
            self.pressed = None;
        }
    }
    /// Disabled controls occupy the side strip but never capture/activate.
    /// Both category identity and button identity must survive until release.
    pub fn mouse(
        &mut self,
        policy: Policy,
        bounds: [[f32; 4]; 3],
        point: Option<[f32; 2]>,
        down: bool,
    ) -> (bool, Option<Action>) {
        self.update(Some(policy), bounds, point);
        let occupied = point.is_some_and(|p| policy.occupied(bounds, p));
        let consumed = occupied || self.captured();
        let answer = if down {
            self.pressed = self.hover.map(|a| (policy.category, a));
            None
        } else {
            self.pressed
                .take()
                .filter(|(category, a)| *category == policy.category && self.hover == Some(*a))
                .map(|(_, a)| a)
        };
        (consumed, answer)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const B: [[f32; 4]; 3] = [
        [5.6, 53.6, 28.8, 76.8],
        [5.6, 77.6, 28.8, 100.8],
        [5.6, 101.6, 28.8, 124.8],
    ];
    fn policy() -> Policy {
        Policy {
            category: 33,
            shown: true,
            nonzero_kind: true,
            lease_available: true,
            build_allowed: true,
            tutorial_allowed: [true; 3],
        }
    }
    fn click(input: &mut Input, q: Policy, a: Action) -> Option<Action> {
        let b = B[a.index()];
        let p = Some([(b[0] + b[2]) / 2., (b[1] + b[3]) / 2.]);
        assert_eq!(input.mouse(q, B, p, true), (true, None));
        input.mouse(q, B, p, false).1
    }
    #[test]
    fn collapse_keeps_side_info_and_lease_and_reveals_before_selection() {
        let mut input = Input::default();
        let q = policy();
        let mut body = true;
        body = false; // original toggle508 changes body, not side visibility
        for a in [Action::Info, Action::Lease] {
            assert!(q.visible(a));
            let action = click(&mut input, q, a).unwrap();
            action.reveal(&mut body);
            assert!(body); // must precede the caller's report/mode selection
            body = false;
        }
    }
    #[test]
    fn unavailable_lease_stays_visible_but_kind_zero_is_hidden() {
        let mut q = policy();
        q.lease_available = false;
        let mut input = Input::default();
        assert!(q.visible(Action::Lease));
        assert!(!q.enabled(Action::Lease));
        assert_eq!(input.mouse(q, B, Some([15., 90.]), true), (true, None));
        assert_eq!(input.mouse(q, B, Some([15., 90.]), false), (true, None));
        q.nonzero_kind = false;
        assert!(!q.visible(Action::Lease) && !q.visible(Action::Build));
        assert_eq!(input.mouse(q, B, Some([15., 90.]), true), (false, None));
        assert!(q.enabled(Action::Info));
    }
    #[test]
    fn build_does_not_force_body_open_and_true_global_hide_removes_all() {
        let mut input = Input::default();
        let mut q = policy();
        let mut body = false;
        click(&mut input, q, Action::Build)
            .unwrap()
            .reveal(&mut body);
        assert!(!body);
        q.shown = false;
        for a in Action::ALL {
            assert!(!q.visible(a));
            assert_eq!(click_hidden(&mut input, q, a), None);
        }
    }
    fn click_hidden(input: &mut Input, q: Policy, a: Action) -> Option<Action> {
        let b = B[a.index()];
        let p = Some([15., (b[1] + b[3]) / 2.]);
        assert_eq!(input.mouse(q, B, p, true), (false, None));
        input.mouse(q, B, p, false).1
    }
    #[test]
    fn capture_refuses_cross_button_category_and_late_disabled_release() {
        let mut input = Input::default();
        let mut q = policy();
        input.mouse(q, B, Some([15., 60.]), true);
        assert_eq!(input.mouse(q, B, Some([15., 90.]), false), (true, None));
        input.mouse(q, B, Some([15., 90.]), true);
        q.category = 34;
        assert_eq!(input.mouse(q, B, Some([15., 90.]), false).1, None);
        input.mouse(q, B, Some([15., 90.]), true);
        q.lease_available = false;
        assert_eq!(input.mouse(q, B, Some([15., 90.]), false), (true, None));
        input.cancel();
        assert!(!input.captured());
    }
    #[test]
    fn strict_x_inclusive_y_and_tutorial_disable_preserve_occupancy() {
        let mut q = policy();
        q.tutorial_allowed = [false; 3];
        assert!(q.occupied(B, [15., 53.6]));
        assert_eq!(q.target(B, [15., 53.6]), None);
        assert!(!q.occupied(B, [5.6, 60.]));
        assert!(!q.occupied(B, [28.8, 60.]));
        assert!(q.occupied(B, [15., 76.8]));
    }
    #[test]
    #[ignore = "requires original ARCHIVE_DIR/BIN; static mesh decode only"]
    fn original_frame_zero_and_nine_body_and_side_geometry() {
        use openmt2001::{archive::ArchiveIndex, mesh::CachedMesh, ui_mesh};
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let e = index
            .find("gamedata2/brand_new_panels/main_panel_v2.tm")
            .unwrap();
        let mesh =
            CachedMesh::parse(&archive[e.offset as usize..(e.offset + e.length) as usize]).unwrap();
        let closed = ui_mesh::regions(&mesh, 0).unwrap();
        let open = ui_mesh::regions(&mesh, 9).unwrap();
        let bounds =
            |rs: &[ui_mesh::Region], name: &str| rs.iter().find(|r| r.name == name).unwrap().bounds;
        for name in [
            "P_MAIN_ICON2",
            "P_MAIN_ICON3",
            "P_MAIN_ICON4",
            "P_MAIN_OPENCLOSE_ICON",
        ] {
            assert_eq!(bounds(&closed, name), bounds(&open, name));
        }
        let c = bounds(&closed, "P_MAIN_MAIN_WINDOW");
        let o = bounds(&open, "P_MAIN_MAIN_WINDOW");
        assert_eq!(c[0].to_bits(), 30.399999618530273_f32.to_bits());
        assert_eq!(c[2].to_bits(), 30.401355743408203_f32.to_bits());
        assert_eq!(o[2].to_bits(), 188.80380249023438_f32.to_bits());
        let mut q = policy();
        let side =
            ["P_MAIN_ICON2", "P_MAIN_ICON3", "P_MAIN_ICON4"].map(|name| bounds(&closed, name));
        assert_eq!(q.target(side, [15., 90.]), Some(Action::Lease));
        q.shown = false;
        assert_eq!(q.target(side, [15., 90.]), None);
    }
}
