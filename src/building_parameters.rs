//! Readonly projection of the configured v1.4 BuildingKind resource.
//! Original process-global words are not saved. Materialized building+28 is.
use crate::pe::PeImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingKinds([[u16; 3]; 3]);

impl BuildingKinds {
    pub fn from_image(image: &PeImage<'_>) -> Result<Self, String> {
        let mut rows = [[0; 3]; 3];
        for (kind, row) in rows.iter_mut().enumerate() {
            let bytes = image.at(0x5365f0 + kind as u32 * 46, 6)?;
            for (value, bytes) in row.iter_mut().zip(bytes.chunks_exact(2)) {
                *value = u16::from_le_bytes(bytes.try_into().unwrap());
            }
        }
        Ok(Self(rows))
    }

    /// Registered4379c0/437480/4375d0, limited to supported finite numeric
    /// tokens. This does not emulate native CRT diagnostics or live mutation.
    pub fn from_parameters(source: &str, image: &PeImage<'_>) -> Result<Self, String> {
        if !source.is_ascii() {
            return Err("building parameters must be original ASCII".into());
        }
        let mut resource = Self::from_image(image)?;
        let names: Vec<_> = (0..3)
            .map(|kind| {
                let bytes = image.at(0x5365d0 + kind * 46, 32)?;
                let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                Ok::<_, String>(&bytes[..end])
            })
            .collect::<Result<_, _>>()?;
        for line in source.lines() {
            let mut fields = line.split("//").next().unwrap().split_whitespace();
            let Some(command) = fields.next() else {
                continue;
            };
            let column = if command.eq_ignore_ascii_case("BuildingRent") {
                0
            } else if command.eq_ignore_ascii_case("BuildingCostPerUnit") {
                1
            } else if command.eq_ignore_ascii_case("BuildingCostToDemolish") {
                2
            } else {
                continue;
            };
            let Some(name) = fields.next() else {
                continue;
            };
            let kind = names
                .iter()
                .rposition(|row| row.eq_ignore_ascii_case(name.as_bytes()))
                .ok_or_else(|| format!("{command} unknown property {name}"))?;
            if let Some(text) = fields.next() {
                // atof returns extended precision, but the original first
                // spills binary32 to56851c, then50e928 truncates signed64.
                let value = text
                    .parse::<f64>()
                    .map_err(|_| format!("unsupported building parameter {text}"))?
                    as f32;
                resource.0[kind][column] = crate::x87::Sum::from_f32(value)?.ftol_low_word() as u16;
            }
        }
        Ok(resource)
    }

    /// Rent, construction and demolition words, in original address order.
    pub fn row(&self, kind: u32) -> Result<[u16; 3], String> {
        self.0
            .get(kind as usize)
            .copied()
            .ok_or_else(|| "building kind outside configured parameter rows".into())
    }
}
