//! Original Navigator category map,43d290/43dd90; paused inspection host.
#[path = "navigator_upper.rs"]
pub(crate) mod upper;
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    board::Block,
    render::{PreparedScene, Renderer},
};
pub struct Navigator {
    pub bounds: [f32; 4],
    surfaces: Vec<Surface>,
    upper_shells: Vec<Vec<Surface>>,
    upper_backing_color: u32,
    ///Retained real primitive products; containing Primary graph stays closed.
    native_primitives: crate::intro::camera_live::primitive_factory::Primitives,
    arrow: Surface,
    ownership: Surface,
    blocks: Vec<Block>,
}
#[derive(Clone, Copy)]
pub struct Owner {
    colour: u32,
    remaining: i32,
    current: bool,
}
/// Original43d577/585 gates by category+4/+5 and43d691 resolves signed
/// owner through the retained roster, independently of compact Board indices.
pub fn runtime_owners(
    categories: &[[u8; 0x17c]; 52],
    players: &openmt2001::player_manager::Manager,
    current_player: u32,
    image: &openmt2001::pe::PeImage<'_>,
) -> Result<Vec<Option<Owner>>, String> {
    categories
        .iter()
        .map(|row| {
            let owner = row[5] as i8;
            if row[4] == 0 || owner == 0 {
                return Ok(None);
            }
            let player = players
                .players
                .get(
                    usize::try_from(i16::from(owner) - 1)
                        .map_err(|_| "Navigator negative owner")?,
                )
                .ok_or("Navigator owner leaves roster")?;
            let character = u32::from_le_bytes(
                player
                    .bytes
                    .get(0x150..0x154)
                    .ok_or("Navigator player character missing")?
                    .try_into()
                    .unwrap(),
            ) as usize;
            Ok(Some(Owner {
                colour: crate::ownership_badges::style(image, Some(character))?.0,
                remaining: i32::from_le_bytes(row[0x7c..0x80].try_into().unwrap()),
                current: current_player == player.reference,
            }))
        })
        .collect()
}
#[cfg(test)]
pub fn saved_owners(
    save: &openmt2001::save::SaveFile<'_>,
    image: &openmt2001::pe::PeImage<'_>,
) -> Result<Vec<Option<Owner>>, String> {
    let badges = crate::ownership_badges::Badge::from_save(save)?;
    let current = crate::saved_current_player::normal_load(save)?.map(|p| p.slot + 1);
    let table = save
        .sections
        .iter()
        .find(|s| s.tag == 4)
        .ok_or("Navigator category table")?
        .payload;
    badges
        .iter()
        .enumerate()
        .map(|(i, badge)| {
            let row = &table[0x9600 + i * 0x8c..];
            if row[4] == 0 || row[5] == 0 {
                return Ok(None);
            }
            Ok(Some(Owner {
                colour: crate::ownership_badges::style(image, badge.character)?.0,
                remaining: badge.remaining,
                current: current == Some(usize::from(row[5])),
            }))
        })
        .collect()
}
impl Navigator {
    pub(crate) fn process_primitives_mut(
        &mut self,
    ) -> &mut crate::intro::camera_live::primitive_factory::Primitives {
        &mut self.native_primitives
    }

