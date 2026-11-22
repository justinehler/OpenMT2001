//! Cached .tm layout read by original v1.4 004edf60.
//! Animation blocks remain preserved; packed positions follow original004f1dd0.

pub struct MeshPart<'a> {
    pub header: &'a [u8; 20],
    pub texture: u16,
    pub first_vertex: usize,
    pub last_vertex: usize,
    /// Indices are relative to first_vertex, not to the whole mesh.
    pub indices: &'a [u8],
}

impl MeshPart<'_> {
    pub fn vertex_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.indices
            .chunks_exact(2)
            .map(|b| self.first_vertex + usize::from(short(b, 0)))
    }
}

/// One original normal-generation part, with indices relative to first_vertex.
pub struct NormalPart<'a> {
    pub first_vertex: usize,
    pub last_vertex: usize,
    pub triangles: &'a [[u32; 3]],
}

/// Original 004e8850: equally weighted unit faces within each part, followed
/// by two normalization passes. Later parts overwrite overlapping ranges.
/// Uncovered vertices receive zero instead of depending on allocator contents.
pub fn generate_normals(
    positions: &[[f32; 3]],
    parts: &[NormalPart<'_>],
) -> Result<Vec<[f32; 3]>, String> {
    let mut normals = vec![[0f32; 3]; positions.len()];
    for part in parts {
        if part.first_vertex > part.last_vertex || part.last_vertex >= positions.len() {
            return Err("normal part exceeds vertex range".into());
        }
        normals[part.first_vertex..=part.last_vertex].fill([0.; 3]);
        for triangle in part.triangles {
            if triangle
                .iter()
                .any(|&i| i as usize > part.last_vertex - part.first_vertex)
            {
                return Err("normal triangle exceeds part range".into());
            }
            let ids = triangle.map(|i| i as usize + part.first_vertex);
            let [p0, p1, p2] = ids.map(|i| positions[i].map(f64::from));
            let a = std::array::from_fn::<_, 3, _>(|i| p0[i] - p1[i]);
            let mut b = std::array::from_fn::<_, 3, _>(|i| p2[i] - p1[i]);
            // 004e898e/98 store only the first two components before cross.
            b[0] = f64::from(b[0] as f32);
            b[1] = f64::from(b[1] as f32);
            let cross = [
                (b[2] * a[1] - b[1] * a[2]) as f32,
                (b[0] * a[2] - b[2] * a[0]) as f32,
                (b[1] * a[0] - b[0] * a[1]) as f32,
            ];
            let [x, y, z] = cross.map(f64::from);
            let inverse = (-1. / ((z * z + y * y) + x * x).sqrt()) as f32;
            for (corner, &id) in ids.iter().enumerate() {
                if ids[..corner].contains(&id) {
                    continue;
                }
                for (axis, &value) in cross.iter().enumerate() {
                    normals[id][axis] = (f64::from(inverse) * f64::from(value)
                        + f64::from(normals[id][axis]))
                        as f32;
                }
            }
        }
        for normal in &mut normals[part.first_vertex..=part.last_vertex] {
            let [x, y, z] = normal.map(f64::from);
            let length = ((z * z + y * y) + x * x).sqrt();
            if length > 0. {
                let inverse = f64::from((1. / length) as f32);
                *normal = normal.map(|v| (inverse * f64::from(v)) as f32);
            } else {
                *normal = [0.; 3];
            }
        }
    }
    for normal in &mut normals {
        let [x, y, z] = normal.map(f64::from);
        let squared = ((x * x + y * y) + z * z) as f32;
        if !(0.8f32..=1.2f32).contains(&squared) {
            *normal = [0.; 3];
        } else {
            let inverse = 1. / f64::from(squared).sqrt();
            *normal = normal.map(|v| (inverse * f64::from(v)) as f32);
        }
    }
    Ok(normals)
}

/// Static part after original004e96d0 first-use vertex compaction and triangle ordering.
/// Source indices allow the caller to retain positions/UVs without duplicating attributes.
pub struct RegisteredPart {
    pub source_vertices: Vec<usize>,
    pub triangles: Vec<[u32; 3]>,
    pub normals: Vec<[f32; 3]>,
}

impl RegisteredPart {
    pub fn new(positions: &[[f32; 3]], input: &[[u32; 3]]) -> Result<Self, String> {
        let mut source_vertices = Vec::new();
        let mut mapping = vec![None; positions.len()];
        let mut triangles = Vec::with_capacity(input.len());
        for triangle in input {
            let mut remapped = [0; 3];
            for (corner, &vertex) in triangle.iter().enumerate() {
                let slot = mapping
                    .get_mut(vertex as usize)
                    .ok_or("registration index exceeds positions")?;
                remapped[corner] = *slot.get_or_insert_with(|| {
                    let index = source_vertices.len() as u32;
                    source_vertices.push(vertex as usize);
                    index
                });
            }
            triangles.push(remapped);
        }
        // Original cache contains ten distinct vertices. Reuse does not move
        // a vertex; new entries prepend and discard the oldest when full.
        let mut cache = Vec::with_capacity(10);
        let insert = |cache: &mut Vec<u32>, triangle: [u32; 3]| {
            for id in triangle {
                if !cache.contains(&id) {
                    if cache.len() == 10 {
                        cache.pop();
                    }
                    cache.insert(0, id);
                }
            }
        };
        if let Some(&first) = triangles.first() {
            insert(&mut cache, first);
        }
        for i in 1..triangles.len().saturating_sub(1) {
            let score = |t: [u32; 3]| t.iter().filter(|id| cache.contains(id)).count();
            let mut best = i;
            for j in i + 1..triangles.len() {
                if score(triangles[j]) > score(triangles[best]) {
                    best = j;
                }
            }
            triangles.swap(i, best);
            insert(&mut cache, triangles[i]);
        }
        let compact: Vec<_> = source_vertices.iter().map(|&i| positions[i]).collect();
        let normals = if compact.is_empty() {
            Vec::new()
        } else {
            generate_normals(
                &compact,
                &[NormalPart {
                    first_vertex: 0,
                    last_vertex: compact.len() - 1,
                    triangles: &triangles,
                }],
            )?
        };
        Ok(Self {
            source_vertices,
            triangles,
            normals,
        })
    }
}

pub struct CachedMesh<'a> {
    pub header: &'a [u8; 164],
    pub vertex_count: usize,
    pub frames: usize,
    pub flags: u32,
    pub vertex_bytes: &'a [u8],
    pub uv_bytes: &'a [u8],
    pub normal_bytes: &'a [u8],
    pub opaque_records_35: &'a [u8],
    pub opaque_records_28: &'a [u8],
    pub parts: Vec<MeshPart<'a>>,
    pub texture_names: Vec<&'a str>,
}

