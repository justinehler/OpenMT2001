//! Original004be960 transport registrations, retained independently of rendering.
use crate::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage};

pub struct Models<'a> {
    entries: Vec<Entry<'a>>,
}
struct Entry<'a> {
    reference: u32,
    mesh: CachedMesh<'a>,
    definition: [u8; 116],
}
impl<'a> Models<'a> {
    #[cfg(test)]
    pub(crate) fn from_fixture_entries(meshes: Vec<(u32, CachedMesh<'a>)>) -> Result<Self, String> {
        let entries = meshes
            .into_iter()
            .map(|(reference, mesh)| {
                let definition = mesh.model_definition()?;
                Ok(Entry {
                    reference,
                    mesh,
                    definition,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self { entries })
    }
    #[cfg(test)]
    pub(crate) fn from_locator_fixture(
        reference: u32,
        mesh: CachedMesh<'a>,
    ) -> Result<Self, String> {
        let definition = mesh.model_definition()?;
        Ok(Self {
            entries: vec![Entry {
                reference,
                mesh,
                definition,
            }],
        })
    }
    pub fn from_original(
        image: &PeImage<'_>,
        index: &ArchiveIndex,
        archive: &'a [u8],
    ) -> Result<Self, String> {
        let mut entries = Vec::with_capacity(REGISTRATIONS.len());
        for (reference, address) in REGISTRATIONS {
            let path = image.at(address, 256)?;
            let end = path
                .iter()
                .position(|&b| b == 0)
                .ok_or("unterminated transport path")?;
            let path = std::str::from_utf8(&path[..end]).map_err(|_| "invalid transport path")?;
            let entry = index
                .find(path)
                .ok_or_else(|| format!("missing transport model {path}"))?;
            let start = usize::try_from(entry.offset).map_err(|_| "transport offset overflow")?;
            let end = usize::try_from(
                entry
                    .offset
                    .checked_add(entry.length)
                    .ok_or("transport range overflow")?,
            )
            .map_err(|_| "transport range overflow")?;
            let mesh = CachedMesh::parse(
                archive
                    .get(start..end)
                    .ok_or("transport model outside archive")?,
            )?;
            let definition = mesh.model_definition()?;
            entries.push(Entry {
                reference,
                mesh,
                definition,
            });
        }
        Ok(Self { entries })
    }
    pub fn mesh(&self, reference: u32) -> Result<&CachedMesh<'a>, String> {
        Ok(&self.entry(reference)?.mesh)
    }
    /// Original registration order, used by the frontend material catalogue.
    pub fn registered_meshes(&self) -> impl Iterator<Item = (u32, &CachedMesh<'a>)> {
        self.entries.iter().map(|entry| (entry.reference, &entry.mesh))
    }
    fn entry(&self, reference: u32) -> Result<&Entry<'a>, String> {
        self.entries
            .iter()
            .find(|entry| entry.reference == reference)
            .ok_or_else(|| format!("unknown transport registration {reference:08x}"))
    }
    pub fn crane_position(&self) -> Result<[f32; 3], String> {
        let definition = &self.entry(0x6883b0)?.definition;
        Ok(std::array::from_fn(|i| {
            f32::from_le_bytes(definition[0x60 + i * 4..0x64 + i * 4].try_into().unwrap())
        }))
    }
    /// Original004dfa40: registration identity plus the116-byte model definition.
    pub fn bind(&self, reference: u32, auxiliary: &mut [u8; 284]) -> Result<(), String> {
        let entry = self.entry(reference)?;
        crate::resident::bind_auxiliary_model(auxiliary, reference, &entry.definition);
        Ok(())
    }
    pub fn routes(&self) -> Result<[super::RouteGeometry; 8], String> {
        (0..8)
            .map(|i| super::RouteGeometry::from_mesh(self.mesh(0x6860e8 + i * 16)?))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "transport route count".into())
    }
    /// Original0044de30 transforms four locator vertices into the engine's
    /// twelve-float matrix. Its signed frame wrap deliberately skips frame0.
    pub fn locator_matrix(&self, reference: u32, tick: u32) -> Result<[f32; 12], String> {
        locator_matrix(self.mesh(reference)?, tick)
    }
}

pub fn locator_matrix(mesh: &CachedMesh<'_>, tick: u32) -> Result<[f32; 12], String> {
    use crate::x87::Sum;
    let last = i32::try_from(mesh.frames).map_err(|_| "locator frame count overflow")? - 1;
    if last <= 0 || mesh.vertex_count < 4 {
        return Err("locator requires four vertices and at least two frames".into());
    }
    let tick = tick as i32;
    let frame = if tick >= last { tick % last + 1 } else { tick };
    let points = mesh.positions(usize::try_from(frame).map_err(|_| "negative locator frame")?)?;
    let mut result = [0.; 12];
    result[9..].copy_from_slice(&[points[0][0], points[0][1], -points[0][2]]);
    for row in 0..3 {
        for axis in 0..3 {
            let mut difference = Sum::from_f32(points[row + 1][axis])?;
            difference.add(Sum::from_f32(points[0][axis])?.negated());
            result[row * 3 + axis] = difference.to_f32();
        }
    }
    // Original525264 is -1f; these are separate stored products, after all
    // differences have been rounded to f32 by FSTP.
    for index in [6, 2, 5, 7] {
        result[index] = -result[index];
    }
    Ok(result)
}

// (registration identity, original filename operand), in004be960 call order.
const REGISTRATIONS: [(u32, u32); 35] = [
    (0x6887e0, 0x54953c),
    (0x6887f0, 0x549520),
    (0x688800, 0x549504),
    (0x685cc8, 0x5494dc),
    (0x685cd8, 0x5494b8),
    (0x687550, 0x54949c),
    (0x687560, 0x54947c),
    (0x687570, 0x54945c),
    (0x687c58, 0x549438),
    (0x688880, 0x549418),
    (0x688890, 0x5493f8),
    (0x6888a0, 0x5493d8),
    (0x6860e8, 0x5493b0),
    (0x6860f8, 0x549384),
    (0x686108, 0x549358),
    (0x686118, 0x54932c),
    (0x686128, 0x549304),
    (0x686138, 0x5492d8),
    (0x686148, 0x5492ac),
    (0x686158, 0x54927c),
    (0x688db0, 0x54925c),
    (0x688dc0, 0x54923c),
    (0x688dd0, 0x54921c),
    (0x688de0, 0x5491fc),
    (0x687790, 0x5491d8),
    (0x6877a0, 0x5491b4),
    (0x6877b0, 0x549190),
    (0x6877c0, 0x54916c),
    (0x689970, 0x549148),
    (0x6896e8, 0x549124),
    (0x688b98, 0x549100),
    (0x687830, 0x5490e0),
    (0x687690, 0x5490b4),
    (0x6876a0, 0x549084),
    // Crane base registered separately at4bdb11, read by44ec50 every tick.
    (0x6883b0, 0x547668),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires supplied original executable and archive"]
    fn original_transport_registrations_load() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            ArchiveIndex::parse(&read("OPENMT2001_ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let models = Models::from_original(&image, &index, &archive).unwrap();
        let routes = models.routes().unwrap();
        let previous = super::super::RouteGeometry::load_routes(&image, &index, &archive).unwrap();
        for (actual, expected) in routes.iter().zip(previous) {
            assert_eq!(
                actual.start.map(f32::to_bits),
                expected.start.map(f32::to_bits)
            );
            assert_eq!(actual.end.map(f32::to_bits), expected.end.map(f32::to_bits));
        }
        for reference in [0x6887e0, 0x6887f0, 0x688800, 0x687550] {
            let mut auxiliary = [0xa5; 284];
            models.bind(reference, &mut auxiliary).unwrap();
            assert_eq!(&auxiliary[..4], &reference.to_le_bytes());
            assert!(models.mesh(reference).unwrap().vertex_count > 0);
            assert_eq!(&auxiliary[4..0x44], &[0xa5; 0x40]);
            assert_eq!(&auxiliary[0xb8..], &[0xa5; 100]);
        }
    }
}
