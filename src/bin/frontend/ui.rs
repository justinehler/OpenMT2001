//! Native UI surfaces; original asset parsing stays in shared core modules.
#[path = "ui/text_entry.rs"]
mod text_entry;
#[path = "ui/text_field.rs"]
pub mod text_field;
use openmt2001::{
    render::{SurfaceMode, TexturedDraw},
    texture::{TextureRecord, loader_pixels},
};
pub use text_entry::text_entry;

pub use openmt2001::human_dialog::integer_money;

#[test]
#[ignore = "requires original integer currency observations"]
fn matches_native_integer_currency() {
    let data = std::fs::read(openmt2001::fixtures::path("NATIVE_INTEGER_CURRENCY")).unwrap();
    assert_eq!(&data[..8], b"MTICUR01");
    let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    let mut at = 12;
    for case in 0..word(8) {
        let value = word(at) as i32;
        let suffix = word(at + 4) != 0;
        let size = word(at + 8) as usize;
        at += 12;
        let symbol = data[at..at + size]
            .iter()
            .map(|b| b - 32)
            .collect::<Vec<_>>();
        at += size;
        let size = word(at) as usize;
        at += 4;
        let expected = data[at..at + size]
            .iter()
            .map(|b| b - 32)
            .collect::<Vec<_>>();
        at += size;
        assert_eq!(
            integer_money(value, &symbol, suffix),
            expected,
            "native currency case{case}"
        );
    }
    assert_eq!(at, data.len());
}

