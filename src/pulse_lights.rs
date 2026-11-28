//! Original 004b5d50's four independent live-city pulsing light rows.
use crate::pe::PeImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    counter: u32,
    period: u32,
    level: i32,
    rising: bool,
    color: u32,
    size: u32,
}

impl Row {
    pub fn color(self) -> u32 {
        self.color
    }
    pub fn size(self) -> u32 {
        self.size
    }
    #[cfg(test)]
    fn words(self) -> [u32; 6] {
        [
            self.counter,
            self.period,
            self.level as u32,
            u32::from(self.rising),
            self.color,
            self.size,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PulseLights {
    rows: [Row; 4],
}

impl PulseLights {
    /// Host-neutral retained rows for the explicitly versioned V2 save tail.
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0; 96];
        for (i, row) in self.rows.iter().enumerate() {
            for (j, word) in [
                row.counter,
                row.period,
                row.level as u32,
                u32::from(row.rising),
                row.color,
                row.size,
            ]
            .into_iter()
            .enumerate()
            {
                bytes[i * 24 + j * 4..i * 24 + j * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
        }
        bytes
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 96 {
            return Err("V2 pulse-light row size invalid".into());
        }
        let rows = std::array::from_fn(|i| {
            let word = |j: usize| {
                u32::from_le_bytes(
                    bytes[i * 24 + j * 4..i * 24 + j * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            };
            Row {
                counter: word(0),
                period: word(1),
                level: word(2) as i32,
                rising: word(3) != 0,
                color: word(4),
                size: word(5),
            }
        });
        if rows
            .iter()
            .any(|r| r.period == 0 || !(0..=255).contains(&r.level))
        {
            return Err("V2 pulse-light state invalid".into());
        }
        Ok(Self { rows })
    }
    /// The four 24-byte original rows at 00545588, including period and RGB.
    pub fn from_original(image: &PeImage<'_>) -> Result<Self, String> {
        let bytes = image.at(0x545588, 96)?;
        let rows = std::array::from_fn(|i| {
            let word = |n: usize| {
                u32::from_le_bytes(
                    bytes[i * 24 + n * 4..i * 24 + n * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            };
            Row {
                counter: word(0),
                period: word(1),
                level: word(2) as i32,
                rising: bytes[i * 24 + 12] != 0,
                color: word(4),
                size: word(5),
            }
        });
        if rows
            .iter()
            .any(|row| row.period == 0 || row.level != 0 || row.color >> 24 != 0)
        {
            return Err("unsupported original pulse-light initializer".into());
        }
        Ok(Self { rows })
    }

    /// One ordered scheduler Helper(004b5d50) call, only in simulation mode 1.
    pub fn tick(&mut self) {
        for row in &mut self.rows {
            row.counter = row.counter.wrapping_add(1);
            if row.counter != row.period {
                continue;
            }
            row.counter = 0;
            row.level = if row.rising {
                row.level.wrapping_add(20)
            } else {
                row.level.wrapping_sub(20)
            };
            if row.level >= 255 {
                row.level = 255;
                row.rising = false;
            }
            if row.level <= 0 {
                row.level = 0;
                row.rising = true;
            }
            row.color = (row.color & 0x00ff_ffff) | ((row.level as u32 & 255) << 24);
        }
    }

    pub fn rows(&self) -> &[Row; 4] {
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires pinned original v1.4 executable and 101-tick native fixture"]
    fn native_four_row_pulse_lifecycle() {
        let exe = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let mut state = PulseLights::from_original(&image).unwrap();
        let fixture = std::fs::read(crate::fixtures::path("NATIVE_CITY_PULSE")).unwrap();
        assert_eq!(&fixture[..8], b"MTPULS01");
        assert_eq!(u32::from_le_bytes(fixture[8..12].try_into().unwrap()), 101);
        assert_eq!(fixture.len(), 12 + 101 * 96);
        for tick in 0..101 {
            if tick != 0 {
                state.tick();
            }
            for (row, original) in state
                .rows
                .iter()
                .zip(fixture[12 + tick * 96..12 + (tick + 1) * 96].chunks_exact(24))
            {
                let expected = std::array::from_fn::<_, 6, _>(|i| {
                    u32::from_le_bytes(original[i * 4..i * 4 + 4].try_into().unwrap())
                });
                assert_eq!(row.words(), expected, "tick {tick}");
            }
        }
    }
}
