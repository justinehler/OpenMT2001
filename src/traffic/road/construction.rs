//! Original44ab20 road graph construction,44a270 connectors and44a4a0 ramps.
//! Opaque reference values match the native tables; they are never host pointers.
use super::{Graph, Sum, cubic, difference, extended, float, put, word};

pub const SEGMENT_BASE: u32 = 0x661d70;
pub const NODE_BASE: u32 = 0x63a1c0;
pub const ROAD_BASE: u32 = 0x6375b8;
pub const LANE_BASE: u32 = 0x661370;

type Orientation = (u8, usize, usize, usize, [f32; 2], [f32; 2]);

pub struct ConstructionSettings {
    values: [f32; 13],
}
impl ConstructionSettings {
    pub fn from_v14(image: &crate::pe::PeImage<'_>) -> Result<Self, String> {
        let addresses = [
            0x525098, 0x52516c, 0x525034, 0x525044, 0x525b3c, 0x525958, 0x525110, 0x525030,
            0x5250a4, 0x525094, 0x525064, 0x5250a0, 0x525b40,
        ];
        let mut values = [0.; 13];
        for (value, address) in values.iter_mut().zip(addresses) {
            *value = image.f32_at(address)?;
        }
        Ok(Self { values })
    }
}

///44a4a0's gate presentation placement. Original auxiliary allocation/model
/// binding is separate from the road graph; the host can use these placements.
#[derive(Debug)]
pub struct GatePlacement {
    pub category: usize,
    pub entry: usize,
    pub position: [f32; 3],
}

