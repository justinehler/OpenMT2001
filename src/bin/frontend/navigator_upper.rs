//! Upper F43d010/43d130/43dd90/43d290/43db40. Private, unapplied.
//! Nested under navigator.rs. No camera, clock, worker or Scene authority.
use openmt2001::{
    camera::primary::{AngularTrig, CursorRequest},
    render::{PreparedScene, Renderer},
    x87::{Sum, portable_trig::PortableTrig},
};
use std::sync::Arc;
type Result<T> = std::result::Result<T, String>;
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn f(b: &[u8], at: usize) -> f32 {
    f32::from_bits(word(b, at))
}
fn s(f: f32) -> Result<Sum> {
    Sum::from_f32(f).map_err(str::to_owned)
}
fn mul(mut a: Sum, b: f32) -> Result<Sum> {
    a.mul_f32(b).map_err(str::to_owned)?;
    Ok(a)
}
fn add(mut a: Sum, b: f32) -> Result<Sum> {
    a.add_f32(b).map_err(str::to_owned)?;
    Ok(a)
}
fn plus(mut a: Sum, b: Sum) -> Sum {
    a.add(b);
    a
}
fn minus(a: Sum, b: Sum) -> Sum {
    plus(a, b.negated())
}
fn product(mut a: Sum, b: Sum) -> Sum {
    a.mul(b);
    a
}
fn round(a: Sum) -> Result<Sum> {
    s(a.to_f32())
}

pub use openmt2001::navigator_inputs::{Inputs, Producer, Tables};

