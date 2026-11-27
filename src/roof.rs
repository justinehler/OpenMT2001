//! Roof vertex emission recovered from v1.4 004861a1..0048759b.
//! Material grouping, ground rectangles and decorative meshes are separate.
use crate::geometry::PreparedLayer;

fn ring(out: &mut Vec<[f32; 3]>, points: &[[f32; 2]], y: f32, copies: usize) {
    for &[x, z] in points {
        out.extend(std::iter::repeat_n([x * 10., y, z * 10.], copies));
    }
}

fn ring_uv(out: &mut Vec<[f32; 2]>, points: &[[f32; 2]], scale: f32, copies: usize) {
    for &[x, z] in points {
        let position = [(f64::from(x) * 10.) as f32, (f64::from(z) * 10.) as f32];
        let uv = [
            (f64::from(position[0]) * f64::from(scale)) as f32,
            (f64::from(position[1]) * f64::from(scale)) as f32,
        ];
        out.extend(std::iter::repeat_n(uv, copies));
    }
}

#[derive(Debug)]
pub struct RoofMesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub pitched_triangle_start: usize,
}

fn quad(triangles: &mut Vec<[u32; 3]>, a: u32, b: u32, c: u32, d: u32) {
    triangles.extend([[a, b, c], [a, c, d]]);
}
fn cap(triangles: &mut Vec<[u32; 3]>, polygon: &[[u16; 3]], start: u32) {
    triangles.extend(polygon.iter().map(|&[a, b, c]| {
        [
            start + u32::from(a),
            start + u32::from(c),
            start + u32::from(b),
        ]
    }));
}

