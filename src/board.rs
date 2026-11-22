//! City topology recovered from patched mc.exe, FUN_00449360 / FUN_00448bc0.
//! Reads the user's embedded map; no original table is compiled into this crate.

use crate::pe::PeImage;

const STRIDE: usize = 113;
const ROWS: usize = 70;

#[derive(Debug, Clone)]
pub struct Road {
    pub grid: [usize; 4], // x, y, width, height in the source ASCII map
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub kind: u32,
    pub angle: f32,
    pub elevated: bool,
    pub neighbors: [Option<usize>; 4], // N, E, S, W
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: usize,
    pub name: String,
    pub text_id: u32,
    pub kind: u8,
    pub center: [f32; 2],
    pub size: [f32; 2],
    /// Original block tint copied into pavement/ground object colors.
    pub pavement_color: u32,
    pub flags: u8,
    pub street_sides: u8, // N=1, E=2, S=4, W=8
}

#[derive(Debug)]
pub struct Board {
    pub roads: Vec<Road>,
    pub blocks: Vec<Block>,
    // Slots without map blocks still receive the native final coordinate shift.
    inactive_category_center: [f32; 2],
    inactive_category_notification: u32,
    segment_count: usize,
}

pub struct CategoryGeometry {
    pub centers: [[f32; 2]; 52],
    pub sizes: [[f32; 2]; 52],
}

/// Road references consumed by resident crossing occupancy checks.
pub struct CrossingTopology {
    pub category_links: [[u8; 4]; 52],
    /// Native road+24..+27 order: north, south, east, west.
    pub road_neighbors: Vec<[u8; 4]>,
    pub lane_base: u32,
}

impl Board {
    /// Fresh lane-table result after0044bd50 and0044ab20. The latter
    /// overwrites the preliminary connectivity flags with signal mask5 and
    /// zero occupancy. Vehicle/route geometry initialization remains separate.
    pub fn fresh_crossing_lanes(&self) -> Vec<[u8; 32]> {
        let mut row = [0; 32];
        row[..4].copy_from_slice(&5_u32.to_le_bytes());
        vec![row; self.roads.len() - self.segment_count]
    }

    /// Original44-byte road records consumed by vehicle graph/search routines.
    /// Includes449bd9's reverse category links, in ascending category order.
    pub fn traffic_roads(&self) -> Result<(Vec<[u8; 44]>, usize), String> {
        let topology = self.crossing_topology()?;
        let mut rows = vec![[0; 44]; self.roads.len()];
        for (index, (row, road)) in rows.iter_mut().zip(&self.roads).enumerate() {
            row[0] = u8::try_from(index).map_err(|_| "road index exceeds byte table")?;
            row[1] = u8::from(road.elevated);
            for (side, neighbor) in road.neighbors.iter().enumerate() {
                if neighbor.is_some() {
                    row[2] |= 1 << side;
                }
            }
            row[4..8].copy_from_slice(&road.kind.to_le_bytes());
            for (i, &value) in road.grid.iter().enumerate() {
                row[8 + i] = u8::try_from(value).map_err(|_| "road grid exceeds byte range")?;
            }
            for (i, value) in road
                .center
                .into_iter()
                .chain(road.size)
                .chain([road.angle])
                .enumerate()
            {
                row[12 + i * 4..16 + i * 4].copy_from_slice(&value.to_le_bytes());
            }
            row[0x24..0x28].copy_from_slice(&topology.road_neighbors[index]);
            row[0x28..0x2c].fill(255);
        }
        for (category, enabled) in self.category_enabled().into_iter().enumerate() {
            if enabled == 0 {
                continue;
            }
            for (side, offset) in [0x29, 0x28, 0x2b, 0x2a].into_iter().enumerate() {
                let road = topology.category_links[category][side];
                if road != 255 {
                    rows[road as usize][offset] = category as u8;
                }
            }
        }
        Ok((rows, self.segment_count))
    }

