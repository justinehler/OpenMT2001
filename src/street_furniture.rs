//! Original v1.4 static street furniture placement (449360, 4b6940, 4b55d0).
use crate::{board::Block, mesh::CachedMesh, people::PeopleTransform};

pub const EARLY_LAMP: &str = "gamedata2/NEW_building stuff/streetlight_1930.tm";
pub const LATE_LAMP: &str = "gamedata2/NEW_building stuff/streetlight4.tm";
pub const BENCH: &str = "gamedata2/NEW_building stuff/landscape/park_bench.tm";
pub const SUBWAY: &str = "gamedata2/NEW_building stuff/subway.tm";
pub const PARKING: &str = "gamedata2/NEW_building stuff/undergr_parking.tm";
pub const BENCH_LOCATORS: &str = "gamedata2/bench_locators/bench_locators.tm";

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub position: [f32; 3],
    pub yaw: f32,
}
impl Pose {
    pub fn identity(position: [f32; 3]) -> Self {
        Self { position, yaw: 0. }
    }
    /// Object rows from 0x4e3380. 0x4e0b50 passes it the negated descriptor
    /// yaw (+0x14) and 0x4e3380 negates it again, so the rows are
    /// (cos, 0, sin) and (-sin, 0, cos) of the stored yaw, the same matrix as
    /// resident and vehicle descriptors. Every furniture creator (0x4b6940
    /// lamps, 0x44fd50 subways, 0x44a4a0 parking, 0x4b55d0 benches and the
    /// 0x44dbe0 signal posts) stores its placement yaw there unchanged.
    fn basis(self) -> [[f32; 3]; 3] {
        let radians = (f64::from(self.yaw) * f64::from(0.017453292_f32)) as f32;
        let (sine, cosine) = f64::from(radians).sin_cos();
        let (sine, cosine) = (sine as f32, cosine as f32);
        [[cosine, 0., sine], [0., 1., 0.], [-sine, 0., cosine]]
    }
    pub fn transform(self) -> PeopleTransform {
        let [a, b, c] = self.basis();
        PeopleTransform {
            matrix: [a, b, c.map(|v| -v), self.position],
            translation_only: self.yaw == 0.,
        }
    }
    pub fn sunlight(self, sun: [f32; 3]) -> [f32; 3] {
        self.basis().map(|row| {
            (f64::from(sun[2]) * f64::from(row[2])
                + (f64::from(sun[1]) * f64::from(row[1]) + f64::from(sun[0]) * f64::from(row[0])))
                as f32
        })
    }
}

///449360 creates two lamps on each exposed side of every enabled category.
///These are the post bases (+0xc/+0x14), not the separate light glow positions.
pub fn lamps(blocks: &[Block]) -> Vec<Pose> {
    let mut poses = Vec::new();
    for block in blocks {
        for (side, x, z, yaw) in [
            (1, -16., 41., 270.),
            (1, 16., 41., 270.),
            (2, 41., 16., 180.),
            (2, 41., -16., 180.),
            (4, 16., -41., 90.),
            (4, -16., -41., 90.),
            (8, -41., -16., 0.),
            (8, -41., 16., 0.),
        ] {
            if block.street_sides & side != 0 {
                poses.push(Pose {
                    position: [
                        (f64::from(block.center[0]) + x) as f32,
                        0.,
                        (f64::from(block.center[1]) + z) as f32,
                    ],
                    yaw,
                });
            }
        }
    }
    poses
}

///44fd50: one subway entrance for each enabled category.
pub fn subways(blocks: &[Block]) -> Vec<Pose> {
    blocks
        .iter()
        .map(|block| Pose {
            position: [
                block.center[0],
                0.,
                (f64::from(block.center[1]) - 41.) as f32,
            ],
            yaw: 180.,
        })
        .collect()
}

///44ab20/44a4a0: a parking entrance on each eastbound road's north category.
///Use the same road/category links as the existing traffic graph constructor.
pub fn parking(board: &crate::board::Board) -> Result<Vec<Pose>, String> {
    let (roads, _) = board.traffic_roads()?;
    let mut result = Vec::new();
    for road in roads {
        if !matches!(u32::from_le_bytes(road[4..8].try_into().unwrap()), 1 | 3) {
            continue;
        }
        if let Some(block) = board
            .blocks
            .iter()
            .find(|b| b.id == usize::from(road[0x29]))
        {
            result.push(Pose {
                position: [
                    block.center[0],
                    0.,
                    (f64::from(block.center[1]) + 41.) as f32,
                ],
                yaw: 180.,
            });
        }
    }
    Ok(result)
}

