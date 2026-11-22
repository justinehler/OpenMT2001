//! Building profile preparation before facade tessellation.
//! Native references: 00484390 and 00484460 in the verified v1.4 executable.
use crate::build_file::BuildingLayer;

#[derive(Debug, Clone)]
pub struct FacadePoint {
    pub position: [f32; 2],
    /// -1 suppresses an edge absorbed into a lower layer.
    pub edge: i32,
    pub group: u32,
    pub u: f32,
    pub repetitions: f32,
}

#[derive(Debug)]
pub struct PreparedLayer {
    pub flags: u8,
    pub base: f32,
    pub height: f32,
    pub kind: u8,
    pub repetitions: u8,
    pub parameters: [f32; 2],
    pub surface: u8,
    pub points: Vec<FacadePoint>,
}

/// Original 00485dee..00485f8d auxiliary entrance list, in layer-zero order.
pub fn entrance_positions(layers: &[PreparedLayer]) -> Result<Vec<[f32; 3]>, String> {
    let points = &layers
        .first()
        .ok_or("entrance profile has no layers")?
        .points;
    let first = points.first().ok_or("entrance profile has no points")?;
    let mut entrances = Vec::new();
    for edge in points.windows(2) {
        if matches!(edge[0].edge, 10 | 11) {
            entrances.push([
                ((f64::from(edge[0].position[0]) + f64::from(edge[1].position[0])) * 5.) as f32,
                6.,
                ((f64::from(edge[0].position[1]) + f64::from(edge[1].position[1])) * 5.) as f32,
            ]);
            if entrances.len() == 8 {
                break;
            }
        }
    }
    if entrances.is_empty() {
        let mut min_x = (f64::from(first.position[0]) * 10.) as f32;
        let mut max_x = min_x;
        // The initial Z maximum remains on x87; later replacements reload the
        // per-point f32 temporary. X extrema and Z minimum are stored floats.
        let mut max_z = f64::from(first.position[1]) * 10.;
        let mut min_z = max_z as f32;
        for point in points.iter().skip(1) {
            let x = f64::from(point.position[0]) * 10.;
            let z = (f64::from(point.position[1]) * 10.) as f32;
            if x < f64::from(min_x) {
                min_x = x as f32;
            }
            if x > f64::from(max_x) {
                max_x = x as f32;
            }
            if z < min_z {
                min_z = z;
            }
            if f64::from(z) > max_z {
                max_z = f64::from(z);
            }
        }
        entrances.push([
            ((f64::from(min_x) + f64::from(max_x)) * 0.5) as f32,
            6.,
            ((f64::from(min_z) + max_z) * 0.5) as f32,
        ]);
    }
    Ok(entrances)
}

/// Four positions per visible facade edge in original emission order.
/// Roofs, materials and decorations are not part of this mesh stage.
pub fn facade_positions(layers: &[PreparedLayer]) -> Vec<[f32; 3]> {
    facade_vertices(layers)
        .into_iter()
        .map(|v| v.position)
        .collect()
}

#[derive(Debug)]
pub struct FacadeVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    /// Original secondary coordinates for the day/night facade texture.
    pub secondary_uv: [f32; 2],
    /// Offset from the selected facade material base (native group*16+edge).
    pub material_offset: u32,
}

pub fn facade_vertices(layers: &[PreparedLayer]) -> Vec<FacadeVertex> {
    facade_vertices_with_seed(layers, 0)
}

