//! Original004bebf0 vehicle registration and004dfa40 model binding.
use super::{ModelExtent, float, put, runtime::VehicleModels, word};
use crate::{archive::ArchiveIndex, mesh::CachedMesh, pe::PeImage};

pub struct Models<'a> {
    paths: Vec<String>,
    meshes: Vec<Option<CachedMesh<'a>>>,
    definitions: Vec<Option<[u8; 116]>>,
}
impl<'a> Models<'a> {
    pub fn from_original(
        image: &PeImage<'_>,
        index: &ArchiveIndex,
        archive: &'a [u8],
    ) -> Result<Self, String> {
        let mut paths = Vec::with_capacity(174);
        let mut meshes = Vec::with_capacity(174);
        let mut definitions = Vec::with_capacity(87);
        for pair in PATHS {
            for (lod, address) in pair.into_iter().enumerate() {
                let bytes = image.at(address, 256)?;
                let end = bytes
                    .iter()
                    .position(|&b| b == 0)
                    .ok_or("unterminated vehicle path")?;
                let path =
                    std::str::from_utf8(&bytes[..end]).map_err(|_| "invalid vehicle path")?;
                let mesh = if let Some(entry) = index.find(path) {
                    let start =
                        usize::try_from(entry.offset).map_err(|_| "vehicle offset overflow")?;
                    let length =
                        usize::try_from(entry.length).map_err(|_| "vehicle length overflow")?;
                    let end = start.checked_add(length).ok_or("vehicle range overflow")?;
                    Some(CachedMesh::parse(
                        archive.get(start..end).ok_or("vehicle outside archive")?,
                    )?)
                } else {
                    None
                };
                if lod == 0 {
                    definitions.push(
                        mesh.as_ref()
                            .map(CachedMesh::model_definition)
                            .transpose()?,
                    );
                }
                paths.push(path.to_owned());
                meshes.push(mesh);
            }
        }
        Ok(Self {
            paths,
            meshes,
            definitions,
        })
    }
    pub fn path(&self, slot: usize, lod: bool) -> Option<&str> {
        self.paths
            .get(slot.checked_mul(2)?.checked_add(usize::from(lod))?)
            .map(String::as_str)
    }
    pub fn source(&self, slot: usize, lod: bool) -> Option<&CachedMesh<'a>> {
        self.meshes
            .get(slot.checked_mul(2)?.checked_add(usize::from(lod))?)?
            .as_ref()
    }
    pub fn definition(&self, slot: usize) -> Option<&[u8; 116]> {
        self.definitions.get(slot)?.as_ref()
    }
    fn slot(reference: u32) -> Result<usize, String> {
        let offset = reference
            .checked_sub(0x689080)
            .ok_or("vehicle model reference below table")?;
        if offset % 16 != 0 || offset / 16 >= 87 {
            return Err("vehicle model reference outside table".into());
        }
        Ok((offset / 16) as usize)
    }
}
impl VehicleModels for Models<'_> {
    fn bind(&mut self, reference: u32, auxiliary: &mut [u8; 284]) -> Result<(), String> {
        let slot = Self::slot(reference)?;
        let definition = self.definition(slot).ok_or_else(|| {
            format!(
                "missing vehicle model {}",
                self.path(slot, false).unwrap_or("unknown")
            )
        })?;
        put(auxiliary, 0, reference);
        auxiliary[0x44..0xb8].copy_from_slice(definition);
        Ok(())
    }
    fn extent(&self, auxiliary: &[u8; 284]) -> Result<ModelExtent, String> {
        let slot = Self::slot(word(auxiliary, 0))?;
        let definition = self.definition(slot).ok_or("vehicle model has no bounds")?;
        Ok(ModelExtent {
            low: float(definition, 8),
            high: float(definition, 92),
        })
    }
}

