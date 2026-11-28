#[path = "intro/connected_start.rs"]
pub(crate) mod connected_start;
use crate::present::Presenter;

#[path = "intro/camera_keys.rs"]
mod camera_keys;
#[path = "intro/camera_live.rs"]
pub(crate) mod camera_live;
#[path = "intro/camera_producer.rs"]
mod camera_producer;
#[path = "intro/dialog_keys.rs"]
mod dialog_keys;
#[path = "intro/focus.rs"]
pub(crate) mod focus;
#[path = "intro/frontend_draw.rs"]
mod frontend_draw;
#[path = "intro/menu_preferences.rs"]
pub(crate) mod menu_preferences;
#[path = "intro/options_ack.rs"]
mod options_ack;
#[path = "intro/pause_keys.rs"]
mod pause_keys;
#[path = "intro/process_frontend.rs"]
pub(crate) mod process_frontend;
#[path = "intro/refresh.rs"]
mod refresh;
use openmt2001::{
    flyby::{CameraSample, Flyby},
    render::{COLOR_FORMAT, DEPTH_FORMAT, PreparedScene, Renderer},
    render_scene::{self, OriginalAssets, Request, Scene, Source},
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Instant,
};

#[path = "construction_scaffold.rs"]
mod construction_scaffold;

// Normal city reset004b9d40 loads53.0 from00545acc into renderer slot14.
const NORMAL_CITY_VERTICAL_FOV: f32 = 53.;

fn fresh_city_camera() -> (CameraSample, openmt2001::camera::BlockCamera) {
    // Normal4b9fa0->4b9d40 selects mode6 and writes4b9e5f..91. Mode6
    // has no pose callback at4e02d0, so it retains these exact floats.
    (
        CameraSample {
            eye: [26., 586., -695.],
            target: [26., 463., -595.],
            up: [0., 1., 0.],
            vertical_fov: NORMAL_CITY_VERTICAL_FOV,
        },
        openmt2001::camera::BlockCamera {
            center: [51., 463., -595.],
            angle: -std::f32::consts::FRAC_PI_2,
            height: 586.,
            radius: 100.,
        },
    )
}

fn update_locator_financial(
    locator: &mut Option<crate::locator::Locator>,
    status: &crate::live_host::Status,
) -> Result<(), String> {
    if let (Some(locator), Some(snapshot)) = (locator, status.financial.as_ref()) {
        locator.apply_financial_snapshot(snapshot.clone())?;
    }
    Ok(())
}

// Normal city owners call this after generic updates and BEFORE borrowing any
// draws for prepare/refresh/upload. Full CPU star capacity remains retained.
fn normal_city_scene_updates(
    scene: &mut Scene,
    camera: CameraSample,
    detail: Option<u8>,
    mut changed: Vec<usize>,
) -> Result<Vec<usize>, String> {
    if let Some(detail) = detail {
        changed.extend(scene.apply_normal_star_detail(detail, camera)?);
    }
    changed.sort_unstable();
    changed.dedup();
    Ok(changed)
}

fn sync_water_options(
    scene: &mut Scene,
    renderer: &Renderer,
    prepared: &mut PreparedScene,
    graphics: crate::settings_menu::GraphicsLevels,
) -> Result<(), String> {
    let changed = scene.apply_water_options(graphics.water_effects, graphics.detail)?;
    let updates = changed
        .into_iter()
        .map(|i| (i, scene.draws[i].borrowed()))
        .collect::<Vec<_>>();
    renderer.update_dynamic_vertices_batch(prepared, &updates)
}

fn city_snapshot_interval(construction_active: bool) -> std::time::Duration {
    if construction_active {
        // Pending-world grows the model every 25 Hz logic tick. Four visible
        // samples per second preserve that motion without rebuilding idle cities.
        std::time::Duration::from_millis(250)
    } else {
        std::time::Duration::from_secs(1)
    }
}

pub struct Paths {
    pub compatibility: crate::compatibility::Mode,
    pub installer: PathBuf,
    pub ffmpeg: PathBuf,
    pub scene: PathBuf,
    pub executable: PathBuf,
    pub map: PathBuf,
    pub quality: String,
    pub language: PathBuf,
    pub profile_root: PathBuf,
    pub save_root: PathBuf,
}
pub struct Assets {
    //Process lifetime: moved into Intro, retained across city retirement.
    pub(crate) primary_products: camera_live::main_construction::CompletedMainLayout,
    pub(crate) ui_images: camera_live::process_images::Images,
    pub(crate) ui_text: camera_live::ui_text_resources::UiTextResources,
    pub(crate) process_runtime: crate::live_host::process_runtime::Handle,
    pub(crate) activity: crate::live_host::process_runtime::Activity,
    pub cursors: Option<crate::cursor::Images>,
    scene: Scene,
    samples: Vec<CameraSample>,
    menu: Option<crate::menu::Assets>,
    inputs: Arc<SceneInputs>,
    selection: crate::selection::Assets,
    navigation: crate::navigation::Navigation,
    navigator: crate::navigator::Navigator,
    tooltip: crate::tooltip::Tooltip,
    result_actors: crate::auction_scene::Assets,
    result_text: crate::fresh_city::result::text::Assets,
    result_firework_texture: crate::fresh_city::result::particles_gpu::FireworkTexture,
    result_medallions: crate::fresh_city::result::medallions::Assets,
    result_audio: crate::fresh_city::result::audio::Assets,
    auction_audio: crate::live_host::auction_audio::Assets,
    transport_audio: crate::transport_audio::Assets,
    era_audio: crate::era_audio::Assets,
    gameplay_sound: crate::gameplay_sound::Assets,
    time_panel: crate::time_panel::Assets,
    cash_arrow: crate::cash_arrow::Assets,
    dust: crate::dust_scene::Assets,
    game_options: crate::game_options::Assets,
    save_dialog: crate::save_dialog::Assets,
    settings_menu: crate::settings_menu::Assets,
    sidewalk_labels: crate::block_sidewalk_labels::Assets,
    tutorial_visual: crate::tutorial_visual::Assets,
    city_pulse: crate::city_pulse_scene::Assets,
    strategic_scene: crate::strategic_scene::Assets,
}
impl Assets {
    pub fn load(paths: Paths) -> Result<Self, String> {
        Self::load_with_source_options(paths, "")
    }
    pub fn load_with_source_options(paths: Paths, source_options: &str) -> Result<Self, String> {
        Self::load_primary_path(paths, source_options)
    }
    pub(crate) fn reordered_main(
        &self,
    ) -> Result<&camera_live::main_construction::CompletedMainLayout, String> {
        Ok(&self.primary_products)
    }
    fn load_primary_path(paths: Paths, source_options: &str) -> Result<Self, String> {
        let read = |path: PathBuf| fs::read(&path).map_err(|e| format!("{}: {e}", path.display()));
        let build_file = read(paths.scene.join("BuildEditorModels/BuildFile.dat"))?;
        let executable = read(paths.executable)?;
        let (process_runtime, activity) =
            crate::live_host::process_runtime::startup(&executable, source_options)?;
        let style = crate::block_sidewalk_labels::load_style(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &paths.installer,
        )?;
        let directory = read(paths.scene.join("archive.DIR"))?;
        let archive = read(paths.scene.join("archive.bin"))?;
        let maps = paths.map.parent().ok_or("intro map parent")?.to_path_buf();
        let scripts = maps.parent().ok_or("script directory missing")?;
        let map = read(paths.map)?;
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
            .map_err(|e| e.to_string())?;
        let mut ui_text = camera_live::ui_text_resources::UiTextResources::load(
            &executable,
            &paths.installer,
            &paths.language,
            &index,
            &archive,
        )?;
        let cursors = Some(crate::cursor::Images::load(&index, &archive)?);
        let mut ui_images =
            camera_live::process_images::Images::load(&executable, &index, &archive, [640, 480])?;
        let navigation =
            crate::navigation::Navigation::load_shared(&index, &archive, &executable, &ui_images)?;
        let mut selection = crate::selection::Assets::load(&executable, &index, &archive)?;
        style.apply_blocks(&mut selection.blocks)?;
        let mut navigator =
            crate::navigator::Navigator::load(&index, &archive, &selection.blocks, &executable)?;
        //The current frontend selects the supplied USA board here.
        ui_text.install_usa_board(&executable, &paths.installer)?;
        let primary_products = {
            let mut prefix = Some(camera_live::main_construction::Prefix::load(
                &executable,
                &index,
                &archive,
                [640, 480],
                &mut ui_text,
                navigator.process_primitives_mut(),
                &mut ui_images,
            )?);
            let mut child5 = Some(
                camera_live::main_construction::AwaitingChild5::continue_child1(
                    &mut prefix,
                    &executable,
                    &mut ui_text,
                    navigator.process_primitives_mut(),
                    &mut ui_images,
                )?,
            );
            let mut child2 = Some(
                camera_live::main_construction::AwaitingChild2::continue_child5(
                    &mut child5,
                    &executable,
                    &mut ui_text,
                    navigator.process_primitives_mut(),
                    &mut ui_images,
                )?,
            );
            let mut parent_tail = Some(
                camera_live::main_construction::AwaitingParentTail::continue_child2(
                    &mut child2,
                    &executable,
                    &mut ui_text,
                    navigator.process_primitives_mut(),
                    &mut ui_images,
                )?,
            );
            let main_prefix =
                camera_live::main_construction::CompletedMainLayout::continue_parent_tail(
                    &mut parent_tail,
                    &executable,
                    &mut ui_text,
                    navigator.process_primitives_mut(),
                    &mut ui_images,
                )?;
            main_prefix
        };
        let tooltip = crate::tooltip::Tooltip::load_shared(
            &index,
            &archive,
            &paths.installer,
            &paths.language,
            &ui_text,
        )?;
        let result_actors = crate::auction_scene::Assets::load_result(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &index,
            &archive,
        )?;
        let result_text = crate::fresh_city::result::text::Assets::load_shared(
            &index,
            &archive,
            &paths.installer,
            &paths.language,
            &ui_text,
        )?;
        let result_medallions = crate::fresh_city::result::medallions::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &index,
            &archive,
        )?;
        let result_audio = crate::fresh_city::result::audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &paths.installer,
            &paths.ffmpeg,
        )?;
        let auction_audio = crate::live_host::auction_audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &paths.installer,
        )?;
        let transport_audio = crate::transport_audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &paths.installer,
        )?;
        let era_audio = crate::era_audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &paths.installer,
            &paths.ffmpeg,
        )?;
        let gameplay_sound = crate::gameplay_sound::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &paths.installer,
        )?;
        let time_panel = crate::time_panel::Assets::load_shared(
            &index,
            &archive,
            &paths.installer,
            &paths.language,
            &executable,
            &ui_text,
        )?;
        let cash_arrow = crate::cash_arrow::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &index,
            &archive,
        )?;
        let dust = crate::dust_scene::Assets::load(&index, &archive)?;
        let game_options =
            crate::game_options::Assets::load(&index, &archive, &paths.installer, &paths.language)?;
        let mut save_dialog =
            crate::save_dialog::Assets::load(&index, &archive, &paths.installer, &paths.language)?;
        let save_catalog = crate::campaign_store::load_catalog(
            &openmt2001::pe::PeImage::parse(&executable)?,
            scripts,
        )?;
        save_dialog.set_level_catalog(&save_catalog);
        let settings_menu = crate::settings_menu::Assets::load(
            &index,
            &archive,
            &paths.installer,
            &paths.language,
        )?;
        let sidewalk_labels = crate::block_sidewalk_labels::Assets::load(
            &index,
            &archive,
            &paths.installer,
            &selection.blocks,
            &style,
        )?;
        let tutorial_visual = crate::tutorial_visual::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &index,
            &archive,
        )?;
        let city_pulse = crate::city_pulse_scene::Assets::load(&index, &archive)?;
        let result_firework_texture =
            crate::fresh_city::result::particles_gpu::FireworkTexture::load(&index, &archive)?;
        let strategic_scene = crate::strategic_scene::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?,
            &index,
            &archive,
            &selection.blocks,
        )?;
        let profile_root = paths.profile_root.clone();
        let menu = Some(crate::menu::Assets::load_shared(
            &index,
            &archive,
            &paths.installer,
            &paths.language,
            scripts,
            paths.profile_root,
            paths.save_root,
            &executable,
            &ui_text,
        )?);
        let suncols = read(paths.installer.join("Standard/SUNCOLS.tga"))?;
        let hub = read(scripts.join("FLYBY1/hub.lua"))?;
        // Offline original startup reseeds the static registry from CRT time.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as u32;
        let mut random_state = openmt2001::startup_rng::Snapshot::from_seed(seed).state(0x93aee8);
        let backdrop =
            openmt2001::flyby_script::select(&hub, &paths.quality, &mut random_state, || {
                process_runtime
                    .performance8b26cc()
                    .map_err(|e| e.to_string())
            })?;
        let track = read(
            paths
                .installer
                .join("Standard/Cut_scenes")
                .join(&backdrop.track),
        )?;
        let track = Flyby::parse(&track)?;
        let samples = (1..track.frame_count())
            .map(|i| track.sample(i))
            .collect::<Result<Vec<_>, _>>()?;
        let phase = backdrop.hour;
        let scene = render_scene::assemble_with_style(
            Request {
                build_file: &build_file,
                source: Source::Map(std::str::from_utf8(&map).map_err(|e| e.to_string())?),
                original: Some(OriginalAssets {
                    executable: &executable,
                    directory: &directory,
                    archive: &archive,
                }),
                suncols: Some(&suncols),
                phase: Some(phase),
                camera: Some(samples[0]),
                width: 1280,
                height: 960,
                animated: true,
            },
            Some(&style),
        )?;
        eprintln!(
            "Intro assembled: {} buildings, {} triangle vertices, {} camera samples; {} track / hour {phase}",
            scene.building_count,
            scene.vertex_count,
            samples.len(),
            backdrop.track
        );
        let inputs = Arc::new(SceneInputs {
            compatibility: paths.compatibility,
            style,
            process_runtime: Some(process_runtime.clone()),
            time_runtime: Default::default(),
            profile_root,
            character_names: crate::fresh_city::character_names(
                &openmt2001::pe::PeImage::parse(&executable)?,
                &paths.language,
            )?,
            installer: paths.installer,
            language: paths.language,
            build_file,
            executable,
            directory,
            archive,
            suncols,
            maps,
        });
        Ok(Self {
            process_runtime,
            activity,
            primary_products,
            ui_images,
            ui_text,
            cursors,
            inputs,
            scene,
            samples,
            selection,
            navigation,
            navigator,
            tooltip,
            result_actors,
            result_text,
            result_firework_texture,
            result_medallions,
            result_audio,
            auction_audio,
            transport_audio,
            era_audio,
            gameplay_sound,
            time_panel,
            cash_arrow,
            dust,
            game_options,
            save_dialog,
            settings_menu,
            sidewalk_labels,
            tutorial_visual,
            city_pulse,
            strategic_scene,
            menu,
        })
    }
}

