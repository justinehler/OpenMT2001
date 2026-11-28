//! Player comparison panel drawing and input (original 426ba0/427210/425900).
//! The state lives in `openmt2001::comparison`.
pub use openmt2001::comparison::{Data, Runtime, Snapshot, State, order};

/// Test helper: build the live comparison data and check it against the world.
#[cfg(any(test, feature = "test-support"))]
pub fn verify_runtime(
    world: &openmt2001::level_startup::World,
    players: &[[u8; 440]],
    image: &openmt2001::pe::PeImage<'_>,
    scenario: &str,
    difficulty: usize,
) -> Data {
    let data = Data::from_runtime(world, players, image, 0x591318).unwrap();
    assert_eq!(
        data.rows.len(),
        players.iter().filter(|p| p[0xc] != 0).count()
    );
    for (slot, values) in &data.rows {
        assert_eq!(
            values[0].to_bits(),
            world.ledgers.players[*slot].cash.to_bits()
        );
    }
    assert_eq!(data.objective, world.objective.kind);
    if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_COMPARISON_ORACLE") {
        let mut bytes = b"MTCFRESH".to_vec();
        bytes.extend((players.len() as u32).to_le_bytes());
        bytes.extend(world.clock.hour().to_le_bytes());
        for p in players {
            bytes.extend(p);
        }
        for c in &world.categories {
            bytes.extend(c);
        }
        for b in &world.buildings {
            bytes.extend(b);
        }
        for b in &world.ledgers.businesses {
            bytes.extend(b);
        }
        bytes.extend((data.rows.len() as u32).to_le_bytes());
        for (slot, values) in &data.rows {
            bytes.extend((*slot as u32).to_le_bytes());
            for v in values {
                bytes.extend(v.to_le_bytes());
            }
        }
        std::fs::write(format!("{prefix}-{scenario}-{difficulty}.bin"), bytes).unwrap();
    }
    data
}

