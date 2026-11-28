//! Private offline adapter. No campaign request or network command is produced.
use crate::{skirmish_roster as roster, skirmish_setup as setup};
use openmt2001::{
    level_catalog::{Catalog, Labels, Level},
    pe::PeImage,
    render::{PreparedScene, Renderer},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub struct Request {
    pub catalog: Catalog,
    pub launch: crate::fresh_city::SkirmishLaunch,
    pub save_root: PathBuf,
}
pub struct Route {
    press: Option<PointerTarget>,
    keyboard: Option<PointerTarget>,
    setup_panel: setup::SetupPanel,
    roster_panel: roster::RosterPanel,
    setup: Option<setup::State>,
    roster: Option<roster::State>,
    executable: Vec<u8>,
    scripts: PathBuf,
    strings: Vec<Vec<u8>>,
    map: Vec<u8>,
    profile: Vec<u8>,
    token: usize,
    save_root: PathBuf,
    metadata: [Vec<u8>; 2],
    pub request: Option<Request>,
    pub busy: bool,
    pub error: Option<String>,
}
// Original language row0 maps code-page byte32 onward to glyph indices.
pub(super) fn encode(map: &[u8], bytes: &[u8]) -> Result<Vec<u8>, String> {
    bytes
        .iter()
        .map(|&b| {
            map.get(usize::from(
                b.checked_sub(32).ok_or("control byte in player text")?,
            ))
            .copied()
            .ok_or("unmapped player byte".into())
        })
        .collect()
}
pub(super) fn decode(map: &[u8], glyphs: &[u8]) -> Result<Vec<u8>, String> {
    glyphs
        .iter()
        .map(|b| {
            map.iter()
                .position(|v| v == b)
                .and_then(|i| u8::try_from(i + 32).ok())
                .ok_or("unmapped token glyph".into())
        })
        .collect()
}
pub(super) fn objective(exe: &[u8], scripts: &Path, level: &Level) -> Result<i32, String> {
    let directory = std::str::from_utf8(level.directory()).map_err(|e| e.to_string())?;
    if !directory.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("invalid catalog directory".into());
    }
    let path = crate::campaign_store::original_path(scripts, &format!("{directory}/hub.lua"))?
        .ok_or("selected original hub missing")?;
    Ok(
        openmt2001::level_script::objective_selection::evaluate_setup(
            &PeImage::parse(exe)?,
            level,
            &fs::read(path).map_err(|e| e.to_string())?,
        )?
        .kind,
    )
}
impl Route {
    pub fn new(
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        scripts: &Path,
        strings: &[&[u8]],
        exe: &[u8],
    ) -> Result<Self, String> {
        Ok(Self {
            press: None,
            keyboard: None,
            setup_panel: setup::SetupPanel::new(
                index, archive, installer, strings, exe, 0xbb000a28,
            )?,
            roster_panel: roster::RosterPanel::new(
                index, archive, installer, strings, exe, 0xbb000a28,
            )?,
            executable: exe.to_vec(),
            scripts: scripts.to_owned(),
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            map: strings.first().ok_or("language mapping missing")?.to_vec(),
            setup: None,
            roster: None,
            profile: vec![],
            token: 0,
            save_root: PathBuf::new(),
            metadata: [vec![], vec![]],
            request: None,
            busy: false,
            error: None,
        })
    }
    pub fn open(
        &mut self,
        catalog: Catalog,
        name: &[u8],
        token: usize,
        save_root: PathBuf,
    ) -> Result<(), String> {
        let maps = crate::skirmish_maps::Catalog::discover(&self.scripts)?;
        let exe = &self.executable;
        let scripts = &self.scripts;
        let mut setup = setup::State::open(catalog, name, None, self.setup_panel.options(), |l| {
            objective(exe, scripts, l)
        })?;
        setup.set_maps(
            maps.names()
                .iter()
                .map(|name| name.as_bytes().to_vec())
                .collect(),
            maps.default_index(),
        )?;
        self.setup = Some(setup);
        self.profile = name.to_vec();
        self.token = token;
        self.save_root = save_root;
        self.roster = None;
        self.keyboard = None;
        self.press = None;
        self.error = None;
        Ok(())
    }
    pub fn visible(&self) -> bool {
        self.setup.as_ref().is_some_and(|s| s.visible)
            || self.roster.as_ref().is_some_and(|s| s.visible)
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        if let Some(r) = self.roster.as_ref().filter(|r| r.visible) {
            return self.roster_panel.hit(r, p).ok().flatten();
        }
        // Setup hit/action is resolved directly from pointer on release.
        self.setup
            .as_ref()
            .filter(|s| s.visible)
            .and_then(|s| self.setup_panel.hit(s, p).ok().flatten())
            .map(setup_action_id)
    }
    fn target(&self, p: [f32; 2]) -> Option<PointerTarget> {
        if let Some(r) = self.roster.as_ref().filter(|r| r.visible) {
            if self.roster_panel.hit_name(r, p).ok()? {
                return Some(PointerTarget::Name);
            }
            return self.roster_panel.hit(r, p).ok()?.map(PointerTarget::Roster);
        }
        self.setup
            .as_ref()
            .filter(|s| s.visible)
            .and_then(|s| self.setup_panel.hit(s, p).ok().flatten())
            .map(PointerTarget::Setup)
    }
    pub fn cancel_input(&mut self) {
        self.press = None;
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        if self.busy {
            return;
        }
        if let Some(PointerTarget::Setup(
            action @ (setup::Action::Scroll { part: 2, .. } | setup::Action::MapScroll { part: 2 }),
        )) = self.press
        {
            if let Some(state) = self.setup.as_mut().filter(|s| s.visible) {
                let exe = &self.executable;
                let scripts = &self.scripts;
                if let Err(e) = self
                    .setup_panel
                    .activate(state, action, p, &mut |l| objective(exe, scripts, l))
                {
                    self.error = Some(e);
                }
            }
        }
    }
    pub fn mouse(&mut self, p: Option<[f32; 2]>, down: bool) {
        if self.busy {
            self.press = None;
            return;
        }
        let target = p.and_then(|p| self.target(p));
        if down {
            self.keyboard = target;
            self.press = target;
            if let Some(p) = p {
                self.pointer(p);
            }
        } else {
            let origin = self.press.take();
            if matching_release(origin, target) {
                self.click(p);
            }
        }
    }
    pub fn click(&mut self, p: Option<[f32; 2]>) {
        if let Err(e) = self.click_inner(p) {
            self.error = Some(e);
        }
    }
    fn click_inner(&mut self, p: Option<[f32; 2]>) -> Result<(), String> {
        let target = p.and_then(|point| self.target(point));
        self.activate_target(target, p)
    }
    fn activate_target(
        &mut self,
        target: Option<PointerTarget>,
        p: Option<[f32; 2]>,
    ) -> Result<(), String> {
        if self.busy || self.error.is_some() {
            return Ok(());
        }
        if let Some(r) = self.roster.as_mut().filter(|r| r.visible) {
            if target == Some(PointerTarget::Name) {
                r.focus_name();
                return Ok(());
            }
            if let Some(PointerTarget::Roster(id)) = target {
                //49edc0 changes the local roster token, not profile9206d4.
                // Keep the profile preference supplied on entry unchanged.
                let outcome = r.activate(id)?;
                match outcome {
                    roster::Outcome::Back(_) => {
                        self.setup.as_mut().ok_or("setup missing")?.visible = true;
                        self.keyboard = None;
                    }
                    roster::Outcome::Start {
                        setup,
                        participants,
                    } => {
                        self.busy = true;
                        r.busy = true;
                        r.visible = true;
                        self.press = None;
                        r.visible = true;
                        let directory =
                            String::from_utf8(setup.directory).map_err(|e| e.to_string())?;
                        self.request = Some(Request {
                            catalog: self.setup.as_ref().ok_or("setup missing")?.catalog.clone(),
                            launch: crate::fresh_city::SkirmishLaunch {
                                level_id: setup.level_id,
                                directory,
                                map: String::from_utf8(setup.map).map_err(|e| e.to_string())?,
                                objective_kind: setup.objective_kind,
                                target: setup.objective_target,
                                difficulty: setup.difficulty as i32,
                                participants,
                            },
                            save_root: self.save_root.clone(),
                        });
                    }
                    _ => (),
                }
            }
            return Ok(());
        }
        if let Some(s) = self.setup.as_mut().filter(|s| s.visible) {
            if let Some(PointerTarget::Setup(action)) = target {
                let exe = &self.executable;
                let scripts = &self.scripts;
                let outcome = match p {
                    Some(point) => self
                        .setup_panel
                        .activate(s, action, point, &mut |l| objective(exe, scripts, l))?,
                    None => self
                        .setup_panel
                        .keyboard_activate(s, action, &mut |l| objective(exe, scripts, l))?,
                };
                if let setup::Outcome::Roster(h) = outcome {
                    let level = s.catalog.selected_level().ok_or("selected level missing")?;
                    self.metadata[0] = match level.labels() {
                        Labels::Localized { title, .. } => self
                            .strings
                            .get(title as usize)
                            .ok_or("title missing")?
                            .clone(),
                        Labels::Literal { title, .. } => encode(&self.map, &title)?,
                    };
                    self.metadata[1] =
                        self.strings[[1654, 1655, 1658][h.difficulty as usize]].clone();
                    let roster = self.roster_panel.initial_roster(
                        &self.profile,
                        self.token,
                        [0; 6],
                        |g| decode(&self.map, g),
                    )?;
                    self.roster = Some(roster::State::open(h, roster)?);
                    s.visible = false;
                    self.keyboard = None;
                }
            }
        }
        Ok(())
    }
    pub fn is_editing(&self) -> bool {
        !self.busy
            && self.error.is_none()
            && (self
                .roster
                .as_ref()
                .is_some_and(|r| r.visible && r.name_focused)
                || (self.roster.as_ref().is_none_or(|r| !r.visible)
                    && self
                        .setup
                        .as_ref()
                        .is_some_and(|s| s.visible && s.name_focused)))
    }
    pub fn navigate(&mut self, delta: i32) {
        if self.busy || self.error.is_some() {
            return;
        }
        let actions = if let Some(r) = self.roster.as_ref().filter(|r| r.visible) {
            std::iter::once(PointerTarget::Name)
                .chain(
                    (0..26)
                        .filter(|&id| r.enabled(id))
                        .map(PointerTarget::Roster),
                )
                .collect::<Vec<_>>()
        } else if let Some(s) = self.setup.as_ref().filter(|s| s.visible) {
            self.setup_panel
                .keyboard_actions(s)
                .into_iter()
                .map(PointerTarget::Setup)
                .collect()
        } else {
            return;
        };
        self.keyboard = next_target(&actions, self.keyboard, delta);
        self.press = None;
        if let Some(r) = self.roster.as_mut().filter(|r| r.visible) {
            r.name_focused = self.keyboard == Some(PointerTarget::Name);
        } else if let Some(s) = self.setup.as_mut().filter(|s| s.visible) {
            s.name_focused = self.keyboard == Some(PointerTarget::Setup(setup::Action::Name));
            if matches!(
                self.keyboard,
                Some(PointerTarget::Setup(
                    setup::Action::Row(_) | setup::Action::MapRow(_)
                ))
            ) {
                self.activate_keyboard();
            }
        }
    }
    pub fn activate_keyboard(&mut self) {
        if let Err(e) = self.activate_target(self.keyboard, None) {
            self.error = Some(e);
        }
    }
    pub fn text(&mut self, text: &str) {
        if self.busy {
            return;
        }
        if !supported_text(text) {
            self.error = Some("Only printable ASCII player text is supported".into());
            return;
        }
        let result = if let Some(r) = self.roster.as_mut().filter(|r| r.visible) {
            r.text_bytes(text.as_bytes())
        } else {
            if let Some(s) = self.setup.as_mut() {
                s.text(text)
            }
            Ok(())
        };
        if let Err(e) = result {
            self.error = Some(e)
        }
    }
    pub fn erase(&mut self) {
        if self.busy {
            return;
        }
        if let Some(r) = self.roster.as_mut().filter(|r| r.visible) {
            if let Err(e) = r.backspace() {
                self.error = Some(e)
            }
        } else if let Some(s) = self.setup.as_mut() {
            s.backspace()
        }
    }
    pub fn finish(&mut self, error: Option<String>) {
        self.busy = false;
        self.error = error;
        self.press = None;
        if let Some(r) = self.roster.as_mut() {
            r.busy = false;
            r.visible = self.error.is_some();
        }
    }
    pub fn back(&mut self) {
        if self.busy {
            return;
        }
        self.keyboard = None;
        if let Some(r) = self.roster.as_mut().filter(|r| r.visible) {
            r.visible = false;
            if let Some(s) = self.setup.as_mut() {
                s.visible = true
            }
        } else if let Some(s) = self.setup.as_mut() {
            s.visible = false
        }
    }
    pub fn scroll(&mut self, p: Option<[f32; 2]>, delta: i32) {
        if self.busy {
            return;
        }
        if let (Some(s), Some(p)) = (self.setup.as_mut().filter(|s| s.visible), p) {
            if let Err(e) = self.setup_panel.scroll(s, p, delta) {
                self.error = Some(e)
            }
        }
    }
    pub fn draw(
        &self,
        renderer: &Renderer,
        p: Option<[f32; 2]>,
        pressed: bool,
    ) -> Result<PreparedScene, String> {
        if let Some(r) = self.roster.as_ref().filter(|r| r.visible) {
            return self.roster_panel.draw(
                r,
                renderer,
                match self.keyboard {
                    Some(PointerTarget::Roster(id)) => Some(id),
                    Some(_) => None,
                    None => p
                        .map(|p| self.roster_panel.hit(r, p))
                        .transpose()?
                        .flatten(),
                },
                pressed,
                [&self.metadata[0], &self.metadata[1]],
                |b| encode(&self.map, b),
            );
        }
        self.setup_panel.draw(
            self.setup.as_ref().ok_or("setup missing")?,
            renderer,
            p,
            pressed,
            match self.keyboard {
                Some(PointerTarget::Setup(a)) => Some(a),
                _ => None,
            },
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_mapping_is_used_both_directions() {
        let map = [2, 0, 1];
        assert_eq!(encode(&map, b" !\"").unwrap(), [2, 0, 1]);
        assert_eq!(decode(&map, &[1, 2]).unwrap(), b"\" ");
        assert!(encode(&map, b"x").is_err());
    }
}
#[cfg(test)]
mod route_contract_tests {
    use super::*;
    #[test]
    fn roster_handoff_preserves_identity_and_back_then_compacts_explicit_ai() {
        let handoff = setup::Handoff {
            level_id: 300001,
            directory: b"net1".to_vec(),
            map: b"empty.lua".to_vec(),
            objective_kind: 6,
            objective_target: 3500.,
            difficulty: 1,
            session_name: b"Justin2".to_vec(),
        };
        let names = std::array::from_fn(|i| format!("token{i}").into_bytes());
        let domain =
            openmt2001::skirmish::roster::Roster::new(b"Justin2", 4, names, [0; 6]).unwrap();
        let mut state = roster::State::open(handoff.clone(), domain).unwrap();
        assert!(!state.enabled(25));
        state.activate(15).unwrap();
        assert!(state.enabled(25));
        state.activate(18).unwrap();
        assert!(!state.enabled(25));
        state.activate(18).unwrap();
        let roster::Outcome::Back(back) = state.activate(24).unwrap() else {
            panic!("Back")
        };
        assert_eq!(back, handoff);
        state.visible = true;
        let roster::Outcome::Start {
            setup,
            participants,
        } = state.activate(25).unwrap()
        else {
            panic!("Start")
        };
        assert_eq!(setup, handoff);
        assert_eq!(participants.len(), 2);
        assert_eq!(participants[0].token, Some(4));
        assert_eq!(participants[1].kind, openmt2001::skirmish::roster::Kind::Ai);
        assert_ne!(participants[0].token, participants[1].token);
    }
}

fn setup_action_id(action: setup::Action) -> usize {
    match action {
        setup::Action::Ok => 100,
        setup::Action::Back => 101,
        setup::Action::Minus => 102,
        setup::Action::Plus => 103,
        setup::Action::Name => 104,
        setup::Action::Difficulty(d) => 110 + d as usize,
        setup::Action::Row(r) => 200 + r,
        setup::Action::MapRow(r) => 1000 + r,
        setup::Action::MapScroll { part } => 140 + part,
        setup::Action::Scroll { description, part } => 120 + usize::from(description) * 10 + part,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointerTarget {
    Setup(setup::Action),
    Roster(usize),
    Name,
}
fn matching_release(origin: Option<PointerTarget>, target: Option<PointerTarget>) -> bool {
    origin.is_some() && origin == target
}
#[cfg(test)]
mod pointer_tests {
    use super::*;
    #[test]
    fn press_drag_release_preserves_actual_action() {
        let plus = Some(PointerTarget::Setup(setup::Action::Plus));
        let minus = Some(PointerTarget::Setup(setup::Action::Minus));
        assert!(!matching_release(plus, minus));
        assert!(!matching_release(None, plus));
        assert!(!matching_release(plus, None));
        assert!(matching_release(plus, plus));
        assert!(!matching_release(
            Some(PointerTarget::Setup(setup::Action::Row(0))),
            Some(PointerTarget::Setup(setup::Action::Row(1)))
        ));
        assert!(!matching_release(
            Some(PointerTarget::Roster(0)),
            Some(PointerTarget::Roster(1))
        ));
    }
}

fn supported_text(text: &str) -> bool {
    text.is_ascii() && text.bytes().all(|b| (32..127).contains(&b))
}
#[cfg(test)]
mod text_tests {
    use super::*;
    #[test]
    fn utf8_is_rejected_before_roster_mutation() {
        assert!(supported_text("Justin2"));
        assert!(!supported_text("José"));
        assert!(!supported_text("東京"));
        assert!(!supported_text("a\nb"));
    }
}

fn next_target(
    actions: &[PointerTarget],
    current: Option<PointerTarget>,
    delta: i32,
) -> Option<PointerTarget> {
    if actions.is_empty() {
        return None;
    }
    let at = current
        .and_then(|v| actions.iter().position(|a| *a == v))
        .map_or(if delta > 0 { -1 } else { 0 }, |i| i as i64);
    Some(actions[(at + i64::from(delta)).rem_euclid(actions.len() as i64) as usize])
}

#[cfg(test)]
mod keyboard_and_failure_tests {
    use super::*;
    // Inert rendering resources deliberately have no pointer hit regions: a
    // keyboard test cannot accidentally pass by using a last cursor position.
    fn fixture() -> Route {
        let exe = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let scripts = openmt2001::fixtures::path("SCRIPTS");
        let catalog = crate::campaign_store::load_catalog(&image, &scripts).unwrap();
        let options = openmt2001::skirmish::Options::from_pe(&image).unwrap();
        let state = setup::State::open(
            catalog,
            b"Justin",
            Some(b"empty.lua".to_vec()),
            &options,
            |_| Ok(6),
        )
        .unwrap();
        Route {
            press: None,
            keyboard: None,
            setup_panel: setup::SetupPanel::inert(options),
            roster_panel: roster::RosterPanel::inert(vec![vec![255]; 1754]),
            setup: Some(state),
            roster: None,
            executable: exe,
            scripts,
            strings: vec![vec![]; 2300],
            map: (0..95).collect(),
            profile: b"Justin".to_vec(),
            token: 4,
            save_root: PathBuf::new(),
            metadata: [vec![], vec![]],
            request: None,
            busy: false,
            error: None,
        }
    }
    fn name_focus(route: &mut Route) {
        // Backwards from no selection reaches Back, OK, modifiers, difficulty,
        // then Name without ever depending on any pointer location.
        for _ in 0..10 {
            route.navigate(-1);
            if route.is_editing() {
                return;
            }
        }
        panic!("keyboard name focus missing");
    }
    #[test]
    #[ignore = "original PE/catalog fixture; root integration required"]
    fn keyboard_names_space_and_failed_handoff_preserve_retry_choice() {
        let mut route = fixture();
        name_focus(&mut route);
        route.activate_keyboard();
        assert!(route.is_editing());
        route.text(" Smith");
        route.keyboard = Some(PointerTarget::Setup(setup::Action::Difficulty(2)));
        route.activate_keyboard();
        route.keyboard = Some(PointerTarget::Setup(setup::Action::Plus));
        route.activate_keyboard();
        let setup::Outcome::Roster(before) = route.setup.as_mut().unwrap().confirm() else {
            panic!("handoff");
        };
        assert_eq!(before.session_name, b"Justin Smith");
        assert_eq!(before.difficulty, 2);
        route.keyboard = Some(PointerTarget::Setup(setup::Action::Ok));
        route.activate_keyboard(); // unmappable token glyph255 in real initial_roster
        assert_eq!(route.error.as_deref(), Some("unmapped token glyph"));
        assert!(route.visible());
        assert!(route.roster.is_none());
        route.error = None; // modal dismissal
        let setup::Outcome::Roster(after) = route.setup.as_mut().unwrap().confirm() else {
            panic!("retry");
        };
        assert_eq!(before, after); // level, difficulty, target, map and name
        // Repair only the injected faulty resource and retry via Enter.
        route.roster_panel = roster::RosterPanel::inert(vec![vec![1]; 1754]);
        route.activate_keyboard();
        assert!(route.error.is_none());
        assert!(!route.setup.as_ref().unwrap().visible);
        assert!(route.roster.as_ref().unwrap().visible);
        route.navigate(1); // first actual roster target is Name
        route.activate_keyboard();
        assert!(route.is_editing());
        route.text(" Smith");
        assert_eq!(
            route.roster.as_ref().unwrap().roster.slots()[0].name,
            b"Justin Smith"
        );
        route.navigate(1); // human left selector, not the name nor pointer
        route.activate_keyboard();
        assert_ne!(
            route.roster.as_ref().unwrap().roster.slots()[0].token,
            Some(4)
        );
        assert!(!route.is_editing());
        route.busy = true;
        route.roster.as_mut().unwrap().busy = true;
        route.finish(Some("injected spawn failure".into()));
        assert!(!route.busy);
        assert!(route.roster.as_ref().unwrap().visible);
        assert!(!route.roster.as_ref().unwrap().busy);
        assert!(route.roster.as_ref().unwrap().enabled(24));
        let selection = route.keyboard;
        route.navigate(1); // modal guard
        assert_eq!(route.keyboard, selection);
    }
}

#[cfg(test)]
mod map_action_routing_tests {
    use super::*;
    #[test]
    fn map_actions_keep_row_identity_and_scroll_domain_distinct() {
        let mut ids = std::collections::BTreeSet::new();
        for a in [
            setup::Action::Ok,
            setup::Action::Back,
            setup::Action::Minus,
            setup::Action::Plus,
            setup::Action::Name,
        ] {
            assert!(ids.insert(setup_action_id(a)));
        }
        for d in 0..3 {
            assert!(ids.insert(setup_action_id(setup::Action::Difficulty(d))));
        }
        for part in 0..3 {
            for description in [false, true] {
                assert!(ids.insert(setup_action_id(setup::Action::Scroll { description, part })));
            }
            assert!(ids.insert(setup_action_id(setup::Action::MapScroll { part })));
        }
        // Original map enumeration retains at most256 rows; this ID namespace
        // is distinct from the supplied seven condition rows and generic actions.
        for row in 0..256 {
            assert!(ids.insert(setup_action_id(setup::Action::MapRow(row))));
        }
        for row in 0..7 {
            assert!(ids.insert(setup_action_id(setup::Action::Row(row))));
        }
        assert!(!matching_release(
            Some(PointerTarget::Setup(setup::Action::MapRow(0))),
            Some(PointerTarget::Setup(setup::Action::MapRow(1)))
        ));
        assert!(!matching_release(
            Some(PointerTarget::Setup(setup::Action::MapScroll { part: 2 })),
            Some(PointerTarget::Setup(setup::Action::Scroll {
                description: false,
                part: 2
            }))
        ));
    }
}