struct SceneInputs {
    process_runtime: Option<crate::live_host::process_runtime::Handle>,
    compatibility: crate::compatibility::Mode,
    style: openmt2001::board_style::Style,
    /// Native560a1c/560a20 persist across level widget reconstruction.
    time_runtime: Arc<std::sync::Mutex<crate::time_panel::Runtime>>,
    profile_root: PathBuf,
    character_names: [Vec<u8>; 10],
    installer: PathBuf,
    language: PathBuf,
    maps: PathBuf,
    build_file: Vec<u8>,
    executable: Vec<u8>,
    directory: Vec<u8>,
    archive: Vec<u8>,
    suncols: Vec<u8>,
}

struct StaticAppearanceContext<'a, 'b> {
    board: &'a openmt2001::board::Board,
    image: &'a openmt2001::pe::PeImage<'b>,
    hour: u32,
}

/// Native 4b7fd0 submits pass-3 descriptors only after the building's
/// auxiliary model has reached unit scale. Status 4 receives a unit matrix
/// from 444ff0 even when its saved auxiliary scale bytes are zero.
pub(crate) fn detail_model_at_unit_scale(record: &[u8]) -> bool {
    let status = u32::from_le_bytes(record[0x14..0x18].try_into().unwrap());
    status == 4 || record[0x74..0x78] == 1f32.to_le_bytes()
}

/// The original483dd0 copies49 words into primary camera allocation0093af60.
/// 004e0590 interpolates eye/target; 004b4430 submits their current4c/58 triples.
/// The modern up input is at64. General original restore of that triple into
/// renderer up is unproven; SAVE_ONE matches the renderer's default[0,1,0].
/// Mode-specific orbit controls are not interchangeable, so derive V2's
/// block controls from that visible pose for subsequent human input.
fn saved_viewpoint(
    bytes: &[u8; 196],
    vertical_fov: f32,
) -> Result<(CameraSample, openmt2001::camera::BlockCamera), String> {
    let scalar = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let triple = |at| [scalar(at), scalar(at + 4), scalar(at + 8)];
    let eye = triple(0x4c);
    let target = triple(0x58);
    let up = triple(0x64);
    if eye
        .into_iter()
        .chain(target)
        .chain(up)
        .any(|x| !x.is_finite())
    {
        return Err("saved camera contains non-finite view coordinates".into());
    }
    let dx = eye[0] - target[0];
    let dz = eye[2] - target[2];
    let radius = dx.hypot(dz);
    if radius < 0.001 || up == [0.; 3] {
        return Err("saved camera has an invalid view direction or up vector".into());
    }
    let angle = dz.atan2(dx);
    let controls = openmt2001::camera::BlockCamera {
        center: [
            target[0] - angle.sin() * 25.,
            target[1],
            target[2] + angle.cos() * 25.,
        ],
        angle,
        height: eye[1],
        radius,
    };
    Ok((
        CameraSample {
            eye,
            target,
            up,
            vertical_fov,
        },
        controls,
    ))
}

// Ordinary restore copies tag100 to0093af60; normal reset supplies its FOV.
// Keep the generic saved_viewpoint helper available for other camera owners.
fn normal_saved_city_viewpoint(
    records: &[[u8; 196]; 2],
) -> Result<(CameraSample, openmt2001::camera::BlockCamera), String> {
    saved_viewpoint(&records[0], NORMAL_CITY_VERTICAL_FOV)
}

fn page_height(height: f32, up: bool) -> f32 {
    (height + if up { -10. } else { 10. }).clamp(20., height.max(400.))
}

pub(crate) fn internal_pointer(position: [f64; 2], size: [u32; 2]) -> Option<[i32; 2]> {
    let scale = (f64::from(size[0]) / 1280.).min(f64::from(size[1]) / 960.);
    if scale <= 0. {
        return None;
    }
    let point = [
        (position[0] - (f64::from(size[0]) - 1280. * scale) / 2.) / scale,
        (position[1] - (f64::from(size[1]) - 960. * scale) / 2.) / scale,
    ];
    if point[0] >= 0. && point[0] < 1280. && point[1] >= 0. && point[1] < 960. {
        Some(point.map(|v| v.round() as i32))
    } else {
        None
    }
}

/// Saved non-block views may start outside the normal block pan clamp.
/// Preserve their recorded pose during manual inspection; the arithmetic
/// mirrors BlockCamera::tick before its final ±500 center clamp.
fn saved_camera_tick(
    controls: &mut openmt2001::camera::BlockCamera,
    keys: [bool; 4],
) -> ([f32; 3], [f32; 3]) {
    let angle = f64::from(controls.angle);
    let sin = f64::from(angle.sin() as f32);
    let cos = f64::from(angle.cos() as f32);
    let target_x = f64::from(controls.center[0]) + sin * 25.;
    let target_z = (f64::from(controls.center[2]) - cos * 25.) as f32;
    let target = [target_x as f32, controls.center[1], target_z];
    let eye = [
        (angle.cos() * f64::from(controls.radius) + target_x) as f32,
        controls.height,
        (angle.sin() * f64::from(controls.radius) + f64::from(target_z)) as f32,
    ];
    for (enabled, dx, dz) in [
        (keys[0], -sin * 2., cos * 2.),
        (keys[1], sin * 2., -cos * 2.),
        (keys[2], cos * 2., sin * 2.),
        (keys[3], -cos * 2., -sin * 2.),
    ] {
        if enabled {
            controls.center[0] = (f64::from(controls.center[0]) + dx) as f32;
            controls.center[2] = (f64::from(controls.center[2]) + dz) as f32;
        }
    }
    (eye, target)
}

