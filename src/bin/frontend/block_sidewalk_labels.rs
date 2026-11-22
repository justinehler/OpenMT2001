//! Original 44faf0/455970 full-name pavement meshes. Font5 is the supplied
//! Copperplate18boldp atlas; 455780 emits ground glyphs before 455970 scales,
//! centers and rotates four copies. This differs from the GUI's base+3 title.
use crate::ui::Surface;
use openmt2001::{archive::ArchiveIndex, board::Block, render::{PreparedScene, Renderer, SurfaceMode}, texture::TextureSet};
use std::{fs, path::Path};

#[cfg(test)]
const PAVEMENT_HALF: f32 = 41.;
const EDGE_ORIGIN: f32 = 35.5;
const LETTER_SCALE: f32 = 0.25;
const LETTER_Y: f32 = 0.5;
// Presentation-only separation for modern float depth interpolation. Native
//455970 source glyphs and pavement are both Y=.5; retain those exact meshes.
const PRESENTATION_LIFT: f32 = 0.002;

struct Glyph {
    page: usize,
    points: [[f32; 2]; 4],
    uv: [[f32; 2]; 4],
}

fn glyphs(tab: &[u8], encoded: &[u8]) -> Result<(Vec<Glyph>, f32, f32), String> {
    // Validate the original 44-byte records, then follow world callback455780
    // directly. Unlike screen callback4554e0, it has no half-pixel offset.
    openmt2001::font::Font::parse(tab)?;
    let mut x = 0i32;
    let mut quads = Vec::with_capacity(encoded.len());
    let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
    for &glyph in encoded {
        let at = 4 + usize::from(glyph) * 44;
        let record = tab.get(at..at + 44).ok_or("pavement glyph outside font")?;
        let metric = |at| i32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        let width = metric(32);
        let top = metric(40);
        let bottom = top.checked_add(metric(36)).ok_or("pavement glyph height overflow")?;
        let right = x.checked_add(width).ok_or("pavement glyph width overflow")?;
        let points = [[x as f32, bottom as f32], [right as f32, bottom as f32],
            [right as f32, top as f32], [x as f32, top as f32]];
        low = low.min(x as f32);
        high = high.max(right as f32);
        quads.push(Glyph { page: usize::from(glyph >= 95), points,
            uv: std::array::from_fn(|i| std::array::from_fn(|axis| {
                let at = (i * 2 + axis) * 4;
                f32::from_le_bytes(record[at..at + 4].try_into().unwrap())
            })) });
        x = x.checked_add(width - 1).ok_or("pavement glyph advance overflow")?;
    }
    let half_width = (high - low) * 0.5;
    // 455a2d..455a65: cap X scale at .25; shrink widths exceeding60 units.
    let scale = (30. / half_width).min(LETTER_SCALE);
    Ok((quads, half_width, scale))
}

fn edge_point(center: [f32; 2], side: usize, x: f32, z: f32) -> [f32; 3] {
    // 455970 rotates the source mesh 0/90/180/270 degrees; identity scene
    // expansion4f2029 then negates source Z. Cardinal forms avoid trig noise.
    let [x, z] = match side {
        0 => [x, -z],
        1 => [z, x],
        2 => [-x, z],
        _ => [-z, -x],
    };
    [center[0] + x, LETTER_Y, center[1] + z]
}

pub struct Assets {
    pages: Vec<Surface>,
    object_colors: Vec<Vec<u32>>,
    centers: Vec<(usize, [f32; 2])>,
    spans: Vec<LabelSpan>,
}

struct LabelSpan {
    block: usize,
    page: usize,
    vertices: std::ops::Range<usize>,
}

#[derive(Default)]
pub struct FadeState {
    colors: Option<[u32; 2]>,
    alpha: Vec<u8>,
    pages: Option<Vec<Surface>>,
    specular: Vec<Vec<[f32; 3]>>,
}

/// 4b3245..4b3321 fades the category's four sidewalk copies over the last
/// 50 scene-depth units. Its distance source is the category object +0x118.
fn distance_alpha(depth: f32, factor: f32) -> u8 {
    crate::category_distance::Fade::from_factor(factor).pavement_alpha(depth)
}