pub struct RoadNetwork {
    pub roads: Vec<[u8; 44]>,
    pub road_segment_count: usize,
    /// Full native pools retain the preassigned indices in unused rows.
    pub segments: Vec<[u8; 108]>,
    pub nodes: Vec<[u8; 44]>,
    pub entries: Vec<[u8; 52]>,
    pub lanes: Vec<[u8; 32]>,
    pub primary_count: usize,
    pub connector_count: usize,
    pub builder_count: usize,
    pub node_count: usize,
    pub entry_count: usize,
    pub gates: Vec<GatePlacement>,
}
fn segment_ref(i: usize) -> u32 {
    SEGMENT_BASE + i as u32 * 108
}
fn node_ref(i: usize) -> u32 {
    NODE_BASE + i as u32 * 44
}
fn node_index(reference: u32) -> Result<usize, String> {
    let offset = reference.checked_sub(NODE_BASE).ok_or("node before pool")?;
    if !offset.is_multiple_of(44) || offset / 44 >= 512 {
        return Err("node outside pool".into());
    }
    Ok((offset / 44) as usize)
}
fn shifted(value: f32, delta: f32) -> Result<f32, String> {
    let mut result = extended(value)?;
    result.add_f32(delta)?;
    Ok(result.to_f32())
}
fn distance(a: [f32; 3], b: [f32; 3]) -> Result<Sum, String> {
    let mut total = extended(0.)?;
    for axis in 0..3 {
        let mut term = difference(a[axis], b[axis])?;
        term.mul(term);
        total.add(term);
    }
    Ok(total.sqrt()?)
}
fn point(row: &[u8], at: usize) -> [f32; 3] {
    std::array::from_fn(|i| float(row, at + i * 4))
}
fn set_point(row: &mut [u8], at: usize, values: [f32; 3]) {
    for (i, v) in values.into_iter().enumerate() {
        put(row, at + i * 4, v.to_bits());
    }
}
fn curve_length(row: &[u8; 108], one: f32) -> Result<f32, String> {
    let mut step = extended(one)?;
    step.div(extended(10.)?)?;
    let step = step.to_f32();
    let mut t = 0.;
    let mut total = 0.;
    let mut previous = cubic(row, 0.)?;
    for _ in 0..10 {
        t = shifted(t, step)?;
        let next = cubic(row, t)?;
        let mut length = distance(previous, next)?;
        length.add_f32(total)?;
        total = length.to_f32();
        previous = next;
    }
    Ok(total)
}
impl RoadNetwork {
    pub fn graph(&mut self) -> Graph<'_> {
        Graph {
            segments: &mut self.segments,
            segment_base: SEGMENT_BASE,
            nodes: &mut self.nodes,
            node_base: NODE_BASE,
        }
    }
    fn new_node(&mut self) -> Result<usize, String> {
        let index = self.node_count;
        if index >= self.nodes.len() {
            return Err("road graph node pool exhausted".into());
        }
        self.node_count += 1;
        Ok(index)
    }
    fn next_segment(&self) -> Result<usize, String> {
        let index = self.primary_count + self.connector_count + self.builder_count;
        if index >= self.segments.len() {
            return Err("road segment pool exhausted".into());
        }
        Ok(index)
    }
    fn finish_curve(&mut self, index: usize, truncate: bool, one: f32) -> Result<(), String> {
        let bits = curve_length(&self.segments[index], one)?.to_bits();
        let bits = if truncate { bits & 0xffffff00 } else { bits };
        put(&mut self.segments[index], 0x58, bits);
        put(&mut self.segments[index], 4, bits);
        let mut inverse = extended(one)?;
        inverse
            .div(extended(f32::from_bits(bits))?)
            .map_err(|e| format!("road vehicle orientation divisor{bits:08x}: {e}"))?;
        put(&mut self.segments[index], 0x5c, inverse.to_f32().to_bits());
        Ok(())
    }
    /// Full fresh startup graph. Road rows come from original city construction;
    /// category records retain all unrelated bytes and receive their lane refs.
    pub fn build(
        roads: &[[u8; 44]],
        road_segment_count: usize,
        categories: &mut [[u8; 380]],
        category_count: usize,
        settings: &ConstructionSettings,
    ) -> Result<Self, String> {
        if road_segment_count > roads.len() || category_count > categories.len() {
            return Err("inconsistent road/category counts".into());
        }
        let c = &settings.values;
        let mut result = Self {
            roads: roads.to_vec(),
            road_segment_count,
            segments: vec![[0; 108]; 700],
            nodes: vec![[0; 44]; 512],
            entries: vec![[0; 52]; 64],
            lanes: vec![[0; 32]; roads.len() - road_segment_count],
            primary_count: 0,
            connector_count: 0,
            builder_count: 0,
            node_count: 0,
            entry_count: 0,
            gates: vec![],
        };
        for (i, row) in result.segments.iter_mut().enumerate() {
            put(row, 0x14, i as u32);
        }
        for (i, row) in result.nodes.iter_mut().enumerate() {
            put(row, 0xc, i as u32);
        }
        for row in &mut result.lanes {
            put(row, 0, 5);
        }
        for (source, road) in roads.iter().enumerate() {
            let orientations: &[Orientation] = match word(road, 4) {
                1 | 3 => &[
                    (2, 0x26, 0x29, 0x168, [c[0], -c[1]], [-c[0], -c[1]]),
                    (8, 0x27, 0x28, 0x16c, [-c[0], c[1]], [c[0], c[1]]),
                ],
                2 | 4 => &[
                    (1, 0x24, 0x2a, 0x174, [c[1], c[0]], [c[1], -c[0]]),
                    (4, 0x25, 0x2b, 0x170, [-c[1], -c[0]], [-c[1], c[0]]),
                ],
                _ => &[],
            };
            for &(direction, neighbor, category, cat_offset, end, start) in orientations {
                let index = result.next_segment()?;
                result.primary_count += 1;
                let end_node = result.new_node()?;
                let start_node = result.new_node()?;
                for (node, offset) in [(end_node, end), (start_node, start)] {
                    set_point(
                        &mut result.nodes[node],
                        0x20,
                        [
                            shifted(float(road, 0xc), offset[0])?,
                            0.,
                            shifted(float(road, 0x10), offset[1])?,
                        ],
                    );
                }
                put(&mut result.nodes[start_node], 0x10, segment_ref(index));
                put(&mut result.nodes[start_node], 0x1c, 1);
                let row = &mut result.segments[index];
                row[0] = 13;
                row[0x20] = direction;
                row[0x21] = 1;
                row[0x22] = 1;
                put(row, 0x24, ROAD_BASE + source as u32 * 44);
                put(row, 0x60, node_ref(end_node));
                put(row, 0x64, node_ref(start_node));
                let lane = (road[neighbor] as u32).wrapping_sub(road_segment_count as u32);
                put(row, 0x68, LANE_BASE.wrapping_add(lane.wrapping_mul(32)));
                if (road[category] as usize) < category_count {
                    put(
                        &mut categories[road[category] as usize],
                        cat_offset,
                        segment_ref(index),
                    );
                }
                let length = distance(
                    point(&result.nodes[start_node], 0x20),
                    point(&result.nodes[end_node], 0x20),
                )?
                .to_f32()
                .to_bits()
                    & 0xffffff00;
                put(row, 0x58, length);
                put(row, 4, length);
                let mut inverse = extended(c[2])?;
                inverse
                    .div(extended(f32::from_bits(length))?)
                    .map_err(|e| format!("road route direction divisor{length:08x}: {e}"))?;
                put(row, 0x5c, inverse.to_f32().to_bits());
                let mut space = extended(f32::from_bits(length))?;
                space.mul_f32(c[3])?;
                put(row, 8, space.to_f32().to_bits());
            }
        }
        for source in 0..result.primary_count {
            let row = &result.segments[source];
            let road = ((word(row, 0x24) - ROAD_BASE) / 44) as usize;
            let (neighbor, turns): (usize, &[(u8, usize, u32)]) = match row[0x20] {
                1 => (0x24, &[(1, 0x24, 0x40), (2, 0x26, 0x120), (8, 0x27, 0x210)]),
                4 => (0x25, &[(4, 0x25, 0x40), (2, 0x26, 0x210), (8, 0x27, 0x120)]),
                2 => (0x26, &[(4, 0x25, 0x120), (1, 0x24, 0x210), (2, 0x26, 0x40)]),
                8 => (0x27, &[(4, 0x25, 0x210), (1, 0x24, 0x120), (8, 0x27, 0x40)]),
                _ => unreachable!(),
            };
            let junction = roads
                .get(roads[road][neighbor] as usize)
                .ok_or("road neighbor outside table")?;
            for &(direction, neighbor, command) in turns {
                if junction[2] & direction == 0 {
                    continue;
                }
                let destination = ROAD_BASE + u32::from(junction[neighbor]) * 44;
                let target = result.segments[..result.primary_count]
                    .iter()
                    .position(|row| word(row, 0x24) == destination && row[0x20] == direction)
                    .ok_or("original connector would dereference missing destination")?;
                result.connector(source, target, command, c)?;
            }
        }
        for source in 0..result.primary_count {
            let row = &result.segments[source];
            let road = ((word(row, 0x24) - ROAD_BASE) / 44) as usize;
            let category = roads[road][0x29] as usize;
            if row[0x20] == 2
                && category != 255
                && categories
                    .get(category)
                    .ok_or("entry category outside table")?[4]
                    != 0
            {
                result.ramps(source, category, roads, categories, c)?;
            }
        }
        Ok(result)
    }
    fn connector(
        &mut self,
        source: usize,
        target: usize,
        command: u32,
        c: &[f32; 13],
    ) -> Result<(), String> {
        let index = self.next_segment()?;
        self.connector_count += 1;
        let from = self.segments[source];
        let to = self.segments[target];
        let start = node_index(word(&from, 0x60))?;
        let end = node_index(word(&to, 0x64))?;
        let p0 = point(&self.nodes[start], 0x20);
        let p3 = point(&self.nodes[end], 0x20);
        let mut row = self.segments[index];
        row[0] = 2;
        row[0x21] = 2;
        row[0x22] = 2;
        row[0x20] = from[0x20] | to[0x20];
        put(&mut row, 0x60, node_ref(end));
        put(&mut row, 0x64, node_ref(start));
        put(&mut row, 0x68, word(&from, 0x68));
        for (at, p) in [(0x28, p0), (0x34, p0), (0x40, p3), (0x4c, p3)] {
            set_point(&mut row, at, p);
        }
        let factor = if command & 0x10 != 0 { c[4] } else { c[5] };
        for (control, a, b) in [
            (0x34, start, node_index(word(&from, 0x64))?),
            (0x40, end, node_index(word(&to, 0x60))?),
        ] {
            for axis in [0, 2] {
                let mut value = difference(
                    float(&self.nodes[a], 0x20 + axis * 4),
                    float(&self.nodes[b], 0x20 + axis * 4),
                )?;
                value.mul_f32(factor)?;
                value.add_f32(float(&row, control + axis * 4))?;
                put(&mut row, control + axis * 4, value.to_f32().to_bits());
            }
        }
        self.segments[index] = row;
        self.finish_curve(index, false, c[2])?;
        //44a3fe masks the NEXT slot, after incrementing684bcc, not this curve.
        if let Some(next) = self.segments.get_mut(index + 1) {
            let bits = word(next, 0x58) & 0xffffff00;
            put(next, 0x58, bits);
        }
        let count = word(&self.nodes[start], 0x1c).wrapping_add(1);
        put(&mut self.nodes[start], 0x1c, count);
        for (mask, offset) in [(0x40, 0x10), (0x10, 0x14), (0x20, 0x18)] {
            if command & mask != 0 {
                put(&mut self.nodes[start], offset, segment_ref(index));
            }
        }
        Ok(())
    }
    fn ramps(
        &mut self,
        source: usize,
        category: usize,
        roads: &[[u8; 44]],
        categories: &[[u8; 380]],
        c: &[f32; 13],
    ) -> Result<(), String> {
        if self.entry_count >= 64 {
            return Err("road entry pool exhausted".into());
        }
        let original = self.segments[source];
        let road = ((word(&original, 0x24) - ROAD_BASE) / 44) as usize;
        let end = node_index(word(&original, 0x60))?;
        let start = node_index(word(&original, 0x64))?;
        let end_pos = point(&self.nodes[end], 0x20);
        let start_pos = point(&self.nodes[start], 0x20);
        let mut new_nodes = [0; 4];
        for n in &mut new_nodes {
            *n = self.new_node()?;
        }
        let ramp0 = [shifted(end_pos[0], -c[6])?, 0., shifted(end_pos[2], -c[7])?];
        let ramp1 = [
            shifted(ramp0[0], -c[9])?,
            shifted(ramp0[1], -c[10])?,
            ramp0[2],
        ];
        let ramp2 = [
            shifted(start_pos[0], c[6])?,
            0.,
            shifted(start_pos[2], -c[7])?,
        ];
        let ramp3 = [
            shifted(ramp2[0], c[9])?,
            shifted(ramp2[1], -c[10])?,
            ramp2[2],
        ];
        for (n, p) in new_nodes.into_iter().zip([ramp0, ramp1, ramp2, ramp3]) {
            set_point(&mut self.nodes[n], 0x20, p);
            put(&mut self.nodes[n], 0x1c, 1);
        }
        let first = self.next_segment()?;
        if first + 4 > self.segments.len() {
            return Err("road ramp segment pool exhausted".into());
        }
        self.builder_count += 4;
        for (i, node) in new_nodes.into_iter().enumerate() {
            put(
                &mut self.nodes[node],
                0x10,
                segment_ref(first + if i == 2 { 3 } else { i }),
            );
        }
        let endpoints = [
            (new_nodes[0], end),
            (new_nodes[1], new_nodes[0]),
            (start, new_nodes[2]),
            (new_nodes[2], new_nodes[3]),
        ];
        let mut controls = [
            [ramp0, end_pos, end_pos, end_pos],
            [ramp1, ramp0, ramp0, ramp0],
            [start_pos, start_pos, start_pos, ramp2],
            [ramp2, ramp2, ramp2, ramp3],
        ];
        for (r, p, axis, delta) in [
            (0, 1, 0, -c[8]),
            (0, 1, 2, -c[7]),
            (0, 2, 0, -c[7]),
            (1, 1, 0, -c[10]),
            (1, 1, 1, -c[11]),
            (1, 2, 0, -c[11]),
            (2, 1, 0, c[7]),
            (2, 2, 0, c[8]),
            (2, 2, 2, -c[7]),
            (3, 1, 0, c[11]),
            (3, 2, 0, c[10]),
            (3, 2, 1, -c[11]),
        ] {
            controls[r][p][axis] = shifted(controls[r][p][axis], delta)?;
        }
        for i in 0..4 {
            let row = &mut self.segments[first + i];
            row[0] = if i == 0 { 5 } else { 0 };
            row[0x20] = if i == 2 { 0 } else { original[0x20] };
            row[0x21] = [0x20, 0x10, 4, 8][i];
            row[0x22] = 2;
            put(row, 0x24, word(&original, 0x24));
            put(row, 0x68, if i == 0 { word(&original, 0x68) } else { 0 });
            put(row, 0x64, node_ref(endpoints[i].0));
            put(row, 0x60, node_ref(endpoints[i].1));
            for (j, point) in controls[i].into_iter().enumerate() {
                set_point(row, 0x28 + j * 12, point);
            }
            self.finish_curve(first + i, true, c[2])?;
        }
        put(&mut self.nodes[start], 0x18, segment_ref(first + 2));
        let count = word(&self.nodes[start], 0x1c).wrapping_add(1);
        put(&mut self.nodes[start], 0x1c, count);
        let entry = &mut self.entries[self.entry_count];
        for (offset, value) in [
            (0, 100),
            (8, category as u32),
            (0x18, ROAD_BASE + u32::from(roads[road][0x27]) * 44),
            (0x1c, ROAD_BASE + u32::from(roads[road][0x26]) * 44),
            (0x20, segment_ref(first + 3)),
            (0x24, segment_ref(first + 1)),
            (0x28, segment_ref(source)),
        ] {
            put(entry, offset, value);
        }
        self.gates.push(GatePlacement {
            category,
            entry: self.entry_count,
            position: [
                float(&categories[category], 0xe0),
                0.,
                shifted(float(&categories[category], 0xe4), c[12])?,
            ],
        });
        self.entry_count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original full city road-graph construction observation"]
    fn matches_original_graph() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let settings = ConstructionSettings::from_v14(&image).unwrap();
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_ROAD_GRAPH").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTGRSET1");
        let bundle = data;
        let mut bundle_at = 12;
        for case in 0..word(&bundle, 8) {
            let length = word(&bundle, bundle_at) as usize;
            bundle_at += 4;
            let data = &bundle[bundle_at..bundle_at + length];
            bundle_at += length;
            assert_eq!(&data[..8], b"MTGRAPH1");
            assert_eq!(word(data, 8), 1);
            let mut at = 44;
            fn rows<const N: usize>(data: &[u8], at: &mut usize, count: usize) -> Vec<[u8; N]> {
                let rows = (0..count)
                    .map(|i| data[*at + i * N..*at + (i + 1) * N].try_into().unwrap())
                    .collect();
                *at += count * N;
                rows
            }
            let roads = rows::<44>(data, &mut at, word(data, 16) as usize);
            let board = crate::board::Board::from_v14(&image).unwrap();
            let (board_roads, segment_count) = board.traffic_roads().unwrap();
            assert_eq!(segment_count, word(data, 12) as usize);
            if case == 0 {
                for (index, (actual, expected)) in board_roads.iter().zip(&roads).enumerate() {
                    assert_eq!(actual, expected, "board road {index}");
                }
            }
            let mut categories = rows::<380>(data, &mut at, 52);
            let expected_categories = rows::<380>(data, &mut at, 52);
            let network = RoadNetwork::build(
                &roads,
                word(data, 12) as usize,
                &mut categories,
                word(data, 20) as usize,
                &settings,
            )
            .unwrap();
            assert_eq!(
                [
                    network.primary_count,
                    network.connector_count,
                    network.builder_count,
                    network.node_count,
                    network.entry_count
                ],
                std::array::from_fn(|i| word(data, 24 + i * 4) as usize)
            );
            fn compare<const N: usize>(actual: &[[u8; N]], expected: &[[u8; N]], name: &str) {
                assert_eq!(actual.len(), expected.len());
                for (r, (a, b)) in actual.iter().zip(expected).enumerate() {
                    for offset in 0..N {
                        assert_eq!(a[offset], b[offset], "{name} row {r} byte {offset:#x}");
                    }
                }
            }
            compare(&categories, &expected_categories, "category");
            compare(
                &network.segments,
                &rows::<108>(data, &mut at, 700),
                "segment",
            );
            compare(&network.nodes, &rows::<44>(data, &mut at, 512), "node");
            compare(&network.entries, &rows::<52>(data, &mut at, 64), "entry");
            compare(
                &network.lanes,
                &rows::<32>(data, &mut at, roads.len() - word(data, 12) as usize),
                "lane",
            );
            let gate_count = word(data, at) as usize;
            at += 4;
            let gate_rows = rows::<284>(data, &mut at, gate_count);
            assert_eq!(network.gates.len(), gate_count);
            for (gate, row) in network.gates.iter().zip(gate_rows) {
                assert_eq!(gate.entry, word(&row, 0x38) as usize);
                assert_eq!(
                    gate.category,
                    word(&network.entries[gate.entry], 8) as usize
                );
                assert_eq!(
                    gate.position.map(f32::to_bits),
                    std::array::from_fn(|i| word(&row, 4 + i * 4))
                );
            }
            assert_eq!(at, data.len());
        }
        assert_eq!(bundle_at, bundle.len());
    }
}
