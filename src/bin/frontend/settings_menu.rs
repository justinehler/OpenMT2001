//! Retained in-game Settings navigation from the original panel meshes.
//! Sound levels drive the host audio buses. Graphics controls retain visible
//! selections; Draw Distance drives moving-scene visibility. Static building
//! distance and profile persistence remain separate seams.
use crate::ui::Surface;
use openmt2001::{
    archive::ArchiveIndex,
    font::{Font, Layout},
    mesh::CachedMesh,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
    ui_button::{self, Primitive, State as ButtonState},
    ui_mesh,
};
use std::{fs, path::Path};

const VIEW: [f32; 16] = [
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Main,
    Sound,
    Graphics,
    Gameplay,
}
impl Page {
    fn index(self) -> usize {
        match self {
            Self::Main => 0,
            Self::Sound => 1,
            Self::Graphics => 2,
            Self::Gameplay => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Sound,
    Graphics,
    Gameplay,
    Back,
    MusicLeft,
    MusicTrack,
    MusicRight,
    EffectsLeft,
    EffectsTrack,
    EffectsRight,
    DetailLow,
    DetailMedium,
    DetailHigh,
    Bilinear,
    Trilinear,
    DistanceLeft,
    DistanceTrack,
    DistanceRight,
    AutoRotate,
    InvertVertical,
    SensitivityLeft,
    SensitivityTrack,
    SensitivityRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Levels {
    pub music: u8,
    pub effects: u8,
}
impl Default for Levels {
    fn default() -> Self {
        Self {
            music: 63,
            effects: 83,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicsLevels {
    pub detail: u8,
    pub filtering: u8,
    pub distance: u8,
}
impl Default for GraphicsLevels {
    // 493bbd seeds 50 during construction; 493ce0 overwrites from the
    // runtime 9206ec value initialized to 35 by 4c18e5..4c1920.
    fn default() -> Self {
        Self {
            detail: 2,
            filtering: 1,
            distance: 35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameplayLevels {
    pub auto_rotate: bool,
    pub invert_vertical: bool,
    pub hardware_cursor: bool,
    pub mouse_sensitivity: u8,
}
impl Default for GameplayLevels {
    fn default() -> Self {
        Self {
            auto_rotate: true,
            invert_vertical: false,
            hardware_cursor: false,
            mouse_sensitivity: 50,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MenuState {
    page: Page,
    hovered: Option<Button>,
    pressed: Option<Button>,
    levels: Levels,
    graphics: GraphicsLevels,
    gameplay: GameplayLevels,
}

struct Panel {
    base: Vec<Surface>,
    regions: Vec<[f32; 4]>,
}
pub struct Assets {
    panels: [Panel; 4],
    fonts: Vec<Surface>,
    font1: Vec<Surface>,
    font3: Vec<Surface>,
    slider_textures: Vec<Surface>,
    radio_buttons: [Surface; 2],
    checkboxes: [Surface; 2],
    tab: Vec<u8>,
    tab1: Vec<u8>,
    tab3: Vec<u8>,
    strings: Vec<Vec<u8>>,
}
impl Assets {
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("missing settings asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or_else(|| format!("settings asset {name} outside archive"))
        };
        let mut panels = Vec::new();
        for name in [
            "options_menu.tm",
            "sound_menu.tm",
            "graphics_options.tm",
            "gameplay_options.tm",
        ] {
            let path = format!("gamedata2/brand_new_panels/{name}");
            let mesh = CachedMesh::parse(payload(&path)?)?;
            let regions: Vec<[f32; 4]> = ui_mesh::regions(&mesh, 0)?
                .iter()
                .map(|r| r.bounds)
                .collect();
            let pos = mesh.positions(0)?;
            let uv = mesh.uvs();
            let mut base = Vec::new();
            // Native 43c3a0 submits the panel mesh before page overlays.
            // Its static submission includes the thin title/options borders.
            let static_parts: &[usize] = match name {
                // The source Default parts are submitted to the native
                // renderer, but the supplied original Settings frame does
                // not show their detached gray raster. The downstream
                // native depth/material outcome is still under study.
                "sound_menu.tm" => &[0, 1, 2, 3, 4, 6],
                "graphics_options.tm" => &[0, 1, 2, 3, 4, 6],
                "gameplay_options.tm" => &[0, 1, 2, 3, 4, 6],
                "options_menu.tm" => &[0, 1, 2, 3, 4],
                _ => &[0, 1, 2, 3],
            };
            for &part_index in static_parts {
                let part = mesh
                    .parts
                    .get(part_index)
                    .ok_or("missing Options border mesh")?;
                let tex = TextureSet::parse(payload(mesh.texture_names[part.texture as usize])?)
                    .map_err(|e| e.to_string())?;
                let mut surface = Surface::new(&tex.textures[0]);
                for i in part.vertex_indices() {
                    surface
                        .vertices
                        .push([pos[i][0], -pos[i][1], 0.5, 1., 1., 1., uv[i][0], uv[i][1]]);
                    surface.alpha.push(255);
                }
                base.push(surface);
            }
            // Native Main 4969e0, Sound 49c080, Graphics 493d80 and Gameplay
            // 4931c0 pass these regions to untextured 433370 rectangles.
            let overlays: &[(usize, u32, u32)] = match name {
                "options_menu.tm" => &[(5, 0xff6e04ab, 0x806e04ab)],
                "sound_menu.tm" => &[
                    (5, 0xff6e04ab, 0x806e04ab),
                    (10, 0xbb000a28, 0xbb000a28),
                    (18, 0xbb000a28, 0xbb000a28),
                    (8, 0xbb000a28, 0xbb000a28),
                    (7, 0xff909090, 0x80909090),
                ],
                "graphics_options.tm" => &[
                    (5, 0xff6e04ab, 0x806e04ab),
                    (10, 0xbb000a28, 0xbb000a28),
                    (19, 0xbb000a28, 0xbb000a28),
                    (26, 0xbb000a28, 0xbb000a28),
                    (8, 0xbb000a28, 0xbb000a28),
                    (7, 0xff909090, 0x80909090),
                ],
                "gameplay_options.tm" => &[
                    (5, 0xff6e04ab, 0x806e04ab),
                    (10, 0xbb000a28, 0xbb000a28),
                    (15, 0xbb000a28, 0xbb000a28),
                    (20, 0xbb000a28, 0xbb000a28),
                    (25, 0xbb000a28, 0xbb000a28),
                    (8, 0xbb000a28, 0xbb000a28),
                    (7, 0xff909090, 0x80909090),
                ],
                _ => &[],
            };
            for &(part_index, left_color, right_color) in overlays {
                let mut surface = Surface::solid();
                surface.rect(
                    regions[part_index],
                    [left_color, left_color, right_color, right_color],
                );
                base.push(surface);
            }
            panels.push(Panel { base, regions });
        }
        let panels = panels.try_into().map_err(|_| "settings panel count")?;
        let atlas = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let fonts = atlas.textures[8..10].iter().map(Surface::new).collect();
        let font1 = atlas.textures[2..4].iter().map(Surface::new).collect();
        let font3 = atlas.textures[6..8].iter().map(Surface::new).collect();
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut slider_textures = vec![Surface::solid(); 105];
        for (i, slot) in slider_textures.iter_mut().enumerate().skip(97) {
            *slot = Surface::new(
                generic
                    .textures
                    .get(i)
                    .ok_or("original Options slider arrow missing")?,
            );
        }
        // Native 4938ae/493985 bind handles 174/175. The generic atlas
        // contains the corresponding selected/clear blue radios here.
        let radio_buttons = [174, 175]
            .map(|i| {
                generic
                    .textures
                    .get(i)
                    .map(Surface::new)
                    .ok_or("original Options radio missing")
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Options radio count")?;
        let checkboxes = [276, 275]
            .map(|i| {
                generic
                    .textures
                    .get(i)
                    .map(Surface::new)
                    .ok_or("original Gameplay checkbox missing")
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Gameplay checkbox count")?;
        let tab = fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
            .map_err(|e| e.to_string())?;
        let tab1 = fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let tab3 = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        let off = fs::read(language.join("tycoon.off")).map_err(|e| e.to_string())?;
        let bin = fs::read(language.join("tycoon.bin")).map_err(|e| e.to_string())?;
        let mut cursor = 0usize;
        let mut rows = Vec::new();
        for bytes in off.chunks_exact(2) {
            let end = cursor + u16::from_le_bytes(bytes.try_into().unwrap()) as usize;
            rows.push(
                bin.get(cursor..end)
                    .ok_or("settings language row outside data")?,
            );
            cursor = end;
        }
        let strings = crate::tooltip::localized_strings(installer, &rows)?;
        for id in [
            1679, 1607, 1608, 1609, 1610, 1770, 1139, 1435, 1129, 1130, 1663, 1586, 1583, 1664,
            1665, 1595, 1587, 1588, 1591, 1592, 1593, 1594, 1596, 1582, 1656, 1655, 1657, 1570,
            1571, 1572, 1573, 1574, 1575, 1584, 1585, 1738,
        ] {
            strings.get(id).ok_or("original Settings string missing")?;
        }
        Ok(Self {
            panels,
            fonts,
            font1,
            font3,
            slider_textures,
            radio_buttons,
            checkboxes,
            tab,
            tab1,
            tab3,
            strings,
        })
    }
    fn panel(&self, page: Page) -> &Panel {
        &self.panels[page.index()]
    }
    pub fn bounds(&self, page: Page, button: Button) -> Option<[f32; 4]> {
        let regions = &self.panel(page).regions;
        let arrow = match (page, button) {
            (Page::Sound, Button::MusicLeft) => Some((14, 0)),
            (Page::Sound, Button::MusicRight) => Some((14, 1)),
            (Page::Sound, Button::EffectsLeft) => Some((22, 0)),
            (Page::Sound, Button::EffectsRight) => Some((22, 1)),
            (Page::Graphics, Button::DistanceLeft) => Some((27, 0)),
            (Page::Graphics, Button::DistanceRight) => Some((27, 1)),
            (Page::Gameplay, Button::SensitivityLeft) => Some((29, 0)),
            (Page::Gameplay, Button::SensitivityRight) => Some((29, 1)),
            _ => None,
        };
        if let Some((track, side)) = arrow {
            return Some(
                crate::ui::slider_arrows(regions.get(track)?.map(f32::trunc), false)[side],
            );
        }
        let index = match (page, button) {
            (Page::Main, Button::Sound) => 6,
            (Page::Main, Button::Graphics) => 7,
            (Page::Main, Button::Gameplay) => 10,
            (Page::Main, Button::Back) => 8,
            (Page::Sound, Button::Back) => 26,
            (Page::Sound, Button::MusicLeft) => 15,
            (Page::Sound, Button::MusicTrack) => 14,
            (Page::Sound, Button::MusicRight) => 16,
            (Page::Sound, Button::EffectsLeft) => 23,
            (Page::Sound, Button::EffectsTrack) => 22,
            (Page::Sound, Button::EffectsRight) => 24,
            (Page::Graphics, Button::DetailLow) => 11,
            (Page::Graphics, Button::DetailMedium) => 12,
            (Page::Graphics, Button::DetailHigh) => 13,
            (Page::Graphics, Button::Bilinear) => 20,
            (Page::Graphics, Button::Trilinear) => 21,
            (Page::Graphics, Button::DistanceLeft) => 30,
            (Page::Graphics, Button::DistanceTrack) => 27,
            (Page::Graphics, Button::DistanceRight) => 29,
            (Page::Graphics, Button::Back) => 34,
            (Page::Gameplay, Button::AutoRotate) => 13,
            (Page::Gameplay, Button::InvertVertical) => 18,
            (Page::Gameplay, Button::SensitivityLeft) => 30,
            (Page::Gameplay, Button::SensitivityTrack) => 29,
            (Page::Gameplay, Button::SensitivityRight) => 31,
            (Page::Gameplay, Button::Back) => 33,
            _ => return None,
        };
        regions.get(index).copied()
    }
    pub fn button(&self, page: Page, pointer: [f32; 2]) -> Option<Button> {
        let buttons: &[Button] = if page == Page::Sound {
            &[
                Button::MusicLeft,
                Button::MusicRight,
                Button::MusicTrack,
                Button::EffectsLeft,
                Button::EffectsRight,
                Button::EffectsTrack,
                Button::Back,
            ]
        } else if page == Page::Graphics {
            &[
                Button::DetailLow,
                Button::DetailMedium,
                Button::DetailHigh,
                Button::Bilinear,
                Button::Trilinear,
                Button::DistanceLeft,
                Button::DistanceRight,
                Button::DistanceTrack,
                Button::Back,
            ]
        } else if page == Page::Gameplay {
            &[
                Button::AutoRotate,
                Button::InvertVertical,
                Button::SensitivityLeft,
                Button::SensitivityRight,
                Button::SensitivityTrack,
                Button::Back,
            ]
        } else {
            &[
                Button::Sound,
                Button::Graphics,
                Button::Gameplay,
                Button::Back,
            ]
        };
        buttons.iter().copied().into_iter().find(|&button| {
            self.bounds(page, button).is_some_and(|[l, t, r, b]| {
                pointer[0] >= l && pointer[0] <= r && pointer[1] >= t && pointer[1] <= b
            })
        })
    }
    /// Original action widgets request cursor1 only inside their enabled bounds.
    /// Slider, radio and checkbox subclasses have separate cursor contracts.
    pub fn action_cursor_hover(&self, page: Page, pointer: Option<[f32; 2]>) -> bool {
        let Some([x, y]) = pointer else {
            return false;
        };
        let buttons: &[Button] = if page == Page::Main {
            &[
                Button::Sound,
                Button::Graphics,
                Button::Gameplay,
                Button::Back,
            ]
        } else {
            &[Button::Back]
        };
        buttons.iter().any(|&button| {
            self.bounds(page, button)
                .is_some_and(|[l, t, r, b]| x > l && x < r && y >= t && y <= b)
        })
    }
    fn label(&self, id: usize) -> &[u8] {
        &self.strings[id]
    }
    #[cfg(test)]
    pub fn label_for_test(&self, id: usize) -> &[u8] {
        self.label(id)
    }
    fn text(
        &self,
        fonts: &mut [Surface],
        text: &[u8],
        bounds: [f32; 4],
        scale: f32,
    ) -> Result<(), String> {
        self.text_colored(fonts, text, bounds, scale, 0xffffffff)
    }
    fn text_colored(
        &self,
        fonts: &mut [Surface],
        text: &[u8],
        bounds: [f32; 4],
        scale: f32,
        color: u32,
    ) -> Result<(), String> {
        let font = Font::parse(&self.tab)?;
        let [l, t, r, b] = bounds;
        let layout = |origin| {
            font.layout(
                text,
                Layout {
                    origin,
                    depth: 0.5,
                    scale,
                    gap: 0,
                    baseline: 0,
                },
            )
        };
        let measure = layout([0., 0.])?;
        let width = measure
            .iter()
            .flat_map(|q| q.positions.iter().map(|p| p[0] + 0.5))
            .fold(0f32, f32::max);
        for q in layout([(l + r - width) / 2., t + (b - t - 25. * scale) / 2.])? {
            fonts[q.page].quad(q.positions.map(|p| [p[0], p[1]]), q.uv, [color; 4]);
        }
        Ok(())
    }
    fn button_surface(
        &self,
        surface: &mut Surface,
        bounds: [f32; 4],
        color: [u8; 3],
        hovered: bool,
        pressed: bool,
    ) -> Result<(), String> {
        self.button_surface_state(
            surface,
            bounds,
            color,
            ButtonState {
                flag_330: pressed && hovered,
                flag_334: pressed && hovered,
                flag_335: hovered,
                flag_333: false,
            },
        )
    }
    fn button_surface_state(
        &self,
        surface: &mut Surface,
        bounds: [f32; 4],
        color: [u8; 3],
        state: ButtonState,
    ) -> Result<(), String> {
        for primitive in ui_button::background(bounds, color, state)? {
            match primitive {
                Primitive::Gradient { bounds, colors } => surface.rect(bounds, colors),
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
                        surface.line(a, z, color);
                    }
                }
                Primitive::Line { start, end, color } => surface.line(start, end, color),
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)] // Original row regions and three draw targets stay explicit.
    fn slider(
        &self,
        art: &mut Surface,
        icons: &mut Vec<Surface>,
        fonts: &mut [Surface],
        panel: &Panel,
        value: u8,
        indices: [usize; 5],
        hovered: Option<Button>,
        pressed: Option<Button>,
        controls: [Button; 3],
    ) -> Result<(), String> {
        let [low, _blue, track, _left, _right] = indices;
        let bounds = panel.regions[track].map(f32::trunc);
        let [left, right] = crate::ui::slider_arrows(bounds, false);
        let center =
            |region: [f32; 4]| [(region[0] + region[2]) * 0.5, (region[1] + region[3]) * 0.5];
        let state_point = |state: Option<Button>| match state {
            Some(button) if button == controls[0] => Some(center(left)),
            Some(button) if button == controls[2] => Some(center(right)),
            _ => None,
        };
        crate::ui::Slider {
            bounds,
            vertical: false,
            value: f32::from(value),
            range: [0., 100.],
            pointer: state_point(hovered),
            pressed: state_point(pressed),
        }
        .paint(art, icons, &self.slider_textures);
        self.text(fonts, self.label(1130), panel.regions[low], 0.5)?;
        self.text(fonts, self.label(1129), panel.regions[low + 1], 0.5)?;
        Ok(())
    }
    #[cfg(test)]
    pub fn prepare(
        &self,
        renderer: &Renderer,
        page: Page,
        hovered: Option<Button>,
        pressed: Option<Button>,
        levels: Levels,
    ) -> Result<PreparedScene, String> {
        self.prepare_with_graphics(
            renderer,
            page,
            hovered,
            pressed,
            levels,
            GraphicsLevels::default(),
        )
    }
    #[cfg(test)]
    fn prepare_with_graphics(
        &self,
        renderer: &Renderer,
        page: Page,
        hovered: Option<Button>,
        pressed: Option<Button>,
        levels: Levels,
        graphics: GraphicsLevels,
    ) -> Result<PreparedScene, String> {
        self.prepare_with_options(
            renderer,
            MenuState {
                page,
                hovered,
                pressed,
                levels,
                graphics,
                gameplay: GameplayLevels::default(),
            },
        )
    }
    fn prepare_with_options(
        &self,
        renderer: &Renderer,
        state: MenuState,
    ) -> Result<PreparedScene, String> {
        let MenuState {
            page,
            hovered,
            pressed,
            levels,
            graphics,
            gameplay,
        } = state;
        let panel = self.panel(page);
        let mut art = Surface::solid();
        let mut icons = Vec::new();
        let mut fonts = self.fonts.clone();
        let mut font1 = self.font1.clone();
        let mut font3 = self.font3.clone();
        let title = match page {
            Page::Main => 1679,
            Page::Sound => 1663,
            Page::Graphics => 1586,
            Page::Gameplay => 1583,
        };
        crate::ui::text_widget_style9(
            &mut fonts,
            &self.tab,
            self.label(title),
            panel.regions[5],
            0xffffffff,
        )?;
        if page == Page::Main {
            for (region, label, button, color) in [
                (6, 1607, Some(Button::Sound), [110, 4, 171]),
                (7, 1608, Some(Button::Graphics), [110, 4, 171]),
                (10, 1609, Some(Button::Gameplay), [110, 4, 171]),
                (11, 1610, None, [110, 4, 171]),
                (12, 1770, None, [110, 4, 171]),
            ] {
                let active = button.is_some();
                let selected = active && button == hovered;
                self.button_surface_state(
                    &mut art,
                    panel.regions[region],
                    color,
                    ButtonState {
                        flag_330: selected && button == pressed,
                        flag_334: selected && button == pressed,
                        flag_335: selected,
                        flag_333: !active,
                    },
                )?;
                crate::ui::text_widget_style9(
                    &mut fonts,
                    &self.tab,
                    self.label(label),
                    panel.regions[region],
                    if button.is_some() {
                        0xffffffff
                    } else {
                        0xff646464
                    },
                )?;
            }
            self.button_surface_state(
                &mut art,
                panel.regions[9],
                [166, 133, 0],
                ButtonState {
                    flag_330: false,
                    flag_334: false,
                    flag_335: false,
                    flag_333: true,
                },
            )?;
            crate::ui::text_widget_style9(
                &mut font1,
                &self.tab1,
                self.label(1139),
                panel.regions[9],
                0xff646464,
            )?;
        } else {
            let heading = match page {
                Page::Sound => 1666,
                Page::Graphics => 1595,
                Page::Gameplay => 1582,
                Page::Main => unreachable!(),
            };
            if matches!(page, Page::Sound | Page::Graphics | Page::Gameplay) {
                crate::ui::text_widget_style4(
                    &mut font1,
                    &self.tab1,
                    self.label(heading),
                    panel.regions[7],
                    0xffffffff,
                )?;
            }
            let rows: &[(usize, usize)] = match page {
                Page::Sound => &[(9, 1664), (17, 1665)],
                Page::Graphics => &[(9, 1587), (18, 1591), (25, 1596)],
                Page::Gameplay => &[(9, 1570), (14, 1572), (19, 1574), (24, 1738)],
                Page::Main => unreachable!(),
            };
            for &(region, label) in rows {
                if page == Page::Sound {
                    // Native 49bade/49bb6a enable the text widget's own
                    // uniform green backing on Music/Effects captions.
                    art.rect(panel.regions[region], [0xff01762f; 4]);
                } else {
                    // Native 4419f0 paints each caption widget's enabled
                    // background before its glyphs (+1b0, +1ac=ff01762f).
                    art.rect(panel.regions[region], [0xff01762f; 4]);
                }
                if matches!(page, Page::Sound | Page::Graphics) {
                    crate::ui::text_widget_style9(
                        &mut font1,
                        &self.tab1,
                        self.label(label),
                        panel.regions[region],
                        0xffffffff,
                    )?;
                } else if page == Page::Gameplay {
                    let (pages, tab) = if region == 24 {
                        (&mut font1, &self.tab1)
                    } else {
                        (&mut font3, &self.tab3)
                    };
                    crate::ui::text_widget_style9(
                        pages,
                        tab,
                        self.label(label),
                        panel.regions[region],
                        0xffffffff,
                    )?;
                }
            }
            if page == Page::Sound {
                self.slider(
                    &mut art,
                    &mut icons,
                    &mut fonts,
                    panel,
                    levels.music,
                    [11, 13, 14, 15, 16],
                    hovered,
                    pressed,
                    [Button::MusicLeft, Button::MusicTrack, Button::MusicRight],
                )?;
                self.slider(
                    &mut art,
                    &mut icons,
                    &mut fonts,
                    panel,
                    levels.effects,
                    [19, 21, 22, 23, 24],
                    hovered,
                    pressed,
                    [
                        Button::EffectsLeft,
                        Button::EffectsTrack,
                        Button::EffectsRight,
                    ],
                )?;
            } else if page == Page::Graphics {
                for (index, label_region, selected, label) in [
                    (11, 14, graphics.detail == 0, 1656),
                    (12, 15, graphics.detail == 1, 1655),
                    (13, 16, graphics.detail == 2, 1657),
                    (20, 22, graphics.filtering == 0, 1592),
                    (21, 23, graphics.filtering == 1, 1593),
                ] {
                    let mut icon = self.radio_buttons[if selected { 0 } else { 1 }].clone();
                    let [l, t, r, b] = panel.regions[index];
                    icon.quad(
                        [[l, t], [l, b], [r, b], [r, t]],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [0xffffffff; 4],
                    );
                    icons.push(icon);
                    crate::ui::text_widget_style9(
                        &mut font3,
                        &self.tab3,
                        self.label(label),
                        panel.regions[label_region],
                        0xffffffff,
                    )?;
                }
                crate::ui::text_widget_style9(
                    &mut font3,
                    &self.tab3,
                    self.label(1588),
                    panel.regions[17],
                    0xffffffff,
                )?;
                crate::ui::text_widget_style9(
                    &mut font3,
                    &self.tab3,
                    self.label(1594),
                    panel.regions[24],
                    0xffffffff,
                )?;
                let bounds = panel.regions[27].map(f32::trunc);
                let [left, right] = crate::ui::slider_arrows(bounds, false);
                let center = |region: [f32; 4]| {
                    [(region[0] + region[2]) * 0.5, (region[1] + region[3]) * 0.5]
                };
                let pointer = match hovered {
                    Some(Button::DistanceLeft) => Some(center(left)),
                    Some(Button::DistanceRight) => Some(center(right)),
                    _ => None,
                };
                let pressed = match pressed {
                    Some(Button::DistanceLeft) => Some(center(left)),
                    Some(Button::DistanceRight) => Some(center(right)),
                    _ => None,
                };
                crate::ui::Slider {
                    bounds,
                    vertical: false,
                    value: f32::from(graphics.distance),
                    range: [0., 100.],
                    pointer,
                    pressed,
                }
                .paint(&mut art, &mut icons, &self.slider_textures);
                crate::ui::text_widget_style9(
                    &mut font3,
                    &self.tab3,
                    self.label(1130),
                    panel.regions[32],
                    0xffffffff,
                )?;
                crate::ui::text_widget_style9(
                    &mut font3,
                    &self.tab3,
                    self.label(1129),
                    panel.regions[31],
                    0xffffffff,
                )?;
            } else if page == Page::Gameplay {
                for (box_region, enabled_region, text_region, enabled) in [
                    (13, 12, 11, gameplay.auto_rotate),
                    (18, 17, 16, gameplay.invert_vertical),
                    (22, 21, 23, gameplay.hardware_cursor),
                ] {
                    let mut icon = self.checkboxes[if enabled { 0 } else { 1 }].clone();
                    let [l, t, r, b] = panel.regions[box_region];
                    icon.quad(
                        [[l, t], [l, b], [r, b], [r, t]],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [0xffffffff; 4],
                    );
                    icons.push(icon);
                    crate::ui::text_widget_style9(
                        &mut font3,
                        &self.tab3,
                        self.label(if enabled { 1584 } else { 1585 }),
                        panel.regions[enabled_region],
                        0xffffffff,
                    )?;
                    let text_id = match text_region {
                        11 => 1571,
                        16 => 1573,
                        23 => 1575,
                        _ => unreachable!(),
                    };
                    crate::ui::text_widget_style3(
                        &mut font3,
                        &self.tab3,
                        self.label(text_id),
                        panel.regions[text_region],
                        0xffffffff,
                    )?;
                }
                let bounds = panel.regions[29].map(f32::trunc);
                let [left, right] = crate::ui::slider_arrows(bounds, false);
                let center = |region: [f32; 4]| {
                    [(region[0] + region[2]) * 0.5, (region[1] + region[3]) * 0.5]
                };
                let pointer = match hovered {
                    Some(Button::SensitivityLeft) => Some(center(left)),
                    Some(Button::SensitivityRight) => Some(center(right)),
                    _ => None,
                };
                let pressed = match pressed {
                    Some(Button::SensitivityLeft) => Some(center(left)),
                    Some(Button::SensitivityRight) => Some(center(right)),
                    _ => None,
                };
                crate::ui::Slider {
                    bounds,
                    vertical: false,
                    value: f32::from(gameplay.mouse_sensitivity),
                    range: [0., 100.],
                    pointer,
                    pressed,
                }
                .paint(&mut art, &mut icons, &self.slider_textures);
                crate::ui::text_widget_style9(
                    &mut font3,
                    &self.tab3,
                    self.label(1130),
                    panel.regions[26],
                    0xffffffff,
                )?;
                crate::ui::text_widget_style9(
                    &mut font3,
                    &self.tab3,
                    self.label(1129),
                    panel.regions[27],
                    0xffffffff,
                )?;
            }
        }
        let back = self
            .bounds(page, Button::Back)
            .ok_or("original Settings OK bounds missing")?;
        self.button_surface(
            &mut art,
            back,
            if page == Page::Main {
                [80, 148, 34]
            } else {
                [0, 180, 0]
            },
            hovered == Some(Button::Back),
            pressed == Some(Button::Back),
        )?;
        crate::ui::text_widget_style9(&mut font1, &self.tab1, self.label(1435), back, 0xffffffff)?;
        let mut draws = panel.base.iter().map(Surface::draw).collect::<Vec<_>>();
        draws.push(art.draw());
        draws.extend(icons.iter().map(Surface::draw));
        draws.extend(fonts.iter().map(Surface::draw));
        draws.extend(font1.iter().map(Surface::draw));
        draws.extend(font3.iter().map(Surface::draw));
        renderer.prepare(&draws, VIEW, None)
    }
}

#[derive(Default)]
pub struct View {
    pub page: Option<Page>,
    pub graphics: GraphicsLevels,
    pub gameplay: GameplayLevels,
    pressed: Option<Button>,
    button_down: bool,
    click_latch: Option<Button>,
    shown: Option<MenuState>,
    pub prepared: Option<PreparedScene>,
}
impl View {
    pub fn cancel_input(&mut self) {
        self.pressed = None;
        self.button_down = false;
        self.click_latch = None;
        self.shown = None;
    }
    pub fn open(&mut self) {
        self.page = Some(Page::Main);
        self.pressed = None;
        self.button_down = false;
        self.click_latch = None;
        self.shown = None;
    }
    pub fn back(&mut self) {
        self.page = match self.page {
            Some(Page::Main) | None => None,
            Some(_) => Some(Page::Main),
        };
        self.pressed = None;
        self.button_down = false;
        self.click_latch = None;
        self.shown = None;
        self.prepared = None;
    }
    /// Native 443360/42f2b0/42c9e0 sound latch for active Settings buttons,
    /// radios and checkboxes. The 43f510 slide zone itself is silent; its arrow
    /// children use the generic button cue. A held pointer can cue on re-entry.
    pub fn pointer_click_cue(&mut self, assets: &Assets, pointer: Option<[f32; 2]>) -> bool {
        let hit = self
            .page
            .and_then(|page| pointer.and_then(|p| assets.button(page, p)));
        if !self.button_down {
            self.click_latch = None;
            return false;
        }
        if matches!(
            self.pressed,
            Some(
                Button::MusicTrack
                    | Button::EffectsTrack
                    | Button::DistanceTrack
                    | Button::SensitivityTrack
            )
        ) {
            self.click_latch = None;
            return false;
        }
        let hit = hit.filter(|button| {
            !matches!(
                button,
                Button::MusicTrack
                    | Button::EffectsTrack
                    | Button::DistanceTrack
                    | Button::SensitivityTrack
            )
        });
        if self.click_latch == hit {
            return false;
        }
        self.click_latch = hit;
        hit.is_some()
    }
    fn track_value(assets: &Assets, button: Button, pointer: [f32; 2]) -> Option<u8> {
        let page = match button {
            Button::DistanceTrack => Page::Graphics,
            Button::SensitivityTrack => Page::Gameplay,
            _ => Page::Sound,
        };
        let bounds = assets.bounds(page, button)?;
        let track = crate::ui::slider_track(bounds.map(f32::trunc), false);
        Some(crate::ui::slider_selection(track, pointer[0], 100, false) as u8)
    }
    pub fn drag(
        &mut self,
        assets: &Assets,
        pointer: Option<[f32; 2]>,
        levels: &mut Levels,
    ) -> bool {
        let Some(pointer) = pointer else {
            return false;
        };
        if self.pressed == Some(Button::DistanceTrack) {
            let value = Self::track_value(assets, Button::DistanceTrack, pointer).unwrap();
            if self.graphics.distance == value {
                return false;
            }
            self.graphics.distance = value;
            self.shown = None;
            return false;
        }
        if self.pressed == Some(Button::SensitivityTrack) {
            let value = Self::track_value(assets, Button::SensitivityTrack, pointer).unwrap();
            if self.gameplay.mouse_sensitivity != value {
                self.gameplay.mouse_sensitivity = value;
                self.shown = None;
            }
            return false;
        }
        let target = match self.pressed {
            Some(Button::MusicTrack) => &mut levels.music,
            Some(Button::EffectsTrack) => &mut levels.effects,
            _ => return false,
        };
        let value = Self::track_value(assets, self.pressed.unwrap(), pointer).unwrap();
        if *target == value {
            return false;
        }
        *target = value;
        true
    }
    pub fn mouse(
        &mut self,
        assets: &Assets,
        pointer: Option<[f32; 2]>,
        pressed: bool,
        levels: &mut Levels,
    ) -> bool {
        let Some(page) = self.page else {
            return false;
        };
        self.button_down = pressed;
        let hit = pointer.and_then(|p| assets.button(page, p));
        if pressed {
            self.pressed = hit;
            return self.drag(assets, pointer, levels);
        }
        let activated = (hit == self.pressed).then_some(hit).flatten();
        self.pressed = None;
        match activated {
            Some(Button::Back) => self.back(),
            Some(Button::Sound) => {
                self.page = Some(Page::Sound);
                self.shown = None;
            }
            Some(Button::Graphics) => {
                self.page = Some(Page::Graphics);
                self.shown = None;
            }
            Some(Button::Gameplay) => {
                self.page = Some(Page::Gameplay);
                self.shown = None;
            }
            Some(Button::DetailLow) => self.graphics.detail = 0,
            Some(Button::DetailMedium) => self.graphics.detail = 1,
            Some(Button::DetailHigh) => self.graphics.detail = 2,
            Some(Button::Bilinear) => self.graphics.filtering = 0,
            Some(Button::Trilinear) => self.graphics.filtering = 1,
            Some(Button::DistanceLeft) => {
                self.graphics.distance = self.graphics.distance.saturating_sub(1)
            }
            Some(Button::DistanceRight) => {
                self.graphics.distance = (self.graphics.distance + 1).min(100)
            }
            Some(Button::AutoRotate) => self.gameplay.auto_rotate = !self.gameplay.auto_rotate,
            Some(Button::InvertVertical) => {
                self.gameplay.invert_vertical = !self.gameplay.invert_vertical
            }
            Some(Button::SensitivityLeft) => {
                self.gameplay.mouse_sensitivity = self.gameplay.mouse_sensitivity.saturating_sub(1)
            }
            Some(Button::SensitivityRight) => {
                self.gameplay.mouse_sensitivity = (self.gameplay.mouse_sensitivity + 1).min(100)
            }
            Some(
                button @ (Button::MusicLeft
                | Button::MusicRight
                | Button::EffectsLeft
                | Button::EffectsRight),
            ) => {
                let value = if matches!(button, Button::MusicLeft | Button::MusicRight) {
                    &mut levels.music
                } else {
                    &mut levels.effects
                };
                let old = *value;
                if matches!(button, Button::MusicLeft | Button::EffectsLeft) {
                    *value = value.saturating_sub(1);
                } else {
                    *value = (*value + 1).min(100);
                }
                return *value != old;
            }
            Some(
                Button::MusicTrack
                | Button::EffectsTrack
                | Button::DistanceTrack
                | Button::SensitivityTrack,
            )
            | None => {}
        }
        false
    }
    pub fn update_levels(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        pointer: Option<[f32; 2]>,
        levels: Levels,
    ) -> Result<(), String> {
        let Some(page) = self.page else {
            return Ok(());
        };
        let hovered = pointer.and_then(|p| assets.button(page, p));
        let state = MenuState {
            page,
            hovered,
            pressed: self.pressed,
            levels,
            graphics: self.graphics,
            gameplay: self.gameplay,
        };
        if self.shown != Some(state) {
            self.prepared = Some(assets.prepare_with_options(renderer, state)?);
            self.shown = Some(state);
        }
        Ok(())
    }
}

#[cfg(test)]
mod graphics_tests {
    #[test]
    #[ignore = "requires original v1.4 Settings parent mesh fixture"]
    fn native_default_parent_submission_does_not_imply_visible_raster() {
        use super::{Assets, Page};
        use std::{fs, path::PathBuf};
        let data = fs::read("work/frontend/native-settings-base.bin").unwrap();
        assert_eq!(&data[..8], b"MTSTBP01");
        let mut cursor = 8usize;
        fn word(data: &[u8], cursor: &mut usize) -> u32 {
            let value = u32::from_le_bytes(data[*cursor..*cursor + 4].try_into().unwrap());
            *cursor += 4;
            value
        }
        assert_eq!(word(&data, &mut cursor), 3);
        let directory = fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
        let archive = fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameters.parent().unwrap().parent().unwrap();
        let executable = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let assets = Assets::load(&index, &archive, installer, &language).unwrap();
        for (expected_page, page) in [Page::Sound, Page::Graphics, Page::Gameplay]
            .into_iter()
            .enumerate()
        {
            let actual_page = word(&data, &mut cursor) as usize;
            let part = word(&data, &mut cursor) as usize;
            let texture = word(&data, &mut cursor);
            let indices = word(&data, &mut cursor) as usize;
            let bounds: [f32; 4] =
                std::array::from_fn(|_| f32::from_bits(word(&data, &mut cursor)));
            let rgba = &data[cursor..cursor + 4];
            cursor += 4;
            assert_eq!(actual_page, expected_page);
            assert_eq!(part, [25, 33, 32][expected_page]);
            assert_eq!((texture, indices), (5, 6));
            assert_eq!(rgba, [146, 144, 144, 255]);
            let panel = assets.panel(page);
            for (got, want) in panel.regions[part].into_iter().zip(bounds) {
                assert_eq!(
                    got.to_bits(),
                    want.to_bits(),
                    "{page:?} native parent bounds"
                );
            }
            // 43c3a0 submits this source part, but the supplied original
            // gameplay Settings frame has no detached gray Default block.
            // The submission probe stubs the deferred material/raster path.
            assert_eq!(
                panel.base.len(),
                [11, 12, 13][expected_page],
                "{page:?} Default source part must not become a visible quad"
            );
        }
        assert_eq!(cursor, data.len());
    }
    #[test]
    #[ignore = "requires original v1.4 Graphics Options mesh and localization"]
    fn original_graphics_controls_and_visual_state() {
        use super::{Assets, Button, GameplayLevels, GraphicsLevels, Levels, Page, View};
        use std::{fs, path::PathBuf};
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameters.parent().unwrap().parent().unwrap();
        let executable = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let assets = Assets::load(&index, &archive, installer, &language).unwrap();
        // Generic action hit4342c0 is strict X and inclusive Y. All other
        // widget families retain their existing cursor policy in this slice.
        for (page, buttons) in [
            (
                Page::Main,
                &[
                    Button::Sound,
                    Button::Graphics,
                    Button::Gameplay,
                    Button::Back,
                ][..],
            ),
            (Page::Sound, &[Button::Back][..]),
            (Page::Graphics, &[Button::Back][..]),
            (Page::Gameplay, &[Button::Back][..]),
        ] {
            assert!(!assets.action_cursor_hover(page, None));
            assert!(!assets.action_cursor_hover(page, Some([1., 1.])));
            for &button in buttons {
                let [l, t, r, b] = assets.bounds(page, button).unwrap();
                let x = (l + r) * 0.5;
                let y = (t + b) * 0.5;
                assert!(
                    assets.action_cursor_hover(page, Some([x, y])),
                    "{page:?} {button:?}"
                );
                assert!(!assets.action_cursor_hover(page, Some([l, y])));
                assert!(!assets.action_cursor_hover(page, Some([r, y])));
                assert!(assets.action_cursor_hover(page, Some([x, t])));
                assert!(assets.action_cursor_hover(page, Some([x, b])));
                assert!(!assets.action_cursor_hover(page, Some([x, t - 0.01])));
                assert!(!assets.action_cursor_hover(page, Some([x, b + 0.01])));
            }
        }
        for region in [9, 11, 12] {
            // disabled Main defaults, Board and Credits
            let [l, t, r, b] = assets.panel(Page::Main).regions[region];
            assert!(!assets.action_cursor_hover(Page::Main, Some([(l + r) * 0.5, (t + b) * 0.5])));
        }
        for (page, button) in [
            (Page::Sound, Button::MusicLeft),
            (Page::Sound, Button::MusicTrack),
            (Page::Sound, Button::MusicRight),
            (Page::Graphics, Button::DetailHigh),
            (Page::Graphics, Button::Trilinear),
            (Page::Gameplay, Button::AutoRotate),
            (Page::Gameplay, Button::InvertVertical),
        ] {
            let [l, t, r, b] = assets.bounds(page, button).unwrap();
            assert!(!assets.action_cursor_hover(page, Some([(l + r) * 0.5, (t + b) * 0.5])));
        }
        let [l, t, r, b] = assets.panel(Page::Sound).regions[25];
        assert_eq!(
            assets.button(Page::Sound, [(l + r) * 0.5, (t + b) * 0.5]),
            None,
            "Sound mesh region 25 is not an interactive Reset control"
        );
        for (button, expected) in [
            (Button::DetailLow, [143.09, 157.84, 159.09, 173.84]),
            (Button::DistanceTrack, [224., 298.59, 422., 314.59]),
        ] {
            let actual = assets.bounds(Page::Graphics, button).unwrap();
            for (got, want) in actual.into_iter().zip(expected) {
                assert!(
                    (got - want).abs() < 0.02,
                    "{button:?} original mesh bounds {actual:?}"
                );
            }
        }
        assert_eq!(
            assets.bounds(Page::Graphics, Button::DistanceLeft),
            Some([224., 298., 240., 314.])
        );
        assert_eq!(
            assets.bounds(Page::Graphics, Button::DistanceRight),
            Some([406., 298., 422., 314.])
        );
        assert_eq!(
            assets.button(Page::Graphics, [215., 306.]),
            None,
            "old mesh arrow position is outside native generated child"
        );
        assert_eq!(
            assets.button(Page::Graphics, [232., 306.]),
            Some(Button::DistanceLeft)
        );
        assert_eq!(
            GraphicsLevels::default(),
            GraphicsLevels {
                detail: 2,
                filtering: 1,
                distance: 35
            }
        );
        let mut view = View::default();
        let mut audio = Levels::default();
        view.open();
        let click = |view: &mut View, button: Button, audio: &mut Levels| {
            let [l, t, r, b] = assets.bounds(view.page.unwrap(), button).unwrap();
            let point = [(l + r) * 0.5, (t + b) * 0.5];
            view.mouse(&assets, Some(point), true, audio);
            assert!(
                view.pointer_click_cue(&assets, Some(point)),
                "{button:?} plays native press cue"
            );
            assert!(
                !view.pointer_click_cue(&assets, Some(point)),
                "{button:?} held inside keeps native click latch"
            );
            view.mouse(&assets, Some(point), false, audio);
            assert!(!view.pointer_click_cue(&assets, Some(point)));
        };
        click(&mut view, Button::Graphics, &mut audio);
        assert_eq!(view.page, Some(Page::Graphics));
        click(&mut view, Button::DetailLow, &mut audio);
        let [l, t, r, b] = assets.bounds(Page::Graphics, Button::DetailLow).unwrap();
        let low = [(l + r) * 0.5, (t + b) * 0.5];
        view.mouse(&assets, Some(low), true, &mut audio);
        assert!(view.pointer_click_cue(&assets, Some(low)));
        assert!(!view.pointer_click_cue(&assets, None));
        assert!(
            view.pointer_click_cue(&assets, Some(low)),
            "native radio cues again on held re-entry"
        );
        view.mouse(&assets, Some(low), false, &mut audio);
        assert!(!view.pointer_click_cue(&assets, Some(low)));
        click(&mut view, Button::Bilinear, &mut audio);
        assert_eq!((view.graphics.detail, view.graphics.filtering), (0, 0));
        let [l, t, r, b] = assets
            .bounds(Page::Graphics, Button::DistanceTrack)
            .unwrap();
        let track = crate::ui::slider_track([l, t, r, b].map(f32::trunc), false);
        assert_eq!(
            View::track_value(
                &assets,
                Button::DistanceTrack,
                [track[0] + 1.1, (t + b) * 0.5]
            ),
            Some(0),
            "native 432260 uses integer maximum+1 bins rather than rounding"
        );
        let start = [track[0] + 1., (t + b) * 0.5];
        view.mouse(&assets, Some(start), true, &mut audio);
        assert!(
            !view.pointer_click_cue(&assets, Some(start)),
            "original slider slide zone is silent"
        );
        view.drag(
            &assets,
            Some([track[0] + (track[2] - track[0]) * 0.72, (t + b) * 0.5]),
            &mut audio,
        );
        assert!(
            !view.pointer_click_cue(&assets, Some([r, (t + b) * 0.5])),
            "captured slider stays silent across arrow"
        );
        view.mouse(&assets, Some([r, (t + b) * 0.5]), false, &mut audio);
        assert!(!view.pointer_click_cue(&assets, Some([r, (t + b) * 0.5])));
        assert_eq!(view.graphics.distance, 72);
        assert_eq!(audio, Levels::default());
        click(&mut view, Button::Back, &mut audio);
        click(&mut view, Button::Gameplay, &mut audio);
        assert_eq!(view.page, Some(Page::Gameplay));
        assert_eq!(view.gameplay, GameplayLevels::default());
        for (button, expected) in [
            (Button::AutoRotate, [138., 159.86, 154., 175.86]),
            (Button::InvertVertical, [138., 206.86, 154., 222.86]),
            (Button::SensitivityTrack, [224., 317.37, 422., 333.37]),
        ] {
            let actual = assets.bounds(Page::Gameplay, button).unwrap();
            for (got, want) in actual.into_iter().zip(expected) {
                assert!(
                    (got - want).abs() < 0.02,
                    "{button:?} original mesh bounds {actual:?}"
                );
            }
        }
        let [l, t, r, b] = assets.panel(Page::Gameplay).regions[22];
        assert_eq!(
            assets.button(Page::Gameplay, [(l + r) * 0.5, (t + b) * 0.5]),
            None,
            "native Hardware Cursor checkbox is disabled"
        );
        click(&mut view, Button::AutoRotate, &mut audio);
        click(&mut view, Button::InvertVertical, &mut audio);
        assert!(!view.gameplay.auto_rotate && view.gameplay.invert_vertical);
        let [l, t, r, b] = assets
            .bounds(Page::Gameplay, Button::SensitivityTrack)
            .unwrap();
        let track = crate::ui::slider_track([l, t, r, b].map(f32::trunc), false);
        assert_eq!(
            View::track_value(
                &assets,
                Button::SensitivityTrack,
                [track[0] + 1.1, (t + b) * 0.5]
            ),
            Some(0)
        );
        view.mouse(
            &assets,
            Some([track[0] + 1., (t + b) * 0.5]),
            true,
            &mut audio,
        );
        view.drag(
            &assets,
            Some([track[0] + (track[2] - track[0]) * 0.75, (t + b) * 0.5]),
            &mut audio,
        );
        view.mouse(&assets, Some([r, (t + b) * 0.5]), false, &mut audio);
        assert_eq!(view.gameplay.mouse_sensitivity, 75);
        assert_eq!(audio, Levels::default());
        #[cfg(feature = "capture")]
        {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            for (name, graphics) in [
                ("default", GraphicsLevels::default()),
                ("selected", view.graphics),
            ] {
                let panel = assets
                    .prepare_with_graphics(&renderer, Page::Graphics, None, None, audio, graphics)
                    .unwrap();
                crate::capture_test::capture_standalone(
                    &renderer,
                    &panel,
                    &format!("work/frontend/graphics-options-{name}.png"),
                );
            }
            let sound = assets
                .prepare(&renderer, Page::Sound, None, None, audio)
                .unwrap();
            crate::capture_test::capture_standalone(
                &renderer,
                &sound,
                "work/frontend/sound-options-native-slider.png",
            );
            for (name, gameplay) in [
                ("default", GameplayLevels::default()),
                ("selected", view.gameplay),
            ] {
                let panel = assets
                    .prepare_with_options(
                        &renderer,
                        super::MenuState {
                            page: Page::Gameplay,
                            hovered: None,
                            pressed: None,
                            levels: audio,
                            graphics: view.graphics,
                            gameplay,
                        },
                    )
                    .unwrap();
                crate::capture_test::capture_standalone(
                    &renderer,
                    &panel,
                    &format!("work/frontend/gameplay-options-{name}.png"),
                );
            }
        }
    }
}