/// Mode 12's 50 Hz callback: emit the current keyboard pose, then apply the
/// right-button mouse delta. The horizontal orbit appears on the next tick.
fn mode12_input_tick(
    controls: &mut openmt2001::camera::BlockCamera,
    saved_camera: bool,
    keys: [bool; 4],
    drag: [i32; 2],
    sensitivity: i32,
    invert_vertical: bool,
) -> Result<([f32; 3], [f32; 3]), String> {
    mode12_input_tick_with_height(
        controls,
        saved_camera,
        keys,
        drag,
        sensitivity,
        invert_vertical,
        false,
    )
}
fn mode12_input_tick_with_height(
    controls: &mut openmt2001::camera::BlockCamera,
    saved_camera: bool,
    keys: [bool; 4],
    drag: [i32; 2],
    sensitivity: i32,
    invert_vertical: bool,
    low_height: bool,
) -> Result<([f32; 3], [f32; 3]), String> {
    let mut pose = if saved_camera {
        saved_camera_tick(controls, keys)
    } else {
        controls.tick(keys)
    };
    if drag != [0; 2] {
        controls.right_drag(
            &mut pose,
            drag,
            sensitivity,
            invert_vertical,
            false,
            low_height,
        )?;
    }
    Ok(pose)
}

#[derive(Clone)]
struct DetailSpan {
    category: usize,
    center: [f32; 2],
    draw: usize,
    start: usize,
    positions: Vec<[f32; 3]>,
    pivot_y: f32,
    presentation: Option<(bool, u32)>,
}

#[derive(Clone, Default)]
struct DetailVisibility {
    spans: Vec<DetailSpan>,
}
impl DetailVisibility {
    fn add(
        &mut self,
        scene: &Scene,
        category: usize,
        center: [f32; 2],
        draw: usize,
        vertices: std::ops::Range<usize>,
        pivot_y: f32,
    ) -> Result<(), String> {
        let source = scene.draws.get(draw).ok_or("detail span draw missing")?;
        let slice = source
            .vertices
            .get(vertices.clone())
            .ok_or("detail span leaves draw")?;
        if !slice.is_empty() {
            self.spans.push(DetailSpan {
                category,
                center,
                draw,
                start: vertices.start,
                positions: slice
                    .iter()
                    .map(|vertex| [vertex[0], vertex[1], vertex[2]])
                    .collect(),
                pivot_y,
                presentation: None,
            });
        }
        Ok(())
    }

    /// Original 4b7fd0 pass3 scales each descriptor's Y axis through the
    /// 50-unit depth band, then skips category details past far. Ordinary
    /// pass1 building materials remain submitted at every setting.
    fn reframe(
        &mut self,
        scene: &mut Scene,
        camera: CameraSample,
        distance: u8,
    ) -> Result<Vec<usize>, String> {
        let factor = crate::people_scene::Policy::distance_factor(u32::from(distance))?;
        let fade = crate::category_distance::Fade::from_factor(factor);
        let view = openmt2001::camera::scene_view_with_up(camera.eye, camera.target, camera.up);
        let frustum = crate::selection::CategoryFrustum::new(camera)?;
        let mut visibility = std::collections::BTreeMap::new();
        let mut changed = Vec::new();
        for span in &mut self.spans {
            let scale = if let Some(&scale) = visibility.get(&span.category) {
                scale
            } else {
                let depth = openmt2001::camera::translated_scene_view(
                    view,
                    [span.center[0], 0., span.center[1]],
                )[11];
                let scale = if frustum.visible(
                    span.category,
                    span.center,
                    scene.category_model_heights.get(&span.category).copied(),
                )? {
                    fade.detail_scale(depth)
                } else {
                    None
                };
                visibility.insert(span.category, scale);
                scale
            };
            let presentation = (scale.is_some(), scale.unwrap_or(0.).to_bits());
            if span.presentation == Some(presentation) {
                continue;
            }
            let draw = scene
                .draws
                .get_mut(span.draw)
                .ok_or("detail span draw missing")?;
            let vertices = draw
                .vertices
                .get_mut(span.start..span.start + span.positions.len())
                .ok_or("detail span leaves draw")?;
            let hidden = span.positions[0];
            for (vertex, &position) in vertices.iter_mut().zip(&span.positions) {
                if let Some(scale) = scale {
                    vertex[..3].copy_from_slice(&[
                        position[0],
                        span.pivot_y + (position[1] - span.pivot_y) * scale,
                        position[2],
                    ]);
                } else {
                    vertex[..3].copy_from_slice(&hidden);
                }
            }
            span.presentation = Some(presentation);
            changed.push(span.draw);
        }
        changed.sort_unstable();
        changed.dedup();
        Ok(changed)
    }
}

struct LoadedInspection {
    camera_epoch: Option<openmt2001::level_startup::SourceEpoch>,
    camera_publisher: Option<crate::save_menu::camera_source::Publisher>,
    camera_inbox: Option<crate::live_host::camera_bridge::Inbox>,
    save_profile_root: Option<PathBuf>,
    logic_tick: Option<u32>,
    construction_active: bool,
    music_era: Option<u32>,
    environment_time: Option<(u32, u32)>,
    // Import provenance only. A later live owner must move across refresh;
    // never export this stale pair as current state after camera movement.
    imported_cameras: Option<openmt2001::camera::bank::Capture>,
    saved_viewpoint: Option<(CameraSample, openmt2001::camera::BlockCamera)>,
    saved_prior_mode: Option<u32>,
    saved_auction: Option<crate::session_save::AuctionContinuation>,
    restored_presentation: Vec<openmt2001::economy::AccountPosting>,
    auction_requests: Option<mpsc::Receiver<crate::auction_bridge::Request>>,
    auction_audio_requests: Option<mpsc::Receiver<crate::live_host::auction_audio::Request>>,
    transport_audio_requests: Option<mpsc::Receiver<crate::transport_audio::Request>>,
    era_audio_requests: Option<mpsc::Receiver<crate::era_audio::Request>>,
    gameplay_sound_requests: Option<mpsc::Receiver<crate::gameplay_sound::Request>>,
    reuse_city: bool,
    player_info: Option<crate::player_info::State>,
    navigator_owners: Vec<Option<crate::navigator::Owner>>,
    block_information: Option<crate::block_information::Report>,
    city_people: Option<crate::city_people::Report>,
    runtime_locator: Option<crate::locator::RuntimeInputs>,
    consumer_polls: Option<[crate::consumer_polls::Poll; 52]>,
    city_ownership: Option<crate::city_ownership::Ownership>,
    city_rights: Option<crate::city_rights::Rights>,
    city_prestige: Option<crate::city_prestige::Prestige>,
    comparison: Option<crate::comparison::Data>,
    trees: Option<crate::tree_scene::Layer>,
    people: Option<crate::people_scene::Layer>,
    vehicles: Option<crate::vehicle_scene::Layer>,
    transport: Option<crate::transport_scene::Layer>,
    // Dropping this channel releases the worker's thread-affine Lua VM/world.
    fresh_session: Option<mpsc::Sender<FreshRequest>>,
    scene: Scene,
    detail_visibility: DetailVisibility,
    /// Retained material draws whose initialized building pose changed.
    updated_city_draws: Vec<usize>,
    status: Option<(String, u32, u32)>,
}
#[derive(Default)]
struct SnapshotCache {
    converted_textures: openmt2001::texture::LoaderCache,
    /// Exact renderer inputs, compared directly; simulation remains authoritative.
    buildings: Vec<[u8; 0x394]>,
    hour: u32,
    city: Option<Scene>,
    detail_visibility: DetailVisibility,
    appearance_assets: Option<crate::save_appearance::Assets>,
    static_details: Option<crate::static_detail::Resources>,
    #[cfg(test)]
    static_detail_ranges: Vec<std::ops::Range<usize>>,
    trees: Option<std::sync::Arc<crate::tree_scene::Resources>>,
    tree_instances: Option<(u64, std::sync::Arc<crate::tree_scene::Instances>)>,
    people: Option<crate::people_scene::Resources>,
    vehicles: Option<crate::vehicle_scene::Resources>,
    transport: Option<crate::transport_scene::Resources>,
    scaffolds: Option<construction_scaffold::Resources>,
    scaffolds_active: bool,
}
enum WorkerStart {
    Connected(Box<connected_start::Prepared>),
    Skirmish(crate::menu::skirmish_route::Request),
    Fresh(crate::singleplayer::Setup),
    Saved(PathBuf, Option<PathBuf>),
}
struct ResultSnapshot {
    presentation: crate::fresh_city::result::ResultPresentation,
    tick: u32,
    hour: u32,
    counter: u32,
}
struct PendingPropertyInspection {
    category: u32,
    reply: mpsc::Receiver<Result<crate::fresh_city::property::Inspection, String>>,
}
struct PendingRightsInspection {
    category: u32,
    reply: mpsc::Receiver<Result<openmt2001::property_rights::Inspection, String>>,
}
/// Original 4c39ed chooses Block Information for building+1c == -1;
/// 4c39f3 chooses the building controller once a business is linked.
pub(crate) fn building_has_management_link(
    building: &openmt2001::render_scene::InspectionBuilding,
) -> bool {
    building.business().is_some() || building.initialized_apartment.is_some()
}
/// Main-panel mode0 is restored on deselection (41eb40/41f290).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InformationTarget {
    Welcome,
    Block(crate::selection::Hit),
    Business,
}
fn information_target(
    selected: Option<crate::selection::Hit>,
    buildings: &[openmt2001::render_scene::InspectionBuilding],
) -> InformationTarget {
    match selected {
        None => InformationTarget::Welcome,
        Some(hit)
            if hit
                .building
                .is_some_and(|i| building_has_management_link(&buildings[i])) =>
        {
            InformationTarget::Business
        }
        Some(hit) => InformationTarget::Block(hit),
    }
}

#[cfg(test)]
#[test]
fn no_city_selection_restores_welcome_information() {
    assert_eq!(information_target(None, &[]), InformationTarget::Welcome);
    let hit = crate::selection::Hit {
        category: 0,
        building: None,
    };
    assert_eq!(
        information_target(Some(hit), &[]),
        InformationTarget::Block(hit)
    );
    assert_eq!(information_target(None, &[]), InformationTarget::Welcome);
}

