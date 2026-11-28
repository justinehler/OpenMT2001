//! Original offline host-setup objective controls (4a40a0/4a4180/4a4925).
//! The network-family level catalog is also used by offline Skirmish. This
//! module does not create a network session or guess the participant roster.
use crate::{level_catalog::Catalog, pe::PeImage};
pub mod launch;
pub mod roster;

pub const OPTION_BASE: u32 = 0x544340;
pub const OPTION_COUNT: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub kind: i32,
    pub label: u32,
    pub minimum: u32,
    pub maximum: u32,
    pub step: u32,
    pub initial: u32,
}

pub struct Options {
    rules: [Rule; OPTION_COUNT],
}

impl Options {
    /// Seven original 24-byte rows, retained in their table order. In
    /// particular, kind5's initial3000 is clamped later, not corrected here.
    pub fn from_pe(image: &PeImage<'_>) -> Result<Self, String> {
        let mut rules = [Rule {
            kind: 0,
            label: 0,
            minimum: 0,
            maximum: 0,
            step: 0,
            initial: 0,
        }; OPTION_COUNT];
        for (i, rule) in rules.iter_mut().enumerate() {
            let bytes = image.at(OPTION_BASE + i as u32 * 24, 24)?;
            let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            *rule = Rule {
                kind: word(0) as i32,
                label: word(4),
                minimum: word(8),
                maximum: word(12),
                step: word(16),
                initial: word(20),
            };
            if rule.minimum > rule.maximum {
                return Err("Skirmish objective bounds are reversed".into());
            }
        }
        Ok(Self { rules })
    }

    pub fn rules(&self) -> &[Rule; OPTION_COUNT] {
        &self.rules
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    Hidden,
    Integer(u32),
    Currency(u32),
}

/// Owner+388 stores the option row, owner+38c its unsigned value. The current
/// objective kind is separate: native selection does not clear a previous row
/// when a script supplies an unrecognized kind.
#[derive(Default, Debug)]
pub struct Modifier {
    kind: i32,
    row: Option<usize>,
    value: u32,
}

impl Modifier {
    pub fn select(&mut self, options: &Options, kind: i32) {
        self.kind = kind;
        if let Some(row) = options.rules.iter().position(|r| r.kind == kind) {
            self.row = Some(row);
        }
        if let Some(row) = self.row {
            self.value = options.rules[row].initial;
            self.clamp(options);
        }
    }

    fn clamp(&mut self, options: &Options) {
        if self.kind != 0
            && let Some(row) = self.row
        {
            let rule = options.rules[row];
            self.value = self.value.clamp(rule.minimum, rule.maximum);
        }
    }

    /// The original applies wrapping DWORD arithmetic, then unsigned bounds.
    /// It does not snap arbitrary values to a step lattice.
    pub fn adjust(&mut self, options: &Options, increase: bool) {
        let Some(row) = self.row else {
            return;
        };
        let step = options.rules[row].step;
        self.value = if increase {
            self.value.wrapping_add(step)
        } else {
            self.value.wrapping_sub(step)
        };
        self.clamp(options);
    }

    pub fn controls_visible(&self) -> bool {
        self.kind != 0
    }
    pub fn label(&self, options: &Options) -> Option<u32> {
        self.row.map(|row| options.rules[row].label)
    }
    pub fn value(&self) -> u32 {
        self.value
    }
    ///4a4815..4831 converts the unsigned value to the selected level's F32
    /// target at+870 before handing off to participant selection.
    pub fn target(&self) -> f32 {
        self.value as f32
    }
    pub fn display(&self, options: &Options) -> Display {
        if self.kind == 0 {
            return Display::Hidden;
        }
        match self.row.map(|row| options.rules[row].kind) {
            Some(3 | 5) => Display::Currency(self.value),
            Some(0) | None => Display::Hidden,
            Some(_) => Display::Integer(self.value),
        }
    }
}

///48da50/48d9f0 argument1 filters record+1==0. Unlock/completion and
/// campaign list positions are irrelevant; preserve actual registration order.
pub fn scenario_rows(catalog: &Catalog) -> Vec<usize> {
    catalog
        .levels
        .iter()
        .enumerate()
        .filter_map(|(i, level)| (!level.single_player()).then_some(i))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        let rows = [
            [3, 1884, 5000, 20000, 1000, 10000],
            [6, 1885, 500, 10000, 500, 2000],
            [4, 1886, 1940, 2000, 5, 1960],
            [5, 1887, 10000, 100000, 10000, 3000],
            [1, 1890, 1, 5, 1, 3],
            [2, 1891, 1, 5, 1, 3],
            [0, 2238, 0, 0, 0, 0],
        ];
        Options {
            rules: rows.map(|r| Rule {
                kind: r[0] as i32,
                label: r[1],
                minimum: r[2],
                maximum: r[3],
                step: r[4],
                initial: r[5],
            }),
        }
    }

