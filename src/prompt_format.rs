//! Packed language formatting used by the original prompt widgets (0x4e7fe0).
//! These are game directives, not C printf: widths/precision and `%%` have no
//! special treatment, and `%i` copies a language row rather than an integer.

/// A native formatting word, or a host byte string replacing a native pointer.
#[derive(Clone, Copy, Debug)]
pub enum Argument<'a> {
    Word(u32),
    Text(&'a [u8]),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Output {
    pub glyphs: Vec<u8>,
    /// The native gatherer reports error 6 for a missing argument, retaining
    /// the prefix already emitted. Its row bridge still displays that prefix.
    /// Invalid native pointers/indices are also bounded here to that prefix.
    pub complete: bool,
}

/// Map a native byte string (0x4e7690); inserted strings are never re-formatted.
pub fn map_text(text: &[u8], row: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for &byte in text {
        if byte == b'\n' {
            out.extend([*row.get(0x3c)?, *row.get(0x4e)?]);
        } else if byte < 32 {
            break;
        } else {
            out.push(*row.get(usize::from(byte - 32))?);
        }
    }
    Some(out)
}

/// Expand one packed row using the font manager's startup configuration:
/// currency symbol/layout 2243/2244, board labels rooted at 2245.
/// Unknown directives retain their percent and resume at the following byte.
pub fn resource(text: &[u8], strings: &[Vec<u8>], arguments: &[Argument<'_>]) -> Output {
    let mut output = Output {
        glyphs: Vec::with_capacity(text.len()),
        complete: false,
    };
    let Some(row) = strings.first().filter(|r| r.len() > 0x53) else {
        return output;
    };
    let percent = row[5];
    let token_matches = |glyph: u8, index: usize| i32::from(glyph as i8) == i32::from(row[index]);
    // Native suffix scanning indexes row0 by glyph + 32, rather than searching
    // the alphabet for a reverse mapping (4e81dd, 4e829d).
    let digit = |glyph: u8| {
        row.get(usize::from(glyph) + 32)
            .copied()
            .filter(u8::is_ascii_digit)
    };
    let mut args = arguments.iter();
    let mut at = 0;
    while at < text.len() {
        if i32::from(text[at] as i8) != i32::from(percent) {
            output.glyphs.push(text[at]);
            at += 1;
            continue;
        }
        let Some(&token) = text.get(at + 1) else {
            // Do not read past a packed row for an incomplete directive.
            output.glyphs.push(percent);
            break;
        };
        if token_matches(token, 0x44) {
            let Some(Argument::Word(word)) = args.next() else {
                return output;
            };
            let Some(mapped) = map_text((*word as i32).to_string().as_bytes(), row) else {
                return output;
            };
            output.glyphs.extend(mapped);
            at += 2;
        } else if token_matches(token, 0x53) {
            let Some(Argument::Text(text)) = args.next() else {
                return output;
            };
            let Some(mapped) = map_text(text, row) else {
                return output;
            };
            output.glyphs.extend(mapped);
            at += 2;
        } else if token_matches(token, 0x49) {
            let Some(Argument::Word(index)) = args.next() else {
                return output;
            };
            let Some(text) = strings.get(*index as usize) else {
                return output;
            };
            output.glyphs.extend_from_slice(text);
            at += 2;
        } else if token_matches(token, 0x43) {
            let Some(symbol) = strings.get(2243) else {
                return output;
            };
            let Some(layout) = strings.get(2244) else {
                return output;
            };
            at += 2;
            if layout.first() != Some(&b'L') {
                while let Some(&glyph) = text.get(at).filter(|g| digit(**g).is_some()) {
                    output.glyphs.push(glyph);
                    at += 1;
                }
            }
            output.glyphs.extend_from_slice(symbol);
        } else if token_matches(token, 0x42) {
            at += 2;
            let mut number = 0u32;
            while let Some(value) = text.get(at).and_then(|g| digit(*g)) {
                let Some(next) = number
                    .checked_mul(10)
                    .and_then(|n| n.checked_add(u32::from(value - b'0')))
                else {
                    return output;
                };
                number = next;
                at += 1;
            }
            let Some(index) = number.checked_mul(4).and_then(|n| n.checked_add(2247)) else {
                return output;
            };
            let Some(label) = strings.get(index as usize) else {
                return output;
            };
            output.glyphs.extend_from_slice(label);
        } else {
            output.glyphs.push(percent);
            at += 1;
        }
    }
    output.complete = true;
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encoded(text: &[u8]) -> Vec<u8> {
        text.iter().map(|b| b - 32).collect()
    }
    fn strings() -> Vec<Vec<u8>> {
        let mut rows = vec![vec![]; 2449];
        rows[0] = (0..224).map(|n| n as u8).collect();
        rows[1] = encoded(b"Label %d");
        rows[2243] = encoded(b"$");
        rows[2244] = vec![b'L'];
        rows[2247] = encoded(b"First");
        rows[2247 + 19 * 4] = encoded(b"Block");
        rows
    }
    fn check(raw: &[u8], args: &[Argument<'_>], expected: &[u8]) {
        assert_eq!(
            resource(&encoded(raw), &strings(), args),
            Output {
                glyphs: encoded(expected),
                complete: true
            }
        );
    }
    #[test]
    fn decimal_string_and_language_arguments_are_distinct_and_sequential() {
        check(
            b"%d %d %d",
            &[
                Argument::Word(0),
                Argument::Word(u32::MAX),
                Argument::Word(i32::MIN as u32),
            ],
            b"0 -1 -2147483648",
        );
        check(
            b"%s: %i",
            &[Argument::Text(b"Name %d\0ignored"), Argument::Word(1)],
            b"Name %d: Label %d",
        );
        check(
            b"%i %d",
            &[Argument::Word(1), Argument::Word(7)],
            b"Label %d 7",
        );
    }
    #[test]
    fn currency_and_numbered_board_labels_do_not_consume_arguments() {
        check(
            b"%b19. %c50, %c100 %d",
            &[Argument::Word(7)],
            b"Block. $50, $100 7",
        );
        check(b"%b %b0 %b019!", &[], b"First First Block!");
        let mut rows = strings();
        rows[2244] = vec![b'R'];
        for (raw, expected) in [
            (b"%c123!".as_slice(), b"123$!".as_slice()),
            (b"%c0", b"0$"),
            (b"%c", b"$"),
        ] {
            assert_eq!(
                resource(&encoded(raw), &rows, &[]).glyphs,
                encoded(expected)
            );
        }
    }
    #[test]
    fn unknown_flags_precision_and_percent_remain_literal() {
        check(b"%u %f %07d %.2f %q %", &[], b"%u %f %07d %.2f %q %");
        check(b"%%", &[], b"%%");
        check(b"%%d", &[Argument::Word(7)], b"%7");
    }
    #[test]
    fn missing_arguments_and_invalid_references_keep_the_emitted_prefix() {
        for (text, args) in [
            (b"A %s after".as_slice(), vec![]),
            (b"A %d after", vec![]),
            (b"A %i after", vec![Argument::Word(u32::MAX)]),
            (b"A %b99999 after", vec![]),
        ] {
            assert_eq!(
                resource(&encoded(text), &strings(), &args),
                Output {
                    glyphs: encoded(b"A "),
                    complete: false
                }
            );
        }
    }
    #[test]
    fn string_mapping_keeps_percent_and_uses_original_newline_and_control_rules() {
        let rows = strings();
        assert_eq!(
            map_text(b"A%%\n\xff\0ignored", &rows[0]).unwrap(),
            vec![33, 5, 5, 0x3c, 0x4e, 223]
        );
        assert_eq!(map_text(b"A\tB", &rows[0]).unwrap(), vec![33]);
    }
    #[test]
    #[ignore = "requires original LANGUAGE, VOTING_OLD_LANGUAGE and INSTALLER fixtures"]
    fn every_shipped_language_row_formats_with_typed_arguments() {
        fn load(path: &std::path::Path) -> Vec<Vec<u8>> {
            let lengths = std::fs::read(path.join("tycoon.off")).unwrap();
            let bytes = std::fs::read(path.join("tycoon.bin")).unwrap();
            let mut at = 0;
            lengths
                .chunks_exact(2)
                .map(|n| {
                    let end = at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
                    let text = bytes[at..end].to_vec();
                    at = end;
                    text
                })
                .collect()
        }
        let board = load(&crate::fixtures::path("INSTALLER").join("Standard/fonts/board/usa"));
        for name in ["LANGUAGE", "VOTING_OLD_LANGUAGE"] {
            let mut rows = load(&crate::fixtures::path(name));
            rows.truncate(2242);
            rows.extend_from_slice(&board[1..]);
            // This checks the entire language bank, a superset of all titles,
            // bodies and indirect references selected by shipped Lua scripts.
            for (index, text) in rows.iter().enumerate().skip(1) {
                let mut args = Vec::new();
                let mut at = 0;
                while at + 1 < text.len() {
                    if text[at] == rows[0][5] {
                        match text[at + 1] {
                            t if t == rows[0][0x53] => args.push(Argument::Text(b"Synthetic")),
                            t if t == rows[0][0x44] => args.push(Argument::Word(7)),
                            t if t == rows[0][0x49] => args.push(Argument::Word(1)),
                            _ => {
                                at += 1;
                                continue;
                            }
                        }
                        at += 1;
                    }
                    at += 1;
                }
                assert!(resource(text, &rows, &args).complete, "{name} row {index}");
            }
        }
    }
}
