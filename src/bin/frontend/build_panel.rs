//! Original424abe construction form ->48ac20 producer inputs.
//! Cash/occupancy predicates come from shared runtime; this adapter owns no world.
/// 422af2..422b1f resolves the selected property's color and calls 48a770
/// before adding special Hotel catalog row 43 to the residential list.
pub fn special_residential_group(
    image: &openmt2001::pe::PeImage<'_>,
    category_identity: u32,
) -> Result<[i32; 4], String> {
    let address = category_identity
        .checked_mul(88)
        .and_then(|offset| offset.checked_add(0x5385a4))
        .ok_or("planning special residential group address overflow")?;
    let color = u32::from(image.at(address, 1)?[0]);
    openmt2001::ai_strategy_queries::group(image, color)
}

/// The live roster maps each nonzero owner byte to one player reference;
/// owner zero maps to the null reference used by 48a770 for city ownership.
pub fn special_residential_group_owned(
    members: &[i32; 4],
    landlord: u8,
    owners: &[u8; 52],
) -> Result<bool, String> {
    openmt2001::ai_strategy_queries::owns_group(u32::from(landlord), members, |id| {
        let index = usize::try_from(id).map_err(|_| "planning special residential group sentinel")?;
        owners
            .get(index)
            .map(|&owner| u32::from(owner))
            .ok_or("planning special residential group member outside city".into())
    })
}
/// Read-only worker snapshot. Raw retained records preserve the distinction
/// between actual scenario settings and executable defaults.
#[derive(Clone)]
pub struct Context {
    pub category: usize,
    pub category_record: [u8; 0x17c],
    pub catalog: openmt2001::business_pool::BusinessCatalog,
    pub products: [[[u32; 2]; 6]; 45],
    pub commodities: [[u8; 0x1c]; 32],
    pub current_owner: u8,
    pub category_owners: [u8; 52],
    pub players: Vec<Vec<u8>>,
    pub era: u32,
}
impl Context {
    pub fn from_session(
        session: &crate::fresh_city::Session<'_>,
        category: usize,
    ) -> Result<Self, String> {
        let player = session
            .roster
            .manager
            .players
            .iter()
            .find(|p| p.reference == session.roster.status.current_player)
            .ok_or("planning current player missing")?;
        Ok(Self {
            category,
            category_record: *session
                .world
                .categories
                .get(category)
                .ok_or("planning category outside city")?,
            catalog: session.world.catalog.info.clone(),
            products: session.world.catalog.products,
            commodities: session.world.commodities.rows,
            current_owner: player.bytes[0x1bc].wrapping_add(1),
            category_owners: session.world.categories.each_ref().map(|row| row[5]),
            players: session
                .roster
                .manager
                .players
                .iter()
                .map(|p| p.bytes.to_vec())
                .collect(),
            era: session.world.clock.calendar[2],
        })
    }
    pub fn entry(&self) -> Entry {
        entry(
            self.category_record[0x6d],
            self.category_record[5] == self.current_owner,
            u32::from_le_bytes(self.category_record[0x78..0x7c].try_into().unwrap()),
        )
    }
    /// Original commodity rows at PE 0x53be98 carry the localized full
    /// text ID at +4. The +8 ID is the shorter poll/UI label.
    pub fn provides_text_ids(&self, business: u32) -> Result<Vec<u32>, String> {
        let row = self
            .catalog
            .row(business as usize)
            .ok_or("planning business outside catalog")?;
        let count = usize::from(row[0x4c]);
        if count > 6 {
            return Err("planning product count exceeds original six slots".into());
        }
        self.products[business as usize][..count]
            .iter()
            .map(|product| {
                let id = product[0];
                let commodity = self
                    .commodities
                    .get(id as usize)
                    .ok_or("planning product leaves commodity table")?;
                if u32::from_le_bytes(commodity[..4].try_into().unwrap()) != id {
                    return Err("planning commodity identity mismatch".into());
                }
                Ok(u32::from_le_bytes(commodity[4..8].try_into().unwrap()))
            })
            .collect()
    }
}

