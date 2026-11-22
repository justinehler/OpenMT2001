//! District material settings from the original539780/539784 tables and board.prm.
//! Parsing and application are pure; the host supplies the original source text.
use crate::{board::Board, pe::PeImage};

#[derive(Clone, Debug)]
pub struct Style {
    names: [String; 12],
    category_kinds: [u8; 52],
    pub pavement: [u32; 12],
    pub text: [u32; 12],
}

impl Style {
    pub fn from_v14(image: &PeImage<'_>) -> Result<Self, String> {
        let mut names: [String; 12] = std::array::from_fn(|_| String::new());
        let mut pavement = [0; 12];
        let mut text = [0; 12];
        for (district, name) in names.iter_mut().enumerate() {
            let at = 0x539780 + district as u32 * 60;
            let row = image.at(at, 60)?;
            pavement[district] = u32::from_le_bytes(row[..4].try_into().unwrap());
            text[district] = u32::from_le_bytes(row[4..8].try_into().unwrap());
            let end = row[8..]
                .iter()
                .position(|&c| c == 0)
                .ok_or("original district name is unterminated")?;
            *name = std::str::from_utf8(&row[8..8 + end])
                .map_err(|_| "original district name is not UTF-8")?
                .to_owned();
        }
        let mut category_kinds = [0; 52];
        for (category, kind) in category_kinds.iter_mut().enumerate() {
            *kind = image.at(0x5385a4 + category as u32 * 0x58, 1)?[0];
            if usize::from(*kind) >= 12 { return Err("original category district is outside the style table".into()); }
        }
        Ok(Self {
            names,
            category_kinds,
            pavement,
            text,
        })
    }

    /// Apply supported original commands in file order. Repeated entries win
    /// last, as436ba0/436e70 overwrite the palette and existing object colors.
    /// Validation errors are asset admission errors, not native UI diagnostics.
    pub fn apply_source(&mut self, source: &str) -> Result<(), String> {
        for line in source.lines() {
            let fields = line
                .split("//")
                .next()
                .unwrap()
                .split_whitespace()
                .collect::<Vec<_>>();
            let Some(command) = fields.first() else {
                continue;
            };
            let is_text = command.eq_ignore_ascii_case("DistrictTextColour");
            if !is_text && !command.eq_ignore_ascii_case("DistrictColour") {
                continue;
            }
            if fields.len() != 5 {
                return Err("original district color requires a name and three channels".into());
            }
            let district = self
                .names
                .iter()
                .position(|name| name.eq_ignore_ascii_case(fields[1]))
                .ok_or("original district color has an unknown district")?;
            let channel = |i: usize| {
                fields[i]
                    .parse::<i32>()
                    .map_err(|_| "original district color channel is not an integer".to_owned())
            };
            let [red, green, blue] = [channel(2)?, channel(3)?, channel(4)?];
            //436d59..436d6d (and437033): arithmetic packing, not channel
            //clamping or independent bitwise OR. Valid source channels are0..255.
            let color = red
                .wrapping_sub(256)
                .wrapping_shl(8)
                .wrapping_add(green)
                .wrapping_shl(8)
                .wrapping_add(blue) as u32;
            if is_text {
                self.text[district] = color;
            } else {
                self.pavement[district] = color;
            }
        }
        Ok(())
    }

    pub fn apply_board(&self, board: &mut Board) -> Result<(), String> {
        self.apply_blocks(&mut board.blocks)
    }

    ///436d8d propagates the palette to every native category, including the
    ///inactive geometry slots40/45. No ownership or membership field is changed.
    pub fn apply_categories(&self, categories: &mut [[u8; 380]; 52]) {
        for (category, &kind) in categories.iter_mut().zip(&self.category_kinds) {
            category[0x94..0x98].copy_from_slice(&self.pavement[usize::from(kind)].to_le_bytes());
        }
    }

    pub fn apply_blocks(&self, blocks: &mut [crate::board::Block]) -> Result<(), String> {
        for block in blocks {
            block.pavement_color = *self
                .pavement
                .get(block.kind as usize)
                .ok_or("original block district is outside the style table")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original PE and native24 board-style command observations"]
    fn original_board_style_commands() {
        let executable = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let mut style = Style::from_v14(&image).unwrap();
        let parameters = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let source = std::fs::read_to_string(
            parameters
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("Standard/fonts/board/usa/board.prm"),
        )
        .unwrap();
        let lines = source
            .lines()
            .filter(|line| {
                let command = line.split_whitespace().next().unwrap_or("");
                command.eq_ignore_ascii_case("DistrictColour")
                    || command.eq_ignore_ascii_case("DistrictTextColour")
            })
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 24);
        let bytes = std::fs::read("work/frontend/native-board-style.bin").unwrap();
        assert_eq!(&bytes[..8], b"MTBSTY01");
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0x037f);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 24);
        assert_eq!(bytes.len(), 16 + 24 * 32);
        for (case, record) in bytes[16..].chunks_exact(32).enumerate() {
            let word = |i: usize| u32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap());
            let district = word(1) as usize;
            style.apply_source(lines[case]).unwrap();
            let actual = if word(0) == 0 {
                style.pavement[district]
            } else {
                style.text[district]
            };
            assert_eq!(actual, word(5), "original command case{case}");
            let membership = u64::from_le_bytes(record[24..32].try_into().unwrap());
            let mut categories = [[0xa5; 380]; 52];
            style.apply_categories(&mut categories);
            for (id, category) in categories.iter().enumerate() {
                assert_eq!(&category[..0x94], &[0xa5; 0x94]);
                assert_eq!(&category[0x98..], &[0xa5; 380 - 0x98]);
                if word(0) == 0 && membership & (1 << id) != 0 {
                    assert_eq!(u32::from_le_bytes(category[0x94..0x98].try_into().unwrap()), word(5),
                        "original category propagation case{case}, slot{id}");
                }
            }
            for block in &board.blocks {
                assert_eq!(
                    membership & (1 << block.id) != 0,
                    block.kind as usize == district,
                    "original command case{case}, active block{}",
                    block.id
                );
            }
        }
        // The native supplied source repeats Ventnor TextColour; black wins.
        assert_eq!(style.text[6], 0xff000000);
        assert_eq!(style.pavement[6], 0xfff9f400);
    }
}
