#![cfg(feature = "capture")]
use openmt2001::render::{
    COLOR_FORMAT, DEPTH_FORMAT, Renderer, SecondaryTexture, SurfaceMode, TexturedDraw,
};

#[test]
#[ignore = "requires a GPU adapter"]
fn clouds_modulate_two_layers_add_and_disable_culling() {
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
    let points = [
        [-1., -1.],
        [0., -1.],
        [0., 1.],
        [-1., -1.],
        [0., 1.],
        [-1., 1.],
        [0., -1.],
        [1., 1.],
        [1., -1.],
        [0., -1.],
        [0., 1.],
        [1., 1.],
    ];
    let clouds = points.map(|[x, y]| [x, y, 0.75, 0.5, 0.25, 1., 0., 0.]);
    let background = [
        [-1., -1.],
        [1., -1.],
        [1., 1.],
        [-1., -1.],
        [1., 1.],
        [-1., 1.],
    ]
    .map(|[x, y]| [x, y, 0.25, 0.1, 0.2, 0.3, 0., 0.]);
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
            vertices: &clouds,
            specular: None,
            diffuse_alpha: Some(&[64; 12]),
            mode: SurfaceMode::Clouds,
            width: 1,
            height: 1,
            rgba: &[128, 64, 255, 128],
            mipmaps: false,
            secondary: Some(SecondaryTexture {
                uv: &[[0., 0.]; 12],
                diffuse_alpha: 127,
                width: 1,
                height: 1,
                rgba: &[64, 255, 128, 64],
                mipmaps: false,
            }),
        },
    ];
    // Both triangle windings must render, even behind the foreground depth.
    // RGB is additive, not alpha-weighted; alpha multiplies both texture alphas
    // and the supplied per-vertex alpha, then adds to the zero background alpha.
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
    for x in [24, 72] {
        let offset = 8 * 512 + x * 4;
        let actual = &bytes[offset..offset + 4];
        for (got, want) in actual.iter().zip([42u8, 67, 205, 8]) {
            assert!(got.abs_diff(want) <= 1, "cloud pixel{x}: {actual:?}");
        }
    }
}
