//! Navigator title (40d080) and finance list/choice (413500) presentation.
use super::{Action, report_text};
use crate::{
    player_info,
    ui::{self, Surface},
};

pub fn title_bounds(first: [f32; 4], second: [f32; 4]) -> [f32; 4] {
    [first[0] + 8., first[1], first[2] + 8., second[3]]
}

pub fn paint_title(
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

pub struct Layout {
    pub list: report_text::Layout,
    pub scrollbar: [f32; 4],
}
impl Layout {
    pub fn new(main: [f32; 4], slider_width: f32) -> Self {
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
    pub fn maximum(&self, count: usize) -> usize {
        count.saturating_sub(self.list.visible_rows())
    }
    pub fn hit(&self, point: [f32; 2], count: usize) -> Option<Action> {
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
    pub fn selection(&self, y: f32, count: usize) -> usize {
        ui::slider_selection(
            ui::slider_track(self.scrollbar, true),
            y,
            self.maximum(count) as i32,
            true,
        ) as usize
    }
}

pub struct Finance<'a> {
    pub pages: &'a [Surface],
    pub tab: &'a [u8],
    pub strings: &'a [Vec<u8>],
    pub palette: &'a [u32; 18],
    pub arrows: &'a [[Surface; 4]; 2],
}
impl Finance<'_> {
    pub fn paint(
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
        let maximum = layout.maximum(player.rows.len());
        let slider = ui::Slider {
            bounds: layout.scrollbar,
            vertical: true,
            value: player.scroll as f32,
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
        out.extend(pages);
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

    #[test]
    #[ignore = "requires supplied Navigator mesh, fonts, language and executable"]
    fn navigator_assets_connect_title_font_and_finance_viewport() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let archive = read("ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&read("ARCHIVE_DIR"), archive.len() as u64)
                .unwrap();
        let language = crate::intro::camera_live::ui_text_resources::PackedLanguage::load(
            &openmt2001::fixtures::path("LANGUAGE").join("tycoon"),
        )
        .unwrap();
        let assets = super::super::Assets::load(
            &index,
            &archive,
            &openmt2001::fixtures::path("INSTALLER"),
            &language.rows(),
            &read("EXE"),
        )
        .unwrap();
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
        let finance = Finance {
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
