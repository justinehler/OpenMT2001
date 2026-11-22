//! Original CPU visibility and composed box picking. Host object eligibility
//! stays separate. The fourth projected component is reciprocal depth.

/// Original CPU screen projection set by004e34f0 from the renderer FOV.
pub struct QueryProjection {
    center: [f32; 2],
    scale: [f32; 2],
    cot: [f32; 2],
    sphere_scale: [f32; 2],
    near: f32,
    far: f32,
}

pub struct QueryBox {
    pub id: u32,
    pub bounds: [[f32; 3]; 8],
    pub center: [f32; 3],
    pub radius: f32,
}

pub struct QueryCategory {
    pub bounds: QueryBox,
    pub visible: bool,
    pub buildings: Vec<QueryBox>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryHit {
    pub category: usize,
    pub building: Option<usize>,
}

/// Normal inspection branch004c2d18: selected category hits do not constrain
/// the other categories' distance. Its building query runs last, even on miss.
pub fn hover(
    projection: &QueryProjection,
    categories: &[QueryCategory],
    selected: Option<usize>,
    mouse: [i32; 2],
) -> Result<Option<QueryHit>, String> {
    let mut nearest = f32::from_bits(0x497423f0);
    let mut category = None;
    let mut any_hit = false;
    for (i, item) in categories.iter().enumerate().filter(|(_, c)| c.visible) {
        let old_distance = nearest;
        let b = &item.bounds;
        if projection.query(&b.bounds, b.center, b.radius, mouse, &mut nearest)? {
            any_hit = true;
            if Some(i) == selected {
                nearest = old_distance;
            } else {
                category = Some(i);
            }
        }
    }
    if category.is_none() && any_hit {
        category = selected;
    }
    let mut building = None;
    for index in [category, selected].into_iter().flatten() {
        let item = &categories[index];
        if !item.visible {
            continue;
        }
        building = nearest_building(projection, &item.buildings, mouse, None)?;
        if building.is_some() {
            category = Some(index);
        }
    }
    Ok(category.map(|category| QueryHit { category, building }))
}

/// Original004c3020 walks category insertion order, optionally restricting the
/// runtime ID. Unlike category drawing, it does not repeat a visibility test.
pub fn nearest_building(
    projection: &QueryProjection,
    buildings: &[QueryBox],
    mouse: [i32; 2],
    only_id: Option<u32>,
) -> Result<Option<usize>, String> {
    let mut nearest = f32::from_bits(0x47c34f80);
    let mut selected = None;
    for (index, building) in buildings.iter().enumerate() {
        if only_id.is_some_and(|id| id != building.id) {
            continue;
        }
        if projection.query(
            &building.bounds,
            building.center,
            building.radius,
            mouse,
            &mut nearest,
        )? {
            selected = Some(index);
        }
    }
    Ok(selected)
}
impl QueryProjection {
    pub fn new(width: f32, height: f32, fov: f32, near: f32, far: f32) -> Result<Self, String> {
        if ![width, height, fov, near, far]
            .iter()
            .all(|v| v.is_finite())
            || width <= 0.
            || height <= 0.
            || !(0. < fov && fov < 180.)
            || !(0. < near && near < far)
        {
            return Err("unsupported picking projection".into());
        }
        let tangent = (f64::from(fov) * 0.5 * f64::from(0.017453292f32)).tan();
        let horizontal = f64::from(width) / f64::from(height) * tangent;
        let cot = [(1. / horizontal) as f32, (1. / tangent) as f32];
        let center = [width * 0.5, height * 0.5];
        let projection = Self {
            center,
            scale: [
                (f64::from(center[0]) * f64::from(cot[0])) as f32,
                (-f64::from(center[1]) * f64::from(cot[1])) as f32,
            ],
            cot,
            sphere_scale: [
                (1. / horizontal.atan().sin()) as f32,
                (1. / tangent.atan().sin()) as f32,
            ],
            near,
            far,
        };
        if projection
            .scale
            .iter()
            .chain(&projection.cot)
            .chain(&projection.sphere_scale)
            .any(|v| !v.is_finite())
        {
            return Err("picking projection exceeds supported float range".into());
        }
        Ok(projection)
    }
    /// Original4e3680 screen-coordinate projection, also used by the planning grid.
    pub fn project_coordinates(&self, point: [f32; 3]) -> [f32; 2] {
        let projected = self.project(point, true);
        [projected[0], projected[1]]
    }

