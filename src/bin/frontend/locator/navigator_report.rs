//! Navigator title (40d080), city lists (40e690/40e9d0), and finance (413500).
use super::{Action, report_text};
use crate::{
    player_info,
    ui::{self, Surface},
};

pub(super) fn title_bounds(first: [f32; 4], second: [f32; 4]) -> [f32; 4] {
    [first[0] + 8., first[1], first[2] + 8., second[3]]
}

pub(super) fn paint_title(
    pages: &[Surface],
    tab: &[u8],
    text: &[u8],
    bounds: [f32; 4],
    color: u32,
) -> Result<Vec<Surface>, String> {
    let mut pages = pages.to_vec();
    // Same font1/style4 widget as the selected-block title.
    ui::text_widget_style4(&mut pages, tab, text, bounds, color)?;
    Ok(pages)
}

pub(super) struct Layout {
    pub(super) list: report_text::Layout,
    pub(super) scrollbar: [f32; 4],
}
impl Layout {
    pub(super) fn new(main: [f32; 4], slider_width: f32) -> Self {
        // 40df24 calls 413500 at main.top+60; both children stop five
        // pixels before main.bottom, and the list reserves the choice width.
        let [l, t, r, b] = main;
        Self {
            list: report_text::Layout {
                bounds: [l, t + 60., r - slider_width, b - 5.],
            },
            scrollbar: [r - slider_width, t + 60., r, b - 5.],
        }
    }
    pub(super) fn city(main: [f32; 4], slider_width: f32) -> Self {
        // 40e690/40e9d0: list top+40, bottom-5; choice top+42 to bottom.
        let [l, t, r, b] = main;
        Self {
            list: report_text::Layout {
                bounds: [l, t + 40., r - slider_width, b - 5.],
            },
            scrollbar: [r - slider_width, (t + 42.).trunc(), r, b.trunc()],
        }
    }
    pub(super) fn row_at(&self, point: [f32; 2]) -> Option<usize> {
        let [l, t, r, _] = self.list.bounds;
        let top = t + 3.;
        let bottom = top + self.list.visible_rows() as f32 * report_text::ROW_PITCH;
        super::inside(point, [l, top, r, bottom])
            .then(|| ((point[1] - top) / report_text::ROW_PITCH) as usize)
    }
    pub(super) fn maximum(&self, count: usize) -> usize {
        count.saturating_sub(self.list.visible_rows())
    }
    pub(super) fn hit(&self, point: [f32; 2], count: usize) -> Option<Action> {
        if self.maximum(count) == 0 {
            return None;
        }
        for (i, bounds) in ui::slider_arrows(self.scrollbar, true)
            .into_iter()
            .enumerate()
        {
            if super::inside(point, bounds) {
                return Some(Action::Scroll(if i == 0 { -1 } else { 1 }));
            }
        }
        super::inside(point, ui::slider_track(self.scrollbar, true)).then_some(Action::ScrollTrack)
    }
    pub(super) fn selection(&self, y: f32, count: usize) -> usize {
        ui::slider_selection(
            ui::slider_track(self.scrollbar, true),
            y,
            self.maximum(count) as i32,
            true,
        ) as usize
    }
}

