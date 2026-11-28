//! Left Alt + F5 saves the presented city frame as `tycoonNNNN.tga`.
//!
//! The central input callback tests left Alt (scan 0x38) held and F5 (scan
//! 0x3f) newly down, and calls the frame capture 0x458af0 with argument 0
//! (0x4ba88d..0x4ba8b2). The check has no UI-hit, panel or network gate, so it
//! fires with panels open and the pointer over UI. Right Alt is a different
//! scan code and never counts.
//!
//! 0x458af0 repaints the frame, reads the render target back and writes it to
//! the first `tycoonNNNN.tga` (counting from 0000 each press) that cannot be
//! opened for reading. The path is relative, so the file lands in the
//! process's working directory. The file is an 18-byte header (image type 2,
//! no colour map, zero origin, the frame's width and height, 32 bits per
//! pixel, descriptor 0) followed by the pixels. The renderer readback that
//! 0x458af0 calls through vtable slot +0x170 stores the bottom row first,
//! which descriptor 0's bottom-left origin shows upright. 0x458af0 swaps the
//! first and third bytes, so each pixel is blue, green, red and a zero fourth
//! byte.
use super::process_frontend::PhysicalSample;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use winit::keyboard::{KeyCode, PhysicalKey};

/// 0x458af0 tries numbers below 10000.
const NAME_LIMIT: u32 = 10_000;

/// Physical history of left Alt and F5, and a capture waiting for the next
/// presented city frame.
#[derive(Default)]
pub(super) struct Input {
    left_alt: bool,
    f5: bool,
    pending: bool,
}
impl Input {
    /// Whether this sample is a new physical F5 press (not an OS repeat, not a
    /// focus-reconciliation press, not a second down without an up).
    pub(super) fn is_f5_press(sample: PhysicalSample) -> bool {
        matches!(
            sample,
            PhysicalSample::Key {
                key: PhysicalKey::Code(KeyCode::F5),
                down: true,
                repeat: false,
            }
        )
    }
    /// Track held keys and queue one capture per F5 press while left Alt is
    /// held. `active` is the city gate, read only for F5 presses.
    pub(super) fn sample(&mut self, sample: PhysicalSample, active: bool) {
        let (key, down, physical) = match sample {
            PhysicalSample::Key { key, down, repeat } => (key, down, !repeat),
            // Focus reconciliation updates held state but is not a press.
            PhysicalSample::SyntheticKey { key, down } => (key, down, false),
            PhysicalSample::Unfocus => {
                self.left_alt = false;
                self.f5 = false;
                return;
            }
            _ => return,
        };
        match key {
            PhysicalKey::Code(KeyCode::AltLeft) => self.left_alt = down,
            PhysicalKey::Code(KeyCode::F5) => {
                let edge = down && physical && !self.f5;
                self.f5 = down;
                if edge && self.left_alt && active {
                    self.pending = true;
                }
            }
            _ => {}
        }
    }
    /// Take the queued capture, if any.
    pub(super) fn take_pending(&mut self) -> bool {
        std::mem::take(&mut self.pending)
    }
}

/// Read `texture` back and save it in the working directory. Failures are
/// logged; the game keeps running.
pub(super) fn capture(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) {
    let saved = read_back(device, queue, texture).and_then(|rgba| {
        save(Path::new(""), texture.width(), texture.height(), &rgba).map_err(|e| e.to_string())
    });
    match saved {
        Ok(path) => {
            let shown = std::env::current_dir().map_or(path.clone(), |dir| dir.join(&path));
            eprintln!("Screenshot saved to {}", shown.display());
        }
        Err(error) => eprintln!("Screenshot failed: {error}"),
    }
}

/// Encode a top-down RGBA frame as 0x458af0's uncompressed 32-bit TGA.
pub(super) fn tga(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let (Ok(w), Ok(h)) = (u16::try_from(width), u16::try_from(height)) else {
        return Err(format!("frame {width}x{height} is too large for a TGA"));
    };
    let row = width as usize * 4;
    if rgba.len() != row * height as usize {
        return Err(format!(
            "frame {width}x{height} has {} bytes, expected {}",
            rgba.len(),
            row * height as usize
        ));
    }
    let mut bytes = Vec::with_capacity(18 + rgba.len());
    bytes.extend_from_slice(&[0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&w.to_le_bytes());
    bytes.extend_from_slice(&h.to_le_bytes());
    bytes.extend_from_slice(&[32, 0]);
    if row != 0 {
        for line in rgba.chunks_exact(row).rev() {
            for [red, green, blue, _] in line.as_chunks::<4>().0 {
                bytes.extend_from_slice(&[*blue, *green, *red, 0]);
            }
        }
    }
    Ok(bytes)
}

/// Create the first `tycoonNNNN.tga` in `dir` that does not exist yet.
pub(super) fn create_next(dir: &Path) -> io::Result<(PathBuf, fs::File)> {
    for number in 0..NAME_LIMIT {
        let path = dir.join(format!("tycoon{number:04}.tga"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::other(
        "tycoon0000.tga through tycoon9999.tga all exist",
    ))
}

/// Encode a top-down RGBA frame and write it to the next free name in `dir`.
pub(super) fn save(dir: &Path, width: u32, height: u32, rgba: &[u8]) -> io::Result<PathBuf> {
    let bytes = tga(width, height, rgba).map_err(io::Error::other)?;
    let (path, mut file) = create_next(dir)?;
    file.write_all(&bytes)?;
    Ok(path)
}

/// Copy an RGBA8 render target to the CPU as tightly packed top-down rows.
fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<Vec<u8>, String> {
    if texture.format() != wgpu::TextureFormat::Rgba8Unorm {
        return Err(format!("unexpected frame format {:?}", texture.format()));
    }
    let (width, height) = (texture.width(), texture.height());
    let row = width * 4;
    let padded =
        row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Screenshot readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| e.to_string())?;
    receiver
        .recv()
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| e.to_string())?;
    let mut rgba = Vec::with_capacity(row as usize * height as usize);
    for line in mapped.chunks_exact(padded as usize) {
        rgba.extend_from_slice(&line[..row as usize]);
    }
    Ok(rgba)
}

#[cfg(test)]
#[path = "screenshot_tests.rs"]
mod tests;
