//! Navigator, strategic and ordinary-panel tooltips:43e0a0/43e3c0, font0.
use crate::ui::Surface;
use openmt2001::{
    archive::ArchiveIndex,
    font::{Font, Layout},
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};
pub struct Tooltip {
    pages: Vec<Surface>,
    tab: Vec<u8>,
    names: Vec<Vec<u8>>,
    icon_labels: [Vec<u8>; 8],
    city_labels: [Vec<u8>; 4],
    compass_labels: [Vec<u8>; 2],
    speed_labels: [Vec<u8>; 2],
}
impl Tooltip {
    pub(crate) fn apply_board(&mut self, board: &openmt2001::packed_language::PackedLanguage) {
        self.names = board.rows().into_iter().map(<[u8]>::to_vec).collect();
    }

    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
    ) -> Result<Self, String> {
        Self::load_impl(index, archive, installer, language, None)
    }
    pub(crate) fn load_shared(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
        shared: &crate::intro::camera_live::ui_text_resources::UiTextResources,
    ) -> Result<Self, String> {
        Self::load_impl(index, archive, installer, language, Some(shared))
    }
    fn load_impl(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
        shared: Option<&crate::intro::camera_live::ui_text_resources::UiTextResources>,
    ) -> Result<Self, String> {
        let (pages, tab) = if let Some(shared) = shared {
            (shared.surfaces(0)?, shared.table(0)?.to_vec())
        } else {
            let entry = index
                .find("gamedata2/font640/textures.gtd")
                .ok_or("tooltip font texture missing")?;
            let textures = TextureSet::parse(
                archive
                    .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                    .ok_or("tooltip texture range")?,
            )
            .map_err(|e| e.to_string())?;
            let pages = textures
                .textures
                .get(..2)
                .ok_or("tooltip font pages missing")?
                .iter()
                .map(Surface::new)
                .collect();
            let tab = fs::read(installer.join("Standard/fonts/cust640p.tab"))
                .map_err(|e| e.to_string())?;
            Font::parse(&tab)?;
            (pages, tab)
        };
        let names = board_strings(installer)?;
        let loaded;
        let packed = match shared {
            Some(text) => &text.fonts().language,
            None => {
                loaded =
                    openmt2001::packed_language::PackedLanguage::load(&language.join("tycoon"))?;
                &loaded
            }
        };
        let labels = packed.rows();
        //40d8f2's four five-word descriptors store these text IDs in child+34.
        let icon_labels = [1146, 1147, 1148, 12, 29, 50, 52, 1141]
            .map(|id| {
                labels
                    .get(id)
                    .map(|s| s.to_vec())
                    .ok_or("panel icon tooltip missing".to_owned())
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "panel icon tooltip count")?;
        //410dbd copies 533788's four descriptor IDs to the green children.
        let city_labels = [1150, 1151, 1152, 1153]
            .map(|id| {
                labels
                    .get(id)
                    .map(|s| s.to_vec())
                    .ok_or("city tab tooltip missing".to_owned())
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "city tab tooltip count")?;
        let compass_labels = [1142, 1143]
            .map(|id| {
                labels
                    .get(id)
                    .map(|s| s.to_vec())
                    .ok_or("compass focus caption missing".to_owned())
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "compass focus caption count")?;
        let speed_labels = [1144, 1145]
            .map(|id| {
                labels
                    .get(id)
                    .map(|s| s.to_vec())
                    .ok_or("speed tooltip missing".to_owned())
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "speed tooltip count")?;
        Ok(Self {
            pages,
            tab,
            names,
            icon_labels,
            city_labels,
            compass_labels,
            speed_labels,
        })
    }
    fn focus_identifier(&self, text_id: u32) -> Result<&[u8], String> {
        let name = match text_id {
            1150..=1153 => &self.city_labels[(text_id - 1150) as usize],
            1142 => &self.compass_labels[0],
            1143 => &self.compass_labels[1],
            1144 => &self.speed_labels[0],
            1145 => &self.speed_labels[1],
            1146 => &self.icon_labels[0],
            1147 => &self.icon_labels[1],
            1148 => &self.icon_labels[2],
            12 => &self.icon_labels[3],
            29 => &self.icon_labels[4],
            50 => &self.icon_labels[5],
            52 => &self.icon_labels[6],
            1141 => &self.icon_labels[7],
            id => self
                .names
                .get(id.checked_sub(2242).ok_or("tooltip text ID")? as usize + 1)
                .ok_or("tooltip category text range")?,
        };
        Ok(name)
    }
    /// Real 43e0a0 font/extent consumer for the finite identifier captions.
    pub(crate) fn measure_focus_identifier(&self, text_id: u32) -> Result<i32, String> {
        let name = self.focus_identifier(text_id)?;
        let width = measure_with_gap(&self.tab, name, -1)?;
        if width >= 150 {
            return Err("tooltip requires unsupported multi-line layout".into());
        }
        Ok(width)
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        text_id: u32,
        point: [i32; 2],
    ) -> Result<PreparedScene, String> {
        let name = self.focus_identifier(text_id)?;
        let width = self.measure_focus_identifier(text_id)?;
        //43e040 sets line gap2;4406a0 measures font0 W height8, giving10.
        //4e6d85 initializes font baseline0; glyph gap−1 is independent.
        let b = rectangle(point, [width as f32, 10.]);
        let mut background = Surface::solid();
        background.rect(b, [0x9f000000; 4]);
        let mut pages = self.pages.clone();
        for q in Font::parse(&self.tab)?.layout(
            name,
            Layout {
                origin: [(b[0] + 10.).trunc() + 2., (b[1] + 2.).trunc()],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )? {
            pages[q.page].quad(q.positions.map(|p| [p[0], p[1]]), q.uv, [0xffffffff; 4]);
        }
        let mut draws = vec![background.draw()];
        draws.extend(pages.iter().map(Surface::draw));
        renderer.prepare(
            &draws,
            [
                2. / 640.,
                0.,
                0.,
                0.,
                0.,
                -2. / 480.,
                0.,
                0.,
                0.,
                0.,
                1.,
                0.,
                -1.,
                1.,
                0.,
                1.,
            ],
            None,
        )
    }
}

///43e310 clears the hovered widget timer; 43e3b0 increments each interface
/// frame; 43e3c0 paints only when the timer exceeds 25.
#[derive(Default)]
pub struct IconDwell {
    target: Option<u32>,
    ticks: u32,
}
impl IconDwell {
    pub fn advance(&mut self, target: Option<u32>) -> Option<u32> {
        if self.target != target {
            self.target = target;
            self.ticks = 0;
        }
        let id = target?;
        self.ticks = self.ticks.saturating_add(1);
        (self.ticks > 25).then_some(id)
    }
}
/// Controlled gap0 oracle contract; production uses explicit gap−1.
#[cfg(test)]
pub(crate) fn measure(tab: &[u8], name: &[u8]) -> Result<i32, String> {
    measure_with_gap(tab, name, 0)
}
pub(crate) fn measure_with_gap(tab: &[u8], name: &[u8], gap: i32) -> Result<i32, String> {
    Font::parse(tab)?;
    name.iter().try_fold(0i32, |sum, &c| {
        let at = 4 + usize::from(c) * 44 + 32;
        let advance = i32::from_le_bytes(
            tab.get(at..at + 4)
                .ok_or("tooltip glyph metric")?
                .try_into()
                .unwrap(),
        );
        advance
            .checked_add(gap)
            .and_then(|advance| sum.checked_add(advance))
            .ok_or("tooltip width overflow".into())
    })
}
fn rectangle(point: [i32; 2], size: [f32; 2]) -> [f32; 4] {
    let width = (f64::from(size[0]) + 20.) as f32;
    let height = f64::from(size[1]) + 4.;
    let mut top = (f64::from(point[1]) + 33.) as f32;
    let mut left = (f64::from(point[0])
        - f64::from(point[0]) * f64::from(1f32 / 640.) * f64::from(width))
        as f32;
    left = left.max(0.);
    if f64::from(left) + f64::from(width) >= 640. {
        left = (640. - f64::from(width)) as f32;
    }
    top = top.max(0.);
    if f64::from(top) + height >= 480. {
        top = (480. - height) as f32;
    }
    [
        left,
        top,
        (f64::from(left) + f64::from(width)) as f32,
        (f64::from(top) + height) as f32,
    ]
}
///4922e0 installs supplied USA board strings at2242, skipping source row0.
pub(crate) fn board_strings(installer: &Path) -> Result<Vec<Vec<u8>>, String> {
    //4922e0 installs USA board strings at2242, skipping the source character map.
    //The current native inspection host uses the supplied USA board.
    let path = installer.join("Standard/fonts/board/usa");
    let packed = openmt2001::packed_language::PackedLanguage::load(&path.join("tycoon"))?;
    Ok(packed.rows().into_iter().map(<[u8]>::to_vec).collect())
}

/// Original4922e0 replaces base-language placeholders with the selected board's
/// currency and property labels. The current frontend selects supplied USA.
pub(crate) fn localized_strings(installer: &Path, base: &[&[u8]]) -> Result<Vec<Vec<u8>>, String> {
    let mut strings: Vec<_> = base.iter().map(|s| s.to_vec()).collect();
    for (i, text) in board_strings(installer)?.into_iter().skip(1).enumerate() {
        *strings
            .get_mut(2242 + i)
            .ok_or("board language destination range")? = text;
    }
    Ok(strings)
}

#[cfg(test)]
mod tests {
    #[test]
    fn strategic_icon_dwell_needs_26_frames_and_resets_on_exit() {
        let mut timer = super::IconDwell::default();
        for _ in 0..25 {
            assert_eq!(timer.advance(Some(1146)), None);
        }
        assert_eq!(timer.advance(Some(1146)), Some(1146));
        assert_eq!(timer.advance(Some(1147)), None);
        assert_eq!(timer.advance(None), None);
        for _ in 0..25 {
            assert_eq!(timer.advance(Some(1146)), None);
        }
        assert_eq!(timer.advance(Some(1146)), Some(1146));
    }

    #[test]
    #[ignore = "supplied original panel descriptors/language/fonts and GPU"]
    fn original_ordinary_toolbar_tooltip_ids_render_with_existing_families() {
        let read = |key| std::fs::read(openmt2001::fixtures::path(key)).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe_path = openmt2001::fixtures::path("EXE");
        let exe = std::fs::read(&exe_path).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let titles = image.at(0x533238, 12).unwrap();
        let ids: [u32; 3] = std::array::from_fn(|n| {
            u32::from_le_bytes(titles[n * 4..n * 4 + 4].try_into().unwrap())
        });
        assert_eq!(ids, [29, 50, 52]);
        let installer = openmt2001::fixtures::path("INSTALLER");
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        let assets = super::Tooltip::load(&index, &archive, &installer, &language).unwrap();
        for id in ids.into_iter().chain([
            1144, 1145, 1146, 1147, 1148, 1150, 1151, 1152, 1153, 12, 2242,
        ]) {
            let panel = assets.prepare(&renderer, id, [40, 100]).unwrap();
            #[cfg(feature = "capture")]
            if ids.contains(&id)
                && let Ok(out) = std::env::var("OPENMT2001_TOOLBAR_CAPTURE")
            {
                let path = std::path::PathBuf::from(out).join(format!("toolbar-tooltip-{id}.png"));
                crate::capture_test::capture_standalone(&renderer, &panel, path.to_str().unwrap());
            }
        }
    }

    #[test]
    #[ignore = "original v1.4 PE and localized language inputs"]
    fn strategic_icon_descriptors_and_localized_lines() {
        let path = openmt2001::fixtures::path("EXE");
        let exe = std::fs::read(&path).unwrap();
        let pe = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let table = pe.at(0x533738, 4 * 20).unwrap();
        for (row, expected) in [
            [1, 0x53380c, 66, 1146, 20],
            [2, 0x5337fc, 68, 1147, 21],
            [3, 0x5337ec, 198, 1148, 22],
            [4, 0x5337dc, 26, 12, 23],
        ]
        .iter()
        .enumerate()
        {
            let words: [u32; 5] = std::array::from_fn(|n| {
                u32::from_le_bytes(
                    table[row * 20 + n * 4..row * 20 + n * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            });
            assert_eq!(&words, expected);
        }
        let installer = openmt2001::fixtures::path("INSTALLER");
        let language = installer.join("US/language/usa");
        let city = pe.at(0x533788, 32).unwrap();
        for (row, (texture, id)) in [(290u32, 1150u32), (75, 1151), (21, 1152), (70, 1153)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(&city[row * 8..row * 8 + 4], &texture.to_le_bytes());
            assert_eq!(&city[row * 8 + 4..row * 8 + 8], &id.to_le_bytes());
        }
        let off = std::fs::read(language.join("tycoon.off")).unwrap();
        let bin = std::fs::read(language.join("tycoon.bin")).unwrap();
        let tab = std::fs::read(installer.join("Standard/fonts/cust640p.tab")).unwrap();
        let mut at = 0;
        for (id, length) in off.chunks_exact(2).enumerate() {
            let end = at + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            if [12, 1146, 1147, 1148, 1150, 1151, 1152, 1153].contains(&id) {
                assert!(
                    super::measure_with_gap(&tab, &bin[at..end], -1).unwrap() < 150,
                    "tooltip id {id} exceeds original one-line width"
                );
            }
            at = end;
        }
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "original archive and GPU"]
    fn capture_strategic_icon_tooltip() {
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let installer = openmt2001::fixtures::path("INSTALLER");
        let language = installer.join("US/language/usa");
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        let asset = super::Tooltip::load(&index, &archive, &installer, &language).unwrap();
        let label = asset.prepare(&renderer, 1146, [40, 100]).unwrap();
        crate::capture_test::capture_standalone(
            &renderer,
            &label,
            "work/frontend/strategic-tooltip-locator.png",
        );
    }

    #[test]
    #[ignore = "original production43e3c0/4406a0 tooltip glyph oracle /037f"]
    fn matches_native_production_tooltip_layout() {
        let bytes = std::fs::read(openmt2001::fixtures::path("NATIVE_TOOLTIP_LAYOUT")).unwrap();
        assert_eq!(&bytes[..8], b"MTTIPL01");
        fn word(bytes: &[u8], at: &mut usize) -> u32 {
            let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            value
        }
        let mut at = 8;
        assert_eq!(word(&bytes, &mut at), 0x037f);
        let length = word(&bytes, &mut at) as usize;
        let tab = &bytes[at..at + length];
        at += length;
        assert_eq!(word(&bytes, &mut at), 117);
        let font = openmt2001::font::Font::parse(tab).unwrap();
        for _ in 0..117 {
            let id = word(&bytes, &mut at);
            let length = word(&bytes, &mut at) as usize;
            let name = &bytes[at..at + length];
            at += length;
            let point = std::array::from_fn(|_| word(&bytes, &mut at) as i32);
            let width = f32::from_bits(word(&bytes, &mut at));
            assert_eq!(
                super::measure_with_gap(tab, name, -1).unwrap() as f32,
                width,
                "name{id}"
            );
            let b = super::rectangle(point, [width, 10.]);
            assert_eq!(word(&bytes, &mut at), 12);
            for value in [
                [b[0], b[1], 0.5],
                [b[0], b[3], 0.5],
                [b[2], b[3], 0.5],
                [b[2], b[1], 0.5],
            ]
            .into_iter()
            .flatten()
            {
                assert_eq!(word(&bytes, &mut at), value.to_bits(), "rectangle{id}");
            }
            let mut pages: [(Vec<f32>, Vec<f32>); 2] = Default::default();
            for glyph in font
                .layout(
                    name,
                    openmt2001::font::Layout {
                        origin: [(b[0] + 10.).trunc() + 2., (b[1] + 2.).trunc()],
                        depth: 0.5,
                        scale: 1.,
                        gap: -1,
                        baseline: 0,
                    },
                )
                .unwrap()
            {
                pages[glyph.page]
                    .0
                    .extend(glyph.positions.into_iter().flatten());
                pages[glyph.page].1.extend(glyph.uv.into_iter().flatten());
            }
            assert_eq!(word(&bytes, &mut at), 2);
            for (vertices, uv) in pages {
                for values in [vertices, uv] {
                    assert_eq!(word(&bytes, &mut at) as usize, values.len());
                    for value in values {
                        assert_eq!(
                            word(&bytes, &mut at),
                            value.to_bits(),
                            "glyph{id} at{point:?}"
                        );
                    }
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "requires original tooltip oracle"]
    fn matches_native_tooltip() {
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_TOOLTIP")).unwrap();
        assert_eq!(&b[..8], b"MTTIP001");
        let w = |a| u32::from_le_bytes(b[a..a + 4].try_into().unwrap());
        let f = |a| f32::from_bits(w(a));
        let mut at = 12;
        for _ in 0..w(8) {
            let bounds =
                super::rectangle([w(at) as i32, w(at + 4) as i32], [f(at + 8), f(at + 12)]);
            at += 16;
            let [l, t, r, bot] = bounds;
            let actual = [[l, t, 0.5], [l, bot, 0.5], [r, bot, 0.5], [r, t, 0.5]]
                .into_iter()
                .flatten()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>();
            assert_eq!(actual, b[at..at + 48]);
            at += 48;
        }
        let n = w(at) as usize;
        at += 4;
        let tab = &b[at..at + n];
        at += n;
        let count = w(at);
        at += 4;
        for _ in 0..count {
            let id = w(at);
            let n = w(at + 4) as usize;
            let lines = w(at + 8);
            let width = w(at + 12);
            at += 16;
            let text = &b[at..at + n];
            at += n;
            assert_eq!(lines, 1, "text{id}");
            assert_eq!(w(at), 0);
            assert_eq!(w(at + 4), n as u32 + 1);
            assert_eq!(super::measure(tab, text).unwrap() as u32, width);
            assert!(width < 150);
            at += 8;
        }
        assert_eq!(at, b.len());
    }
}
