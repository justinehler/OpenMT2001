//! Original4b22e8 result title draw: font4, center X=320, top Y=340,
//! white RGB with the 4b1eb0 controller's alpha byte. Text comes from the
//! original language table via4dad30/4b17e0, not from embedded English.
use super::ResultPresentation;
use crate::ui::{self, Surface};
use openmt2001::{archive::ArchiveIndex, render::{PreparedScene, Renderer}, texture::TextureSet};
use std::{fs, path::Path};

pub struct Assets {
    pages: Vec<Surface>,
    tab: Vec<u8>,
    credit_heading_pages: Vec<Surface>,
    credit_heading_tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
}

impl Assets {
    pub fn load(index: &ArchiveIndex, archive: &[u8], installer: &Path, language: &Path)
        -> Result<Self, String> {
        let entry = index.find("gamedata2/font640/textures.gtd")
            .ok_or("result font archive missing")?;
        let set = TextureSet::parse(archive.get(entry.offset as usize..(entry.offset + entry.length) as usize)
            .ok_or("result font archive range")?).map_err(|e| e.to_string())?;
        // Native4b230b selects font4. The original font640 pair for font4 is
        // the same pair used by the reconstructed menu's font4 presentation.
        let pages = set.textures.get(8..10).ok_or("result font4 pages missing")?
            .iter().map(Surface::new).collect();
        let tab = fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
            .map_err(|e| e.to_string())?;
        openmt2001::font::Font::parse(&tab)?;
        // Native4b26ef selects font3 for the row heading;4b270a selects
        // font4 for each of its five possible names.
        let credit_heading_pages = set.textures.get(6..8)
            .ok_or("result font3 pages missing")?.iter().map(Surface::new).collect();
        let credit_heading_tab = fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
            .map_err(|e| e.to_string())?;
        openmt2001::font::Font::parse(&credit_heading_tab)?;
        let off = fs::read(language.join("tycoon.off")).map_err(|e| e.to_string())?;
        let bin = fs::read(language.join("tycoon.bin")).map_err(|e| e.to_string())?;
        if !off.len().is_multiple_of(2) { return Err("result language offsets truncated".into()); }
        let mut at = 0usize;
        let mut strings = Vec::with_capacity(off.len() / 2);
        for length in off.chunks_exact(2) {
            let end = at.checked_add(usize::from(u16::from_le_bytes(length.try_into().unwrap())))
                .ok_or("result language length overflow")?;
            strings.push(bin.get(at..end).ok_or("result language entry range")?.to_vec());
            at = end;
        }
        if at != bin.len() { return Err("result language trailing bytes".into()); }
        Ok(Self { pages, tab, credit_heading_pages, credit_heading_tab, strings })
    }

    /// Produces a 640x480 overlay to draw after the world and actor meshes.
    /// The original title is absent until its result-script identifier exists.
    /// Caller supplies the current 50 Hz result-controller alpha.
    pub fn prepare(&self, renderer: &Renderer, result: &ResultPresentation, alpha: u8)
        -> Result<Option<PreparedScene>, String> {
        let Some(surfaces) = self.surfaces(result, alpha)? else { return Ok(None); };
        self.prepare_surfaces(renderer, &surfaces).map(Some)
    }

    /// Original4b26ec draws the current credit record at Y220 heading and
    /// Y240,260,280,300,320 names, with the controller's shared alpha.
    pub fn prepare_credit(&self, renderer: &Renderer, result: &ResultPresentation,
        row: usize, alpha: u8) -> Result<Option<PreparedScene>, String> {
        let Some(surfaces) = self.credit_surfaces(result, row, alpha)? else { return Ok(None); };
        self.prepare_surfaces(renderer, &surfaces).map(Some)
    }

    fn prepare_surfaces(&self, renderer: &Renderer, surfaces: &[Surface])
        -> Result<PreparedScene, String> {
        let draws = surfaces.iter().map(Surface::draw).collect::<Vec<_>>();
        renderer.prepare(&draws, [
            2. / 640., 0., 0., 0.,
            0., -2. / 480., 0., 0.,
            0., 0., 1., 0.,
            -1., 1., 0., 1.,
        ], None)
    }

    fn surfaces(&self, result: &ResultPresentation, alpha: u8)
        -> Result<Option<Vec<Surface>>, String> {
        let Some(id) = result.text_id() else { return Ok(None); };
        if alpha == 0 { return Ok(None); }
        let glyphs = self.strings.get(id as usize).ok_or("result text language ID")?;
        let mut pages = self.pages.clone();
        centered_line(&mut pages, &self.tab, glyphs, 340., alpha)?;
        Ok(Some(pages))
    }

