//! Fixed city structures built by v1.4 004891f0, outside the Lua building pool.
use crate::{
    board::Board,
    build_file::{BuildFile, ModelQuery},
    scene::{BuildingSpec, SceneBuilding},
};

/// Presentation-only kind, outside the three Lua building kinds.
pub const FIXED_KIND: u8 = 3;

/// Search sizes are tried in order; the last match of the first nonempty
/// search wins (00489312..004893a1). The transpose marker is discarded by
/// 004d4a20, and these models never pass through 00488230's lot rotation.
fn select(file: &BuildFile<'_>, category: u8) -> Result<(usize, usize), String> {
    for [width, depth] in [[7, 7], [7, 3], [3, 7]] {
        let matches = file.search(ModelQuery {
            width,
            depth,
            height: 1,
            quality: 0,
            placement: 1,
            category,
            limit: 20,
        })?;
        if let Some(selected) = matches.last() {
            return Ok((selected.index, matches.len()));
        }
    }
    Err(format!(
        "missing original fixed city model category {category}"
    ))
}

/// These are presentation objects, not rentable buildings or businesses.
/// 004891f0 pairs model categories with board properties and places the raw
/// profile at (property X - 35, .5, property Z + 35 + offset).
pub fn buildings(board: &Board, file: &BuildFile<'_>) -> Result<Vec<SceneBuilding>, String> {
    [
        (2, 7, 0., 0x50),
        (35, 8, 0., 0x50),
        (42, 9, 0., 0x50),
        (39, 10, 0., 0x50),
        (31, 12, 40., 0x190),
    ]
    .into_iter()
    .map(|(property, category, z_offset, variation)| {
        let block = board
            .blocks
            .iter()
            .find(|block| block.id == property)
            .ok_or_else(|| format!("missing fixed city property {property}"))?;
        let (index, model_variants) = select(file, category)?;
        Ok(SceneBuilding {
            spec: BuildingSpec {
                block: block.name.clone(),
                lot: [0, 0, 7, 7],
                height: 1,
                kind: FIXED_KIND,
                variation,
                quality: 0,
                businesses: Vec::new(),
            },
            block_id: property,
            center: [block.center[0], block.center[1] + z_offset],
            selected: crate::build_file::ModelMatch {
                index,
                transposed: false,
            },
            model_variants,
            angle: 0.,
            mirror: false,
            canonical_size: [7, 7],
            layers: file.profile(index, 1)?,
        })
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_file(models: &[u8]) -> BuildFile<'_> {
        BuildFile {
            header: [0; 25],
            points: &[],
            point_attributes: &[],
            polygons: &[],
            attribute_ranges: &[],
            layers: &[],
            models,
            details: &[],
        }
    }

    #[test]
    fn fixed_model_search_uses_last_match_and_ordered_size_fallbacks() {
        let mut models = Vec::new();
        for (width, depth, height, quality, placement) in [
            (7, 3, 1, 1, 1),
            (7, 7, 1, 1, 1),
            (7, 7, 1, 3, 1),
            (7, 7, 2, 1, 1),
            (7, 7, 1, 1, 2),
        ] {
            let mut row = [0; 28];
            row[21..28].copy_from_slice(&[width, depth, 1, 7, quality, height, placement]);
            models.extend(row);
        }
        assert_eq!(select(&model_file(&models), 7).unwrap(), (2, 2));
        assert_eq!(select(&model_file(&models[..28]), 7).unwrap(), (0, 1));
        assert!(select(&model_file(&models), 8).is_err());
        // A square search's 20-match cap also applies to fixed structures.
        let repeated = models[28..56].repeat(21);
        assert_eq!(select(&model_file(&repeated), 7).unwrap(), (19, 20));
    }
}
