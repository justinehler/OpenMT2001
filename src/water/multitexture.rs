//! Isolated v1.4 water modes 15/16. No live scene selects this path implicitly.
//! Callers must supply original enable/ValidateDevice/Detail facts and provenance.
use super::{WaterPhase, WaterVertex};
use crate::x87::Sum;
use std::cmp::Ordering;

/// One correlated original state observation, not a query of modern GPU support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteFacts<'a> {
    pub enabled: bool,
    pub valid15: bool,
    pub valid16: bool,
    pub detail: i32,
    provenance: &'a str,
}

impl<'a> RouteFacts<'a> {
    /// The reference must identify the source of all four facts. A nonempty
    /// identifier records custody; it does not authenticate the observation.
    pub fn new(
        enabled: bool,
        valid15: bool,
        valid16: bool,
        detail: i32,
        provenance: &'a str,
    ) -> Result<Self, &'static str> {
        if provenance.trim().is_empty() {
            return Err("water route requires explicit original-state provenance");
        }
        Ok(Self {
            enabled,
            valid15,
            valid16,
            detail,
            provenance,
        })
    }

    pub fn provenance(self) -> &'a str {
        self.provenance
    }

    /// 004c5370: base mode9 is submitted separately before this route.
    pub fn route(self) -> Route {
        if !self.enabled || !self.valid16 {
            Route::Fallback
        } else if self.detail > 0 && self.valid15 {
            Route::FirstAndQuality
        } else {
            Route::First
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Fallback,
    First,
    FirstAndQuality,
}

impl Route {
    /// Pass order, each submitted river then sea. Successful gates also select
    /// their mode; gate15's success is followed by the dispatcher's mode15 select.
    pub fn modes(self) -> &'static [u32] {
        match self {
            Self::Fallback => &[20],
            Self::First => &[16],
            Self::FirstAndQuality => &[16, 15],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    First,
    Quality,
}

/// Complete fixed preset fields, excluding name/id/validation metadata.
/// These are original selectors, not D3D device enum values. In particular zero
/// selectors can inherit earlier state; do not convert them to guessed defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Material {
    pass: Pass,
    render: [u32; 24],      // original record +10..6f
    stages: [[u32; 14]; 8], // +70, stride38; every reserved/unused word retained
}

impl Material {
    /// 004599ee..00459bdc: modes15/16 have two stages and zero LOD bias.
    pub fn original(pass: Pass) -> Self {
        let mut render = [
            1, 2, 2, 2, 2, 0, 0, 0, 0, 0, 2, 1, 4, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2,
        ];
        if pass == Pass::Quality {
            render[11] = 2; // ONE rather than ZERO source blend
            render[12] = 2; // ONE rather than SRCCOLOR destination blend
        }
        let mut stages = [[0; 14]; 8];
        stages[0] = [2, 0, 0, 2, 2, 1, 1, 1, 1, 3, 1, 1, 3, 1];
        stages[1] = [2, 0, 0, 2, 2, 1, 1, 1, 1, 2, 1, 1, 2, 1];
        stages[2][0] = 1; // +e0: explicitly disable the following stage
        Self {
            pass,
            render,
            stages,
        }
    }

    pub fn mode(&self) -> u32 {
        match self.pass {
            Pass::First => 16,
            Pass::Quality => 15,
        }
    }

    /// Read a fixed field at its documented original offset. Metadata +00..0f
    /// is intentionally absent: ValidateDevice success is a caller fact.
    pub fn field(&self, offset: usize) -> Option<u32> {
        if offset % 4 != 0 {
            return None;
        }
        match offset {
            0x10..=0x6c => Some(self.render[(offset - 0x10) / 4]),
            0x70..=0x22c => {
                let word = (offset - 0x70) / 4;
                Some(self.stages[word / 14][word % 14])
            }
            _ => None,
        }
    }

    pub fn stage(&self, index: usize) -> Option<&[u32; 14]> {
        self.stages.get(index)
    }
}

/// Two independent original phase owners (0054f228/00931ab8 and
/// 0054f230/00931ac0). The existing fallback owner remains separate.
#[derive(Clone, Copy, Debug)]
pub struct Phases {
    pub first: WaterPhase,
    pub quality: WaterPhase,
}

impl Phases {
    pub fn fresh() -> Self {
        Self {
            first: WaterPhase {
                previous_tick: u32::MAX,
                phase: 0.,
            },
            quality: WaterPhase {
                previous_tick: u32::MAX,
                phase: 0.,
            },
        }
    }

    /// Prepare both original models for the explicitly chosen route. Fallback
    /// carries no new batches: the existing mode20 owner must handle that route.
    /// This has no renderer/settings caller and changes no live default.
    pub fn prepare<'a>(
        &mut self,
        facts: RouteFacts<'a>,
        models: [&[[f32; 3]]; 2],
        camera: [f32; 3],
        sun: [f32; 3],
        tick: u32,
    ) -> Result<Prepared<'a>, &'static str> {
        let route = facts.route();
        let mut pending = *self;
        let mut batches = Vec::new();
        if route != Route::Fallback {
            for pass in [Pass::First, Pass::Quality] {
                if pass == Pass::Quality && route != Route::FirstAndQuality {
                    break;
                }
                let phase = match pass {
                    Pass::First => &mut pending.first,
                    Pass::Quality => &mut pending.quality,
                };
                let vertices = [
                    vertices(phase, pass, models[0], camera, sun, tick)?,
                    vertices(phase, pass, models[1], camera, sun, tick)?,
                ];
                batches.push(Batch {
                    material: Material::original(pass),
                    vertices,
                });
            }
        }
        *self = pending;
        Ok(Prepared { facts, batches })
    }
}

