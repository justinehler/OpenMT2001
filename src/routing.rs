//! Original category route search00449150/00448f20 and cache completion0045ebd0.

pub struct CategoryRoutes {
    adjacency: [[u8; 52]; 52],
    cache: [[u8; 52]; 52],
    max_cost: i32,
}

struct Route {
    path: Vec<u8>,
    cost: i32,
}

struct Search<'a> {
    adjacency: &'a [[u8; 52]; 52],
    best: i32,
    path: [u8; 64],
    routes: Vec<Route>,
    overflow: bool,
}

impl Search<'_> {
    fn visit(&mut self, node: usize, target: usize, depth: usize) -> bool {
        // Native pruning precedes the target equality check. A conventional
        // shortest-path substitution would change which discoveries survive.
        if depth as i32 >= self.best {
            return false;
        }
        self.path[depth] = node as u8;
        if node == target {
            return true;
        }
        for candidate in 0..52 {
            if self.adjacency[node][candidate] == 0
                || self.path[..depth].contains(&(candidate as u8))
            {
                continue;
            }
            if self.visit(candidate, target, depth + 1) {
                if self.routes.len() >= 256 {
                    self.overflow = true;
                    return false;
                }
                self.routes.push(Route {
                    path: self.path[..depth + 2].to_vec(),
                    cost: depth as i32,
                });
                self.best = self.best.min(depth as i32);
            } else if self.overflow {
                return false;
            }
        }
        false
    }
}

impl CategoryRoutes {
    /// Original00449040 uses source block size plus37, exact axis alignment,
    /// and two explicit map links. The matrix can be directed for unequal sizes.
    pub fn from_geometry(centers: &[[f32; 2]; 52], sizes: &[[f32; 2]; 52], max_cost: i32) -> Self {
        let mut adjacency = [[0; 52]; 52];
        let same = |a: f32, b: f32| a == b || a.is_nan() || b.is_nan();
        for source in 0..52 {
            for target in 0..52 {
                if source == target {
                    continue;
                }
                let axis = if same(centers[source][0], centers[target][0]) {
                    Some(1)
                } else if same(centers[source][1], centers[target][1]) {
                    Some(0)
                } else {
                    None
                };
                if let Some(axis) = axis {
                    let distance =
                        (f64::from(centers[source][axis]) - f64::from(centers[target][axis])).abs();
                    adjacency[source][target] =
                        u8::from(f64::from(sizes[source][axis]) + 37. >= distance);
                }
            }
        }
        adjacency[30][36] = 1;
        adjacency[36][30] = 1;
        Self::new(adjacency, max_cost)
    }

    /// Build from all runtime categories, including the board's unused slots.
    pub fn from_board(board: &crate::board::Board, max_cost: i32) -> Self {
        let geometry = board.category_geometry();
        Self::from_geometry(&geometry.centers, &geometry.sizes, max_cost)
    }

    pub fn adjacency(&self) -> &[[u8; 52]; 52] {
        &self.adjacency
    }

    pub fn new(adjacency: [[u8; 52]; 52], max_cost: i32) -> Self {
        Self::from_cache(adjacency, [[0xfe; 52]; 52], max_cost)
    }

    pub fn from_cache(adjacency: [[u8; 52]; 52], cache: [[u8; 52]; 52], max_cost: i32) -> Self {
        Self {
            adjacency,
            cache,
            max_cost,
        }
    }

    pub fn cache(&self) -> &[[u8; 52]; 52] {
        &self.cache
    }

    /// Keep resolved cells unchanged. An FE cell runs the original ordered DFS,
    /// selecting the first strictly lower-cost record and populating its suffixes.
    pub fn next_hop(&mut self, source: u8, target: u8) -> Result<u8, String> {
        let source = usize::from(source);
        let target = usize::from(target);
        if source >= 52 || target >= 52 {
            return Err("route category outside table".into());
        }
        if self.cache[source][target] != 0xfe {
            return Ok(self.cache[source][target]);
        }
        let mut search = Search {
            adjacency: &self.adjacency,
            best: self.max_cost,
            path: [0; 64],
            routes: Vec::new(),
            overflow: false,
        };
        search.visit(source, target, 0);
        let mut best = None;
        let mut cost = 99999;
        for route in &search.routes {
            if route.cost < cost {
                cost = route.cost;
                best = Some(route);
            }
        }
        if let Some(route) = best.filter(|r| r.cost <= self.max_cost) {
            for pair in route.path.windows(2) {
                self.cache[usize::from(pair[0])][target] = pair[1];
            }
        } else {
            self.cache[source][target] = 0xff;
        }
        Ok(self.cache[source][target])
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original board adjacency observations"]
    fn matches_native_board_adjacency() {
        // The render Board omits unused categories40/45. Routing retains all52
        // runtime records, including their original zero geometry.
        let city = std::fs::read(std::env::var("OPENMT2001_NATIVE_CITY").unwrap()).unwrap();
        assert_eq!(&city[..8], b"MTCITY14");
        let start = 20 + u32::from_le_bytes(city[16..20].try_into().unwrap()) as usize * 44;
        let mut centers = [[0.; 2]; 52];
        let mut sizes = [[0.; 2]; 52];
        for i in 0..52 {
            for axis in 0..2 {
                let read = |offset| {
                    f32::from_le_bytes(
                        city[start + i * 380 + offset..start + i * 380 + offset + 4]
                            .try_into()
                            .unwrap(),
                    )
                };
                centers[i][axis] = read(0xe0 + axis * 4);
                sizes[i][axis] = read(0xd0 + axis * 4);
            }
        }
        let routes = super::CategoryRoutes::from_geometry(&centers, &sizes, 256);
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CATEGORY_ADJACENCY").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTADJ001");
        assert_eq!(data.len(), 8 + 52 * 52);
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let board =
            crate::board::Board::from_v14(&crate::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let from_board = super::CategoryRoutes::from_board(&board, 256);
        assert_eq!(from_board.adjacency(), routes.adjacency());
        for (i, row) in routes.adjacency().iter().enumerate() {
            assert_eq!(
                row.as_slice(),
                &data[8 + i * 52..8 + (i + 1) * 52],
                "category{i}"
            );
        }
    }

    #[test]
    #[ignore = "requires original route construction observations"]
    fn matches_native_category_routes() {
        let data =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CATEGORY_ROUTES").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTROUTE1");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for group in 0..word(8) {
            let limit = word(at) as i32;
            let count = word(at + 4);
            at += 8;
            let mut graph = [[0; 52]; 52];
            for row in &mut graph {
                row.copy_from_slice(&data[at..at + 52]);
                at += 52;
            }
            let mut routes = super::CategoryRoutes::new(graph, limit);
            for query in 0..count {
                let source = word(at) as u8;
                let target = word(at + 4) as u8;
                let result = word(at + 8) as u8;
                at += 12;
                assert_eq!(
                    routes.next_hop(source, target).unwrap(),
                    result,
                    "group{group} query{query}"
                );
                for row in routes.cache() {
                    assert_eq!(
                        row.as_slice(),
                        &data[at..at + 52],
                        "group{group} query{query}"
                    );
                    at += 52;
                }
            }
        }
        assert_eq!(at, data.len());
    }
}
