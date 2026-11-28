//! Original420100/420670/420890 transient rival notices, at their original
//!640x480 coordinates with font3, character colours and localized text.
use crate::{
    notifications::Notice,
    ui::{self, Surface},
};
use openmt2001::{
    archive::ArchiveIndex,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
use std::{fs, path::Path};
pub struct Assets {
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    categories: Vec<u32>,
    businesses: Vec<u32>,
    colors: Vec<u32>,
}
impl Assets {
    pub(crate) fn apply_board(
        &mut self,
        board: &openmt2001::packed_language::PackedLanguage,
    ) -> Result<(), String> {
        crate::board_selection::overlay(&mut self.strings, board)
    }

    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        strings: &[&[u8]],
        exe: &[u8],
    ) -> Result<Self, String> {
        let entry = index
            .find("gamedata2/font640/textures.gtd")
            .ok_or("Notification font archive missing")?;
        let set = TextureSet::parse(
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Notification font range")?,
        )
        .map_err(|e| e.to_string())?;
        let image = PeImage::parse(exe)?;
        let table = |base: u32, count: u32, stride: u32| -> Result<Vec<u32>, String> {
            (0..count)
                .map(|i| {
                    Ok(u32::from_le_bytes(
                        image.at(base + i * stride, 4)?.try_into().unwrap(),
                    ))
                })
                .collect()
        };
        Ok(Self {
            fonts: set.textures[6..8].iter().map(Surface::new).collect(),
            tab: fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
                .map_err(|e| e.to_string())?,
            strings: crate::tooltip::localized_strings(installer, strings)?,
            categories: table(0x5385a0, 52, 88)?,
            businesses: table(0x53a964, 45, 116)?,
            colors: table(0x541af0, 10, 40)?,
        })
    }
    fn text(&self, notice: &Notice) -> Result<Vec<u8>, String> {
        let end = notice
            .actor
            .iter()
            .position(|b| *b == 0)
            .ok_or("Notification actor terminator")?;
        let actor = notice.actor[..end]
            .iter()
            .map(|b| {
                b.checked_sub(32)
                    .filter(|c| *c <= 94)
                    .ok_or("Notification actor glyph")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let category = *self
            .categories
            .get(notice.category_kind as usize)
            .ok_or("Notification category type")?
            + 1;
        let category = self
            .strings
            .get(category as usize)
            .ok_or("Notification category language")?;
        let mut parameters = vec![actor.as_slice()];
        let id = if let Some(business) = notice.business_kind {
            let name = *self
                .businesses
                .get(business as usize)
                .ok_or("Notification business type")?;
            parameters.push(
                self.strings
                    .get(name as usize)
                    .ok_or("Notification business language")?,
            );
            1507
        } else {
            1506
        };
        parameters.push(category);
        let mut text = self
            .strings
            .get(id)
            .ok_or("Notification text language")?
            .clone();
        for value in parameters {
            let at = text
                .windows(2)
                .position(|s| s[0] == b'%' - 32 && [b's' - 32, b'i' - 32].contains(&s[1]))
                .ok_or("Notification template placeholder")?;
            text.splice(at..at + 2, value.iter().copied());
        }
        Ok(text)
    }
    pub fn prepare(
        &self,
        renderer: &Renderer,
        notices: &[Notice],
        hover: Option<usize>,
    ) -> Result<PreparedScene, String> {
        let mut backgrounds = Surface::solid();
        let mut fonts = self.fonts.clone();
        for (i, notice) in notices.iter().enumerate() {
            let color = if hover == Some(i) {
                0xff000000
            } else {
                *self
                    .colors
                    .get(notice.character as usize)
                    .ok_or("Notification character colour")?
                    | 0xff000000
            };
            let faded = (color & 0xffffff) | 0x77000000;
            let y = 5. + 12. * i as f32;
            backgrounds.rect([191., y, 492., y + 12.], [color, color, faded, faded]);
            ui::text(
                &mut fonts,
                &self.tab,
                &self.text(notice)?,
                [191., y, 492., y + 11.],
                false,
                u32::MAX,
            )?;
        }
        let surfaces = std::iter::once(backgrounds)
            .chain(fonts)
            .collect::<Vec<_>>();
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
#[derive(Default)]
pub struct View {
    key: Option<(Vec<Notice>, Option<usize>)>,
    pub prepared: Option<PreparedScene>,
}
impl View {
    fn hover(notices: &[Notice], pointer: Option<[f32; 2]>) -> Option<usize> {
        let [x, y] = pointer?;
        notices.iter().enumerate().find_map(|(i, _)| {
            let top = 5. + 12. * i as f32;
            ((191. ..=492.).contains(&x) && (top..=top + 11.).contains(&y)).then_some(i)
        })
    }
    pub fn category(notices: &[Notice], pointer: Option<[f32; 2]>) -> Option<u32> {
        Self::hover(notices, pointer).map(|i| notices[i].category)
    }
    pub fn prepare(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        notices: &[Notice],
        pointer: Option<[f32; 2]>,
    ) -> Result<(), String> {
        let key = (!notices.is_empty()).then(|| (notices.to_vec(), Self::hover(notices, pointer)));
        if self.key != key {
            self.prepared = key
                .as_ref()
                .map(|(n, h)| assets.prepare(renderer, n, *h))
                .transpose()?;
            self.key = key;
        }
        Ok(())
    }
}

#[test]
#[ignore = "requires original notification fonts/language and GPU"]
fn original_notification_panel() {
    let read = |key| fs::read(openmt2001::fixtures::path(key)).unwrap();
    let exe = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = openmt2001::fixtures::path("PARAMETERS");
    let installer = parameter.parent().unwrap().parent().unwrap();
    let exe_path = openmt2001::fixtures::path("EXE");
    let language = exe_path
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("US/language/usa");
    let offsets = fs::read(language.join("tycoon.off")).unwrap();
    let bytes = fs::read(language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings = offsets
        .chunks_exact(2)
        .map(|n| {
            let end = at + usize::from(u16::from_le_bytes(n.try_into().unwrap()));
            let s = &bytes[at..end];
            at = end;
            s
        })
        .collect::<Vec<_>>();
    let assets = Assets::load(&index, &archive, installer, &strings, &exe).unwrap();
    let notices = vec![
        Notice {
            category: 33,
            category_kind: 33,
            character: 2,
            actor: b"Dog\0".to_vec(),
            business_kind: None,
        },
        Notice {
            category: 2,
            category_kind: 2,
            character: 7,
            actor: b"Battleship\0".to_vec(),
            business_kind: Some(0),
        },
    ];
    for notice in &notices {
        println!(
            "{}",
            String::from_utf8(
                assets
                    .text(notice)
                    .unwrap()
                    .iter()
                    .map(|b| b + 32)
                    .collect()
            )
            .unwrap()
        );
    }
    assert_eq!(View::category(&notices, Some([200., 6.])), Some(33));
    assert_eq!(View::category(&notices, Some([200., 18.])), Some(2));
    #[cfg(feature = "capture")]
    if let Ok(path) = std::env::var("OPENMT2001_NOTIFICATION_CAPTURE") {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let panel = assets.prepare(&renderer, &notices, Some(1)).unwrap();
        let mut background = Surface::solid();
        background.rect([0., 0., 640., 480.], [0xff283c48; 4]);
        let background = renderer
            .prepare(
                &[background.draw()],
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
            .unwrap();
        crate::capture_test::capture_overlay(&renderer, &background, &panel, &path);
    }
}
