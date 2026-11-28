//! Frontend pointer-group modal fading (43cb80/43cbf0, update 43bce0).
//! Only bound panel models fade. Custom paint callbacks and children retain
//! their own colours; a dialog's translucent body can cover those afterwards.
use crate::ui::Surface;
use openmt2001::render::TexturedDraw;

pub struct Fade {
    alpha: u8,
    modal: bool,
    step: i8,
}

impl Default for Fade {
    fn default() -> Self {
        Self {
            alpha: 255,
            modal: false,
            step: 0,
        }
    }
}

impl Fade {
    pub fn alpha(&self) -> u8 {
        self.alpha
    }

    /// One delivered frontend UI sample, independent of OS input repeats.
    /// The native stop test is <=128, not a clamp: 255-26*5 settles at125.
    pub fn advance(&mut self, modal: bool) -> bool {
        if self.modal != modal {
            self.modal = modal;
            self.step = if modal {
                -5
            } else if self.alpha != 255 {
                5
            } else {
                0
            };
        }
        if self.step == 0 {
            return false;
        }
        self.alpha = self.alpha.wrapping_add(self.step as u8);
        if (self.step < 0 && self.alpha <= 128) || (self.step > 0 && self.alpha == 255) {
            self.step = 0;
        }
        true
    }
}

/// Borrow textures/geometry and override only the model descriptor's alpha.
/// Keeping the source immutable prevents successive modal draws compounding it.
pub struct Backdrop<'a> {
    surfaces: &'a [Surface],
    alpha: Vec<Vec<u8>>,
}

impl<'a> Backdrop<'a> {
    pub fn new(surfaces: &'a [Surface], alpha: u8) -> Self {
        Self {
            surfaces,
            alpha: surfaces
                .iter()
                .map(|s| vec![alpha; s.vertices.len()])
                .collect(),
        }
    }

    pub fn draws(&self) -> Vec<TexturedDraw<'_>> {
        self.surfaces
            .iter()
            .zip(&self.alpha)
            .map(|(surface, alpha)| {
                let mut draw = surface.draw();
                draw.diffuse_alpha = Some(alpha);
                draw
            })
            .collect()
    }
}

pub fn set_alpha(surfaces: &mut [Surface], alpha: u8) {
    for surface in surfaces {
        surface.alpha.fill(alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_frame_draw_alpha_fades_and_restores_without_changing_art() {
        let mut frame = Surface::solid();
        frame.rect([0., 0., 640., 480.], [0xffffffff; 4]);
        let surfaces = [frame];
        let mut fade = Fade::default();
        let assert_draw = |alpha| {
            let backdrop = Backdrop::new(&surfaces, alpha);
            let draws = backdrop.draws();
            assert_eq!(draws[0].diffuse_alpha.unwrap(), [alpha; 6]);
            assert_eq!(draws[0].vertices, surfaces[0].vertices);
            assert_eq!(draws[0].rgba, surfaces[0].pixels);
        };
        assert_draw(fade.alpha());
        assert!(fade.advance(true));
        assert_eq!(fade.alpha(), 250);
        for _ in 1..26 {
            assert!(fade.advance(true));
        }
        assert_eq!(fade.alpha(), 125);
        assert!(!fade.advance(true));
        assert_draw(fade.alpha());
        for _ in 0..26 {
            assert!(fade.advance(false));
        }
        assert_eq!(fade.alpha(), 255);
        assert!(!fade.advance(false));
        assert_draw(fade.alpha());
        assert_eq!(surfaces[0].alpha, [255; 6]);
    }

    #[test]
    fn remaining_modal_keeps_frame_faded_and_early_close_reverses_fade() {
        let mut fade = Fade::default();
        for _ in 0..40 {
            fade.advance(true);
        }
        // Registration replaced by a warning still leaves one modal open.
        for _ in 0..40 {
            assert!(!fade.advance(true));
        }
        assert_eq!(fade.alpha(), 125);
        for _ in 0..26 {
            fade.advance(false);
        }
        fade.advance(true);
        fade.advance(true);
        assert_eq!(fade.alpha(), 245);
        fade.advance(false);
        fade.advance(false);
        assert_eq!(fade.alpha(), 255);
    }
}