// Filename operands in original table689080 row order.
const PATHS: [[u32; 2]; 87] = [
    [0x54b470, 0x54af80],
    [0x54b44c, 0x54af58],
    [0x54b420, 0x54af28],
    [0x54b3f4, 0x54aef8],
    [0x54b3c8, 0x54aec8],
    [0x54b3a4, 0x54ae9c],
    [0x54b378, 0x54ae6c],
    [0x54b34c, 0x54ae3c],
    [0x54b320, 0x54ae0c],
    [0x54b2f4, 0x54addc],
    [0x54b2c8, 0x54adac],
    [0x54b29c, 0x54ad7c],
    [0x54b26c, 0x54ad48],
    [0x54b23c, 0x54ad14],
    [0x54b20c, 0x54ace0],
    [0x54b1e0, 0x54acac],
    [0x54b1b0, 0x54ac78],
    [0x54b180, 0x54ac44],
    [0x54b154, 0x54ac14],
    [0x54b128, 0x54abe0],
    [0x54b0fc, 0x54abac],
    [0x54b0d0, 0x54ab7c],
    [0x54b0a4, 0x54ab48],
    [0x54b078, 0x54ab18],
    [0x54b050, 0x54aaec],
    [0x54b028, 0x54aac0],
    [0x54b000, 0x54aa94],
    [0x54afd8, 0x54aa68],
    [0x54afb0, 0x54aa3c],
    [0x54aa10, 0x54a51c],
    [0x54a9ec, 0x54a4f4],
    [0x54a9c0, 0x54a4c4],
    [0x54a994, 0x54a494],
    [0x54a968, 0x54a464],
    [0x54a944, 0x54a438],
    [0x54a918, 0x54a408],
    [0x54a8ec, 0x54a3d8],
    [0x54a8c0, 0x54a3a8],
    [0x54a894, 0x54a378],
    [0x54a868, 0x54a348],
    [0x54a83c, 0x54a318],
    [0x54a80c, 0x54a2e4],
    [0x54a7dc, 0x54a2b0],
    [0x54a7ac, 0x54a27c],
    [0x54a77c, 0x54a248],
    [0x54a74c, 0x54a214],
    [0x54a71c, 0x54a1e0],
    [0x54a6f0, 0x54a1ac],
    [0x54a6c4, 0x54a17c],
    [0x54a698, 0x54a148],
    [0x54a66c, 0x54a118],
    [0x54a640, 0x54a0e8],
    [0x54a614, 0x54a0b8],
    [0x54a5ec, 0x54a08c],
    [0x54a5c4, 0x54a060],
    [0x54a59c, 0x54a034],
    [0x54a574, 0x54a008],
    [0x54a54c, 0x549fdc],
    [0x549fb0, 0x549ac0],
    [0x549f8c, 0x549a98],
    [0x549f60, 0x549a68],
    [0x549f34, 0x549a38],
    [0x549f08, 0x549a08],
    [0x549ee4, 0x5499dc],
    [0x549eb8, 0x5499ac],
    [0x549e8c, 0x549978],
    [0x549e60, 0x549948],
    [0x549e34, 0x549914],
    [0x549e08, 0x5498e4],
    [0x549ddc, 0x5498b4],
    [0x549dac, 0x549880],
    [0x549d7c, 0x54984c],
    [0x549d4c, 0x549818],
    [0x549d20, 0x5497e4],
    [0x549cf0, 0x5497b0],
    [0x549cc0, 0x54977c],
    [0x549c94, 0x54974c],
    [0x549c68, 0x549718],
    [0x549c3c, 0x5496e4],
    [0x549c10, 0x5496b4],
    [0x549be4, 0x549684],
    [0x549bb8, 0x549654],
    [0x549b90, 0x549628],
    [0x549b68, 0x5495fc],
    [0x549b40, 0x5495d0],
    [0x549b18, 0x5495a4],
    [0x549af0, 0x549578],
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires original vehicle assets"]
    fn loads_road_admission_models() {
        let read = |key| std::fs::read(crate::fixtures::path(key)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            ArchiveIndex::parse(&read("OPENMT2001_ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut models = Models::from_original(&image, &index, &archive).unwrap();
        let data = read("OPENMT2001_NATIVE_VEHICLE_MODELS");
        assert_eq!(&data[..8], b"MTVMODL1");
        let mut at = 20;
        for _ in 0..word(&data, 16) {
            let slot = word(&data, at + 4) as usize;
            let lod = word(&data, at + 8) != 0;
            assert_eq!(word(&data, at + 12), 0x689080 + slot as u32 * 16);
            let length = word(&data, at + 32) as usize;
            let path_length = word(&data, at + 36) as usize;
            at += 40;
            assert_eq!(
                models.path(slot, lod).unwrap().as_bytes(),
                &data[at..at + path_length]
            );
            at += path_length;
            if length != 0 {
                let definition = models
                    .source(slot, lod)
                    .unwrap()
                    .model_definition()
                    .unwrap();
                assert_eq!(
                    &definition[..],
                    &data[at..at + length],
                    "slot{slot} lod{lod}"
                );
            } else {
                assert!(models.source(slot, lod).is_none());
            }
            at += length;
        }
        assert_eq!(at, data.len());
        for era in 0..3 {
            for kind in [0, 3].into_iter().chain(5..24) {
                let slot = era * 29 + kind;
                let mut auxiliary = [0; 284];
                models
                    .bind(0x689080 + slot as u32 * 16, &mut auxiliary)
                    .unwrap();
                let extent = models.extent(&auxiliary).unwrap();
                assert!(extent.high > extent.low);
                assert!(models.source(slot, false).is_some());
            }
        }
    }
}
