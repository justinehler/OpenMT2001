//! Renderer-facing owned details for checked v1.4 appearance constructors.
use std::{collections::BTreeMap, sync::Arc};

use glam::{Mat4, Vec3, Vec4};
use openmt2001::{
    archive::ArchiveIndex,
    font::{Font, Layout},
    pe::PeImage,
    render::SurfaceMode,
    render_scene::OwnedDraw,
    save::appearance::{ModelIdentity, RendererRegistry, RendererTemplate},
    scene::SceneBuilding,
    texture::{TextureSet, loader_pixels},
};

#[derive(Clone)]
pub struct TexturePage {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

pub struct Assets {
    pub generic: Vec<TexturePage>,
    pub generic640: Vec<TexturePage>,
    pub building: Vec<TexturePage>,
    pub signs: Vec<TexturePage>,
    pub font4: Vec<TexturePage>,
    font4_tab: Vec<u8>,
    business_value: Vec<u8>,
    language_rows: Vec<Vec<u8>>,
}

impl Assets {
    pub fn from_original(directory: &[u8], archive: &[u8]) -> Result<Self, String> {
        let index = ArchiveIndex::parse(directory, archive.len() as u64)
            .map_err(|error| error.to_string())?;
        let load = |path| -> Result<Vec<TexturePage>, String> {
            let entry = index.find(path).ok_or_else(|| format!("missing appearance texture {path}"))?;
            let bytes = archive.get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("appearance texture exceeds archive")?;
            let set = TextureSet::parse(bytes).map_err(|error| error.to_string())?;
            Ok(set.textures.iter().map(|texture| TexturePage {
                width: texture.width,
                height: texture.height,
                rgba: Arc::from(loader_pixels(texture)),
            }).collect())
        };
        Ok(Self {
            generic: load("gamedata2/GenericTextures/textures.gtd")?,
            generic640: load("gamedata2/GenericTextures640/textures.gtd")?,
            building: load("gamedata2/BuildingTextures/textures.gtd")?,
            signs: load("gamedata2/SignTextures/textures.gtd")?,
            font4: load("gamedata2/font640/textures.gtd")?.into_iter().skip(8).take(2).collect(),
            font4_tab: Vec::new(),
            business_value: Vec::new(),
            language_rows: Vec::new(),
        })
    }

    /// Supply original font4 metrics and the localized row 0x45c used by
    /// 485200's 0x01010201 constructor. The archive supplies font pages.
    pub fn with_font4(mut self, tab: &[u8], business_value: &[u8]) -> Result<Self, String> {
        Font::parse(tab)?;
        if self.font4.len() != 2 || business_value.is_empty() || business_value.len() > 100 {
            return Err("appearance font4 pages or localized label unavailable".into());
        }
        self.font4_tab = tab.to_vec();
        self.business_value = business_value.to_vec();
        Ok(self)
    }

    pub fn with_language(mut self, tab: &[u8], offsets: &[u8], text: &[u8])
        -> Result<Self, String> {
        if !offsets.len().is_multiple_of(2) { return Err("partial appearance language length".into()); }
        let mut at = 0usize;
        for row in offsets.chunks_exact(2) {
            let end = at.checked_add(usize::from(u16::from_le_bytes(row.try_into().unwrap())))
                .ok_or("appearance language length overflow")?;
            self.language_rows.push(text.get(at..end)
                .ok_or("appearance language row leaves text")?.to_vec());
            at = end;
        }
        if at != text.len() { return Err("appearance language lengths do not cover text".into()); }
        let label = self.language_rows.get(0x45c).ok_or("appearance City label missing")?.clone();
        self.with_font4(tab, &label)
    }

    ///4e7690 maps direct C-string bytes through the first language row;
    ///4e8520 resolves a business name ID from the same original table.
    pub fn refresh_labels(&self, owner_name: Option<&[u8]>, business_id: u32)
        -> Result<RefreshLabels, String> {
        let owner = if let Some(name) = owner_name {
            let map = self.language_rows.first().ok_or("appearance character map missing")?;
            name.iter().map(|&byte| byte.checked_sub(32)
                .and_then(|index| map.get(usize::from(index)).copied())
                .ok_or_else(|| "appearance owner character outside map".to_owned()))
                .collect::<Result<Vec<_>, _>>()?
        } else { self.business_value.clone() };
        let business = self.language_rows.get(business_id as usize)
            .ok_or("appearance business name ID outside language")?.clone();
        Ok(RefreshLabels { owner, business, banner: None })
    }
}

///485520 uses owner metadata for10000 and BusinessInfo+4 for20000.
pub struct RefreshLabels {
    pub owner: Vec<u8>,
    pub business: Vec<u8>,
    /// Required only when refreshing01080202; text labels alone claim no selector.
    pub banner: Option<BannerSelection>,
}

/// Original4846f0 banner selectors are distinct from the ordinary texture class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BannerSelection {
    owner_style: u32,
    business_page: Option<usize>,
}
impl BannerSelection {
    //485368 passes selectors0/0 even while its ordinary texture class is10.
    pub const INITIAL: Self = Self { owner_style: 0, business_page: Some(0) };

    ///485520 supplies owner metadata (or10 for City), and business kind (or-1).
    pub fn refreshed(owner_style: u32, business_kind: i32) -> Result<Self, String> {
        if owner_style > 10 || !(-1..45).contains(&business_kind) {
            return Err("appearance banner owner/business selector outside recovered domain".into());
        }
        Ok(Self { owner_style,
            business_page: (business_kind >= 0).then_some(business_kind as usize) })
    }
}

/// Original 457210 selects a generic front and BuildingTextures back using
/// two PE DWORD tables, indexed by the 4846f0 appearance texture class.
pub fn sign_material_indices(image: &PeImage<'_>, class: u32) -> Result<(usize, usize), String> {
    if class > 10 {
        return Err(format!("appearance sign texture class {class} is unverified"));
    }
    let offset = class.checked_mul(4).ok_or("appearance class offset overflow")?;
    let read = |address| -> Result<usize, String> {
        Ok(u32::from_le_bytes(image.at(address, 4)?.try_into().unwrap()) as usize)
    };
    Ok((
        read(0x53a228 + offset)?,
        read(0x53a1d0 + offset)?,
    ))
}

/// Original 4575b0 chooses both curved-body and hanging-trim pages from
/// BuildingTextures. Classes 0 and 10 have native constructor checks;
/// the intervening classes use the same geometry and indexed PE tables.
pub fn awning_material_indices(image: &PeImage<'_>, class: u32) -> Result<(usize, usize), String> {
    if class > 10 {
        return Err(format!("appearance awning texture class {class} is unverified"));
    }
    let offset = class.checked_mul(4).ok_or("appearance class offset overflow")?;
    let read = |address| -> Result<usize, String> {
        Ok(u32::from_le_bytes(image.at(address, 4)?.try_into().unwrap()) as usize)
    };
    Ok((read(0x53a1d0 + offset)?, read(0x53a1fc + offset)?))
}

///457c60 selects an owner backing/portrait and an optional business lower pair.
/// Modern640-family UI textures retain the original handle-bank indices.
pub fn banner_material_indices(image: &PeImage<'_>, selection: BannerSelection,
) -> Result<(usize, usize, Option<usize>), String> {
    let read = |address| -> Result<usize, String> {
        Ok(u32::from_le_bytes(image.at(address, 4)?.try_into().unwrap()) as usize)
    };
    let portrait = if selection.owner_style == 10 {
        //68b2a0 is generic UI bank68ae28+286*4; no character record10 lookup.
        286
    } else { read(0x541af8 + selection.owner_style * 40)? };
    Ok((read(0x53a254 + selection.owner_style * 4)?, portrait, selection.business_page))
}

#[derive(Clone)]
pub struct SignGeometry {
    pub positions: [[f32; 3]; 8],
    pub uv: [[f32; 2]; 8],
    pub indices: [u16; 12],
}

/// Original 455db0 emits two four-vertex quads through 457210. The local
/// model is centered; the caller must supply the verified world/detail pose.
pub fn sign_geometry(width: f32, height: f32) -> Result<SignGeometry, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return Err("appearance sign dimensions must be positive finite".into());
    }
    let left = -(width * 0.5);
    let right = width * 0.5;
    let bottom = -(height * 0.5);
    let top = height * 0.5;
    let mut positions = [[0.; 3]; 8];
    for (layer, z) in [0.2f32, 0.1f32].into_iter().enumerate() {
        for (i, (x, y)) in [(left, bottom), (right, bottom), (left, top), (right, top)]
            .into_iter().enumerate()
        {
            positions[layer * 4 + i] = [x, y, z];
        }
    }
    Ok(SignGeometry {
        positions,
        uv: [
            [1., 1.], [0., 1.], [1., 0.], [0., 0.],
            [0.45, 0.45], [0.55, 0.45], [0.45, 0.55], [0.55, 0.55],
        ],
        indices: [0, 1, 2, 1, 3, 2, 4, 5, 6, 5, 7, 6],
    })
}

