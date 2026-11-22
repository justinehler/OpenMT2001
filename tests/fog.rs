#![cfg(feature = "capture")]
use openmt2001::render::{COLOR_FORMAT, DEPTH_FORMAT, EyeFog, Renderer, SurfaceMode, TexturedDraw};

#[test]
#[ignore = "requires a GPU adapter"]
fn table_fog_uses_eye_depth_after_specular_and_preserves_alpha() {
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
    let mut vertices = Vec::new();
    // Six equally sized screen panels, but different eye depths. All have the
    // same depth-buffer z=0.5; confusing z with w cannot pass this comparison.
    for (i, (near, far)) in [(1., 1.), (2., 2.), (4., 4.), (6., 6.), (8., 8.), (2., 6.)]
        .into_iter()
        .enumerate()
    {
        let left = -1. + i as f32 / 3.;
        let right = left + 1. / 3.;
        for [x, y] in [
            [left, -1.],
            [right, -1.],
            [right, 1.],
            [left, -1.],
            [right, 1.],
            [left, 1.],
        ] {
            let z = if x == left { near } else { far };
            vertices.push([x * z, y * z, z, 1., 0., 0., 0., 0.]);
        }
    }
    let specular = vec![[0., 0.25, 0.]; vertices.len()];
    let draw = TexturedDraw {
        diffuse_alpha: None,
        view_projection: None,
        vertices: &vertices,
        specular: Some(&specular),
        mode: SurfaceMode::StaticOpaque,
        width: 1,
        height: 1,
        rgba: &[255, 255, 255, 128],
        mipmaps: false,
        secondary: None,
    };
    let projection = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0.5, 1., 0., 0., 0., 0.,
    ];
    renderer
        .draw_textured(
            &target.create_view(&Default::default()),
            &depth.create_view(&Default::default()),
            &[draw],
            projection,
            Some(EyeFog {
                color: 0xff0000ff,
                start: 2.,
                end: 6.,
            }),
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
    for (i, expected) in [
        [255u8, 64, 0, 128],
        [255, 64, 0, 128],
        [128, 32, 128, 128],
        [0, 0, 255, 128],
        [0, 0, 255, 128],
        // Center sample is 8.5/16 across the last panel: harmonic eye depth
        // 1 / ((1-t)/2 + t/6), not the arithmetic interpolation of 2 and 6.
        [185, 46, 70, 128],
    ]
    .into_iter()
    .enumerate()
    {
        let offset = 8 * 512 + (i * 16 + 8) * 4;
        let actual = &bytes[offset..offset + 4];
        for (got, want) in actual.iter().zip(expected) {
            assert!(
                got.abs_diff(want) <= 1,
                "panel {i}: {actual:?} != {expected:?}"
            );
        }
    }
}
