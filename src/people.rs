//! Original people model sources and family-shared binding definitions.
use crate::{archive::ArchiveIndex, economy::ResidentModelBinding, mesh::CachedMesh, pe::PeImage};

// Original004be0f0 registration path operands, in table68aa48 row order.
const PATHS: [[u32; 2]; 32] = [
    [0x00549040, 0x00549008],
    [0x00548fd4, 0x00548f9c],
    [0x00548f68, 0x00548f30],
    [0x00548efc, 0x00548ec4],
    [0x00548e84, 0x00548e3c],
    [0x00548dfc, 0x00548db4],
    [0x00548d74, 0x00548d2c],
    [0x00548cec, 0x00548ca4],
    [0x00548c68, 0x00548c28],
    [0x00548bec, 0x00548bac],
    [0x00548b70, 0x00548b30],
    [0x00548af4, 0x00548ab4],
    [0x00548a80, 0x00548a48],
    [0x00548a14, 0x005489dc],
    [0x005489a8, 0x00548970],
    [0x0054893c, 0x00548904],
    [0x005488d4, 0x005488a0],
    [0x00548870, 0x0054883c],
    [0x0054880c, 0x005487d8],
    [0x005487a8, 0x00548774],
    [0x00548730, 0x005486e4],
    [0x005486a0, 0x00548654],
    [0x00548610, 0x005485c4],
    [0x00548580, 0x00548534],
    [0x005484f4, 0x005484b0],
    [0x00548470, 0x0054842c],
    [0x005483ec, 0x005483a8],
    [0x00548368, 0x00548324],
    [0x005482ec, 0x005482b0],
    [0x00548278, 0x0054823c],
    [0x00548204, 0x005481c8],
    [0x00548190, 0x00548154],
];

// Original 004be8ce: one bag per appearance group; both child groups use boys.
const BAG_PATHS: [u32; 8] = [
    0x00548120, 0x005480dc, 0x0054809c, 0x00548064, 0x00548120, 0x0054801c, 0x00547fd8, 0x00547f9c,
];

pub struct PeopleModels<'a> {
    paths: Vec<String>,
    meshes: Vec<CachedMesh<'a>>,
}

/// Compact draw part with variant-local UVs/materials and family geometry.
pub struct PreparedPart {
    pub source_vertices: Vec<usize>,
    pub triangles: Vec<[u32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub material_header: [u8; 20],
}

pub struct PreparedPeopleModel<'m, 'a> {
    geometry: &'m CachedMesh<'a>,
    pub parts: Vec<PreparedPart>,
    pub texture_names: Vec<String>,
}

/// Original queued batch matrix: three basis groups followed by translation.
/// The caller supplies the renderer's sign-adjusted matrix and fast-path flag.
#[derive(Clone, Copy, Debug)]
pub struct PeopleTransform {
    pub matrix: [[f32; 3]; 4],
    pub translation_only: bool,
}

impl PeopleTransform {
    /// Original004f2029 fast path or004e3870 general path. Keep the native
    /// component-specific sum order, with one final f32 store per coordinate.
    pub fn position(&self, position: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = position.map(f64::from);
        let [a, b, c, t] = self.matrix.map(|group| group.map(f64::from));
        if self.translation_only {
            [(t[0] + x) as f32, (t[1] + y) as f32, (t[2] - z) as f32]
        } else {
            [
                ((z * c[0] + t[0]) + (y * b[0] + x * a[0])) as f32,
                ((y * b[1] + x * a[1]) + (z * c[1] + t[1])) as f32,
                (((x * a[2] + t[2]) + y * b[2]) + z * c[2]) as f32,
            ]
        }
    }
}

impl PreparedPeopleModel<'_, '_> {
    pub fn frames(&self) -> usize {
        self.geometry.frames
    }