/// Picking returns a geometry-vector index; reports and transactions use the
/// native category ID. The original board omits inactive slots 40 and 45.
pub(crate) fn utility_report_category(
    selected: Option<crate::selection::Hit>,
    blocks: &[openmt2001::board::Block],
    report: Option<&crate::block_information::Report>,
) -> Option<usize> {
    let selected = selected?;
    let category = blocks.get(selected.category)?.id;
    let report = report?;
    (selected.building.is_none()
        && (report.categories.get(category)?.is_some()
            || report.utility_categories.contains(&category)
            || report.railroads.is_some() && report.railroad_categories.contains(&category)))
    .then_some(category)
}

/// Scene buildings and report hyperlinks retain native IDs; Hit addresses the
/// active geometry vector, which omits original category slots 40 and 45.
pub(crate) fn inspection_hit(
    blocks: &[openmt2001::board::Block],
    native_category: usize,
    building: Option<usize>,
) -> Option<crate::selection::Hit> {
    Some(crate::selection::Hit {
        category: blocks
            .iter()
            .position(|block| block.id == native_category)?,
        building,
    })
}
#[cfg(test)]
struct DispatchProbe {
    pause: [u8; 9],
    tasks: Vec<openmt2001::timers::Task>,
    frequency: (f32, Option<usize>, Option<usize>),
    frame: openmt2001::timers::Frame,
    epoch: Instant,
}
enum FreshRequest {
    #[cfg(test)]
    DispatchProbe(mpsc::Sender<DispatchProbe>),
    CameraTurn {
        turn: camera_producer::Turn,
        reply: mpsc::Sender<Result<camera_producer::Payload, String>>,
    },
    UiCameraReady {
        reply: mpsc::Sender<Result<(), String>>,
    },
    PropertyRights {
        category: u32,
        panel_mode: u8,
        activate: bool,
        reply: mpsc::Sender<Result<openmt2001::property_rights::Inspection, String>>,
    },
    PropertyOffer {
        request: crate::utility_report::PropertyRequest,
        reply: mpsc::Sender<Result<crate::fresh_city::property::Inspection, String>>,
    },
    SaveSelected {
        target: crate::save_menu::Target,
        camera: CameraSample,
        // Event-thread by-value cut; Some is supplied only by the live owner.
        capture: Option<openmt2001::camera::bank::Capture>,
        game_mode: u32,
        interface_mode: u32,
        reply: mpsc::Sender<Result<PathBuf, String>>,
    },
    DeleteSave {
        id: String,
        reply: mpsc::Sender<Result<(), String>>,
    },
    ResultState {
        reply: mpsc::Sender<Result<ResultSnapshot, String>>,
    },
    ResultCredits {
        reply: mpsc::Sender<Result<crate::fresh_city::result::ResultPresentation, String>>,
    },
    ResultEvent {
        name: &'static str,
        reply: mpsc::Sender<Result<crate::fresh_city::result::ResultPresentation, String>>,
    },
    ResultComplete {
        reply: mpsc::Sender<Result<crate::live_host::Status, String>>,
    },
    InspectOperations {
        building: usize,
        bounds: [f32; 4],
        reply: mpsc::Sender<Result<crate::business_operations::Inspection, String>>,
    },
    BusinessOperation {
        building: usize,
        action: openmt2001::business_controls::Action,
        reply: mpsc::Sender<Result<bool, String>>,
    },
    Card(crate::fresh_city::cards::Action),
    AuctionButton(u32),
    Stats(crate::live_host::StatsAction),
    ScenarioReminder,
    DismissMarketShare,
    HumanDialog(
        crate::human_dialog::Reply,
        mpsc::Sender<Vec<crate::prompt::Request>>,
    ),
    Residents {
        reply: mpsc::Sender<Result<crate::people_scene::Layer, String>>,
    },
    Vehicles {
        reply: mpsc::Sender<Result<crate::vehicle_scene::Layer, String>>,
    },
    Transport {
        reply: mpsc::Sender<Result<crate::transport_scene::Layer, String>>,
    },
    EditBusiness {
        target: crate::business_edit::Target,
        action: crate::business_edit::Action,
        reply: mpsc::Sender<Result<[u8; 544], String>>,
    },
    Run(bool),
    Focus(focus::Event),
    Options {
        open: bool,
        reply: mpsc::Sender<Result<(), String>>,
    },
    ToggleSpeed,
    ComparisonMode(usize),
    DismissPrompt(u32, Option<mpsc::Sender<Vec<crate::prompt::Request>>>),
    TutorialControl(u32),
    TutorialProducer {
        control: u32,
        event: u32,
    },
    TutorialPick(crate::tutorial_two_interaction::Pick),
    WindowTexturePublished(u64),
    Status {
        camera: [f32; 3],
        reply: mpsc::Sender<crate::live_host::Status>,
    },
    LogicTick(mpsc::Sender<crate::time_panel::ClockSample>),
    Planning {
        category: usize,
        reply: mpsc::Sender<Result<crate::build_panel::Context, String>>,
    },
    BuildDrag {
        category: usize,
        reply: mpsc::Sender<crate::build_panel::PlacementReply>,
    },
    Build {
        form: crate::build_panel::Request,
        submit: bool,
        reply: mpsc::Sender<crate::fresh_city::BuildReply>,
    },
    Inspect {
        camera: CameraSample,
        reply: mpsc::Sender<Result<LoadedInspection, crate::load::Failure>>,
    },
}
#[path = "intro/scene_inputs.rs"]
mod scene_inputs;
struct Inspection {
    music_admission: Option<focus::MusicAdmission>,
    camera_epoch: Option<openmt2001::level_startup::SourceEpoch>,
    camera_turns: std::collections::VecDeque<camera_producer::Pending>,
    camera_producer: Option<camera_producer::Producer>,
    camera_ready: Option<mpsc::Receiver<Result<(), String>>>,
    ui_graph: Option<camera_live::ui_graph::UiGraphState>,
    camera_navigation: Option<camera_live::navigation::State>,
    camera_focus: camera_live::focus::Focus,
    navigation_click_latched: bool,
    camera_endpoint: Option<camera_live::endpoint::Endpoint>,
    camera_capture_pending: Option<(
        crate::save_menu::Target,
        mpsc::Receiver<Result<openmt2001::camera::bank::Capture, String>>,
    )>,
    camera_ui: Option<camera_live::State>,
    save_profile_root: Option<PathBuf>,
    saved_camera: bool,
    result_pending: Option<mpsc::Receiver<Result<ResultSnapshot, String>>>,
    result: Option<ResultView>,
    auction_requests: Option<mpsc::Receiver<crate::auction_bridge::Request>>,
    auction_audio_requests: Option<mpsc::Receiver<crate::live_host::auction_audio::Request>>,
    auction_audio_player: crate::live_host::auction_audio::Player,
    transport_audio_requests: Option<mpsc::Receiver<crate::transport_audio::Request>>,
    transport_audio_player: crate::transport_audio::Player,
    era_audio_requests: Option<mpsc::Receiver<crate::era_audio::Request>>,
    era_audio_player: crate::era_audio::Player,
    gameplay_sound_requests: Option<mpsc::Receiver<crate::gameplay_sound::Request>>,
    gameplay_sound_player: crate::gameplay_sound::Player,
    auction: crate::auction_bridge::View,
    auction_panel: crate::auction_panel::View,
    notifications: crate::notice_panel::View,
    planning: Option<crate::build_view::State>,
    planning_pending: Option<mpsc::Receiver<Result<crate::build_panel::Context, String>>>,
    planning_pressed: bool,
    live_status: Option<crate::live_host::Status>,
    time_panel: crate::time_panel::View,
    cash_arrow: crate::cash_arrow::View,
    dust: crate::dust_scene::View,
    time_pressed: Option<crate::time_panel::Button>,
    game_options: crate::game_options::View,
    save_dialog: crate::save_dialog::View,
    save_pending: Option<mpsc::Receiver<Result<PathBuf, String>>>,
    options_ack: options_ack::Gate,
    save_delete_pending: Option<mpsc::Receiver<Result<(), String>>>,
    settings_menu: crate::settings_menu::View,
    pulse_draw: Option<usize>,
    pulse_state: Option<openmt2001::pulse_lights::PulseLights>,
    status_pending: Option<mpsc::Receiver<crate::live_host::Status>>,
    environment_pending: Option<mpsc::Receiver<crate::time_panel::ClockSample>>,
    environment_tick: u32,
    environment_time: Option<(u32, u32)>,
    last_status_query: Instant,
    last_snapshot: Instant,
    construction_active: bool,
    snapshot_tick: u32,
    card_input: crate::card_panel::Input,
    card_selection: Option<(i32, i32)>,
    prompts: crate::prompt::State,
    event_list: crate::event_list::View,
    stats_panel: crate::stats_panel::View,
    market_share_panel: crate::market_share_panel::View,
    stats_requested: bool,
    tutorial_camera_seen: usize,
    auction_notice: crate::auction_notice::State,
    prompt_reply: Option<mpsc::Receiver<Vec<crate::prompt::Request>>>,
    block_information: Option<crate::block_information::Report>,
    utility_report: crate::utility_report::View,
    property_pending: Option<PendingPropertyInspection>,
    rights_pending: Option<PendingRightsInspection>,
    comparison: Option<crate::comparison::View>,
    trees: Option<crate::tree_scene::Layer>,
    people: Option<crate::people_scene::Layer>,
    people_pending: Option<mpsc::Receiver<Result<crate::people_scene::Layer, String>>>,
    last_people_query: Instant,
    vehicles: Option<crate::vehicle_scene::Layer>,
    vehicles_pending: Option<mpsc::Receiver<Result<crate::vehicle_scene::Layer, String>>>,
    last_vehicles_query: Instant,
    transport: Option<crate::transport_scene::Layer>,
    transport_pending: Option<mpsc::Receiver<Result<crate::transport_scene::Layer, String>>>,
    last_transport_query: Instant,
    _fresh_session: Option<mpsc::Sender<FreshRequest>>,
    adjustment: Option<crate::business_adjustment::State>,
    operations: crate::business_view::View,
    locator: Option<crate::locator::Locator>,
    locator_blocked: bool,
    scene: Scene,
    detail_visibility: DetailVisibility,
    prepared: PreparedScene,
    markers: PreparedScene,
    navigator: PreparedScene,
    navigator_owners: Vec<Option<crate::navigator::Owner>>,
    navigator_bounds: [f32; 4],
    navigator_pressed: bool,
    tutorial_go_to_block_pressed: bool,
    tooltip: Option<PreparedScene>,
    tooltip_state: Option<(u32, [i32; 2])>,
    icon_tooltip_dwell: crate::tooltip::IconDwell,
    navigation: [PreparedScene; 3],
    navigation_bounds: [f32; 4],
    navigation_pressed: bool,
    double_click: crate::navigation::DoubleClick,
    strategic: bool,
    return_camera: Option<camera_producer::Controls>,
    reframe_pending: bool,
    information: Option<(PreparedScene, [f32; 4])>,
    details_visible: bool,
    details_pressed: bool,
    toolbar_input: crate::locator::toolbar_collapse::Input,
    // Physical Right events delivered to inspection, sampled before routing.
    // Separate from camera/drag mouse_buttons, which modal handlers may swallow.
    toolbar_right_held: bool,
    toolbar_bounds: [[f32; 4]; 3],
    toolbar_regions: [Option<[f32; 4]>; 3],
    detail_icons: [PreparedScene; 2],
    detail_bounds: [f32; 4],
    camera: CameraSample,
    controls: openmt2001::camera::BlockCamera,
    building_angle: f32,
    building_camera: bool,
    inertia: openmt2001::camera::BlockCameraInertia,
    keys: [bool; 4],
    taps: [bool; 4],
    cursor: Option<[f64; 2]>,
    mouse_buttons: [bool; 2],
    drag: [i32; 2],
    pointer: Option<[i32; 2]>,
    hovered: Option<crate::selection::Hit>,
    selected: Option<crate::selection::Hit>,
    title_state: Option<(Option<crate::selection::Hit>, Option<crate::selection::Hit>)>,
    started: Instant,
    tick: usize,
}

