//! Native UI surfaces; original asset parsing stays in shared core modules.
use openmt2001::{
    render::{SurfaceMode, TexturedDraw},
    texture::{TextureRecord, loader_pixels},
};

/// Original43a8c0. The symbol and result use the game's glyph-byte encoding.
/// NEG wraps at signed MIN, including the original double-negative rendering.
pub fn integer_money(value: i32, symbol: &[u8], suffix: bool) -> Vec<u8> {
    let magnitude = if value < 0 {
        value.wrapping_neg()
    } else {
        value
    };
    let digits = magnitude
        .to_string()
        .bytes()
        .map(|b| b - 32)
        .collect::<Vec<_>>();
    let mut text = Vec::new();
    if value < 0 {
        text.push(b'-' - 32);
    }
    if !suffix {
        text.extend_from_slice(symbol);
    }
    text.extend_from_slice(&digits);
    if suffix {
        text.extend_from_slice(symbol);
    }
    text
}

#[test]
#[ignore = "requires original integer currency observations"]
fn matches_native_integer_currency() {
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_INTEGER_CURRENCY").unwrap()).unwrap();
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
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_CURRENCY").unwrap()).unwrap();
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
    let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_FLOAT_MONEY").unwrap()).unwrap();
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
        for v in part.vertex_indices() {
            s.vertices
                .push([pos[v][0], -pos[v][1], 0.5, 1., 1., 1., uv[v][0], uv[v][1]]);
            s.alpha.push(255);
        }
        base.push(s);
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
    let (offsets, wrapped) = crate::prompt::wrap_with_gap(tab, encoded, width.trunc() as i32, -1)?;
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
        crate::prompt::wrap_with_gap(tab, encoded, span[0].trunc() as i32, -1)?;
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
        crate::prompt::wrap_with_gap(tab, encoded, span[0].trunc() as i32, -1)?;
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
    for q in font.layout(
        encoded,
        Layout {
            origin: [if center { (l + r - width) / 2. } else { l }, t],
            ..layout
        },
    )? {
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
    #[ignore = "requires original v1.4 Graphics Options style9 fixture"]
    fn graphics_style9_matches_native_glyph_streams() {
        assert_style9_fixture(
            "work/frontend/native-graphics-style9.bin",
            &[1587, 1656, 1588, 1591, 1594, 1596, 1129, 1130],
        );
    }
    #[test]
    #[ignore = "requires original v1.4 Main Options style9 fixture"]
    fn main_options_style9_matches_native_glyph_streams() {
        assert_style9_fixture(
            "work/frontend/native-options-main-style9.bin",
            &[1679, 1607, 1608, 1609, 1610, 1770, 1435, 1139],
        );
    }
    #[test]
    #[ignore = "requires original v1.4 in-game Options style9 fixture"]
    fn game_options_style9_matches_native_glyph_streams() {
        assert_style9_fixture(
            "work/frontend/native-game-options-style9.bin",
            &[1678, 1680, 1679, 1682],
        );
    }
    #[test]
    #[ignore = "requires original v1.4 Tutorial caption fixture"]
    fn tutorial_captions_match_native_glyph_streams() {
        assert_style9_fixture("work/frontend/native-tutorial-style9.bin", &[1667, 1668]);
    }
    fn assert_style9_fixture(path: &str, ids: &[u32]) {
        let data = std::fs::read(path).unwrap();
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
        let data = std::fs::read("work/frontend/native-gameplay-caption.bin").unwrap();
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
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_MPP_LAYOUT").unwrap()).unwrap();
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
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_MPP_STYLE").unwrap()).unwrap();
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
