//! Host presentation of the original four planning stages. All quotes,
//! affordability and placements are checked again by the authoritative worker.
use crate::{
    build_panel::{self, Context, Request, Styles},
    fresh_city::BuildInspection,
    ui::{self, Surface},
};
use openmt2001::{
    archive::ArchiveIndex,
    render::{PreparedScene, Renderer},
    render_scene::{self, OriginalAssets},
    texture::TextureSet,
};
use std::{fs, path::Path, sync::mpsc};

pub struct Assets {
    base: Vec<Surface>,
    textures: Vec<Surface>,
    fonts: [Vec<Surface>; 3],
    tabs: [Vec<u8>; 3],
    strings: Vec<Vec<u8>>,
    styles: Vec<Styles>,
    order: [u32; 45],
    block_titles: [Option<u32>; 52],
    special_residential_groups: [[i32; 4]; 52],
    text_colors: [u32; 17],
    business_list_color: u32,
    pub entry_bounds: [f32; 4],
    input_bounds: [f32; 4],
}
const CANCEL: [f32; 4] = [118.8038, 264.6, 178.8038, 288.6];
const ACCEPT: [f32; 4] = [40.4, 264.6, 100.4, 288.6];
const TYPE: [[f32; 4]; 3] = [
    [30.4, 43.6, 166.8038, 57.6],
    [30.4, 71.6, 166.8038, 85.6],
    [30.4, 99.6, 166.8038, 113.6],
];
const QUALITY: [[f32; 4]; 3] = [
    [40.4, 170.6, 56.4, 186.6],
    [90.4, 170.6, 106.4, 186.6],
    [140.4, 170.6, 156.4, 186.6],
];
const FLOOR_SLIDER: [f32; 4] = [30., 142., 188., 158.];
const STYLE_SLIDER: [f32; 4] = [30., 200., 188., 216.];
const STEPS: [[f32; 4]; 4] = [
    [5.6, 25.6, 28.8, 48.8],
    [5.6, 54.6, 28.8, 77.8],
    [5.6, 81.2, 28.8, 104.4],
    [5.6, 109.4, 28.8, 132.6],
];
// Native 420f20 constructor text-widget rectangles. The captions sit below
// their stage-circle tops; using STEPS[i][1] shifts every caption upward.
const STEP_TITLES: [[f32; 4]; 4] = [
    [30.4, 29.6, 188.8038, 43.6],
    [30.4, 57.6, 188.8038, 71.6],
    [30.4, 85.6, 188.8038, 99.6],
    [30.4, 113.6, 188.8038, 127.6],
];
const STEP_CHOICES: [[f32; 4]; 3] = [
    [30.4, 43.6, 188.8038, 57.6],
    [30.4, 71.6, 188.8038, 85.6],
    [30.4, 99.6, 188.8038, 113.6],
];
const STEP_BACK: [[f32; 4]; 3] = [
    [168.8038, 45., 180.8038, 58.],
    [168.8038, 73., 180.8038, 85.],
    [168.8038, 101., 180.8038, 113.],
];
// Original422fd0/43f510, frame9 body; font3 native4406a0 height9,
// default interline gap0. Its91px list contains ten complete rows.
const BUSINESS_ROWS: usize = 10;
const BUSINESS_ROW_TOP: f32 = 74.;
const BUSINESS_ROW_HEIGHT: f32 = 9.;
const BUSINESS_LIST: [f32; 4] = [30.4, 71.6, 172.8038, 162.6];
const BUSINESS_SCROLL: [f32; 4] = [172., 71., 188., 166.];
const BUSINESS_UP: [f32; 4] = [172., 71., 188., 87.];
const BUSINESS_DOWN: [f32; 4] = [172., 150., 188., 166.];
// Native 42e4ac/42e4b5 selects font3 and cyan index9 for the Provides list.
const PROVIDES_TEXT_COLOR_INDEX: usize = 9;
fn provides_row_bounds(row: usize) -> [f32; 4] {
    // 42e000 insets the list by (2,1); style0 adds 2 to the retained -1
    // horizontal offset. 4419f0 truncates the origin and advances by font3's
    // nine-pixel line height without shrinking the glyphs.
    [33., 178. + row as f32 * 9., 186.8, 250.6]
}
fn business_row(point: [f32; 2]) -> Option<usize> {
    if !hit(BUSINESS_LIST, point) {
        return None;
    }
    // Native42f640 subtracts the list's3px glyph inset before FTOL;
    // the top inset still picks row0 because FTOL truncates toward zero.
    let row = ((point[1] - BUSINESS_LIST[1] - 3.) / BUSINESS_ROW_HEIGHT).trunc() as i32;
    (0..BUSINESS_ROWS as i32)
        .contains(&row)
        .then_some(row as usize)
}
fn business_selection(row: usize) -> ([f32; 4], u32) {
    // Original430c58/430deb:3px margin, one-pixel inset on lower edge.
    let top = BUSINESS_LIST[1] + 4. + row as f32 * BUSINESS_ROW_HEIGHT;
    (
        [
            BUSINESS_LIST[0],
            top,
            BUSINESS_LIST[2] - 1.,
            top + BUSINESS_ROW_HEIGHT - 1.,
        ],
        0xff288be1,
    )
}
const PROJECTION: [f32; 16] = [
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
pub struct PreviewFiles<'a> {
    pub build_file: &'a [u8],
    pub executable: &'a [u8],
    pub directory: &'a [u8],
    pub archive: &'a [u8],
    pub suncols: &'a [u8],
    pub phase: u32,
}
fn hit(rect: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= rect[0] && p[0] <= rect[2] && p[1] >= rect[1] && p[1] <= rect[3]
}
fn type_text(kind: u32) -> u32 {
    [860, 859, 861][kind as usize]
}
// Native 420f20 assigns index16 via 440ad0 to stage titles and the
// adjustment/cost captions, including Provides at 421a1f. Selected values
// retain index0; the Provides list has its own cyan treatment.
fn planner_text_color_index(text_id: u32) -> usize {
    match text_id {
        1512..=1516 | 1518 | 1520..=1523 => 16,
        _ => 0,
    }
}
fn word(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(row[at..at + 4].try_into().unwrap())
}
fn rect(surface: &mut Surface, [l, t, r, b]: [f32; 4], color: u32) {
    surface.quad(
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
        let name = "gamedata2/brand_new_panels/main_panel_v2.tm";
        let (mut base, regions) =
            ui::panel_frame_base(index, archive, name, &(0..9).collect::<Vec<_>>(), 9)?;
        let payload = |name: &str| -> Result<&[u8], String> {
            let entry = index
                .find(name)
                .ok_or_else(|| format!("Missing planning asset {name}"))?;
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Planning asset bounds".into())
        };
        let image = openmt2001::pe::PeImage::parse(exe)?;
        let mesh = openmt2001::mesh::CachedMesh::parse(payload(name)?)?;
        base.push(ui::mpp_background(&mesh, 9, &image)?);
        let textures = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect();
        let font = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        // Constructor440b70 selects font1(title), font3(body), and font0
        // for the three Build Quality labels (widget IDs33/35/37).
        let fonts = [
            font.textures[2..4].iter().map(Surface::new).collect(),
            font.textures[6..8].iter().map(Surface::new).collect(),
            font.textures[0..2].iter().map(Surface::new).collect(),
        ];
        let tabs = [
            "tahoma12bldnoantip.tab",
            "tahoma10bldnoantip.tab",
            "cust640p.tab",
        ]
        .map(|name| {
            fs::read(installer.join("Standard/fonts").join(name))
                .map_err(|e| format!("{name}: {e}"))
        });
        let [title, body, quality] = tabs;
        let styles = (1930..=2000)
            .step_by(10)
            .map(|year| {
                let source = fs::read_to_string(installer.join(format!("parameters/{year}.prm")))
                    .map_err(|e| e.to_string())?;
                Styles::from_parameters(&source)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let order = std::array::from_fn(|i| word(image.at(0x53bdc4, 180).unwrap(), i * 4));
        let mut block_titles = [None; 52];
        for block in openmt2001::board::Board::from_v14(&image)?.blocks {
            *block_titles
                .get_mut(block.id)
                .ok_or("Planning block outside category range")? = Some(block.text_id);
        }
        let mut special_residential_groups = [[-1; 4]; 52];
        for (identity, members) in special_residential_groups.iter_mut().enumerate() {
            *members = build_panel::special_residential_group(&image, identity as u32)?;
        }
        Ok(Self {
            base,
            textures,
            fonts,
            tabs: [title?, body?, quality?],
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            styles,
            order,
            block_titles,
            special_residential_groups,
            text_colors: {
                let colors = image.at(0x536288, 17 * 4)?;
                std::array::from_fn(|i| word(colors, i * 4))
            },
            // 423156/423160 set color index9 on both native list widgets.
            business_list_color: word(image.at(0x536288 + 9 * 4, 4)?, 0),
            entry_bounds: regions[16],
            input_bounds: openmt2001::ui_mesh::parent_bounds(&mesh, 9)?,
        })
    }
    pub fn entry(&self, renderer: &Renderer, hovered: bool) -> Result<PreparedScene, String> {
        let mut background = self.textures[if hovered { 193 } else { 191 }].clone();
        rect(&mut background, self.entry_bounds, 0xffffffff);
        let mut icon = self.textures[18].clone();
        rect(&mut icon, self.entry_bounds, 0xffffffff);
        renderer.prepare(&[background.draw(), icon.draw()], PROJECTION, None)
    }
    fn label(
        &self,
        pages: &mut [Surface],
        id: u32,
        bounds: [f32; 4],
        color: u32,
    ) -> Result<(), String> {
        ui::text_fit(
            pages,
            &self.tabs[1],
            self.strings
                .get(id as usize)
                .ok_or("Planning language ID")?,
            bounds,
            false,
            color,
        )
    }
    fn heading(&self, pages: &mut [Surface], id: u32, bounds: [f32; 4]) -> Result<(), String> {
        ui::text_widget_style4(
            pages,
            &self.tabs[1],
            self.strings
                .get(id as usize)
                .ok_or("Planning heading language ID")?,
            bounds,
            self.text_colors[planner_text_color_index(id)],
        )
    }
    pub fn open(&self, context: Context) -> Result<State, String> {
        let styles = self
            .styles
            .get(context.era as usize)
            .ok_or("Planning era outside supplied parameter files")?
            .clone();
        let type_businesses = [
            self.businesses_for(&context, 0)?,
            self.businesses_for(&context, 1)?,
            self.businesses_for(&context, 2)?,
        ];
        let businesses = type_businesses[0].clone();
        let display_rows = display_rows_for(&context, 0, &businesses)?;
        let message = match context.entry() {
            build_panel::Entry::Open => None,
            build_panel::Entry::Message { text, .. } => Some(text),
        };
        Ok(State {
            input_bounds: self.input_bounds,
            context,
            styles,
            type_businesses,
            businesses,
            display_rows,
            kind: 0,
            tutorial_build_type: -1,
            tutorial_build_use: 999,
            tutorial_activated: None,
            model_variants: 0,
            model_query_key: None,
            step: 0,
            business: None,
            preview_business: None,
            rectangle: None,
            height: 1,
            quality: 0,
            choices: [0; 3],
            scroll: 0,
            pointer: None,
            pressed: None,
            button_down: false,
            click_latch: None,
            slider_capture: None,
            drag: None,
            plate_alpha: 0x80,
            plate_delta: 0x20,
            grid: None,
            quote: None,
            preview_cache: None,
            preview_marker_cache: None,
            preview_draws: 0,
            preview_failure: None,
            preview_phase: None,
            pending: None,
            placement_choices_pending: false,
            placement_pending: None,
            quote_request: None,
            needs_quote: false,
            submit: false,
            close: false,
            message,
            diagnostic: None,
            dirty: true,
            prepared: None,
        })
    }
    fn businesses_for(&self, context: &Context, kind: u32) -> Result<Vec<u32>, String> {
        let mut businesses = Vec::new();
        let identity = word(&context.category_record, 0x8c) as usize;
        let members = self
            .special_residential_groups
            .get(identity)
            .ok_or("planning category identity leaves special residential group table")?;
        let special = build_panel::special_residential_group_owned(
            members,
            context.category_record[5],
            &context.category_owners,
        )?;
        // 4229f0 visits both opening-hour groups only for retail. Row 43
        // enters the residential list through its separate 48a770 gate.
        for hour in [9, 18].into_iter().take(if kind == 0 { 2 } else { 1 }) {
            for &id in &self.order {
                let row = context
                    .catalog
                    .row(id as usize)
                    .ok_or("Planning business index")?;
                if row[0x30] == 0 {
                    continue;
                }
                if id == 43 {
                    if kind == 1 && special {
                        businesses.push(id);
                    }
                    continue;
                }
                if word(row, 0x0c) == kind && (kind != 0 || row[0x5c] == hour) {
                    businesses.push(id);
                }
            }
        }
        Ok(businesses)
    }
    pub fn prepare(&self, renderer: &Renderer, state: &mut State) -> Result<(), String> {
        if !state.dirty {
            return Ok(());
        }
        let mut surfaces = self.base.clone();
        // Shared MPP mode3 takes the default title icon path in 41edb0.
        let mut title_icon = self.textures[20].clone();
        let title_icon_bounds = [
            35.,
            5.,
            35. + title_icon.width as f32,
            5. + title_icon.height as f32,
        ];
        rect(&mut title_icon, title_icon_bounds, 0xffffffff);
        surfaces.push(title_icon);
        let mut body = self.fonts[1].clone();
        let mut title = self.fonts[0].clone();
        let mut quality_font = if state.step == 3 && state.kind != 2 {
            self.fonts[2].clone()
        } else {
            Vec::new()
        };
        let mut panel = Surface::solid();
        let mut controls = Vec::new();
        let step_titles = [1512, 1522, 1514, 1515];
        let block_title = self
            .block_titles
            .get(state.context.category)
            .copied()
            .flatten()
            .ok_or("Planning block has no original name")?;
        ui::text_widget_style4(
            &mut title,
            &self.tabs[0],
            self.strings
                .get(block_title as usize)
                .ok_or("Planning heading")?,
            [40.403656, 5.6, 175.66975, 28.],
            self.text_colors[0],
        )?;
        // Supplied native captures show only completed/current stage circles:
        // two in Screenshots/4.jpg, all four in Screenshots/3.jpg.
        for (i, bounds) in STEPS.into_iter().enumerate().take(state.step + 1) {
            let mut background = self.textures[if i == state.step { 15 } else { 14 }].clone();
            rect(&mut background, bounds, 0xffffffff);
            surfaces.push(background);
            // Native probe records texture handles(index+1), not indices.
            let mut icon = self.textures[176 + i].clone();
            rect(
                &mut icon,
                bounds,
                if i <= state.step {
                    0xffffffff
                } else {
                    0xff777777
                },
            );
            surfaces.push(icon);
        }
        if let Some(id) = state.message {
            self.label(
                &mut body,
                id,
                [30.4, 43.6, 188.8, 160.],
                self.text_colors[0],
            )?;
        } else {
            // 422d30/422fd0 enable the text-widget background on every
            // visible stage heading. 4419f0 draws these quads before glyphs.
            for bounds in STEP_TITLES.into_iter().take(state.step + 1) {
                panel.rect(bounds, [0xff01762f; 4]);
            }
            // The completed-stage choice widgets carry their own navy fill.
            for bounds in STEP_CHOICES.into_iter().take(state.step) {
                panel.rect(bounds, [0xff000a48; 4]);
            }
            match state.step {
                0 => {
                    self.heading(&mut body, 1512, STEP_TITLES[0])?;
                    // 4233ad..4233e2 orders Retail, Residential, Landscape.
                    // Native 420f20 creates the three row widgets with all
                    // texture handles set to -1. Their bounds are hit targets,
                    // not the grey framed buttons previously drawn here.
                    for (i, id) in [860, 859, 861].into_iter().enumerate() {
                        let available = !state.type_businesses[i].is_empty();
                        self.label(
                            &mut body,
                            id,
                            TYPE[i],
                            if available {
                                self.text_colors[0]
                            } else {
                                0xff888888
                            },
                        )?;
                    }
                }
                1 => {
                    self.heading(&mut body, 1512, STEP_TITLES[0])?;
                    self.label(
                        &mut body,
                        type_text(state.kind),
                        STEP_CHOICES[0],
                        self.text_colors[0],
                    )?;
                    self.heading(&mut body, 1522, STEP_TITLES[1])?;
                    for (n, &row) in state
                        .display_rows
                        .iter()
                        .skip(state.scroll)
                        .take(BUSINESS_ROWS)
                        .enumerate()
                    {
                        let top = BUSINESS_ROW_TOP + n as f32 * BUSINESS_ROW_HEIGHT;
                        let bounds = [32., top, BUSINESS_LIST[2], BUSINESS_LIST[3]];
                        let label = match row {
                            BusinessRow::Heading(label) => {
                                // Native 430e60 paints the style1 heading on
                                // transition to the following style2 row.
                                let top = BUSINESS_LIST[1] + 2. + n as f32 * BUSINESS_ROW_HEIGHT;
                                panel.rect(
                                    [BUSINESS_LIST[0], top, BUSINESS_LIST[2], top + 10.],
                                    [0xff01762f; 4],
                                );
                                label
                            }
                            BusinessRow::Separator => continue,
                            BusinessRow::Business(id) => {
                                if state.preview_business == Some(id) {
                                    let (bounds, color) = business_selection(n);
                                    panel.rect(bounds, [color; 4]);
                                }
                                word(state.context.catalog.row(id as usize).unwrap(), 4)
                            }
                        };
                        ui::text(
                            &mut body,
                            &self.tabs[1],
                            self.strings
                                .get(label as usize)
                                .ok_or("Planning business label")?,
                            bounds,
                            false,
                            self.business_list_color,
                        )?;
                    }
                    ui::Slider {
                        bounds: BUSINESS_SCROLL,
                        vertical: true,
                        value: state.scroll as f32,
                        range: [
                            0.,
                            state.display_rows.len().saturating_sub(BUSINESS_ROWS) as f32,
                        ],
                        pointer: state.pointer,
                        pressed: state.pressed,
                    }
                    .paint(&mut panel, &mut controls, &self.textures);
                    panel.rect([30.4, 163.6, 188.8, 177.6], [0xff01762f; 4]);
                    self.heading(&mut body, 1513, [30.4, 163.6, 188.8, 177.6])?;
                    if let Some(business) = state.preview_business {
                        for (i, text_id) in state
                            .context
                            .provides_text_ids(business)?
                            .into_iter()
                            .enumerate()
                        {
                            ui::text(
                                &mut body,
                                &self.tabs[1],
                                self.strings
                                    .get(text_id as usize)
                                    .ok_or("Planning Provides language ID")?,
                                provides_row_bounds(i),
                                false,
                                self.text_colors[PROVIDES_TEXT_COLOR_INDEX],
                            )?;
                        }
                    }
                }
                2 => {
                    // The original planner retains completed stage captions
                    // and choices while the player drags the base plate.
                    for (i, id) in step_titles.into_iter().enumerate().take(3) {
                        self.heading(&mut body, id, STEP_TITLES[i])?;
                    }
                    self.label(
                        &mut body,
                        type_text(state.kind),
                        STEP_CHOICES[0],
                        self.text_colors[0],
                    )?;
                    let row = state
                        .context
                        .catalog
                        .row(state.business.ok_or("No planning business")? as usize)
                        .ok_or("Planning catalog")?;
                    self.label(
                        &mut body,
                        word(row, 4),
                        STEP_CHOICES[1],
                        self.text_colors[0],
                    )?;
                }
                _ => {
                    let row = state
                        .context
                        .catalog
                        .row(state.business.ok_or("No planning business")? as usize)
                        .ok_or("Planning catalog")?;
                    // Supplied v1.4 screenshot 3.jpg keeps all four stage captions
                    // visible above the adjustment controls. Their sublines show
                    // the choices already made in the first three stages.
                    for (i, id) in step_titles.into_iter().enumerate() {
                        self.heading(&mut body, id, STEP_TITLES[i])?;
                    }
                    self.label(
                        &mut body,
                        type_text(state.kind),
                        STEP_CHOICES[0],
                        self.text_colors[0],
                    )?;
                    self.label(
                        &mut body,
                        word(row, 4),
                        STEP_CHOICES[1],
                        self.text_colors[0],
                    )?;
                    if let Some([x0, y0, x1, y1]) = state.rectangle {
                        ascii(
                            &mut body,
                            &self.tabs[1],
                            &format!("{} * {}", x1 - x0 + 1, y1 - y0 + 1),
                            STEP_CHOICES[2],
                        )?;
                    }
                    if state.kind != 2 {
                        ui::text_widget_style0(
                            &mut body,
                            &self.tabs[1],
                            self.strings.get(1516).ok_or("Planning Floors caption")?,
                            [30.4, 127.6, 188.8038, 141.6],
                            self.text_colors[planner_text_color_index(1516)],
                        )?;
                        let floors = formatted_pair(
                            self.strings.get(1519).ok_or("Planning step count text")?,
                            state.height,
                            word(row, 0x44) as i32,
                        )?;
                        ui::text_widget_style5(
                            &mut body,
                            &self.tabs[1],
                            &floors,
                            [30.4, 127.6, 188.8, 141.6],
                            self.text_colors[0],
                        )?;
                        ui::Slider {
                            bounds: FLOOR_SLIDER,
                            vertical: false,
                            value: state.height as f32,
                            range: [word(row, 0x48) as f32, word(row, 0x44) as f32],
                            pointer: state.pointer,
                            pressed: state.pressed,
                        }
                        .paint(&mut panel, &mut controls, &self.textures);
                        ui::text_widget_style0(
                            &mut body,
                            &self.tabs[1],
                            self.strings
                                .get(1520)
                                .ok_or("Planning Build Quality caption")?,
                            [30.4, 157.6, 188.8038, 171.6],
                            self.text_colors[planner_text_color_index(1520)],
                        )?;
                        // Native child31 is a filled navy text widget behind
                        // the quality choices, even though its text is empty.
                        panel.rect([30.4, 171.6, 188.8038, 185.6], [0xff000a48; 4]);
                        for (i, id) in [1656, 1655, 1657].into_iter().enumerate() {
                            let bounds = QUALITY[i];
                            // Native 43ed20 receives texture handles 175/176 here.
                            // Handles are one-based in the original registry.
                            let mut radio =
                                self.textures[if state.quality == i { 174 } else { 175 }].clone();
                            rect(&mut radio, bounds, 0xffffffff);
                            controls.push(radio);
                            ui::text_widget_style3(
                                &mut quality_font,
                                &self.tabs[2],
                                self.strings.get(id).ok_or("Planning quality label")?,
                                [bounds[2], bounds[1], bounds[2] + 100., bounds[3]],
                                self.text_colors[0],
                            )?;
                        }
                        ui::text_widget_style0(
                            &mut body,
                            &self.tabs[1],
                            self.strings
                                .get(1518)
                                .ok_or("Planning Build Style caption")?,
                            [30.4, 186.6, 188.8038, 200.6],
                            self.text_colors[planner_text_color_index(1518)],
                        )?;
                        let count = state.design_count();
                        let style = formatted_pair(
                            self.strings.get(1519).ok_or("Planning step count text")?,
                            state.choices[state.quality] + 1,
                            count as i32,
                        )?;
                        ui::text_widget_style5(
                            &mut body,
                            &self.tabs[1],
                            &style,
                            [30.4, 186.6, 188.8, 200.6],
                            self.text_colors[0],
                        )?;
                        ui::Slider {
                            bounds: STYLE_SLIDER,
                            vertical: false,
                            value: state.choices[state.quality] as f32,
                            range: [0., count.saturating_sub(1) as f32],
                            pointer: state.pointer,
                            pressed: state.pressed,
                        }
                        .paint(&mut panel, &mut controls, &self.textures);
                    }
                    self.heading(
                        &mut body,
                        if state.kind == 2 { 1523 } else { 1521 },
                        [30.4, 213.6, 188.8, 227.6],
                    )?;
                    if let Some(quote) = &state.quote {
                        // Child39 carries the quoted amount on its own navy
                        // text-widget background in the original constructor.
                        panel.rect([30.4, 229.6, 188.8038, 243.6], [0xff000a48; 4]);
                        let amount = ui::formatted_float_money(
                            quote.total,
                            self.strings.get(2243).ok_or("Planning board currency")?,
                            self.strings.get(2244).and_then(|value| value.first()) != Some(&0x4c),
                        )?;
                        ui::text(
                            &mut body,
                            &self.tabs[1],
                            &amount,
                            [30.4, 229.6, 188.8, 243.6],
                            true,
                            self.text_colors[0],
                        )?;
                    }
                    if state.kind != 2
                        && let Some([x0, y0, x1, y1]) = state.rectangle
                    {
                        // Child25 is the filled capacity field.
                        panel.rect([30.4, 243.6, 188.8038, 257.6], [0xff000a48; 4]);
                        let area = (x1 - x0 + 1) * (y1 - y0 + 1);
                        let height = u8::try_from(state.height)
                            .map_err(|_| "Planning floor count outside byte")?;
                        let (caption, capacity) = if state.kind == 1 {
                            // 42453f..424585 uses BuildingInfo+34 and the
                            // residential building-capacity rounding path.
                            let mut record = [0u8; openmt2001::building_pool::RECORD_SIZE];
                            record[4..8].copy_from_slice(&[x0 as u8, y0 as u8, x1 as u8, y1 as u8]);
                            record[8] = height;
                            (
                                1525,
                                openmt2001::building_pool::capacity(
                                    &record,
                                    f32::from_bits(word(row, 0x34)),
                                ),
                            )
                        } else {
                            (
                                1524,
                                openmt2001::economy::stock_capacity(
                                    area,
                                    height,
                                    f32::from_bits(word(row, 0x58)),
                                ),
                            )
                        };
                        let label = formatted_integer(
                            self.strings.get(caption).ok_or("Planning capacity text")?,
                            capacity,
                        )?;
                        ui::text(
                            &mut body,
                            &self.tabs[1],
                            &label,
                            [30.4, 243.6, 188.8, 257.6],
                            true,
                            self.text_colors[0],
                        )?;
                    }
                }
            }
            // 420f20 allocates one back arrow beside each completed stage
            // choice. Native handles 98/100/99 are normal/hover/pressed.
            for bounds in STEP_BACK.into_iter().take(state.step) {
                let over = state.pointer.is_some_and(|point| hit(bounds, point));
                let index = if over && state.pressed.is_some_and(|origin| hit(bounds, origin)) {
                    98
                } else if over {
                    99
                } else {
                    97
                };
                let mut arrow = self.textures[index].clone();
                rect(&mut arrow, bounds, 0xffffffff);
                controls.push(arrow);
            }
        }
        // 420f20 registers native one-based normal/hover/disabled/pressed
        // handles: Cancel 83/85/144/84, Accept 80/82/144/81.
        for (bounds, label, normal, hover, pressed, disabled) in [
            (CANCEL, 1436, 82, 84, 83, false),
            (ACCEPT, 1511, 79, 81, 80, !state.can_submit()),
        ] {
            let over = state.pointer.is_some_and(|point| hit(bounds, point));
            let index = if disabled {
                143
            } else if over && state.pressed.is_some_and(|origin| hit(bounds, origin)) {
                pressed
            } else if over {
                hover
            } else {
                normal
            };
            let mut button = self.textures[index].clone();
            rect(&mut button, bounds, 0xffffffff);
            surfaces.push(button);
            self.label(
                &mut body,
                label,
                [bounds[0] + 8., bounds[1] + 7., bounds[2] - 4., bounds[3]],
                if disabled {
                    0xff999999
                } else {
                    self.text_colors[0]
                },
            )?;
        }
        if let Some(message) = &state.diagnostic {
            ascii(&mut body, &self.tabs[1], message, [30.4, 292., 190., 320.])?;
        }
        surfaces.push(panel);
        surfaces.extend(controls);
        surfaces.extend(title);
        surfaces.extend(body);
        surfaces.extend(quality_font);
        state.prepared = Some(renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            PROJECTION,
            None,
        )?);
        state.dirty = false;
        Ok(())
    }
}
fn ascii(pages: &mut [Surface], tab: &[u8], s: &str, bounds: [f32; 4]) -> Result<(), String> {
    let encoded = s
        .bytes()
        .map(|b| {
            if (32..=126).contains(&b) {
                b - 32
            } else {
                b'?' - 32
            }
        })
        .collect::<Vec<_>>();
    ui::text_fit(pages, tab, &encoded, bounds, false, 0xffffffff)
}
fn formatted_integer(template: &[u8], value: i32) -> Result<Vec<u8>, String> {
    // The v1.4 language row uses encoded "%d" (0x05, 'D'). Preserve its
    // localized prefix and suffix while replacing that single conversion.
    let marker = template
        .windows(2)
        .position(|pair| pair == [5, b'D'])
        .ok_or("Planning integer label lacks native placeholder")?;
    let mut text = template[..marker].to_vec();
    text.extend(value.to_string().bytes().map(|b| b - 32));
    text.extend_from_slice(&template[marker + 2..]);
    Ok(text)
}
fn formatted_pair(template: &[u8], first: i32, second: i32) -> Result<Vec<u8>, String> {
    formatted_integer(&formatted_integer(template, first)?, second)
}
fn slider_arrows(bounds: [f32; 4]) -> [[f32; 4]; 2] {
    ui::slider_arrows(bounds, false)
}
fn slider_track(bounds: [f32; 4]) -> [f32; 4] {
    ui::slider_track(bounds, false)
}
fn slider_selection(track: [f32; 4], point: f32, maximum: i32) -> i32 {
    ui::slider_selection(track, point, maximum, false)
}
fn slider_arrow_step(bounds: [f32; 4], point: [f32; 2], press: [f32; 2]) -> Option<i32> {
    slider_arrows(bounds)
        .into_iter()
        .position(|arrow| hit(arrow, point) && hit(arrow, press))
        .map(|index| if index == 0 { -1 } else { 1 })
}
#[cfg(test)]
fn slider_handle(track: [f32; 4], selection: i32, maximum: i32) -> [f32; 4] {
    ui::slider_handle(track, selection, maximum, false)
}
#[cfg(test)]
fn slider_edges(bounds: [f32; 4]) -> [[[f32; 2]; 4]; 2] {
    ui::slider_edges(bounds)
}
#[cfg(test)]
fn slider_arrow_texture(side: usize, pressed: bool, hovered: bool, enabled: bool) -> usize {
    ui::slider_arrow_texture(side, pressed, hovered, enabled, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BusinessRow {
    Heading(u32),
    Separator,
    Business(u32),
}

impl BusinessRow {
    fn business(self) -> Option<u32> {
        match self {
            Self::Business(id) => Some(id),
            Self::Heading(_) | Self::Separator => None,
        }
    }
}

fn display_rows_for(
    context: &Context,
    kind: u32,
    businesses: &[u32],
) -> Result<Vec<BusinessRow>, String> {
    if kind != 0 {
        return Ok(businesses
            .iter()
            .copied()
            .map(BusinessRow::Business)
            .collect());
    }
    // 4229f0 appends both retail hour groups through 4317f0, with a blank
    // 431860 row between them. All three rows occupy list/scroll slots.
    let mut rows = vec![BusinessRow::Heading(1526)];
    for hour in [9, 18] {
        if hour == 18 {
            rows.push(BusinessRow::Separator);
            rows.push(BusinessRow::Heading(1527));
        }
        for &id in businesses {
            let row = context
                .catalog
                .row(id as usize)
                .ok_or("Planning business index")?;
            if row[0x5c] == hour {
                rows.push(BusinessRow::Business(id));
            }
        }
    }
    Ok(rows)
}

pub struct State {
    input_bounds: [f32; 4],
    pub context: Context,
    styles: Styles,
    businesses: Vec<u32>,
    display_rows: Vec<BusinessRow>,
    type_businesses: [Vec<u32>; 3],
    kind: u32,
    tutorial_build_type: i32,
    tutorial_build_use: i32,
    tutorial_activated: Option<u32>,
    model_variants: usize,
    // The original 423c20 total is the current 868190 template-match count
    // times the selected quality bank length. Keep its query independent of
    // the slower worker quote so the caption never displays an invented zero.
    model_query_key: Option<([i32; 4], i32, u32, usize)>,
    pub step: usize,
    business: Option<u32>,
    preview_business: Option<u32>,
    rectangle: Option<[i32; 4]>,
    height: i32,
    quality: usize,
    choices: [i32; 3],
    scroll: usize,
    pointer: Option<[f32; 2]>,
    pressed: Option<[f32; 2]>,
    button_down: bool,
    click_latch: Option<u8>,
    // Original432360 event5 retains the slider until event6, including
    // samples outside its track. A release does not sample a new value.
    slider_capture: Option<usize>,
    drag: Option<u8>,
    plate_alpha: i32,
    plate_delta: i32,
    pub grid: Option<[[f32; 2]; 64]>,
    pub quote: Option<BuildInspection>,
    preview_cache: Option<PreparedScene>,
    preview_marker_cache: Option<PreparedScene>,
    preview_draws: usize,
    preview_failure: Option<String>,
    preview_phase: Option<u32>,
    pub pending: Option<mpsc::Receiver<Result<BuildInspection, String>>>,
    placement_choices_pending: bool,
    pub placement_pending: Option<mpsc::Receiver<Result<[i32; 3], String>>>,
    pub quote_request: Option<Request>,
    pub needs_quote: bool,
    pub submit: bool,
    pub close: bool,
    message: Option<u32>,
    pub diagnostic: Option<String>,
    pub dirty: bool,
    pub prepared: Option<PreparedScene>,
}
impl State {
    fn parent_hit(&self, pointer: [f32; 2]) -> bool {
        openmt2001::ui_mesh::parent_hit(self.input_bounds, pointer, [0., 0.])
    }
    pub fn take_placement_request(&mut self) -> bool {
        std::mem::take(&mut self.placement_choices_pending)
    }
    pub fn apply_placement_choices(&mut self, choices: [i32; 3]) {
        self.placement_choices_pending = false;
        self.placement_pending = None;
        self.choices = choices;
        self.model_query_key = None;
        self.step = 3;
        self.changed();
    }
    fn design_count(&self) -> usize {
        let columns = self
            .quote
            .as_ref()
            .and_then(|quote| quote.preview.as_ref())
            .map_or(self.model_variants, |preview| preview.model_variants);
        // Original423c20: styles per quality * selected template columns.
        self.styles.qualities[self.quality].len() * columns
    }
    pub fn sync_model_variants(&mut self, build_bytes: &[u8]) -> Result<(), String> {
        if self.step != 3 || self.kind == 2 {
            return Ok(());
        }
        let rectangle = self.rectangle.ok_or("Planning footprint unselected")?;
        let key = (rectangle, self.height, self.kind, self.quality);
        if self.model_query_key == Some(key) {
            return Ok(());
        }
        let request = self.request()?;
        let spec = openmt2001::scene::BuildingSpec {
            block: String::new(), // Template search depends on lot/type, not block name.
            lot: [
                u8::try_from(request.origin[0]).map_err(|_| "Planning lot X")?,
                u8::try_from(request.origin[1]).map_err(|_| "Planning lot Y")?,
                u8::try_from(request.size[0]).map_err(|_| "Planning lot width")?,
                u8::try_from(request.size[1]).map_err(|_| "Planning lot depth")?,
            ],
            height: u8::try_from(request.height).map_err(|_| "Planning height")?,
            kind: u8::try_from(request.kind).map_err(|_| "Planning kind")?,
            variation: request.packed_model,
            quality: request.quality,
            businesses: Vec::new(),
        };
        let file = openmt2001::build_file::BuildFile::parse(build_bytes)?;
        self.model_variants = openmt2001::scene::SceneBuilding::model_variant_count(&spec, &file)?;
        self.model_query_key = Some(key);
        self.dirty = true;
        Ok(())
    }
    /// Original controller+788 is consulted only by the type-click producer
    /// (424f96). Changing this restriction does not rewrite a selected form.
    pub fn set_tutorial_build_type(&mut self, kind: i32) {
        self.tutorial_build_type = kind;
    }
    /// Native planner+78c admits a use-list index when it matches, or when
    /// the retained gate is -1/999. 425034 publishes tutorial87 before this
    /// selection check, so a rejected row still has its native callback.
    pub fn set_tutorial_build_use(&mut self, use_index: i32) {
        self.tutorial_build_use = use_index;
    }
    pub fn take_tutorial_control(&mut self) -> Option<u32> {
        self.tutorial_activated.take()
    }
    pub fn tutorial_control_pressed(&self) -> bool {
        self.pressed.is_some() || self.drag.is_some()
    }
    /// Footprint input is a controller producer, not registered widget88;
    /// native88 is emitted only after a successful footprint commit.
    pub fn tutorial_drag_at(&self) -> bool {
        self.step == 2
            && self.pointer.is_some_and(|p| {
                !self.parent_hit(p)
                    && self.grid.as_ref().is_some_and(|grid| {
                        build_panel::nearest_cell(
                            &build_panel::cell_centers(grid),
                            p.map(|v| (v * 2.) as i32),
                        )
                        .ok()
                        .flatten()
                        .is_some()
                    })
            })
    }
    pub fn tutorial_control_at(&self) -> Option<u32> {
        let p = self.pointer?;
        if hit(CANCEL, p) {
            return Some(98);
        }
        if self.message.is_some() {
            return None;
        }
        if hit(ACCEPT, p) && self.can_submit() {
            return Some(97);
        }
        if self.step > 2 && hit(STEP_BACK[2], p) {
            return Some(89);
        }
        if self.step > 2 && hit(STEP_CHOICES[2], p) {
            return Some(90);
        }
        for (step, bounds) in TYPE.iter().enumerate() {
            if step < self.step && hit(*bounds, p) {
                return Some(100 + step as u32);
            }
        }
        match self.step {
            0 => TYPE
                .iter()
                .position(|r| hit(*r, p))
                .filter(|&kind| {
                    !self.type_businesses[kind].is_empty()
                        && (self.tutorial_build_type == -1
                            || self.tutorial_build_type == kind as i32)
                })
                .map(|_| 86),
            1 => self.hovered_business().map(|_| 87),
            2 => None,
            3 if self.kind != 2 => {
                if let Some(q) = QUALITY.iter().position(|r| hit(*r, p)) {
                    return Some(94 + q as u32);
                }
                if hit(FLOOR_SLIDER, p) {
                    return Some(93);
                }
                if hit(STYLE_SLIDER, p) {
                    return Some(92);
                }
                None
            }
            _ => None,
        }
    }
    pub fn reset_preview(&mut self) {
        self.preview_cache = None;
        self.preview_marker_cache = None;
        self.preview_draws = 0;
        self.preview_failure = None;
        self.preview_phase = None;
    }
    /// Quote-specific original model materials upload once; camera changes only
    /// rewrite the retained uniform buffers. A diagnostic shell is used only if
    /// the supplied original facade/roof materials cannot be resolved.
    pub fn preview(
        &mut self,
        renderer: &Renderer,
        camera: [f32; 16],
        fog: Option<openmt2001::render::EyeFog>,
        selection: &crate::selection::Assets,
        files: PreviewFiles<'_>,
    ) -> Result<Option<&PreparedScene>, String> {
        if self
            .quote
            .as_ref()
            .and_then(|q| q.preview.as_ref())
            .is_none()
        {
            return Ok(None);
        }
        if self.preview_phase != Some(files.phase) {
            self.reset_preview();
        }
        if self.preview_cache.is_none() {
            let building = self
                .quote
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .clone();
            let original = render_scene::assemble(render_scene::Request {
                build_file: files.build_file,
                source: render_scene::Source::Preview(building.clone()),
                original: Some(OriginalAssets {
                    executable: files.executable,
                    directory: files.directory,
                    archive: files.archive,
                }),
                suncols: Some(files.suncols),
                phase: Some(files.phase),
                camera: None,
                width: 1280,
                height: 960,
                animated: false,
            })
            .and_then(|scene| {
                if scene.draws.is_empty() {
                    return Err("original preview has no material surfaces".into());
                }
                let draws = scene
                    .draws
                    .iter()
                    .map(|draw| draw.borrowed())
                    .collect::<Vec<_>>();
                let prepared = renderer.prepare(&draws, camera, fog)?;
                let marker = selection
                    .preview_marker_surface(&scene)
                    .and_then(|surface| {
                        let mut draw = surface.draw();
                        draw.mode = openmt2001::render::SurfaceMode::StaticLit;
                        renderer.prepare(&[draw], camera, fog)
                    });
                Ok((prepared, draws.len(), marker))
            });
            match original {
                Ok((prepared, count, marker)) => {
                    self.preview_cache = Some(prepared);
                    match marker {
                        Ok(marker) => self.preview_marker_cache = Some(marker),
                        Err(error) => eprintln!("Original planning marker unavailable: {error}"),
                    }
                    self.preview_draws = count;
                }
                Err(error) => {
                    eprintln!("Original planning model unavailable: {error}");
                    self.preview_failure = Some(error.clone());
                    self.diagnostic = Some(format!("Original model unavailable: {error}"));
                    self.dirty = true;
                    self.preview_cache =
                        Some(diagnostic_preview(renderer, camera, fog, &building)?);
                    self.preview_draws = 1;
                }
            }
            self.preview_phase = Some(files.phase);
        }
        let prepared = self.preview_cache.as_ref().unwrap();
        for index in 0..self.preview_draws {
            renderer.update_camera(prepared, index, camera, fog)?;
        }
        if let Some(marker) = &self.preview_marker_cache {
            renderer.update_camera(marker, 0, camera, fog)?;
        }
        Ok(Some(prepared))
    }
    pub fn preview_marker(&self) -> Option<&PreparedScene> {
        self.preview_marker_cache.as_ref()
    }
    #[cfg(all(test, feature = "capture"))]
    pub fn preview_failure(&self) -> Option<&str> {
        self.preview_failure.as_deref()
    }
    pub fn project_grid(
        &mut self,
        camera: openmt2001::flyby::CameraSample,
        center: [f32; 2],
    ) -> Result<(), String> {
        use openmt2001::{camera as cpu, picking};
        let view = cpu::translated_scene_view(
            cpu::scene_view_with_up(camera.eye, camera.target, camera.up),
            [center[0], 0., center[1]],
        );
        let bounds = picking::category_bounds([])?.map(|p| cpu::transform_scene_point(view, p));
        let [near, far] = cpu::city_clip(camera.eye[1])?;
        let projection =
            picking::QueryProjection::new(1280., 960., camera.vertical_fov, near, far)?;
        self.grid =
            Some(build_panel::grid_points(&bounds)?.map(|p| projection.project_coordinates(p)));
        Ok(())
    }
    fn footprint_allowed(&self, [x0, y0, x1, y1]: [i32; 4]) -> bool {
        let Some(row) = self
            .business
            .and_then(|business| self.context.catalog.row(business as usize))
        else {
            return false;
        };
        let w = x1 - x0 + 1;
        let h = y1 - y0 + 1;
        let occupancy: &[u8; 49] = self.context.category_record[0x9c..0xcd].try_into().unwrap();
        w < 7
            && h < 7
            && w * h >= word(row, 0x38) as i32
            && w * h <= word(row, 0x3c) as i32
            && w.max(h) <= word(row, 0x40) as i32
            && openmt2001::building_pool::can_place(occupancy, [x0, y0, x1, y1])
    }
    pub fn grid_overlay(&mut self, renderer: &Renderer) -> Result<Option<PreparedScene>, String> {
        let Some(grid) = &self.grid else {
            return Ok(None);
        };
        // Stage 2 shows the dragged base plate. The later adjustment screen
        // shows the chosen building instead.
        if self.step != 2 {
            return Ok(None);
        }
        let Some(first) = self.drag else {
            return Ok(None);
        };
        let Some(pointer) = self.pointer.filter(|p| !self.parent_hit(*p)) else {
            return Ok(None);
        };
        let Some(last) = build_panel::nearest_cell(
            &build_panel::cell_centers(grid),
            pointer.map(|v| (v * 2.) as i32),
        )?
        else {
            return Ok(None);
        };
        let rectangle = build_panel::rectangle_from_cells(first, last)?;
        let [x0, y0, x1, y1] = rectangle;
        let ids = [
            x0 * 8 + y0,
            (x1 + 1) * 8 + y0,
            (x1 + 1) * 8 + y1 + 1,
            x0 * 8 + y1 + 1,
        ];
        // Original 4b48ee..4b4940 submits just the selected two-triangle
        // base plate, not 8x8 grid lines. 4c3761 selects pulsating white for
        // an eligible footprint or red for an invalid one.
        let rgb = if self.footprint_allowed(rectangle) {
            0x00ff_ffff
        } else {
            0x00ff_0000
        };
        let color = ((self.plate_alpha as u32) << 24) | rgb;
        let mut surface = Surface::solid();
        surface.quad(
            ids.map(|i| grid[i as usize].map(|v| v * 0.5)),
            [[0.; 2]; 4],
            [color; 4],
        );
        let next = self.plate_alpha + self.plate_delta;
        if !(0..=255).contains(&next) {
            self.plate_alpha = next.clamp(0, 255);
            self.plate_delta = -self.plate_delta;
        } else {
            self.plate_alpha = next;
        }
        Ok(Some(renderer.prepare(
            &[surface.draw()],
            PROJECTION,
            None,
        )?))
    }
    pub fn pointer(&mut self, point: Option<[i32; 2]>) {
        let p = point.map(|p| p.map(|v| v as f32 * 0.5));
        if p != self.pointer {
            self.pointer = p;
            if self.step == 1
                && let Some(id) = self.hovered_business()
            {
                self.preview_business = Some(id);
            }
            self.dirty = true;
        }
        self.sample_captured_slider();
    }
    /// Native generic 443360 press latch for the builder's 43ed20 children.
    /// List rows (43ec30), text captions (43e9a0), and 43f510 tracks are silent.
    pub fn pointer_click_cue(&mut self) -> bool {
        if !self.button_down || self.slider_capture.is_some() {
            self.click_latch = None;
            return false;
        }
        let target = self.pointer.and_then(|p| self.click_target(p));
        if target == self.click_latch {
            return false;
        }
        self.click_latch = target;
        target.is_some()
    }
    fn click_target(&self, p: [f32; 2]) -> Option<u8> {
        if self.placement_choices_pending || self.placement_pending.is_some() {
            return None;
        }
        if hit(CANCEL, p) {
            return Some(0);
        }
        if self.message.is_some() {
            return None;
        }
        if hit(ACCEPT, p) && self.can_submit() {
            return Some(1);
        }
        if let Some(i) = STEPS.iter().take(self.step).position(|r| hit(*r, p)) {
            return Some(2 + i as u8);
        }
        if let Some(i) = STEP_BACK.iter().take(self.step).position(|r| hit(*r, p)) {
            return Some(6 + i as u8);
        }
        match self.step {
            0 => TYPE
                .iter()
                .enumerate()
                .position(|(i, r)| {
                    hit(*r, p)
                        && !self.type_businesses[i].is_empty()
                        && (self.tutorial_build_type == -1 || self.tutorial_build_type == i as i32)
                })
                .map(|i| 9 + i as u8),
            1 => {
                if hit(BUSINESS_UP, p) {
                    Some(12)
                } else if hit(BUSINESS_DOWN, p) {
                    Some(13)
                } else {
                    None
                }
            }
            3 if self.kind != 2 => {
                if let Some(i) = QUALITY.iter().position(|r| hit(*r, p)) {
                    return Some(14 + i as u8);
                }
                for (slider, bounds) in [FLOOR_SLIDER, STYLE_SLIDER].into_iter().enumerate() {
                    if let Some(side) = slider_arrows(bounds).iter().position(|r| hit(*r, p)) {
                        return Some(17 + slider as u8 * 2 + side as u8);
                    }
                }
                None
            }
            _ => None,
        }
    }
    fn sample_captured_slider(&mut self) {
        let (Some(slider), Some(p)) = (self.slider_capture, self.pointer) else {
            return;
        };
        if slider == 2 && self.step == 1 {
            let maximum = self.display_rows.len().saturating_sub(BUSINESS_ROWS) as i32;
            let value =
                ui::slider_selection(ui::slider_track(BUSINESS_SCROLL, true), p[1], maximum, true)
                    as usize;
            if value != self.scroll {
                self.scroll = value;
                self.dirty = true;
            }
            return;
        }
        if self.step != 3 || self.kind == 2 {
            return;
        }
        if slider == 0 {
            let row = self
                .context
                .catalog
                .row(self.business.unwrap() as usize)
                .unwrap();
            let min = word(row, 0x48) as i32;
            let max = word(row, 0x44) as i32;
            let value = min + slider_selection(slider_track(FLOOR_SLIDER), p[0], max - min);
            if value != self.height {
                self.height = value;
                self.changed();
            }
        } else {
            let count = self.design_count();
            if count == 0 {
                return;
            }
            let value = slider_selection(slider_track(STYLE_SLIDER), p[0], count as i32 - 1);
            if value != self.choices[self.quality] {
                self.choices[self.quality] = value;
                self.changed();
            }
        }
    }
    pub fn scroll(&mut self, delta: i32) {
        if self.step == 1 {
            self.scroll = (self.scroll as i64 - i64::from(delta)).clamp(
                0,
                self.display_rows.len().saturating_sub(BUSINESS_ROWS) as i64,
            ) as usize;
            self.dirty = true;
        }
    }
    fn hovered_business(&self) -> Option<u32> {
        let p = self.pointer?;
        self.display_rows
            .get(self.scroll + business_row(p)?)
            .and_then(|row| row.business())
    }
    pub fn request(&self) -> Result<Request, String> {
        let business = self.business.ok_or("Planning business unselected")?;
        let rectangle = self.rectangle.ok_or("Planning footprint unselected")?;
        let form = build_panel::Form {
            category: self.context.category as i32,
            rectangle,
            height: self.height,
            quality: self.quality,
            choices: self.choices,
            variants: self.styles.qualities[self.quality].clone(),
            kind: self.kind,
            business,
            displayed_quote: 0.,
        };
        form.request(false, true, true)?
            .map_err(|e| format!("Planning form: {e:?}"))
    }
    pub fn can_submit(&self) -> bool {
        self.step == 3
            && !self.needs_quote
            && self.pending.is_none()
            && self.quote.as_ref().is_some_and(BuildInspection::allowed)
            && self.request().ok() == self.quote_request
    }
    /// Reports a completed panel widget activation. The base-plate drag and
    /// slider drags do not emit the generic UI click cue.
    pub fn mouse(&mut self, down: bool) -> bool {
        self.button_down = down;
        if !down {
            self.click_latch = None;
        }
        if self.placement_choices_pending || self.placement_pending.is_some() {
            return false;
        }
        if let Some(preview) = self.quote.as_ref().and_then(|quote| quote.preview.as_ref()) {
            self.model_variants = preview.model_variants;
        }
        let Some(p) = self.pointer else {
            self.pressed = None;
            self.drag = None;
            if !down {
                self.slider_capture = None;
            }
            return false;
        };
        self.dirty = true;
        if down {
            self.pressed = Some(p);
            if self.step == 1 && hit(ui::slider_track(BUSINESS_SCROLL, true), p) {
                self.slider_capture = Some(2);
                self.sample_captured_slider();
            }
            if self.step == 3 && self.kind != 2 && self.message.is_none() {
                self.slider_capture = [FLOOR_SLIDER, STYLE_SLIDER]
                    .iter()
                    .position(|bounds| hit(slider_track(*bounds), p));
                self.sample_captured_slider();
            }
            if self.step == 2
                && !self.parent_hit(p)
                && let Some(grid) = &self.grid
            {
                self.drag = build_panel::nearest_cell(
                    &build_panel::cell_centers(grid),
                    p.map(|v| (v * 2.) as i32),
                )
                .ok()
                .flatten();
                self.plate_alpha = 0x80;
                self.plate_delta = 0x20;
            }
            return false;
        }
        if self.slider_capture.take().is_some() {
            self.pressed = None;
            return false;
        }
        let Some(origin) = self.pressed.take() else {
            return false;
        };
        if hit(CANCEL, p) && hit(CANCEL, origin) {
            self.close = true;
            return true;
        }
        if self.message.is_some() {
            return false;
        }
        if hit(ACCEPT, p) && hit(ACCEPT, origin) && self.can_submit() {
            self.submit = true;
            self.tutorial_activated = Some(97);
            return true;
        }
        let previous_step = STEPS
            .iter()
            .position(|r| hit(*r, p) && hit(*r, origin))
            .or_else(|| STEP_BACK.iter().position(|r| hit(*r, p) && hit(*r, origin)));
        if let Some(step) = previous_step
            && step < self.step
        {
            self.step = step;
            self.quote = None;
            self.reset_preview();
            // Both native drag-back children return to footprint selection;
            // native type/use back regions emit no tutorial callback.
            if step == 2 && hit(STEP_BACK[2], p) {
                self.tutorial_activated = Some(89);
            }
            return true;
        }
        match self.step {
            0 => {
                if let Some(kind) = TYPE
                    .iter()
                    .position(|rect| hit(*rect, p) && hit(*rect, origin))
                {
                    let businesses = self.type_businesses[kind].clone();
                    if self.tutorial_build_type != -1 && self.tutorial_build_type != kind as i32 {
                        return false;
                    }
                    if !businesses.is_empty() {
                        self.kind = kind as u32;
                        self.display_rows =
                            display_rows_for(&self.context, kind as u32, &businesses)
                                .expect("validated planning catalog");
                        self.businesses = businesses;
                        self.business = None;
                        self.preview_business =
                            self.display_rows.first().and_then(|row| row.business());
                        self.rectangle = None;
                        self.scroll = 0;
                        self.quote = None;
                        self.reset_preview();
                        self.tutorial_activated = Some(86);
                        if kind != 0 && self.businesses.len() == 1 {
                            // 422c2d auto-selects the only listed business.
                            self.business = self.businesses.first().copied();
                            self.step = 2;
                        } else {
                            self.step = 1;
                        }
                        return true;
                    }
                }
            }
            1 => {
                if hit(BUSINESS_UP, p) && hit(BUSINESS_UP, origin) {
                    self.scroll(1);
                    return true;
                }
                if hit(BUSINESS_DOWN, p) && hit(BUSINESS_DOWN, origin) {
                    self.scroll(-1);
                    return true;
                }
                let row = |point: [f32; 2]| business_row(point).map(|row| self.scroll + row);
                if let Some(i) = row(p).filter(|i| Some(*i) == row(origin))
                    && let Some(id) = self.display_rows.get(i).and_then(|row| row.business())
                {
                    self.tutorial_activated = Some(87);
                    if self.tutorial_build_use != -1
                        && self.tutorial_build_use != 999
                        && self.tutorial_build_use != i as i32
                    {
                        // Original 42505b returns after event87, without
                        // selecting this Use or advancing the form.
                        return true;
                    }
                    self.business = Some(id);
                    self.preview_business = Some(id);
                    self.step = 2;
                    self.rectangle = None;
                    return true;
                }
            }
            2 => {
                if let (Some(first), Some(grid)) = (self.drag.take(), &self.grid)
                    && !self.parent_hit(p)
                    && let Ok(Some(last)) = build_panel::nearest_cell(
                        &build_panel::cell_centers(grid),
                        p.map(|v| (v * 2.) as i32),
                    )
                {
                    let rectangle = build_panel::rectangle_from_cells(first, last).unwrap();
                    if self.footprint_allowed(rectangle) {
                        self.rectangle = Some(rectangle);
                        self.height = word(
                            self.context
                                .catalog
                                .row(self.business.unwrap() as usize)
                                .unwrap(),
                            0x48,
                        ) as i32;
                        self.quality = 0;
                        self.quote = None;
                        self.needs_quote = false;
                        // Original424010 generates all three authoritative
                        // design choices before event88 and the stage3 switch.
                        self.placement_choices_pending = true;
                    } else {
                        self.diagnostic = Some("Footprint outside business limits".into());
                    }
                }
            }
            3 => {
                if self.kind == 2 {
                    return false;
                }
                if let Some(q) = QUALITY.iter().position(|r| hit(*r, p) && hit(*r, origin)) {
                    self.quality = q;
                    self.changed();
                    return true;
                }
                if let Some(delta) = slider_arrow_step(FLOOR_SLIDER, p, origin) {
                    let row = self
                        .context
                        .catalog
                        .row(self.business.unwrap() as usize)
                        .unwrap();
                    let min = word(row, 0x48) as i32;
                    let max = word(row, 0x44) as i32;
                    let height = (self.height + delta).clamp(min, max);
                    if height == self.height {
                        return true;
                    }
                    self.height = height;
                    self.changed();
                    return true;
                }
                let floor_track = slider_track(FLOOR_SLIDER);
                if hit(floor_track, p) && hit(floor_track, origin) {
                    let row = self
                        .context
                        .catalog
                        .row(self.business.unwrap() as usize)
                        .unwrap();
                    let min = word(row, 0x48) as i32;
                    let max = word(row, 0x44) as i32;
                    let height = min + slider_selection(floor_track, p[0], max - min);
                    if height == self.height {
                        return true;
                    }
                    self.height = height;
                    self.changed();
                    return true;
                }
                if let Some(delta) = slider_arrow_step(STYLE_SLIDER, p, origin) {
                    let count = self.design_count();
                    if count == 0 {
                        return false;
                    }
                    let max = count.saturating_sub(1) as i32;
                    let choice = (self.choices[self.quality] + delta).clamp(0, max);
                    if choice == self.choices[self.quality] {
                        return false;
                    }
                    self.choices[self.quality] = choice;
                    self.changed();
                    return true;
                }
                let style_track = slider_track(STYLE_SLIDER);
                if hit(style_track, p) && hit(style_track, origin) {
                    let count = self.design_count();
                    if count == 0 {
                        return false;
                    }
                    let choice =
                        slider_selection(style_track, p[0], count.saturating_sub(1) as i32);
                    if choice == self.choices[self.quality] {
                        return false;
                    }
                    self.choices[self.quality] = choice;
                    self.changed();
                    return true;
                }
            }
            _ => {}
        }
        false
    }
    fn changed(&mut self) {
        self.needs_quote = true;
        self.quote = None;
        self.reset_preview();
        self.diagnostic = None;
    }
}

fn diagnostic_preview(
    renderer: &Renderer,
    camera: [f32; 16],
    fog: Option<openmt2001::render::EyeFog>,
    building: &openmt2001::scene::SceneBuilding,
) -> Result<PreparedScene, String> {
    let layers = openmt2001::geometry::prepare(&building.layers);
    let facade = openmt2001::geometry::facade_positions(&layers);
    let roof = openmt2001::roof::mesh(&layers);
    let mut surface = Surface::solid();
    let mut emit = |positions: &[[f32; 3]], triangle: [u32; 3]| {
        for index in building.local_triangle(triangle) {
            let [x, y, z] = building.local_position(positions[index as usize]);
            surface.vertices.push([
                x + building.center[0],
                y,
                -z + building.center[1],
                0.3,
                0.95,
                0.4,
                0.,
                0.,
            ]);
            surface.alpha.push(150);
        }
    };
    for quad in 0..facade.len() / 4 {
        for triangle in [[0, 1, 3], [0, 3, 2]] {
            emit(&facade, triangle.map(|i| i + quad as u32 * 4));
        }
    }
    for triangle in roof.triangles {
        emit(&roof.positions, triangle);
    }
    let mut draw = surface.draw();
    draw.mode = openmt2001::render::SurfaceMode::StaticLit;
    renderer.prepare(&[draw], camera, fog)
}

#[cfg(test)]
fn verify_native_retail_rows(state: &State, data: &[u8]) {
    assert_eq!(&data[..8], b"MTPLROW1");
    let fixture_word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    assert_eq!(fixture_word(40), 0x037f);
    let count = fixture_word(44) as usize;
    assert_eq!(count, state.display_rows.len());
    let mut at = 48;
    for (index, row) in state.display_rows.iter().enumerate() {
        let actual = match row {
            BusinessRow::Heading(id) => (1, *id, u32::MAX),
            BusinessRow::Separator => (2, 0, u32::MAX),
            BusinessRow::Business(id) => (
                2,
                word(state.context.catalog.row(*id as usize).unwrap(), 4),
                *id,
            ),
        };
        assert_eq!(
            actual,
            (fixture_word(at), fixture_word(at + 4), fixture_word(at + 8)),
            "native retail row {index}"
        );
        at += 12;
    }
    let green_count = fixture_word(at) as usize;
    at += 4;
    assert_eq!(green_count, 2);
    for _ in 0..green_count {
        let current = fixture_word(at) as usize;
        let scroll = fixture_word(at + 4) as usize;
        let visible = current - 1 - scroll;
        assert!(matches!(
            state.display_rows[current - 1],
            BusinessRow::Heading(_)
        ));
        let top = BUSINESS_LIST[1] + 2. + visible as f32 * BUSINESS_ROW_HEIGHT;
        let actual = [BUSINESS_LIST[0], top, BUSINESS_LIST[2], top + 10.];
        for (axis, value) in actual.into_iter().enumerate() {
            assert_eq!(
                value.to_bits(),
                fixture_word(at + 8 + axis * 4),
                "native green axis {axis}"
            );
        }
        assert_eq!(fixture_word(at + 24), 0xff01762f);
        at += 28;
    }
    assert_eq!(at, data.len());
}

#[cfg(test)]
fn verify_native_list_color(assets: &Assets, data: &[u8]) {
    assert_eq!(&data[..8], b"MTPLCOL1");
    let word_at = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    assert_eq!(word_at(40), 9);
    assert_eq!(word_at(44), assets.business_list_color);
    assert_eq!(assets.business_list_color, 0xff6ed1e1);
    let mut at = 48;
    for offset in [0x13c, 0x2f0] {
        assert_eq!(word_at(at), offset);
        assert_eq!(word_at(at + 4), 3);
        assert_eq!(&data[at + 8..at + 11], &[9; 3]);
        at += 11;
    }
    assert_eq!(at, data.len());
}

#[cfg(test)]
pub fn verify_live_context(
    context: Context,
    scene: &mut openmt2001::render_scene::Scene,
    inspect: impl FnMut(Request) -> BuildInspection,
    mut placement: impl FnMut() -> [i32; 3],
) {
    #[cfg(not(feature = "capture"))]
    let _ = &mut placement;
    let mut inspect = inspect;
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let executable = read("OPENMT2001_EXE");
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let block = board
        .blocks
        .iter()
        .find(|b| b.id == context.category)
        .unwrap();
    let mut control = crate::navigation::block_entry([block.center[0], 0., block.center[1]]);
    let (eye, target) = control.tick([false; 4]);
    let camera = openmt2001::flyby::CameraSample {
        eye,
        target,
        up: [0., 1., 0.],
        vertical_fov: 45.,
    };
    scene.reframe(camera).unwrap();
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe_path = openmt2001::fixtures::path("EXE");
    let language = exe_path
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
            let s = &bytes[at..end];
            at = end;
            s
        })
        .collect::<Vec<_>>();
    let localized = crate::tooltip::localized_strings(installer, &strings).unwrap();
    let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let assets = Assets::load(&index, &archive, installer, &strings, &executable).unwrap();
    assert_eq!(assets.strings[2243], [4], "USA board currency is encoded $");
    assert_eq!(assets.strings[1521], b"4HIS\0BUILDING\0WILL\0COST\0YOU\x1a");
    assert_eq!(
        formatted_integer(&assets.strings[1524], 12).unwrap(),
        b"3TOCK\0#APACITY\0\x11\x12"
    );
    assert_eq!(
        formatted_integer(&assets.strings[1525], 12).unwrap(),
        b"4ENANT\0#APACITY\0\x11\x12"
    );
    assert_eq!(
        formatted_pair(&assets.strings[1519], 2, 9).unwrap(),
        b"\x12\0OF\0\x19",
        "native current-of-total caption stays localized"
    );
    let mut state = assets.open(context).unwrap();
    if let Some(path) = openmt2001::fixtures::optional("NATIVE_PLANNER_LIST_COLOR") {
        verify_native_list_color(&assets, &std::fs::read(path).unwrap());
    }
    if let Some(path) = openmt2001::fixtures::optional("NATIVE_PLANNER_ROWS") {
        verify_native_retail_rows(&state, &std::fs::read(path).unwrap());
    }
    assert_eq!(state.type_businesses[1], [42, 43]);
    assert_eq!(state.type_businesses[2], [44]);
    assert!(
        !state.type_businesses[0].contains(&43),
        "native Hotel row 43 requires a separate group-owned path"
    );
    assert!(!state.businesses.is_empty());
    // The native 32-row commodity table is indexed by product ID, but its
    // localized text IDs are deliberately not in product-ID order.
    for (id, full, short) in [(3, 1062, 1094), (17, 1077, 1109)] {
        let row = &state.context.commodities[id];
        assert_eq!(
            &row[..12],
            image.at(0x53be98 + id as u32 * 0x1c, 12).unwrap()
        );
        assert_eq!(word(row, 0), id as u32);
        assert_eq!(word(row, 4), full);
        assert_eq!(word(row, 8), short);
    }
    assert_eq!(state.context.catalog.row(39).unwrap()[0x4c], 1);
    assert_eq!(state.context.products[39][0][0], 17);
    assert_eq!(state.context.provides_text_ids(39).unwrap(), [1077]);
    let click = |state: &mut State, p: [i32; 2]| {
        state.pointer(Some(p));
        assert!(!state.mouse(true));
        state.mouse(false)
    };
    // Fresh 1930 category 33 owns its original whole property group, so the
    // special Hotel row is present without changing the live worker.
    let identity = word(&state.context.category_record, 0x8c) as usize;
    let members = assets.special_residential_groups[identity];
    assert_eq!(members, [27, 23, 33, -1]);
    let mut split_context = state.context.clone();
    split_context.category_owners[23] = 2;
    let split = assets.open(split_context).unwrap();
    assert_eq!(
        split.type_businesses[1],
        [42],
        "48a770 excludes Hotel when one group member has another owner"
    );
    assert!(!state.can_submit());
    // Original 424f71 chooses kinds 1/2. Residential retains its list stage
    // when the whole-group gate adds Hotel row 43; landscape has one
    // business and automatically advances to the lot-drag stage.
    for (kind, business, window, screen, point) in [
        (1, 43, [1280, 720], [307., 117.], [196, 156]),
        (1, 42, [960, 720], [147., 117.], [196, 156]),
        (2, 44, [1280, 720], [307., 159.], [196, 212]),
    ] {
        #[cfg(feature = "capture")]
        let capture_kind = if business == 43 {
            "special".to_owned()
        } else {
            format!("type{kind}")
        };
        let mapped = crate::intro::internal_pointer(screen, window).unwrap();
        assert_eq!(mapped, point, "visible native type row maps to its hitbox");
        assert!(click(&mut state, mapped));
        assert_eq!(state.kind, kind);
        if kind == 1 {
            assert_eq!(state.step, 1, "two residential choices retain list stage");
            #[cfg(feature = "capture")]
            if business == 43
                && let Ok(prefix) = std::env::var("OPENMT2001_PLANNER_CAPTURE")
            {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                assets.prepare(&renderer, &mut state).unwrap();
                crate::capture_test::capture(
                    &renderer,
                    state.prepared.as_ref(),
                    scene,
                    &format!("{prefix}-special-1-owned.png"),
                );
            }
            let row = state
                .businesses
                .iter()
                .position(|&id| id == business)
                .unwrap();
            let row_y = (f64::from(BUSINESS_ROW_TOP)
                + (row as f64 + 0.5) * f64::from(BUSINESS_ROW_HEIGHT))
                * 1.5;
            let row_screen = if window == [1280, 720] {
                [310., row_y]
            } else {
                [232.5, row_y]
            };
            let row_point = crate::intro::internal_pointer(row_screen, window).unwrap();
            assert!(
                click(&mut state, row_point),
                "visible residential row selects {business}"
            );
        }
        assert_eq!(state.business, Some(business));
        assert_eq!(state.step, 2);
        state.project_grid(camera, block.center).unwrap();
        #[cfg(feature = "capture")]
        let typed_capture = std::env::var("OPENMT2001_PLANNER_CAPTURE")
            .ok()
            .map(|prefix| {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) {
                    descriptor.backends = wgpu::Backends::DX12;
                }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                (prefix, renderer)
            });
        #[cfg(feature = "capture")]
        let sidewalk_capture = typed_capture.as_ref().map(|(_, renderer)| {
            let sidewalk_assets = crate::block_sidewalk_labels::Assets::load(
                &index,
                &archive,
                installer,
                &board.blocks,
                &crate::block_sidewalk_labels::load_style(&image, installer).unwrap(),
            )
            .unwrap();
            sidewalk_assets.prepare(renderer, scene).unwrap().0
        });
        let row = *state.context.catalog.row(business as usize).unwrap();
        let centers = build_panel::cell_centers(state.grid.as_ref().unwrap());
        let occupied = state.context.category_record[0x9c..0xcd]
            .try_into()
            .unwrap();
        let build_bytes = read("OPENMT2001_BUILD_FILE");
        let file = openmt2001::build_file::BuildFile::parse(&build_bytes).unwrap();
        let mut lot = None;
        'lots: for w in 1..=6u32 {
            for h in 1..=6u32 {
                if w * h < word(&row, 0x38)
                    || w * h > word(&row, 0x3c)
                    || w.max(h) > word(&row, 0x40)
                {
                    continue;
                }
                for x in 0..=7 - w {
                    for y in 0..=7 - h {
                        let rectangle =
                            [x as i32, y as i32, (x + w - 1) as i32, (y + h - 1) as i32];
                        if !openmt2001::building_pool::can_place(occupied, rectangle) {
                            continue;
                        }
                        let a = centers[(x * 7 + y) as usize];
                        let b = centers[((x + w - 1) * 7 + y + h - 1) as usize];
                        if state.parent_hit(a.map(|v| v * 0.5))
                            || state.parent_hit(b.map(|v| v * 0.5))
                        {
                            continue;
                        }
                        let spec = openmt2001::scene::BuildingSpec {
                            block: block.name.clone(),
                            lot: [x as u8, y as u8, w as u8, h as u8],
                            height: word(&row, 0x48) as u8,
                            kind: kind as u8,
                            variation: 0,
                            quality: 0,
                            businesses: vec![],
                        };
                        if openmt2001::scene::SceneBuilding::resolve(spec, &board, &file).is_ok() {
                            lot = Some((w, h, a, b));
                            break 'lots;
                        }
                    }
                }
            }
        }
        let (w, h, a, b) = lot.expect("original business has visible buildable model lot");
        state.pointer(Some(a.map(|v| v as i32)));
        state.mouse(true);
        state.pointer(Some(b.map(|v| v as i32)));
        #[cfg(feature = "capture")]
        if let Some((prefix, renderer)) = &typed_capture {
            assets.prepare(renderer, &mut state).unwrap();
            let plate = state.grid_overlay(renderer).unwrap().unwrap();
            crate::capture_test::capture_scene_world_overlays(
                renderer,
                scene,
                &[sidewalk_capture.as_ref().unwrap()],
                &[&plate, state.prepared.as_ref().unwrap()],
                &format!("{prefix}-{capture_kind}-2-grid.png"),
            );
        }
        state.mouse(false);
        assert!(state.take_placement_request());
        assert_eq!(state.step, 2);
        // This geometry/quote fixture deliberately selects original design0;
        // the native three-call RNG stream has a separate original oracle.
        state.apply_placement_choices([0; 3]);
        assert_eq!(state.step, 3);
        if kind != 2 {
            state.sync_model_variants(&build_bytes).unwrap();
            assert!(
                state.design_count() > 0,
                "stage3 style total is available before the worker quote"
            );
            assert!(
                state.quote.is_none(),
                "first adjustment frame precedes the worker quote"
            );
            #[cfg(feature = "capture")]
            if let Some((prefix, renderer)) = &typed_capture {
                assets.prepare(renderer, &mut state).unwrap();
                crate::capture_test::capture_scene_world_overlays(
                    renderer,
                    scene,
                    &[sidewalk_capture.as_ref().unwrap()],
                    &[state.prepared.as_ref().unwrap()],
                    &format!("{prefix}-{capture_kind}-3-before-quote.png"),
                );
            }
        }
        let request = state.request().unwrap();
        assert_eq!(
            (request.kind, request.business, request.size, request.height),
            (
                kind,
                business,
                [w as i32, h as i32],
                word(&row, 0x48) as i32
            )
        );
        let quote = inspect(request.clone());
        if kind != 2 {
            assert_eq!(
                state.model_variants,
                quote.preview.as_ref().unwrap().model_variants,
                "planning count and authoritative preview use the same original template search"
            );
        }
        assert!(
            quote.preview.is_some(),
            "type {kind} business {business} original model preview: placement={}, locked={}, affordable={}, request={:?}",
            quote.placement_allowed,
            quote.category_locked,
            quote.affordable,
            (
                request.origin,
                request.size,
                request.height,
                request.packed_model
            )
        );
        assert!(
            quote.allowed(),
            "type {kind} quote supports live submission"
        );
        state.quote_request = Some(request);
        state.quote = Some(quote);
        state.needs_quote = false;
        assert!(
            state.can_submit(),
            "type {kind} native Accept button is enabled"
        );
        #[cfg(feature = "capture")]
        if let Some((prefix, renderer)) = &typed_capture {
            state.dirty = true;
            assets.prepare(renderer, &mut state).unwrap();
            let selection = crate::selection::Assets::load(&executable, &index, &archive).unwrap();
            let build_file = read("OPENMT2001_BUILD_FILE");
            let suncols = read("OPENMT2001_SUNCOLS");
            state
                .preview(
                    renderer,
                    scene.camera,
                    scene.fog,
                    &selection,
                    PreviewFiles {
                        build_file: &build_file,
                        executable: &executable,
                        directory: &directory,
                        archive: &archive,
                        suncols: &suncols,
                        phase: 9,
                    },
                )
                .unwrap()
                .unwrap();
            let mut overlays = vec![state.preview_cache.as_ref().unwrap()];
            if let Some(marker) = state.preview_marker() {
                overlays.push(marker);
            }
            overlays.push(state.prepared.as_ref().unwrap());
            crate::capture_test::capture_scene_world_overlays(
                renderer,
                scene,
                &[sidewalk_capture.as_ref().unwrap()],
                &overlays,
                &format!("{prefix}-{capture_kind}-3-quoted.png"),
            );
        }
        let accept_screen = if window == [960, 720] {
            [105., 414.]
        } else {
            [265., 414.]
        };
        let accept = crate::intro::internal_pointer(accept_screen, window).unwrap();
        assert_eq!(accept, [140, 552]);
        assert!(
            click(&mut state, accept),
            "visible Accept completes kind {kind}"
        );
        assert!(state.submit);
        state.submit = false;
        assert!(click(&mut state, [34, 74]));
        assert_eq!(state.step, 0);
    }
    assert!(click(&mut state, [196, 100]));
    assert_eq!(state.step, 1);
    let business_count = state.display_rows.len();
    assert_eq!(state.display_rows[0], BusinessRow::Heading(1526));
    let separator = state
        .display_rows
        .iter()
        .position(|row| *row == BusinessRow::Separator)
        .unwrap();
    assert_eq!(
        state.display_rows[separator + 1],
        BusinessRow::Heading(1527)
    );
    assert_eq!(business_count, state.businesses.len() + 3);
    assert_eq!(state.preview_business, None);
    assert!(
        !click(&mut state, [200, 158]),
        "Retail group heading cannot select a business"
    );
    assert_eq!(state.step, 1);
    #[cfg(feature = "capture")]
    if let Ok(prefix) = std::env::var("OPENMT2001_PLANNER_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        assets.prepare(&renderer, &mut state).unwrap();
        crate::capture_test::capture(
            &renderer,
            state.prepared.as_ref(),
            scene,
            &format!("{prefix}-1-retail-initial.png"),
        );
    }
    assert!(business_count > BUSINESS_ROWS);
    assert!(click(&mut state, [356, 312]));
    assert_eq!(state.scroll, 1, "down arrow advances the business list");
    assert!(click(&mut state, [356, 160]));
    assert_eq!(state.scroll, 0, "up arrow reverses the business list");
    state.scroll(-100);
    assert_eq!(state.scroll, business_count - BUSINESS_ROWS);
    state.pointer(Some([200, 313]));
    assert_eq!(
        state.hovered_business(),
        state.display_rows.last().and_then(|row| row.business())
    );
    state.scroll(100);
    assert_eq!(state.scroll, 0);
    state.scroll = separator - 5;
    assert!(
        !click(&mut state, [200, 247]),
        "blank separator cannot select a business"
    );
    assert_eq!(state.step, 1);
    state.scroll = 0;
    // Original 43ec30 list selection does not emit the generic 443360 cue.
    state.pointer(Some([200, 158]));
    assert!(!state.mouse(true));
    assert!(!state.pointer_click_cue());
    state.pointer(Some([700, 700]));
    assert!(!state.pointer_click_cue());
    assert!(!state.mouse(false));
    state.pointer(Some([360, 200]));
    assert!(!state.mouse(true));
    state.pointer(Some([360, 600]));
    assert_eq!(state.scroll, business_count - BUSINESS_ROWS);
    state.pointer = Some([180., 100.]);
    assert!(!state.mouse(false));
    assert_eq!(
        state.scroll,
        business_count - BUSINESS_ROWS,
        "scrollbar release preserves last held sample"
    );
    state.scroll(100);
    // A release over a list row after pressing elsewhere must not select it.
    state.pointer(Some([196, 116]));
    state.mouse(true);
    state.pointer(Some([200, 158]));
    assert!(!state.mouse(false));
    assert_eq!(state.step, 1);
    assert_eq!(
        state.hovered_business(),
        None,
        "the Day heading is not a business"
    );
    assert!(click(&mut state, [200, 176]));
    assert_eq!(state.step, 2);
    // The retained completed-stage circle remains clickable under both
    // window transforms while the base-plate drag stage is open.
    for (size, screen) in [([960, 720], [25.5, 99.]), ([1280, 720], [185.5, 99.])] {
        let pointer = crate::intro::internal_pointer(screen, size).unwrap();
        state.pointer(Some(pointer));
        assert!(!state.mouse(true));
        assert!(state.mouse(false));
        assert_eq!(state.step, 1);
        state.step = 2;
    }
    // The constructor's second back-arrow widget returns to business choice.
    assert!(click(&mut state, [350, 158]));
    assert_eq!(state.step, 1);
    state.step = 2;
    assert!(crate::intro::internal_pointer([50., 99.], [1280, 720]).is_none());
    let row = state
        .context
        .catalog
        .row(state.business.unwrap() as usize)
        .unwrap()
        .to_vec();
    let capacity_rate = f32::from_bits(word(&row, 0x58));
    state.project_grid(camera, block.center).unwrap();
    let centers = build_panel::cell_centers(state.grid.as_ref().unwrap());
    let occupied = state.context.category_record[0x9c..0xcd]
        .try_into()
        .unwrap();
    // Choose an unoccupied catalog-sized rectangle whose endpoints can be
    // reached outside the panel, so the native drag plate is legal and white.
    let mut chosen = None;
    'placements: for w in 1..=6u32 {
        for h in 1..=6u32 {
            if w * h < word(&row, 0x38) || w * h > word(&row, 0x3c) || w.max(h) > word(&row, 0x40) {
                continue;
            }
            for x in 0..=7 - w {
                for y in 0..=7 - h {
                    let rectangle = [x as i32, y as i32, (x + w - 1) as i32, (y + h - 1) as i32];
                    if !openmt2001::building_pool::can_place(occupied, rectangle) {
                        continue;
                    }
                    let a = centers[(x * 7 + y) as usize];
                    let b = centers[((x + w - 1) * 7 + y + h - 1) as usize];
                    if !state.parent_hit(a.map(|v| v * 0.5))
                        && !state.parent_hit(b.map(|v| v * 0.5))
                    {
                        chosen = Some((w, h, a, b));
                        break 'placements;
                    }
                }
            }
        }
    }
    let (w, h, a, b) = chosen.unwrap();
    state.pointer(Some(a.map(|v| v as i32)));
    state.mouse(true);
    state.pointer(Some(b.map(|v| v as i32)));
    state.mouse(false);
    assert!(state.take_placement_request());
    assert_eq!(state.step, 2);
    // Controlled design0 keeps this input/quote fixture independent of RNG.
    state.apply_placement_choices([0; 3]);
    assert_eq!(state.step, 3);
    // The caption is a native 43e9a0 text child, not one of the planner's
    // event12 back controls. Tutorial90's separate gate remains addressable.
    state.pointer(Some([200, 210]));
    assert_eq!(state.tutorial_control_at(), Some(90));
    assert!(!click(&mut state, [200, 210]));
    assert_eq!(state.step, 3);
    assert!(state.needs_quote);
    assert!(!state.can_submit());
    let request = state.request().unwrap();
    assert_eq!(request.kind, 0);
    assert_eq!(request.size, [w as i32, h as i32]);
    assert_eq!(request.packed_model, state.styles.qualities[0][0]);
    assert!(
        openmt2001::economy::stock_capacity((w * h) as i32, state.height as u8, capacity_rate) > 0
    );
    state.pointer(Some([196, 156]));
    state.mouse(true);
    state.pointer(Some([196, 355]));
    state.mouse(false);
    assert_eq!(state.quality, 0);
    assert!(click(&mut state, [196, 355]));
    assert_eq!(state.quality, 1);
    assert!(state.needs_quote);
    assert_eq!(
        state.request().unwrap().packed_model,
        state.styles.qualities[1][0]
    );
    // Native generic radio children cue on press and after leaving/re-entering
    // while held; release clears the latch for the next press.
    state.pointer(Some([196, 355]));
    assert!(!state.mouse(true));
    assert!(state.pointer_click_cue());
    assert!(!state.pointer_click_cue());
    state.pointer(Some([700, 700]));
    assert!(!state.pointer_click_cue());
    state.pointer(Some([196, 355]));
    assert!(state.pointer_click_cue());
    state.pointer(Some([700, 700]));
    assert!(!state.mouse(false));
    state.pointer(Some([196, 355]));
    assert!(!state.mouse(true));
    assert!(state.pointer_click_cue());
    assert!(state.mouse(false));
    // At the default 960x720 window, one planning-panel unit is 1.5
    // physical pixels. The visible native radio and arrow footprints own
    // their respective hitboxes; crossing between adjacent controls does not.
    let panel_point = |p: [f32; 2]| p.map(|v| v / 1.5);
    assert!(hit(QUALITY[1], panel_point([147., 267.])));
    assert!(!hit(QUALITY[0], panel_point([147., 267.])));
    assert!(hit(
        slider_arrows(FLOOR_SLIDER)[1],
        panel_point([270., 223.])
    ));
    assert!(hit(
        slider_arrows(STYLE_SLIDER)[1],
        panel_point([270., 310.5])
    ));
    let floor = state.height;
    assert!(click(&mut state, [360, 298]));
    assert_eq!(state.height, floor + 1);
    assert!(click(&mut state, [76, 298]));
    assert_eq!(state.height, floor);
    // Original432360 event5 captures and samples before release, including
    // points beyond the track. Event6 clears capture without sampling.
    let min = word(&row, 0x48) as i32;
    let max = word(&row, 0x44) as i32;
    state.pointer(Some([120, 298]));
    assert!(!state.mouse(true));
    assert!(
        !state.pointer_click_cue(),
        "native 43f510 track drag is silent"
    );
    state.pointer(Some([600, 298]));
    assert_eq!(
        state.height, max,
        "captured drag clamps to maximum outside panel"
    );
    state.pointer = Some([60., 149.]);
    assert!(!state.mouse(false));
    assert_eq!(
        state.height, max,
        "release does not resample its new position"
    );
    state.pointer(Some([120, 298]));
    state.mouse(true);
    state.pointer(Some([0, 298]));
    assert_eq!(state.height, min);
    assert!(!state.mouse(false));
    state.height = floor;
    state.changed();
    // The native design range is populated by the selected model preview.
    // Obtain that authoritative count before exercising its arrow controls.
    let design_request = state.request().unwrap();
    let design_quote = inspect(design_request.clone());
    assert!(design_quote.preview.is_some());
    state.quote_request = Some(design_request);
    state.quote = Some(design_quote);
    assert!(click(&mut state, [360, 414]));
    assert_eq!(state.choices[1], 1);
    assert!(click(&mut state, [76, 414]));
    assert_eq!(state.choices[1], 0);
    state.pointer(Some([76, 298]));
    state.mouse(true);
    state.pointer(Some([360, 298]));
    assert!(!state.mouse(false));
    assert_eq!(state.height, floor);
    #[cfg(feature = "capture")]
    if let Ok(prefix) = std::env::var("OPENMT2001_PLANNER_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let sidewalk_assets = crate::block_sidewalk_labels::Assets::load(
            &index,
            &archive,
            installer,
            &board.blocks,
            &crate::block_sidewalk_labels::load_style(&image, installer).unwrap(),
        )
        .unwrap();
        let selection_assets =
            crate::selection::Assets::load(&executable, &index, &archive).unwrap();
        let (sidewalk, _) = sidewalk_assets.prepare(&renderer, scene).unwrap();
        for stage in 0..=3 {
            state.step = stage;
            if stage == 1 {
                state.pointer(Some([200, 158]));
            }
            state.dirty = true;
            assets.prepare(&renderer, &mut state).unwrap();
            crate::capture_test::capture(
                &renderer,
                state.prepared.as_ref(),
                scene,
                &format!("{prefix}-{stage}.png"),
            );
            if stage == 2 {
                state.pointer(Some(a.map(|v| v as i32)));
                assert!(!state.mouse(true));
                state.pointer(Some(b.map(|v| v as i32)));
                assets.prepare(&renderer, &mut state).unwrap();
                let plate = state.grid_overlay(&renderer).unwrap().unwrap();
                crate::capture_test::capture_scene_world_overlays(
                    &renderer,
                    scene,
                    &[&sidewalk],
                    &[&plate, state.prepared.as_ref().unwrap()],
                    &format!("{prefix}-{stage}-grid.png"),
                );
                assert!(!state.mouse(false));
                assert!(state.take_placement_request());
                state.apply_placement_choices(placement());
            }
        }
        let furniture = state
            .display_rows
            .iter()
            .position(|row| *row == BusinessRow::Business(39))
            .unwrap();
        state.scroll = furniture.saturating_sub(4);
        state.step = 1;
        let furniture_point = [
            200,
            ((BUSINESS_ROW_TOP
                + (furniture - state.scroll) as f32 * BUSINESS_ROW_HEIGHT
                + BUSINESS_ROW_HEIGHT * 0.5)
                * 2.) as i32,
        ];
        state.pointer(Some(furniture_point));
        assert_eq!(state.hovered_business(), Some(39));
        state.pointer(Some([
            200,
            ((BUSINESS_ROW_TOP
                + (BUSINESS_ROWS - 1) as f32 * BUSINESS_ROW_HEIGHT
                + BUSINESS_ROW_HEIGHT * 0.5)
                * 2.) as i32,
        ]));
        assert_eq!(
            state.hovered_business(),
            state
                .display_rows
                .get(state.scroll + BUSINESS_ROWS - 1)
                .and_then(|row| row.business())
        );
        state.pointer(Some([
            200,
            ((BUSINESS_ROW_TOP + BUSINESS_ROWS as f32 * BUSINESS_ROW_HEIGHT) * 2.) as i32,
        ]));
        assert_eq!(state.hovered_business(), None);
        state.pointer(Some(furniture_point));
        state.pointer(Some([700, 700]));
        assert_eq!(state.hovered_business(), None);
        assert_eq!(state.preview_business, Some(39));
        assets.prepare(&renderer, &mut state).unwrap();
        crate::capture_test::capture(
            &renderer,
            state.prepared.as_ref(),
            scene,
            &format!("{prefix}-1-furniture.png"),
        );
        let request = state.request().unwrap();
        let quote = inspect(request.clone());
        assert!(
            quote.preview.is_some(),
            "real planning quote has a model preview"
        );
        let quoted_building = quote.preview.as_ref().unwrap();
        let [lot_x, lot_y, lot_w, lot_d] = quoted_building.spec.lot;
        let occupied: &[u8; 49] = state.context.category_record[0x9c..0xcd]
            .try_into()
            .unwrap();
        assert!(
            openmt2001::building_pool::can_place(
                occupied,
                [
                    i32::from(lot_x),
                    i32::from(lot_y),
                    i32::from(lot_x + lot_w - 1),
                    i32::from(lot_y + lot_d - 1),
                ]
            ),
            "the quoted footprint is unoccupied in the native 7x7 grid"
        );
        for existing in scene
            .inspection_buildings
            .iter()
            .filter(|b| b.category == state.context.category)
        {
            let [x, y, w, d] = existing.lot;
            assert!(
                lot_x + lot_w <= x || x + w <= lot_x || lot_y + lot_d <= y || y + d <= lot_y,
                "quoted footprint intersects existing logical building lot"
            );
        }
        let block = board
            .blocks
            .iter()
            .find(|block| block.id == state.context.category)
            .unwrap();
        assert_eq!(
            quoted_building.center,
            [
                block.center[0] - 35. + f32::from(lot_x) * 10. + f32::from(lot_w) * 5.,
                block.center[1] - 35. + f32::from(lot_y) * 10. + f32::from(lot_d) * 5.,
            ],
            "quoted preview retains native lot-center placement"
        );
        state.quote_request = Some(request);
        state.quote = Some(quote);
        state.reset_preview();
        state.needs_quote = false;
        state.step = 3;
        state.dirty = true;
        assets.prepare(&renderer, &mut state).unwrap();
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        state
            .preview(
                &renderer,
                scene.camera,
                scene.fog,
                &selection_assets,
                PreviewFiles {
                    build_file: &build_file,
                    executable: &executable,
                    directory: &directory,
                    archive: &archive,
                    suncols: &suncols,
                    phase: 9,
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            state.preview_failure(),
            None,
            "original preview materials must resolve"
        );
        assert!(state.grid_overlay(&renderer).unwrap().is_none());
        let preview = state.preview_cache.as_ref().unwrap();
        let marker = state.preview_marker().expect("quoted building marker");
        crate::capture_test::capture_scene_overlays(
            &renderer,
            scene,
            &[&sidewalk, preview, marker, state.prepared.as_ref().unwrap()],
            &format!("{prefix}-3-quoted.png"),
        );
        crate::capture_test::capture_scene_world_overlays(
            &renderer,
            scene,
            &[&sidewalk, preview, marker],
            &[state.prepared.as_ref().unwrap()],
            &format!("{prefix}-3-quoted-depth.png"),
        );
        let retained = state.preview_cache.as_ref().unwrap() as *const PreparedScene;
        let retained_marker = state.preview_marker().unwrap() as *const PreparedScene;
        state
            .preview(
                &renderer,
                scene.camera,
                scene.fog,
                &selection_assets,
                PreviewFiles {
                    build_file: &build_file,
                    executable: &executable,
                    directory: &directory,
                    archive: &archive,
                    suncols: &suncols,
                    phase: 9,
                },
            )
            .unwrap();
        assert_eq!(
            retained,
            state.preview_cache.as_ref().unwrap() as *const PreparedScene
        );
        assert_eq!(
            retained_marker,
            state.preview_marker().unwrap() as *const PreparedScene
        );
    }
    state.step = 3;
    click(&mut state, [300, 550]);
    assert!(state.close);
}