/// `random_state` is the state after material selection (00488050), before
/// 00485900 advances it once for each eligible layer, even an empty layer.
pub fn facade_vertices_with_seed(
    layers: &[PreparedLayer],
    mut random_state: u32,
) -> Vec<FacadeVertex> {
    let mut vertices = Vec::new();
    for (layer_index, layer) in layers.iter().enumerate() {
        if layer.flags != 0 || layer.kind & 0x60 != 0 {
            continue;
        }
        random_state = random_state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        let secondary_offset = (((random_state >> 16) & 0x7fff) % 9) as f32 * 0.125;
        for edge in layer.points.windows(2) {
            if edge[0].edge == -1 {
                continue;
            }
            // 00485a97 stores the per-floor height to f32 before the later
            // x87 multiply/add that emits the final vertex height.
            let floor_height = layer.height / f32::from(layer.repetitions);
            let top = (f64::from(floor_height) * f64::from(edge[0].repetitions)
                + f64::from(layer.base)) as f32;
            for point in edge {
                let x = point.position[0] * 10.;
                let z = point.position[1] * 10.;
                let u = f32::from(layer.surface) * point.u;
                let v = layer_index as f32 + 1.;
                let secondary_u = (f64::from(u) * 0.125 + f64::from(secondary_offset)) as f32;
                let secondary_v = v * 0.125;
                let material_offset = edge[0].group * 16 + edge[0].edge as u32;
                vertices.push(FacadeVertex {
                    position: [x, layer.base, z],
                    uv: [u, v],
                    secondary_uv: [secondary_u, secondary_v],
                    material_offset,
                });
                vertices.push(FacadeVertex {
                    position: [x, top, z],
                    uv: [u, v - edge[0].repetitions],
                    secondary_uv: [
                        secondary_u,
                        (f64::from(edge[0].repetitions) * 0.125 + f64::from(secondary_v)) as f32,
                    ],
                    material_offset,
                });
            }
        }
    }
    vertices
}

pub fn prepare(layers: &[BuildingLayer]) -> Vec<PreparedLayer> {
    let mut prepared: Vec<_> = layers
        .iter()
        .map(|layer| {
            let mut points: Vec<_> = layer
                .outline
                .iter()
                .enumerate()
                .map(|(i, p)| FacadePoint {
                    position: p.position,
                    edge: i32::from(p.attributes[0]),
                    group: 0,
                    u: i as f32,
                    repetitions: f32::from(layer.repetitions),
                })
                .collect();
            let mut i = 1;
            while i + 1 < points.len() {
                if redundant(&points[i - 1], &points[i], &points[i + 1]) {
                    points.remove(i);
                } else {
                    i += 1;
                }
            }
            PreparedLayer {
                flags: layer.flags & 1,
                base: layer.base,
                height: layer.height,
                kind: layer.kind,
                repetitions: layer.repetitions,
                parameters: layer.parameters.map(f32::from_bits),
                surface: layer.surface,
                points,
            }
        })
        .collect();
    for layer_index in 0..prepared.len() {
        if prepared[layer_index].flags != 0 {
            continue;
        }
        for point_index in 0..prepared[layer_index].points.len().saturating_sub(1) {
            let point = prepared[layer_index].points[point_index].clone();
            let end = prepared[layer_index].points[point_index + 1].position;
            let mut repetitions = f64::from(prepared[layer_index].repetitions);
            let unit_height = f64::from(prepared[layer_index].height) / repetitions;
            let mut top = unit_height * repetitions + f64::from(prepared[layer_index].base);
            for later in prepared.iter_mut().skip(layer_index + 1) {
                if later.flags != 0
                    || top != f64::from(later.base)
                    || f64::from(later.height) / f64::from(later.repetitions) != unit_height
                {
                    continue;
                }
                for j in 0..later.points.len().saturating_sub(1) {
                    if point.edge != -1
                        && point.position == later.points[j].position
                        && point.edge == later.points[j].edge
                        && end == later.points[j + 1].position
                    {
                        later.points[j].edge = -1;
                        top = f64::from(later.height) + f64::from(later.base);
                        repetitions += f64::from(later.repetitions);
                    }
                }
            }
            prepared[layer_index].points[point_index].repetitions = repetitions as f32;
        }
    }
    prepared
}