fn short(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap())
}
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn take<'a>(input: &mut &'a [u8], count: usize, stride: usize) -> Result<&'a [u8], String> {
    let length = count.checked_mul(stride).ok_or("mesh size overflow")?;
    let (head, tail) = input
        .split_at_checked(length)
        .ok_or("truncated cached mesh")?;
    *input = tail;
    Ok(head)
}

impl<'a> CachedMesh<'a> {
    pub fn parse(mut bytes: &'a [u8]) -> Result<Self, String> {
        let header: &[u8; 164] = take(&mut bytes, 164, 1)?.try_into().unwrap();
        let vertex_count = usize::from(short(header, 0));
        let part_count = usize::from(short(header, 2));
        let frames = usize::from(short(header, 4));
        let name_bytes = usize::from(short(header, 6));
        let flags = word(header, 16);
        let samples = vertex_count
            .checked_mul(frames)
            .ok_or("mesh frame size overflow")?;
        let vertex_bytes = take(&mut bytes, samples, if flags & 0x30 != 0 { 4 } else { 12 })?;
        let uv_bytes = take(&mut bytes, vertex_count, 8)?;
        let normal_bytes = take(&mut bytes, vertex_count, 12)?;
        let opaque_records_35 = take(&mut bytes, word(header, 8) as usize, 35)?;
        let opaque_records_28 = take(&mut bytes, word(header, 12) as usize, 28)?;
        let part_headers = take(&mut bytes, part_count, 20)?;
        let mut indices = if part_count > 0 {
            let count = word(take(&mut bytes, 4, 1)?, 0) as usize;
            take(&mut bytes, count, 2)?
        } else {
            &[]
        };
        let mut parts = Vec::with_capacity(part_count);
        for header in part_headers.chunks_exact(20) {
            let count = usize::from(short(header, 6));
            if !count.is_multiple_of(3) {
                return Err("mesh part index count is not triangular".into());
            }
            let part_indices = take(&mut indices, count, 2)?;
            let first_vertex = word(header, 12) as usize;
            let last_vertex = word(header, 16) as usize;
            if count != 0 && (first_vertex > last_vertex || last_vertex >= vertex_count) {
                return Err("mesh part vertex range exceeds mesh".into());
            }
            for i in part_indices.chunks_exact(2) {
                if usize::from(short(i, 0)) > last_vertex - first_vertex {
                    return Err("mesh index exceeds part vertex range".into());
                }
            }
            parts.push(MeshPart {
                header: header.try_into().unwrap(),
                texture: short(header, 4),
                first_vertex,
                last_vertex,
                indices: part_indices,
            });
        }
        if !indices.is_empty() {
            return Err("unclaimed mesh indices".into());
        }
        let mut names = take(&mut bytes, name_bytes, 1)?;
        let mut texture_names = Vec::new();
        while !names.is_empty() {
            let end = names
                .iter()
                .position(|&b| b == 0)
                .ok_or("unterminated mesh texture name")?;
            texture_names.push(
                std::str::from_utf8(&names[..end]).map_err(|_| "non-UTF8 mesh texture name")?,
            );
            names = &names[end + 1..];
        }
        if !bytes.is_empty() {
            return Err("trailing cached mesh bytes".into());
        }
        Ok(Self {
            header,
            vertex_count,
            frames,
            flags,
            vertex_bytes,
            uv_bytes,
            normal_bytes,
            opaque_records_35,
            opaque_records_28,
            parts,
            texture_names,
        })
    }

