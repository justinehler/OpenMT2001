//! Shared editing rule for single-line text fields (original 443aa0).
//!
//! The original stores field text as code-page bytes and draws them through
//! language row 0, which maps each byte from 32 upward to a glyph index. V2
//! keeps the text as a Rust string and converts each character to its
//! Windows-1252 byte only to admit, measure and draw it.
use super::text_width;

/// 443aa0 refuses an append once a field holds 128 bytes, whatever its
/// configured maximum.
pub const HARD_CAP: usize = 128;

/// Characters a filename-safe field refuses before any edit (443850).
pub const FILENAME_UNSAFE: &str = "\"*/:<>?\\|";

/// One field's configured limits. `None` is the original's -1 (unlimited).
#[derive(Clone, Copy, Debug)]
pub struct Rule {
    pub max_bytes: Option<usize>,
    /// Widest candidate text admitted, in pixels of the field's font.
    pub width: Option<i32>,
    pub filename_safe: bool,
}

impl Rule {
    /// Save As name (496250): maximum 215, so the 128-byte hard cap applies.
    pub const SAVE_NAME: Self = Self {
        max_bytes: Some(215),
        width: Some(155),
        filename_safe: true,
    };
    /// New profile name (495b50, 495ed0): maximum 215, width 90.
    pub const PROFILE_NAME: Self = Self {
        max_bytes: Some(215),
        width: Some(90),
        filename_safe: true,
    };

    /// Append `typed` to `text` one character at a time, as the original
    /// feeds one byte per character event. Returns whether anything changed.
    pub fn type_text(&self, text: &mut String, typed: &str, font: FieldFont<'_>) -> bool {
        let mut changed = false;
        for character in typed.chars() {
            changed |= self.type_char(text, character, font);
        }
        changed
    }

    /// 443aa0's admission test for one typed character.
    pub fn type_char(&self, text: &mut String, character: char, font: FieldFont<'_>) -> bool {
        if self.filename_safe && FILENAME_UNSAFE.contains(character) {
            return false;
        }
        let Some(byte) = windows_1252(character) else {
            return false;
        };
        let count = text.chars().count();
        if byte < 32 || count >= HARD_CAP || self.max_bytes.is_some_and(|maximum| count >= maximum)
        {
            return false;
        }
        if let Some(limit) = self.width {
            let mut candidate = font.glyphs(text);
            candidate.push(font.glyph(byte));
            // Width equality is admitted. A font that cannot be measured
            // admits nothing rather than letting text overrun its box.
            match text_width(font.tab, &candidate, 1.) {
                Ok(width) if width <= limit => {}
                _ => return false,
            }
        }
        text.push(character);
        true
    }
}

/// The font a field is measured and drawn with: its glyph table and the
/// language's row 0 (code-page byte minus 32 to glyph index).
#[derive(Clone, Copy)]
pub struct FieldFont<'a> {
    pub tab: &'a [u8],
    pub map: &'a [u8],
}

impl FieldFont<'_> {
    /// Glyph indices for drawing or measuring `text`. A character outside
    /// Windows-1252 (only possible in names from other sources) draws as `?`.
    pub fn glyphs(&self, text: &str) -> Vec<u8> {
        text.chars()
            .map(|c| self.glyph(windows_1252(c).filter(|&b| b >= 32).unwrap_or(b'?')))
            .collect()
    }

    fn glyph(&self, byte: u8) -> u8 {
        // Row 0 maps bytes without a glyph to glyph 0 (a space), as here.
        self.map
            .get(usize::from(byte.saturating_sub(32)))
            .copied()
            .unwrap_or(0)
    }
}

/// `text` as original code-page bytes, `?` for characters outside it.
pub fn code_page(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| windows_1252(c).unwrap_or(b'?'))
        .collect()
}

/// The Windows-1252 byte for `character`, if it has one. Control
/// characters have none: they are editing keys, never field text.
pub fn windows_1252(character: char) -> Option<u8> {
    const HIGH: [(char, u8); 27] = [
        ('\u{20ac}', 0x80),
        ('\u{201a}', 0x82),
        ('\u{0192}', 0x83),
        ('\u{201e}', 0x84),
        ('\u{2026}', 0x85),
        ('\u{2020}', 0x86),
        ('\u{2021}', 0x87),
        ('\u{02c6}', 0x88),
        ('\u{2030}', 0x89),
        ('\u{0160}', 0x8a),
        ('\u{2039}', 0x8b),
        ('\u{0152}', 0x8c),
        ('\u{017d}', 0x8e),
        ('\u{2018}', 0x91),
        ('\u{2019}', 0x92),
        ('\u{201c}', 0x93),
        ('\u{201d}', 0x94),
        ('\u{2022}', 0x95),
        ('\u{2013}', 0x96),
        ('\u{2014}', 0x97),
        ('\u{02dc}', 0x98),
        ('\u{2122}', 0x99),
        ('\u{0161}', 0x9a),
        ('\u{203a}', 0x9b),
        ('\u{0153}', 0x9c),
        ('\u{017e}', 0x9e),
        ('\u{0178}', 0x9f),
    ];
    match character {
        ' '..='~' | '\u{a0}'..='\u{ff}' => Some(character as u8),
        _ => HIGH
            .iter()
            .find(|(c, _)| *c == character)
            .map(|&(_, byte)| byte),
    }
}

