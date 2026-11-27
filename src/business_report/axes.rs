//! Original42d220 grid/label submissions for the4066e0 business graph.
use super::{Graph, Sum};

#[derive(Clone, Debug, PartialEq)]
pub enum AxisItem {
    Rectangle {
        bounds: [f32; 4],
        color: u32,
    },
    ///42ce80 arguments: mode0 is decimal with format0 (no decimals); mode1
    ///uses43a8c0 integer currency after FTOL. Flags8=center below,2=left of axis.
    Label {
        position: [f32; 2],
        value: f32,
        format: u32,
        mode: u32,
        flags: u32,
    },
}
fn sum(a: f32, b: f32) -> Result<Sum, String> {
    let mut s = Sum::from_f32(a)?;
    s.add_f32(b)?;
    Ok(s)
}
fn spacing(
    span: f32,
    pixels: f32,
    desired: f32,
    major: i32,
    vertical: bool,
) -> Result<f32, String> {
    let mut denominator = Sum::from_f32(pixels)?;
    denominator.div_f32(desired)?;
    let mut nominal = Sum::from_f32(span)?;
    nominal.div(denominator)?;
    let mut raw = Sum::from_i32(major);
    raw.mul_f32(nominal.to_f32())?;
    raw.mul_f32(100.)?;
    let mut raw = Sum::from_f32(raw.to_f32())?;
    let mut scale = Sum::from_f32(0.01)?;
    for _ in 0..100 {
        if !raw.compare(Sum::from_i32(100)).is_gt() {
            break;
        }
        raw.mul_f32(0.1)?;
        scale.mul_f32(10.)?;
    }
    let below = |v| raw.compare(Sum::from_i32(v)).is_lt();
    let bucket = if below(8) {
        5.
    } else if below(17) {
        10.
    } else if vertical && below(40) {
        20.
    } else if below(75) {
        50.
    } else {
        100.
    };
    scale.mul_f32(bucket)?;
    scale.div(Sum::from_i32(major))?;
    let step = scale.to_f32();
    if !step.is_finite() || step <= 0. {
        return Err("graph axis requires a positive finite tick interval".into());
    }
    Ok(step)
}
fn first_tick(min: f32, step: f32) -> Result<f32, String> {
    let mut quotient = Sum::from_f32(min)?;
    quotient.div_f32(step)?;
    let mut first = Sum::from_i32(quotient.ftol_low_word());
    first.mul_f32(step)?;
    let value = first.to_f32();
    if first.compare(Sum::from_f32(min)?).is_lt()
        && sum(value, -min)?.to_f64().abs() > 0.0010000000474974513
    {
        Ok(sum(value, step)?.to_f32())
    } else {
        Ok(value)
    }
}
fn tick_number(value: f32, step: f32) -> Result<i32, String> {
    let mut quotient = Sum::from_f32(value)?;
    quotient.div_f32(step)?;
    let rounded = quotient.to_f32().round_ties_even();
    Ok(if (-2147483648.0..2147483648.0).contains(&rounded) {
        rounded as i32
    } else {
        i32::MIN
    })
}
impl Graph {
    /// Finite native grid placement. Font metrics/alignment and glyph drawing
    /// remain the host's responsibility; label values retain the original bump
    /// by1.000000119 before formatting/truncation.
    pub fn axes(&self) -> Result<Vec<AxisItem>, String> {
        let mut out = Vec::new();
        if self.domain[2] == 0. || self.domain[3] == 0. {
            return Ok(out);
        }
        let [left, top, width, height] = self.plot;
        let [xmin, ymin, xspan, yspan] = self.domain;
        let xmax = sum(xmin, xspan)?.to_f32();
        let ymax = sum(ymin, yspan)?.to_f32();
        let mut sx = Sum::from_f32(width)?;
        sx.div_f32(xspan)?;
        let sx = sx.to_f32();
        let mut sy = Sum::from_f32(height)?;
        sy.div_f32(yspan)?;
        let sy = sy.to_f32();
        let bottom = sum(top, height)?;
        let right = sum(left, width)?.to_f32();
        for vertical in [false, true] {
            let (min, max, span, pixels, desired, major, thick) = if vertical {
                (ymin, ymax, yspan, height, 32., 1, 5)
            } else {
                (xmin, xmax, xspan, width, 17., 2, 2)
            };
            let step = spacing(span, pixels, desired, major, vertical)?;
            let mut tick = first_tick(min, step)?;
            for iteration in 0..1024 {
                if tick >= max {
                    break;
                }
                if iteration == 1023 {
                    return Err("graph axis exceeds original panel's practical tick range".into());
                }
                let index = tick_number(tick, step)?;
                let mut offset = sum(tick, -min)?;
                offset.mul_f32(if vertical { sy } else { sx })?;
                let coordinate = if vertical {
                    let mut y = bottom;
                    y.add(offset.negated());
                    y
                } else {
                    offset.add_f32(left)?;
                    offset
                };
                let mut end = coordinate;
                end.add_f32(1.)?;
                let mut end = end.to_f32();
                if index % thick == 0 {
                    end = sum(end, 1.)?.to_f32();
                }
                if index % major == 0 {
                    let mut value = Sum::from_f32(tick)?;
                    value.mul_f32(1.0000001)?;
                    out.push(AxisItem::Label {
                        position: if vertical {
                            [left, coordinate.to_f32()]
                        } else {
                            [coordinate.to_f32(), bottom.to_f32()]
                        },
                        value: value.to_f32(),
                        format: 0,
                        mode: u32::from(vertical),
                        flags: if vertical { 2 } else { 8 },
                    });
                }
                out.push(AxisItem::Rectangle {
                    bounds: if vertical {
                        [left, coordinate.to_f32(), right, end]
                    } else {
                        [coordinate.to_f32(), top, end, bottom.to_f32()]
                    },
                    color: 0xff003f00,
                });
                let next = sum(tick, step)?;
                let stored = next.to_f32();
                if stored == tick {
                    return Err("graph tick interval no longer advances at f32 precision".into());
                }
                tick = stored;
                if !next.compare(Sum::from_f32(max)?).is_lt() {
                    break;
                }
            }
        }
        let origin = self.points[0][0];
        if xmin <= origin && xmax >= origin {
            let mut x = sum(origin, -xmin)?;
            x.mul_f32(sx)?;
            x.add_f32(left)?;
            let mut l = x;
            l.add_f32(-1.)?;
            let mut r = x;
            r.add_f32(1.)?;
            out.push(AxisItem::Rectangle {
                bounds: [l.to_f32(), top, r.to_f32(), bottom.to_f32()],
                color: 0xff007f00,
            });
        }
        if ymin <= 0. && ymax >= 0. {
            let mut offset = Sum::from_f32(-ymin)?;
            offset.mul_f32(sy)?;
            let mut y = bottom;
            y.add(offset.negated());
            let mut t = y;
            t.add_f32(-1.)?;
            let mut b = y;
            b.add_f32(1.)?;
            out.push(AxisItem::Rectangle {
                bounds: [left, t.to_f32(), right, b.to_f32()],
                color: 0xff007f00,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original business graph axis drawing capture"]
    fn matches_native_axes() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_BUSINESS_GRAPH_AXES")).unwrap();
        let mut at = 0;
        fn word(b: &[u8], at: &mut usize) -> u32 {
            let v = u32::from_le_bytes(b[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        }
        fn values<const N: usize>(b: &[u8], at: &mut usize) -> [f32; N] {
            std::array::from_fn(|_| f32::from_bits(word(b, at)))
        }
        assert_eq!(&bytes[..8], b"MTBGAX01");
        at += 8;
        assert_eq!(word(&bytes, &mut at), 0x37f);
        for case in 0..word(&bytes, &mut at) {
            let graph = Graph {
                points: std::array::from_fn(|_| values(&bytes, &mut at)),
                rect: values(&bytes, &mut at),
                plot: values(&bytes, &mut at),
                domain: values(&bytes, &mut at),
            };
            let expected = (0..word(&bytes, &mut at))
                .map(|_| {
                    if word(&bytes, &mut at) == 0 {
                        let bounds = values(&bytes, &mut at);
                        let color = word(&bytes, &mut at);
                        assert_eq!(word(&bytes, &mut at), 0);
                        AxisItem::Rectangle { bounds, color }
                    } else {
                        AxisItem::Label {
                            position: values(&bytes, &mut at),
                            value: f32::from_bits(word(&bytes, &mut at)),
                            format: word(&bytes, &mut at),
                            mode: word(&bytes, &mut at),
                            flags: word(&bytes, &mut at),
                        }
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(graph.axes().unwrap(), expected, "case{case}");
        }
        assert_eq!(at, bytes.len());
    }
}
