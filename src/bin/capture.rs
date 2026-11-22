//! Capture reconstructed original buildings or the supplied static map.
//! Camera pose remains diagnostic; building/scenery lighting uses recovered palettes.
use glam::Vec3;
use openmt2001::{
    render::{COLOR_FORMAT, DEPTH_FORMAT, Renderer},
    render_scene::{self, OriginalAssets, Request, Source},
};
use std::{env, fs, io::BufWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = env::args().collect();
    // Explicit diagnostic pose also permits inspecting the sky from street
    // height; the overview's distant framing can clip the original100-unit orbit.
    let camera_override = if let Some(at) = args.iter().position(|s| s == "--camera") {
        if args.get(1).is_none_or(|s| s != "save") || at < 9 || at + 7 > args.len() {
            return Err("--camera requires six coordinates after save-mode inputs".into());
        }
        let coordinates = args[at + 1..at + 7]
            .iter()
            .map(|s| s.parse::<f32>())
            .collect::<Result<Vec<_>, _>>()?;
        if !coordinates.iter().all(|v| v.is_finite()) || coordinates[..3] == coordinates[3..] {
            return Err("camera requires finite distinct eye and target positions".into());
        }
        args.drain(at..at + 7);
        Some((
            Vec3::from_slice(&coordinates[..3]),
            Vec3::from_slice(&coordinates[3..]),
        ))
    } else {
        None
    };
    // Diagnostic phase selection leaves the supplied save and its clock intact.
    let phase_override = if args.len() == 11 && args[1] == "save" && args[9] == "--phase" {
        let phase = args[10].parse::<u32>()?;
        if phase >= 24 {
            return Err("diagnostic lighting phase must be in 0..24".into());
        }
        args.truncate(9);
        Some(phase)
    } else {
        None
    };
    let save_mode = args.get(1).is_some_and(|a| a == "save");
    let scene_mode = save_mode || args.get(1).is_some_and(|a| a == "map");
    if !(scene_mode && (args.len() == 8 || save_mode && args.len() == 9)
        || !scene_mode
            && (args.len() == 5 || args.len() == 10 && args[5] == "--original-materials"))
    {
        return Err("usage: capture <BuildFile.dat> <model> <height> <output.png> [--original-materials <mc.exe> <archive.DIR> <archive.bin> <variation>]\n       capture <map|save> <BuildFile.dat> <map.lua|game.sav> <mc.exe> <archive.DIR> <archive.bin> <output.png> [suncols.tga [--phase 0..23] [--camera x y z target-x target-y target-z] (save mode)]".into());
    }
    let data = fs::read(&args[if scene_mode { 2 } else { 1 }])?;
    let original = if scene_mode {
        Some((&args[4], &args[5], &args[6]))
    } else if args.len() == 10 {
        Some((&args[6], &args[7], &args[8]))
    } else {
        None
    };
    let executable = original.map(|(exe, _, _)| fs::read(exe)).transpose()?;
    let archive = original.map(|(_, _, bin)| fs::read(bin)).transpose()?;
    let directory = original.map(|(_, dir, _)| fs::read(dir)).transpose()?;
    let source_bytes = if scene_mode {
        Some(fs::read(&args[3])?)
    } else {
        None
    };
    let source = if save_mode {
        Source::Save(source_bytes.as_deref().unwrap())
    } else if scene_mode {
        Source::Map(std::str::from_utf8(source_bytes.as_deref().unwrap())?)
    } else {
        Source::Model {
            index: args[2].parse()?,
            height: args[3].parse()?,
            variation: if original.is_some() {
                args[9].parse()?
            } else {
                0
            },
        }
    };
    let suncols = if save_mode && args.len() == 9 {
        Some(fs::read(&args[8])?)
    } else {
        None
    };
    let width = 1280u32;
    let height_pixels = 960u32;
    let scene = render_scene::assemble(Request {
        build_file: &data,
        source,
        original: executable.as_deref().map(|executable| OriginalAssets {
            executable,
            directory: directory.as_deref().unwrap(),
            archive: archive.as_deref().unwrap(),
        }),
        suncols: suncols.as_deref(),
        phase: phase_override,
        camera: camera_override.map(|(eye, target)| openmt2001::flyby::CameraSample {
            eye: eye.to_array(),
            target: target.to_array(),
            up: [0., 1., 0.],
            vertical_fov: 45.,
        }),
        width,
        height: height_pixels,
        animated: false,
    })?;
    for message in &scene.diagnostics {
        println!("{message}");
    }
    let output = &args[if scene_mode { 7 } else { 4 }];
    let building_count = scene.building_count;
    let vertex_count = scene.vertex_count;
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    // Prefer the native Windows backend. Enumerating every driver crashes on
    // the current Intel/RDP host; explicit DX12 has been tested successfully.
    if cfg!(target_os = "windows") && env::var_os("WGPU_BACKEND").is_none() {
        descriptor.backends = wgpu::Backends::DX12;
    }
    eprintln!("Initializing graphics: {:?}", descriptor.backends);
    let renderer = pollster::block_on(Renderer::new(descriptor))?;
    eprintln!("Rendering with {}", renderer.adapter_name);
    let extent = wgpu::Extent3d {
        width,
        height: height_pixels,
        depth_or_array_layers: 1,
    };
    let make_texture = |format, usage| {
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
    let color = make_texture(
        COLOR_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = make_texture(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let target_view = color.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let draws: Vec<_> = scene.draws.iter().map(|d| d.borrowed()).collect();
    renderer.draw_textured(&target_view, &depth_view, &draws, scene.camera, scene.fog)?;
    let row_bytes = (width * 4).div_ceil(256) * 256;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture readback"),
        size: u64::from(row_bytes) * u64::from(height_pixels),
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
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(height_pixels),
            },
        },
        extent,
    );
    renderer.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    renderer.device.poll(wgpu::PollType::wait_indefinitely())?;
    receiver.recv()??;
    let mapped = buffer.slice(..).get_mapped_range()?;
    let mut rgba = Vec::with_capacity((width * height_pixels * 4) as usize);
    for row in mapped.chunks_exact(row_bytes as usize) {
        rgba.extend_from_slice(&row[..width as usize * 4]);
    }
    // A screen capture displays RGB; the legacy backbuffer alpha is not
    // display transparency (multiplicative water legitimately writes zero).
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    let mut png = png::Encoder::new(
        BufWriter::new(fs::File::create(output)?),
        width,
        height_pixels,
    );
    png.set_color(png::ColorType::Rgba);
    png.set_depth(png::BitDepth::Eight);
    png.write_header()?.write_image_data(&rgba)?;
    println!(
        "{}: {building_count} building(s), {vertex_count} triangle vertices -> {output}",
        renderer.adapter_name
    );
    Ok(())
}