pub struct View {
    pub data: Data,
    pub state: State,
    pub prepared: Option<openmt2001::render::PreparedScene>,
    hover: Option<usize>,
    pressed: Option<usize>,
    dirty: bool,
    up: [[bool; 6]; 9],
    live: bool,
    chat_pressed: Option<usize>,
}
impl View {
    pub fn new(data: Data) -> Self {
        Self {
            state: State::new(data.objective),
            data,
            prepared: None,
            hover: None,
            pressed: None,
            dirty: true,
            up: [[true; 6]; 9],
            live: false,
            chat_pressed: None,
        }
    }
    /// Keep fresh data and enabled metrics; retain only valid user choices.
    pub fn retain_ui_from(&mut self, old: &Self) {
        self.state.open = old.state.open;
        if self.state.enabled.get(old.state.mode) == Some(&true) {
            self.state.mode = old.state.mode;
        }
        self.up = old.up;
        self.live = old.live;
        if self.live {
            self.data = old.data.clone();
        }
        self.cancel_press();
        self.prepared = None;
    }
    pub fn apply_live(&mut self, snapshot: &Snapshot) {
        // A poll sent before a mode click can arrive after it. Wait for the
        // worker's selected-mode acknowledgement before accepting its rows.
        if snapshot.mode == self.state.mode {
            self.dirty |= self.data != snapshot.data || self.up != snapshot.up;
            self.data = snapshot.data.clone();
            self.up = snapshot.up;
            self.live = true;
        }
    }
    pub fn contains(&self, point: [i32; 2]) -> bool {
        let [x, y] = point.map(|v| v as f32 * 0.5);
        (4. ..=if self.state.open { 190.4 } else { 30.4 }).contains(&x)
            && (302. ..=472.4).contains(&y)
    }
    pub fn pressed(&self) -> bool {
        self.pressed.is_some()
    }
    pub fn pointer(&mut self, assets: &Assets, point: Option<[i32; 2]>) {
        let hover = point.and_then(|p| assets.button_at(&self.state, p));
        if hover != self.hover {
            self.hover = hover;
            self.dirty = true;
        }
    }
    pub fn chat_mouse(
        &mut self,
        assets: &Assets,
        point: Option<[i32; 2]>,
        down: bool,
    ) -> Option<usize> {
        let player = point.and_then(|p| assets.player_at(self, p));
        if down {
            self.chat_pressed = player;
            None
        } else {
            self.chat_pressed.take().filter(|p| Some(*p) == player)
        }
    }
    pub fn mouse(&mut self, down: bool) -> Option<usize> {
        let mut activated = None;
        if down {
            self.pressed = self.hover;
        } else if let Some(button) = self.pressed.take()
            && self.hover == Some(button)
        {
            activated = Some(button);
            match button {
                0 => self.state.open = !self.state.open,
                1 => self.state.cycle(false),
                2 => self.state.cycle(true),
                _ => {}
            }
        }
        self.dirty = true;
        activated
    }
    pub fn cancel_press(&mut self) {
        self.chat_pressed = None;
        self.pressed = None;
        self.hover = None;
        self.dirty = true;
    }
    pub fn prepare(
        &mut self,
        assets: &Assets,
        renderer: &openmt2001::render::Renderer,
        players: &[[u8; 440]],
    ) -> Result<(), String> {
        if self.dirty || self.prepared.is_none() {
            self.prepared = Some(assets.prepare(
                renderer,
                &self.state,
                players,
                &self.data.values(self.state.mode),
                &self.up[self.state.mode],
                self.hover,
                self.pressed,
            )?);
            self.dirty = false;
        }
        Ok(())
    }
}
/// Original426ca0: side gradient is always visible; body fills appear at
/// completed opening frame9, before dynamic player rows and widgets.
fn background_palette(frame: usize, body: u32) -> Vec<(&'static str, [u32; 4])> {
    let mut result = vec![(
        "P_STATS_LEFT_VERT",
        [0xff909090, 0x7f909090, 0x7f909090, 0xff909090],
    )];
    if frame == 9 {
        result.extend([
            ("P_STATS_MODE_BACKING1", [body; 4]),
            ("P_STATS_STATS_BACKING", [body; 4]),
        ]);
    }
    result
}
// The original mesh pass (43c4e0) includes the bar rings and selector
// backing. Reverse-facing regions remain layout-only, culled by the loader.
fn panel_frame(
    index: &openmt2001::archive::ArchiveIndex,
    archive: &[u8],
    frame: usize,
) -> Result<(Vec<crate::ui::Surface>, Vec<[f32; 4]>), String> {
    let parts: Vec<_> = (0..68).collect();
    crate::ui::panel_frame_base(
        index,
        archive,
        "gamedata2/brand_new_panels/stats_panel.tm",
        &parts,
        frame,
    )
}

fn add_background(
    surfaces: &mut Vec<crate::ui::Surface>,
    bounds: &[[f32; 4]],
    frame: usize,
    body: u32,
) -> Result<(), String> {
    let mut side = crate::ui::Surface::solid();
    let mut backing = crate::ui::Surface::solid();
    for ((_, colors), region) in background_palette(frame, body).into_iter().zip([54, 4, 5]) {
        let rect = *bounds.get(region).ok_or("comparison background region")?;
        if region == 54 {
            side.rect(rect, colors);
        } else {
            backing.rect(rect, colors);
        }
    }
    if frame == 9 {
        // 426ca0 submits the translucent fills behind the mesh artwork;
        // the opaque selector backing and all six rings precede row fills.
        surfaces.insert(0, backing);
    }
    surfaces.push(side);
    Ok(())
}

