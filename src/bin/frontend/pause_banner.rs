//! Original 4bad40 pause banner, drawn by the render frame (4b4dae) after the
//! interface groups and before the cursor: a translucent red band across the
//! bottom of the 640x480 interface with a centred font5 caption.
use crate::ui::Surface;
use openmt2001::{
    font::{Font, Layout},
    render::{PreparedScene, Renderer},
};

/// Caption for player pause reason 3 (F1/P, 43e5f0).
const PLAYER_PAUSE_TEXT: u32 = 0x46d;
/// Caption for pause reason 2, which replaces the player caption.
const REASON_TWO_TEXT: u32 = 0x6df;
/// 4bad40 band: left, top, right, bottom in interface units.
const BAND: [f32; 4] = [0., 450., 640., 480.];
/// 4bad40 passes ARGB b4ff0000 to the uniform interface quad.
const BAND_COLOR: u32 = 0xb4ff0000;
/// Caption top edge, 455.0 in the 4e7e60 call.
const CAPTION_TOP: f32 = 455.;
/// 4bad40 selects font5 (455420) for the caption.
const FONT: u32 = 5;

/// Text identifier 4bad40 draws for the four pause reason counters, if any.
/// Reason 2 takes precedence over the player's reason 3; reasons 0 and 1
/// (Options, scripts) never show the banner.
pub(crate) fn caption(reasons: [u8; 4]) -> Option<u32> {
    if reasons[2] != 0 {
        Some(REASON_TWO_TEXT)
    } else if reasons[3] != 0 {
        Some(PLAYER_PAUSE_TEXT)
    } else {
        None
    }
}

pub struct Assets {
    pages: Vec<Surface>,
    tab: Vec<u8>,
    player: Vec<u8>,
    reason_two: Vec<u8>,
}
impl Assets {
    pub(crate) fn load_shared(
        shared: &crate::intro::camera_live::ui_text_resources::UiTextResources,
    ) -> Result<Self, String> {
        let rows = shared.fonts().language.rows();
        let row = |id: u32| {
            rows.get(id as usize)
                .map(|s| s.to_vec())
                .ok_or_else(|| format!("pause banner text {id} missing"))
        };
        let tab = shared.table(FONT)?.to_vec();
        Font::parse(&tab)?;
        Ok(Self {
            pages: shared.surfaces(FONT)?,
            tab,
            player: row(PLAYER_PAUSE_TEXT)?,
            reason_two: row(REASON_TWO_TEXT)?,
        })
    }
    fn text(&self, id: u32) -> Result<&[u8], String> {
        match id {
            PLAYER_PAUSE_TEXT => Ok(&self.player),
            REASON_TWO_TEXT => Ok(&self.reason_two),
            _ => Err(format!("pause banner has no text {id}")),
        }
    }
    pub fn prepare(&self, renderer: &Renderer, id: u32) -> Result<PreparedScene, String> {
        let text = self.text(id)?;
        let width = crate::tooltip::measure_with_gap(&self.tab, text, -1)?;
        let mut band = Surface::solid();
        band.rect(BAND, [BAND_COLOR; 4]);
        let mut pages = self.pages.clone();
        // 4bad40 passes the integer 320 - width/2 as the caption's left edge.
        let left = 320 - width / 2;
        for q in Font::parse(&self.tab)?.layout(
            text,
            Layout {
                origin: [left as f32, CAPTION_TOP],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            pages[q.page].quad(q.positions.map(|p| [p[0], p[1]]), q.uv, [0xffffffff; 4]);
        }
        let mut draws = vec![band.draw()];
        draws.extend(pages.iter().map(Surface::draw));
        renderer.prepare(
            &draws,
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
}

/// Retained banner for one inspection; prepared again only when the caption changes.
#[derive(Default)]
pub struct View {
    shown: Option<(u32, PreparedScene)>,
}
impl View {
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        caption: Option<u32>,
    ) -> Result<Option<&PreparedScene>, String> {
        match caption {
            None => self.shown = None,
            Some(id) if self.shown.as_ref().is_some_and(|(shown, _)| *shown == id) => {}
            Some(id) => self.shown = Some((id, assets.prepare(renderer, id)?)),
        }
        Ok(self.shown.as_ref().map(|(_, scene)| scene))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caption_follows_4bad40_reason_priority() {
        assert_eq!(caption([0; 4]), None);
        assert_eq!(caption([1, 0, 0, 0]), None, "Options pause has no banner");
        assert_eq!(caption([0, 1, 0, 0]), None, "script pause has no banner");
        assert_eq!(caption([0, 0, 0, 1]), Some(PLAYER_PAUSE_TEXT));
        assert_eq!(caption([1, 1, 0, 1]), Some(PLAYER_PAUSE_TEXT));
        assert_eq!(caption([0, 0, 1, 0]), Some(REASON_TWO_TEXT));
        assert_eq!(caption([0, 0, 1, 1]), Some(REASON_TWO_TEXT));
    }
}