#[cfg(test)]
mod caption_tests {
    #[test]
    fn planner_caption_and_value_color_indices() {
        for (class, ids) in [
            ("stage headings", &[1512, 1522, 1514, 1515][..]),
            ("Provides heading", &[1513][..]),
            ("adjustment captions", &[1516, 1520, 1518][..]),
            ("cost captions", &[1521, 1523][..]),
        ] {
            for &id in ids {
                assert_eq!(super::planner_text_color_index(id), 16, "{class}: {id}");
            }
        }
        assert_eq!(
            super::PROVIDES_TEXT_COLOR_INDEX,
            9,
            "provided goods/services"
        );
        for (class, ids) in [
            ("selected types", &[860, 859, 861][..]),
            ("floor and style counts", &[1519][..]),
            ("quality choices", &[1656, 1655, 1657][..]),
            ("amounts and capacity", &[2243, 2244, 1524, 1525][..]),
        ] {
            for &id in ids {
                assert_eq!(super::planner_text_color_index(id), 0, "{class}: {id}");
            }
        }
    }

    #[test]
    #[ignore = "requires original v1.4 static planner caption glyph fixture"]
    fn original_static_adjustment_caption_glyphs() {
        assert_native_fixture(
            "work/frontend/native-construction-static-labels.bin",
            b"MTCLBL01",
            &[1516, 1520, 1518],
            3,
            0,
        );
    }
    #[test]
    #[ignore = "requires original v1.4 quality-label glyph fixture"]
    fn original_quality_label_glyphs() {
        assert_native_fixture(
            "work/frontend/native-construction-quality-labels.bin",
            b"MTQLBL01",
            &[1656, 1655, 1657],
            0,
            3,
        );
    }
    #[test]
    #[ignore = "requires original v1.4 stage-title glyph fixture"]
    fn original_stage_title_glyphs() {
        assert_native_fixture(
            "work/frontend/native-construction-stage-title.bin",
            b"MTSTTL01",
            &[1512, 1521],
            3,
            4,
        );
    }
    #[test]
    #[ignore = "requires original v1.4 numeric-caption glyph fixture"]
    fn original_floor_count_glyphs() {
        assert_native_fixture(
            "work/frontend/native-construction-floor-count.bin",
            b"MTFCNT01",
            &[1519],
            3,
            5,
        );
    }
    #[test]
    #[ignore = "requires original v1.4 short and wrapped block-title fixture"]
    fn original_block_title_glyphs() {
        assert_native_fixture(
            "work/frontend/native-construction-block-title.bin",
            b"MTBTIT01",
            &[1436, 1437, 1438],
            1,
            4,
        );
    }
    fn assert_native_fixture(path: &str, magic: &[u8; 8], ids: &[u32], font_id: u32, style: u32) {
        let data = std::fs::read(path).unwrap();
        assert_eq!(&data[..8], magic);
        let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x037f);
        let font_length = word(12) as usize;
        assert_eq!(word(16) as usize, ids.len());
        let font = &data[20..20 + font_length];
        let mut at = 20 + font_length;
        for &expected_id in ids {
            let id = word(at);
            assert_eq!(id, expected_id);
            assert_eq!(word(at + 4), font_id, "original constructor font");
            let bounds = std::array::from_fn(|i| f32::from_bits(word(at + 8 + i * 4)));
            let text_length = word(at + 24) as usize;
            at += 28;
            let encoded = &data[at..at + text_length];
            at += text_length;
            let mut pages = [super::Surface::solid(), super::Surface::solid()];
            match style {
                0 => crate::ui::text_widget_style0(&mut pages, font, encoded, bounds, 0xffffffff),
                3 => crate::ui::text_widget_style3(&mut pages, font, encoded, bounds, 0xffffffff),
                4 => crate::ui::text_widget_style4(&mut pages, font, encoded, bounds, 0xffffffff),
                5 => crate::ui::text_widget_style5(&mut pages, font, encoded, bounds, 0xffffffff),
                _ => unreachable!(),
            }
            .unwrap();
            for (page, actual) in pages.iter().enumerate() {
                let count = word(at) as usize;
                at += 4;
                assert_eq!(
                    actual.vertices.len(),
                    count * 6,
                    "label {id} page {page} glyph count"
                );
                for glyph in 0..count {
                    for (vertex, index) in [0, 1, 2, 5].into_iter().enumerate() {
                        let actual_vertex = actual.vertices[glyph * 6 + index];
                        for (axis, value) in actual_vertex[..3].iter().enumerate() {
                            assert_eq!(
                                value.to_bits(),
                                word(at + glyph * 48 + vertex * 12 + axis * 4),
                                "label {id} page {page} glyph {glyph} position {vertex}:{axis}"
                            );
                        }
                    }
                }
                at += count * 48;
                for glyph in 0..count {
                    for (vertex, index) in [0, 1, 2, 5].into_iter().enumerate() {
                        let actual_vertex = actual.vertices[glyph * 6 + index];
                        for axis in 0..2 {
                            assert_eq!(
                                actual_vertex[6 + axis].to_bits(),
                                word(at + glyph * 32 + vertex * 8 + axis * 4),
                                "label {id} page {page} glyph {glyph} UV {vertex}:{axis}"
                            );
                        }
                    }
                }
                at += count * 32;
            }
        }
        assert_eq!(at, data.len());
    }
}