/// Original 457490/457210 with its double-sided flag repeats the same two
/// quads with opposite winding, retaining the original per-page grouping.
pub fn double_sign_indices() -> [u16; 24] {
    [0, 1, 2, 1, 3, 2, 1, 0, 2, 3, 1, 2,
     4, 5, 6, 5, 7, 6, 5, 4, 6, 7, 5, 6]
}

pub struct TextSignGeometry {
    pub positions: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub colors: Vec<u32>,
    pub indices: Vec<u16>,
    pub font_page_indices: [usize; 2],
}

/// Original 4574c0/456d40 renders font4 glyph quads, recenters their bounds,
/// scales the text into 90% x 85% of the sign, and emits a backing quad.
/// Input bytes are original glyph indices after 4e7690's character map.
pub fn text_sign_geometry(tab: &[u8], glyphs: &[u8], width: f32, height: f32)
    -> Result<TextSignGeometry, String> {
    if glyphs.is_empty() || glyphs.len() > 100 {
        return Err("appearance text sign has invalid glyph count".into());
    }
    let backing = sign_geometry(width, height)?;
    let quads = Font::parse(tab)?.layout(glyphs, Layout {
        origin: [0., 0.], depth: 0., scale: 1., gap: -1, baseline: 0,
    })?;
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    for quad in &quads {
        for point in quad.positions {
            low[0] = low[0].min(point[0]);
            high[0] = high[0].max(point[0]);
            low[1] = low[1].min(-point[1]);
            high[1] = high[1].max(-point[1]);
        }
    }
    let span = [high[0] - low[0], high[1] - low[1]];
    if !span.into_iter().all(|v| v.is_finite() && v > 0.) {
        return Err("appearance text sign has empty or nonfinite glyph bounds".into());
    }
    let center = [(low[0] + high[0]) * 0.5, (low[1] + high[1]) * 0.5];
    let sx = ((f64::from(width) / f64::from(span[0]))
        * f64::from(f32::from_bits(0x3f66_6666))) as f32;
    let sy = ((f64::from(height) / f64::from(span[1]))
        * f64::from(f32::from_bits(0x3f59_999a))) as f32;
    let mut positions = Vec::with_capacity(quads.len() * 4 + 4);
    let mut uv = Vec::with_capacity(quads.len() * 4 + 4);
    let mut colors = Vec::with_capacity(quads.len() * 4 + 4);
    let mut groups = [Vec::<u16>::new(), Vec::new()];
    for quad in quads {
        let start = u16::try_from(positions.len()).map_err(|_| "appearance text vertex overflow")?;
        for (point, tex) in quad.positions.into_iter().zip(quad.uv) {
            positions.push([(point[0] - center[0]) * sx,
                            (-point[1] - center[1]) * sy, 0.2]);
            uv.push(tex);
            colors.push(0xffff_ffff);
        }
        groups[quad.page].extend_from_slice(&[
            start, start+1, start+2, start, start+2, start+3,
        ]);
    }
    let backing_start = u16::try_from(positions.len()).map_err(|_| "appearance text backing overflow")?;
    positions.extend_from_slice(&backing.positions[4..8]);
    uv.extend_from_slice(&backing.uv[4..8]);
    colors.extend_from_slice(&[0xffff_fffe; 4]);
    let font_page_indices = [groups[0].len(), groups[1].len()];
    let mut indices = groups.into_iter().flatten().collect::<Vec<_>>();
    indices.extend_from_slice(&[backing_start, backing_start+1, backing_start+2,
                                backing_start+1, backing_start+3, backing_start+2]);
    Ok(TextSignGeometry { positions, uv, colors, indices, font_page_indices })
}

/// Native 4574f0 uses the same 456d40 text geometry as 4574c0, then emits
/// the back of the sign with opposite winding. The original v1.4 model311
/// constructor confirms identical positions, UVs, colors and front indices.
pub fn double_backed_text_sign_geometry(
    tab: &[u8], glyphs: &[u8], width: f32, height: f32,
) -> Result<TextSignGeometry, String> {
    let mut geometry = text_sign_geometry(tab, glyphs, width, height)?;
    let start = u16::try_from(geometry.positions.len() - 4)
        .map_err(|_| "appearance text backing vertex overflow")?;
    geometry.indices.extend_from_slice(&[
        start + 1, start, start + 2, start + 3, start + 1, start + 2,
    ]);
    Ok(geometry)
}

/// Original 456440 builds the tall Business Value sign with one centered
/// font4 glyph per line. 457520 draws the front, 457550 also reverses the
/// frame, and 457580 adds the rotated back text. The font metrics and f32
/// stores follow the bounded v1.4 4561f0/4e7f10 replay.
pub fn vertical_text_sign_geometry(
    tab: &[u8],
    glyphs: &[u8],
    width: f32,
    height: f32,
    frame_reverse: bool,
    text_back: bool,
) -> Result<TextSignGeometry, String> {
    if glyphs.is_empty()
        || glyphs.len() > 100
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.
        || height <= 0.
    {
        return Err("appearance vertical text dimensions or glyphs invalid".into());
    }
    Font::parse(tab)?;
    let mut positions = Vec::with_capacity(glyphs.len() * if text_back { 8 } else { 4 } + 4);
    let mut uv = Vec::with_capacity(positions.capacity());
    let mut colors = Vec::with_capacity(positions.capacity());
    let mut page_indices = [Vec::<u16>::new(), Vec::new()];
    let mut cursor = 0f32;
    for &glyph in glyphs {
        let offset = 4 + usize::from(glyph) * 44;
        let record = tab
            .get(offset..offset + 44)
            .ok_or("vertical text glyph leaves font4")?;
        let metric = |at| i32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let gw = metric(32);
        let gh = metric(36);
        let left = (-(f64::from(gw) * 0.5)) as f32;
        let right = (f64::from(gw) * 0.5) as f32;
        let bottom = (f64::from(cursor) - f64::from(gh)) as f32;
        let start = u16::try_from(positions.len()).map_err(|_| "vertical text vertex overflow")?;
        positions.extend_from_slice(&[
            [left, bottom, 0.],
            [right, bottom, 0.],
            [right, cursor, 0.],
            [left, cursor, 0.],
        ]);
        for i in 0..4 {
            uv.push(std::array::from_fn(|axis| {
                let at = (i * 2 + axis) * 4;
                f32::from_le_bytes(record[at..at + 4].try_into().unwrap())
            }));
        }
        colors.extend_from_slice(&[0xffff_ffff; 4]);
        page_indices[usize::from(glyph >= 95)].extend_from_slice(&[
            start,
            start + 1,
            start + 2,
            start,
            start + 2,
            start + 3,
        ]);
        cursor =
            (f64::from(cursor) - f64::from(gh) * f64::from(f32::from_bits(0x3f99_999a))) as f32;
    }
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    for vertex in &positions {
        for axis in 0..2 {
            low[axis] = low[axis].min(vertex[axis]);
            high[axis] = high[axis].max(vertex[axis]);
        }
    }
    let span = [high[0] - low[0], high[1] - low[1]];
    if !span
        .into_iter()
        .all(|value| value.is_finite() && value > 0.)
    {
        return Err("appearance vertical text has empty bounds".into());
    }
    let offset = [
        -((f64::from(high[0]) + f64::from(low[0])) * 0.5) as f32,
        -((f64::from(high[1]) + f64::from(low[1])) * 0.5) as f32,
    ];
    let scale = [
        (f64::from(width) / f64::from(span[0])) as f32,
        (f64::from(height) / f64::from(span[1])) as f32,
    ];
    for vertex in &mut positions {
        vertex[0] = ((f64::from(vertex[0]) + f64::from(offset[0])) as f32 as f64
            * f64::from(scale[0])) as f32;
        vertex[1] = ((f64::from(vertex[1]) + f64::from(offset[1])) as f32 as f64
            * f64::from(scale[1])) as f32;
        vertex[2] = 0.2;
    }
    if text_back {
        let front = positions.clone();
        let angle = f64::from(180f32) * f64::from(f32::from_bits(0x3c8e_fa35));
        let cosine = angle.cos() as f32;
        let sine = angle.sin() as f32;
        for vertex in front {
            positions.push([
                (f64::from(vertex[0]) * f64::from(cosine) + f64::from(vertex[2]) * f64::from(sine))
                    as f32,
                vertex[1],
                (f64::from(vertex[2]) * f64::from(cosine) - f64::from(vertex[0]) * f64::from(sine))
                    as f32,
            ]);
        }
        let original_uv = uv.clone();
        uv.extend(original_uv);
        colors.extend_from_within(..);
        let copy_base =
            u16::try_from(glyphs.len() * 4).map_err(|_| "vertical text back vertex overflow")?;
        for indices in &mut page_indices {
            let original = indices.clone();
            indices.extend(original.into_iter().map(|index| index + copy_base));
        }
    }
    let frame = u16::try_from(positions.len()).map_err(|_| "vertical frame vertex overflow")?;
    positions.extend_from_slice(&[
        [-(width * 0.5), -(height * 0.5), 0.],
        [width * 0.5, -(height * 0.5), 0.],
        [-(width * 0.5), height * 0.5, 0.1],
        [width * 0.5, height * 0.5, 0.1],
    ]);
    uv.extend_from_slice(&[[0.5, 0.5], [4.5, 0.5], [0.5, 12.5], [4.5, 12.5]]);
    colors.extend_from_slice(&[0xffff_0000; 4]);
    let font_page_indices = [page_indices[0].len(), page_indices[1].len()];
    let mut indices = page_indices.into_iter().flatten().collect::<Vec<_>>();
    indices.extend_from_slice(&[frame, frame + 1, frame + 2, frame + 1, frame + 3, frame + 2]);
    if frame_reverse {
        indices.extend_from_slice(&[frame + 1, frame, frame + 2, frame + 3, frame + 1, frame + 2]);
    }
    Ok(TextSignGeometry {
        positions,
        uv,
        colors,
        indices,
        font_page_indices,
    })
}

