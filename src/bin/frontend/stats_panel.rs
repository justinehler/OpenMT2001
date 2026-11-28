//! Manager graph dialog: native49dcb0 show,49cd40 metric and49e630 actions.
//! Distinct from the small426ba0 Comparison panel. The dialog uses graph_panel.tm.

pub const PANEL: &str = "gamedata2/brand_new_panels/graph_panel.tm";
//49ce08's six-way jump table determines order, rather than adjacent case bodies.
pub const LABELS: [u32; 6] = [0x56a, 0x566, 0x565, 0x567, 0x56b, 0x602];

use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    render::{PreparedScene, Renderer, SurfaceMode},
    texture::TextureSet,
};
use std::{fs, path::Path};

pub struct Assets {
    model: Vec<Surface>,
    regions: Vec<(String, [f32; 4])>,
    generic: Vec<Surface>,
    fonts: Vec<Surface>,
    tabs: [Vec<u8>; 3],
    strings: Vec<Vec<u8>>,
    colors: [u32; 10],
    icons: [[usize; 2]; 10],
    body: u32,
    root: [f32; 4],
}
#[derive(Default)]
pub struct View {
    pub prepared: Option<PreparedScene>,
    snapshot: Option<Snapshot>,
    hover: Option<usize>,
    pressed: Option<usize>,
    down: bool,
    dirty: bool,
}
impl View {
    fn visual_pressed(&self) -> Option<usize> {
        //43c8e0 sends leave3 when the pointer leaves.44340f clears330,
        //but334 retains the down latch used by the eventual release.
        self.pressed.filter(|index| self.hover == Some(*index))
    }
    pub fn cancel_input(&mut self) {
        self.dirty |= self.down || self.pressed.is_some() || self.hover.is_some();
        self.down = false;
        self.pressed = None;
        self.hover = None;
    }
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        snapshot: Option<&Snapshot>,
    ) -> Result<(), String> {
        let Some(snapshot) = snapshot else {
            self.prepared = None;
            self.snapshot = None;
            self.hover = None;
            self.pressed = None;
            self.down = false;
            return Ok(());
        };
        if self.prepared.is_none() || self.snapshot.as_ref() != Some(snapshot) || self.dirty {
            self.prepared =
                Some(assets.prepare(renderer, snapshot, self.hover, self.visual_pressed())?);
            self.snapshot = Some(snapshot.clone());
            self.dirty = false;
        }
        Ok(())
    }
    /// Retain mouse capture identity; release inside the same widget emits12.
    pub fn pointer(
        &mut self,
        assets: &Assets,
        point: Option<[f32; 2]>,
        down: bool,
        snapshot: &Snapshot,
    ) -> Option<crate::live_host::StatsAction> {
        let hit = point.and_then(|point| assets.button_at(point, snapshot));
        self.dirty |= self.hover != hit;
        self.hover = hit;
        let was_down = self.down;
        self.down = down;
        if down {
            if !was_down {
                self.pressed = hit;
                self.dirty = true;
            }
            None
        } else {
            let pressed = self.pressed.take();
            self.dirty |= pressed.is_some();
            if pressed.is_some() && pressed == hit {
                hit.map(action)
            } else {
                None
            }
        }
    }
}
fn action(index: usize) -> crate::live_host::StatsAction {
    use crate::live_host::StatsAction;
    match index {
        0 => StatsAction::Close,
        1 => StatsAction::Cycle(false),
        2 => StatsAction::Cycle(true),
        _ => StatsAction::Toggle(index - 3),
    }
}

// Stats slot14 uses native CULL_CW. Its transformed positions are Y-down
// screen coordinates: negative area faces the viewer, positive area is the
// reverse-facing layout geometry. Keep native index order; do not flip faces.
// This applies only to authored Stats mesh triangles, never palette/UI quads.
fn stats_mesh_triangles<'a>(
    positions: &'a [[f32; 3]],
    indices: &'a [usize],
) -> impl Iterator<Item = [usize; 3]> + 'a {
    indices.chunks_exact(3).filter_map(move |triangle| {
        let ids = [triangle[0], triangle[1], triangle[2]];
        let [a, b, c] = ids.map(|id| positions[id]);
        let area = (f64::from(b[0]) - f64::from(a[0])) * (f64::from(c[1]) - f64::from(a[1]))
            - (f64::from(b[1]) - f64::from(a[1])) * (f64::from(c[0]) - f64::from(a[0]));
        // No epsilon: degenerate and nonfinite faces contribute no raster area.
        (area.is_finite() && area < 0.).then_some(ids)
    })
}

