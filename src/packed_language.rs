//! Packed language tables (an `.off` file of u16 row lengths and a `.bin` file
//! of row bytes), as the original's text resources (native 4c1380) hold them.
//! The `test_*` functions are fixtures for the binary's tests.
use std::{fs, ops::Range, path::Path};

/// All row references are IDs re-resolved through this one current allocation.
/// Neither a copied alphabet nor cached row slices survive a board replacement.
#[derive(Clone, Debug)]
pub struct PackedLanguage {
    bytes: Vec<u8>,
    ranges: Vec<Range<usize>>,
}
impl PackedLanguage {
    /// Factored existing menu/tooltip/result parser: OFF contains u16 lengths.
    pub fn parse(off: &[u8], bytes: Vec<u8>) -> Result<Self, String> {
        if !off.len().is_multiple_of(2) {
            return Err("packed language: incomplete OFF length".into());
        }
        let mut at = 0usize;
        let mut ranges = Vec::new();
        for n in off.chunks_exact(2) {
            let end = at
                .checked_add(usize::from(u16::from_le_bytes(n.try_into().unwrap())))
                .ok_or("packed language: length overflow")?;
            bytes
                .get(at..end)
                .ok_or("packed language: row outside BIN")?;
            ranges.push(at..end);
            at = end;
        }
        if at != bytes.len() {
            return Err("packed language: OFF lengths do not cover BIN".into());
        }
        if ranges.is_empty() {
            return Err("packed language: missing character-map row0".into());
        }
        Ok(Self { bytes, ranges })
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let read = |extension| {
            let p = path.with_extension(extension);
            fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))
        };
        Self::parse(&read("off")?, read("bin")?).map_err(|e| format!("{}: {e}", path.display()))
    }
    pub fn row(&self, id: u32) -> Result<&[u8], String> {
        let range = self
            .ranges
            .get(id as usize)
            .ok_or_else(|| format!("actual language identifier{id} outside table"))?;
        Ok(&self.bytes[range.clone()])
    }
    pub fn row_range(&self, id: u32) -> Result<Range<usize>, String> {
        self.ranges
            .get(id as usize)
            .cloned()
            .ok_or_else(|| format!("language range{id} absent"))
    }
    pub fn rows(&self) -> Vec<&[u8]> {
        self.ranges.iter().map(|r| &self.bytes[r.clone()]).collect()
    }
    pub fn row_count(&self) -> usize {
        self.ranges.len()
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    ///4922e0/4e6ef0 skips board row0 and replaces the suffix at2242.
    /// Narrow modern domain: complete suffix replacement, no unsafe partial OFF.
    pub fn overlay(&mut self, start: u32, board: &Self) -> Result<(), String> {
        let start = start as usize;
        let added = board
            .ranges
            .len()
            .checked_sub(1)
            .ok_or("board row0 absent")?;
        if start.checked_add(added) != Some(self.ranges.len()) {
            return Err("board language: replacement does not cover existing suffix".into());
        }
        let cut = self
            .ranges
            .get(start)
            .ok_or("board language: destination range")?
            .start;
        let mut bytes = self.bytes[..cut].to_vec();
        let mut ranges = self.ranges[..start].to_vec();
        for row in board.rows().into_iter().skip(1) {
            let at = bytes.len();
            bytes.extend_from_slice(row);
            ranges.push(at..bytes.len());
        }
        self.bytes = bytes;
        self.ranges = ranges;
        Ok(())
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_rows(rows: Vec<Vec<u8>>) -> Self {
        let off = rows
            .iter()
            .flat_map(|r| u16::try_from(r.len()).unwrap().to_le_bytes())
            .collect::<Vec<_>>();
        Self::parse(&off, rows.concat()).unwrap()
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_replace(&mut self, id: u32, row: Vec<u8>) {
        let mut rows = self
            .rows()
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        rows[id as usize] = row;
        *self = Self::test_rows(rows);
    }
    #[cfg(any(test, feature = "test-support"))]
    pub fn test_byte(&mut self, at: usize, value: u8) {
        self.bytes[at] = value;
    }
}
