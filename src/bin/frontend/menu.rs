//! Native menu presentation assembled from original panel, glyph and logo bytes.
//! 4954a0 supplies seven regions/colors; 495730 places logos. Text alignment
//! follows the supplied main-menu recording. Destination screens remain explicit.
//! Textures currently use individual images rather than original packed pages;
//! bitmap metrics are native-checked, but line rasterization is a GPU approximation.
//! Pending/quit prompts are development UI, not reconstructed original dialogs.
use crate::ui::Surface;
#[path = "preferred_token.rs"]
pub mod preferred_token;
#[path = "skirmish_route.rs"]
pub mod skirmish_route;
use openmt2001::{
    archive::ArchiveIndex,
    font::{Font, Layout},
    mesh::CachedMesh,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
    ui_button::{self, Primitive, State},
    ui_mesh,
};
use std::{fs, path::Path};

pub struct Assets {
    skirmish: Option<skirmish_route::Route>,
    settings: crate::settings_menu::Assets,
    planning: crate::build_view::Assets,
    prompts: crate::prompt::Assets,
    event_list: crate::event_list::Assets,
    stats: crate::stats_panel::Assets,
    cards: crate::card_panel::Assets,
    business: crate::business_view::Assets,
    auction_notice: crate::auction_notice::Assets,
    auction_panel: crate::auction_panel::Assets,
    notifications: crate::notice_panel::Assets,
    comparison: crate::comparison::Assets,
    locator: crate::locator::Assets,
    adjustment: crate::business_adjustment::Assets,
    profile: Option<crate::profile::Profile>,
    catalog: openmt2001::level_catalog::Catalog,
    profile_root: std::path::PathBuf,
    load: Option<crate::load::Load>,
    single: Option<crate::singleplayer::SinglePlayer>,
    base: Vec<Surface>,
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    labels: Vec<Vec<u8>>,
    mode_labels: Vec<Vec<u8>>,
    language: Vec<Vec<u8>>,
    pub bounds: Vec<[f32; 4]>,
}
impl Assets {
    #[allow(clippy::too_many_arguments)] // Independent original-input roots and V2 output roots.
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
        scripts: &Path,
        profile_root: std::path::PathBuf,
        save_root: std::path::PathBuf,
        executable: &[u8],
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("missing menu asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("menu asset range".into())
        };
        let mesh = CachedMesh::parse(payload("gamedata2\\brand_new_panels\\main_menu.tm")?)?;
        let regions = ui_mesh::regions(&mesh, 0)?;
        let pos = mesh.positions(0)?;
        let uv = mesh.uvs();
        let mut base = vec![];
        for part in mesh.parts.iter().take(4) {
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
        for (part, texture) in [(4, 273), (5, 274), (13, 287)] {
            let mut s = Surface::new(generic.textures.get(texture).ok_or("missing menu logo")?);
            let [l, t, r, b] = regions[part].bounds;
            s.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            base.push(s);
        }
        let fonts = TextureSet::parse(payload("gamedata2\\font640\\textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[8..10]
            .iter()
            .map(Surface::new)
            .collect();
        let tab = fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
            .map_err(|e| e.to_string())?;
        let off = fs::read(language.join("tycoon.off")).map_err(|e| e.to_string())?;
        let bin = fs::read(language.join("tycoon.bin")).map_err(|e| e.to_string())?;
        if !off.len().is_multiple_of(2) {
            return Err("incomplete language offsets".into());
        }
        let mut strings = vec![];
        let mut offset = 0;
        for n in off.chunks_exact(2) {
            let end = offset + u16::from_le_bytes(n.try_into().unwrap()) as usize;
            strings.push(bin.get(offset..end).ok_or("language length")?);
            offset = end;
        }
        if offset != bin.len() {
            return Err("language lengths do not cover text data".into());
        }
        let localized = crate::tooltip::localized_strings(installer, &strings)?;
        let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
        for block in
            openmt2001::board::Board::from_v14(&openmt2001::pe::PeImage::parse(executable)?)?.blocks
        {
            strings
                .get(block.text_id as usize)
                .ok_or("original category name leaves language table")?;
        }
        let profile = Some(crate::profile::Profile::load(
            index,
            archive,
            installer,
            &strings,
            profile_root.clone(),
        )?);
        let catalog = crate::campaign_store::load_catalog(
            &openmt2001::pe::PeImage::parse(executable)?, scripts,
        )?;
        let load = crate::load::Load::new(
            index, archive, installer, &strings, save_root, profile_root.clone(),
        )?;
        let load = Some(load);
        let labels = [0x63d, 0x63e, 0x63f, 0x640, 0x641, 0x68f, 0x643]
            .iter()
            .map(|&i| {
                strings
                    .get(i)
                    .map(|s| s.to_vec())
                    .ok_or("menu language ID".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let single = Some(crate::singleplayer::SinglePlayer::new(
            index, archive, installer, &strings, executable,
        )?);
        let mode_labels = [2239, 2226, 2227, 2240, 1439]
            .iter()
            .map(|&i| {
                strings
                    .get(i)
                    .map(|s| s.to_vec())
                    .ok_or("mode label missing".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bounds = (6..13).map(|i| regions[i].bounds).collect();
        let locator =
            crate::locator::Assets::load(index, archive, installer, &strings, executable)?;
        let adjustment = crate::business_adjustment::Assets::load(
            index, archive, installer, &strings, executable,
        )?;
        let comparison =
            crate::comparison::Assets::load(index, archive, installer, &strings, executable)?;
        Ok(Self {
            skirmish: Some(skirmish_route::Route::new(
                index, archive, installer, scripts, &strings, executable,
            )?),
            settings: crate::settings_menu::Assets::load(index, archive, installer, language)?,
            planning: crate::build_view::Assets::load(
                index, archive, installer, &strings, executable,
            )?,
            prompts: crate::prompt::Assets::load(index, archive, installer, &strings)?,
            event_list: crate::event_list::Assets::load(index, archive, installer, &strings)?,
            stats: crate::stats_panel::Assets::load(index, archive, installer, &strings, executable,
            )?,
            cards: crate::card_panel::Assets::load(index, archive, installer, &strings)?,
            business: crate::business_view::Assets::load(
                index, archive, installer, &strings, executable,
            )?,
            auction_notice: crate::auction_notice::Assets::load(
                index, archive, installer, &strings, executable,
            )?,
            auction_panel: crate::auction_panel::Assets::load(
                index, archive, installer, &strings, executable,
            )?,
            notifications: crate::notice_panel::Assets::load(
                index, archive, installer, &strings, executable,
            )?,
            comparison,
            locator,
            adjustment,
            profile,
            catalog,
            profile_root,
            load,
            base,
            fonts,
            tab,
            labels,
            mode_labels,
            language: strings.iter().map(|s| s.to_vec()).collect(),
            single,
            bounds,
        })
    }
}
pub struct Menu {
    pub skirmish: skirmish_route::Route,

    pub settings: crate::settings_menu::View,
    pub settings_audio: crate::settings_menu::Levels,
    profile: crate::profile::Profile,
    pub load: crate::load::Load,
    pub single: crate::singleplayer::SinglePlayer,
    assets: Assets,
    prepared: Option<PreparedScene>,
    pub selected: Option<usize>,
    pub pressed: bool,
    pressed_origin: Option<usize>,
    light_click_target: Option<(u8, usize)>,
    pointer_position: Option<[f32; 2]>,
    pub pending: Option<usize>,
    dirty: bool,
    single_mode: bool,
}
#[derive(Clone, Copy)]
enum MenuCuePage {
    Main,
    Load,
    LoadPrompt,
    Profile,
    Single,
    Other,
}
fn generic_cue_target(
    page: MenuCuePage,
    selected: Option<usize>,
    held: bool,
    pending: bool,
    enabled: bool,
) -> Option<(u8, usize)> {
    if !held || pending {
        return None;
    }
    match page {
        MenuCuePage::Main => selected.filter(|&i| i < 7).map(|i| (0, i)),
        MenuCuePage::Load => selected.filter(|&i| i < 3 && enabled).map(|i| (1, i)),
        MenuCuePage::LoadPrompt => selected.filter(|&i| i < 2).map(|i| (2, i)),
        MenuCuePage::Profile => selected.filter(|&i| i < 4 && enabled).map(|i| (3, i)),
        MenuCuePage::Single => selected.filter(|&i| i < 4 && enabled).map(|i| (4, i)),
        MenuCuePage::Other => None,
    }
}
fn held_entry_click(last: &mut Option<(u8, usize)>, target: Option<(u8, usize)>) -> bool {
    let play = target.is_some() && target != *last;
    *last = target;
    play
}
fn release_activates(held: bool, origin: Option<usize>, selected: Option<usize>) -> bool {
    held && origin == selected && selected.is_some()
}
fn release_activates_without_track(held: bool, origin: Option<usize>, selected: Option<usize>,
    retained_track: bool,
) -> bool {
    release_activates(held, origin, selected) && !retained_track
}
fn main_button_state(held: bool, origin: Option<usize>, selected: Option<usize>, i: usize,
) -> State {
    let hover = selected == Some(i);
    State {
        // 443360 event5 held entry sets +0x330, but only event4 sets +0x334.
        flag_330: held && hover,
        flag_334: held && origin == Some(i) && hover,
        flag_335: hover,
        flag_333: false,
    }
}
impl Menu {
    pub fn cancel_input(&mut self) {
        self.settings.cancel_input();
        self.skirmish.cancel_input();
        self.pressed = false;
        self.pressed_origin = None;
        self.light_click_target = None;
    }
    pub fn settings_snapshot(&self) -> crate::profile_settings::Snapshot {
        crate::profile_settings::Snapshot {
            audio: self.settings_audio,
            graphics: self.settings.graphics,
            gameplay: self.settings.gameplay,
        }
    }

    /// V2-owned Options record for the active player, keyed by the same
    /// reversible profile identity as campaign progress.
    pub fn current_settings_path(&self) -> Option<std::path::PathBuf> {
        let name = self.profile.store.names.get(self.profile.store.current?)?;
        Some(crate::profile_settings::profile_path(
            &self.assets.profile_root, name.as_bytes(),
        ))
    }

    pub fn current_save_root(&self) -> Option<std::path::PathBuf> {
        let name = self.profile.store.names.get(self.profile.store.current?)?;
        Some(crate::save_menu::selected_profile_root(
            &self.assets.profile_root, name.as_bytes(),
        ))
    }

    /// Reopen the selected V2 campaign record after a finished result scene.
    pub fn refresh_campaign(&mut self) -> Result<(), String> {
        self.single.reload_progress()?;
        self.dirty = true;
        Ok(())
    }
    pub fn planning_assets(&self) -> &crate::build_view::Assets {
        &self.assets.planning
    }
    pub fn event_list_assets(&self) -> &crate::event_list::Assets {
        &self.assets.event_list
    }
    pub fn stats_assets(&self) -> &crate::stats_panel::Assets {
        &self.assets.stats
    }
    pub fn prompt_assets(&self) -> &crate::prompt::Assets {
        &self.assets.prompts
    }
    pub fn card_assets(&self) -> &crate::card_panel::Assets {
        &self.assets.cards
    }
    pub fn auction_notice_assets(&self) -> &crate::auction_notice::Assets {
        &self.assets.auction_notice
    }
    pub fn auction_panel_assets(&self) -> &crate::auction_panel::Assets {
        &self.assets.auction_panel
    }
    pub fn notification_assets(&self) -> &crate::notice_panel::Assets {
        &self.assets.notifications
    }
    pub fn comparison_assets(&self) -> &crate::comparison::Assets {
        &self.assets.comparison
    }
    pub fn business_assets(&self) -> &crate::business_view::Assets {
        &self.assets.business
    }
    pub fn adjustment_assets(&self) -> &crate::business_adjustment::Assets {
        &self.assets.adjustment
    }
    pub fn locator_assets(&self) -> &crate::locator::Assets {
        &self.assets.locator
    }
    pub fn validate_inspection_names(
        &self,
        scene: &openmt2001::render_scene::Scene,
    ) -> Result<(), String> {
        for business in scene
            .inspection_buildings
            .iter()
            .filter_map(|b| b.business())
        {
            self.assets
                .language
                .get(business.name_text_id as usize)
                .ok_or("saved business name leaves original language table")?;
        }
        Ok(())
    }
    pub fn new(mut assets: Assets) -> Self {
        Self {
            skirmish: assets.skirmish.take().unwrap(),

            settings: Default::default(),
            settings_audio: Default::default(),
            profile: assets.profile.take().unwrap(),
            load: assets.load.take().unwrap(),
            single: assets.single.take().unwrap(),
            assets,
            prepared: None,
            selected: None,
            pressed: false,
            pressed_origin: None,
            light_click_target: None,
            pointer_position: None,
            pending: None,
            dirty: true,
            single_mode: false,
        }
    }
    pub fn select(&mut self, selected: Option<usize>, pressed: bool) {
        if self.load.visible
            && let Some(i) = selected
        {
            self.load.reveal(i);
        }
        if self.profile.visible
            && let Some(index) = selected
        {
            self.profile.reveal(index);
        }
        if self.selected != selected || self.pressed != pressed {
            self.selected = selected;
            self.pressed = pressed;
            self.dirty = true;
        }
    }
    pub fn pointer(&mut self, p: [f32; 2]) {
        self.pointer_position = Some(p);
        if self.skirmish.error.is_none() && self.skirmish.visible() { self.skirmish.pointer(p); }
        if self.settings.page.is_some() {
            self.settings.drag(&self.assets.settings, Some(p), &mut self.settings_audio);
            return;
        }
        self.load.pointer(p);
        self.profile.pointer(p);
        self.single.pointer(p);
        if self.profile.visible && self.pressed
            && self.profile.drag_track(self.pressed_origin, p) {
            self.dirty = true;
        }
        if self.single.visible && self.pressed
            && self.single.drag_track(self.pressed_origin, p) {
            self.dirty = true;
        }
        self.select(self.hit(p), self.pressed);
    }
    /// 443360 emits its light click on the first held entry and on a held
    /// leave/re-entry. The audio latch is separate from release activation.
    pub fn light_click_on_held_entry(&mut self) -> bool {
        if self.settings.page.is_some() {
            return self.settings.pointer_click_cue(&self.assets.settings, self.pointer_position);
        }
        let page = if self.load.visible {
            if self.load.delete_pending() { MenuCuePage::LoadPrompt }
            else { MenuCuePage::Load }
        } else if self.profile.visible {
            MenuCuePage::Profile
        } else if self.single.visible {
            MenuCuePage::Single
        } else if self.single_mode {
            MenuCuePage::Other
        } else {
            MenuCuePage::Main
        };
        let enabled = self.selected.is_some_and(|i| match page {
            MenuCuePage::Load => self.load.generic_button_enabled(i),
            MenuCuePage::Profile => self.profile.generic_button_enabled(i),
            MenuCuePage::Single => self.single.generic_click_enabled(i),
            _ => false,
        });
        let target = generic_cue_target(page, self.selected, self.pressed,
            self.pending.is_some(), enabled,
        );
        held_entry_click(&mut self.light_click_target, target)
    }
    pub fn hit(&self, p: [f32; 2]) -> Option<usize> {
        if self.skirmish.error.is_some() {
            let [l, t, r, b] = self.assets.prompts.button_at([0.; 2]);
            return (p[0] >= l && p[0] <= r && p[1] >= t && p[1] <= b).then_some(0);
        }
        if self.skirmish.visible() {
            return self.skirmish.hit(p);
        }
        if self.single.visible {
            return self.single.hit(p);
        }
        if self.load.visible {
            if self.load.delete_pending() {
                return self.assets.prompts.dialog_buttons(2).iter().position(|bounds| {
                    p[0] >= bounds[0] && p[0] <= bounds[2]
                        && p[1] >= bounds[1] && p[1] <= bounds[3]
                });
            }
            return self.load.hit(p);
        }
        if self.profile.visible {
            return self.profile.hit(p);
        }
        (if self.single_mode {
            &self.assets.bounds[1..6]
        } else {
            &self.assets.bounds[..]
        })
        .iter()
        .position(|&[l, t, r, b]| p[0] >= l && p[0] <= r && p[1] >= t && p[1] <= b)
    }
    pub fn mouse_button(&mut self, pressed: bool) {
        if self.skirmish.error.is_none() && self.skirmish.visible() {
            self.skirmish.mouse(self.pointer_position, pressed);
            self.pressed = pressed;
            self.dirty = true;
            return;
        }
        if self.settings.page.is_some() {
            self.settings.mouse(&self.assets.settings, self.pointer_position, pressed,
                &mut self.settings_audio,
            );
            return;
        }
        // A panel transition can change the hit target without a CursorMoved event.
        if pressed && let Some(point) = self.pointer_position {
            self.select(self.hit(point), false);
        }
        if pressed {
            self.pressed_origin = self.selected;
            if self.profile.visible
                && let Some(point) = self.pointer_position
                && self.profile.drag_track(self.pressed_origin, point) {
                self.dirty = true;
            }
            if self.single.visible
                && let Some(point) = self.pointer_position
                && self.single.drag_track(self.pressed_origin, point) {
                self.dirty = true;
            }
        } else if release_activates_without_track(self.pressed, self.pressed_origin, self.selected,
            self.profile.visible && self.profile.is_scroll_track(self.pressed_origin)
                || self.single.visible && self.single.is_scroll_track(self.pressed_origin),
        ) {
            self.activate();
        }
        self.select(self.selected, pressed);
    }
    pub fn activate(&mut self) {
        if self.skirmish.error.is_some() {
            self.skirmish.error = None;
            self.dirty = true;
            return;
        }
        if self.skirmish.visible() {
            self.skirmish.activate_keyboard();
            self.selected = None;
            self.dirty = true;
            return;
        }
        if self.settings.page.is_some() { return; }
        if self.single.visible {
            if let Some(i) = self.selected {
                let count = self.single.count();
                self.single.activate(i);
                if count != self.single.count() || !self.single.visible {
                    self.selected = None;
                }
                self.dirty = true;
            }
            return;
        }
        if self.single_mode {
            match self.selected {
                Some(4) => {
                    self.single_mode = false;
                    self.selected = None;
                }
                Some(1) => {
                    if let Some(current) = self.profile.store.current {
                        let name = self.profile.store.names[current].as_bytes();
                        let path =
                            crate::campaign_store::profile_path(&self.assets.profile_root, name);
                        let mut catalog = self.assets.catalog.clone();
                        let result = crate::campaign_store::load_progress(&mut catalog, &path)
                            .and_then(|()| {
                                self.skirmish.open(
                                    catalog,
                                    name,
                                    preferred_token::Token::load(&preferred_token::path(
                                        &self.assets.profile_root,
                                        name,
                                    ))?
                                    .index(),
                                    crate::save_menu::selected_profile_root(
                                        &self.assets.profile_root,
                                        name,
                                    ),
                                )
                            });
                        if let Err(error) = result {
                            self.skirmish.error = Some(error);
                        }
                    } else {
                        self.single_mode = false;
                        self.profile.visible = true;
                    }
                    self.selected = None;
                }
                Some(i @ (0 | 2)) => {
                    if let Some(current) = self.profile.store.current {
                        let name = &self.profile.store.names[current];
                        let path = crate::campaign_store::profile_path(
                            &self.assets.profile_root, name.as_bytes(),
                        );
                        let mut catalog = self.assets.catalog.clone();
                        match crate::campaign_store::load_progress(&mut catalog, &path) {
                            Ok(()) => self.single.open(i == 2, name, catalog, path),
                            Err(error) => self.single.message = Some(error),
                        }
                        self.selected = None;
                    } else {
                        self.single_mode = false;
                        self.profile.visible = true;
                        self.selected = None;
                    }
                }
                Some(_) => self.pending = Some(2),
                None => (),
            }
            self.dirty = true;
            return;
        }
        if self.selected == Some(1) && !self.profile.visible && !self.load.visible {
            if let Some(current) = self.profile.store.current {
                let name = &self.profile.store.names[current];
                let path = crate::campaign_store::profile_path(
                    &self.assets.profile_root, name.as_bytes());
                let mut catalog = self.assets.catalog.clone();
                match crate::campaign_store::load_progress(&mut catalog, &path) {
                    Ok(()) => self.single.open_tutorial(name, catalog, path),
                    Err(error) => self.single.message = Some(error),
                }
            } else {
                self.profile.visible = true;
            }
            self.selected = None;
            self.dirty = true;
            return;
        }
        if self.selected == Some(2) && !self.profile.visible && !self.load.visible {
            self.single_mode = true;
            self.selected = None;
            self.dirty = true;
            return;
        }

        if self.load.visible {
            if let Some(i) = self.selected {
                let count = self.load.count();
                self.load.activate(i);
                self.dirty = true;
                if count != self.load.count() || !self.load.visible {
                    self.selected = None;
                }
            }
            return;
        }
        if self.selected == Some(4) && !self.profile.visible {
            let name = self.profile.store.current
                .and_then(|current| self.profile.store.names.get(current))
                .map(String::as_bytes);
            self.load.select_profile(name);
            self.load.open();
            self.selected = None;
            self.dirty = true;
            return;
        }
        if self.profile.visible {
            if let Some(i) = self.selected.or(self.profile.name.as_ref().map(|_| 0)) {
                self.profile.activate(i);
                self.dirty = true;
                if !self.profile.visible {
                    self.selected = None;
                }
            }
            return;
        }
        if self.selected == Some(0) {
            self.profile.visible = true;
            self.selected = None;
            self.dirty = true;
            return;
        }
        if self.selected == Some(5) {
            self.settings.open();
            self.selected = None;
            self.pressed = false;
            self.pressed_origin = None;
            self.dirty = true;
            return;
        }
        if let Some(i) = self.selected {
            self.pending = Some(i);
            self.dirty = true;
        }
    }
    pub fn is_editing(&self) -> bool {
        self.skirmish.is_editing() || (self.profile.visible && self.profile.name.is_some())
    }
    pub fn navigate(&mut self, delta: i32) {
        if self.skirmish.error.is_some() { return; }
        if self.skirmish.visible() {
            self.skirmish.navigate(delta);
            self.dirty = true;
            return;
        }
        if self.settings.page.is_some() { return; }
        if self.pending.is_some() {
            return;
        }
        let count = if self.single.visible {
            self.single.count()
        } else if self.load.visible {
            self.load.count()
        } else if self.profile.visible {
            self.profile.count()
        } else if self.single_mode {
            5
        } else {
            7
        };
        let next = (self
            .selected
            .map_or(if delta > 0 { -1 } else { 0 }, |i| i as i32)
            + delta)
            .rem_euclid(count as i32) as usize;
        self.select(Some(next), false);
    }
    pub fn input(&mut self, text: &str) {
        if self.skirmish.error.is_some() {
            return;
        }
        if self.skirmish.visible() {
            self.skirmish.text(text);
            self.dirty = true;
            return;
        }
        if self.profile.visible {
            self.profile.input(text);
            self.dirty = true;
        }
    }
    pub fn select_all(&mut self) {
        if self.profile.visible {
            self.profile.select_all();
            self.dirty = true;
        }
    }
    pub fn erase(&mut self) {
        if self.skirmish.error.is_some() {
            return;
        }
        if self.skirmish.visible() {
            self.skirmish.erase();
            self.dirty = true;
            return;
        }
        if self.profile.visible {
            self.profile.erase();
            self.dirty = true;
        }
    }
    pub fn scroll(&mut self, delta: i32) {
        if self.skirmish.error.is_some() {
            return;
        }
        if self.skirmish.visible() {
            self.skirmish.scroll(self.pointer_position, delta);
            self.dirty = true;
            return;
        }
        if self.settings.page.is_some() { return; }
        if self.single.visible {
            self.single.scroll(delta);
            self.dirty = true;
            return;
        }
        if self.load.visible {
            self.load.scroll(delta);
            self.dirty = true;
        }
        if self.profile.visible {
            self.profile.scroll(delta);
            self.dirty = true;
        }
    }
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }
    pub fn back(&mut self) {
        if self.skirmish.error.is_some() {
            self.skirmish.error = None;
            self.dirty = true;
            return;
        }
        if self.skirmish.visible() {
            self.skirmish.back();
            self.dirty = true;
            return;
        }
        if self.settings.page.is_some() { self.settings.back(); return; }
        if self.single.visible {
            self.single.back();
            self.selected = None;
            self.dirty = true;
            return;
        }
        if self.single_mode && self.pending.is_none() {
            self.single_mode = false;
            self.selected = None;
            self.dirty = true;
            return;
        }
        if self.load.visible {
            self.load.back();
            self.selected = None;
            self.dirty = true;
            return;
        }
        if self.profile.visible {
            self.profile.back();
            self.selected = None;
            self.dirty = true;
            return;
        }
        self.pending = None;
        self.dirty = true;
    }
    pub fn draw(
        &mut self,
        renderer: &Renderer,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) -> Result<(), String> {
        if let Some(error) = &self.skirmish.error {
            let panel = self.assets.prompts.prepare_error(renderer, error)?;
            renderer.draw_overlay(color, depth, &panel);
            return Ok(());
        }
        if self.settings.page.is_some() {
            self.settings.update_levels(&self.assets.settings, renderer,
                self.pointer_position, self.settings_audio,
            )?;
            if let Some(panel) = &self.settings.prepared {
                renderer.draw_overlay(color, depth, panel);
            }
            return Ok(());
        }
        if self.skirmish.visible() {
            let panel = self
                .skirmish
                .draw(renderer, self.pointer_position, self.pressed)?;
            renderer.draw_overlay(color, depth, &panel);
            return Ok(());
        }
        if self.single.visible {
            if self.dirty {
                self.prepared = Some(self.single.draw(renderer, self.selected, self.pressed)?);
                self.dirty = false;
            }
            renderer.draw_overlay(color, depth, self.prepared.as_ref().unwrap());
            return Ok(());
        }
        if self.load.visible {
            if self.dirty {
                self.prepared = Some(self.load.draw(renderer, self.selected, self.pressed)?);
                self.dirty = false;
            }
            renderer.draw_overlay(color, depth, self.prepared.as_ref().unwrap());
            if let Some(dialog) = self.load.generic_prompt() {
                let pressed_button = (self.pressed && self.selected.is_some_and(|i| i < 2))
                    .then_some(self.selected).flatten();
                let panel = self.assets.prompts.prepare_dialog(renderer, dialog,
                    self.pointer_position.unwrap_or([-1.; 2]), pressed_button,
                )?;
                renderer.draw_overlay(color, depth, &panel);
            }
            return Ok(());
        }
        if self.profile.visible {
            if self.dirty {
                self.prepared = Some(self.profile.draw(renderer, self.selected, self.pressed)?);
                self.dirty = false;
            }
            renderer.draw_overlay(color, depth, self.prepared.as_ref().unwrap());
            return Ok(());
        }
        if self.dirty {
            let mut buttons = Surface::solid();
            let main_colors = [
                [192, 0, 0],
                [192, 135, 0],
                [192, 192, 0],
                [0, 192, 0],
                [0, 0, 192],
                [110, 4, 171],
                [185, 7, 187],
            ];
            let mode_colors = [
                [192, 192, 0],
                [0, 192, 0],
                [0, 0, 192],
                [110, 4, 171],
                [185, 7, 187],
            ];
            let colors = if self.single_mode {
                &mode_colors[..]
            } else {
                &main_colors[..]
            };
            let bounds = if self.single_mode {
                &self.assets.bounds[1..6]
            } else {
                &self.assets.bounds[..]
            };
            for (i, &bounds) in bounds.iter().enumerate() {
                // Native4433fd..443444: event2/3 enter/leave sets flag335.
                // Event4 press sets330/334; event6 release clears them.
                for p in ui_button::background(
                    bounds,
                    colors[i],
                    main_button_state(self.pressed, self.pressed_origin, self.selected, i),
                )? {
                    match p {
                        Primitive::Gradient { bounds, colors } => buttons.rect(bounds, colors),
                        Primitive::Outline {
                            bounds: [l, t, r, b],
                            color,
                        } => {
                            for (a, z) in [
                                ([l, t], [r, t]),
                                ([r, t], [r, b]),
                                ([r, b], [l, b]),
                                ([l, b], [l, t]),
                            ] {
                                buttons.line(a, z, color)
                            }
                        }
                        Primitive::Line { start, end, color } => buttons.line(start, end, color),
                    }
                }
            }
            for f in &mut self.assets.fonts {
                f.vertices.clear();
                f.alpha.clear();
            }
            let font = Font::parse(&self.assets.tab)?;
            let labels = if self.single_mode {
                &self.assets.mode_labels
            } else {
                &self.assets.labels
            };
            for (i, original_label) in labels.iter().enumerate() {
                let mut label = original_label.clone();
                if !self.single_mode
                    && i == 0
                    && let Some(current) = self.profile.store.current
                {
                    label.push(0);
                    label.extend(self.profile.store.names[current].bytes().map(|c| c - 32));
                }
                let label = &label;
                let [l, t, r, _] = bounds[i];
                let measure = font.layout(
                    label,
                    Layout {
                        origin: [0., 0.],
                        depth: 0.5,
                        scale: 1.,
                        gap: 0,
                        baseline: 0,
                    },
                )?;
                let width = measure
                    .iter()
                    .flat_map(|q| q.positions.iter().map(|p| p[0] + 0.5))
                    .fold(0.0f32, f32::max);
                let quads = font.layout(
                    label,
                    Layout {
                        origin: [(l + r - width) / 2., t + 5.],
                        depth: 0.5,
                        scale: 1.,
                        gap: 0,
                        baseline: 0,
                    },
                )?;
                for q in quads {
                    self.assets.fonts[q.page].quad(
                        q.positions.map(|p| [p[0], p[1]]),
                        q.uv,
                        [0xffffffff; 4],
                    );
                }
            }
            let mut modal = Surface::solid();
            let mut modal_fonts = self.assets.fonts.clone();
            for f in &mut modal_fonts {
                f.vertices.clear();
                f.alpha.clear();
            }
            if let Some(i) = self.pending {
                modal.rect([60., 195., 580., 285.], [0xee101030; 4]);
                let text = if i == 6 {
                    "QUIT? ENTER CONFIRMS - ESC RETURNS"
                } else {
                    [
                        "NAME SCREEN PENDING",
                        "TUTORIALS PENDING",
                        "SINGLE PLAYER PENDING",
                        "MULTI-PLAYER PENDING",
                        "LOAD GAME PENDING",
                        "SETTINGS PENDING",
                    ][i]
                };
                let text = if i == 6 {
                    text.to_owned()
                } else {
                    format!("{text} - ESC RETURNS")
                };
                let encoded: Vec<_> = text.bytes().map(|b| b - 32).collect();
                for q in font.layout(
                    &encoded,
                    Layout {
                        origin: [75., 222.],
                        depth: 0.5,
                        scale: 0.65,
                        gap: 0,
                        baseline: 0,
                    },
                )? {
                    modal_fonts[q.page].quad(
                        q.positions.map(|p| [p[0], p[1]]),
                        q.uv,
                        [0xffffffff; 4],
                    );
                }
            }
            let mut draws: Vec<_> = self
                .assets
                .base
                .iter()
                .enumerate()
                .filter(|(i, _)| !self.single_mode || *i < 4 || *i == 6)
                .map(|(_, s)| s.draw())
                .collect();
            draws.push(buttons.draw());
            draws.extend(self.assets.fonts.iter().map(Surface::draw));
            if self.pending.is_some() {
                draws.push(modal.draw());
                draws.extend(modal_fonts.iter().map(Surface::draw));
            }
            let projection = [
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
            ];
            self.prepared = Some(renderer.prepare(&draws, projection, None)?);
            self.dirty = false;
        }
        renderer.draw_overlay(color, depth, self.prepared.as_ref().unwrap());
        Ok(())
    }
}

#[cfg(test)]
mod light_click_tests {
    use super::{MenuCuePage, generic_cue_target, held_entry_click, main_button_state,
        release_activates, release_activates_without_track,
    };

    #[test]
    fn generic_button_first_held_entry_and_reentry_cue() {
        let button = generic_cue_target(MenuCuePage::Main, Some(4), true, false, false);
        let mut last = None;
        assert!(held_entry_click(&mut last, button));
        assert!(!held_entry_click(&mut last, button));
        assert!(!held_entry_click(&mut last, None)); // leave while held
        assert!(held_entry_click(&mut last, button));
        assert!(!held_entry_click(&mut last, None)); // release
        assert!(held_entry_click(&mut last, button)); // next press
    }

    #[test]
    fn only_generic_menu_buttons_cue() {
        let target = |page, selected, enabled| generic_cue_target(page, selected, true, false, enabled);
        assert_eq!(target(MenuCuePage::Main, Some(6), false), Some((0, 6)));
        assert_eq!(target(MenuCuePage::Load, Some(0), true), Some((1, 0)));
        assert_eq!(target(MenuCuePage::Load, Some(1), true), Some((1, 1)));
        assert_eq!(target(MenuCuePage::Load, Some(0), false), None);
        assert_eq!(target(MenuCuePage::Load, Some(2), false), None); // original import cannot delete
        assert_eq!(target(MenuCuePage::Load, Some(3), false), None); // list/slider
        assert_eq!(target(MenuCuePage::LoadPrompt, Some(1), false), Some((2, 1)));
        assert_eq!(target(MenuCuePage::Profile, Some(0), true), Some((3, 0)));
        assert_eq!(target(MenuCuePage::Profile, Some(2), false), None);
        assert_eq!(target(MenuCuePage::Profile, Some(4), true), None); // list row
        assert_eq!(target(MenuCuePage::Single, Some(0), true), Some((4, 0)));
        assert_eq!(target(MenuCuePage::Single, Some(1), true), Some((4, 1)));
        assert_eq!(target(MenuCuePage::Single, Some(3), true), Some((4, 3))); // Character arrow
        assert_eq!(target(MenuCuePage::Single, Some(0), false), None); // loading
        assert_eq!(target(MenuCuePage::Single, Some(4), true), None); // radio/list
        assert_eq!(target(MenuCuePage::Other, Some(0), false), None);
        assert_eq!(generic_cue_target(MenuCuePage::Main, Some(0), false, false, false), None);
        assert_eq!(generic_cue_target(MenuCuePage::Main, Some(0), true, true, false), None);
    }

    #[test]
    fn drag_to_another_main_button_hover_cues_without_press_or_activation() {
        let a = main_button_state(true, Some(0), Some(0), 0);
        assert!(a.flag_330 && a.flag_334 && a.flag_335);
        let b = main_button_state(true, Some(0), Some(1), 1);
        assert!(b.flag_330 && b.flag_335);
        assert!(!b.flag_334);
        assert!(!release_activates(true, Some(0), Some(1)));
        assert!(release_activates(true, Some(0), Some(0)));
    }

    #[test]
    fn profile_track_release_keeps_the_last_held_drag_sample() {
        assert!(!release_activates_without_track(true, Some(12), Some(12), true));
        assert!(release_activates_without_track(true, Some(11), Some(11), false));
        assert!(!release_activates_without_track(true, Some(12), None, true));
    }
}

#[cfg(test)]
mod settings_route_tests {
    #[test]
    #[ignore = "requires original menu/settings assets; OPENMT2001_SCRIPTS points to scripts root"]
    fn main_menu_settings_route_persists_selected_profile_for_launch() {
        use super::*;
        use crate::profile_settings::Snapshot;
        use crate::settings_menu::{Button, Page};
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let executable = read("OPENMT2001_EXE");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let scripts = std::path::PathBuf::from(std::env::var("OPENMT2001_SCRIPTS").unwrap());
        let root = std::env::current_dir().unwrap().join(format!("target/settings-route-{}-{}",
            std::process::id(), std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
        let _cleanup = Cleanup(root.clone());
        let assets = Assets::load(&index, &archive, installer, &language, &scripts,
            root.clone(), root.join("saves"), &executable,
        ).unwrap();
        let mut menu = Menu::new(assets);
        menu.profile.store.names = vec!["Settings Player".into(), "Other Player".into()];
        menu.profile.store.current = Some(0);
        menu.profile.visible = false;
        let path = menu.current_settings_path().unwrap();
        let initial = Snapshot::load(&path).unwrap();
        menu.settings_audio = initial.audio;
        menu.settings.graphics = initial.graphics;
        menu.settings.gameplay = initial.gameplay;
        menu.selected = Some(5);
        menu.activate(); // shared mouse-release / Enter activation path
        assert_eq!(menu.settings.page, Some(Page::Main));
        assert_eq!(menu.pending, None);
        let click = |menu: &mut Menu, button| {
            let bounds = menu.assets.settings.bounds(menu.settings.page.unwrap(), button).unwrap();
            menu.pointer([(bounds[0]+bounds[2])/2., (bounds[1]+bounds[3])/2.]);
            menu.mouse_button(true);
            menu.mouse_button(false);
        };
        click(&mut menu, Button::Gameplay);
        let track = menu.assets.settings.bounds(Page::Gameplay, Button::SensitivityTrack).unwrap();
        menu.pointer([track[0], (track[1] + track[3]) / 2.]);
        menu.mouse_button(true);
        let held = menu.settings.gameplay;
        menu.cancel_input(); // focus loss must cancel without activating a button
        menu.pointer([track[2], (track[1] + track[3]) / 2.]);
        assert_eq!(menu.settings.gameplay, held, "unheld motion after focus loss dragged slider");
        assert_eq!(menu.settings.page, Some(Page::Gameplay));
        click(&mut menu, Button::AutoRotate);
        menu.back(); // Escape from Gameplay
        assert_eq!(menu.settings.page, Some(Page::Main));
        click(&mut menu, Button::Back);
        assert_eq!(menu.settings.page, None);
        assert_eq!(menu.current_settings_path(), Some(path.clone()));
        assert_eq!(menu.profile.store.current, Some(0));
        assert!(menu.single.request.is_none());
        let launch = menu.settings_snapshot(); // same snapshot copied to Intro's launch levels
        launch.save(&path).unwrap();
        assert_eq!(Snapshot::load(&path).unwrap(), launch);
        assert_eq!(launch.gameplay.auto_rotate, !initial.gameplay.auto_rotate);
        assert_eq!(Snapshot::load(&crate::profile_settings::profile_path(&root, b"Other Player"))
            .unwrap(), Snapshot::default());
        menu.selected = Some(5);
        menu.activate();
        menu.back(); // Escape directly from Settings to menu
        assert_eq!(menu.settings.page, None);
        assert_eq!(menu.pending, None);
    }
}
