//! Original default, right-drag, pan, enabled-menu-hover and city-hover (states 4-7)
//! images. The native host owns cursor presentation; no OS cursor API enters shared
//! assets or simulation.
use openmt2001::{
    archive::ArchiveIndex,
    texture::{TextureSet, loader_pixels},
};
use winit::{event_loop::ActiveEventLoop, window::CustomCursor};
struct Image {
    width: u16,
    height: u16,
    rgba: Vec<u8>,
    hotspot: [u16; 2],
}
/// Host cursor count: original states 0-7 (records 0-7 of the cursor collection).
pub const COUNT: usize = 8;
pub struct Images([Image; COUNT]);
impl Images {
    pub fn load(index: &ArchiveIndex, archive: &[u8]) -> Result<Self, String> {
        // Original004bd677 registers this collection in file order at6874f0.
        let entry = index
            .find("gamedata2/Cursors/textures.gtd")
            .ok_or("missing original cursors")?;
        let data = archive
            .get(entry.offset as usize..(entry.offset + entry.length) as usize)
            .ok_or("cursor collection outside archive")?;
        let set = TextureSet::parse(data).map_err(|e| e.to_string())?;
        // Host order keeps the first four city indices; states 4-7 follow in order.
        let images = [
            (0, 5, 1),
            (2, 8, 8),
            (3, 5, -1),
            (1, 8, 1),
            (4, 5, -1),
            (5, 5, -1),
            (6, 5, -1),
            (7, 5, -1),
        ]
        .into_iter()
        .map(|(i, hotspot_x, hotspot_y): (usize, u16, i8)| {
            // Original004391e0 sets dimensions from the registered image and
            // these hotspots;004390a0 subtracts them from pointer coordinates.
            let image = set.textures.get(i).ok_or("missing inspection cursor")?;
            let mut rgba = loader_pixels(image);
            // Hotspot Y -1 (states 3-7): the image starts one pixel below the pointer.
            // Native custom cursors require a nonnegative hotspot, so prepend one clear row.
            let padded = hotspot_y < 0;
            if padded {
                let mut rows = vec![0; image.width as usize * 4];
                rows.extend(rgba);
                rgba = rows;
            }
            Ok(Image {
                width: u16::try_from(image.width).map_err(|_| "cursor width")?,
                height: u16::try_from(image.height + u32::from(padded))
                    .map_err(|_| "cursor height")?,
                rgba,
                hotspot: [hotspot_x, hotspot_y.max(0).unsigned_abs().into()],
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
        Ok(Self(images.try_into().map_err(|_| "cursor count")?))
    }
}

/// Size of the original's virtual UI space. With `HardwareCursor = false` the original
/// hides the OS pointer (0x438fd0) and 0x4390a0 draws the cursor quad in this space, so
/// the 2D path stretches the image and its hotspot by the client size over 640x480.
/// The X and Y factors are independent: the input layer scales the pointer by
/// 640/client width and 480/client height separately.
const UI_SPACE: [u32; 2] = [640, 480];

/// Per-axis factor the original's software cursor is stretched by in a window of
/// `size` physical pixels. Hardware cursors (pending #66) stay at texel size.
fn ui_scale(size: [u32; 2], hardware_cursor: bool) -> [f64; 2] {
    if hardware_cursor || size[0] == 0 || size[1] == 0 {
        return [1.0, 1.0];
    }
    [0, 1].map(|axis| f64::from(size[axis]) / f64::from(UI_SPACE[axis]))
}

impl Image {
    /// The image as the original's 2D path shows it at `scale`: bilinear samples at
    /// texel centres with clamped edges, interpolating straight RGBA like the
    /// fixed-function sampler, and the hotspot scaled with it.
    fn scaled(&self, scale: [f64; 2]) -> Self {
        if scale == [1.0, 1.0] {
            return Self {
                width: self.width,
                height: self.height,
                rgba: self.rgba.clone(),
                hotspot: self.hotspot,
            };
        }
        let source = [u32::from(self.width), u32::from(self.height)];
        let size = [0, 1].map(|axis| {
            ((f64::from(source[axis]) * scale[axis]).round() as u32).clamp(1, u32::from(u16::MAX))
        });
        // One source row or column per destination pixel: the two neighbours and the
        // weight of the second.
        let taps = |axis: usize| -> Vec<(usize, usize, f64)> {
            let last = source[axis] as usize - 1;
            (0..size[axis])
                .map(|i| {
                    let at = ((f64::from(i) + 0.5) / scale[axis] - 0.5).max(0.0);
                    let low = (at.floor() as usize).min(last);
                    let high = (low + 1).min(last);
                    (low, high, at - low as f64)
                })
                .collect()
        };
        let (columns, rows) = (taps(0), taps(1));
        let stride = source[0] as usize * 4;
        let texel =
            |x: usize, y: usize, channel: usize| f64::from(self.rgba[y * stride + x * 4 + channel]);
        let mut rgba = Vec::with_capacity(size[0] as usize * size[1] as usize * 4);
        for &(top, bottom, down) in &rows {
            for &(left, right, across) in &columns {
                for channel in 0..4 {
                    let upper = texel(left, top, channel) * (1.0 - across)
                        + texel(right, top, channel) * across;
                    let lower = texel(left, bottom, channel) * (1.0 - across)
                        + texel(right, bottom, channel) * across;
                    rgba.push((upper * (1.0 - down) + lower * down).round() as u8);
                }
            }
        }
        let hotspot = [0, 1].map(|axis| {
            ((f64::from(self.hotspot[axis]) * scale[axis]).round() as u32).min(size[axis] - 1)
                as u16
        });
        Self {
            width: size[0] as u16,
            height: size[1] as u16,
            rgba,
            hotspot,
        }
    }
}

/// Native cursors for every original state, rebuilt when the window size or the
/// hardware-cursor setting changes the scale they are shown at.
pub struct Cursors {
    images: Images,
    scale: Option<[f64; 2]>,
    cursors: Vec<CustomCursor>,
}
impl Cursors {
    pub fn new(images: Images) -> Self {
        Self {
            images,
            scale: None,
            cursors: Vec::new(),
        }
    }
    /// Rebuilds the cursors for a window of `size` physical pixels. Returns whether
    /// they changed, in which case the shown cursor has to be set again.
    pub fn update(
        &mut self,
        event_loop: &ActiveEventLoop,
        size: [u32; 2],
        hardware_cursor: bool,
    ) -> Result<bool, String> {
        let scale = ui_scale(size, hardware_cursor);
        if self.scale == Some(scale) {
            return Ok(false);
        }
        self.cursors = self
            .images
            .0
            .iter()
            .map(|image| {
                let image = image.scaled(scale);
                let source = CustomCursor::from_rgba(
                    image.rgba,
                    image.width,
                    image.height,
                    image.hotspot[0],
                    image.hotspot[1],
                )
                .map_err(|e| e.to_string())?;
                Ok(event_loop.create_custom_cursor(source))
            })
            .collect::<Result<_, String>>()?;
        self.scale = Some(scale);
        Ok(true)
    }
    pub fn get(&self, kind: usize) -> Option<&CustomCursor> {
        self.cursors.get(kind)
    }
}
#[cfg(test)]
mod tests {
    use super::{Image, ui_scale};

    /// A `width`x`height` image, clear except for an opaque white `lit` rectangle
    /// starting at `at`.
    fn image(width: u16, height: u16, lit: [u16; 2], at: [u16; 2], hotspot: [u16; 2]) -> Image {
        let mut rgba = vec![0; usize::from(width) * usize::from(height) * 4];
        for y in at[1]..at[1] + lit[1] {
            for x in at[0]..at[0] + lit[0] {
                let i = (usize::from(y) * usize::from(width) + usize::from(x)) * 4;
                rgba[i..i + 4].copy_from_slice(&[255; 4]);
            }
        }
        Image {
            width,
            height,
            rgba,
            hotspot,
        }
    }

    /// Width and height of the pixels at least half opaque.
    fn lit_span(image: &Image) -> [u32; 2] {
        let (mut low, mut high) = ([u32::MAX; 2], [0; 2]);
        for (i, pixel) in image.rgba.as_chunks::<4>().0.iter().enumerate() {
            if pixel[3] >= 128 {
                let at = [
                    i as u32 % u32::from(image.width),
                    i as u32 / u32::from(image.width),
                ];
                for axis in 0..2 {
                    low[axis] = low[axis].min(at[axis]);
                    high[axis] = high[axis].max(at[axis]);
                }
            }
        }
        [0, 1].map(|axis| high[axis] + 1 - low[axis])
    }

    #[test]
    fn software_cursor_scales_each_axis_by_the_window_over_640_by_480() {
        assert_eq!(ui_scale([640, 480], false), [1.0, 1.0]);
        assert_eq!(ui_scale([1024, 768], false), [1.6, 1.6]);
        assert_eq!(ui_scale([1280, 960], false), [2.0, 2.0]);
        // Not 4:3: the original's UI space stretches each axis on its own.
        assert_eq!(ui_scale([1280, 480], false), [2.0, 1.0]);
        // Hardware cursors stay at texel size until #66 decides their presentation.
        assert_eq!(ui_scale([1024, 768], true), [1.0, 1.0]);
        // A minimised window keeps whatever cursors it has at texel size.
        assert_eq!(ui_scale([0, 0], false), [1.0, 1.0]);
    }

    #[test]
    fn scaled_cursor_size_hotspot_and_lit_span_follow_the_ui_scale() {
        // Block cursor: 36x35 record plus the clear row for hotspot (5,-1), about 30x30
        // lit texels. Arrow: hotspot (5,1), 12x16 lit texels.
        let block = image(36, 36, [30, 30], [3, 3], [5, 0]);
        let arrow = image(32, 32, [12, 16], [4, 1], [5, 1]);
        for (window, block_size, block_hotspot, block_lit, arrow_hotspot, arrow_lit) in [
            ([640, 480], [36, 36], [5, 0], [30, 30], [5, 1], [12, 16]),
            ([1024, 768], [58, 58], [8, 0], [48, 48], [8, 2], [20, 25]),
            ([1280, 960], [72, 72], [10, 0], [60, 60], [10, 2], [24, 32]),
        ] {
            let scale = ui_scale(window, false);
            let scaled = block.scaled(scale);
            assert_eq!([scaled.width, scaled.height], block_size, "{window:?}");
            assert_eq!(scaled.hotspot, block_hotspot, "{window:?}");
            assert_eq!(lit_span(&scaled), block_lit, "{window:?}");
            // The padded row stays clear, so the image still starts below the pointer.
            assert!(
                scaled.rgba[..usize::from(scaled.width) * 4]
                    .iter()
                    .all(|&v| v == 0)
            );
            let scaled = arrow.scaled(scale);
            assert_eq!(scaled.hotspot, arrow_hotspot, "{window:?}");
            assert_eq!(lit_span(&scaled), arrow_lit, "{window:?}");
        }
        // At 640x480 the image is the original texels unchanged.
        assert_eq!(block.scaled(ui_scale([640, 480], false)).rgba, block.rgba);
    }

    #[test]
    fn exact_integer_scale_keeps_solid_texels_solid() {
        let scaled = image(4, 4, [2, 2], [1, 1], [1, 1]).scaled([2.0, 2.0]);
        let alpha = |x: usize, y: usize| scaled.rgba[(y * 8 + x) * 4 + 3];
        // Texel (1,1) covers destination (2..4, 2..4); its inner pixels stay opaque.
        assert_eq!(alpha(3, 3), 255);
        assert_eq!(alpha(4, 4), 255);
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(7, 7), 0);
    }

    #[test]
    #[ignore = "requires original pan cursor oracle and archive"]
    fn matches_native_pan_cursor() {
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = openmt2001::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let images = super::Images::load(&directory, &archive).unwrap();
        let b = read("OPENMT2001_NATIVE_PAN_CURSORS");
        assert_eq!(&b[..8], b"MTCURS01");
        let w = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!(w(8), 3);
        assert_eq!(b.len(), 72);
        let i = &images.0[2];
        assert_eq!(w(52), 3);
        assert_eq!(u32::from(i.width), w(56));
        assert_eq!(u32::from(i.height), w(60) + 1);
        assert_eq!(u32::from(i.hotspot[0]), w(64));
        assert_eq!(w(68), u32::MAX);
        assert_eq!(i.hotspot[1], 0);
        assert!(i.rgba[..usize::from(i.width) * 4].iter().all(|&v| v == 0));
    }
    #[test]
    #[ignore = "requires original archive"]
    fn city_hover_cursors_use_records_four_to_seven_one_row_below_the_pointer() {
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = openmt2001::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let entry = directory.find("gamedata2/Cursors/textures.gtd").unwrap();
        let set = openmt2001::texture::TextureSet::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        let images = super::Images::load(&directory, &archive).unwrap();
        for record in 4..8 {
            let texture = &set.textures[record];
            let image = &images.0[record];
            assert_eq!(u32::from(image.width), texture.width);
            // Hotspot (5,-1): one clear row above the record's own pixels.
            assert_eq!(u32::from(image.height), texture.height + 1);
            assert_eq!(image.hotspot, [5, 0]);
            let row = usize::from(image.width) * 4;
            assert!(image.rgba[..row].iter().all(|&v| v == 0));
            assert_eq!(
                image.rgba[row..],
                openmt2001::texture::loader_pixels(texture)[..]
            );
        }
    }
    #[test]
    #[ignore = "requires original archive and native cursor setter observations"]
    fn matches_native_inspection_cursor() {
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = openmt2001::archive::ArchiveIndex::parse(
            &read("OPENMT2001_ARCHIVE_DIR"),
            archive.len() as u64,
        )
        .unwrap();
        let images = super::Images::load(&directory, &archive).unwrap();
        let fixture = read("OPENMT2001_NATIVE_INSPECTION_CURSOR");
        assert_eq!(&fixture[..8], b"MTCURS01");
        let word = |at| u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 2);
        assert_eq!(fixture.len(), 52);
        for (i, image) in images.0.iter().take(2).enumerate() {
            let at = 12 + i * 20;
            assert_eq!(word(at), [0, 2][i]);
            assert_eq!(
                [
                    u32::from(image.width),
                    u32::from(image.height),
                    u32::from(image.hotspot[0]),
                    u32::from(image.hotspot[1])
                ],
                std::array::from_fn(|j| word(at + 4 + j * 4))
            );
        }
    }
}
