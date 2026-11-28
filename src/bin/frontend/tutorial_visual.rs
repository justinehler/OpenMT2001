//! Tutorial pointer overlay for Interface.AddArrow, and the gate on the 3D
//! Mr. Monopoly tutor figure that the original keeps hidden.
use openmt2001::{
    render::{PreparedScene, Renderer, SurfaceMode},
    render_scene::OwnedDraw,
};

/// 640x480 overlay units to clip space.
const VIEW: [f32; 16] = [
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
];

/// The 3D Mr. Monopoly tutor figure (`gamedata2\tutor\mr_tutor_body.tm`
/// and `mr_tutor_anim.tm`).
///
/// City start (init 0x4c5f50, called at 0x4b998e) places it at (250, 450)
/// in 640x480 units with scale 50 and clears its animation state, but it
/// never writes the visibility byte 0x934409, which therefore stays 0. The
/// per-frame draw 0x4c6310 (called from the city render at 0x4b4986) returns
/// at once while that byte is 0. Its only writer is the animation-finished
/// callback 0x4c6260, reached from sequences that 0x4c65c0 starts. The only
/// ways into 0x4c65c0 are 0x4c6700 (appear), 0x4c6720 (leave) and 0x4c6740
/// (idle tick), and nothing in v1.4 calls any of them, so the figure never
/// becomes visible in any tutorial or game mode. Because nothing can draw
/// it, its meshes and texture pages are not loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TutorFigure {
    /// 0x934409.
    visible: bool,
}

impl TutorFigure {
    /// The state 0x4c5f50 leaves at city start.
    pub const fn at_city_start() -> Self {
        Self { visible: false }
    }

    /// Whether 0x4c6310 draws the figure this frame.
    pub const fn visible(self) -> bool {
        self.visible
    }
}

/// The tutorial pointer and the tutor figure drawn over the city view.
pub struct Overlay {
    tutor: TutorFigure,
}

impl Overlay {
    /// The overlay as a city starts. Nothing in v1.4 changes the tutor's
    /// visibility afterwards, so one state serves every city.
    pub const fn at_city_start() -> Self {
        Self {
            tutor: TutorFigure::at_city_start(),
        }
    }

    /// Draws for one frame with a pointer from the prompt at `origin`,
    /// moved by Interface.PositionPrompt's `offset`, to the absolute AddArrow
    /// `target`. The pointer placement (0x429fbd -> 0x4c5f30) only sets the
    /// pointer origin; the tutor figure stays where 0x4c5f50 put it and is
    /// drawn only while it is visible.
    pub fn draws(
        &self,
        origin: [f32; 2],
        target: [f32; 2],
        offset: [f32; 2],
    ) -> Result<Vec<OwnedDraw>, String> {
        if self.tutor.visible() {
            return Err("the tutor figure is visible, but v1.4 never shows it, \
                        so its meshes are not loaded"
                .into());
        }
        let origin = [origin[0] + offset[0], origin[1] + offset[1]];
        Ok(arrow_draw(origin, target).into_iter().collect())
    }

    /// [`Overlay::draws`] prepared for the renderer, or `None` when there is
    /// nothing to draw.
    pub fn prepare(
        &self,
        renderer: &Renderer,
        origin: [f32; 2],
        target: [f32; 2],
        offset: [f32; 2],
    ) -> Result<Option<PreparedScene>, String> {
        let draws = self.draws(origin, target, offset)?;
        if draws.is_empty() {
            return Ok(None);
        }
        renderer
            .prepare(
                &draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>(),
                VIEW,
                None,
            )
            .map(Some)
    }
}

