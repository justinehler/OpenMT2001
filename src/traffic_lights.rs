//! Junction traffic-light posts and their red/green glow points.
//!
//! City setup (0x44bd50) places one post per connected direction at every
//! junction with more than two road links. The normal city frame (0x44dbe0)
//! draws the era's post model at each one and adds two glow points chosen by
//! the same shared crossing masks the road vehicles obey (`traffic::Signals`).
use crate::{board::Board, street_furniture::Pose};

/// Model row 0x687858, drawn while the signed era is below 2.
pub const EARLY_POST: &str = "gamedata2/NEW_building stuff/trafficlight_small_1930.tm";
/// Model row 0x687868, drawn from era 2 on.
pub const LATE_POST: &str = "gamedata2/NEW_building stuff/trafficlight_small_red.tm";

/// Glow point radius and depth offset set on collector 0x8b22e0 by 0x44dbe0.
pub const GLOW_RADIUS: f32 = 2.;
pub const GLOW_DEPTH_OFFSET: f32 = -0.4;

/// 0x44dbe0 picks model row 0x687858 for signed era < 2, otherwise 0x687868.
/// This is not the 0/3/5 boundary used by the road-vehicle families.
pub fn post_model(era: u32) -> &'static str {
    if (era as i32) < 2 {
        EARLY_POST
    } else {
        LATE_POST
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Post {
    /// Stored post base (record +0..+8); Y is always zero.
    pub position: [f32; 3],
    /// Stored yaw in degrees (record +0xc): 0, 180, 270 or 90.
    pub yaw: f32,
    /// Direction bit tested against the crossing's green and amber masks.
    pub mask: u8,
    /// Crossing lane row (road index minus the plain segment count).
    pub crossing: usize,
    /// 0x44bc30: upper red, lower green, then the second red/green pair.
    pub lights: [[f32; 3]; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glow {
    Red,
    Green,
}
impl Glow {
    /// ARGB colour without alpha, as 0x44dbe0 adds it to the shifted alpha.
    pub fn rgb(self) -> u32 {
        match self {
            Self::Red => 0x00ff0000,
            Self::Green => 0x0000ff00,
        }
    }
}

impl Post {
    /// 0x44dbe0's per-post test of the shared crossing masks. Green wins;
    /// otherwise amber suppresses the points (the mesh is still drawn);
    /// otherwise the red points show.
    pub fn glow(&self, lanes: &[[u8; 32]]) -> Result<Option<Glow>, String> {
        let lane = lanes
            .get(self.crossing)
            .ok_or("traffic-light crossing outside lane table")?;
        let mask = u32::from(self.mask);
        let word =
            |at: usize| u32::from_le_bytes([lane[at], lane[at + 1], lane[at + 2], lane[at + 3]]);
        Ok(if word(0) & mask != 0 {
            Some(Glow::Green)
        } else if word(4) & mask != 0 {
            None
        } else {
            Some(Glow::Red)
        })
    }

    /// The two points 0x44dbe0 emits for a glow colour.
    pub fn glow_points(&self, glow: Glow) -> [[f32; 3]; 2] {
        match glow {
            Glow::Red => [self.lights[0], self.lights[2]],
            Glow::Green => [self.lights[1], self.lights[3]],
        }
    }

    /// Pose for the post mesh in the static scenery path. 0x4e0b50 passes
    /// the negated yaw to 0x4e3380, which negates it again, so the object
    /// rows are (cos, 0, sin) and (-sin, 0, cos) of the stored yaw (the same
    /// matrix as resident and vehicle descriptors). `Pose` rotates by the
    /// negated yaw, so it is given the negated stored yaw. This keeps the
    /// arm heads on the 0x44bc30 glow points at yaws 90 and 270.
    pub fn mesh_pose(&self) -> Pose {
        Pose {
            position: self.position,
            yaw: -self.yaw,
        }
    }

    /// Glow alpha from 0x44dbe0. The value it reads at descriptor +0x10c is
    /// element [2][2] of the composed model-view matrix (0x4e2fc0 after
    /// 0x4e0b50 rebuilds the yaw matrix with 0x4e3380): the third object row
    /// (-sin yaw, 0, cos yaw) dotted with the camera's depth column. It is
    /// stored as f32, scaled by 255.0f (0x525af0), stored as f32 again and
    /// converted with FISTP (round half to even). `view` is the original
    /// scene view from `camera::scene_view_with_up`. Points are only emitted
    /// when the result is positive.
    pub fn glow_alpha(&self, view: &[f32; 12]) -> i32 {
        let radians = (f64::from(self.yaw) * f64::from(0.017453292_f32)) as f32;
        let (sine, cosine) = f64::from(radians).sin_cos();
        let row = [-f64::from(sine as f32), 0., f64::from(cosine as f32)].map(|v| v as f32);
        let m22 = ((f64::from(row[0]) * f64::from(view[2])
            + f64::from(row[1]) * f64::from(view[5]))
            + f64::from(row[2]) * f64::from(view[8])) as f32;
        let scaled = (f64::from(m22) * 255.) as f32;
        let rounded = scaled.round_ties_even();
        if rounded.is_finite() && rounded.abs() < 2_147_483_648. {
            rounded as i32
        } else {
            // FISTP stores the integer indefinite value for out-of-range input.
            i32::MIN
        }
    }
}

/// 0x44bd50 over the board's road table, in native road order.
pub fn posts(board: &Board) -> Result<Vec<Post>, String> {
    let (roads, segment_count) = board.traffic_roads()?;
    posts_from_roads(&roads, segment_count)
}

/// 0x44bd50 over 44-byte road records: kind at +4, link mask at +2 and the
/// shifted centre at +0xc/+0x10. Only signed kind >= 11 with more than two
/// links gets posts, one per link in bit order 4, 1, 8, 2.
pub fn posts_from_roads(roads: &[[u8; 44]], segment_count: usize) -> Result<Vec<Post>, String> {
    let float = |road: &[u8; 44], at: usize| {
        f32::from_le_bytes([road[at], road[at + 1], road[at + 2], road[at + 3]])
    };
    let mut posts = Vec::new();
    for (index, road) in roads.iter().enumerate() {
        let kind = i32::from_le_bytes([road[4], road[5], road[6], road[7]]);
        let links = road[2] & 15;
        if kind < 11 || links.count_ones() <= 2 {
            continue;
        }
        let crossing = index
            .checked_sub(segment_count)
            .ok_or("traffic-light junction precedes the crossing table")?;
        let [x, z] = [float(road, 12), float(road, 16)];
        // Offsets use the 12.0f literal at 0x525084.
        for (link, dx, dz, yaw, mask) in [
            (4, -12., 12., 0., 1),
            (1, 12., -12., 180., 4),
            (8, 12., 12., 270., 2),
            (2, -12., -12., 90., 8),
        ] {
            if links & link == 0 {
                continue;
            }
            let position = [(f64::from(x) + dx) as f32, 0., (f64::from(z) + dz) as f32];
            posts.push(Post {
                position,
                yaw,
                mask,
                crossing,
                lights: light_points(position, yaw),
            });
        }
    }
    Ok(posts)
}

/// 0x44bc30: two red/green pairs rotated by the post's yaw. Cosine is stored
/// as binary32 while sine stays extended, as in the original x87 sequence.
fn light_points(position: [f32; 3], yaw: f32) -> [[f32; 3]; 4] {
    let radians = f64::from(yaw) * f64::from(0.017453292_f32);
    let cosine = f64::from(radians.cos() as f32);
    let sine = radians.sin();
    let [x, y, z] = position.map(f64::from);
    let drop = f64::from(1.95_f32);
    // First pair: offsets 15.92 / -1 at height 12.85.
    let (a, b) = (f64::from(15.92_f32), -1.);
    let offset_z = (sine * a + cosine * b) as f32;
    let first = [
        (cosine * a - sine * b + x) as f32,
        (y + f64::from(12.85_f32)) as f32,
        (f64::from(offset_z) + z) as f32,
    ];
    // Second pair: offsets 25.143 / -1.242 at height 5.135.
    let (c, d) = (f64::from(25.143_f32), f64::from(-1.242_f32));
    let offset_x = (cosine * c - sine * d) as f32;
    let second = [
        (f64::from(offset_x) + x) as f32,
        (y + f64::from(5.135_f32)) as f32,
        (sine * c + cosine * d + z) as f32,
    ];
    let lower = |p: [f32; 3]| [p[0], (f64::from(p[1]) - drop) as f32, p[2]];
    [first, lower(first), second, lower(second)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn road(kind: u32, links: u8, center: [f32; 2]) -> [u8; 44] {
        let mut row = [0; 44];
        row[2] = links;
        row[4..8].copy_from_slice(&kind.to_le_bytes());
        row[12..16].copy_from_slice(&center[0].to_le_bytes());
        row[16..20].copy_from_slice(&center[1].to_le_bytes());
        row
    }

    #[test]
    fn junction_posts_follow_link_order_offsets_and_yaws() {
        let roads = [
            road(1, 15, [0., 0.]),
            road(11, 15, [100., -200.]),
            road(12, 1 | 2 | 8, [40., 60.]),
            // Two links: a bend or a straight, no posts.
            road(11, 1 | 4, [0., 0.]),
            // Signed kind below 11 is not a junction.
            road(u32::MAX, 15, [0., 0.]),
        ];
        let posts = posts_from_roads(&roads, 1).unwrap();
        let summary: Vec<_> = posts
            .iter()
            .map(|p| (p.position, p.yaw, p.mask, p.crossing))
            .collect();
        assert_eq!(
            summary,
            [
                ([88., 0., -188.], 0., 1, 0),
                ([112., 0., -212.], 180., 4, 0),
                ([112., 0., -188.], 270., 2, 0),
                ([88., 0., -212.], 90., 8, 0),
                ([52., 0., 48.], 180., 4, 1),
                ([52., 0., 72.], 270., 2, 1),
                ([28., 0., 48.], 90., 8, 1),
            ]
        );
        assert!(posts_from_roads(&roads, 2).is_err());
    }

    #[test]
    fn light_points_rotate_two_pairs_with_the_post() {
        let posts = posts_from_roads(&[road(11, 15, [0., 0.])], 0).unwrap();
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4);
        let p = &posts[0];
        assert!(close(p.lights[0], [3.92, 12.85, 11.]));
        assert!(close(p.lights[1], [3.92, 10.9, 11.]));
        assert!(close(p.lights[2], [13.143, 5.135, 10.758]));
        assert!(close(p.lights[3], [13.143, 3.185, 10.758]));
        // Yaw 90 turns the arm from +X to +Z.
        let p = &posts[3];
        assert!(close(p.lights[0], [-11., 12.85, 3.92]));
        assert!(close(p.lights[2], [-10.758, 5.135, 13.143]));
    }

    #[test]
    fn post_mesh_arm_reaches_the_glow_points() {
        for post in posts_from_roads(&[road(11, 15, [0., 0.])], 0).unwrap() {
            // 0x44bc30 places the first pair at object point (15.92, 12.85, -1);
            // .tm vertices reach the object matrix with Z negated.
            let arm = post.mesh_pose().transform().position([15.92, 12.85, 1.]);
            for (actual, expected) in arm.into_iter().zip(post.lights[0]) {
                assert!((actual - expected).abs() < 1e-4, "yaw {}", post.yaw);
            }
        }
    }

    #[test]
    fn post_model_switches_at_era_two() {
        assert_eq!(post_model(0), EARLY_POST);
        assert_eq!(post_model(1), EARLY_POST);
        assert_eq!(post_model(2), LATE_POST);
        assert_eq!(post_model(5), LATE_POST);
    }

    #[test]
    fn glow_follows_the_green_and_amber_masks() {
        let posts = posts_from_roads(&[road(11, 15, [0., 0.])], 0).unwrap();
        let lane = |green: u32, amber: u32| {
            let mut row = [0; 32];
            row[..4].copy_from_slice(&green.to_le_bytes());
            row[4..8].copy_from_slice(&amber.to_le_bytes());
            row
        };
        let glows = |green, amber| {
            let lanes = [lane(green, amber)];
            posts
                .iter()
                .map(|p| (p.mask, p.glow(&lanes).unwrap()))
                .collect::<Vec<_>>()
        };
        use Glow::*;
        assert_eq!(
            glows(5, 0),
            [
                (1, Some(Green)),
                (4, Some(Green)),
                (2, Some(Red)),
                (8, Some(Red))
            ]
        );
        assert_eq!(
            glows(0, 5),
            [(1, None), (4, None), (2, Some(Red)), (8, Some(Red))]
        );
        assert_eq!(
            glows(10, 0),
            [
                (1, Some(Red)),
                (4, Some(Red)),
                (2, Some(Green)),
                (8, Some(Green))
            ]
        );
        // Green is tested first, so it wins over a simultaneous amber bit.
        assert_eq!(glows(5, 5)[0], (1, Some(Green)));
        let p = &posts[0];
        assert_eq!(p.glow_points(Red), [p.lights[0], p.lights[2]]);
        assert_eq!(p.glow_points(Green), [p.lights[1], p.lights[3]]);
        assert_eq!((Red.rgb(), Green.rgb()), (0xff0000, 0xff00));
        assert!(p.glow(&[]).is_err());
    }

    #[test]
    #[ignore = "requires the original v1.4 executable"]
    fn original_signal_cycle_drives_the_glow_at_each_counter_position() {
        let executable = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = crate::pe::PeImage::parse(&executable).unwrap();
        let posts = posts_from_roads(&[road(11, 15, [0., 0.])], 0).unwrap();
        use Glow::*;
        for (counter, expected) in [
            (0_u32, [Some(Green), Some(Green), Some(Red), Some(Red)]),
            (174, [Some(Green), Some(Green), Some(Red), Some(Red)]),
            // A amber: the 1|4 posts lose their points, 2|8 stay red.
            (175, [None, None, Some(Red), Some(Red)]),
            (200, [Some(Red), Some(Red), Some(Green), Some(Green)]),
            // B amber.
            (375, [Some(Red), Some(Red), None, None]),
        ] {
            // advance() increments first, so start one before the position.
            let mut signals =
                crate::traffic::Signals::from_v14(&image, counter.wrapping_sub(1)).unwrap();
            let mut lanes = vec![[0; 32]];
            signals.advance(&mut lanes);
            assert_eq!(signals.counter, counter);
            let glows: Vec<_> = posts.iter().map(|p| p.glow(&lanes).unwrap()).collect();
            assert_eq!(glows, expected, "counter {counter}");
        }
    }

    #[test]
    fn glow_alpha_is_the_rounded_view_depth_of_the_post_axis() {
        let posts = posts_from_roads(&[road(11, 15, [0., 0.])], 0).unwrap();
        let view = |depth: [f32; 3]| {
            let mut view = [0.; 12];
            view[2] = depth[0];
            view[5] = depth[1];
            view[8] = depth[2];
            view
        };
        // Camera looking straight along +Z: only the yaw-0 post faces it.
        let alphas: Vec<_> = posts
            .iter()
            .map(|p| p.glow_alpha(&view([0., 0., 1.])))
            .collect();
        assert_eq!(alphas, [255, -255, 0, 0]);
        // 0.5 * 255 = 127.5 rounds half to even; 63.75 rounds to 64.
        assert_eq!(posts[0].glow_alpha(&view([0., 0., 0.5])), 128);
        assert_eq!(posts[0].glow_alpha(&view([0., 0., 0.25])), 64);
        // Yaw 270's third object row is (+1, 0, 0); yaw 90's is (-1, 0, 0).
        assert_eq!(posts[2].glow_alpha(&view([1., 0., 0.])), 255);
        assert_eq!(posts[3].glow_alpha(&view([-1., 0., 0.])), 255);
        // The camera's real depth column comes from the original builder.
        let scene = crate::camera::scene_view([0., 100., 0.], [0., 0., 100.]);
        assert_eq!(posts[0].glow_alpha(&scene), 180);
    }
}