    /// Stored XYZ coordinates for one frame. Original004f1dd0 decodes10/11/11
    /// unsigned fields with the cached per-axis scale and offset (file+18/+24).
    pub fn positions(&self, frame: usize) -> Result<Vec<[f32; 3]>, String> {
        if frame >= self.frames {
            return Err("mesh frame out of range".into());
        }
        if self.flags & 0x30 != 0 {
            let start = frame * self.vertex_count * 4;
            let scale = std::array::from_fn(|i| f32::from_bits(word(self.header, 24 + i * 4)));
            let offset = std::array::from_fn(|i| f32::from_bits(word(self.header, 36 + i * 4)));
            return Ok(self.vertex_bytes[start..start + self.vertex_count * 4]
                .chunks_exact(4)
                .map(|v| decode_position(word(v, 0), scale, offset))
                .collect());
        }
        let size = self.vertex_count * 12;
        Ok(self.vertex_bytes[frame * size..(frame + 1) * size]
            .chunks_exact(12)
            .map(|v| core::array::from_fn(|a| f32::from_bits(word(v, a * 4))))
            .collect())
    }

    /// Rebuild original004eaf30's pointer-free manager+5c definition from
    /// every animation frame. Engine coordinates negate stored Z.
    pub fn model_definition(&self) -> Result<[u8; 116], String> {
        if self.flags & 0x30 != 0 {
            return Err("packed model bounds require decoded positions".into());
        }
        model_definition(self.vertex_bytes)
    }

    pub fn uvs(&self) -> Vec<[f32; 2]> {
        self.uv_bytes
            .chunks_exact(8)
            .map(|v| [f32::from_bits(word(v, 0)), f32::from_bits(word(v, 4))])
            .collect()
    }
}

/// The packed renderer keeps multiply/add on x87 until each XYZ float store.
pub fn decode_position(packed: u32, scale: [f32; 3], offset: [f32; 3]) -> [f32; 3] {
    let fields = [
        packed & 0x3ff,
        (packed >> 10) & 0x7ff,
        (packed >> 21) & 0x7ff,
    ];
    std::array::from_fn(|i| {
        (f64::from(fields[i]) * f64::from(scale[i]) + f64::from(offset[i])) as f32
    })
}

