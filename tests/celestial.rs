#![cfg(feature = "capture")]
use openmt2001::render::{COLOR_FORMAT, DEPTH_FORMAT, Renderer, SurfaceMode, TexturedDraw};

#[test]
#[ignore = "requires a GPU adapter"]
fn celestial_quads_blend_without_depth_test_or_writes() {
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
    let quad = |left: f32, right: f32, z: f32, color: [f32; 3]| {
        [
            [right, -1.],
            [right, 1.],
            [left, 1.],
            [right, -1.],
            [left, 1.],
            [left, -1.],
        ]
        .map(|[x, y]| [x, y, z, color[0], color[1], color[2], 0., 0.])
    };
    let background = quad(-1., 0., 0.1, [0.; 3]);
    let celestial = quad(-1., 1., 0.9, [1.; 3]);
    let foreground = quad(0., 1., 0.95, [0., 1., 0.]);
    let draw = |vertices, mode, rgba| TexturedDraw {
        view_projection: None,
        vertices,
        specular: None,
        diffuse_alpha: None,
        mode,
        width: 1,
        height: 1,
        rgba,
        mipmaps: false,
        secondary: None,
    };
    let draws = [
        draw(&background, SurfaceMode::StaticOpaque, &[255, 255, 255, 0]),
        draw(&celestial, SurfaceMode::Celestial, &[255, 0, 0, 128]),
        draw(&foreground, SurfaceMode::StaticOpaque, &[255; 4]),
    ];
    renderer
        .draw_textured(
            &target.create_view(&Default::default()),
            &depth.create_view(&Default::default()),
            &draws,
            glam::Mat4::IDENTITY.to_cols_array(),
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
    // The left half has nearer opaque depth: the sprite must still blend.
    // The right half's later opaque draw is behind the sprite: it must pass,
    // proving the celestial layer did not write depth.
    for (x, expected) in [(24, [128u8, 0, 0, 64]), (72, [0, 255, 0, 255])] {
        let at = 8 * 512 + x * 4;
        let pixel = &bytes[at..at + 4];
        for (got, want) in pixel.iter().zip(expected) {
            assert!(
                got.abs_diff(want) <= 1,
                "celestial depth/blend pixel {x}: {pixel:?}"
            );
        }
    }
}
