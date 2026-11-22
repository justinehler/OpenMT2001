#![cfg(feature = "capture")]
use openmt2001::render::{COLOR_FORMAT, DEPTH_FORMAT, Renderer, SurfaceMode, TexturedDraw};

#[test]
#[ignore = "requires a GPU adapter"]
fn stars_are_single_pixel_points_with_original_alpha_blend() {
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
    let background = [
        [-1., -1.],
        [1., -1.],
        [1., 1.],
        [-1., -1.],
        [1., 1.],
        [-1., 1.],
    ]
    .map(|[x, y]| [x, y, 0.1, 0., 0., 0., 0., 0.]);
    let stars = [
        [-0.5, 0., 0.9, 1., 1., 1., 0., 0.],
        [0.5, 0., 0.9, 1., 1., 1., 0., 0.],
    ];
    let draws = [
        TexturedDraw {
            view_projection: None,
            vertices: &background,
            specular: None,
            diffuse_alpha: None,
            mode: SurfaceMode::StaticOpaque,
            width: 1,
            height: 1,
            rgba: &[255, 255, 255, 0],
            mipmaps: false,
            secondary: None,
        },
        TexturedDraw {
            view_projection: None,
            vertices: &stars,
            specular: None,
            diffuse_alpha: Some(&[64, 128]),
            mode: SurfaceMode::Stars,
            width: 1,
            height: 1,
            rgba: &[255; 4],
            mipmaps: false,
            secondary: None,
        },
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
    let mut pixels = Vec::new();
    for y in 0..16 {
        for x in 0..96 {
            let at = y * 512 + x * 4;
            let pixel = &bytes[at..at + 4];
            if pixel != [0, 0, 0, 0] {
                pixels.push(pixel.to_vec());
            }
        }
    }
    pixels.sort();
    assert_eq!(pixels.len(), 2, "each star must cover exactly one pixel");
    for (actual, expected) in pixels.iter().zip([[64u8, 64, 64, 16], [128, 128, 128, 64]]) {
        for (got, want) in actual.iter().zip(expected) {
            assert!(got.abs_diff(want) <= 1, "point color/alpha {actual:?}");
        }
    }
}
