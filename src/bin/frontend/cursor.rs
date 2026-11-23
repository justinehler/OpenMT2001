//! Original default, right-drag, pan and enabled-menu-hover images. The native host owns
//! cursor presentation; no OS cursor API enters shared assets or simulation.
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
pub struct Images([Image; 4]);
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
        let images = [(0, [5, 1]), (2, [8, 8]), (3, [5, 0]), (1, [8, 1])]
            .into_iter()
            .map(|(i, hotspot)| {
                // Original004391e0 sets dimensions from the registered image and
                // these hotspots;004390a0 subtracts them from pointer coordinates.
                let image = set.textures.get(i).ok_or("missing inspection cursor")?;
                let mut rgba = loader_pixels(image);
                // Original cursor3 hotspotY=-1: image starts one pixel below pointer.
                // Native custom cursors require a nonnegative hotspot, so prepend one clear row.
                if i == 3 {
                    let mut padded = vec![0; image.width as usize * 4];
                    padded.extend(rgba);
                    rgba = padded;
                }
                Ok(Image {
                    width: u16::try_from(image.width).map_err(|_| "cursor width")?,
                    height: u16::try_from(image.height + u32::from(i == 3))
                        .map_err(|_| "cursor height")?,
                    rgba,
                    hotspot,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self(images.try_into().map_err(|_| "cursor count")?))
    }
    pub fn create(self, event_loop: &ActiveEventLoop) -> Result<[CustomCursor; 4], String> {
        let cursors = self
            .0
            .into_iter()
            .map(|image| {
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
            .collect::<Result<Vec<_>, String>>()?;
        cursors.try_into().map_err(|_| "cursor count".into())
    }
}
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original pan cursor oracle and archive"]
    fn matches_native_pan_cursor() {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
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
    #[ignore = "requires original archive and native cursor setter observations"]
    fn matches_native_inspection_cursor() {
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
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