/// Actual93c5c0/dc logical coordinates and93c60d retained independently of
/// display hover. The real source leaves these cells available off-window.
#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub xy: [i32; 2],
    pub blocked: u8,
    pub restricted: u8,
    pub allowed: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub bounds: [f32; 4],
    pub second: [f32; 4],
    pub backing: [f32; 4],
    pub parent_open: [f32; 4],
    pub parent_closed: [f32; 4],
}
impl Geometry {
    pub fn origin(&self, offset: [f32; 2]) -> Result<[f32; 2]> {
        let [l, t, r, b] = self.bounds;
        Ok([
            add(mul(add(s(r)?, l)?, 0.5)?, offset[0])?.to_f32(),
            add(mul(add(s(b)?, t)?, 0.5)?, offset[1])?.to_f32(),
        ])
    }
    ///4342c0: strict X BOTH sides, inclusive Y. Each sum remains PC64.
    pub fn contains(&self, p: Pointer, offset: [f32; 2]) -> Result<bool> {
        if p.blocked != 0 {
            return Ok(false);
        }
        let [l, t, r, b] = self.bounds;
        let [x, y] = p.xy.map(Sum::from_i32);
        use std::cmp::Ordering::{Greater, Less};
        Ok(x.compare(add(s(l)?, offset[0])?) == Greater
            && x.compare(add(s(r)?, offset[0])?) == Less
            && y.compare(add(s(t)?, offset[1])?) != Less
            && y.compare(add(s(b)?, offset[1])?) != Greater)
    }
}
#[derive(Clone, Debug)]
pub struct State {
    pub geometry: Geometry,
    pub selected: i32, //native F+15c0, hover result; NOT822d50 selection
    pub active: u8,    //native F+15c4, hover-valid; NOTclick latch
    pub enabled: u8,   //native F+15e0, DOWN4/UP6 latch; NOTsuppress-update
    pub pointer: Pointer,
    pub input: Arc<Inputs>,
}
#[derive(Clone, Debug)]
pub struct Quad {
    pub points: [[f32; 3]; 4],
    pub color: u32,
}
#[derive(Clone, Copy, Debug)]
pub enum Texture {
    Map,
    Ownership,
    Arrow,
    Colored,
}
#[derive(Clone, Debug)]
pub struct Batch {
    pub texture: Texture,
    pub quads: Vec<Quad>,
}
#[derive(Clone, Debug)]
pub struct Draw {
    pub batches: Vec<Batch>,
}
#[derive(Clone, Debug)]
pub struct Hover {
    pub cursor: Vec<CursorRequest>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    Done,
    Click(i32),
}
impl State {
    /// Only process/layout factory supplies these consumed cells. Warm scene
    /// refresh and world layout-reset retain selected/active (43d010 does not
    /// store them). All values come from actual first writers/PE allocation.
    pub fn installed(
        geometry: Geometry,
        selected: i32,
        active: u8,
        enabled: u8,
        pointer: Pointer,
        input: Arc<Inputs>,
    ) -> Self {
        Self {
            geometry,
            selected,
            active,
            enabled,
            pointer,
            input,
        }
    }
    pub fn reset_layout(&mut self, geometry: Geometry) {
        self.enabled = 0;
        self.geometry = geometry;
    }
    ///43dd90's modal-over-bounds branch returns BEFORE resetting selected.
    ///The ordinary all-miss loop explicitly writes selected=-1/active=0.
    pub fn hover(&mut self, offset: [f32; 2], modal: u8, issue: bool) -> Result<Hover> {
        let inside = self.geometry.contains(self.pointer, offset)?;
        let mut cursor = Vec::new();
        if modal == 1 && inside {
            cursor.push(CursorRequest {
                state: 0,
                priority: 1,
            });
            return Ok(Hover { cursor });
        }
        if inside {
            cursor.push(CursorRequest {
                state: 0,
                priority: 1,
            });
        }
        self.selected = -1;
        self.active = 0;
        let origin = self.geometry.origin(offset)?;
        let limit = self.input.count; //native snapshots once for hover
        for i in 0..limit.max(0) as usize {
            let row = self
                .input
                .rows
                .get(i)
                .ok_or("F hover count outside live52")?;
            if row[0x90] & 1 != 0 {
                continue;
            }
            let width = mul(s(f(row, 0xd0))?, 0.1)?; //retained width
            let height = mul(s(f(row, 0xd4))?, 0.1)?.to_f32();
            let left = minus(
                add(mul(s(f(row, 0xe0))?, 0.1)?, origin[0])?,
                mul(width, 0.5)?,
            );
            let top = minus(
                add(mul(s(f(row, 0xe4))?, -0.1)?, origin[1])?,
                mul(s(height)?, 0.5)?,
            )
            .to_f32();
            //43de9c left/right use retained80; top was stored F32 at43de96.
            if left.ftol_low_word() > self.pointer.xy[0] {
                continue;
            }
            if plus(left, width).ftol_low_word() < self.pointer.xy[0] {
                continue;
            }
            if s(top)?.ftol_low_word() > self.pointer.xy[1] {
                continue;
            }
            if add(s(top)?, height)?.ftol_low_word() < self.pointer.xy[1] {
                continue;
            }
            if self.pointer.restricted == 1 && i as i32 != self.pointer.allowed {
                continue;
            }
            self.selected = i as i32;
            if issue {
                cursor.push(CursorRequest {
                    state: 1,
                    priority: 1,
                });
            }
            self.active = 1;
            return Ok(Hover { cursor });
        }
        Ok(Hover { cursor })
    }
    ///43d130 suffix. Prelude hover is invoked by upper_f::message from node1d,
    ///for EVERY event (including1/11/3). Parameter12 zero falls into4.
    pub fn message_suffix(&mut self, event: u32, parameter: i32, modal: u8) -> Message {
        match event {
            3 | 6 => {
                self.enabled = 0;
                Message::Done
            }
            4 => {
                self.enabled = 1;
                Message::Done
            }
            12 if parameter == 0 => {
                self.enabled = 1;
                Message::Done
            }
            5 if modal != 1 && self.enabled != 0 && self.selected > -1 => {
                Message::Click(self.selected)
            }
            _ => Message::Done,
        }
    }
    ///43d290's primary focus effects happen DURING rebuild, not hover or a
    ///second point pick. Negative indices are never looked up: native loops
    ///nonnegative rows and compares each row with selected only if active==1.
    pub fn focus_category(&self) -> Option<usize> {
        (0..self.input.count.max(0) as usize).find(|&i| {
            self.input.rows[i][0x90] & 1 == 0 && self.active == 1 && self.selected == i as i32
        })
    }
    pub fn category_label(&self, index: usize) -> Result<u32> {
        let row = self
            .input
            .rows
            .get(index)
            .ok_or("F category descriptor index outside live owner")?;
        let title = usize::try_from(word(row, 0x8c) as i32)
            .map_err(|_| "F category title before supplied property-title table")?;
        self.input
            .property_labels
            .get(title)
            .copied()
            .ok_or_else(|| "F category title outside supplied property-title table".into())
    }
    pub fn clears_primary(&self, offset: [f32; 2]) -> Result<bool> {
        Ok(self.active == 0 && self.geometry.contains(self.pointer, offset)?)
    }
    ///43d0f0: callers must perform focus/material effects then read actual raw
    ///Owner eye+4/+c/yaw+7c AFTER rebuild, before calling arrow. See upper_f.
    pub fn map(&self, offset: [f32; 2], alpha: u8) -> Result<Vec<Batch>> {
        let origin = self.geometry.origin(offset)?;
        let texture = |t| {
            if self.input.presentation_mode == 0 {
                t
            } else {
                Texture::Colored
            }
        };
        let mut base = Batch {
            texture: texture(Texture::Map),
            quads: Vec::new(),
        };
        let mut ownership = Batch {
            texture: texture(Texture::Ownership),
            quads: Vec::new(),
        };
        let distance = ((self.input.phase & 31) as i32 - 15).abs();
        for i in 0..self.input.count.max(0) as usize {
            let row = &self.input.rows[i];
            if row[0x90] & 1 == 0 {
                let extent =
                    [f(row, 0xd0), f(row, 0xd4)].map(|v| mul(s(v)?, 0.12).map(Sum::to_f32));
                let extent = [extent[0].clone()?, extent[1].clone()?];
                let (left, top) = placement(origin, row, extent)?;
                let color = if i == 19 {
                    0
                } else {
                    (word(row, 0x94) & 0xffffff) | (u32::from(alpha) << 24)
                };
                base.quads.push(quad(s(left)?, s(top)?, extent, color)?);
            }
            if row[4] != 0 && row[5] != 0 {
                let owner = self.input.owners[i]
                    .ok_or("F active owner missing actual Manager resolution")?;
                let extent = [
                    mul(s(f(row, 0xd0))?, 0.075)?.to_f32(),
                    mul(s(f(row, 0xd4))?, 0.075)?.to_f32(),
                ];
                let (left, top) = placement_extended(origin, row, extent)?;
                let color = if i == 19 {
                    0
                } else if owner.remaining <= 24 {
                    if self.input.phase & 15 > 8 {
                        0
                    } else {
                        owner.color
                    }
                } else if owner.current {
                    let factor = add(mul(Sum::from_i32(distance), 0.03125)?, 0.5)?;
                    let component = |shift| -> Result<u32> {
                        Ok(product(
                            Sum::from_i32(((owner.color >> shift) & 255u32) as i32),
                            factor,
                        )
                        .ftol_low_word() as u32)
                    };
                    let r = component(16)?;
                    let g = component(8)?;
                    let b = component(0)?;
                    r.wrapping_sub(256)
                        .wrapping_shl(8)
                        .wrapping_add(g)
                        .wrapping_shl(8)
                        .wrapping_add(b)
                } else {
                    owner.color
                };
                ownership.quads.push(quad(s(left)?, top, extent, color)?);
            }
        }
        let mut batches = vec![base]; //native submits first batch even count0
        if !ownership.quads.is_empty() {
            batches.push(ownership);
        }
        Ok(batches)
    }
}
fn placement(origin: [f32; 2], row: &[u8; 380], extent: [f32; 2]) -> Result<(f32, f32)> {
    let (l, t) = placement_extended(origin, row, extent)?;
    Ok((l, t.to_f32()))
}
fn placement_extended(origin: [f32; 2], row: &[u8; 380], extent: [f32; 2]) -> Result<(f32, Sum)> {
    let l = minus(
        add(mul(s(f(row, 0xe0))?, 0.1)?, origin[0])?,
        mul(s(extent[0])?, 0.5)?,
    )
    .to_f32();
    let t = minus(
        add(mul(s(f(row, 0xe4))?, -0.1)?, origin[1])?,
        mul(s(extent[1])?, 0.5)?,
    );
    Ok((l, t))
}
fn quad(left: Sum, top: Sum, extent: [f32; 2], color: u32) -> Result<Quad> {
    let right = add(left, extent[0])?.to_f32();
    let bottom = add(top, extent[1])?.to_f32();
    let left = left.to_f32();
    let top = top.to_f32();
    Ok(Quad {
        points: [
            [left, top, 0.5],
            [left, bottom, 0.5],
            [right, bottom, 0.5],
            [right, top, 0.5],
        ],
        color,
    })
}
///43db40 uses actual logic eye and yaw. No atan2, sampled pose, local clock,
///camera stepping or copies of evolving camera records are introduced.
pub fn arrow(
    geometry: Geometry,
    offset: [f32; 2],
    eye: [u32; 2],
    yaw: u32,
    phase: u32,
) -> Result<Batch> {
    let origin = geometry.origin(offset)?;
    let mut trig = PortableTrig;
    let a = s(f32::from_bits(yaw))?;
    let negative = a.negated();
    let distance = mul(Sum::from_i32(((phase & 31) as i32 - 15).abs()), 1f32 / 15.)?;
    let pulse = mul(product(distance, distance), 4.)?;
    let x = plus(
        add(mul(s(f32::from_bits(eye[0]))?, 0.1)?, origin[0])?,
        product(trig.cosine(negative).map_err(str::to_owned)?, pulse),
    )
    .to_f32();
    let mut x = s(x)?;
    let mut y = plus(
        add(mul(s(f32::from_bits(eye[1]))?, -0.1)?, origin[1])?,
        product(trig.sine(negative).map_err(str::to_owned)?, pulse),
    );
    use std::cmp::Ordering::{Greater, Less};
    if minus(x, s(origin[0])?).compare(s(-60.)?) == Less {
        x = round(add(s(origin[0])?, -60.)?)?;
    }
    if minus(x, s(origin[0])?).compare(s(60.)?) == Greater {
        x = round(add(s(origin[0])?, 60.)?)?;
    }
    if minus(y, s(origin[1])?).compare(s(-50.)?) == Less {
        y = add(s(origin[1])?, -50.)?;
    }
    if minus(y, s(origin[1])?).compare(s(50.)?) == Greater {
        y = add(s(origin[1])?, 50.)?;
    }
    let rotation = minus(s(f32::from_bits(0x3f490fdb))?, a);
    //Native FSTP rounds sin before scaling; cosine remains retained.
    let sin = mul(round(trig.sine(rotation).map_err(str::to_owned)?)?, 20.)?;
    let cos = mul(trig.cosine(rotation).map_err(str::to_owned)?, 20.)?;
    let ns = round(mul(sin, -0.5)?)?;
    let nc = round(mul(cos, -0.5)?)?;
    let pc = mul(cos, 0.5)?;
    let ps = mul(round(sin)?, 0.5)?;
    let points = [
        [
            plus(x, mul(sin, -0.5)?).to_f32(),
            minus(y, nc).to_f32(),
            0.5,
        ],
        [plus(x, pc).to_f32(), minus(y, ns).to_f32(), 0.5],
        [plus(x, ps).to_f32(), minus(y, pc).to_f32(), 0.5],
        [plus(x, nc).to_f32(), minus(y, round(ps)?).to_f32(), 0.5],
    ];
    Ok(Batch {
        texture: Texture::Arrow,
        quads: vec![Quad {
            points,
            color: 0xffffffff,
        }],
    })
}
///Real retained GPU consumer using the existing Navigator textures. Render
///state's draw is built from the SAME State used by hover/focus/phase11.
pub fn prepare(
    nav: &super::Navigator,
    renderer: &Renderer,
    draw: &Draw,
    geometry: Geometry,
    frame: u16,
    offset: [f32; 2],
    alpha: u8,
    model: bool,
    body: bool,
) -> Result<PreparedScene> {
    let mut surfaces = Vec::new();
    //43c3a0: specialized backing, bound model, specialized body, children.
    //The source-backed F tree is null; no generic child draw is omitted.
    if body {
        let mut backing = crate::ui::Surface::solid();
        let [l, t, r, b] = geometry.backing;
        backing.rect(
            [l + offset[0], t + offset[1], r + offset[0], b + offset[1]],
            [nav.upper_backing_color; 4],
        );
        surfaces.push(backing);
    }
    if model {
        let mut shell = nav
            .upper_shells
            .get(frame as usize)
            .ok_or("F current frame outside supplied model")?
            .clone();
        for surface in &mut shell {
            for vertex in &mut surface.vertices {
                vertex[0] += offset[0];
                vertex[1] += offset[1];
            }
            surface.alpha.fill(alpha);
        }
        surfaces.extend(shell);
    }
    for batch in &draw.batches {
        let mut surface = match batch.texture {
            Texture::Map => nav.surfaces.last().ok_or("F map texture absent")?.clone(),
            Texture::Ownership => nav.ownership.clone(),
            Texture::Arrow => nav.arrow.clone(),
            Texture::Colored => crate::ui::Surface::solid(),
        };
        surface.vertices.clear();
        surface.alpha.clear();
        for q in &batch.quads {
            surface.quad(
                q.points.map(|p| [p[0], p[1]]),
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [q.color; 4],
            );
        }
        surfaces.push(surface);
    }
    renderer.prepare(
        &surfaces
            .iter()
            .map(crate::ui::Surface::draw)
            .collect::<Vec<_>>(),
        super::overlay_matrix(),
        None,
    )
}