    /// Discrete original frame samples in stored model coordinates. Animation
    /// timing/interpolation and world transforms remain separate responsibilities.
    pub fn frame_positions(&self, frame: usize) -> Result<Vec<Vec<[f32; 3]>>, String> {
        let positions = self.geometry.positions(frame)?;
        Ok(self
            .parts
            .iter()
            .map(|part| part.source_vertices.iter().map(|&i| positions[i]).collect())
            .collect())
    }

    /// Select the auxiliary record's signed integer frame and transform its
    /// vertices. Invalid native indices are reported instead of reading outside
    /// the model. Animation cadence and matrix formation remain caller-owned.
    pub fn world_positions(
        &self,
        frame: i16,
        transform: &PeopleTransform,
    ) -> Result<Vec<Vec<[f32; 3]>>, String> {
        let frame = usize::try_from(frame).map_err(|_| "negative people animation frame")?;
        Ok(self
            .frame_positions(frame)?
            .into_iter()
            .map(|part| {
                part.into_iter()
                    .map(|position| transform.position(position))
                    .collect()
            })
            .collect())
    }
}

impl<'a> PeopleModels<'a> {
    /// Read-only load of all32 normal/LOD pairs and their shopping bags.
    /// The host supplies opaque references when binding resident bodies.
    pub fn load(
        image: &PeImage<'_>,
        index: &ArchiveIndex,
        archive: &'a [u8],
    ) -> Result<Self, String> {
        let mut paths = Vec::with_capacity(72);
        let mut meshes = Vec::with_capacity(72);
        for address in PATHS.into_iter().flatten().chain(BAG_PATHS) {
            let bytes = image.at(address, 256)?;
            let end = bytes
                .iter()
                .position(|&b| b == 0)
                .ok_or("unterminated people path")?;
            let path = std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid people path")?;
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing people model {path}"))?;
            let start =
                usize::try_from(entry.offset).map_err(|_| "people archive offset overflow")?;
            let end = usize::try_from(
                entry
                    .offset
                    .checked_add(entry.length)
                    .ok_or("people archive range overflow")?,
            )
            .map_err(|_| "people archive end overflow")?;
            let bytes = archive
                .get(start..end)
                .ok_or("people model outside archive")?;
            meshes.push(CachedMesh::parse(bytes)?);
            paths.push(path.to_owned());
        }
        Ok(Self { paths, meshes })
    }

    /// Bag geometry follows appearance >> 2, independently of body LOD.
    pub fn bag_source(&self, family: usize) -> Option<&CachedMesh<'a>> {
        if family >= 8 {
            return None;
        }
        self.meshes.get(64 + family)
    }

    pub fn source(&self, row: usize, lod: bool) -> Option<&CachedMesh<'a>> {
        if row >= 32 {
            return None;
        }
        self.meshes.get(row * 2 + usize::from(lod))
    }

    pub fn path(&self, row: usize, lod: bool) -> Option<&str> {
        if row >= 32 {
            return None;
        }
        self.paths
            .get(row * 2 + usize::from(lod))
            .map(String::as_str)
    }

    /// Original004ffe20 shares family-1 positions/frame count/bounds. UVs
    /// and part/material metadata remain variant-owned and must not be merged.
    /// Sources are raw cached meshes; runtime vertex compaction is separate.
    pub fn geometry_source(&self, row: usize, lod: bool) -> Option<&CachedMesh<'a>> {
        if row >= 32 {
            return None;
        }
        self.source(row & !3, lod)
    }

    /// Prepare original People parts, honoring004e96d0 flag1000 skip behavior
    /// and using004e8850 normal generation in the resulting triangle order.
    pub fn prepare(&self, row: usize, lod: bool) -> Result<PreparedPeopleModel<'_, 'a>, String> {
        let geometry = self
            .geometry_source(row, lod)
            .ok_or("people row exceeds registry")?;
        let variant = self.source(row, lod).unwrap();
        prepare_mesh(geometry, variant)
    }

    ///004dfa40 uses normal handles; each variant receives family-1 bounds.
    pub fn resident_bindings(
        &self,
        handles: &[[u32; 2]; 32],
    ) -> Result<[ResidentModelBinding; 32], String> {
        let mut bindings = Vec::with_capacity(32);
        for (row, handle) in handles.iter().enumerate() {
            bindings.push(ResidentModelBinding::from_mesh(
                handle[0],
                self.geometry_source(row, false).unwrap(),
            )?);
        }
        bindings
            .try_into()
            .map_err(|_| "incorrect people binding count".into())
    }
}

