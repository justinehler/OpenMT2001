//! Building management presentation. Quotes and admission come from the live worker.
use crate::{
    business_operations::Inspection,
    ui::{self, Surface},
};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path, sync::mpsc};

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub action: [f32; 4],
    pub description: [f32; 4],
    pub quote: [f32; 4],
    pub list_top: f32,
}
impl Layout {
    /// Original408da0, with432ff0/433010 dimensions already in640-pixel coordinates.
    pub fn new([l, t, r, _b]: [f32; 4], [w, h]: [u32; 2]) -> Self {
        let x = l + ((f64::from(r) - f64::from(l)) * 0.5 - f64::from(w / 2)).trunc() as f32;
        Self {
            action: [x, t + 70., x + w as f32, t + 70. + h as f32],
            description: [l + 4., t + 5., r - 4., t + 45.],
            quote: [l + 4., t + 40., r - 4., t + 55.],
            list_top: (f64::from(t + 40.) + f64::from(t) + 40.).trunc() as f32,
        }
    }
}
pub struct Assets {
    base: Vec<Surface>,
    title_backgrounds: Vec<Surface>,
    textures: Vec<Surface>,
    fonts: [Vec<Surface>; 3],
    tabs: [Vec<u8>; 3],
    title_fonts: Vec<Surface>,
    title_tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    modes: [[f32; 4]; 6],
    bounds: [f32; 4],
    title_text_bounds: [f32; 4],
    pub layout: Layout,
    detail_buttons: [[f32; 4]; 2],
    information: ui::MppModes,
    color: u32,
    palette: Vec<u32>,
    report_line_height: i32,
}
fn hit([l, t, r, b]: [f32; 4], [x, y]: [f32; 2]) -> bool {
    x >= l && x <= r && y >= t && y <= b
}
fn quad(s: &mut Surface, [l, t, r, b]: [f32; 4], color: u32) {
    s.quad(
        [[l, t], [l, b], [r, b], [r, t]],
        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
        [color; 4],
    );
}
impl Assets {
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        exe: &[u8],
    ) -> Result<Self, String> {
        let payload = |n: &str| -> Result<&[u8], String> {
            let e = index
                .find(n)
                .ok_or_else(|| format!("Missing building panel asset {n}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("Building asset bounds".into())
        };
        let name = "gamedata2/brand_new_panels/main_panel_v2.tm";
        let (base, _) = ui::panel_frame_base(index, archive, name, &(0..9).collect::<Vec<_>>(), 9)?;
        let mesh = CachedMesh::parse(payload(name)?)?;
        let image = PeImage::parse(exe)?;
        // 4087c2 uses neutral owner gray when the building has no owner.
        let mut title_backgrounds = vec![ui::mpp_background_title(
            &mesh,
            9,
            &image,
            Some([0xff808080, 0xff808080, 0x55808080, 0x55808080]),
        )?];
        for character in 0..10 {
            // 408750 reads the owner's character palette, then 41f080 sets
            // the title's opaque-left and one-third-alpha-right colors.
            let color =
                u32::from_le_bytes(image.at(0x541af0 + character * 40, 4)?.try_into().unwrap());
            let faded = (color & 0x00ff_ffff) | (((color >> 24) / 3) << 24);
            title_backgrounds.push(ui::mpp_background_title(
                &mesh,
                9,
                &image,
                Some([color, color, faded, faded]),
            )?);
        }
        let regions = openmt2001::ui_mesh::regions(&mesh, 9)?;
        let region = |n: &str| -> Result<[f32; 4], String> {
            regions
                .iter()
                .find(|r| r.name == n)
                .map(|r| r.bounds)
                .ok_or_else(|| format!("Building panel region {n}"))
        };
        let modes = [
            region("P_MAIN_ICON2")?,
            region("P_MAIN_ICON3")?,
            region("P_MAIN_ICON4")?,
            region("P_MAIN_ICON5")?,
            region("P_MAIN_ICON6")?,
            region("P_MAIN_OVERLAY_TOGGLE2")?,
        ];
        let mut title_text_bounds = region("P_MAIN_TITLE_AREA1")?;
        title_text_bounds[0] += 10.;
        title_text_bounds[2] += 10.;
        title_text_bounds[3] = region("P_MAIN_TITLE_AREA2")?[3];
        let bounds = region("P_MAIN_MAIN_WINDOW")?;
        let textures = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect::<Vec<_>>();
        let font = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let read =
            |n: &str| fs::read(installer.join("Standard/fonts").join(n)).map_err(|e| e.to_string());
        let layout = Layout::new(bounds, [textures[76].width, textures[76].height]);
        let information = ui::mpp_modes(bounds, [textures[13].width, textures[13].height])?;
        let detail_buttons = {
            let w = textures[97].width as f32;
            let h = textures[97].height as f32;
            let half = (textures[97].width / 2) as f32;
            let y = bounds[1] + 140.;
            let l = bounds[0] + w + half;
            let r = bounds[2] - w - w - half;
            [[l, y, l + w, y + h], [r, y, r + w, y + h]]
        };
        Ok(Self {
            base,
            title_backgrounds,
            textures,
            fonts: [
                font.textures[6..8].iter().map(Surface::new).collect(),
                font.textures[4..6].iter().map(Surface::new).collect(),
                font.textures[0..2].iter().map(Surface::new).collect(),
            ],
            tabs: [
                read("tahoma10bldnoantip.tab")?,
                read("tahoma11noantip.tab")?,
                read("cust640p.tab")?,
            ],
            title_fonts: font.textures[2..4].iter().map(Surface::new).collect(),
            title_tab: read("tahoma12bldnoantip.tab")?,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            modes,
            bounds,
            title_text_bounds,
            detail_buttons,
            information,
            layout,
            color: u32::from_le_bytes(image.at(0x536288, 4)?.try_into().unwrap()),
            palette: image
                .at(0x536288, 17 * 4)?
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .collect(),
            //4406a0 measures font3's W, then adds the explicit +144 gap2.
            report_line_height: i32::from_le_bytes(
                read("tahoma10bldnoantip.tab")?[4 + 55 * 44 + 36..4 + 55 * 44 + 40]
                    .try_into()
                    .unwrap(),
            ) + 2,
        })
    }
    pub fn bounds(&self) -> [f32; 4] {
        self.bounds
    }
    pub fn contains(&self, p: [f32; 2]) -> bool {
        hit([0., 0., 196., 320.], p)
    }
    /// Main panel frame 9's P_MAIN_OVERLAY_TOGGLE2, the building view's
    /// Go To Block control. Tutorial 2 enables its native control id 14.
    pub fn go_to_block_contains(&self, p: [f32; 2]) -> bool {
        hit(self.modes[5], p)
    }
    fn money(&self, v: f32) -> Result<Vec<u8>, String> {
        ui::formatted_float_money(
            v,
            &self.strings[2243],
            self.strings[2244].first() != Some(&0x4c),
        )
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        state: &View,
        name: u32,
    ) -> Result<PreparedScene, String> {
        let mut surfaces = self.base.clone();
        let title = state
            .inspection
            .as_ref()
            .and_then(|data| data.owner_character)
            .and_then(|character| self.title_backgrounds.get(character as usize + 1))
            .unwrap_or(&self.title_backgrounds[0]);
        surfaces.push(title.clone());
        // Shared MPP mode2 title icon: 41edb0 submits GenericTextures slot22
        // at integer logical (35,5), independent of owner selection.
        if state.building.is_some() {
            let mut title_icon = self.textures[22].clone();
            let icon_bounds = [
                35.,
                5.,
                35. + title_icon.width as f32,
                5. + title_icon.height as f32,
            ];
            quad(&mut title_icon, icon_bounds, 0xffffffff);
            surfaces.push(title_icon);
        }
        let mut font = self.fonts[0].clone();
        let mut buttons = self.fonts[1].clone();
        let mut title_font = self.title_fonts.clone();
        let mut graph_font = self.fonts[2].clone();
        let text = |pages: &mut [Surface], bytes: &[u8], rect, right| {
            ui::text_fit(pages, &self.tabs[0], bytes, rect, right, self.color)
        };
        ui::text_widget_style4(
            &mut title_font,
            &self.title_tab,
            self.strings
                .get(name as usize)
                .ok_or("Building name language missing")?,
            self.title_text_bounds,
            self.color,
        )?;
        if let Some(data) = &state.inspection {
            for (i, texture) in [66, 25, 67, 74, 23, 138].into_iter().enumerate() {
                let enabled = data.permissions.modes[i];
                let mut backing = self.textures[if i == 5 {
                    111
                } else if state.mode == i {
                    193
                } else {
                    191
                }]
                .clone();
                quad(
                    &mut backing,
                    self.modes[i],
                    if enabled { 0xffffffff } else { 0xff606060 },
                );
                surfaces.push(backing);
                let mut icon = self.textures[texture].clone();
                quad(
                    &mut icon,
                    self.modes[i],
                    if enabled { 0xffffffff } else { 0xff606060 },
                );
                surfaces.push(icon);
            }
            if state.mode == 0 {
                // Shared MPP constructor 40dbd2 enables this text widget's
                // native green background before its heading glyphs.
                let mut heading_fill = Surface::solid();
                heading_fill.rect(self.information.header, [0xff01762f; 4]);
                surfaces.push(heading_fill);
                for (i, icon) in [292, 67].into_iter().enumerate() {
                    let phase = if state.info == i || state.pressed == Some(100 + i) {
                        15
                    } else if state.hovered == Some(100 + i) {
                        14
                    } else {
                        13
                    };
                    let mut button = self.textures[phase].clone();
                    quad(&mut button, self.information.buttons[i], 0xffffffff);
                    surfaces.push(button);
                    let mut icon = self.textures[icon].clone();
                    quad(
                        &mut icon,
                        ui::mpp_icon(self.information.buttons[i])?,
                        0xffffffff,
                    );
                    surfaces.push(icon);
                }
                text(
                    &mut font,
                    &self.strings[[1162, 1164][state.info]],
                    self.information.header,
                    false,
                )?;
                if state.info == 1 {
                    //408c50 uses the separate+8c list: top+40, font3 offset3.
                    let values = [
                        (1118, self.money(data.base_value)?),
                        (
                            93,
                            data.rent_age.to_string().bytes().map(|b| b - 32).collect(),
                        ),
                        (
                            if data.owned { 1413 } else { 1414 },
                            data.trade_price
                                .map(|v| self.money(v))
                                .transpose()?
                                .unwrap_or_default(),
                        ),
                    ];
                    for (row, (label, value)) in values.into_iter().enumerate() {
                        let y = self.bounds[1] + 43. + row as f32 * self.report_line_height as f32;
                        text(
                            &mut font,
                            &self.strings[label],
                            [self.bounds[0], y, self.bounds[2], self.bounds[1] + 140.],
                            false,
                        )?;
                        ui::text_fit(
                            &mut font,
                            &self.tabs[0],
                            &value,
                            [
                                self.bounds[0],
                                y,
                                self.bounds[2] - 4.,
                                self.bounds[1] + 140.,
                            ],
                            true,
                            self.palette[9],
                        )?;
                    }
                }
            }
            if state.mode == 0 && state.info == 0 {
                use openmt2001::business_report::Value;
                //430210 appends one literal-space row to both text columns.
                //The class2 report exits after three rows; normal reports have
                //one leading blank and a blank before the three account totals.
                let summary = data.report.summary.iter().enumerate().map(|(i, row)| {
                    (
                        if data.report.expenses.is_empty() || i < 4 {
                            i + 1
                        } else {
                            i + 2
                        },
                        row,
                    )
                });
                let detail = match state.detail {
                    1 => data.report.income.as_slice(),
                    2 => data.report.expenses.as_slice(),
                    _ => &[],
                }
                .iter()
                .enumerate()
                .map(|(i, row)| (i + 12, row));
                let mut extra_rows = 0usize;
                let mut summary_overflows = 0usize;
                for (slot, row) in summary.chain(detail) {
                    let value = match &row.value {
                        Value::Language(id) => self.strings[*id as usize].clone(),
                        Value::Owner(0) => self.strings[1116].clone(),
                        Value::Owner(_) => data
                            .owner_name
                            .iter()
                            .copied()
                            .take_while(|b| *b != 0)
                            .map(|b| b - 32)
                            .collect(),
                        Value::Money(v) => self.money(*v)?,
                        Value::IntegerMoney(v) => ui::integer_money(
                            *v,
                            &self.strings[2243],
                            self.strings[2244].first() != Some(&0x4c),
                        ),
                        Value::Number(v) => format!("{v:.6}").bytes().map(|b| b - 32).collect(),
                        Value::Integer(v) => {
                            (*v as i32).to_string().bytes().map(|b| b - 32).collect()
                        }
                        Value::Empty => Vec::new(),
                    };
                    let y = self.bounds[1]
                        + 33.
                        + (slot + extra_rows) as f32 * self.report_line_height as f32;
                    text(
                        &mut font,
                        &self.strings[row.label as usize],
                        [self.bounds[0], y, self.bounds[2], self.bounds[3]],
                        false,
                    )?;
                    let wrap = openmt2001::business_report::text::value_wrap(
                        [self.bounds[0], self.bounds[2]],
                        1,
                        [
                            ui::text_width(&self.tabs[0], &self.strings[row.label as usize], 1.)?,
                            ui::text_width(&self.tabs[0], &value, 1.)?,
                        ],
                    )?;
                    let value_y = y + if wrap {
                        self.report_line_height as f32
                    } else {
                        0.
                    };
                    let style = if row.label == 1412 {
                        data.report.highlight
                    } else {
                        [9, 0, 0]
                    };
                    let color = self.palette[usize::from(
                        openmt2001::business_report::text::palette_index(style, state.logic_tick),
                    )];
                    ui::text_fit(
                        &mut font,
                        &self.tabs[0],
                        &value,
                        [self.bounds[0], value_y, self.bounds[2] - 4., self.bounds[3]],
                        true,
                        color,
                    )?;
                    if wrap {
                        extra_rows += 1;
                        if slot < 12 {
                            summary_overflows += 1;
                        }
                    }
                }
                if matches!(state.detail, 1 | 2) {
                    let header = if state.detail == 1 { 65 } else { 72 };
                    //The explicit type1 list header is separate from widget+24.
                    let y = self.bounds[1]
                        + 33.
                        + (11 + summary_overflows) as f32 * self.report_line_height as f32;
                    ui::text_fit(
                        &mut font,
                        &self.tabs[0],
                        &self.strings[header],
                        [self.bounds[0], y, self.bounds[2], self.bounds[3]],
                        false,
                        self.palette[10],
                    )?;
                }
            }
            if state.mode == 0
                && state.info == 0
                && state.detail == 0
                && !data.report.expenses.is_empty()
            {
                let graph = &data.graph;
                let mut plot = Surface::solid();
                let [l, t, w, h] = graph.rect;
                plot.rect([l, t, l + w, t + h], [0xff000000; 4]);
                for item in graph.axes()? {
                    use openmt2001::business_report::AxisItem;
                    match item {
                        AxisItem::Rectangle { bounds, color } => plot.rect(bounds, [color; 4]),
                        AxisItem::Label {
                            position,
                            value,
                            format,
                            mode,
                            flags,
                        } => {
                            let glyphs = match (mode, format) {
                                (0, 0) => format!("{value:.0}")
                                    .bytes()
                                    .map(|b| b - 32)
                                    .collect::<Vec<_>>(),
                                (1, _) => ui::integer_money(
                                    openmt2001::x87::Sum::from_f32(value)?.ftol_low_word(),
                                    &self.strings[2243],
                                    self.strings[2244].first() != Some(&0x4c),
                                ),
                                _ => return Err("unsupported original graph label format".into()),
                            };
                            //42ce80 centers using integral half-width/half-height;
                            //4e75f0 measures max(glyph height+offset)-min(offset).
                            let width = ui::text_width(&self.tabs[2], &glyphs, 1.)?;
                            let mut low = 999i32;
                            let mut high = 0i32;
                            for &glyph in &glyphs {
                                let at = 4 + usize::from(glyph) * 44;
                                let height = i32::from_le_bytes(
                                    self.tabs[2][at + 36..at + 40].try_into().unwrap(),
                                );
                                let offset = i32::from_le_bytes(
                                    self.tabs[2][at + 40..at + 44].try_into().unwrap(),
                                );
                                low = low.min(offset);
                                high = high.max(offset + height);
                            }
                            let half = [width / 2, (high - low) / 2];
                            let mut origin =
                                [position[0] - half[0] as f32, position[1] - half[1] as f32];
                            for (bit, axis, delta) in [
                                (1, 0, half[0] + 6),
                                (2, 0, -half[0] - 6),
                                (4, 1, -half[1] - 2),
                                (8, 1, half[1] + 2),
                            ] {
                                if flags & bit != 0 {
                                    origin[axis] += delta as f32;
                                }
                            }
                            ui::text(
                                &mut graph_font,
                                &self.tabs[2],
                                &glyphs,
                                [origin[0], origin[1], 640., 480.],
                                false,
                                0xffffffff,
                            )?;
                        }
                    }
                }
                for vertices in graph.stroke()? {
                    plot.quad(vertices, [[0., 0.]; 4], [0x9fbf0000; 4]);
                }
                surfaces.push(plot);
            }
            if state.mode == 0 && state.info == 0 && !data.report.expenses.is_empty() {
                for (i, rect) in self.detail_buttons.iter().enumerate() {
                    let texture = 97
                        + i * 4
                        + if state.pressed == Some(7 + i) {
                            2
                        } else if state.hovered == Some(7 + i) {
                            1
                        } else {
                            0
                        };
                    let mut arrow = self.textures[texture].clone();
                    quad(&mut arrow, *rect, 0xffffffff);
                    surfaces.push(arrow);
                }
                text(
                    &mut font,
                    &self.strings[1502 + state.detail],
                    [
                        self.detail_buttons[0][2],
                        self.bounds[1] + 138.,
                        self.detail_buttons[1][0],
                        self.bounds[1] + 154.,
                    ],
                    false,
                )?;
            }
            let sell = data.owned;
            let (label, price, enabled) = match state.mode {
                2 => (
                    if sell { 1416 } else { 1415 },
                    data.trade_price,
                    data.permissions.trade.is_some(),
                ),
                3 => (
                    85,
                    state
                        .selected
                        .and_then(|i| data.conversions.get(i))
                        .map(|v| v.price),
                    data.permissions.convert && state.selected.is_some(),
                ),
                4 => (88, data.demolition_price, data.permissions.demolish),
                _ => (0, None, false),
            };
            if matches!(state.mode, 2..=4) {
                if matches!(state.mode, 3 | 4) || state.mode == 2 && data.owned {
                    let dialog = crate::human_dialog::Dialog {
                        body: 9,
                        arguments: [
                            crate::human_dialog::Argument::Name(data.owner_name),
                            crate::human_dialog::Argument::Language(if state.mode == 4 {
                                data.demolition_label
                            } else {
                                data.business_label
                            }),
                            crate::human_dialog::Argument::Integer(0),
                        ],
                        argument_count: 2,
                        ..Default::default()
                    };
                    let text = dialog.paragraphs(&self.strings)?.remove(0);
                    let (lines, copy) = crate::prompt::wrap_with_gap(
                        &self.tabs[0],
                        &text,
                        (self.layout.description[2] - self.layout.description[0]).trunc() as i32,
                        -1,
                    )?;
                    for (i, pair) in lines.windows(2).enumerate() {
                        let end = (pair[1] - 1).min(copy.len());
                        let start = pair[0].min(end);
                        let [l, t, r, b] = self.layout.description;
                        ui::text_fit(
                            &mut font,
                            &self.tabs[0],
                            &copy[start..end],
                            [l, t + i as f32 * 10., r, b],
                            false,
                            if state.mode == 2 {
                                self.palette[9]
                            } else {
                                self.color
                            },
                        )?;
                    }
                } else if state.mode == 2 {
                    //4085e0 writes Buy82 into+c4 when the owner is not current.
                    text(&mut font, &self.strings[82], self.layout.description, false)?;
                }

                let cost_label = if state.mode == 4 {
                    84
                } else if state.mode == 3 {
                    87
                } else if sell {
                    1413
                } else {
                    1414
                };
                if state.mode == 2 {
                    //4085e0: one445520 price in+c0, not the Info1+8c list.
                    if let Some(price) = price {
                        ui::text_fit(
                            &mut font,
                            &self.tabs[0],
                            &self.money(price)?,
                            self.layout.quote,
                            true,
                            self.palette[9],
                        )?;
                    }
                } else {
                    text(
                        &mut font,
                        &self.strings[cost_label],
                        self.layout.quote,
                        false,
                    )?;
                    if let Some(price) = price {
                        ui::text_fit(
                            &mut font,
                            &self.tabs[0],
                            &self.money(price)?,
                            self.layout.quote,
                            true,
                            self.palette[9],
                        )?;
                    }
                }
                let mut action = self.textures[if !enabled || state.action_pending.is_some() {
                    143
                } else if state.pressed == Some(6) {
                    78
                } else if state.hovered == Some(6) {
                    77
                } else {
                    76
                }]
                .clone();
                quad(&mut action, self.layout.action, 0xffffffff);
                surfaces.push(action);
                let w = ui::text_width(&self.tabs[1], &self.strings[label], 1.)? as f32;
                let [l, t, r, b] = self.layout.action;
                ui::text_fit(
                    &mut buttons,
                    &self.tabs[1],
                    &self.strings[label],
                    [(l + r - w) * 0.5, t + 2., r, b],
                    false,
                    self.color,
                )?;
                if state.mode == 3 {
                    let top = self.layout.list_top;
                    for (i, row) in data
                        .conversions
                        .iter()
                        .enumerate()
                        .skip(state.scroll)
                        .take(15)
                    {
                        let y = top + (i - state.scroll) as f32 * 10.;
                        if y + 10. > self.bounds[3] - 2. {
                            break;
                        }
                        let rect = [self.bounds[0], y, self.bounds[2] - 12., self.bounds[3]];
                        if state.selected == Some(i) {
                            let mut tint = Surface::solid();
                            tint.rect([rect[0], y, rect[2], y + 10.], [0xff01762f; 4]);
                            surfaces.push(tint);
                        }
                        text(&mut font, &self.strings[row.label as usize], rect, false)?;
                    }
                }
            }
        }
        if let Some(error) = &state.error {
            let glyphs = error
                .bytes()
                .filter(|b| *b >= 32)
                .map(|b| b - 32)
                .collect::<Vec<_>>();
            text(&mut font, &glyphs, [32., 285., 184., 313.], false)?;
        }
        surfaces.extend(font);
        surfaces.extend(buttons);
        surfaces.extend(title_font);
        surfaces.extend(graph_font);
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
#[derive(Default)]
pub struct View {
    pub building: Option<usize>,
    pub inspection: Option<Inspection>,
    pub pending: Option<mpsc::Receiver<Result<Inspection, String>>>,
    pub action_pending: Option<mpsc::Receiver<Result<bool, String>>>,
    pub mode: usize,
    /// Native controller+2c information choice, separate from green mode+1c.
    pub info: usize,
    pub detail: usize,
    pub logic_tick: u32,
    pub selected: Option<usize>,
    pub scroll: usize,
    pub action: Option<openmt2001::business_controls::Action>,
    pub navigation: Option<usize>,
    pub error: Option<String>,
    pressed: Option<usize>,
    hovered: Option<usize>,
    tutorial_control: Option<u32>,
    tutorial_producer: Option<(u32, u32)>,
    pub refresh: bool,
}
impl View {
    pub fn select(&mut self, building: Option<usize>) {
        if self.building != building {
            *self = Self {
                building,
                refresh: building.is_some(),
                ..Self::default()
            };
        }
    }
    pub fn poll(&mut self) {
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok(i) => {
                            if !i.permissions.modes[self.mode] {
                                self.mode = 0;
                            }
                            self.inspection = Some(i);
                            self.error = None;
                        }
                        Err(e) => {
                            self.inspection = None;
                            self.error = Some(e);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                    self.error = Some("Building worker disconnected".into());
                }
            }
        }
        if let Some(rx) = &self.action_pending {
            match rx.try_recv() {
                Ok(result) => {
                    self.action_pending = None;
                    match result {
                        Ok(_) => {
                            self.refresh = true;
                            self.selected = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.action_pending = None;
                    self.error = Some("Building worker disconnected".into());
                }
            }
        }
    }
    pub fn captured(&self) -> bool {
        self.pressed.is_some()
    }
    pub fn tutorial_control_pressed(&self) -> bool {
        self.captured()
    }
    pub fn take_tutorial_control(&mut self) -> Option<u32> {
        self.tutorial_control.take()
    }
    pub fn take_tutorial_producer(&mut self) -> Option<(u32, u32)> {
        let producer = self.tutorial_producer.take();
        if producer.is_some() && self.tutorial_control == Some(7) {
            self.tutorial_control = None;
        }
        producer
    }
    /// Registered control IDs are distinct from the callbacks of list children.
    pub fn tutorial_control_at(&mut self, assets: &Assets, point: Option<[f32; 2]>) -> Option<u32> {
        self.pointer(assets, point);
        match self.hovered? {
            mode @ 0..=5 => Some(9 + mode as u32),
            6 => match self.mode {
                3 => Some(38),
                4 => Some(39),
                _ => None,
            },
            7 => Some(36),
            8 => Some(37),
            100..=101 => Some(15 + (self.hovered.unwrap() - 100) as u32),
            9.. => Some(40),
        }
    }
    pub fn pointer(&mut self, assets: &Assets, point: Option<[f32; 2]>) {
        let Some(data) = &self.inspection else {
            self.hovered = None;
            return;
        };
        let mode = (0..6)
            .find(|&i| data.permissions.modes[i] && point.is_some_and(|p| hit(assets.modes[i], p)));
        let action = self.action_pending.is_none()
            && point.is_some_and(|p| hit(assets.layout.action, p))
            && match self.mode {
                2 => data.permissions.trade.is_some(),
                3 => data.permissions.convert && self.selected.is_some(),
                4 => data.permissions.demolish,
                _ => false,
            };
        let information = (self.mode == 0)
            .then(|| (0..2).find(|&i| point.is_some_and(|p| hit(assets.information.buttons[i], p))))
            .flatten();
        let detail = (self.mode == 0 && self.info == 0 && !data.report.expenses.is_empty())
            .then(|| (0..2).find(|&i| point.is_some_and(|p| hit(assets.detail_buttons[i], p))))
            .flatten();
        let row = (self.mode == 3
            && data.permissions.convert
            && data.owned
            && self.action_pending.is_none())
        .then(|| {
            point.and_then(|[x, y]| {
                let top = assets.layout.list_top;
                (x >= assets.bounds[0]
                    && x < assets.bounds[2] - 12.
                    && y >= top
                    && y < assets.bounds[3] - 2.)
                    .then(|| self.scroll + ((y - top) / 10.) as usize)
                    .filter(|&row| row < data.conversions.len())
            })
        })
        .flatten();
        self.hovered = mode
            .or(information.map(|i| 100 + i))
            .or(action.then_some(6))
            .or(detail.map(|i| 7 + i))
            .or(row.map(|i| 9 + i));
    }
    pub fn mouse(&mut self, assets: &Assets, point: Option<[f32; 2]>, down: bool) {
        self.pointer(assets, point);
        let Some(data) = &self.inspection else {
            if !down {
                self.pressed = None;
            }
            return;
        };
        if down {
            self.pressed = self.hovered;
            if self.mode == 3
                && data.permissions.convert
                && data.owned
                && self.action_pending.is_none()
                && let Some([x, y]) = point
            {
                let top = assets.layout.list_top;
                if x >= assets.bounds[0]
                    && x < assets.bounds[2] - 12.
                    && y >= top
                    && y < assets.bounds[3] - 2.
                {
                    let row = self.scroll + ((y - top) / 10.) as usize;
                    if row < data.conversions.len() {
                        self.selected = Some(row);
                        //408430 emits7 after an eligible candidate's quote is
                        //shown, including a repeated selection. The worker's
                        //conversion list already applies fit/owner/pending gates.
                        self.tutorial_control = Some(7);
                        self.tutorial_producer = Some((40, 7));
                    }
                }
            }
            return;
        }
        let prior = self.pressed.take();
        if prior.is_none() || prior != self.hovered {
            return;
        }
        match prior.unwrap() {
            0..=5 => {
                let mode = prior.unwrap();
                // Original41ff49 emits9+the admitted mode index.
                self.tutorial_control = Some(9 + mode as u32);
                if matches!(mode, 1 | 5) {
                    self.navigation = Some(mode);
                } else {
                    self.mode = mode;
                    self.selected = None;
                    self.refresh = true;
                }
            }
            7 => {
                self.detail = (self.detail + 2) % 3;
                self.tutorial_control = Some(36);
            }
            8 => {
                self.detail = (self.detail + 1) % 3;
                self.tutorial_control = Some(37);
            }
            6 => {
                self.action = match self.mode {
                    2 => Some(openmt2001::business_controls::Action::Trade),
                    3 => self
                        .selected
                        .and_then(|i| data.conversions.get(i))
                        .map(|v| openmt2001::business_controls::Action::Convert {
                            selected_label: v.label,
                        }),
                    4 => Some(openmt2001::business_controls::Action::Demolish),
                    _ => None,
                };
                if self.action.is_some() {
                    self.tutorial_control = match self.mode {
                        3 => Some(38),
                        4 => Some(39),
                        _ => None,
                    };
                }
            }
            index @ 100..=101 => {
                self.info = index - 100;
                self.tutorial_control = Some(15 + self.info as u32);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tutorial_two_tests {
    #[test]
    #[ignore = "requires original v1.4 main-panel mesh and fonts"]
    fn original_go_to_block_overlay_is_tutorial_two_target() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let executable = read("OPENMT2001_EXE");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let assets = super::Assets::load(&index, &archive, installer, &[], &executable).unwrap();
        let panel = index
            .find("gamedata2/brand_new_panels/main_panel_v2.tm")
            .unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[panel.offset as usize..(panel.offset + panel.length) as usize],
        )
        .unwrap();
        let region = openmt2001::ui_mesh::regions(&mesh, 9)
            .unwrap()
            .into_iter()
            .find(|region| region.name == "P_MAIN_OVERLAY_TOGGLE2")
            .unwrap();
        assert_eq!(assets.modes[5], region.bounds);
        assert_eq!(
            region.bounds.map(|value| value.round() as i32),
            [6, 275, 28, 297]
        );
        assert!(assets.go_to_block_contains([20., 288.]));
        assert!(!assets.go_to_block_contains([29., 288.]));
        assert!(!assets.go_to_block_contains([20., 270.]));
        let script = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Original Distribution/Monopoly Tycoon ISO 2001 Archive.org/Extracted Installer/scripts/TUTORIAL2/hub.lua")).unwrap();
        assert!(script.contains("Interface.AddArrow( 29, 288 )"));
        assert!(script.contains("Interface.EnableControl(TUT_MPP_GREENBUTTON_6)"));
    }
}

#[cfg(all(test, feature = "capture"))]
pub fn capture_inspection(data: &Inspection) {
    let Ok(path) = std::env::var("OPENMT2001_BUILDING_PANEL_CAPTURE") else {
        return;
    };
    let (assets, _) = tests::original_assets();
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        desc.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(desc)).unwrap();
    let mut state = View {
        building: Some(data.building),
        inspection: Some(data.clone()),
        selected: (!data.conversions.is_empty()).then_some(0),
        ..Default::default()
    };
    for (mode, detail) in [(0, 0), (0, 1), (0, 2), (2, 0), (3, 0), (4, 0)] {
        state.mode = mode;
        state.detail = detail;
        let panel = assets
            .prepare(&renderer, &state, data.demolition_label)
            .unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &panel,
            &format!("{path}-live-{mode}-{detail}.png"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn original_assets() -> (Assets, Vec<u8>) {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let exe = read("OPENMT2001_EXE");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameters.parent().unwrap().parent().unwrap();
        let executable = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let bytes = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets
            .chunks_exact(2)
            .map(|n| {
                let end = at + u16::from_le_bytes(n.try_into().unwrap()) as usize;
                let text = &bytes[at..end];
                at = end;
                text
            })
            .collect::<Vec<_>>();
        //4922e0 replaces currency/property placeholders before Main is shown.
        let localized = crate::tooltip::localized_strings(installer, &strings).unwrap();
        let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let assets = Assets::load(&index, &archive, installer, &strings, &exe).unwrap();
        assert_eq!(assets.strings[2243], [4], "USA currency is encoded $");
        (assets, exe)
    }
    #[test]
    #[ignore = "original Main41e40f/407b07 constructor and icon oracle"]
    fn original_business_information_layout() {
        let bytes = fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("work/economy/native-main-information-layout.bin"),
        )
        .unwrap();
        assert_eq!(&bytes[..8], b"MTMAINIF");
        let mut words = bytes[8..]
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        assert_eq!(words.next(), Some(9));
        // The tool executes both original constructor/icon paths under037f.
        for _ in 0..9 {
            let bounds = std::array::from_fn(|_| f32::from_bits(words.next().unwrap()));
            let dimensions = std::array::from_fn(|_| words.next().unwrap());
            let layout = ui::mpp_modes(bounds, dimensions).unwrap();
            assert_eq!(words.next(), Some(6)); //five buttons, then header
            for rect in layout.buttons[..5]
                .iter()
                .chain(std::iter::once(&layout.header))
            {
                for value in rect {
                    assert_eq!(value.to_bits(), words.next().unwrap());
                }
            }
            for button in &layout.buttons[..2] {
                for value in ui::mpp_icon(*button).unwrap() {
                    assert_eq!(value.to_bits(), words.next().unwrap());
                }
            }
        }
        assert_eq!(words.next(), None);
    }
    #[test]
    #[ignore = "original building constructor and panel assets"]
    fn original_building_resources() {
        let (assets, exe) = original_assets();
        let bytes =
            fs::read(std::env::var("OPENMT2001_NATIVE_BUILDING_PANEL_LAYOUT").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTBLAY01");
        let mut words = bytes[8..]
            .chunks_exact(4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()));
        assert_eq!(words.next(), Some(0x37f));
        for _ in 0..words.next().unwrap() {
            let bounds = std::array::from_fn(|_| f32::from_bits(words.next().unwrap()));
            let dims = std::array::from_fn(|_| words.next().unwrap());
            let _arrows = [words.next().unwrap(), words.next().unwrap()];
            let l = Layout::new(bounds, dims);
            for rect in [
                l.action,
                l.action,
                l.action,
                l.description,
                l.description,
                l.quote,
                l.quote,
            ] {
                for v in rect {
                    assert_eq!(v.to_bits(), words.next().unwrap());
                }
            }
        }
        assert_eq!(words.next(), None);
        let mut owner = [0; 96];
        owner[..6].copy_from_slice(b"Justin");
        let mut state = View {
            building: Some(0),
            inspection: Some(Inspection {
                building: 0,
                business_label: 128,
                demolition_label: 128,
                owned: true,
                owner_name: owner,
                owner_character: Some(0),
                permissions: openmt2001::business_permissions::Permissions {
                    modes: [true; 6],
                    trade: Some(openmt2001::business_permissions::Trade::Sell),
                    convert: true,
                    demolish: true,
                },
                graph: openmt2001::business_report::graph(
                    &[0; 544],
                    [0, 0],
                    assets.bounds,
                    &PeImage::parse(&exe).unwrap(),
                )
                .unwrap(),
                report: openmt2001::business_report::Report {
                    summary: vec![],
                    income: vec![],
                    expenses: vec![],
                    highlight: [0; 3],
                },
                base_value: 2500.,
                rent_age: 85,
                trade_price: Some(2222.25),
                demolition_price: Some(500.),
                conversions: vec![crate::business_operations::Conversion {
                    kind: 0,
                    label: 128,
                    price: 750.,
                }],
            }),
            ..Default::default()
        };
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_BUILDING_PANEL_CAPTURE") {
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                desc.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(desc)).unwrap();
            for (mode, info) in [(0, 0), (0, 1), (2, 0), (3, 0), (4, 0)] {
                state.mode = mode;
                state.info = info;
                let panel = assets.prepare(&renderer, &state, 128).unwrap();
                crate::capture_test::capture_standalone(
                    &renderer,
                    &panel,
                    &format!("{path}-{mode}-{info}.png"),
                );
            }
        }
        state.mode = 2;
        let r = assets.layout.action;
        let p = Some([(r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5]);
        state.mouse(&assets, p, true);
        state.mouse(&assets, p, false);
        assert!(matches!(
            state.action,
            Some(openmt2001::business_controls::Action::Trade)
        ));
        state.mode = 0;
        for info in [1, 0] {
            let rect = assets.information.buttons[info];
            let point = Some([(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5]);
            assert_eq!(
                state.tutorial_control_at(&assets, point),
                Some(15 + info as u32)
            );
            state.mouse(&assets, point, true);
            assert!(state.tutorial_control_pressed());
            assert_eq!(state.take_tutorial_control(), None);
            state.mouse(&assets, point, false);
            assert_eq!(state.info, info);
            assert_eq!(state.mode, 0);
            assert_eq!(state.take_tutorial_control(), Some(15 + info as u32));
        }
        let rect = assets.information.buttons[1];
        state.mouse(
            &assets,
            Some([(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5]),
            true,
        );
        state.mouse(&assets, Some([600., 450.]), false);
        assert!(!state.tutorial_control_pressed());
        assert_eq!(state.info, 0);
        assert_eq!(state.take_tutorial_control(), None);
    }
}

// Shared only by these two supplied component fixtures; no production API.
#[cfg(test)]
pub(crate) mod facing_fixture {
    use super::*;
    use std::path::PathBuf;
    pub struct Inputs {
        pub directory: Vec<u8>,
        pub archive: Vec<u8>,
        pub executable: Vec<u8>,
        pub installer: PathBuf,
        pub strings: Vec<Vec<u8>>,
    }
    fn guard(bytes: &[u8], expected: &str) {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("fixture SHA guards need sha256sum");
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success());
        assert_eq!(
            std::str::from_utf8(&result.stdout)
                .unwrap()
                .split_whitespace()
                .next(),
            Some(expected)
        );
    }
    pub fn same(a: &Surface, b: &Surface) {
        assert_eq!(a.vertices, b.vertices);
        assert_eq!(a.alpha, b.alpha);
        assert_eq!((a.width, a.height), (b.width, b.height));
        assert_eq!(a.pixels, b.pixels);
        assert!(matches!(
            a.draw().mode,
            openmt2001::render::SurfaceMode::Overlay
        ));
    }
    fn area(positions: &[[f32; 3]], ids: [usize; 3]) -> f64 {
        let [a, b, c] = ids.map(|id| [f64::from(positions[id][0]), -f64::from(positions[id][1])]);
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    #[test]
    fn own_pose_oracle_distinguishes_reversed_and_degenerate_faces() {
        let p = [[0., 0., 7.], [0., -2., 7.], [3., -2., 7.], [1.5, -1., 7.]];
        assert_eq!(area(&p, [0, 1, 2]), -6.);
        assert_eq!(area(&p, [0, 2, 1]), 6.);
        assert_eq!(area(&p, [0, 3, 2]), 0.);
        assert_eq!(area(&p, [0, 0, 1]), 0.);
        assert_eq!(area(&p.map(|[x, y, z]| [x, -y, z]), [0, 1, 2]), 6.);
        assert_eq!(
            area(&p.map(|[x, y, _]| [x + 10., y - 8., -99.]), [0, 1, 2]),
            -6.
        );
    }
    impl Inputs {
        pub fn read() -> Self {
            let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
            let directory = read("OPENMT2001_ARCHIVE_DIR");
            guard(
                &directory,
                "8ecb0d18a3896eba61db9ff6edc10869a84a64229169b52f0f7d981b81980d52",
            );
            let archive = read("OPENMT2001_ARCHIVE_BIN");
            let executable = read("OPENMT2001_EXE");
            guard(
                &executable,
                "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b",
            );
            let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
            guard(
                &fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab")).unwrap(),
                "03b9924e83bb7a8c650bd58c87bcb3d669dcdf0198bb240e5f964d434d8c62be",
            );
            guard(
                &fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab")).unwrap(),
                "7df258462744a70bce18516dd6d79239cd0fd434c7e100ce4ef8ad399d36c644",
            );
            guard(
                &fs::read(installer.join("Standard/fonts/tahoma11noantip.tab")).unwrap(),
                "0bb10874f0ccac54cb6bfba360aecb713c870d9b74325beb36404a0b683f1976",
            );
            guard(
                &fs::read(installer.join("Standard/fonts/cust640p.tab")).unwrap(),
                "082d6a0488289a0926ee3b2e279882a03c31922e6f1ec6aa75e56a6ffc1d72d5",
            );
            guard(
                &fs::read(installer.join("Standard/fonts/board/usa/tycoon.off")).unwrap(),
                "0cb07d00caba4717e0ae7ee5a757ce9e2a18eb3b3013d5812e9da485a1090f3a",
            );
            guard(
                &fs::read(installer.join("Standard/fonts/board/usa/tycoon.bin")).unwrap(),
                "42696b634f2b7517a645516317341cf40ed2d25efe6931e033f65c527a3d28e1",
            );
            guard(
                &fs::read(installer.join("scripts/DEFAULT/commoditysettings.lua")).unwrap(),
                "9dc3e30cf2fca8743611e5a0c6f2f88de51b97b584d9ad65ba6c51573500cdde",
            );
            let executable_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let language = std::env::var_os("OPENMT2001_FACING_LANGUAGE")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    executable_path
                        .parent()
                        .unwrap()
                        .parent()
                        .unwrap()
                        .join("US/language/usa")
                });
            let offsets = fs::read(language.join("tycoon.off")).unwrap();
            let bytes = fs::read(language.join("tycoon.bin")).unwrap();
            guard(
                &offsets,
                "54a2c8adf44e37138bcaf36bf9c9218e0543880e308a0b9cc9a883ee98265b4d",
            );
            guard(
                &bytes,
                "4d5e302035f3579d2c0cb8cd35a58abae5ea6d8159e6c3661bef091fedff6c38",
            );
            let mut at = 0;
            let raw = offsets
                .chunks_exact(2)
                .map(|n| {
                    let end = at + u16::from_le_bytes(n.try_into().unwrap()) as usize;
                    let row = &bytes[at..end];
                    at = end;
                    row
                })
                .collect::<Vec<_>>();
            assert_eq!(at, bytes.len());
            let strings = crate::tooltip::localized_strings(&installer, &raw).unwrap();
            let result = Self {
                directory,
                archive,
                executable,
                installer,
                strings,
            };
            for (name, hash) in [
                (
                    "gamedata2/brand_new_panels/main_panel_v2.tm",
                    "b405f1b5146fdc6ea76a13984a7cd97f54c13ce41cb26f752c4ffec62549eab1",
                ),
                (
                    "1New_Maya_grey_border.tt",
                    "31a3aafc028c5b3798eb8fb8eca314c1f2cf4e88b0fd3af3204d9c004396cd9f",
                ),
                (
                    "1New_Maya_alphablack50.tt",
                    "7b53dfc9bd46c56b5ce2082645586fa328c46590a86e9a79be676393bded0f92",
                ),
                (
                    "gamedata2/GenericTextures640/textures.gtd",
                    "10a278bd4ed76076f5b517069f1c8a96ddc7a8b3bb282ff8998842fa8beb88f4",
                ),
                (
                    "gamedata2/font640/textures.gtd",
                    "21ed8e328fc95d98eb8cf6f99927d2dc28bd7b3bd309b67bbfbbec52f9be5912",
                ),
            ] {
                guard(result.payload(name), hash);
            }
            result
        }
        pub fn index(&self) -> ArchiveIndex {
            ArchiveIndex::parse(&self.directory, self.archive.len() as u64).unwrap()
        }
        pub fn payload(&self, name: &str) -> &[u8] {
            let index = self.index();
            let e = index.find(name).unwrap();
            &self.archive[e.offset as usize..(e.offset + e.length) as usize]
        }
        pub fn open_borders(&self, actual: &[Surface]) -> Vec<openmt2001::ui_mesh::Region> {
            assert_eq!(
                actual.len(),
                9,
                "only nine front-facing borders, in source order"
            );
            let mesh =
                CachedMesh::parse(self.payload("gamedata2/brand_new_panels/main_panel_v2.tm"))
                    .unwrap();
            assert_eq!(
                (mesh.vertex_count, mesh.parts.len(), mesh.frames),
                (196, 49, 10)
            );
            let p = mesh.positions(9).unwrap();
            let uv = mesh.uvs();
            let regions = openmt2001::ui_mesh::regions(&mesh, 9).unwrap();
            assert_eq!(regions.len(), 49);
            assert_eq!(
                regions.iter().map(|r| r.part).collect::<Vec<_>>(),
                (0..49).collect::<Vec<_>>()
            );
            let (_, bounds) = ui::panel_frame_base(
                &self.index(),
                &self.archive,
                "gamedata2/brand_new_panels/main_panel_v2.tm",
                &(0..9).collect::<Vec<_>>(),
                9,
            )
            .unwrap();
            for (i, r) in regions.iter().enumerate() {
                let ids = mesh.parts[r.part].vertex_indices().collect::<Vec<_>>();
                let expected = [
                    ids.iter().map(|&j| p[j][0]).fold(f32::INFINITY, f32::min),
                    ids.iter().map(|&j| -p[j][1]).fold(f32::INFINITY, f32::min),
                    ids.iter()
                        .map(|&j| p[j][0])
                        .fold(f32::NEG_INFINITY, f32::max),
                    ids.iter()
                        .map(|&j| -p[j][1])
                        .fold(f32::NEG_INFINITY, f32::max),
                ];
                assert_eq!(r.bounds, expected);
                assert_eq!(bounds[i], expected);
            }
            let mut counts = [0usize; 2];
            for (i, part) in mesh.parts[..12].iter().enumerate() {
                let ids = part.vertex_indices().collect::<Vec<_>>();
                for t in ids.chunks_exact(3) {
                    let a = area(&p, t.try_into().unwrap());
                    assert!(a.is_finite() && a != 0.);
                    if i < 9 {
                        assert!(a < 0.);
                        counts[0] += 1;
                    } else {
                        assert!(a > 0.);
                        counts[1] += 1;
                    }
                }
                let tex =
                    TextureSet::parse(self.payload(mesh.texture_names[part.texture as usize]))
                        .unwrap();
                if i < 9 {
                    let mut expected = Surface::new(&tex.textures[0]);
                    for j in ids {
                        expected
                            .vertices
                            .push([p[j][0], -p[j][1], 0.5, 1., 1., 1., uv[j][0], uv[j][1]]);
                        expected.alpha.push(255);
                    }
                    same(&actual[i], &expected);
                } else {
                    assert!(
                        tex.textures[0]
                            .texels
                            .chunks_exact(4)
                            .all(|p| p == [0, 0, 0, 137])
                    );
                }
            }
            assert_eq!(counts, [18, 6]);
            for (i, name) in [
                (9, "P_MAIN_MAIN_WINDOW"),
                (10, "P_MAIN_VERTCAL_BAR_BACKING"),
                (11, "P_MAIN_TITLE_AREA_BACKING"),
            ] {
                assert_eq!(regions[i].name, name);
            }
            regions
        }
    }
    #[test]
    #[ignore = "SHA-pinned supplied mesh/material/PE/font/language assets; CPU component only"]
    fn supplied_business_constructor_keeps_front_borders_and_all_owner_palettes() {
        let f = Inputs::read();
        let index = f.index();
        let strings = f.strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let assets =
            Assets::load(&index, &f.archive, &f.installer, &strings, &f.executable).unwrap();
        let regions = f.open_borders(&assets.base);
        assert_eq!(assets.bounds, regions[9].bounds);
        let mesh =
            CachedMesh::parse(f.payload("gamedata2/brand_new_panels/main_panel_v2.tm")).unwrap();
        let image = PeImage::parse(&f.executable).unwrap();
        assert_eq!(assets.title_backgrounds.len(), 11);
        for (i, actual) in assets.title_backgrounds.iter().enumerate() {
            let colors = if i == 0 {
                [0xff808080, 0xff808080, 0x55808080, 0x55808080]
            } else {
                let c = u32::from_le_bytes(
                    image
                        .at(0x541af0 + (i as u32 - 1) * 40, 4)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                let faded = (c & 0x00ff_ffff) | (((c >> 24) / 3) << 24);
                [c, c, faded, faded]
            };
            same(
                actual,
                &ui::mpp_background_title(&mesh, 9, &image, Some(colors)).unwrap(),
            );
        }
    }
    #[cfg(feature = "capture")]
    pub fn capture(surfaces: &[Surface], path: &str) {
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            d.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(d)).unwrap();
        let view = [
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
        let mut checker = Surface::solid();
        for row in 0..15 {
            for col in 0..20 {
                let c = if (row + col) % 2 == 0 {
                    0xfff04020
                } else {
                    0xff1040e0
                };
                checker.rect(
                    [
                        col as f32 * 32.,
                        row as f32 * 32.,
                        (col + 1) as f32 * 32.,
                        (row + 1) as f32 * 32.,
                    ],
                    [c; 4],
                );
            }
        }
        let underlay = renderer.prepare(&[checker.draw()], view, None).unwrap();
        let panel = renderer
            .prepare(
                &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
                view,
                None,
            )
            .unwrap();
        crate::capture_test::capture_overlay(&renderer, &underlay, &panel, path);
    }
}