pub struct Prepared<'a> {
    pub facts: RouteFacts<'a>,
    /// Original first pair followed by optional quality pair; no mode9 base.
    pub batches: Vec<Batch>,
}

pub struct Batch {
    pub material: Material,
    pub vertices: [Vec<WaterVertex>; 2],
}

fn normalized(vector: [f32; 3]) -> Result<[f32; 3], &'static str> {
    let [x, y, z] = vector.map(Sum::from_f32);
    let [x, y, z] = [x?, y?, z?];
    let mut length = x;
    length.mul(x);
    let mut square = y;
    square.mul(y);
    length.add(square);
    square = z;
    square.mul(z);
    length.add(square);
    let mut inverse = Sum::from_f32(1.)?;
    inverse.div(length.sqrt()?)?;
    Ok([x, y, z].map(|mut value| {
        value.mul(inverse);
        value.to_f32()
    }))
}

fn relative(position: [f32; 3], camera: [f32; 3]) -> Result<[f32; 3], &'static str> {
    let mut x = Sum::from_f32(position[0])?;
    x.add_f32(-camera[0])?;
    let mut y = Sum::from_f32(position[1])?;
    y.add_f32(-camera[1])?;
    let mut z = Sum::from_f32(camera[2])?;
    z.add_f32(position[2])?;
    Ok([x.to_f32(), y.negated().to_f32(), z.to_f32()])
}

/// Finite nearest-even FISTP32; out-of-range produces the original indefinite.
fn integer(value: f32) -> i32 {
    let rounded = f64::from(value).round_ties_even();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) || !rounded.is_finite() {
        i32::MIN
    } else {
        rounded as i32
    }
}

fn sky_color(direction: [f32; 3], sun: [f32; 3]) -> Result<[i32; 3], &'static str> {
    let y = sun[1];
    let mut base = [0; 3];
    for (out, factor) in base.iter_mut().zip([183., 210., 270.]) {
        let mut value = Sum::from_f32(y)?;
        value.mul_f32(factor)?;
        *out = integer(value.to_f32());
    }
    let mut stretched = direction;
    let mut value = Sum::from_f32(direction[1])?;
    value.mul_f32(20.)?;
    stretched[1] = value.to_f32();
    let n = normalized(stretched)?;
    let mut dot = Sum::from_f32(sun[2])?;
    dot.mul_f32(n[2])?;
    value = Sum::from_f32(sun[1])?;
    value.mul_f32(n[1])?;
    dot.add(value);
    value = Sum::from_f32(sun[0])?;
    value.mul_f32(n[0])?;
    dot.add(value);
    value = Sum::from_f32(1.)?;
    value.add_f32(if y < 0. { y } else { -y })?;
    for _ in 0..if y < 0. { 4 } else { 2 } {
        dot.mul_f32(value.to_f32())?;
    }
    let light = dot.to_f32();
    for i in 0..2 {
        value = Sum::from_f32(light)?;
        value.mul_f32(if i == 0 { 256. } else { 64. })?;
        base[i] = base[i].wrapping_add(integer(value.to_f32()));
    }
    Ok(base.map(|v| v.clamp(0, 255)))
}

