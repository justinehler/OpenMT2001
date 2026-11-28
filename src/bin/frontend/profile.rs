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
    fn clear_orphan_progress(&self, name: &str) -> Result<(), String> {
        let root = self.path.parent().ok_or("profile directory")?;
        let path = crate::campaign_store::profile_path(root, name.as_bytes());
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    fn clear_orphan_options(&self, name: &str) -> Result<(), String> {
        let root = self.path.parent().ok_or("profile directory")?;
        crate::menu::preferred_token::clear(root, name.as_bytes())?;
        let path = crate::profile_settings::profile_path(root, name.as_bytes());
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    fn clear_orphan_saves(&self, name: &str) -> Result<(), String> {
        let root = self.path.parent().ok_or("profile directory")?;
        crate::save_menu::clear_profile_saves(root, name.as_bytes())
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
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 31 && name.bytes().all(|c| (32..127).contains(&c))
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
    labels: Vec<Vec<u8>>,
    slider_textures: Vec<Surface>,
}
pub struct Profile {
    assets: Assets,
    pub store: Profiles,
    pub visible: bool,
    pub name: Option<String>,
    chosen: Option<usize>,
    first: usize,
    error: Option<String>,
    deleting: bool,
    selected_all: bool,
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
        let chosen = store.current;
        Ok(Self {
            assets: Assets {
                base,
                modal: vec![border],
                bounds: regions.into_iter().map(|r| r.bounds).collect(),
                confirm,
                fonts,
                tab,
                labels,
                slider_textures,
            },
            store,
            visible: false,
            name: None,
            chosen,
            first: 0,
            error: None,
            deleting: false,
            selected_all: false,
            pointer_position: None,
        })
    }
    pub(crate) fn modal_open(&self) -> bool {
        self.name.is_some() || self.deleting
    }
    pub fn count(&self) -> usize {
        if self.name.is_some() || self.deleting {
            2
        } else {
            4 + self.store.names.len() + if self.store.names.len() > 6 { 3 } else { 0 }
        }
    }
    /// The four LOGIN_SELECT action children are generic 43ed20 buttons.
    /// Selected-player actions carry the same disabled admission as their art.
    pub fn generic_button_enabled(&self, i: usize) -> bool {
        action_button_enabled(
            self.chosen.is_some(),
            self.name.is_some() || self.deleting,
            i,
        )
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        let hit = |[l, t, r, b]: [f32; 4]| p[0] >= l && p[0] <= r && p[1] >= t && p[1] <= b;
        if self.name.is_some() || self.deleting {
            return [4, 6].iter().position(|&i| hit(self.assets.confirm[i]));
        }
        for (i, part) in [11, 10, 13, 12].iter().enumerate() {
            if self.generic_button_enabled(i) && hit(self.assets.bounds[*part]) {
                return Some(i);
            }
        }
        if self.store.names.len() > 6 {
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
        let [l, t, r, b] = self.assets.bounds[5];
        if hit([l, t, r, b]) {
            let row = ((p[1] - t) / 14.) as usize + self.first;
            if row < self.store.names.len() && row < self.first + 6 {
                return Some(4 + row);
            }
        }
        None
    }
    pub fn back(&mut self) {
        self.error = None;
        if self.name.take().is_none() && !self.deleting {
            self.visible = false;
        }
        self.deleting = false;
    }
    pub fn select_all(&mut self) {
        self.selected_all = self.name.is_some();
    }
    pub fn input(&mut self, text: &str) {
        if let Some(name) = &mut self.name {
            if self.selected_all {
                name.clear();
                self.selected_all = false;
            }
            for c in text.chars() {
                if c.is_ascii() && !c.is_control() && name.len() < 31 {
                    name.push(c);
                }
            }
            self.error = None;
        }
    }
    pub fn erase(&mut self) {
        if let Some(name) = &mut self.name {
            if self.selected_all {
                name.clear();
                self.selected_all = false;
            } else {
                name.pop();
            }
            self.error = None;
        }
    }
    pub fn reveal(&mut self, index: usize) {
        if self.name.is_none() && !self.deleting && index >= 4 && index - 4 < self.store.names.len()
        {
            let row = index - 4;
            if row < self.first {
                self.first = row;
            } else if row >= self.first + 6 {
                self.first = row - 5;
            }
        }
    }
    pub fn scroll(&mut self, delta: i32) {
        if self.name.is_some() || self.deleting {
            return;
        }
        self.first = (self.first as i32 + delta)
            .clamp(0, self.store.names.len().saturating_sub(6) as i32)
            as usize;
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        self.pointer_position = Some(p);
    }
    pub fn drag_track(&mut self, origin: Option<usize>, p: [f32; 2]) -> bool {
        if self.name.is_some()
            || self.deleting
            || self.store.names.len() <= 6
            || origin != Some(4 + self.store.names.len() + 2)
        {
            return false;
        }
        let maximum = self.store.names.len().saturating_sub(6) as i32;
        let next = ui::slider_selection(ui::slider_track(PROFILE_SCROLL, true), p[1], maximum, true)
            as usize;
        let changed = next != self.first;
        self.first = next;
        changed
    }
    pub fn is_scroll_track(&self, index: Option<usize>) -> bool {
        self.store.names.len() > 6 && index == Some(4 + self.store.names.len() + 2)
    }
    pub fn activate(&mut self, index: usize) {
        self.error = None;
        let before = (self.store.names.clone(), self.store.current, self.chosen);
        let result = (|| -> Result<(), String> {
            if self.deleting {
                if index == 0
                    && let Some(i) = self.chosen
                {
                    self.store.delete_chosen(i)?;
                    self.chosen = None;
                }
                self.deleting = false;
                return Ok(());
            }
            if let Some(name) = &self.name {
                if index == 1 {
                    self.name = None;
                    return Ok(());
                }
                if !valid_name(name) {
                    return Err("Enter a name (1-31 ASCII characters)".into());
                }
                if self.store.names.contains(name) {
                    return Err("This player name already exists".into());
                }
                self.store.clear_orphan_progress(name)?;
                self.store.clear_orphan_options(name)?;
                self.store.clear_orphan_saves(name)?;
                self.store.names.push(name.clone());
                self.chosen = Some(self.store.names.len() - 1);
                self.store.current = self.chosen;
                self.store.save()?;
                self.name = None;
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
                            let maximum = self.store.names.len().saturating_sub(6) as i32;
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
                1 => self.deleting = self.chosen.is_some(),
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
        for (i, name) in self.store.names.iter().enumerate().skip(self.first).take(6) {
            let y = t + (i - self.first) as f32 * 14.;
            if self.chosen == Some(i) {
                shapes.rect([l, y, r, y + 14.], [0xff008ed0; 4]);
            }
            let encoded: Vec<_> = name.bytes().map(|c| c - 32).collect();
            ui::text(
                &mut fonts,
                &self.assets.tab,
                &encoded,
                [l + 3., y, r, b],
                false,
                0xffffffff,
            )?;
        }
        let scroll_base = 4 + self.store.names.len();
        ui::Slider {
            bounds: PROFILE_SCROLL,
            vertical: true,
            value: self.first as f32,
            range: [0., self.store.names.len().saturating_sub(6) as f32],
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
                || ((self.name.is_some() || self.deleting) && i != 0);
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
        let modal_open = self.name.is_some() || self.deleting;
        if modal_open {
            modal.rect(self.assets.confirm[1], [0xaa000000; 4]);
            modal.rect(
                self.assets.confirm[2],
                [0xff909090, 0xff909090, 0x80909090, 0x80909090],
            );
            let mut title = self.assets.confirm[2];
            title[1] += 4.;
            let delete_title: Vec<_> = "Delete player?".bytes().map(|c| c - 32).collect();
            ui::text(
                &mut modal_fonts,
                &self.assets.tab,
                if self.deleting {
                    &delete_title
                } else {
                    &self.assets.labels[5]
                },
                title,
                true,
                0xffffffff,
            )?;
            let mut input = self.assets.confirm[3];
            input[1] += 10.;
            input[3] -= 10.;
            modal.rect(
                input,
                [if self.selected_all {
                    0xff0070c0
                } else if self.name.is_some() {
                    0xff002896 // Registration painter 495e5e; deletion has a separate dialog.
                } else {
                    0xff001a90
                }; 4],
            );
            let name = self.name.as_deref().unwrap_or_else(|| {
                self.chosen
                    .map(|i| self.store.names[i].as_str())
                    .unwrap_or("")
            });
            let encoded: Vec<_> = name.bytes().map(|c| c - 32).collect();
            if self.name.is_some() {
                // Registration configures style 4 on the full CONFIRM_TEXT
                // region (495c89..495cb2), not the inset blue backing.
                ui::text_entry(
                    &mut modal_fonts,
                    &mut modal,
                    &self.assets.tab,
                    &encoded,
                    self.assets.confirm[3],
                    true,
                    true,
                )?;
            } else {
                input[1] += 5.;
                ui::text(
                    &mut modal_fonts,
                    &self.assets.tab,
                    &encoded,
                    input,
                    true,
                    0xffffffff,
                )?;
            }
            for (i, part) in [4, 6].iter().enumerate() {
                let bounds = self.assets.confirm[*part];
                ui::button(
                    &mut modal,
                    bounds,
                    if i == 0 { [80, 148, 34] } else { [175, 6, 6] },
                    selected == Some(i),
                    pressed && selected == Some(i),
                    i == 0 && self.name.as_ref().is_some_and(|n| !valid_name(n)),
                )?;
                let mut text = bounds;
                text[1] += 5.;
                ui::text(
                    &mut modal_fonts,
                    &self.assets.tab,
                    &self.assets.labels[if i == 0 { 2 } else { 3 }],
                    text,
                    true,
                    0xffffffff,
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
        let backdrop = crate::menu::modal_frame::Backdrop::new(&self.assets.base[..5], panel_alpha);
        let mut draws = backdrop.draws();
        draws.extend(self.assets.base[5..].iter().map(Surface::draw));
        draws.push(shapes.draw());
        draws.push(slider_shapes.draw());
        draws.extend(slider_controls.iter().map(Surface::draw));
        draws.extend(fonts.iter().map(Surface::draw));
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
        let mut profile = Profile {
            assets: Assets {
                base: vec![],
                modal: vec![],
                bounds,
                confirm: vec![],
                fonts: vec![],
                tab: vec![],
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
            chosen: None,
            first: 0,
            error: None,
            deleting: false,
            selected_all: false,
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
        assert!(profile.first >= 1 && profile.first <= 3);
        let before_up = profile.first;
        profile.activate(base);
        assert_eq!(profile.first, before_up - 1);
        assert!(profile.drag_track(Some(base + 2), [448., 999.]));
        assert_eq!(profile.first, 3);
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
            "work/frontend/profile-scrollbar-city.png",
        );
    }
}