    #[cfg(test)]
    pub(crate) fn native_primitives(
        &self,
    ) -> &crate::intro::camera_live::primitive_factory::Primitives {
        &self.native_primitives
    }
    #[cfg(test)]
    pub(crate) fn native_primitives_mut(
        &mut self,
    ) -> &mut crate::intro::camera_live::primitive_factory::Primitives {
        &mut self.native_primitives
    }
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        blocks: &[Block],
        executable: &[u8],
    ) -> Result<Self, String> {
        let (mut surfaces, regions) = ui::panel_base(
            index,
            archive,
            "gamedata2/brand_new_panels/new_nav_top.tm",
            &[1],
        )?;
        //43d010 resolves backing separately;43d0b0 fills it at depth0.5.
        // Part0 is that full rectangle, not border artwork. Keep only part1
        // textured; otherwise its opaque gray texture covers the palette fill.
        let e = index
            .find("gamedata2/brand_new_panels/new_nav_top.tm")
            .ok_or("F model absent")?;
        let mesh = openmt2001::mesh::CachedMesh::parse(
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or("F model range")?,
        )?;
        if mesh.frames == 0 || mesh.frames > u16::MAX as usize {
            return Err("F supplied model frame count".into());
        }
        let cpu_model = crate::intro::camera_live::panel_models::CpuModel::parsed(
            crate::intro::camera_live::ui_graph::PanelId::F,
            e,
            &mesh,
        )?;
        let native_primitives =
            crate::intro::camera_live::primitive_factory::Primitives::navigator(
                executable, cpu_model,
            )?;
        let upper_shells = (0..mesh.frames)
            .map(|frame| {
                ui::panel_frame_base(
                    index,
                    archive,
                    "gamedata2/brand_new_panels/new_nav_top.tm",
                    &[1],
                    frame,
                )
                .map(|v| v.0)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let image = openmt2001::pe::PeImage::parse(executable)?;
        let color = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        let mut backing = Surface::solid();
        backing.rect(
            *regions.first().ok_or("Navigator backing missing")?,
            [color; 4],
        );
        surfaces.insert(0, backing);
        let bounds = *regions.get(2).ok_or("Navigator main window missing")?;
        let origin = origin(bounds);
        //4c0e9b loads generic textures at689f90;43db40 reads689fc0 (index12).
        let entry = index
            .find("gamedata2/GenericTextures/textures.gtd")
            .ok_or("Navigator arrow textures missing")?;
        let textures = openmt2001::texture::TextureSet::parse(
            archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Navigator arrow texture range")?,
        )
        .map_err(|e| e.to_string())?;
        let mut map = Surface::new(
            textures
                .textures
                .first()
                .ok_or("Navigator base texture missing")?,
        );
        for block in blocks.iter().filter(|b| b.flags & 1 == 0) {
            let [l, t, r, b] = rectangle(origin, block.center, block.size);
            // Original category19 base rectangle is transparent; other colors use runtime+94.
            let color = if block.id == 19 {
                0
            } else {
                block.pavement_color | 0xff000000
            };
            map.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [color; 4],
            );
        }
        surfaces.push(map);
        let arrow = Surface::new(
            textures
                .textures
                .get(12)
                .ok_or("Navigator arrow texture missing")?,
        );
        Ok(Self {
            bounds,
            surfaces,
            upper_shells,
            upper_backing_color: color,
            native_primitives,
            arrow,
            ownership: Surface::new(
                textures
                    .textures
                    .get(13)
                    .ok_or("Navigator ownership texture missing")?,
            ),
            blocks: blocks.to_vec(),
        })
    }
    #[cfg(test)]
    pub fn prepare(&self, renderer: &Renderer) -> Result<PreparedScene, String> {
        self.prepare_with_ownership(renderer, &[])
    }
    pub fn prepare_with_ownership(
        &self,
        renderer: &Renderer,
        owners: &[Option<Owner>],
    ) -> Result<PreparedScene, String> {
        let arrow = self.arrow_surface([0., 0.], 0., 0);
        let ownership = self.ownership_surface(owners, 0);
        let mut draws: Vec<_> = self.surfaces.iter().map(Surface::draw).collect();
        draws.push(ownership.draw());
        draws.push(arrow.draw());
        renderer.prepare(&draws, overlay_matrix(), None)
    }
    fn ownership_surface(&self, owners: &[Option<Owner>], tick: u32) -> Surface {
        let mut surface = self.ownership.clone();
        for block in &self.blocks {
            if let Some(owner) = owners.get(block.id).copied().flatten() {
                surface.quad(
                    ownership_quad(origin(self.bounds), block.center, block.size)
                        .map(|p| [p[0], p[1]]),
                    [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                    [ownership_colour(block.id, owner.colour, owner.remaining, owner.current, tick);
                        4],
                );
            }
        }
        surface
    }
    fn arrow_surface(&self, eye: [f32; 2], angle: f32, tick: u32) -> Surface {
        let mut arrow = self.arrow.clone();
        arrow.quad(
            arrow_quad(origin(self.bounds), eye, angle, tick).map(|p| [p[0], p[1]]),
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        arrow
    }
    #[cfg(test)]
    pub fn update_arrow(
        &self,
        renderer: &Renderer,
        scene: &mut PreparedScene,
        eye: [f32; 2],
        angle: f32,
    ) -> Result<(), String> {
        let arrow = self.arrow_surface(eye, angle, 0);
        renderer.update_surface(
            scene,
            self.surfaces.len() + 1,
            &arrow.draw(),
            overlay_matrix(),
            None,
        )
    }
    /// Both original43d691 ownership colors and43db43 arrow size use822d40.
    /// Update only these two retained surfaces; preserve base city geometry.
    pub fn update_ownership_arrow(
        &self,
        renderer: &Renderer,
        scene: &mut PreparedScene,
        eye: [f32; 2],
        angle: f32,
        tick: u32,
        owners: &[Option<Owner>],
    ) -> Result<(), String> {
        let ownership = self.ownership_surface(owners, tick);
        renderer.update_surface(
            scene,
            self.surfaces.len(),
            &ownership.draw(),
            overlay_matrix(),
            None,
        )?;
        let arrow = self.arrow_surface(eye, angle, tick);
        renderer.update_surface(
            scene,
            self.surfaces.len() + 1,
            &arrow.draw(),
            overlay_matrix(),
            None,
        )
    }
    /// Inspection picking uses1280x960; original UI geometry uses640x480.
    pub fn pick_inspection(&self, blocks: &[Block], point: [i32; 2]) -> Option<(usize, [i32; 2])> {
        let point = point.map(|v| v / 2);
        self.pick(blocks, point).map(|category| (category, point))
    }
    fn pick(&self, blocks: &[Block], point: [i32; 2]) -> Option<usize> {
        let origin = origin(self.bounds);
        blocks
            .iter()
            .position(|b| b.flags & 1 == 0 && hit(origin, b.center, b.size, point))
    }
}
fn overlay_matrix() -> [f32; 16] {
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
    ]
}
/// Original43d593 geometry; the bottom retains extended top precision.
fn ownership_quad(origin: [f32; 2], center: [f32; 2], size: [f32; 2]) -> [[f32; 3]; 4] {
    use openmt2001::x87::Sum;
    let extent = size.map(|v| {
        let mut n = Sum::from_f32(v).unwrap();
        n.mul_f32(0.075).unwrap();
        n.to_f32()
    });
    let mut coordinates = [Sum::default(); 2];
    for axis in 0..2 {
        let mut n = Sum::from_f32(center[axis]).unwrap();
        n.mul_f32(if axis == 0 { 0.1 } else { -0.1 }).unwrap();
        n.add_f32(origin[axis]).unwrap();
        let mut half = Sum::from_f32(extent[axis]).unwrap();
        half.mul_f32(-0.5).unwrap();
        n.add(half);
        coordinates[axis] = n;
    }
    let l = coordinates[0].to_f32();
    let t = coordinates[1].to_f32();
    coordinates[1].add_f32(extent[1]).unwrap();
    let b = coordinates[1].to_f32();
    let mut right = Sum::from_f32(l).unwrap();
    right.add_f32(extent[0]).unwrap();
    let r = right.to_f32();
    [[l, t, 0.5], [l, b, 0.5], [r, b, 0.5], [r, t, 0.5]]
}
fn ownership_colour(category: usize, colour: u32, remaining: i32, current: bool, tick: u32) -> u32 {
    if category == 19 || (remaining <= 24 && tick & 15 > 8) {
        return 0;
    }
    if remaining <= 24 || !current {
        return colour;
    }
    let scale = ((tick & 31) as i32 - 15).unsigned_abs() as f32 * 0.03125 + 0.5;
    let channel = |shift: u32| {
        let mut n = openmt2001::x87::Sum::from_i32(((colour >> shift) & 255) as i32);
        n.mul_f32(scale).unwrap();
        n.ftol_low_word() as u32
    };
    0xff000000 | channel(16) << 16 | channel(8) << 8 | channel(0)
}
fn origin([l, t, r, b]: [f32; 4]) -> [f32; 2] {
    [
        ((f64::from(l) + f64::from(r)) * 0.5) as f32,
        ((f64::from(t) + f64::from(b)) * 0.5) as f32,
    ]
}
/// Original drawn extent uses0.12, while original hit extent uses0.1.
fn rectangle(origin: [f32; 2], center: [f32; 2], size: [f32; 2]) -> [f32; 4] {
    let extent = size.map(|v| (f64::from(v) * f64::from(0.12f32)) as f32);
    let left = (f64::from(center[0]) * f64::from(0.1f32) + f64::from(origin[0])
        - f64::from(extent[0]) * 0.5) as f32;
    let top = (f64::from(center[1]) * f64::from(-0.1f32) + f64::from(origin[1])
        - f64::from(extent[1]) * 0.5) as f32;
    [
        left,
        top,
        (f64::from(left) + f64::from(extent[0])) as f32,
        (f64::from(top) + f64::from(extent[1])) as f32,
    ]
}
fn hit(origin: [f32; 2], center: [f32; 2], size: [f32; 2], point: [i32; 2]) -> bool {
    let width = f64::from(size[0]) * f64::from(0.1f32);
    let height = (f64::from(size[1]) * f64::from(0.1f32)) as f32;
    let left = f64::from(center[0]) * f64::from(0.1f32) + f64::from(origin[0]) - width * 0.5;
    let top = (f64::from(center[1]) * f64::from(-0.1f32) + f64::from(origin[1])
        - f64::from(height) * 0.5) as f32;
    left.trunc() as i32 <= point[0]
        && (left + width).trunc() as i32 >= point[0]
        && top.trunc() as i32 <= point[1]
        && (f64::from(top) + f64::from(height)).trunc() as i32 >= point[1]
}

/// Original43db40. Tick is simulation time; paused inspection passes zero.
fn arrow_quad(origin: [f32; 2], eye: [f32; 2], angle: f32, tick: u32) -> [[f32; 3]; 4] {
    let pulse = (f64::from((tick & 31) as i32 - 15).abs() * f64::from(1f32 / 15.)).powi(2) * 4.;
    let a = -f64::from(angle);
    let mut x =
        (f64::from(eye[0]) * f64::from(0.1f32) + f64::from(origin[0]) + a.cos() * pulse) as f32;
    let mut y = f64::from(eye[1]) * f64::from(-0.1f32) + f64::from(origin[1]) + a.sin() * pulse;
    if f64::from(x) - f64::from(origin[0]) < -60. {
        x = (f64::from(origin[0]) - 60.) as f32;
    }
    if f64::from(x) - f64::from(origin[0]) > 60. {
        x = (f64::from(origin[0]) + 60.) as f32;
    }
    y = y.clamp(f64::from(origin[1]) - 50., f64::from(origin[1]) + 50.);
    let rotation = f64::from(std::f32::consts::FRAC_PI_4) - f64::from(angle);
    let sin = f64::from(rotation.sin() as f32) * 20.;
    let cos = rotation.cos() * 20.;
    let negative_sin = (sin * -0.5) as f32;
    let negative_cos = (cos * -0.5) as f32;
    let positive_sin = f64::from(sin as f32) * 0.5;
    [
        [
            (f64::from(x) + sin * -0.5) as f32,
            (y - f64::from(negative_cos)) as f32,
            0.5,
        ],
        [
            (f64::from(x) + cos * 0.5) as f32,
            (y - f64::from(negative_sin)) as f32,
            0.5,
        ],
        [
            (f64::from(x) + positive_sin) as f32,
            (y - cos * 0.5) as f32,
            0.5,
        ],
        [
            (f64::from(x) + f64::from(negative_cos)) as f32,
            (y - f64::from(positive_sin as f32)) as f32,
            0.5,
        ],
    ]
}

#[cfg(test)]
mod tests {
    // Geometry oracle cases reuse one real asset owner; no placeholder primitive.
    fn supplied_navigator() -> super::Navigator {
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        super::Navigator::load(&index, &archive, &board.blocks, &executable).unwrap()
    }
    #[test]
    #[ignore = "requires supplied original upper Navigator mesh"]
    fn original_upper_navigator_regions() {
        let directory = std::fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
        let archive = std::fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let e = index
            .find("gamedata2/brand_new_panels/new_nav_top.tm")
            .unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[e.offset as usize..(e.offset + e.length) as usize],
        )
        .unwrap();
        let regions = openmt2001::ui_mesh::regions(&mesh, 0).unwrap();
        assert_eq!(
            regions
                .iter()
                .map(|r| (r.name.as_str(), r.part))
                .collect::<Vec<_>>(),
            [
                ("NAV_BACKING_MAIN", 0),
                ("NAV_BORDER01", 1),
                ("P_NAVIGATOR_MAINWINDOW", 2)
            ]
        );
        assert_eq!(
            regions[0].bounds,
            [
                530.3883666992188,
                355.96807861328125,
                637.0919189453125,
                446.5613708496094
            ]
        );
        assert_eq!(mesh.parts[0].indices.len(), 12); // full backing: two triangles
        assert_eq!(mesh.parts[1].indices.len(), 48); // border: four edge quads
        assert_eq!(
            mesh.texture_names[mesh.parts[1].texture as usize],
            "1NEW_MAYA_GREY_BORDER.TT"
        );
        let executable = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        assert_eq!(image.at(0x5362c0, 4).unwrap(), &0xbb000a28u32.to_le_bytes());
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let navigator =
            super::Navigator::load(&index, &archive, &board.blocks, &executable).unwrap();
        assert_eq!(
            navigator.surfaces.len(),
            3,
            "backing, border and map keep dynamic indices stable"
        );
        // Real asset-load boundary: one retained source-owned F construction.
        use crate::intro::camera_live::{
            primitive_factory::{DescriptorIdentity, Phase, Step},
            ui_graph::PanelId,
        };
        let primitive = &navigator.native_primitives;
        let panel = primitive.panels.get(&0x58cdb0).unwrap();
        assert_eq!(panel.phase, Phase::Bound);
        assert_eq!(
            panel.descriptor,
            Some(DescriptorIdentity::Archive(PanelId::F))
        );
        assert_eq!(primitive.panels.len(), 1);
        assert_eq!(primitive.texts.len(), 1);
        assert_eq!(
            primitive
                .steps
                .iter()
                .filter(|s| **s == Step::NavigatorSuffix(0x58cdb0))
                .count(),
            1
        );
        assert_eq!(primitive.cells().byte(0x58cdb0 + 0x184).unwrap(), 1);
        let model = primitive.resources.model(PanelId::F).unwrap();
        assert_eq!(model.source.path, e.path);
        assert_eq!(
            (model.source.offset, model.source.length),
            (e.offset, e.length)
        );
        assert_eq!(model.frames.len(), mesh.frames);
        assert_eq!(
            primitive.cells().word(0x58cdb0 + 0x180).unwrap(),
            mesh.frames as u32
        );
        assert!(primitive.require_process_graph().is_err());
        assert!(primitive.require_text_layout(panel.caption).is_err());
        eprintln!(
            "retained F construction frames={} panel_count={} caption_count={} phase={:?}",
            model.frames.len(),
            primitive.panels.len(),
            primitive.texts.len(),
            panel.phase
        );

        let backing = &navigator.surfaces[0];
        assert_eq!((backing.width, backing.height), (1, 1));
        assert_eq!(backing.pixels, [255; 4]);
        assert_eq!(
            backing.alpha, [187; 6],
            "original publish color alpha survives submission"
        );
        let [l, t, r, b] = regions[0].bounds;
        let corners = [[l, t], [l, b], [r, b], [r, t]];
        for (vertex, corner) in backing.vertices.iter().zip([0, 1, 2, 0, 2, 3]) {
            assert_eq!(vertex[..3], [corners[corner][0], corners[corner][1], 0.5]);
            assert_eq!(vertex[3..6], [0., 10. / 255., 40. / 255.]);
        }
        assert_eq!(
            backing.draw().mode,
            openmt2001::render::SurfaceMode::Overlay
        );
    }
    #[test]
    #[ignore = "private saved cities and GPU"]
    fn saved_ownership_gpu() {
        use openmt2001::{
            board::Board,
            pe::PeImage,
            render_scene::{self, OriginalAssets, Request, Source},
        };
        let read = |name| std::fs::read(openmt2001::fixtures::path(name)).unwrap();
        let exe = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let image = PeImage::parse(&exe).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let index =
            openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let nav = super::Navigator::load(&index, &archive, &board.blocks, &exe).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(target_os = "windows") {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(openmt2001::render::Renderer::new(descriptor)).unwrap();
        // Empty/fresh ownership still retains the arrow update slot.
        let mut empty = nav.prepare(&renderer).unwrap();
        nav.update_arrow(&renderer, &mut empty, [0., 0.], 0.)
            .unwrap();
        for (i, name) in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"]
            .iter()
            .enumerate()
        {
            let bytes = read(name);
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let owners = super::saved_owners(&save, &image).unwrap();
            assert_eq!(owners.len(), 52);
            // Both supplied saves have no category leases; businesses have
            // separate ownership and must not be substituted here.
            assert!(owners.iter().all(Option::is_none));
            let mut prepared = nav.prepare_with_ownership(&renderer, &owners).unwrap();
            nav.update_arrow(&renderer, &mut prepared, [-220., -30.1], -1.5)
                .unwrap();
            let scene = render_scene::assemble(Request {
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
            #[cfg(feature = "capture")]
            if let Ok(prefix) = std::env::var("OPENMT2001_NAVIGATOR_CAPTURE") {
                crate::capture_test::capture(
                    &renderer,
                    Some(&prepared),
                    &scene,
                    &format!("{prefix}-{i}.png"),
                );
                let mut synthetic = vec![None; 52];
                for (category, character) in (0..10).enumerate() {
                    synthetic[category] = Some(super::Owner {
                        colour: crate::ownership_badges::style(&image, Some(character))
                            .unwrap()
                            .0,
                        remaining: 25,
                        current: category == 0,
                    });
                }
                for (category, owner) in synthetic.iter_mut().enumerate().skip(46) {
                    *owner = Some(super::Owner {
                        colour: crate::ownership_badges::style(&image, Some(category % 10))
                            .unwrap()
                            .0,
                        remaining: 24,
                        current: true,
                    });
                }
                let mut overlay = nav.prepare_with_ownership(&renderer, &synthetic).unwrap();
                for tick in [0, 9, 15, 31] {
                    nav.update_ownership_arrow(
                        &renderer,
                        &mut overlay,
                        [-220., -30.1],
                        -1.5,
                        tick,
                        &synthetic,
                    )
                    .unwrap();
                }
                crate::capture_test::capture(
                    &renderer,
                    Some(&overlay),
                    &scene,
                    &format!("{prefix}-{i}-synthetic-owners.png"),
                );
            }
            let _ = (i, scene);
        }
    }
    #[test]
    #[ignore = "private original Navigator ownership oracle"]
    fn matches_native_navigator_ownership() {
        let mut navigator = supplied_navigator();
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_NAVIGATOR_OWNERSHIP")).unwrap();
        assert_eq!(&b[..8], b"MTNOWN01");
        let w = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let f = |at| f32::from_bits(w(at));
        let executable = std::fs::read(openmt2001::fixtures::path("EXE")).unwrap();
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let colours: Vec<_> = (0..10)
            .map(|character| {
                crate::ownership_badges::style(&image, Some(character))
                    .unwrap()
                    .0
            })
            .collect();
        let mut at = 12;
        for case in 0..w(8) {
            let quad = super::ownership_quad(
                [f(at), f(at + 4)],
                [f(at + 8), f(at + 12)],
                [f(at + 16), f(at + 20)],
            );
            let count = u32::from(w(at + 28) != 0 && w(at + 32) != 0);
            assert_eq!(count, w(at + 52), "gate {case}");
            let colour = super::ownership_colour(
                w(at + 24) as usize,
                w(at + 36),
                w(at + 40) as i32,
                w(at + 44) != 0,
                w(at + 48),
            );
            let category = w(at + 24) as usize;
            let mut categories = [[0; 0x17c]; 52];
            categories[category][4] = w(at + 28) as u8;
            categories[category][5] = w(at + 32) as u8;
            categories[category][0x7c..0x80].copy_from_slice(&w(at + 40).to_le_bytes());
            let character = colours
                .iter()
                .position(|&colour| colour == w(at + 36))
                .unwrap();
            let mut player = openmt2001::player_manager::Player {
                reference: 0x10040000,
                bytes: vec![0; 0x1e50],
                auxiliaries: vec![],
            };
            player.bytes[0x150..0x154].copy_from_slice(&(character as u32).to_le_bytes());
            let current = if w(at + 44) != 0 { player.reference } else { 0 };
            let manager = openmt2001::player_manager::Manager {
                bytes: [0; 0x28],
                players: vec![player],
            };
            let owners = super::runtime_owners(&categories, &manager, current, &image).unwrap();
            assert_eq!(
                owners.iter().filter(|owner| owner.is_some()).count(),
                count as usize
            );
            let mut blocks = board.blocks.clone();
            let target = blocks
                .iter_mut()
                .find(|block| block.id == category)
                .unwrap();
            target.center = [f(at + 8), f(at + 12)];
            target.size = [f(at + 16), f(at + 20)];
            navigator.bounds = [f(at), f(at + 4), f(at), f(at + 4)];
            navigator.blocks = blocks;
            let surface = navigator.ownership_surface(&owners, w(at + 48));
            assert_eq!(
                surface.vertices.len(),
                count as usize * 6,
                "production gate {case}"
            );
            if count != 0 {
                for (vertex, index) in surface.vertices.iter().zip([0, 1, 2, 0, 2, 3]) {
                    let point = quad[index];
                    assert_eq!(
                        vertex[..2],
                        point[..2],
                        "production native-ID geometry {case}"
                    );
                    assert_eq!(vertex[3], ((colour >> 16) & 255) as f32 / 255.);
                    assert_eq!(vertex[4], ((colour >> 8) & 255) as f32 / 255.);
                    assert_eq!(vertex[5], (colour & 255) as f32 / 255.);
                }
                assert!(
                    surface
                        .alpha
                        .iter()
                        .all(|&alpha| alpha == (colour >> 24) as u8)
                );
            }
            at += 56;
            if count != 0 {
                for n in quad.into_iter().flatten() {
                    assert_eq!(n.to_bits(), w(at), "geometry {case} offset {at}");
                    at += 4;
                }
                assert_eq!(colour, w(at), "colour {case}");
                at += 4;
            }
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "requires original Navigator arrow oracle"]
    fn matches_native_navigator_arrow() {
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_NAVIGATOR_ARROW")).unwrap();
        assert_eq!(&b[..8], b"MTNARR01");
        let w = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let f = |at| f32::from_bits(w(at));
        let mut at = 12;
        for case in 0..w(8) {
            let q = super::arrow_quad(
                [f(at), f(at + 4)],
                [f(at + 8), f(at + 12)],
                f(at + 16),
                w(at + 20),
            );
            at += 24;
            for value in q.into_iter().flatten() {
                assert_eq!(
                    value.to_bits(),
                    w(at),
                    "case{case} offset{at}: {value} vs {}",
                    f(at)
                );
                at += 4;
            }
        }
        assert_eq!(at, b.len());
    }

    #[test]
    #[ignore = "requires original Navigator geometry oracle, executable and archive assets"]
    fn matches_native_navigator() {
        let mut navigator = supplied_navigator();
        let b = std::fs::read(openmt2001::fixtures::path("NATIVE_NAVIGATOR")).unwrap();
        assert_eq!(&b[..8], b"MTNAV001");
        let w = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let f = |at| f32::from_bits(w(at));
        let mut at = 12;
        for case in 0..w(8) {
            let origin = [f(at), f(at + 4)];
            let center = [f(at + 8), f(at + 12)];
            let size = [f(at + 16), f(at + 20)];
            at += 24;
            let [l, t, r, bot] = super::rectangle(origin, center, size);
            let actual = [[l, t, 0.5], [l, bot, 0.5], [r, bot, 0.5], [r, t, 0.5]]
                .into_iter()
                .flatten()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>();
            assert_eq!(actual, b[at..at + 48], "quad{case}");
            at += 48;
            for _ in 0..7 {
                let point = [w(at) as i32, w(at + 4) as i32];
                assert_eq!(
                    super::hit(origin, center, size, point),
                    w(at + 8) == 0,
                    "hit{case} {point:?}"
                );
                navigator.bounds = [origin[0], origin[1], origin[0], origin[1]];
                let block = openmt2001::board::Block {
                    id: 0,
                    name: String::new(),
                    text_id: 0,
                    kind: 0,
                    center,
                    size,
                    pavement_color: 0,
                    flags: 0,
                    street_sides: 0,
                };
                assert_eq!(
                    navigator
                        .pick_inspection(&[block], point.map(|v| v * 2))
                        .map(|(category, ui)| {
                            assert_eq!(ui, point);
                            category
                        }),
                    if w(at + 8) == 0 { Some(0) } else { None },
                    "inspection host hit{case} {point:?}"
                );
                at += 12;
            }
        }
        assert_eq!(at, b.len());
    }
}
