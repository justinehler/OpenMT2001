//! Native profile store and original login/registration panel presentation.
//! Original profiles stay private immutable inputs; this host writes only V2 data.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
    ui_mesh,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Profiles {
    pub names: Vec<String>,
    pub current: Option<usize>,
    path: PathBuf,
}
impl Profiles {
    /// The selected player's name as original code-page bytes: the identity
    /// that keys every V2 file of the profile, and the player name sessions
    /// draw through the language's glyph row.
    pub fn current_code_page(&self) -> Option<Vec<u8>> {
        let name = self.names.get(self.current?)?;
        Some(ui::text_field::code_page(name))
    }
    fn clear_orphan_progress(&self, name: &str) -> Result<(), String> {
        let root = self.path.parent().ok_or("profile directory")?;
        let path = crate::campaign_store::profile_path(root, &ui::text_field::code_page(name));
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    fn clear_orphan_options(&self, name: &str) -> Result<(), String> {
        let root = self.path.parent().ok_or("profile directory")?;
        let identity = ui::text_field::code_page(name);
        crate::preferred_token::clear(root, &identity)?;
        let path = crate::profile_settings::profile_path(root, &identity);
        match fs::remove_file(path.with_extension("board")) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    fn clear_orphan_saves(&self, name: &str) -> Result<(), String> {
        let root = self.path.parent().ok_or("profile directory")?;
        crate::save_menu::clear_profile_saves(root, &ui::text_field::code_page(name))
    }
    fn delete_chosen(&mut self, index: usize) -> Result<(), String> {
        let name = self.names.remove(index);
        self.current = self.current.and_then(|current| {
            if current == index {
                None
            } else if current > index {
                Some(current - 1)
            } else {
                Some(current)
            }
        });
        self.save()?;
        // Once the profile list is published, a cleanup error must not roll
        // the in-memory selection back to a deleted profile. Registration
        // retries cleanup before that same encoded identity can be reused.
        if let Err(error) = self.clear_orphan_progress(&name) {
            eprintln!("Profile progress cleanup: {error}");
        }
        if let Err(error) = self.clear_orphan_options(&name) {
            eprintln!("Profile options cleanup: {error}");
        }
        if let Err(error) = self.clear_orphan_saves(&name) {
            eprintln!("Profile saves cleanup: {error}");
        }
        Ok(())
    }
    fn load(root: PathBuf) -> Result<Self, String> {
        let path = root.join("profiles.txt");
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    names: vec![],
                    current: None,
                    path,
                });
            }
            Err(e) => return Err(e.to_string()),
        };
        let mut lines = raw.lines();
        if lines.next() != Some("OpenMT2001 profiles 1") {
            return Err("unsupported V2 profile file".into());
        }
        let selected = lines
            .next()
            .ok_or("missing selected profile")?
            .parse::<i32>()
            .map_err(|e| e.to_string())?;
        let names: Vec<String> = lines.map(str::to_owned).collect();
        if names.iter().any(|n| !valid_name(n))
            || names
                .iter()
                .enumerate()
                .any(|(i, n)| names[..i].contains(n))
        {
            return Err("invalid V2 profile name".into());
        }
        let current = if selected < 0 {
            None
        } else {
            Some(selected as usize)
        };
        if current.is_some_and(|i| i >= names.len()) {
            return Err("selected profile outside list".into());
        }
        Ok(Self {
            names,
            current,
            path,
        })
    }
    fn save(&self) -> Result<(), String> {
        let root = self.path.parent().ok_or("profile directory")?;
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let raw = format!(
            "OpenMT2001 profiles 1\n{}\n{}",
            self.current.map_or(-1, |i| i as i32),
            self.names
                .iter()
                .map(|n| format!("{n}\n"))
                .collect::<String>()
        );
        let temporary = root.join("profiles.tmp");
        fs::write(&temporary, raw).map_err(|e| e.to_string())?;
        fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
    }
}
/// A stored or entered profile name: printable Windows-1252 text within the
/// editor's 128-byte hard cap. Width and the filename filter are input rules
/// (443aa0), so names saved before they applied still load.
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= ui::text_field::HARD_CAP
        && name
            .chars()
            .all(|c| ui::text_field::windows_1252(c).is_some_and(|b| b >= 32))
}
pub fn default_directory() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("OpenMT2001")
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/OpenMT2001")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local/share")
            })
            .join("openmt2001")
    }
}
struct Assets {
    base: Vec<Surface>,
    modal: Vec<Surface>,
    bounds: Vec<[f32; 4]>,
    confirm: Vec<[f32; 4]>,
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    /// Language row 0: code-page byte minus 32 to glyph index.
    glyph_map: Vec<u8>,
    list_fonts: Vec<Surface>,
    list_tab: Vec<u8>,
    list_layout: openmt2001::frontend_list::Layout,
    labels: Vec<Vec<u8>>,
    slider_textures: Vec<Surface>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Notice {
    Delete,
    Duplicate,
}

pub struct Profile {
    assets: Assets,
    pub store: Profiles,
    pub visible: bool,
    pub name: Option<String>,
    first_registration: bool,
    chosen: Option<usize>,
    first: usize,
    error: Option<String>,
    notice: Option<Notice>,
    notice_keys: crate::desktop_quit::Modal,
    pointer_position: Option<[f32; 2]>,
}
const PROFILE_SCROLL: [f32; 4] = [440., 200., 456., 287.];
fn action_button_enabled(chosen: bool, modal: bool, i: usize) -> bool {
    !modal && i < 4 && (i == 0 || chosen)
}
impl Profile {
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        root: PathBuf,
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("missing profile asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("profile asset range".into())
        };
        let mesh = CachedMesh::parse(payload("gamedata2\\brand_new_panels\\login_select.tm")?)?;
        let regions = ui_mesh::regions(&mesh, 0)?;
        let pos = mesh.positions(0)?;
        let uv = mesh.uvs();
        let mut base = vec![];
        for part in mesh.parts.iter().take(4).chain(mesh.parts.get(6)) {
            let tex = TextureSet::parse(payload(mesh.texture_names[part.texture as usize])?)
                .map_err(|e| e.to_string())?;
            let mut s = Surface::new(&tex.textures[0]);
            for i in part.vertex_indices() {
                s.vertices
                    .push([pos[i][0], -pos[i][1], 0.5, 1., 1., 1., uv[i][0], uv[i][1]]);
                s.alpha.push(255);
            }
            base.push(s);
        }
        let generic = TextureSet::parse(payload("gamedata2\\GenericTextures640\\textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut slider_textures = vec![Surface::solid(); 109];
        for first in [105, 93] {
            for state in 0..4 {
                slider_textures[first + state] = Surface::new(
                    generic
                        .textures
                        .get(first + state)
                        .ok_or("original Profile scrollbar arrow missing")?,
                );
            }
        }
        for (part, texture) in [(7, 287), (8, 273), (9, 274)] {
            let mut s = Surface::new(&generic.textures[texture]);
            let [l, t, r, b] = regions[part].bounds;
            s.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            base.push(s);
        }
        let mesh = CachedMesh::parse(payload("gamedata2\\brand_new_panels\\confirm_panel.tm")?)?;
        let confirm = ui_mesh::regions(&mesh, 0)?
            .into_iter()
            .map(|r| r.bounds)
            .collect();
        let pos = mesh.positions(0)?;
        let uv = mesh.uvs();
        let tex = TextureSet::parse(payload(mesh.texture_names[0])?).map_err(|e| e.to_string())?;
        let mut border = Surface::new(&tex.textures[0]);
        for i in mesh.parts[0].vertex_indices() {
            border
                .vertices
                .push([pos[i][0], -pos[i][1], 0.5, 1., 1., 1., uv[i][0], uv[i][1]]);
            border.alpha.push(255);
        }
        let fonts = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[2..4]
            .iter()
            .map(Surface::new)
            .collect();
        let tab = fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let list_tab = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let list_fonts = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[6..8]
            .iter()
            .map(Surface::new)
            .collect();
        let mut list_bounds = regions[5].bounds;
        // 42e000 insets the text viewport vertically by one on each edge;
        // 42df90 uses font3, gap2, and scroll=-1 (cancelling its top inset).
        list_bounds[3] -= 2.;
        let metric = openmt2001::frontend_list::Layout::from_font(list_bounds, &list_tab)?;
        let list_layout = openmt2001::frontend_list::Layout::new(list_bounds, metric.pitch, 0.)?;
        let labels = [1430, 1434, 1435, 1439, 0x662, 0x645]
            .iter()
            .map(|&i| {
                strings
                    .get(i)
                    .map(|s| s.to_vec())
                    .ok_or("profile language ID".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let store = Profiles::load(root)?;
        Ok(Self::from_store(
            Assets {
                base,
                modal: vec![border],
                bounds: regions.into_iter().map(|r| r.bounds).collect(),
                confirm,
                fonts,
                tab,
                glyph_map: strings
                    .first()
                    .ok_or("profile language glyph row missing")?
                    .to_vec(),
                list_fonts,
                list_tab,
                list_layout,
                labels,
                slider_textures,
            },
            store,
        ))
    }
    fn from_store(assets: Assets, store: Profiles) -> Self {
        // 491835..49184d opens registration instead of the main menu when
        // profile discovery returns no entries. Menu presents this after the intro.
        let first_registration = store.names.is_empty();
        let chosen = store.current;
        Self {
            assets,
            store,
            visible: first_registration,
            name: first_registration.then(String::new),
            first_registration,
            chosen,
            first: 0,
            error: None,
            notice: None,
            notice_keys: Default::default(),
            pointer_position: None,
        }
    }
    pub fn open_selection(&mut self) {
        self.visible = true;
        self.name = None;
        self.first_registration = false;
    }
    fn open_notice(&mut self, notice: Notice) {
        self.notice = Some(notice);
        self.notice_keys.open(self.pointer_position);
    }
    pub fn prompt_pending(&self) -> bool {
        self.notice.is_some()
    }
    pub fn generic_prompt(&self) -> Option<crate::human_dialog::Dialog> {
        use crate::human_dialog::{Argument, Dialog};
        let mut dialog = Dialog {
            portrait: crate::human_dialog::NO_PORTRAIT,
            timeout: false,
            ..Default::default()
        };
        match self.notice? {
            Notice::Duplicate => {
                // 495f8f..496007; reply 495ea0 restores registration unchanged.
                dialog.title = 0x6cd;
                dialog.body = 0x6ce;
                dialog.buttons = [0x467, 0];
                dialog.button_count = 1;
                dialog.callback = 0x495ea0;
            }
            Notice::Delete => {
                // 498eab..498f47; zero reply deletes, one preserves selection.
                dialog.title = 0x663;
                dialog.body = 0x666;
                dialog.buttons = [0x664, 0x665];
                dialog.button_count = 2;
                dialog.callback = 0x498c70;
                let name = self.store.names.get(self.chosen?)?;
                let mut argument = [0; 96];
                argument[..name.len()].copy_from_slice(name.as_bytes());
                dialog.arguments[0] = Argument::Name(argument);
                dialog.argument_count = 1;
            }
        }
        Some(dialog)
    }
    pub fn ui_event1(&mut self, sample: crate::desktop_quit::Sample) -> bool {
        let edges = self.notice_keys.event1(sample);
        let mut replied = false;
        for (button, edge) in edges.into_iter().enumerate() {
            if edge && self.prompt_pending() {
                self.activate(button);
                replied = true;
            }
        }
        replied
    }
    pub fn modal_open(&self) -> bool {
        self.name.is_some() || self.notice.is_some()
    }
    pub fn count(&self) -> usize {
        if let Some(dialog) = self.generic_prompt() {
            return dialog.button_count;
        }
        if self.first_registration {
            1
        } else if self.name.is_some() || self.notice.is_some() {
            2
        } else {
            4 + self.store.names.len()
                + if self.store.names.len() > self.assets.list_layout.visible_rows() {
                    3
                } else {
                    0
                }
        }
    }
    /// The four LOGIN_SELECT action children are generic 43ed20 buttons.
    /// Selected-player actions carry the same disabled admission as their art.
    pub fn generic_button_enabled(&self, i: usize) -> bool {
        action_button_enabled(
            self.chosen.is_some(),
            self.name.is_some() || self.notice.is_some(),
            i,
        )
    }
    fn modal_button_enabled(&self, i: usize) -> bool {
        match i {
            0 => self.name.as_ref().is_none_or(|name| valid_name(name)),
            1 => !self.first_registration,
            _ => false,
        }
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        let hit = |[l, t, r, b]: [f32; 4]| p[0] >= l && p[0] <= r && p[1] >= t && p[1] <= b;
        if self.notice.is_some() {
            return None; // Menu owns shared prompt geometry.
        }
        if self.name.is_some() {
            return [4, 6].iter().enumerate().find_map(|(i, &part)| {
                (self.modal_button_enabled(i) && hit(self.assets.confirm[part])).then_some(i)
            });
        }
        for (i, part) in [11, 10, 13, 12].iter().enumerate() {
            if self.generic_button_enabled(i) && hit(self.assets.bounds[*part]) {
                return Some(i);
            }
        }
        if self.store.names.len() > self.assets.list_layout.visible_rows() {
            for (part, bounds) in ui::slider_arrows(PROFILE_SCROLL, true)
                .into_iter()
                .enumerate()
            {
                if hit(bounds) {
                    return Some(4 + self.store.names.len() + part);
                }
            }
            if hit(ui::slider_track(PROFILE_SCROLL, true)) {
                return Some(4 + self.store.names.len() + 2);
            }
        }
        self.assets
            .list_layout
            .hit(p, self.first, self.store.names.len())
            .map(|row| 4 + row)
    }
    pub fn back(&mut self) {
        // 495d1a..495d45 disables Back only for startup registration.
        if self.first_registration || self.notice.is_some() {
            return; // Warning replies use Return/Backspace, not Escape.
        }
        self.error = None;
        if self.name.take().is_none() {
            self.visible = false;
        }
    }
    /// Glyph indices for a player name in any of the original fonts.
    pub fn name_glyphs(&self, name: &str) -> Vec<u8> {
        ui::text_field::FieldFont {
            tab: &self.assets.tab,
            map: &self.assets.glyph_map,
        }
        .glyphs(name)
    }
    pub fn input(&mut self, text: &str) {
        if self.notice.is_some() {
            return;
        }
        if let Some(name) = &mut self.name {
            // 495b50 configures the registration field filename-safe, 90 px.
            let font = ui::text_field::FieldFont {
                tab: &self.assets.tab,
                map: &self.assets.glyph_map,
            };
            ui::text_field::Rule::PROFILE_NAME.type_text(name, text, font);
            self.error = None;
        }
    }
    pub fn erase(&mut self) {
        if self.notice.is_some() {
            return;
        }
        if let Some(name) = &mut self.name {
            name.pop();
            self.error = None;
        }
    }
    pub fn reveal(&mut self, index: usize) {
        if self.name.is_none()
            && self.notice.is_none()
            && index >= 4
            && index - 4 < self.store.names.len()
        {
            let row = index - 4;
            if row < self.first {
                self.first = row;
            } else if row >= self.first + self.assets.list_layout.visible_rows() {
                self.first = row + 1 - self.assets.list_layout.visible_rows();
            }
        }
    }
    pub fn scroll(&mut self, delta: i32) {
        if self.name.is_some() || self.notice.is_some() {
            return;
        }
        self.first = (self.first as i32 + delta).clamp(
            0,
            self.store
                .names
                .len()
                .saturating_sub(self.assets.list_layout.visible_rows()) as i32,
        ) as usize;
    }
    pub fn prompt_mouse(&mut self, down: bool, hit: Option<usize>) -> bool {
        if let Some(button) = self.notice_keys.left(down, hit) {
            self.activate(button);
            return true;
        }
        false
    }
    pub fn prompt_right(&mut self, down: bool) {
        self.notice_keys.right(down);
    }
    pub fn cancel_input(&mut self) {
        self.pointer_position = None;
        self.notice_keys.cancel_pointer();
    }
    pub fn prompt_controls(&self) -> &crate::desktop_quit::Modal {
        &self.notice_keys
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        self.pointer_position = Some(p);
        self.notice_keys.cursor(p);
    }
    pub fn drag_track(&mut self, origin: Option<usize>, p: [f32; 2]) -> bool {
        if self.name.is_some()
            || self.notice.is_some()
            || self.store.names.len() <= self.assets.list_layout.visible_rows()
            || origin != Some(4 + self.store.names.len() + 2)
        {
            return false;
        }
        let maximum = self
            .store
            .names
            .len()
            .saturating_sub(self.assets.list_layout.visible_rows()) as i32;
        let next = ui::slider_selection(ui::slider_track(PROFILE_SCROLL, true), p[1], maximum, true)
            as usize;
        let changed = next != self.first;
        self.first = next;
        changed
    }
    pub fn is_scroll_track(&self, index: Option<usize>) -> bool {
        self.store.names.len() > self.assets.list_layout.visible_rows()
            && index == Some(4 + self.store.names.len() + 2)
    }
    pub fn activate(&mut self, index: usize) {
        self.error = None;
        let before = (self.store.names.clone(), self.store.current, self.chosen);
        let result = (|| -> Result<(), String> {
            if let Some(notice) = self.notice {
                if index > 1 {
                    return Ok(());
                }
                self.notice = None;
                self.notice_keys = Default::default();
                // 42a2dc delivers Backspace even to one-button records;
                // 495ea0 reopens registration only for reply zero. Original
                // +498 marks New from selection (the inverse of startup);
                // neither warning reply changes it or opens Player Selection.
                if notice == Notice::Duplicate && index == 1 && !self.first_registration {
                    self.name = None;
                }
                if notice == Notice::Delete
                    && index == 0
                    && let Some(i) = self.chosen
                {
                    self.store.delete_chosen(i)?;
                    self.chosen = None;
                    self.first = self.first.min(
                        self.store
                            .names
                            .len()
                            .saturating_sub(self.assets.list_layout.visible_rows()),
                    );
                }
                return Ok(());
            }
            if let Some(name) = &self.name {
                if index == 1 {
                    self.back();
                    return Ok(());
                }
                // 495ef8/496015 ignores an empty OK and retains input focus.
                if name.is_empty() {
                    return Ok(());
                }
                if !valid_name(name) {
                    return Err("Enter a player name".into());
                }
                if self.store.names.contains(name) {
                    self.open_notice(Notice::Duplicate);
                    return Ok(());
                }
                self.store.clear_orphan_progress(name)?;
                self.store.clear_orphan_options(name)?;
                self.store.clear_orphan_saves(name)?;
                self.store.names.push(name.clone());
                self.chosen = Some(self.store.names.len() - 1);
                self.store.current = self.chosen;
                self.store.save()?;
                self.name = None;
                // 495f27..495f5c returns New to Player Selection, but the
                // first registration admits the new player to the main menu.
                if self.first_registration {
                    self.visible = false;
                    self.first_registration = false;
                }
                return Ok(());
            }
            if index < 4 && !self.generic_button_enabled(index) {
                return Ok(());
            }
            let scroll_base = 4 + self.store.names.len();
            if (scroll_base..scroll_base + 3).contains(&index) {
                match index - scroll_base {
                    0 => self.scroll(-1),
                    1 => self.scroll(1),
                    _ => {
                        if let Some(point) = self.pointer_position {
                            let maximum = self
                                .store
                                .names
                                .len()
                                .saturating_sub(self.assets.list_layout.visible_rows())
                                as i32;
                            self.first = ui::slider_selection(
                                ui::slider_track(PROFILE_SCROLL, true),
                                point[1],
                                maximum,
                                true,
                            ) as usize;
                        }
                    }
                }
                return Ok(());
            }
            match index {
                0 => self.name = Some(String::new()),
                1 => {
                    if self.chosen.is_some() {
                        self.open_notice(Notice::Delete);
                    }
                }
                2 => {
                    if self.chosen.is_some() {
                        self.store.current = self.chosen;
                        self.store.save()?;
                        self.visible = false;
                    }
                }
                3 => self.visible = false,
                i => {
                    if i - 4 < self.store.names.len() {
                        self.chosen = Some(i - 4);
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            (self.store.names, self.store.current, self.chosen) = before;
            self.error = Some(e);
        }
    }
    pub fn draw(
        &self,
        renderer: &Renderer,
        selected: Option<usize>,
        pressed: bool,
        panel_alpha: u8,
    ) -> Result<PreparedScene, String> {
        let mut shapes = Surface::solid();
        let mut slider_shapes = Surface::solid();
        let mut slider_controls = Vec::new();
        let mut fonts = self.assets.fonts.clone();
        let mut list_fonts = self.assets.list_fonts.clone();
        let [l, t, r, b] = self.assets.bounds[5];
        shapes.rect([l, t, r, b], [0xbb000a28; 4]);
        shapes.rect(
            self.assets.bounds[4],
            [0xff909090, 0xff909090, 0x80909090, 0x80909090],
        );
        let mut title = self.assets.bounds[4];
        title[1] += 4.;
        ui::text(
            &mut fonts,
            &self.assets.tab,
            &self.assets.labels[4],
            title,
            true,
            0xffffffff,
        )?;
        for (i, name) in self
            .store
            .names
            .iter()
            .enumerate()
            .skip(self.first)
            .take(self.assets.list_layout.visible_rows())
        {
            let mut row = self.assets.list_layout.text_bounds(i - self.first);
            row[0] += 2.;
            if self.chosen == Some(i) {
                // Profile's separate 42e430 widget uses a full-pitch bar at +2.
                let pitch = self.assets.list_layout.pitch;
                let top = t + (i - self.first) as f32 * pitch + 2.;
                // 0x42e430 paints this list itself but reads the shared selection colour at 0x536294.
                shapes.rect(
                    [l, top, r, top + pitch],
                    [openmt2001::frontend_list::SELECTED_COLOR; 4],
                );
            }
            let encoded = self.name_glyphs(name);
            ui::text_widget_style0(
                &mut list_fonts,
                &self.assets.list_tab,
                &encoded,
                row,
                0xffffffff,
            )?;
        }
        let scroll_base = 4 + self.store.names.len();
        ui::Slider {
            bounds: PROFILE_SCROLL,
            vertical: true,
            value: self.first as f32,
            range: [
                0.,
                self.store
                    .names
                    .len()
                    .saturating_sub(self.assets.list_layout.visible_rows()) as f32,
            ],
            pointer: self.pointer_position,
            pressed: (pressed
                && selected.is_some_and(|i| (scroll_base..scroll_base + 3).contains(&i)))
            .then_some(self.pointer_position)
            .flatten(),
        }
        .paint(
            &mut slider_shapes,
            &mut slider_controls,
            &self.assets.slider_textures,
        );
        for (i, part) in [11, 10, 13, 12].iter().enumerate() {
            let bounds = self.assets.bounds[*part];
            let disabled = ((1..=3).contains(&i) && self.chosen.is_none())
                || ((self.name.is_some() || self.notice.is_some()) && i != 0);
            ui::button(
                &mut shapes,
                bounds,
                [[166, 133, 0], [175, 6, 6], [80, 148, 34], [175, 6, 6]][i],
                selected == Some(i),
                pressed && selected == Some(i),
                disabled,
            )?;
            let mut text = bounds;
            text[1] += (bounds[3] - bounds[1] - 14.) / 2.;
            ui::text(
                &mut fonts,
                &self.assets.tab,
                &self.assets.labels[i],
                text,
                true,
                if disabled { 0xff808080 } else { 0xffffffff },
            )?;
        }
        let mut modal = Surface::solid();
        let mut modal_fonts = self.assets.fonts.clone();
        let modal_open = self.name.is_some() && self.notice.is_none();
        if modal_open {
            modal.rect(self.assets.confirm[1], [0xaa000000; 4]);
            modal.rect(
                self.assets.confirm[2],
                [0xff909090, 0xff909090, 0x80909090, 0x80909090],
            );
            let mut title = self.assets.confirm[2];
            title[1] += 4.;
            ui::text(
                &mut modal_fonts,
                &self.assets.tab,
                &self.assets.labels[5],
                title,
                true,
                0xffffffff,
            )?;
            let mut input = self.assets.confirm[3];
            input[1] += 10.;
            input[3] -= 10.;
            modal.rect(
                input,
                [0xff002896; 4], // Registration painter 495e5e.
            );
            let encoded = self.name_glyphs(self.name.as_deref().unwrap_or(""));
            // Registration configures style 4 on the full CONFIRM_TEXT region.
            ui::text_entry(
                &mut modal_fonts,
                &mut modal,
                &self.assets.tab,
                &encoded,
                self.assets.confirm[3],
                true,
                true,
            )?;
            for (i, part) in [4, 6].iter().enumerate() {
                let bounds = self.assets.confirm[*part];
                ui::button(
                    &mut modal,
                    bounds,
                    if i == 0 { [80, 148, 34] } else { [175, 6, 6] },
                    selected == Some(i),
                    pressed && selected == Some(i),
                    !self.modal_button_enabled(i),
                )?;
                let mut text = bounds;
                text[1] += 5.;
                ui::text(
                    &mut modal_fonts,
                    &self.assets.tab,
                    &self.assets.labels[if i == 0 { 2 } else { 3 }],
                    text,
                    true,
                    if i == 1 && self.first_registration {
                        0xff808080
                    } else {
                        0xffffffff
                    },
                )?;
            }
        }
        if let Some(error) = &self.error {
            let encoded: Vec<_> = error
                .bytes()
                .filter(|c| (32..127).contains(c))
                .take(60)
                .map(|c| c - 32)
                .collect();
            ui::text(
                &mut modal_fonts,
                &self.assets.tab,
                &encoded,
                [20., 465., 620., 480.],
                true,
                0xffff9999,
            )?;
        }
        let backdrop = crate::modal_frame::Backdrop::new(&self.assets.base[..5], panel_alpha);
        let mut draws = Vec::new();
        if !self.first_registration {
            draws.extend(backdrop.draws());
            draws.extend(self.assets.base[5..].iter().map(Surface::draw));
            draws.push(shapes.draw());
            draws.push(slider_shapes.draw());
            draws.extend(slider_controls.iter().map(Surface::draw));
            draws.extend(fonts.iter().map(Surface::draw));
            draws.extend(list_fonts.iter().map(Surface::draw));
        }
        if modal_open {
            draws.extend(self.assets.modal.iter().map(Surface::draw));
            draws.push(modal.draw());
        }
        draws.extend(modal_fonts.iter().map(Surface::draw));
        renderer.prepare(
            &draws,
            [
                2. / 640.,
                0.,
                0.,
                0.,
                0.,
                -2. / 480.,
                0.,
                0.,
                0.,
                0.,
                1.,
                0.,
                -1.,
                1.,
                0.,
                1.,
            ],
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Assets, PROFILE_SCROLL, Profile, Profiles, action_button_enabled};
    use crate::ui;
    use std::{fs, path::PathBuf};

    fn synthetic_assets() -> Assets {
        let mut confirm = vec![[0.; 4]; 7];
        confirm[4] = [200., 265., 300., 290.];
        confirm[6] = [340., 265., 440., 290.];
        let (tab, glyph_map) = ui::text_field::synthetic_font();
        Assets {
            base: vec![],
            modal: vec![],
            bounds: vec![[0.; 4]; 14],
            confirm,
            fonts: vec![],
            tab,
            glyph_map,
            list_fonts: vec![],
            list_tab: vec![],
            list_layout: openmt2001::frontend_list::Layout::new([198., 200., 440., 285.], 11., 0.)
                .unwrap(),
            labels: vec![],
            slider_textures: vec![],
        }
    }
    /// Empty the registration field the only way the original offers:
    /// Backspace removes one trailing byte at a time (443aa0).
    fn clear_name(profile: &mut Profile) {
        for _ in 0..profile.name.as_ref().map_or(0, |name| name.chars().count()) {
            profile.erase();
        }
        assert_eq!(profile.name.as_deref(), Some(""));
    }
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "omt-first-registration-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn load(&self) -> Profile {
            Profile::from_store(synthetic_assets(), Profiles::load(self.0.clone()).unwrap())
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn first_launch_registration_blocks_back_and_requires_a_name() {
        let scratch = Scratch::new();
        let mut profile = scratch.load();
        assert!(profile.visible);
        assert_eq!(profile.name.as_deref(), Some(""));
        assert_eq!(profile.count(), 1, "keyboard navigation cannot select Back");
        assert_eq!(profile.hit([250., 275.]), None, "empty OK is disabled");
        assert_eq!(profile.hit([390., 275.]), None, "Back is disabled");
        profile.activate(0);
        assert!(profile.error.is_none(), "empty OK only retains name entry");
        assert!(profile.visible && profile.name.is_some());
        assert!(profile.store.names.is_empty());
        profile.input("Alice");
        assert_eq!(profile.hit([250., 275.]), Some(0));
        profile.back(); // Escape uses this same route from Menu::back.
        profile.activate(1); // Button activation cannot bypass admission.
        assert!(profile.visible);
        assert_eq!(profile.name.as_deref(), Some("Alice"));
        assert!(!scratch.0.join("profiles.txt").exists());
    }

    #[test]
    fn first_registration_selects_and_persists_then_relaunch_opens_normal_menu() {
        let scratch = Scratch::new();
        let mut profile = scratch.load();
        profile.input("Alice");
        profile.activate(0);
        assert!(profile.error.is_none());
        assert!(!profile.visible && profile.name.is_none());
        assert_eq!(profile.store.names, ["Alice"]);
        assert_eq!(profile.store.current, Some(0));
        let mut relaunched = scratch.load();
        assert!(!relaunched.visible && relaunched.name.is_none());
        assert_eq!(relaunched.store.current, Some(0));
        // Discovery uses the profile list, even if no current row was saved.
        relaunched.store.current = None;
        relaunched.store.save().unwrap();
        assert!(!scratch.load().visible);
    }

    #[test]
    fn registration_field_applies_the_shared_name_rule() {
        let scratch = Scratch::new();
        let mut profile = scratch.load();
        // 90 px of the registration font: 7 W (84 px), or exactly 30 i.
        profile.input(&"W".repeat(20));
        assert_eq!(profile.name.as_deref(), Some("W".repeat(7).as_str()));
        clear_name(&mut profile);
        profile.input(&"i".repeat(40));
        assert_eq!(profile.name.as_deref(), Some("i".repeat(30).as_str()));
        clear_name(&mut profile);
        profile.input("Ren\u{e9}e: A/B?");
        assert_eq!(profile.name.as_deref(), Some("Ren\u{e9}e AB"));
        assert_eq!(
            profile.name_glyphs("\u{e9}"),
            [0xe9 - 0xc0 + 112],
            "high bytes draw through the language row"
        );
        profile.activate(0);
        assert!(profile.error.is_none());
        // The name round-trips through profiles.txt, and its files are keyed
        // by its code-page bytes.
        let relaunched = scratch.load();
        assert_eq!(relaunched.store.names, ["Ren\u{e9}e AB"]);
        let identity = relaunched.store.current_code_page().unwrap();
        assert_eq!(identity, b"Ren\xe9e AB");
        assert_eq!(
            crate::campaign_store::profile_path(&scratch.0, &identity)
                .file_name()
                .unwrap(),
            "52656ee965204142.lua"
        );
    }

    #[test]
    fn new_registration_returns_to_selection_and_can_cancel_with_zero_profiles() {
        let scratch = Scratch::new();
        let mut profile = scratch.load();
        profile.input("Alice");
        profile.activate(0);
        profile.open_selection();
        profile.activate(0); // New from Player Selection.
        assert_eq!(profile.count(), 2);
        assert_eq!(profile.hit([390., 275.]), Some(1));
        profile.input("Alice");
        profile.activate(0);
        assert!(profile.prompt_pending(), "duplicate opens the Warning");
        assert!(profile.error.is_none());
        assert_eq!(profile.store.names, ["Alice"]);
        profile.activate(0); // Warning OK returns to registration.
        clear_name(&mut profile);
        profile.input("Bob");
        profile.activate(0);
        assert!(profile.error.is_none());
        assert!(profile.visible && profile.name.is_none());
        assert_eq!(profile.store.current, Some(1));
        profile.store.delete_chosen(1).unwrap();
        profile.store.delete_chosen(0).unwrap();
        profile.chosen = None;
        profile.activate(0);
        profile.back();
        assert!(profile.visible && profile.name.is_none());
        profile.activate(0);
        profile.activate(1);
        assert!(profile.visible && profile.name.is_none());
        // A new process with this empty stored list must require registration again.
        let relaunched = scratch.load();
        assert!(relaunched.visible && relaunched.name.is_some());
    }

    #[test]
    fn first_registration_duplicate_replies_preserve_text_and_startup_gate() {
        use winit::keyboard::{KeyCode, PhysicalKey};
        for key in [KeyCode::Enter, KeyCode::Backspace] {
            let scratch = Scratch::new();
            let mut profile = scratch.load();
            // Exercise discovery of a colliding name while startup entry is open.
            profile.store.names.push("Alice".into());
            profile.input("Alice");
            profile.activate(0);
            assert!(profile.prompt_pending() && profile.first_registration);
            assert_eq!(profile.generic_prompt().unwrap().title, 0x6cd);
            let mut keys = crate::desktop_quit::Keys::default();
            profile.ui_event1(keys.sample());
            keys.physical(PhysicalKey::Code(key), true, false);
            assert!(profile.ui_event1(keys.sample()));
            assert!(!profile.prompt_pending());
            assert!(profile.visible && profile.first_registration);
            assert_eq!(profile.name.as_deref(), Some("Alice"));
            assert_eq!(profile.count(), 1);
            assert_eq!(profile.hit([390., 275.]), None);
            profile.back();
            profile.activate(1);
            assert_eq!(profile.name.as_deref(), Some("Alice"));
            assert!(profile.first_registration);
            clear_name(&mut profile);
            profile.input("Bob");
            profile.activate(0);
            assert!(!profile.visible && !profile.first_registration);
            assert_eq!(profile.store.current, Some(1));
            assert_eq!(scratch.load().store.names, ["Alice", "Bob"]);
        }
    }

    #[test]
    fn first_registration_stays_open_when_profile_storage_fails() {
        let scratch = Scratch::new();
        let mut profile = scratch.load();
        fs::create_dir(scratch.0.join("profiles.txt")).unwrap();
        profile.input("Alice");
        profile.activate(0);
        assert!(profile.error.is_some());
        assert!(profile.visible && profile.first_registration);
        assert_eq!(profile.name.as_deref(), Some("Alice"));
        assert!(profile.store.names.is_empty());
        assert_eq!(profile.store.current, None);
        profile.back();
        assert!(profile.name.is_some());
    }

    fn profile(root: PathBuf) -> Profile {
        let (tab, glyph_map) = ui::text_field::synthetic_font();
        Profile {
            assets: Assets {
                base: vec![],
                modal: vec![],
                bounds: vec![[0.; 4]; 14],
                confirm: vec![[0.; 4]; 7],
                fonts: vec![],
                tab,
                glyph_map,
                list_fonts: vec![],
                list_tab: vec![],
                list_layout: openmt2001::frontend_list::Layout::new(
                    [198., 200., 440., 285.],
                    11.,
                    0.,
                )
                .unwrap(),
                labels: vec![],
                slider_textures: vec![],
            },
            store: Profiles {
                names: vec!["Alice".into(), "Bob".into()],
                current: Some(0),
                path: root.join("profiles.txt"),
            },
            visible: true,
            name: None,
            first_registration: false,
            chosen: Some(1),
            first: 0,
            error: None,
            notice: None,
            notice_keys: Default::default(),
            pointer_position: None,
        }
    }
    fn strings() -> Vec<Vec<u8>> {
        let mut strings = vec![vec![]; 0x6cf];
        for (id, text) in [
            (0x663, "Delete Profile"),
            (0x664, "Yes"),
            (0x665, "No"),
            (0x666, "Do you want to delete profile %s?"),
            (0x467, "OK"),
            (0x6cd, "Warning"),
            (
                0x6ce,
                "A profile with this name already exists. Please use a different name or delete the other profile.",
            ),
        ] {
            strings[id] = text.bytes().map(|b| b - 32).collect();
        }
        strings
    }
    fn text(bytes: &[u8]) -> String {
        bytes.iter().map(|b| char::from(b + 32)).collect()
    }
    #[test]
    fn duplicate_warning_restores_editable_name_and_keeps_modal_frame_faded() {
        let mut profile = profile(PathBuf::from("unused"));
        let strings = strings();
        profile.activate(0);
        profile.input("Alice");
        profile.activate(0);
        assert!(profile.error.is_none());
        let dialog = profile.generic_prompt().unwrap();
        assert_eq!(text(&dialog.title_text(&strings).unwrap()), "Warning");
        assert_eq!(
            text(&dialog.paragraphs(&strings).unwrap()[0]),
            "A profile with this name already exists. Please use a different name or delete the other profile."
        );
        assert_eq!(
            (dialog.buttons, dialog.button_count, dialog.timeout),
            ([0x467, 0], 1, false)
        );
        profile.input("ignored");
        profile.erase();
        profile.back();
        assert_eq!(profile.name.as_deref(), Some("Alice"));
        let mut fade = crate::modal_frame::Fade::default();
        for _ in 0..26 {
            fade.advance(profile.modal_open());
        }
        assert_eq!(fade.alpha(), 125);
        // Out-of-range replies cannot dismiss the warning.
        profile.activate(2);
        assert!(profile.prompt_pending());
        profile.activate(0);
        assert!(!profile.prompt_pending());
        assert!(profile.visible && profile.modal_open());
        assert!(!fade.advance(profile.modal_open()));
        profile.erase();
        profile.input("ia");
        assert_eq!(profile.name.as_deref(), Some("Alicia"));
        assert_eq!(profile.store.names, ["Alice", "Bob"]);
    }
    #[test]
    fn warning_backspace_matches_original_nonzero_reply() {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let mut profile = profile(PathBuf::from("unused"));
        let mut keys = crate::desktop_quit::Keys::default();
        profile.activate(0);
        profile.input("Alice");
        profile.activate(0);
        profile.ui_event1(keys.sample());
        keys.physical(PhysicalKey::Code(KeyCode::Backspace), true, false);
        assert!(profile.ui_event1(keys.sample()));
        assert!(!profile.prompt_pending() && profile.name.is_none() && profile.visible);
        assert_eq!(profile.store.names, ["Alice", "Bob"]);
    }
    #[test]
    fn delete_prompt_mouse_and_original_key_replies_share_completion() {
        use winit::keyboard::{KeyCode, PhysicalKey};
        let root = std::env::temp_dir().join(format!(
            "openmt-profile-dialog-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut profile = profile(root.clone());
        let mut keys = crate::desktop_quit::Keys::default();
        profile.activate(1);
        let dialog = profile.generic_prompt().unwrap();
        assert_eq!(
            text(&dialog.title_text(&strings()).unwrap()),
            "Delete Profile"
        );
        assert_eq!(
            text(&dialog.paragraphs(&strings()).unwrap()[0]),
            "Do you want to delete profile Bob?"
        );
        assert_eq!(
            (dialog.buttons, dialog.button_count, dialog.timeout),
            ([0x664, 0x665], 2, false)
        );
        let mut fade = crate::modal_frame::Fade::default();
        fade.advance(profile.modal_open());
        assert_eq!(fade.alpha(), 250);
        profile.activate(4);
        assert!(profile.prompt_pending());
        profile.ui_event1(keys.sample());
        keys.physical(PhysicalKey::Code(KeyCode::Backspace), true, false);
        assert!(profile.ui_event1(keys.sample()));
        assert!(!profile.prompt_pending());
        assert_eq!(profile.chosen, Some(1));
        assert_eq!(profile.store.names, ["Alice", "Bob"]);
        profile.activate(1);
        assert!(!profile.prompt_mouse(true, Some(1)));
        assert!(profile.prompt_mouse(false, Some(1)));
        assert_eq!(profile.store.names, ["Alice", "Bob"]);
        profile.activate(1);
        profile.ui_event1(keys.sample());
        keys.physical(PhysicalKey::Code(KeyCode::Enter), true, false);
        assert!(profile.ui_event1(keys.sample()));
        assert_eq!(profile.store.names, ["Alice"]);
        assert_eq!(profile.store.current, Some(0));
        assert_eq!(profile.chosen, None);
        assert_eq!(Profiles::load(root.clone()).unwrap().names, ["Alice"]);
        profile.chosen = Some(0);
        profile.activate(1);
        profile.prompt_mouse(true, Some(0));
        profile.prompt_mouse(false, Some(0));
        assert!(profile.store.names.is_empty());
        assert!(!profile.modal_open());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires original v1.4 language resources"]
    fn original_profile_dialog_strings_match_verified_call_sites() {
        let offsets = fs::read(openmt2001::fixtures::path("LANGUAGE_OFF")).unwrap();
        let bytes = fs::read(openmt2001::fixtures::path("LANGUAGE_BIN")).unwrap();
        let mut at = 0;
        let rows = offsets
            .as_chunks::<2>()
            .0
            .iter()
            .map(|n| {
                let end = at + usize::from(u16::from_le_bytes(*n));
                let row = bytes[at..end].to_vec();
                at = end;
                row
            })
            .collect::<Vec<_>>();
        for id in [0x663, 0x664, 0x665, 0x666, 0x467, 0x6cd, 0x6ce] {
            assert_eq!(rows[id], strings()[id], "language ID {id:x}");
        }
    }

    #[test]
    fn deleting_a_different_profile_preserves_and_reindexes_the_active_one() {
        let root = std::env::temp_dir().join(format!(
            "omt2-profile-delete-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = Profiles {
            names: vec!["Alice".into(), "Bob".into(), "Carol".into()],
            current: Some(0),
            path: root.join("profiles.txt"),
        };
        store.delete_chosen(1).unwrap();
        assert_eq!(store.names, ["Alice", "Carol"]);
        assert_eq!(
            store.current,
            Some(0),
            "deleting Bob must keep active Alice"
        );
        store.current = Some(1);
        store.delete_chosen(0).unwrap();
        assert_eq!(store.names, ["Carol"]);
        assert_eq!(
            store.current,
            Some(0),
            "deleting an earlier row reindexes active Carol"
        );
        store.delete_chosen(0).unwrap();
        assert!(store.current.is_none());
        fs::remove_file(root.join("profiles.txt")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn profile_generic_actions_exclude_rows_and_disabled_controls() {
        assert!(action_button_enabled(false, false, 0));
        assert!(!action_button_enabled(false, false, 1));
        assert!(!action_button_enabled(false, false, 2));
        assert!(!action_button_enabled(false, false, 3));
        assert!(action_button_enabled(true, false, 1));
        assert!(action_button_enabled(true, false, 2));
        assert!(action_button_enabled(true, false, 3));
        assert!(!action_button_enabled(true, false, 4));
        assert!(!action_button_enabled(true, true, 0));
    }
    #[test]
    fn profile_scrollbar_hits_and_moves_without_selecting_a_hidden_row() {
        let mut bounds = vec![[0.; 4]; 14];
        bounds[5] = [198., 200., 440., 287.];
        let (tab, glyph_map) = ui::text_field::synthetic_font();
        let mut profile = Profile {
            assets: Assets {
                base: vec![],
                modal: vec![],
                bounds,
                confirm: vec![],
                fonts: vec![],
                tab,
                glyph_map,
                list_fonts: vec![],
                list_tab: vec![],
                list_layout: openmt2001::frontend_list::Layout::new(
                    [198., 200., 440., 285.],
                    11.,
                    0.,
                )
                .unwrap(),
                labels: vec![],
                slider_textures: vec![],
            },
            store: Profiles {
                names: (0..9).map(|i| format!("Player {i}")).collect(),
                current: None,
                path: PathBuf::from("unused/profiles.txt"),
            },
            visible: true,
            name: None,
            first_registration: false,
            chosen: None,
            first: 0,
            error: None,
            notice: None,
            notice_keys: Default::default(),
            pointer_position: None,
        };
        let base = 4 + profile.store.names.len();
        assert_eq!(profile.hit([448., 279.]), Some(base + 1));
        profile.reveal(base + 1);
        assert_eq!(profile.first, 0);
        profile.activate(base + 1);
        assert_eq!(profile.first, 1);
        profile.pointer([448., 260.]);
        assert_eq!(profile.hit([448., 260.]), Some(base + 2));
        profile.activate(base + 2);
        assert!(profile.first >= 1 && profile.first <= 2);
        let before_up = profile.first;
        profile.activate(base);
        assert_eq!(profile.first, before_up - 1);
        assert!(profile.drag_track(Some(base + 2), [448., 999.]));
        assert_eq!(profile.first, 2);
        assert!(profile.drag_track(Some(base + 2), [448., -100.]));
        assert_eq!(profile.first, 0);
        profile.store.names.truncate(1);
        assert_eq!(profile.hit([448., 279.]), None);
    }

    #[test]
    #[ignore = "requires original v1.4 Profile slider fixture"]
    fn profile_scrollbar_matches_original_constructor() {
        let path = openmt2001::fixtures::path("NATIVE_PROFILE_SCROLLBAR");
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[..8], b"MTPRSL01");
        let mut at = 8;
        let mut u32_at = || {
            let value = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            at += 4;
            value
        };
        assert_eq!(u32_at(), 0x037f);
        assert_eq!(u32_at(), 5);
        let source_bounds: [f32; 4] = std::array::from_fn(|_| f32::from_bits(u32_at()));
        assert_eq!(source_bounds[2].trunc(), PROFILE_SCROLL[0]);
        let args: [u32; 5] = std::array::from_fn(|_| u32_at());
        assert_eq!(args, [6, 440, 200, 87, 1]);
        let outer: [f32; 4] = std::array::from_fn(|_| f32::from_bits(u32_at()));
        let track: [f32; 5] = std::array::from_fn(|_| f32::from_bits(u32_at()));
        assert_eq!(outer, ui::slider_track(PROFILE_SCROLL, true));
        assert_eq!(track[..4], outer);
        for (part, texture) in [105, 93].into_iter().enumerate() {
            assert_eq!(u32_at(), texture);
            let arrow: [f32; 4] = std::array::from_fn(|_| f32::from_bits(u32_at()));
            assert_eq!(arrow, ui::slider_arrows(PROFILE_SCROLL, true)[part]);
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires original v1.4 inputs and an offscreen graphics device"]
    fn capture_original_profile_scrollbar() {
        use openmt2001::{
            flyby::CameraSample,
            render::Renderer,
            render_scene::{self, OriginalAssets, Request, Source},
        };
        let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let executable = openmt2001::fixtures::path("EXE");
        let language = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let installer = openmt2001::fixtures::path("PARAMETERS")
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let glyphs = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0usize;
        let rows = offsets
            .chunks_exact(2)
            .map(|length| {
                let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
                let row = &glyphs[at..end];
                at = end;
                row
            })
            .collect::<Vec<_>>();
        let strings = crate::tooltip::localized_strings(&installer, &rows).unwrap();
        let refs = strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let root = std::env::temp_dir().join(format!(
            "omt-v2-profile-scroll-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut profile = Profile::load(&index, &archive, &installer, &refs, root.clone()).unwrap();
        profile.store.names = vec!["Justin".into()];
        profile.chosen = Some(0);
        profile.open_selection();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let exe = read("OPENMT2001_EXE");
        let scene = render_scene::assemble(Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets {
                executable: &exe,
                directory: &directory,
                archive: &archive,
            }),
            suncols: Some(&suncols),
            phase: Some(12),
            camera: Some(CameraSample {
                eye: [26., 586., -695.],
                target: [26., 463., -595.],
                up: [0., 1., 0.],
                vertical_fov: 45.,
            }),
            width: 1280,
            height: 960,
            animated: true,
        })
        .unwrap();
        let panel = profile.draw(&renderer, None, false, 255).unwrap();
        crate::capture_test::capture_scene_overlays(
            &renderer,
            &scene,
            &[&panel],
            "frontend/profile-scrollbar-city.png",
        );
    }
}
