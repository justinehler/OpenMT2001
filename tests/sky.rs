#![cfg(feature = "capture")]
use openmt2001::render::{COLOR_FORMAT, DEPTH_FORMAT, Renderer, SurfaceMode, TexturedDraw};

#[test]
#[ignore = "requires a GPU adapter"]
fn sky_uses_separate_camera_tests_depth_and_does_not_write_it() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    // Match capture's workaround for the current Windows driver's enumeration crash.
    if cfg!(target_os = "windows") && std::env::var_os("WGPU_BACKEND").is_none() {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let extent = wgpu::Extent3d {
        width: 96,
        height: 16,
        depth_or_array_layers: 1,
    };
    let texture = |format, usage| {
        renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let target = texture(
        COLOR_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = texture(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let quad = |left: f32, right: f32, depth: f32, color: [f32; 3]| {
        [
            [left, -1.],
            [right, -1.],
            [right, 1.],
            [left, -1.],
            [right, 1.],
            [left, 1.],
        ]
        .map(|[x, y]| [x, y, depth, color[0], color[1], color[2], 0., 0.])
    };
    let red = quad(-1., -1. / 3., 0.25, [1., 0., 0.]);
    let green = quad(0., 1. / 3., 0.75, [0., 1., 0.]);
    let blue = quad(-1., -1. / 3., 0.9, [0., 0., 1.]);
    let identity = glam::Mat4::IDENTITY.to_cols_array();
    let mut sky_camera = identity;
    sky_camera[12] = 2. / 3.;
    let draw = |vertices, mode, view_projection| TexturedDraw {
        diffuse_alpha: None,
        vertices,
        mode,
        view_projection,
        specular: None,
        width: 1,
        height: 1,
        rgba: &[255; 4],
        mipmaps: false,
        secondary: None,
    };
    // Sky projects into the middle third. Green is behind the first sky draw
    // but must replace it. A later sky is behind green and must fail its depth
    // test while replacing red on the unoccupied half of the middle third.
    renderer
        .draw_textured(
            &target.create_view(&Default::default()),
            &depth.create_view(&Default::default()),
            &[
                draw(&red, SurfaceMode::Sky, Some(sky_camera)),
                draw(&green, SurfaceMode::StaticOpaque, None),
                draw(&blue, SurfaceMode::Sky, Some(sky_camera)),
            ],
            identity,
            None,
        )
        .unwrap();
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 512 * 16,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512),
                rows_per_image: Some(16),
            },
        },
        extent,
    );
    renderer.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| sender.send(r).unwrap());
    renderer
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    receiver.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().unwrap();
    for (x, expected) in [(40, [0, 0, 255, 255]), (56, [0, 255, 0, 255])] {
        let offset = 8 * 512 + x * 4;
        assert_eq!(&bytes[offset..offset + 4], &expected, "pixel{x}");
    }
    for x in [8, 80] {
        let offset = 8 * 512 + x * 4;
        assert!(
            bytes[offset..offset + 3].iter().all(|&c| c < 20),
            "sky camera did not translate coverage"
        );
    }
}
