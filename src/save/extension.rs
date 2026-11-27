//! Explicit V2 runtime records embedded beside the recovered native records.
//! This extension is not evidence of original-game compatibility. No host I/O.
use super::SaveFile;
pub const TAG: u32 = 0x56320001;
const MAGIC: &[u8; 8] = b"OMT2SAVE";
pub const VERSION: u32 = 1;
// VERSION1: frontend base runtime records1..13.
// VERSION2: base plus14 auction continuation OR15 strategic host view.
// VERSION3: base plus16 independent placement RNG, optionally14 OR15.
// VERSION4: VERSION3 plus17 complete portable dust pool and independent RNG.
// VERSION5: VERSION4 plus18 legacy empty-map offline Skirmish selection.
// VERSION6: same exact section set, with versioned map provenance in18.
// The frontend validates each exact record set; this container keeps payloads
// opaque and retains VERSION1 as the default for legacy append callers.
pub struct Extension<'a> {
    pub version: u32,
    pub records: std::collections::BTreeMap<u32, &'a [u8]>,
}
impl<'a> Extension<'a> {
    /// The retained frontend's exact supported runtime record sets.
    pub fn validate_runtime_section_set(&self) -> Result<(), String> {
        let auction = self.records.contains_key(&14);
        let view = self.records.contains_key(&15);
        let valid = match self.version {
            1 => self.records.len() == 13,
            2 => self.records.len() == 14 && auction != view,
            3 => {
                self.records.contains_key(&16)
                    && !(auction && view)
                    && self.records.len() == 14 + usize::from(auction || view)
            }
            4 => {
                self.records.contains_key(&16)
                    && self.records.contains_key(&17)
                    && !(auction && view)
                    && self.records.len() == 15 + usize::from(auction || view)
            }
            5 | 6 => {
                self.records.contains_key(&16)
                    && self.records.contains_key(&17)
                    && self.records.contains_key(&18)
                    && !(auction && view)
                    && self.records.len() == 16 + usize::from(auction || view)
            }
            _ => false,
        };
        if !(1..=13).all(|tag| self.records.contains_key(&tag)) || !valid {
            return Err("V2 runtime section set is incomplete or unsupported".into());
        }
        Ok(())
    }
    pub fn parse(save: &SaveFile<'a>) -> Result<Option<Self>, String> {
        let mut sections = save.sections.iter().filter(|s| s.tag == TAG);
        let Some(section) = sections.next() else {
            return Ok(None);
        };
        if sections.next().is_some() {
            return Err("duplicate V2 save extension".into());
        }
        let bytes = section.payload;
        let version = bytes
            .get(8..12)
            .map(|row| u32::from_le_bytes(row.try_into().unwrap()));
        if bytes.get(..8) != Some(MAGIC) || !matches!(version, Some(1..=6)) {
            return Err("unsupported V2 save extension version".into());
        }
        let mut at = 12;
        let mut records = std::collections::BTreeMap::new();
        while at < bytes.len() {
            let header = bytes
                .get(at..at + 8)
                .ok_or("truncated V2 extension header")?;
            let tag = u32::from_le_bytes(header[..4].try_into().unwrap());
            let size = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            at += 8;
            let end = at.checked_add(size).ok_or("V2 extension length overflow")?;
            let payload = bytes.get(at..end).ok_or("truncated V2 extension record")?;
            if records.insert(tag, payload).is_some() {
                return Err("duplicate V2 extension record".into());
            }
            at = end;
        }
        Ok(Some(Self {
            version: version.unwrap(),
            records,
        }))
    }
    pub fn required(&self, tag: u32) -> Result<&'a [u8], String> {
        self.records
            .get(&tag)
            .copied()
            .ok_or_else(|| format!("V2 save missing runtime record {tag}"))
    }
}
/// Append a single versioned extension before the native eight-byte terminator.
pub fn append(native: &mut Vec<u8>, records: &[(u32, Vec<u8>)]) -> Result<(), String> {
    append_version(native, records, VERSION)
}
pub fn append_version(
    native: &mut Vec<u8>,
    records: &[(u32, Vec<u8>)],
    version: u32,
) -> Result<(), String> {
    if !matches!(version, 1..=6) {
        return Err("unsupported V2 save extension output version".into());
    }
    let save = SaveFile::parse(native)?;
    if Extension::parse(&save)?.is_some() {
        return Err("save already contains a V2 extension".into());
    }
    let mut payload = MAGIC.to_vec();
    payload.extend_from_slice(&version.to_le_bytes());
    let mut seen = std::collections::BTreeSet::new();
    for (tag, row) in records {
        if !seen.insert(*tag) {
            return Err("duplicate V2 extension output record".into());
        }
        payload.extend_from_slice(&tag.to_le_bytes());
        payload.extend_from_slice(
            &u32::try_from(row.len())
                .map_err(|_| "V2 record exceeds DWORD")?
                .to_le_bytes(),
        );
        payload.extend_from_slice(row);
    }
    let terminator = native.split_off(native.len() - 8);
    native.extend_from_slice(&TAG.to_le_bytes());
    native.extend_from_slice(
        &u32::try_from(payload.len())
            .map_err(|_| "V2 extension exceeds DWORD")?
            .to_le_bytes(),
    );
    native.extend_from_slice(&payload);
    native.extend_from_slice(&terminator);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supported_runtime_versions_require_exact_records() {
        for version in 1..=6 {
            let mut records: std::collections::BTreeMap<u32, &[u8]> =
                (1..=13).map(|tag| (tag, &[][..])).collect();
            if version == 2 {
                records.insert(15, &[]);
            }
            if version >= 3 {
                records.insert(16, &[]);
            }
            if version >= 4 {
                records.insert(17, &[]);
            }
            if version >= 5 {
                records.insert(18, &[]);
            }
            let mut extension = Extension { version, records };
            extension.validate_runtime_section_set().unwrap();
            if version >= 3 {
                extension.records.insert(14, &[]);
                extension.validate_runtime_section_set().unwrap();
                extension.records.insert(15, &[]);
                assert!(extension.validate_runtime_section_set().is_err());
                extension.records.remove(&15);
            }
            if version >= 5 {
                extension.records.remove(&18);
                assert!(extension.validate_runtime_section_set().is_err());
                extension.records.insert(18, &[]);
            }
            extension.records.insert(19, &[]);
            assert!(extension.validate_runtime_section_set().is_err());
            extension.records.remove(&19);
            extension.records.remove(&7);
            assert!(extension.validate_runtime_section_set().is_err());
        }
    }
}
