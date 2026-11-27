//! Named original UI mesh regions. Original43af30 resolves a35-byte name
//! record;43aca0 returns the indexed part's bounds in640x480 screen units.
use crate::mesh::CachedMesh;

#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub name: String,
    pub part: usize,
    /// Left, top, right, bottom; original mesh Y is negated for screen space.
    pub bounds: [f32; 4],
}

/// Original43b5f0 derives a parent rectangle from every frame vertex.
pub fn parent_bounds(mesh: &CachedMesh<'_>, frame: usize) -> Result<[f32; 4], String> {
    let positions = mesh.positions(frame)?;
    if positions.is_empty() {
        return Err("UI parent has no vertices".into());
    }
    let mut bounds = [99999f32, 99999f32, -99999f32, -99999f32];
    for [x, y, _] in positions {
        if !x.is_finite() || !y.is_finite() {
            return Err("nonfinite UI parent vertex".into());
        }
        bounds[0] = bounds[0].min(x);
        bounds[2] = bounds[2].max(x);
        bounds[1] = bounds[1].min(-y);
        bounds[3] = bounds[3].max(-y);
    }
    Ok(bounds)
}
/// Original4342c0 has strict horizontal and inclusive vertical edges.
pub fn parent_hit(bounds: [f32; 4], pointer: [f32; 2], offset: [f32; 2]) -> bool {
    pointer[0] > bounds[0] + offset[0]
        && pointer[0] < bounds[2] + offset[0]
        && pointer[1] >= bounds[1] + offset[1]
        && pointer[1] <= bounds[3] + offset[1]
}

pub fn regions(mesh: &CachedMesh<'_>, frame: usize) -> Result<Vec<Region>, String> {
    let positions = mesh.positions(frame)?;
    let mut regions: Vec<Region> = Vec::new();
    for record in mesh.opaque_records_35.chunks_exact(35) {
        let end = record[..33]
            .iter()
            .position(|&b| b == 0)
            .ok_or("unterminated UI region name")?;
        let name = core::str::from_utf8(&record[..end])
            .map_err(|_| "invalid UI region name")?
            .to_ascii_uppercase();
        if name.is_empty() || regions.iter().any(|r| r.name == name) {
            return Err("empty or duplicate UI region name".into());
        }
        let part_index = usize::from(record[33]);
        let part = mesh
            .parts
            .get(part_index)
            .ok_or("UI region exceeds mesh parts")?;
        // Original43aca0 starts at +/-99999 and visits the part index list.
        let mut minimum = [99999f32; 2];
        let mut maximum = [-99999f32; 2];
        if part.indices.is_empty() {
            return Err("UI region has no indexed vertices".into());
        }
        for index in part.vertex_indices() {
            let point = positions.get(index).ok_or("UI vertex exceeds positions")?;
            if !point[..2].iter().all(|v| v.is_finite()) {
                return Err("non-finite UI position".into());
            }
            for axis in 0..2 {
                minimum[axis] = minimum[axis].min(point[axis]);
                maximum[axis] = maximum[axis].max(point[axis]);
            }
        }
        regions.push(Region {
            name,
            part: part_index,
            bounds: [minimum[0], -maximum[1], maximum[0], -minimum[1]],
        });
    }
    Ok(regions)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "private original builder mesh and native input fixture"]
    fn matches_native_builder_parent_input() {
        let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
        let bytes = read("OPENMT2001_NATIVE_BUILDER_INPUT");
        assert_eq!(&bytes[..8], b"MTBINP01");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x037f);
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = crate::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find("gamedata2/brand_new_panels/main_panel_v2.tm")
            .unwrap();
        let mesh = crate::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        let bounds = super::parent_bounds(&mesh, 9).unwrap();
        for (i, bound) in bounds.iter().enumerate() {
            assert_eq!(
                bound.to_bits(),
                u32::from_le_bytes(bytes[12 + i * 4..16 + i * 4].try_into().unwrap())
            );
        }
        let count = u32::from_le_bytes(bytes[32..36].try_into().unwrap()) as usize;
        assert_eq!(bytes.len(), 36 + count * 24);
        for row in bytes[36..].chunks_exact(24) {
            let word = |at| u32::from_le_bytes(row[at..at + 4].try_into().unwrap());
            let offset = [f32::from_bits(word(0)), f32::from_bits(word(4))];
            let pointer = [word(12) as i32 as f32, word(16) as i32 as f32];
            assert_eq!(
                word(8) != 0 && super::parent_hit(bounds, pointer, offset),
                word(20) != 0
            );
        }
    }
    #[test]
    #[ignore = "requires original main-menu bounds fixture"]
    fn matches_native_main_menu_bounds() {
        let bytes = std::fs::read(crate::fixtures::path("NATIVE_UI_MESH")).unwrap();
        assert_eq!(&bytes[..8], b"MTUIM001");
        let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        let mesh = crate::mesh::CachedMesh::parse(&bytes[16..16 + length]).unwrap();
        let regions = super::regions(&mesh, 0).unwrap();
        assert_eq!(regions.len(), count);
        for region in &regions {
            let at = 16 + length + region.part * 16;
            let native: [u32; 4] = std::array::from_fn(|i| {
                u32::from_le_bytes(bytes[at + i * 4..at + i * 4 + 4].try_into().unwrap())
            });
            assert_eq!(region.bounds.map(f32::to_bits), native, "{}", region.name);
        }
        assert_eq!(regions[6].name, "MAIN_BUTT01");
        assert_eq!(regions[13].name, "MAIN_TYCOON_LOGO");
    }
}