/// Original part registration shared by resident and auction character meshes.
pub fn prepare_mesh<'m, 'a>(
    geometry: &'m CachedMesh<'a>,
    variant: &'m CachedMesh<'a>,
) -> Result<PreparedPeopleModel<'m, 'a>, String> {
    if geometry.vertex_count != variant.vertex_count || geometry.parts.len() != variant.parts.len()
    {
        return Err("people variant geometry shape mismatch".into());
    }
    let positions = geometry.positions(0)?;
    let uvs = variant.uvs();
    let mut seen = vec![false; positions.len()];
    let mut parts = Vec::with_capacity(variant.parts.len());
    for (source, material) in geometry.parts.iter().zip(&variant.parts) {
        let indices: Vec<_> = source.vertex_indices().map(|i| i as u32).collect();
        if !indices
            .iter()
            .copied()
            .eq(material.vertex_indices().map(|i| i as u32))
        {
            return Err("people variant index ordering mismatch".into());
        }
        let triangles: Vec<[u32; 3]> = indices
            .chunks_exact(3)
            .map(|t| t.try_into().unwrap())
            .collect();
        let registered = if geometry.flags & 0x1000 != 0 {
            //004e96f1 tests AH bit10 and returns without reordering.
            let source_vertices: Vec<_> = (source.first_vertex..=source.last_vertex).collect();
            let local: Vec<[u32; 3]> = triangles
                .iter()
                .map(|tri| tri.map(|i| i - source.first_vertex as u32))
                .collect();
            let part_positions = &positions[source.first_vertex..=source.last_vertex];
            let normals = crate::mesh::generate_normals(
                part_positions,
                &[crate::mesh::NormalPart {
                    first_vertex: 0,
                    last_vertex: part_positions.len() - 1,
                    triangles: &local,
                }],
            )?;
            crate::mesh::RegisteredPart {
                source_vertices,
                triangles: local,
                normals,
            }
        } else {
            crate::mesh::RegisteredPart::new(&positions, &triangles)?
        };
        for &index in &registered.source_vertices {
            if seen[index] {
                return Err("overlapping people parts require global compaction".into());
            }
            seen[index] = true;
        }
        // Cached packed positions were quantized after the stored normals
        // were generated.004f1ec0/004f2580 consume manager+50 directly;
        // regenerating from decoded positions would change their lighting.
        let normals = if geometry.flags & 0x30 != 0 {
            registered
                .source_vertices
                .iter()
                .map(|&i| {
                    std::array::from_fn(|axis| {
                        f32::from_le_bytes(
                            geometry.normal_bytes[i * 12 + axis * 4..i * 12 + axis * 4 + 4]
                                .try_into()
                                .unwrap(),
                        )
                    })
                })
                .collect()
        } else {
            registered.normals
        };
        parts.push(PreparedPart {
            uvs: registered.source_vertices.iter().map(|&i| uvs[i]).collect(),
            source_vertices: registered.source_vertices,
            triangles: registered.triangles,
            normals,
            material_header: *material.header,
        });
    }
    Ok(PreparedPeopleModel {
        geometry,
        parts,
        texture_names: variant
            .texture_names
            .iter()
            .map(|s| s.to_string())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original people world vertex observations"]
    fn matches_native_people_world() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let models = PeopleModels::load(&image, &index, &archive).unwrap();
        let data = read("OPENMT2001_NATIVE_PEOPLE_WORLD");
        assert_eq!(&data[..8], b"MTPWORLD");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for case in 0..word(8) {
            let row = word(at) as usize;
            let lod = word(at + 4) != 0;
            let frame = word(at + 8) as i16;
            let part = word(at + 12) as usize;
            let translation_only = word(at + 16) != 0;
            let count = word(at + 20) as usize;
            at += 24;
            let matrix = std::array::from_fn(|i| {
                std::array::from_fn(|j| f32::from_bits(word(at + (i * 3 + j) * 4)))
            });
            at += 48;
            let prepared = models.prepare(row, lod).unwrap();
            let positions = prepared
                .world_positions(
                    frame,
                    &super::PeopleTransform {
                        matrix,
                        translation_only,
                    },
                )
                .unwrap();
            assert_eq!(positions[part].len(), count);
            for (i, position) in positions[part].iter().enumerate() {
                let expected: [u32; 3] = std::array::from_fn(|j| word(at + (i * 3 + j) * 4));
                assert_eq!(position.map(f32::to_bits), expected, "case{case} vertex{i}");
            }
            at += count * 12;
            assert!(
                prepared
                    .world_positions(
                        -1,
                        &super::PeopleTransform {
                            matrix,
                            translation_only
                        }
                    )
                    .is_err()
            );
            assert!(
                prepared
                    .world_positions(
                        prepared.frames() as i16,
                        &super::PeopleTransform {
                            matrix,
                            translation_only
                        }
                    )
                    .is_err()
            );
        }
        assert_eq!(at, data.len());
    }

    #[test]
    #[ignore = "requires original people preparation observations"]
    fn matches_native_people_preparation() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let models = PeopleModels::load(&image, &index, &archive).unwrap();
        let data = read("OPENMT2001_NATIVE_PEOPLE_PREPARATION");
        assert_eq!(&data[..8], b"MTPREP01");
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        let mut at = 12;
        let mut family: Vec<(usize, usize, &[u8], &[u8])> = Vec::new();
        for _ in 0..word(8) {
            let row = word(at);
            let mode = word(at + 4);
            let vertices = word(at + 8);
            let frames = word(at + 12);
            let parts = word(at + 16);
            let lengths: [usize; 5] = std::array::from_fn(|i| word(at + 20 + i * 4));
            at += 40;
            let fields: [&[u8]; 5] = std::array::from_fn(|i| {
                let f = &data[at..at + lengths[i]];
                at += lengths[i];
                f
            });
            if row == 0 {
                family.push((vertices, frames, fields[0], fields[4]));
            }
            let (family_vertices, family_frames, family_positions, family_normals) = family[mode];
            let prepared = models.prepare(row, mode != 0).unwrap();
            assert_eq!(prepared.frames(), family_frames);
            assert_eq!(prepared.parts.len(), parts);
            let mut index_at = 0;
            for (i, part) in prepared.parts.iter().enumerate() {
                let header = &fields[2][i * 20..i * 20 + 20];
                let get =
                    |off| u32::from_le_bytes(header[off..off + 4].try_into().unwrap()) as usize;
                let first = get(12);
                let last = get(16);
                assert_eq!(part.source_vertices, (first..=last).collect::<Vec<_>>());
                let mut material = part.material_header;
                material[8..12].fill(0);
                assert_eq!(material.as_slice(), header);
                for (j, uv) in part.uvs.iter().enumerate() {
                    for (axis, value) in uv.iter().enumerate() {
                        let offset = (first + j) * 8 + axis * 4;
                        assert_eq!(
                            value.to_bits(),
                            u32::from_le_bytes(fields[1][offset..offset + 4].try_into().unwrap()),
                            "row{row} mode{mode} UV"
                        );
                    }
                }
                for (j, normal) in part.normals.iter().enumerate() {
                    for (axis, value) in normal.iter().enumerate() {
                        let offset = (first + j) * 12 + axis * 4;
                        assert_eq!(
                            value.to_bits(),
                            u32::from_le_bytes(
                                family_normals[offset..offset + 4].try_into().unwrap()
                            ),
                            "row{row} mode{mode} normal part{i} vertex{j}"
                        );
                    }
                }
                for tri in &part.triangles {
                    for &index in tri {
                        assert_eq!(
                            index,
                            u16::from_le_bytes(
                                fields[3][index_at..index_at + 2].try_into().unwrap()
                            ) as u32
                        );
                        index_at += 2;
                    }
                }
            }
            assert_eq!(index_at, fields[3].len());
            for frame in 0..family_frames {
                let actual = prepared.frame_positions(frame).unwrap();
                for (part, positions) in prepared.parts.iter().zip(actual) {
                    for (&source, position) in part.source_vertices.iter().zip(positions) {
                        for (axis, value) in position.iter().enumerate() {
                            let offset = (frame * family_vertices + source) * 12 + axis * 4;
                            assert_eq!(
                                value.to_bits(),
                                u32::from_le_bytes(
                                    family_positions[offset..offset + 4].try_into().unwrap()
                                )
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(at, data.len());
    }
    #[test]
    #[ignore = "requires original people registry trace and archives"]
    fn matches_native_people_registry() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = PeImage::parse(&exe).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let native = read("OPENMT2001_NATIVE_PEOPLE_REGISTRY");
        assert_eq!(&native[..8], b"MTPEOP01");
        let word = |at| u32::from_le_bytes(native[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 112);
        let models = PeopleModels::load(&image, &index, &archive).unwrap();
        let mut at = 12;
        for (row, pair) in PATHS.iter().enumerate() {
            for (mode, path) in pair.iter().enumerate() {
                let event: [u32; 4] = std::array::from_fn(|i| word(at + i * 4));
                at += 16;
                assert_eq!(
                    event,
                    [0x4fff00, *path, 0x68aa48 + row as u32 * 16, mode as u32]
                );
                let mesh = models.source(row, mode != 0).unwrap();
                let prepared = models.prepare(row, mode != 0).unwrap();
                assert_eq!(
                    prepared.frames(),
                    models.geometry_source(row, mode != 0).unwrap().frames
                );
                assert_eq!(prepared.parts.len(), mesh.parts.len());
                for frame in [0, prepared.frames() - 1] {
                    let positions = prepared.frame_positions(frame).unwrap();
                    assert_eq!(positions.len(), prepared.parts.len());
                }

                let entry = index.find(models.path(row, mode != 0).unwrap()).unwrap();
                assert_eq!(
                    mesh.header.as_slice(),
                    &archive[entry.offset as usize..entry.offset as usize + 164]
                );
                if row % 4 != 0 {
                    let event: [u32; 4] = std::array::from_fn(|i| word(at + i * 4));
                    at += 16;
                    assert_eq!(
                        event,
                        [
                            0x4ffe20,
                            0x68aa48 + (row as u32 & !3) * 16,
                            0x68aa48 + row as u32 * 16,
                            mode as u32
                        ]
                    );
                }
            }
        }
        assert_eq!(at, native.len());
        let handles = std::array::from_fn(|i| [500 + i as u32 * 7, 900 + i as u32 * 11]);
        let bindings = models.resident_bindings(&handles).unwrap();
        for row in 0..32 {
            assert_eq!(bindings[row].reference, handles[row][0]);
            assert_eq!(
                bindings[row].definition,
                models
                    .source(row & !3, false)
                    .unwrap()
                    .model_definition()
                    .unwrap()
            );
            let mut auxiliary = [0xa5; 284];
            crate::resident::bind_auxiliary_model(
                &mut auxiliary,
                bindings[row].reference,
                &bindings[row].definition,
            );
            assert_eq!(&auxiliary[0x44..0xb8], &bindings[row].definition);
        }
    }
}