/// Host I/O boundary: one palette source is shared with the worker and scene.
pub fn load_style(image: &openmt2001::pe::PeImage<'_>, installer: &Path)
    -> Result<openmt2001::board_style::Style, String> {
    let mut style = openmt2001::board_style::Style::from_v14(image)?;
    let source = fs::read_to_string(installer.join("Standard/fonts/board/usa/board.prm"))
        .map_err(|e| e.to_string())?;
    style.apply_source(&source)?;
    Ok(style)
}

impl Assets {
    pub fn load(index: &ArchiveIndex, archive: &[u8], installer: &Path,
        blocks: &[Block], style: &openmt2001::board_style::Style) -> Result<Self, String> {
        let entry = index.find("gamedata2/font640/textures.gtd")
            .ok_or("Original board lettering font missing")?;
        let font = TextureSet::parse(archive.get(entry.offset as usize
            ..(entry.offset + entry.length) as usize)
            .ok_or("Original board lettering font outside archive")?)
            .map_err(|e| e.to_string())?;
        // 455992 selects font5; 455c4e resolves its paired atlas handles.
        let mut pages = font.textures.get(10..12)
            .ok_or("Original board lettering font pages missing")?
            .iter().map(Surface::new).collect::<Vec<_>>();
        let tab = fs::read(installer.join("Standard/fonts/copperplate18boldp.tab"))
            .map_err(|e| e.to_string())?;
        let names = crate::tooltip::board_strings(installer)?;
        let district_colors = &style.text;
        let mut object_colors = vec![Vec::new(); pages.len()];
        let mut centers = Vec::new();
        let mut spans = Vec::new();
        for block in blocks.iter().filter(|block| block.flags & 1 == 0) {
            // 44fc48 passes the base text ID to455970. USA row0 is the
            // character map, so the installed base string is source row+1.
            let name_index = block.text_id.checked_sub(2242)
                .and_then(|i| i.checked_add(1))
                .ok_or("Original pavement text ID outside board language")?;
            let name = names.get(name_index as usize)
                .ok_or("Original full uppercase pavement name missing")?;
            if name.is_empty() { continue; }
            let (quads, half_width, scale) = glyphs(&tab, name)?;
            let group = centers.len();
            centers.push((block.id, block.center));
            let starts = pages.iter().map(|page| page.vertices.len()).collect::<Vec<_>>();
            for side in 0..4 {
                for glyph in &quads {
                    let page = &mut pages[glyph.page];
                    let start = page.vertices.len();
                    page.quad(glyph.points, glyph.uv, [0xffffffff; 4]);
                    let color = *district_colors.get(block.kind as usize)
                        .ok_or("pavement district outside original palette")?;
                    object_colors[glyph.page].extend([color; 6]);
                    for vertex in &mut page.vertices[start..] {
                        let x = vertex[0] * scale + -half_width * scale;
                        let z = EDGE_ORIGIN + vertex[1] * LETTER_SCALE;
                        vertex[..3].copy_from_slice(&edge_point(block.center, side, x, z));
                    }
                }
            }
            for (page, start) in starts.into_iter().enumerate() {
                let end = pages[page].vertices.len();
                if end > start {
                    spans.push(LabelSpan { block: group, page, vertices: start..end });
                }
            }
        }
        Ok(Self { pages, object_colors, centers, spans })
    }

    fn painted_pages(&self, colors: [u32; 2], alpha: &[u8]) -> Vec<Surface> {
        let mut pages = self.pages.clone();
        for (page, object_colors) in pages.iter_mut().zip(&self.object_colors) {
            for (vertex, &object_color) in page.vertices.iter_mut().zip(object_colors) {
                vertex[1] += PRESENTATION_LIFT;
                //4f2580 shades only white(-1) object diffuse. Board.prm
                //black district text takes the constant-color branch.
                let diffuse = if object_color == 0xffffffff { colors[0] } else { object_color };
                for (component, shift) in [16, 8, 0].into_iter().enumerate() {
                    vertex[3 + component] = ((diffuse >> shift) & 255) as f32 / 255.;
                }
            }
        }
        for span in &self.spans {
            let page = &mut pages[span.page];
            let opacity = alpha.get(span.block).copied().unwrap_or(255);
            page.alpha[span.vertices.clone()].fill(opacity);
            if opacity == 0 {
                // Native 4b32e1/4b3321 skips the whole label object beyond
                // far. Degenerate its retained batched triangles so invisible
                // glyphs cannot write depth on the portable static pipeline.
                let source = page.vertices[span.vertices.start];
                let point = [source[0], source[1], source[2]];
                for vertex in &mut page.vertices[span.vertices.clone()] {
                    vertex[..3].copy_from_slice(&point);
                }
            }
        }
        pages
    }

    fn change_alpha(&self, pages: &mut [Surface], old: &[u8], next: &[u8]) {
        for span in &self.spans {
            let opacity = next[span.block];
            if old.get(span.block) == Some(&opacity) { continue; }
            let page = &mut pages[span.page];
            page.alpha[span.vertices.clone()].fill(opacity);
            if opacity == 0 {
                let source = page.vertices[span.vertices.start];
                let point = [source[0], source[1], source[2]];
                for vertex in &mut page.vertices[span.vertices.clone()] {
                    vertex[..3].copy_from_slice(&point);
                }
            } else if old.get(span.block) == Some(&0) {
                for (vertex, original) in page.vertices[span.vertices.clone()].iter_mut()
                    .zip(&self.pages[span.page].vertices[span.vertices.clone()]) {
                    vertex[..3].copy_from_slice(&[
                        original[0], original[1] + PRESENTATION_LIFT, original[2]
                    ]);
                }
            }
        }
    }

    fn specular(&self, colors: [u32; 2]) -> Vec<Vec<[f32; 3]>> {
        self.object_colors.iter().map(|vertices| vertices.iter()
            .map(|&color| if color == 0xffffffff { rgb(colors[1]) } else { [0.; 3] })
            .collect()).collect()
    }

    pub fn prepare(&self, renderer: &Renderer, scene: &openmt2001::render_scene::Scene)
        -> Result<(PreparedScene, usize), String> {
        let colors = scene.ground_shading()?;
        let pages = self.painted_pages(colors, &[]);
        let specular = self.specular(colors);
        let mut draws = pages.iter().map(Surface::draw).collect::<Vec<_>>();
        for (draw, specular) in draws.iter_mut().zip(&specular) {
            draw.mode = SurfaceMode::StaticLit;
            draw.specular = Some(specular);
        }
        let len = draws.len();
        Ok((renderer.prepare(&draws, scene.camera, scene.fog)?, len))
    }

    pub fn update(&self, renderer: &Renderer, prepared: &mut PreparedScene,
        previous: &mut FadeState, scene: &openmt2001::render_scene::Scene,
        camera: openmt2001::flyby::CameraSample, distance: u8) -> Result<(), String> {
        let colors = scene.ground_shading()?;
        let factor = crate::people_scene::Policy::distance_factor(u32::from(distance))?;
        let view = openmt2001::camera::scene_view_with_up(camera.eye, camera.target, camera.up);
        let frustum = crate::selection::CategoryFrustum::new(camera)?;
        let alpha = self.centers.iter().map(|&(id, [x, z])| -> Result<u8, String> {
            if !frustum.visible(id, [x, z], scene.category_model_heights.get(&id).copied())? {
                return Ok(0);
            }
            let depth = openmt2001::camera::translated_scene_view(view, [x, 0., z])[11];
            Ok(distance_alpha(depth, factor))
        }).collect::<Result<Vec<_>, _>>()?;
        if previous.colors != Some(colors) || previous.alpha != alpha {
            if previous.colors != Some(colors) || previous.pages.is_none() {
                previous.pages = Some(self.painted_pages(colors, &alpha));
                previous.specular = self.specular(colors);
            } else if let Some(pages) = previous.pages.as_mut() {
                self.change_alpha(pages, &previous.alpha, &alpha);
            }
            for (index, page) in previous.pages.as_ref()
                .ok_or("sidewalk fade pages absent after repaint")?.iter().enumerate() {
                let mut draw = page.draw();
                draw.mode = SurfaceMode::StaticLit;
                draw.specular = Some(&previous.specular[index]);
                renderer.update_surface(prepared, index, &draw, scene.camera, scene.fog)?;
            }
            previous.colors = Some(colors);
            previous.alpha = alpha;
        } else {
            for index in 0..self.pages.len() {
                renderer.update_camera(prepared, index, scene.camera, scene.fog)?;
            }
        }
        Ok(())
    }
}

fn rgb(color: u32) -> [f32; 3] {
    [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.)
}

#[cfg(all(test, feature = "capture"))]
pub(crate) fn capture_marvin(
    scene: &openmt2001::render_scene::Scene,
    report: &crate::block_information::Report,
    path: &str,
) {
    let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
    let executable = read("OPENMT2001_EXE");
    let directory = read("OPENMT2001_ARCHIVE_DIR");
    let archive = read("OPENMT2001_ARCHIVE_BIN");
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
    let installer = parameter.parent().unwrap().parent().unwrap();
    let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
    let board = openmt2001::board::Board::from_v14(&image).unwrap();
    let block = board.blocks.iter().find(|block| block.id == 23).unwrap();
    // The v1.4 base-language table has the global IDs used by GUI rows;
    // installer USA strings are an older table with different indexing.
    let exe_path = std::path::PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
    let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
    let off = fs::read(language.join("tycoon.off")).unwrap();
    let bin = fs::read(language.join("tycoon.bin")).unwrap();
    let mut at = 0;
    let strings = off.chunks_exact(2).map(|length| {
        let length = u16::from_le_bytes(length.try_into().unwrap()) as usize;
        let text = &bin[at..at + length];
        at += length;
        text
    }).collect::<Vec<_>>();
    let localized = crate::tooltip::localized_strings(installer, &strings).unwrap();
    let strings = localized.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let gui = crate::locator::Assets::load(&index, &archive, installer, &strings, &executable).unwrap();
    let labels = Assets::load(&index, &archive, installer, &board.blocks, &load_style(&image, installer).unwrap()).unwrap();
    let mut city = scene.clone();
    let [x, z] = block.center;
    let camera = openmt2001::flyby::CameraSample {
        eye: [x - 18., 125., z + 110.], target: [x, 0., z],
        up: [0., 1., 0.], vertical_fov: 45.,
    };
    city.reframe(camera).unwrap();
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let (mut prepared_labels, _) = labels.prepare(&renderer, &city).unwrap();
    labels.update(&renderer, &mut prepared_labels, &mut FadeState::default(),
        &city, camera, 35).unwrap();
    let (panel, _) = gui.property_panel(&renderer, report, 23, &crate::utility_report::View::default()).unwrap();
    crate::capture_test::capture_scene_world_overlays(&renderer, &city,
        &[&prepared_labels], &[&panel], path);
    eprintln!("Original Marvin title/pavement capture {path}; block center {:?}", block.center);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_category_depth_fades_sidewalk_names() {
        // 4b3245..4b331f stores f32 near/far thresholds and truncates the
        // 50-unit alpha ramp through 50e928 under the original FPCW.
        assert_eq!(distance_alpha(150., 0.), 255);
        assert_eq!(distance_alpha(175., 0.), 127);
        assert_eq!(distance_alpha(200., 0.), 0);
        assert_eq!(distance_alpha(201., 0.), 0);
        let factor = crate::people_scene::Policy::distance_factor(35).unwrap();
        assert_eq!(distance_alpha(220., factor), 255);
        assert_eq!(distance_alpha(245., factor), 127);
        assert_eq!(distance_alpha(270., factor), 0);
        let factor = crate::people_scene::Policy::distance_factor(100).unwrap();
        assert_eq!(distance_alpha(450., factor), 255);
        assert_eq!(distance_alpha(475., factor), 127);
        assert_eq!(distance_alpha(500., factor), 0);
    }
    #[test]
    fn distant_label_skip_restores_original_world_geometry() {
        let mut page = Surface::solid();
        page.quad([[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [[0., 0.]; 4], [0xffffffff; 4]);
        let assets = Assets {
            pages: vec![page], object_colors: vec![vec![0xffffffff; 6]],
            centers: vec![(0, [0., 0.])],
            spans: vec![LabelSpan { block: 0, page: 0, vertices: 0..6 }],
        };
        let mut painted = assets.painted_pages([0xffffffff; 2], &[255]);
        let near = painted[0].vertices.iter().map(|v| v[..3].to_vec()).collect::<Vec<_>>();
        assets.change_alpha(&mut painted, &[255], &[0]);
        assert!(painted[0].alpha.iter().all(|&value| value == 0));
        assert!(painted[0].vertices.iter().all(|vertex| &vertex[..3] == near[0].as_slice()));
        assets.change_alpha(&mut painted, &[0], &[127]);
        assert!(painted[0].alpha.iter().all(|&value| value == 127));
        assert_eq!(painted[0].vertices.iter().map(|v| v[..3].to_vec()).collect::<Vec<_>>(), near);
    }
    #[test]
    #[ignore = "requires original v1.4 4b3230 label-alpha oracle"]
    fn matches_native_sidewalk_alpha_and_initial_gate() {
        let fixture = fs::read("work/frontend/native-sidewalk-alpha.bin").unwrap();
        assert_eq!(&fixture[..8], b"MTALPH01");
        let count = u32::from_le_bytes(fixture[8..12].try_into().unwrap()) as usize;
        assert_eq!(fixture.len(), 12 + count * 16);
        for row in fixture[12..].chunks_exact(16) {
            let detail = u32::from_le_bytes(row[..4].try_into().unwrap());
            let depth = f32::from_le_bytes(row[4..8].try_into().unwrap());
            let flags = u32::from_le_bytes(row[8..12].try_into().unwrap());
            let native_color = u32::from_le_bytes(row[12..16].try_into().unwrap());
            if flags == 1 {
                // Bit1 is the native per-frame scene/frustum visibility result.
                // Portable projection clips this category; the dynamic bit is
                // not represented as a separate V2 object state.
                assert_eq!(native_color, 0);
                continue;
            }
            let factor = crate::people_scene::Policy::distance_factor(detail).unwrap();
            let alpha = distance_alpha(depth, factor);
            let portable_color = if flags & 1 != 0 && alpha != 0 {
                (u32::from(alpha) << 24) | 0x345678
            } else { 0 };
            assert_eq!(portable_color, native_color,
                "Draw Distance {detail}, depth {depth}, category visibility bits {flags}");
        }
    }
    #[test]
    #[ignore = "requires original455970/455780 mesh observations"]
    fn matches_original_full_label_meshes() {
        let fixture = fs::read(std::env::var("OPENMT2001_NATIVE_SIDEWALK_LABELS").unwrap()).unwrap();
        assert_eq!(&fixture[..8], b"MTPAVL02");
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let exe = fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let colors = load_style(&image, installer).unwrap().text;
        assert_eq!(colors[1], 0xff000000, "USA Oriental black lettering");
        assert_eq!(colors[6], 0xff000000, "last USA Ventnor color overrides the earlier white entry");
        assert_eq!(colors[11], 0xff000000, "USA utility black lettering");
        let tab = fs::read(installer.join("Standard/fonts/copperplate18boldp.tab")).unwrap();
        let names = crate::tooltip::board_strings(installer).unwrap();
        let word = |at| u32::from_le_bytes(fixture[at..at + 4].try_into().unwrap());
        assert_eq!(word(8), 52);
        let mut at = 12;
        for id in 0..52 {
            assert_eq!(word(at), id);
            let text_id = word(at + 4);
            assert_eq!(image.at(0x5385a0 + id * 88, 4).unwrap(), text_id.to_le_bytes());
            let length = word(at + 8) as usize;
            at += 12;
            let encoded = &fixture[at..at + length];
            at += length;
            assert_eq!(encoded, names[(text_id - 2242 + 1) as usize]);
            if id == 23 {
                assert_eq!(encoded.iter().map(|b| b + 32).collect::<Vec<_>>(), b"MARVIN GARDENS");
                assert_eq!(names[(text_id - 2242 + 4) as usize].iter().map(|b| b + 32).collect::<Vec<_>>(), b"MARVIN");
            }
            let (quads, half, scale) = glyphs(&tab, encoded).unwrap();
            let count = word(at) as usize;
            at += 4;
            assert_eq!(count, quads.len() * 4 * 4);
            let (positions, uvs, colors) = (at, at + count * 12, at + count * 20);
            for side in 0..4 {
                for (n, glyph) in quads.iter().enumerate() {
                    for point in 0..4 {
                        let vertex = (side * quads.len() + n) * 4 + point;
                        let [x, y] = glyph.points[point];
                        let actual = edge_point([0.; 2], side,
                            x * scale + -half * scale, EDGE_ORIGIN + y * LETTER_SCALE);
                        let native: [f32; 3] = std::array::from_fn(|axis| {
                            f32::from_bits(word(positions + vertex * 12 + axis * 4))
                        });
                        // 4f2029 identity expansion negates mesh Z. Native
                        // cardinal rotations retain tiny x87 trig residuals.
                        let native = [native[0], native[1], -native[2]];
                        for axis in 0..3 {
                            assert!((actual[axis] - native[axis]).abs() < 0.00005,
                                "block{id} side{side} glyph{n} point{point}: {actual:?} vs {native:?}");
                        }
                        for axis in 0..2 {
                            assert_eq!(glyph.uv[point][axis].to_bits(), word(uvs + vertex * 8 + axis * 4));
                        }
                        assert_eq!(word(colors + vertex * 4), 0xffffffff,
                            "455970 passes white diffuse to the mesh callback");
                    }
                }
            }
            at += count * 24;
        }
        assert_eq!(at, fixture.len());
    }
    #[test]
    fn four_labels_stay_on_pavement_top() {
        let center = [13., -7.];
        for side in 0..4 {
            let point = edge_point(center, side, 0., EDGE_ORIGIN);
            assert_eq!(point[1], LETTER_Y);
            let distance = (point[0] - center[0]).abs().max((point[2] - center[1]).abs());
            assert_eq!(distance, EDGE_ORIGIN);
            assert!(distance < PAVEMENT_HALF);
        }
        assert!(edge_point(center, 0, 1., EDGE_ORIGIN)[0] > center[0]);
        assert!(edge_point(center, 1, 1., EDGE_ORIGIN)[2] > center[1]);
        assert!(edge_point(center, 2, 1., EDGE_ORIGIN)[0] < center[0]);
        assert!(edge_point(center, 3, 1., EDGE_ORIGIN)[2] < center[1]);
    }
    #[test]
    #[ignore = "requires original board and font assets"]
    fn original_names_have_ground_geometry() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        let pavement = index.find("gamedata2/NEW_building stuff/pavement.tm").unwrap();
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[pavement.offset as usize..(pavement.offset + pavement.length) as usize]
        ).unwrap();
        let positions = mesh.positions(0).unwrap();
        let top = positions.iter().map(|point| point[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(top.to_bits(), 0.5_f32.to_bits(), "original pavement top");
        let strip_x = positions.iter().filter(|point| point[1] == top && point[0] > 0.)
            .map(|point| point[0]).collect::<Vec<_>>();
        let inner = strip_x.iter().copied().fold(f32::INFINITY, f32::min);
        let outer = strip_x.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert_eq!((inner, outer), (35., 41.), "original pavement strip edges");
        assert_eq!(PAVEMENT_HALF, outer);
        assert_eq!(LETTER_Y, top, "native455780/455970 lettering is on pavement top");
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let assets = Assets::load(&index, &archive, installer, &board.blocks, &load_style(&image, installer).unwrap()).unwrap();
        let counts = assets.pages.iter().map(|page| page.vertices.len()).collect::<Vec<_>>();
        eprintln!("Sidewalk glyph vertices {counts:?}");
        assert!(counts.iter().sum::<usize>() > 52 * 4 * 6);
        for page in &assets.pages {
            assert!(page.vertices.iter().all(|v| v[1] == LETTER_Y));
        }
    }
}