/// Original linked CRT %.2f rounds an exact halfway magnitude away from zero.
/// A float's24-bit significand times100 fits exactly in f64; large float values
/// already have no fractional part. The CRT retains17 significant decimal digits.
pub fn decimal2(value: f32) -> Result<String, String> {
    if !value.is_finite() {
        return Err("non-finite UI monetary value".into());
    }
    if value == 0. {
        return Ok("0.00".into());
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value.abs() >= 1e17 {
        let mut digits = format!("{:.0}", value.abs()).into_bytes();
        if digits.len() > 17 {
            let round = digits[17] >= b'5';
            digits[17..].fill(b'0');
            if round {
                for i in (0..17).rev() {
                    if digits[i] != b'9' {
                        digits[i] += 1;
                        break;
                    }
                    digits[i] = b'0';
                    if i == 0 {
                        digits.insert(0, b'1');
                    }
                }
            }
        }
        return Ok(format!("{sign}{}.00", String::from_utf8(digits).unwrap()));
    }
    let scaled = f64::from(value.abs()) * 100.;
    if scaled.fract() == 0.5 {
        let cents = scaled.ceil() as u64;
        Ok(format!("{sign}{}.{:02}", cents / 100, cents % 100))
    } else {
        Ok(format!("{value:.2}"))
    }
}

/// Original 43a950: format an absolute f32 with the linked CRT, then place
/// the localized currency symbol before or after it. The negative sign is
/// always first, including when the symbol precedes the digits.
pub fn formatted_float_money(value: f32, symbol: &[u8], suffix: bool) -> Result<Vec<u8>, String> {
    let digits = decimal2(value.abs())?;
    let mut text = Vec::with_capacity(digits.len() + symbol.len() + 1);
    if value < 0. {
        text.push(b'-' - 32);
    }
    if !suffix {
        text.extend_from_slice(symbol);
    }
    text.extend(digits.bytes().map(|byte| byte - 32));
    if suffix {
        text.extend_from_slice(symbol);
    }
    Ok(text)
}

#[cfg(test)]
#[test]
#[ignore = "requires original currency fixture"]
fn matches_native_currency() {
    let data = std::fs::read(openmt2001::fixtures::path("NATIVE_CURRENCY")).unwrap();
    assert_eq!(&data[..8], b"MTCUR001");
    let mut at = 8;
    let word = |at: &mut usize| {
        let value = u32::from_le_bytes(data[*at..*at + 4].try_into().unwrap());
        *at += 4;
        value
    };
    let count = word(&mut at);
    for _ in 0..count {
        let bits = word(&mut at);
        let len = word(&mut at) as usize;
        assert_eq!(
            decimal2(f32::from_bits(bits)).unwrap().as_bytes(),
            &data[at..at + len],
            "{bits:08x}"
        );
        at += len;
    }
    assert_eq!(at, data.len());
}

#[cfg(test)]
#[test]
#[ignore = "requires original 43a950 symbol and side fixture"]
fn matches_native_float_money() {
    let data = std::fs::read(openmt2001::fixtures::path("NATIVE_FLOAT_MONEY")).unwrap();
    assert_eq!(&data[..8], b"MTFMON01");
    let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    let count = word(8) as usize;
    let mut at = 12;
    for case in 0..count {
        let value = f32::from_bits(word(at));
        let suffix = word(at + 4) != 0;
        let symbol_len = word(at + 8) as usize;
        let rendered_len = word(at + 12) as usize;
        at += 16;
        let symbol = data[at..at + symbol_len]
            .iter()
            .map(|byte| byte - 32)
            .collect::<Vec<_>>();
        at += symbol_len;
        let expected = data[at..at + rendered_len]
            .iter()
            .map(|byte| byte - 32)
            .collect::<Vec<_>>();
        at += rendered_len;
        assert_eq!(
            formatted_float_money(value, &symbol, suffix).unwrap(),
            expected,
            "original 43a950 case {case}"
        );
    }
    assert_eq!(at, data.len());
}

/// Original40cd70/41dc20 palettes, submitted by40fe50 through433030.
/// The names resolve the animated mesh frame; these are runtime fills rather
/// than baked colours in the panel texture.
pub fn mpp_background(
    mesh: &openmt2001::mesh::CachedMesh<'_>,
    frame: usize,
    image: &openmt2001::pe::PeImage<'_>,
) -> Result<Surface, String> {
    mpp_background_title(mesh, frame, image, None)
}
pub fn mpp_background_title(
    mesh: &openmt2001::mesh::CachedMesh<'_>,
    frame: usize,
    image: &openmt2001::pe::PeImage<'_>,
    title: Option<[u32; 4]>,
) -> Result<Surface, String> {
    let regions = openmt2001::ui_mesh::regions(mesh, frame)?;
    let mut surface = Surface::solid();
    for (name, colors) in mpp_palette(u32::from_le_bytes(
        image.at(0x5362c0, 4)?.try_into().unwrap(),
    )) {
        // Original43af30 returns its zero-initialized rectangle when the name
        // is absent (43aca0 with part -1). The v1.4 mesh lacks slider backing2.
        if let Some(region) = regions.iter().find(|r| r.name == name) {
            surface.rect(
                region.bounds,
                if name == "P_MAIN_TITLE_AREA_BACKING" {
                    title.unwrap_or(colors)
                } else {
                    colors
                },
            );
        }
    }
    Ok(surface)
}

fn mpp_palette(body: u32) -> [(&'static str, [u32; 4]); 6] {
    [
        ("P_MAIN_MAIN_WINDOW", [body; 4]),
        (
            "P_MAIN_VERTCAL_BAR_BACKING",
            [0xff777777, 0x66777777, 0x66777777, 0xff777777],
        ),
        (
            "P_MAIN_TITLE_AREA_BACKING",
            [0xff777777, 0xff777777, 0x66777777, 0x66777777],
        ),
        ("P_MAIN_TITLE_AREA1", [0x00ff0000; 4]),
        ("P_MAIN_TITLE_AREA2", [0x00ff0000; 4]),
        ("P_MAIN_SLIDER_BACKING2", [0xff777777; 4]),
    ]
}

/// The state of one tab in a panel's tab row, as the original's button draw
/// (0x443080) resolves it.
///
/// The tab rows are toggle-mode buttons (+0x338 = 1). They are built by the
/// info-tab loop in 0x41dcf0 (block and building panels) and the
/// mode-tab loops at 0x40da50 and 0x40cfba (city and locator panels). The
/// draw starts from the normal image. Hover replaces it, or the pressed image
/// replaces it when the button is held: a press (phase 2, 0x443426) clears the
/// hover flag. A disabled button (+0x333) then takes the disabled image, and a
/// selected button (+0x336) finally takes the selected image over every other
/// state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabState {
    Normal,
    Hover,
    Pressed,
    Disabled,
    Selected,
}

impl TabState {
    pub fn of(selected: bool, disabled: bool, pressed: bool, hover: bool) -> Self {
        if selected {
            Self::Selected
        } else if disabled {
            Self::Disabled
        } else if pressed {
            Self::Pressed
        } else if hover {
            Self::Hover
        } else {
            Self::Normal
        }
    }

    /// The generic texture for this state's tab backing. 0x41dcf0, 0x40da50
    /// and 0x40cfba all load images 13 (normal), 15 (hover), 17 (disabled),
    /// 14 (pressed) and 16 (selected, the backing with the rim) into the
    /// button's five image slots.
    pub fn backing_image(self) -> usize {
        match self {
            Self::Normal => 13,
            Self::Hover => 15,
            Self::Pressed => 14,
            Self::Disabled => 17,
            Self::Selected => 16,
        }
    }
}

pub struct MppModes {
    pub buttons: [[f32; 4]; 6],
    pub icons: [[f32; 4]; 4],
    pub header: [f32; 4],
}
pub fn mpp_icon(button: [f32; 4]) -> Result<[f32; 4], String> {
    use openmt2001::x87::Sum;
    let mut icon = [0.; 4];
    for axis in 0..4 {
        let mut inset = Sum::from_f64(0.025)?;
        inset.mul_f32(button[axis])?;
        if axis >= 2 {
            inset = inset.negated();
        }
        inset.add_f32(button[axis])?;
        icon[axis] = inset.to_f32();
    }
    Ok(icon)
}
/// Original40d90c..40dc09, then410dbd..410e8d. Texture195 supplies
/// horizontal spacing, texture13 the button dimensions. The supplied set
/// uses25x25 for both. Original icon inset scales absolute coordinates.
pub fn mpp_modes(bounds: [f32; 4], dimensions: [u32; 2]) -> Result<MppModes, String> {
    use openmt2001::x87::Sum;
    if bounds.iter().any(|v| !v.is_finite() || v.abs() > 16384.)
        || dimensions.iter().any(|&v| v == 0 || v > 1024)
    {
        return Err("unsupported MPP screen dimensions".into());
    }
    let x = (bounds[0] + 1.).trunc() as i32 + 1;
    let y = (bounds[1] + 2.).trunc() as i32 + 1;
    let extent = |start: f32, size: u32| -> Result<f32, String> {
        let mut v = Sum::from_f64(0.9)?;
        v.mul_f32(size as f32)?;
        v.add_f32(start)?;
        Ok(v.to_f32())
    };
    let mut buttons = [[0.; 4]; 6];
    for (i, button) in buttons.iter_mut().enumerate() {
        let left = (x + i as i32 * dimensions[0] as i32) as f32;
        *button = [
            left,
            y as f32,
            extent(left, dimensions[0])?,
            extent(y as f32, dimensions[1])?,
        ];
    }
    let mut icons = [[0.; 4]; 4];
    for (icon, button) in icons.iter_mut().zip(buttons) {
        *icon = mpp_icon(button)?;
    }
    Ok(MppModes {
        buttons,
        icons,
        header: [
            bounds[0],
            bounds[1] + dimensions[1] as f32 + 2.,
            bounds[2],
            bounds[1] + dimensions[1] as f32 + 16.,
        ],
    })
}

pub fn panel_base(
    index: &openmt2001::archive::ArchiveIndex,
    archive: &[u8],
    name: &str,
    parts: &[usize],
) -> Result<(Vec<Surface>, Vec<[f32; 4]>), String> {
    panel_frame_base(index, archive, name, parts, 0)
}

/// The original UI preset stores cull state 3 at 0x459237, which becomes
/// CULL_CW at 0x4efa5e–0x4efa69 for the shared widget mesh pass (0x43c4e0).
/// In the panel's X/-Y screen pose, reverse-facing triangles mark layout
/// regions; drawing them would back runtime translucent fills with texture.
fn panel_triangle_visible(points: [[f32; 3]; 3]) -> bool {
    let [a, b, c] = points.map(|p| [f64::from(p[0]), -f64::from(p[1])]);
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    area.is_finite() && area < 0.
}

pub fn panel_frame_base(
    index: &openmt2001::archive::ArchiveIndex,
    archive: &[u8],
    name: &str,
    parts: &[usize],
    frame: usize,
) -> Result<(Vec<Surface>, Vec<[f32; 4]>), String> {
    let payload = |name: &str| -> Result<&[u8], String> {
        let e = index
            .find(name)
            .ok_or_else(|| format!("missing panel asset {name}"))?;
        archive
            .get(e.offset as usize..(e.offset + e.length) as usize)
            .ok_or("panel asset range".into())
    };
    let mesh = openmt2001::mesh::CachedMesh::parse(payload(name)?)?;
    let bounds = openmt2001::ui_mesh::regions(&mesh, frame)?
        .iter()
        .map(|r| r.bounds)
        .collect();
    let pos = mesh.positions(frame)?;
    let uv = mesh.uvs();
    let mut base = vec![];
    for &i in parts {
        let part = mesh.parts.get(i).ok_or("missing panel part")?;
        let tex = openmt2001::texture::TextureSet::parse(payload(
            mesh.texture_names[part.texture as usize],
        )?)
        .map_err(|e| e.to_string())?;
        let mut s = Surface::new(&tex.textures[0]);
        let indices = part.vertex_indices().collect::<Vec<_>>();
        for triangle in indices.as_chunks::<3>().0 {
            if panel_triangle_visible(triangle.map(|v| pos[v])) {
                for &v in triangle {
                    s.vertices
                        .push([pos[v][0], -pos[v][1], 0.5, 1., 1., 1., uv[v][0], uv[v][1]]);
                    s.alpha.push(255);
                }
            }
        }
        if !s.vertices.is_empty() {
            base.push(s);
        }
    }
    Ok((base, bounds))
}
#[derive(Clone)]
pub struct Surface {
    pub vertices: Vec<[f32; 8]>,
    pub alpha: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
impl Surface {
    pub fn new(texture: &TextureRecord<'_>) -> Self {
        Self {
            vertices: vec![],
            alpha: vec![],
            width: texture.width,
            height: texture.height,
            pixels: loader_pixels(texture),
        }
    }
    pub fn solid() -> Self {
        Self {
            vertices: vec![],
            alpha: vec![],
            width: 1,
            height: 1,
            pixels: vec![255; 4],
        }
    }
    pub fn quad(&mut self, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], c: [u32; 4]) {
        for i in [0, 1, 2, 0, 2, 3] {
            self.vertices.push([
                p[i][0],
                p[i][1],
                0.5,
                ((c[i] >> 16) & 255) as f32 / 255.,
                ((c[i] >> 8) & 255) as f32 / 255.,
                (c[i] & 255) as f32 / 255.,
                uv[i][0],
                uv[i][1],
            ]);
            self.alpha.push((c[i] >> 24) as u8);
        }
    }
    pub fn rect(&mut self, b: [f32; 4], c: [u32; 4]) {
        let [l, t, r, b] = b;
        self.quad([[l, t], [l, b], [r, b], [r, t]], [[0., 0.]; 4], c);
    }
    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], c: u32) {
        if a[0] == b[0] {
            self.rect(
                [a[0] - 0.5, a[1].min(b[1]), a[0] + 0.5, a[1].max(b[1])],
                [c; 4],
            )
        } else {
            self.rect(
                [a[0].min(b[0]), a[1] - 0.5, a[0].max(b[0]), a[1] + 0.5],
                [c; 4],
            )
        }
    }
    pub fn draw(&self) -> TexturedDraw<'_> {
        TexturedDraw {
            view_projection: None,
            vertices: &self.vertices,
            specular: None,
            diffuse_alpha: Some(&self.alpha),
            mode: SurfaceMode::Overlay,
            width: self.width,
            height: self.height,
            rgba: &self.pixels,
            mipmaps: false,
            secondary: None,
        }
    }
}

