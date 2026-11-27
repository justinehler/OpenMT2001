//! Concrete supplied-model frame consumer for common panel transition shells.
//! Immutable archive assets are borrowed only while loading; no original bytes
//! are embedded in source. Actual descriptor position/alpha stays in UiGraph.
use super::ui_graph::{PanelId, UiGraphState};
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    mesh::CachedMesh,
    render::{PreparedScene, Renderer},
};
use std::collections::BTreeMap;
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
fn path(id: PanelId) -> Result<&'static str, String> {
    let name = match id {
        PanelId::A | PanelId::G => "main_panel.tm",
        PanelId::B => "bap_v2.tm",
        PanelId::C => "main_panel_v2.tm",
        PanelId::E => "new_nav_bottom.tm",
        PanelId::F => "new_nav_top.tm",
        PanelId::H => "bap_apartments_v2.tm",
        _ => return Err(format!("No ordinary mesh descriptor for {id:?}")),
    };
    Ok(match name {
        "main_panel.tm" => "gamedata2/brand_new_panels/main_panel.tm",
        "bap_v2.tm" => "gamedata2/brand_new_panels/bap_v2.tm",
        "main_panel_v2.tm" => "gamedata2/brand_new_panels/main_panel_v2.tm",
        "new_nav_bottom.tm" => "gamedata2/brand_new_panels/new_nav_bottom.tm",
        "new_nav_top.tm" => "gamedata2/brand_new_panels/new_nav_top.tm",
        "bap_apartments_v2.tm" => "gamedata2/brand_new_panels/bap_apartments_v2.tm",
        _ => unreachable!(),
    })
}
pub struct Assets {
    frames: BTreeMap<PanelId, Vec<Vec<Surface>>>,
}
impl Assets {
    ///Supply exactly the model-backed graph panels registered for this UI.
    ///D is a separate generated world descriptor, never stats_panel.tm.
    pub fn load(index: &ArchiveIndex, archive: &[u8], ids: &[PanelId]) -> Result<Self, String> {
        let mut frames = BTreeMap::new();
        for &id in ids {
            let path = path(id)?;
            let entry = index.find(path).ok_or_else(|| format!("Missing {path}"))?;
            let bytes = archive
                .get(entry.offset as usize..(entry.offset + entry.length) as usize)
                .ok_or("Panel model range leaves supplied archive")?;
            let mesh = CachedMesh::parse(bytes)?;
            //Bound memory and reject corrupt supplied descriptor counts.
            if mesh.frames == 0 || mesh.frames > u16::MAX as usize {
                return Err("Panel frame count leaves nativeu16".into());
            }
            let parts = (0..mesh.parts.len()).collect::<Vec<_>>();
            let mut model = Vec::with_capacity(mesh.frames);
            for frame in 0..mesh.frames {
                model.push(ui::panel_frame_base(index, archive, path, &parts, frame)?.0);
            }
            frames.insert(id, model);
        }
        Ok(Self { frames })
    }
    ///43c3a0 model submission remains eligible while hiding or closed with a
    ///main region. Body overlays use graph.body independently. The caller must draw
    ///only this shell during motion, in the real group's registration order.
    ///This uses the existing modern2D supplied-panel conversion; exact native
    ///material matrix/depth/backing parity is explicitly still a render gap.
    pub fn prepare_shell(
        &self,
        renderer: &Renderer,
        graph: &UiGraphState,
        id: PanelId,
    ) -> Result<Option<PreparedScene>, String> {
        if !graph.shell(id) {
            return Ok(None);
        }
        let panel = graph.panel(id)?;
        if panel.independent != 0 {
            return Ok(None);
        } //native skips model submit
        let model = self
            .frames
            .get(&id)
            .ok_or("Reached panel shell lacks supplied model binding")?;
        if model.len() != panel.frame_count as usize {
            return Err("Live panel frame count differs from actual model".into());
        }
        let mut surfaces = model
            .get(panel.frame as usize)
            .ok_or("Live panel frame leaves actual model")?
            .clone();
        for surface in &mut surfaces {
            for vertex in &mut surface.vertices {
                vertex[0] += panel.position[0];
                vertex[1] += panel.position[1];
            }
            for alpha in &mut surface.alpha {
                *alpha = panel.alpha;
            }
        }
        Ok(Some(renderer.prepare(
            &surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
            VIEW,
            None,
        )?))
    }
}