pub struct Assets {
    base: [Vec<crate::ui::Surface>; 2],
    regions: [Vec<[f32; 4]>; 2],
    generic: Vec<crate::ui::Surface>,
    fonts: Vec<crate::ui::Surface>,
    tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    chat_art: crate::chat_panel::Art,
    colors: [u32; 10],
    icons: [usize; 10],
}
impl Assets {
    pub fn apply_board(
        &mut self,
        board: &openmt2001::packed_language::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn verify_fresh(
        &self,
        data: Data,
        players: &[[u8; 440]],
        scene: &openmt2001::render_scene::Scene,
        scenario: &str,
    ) {
        assert!(scene.saved_players.is_empty());
        let mut view = View::new(data);
        let initial = view.state.mode;
        #[cfg(feature = "capture")]
        if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_COMPARISON_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            for _ in 0..9 {
                view.prepare(self, &renderer, players).unwrap();
                if view.state.mode == initial || view.state.mode == 0 {
                    crate::capture_test::capture(
                        &renderer,
                        view.prepared.as_ref(),
                        scene,
                        &format!("{prefix}-{scenario}-{}.png", view.state.mode),
                    );
                }
                view.state.cycle(true);
                view.dirty = true;
            }
            view.state.open = false;
            view.dirty = true;
            view.prepare(self, &renderer, players).unwrap();
        }
        let _ = (&mut view, initial, players, scenario);
    }
    pub fn load(
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
        installer: &std::path::Path,
        strings: &[&[u8]],
        exe: &[u8],
    ) -> Result<Self, String> {
        use crate::ui::Surface;
        let (mut closed, closed_regions) = panel_frame(index, archive, 0)?;
        let (mut open, open_regions) = panel_frame(index, archive, 9)?;
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index.find(name).ok_or("missing comparison asset")?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("comparison asset range".into())
        };
        let generic = openmt2001::texture::TextureSet::parse(payload(
            "gamedata2/GenericTextures640/textures.gtd",
        )?)
        .map_err(|e| e.to_string())?
        .textures
        .iter()
        .map(Surface::new)
        .collect();
        let fonts =
            openmt2001::texture::TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
                .map_err(|e| e.to_string())?
                .textures[6..8]
                .iter()
                .map(Surface::new)
                .collect();
        let image = openmt2001::pe::PeImage::parse(exe)?;
        let body = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        add_background(&mut closed, &closed_regions, 0, body)?;
        add_background(&mut open, &open_regions, 9, body)?;
        let mut colors = [0; 10];
        let mut icons = [0; 10];
        for i in 0..10 {
            colors[i] =
                u32::from_le_bytes(image.at(0x541af0 + i as u32 * 40, 4)?.try_into().unwrap());
            icons[i] =
                u32::from_le_bytes(image.at(0x541b00 + i as u32 * 40, 4)?.try_into().unwrap())
                    as usize;
        }
        Ok(Self {
            base: [closed, open],
            regions: [closed_regions, open_regions],
            generic,
            fonts,
            tab: std::fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
                .map_err(|e| e.to_string())?,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            chat_art: crate::chat_panel::Art::load(index, archive, body)?,
            colors,
            icons,
        })
    }
    pub fn chat_layout(&self) -> &crate::chat_panel::Layout {
        &self.chat_art.layout
    }
    pub fn player_at(&self, view: &View, point: [i32; 2]) -> Option<usize> {
        let [x, y] = point.map(|v| v as f32 * 0.5);
        let values = view.data.values(view.state.mode);
        let amounts: Vec<_> = values.iter().map(|r| r.1).collect();
        let order = order(&amounts);
        for (slot, &row) in order.iter().enumerate() {
            let [l, t, r, b] = self.regions[usize::from(view.state.open)][59 + slot];
            if x >= l && x <= r && y >= t && y <= b {
                return Some(values[row].0);
            }
        }
        None
    }
    pub fn prepare_chat(
        &self,
        renderer: &openmt2001::render::Renderer,
        chat: &openmt2001::network::chat::View,
        panel: &crate::chat_panel::Panel,
    ) -> Result<openmt2001::render::PreparedScene, String> {
        let surfaces = self.chat_surfaces(chat, panel)?;
        renderer.prepare(
            &surfaces
                .iter()
                .map(crate::ui::Surface::draw)
                .collect::<Vec<_>>(),
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
    fn chat_surfaces(
        &self,
        chat: &openmt2001::network::chat::View,
        panel: &crate::chat_panel::Panel,
    ) -> Result<Vec<crate::ui::Surface>, String> {
        use crate::ui::{self, Surface};
        let mut fonts = self.fonts.clone();
        let mut shapes = Surface::solid();
        let mut surfaces = Vec::new();
        let glyphs = |text: &[u8]| {
            text.iter()
                .map(|b| b.saturating_sub(32))
                .collect::<Vec<_>>()
        };
        let mut bottom = 478.;
        for line in chat.lines.iter().rev() {
            let color = line.color(&self.colors);
            ui::text_fit(
                &mut fonts,
                &self.tab,
                &glyphs(&line.text),
                [194., bottom - 14., 636., bottom],
                false,
                color,
            )?;
            bottom -= 14.;
        }
        if panel.open {
            let layout = &self.chat_art.layout;
            surfaces.extend(self.chat_art.base.clone());
            shapes.rect(layout.backing, [self.chat_art.body; 4]);
            shapes.rect(
                layout.title,
                [0xff909090, 0xff909090, 0x80909090, 0x80909090],
            );
            shapes.rect(layout.to, [0xff1f2f75; 4]);
            shapes.rect(layout.input, [0xff1f2f75; 4]);
            ui::text_fit(
                &mut fonts,
                &self.tab,
                self.strings
                    .get(1481)
                    .map(Vec::as_slice)
                    .unwrap_or(&glyphs(b"Chat")),
                layout.title,
                false,
                0xffffffff,
            )?;
            ui::text_fit(
                &mut fonts,
                &self.tab,
                self.strings
                    .get(1482)
                    .map(Vec::as_slice)
                    .unwrap_or(&glyphs(b"To:")),
                layout.to,
                false,
                0xffffffff,
            )?;
            for (&bounds, row) in layout.icons.iter().zip(crate::chat_panel::other_rows(chat)) {
                let token = chat.snapshot.rows[row].player.token;
                if let Ok(token) = usize::try_from(token)
                    && let Some(&texture) = self.icons.get(token)
                {
                    let mut icon = self.generic[texture].clone();
                    let [l, t, r, b] = bounds;
                    let color = if panel.recipients.0[row] {
                        0xffffffff
                    } else {
                        0x607f7f7f
                    };
                    icon.quad(
                        [[l, t], [l, b], [r, b], [r, t]],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [color; 4],
                    );
                    surfaces.push(icon);
                }
            }
            for (id, bounds, color) in [
                (1483, layout.all, 0xffff8800),
                (1761, layout.close, 0xffb20c0c),
            ] {
                shapes.rect(bounds, [color; 4]);
                ui::text_fit(
                    &mut fonts,
                    &self.tab,
                    self.strings.get(id).map(Vec::as_slice).unwrap_or(&[]),
                    bounds,
                    false,
                    0xffffffff,
                )?;
            }
            ui::text_entry(
                &mut fonts,
                &mut shapes,
                &self.tab,
                &glyphs(&panel.input),
                layout.input,
                false,
                true,
            )?;
        }
        surfaces.insert(0, shapes);
        surfaces.extend(fonts);
        Ok(surfaces)
    }
    pub fn button_at(&self, state: &State, point: [i32; 2]) -> Option<usize> {
        let x = point[0] as f32 * 0.5;
        let y = point[1] as f32 * 0.5;
        [57, 65, 66]
            .into_iter()
            .filter(|i| state.open || *i == 57)
            .position(|i| {
                let [l, t, r, b] = self.regions[usize::from(state.open)][i];
                x >= l && x <= r && y >= t && y <= b
            })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &self,
        renderer: &openmt2001::render::Renderer,
        state: &State,
        players: &[[u8; 440]],
        rows: &[(usize, f32)],
        up: &[bool; 6],
        hover: Option<usize>,
        pressed: Option<usize>,
    ) -> Result<openmt2001::render::PreparedScene, String> {
        let surfaces = self.surfaces(state, players, rows, up, hover, pressed)?;
        let draws: Vec<_> = surfaces.iter().map(crate::ui::Surface::draw).collect();
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
    fn surfaces(
        &self,
        state: &State,
        players: &[[u8; 440]],
        rows: &[(usize, f32)],
        up: &[bool; 6],
        hover: Option<usize>,
        pressed: Option<usize>,
    ) -> Result<Vec<crate::ui::Surface>, String> {
        use crate::ui::{self, Surface};
        let bounds = &self.regions[usize::from(state.open)];
        let mut surfaces = self.base[usize::from(state.open)].clone();
        let mut solid = Surface::solid();
        let mut fonts = self.fonts.clone();
        let mut row_icons = Vec::new();
        let mut icon = |texture: usize, region: usize| -> Result<(), String> {
            let mut s = self
                .generic
                .get(texture)
                .ok_or("comparison texture index")?
                .clone();
            let [l, t, r, b] = bounds[region];
            s.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(s);
            Ok(())
        };
        icon(
            88 + if pressed == Some(0) {
                2
            } else if hover == Some(0) {
                1
            } else {
                0
            },
            57,
        )?;
        icon(if state.open { 71 } else { 72 }, 58)?;
        if state.open {
            icon(
                97 + if pressed == Some(1) {
                    2
                } else if hover == Some(1) {
                    1
                } else {
                    0
                },
                65,
            )?;
            icon(
                101 + if pressed == Some(2) {
                    2
                } else if hover == Some(2) {
                    1
                } else {
                    0
                },
                66,
            )?;
            // Original427154..4271cb, all registered consecutively at68ae28.
            icon([10, 142, 139, 141, 140, 277, 289, 299, 298][state.mode], 56)?;
            ui::text_fit(
                &mut fonts,
                &self.tab,
                &self.strings[state.label() as usize],
                bounds[67],
                false,
                0xffffffff,
            )?;
        }
        let values: Vec<_> = rows.iter().map(|r| r.1).collect();
        let order = order(&values);
        let greatest = order.first().map_or(0., |&i| values[i]);
        for slot in 0..6 {
            let color = if let Some(&row) = order.get(slot) {
                let (player, value) = rows[row];
                let p = players.get(player).ok_or("comparison player index")?;
                let character = u32::from_le_bytes(p[0x14c..0x150].try_into().unwrap()) as usize;
                let color = *self
                    .colors
                    .get(character)
                    .ok_or("comparison character index")?;
                icon(self.icons[character], 59 + slot)?;
                if state.open {
                    let mut text_bounds = bounds[13 + slot * 7];
                    let [l, t, _, b] = text_bounds;
                    let mut cash = self.generic[10].clone();
                    cash.quad(
                        [[l, t], [l, b], [l + 12., b], [l + 12., t]],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [0xffffffff; 4],
                    );
                    row_icons.push(cash);
                    text_bounds[0] += 13.;
                    let name = &p[0x154..];
                    let end = name
                        .iter()
                        .position(|b| *b == 0)
                        .ok_or("unterminated comparison name")?;
                    let name = name[..end]
                        .iter()
                        .map(|b| b.checked_sub(32).ok_or("comparison name glyph"))
                        .collect::<Result<Vec<_>, _>>()?;
                    ui::text_fit(&mut fonts, &self.tab, &name, text_bounds, false, 0xffffffff)?;
                    let integer =
                        openmt2001::x87::Sum::from_f32(value).map_or(0, |v| v.ftol_low_word());
                    let mut amount = Vec::new();
                    if matches!(state.mode, 0 | 3 | 4 | 5) {
                        if integer < 0 {
                            amount.push(b'-' - 32);
                        }
                        let left = self.strings[2244].first() == Some(&0x4c);
                        if left {
                            amount.extend_from_slice(&self.strings[2243]);
                        }
                        amount.extend(integer.wrapping_abs().to_string().bytes().map(|b| b - 32));
                        if !left {
                            amount.extend_from_slice(&self.strings[2243]);
                        }
                    } else {
                        amount.extend(integer.to_string().bytes().map(|b| b - 32));
                    }
                    // 426d30 uses the original up-image width for both arrows.
                    // Generic ordinals110/12 are globals68afe0/68ae58.
                    let mut arrow = self.generic[if up[row] { 110 } else { 12 }].clone();
                    let [_, top, right, bottom] = text_bounds;
                    text_bounds[2] -= self.generic[110].width as f32;
                    arrow.quad(
                        [
                            [text_bounds[2], top],
                            [text_bounds[2], bottom],
                            [right, bottom],
                            [right, top],
                        ],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [0xffffffff; 4],
                    );
                    row_icons.push(arrow);
                    ui::text_fit(
                        &mut fonts,
                        &self.tab,
                        &amount,
                        text_bounds,
                        true,
                        0xffffffff,
                    )?;
                    let mut bar = bounds[17 + slot * 7];
                    let fraction = if greatest > 0. {
                        value.max(0.) / greatest
                    } else {
                        0.
                    };
                    bar[2] = bar[0] + (bar[2] - bar[0]) * fraction;
                    let border = bounds[12 + slot * 7];
                    let width = f64::from(border[2] - border[0]).trunc();
                    let alpha = |x: f32| {
                        (((f64::from(border[2]) - f64::from(x)) * 128. / width + 127.).trunc()
                            as u32)
                            .wrapping_shl(24)
                            | color
                    };
                    solid.rect(
                        bar,
                        [alpha(bar[0]), alpha(bar[0]), alpha(bar[2]), alpha(bar[2])],
                    );
                }
                color
            } else {
                0x3f3f3f
            };
            if state.open {
                for part in 13..=16 {
                    let b = bounds[part + slot * 7];
                    let border = bounds[12 + slot * 7];
                    let width = f64::from(border[2] - border[0]).trunc();
                    let right = (((f64::from(border[2]) - f64::from(b[2])) * 128. / width + 127.)
                        .trunc() as u32)
                        .wrapping_shl(24)
                        | color;
                    solid.rect(b, [0xff000000 | color, 0xff000000 | color, right, right]);
                }
            }
        }
        surfaces.push(solid);
        surfaces.extend(row_icons);
        surfaces.extend(fonts);
        Ok(surfaces)
    }
}
#[cfg(test)]
mod tests {

    #[test]
    fn live_snapshot_survives_scene_refresh_and_rejects_old_mode_poll() {
        let data = |value| super::Data {
            objective: 0,
            rows: vec![(0, [value; 9])],
        };
        let mut runtime = super::Runtime::new(data(100.));
        runtime.refresh(data(50.));
        let mut view = super::View::new(data(100.));
        view.apply_live(&runtime.snapshot);
        assert!(!view.up[0][0]);
        let mut refreshed = super::View::new(data(999.));
        refreshed.retain_ui_from(&view);
        assert_eq!(refreshed.data.values(0), vec![(0, 50.)]);
        assert!(!refreshed.up[0][0]);
        refreshed.state.cycle(true);
        refreshed.apply_live(&runtime.snapshot);
        assert_eq!(refreshed.state.mode, 1);
        assert_eq!(refreshed.data.values(0), vec![(0, 50.)]);
        runtime.select(1, data(12.)).unwrap();
        refreshed.apply_live(&runtime.snapshot);
        assert_eq!(refreshed.data.values(1), vec![(0, 12.)]);
        assert!(refreshed.up[1][0]);
    }

    #[test]
    #[ignore = "requires supplied original Comparison mesh and UI assets"]
    fn original_comparison_background_materials() {
        use crate::ui::{self, Surface};
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let dir = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = openmt2001::archive::ArchiveIndex::parse(&dir, archive.len() as u64).unwrap();
        let path = "gamedata2/brand_new_panels/stats_panel.tm";
        let entry = index.find(path).unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        let regions = openmt2001::ui_mesh::regions(&mesh, 9).unwrap();
        for (name, part) in [("P_STATS_MODE_BACKING1", 4), ("P_STATS_STATS_BACKING", 5)] {
            assert_eq!(regions.iter().find(|r| r.name == name).unwrap().part, part);
            assert_eq!(
                mesh.parts[part].indices.len(),
                12,
                "runtime fill covers a full placeholder quad"
            );
            assert!(
                mesh.texture_names[mesh.parts[part].texture as usize]
                    .eq_ignore_ascii_case("1NEW_MAYA_GREY_BORDER.TT")
            );
        }
        let sun = openmt2001::fixtures::path("SUNCOLS");
        let assets = super::Assets::load(
            &index,
            &archive,
            sun.parent().unwrap().parent().unwrap(),
            &[],
            &exe,
        )
        .unwrap();
        let body = u32::from_le_bytes(
            openmt2001::pe::PeImage::parse(&exe)
                .unwrap()
                .at(0x5362c0, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        assert_eq!(body, 0xbb000a28);
        // The selector and bar rings are opaque mesh textures, not extra
        // runtime rectangles or sampled screenshot colours.
        let positions = mesh.positions(9).unwrap();
        for (part, source_color, vertex_count) in [
            (55, [0, 23, 111, 255], 6),
            (12, [146, 144, 144, 255], 24),
            (19, [146, 144, 144, 255], 24),
            (26, [146, 144, 144, 255], 24),
            (33, [146, 144, 144, 255], 24),
            (40, [146, 144, 144, 255], 24),
            (47, [146, 144, 144, 255], 24),
        ] {
            let entry = index
                .find(mesh.texture_names[mesh.parts[part].texture as usize])
                .unwrap();
            let texture = openmt2001::texture::TextureSet::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap();
            assert!(
                texture.textures[0]
                    .texels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| *pixel == source_color)
            );
            let actual = &assets.base[1][if part == 55 { 17 } else { 11 + (part - 12) / 7 }];
            assert_eq!(actual.vertices.len(), vertex_count);
            assert!(actual.alpha.iter().all(|alpha| *alpha == 255));
            assert_eq!(
                actual.pixels,
                openmt2001::texture::loader_pixels(&texture.textures[0])
            );
            let expected: Vec<_> = mesh.parts[part]
                .vertex_indices()
                .map(|i| [positions[i][0], -positions[i][1]])
                .collect();
            assert_eq!(
                actual
                    .vertices
                    .iter()
                    .map(|v| [v[0], v[1]])
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert_eq!(assets.base[1].len(), 19);
        for (state, frame) in [(0, 0), (1, 9)] {
            let parts: Vec<_> = (0..68).collect();
            let (border, _) = ui::panel_frame_base(&index, &archive, path, &parts, frame).unwrap();
            let submitted = &assets.base[state];
            let offset = usize::from(frame == 9);
            assert_eq!(submitted.len(), border.len() + offset + 1);
            for (actual, original) in submitted[offset..offset + border.len()].iter().zip(&border) {
                assert_eq!(
                    actual.vertices, original.vertices,
                    "retained border geometry/order"
                );
                assert_eq!(actual.pixels, original.pixels, "retained border material");
                assert_eq!(actual.alpha, original.alpha, "retained border opacity");
            }
            let named = openmt2001::ui_mesh::regions(&mesh, frame).unwrap();
            let mut side = Surface::solid();
            let mut backing = Surface::solid();
            for (name, colors) in super::background_palette(frame, body) {
                let bounds = named.iter().find(|r| r.name == name).unwrap().bounds;
                if name == "P_STATS_LEFT_VERT" {
                    side.rect(bounds, colors);
                } else {
                    backing.rect(bounds, colors);
                }
            }
            let actual_side = submitted.last().unwrap();
            assert_eq!(actual_side.vertices, side.vertices);
            assert_eq!(
                actual_side.alpha, side.alpha,
                "intentional gray side gradient retained"
            );
            if frame == 9 {
                let actual = &submitted[0];
                assert_eq!(
                    actual.vertices, backing.vertices,
                    "navy body precedes every border"
                );
                assert_eq!(actual.alpha, [187; 12]);
                assert_eq!(actual.pixels, [255; 4]);
                assert_eq!(actual.draw().mode, openmt2001::render::SurfaceMode::Overlay);
            }
        }
    }
    #[test]
    #[ignore = "private original comparison draw fixture"]
    fn matches_native_background_palette() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_STATS_STYLE")).unwrap();
        assert_eq!(&bytes[..8], b"MTSTATCL");
        let mut at = 8;
        fn word(bytes: &[u8], at: &mut usize) -> u32 {
            let v = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        }
        for _ in 0..word(&bytes, &mut at) {
            let frame = word(&bytes, &mut at) as usize;
            let actual = super::background_palette(frame, 0xbb000a28);
            assert_eq!(word(&bytes, &mut at) as usize, actual.len());
            for (name, colors) in actual {
                let length = word(&bytes, &mut at) as usize;
                assert_eq!(name.as_bytes(), &bytes[at..at + length]);
                at += length;
                assert_eq!(colors, std::array::from_fn(|_| word(&bytes, &mut at)));
            }
        }
        assert_eq!(at, bytes.len());
    }

    use super::*;
    #[test]
    #[ignore = "requires original saved city and UI assets"]
    fn comparison_saved_panel_flow() {
        use openmt2001::{
            archive::ArchiveIndex,
            pe::PeImage,
            render::Renderer,
            render_scene::{OriginalAssets, Request, Source, assemble},
        };
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe_path = openmt2001::fixtures::path("EXE");
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let off = std::fs::read(language.join("tycoon.off")).unwrap();
        let bin = std::fs::read(language.join("tycoon.bin")).unwrap();
        let mut offset = 0;
        let strings: Vec<_> = off
            .chunks_exact(2)
            .map(|n| {
                let len = u16::from_le_bytes(n.try_into().unwrap()) as usize;
                let text = &bin[offset..offset + len];
                offset += len;
                text
            })
            .collect();
        let sun = openmt2001::fixtures::path("SUNCOLS");
        let assets = Assets::load(
            &index,
            &archive,
            sun.parent().unwrap().parent().unwrap(),
            &strings,
            &exe,
        )
        .unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(target_os = "windows") {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        for (i, name) in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"]
            .iter()
            .enumerate()
        {
            let bytes = read(name);
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let data = Data::from_save(&save, &image, save.clock().unwrap().hour()).unwrap();
            let mut view = View::new(data);
            let players: Vec<_> = save.players().unwrap().iter().map(|p| *p.bytes).collect();
            let scene = assemble(Request {
                build_file: &build,
                source: Source::Save(&bytes),
                original: Some(OriginalAssets {
                    executable: &exe,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: None,
                phase: None,
                camera: Some(
                    crate::navigation::Navigation::load(&index, &archive, &exe)
                        .unwrap()
                        .camera,
                ),
                width: 1280,
                height: 960,
                animated: false,
            })
            .unwrap();
            for mode in 0..9 {
                view.state.mode = mode;
                view.dirty = true;
                view.prepare(&assets, &renderer, &players).unwrap();
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_COMPARISON_CAPTURE") {
                    crate::capture_test::capture(
                        &renderer,
                        view.prepared.as_ref(),
                        &scene,
                        &format!("{prefix}-{i}-{mode}.png"),
                    );
                }
            }
            let _ = (&scene, i);
            let initial = view.state.mode;
            view.pointer(&assets, Some([350, 630]));
            view.mouse(true);
            view.pointer(&assets, Some([400, 630]));
            view.mouse(false);
            assert_eq!(view.state.mode, initial);
            view.pointer(&assets, Some([350, 630]));
            view.mouse(true);
            view.mouse(false);
            assert_ne!(view.state.mode, initial);
            view.pointer(&assets, Some([30, 630]));
            view.mouse(true);
            view.mouse(false);
            assert!(!view.state.open);
            assert!(!view.contains([200, 700]));
            assert!(view.contains([30, 700]));
            view.prepare(&assets, &renderer, &players).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_COMPARISON_CAPTURE") {
                crate::capture_test::capture(
                    &renderer,
                    view.prepared.as_ref(),
                    &scene,
                    &format!("{prefix}-{i}-closed.png"),
                );
            }
            view.mouse(true);
            view.cancel_press();
            view.mouse(false);
            assert!(!view.state.open);
        }
    }
}

#[cfg(test)]
mod refresh_ui_tests {
    use super::*;
    #[test]
    fn selected_metric_survives_new_objective_and_values_without_input_capture() {
        let mut old = View::new(Data {
            objective: 0,
            rows: vec![(0, [1.; 9])],
        });
        old.state.mode = 4;
        old.state.open = false;
        old.pressed = Some(2);
        old.hover = Some(2);
        let mut new = View::new(Data {
            objective: 1,
            rows: vec![(1, [42.; 9])],
        });
        assert_eq!(new.state.mode, 6);
        new.retain_ui_from(&old);
        assert_eq!(new.state.mode, 4);
        assert!(!new.state.open);
        assert_eq!(new.data.objective, 1);
        assert_eq!(new.data.values(4), vec![(1, 42.)]);
        assert!(!new.state.enabled[7]);
        assert!(
            new.pressed.is_none() && new.hover.is_none() && new.prepared.is_none() && new.dirty
        );
        old.state.mode = 7;
        old.state.open = true;
        let mut fallback = View::new(new.data.clone());
        fallback.retain_ui_from(&old);
        assert!(fallback.state.open);
        assert_eq!(fallback.state.mode, 6);
        old.state.mode = usize::MAX;
        fallback.retain_ui_from(&old);
        assert_eq!(fallback.state.mode, 6);
    }
}

#[cfg(test)]
#[path = "comparison_draw_tests.rs"]
mod draw_tests;