pub fn button(
    surface: &mut Surface,
    bounds: [f32; 4],
    rgb: [u8; 3],
    hover: bool,
    pressed: bool,
    disabled: bool,
) -> Result<(), String> {
    button_state(
        surface,
        bounds,
        rgb,
        openmt2001::ui_button::State {
            flag_330: pressed,
            flag_334: pressed,
            flag_335: hover && !disabled,
            flag_333: disabled,
        },
    )
}
/// Original colored backgrounds also distinguish held entry from press origin.
pub fn button_state(
    surface: &mut Surface,
    bounds: [f32; 4],
    rgb: [u8; 3],
    state: openmt2001::ui_button::State,
) -> Result<(), String> {
    use openmt2001::ui_button::{self, Primitive};
    for p in ui_button::background(bounds, rgb, state)? {
        match p {
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
                    surface.line(a, z, color)
                }
            }
            Primitive::Line { start, end, color } => surface.line(start, end, color),
        }
    }
    Ok(())
}
/// Generic 442b10 switches unavailable captions to palette style 8, also
/// used by the disabled Save button. Font art retains its embossed edges.
pub fn button_label(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    disabled: bool,
) -> Result<(), String> {
    text_widget_style9(
        pages,
        tab,
        encoded,
        bounds,
        if disabled { 0xff646464 } else { 0xffffffff },
    )
}

