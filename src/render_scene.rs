//! Shared original scene assembly from byte inputs. Host I/O and presentation stay outside.
use crate::{
    archive::ArchiveIndex,
    board::Board,
    build_file::BuildFile,
    geometry,
    material::{BuildingMaterials, FacadeMaterial},
    pe::PeImage,
    render::{SecondaryTexture, SurfaceMode, TexturedDraw},
    roof,
    scene::{self, SceneBuilding},
    texture::TextureSet,
};
use glam::{Mat4, Vec3};
use std::{collections::BTreeMap, sync::Arc};

/// Original zero-rotation auxiliary pose used by initialized buildings.
/// 4f24d0 negates the matrix's local-Z row before submitting vertices.
#[derive(Clone, Copy, Debug)]
struct BuildingPose {
    translation: [f32; 3],
    scale: [f32; 3],
}
impl BuildingPose {
    fn from_record(record: &[u8]) -> Result<Self, String> {
        let value = |at| f32::from_le_bytes(record[at..at + 4].try_into().unwrap());
        if [0x64, 0x68, 0x6c].map(value).iter().any(|v| *v != 0.) {
            return Err("runtime building has unsupported auxiliary rotation".into());
        }
        // 444ff0 builds the completed building's +0x110 model matrix with
        // unit scale. Tag-4 saves serialize only the first 0x40 bytes, so
        // their reconstructed auxiliary +0x70 fields are zero; static map
        // buildings also reach status 4 with those fields untouched. The
        // pending-world growth stages 1..3 do use +0x70 as their live scale.
        let status = u32::from_le_bytes(record[0x14..0x18].try_into().unwrap());
        let scale = if status == 4 {
            [1.; 3]
        } else {
            std::array::from_fn(|i| value(0x70 + i * 4))
        };
        let pose = Self {
            translation: std::array::from_fn(|i| value(0x58 + i * 4)),
            scale,
        };
        if !pose
            .translation
            .into_iter()
            .chain(pose.scale)
            .all(f32::is_finite)
        {
            return Err("runtime building has nonfinite auxiliary pose".into());
        }
        Ok(pose)
    }
    // 4f2390 calls 4e3770 before multiplying the light vector by127.
    // shade_static performs the remaining local-Z sign and127 operations.
    fn sunlight(self, sun: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| (f64::from(sun[i]) * f64::from(self.scale[i])) as f32)
    }
    fn position(self, position: [f32; 3]) -> Result<[f32; 3], String> {
        let mut result = [0.; 3];
        for i in 0..3 {
            let mut value = crate::x87::Sum::from_f32(position[i])?;
            value.mul_f32(if i == 2 {
                -self.scale[i]
            } else {
                self.scale[i]
            })?;
            value.add_f32(self.translation[i])?;
            result[i] = value.to_f32();
        }
        Ok(result)
    }
}

#[derive(Clone, Copy)]
pub struct OriginalAssets<'a> {
    pub executable: &'a [u8],
    pub directory: &'a [u8],
    pub archive: &'a [u8],
}

pub enum Source<'a> {
    Model {
        index: usize,
        height: u8,
        variation: u32,
    },
    /// One already-selected, world-placed building with original materials.
    /// The caller owns and can cache the resulting draws separately from the city.
    Preview(SceneBuilding),
    Map(&'a str),
    Save(&'a [u8]),
    /// Borrow initialized live building records. No save container is synthesized.
    RuntimeBuildings {
        records: &'a [[u8; 0x394]],
        category_base: u32,
    },
}

pub struct Request<'a> {
    pub build_file: &'a [u8],
    pub source: Source<'a>,
    pub original: Option<OriginalAssets<'a>>,
    pub suncols: Option<&'a [u8]>,
    pub phase: Option<u32>,
    pub camera: Option<crate::flyby::CameraSample>,
    pub width: u32,
    pub height: u32,
    /// Retain the complete sky for camera animation; capture leaves this false.
    pub animated: bool,
}

/// Owned surfaces retain decoded textures once per source image.
#[derive(Clone)]
pub struct Scene {
    pub draws: Vec<OwnedDraw>,
    pub camera: [f32; 16],
    pub fog: Option<crate::render::EyeFog>,
    pub building_count: usize,
    /// Upper model Y per category, before the scene object's Y translation.
    pub category_model_heights: BTreeMap<usize, f32>,
    pub inspection_buildings: Vec<InspectionBuilding>,
    /// Original one-based category owner slots; absent for unsaved map previews.
    pub saved_category_owners: Option<[u8; 52]>,
    /// Snapshot catalog, including original availability; absent for map previews.
    pub saved_business_catalog: Option<crate::business_pool::BusinessCatalog>,
    pub saved_commodity_rows: Option<[[u8; 0x1c]; 32]>,
    /// Exact saved player prefixes for read-only frontend reports.
    pub saved_players: Vec<[u8; 0x1b8]>,
    pub vertex_count: usize,
    pub diagnostics: Vec<String>,
    animation: Option<Animation>,
    runtime_patches: Option<RuntimePatches>,
    ground_colors: [u32; 2],
}

/// Original model-space inputs for the portions of shared material batches
/// emitted by initialized buildings. A construction tick changes only its
/// auxiliary pose; retaining these inputs avoids rebuilding all city scenery.
#[derive(Clone)]
struct RuntimePatches {
    spans: Vec<RuntimeSpan>,
    sunlight: [f32; 3],
    palette: Option<crate::lighting::Palette>,
}

#[derive(Clone)]
struct RuntimeSpan {
    slot: usize,
    key: (usize, usize),
    draw: usize,
    start: usize,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
}

/// Model-space bounds retained in original category insertion order.
#[derive(Clone)]
pub struct InspectionBuilding {
    pub category: usize,
    pub saved_slot: Option<usize>,
    /// Live pool identity for freshly initialized records, distinct from a save.
    pub runtime_slot: Option<usize>,
    pub lot: [u8; 4],
    pub height: u8,
    pub kind: u8,
    pub quality: u8,
    pub saved_business: Option<SavedBusinessInspection>,
    /// Snapshot of a validated live retail or landscape ledger, distinct from a save input.
    pub initialized_business: Option<SavedBusinessInspection>,
    pub initialized_apartment: Option<InitializedApartmentInspection>,
    /// Saved building+2d owner resolved through ordered players and player+150.
    pub saved_owner_character: Option<u32>,
    /// First original category-relative entrance (building+224); absent is unresolved.
    pub camera_entrance: Option<[f32; 3]>,
    pub bounds: [[f32; 3]; 8],
    pub center: [f32; 3],
    pub radius: f32,
    pub translation: [f32; 3],
}

/// Read-only snapshot from a validated live apartment/business link.
#[derive(Clone)]
pub struct InitializedApartmentInspection {
    pub slot: usize,
    pub name_text_id: u32,
    pub record: [u8; 0x220],
    /// Linked business+0xa proprietor, resolved through player+0x150.
    pub owner_character: Option<u32>,
}

#[derive(Clone)]
pub struct SavedBusinessInspection {
    pub slot: usize,
    /// Exact business bytes for original panel adapters; the containing field
    /// distinguishes a saved record from an initialized live record.
    pub record: [u8; 0x220],
    /// Linked business proprietor, distinct from building+0x2d land ownership.
    pub owner_character: Option<u32>,
    pub business_type: u32,
    pub name_text_id: u32,
    pub account: crate::economy::BusinessAccount,
}

impl InspectionBuilding {
    pub fn business(&self) -> Option<&SavedBusinessInspection> {
        self.initialized_business.as_ref().or(self.saved_business.as_ref())
    }
}

#[derive(Clone)]
pub struct OwnedDraw {
    active_vertices: Option<usize>,
    pub view_projection: Option<[f32; 16]>,
    pub vertices: Vec<[f32; 8]>,
    pub specular: Option<Vec<[f32; 3]>>,
    pub diffuse_alpha: Option<Vec<u8>>,
    pub mode: SurfaceMode,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
    pub mipmaps: bool,
    pub secondary: Option<OwnedSecondary>,
}

#[derive(Clone)]
pub struct OwnedSecondary {
    pub uv: Vec<[f32; 2]>,
    pub diffuse_alpha: u8,
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
    pub mipmaps: bool,
}

impl OwnedDraw {
    /// Retained standalone material surface; callers fill verified geometry.
    pub fn texture_page(mode: SurfaceMode, width: u32, height: u32, rgba: Arc<[u8]>) -> Self {
        Self {
            active_vertices: None,
            view_projection: None,
            vertices: Vec::new(),
            specular: Some(Vec::new()),
            diffuse_alpha: None,
            mode,
            width,
            height,
            rgba,
            mipmaps: true,
            secondary: None,
        }
    }

    pub fn borrowed(&self) -> TexturedDraw<'_> {
        let count = self.active_vertices.unwrap_or(self.vertices.len());
        TexturedDraw {
            view_projection: self.view_projection,
            vertices: &self.vertices[..count],
            specular: self.specular.as_ref().map(|v| &v[..count]),
            diffuse_alpha: self.diffuse_alpha.as_ref().map(|v| &v[..count]),
            mode: self.mode,
            width: self.width,
            height: self.height,
            rgba: &self.rgba,
            mipmaps: self.mipmaps,
            secondary: self.secondary.as_ref().map(|s| SecondaryTexture {
                uv: &s.uv[..count],
                diffuse_alpha: s.diffuse_alpha,
                width: s.width,
                height: s.height,
                rgba: &s.rgba,
                mipmaps: s.mipmaps,
            }),
        }
    }
}

pub fn assemble(request: Request<'_>) -> Result<Scene, String> {
    assemble_with_style(request, None)
}

/// The host's original board.prm palette; omitted by PE-default geometry fixtures.
pub fn assemble_with_style(request: Request<'_>, style: Option<&crate::board_style::Style>) -> Result<Scene, String> {
    assemble_with_style_internal(request, style, None)
}

pub fn assemble_with_style_cached(request: Request<'_>, style: Option<&crate::board_style::Style>,
    textures: &mut crate::texture::LoaderCache) -> Result<Scene, String> {
    assemble_with_style_internal(request, style, Some(textures))
}