mod slider_tests {
    #[test]
    #[ignore = "requires original construction list/slider fixture"]
    fn construction_list_and_slider_background_match_original() {
        let source = openmt2001::fixtures::path("NATIVE_BUILDER_SLIDERS");
        let font = std::fs::read(source.with_file_name("native-construction-font.bin")).unwrap();
        assert_eq!(&font[..8], b"MTCFONT1");
        let fw = |at| u32::from_le_bytes(font[at..at + 4].try_into().unwrap());
        assert_eq!(fw(8), 0x037f);
        assert_eq!(super::BUSINESS_ROW_HEIGHT, fw(12) as f32);
        assert_eq!(super::BUSINESS_ROW_TOP, fw(20) as f32);
        let list = std::fs::read(source.with_file_name("native-construction-list.bin")).unwrap();
        assert_eq!(&list[..8], b"MTCLIST1");
        let word = |at| u32::from_le_bytes(list[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x037f);
        assert_eq!(word(12), 13);
        let rect = |at| std::array::from_fn::<_, 4, _>(|i| f32::from_bits(word(at + i * 4)));
        for (actual, expected) in [
            (super::BUSINESS_LIST, rect(16)),
            (
                crate::ui::slider_track(super::BUSINESS_SCROLL, true),
                rect(32),
            ),
            (super::BUSINESS_UP, rect(48)),
            (super::BUSINESS_DOWN, rect(64)),
        ] {
            for (a, b) in actual.into_iter().zip(expected) {
                assert!((a - b).abs() < 0.0001);
            }
        }
        for i in 0..word(80) as usize {
            let y = word(84 + i * 8) as f32;
            if (super::BUSINESS_LIST[1]..=super::BUSINESS_LIST[3]).contains(&y) {
                assert_eq!(
                    super::business_row([50., y]),
                    Some(word(88 + i * 8) as usize)
                );
            }
        }
        let mut at = 84 + word(80) as usize * 8;
        let count = word(at);
        at += 4;
        for _ in 0..count {
            let row = word(at) as usize;
            let fills = word(at + 8) as usize;
            at += 12;
            assert_eq!(
                fills, 2,
                "original hover fill precedes opaque selected fill"
            );
            for i in 0..fills {
                let expected = rect(at);
                let (actual, color) = super::business_selection(row);
                for (a, b) in actual.into_iter().zip(expected) {
                    assert!((a - b).abs() < 0.0001);
                }
                assert_eq!(word(at + 16), if i == 0 { 0xff6ed1e1 } else { color });
                at += 20;
            }
        }
        assert_eq!(at, list.len());
        let data = std::fs::read(source.with_file_name("native-slider-track.bin")).unwrap();
        assert_eq!(&data[..8], b"MTTRACK1");
        let w = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(w(8), 0x037f);
        for i in 0..w(12) as usize {
            let at = 16 + i * 48;
            let track = std::array::from_fn(|j| f32::from_bits(w(at + 12 + j * 4)));
            let expected = std::array::from_fn(|j| f32::from_bits(w(at + 28 + j * 4)));
            assert_eq!(
                crate::ui::slider_background(track, w(at) != 0, w(at + 4) > 0 && w(at + 8) == 0),
                (expected, w(at + 44))
            );
        }
    }
    #[test]
    #[ignore = "requires original slider constructor/input fixture"]
    fn matches_native_builder_slider_geometry_and_input() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_BUILDER_SLIDERS")).unwrap();
        assert_eq!(&bytes[..8], b"MTSLDR01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x037f);
        let constructors = word(12) as usize;
        let mut at = 16;
        for _ in 0..constructors {
            let x = word(at + 4) as i32 as f32;
            let y = word(at + 8) as i32 as f32;
            let width = word(at + 12) as i32 as f32;
            let bounds = [x, y, x + width, y + 16.];
            let native = |offset| {
                std::array::from_fn::<_, 4, _>(|i| f32::from_bits(word(at + 20 + offset + i * 4)))
            };
            assert_eq!(super::slider_track(bounds), native(16));
            assert_eq!(super::slider_arrows(bounds), [native(36), native(52)]);
            at += 88;
        }
        let draws = word(at) as usize;
        at += 4;
        for _ in 0..draws {
            let maximum = word(at) as i32;
            let value = word(at + 4) as i32;
            let disabled = word(at + 8) != 0;
            let handle = super::slider_handle([116., 75.5, 284., 90.5], value, maximum);
            for (i, value) in handle.iter().enumerate() {
                assert_eq!(value.to_bits(), word(at + 12 + i * 4));
            }
            let fill = if disabled || maximum == 0 {
                0xcc676767
            } else {
                0xff1067cf
            };
            for i in 0..4 {
                assert_eq!(fill, word(at + 28 + i * 4));
            }
            let colors = if disabled || maximum == 0 {
                [0xb4505050, 0xb4909090]
            } else {
                [0xff90c0e0, 0xff104790]
            };
            for (strip, points) in super::slider_edges(handle).iter().enumerate() {
                for (i, point) in points.iter().enumerate() {
                    let base = at + 44 + strip * 64 + i * 16;
                    assert_eq!(point[0].to_bits(), word(base));
                    assert_eq!(point[1].to_bits(), word(base + 4));
                    assert_eq!(0.5f32.to_bits(), word(base + 8));
                    assert_eq!(colors[strip], word(base + 12));
                }
            }
            at += 172;
        }
        let count = word(at) as usize;
        at += 4;
        for row in bytes[at..].chunks_exact(12) {
            let w = |offset| u32::from_le_bytes(row[offset..offset + 4].try_into().unwrap());
            assert_eq!(
                super::slider_selection([116., 75.5, 284., 90.5], w(4) as f32, w(0) as i32),
                w(8) as i32
            );
        }
        assert_eq!(bytes.len(), at + count * 12);
        let path = openmt2001::fixtures::path("NATIVE_BUILDER_SLIDERS")
            .with_file_name("native-builder-slider-arrows.bin");
        let arrows = std::fs::read(path).unwrap();
        assert_eq!(&arrows[..8], b"MTSLARR1");
        for row in arrows[16..].chunks_exact(24) {
            let w = |offset| u32::from_le_bytes(row[offset..offset + 4].try_into().unwrap());
            for (i, (pressed, hovered, enabled)) in [
                (false, false, true),
                (false, true, true),
                (true, true, true),
                (false, false, false),
                (true, false, true),
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(
                    super::slider_arrow_texture(w(0) as usize, pressed, hovered, enabled),
                    w(4 + i * 4) as usize
                );
            }
        }
    }
}

#[cfg(test)]
mod mesh_facing_tests {
    use super::*;
    use openmt2001::{mesh::CachedMesh, pe::PeImage, ui_mesh};

