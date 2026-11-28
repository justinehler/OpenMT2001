//! Authored custom picker; headers deliberately carry no level identity.
use super::*;
#[derive(Debug, PartialEq)]
pub(super) enum Row {
    Header(Vec<u8>),
    Space,
    Level(usize),
}
pub(super) struct Custom {
    pub(super) rows: Vec<Row>,
    pub(super) selected: Option<usize>,
    pub(super) first: usize,
    root: PathBuf,
    pub(super) roster: Option<openmt2001::player_script::PendingRoster>,
}
fn rows(catalog: &openmt2001::level_catalog::Catalog) -> Vec<Row> {
    let mut rows = vec![];
    let mut previous: Option<Vec<u8>> = None;
    for (i, level) in catalog.levels.iter().enumerate() {
        if level.tutorial() || level.bytes[2] != 1 || level.id() / 100000 != 5 {
            continue;
        }
        let group = level
            .directory()
            .split(|&b| b == b'\\' || b == b'/')
            .nth(1)
            .unwrap_or_default()
            .to_vec();
        if previous.as_ref() != Some(&group) {
            if previous.is_some() {
                rows.push(Row::Space);
            }
            rows.push(Row::Header(group.clone()));
            previous = Some(group);
        }
        rows.push(Row::Level(i));
    }
    rows
}
// Native cups use a fixed12-unit stride; headers/spacers consume a row too.
fn visible_rows(
    rows: &[Row],
    first: usize,
    bounds: [f32; 4],
) -> impl Iterator<Item = (usize, &Row)> {
    rows.iter().skip(first).enumerate().take(
        openmt2001::frontend_list::Layout::new(bounds, SCENARIO_ROW_HEIGHT, 3.)
            .map_or(0, |layout| layout.visible_rows()),
    )
}
// Native 49a297..49a3d2: viewport row, three independent completion bytes,
// bank 296 empty / 297..299 earned. Callback is the production submission seam.
fn submit_row_cups(
    rows: &[Row],
    first: usize,
    bounds: [f32; 4],
    catalog: &openmt2001::level_catalog::Catalog,
    mut submit: impl FnMut(usize, [f32; 4]) -> Result<(), String>,
) -> Result<(), String> {
    for (visible_row, item) in visible_rows(rows, first, bounds) {
        let Row::Level(index) = item else {
            continue;
        };
        let Some(level) = catalog.levels.get(*index) else {
            continue;
        };
        for (cup, earned) in level.completed().into_iter().enumerate() {
            submit(
                if earned {
                    297 + cup
                } else {
                    SCENARIO_CUP_EMPTY
                },
                scenario_cup_rect(visible_row, cup),
            )?;
        }
    }
    Ok(())
}
// Resolve Windows spelling without allowing traversal, ambiguous names or symlink escapes.
fn players_path(root: &Path, directory: &[u8]) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("Custom scripts root: {e}"))?;
    let mut path = root.clone();
    let directory = std::str::from_utf8(directory).map_err(|_| "Custom directory is not UTF-8")?;
    for part in directory
        .split(['\\', '/'])
        .chain(std::iter::once("players.lua"))
    {
        if part.is_empty() || part == "." || part == ".." || part.contains(':') {
            return Err("Unsafe custom script path".into());
        }
        let matches = fs::read_dir(&path)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(part))
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(format!(
                "Missing or ambiguous custom script component {part}"
            ));
        }
        path = matches[0]
            .path()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !path.starts_with(&root) {
            return Err("Custom script escapes supplied root".into());
        }
    }
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
        return Err("Custom players script exceeds limit".into());
    }
    Ok(path)
}
fn first_uncompleted(c: &openmt2001::level_catalog::Catalog) -> Option<usize> {
    rows(c).iter().find_map(|r| match r {
        Row::Level(i) if c.levels[*i].completed() == [false; 3] => Some(*i),
        _ => None,
    })
}
impl SinglePlayer {
    pub fn open_custom(
        &mut self,
        name: &str,
        catalog: openmt2001::level_catalog::Catalog,
        profile_path: PathBuf,
        scripts_root: PathBuf,
    ) {
        self.open(false, name, catalog, profile_path);
        let catalog = self.catalog.as_ref();
        let items = catalog.map(rows).unwrap_or_default();
        let selected = catalog.and_then(first_uncompleted);
        self.custom = Some(Custom {
            rows: items,
            selected,
            first: 0,
            root: scripts_root,
            roster: None,
        });
    }
    pub(super) fn custom_roster(&mut self) -> Result<(), String> {
        let custom = self.custom.as_mut().ok_or("Custom picker missing")?;
        custom.roster = None;
        let level = &self
            .catalog
            .as_ref()
            .ok_or("Custom catalog missing")?
            .levels[custom.selected.ok_or("Choose a custom scenario")?];
        let path = players_path(&custom.root, level.directory())?;
        let source = fs::read(&path).map_err(|e| e.to_string())?;
        let mut roster = openmt2001::player_script::PendingRoster::default();
        roster.load_normal(
            &path.to_string_lossy(),
            &source,
            self.difficulty as i32,
            self.character as i32,
        )?;
        if roster
            .records
            .iter()
            .any(|r| r.kind != -1 && !(0..10).contains(&r.character))
            || self.name.is_empty()
        {
            return Err("Invalid authored roster or empty profile name".into());
        }
        custom.roster = Some(roster);
        Ok(())
    }
    pub fn keyboard_enabled(&self, i: usize) -> bool {
        if self.tutorial_screen && i == 0 && self.tutorial_index.is_none() {
            return false;
        }
        if i == 0
            && !self.tutorial_screen
            && !self.sandbox
            && self.custom.is_none()
            && self.scenario_id.is_none()
        {
            return false;
        }
        let Some(c) = &self.custom else {
            return true;
        };
        if self.character_screen {
            return true;
        }
        match i {
            0 => c.selected.is_some(),
            1..=4 => true,
            5..=7 => self.scenario_description_limit() > 0,
            8..=10 => self.custom_list_limit() > 0,
            _ => matches!(c.rows.get(i.saturating_sub(11)), Some(Row::Level(_))),
        }
    }
    pub(super) fn custom_count(&self) -> usize {
        11 + self.custom.as_ref().unwrap().rows.len()
    }
    pub(super) fn custom_list_limit(&self) -> usize {
        self.custom.as_ref().unwrap().rows.len().saturating_sub(
            self.scenario_list_layout()
                .map_or(0, |layout| layout.visible_rows()),
        )
    }
    pub(super) fn custom_activate(&mut self, i: usize) {
        match i {
            0 => match self.custom_roster() {
                Ok(()) => {
                    self.character_screen = true;
                    self.description_first = 0;
                    self.message = None;
                }
                Err(e) => self.message = Some(e),
            },
            1 => self.back(),
            2..=4 => {
                self.difficulty = i - 2;
                if let Err(e) = self.custom_roster() {
                    self.message = Some(e);
                }
            }
            5..=7 => self.custom_scroll_control(i, false),
            8..=10 => self.custom_scroll_control(i - 3, true),
            _ => {
                if let Some(index) = self
                    .custom
                    .as_ref()
                    .unwrap()
                    .rows
                    .get(i.saturating_sub(11))
                    .and_then(|r| {
                        if let Row::Level(i) = r {
                            Some(*i)
                        } else {
                            None
                        }
                    })
                {
                    self.custom.as_mut().unwrap().selected = Some(index);
                    self.description_first = 0;
                    self.message = self.custom_roster().err();
                }
            }
        }
    }
    pub(super) fn custom_scroll_control(&mut self, i: usize, list: bool) {
        let maximum = if list {
            self.custom_list_limit()
        } else {
            self.scenario_description_limit()
        };
        let old = if list {
            self.custom.as_ref().unwrap().first
        } else {
            self.description_first
        };
        let value = match i {
            5 => old.saturating_sub(1),
            6 => (old + 1).min(maximum),
            _ => self
                .pointer_position
                .map(|p| {
                    ui::slider_selection(
                        ui::slider_track(
                            if list {
                                SCENARIO_LIST_SCROLL
                            } else {
                                SCENARIO_INFO_SCROLL
                            },
                            true,
                        ),
                        p[1],
                        maximum as i32,
                        true,
                    ) as usize
                })
                .unwrap_or(old),
        };
        if list {
            self.custom.as_mut().unwrap().first = value;
        } else {
            self.description_first = value;
        }
    }
    pub(super) fn custom_hit(&self, p: [f32; 2]) -> Option<usize> {
        let inside = |b: [f32; 4]| p[0] >= b[0] && p[0] <= b[2] && p[1] >= b[1] && p[1] <= b[3];
        for (base, bounds, maximum) in [
            (5, SCENARIO_INFO_SCROLL, self.scenario_description_limit()),
            (8, SCENARIO_LIST_SCROLL, self.custom_list_limit()),
        ] {
            if maximum > 0 {
                if let Some(i) = ui::slider_arrows(bounds, true).into_iter().position(inside) {
                    return Some(base + i);
                }
                if inside(ui::slider_track(bounds, true)) {
                    return Some(base + 2);
                }
            }
        }
        for (i, part) in [26, 25, 19, 18, 17].into_iter().enumerate() {
            if inside(self.scenario.bounds[part])
                && (i != 0 || self.custom.as_ref()?.selected.is_some())
            {
                return Some(i);
            }
        }
        let custom = self.custom.as_ref()?;
        if let Some(row) = self
            .scenario_list_layout()
            .ok()?
            .hit(p, custom.first, custom.rows.len())
            && matches!(custom.rows.get(row), Some(Row::Level(_)))
        {
            return Some(11 + row);
        }
        None
    }
    pub(super) fn custom_text(&self, title: bool) -> Vec<u8> {
        self.custom
            .as_ref()
            .and_then(|c| c.selected)
            .and_then(|i| self.catalog.as_ref()?.levels.get(i))
            .map(|l| self.custom_label(l, title))
            .unwrap_or_default()
    }
    fn custom_label(&self, level: &openmt2001::level_catalog::Level, title: bool) -> Vec<u8> {
        match level.labels() {
            openmt2001::level_catalog::Labels::Localized {
                title: t,
                description: d,
            } => self
                .strings
                .get(if title { t } else { d } as usize)
                .cloned()
                .unwrap_or_default(),
            openmt2001::level_catalog::Labels::Literal {
                title: t,
                description: d,
            } => (if title { t } else { d })
                .into_iter()
                .map(|b| if (32..=126).contains(&b) { b - 32 } else { 31 })
                .collect(),
        }
    }
    pub(super) fn custom_cups_draw(&self, out: &mut Vec<Surface>) -> Result<(), String> {
        let custom = self.custom.as_ref().ok_or("Custom picker missing")?;
        let catalog = self.catalog.as_ref().ok_or("Custom catalog missing")?;
        submit_row_cups(
            &custom.rows,
            custom.first,
            self.scenario.bounds[8],
            catalog,
            |texture, rect| self.image(out, texture, rect),
        )
    }
    pub(super) fn custom_list_draw(
        &self,
        shapes: &mut Surface,
        fonts: &mut [Surface],
    ) -> Result<(), String> {
        let c = self.custom.as_ref().unwrap();
        let layout = self.scenario_list_layout()?;
        for (row, item) in c
            .rows
            .iter()
            .skip(c.first)
            .take(layout.visible_rows())
            .enumerate()
        {
            let mut bounds = layout.text_bounds(row);
            bounds[0] += 3.;
            let text = match item {
                Row::Header(s) => ascii(&String::from_utf8_lossy(s)),
                Row::Space => vec![],
                Row::Level(i) => {
                    if c.selected == Some(*i) {
                        shapes.rect(layout.highlight(row), [0xff008ed0; 4]);
                    }
                    self.custom_label(&self.catalog.as_ref().unwrap().levels[*i], true)
                }
            };
            ui::text(
                fonts,
                &self.scenario_list_tab,
                &text,
                bounds,
                false,
                0xffffffff,
            )?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn groups_are_contiguous_case_sensitive_and_ignore_unlock() {
        use openmt2001::level_catalog::{Catalog, Labels, Registration};
        let mut c = Catalog::default();
        for (i, path) in ["custom\\A\\one", "custom\\a\\two", "custom\\A\\three"]
            .iter()
            .enumerate()
        {
            c.register(
                Registration {
                    id: 500000 + i as i32,
                    directory: path.as_bytes().to_vec(),
                    labels: Labels::Literal {
                        title: vec![],
                        description: vec![],
                    },
                    checksum: 0,
                },
                false,
                true,
            )
            .unwrap();
        }
        c.reset_progress();
        assert_eq!(
            rows(&c),
            vec![
                Row::Header(b"A".to_vec()),
                Row::Level(0),
                Row::Space,
                Row::Header(b"a".to_vec()),
                Row::Level(1),
                Row::Space,
                Row::Header(b"A".to_vec()),
                Row::Level(2)
            ]
        );
    }
    #[test]
    fn slash_paths_preserve_distinct_author_groups() {
        use openmt2001::level_catalog::{Catalog, Labels, Registration};
        let mut c = Catalog::default();
        for (i, path) in ["custom/One/first", "custom/Two/second"].iter().enumerate() {
            c.register(
                Registration {
                    id: 500000 + i as i32,
                    directory: path.as_bytes().to_vec(),
                    labels: Labels::Literal {
                        title: vec![],
                        description: vec![],
                    },
                    checksum: 0,
                },
                false,
                true,
            )
            .unwrap();
        }
        assert_eq!(
            rows(&c),
            vec![
                Row::Header(b"One".to_vec()),
                Row::Level(0),
                Row::Space,
                Row::Header(b"Two".to_vec()),
                Row::Level(1)
            ]
        );
    }
    #[test]
    fn empty_and_completed_catalog_have_no_default_choice() {
        use openmt2001::level_catalog::{Catalog, Labels, Registration};
        let mut c = Catalog::default();
        assert_eq!(first_uncompleted(&c), None);
        c.register(
            Registration {
                id: 500000,
                directory: b"custom\\author\\level".to_vec(),
                labels: Labels::Literal {
                    title: b"Literal title".to_vec(),
                    description: b"Description".to_vec(),
                },
                checksum: 0,
            },
            false,
            true,
        )
        .unwrap();
        assert_eq!(first_uncompleted(&c), Some(0));
        c.levels[0].bytes[0x874] = 1;
        assert_eq!(first_uncompleted(&c), None);
        assert!(matches!(rows(&c).last(), Some(Row::Level(0))));
    }
    #[test]
    fn traversal_is_refused() {
        assert!(players_path(Path::new("/"), b"../players").is_err());
    }
    #[test]
    fn custom_cup_submissions_follow_viewport_and_independent_bytes() {
        use openmt2001::level_catalog::{Catalog, Labels, Registration};
        let mut catalog = Catalog::default();
        for id in [500000, 500001] {
            catalog
                .register(
                    Registration {
                        id,
                        directory: b"custom/author/level".to_vec(),
                        labels: Labels::Literal {
                            title: vec![],
                            description: vec![],
                        },
                        checksum: 0,
                    },
                    false,
                    true,
                )
                .unwrap();
        }
        catalog.levels[0].bytes[0x874..0x877].copy_from_slice(&[1, 0, 1]);
        catalog.levels[1].bytes[0x874..0x877].copy_from_slice(&[0, 1, 0]);
        let rows = [
            Row::Header(vec![]),
            Row::Level(0),
            Row::Space,
            Row::Header(vec![]),
            Row::Level(1),
            Row::Level(usize::MAX),
        ];
        // Exactly five visible text rows. The sixth row must not submit anything.
        let bounds = [32., 97., 300., 157.];
        let mut draws = vec![];
        submit_row_cups(&rows, 0, bounds, &catalog, |id, rect| {
            draws.push((id, rect));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            draws,
            vec![
                (297, [242., 110., 259., 124.]),
                (296, [262., 110., 279., 124.]),
                (299, [282., 110., 299., 124.]),
                (296, [242., 146., 259., 160.]),
                (298, [262., 146., 279., 160.]),
                (296, [282., 146., 299., 160.]),
            ]
        );
        draws.clear();
        // Scrolling uses viewport-relative y, never catalog index or absolute row.
        submit_row_cups(&rows, 4, [32., 97., 300., 121.], &catalog, |id, rect| {
            draws.push((id, rect));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            draws,
            vec![
                (296, [242., 98., 259., 112.]),
                (298, [262., 98., 279., 112.]),
                (296, [282., 98., 299., 112.]),
            ]
        );
        draws.clear();
        submit_row_cups(
            &[Row::Level(0), Row::Level(1)],
            0,
            [32., 97., 300., 109.],
            &catalog,
            |id, rect| {
                draws.push((id, rect));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(draws.len(), 3); // A valid offscreen level must not submit.
        assert_eq!(draws[0], (297, [242., 98., 259., 112.]));
        submit_row_cups(
            &[Row::Level(0)],
            0,
            [32., 97., 300., 108.],
            &catalog,
            |_, _| panic!("partial text row must not submit"),
        )
        .unwrap();
        for first in [rows.len(), rows.len() + 1] {
            submit_row_cups(&rows, first, bounds, &catalog, |_, _| {
                panic!("off-end rows must not submit")
            })
            .unwrap();
        }
        let mut calls = 0;
        assert_eq!(
            submit_row_cups(&rows, 0, bounds, &catalog, |_, _| {
                calls += 1;
                Err("submission failure".into())
            }),
            Err("submission failure".into())
        );
        assert_eq!(calls, 1);
    }
}
