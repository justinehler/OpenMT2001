//! Board-version panel: native492090 layout,492380 selection,492590 accept.
use super::*;
impl Assets {
    #[cfg(test)]
    pub(crate) fn board_point_for_test(
        &self,
        name: crate::board_selection::Name,
    ) -> Option<[f32; 2]> {
        let row = self.boards.iter().position(|entry| entry.name == name)?;
        let bounds = self.panel(Page::Board).regions[2];
        Some([
            bounds[0] + 2.,
            bounds[1] + row as f32 * self.board_pitch + 2.,
        ])
    }

    pub(super) fn board_visible_rows(&self) -> usize {
        let r = self.panel(Page::Board).regions[2];
        (((r[3] - r[1]) / self.board_pitch) as usize).max(1)
    }
    pub(super) fn board_scroll_bounds(&self) -> [f32; 4] {
        let [_, t, r, b] = self.panel(Page::Board).regions[2];
        [r - 16., t.trunc(), r, t.trunc() + (b - t).trunc()]
    }
    pub(super) fn prepare_board(
        &self,
        renderer: &Renderer,
        selected: Option<usize>,
        scroll: usize,
        hovered: Option<Button>,
        pressed: Option<Button>,
    ) -> Result<PreparedScene, String> {
        let surfaces = self.board_surfaces(selected, scroll, hovered, pressed)?;
        let draws = self
            .panel(Page::Board)
            .base
            .iter()
            .chain(&surfaces)
            .map(Surface::draw)
            .collect::<Vec<_>>();
        renderer.prepare(&draws, VIEW, None)
    }
    fn board_surfaces(
        &self,
        selected: Option<usize>,
        scroll: usize,
        hovered: Option<Button>,
        pressed: Option<Button>,
    ) -> Result<Vec<Surface>, String> {
        let regions = &self.panel(Page::Board).regions;
        let mut art = Surface::solid();
        let mut title = self.fonts.clone();
        let mut text = self.font3.clone();
        let mut actions = self.font1.clone();
        art.rect(regions[0], [0xff6e04ab, 0xff6e04ab, 0x806e04ab, 0x806e04ab]);
        art.rect(regions[1], [0x80909090; 4]);
        crate::ui::text_widget_style9(
            &mut title,
            &self.tab,
            self.label(1568),
            regions[0],
            0xffffffff,
        )?;
        crate::ui::text_widget_style9(
            &mut text,
            &self.tab3,
            self.label(1569),
            regions[1],
            0xffffffff,
        )?;
        let [l, t, r, b] = regions[2];
        art.rect([l, t, r, b + 2.], [0xbb000a28; 4]);
        for (row, entry) in self
            .boards
            .iter()
            .enumerate()
            .skip(scroll)
            .take(self.board_visible_rows())
        {
            let top = t + (row - scroll) as f32 * self.board_pitch;
            let bounds = [l, top, r - 16., top + self.board_pitch];
            if selected == Some(row) || hovered == Some(Button::BoardRow(row)) {
                //42e430 draws full-pitch bars at+2 using536294/5362ac.
                art.rect(
                    [l, top + 2., r - 16., top + self.board_pitch + 2.],
                    [if hovered == Some(Button::BoardRow(row)) {
                        0xbb6ed1e1
                    } else {
                        0xff288be1
                    }; 4],
                );
            }
            crate::ui::text_widget_style0(&mut text, &self.tab3, &entry.label, bounds, 0xff90c0c0)?;
        }
        let mut icons = Vec::new();
        let point = |button| match button {
            Some(Button::BoardUp) => Some([r - 8., t + 8.]),
            Some(Button::BoardDown) => Some([r - 8., b - 8.]),
            _ => None,
        };
        crate::ui::Slider {
            bounds: self.board_scroll_bounds(),
            vertical: true,
            value: scroll as f32,
            range: [
                0.,
                self.boards.len().saturating_sub(self.board_visible_rows()) as f32,
            ],
            pointer: point(hovered),
            pressed: point(pressed),
        }
        .paint(&mut art, &mut icons, &self.slider_textures);
        let ok = regions[4];
        self.button_surface(
            &mut art,
            ok,
            [80, 148, 34],
            hovered == Some(Button::Back),
            pressed == Some(Button::Back),
        )?;
        crate::ui::text_widget_style9(&mut actions, &self.tab1, self.label(1435), ok, 0xffffffff)?;
        let mut surfaces = vec![art];
        surfaces.extend(icons);
        surfaces.extend(title);
        surfaces.extend(text);
        surfaces.extend(actions);
        Ok(surfaces)
    }
}
impl View {
    pub(crate) fn board_language(
        &self,
        installer: &Path,
    ) -> Result<openmt2001::packed_language::PackedLanguage, String> {
        self.board.language(installer)
    }

