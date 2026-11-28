use openmt2001::save::{SaveFile, extension};

pub const WORDS: [u32; 4] = [200001, 1950, 120.5f32.to_bits(), 3];

pub struct Case {
    pub name: String,
    pub bytes: Vec<u8>,
    pub readable: bool,
    pub full_error: Option<&'static str>,
}

impl Case {
    pub fn check_full_parse(&self) {
        match self.full_error {
            Some(message) => assert_eq!(SaveFile::parse(&self.bytes).unwrap_err(), message),
            None => assert!(SaveFile::parse(&self.bytes).is_ok(), "{}", self.name),
        }
    }
}

fn record(bytes: &mut Vec<u8>, tag: u32, payload: &[u8]) {
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
}

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for version in [None, Some(1), Some(6)] {
        let format = version.map_or_else(|| "original".into(), |v| format!("v2-version{v}"));
        let mut bytes = Vec::new();
        record(&mut bytes, 1, &[0x5a; 40]);
        record(
            &mut bytes,
            0x2000,
            &WORDS
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let metadata_end = bytes.len();
        record(&mut bytes, 4, &[0; 32]);
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        if let Some(version) = version {
            let mut records = (1..=13).map(|tag| (tag, vec![0; 32])).collect::<Vec<_>>();
            if version == 6 {
                records.extend((16..=18).map(|tag| (tag, vec![0; 32])));
            }
            extension::append_version(&mut bytes, &records, version).unwrap();
            extension::Extension::parse(&SaveFile::parse(&bytes).unwrap())
                .unwrap()
                .unwrap()
                .validate_runtime_section_set()
                .unwrap();
        }
        let mut mismatch = bytes.clone();
        mismatch[8..48].fill(0xa5);
        let mut variants = vec![
            ("intact", bytes.clone(), true, None),
            (
                "truncated-world",
                bytes[..metadata_end + 8 + 7].to_vec(),
                true,
                Some("truncated save section payload"),
            ),
            (
                "truncated-metadata",
                bytes[..metadata_end - 1].to_vec(),
                false,
                Some("truncated save section payload"),
            ),
            ("compatibility-mismatch", mismatch, true, None),
        ];
        if version.is_some() {
            variants.push((
                "truncated-extension",
                bytes[..bytes.len() - 8 - 1].to_vec(),
                true,
                Some("truncated save section payload"),
            ));
        }
        cases.extend(
            variants
                .into_iter()
                .map(|(name, bytes, readable, full_error)| Case {
                    name: format!("{format}-{name}"),
                    bytes,
                    readable,
                    full_error,
                }),
        );
    }
    cases
}
