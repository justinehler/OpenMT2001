//! Direct-IP lobby route. Connect/probe work lives in network::lobby::worker;
//! no blocking socket calls, DNS or worker joins occur in this UI adapter.
use super::skirmish_route::{decode, encode, objective};
use crate::{skirmish_roster as roster, skirmish_setup as setup};
use openmt2001::{
    level_catalog::{Catalog, Labels},
    network::{
        RowKind,
        lobby::{
            Action, Settings, Snapshot,
            worker::{Handle, Open, Selection, Update, View},
        },
    },
    render::{PreparedScene, Renderer},
    skirmish::roster::{Kind, Participant},
};
use std::path::{Path, PathBuf};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Browser,
    Setup,
    Waiting,
    Lobby,
}
#[derive(Default)]
struct Browser {
    address: String,
    result: Option<Selection>,
    selected: bool,
    editing: bool,
}
impl Browser {
    fn invalidate(&mut self) {
        self.result = None;
        self.selected = false;
    }
    fn selection(&self) -> Option<&Selection> {
        self.result.as_ref().filter(|r| {
            self.selected
                && self.address.parse().ok() == Some(r.address)
                && r.snapshot.capacity().0 < r.snapshot.capacity().1
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Browser(usize),
    Setup(setup::Action),
    Lobby(usize),
    Difficulty(usize),
}
pub struct Route {
    pub request: Option<crate::intro::connected_start::Request>,
    pub busy: bool,
    cancellation: Option<openmt2001::network::lobby::worker::Cancellation>,
    pub visible: bool,
    pub notice: Option<String>,
    page: Page,
    browser: Browser,
    setup: Option<setup::State>,
    setup_panel: setup::SetupPanel,
    roster_panel: roster::RosterPanel,
    worker: Option<Handle>,
    view: Option<View>,
    catalog: Catalog,
    scripts: PathBuf,
    executable: Vec<u8>,
    strings: Vec<Vec<u8>>,
    map: Vec<u8>,
    token_names: [Vec<u8>; 10],
    name: Vec<u8>,
    token: i8,
    keyboard: Option<Target>,
    press: Option<Target>,
    status: String,
}
fn inside(b: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3]
}
impl Route {
    pub fn new(
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        scripts: &Path,
        strings: &[&[u8]],
        executable: &[u8],
    ) -> Result<Self, String> {
        let map = strings.first().ok_or("Missing language mapping")?.to_vec();
        let roster_panel =
            roster::RosterPanel::new(index, archive, installer, strings, executable, 0xbb000a28)?;
        let token_names = roster_panel.token_names(|g| decode(&map, g))?;
        let mut setup_panel =
            setup::SetupPanel::new(index, archive, installer, strings, executable, 0xbb000a28)?;
        setup_panel.network_mode();
        Ok(Self {
            request: None,
            busy: false,
            cancellation: None,
            visible: false,
            notice: None,
            page: Page::Browser,
            browser: Browser::default(),
            setup: None,
            setup_panel,
            roster_panel,
            worker: None,
            view: None,
            catalog: Catalog::default(),
            scripts: scripts.into(),
            executable: executable.to_vec(),
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            map,
            token_names,
            name: vec![],
            token: 0,
            keyboard: None,
            press: None,
            status: String::new(),
        })
    }
    pub fn open(&mut self, catalog: Catalog, name: &[u8], token: usize) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self.worker = None;
        self.request = None;
        self.busy = false;
        self.view = None;
        self.catalog = catalog;
        self.name = name.to_vec();
        self.token = token as i8;
        self.visible = true;
        self.page = Page::Browser;
        self.notice = None;
        self.browser.invalidate();
        self.keyboard = None;
        self.press = None;
        self.status.clear();
    }
    pub fn finish(&mut self, error: Option<String>) {
        self.cancellation = None;
        self.request = None;
        self.busy = false;
        if let Some(error) = error {
            self.page = Page::Browser;
            self.notice = Some(error);
        } else {
            self.visible = false;
        }
    }
    pub fn poll(&mut self) {
        while let Some(update) = self.worker.as_ref().and_then(Handle::poll) {
            match update {
                Update::Prepare(ticket) => {
                    self.cancellation = self.worker.as_ref().map(Handle::cancellation);
                    self.request = Some(crate::intro::connected_start::Request {
                        ticket,
                        worker: self.worker.take().unwrap(),
                    });
                    self.busy = true;
                    self.page = Page::Waiting;
                    self.status = "Validating connected content...".into();
                }
                Update::Validated => (),
                Update::Probed(selection) => {
                    self.browser.result = Some(selection);
                    self.browser.selected = false;
                    self.worker = None;
                    self.status = "Select the session, then Join.".into();
                }
                Update::Lobby(view) => {
                    self.view = Some(view);
                    self.page = Page::Lobby;
                    self.keyboard = None;
                }
                Update::Notice(message) => self.notice = Some(message),
                Update::Failed(message) => {
                    self.worker = None;
                    self.view = None;
                    self.page = Page::Browser;
                    self.browser.invalidate();
                    self.notice = Some(message);
                    self.status.clear();
                }
            }
        }
    }
    fn browser_controls(&self) -> Vec<(usize, [f32; 4], Vec<u8>, bool)> {
        let busy = self.worker.is_some();
        vec![
            (
                0,
                [55., 90., 230., 125.],
                b"Host (TCP 2300)".to_vec(),
                !busy,
            ),
            (
                1,
                [55., 145., 435., 180.],
                format!("Address: {}", self.browser.address).into_bytes(),
                !busy,
            ),
            (
                2,
                [450., 145., 585., 180.],
                b"Probe".to_vec(),
                !busy && self.browser.address.parse::<std::net::SocketAddr>().is_ok(),
            ),
            (
                3,
                [55., 200., 585., 235.],
                self.browser
                    .result
                    .as_ref()
                    .map(|r| {
                        let (used, capacity) = r.snapshot.capacity();
                        let mut line = if self.browser.selected {
                            b"> ".to_vec()
                        } else {
                            vec![]
                        };
                        line.extend_from_slice(&r.snapshot.settings.name);
                        line.extend_from_slice(format!("  {used}/{capacity} humans").as_bytes());
                        line
                    })
                    .unwrap_or_else(|| b"No probed session".to_vec()),
                !busy && self.browser.result.is_some(),
            ),
            (
                4,
                [450., 245., 585., 280.],
                b"Join".to_vec(),
                !busy && self.browser.selection().is_some(),
            ),
            (5, [55., 395., 190., 430.], b"Cancel".to_vec(), true),
        ]
    }
    fn enabled(&self, id: usize) -> bool {
        let Some(view) = &self.view else {
            return false;
        };
        let local = view.snapshot.local_row(view.local_peer);
        match id {
            0..=11 => {
                local == Some(id / 2)
                    || view.host && view.snapshot.rows[id / 2].player.kind == RowKind::Ai
            }
            12..=17 => view.host && view.snapshot.rows[id - 12].peer.is_none(),
            18..=23 => local == Some(id - 18),
            24 => true,
            25 => view.host && view.snapshot.start_enabled(),
            _ => false,
        }
    }
    fn target(&self, p: [f32; 2]) -> Option<Target> {
        if self.notice.is_some() {
            return Some(Target::Browser(6));
        }
        match self.page {
            Page::Browser => self
                .browser_controls()
                .iter()
                .find(|(_, b, _, enabled)| *enabled && inside(*b, p))
                .map(|(id, ..)| Target::Browser(*id)),
            Page::Setup => self
                .setup_panel
                .hit(self.setup.as_ref()?, p)
                .ok()?
                .map(Target::Setup),
            Page::Waiting => inside([55., 395., 190., 430.], p).then_some(Target::Browser(5)),
            Page::Lobby => {
                for id in 0..26 {
                    if self.enabled(id) && inside(self.roster_panel.action_bounds(id).ok()?, p) {
                        return Some(Target::Lobby(id));
                    }
                }
                if self.view.as_ref()?.host {
                    for row in 0..6 {
                        if self.view.as_ref()?.snapshot.rows[row].player.kind == RowKind::Ai
                            && inside(self.roster_panel.name_bounds(row).ok()?, p)
                        {
                            return Some(Target::Difficulty(row));
                        }
                    }
                }
                None
            }
        }
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        self.target(p).map(|_| 0)
    }
    pub fn cancel_input(&mut self) {
        self.press = None;
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        if let Some(Target::Setup(
            action @ (setup::Action::Scroll { part: 2, .. } | setup::Action::MapScroll { part: 2 }),
        )) = self.press
        {
            let result =
                self.setup_panel
                    .activate(self.setup.as_mut().unwrap(), action, p, &mut |l| {
                        objective(&self.executable, &self.scripts, l)
                    });
            if let Err(e) = result {
                self.notice = Some(e);
            }
        }
    }
    pub fn mouse(&mut self, p: Option<[f32; 2]>, down: bool) {
        let target = p.and_then(|p| self.target(p));
        self.keyboard = None;
        if down {
            self.press = target;
            if let Some(p) = p {
                self.pointer(p);
            }
        } else if self
            .press
            .take()
            .is_some_and(|origin| Some(origin) == target)
        {
            self.activate(target, p);
        }
    }
    fn activate(&mut self, target: Option<Target>, p: Option<[f32; 2]>) {
        if self.notice.take().is_some() {
            return;
        }
        if let Err(e) = self.activate_inner(target, p) {
            self.notice = Some(e);
        }
    }
    fn activate_inner(
        &mut self,
        target: Option<Target>,
        p: Option<[f32; 2]>,
    ) -> Result<(), String> {
        match target {
            Some(Target::Browser(0)) => {
                let maps = crate::skirmish_maps::Catalog::discover(&self.scripts)?;
                let mut state = setup::State::open(
                    self.catalog.clone(),
                    &self.name,
                    None,
                    self.setup_panel.options(),
                    |l| objective(&self.executable, &self.scripts, l),
                )?;
                state.set_maps(
                    maps.names().iter().map(|s| s.as_bytes().to_vec()).collect(),
                    maps.default_index(),
                )?;
                self.setup = Some(state);
                self.page = Page::Setup;
            }
            Some(Target::Browser(1)) => self.browser.editing = true,
            Some(Target::Browser(2)) => {
                let address =
                    self.browser.address.parse().map_err(
                        |_| "Enter an IP address and port, for example 100.120.50.24:2300.",
                    )?;
                self.browser.invalidate();
                self.browser.editing = false;
                self.worker = Some(Handle::open(Open::Probe(address))?);
                self.status = "Probing...".into();
            }
            Some(Target::Browser(3)) => self.browser.selected = self.browser.result.is_some(),
            Some(Target::Browser(4)) => {
                let selection = self
                    .browser
                    .selection()
                    .ok_or("Select a probed session with an open slot.")?
                    .clone();
                self.worker = Some(Handle::open(Open::Join {
                    selection,
                    name: self.name.clone(),
                    token: self.token,
                })?);
                self.page = Page::Waiting;
                self.status = "Joining...".into();
            }
            Some(Target::Browser(5)) => self.back(),
            Some(Target::Setup(action)) => {
                let state = self.setup.as_mut().ok_or("Setup missing")?;
                let outcome = if let Some(p) = p {
                    self.setup_panel.activate(state, action, p, &mut |l| {
                        objective(&self.executable, &self.scripts, l)
                    })?
                } else {
                    self.setup_panel
                        .keyboard_activate(state, action, &mut |l| {
                            objective(&self.executable, &self.scripts, l)
                        })?
                };
                match outcome {
                    setup::Outcome::Back => self.page = Page::Browser,
                    setup::Outcome::Roster(h) => {
                        let settings = Settings {
                            name: h.session_name,
                            level: h.level_id,
                            parameter_bits: h.objective_target.to_bits(),
                            difficulty: h.difficulty as u8,
                            map: h.map,
                        };
                        self.worker = Some(Handle::open(Open::Host {
                            address: "0.0.0.0:2300".parse().unwrap(),
                            snapshot: Snapshot::host(settings, self.name.clone(), self.token)
                                .map_err(|e| e.to_string())?,
                            token_names: self.token_names.clone(),
                        })?);
                        self.page = Page::Waiting;
                        self.status = "Opening host...".into();
                    }
                    _ => (),
                }
            }
            Some(Target::Lobby(24)) => self.back(),
            Some(Target::Lobby(id)) if self.enabled(id) => {
                let action = match id {
                    0..=11 => Action::Character {
                        row: id / 2,
                        direction: if id % 2 == 0 { -1 } else { 1 },
                    },
                    12..=17 => Action::Kind(id - 12),
                    18..=23 => Action::Ready,
                    25 => Action::Start,
                    _ => return Ok(()),
                };
                self.worker.as_ref().ok_or("Lobby closed")?.action(action)?;
            }
            Some(Target::Difficulty(row)) => self
                .worker
                .as_ref()
                .ok_or("Lobby closed")?
                .action(Action::Difficulty(row))?,
            _ => (),
        }
        self.keyboard = None;
        self.press = None;
        Ok(())
    }
    pub fn activate_keyboard(&mut self) {
        self.activate(self.keyboard, None);
    }
    pub fn navigate(&mut self, delta: i32) {
        let targets = match self.page {
            Page::Browser => self
                .browser_controls()
                .iter()
                .filter(|(_, _, _, enabled)| *enabled)
                .map(|(id, ..)| Target::Browser(*id))
                .collect::<Vec<_>>(),
            Page::Setup => self
                .setup
                .as_ref()
                .map(|s| {
                    self.setup_panel
                        .keyboard_actions(s)
                        .into_iter()
                        .map(Target::Setup)
                        .collect()
                })
                .unwrap_or_default(),
            Page::Waiting => vec![Target::Browser(5)],
            Page::Lobby => {
                let mut targets = (0..26)
                    .filter(|id| self.enabled(*id))
                    .map(Target::Lobby)
                    .collect::<Vec<_>>();
                if let Some(view) = self.view.as_ref().filter(|v| v.host) {
                    targets.extend(
                        (0..6)
                            .filter(|&r| view.snapshot.rows[r].player.kind == RowKind::Ai)
                            .map(Target::Difficulty),
                    );
                }
                targets
            }
        };
        if !targets.is_empty() {
            let next = self
                .keyboard
                .and_then(|t| targets.iter().position(|v| *v == t))
                .map(|i| (i as i32 + delta).rem_euclid(targets.len() as i32) as usize)
                .unwrap_or(if delta >= 0 { 0 } else { targets.len() - 1 });
            self.keyboard = Some(targets[next]);
        }
    }
    pub fn is_editing(&self) -> bool {
        match self.page {
            Page::Browser => self.browser.editing && self.worker.is_none(),
            Page::Setup => self.setup.as_ref().is_some_and(|s| s.name_focused),
            _ => false,
        }
    }
    pub fn text(&mut self, text: &str) {
        if self.notice.is_some() {
            return;
        }
        if self.page == Page::Setup {
            if let Some(s) = &mut self.setup {
                s.text(text);
            }
        } else if self.is_editing() {
            self.browser.invalidate();
            self.browser.address.extend(
                text.chars()
                    .filter(|c| c.is_ascii_graphic())
                    .take(64usize.saturating_sub(self.browser.address.len())),
            );
        }
    }
    pub fn erase(&mut self) {
        if self.page == Page::Setup {
            if let Some(s) = &mut self.setup {
                s.backspace();
            }
        } else if self.is_editing() {
            self.browser.invalidate();
            self.browser.address.pop();
        }
    }
    pub fn select_all(&mut self) {
        if self.is_editing() && self.page == Page::Browser {
            self.browser.address.clear();
            self.browser.invalidate();
        }
    }
    pub fn scroll(&mut self, p: Option<[f32; 2]>, delta: i32) {
        if self.page == Page::Setup
            && let (Some(s), Some(p)) = (&mut self.setup, p)
            && let Err(e) = self.setup_panel.scroll(s, p, delta)
        {
            self.notice = Some(e);
        }
    }
    pub fn back(&mut self) {
        if self.notice.take().is_some() {
            return;
        }
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self.request = None;
        self.busy = false;
        self.worker = None;
        self.view = None;
        self.keyboard = None;
        self.press = None;
        self.browser.invalidate();
        self.browser.editing = false;
        self.status.clear();
        if self.page == Page::Browser {
            self.visible = false;
        } else {
            self.page = Page::Browser;
        }
    }
    fn title(&self, level: u32) -> Result<Vec<u8>, String> {
        let Some(level) = self.catalog.find(level).map(|i| &self.catalog.levels[i]) else {
            return encode(&self.map, format!("Scenario {level}").as_bytes());
        };
        match level.labels() {
            Labels::Localized { title, .. } => self
                .strings
                .get(title as usize)
                .cloned()
                .ok_or("Scenario title missing".into()),
            Labels::Literal { title, .. } => encode(&self.map, &title),
        }
    }
    pub fn draw(
        &self,
        renderer: &Renderer,
        p: Option<[f32; 2]>,
        pressed: bool,
        panel_alpha: u8,
    ) -> Result<PreparedScene, String> {
        if self.page == Page::Setup {
            return self.setup_panel.draw(
                self.setup.as_ref().ok_or("Setup missing")?,
                renderer,
                p,
                pressed,
                match self.keyboard {
                    Some(Target::Setup(a)) => Some(a),
                    _ => None,
                },
                panel_alpha,
            );
        }
        if let Some(view) = self.view.as_ref().filter(|_| self.page == Page::Lobby) {
            let rows = std::array::from_fn(|i| {
                let row = &view.snapshot.rows[i];
                Participant {
                    kind: match row.player.kind {
                        RowKind::Human => Kind::Human,
                        RowKind::Ai => Kind::Ai,
                        _ => Kind::Empty,
                    },
                    token: usize::try_from(row.player.token).ok(),
                    name: row.player.name.clone(),
                    ready: row.player.ready,
                    difficulty: row.difficulty as i32,
                }
            });
            let title = self.title(view.snapshot.settings.level)?;
            let difficulty = self.strings
                [[1654, 1655, 1658][view.snapshot.settings.difficulty as usize]]
                .clone();
            let mut details = vec![view.snapshot.settings.name.clone()];
            for (i, row) in view
                .snapshot
                .rows
                .iter()
                .enumerate()
                .filter(|(_, r)| r.player.kind == RowKind::Ai)
            {
                details.push(
                    format!(
                        "Row {} AI difficulty: {}",
                        i + 1,
                        ["Easy", "Medium", "Hard"][row.difficulty as usize]
                    )
                    .into_bytes(),
                );
            }
            let selected = match self.keyboard.or_else(|| p.and_then(|p| self.target(p))) {
                Some(Target::Lobby(id)) => Some(id),
                _ => None,
            };
            return self.roster_panel.draw_rows(
                renderer,
                selected,
                pressed,
                [&title, &difficulty],
                |b| encode(&self.map, b),
                roster::Presentation {
                    visible: true,
                    busy: false,
                    name_focused: false,
                    rows: &rows,
                    enabled: std::array::from_fn(|id| self.enabled(id)),
                    start_enabled: view.snapshot.start_enabled(),
                    local_row: view.snapshot.local_row(view.local_peer).unwrap_or(0),
                    closed: std::array::from_fn(|i| {
                        view.snapshot.rows[i].player.kind == RowKind::Closed
                    }),
                    details: &details,
                },
                panel_alpha,
            );
        }
        let controls = if self.page == Page::Waiting {
            vec![(5, [55., 395., 190., 430.], b"Cancel".to_vec(), true)]
        } else {
            self.browser_controls()
        };
        let mut lines = vec![
            b"Direct IP: enter an IP address and port.".to_vec(),
            self.status.as_bytes().to_vec(),
        ];
        if let Some(result) = &self.browser.result {
            // Scenario title uses the local language, as in the existing setup panel.
            let title = self.title(result.snapshot.settings.level)?;
            lines.push(decode(&self.map, &title)?);
            lines.push(
                format!(
                    "Level {} | difficulty {} | target {}",
                    result.snapshot.settings.level,
                    result.snapshot.settings.difficulty + 1,
                    f32::from_bits(result.snapshot.settings.parameter_bits)
                )
                .into_bytes(),
            );
        }
        let selected = match self.keyboard.or_else(|| p.and_then(|p| self.target(p))) {
            Some(Target::Browser(id)) => Some(id),
            _ => None,
        };
        self.roster_panel.browser(
            renderer,
            b"Multi-Player",
            &lines,
            &controls,
            selected,
            pressed,
            |b| encode(&self.map, b),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn join_requires_a_selected_probe_for_the_current_address_and_capacity() {
        let snapshot = Snapshot::host(
            Settings {
                name: b"Real session".to_vec(),
                level: 300001,
                parameter_bits: 2f32.to_bits(),
                difficulty: 0,
                map: b"default".to_vec(),
            },
            b"Host".to_vec(),
            0,
        )
        .unwrap();
        let mut b = Browser {
            address: "127.0.0.1:2300".into(),
            ..Default::default()
        };
        assert!(b.selection().is_none());
        b.result = Some(Selection {
            address: b.address.parse().unwrap(),
            session: [1; 16],
            snapshot,
        });
        assert!(b.selection().is_none());
        b.selected = true;
        assert!(b.selection().is_some());
        b.address = "127.0.0.1:2301".into();
        assert!(b.selection().is_none());
        b.invalidate();
        assert!(b.result.is_none());
        assert!(!b.selected);
    }
}