    pub fn set_board(&mut self, board: crate::board_selection::Name) {
        self.board = board;
        self.saved_board = board;
    }
    pub fn persist_board(&mut self, options: &Path) -> Result<(), String> {
        if self.board != self.saved_board
            || !options
                .with_extension("board")
                .try_exists()
                .map_err(|e| e.to_string())?
        {
            self.board.save(options)?;
            self.saved_board = self.board;
        }
        Ok(())
    }
    pub fn scroll_boards(&mut self, assets: &Assets, delta: i32) {
        if self.page == Some(Page::Board) {
            self.board_scroll = (self.board_scroll as i64 - i64::from(delta)).clamp(
                0,
                assets
                    .boards
                    .len()
                    .saturating_sub(assets.board_visible_rows()) as i64,
            ) as usize;
            self.shown = None;
        }
    }
    pub(super) fn board_hit(&self, assets: &Assets, pointer: [f32; 2]) -> Option<Button> {
        let regions = &assets.panel(Page::Board).regions;
        if ui_mesh::parent_hit(regions[4], pointer, [0.; 2]) {
            return Some(Button::Back);
        }
        let [l, t, r, b] = regions[2];
        let [x, y] = pointer;
        if x < l || x > r || y < t || y > b + 2. {
            return None;
        }
        if x >= r - 16. {
            return Some(if y < t + 16. {
                Button::BoardUp
            } else if y > b - 16. {
                Button::BoardDown
            } else {
                Button::BoardTrack
            });
        }
        let row = self.board_scroll + ((y - t) / assets.board_pitch) as usize;
        (row < assets.boards.len()).then_some(Button::BoardRow(row))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board_selection::{self, Name};
    use openmt2001::packed_language::PackedLanguage;

    fn click(view: &mut View, assets: &Assets, point: [f32; 2]) {
        let mut audio = Levels::default();
        view.mouse(assets, Some(point), true, &mut audio);
        view.mouse(assets, Some(point), false, &mut audio);
    }
    fn center(bounds: [f32; 4]) -> [f32; 2] {
        [(bounds[0] + bounds[2]) / 2., (bounds[1] + bounds[3]) / 2.]
    }
    fn write_language(path: &Path, rows: &[&[u8]]) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("tycoon.off"),
            rows.iter()
                .flat_map(|row| (row.len() as u16).to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        fs::write(path.join("tycoon.bin"), rows.concat()).unwrap();
        fs::write(path.join("board.prm"), b"// synthetic board").unwrap();
    }

    #[test]
    fn board_settings_route_accept_cancel_scroll_and_profile_reload_change_city_name_source() {
        let root = std::env::temp_dir().join(format!(
            "openmt-board-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let us = Name::parse(b"usa").unwrap();
        let other = Name::parse(b"synthetic").unwrap();
        write_language(
            &us.directory(&root),
            &[b"map", b"Country A", b"$", b"L", b"First Street"],
        );
        write_language(
            &other.directory(&root),
            &[b"map", b"Country B", b"kr", b"R", b"Second Street"],
        );
        let mut assets = super::super::settings_draw_tests::synthetic_assets();
        assets.boards = board_selection::catalog(&root).unwrap();
        assert_eq!(
            assets.boards.iter().map(|b| b.name).collect::<Vec<_>>(),
            [other, us]
        );
        assets.panels[Page::Board.index()].regions = vec![
            [10., 10., 610., 50.],
            [170., 120., 455., 140.],
            [170., 140., 455., 151.],
            [0.; 4],
            [270., 370., 360., 400.],
        ];
        let board_button = center(assets.bounds(Page::Main, Button::Board).unwrap());
        let mut view = View::default();
        view.set_board(us);
        let initial_profile = crate::profile_settings::profile_path(&root, b"Initial");
        view.persist_board(&initial_profile).unwrap();
        assert_eq!(
            Name::load(&initial_profile).unwrap(),
            us,
            "new profiles freeze their installed default too"
        );
        view.open_in_game();
        click(&mut view, &assets, board_button);
        assert_eq!(view.page, Some(Page::Main), "running city forbids Board");
        view.open();
        click(&mut view, &assets, board_button);
        assert_eq!(view.page, Some(Page::Board));
        assert_eq!(view.board_row, Some(1));
        assert_eq!(view.board_scroll, 1);
        view.scroll_boards(&assets, 1);
        let mut audio = Levels::default();
        view.mouse(&assets, Some([180., 145.]), true, &mut audio);
        assert_eq!(view.board_row, Some(0), "native list stages on mouse-down");
        assert_eq!(view.board, us);
        view.mouse(&assets, Some([10., 10.]), false, &mut audio);
        view.back();
        assert_eq!(view.board, us, "Escape cancels staged row");
        click(&mut view, &assets, board_button);
        view.scroll_boards(&assets, 1);
        click(&mut view, &assets, [180., 145.]);
        assert_eq!(view.board, us, "row selection waits for Ok");
        click(
            &mut view,
            &assets,
            center(assets.bounds(Page::Board, Button::Back).unwrap()),
        );
        assert_eq!(view.page, Some(Page::Main));
        assert_eq!(view.board, other);
        let profile = crate::profile_settings::profile_path(&root, b"Alice");
        view.persist_board(&profile).unwrap();
        let mut restored = View::default();
        restored.set_board(Name::load(&profile).unwrap());
        let language = restored.board_language(&root).unwrap(); // same source used by Intro::sync_board_names
        let mut city_strings = vec![Vec::new(); 2246];
        board_selection::overlay(&mut city_strings, &language).unwrap();
        let mut shared = PackedLanguage::parse(&[0; 2246 * 2], Vec::new()).unwrap();
        shared.overlay(2242, &language).unwrap();
        assert_eq!(shared.row(2245).unwrap(), b"Second Street");
        assert_eq!(shared.row(2243).unwrap(), b"kr");
        assert_eq!(city_strings[2245], b"Second Street");
        assert_eq!(city_strings[2243], b"kr");
        assert_eq!(city_strings[2244], b"R");
        assert_eq!(
            Name::load(&crate::profile_settings::profile_path(&root, b"Bob")).unwrap(),
            Name::initial()
        );
        assert!(Name::parse(b"../outside").is_err());
        assert!(Name::parse(b"").is_err());
        assert!(board_selection::overlay(&mut city_strings[..2245], &language).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires supplied original board languages, settings mesh and v1.4 executable"]
    fn original_board_catalog_and_german_selection_use_supplied_names() {
        let installer = openmt2001::fixtures::path("INSTALLER");
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("ARCHIVE_DIR");
        let archive = read("ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let language = openmt2001::fixtures::path("LANGUAGE");
        let assets = Assets::load(&index, &archive, &installer, &language).unwrap();
        let mut view = View::default();
        view.open();
        click(
            &mut view,
            &assets,
            center(assets.bounds(Page::Main, Button::Board).unwrap()),
        );
        let german = Name::parse(b"ger").unwrap();
        let row = assets.boards.iter().position(|b| b.name == german).unwrap();
        view.scroll_boards(&assets, 100);
        let bounds = assets.panel(Page::Board).regions[2];
        click(
            &mut view,
            &assets,
            [
                bounds[0] + 2.,
                bounds[1] + row as f32 * assets.board_pitch + 2.,
            ],
        );
        click(
            &mut view,
            &assets,
            center(assets.bounds(Page::Board, Button::Back).unwrap()),
        );
        assert_eq!(view.board, german);
        let packed = view.board_language(&installer).unwrap();
        let mut localized = PackedLanguage::load(&language.join("tycoon")).unwrap();
        localized.overlay(2242, &packed).unwrap();
        let dictionary = localized.row(0).unwrap();
        assert!(localized.rows().iter().skip(2242).any(|row| {
            row.iter()
                .map(|glyph| dictionary.iter().position(|b| b == glyph).unwrap() as u8 + 32)
                .collect::<Vec<_>>()
                == b"Goethestra\xdfe"
        }));
        assert_eq!(assets.boards[row].label, packed.row(1).unwrap());
        // USA is near the bottom;492380 scrolls enough to make it visible.
        view.set_board(Name::parse(b"usa").unwrap());
        click(
            &mut view,
            &assets,
            center(assets.bounds(Page::Main, Button::Board).unwrap()),
        );
        assert_eq!(
            view.board_scroll,
            assets.boards.len() - assets.board_visible_rows()
        );
        assert_eq!(
            assets.boards[view.board_scroll].name,
            Name::parse(b"dan").unwrap()
        );
        let surfaces = assets
            .board_surfaces(view.board_row, view.board_scroll, None, None)
            .unwrap();
        assert!(surfaces.iter().any(|surface| !surface.vertices.is_empty()));
        let executable = read("EXE");
        let mut resources = crate::intro::camera_live::ui_text_resources::UiTextResources::load(
            &executable,
            &installer,
            &language,
            &index,
            &archive,
        )
        .unwrap();
        resources
            .install_board(&executable, &installer, german)
            .unwrap();
        for id in 2242..localized.row_count() as u32 {
            assert_eq!(
                resources.fonts().language.row(id).unwrap(),
                localized.row(id).unwrap()
            );
        }
        let before = resources.fonts().language.bytes().to_vec();
        assert!(
            resources
                .install_board(
                    &executable,
                    &installer,
                    Name::parse(b"missing_board").unwrap()
                )
                .is_err()
        );
        assert_eq!(resources.fonts().language.bytes(), before);
        resources
            .install_board(&executable, &installer, Name::parse(b"usa").unwrap())
            .unwrap();
        let usa = Name::parse(b"usa").unwrap().language(&installer).unwrap();
        assert_eq!(
            resources.fonts().language.row(2242).unwrap(),
            usa.row(1).unwrap()
        );
    }
}