/// 429eef..42a0be submits one tapered quad from the prompt's elliptical
/// border to the indicated screen point. The first two vertices are half-
/// alpha red; the last two are opaque white. Native 4e2b70 normalizes the
/// centre-to-target vector under FPCW 0x037f.
fn arrow_draw(origin: [f32; 2], target: [f32; 2]) -> Option<OwnedDraw> {
    let dx = target[0] - origin[0];
    let dy = target[1] - origin[1];
    let length = dx.hypot(dy);
    if length < 1. {
        return None;
    }
    let dir = [dx / length, dy / length];
    let start = [origin[0] + dir[0] * 160., origin[1] + dir[1] * 80.];
    let points = [
        [start[0] + dir[1] * 5., start[1] - dir[0] * 5.],
        [start[0] - dir[1] * 5., start[1] + dir[0] * 5.],
        [target[0] - 1., target[1]],
        [target[0] + 1., target[1]],
    ];
    let mut draw = OwnedDraw::texture_page(SurfaceMode::Overlay, 1, 1, vec![255u8; 4].into());
    draw.specular = None;
    draw.diffuse_alpha = Some(Vec::new());
    draw.mipmaps = false;
    for index in [0, 1, 2, 0, 2, 3] {
        let point = points[index];
        let red = index < 2;
        draw.vertices.push([
            point[0],
            point[1],
            0.1,
            1.,
            if red { 0. } else { 1. },
            if red { 0. } else { 1. },
            0.,
            0.,
        ]);
        draw.diffuse_alpha
            .as_mut()
            .unwrap()
            .push(if red { 127 } else { 255 });
    }
    Some(draw)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_arrow_quad() {
        // 429eef..42a0be under FPCW 0x037f, original v1.4 mc.exe. The
        // independent Unicorn oracle is work/frontend/native_tutorial_arrow.py.
        for (target, expected) in [
            (
                [534., 461.],
                [
                    [434.894, 293.9932],
                    [427.7101, 300.9496],
                    [533., 461.],
                    [535., 461.],
                ],
            ),
            (
                [100., 100.],
                [
                    [182.3298, 201.2683],
                    [187.6985, 192.8316],
                    [99., 100.],
                    [101., 100.],
                ],
            ),
            (
                [540., 120.],
                [
                    [458.069, 197.3024],
                    [462.8575, 206.0813],
                    [539., 120.],
                    [541., 120.],
                ],
            ),
            (
                [100., 420.],
                [
                    [199.333, 294.5288],
                    [193.0007, 286.7892],
                    [99., 420.],
                    [101., 420.],
                ],
            ),
        ] {
            let draw = arrow_draw([320., 240.], target).unwrap();
            for (i, expected) in expected.iter().enumerate() {
                let vertex = draw.vertices[[0, 1, 2, 5][i]];
                assert!((vertex[0] - expected[0]).abs() < 0.003);
                assert!((vertex[1] - expected[1]).abs() < 0.003);
                assert_eq!(vertex[2].to_bits(), 0.1f32.to_bits());
                assert_eq!(vertex[3..6], if i < 2 { [1., 0., 0.] } else { [1.; 3] });
            }
            assert_eq!(
                draw.diffuse_alpha.as_deref(),
                Some(&[127, 127, 255, 127, 255, 255][..])
            );
        }
    }
    #[test]
    fn pointer_step_draws_the_pointer_line_and_no_tutor_figure() {
        let overlay = Overlay::at_city_start();
        assert!(!overlay.tutor.visible());
        // Tutorial 1's clock pointer (AddArrow 507,18), with the prompt
        // moved by PositionPrompt.
        let draws = overlay
            .draws([320., 240.], [507., 18.], [12., -8.])
            .unwrap();
        // Only the red pointer line: one quad, no tutor pages or meshes.
        assert_eq!(draws.len(), 1);
        let expected = arrow_draw([332., 232.], [507., 18.]).unwrap();
        assert_eq!(draws[0].vertices, expected.vertices);
        assert_eq!(draws[0].vertices.len(), 6);
        assert_eq!(draws[0].vertices[0][3..6], [1., 0., 0.]);
        assert_eq!(draws[0].vertices[2][0..2], [506., 18.]);
    }
}