/// Native vertex and triangle order for equal roof material IDs, including
/// duplicated positions used for separate face attributes.
pub fn mesh(layers: &[PreparedLayer]) -> RoofMesh {
    let mut out = Vec::new();
    let mut uvs = Vec::new();
    let mut triangles = Vec::new();
    for (i, layer) in layers.iter().enumerate() {
        let points: Vec<_> = layer.points.iter().map(|p| p.position).collect();
        let identical_next = layers.get(i + 1).is_some_and(|next| {
            next.points.len() == points.len()
                && next
                    .points
                    .iter()
                    .zip(&points)
                    .all(|(a, b)| a.position == *b)
        });
        if identical_next {
            continue;
        }
        let top = (f64::from(layer.base) + f64::from(layer.height)) as f32;
        let start = out.len() as u32;
        let n = points.len() as u32;
        // Assignment to kind in each branch clears the lower-priority bits.
        if layer.kind & 1 != 0 {
            ring(&mut out, &points, top, 1);
            ring_uv(&mut uvs, &points, 0.2, 1);
            cap(&mut triangles, &crate::polygon::triangulate(&points), start);
        } else if layer.kind & 2 != 0 {
            ring(&mut out, &points, top, 2);
            ring_uv(&mut uvs, &points, 0.5, 2);
            let inset = inset(&points, layer.parameters[1] * 0.1);
            let bottom = (f64::from(layer.base) + f64::from(layer.height)
                - f64::from(layer.parameters[0])) as f32;
            ring(&mut out, &inset, bottom, 2);
            ring(&mut out, &inset, bottom, 1);
            ring_uv(&mut uvs, &inset, 0.5, 2);
            ring_uv(&mut uvs, &inset, 0.5, 1);
            let polygon = crate::polygon::triangulate(&inset);
            if !polygon.is_empty() {
                for i in 0..n - 1 {
                    quad(
                        &mut triangles,
                        start + i * 2 + 2,
                        start + i * 2 + 1,
                        start + n * 2 + i * 2 + 1,
                        start + n * 2 + i * 2 + 2,
                    );
                }
                cap(&mut triangles, &polygon, start + n * 4);
            }
        } else if layer.kind & 4 != 0 {
            let inset = inset(&points, layer.parameters[1] * 0.1);
            let polygon = crate::polygon::triangulate(&inset);
            if polygon.is_empty() {
                continue;
            }
            let bottom = (f64::from(layer.base) + f64::from(layer.height)
                - f64::from(layer.parameters[0])) as f32;
            ring(&mut out, &points, top, 1);
            ring(&mut out, &inset, top, 1);
            ring(&mut out, &inset, top, 2);
            ring(&mut out, &inset, bottom, 2);
            ring(&mut out, &inset, bottom, 1);
            ring_uv(&mut uvs, &points, 0.5, 1);
            ring_uv(&mut uvs, &inset, 0.5, 1);
            for point in &layer.points {
                uvs.extend([[point.u, 1.]; 2]);
            }
            for point in &layer.points {
                uvs.extend([[point.u, 0.]; 2]);
            }
            ring_uv(&mut uvs, &inset, 0.5, 1);
            for i in 0..n - 1 {
                quad(
                    &mut triangles,
                    start + i + 1,
                    start + i,
                    start + n + i,
                    start + n + i + 1,
                );
            }
            for i in 0..n - 1 {
                quad(
                    &mut triangles,
                    start + n * 2 + i * 2 + 2,
                    start + n * 2 + i * 2 + 1,
                    start + n * 4 + i * 2 + 1,
                    start + n * 4 + i * 2 + 2,
                );
            }
            cap(&mut triangles, &polygon, start + n * 6);
        }
    }
    let pitched_triangle_start = triangles.len();
    for layer in layers {
        // Earlier kind assignments clear pitched bits in these cases.
        if layer.kind & 7 != 0 || layer.kind & 0x60 == 0 || layer.points.len() != 5 {
            continue;
        }
        let points: Vec<_> = layer
            .points
            .iter()
            .map(|p| [p.position[0] * 10., p.position[1] * 10.])
            .collect();
        let mut selected = 0;
        let longest = layer.kind & 0x20 != 0;
        let mut best = if longest { 0. } else { 1e10f32 };
        for i in 0..4 {
            let dx = f64::from(points[i][0]) - f64::from(points[i + 1][0]);
            let dz = f64::from(points[i][1]) - f64::from(points[i + 1][1]);
            let length = (dx * dx + dz * dz) as f32;
            if if longest {
                length > best
            } else {
                length < best
            } {
                best = length;
                selected = i;
            }
        }
        let start = out.len();
        for i in selected..selected + 4 {
            let [x, z] = points[i % 5];
            out.extend([[x, layer.base, z]; 2]);
            let u = (f64::from(layer.surface) * f64::from(layer.points[i % 5].u)) as f32;
            uvs.extend([[u, 1.]; 2]);
        }
        let a = out[start];
        let b = out[start + 6];
        let c = out[start + 2];
        let d = out[start + 4];
        let midpoint = |p: [f32; 3], q: [f32; 3]| {
            [
                ((f64::from(p[0]) + f64::from(q[0])) * 0.5) as f32,
                ((f64::from(p[1]) + f64::from(q[1])) * 0.5 + f64::from(layer.height)) as f32,
                ((f64::from(p[2]) + f64::from(q[2])) * 0.5) as f32,
            ]
        };
        let a = midpoint(a, b);
        let b = midpoint(c, d);
        let delta = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let length = delta
            .iter()
            .map(|&v| f64::from(v) * f64::from(v))
            .sum::<f64>()
            .sqrt();
        let direction = delta.map(|v| (f64::from(v) / length) as f32);
        let ridge_a = [
            (f64::from(direction[0]) * f64::from(layer.parameters[1]) + f64::from(a[0])) as f32,
            a[1],
            (f64::from(direction[2]) * f64::from(layer.parameters[1]) + f64::from(a[2])) as f32,
        ];
        let ridge_b = [
            (f64::from(b[0]) - f64::from(direction[0]) * f64::from(layer.parameters[0])) as f32,
            b[1],
            (f64::from(b[2]) - f64::from(direction[2]) * f64::from(layer.parameters[0])) as f32,
        ];
        let edge_u = |i: usize| {
            f64::from(
                (f64::from(layer.surface) * f64::from(layer.points[(selected + i) % 5].u)) as f32,
            )
        };
        let point_two_tenths = |value: f32| f64::from(value) * f64::from(0.2f32);
        let ridge_a_u0 = (edge_u(0) + point_two_tenths(layer.parameters[1])) as f32;
        let ridge_a_u1 = (edge_u(3) - point_two_tenths(layer.parameters[1])) as f32;
        let ridge_b_u0 = (edge_u(1) - point_two_tenths(layer.parameters[0])) as f32;
        let ridge_b_u1 = (edge_u(2) + point_two_tenths(layer.parameters[0])) as f32;
        let ridge_v = |u: f32| [u, (f64::from(layer.height) * f64::from(-0.2f32)) as f32];
        let ridge_a_uv = [
            ridge_v(ridge_a_u0),
            ridge_v(ridge_a_u1),
            ridge_v(((edge_u(0) + edge_u(3)) * f64::from(0.5f32)) as f32),
        ];
        let ridge_b_uv = [
            ridge_v(ridge_b_u0),
            ridge_v(ridge_b_u1),
            ridge_v(((edge_u(1) + edge_u(2)) * f64::from(0.5f32)) as f32),
        ];
        out.extend([ridge_a; 3]);
        out.extend([ridge_b; 3]);
        uvs.extend(ridge_a_uv);
        uvs.extend(ridge_b_uv);
        for i in [0, 2, 4, 6] {
            out.push(out[start + i]);
        }
        for i in 0..4 {
            let uv = i as f32 * 0.01;
            uvs.push([uv, uv]);
        }
        let s = start as u32;
        quad(&mut triangles, s + 2, s + 1, s + 8, s + 11);
        quad(&mut triangles, s + 6, s + 5, s + 12, s + 9);
        triangles.extend([[s + 10, s, s + 7], [s + 4, s + 3, s + 13]]);
        quad(&mut triangles, s + 14, s + 15, s + 16, s + 17);
    }
    RoofMesh {
        positions: out,
        uvs,
        triangles,
        pitched_triangle_start,
    }
}

