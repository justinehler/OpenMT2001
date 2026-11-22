//! In-game Options overlay. The supplied original recording places its four
//! rows in `main_menu.tm` MAIN_BUTT02..05, beneath the retained logo.
use crate::ui::Surface;
use openmt2001::{archive::ArchiveIndex, font::{Font, Layout}, mesh::CachedMesh,
    render::{PreparedScene, Renderer}, texture::TextureSet, ui_button::{self, Primitive, State}, ui_mesh};
use std::{fs, path::Path};

const VIEW: [f32; 16] = [2./640., 0., 0., 0., 0., -2./480., 0., 0., 0., 0., 1., 0., -1., 1., 0., 1.];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button { Continue, Save, Settings, Quit }

pub struct Assets {
    base: Vec<Surface>,
    fonts: Vec<Surface>,
    tab: Vec<u8>,
    labels: [Vec<u8>; 4],
    bounds: [[f32; 4]; 4],
}

impl Assets {
    pub fn load(index: &ArchiveIndex, archive: &[u8], installer: &Path,
        language: &Path) -> Result<Self, String> {
        let payload = |name: &str| -> Result<&[u8], String> {
            let e = index.find(name).ok_or_else(|| format!("missing options asset {name}"))?;
            archive.get(e.offset as usize..(e.offset + e.length) as usize)
                .ok_or_else(|| format!("options asset {name} outside archive"))
        };
        let mesh = CachedMesh::parse(payload("gamedata2/brand_new_panels/main_menu.tm")?)?;
        let regions = ui_mesh::regions(&mesh, 0)?;
        // Native49e750 indexes the five-word descriptors at543a68. Their
        // first words name MAIN_BUTT02..05, resolved by43af30 unchanged.
        let bounds: [[f32; 4]; 4] = ["MAIN_BUTT02", "MAIN_BUTT03", "MAIN_BUTT04", "MAIN_BUTT05"]
            .map(|name| regions.iter().find(|r| r.name == name).map(|r| r.bounds)
                .ok_or_else(|| format!("original options row {name} missing")))
            .into_iter().collect::<Result<Vec<_>, _>>()?.try_into()
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
            for i in part.vertex_indices() {
                s.vertices.push([pos[i][0], -pos[i][1], 0.5, 1., 1., 1., uv[i][0], uv[i][1]]);
                s.alpha.push(255);
            }
            base.push(s);
        }
        let generic = TextureSet::parse(payload("gamedata2/GenericTextures640/textures.gtd")?)
            .map_err(|e| e.to_string())?;
        let mut logo = Surface::new(generic.textures.get(287).ok_or("original Tycoon logo missing")?);
        let [l,t,r,b] = regions[13].bounds;
        logo.quad([[l,t],[l,b],[r,b],[r,t]], [[0.,0.],[0.,1.],[1.,1.],[1.,0.]], [0xffffffff;4]);
        base.push(logo);
        let fonts = TextureSet::parse(payload("gamedata2/font640/textures.gtd")?)
            .map_err(|e| e.to_string())?.textures[8..10].iter().map(Surface::new).collect();
        let tab = fs::read(installer.join("Standard/fonts/coppgoth23p.tab"))
            .map_err(|e| e.to_string())?;
        let off = fs::read(language.join("tycoon.off")).map_err(|e| e.to_string())?;
        let bin = fs::read(language.join("tycoon.bin")).map_err(|e| e.to_string())?;
        let mut cursor = 0usize;
        let mut rows = Vec::new();
        for length in off.chunks_exact(2) {
            let end = cursor + u16::from_le_bytes(length.try_into().unwrap()) as usize;
            rows.push(bin.get(cursor..end).ok_or("options language row outside data")?);
            cursor = end;
        }
        let localized = crate::tooltip::localized_strings(installer, &rows)?;
        // Native in-game menu strings: Continue, Save Game, Settings, Quit.
        let labels = [1678, 1680, 1679, 1682].map(|i| localized.get(i)
            .cloned().ok_or("original in-game Options label missing".to_owned()))
            .into_iter().collect::<Result<Vec<_>, _>>()?.try_into()
            .map_err(|_| "original in-game Options label count")?;
        Ok(Self { base, fonts, tab, labels, bounds })
    }
    pub fn button(&self, pointer: [f32; 2]) -> Option<Button> {
        self.bounds.iter().position(|&[l,t,r,b]| pointer[0] >= l && pointer[0] <= r
            && pointer[1] >= t && pointer[1] <= b)
            .map(|i| [Button::Continue, Button::Save, Button::Settings, Button::Quit][i])
    }
    #[cfg(test)]
    pub fn bounds(&self) -> [[f32; 4]; 4] { self.bounds }
    #[cfg(test)]
    pub fn labels(&self) -> &[Vec<u8>; 4] { &self.labels }
    #[cfg(test)]
    pub fn prepare(&self, renderer: &Renderer, hovered: Option<Button>, pressed: Option<Button>)
        -> Result<PreparedScene, String> {
        self.prepare_message(renderer, hovered, pressed, None, false)
    }
    fn prepare_message(&self, renderer: &Renderer, hovered: Option<Button>, pressed: Option<Button>,
        message: Option<&str>, save_disabled: bool) -> Result<PreparedScene, String> {
        let mut buttons = Surface::solid();
        let colors = [[0,192,0], [0,0,192], [110,4,171], [185,7,187]];
        let actions = [Button::Continue, Button::Save, Button::Settings, Button::Quit];
        for (i, &bounds) in self.bounds.iter().enumerate() {
            for primitive in ui_button::background(bounds, colors[i], State {
                flag_330: (!save_disabled || actions[i]!=Button::Save)
                    && pressed == Some(actions[i]) && hovered == pressed,
                flag_334: (!save_disabled || actions[i]!=Button::Save)
                    && pressed == Some(actions[i]) && hovered == pressed,
                flag_335: hovered == Some(actions[i]) && (!save_disabled || actions[i]!=Button::Save),
                flag_333: save_disabled && actions[i]==Button::Save,
            })? {
                match primitive {
                    Primitive::Gradient { bounds, colors } => buttons.rect(bounds, colors),
                    Primitive::Outline { bounds: [l,t,r,b], color } => {
                        for (a,z) in [([l,t],[r,t]),([r,t],[r,b]),([r,b],[l,b]),([l,b],[l,t])] {
                            buttons.line(a,z,color);
                        }
                    }
                    Primitive::Line { start, end, color } => buttons.line(start,end,color),
                }
            }
        }
        let mut fonts = self.fonts.clone();
        let font = Font::parse(&self.tab)?;
        for (i, label) in self.labels.iter().enumerate() {
            // Native 49e750 sets font4 and 440650 style9 on each row.
            // Generic 442b10 changes disabled Save text to style8.
            let color = if save_disabled && i == 1 { 0xff646464 } else { 0xffffffff };
            crate::ui::text_widget_style9(&mut fonts, &self.tab, label,
                self.bounds[i], color)?;
        }
        if let Some(message) = message {
            // Save feedback is host presentation text beneath the original menu.
            // Wrap long errors so the complete result remains visible.
            let text = crate::load::ascii(message);
            for (i, line) in crate::load::wrap_lines(&self.tab, &text, 480.)?.iter().enumerate() {
                for q in font.layout(line, Layout { origin: [80., self.bounds[3][3] + 12. + i as f32 * 24.],
                    depth: 0.5, scale: 1., gap: 0, baseline: 0 })? {
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
        running
    }
    pub fn set_message(&mut self, message: String) {
        self.message = Some(message);
        self.shown = None;
    }
    pub fn set_save_disabled(&mut self, disabled: bool) {
        if self.save_disabled!=disabled { self.save_disabled=disabled; self.shown=None; }
    }
    pub fn mouse(&mut self, assets: &Assets, pointer: Option<[f32; 2]>, pressed: bool)
        -> Option<Button> {
        let hit = pointer.and_then(|p| assets.button(p))
            .filter(|button| !self.save_disabled || *button!=Button::Save);
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
            activation
        }
    }
    /// Generic 443360 buttons emit one light click on first held entry.
    /// Leaving clears the widget latch, so re-entry can sound again.
    pub fn pointer_click_cue(&mut self, assets: &Assets,
        pointer: Option<[f32; 2]>) -> bool {
        if !self.held {
            self.cue_latch = None;
            return false;
        }
        let hit = pointer.and_then(|point| assets.button(point))
            .filter(|button| !self.save_disabled || *button != Button::Save);
        let cue = hit.is_some() && hit != self.cue_latch;
        self.cue_latch = hit;
        cue
    }
    pub fn update(&mut self, assets: &Assets, renderer: &Renderer, pointer: Option<[f32;2]>)
        -> Result<(), String> {
        if !self.open { return Ok(()); }
        let hover = pointer.and_then(|p| assets.button(p))
            .filter(|button| !self.save_disabled || *button!=Button::Save);
        let state = (hover, self.pressed);
        if self.shown != Some(state) {
            self.prepared = Some(assets.prepare_message(renderer, hover, self.pressed,
                self.message.as_deref(),self.save_disabled)?);
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
        let assets = Assets { base: Vec::new(), fonts: Vec::new(), tab: Vec::new(),
            labels: std::array::from_fn(|_| Vec::new()),
            bounds: [[0.,0.,10.,10.],[20.,0.,30.,10.],
                [40.,0.,50.,10.],[60.,0.,70.,10.]] };
        let mut view = View::default();
        view.open(false);
        assert_eq!(view.mouse(&assets, Some([5.,5.]), true), None);
        assert!(view.pointer_click_cue(&assets, Some([5.,5.])));
        assert!(!view.pointer_click_cue(&assets, Some([5.,5.])));
        assert!(!view.pointer_click_cue(&assets, None));
        assert!(view.pointer_click_cue(&assets, Some([5.,5.])));
        assert_eq!(view.mouse(&assets, Some([5.,5.]), false), Some(Button::Continue));
        assert!(!view.pointer_click_cue(&assets, Some([5.,5.])));
        view.set_save_disabled(true);
        view.mouse(&assets, Some([25.,5.]), true);
        assert!(!view.pointer_click_cue(&assets, Some([25.,5.])));
        assert_eq!(view.mouse(&assets, Some([25.,5.]), false), None);
    }
}