    /// Runtime category+4 is an enabled byte, distinct from the name-table
    /// block kind. Original board construction enables only populated slots.
    pub fn category_enabled(&self) -> [u8; 52] {
        let mut enabled = [0; 52];
        for block in &self.blocks {
            enabled[block.id] = 1;
        }
        enabled
    }

    /// These addresses are specific to the verified 1.4 replacement executable.
    /// The caller must select that version; this is not a version detector.
    pub fn from_v14(image: &PeImage<'_>) -> Result<Self, String> {
        let mut board = Self::topology_prefix(image)?;
        // 00449c67..00449cd9 runs after topology construction. Its shift
        // aligns the procedural board with the original static scenery.
        let shift = [image.f32_at(0x525b30)?, image.f32_at(0x525b2c)?];
        for center in board
            .roads
            .iter_mut()
            .map(|r| &mut r.center)
            .chain(board.blocks.iter_mut().map(|b| &mut b.center))
        {
            for axis in 0..2 {
                center[axis] = (f64::from(center[axis]) - f64::from(shift[axis])) as f32;
            }
        }
        board.inactive_category_center = shift.map(|v| (0. - f64::from(v)) as f32);
        Ok(board)
    }

    /// Full runtime table, including unused map slots40/45. Active geometry
    /// comes from the same blocks used for rendering; unused sizes remain zero.
    pub fn category_geometry(&self) -> CategoryGeometry {
        let mut result = CategoryGeometry {
            centers: [self.inactive_category_center; 52],
            sizes: [[0.; 2]; 52],
        };
        for block in &self.blocks {
            result.centers[block.id] = block.center;
            result.sizes[block.id] = block.size;
        }
        result
    }

    /// Native runtime+8c indexes the original 88-byte name table. Unused
    /// categories retain index zero, so their notification value uses row zero.
    pub fn category_notification_values(&self) -> [u32; 52] {
        let mut values = [self.inactive_category_notification; 52];
        for block in &self.blocks {
            values[block.id] = block.text_id;
        }
        values
    }