#[derive(Clone)]
pub struct AwningGeometry {
    pub positions: [[f32; 3]; 42],
    pub uv: [[f32; 2]; 42],
    pub indices: [u16; 102],
}

/// Original 4575b0 makes eight quarter-circle strips, two end caps, and a
/// short hanging flap. The slight difference between strip and cap vertices
/// follows the original x87 angle accumulation and f32 store order.
pub fn awning_geometry(width: f32, height: f32) -> Result<AwningGeometry, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return Err("appearance awning dimensions must be positive finite".into());
    }
    let span = width * 0.9f32;
    let half = span * 0.5f32;
    let v_step = height * 1.5f32 * 0.125f32;
    let step = f32::from_bits(0x3e49_0e56);
    let mut positions = [[0.; 3]; 42];
    let mut uv = [[0.; 2]; 42];
    let mut indices = [0u16; 102];
    let mut side_angle = step;
    for segment in 0..=8 {
        let (y, z) = if segment == 0 {
            (height, 0.)
        } else if segment == 1 {
            ((f64::from(step).cos() * f64::from(height)) as f32,
             (f64::from(step).sin() * f64::from(height)) as f32)
        } else {
            let extended = f64::from(side_angle) + f64::from(step);
            side_angle = extended as f32;
            ((f64::from(side_angle).cos() * f64::from(height)) as f32,
             (extended.sin() * f64::from(height)) as f32)
        };
        for (side, x) in [half, -half].into_iter().enumerate() {
            let index = segment * 2 + side;
            positions[index] = [x, y, z];
            uv[index] = [if side == 0 { 0. } else { span }, segment as f32 * v_step];
        }
        if segment < 8 {
            let at = segment * 6;
            let a = (segment * 2) as u16;
            indices[at..at + 6].copy_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
        }
    }
    for (cap, x) in [-half, half].into_iter().enumerate() {
        let base = 18 + cap * 10;
        positions[base] = [x, 0., 0.];
        positions[base + 1] = [x, height, 0.];
        uv[base] = [0.45, 0.45];
        uv[base + 1] = [0.45, 0.45];
        let mut angle = 0f64;
        for segment in 1..=8 {
            angle += f64::from(step);
            positions[base + 1 + segment] = [x,
                (angle.cos() * f64::from(height)) as f32,
                (angle.sin() * f64::from(height)) as f32];
            uv[base + 1 + segment] = [0.5, 0.5];
        }
        for segment in 0..8 {
            let at = 48 + cap * 24 + segment * 3;
            let rim = (base + 1 + segment) as u16;
            let triangle = if cap == 0 {
                [(base as u16), rim + 1, rim]
            } else {
                [(base as u16), rim, rim + 1]
            };
            indices[at..at + 3].copy_from_slice(&triangle);
        }
    }
    let last = positions[16];
    positions[38] = last;
    positions[39] = positions[17];
    positions[40] = [last[0], last[1] - 0.5f32, last[2]];
    positions[41] = [positions[39][0], positions[39][1] - 0.5f32, positions[39][2]];
    uv[38..42].copy_from_slice(&[[0., 0.0625], [span, 0.0625],
                                 [0., 0.9375], [span, 0.9375]]);
    indices[96..].copy_from_slice(&[38, 39, 40, 39, 41, 40]);
    Ok(AwningGeometry { positions, uv, indices })
}

#[derive(Clone)]
pub struct BannerGeometry {
    pub positions: [[f32; 3]; 20],
    pub uv: [[f32; 2]; 20],
    pub indices: [u16; 36],
}

/// Original 457c60 for saved descriptor 01080202 with selector 0/0: a
/// double-sided backing and two double-sided sign faces, three materials.
pub fn banner_geometry(width: f32, height: f32) -> Result<BannerGeometry, String> {
    if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
        return Err("appearance banner dimensions must be positive finite".into());
    }
    let w = width * 0.9f32;
    let h = height * 0.9f32;
    let base = height * 0.7f32;
    let mut positions = [[0.; 3]; 20];
    let mut uv = [[0.; 2]; 20];
    let mut indices = [0u16; 36];
    let mut quad = |part: usize, x0: f32, x1: f32, y0: f32, y1: f32, z: f32| {
        let at = part * 4;
        positions[at..at + 4].copy_from_slice(&[
            [x0, y0, z], [x1, y0, z], [x0, y1, z], [x1, y1, z],
        ]);
        uv[at..at + 4].copy_from_slice(&[
            [0., 1.], [1., 1.], [0., 0.], [1., 0.],
        ]);
    };
    quad(0, w * -0.45f32, w * 0.45f32,
         base - h * 0.7f32, base + h * 0.45f32, 0.);
    quad(1, w * -0.15f32, w * 0.15f32,
         base - h * 0.1f32, base + h * 0.35f32, 0.1);
    quad(2, w * 0.15f32, w * -0.15f32,
         base - h * 0.1f32, base + h * 0.35f32, -0.1);
    quad(3, w * -0.35f32, w * 0.35f32,
         base - h * 0.6f32, base - h * 0.2f32, 0.1);
    quad(4, w * 0.35f32, w * -0.35f32,
         base - h * 0.6f32, base - h * 0.2f32, -0.1);
    indices[..12].copy_from_slice(&double_sign_indices()[..12]);
    for part in 1..=4 {
        let at = 12 + (part - 1) * 6;
        let base = (part * 4) as u16;
        indices[at..at + 6].copy_from_slice(&[
            base, base + 1, base + 2, base + 1, base + 3, base + 2,
        ]);
    }
    Ok(BannerGeometry { positions, uv, indices })
}

/// 488230's post-485200 detail origin. Registry Z is the negative of raw
/// model Z; the shared building transform bakes transpose/mirror/rotation,
/// then the original scene translation adds center X/Y and flips local Z.
pub fn saved_detail_origin(building: &SceneBuilding, row: &RendererTemplate) -> Result<[f32; 3], String> {
    let component = |at| f32::from_le_bytes(row.row[at..at + 4].try_into().unwrap());
    let local = building.local_position([component(12), component(16), -component(20)]);
    let world = [local[0] + building.center[0], local[1] + 0.5,
                 building.center[1] - local[2]];
    if !world.into_iter().all(f32::is_finite) {
        return Err("saved detail world origin is nonfinite".into());
    }
    Ok(world)
}

/// Original 488230 updates renderer row +0x24 after the building's optional
/// mirror and rotation. The two f32 stores preserve its native update order.
pub fn saved_detail_rotation_degrees(building: &SceneBuilding, row: &RendererTemplate) -> Result<f32, String> {
    let mut angle = f32::from_le_bytes(row.row[36..40].try_into().unwrap());
    if building.mirror {
        angle = 180.0f32 - angle;
    }
    angle -= building.angle;
    if !angle.is_finite() {
        return Err("saved detail world rotation is nonfinite".into());
    }
    Ok(angle)
}