/// Original4c3150..4c3244 grid construction from category view-space bounds.
/// Preserve the intermediate stores before4e3680 projects these64 points.
pub fn grid_points(bounds: &[[f32; 3]; 8]) -> Result<[[f32; 3]; 64], String> {
    use openmt2001::x87::Sum;
    let mut u = [Sum::from_i32(0); 3];
    let mut v = u;
    let mut offset = [0.; 3];
    for axis in 0..3 {
        u[axis] = Sum::from_f32(bounds[4][axis])?;
        u[axis].add(Sum::from_f32(bounds[0][axis])?.negated());
        u[axis].mul_f32(f32::from_bits(0x3df9c190))?;
        v[axis] = Sum::from_f32(bounds[1][axis])?;
        v[axis].add(Sum::from_f32(bounds[0][axis])?.negated());
        v[axis].mul_f32(f32::from_bits(0x3df9c190))?;
        if axis == 2 {
            v[axis] = Sum::from_f32(v[axis].to_f32())?;
        }
        let mut sum = u[axis];
        sum.add(v[axis]);
        sum.mul_f32(0.6)?;
        offset[axis] = sum.to_f32();
    }
    let mut grid = [[0.; 3]; 64];
    for row in 0..8 {
        let mut row_offset = v;
        for (axis, value) in row_offset.iter_mut().enumerate() {
            value.mul_f32(row as f32)?;
            if axis != 2 {
                *value = Sum::from_f32(value.to_f32())?;
            }
        }
        for col in 0..8 {
            for axis in 0..3 {
                let mut sum = u[axis];
                sum.mul_f32(col as f32)?;
                if axis == 1 {
                    sum.add_f32(bounds[0][axis])?;
                }
                sum.add(row_offset[axis]);
                if axis == 0 {
                    sum.add_f32(bounds[0][axis])?;
                }
                sum.add_f32(offset[axis])?;
                if axis == 2 {
                    sum.add_f32(bounds[0][axis])?;
                }
                grid[row * 8 + col][axis] = sum.to_f32();
            }
        }
    }
    Ok(grid)
}

/// Original4c325e..4c329f: averaging happens after projection, preserving
/// perspective distortion. Input is the8x8 projected grid in row-major order.
pub fn cell_centers(grid: &[[f32; 2]; 64]) -> [[f32; 2]; 49] {
    std::array::from_fn(|i| {
        let x = i / 7;
        let y = i % 7;
        std::array::from_fn(|axis| {
            ((f64::from(grid[x * 8 + y][axis]) + f64::from(grid[(x + 1) * 8 + y + 1][axis])) * 0.5)
                as f32
        })
    })
}

/// Original4c36bd nearest projected cell. Inputs are the49 averages produced
/// by4c3130 from opposite projected grid corners, in original cell order.
pub fn nearest_cell(centers: &[[f32; 2]; 49], mouse: [i32; 2]) -> Result<Option<u8>, String> {
    let mut best = 999_999i32;
    let mut selected = None;
    for (i, center) in centers.iter().enumerate() {
        let x = openmt2001::x87::Sum::from_f32(center[0])
            .map_err(str::to_owned)?
            .ftol_low_word();
        let y = openmt2001::x87::Sum::from_f32(center[1])
            .map_err(str::to_owned)?
            .ftol_low_word();
        let distance = mouse[0]
            .wrapping_sub(x)
            .wrapping_abs()
            .wrapping_add(mouse[1].wrapping_sub(y).wrapping_abs());
        if distance < best {
            best = distance;
            selected = Some(i as u8);
        }
    }
    Ok(selected)
}