impl Assets {
    pub(crate) fn apply_board(
        &mut self,
        board: &crate::intro::camera_live::ui_text_resources::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

    #[cfg(test)]
    pub(crate) fn input_test_assets() -> Self {
        Self {
            model: vec![],
            regions: vec![("STATS_OK_BUTTON".into(), [300., 400., 340., 430.])],
            generic: vec![],
            fonts: vec![],
            tabs: Default::default(),
            strings: vec![],
            colors: [0; 10],
            icons: [[0; 2]; 10],
            body: 0,
            root: [50., 40., 550., 450.],
        }
    }
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        executable: &[u8],
    ) -> Result<Self, String> {
        let payload = |name: &str| {
            let entry = index
                .find(name)
                .ok_or_else(|| format!("Missing Stats asset {name}"))?;
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or_else(|| "Stats asset range".to_string())
        };
        let mesh = CachedMesh::parse(payload(PANEL)?)?;
        let positions = panel_positions(&mesh)?;
        let uv = mesh.uvs();
        let mut model = Vec::new();
        for part in &mesh.parts {
            let texture = TextureSet::parse(payload(mesh.texture_names[part.texture as usize])?)
                .map_err(|e| e.to_string())?;
            let mut surface =
                Surface::new(texture.textures.first().ok_or("Stats mesh texture empty")?);
            let indices = part.vertex_indices().collect::<Vec<_>>();
            for triangle in stats_mesh_triangles(&positions, &indices) {
                for vertex in triangle {
                    surface.vertices.push([
                        positions[vertex][0],
                        positions[vertex][1],
                        positions[vertex][2],
                        1.,
                        1.,
                        1.,
                        uv[vertex][0],
                        uv[vertex][1],
                    ]);
                    surface.alpha.push(255);
                }
            }
            if !surface.vertices.is_empty() {
                model.push(surface);
            }
        }
        let root = openmt2001::ui_mesh::parent_bounds(&mesh, 0)?;
        let regions = openmt2001::ui_mesh::regions(&mesh, 0)?
            .into_iter()
            .map(|r| (r.name, r.bounds))
            .collect();
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect();
        let fonts = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures
            .iter()
            .map(Surface::new)
            .collect();
        let tabs = ["cust640p.tab", "tahoma12bldnoantip.tab", "coppgoth23p.tab"]
            .map(|name| {
                fs::read(installer.join("Standard/fonts").join(name)).map_err(|e| e.to_string())
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Stats font count")?;
        let image = openmt2001::pe::PeImage::parse(executable)?;
        let word = |at| {
            image
                .at(at, 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        let colors = (0..10)
            .map(|i| word(0x541af0 + i * 40))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "Stats palette count")?;
        let icons = (0..10)
            .map(|i| {
                Ok([
                    word(0x541b00 + i * 40)? as usize,
                    word(0x541b08 + i * 40)? as usize,
                ])
            })
            .collect::<Result<Vec<_>, String>>()?
            .try_into()
            .map_err(|_| "Stats icon count")?;
        Ok(Self {
            model,
            regions,
            generic,
            fonts,
            tabs,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            colors,
            icons,
            body: word(0x5362c0)?,
            root,
        })
    }
    fn bounds(&self, name: &str) -> [f32; 4] {
        //43aca0's absent-part result is the prezeroed native rectangle.
        self.regions
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| *b)
            .unwrap_or([0.; 4])
    }
    pub fn contains(&self, point: [f32; 2]) -> bool {
        openmt2001::ui_mesh::parent_hit(self.root, point, [0., 0.])
    }
    fn button_bounds(&self, index: usize) -> [f32; 4] {
        self.bounds(&match index {
            0 => "STATS_OK_BUTTON".into(),
            1 => "STATS_LEFT_CHANGER".into(),
            2 => "STATS_RIGHT_CHANGER".into(),
            _ => format!("STATS_TOKEN_ICON{}", index - 2),
        })
    }
    fn button_at(&self, point: [f32; 2], snapshot: &Snapshot) -> Option<usize> {
        if !self.contains(point) {
            return None;
        }
        (0..3 + snapshot.players.len())
            .find(|&i| openmt2001::ui_mesh::parent_hit(self.button_bounds(i), point, [0., 0.]))
    }
    pub fn pointer(
        &self,
        point: [f32; 2],
        pressed: bool,
        snapshot: &Snapshot,
    ) -> Option<crate::live_host::StatsAction> {
        pressed
            .then(|| self.button_at(point, snapshot).map(action))
            .flatten()
    }
    fn text(&self, id: u32) -> Result<&[u8], String> {
        self.strings
            .get(id as usize)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("Stats string {id} missing"))
    }
    fn glyphs(
        &self,
        font: usize,
        quads: Vec<openmt2001::font::GlyphQuad>,
        color: u32,
    ) -> Result<Vec<Surface>, String> {
        let mut pages = self
            .fonts
            .get(font * 2..font * 2 + 2)
            .ok_or("Stats font pages missing")?
            .to_vec();
        for glyph in quads {
            pages
                .get_mut(glyph.page)
                .ok_or("Stats glyph page range")?
                .quad(glyph.positions.map(|p| [p[0], p[1]]), glyph.uv, [color; 4]);
        }
        Ok(pages)
    }
    fn label(
        &self,
        font: usize,
        text: &[u8],
        bounds: [f32; 4],
        style: u32,
    ) -> Result<Vec<Surface>, String> {
        let tab = &self.tabs[match font {
            0 => 0,
            1 => 1,
            4 => 2,
            _ => return Err("Stats font index".into()),
        }];
        self.glyphs(font, text_quads(tab, text, bounds, style)?, 0xffffffff)
    }
    fn name(&self, text: &[u8]) -> Result<Vec<u8>, String> {
        let dictionary = self.text(0)?;
        text.iter()
            .map(|&c| {
                dictionary
                    .get(usize::from(
                        c.checked_sub(32).ok_or("Stats name control byte")?,
                    ))
                    .copied()
                    .ok_or_else(|| "Stats name dictionary range".into())
            })
            .collect()
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        snapshot: &Snapshot,
        hover: Option<usize>,
        pressed: Option<usize>,
    ) -> Result<PreparedScene, String> {
        if snapshot.players.len() > 6 || snapshot.state.mode >= 6 {
            return Err("Stats presentation outside native slots".into());
        }
        let graph = snapshot.graph(self.bounds("STATS_GRAPH_AREA"))?;
        let grid = graph.grid()?;
        let mut surfaces = self.model.clone();
        let mut fill = Surface::solid();
        fill.rect(
            self.bounds("STATS_TITLE_AREA"),
            [0xff0000c0, 0xff0000c0, 0x800000c0, 0x800000c0],
        );
        fill.rect(
            self.bounds("STATS_SESS_TITLE"),
            [0xff909090, 0xff909090, 0x80909090, 0x80909090],
        );
        fill.rect(self.bounds("STATS_BACKING3"), [self.body; 4]);
        fill.rect(self.bounds("STATS_COMPARE_ITEM"), [0xff1f2f70; 4]);
        fill.rect(self.bounds("STATS_GRAPH_AREA"), [0x80000000; 4]);
        for (slot, player) in snapshot.players.iter().enumerate().rev() {
            let color = *self
                .colors
                .get(player.character as usize)
                .ok_or("Stats character palette range")?
                & 0xffffff;
            fill.rect(
                self.bounds(&format!("STATS_TOKEN_BACKING{:02}", slot + 1)),
                [
                    color | 0xff000000,
                    color | 0xff000000,
                    color | 0x7f000000,
                    color | 0x7f000000,
                ],
            );
        }
        let f = |i| f32::from_bits(graph.words[i]);
        fill.rect(
            [
                f(4),
                f(5),
                (f64::from(f(4)) + f64::from(f(6))) as f32,
                (f64::from(f(5)) + f64::from(f(7))) as f32,
            ],
            [0x7f000000; 4],
        );
        for (bounds, color) in grid.rectangles {
            fill.rect(bounds, [color; 4]);
        }
        surfaces.push(fill);
        for args in grid.labels {
            let text = axis_text(
                &self.tabs[0],
                self.text(0)?,
                self.text(1548)?,
                self.text(2243)?,
                self.text(2244)?.first() != Some(&0x4c),
                args,
            )?;
            surfaces.extend(self.glyphs(0, text.glyphs, text.color)?);
        }
        let mut lines = Surface::solid();
        for (slot, points) in &graph.series {
            let color =
                self.colors[snapshot.players[*slot].character as usize] & 0xffffff | 0x9f000000;
            for quad in line_quads(
                points,
                [f(4), f(5), f(6), f(7), f(8), f(9), f(10), f(11)],
                4.,
            )? {
                lines.quad(quad.map(|p| [p[0], p[1]]), [[0.; 2]; 4], [color; 4]);
            }
        }
        surfaces.push(lines);
        surfaces.extend(self.label(
            1,
            self.text(LABELS[snapshot.state.mode])?,
            self.bounds("STATS_COMPARE_ITEM"),
            9,
        )?);
        for (slot, player) in snapshot.players.iter().enumerate().rev() {
            surfaces.extend(self.label(
                0,
                &self.name(&player.name)?,
                self.bounds(&format!("STATS_TOKEN_LABEL{}", slot + 1)),
                3,
            )?);
        }
        surfaces.extend(self.label(1, &[], self.bounds("STATS_COMPARE_LABEL"), 3)?);
        surfaces.extend(self.label(1, &[0], self.bounds("STATS_SESS_TITLE"), 9)?);
        surfaces.extend(self.label(4, self.text(1676)?, self.bounds("STATS_TITLE_AREA"), 9)?);
        //43c3a0 invokes the child list after49de60. Textured443080 draws its
        // caption first and mode1 selected icons force the hover texture.
        for i in (0..3 + snapshot.players.len()).rev() {
            let bounds = self.button_bounds(i);
            if i == 0 {
                let mut button = Surface::solid();
                ui::button(
                    &mut button,
                    bounds,
                    [80, 148, 34],
                    hover == Some(i),
                    pressed == Some(i),
                    false,
                )?;
                surfaces.push(button);
                //4419f0 enqueues the caption before442b10, but455390 submits
                //the remaining font batch after the child background.
                surfaces.extend(self.label(1, self.text(1435)?, bounds, 9)?);
            } else {
                let texture = if i == 1 {
                    if pressed == Some(i) {
                        99
                    } else if hover == Some(i) {
                        98
                    } else {
                        97
                    }
                } else if i == 2 {
                    if pressed == Some(i) {
                        103
                    } else if hover == Some(i) {
                        102
                    } else {
                        101
                    }
                } else {
                    let slot = i - 3;
                    let icon = self.icons[snapshot.players[slot].character as usize];
                    icon[usize::from(snapshot.state.selected[slot] || hover == Some(i))]
                };
                let mut surface = self
                    .generic
                    .get(texture)
                    .ok_or("Stats widget texture range")?
                    .clone();
                surface.quad(
                    [
                        [bounds[0], bounds[1]],
                        [bounds[0], bounds[3]],
                        [bounds[2], bounds[3]],
                        [bounds[2], bounds[1]],
                    ],
                    [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                    [0xffffffff; 4],
                );
                surfaces.push(surface);
            }
        }
        let draws = surfaces
            .iter()
            .map(|s| {
                let mut draw = s.draw();
                draw.mode = SurfaceMode::UiDepth;
                draw
            })
            .collect::<Vec<_>>();
        //Native UI viewZ15, projectionZ.0001; pixel positions remain canonical640.
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
                0.0001,
                0.,
                -1.,
                1.,
                (15f64 * f64::from(0.0001f32)) as f32,
                1.,
            ],
            None,
        )
    }
}

///4918eb retains a180-degree X pose for frontend slot14.43b770 first
/// stores rawZ*-1+1, then43c3a0/4f24d0 reverses the third matrix row.
pub fn panel_positions(mesh: &openmt2001::mesh::CachedMesh<'_>) -> Result<Vec<[f32; 3]>, String> {
    let radians = (180f64 * f64::from(f32::from_bits(0x3c8efa35))) as f32;
    let (sine, cosine) = f64::from(radians).sin_cos();
    let (sine, cosine) = (sine as f32, cosine as f32);
    let transform = openmt2001::people::PeopleTransform {
        matrix: [
            [1., -0., 0.],
            [0., cosine, sine],
            [-0., sine, -cosine],
            [0., 0., -0.],
        ],
        translation_only: false,
    };
    Ok(mesh
        .positions(0)?
        .into_iter()
        .map(|mut position| {
            position[2] = (-f64::from(position[2]) + 1.) as f32;
            transform.position(position)
        })
        .collect())
}