/// A renderer row is rotated in original model space, then the shared scene
/// path reverses Z for V2. Original 4e0b50 stores the negated yaw as f32
/// radians before 4e3380 builds the row-vector matrix.
pub fn saved_detail_pose(building: &SceneBuilding, row: &RendererTemplate) -> Result<Mat4, String> {
    let [x, y, z] = saved_detail_origin(building, row)?;
    let degrees = saved_detail_rotation_degrees(building, row)?;
    let mut pose = saved_detail_basis(degrees);
    pose.w_axis = Vec4::new(x, y, z, 1.);
    Ok(pose)
}

fn saved_detail_basis(degrees: f32) -> Mat4 {
    let native_radians = -degrees * f32::from_bits(0x3c8e_fa35);
    let (sine, cosine) = f64::from(native_radians).sin_cos();
    let sine = sine as f32;
    let cosine = cosine as f32;
    // Native row-vector basis is [cos, 0, -sin], [0, 1, 0],
    // [sin, 0, cos] for its negative-radians input. The output's Z flip
    // maps its X and Z basis into these V2 column vectors.
    Mat4::from_cols(
        Vec4::new(cosine, 0., sine, 0.),
        Vec4::Y,
        Vec4::new(sine, 0., -cosine, 0.),
        Vec4::W,
    )
}

/// Stable V2 handles own one or more detail draws. Releasing a handle drops
/// its CPU draw data before the replacement is inserted by 485520's caller.
#[derive(Default)]
pub struct Adapter {
    next: u32,
    draws: BTreeMap<u32, Vec<OwnedDraw>>,
}

type PreparedDetail<'a> = (
    Vec<[f32; 3]>,
    Vec<[f32; 2]>,
    Vec<u32>,
    Vec<u16>,
    Vec<(&'a TexturePage, std::ops::Range<usize>)>,
);

impl Adapter {
    pub fn construct(
        &mut self,
        template: &RendererTemplate,
        class: u32,
        world_from_detail: Mat4,
        image: &PeImage<'_>,
        assets: &Assets,
    ) -> Result<u32, String> {
        self.construct_with_labels(template, class, world_from_detail, image, assets, None)
    }

    fn construct_with_labels(
        &mut self,
        template: &RendererTemplate,
        class: u32,
        world_from_detail: Mat4,
        image: &PeImage<'_>,
        assets: &Assets,
        labels: Option<&RefreshLabels>,
    ) -> Result<u32, String> {
        if u32::from_le_bytes(template.row[..4].try_into().unwrap()) != 1 {
            return Err(format!("appearance detail {:08x} has no draw constructor", template.descriptor));
        }
        if !world_from_detail.to_cols_array().into_iter().all(f32::is_finite) {
            return Err("appearance detail pose is nonfinite".into());
        }
        let word = |at| u32::from_le_bytes(template.row[at..at + 4].try_into().unwrap());
        let width = f32::from_bits(word(44));
        let height = f32::from_bits(word(48));
        let (positions, uv, colors, indices, parts): PreparedDetail<'_> = match template.descriptor {
            0x0104_0201 | 0x0104_0202 | 0x0104_0204 => {
                let geometry = sign_geometry(width, height)?;
                let (generic, building) = sign_material_indices(image, class)?;
                let (indices, split) = if template.descriptor != 0x0104_0201 {
                    (double_sign_indices().to_vec(), 12)
                } else {
                    (geometry.indices.to_vec(), 6)
                };
                (geometry.positions.to_vec(), geometry.uv.to_vec(), vec![0xffff_ffff; 8], indices,
                 vec![(assets.generic.get(generic).ok_or("appearance generic texture leaves collection")?, 0..split),
                      (assets.building.get(building).ok_or("appearance building texture leaves collection")?, split..split*2)])
            }
            0x0200_0001 => {
                let geometry = awning_geometry(width, height)?;
                let (body, trim) = awning_material_indices(image, class)?;
                (geometry.positions.to_vec(), geometry.uv.to_vec(), vec![0xffff_ffff; 42], geometry.indices.to_vec(),
                 vec![(assets.building.get(body).ok_or("appearance awning body texture leaves collection")?, 0..96),
                      (assets.building.get(trim).ok_or("appearance awning trim texture leaves collection")?, 96..102)])
            }
            0x0108_0202 => {
                let selection = match labels {
                    None => BannerSelection::INITIAL,
                    Some(payload) => payload.banner.ok_or("appearance banner refresh selectors missing")?,
                };
                let geometry = banner_geometry(width, height)?;
                let (outer, portrait, lower) = banner_material_indices(image, selection)?;
                let mut parts = vec![
                    (assets.signs.get(outer).ok_or("appearance banner backing texture leaves collection")?, 0..12),
                    (assets.generic640.get(portrait).ok_or("appearance banner portrait texture leaves collection")?, 12..24),
                ];
                if let Some(lower) = lower {
                    parts.push((assets.signs.get(lower).ok_or("appearance banner lower texture leaves collection")?, 24..36));
                }
                let vertices = if lower.is_some() { 20 } else { 12 };
                let indices = if lower.is_some() { 36 } else { 24 };
                (geometry.positions[..vertices].to_vec(), geometry.uv[..vertices].to_vec(),
                 vec![0xffff_ffff; vertices], geometry.indices[..indices].to_vec(), parts)
            }
            0x0101_0201 | 0x0102_0201 | 0x0101_0204 | 0x0102_0204 => {
                if assets.font4_tab.is_empty() {
                    return Err("appearance font4 metrics were not supplied".into());
                }
                let static_label = b"TO-LET".iter().map(|&byte| byte - 32).collect::<Vec<_>>();
                let glyphs = if let Some(labels) = labels {
                    if template.descriptor & 0x0002_0000 != 0 { &labels.business } else { &labels.owner }
                } else if matches!(template.descriptor, 0x0102_0201 | 0x0102_0204) {
                    &static_label
                } else {
                    &assets.business_value
                };
                let geometry = if matches!(template.descriptor, 0x0101_0204 | 0x0102_0204) {
                    double_backed_text_sign_geometry(&assets.font4_tab, glyphs, width, height)?
                } else {
                    text_sign_geometry(&assets.font4_tab, glyphs, width, height)?
                };
                let (_, backing) = sign_material_indices(image, class)?;
                let [first, second] = geometry.font_page_indices;
                let mut parts = Vec::new();
                if first > 0 {
                    parts.push((assets.font4.first().ok_or("appearance font4 first page missing")?, 0..first));
                }
                if second > 0 {
                    parts.push((assets.font4.get(1).ok_or("appearance font4 second page missing")?, first..first+second));
                }
                parts.push((assets.building.get(backing).ok_or("appearance text backing texture leaves collection")?,
                            first+second..geometry.indices.len()));
                (geometry.positions, geometry.uv, geometry.colors, geometry.indices, parts)
            }
            0x0101_0101 | 0x0102_0101 | 0x0101_0102 | 0x0102_0102 | 0x0101_0104 => {
                if assets.font4_tab.is_empty() {
                    return Err("appearance font4 metrics were not supplied".into());
                }
                let (frame_reverse, text_back) = match template.descriptor {
                    0x0101_0101 | 0x0102_0101 => (false, false),
                    0x0101_0104 => (true, false),
                    _ => (true, true),
                };
                // 485200 selects localized Business Value for the 0x10000
                // text bit and static TO-LET for 0x20000 before 4846f0.
                let static_label = b"TO-LET".iter().map(|&byte| byte - 32).collect::<Vec<_>>();
                let glyphs = if let Some(labels) = labels {
                    if template.descriptor & 0x0002_0000 != 0 { &labels.business } else { &labels.owner }
                } else if matches!(template.descriptor, 0x0102_0101 | 0x0102_0102) {
                    &static_label
                } else {
                    &assets.business_value
                };
                let geometry = vertical_text_sign_geometry(
                    &assets.font4_tab,
                    glyphs,
                    width,
                    height,
                    frame_reverse,
                    text_back,
                )?;
                let [first, second] = geometry.font_page_indices;
                let mut parts = Vec::new();
                if first > 0 {
                    parts.push((
                        assets
                            .font4
                            .first()
                            .ok_or("vertical sign font4 first page missing")?,
                        0..first,
                    ));
                }
                if second > 0 {
                    parts.push((
                        assets
                            .font4
                            .get(1)
                            .ok_or("vertical sign font4 second page missing")?,
                        first..first + second,
                    ));
                }
                // 4c10f0 fills the BuildingTextures pointer table at 689620;
                // 456440 reads its fourth entry at 68962c for this frame.
                parts.push((
                    assets
                        .building
                        .get(3)
                        .ok_or("vertical sign BuildingTextures frame index 3 missing")?,
                    first + second..geometry.indices.len(),
                ));
                (
                    geometry.positions,
                    geometry.uv,
                    geometry.colors,
                    geometry.indices,
                    parts,
                )
            }
            _ => return Err(format!("appearance detail {:08x} has no draw constructor", template.descriptor)),
        };
        let mut draws = Vec::with_capacity(parts.len());
        for (page, range) in parts {
            let mut draw = OwnedDraw::texture_page(
                SurfaceMode::StaticLit, page.width, page.height, page.rgba.clone());
            draw.mipmaps = page.width.is_power_of_two()
                && page.height.is_power_of_two()
                && u64::from(page.width) * u64::from(page.height) * 4
                    == page.rgba.len() as u64;
            draw.specular = None;
            for &index in &indices[range] {
                let i = index as usize;
                let p = world_from_detail.transform_point3(Vec3::from_array(positions[i]));
                let tex = uv[i];
                let color = colors[i];
                draw.vertices.push([p.x, p.y, p.z,
                    f32::from((color >> 16) as u8) / 255.,
                    f32::from((color >> 8) as u8) / 255.,
                    f32::from(color as u8) / 255., tex[0], tex[1]]);
            }
            draws.push(draw);
        }
        let handle = self.next.checked_add(1).ok_or("appearance draw handles exhausted")?;
        self.next = handle;
        self.draws.insert(handle, draws);
        Ok(handle)
    }

    pub fn release(&mut self, handle: u32) -> Result<(), String> {
        self.draws.remove(&handle)
            .ok_or_else(|| format!("appearance draw handle {handle} is not owned"))?;
        Ok(())
    }

    pub fn draws(&self, handle: u32) -> Option<&[OwnedDraw]> {
        self.draws.get(&handle).map(Vec::as_slice)
    }
}

