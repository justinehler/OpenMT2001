//! Read-only PE32 image access. This never loads or executes the image.

pub struct PeImage<'a> {
    bytes: &'a [u8],
    base: u32,
    sections: Vec<Section>,
}

struct Section {
    rva: u32,
    raw_offset: usize,
    raw_size: usize,
}

impl<'a> PeImage<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let u16_at = |at| -> Result<u16, String> {
            Ok(u16::from_le_bytes(
                bytes
                    .get(at..at + 2)
                    .ok_or("truncated PE")?
                    .try_into()
                    .unwrap(),
            ))
        };
        let u32_at = |at| -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                bytes
                    .get(at..at + 4)
                    .ok_or("truncated PE")?
                    .try_into()
                    .unwrap(),
            ))
        };
        if bytes.get(..2) != Some(b"MZ") {
            return Err("missing DOS signature".into());
        }
        let pe = u32_at(0x3c)? as usize;
        // Bound the header before doing arithmetic on untrusted offsets (including wasm32).
        if pe > bytes.len().saturating_sub(24) {
            return Err("PE header outside file".into());
        }
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0") || u16_at(pe + 4)? != 0x14c {
            return Err("expected x86 PE image".into());
        }
        let count = usize::from(u16_at(pe + 6)?);
        let optional_size = usize::from(u16_at(pe + 20)?);
        let optional = pe + 24;
        if optional_size < 32 || u16_at(optional)? != 0x10b {
            return Err("expected PE32 optional header".into());
        }
        let base = u32_at(optional + 28)?;
        let table = optional
            .checked_add(optional_size)
            .ok_or("PE table overflow")?;
        let end = table.checked_add(count * 40).ok_or("PE table overflow")?;
        if end > bytes.len() {
            return Err("truncated section table".into());
        }
        let mut sections = Vec::with_capacity(count);
        for n in 0..count {
            let at = table + n * 40;
            let raw_size = u32_at(at + 16)? as usize;
            let raw_offset = u32_at(at + 20)? as usize;
            if raw_offset
                .checked_add(raw_size)
                .is_none_or(|end| end > bytes.len())
            {
                return Err("section outside file".into());
            }
            sections.push(Section {
                rva: u32_at(at + 12)?,
                raw_offset,
                raw_size,
            });
        }
        Ok(Self {
            bytes,
            base,
            sections,
        })
    }

    /// Return initialized bytes at a preferred virtual address, without relocations.
    pub fn at(&self, address: u32, length: usize) -> Result<&'a [u8], String> {
        let rva = address
            .checked_sub(self.base)
            .ok_or("address precedes image")?;
        for section in &self.sections {
            if let Some(delta) = rva.checked_sub(section.rva) {
                let delta = delta as usize;
                if delta
                    .checked_add(length)
                    .is_some_and(|end| end <= section.raw_size)
                {
                    return Ok(&self.bytes
                        [section.raw_offset + delta..section.raw_offset + delta + length]);
                }
            }
        }
        Err(format!(
            "address {address:08x} length {length} is not backed by image bytes"
        ))
    }

    pub fn f32_at(&self, address: u32) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.at(address, 4)?.try_into().unwrap()))
    }
}
