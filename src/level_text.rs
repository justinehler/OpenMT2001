//! Level picker text: the original's glyph encoding for literal text, the
//! localized title and description of each level, and word wrapping against
//! an original font table.

pub fn ascii(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            if c.is_ascii() && !c.is_control() {
                c as u8 - 32
            } else {
                b'?' - 32
            }
        })
        .collect()
}
/// Owned glyph text only: no script runtime, filesystem, or borrowed catalog.
/// First duplicate ID wins, matching Catalog::find.
#[derive(Default)]
pub struct LevelTextCatalog {
    entries: std::collections::BTreeMap<u32, (Option<Vec<u8>>, Option<Vec<u8>>)>,
}
impl LevelTextCatalog {
    pub fn new(catalog: &crate::level_catalog::Catalog, strings: &[Vec<u8>]) -> Self {
        use crate::level_catalog::Labels;
        let mut result = Self::default();
        for level in &catalog.levels {
            result
                .entries
                .entry(level.id())
                .or_insert_with(|| match level.labels() {
                    Labels::Localized { title, description } => (
                        strings.get(title as usize).cloned(),
                        strings.get(description as usize).cloned(),
                    ),
                    Labels::Literal { title, description } => {
                        // Match the custom picker's byte-to-original-font conversion.
                        let glyphs = |text: Vec<u8>| {
                            text.into_iter()
                                .map(|b| if (32..=126).contains(&b) { b - 32 } else { 31 })
                                .collect()
                        };
                        (Some(glyphs(title)), Some(glyphs(description)))
                    }
                });
        }
        result
    }
    pub fn title(&self, id: u32, strings: &[Vec<u8>]) -> Vec<u8> {
        self.entries
            .get(&id)
            .and_then(|text| text.0.as_ref())
            .or_else(|| level_labels(id).and_then(|(title, _)| strings.get(title)))
            .cloned()
            .unwrap_or_else(|| ascii(&format!("Level {id}")))
    }
    pub fn description<'a>(&'a self, id: u32, strings: &'a [Vec<u8>]) -> Option<&'a [u8]> {
        self.entries
            .get(&id)
            .and_then(|text| text.1.as_ref())
            .or_else(|| level_labels(id).and_then(|(_, description)| strings.get(description)))
            .filter(|text| !text.is_empty())
            .map(Vec::as_slice)
    }
}
// GAMESTART/hub.lua level IDs + original4dc80d numeric language registrations.
pub fn level_labels(id: u32) -> Option<(usize, usize)> {
    match id {
        200001..=200016 => {
            let n = 1956 + (id - 200001) as usize * 2;
            Some((n, n + 1))
        }
        400001 => Some((1988, 1989)),
        400002 => Some((2229, 2230)),
        300000..=300005 => {
            let n = 1990 + (id - 300000) as usize * 2;
            Some((n, n + 1))
        }
        300006 => Some((2231, 2232)),
        100000..=100012 => Some((
            [
                2016, 2031, 2045, 2063, 2084, 2104, 2129, 2133, 2150, 2164, 2188, 2200, 2214,
            ][(id - 100000) as usize],
            2002 + (id - 100000) as usize,
        )),
        _ => None,
    }
}
pub fn wrap_lines(tab: &[u8], text: &[u8], max_width: f32) -> Result<Vec<Vec<u8>>, String> {
    use crate::font::{Font, Layout};
    let font = Font::parse(tab)?;
    let mut line = vec![];
    let mut lines = vec![];
    for word in text.split(|c| *c == 0) {
        let mut candidate = line.clone();
        if !candidate.is_empty() {
            candidate.push(0);
        }
        candidate.extend_from_slice(word);
        let q = font.layout(
            &candidate,
            Layout {
                origin: [0., 0.],
                depth: 0.5,
                scale: 1.,
                gap: 0,
                baseline: 0,
            },
        )?;
        let width = q.iter().map(|q| q.positions[2][0] + 0.5).fold(0., f32::max);
        if width > max_width && !line.is_empty() {
            lines.push(line);
            line = word.to_vec();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    Ok(lines)
}