/// All-frame bounds for original unpacked XYZ samples. Explicit f64 arithmetic
/// reproduces the original x87 53-bit control word between f32 stores.
pub fn model_definition(samples: &[u8]) -> Result<[u8; 116], String> {
    if !samples.len().is_multiple_of(12) {
        return Err("partial model position".into());
    }
    let mut low = [9_999_999_f32; 3];
    let mut high = [-9_999_999_f32; 3];
    for sample in samples.chunks_exact(12) {
        let mut point: [f32; 3] =
            std::array::from_fn(|axis| f32::from_bits(word(sample, axis * 4)));
        point[2] = -point[2];
        for axis in 0..3 {
            if !point[axis].is_finite() {
                return Err("non-finite model position".into());
            }
            if point[axis] < low[axis] {
                low[axis] = point[axis];
            }
            if point[axis] > high[axis] {
                high[axis] = point[axis];
            }
        }
    }
    let mut values = [0_f32; 29];
    for corner in 0..8 {
        for axis in 0..3 {
            values[corner * 3 + axis] = if corner & (1 << axis) == 0 {
                low[axis]
            } else {
                high[axis]
            };
        }
    }
    let mut half = [0_f64; 3];
    for axis in 0..3 {
        values[24 + axis] = ((f64::from(high[axis]) + f64::from(low[axis])) * 0.5) as f32;
        half[axis] = (f64::from(high[axis]) - f64::from(low[axis])) * 0.5;
    }
    let squared = (half[2] * half[2] + half[1] * half[1]) + half[0] * half[0];
    values[27] = squared as f32;
    values[28] = squared.sqrt() as f32;
    let mut bytes = [0; 116];
    for (target, value) in bytes.chunks_exact_mut(4).zip(values) {
        target.copy_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original packed model observations and archive"]
    fn matches_native_packed_positions() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let data = read("OPENMT2001_NATIVE_PACKED_POSITIONS");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = crate::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        assert_eq!(&data[..8], b"MTPACK01");
        assert_eq!(word(&data, 8), 0x037f);
        let mut at = 16;
        let mut vertices = 0;
        for case in 0..word(&data, 12) {
            let length = word(&data, at) as usize;
            let frame = word(&data, at + 4) as usize;
            let count = word(&data, at + 8) as usize;
            at += 12;
            let name = std::str::from_utf8(&data[at..at + length]).unwrap();
            at += length;
            let scale = std::array::from_fn(|i| f32::from_bits(word(&data, at + i * 4)));
            let offset = std::array::from_fn(|i| f32::from_bits(word(&data, at + 12 + i * 4)));
            at += 24;
            let source = &data[at..at + count * 4];
            at += count * 4;
            let expected = &data[at..at + count * 12];
            at += count * 12;
            let entry = index.find(name).unwrap();
            let mesh = super::CachedMesh::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap();
            let actual = mesh.positions(frame).unwrap();
            assert_eq!(actual.len(), count);
            for (i, vertex) in actual.iter().enumerate() {
                let wanted = std::array::from_fn::<_, 3, _>(|j| word(expected, i * 12 + j * 4));
                assert_eq!(
                    vertex.map(f32::to_bits),
                    wanted,
                    "case{case} {name} frame{frame} vertex{i}"
                );
                assert_eq!(
                    super::decode_position(word(source, i * 4), scale, offset).map(f32::to_bits),
                    wanted
                );
            }
            vertices += count;
        }
        assert_eq!(at, data.len());
        println!("Original packed positions: {vertices} vertices across70 character/model frames");
    }
    #[test]
    #[ignore = "requires original model bounds and archive"]
    fn matches_native_model_definition() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let data = read("OPENMT2001_NATIVE_MODEL_DEFINITION");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = crate::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        assert_eq!(&data[..8], b"MTBOUND1");
        let mut at = 12;
        for case in 0..word(&data, 8) {
            let slot = word(&data, at);
            let length = word(&data, at + 4) as usize;
            at += 8;
            let samples = &data[at..at + length];
            at += length;
            let expected = &data[at..at + 116];
            at += 116;
            assert_eq!(
                model_definition(samples).unwrap().as_slice(),
                expected,
                "case{case}"
            );
            if slot != u32::MAX {
                let entry = &index.entries[slot as usize];
                let mesh = CachedMesh::parse(
                    &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
                )
                .unwrap();
                assert_eq!(mesh.vertex_bytes, samples);
                let binding =
                    crate::economy::ResidentModelBinding::from_mesh(slot + 1, &mesh).unwrap();
                assert_eq!(
                    binding.definition.as_slice(),
                    expected,
                    "archive slot{slot}"
                );
                assert_eq!(&mesh.header[0x30..0xa4], expected);
                assert_ne!(
                    model_definition(&samples[..mesh.vertex_count * 12]).unwrap(),
                    binding.definition,
                    "frame zero must not substitute for animation bounds"
                );
                let mut auxiliary = [0xa5; 284];
                crate::resident::bind_auxiliary_model(
                    &mut auxiliary,
                    binding.reference,
                    &binding.definition,
                );
                assert_eq!(&auxiliary[0x44..0xb8], expected);
                assert_eq!(word(&auxiliary, 0), slot + 1);
            }
        }
        assert_eq!(at, data.len());
    }

    use super::*;

    #[test]
    #[ignore = "requires original-instruction mesh registration fixture"]
    fn matches_native_static_registration() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_REGISTRATION").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTREG002");
        let n = word(&data, 8) as usize;
        let t = word(&data, 12) as usize;
        assert_eq!(data.len(), 16 + (n * 4 + t * 2) * 12);
        let mut at = 16;
        let mut triples = |count: usize| {
            let result: Vec<[u32; 3]> = (0..count)
                .map(|_| {
                    let v = std::array::from_fn(|i| word(&data, at + i * 4));
                    at += 12;
                    v
                })
                .collect();
            result
        };
        let positions: Vec<_> = triples(n)
            .into_iter()
            .map(|v| v.map(f32::from_bits))
            .collect();
        let triangles = triples(t);
        let registered = RegisteredPart::new(&positions, &triangles).unwrap();
        let actual: Vec<_> = registered
            .source_vertices
            .iter()
            .map(|&i| positions[i].map(f32::to_bits))
            .collect();
        assert_eq!(actual, triples(n));
        assert_eq!(registered.triangles, triples(t));
        assert_eq!(
            registered
                .normals
                .iter()
                .map(|v| v.map(f32::to_bits))
                .collect::<Vec<_>>(),
            triples(n)
        );
        assert_eq!(
            registered
                .normals
                .iter()
                .map(|&v| crate::scene::baked_normal(v, true, 90.).map(f32::to_bits))
                .collect::<Vec<_>>(),
            triples(n)
        );
    }

    #[test]
    #[ignore = "requires original-instruction normal fixture"]
    fn matches_native_generated_normals() {
        let data = std::fs::read(std::env::var("OPENMT2001_NATIVE_NORMALS").unwrap()).unwrap();
        assert_eq!(&data[..8], b"MTNORM01");
        let mut bytes = &data[8..];
        fn next(bytes: &mut &[u8]) -> u32 {
            let result = u32::from_le_bytes(bytes[..4].try_into().unwrap());
            *bytes = &bytes[4..];
            result
        }
        let count = next(&mut bytes);
        assert_eq!(count, 12);
        for _ in 0..count {
            let name_len = u16::from_le_bytes(bytes[..2].try_into().unwrap()) as usize;
            let name = std::str::from_utf8(&bytes[2..2 + name_len])
                .unwrap()
                .to_owned();
            bytes = &bytes[2 + name_len..];
            let n = next(&mut bytes) as usize;
            let positions: Vec<_> = (0..n)
                .map(|_| std::array::from_fn(|_| f32::from_bits(next(&mut bytes))))
                .collect();
            let part_count = next(&mut bytes);
            let mut owned = Vec::new();
            for _ in 0..part_count {
                let first = next(&mut bytes) as usize;
                let last = next(&mut bytes) as usize;
                let count = next(&mut bytes);
                assert_eq!(count % 3, 0);
                let triangles: Vec<[u32; 3]> = (0..count / 3)
                    .map(|_| std::array::from_fn(|_| next(&mut bytes)))
                    .collect();
                owned.push((first, last, triangles));
            }
            let parts: Vec<_> = owned
                .iter()
                .map(|(first, last, triangles)| NormalPart {
                    first_vertex: *first,
                    last_vertex: *last,
                    triangles,
                })
                .collect();
            let actual = generate_normals(&positions, &parts).unwrap();
            for (index, normal) in actual.iter().enumerate() {
                let expected: [u32; 3] = std::array::from_fn(|_| next(&mut bytes));
                assert_eq!(normal.map(f32::to_bits), expected, "{name} vertex {index}");
            }
        }
        assert!(bytes.is_empty());
    }

    #[test]
    #[ignore = "requires original archive and native mesh draw observations"]
    fn matches_native_part_vertex_submission() {
        let read = |key| std::fs::read(std::env::var(key).expect(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = crate::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let oracle = read("OPENMT2001_NATIVE_MESH_DRAW");
        assert_eq!(&oracle[..8], b"MTMESHD1");
        let mut at = 12;
        for _ in 0..word(&oracle, 8) {
            let length = word(&oracle, at) as usize;
            at += 4;
            let name = std::str::from_utf8(&oracle[at..at + length]).unwrap();
            at += length;
            let translation: [f32; 3] =
                std::array::from_fn(|i| f32::from_bits(word(&oracle, at + i * 4)));
            let expected_count = word(&oracle, at + 12) as usize;
            at += 16;
            let entry = index.find(name).unwrap();
            let mesh = CachedMesh::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap();
            let positions = mesh.positions(0).unwrap();
            let uvs = mesh.uvs();
            let mut count = 0;
            for vertex in mesh.parts.iter().flat_map(MeshPart::vertex_indices) {
                let p = positions[vertex];
                let actual = [
                    (f64::from(translation[0]) + f64::from(p[0])) as f32,
                    (f64::from(translation[1]) + f64::from(p[1])) as f32,
                    (f64::from(translation[2]) - f64::from(p[2])) as f32,
                    uvs[vertex][0],
                    uvs[vertex][1],
                ];
                for (i, value) in actual.into_iter().enumerate() {
                    assert_eq!(
                        value.to_bits(),
                        word(&oracle, at + i * 4),
                        "{name} vertex{count} field{i}"
                    );
                }
                count += 1;
                at += 20;
            }
            assert_eq!(count, expected_count, "{name}");
        }
        assert_eq!(at, oracle.len());
    }

    #[test]
    fn rejects_truncated_and_trailing_mesh_data() {
        assert!(CachedMesh::parse(&[0; 163]).is_err());
        let mut empty = vec![0; 164];
        assert!(CachedMesh::parse(&empty).is_ok());
        empty.push(0);
        assert!(CachedMesh::parse(&empty).is_err());
        empty.truncate(164);
        empty[0] = 1;
        assert!(CachedMesh::parse(&empty).is_err());
    }

    #[test]
    #[ignore = "requires original archive.DIR and archive.bin"]
    fn parses_original_cached_mesh_corpus() {
        let read = |name| std::fs::read(std::env::var(name).expect(name)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let bytes = read("OPENMT2001_ARCHIVE_BIN");
        let index = crate::archive::ArchiveIndex::parse(&directory, bytes.len() as u64).unwrap();
        let mut count = 0;
        for entry in index
            .entries
            .iter()
            .filter(|e| e.path.to_ascii_lowercase().ends_with(".tm"))
        {
            let mesh = CachedMesh::parse(
                &bytes[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap_or_else(|e| panic!("{}: {e}", entry.path));
            if entry
                .path
                .eq_ignore_ascii_case("gamedata2\\roads\\roads.tm")
            {
                assert_eq!(mesh.vertex_count, 544);
                assert_eq!(mesh.positions(0).unwrap().len(), 544);
                assert_eq!(
                    mesh.parts
                        .iter()
                        .map(|p| p.indices.len() / 6)
                        .sum::<usize>(),
                    272
                );
                assert_eq!(mesh.texture_names.len(), 2);
            }
            count += 1;
        }
        assert_eq!(count, 468);
    }
}