pub(super) struct List<'a> {
    pub(super) pages: &'a [Surface],
    pub(super) tab: &'a [u8],
    pub(super) strings: &'a [Vec<u8>],
    pub(super) palette: &'a [u32; 18],
    pub(super) arrows: &'a [[Surface; 4]; 2],
}
impl List<'_> {
    pub(super) fn paint(
        &self,
        player: &player_info::State,
        layout: &Layout,
        hover: Option<Action>,
        pressed: Option<Action>,
    ) -> Result<Vec<Surface>, String> {
        let mut pages = self.pages.to_vec();
        let mut solid = Surface::solid();
        for (i, row) in player
            .rows
            .iter()
            .skip(player.scroll)
            .take(layout.list.visible_rows())
            .enumerate()
        {
            if row.style >= 2 {
                continue;
            }
            let label = self
                .strings
                .get(row.label as usize)
                .ok_or("finance label leaves language")?;
            if row.style == 1 {
                solid.rect(layout.list.heading(i), [self.palette[17]; 4]);
                ui::text(
                    &mut pages,
                    self.tab,
                    label,
                    layout.list.line(i, false),
                    false,
                    self.palette[16],
                )?;
            } else {
                let symbol = self
                    .strings
                    .get(2243)
                    .ok_or("finance currency symbol missing")?;
                let value = ui::formatted_float_money(f32::from_bits(row.amount), symbol, false)?;
                report_text::paint_linked_pair(
                    &mut pages,
                    self.tab,
                    &layout.list,
                    i,
                    [label, &value],
                    [self.palette[0], self.palette[report_text::VALUE_PALETTE]],
                )?;
            }
        }
        let mut out = self.paint_scrollbar(
            solid,
            layout,
            player.rows.len(),
            player.scroll,
            hover,
            pressed,
        );
        out.extend(pages);
        Ok(out)
    }
    fn paint_scrollbar(
        &self,
        mut solid: Surface,
        layout: &Layout,
        count: usize,
        scroll: usize,
        hover: Option<Action>,
        pressed: Option<Action>,
    ) -> Vec<Surface> {
        let maximum = layout.maximum(count);
        let slider = ui::Slider {
            bounds: layout.scrollbar,
            vertical: true,
            value: scroll as f32,
            range: [0., maximum as f32],
            pointer: None,
            pressed: None,
        };
        let (bounds, color) =
            ui::slider_background(ui::slider_track(layout.scrollbar, true), true, maximum > 0);
        solid.rect(bounds, [color; 4]);
        slider.paint_handle(&mut solid);
        let mut out = vec![solid];
        for (side, bounds) in ui::slider_arrows(layout.scrollbar, true)
            .into_iter()
            .enumerate()
        {
            let action = Some(Action::Scroll(if side == 0 { -1 } else { 1 }));
            let texture = ui::slider_arrow_texture(
                side,
                pressed == action,
                hover == action,
                maximum > 0,
                true,
            );
            let mut arrow = self.arrows[side][texture - [105, 93][side]].clone();
            let [l, t, r, b] = bounds;
            arrow.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            out.push(arrow);
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CityRow {
    Space,
    Grade(i32),
    Player(usize),
    Empty,
    Block(usize),
}

pub(super) fn city_rows(state: &super::Locator) -> Vec<CityRow> {
    if state.prestige_mode {
        state.prestige.as_ref().map_or_else(Vec::new, |report| {
            report
                .rows
                .iter()
                .map(|row| match row {
                    crate::city_prestige::Row::Space => CityRow::Space,
                    crate::city_prestige::Row::Grade(grade) => CityRow::Grade(*grade),
                    crate::city_prestige::Row::Block(id) => CityRow::Block(*id),
                })
                .collect()
        })
    } else if state.ownership_mode {
        state.ownership.as_ref().map_or_else(Vec::new, |report| {
            report
                .groups
                .iter()
                .enumerate()
                .flat_map(|(slot, group)| {
                    std::iter::once(CityRow::Player(slot)).chain(if group.is_empty() {
                        vec![CityRow::Empty]
                    } else {
                        group.iter().copied().map(CityRow::Block).collect()
                    })
                })
                .collect()
        })
    } else {
        Vec::new()
    }
}

pub(super) fn city_hit(state: &super::Locator, layout: &Layout, point: [f32; 2]) -> Option<Action> {
    let rows = city_rows(state);
    layout.hit(point, rows.len()).or_else(|| {
        let row = layout.row_at(point)?;
        match rows.get(state.ownership_scroll + row) {
            Some(CityRow::Block(category)) => Some(Action::OwnershipBlock(*category)),
            _ => None,
        }
    })
}

pub(super) struct City<'a> {
    pub(super) list: List<'a>,
    pub(super) short_labels: &'a [Vec<u8>; 52],
    pub(super) ownership_labels: &'a [Vec<u8>; 52],
}
impl City<'_> {
    pub(super) fn paint(
        &self,
        state: &super::Locator,
        layout: &Layout,
    ) -> Result<Vec<Surface>, String> {
        let rows = city_rows(state);
        let mut pages = self.list.pages.to_vec();
        let mut solid = Surface::solid();
        for (i, row) in rows
            .iter()
            .skip(state.ownership_scroll)
            .take(layout.list.visible_rows())
            .enumerate()
        {
            let heading = matches!(row, CityRow::Grade(_) | CityRow::Player(_));
            let text = match row {
                CityRow::Space => continue,
                CityRow::Grade(grade) => {
                    let template = &self.list.strings[43];
                    let marker = [b'%' - 32, b'd' - 32];
                    let at = template
                        .windows(2)
                        .position(|w| w == marker)
                        .ok_or("prestige grade template lacks original integer field")?;
                    let mut text = template[..at].to_vec();
                    text.extend(grade.to_string().bytes().map(|b| b - 32));
                    text.extend_from_slice(&template[at + 2..]);
                    text
                }
                CityRow::Player(slot) => {
                    let player = state
                        .player
                        .records
                        .get(*slot)
                        .ok_or("ownership player unavailable")?;
                    let name = &player[0x154..];
                    let end = name
                        .iter()
                        .position(|&b| b == 0)
                        .ok_or("unterminated ownership player name")?;
                    name[..end]
                        .iter()
                        .map(|b| {
                            if (32..=126).contains(b) {
                                Ok(b - 32)
                            } else {
                                Err("unsupported ownership name glyph".to_string())
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
                CityRow::Empty => self.list.strings[19].clone(),
                CityRow::Block(id) => {
                    // 40f6c0 selects board base+1 (short mixed-case); 40eb70
                    // selects base+2 (mixed-case full name), never base+0 caps.
                    let labels = if state.prestige_mode {
                        self.short_labels
                    } else {
                        self.ownership_labels
                    };
                    labels
                        .get(*id)
                        .ok_or("city report category leaves board")?
                        .clone()
                }
            };
            if heading {
                solid.rect(layout.list.heading(i), [self.list.palette[17]; 4]);
            }
            // Shared font3 and heading bar, also used by List/block reports.
            let mut bounds = layout.list.line(i, false);
            bounds[3] += report_text::ROW_PITCH;
            ui::text(
                &mut pages,
                self.list.tab,
                &text,
                bounds,
                false,
                self.list.palette[if heading { 16 } else { 0 }],
            )?;
        }
        let mut out = self.list.paint_scrollbar(
            solid,
            layout,
            rows.len(),
            state.ownership_scroll,
            state.hover,
            state.pressed,
        );
        out.extend(pages);
        Ok(out)
    }
}

pub(super) fn paint_rights_badge(icon: &Surface, badge: crate::city_rights::Badge) -> Vec<Surface> {
    let mut background = Surface::solid();
    background.rect(badge.background, [badge.color; 4]);
    let mut icon = icon.clone();
    let [l, t, r, b] = badge.icon_rect;
    icon.quad(
        [[l, t], [l, b], [r, b], [r, t]],
        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
        [badge.icon_color; 4],
    );
    vec![background, icon]
}

pub(super) struct Rights<'a> {
    pub(super) pages: &'a [Surface],
    pub(super) tab: &'a [u8],
    pub(super) descriptions: &'a [Surface],
    pub(super) description_tab: &'a [u8],
    pub(super) strings: &'a [Vec<u8>],
    pub(super) icon: &'a Surface,
    pub(super) color: u32,
}
impl Rights<'_> {
    pub(super) fn paint(&self, main: [f32; 4]) -> Result<Vec<Surface>, String> {
        let [l, t, r, _] = main;
        let mut out = Vec::new();
        let mut pages = self.pages.to_vec();
        let mut body = self.descriptions.to_vec();
        let mut rules = Surface::solid();
        // 40ecd0 constructs only these eight captions. 411dd0 draws the
        // marker centers at +60/110/170/230 and rules at +55/105/165/225.
        for (title, description, y, mode, owned) in [
            (13, 14, 43., 0, false),
            (15, 16, 93., 1, false),
            (17, 18, 153., 1, true),
            (1801, 1441, 213., 0, true),
        ] {
            if let Some(badge) = crate::city_rights::badge(
                mode,
                owned,
                [l + 20., t + y + 17.],
                [self.icon.width, self.icon.height],
                0,
            ) {
                out.extend(paint_rights_badge(self.icon, badge));
            }
            ui::text_widget_style0(
                &mut pages,
                self.tab,
                &self.strings[title],
                [l + 40., t + y, r + 2., t + y + 30.],
                self.color,
            )?;
            rules.rect([l + 40., t + y + 12., r, t + y + 13.], [0xffe55f49; 4]);
            let (offsets, text) = crate::prompt::wrap_with_gap(
                self.description_tab,
                &self.strings[description],
                (r - l - 38.) as i32,
                -1,
            )?;
            for (line, pair) in offsets.windows(2).enumerate() {
                let encoded =
                    &text[pair[0].min(text.len())..pair[1].saturating_sub(1).min(text.len())];
                ui::text_widget_style0(
                    &mut body,
                    self.description_tab,
                    encoded,
                    [
                        l + 40.,
                        t + y + 14. + line as f32 * 10.,
                        r + 2.,
                        t + y + 94.,
                    ],
                    self.color,
                )?;
            }
        }
        out.push(rules);
        out.extend(pages);
        out.extend(body);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locator::strategic::{self, EnabledBufferProvenance};
    use openmt2001::{
        economy::PlayerAccount,
        player_manager::{Manager, Player},
    };
    use std::sync::Arc;

    fn original_assets() -> super::super::Assets {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let archive = read("ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&read("ARCHIVE_DIR"), archive.len() as u64)
                .unwrap();
        let language = crate::intro::camera_live::ui_text_resources::PackedLanguage::load(
            &openmt2001::fixtures::path("LANGUAGE").join("tycoon"),
        )
        .unwrap();
        super::super::Assets::load(
            &index,
            &archive,
            &openmt2001::fixtures::path("INSTALLER"),
            &language.rows(),
            &read("EXE"),
        )
        .unwrap()
    }

    #[test]
    #[ignore = "requires supplied Navigator mesh, fonts, language and executable"]
    fn navigator_assets_connect_title_font_and_finance_viewport() {
        let assets = original_assets();
        let tab = std::fs::read(
            openmt2001::fixtures::path("INSTALLER").join("Standard/fonts/tahoma12bldnoantip.tab"),
        )
        .unwrap();
        assert_eq!(assets.property_title_tab, tab);
        for id in [1450, 1452] {
            let pages = paint_title(
                &assets.property_title_fonts,
                &assets.property_title_tab,
                &assets.strings[id],
                assets.navigator_title_bounds,
                assets.text_palette[0],
            )
            .unwrap();
            assert!(pages.iter().any(|page| !page.vertices.is_empty()));
        }
        let mut locator = assets.people_state().unwrap();
        assert_eq!(
            locator.player.visible_rows,
            assets.finance_layout().list.visible_rows()
        );
        assert_eq!(assets.finance_layout().scrollbar[1], assets.bounds[1] + 60.);
        assert_eq!(assets.finance_layout().scrollbar[3], assets.bounds[3] - 5.);
        let owner = finance_owner();
        locator.attach_players(player_info::State::new(&owner.input.financial.records).unwrap());
        locator.player_info = true;
        locator.open = true;
        let [l, _, r, b] = assets.finance_layout().scrollbar;
        assert_eq!(
            assets.hit(&locator, [(l + r) as i32, ((b - 8.) * 2.) as i32]),
            Action::Scroll(1)
        );
        locator.scroll(&assets, 1000);
        assert_eq!(
            locator.player.scroll,
            assets.finance_layout().maximum(locator.player.rows.len())
        );
    }

    #[test]
    #[ignore = "requires supplied board language, Navigator mesh, fonts and executable"]
    fn navigator_original_city_builders_supply_names_layout_input_and_legend() {
        let assets = original_assets();
        assert_eq!(
            assets.tab,
            std::fs::read(openmt2001::fixtures::path("INSTALLER").join(report_text::FONT_TABLE))
                .unwrap()
        );
        let exe = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let names =
            crate::tooltip::board_strings(&openmt2001::fixtures::path("INSTALLER")).unwrap();
        let mut abbreviated = false;
        for block in &board.blocks {
            let base = block.text_id as usize - 2242 + 1;
            assert_eq!(assets.short_labels[block.id], names[base + 1]);
            assert_eq!(assets.ownership_report_labels[block.id], names[base + 2]);
            abbreviated |= names[base + 1] != names[base + 2];
        }
        assert!(
            abbreviated,
            "the board supplies distinct short/full mixed-case names"
        );
        for (address, value) in [(0x525078, 40f32), (0x525020, 5.), (0x5250c4, 42.)] {
            assert_eq!(image.at(address, 4).unwrap(), value.to_le_bytes());
        }
        let mut state = city_locator();
        let layout = assets.city_report_layout();
        state.city_visible_rows = layout.list.visible_rows();
        for prestige in [true, false] {
            state.prestige_mode = prestige;
            state.ownership_mode = !prestige;
            state.ownership_scroll = 0;
            let output = assets
                .city_report(&state, &mut assets.fonts.clone())
                .unwrap();
            assert!(output.iter().any(|p| !p.vertices.is_empty()));
            let [l, _, r, b] = layout.scrollbar;
            assert_eq!(
                assets.hit(&state, [(l + r) as i32, ((b - 8.) * 2.) as i32]),
                Action::Scroll(1)
            );
            state.scroll(&assets, 1000);
            assert_eq!(
                state.ownership_scroll,
                layout.maximum(city_rows(&state).len())
            );
            state.scroll(&assets, -1000);
            assert_eq!(state.ownership_scroll, 0);
        }
        // The legend takes no player data: 40ecd0 builds only the eight
        // fixed captions, so no player-name line can appear above them.
        let legend = assets.rights_report(&mut assets.fonts.clone()).unwrap();
        let rule = &legend[8];
        assert_eq!(rule.vertices.len(), 24);
        for (address, value) in [
            (0x5250fc, 55f32),
            (0x5250f8, 105.),
            (0x5250f4, 165.),
            (0x5250f0, 225.),
        ] {
            assert_eq!(image.at(address, 4).unwrap(), value.to_le_bytes());
        }
    }

    fn encoded(text: &str) -> Vec<u8> {
        text.bytes().map(|b| b - 32).collect()
    }

    fn city_locator() -> super::super::Locator {
        let mut state = crate::locator::refresh_ui_tests::fresh();
        state.city_info = true;
        state.open = true;
        state.prestige_mode = true;
        state.prestige = Some(crate::city_prestige::Prestige {
            rows: vec![
                crate::city_prestige::Row::Grade(5),
                crate::city_prestige::Row::Block(0),
                crate::city_prestige::Row::Space,
                crate::city_prestige::Row::Grade(4),
                crate::city_prestige::Row::Block(1),
            ]
            .into_iter()
            .chain((2..30).map(crate::city_prestige::Row::Block))
            .collect(),
            values: [0.; 52],
            overlay_categories: Vec::new(),
        });
        state.ownership = Some(crate::city_ownership::Ownership {
            badges: [crate::ownership_badges::Badge {
                character: None,
                remaining: 0,
            }; 52],
            groups: vec![Vec::new(), (0..30).collect()],
        });
        let records = ["First", "Second"].map(|name| {
            let mut row = [0; 440];
            row[0xc] = 1;
            row[0x154..0x154 + name.len()].copy_from_slice(name.as_bytes());
            row
        });
        state.attach_players(player_info::State::new(&records).unwrap());
        state
    }

    #[test]
    fn navigator_city_lists_use_green_groups_board_labels_font3_and_scrollbar() {
        let layout = Layout::city([30., 30., 198., 270.], 16.);
        assert_eq!(layout.list.bounds, [30., 70., 182., 265.]);
        assert_eq!(layout.scrollbar, [182., 72., 198., 270.]);
        let pages = [Surface::solid(), Surface::solid()];
        let tab = font(4, 9);
        let mut strings = vec![Vec::new(); 2244];
        strings[43] = encoded("%d Star");
        strings[19] = encoded("No blocks owned");
        let mut palette = [0xffffffff; 18];
        palette[16] = 0xff00ff00;
        palette[17] = 0xff123456;
        let arrows = std::array::from_fn(|_| std::array::from_fn(|_| Surface::solid()));
        // Deliberately different variants: rendering must choose the correct
        // board data supplied by Assets, including its original mixed case.
        let short_labels = std::array::from_fn(|i| encoded(&format!("Short{i}")));
        let ownership_labels = std::array::from_fn(|i| encoded(&format!("Full name {i}")));
        let city = City {
            list: List {
                pages: &pages,
                tab: &tab,
                strings: &strings,
                palette: &palette,
                arrows: &arrows,
            },
            short_labels: &short_labels,
            ownership_labels: &ownership_labels,
        };
        let color_count = |surface: &Surface, color: u32| {
            let rgb = [
                (color >> 16 & 255) as f32 / 255.,
                (color >> 8 & 255) as f32 / 255.,
                (color & 255) as f32 / 255.,
            ];
            surface.vertices.iter().filter(|v| v[3..6] == rgb).count()
        };
        let mut state = city_locator();
        for prestige in [true, false] {
            state.prestige_mode = prestige;
            state.ownership_mode = !prestige;
            state.ownership_scroll = 0;
            let output = city.paint(&state, &layout).unwrap();
            assert_eq!(
                color_count(&output[0], palette[17]),
                12,
                "two green heading bars"
            );
            assert!(color_count(&output[0], 0xff183088) > 0, "blue track");
            assert!(color_count(&output[0], 0xff1067cf) > 0, "blue handle");
            assert_eq!(output[1].vertices.len(), 6);
            assert_eq!(output[2].vertices.len(), 6);
            let labels = &output[3];
            assert!(labels.vertices.iter().all(|v| v[0] < layout.scrollbar[0]));
            let expected = if prestige {
                encoded("5 StarShort0").len()
            } else {
                encoded("FirstNo blocks owned").len()
            };
            let second_row_end = labels.vertices.iter().take_while(|v| v[1] < 94.).count();
            assert_eq!(second_row_end, expected * 6);
            assert_eq!(
                labels.vertices[0][1] - labels.vertices[2][1],
                9.,
                "full font3 height"
            );
            let (glyphs, _) = labels.vertices.as_chunks::<6>();
            let block_row = if prestige { 1. } else { 3. };
            let count = glyphs
                .iter()
                .filter(|q| q[2][1] == 72.5 + block_row * 11.)
                .count();
            assert_eq!(
                count,
                if prestige {
                    short_labels[0].len()
                } else {
                    ownership_labels[0].len()
                }
            );
            assert_eq!(
                city_hit(&state, &layout, [190., 80.]),
                Some(Action::Scroll(-1))
            );
            assert_eq!(
                city_hit(&state, &layout, [190., 260.]),
                Some(Action::Scroll(1))
            );
            assert_eq!(
                city_hit(&state, &layout, [190., 150.]),
                Some(Action::ScrollTrack)
            );
            let row = if prestige { 1 } else { 3 };
            assert_eq!(
                city_hit(&state, &layout, [40., 73. + row as f32 * 11.]),
                Some(Action::OwnershipBlock(0))
            );
            assert_eq!(
                city_hit(&state, &layout, [40., 73.]),
                None,
                "heading is not a block"
            );
            let count = city_rows(&state).len();
            state.ownership_scroll = layout.selection(1000., count);
            assert_eq!(state.ownership_scroll, count - layout.list.visible_rows());
            let output = city.paint(&state, &layout).unwrap();
            // Every last block remains reachable, including the final row.
            assert!(!output[3].vertices.is_empty());
            assert!(output[3].vertices.iter().any(|v| v[1] > 250.));
        }
        assert_eq!(
            layout.hit([190., 80.], 2),
            None,
            "short reports disable the slider"
        );
        let compact = Layout::city([30., 30., 198., 180.], 16.);
        assert!(
            compact.maximum(30) > layout.maximum(30),
            "capacity follows panel bounds"
        );
    }

    #[test]
    fn navigator_city_strategic_projection_preserves_scrolled_report_rows() {
        let mut owner = strategic::selector_boundary_tests::state_fixture(
            crate::compatibility::Mode::Modern,
            EnabledBufferProvenance::Unrepresented,
        );
        let input = Arc::make_mut(&mut owner.input);
        input.financial.records[0][0x154..0x159].copy_from_slice(b"Owner");
        for i in 0..30 {
            input.categories[i][4] = 1;
            input.categories[i][0x74] = 1;
            let grade = 5. - (i / 10) as f32;
            input.categories[i][0x84..0x88].copy_from_slice(&grade.to_le_bytes());
            input.financial.records[0][0x174 + i] = 1;
        }
        owner.tab = 0;
        owner.city_page = 1;
        owner.show(&mut [0; 600]).unwrap();
        owner.message(11, false, false, 0, &mut [0; 600]).unwrap();
        let mut state = crate::locator::refresh_ui_tests::fresh();
        state.install_strategic(owner).unwrap();
        for page in [1, 3] {
            let owner = state.strategic_mut().unwrap();
            owner.city_page = page;
            owner.message(11, false, false, 0, &mut [0; 600]).unwrap();
            state.publish_strategic().unwrap();
            let before = city_rows(&state);
            assert!(before.len() > 17);
            assert_eq!(
                before[0],
                if page == 1 {
                    CityRow::Grade(5)
                } else {
                    CityRow::Player(0)
                }
            );
            state.ownership_scroll = 7;
            state.publish_strategic().unwrap();
            assert_eq!(
                state.ownership_scroll, 7,
                "per-frame projection retains city scrolling"
            );
            assert_eq!(city_rows(&state), before);
            assert_eq!(before.last(), Some(&CityRow::Block(29)));
        }
    }

    #[test]
    fn navigator_rights_legend_has_only_eight_captions_and_four_fixed_rules() {
        let pages = [Surface::solid(), Surface::solid()];
        let tab = font(4, 9);
        let description_tab = font(3, 6);
        let mut strings = vec![Vec::new(); 1802];
        for id in [13, 15, 17, 1801] {
            strings[id] = encoded("Heading");
        }
        for id in [14, 16, 18, 1441] {
            strings[id] = encoded("Description");
        }
        let legend = Rights {
            pages: &pages,
            tab: &tab,
            descriptions: &pages,
            description_tab: &description_tab,
            strings: &strings,
            icon: &Surface::solid(),
            color: 0xffffffff,
        };
        let output = legend.paint([30., 30., 198., 290.]).unwrap();
        let rules = &output[8]; // four badge background/icon pairs
        assert_eq!(rules.vertices.len(), 24);
        let (quads, remainder) = rules.vertices.as_chunks::<6>();
        assert!(remainder.is_empty());
        for (quad, y) in quads.iter().zip([85., 135., 195., 255.]) {
            assert_eq!(quad.iter().map(|v| v[0]).reduce(f32::min), Some(70.));
            assert_eq!(quad.iter().map(|v| v[0]).reduce(f32::max), Some(198.));
            assert_eq!(quad.iter().map(|v| v[1]).reduce(f32::min), Some(y));
            assert_eq!(quad.iter().map(|v| v[1]).reduce(f32::max), Some(y + 1.));
            assert_eq!(quad[0][3..6], [229. / 255., 95. / 255., 73. / 255.]);
        }
        assert_eq!(output[9].vertices.len(), 4 * "Heading".len() * 6);
        assert_eq!(output[11].vertices.len(), 4 * "Description".len() * 6);
        assert!(
            output[9].vertices.iter().all(|v| v[0] >= 71.5),
            "no player-name line above legend"
        );
        let (glyphs, _) = output[9].vertices.as_chunks::<6>();
        for (i, glyph) in glyphs.iter().step_by("Heading".len()).enumerate() {
            assert_eq!(glyph[2][1], 72.5 + [0., 50., 110., 170.][i]);
            assert_eq!(glyph[0][1] - glyph[2][1], 9.);
        }
    }

    fn finance_owner() -> strategic::State {
        let mut bytes = vec![0; 444];
        bytes[0x10] = 1;
        bytes[0x158..0x15d].copy_from_slice(b"Human");
        let mut account = PlayerAccount::from_runtime(&bytes).unwrap();
        for multiplier in [10., 1.] {
            if multiplier == 1. {
                account.rotate_ledger();
            }
            for category in 0..21 {
                account
                    .record(-(category as f32 + 1.) * multiplier, category)
                    .unwrap();
            }
            for category in 22..37 {
                account
                    .record((category as f32 + 1.) * multiplier, category)
                    .unwrap();
            }
        }
        let manager = Manager {
            bytes: [0; 0x28],
            players: vec![Player {
                reference: 0x1234,
                bytes,
                auxiliaries: vec![],
            }],
        };
        let mut owner = strategic::selector_boundary_tests::state_fixture(
            crate::compatibility::Mode::Modern,
            EnabledBufferProvenance::Unrepresented,
        );
        Arc::make_mut(&mut owner.input).financial =
            player_info::Snapshot::from_runtime(&manager, &[account], 0x1234).unwrap();
        owner.tab = 1;
        owner.show(&mut [0; 600]).unwrap();
        owner.message(11, false, false, 0, &mut [0; 600]).unwrap();
        owner
    }

    #[test]
    fn navigator_finance_uses_both_ledger_banks_and_original_cost_order() {
        let mut owner = finance_owner();
        for (page, multiplier) in [(1, 1.), (0, 10.)] {
            owner.finance_page = page;
            owner.message(11, false, false, 0, &mut [0; 600]).unwrap();
            let costs = &owner.finance_rows[owner
                .finance_rows
                .iter()
                .position(|r| r.label == 0x43)
                .unwrap()..];
            let pairs = costs
                .iter()
                .filter(|r| r.style == 0)
                .map(|r| (r.label, f32::from_bits(r.amount)))
                .collect::<Vec<_>>();
            // 412810 emits Building Work and Buy out before Chance Cards and
            // Spoilage. Expenses are negated for display; category21 is excluded.
            assert_eq!(
                pairs,
                [
                    (0x4a, 18.),
                    (0x49, 15.),
                    (0x50, 26.),
                    (0x28, 1.),
                    (0x5bc, 32.),
                    (0x5b, 4.),
                    (0x5c, 19.),
                    (0x5bd, 20.),
                    (0x40, 231.)
                ]
                .map(|(label, amount)| (label, amount * multiplier))
            );
        }
        let record = &mut Arc::make_mut(&mut owner.input).financial.records[0];
        // The selected current bank is1; omit the two optional zero categories.
        for offset in [0x5c, 0x60] {
            record[offset - 4 + 152..offset + 152].fill(0);
        }
        owner.finance_page = 1;
        owner.message(11, false, false, 0, &mut [0; 600]).unwrap();
        let costs = &owner.finance_rows[owner
            .finance_rows
            .iter()
            .position(|r| r.label == 0x43)
            .unwrap()..];
        assert!(!costs.iter().any(|r| r.label == 0x5c || r.label == 0x5bd));
        assert_eq!(f32::from_bits(costs.last().unwrap().amount), 192.);
    }

    fn font(width: i32, height: i32) -> Vec<u8> {
        let mut tab = vec![0; 4 + 128 * 44];
        tab[..4].copy_from_slice(&128u32.to_le_bytes());
        for i in 0..128 {
            let at = 4 + i * 44;
            tab[at + 32..at + 36].copy_from_slice(&width.to_le_bytes());
            tab[at + 36..at + 40].copy_from_slice(&height.to_le_bytes());
        }
        tab
    }

    #[test]
    fn navigator_title_uses_centered_full_size_title_widget() {
        let bounds = title_bounds([30., 5., 190., 15.], [30., 15., 190., 29.]);
        assert_eq!(bounds, [38., 5., 198., 29.]);
        for text in [b"CITY INFORMATION".as_slice(), b"PLAYER INFO.".as_slice()] {
            let text = text.iter().map(|b| b - 32).collect::<Vec<_>>();
            let pages = paint_title(
                &[Surface::solid(), Surface::solid()],
                &font(8, 12),
                &text,
                bounds,
                0xffffffff,
            )
            .unwrap();
            let vertices = &pages[0].vertices;
            assert_eq!(vertices.len(), text.len() * 6);
            assert_eq!(
                vertices[0][0],
                38. + ((160. - text.len() as f32 * 7.) * 0.5 + 0.5).trunc() - 0.5
            );
            assert_eq!(vertices[0][1] - vertices[2][1], 12.);
            assert_eq!(vertices[2][1], 10.5);
        }
    }

    #[test]
    fn navigator_finance_projection_paints_and_retains_scrollable_cost_rows() {
        let owner = finance_owner();
        let layout = Layout::new([30., 40., 190., 320.], 16.);
        assert_eq!(layout.list.bounds, [30., 100., 174., 315.]);
        assert_eq!(layout.scrollbar, [174., 100., 190., 315.]);
        let mut locator = crate::locator::refresh_ui_tests::fresh();
        locator.player.set_visible_rows(layout.list.visible_rows());
        locator.install_strategic(owner).unwrap();
        let count = locator.player.rows.len();
        let maximum = layout.maximum(count);
        assert!(maximum > 0);
        assert_eq!(layout.hit([182., 108.], count), Some(Action::Scroll(-1)));
        assert_eq!(layout.hit([182., 308.], count), Some(Action::Scroll(1)));
        assert_eq!(layout.hit([182., 200.], count), Some(Action::ScrollTrack));
        assert_eq!(layout.hit([40., 200.], count), None);
        locator.player.scroll = layout.selection(1000., count);
        assert_eq!(locator.player.scroll, maximum);
        locator.publish_strategic().unwrap();
        assert_eq!(
            locator.player.scroll, maximum,
            "per-frame projection must retain scrolling"
        );
        assert_eq!(locator.player.rows.last().unwrap().label, 0x40);
        let mut strings = vec![vec![1]; 2244];
        strings[2243] = vec![b'$' - 32];
        let mut palette = [0xffffffff; 18];
        palette[17] = 0xff123456;
        let arrows = std::array::from_fn(|_| std::array::from_fn(|_| Surface::solid()));
        let pages = [Surface::solid(), Surface::solid()];
        let tab = font(4, 9);
        let finance = List {
            pages: &pages,
            tab: &tab,
            strings: &strings,
            palette: &palette,
            arrows: &arrows,
        };
        let output = finance.paint(&locator.player, &layout, None, None).unwrap();
        // Production blue track/handle, two arrow widgets and full-height font3.
        assert!(
            output[0]
                .vertices
                .iter()
                .any(|v| v[3..6] == [24. / 255., 48. / 255., 136. / 255.])
        );
        assert!(
            output[0]
                .vertices
                .iter()
                .any(|v| v[3..6] == [16. / 255., 103. / 255., 207. / 255.])
        );
        assert_eq!(output[1].vertices.len(), 6);
        assert_eq!(output[2].vertices.len(), 6);
        assert!(!output[3].vertices.is_empty());
        assert!(
            output[3]
                .vertices
                .iter()
                .all(|v| v[0] < layout.scrollbar[0])
        );
        // The last (Total Costs) pair remains visible when dragged to the end.
        let last = &output[3].vertices[output[3].vertices.len() - 6..];
        assert!(
            last.iter()
                .all(|v| v[1] >= layout.list.line(layout.list.visible_rows() - 1, false)[1] - 1.)
        );
        locator.strategic_mut().unwrap().finance_page = 0;
        locator.publish_strategic().unwrap();
        assert_eq!(
            locator.player.scroll, 0,
            "period change resets the report viewport"
        );
        assert_eq!(layout.hit([182., 108.], 1), None);
    }
}