    // Test oracle only: production uses a finite authored part list, not a
    // new renderer/filter. Raw mesh Y is negated by the shared panel loader.
    fn area(positions: &[[f32; 3]], ids: [usize; 3]) -> f64 {
        let [a, b, c] = ids.map(|id| [f64::from(positions[id][0]), -f64::from(positions[id][1])]);
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    }
    #[test]
    fn canonical_facing_oracle_checks_orientation_degeneracy_and_order() {
        let positions = [
            [0., 0., 9.],
            [0., -2., 9.],
            [3., -2., 9.],
            [3., 0., 9.],
            [1.5, -1., 9.],
        ];
        let triangles = [[0, 2, 1], [0, 1, 2], [0, 4, 2], [0, 0, 1], [0, 2, 3]];
        assert_eq!(
            triangles.map(|ids| area(&positions, ids)),
            [6., -6., 0., 0., -6.]
        );
        let fronts = |points: &[[f32; 3]]| {
            triangles
                .into_iter()
                .filter(|&ids| area(points, ids) < 0.)
                .collect::<Vec<_>>()
        };
        assert_eq!(fronts(&positions), [[0, 1, 2], [0, 2, 3]]);
        assert_eq!(fronts(&positions.map(|[x, y, z]| [x, -y, z])), [[0, 2, 1]]);
        assert_eq!(
            fronts(&positions.map(|[x, y, _]| [x * 2. + 11., y * 3. - 7., -99.])),
            [[0, 1, 2], [0, 2, 3]]
        );
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
    fn same_surface(a: &Surface, b: &Surface) {
        assert_eq!(a.vertices, b.vertices);
        assert_eq!(a.alpha, b.alpha);
        assert_eq!((a.width, a.height), (b.width, b.height));
        assert_eq!(a.pixels, b.pixels);
        assert!(matches!(
            a.draw().mode,
            openmt2001::render::SurfaceMode::Overlay
        ));
    }
    fn supplied() -> Assets {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        guard(
            &directory,
            "8ecb0d18a3896eba61db9ff6edc10869a84a64229169b52f0f7d981b81980d52",
        );
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let payload = |name: &str| {
            let e = index.find(name).unwrap();
            &archive[e.offset as usize..(e.offset + e.length) as usize]
        };
        let name = "gamedata2/brand_new_panels/main_panel_v2.tm";
        guard(
            payload(name),
            "b405f1b5146fdc6ea76a13984a7cd97f54c13ce41cb26f752c4ffec62549eab1",
        );
        for (name, hash) in [
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
            guard(payload(name), hash);
        }
        let executable = read("OPENMT2001_EXE");
        guard(
            &executable,
            "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b",
        );
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        guard(
            &fs::read(installer.join("Standard/fonts/tahoma12bldnoantip.tab")).unwrap(),
            "03b9924e83bb7a8c650bd58c87bcb3d669dcdf0198bb240e5f964d434d8c62be",
        );
        guard(
            &fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab")).unwrap(),
            "7df258462744a70bce18516dd6d79239cd0fd434c7e100ce4ef8ad399d36c644",
        );
        guard(
            &fs::read(installer.join("Standard/fonts/cust640p.tab")).unwrap(),
            "082d6a0488289a0926ee3b2e279882a03c31922e6f1ec6aa75e56a6ffc1d72d5",
        );
        guard(
            &fs::read(installer.join("parameters/1930.prm")).unwrap(),
            "1c172a6fb41104ae39ca24b7a171e93d3619c8b8dc642b4b6dfddacca32e1648",
        );
        guard(
            &fs::read(installer.join("parameters/1940.prm")).unwrap(),
            "adfe73d6999158310ca5988a05a097263e383f30bb753d3c2fc3b76e5815e142",
        );
        guard(
            &fs::read(installer.join("parameters/1950.prm")).unwrap(),
            "70ec82d8074a2f0ba853fc450e911fef7a14025262876d71f5818de3a215a526",
        );
        guard(
            &fs::read(installer.join("parameters/1960.prm")).unwrap(),
            "98cc9a74d43e0a39b95141d5b7bfe80493dacc58d5f3a651df9f2c97a274477a",
        );
        guard(
            &fs::read(installer.join("parameters/1970.prm")).unwrap(),
            "98cc9a74d43e0a39b95141d5b7bfe80493dacc58d5f3a651df9f2c97a274477a",
        );
        guard(
            &fs::read(installer.join("parameters/1980.prm")).unwrap(),
            "98cc9a74d43e0a39b95141d5b7bfe80493dacc58d5f3a651df9f2c97a274477a",
        );
        guard(
            &fs::read(installer.join("parameters/1990.prm")).unwrap(),
            "98cc9a74d43e0a39b95141d5b7bfe80493dacc58d5f3a651df9f2c97a274477a",
        );
        guard(
            &fs::read(installer.join("parameters/2000.prm")).unwrap(),
            "98cc9a74d43e0a39b95141d5b7bfe80493dacc58d5f3a651df9f2c97a274477a",
        );
        // Assets::load stores these strings without indexing them; no planner
        // State/Context or worker is opened by this geometry/material fixture.
        let assets = Assets::load(&index, &archive, installer, &[], &executable).unwrap();
        let mesh = CachedMesh::parse(payload(name)).unwrap();
        assert_eq!(
            (mesh.vertex_count, mesh.parts.len(), mesh.frames),
            (196, 49, 10)
        );
        let pos = mesh.positions(9).unwrap();
        let uv = mesh.uvs();
        let regions = ui_mesh::regions(&mesh, 9).unwrap();
        assert_eq!(regions.len(), 49);
        assert_eq!(
            regions.iter().map(|r| r.part).collect::<Vec<_>>(),
            (0..49).collect::<Vec<_>>()
        );
        // The shared helper must return every region even when most parts
        // are not uploaded. Verify raw indexed bounds for all 49 records.
        let (shared, bounds) =
            ui::panel_frame_base(&index, &archive, name, &(0..9).collect::<Vec<_>>(), 9).unwrap();
        assert_eq!(bounds.len(), 49);
        for (i, region) in regions.iter().enumerate() {
            let ids = mesh.parts[region.part].vertex_indices().collect::<Vec<_>>();
            let expected = [
                ids.iter().map(|&j| pos[j][0]).fold(f32::INFINITY, f32::min),
                ids.iter()
                    .map(|&j| -pos[j][1])
                    .fold(f32::INFINITY, f32::min),
                ids.iter()
                    .map(|&j| pos[j][0])
                    .fold(f32::NEG_INFINITY, f32::max),
                ids.iter()
                    .map(|&j| -pos[j][1])
                    .fold(f32::NEG_INFINITY, f32::max),
            ];
            assert_eq!(region.bounds, expected);
            assert_eq!(bounds[i], expected);
        }
        assert_eq!(assets.entry_bounds, regions[16].bounds);
        assert_eq!(
            assets.input_bounds,
            ui_mesh::parent_bounds(&mesh, 9).unwrap()
        );
        for (i, name) in [
            (9, "P_MAIN_MAIN_WINDOW"),
            (10, "P_MAIN_VERTCAL_BAR_BACKING"),
            (11, "P_MAIN_TITLE_AREA_BACKING"),
        ] {
            assert_eq!(regions[i].name, name);
        }
        assert_eq!(
            assets.base.len(),
            10,
            "nine borders and the live palette, no black placeholders"
        );
        let mut counts = [0usize; 2];
        for (part_id, part) in mesh.parts[..12].iter().enumerate() {
            let ids = part.vertex_indices().collect::<Vec<_>>();
            for triangle in ids.chunks_exact(3) {
                let signed = area(&pos, triangle.try_into().unwrap());
                assert!(signed.is_finite() && signed != 0.);
                if part_id < 9 {
                    assert!(signed < 0.);
                    counts[0] += 1;
                } else {
                    assert!(signed > 0.);
                    counts[1] += 1;
                }
            }
            let textures =
                TextureSet::parse(payload(mesh.texture_names[part.texture as usize])).unwrap();
            if part_id < 9 {
                let mut expected = Surface::new(&textures.textures[0]);
                for id in ids {
                    expected.vertices.push([
                        pos[id][0],
                        -pos[id][1],
                        0.5,
                        1.,
                        1.,
                        1.,
                        uv[id][0],
                        uv[id][1],
                    ]);
                    expected.alpha.push(255);
                }
                same_surface(&assets.base[part_id], &expected);
                same_surface(&assets.base[part_id], &shared[part_id]);
            } else {
                assert!(
                    textures.textures[0]
                        .texels
                        .chunks_exact(4)
                        .all(|p| p == [0, 0, 0, 137])
                );
            }
        }
        assert_eq!(counts, [18, 6]);
        let image = PeImage::parse(&executable).unwrap();
        assert_eq!(image.at(0x5362c0, 4).unwrap(), 0xbb000a28u32.to_le_bytes());
        let palette = ui::mpp_background(&mesh, 9, &image).unwrap();
        same_surface(&assets.base[9], &palette);
        assert!(!palette.vertices.is_empty());
        assets
    }
    #[test]
    #[ignore = "requires SHA-pinned original Build mesh/materials/PE/fonts/year parameters; CPU only"]
    fn supplied_build_front_parts_regions_and_materials() {
        let _ = supplied();
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "SHA-pinned supplied assets, offscreen GPU and output path; base/palette diagnostic only"]
    fn capture_build_base_palette_over_checker() {
        let assets = supplied();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut checker = Surface::solid();
        for row in 0..15 {
            for col in 0..20 {
                let color = if (row + col) % 2 == 0 {
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
                    [color; 4],
                );
            }
        }
        let underlay = renderer
            .prepare(&[checker.draw()], PROJECTION, None)
            .unwrap();
        let panel = renderer
            .prepare(
                &assets.base.iter().map(Surface::draw).collect::<Vec<_>>(),
                PROJECTION,
                None,
            )
            .unwrap();
        crate::capture_test::capture_overlay(
            &renderer,
            &underlay,
            &panel,
            &std::env::var("OPENMT2001_BUILD_BASE_CAPTURE_PATH").unwrap(),
        );
    }
}
