//! Normal Event List subtype41cdc0/41cfb0/41d000 over original resources.
//! The caller supplies authoritative settled visibility; nativeShow keeps offset−100.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    font::{Font, Layout},
    mesh::CachedMesh,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};

struct Label {
    id: u32,
    bounds: [f32; 4],
    paint: [f32; 4],
    colors: [u32; 4],
    centered: bool,
}
pub struct Assets {
    border: Vec<Surface>,
    pages: Vec<Surface>,
    tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    labels: Vec<Label>,
    root_bounds: [f32; 4],
}
#[derive(Default)]
pub struct View {
    settled: Option<bool>,
    pub prepared: Option<PreparedScene>,
}
impl View {
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        visible: bool,
        settled: bool,
    ) -> Result<(), String> {
        if !visible {
            self.prepared = None;
            self.settled = None;
        } else if self.prepared.is_none() || self.settled != Some(settled) {
            self.prepared = Some(assets.prepare(renderer, settled)?);
            self.settled = Some(settled);
        }
        Ok(())
    }
}
const TEXT_ORDER: [usize; 11] = [10, 1, 0, 3, 2, 5, 4, 7, 6, 9, 8];
fn labels(regions: &[(String, [f32; 4])]) -> Result<Vec<Label>, String> {
    let bounds = |name: &str| {
        regions
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| *b)
            .ok_or_else(|| format!("Missing Event List region {name}"))
    };
    let mut labels = Vec::with_capacity(11);
    for row in 0..5 {
        let paint = bounds(&format!("DAY_GREEN_AREA{}", row + 1))?;
        labels.push(Label {
            id: 1491 + row,
            bounds: paint,
            paint,
            colors: [0xff01762f; 4],
            centered: false,
        });
        let paint = bounds(&format!("DAY_TEXTBOX{}", row + 1))?;
        let mut text = paint;
        text[0] += 3.; //41cdc0 text inset;41d000 paints the original full area.
        labels.push(Label {
            id: 1496 + row,
            bounds: text,
            paint,
            colors: [0x80000d5a; 4],
            centered: false,
        });
    }
    let paint = bounds("DAY_TITLE_AREA")?;
    labels.push(Label {
        id: 1501,
        bounds: paint,
        paint,
        colors: [0xff909090, 0xff909090, 0x80909090, 0x80909090],
        centered: true,
    });
    Ok(labels)
}
fn shifted([l, t, r, b]: [f32; 4], [x, y]: [f32; 2]) -> [f32; 4] {
    [l + x, t + y, r + x, b + y]
}
fn text_glyphs(
    tab: &[u8],
    text: &[u8],
    label: &Label,
    offset: [f32; 2],
) -> Result<Vec<openmt2001::font::GlyphQuad>, String> {
    let font = Font::parse(tab)?;
    let line_height =
        i32::from_le_bytes(tab[4 + 55 * 44 + 36..4 + 55 * 44 + 40].try_into().unwrap());
    let mut glyphs = Vec::new();
    let [l, t, r, b] = label.bounds;
    let (lines, copy) = crate::prompt::wrap_with_gap(tab, text, (r - l - 4.).trunc() as i32, -1)?;
    let count = lines.len().saturating_sub(1);
    let y = ((f64::from(b) - f64::from(t) - f64::from(line_height) * count as f64) * 0.5).trunc()
        as f32;
    for (row, pair) in lines.windows(2).enumerate() {
        let end = (pair[1] - 1).min(copy.len());
        let text = &copy[pair[0].min(end)..end];
        let width = ui::text_width(tab, text, 1.)?;
        let x = if label.centered {
            ((f64::from(r) - f64::from(l) - f64::from(width)) * 0.5 + 0.5).trunc() as f32
        } else {
            2.
        };
        glyphs.extend(font.layout(
            text,
            Layout {
                origin: [
                    l.trunc() + x + offset[0].trunc(),
                    t.trunc() + y + offset[1].trunc() + row as f32 * line_height as f32,
                ],
                depth: 0.5,
                scale: 1.,
                gap: -1,
                baseline: 0,
            },
        )?);
    }
    Ok(glyphs)
}
impl Assets {
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
    ) -> Result<Self, String> {
        let payload = |name: &str| {
            let entry = index
                .find(name)
                .ok_or_else(|| format!("Missing Event List asset {name}"))?;
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or_else(|| "Event List asset range".to_string())
        };
        let name = "gamedata2/brand_new_panels/day_tutor_panel.tm";
        let mesh = CachedMesh::parse(payload(name)?)?;
        let root_bounds = openmt2001::ui_mesh::parent_bounds(&mesh, 0)?;
        let regions = openmt2001::ui_mesh::regions(&mesh, 0)?
            .into_iter()
            .map(|region| (region.name, region.bounds))
            .collect::<Vec<_>>();
        let (border, _) = ui::panel_frame_base(index, archive, name, &[0, 12, 13, 14, 15], 0)?;
        let font = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let tab = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        Font::parse(&tab)?;
        Ok(Self {
            border,
            pages: font.textures[6..8].iter().map(Surface::new).collect(),
            tab,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            labels: labels(&regions)?,
            root_bounds,
        })
    }
    ///41d000 backgrounds, original border geometry, then41cfb0's text order.
    pub fn input_bounds(&self) -> [f32; 4] {
        shifted(self.root_bounds, [0., -100.])
    }
    pub fn contains(&self, point: [f32; 2]) -> bool {
        openmt2001::ui_mesh::parent_hit(self.root_bounds, point, [0., -100.])
    }
    pub fn prepare(&self, renderer: &Renderer, settled: bool) -> Result<PreparedScene, String> {
        let offset = [0., -100.];
        let mut background = Surface::solid();
        for index in std::iter::once(10).chain(0..10).filter(|_| settled) {
            let label = &self.labels[index];
            background.rect(shifted(label.paint, offset), label.colors);
        }
        let mut surfaces = vec![background];
        for mut border in self.border.clone() {
            for vertex in &mut border.vertices {
                vertex[0] += offset[0];
                vertex[1] += offset[1];
            }
            surfaces.push(border);
        }
        let mut pages = self.pages.clone();
        for index in TEXT_ORDER.into_iter().filter(|_| settled) {
            let label = &self.labels[index];
            let text = self
                .strings
                .get(label.id as usize)
                .ok_or("Event List text range")?;
            for glyph in text_glyphs(&self.tab, text, label, offset)? {
                pages[glyph.page].quad(
                    glyph.positions.map(|p| [p[0], p[1]]),
                    glyph.uv,
                    [0xffffffff; 4],
                );
            }
        }
        surfaces.extend(pages);
        renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
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

#[cfg(test)]
mod tests {
    use super::*;
    fn word(bytes: &[u8], at: &mut usize) -> u32 {
        let value = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
        *at += 4;
        value
    }
    fn original_regions() -> Vec<(String, [f32; 4])> {
        let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let entry = index
            .find("gamedata2/brand_new_panels/day_tutor_panel.tm")
            .unwrap();
        let mesh = CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )
        .unwrap();
        openmt2001::ui_mesh::regions(&mesh, 0)
            .unwrap()
            .into_iter()
            .map(|r| (r.name, r.bounds))
            .collect()
    }
    #[test]
    #[ignore = "original EventList constructor/background and full font3 native streams /037f"]
    fn original_event_list_resources() {
        let labels = labels(&original_regions()).unwrap();
        let read = |name: &str| {
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("work/frontend")
                    .join(name),
            )
            .unwrap()
        };
        let bytes = read("native-tutorial-event-list.bin");
        assert_eq!(&bytes[..8], b"MTEVLST1");
        let mut at = 8;
        assert_eq!(word(&bytes, &mut at), 0x037f);
        assert_eq!(word(&bytes, &mut at), 11);
        for (index, label) in labels.iter().enumerate() {
            assert_eq!(word(&bytes, &mut at) as usize, index);
            for value in label.bounds.into_iter().chain([0.5]) {
                assert_eq!(word(&bytes, &mut at), value.to_bits());
            }
            assert_eq!(word(&bytes, &mut at), 3);
            assert_eq!(word(&bytes, &mut at), label.id);
            assert_eq!(word(&bytes, &mut at), if label.centered { 4 } else { 3 });
            assert_eq!(word(&bytes, &mut at) as i32, -999);
            assert_eq!(word(&bytes, &mut at) as i32, -999);
        }
        assert_eq!(word(&bytes, &mut at), 11);
        for index in TEXT_ORDER {
            assert_eq!(word(&bytes, &mut at) as usize, index);
        }
        assert_eq!(word(&bytes, &mut at), 2);
        for _ in 0..2 {
            let offset = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
            assert_eq!(word(&bytes, &mut at), 11);
            for index in std::iter::once(10).chain(0..10) {
                for value in shifted(labels[index].paint, offset) {
                    assert_eq!(word(&bytes, &mut at), value.to_bits());
                }
                for color in labels[index].colors {
                    assert_eq!(word(&bytes, &mut at), color);
                }
                assert_eq!(word(&bytes, &mut at), 0.5f32.to_bits());
            }
        }
        assert_eq!(at, bytes.len());
        let bytes = read("native-event-list-text.bin");
        assert_eq!(&bytes[..8], b"MTEVTXT1");
        let mut at = 8;
        assert_eq!(word(&bytes, &mut at), 0x037f);
        let length = word(&bytes, &mut at) as usize;
        let tab = &bytes[at..at + length];
        at += length;
        assert_eq!(word(&bytes, &mut at), 22);
        for _ in 0..22 {
            let index = word(&bytes, &mut at) as usize;
            let length = word(&bytes, &mut at) as usize;
            let text = &bytes[at..at + length];
            at += length;
            let offset = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
            let mut pages: [(Vec<f32>, Vec<f32>); 2] = Default::default();
            for glyph in text_glyphs(tab, text, &labels[index], offset).unwrap() {
                pages[glyph.page]
                    .0
                    .extend(glyph.positions.into_iter().flatten());
                pages[glyph.page].1.extend(glyph.uv.into_iter().flatten());
            }
            assert_eq!(word(&bytes, &mut at), 2);
            for (vertices, uv) in pages {
                for values in [vertices, uv] {
                    assert_eq!(word(&bytes, &mut at) as usize, values.len(), "label{index}");
                    for value in values {
                        assert_eq!(
                            word(&bytes, &mut at),
                            value.to_bits(),
                            "label{index} offset{offset:?}"
                        );
                    }
                }
            }
        }
        assert_eq!(at, bytes.len());
    }
    #[test]
    #[ignore = "original EventList parent input and opening/settled GPU resources"]
    fn original_event_list_input_and_capture() {
        let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = openmt2001::fixtures::path("PARAMETERS");
        let installer = parameters.parent().unwrap().parent().unwrap();
        let executable = openmt2001::fixtures::path("EXE");
        let language = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let offsets = fs::read(language.join("tycoon.off")).unwrap();
        let strings = fs::read(language.join("tycoon.bin")).unwrap();
        let mut at = 0;
        let strings = offsets
            .chunks_exact(2)
            .map(|n| {
                let end = at + u16::from_le_bytes(n.try_into().unwrap()) as usize;
                let text = &strings[at..end];
                at = end;
                text
            })
            .collect::<Vec<_>>();
        let assets = Assets::load(&index, &archive, installer, &strings).unwrap();
        let bytes = fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("work/frontend/native-tutorial-event-list-input.bin"),
        )
        .unwrap();
        assert_eq!(&bytes[..8], b"MTEVINP1");
        let mut at = 8;
        assert_eq!(word(&bytes, &mut at), 0x037f);
        let bounds = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
        let offset = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
        assert_eq!(offset, [0., -100.]);
        assert_eq!(
            assets.input_bounds().map(f32::to_bits),
            shifted(bounds, offset).map(f32::to_bits)
        );
        let count = word(&bytes, &mut at);
        for _ in 0..count {
            let point = std::array::from_fn(|_| f32::from_bits(word(&bytes, &mut at)));
            assert_eq!(
                assets.contains(point),
                word(&bytes, &mut at) != 0,
                "{point:?}"
            );
        }
        assert_eq!(at, bytes.len());
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_EVENT_LIST_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            for settled in [false, true] {
                let panel = assets.prepare(&renderer, settled).unwrap();
                crate::capture_test::capture_standalone(
                    &renderer,
                    &panel,
                    &format!("{path}-{}.png", u8::from(settled)),
                );
            }
        }
    }
}
