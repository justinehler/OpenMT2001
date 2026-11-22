//! Reader for the original `archive.DIR` file table.
//!
//! The supplied original index is a little-endian table.  Its 32-byte header is
//! followed by fixed-size 308-byte records.  A record contains a 256-byte
//! NUL-terminated path and thirteen little-endian words.  The second and
//! third words are the offset and stored length into `archive.bin`; all words
//! are retained because their original consumers have not yet been recovered.
//! In the supplied corpus, record words 0 and 8 through 12 are always zero,
//! while words 5 through 7 vary; they are therefore preserved as raw words
//! rather than being treated as padding.

use core::fmt;

const HEADER_SIZE: usize = 32;
const RECORD_SIZE: usize = 308;
const PATH_SIZE: usize = 256;
const HEADER_WORDS: usize = HEADER_SIZE / 4;
const RECORD_WORDS: usize = (RECORD_SIZE - PATH_SIZE) / 4;
const SUPPORTED_VERSION: u32 = 3;

/// One file listed by `archive.DIR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// The path exactly as represented by the index (without its NUL padding).
    pub path: String,
    /// Byte offset into the associated `archive.bin` payload.
    pub offset: u64,
    /// Number of stored bytes in the associated `archive.bin` payload.
    pub length: u64,
    /// All thirteen little-endian words following the path, in original
    /// order.  The words at indices 1 and 2 are exposed as `offset` and
    /// `length`; the other words remain deliberately opaque.
    pub metadata: [u32; RECORD_WORDS],
}

/// Parsed original archive index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveIndex {
    /// All eight little-endian header words, in original order.
    pub header_words: [u32; HEADER_WORDS],
    /// Header word zero.  The original input contains `3`.
    pub version: u32,
    /// Header word two, retained as an opaque format field.
    pub header_flags: u32,
    /// Entries in their original on-disk order.
    pub entries: Vec<ArchiveEntry>,
}

impl ArchiveIndex {
    /// Parse an `archive.DIR` byte slice and validate every payload range.
    ///
    /// `payload_len` must be the byte length of the associated `archive.bin`.
    /// The index stores 32-bit offsets and lengths, but the public values use
    /// `u64` so callers can compare them directly with a file length without
    /// narrowing arithmetic.
    pub fn parse(bytes: &[u8], payload_len: u64) -> Result<Self, ArchiveIndexError> {
        if bytes.len() < HEADER_SIZE {
            return Err(ArchiveIndexError::TruncatedHeader {
                actual: bytes.len(),
            });
        }

        let header_words = std::array::from_fn(|word| read_u32(bytes, word * 4));
        let version = header_words[0];
        if version != SUPPORTED_VERSION {
            return Err(ArchiveIndexError::UnsupportedVersion { version });
        }
        let count = header_words[1] as usize;
        let header_flags = header_words[2];
        let records_len = count
            .checked_mul(RECORD_SIZE)
            .ok_or(ArchiveIndexError::RecordTableOverflow { count })?;
        let expected_len = HEADER_SIZE
            .checked_add(records_len)
            .ok_or(ArchiveIndexError::RecordTableOverflow { count })?;
        if bytes.len() != expected_len {
            return Err(ArchiveIndexError::InvalidTableLength {
                expected: expected_len,
                actual: bytes.len(),
            });
        }

        let mut entries = Vec::with_capacity(count);
        for index in 0..count {
            let record_start = HEADER_SIZE + index * RECORD_SIZE;
            let path_bytes = &bytes[record_start..record_start + PATH_SIZE];
            let path_end = path_bytes
                .iter()
                .position(|&byte| byte == 0)
                .unwrap_or(PATH_SIZE);
            let path = String::from_utf8(path_bytes[..path_end].to_vec())
                .map_err(|_| ArchiveIndexError::InvalidPath { record: index })?;

            let metadata_start = record_start + PATH_SIZE;
            let metadata = std::array::from_fn(|word| read_u32(bytes, metadata_start + word * 4));
            let offset = u64::from(metadata[1]);
            let length = u64::from(metadata[2]);
            let end = offset
                .checked_add(length)
                .ok_or(ArchiveIndexError::RangeOverflow {
                    record: index,
                    offset,
                    length,
                })?;
            if end > payload_len {
                return Err(ArchiveIndexError::PayloadOutOfBounds {
                    record: index,
                    offset,
                    length,
                    payload_len,
                });
            }

            entries.push(ArchiveEntry {
                path,
                offset,
                length,
                metadata,
            });
        }

        Ok(Self {
            header_words,
            version,
            header_flags,
            entries,
        })
    }

