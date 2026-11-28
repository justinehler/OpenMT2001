//! In-game Options overlay. The supplied original recording places its four
//! rows in `main_menu.tm` MAIN_BUTT02..05, beneath the retained logo.
use crate::ui::Surface;
use openmt2001::{
    archive::ArchiveIndex,
    font::{Font, Layout},
    mesh::CachedMesh,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
    ui_button::{self, Primitive, State},
    ui_mesh,
};
use std::{fs, path::Path};

const VIEW: [f32; 16] = [
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
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Continue,
    Save,
    Settings,
    Quit,
}

pub const QUIT_CALLBACK: u32 = 0x49e940;

/// Original 49e970's abandonment prompt, distinct from desktop Quit.
pub fn quit_dialog() -> crate::human_dialog::Dialog {
    crate::human_dialog::Dialog {
        portrait: u32::MAX,
        title: 1763,
        body: 1764,
        buttons: [1636, 1637],
        button_count: 2,
        callback: QUIT_CALLBACK,
        timeout: false,
        handler_written: true,
        context_written: true,
        timeout_written: true,
        ..Default::default()
    }
}

pub struct Assets {
    base: Vec<Surface>,
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    labels: [Vec<u8>; 4],
    bounds: [[f32; 4]; 4],
}

// This loader's own canonical pose is X/-Y. Original UI CULL_CW rejects
// positive screen area; keep front-facing indices in their original order.
// Only authored main_menu.tm triangles pass here, not logo/button/UI quads.
fn options_mesh_triangles<'a>(
    positions: &'a [[f32; 3]],
    indices: &'a [usize],
) -> impl Iterator<Item = [usize; 3]> + 'a {
    indices.chunks_exact(3).filter_map(move |triangle| {
        let ids = [triangle[0], triangle[1], triangle[2]];
        let [a, b, c] = ids.map(|id| [positions[id][0], -positions[id][1]]);
        let area = (f64::from(b[0]) - f64::from(a[0])) * (f64::from(c[1]) - f64::from(a[1]))
            - (f64::from(b[1]) - f64::from(a[1])) * (f64::from(c[0]) - f64::from(a[0]));
        // Degenerates have no raster area; do not introduce an epsilon.
        (area.is_finite() && area < 0.).then_some(ids)
    })
}