/// Original404eb0 planning entry, separate from per-request placement checks.
/// `rights` is the category +78 dword; `restricted` is the +6d byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Open,
    Message { text: u32, show_rights_action: bool },
}
pub fn entry(restricted: u8, is_landlord: bool, rights: u32) -> Entry {
    let inspected=openmt2001::property_rights::inspect_values(restricted,is_landlord,rights,0);
    match inspected.message {
        Some(text)=>Entry::Message{text,show_rights_action:inspected.activate_visible},
        None=>Entry::Open,
    }
}

#[derive(Clone, Debug)]
pub struct Form {
    pub category: i32,
    pub rectangle: [i32; 4],
    pub height: i32,
    pub quality: usize,
    pub choices: [i32; 3],
    pub variants: Vec<u32>,
    pub kind: u32,
    pub business: u32,
    pub displayed_quote: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub category: i32,
    pub origin: [i32; 2],
    pub size: [i32; 2],
    pub height: i32,
    pub packed_model: u32,
    pub kind: u32,
    pub quality: u8,
    pub displayed_quote: f32,
    pub business: u32,
}
/// Original424010 successful footprint completion, before tutorial event88.
/// Bank lengths are ordered low/medium/high (86ce54/86ce4c/86ce50).
/// The inclusive choice can equal a bank length:424221 interprets that as
/// model variant1/style0, rather than clamping to the last texture.
pub fn placement_choices(state: &mut u32, counts: [usize; 3]) -> Result<[i32; 3], String> {
    let limits = counts.map(|count| i32::try_from(count).map(|v| v.wrapping_add(1)));
    let mut divisors = [0; 3];
    for (divisor, limit) in divisors.iter_mut().zip(limits) {
        *divisor = limit.map_err(|_| "original build style count outside signed word")?;
        if *divisor == 0 {
            return Err("original build style count produces zero divisor".into());
        }
    }
    Ok(divisors.map(|divisor| {
        *state = state.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        (((*state >> 16) & 0x7fff) as i32) % divisor
    }))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocked {
    CategoryLocked,
    Cash,
    NoPreview,
}
impl Form {
    pub fn request(
        &self,
        category_locked: bool,
        cash_allowed: bool,
        preview_present: bool,
    ) -> Result<Result<Request, Blocked>, String> {
        // Exact424a4f/424a84/424a92 guard order; quote calculation is separate.
        if category_locked {
            return Ok(Err(Blocked::CategoryLocked));
        }
        if !cash_allowed {
            return Ok(Err(Blocked::Cash));
        }
        if !preview_present {
            return Ok(Err(Blocked::NoPreview));
        }
        let choice = *self
            .choices
            .get(self.quality)
            .ok_or("construction quality outside original tabs")?;
        let columns =
            i32::try_from(self.variants.len()).map_err(|_| "construction variant count")?;
        if columns == 0 || choice < 0 {
            return Err("construction selection outside populated variant grid".into());
        }
        let model = self.variants[(choice % columns) as usize]
            .wrapping_add(((choice / columns) as u32).wrapping_shl(16));
        let [x0, y0, x1, y1] = self.rectangle;
        Ok(Ok(Request {
            category: self.category,
            origin: [x0, y0],
            size: [
                x1.wrapping_sub(x0).wrapping_add(1),
                y1.wrapping_sub(y0).wrapping_add(1),
            ],
            height: self.height,
            packed_model: model,
            kind: self.kind,
            quality: self.quality as u8,
            displayed_quote: self.displayed_quote,
            business: self.business,
        }))
    }
}
/// Original423e0b normalizes the two selected seven-by-seven lot cells.
pub fn rectangle_from_cells(first: u8, last: u8) -> Result<[i32; 4], String> {
    if first >= 49 || last >= 49 {
        return Err("construction lot cell outside original7x7 block".into());
    }
    let a = [i32::from(first / 7), i32::from(first % 7)];
    let b = [i32::from(last / 7), i32::from(last % 7)];
    Ok([
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[0].max(b[0]),
        a[1].max(b[1]),
    ])
}
pub use openmt2001::building_styles::Styles;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires pinned native424010 completion observations"]
    fn matches_native_placement_choices() {
        let bytes = std::fs::read("work/economy/native-tutorial-build-drag.bin").unwrap();
        assert_eq!(&bytes[..8], b"MTBDRG01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 0x037f);
        assert_eq!(bytes.len(), 16 + word(12) as usize * 56);
        for row in bytes[16..].chunks_exact(56) {
            let w = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
            let mut state = w(0);
            let counts = [w(1), w(2), w(3)].map(|value| value as usize);
            if w(4) == 1 {
                let choices = placement_choices(&mut state, counts).unwrap();
                assert_eq!(choices.map(|v| v as u32), [w(6), w(7), w(8)]);
                assert_eq!([w(11), w(12), w(13)], [2, 88, 3]);
            } else {
                assert_eq!(w(11), 0);
            }
            assert_eq!(state, w(5));
        }
    }
    #[test]
    #[ignore = "requires private original v1.4 executable"]
    fn original_special_residential_group() {
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let members = special_residential_group(&image, 33).unwrap();
        assert_eq!(members, [27, 23, 33, -1]);
        let mut owners = [0u8; 52];
        for id in [27, 23, 33] { owners[id] = 1; }
        assert!(special_residential_group_owned(&members, 1, &owners).unwrap());
        owners[23] = 2;
        assert!(!special_residential_group_owned(&members, 1, &owners).unwrap());
        owners[23] = 0;
        assert!(!special_residential_group_owned(&members, 1, &owners).unwrap());
        owners[27] = 0;
        owners[33] = 0;
        assert!(special_residential_group_owned(&members, 0, &owners).unwrap(),
            "48a770 compares the null city-owner reference too");
    }
    #[test]
    #[ignore = "private original planning grid oracle"]
    fn original_grid_points() {
        let path = std::path::PathBuf::from(std::env::var("OPENMT2001_NATIVE_BUILD_UI").unwrap());
        let b = std::fs::read(path.with_file_name("native-build-ui-grid.bin")).unwrap();
        assert_eq!(&b[..8], b"MTBLDGRD");
        assert_eq!(
            b.len(),
            12 + u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize * 864
        );
        for row in b[12..].chunks_exact(864) {
            let word = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
            let bounds =
                std::array::from_fn(|i| std::array::from_fn(|a| f32::from_bits(word(i * 3 + a))));
            for (i, p) in grid_points(&bounds).unwrap().iter().enumerate() {
                assert_eq!(
                    p.map(f32::to_bits),
                    std::array::from_fn(|a| word(24 + i * 3 + a)),
                    "point {i}"
                );
            }
        }
    }

    #[test]
    #[ignore = "private original projected grid oracle"]
    fn original_cell_centers() {
        let path = std::path::PathBuf::from(std::env::var("OPENMT2001_NATIVE_BUILD_UI").unwrap());
        let b = std::fs::read(path.with_file_name("native-build-ui-centers.bin")).unwrap();
        assert_eq!(&b[..8], b"MTBLDCEN");
        assert_eq!(
            b.len(),
            12 + u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize * 904
        );
        for row in b[12..].chunks_exact(904) {
            let word = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
            let grid = std::array::from_fn(|i| {
                [f32::from_bits(word(i * 2)), f32::from_bits(word(i * 2 + 1))]
            });
            let actual = cell_centers(&grid);
            for (i, c) in actual.iter().enumerate() {
                assert_eq!(c.map(f32::to_bits), [word(128 + i * 2), word(129 + i * 2)]);
            }
        }
    }

    #[test]
    #[ignore = "private original projected lot selection oracle"]
    fn original_cell_picking() {
        let path = std::path::PathBuf::from(std::env::var("OPENMT2001_NATIVE_BUILD_UI").unwrap());
        let b = std::fs::read(path.with_file_name("native-build-ui-picking.bin")).unwrap();
        assert_eq!(&b[..8], b"MTBLDPCK");
        let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        assert_eq!(b.len(), 12 + n * 404);
        for row in b[12..].chunks_exact(404) {
            let word = |i: usize| u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap());
            let centers = std::array::from_fn(|i| {
                [f32::from_bits(word(i * 2)), f32::from_bits(word(i * 2 + 1))]
            });
            let expected = (word(100) != u32::MAX).then_some(word(100) as u8);
            assert_eq!(
                nearest_cell(&centers, [word(98) as i32, word(99) as i32]).unwrap(),
                expected
            );
        }
    }

    #[test]
    #[ignore = "private original planning entry oracle"]
    fn original_planning_entry() {
        let path = std::path::PathBuf::from(std::env::var("OPENMT2001_NATIVE_BUILD_UI").unwrap());
        let b = std::fs::read(path.with_file_name("native-build-ui-entry.bin")).unwrap();
        assert_eq!(&b[..8], b"MTBLDENT");
        let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        assert_eq!(b.len(), 12 + n * 20);
        for row in b[12..].chunks_exact(20) {
            let v: [u32; 5] = std::array::from_fn(|i| {
                u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap())
            });
            let expected = if v[3] != 0 {
                Entry::Open
            } else {
                Entry::Message {
                    text: v[4],
                    show_rights_action: v[4] == 1202,
                }
            };
            assert_eq!(entry(v[0] as u8, v[1] != 0, v[2]), expected);
        }
    }

    #[test]
    #[ignore = "private original construction UI request oracle"]
    fn original_build_button_requests() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_BUILD_UI").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTBLDUI1");
        let mut at = 8;
        let word = |at: &mut usize| {
            let v = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        };
        for _ in 0..word(&mut at) {
            let quality = word(&mut at) as usize;
            let columns = word(&mut at);
            let choice = word(&mut at) as i32;
            let gate = word(&mut at);
            let present = word(&mut at) != 0;
            let form = Form {
                category: 7,
                rectangle: [2, 3, 5, 7],
                height: 4,
                quality,
                choices: [choice; 3],
                variants: (0..columns).map(|i| 17 + i * 3).collect(),
                kind: 2,
                business: 11,
                displayed_quote: 1000.,
            };
            let result = form.request(gate == 1, gate != 2, gate != 3).unwrap();
            assert_eq!(result.is_ok(), present);
            if present {
                let expected: [u32; 11] = std::array::from_fn(|_| word(&mut at));
                let r = result.unwrap();
                assert_eq!(
                    [
                        r.category as u32,
                        r.origin[0] as u32,
                        r.origin[1] as u32,
                        r.size[0] as u32,
                        r.size[1] as u32,
                        r.height as u32,
                        r.packed_model,
                        r.kind,
                        u32::from(r.quality),
                        r.displayed_quote.to_bits(),
                        r.business
                    ],
                    expected
                );
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "private original construction rectangle oracle"]
    fn original_drag_rectangles() {
        let path = std::path::PathBuf::from(std::env::var("OPENMT2001_NATIVE_BUILD_UI").unwrap());
        let b = std::fs::read(path.with_file_name("native-build-ui-rectangles.bin")).unwrap();
        assert_eq!(&b[..8], b"MTBLDREC");
        let n = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        assert_eq!(b.len(), 12 + n * 24);
        for row in b[12..].chunks_exact(24) {
            let values: [u32; 6] = std::array::from_fn(|i| {
                u32::from_le_bytes(row[i * 4..i * 4 + 4].try_into().unwrap())
            });
            assert_eq!(
                rectangle_from_cells(values[0] as u8, values[1] as u8).unwrap(),
                values[2..]
                    .iter()
                    .map(|v| *v as i32)
                    .collect::<Vec<_>>()
                    .as_slice()
            );
        }
    }
}
