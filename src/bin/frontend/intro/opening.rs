//! Original 4b27e0 overlay: black bars, centered supplied logos and localized
//! 4b2260 captions. Positions use the original 640x480 coordinate space.
use super::camera_live::{process_images::Images, ui_text_resources::UiTextResources};
use crate::ui::{self, Surface};
use openmt2001::{
    opening_sequence::{Card, Controller},
    render::{PreparedScene, Renderer},
};

pub(super) fn skip_key(key: winit::keyboard::NamedKey) -> bool {
    use winit::keyboard::NamedKey;
    matches!(key, NamedKey::Escape | NamedKey::Space | NamedKey::Enter)
}

pub(super) struct Assets {
    logos: Vec<Surface>,
    large: Vec<Surface>,
    large_tab: Vec<u8>,
    small: Vec<Surface>,
    small_tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
}

impl Assets {
    pub(super) fn load(images: &Images, text: &UiTextResources) -> Result<Self, String> {
        let textures = images.textures()?;
        let logos = [Card::Publisher, Card::Developer, Card::Title]
            .into_iter()
            .map(|card| {
                textures
                    .textures
                    .get(card.texture())
                    .map(Surface::new)
                    .ok_or_else(|| "opening logo missing from supplied images".to_owned())
            })
            .collect::<Result<_, _>>()?;
        let strings = text
            .fonts()
            .language
            .rows()
            .get(0x6fd..=0x703)
            .ok_or("opening language entries missing")?
            .iter()
            .map(|s| s.to_vec())
            .collect();
        Ok(Self {
            logos,
            large: text.surfaces(4)?,
            large_tab: text.table(4)?.to_vec(),
            small: text.surfaces(1)?,
            small_tab: text.table(1)?.to_vec(),
            strings,
        })
    }

    fn surfaces(&self, clock: &Controller) -> Result<Vec<Surface>, String> {
        let mut surfaces = vec![letterbox_bars(clock.height())];
        let Some((card, alpha)) = clock.card() else {
            return Ok(surfaces);
        };
        let index = match card {
            Card::Publisher => 0,
            Card::Developer => 1,
            Card::Title => 2,
        };
        let mut logo = self.logos[index].clone();
        let half = [logo.width as f32 * 0.5, logo.height as f32 * 0.5];
        logo.quad(
            [
                [320. - half[0], 240. - half[1]],
                [320. - half[0], 240. + half[1]],
                [320. + half[0], 240. + half[1]],
                [320. + half[0], 240. - half[1]],
            ],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [(u32::from(alpha) << 24) | 0xffffff; 4],
        );
        surfaces.push(logo);
        let mut large = self.large.clone();
        let y = if card == Card::Developer {
            210. - half[1]
        } else {
            240. + half[1]
        };
        centered(
            &mut large,
            &self.large_tab,
            &self.strings[index],
            y.trunc(),
            alpha,
        )?;
        surfaces.extend(large);
        if card == Card::Title {
            let mut small = self.small.clone();
            for (row, offset) in [270., 285., 300., 315.].into_iter().enumerate() {
                centered(
                    &mut small,
                    &self.small_tab,
                    &self.strings[3 + row],
                    (half[1] + offset).trunc(),
                    alpha,
                )?;
            }
            surfaces.extend(small);
        }
        Ok(surfaces)
    }

    pub(super) fn prepare(
        &self,
        renderer: &Renderer,
        clock: &Controller,
    ) -> Result<PreparedScene, String> {
        prepare_overlay(renderer, &self.surfaces(clock)?)
    }
}

/// 4b27e0's black bars for any active sequence clock: the top bar spans
/// y -5..height and the bottom bar 480-height..480 in 640x480 space. The
/// opening and the result cut scenes share this clock field (+0x14).
pub(super) fn letterbox_bars(height: u8) -> Surface {
    let mut bars = Surface::solid();
    let height = f32::from(height);
    bars.rect([0., -5., 640., height], [0xff000000; 4]);
    bars.rect([0., 480. - height, 640., 480.], [0xff000000; 4]);
    bars
}

/// Prepares sequence overlay surfaces in the original 640x480 space.
pub(super) fn prepare_overlay(
    renderer: &Renderer,
    surfaces: &[Surface],
) -> Result<PreparedScene, String> {
    renderer.prepare(
        &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
        [
            2. / 640.,
            0.,
            0.,
            0.,
            0.,
            -2. / 480.,
            0.,
            0.,
            0.,
            0.,
            1.,
            0.,
            -1.,
            1.,
            0.,
            1.,
        ],
        None,
    )
}

fn centered(
    pages: &mut [Surface],
    tab: &[u8],
    glyphs: &[u8],
    y: f32,
    alpha: u8,
) -> Result<(), String> {
    let width = ui::text_width(tab, glyphs, 1.)?;
    ui::text(
        pages,
        tab,
        glyphs,
        [(320 - width / 2) as f32, y, 640., 480.],
        false,
        (u32::from(alpha) << 24) | 0xffffff,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_keys_keep_the_existing_frontend_route() {
        use winit::keyboard::NamedKey;
        for key in [NamedKey::Escape, NamedKey::Space, NamedKey::Enter] {
            assert!(skip_key(key));
        }
        for key in [NamedKey::ArrowDown, NamedKey::Tab, NamedKey::Backspace] {
            assert!(!skip_key(key));
        }
    }

    #[test]
    fn overlay_draws_bars_then_centered_art_and_language_glyphs() {
        let logo = |width, height| Surface {
            width,
            height,
            ..Surface::solid()
        };
        let assets = Assets {
            logos: vec![logo(160, 80), logo(200, 100), logo(240, 120)],
            large: vec![Surface::solid(); 2],
            large_tab: ui::synthetic_text_font(8, 12),
            small: vec![Surface::solid(); 2],
            small_tab: ui::synthetic_text_font(6, 10),
            // Deliberately distinct encoded rows, rather than English captions.
            strings: (1..=7).map(|n| vec![n; n as usize]).collect(),
        };
        for (tick, card, top, y) in [
            (229, Card::Publisher, 200., 280.),
            (766, Card::Developer, 190., 160.),
            (1303, Card::Title, 180., 300.),
        ] {
            let mut clock = Controller::default();
            clock.advance_to(tick);
            assert_eq!(clock.card(), Some((card, 128)));
            let surfaces = assets.surfaces(&clock).unwrap();
            assert_eq!(surfaces[0].vertices.len(), 12);
            assert_eq!(surfaces[0].vertices[1][1], 85.);
            assert_eq!(surfaces[0].vertices[6][1], 395.);
            assert!(surfaces[0].vertices.iter().all(|v| v[3..6] == [0.; 3]));
            assert!(surfaces[0].alpha.iter().all(|&a| a == 255));
            assert_eq!(surfaces[1].vertices[0][1], top);
            assert_eq!(
                surfaces[1].vertices[0][0] + surfaces[1].vertices[2][0],
                640.
            );
            assert!(
                surfaces[1..]
                    .iter()
                    .flat_map(|s| &s.alpha)
                    .all(|&a| a == 128)
            );
            let vertices = surfaces[2..]
                .iter()
                .flat_map(|s| &s.vertices)
                .collect::<Vec<_>>();
            assert_eq!(
                vertices.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min),
                y - 0.5
            );
            let letters = match card {
                Card::Publisher => 1,
                Card::Developer => 2,
                Card::Title => 3 + 4 + 5 + 6 + 7,
            };
            assert_eq!(vertices.len(), letters * 6);
        }
    }
}