fn update_city_pulse(
    assets: &crate::city_pulse_scene::Assets,
    renderer: &Renderer,
    index: Option<usize>,
    state: Option<&openmt2001::pulse_lights::PulseLights>,
    camera: CameraSample,
    scene: &mut Scene,
    prepared: &mut PreparedScene,
) -> Result<(), String> {
    if let (Some(index), Some(state)) = (index, state) {
        let draw = &mut scene.draws[index];
        assets.fill(draw, state, camera)?;
        renderer.update_surface(prepared, index, &draw.borrowed(), scene.camera, None)?;
    }
    Ok(())
}
enum ResultPhase {
    Flyby,
    CutScene,
}
struct ResultView {
    state: crate::fresh_city::result::ResultPresentation,
    fireworks: Option<crate::fresh_city::result::fireworks::Fireworks>,
    fireworks_draw: Option<usize>,
    fireworks_ticks: usize,
    _music: Option<(rodio::OutputStream, crate::audio_bus::focus::Music)>,
    samples: Vec<CameraSample>,
    started: Instant,
    phase: ResultPhase,
    controller: crate::result_controller::Controller,
    serviced_ticks: usize,
    bypass_input: bool,
    pending: Option<mpsc::Receiver<Result<crate::fresh_city::result::ResultPresentation, String>>>,
    credit_pending:
        Option<mpsc::Receiver<Result<crate::fresh_city::result::ResultPresentation, String>>>,
    cutscene_pending:
        Option<mpsc::Receiver<Result<crate::fresh_city::result::ResultPresentation, String>>>,
    completed: Option<mpsc::Receiver<Result<crate::live_host::Status, String>>>,
    actor_light: openmt2001::lighting::SceneLight,
    actors: PreparedScene,
    actor_draws: usize,
    title: Option<(Option<u32>, Option<usize>, u8, PreparedScene)>,
}
#[path = "intro/inspection.rs"]
mod inspection;

/// Mode 15 keeps the flyby camera's current triples when 4952f0 selects the
/// menu. Each 4e02d0 interface tick runs 483750, then moves those triples a
/// tenth of the remaining distance toward the new orbit endpoint. 4e0590
/// interpolates one more tenth by the presentation phase.
struct MenuCamera {
    eye_start: [f32; 3],
    target_start: [f32; 3],
    eye_endpoint: [f32; 3],
    target_endpoint: [f32; 3],
    angle: f32,
    tick: usize,
}
impl MenuCamera {
    fn new(flyby: CameraSample) -> Self {
        Self {
            eye_start: flyby.eye,
            target_start: flyby.target,
            eye_endpoint: flyby.eye,
            target_endpoint: flyby.target,
            angle: std::f32::consts::FRAC_PI_2,
            tick: 0,
        }
    }
    fn step(&mut self) {
        let angle = f64::from(self.angle);
        self.eye_endpoint = [
            (-53. + angle.cos() * 30.) as f32,
            30.,
            (44. + angle.sin() * 30.) as f32,
        ];
        self.target_endpoint = [-53., 5., 44.];
        self.angle = (angle + f64::from(f32::from_bits(0x3a03126f))) as f32;
        let advance = |start: &mut [f32; 3], endpoint: [f32; 3]| {
            for (current, desired) in start.iter_mut().zip(endpoint) {
                *current =
                    (f64::from(*current) + (f64::from(desired) - f64::from(*current)) / 10.) as f32;
            }
        };
        advance(&mut self.eye_start, self.eye_endpoint);
        advance(&mut self.target_start, self.target_endpoint);
        self.tick += 1;
    }
    fn sample(&mut self, tick: usize, phase: f32) -> CameraSample {
        while self.tick < tick {
            self.step();
        }
        let interpolate = |start: [f32; 3], endpoint: [f32; 3]| {
            std::array::from_fn(|axis| {
                (f64::from(start[axis])
                    + (f64::from(endpoint[axis]) - f64::from(start[axis])) * f64::from(phase) / 10.)
                    as f32
            })
        };
        CameraSample {
            eye: interpolate(self.eye_start, self.eye_endpoint),
            target: interpolate(self.target_start, self.target_endpoint),
            up: [0., 1., 0.],
            vertical_fov: 45.,
        }
    }
}

pub struct Intro {
    camera_worker: Option<camera_producer::WorkerExit>,
    camera_worker_failure: Option<String>,
    focus: focus::Route,
    pub(crate) process_frontend: Option<process_frontend::Owner>,
    camera_physical: camera_live::PhysicalInput,
    pause_keys: pause_keys::Input,
    camera_keys: camera_keys::Input,
    camera_parked: Option<camera_live::owner::Owner>,
    assets: Assets,
    renderer: Renderer,
    prepared: PreparedScene,
    mr_monopoly: PreparedScene,
    mr_monopoly_draws: usize,
    sidewalk_labels: PreparedScene,
    sidewalk_shading: crate::block_sidewalk_labels::FadeState,
    sidewalk_camera: CameraSample,
    strategic_scene: crate::strategic_scene::View,
    _color: wgpu::Texture,
    _depth: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    binding: wgpu::BindGroup,
    started: Instant,
    last_sample: Option<usize>,
    ended: bool,
    pub menu: Option<crate::menu::Menu>,
    menu_camera: Option<MenuCamera>,
    loading: Option<mpsc::Receiver<Result<LoadedInspection, crate::load::Failure>>>,
    loading_from_setup: bool,
    pending_connected: Option<connected_start::Pending>,
    loading_connected: bool,
    loading_skirmish: bool,
    refreshing_fresh: bool,
    refresh_retry: refresh::Retry,
    inspection: Option<Inspection>,
    audio_levels: crate::settings_menu::Levels,
    graphics_levels: crate::settings_menu::GraphicsLevels,
    gameplay_levels: crate::settings_menu::GameplayLevels,
    settings_profile: Option<std::path::PathBuf>,
    saved_settings: crate::profile_settings::Snapshot,
    settings_save_error: Option<String>,
    dialog_keys: dialog_keys::Input,
}
impl Drop for Intro {
    fn drop(&mut self) {
        // Window close/error teardown must cross the same source retirement
        // boundary before fields drop assets or the Session lease.
        self.retire_inspection();
    }
}
impl Intro {
    pub(crate) fn window_focus(&mut self, focused: bool) -> Result<(), String> {
        self.dialog_keys.focus(focused);
        self.service_camera_turns();
        if let Some(process) = &self.process_frontend {
            process.borrow_mut().observe(focused)?;
        }
        self.focus.observe(focused)?;
        if let Some(process) = &self.process_frontend {
            process.borrow_mut().note_sent(self.focus.sent());
        }
        Ok(())
    }
    pub(crate) fn dialog_physical_sample(&mut self, sample: process_frontend::PhysicalSample) {
        self.dialog_keys.physical(sample);
    }
    pub(crate) fn dialog_ui_event1(&mut self) {
        let active = !self.focus_pending() && self.loading.is_none();
        let prompts = self
            .inspection
            .as_mut()
            .filter(|view| {
                active
                    && view._fresh_session.is_some()
                    && !view.save_visible()
                    && !view.stats_requested
                    && view.result.is_none()
                    && view.result_pending.is_none()
                    && view
                        .live_status
                        .as_ref()
                        .is_none_or(|s| s.stats.is_none() && s.result_request.is_none())
            })
            .map(|view| {
                if view.game_options.quit_prompt.request().is_some() {
                    &mut view.game_options.quit_prompt
                } else {
                    &mut view.prompts
                }
            });
        self.dialog_keys.event1(prompts);
    }
    /// Drain before modal/loading early returns and every dependent worker poll.
    /// No call in this service can request work from the suspended Session.
    fn service_camera_turns(&mut self) {
        if let Some(view) = &mut self.inspection {
            view.service_camera_turns(
                &self.assets.selection,
                self.menu.as_ref().map(|menu| menu.adjustment_assets()),
            );
        }
    }
    /// Full default activation waits for the real audio prefix provider.
    /// Install only before starting a World; installation is not cold attach.
    pub(crate) fn install_focus_effects(
        &mut self,
        effects: Box<dyn focus::Effects>,
    ) -> Result<(), String> {
        if self.inspection.is_some() || self.loading.is_some() {
            return Err("Focus effects must be installed before World loading".into());
        }
        self.focus.install(effects)
    }
    pub(crate) fn service_focus(&mut self) -> Result<(), String> {
        self.service_camera_turns();
        let result: Result<(), String> = (|| {
            self.focus.poll()?;
            if let Some(process) = &self.process_frontend {
                process
                    .borrow_mut()
                    .complete_physical(self.focus.completed());
            }
            // A single prefix: release its typed suffix before the next prefix.
            self.focus.service_one()?;
            self.focus.poll()?;
            if let Some(process) = &self.process_frontend {
                process
                    .borrow_mut()
                    .complete_physical(self.focus.completed());
            }
            Ok(())
        })();
        if let Err(error) = &result {
            if let Some(process) = &self.process_frontend {
                process.borrow_mut().fail(error.clone());
            }
        }
        result
    }
    pub fn request_process_exit(
        &self,
        writer: crate::live_host::process_runtime::QuitWriter,
    ) -> Result<(), String> {
        self.assets
            .process_runtime
            .request_quit(writer)
            .map_err(|e| e.to_string())
    }
    pub fn process_exit_ready(&self) -> Result<bool, String> {
        self.assets
            .process_runtime
            .exit_ready()
            .map_err(|e| e.to_string())
    }