/// Materialize the registry created during tag-4 binding, then execute the
/// later 4bbd20/485520 callbacks in their category-list order. Handles are
/// V2-owned; all live draws are copied into the resulting scene before this
/// temporary registry is dropped. Scene rebuilds repeat this construction.
pub fn restored_draws(
    buildings: &BTreeMap<usize, (SceneBuilding, ModelIdentity, u32)>,
    callbacks: &[usize],
    image: &PeImage<'_>,
    assets: &Assets,
    labels: &BTreeMap<usize, RefreshLabels>,
) -> Result<(Vec<OwnedDraw>, usize), String> {
    let (draws, replacements, _) = restored_draws_with_spans(
        buildings, callbacks, image, assets, labels)?;
    Ok((draws, replacements))
}

/// The original category renderer walks every descriptor row in slot order.
/// Retain each materialized row's draw range and descriptor translation so
/// its native distance scale can act around that row's own Y pivot.
pub type RestoredDrawsWithSpans =
    (Vec<OwnedDraw>, usize, Vec<(usize, std::ops::Range<usize>, f32)>);

pub fn restored_draws_with_spans(
    buildings: &BTreeMap<usize, (SceneBuilding, ModelIdentity, u32)>,
    callbacks: &[usize],
    image: &PeImage<'_>,
    assets: &Assets,
    labels: &BTreeMap<usize, RefreshLabels>,
) -> Result<RestoredDrawsWithSpans, String> {
    let adapter = std::cell::RefCell::new(Adapter::default());
    let mut registries = BTreeMap::new();
    for (&slot, (building, identity, _)) in buildings {
        let registry = RendererRegistry::prepare(identity, |row| {
            let pose = saved_detail_pose(building, row)?;
            //485368 supplies10 independently of the eventual owner metadata.
            adapter.borrow_mut().construct(row, 10, pose, image, assets)
        })?;
        registries.insert(slot, registry);
    }
    let mut replacements = 0;
    for &slot in callbacks {
        let (building, _, class) = buildings.get(&slot)
            .ok_or_else(|| format!("saved appearance callback slot {slot} has no prepared model"))?;
        let registry = registries.get_mut(&slot).unwrap();
        let text = labels.get(&slot).ok_or("appearance refresh text payload missing")?;
        registry.refresh(
            |handle| adapter.borrow_mut().release(handle),
            |row| {
                replacements += 1;
                let pose = saved_detail_pose(building, row)?;
                adapter.borrow_mut().construct_with_labels(row, *class, pose, image, assets, Some(text))
            },
        )?;
    }
    let adapter = adapter.into_inner();
    let mut draws = Vec::new();
    let mut spans = Vec::new();
    for (&slot, registry) in &registries {
        let building = &buildings.get(&slot).ok_or("appearance registry slot lost building")?.0;
        for row in &registry.rows {
            if u32::from_le_bytes(row.row[..4].try_into().unwrap()) != 1 { continue; }
            let handle = u32::from_le_bytes(row.row[0x28..0x2c].try_into().unwrap());
            let owned = adapter.draws(handle)
                .ok_or_else(|| format!("saved appearance row lost V2 handle {handle}"))?;
            let start = draws.len();
            draws.extend_from_slice(owned);
            if draws.len() > start {
                // 4b8228 adds the scene-Y correction after the 485200
                // constructor generated these descriptor-local draw handles.
                for draw in &mut draws[start..] {
                    for vertex in &mut draw.vertices {
                        vertex[1] += crate::category_distance::PASS3_Y_OFFSET;
                    }
                }
                let pivot_y = saved_detail_origin(building, row)?[1]
                    + crate::category_distance::PASS3_Y_OFFSET;
                spans.push((slot, start..draws.len(), pivot_y));
            }
        }
    }
    Ok((draws, replacements, spans))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original language/font4 and native valid-ID text-family oracle"]
    fn original_refresh_text_families() {
        let data = std::fs::read("work/save/native-appearance-label-families.bin").unwrap();
        assert_eq!(&data[..8], b"MTLBLF01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!((word(8), word(12)), (0x37f, 40));
        let parameters = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameters.parent().unwrap().parent().unwrap();
        let tab = std::fs::read(installer.join("Standard/fonts/coppgoth23p.tab")).unwrap();
        let language = installer.join("US/language/usa");
        let offsets = std::fs::read(language.join("tycoon.off")).unwrap();
        let text = std::fs::read(language.join("tycoon.bin")).unwrap();
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let assets = Assets::from_original(&read("OPENMT2001_ARCHIVE_DIR"), &read("OPENMT2001_ARCHIVE_BIN"))
            .unwrap().with_language(&tab, &offsets, &text).unwrap();
        let mut at = 16;
        for case in 0..40 {
            let descriptor = word(at);
            let id = word(at + 4);
            let width = f32::from_bits(word(at + 8));
            let height = f32::from_bits(word(at + 12));
            let vertices = word(at + 16) as usize;
            let indices = word(at + 20) as usize;
            let groups = word(at + 24) as usize;
            let len = word(at + 28) as usize;
            at += 32;
            let label = &data[at..at + len];
            at += len;
            let glyphs = if id == u32::MAX {
                assets.refresh_labels(Some(label), 0x36b).unwrap().owner
            } else {
                let labels = assets.refresh_labels(None, id).unwrap();
                assert_eq!(labels.business, label.iter().map(|byte| byte - 32).collect::<Vec<_>>());
                labels.business
            };
            let actual = match descriptor & 0xffff {
                0x0201 => text_sign_geometry(&tab, &glyphs, width, height).unwrap(),
                0x0204 => double_backed_text_sign_geometry(&tab, &glyphs, width, height).unwrap(),
                0x0101 => vertical_text_sign_geometry(&tab, &glyphs, width, height, false, false).unwrap(),
                0x0102 => vertical_text_sign_geometry(&tab, &glyphs, width, height, true, true).unwrap(),
                0x0104 => vertical_text_sign_geometry(&tab, &glyphs, width, height, true, false).unwrap(),
                _ => panic!("unexpected original descriptor {descriptor:x}"),
            };
            assert_eq!((actual.positions.len(), actual.indices.len()), (vertices, indices), "case {case}");
            let native_groups = (0..groups).map(|i| word(at + i * 8) as usize).collect::<Vec<_>>();
            assert_eq!(native_groups.iter().sum::<usize>(), indices);
            let expected_groups = actual.font_page_indices.into_iter().filter(|&n| n != 0)
                .chain(std::iter::once(indices - actual.font_page_indices.iter().sum::<usize>()))
                .collect::<Vec<_>>();
            assert_eq!(native_groups, expected_groups, "case {case} groups");
            at += groups * 8;
            for (i, point) in actual.positions.iter().enumerate() {
                for (axis, &value) in point.iter().enumerate() {
                    let expected = f32::from_bits(word(at + i * 12 + axis * 4));
                    assert!((value - expected).abs() <= 1e-5, "case {case} vertex {i}/{axis}: {value} vs {expected}");
                }
            }
            at += vertices * 12;
            for (i, uv) in actual.uv.iter().enumerate() {
                for (axis, value) in uv.iter().enumerate() {
                    assert_eq!(value.to_bits(), word(at + i * 8 + axis * 4), "case {case} UV {i}/{axis}");
                }
            }
            at += vertices * 8;
            for (i, &color) in actual.colors.iter().enumerate() {
                assert_eq!(color, word(at + i * 4), "case {case} color {i}");
            }
            at += vertices * 4;
            for (i, &index) in actual.indices.iter().enumerate() {
                assert_eq!(index, u16::from_le_bytes(data[at + i * 2..at + i * 2 + 2].try_into().unwrap()), "case {case} index {i}");
            }
            at += indices * 2;
        }
        assert_eq!(at, data.len());
    }
    use openmt2001::{board::Board, build_file::BuildFile, save::{SaveFile, appearance}};

    #[test]
    #[ignore = "requires pinned v1.4 text constructor and original font4 metrics"]
    fn supplied_text_sign_geometry_matches_native_constructor() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_SAVE_TEXT_GEOMETRY").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTDTXT01");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned.as_bytes().chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], sha);
        let word = |at| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!((word(40), word(44)), (0x037f, 4));
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let tab = std::fs::read(parameter.parent().unwrap().parent().unwrap()
            .join("Standard/fonts/coppgoth23p.tab")).unwrap();
        let mut at = 48;
        for case in 0..4 {
            let len = word(at) as usize;
            at += 4;
            let label = &bytes[at..at+len];
            at += len;
            let width = f32::from_bits(word(at));
            let height = f32::from_bits(word(at+4));
            let vertex_count = word(at+8) as usize;
            let index_count = word(at+12) as usize;
            let group_count = word(at+16) as usize;
            at += 20;
            assert_eq!(group_count, 2);
            let groups = [word(at) as usize, word(at+8) as usize];
            assert_eq!((word(at+4), word(at+12)), (0x71110000, 0x72140000));
            at += 16;
            let glyphs = label.iter().map(|byte| byte.checked_sub(32).unwrap())
                .collect::<Vec<_>>();
            let actual = text_sign_geometry(&tab, &glyphs, width, height).unwrap();
            assert_eq!((actual.positions.len(), actual.indices.len()), (vertex_count, index_count));
            assert_eq!(actual.font_page_indices, [groups[0], 0]);
            assert_eq!(groups[1], 6);
            for (i, point) in actual.positions.iter().enumerate() {
                for (axis, value) in point.iter().enumerate() {
                    let expected = f32::from_bits(word(at+i*12+axis*4));
                    assert!((*value-expected).abs() <= 1e-5,
                            "case {case} vertex {i} axis {axis}: {value} vs {expected}");
                }
            }
            at += vertex_count * 12;
            for (i, tex) in actual.uv.iter().enumerate() {
                for (axis, value) in tex.iter().enumerate() {
                    assert_eq!(value.to_bits(), word(at+i*8+axis*4),
                               "case {case} vertex {i} UV {axis}");
                }
            }
            at += vertex_count * 8;
            for (i, &color) in actual.colors.iter().enumerate() {
                assert_eq!(color, word(at+i*4), "case {case} vertex {i} color");
            }
            at += vertex_count * 4;
            for (i, &index) in actual.indices.iter().enumerate() {
                assert_eq!(index, u16::from_le_bytes(bytes[at+i*2..at+i*2+2].try_into().unwrap()),
                           "case {case} index {i}");
            }
            at += index_count * 2;
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires pinned v1.4 retail text geometry and original font4 metrics"]
    fn original_double_backed_text_matches_native_constructor() {
        let bytes = std::fs::read(
            std::env::var("OPENMT2001_NATIVE_APPEARANCE_GAP").unwrap(),
        ).unwrap();
        assert_eq!(&bytes[..8], b"MTRETGE1");
        let word = |at: usize| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!((word(40), word(44)), (0x037f, 2));
        assert_eq!((word(48), word(52), word(56), word(60)),
            (311, 0, 0x0102_0204, 0x4574f0));
        let width = f32::from_bits(word(64));
        let height = f32::from_bits(word(68));
        let label_len = word(72) as usize;
        let label = &bytes[76..76+label_len];
        assert_eq!(label, b"TO-LET");
        let mut at = 76 + label_len;
        let vertex_count = word(at) as usize;
        let index_count = word(at+4) as usize;
        assert_eq!((vertex_count, index_count, word(at+8)), (28, 48, 2));
        at += 12;
        let groups = [word(at) as usize, word(at+8) as usize];
        assert_eq!(groups, [36, 12]);
        at += 16;
        let mut raw = Vec::new();
        for _ in 0..4 {
            let len = word(at) as usize;
            at += 4;
            raw.push(&bytes[at..at+len]);
            at += len;
        }
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let tab = std::fs::read(parameter.parent().unwrap().parent().unwrap()
            .join("Standard/fonts/coppgoth23p.tab")).unwrap();
        let glyphs = label.iter().map(|byte| byte - 32).collect::<Vec<_>>();
        let actual = double_backed_text_sign_geometry(&tab, &glyphs, width, height).unwrap();
        assert_eq!((actual.positions.len(), actual.indices.len()), (vertex_count, index_count));
        assert_eq!(actual.font_page_indices, [groups[0], 0]);
        for (i, point) in actual.positions.iter().enumerate() {
            for (axis, value) in point.iter().enumerate() {
                let offset = i*12 + axis*4;
                let expected = f32::from_le_bytes(raw[0][offset..offset+4].try_into().unwrap());
                assert!((*value - expected).abs() <= 1e-5,
                    "double-backed text vertex {i} axis {axis}");
            }
        }
        for (i, tex) in actual.uv.iter().enumerate() {
            for (axis, value) in tex.iter().enumerate() {
                let offset = i*8 + axis*4;
                assert_eq!(value.to_bits(),
                    u32::from_le_bytes(raw[1][offset..offset+4].try_into().unwrap()));
            }
        }
        for (i, color) in actual.colors.iter().enumerate() {
            assert_eq!(*color, u32::from_le_bytes(raw[2][i*4..i*4+4].try_into().unwrap()));
        }
        for (i, index) in actual.indices.iter().enumerate() {
            assert_eq!(*index, u16::from_le_bytes(raw[3][i*2..i*2+2].try_into().unwrap()));
        }
    }

    #[test]
    #[ignore = "requires pinned v1.4 456440 wrapper fixture and original font4"]
    fn original_vertical_text_signs_match_complete_native_buffers() {
        let bytes = std::fs::read("work/save/text-sign-constructor-probe.bin").unwrap();
        assert_eq!(&bytes[..8], b"MT456440");
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(40), 0x037f);
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let tab = std::fs::read(
            parameter
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("Standard/fonts/coppgoth23p.tab"),
        )
        .unwrap();
        let business_value = b"Business Value"
            .iter()
            .map(|byte| byte - 32)
            .collect::<Vec<_>>();
        let to_let = b"TO-LET"
            .iter()
            .map(|byte| byte - 32)
            .collect::<Vec<_>>();
        let mut at = 48;
        let mut checked = 0;
        for _ in 0..word(44) {
            let wrapper = word(at);
            let descriptor = word(at + 4);
            let width = f32::from_bits(word(at + 8));
            let height = f32::from_bits(word(at + 12));
            let count = (word(at + 16) as usize, word(at + 20) as usize);
            at += 24;
            let mut raw = Vec::new();
            for _ in 0..4 {
                let len = word(at) as usize;
                at += 4;
                raw.push(&bytes[at..at + len]);
                at += len;
            }
            let flags = match wrapper {
                0x457520 => Some((false, false)),
                0x457550 => Some((true, false)),
                0x457580 => Some((true, true)),
                0x4574f0 => None,
                _ => panic!("unknown native retail wrapper {wrapper:#x}"),
            };
            if let Some((frame_reverse, text_back)) = flags {
                assert!(matches!(
                    descriptor,
                    0x0101_0101 | 0x0102_0101 | 0x0101_0102 | 0x0102_0102 | 0x0101_0104
                ));
                let glyphs = if descriptor & 0x0002_0000 != 0 {
                    &to_let
                } else {
                    &business_value
                };
                let actual = vertical_text_sign_geometry(
                    &tab,
                    glyphs,
                    width,
                    height,
                    frame_reverse,
                    text_back,
                )
                .unwrap();
                assert_eq!((actual.positions.len(), actual.indices.len()), count);
                let positions = actual
                    .positions
                    .iter()
                    .flat_map(|point| point.iter().flat_map(|value| value.to_le_bytes()))
                    .collect::<Vec<_>>();
                let uv = actual
                    .uv
                    .iter()
                    .flat_map(|point| point.iter().flat_map(|value| value.to_le_bytes()))
                    .collect::<Vec<_>>();
                let colors = actual
                    .colors
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect::<Vec<_>>();
                let indices = actual
                    .indices
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect::<Vec<_>>();
                for (label, actual, expected) in [
                    ("positions", positions.as_slice(), raw[0]),
                    ("UV", uv.as_slice(), raw[1]),
                    ("colors", colors.as_slice(), raw[2]),
                    ("indices", indices.as_slice(), raw[3]),
                ] {
                    assert_eq!(
                        actual, expected,
                        "{descriptor:#x} {label} differs from native"
                    );
                }
                checked += 1;
            }
        }
        assert_eq!(at, bytes.len());
        assert_eq!(checked, 6, "five descriptor classes plus legacy 3x14 check");
    }

    #[test]
    #[ignore = "requires original v1.4 save, font4 and texture archive"]
    fn supplied_text_sign_rows_construct_and_release_owned_draws() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let tab = std::fs::read(installer.join("Standard/fonts/coppgoth23p.tab")).unwrap();
        let language = installer.join("US/language/usa");
        let offsets = std::fs::read(language.join("tycoon.off")).unwrap();
        let bytes = std::fs::read(language.join("tycoon.bin")).unwrap();
        let lengths = offsets.chunks_exact(2).map(|chunk| u16::from_le_bytes(chunk.try_into().unwrap()) as usize)
            .collect::<Vec<_>>();
        let begin = lengths[..0x45c].iter().sum::<usize>();
        let business_value = &bytes[begin..begin+lengths[0x45c]];
        let assets = Assets::from_original(&directory, &archive).unwrap()
            .with_font4(&tab, business_value).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let save = SaveFile::parse(&save_bytes).unwrap();
        let mut found = [false; 2];
        for source in save.buildings().unwrap() {
            let building = SceneBuilding::from_saved(&source, &board, &file).unwrap();
            let identity = appearance::model_identity(&file, building.selected, source.height()).unwrap();
            for row in &identity.renderer_templates {
                let (slot, glyph_indices) = match row.descriptor {
                    0x0101_0201 => (0, 84),
                    0x0102_0201 => (1, 36),
                    _ => continue,
                };
                let mut adapter = Adapter::default();
                let pose = saved_detail_pose(&building, row).unwrap();
                let handle = adapter.construct(row, 10, pose, &image, &assets).unwrap();
                let draws = adapter.draws(handle).unwrap();
                assert_eq!(draws.len(), 2);
                assert_eq!(draws[0].vertices.len(), glyph_indices);
                assert_eq!(draws[1].vertices.len(), 6);
                assert_eq!(draws[0].width, assets.font4[0].width);
                adapter.release(handle).unwrap();
                assert!(adapter.draws(handle).is_none());
                found[slot] = true;
            }
        }
        assert_eq!(found, [true, true]);
    }

    #[test]
    #[ignore = "requires pinned v1.4 4e3380 matrix fixture"]
    fn saved_detail_basis_matches_native_matrix_after_scene_z_flip() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_SAVE_DETAIL_MATRIX").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTDMAT01");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned.as_bytes().chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], sha);
        let word = |at| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!((word(40), word(44)), (0x037f, 6));
        assert_eq!(bytes.len(), 48 + 6 * 52);
        for case in 0..6 {
            let at = 48 + case * 52;
            let degrees = f32::from_bits(word(at));
            let native = (0..12).map(|i| f32::from_bits(word(at + 4 + i * 4)))
                .collect::<Vec<_>>();
            let basis = saved_detail_basis(degrees).to_cols_array();
            for row in 0..3 {
                for col in 0..3 {
                    let expected = native[col * 3 + row] * if row == 2 { -1. } else { 1. };
                    let actual = basis[col * 4 + row];
                    assert!((actual - expected).abs() <= 1e-7,
                            "angle {degrees} axis {row} basis {col}: {actual} vs {expected}");
                }
            }
        }
    }

    #[test]
    #[ignore = "requires pinned v1.4 geometry fixture and original texture collections"]
    fn supplied_sign_detail_geometry_and_draw_assets_match_native() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SAVE_DETAIL_GEOMETRY");
        assert_eq!(&bytes[..8], b"MTDTGEO1");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned.as_bytes().chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], sha);
        let word = |at| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!(word(40), 0x037f);
        assert_eq!(word(44), 4);
        let mut at = 48;
        for _ in 0..4 {
            let class = word(at);
            let width = f32::from_bits(word(at+4));
            let height = f32::from_bits(word(at+8));
            let indices = sign_material_indices(&image, class).unwrap();
            assert_eq!(indices, (word(at+12) as usize, word(at+16) as usize));
            assert_eq!((word(at+20), word(at+24)), (8, 12));
            at += 28;
            let geometry = sign_geometry(width, height).unwrap();
            for position in &geometry.positions {
                for component in position {
                    assert_eq!(component.to_bits(), word(at));
                    at += 4;
                }
            }
            for uv in &geometry.uv {
                for component in uv {
                    assert_eq!(component.to_bits(), word(at));
                    at += 4;
                }
            }
            for _ in 0..8 {
                assert_eq!(word(at), 0xffff_fffe);
                at += 4;
            }
            for index in geometry.indices {
                assert_eq!(index, u16::from_le_bytes(bytes[at..at+2].try_into().unwrap()));
                at += 2;
            }
        }
        assert_eq!(at, bytes.len());

        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let assets = Assets::from_original(&directory, &archive).unwrap();
        assert!(assets.generic.len() > 11 && assets.building.len() > 14);
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let save = SaveFile::parse(&save_bytes).unwrap();
        let saved = save.buildings().unwrap();
        let first = &saved[0];
        let selected = openmt2001::scene::SceneBuilding::from_saved(first, &board, &file).unwrap();
        let identity = appearance::model_identity(&file, selected.selected, first.height()).unwrap();
        let detail = identity.renderer_templates.iter()
            .find(|row| row.descriptor == 0x0104_0201).unwrap();
        let mut adapter = Adapter::default();
        let handle = adapter.construct(detail, 10, Mat4::IDENTITY, &image, &assets).unwrap();
        let draws = adapter.draws(handle).unwrap();
        assert_eq!(draws.len(), 2);
        assert!(draws.iter().all(|draw| draw.vertices.len() == 6));
        assert_eq!(draws[0].width, assets.generic[11].width);
        assert_eq!(draws[1].height, assets.building[14].height);
        adapter.release(handle).unwrap();
        assert!(adapter.draws(handle).is_none());
        let next = adapter.construct(detail, 10, Mat4::IDENTITY, &image, &assets).unwrap();
        assert!(next > handle);
    }

    #[test]
    #[ignore = "requires pinned v1.4 double-sided sign fixture and original collections"]
    fn supplied_double_sign_geometry_and_draw_assets_match_native() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SAVE_DOUBLE_SIGN_GEOMETRY");
        assert_eq!(&bytes[..8], b"MTDSIGN2");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned.as_bytes().chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], sha);
        let word = |at| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!((word(40), word(44)), (0x037f, 4));
        let mut at = 48;
        for _ in 0..4 {
            let class = word(at);
            let width = f32::from_bits(word(at+4));
            let height = f32::from_bits(word(at+8));
            assert_eq!(sign_material_indices(&image, class).unwrap(),
                       (word(at+12) as usize, word(at+16) as usize));
            assert_eq!((word(at+20), word(at+24)), (8, 24));
            at += 28;
            let geometry = sign_geometry(width, height).unwrap();
            for position in &geometry.positions {
                for component in position {
                    assert_eq!(component.to_bits(), word(at));
                    at += 4;
                }
            }
            for uv in &geometry.uv {
                for component in uv {
                    assert_eq!(component.to_bits(), word(at));
                    at += 4;
                }
            }
            for _ in 0..8 {
                assert_eq!(word(at), 0xffff_fffe);
                at += 4;
            }
            for index in double_sign_indices() {
                assert_eq!(index, u16::from_le_bytes(bytes[at..at+2].try_into().unwrap()));
                at += 2;
            }
        }
        assert_eq!(at, bytes.len());
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let assets = Assets::from_original(&directory, &archive).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let save = SaveFile::parse(&save_bytes).unwrap();
        let mut tested = false;
        for building in save.buildings().unwrap() {
            let selected = openmt2001::scene::SceneBuilding::from_saved(&building, &board, &file).unwrap();
            let identity = appearance::model_identity(&file, selected.selected, building.height()).unwrap();
            if let Some(detail) = identity.renderer_templates.iter()
                .find(|row| row.descriptor == 0x0104_0202) {
                let mut adapter = Adapter::default();
                let handle = adapter.construct(detail, 10, Mat4::IDENTITY, &image, &assets).unwrap();
                let draws = adapter.draws(handle).unwrap();
                assert_eq!(draws.len(), 2);
                assert_eq!((draws[0].vertices.len(), draws[1].vertices.len()), (12, 12));
                adapter.release(handle).unwrap();
                tested = true;
                break;
            }
        }
        assert!(tested, "supplied save has no active 01040202 detail");
    }

    #[test]
    #[ignore = "requires pinned v1.4 awning fixture and original texture collections"]
    fn supplied_awning_detail_geometry_and_draw_assets_match_native() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SAVE_POLE_GEOMETRY");
        assert_eq!(&bytes[..8], b"MTPOLE01");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned.as_bytes().chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], sha);
        let word = |at| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!(word(40), 0x037f);
        assert_eq!(word(44), 3);
        let mut at = 48;
        for _ in 0..3 {
            let class = word(at);
            let width = f32::from_bits(word(at+4));
            let height = f32::from_bits(word(at+8));
            assert_eq!(awning_material_indices(&image, class).unwrap(),
                       (word(at+12) as usize, word(at+16) as usize));
            assert_eq!((word(at+20), word(at+24), word(at+28)), (42, 102, 96));
            at += 32;
            let geometry = awning_geometry(width, height).unwrap();
            for position in &geometry.positions {
                for component in position {
                    let native = f32::from_bits(word(at));
                    assert!((component - native).abs() <= 1e-6,
                            "position at {at}: {component} versus {native}");
                    at += 4;
                }
            }
            for uv in &geometry.uv {
                for component in uv {
                    assert_eq!(component.to_bits(), word(at), "uv at {at}");
                    at += 4;
                }
            }
            for _ in 0..42 {
                assert_eq!(word(at), 0xffff_ffff, "color at {at}");
                at += 4;
            }
            for index in geometry.indices {
                assert_eq!(index, u16::from_le_bytes(bytes[at..at+2].try_into().unwrap()));
                at += 2;
            }
        }
        assert_eq!(at, bytes.len());
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let assets = Assets::from_original(&directory, &archive).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let save = SaveFile::parse(&save_bytes).unwrap();
        let mut tested = false;
        for building in save.buildings().unwrap() {
            let selected = openmt2001::scene::SceneBuilding::from_saved(&building, &board, &file).unwrap();
            let identity = appearance::model_identity(&file, selected.selected, building.height()).unwrap();
            if let Some(detail) = identity.renderer_templates.iter()
                .find(|row| row.descriptor == 0x0200_0001) {
                let mut adapter = Adapter::default();
                let handle = adapter.construct(detail, 10, Mat4::IDENTITY, &image, &assets).unwrap();
                let draws = adapter.draws(handle).unwrap();
                assert_eq!(draws.len(), 2);
                assert_eq!((draws[0].vertices.len(), draws[1].vertices.len()), (96, 6));
                assert_eq!(draws[0].width, assets.building[14].width);
                assert_eq!(draws[1].height, assets.building[25].height);
                adapter.release(handle).unwrap();
                tested = true;
                break;
            }
        }
        assert!(tested, "supplied save has no active 02000001 detail");
    }

    #[test]
    #[ignore = "requires pinned v1.4 banner fixture and original texture collections"]
    fn supplied_banner_detail_geometry_and_draw_assets_match_native() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SAVE_BANNER_GEOMETRY");
        assert_eq!(&bytes[..8], b"MTBANR01");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned.as_bytes().chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], sha);
        let word = |at| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!((word(40), word(44)), (0x037f, 2));
        let mut at = 48;
        for _ in 0..2 {
            let width = f32::from_bits(word(at));
            let height = f32::from_bits(word(at+4));
            assert_eq!(banner_material_indices(&image, BannerSelection::INITIAL).unwrap(),
                       (word(at+8) as usize, word(at+12) as usize, Some(word(at+16) as usize)));
            assert_eq!((word(at+20), word(at+24), word(at+28)), (20, 36, 3));
            assert_eq!((word(at+32), word(at+36), word(at+40)), (12, 12, 12));
            at += 44;
            let geometry = banner_geometry(width, height).unwrap();
            for position in &geometry.positions {
                for component in position {
                    let native = f32::from_bits(word(at));
                    assert!((component - native).abs() <= 1e-6,
                            "position at {at}: {component} versus {native}");
                    at += 4;
                }
            }
            for uv in &geometry.uv {
                for component in uv {
                    assert_eq!(component.to_bits(), word(at), "uv at {at}");
                    at += 4;
                }
            }
            for _ in 0..20 {
                assert_eq!(word(at), 0xffff_fffe, "color at {at}");
                at += 4;
            }
            for index in geometry.indices {
                assert_eq!(index, u16::from_le_bytes(bytes[at..at+2].try_into().unwrap()));
                at += 2;
            }
        }
        assert_eq!(at, bytes.len());
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let assets = Assets::from_original(&directory, &archive).unwrap();
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let save = SaveFile::parse(&save_bytes).unwrap();
        let mut tested = false;
        for building in save.buildings().unwrap() {
            let selected = openmt2001::scene::SceneBuilding::from_saved(&building, &board, &file).unwrap();
            let identity = appearance::model_identity(&file, selected.selected, building.height()).unwrap();
            if let Some(detail) = identity.renderer_templates.iter()
                .find(|row| row.descriptor == 0x0108_0202) {
                let mut adapter = Adapter::default();
                let handle = adapter.construct(detail, 10, Mat4::IDENTITY, &image, &assets).unwrap();
                let draws = adapter.draws(handle).unwrap();
                assert_eq!(draws.len(), 3);
                assert_eq!(draws.iter().map(|d| d.vertices.len()).collect::<Vec<_>>(), [12, 12, 12]);
                assert_eq!(draws[0].width, assets.signs[45].width);
                assert_eq!(draws[1].height, assets.generic640[1].height);
                assert_eq!(draws[2].width, assets.signs[0].width);
                adapter.release(handle).unwrap();
                tested = true;
                break;
            }
        }
        assert!(tested, "supplied save has no active 01080202 detail");
    }

    #[test]
    #[ignore = "requires pinned v1.4 488230 final detail-origin fixture"]
    fn supplied_saved_detail_origins_match_native_world_transform() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SAVE_DETAIL_POSE");
        assert_eq!(&bytes[..8], b"MTDPOS01");
        let pinned = "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b";
        let sha: Vec<_> = pinned.as_bytes().chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(&bytes[8..40], sha);
        let word = |at| u32::from_le_bytes(bytes[at..at+4].try_into().unwrap());
        assert_eq!((word(40), word(44)), (0x037f, 15));
        let file_bytes = read("OPENMT2001_BUILD_FILE");
        let file = BuildFile::parse(&file_bytes).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let save = SaveFile::parse(&save_bytes).unwrap();
        let saved = save.buildings().unwrap();
        let mut at = 48;
        let mut checked = 0;
        for _ in 0..15 {
            let slot = word(at) as usize;
            let count = word(at+4) as usize;
            at += 8;
            let source = saved.iter().find(|building| building.slot == slot).unwrap();
            let building = SceneBuilding::from_saved(source, &board, &file).unwrap();
            let identity = appearance::model_identity(&file, building.selected, source.height()).unwrap();
            assert_eq!(identity.renderer_templates.len(), count);
            for row in &identity.renderer_templates {
                let index = word(at);
                assert!(index < 1000);
                at += 4;
                let native = &bytes[at..at+52];
                let origin = saved_detail_origin(&building, row).unwrap();
                for (axis, offset) in [12, 16, 20].into_iter().enumerate() {
                    let expected = u32::from_le_bytes(native[offset..offset+4].try_into().unwrap());
                    assert_eq!(origin[axis].to_bits(), expected,
                               "slot {slot} detail {} axis {axis}", row.detail_slot);
                }
                let rotation = saved_detail_rotation_degrees(&building, row).unwrap();
                assert_eq!(rotation.to_bits(),
                           u32::from_le_bytes(native[36..40].try_into().unwrap()),
                           "slot {slot} detail {} rotation", row.detail_slot);
                at += 52;
                checked += 1;
            }
        }
        assert_eq!(at, bytes.len());
        assert_eq!(checked, 59);
    }
}

#[cfg(test)]
#[path = "rooftop_identity_tests.rs"]
mod rooftop_identity_tests;
