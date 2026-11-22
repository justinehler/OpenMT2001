//! Original player-comparison control rules (426ba0/427210/425900).
#[derive(Clone, Debug)]
pub struct State {
    pub mode: usize,
    pub enabled: [bool; 9],
    pub open: bool,
}
impl State {
    pub fn new(objective: i32) -> Self {
        let mut result = Self {
            mode: 0,
            enabled: [true; 9],
            open: true,
        };
        result.enabled[2] = false;
        if (0..=6).contains(&objective) {
            result.enabled[7] = false;
            if objective != 1 {
                result.enabled[6] = false;
            }
            result.mode = match objective {
                1 => 6,
                3 => 3,
                4 | 5 => 4,
                6 => 8,
                _ => 0,
            };
        }
        result
    }
    pub fn cycle(&mut self, right: bool) {
        for _ in 0..9 {
            self.mode = (self.mode + if right { 10 } else { 8 }) % 9;
            if self.enabled[self.mode] {
                break;
            }
        }
    }
    pub fn label(&self) -> u32 {
        [1530, 1531, 1532, 1533, 1534, 1535, 1536, 1538, 1386][self.mode]
    }
}
/// Original swap order, including its tie and unordered-comparison behavior.
/// Input rows arrive in reverse manager order from4256a0.
pub fn order(amounts: &[f32]) -> Vec<usize> {
    let mut result: Vec<_> = (0..amounts.len()).collect();
    for i in (0..result.len()).rev() {
        for j in (0..i).rev() {
            if amounts[result[i]] > amounts[result[j]] {
                result.swap(i, j);
            }
        }
    }
    result
}
/// 489fe0 writes60 bytes beginning at86ce60 under tag10000;48a000 restores it.
pub fn saved_objective(save: &openmt2001::save::SaveFile<'_>) -> Result<i32, String> {
    let mut rows = save.sections.iter().filter(|s| s.tag == 0x10000);
    let row = rows
        .next()
        .ok_or("save has no comparison objective descriptor")?;
    if row.payload.len() != 60 || rows.next().is_some() {
        return Err("invalid saved objective descriptor".into());
    }
    Ok(i32::from_le_bytes(row.payload[..4].try_into().unwrap()))
}
#[derive(Clone)]
pub struct Data {
    pub objective: i32,
    /// Manager index then all nine settled values; reverse manager input order.
    pub rows: Vec<(usize, [f32; 9])>,
}
use openmt2001::player_metrics::Building;
impl Data {
    #[cfg(test)]
    pub fn verify_runtime(
        world: &openmt2001::level_startup::World,
        players: &[[u8; 440]],
        image: &openmt2001::pe::PeImage<'_>,
        scenario: &str,
        difficulty: usize,
    ) -> Self {
        let data = Self::from_runtime(world, players, image, 0x591318).unwrap();
        assert_eq!(
            data.rows.len(),
            players.iter().filter(|p| p[0xc] != 0).count()
        );
        for (slot, values) in &data.rows {
            assert_eq!(
                values[0].to_bits(),
                world.ledgers.players[*slot].cash.to_bits()
            );
        }
        assert_eq!(data.objective, world.objective.kind);
        if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_COMPARISON_ORACLE") {
            let mut bytes = b"MTCFRESH".to_vec();
            bytes.extend((players.len() as u32).to_le_bytes());
            bytes.extend(world.clock.hour().to_le_bytes());
            for p in players {
                bytes.extend(p);
            }
            for c in &world.categories {
                bytes.extend(c);
            }
            for b in &world.buildings {
                bytes.extend(b);
            }
            for b in &world.ledgers.businesses {
                bytes.extend(b);
            }
            bytes.extend((data.rows.len() as u32).to_le_bytes());
            for (slot, values) in &data.rows {
                bytes.extend((*slot as u32).to_le_bytes());
                for v in values {
                    bytes.extend(v.to_le_bytes());
                }
            }
            std::fs::write(format!("{prefix}-{scenario}-{difficulty}.bin"), bytes).unwrap();
        }
        data
    }
    pub fn from_save(
        save: &openmt2001::save::SaveFile<'_>,
        image: &openmt2001::pe::PeImage<'_>,
        hour: u32,
    ) -> Result<Self, String> {
        let players: Vec<_> = save.players()?.iter().map(|p| *p.bytes).collect();
        let economy = save.economy()?;
        let businesses: Vec<_> = economy.businesses().map(|b| b.bytes.as_slice()).collect();
        let buildings: Vec<_> = save
            .buildings()?
            .into_iter()
            .map(|b| Building {
                category: b.category,
                slot: b.slot,
                bytes: b.bytes,
            })
            .collect();
        let categories = (0..52)
            .map(|i| Ok((save.category_owner(i)?, save.category_prestige(i)?)))
            .collect::<Result<Vec<_>, String>>()?;
        Self::from_records(
            &players,
            &businesses,
            &buildings,
            &categories,
            saved_objective(save)?,
            image,
            hour,
        )
    }
    pub fn from_runtime(
        world: &openmt2001::level_startup::World,
        players: &[[u8; 440]],
        image: &openmt2001::pe::PeImage<'_>,
        building_base: u32,
    ) -> Result<Self, String> {
        Ok(Self {
            objective: world.objective.kind,
            rows: openmt2001::player_metrics::from_runtime(
                &world.city,
                players,
                image,
                world.clock.hour(),
                building_base,
            )?,
        })
    }
    fn from_records(
        players: &[[u8; 440]],
        businesses: &[&[u8]],
        buildings: &[Building<'_>],
        categories: &[(u8, f32)],
        objective: i32,
        image: &openmt2001::pe::PeImage<'_>,
        hour: u32,
    ) -> Result<Self, String> {
        Ok(Self {
            objective,
            rows: openmt2001::player_metrics::comparison(
                players, businesses, buildings, categories, image, hour,
            )?,
        })
    }
    pub fn values(&self, mode: usize) -> Vec<(usize, f32)> {
        self.rows.iter().map(|(i, v)| (*i, v[mode])).collect()
    }
}
pub struct View {
    pub data: Data,
    pub state: State,
    pub prepared: Option<openmt2001::render::PreparedScene>,
    hover: Option<usize>,
    pressed: Option<usize>,
    dirty: bool,
}
impl View {
    pub fn new(data: Data) -> Self {
        Self {
            state: State::new(data.objective),
            data,
            prepared: None,
            hover: None,
            pressed: None,
            dirty: true,
        }
    }
    pub fn contains(&self, point: [i32; 2]) -> bool {
        let [x, y] = point.map(|v| v as f32 * 0.5);
        (4. ..=if self.state.open { 190.4 } else { 30.4 }).contains(&x)
            && (302. ..=472.4).contains(&y)
    }
    pub fn pressed(&self) -> bool {
        self.pressed.is_some()
    }
    pub fn pointer(&mut self, assets: &Assets, point: Option<[i32; 2]>) {
        let hover = point.and_then(|p| assets.button_at(&self.state, p));
        if hover != self.hover {
            self.hover = hover;
            self.dirty = true;
        }
    }
    pub fn mouse(&mut self, down: bool) -> Option<usize> {
        let mut activated = None;
        if down {
            self.pressed = self.hover;
        } else if let Some(button) = self.pressed.take()
            && self.hover == Some(button)
        {
            activated = Some(button);
            match button {
                0 => self.state.open = !self.state.open,
                1 => self.state.cycle(false),
                2 => self.state.cycle(true),
                _ => {}
            }
        }
        self.dirty = true;
        activated
    }
    pub fn cancel_press(&mut self) {
        self.pressed = None;
        self.hover = None;
        self.dirty = true;
    }
    pub fn prepare(
        &mut self,
        assets: &Assets,
        renderer: &openmt2001::render::Renderer,
        players: &[[u8; 440]],
    ) -> Result<(), String> {
        if self.dirty || self.prepared.is_none() {
            self.prepared = Some(assets.prepare(
                renderer,
                &self.state,
                players,
                &self.data.values(self.state.mode),
                self.hover,
                self.pressed,
            )?);
            self.dirty = false;
        }
        Ok(())
    }
}
/// Original426ca0: side gradient is always visible; body fills appear at
/// completed opening frame9, before dynamic player rows and widgets.
fn background_palette(frame: usize, body: u32) -> Vec<(&'static str, [u32; 4])> {
    let mut result = vec![(
        "P_STATS_LEFT_VERT",
        [0xff909090, 0x7f909090, 0x7f909090, 0xff909090],
    )];
    if frame == 9 {
        result.extend([
            ("P_STATS_MODE_BACKING1", [body; 4]),
            ("P_STATS_STATS_BACKING", [body; 4]),
        ]);
    }
    result
}
pub struct Assets {
    base: [Vec<crate::ui::Surface>; 2],
    regions: [Vec<[f32; 4]>; 2],
    generic: Vec<crate::ui::Surface>,
    fonts: Vec<crate::ui::Surface>,
    tab: Vec<u8>,
    strings: Vec<Vec<u8>>,
    colors: [u32; 10],
    icons: [usize; 10],
}
impl Assets {
    #[cfg(test)]
    pub fn verify_fresh(
        &self,
        data: Data,
        players: &[[u8; 440]],
        scene: &openmt2001::render_scene::Scene,
        scenario: &str,
    ) {
        assert!(scene.saved_players.is_empty());
        let mut view = View::new(data);
        let initial = view.state.mode;
        #[cfg(feature = "capture")]
        if let Ok(prefix) = std::env::var("OPENMT2001_FRESH_COMPARISON_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer =
                pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
            for _ in 0..9 {
                view.prepare(self, &renderer, players).unwrap();
                if view.state.mode == initial || view.state.mode == 0 {
                    crate::capture_test::capture(
                        &renderer,
                        view.prepared.as_ref(),
                        scene,
                        &format!("{prefix}-{scenario}-{}.png", view.state.mode),
                    );
                }
                view.state.cycle(true);
                view.dirty = true;
            }
            view.state.open = false;
            view.dirty = true;
            view.prepare(self, &renderer, players).unwrap();
        }
        let _ = (&mut view, initial, players, scenario);
    }
    pub fn load(
        index: &openmt2001::archive::ArchiveIndex,
        archive: &[u8],
        installer: &std::path::Path,
        strings: &[&[u8]],
        exe: &[u8],
    ) -> Result<Self, String> {
        use crate::ui::{self, Surface};
        let parts: Vec<_> = (0..12).collect();
        let (mut closed, closed_regions) = ui::panel_frame_base(
            index,
            archive,
            "gamedata2/brand_new_panels/stats_panel.tm",
            &parts,
            0,
        )?;
        let (mut open, open_regions) = ui::panel_frame_base(
            index,
            archive,
            "gamedata2/brand_new_panels/stats_panel.tm",
            &parts,
            9,
        )?;
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index.find(name).ok_or("missing comparison asset")?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("comparison asset range".into())
        };
        let generic = openmt2001::texture::TextureSet::parse(payload(
            "gamedata2/GenericTextures640/textures.gtd",
        )?)
        .map_err(|e| e.to_string())?
        .textures
        .iter()
        .map(Surface::new)
        .collect();
        let fonts =
            openmt2001::texture::TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
                .map_err(|e| e.to_string())?
                .textures[6..8]
                .iter()
                .map(Surface::new)
                .collect();
        let image = openmt2001::pe::PeImage::parse(exe)?;
        let body = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        let mesh = openmt2001::mesh::CachedMesh::parse(payload(
            "gamedata2/brand_new_panels/stats_panel.tm",
        )?)?;
        for (frame, surfaces) in [(0, &mut closed), (9, &mut open)] {
            let regions = openmt2001::ui_mesh::regions(&mesh, frame)?;
            let mut fill = Surface::solid();
            for (name, colors) in background_palette(frame, body) {
                let region = regions
                    .iter()
                    .find(|r| r.name == name)
                    .ok_or("comparison background region")?;
                fill.rect(region.bounds, colors);
            }
            surfaces.push(fill);
        }
        let mut colors = [0; 10];
        let mut icons = [0; 10];
        for i in 0..10 {
            colors[i] =
                u32::from_le_bytes(image.at(0x541af0 + i as u32 * 40, 4)?.try_into().unwrap());
            icons[i] =
                u32::from_le_bytes(image.at(0x541b00 + i as u32 * 40, 4)?.try_into().unwrap())
                    as usize;
        }
        Ok(Self {
            base: [closed, open],
            regions: [closed_regions, open_regions],
            generic,
            fonts,
            tab: std::fs::read(installer.join("Standard/fonts/tahoma10bldnoantip.tab"))
                .map_err(|e| e.to_string())?,
            strings: strings.iter().map(|s| s.to_vec()).collect(),
            colors,
            icons,
        })
    }
    pub fn button_at(&self, state: &State, point: [i32; 2]) -> Option<usize> {
        let x = point[0] as f32 * 0.5;
        let y = point[1] as f32 * 0.5;
        [57, 65, 66]
            .into_iter()
            .filter(|i| state.open || *i == 57)
            .position(|i| {
                let [l, t, r, b] = self.regions[usize::from(state.open)][i];
                x >= l && x <= r && y >= t && y <= b
            })
    }
    pub fn prepare(
        &self,
        renderer: &openmt2001::render::Renderer,
        state: &State,
        players: &[[u8; 440]],
        rows: &[(usize, f32)],
        hover: Option<usize>,
        pressed: Option<usize>,
    ) -> Result<openmt2001::render::PreparedScene, String> {
        use crate::ui::{self, Surface};
        let bounds = &self.regions[usize::from(state.open)];
        let mut surfaces = self.base[usize::from(state.open)].clone();
        let mut solid = Surface::solid();
        let mut fonts = self.fonts.clone();
        let mut row_icons = Vec::new();
        let mut icon = |texture: usize, region: usize| -> Result<(), String> {
            let mut s = self
                .generic
                .get(texture)
                .ok_or("comparison texture index")?
                .clone();
            let [l, t, r, b] = bounds[region];
            s.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(s);
            Ok(())
        };
        icon(
            88 + if pressed == Some(0) {
                2
            } else if hover == Some(0) {
                1
            } else {
                0
            },
            57,
        )?;
        icon(if state.open { 71 } else { 72 }, 58)?;
        if state.open {
            icon(
                97 + if pressed == Some(1) {
                    2
                } else if hover == Some(1) {
                    1
                } else {
                    0
                },
                65,
            )?;
            icon(
                101 + if pressed == Some(2) {
                    2
                } else if hover == Some(2) {
                    1
                } else {
                    0
                },
                66,
            )?;
            // Original427154..4271cb, all registered consecutively at68ae28.
            icon([10, 142, 139, 141, 140, 277, 289, 299, 298][state.mode], 56)?;
            ui::text_fit(
                &mut fonts,
                &self.tab,
                &self.strings[state.label() as usize],
                bounds[67],
                false,
                0xffffffff,
            )?;
        }
        let values: Vec<_> = rows.iter().map(|r| r.1).collect();
        let order = order(&values);
        let greatest = order.first().map_or(0., |&i| values[i]);
        for slot in 0..6 {
            let color = if let Some(&row) = order.get(slot) {
                let (player, value) = rows[row];
                let p = players.get(player).ok_or("comparison player index")?;
                let character = u32::from_le_bytes(p[0x14c..0x150].try_into().unwrap()) as usize;
                let color = *self
                    .colors
                    .get(character)
                    .ok_or("comparison character index")?;
                icon(self.icons[character], 59 + slot)?;
                if state.open {
                    let mut text_bounds = bounds[13 + slot * 7];
                    let [l, t, _, b] = text_bounds;
                    let mut cash = self.generic[10].clone();
                    cash.quad(
                        [[l, t], [l, b], [l + 12., b], [l + 12., t]],
                        [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                        [0xffffffff; 4],
                    );
                    row_icons.push(cash);
                    text_bounds[0] += 13.;
                    let name = &p[0x154..];
                    let end = name
                        .iter()
                        .position(|b| *b == 0)
                        .ok_or("unterminated comparison name")?;
                    let name = name[..end]
                        .iter()
                        .map(|b| b.checked_sub(32).ok_or("comparison name glyph"))
                        .collect::<Result<Vec<_>, _>>()?;
                    ui::text_fit(&mut fonts, &self.tab, &name, text_bounds, false, 0xffffffff)?;
                    let integer =
                        openmt2001::x87::Sum::from_f32(value).map_or(0, |v| v.ftol_low_word());
                    let mut amount = Vec::new();
                    if matches!(state.mode, 0 | 3 | 4 | 5) {
                        if integer < 0 {
                            amount.push(b'-' - 32);
                        }
                        let left = self.strings[2244].first() == Some(&0x4c);
                        if left {
                            amount.extend_from_slice(&self.strings[2243]);
                        }
                        amount.extend(integer.wrapping_abs().to_string().bytes().map(|b| b - 32));
                        if !left {
                            amount.extend_from_slice(&self.strings[2243]);
                        }
                    } else {
                        amount.extend(integer.to_string().bytes().map(|b| b - 32));
                    }
                    // Original right alignment reserves trend-icon width. Static saved
                    // inspection has no observed previous update to fabricate a trend.
                    text_bounds[2] -= 8.;
                    ui::text_fit(
                        &mut fonts,
                        &self.tab,
                        &amount,
                        text_bounds,
                        true,
                        0xffffffff,
                    )?;
                    let mut bar = bounds[17 + slot * 7];
                    let fraction = if greatest > 0. {
                        value.max(0.) / greatest
                    } else {
                        0.
                    };
                    bar[2] = bar[0] + (bar[2] - bar[0]) * fraction;
                    let border = bounds[12 + slot * 7];
                    let width = f64::from(border[2] - border[0]).trunc();
                    let alpha = |x: f32| {
                        (((f64::from(border[2]) - f64::from(x)) * 128. / width + 127.).trunc()
                            as u32)
                            .wrapping_shl(24)
                            | color
                    };
                    solid.rect(
                        bar,
                        [alpha(bar[0]), alpha(bar[0]), alpha(bar[2]), alpha(bar[2])],
                    );
                }
                color
            } else {
                0x3f3f3f
            };
            if state.open {
                for part in 13..=16 {
                    let b = bounds[part + slot * 7];
                    let border = bounds[12 + slot * 7];
                    let width = f64::from(border[2] - border[0]).trunc();
                    let right = (((f64::from(border[2]) - f64::from(b[2])) * 128. / width + 127.)
                        .trunc() as u32)
                        .wrapping_shl(24)
                        | color;
                    solid.rect(b, [0xff000000 | color, 0xff000000 | color, right, right]);
                }
            }
        }
        surfaces.push(solid);
        surfaces.extend(row_icons);
        surfaces.extend(fonts);
        let draws: Vec<_> = surfaces.iter().map(Surface::draw).collect();
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
#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "private original comparison draw fixture"]
    fn matches_native_background_palette() {
        let bytes = std::fs::read(std::env::var("OPENMT2001_NATIVE_STATS_STYLE").unwrap()).unwrap();
        assert_eq!(&bytes[..8], b"MTSTATCL");
        let mut at = 8;
        fn word(bytes: &[u8], at: &mut usize) -> u32 {
            let v = u32::from_le_bytes(bytes[*at..*at + 4].try_into().unwrap());
            *at += 4;
            v
        }
        for _ in 0..word(&bytes, &mut at) {
            let frame = word(&bytes, &mut at) as usize;
            let actual = super::background_palette(frame, 0xbb000a28);
            assert_eq!(word(&bytes, &mut at) as usize, actual.len());
            for (name, colors) in actual {
                let length = word(&bytes, &mut at) as usize;
                assert_eq!(name.as_bytes(), &bytes[at..at + length]);
                at += length;
                assert_eq!(colors, std::array::from_fn(|_| word(&bytes, &mut at)));
            }
        }
        assert_eq!(at, bytes.len());
    }

    use super::*;
    #[test]
    #[ignore = "requires original saved comparison metrics"]
    fn matches_native_saved_comparison() {
        let b =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PLAYER_COMPARISON").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTCOMP01");
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 3);
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let mut at = 12;
        for name in [
            "OPENMT2001_SAVE_ONE",
            "OPENMT2001_SAVE_TWO",
            "OPENMT2001_COMPARISON_SYNTHETIC",
        ] {
            let bytes = std::fs::read(std::env::var(name).unwrap()).unwrap();
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            at += 32;
            assert_eq!(word(at) as usize, save.players().unwrap().len());
            at += 4;
            for _ in 0..36 {
                let hour = word(at);
                let mode = word(at + 4) as usize;
                let count = word(at + 8) as usize;
                at += 12;
                let expected: Vec<_> = (0..count)
                    .map(|i| (word(at + i * 8) as usize, word(at + i * 8 + 4)))
                    .collect();
                at += count * 8;
                let data = Data::from_save(&save, &image, hour).unwrap();
                let actual: Vec<_> = data
                    .values(mode)
                    .iter()
                    .map(|(i, v)| (*i, v.to_bits()))
                    .collect();
                assert_eq!(actual, expected, "{name} hour{hour} mode{mode}");
            }
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "requires original saved city and UI assets"]
    fn comparison_saved_panel_flow() {
        use openmt2001::{
            archive::ArchiveIndex,
            pe::PeImage,
            render::Renderer,
            render_scene::{OriginalAssets, Request, Source, assemble},
        };
        let read = |name| std::fs::read(std::env::var(name).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let image = PeImage::parse(&exe).unwrap();
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = exe_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("US/language/usa");
        let off = std::fs::read(language.join("tycoon.off")).unwrap();
        let bin = std::fs::read(language.join("tycoon.bin")).unwrap();
        let mut offset = 0;
        let strings: Vec<_> = off
            .chunks_exact(2)
            .map(|n| {
                let len = u16::from_le_bytes(n.try_into().unwrap()) as usize;
                let text = &bin[offset..offset + len];
                offset += len;
                text
            })
            .collect();
        let sun = std::path::PathBuf::from(std::env::var("OPENMT2001_SUNCOLS").unwrap());
        let assets = Assets::load(
            &index,
            &archive,
            sun.parent().unwrap().parent().unwrap(),
            &strings,
            &exe,
        )
        .unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(target_os = "windows") {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        for (i, name) in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"]
            .iter()
            .enumerate()
        {
            let bytes = read(name);
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let data = Data::from_save(&save, &image, save.clock().unwrap().hour()).unwrap();
            let mut view = View::new(data);
            let players: Vec<_> = save.players().unwrap().iter().map(|p| *p.bytes).collect();
            let scene = assemble(Request {
                build_file: &build,
                source: Source::Save(&bytes),
                original: Some(OriginalAssets {
                    executable: &exe,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: None,
                phase: None,
                camera: Some(
                    crate::navigation::Navigation::load(&index, &archive, &exe)
                        .unwrap()
                        .camera,
                ),
                width: 1280,
                height: 960,
                animated: false,
            })
            .unwrap();
            for mode in 0..9 {
                view.state.mode = mode;
                view.dirty = true;
                view.prepare(&assets, &renderer, &players).unwrap();
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_COMPARISON_CAPTURE") {
                    crate::capture_test::capture(
                        &renderer,
                        view.prepared.as_ref(),
                        &scene,
                        &format!("{prefix}-{i}-{mode}.png"),
                    );
                }
            }
            let _ = (&scene, i);
            let initial = view.state.mode;
            view.pointer(&assets, Some([350, 630]));
            view.mouse(true);
            view.pointer(&assets, Some([400, 630]));
            view.mouse(false);
            assert_eq!(view.state.mode, initial);
            view.pointer(&assets, Some([350, 630]));
            view.mouse(true);
            view.mouse(false);
            assert_ne!(view.state.mode, initial);
            view.pointer(&assets, Some([30, 630]));
            view.mouse(true);
            view.mouse(false);
            assert!(!view.state.open);
            assert!(!view.contains([200, 700]));
            assert!(view.contains([30, 700]));
            view.prepare(&assets, &renderer, &players).unwrap();
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_COMPARISON_CAPTURE") {
                crate::capture_test::capture(
                    &renderer,
                    view.prepared.as_ref(),
                    &scene,
                    &format!("{prefix}-{i}-closed.png"),
                );
            }
            view.mouse(true);
            view.cancel_press();
            view.mouse(false);
            assert!(!view.state.open);
        }
    }
    #[test]
    #[ignore = "requires original comparison control observations"]
    fn matches_native_comparison_controls() {
        let b =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_PLAYER_COMPARISON_CONTROLS").unwrap())
                .unwrap();
        assert_eq!(&b[..8], b"MTCMCTL1");
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let mut at = 12;
        for _ in 0..word(8) {
            let state = State::new(word(at) as i32);
            assert_eq!(state.mode, word(at + 4) as usize);
            assert_eq!(state.enabled.map(u8::from), b[at + 8..at + 17]);
            at += 17;
            for initial in 0..9 {
                for right in [false, true] {
                    let mut s = state.clone();
                    s.mode = initial;
                    s.cycle(right);
                    assert_eq!(s.mode, word(at) as usize);
                    at += 4;
                }
            }
        }
        let cases = word(at);
        at += 4;
        for _ in 0..cases {
            let n = word(at) as usize;
            at += 4;
            let values: Vec<_> = (0..n).map(|i| f32::from_bits(word(at + i * 4))).collect();
            at += n * 4;
            let expected: Vec<_> = (0..n).map(|i| word(at + i * 4) as usize).collect();
            at += n * 4;
            assert_eq!(order(&values), expected);
        }
        assert_eq!(at, b.len());
    }
}
