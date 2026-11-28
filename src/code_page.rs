//! Windows-1252, the code page the original stores names and field text in.
//! V2 keeps text as Rust strings and converts each character to its byte
//! only where the original's byte count, comparison or glyph lookup applies.

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