fn redundant(previous: &FacadePoint, current: &FacadePoint, next: &FacadePoint) -> bool {
    if previous.edge != current.edge || previous.group != current.group {
        return false;
    }
    let a = f64::from(current.u) - f64::from(previous.u);
    let b = f64::from(next.u) - f64::from(current.u);
    (0..2).all(|axis| {
        let difference = ((f64::from(current.position[axis]) - f64::from(previous.position[axis]))
            / a
            - (f64::from(next.position[axis]) - f64::from(current.position[axis])) / b)
            .abs();
        // Native FCOMP/Jcc accepts the unordered flag as well as less-than.
        difference < f64::from(0.01f32) || difference.is_nan()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original BuildFile and native facade UV observations"]
    fn matches_native_facade_uvs() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let data = read("OPENMT2001_BUILD_FILE");
        let native = read("OPENMT2001_NATIVE_FACADE_UVS");
        let file = crate::build_file::BuildFile::parse(&data).unwrap();
        assert_eq!(&native[..8], b"MTFUV002");
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        let seed = word(12);
        let mut at = 16;
        for _ in 0..word(8) {
            let index = word(at) as usize;
            let height = word(at + 4) as u8;
            let count = word(at + 8) as usize;
            at += 12;
            let vertices =
                facade_vertices_with_seed(&prepare(&file.profile(index, height).unwrap()), seed);
            assert_eq!(vertices.len(), count);
            for (i, v) in vertices.iter().enumerate() {
                assert_eq!(
                    v.uv.map(f32::to_bits),
                    [word(at + i * 8), word(at + i * 8 + 4)],
                    "model {index}, height {height}, vertex {i}"
                );
                assert_eq!(
                    v.secondary_uv.map(f32::to_bits),
                    [
                        word(at + count * 8 + i * 8),
                        word(at + count * 8 + i * 8 + 4)
                    ],
                    "secondary model {index}, height {height}, vertex {i}"
                );
            }
            at += count * 16;
        }
        assert_eq!(at, native.len());
    }
    #[test]
    #[ignore = "requires private BuildFile and native facade vertex observations"]
    fn matches_native_facade_vertices() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let data = read("OPENMT2001_BUILD_FILE");
        let native = read("OPENMT2001_NATIVE_FACADES");
        let file = crate::build_file::BuildFile::parse(&data).unwrap();
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTFACADE");
        let mut at = 12;
        for case in 0..word(8) {
            let index = word(at) as usize;
            let height = word(at + 4) as u8;
            let count = word(at + 8) as usize;
            at += 12;
            let vertices = facade_positions(&prepare(&file.profile(index, height).unwrap()));
            assert_eq!(vertices.len(), count, "case {case} vertex count");
            for (v, position) in vertices.iter().enumerate() {
                assert_eq!(
                    position.map(f32::to_bits),
                    [
                        word(at + v * 12),
                        word(at + v * 12 + 4),
                        word(at + v * 12 + 8)
                    ],
                    "case {case} vertex {v}"
                );
            }
            at += count * 12;
        }
        assert_eq!(at, native.len());
    }
    #[test]
    #[ignore = "requires private BuildFile and original-instruction prepared profiles"]
    fn matches_native_preparation() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let data = read("OPENMT2001_BUILD_FILE");
        let native = read("OPENMT2001_NATIVE_PREPARED");
        let file = crate::build_file::BuildFile::parse(&data).unwrap();
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(&native[..8], b"MTPREPAR");
        let mut at = 12;
        for case in 0..word(8) {
            let index = word(at) as usize;
            let height = word(at + 4) as u8;
            at += 8;
            let layers = prepare(&file.profile(index, height).unwrap());
            assert_eq!(layers.len(), word(at + 24) as usize);
            for (i, layer) in layers.iter().enumerate() {
                let start = at + 28 + i * 0x122c;
                assert_eq!(
                    layer.points.len(),
                    word(start + 40) as usize,
                    "case {case} layer {i} point count"
                );
                for (j, point) in layer.points.iter().enumerate() {
                    let p = start + 44 + j * 36;
                    assert_eq!(
                        point.position.map(f32::to_bits),
                        [word(p), word(p + 4)],
                        "case {case} layer {i} point {j}"
                    );
                    assert_eq!(
                        point.edge as u32,
                        word(p + 8),
                        "case {case} layer {i} point {j} edge"
                    );
                    assert_eq!(point.group, word(p + 12));
                    assert_eq!(point.u.to_bits(), word(p + 16));
                    assert_eq!(
                        point.repetitions.to_bits(),
                        word(p + 28),
                        "case {case} layer {i} point {j} repetitions"
                    );
                }
            }
            at += 0x1ca80;
        }
        assert_eq!(at, native.len());
    }
}