fn assemble_with_style_internal(request: Request<'_>, style: Option<&crate::board_style::Style>,
    mut textures: Option<&mut crate::texture::LoaderCache>) -> Result<Scene, String> {
    if request.width == 0
        || request.height == 0
        || request.width > i32::MAX as u32
        || request.height > i32::MAX as u32
    {
        return Err("scene viewport must fit positive i32 dimensions".into());
    }
    if let Some(sample) = request.camera {
        validate_camera(sample)?;
    }
    if request.suncols.is_some() && request.original.is_none() {
        return Err("scene lighting requires original assets".into());
    }
    let mut diagnostics = Vec::new();
    let scene_mode = !matches!(&request.source, Source::Model { .. } | Source::Preview(_));
    let preview_mode = matches!(&request.source, Source::Preview(_));
    let file = BuildFile::parse(request.build_file)?;
    let executable = request.original.map(|a| a.executable);
    let archive = request.original.map(|a| a.archive);
    let directory = request.original.map(|a| a.directory);
    if (scene_mode || preview_mode) && request.original.is_none() {
        return Err("city and building preview require original assets".into());
    }
    let materials = executable.map(BuildingMaterials::from_v14).transpose()?;
    let mut sets = Vec::new();
    let mut scenery_pages = Vec::new();
    let mut scenery_families = BTreeMap::new();
    let mut water_families = Vec::new();
    let mut water_meshes = Vec::new();
    let mut saved_phase = None;
    let mut saved_category_owners = None;
    let mut saved_business_catalog = None;
    let mut saved_commodity_rows = None;
    let mut saved_players = Vec::new();
    if let (Some(archive), Some(directory)) = (&archive, &directory) {
        let index =
            ArchiveIndex::parse(directory, archive.len() as u64).map_err(|e| e.to_string())?;
        for path in [
            "gamedata2/buildtextures/textures.gtd",
            "gamedata2/rooftextures/textures.gtd",
            "gamedata2/BuildingTextures/textures.gtd",
        ] {
            let entry = index
                .find(path)
                .ok_or("missing original texture collection")?;
            sets.push(
                TextureSet::parse(
                    &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
                )
                .map_err(|e| e.to_string())?,
            );
        }
    }
    let mut inputs = Vec::new();
    let mut saved_slots = Vec::new();
    let mut camera_entrances = Vec::new();
    let mut runtime_entrances = Vec::new();
    let mut runtime_slots = Vec::new();
    let mut runtime_poses = Vec::new();
    let mut saved_businesses = Vec::new();
    let mut saved_owner_characters = Vec::new();
    if scene_mode {
        let mut board = Board::from_v14(&PeImage::parse(executable.unwrap())?)?;
        if let Some(style) = style { style.apply_board(&mut board)?; }
        let buildings = match request.source {
            Source::Save(data) => {
                let save = crate::save::SaveFile::parse(data)?;
                saved_phase = Some(request.phase.unwrap_or(save.clock()?.calendar[0]));
                save.validate_compatibility(&PeImage::parse(executable.unwrap())?)?;
                let players = save.players()?;
                let player_count = players.len();
                saved_players = players.iter().map(|p| *p.bytes).collect();
                let mut owners = [0; 52];
                for (category, owner) in owners.iter_mut().enumerate() {
                    *owner = save.category_owner(category)?;
                    if usize::from(*owner) > player_count {
                        return Err("saved category owner leaves player roster".into());
                    }
                }
                saved_category_owners = Some(owners);
                let economy = save.economy()?;
                let commodities = save
                    .sections
                    .iter()
                    .find(|s| s.tag == 0x200000)
                    .ok_or("missing saved commodity catalog")?
                    .payload;
                if commodities.len() != 32 * 0x1c {
                    return Err("saved commodity catalog length".into());
                }
                saved_commodity_rows = Some(std::array::from_fn(|i| {
                    commodities[i * 0x1c..(i + 1) * 0x1c].try_into().unwrap()
                }));
                let businesses = economy.businesses().collect::<Vec<_>>();
                let business_info = save
                    .sections
                    .iter()
                    .find(|s| s.tag == 0x40000)
                    .ok_or("missing saved business catalog")?
                    .payload;
                if business_info.len() != 45 * 0x74 {
                    return Err("saved business catalog length".into());
                }
                let rows = business_info
                    .chunks_exact(0x74)
                    .map(|r| r.try_into().unwrap())
                    .collect::<Vec<[u8; 0x74]>>();
                saved_business_catalog = Some(crate::business_pool::BusinessCatalog::from_rows(
                    rows.try_into()
                        .map_err(|_| "saved business catalog length")?,
                ));
                save.buildings()?
                    .iter()
                    .map(|saved| {
                        saved_slots.push(saved.slot);
                        //4454a0 uses building ownership, not the business proprietor.
                        //48c799 saves runtime player+4, so runtime+150 is saved+14c.
                        let owner = saved.bytes[0x2d];
                        let character = if owner == 0 {
                            None
                        } else {
                            let player = players
                                .get(usize::from(owner) - 1)
                                .ok_or("saved building owner leaves player roster")?;
                            let character =
                                u32::from_le_bytes(player.bytes[0x14c..0x150].try_into().unwrap());
                            if character >= 10 {
                                return Err("saved owner character leaves original palette".into());
                            }
                            Some(character)
                        };
                        saved_owner_characters.push(character);
                        // Same saved building+1c linkage used by housing::Topology;
                        // business+8 independently resolves back to this building.
                        let index = i32::from_le_bytes(saved.bytes[0x1c..0x20].try_into().unwrap());
                        let business = if index == -1 {
                            None
                        } else {
                            let slot = usize::try_from(index)
                                .map_err(|_| "negative saved business index")?;
                            let business =
                                businesses.get(slot).ok_or("saved business leaves pool")?;
                            if usize::from(business.building_index()) != saved.slot {
                                return Err("saved business/building linkage disagrees".into());
                            }
                            let proprietor = business.bytes[0xa];
                            let owner_character = if proprietor == 0 {
                                None
                            } else {
                                let player = players.get(usize::from(proprietor) - 1)
                                    .ok_or("saved business owner leaves player roster")?;
                                let character = u32::from_le_bytes(
                                    player.bytes[0x14c..0x150].try_into().unwrap());
                                if character >= 10 {
                                    return Err("saved business owner character leaves palette".into());
                                }
                                Some(character)
                            };
                            Some(SavedBusinessInspection {
                                slot,
                                record: *business.bytes,
                                owner_character,
                                business_type: business.business_type(),
                                // Original004616d0 uses the full name at row+4.
                                name_text_id: {
                                    match business.business_type() {
                                        u32::MAX => 0x35e,
                                        kind @ 0..45 => {
                                            let at = kind as usize * 0x74 + 4;
                                            u32::from_le_bytes(
                                                business_info[at..at + 4].try_into().unwrap(),
                                            )
                                        }
                                        _ => {
                                            return Err("saved business type leaves catalog".into());
                                        }
                                    }
                                },
                                account: crate::economy::BusinessAccount::from_saved(business),
                            })
                        };
                        saved_businesses.push(business);
                        SceneBuilding::from_saved(saved, &board, &file)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            Source::RuntimeBuildings {
                records,
                category_base,
            } => {
                if records.len() != 600 || category_base == 0 {
                    return Err("runtime scene requires initialized600-building arena".into());
                }
                saved_phase = request.phase;
                records
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| r[0x3f] != 0)
                    .map(|(slot, record)| {
                        let word = |at| u32::from_le_bytes(record[at..at + 4].try_into().unwrap());
                        if word(0) as usize != slot || word(0x170) == 0 {
                            return Err("runtime scene building identity/model binding".into());
                        }
                        let category_offset = word(0x40)
                            .checked_sub(category_base)
                            .ok_or("runtime scene category precedes arena")?;
                        if !category_offset.is_multiple_of(0x17c) || category_offset / 0x17c >= 52 {
                            return Err("runtime scene category reference".into());
                        }
                        let category = (category_offset / 0x17c) as usize;
                        let entrance_count = word(0x220);
                        if entrance_count > 8 {
                            return Err("runtime scene entrance count".into());
                        }
                        let entrance = if entrance_count == 0 {
                            None
                        } else {
                            let p: [f32; 3] =
                                std::array::from_fn(|i| f32::from_bits(word(0x224 + i * 4)));
                            if !p.into_iter().all(f32::is_finite) {
                                return Err("runtime scene nonfinite entrance".into());
                            }
                            Some(p)
                        };
                        runtime_entrances.push(entrance);
                        runtime_slots.push(slot);
                        runtime_poses.push(BuildingPose::from_record(record)?);
                        // The native saved prefix is exactly the first0x40 live bytes.
                        // Reuse its field decoder without encoding a save or claiming saved provenance.
                        SceneBuilding::from_saved(
                            &crate::save::SavedBuilding {
                                slot,
                                category,
                                bytes: record[..0x40].try_into().unwrap(),
                            },
                            &board,
                            &file,
                        )
                    })
                    .collect::<Result<Vec<_>, String>>()?
            }
            Source::Map(text) => {
                saved_phase = request.phase;
                scene::parse_static_map(text)?
                    .into_iter()
                    .map(|spec| SceneBuilding::resolve(spec, &board, &file))
                    .collect::<Result<Vec<_>, _>>()?
            }
            Source::Model { .. } | Source::Preview(_) => unreachable!(),
        };
        for (ordinal, mut building) in buildings.into_iter().enumerate() {
            let category = board
                .blocks
                .iter()
                .find(|b| b.id == building.block_id)
                .ok_or("building camera category missing")?;
            camera_entrances.push(if let Some(entrance) = runtime_entrances.get(ordinal) {
                *entrance
            } else {
                building
                    .entrance_positions(category.center)?
                    .first()
                    .copied()
            });
            inputs.push((
                std::mem::take(&mut building.layers),
                building.spec.variation,
                Some(building),
            ));
        }
    } else {
        match request.source {
            Source::Model { index, height, variation } => {
                inputs.push((file.profile(index, height)?, variation, None));
            }
            Source::Preview(mut building) => {
                saved_phase = request.phase;
                camera_entrances.push(None);
                inputs.push((
                    std::mem::take(&mut building.layers),
                    building.spec.variation,
                    Some(building),
                ));
            }
            _ => unreachable!(),
        }
    }
    let suncolors = request.suncols.map(crate::lighting::SunColors::parse).transpose()?;
    let light = if let Some(colors) = &suncolors {
        let phase = saved_phase.ok_or("lighting requires a scene phase")?;
        if phase >= 24 {
            return Err("lighting phase must be in 0..24".into());
        }
        Some(colors.sample(phase, 0)?)
    } else {
        None
    };
    let palette = light
        .as_ref()
        .map(|l| crate::lighting::Palette::new(l.diffuse, l.ambient, l.diffuse, 0, l.scale));
    let mut specular_batches = BTreeMap::<(usize, usize), Vec<[f32; 3]>>::new();
    let facade_material = light
        .as_ref()
        .and_then(|l| FacadeMaterial::select(l.diffuse, true, true));
    let mut facade_inputs = BTreeMap::<(usize, usize), Vec<([f32; 3], [f32; 2])>>::new();
    let building_count = inputs.len();
    // Texture family 0 is diagnostic white, 1 facade, 2 roof.
    let mut batches = BTreeMap::<(usize, usize), Vec<[f32; 8]>>::new();
    let mut runtime_spans = Vec::<RuntimeSpan>::new();
    let mut fallback_texture = None;
    let mut category_model_heights = BTreeMap::<usize, f32>::new();
    let mut inspection_buildings = Vec::new();
    for (ordinal, (profile, variation, building)) in inputs.into_iter().enumerate() {
        let layers = geometry::prepare(&profile);
        let selection = materials.as_ref().map(|m| m.select(variation));
        let base = selection.as_ref().and_then(|s| s.facade_base);
        if selection.is_some() && base.is_none() && fallback_texture.is_none() {
            let image = PeImage::parse(executable.unwrap())?;
            let bytes = archive.unwrap();
            let index = ArchiveIndex::parse(directory.unwrap(), bytes.len() as u64)
                .map_err(|e| e.to_string())?;
            fallback_texture = Some(crate::scene_assets::initial_facade_fallback(
                &image, &index, bytes, 1280,
            )?);
        }
        let place = |position| -> Result<[f32; 3], String> {
            if let Some(building) = &building {
                let [x, y, z] = building.local_position(position);
                if let Some(pose) = runtime_poses.get(ordinal) {
                    return pose.position([x, y, z]);
                }
                // 00488fa2 assigns this translation to the scene object.
                // 00444ff0 forwards it to CPU matrix slots via 004e30a0.
                // The indirect GPU vertex path is still under review.
                // 00488faf clears object Y rotation after baking the mesh.
                // Normal draw 004f1ec0 applies the remaining local-Z flip.
                Ok([x + building.center[0], y + 0.5, building.center[1] - z])
            } else {
                Ok(position)
            }
        };
        let winding = |triangle| {
            building
                .as_ref()
                .map_or(triangle, |b| b.local_triangle(triangle))
        };
        let mut emit = |positions: &[[f32; 3]],
                        uvs: &[[f32; 2]],
                        secondary_uvs: Option<&[[f32; 2]]>,
                        triangles: &[[u32; 3]],
                        key: (usize, usize),
                        diagnostic: [f32; 3]|
         -> Result<(), String> {
            let part = crate::mesh::RegisteredPart::new(positions, triangles)?;
            let runtime_slot = runtime_slots.get(ordinal).copied();
            let start = batches.get(&key).map_or(0, Vec::len);
            let mut source_positions = Vec::new();
            let mut source_normals = Vec::new();
            let colors = if let (Some(palette), Some(light)) = (&palette, &light) {
                Some(
                    part.normals
                        .iter()
                        .map(|&normal| {
                            let normal =
                                building.as_ref().map_or(normal, |b| b.local_normal(normal));
                            palette.shade_static(
                                normal,
                                runtime_poses
                                    .get(ordinal)
                                    .map_or(light.direction, |pose| pose.sunlight(light.direction)),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else {
                None
            };
            for triangle in &part.triangles {
                for index in winding(*triangle) {
                    let index = index as usize;
                    let source = part.source_vertices[index];
                    if runtime_slot.is_some() {
                        let building = building.as_ref().ok_or("runtime span has no building")?;
                        source_positions.push(building.local_position(positions[source]));
                        if colors.is_some() {
                            source_normals.push(building.local_normal(part.normals[index]));
                        }
                    }
                    if let Some(uvs) = secondary_uvs {
                        let position = building
                            .as_ref()
                            .map_or(positions[source], |b| b.local_position(positions[source]));
                        facade_inputs
                            .entry(key)
                            .or_default()
                            .push((position, uvs[source]));
                    }
                    let [x, y, z] = place(positions[source])?;
                    let [u, v] = uvs[source];
                    let rgb =
                        |color: u32| [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.);
                    let [r, g, b] = if let Some(colors) = &colors {
                        specular_batches
                            .entry(key)
                            .or_default()
                            .push(rgb(colors[index][1]));
                        rgb(colors[index][0])
                    } else {
                        diagnostic
                    };
                    batches
                        .entry(key)
                        .or_default()
                        .push([x, y, z, r, g, b, u, v]);
                }
            }
            if let Some(slot) = runtime_slot && !source_positions.is_empty() {
                runtime_spans.push(RuntimeSpan {
                    slot,
                    key,
                    draw: usize::MAX,
                    start,
                    positions: source_positions,
                    normals: source_normals,
                });
            }
            Ok(())
        };
        let facade = geometry::facade_vertices_with_seed(
            &layers,
            selection.as_ref().map_or(0, |s| s.random_state),
        );
        let positions: Vec<_> = facade.iter().map(|v| v.position).collect();
        let uvs: Vec<_> = facade.iter().map(|v| v.uv).collect();
        let secondary_uvs: Vec<_> = facade.iter().map(|v| v.secondary_uv).collect();
        let mut facade_parts = BTreeMap::<usize, Vec<[u32; 3]>>::new();
        for (quad, vertices) in facade.chunks_exact(4).enumerate() {
            let triangles = facade_parts
                .entry(vertices[0].material_offset as usize)
                .or_default();
            triangles.extend([[0, 1, 3], [0, 3, 2]].map(|t| t.map(|i| i + quad as u32 * 4)));
        }
        for (material, triangles) in facade_parts {
            let key = if let Some(base) = base {
                (1, base + material)
            } else if selection.is_some() {
                (1, fallback_texture.unwrap())
            } else {
                (0, 0)
            };
            emit(
                &positions,
                &uvs,
                Some(&secondary_uvs),
                &triangles,
                key,
                if selection.is_some() {
                    [1.; 3]
                } else {
                    [0.66, 0.53, 0.36]
                },
            )?;
        }
        let roof = roof::mesh(&layers);
        for (triangles, pitched) in [
            (&roof.triangles[..roof.pitched_triangle_start], false),
            (&roof.triangles[roof.pitched_triangle_start..], true),
        ] {
            let key = selection.as_ref().map_or((0, 0), |s| {
                (2, if pitched { s.pitched_roof } else { s.flat_roof })
            });
            emit(
                &roof.positions,
                &roof.uvs,
                None,
                triangles,
                key,
                if selection.is_some() {
                    [1.; 3]
                } else {
                    [0.38, 0.32, 0.29]
                },
            )?;
        }
        if let Some(building) = &building {
            let samples = positions
                .iter()
                .chain(&roof.positions)
                .flat_map(|p| {
                    building
                        .local_position(*p)
                        .into_iter()
                        .flat_map(f32::to_le_bytes)
                })
                .collect::<Vec<_>>();
            let definition = crate::mesh::model_definition(&samples)?;
            // Manager+5c definition, upper-Y corner at+58 -> model+b4.
            let height = f32::from_le_bytes(definition[88..92].try_into().unwrap());
            let current = category_model_heights
                .entry(building.block_id)
                .or_insert(10.);
            if height > *current {
                *current = height;
            }
            let value =
                |i: usize| f32::from_le_bytes(definition[i * 4..i * 4 + 4].try_into().unwrap());
            inspection_buildings.push(InspectionBuilding {
                category: building.block_id,
                saved_slot: saved_slots.get(ordinal).copied(),
                runtime_slot: runtime_slots.get(ordinal).copied(),
                lot: building.spec.lot,
                height: building.spec.height,
                kind: building.spec.kind,
                quality: building.spec.quality,
                saved_business: saved_businesses.get_mut(ordinal).and_then(Option::take),
                initialized_business: None,
                initialized_apartment: None,
                saved_owner_character: saved_owner_characters.get(ordinal).copied().flatten(),
                camera_entrance: camera_entrances[ordinal],
                bounds: std::array::from_fn(|i| std::array::from_fn(|j| value(i * 3 + j))),
                center: [value(24), value(25), value(26)],
                radius: value(28),
                translation: [building.center[0], 0.5, building.center[1]],
            });
        }
    }

    if scene_mode {
        let image = PeImage::parse(executable.unwrap())?;
        let mut board = Board::from_v14(&image)?;
        if let Some(style) = style { style.apply_board(&mut board)?; }
        // 0044a0a0 creates this quad; 0044faf0 places the owning scene
        // object at the block center. 004b8760/004b8870 exclude flag 1
        // objects and flag 2 ground. 00689624 is BuildingTextures index 1.
        for block in board.blocks.iter().filter(|b| b.flags & 3 == 0) {
            // Hardware path 004b8760 copies the pavement object's color.
            let rgb = [16, 8, 0].map(|shift| ((block.pavement_color >> shift) & 255) as f32 / 255.);
            for index in [0u32, 1, 2, 0, 2, 3] {
                let x = image.f32_at(0x539a58 + index * 12)?;
                let y = image.f32_at(0x539a5c + index * 12)?;
                let z = image.f32_at(0x539a60 + index * 12)?;
                let u = image.f32_at(0x539a88 + index * 8)?;
                let v = image.f32_at(0x539a8c + index * 8)?;
                batches.entry((3, 1)).or_default().push([
                    block.center[0] + x,
                    y,
                    block.center[1] - z,
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    u,
                    v,
                ]);
                if light.is_some() {
                    specular_batches.entry((3, 1)).or_default().push([0.; 3]);
                }
            }
        }
        let archive = archive.unwrap();
        let directory = ArchiveIndex::parse(directory.unwrap(), archive.len() as u64)
            .map_err(|e| e.to_string())?;
        let scenery_records =
            crate::scene_assets::scenery_textures(&image, &directory, archive, 1280)?;
        let (pages, scenery_regions) = crate::scene_assets::scenery_pages_with_loader(&scenery_records,
            |image| textures.as_deref_mut().map_or_else(
                || crate::texture::loader_pixels(image).into(), |cache| cache.pixels(image)))?;
        scenery_pages = pages;
        let registered = |name: &str| {
            let key = name.rsplit('\\').next().unwrap();
            scenery_records.iter().position(|record| {
                record.name.as_ref().is_some_and(|seen| {
                    seen.as_bytes()
                        .get(..key.len())
                        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(key.as_bytes()))
                })
            })
        };
        // 004c0200 loads these ground meshes into 00685b08. 0044fe60
        // wraps them with zero translation/rotation and unit scale, and
        // both normal city render paths draw the resulting 0068ade8 list.
        for mesh_path in [
            "gamedata2/roads/roads.tm",
            // 004bdc10 loads 00688b48; 0044faf0 instances it in 00688740.
            "gamedata2/NEW_building stuff/pavement.tm",
            "gamedata2/New_building stuff/airport_ground.tm",
            "gamedata2/New_building stuff/train_stations/pensyl_base.tm",
            "gamedata2/New_building stuff/train_stations/shortline_ground.tm",
            "gamedata2/New_building stuff/train_stations/bandO_ground.tm",
            // 004c0240 loads 0068a988; 0044ff2b..0044ff81 wraps it in
            // 00687748, drawn alongside 0068ade8 by normal 004b3870.
            "gamedata2/New_building stuff/townhall_hospital/split_park_ground.tm",
            // 004c0375..004c046d load the surrounding land and river banks
            // into 006871d0. 0044ff83 wraps them in 006885c0; both normal
            // rendering paths draw that list with the same default transform.
            "gamedata2/New_building stuff/surrounds/land&sea/left_land_beach.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/left_land_hills.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/left_land_mount.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/right_land_hill.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/right_land_beach.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/left_bank.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/right_bank1.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/right_bank2.tm",
            // Further members of the same 006871d0 -> 006885c0 list,
            // loaded at 004c0490..004c0617.
            "gamedata2/New_building stuff/townhall_hospital/split_park_walls.tm",
            "gamedata2/New_building stuff/townhall_hospital/split_towhall_part1.tm",
            "gamedata2/New_building stuff/townhall_hospital/split_towhall_part2.tm",
            "gamedata2/New_building stuff/townhall_hospital/split_towhall_part3.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/eastern_road1.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/eastern_road2.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/eastern_bridge.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/car_bridge.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/foot_bridge.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/suspension_bridge.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/museum.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/marina.tm",
            // Remaining 006885c0 scenery, loaded at 004c063a..004c09f7.
            "gamedata2/New_building stuff/airport_t1.tm",
            "gamedata2/New_building stuff/airport_t2.tm",
            "gamedata2/New_building stuff/airport_t3.tm",
            "gamedata2/New_building stuff/police.tm",
            "gamedata2/New_building stuff/school.tm",
            "gamedata2/New_building stuff/zoo.tm",
            "gamedata2/New_building stuff/university.tm",
            "gamedata2/New_building stuff/townhall_hospital/hospital.tm",
            "gamedata2/New_building stuff/train_stations/reading_rail.tm",
            "gamedata2/New_building stuff/train_stations/spilt_shortline_ground.tm",
            "gamedata2/New_building stuff/train_stations/spilt_shortline_rail.tm",
            "gamedata2/New_building stuff/train_stations/split_band0_rail.tm",
            "gamedata2/New_building stuff/train_stations/split_band0_platform.tm",
            "gamedata2/New_building stuff/train_stations/split_pensyl_rail.tm",
            "gamedata2/New_building stuff/train_stations/split_pensyl_platform.tm",
            "gamedata2/New_building stuff/stadium/stadium.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/museum_bank1.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/museum_bank2.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/pensyl_bank.tm",
            "gamedata2/New_building stuff/surrounds/river_bank/elec_bank.tm",
            "gamedata2/New_building stuff/train_stations/reading_tracks.tm",
            "gamedata2/New_building stuff/train_stations/pensyl_tracks.tm",
            "gamedata2/New_building stuff/train_stations/pensyl_tunnel.tm",
            "gamedata2/New_building stuff/train_stations/shortline_tracks.tm",
            "gamedata2/New_building stuff/train_stations/bando_tracks.tm",
            "gamedata2/New_building stuff/train_stations/bando_pillars.tm",
            "gamedata2/New_building stuff/train_stations/bando_tunnel.tm",
            "gamedata2/New_building stuff/bus_station.tm",
            // 004c5510 submits this first river/sea pair before the
            // animated water pass. It uses the recovered base-water blend.
            "gamedata2/New_building stuff/surrounds/land&sea/river.tm",
            "gamedata2/New_building stuff/surrounds/land&sea/sea.tm",
        ] {
            let entry = directory
                .find(mesh_path)
                .ok_or_else(|| format!("missing original city mesh {mesh_path}"))?;
            let mesh = crate::mesh::CachedMesh::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )?;
            let positions = mesh.positions(0)?;
            // Original004ea510 transforms every indexed UV without clamping.
            // Small excursions sample the registered border; coordinates beyond
            // a page edge use state1's whole-page wrapping (not source wrapping).
            let registrations: Vec<_> = mesh
                .texture_names
                .iter()
                .map(|name| registered(name))
                .collect();
            let rectangles: Vec<_> = registrations
                .iter()
                .map(|entry| entry.map_or([1., 1., 0., 0.], |i| scenery_regions[i].uv))
                .collect();
            let uvs = crate::scene_assets::scenery_mesh_uvs(&mesh, &rectangles)?;
            let first_family = sets.len() + 1;
            if mesh_path.ends_with("/river.tm") || mesh_path.ends_with("/sea.tm") {
                water_families.extend(first_family..first_family + mesh.texture_names.len());
                if request.suncols.is_some() {
                    if mesh.parts.len() != 1 {
                        return Err("unsupported multi-part water mesh".into());
                    }
                    let indices: Vec<_> =
                        mesh.parts[0].vertex_indices().map(|i| i as u16).collect();
                    water_meshes.push((positions.clone(), indices));
                }
            }
            let colors = if !water_families.contains(&first_family) {
                if let (Some(light), Some(palette)) = (&light, &palette) {
                    Some(
                        mesh.normal_bytes
                            .chunks_exact(12)
                            .map(|b| {
                                let normal = std::array::from_fn(|i| {
                                    f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
                                });
                                palette.shade_static(normal, light.direction)
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                } else {
                    None
                }
            } else {
                None
            };
            let folder = mesh_path.rsplit_once('/').unwrap().0;
            for (material, name) in mesh.texture_names.iter().enumerate() {
                let path = format!("{folder}/{name}");
                let entry = directory
                    .find(&path)
                    .or_else(|| directory.find(name))
                    .ok_or_else(|| format!("missing city texture {path}"))?;
                sets.push(
                    TextureSet::parse(
                        &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
                    )
                    .map_err(|e| e.to_string())?,
                );
                if let Some(i) = registrations[material] {
                    scenery_families.insert(sets.len(), scenery_regions[i].page);
                }
            }
            let instances = if mesh_path.ends_with("/pavement.tm") {
                board
                    .blocks
                    .iter()
                    .filter(|b| b.flags & 1 == 0)
                    .map(|b| (b.center, b.pavement_color))
                    .collect::<Vec<_>>()
            } else {
                vec![([0.; 2], 0xffffffff)]
            };
            for (translation, tint) in instances {
                for part in &mesh.parts {
                    if usize::from(part.texture) >= mesh.texture_names.len() {
                        return Err("city mesh texture index out of bounds".into());
                    }
                    let family = first_family + usize::from(part.texture);
                    for index in part.vertex_indices() {
                        let [x, y, z] = positions[index];
                        let [u, v] = uvs[index];
                        let unpack = |color: u32| {
                            [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.)
                        };
                        let rgb = if tint != 0xffffffff {
                            // 004f2580's constant-object-color branch bypasses normals.
                            if light.is_some() {
                                specular_batches
                                    .entry((family, 0))
                                    .or_default()
                                    .push([0.; 3]);
                            }
                            unpack(tint)
                        } else if let Some(colors) = &colors {
                            specular_batches
                                .entry((family, 0))
                                .or_default()
                                .push(unpack(colors[index][1]));
                            unpack(colors[index][0])
                        } else {
                            [1.; 3]
                        };
                        batches
                            .entry((family, 0))
                            .or_default()
                            // Original 004f2029..004f2042 expands an identity
                            // scene object as translation + [x, y, -z]. Pavement
                            // instances use their original block centers.
                            .push([
                                (f64::from(translation[0]) + f64::from(x)) as f32,
                                y,
                                (f64::from(translation[1]) - f64::from(z)) as f32,
                                rgb[0],
                                rgb[1],
                                rgb[2],
                                u,
                                v,
                            ]);
                    }
                }
            }
        }
    }

    let mut vertex_count: usize = batches.values().map(Vec::len).sum();
    if vertex_count == 0 {
        return Err("scene has no surfaces".into());
    }
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for (&(family, _), vertices) in &batches {
        // Frame buildings and block ground; distant landscape still renders.
        if scene_mode && family > 3 {
            continue;
        }
        for v in vertices {
            let p = Vec3::new(v[0], v[1], v[2]);
            minimum = minimum.min(p);
            maximum = maximum.max(p);
        }
    }
    let center = (minimum + maximum) * 0.5;
    let width = request.width;
    let height_pixels = request.height;
    // Diagnostic framing: fit every bounding-box corner into the perspective
    // frustum so the reconstructed city remains large enough to inspect.
    let direction = Vec3::new(1.1, 0.8, 1.25).normalize();
    let right = Vec3::Y.cross(direction).normalize();
    let up = direction.cross(right);
    let tangent = (45f32.to_radians() * 0.5).tan();
    let aspect = width as f32 / height_pixels as f32;
    let mut distance = 1f32;
    for x in [minimum.x, maximum.x] {
        for y in [minimum.y, maximum.y] {
            for z in [minimum.z, maximum.z] {
                let corner = Vec3::new(x, y, z) - center;
                let fit = (corner.dot(right).abs() / (tangent * aspect))
                    .max(corner.dot(up).abs() / tangent);
                distance = distance.max(corner.dot(direction) + fit);
            }
        }
    }
    let (camera_position, center) = request
        .camera
        .map(|c| (Vec3::from_array(c.eye), Vec3::from_array(c.target)))
        .unwrap_or((center + direction * distance * 1.05, center));
    let camera_up = request.camera.map_or([0., 1., 0.], |c| c.up);
    let fov = request.camera.map_or(45., |c| c.vertical_fov);
    // Normal scene clipping is applied after the original 45-degree FOV setup.
    // This overview's pose remains diagnostic until camera controls are recovered.
    let [near, far] = crate::camera::city_clip(camera_position.y)?;
    let projection = crate::camera::projection(
        fov,
        near,
        far,
        width as i32,
        height_pixels as i32,
        [0., 1., 0., 1.],
    )?;
    let view =
        crate::camera::view_with_up(camera_position.to_array(), center.to_array(), camera_up);
    let scene_view =
        crate::camera::scene_view_with_up(camera_position.to_array(), center.to_array(), camera_up);
    let facade_uvs: BTreeMap<_, Vec<_>> = facade_inputs
        .iter()
        .map(|(&key, inputs)| {
            (
                key,
                inputs
                    .iter()
                    .map(|&(position, uv)| {
                        facade_material.map_or(uv, |m| m.coordinates(&scene_view, position, uv))
                    })
                    .collect(),
            )
        })
        .collect();
    let camera = Mat4::from_cols_array(&projection) * Mat4::from_cols_array(&view);
    let mut sky_vertices = Vec::new();
    let mut cloud_vertices = Vec::new();
    let mut star_vertices = Vec::new();
    let mut star_alpha = Vec::new();
    let mut celestial_pages = Vec::new();
    let mut celestial_batches = Vec::new();
    let mut cloud_secondary_uvs = Vec::new();
    let mut cloud_alpha = Vec::new();
    let sky_direction = std::array::from_fn(|i| {
        (f64::from(center.to_array()[i]) - f64::from(camera_position.to_array()[i])) as f32
    });
    let sky_view = crate::camera::view_with_up([0.; 3], sky_direction, camera_up);
    let sky_camera = Mat4::from_cols_array(&projection) * Mat4::from_cols_array(&sky_view);
    let mut fallback_family = None;
    if let Some(suncols) = request.suncols {
        let daylight = crate::water::Daylight::from_suncols(suncols)?;
        // This is a fresh capture at the selected phase boundary. The original
        // within-phase counter and animation timeline are not persisted here.
        let sun = daylight.sun_direction(saved_phase.unwrap(), 0);
        let upper =
            crate::sky::upper_view_angle(camera_position.to_array(), center.to_array(), fov)?;
        let strips = if request.animated {
            16
        } else {
            crate::sky::visible_strips(upper)?
        };
        if strips != 0 {
            let archive = archive.unwrap();
            let index = ArchiveIndex::parse(directory.unwrap(), archive.len() as u64)
                .map_err(|e| e.to_string())?;
            let panels = crate::panels::FontPanels::from_v14(
                &PeImage::parse(executable.unwrap())?,
                &index,
                archive,
                1280,
            )?;
            // Include font, cursor and panel neighbors before page-wide mips.
            let atlas_inputs: Vec<_> = panels
                .textures
                .iter()
                .map(|image| crate::atlas::Input {
                    width: image.width,
                    height: image.height,
                    group: 0,
                    padding: 1,
                })
                .collect();
            let mut layout = crate::atlas::Layout::default();
            let regions = layout.pack(&atlas_inputs)?;
            celestial_pages.resize_with(layout.page_count(), || vec![0; 256 * 256 * 4]);
            let mut rectangles = vec![[0.; 4]; regions.len()];
            for i in crate::atlas::Layout::packing_order(&atlas_inputs) {
                let region = regions[i];
                let pixels = crate::texture::loader_pixels(&panels.textures[i]);
                let mut loaded = panels.textures[i].clone();
                loaded.header[0] = crate::texture::loader_flags(&loaded);
                loaded.texels = &pixels;
                rectangles[i] = crate::texture::write_atlas_region(
                    &loaded,
                    &mut celestial_pages[region.page],
                    256,
                    region.origin,
                    1,
                )?;
            }
            let quads = crate::sky::celestial_quads(
                camera_position.to_array(),
                scene_view,
                daylight.phase_angle(saved_phase.unwrap(), 0),
                sun,
                light.as_ref().unwrap().diffuse,
            );
            for quad in quads {
                let texture = panels
                    .billboards
                    .clone()
                    .nth(quad.texture)
                    .ok_or("missing celestial registration")?;
                let &[su, sv, ou, ov] = rectangles.get(texture).ok_or("missing celestial image")?;
                let vertices: Vec<_> = [0, 1, 2, 0, 2, 3]
                    .into_iter()
                    .map(|i| {
                        let [x, y, z] = quad.positions[i];
                        let [u, v] = quad.uv[i];
                        [
                            x,
                            y,
                            z,
                            ((quad.color >> 16) & 255) as f32 / 255.,
                            ((quad.color >> 8) & 255) as f32 / 255.,
                            (quad.color & 255) as f32 / 255.,
                            (f64::from(su) * f64::from(u) + f64::from(ou)) as f32,
                            (f64::from(sv) * f64::from(v) + f64::from(ov)) as f32,
                        ]
                    })
                    .collect();
                celestial_batches.push((vertices, regions[texture].page));
            }
            let sky = crate::sky::Sky::original();
            let colors = sky.colors(sun);
            let cloud_uvs = sky.cloud_coordinates();
            let cloud_colors = sky.cloud_colors(sun);
            for &index in &sky.indices[..strips * 384] {
                let i = usize::from(index);
                let [x, y, z] = sky.positions[i];
                let color = colors[i];
                let cloud = cloud_colors[i];
                cloud_alpha.push((cloud >> 24) as u8);
                let [u, v, s, t] = cloud_uvs[i];
                cloud_vertices.push([
                    x,
                    y,
                    z,
                    ((cloud >> 16) & 255) as f32 / 255.,
                    ((cloud >> 8) & 255) as f32 / 255.,
                    (cloud & 255) as f32 / 255.,
                    u,
                    v,
                ]);
                cloud_secondary_uvs.push([s, t]);

                sky_vertices.push([
                    x,
                    y,
                    z,
                    ((color >> 16) & 255) as f32 / 255.,
                    ((color >> 8) & 255) as f32 / 255.,
                    (color & 255) as f32 / 255.,
                    0.,
                    0.,
                ]);
            }
        }
        if strips != 0 && (request.animated || light.as_ref().is_some_and(|l| l.diffuse & 255 < 127)) {
            let image = crate::pe::PeImage::parse(executable.unwrap())?;
            for star in crate::sky::stars(&image)? {
                let [x, y, z] = star.position;
                star_vertices.push([x, y, z, 1., 1., 1., 0., 0.]);
                star_alpha.push((star.color >> 24) as u8);
            }
        }
        diagnostics.push(format!(
            "Original sky: upper view angle {upper}, {strips} strips"
        ));

        let mut phase = crate::water::WaterPhase {
            previous_tick: u32::MAX,
            phase: 0.,
        };
        let family = sets.len() + 1;
        let archive = archive.unwrap();
        let index = ArchiveIndex::parse(directory.unwrap(), archive.len() as u64)
            .map_err(|e| e.to_string())?;
        let entry = index
            .find("gamedata2/WaterTextures/textures.gtd")
            .ok_or("missing original water textures")?;
        sets.push(
            TextureSet::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .map_err(|e| e.to_string())?,
        );
        let batch = batches.entry((family, 0)).or_default();
        for (positions, indices) in &water_meshes {
            let samples = phase.fallback_vertices(positions, camera_position.to_array(), sun, 0);
            for &index in indices {
                let i = usize::from(index);
                let [x, y, z] = positions[i];
                let sample = samples[i];
                let rgb = [16, 8, 0].map(|shift| ((sample.color >> shift) & 255) as f32 / 255.);
                batch.push([x, y, -z, rgb[0], rgb[1], rgb[2], sample.uv[0], sample.uv[1]]);
            }
        }
        vertex_count += batch.len();
        fallback_family = Some(family);
        diagnostics.push(format!(
            "Fallback water: selected phase {}, phase-boundary sun {:?}, animation phase0",
            saved_phase.unwrap(),
            sun
        ));
    }
    // Both original GTD and TT loaders apply004ff7a0 before caching/upload.
    // Retain borrowed original inputs and own only the transformed pixels.
    let loaded_pixels: Vec<Vec<_>> = sets
        .iter()
        .map(|set| {
            set.textures
                .iter()
                .map(|image| textures.as_deref_mut().map_or_else(
                    || crate::texture::loader_pixels(image).into(), |cache| cache.pixels(image)))
                .collect()
        })
        .collect();
    for (set, pixels) in sets.iter_mut().zip(&loaded_pixels) {
        for (texture, pixels) in set.textures.iter_mut().zip(pixels) {
            texture.texels = pixels;
        }
    }
    let white = [255u8; 4];
    let mut draws = Vec::new();
    if !sky_vertices.is_empty() {
        draws.push(TexturedDraw {
            diffuse_alpha: None,
            view_projection: Some(sky_camera.to_cols_array()),
            vertices: &sky_vertices,
            specular: None,
            mode: SurfaceMode::Sky,
            width: 1,
            height: 1,
            rgba: &white,
            mipmaps: false,
            secondary: None,
        });
    }

    for (&(family, index), vertices) in &batches {
        let scenery_page = scenery_families.get(&family).map(|&i| &scenery_pages[i]);
        let (width, height, rgba) = if let Some(page) = scenery_page {
            (page.side, page.side, page.rgba.as_slice())
        } else if family == 0 {
            (1, 1, white.as_slice())
        } else {
            let texture = sets[family - 1]
                .textures
                .get(index)
                .ok_or("material exceeds original texture collection")?;
            (texture.width, texture.height, texture.texels)
        };
        let secondary = if family == 1 {
            facade_material.map(|m| {
                let texture = &sets[2].textures[m.texture];
                SecondaryTexture {
                    uv: &facade_uvs[&(family, index)],
                    diffuse_alpha: m.diffuse_alpha,
                    width: texture.width,
                    height: texture.height,
                    rgba: texture.texels,
                    mipmaps: true,
                }
            })
        } else {
            None
        };
        let facade_blend = secondary.is_some();
        draws.push(TexturedDraw {
            diffuse_alpha: None,
            view_projection: None,
            secondary,
            // 004c10f0 registers these square GTD images with dedicated-page flag1.
            mipmaps: (1..=3).contains(&family)
                || fallback_family == Some(family)
                || scenery_page.is_some()
                || (family > 3
                    && !water_families.contains(&family)
                    && dedicated_scenery_mipmaps(&sets[family - 1].textures[index])),
            specular: specular_batches.get(&(family, index)).map(Vec::as_slice),
            mode: if fallback_family == Some(family) {
                SurfaceMode::WaterFallback
            } else if water_families.contains(&family) {
                SurfaceMode::WaterBase
            } else if facade_blend {
                SurfaceMode::FacadeBlend
            } else if specular_batches.contains_key(&(family, index)) {
                // Main facade (building+170) uses state 2 when the original
                // day/night multitexture materials are unavailable (004b7d90).
                // State 1 draws opaque pages first with blending disabled
                // (004f22d0/004f2160); other pages retain its alpha blend.
                if family == 1
                    || family == 0
                    || scenery_page.map_or(sets[family - 1].textures[index].header[0], |page| {
                        crate::texture::uploaded_page_flags(page.flags)
                    }) & 0x8000
                        != 0
                {
                    SurfaceMode::StaticOpaque
                } else {
                    SurfaceMode::StaticLit
                }
            } else {
                SurfaceMode::Diagnostic
            },
            vertices,
            width,
            height,
            rgba,
        });
    }
    if !cloud_vertices.is_empty() {
        // 004c10f0 registers BuildingTextures in order at689620.
        let first = sets[2]
            .textures
            .get(28)
            .ok_or("missing primary cloud texture")?;
        let second = sets[2]
            .textures
            .get(27)
            .ok_or("missing secondary cloud texture")?;
        draws.push(TexturedDraw {
            view_projection: Some(sky_camera.to_cols_array()),
            vertices: &cloud_vertices,
            specular: None,
            mode: SurfaceMode::Clouds,
            diffuse_alpha: Some(&cloud_alpha),
            width: first.width,
            height: first.height,
            rgba: first.texels,
            mipmaps: true,
            secondary: Some(SecondaryTexture {
                uv: &cloud_secondary_uvs,
                diffuse_alpha: 127,
                width: second.width,
                height: second.height,
                rgba: second.texels,
                mipmaps: true,
            }),
        });
    }
    if !star_vertices.is_empty() {
        draws.push(TexturedDraw {
            diffuse_alpha: Some(&star_alpha),
            view_projection: Some(sky_camera.to_cols_array()),
            vertices: &star_vertices,
            specular: None,
            mode: SurfaceMode::Stars,
            width: 1,
            height: 1,
            rgba: &white,
            mipmaps: false,
            secondary: None,
        });
    }
    for (vertices, page) in &celestial_batches {
        draws.push(TexturedDraw {
            view_projection: Some(projection),
            vertices,
            specular: None,
            diffuse_alpha: None,
            mode: SurfaceMode::Celestial,
            width: 256,
            height: 256,
            rgba: &celestial_pages[*page],
            mipmaps: true,
            secondary: None,
        });
    }
    // Normal view004b3870 submits sky, then water, then city scenery.
    draws.sort_by_key(|draw| match draw.mode {
        SurfaceMode::Sky => 0,
        SurfaceMode::Clouds => 1,
        SurfaceMode::Stars => 2,
        SurfaceMode::Celestial => 3,
        SurfaceMode::WaterBase => 4,
        SurfaceMode::WaterFallback => 5,
        SurfaceMode::Diagnostic | SurfaceMode::StaticOpaque => 6,
        SurfaceMode::StaticLit => 7,
        SurfaceMode::FacadeBlend => 8,
        SurfaceMode::ResultParticles => 9,
        SurfaceMode::Overlay | SurfaceMode::UiDepth => 9,
    });
    // Reproduce the original FOGTABLE + WFOG device path. Camera pose remains
    // diagnostic; colors/distances come from the original SUNCOLS update.
    let fog = light.as_ref().map(|l| {
        let [start, end] = l.fog.distances();
        crate::render::EyeFog {
            color: l.fog.color,
            start,
            end,
        }
    });
    let animation = if request.animated {
        let facades = draws
            .iter()
            .enumerate()
            .filter_map(|(draw_index, draw)| {
                if draw.mode != SurfaceMode::FacadeBlend {
                    return None;
                }
                batches
                    .iter()
                    .find(|(_, vertices)| vertices.as_ptr() == draw.vertices.as_ptr())
                    .and_then(|(key, _)| facade_inputs.remove(key))
                    .map(|inputs| (draw_index, inputs))
            })
            .collect();
        let sky = if let Some(bytes) = request.suncols {
            let daylight = crate::water::Daylight::from_suncols(bytes)?;
            let phase = saved_phase.unwrap();
            Some((
                daylight.sun_direction(phase, 0),
                daylight.phase_angle(phase, 0),
                light.as_ref().unwrap().diffuse,
            ))
        } else {
            None
        };
        let clouds = draws.iter().enumerate().find(|(_, draw)| draw.mode == SurfaceMode::Clouds)
            .map(|(index, draw)| {
                let secondary = draw.secondary.as_ref().unwrap();
                let coordinates = draw.vertices.iter().zip(secondary.uv.iter())
                    .map(|(vertex, uv)| [vertex[6], vertex[7], uv[0], uv[1]])
                    .collect();
                (index, crate::sky::CloudMotion::default(), coordinates)
            });
        Some(Animation {
            width,
            height: height_pixels,
            facades,
            facade_material,
            water_meshes,
            water: crate::water::WaterPhase {
                previous_tick: u32::MAX,
                phase: 0.,
            },
            sky,
            suncolors: suncolors.map(Arc::new),
            daylight_time: saved_phase.map(|phase| (phase, 0)),
            clouds,
        })
    } else {
        None
    };
    let mut images = BTreeMap::<(usize, usize), Arc<[u8]>>::new();
    let mut own_image = |bytes: &[u8]| {
        images
            .entry((bytes.as_ptr() as usize, bytes.len()))
            .or_insert_with(|| Arc::from(bytes))
            .clone()
    };
    let ground_colors = match (&palette, &light) {
        (Some(palette), Some(light)) => palette.shade_static([0., 1., 0.], light.direction)?,
        _ => [0xffffffff, 0xff000000],
    };
    let runtime_patches = if runtime_spans.is_empty() {
        None
    } else {
        for span in &mut runtime_spans {
            let vertices = batches.get(&span.key).ok_or("runtime material batch missing")?;
            span.draw = draws
                .iter()
                .position(|draw| draw.vertices.as_ptr() == vertices.as_ptr())
                .ok_or("runtime material draw missing")?;
        }
        Some(RuntimePatches {
            spans: runtime_spans,
            sunlight: light.as_ref().map_or([0.; 3], |sample| sample.direction),
            palette,
        })
    };
    let draws = draws
        .into_iter()
        .map(|draw| OwnedDraw {
            active_vertices: None,
            view_projection: draw.view_projection,
            vertices: draw.vertices.to_vec(),
            specular: draw.specular.map(<[_]>::to_vec),
            diffuse_alpha: draw.diffuse_alpha.map(<[_]>::to_vec),
            mode: draw.mode,
            width: draw.width,
            height: draw.height,
            rgba: own_image(draw.rgba),
            mipmaps: draw.mipmaps,
            secondary: draw.secondary.map(|s| OwnedSecondary {
                uv: s.uv.to_vec(),
                diffuse_alpha: s.diffuse_alpha,
                width: s.width,
                height: s.height,
                rgba: own_image(s.rgba),
                mipmaps: s.mipmaps,
            }),
        })
        .collect();
    Ok(Scene {
        draws,
        camera: camera.to_cols_array(),
        fog,
        building_count,
        category_model_heights,
        inspection_buildings,
        saved_category_owners,
        saved_business_catalog,
        saved_commodity_rows,
        saved_players,
        vertex_count,
        diagnostics,
        animation,
        runtime_patches,
        ground_colors,
    })
}

type FacadeInputs = Vec<([f32; 3], [f32; 2])>;

#[derive(Clone)]
struct Animation {
    width: u32,
    height: u32,
    facades: Vec<(usize, FacadeInputs)>,
    facade_material: Option<FacadeMaterial>,
    water_meshes: Vec<(Vec<[f32; 3]>, Vec<u16>)>,
    water: crate::water::WaterPhase,
    sky: Option<([f32; 3], f32, u32)>,
    suncolors: Option<Arc<crate::lighting::SunColors>>,
    daylight_time: Option<(u32, u32)>,
    clouds: Option<(usize, crate::sky::CloudMotion, Vec<[f32; 4]>)>,
}

impl Scene {
    /// Original ground glyphs are generated with +Y normals at4e8850 and
    /// use4f2580's ordinary palette branch after first-use registration.
    pub fn ground_shading(&self) -> Result<[u32; 2], String> {
        Ok(self.ground_colors)
    }
    /// Reuse the exact original material/scenery layout when only initialized
    /// building auxiliary translations or scales changed. Returns `None` when
    /// any other record byte changed, so the caller can rebuild structurally.
    pub fn patch_runtime_poses(
        &mut self,
        before: &[[u8; 0x394]],
        after: &[[u8; 0x394]],
    ) -> Result<Option<Vec<usize>>, String> {
        let Some(patches) = &self.runtime_patches else {
            return Ok(None);
        };
        if before.len() != after.len() {
            return Ok(None);
        }
        let mut changed_slots = Vec::new();
        for (slot, (old, new)) in before.iter().zip(after).enumerate() {
            if old == new {
                continue;
            }
            if old[..0x58] != new[..0x58]
                || old[0x64..0x70] != new[0x64..0x70]
                || old[0x7c..] != new[0x7c..]
            {
                return Ok(None);
            }
            changed_slots.push(slot);
        }
        let unpack = |color: u32| {
            [16, 8, 0].map(|shift| ((color >> shift) & 255) as f32 / 255.)
        };
        let mut changed_draws = Vec::new();
        for slot in changed_slots {
            let pose = BuildingPose::from_record(&after[slot])?;
            let sunlight = pose.sunlight(patches.sunlight);
            for span in patches.spans.iter().filter(|span| span.slot == slot) {
                let draw = self.draws.get_mut(span.draw).ok_or("runtime pose draw missing")?;
                let end = span.start + span.positions.len();
                let vertices = draw.vertices.get_mut(span.start..end)
                    .ok_or("runtime pose span leaves material draw")?;
                if let Some(palette) = &patches.palette {
                    if span.normals.len() != vertices.len() {
                        return Err("runtime pose normal span mismatch".into());
                    }
                    let specular = draw.specular.as_mut()
                        .and_then(|values| values.get_mut(span.start..end))
                        .ok_or("runtime pose specular span missing")?;
                    for (((vertex, highlight), &position), &normal) in vertices.iter_mut()
                        .zip(specular).zip(&span.positions).zip(&span.normals) {
                        vertex[..3].copy_from_slice(&pose.position(position)?);
                        let [diffuse, shine] = palette.shade_static(normal, sunlight)?;
                        vertex[3..6].copy_from_slice(&unpack(diffuse));
                        *highlight = unpack(shine);
                    }
                } else {
                    for (vertex, &position) in vertices.iter_mut().zip(&span.positions) {
                        vertex[..3].copy_from_slice(&pose.position(position)?);
                    }
                }
                changed_draws.push(span.draw);
            }
        }
        changed_draws.sort_unstable();
        changed_draws.dedup();
        Ok(Some(changed_draws))
    }

    /// Recompute original 004c5a70/004c4840 daylight at the retained
    /// 00822950 hour and 00822948 within-hour counter. Static building
    /// palettes are rebuilt on the existing hourly snapshot boundary.
    pub fn advance_daylight(&mut self, phase: u32, counter: u32,
        camera: crate::flyby::CameraSample) -> Result<Vec<usize>, String> {
        let state = self.animation.as_mut().ok_or("scene was not prepared for animation")?;
        if state.daylight_time == Some((phase, counter)) { return Ok(Vec::new()); }
        let colors = state.suncolors.as_ref().ok_or("animated scene has no sun colors")?;
        let light = colors.sample(phase, counter)?;
        let sun = light.direction;
        self.ground_colors = crate::lighting::Palette::new(
            light.diffuse, light.ambient, light.diffuse, 0, light.scale,
        ).shade_static([0., 1., 0.], sun)?;
        state.sky = Some((sun, colors.daylight.phase_angle(phase, counter), light.diffuse));
        state.daylight_time = Some((phase, counter));
        let [start, end] = light.fog.distances();
        self.fog = Some(crate::render::EyeFog { color: light.fog.color, start, end });
        let sky = crate::sky::Sky::original();
        let sky_colors = sky.colors(sun);
        let cloud_colors = sky.cloud_colors(sun);
        let view = crate::camera::scene_view_with_up(camera.eye, camera.target, camera.up);
        let quads = crate::sky::celestial_quads(camera.eye, view,
            colors.daylight.phase_angle(phase, counter), sun, light.diffuse);
        let mut changed = Vec::new();
        let mut celestial = 0;
        for (index, draw) in self.draws.iter_mut().enumerate() {
            match draw.mode {
                SurfaceMode::Sky | SurfaceMode::Clouds => {
                    if draw.vertices.len() != sky.indices.len() {
                        return Err("animated sky vertex topology changed".into());
                    }
                    let cloud = draw.mode == SurfaceMode::Clouds;
                    for (vertex, &source) in draw.vertices.iter_mut().zip(&sky.indices) {
                        let color = if cloud { cloud_colors[usize::from(source)] }
                            else { sky_colors[usize::from(source)] };
                        for (component, shift) in [16, 8, 0].into_iter().enumerate() {
                            vertex[3 + component] = ((color >> shift) & 255) as f32 / 255.;
                        }
                    }
                    if cloud {
                        let alpha = draw.diffuse_alpha.as_mut().ok_or("cloud alpha missing")?;
                        if alpha.len() != sky.indices.len() {
                            return Err("animated cloud alpha topology changed".into());
                        }
                        for (out, &source) in alpha.iter_mut().zip(&sky.indices) {
                            *out = (cloud_colors[usize::from(source)] >> 24) as u8;
                        }
                    }
                    changed.push(index);
                }
                SurfaceMode::Celestial => {
                    if draw.vertices.len() != 6 || celestial >= quads.len() {
                        return Err("animated celestial vertex topology changed".into());
                    }
                    for (vertex, corner) in draw.vertices.iter_mut().zip([0, 1, 2, 0, 2, 3]) {
                        vertex[..3].copy_from_slice(&quads[celestial].positions[corner]);
                        let color = quads[celestial].color;
                        for (component, shift) in [16, 8, 0].into_iter().enumerate() {
                            vertex[3 + component] = ((color >> shift) & 255) as f32 / 255.;
                        }
                    }
                    celestial += 1;
                    changed.push(index);
                }
                SurfaceMode::Stars => {
                    let strips = crate::sky::visible_strips(crate::sky::upper_view_angle(
                        camera.eye, camera.target, camera.vertical_fov)?)?;
                    let visible = strips != 0 && light.diffuse & 255 < 127;
                    let count = if visible { draw.vertices.len() } else { 0 };
                    if draw.active_vertices != Some(count) {
                        draw.active_vertices = Some(count);
                        changed.push(index);
                    }
                }
                SurfaceMode::WaterFallback => {
                    let tick = if state.water.previous_tick == u32::MAX { 0 }
                        else { state.water.previous_tick };
                    update_fallback_water(draw, state, camera.eye, tick)?;
                    changed.push(index);
                }
                _ => (),
            }
        }
        Ok(changed)
    }
    /// Advance original 822d40-driven water and cloud coordinates. Reframing
    /// the camera at the same logic tick recomputes reflection colors without
    /// advancing either phase again.
    pub fn advance_environment(&mut self, tick: u32, camera: [f32; 3]) -> Result<Vec<usize>, String> {
        let state = self.animation.as_mut().ok_or("scene was not prepared for animation")?;
        let mut changed = Vec::new();
        if let Some((index, motion, coordinates)) = &mut state.clouds
            && motion.previous_tick != tick {
                motion.advance(tick, coordinates);
                let draw = &mut self.draws[*index];
                let secondary = draw.secondary.as_mut().ok_or("cloud secondary layer missing")?;
                if draw.vertices.len() != coordinates.len() || secondary.uv.len() != coordinates.len() {
                    return Err("retained cloud vertex topology changed".into());
                }
                for ((vertex, uv), value) in draw.vertices.iter_mut()
                    .zip(secondary.uv.iter_mut()).zip(coordinates.iter()) {
                    vertex[6..8].copy_from_slice(&value[..2]);
                    *uv = [value[2], value[3]];
                }
                changed.push(*index);
        }
        for (index, draw) in self.draws.iter_mut().enumerate() {
            if draw.mode == SurfaceMode::WaterFallback {
                update_fallback_water(draw, state, camera, tick)?;
                changed.push(index);
            }
        }
        Ok(changed)
    }

    /// A refreshed city snapshot owns new static materials, while the normal
    /// clock and cloud/water motion continue from the previously drawn scene.
    pub fn retain_environment(&mut self, previous: &Scene) -> Result<(), String> {
        let (Some(state), Some(prior)) = (&mut self.animation, &previous.animation) else {
            return Ok(());
        };
        state.water = prior.water;
        if let (Some((index, motion, coordinates)), Some((_, old_motion, old_coordinates))) =
            (&mut state.clouds, &prior.clouds) {
            if coordinates.len() != old_coordinates.len() {
                return Err("refreshed cloud vertex topology changed".into());
            }
            *motion = *old_motion;
            coordinates.copy_from_slice(old_coordinates);
            let draw = &mut self.draws[*index];
            let secondary = draw.secondary.as_mut().ok_or("cloud secondary layer missing")?;
            for ((vertex, uv), value) in draw.vertices.iter_mut()
                .zip(secondary.uv.iter_mut()).zip(coordinates.iter()) {
                vertex[6..8].copy_from_slice(&value[..2]);
                *uv = [value[2], value[3]];
            }
        }
        Ok(())
    }

    /// Advance camera-dependent geometry with an explicit original camera pose.
    /// The initial retained scene must be uploaded before the first reframe so
    /// its complete sky buffers reserve capacity for every view direction.
    pub fn reframe(&mut self, sample: crate::flyby::CameraSample) -> Result<Vec<usize>, String> {
        validate_camera(sample)?;
        let state = self
            .animation
            .as_mut()
            .ok_or("scene was not prepared for animation")?;
        let [near, far] = crate::camera::city_clip(sample.eye[1])?;
        let projection = crate::camera::projection(
            sample.vertical_fov,
            near,
            far,
            state.width as i32,
            state.height as i32,
            [0., 1., 0., 1.],
        )?;
        let view = crate::camera::view_with_up(sample.eye, sample.target, sample.up);
        let scene_view = crate::camera::scene_view_with_up(sample.eye, sample.target, sample.up);
        self.camera =
            (Mat4::from_cols_array(&projection) * Mat4::from_cols_array(&view)).to_cols_array();
        let direction = std::array::from_fn(|i| {
            (f64::from(sample.target[i]) - f64::from(sample.eye[i])) as f32
        });
        let sky_view = crate::camera::view_with_up([0.; 3], direction, sample.up);
        let sky_camera =
            (Mat4::from_cols_array(&projection) * Mat4::from_cols_array(&sky_view)).to_cols_array();
        let mut changed = Vec::new();
        for (index, inputs) in &state.facades {
            let uv = &mut self.draws[*index].secondary.as_mut().unwrap().uv;
            for (out, &(position, original_uv)) in uv.iter_mut().zip(inputs) {
                *out =
                    state
                        .facade_material
                        .unwrap()
                        .coordinates(&scene_view, position, original_uv);
            }
            changed.push(*index);
        }
        let strips = crate::sky::visible_strips(crate::sky::upper_view_angle(
            sample.eye,
            sample.target,
            sample.vertical_fov,
        )?)?;
        let quads = state.sky.map(|(sun, angle, color)| {
            crate::sky::celestial_quads(sample.eye, scene_view, angle, sun, color)
        });
        let tick = if state.water.previous_tick == u32::MAX { 0 } else { state.water.previous_tick };
        let mut celestial = 0;
        for (index, draw) in self.draws.iter_mut().enumerate() {
            match draw.mode {
                SurfaceMode::Sky | SurfaceMode::Clouds | SurfaceMode::Stars => {
                    draw.view_projection = Some(sky_camera);
                    let count = if draw.mode == SurfaceMode::Stars {
                        if strips == 0 || state.sky.is_some_and(|(_, _, color)| color & 255 >= 127)
                            { 0 } else { draw.vertices.len() }
                    } else {
                        strips * 384
                    };
                    if draw.active_vertices != Some(count) {
                        draw.active_vertices = Some(count);
                        changed.push(index);
                    }
                }
                SurfaceMode::Celestial => {
                    draw.view_projection = Some(projection);
                    if let Some(quads) = &quads {
                        for (vertex, corner) in draw.vertices.iter_mut().zip([0, 1, 2, 0, 2, 3]) {
                            vertex[..3].copy_from_slice(&quads[celestial].positions[corner]);
                        }
                        celestial += 1;
                        draw.active_vertices = Some(if strips == 0 { 0 } else { 6 });
                        changed.push(index);
                    }
                }
                SurfaceMode::WaterFallback => {
                    update_fallback_water(draw, state, sample.eye, tick)?;
                    changed.push(index);
                }
                _ => (),
            }
        }
        Ok(changed)
    }
}

fn update_fallback_water(
    draw: &mut OwnedDraw,
    state: &mut Animation,
    camera: [f32; 3],
    tick: u32,
) -> Result<(), String> {
    let Some((sun, _, _)) = state.sky else { return Ok(()); };
    let mut cursor = 0;
    for (positions, indices) in &state.water_meshes {
        let samples = state.water.fallback_vertices(positions, camera, sun, tick);
        for &i in indices {
            let sample = samples[usize::from(i)];
            let vertex = draw.vertices.get_mut(cursor)
                .ok_or("retained fallback water vertex topology changed")?;
            for (component, shift) in [16, 8, 0].into_iter().enumerate() {
                vertex[3 + component] = ((sample.color >> shift) & 255) as f32 / 255.;
            }
            vertex[6..8].copy_from_slice(&sample.uv);
            cursor += 1;
        }
    }
    if cursor != draw.vertices.len() {
        return Err("retained fallback water vertex count changed".into());
    }
    Ok(())
}

fn validate_camera(sample: crate::flyby::CameraSample) -> Result<(), String> {
    if !sample
        .eye
        .into_iter()
        .chain(sample.target)
        .chain(sample.up)
        .all(f32::is_finite)
        || !sample.vertical_fov.is_finite()
        || sample.vertical_fov <= 0.
        || sample.vertical_fov >= 180.
        || (Vec3::from_array(sample.target) - Vec3::from_array(sample.eye))
            .cross(Vec3::from_array(sample.up))
            .length_squared()
            == 0.
    {
        return Err("camera requires finite independent direction/up and FOV in 0..180".into());
    }
    Ok(())
}

/// 004ffde0 selects dedicated TT registration from flag2. 004de8b0 allocates
/// a square page of max(width,height); square power-of-two sources fill it,
/// preserving their UVs and making the existing page mip reducer applicable.
/// Shared atlas and rectangular dedicated sources still need page assembly.
fn dedicated_scenery_mipmaps(texture: &crate::texture::TextureRecord<'_>) -> bool {
    texture.header[0] & 2 != 0 && texture.width == texture.height && texture.width.is_power_of_two()
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "private original auxiliary sunlight transform oracle"]
    fn original_building_pose_sunlight() {
        let path =
            std::path::PathBuf::from(std::env::var("OPENMT2001_NATIVE_BUILDING_POSE").unwrap());
        let b = std::fs::read(path.with_file_name("native-building-pose-light.bin")).unwrap();
        assert_eq!(&b[..8], b"MTBLIGHT");
        let count = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        assert_eq!(b.len(), 12 + count * 28);
        for row in b[12..].chunks_exact(28) {
            let value = |at| f32::from_le_bytes(row[at..at + 4].try_into().unwrap());
            let pose = super::BuildingPose {
                translation: [0.; 3],
                scale: [value(0); 3],
            };
            let sun = std::array::from_fn(|i| value(4 + i * 4));
            let mut actual = pose.sunlight(sun);
            actual[2] = -actual[2];
            let expected: [f32; 3] = std::array::from_fn(|i| value(16 + i * 4));
            // Signed zero from the original three-term dot product has no palette effect.
            assert_eq!(actual, expected);
        }
    }
    #[test]
    #[ignore = "private original auxiliary matrix and vertex oracle"]
    fn original_building_pose() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_BUILDING_POSE").unwrap()).unwrap();
        assert_eq!(&b[..8], b"MTBLPOSE");
        let word = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let count = word(8) as usize;
        let points = word(12) as usize;
        let stride = 64 + points * 24;
        assert_eq!(b.len(), 16 + count * stride);
        for row in b[16..].chunks_exact(stride) {
            let value = |at| f32::from_le_bytes(row[at..at + 4].try_into().unwrap());
            let pose = super::BuildingPose {
                translation: std::array::from_fn(|i| value(i * 4)),
                scale: [value(12); 3],
            };
            for i in 0..points {
                let input = std::array::from_fn(|j| value(64 + i * 12 + j * 4));
                let expected: [f32; 3] =
                    std::array::from_fn(|j| value(64 + points * 12 + i * 12 + j * 4));
                assert_eq!(
                    pose.position(input).unwrap().map(f32::to_bits),
                    expected.map(f32::to_bits)
                );
            }
        }
    }
    use super::*;

    #[test]
    fn completed_building_uses_unit_model_scale() {
        let mut record = [0u8; 0x394];
        record[0x14..0x18].copy_from_slice(&4u32.to_le_bytes());
        assert_eq!(BuildingPose::from_record(&record).unwrap().scale, [1.; 3]);
        record[0x14..0x18].copy_from_slice(&3u32.to_le_bytes());
        for axis in 0..3 {
            record[0x70 + axis * 4..0x74 + axis * 4]
                .copy_from_slice(&0.25f32.to_le_bytes());
        }
        assert_eq!(BuildingPose::from_record(&record).unwrap().scale, [0.25; 3]);
    }

    #[test]
    #[ignore = "requires original v1.4 building geometry and material assets"]
    fn runtime_construction_pose_patch_matches_full_original_scene_assembly() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let save_bytes = read("OPENMT2001_SAVE_ONE");
        let save = crate::save::SaveFile::parse(&save_bytes).unwrap();
        let image = PeImage::parse(&executable).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let file = BuildFile::parse(&build_file).unwrap();
        let mut before = vec![[0u8; 0x394]; 600];
        for saved in save.buildings().unwrap() {
            let building = SceneBuilding::from_saved(&saved, &board, &file).unwrap();
            let record = &mut before[saved.slot];
            record[..0x40].copy_from_slice(saved.bytes);
            // Reuse the original saved geometry as a pose fixture while
            // exercising the pending-world construction stage, whose +0x70
            // scale is live. Complete status 4 uses the unit model matrix.
            record[0x14..0x18].copy_from_slice(&3u32.to_le_bytes());
            assert_ne!(record[0x3f], 0);
            let category = crate::building_geometry::V14_CATEGORY_BASE
                + saved.category as u32 * 0x17c;
            record[0x40..0x44].copy_from_slice(&category.to_le_bytes());
            record[0x170..0x174].copy_from_slice(&1u32.to_le_bytes());
            for (axis, value) in [building.center[0], 0.5, building.center[1]]
                .into_iter().enumerate() {
                record[0x58 + axis * 4..0x5c + axis * 4]
                    .copy_from_slice(&value.to_le_bytes());
                record[0x70 + axis * 4..0x74 + axis * 4]
                    .copy_from_slice(&1f32.to_le_bytes());
            }
        }
        let assemble_runtime = |records: &[[u8; 0x394]]| {
            assemble(Request {
                build_file: &build_file,
                source: Source::RuntimeBuildings {
                    records,
                    category_base: crate::building_geometry::V14_CATEGORY_BASE,
                },
                original: Some(OriginalAssets {
                    executable: &executable,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: Some(&suncols),
                phase: Some(9),
                camera: None,
                width: 1280,
                height: 960,
                animated: false,
            }).unwrap()
        };
        let mut patched = assemble_runtime(&before);
        #[cfg(feature = "capture")]
        let (renderer, mut prepared) = {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(crate::render::Renderer::new(descriptor)).unwrap();
            let draws = patched.draws.iter().map(OwnedDraw::borrowed).collect::<Vec<_>>();
            let prepared = renderer.prepare(&draws, patched.camera, patched.fog).unwrap();
            (renderer, prepared)
        };
        let mut after = before.clone();
        let slot = after.iter().position(|row| row[0x3f] != 0).unwrap();
        for axis in 0..3 {
            after[slot][0x70 + axis * 4..0x74 + axis * 4]
                .copy_from_slice(&0.5f32.to_le_bytes());
        }
        let started = std::time::Instant::now();
        let changed = patched.patch_runtime_poses(&before, &after).unwrap().unwrap();
        let patch_time = started.elapsed();
        assert!(!changed.is_empty());
        #[cfg(feature = "capture")]
        for &index in &changed {
            let draw = &patched.draws[index];
            renderer.update_dynamic_surface(&mut prepared, index, &draw.borrowed(),
                draw.view_projection.unwrap_or(patched.camera), patched.fog).unwrap();
        }
        let started = std::time::Instant::now();
        let rebuilt = assemble_runtime(&after);
        eprintln!("saved-building pose patch {patch_time:?}, full assembly {:?}", started.elapsed());
        assert_eq!(patched.draws.len(), rebuilt.draws.len());
        for (index, (partial, full)) in patched.draws.iter().zip(&rebuilt.draws).enumerate() {
            assert_eq!(partial.vertices.len(), full.vertices.len(), "draw {index} vertex count");
            for (at, (a, b)) in partial.vertices.iter().zip(&full.vertices).enumerate() {
                assert_eq!(a.map(f32::to_bits), b.map(f32::to_bits),
                    "draw {index} vertex {at}");
            }
            assert_eq!(
                partial.specular.as_ref().map(|values| values.iter().map(|v| v.map(f32::to_bits)).collect::<Vec<_>>()),
                full.specular.as_ref().map(|values| values.iter().map(|v| v.map(f32::to_bits)).collect::<Vec<_>>()),
                "draw {index} specular",
            );
        }
        let mut structural = after.clone();
        structural[slot][0x2d] ^= 1;
        assert!(patched.patch_runtime_poses(&after, &structural).unwrap().is_none());
    }

    #[test]
    #[ignore = "requires original v1.4 building and material assets"]
    fn quoted_building_uses_world_pose_and_original_materials() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_MAP");
        let image = PeImage::parse(&executable).unwrap();
        let board = Board::from_v14(&image).unwrap();
        let file = BuildFile::parse(&build_file).unwrap();
        let spec = scene::parse_static_map(std::str::from_utf8(&map).unwrap())
            .unwrap().into_iter().next().unwrap();
        let building = SceneBuilding::resolve(spec, &board, &file).unwrap();
        let center = building.center;
        let result = assemble(Request {
            build_file: &build_file,
            source: Source::Preview(building),
            original: Some(OriginalAssets {
                executable: &executable,
                directory: &directory,
                archive: &archive,
            }),
            suncols: Some(&suncols),
            phase: Some(9),
            camera: None,
            width: 1280,
            height: 960,
            animated: false,
        }).unwrap();
        assert_eq!(result.building_count, 1);
        assert_eq!(result.inspection_buildings.len(), 1);
        assert_eq!(result.inspection_buildings[0].translation,
            [center[0], 0.5, center[1]]);
        assert!(result.draws.iter().any(|draw| {
            !draw.vertices.is_empty() && draw.width > 1 && draw.height > 1
                && draw.rgba.len() == (draw.width * draw.height * 4) as usize
        }), "quoted building produced no textured geometry");
    }

    #[test]
    #[ignore = "requires original scenery page observations and archive"]
    fn matches_native_dedicated_scenery_pages() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index =
            ArchiveIndex::parse(&read("OPENMT2001_ARCHIVE_DIR"), archive.len() as u64).unwrap();
        let bytes = read("OPENMT2001_NATIVE_SCENERY_PAGES");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(&bytes[..8], b"MTSCDP02");
        assert_eq!((word(8), word(12), word(16)), (122, 122, 0x027f));
        let mut expected = BTreeMap::new();
        let mut at = 20;
        for _ in 0..word(8) {
            let name_len = word(at) as usize;
            let source_index = word(at + 4) as usize;
            assert_eq!(word(at + 8), 1, "native dedicated registration");
            at += 22; // source identity, group and six registration settings
            let name = std::str::from_utf8(&bytes[at..at + name_len]).unwrap();
            at += name_len;
            let entry = index.find(name).unwrap();
            let mut set = TextureSet::parse(
                &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
            )
            .unwrap();
            let texture = &mut set.textures[source_index];
            texture.header[0] |= (entry.metadata[4] >> 16) & 2;
            assert!(dedicated_scenery_mipmaps(texture), "{name}");
            assert_eq!(word(at), crate::texture::loader_flags(texture), "{name}");
            assert_eq!(
                (word(at + 4), word(at + 8)),
                (texture.width, texture.height)
            );
            let page = word(at + 12);
            assert_eq!((word(at + 16), word(at + 20)), (0, 0), "{name}");
            assert_eq!([word(at + 24), word(at + 28)], [1f32.to_bits(); 2]);
            assert_eq!([word(at + 32), word(at + 36)], [0; 2]);
            at += 40;
            let pixels = crate::texture::loader_pixels(texture);
            assert_eq!(pixels, bytes[at..at + pixels.len()], "{name}");
            at += pixels.len();
            let mut levels = vec![pixels];
            levels.extend(
                crate::texture::page_mipmaps(texture.width, texture.height, &levels[0]).unwrap(),
            );
            assert!(expected.insert(page, (texture.width, levels)).is_none());
        }
        for page in 0..word(12) {
            let (side, levels) = &expected[&page];
            assert_eq!((word(at + 8), word(at + 12)), (*side, *side));
            assert_eq!(word(at + 16) as usize, levels.len());
            at += 20;
            for level in levels {
                assert_eq!(level, &bytes[at..at + level.len()], "page {page}");
                at += level.len();
            }
        }
        assert_eq!(at, bytes.len());
    }

    #[test]
    #[ignore = "requires original intro scene assets and flyby track"]
    fn animated_reframe_matches_fresh_scene() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let track = read("OPENMT2001_INTRO_TRACK");
        let track = crate::flyby::Flyby::parse(&track).unwrap();
        let request = |frame, animated| Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets {
                executable: &executable,
                directory: &directory,
                archive: &archive,
            }),
            suncols: Some(&suncols),
            phase: Some(20),
            camera: Some(track.sample(frame).unwrap()),
            width: 1280,
            height: 960,
            animated,
        };
        let mut animated = assemble(request(1, true)).unwrap();
        for frame in [1, 1000, 6000] {
            animated.reframe(track.sample(frame).unwrap()).unwrap();
            let fresh = assemble(request(frame, false)).unwrap();
            assert_eq!(
                animated.camera.map(f32::to_bits),
                fresh.camera.map(f32::to_bits)
            );
            let active: Vec<_> = animated
                .draws
                .iter()
                .map(OwnedDraw::borrowed)
                .filter(|d| !d.vertices.is_empty())
                .collect();
            let expected: Vec<_> = fresh
                .draws
                .iter()
                .map(OwnedDraw::borrowed)
                .filter(|d| !d.vertices.is_empty())
                .collect();
            assert_eq!(active.len(), expected.len(), "frame {frame}");
            for (actual, expected) in active.iter().zip(&expected) {
                assert_eq!(actual.mode, expected.mode);
                assert_eq!(actual.view_projection, expected.view_projection);
                assert_eq!(
                    bytemuck::cast_slice::<_, u8>(actual.vertices),
                    bytemuck::cast_slice::<_, u8>(expected.vertices),
                    "frame {frame}, {:?}",
                    actual.mode
                );
                assert_eq!(actual.specular, expected.specular);
                assert_eq!(actual.diffuse_alpha, expected.diffuse_alpha);
                assert_eq!(
                    (actual.width, actual.height, actual.mipmaps),
                    (expected.width, expected.height, expected.mipmaps)
                );
                assert_eq!(actual.rgba, expected.rgba);
                match (&actual.secondary, &expected.secondary) {
                    (Some(actual), Some(expected)) => {
                        assert_eq!(actual.uv, expected.uv, "frame {frame}");
                        assert_eq!(actual.rgba, expected.rgba);
                    }
                    (None, None) => (),
                    _ => panic!("secondary surface changed"),
                }
            }
        }
    }

    #[test]
    #[ignore = "requires original intro assets and a GPU adapter"]
    fn animated_environment_advances_on_normal_logic_ticks() {
        let read = |key| std::fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let track_bytes = read("OPENMT2001_INTRO_TRACK");
        let track = crate::flyby::Flyby::parse(&track_bytes).unwrap();
        let sample = track.sample(1).unwrap();
        let make = |phase| assemble(Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets { executable: &executable,
                directory: &directory, archive: &archive }),
            suncols: Some(&suncols), phase: Some(phase), camera: Some(sample),
            width: 1280, height: 960, animated: true,
        }).unwrap();
        let mut scene = make(20);
        let cloud = scene.draws.iter().position(|draw| draw.mode == SurfaceMode::Clouds).unwrap();
        let water_draw = scene.draws.iter().position(|draw| draw.mode == SurfaceMode::WaterFallback).unwrap();
        let sky_draw = scene.draws.iter().position(|draw| draw.mode == SurfaceMode::Sky).unwrap();
        let celestial = scene.draws.iter().position(|draw| draw.mode == SurfaceMode::Celestial).unwrap();
        let layout = scene.draws.iter().map(|draw| draw.vertices.len()).collect::<Vec<_>>();
        let first_uv = scene.draws[cloud].vertices[0][6..8].to_vec();
        let first_water_uv = scene.draws[water_draw].vertices[0][6..8].to_vec();
        // Original cold image starts cloud previous_tick at zero and fallback
        // water previous_tick at FFFFFFFF. A restored clock is already loaded
        // before the first original world draw.
        let saved_tick = 12_345;
        scene.advance_environment(saved_tick, sample.eye).unwrap();
        scene.advance_daylight(6, 0, sample).unwrap();
        let morning_sky = scene.draws[sky_draw].vertices[384][3..6].to_vec();
        let morning_sun = scene.draws[celestial].vertices[0][..3].to_vec();
        let morning_fog = scene.fog.unwrap();
        let initial_water = scene.animation.as_ref().unwrap().water;
        assert_eq!(initial_water.previous_tick, saved_tick);
        assert_eq!(initial_water.phase.to_bits(), 0f32.to_bits());
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(crate::render::Renderer::new(descriptor)).unwrap();
        let mut prepared = renderer.prepare(&scene.draws.iter().map(OwnedDraw::borrowed)
            .collect::<Vec<_>>(), scene.camera, scene.fog).unwrap();
        let daylight_changed = scene.advance_daylight(6, 197, sample).unwrap();
        assert!(daylight_changed.contains(&sky_draw));
        assert!(daylight_changed.contains(&cloud));
        assert!(daylight_changed.contains(&water_draw));
        assert!(daylight_changed.contains(&celestial));
        assert_ne!(scene.draws[sky_draw].vertices[384][3..6], morning_sky);
        assert_ne!(scene.draws[celestial].vertices[0][..3], morning_sun);
        let later_fog = scene.fog.unwrap();
        assert_ne!((later_fog.color, later_fog.start.to_bits(), later_fog.end.to_bits()),
            (morning_fog.color, morning_fog.start.to_bits(), morning_fog.end.to_bits()));
        for index in daylight_changed {
            let draw = &scene.draws[index];
            renderer.update_dynamic_surface(&mut prepared, index, &draw.borrowed(),
                draw.view_projection.unwrap_or(scene.camera), scene.fog).unwrap();
        }
        for (index, draw) in scene.draws.iter().enumerate() {
            renderer.update_camera(&prepared, index,
                draw.view_projection.unwrap_or(scene.camera), scene.fog).unwrap();
        }
        for tick in [saved_tick, saved_tick + 25, saved_tick + 50,
            saved_tick + 50, saved_tick + 100] {
            let changed = scene.advance_environment(tick, sample.eye).unwrap();
            for index in changed {
                let draw = &scene.draws[index];
                renderer.update_dynamic_surface(&mut prepared, index, &draw.borrowed(),
                    draw.view_projection.unwrap_or(scene.camera), scene.fog).unwrap();
            }
            assert_eq!(scene.draws.iter().map(|draw| draw.vertices.len()).collect::<Vec<_>>(), layout);
        }
        assert_ne!(scene.draws[cloud].vertices[0][6..8], first_uv);
        assert_ne!(scene.draws[water_draw].vertices[0][6..8], first_water_uv);
        let motion = scene.animation.as_ref().unwrap().clouds.as_ref().unwrap().1;
        assert_eq!(motion.previous_tick, saved_tick + 100);
        let water = scene.animation.as_ref().unwrap().water;
        assert_eq!(water.previous_tick, saved_tick + 100);
        scene.reframe(track.sample(1000).unwrap()).unwrap();
        assert_eq!(scene.animation.as_ref().unwrap().water.phase.to_bits(), water.phase.to_bits());
        let mut refreshed = make(20);
        refreshed.retain_environment(&scene).unwrap();
        refreshed.advance_daylight(6, 197, sample).unwrap();
        assert_eq!(refreshed.draws[sky_draw].vertices[384][3..6],
            scene.draws[sky_draw].vertices[384][3..6]);
        assert_eq!(refreshed.draws[cloud].diffuse_alpha,
            scene.draws[cloud].diffuse_alpha);
        assert_eq!(refreshed.animation.as_ref().unwrap().water.phase.to_bits(), water.phase.to_bits());
        assert_eq!(refreshed.animation.as_ref().unwrap().clouds.as_ref().unwrap().2,
            scene.animation.as_ref().unwrap().clouds.as_ref().unwrap().2);
        refreshed.advance_environment(saved_tick + 125, sample.eye).unwrap();
        assert_eq!(refreshed.animation.as_ref().unwrap().water.previous_tick, saved_tick + 125);
        assert_ne!(refreshed.animation.as_ref().unwrap().water.phase.to_bits(), water.phase.to_bits());
        let mut daytime = make(12);
        let stars = daytime.draws.iter().position(|draw| draw.mode == SurfaceMode::Stars).unwrap();
        let mut day_prepared = renderer.prepare(&daytime.draws.iter().map(OwnedDraw::borrowed)
            .collect::<Vec<_>>(), daytime.camera, daytime.fog).unwrap();
        daytime.reframe(sample).unwrap();
        assert_eq!(daytime.draws[stars].active_vertices, Some(0));
        let night_changed = daytime.advance_daylight(20, 0, sample).unwrap();
        assert_eq!(daytime.draws[stars].active_vertices, Some(daytime.draws[stars].vertices.len()));
        assert!(night_changed.contains(&stars));
        renderer.update_dynamic_surface(&mut day_prepared, stars,
            &daytime.draws[stars].borrowed(), daytime.draws[stars].view_projection.unwrap(),
            daytime.fog).unwrap();
    }
}
