//! Original load panel for original imports and V2 retained-session saves.
//! 493f90 supplies buttons; 494100 supplies labels; 494760 reads tag2000 metadata.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::PathBuf, time::SystemTime};

#[path = "load/refusal.rs"]
pub(crate) mod refusal;
pub(crate) use refusal::Failure;
// The level text helpers are library code (#99).
pub(crate) use openmt2001::level_text::{LevelTextCatalog, ascii, level_labels, wrap_lines};

const LIST_SCROLL: [f32; 4] = [305., 70., 321., 383.];
const INFO_SCROLL: [f32; 4] = [588., 287., 604., 378.];

struct Entry {
    path: PathBuf,
    name: Vec<u8>,
    metadata: Result<[u32; 4], String>,
    modified: Option<SystemTime>,
    /// Only a validated slot in this V2 profile may be removed here.
    writable_id: Option<String>,
}
impl Entry {
    fn read(
        path: PathBuf,
        name: Vec<u8>,
        modified: Option<SystemTime>,
        writable_id: Option<String>,
    ) -> Self {
        let metadata = (|| {
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            openmt2001::save::read_level_metadata(&bytes)
        })();
        Self {
            path,
            name,
            metadata,
            modified,
            writable_id,
        }
    }
}

pub struct Load {
    base: Vec<Surface>,
    bounds: Vec<[f32; 4]>,
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    list_fonts: Vec<Surface>,
    list_layout: openmt2001::frontend_list::Layout,
    info_layout: openmt2001::frontend_list::Layout,
    list_tab: Vec<u8>,
    description_fonts: Vec<Surface>,
    description_tab: Vec<u8>,
    slider_textures: Vec<Surface>,
    title_fonts: Vec<Surface>,
    title_tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    level_text: LevelTextCatalog,
    directories: Vec<PathBuf>,
    app_root: PathBuf,
    import_directory: PathBuf,
    campaign_filename: Option<String>,
    profile_root: PathBuf,
    entries: Vec<Entry>,
    chosen: Option<usize>,
    first: usize,
    description_first: usize,
    over_description: bool,
    pointer_position: Option<[f32; 2]>,
    pub visible: bool,
    pub busy: bool,
    pub request: Option<PathBuf>,
    pub message: Option<String>,
    delete_confirmation: Option<String>,
    refusal_messages: refusal::Messages,
    refusal_prompt: Option<crate::prompt::Request>,
}
impl Load {
    pub(crate) fn apply_board(
        &mut self,
        board: &crate::intro::camera_live::ui_text_resources::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

    pub fn new(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &std::path::Path,
        strings: &[&[u8]],
        directory: PathBuf,
        profile_root: PathBuf,
        image: &openmt2001::pe::PeImage<'_>,
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("missing load asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("load asset range".into())
        };
        let (base, bounds) = ui::panel_base(
            index,
            archive,
            "gamedata2\\brand_new_panels\\load_menu_V2.tm",
            &[0, 1, 2, 3, 4, 5, 7],
        )?;
        let textures = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let generic = TextureSet::parse(payload("gamedata2\\GenericTextures640\\textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut slider_textures = vec![Surface::solid(); 109];
        for first in [105, 93] {
            for state in 0..4 {
                slider_textures[first + state] = Surface::new(
                    generic
                        .textures
                        .get(first + state)
                        .ok_or("original Load scrollbar arrow missing")?,
                );
            }
        }
        // 493f90 selects font3 with a two-unit gap and three-unit inset.
        let list_tab = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let mut list_bounds = bounds[8];
        list_bounds[3] -= 25.;
        list_bounds[2] -= 16.;
        // 4940f0's list and info primary payloads use horizontal0/vertical3.
        let list_layout = openmt2001::frontend_list::Layout::from_font_with_insets(
            list_bounds,
            &list_tab,
            [0., 3.],
        )?;
        let description_tab = fs::read(installer.join("Standard/fonts/tahoma11noantip.tab"))
            .map_err(|e| e.to_string())?;
        let mut info_bounds = bounds[21];
        info_bounds[2] -= 16.;
        let info_layout = openmt2001::frontend_list::Layout::from_font_with_insets(
            info_bounds,
            &description_tab,
            [0., 3.],
        )?;
        Ok(Self {
            base,
            bounds,
            list_fonts: textures.textures[6..8].iter().map(Surface::new).collect(),
            list_layout,
            info_layout,
            list_tab,
            fonts: textures.textures[2..4].iter().map(Surface::new).collect(),
            tab: fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab"))
                .map_err(|e| e.to_string())?,
            description_fonts: textures.textures[4..6].iter().map(Surface::new).collect(),
            description_tab,
            slider_textures,
            title_fonts: textures.textures[8..10].iter().map(Surface::new).collect(),
            title_tab: fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
                .map_err(|e| e.to_string())?,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            level_text: LevelTextCatalog::default(),
            directories: vec![directory.clone()],
            app_root: profile_root.clone(),
            import_directory: directory,
            campaign_filename: None,
            profile_root: profile_root.join("profiles").join("_unselected"),
            entries: vec![],
            chosen: None,
            first: 0,
            description_first: 0,
            over_description: false,
            pointer_position: None,
            visible: false,
            busy: false,
            request: None,
            message: None,
            delete_confirmation: None,
            refusal_messages: refusal::Messages::from_original(image)?,
            refusal_prompt: None,
        })
    }
    /// Retain registered metadata using this panel's active language glyphs.
    pub fn set_level_catalog(&mut self, catalog: &openmt2001::level_catalog::Catalog) {
        self.level_text = LevelTextCatalog::new(catalog, &self.strings);
        self.description_first = 0;
    }
    /// Called when opening Load, since the selected profile can change after
    /// the menu assets were constructed.
    pub fn select_profile(&mut self, name: Option<&[u8]>) {
        self.campaign_filename = name.map(|name| {
            crate::campaign_store::profile_path(&self.app_root, name)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        });
        self.profile_root = name.map_or_else(
            || self.app_root.join("profiles").join("_unselected"),
            |name| crate::save_menu::selected_profile_root(&self.app_root, name),
        );
        self.directories = vec![
            self.profile_root.join("saves"),
            self.app_root.join("saves"),
            self.import_directory.clone(),
        ];
    }
    pub fn open(&mut self) {
        self.visible = true;
        self.busy = false;
        self.request = None;
        self.message = None;
        self.delete_confirmation = None;
        self.refusal_prompt = None;
        self.entries.clear();
        self.first = 0;
        self.description_first = 0;
        let result = (|| -> Result<(), String> {
            let mut seen = std::collections::BTreeSet::new();
            let writable_directory = self.profile_root.join("saves");
            let legacy_directory = self.app_root.join("saves");
            let legacy_paths = self
                .campaign_filename
                .as_deref()
                .map(|filename| crate::save_menu::matching_legacy_paths(&self.app_root, filename))
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            let writable_ids = crate::save_menu::list_slots(&self.profile_root)?
                .into_iter()
                .map(|slot| slot.id)
                .collect::<std::collections::BTreeSet<_>>();
            for directory in &self.directories {
                let files = match fs::read_dir(directory) {
                    Ok(f) => f,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e.to_string()),
                };
                for file in files {
                    let file = file.map_err(|e| e.to_string())?;
                    let regular = file.file_type().map_err(|e| e.to_string())?.is_file();
                    let path = file.path();
                    if directory == &legacy_directory
                        && directory != &writable_directory
                        && !legacy_paths.contains(&path)
                    {
                        continue;
                    }
                    if !path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("sav"))
                        || !regular
                    {
                        continue;
                    }
                    let canonical = fs::canonicalize(&path).map_err(|e| e.to_string())?;
                    if !seen.insert(canonical) {
                        continue;
                    }
                    let name = ascii(&path.file_stem().unwrap_or_default().to_string_lossy());
                    let modified = file
                        .metadata()
                        .ok()
                        .and_then(|metadata| metadata.modified().ok());
                    let writable_id = (directory == &writable_directory)
                        .then(|| file.file_name().to_str().map(str::to_owned))
                        .flatten()
                        .filter(|id| writable_ids.contains(id));
                    self.entries
                        .push(Entry::read(path, name, modified, writable_id));
                }
            }
            self.entries.sort_by(|a, b| {
                crate::save_menu::compare_save_names(
                    &a.path.file_name().unwrap_or_default().to_string_lossy(),
                    &b.path.file_name().unwrap_or_default().to_string_lossy(),
                )
                // Different import/profile directories may contain the same
                // name; retain a deterministic target for each displayed row.
                .then_with(|| a.path.cmp(&b.path))
            });
            Ok(())
        })();
        // Original 494ac0 clears the list selection at +0x388 after the
        // enumeration, including when a saved game already occupies row 0.
        self.chosen = None;
        if let Err(e) = result {
            self.message = Some(e);
        }
    }
    pub fn count(&self) -> usize {
        if self.refusal_prompt.is_some() {
            1
        } else if self.delete_confirmation.is_some() {
            2
        } else {
            3 + self.entries.len() + 6
        }
    }
    /// The bottom generic buttons share this +0x333 disabled state.
    pub fn generic_button_enabled(&self, i: usize) -> bool {
        match i {
            0 => self.chosen.is_some() && !self.busy,
            1 => true,
            2 => self
                .chosen
                .and_then(|row| self.entries.get(row))
                .is_some_and(|entry| entry.writable_id.is_some()),
            _ => false,
        }
    }
    pub fn prompt_pending(&self) -> bool {
        self.refusal_prompt.is_some() || self.delete_confirmation.is_some()
    }
    pub fn prompt_request(&self) -> Option<crate::prompt::Request> {
        self.refusal_prompt.clone().or_else(|| {
            let dialog = self.generic_prompt()?;
            Some(crate::prompt::Request {
                script: None,
                dialog: Some(dialog),
                title: dialog.title,
                body: dialog.body,
                dismissal_id: 0,
                card: None,
            })
        })
    }
    pub fn refuse(&mut self, failure: Failure) {
        self.busy = false;
        self.request = None;
        self.refusal_prompt = self.refusal_messages.prompt(&failure);
        self.message = match failure {
            refusal::Failure::Storage(error) => Some(error),
            _ => None,
        };
    }
    /// Original 495191 opens the shared 0x88c0d4 generic prompt. Its
    /// answer callback remains local to Load and deletes only a profile slot.
    pub fn generic_prompt(&self) -> Option<crate::human_dialog::Dialog> {
        let id = self.delete_confirmation.as_ref()?;
        let display = id.rsplit_once('.').map_or(id.as_str(), |(name, _)| name);
        let mut name = [0u8; 96];
        for (byte, character) in name.iter_mut().take(95).zip(display.chars()) {
            *byte = if (' '..='~').contains(&character) {
                character as u8
            } else {
                b'?'
            };
        }
        Some(crate::human_dialog::Dialog {
            title: 0x65e,
            body: 0x660,
            arguments: [
                crate::human_dialog::Argument::Name(name),
                crate::human_dialog::Argument::Integer(0),
                crate::human_dialog::Argument::Integer(0),
            ],
            argument_count: 1,
            buttons: [0x664, 0x665],
            button_count: 2,
            timeout: false,
            ..crate::human_dialog::Dialog::default()
        })
    }
    pub fn reveal(&mut self, i: usize) {
        if !self.prompt_pending() && i >= 3 && i < 3 + self.entries.len() {
            let row = i - 3;
            if row < self.first {
                self.first = row;
            } else if row >= self.first + self.list_layout.visible_rows() {
                self.first = row + 1 - self.list_layout.visible_rows();
            }
        }
    }
    fn info_rows(&self) -> usize {
        self.chosen
            .and_then(|index| self.entries.get(index))
            .and_then(|entry| entry.metadata.as_ref().ok())
            .and_then(|words| self.level_text.description(words[0], &self.strings))
            .and_then(|text| {
                crate::prompt::wrap_with_gap(
                    &self.description_tab,
                    text,
                    (self.bounds[21][2] - self.bounds[21][0] - 18.).trunc() as i32,
                    -1,
                )
                .ok()
            })
            .map_or(0, |(offsets, _)| offsets.len().saturating_sub(1))
    }
    pub fn scroll(&mut self, delta: i32) {
        if self.prompt_pending() {
            return;
        }
        if self.over_description {
            self.description_first = (self.description_first as i32 + delta).clamp(
                0,
                self.info_rows()
                    .saturating_sub(self.info_layout.visible_rows()) as i32,
            ) as usize;
            return;
        }
        self.first = (self.first as i32 + delta).clamp(
            0,
            self.entries
                .len()
                .saturating_sub(self.list_layout.visible_rows()) as i32,
        ) as usize;
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        self.pointer_position = Some(p);
        if self.prompt_pending() {
            return;
        }
        let [l, t, r, b] = self.bounds[21];
        self.over_description = p[0] >= l && p[0] <= r && p[1] >= t && p[1] <= b;
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        let inside = |b: [f32; 4]| p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3];
        if self.prompt_pending() {
            return None;
        }
        for (i, part) in [23, 24, 22].iter().enumerate() {
            // Native 443398 returns before generic hit/callback handling when
            // this child carries the +0x333 disabled state.
            if inside(self.bounds[*part]) && self.generic_button_enabled(i) {
                return Some(i);
            }
        }
        for (index, bounds) in [LIST_SCROLL, INFO_SCROLL].into_iter().enumerate() {
            let arrows = ui::slider_arrows(bounds, true);
            let part = if inside(arrows[0]) {
                Some(0)
            } else if inside(arrows[1]) {
                Some(1)
            } else if inside(ui::slider_track(bounds, true)) {
                Some(2)
            } else {
                None
            };
            if let Some(part) = part {
                return Some(3 + self.entries.len() + index * 3 + part);
            }
        }
        if let Some(row) = self.list_layout.hit(p, self.first, self.entries.len()) {
            return Some(3 + row);
        }
        None
    }
    pub fn activate(&mut self, i: usize) {
        if self.refusal_prompt.is_some() {
            if i == 0 {
                self.refusal_prompt = None;
            }
            return;
        }
        if let Some(id) = self.delete_confirmation.take() {
            if i == 0 {
                match crate::save_menu::delete_slot(&self.profile_root, &id) {
                    Ok(()) => self.open(),
                    Err(error) => self.message = Some(error),
                }
            }
            return;
        }
        // Keyboard activation uses the same disabled child state as pointer
        // input; it must not bypass the native generic-button guard.
        if i < 3 && !self.generic_button_enabled(i) {
            return;
        }
        if self.busy && i != 1 {
            return;
        }
        if i >= 3 + self.entries.len() {
            let control = i - 3 - self.entries.len();
            if control < 6 {
                let (index, part) = (control / 3, control % 3);
                let bounds = [LIST_SCROLL, INFO_SCROLL][index];
                let maximum = if index == 0 {
                    self.entries
                        .len()
                        .saturating_sub(self.list_layout.visible_rows())
                } else {
                    self.info_rows()
                        .saturating_sub(self.info_layout.visible_rows())
                };
                let current = if index == 0 {
                    self.first
                } else {
                    self.description_first
                };
                let selected = match part {
                    0 => current.saturating_sub(1),
                    1 => current.saturating_add(1).min(maximum),
                    _ => self.pointer_position.map_or(current, |point| {
                        ui::slider_selection(
                            ui::slider_track(bounds, true),
                            point[1],
                            maximum as i32,
                            true,
                        ) as usize
                    }),
                };
                if index == 0 {
                    self.first = selected;
                } else {
                    self.description_first = selected;
                }
            }
            return;
        }
        match i {
            0 => {
                if let Some(e) = self.chosen.and_then(|i| self.entries.get(i)) {
                    match &e.metadata {
                        Ok(_) => {
                            self.request = Some(e.path.clone());
                            self.busy = true;
                            self.message = Some("Loading saved game...".into());
                        }
                        Err(e) => self.message = Some(e.clone()),
                    }
                }
            }
            1 => self.visible = false,
            2 => {
                if let Some(entry) = self.chosen.and_then(|index| self.entries.get(index)) {
                    if let Some(id) = &entry.writable_id {
                        self.delete_confirmation = Some(id.clone());
                        self.message = None;
                    } else {
                        self.message = Some("Original saved games are read-only".into());
                    }
                }
            }
            i => {
                if i - 3 < self.entries.len() {
                    self.chosen = Some(i - 3);
                    self.description_first = 0;
                    self.message = None;
                }
            }
        }
    }
    pub fn back(&mut self) {
        if self.refusal_prompt.take().is_none() && self.delete_confirmation.take().is_none() {
            self.visible = false;
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
        let mut headings = Surface::solid();
        let mut controls = Vec::new();
        let mut fonts = self.fonts.clone();
        let mut list_fonts = self.list_fonts.clone();
        let mut description_fonts = self.description_fonts.clone();
        let mut title_fonts = self.title_fonts.clone();
        shapes.rect(
            self.bounds[6],
            [0xff0000ff, 0xff0000ff, 0x800000ff, 0x800000ff],
        );
        for part in [8, 11] {
            shapes.rect(self.bounds[part], [0xbb000a28; 4]);
        }
        for (part, id) in [(9, 0x65b), (10, 0x65c)] {
            headings.rect(
                self.bounds[part],
                [0xff909090, 0xff909090, 0x80909090, 0x80909090],
            );
            let mut b = self.bounds[part];
            b[1] += 4.;
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[id],
                b,
                true,
                0xffffffff,
            )?;
        }
        let mut title = self.bounds[6];
        title[1] += 3.;
        ui::text(
            &mut title_fonts,
            &self.title_tab,
            &self.strings[0x641],
            title,
            true,
            0xffffffff,
        )?;
        for (i, e) in self
            .entries
            .iter()
            .enumerate()
            .skip(self.first)
            .take(self.list_layout.visible_rows())
        {
            let row = self.list_layout.text_bounds(i - self.first);
            ui::list_highlight(
                &mut shapes,
                self.list_layout,
                i - self.first,
                self.chosen == Some(i),
                selected == Some(3 + i),
            );
            let (name, timestamp) = row_columns(row);
            ui::text_fit(
                &mut list_fonts,
                &self.list_tab,
                &e.name,
                name,
                false,
                0xffffffff,
            )?;
            if let Some(modified) = crate::save_menu::format_modified_local(e.modified) {
                ui::text_fit(
                    &mut list_fonts,
                    &self.list_tab,
                    &ascii(&modified),
                    timestamp,
                    false,
                    0xff6ed1e1,
                )?;
            }
        }
        for (part, id) in [(12, 1612), (14, 1615), (16, 1613), (18, 1619), (20, 1703)] {
            shapes.rect(self.bounds[part], [0xff087b18; 4]);
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[id],
                self.bounds[part],
                true,
                0xff00ff00,
            )?;
        }
        if let Some(entry) = self.chosen.and_then(|i| self.entries.get(i)) {
            ui::text(
                &mut fonts,
                &self.tab,
                &entry.name,
                self.bounds[13],
                true,
                0xffffffff,
            )?;
            if let Ok(words) = entry.metadata {
                let title = self.level_text.title(words[0], &self.strings);
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &title,
                    self.bounds[15],
                    true,
                    0xffffffff,
                )?;
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &ascii(&words[1].to_string()),
                    self.bounds[17],
                    true,
                    0xffffffff,
                )?;
                let money = f32::from_bits(words[2]);
                let symbol = self
                    .strings
                    .get(2243)
                    .ok_or("Load currency symbol missing")?;
                let suffix = self.strings.get(2244).and_then(|row| row.first()) != Some(&0x4c);
                let money_text = ui::formatted_float_money(money, symbol, suffix)?;
                ui::text(
                    &mut fonts,
                    &self.tab,
                    &money_text,
                    self.bounds[19],
                    true,
                    0xffffffff,
                )?;
                if let Some(description) = self.level_text.description(words[0], &self.strings) {
                    let info = self.bounds[21];
                    let (offsets, wrapped) = crate::prompt::wrap_with_gap(
                        &self.description_tab,
                        description,
                        (info[2] - info[0] - 18.).trunc() as i32,
                        -1,
                    )?;
                    for (line, range) in offsets
                        .windows(2)
                        .skip(self.description_first)
                        .take(self.info_layout.visible_rows())
                        .enumerate()
                    {
                        let end = range[1].saturating_sub(1).min(wrapped.len());
                        let row = &wrapped[range[0].min(end)..end];
                        ui::text_widget_style0(
                            &mut description_fonts,
                            &self.description_tab,
                            row,
                            self.info_layout.text_bounds(line),
                            0xffffffff,
                        )?;
                    }
                }
            }
        }
        for (index, bounds, value, maximum) in [
            (
                0,
                LIST_SCROLL,
                self.first,
                self.entries
                    .len()
                    .saturating_sub(self.list_layout.visible_rows()),
            ),
            (
                1,
                INFO_SCROLL,
                self.description_first,
                self.info_rows()
                    .saturating_sub(self.info_layout.visible_rows()),
            ),
        ] {
            let slider = ui::Slider {
                bounds,
                vertical: true,
                value: value as f32,
                range: [0., maximum as f32],
                pointer: self.pointer_position,
                pressed: (pressed
                    && selected.is_some_and(|i| {
                        i >= 3 + self.entries.len() + index * 3
                            && i < 3 + self.entries.len() + index * 3 + 3
                    }))
                .then_some(self.pointer_position)
                .flatten(),
            };
            if index == 1 {
                // 431ba0→4326d0 keeps the selected information list's
                // paint extent positive even when its scroll offset is 0.
                // Native 432a80 then gives the short list a half-track blue
                // handle. The actual scroll limit remains `maximum` above.
                let rows = self.info_rows();
                let extent = if rows > 0 {
                    rows.max(self.info_layout.visible_rows()) - self.info_layout.visible_rows() + 1
                } else {
                    0
                };
                slider.paint_with_extent(
                    &mut slider_shapes,
                    &mut controls,
                    &self.slider_textures,
                    extent as i32,
                );
            } else {
                slider.paint(&mut slider_shapes, &mut controls, &self.slider_textures);
            }
        }
        for (i, part) in [23, 24, 22].iter().enumerate() {
            let b = self.bounds[*part];
            let disabled = !self.generic_button_enabled(i);
            ui::button(
                &mut shapes,
                b,
                if i == 0 { [80, 148, 34] } else { [175, 6, 6] },
                selected == Some(i),
                pressed && selected == Some(i),
                disabled,
            )?;
            let mut text = b;
            text[1] += (b[3] - b[1] - 14.) / 2.;
            ui::text(
                &mut fonts,
                &self.tab,
                &self.strings[[1435, 1439, 1434][i]],
                text,
                true,
                if disabled { 0xff777777 } else { 0xffffffff },
            )?;
        }
        if let Some(message) = &self.message {
            wrapped(
                &mut fonts,
                &self.tab,
                &ascii(message),
                [32., 438., 608., 458.],
                0,
            )?;
        }
        let mut surfaces = self.base.clone();
        crate::menu::modal_frame::set_alpha(&mut surfaces, panel_alpha);
        surfaces.push(shapes);
        surfaces.push(slider_shapes);
        surfaces.extend(controls);
        // Original 43e790 submits list/info slider children before the Load
        // parent headings; the y70 list arrow sits under the y71..94 title.
        surfaces.push(headings);
        surfaces.extend(fonts);
        surfaces.extend(list_fonts);
        surfaces.extend(title_fonts);
        surfaces.extend(description_fonts);
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
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
/// Native 494c90/494cb0 append the name and formatted modified time as two
/// list fields. The supplied 430 Load frame places the cyan field to the
/// right of the name; the same original panel family uses these widths in
/// the Save list. Selection highlights the entire row behind both fields.
fn row_columns([left, top, right, bottom]: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    (
        [left + 3., top, left + 151., bottom],
        [left + 155., top, right, bottom],
    )
}
pub(crate) fn wrapped(
    pages: &mut [Surface],
    tab: &[u8],
    text: &[u8],
    b: [f32; 4],
    first: usize,
) -> Result<(), String> {
    for (i, line) in wrap_lines(tab, text, b[2] - b[0])?
        .iter()
        .skip(first)
        .enumerate()
    {
        let y = b[1] + i as f32 * 14.;
        if y + 14. > b[3] {
            break;
        }
        ui::text(pages, tab, line, [b[0], y, b[2], b[3]], false, 0xffffffff)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "load_list_order_tests.rs"]
mod load_list_order_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn register_text(
        catalog: &mut openmt2001::level_catalog::Catalog,
        id: i32,
        labels: openmt2001::level_catalog::Labels,
    ) {
        catalog
            .register(
                openmt2001::level_catalog::Registration {
                    id,
                    directory: b"CUSTOM/Example".to_vec(),
                    labels,
                    checksum: 0,
                },
                false,
                true,
            )
            .unwrap();
    }

    #[test]
    fn registered_literal_text_retains_first_id_and_empty_description() {
        use openmt2001::level_catalog::{Catalog, Labels};
        let mut catalog = Catalog::default();
        register_text(
            &mut catalog,
            500000,
            Labels::Literal {
                title: b"Custom title".to_vec(),
                description: b"Custom description".to_vec(),
            },
        );
        register_text(
            &mut catalog,
            500000,
            Labels::Literal {
                title: b"Duplicate".to_vec(),
                description: vec![],
            },
        );
        register_text(
            &mut catalog,
            500001,
            Labels::Literal {
                title: vec![0x80],
                description: vec![],
            },
        );
        let text = LevelTextCatalog::new(&catalog, &[]);
        drop(catalog); // Panels own the text, independent of catalog lifetime.
        assert_eq!(text.title(500000, &[]), ascii("Custom title"));
        assert_eq!(
            text.description(500000, &[]),
            Some(ascii("Custom description").as_slice())
        );
        assert_eq!(text.title(500001, &[]), vec![31]);
        assert_eq!(text.description(500001, &[]), None);
    }

    #[test]
    fn localized_text_preserves_language_glyphs_and_registered_empty_values() {
        use openmt2001::level_catalog::{Catalog, Labels};
        let mut catalog = Catalog::default();
        register_text(
            &mut catalog,
            500000,
            Labels::Localized {
                title: 0,
                description: 1,
            },
        );
        register_text(
            &mut catalog,
            200001,
            Labels::Localized {
                title: 2,
                description: 2,
            },
        );
        let strings = vec![vec![128, 201, 0], vec![199, 42], vec![]];
        let text = LevelTextCatalog::new(&catalog, &strings);
        assert_eq!(text.title(500000, &strings), strings[0]);
        assert_eq!(
            text.description(500000, &strings),
            Some(strings[1].as_slice())
        );
        assert!(text.title(200001, &strings).is_empty());
        assert_eq!(text.description(200001, &strings), None);
    }

    #[test]
    fn missing_registration_or_language_rows_use_safe_builtin_fallbacks() {
        use openmt2001::level_catalog::{Catalog, Labels};
        let mut strings = vec![vec![]; 1958];
        strings[1956] = vec![81];
        strings[1957] = vec![82];
        let mut catalog = Catalog::default();
        register_text(
            &mut catalog,
            200001,
            Labels::Localized {
                title: u32::MAX,
                description: u32::MAX,
            },
        );
        for text in [
            LevelTextCatalog::default(),
            LevelTextCatalog::new(&catalog, &strings),
        ] {
            assert_eq!(text.title(200001, &strings), vec![81]);
            assert_eq!(text.description(200001, &strings), Some([82].as_slice()));
            for id in [0, 500099, u32::MAX] {
                assert_eq!(text.title(id, &[]), ascii(&format!("Level {id}")));
                assert_eq!(text.description(id, &[]), None);
            }
            assert_eq!(text.description(200001, &[]), None);
        }
    }

    #[test]
    #[ignore = "requires original v1.4 executable and supplied original save"]
    fn original_load_delete_confirmation_and_profile_scope() {
        let exe = fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        // 4950f0 event12 index2 opens the generic delete question; index0
        // accepts through 494f30, while a nonzero answer returns untouched.
        assert_eq!(
            image.at(0x4951b3, 9).unwrap(),
            &[0x6a, 0, 0x68, 0x5e, 0x06, 0, 0, 0x6a, 0]
        );
        assert_eq!(image.at(0x494f4c, 2).unwrap(), &[0x85, 0xc0]);
        assert_eq!(image.at(0x542b24, 14).unwrap(), b"%d:%02d%s %s\0\0");
        for call in [0x4945f3u32, 0x494710] {
            let bytes = image.at(call, 5).unwrap();
            assert_eq!(bytes[0], 0xe8);
            let displacement = i32::from_le_bytes(bytes[1..5].try_into().unwrap());
            assert_eq!(i64::from(call) + 5 + i64::from(displacement), 0x43f510);
        }
        let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let (_, bounds) = ui::panel_base(
            &index,
            &archive,
            "gamedata2\\brand_new_panels\\load_menu_V2.tm",
            &[0, 1, 2, 3, 4, 5, 7],
        )
        .unwrap();
        let native_slider = |parent: [f32; 4]| {
            let x = (parent[2] - 16.).trunc();
            let y = parent[1].trunc();
            let height = (parent[3] - parent[1]).trunc();
            [x, y, x + 16., y + height]
        };
        assert_eq!(native_slider(bounds[7]), LIST_SCROLL);
        assert_eq!(native_slider(bounds[21]), INFO_SCROLL);
        // v1.4 4321e0→432a80, recorded by native_load_info_slider.py for
        // the original Load information track with paint extent one. Its
        // input offset may still be clamped to zero by the list itself.
        let info_track = ui::slider_track(INFO_SCROLL, true);
        assert_eq!(
            ui::slider_handle(info_track, 0, 1, true),
            [593., 305., 599., 332.5]
        );
        assert_eq!(
            ui::slider_handle(info_track, 1, 1, true),
            [593., 332.5, 599., 360.]
        );
        assert_eq!(ui::slider_background(info_track, true, true).1, 0xcc183088);
        let original = openmt2001::fixtures::path("SAVE_ONE");
        let original_bytes = fs::read(&original).unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("omt-v2-load-delete-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let slot = crate::save_menu::write_target(
            &root,
            &crate::save_menu::Target::NewName("V2 Delete Gate".into()),
            &original_bytes,
        )
        .unwrap();
        let mut load = Load {
            base: Vec::new(),
            info_layout: openmt2001::frontend_list::Layout::new([338., 287., 588., 379.], 12., 3.)
                .unwrap(),
            bounds: vec![[0.; 4]; 25],
            fonts: Vec::new(),
            tab: Vec::new(),
            list_fonts: Vec::new(),
            list_tab: Vec::new(),
            list_layout: openmt2001::frontend_list::Layout::new([20., 40., 284., 302.], 11., 3.)
                .unwrap(),
            description_fonts: Vec::new(),
            description_tab: Vec::new(),
            slider_textures: Vec::new(),
            title_fonts: Vec::new(),
            title_tab: Vec::new(),
            level_text: LevelTextCatalog::default(),
            strings: Vec::new(),
            directories: vec![root.join("saves"), original.parent().unwrap().to_path_buf()],
            app_root: root.clone(),
            import_directory: original.parent().unwrap().to_path_buf(),
            campaign_filename: None,
            profile_root: root.clone(),
            entries: Vec::new(),
            chosen: None,
            first: 0,
            description_first: 0,
            over_description: false,
            pointer_position: None,
            visible: false,
            busy: false,
            request: None,
            message: None,
            delete_confirmation: None,
            refusal_messages: refusal::Messages::default(),
            refusal_prompt: None,
        };
        load.open();
        assert_eq!(
            load.chosen, None,
            "native 494cec clears the initial selection"
        );
        for part in [22, 23, 24] {
            load.bounds[part] = bounds[part];
        }
        let center =
            |[left, top, right, bottom]: [f32; 4]| [(left + right) * 0.5, (top + bottom) * 0.5];
        assert_eq!(
            load.hit(center(bounds[23])),
            None,
            "disabled Load button must not admit a pointer event"
        );
        load.activate(0); // keyboard selection also cannot invoke disabled Load
        assert!(load.request.is_none() && !load.busy);
        let listed = load.entries.len();
        assert!(listed >= 2);
        let original_row = load
            .entries
            .iter()
            .position(|entry| entry.path == original)
            .unwrap();
        assert_eq!(
            load.entries[original_row].modified,
            fs::metadata(&original).unwrap().modified().ok()
        );
        load.chosen = Some(original_row);
        assert_eq!(
            load.hit(center(bounds[22])),
            None,
            "original-import Delete is a disabled generic button"
        );
        load.activate(2);
        assert!(load.delete_confirmation.is_none());
        assert!(
            load.message.is_none(),
            "keyboard activation cannot bypass disabled Delete"
        );
        let writable_row = load
            .entries
            .iter()
            .position(|entry| entry.writable_id.as_deref() == Some("V2 Delete Gate.sav"))
            .unwrap();
        assert_eq!(
            load.entries[writable_row].modified,
            fs::metadata(&slot).unwrap().modified().ok()
        );
        for row in [original_row, writable_row] {
            assert!(
                crate::save_menu::format_modified_local(load.entries[row].modified)
                    .is_some_and(|text| text.contains(':') && text.contains(' '))
            );
        }
        let (name, time) = row_columns([20., 40., 300., 54.]);
        assert_eq!(name, [23., 40., 171., 54.]);
        assert_eq!(time, [175., 40., 300., 54.]);
        assert!(name[2] < time[0] && time[2] == 300.);
        let (next_name, next_time) = row_columns([20., 54., 300., 68.]);
        assert_eq!(next_name, [23., 54., 171., 68.]);
        assert_eq!(next_time, [175., 54., 300., 68.]);
        for i in 0..30 {
            load.entries.push(Entry {
                path: root.join(format!("synthetic{i}.sav")),
                name: ascii(&format!("synthetic{i}")),
                metadata: Err("input-only row".into()),
                modified: None,
                writable_id: None,
            });
        }
        load.bounds[8] = bounds[8];
        let list = bounds[8];
        let mut viewport = list;
        viewport[3] -= 25.;
        viewport[2] -= 16.;
        load.list_layout = openmt2001::frontend_list::Layout::new(viewport, 11., 3.).unwrap();
        let unpainted_strip = [(list[0] + list[2]) * 0.5, viewport[3] + 1.];
        assert!(unpainted_strip[1] < list[3]);
        assert_eq!(
            load.hit(unpainted_strip),
            None,
            "native list viewport rejects the lower strip"
        );
        load.pointer(unpainted_strip);
        if let Some(index) = load.hit(unpainted_strip) {
            load.reveal(index);
        }
        assert_eq!(
            load.first, 0,
            "hover below the painted rows must not auto-scroll"
        );
        let down = center(ui::slider_arrows(LIST_SCROLL, true)[1]);
        load.pointer(down);
        load.activate(load.hit(down).unwrap());
        assert_eq!(load.first, 1, "list bottom arrow advances one row");
        let up = center(ui::slider_arrows(LIST_SCROLL, true)[0]);
        load.pointer(up);
        load.activate(load.hit(up).unwrap());
        assert_eq!(load.first, 0, "list top arrow reverses one row");
        load.entries.truncate(listed);
        load.chosen = Some(writable_row);
        load.activate(2);
        assert_eq!(load.count(), 2);
        let dialog = load.generic_prompt().unwrap();
        assert_eq!(
            (
                dialog.title,
                dialog.body,
                dialog.buttons,
                dialog.button_count
            ),
            (0x65e, 0x660, [0x664, 0x665], 2)
        );
        assert!(matches!(dialog.arguments[0],
            crate::human_dialog::Argument::Name(name)
                if name.starts_with(b"V2 Delete Gate")));
        assert_eq!(
            load.hit([260., 250.]),
            None,
            "only the shared generic prompt owns modal button hit testing"
        );
        load.activate(1); // No
        assert!(slot.exists());
        load.activate(2);
        load.back(); // Escape cancels the popup, not Load Game.
        assert!(load.visible && load.delete_confirmation.is_none() && slot.exists());
        load.activate(2);
        load.activate(0); // Yes; validated V2 slot is deleted and list refreshed.
        assert!(!slot.exists());
        assert!(load.visible && load.delete_confirmation.is_none());
        assert_eq!(load.chosen, None, "native list refresh clears selection");
        assert_eq!(load.entries.len(), listed - 1);
        assert_eq!(fs::read(&original).unwrap(), original_bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "requires original v1.4 inputs and an offscreen graphics device"]
    fn capture_original_load_scrollbars_and_delete_prompt() {
        use openmt2001::{
            flyby::CameraSample,
            render_scene::{self, OriginalAssets, Request, Source},
        };
        let read = |name| fs::read(openmt2001::fixtures::path(name)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
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
        let prompt_assets = crate::prompt::Assets::load(
            &index,
            &archive,
            &installer,
            &rows,
            &openmt2001::pe::PeImage::parse(&fs::read(&executable).unwrap()).unwrap(),
        )
        .unwrap();
        let stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = openmt2001::fixtures::scratch("LOAD_CAPTURE").join(format!(
            "omt-v2-load-capture-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let original = openmt2001::fixtures::path("SAVE_ONE");
        let mut load = Load::new(
            &index,
            &archive,
            &installer,
            &refs,
            original.parent().unwrap().to_path_buf(),
            root.clone(),
            &openmt2001::pe::PeImage::parse(&fs::read(&executable).unwrap()).unwrap(),
        )
        .unwrap();
        let selected = crate::save_menu::selected_profile_root(&root, b"Load capture");
        crate::save_menu::write_new(&selected, &fs::read(&original).unwrap()).unwrap();
        load.select_profile(Some(b"Load capture"));
        load.open();
        assert!(load.entries.len() >= 2);
        for (i, entry) in load.entries.iter_mut().enumerate() {
            entry.modified = Some(
                std::time::UNIX_EPOCH
                    + std::time::Duration::from_secs(1_782_073_260 + i as u64 * 120),
            );
        }
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
        let panel = load.draw(&renderer, None, false, 255).unwrap();
        crate::capture_test::capture_scene_overlays(
            &renderer,
            &scene,
            &[&panel],
            "work/frontend/load-open-unselected-city.png",
        );
        load.chosen = load.entries.iter().position(|entry| entry.path == original);
        let panel = load.draw(&renderer, None, false, 255).unwrap();
        crate::capture_test::capture_scene_overlays(
            &renderer,
            &scene,
            &[&panel],
            "work/frontend/load-import-selected-city.png",
        );
        crate::save_menu::write_target(
            &selected,
            &crate::save_menu::Target::NewName("V2 Delete Capture".into()),
            &read("OPENMT2001_SAVE_ONE"),
        )
        .unwrap();
        load.open();
        load.chosen = load
            .entries
            .iter()
            .position(|entry| entry.writable_id.as_deref() == Some("V2 Delete Capture.sav"));
        assert!(load.chosen.is_some());
        load.activate(2);
        assert!(load.delete_confirmation.is_some());
        let panel = load.draw(&renderer, None, false, 125).unwrap();
        let prompt = prompt_assets
            .prepare_dialog(&renderer, load.generic_prompt().unwrap(), [-1.; 2], None)
            .unwrap();
        let yes = prompt_assets.dialog_buttons(2)[0];
        assert!(yes[2] > yes[0] && yes[3] > yes[1]);
        crate::capture_test::capture_scene_overlays(
            &renderer,
            &scene,
            &[&panel, &prompt],
            "work/frontend/load-v2-delete-prompt-city.png",
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "load_metadata_tests.rs"]
mod load_metadata_tests;