/// A 176-glyph table and language row shaped like the original's: `W`
/// advances 12 px, `i` and `.` 3 px, everything else 7 px, and the row
/// maps code-page bytes onto those glyphs the way the US row 0 does.
#[cfg(test)]
pub fn synthetic_font() -> (Vec<u8>, Vec<u8>) {
    let mut tab = vec![0; 4 + 176 * 44];
    tab[..4].copy_from_slice(&176u32.to_le_bytes());
    for glyph in 0..176usize {
        let width: i32 = match glyph as u8 + 32 {
            b'W' => 13,
            b'i' | b'.' => 4,
            _ => 8,
        };
        let at = 4 + glyph * 44;
        tab[at + 32..at + 36].copy_from_slice(&width.to_le_bytes());
        tab[at + 36..at + 40].copy_from_slice(&11i32.to_le_bytes());
        tab[at + 40..at + 44].copy_from_slice(&3i32.to_le_bytes());
    }
    // ASCII maps onto itself; 0xc0..=0xff onto glyphs 112..=175, as the
    // US row does; the C1 range has no glyph.
    let map = (32..=255u8)
        .map(|byte| match byte {
            32..=126 => byte - 32,
            0xc0..=0xff => byte - 0xc0 + 112,
            _ => 0,
        })
        .collect();
    (tab, map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(rule: Rule, text: &str) -> String {
        let (tab, map) = synthetic_font();
        let mut field = String::new();
        rule.type_text(
            &mut field,
            text,
            FieldFont {
                tab: &tab,
                map: &map,
            },
        );
        field
    }

    #[test]
    fn save_name_stops_at_155_pixels() {
        // W advances 12 px: 12 fit in 144 px, a 13th would be 156.
        assert_eq!(typed(Rule::SAVE_NAME, &"W".repeat(40)), "W".repeat(12));
        // i advances 3 px: 51 fit in 153 px, a 52nd would be 156.
        assert_eq!(typed(Rule::SAVE_NAME, &"i".repeat(80)), "i".repeat(51));
    }

    #[test]
    fn profile_name_stops_at_90_pixels_and_admits_equality() {
        assert_eq!(typed(Rule::PROFILE_NAME, &"W".repeat(20)), "W".repeat(7));
        // 30 i are exactly 90 px wide.
        assert_eq!(typed(Rule::PROFILE_NAME, &"i".repeat(40)), "i".repeat(30));
        // A wide glyph that no longer fits does not stop a narrow one.
        assert_eq!(
            typed(Rule::PROFILE_NAME, &format!("{}Wi", "W".repeat(7))),
            format!("{}i", "W".repeat(7))
        );
    }

    #[test]
    fn hard_cap_is_128_bytes_whatever_the_maximum() {
        let unlimited_width = Rule {
            max_bytes: Some(215),
            width: None,
            filename_safe: true,
        };
        assert_eq!(typed(unlimited_width, &"a".repeat(300)), "a".repeat(128));
        let unconfigured = Rule {
            max_bytes: None,
            width: None,
            filename_safe: false,
        };
        assert_eq!(typed(unconfigured, &"é".repeat(300)).chars().count(), 128);
        let small = Rule {
            max_bytes: Some(31),
            ..unconfigured
        };
        assert_eq!(typed(small, &"a".repeat(40)), "a".repeat(31));
    }

    #[test]
    fn filename_safe_fields_reject_nine_characters() {
        assert_eq!(
            typed(Rule::SAVE_NAME, "a\"b*c/d:e<f>g?h\\i|j"),
            "abcdefghij"
        );
        assert_eq!(typed(Rule::PROFILE_NAME, "x?y"), "xy");
        let plain = Rule {
            filename_safe: false,
            ..Rule::SAVE_NAME
        };
        assert_eq!(typed(plain, "a?b"), "a?b");
        // Everything else from 32 up is admitted, including punctuation.
        assert_eq!(typed(Rule::SAVE_NAME, "a.b-c_d'e"), "a.b-c_d'e");
    }

    #[test]
    fn high_bytes_are_admitted_and_drawn_through_the_language_row() {
        let (tab, map) = synthetic_font();
        let font = FieldFont {
            tab: &tab,
            map: &map,
        };
        assert_eq!(typed(Rule::SAVE_NAME, "Café"), "Café");
        assert_eq!(font.glyphs("é"), [0xe9 - 0xc0 + 112]);
        assert_eq!(code_page("Café"), b"Caf\xe9");
        // € has a Windows-1252 byte but no glyph in this row: admitted,
        // drawn as glyph 0, as the original does.
        assert_eq!(typed(Rule::SAVE_NAME, "€"), "€");
        assert_eq!(font.glyphs("€"), [0]);
    }

    #[test]
    fn characters_without_a_code_page_byte_are_refused() {
        assert_eq!(typed(Rule::SAVE_NAME, "a\u{3b1}b\u{1f600}c"), "abc");
        assert_eq!(typed(Rule::SAVE_NAME, "a\tb\u{7f}c\u{85}d"), "abcd");
        assert_eq!(code_page("\u{3b1}"), b"?");
    }
}
