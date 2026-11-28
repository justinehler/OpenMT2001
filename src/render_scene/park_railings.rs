//! The original 00686198 railings model: the town hall park's wire-mesh
//! fences and the lattice girders along the Reading Railroad bridge.
//!
//! 4c0c7c..4c0c81 registers it from the City Surround group (4c0240) in its
//! own model slot, outside the 006885c0 and 0068a978 lists. Both normal city
//! render paths (4b3b6a..4b3b82 in 4b3870, and the matching pass sequence)
//! submit it through 4e0d00 with the default 4df9c0 transform after the
//! scaffold pass. The gate is the unconditional 4524b0, so no detail level or
//! era hides it. Positions are baked in city space, and its pages take the
//! ordinary state-1 path: opaque pages first, then alpha pages blended
//! (4f22d0/4f2160).

pub(super) const PATH: &str = "gamedata2/New_building stuff/townhall_hospital/park_railings.tm";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original v1.4 city meshes, maps and save"]
    fn original_railings_reach_city_draws_including_the_bridge_girders() {
        use crate::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage, render_scene::*};
        let read = |name| std::fs::read(crate::fixtures::path(name)).unwrap();
        let executable = read("EXE");
        let directory = read("ARCHIVE_DIR");
        let archive = read("ARCHIVE_BIN");
        let build_file = read("BUILD_FILE");
        let suncols = read("SUNCOLS");
        let save = read("SAVE_TWO");
        let default = crate::fixtures::path("DEFAULT_SETTINGS");
        let scripts = default.parent().unwrap().parent().unwrap();
        let fresh = std::fs::read_to_string(scripts.join("MAPS/apartments2.lua")).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        // 4c0c7c pushes this path for the 00686198 slot.
        let bytes = image.at(0x54b6dc, 260).unwrap();
        let end = bytes.iter().position(|&b| b == 0).unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes[..end])
                .unwrap()
                .replace('\\', "/"),
            PATH
        );
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index.find(PATH).unwrap();
        let model = CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        let positions = model.positions(0).unwrap();
        // The girders are the rail-bridge texture parts. Some of them must
        // span the water between the bridge's two pairs of end posts in
        // reading_tracks.tm (x -135.7 and -60.6 in authored coordinates).
        let girders: Vec<[f32; 3]> = model
            .parts
            .iter()
            .filter(|part| {
                model.texture_names[usize::from(part.texture)]
                    .eq_ignore_ascii_case("generic_rail_bridge.tt")
            })
            .flat_map(|part| part.vertex_indices().map(|i| positions[i]))
            .collect();
        assert!(
            girders
                .iter()
                .any(|v| v[0] > -135. && v[0] < -61. && (400. ..470.).contains(&v[2]) && v[1] > 5.)
        );
        let camera = crate::flyby::CameraSample {
            eye: [-100., 200., -560.],
            target: [-100., 0., -435.],
            up: [0., 1., 0.],
            vertical_fov: 53.,
        };
        for source in [Source::Map(&fresh), Source::Save(&save)] {
            let city = assemble(Request {
                build_file: &build_file,
                source,
                original: Some(OriginalAssets {
                    executable: &executable,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: Some(&suncols),
                phase: Some(13),
                camera: Some(camera),
                width: 640,
                height: 480,
                animated: false,
            })
            .unwrap();
            let drawn: std::collections::HashSet<[u32; 3]> = city
                .draws
                .iter()
                .filter(|draw| {
                    matches!(
                        draw.mode,
                        crate::render::SurfaceMode::StaticLit
                            | crate::render::SurfaceMode::StaticOpaque
                    )
                })
                .flat_map(|draw| {
                    draw.vertices
                        .iter()
                        .map(|v| [v[0], v[1], v[2]].map(f32::to_bits))
                })
                .collect();
            for [x, y, z] in &girders {
                // Only the native Z axis flips; there is no other transform.
                assert!(drawn.contains(&[*x, *y, -z].map(f32::to_bits)));
            }
        }
    }
}