///4406a0 styles3 and9. Style9 measures first-row glyph height plus the
/// glyph's vertical offset through4e79b0; style3 uses the W line height.
pub fn text_quads(
    tab: &[u8],
    text: &[u8],
    bounds: [f32; 4],
    style: u32,
) -> Result<Vec<openmt2001::font::GlyphQuad>, String> {
    if !matches!(style, 3 | 9) {
        return Err("Stats text style leaves native constructor domain".into());
    }
    let font = openmt2001::font::Font::parse(tab)?;
    let metric = |glyph: u8, offset: usize| -> Result<i32, String> {
        let at = 4 + usize::from(glyph) * 44 + offset;
        Ok(i32::from_le_bytes(
            tab.get(at..at + 4)
                .ok_or("Stats glyph metric outside font")?
                .try_into()
                .unwrap(),
        ))
    };
    let line_height = metric(b'W' - 32, 36)?;
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let wrap_width = (f64::from(bounds[2]) - f64::from(bounds[0])).trunc() as i32;
    let (offsets, copy) = if wrap_width == 0 && text == [0] {
        //The absent STATS_SESS_TITLE retains one encoded space and zero bounds.
        //Whole4e7a90/4419f0 emits one degenerate glyph, not an invented region.
        (vec![0, 2], vec![0, 0])
    } else {
        crate::prompt::wrap_with_gap(tab, text, wrap_width, -1)?
    };
    let count = offsets.len() - 1;
    let first = &copy[offsets[0]..offsets[1].saturating_sub(1).min(copy.len())];
    let height = if style == 9 {
        first.iter().try_fold(0i32, |maximum, &glyph| {
            Ok::<_, String>(maximum.max(metric(glyph, 36)?.wrapping_add(metric(glyph, 40)?)))
        })?
    } else {
        line_height
    };
    let top = bounds[1].trunc()
        + ((f64::from(bounds[3]) - f64::from(bounds[1]) - count.max(1) as f64 * f64::from(height))
            * 0.5)
            .trunc() as f32;
    let mut quads = Vec::new();
    for (row, pair) in offsets.windows(2).enumerate() {
        let end = pair[1].saturating_sub(1).min(copy.len());
        let text = &copy[pair[0].min(end)..end];
        let width = crate::ui::text_width(tab, text, 1.)?;
        let x = if style == 9 {
            ((f64::from(bounds[2]) - f64::from(bounds[0]) - f64::from(width)) * 0.5 + 0.5).trunc()
                as f32
        } else {
            2.
        };
        let row_top = top + row as f32 * line_height as f32;
        let mut row_quads = font.layout(
            text,
            openmt2001::font::Layout {
                origin: [bounds[0].trunc() + x, row_top.max(0.)],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )?;
        if row_top < 0. {
            //441cc9 writes highword0 before FILD64: a negative signed rowY
            //becomes unsigned32.4e7730 FST32 precedes callback FTOL lowword.
            let legacy_top = (row_top as i32 as u32) as f32;
            for (&glyph, quad) in text.iter().zip(&mut row_quads) {
                let y = (f64::from(legacy_top) + f64::from(metric(glyph, 40)?)) as f32;
                let top = f64::from(openmt2001::x87::Sum::from_f32(y)?.ftol_low_word()) - 0.5;
                let bottom = (top + f64::from(metric(glyph, 36)?)) as f32;
                let top = top as f32;
                quad.positions[0][1] = bottom;
                quad.positions[1][1] = bottom;
                quad.positions[2][1] = top;
                quad.positions[3][1] = top;
            }
        }
        quads.extend(row_quads);
    }
    Ok(quads)
}

///42ce80 keeps the original hour-format branch's literal"5", supplied to
/// localized1548. Its direct4e8520 path uses alpha15 and skips centering.
pub struct AxisText {
    pub encoded: Vec<u8>,
    pub glyphs: Vec<openmt2001::font::GlyphQuad>,
    pub color: u32,
}
pub fn axis_text(
    tab: &[u8],
    dictionary: &[u8],
    template: &[u8],
    symbol: &[u8],
    suffix: bool,
    args: [u32; 6],
) -> Result<AxisText, String> {
    let [x, y, value] = [args[0], args[1], args[2]].map(f32::from_bits);
    if ![x, y, value].into_iter().all(f32::is_finite) || args[3] > 2 || args[4] > 3 {
        return Err("Stats axis label leaves finite native formatter domain".into());
    }
    let font = openmt2001::font::Font::parse(tab)?;
    let encode = |text: &[u8]| -> Result<Vec<u8>, String> {
        text.iter()
            .map(|&c| {
                dictionary
                    .get(usize::from(
                        c.checked_sub(32).ok_or("Stats ASCII control character")?,
                    ))
                    .copied()
                    .ok_or_else(|| "Stats ASCII dictionary range".into())
            })
            .collect()
    };
    let precision = args[3] as usize;
    let ascii = if args[4] == 3 {
        b"5".to_vec()
    } else if args[4] == 1 {
        let amount = openmt2001::x87::Sum::from_f32(value)?.ftol_low_word();
        // Symbol is already glyph-encoded, so money bypasses ASCII conversion.
        let text = crate::ui::integer_money(amount, symbol, suffix);
        return axis_glyphs(tab, &font, text.clone(), text, [x, y], args[5]);
    } else {
        let factor = 10u32.pow(precision as u32);
        let scaled = f64::from(value.abs()) * f64::from(factor);
        let mut text = if value.abs() >= 1e17 {
            let mut text = crate::ui::decimal2(value)?;
            text.truncate(text.len() - 2 + precision);
            if precision == 0 {
                text.pop();
            }
            text
        } else if scaled.fract() == 0.5 {
            let units = scaled.ceil() as u64;
            let sign = if value.is_sign_negative() { "-" } else { "" };
            if precision == 0 {
                format!("{sign}{units}")
            } else {
                format!(
                    "{sign}{}.{:0precision$}",
                    units / u64::from(factor),
                    units % u64::from(factor)
                )
            }
        } else {
            format!("{value:.precision$}")
        };
        if args[4] == 2 {
            text.push('%');
        }
        text.into_bytes()
    };
    //42d0f9 measures the raw number before localized1548 adds "%".
    let measured = encode(if args[4] == 2 {
        &ascii[..ascii.len() - 1]
    } else {
        &ascii
    })?;
    let numeric = encode(&ascii)?;
    let placeholder = [b'%' - 32, b's' - 32];
    let start = template
        .windows(2)
        .position(|s| s == placeholder)
        .ok_or("Stats axis localized1548 lacks%s")?;
    let mut text = template[..start].to_vec();
    text.extend_from_slice(&numeric);
    text.extend_from_slice(&template[start + 2..]);
    if args[4] == 3 {
        let quads = font.layout(
            &text,
            openmt2001::font::Layout {
                origin: [x, y],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )?;
        Ok(AxisText {
            encoded: text,
            glyphs: quads,
            color: 0x0fffffff,
        })
    } else {
        axis_glyphs(tab, &font, text, measured, [x, y], args[5])
    }
}
fn axis_glyphs(
    tab: &[u8],
    font: &openmt2001::font::Font<'_>,
    text: Vec<u8>,
    measured: Vec<u8>,
    [x, y]: [f32; 2],
    alignment: u32,
) -> Result<AxisText, String> {
    let width = crate::ui::text_width(tab, &measured, 1.)? / 2;
    let mut minimum = 999u32;
    let mut maximum = 0u32;
    for glyph in measured {
        let at = 4 + usize::from(glyph) * 44;
        let offset = u32::from_le_bytes(
            tab.get(at + 40..at + 44)
                .ok_or("Stats axis glyph offset")?
                .try_into()
                .unwrap(),
        );
        let height = u32::from_le_bytes(tab[at + 36..at + 40].try_into().unwrap());
        minimum = minimum.min(offset);
        maximum = maximum.max(offset.wrapping_add(height));
    }
    let height = (maximum.wrapping_sub(minimum) as i32) / 2;
    let mut left = (f64::from(x) - f64::from(width)) as f32;
    let mut top = (f64::from(y) - f64::from(height)) as f32;
    if alignment & 1 != 0 {
        left = (f64::from(left) + f64::from(width + 6)) as f32;
    }
    if alignment & 2 != 0 {
        left = (f64::from(left) - f64::from(width + 6)) as f32;
    }
    if alignment & 4 != 0 {
        top = (f64::from(top) - f64::from(height + 2)) as f32;
    }
    if alignment & 8 != 0 {
        top = (f64::from(top) + f64::from(height + 2)) as f32;
    }
    let quads = font.layout(
        &text,
        openmt2001::font::Layout {
            origin: [left, top],
            depth: 0.5,
            scale: 1.,
            gap: -1,
            baseline: 0,
        },
    )?;
    Ok(AxisText {
        encoded: text,
        glyphs: quads,
        color: 0xffffffff,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub mode: usize,
    pub selected: [bool; 6],
    pub visible: [bool; 6],
}
impl State {
    ///49dcb0 shows every manager slot, regardless of its active accounting byte.
    /// Only stopped objective1/2 presentations expose the sixth market-share metric.
    pub fn show(objective: i32, running: bool, players: usize) -> Result<Self, String> {
        if players > 6 {
            return Err("Stats roster exceeds six native selectors".into());
        }
        let mode = match objective {
            1 | 2 if !running => 5,
            3 => 4,
            4 | 5 => 1,
            6 => 0,
            _ => 2,
        };
        Ok(Self {
            mode,
            selected: std::array::from_fn(|i| i < players),
            visible: std::array::from_fn(|i| i < players),
        })
    }
    ///49cd40 wraps at either boundary without a remainder operation.
    pub fn set_mode(&mut self, value: i32) {
        self.mode = if value < 0 {
            5
        } else if value > 5 {
            0
        } else {
            value as usize
        };
    }
    ///49e630 checks live running/objective values when an arrow is activated.
    pub fn cycle(&mut self, right: bool, objective: i32, running: bool) {
        let all = !running && matches!(objective, 1 | 2);
        let value = self.mode as i32 + if right { 1 } else { -1 };
        if all {
            self.set_mode(value);
        } else {
            self.set_mode(if right && value == 5 {
                0
            } else if !right && self.mode == 0 {
                4
            } else {
                value
            });
        }
    }
    pub fn toggle(&mut self, player: usize) -> Result<(), String> {
        if !self.visible.get(player).copied().unwrap_or(false) {
            return Err("Stats selector leaves visible manager slots".into());
        }
        self.selected[player] = !self.selected[player];
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub character: u32,
    pub name: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub state: State,
    pub players: Vec<Player>,
    /// Unmodified retained20000 history pool. Assets supply original graph
    /// bounds; the worker retains host-free data rather than a GPU object.
    pub history: Vec<u8>,
    pub years_per_day: i32,
}
impl Snapshot {
    pub fn from_session(
        session: &crate::fresh_city::Session<'_>,
        state: &State,
    ) -> Result<Self, String> {
        let players = session
            .roster
            .manager
            .players
            .iter()
            .map(|player| {
                let character = u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap());
                if character >= 10 {
                    return Err("Stats player character leaves original descriptor table".into());
                }
                let name = &player.bytes[0x158..0x178];
                let end = name
                    .iter()
                    .position(|&byte| byte == 0)
                    .ok_or("Stats player name lacks original32-byte terminator")?;
                Ok(Player {
                    character,
                    name: name[..end].to_vec(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if players.len() > 6 || state.visible != std::array::from_fn(|slot| slot < players.len()) {
            return Err("Stats snapshot selectors disagree with actualmanager count".into());
        }
        Ok(Self {
            state: state.clone(),
            players,
            history: session.history.as_bytes().to_vec(),
            years_per_day: session.world.settings.years_per_day() as i32,
        })
    }
    pub fn graph(&self, bounds: [f32; 4]) -> Result<Graph, String> {
        Graph::from_history(
            &openmt2001::player_history::State::from_bytes(&self.history)?,
            &self.state,
            bounds,
            self.years_per_day,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Graph {
    /// Native42ccc0/42d220 plot allocation through+64, retained as explicit words.
    pub words: [u32; 26],
    /// Native49e4d8 iterates manager slots backwards. Hidden selectors affect
    /// scale computation, but only selected slots produce these streams.
    pub series: Vec<(usize, Vec<[f32; 2]>)>,
}

pub struct Grid {
    pub rectangles: Vec<([f32; 4], u32)>,
    ///42ce80(x,y,value,decimal precision,format,alignment).
    pub labels: Vec<[u32; 6]>,
}
fn tick_step(raw: f32, count: u32, kind: u32) -> f32 {
    if !raw.is_finite() || raw == 0. {
        return raw;
    }
    let tenth = f64::from(0.1f32);
    if kind == 3 {
        let mut value = f64::from(raw.abs());
        let mut power = 1f64;
        while value < 1. {
            value *= 10.;
            power *= tenth;
        }
        while value > 10. {
            value *= tenth;
            power *= 10.;
        }
        return (value.trunc() * f64::from(power as f32) * f64::from(raw.signum())) as f32;
    }
    let mut value = f64::from((f64::from(count) * f64::from(raw) * 100.) as f32);
    let mut power = f64::from(0.01f32);
    while value > 100. {
        value *= tenth;
        power *= 10.;
    }
    let scale = if value < 8. {
        5.
    } else if value < 17. {
        10.
    } else if kind != 0 && value < 40. {
        20.
    } else if value < 75. {
        50.
    } else {
        100.
    };
    (power * scale / f64::from(count)) as f32
}
impl Graph {
    /// Native42d220 axis stream for49de60's48/32 spacing and supplied
    /// major/minor colors. Its tick index uses nearest-even FIST afterFST,
    /// while the first tick uses truncating FTOL before range admission.
    pub fn grid(&self) -> Result<Grid, String> {
        let f = |i| f32::from_bits(self.words[i]);
        let mut grid = Grid {
            rectangles: Vec::new(),
            labels: Vec::new(),
        };
        if f(10) == 0. || f(11) == 0. {
            return Ok(grid);
        }
        if self.words[14] != 2
            || self.words[15] != 2
            || !matches!(self.words[16], 0 | 3)
            || self.words[17] > 2
            || self.words[12] != 0
            || self.words[13] != 0
        {
            return Err("Stats grid leaves original initialized axis domain".into());
        }
        let step_x = tick_step(
            (f64::from(f(10)) / (f64::from(f(6)) / 48.)) as f32,
            2,
            self.words[16],
        );
        let step_y = tick_step((f64::from(f(11)) / (f64::from(f(7)) / 32.)) as f32, 2, 1);
        if [step_x, step_y]
            .into_iter()
            .any(|step| !step.is_finite() || step <= 0.)
        {
            return Err("Stats grid spacing is nonpositive or non-finite".into());
        }
        let end_x = (f64::from(f(8)) + f64::from(f(10))) as f32;
        let end_y = (f64::from(f(9)) + f64::from(f(11))) as f32;
        let scale_x = (f64::from(f(6)) / f64::from(f(10))) as f32;
        let scale_y = (f64::from(f(7)) / f64::from(f(11))) as f32;
        let first = |minimum: f32, step: f32| {
            // Native FTOL→FILD converts a fractional negative start to integer
            // zero, discarding its sign before multiplication by the step.
            let integer = (f64::from(minimum) / f64::from(step)).trunc() as i32;
            let mut tick = (f64::from(integer) * f64::from(step)) as f32;
            if tick < minimum && (f64::from(tick) - f64::from(minimum)).abs() > f64::from(0.001f32)
            {
                tick = (f64::from(tick) + f64::from(step)) as f32;
            }
            tick
        };
        let color = 0xff1f1f1f;
        let axis_color = 0xff3f3f3f;
        let mut tick = first(f(8), step_x);
        let mut iterations = 0;
        while tick < end_x {
            iterations += 1;
            if iterations > 4096 {
                return Err("Stats grid exceeds bounded native plot ticks".into());
            }
            let x = (f64::from(tick) - f64::from(f(8))) * f64::from(scale_x) + f64::from(f(4));
            let index = ((f64::from(tick) / f64::from(step_x)) as f32).round_ties_even() as i32;
            if index % 2 == 0 {
                grid.labels.push([
                    (x as f32).to_bits(),
                    ((f64::from(f(5)) + f64::from(f(7))) as f32).to_bits(),
                    ((f64::from(tick) * f64::from(f32::from_bits(0x3f800001))) as f32).to_bits(),
                    self.words[18],
                    self.words[16],
                    8,
                ]);
            }
            grid.rectangles.push((
                [
                    x as f32,
                    f(5),
                    (x + 1.) as f32,
                    (f64::from(f(5)) + f64::from(f(7))) as f32,
                ],
                color,
            ));
            tick = (f64::from(tick) + f64::from(step_x)) as f32;
        }
        let mut tick = first(f(9), step_y);
        iterations = 0;
        while tick < end_y {
            iterations += 1;
            if iterations > 4096 {
                return Err("Stats grid exceeds bounded native plot ticks".into());
            }
            let y = f64::from(f(7)) + f64::from(f(5))
                - (f64::from(tick) - f64::from(f(9))) * f64::from(scale_y);
            let index = ((f64::from(tick) / f64::from(step_y)) as f32).round_ties_even() as i32;
            if index % 2 == 0 {
                grid.labels.push([
                    f(4).to_bits(),
                    (y as f32).to_bits(),
                    ((f64::from(tick) * f64::from(f32::from_bits(0x3f800001))) as f32).to_bits(),
                    self.words[19],
                    self.words[17],
                    2,
                ]);
            }
            grid.rectangles.push((
                [
                    f(4),
                    y as f32,
                    (f64::from(f(6)) + f64::from(f(4))) as f32,
                    (y + 1.) as f32,
                ],
                color,
            ));
            tick = (f64::from(tick) + f64::from(step_y)) as f32;
        }
        if f(8) <= f(20) && end_x >= f(20) {
            let x = (f64::from(f(20)) - f64::from(f(8))) * f64::from(scale_x) + f64::from(f(4));
            grid.rectangles.push((
                [
                    (x - 1.) as f32,
                    f(5),
                    (x + 1.) as f32,
                    (f64::from(f(5)) + f64::from(f(7))) as f32,
                ],
                axis_color,
            ));
        }
        if f(9) <= f(21) && end_y >= f(21) {
            let y = f64::from(f(7)) + f64::from(f(5))
                - (f64::from(f(21)) - f64::from(f(9))) * f64::from(scale_y);
            grid.rectangles.push((
                [
                    f(4),
                    (y - 1.) as f32,
                    (f64::from(f(6)) + f64::from(f(4))) as f32,
                    (y + 1.) as f32,
                ],
                axis_color,
            ));
        }
        Ok(grid)
    }
}

///42d970 expands one history line. Native endpoint construction mirrors only
/// X, retaining its neighbor's Y; ordinary perpendicular caps are different.
pub fn line_quads(
    points: &[[f32; 2]],
    plot: [f32; 8],
    width: f32,
) -> Result<Vec<[[f32; 3]; 4]>, String> {
    if points.len() < 2 {
        return Ok(Vec::new());
    }
    if points.len() > 1024
        || plot.iter().chain([&width]).any(|v| !v.is_finite())
        || plot[6] == 0.
        || plot[7] == 0.
        || points.iter().flatten().any(|v| !v.is_finite())
    {
        return Err("Stats line leaves native finite plot domain".into());
    }
    let [
        left,
        top,
        plot_width,
        plot_height,
        min_x,
        min_y,
        range_x,
        range_y,
    ] = plot.map(f64::from);
    let project = |[x, y]: [f64; 2]| {
        [
            (x - min_x) * plot_width / range_x + left,
            plot_height + top - (y - min_y) * plot_height / range_y,
        ]
    };
    let mut edges = Vec::with_capacity(points.len());
    for (index, &point) in points.iter().enumerate() {
        let [x, y] = point.map(f64::from);
        let current = project([x, y]).map(|v| v as f32);
        let previous = if index == 0 {
            [2. * x - f64::from(points[1][0]), f64::from(points[1][1])]
        } else {
            points[index - 1].map(f64::from)
        };
        let next = if index + 1 == points.len() {
            [
                f64::from((2. * x - f64::from(points[index - 1][0])) as f32),
                f64::from(points[index - 1][1]),
            ]
        } else {
            points[index + 1].map(f64::from)
        };
        let a = project(previous);
        let b = project(next);
        let mut a = std::array::from_fn::<_, 2, _>(|i| a[i] - f64::from(current[i]));
        let mut b = std::array::from_fn::<_, 2, _>(|i| b[i] - f64::from(current[i]));
        let normalize = |vector: &mut [f64; 2]| {
            let length = (vector[0] * vector[0] + vector[1] * vector[1]).sqrt();
            if length != 0. {
                vector[0] /= length;
                vector[1] /= length;
            }
        };
        normalize(&mut a);
        normalize(&mut b);
        let mut miter = [-(b[1] - a[1]) as f32, (b[0] - a[0]) as f32];
        let length = (f64::from(miter[0]) * f64::from(miter[0])
            + f64::from(miter[1]) * f64::from(miter[1]))
        .sqrt();
        if length != 0. {
            miter = miter.map(|v| (f64::from(v) / length) as f32);
        }
        let dot = ((b[0] * a[0] + b[1] * a[1]) as f32).clamp(-1., 1.);
        let angle = if dot == -1. {
            f64::from(f32::from_bits(0x40490fdb))
        } else {
            f64::from(dot).acos()
        };
        let sine = (angle * 0.5).sin();
        let scale = if sine < f64::from(0.1f32) {
            10.
        } else {
            1. / sine
        } * f64::from(width)
            * 0.5;
        let dx = (scale * f64::from(miter[0])) as f32;
        let dy = scale * f64::from(miter[1]);
        let plus = [
            (f64::from(current[0]) + f64::from(dx)) as f32,
            (f64::from(current[1]) + dy) as f32,
            0.5,
        ];
        let minus = [
            (f64::from(current[0]) - f64::from(dx)) as f32,
            (f64::from(current[1]) - dy) as f32,
            0.5,
        ];
        edges.push([minus, plus]);
    }
    Ok((0..points.len() - 1)
        .rev()
        .map(|i| [edges[i][0], edges[i][1], edges[i + 1][1], edges[i + 1][0]])
        .collect())
}
impl Graph {
    pub fn from_history(
        history: &openmt2001::player_history::State,
        state: &State,
        bounds: [f32; 4],
        years_per_day: i32,
    ) -> Result<Self, String> {
        let bytes = history.as_bytes();
        let int = |at| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(int(at) as u32);
        let mode = state.mode;
        if mode >= 6 {
            return Err("Stats metric leaves native six-series table".into());
        }
        let frequency = int(0x10f20 + mode * 4);
        if !(0..=1).contains(&frequency) {
            return Err("Stats history frequency leaves native initialized domains".into());
        }
        let hour = int(0x10f38);
        let year = int(0x10f3c)
            .wrapping_add(int(0x10f40).wrapping_mul(10))
            .wrapping_add(1930);
        let mut counts = [0usize; 6];
        for (slot, count) in counts.iter_mut().enumerate() {
            if state.visible[slot] {
                let n = int(0x10e90 + (slot * 6 + mode) * 4);
                if !(0..=480).contains(&n) {
                    return Err("Stats history count leaves retained480-sample ring".into());
                }
                *count = n as usize;
            }
        }
        let maximum = counts.iter().copied().max().unwrap_or(0);
        let mut words = [0u32; 26];
        let [left, top, right, bottom] = bounds;
        let width = right - left;
        let height = bottom - top;
        for (i, v) in [
            left,
            top,
            width,
            height,
            (f64::from(left) + 80.) as f32,
            (f64::from(top) + 5.) as f32,
            (f64::from(width) - 110.) as f32,
            (f64::from(height) - 35.) as f32,
        ]
        .into_iter()
        .enumerate()
        {
            words[i] = v.to_bits();
        }
        words[14] = 2;
        words[15] = 2;
        words[17] = match mode {
            1 | 2 | 4 => 1,
            5 => 2,
            _ => 0,
        };
        words[20] = 1930f32.to_bits();
        let mut min_x = f32::MAX;
        let mut max_x = -f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_y = -f32::MAX;
        let xs: Vec<f32> = (0..maximum)
            .map(|sample| {
                if frequency == 1 {
                    f64::from(year) + f64::from(years_per_day.wrapping_mul(sample as i32))
                } else if maximum > 48 {
                    f64::from(year)
                        + f64::from(years_per_day.wrapping_mul(hour.wrapping_add(sample as i32)))
                            * f64::from(f32::from_bits(0x3d2aaaab))
                } else {
                    f64::from(hour.wrapping_add(sample as i32))
                }
            })
            .map(|v| v as f32)
            .collect();
        if frequency == 0 {
            if maximum > 48 {
                min_x = min_x.min(year as f32);
                max_x = max_x.max(year as f32);
            } else {
                words[16] = 3;
                words[20] = (hour as f32).to_bits();
                let end = hour.wrapping_add(24) as f32;
                min_x = min_x.min(end);
                max_x = max_x.max(end);
            }
        }
        let mut streams = Vec::new();
        for slot in (0..6).rev().filter(|&slot| state.visible[slot]) {
            let cursor = int(0x10e00 + (slot * 6 + mode) * 4);
            if !(0..480).contains(&cursor) {
                return Err("Stats history cursor leaves retained ring".into());
            }
            let count = counts[slot];
            let mut points = Vec::with_capacity(count);
            for (sample, &x) in xs.iter().take(count).enumerate() {
                let index = (cursor + sample as i32 - count as i32 + 480) % 480;
                let y = float(((slot * 6 + mode) * 480 + index as usize) * 4);
                if !x.is_finite() || !y.is_finite() {
                    return Err("Stats history graph contains non-finite sample".into());
                }
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                points.push([x, y]);
            }
            if state.selected[slot] {
                streams.push((slot, points));
            }
        }
        max_y = max_y.max(0.);
        min_y = min_y.min(0.);
        words[22] = min_x.to_bits();
        words[23] = min_y.to_bits();
        words[24] = max_x.to_bits();
        words[25] = max_y.to_bits();
        //42ccc0 keeps PC64 lower margin but stores the upper margin to F32.
        // A zero Y range retains its original +10/-1 asymmetric defaults.
        let delta = f64::from(max_y) - f64::from(min_y);
        let margin = if delta == 0. {
            1.
        } else {
            f64::from(delta as f32) * 0.25
        };
        let upper = if delta == 0. { 10f32 } else { margin as f32 };
        let high = (f64::from(max_y) + f64::from(upper)) as f32;
        let low = (f64::from(min_y) - margin) as f32;
        if max_x > min_x && high > low {
            words[8] = min_x.to_bits();
            words[9] = low.to_bits();
            words[10] = ((f64::from(max_x) - f64::from(min_x)) as f32).to_bits();
            words[11] = ((f64::from(high) - f64::from(low)) as f32).to_bits();
        }
        Ok(Self {
            words,
            series: streams,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "original Stats443360 full widget input and parent notification oracle /037f"]
    fn original_stats_widget_input() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let executable = read("OPENMT2001_EXE");
        let exe_path = openmt2001::fixtures::path("EXE");
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let text = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets
            .chunks_exact(2)
            .map(|row| {
                let end = at + usize::from(u16::from_le_bytes(row.try_into().unwrap()));
                let row = &text[at..end];
                at = end;
                row
            })
            .collect::<Vec<_>>();
        let parameter = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameter.parent().unwrap().parent().unwrap();
        let localized = crate::tooltip::localized_strings(installer, &strings).unwrap();
        let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let assets = Assets::load(&index, &archive, installer, &strings, &executable).unwrap();
        let snapshot = Snapshot {
            state: State::show(0, true, 6).unwrap(),
            players: (0..6)
                .map(|slot| Player {
                    character: (slot * 2 + 3) % 10,
                    name: format!("Player{slot}").into_bytes(),
                })
                .collect(),
            history: vec![0; openmt2001::player_history::RECORD_BYTES],
            years_per_day: 1,
        };
        let bytes = fs::read(openmt2001::fixtures::path("NATIVE_STATS_INPUT")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSTIN01");
        assert_eq!(r.word(), 0x37f);
        for case in 0..r.word() {
            let index = r.word() as usize;
            assert_eq!(r.word(), 3);
            assert_eq!(r.word(), u32::from(index >= 3));
            let mut view = View::default();
            let mut down = false;
            let mut notifications = Vec::new();
            for _ in 0..r.word() {
                let event = r.word();
                let point = [r.word() as i32 as f32, r.word() as i32 as f32];
                if event == 4 {
                    down = true;
                } else if event == 6 {
                    down = false;
                }
                if let Some(actual) = view.pointer(&assets, Some(point), down, &snapshot) {
                    let actual_index = match actual {
                        crate::live_host::StatsAction::Close => 0,
                        crate::live_host::StatsAction::Cycle(right) => 1 + usize::from(right),
                        crate::live_host::StatsAction::Toggle(slot) => 3 + slot,
                        _ => panic!("Widget cannot open itself"),
                    };
                    notifications.push([12, actual_index as u32, 0]);
                }
                if event == 3 {
                    assert_eq!(
                        view.visual_pressed(),
                        None,
                        "native44340f clears the pressed visual on leave"
                    );
                }
            }
            let expected = (0..r.word())
                .map(|_| [r.word(), r.word(), r.word()])
                .collect::<Vec<_>>();
            assert_eq!(
                notifications, expected,
                "native widget sequence{case}/index{index}"
            );
        }
        assert!(r.0.is_empty());
    }
    struct Reader<'a>(&'a [u8]);
    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> &'a [u8] {
            let (a, b) = self.0.split_at(n);
            self.0 = b;
            a
        }
        fn word(&mut self) -> u32 {
            u32::from_le_bytes(self.take(4).try_into().unwrap())
        }
    }
    #[test]
    #[ignore = "requires original Stats frontend pose/bind/draw oracle"]
    fn original_stats_mesh_transform() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_STATS_MESH")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSTMS01");
        assert_eq!(r.word(), 0x37f);
        let size = r.word() as usize;
        assert_eq!(r.word(), 184);
        assert_eq!(r.word(), 0);
        let raw = r.take(size);
        let mesh = openmt2001::mesh::CachedMesh::parse(raw).unwrap();
        r.take(48);
        for point in panel_positions(&mesh).unwrap() {
            assert_eq!(point.map(f32::to_bits), [r.word(), r.word(), r.word()]);
        }
        assert_eq!(r.word(), 34);
        for part in 0..34 {
            assert_eq!(r.word(), part);
        }
        let states: Vec<_> = (0..r.word()).map(|_| (r.word(), r.word())).collect();
        for required in [(14, 1), (7, 1), (23, 4), (27, 1)] {
            assert!(states.contains(&required));
        }
        for _ in 0..r.word() {
            r.word();
            r.take(64);
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires original Stats wrap/style9/full glyph oracle"]
    fn original_stats_text_layout() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_STATS_TEXT")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSTTX01");
        assert_eq!(r.word(), 0x37f);
        let mut fonts = std::collections::BTreeMap::new();
        for _ in 0..r.word() {
            let id = r.word();
            let length = r.word() as usize;
            fonts.insert(id, r.take(length));
        }
        for case in 0..r.word() {
            r.word();
            let font = r.word();
            let style = r.word();
            let bounds = std::array::from_fn(|_| f32::from_bits(r.word()));
            let length = r.word() as usize;
            let quads = text_quads(fonts[&font], r.take(length), bounds, style).unwrap();
            for page in 0..2 {
                let expected = r.word() as usize;
                let actual: Vec<_> = quads.iter().filter(|q| q.page == page).collect();
                assert_eq!(actual.len(), expected, "Stats text case{case}");
                for quad in &actual {
                    for &value in quad.positions.iter().flatten() {
                        assert_eq!(value.to_bits(), r.word(), "Stats text case{case}");
                    }
                }
                for quad in actual {
                    for &value in quad.uv.iter().flatten() {
                        assert_eq!(value.to_bits(), r.word());
                    }
                }
            }
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires whole original42d220 tick/axis producer oracle"]
    fn original_stats_grid_geometry() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_STATS_GRID")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSTGD01");
        assert_eq!(r.word(), 0x37f);
        for case in 0..r.word() {
            let graph = Graph {
                words: std::array::from_fn(|_| r.word()),
                series: Vec::new(),
            };
            assert_eq!(
                [r.word(), r.word(), r.word(), r.word()],
                [48f32.to_bits(), 32f32.to_bits(), 0xff1f1f1f, 0xff3f3f3f]
            );
            let grid = graph.grid().unwrap();
            assert_eq!(grid.rectangles.len(), r.word() as usize, "grid case{case}");
            for (bounds, color) in grid.rectangles {
                assert_eq!(
                    bounds.map(f32::to_bits),
                    [r.word(), r.word(), r.word(), r.word()],
                    "grid case{case}"
                );
                for _ in 0..4 {
                    assert_eq!(r.word(), color);
                }
                assert_eq!(r.word(), 0.5f32.to_bits());
            }
            assert_eq!(grid.labels.len(), r.word() as usize, "grid case{case}");
            for label in grid.labels {
                assert_eq!(label, std::array::from_fn(|_| r.word()), "grid case{case}");
            }
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires original42ce80 linkedCRT/axis fullglyph oracle"]
    fn original_stats_axis_text() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_STATS_AXIS_TEXT")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSTAX01");
        assert_eq!(r.word(), 0x37f);
        let length = r.word() as usize;
        let tab = r.take(length);
        let dictionary = (0..224).collect::<Vec<u8>>();
        for case in 0..r.word() {
            let args = std::array::from_fn(|_| r.word());
            let length = r.word() as usize;
            let expected = r.take(length);
            let actual_text = axis_text(
                tab,
                &dictionary,
                &[b'%' - 32, b's' - 32],
                &[b'$' - 32],
                false,
                args,
            )
            .unwrap();
            let quads = actual_text.glyphs;
            assert_eq!(
                actual_text.encoded, expected,
                "Stats axis formatter case{case}"
            );
            assert_eq!(
                actual_text.color,
                if args[4] == 3 { 0x0fffffff } else { 0xffffffff }
            );
            for page in 0..2 {
                let expected = r.word() as usize;
                let actual: Vec<_> = quads.iter().filter(|q| q.page == page).collect();
                assert_eq!(actual.len(), expected, "Stats axis text case{case}");
                for quad in &actual {
                    for &value in quad.positions.iter().flatten() {
                        assert_eq!(value.to_bits(), r.word(), "Stats axis text case{case}");
                    }
                }
                for quad in actual {
                    for &value in quad.uv.iter().flatten() {
                        assert_eq!(value.to_bits(), r.word());
                    }
                }
            }
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires complete original42d970 polyline geometry oracle"]
    fn original_stats_line_geometry() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_STATS_GRAPH")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSTGR01");
        assert_eq!(r.word(), 0x37f);
        for case in 0..r.word() {
            let count = r.word() as usize;
            let width = f32::from_bits(r.word());
            let draws = r.word() as usize;
            let plot = std::array::from_fn(|_| f32::from_bits(r.word()));
            let points: Vec<_> = (0..count)
                .map(|_| [f32::from_bits(r.word()), f32::from_bits(r.word())])
                .collect();
            let actual = line_quads(&points, plot, width).unwrap();
            assert_eq!(actual.len(), draws);
            for quad in actual {
                for value in quad.into_iter().flatten() {
                    assert_eq!(value.to_bits(), r.word(), "Stats native line case{case}");
                }
                for _ in 0..4 {
                    assert_eq!(r.word(), 0x9f40a0c0);
                }
            }
        }
        assert!(r.0.is_empty());
    }
    #[test]
    #[ignore = "requires pinned original Stats constructor/show/action oracle"]
    fn original_stats_controls() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_STATS_PANEL_GRAPH")).unwrap();
        let mut r = Reader(&bytes);
        assert_eq!(r.take(8), b"MTSTAT01");
        assert_eq!(r.word(), 0x37f);
        let size = r.word() as usize;
        let count = r.word();
        let original = r.take(size);
        let index = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index = openmt2001::archive::ArchiveIndex::parse(&index, archive.len() as u64).unwrap();
        let entry = index.find(PANEL).unwrap();
        assert_eq!(
            original,
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize]
        );
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        for _ in 0..count {
            let players = r.word() as usize;
            let objective = r.word() as i32;
            let running = r.word() != 0;
            let state = State::show(objective, running, players).unwrap();
            assert_eq!(state.mode, r.word() as usize);
            for slot in 0..6 {
                assert_eq!(state.visible[slot], r.word() != 0);
                assert_eq!(state.selected[slot], r.word() != 0);
                let textures: [u32; 6] = std::array::from_fn(|_| r.word());
                if slot < players {
                    let character = (slot * 2 + 3) % 10;
                    let word =
                        |at| u32::from_le_bytes(image.at(at, 4).unwrap().try_into().unwrap());
                    let normal = word(0x541b00 + character as u32 * 40);
                    let hover = word(0x541b08 + character as u32 * 40);
                    assert_eq!(textures, [normal, hover, u32::MAX, normal, hover, normal]);
                }
            }
        }
        for _ in 0..r.word() {
            let running = r.word() != 0;
            let objective = r.word() as i32;
            let before = r.word() as usize;
            let option = r.word();
            let mut state = State::show(objective, running, 6).unwrap();
            state.mode = before;
            state.cycle(option == 2, objective, running);
            assert_eq!(state.mode, r.word() as usize);
            assert_eq!(LABELS[state.mode], r.word());
        }
        for _ in 0..r.word() {
            let count = r.word() as usize;
            let mode = r.word() as usize;
            let frequency = r.word();
            let expected: [u32; 26] = std::array::from_fn(|_| r.word());
            r.word(); // adjacent native start-hour local
            let mut raw = vec![0u8; openmt2001::player_history::RECORD_BYTES];
            let put = |raw: &mut [u8], at: usize, value: u32| {
                raw[at..at + 4].copy_from_slice(&value.to_le_bytes())
            };
            put(&mut raw, 0x10f20 + mode * 4, frequency);
            put(&mut raw, 0x10f38, 3);
            put(&mut raw, 0x10f3c, 2);
            put(&mut raw, 0x10f40, 4);
            for slot in 0..3 {
                put(
                    &mut raw,
                    0x10e00 + (slot * 6 + mode) * 4,
                    (count % 480) as u32,
                );
                put(&mut raw, 0x10e90 + (slot * 6 + mode) * 4, count as u32);
                for sample in 0..count {
                    put(
                        &mut raw,
                        ((slot * 6 + mode) * 480 + sample) * 4,
                        (((sample as i32 - 7) * (slot as i32 + 1)) as f32 * 1.25).to_bits(),
                    );
                }
            }
            let mut state = State::show(0, true, 3).unwrap();
            state.mode = mode;
            let history = openmt2001::player_history::State::from_bytes(&raw).unwrap();
            let bounds = [0x421b273f, 0x42c75adb, 0x44153274, 0x43b59c1c].map(f32::from_bits);
            let graph = Graph::from_history(&history, &state, bounds, 1).unwrap();
            assert_eq!(
                graph.words, expected,
                "native graph count{count}/mode{mode}/frequency{frequency}"
            );
            assert_eq!(r.word() as usize, graph.series.len());
            for (slot, points) in graph.series {
                assert_eq!(r.word() as usize, points.len());
                let character = (slot * 2 + 3) % 10;
                let color = u32::from_le_bytes(
                    image
                        .at(0x541af0 + character as u32 * 40, 4)
                        .unwrap()
                        .try_into()
                        .unwrap(),
                );
                assert_eq!(r.word(), color & 0xffffff | 0x9f000000);
                assert_eq!(r.word(), 4f32.to_bits());
                for point in points {
                    assert_eq!(point.map(f32::to_bits), [r.word(), r.word()]);
                }
            }
        }
        assert!(r.0.is_empty());
    }
}

#[cfg(test)]
mod mesh_culling_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn stats_mesh_winding_degeneracy_and_retained_order() {
        let points = [
            [0., 0., 7.],
            [0., 2., 7.],
            [3., 2., 7.],
            [3., 0., 7.],
            [1.5, 1., 7.],
        ];
        let indices = [0, 2, 1, 0, 1, 2, 0, 4, 2, 0, 0, 1, 0, 2, 3];
        assert_eq!(
            stats_mesh_triangles(&points, &indices).collect::<Vec<_>>(),
            [[0, 1, 2], [0, 2, 3]]
        );
        // A real screen reflection changes which face survives. Translation,
        // positive scaling and depth changes do not reverse screen winding.
        let reflected = points.map(|[x, y, z]| [x, -y, z]);
        assert_eq!(
            stats_mesh_triangles(&reflected, &indices).collect::<Vec<_>>(),
            [[0, 2, 1]]
        );
        let moved = points.map(|[x, y, _]| [x * 2. + 10., y * 3. - 5., -42.]);
        assert_eq!(
            stats_mesh_triangles(&moved, &indices).collect::<Vec<_>>(),
            [[0, 1, 2], [0, 2, 3]]
        );
        assert_eq!(
            stats_mesh_triangles(&points, &[0, 4, 2, 0, 0, 1]).count(),
            0
        );
    }
    fn guard(bytes: &[u8], expected: &str) {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut process = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("fixture checks require sha256sum");
        process.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = process.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::str::from_utf8(&output.stdout)
                .unwrap()
                .split_whitespace()
                .next(),
            Some(expected)
        );
    }
    fn guarded(path: &Path, expected: &str) -> Vec<u8> {
        let bytes = fs::read(path).unwrap();
        guard(&bytes, expected);
        bytes
    }
    struct Supplied {
        directory: Vec<u8>,
        archive: Vec<u8>,
        executable: Vec<u8>,
        installer: PathBuf,
        strings: Vec<Vec<u8>>,
    }
    impl Supplied {
        fn read() -> Self {
            let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
            let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
            let executable = guarded(
                &openmt2001::fixtures::path("EXE"),
                "1401be713210eec37f2eddfedb49afb358cc252a566c6bb0e0d6a64e1c7d4e2b",
            );
            let installer = openmt2001::fixtures::path("PARAMETERS")
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf();
            for (name, hash) in [
                (
                    "cust640p.tab",
                    "082d6a0488289a0926ee3b2e279882a03c31922e6f1ec6aa75e56a6ffc1d72d5",
                ),
                (
                    "tahoma12bldnoantip.tab",
                    "03b9924e83bb7a8c650bd58c87bcb3d669dcdf0198bb240e5f964d434d8c62be",
                ),
                (
                    "coppgoth23p.tab",
                    "5e5ac4756550b3b22058510de5fafa31114709b10735fdfb41823b7d283ac300",
                ),
            ] {
                guarded(&installer.join("Standard/fonts").join(name), hash);
            }
            let language = openmt2001::fixtures::path("STATS_LANGUAGE");
            let off = guarded(
                &language.join("tycoon.off"),
                "54a2c8adf44e37138bcaf36bf9c9218e0543880e308a0b9cc9a883ee98265b4d",
            );
            let bin = guarded(
                &language.join("tycoon.bin"),
                "4d5e302035f3579d2c0cb8cd35a58abae5ea6d8159e6c3661bef091fedff6c38",
            );
            assert_eq!(off.len() % 2, 0);
            let mut at = 0;
            let strings = off
                .chunks_exact(2)
                .map(|n| {
                    let end = at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
                    let row = bin.get(at..end).unwrap().to_vec();
                    at = end;
                    row
                })
                .collect();
            assert_eq!(at, bin.len());
            let supplied = Self {
                directory,
                archive,
                executable,
                installer,
                strings,
            };
            for (name, hash) in [
                (
                    PANEL,
                    "2b80d6798bd9a42ec464979d29d2e75f1b0acb65f28146f2d4c2ac3685a45958",
                ),
                (
                    "1New_Maya_grey_border.tt",
                    "31a3aafc028c5b3798eb8fb8eca314c1f2cf4e88b0fd3af3204d9c004396cd9f",
                ),
                (
                    "front_end_background_blue.tt",
                    "c43054ad3f84223b7719938db42b92675648f361d47a1f1acf842dfecd2957bb",
                ),
                (
                    "FRONT_END_BLUE_BOARD.TT",
                    "6b36529ac23bdfbeadc166a9838d8ed2dc716aad52093ffad4f77a30158772a0",
                ),
                (
                    "4by4_colours_black4by4.tt",
                    "9a19ef3b8659aab6370b591615e418b5776d12b7de5ad3a85cea92a83e95a934",
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
                guard(supplied.payload(name), hash);
            }
            supplied
        }
        fn payload(&self, name: &str) -> &[u8] {
            let index = ArchiveIndex::parse(&self.directory, self.archive.len() as u64).unwrap();
            let entry = index.find(name).unwrap();
            &self.archive[entry.offset as usize..(entry.offset + entry.length) as usize]
        }
        fn assets(&self) -> Assets {
            let index = ArchiveIndex::parse(&self.directory, self.archive.len() as u64).unwrap();
            let refs = self.strings.iter().map(Vec::as_slice).collect::<Vec<_>>();
            Assets::load(
                &index,
                &self.archive,
                &self.installer,
                &refs,
                &self.executable,
            )
            .unwrap()
        }
    }
    #[test]
    #[ignore = "requires SHA-pinned original Stats mesh/assets and sha256sum"]
    fn supplied_stats_mesh_retains_borders_background_and_all_regions() {
        let supplied = Supplied::read();
        let mesh = CachedMesh::parse(supplied.payload(PANEL)).unwrap();
        assert_eq!(
            (mesh.vertex_count, mesh.parts.len(), mesh.frames),
            (184, 34, 1)
        );
        let positions = panel_positions(&mesh).unwrap();
        let uv = mesh.uvs();
        let assets = supplied.assets(); // exercise the actual production loader
        let retained = [0, 1, 2, 3, 4, 6, 7, 8, 9];
        let rejected: Vec<_> = (0..34).filter(|part| !retained.contains(part)).collect();
        assert_eq!(rejected, [5].into_iter().chain(10..34).collect::<Vec<_>>());
        let mut kept_triangles = 0;
        let mut rejected_triangles = 0;
        for (part_id, part) in mesh.parts.iter().enumerate() {
            let indices = part.vertex_indices().collect::<Vec<_>>();
            let visible = stats_mesh_triangles(&positions, &indices).collect::<Vec<_>>();
            if retained.contains(&part_id) {
                assert_eq!(
                    visible
                        .iter()
                        .flat_map(|tri| tri.iter().copied())
                        .collect::<Vec<_>>(),
                    indices,
                    "front-facing part{part_id} changed order"
                );
                kept_triangles += visible.len();
            } else {
                assert!(visible.is_empty(), "layout part{part_id} became visible");
                rejected_triangles += indices.len() / 3;
            }
        }
        assert_eq!((retained.len(), kept_triangles), (9, 42));
        assert_eq!((rejected.len(), rejected_triangles), (25, 50));
        assert_eq!(assets.model.len(), 9);
        // All supplied named bounds survive, including non-rendered rectangles.
        let regions = openmt2001::ui_mesh::regions(&mesh, 0)
            .unwrap()
            .into_iter()
            .map(|region| (region.name, region.bounds))
            .collect::<Vec<_>>();
        assert_eq!(assets.regions, regions);
        assert_eq!(assets.regions.len(), 34);
        assert_eq!(
            assets.root,
            openmt2001::ui_mesh::parent_bounds(&mesh, 0).unwrap()
        );
        assert_eq!(
            assets.bounds("STATS_BACKING3"),
            [32.017193, 66.70275, 603.9501, 404.15662]
        );
        assert_ne!(assets.bounds("STATS_TOKEN_BACKING06"), [0.; 4]);
        assert_eq!(assets.body, 0xbb000a28);
        // Check actual output surfaces against original retained indices, UVs,
        // alpha and original texture bytes, not a mirror of the cull predicate.
        for (actual, &part_id) in assets.model.iter().zip(&retained) {
            let part = &mesh.parts[part_id];
            let expected_vertices = part
                .vertex_indices()
                .map(|vertex| {
                    [
                        positions[vertex][0],
                        positions[vertex][1],
                        positions[vertex][2],
                        1.,
                        1.,
                        1.,
                        uv[vertex][0],
                        uv[vertex][1],
                    ]
                })
                .collect::<Vec<_>>();
            assert_eq!(
                actual.vertices, expected_vertices,
                "submitted part{part_id}"
            );
            assert_eq!(actual.alpha, vec![255; expected_vertices.len()]);
            let textures =
                TextureSet::parse(supplied.payload(mesh.texture_names[part.texture as usize]))
                    .unwrap();
            let expected = Surface::new(&textures.textures[0]);
            assert_eq!(
                (actual.width, actual.height, &actual.pixels),
                (expected.width, expected.height, &expected.pixels)
            );
        }
        let gray = TextureSet::parse(supplied.payload("1New_Maya_grey_border.tt")).unwrap();
        assert_eq!((gray.textures[0].width, gray.textures[0].height), (4, 4));
        assert!(
            gray.textures[0]
                .texels
                .chunks_exact(4)
                .all(|p| p == [146, 144, 144, 255])
        );
        for part in [4, 6, 7, 8, 9] {
            assert!(
                mesh.texture_names[mesh.parts[part].texture as usize]
                    .eq_ignore_ascii_case("1New_Maya_grey_border.tt")
            );
            assert!(retained.contains(&part));
        }
        let background =
            TextureSet::parse(supplied.payload("front_end_background_blue.tt")).unwrap();
        assert_eq!(
            (background.textures[0].width, background.textures[0].height),
            (32, 28)
        );
        assert!(
            background.textures[0]
                .texels
                .chunks_exact(4)
                .all(|p| p[3] == 128)
        );
        assert!(
            mesh.texture_names[mesh.parts[3].texture as usize]
                .eq_ignore_ascii_case("front_end_background_blue.tt")
        );
    }

    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "SHA-pinned supplied assets/save, offscreen GPU and output directory"]
    fn capture_stats_surround_over_nonuniform_underlay() {
        let supplied = Supplied::read();
        let assets = supplied.assets();
        let saved = guarded(
            &openmt2001::fixtures::path("STATS_SAVE_ONE"),
            "2c3bfdbbdaeb2e12e08752e9f3ed05d6e5c4dc60aa48208113f46a84b329dded",
        );
        let save = openmt2001::save::SaveFile::parse(&saved).unwrap();
        let players = save
            .players()
            .unwrap()
            .iter()
            .map(|player| {
                // SavedPlayer bytes start at runtime+4, not runtime+0.
                let character = u32::from_le_bytes(player.bytes[0x14c..0x150].try_into().unwrap());
                let raw = &player.bytes[0x154..0x174];
                let end = raw.iter().position(|&b| b == 0).unwrap();
                Player {
                    character,
                    name: raw[..end].to_vec(),
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(players.len(), 1);
        let history = save.player_history().unwrap().as_bytes().to_vec();
        let supplied_one = Snapshot {
            state: State::show(6, true, 1).unwrap(),
            players,
            history,
            years_per_day: 1,
        }; // component display scale only
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let output = PathBuf::from(std::env::var("OPENMT2001_STATS_CAPTURE_DIR").unwrap());
        assert!(output.is_dir());
        for count in [0, 1, 6] {
            let mut snapshot = supplied_one.clone();
            snapshot.state = State::show(6, true, count).unwrap();
            if count == 0 {
                snapshot.players.clear();
            }
            if count == 6 {
                // Explicit synthetic roster diagnostic; no supplied six-player
                // save/city state is claimed or manufactured as gameplay evidence.
                snapshot.players = (0..6)
                    .map(|slot| Player {
                        character: slot as u32,
                        name: format!("Synthetic{slot}").into_bytes(),
                    })
                    .collect();
            }
            let prepared = assets.prepare(&renderer, &snapshot, None, None).unwrap();
            let mut checker = Surface::solid();
            for row in 0..15 {
                for col in 0..20 {
                    let rgb = if (row + col) % 2 == 0 {
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
                        [rgb; 4],
                    );
                }
            }
            let underlay = renderer
                .prepare(
                    &[checker.draw()],
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
                .unwrap();
            let label = if count == 1 {
                "supplied-one"
            } else {
                "synthetic-slots"
            };
            let path = output.join(format!("stats-{label}-{count}-underlay.png"));
            crate::capture_test::capture_overlay(
                &renderer,
                &underlay,
                &prepared,
                path.to_str().unwrap(),
            );
        }
        // No input/session/profile mutation or original runtime was performed.
    }
}