    fn project(&self, point: [f32; 3], coordinates_only: bool) -> [f32; 4] {
        // Both projection helpers use fixed Z1, separate from the view clip.
        let close = (f64::from(point[2]) - 1.) as f32;
        let behind = close.to_bits() & 0x8000_0000 != 0;
        let reciprocal = if behind { 1. } else { 1. / f64::from(point[2]) };
        let scale = if behind && coordinates_only {
            [self.scale[1], self.scale[0]]
        } else {
            self.scale
        };
        [
            (reciprocal * f64::from(scale[0]) * f64::from(point[0]) + f64::from(self.center[0]))
                as f32,
            (reciprocal * f64::from(scale[1]) * f64::from(point[1]) + f64::from(self.center[1]))
                as f32,
            point[2],
            reciprocal as f32,
        ]
    }
    /// Visibility decision of004e07d0/004e0910 on already transformed bounds.
    pub fn visible(&self, bounds: &[[f32; 3]; 8], center: [f32; 3], radius: f32) -> bool {
        let [x, y, z] = center.map(f64::from);
        let r = f64::from(radius);
        if z + r < f64::from(self.near) || z - r > f64::from(self.far) {
            return false;
        }
        let xy = [x, y];
        let mut needs_corners = z - r < f64::from(self.near);
        for (i, coordinate) in xy.into_iter().enumerate() {
            let extent = (f64::from(self.sphere_scale[i]) * r) as f32;
            let distance = ((coordinate * f64::from(self.cot[i])) as f32).abs();
            if f64::from(distance) - f64::from(extent) >= z {
                return false;
            }
            needs_corners |= f64::from(distance) + f64::from(extent) >= z;
        }
        if !needs_corners {
            return true;
        }
        for plane in 0..6 {
            if bounds.iter().all(|p| match plane {
                0 => f64::from(p[0]) * f64::from(self.cot[0]) < -f64::from(p[2]),
                1 => f64::from(p[0]) * f64::from(self.cot[0]) > f64::from(p[2]),
                2 => f64::from(p[1]) * f64::from(self.cot[1]) < -f64::from(p[2]),
                3 => f64::from(p[1]) * f64::from(self.cot[1]) > f64::from(p[2]),
                4 => p[2] < self.near,
                _ => p[2] > self.far,
            }) {
                return false;
            }
        }
        true
    }
    /// Original005000f0: square radius gate, box faces, strict nearest update.
    pub fn query(
        &self,
        bounds: &[[f32; 3]; 8],
        center: [f32; 3],
        radius: f32,
        mouse: [i32; 2],
        nearest: &mut f32,
    ) -> Result<bool, String> {
        if bounds
            .iter()
            .flatten()
            .chain(center.iter())
            .any(|v| !v.is_finite())
            || !radius.is_finite()
            || radius < 0.
            || !nearest.is_finite()
        {
            return Err("unsupported picking bounds".into());
        }
        let projected = self.project(center, true);
        let mut edge = center;
        edge[0] = (f64::from(edge[0]) + f64::from(radius)) as f32;
        let projected_edge = self.project(edge, true);
        let radius = (f64::from(projected[0]) - f64::from(projected_edge[0])).abs();
        if radius >= 2_147_483_648. {
            return Err("unsupported projected radius".into());
        }
        let radius = radius.trunc();
        if (0..2).any(|i| {
            f64::from(((f64::from(projected[i]) - f64::from(mouse[i])) as f32).abs()) > radius
        }) {
            return Ok(false);
        }
        let vertices = bounds.map(|p| self.project(p, false));
        if let Some(distance) = projected_box(&vertices, mouse)?
            && distance != 0.
            && distance < f64::from(*nearest)
        {
            *nearest = distance as f32;
            return Ok(true);
        }
        Ok(false)
    }
}

/// Face order embedded at0055c724, shared by original bounding-box operations.
pub const FACES: [[usize; 4]; 6] = [
    [0, 1, 3, 2],
    [5, 4, 6, 7],
    [1, 5, 7, 3],
    [4, 0, 2, 6],
    [2, 3, 7, 6],
    [0, 4, 5, 1],
];

fn edge_x(base: i32, value: f64) -> Result<i32, String> {
    if !value.is_finite() || value < f64::from(i32::MIN) || value >= 2_147_483_648. {
        return Err("unsupported picking edge conversion".into());
    }
    base.checked_add(value.trunc() as i32)
        .ok_or_else(|| "picking edge exceeds supported range".into())
}

pub fn projected_box(vertices: &[[f32; 4]; 8], mouse: [i32; 2]) -> Result<Option<f64>, String> {
    // The original assumes screen coordinates supported by FISTP/FTOL.
    if vertices.iter().flatten().any(|x| !x.is_finite())
        || vertices
            .iter()
            .any(|v| v[0].abs() > 1_000_000. || v[1].abs() > 1_000_000.)
        || mouse.iter().any(|v| v.unsigned_abs() > 1_000_000)
    {
        return Err("unsupported projected picking coordinates".into());
    }
    for face in FACES {
        let p = face.map(|i| vertices[i].map(f64::from));
        let a = (p[0][0] - p[1][0]) * (p[0][1] - p[2][1]);
        let b = (p[0][1] - p[1][1]) * (p[0][0] - p[2][0]);
        if b <= a {
            continue;
        }
        let rounded = p.map(|v| [v[0].round_ties_even() as i32, v[1].round_ties_even() as i32]);
        let mut left = 999_999_999i32;
        let mut right = -999_999_999i32;
        let mut left_w = 0f32;
        let mut right_w = 0f32;
        for i in 0..4 {
            let j = (i + 1) & 3;
            let yi = rounded[i][1];
            let yj = rounded[j][1];
            if yi < yj {
                if mouse[1] < yi || mouse[1] > yj {
                    continue;
                }
                let dy = p[j][1] - p[i][1];
                let inverse = (1. / dy) as f32;
                if dy == 0. {
                    continue;
                }
                let offset = f64::from(mouse[1] - yi);
                left = edge_x(
                    rounded[i][0],
                    (p[j][0] - p[i][0]) * offset * f64::from(inverse),
                )?;
                left_w = ((p[j][3] - p[i][3]) * offset * f64::from(inverse) + p[i][3]) as f32;
            } else {
                if mouse[1] < yj || mouse[1] > yi {
                    continue;
                }
                let dy = p[i][1] - p[j][1];
                let inverse = (1. / dy) as f32;
                if dy == 0. {
                    continue;
                }
                right = edge_x(
                    rounded[j][0],
                    (p[i][0] - p[j][0]) * f64::from(mouse[1] - yj) * f64::from(inverse),
                )?;
                right_w = ((p[i][3] - p[j][3]) * f64::from(mouse[1] - yi) * f64::from(inverse)
                    + p[i][3]) as f32;
            }
            if left < right {
                if mouse[0] >= left && mouse[0] <= right {
                    let span = right.checked_sub(left).ok_or("unsupported picking span")?;
                    let offset = mouse[0]
                        .checked_sub(left)
                        .ok_or("unsupported picking offset")?;
                    let fraction = f64::from(offset) / f64::from(span);
                    return Ok(Some(
                        1. / ((f64::from(right_w) - f64::from(left_w)) * fraction
                            + f64::from(left_w)),
                    ));
                }
                break;
            }
        }
    }
    Ok(None)
}

/// Original00447a90, after caller44605f chooses max(10, model+b4).
pub fn category_bounds(
    model_heights: impl IntoIterator<Item = f32>,
) -> Result<[[f32; 3]; 8], String> {
    let mut height = 10f32;
    for value in model_heights {
        if !value.is_finite() || value.abs() > 1_000_000. {
            return Err("unsupported category model height".into());
        }
        if value > height {
            height = value;
        }
    }
    Ok(std::array::from_fn(|i| {
        [
            if i & 1 == 0 { -41. } else { 41. },
            if i & 2 == 0 { 0. } else { height },
            if i & 4 == 0 { -41. } else { 41. },
        ]
    }))
}

/// One corner object emitted by original 004b4f40 for an unrotated block.
#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub color: u32,
}