/// 4a1250 / 4a8730 draw SELECTP before the translucent player backing.
/// Opaque name, portrait and lower-slot art then mask its interior. The
/// authored rectangles determine the border width; this is not a line box.
#[derive(Default)]
pub struct LobbyHighlight {
    tick: u32,
}
impl LobbyHighlight {
    pub fn advance(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
    pub fn paint(&self, surface: &mut Surface, bounds: [f32; 4]) {
        let alpha = 255 - (self.tick.wrapping_mul(8) & 255);
        surface.rect(bounds, [(alpha << 24) | 0x00ff00; 4]);
    }
}
/// Shared 430b50 list bars; selection is drawn after hover in the original.
pub fn list_highlight(
    shapes: &mut Surface,
    layout: openmt2001::frontend_list::Layout,
    row: usize,
    selected: bool,
    hovered: bool,
) {
    if let Some(color) = openmt2001::frontend_list::row_color(selected, hovered) {
        shapes.rect(layout.highlight(row), [color; 4]);
    }
}

/// 4406a0 style0 inset3 + 2; 4419f0 advances rows without row-edge clipping.
pub fn list_text(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    layout: openmt2001::frontend_list::Layout,
    row: usize,
) -> Result<(), String> {
    let mut bounds = layout.text_bounds(row);
    bounds[0] = layout.text_origin(row)[0];
    text(pages, tab, encoded, bounds, false, 0xffffffff)
}

pub fn text(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    center: bool,
    color: u32,
) -> Result<(), String> {
    text_scaled(pages, tab, encoded, bounds, center, color, 1.)
}
/// Text palette 0x536288 entry 16 (mint, via 440ad0), the attribute every
/// original section caption passes. 0xff00ff00 is entry 4, which no caption
/// uses.
pub const SECTION_HEADER_COLOR: u32 = 0xff3afd87;
/// Text palette 0x536288 entry 16 (mint), the attribute that Save 496c70
/// and Load 4940f0 give the captions on their green field bars.
pub const FIELD_CAPTION_COLOR: u32 = 0xff3afd87;
/// Save 497ab0 and Load 494d70 fill each field's caption bar with this green
/// (palette entry 17) and the value region under it with this half-alpha
/// navy band (entry 15 at alpha 0x80), over the 0x5362c0 panel backing.
pub const FIELD_HEADER_BAR: u32 = 0xff01762f;
pub const FIELD_VALUE_BAND: u32 = 0x801f2f70;

/// Green section captions shared by the single-player setup screens.
pub fn section_header(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
) -> Result<(), String> {
    text(pages, tab, encoded, bounds, true, SECTION_HEADER_COLOR)
}
pub fn text_fit(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    right: bool,
    color: u32,
) -> Result<(), String> {
    text_fit_with_scale(pages, tab, encoded, bounds, right, color, 1.)
}
/// Original 440540 default style 0 for the planner's static adjustment
/// captions. The widget truncates its origin and applies a two-pixel inset.
pub fn text_widget_style0(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    color: u32,
) -> Result<(), String> {
    use openmt2001::font::{Font, Layout};
    if bounds.into_iter().any(|value| !value.is_finite())
        || bounds[2] <= bounds[0]
        || bounds[3] <= bounds[1]
    {
        return Err("invalid text widget bounds".into());
    }
    let font = Font::parse(tab)?;
    let origin = [bounds[0].trunc() + 2., bounds[1].trunc()];
    for quad in font.layout(
        encoded,
        Layout {
            origin,
            depth: 0.5,
            scale: 1.,
            gap: -1,
            baseline: 0,
        },
    )? {
        pages[quad.page].quad(quad.positions.map(|p| [p[0], p[1]]), quad.uv, [color; 4]);
    }
    Ok(())
}
/// Original 440650 style 3, used by construction quality labels and Gameplay
/// descriptions. Rows keep full-size glyphs and the retained W line advance.
pub fn text_widget_style3(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    color: u32,
) -> Result<(), String> {
    use openmt2001::font::{Font, Layout};
    let font = Font::parse(tab)?;
    let w = usize::from(b'W' - 32);
    let at = 4 + w * 44 + 36;
    let height = i32::from_le_bytes(
        tab.get(at..at + 4)
            .ok_or("font W height metric")?
            .try_into()
            .unwrap(),
    );
    let width = f64::from(bounds[2]) - f64::from(bounds[0]);
    let widget_height = f64::from(bounds[3]) - f64::from(bounds[1]);
    if !width.is_finite() || !widget_height.is_finite() || width <= 0. || widget_height <= 0. {
        return Err("invalid text widget bounds".into());
    }
    let (offsets, wrapped) =
        openmt2001::prompt_queue::wrap_with_gap(tab, encoded, width.trunc() as i32, -1)?;
    let rows = offsets.windows(2).collect::<Vec<_>>();
    let top = bounds[1].trunc()
        + ((widget_height - rows.len().max(1) as f64 * f64::from(height)) * 0.5).trunc() as f32;
    for (line, range) in rows.into_iter().enumerate() {
        // Original row storage includes one scratch terminator per row.
        let end = range[1].saturating_sub(1).min(wrapped.len());
        let row = &wrapped[range[0].min(end)..end];
        let origin = [bounds[0].trunc() + 2., top + line as f32 * height as f32];
        for quad in font.layout(
            row,
            Layout {
                origin,
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            pages[quad.page].quad(quad.positions.map(|p| [p[0], p[1]]), quad.uv, [color; 4]);
        }
    }
    Ok(())
}
/// Original 440650 style 4 at scale 1, used by construction stage headings.
/// Native 1512/1521 glyph streams fix the integer center and baseline here.
pub fn text_widget_style4(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    color: u32,
) -> Result<(), String> {
    use openmt2001::font::{Font, Layout};
    let font = Font::parse(tab)?;
    let height_at = 4 + usize::from(b'W' - 32) * 44 + 36;
    let height = i32::from_le_bytes(
        tab.get(height_at..height_at + 4)
            .ok_or("font W height metric")?
            .try_into()
            .unwrap(),
    );
    let span = [
        f64::from(bounds[2]) - f64::from(bounds[0]),
        f64::from(bounds[3]) - f64::from(bounds[1]),
    ];
    if span.into_iter().any(|v| !v.is_finite() || v <= 0.) {
        return Err("invalid text widget bounds".into());
    }
    let (offsets, wrapped) =
        openmt2001::prompt_queue::wrap_with_gap(tab, encoded, span[0].trunc() as i32, -1)?;
    let rows = offsets.windows(2).collect::<Vec<_>>();
    let top = bounds[1].trunc()
        + ((span[1] - rows.len().max(1) as f64 * f64::from(height)) * 0.5).trunc() as f32;
    for (line, range) in rows.into_iter().enumerate() {
        // Native line storage includes one terminal scratch byte per row.
        let end = range[1].saturating_sub(1).min(wrapped.len());
        let row = &wrapped[range[0].min(end)..end];
        let width = f64::from(text_width(tab, row, 1.)?);
        let origin = [
            bounds[0].trunc() + ((span[0] - width) * 0.5 + 0.5).trunc() as f32,
            top + line as f32 * height as f32,
        ];
        for quad in font.layout(
            row,
            Layout {
                origin,
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            pages[quad.page].quad(quad.positions.map(|p| [p[0], p[1]]), quad.uv, [color; 4]);
        }
    }
    Ok(())
}
/// Original 440650 style 5, used by the construction "current of total"
/// numeric captions. Native glyph origins are right aligned and vertically
/// centered without rescaling the font.
pub fn text_widget_style5(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    color: u32,
) -> Result<(), String> {
    use openmt2001::font::{Font, Layout};
    let font = Font::parse(tab)?;
    let height_at = 4 + usize::from(b'W' - 32) * 44 + 36;
    let height = i32::from_le_bytes(
        tab.get(height_at..height_at + 4)
            .ok_or("font W height metric")?
            .try_into()
            .unwrap(),
    );
    let span = f64::from(bounds[3]) - f64::from(bounds[1]);
    if !span.is_finite() || span <= 0. {
        return Err("invalid text widget bounds".into());
    }
    let width = text_width(tab, encoded, 1.)? as f32;
    let origin = [
        bounds[2].trunc() - width,
        bounds[1].trunc() + ((span - f64::from(height)) * 0.5).trunc() as f32,
    ];
    for quad in font.layout(
        encoded,
        Layout {
            origin,
            depth: 0.5,
            scale: 1.,
            gap: -1,
            baseline: 0,
        },
    )? {
        pages[quad.page].quad(quad.positions.map(|p| [p[0], p[1]]), quad.uv, [color; 4]);
    }
    Ok(())
}
/// Original 440650 style 9 used by Graphics Options. Each wrapped row is
/// centered separately. Its vertical measure is the greatest glyph
/// y-offset plus glyph height in that row, not the font's W line height.
pub fn text_widget_style9(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    color: u32,
) -> Result<(), String> {
    use openmt2001::font::{Font, Layout};
    let font = Font::parse(tab)?;
    let span = [
        f64::from(bounds[2]) - f64::from(bounds[0]),
        f64::from(bounds[3]) - f64::from(bounds[1]),
    ];
    if span.into_iter().any(|v| !v.is_finite() || v <= 0.) {
        return Err("invalid text widget bounds".into());
    }
    let (offsets, wrapped) =
        openmt2001::prompt_queue::wrap_with_gap(tab, encoded, span[0].trunc() as i32, -1)?;
    let rows = offsets
        .windows(2)
        .map(|range| {
            let end = range[1].saturating_sub(1).min(wrapped.len());
            &wrapped[range[0].min(end)..end]
        })
        .collect::<Vec<_>>();
    let heights = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|&glyph| {
                    let record = 4 + usize::from(glyph) * 44;
                    let height =
                        i32::from_le_bytes(tab[record + 36..record + 40].try_into().unwrap());
                    let offset =
                        i32::from_le_bytes(tab[record + 40..record + 44].try_into().unwrap());
                    height + offset
                })
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let total_height: i32 = heights.iter().sum();
    let w_at = 4 + usize::from(b'W' - 32) * 44 + 36;
    let line_advance = i32::from_le_bytes(tab[w_at..w_at + 4].try_into().unwrap());
    let mut y = bounds[1].trunc() + ((span[1] - f64::from(total_height)) * 0.5).trunc() as f32;
    for row in rows {
        let width = f64::from(text_width(tab, row, 1.)?);
        let x = bounds[0].trunc() + ((span[0] - width) * 0.5 + 0.5).trunc() as f32;
        for quad in font.layout(
            row,
            Layout {
                origin: [x, y],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            pages[quad.page].quad(quad.positions.map(|p| [p[0], p[1]]), quad.uv, [color; 4]);
        }
        y += line_advance as f32;
    }
    Ok(())
}
pub fn text_fit_with_scale(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    right: bool,
    color: u32,
    max_scale: f32,
) -> Result<(), String> {
    let width = text_width(tab, encoded, 1.)? as f32;
    let scale = if width > 0. {
        ((bounds[2] - bounds[0]) / width).min(max_scale)
    } else {
        max_scale
    };
    let scaled_width = text_width(tab, encoded, scale)? as f32;
    text_scaled(
        pages,
        tab,
        encoded,
        [
            if right {
                bounds[2] - scaled_width
            } else {
                bounds[0]
            },
            bounds[1],
            bounds[2],
            bounds[3],
        ],
        false,
        color,
        scale,
    )
}
/// Original4e74f0 sums each truncated glyph advance, including the global
/// minus-one gap set by4c13de..4c145a through4e6ea0.
pub fn text_width(tab: &[u8], encoded: &[u8], scale: f32) -> Result<i32, String> {
    openmt2001::font::Font::parse(tab)?;
    if !scale.is_finite() || scale <= 0. {
        return Err("invalid text measurement scale".into());
    }
    encoded.iter().try_fold(0i32, |sum, &glyph| {
        let at = 4 + usize::from(glyph) * 44 + 32;
        let width = i32::from_le_bytes(
            tab.get(at..at + 4)
                .ok_or("text glyph metric")?
                .try_into()
                .unwrap(),
        );
        let advance = (f64::from(width.wrapping_sub(1)) * f64::from(scale)).trunc();
        if advance < f64::from(i32::MIN) || advance > f64::from(i32::MAX) {
            return Err("text advance overflow".into());
        }
        sum.checked_add(advance as i32)
            .ok_or_else(|| "text width overflow".into())
    })
}
fn text_scaled(
    pages: &mut [Surface],
    tab: &[u8],
    encoded: &[u8],
    bounds: [f32; 4],
    center: bool,
    color: u32,
    scale: f32,
) -> Result<(), String> {
    use openmt2001::font::{Font, Layout};
    let font = Font::parse(tab)?;
    let [l, t, r, _] = bounds;
    let layout = Layout {
        origin: [0., 0.],
        depth: 0.5,
        scale,
        gap: -1,
        baseline: 0,
    };
    let width = text_width(tab, encoded, scale)? as f32;
    text_at(
        pages,
        &font,
        encoded,
        bounds,
        color,
        Layout {
            origin: [if center { (l + r - width) / 2. } else { l }, t],
            ..layout
        },
    )
}

fn text_at(
    pages: &mut [Surface],
    font: &openmt2001::font::Font<'_>,
    encoded: &[u8],
    bounds: [f32; 4],
    color: u32,
    layout: openmt2001::font::Layout,
) -> Result<(), String> {
    for q in font.layout(encoded, layout)? {
        // Host clipping keeps long names inside their original widget bounds.
        // Glyph order is bottom-left, bottom-right, top-right, top-left.
        let left = q.positions[0][0];
        let right = q.positions[1][0];
        let top = q.positions[2][1];
        let bottom = q.positions[0][1];
        let clip = [
            left.max(bounds[0] - 0.5),
            top.max(bounds[1] - 0.5),
            right.min(bounds[2] - 0.5),
            bottom.min(bounds[3] - 0.5),
        ];
        if clip[0] >= clip[2] || clip[1] >= clip[3] {
            continue;
        }
        let positions = [
            [clip[0], clip[3]],
            [clip[2], clip[3]],
            [clip[2], clip[1]],
            [clip[0], clip[1]],
        ];
        let uv = positions.map(|p| {
            let x = (p[0] - left) / (right - left);
            let y = (p[1] - top) / (bottom - top);
            std::array::from_fn(|axis| {
                let upper = q.uv[3][axis] + (q.uv[2][axis] - q.uv[3][axis]) * x;
                let lower = q.uv[0][axis] + (q.uv[1][axis] - q.uv[0][axis]) * x;
                upper + (lower - upper) * y
            })
        });
        pages[q.page].quad(positions, uv, [color; 4]);
    }
    Ok(())
}

// Original432260 divides the integer mouse coordinate into maximum+1 bins.
pub fn slider_selection(track: [f32; 4], point: f32, maximum: i32, vertical: bool) -> i32 {
    let axis = usize::from(vertical);
    let mut position = openmt2001::x87::Sum::from_i32(point as i32);
    if position
        .compare(openmt2001::x87::Sum::from_f32(track[axis]).unwrap())
        .is_lt()
    {
        position = openmt2001::x87::Sum::from_f32(track[axis]).unwrap();
    }
    position.add_f32(-track[axis]).unwrap();
    let mut width = openmt2001::x87::Sum::from_f32(track[axis + 2]).unwrap();
    width.add_f32(-track[axis]).unwrap();
    position.div(width).unwrap();
    position.mul(openmt2001::x87::Sum::from_i32(maximum + 1));
    position.ftol_low_word().min(maximum)
}

// Original4321e0 stores the pixel center;432a80 sizes and clamps the handle.
pub fn slider_handle(track: [f32; 4], selection: i32, maximum: i32, vertical: bool) -> [f32; 4] {
    use openmt2001::x87::Sum;
    let axis = usize::from(vertical);
    let mut width = Sum::from_f32(track[axis + 2]).unwrap();
    width.add_f32(-track[axis]).unwrap();
    let mut size = width;
    size.div(Sum::from_i32(maximum + 1)).unwrap();
    let mut offset = width;
    offset.mul(Sum::from_i32(selection));
    offset.div(Sum::from_i32(maximum + 1)).unwrap();
    let mut half = size;
    half.mul_f32(0.5).unwrap();
    offset.add(half);
    let stored_center = offset.to_f32();
    if size.compare(Sum::from_i32(5)).is_lt() {
        size = Sum::from_i32(5);
    }
    let mut half = size;
    half.mul_f32(0.5).unwrap();
    let mut left = Sum::from_f32(stored_center).unwrap();
    left.add_f32(track[axis]).unwrap();
    left.add(half.negated());
    let mut stored_left = left.to_f32();
    if left.compare(Sum::from_f32(track[axis]).unwrap()).is_lt() {
        stored_left = track[axis];
    }
    let mut right = Sum::from_f32(stored_left).unwrap();
    right.add(size);
    if right
        .compare(Sum::from_f32(track[axis + 2]).unwrap())
        .is_gt()
    {
        left = Sum::from_f32(track[axis + 2]).unwrap();
        left.add(size.negated());
        stored_left = left.to_f32();
    }
    let mut right = Sum::from_f32(stored_left).unwrap();
    right.add(size);
    if vertical {
        [track[0] + 5., stored_left, track[2] - 5., right.to_f32()]
    } else {
        [stored_left, track[1] + 4., right.to_f32(), track[3] - 3.]
    }
}
pub fn slider_edges([l, t, r, b]: [f32; 4]) -> [[[f32; 2]; 4]; 2] {
    [
        [[l, t], [r + 0.5, t], [r, t], [r, b]],
        [[l, t + 0.5], [l, b], [l, b], [r, b]],
    ]
}
pub fn slider_arrow_texture(
    side: usize,
    pressed: bool,
    hovered: bool,
    enabled: bool,
    vertical: bool,
) -> usize {
    let base = if vertical {
        [105, 93][side]
    } else {
        97 + side * 4
    };
    base + if !enabled {
        3
    } else if pressed {
        2
    } else if hovered {
        1
    } else {
        0
    }
}

/// Original43f510/431f50 native sixteen-pixel arrows and track rectangles.
pub fn slider_arrows(bounds: [f32; 4], vertical: bool) -> [[f32; 4]; 2] {
    if vertical {
        [
            [bounds[0], bounds[1], bounds[2], bounds[1] + 16.],
            [bounds[0], bounds[3] - 16., bounds[2], bounds[3]],
        ]
    } else {
        [
            [bounds[0], bounds[1], bounds[0] + 16., bounds[3]],
            [bounds[2] - 16., bounds[1], bounds[2], bounds[3]],
        ]
    }
}
pub fn slider_track(bounds: [f32; 4], vertical: bool) -> [f32; 4] {
    if vertical {
        [bounds[0], bounds[1] + 18., bounds[2], bounds[3] - 18.]
    } else {
        [
            bounds[0] + 16.,
            bounds[1] + 0.5,
            bounds[2] - 16.,
            bounds[3] - 0.5,
        ]
    }
}

fn slider_contains([l, t, r, b]: [f32; 4], [x, y]: [f32; 2]) -> bool {
    x >= l && x <= r && y >= t && y <= b
}

pub struct Slider {
    pub bounds: [f32; 4],
    pub vertical: bool,
    pub value: f32,
    pub range: [f32; 2],
    pub pointer: Option<[f32; 2]>,
    pub pressed: Option<[f32; 2]>,
}
pub fn slider_background([l, t, r, b]: [f32; 4], vertical: bool, enabled: bool) -> ([f32; 4], u32) {
    let bounds = if vertical {
        [l + 2., t - 7., r - 1., b + 7.]
    } else {
        [l - 5., t + 1., r + 6., b - 1.]
    };
    (bounds, if enabled { 0xcc183088 } else { 0xcc444444 })
}
impl Slider {
    /// Original432990 paints the track before432a80 paints its handle.
    fn background_with_extent(&self, extent: i32) -> ([f32; 4], u32) {
        slider_background(
            slider_track(self.bounds, self.vertical),
            self.vertical,
            extent > 0,
        )
    }
    pub fn paint_handle(&self, surface: &mut Surface) {
        self.paint_handle_with_extent(surface, (self.range[1] - self.range[0]) as i32);
    }
    fn paint_handle_with_extent(&self, surface: &mut Surface, extent: i32) {
        let track = slider_track(self.bounds, self.vertical);
        let [min, _] = self.range;
        let value = self.value;
        let handle = slider_handle(track, (value - min) as i32, extent, self.vertical);
        let enabled = extent > 0;
        surface.rect(handle, [if enabled { 0xff1067cf } else { 0xcc676767 }; 4]);
        // Native renderer mode0 submits these exact two line strips. The portable
        // Surface.line producer expands each segment to a one-pixel quad.
        let colors = if enabled {
            [0xff90c0e0, 0xff104790]
        } else {
            [0xb4505050, 0xb4909090]
        };
        for (points, color) in slider_edges(handle).into_iter().zip(colors) {
            for segment in points.windows(2) {
                surface.line(segment[0], segment[1], color);
            }
        }
    }

    pub fn paint(&self, surface: &mut Surface, controls: &mut Vec<Surface>, textures: &[Surface]) {
        self.paint_with_extent(
            surface,
            controls,
            textures,
            (self.range[1] - self.range[0]) as i32,
        );
    }
    /// Native 431ba0→4326d0 stores a paint extent on the slider separately
    /// from the list's scrollable offset. Keep input clamping in the caller.
    pub fn paint_with_extent(
        &self,
        surface: &mut Surface,
        controls: &mut Vec<Surface>,
        textures: &[Surface],
        extent: i32,
    ) {
        let (bounds, color) = self.background_with_extent(extent);
        surface.rect(bounds, [color; 4]);
        self.paint_handle_with_extent(surface, extent);
        let enabled = extent > 0;
        for (i, arrow) in slider_arrows(self.bounds, self.vertical)
            .into_iter()
            .enumerate()
        {
            let hovered = self.pointer.is_some_and(|p| slider_contains(arrow, p));
            let captured = self.pressed.is_some_and(|p| slider_contains(arrow, p));
            let mut icon = textures
                [slider_arrow_texture(i, captured, hovered, enabled, self.vertical)]
            .clone();
            let [l, t, r, b] = arrow;
            icon.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            controls.push(icon);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tab_backings_follow_the_original_button_states() {
        use super::TabState;
        // 0x41dcf0 image slots: normal 13, hover 15, disabled 17, pressed 14,
        // selected 16.
        for (state, image) in [
            (TabState::Normal, 13),
            (TabState::Hover, 15),
            (TabState::Pressed, 14),
            (TabState::Disabled, 17),
            (TabState::Selected, 16),
        ] {
            assert_eq!(state.backing_image(), image, "{state:?}");
        }
        // 0x443080: the selected image wins over every other state, and the
        // disabled image over hover and press.
        assert_eq!(TabState::of(false, false, false, false), TabState::Normal);
        assert_eq!(TabState::of(false, false, false, true), TabState::Hover);
        assert_eq!(TabState::of(false, false, true, true), TabState::Pressed);
        assert_eq!(TabState::of(false, true, true, true), TabState::Disabled);
        assert_eq!(TabState::of(true, true, true, true), TabState::Selected);
        assert_eq!(TabState::of(true, false, false, false), TabState::Selected);
    }

    #[test]
    fn panel_backings_reject_clockwise_layout_faces() {
        let front = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]];
        assert!(super::panel_triangle_visible(front));
        assert!(!super::panel_triangle_visible([
            front[0], front[2], front[1]
        ]));
        assert!(!super::panel_triangle_visible([front[0]; 3]));
        let moved = front.map(|p| [p[0] + 42., p[1] - 12., p[2] + 0.3]);
        assert!(super::panel_triangle_visible(moved));
        assert!(!super::panel_triangle_visible([[f32::NAN; 3]; 3]));
    }

    #[test]
    fn default_panel_loaders_cull_layout_quads_without_losing_texture_alpha_or_bounds() {
        // A front-facing textured triangle and a reverse-facing layout
        // triangle share a rectangle, as the original panel meshes do.
        let mut mesh = vec![0u8; 164];
        mesh[..8].copy_from_slice(&[3, 0, 2, 0, 1, 0, 9, 0]);
        mesh[8..12].copy_from_slice(&1u32.to_le_bytes());
        for point in [[0f32, 0., 0.], [1., 0., 0.], [1., 1., 0.]] {
            for value in point {
                mesh.extend(value.to_le_bytes());
            }
        }
        mesh.extend([0u8; 3 * 20]); // UVs and normals.
        let mut region = [0u8; 35];
        region[..6].copy_from_slice(b"LAYOUT");
        region[33] = 1;
        mesh.extend(region);
        for _ in 0..2 {
            let mut part = [0u8; 20];
            part[6..8].copy_from_slice(&3u16.to_le_bytes());
            part[16..20].copy_from_slice(&2u32.to_le_bytes());
            mesh.extend(part);
        }
        mesh.extend(6u32.to_le_bytes());
        for index in [0u16, 1, 2, 0, 2, 1] {
            mesh.extend(index.to_le_bytes());
        }
        mesh.extend(b"alpha.tt\0");
        let mut texture = vec![0u8; 60];
        for (offset, value) in [
            (0, 1u32),
            (8, 1),
            (12, 1),
            (16, 1f32.to_bits()),
            (20, 1f32.to_bits()),
        ] {
            texture[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        texture.extend([80, 90, 100, 128]);
        let mut archive = mesh.clone();
        archive.extend(texture);
        let mut directory = vec![0u8; 32 + 2 * 308];
        directory[..4].copy_from_slice(&3u32.to_le_bytes());
        directory[4..8].copy_from_slice(&2u32.to_le_bytes());
        for (i, name, offset, length) in [
            (0, "panel.tm", 0, mesh.len()),
            (1, "alpha.tt", mesh.len(), archive.len() - mesh.len()),
        ] {
            let at = 32 + i * 308;
            directory[at..at + name.len()].copy_from_slice(name.as_bytes());
            directory[at + 260..at + 264].copy_from_slice(&(offset as u32).to_le_bytes());
            directory[at + 264..at + 268].copy_from_slice(&(length as u32).to_le_bytes());
        }
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        for (base, bounds) in [
            super::panel_base(&index, &archive, "panel.tm", &[0, 1]).unwrap(),
            super::panel_frame_base(&index, &archive, "panel.tm", &[0, 1], 0).unwrap(),
        ] {
            assert_eq!(base.len(), 1);
            assert_eq!(base[0].vertices.len(), 3);
            assert_eq!(base[0].alpha, [255; 3]);
            assert_eq!(base[0].pixels[3], 128);
            assert_eq!(bounds, [[0., -1., 1., -0.]]);
        }
    }

    #[test]
    #[ignore = "requires original v1.4 Graphics Options style9 fixture"]
    fn graphics_style9_matches_native_glyph_streams() {
        assert_style9_fixture(
            "NATIVE_GRAPHICS_STYLE9",
            &[1587, 1656, 1588, 1591, 1594, 1596, 1129, 1130],
        );
    }
    #[test]
    #[ignore = "requires original v1.4 Main Options style9 fixture"]
    fn main_options_style9_matches_native_glyph_streams() {
        assert_style9_fixture(
            "NATIVE_OPTIONS_MAIN_STYLE9",
            &[1679, 1607, 1608, 1609, 1610, 1770, 1435, 1139],
        );
    }
    #[test]
    #[ignore = "requires original v1.4 in-game Options style9 fixture"]
    fn game_options_style9_matches_native_glyph_streams() {
        assert_style9_fixture("NATIVE_GAME_OPTIONS_STYLE9", &[1678, 1680, 1679, 1682]);
    }
    #[test]
    #[ignore = "requires original v1.4 Tutorial caption fixture"]
    fn tutorial_captions_match_native_glyph_streams() {
        assert_style9_fixture("NATIVE_TUTORIAL_STYLE9", &[1667, 1668]);
    }
    fn assert_style9_fixture(path: &str, ids: &[u32]) {
        let data = std::fs::read(openmt2001::fixtures::path(path)).unwrap();
        assert_eq!(&data[..8], b"MTG9TX01");
        let mut at = 8usize;
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(data[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        assert_eq!(word(&mut at), 0x037f);
        let mut fonts = std::collections::HashMap::new();
        for _ in 0..word(&mut at) {
            let id = word(&mut at);
            let len = word(&mut at) as usize;
            fonts.insert(id, &data[at..at + len]);
            at += len;
        }
        assert_eq!(word(&mut at) as usize, ids.len());
        for &expected_id in ids {
            let id = word(&mut at);
            assert_eq!(id, expected_id);
            let font_id = word(&mut at);
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let len = word(&mut at) as usize;
            let encoded = &data[at..at + len];
            at += len;
            let mut pages = [super::Surface::solid(), super::Surface::solid()];
            super::text_widget_style9(&mut pages, fonts[&font_id], encoded, bounds, 0xffffffff)
                .unwrap();
            for (page, actual) in pages.iter().enumerate() {
                let count = word(&mut at) as usize;
                assert_eq!(
                    actual.vertices.len(),
                    count * 6,
                    "caption {id} page {page} count"
                );
                for glyph in 0..count {
                    for (vertex, index) in [0, 1, 2, 5].into_iter().enumerate() {
                        let got = actual.vertices[glyph * 6 + index];
                        for (axis, value) in got[..3].iter().enumerate() {
                            assert_eq!(
                                value.to_bits(),
                                word(&mut at),
                                "caption {id} page {page} glyph {glyph} vertex {vertex}:{axis}"
                            );
                        }
                    }
                }
                for glyph in 0..count {
                    for (vertex, index) in [0, 1, 2, 5].into_iter().enumerate() {
                        let got = actual.vertices[glyph * 6 + index];
                        for axis in 0..2 {
                            assert_eq!(
                                got[6 + axis].to_bits(),
                                word(&mut at),
                                "caption {id} page {page} glyph {glyph} UV {vertex}:{axis}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original v1.4 Gameplay Options caption fixture"]
    fn gameplay_captions_match_native_glyph_streams() {
        let data = std::fs::read(openmt2001::fixtures::path("NATIVE_GAMEPLAY_CAPTION")).unwrap();
        assert_eq!(&data[..8], b"MTGPTX01");
        let mut at = 8usize;
        let word = |at: &mut usize| {
            let value = u32::from_le_bytes(data[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        };
        assert_eq!(word(&mut at), 0x037f);
        let mut fonts = std::collections::HashMap::new();
        for _ in 0..word(&mut at) {
            let id = word(&mut at);
            let len = word(&mut at) as usize;
            fonts.insert(id, &data[at..at + len]);
            at += len;
        }
        assert_eq!(word(&mut at), 4);
        for expected_id in [1571, 1573, 1575, 1584] {
            let id = word(&mut at);
            assert_eq!(id, expected_id);
            let font_id = word(&mut at);
            let style = word(&mut at);
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let len = word(&mut at) as usize;
            let encoded = &data[at..at + len];
            at += len;
            let mut pages = [super::Surface::solid(), super::Surface::solid()];
            match style {
                3 => super::text_widget_style3(
                    &mut pages,
                    fonts[&font_id],
                    encoded,
                    bounds,
                    0xffffffff,
                ),
                9 => super::text_widget_style9(
                    &mut pages,
                    fonts[&font_id],
                    encoded,
                    bounds,
                    0xffffffff,
                ),
                _ => panic!("unexpected Gameplay caption style {style}"),
            }
            .unwrap();
            for (page, actual) in pages.iter().enumerate() {
                let count = word(&mut at) as usize;
                assert_eq!(
                    actual.vertices.len(),
                    count * 6,
                    "caption {id} page {page} count"
                );
                for glyph in 0..count {
                    for (vertex, index) in [0, 1, 2, 5].into_iter().enumerate() {
                        let got = actual.vertices[glyph * 6 + index];
                        for (axis, value) in got[..3].iter().enumerate() {
                            assert_eq!(
                                value.to_bits(),
                                word(&mut at),
                                "caption {id} page {page} glyph {glyph} vertex {vertex}:{axis}"
                            );
                        }
                    }
                }
                for glyph in 0..count {
                    for (vertex, index) in [0, 1, 2, 5].into_iter().enumerate() {
                        let got = actual.vertices[glyph * 6 + index];
                        for axis in 0..2 {
                            assert_eq!(
                                got[6 + axis].to_bits(),
                                word(&mut at),
                                "caption {id} page {page} glyph {glyph} UV {vertex}:{axis}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original MPP layout fixture"]
    fn mpp_mode_layout_matches_original() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_MPP_LAYOUT")).unwrap();
        assert_eq!(&bytes[..8], b"MTMPPLAY");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        for _ in 0..word(&mut at) {
            let bounds = std::array::from_fn(|_| f32::from_bits(word(&mut at)));
            let dimensions = std::array::from_fn(|_| word(&mut at));
            let layout = super::mpp_modes(bounds, dimensions).unwrap();
            assert_eq!(word(&mut at), 7);
            for rectangle in layout
                .buttons
                .into_iter()
                .chain([layout.header])
                .chain(layout.icons)
            {
                assert_eq!(
                    rectangle.map(f32::to_bits),
                    std::array::from_fn(|_| word(&mut at))
                );
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original MPP palette fixture"]
    fn mpp_runtime_palettes_match_original() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_MPP_STYLE")).unwrap();
        assert_eq!(&bytes[..8], b"MTMPP001");
        let mut at = 8;
        let word = |at: &mut usize| {
            let n = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            n
        };
        let count = word(&mut at);
        assert_eq!(count, 2);
        for _ in 0..count {
            for (name, colors) in super::mpp_palette(0xbb000a28) {
                let len = word(&mut at) as usize;
                assert_eq!(&bytes[at..at + len], name.as_bytes());
                at += len;
                assert_eq!(std::array::from_fn::<_, 4, _>(|_| word(&mut at)), colors);
            }
        }
        assert_eq!(at, bytes.len());
    }
}

#[cfg(any(test, feature = "test-support"))]
pub fn synthetic_text_font(width: i32, height: i32) -> Vec<u8> {
    let mut tab = 96u32.to_le_bytes().to_vec();
    for _ in 0..96 {
        tab.extend([0; 32]);
        tab.extend(width.to_le_bytes());
        tab.extend(height.to_le_bytes());
        tab.extend(0i32.to_le_bytes());
    }
    tab
}

#[test]
fn singleplayer_text_style_section_header_draws_palette_entry_16_mint() {
    let tab = synthetic_text_font(7, 10);
    let mut pages = vec![Surface::solid(); 2];
    section_header(&mut pages, &tab, &[33, 34], [10., 20., 110., 40.]).unwrap();
    assert_eq!(pages[0].vertices.len(), 12);
    let mint = [0x3a, 0xfd, 0x87].map(|c: u8| f32::from(c) / 255.);
    for vertex in &pages[0].vertices {
        for (actual, expected) in vertex[3..6].iter().zip(mint) {
            assert!((actual - expected).abs() < 1e-6, "{:?}", &vertex[3..6]);
        }
    }
    assert_eq!(pages[0].alpha, [255; 12]);
    assert_eq!(pages[0].vertices[0][0], 53.5);
    assert_eq!(pages[0].vertices[2][1], 19.5);
}

#[cfg(test)]
mod text_widget_style4_tests {
    use super::*;

    // Glyph cells 5 wide (advance 4 with the -1 gap) and 6 high, no rise;
    // glyph 2 is 4 wide so the odd width exercises the +0.5 rounding.
    fn synthetic_font() -> Vec<u8> {
        let mut tab = vec![0; 4 + 64 * 44];
        tab[..4].copy_from_slice(&64u32.to_le_bytes());
        for glyph in 0..64 {
            let at = 4 + glyph * 44;
            let width: i32 = if glyph == 2 { 4 } else { 5 };
            tab[at + 32..at + 36].copy_from_slice(&width.to_le_bytes());
            tab[at + 36..at + 40].copy_from_slice(&6i32.to_le_bytes());
        }
        tab
    }
    fn extent(pages: &[Surface]) -> [f32; 4] {
        pages.iter().flat_map(|p| p.vertices.iter()).fold(
            [f32::MAX, f32::MAX, f32::MIN, f32::MIN],
            |b, v| {
                [
                    b[0].min(v[0]),
                    b[1].min(v[1]),
                    b[2].max(v[0]),
                    b[3].max(v[1]),
                ]
            },
        )
    }

    #[test]
    fn style4_centres_a_title_in_the_main_panel_bar() {
        let tab = synthetic_font();
        // The block panel's title bar: panel content left/right, 14 high.
        let bar = [30., 57.6, 190., 71.6];
        let mut pages = vec![Surface::solid(), Surface::solid()];
        text_widget_style4(&mut pages, &tab, &[33; 5], bar, 0xffffffff).unwrap();
        // Width 20: x = 30 + trunc((160 - 20) / 2 + 0.5) = 100, and
        // y = trunc(57.6) + trunc((14 - 6) / 2) = 61, less the half pixel.
        assert_eq!(extent(&pages), [99.5, 60.5, 120.5, 66.5]);

        let mut pages = vec![Surface::solid(), Surface::solid()];
        text_widget_style4(&mut pages, &tab, &[2], bar, 0xffffffff).unwrap();
        // Width 3 rounds up to x 109; plain centring would start at 108.5.
        assert_eq!(extent(&pages)[0], 108.5);
    }
}