/// Original offset construction (004841f0), retaining each float store.
pub fn inset(points: &[[f32; 2]], distance: f32) -> Vec<[f32; 2]> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let n = points.len() - 1;
    let mut result = vec![[0.; 2]; points.len()];
    for i in 1..=n {
        let current = points[i % n];
        let forward = normalize([
            points[(i + 1) % n][0] - current[0],
            points[(i + 1) % n][1] - current[1],
        ]);
        let backward = normalize([
            points[(i - 1) % n][0] - current[0],
            points[(i - 1) % n][1] - current[1],
        ]);
        let normal = normalize([-(forward[1] - backward[1]), forward[0] - backward[0]]);
        let dot = f64::from(forward[0]) * f64::from(backward[0])
            + f64::from(forward[1]) * f64::from(backward[1]);
        let sine = (dot.acos() * 0.5).sin();
        // Native FCOM tests unordered as well as less-than: NaN uses the cap.
        let scale = if sine >= f64::from(0.1f32) {
            1. / sine
        } else {
            10.
        };
        let normal = normal.map(|v| (f64::from(v) * scale) as f32);
        result[i % n] = [0, 1].map(|axis| {
            (f64::from(normal[axis]) * f64::from(distance) + f64::from(current[axis])) as f32
        });
    }
    result[n] = result[0];
    result
}

fn normalize(v: [f32; 2]) -> [f32; 2] {
    let length = (f64::from(v[0]) * f64::from(v[0]) + f64::from(v[1]) * f64::from(v[1])).sqrt();
    let scale = if length > f64::from(0.001f32) {
        1. / length
    } else {
        f64::from(999.99994f32)
    };
    v.map(|value| (f64::from(value) * scale) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires private original BuildFile and native roof observations"]
    fn matches_native_roof_positions() {
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let data = read("OPENMT2001_BUILD_FILE");
        for (fixture, surface) in [
            ("OPENMT2001_NATIVE_ROOFS", None),
            ("OPENMT2001_NATIVE_ROOFS_SURFACE3", Some(3)),
        ] {
            let native = read(fixture);
            let file = crate::build_file::BuildFile::parse(&data).unwrap();
            let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
            assert_eq!(&native[..8], b"MTROOFS2");
            let mut at = 12;
            for _ in 0..word(8) {
                let index = word(at) as usize;
                let height = word(at + 4) as u8;
                let count = word(at + 8) as usize;
                at += 12;
                let mut profile = file.profile(index, height).unwrap();
                if let Some(surface) = surface {
                    for layer in &mut profile {
                        layer.surface = surface;
                    }
                }
                let mesh = mesh(&crate::geometry::prepare(&profile));
                let vertices = &mesh.positions;
                assert_eq!(
                    vertices.len(),
                    count,
                    "model {index}, height {height} count"
                );
                for (v, p) in vertices.iter().enumerate() {
                    let expected = [
                        word(at + v * 12),
                        word(at + v * 12 + 4),
                        word(at + v * 12 + 8),
                    ];
                    assert_eq!(
                        p.map(f32::to_bits),
                        expected,
                        "model {index}, height {height}, vertex {v}, ours={p:?}, native={:?}",
                        expected.map(f32::from_bits)
                    );
                }
                at += count * 12;
                for (v, uv) in mesh.uvs.iter().enumerate() {
                    let expected = [word(at + v * 8), word(at + v * 8 + 4)];
                    assert_eq!(
                        uv.map(f32::to_bits),
                        expected,
                        "model {index}, height {height}, uv {v}, ours={uv:?}, native={:?}",
                        expected.map(f32::from_bits)
                    );
                }
                assert_eq!(mesh.uvs.len(), count, "model {index}, {height} UV count");
                at += count * 8;
                let count = word(at) as usize;
                at += 4;
                let triangles: Vec<_> = native[at..at + count * 6]
                    .chunks_exact(6)
                    .map(|t| {
                        [
                            u16::from_le_bytes(t[0..2].try_into().unwrap()) as u32,
                            u16::from_le_bytes(t[2..4].try_into().unwrap()) as u32,
                            u16::from_le_bytes(t[4..6].try_into().unwrap()) as u32,
                        ]
                    })
                    .collect();
                assert_eq!(
                    mesh.triangles, triangles,
                    "model {index}, height {height}, triangles"
                );
                at += count * 6;
                let flat = word(at) as usize;
                assert_eq!(
                    mesh.pitched_triangle_start, flat,
                    "model {index}, height {height}, pitched boundary"
                );
                at += 4;
            }
            assert_eq!(at, native.len());
        }
    }
}
