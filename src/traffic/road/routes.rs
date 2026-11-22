//! Original448590/4482f0 route-table selection. The game reads the supplied
//! gamedata/route_smalltable.bin; it does not search the geometric road graph.
use super::{RouteChoice, RouteRequest};

pub struct RoadRoutes<'a> {
    table: &'a [u8; 65536],
    road_index_base: u32,
}
fn direction_index(direction: u32) -> Result<usize, String> {
    match direction {
        1 => Ok(0),
        4 => Ok(1),
        2 => Ok(2),
        8 => Ok(3),
        _ => Err("invalid route-table direction".into()),
    }
}
impl<'a> RoadRoutes<'a> {
    /// Host I/O supplies original bytes; no reconstructed cache is substituted.
    pub fn from_bytes(table: &'a [u8], road_index_base: u32) -> Result<Self, String> {
        Ok(Self {
            table: table
                .try_into()
                .map_err(|_| "route_smalltable.bin must contain65536 bytes")?,
            road_index_base,
        })
    }
    /// Original4482f0. Select the last two-bit state3 when preferred; otherwise
    /// the last state2, falling back to the last state1. Modes are direction bits.
    pub fn direction(
        &self,
        source: u32,
        direction: u32,
        target: u32,
        mode: u32,
        preferred: bool,
    ) -> Result<u32, String> {
        let source = source
            .checked_sub(self.road_index_base)
            .ok_or("route source precedes crossing table")? as usize;
        let target = target
            .checked_sub(self.road_index_base)
            .ok_or("route target precedes crossing table")? as usize;
        if source >= 64 || target >= 64 {
            return Err("route key outside64-crossing table".into());
        }
        let offset =
            ((source * 4 + direction_index(direction)?) * 64 + target) * 4 + direction_index(mode)?;
        let bits = self.table[offset];
        let mut selected = [None; 4];
        for i in 0..4 {
            selected[((bits >> (i * 2)) & 3) as usize] = Some(i);
        }
        let index = if preferred {
            selected[3]
        } else {
            selected[2].or(selected[1])
        };
        Ok(index.map_or(0, |i| [1, 4, 2, 8][i]))
    }
    /// Original448590 with the road vehicle caller's fixed mode2/search1.
    pub fn choose(&self, request: RouteRequest, roads: &[[u8; 44]]) -> Result<RouteChoice, String> {
        self.select(request, roads, 2, true)
    }
    fn select(
        &self,
        request: RouteRequest,
        roads: &[[u8; 44]],
        mode: u32,
        preferred: bool,
    ) -> Result<RouteChoice, String> {
        let selected = self.direction(
            request.source_road,
            request.direction,
            request.destination_road,
            mode,
            preferred,
        )?;
        let command = if selected == request.direction {
            0x40
        } else if selected == ((request.direction >> 1).wrapping_sub(request.direction << 3) & 15) {
            0x10
        } else if selected == ((request.direction >> 3).wrapping_add(request.direction << 1) & 15) {
            0x20
        } else {
            0
        };
        // The native early return for direction8 precedes its redundant switch
        // case: this direction never reports the two-hop destination flag.
        let found = if request.direction == 8 {
            false
        } else {
            let field = match request.direction {
                1 => 0x24,
                2 => 0x26,
                4 => 0x25,
                _ => return Err("invalid route direction".into()),
            };
            let first = roads
                .get(request.source_road as usize)
                .ok_or("route source road missing")?[field] as usize;
            let second = roads.get(first).ok_or("route second road missing")?[field];
            u32::from(second) == request.destination_road
        };
        Ok(RouteChoice { command, found })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::word;
    #[test]
    #[ignore = "requires original route table and road-choice observations"]
    fn matches_original_routes() {
        let table = std::fs::read(std::env::var("OPENMT2001_ROAD_ROUTE_TABLE").unwrap()).unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let board = crate::board::Board::from_v14(&image).unwrap();
        let (city_roads, _) = board.traffic_roads().unwrap();
        for (env, actual_city) in [
            ("OPENMT2001_NATIVE_ROAD_ROUTE_CHOICE", false),
            ("OPENMT2001_NATIVE_ROAD_ROUTE_CITY", true),
        ] {
            let data = std::fs::read(std::env::var(env).unwrap()).unwrap();
            assert_eq!(&data[..8], b"MTRCHOI1");
            assert_eq!(word(&data, 8), 1);
            let mut roads = vec![[0; 44]; word(&data, 20) as usize];
            if actual_city {
                roads = city_roads.clone();
            } else {
                for (i, row) in roads.iter_mut().enumerate() {
                    for j in 0..4 {
                        row[0x24 + j] = (i + j) as u8;
                    }
                }
            }
            let mut at = 28;
            for case in 0..word(&data, 12) {
                let input: [u32; 12] = std::array::from_fn(|i| word(&data, at + i * 4));
                at += 48;
                let synthetic = vec![input[5] as u8; 65536];
                let routes = RoadRoutes::from_bytes(
                    if input[10] > 0 { &table } else { &synthetic },
                    word(&data, 16),
                )
                .unwrap();
                let result = routes
                    .select(
                        RouteRequest {
                            source_road: input[0],
                            destination_road: input[1],
                            direction: input[2],
                        },
                        &roads,
                        input[3],
                        input[4] == 1,
                    )
                    .unwrap();
                assert_eq!(
                    (result.command, u32::from(result.found)),
                    (input[8], input[7]),
                    "{env} case{case}"
                );
                at += input[9] as usize * 12 + input[10] as usize * 24;
            }
            assert_eq!(at, data.len());
        }
    }
}