/// Fixed fresh Primary catalog: each slot labels one distinct successful load.
/// The lower loader allocates fresh mutable model payloads even for A/G. This
/// label is not a native handle or general model identity: shared references,
/// geometry transfer and later lifetimes are outside this initial-factory API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub panel: PanelId,
    pub path: String,
    pub offset: u64,
    pub length: u64,
    pub metadata: [u32; 13],
}
pub struct CpuPart {
    pub header: [u8; 20],
    pub indices: Vec<usize>,
}
/// Owns parsed products after the archive borrow ends. Positions are exactly
/// CachedMesh::positions (cached wire coordinates); 43b770 mutates their Z.
/// Serialized definition bytes are retained, not recomputed after UI binding.
pub struct CpuModel {
    pub source: Source,
    pub header: [u8; 164],
    pub definition: [u8; 116],
    pub frames: Vec<Vec<[f32; 3]>>,
    pub parts: Vec<CpuPart>,
    pub records35: Vec<[u8; 35]>,
    pub records28: Vec<[u8; 28]>,
    pub uvs: Vec<[f32; 2]>,
    pub normals: Vec<u8>,
    pub texture_names: Vec<String>,
}
impl CpuModel {
    /// Invoke at an existing successful mesh parse, not from a frame-count
    /// argument or synthetic production provider. No missing-resource fallback.
    pub fn parsed(
        panel: PanelId,
        entry: &openmt2001::archive::ArchiveEntry,
        mesh: &CachedMesh<'_>,
    ) -> Result<Self, String> {
        if mesh.frames == 0 || mesh.frames > u16::MAX as usize {
            return Err("CPU UI model has no native frame domain".into());
        }
        let index_bytes = if mesh.parts.is_empty() {
            0
        } else {
            4 + mesh.parts.iter().map(|p| p.indices.len()).sum::<usize>()
        };
        let wire_length = 164
            + mesh.vertex_bytes.len()
            + mesh.uv_bytes.len()
            + mesh.normal_bytes.len()
            + mesh.opaque_records_35.len()
            + mesh.opaque_records_28.len()
            + mesh.parts.len() * 20
            + index_bytes
            + mesh
                .texture_names
                .iter()
                .map(|n| n.len() + 1)
                .sum::<usize>();
        if entry.length != wire_length as u64
            || entry.offset != u64::from(entry.metadata[1])
            || entry.length != u64::from(entry.metadata[2])
        {
            return Err("CPU UI source entry/parsed mesh range mismatch".into());
        }
        let frames = (0..mesh.frames)
            .map(|f| mesh.positions(f))
            .collect::<Result<Vec<_>, _>>()?;
        if frames.iter().flatten().flatten().any(|v| !v.is_finite()) {
            return Err("CPU UI binding requires finite supplied XYZ samples".into());
        }
        let parts = mesh
            .parts
            .iter()
            .map(|p| CpuPart {
                header: *p.header,
                indices: p.vertex_indices().collect(),
            })
            .collect();
        let records35 = mesh
            .opaque_records_35
            .chunks_exact(35)
            .map(|r| r.try_into().unwrap())
            .collect();
        let records28 = mesh
            .opaque_records_28
            .chunks_exact(28)
            .map(|r| r.try_into().unwrap())
            .collect();
        Ok(Self {
            source: Source {
                panel,
                path: entry.path.clone(),
                offset: entry.offset,
                length: entry.length,
                metadata: entry.metadata,
            },
            header: *mesh.header,
            definition: mesh.header[48..164].try_into().unwrap(),
            frames,
            parts,
            records35,
            records28,
            uvs: mesh.uvs(),
            normals: mesh.normal_bytes.to_vec(),
            texture_names: mesh.texture_names.iter().map(|n| (*n).to_owned()).collect(),
        })
    }
    pub fn frame_count(&self) -> u32 {
        self.frames.len() as u32
    }
    /// Native uppercases each ACTUAL record it visits, stopping at first match.
    /// Desired bytes are not uppercased. Preserve duplicate-name first-match,
    /// record ordinal versus part identity, and the unvisited record tails.
    pub fn find(&mut self, desired: Option<&[u8]>) -> Result<u32, String> {
        let Some(desired) = desired.filter(|n| !n.is_empty()) else {
            return Ok(u32::MAX);
        };
        if desired.contains(&0) || !desired.is_ascii() {
            return Err("UI desired region requires bounded ASCII NTBS bytes".into());
        }
        for record in &mut self.records35 {
            let n = record[..33]
                .iter()
                .position(|&b| b == 0)
                .ok_or("UI actual region name lacks terminator")?;
            if !record[..n].is_ascii() {
                return Err("UI region uppercase requires unsupplied locale consumer".into());
            }
            record[..n].make_ascii_uppercase();
            if &record[..n] == desired {
                return Ok(u32::from(record[33]));
            }
        }
        Ok(u32::MAX)
    }
    ///43aca0 -1 is a NO-WRITE result. Empty indexed parts retain the native
    /// extrema; unknown frame/part is an error, never a zero rectangle.
    pub fn region(&self, part: u32, frame: usize) -> Result<Option<[f32; 4]>, String> {
        if part == u32::MAX {
            return Ok(None);
        }
        let positions = self
            .frames
            .get(frame)
            .ok_or("UI region frame outside actual model")?;
        let part = self
            .parts
            .get(part as usize)
            .ok_or("UI region part outside actual model")?;
        let mut lo = [99999f32; 2];
        let mut hi = [-99999f32; 2];
        for &i in &part.indices {
            let p = positions
                .get(i)
                .ok_or("UI region index outside actual model")?;
            for a in 0..2 {
                if p[a] < lo[a] {
                    lo[a] = p[a]
                }
                if p[a] > hi[a] {
                    hi[a] = p[a]
                }
            }
        }
        Ok(Some([lo[0], -hi[1], hi[0], -lo[1]]))
    }
    pub fn parent(&self, frame: usize) -> Result<[f32; 4], String> {
        let positions = self
            .frames
            .get(frame)
            .ok_or("UI parent frame outside actual model")?;
        let mut lo = [99999f32; 2];
        let mut hi = [-99999f32; 2];
        for p in positions {
            for a in 0..2 {
                if p[a] < lo[a] {
                    lo[a] = p[a]
                }
                if p[a] > hi[a] {
                    hi[a] = p[a]
                }
            }
        }
        Ok([lo[0], -hi[1], hi[0], -lo[1]])
    }
}
/// Explicit initialized descriptor FIELD PROJECTION, not a 284-byte fake
/// zero allocation. The native descriptor reset writes these fields; Primary43cc60 selects
/// angleX180/kind11 and model0 reference. Unconsumed carrier bytes stay absent.
/// Model identity/reference is typed; no guessed native registry key exists.
pub struct CpuDescriptor {
    pub slot: PanelId,
    pub reference: Option<PanelId>,
    pub position: [f32; 3],
    pub frame: u16,
    pub alpha: u8,
    pub angle_x: f32,
    pub kind: u16,
    pub definition: [u8; 116],
}
impl CpuDescriptor {
    pub fn primary(model: &CpuModel) -> Result<Self, String> {
        if !super::registration::PRIMARY.iter().any(|e| {
            e.id == model.source.panel
                && matches!(e.descriptor, super::registration::Descriptor::Model(_))
        }) {
            return Err(
                "Primary descriptor producer requires actual model registration slot".into(),
            );
        }
        Ok(Self {
            slot: model.source.panel,
            reference: Some(model.source.panel),
            position: [0.; 3],
            frame: 0,
            alpha: 255,
            angle_x: 180.,
            kind: 11,
            definition: model.definition,
        })
    }
}
/// A single source-owned registry. A reference must resolve to the same actual
/// initial catalog slot and source path. Duplicate insertion never replaces owners.
/// Definition equality below is a bounded construction invariant, not native identity.
pub struct CpuResources {
    pub(super) models: BTreeMap<PanelId, CpuModel>,
    pub(super) descriptors: BTreeMap<PanelId, CpuDescriptor>,
}
impl CpuResources {
    pub fn new() -> Self {
        Self {
            models: BTreeMap::new(),
            descriptors: BTreeMap::new(),
        }
    }
    pub fn insert_primary(
        &mut self,
        c: &super::process_image::Cells,
        model: CpuModel,
    ) -> Result<(), String> {
        let id = model.source.panel;
        let entry = super::registration::PRIMARY
            .iter()
            .find(|e| e.id == id)
            .ok_or("No actual Primary model slot")?;
        let super::registration::Descriptor::Model(ordinal) = entry.descriptor else {
            return Err("Generated descriptor is not an archive model".into());
        };
        let actual = super::registration::asset(c, super::navigation::Group::Primary, ordinal)?;
        let normalize = |s: &str| s.replace('\\', "/").to_ascii_lowercase();
        if normalize(&actual) != normalize(&model.source.path) {
            return Err("CPU model source path differs from actual descriptor asset".into());
        }
        if self.models.contains_key(&id) || self.descriptors.contains_key(&id) {
            return Err("CPU UI descriptor/model alias already owned".into());
        }
        let descriptor = CpuDescriptor::primary(&model)?;
        self.models.insert(id, model);
        self.descriptors.insert(id, descriptor);
        Ok(())
    }
    /// Loader ownership only: no descriptor copy, name lookup or model bind.
    pub(super) fn insert_unbound_model(
        &mut self,
        c: &super::process_image::Cells,
        model: CpuModel,
    ) -> Result<(), String> {
        let id = model.source.panel;
        let entry = super::registration::PRIMARY
            .iter()
            .find(|e| e.id == id)
            .ok_or("No actual Primary model slot")?;
        let super::registration::Descriptor::Model(ordinal) = entry.descriptor else {
            return Err("Special descriptor is not an archive model".into());
        };
        let actual = super::registration::asset(c, super::navigation::Group::Primary, ordinal)?;
        let normalize = |s: &str| s.replace('\\', "/").to_ascii_lowercase();
        if normalize(&actual) != normalize(&model.source.path)
            || self.models.contains_key(&id)
            || self.descriptors.contains_key(&id)
        {
            return Err("Unbound Primary source path/unique ownership differs".into());
        }
        self.models.insert(id, model);
        Ok(())
    }
    /// Reached43cd10 ordinal copy. The same parsed model stays in this owner.
    pub(super) fn prepare_primary_descriptor(&mut self, ordinal: u8) -> Result<PanelId, String> {
        let entry = super::registration::PRIMARY
            .iter()
            .find(
                |e| matches!(e.descriptor,super::registration::Descriptor::Model(i) if i==ordinal),
            )
            .ok_or("Primary pool ordinal absent")?;
        if self.descriptors.contains_key(&entry.id) {
            return Err("Primary descriptor copy cannot replay".into());
        }
        let descriptor = CpuDescriptor::primary(self.model(entry.id)?)?;
        self.descriptors.insert(entry.id, descriptor);
        Ok(entry.id)
    }
    pub fn descriptor(&self, id: PanelId) -> Result<&CpuDescriptor, String> {
        self.descriptors
            .get(&id)
            .ok_or_else(|| "CPU UI descriptor missing".into())
    }
    pub fn model(&self, id: PanelId) -> Result<&CpuModel, String> {
        self.models
            .get(&id)
            .ok_or_else(|| "CPU UI model missing".into())
    }
    pub(super) fn resolve(&self, id: PanelId) -> Result<Option<PanelId>, String> {
        let d = self.descriptor(id)?;
        if d.slot != id {
            return Err("CPU descriptor slot alias mismatch".into());
        }
        let Some(key) = d.reference else {
            return Ok(None);
        };
        if key != id {
            return Err("CPU descriptor/model reference alias mismatch".into());
        }
        let model = self.model(key)?;
        if model.source.panel != key || model.definition != d.definition {
            return Err("CPU descriptor/model definition identity mismatch".into());
        }
        Ok(Some(key))
    }
}
