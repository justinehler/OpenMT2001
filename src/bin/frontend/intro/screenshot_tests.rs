use super::*;
use crate::present::{Blit, letterbox};
use winit::keyboard::{KeyCode, PhysicalKey};

fn key(code: KeyCode, down: bool, repeat: bool) -> PhysicalSample {
    PhysicalSample::Key {
        key: PhysicalKey::Code(code),
        down,
        repeat,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = openmt2001::fixtures::scratch("screenshot").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn names_start_at_zero_and_skip_existing_files() {
    let dir = scratch("names");
    let (first, _) = create_next(&dir).unwrap();
    assert_eq!(first, dir.join("tycoon0000.tga"));
    fs::write(dir.join("tycoon0002.tga"), b"kept").unwrap();
    let (second, _) = create_next(&dir).unwrap();
    assert_eq!(second, dir.join("tycoon0001.tga"));
    // 0002 exists, so the next free name is 0003 and 0002 is untouched.
    let (third, _) = create_next(&dir).unwrap();
    assert_eq!(third, dir.join("tycoon0003.tga"));
    assert_eq!(fs::read(dir.join("tycoon0002.tga")).unwrap(), b"kept");
    // The counter restarts at 0000 for every press: a freed low name is reused.
    fs::remove_file(dir.join("tycoon0000.tga")).unwrap();
    let (reused, _) = create_next(&dir).unwrap();
    assert_eq!(reused, dir.join("tycoon0000.tga"));
}

#[test]
fn tiny_frame_has_original_header_and_bottom_up_bgr_pixels() {
    // 2x2 top-down RGBA: top row red, green; bottom row blue, white.
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, //
        0, 0, 255, 255, 255, 255, 255, 128,
    ];
    let bytes = tga(2, 2, &rgba).unwrap();
    assert_eq!(
        bytes[..18],
        [0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 32, 0]
    );
    assert_eq!(
        bytes[18..],
        [
            255, 0, 0, 0, 255, 255, 255, 0, // bottom row first: blue, white
            0, 0, 255, 0, 0, 255, 0, 0, // then the top row: red, green
        ]
    );
}

#[test]
fn header_stores_low_and_high_dimension_bytes() {
    let bytes = tga(1280, 3, &vec![0; 1280 * 3 * 4]).unwrap();
    assert_eq!(bytes[12..16], [0x00, 0x05, 0x03, 0x00]);
    assert_eq!(bytes.len(), 18 + 1280 * 3 * 4);
    assert!(tga(2, 2, &[0; 4]).is_err());
}

#[test]
fn save_writes_the_encoded_frame_to_the_next_name() {
    let dir = scratch("save");
    fs::write(dir.join("tycoon0000.tga"), b"existing").unwrap();
    let path = save(&dir, 1, 1, &[1, 2, 3, 4]).unwrap();
    assert_eq!(path, dir.join("tycoon0001.tga"));
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 22);
    assert_eq!(bytes[18..], [3, 2, 1, 0]);
}

#[test]
fn one_capture_per_f5_press_with_left_alt_held() {
    let mut input = Input::default();
    input.sample(key(KeyCode::AltLeft, true, false), true);
    input.sample(key(KeyCode::AltLeft, true, true), true);
    input.sample(key(KeyCode::F5, true, false), true);
    assert!(input.take_pending());
    // Holding both keys: OS repeats and a duplicate down queue nothing.
    input.sample(key(KeyCode::F5, true, true), true);
    input.sample(key(KeyCode::AltLeft, true, true), true);
    input.sample(key(KeyCode::F5, true, false), true);
    assert!(!input.take_pending());
    // Release and press F5 again while Alt stays held: a second capture.
    input.sample(key(KeyCode::F5, false, false), true);
    input.sample(key(KeyCode::F5, true, false), true);
    assert!(input.take_pending());
    assert!(!input.take_pending());
}

#[test]
fn right_alt_plain_f5_and_inactive_city_do_not_capture() {
    let mut input = Input::default();
    input.sample(key(KeyCode::F5, true, false), true);
    input.sample(key(KeyCode::F5, false, false), true);
    assert!(!input.take_pending());
    input.sample(key(KeyCode::AltRight, true, false), true);
    input.sample(key(KeyCode::F5, true, false), true);
    input.sample(key(KeyCode::F5, false, false), true);
    assert!(!input.take_pending());
    input.sample(key(KeyCode::AltRight, false, false), true);
    input.sample(key(KeyCode::AltLeft, true, false), true);
    input.sample(key(KeyCode::F5, true, false), false);
    assert!(!input.take_pending());
}

