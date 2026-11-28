//! Original424abe construction form ->48ac20 producer inputs.
//! Cash/occupancy predicates come from shared runtime; this adapter owns no world.
/// 422af2..422b1f resolves the selected property's color and calls 48a770
/// before adding special Hotel catalog row 43 to the residential list.
pub fn special_residential_group(
    image: &crate::pe::PeImage<'_>,
    category_identity: u32,
) -> Result<[i32; 4], String> {
    let address = category_identity
        .checked_mul(88)
        .and_then(|offset| offset.checked_add(0x5385a4))
        .ok_or("planning special residential group address overflow")?;
    let color = u32::from(image.at(address, 1)?[0]);
    crate::ai_strategy_queries::group(image, color)
}

/// The live roster maps each nonzero owner byte to one player reference;
/// owner zero maps to the null reference used by 48a770 for city ownership.
pub fn special_residential_group_owned(
    members: &[i32; 4],
    landlord: u8,
    owners: &[u8; 52],
) -> Result<bool, String> {
    crate::ai_strategy_queries::owns_group(u32::from(landlord), members, |id| {
        let index =
            usize::try_from(id).map_err(|_| "planning special residential group sentinel")?;
        owners
            .get(index)
            .map(|&owner| u32::from(owner))
            .ok_or("planning special residential group member outside city".into())
    })
}
/// Hotel admission additionally requires a player landlord (4229f0).
/// Keep the shared complete-set query's null-owner identity semantics intact.
pub fn hotel_allowed(members: &[i32; 4], landlord: u8, owners: &[u8; 52]) -> Result<bool, String> {
    Ok(landlord != 0 && special_residential_group_owned(members, landlord, owners)?)
}

/// Original424600 leaves the builder before processing any planning input.
pub fn controller_exits(busy: u8, player_active: bool) -> bool {
    busy == 1 || !player_active
}
/// None closes planning without completing the placement callback.
pub type PlacementReply = Result<Option<[i32; 3]>, String>;
/// Original4c3150..4c3244 grid construction from category view-space bounds.
/// Preserve the intermediate stores before4e3680 projects these64 points.
pub fn grid_points(bounds: &[[f32; 3]; 8]) -> Result<[[f32; 3]; 64], String> {
    use crate::x87::Sum;
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
        let x = crate::x87::Sum::from_f32(center[0])
            .map_err(str::to_owned)?
            .ftol_low_word();
        let y = crate::x87::Sum::from_f32(center[1])
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

/// Planning controller exit or original404eb0 permission result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Exit,
    Open,
    Message { text: u32, show_rights_action: bool },
}
/// `rights` is the category +78 dword; `restricted` is the +6d byte.
pub fn entry(restricted: u8, is_landlord: bool, rights: u32) -> Entry {
    let inspected = crate::property_rights::inspect_values(restricted, is_landlord, rights, 0);
    match inspected.message {
        Some(text) => Entry::Message {
            text,
            show_rights_action: inspected.activate_visible,
        },
        None => Entry::Open,
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
    /// Human confirmation gate; inspection and scripted submission ignore it.
    /// The live planner only confirms after it has received a preview quote.
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
pub use crate::building_styles::Styles;

#[cfg(test)]
#[path = "build_panel_tests.rs"]
mod tests;