    /// Original00449a3a..00449bd9 scans roads in native order. It uses the
    /// source block's X size for all four probes, and full road sizes as bounds.
    pub fn crossing_topology(&self) -> Result<CrossingTopology, String> {
        let geometry = self.category_geometry();
        let mut links = [[255; 4]; 52];
        for (category, row) in links.iter_mut().enumerate() {
            let [x, z] = geometry.centers[category];
            let [width, depth] = geometry.sizes[category];
            let left = (f64::from(x) - f64::from(width) * 0.5) as f32;
            let bottom = (f64::from(z) - f64::from(depth) * 0.5) as f32;
            let add = |a: f32, b: f64| (f64::from(a) + b) as f32;
            let half = f64::from(width) * 0.5;
            let points = [
                [add(left, half), add(bottom, f64::from(width))],
                [add(left, half), bottom],
                [add(left, f64::from(width)), add(bottom, half)],
                [left, add(bottom, half)],
            ];
            for (slot, point) in row.iter_mut().zip(points) {
                if let Some(index) = self.roads.iter().position(|road| {
                    (0..2).all(|axis| {
                        f64::from(road.size[axis])
                            >= (f64::from(road.center[axis]) - f64::from(point[axis])).abs()
                    })
                }) {
                    *slot = u8::try_from(index)
                        .ok()
                        .filter(|i| *i != 255)
                        .ok_or("crossing road exceeds byte table")?;
                }
            }
        }
        let road_neighbors = self
            .roads
            .iter()
            .map(|road| {
                let mut result = [255; 4];
                for (slot, side) in [0, 2, 1, 3].into_iter().enumerate() {
                    if let Some(index) = road.neighbors[side] {
                        result[slot] = u8::try_from(index)
                            .ok()
                            .filter(|i| *i != 255)
                            .ok_or("crossing neighbor exceeds byte table")?;
                    }
                }
                Ok(result)
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(CrossingTopology {
            category_links: links,
            road_neighbors,
            lane_base: self.segment_count as u32,
        })
    }

    fn topology_prefix(image: &PeImage<'_>) -> Result<Self, String> {
        let map = image.at(0x5366b8, STRIDE * ROWS)?;
        let names = image.at(0x5385a0, 52 * 88)?;
        Self::decode(
            map,
            names,
            image.at(0x539780, 12 * 0x3c)?,
            [image.f32_at(0x525b38)?, image.f32_at(0x525b34)?],
        )
    }

    fn decode(map: &[u8], names: &[u8], colors: &[u8], origin: [f32; 2]) -> Result<Self, String> {
        if map.len() != STRIDE * ROWS || names.len() != 52 * 88 {
            return Err("invalid city table lengths".into());
        }
        let cell = |x: usize, y: usize| -> u8 {
            if x < STRIDE && y < ROWS {
                map[y * STRIDE + x]
            } else {
                0
            }
        };
        let mut roads = Vec::new();
        let mut segment_count = 0;
        // Native order is every road segment, then every intersection, row-major.
        for marker in *b".+" {
            if marker == b'+' {
                segment_count = roads.len();
            }
            for y in 1..ROWS {
                for x in 1..STRIDE {
                    if cell(x, y) != marker || cell(x - 1, y) == marker || cell(x, y - 1) == marker
                    {
                        continue;
                    }
                    let mut w = 1;
                    let mut h = 1;
                    while cell(x + w, y) == marker {
                        w += 1;
                    }
                    while cell(x, y + h) == marker {
                        h += 1;
                    }
                    let (size, kind, angle) = match (marker, w, h) {
                        (b'.', 4, 5) => ([25., 82.], 2, 0.),
                        (b'.', 6, 5) => ([37., 82.], 4, 0.),
                        (b'.', 7, 3) => ([82., 25.], 1, 90.),
                        (b'.', 7, 4) => ([82., 37.], 3, 90.),
                        (b'.', 2, 5) => ([12., 82.], 6, 0.),
                        (b'+', 4, 3) => ([25., 25.], 12, 0.),
                        (b'+', 4, 4) => ([25., 37.], 13, 0.),
                        (b'+', 6, 3) => ([37., 25.], 14, 90.),
                        (b'+', 6, 4) => ([37., 37.], 15, 0.),
                        (b'+', 2, 3) => ([12., 25.], 16, 0.),
                        _ => {
                            return Err(format!("unrecognized road rectangle at {x},{y}: {w}x{h}"));
                        }
                    };
                    roads.push(Road {
                        grid: [x, y, w, h],
                        size,
                        kind,
                        angle,
                        elevated: cell(x + 1, y + 1) == b'H',
                        center: [0.; 2],
                        neighbors: [None; 4],
                    });
                }
            }
        }
        if roads.is_empty() || roads.len() > 255 {
            return Err("invalid road count".into());
        }
        roads[0].center = [
            roads[0].size[0] * 0.5 - origin[0],
            origin[1] - roads[0].size[1] * 0.5,
        ];
        connect(&mut roads, 0);
        let mut xs = [None; STRIDE];
        let mut ys = [None; ROWS];
        for road in &roads {
            xs[road.grid[0]] = Some(road.center[0] - road.size[0] * 0.5);
            ys[road.grid[1]] = Some(road.center[1] + road.size[1] * 0.5);
        }
        let mut blocks = Vec::new();
        for y in 1..ROWS {
            for x in 1..STRIDE {
                if cell(x, y) != b'*' || cell(x - 1, y) == b'*' || cell(x, y - 1) == b'*' {
                    continue;
                }
                let mut w = 1;
                let mut h = 1;
                while cell(x + w, y) == b'*' {
                    w += 1;
                }
                while cell(x, y + h) == b'*' {
                    h += 1;
                }
                let mut id = 0usize;
                let mut n = 1;
                while cell(x + n, y + 1).is_ascii_digit() {
                    id = id
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(usize::from(cell(x + n, y + 1) - b'0')))
                        .ok_or("block ID overflow")?;
                    n += 1;
                }
                if n == 1 || id >= 52 || blocks.iter().any(|b: &Block| b.id == id) {
                    return Err("invalid or duplicate block ID".into());
                }
                let left = xs[x].ok_or("missing block left coordinate")?;
                let right = xs
                    .get(x + w)
                    .copied()
                    .flatten()
                    .ok_or("missing block right coordinate")?;
                let top = ys[y].ok_or("missing block top coordinate")?;
                let bottom = ys
                    .get(y + h)
                    .copied()
                    .flatten()
                    .ok_or("missing block bottom coordinate")?;
                let record = &names[id * 88..(id + 1) * 88];
                let end = record[5..]
                    .iter()
                    .position(|&v| v == 0)
                    .ok_or("unterminated block name")?;
                let name = std::str::from_utf8(&record[5..5 + end])
                    .map_err(|_| "invalid block name")?
                    .to_owned();
                let mut flags = 0;
                let mut sides = 0;
                for dx in 1..w {
                    match cell(x + dx, y + 2) {
                        b'H' => flags |= 1,
                        b'I' => flags |= 2,
                        _ => {}
                    }
                    match cell(x + dx, y + 3) {
                        b'N' => sides |= 1,
                        b'E' => sides |= 2,
                        b'S' => sides |= 4,
                        b'W' => sides |= 8,
                        _ => {}
                    }
                }
                blocks.push(Block {
                    id,
                    name,
                    text_id: u32::from_le_bytes(record[..4].try_into().unwrap()),
                    kind: record[4],
                    pavement_color: u32::from_le_bytes(
                        colors
                            .get(usize::from(record[4]) * 0x3c..usize::from(record[4]) * 0x3c + 4)
                            .ok_or("block tint outside original palette")?
                            .try_into()
                            .unwrap(),
                    ),
                    center: [(right - left) * 0.5 + left, top - (top - bottom) * 0.5],
                    size: [right - left, top - bottom],
                    flags,
                    street_sides: sides,
                });
            }
        }
        blocks.sort_by_key(|b| b.id);
        Ok(Self {
            roads,
            blocks,
            inactive_category_center: [0.; 2],
            inactive_category_notification: u32::from_le_bytes(names[..4].try_into().unwrap()),
            segment_count,
        })
    }
}

/// Same N/E/S/W depth-first edge visitation as 00448bc0. Neighbors can be
/// revisited through a different edge; a conventional node-visited BFS changes
/// the native assignment order. All recovered dimensions are exact half-units.
fn connect(roads: &mut [Road], index: usize) {
    let [x, y, w, h] = roads[index].grid.map(|v| v as i32);
    let probes = [
        [x, y - 1, x + w - 1, y - 1],
        [x + w, y, x + w, y + h - 1],
        [x, y + h, x + w - 1, y + h],
        [x - 1, y, x - 1, y + h - 1],
    ];
    for (side, probe) in probes.iter().enumerate() {
        if roads[index].neighbors[side].is_some() {
            continue;
        }
        let neighbor = roads.iter().position(|r| {
            let [rx, ry, rw, rh] = r.grid.map(|v| v as i32);
            probe[0] >= rx && probe[2] < rx + rw && probe[1] >= ry && probe[3] < ry + rh
        });
        if let Some(other) = neighbor {
            roads[index].neighbors[side] = Some(other);
            roads[other].neighbors[(side + 2) % 4] = Some(index);
            let mut center = roads[index].center;
            let axis = usize::from(side % 2 == 0);
            let distance = (roads[index].size[axis] + roads[other].size[axis]) * 0.5;
            center[axis] += if side < 2 { distance } else { -distance };
            roads[other].center = center;
            connect(roads, other);
        }
    }
    let mask = roads[index]
        .neighbors
        .iter()
        .enumerate()
        .fold(0, |m, (i, n)| m | if n.is_some() { 1 << i } else { 0 });
    match (roads[index].kind, mask) {
        (1, 2) => {
            roads[index].kind = 10;
            roads[index].angle = 270.;
        }
        (1, 8) => {
            roads[index].kind = 9;
            roads[index].angle = 90.;
        }
        (2, 1) => {
            roads[index].kind = 8;
            roads[index].angle = 0.;
        }
        (2, 4) => {
            roads[index].kind = 7;
            roads[index].angle = 180.;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original block-to-road crossing links"]
    fn matches_native_crossing_topology() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY_CROSSINGS").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTCITYX1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let board = Board::from_v14(&PeImage::parse(&exe).unwrap()).unwrap();
        let topology = board.crossing_topology().unwrap();
        assert_eq!(topology.lane_base, word(8));
        assert_eq!(topology.road_neighbors.len(), word(16) as usize);
        for (index, neighbors) in topology.road_neighbors.iter().enumerate() {
            assert_eq!(
                neighbors.as_slice(),
                &data[20 + index * 44 + 36..20 + index * 44 + 40],
                "road{index}"
            );
        }
        let blocks = 20 + topology.road_neighbors.len() * 44;
        for (category, links) in topology.category_links.iter().enumerate() {
            assert_eq!(
                links.as_slice(),
                &data[blocks + category * 380 + 0x164..blocks + category * 380 + 0x168],
                "category{category}"
            );
        }
    }

    #[test]
    #[ignore = "requires original executable and native final coordinate observations"]
    fn matches_native_world_coordinates() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let oracle = std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY_WORLD").unwrap()).unwrap();
        let board = Board::from_v14(&PeImage::parse(&exe).unwrap()).unwrap();
        assert_eq!(&oracle[..8], b"MTCITYW1");
        let word = |at| u32::from_le_bytes(oracle[at..at + 4].try_into().unwrap());
        assert_eq!(board.roads.len(), word(16) as usize);
        for (i, road) in board.roads.iter().enumerate() {
            let at = 20 + i * 44 + 12;
            assert_eq!(
                road.center.map(f32::to_bits),
                [word(at), word(at + 4)],
                "road {i}"
            );
        }
        let geometry = board.category_geometry();
        let notifications = board.category_notification_values();
        for (category, &notification) in notifications.iter().enumerate() {
            let at = 20 + board.roads.len() * 44 + category * 380;
            let image = PeImage::parse(&exe).unwrap();
            let value = u32::from_le_bytes(
                image
                    .at(0x5385a0 + word(at + 0x8c) * 88, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            assert_eq!(notification, value, "category{category} notification value");
            assert_eq!(
                geometry.centers[category].map(f32::to_bits),
                [word(at + 0xe0), word(at + 0xe4)],
                "category{category} center"
            );
            assert_eq!(
                geometry.sizes[category].map(f32::to_bits),
                [word(at + 0xd0), word(at + 0xd4)],
                "category{category} size"
            );
        }
    }

    #[test]
    #[ignore = "requires private v1.4 executable and original lane initializer oracle"]
    fn matches_native_crossing_lanes() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let oracle =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_TRAFFIC_LANES").unwrap()).unwrap();
        let board = Board::from_v14(&PeImage::parse(&exe).unwrap()).unwrap();
        let word = |at| u32::from_le_bytes(oracle[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(&oracle[..8], b"MTLANE02");
        assert_eq!(word(8), 1);
        assert_eq!(word(12), board.segment_count);
        assert_eq!(word(20), board.roads.len());
        let lanes = board.fresh_crossing_lanes();
        assert_eq!(word(16), lanes.len());
        assert_eq!(word(24), 44);
        assert_eq!(word(28), 32);
        let before = 52 + word(40);
        let after = before + word(44);
        assert!(oracle[before..after].iter().all(|&byte| byte == 0));
        assert_eq!(word(48), lanes.len() * 32);
        for (index, lane) in lanes.iter().enumerate() {
            let road = 52 + (board.segment_count + index) * 44;
            assert!(word(road + 4) >= 11);
            assert_eq!(
                (oracle[road + 2] & 15).count_ones() as usize,
                board.roads[board.segment_count + index]
                    .neighbors
                    .iter()
                    .filter(|n| n.is_some())
                    .count(),
                "crossing {index} connected directions"
            );
            assert_eq!(
                lane.as_slice(),
                &oracle[after + index * 32..after + (index + 1) * 32],
                "crossing {index} full lane"
            );
        }
    }

    /// Independent native instructions produce the reference with tools/native_city.py.
    /// This checks topology fields only, not rendering or simulation fidelity.
    #[test]
    #[ignore = "requires private v1.4 executable and original-instruction oracle"]
    fn matches_native_topology() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").expect("OPENMT2001_EXE")).unwrap();
        let oracle =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY").expect("OPENMT2001_NATIVE_CITY"))
                .unwrap();
        let board = Board::topology_prefix(&PeImage::parse(&exe).unwrap()).unwrap();
        assert_eq!(&oracle[..8], b"MTCITY14");
        let word = |bytes: &[u8], at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(board.roads.len(), word(&oracle, 16) as usize);
        for (i, road) in board.roads.iter().enumerate() {
            let r = &oracle[20 + i * 44..20 + (i + 1) * 44];
            assert_eq!(
                road.grid,
                r[8..12]
                    .iter()
                    .map(|&v| usize::from(v))
                    .collect::<Vec<_>>()
                    .as_slice(),
                "road {i} grid"
            );
            assert_eq!(road.kind, word(r, 4), "road {i} kind");
            assert_eq!(road.elevated, r[1] & 1 != 0, "road {i} elevation flag");
            for axis in 0..2 {
                assert_eq!(
                    road.center[axis].to_bits(),
                    word(r, 12 + axis * 4),
                    "road {i} center axis {axis}"
                );
                assert_eq!(
                    road.size[axis].to_bits(),
                    word(r, 20 + axis * 4),
                    "road {i} size axis {axis}"
                );
            }
            assert_eq!(road.angle.to_bits(), word(r, 28), "road {i} angle");
            for (side, offset) in [36, 38, 37, 39].iter().enumerate() {
                assert_eq!(
                    road.neighbors[side],
                    if r[*offset] == 255 {
                        None
                    } else {
                        Some(usize::from(r[*offset]))
                    },
                    "road {i} neighbor {side}"
                );
            }
        }
        let native_blocks = &oracle[20 + board.roads.len() * 44..];
        assert_eq!(
            board.category_enabled(),
            std::array::from_fn(|i| native_blocks[i * 380 + 4])
        );
        assert_eq!(
            board.blocks.len(),
            native_blocks
                .chunks_exact(380)
                .filter(|r| r[4] != 0)
                .count()
        );
        for block in &board.blocks {
            let r = &native_blocks[block.id * 380..(block.id + 1) * 380];
            assert_eq!(
                block.pavement_color,
                word(r, 0x94),
                "{} pavement tint",
                block.name
            );
            assert_eq!(
                block.flags,
                u8::try_from(word(r, 0x90)).unwrap(),
                "{} flags",
                block.name
            );
            assert_eq!(
                u32::from(block.street_sides),
                word(r, 0x178),
                "{} sides",
                block.name
            );
            for axis in 0..2 {
                assert_eq!(
                    block.center[axis].to_bits(),
                    word(r, 0xe0 + axis * 4),
                    "{} center axis {axis}",
                    block.name
                );
                assert_eq!(
                    block.size[axis].to_bits(),
                    word(r, 0xd0 + axis * 4),
                    "{} size axis {axis}",
                    block.name
                );
            }
        }
    }
}
