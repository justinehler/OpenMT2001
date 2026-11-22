//! Shared offscreen frontend integration-test capture.
use openmt2001::{
    render::{PreparedScene, Renderer},
    render_scene::Scene,
};
use std::fs;
pub(crate) fn capture(
    renderer: &Renderer,
    panel: Option<&PreparedScene>,
    scene: &Scene,
    path: &str,
) {
    capture_impl(
        renderer,
        &[],
        &panel.into_iter().collect::<Vec<_>>(),
        Some(scene),
        None,
        path,
    );
}
pub(crate) fn capture_prepared(
    renderer: &Renderer,
    scene: &Scene,
    prepared: &PreparedScene,
    path: &str,
) {
    capture_impl(renderer, &[], &[], Some(scene), Some(prepared), path);
}
pub(crate) fn capture_standalone(renderer: &Renderer, prepared: &PreparedScene, path: &str) {
    capture_impl(renderer, &[], &[], None, Some(prepared), path);
}
pub(crate) fn capture_overlay(
    renderer: &Renderer,
    prepared: &PreparedScene,
    panel: &PreparedScene,
    path: &str,
) {
    capture_impl(renderer, &[], &[panel], None, Some(prepared), path);
}
pub(crate) fn capture_scene_overlays(
    renderer: &Renderer,
    scene: &Scene,
    panels: &[&PreparedScene],
    path: &str,
) {
    capture_impl(renderer, &[], panels, Some(scene), None, path);
}
pub(crate) fn capture_scene_world_overlays(
    renderer: &Renderer,
    scene: &Scene,
    world: &[&PreparedScene],
    panels: &[&PreparedScene],
    path: &str,
) {
    capture_impl(renderer, world, panels, Some(scene), None, path);
}
pub(crate) fn capture_prepared_world_overlays(
    renderer: &Renderer,
    prepared: &PreparedScene,
    world: &[&PreparedScene],
    panels: &[&PreparedScene],
    path: &str,
) {
    capture_impl(renderer, world, panels, None, Some(prepared), path);
}
fn capture_impl(
    renderer: &Renderer,
    world: &[&PreparedScene],
    panels: &[&PreparedScene],
    scene: Option<&Scene>,
    prepared: Option<&PreparedScene>,
    path: &str,
) {
    let size = wgpu::Extent3d {
        width: 1280,
        height: 960,
        depth_or_array_layers: 1,
    };
    let texture = |format, usage| {
        renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let color = texture(
        openmt2001::render::COLOR_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = texture(
        openmt2001::render::DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let cv = color.create_view(&Default::default());
    let dv = depth.create_view(&Default::default());
    if let Some(prepared) = prepared {
        renderer.draw_prepared(&cv, &dv, prepared);
    } else {
        let scene = scene.expect("unprepared capture requires scene geometry");
        renderer
            .draw_textured(
                &cv,
                &dv,
                &scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
                scene.camera,
                scene.fog,
            )
            .unwrap();
    }
    for geometry in world {
        renderer.draw_world_overlay(&cv, &dv, geometry);
    }
    for panel in panels {
        renderer.draw_overlay(&cv, &dv, panel);
    }
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 1280 * 960 * 4,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &color,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1280 * 4),
                rows_per_image: Some(960),
            },
        },
        size,
    );
    renderer.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    renderer
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    rx.recv().unwrap().unwrap();
    let mut rgba = buffer.slice(..).get_mapped_range().unwrap().to_vec();
    for p in rgba.chunks_exact_mut(4) {
        p[3] = 255;
    }
    let mut png = png::Encoder::new(
        std::io::BufWriter::new(fs::File::create(path).unwrap()),
        1280,
        960,
    );
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header().unwrap().write_image_data(&rgba).unwrap();
}

/// Capture an actual retained construction record without advancing its state.
pub(crate) fn construction_stage(
    session: &crate::fresh_city::Session,
    build: &[u8],
    original: openmt2001::render_scene::OriginalAssets<'_>,
    slot: usize,
    path: &str,
) {
    let record = &session.world.buildings[slot];
    let value = |at| f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
    let center = [value(0x58), 0., value(0x60)];
    // Fixed diagnostic camera across stages; this does not activate a game camera mode.
    let camera = openmt2001::flyby::CameraSample {
        vertical_fov: 45.,
        eye: [center[0], 120., center[2] + 100.],
        target: center,
        up: [0., 1., 0.],
    };
    let sun = fs::read(std::env::var("OPENMT2001_SUNCOLS").unwrap()).unwrap();
    let scene = session
        .scene(build, original, &sun, Some(camera), false)
        .unwrap();
    assert!(
        scene
            .inspection_buildings
            .iter()
            .any(|b| b.runtime_slot == Some(slot))
    );
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    capture(&renderer, None, &scene, path);
}