/// Original corner placement and pulse. `frame` is the original animation
/// counter; scheduling it belongs to the host. Rotated source objects are not
/// covered by this block-specific path.
pub fn block_markers(
    bounds: &[[f32; 3]; 8],
    translation: [f32; 3],
    color: u32,
    selected: bool,
    frame: u32,
) -> Result<[Marker; 8], String> {
    if bounds
        .iter()
        .flatten()
        .chain(translation.iter())
        .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
    {
        return Err("unsupported marker coordinates".into());
    }
    let phase = ((frame & 31) as i32 - 15).unsigned_abs();
    let expansion = if selected {
        f64::from(phase + 256) / 256.
    } else {
        1.
    };
    let mut minimum = 100_000_000f32;
    for (corner, axis) in [(1, 0), (2, 1), (4, 2)] {
        let extent = (f64::from(bounds[corner][axis]) - f64::from(bounds[0][axis])).abs();
        if extent < f64::from(minimum) {
            minimum = extent as f32;
        }
    }
    let mut scale = f64::from(minimum) * f64::from(0.333_333_34f32);
    if scale > 3. || (f64::from(0.24f32)..=f64::from(0.26f32)).contains(&scale) {
        scale = 3.;
    }
    let color = if selected {
        let factor = 0.5 + f64::from(phase) / 32.;
        [0, 8, 16]
            .into_iter()
            .fold(color & 0xff00_0000, |result, shift| {
                result | (((f64::from((color >> shift) & 255) * factor).trunc() as u32) << shift)
            })
    } else {
        color
    };
    let rotations = [
        [0., 90., 0.],
        [0., 180., 0.],
        [180., 0., 0.],
        [180., 90., 0.],
        [0., 0., 0.],
        [0., -90., 0.],
        [-180., -90., 0.],
        [-180., -180., 0.],
    ];
    Ok(std::array::from_fn(|i| Marker {
        position: std::array::from_fn(|j| {
            ((f64::from(bounds[i][j]) * expansion) as f32) + translation[j]
        }),
        rotation: rotations[i],
        scale: [scale as f32; 3],
        color,
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original normal hover branch observations"]
    fn matches_native_hover_precedence() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_HOVER").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTHOVER1");
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let projection = super::QueryProjection::new(640., 480., 45., 4., 3000.).unwrap();
        assert_eq!(bytes.len(), 12 + word(8) as usize * 484);
        for n in 0..word(8) as usize {
            let base = 12 + n * 484;
            let selected = word(base);
            let categories = (0..2)
                .map(|i| {
                    let offset = base + 20 + i * 232;
                    let query = |offset, id| {
                        let f = |i| f32::from_bits(word(offset + i * 4));
                        super::QueryBox {
                            id,
                            bounds: std::array::from_fn(|i| std::array::from_fn(|j| f(i * 3 + j))),
                            center: [f(24), f(25), f(26)],
                            radius: f(27),
                        }
                    };
                    super::QueryCategory {
                        visible: word(offset) != 0,
                        bounds: query(offset + 4, i as u32),
                        buildings: vec![query(offset + 120, word(offset + 116))],
                    }
                })
                .collect::<Vec<_>>();
            let actual = super::hover(
                &projection,
                &categories,
                (selected != u32::MAX).then_some(selected as usize),
                [word(base + 4) as i32, word(base + 8) as i32],
            )
            .unwrap();
            assert_eq!(
                actual.map(|h| h.category as i32).unwrap_or(-1),
                word(base + 12) as i32,
                "category case {n}"
            );
            assert_eq!(
                actual
                    .filter(|h| h.building.is_some())
                    .map(|h| h.category as i32)
                    .unwrap_or(-1),
                word(base + 16) as i32,
                "building case {n}"
            );
        }
    }
    #[test]
    #[ignore = "requires original individual building query observations"]
    fn matches_native_building_queries() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_BUILDING_PICK").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTBQUERY");
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let projection = super::QueryProjection::new(640., 480., 45., 4., 3000.).unwrap();
        let mut offset = 12;
        for n in 0..word(8) {
            let count = word(offset) as usize;
            let filter = word(offset + 4);
            let mouse = [word(offset + 8) as i32, word(offset + 12) as i32];
            let expected = word(offset + 16) as i32;
            offset += 20;
            let mut buildings = Vec::new();
            for _ in 0..count {
                let f = |i| f32::from_bits(word(offset + 4 + i * 4));
                buildings.push(super::QueryBox {
                    id: word(offset),
                    bounds: std::array::from_fn(|i| std::array::from_fn(|j| f(i * 3 + j))),
                    center: [f(24), f(25), f(26)],
                    radius: f(27),
                });
                offset += 116;
            }
            let actual = super::nearest_building(
                &projection,
                &buildings,
                mouse,
                (filter != u32::MAX).then_some(filter),
            )
            .unwrap();
            assert_eq!(actual.map(|i| i as i32).unwrap_or(-1), expected, "case {n}");
        }
        assert_eq!(offset, bytes.len());
    }
    #[test]
    #[ignore = "requires original composed category query observations"]
    fn matches_native_category_queries() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CATEGORY_PICK").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTCQUERY");
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(bytes.len(), 12 + count * 136);
        let projection = super::QueryProjection::new(640., 480., 45., 4., 3000.).unwrap();
        for n in 0..count {
            let base = 12 + n * 136;
            let f = |i| f32::from_bits(word(base + i * 4));
            let bounds = std::array::from_fn(|i| std::array::from_fn(|j| f(i * 3 + j)));
            let center = [f(24), f(25), f(26)];
            let radius = f(27);
            let visible = projection.visible(&bounds, center, radius);
            assert_eq!(visible, word(base + 124) != 0, "visibility case {n}");
            let mut nearest = f(30);
            let hit = visible
                && projection
                    .query(
                        &bounds,
                        center,
                        radius,
                        [word(base + 112) as i32, word(base + 116) as i32],
                        &mut nearest,
                    )
                    .unwrap();
            assert_eq!(hit, word(base + 128) != 0, "hit case {n}");
            assert_eq!(nearest.to_bits(), word(base + 132), "distance case {n}");
        }
    }
    #[test]
    #[ignore = "requires original category bounds observations"]
    fn matches_native_category_bounds() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CATEGORY_BOUNDS").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTCBND01");
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(bytes.len(), 12 + count * 112);
        for n in 0..count {
            let base = 12 + n * 112;
            let heights = (0..word(base) as usize).map(|i| f32::from_bits(word(base + 4 + i * 4)));
            let bounds = super::category_bounds(heights).unwrap();
            for (i, value) in bounds.iter().flatten().enumerate() {
                assert_eq!(
                    value.to_bits(),
                    word(base + 16 + i * 4),
                    "case {n}, component {i}"
                );
            }
        }
    }
    #[test]
    #[ignore = "requires original selection-marker emissions"]
    fn matches_native_selection_markers() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_SELECTION_MARKER").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTMARK01");
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(bytes.len(), 12 + count * 428);
        for n in 0..count {
            let base = 12 + n * 428;
            let bounds = std::array::from_fn(|i| {
                std::array::from_fn(|j| f32::from_bits(word(base + (i * 3 + j) * 4)))
            });
            let markers = super::block_markers(
                &bounds,
                [125., 0., -205.],
                word(base + 96),
                word(base + 100) != 0,
                word(base + 104),
            )
            .unwrap();
            for (i, marker) in markers.iter().enumerate() {
                for (j, value) in marker
                    .position
                    .iter()
                    .chain(&marker.rotation)
                    .chain(&marker.scale)
                    .enumerate()
                {
                    assert_eq!(
                        value.to_bits(),
                        word(base + 108 + i * 40 + j * 4),
                        "case {n}, marker {i}, component {j}"
                    );
                }
                assert_eq!(
                    marker.color,
                    word(base + 108 + i * 40 + 36),
                    "case {n}, marker {i}"
                );
            }
        }
    }
    #[test]
    #[ignore = "requires native projected-box observations"]
    fn matches_native_projected_pick() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PROJECTED_PICK").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTPICK01");
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(bytes.len(), 12 + count * 144);
        for n in 0..count {
            let base = 12 + n * 144;
            let vertices = std::array::from_fn(|i| {
                std::array::from_fn(|j| f32::from_bits(word(base + (i * 4 + j) * 4)))
            });
            let mouse = [word(base + 128) as i32, word(base + 132) as i32];
            let expected = f64::from_le_bytes(bytes[base + 136..base + 144].try_into().unwrap());
            let actual = super::projected_box(&vertices, mouse)
                .unwrap()
                .unwrap_or(0.);
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "case {n}: {actual} vs {expected}"
            );
        }
    }
}
