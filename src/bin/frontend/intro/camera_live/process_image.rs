//! Owned *process allocation* cells, never a fresh-game/warm-load reset.
//! Data comes from the supplied PE; no original payload is embedded here.
use std::collections::BTreeMap;

struct Allocation {
    bytes: Vec<u8>,
    initialized: Vec<bool>,
}
pub struct Cells {
    allocations: BTreeMap<u32, Allocation>,
}
fn le16(b: &[u8], i: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        b.get(i..i + 2)
            .ok_or("truncated supplied PE")?
            .try_into()
            .unwrap(),
    ))
}
fn le32(b: &[u8], i: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(i..i + 4)
            .ok_or("truncated supplied PE")?
            .try_into()
            .unwrap(),
    ))
}
impl Cells {
    /// Allocate sections at their preferred addresses. This models only
    /// Windows' file copy / virtual-tail zero-fill, not imports/relocations,
    /// CRT constructors, dynamic allocation or execution of any PE code.
    pub fn process_allocation(pe: &[u8]) -> Result<Self, String> {
        if pe.get(..2) != Some(b"MZ") {
            return Err("missing DOS header".into());
        }
        let p = le32(pe, 0x3c)? as usize;
        if pe.get(p..p + 4) != Some(b"PE\0\0") || le16(pe, p + 4)? != 0x14c {
            return Err("UI process cells require supplied i386 PE".into());
        }
        let optional = p + 24;
        if le16(pe, optional)? != 0x10b || le16(pe, p + 20)? < 64 {
            return Err("UI process cells require PE32 optional header".into());
        }
        let base = le32(pe, optional + 28)?;
        if base != 0x400000 {
            return Err("UI preferred-address mapping requires400000 base".into());
        }
        let mut cells = Self {
            allocations: BTreeMap::new(),
        };
        let table = optional + usize::from(le16(pe, p + 20)?);
        for i in 0..usize::from(le16(pe, p + 6)?) {
            let s = table + i * 40;
            let virtual_size = le32(pe, s + 8)? as usize;
            let raw_size = le32(pe, s + 16)? as usize;
            let raw = le32(pe, s + 20)? as usize;
            let size = virtual_size.max(raw_size);
            if size == 0 {
                continue;
            }
            // Bounded parsing, not a game resource limit. Actual image<16MiB.
            if size > 64 * 1024 * 1024 {
                return Err("supplied PE section too large".into());
            }
            let start = base
                .checked_add(le32(pe, s + 12)?)
                .ok_or("PE address overflow")?;
            let mut bytes = vec![0; size];
            bytes[..raw_size].copy_from_slice(
                pe.get(raw..raw.checked_add(raw_size).ok_or("PE raw range overflow")?)
                    .ok_or("supplied PE raw section leaves file")?,
            );
            cells.insert(start, bytes, true)?;
        }
        Ok(cells)
    }
    fn insert(&mut self, start: u32, bytes: Vec<u8>, known: bool) -> Result<(), String> {
        let end = u64::from(start) + bytes.len() as u64;
        if end > u64::from(u32::MAX) + 1 {
            return Err("UI allocation wraps address space".into());
        }
        if self.allocations.iter().any(|(&s, a)| {
            u64::from(s) < end && u64::from(start) < u64::from(s) + a.bytes.len() as u64
        }) {
            return Err("overlapping UI allocation".into());
        }
        self.allocations.insert(
            start,
            Allocation {
                initialized: vec![known; bytes.len()],
                bytes,
            },
        );
        Ok(())
    }
    /// Native heap storage is NOT presumed zero. Every consumed byte must be
    /// supplied by its actual constructor/layout writer before a read.
    pub fn heap_allocation(&mut self, start: u32, length: usize) -> Result<(), String> {
        self.insert(start, vec![0; length], false)
    }
    fn location(&self, address: u32, length: usize) -> Result<(u32, usize), String> {
        let (&start, a) = self
            .allocations
            .range(..=address)
            .next_back()
            .ok_or_else(|| format!("UI cell{address:08x} has no allocation"))?;
        let offset = (address - start) as usize;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > a.bytes.len())
        {
            return Err(format!(
                "UI cell range{address:08x}/{length} leaves actual allocation"
            ));
        }
        Ok((start, offset))
    }
    pub fn read(&self, address: u32, length: usize) -> Result<&[u8], String> {
        let (s, o) = self.location(address, length)?;
        let a = &self.allocations[&s];
        if a.initialized[o..o + length].iter().any(|v| !*v) {
            return Err(format!(
                "UI cell{address:08x}/{length} lacks actual constructor/layout producer"
            ));
        }
        Ok(&a.bytes[o..o + length])
    }
    pub fn write(&mut self, address: u32, bytes: &[u8]) -> Result<(), String> {
        let (s, o) = self.location(address, bytes.len())?;
        let a = self.allocations.get_mut(&s).unwrap();
        a.bytes[o..o + bytes.len()].copy_from_slice(bytes);
        a.initialized[o..o + bytes.len()].fill(true);
        Ok(())
    }
    pub fn byte(&self, a: u32) -> Result<u8, String> {
        Ok(self.read(a, 1)?[0])
    }
    pub fn half(&self, a: u32) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.read(a, 2)?.try_into().unwrap()))
    }
    pub fn word(&self, a: u32) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.read(a, 4)?.try_into().unwrap()))
    }
    pub fn float(&self, a: u32) -> Result<f32, String> {
        Ok(f32::from_bits(self.word(a)?))
    }
    pub fn set_byte(&mut self, a: u32, v: u8) -> Result<(), String> {
        self.write(a, &[v])
    }
    pub fn set_word(&mut self, a: u32, v: u32) -> Result<(), String> {
        self.write(a, &v.to_le_bytes())
    }
    pub fn rect(&self, a: u32) -> Result<[f32; 4], String> {
        Ok([
            self.float(a)?,
            self.float(a + 4)?,
            self.float(a + 8)?,
            self.float(a + 12)?,
        ])
    }
    pub fn extent(&self, a: u32) -> Result<[f32; 5], String> {
        let r = self.rect(a)?;
        Ok([r[0], r[1], r[2], r[3], self.float(a + 16)?])
    }
    pub fn ntbs(&self, a: u32, max: usize) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        for i in 0..max {
            let b = self.byte(a.checked_add(i as u32).ok_or("NTBS address overflow")?)?;
            if b == 0 {
                return Ok(out);
            }
            out.push(b);
        }
        Err("supplied UI NTBS exceeds bounded actual consumer".into())
    }
}