    pub(crate) fn birth_process_frontend(
        &mut self,
        window: winit::window::WindowId,
    ) -> Result<(), String> {
        self.sync_profile_settings()?;
        let receipt = self.assets.activity.take_frontend_startup()?;
        let process = process_frontend::ProcessFrontend::new(
            receipt,
            self.assets.process_runtime.clone(),
            window,
            self.audio_levels,
        )?;
        self.install_focus_effects(Box::new(process_frontend::Provider(process.clone())))?;
        self.process_frontend = Some(process);
        Ok(())
    }
    pub(crate) fn physical_sample(
        &mut self,
        window: winit::window::WindowId,
        sample: process_frontend::PhysicalSample,
    ) -> Result<(), String> {
        if let Some(process) = &self.process_frontend {
            process.borrow_mut().sample(
                window,
                sample,
                self.focus.sent(),
                self.focus.completed(),
            )?;
        }
        Ok(())
    }
    fn process_start_ready(&self) -> bool {
        camera_producer::startup_ready(&self.camera_worker, &self.camera_worker_failure)
            && !self.focus_pending()
            && self
                .process_frontend
                .as_ref()
                .is_none_or(|p| p.borrow().phase == process_frontend::Phase::Menu)
    }
    fn start_custody(
        &mut self,
        fresh: bool,
    ) -> Result<Option<crate::process_dispatch::DispatchCustody>, String> {
        self.process_frontend
            .as_ref()
            .map(|p| p.borrow_mut().take_custody(fresh))
            .transpose()
    }
    pub(crate) fn focus_pending(&self) -> bool {
        self.focus.sent() != self.focus.completed()
    }
    pub fn ui_effects_level(&self) -> u8 {
        self.audio_levels.effects
    }

    fn settings_snapshot(&self) -> crate::profile_settings::Snapshot {
        crate::profile_settings::Snapshot {
            audio: self.audio_levels,
            graphics: self
                .inspection
                .as_ref()
                .map_or(self.graphics_levels, |view| view.settings_menu.graphics),
            gameplay: self
                .inspection
                .as_ref()
                .map_or(self.gameplay_levels, |view| view.settings_menu.gameplay),
        }
    }

    fn persist_settings(&mut self) {
        let snapshot = self.settings_snapshot();
        menu_preferences::Cells {
            profile: &mut self.settings_profile,
            saved: &mut self.saved_settings,
            error: &mut self.settings_save_error,
            audio: &mut self.audio_levels,
            graphics: &mut self.graphics_levels,
            gameplay: &mut self.gameplay_levels,
        }
        .persist(snapshot);
    }
    fn sync_profile_settings(&mut self) -> Result<(), String> {
        menu_preferences::Cells {
            profile: &mut self.settings_profile,
            saved: &mut self.saved_settings,
            error: &mut self.settings_save_error,
            audio: &mut self.audio_levels,
            graphics: &mut self.graphics_levels,
            gameplay: &mut self.gameplay_levels,
        }
        .sync_profile(self.menu.as_mut(), self.process_frontend.as_ref())
    }
    /// App invokes this once after EVERY synchronous window-event dispatch,
    /// including inner early returns, before any following OS focus prefix.
    pub(crate) fn commit_menu_settings(&mut self) -> Result<(), String> {
        if self.inspection.is_none()
            && let Some(menu) = &mut self.menu
        {
            menu_preferences::Cells {
                profile: &mut self.settings_profile,
                saved: &mut self.saved_settings,
                error: &mut self.settings_save_error,
                audio: &mut self.audio_levels,
                graphics: &mut self.graphics_levels,
                gameplay: &mut self.gameplay_levels,
            }
            .commit_menu(menu, self.process_frontend.as_ref())?;
        } else {
            self.sync_profile_settings()?;
            self.persist_settings();
        }
        Ok(())
    }