#[test]
fn focus_changes_neither_press_nor_hold_keys() {
    let mut input = Input::default();
    // A focus-gain reconciliation press of F5 is not a new press.
    input.sample(key(KeyCode::AltLeft, true, false), true);
    input.sample(
        PhysicalSample::SyntheticKey {
            key: PhysicalKey::Code(KeyCode::F5),
            down: true,
        },
        true,
    );
    assert!(!input.take_pending());
    // Losing focus releases left Alt, so a later F5 alone does nothing.
    input.sample(PhysicalSample::Unfocus, true);
    input.sample(key(KeyCode::F5, true, false), true);
    assert!(!input.take_pending());
}

/// A 1280x960 frame (the city frame texture's internal size) with one colour
/// per quadrant: red, green / blue, white.
fn quadrant_frame(device: &wgpu::Device, queue: &wgpu::Queue, blit: &Blit) -> wgpu::BindGroup {
    let (width, height) = (1280u32, 960u32);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(match (x < width / 2, y < height / 2) {
                (true, true) => &[255, 0, 0, 255],
                (false, true) => &[0, 255, 0, 255],
                (true, false) => &[0, 0, 255, 255],
                (false, false) => &[255, 255, 255, 255],
            });
        }
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        texture.size(),
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    blit.binding(device, &texture.create_view(&Default::default()), &sampler)
}

/// The BGR0 pixel at column `x`, row `y` counted from the top, of a
/// bottom-up TGA.
fn tga_pixel(bytes: &[u8], width: usize, height: usize, x: usize, y: usize) -> [u8; 4] {
    let at = 18 + ((height - 1 - y) * width + x) * 4;
    bytes[at..at + 4].try_into().unwrap()
}

#[test]
fn letterbox_fills_4_3_windows_and_centres_wider_ones() {
    assert_eq!(letterbox(1024, 768), [0., 0., 1024., 768.]);
    assert_eq!(letterbox(1280, 720), [160., 0., 960., 720.]);
    assert_eq!(letterbox(800, 800), [0., 100., 800., 600.]);
}

#[test]
fn capture_is_window_sized_whatever_the_frame_texture_size() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) {
        descriptor.backends = wgpu::Backends::DX12;
    }
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    // Surfaces are usually BGRA; the capture target is RGBA either way.
    let blit = Blit::new(&device, wgpu::TextureFormat::Bgra8Unorm);
    let frame = quadrant_frame(&device, &queue, &blit);
    let dir = scratch("window_size");

    // A 1024x768 window: the 1280x960 frame is scaled down to fill it.
    let shot = blit.capture(&device, &queue, &frame, 1024, 768).unwrap();
    let path = save(&dir, shot.width, shot.height, &shot.rgba).unwrap();
    assert_eq!(path, dir.join("tycoon0000.tga"));
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 3_145_746);
    assert_eq!(
        bytes[..18],
        [
            0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x04, 0x00, 0x03, 32, 0
        ]
    );
    assert_eq!(tga_pixel(&bytes, 1024, 768, 256, 192), [0, 0, 255, 0]);
    assert_eq!(tga_pixel(&bytes, 1024, 768, 768, 192), [0, 255, 0, 0]);
    assert_eq!(tga_pixel(&bytes, 1024, 768, 256, 576), [255, 0, 0, 0]);
    assert_eq!(tga_pixel(&bytes, 1024, 768, 768, 576), [255, 255, 255, 0]);
    assert_eq!(tga_pixel(&bytes, 1024, 768, 0, 0), [0, 0, 255, 0]);

    // A 1280x720 window shows the frame pillarboxed; so does the screenshot.
    let shot = blit.capture(&device, &queue, &frame, 1280, 720).unwrap();
    let path = save(&dir, shot.width, shot.height, &shot.rgba).unwrap();
    assert_eq!(path, dir.join("tycoon0001.tga"));
    let bytes = fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 18 + 1280 * 720 * 4);
    assert_eq!(bytes[12..16], [0x00, 0x05, 0xd0, 0x02]);
    assert_eq!(tga_pixel(&bytes, 1280, 720, 80, 360), [0, 0, 0, 0]);
    assert_eq!(tga_pixel(&bytes, 1280, 720, 1200, 360), [0, 0, 0, 0]);
    assert_eq!(tga_pixel(&bytes, 1280, 720, 400, 180), [0, 0, 255, 0]);

    assert!(blit.capture(&device, &queue, &frame, 0, 768).is_err());
}