fn advance(state: &mut WaterPhase, tick: u32) -> Result<Sum, &'static str> {
    let previous = if state.previous_tick == u32::MAX {
        tick
    } else {
        state.previous_tick
    };
    let mut phase = Sum::from_f64(f64::from(tick.wrapping_sub(previous)))?;
    phase.mul_f32(f32::from_bits(0x3983126f))?;
    phase.add_f32(state.phase)?;
    let one = Sum::from_f32(1.)?;
    while phase.compare(one) == Ordering::Greater {
        phase.add(one.negated());
    }
    state.phase = phase.to_f32();
    state.previous_tick = tick;
    Ok(phase)
}

/// 004c4cd0/004c5140 finite PC64 preparation. Outputs retain original mesh
/// coordinates; the eventual renderer owns world projection/texture binding.
/// Empty batches still advance the corresponding original phase owner.
pub fn vertices(
    state: &mut WaterPhase,
    pass: Pass,
    positions: &[[f32; 3]],
    camera: [f32; 3],
    sun: [f32; 3],
    tick: u32,
) -> Result<Vec<WaterVertex>, &'static str> {
    for value in camera
        .iter()
        .chain(sun.iter())
        .chain(positions.iter().flatten())
    {
        if !value.is_finite() {
            return Err("nonfinite original water input");
        }
    }
    let mut pending = *state;
    let mut phase = advance(&mut pending, tick)?;
    let mut output = Vec::with_capacity(positions.len());
    for &position in positions {
        let q = relative(position, camera)?;
        let (uv, secondary_uv) = match pass {
            Pass::First => {
                let mut x = Sum::from_f32(position[0])?;
                x.mul(Sum::from_f64(0.0091)?);
                let mut twice = phase;
                twice.add(phase);
                x.add(twice);
                let mut z = Sum::from_f32(position[2])?;
                z.mul(Sum::from_f64(0.0091)?);
                let mut secondary = Sum::from_f32(position[0])?;
                secondary.mul(Sum::from_f64(0.0091)?);
                twice = Sum::from_f32(pending.phase)?;
                twice.add(twice);
                secondary.add(twice.negated());
                let mut v = z;
                v.add(Sum::from_f64(f64::from(0.3f32))?);
                ([x.to_f32(), z.to_f32()], [secondary.to_f32(), v.to_f32()])
            }
            Pass::Quality => {
                let mut x = Sum::from_f32(q[0])?;
                x.mul(Sum::from_f64(0.31)?);
                let mut z = Sum::from_f32(q[2])?;
                z.mul(Sum::from_f64(-0.31)?);
                let stored_z = z.to_f64(); // native FST64 at004c5245
                let mut u = x;
                u.mul(Sum::from_f64(f64::from(0.05f32))?);
                u.add(phase);
                z.mul(Sum::from_f64(f64::from(0.05f32))?);
                let secondary = [u.to_f32(), z.to_f32()];
                x.mul(Sum::from_f64(f64::from(0.0022f32))?);
                let mut motion = Sum::from_f32(pending.phase)?;
                motion.mul_f32(3.)?;
                x.add(motion);
                z = Sum::from_f64(stored_z)?;
                z.mul(Sum::from_f64(f64::from(0.0022f32))?);
                ([x.to_f32(), z.to_f32()], secondary)
            }
        };
        let direction = normalized(q)?;
        let sky = sky_color(direction, sun)?;
        let color = match pass {
            Pass::First => {
                let rgb = [
                    (sky[0] + 38) * 286,
                    (sky[1] + 48) * 446,
                    (sky[2] + 43) * 366,
                ]
                .map(|v| (v >> 8).min(255) as u32);
                (rgb[0] << 16) | (rgb[1] << 8) | rgb[2]
            }
            Pass::Quality => {
                let mut alpha = Sum::from_f32(direction[1])?;
                alpha.mul_f32(127.)?;
                let weight = 255i32.wrapping_sub(integer(alpha.to_f32())).min(255);
                let rgb = sky.map(|v| v.wrapping_mul(weight) >> 8);
                0xff000000u32
                    .wrapping_add((rgb[0] as u32) << 16)
                    .wrapping_add((rgb[1] as u32) << 8)
                    .wrapping_add(rgb[2] as u32)
            }
        };
        output.push(WaterVertex {
            uv,
            secondary_uv,
            color,
        });
        phase = Sum::from_f32(pending.phase)?; // native reload before later vertices
    }
    *state = pending;
    Ok(output)
}