///4b55d0 uses only the second half of the locator records. The first half
///contains duplicate positions without the facing vectors needed by the draw.
pub fn benches(mesh: &CachedMesh<'_>) -> Result<Vec<Pose>, String> {
    let records = mesh.opaque_records_28.as_chunks::<28>().0;
    records[records.len() / 2..]
        .iter()
        .map(|record| {
            let float = |at| f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
            let position = [float(0), float(4), -float(8)];
            let direction = [float(12), float(20)];
            if !position.into_iter().chain(direction).all(f32::is_finite) {
                return Err("non-finite street furniture locator".into());
            }
            Ok(Pose {
                position,
                yaw: (f64::from(direction[0]).atan2(f64::from(direction[1]))
                    * f64::from(57.29578_f32)) as f32,
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    EarlyLamp,
    LateLamp,
    Bench,
    Entrance,
    /// Junction traffic-light posts (0x44dbe0's model rows 0x687858/0x687868).
    EarlySignal,
    LateSignal,
}
impl Kind {
    pub fn visible(self, era: u32, detail: u8) -> bool {
        match self {
            Self::EarlyLamp => era < 2,
            // The native >=6 branch is unreachable after the >=2 test.
            Self::LateLamp => era >= 2,
            Self::Bench => detail != 0,
            Self::Entrance => true,
            Self::EarlySignal => {
                crate::traffic_lights::post_model(era) == crate::traffic_lights::EARLY_POST
            }
            Self::LateSignal => {
                crate::traffic_lights::post_model(era) == crate::traffic_lights::LATE_POST
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_sides_place_and_orient_original_post_bases() {
        let block = Block {
            id: 0,
            name: String::new(),
            text_id: 0,
            kind: 0,
            center: [100., -200.],
            size: [80.; 2],
            pavement_color: 0,
            flags: 0,
            street_sides: 1 | 4,
        };
        let poses = lamps(std::slice::from_ref(&block));
        let entrance = subways(std::slice::from_ref(&block));
        assert_eq!(entrance[0].position, [100., 0., -241.]);
        assert_eq!(entrance[0].yaw, 180.);
        assert_eq!(poses.len(), 4);
        assert_eq!(poses[0].position, [84., 0., -159.]);
        assert_eq!(poses[1].position, [116., 0., -159.]);
        assert_eq!(poses[2].position, [116., 0., -241.]);
        assert_eq!(poses[3].position, [84., 0., -241.]);
        assert_eq!(
            poses.iter().map(|p| p.yaw).collect::<Vec<_>>(),
            [270., 270., 90., 90.]
        );
    }
    #[test]
    fn ninety_degree_pose_follows_the_original_object_matrix() {
        // 0x4e3380 rows at yaw 90 are (0, 0, 1) and (-1, 0, 0); .tm vertices
        // reach them with Z negated. Local +X turns to world +Z, and local +Z
        // (model -Z) turns to world +X.
        let p = Pose {
            position: [10., 2., 20.],
            yaw: 90.,
        }
        .transform()
        .position([3., 4., 5.]);
        for (actual, expected) in p.into_iter().zip([15., 6., 23.]) {
            assert!((actual - expected).abs() < 0.00001, "{p:?}");
        }
    }
    #[test]
    fn lamp_heads_reach_the_original_glow_centres_on_every_side() {
        // 0x449360 stores each lamp's post base at +/-41 and its glow centre
        // at +/-38 from the block centre. The side-8 lamp (yaw 0) puts the head
        // 3 units along local +X; the rotation has to carry that offset onto
        // the glow centre on the other sides too.
        let block = Block {
            id: 0,
            name: String::new(),
            text_id: 0,
            kind: 0,
            center: [100., -200.],
            size: [80.; 2],
            pavement_color: 0,
            flags: 0,
            street_sides: 1 | 2 | 4 | 8,
        };
        let blocks = std::slice::from_ref(&block);
        let poses = lamps(blocks);
        let halos = crate::light_halos::lamps(blocks);
        assert_eq!(poses.len(), 8);
        for (pose, halo) in poses.iter().zip(&halos) {
            let head = pose.transform().position([3., halo.position[1], 0.]);
            for (actual, expected) in head.into_iter().zip(halo.position) {
                assert!((actual - expected).abs() < 1e-4, "yaw {}", pose.yaw);
            }
        }
    }
    #[test]
    fn synthetic_locators_use_second_half_and_facing_vector() {
        let mut bytes = vec![0; 164];
        bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&5_u32.to_le_bytes());
        for values in [
            [99., 99., 99., 0., 0., 0.],
            [88., 88., 88., 0., 0., 0.],
            [1., 2., 3., 0., 0., 1.],
            [4., 5., 6., 1., 0., 0.],
            [7., 8., 9., 0., 0., -1.],
        ] {
            for value in values {
                bytes.extend_from_slice(&f32::to_le_bytes(value));
            }
            bytes.extend_from_slice(&0_u32.to_le_bytes());
        }
        let mesh = CachedMesh::parse(&bytes).unwrap();
        let poses = benches(&mesh).unwrap();
        assert_eq!(poses.len(), 3);
        assert_eq!(poses[0].position, [1., 2., -3.]);
        assert_eq!(poses[1].position, [4., 5., -6.]);
        assert_eq!(
            poses
                .iter()
                .map(|p| p.yaw.round() as i32)
                .collect::<Vec<_>>(),
            [0, 90, 180]
        );
        assert!(Kind::EarlyLamp.visible(1, 0));
        assert!(!Kind::EarlyLamp.visible(2, 2));
        assert!(Kind::LateLamp.visible(6, 0));
        assert!(!Kind::Bench.visible(0, 0));
        assert!(Kind::Bench.visible(0, 1));
    }
}