    fn retire_inspection(&mut self) {
        self.focus.detach();
        if let Some(process) = &self.process_frontend {
            process.borrow_mut().draining();
        }
        if let Some(producer) = self
            .inspection
            .as_mut()
            .and_then(|view| view.camera_producer.take())
        {
            let (owner, result) = producer.retire();
            assert!(
                self.camera_parked.is_none(),
                "camera Owner has one process custodian"
            );
            self.camera_parked = Some(owner);
            if let Err(error) = result {
                eprintln!("Camera source retirement failed: {error}");
            }
        }
        if let Some(view) = &mut self.inspection {
            camera_producer::cancel_pending(&mut view.camera_turns);
        }
        if let Err(error) = refresh::retire(
            &mut self.refresh_retry,
            &mut self.refreshing_fresh,
            &mut self.loading,
            self.inspection
                .as_mut()
                .map(|view| &mut view.camera_endpoint),
            &mut self.camera_parked,
        ) {
            eprintln!("Property source retirement failed (latched in parked Owner): {error}");
        }
        if let Some(view) = self.inspection.take() {
            self.graphics_levels = view.settings_menu.graphics;
            self.gameplay_levels = view.settings_menu.gameplay;
            if let Some(menu) = &mut self.menu {
                menu.settings_audio = self.audio_levels;
                menu.settings.graphics = self.graphics_levels;
                menu.settings.gameplay = self.gameplay_levels;
            }
        }
    }
    fn apply_audio_levels(
        process: &Option<process_frontend::Owner>,
        levels: crate::settings_menu::Levels,
    ) {
        if let Some(process) = process {
            process.borrow_mut().commit_levels(levels);
        }
    }
    fn draw_tutorial_visual(
        assets: &Assets,
        menu: &Option<crate::menu::Menu>,
        renderer: &Renderer,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        view: &Inspection,
    ) -> Result<(), String> {
        let Some(target) = view
            .live_status
            .as_ref()
            .and_then(|status| status.tutorial_arrows.last())
        else {
            return Ok(());
        };
        let origin = menu
            .as_ref()
            .ok_or("tutorial prompt assets missing")?
            .prompt_assets()
            .arrow_origin();
        // Frame 50 is the standing tutor pose in the original 90-frame mesh.
        // The native transition timing remains to be recovered.
        let offset = view.live_status.as_ref().map_or([0.; 2], |status| {
            status.tutorial_interaction.prompt_offset()
        });
        let overlay = assets.tutorial_visual.prepare_positioned(
            renderer,
            origin,
            [target[0] as f32, target[1] as f32],
            50,
            offset,
        )?;
        renderer.draw_overlay(color_view, depth_view, &overlay);
        Ok(())
    }
    pub fn new(presenter: &Presenter, assets: Assets) -> Result<Self, String> {
        let sidewalk_camera = assets.samples[0];
        let renderer = Renderer::from_device(
            presenter.device.clone(),
            presenter.queue.clone(),
            "native surface device".into(),
        );
        let draws: Vec<_> = assets.scene.draws.iter().map(|d| d.borrowed()).collect();
        let prepared = renderer.prepare(&draws, assets.scene.camera, assets.scene.fog)?;
        // Original normal city render 4b3870 calls 4b3130; the executable's
        // initial 5455e8/5455ec pair is 10/10, selecting the Park Mr.
        // Monopoly mesh at the menu orbit's focus. Keep its texture pages
        // resident and submit it after the city with the shared depth.
        let actor = crate::auction_scene::result_layout((10, 10))?;
        let actor_draws = assets.result_actors.draws(&actor)?;
        let mr_monopoly = renderer.prepare(
            &actor_draws
                .iter()
                .map(|draw| draw.borrowed())
                .collect::<Vec<_>>(),
            assets.scene.camera,
            assets.scene.fog,
        )?;
        let (sidewalk_labels, _) = assets.sidewalk_labels.prepare(&renderer, &assets.scene)?;
        let make_texture = |format, usage, view_formats: &[wgpu::TextureFormat]| {
            renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Intro render target"),
                size: wgpu::Extent3d {
                    width: 1280,
                    height: 960,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats,
            })
        };
        let color = make_texture(
            COLOR_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            &[wgpu::TextureFormat::Rgba8UnormSrgb],
        );
        let depth = make_texture(DEPTH_FORMAT, wgpu::TextureUsages::RENDER_ATTACHMENT, &[]);
        let color_view = color.create_view(&Default::default());
        let display_view = color.create_view(&wgpu::TextureViewDescriptor {
            format: Some(presenter.scene_view_format()),
            ..Default::default()
        });
        let depth_view = depth.create_view(&Default::default());
        let binding = presenter.image_binding(&display_view);
        Ok(Self {
            camera_worker: None,
            camera_worker_failure: None,
            focus: focus::Route::default(),
            process_frontend: None,
            assets,
            renderer,
            prepared,
            mr_monopoly,
            mr_monopoly_draws: actor_draws.len(),
            sidewalk_labels,
            sidewalk_shading: crate::block_sidewalk_labels::FadeState::default(),
            sidewalk_camera,
            strategic_scene: crate::strategic_scene::View::default(),
            _color: color,
            _depth: depth,
            color_view,
            depth_view,
            binding,
            started: Instant::now(),
            last_sample: None,
            ended: false,
            menu: None,
            menu_camera: None,
            loading: None,
            loading_from_setup: false,
            pending_connected: None,
            loading_connected: false,
            loading_skirmish: false,
            refreshing_fresh: false,
            refresh_retry: refresh::Retry::default(),
            inspection: None,
            camera_physical: camera_live::PhysicalInput::default(),
            pause_keys: pause_keys::Input::default(),
            camera_keys: camera_keys::Input::default(),
            camera_parked: Some(camera_producer::new_process_owner()?),
            audio_levels: crate::settings_menu::Levels::default(),
            graphics_levels: crate::settings_menu::GraphicsLevels::default(),
            gameplay_levels: crate::settings_menu::GameplayLevels::default(),
            settings_profile: None,
            saved_settings: crate::profile_settings::Snapshot::default(),
            settings_save_error: None,
            dialog_keys: dialog_keys::Input::default(),
        })
    }
    pub fn open_menu(&mut self) {
        if let Some(assets) = self.assets.menu.take() {
            let last = self.last_sample.unwrap_or(0);
            self.menu_camera = Some(MenuCamera::new(self.assets.samples[last]));
            self.menu = Some(crate::menu::Menu::new(assets));
            self.started = Instant::now();
            self.last_sample = None;
        }
    }
    pub fn inspection_cursor_presented(&mut self) {
        if let Some(state) = self
            .inspection
            .as_mut()
            .and_then(|view| view.camera_ui.as_mut())
        {
            state.cursor_presented();
        }
    }
    pub fn inspection_cursor_kind(&self) -> Option<usize> {
        if self.inspection.is_none() {
            // Keep the existing city indices; appended index3 is original cursor1.
            let hover = self.loading.is_none()
                && self
                    .menu
                    .as_ref()
                    .is_some_and(|menu| menu.enabled_cursor_hover());
            return Some(if hover { 3 } else { 0 });
        }
        self.inspection.as_ref().map(|view| {
            if let Some(state) = &view.camera_ui {
                return state.cursor_index();
            }
            if view.secondary_enabled()
                && let Some(page) = view.settings_menu.page
            {
                let blocked = view.toolbar_right_held
                    || view.save_visible()
                    || view.stats_requested
                    || view
                        .live_status
                        .as_ref()
                        .is_some_and(|status| status.stats.is_some());
                let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                return if !blocked && self.assets.settings_menu.action_cursor_hover(page, pointer) {
                    3
                } else {
                    0
                };
            }
            if self.menu.as_ref().is_some_and(|menu| {
                view.toolbar_target(menu, &self.assets.selection.blocks)
                    .is_some()
            }) {
                return 3;
            }
            // Original mode12/13 use cursor2 for right-only and cursor3 for both buttons.
            if view.strategic || !view.mouse_buttons[1] {
                0
            } else if view.mouse_buttons[0] {
                2
            } else {
                1
            }
        })
    }
    /// Submit or inspect a form on the thread that owns the retained Lua/world state.
    /// Submission queues the original command; it does not advance a paused simulation.
    pub fn request_build_form(
        &self,
        form: crate::build_panel::Request,
        submit: bool,
    ) -> Result<mpsc::Receiver<crate::fresh_city::BuildReply>, String> {
        let sender = self
            .inspection
            .as_ref()
            .and_then(|view| view._fresh_session.as_ref())
            .ok_or("construction requires a retained fresh city")?;
        let (reply, result) = mpsc::channel();
        sender
            .send(FreshRequest::Build {
                form,
                submit,
                reply,
            })
            .map_err(|e| e.to_string())?;
        Ok(result)
    }
    /// Refresh authoritative scene/report records after the worker has executed a command.
    pub fn refresh_fresh_city(&mut self) -> Result<(), String> {
        camera_producer::request_refresh(
            self.inspection.as_mut(),
            &self.assets.selection,
            self.menu.as_ref().map(|menu| menu.adjustment_assets()),
            &mut self.loading,
            &mut self.refreshing_fresh,
            &mut self.refresh_retry,
        )
    }
    pub fn inspecting(&self) -> bool {
        self.inspection.is_some()
    }
    /// App feeds every event before logical/menu/Inspection routing. Keep this
    /// process input across city exit, true Load and presentation refresh.
    pub fn camera_physical_event(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::{ElementState, WindowEvent};
        match event {
            WindowEvent::KeyboardInput { event, .. } => self
                .camera_physical
                .key(event.physical_key, event.state == ElementState::Pressed),
            WindowEvent::MouseInput { button, state, .. } => self
                .camera_physical
                .button(*button, *state == ElementState::Pressed),
            WindowEvent::CursorMoved { position, .. } => {
                self.camera_physical.pointer([position.x, position.y])
            }
            WindowEvent::Focused(false) => self.camera_physical.unfocus(),
            _ => (),
        }
    }
    pub(crate) fn camera_physical_sample(
        &mut self,
        sample: process_frontend::PhysicalSample,
    ) -> Result<(), String> {
        let mut gates = pause_keys::Gates::default();
        if matches!(
            sample,
            process_frontend::PhysicalSample::Key {
                key: winit::keyboard::PhysicalKey::Code(
                    winit::keyboard::KeyCode::F1 | winit::keyboard::KeyCode::KeyP
                ),
                down: true,
                repeat: false,
            }
        ) && let Some(view) = &self.inspection
            && view._fresh_session.is_some()
        {
            gates.active = true;
            gates.modal = view.options_visible()
                || view.save_visible()
                || view.stats_requested
                || view.result.is_some()
                || view.result_pending.is_some()
                || view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.stats.is_some() || s.result_request.is_some())
                || view
                    .camera_navigation
                    .as_ref()
                    .is_some_and(|n| n.mode() == 4);
            if !matches!(
                sample,
                process_frontend::PhysicalSample::Key {
                    key: winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F1),
                    ..
                }
            ) && let Some(graph) = &view.ui_graph
            {
                gates.text_entry = graph.chat_guard57e794_plus345()? != 0;
            }
            use crate::live_host::process_runtime::Byte;
            gates.suppressed = self
                .assets
                .process_runtime
                .byte(Byte::Media8fca98)
                .map_err(|e| e.to_string())?
                != 0
                || self
                    .assets
                    .process_runtime
                    .byte(Byte::Suppressed8b0700)
                    .map_err(|e| e.to_string())?
                    != 0;
        }
        self.dispatch_camera_shortcut(sample);
        self.pause_keys.dispatch(sample, gates, &mut self.focus)?;
        process_frontend::project_camera_sample(
            &mut self.camera_physical,
            self.inspection.as_mut(),
            sample,
        );
        Ok(())
    }
    fn dispatch_camera_shortcut(&mut self, sample: process_frontend::PhysicalSample) {
        let mut gates = camera_keys::Gates::default();
        if matches!(
            sample,
            process_frontend::PhysicalSample::Key {
                key: winit::keyboard::PhysicalKey::Code(
                    winit::keyboard::KeyCode::Tab | winit::keyboard::KeyCode::Home
                ),
                down: true,
                repeat: false,
            }
        ) && let Some(view) = &self.inspection
        {
            gates.active = !self.focus_pending() && self.loading.is_none();
            gates.modal = view.options_visible()
                || view.save_visible()
                || view.settings_visible()
                || view.prompts.blocks_controls()
                || view.prompt_reply.is_some()
                || view.auction_visible()
                || view.stats_requested
                || view.result.is_some()
                || view.result_pending.is_some()
                || view.adjustment.is_some()
                || view.planning.is_some()
                || view.planning_pending.is_some()
                || view.live_status.as_ref().is_some_and(|s| {
                    s.stats.is_some() || s.result_request.is_some() || s.market_share.is_some()
                });
            gates.tutorial = view.live_status.as_ref().is_some_and(|s| {
                s.tutorial_controls.all_disabled || s.tutorial_controls.camera_disabled
            });
            gates.city_view = view
                .camera_navigation
                .as_ref()
                .is_none_or(|n| n.mode() != 1)
                && (view.ui_graph.is_none() || view.graph_input(camera_live::ui_graph::PanelId::E));
            if let Some(graph) = &view.ui_graph {
                // The installed UI owns text focus on this route, including
                // chat. The native continuation is not the live input route.
                gates.text_entry = graph.body(camera_live::ui_graph::PanelId::OtherPrimary(3))
                    || [
                        camera_live::navigation::Group::Primary,
                        camera_live::navigation::Group::Secondary,
                        camera_live::navigation::Group::Tertiary,
                    ]
                    .into_iter()
                    .any(|group| graph.group(group).key_target.is_some());
            }
        }
        if let Some(action) = self.camera_keys.sample(sample, gates)
            && let Some(view) = &mut self.inspection
        {
            match action {
                // The compass transaction owns both raw records and the return pose.
                camera_keys::Action::CityView => view.toggle_navigation(&self.assets.navigation),
                camera_keys::Action::ParentBlock => view.focus_parent_block(&self.assets.selection),
            }
        }
    }
    fn refresh_inspection_projection(&mut self) -> Result<(), String> {
        if let Some(view) = &mut self.inspection {
            if !view.reframe_pending {
                return Ok(());
            }
            let mut changed = view.scene.reframe(view.camera)?;
            if let Some(trees) = &mut view.trees {
                changed.extend(trees.reframe(&mut view.scene, view.camera)?);
            }
            changed.extend(view.detail_visibility.reframe(
                &mut view.scene,
                view.camera,
                view.settings_menu.graphics.distance,
            )?);
            if let Some(locator) = &mut view.locator {
                locator.dirty = true;
            }
            let draw_distance = u32::from(view.settings_menu.graphics.distance);
            let resident_draws = if let Some(people) = &view.people {
                people.reframe(&mut view.scene, view.camera, draw_distance)?
            } else {
                0..0
            };
            changed.extend(resident_draws.clone());
            if let Some(vehicles) = &view.vehicles {
                changed.extend(vehicles.reframe(&mut view.scene, view.camera, draw_distance)?);
            }
            if let Some(transport) = &view.transport {
                changed.extend(transport.reframe(&mut view.scene, view.camera, draw_distance)?);
            }
            let star_detail = view.normal_star_detail();
            let changed =
                normal_city_scene_updates(&mut view.scene, view.camera, star_detail, changed)?;
            for index in changed {
                self.renderer.update_dynamic_vertices(
                    &mut view.prepared,
                    index,
                    &view.scene.draws[index].borrowed(),
                )?;
            }
            let cameras = view
                .scene
                .draws
                .iter()
                .map(|draw| draw.view_projection.unwrap_or(view.scene.camera))
                .collect::<Vec<_>>();
            self.renderer
                .update_cameras(&view.prepared, &cameras, view.scene.fog)?;
            view.reframe_pending = false;
        }
        Ok(())
    }
    fn inspection_update(&mut self) -> Result<(), String> {
        if self
            .inspection
            .as_ref()
            .is_some_and(|view| view.camera_ui.is_some())
        {
            return self.refresh_inspection_projection();
        }
        if let Some(view) = &mut self.inspection {
            if !view.camera_turns.is_empty() {
                return Ok(());
            }
            let callback_time = view.started.elapsed().as_secs_f64() * 50.;
            let tick = callback_time.floor() as usize;
            let previous = view.camera;
            while view.tick <= tick {
                if view.live_status.as_ref().is_some_and(|status| {
                    status.tutorial_controls.camera_disabled
                        || status.tutorial_controls.all_disabled
                }) {
                    view.tick = tick + 1;
                    view.keys = [false; 4];
                    view.taps = [false; 4];
                    view.drag = [0; 2];
                    view.mouse_buttons = [false; 2];
                    break;
                }
                if !view.camera_producer.as_ref().map_or(!view.strategic, |p| {
                    p.legacy_callback_enabled(view.strategic)
                }) {
                    view.tick = tick + 1;
                    view.drag = [0; 2];
                    break;
                }
                // Preserve short host key taps until one original callback consumes them.
                let keys = std::array::from_fn(|i| view.keys[i] || view.taps[i]);
                // Sample the SAME process word once for this combined callback.
                let low_height = self
                    .assets
                    .process_runtime
                    .performance_nonzero()
                    .map_err(|e| e.to_string())?;
                let buttons = if self.process_frontend.is_some() {
                    self.camera_physical.snapshot()?.1
                } else {
                    view.mouse_buttons
                };
                let pose = if buttons == [true, true] {
                    crate::navigation::pan_tick(
                        &mut view.controls,
                        view.building_camera.then_some(&mut view.building_angle),
                        keys,
                        view.drag,
                        i32::from(view.settings_menu.gameplay.mouse_sensitivity),
                    )?
                } else if view.building_camera {
                    crate::navigation::building_tick_with_height(
                        &mut view.controls,
                        &mut view.building_angle,
                        keys,
                        view.drag,
                        i32::from(view.settings_menu.gameplay.mouse_sensitivity),
                        view.settings_menu.gameplay.invert_vertical,
                        low_height,
                    )?
                } else {
                    mode12_input_tick_with_height(
                        &mut view.controls,
                        view.saved_camera,
                        keys,
                        view.drag,
                        i32::from(view.settings_menu.gameplay.mouse_sensitivity),
                        view.settings_menu.gameplay.invert_vertical,
                        low_height,
                    )?
                };
                view.drag = [0; 2];
                let (eye, target) = pose;
                view.taps = [false; 4];
                view.inertia.tick(eye, target);
                if self.process_frontend.is_some() {
                    self.camera_physical.consumed();
                }
                view.tick += 1;
            }
            // Original4b9c94 samples4e0590 using the camera scheduler's remainder
            // fraction (4eb7f0). Host monotonic time supplies the50Hz interval.
            if view.camera_producer.as_ref().map_or(!view.strategic, |p| {
                p.legacy_callback_enabled(view.strategic)
            }) {
                let (eye, target) = view.inertia.sample(callback_time.fract() as f32)?;
                view.camera.eye = eye;
                view.camera.target = target;
            }
            view.commit_camera();
            if !view.reframe_pending
                && previous.eye == view.camera.eye
                && previous.target == view.camera.target
            {
                return Ok(());
            }
            view.reframe_pending = false;
            let mut changed = view.scene.reframe(view.camera)?;
            if let Some(trees) = &mut view.trees {
                changed.extend(trees.reframe(&mut view.scene, view.camera)?);
            }
            changed.extend(view.detail_visibility.reframe(
                &mut view.scene,
                view.camera,
                view.settings_menu.graphics.distance,
            )?);
            if let Some(locator) = &mut view.locator {
                locator.dirty = true;
            }
            let draw_distance = u32::from(view.settings_menu.graphics.distance);
            let resident_draws = if let Some(people) = &view.people {
                people.reframe(&mut view.scene, view.camera, draw_distance)?
            } else {
                0..0
            };
            changed.extend(resident_draws.clone());
            if let Some(vehicles) = &view.vehicles {
                changed.extend(vehicles.reframe(&mut view.scene, view.camera, draw_distance)?);
            }
            if let Some(transport) = &view.transport {
                changed.extend(transport.reframe(&mut view.scene, view.camera, draw_distance)?);
            }
            let star_detail = view.normal_star_detail();
            let changed =
                normal_city_scene_updates(&mut view.scene, view.camera, star_detail, changed)?;
            for index in changed {
                self.renderer.update_dynamic_vertices(
                    &mut view.prepared,
                    index,
                    &view.scene.draws[index].borrowed(),
                )?;
            }
            let cameras = view
                .scene
                .draws
                .iter()
                .map(|draw| draw.view_projection.unwrap_or(view.scene.camera))
                .collect::<Vec<_>>();
            self.renderer
                .update_cameras(&view.prepared, &cameras, view.scene.fog)?;
        }
        Ok(())
    }
    fn draw_frontend(
        &mut self,
        presenter: &Presenter,
        review: Option<crate::live_host::ResultReview>,
    ) -> Result<(), String> {
        if self.menu.is_some() && self.last_sample.is_none() {
            presenter
                .window
                .set_title("OpenMT2001 - main menu / screens in development");
            eprintln!("Main menu: original mode15 orbit and seven original buttons");
        }
        // Original startup registers camera callback4ba580 at50Hz; presentation
        // cadence is independent. Frame0 is a basis record and is never shown.
        let prepare_started = presenter.profile_started();
        let time = self.started.elapsed().as_secs_f64() * 50.;
        let tick = time as usize;
        let sample = tick.min(self.assets.samples.len() - 1);
        if self.last_sample != Some(sample) || self.menu.is_some() {
            let camera = if self.menu.is_some() {
                // 4952f0 -> 4836e0/483750; 4e02d0 updates the start pose
                // at 50 Hz and 4e0590 presents its fractional phase. Wall
                // fraction stands in for the native scheduler's render phase.
                let phase = (time - tick as f64) as f32;
                self.menu_camera
                    .as_mut()
                    .ok_or("menu camera missing")?
                    .sample(tick, phase)
            } else {
                self.assets.samples[sample]
            };
            self.sidewalk_camera = camera;
            let mut changed = self.assets.scene.reframe(camera)?;
            if let Some(menu) = &self.menu {
                changed.extend(
                    self.assets
                        .scene
                        .apply_normal_star_detail(menu.settings.graphics.detail, camera)?,
                );
                changed.sort_unstable();
                changed.dedup();
            }
            let updates = changed
                .into_iter()
                .map(|index| (index, self.assets.scene.draws[index].borrowed()))
                .collect::<Vec<_>>();
            self.renderer
                .update_dynamic_vertices_batch(&mut self.prepared, &updates)?;
            let cameras = self
                .assets
                .scene
                .draws
                .iter()
                .map(|draw| draw.view_projection.unwrap_or(self.assets.scene.camera))
                .collect::<Vec<_>>();
            self.renderer
                .update_cameras(&self.prepared, &cameras, self.assets.scene.fog)?;
            self.last_sample = Some(sample);
        }
        // The shared actor is also drawn by live city inspections. Restore
        // its intro matrix even when the flyby sample has not advanced.
        for index in 0..self.mr_monopoly_draws {
            self.renderer.update_camera(
                &self.mr_monopoly,
                index,
                self.assets.scene.camera,
                self.assets.scene.fog,
            )?;
        }
        self.assets.sidewalk_labels.update(
            &self.renderer,
            &mut self.sidewalk_labels,
            &mut self.sidewalk_shading,
            &self.assets.scene,
            self.sidewalk_camera,
            35,
        )?;
        if tick >= self.assets.samples.len() && !self.ended && self.menu.is_none() {
            self.ended = true;
            self.open_menu();
            eprintln!("FlybyEnded: main menu");
            presenter.window.set_title("OpenMT2001 - main menu");
        }
        let submit_started = presenter.profile_menu_prepare(prepare_started);
        for layer in frontend_draw::layers(review) {
            match layer {
                frontend_draw::Layer::Background => {
                    self.renderer
                        .draw_prepared(&self.color_view, &self.depth_view, &self.prepared);
                    self.renderer.draw_world_overlay(
                        &self.color_view,
                        &self.depth_view,
                        &self.sidewalk_labels,
                    );
                    self.renderer.draw_world_overlay(
                        &self.color_view,
                        &self.depth_view,
                        &self.mr_monopoly,
                    );
                }
                frontend_draw::Layer::Menu => {
                    if let Some(menu) = &mut self.menu {
                        menu.draw(&self.renderer, &self.color_view, &self.depth_view)?;
                    }
                }
                frontend_draw::Layer::Statistics => {
                    let menu = self.menu.as_ref().ok_or("Final statistics menu missing")?;
                    let view = self
                        .inspection
                        .as_mut()
                        .ok_or("Final statistics view missing")?;
                    view.stats_panel.update(
                        menu.stats_assets(),
                        &self.renderer,
                        view.live_status.as_ref().and_then(|s| s.stats.as_ref()),
                    )?;
                    if let Some(panel) = &view.stats_panel.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                }
            }
        }
        presenter.profile_menu_submit(submit_started);
        presenter
            .present_binding(&self.binding)
            .map_err(|e| e.to_string())
    }
}
#[path = "intro/draw.rs"]
mod draw;
#[path = "intro/input.rs"]
mod input;

#[cfg(test)]
#[path = "intro/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "intro/focus_worker_tests.rs"]
mod focus_worker_tests;

#[cfg(test)]
#[path = "intro/camera_producer_live_tests.rs"]
mod camera_producer_live_tests;

#[cfg(test)]
#[path = "intro/process_worker_tests.rs"]
mod process_worker_tests;

#[cfg(test)]
#[path = "intro/process_review_tests.rs"]
pub(crate) mod process_review_tests;