    /// Find an entry using a convenience key that lowercases ASCII and treats
    /// either slash as a separator.  This is an application helper; it does
    /// not assert that the original game used these lookup semantics.
    pub fn find(&self, requested_path: &str) -> Option<&ArchiveEntry> {
        let key = normalized_key(requested_path);
        self.entries
            .iter()
            .find(|entry| normalized_key(&entry.path) == key)
    }
}

/// Errors encountered while decoding or validating an archive index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveIndexError {
    TruncatedHeader {
        actual: usize,
    },
    UnsupportedVersion {
        version: u32,
    },
    RecordTableOverflow {
        count: usize,
    },
    InvalidTableLength {
        expected: usize,
        actual: usize,
    },
    InvalidPath {
        record: usize,
    },
    RangeOverflow {
        record: usize,
        offset: u64,
        length: u64,
    },
    PayloadOutOfBounds {
        record: usize,
        offset: u64,
        length: u64,
        payload_len: u64,
    },
}

impl core::error::Error for ArchiveIndexError {}

impl fmt::Display for ArchiveIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { actual } => {
                write!(
                    formatter,
                    "archive index header is truncated ({actual} bytes)"
                )
            }
            Self::UnsupportedVersion { version } => {
                write!(formatter, "unsupported archive index version {version}")
            }
            Self::RecordTableOverflow { count } => {
                write!(
                    formatter,
                    "archive record table size overflows for {count} records"
                )
            }
            Self::InvalidTableLength { expected, actual } => write!(
                formatter,
                "archive index has {actual} bytes; expected exactly {expected}"
            ),
            Self::InvalidPath { record } => {
                write!(formatter, "archive record {record} has a non-UTF-8 path")
            }
            Self::RangeOverflow {
                record,
                offset,
                length,
            } => write!(
                formatter,
                "archive record {record} range overflows (offset {offset}, length {length})"
            ),
            Self::PayloadOutOfBounds {
                record,
                offset,
                length,
                payload_len,
            } => write!(
                formatter,
                "archive record {record} range [{offset}, {}) exceeds payload length {payload_len}",
                offset.saturating_add(*length)
            ),
        }
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn normalized_key(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(path: &[u8], offset: u32, length: u32) -> Vec<u8> {
        let mut words = [0; RECORD_WORDS];
        words[1] = offset;
        words[2] = length;
        fixture_with_words(path, words)
    }

    fn fixture_with_words(path: &[u8], words: [u32; RECORD_WORDS]) -> Vec<u8> {
        let mut bytes = vec![0; HEADER_SIZE + RECORD_SIZE];
        bytes[0..4].copy_from_slice(&SUPPORTED_VERSION.to_le_bytes());
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        bytes[HEADER_SIZE..HEADER_SIZE + path.len()].copy_from_slice(path);
        let metadata_start = HEADER_SIZE + PATH_SIZE;
        for (word, value) in words.into_iter().enumerate() {
            bytes[metadata_start + word * 4..metadata_start + word * 4 + 4]
                .copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn parses_record_and_normalizes_lookup_key() {
        let bytes = fixture(b"GameData2\\Foo.TM\0", 3, 4);
        let index = ArchiveIndex::parse(&bytes, 7).expect("valid fixture");
        assert_eq!(index.entries[0].path, "GameData2\\Foo.TM");
        assert_eq!(index.entries[0].offset, 3);
        assert_eq!(index.entries[0].length, 4);
        assert!(index.find("gamedata2/foo.tm").is_some());
    }

    #[test]
    fn retains_all_record_words() {
        let mut words = [0; RECORD_WORDS];
        words[0] = 0x1020_3040;
        words[1] = 3;
        words[2] = 4;
        words[3] = 0x5060_7080;
        words[4] = 0x90a0_b0c0;
        words[5] = 0xd0e0_f001;
        words[12] = 0x2345_6789;
        let bytes = fixture_with_words(b"foo\0", words);
        let index = ArchiveIndex::parse(&bytes, 7).expect("valid fixture");
        assert_eq!(index.entries[0].metadata, words);
    }

    #[test]
    fn retains_all_header_words() {
        let mut bytes = fixture(b"foo\0", 0, 0);
        let expected = [
            SUPPORTED_VERSION,
            1,
            1,
            0x1020_3040,
            0x5060_7080,
            0x90a0_b0c0,
            0xd0e0_f001,
            0x2345_6789,
        ];
        for (word, value) in expected.into_iter().enumerate() {
            bytes[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        let index = ArchiveIndex::parse(&bytes, 0).expect("valid fixture");
        assert_eq!(index.header_words, expected);
    }

    #[test]
    fn rejects_truncated_header() {
        let error = ArchiveIndex::parse(&[3, 0, 0], 0).unwrap_err();
        assert_eq!(error, ArchiveIndexError::TruncatedHeader { actual: 3 });
    }

    #[test]
    fn rejects_truncated_record_table() {
        let mut bytes = vec![0; HEADER_SIZE];
        bytes[0..4].copy_from_slice(&SUPPORTED_VERSION.to_le_bytes());
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        let error = ArchiveIndex::parse(&bytes, 0).unwrap_err();
        assert_eq!(
            error,
            ArchiveIndexError::InvalidTableLength {
                expected: HEADER_SIZE + RECORD_SIZE,
                actual: HEADER_SIZE
            }
        );
    }

    #[test]
    fn rejects_payload_range_past_end() {
        let bytes = fixture(b"foo\0", 6, 2);
        let error = ArchiveIndex::parse(&bytes, 7).unwrap_err();
        assert_eq!(
            error,
            ArchiveIndexError::PayloadOutOfBounds {
                record: 0,
                offset: 6,
                length: 2,
                payload_len: 7
            }
        );
    }

    #[test]
    fn rejects_non_utf8_path() {
        let bytes = fixture(&[0xff, 0], 0, 0);
        assert_eq!(
            ArchiveIndex::parse(&bytes, 0).unwrap_err(),
            ArchiveIndexError::InvalidPath { record: 0 }
        );
    }

    #[test]
    #[ignore = "requires OPENMT2001_ARCHIVE_DIR and OPENMT2001_ARCHIVE_BIN"]
    fn parses_original_index_when_path_is_supplied() {
        let path = std::env::var("OPENMT2001_ARCHIVE_DIR")
            .expect("OPENMT2001_ARCHIVE_DIR must point to original archive.DIR");
        let payload_path = std::env::var("OPENMT2001_ARCHIVE_BIN")
            .expect("OPENMT2001_ARCHIVE_BIN must point to original archive.bin");
        let bytes = std::fs::read(path).expect("read OPENMT2001_ARCHIVE_DIR");
        let payload_len = std::fs::metadata(payload_path)
            .expect("stat OPENMT2001_ARCHIVE_BIN")
            .len();
        let index = ArchiveIndex::parse(&bytes, payload_len).expect("parse original archive.DIR");
        assert_eq!(index.version, 3);
        assert_eq!(index.header_flags, 1);
        assert_eq!(index.entries.len(), 1_506);
        assert_eq!(index.entries.first().unwrap().offset, 0);
        assert_eq!(
            index.entries.last().unwrap().offset + index.entries.last().unwrap().length,
            payload_len
        );
    }
}