impl Assets {
    #[cfg(test)]
    pub(crate) fn test_menu() -> Self {
        Self {
            base: Vec::new(),
            fonts: Vec::new(),
            tab: Vec::new(),
            labels: std::array::from_fn(|_| Vec::new()),
            bounds: [
                [0., 0., 10., 10.],
                [20., 0., 30., 10.],
                [40., 0., 50., 10.],
                [60., 0., 70., 10.],
            ],
        }
    }
    pub fn load(
        index: &ArchiveIndex,
        archive: &[u8],
        installer: &Path,
        language: &Path,
    ) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index
                .find(name)
                .ok_or_else(|| format!("missing options asset {name}"))?;
            archive
                .get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or_else(|| format!("options asset {name} outside archive"))
        };
        let mesh = CachedMesh::parse(payload("gamedata2/brand_new_panels/main_menu.tm")?)?;
        let regions = ui_mesh::regions(&mesh, 0)?;
        // Native49e750 indexes the five-word descriptors at543a68. Their
        // first words name MAIN_BUTT02..05, resolved by43af30 unchanged.
        let bounds: [[f32; 4]; 4] = ["MAIN_BUTT02", "MAIN_BUTT03", "MAIN_BUTT04", "MAIN_BUTT05"]
            .map(|name| {
                regions
                    .iter()
                    .find(|r| r.name == name)
                    .map(|r| r.bounds)
                    .ok_or_else(|| format!("original options row {name} missing"))
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "original options rows missing")?;
        let pos = mesh.positions(0)?;
        let uv = mesh.uvs();
        let mut base = Vec::new();
        // Native 43c3a0 submits the complete main_menu.tm parent model before
        // 49e900 paints the logo and 43ed20 paints its child buttons.
        for part in &mesh.parts {
            let tex = TextureSet::parse(payload(mesh.texture_names[part.texture as usize])?)
                .map_err(|e| e.to_string())?;
            let mut s = Surface::new(&tex.textures[0]);
            let indices = part.vertex_indices().collect::<Vec<_>>();
            for triangle in options_mesh_triangles(&pos, &indices) {
                for i in triangle {
                    s.vertices
                        .push([pos[i][0], -pos[i][1], 0.5, 1., 1., 1., uv[i][0], uv[i][1]]);
                    s.alpha.push(255);
                }
            }
            if !s.vertices.is_empty() {
                base.push(s);
            }
        }
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut logo = Surface::new(
            generic
                .textures
                .get(287)
                .ok_or("original Tycoon logo missing")?,
        );
        let [l, t, r, b] = regions[13].bounds;
        logo.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        base.push(logo);
        let fonts = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?
            .textures[8..10]
            .iter()
            .map(Surface::new)
            .collect();
        let tab = fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
            .map_err(|e| e.to_string())?;
        let off = fs::read(language.join("tycoon.off")).map_err(|e| e.to_string())?;
        let bin = fs::read(language.join("tycoon.bin")).map_err(|e| e.to_string())?;
        let mut cursor = 0usize;
        let mut rows = Vec::new();
        for length in off.chunks_exact(2) {
            let end = cursor + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            rows.push(
                bin.get(cursor..end)
                    .ok_or("options language row outside data")?,
            );
            cursor = end;
        }
        let localized = crate::tooltip::localized_strings(installer, &rows)?;
        // Native in-game menu strings: Continue, Save Game, Settings, Quit.
        let labels = [1678, 1680, 1679, 1682]
            .map(|i| {
                localized
                    .get(i)
                    .cloned()
                    .ok_or("original in-game Options label missing".to_owned())
            })
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "original in-game Options label count")?;
        Ok(Self {
            base,
            fonts,
            tab,
            labels,
            bounds,
        })
    }
    pub fn button(&self, pointer: [f32; 2]) -> Option<Button> {
        self.bounds
            .iter()
            .position(|&[l, t, r, b]| {
                pointer[0] >= l && pointer[0] <= r && pointer[1] >= t && pointer[1] <= b
            })
            .map(|i| {
                [
                    Button::Continue,
                    Button::Save,
                    Button::Settings,
                    Button::Quit,
                ][i]
            })
    }
    #[cfg(test)]
    pub fn bounds(&self) -> [[f32; 4]; 4] {
        self.bounds
    }
    #[cfg(test)]
    pub fn labels(&self) -> &[Vec<u8>; 4] {
        &self.labels
    }
    #[cfg(test)]
    pub fn prepare(
        &self,
        renderer: &Renderer,
        hovered: Option<Button>,
        pressed: Option<Button>,
    ) -> Result<PreparedScene, String> {
        self.prepare_message(renderer, hovered, pressed, None, false)
    }
    fn prepare_message(
        &self,
        renderer: &Renderer,
        hovered: Option<Button>,
        pressed: Option<Button>,
        message: Option<&str>,
        save_disabled: bool,
    ) -> Result<PreparedScene, String> {
        let mut buttons = Surface::solid();
        let colors = [[0, 192, 0], [0, 0, 192], [110, 4, 171], [185, 7, 187]];
        let actions = [
            Button::Continue,
            Button::Save,
            Button::Settings,
            Button::Quit,
        ];
        for (i, &bounds) in self.bounds.iter().enumerate() {
            for primitive in ui_button::background(
                bounds,
                colors[i],
                State {
                    flag_330: (!save_disabled || actions[i] != Button::Save)
                        && pressed == Some(actions[i])
                        && hovered == pressed,
                    flag_334: (!save_disabled || actions[i] != Button::Save)
                        && pressed == Some(actions[i])
                        && hovered == pressed,
                    flag_335: hovered == Some(actions[i])
                        && (!save_disabled || actions[i] != Button::Save),
                    flag_333: save_disabled && actions[i] == Button::Save,
                },
            )? {
                match primitive {
                    Primitive::Gradient { bounds, colors } => buttons.rect(bounds, colors),
                    Primitive::Outline {
                        bounds: [l, t, r, b],
                        color,
                    } => {
                        for (a, z) in [
                            ([l, t], [r, t]),
                            ([r, t], [r, b]),
                            ([r, b], [l, b]),
                            ([l, b], [l, t]),
                        ] {
                            buttons.line(a, z, color);
                        }
                    }
                    Primitive::Line { start, end, color } => buttons.line(start, end, color),
                }
            }
        }
        let mut fonts = self.fonts.clone();
        let font = Font::parse(&self.tab)?;
        for (i, label) in self.labels.iter().enumerate() {
            // Native 49e750 sets font4 and 440650 style9 on each row.
            // Generic 442b10 changes disabled Save text to style8.
            let color = if save_disabled && i == 1 {
                0xff646464
            } else {
                0xffffffff
            };
            crate::ui::text_widget_style9(&mut fonts, &self.tab, label, self.bounds[i], color)?;
        }
        if let Some(message) = message {
            // Save feedback is host presentation text beneath the original menu.
            // Wrap long errors so the complete result remains visible.
            let text = crate::load::ascii(message);
            for (i, line) in crate::load::wrap_lines(&self.tab, &text, 480.)?
                .iter()
                .enumerate()
            {
                for q in font.layout(
                    line,
                    Layout {
                        origin: [80., self.bounds[3][3] + 12. + i as f32 * 24.],
                        depth: 0.5,
                        scale: 1.,
                        gap: 0,
                        baseline: 0,
                    },
                )? {
                    fonts[q.page].quad(q.positions.map(|p| [p[0], p[1]]), q.uv, [0xffffffff; 4]);
                }
            }
        }
        let mut draws = self.base.iter().map(Surface::draw).collect::<Vec<_>>();
        draws.push(buttons.draw());
        draws.extend(fonts.iter().map(Surface::draw));
        renderer.prepare(&draws, VIEW, None)
    }
}

