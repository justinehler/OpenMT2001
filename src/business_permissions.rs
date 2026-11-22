//! World-dependent building-panel gates407970/407cd0/407f60/408140/408430/4085e0.
//! Panel visibility and collapsed state are host presentation conditions.
use crate::{business_pool::BusinessCatalog, player_manager::word};

#[derive(Clone, Copy, Debug)]
pub struct Inputs {
    pub building_present: bool,
    pub business_class: Option<u32>,
    pub business_kind: u32,
    /// Resolved native owner references; zero means no owner.
    pub business_owner: u32,
    pub category_owner: u32,
    pub current_player: u32,
    /// Category+6c blocks the three management modes only when exactly1.
    pub category_reserved: u8,
    /// Building+3c/+3d/+3e respectively. These gates are independent.
    pub pending: [u8; 3],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trade {
    Sell,
    Takeover,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Permissions {
    /// Main panel modes0..5; this includes navigation with a disabled action.
    pub modes: [bool; 6],
    pub trade: Option<Trade>,
    pub convert: bool,
    pub demolish: bool,
}

pub fn evaluate(input: &Inputs) -> Permissions {
    if !input.building_present {
        return Permissions::default();
    }
    let mut result = Permissions {
        modes: [true, true, false, false, false, true],
        ..Default::default()
    };
    let Some(class) = input.business_class else {
        return result;
    };
    let owns_business = input.business_owner == input.current_player;
    let owns_category = input.category_owner == input.current_player;
    result.modes[2] = owns_business || owns_category;
    result.modes[3] = owns_business && class != 1 && input.business_kind != 43;
    result.modes[4] = owns_business;
    if class == 2 {
        result.modes[1] = false;
        result.modes[2] = false;
        result.modes[3] = false;
    }
    if input.category_reserved == 1 {
        result.modes[2..5].fill(false);
    }
    if result.modes[2] && input.pending[2] == 0 {
        result.trade = Some(if owns_business {
            Trade::Sell
        } else {
            Trade::Takeover
        });
    }
    result.convert = result.modes[3] && input.pending[1] == 0;
    result.demolish = result.modes[4] && input.pending[0] == 0 && (class != 2 || owns_category);
    result
}

///408140 list filtering. Native463bb0 geometry/era fit remains the caller's
/// already shared query; the catalog row must be the live script-mutated row.
pub fn conversion_candidate(
    input: &Inputs,
    target: u32,
    catalog: &BusinessCatalog,
    fit: bool,
) -> Result<bool, String> {
    if !evaluate(input).convert || !fit || target == input.business_kind || target == 43 {
        return Ok(false);
    }
    let row = catalog
        .row(target as usize)
        .ok_or("conversion candidate outside live catalog")?;
    Ok(!matches!(word(row, 0xc), 1 | 2))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original building-panel gate observations"]
    fn matches_native_permissions() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_BUSINESS_PERMISSIONS").unwrap())
            .unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = crate::pe::PeImage::parse(&exe).unwrap();
        let mut catalog = BusinessCatalog::from_pe(&image).unwrap();
        assert_eq!(&data[..8], b"MTBPER01");
        assert_eq!(word(&data, 8), 1);
        assert_eq!(word(&data, 12), 0x037f);
        assert_eq!(data.len(), 20 + word(&data, 16) as usize * 88);
        for (index, case) in data[20..].chunks_exact(88).enumerate() {
            let c: [u32; 12] = std::array::from_fn(|i| word(case, i * 4));
            let expected: [u32; 10] = std::array::from_fn(|i| word(case, (i + 12) * 4));
            let owner = |v| match v {
                0 => 0,
                1 => 0x10010000,
                2 => 0x10018000,
                _ => panic!("fixture owner"),
            };
            let input = Inputs {
                building_present: c[0] != 0,
                business_class: (c[1] != u32::MAX).then_some(c[1]),
                business_kind: c[2],
                business_owner: owner(c[3]),
                category_owner: owner(c[4]),
                current_player: 0x10010000,
                category_reserved: c[5] as u8,
                pending: [c[6] as u8, c[7] as u8, c[8] as u8],
            };
            let p = evaluate(&input);
            crate::player_manager::put(catalog.row_mut(c[9] as usize).unwrap(), 12, c[10]);
            let candidate = conversion_candidate(&input, c[9], &catalog, c[11] != 0).unwrap();
            let mut actual = [0; 10];
            actual[..6].copy_from_slice(&p.modes.map(u32::from));
            actual[6] = match p.trade {
                None => 0,
                Some(Trade::Sell) => 1,
                Some(Trade::Takeover) => 2,
            };
            actual[7] = u32::from(p.convert);
            actual[8] = u32::from(p.demolish);
            actual[9] = u32::from(candidate);
            assert_eq!(
                actual, expected,
                "native panel case{index}: {input:?} target{} class{} fit{}",
                c[9], c[10], c[11]
            );
        }
    }
}