    #[test]
    fn objective_controls_keep_native_bounds_defaults_and_format() {
        let options = options();
        let mut m = Modifier::default();
        for (kind, initial, display) in [
            (3, 10000, Display::Currency(10000)),
            (6, 2000, Display::Integer(2000)),
            (4, 1960, Display::Integer(1960)),
            (5, 10000, Display::Currency(10000)),
            (1, 3, Display::Integer(3)),
            (2, 3, Display::Integer(3)),
            (0, 0, Display::Hidden),
        ] {
            m.select(&options, kind);
            assert_eq!(m.value(), initial);
            assert_eq!(m.display(&options), display);
            assert_eq!(m.controls_visible(), kind != 0);
            let rule = options.rules.iter().find(|r| r.kind == kind).unwrap();
            for _ in 0..250 {
                m.adjust(&options, false);
            }
            assert_eq!(m.value(), rule.minimum);
            for _ in 0..250 {
                m.adjust(&options, true);
            }
            assert_eq!(m.value(), rule.maximum);
            assert_eq!(m.target().to_bits(), (rule.maximum as f32).to_bits());
        }
        assert_eq!(options.rules[3].initial, 3000);
    }

    #[test]
    fn unrecognized_kind_and_unsigned_adjustment_keep_native_order() {
        let options = options();
        let mut m = Modifier::default();
        m.select(&options, 99);
        m.adjust(&options, false);
        assert_eq!(m.value(), 0);
        assert_eq!(m.label(&options), None);
        m.select(&options, 3);
        m.adjust(&options, true);
        m.select(&options, 99);
        assert_eq!(m.value(), 10000);
        assert_eq!(m.label(&options), Some(1884));
        m.value = 0;
        m.adjust(&options, false);
        assert_eq!(
            m.value(),
            20000,
            "wrapped subtraction clamps at upper bound"
        );
    }

    #[test]
    fn skirmish_catalog_keeps_family_and_registration_order() {
        use crate::level_catalog::{Labels, Registration};
        let mut catalog = Catalog::default();
        for id in [200001, 300005, 100000, 300001, 400002, 500001, 900001] {
            catalog
                .register(
                    Registration {
                        id,
                        directory: format!("level{id}").into_bytes(),
                        labels: Labels::Literal {
                            title: vec![],
                            description: vec![],
                        },
                        checksum: 0,
                    },
                    true,
                    true,
                )
                .unwrap();
        }
        let before = catalog.levels.clone();
        let rows = scenario_rows(&catalog);
        assert_eq!(
            rows.iter()
                .map(|&i| catalog.levels[i].id())
                .collect::<Vec<_>>(),
            [300005, 300001, 900001]
        );
        assert!(catalog.select(300001, true));
        assert!(!catalog.select(300001, false));
        assert_eq!(catalog.levels, before);
    }

    #[test]
    #[ignore = "requires player-supplied v1.4 PE as inert table data"]
    fn original_skirmish_option_rows_match_recovered_contract() {
        let bytes = std::fs::read(crate::fixtures::path("EXE")).unwrap();
        let image = PeImage::parse(&bytes).unwrap();
        let original = Options::from_pe(&image).unwrap();
        assert_eq!(original.rules, options().rules);
    }
}