    fn credit_surfaces(&self, result: &ResultPresentation, row: usize, alpha: u8)
        -> Result<Option<Vec<Surface>>, String> {
        if alpha == 0 { return Ok(None); }
        let record = result.credit_lines.get(row).ok_or("result credit row index")?;
        let mut heading = self.credit_heading_pages.clone();
        if let Some(id) = record.heading_id {
            let glyphs = self.strings.get(id as usize).ok_or("result credit heading ID")?;
            centered_line(&mut heading, &self.credit_heading_tab, glyphs, 220., alpha)?;
        }
        let mut names = self.pages.clone();
        for (index, glyphs) in record.names.iter().enumerate() {
            if index >= 5 { return Err("original credit row has more than five names".into()); }
            centered_line(&mut names, &self.tab, glyphs, 240. + index as f32 * 20., alpha)?;
        }
        heading.extend(names);
        Ok(Some(heading))
    }
}

fn centered_line(pages: &mut [Surface], tab: &[u8], glyphs: &[u8], y: f32, alpha: u8)
    -> Result<(), String> {
    let width = ui::text_width(tab, glyphs, 1.)?;
    if width > 640 { return Err("result text exceeds native screen width".into()); }
    // Native4b21a4/4b2230/4b2342 halves signed width with SAR before
    // subtracting from320; Font::layout applies the D3D8 half-pixel offset.
    let left = (320 - width / 2) as f32;
    ui::text(pages, tab, glyphs, [left, y, 640., 480.], false,
        (u32::from(alpha) << 24) | 0x00ff_ffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmt2001::outcome_notice::Sequence;

    #[test]
    #[ignore = "requires original v1.4 font atlas, language and optional GPU"]
    fn original_result_titles_layout_and_capture() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let assets = Assets::load(&index, &archive, installer, &language).unwrap();
        for (sequence, identifier) in [(Sequence::Win, 1), (Sequence::Lose, 2), (Sequence::Complete, 3)] {
            let mut result = ResultPresentation::new(sequence, (0, 1));
            result.cut_scene_identifier = Some(identifier);
            let surfaces = assets.surfaces(&result, 128).unwrap().unwrap();
            let vertices = surfaces.iter().flat_map(|page| &page.vertices).collect::<Vec<_>>();
            assert!(!vertices.is_empty());
            let left = vertices.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min);
            let right = vertices.iter().map(|v| v[0]).fold(f32::NEG_INFINITY, f32::max);
            assert!((left + right - 639.).abs() <= 2., "result title {identifier} was not centered");
            assert!(vertices.iter().all(|v| v[1] > 320. && v[1] < 390.));
            assert!(surfaces.iter().flat_map(|s| &s.alpha).all(|&a| a == 128));
            #[cfg(feature = "capture")]
            if let Ok(folder) = std::env::var("OPENMT2001_RESULT_TEXT_CAPTURE") {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                let prepared = assets.prepare(&renderer, &result, 254).unwrap().unwrap();
                crate::capture_test::capture_standalone(&renderer, &prepared,
                    &format!("{folder}/result-title-{identifier}.png"));
            }
        }
    }

    #[test]
    #[ignore = "requires original v1.4 credits, fonts, language and optional GPU"]
    fn original_complete_credit_rows_layout_and_capture() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let assets = Assets::load(&index, &archive, installer, &language).unwrap();
        let mut result = ResultPresentation::new(Sequence::Complete, (0, 1));
        let mut state = 0x1234_5678;
        result.credit_lines = super::super::credit_lines(&image, &mut state).unwrap();
        result.credit_rows = result.credit_lines.len();
        assert_eq!(result.credit_rows, 32);
        for row in [0, 1, 14, 24, 31] {
            let surfaces = assets.credit_surfaces(&result, row, 128).unwrap().unwrap();
            let vertices = surfaces.iter().flat_map(|page| &page.vertices).collect::<Vec<_>>();
            assert!(!vertices.is_empty(), "credit row {row}");
            assert!(vertices.iter().all(|v| v[0] > 0. && v[0] < 640. && v[1] > 210. && v[1] < 350.));
            assert!(surfaces.iter().flat_map(|s| &s.alpha).all(|&a| a == 128));
            #[cfg(feature = "capture")]
            if let Ok(folder) = std::env::var("OPENMT2001_RESULT_CREDIT_CAPTURE") {
                let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
                let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                let prepared = assets.prepare_credit(&renderer, &result, row, 254).unwrap().unwrap();
                crate::capture_test::capture_standalone(&renderer, &prepared,
                    &format!("{folder}/complete-credit-{row:02}.png"));
            }
        }
    }
}
