//! Original mode19 smoke uses billboard-repeat texture zero (004b6104),
//! radius scale1 (004b6114), and the ordered 004b6147 point stream.
use crate::result_presentation::{fireworks::Point, particles_gpu};
use openmt2001::{
    chimney_smoke::Smoke, flyby::CameraSample, render::SurfaceMode, render_scene::OwnedDraw,
};

pub fn empty(assets: &crate::dust_scene::Assets) -> OwnedDraw {
    let mut draw = assets.empty();
    draw.mode = SurfaceMode::ChimneySmoke;
    draw.mipmaps = true;
    draw
}

pub fn fill(
    draw: &mut OwnedDraw,
    state: &Smoke,
    detail: u8,
    camera: CameraSample,
) -> Result<(), String> {
    let points = state
        .points(detail)
        .into_iter()
        .map(|point| Point {
            position: point.position,
            size: point.size,
            argb: point.argb,
        })
        .collect::<Vec<_>>();
    particles_gpu::fill_with_scale(draw, &points, camera, 1.)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chimney_smoke_live_instance_list_and_billboard_fields() {
        let mut state = Smoke::new(0, &[[10., 20., 30.], [40., 50., 60.]]);
        state.tick(10);
        let mut draw = particles_gpu::empty_draw();
        draw.mode = SurfaceMode::ChimneySmoke;
        let camera = CameraSample {
            eye: [0.; 3],
            target: [0., 0., -1.],
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        fill(&mut draw, &state, 2, camera).unwrap();
        let points = state.points(2);
        assert_eq!(draw.vertices.len(), 18);
        assert_eq!(draw.diffuse_alpha.as_ref().unwrap(), &[10; 18]);
        for (vertices, point) in draw.vertices.as_chunks::<6>().0.iter().zip(points) {
            assert_eq!(
                vertices[0][..3],
                [
                    point.position[0] - point.size,
                    point.position[1] - point.size,
                    -point.position[2]
                ]
            );
            assert!(vertices.iter().all(|v| v[3..6] == [0.; 3]));
        }
        fill(&mut draw, &state, 0, camera).unwrap();
        assert_eq!(
            draw.vertices.len(),
            6,
            "low detail retains only fixed emitter"
        );
    }
}