#[derive(Default)]
pub struct View {
    pub open: bool,
    pub resume_running: bool,
    pub pressed: Option<Button>,
    held: bool,
    cue_latch: Option<Button>,
    pub prepared: Option<PreparedScene>,
    shown: Option<(Option<Button>, Option<Button>)>,
    message: Option<String>,
    save_disabled: bool,
    pub quit_prompt: crate::prompt::State,
    pub quit_reply: Option<std::sync::mpsc::Receiver<Vec<crate::prompt::Request>>>,
}
impl View {
    pub fn open(&mut self, running: bool) {
        self.open = true;
        self.resume_running = running;
        self.pressed = None;
        self.held = false;
        self.cue_latch = None;
        self.save_disabled = false;
        self.shown = None;
    }
    pub fn close(&mut self) -> bool {
        let running = self.resume_running;
        self.open = false;
        self.pressed = None;
        self.held = false;
        self.cue_latch = None;
        self.prepared = None;
        self.shown = None;
        self.quit_prompt = crate::prompt::State::default();
        self.quit_reply = None;
        running
    }
    pub fn set_message(&mut self, message: String) {
        self.message = Some(message);
        self.shown = None;
    }
    pub fn set_save_disabled(&mut self, disabled: bool) {
        if self.save_disabled != disabled {
            self.save_disabled = disabled;
            self.shown = None;
        }
    }
    pub fn mouse(
        &mut self,
        assets: &Assets,
        pointer: Option<[f32; 2]>,
        pressed: bool,
    ) -> Option<Button> {
        if self.quit_prompt.request().is_some() {
            return None;
        }
        let hit = pointer
            .and_then(|p| assets.button(p))
            .filter(|button| !self.save_disabled || *button != Button::Save);
        if pressed {
            self.held = true;
            self.cue_latch = None;
            self.pressed = hit;
            None
        } else {
            self.held = false;
            self.cue_latch = None;
            let activation = (hit == self.pressed).then_some(hit).flatten();
            self.pressed = None;
            if activation == Some(Button::Quit) {
                let dialog = quit_dialog();
                self.quit_prompt.submit(crate::prompt::Request {
                    script: None,
                    dialog: Some(dialog),
                    title: dialog.title,
                    body: dialog.body,
                    dismissal_id: 0,
                    card: None,
                });
            }
            activation
        }
    }
    /// Generic 443360 buttons emit one light click on first held entry.
    /// Leaving clears the widget latch, so re-entry can sound again.
    pub fn pointer_click_cue(&mut self, assets: &Assets, pointer: Option<[f32; 2]>) -> bool {
        if !self.held {
            self.cue_latch = None;
            return false;
        }
        let hit = pointer
            .and_then(|point| assets.button(point))
            .filter(|button| !self.save_disabled || *button != Button::Save);
        let cue = hit.is_some() && hit != self.cue_latch;
        self.cue_latch = hit;
        cue
    }
    pub fn update(
        &mut self,
        assets: &Assets,
        renderer: &Renderer,
        pointer: Option<[f32; 2]>,
    ) -> Result<(), String> {
        if !self.open {
            return Ok(());
        }
        let hover = pointer
            .and_then(|p| assets.button(p))
            .filter(|button| !self.save_disabled || *button != Button::Save);
        let state = (hover, self.pressed);
        if self.shown != Some(state) {
            self.prepared = Some(assets.prepare_message(
                renderer,
                hover,
                self.pressed,
                self.message.as_deref(),
                self.save_disabled,
            )?);
            self.shown = Some(state);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Assets, Button, View};

    #[test]
    fn options_generic_button_cue_tracks_held_entry_and_disabled_save() {
        let assets = Assets::test_menu();
        let mut view = View::default();
        view.open(false);
        assert_eq!(view.mouse(&assets, Some([5., 5.]), true), None);
        assert!(view.pointer_click_cue(&assets, Some([5., 5.])));
        assert!(!view.pointer_click_cue(&assets, Some([5., 5.])));
        assert!(!view.pointer_click_cue(&assets, None));
        assert!(view.pointer_click_cue(&assets, Some([5., 5.])));
        assert_eq!(
            view.mouse(&assets, Some([5., 5.]), false),
            Some(Button::Continue)
        );
        assert!(!view.pointer_click_cue(&assets, Some([5., 5.])));
        view.set_save_disabled(true);
        view.mouse(&assets, Some([25., 5.]), true);
        assert!(!view.pointer_click_cue(&assets, Some([25., 5.])));
        assert_eq!(view.mouse(&assets, Some([25., 5.]), false), None);
    }
}

#[cfg(test)]
mod mesh_culling_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn options_own_pose_winding_degenerates_and_order() {
        // Raw mesh Y points up; the loader negates it before facing selection.
        let positions = [
            [0., 0., 3.],
            [0., -2., 3.],
            [3., -2., 3.],
            [3., 0., 3.],
            [1.5, -1., 3.],
        ];
        let indices = [0, 2, 1, 0, 1, 2, 0, 4, 2, 0, 0, 1, 0, 2, 3];
        assert_eq!(
            options_mesh_triangles(&positions, &indices).collect::<Vec<_>>(),
            [[0, 1, 2], [0, 2, 3]]
        );
        let reflected = positions.map(|[x, y, z]| [x, -y, z]);
        assert_eq!(
            options_mesh_triangles(&reflected, &indices).collect::<Vec<_>>(),
            [[0, 2, 1]]
        );
        let moved = positions.map(|[x, y, _]| [x * 2. + 11., y * 3. - 7., -99.]);
        assert_eq!(
            options_mesh_triangles(&moved, &indices).collect::<Vec<_>>(),
            [[0, 1, 2], [0, 2, 3]]
        );
        assert_eq!(
            options_mesh_triangles(&positions, &[0, 4, 2, 0, 0, 1]).count(),
            0
        );
    }
    fn guard(bytes: &[u8], expected: &str) {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("sha256sum")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("fixture checks require sha256sum");
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            std::str::from_utf8(&output.stdout)
                .unwrap()
                .split_whitespace()
                .next(),
            Some(expected)
        );
    }
    fn guarded(path: &Path, expected: &str) -> Vec<u8> {
        let bytes = fs::read(path).unwrap();
        guard(&bytes, expected);
        bytes
    }
    struct Supplied {
        directory: Vec<u8>,
        archive: Vec<u8>,
        installer: PathBuf,
        language: PathBuf,
    }
    impl Supplied {
        fn read() -> Self {
            let directory = fs::read(openmt2001::fixtures::path("ARCHIVE_DIR")).unwrap();
            let archive = fs::read(openmt2001::fixtures::path("ARCHIVE_BIN")).unwrap();
            let installer = openmt2001::fixtures::path("PARAMETERS")
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf();
            let language = openmt2001::fixtures::path("OPTIONS_LANGUAGE");
            guarded(
                &installer.join("Standard/fonts/coppgoth23p.tab"),
                "5e5ac4756550b3b22058510de5fafa31114709b10735fdfb41823b7d283ac300",
            );
            guarded(
                &language.join("tycoon.off"),
                "54a2c8adf44e37138bcaf36bf9c9218e0543880e308a0b9cc9a883ee98265b4d",
            );
            guarded(
                &language.join("tycoon.bin"),
                "4d5e302035f3579d2c0cb8cd35a58abae5ea6d8159e6c3661bef091fedff6c38",
            );
            for (name, hash) in [
                (
                    "tycoon.off",
                    "0cb07d00caba4717e0ae7ee5a757ce9e2a18eb3b3013d5812e9da485a1090f3a",
                ),
                (
                    "tycoon.bin",
                    "42696b634f2b7517a645516317341cf40ed2d25efe6931e033f65c527a3d28e1",
                ),
            ] {
                guarded(&installer.join("Standard/fonts/board/usa").join(name), hash);
            }
            let supplied = Self {
                directory,
                archive,
                installer,
                language,
            };
            for (name, hash) in [
                (
                    "gamedata2/brand_new_panels/main_menu.tm",
                    "3b06d3a1a95600814b7ba56e7e47e936ee4a37e0014d4ddca6745cdc6c4ce3e8",
                ),
                (
                    "FRONT_END_BLUE_BOARD.TT",
                    "6b36529ac23bdfbeadc166a9838d8ed2dc716aad52093ffad4f77a30158772a0",
                ),
                (
                    "4by4_colours_black4by4.tt",
                    "9a19ef3b8659aab6370b591615e418b5776d12b7de5ad3a85cea92a83e95a934",
                ),
                (
                    "front_end_background_blue.tt",
                    "c43054ad3f84223b7719938db42b92675648f361d47a1f1acf842dfecd2957bb",
                ),
                (
                    "gamedata2/GenericTextures640/textures.gtd",
                    "10a278bd4ed76076f5b517069f1c8a96ddc7a8b3bb282ff8998842fa8beb88f4",
                ),
                (
                    "gamedata2/font640/textures.gtd",
                    "21ed8e328fc95d98eb8cf6f99927d2dc28bd7b3bd309b67bbfbbec52f9be5912",
                ),
            ] {
                guard(supplied.payload(name), hash);
            }
            supplied
        }
        fn payload(&self, name: &str) -> &[u8] {
            let index = ArchiveIndex::parse(&self.directory, self.archive.len() as u64).unwrap();
            let e = index.find(name).unwrap();
            &self.archive[e.offset as usize..(e.offset + e.length) as usize]
        }
        fn assets(&self) -> Assets {
            let index = ArchiveIndex::parse(&self.directory, self.archive.len() as u64).unwrap();
            Assets::load(&index, &self.archive, &self.installer, &self.language).unwrap()
        }
    }
    #[test]
    #[ignore = "SHA-pinned original main-menu mesh/materials/language and sha256sum"]
    fn supplied_options_mesh_faces_regions_materials_and_logo() {
        let supplied = Supplied::read();
        let mesh =
            CachedMesh::parse(supplied.payload("gamedata2/brand_new_panels/main_menu.tm")).unwrap();
        assert_eq!(
            (mesh.vertex_count, mesh.parts.len(), mesh.frames),
            (92, 14, 1)
        );
        let pos = mesh.positions(0).unwrap();
        let uv = mesh.uvs();
        let regions = ui_mesh::regions(&mesh, 0).unwrap();
        assert_eq!(regions.len(), 14); // regions come from the full unfiltered mesh
        assert_eq!(
            regions.iter().map(|r| r.part).collect::<Vec<_>>(),
            (0..14).collect::<Vec<_>>()
        );
        assert_eq!(regions[13].name, "MAIN_TYCOON_LOGO");
        assert_eq!(regions[4].name, "MAIN_DR_LOGO");
        assert_eq!(regions[5].name, "MAIN_INFOGRAMES_LOGO");
        let mut retained = Vec::new();
        let mut rejected = Vec::new();
        let mut front_count = 0;
        let mut back_count = 0;
        for (part_id, part) in mesh.parts.iter().enumerate() {
            let indices = part.vertex_indices().collect::<Vec<_>>();
            let visible = options_mesh_triangles(&pos, &indices).collect::<Vec<_>>();
            if part_id < 4 {
                assert_eq!(
                    visible
                        .iter()
                        .flat_map(|t| t.iter().copied())
                        .collect::<Vec<_>>(),
                    indices
                );
                retained.push(part_id);
                front_count += visible.len();
            } else {
                assert!(visible.is_empty(), "layout part{part_id} became visible");
                rejected.push(part_id);
                back_count += indices.len() / 3;
            }
        }
        assert_eq!(retained, [0, 1, 2, 3]);
        assert_eq!(front_count, 26);
        assert_eq!(rejected, (4..14).collect::<Vec<_>>());
        assert_eq!(back_count, 20);
        let assets = supplied.assets(); // actual production upload path
        assert_eq!(assets.base.len(), 5); // four mesh surfaces + real logo
        for (part_id, actual) in assets.base[..4].iter().enumerate() {
            let part = &mesh.parts[part_id];
            let expected = part
                .vertex_indices()
                .map(|i| [pos[i][0], -pos[i][1], 0.5, 1., 1., 1., uv[i][0], uv[i][1]])
                .collect::<Vec<_>>();
            assert_eq!(
                actual.vertices, expected,
                "original retained part/order{part_id}"
            );
            assert_eq!(actual.alpha, vec![255; expected.len()]);
            let tex =
                TextureSet::parse(supplied.payload(mesh.texture_names[part.texture as usize]))
                    .unwrap();
            let material = Surface::new(&tex.textures[0]);
            assert_eq!(
                (actual.width, actual.height, &actual.pixels),
                (material.width, material.height, &material.pixels)
            );
            assert!(matches!(
                actual.draw().mode,
                openmt2001::render::SurfaceMode::Overlay
            ));
        }
        // All layout boxes remain queryable from the original mesh. Only
        // their raster triangles are filtered; these bounds are not rebuilt.
        for part_id in 0..14 {
            let points = mesh.parts[part_id]
                .vertex_indices()
                .map(|i| pos[i])
                .collect::<Vec<_>>();
            let bounds = points.iter().fold(
                [
                    f32::INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                ],
                |b, p| {
                    [
                        b[0].min(p[0]),
                        b[1].min(-p[1]),
                        b[2].max(p[0]),
                        b[3].max(-p[1]),
                    ]
                },
            );
            assert_eq!(regions[part_id].bounds, bounds);
        }
        for (slot, button) in [
            Button::Continue,
            Button::Save,
            Button::Settings,
            Button::Quit,
        ]
        .into_iter()
        .enumerate()
        {
            let bounds = regions[slot + 7].bounds;
            assert_eq!(assets.bounds[slot], bounds);
            assert_eq!(
                assets.button([(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5]),
                Some(button)
            );
        }
        let background =
            TextureSet::parse(supplied.payload("front_end_background_blue.tt")).unwrap();
        assert!(
            background.textures[0]
                .texels
                .chunks_exact(4)
                .all(|pixel| pixel[3] == 128)
        );
        for part in 3..14 {
            assert!(
                mesh.texture_names[mesh.parts[part].texture as usize]
                    .eq_ignore_ascii_case("front_end_background_blue.tt")
            );
        }
        let generic =
            TextureSet::parse(supplied.payload("gamedata2/GenericTextures640/textures.gtd"))
                .unwrap();
        let mut logo = Surface::new(&generic.textures[287]);
        let [l, t, r, b] = regions[13].bounds;
        logo.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        let actual = &assets.base[4];
        assert_eq!(actual.vertices, logo.vertices);
        assert_eq!(actual.alpha, logo.alpha);
        assert_eq!(
            (actual.width, actual.height, &actual.pixels),
            (logo.width, logo.height, &logo.pixels)
        );
    }
    #[test]
    #[cfg(feature = "capture")]
    #[ignore = "SHA-pinned supplied assets, offscreen GPU and output filename"]
    fn capture_options_menu_over_nonuniform_underlay() {
        let supplied = Supplied::read();
        let assets = supplied.assets();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) {
            descriptor.backends = wgpu::Backends::DX12;
        }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let panel = assets.prepare(&renderer, None, None).unwrap();
        let mut checker = Surface::solid();
        for row in 0..15 {
            for col in 0..20 {
                let rgb = if (row + col) % 2 == 0 {
                    0xfff04020
                } else {
                    0xff1040e0
                };
                checker.rect(
                    [
                        col as f32 * 32.,
                        row as f32 * 32.,
                        (col + 1) as f32 * 32.,
                        (row + 1) as f32 * 32.,
                    ],
                    [rgb; 4],
                );
            }
        }
        let underlay = renderer.prepare(&[checker.draw()], VIEW, None).unwrap();
        let path = std::env::var("OPENMT2001_OPTIONS_CAPTURE_PATH").unwrap();
        crate::capture_test::capture_overlay(&renderer, &underlay, &panel, &path);
    }
}
