use crate::present::Presenter;

#[path = "intro/camera_live.rs"]
pub(crate) mod camera_live;
#[path = "intro/refresh.rs"]
mod refresh;
#[path = "intro/options_ack.rs"]
mod options_ack;
#[path = "intro/focus.rs"]
pub(crate) mod focus;
#[path = "intro/process_frontend.rs"]
pub(crate) mod process_frontend;
#[path="intro/menu_preferences.rs"]
pub(crate) mod menu_preferences;
#[path = "intro/camera_producer.rs"]
mod camera_producer;
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
    (CameraSample { eye: [26., 586., -695.], target: [26., 463., -595.],
        up: [0., 1., 0.], vertical_fov: NORMAL_CITY_VERTICAL_FOV,
        },
     openmt2001::camera::BlockCamera { center: [51., 463., -595.],
        angle: -std::f32::consts::FRAC_PI_2, height: 586., radius: 100.,
        },
    )
}

fn update_locator_financial(locator: &mut Option<crate::locator::Locator>,
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
    scene: &mut Scene, camera: CameraSample, detail: Option<u8>, mut changed: Vec<usize>,
) -> Result<Vec<usize>, String> {
    if let Some(detail) = detail {
        changed.extend(scene.apply_normal_star_detail(detail, camera)?);
    }
    changed.sort_unstable();
    changed.dedup();
    Ok(changed)
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
    pub(crate) primary_products: camera_live::deferred_primary::PrimaryProducts,
    pub(crate) ui_images: camera_live::process_images::Images,
    pub(crate) ui_text: camera_live::ui_text_resources::UiTextResources,
    pub(crate) process_runtime:crate::live_host::process_runtime::Handle,
    pub(crate) activity:crate::live_host::process_runtime::Activity,
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
    pub fn load(paths: Paths) -> Result<Self, String> { Self::load_with_source_options(paths,"") }
    pub fn load_with_source_options(paths:Paths,source_options:&str)->Result<Self,String>{
        Self::load_primary_path(paths,source_options,false)
    }
    ///Explicit cold fresh lane; default/CLI behavior remains the reordered
    ///eager lane; complete ordered construction and startup admission remain gated.
    pub(crate) fn load_deferred_primary(paths:Paths,source_options:&str)->Result<Self,String>{
        Self::load_primary_path(paths,source_options,true)
    }
    pub(crate) fn reordered_main(&self)->Result<&camera_live::main_construction::CompletedMainLayout,String>{self.primary_products.reordered()}
    fn load_primary_path(paths:Paths,source_options:&str,deferred:bool)->Result<Self,String>{
        let read = |path: PathBuf| fs::read(&path).map_err(|e| format!("{}: {e}", path.display()));
        let build_file = read(paths.scene.join("BuildEditorModels/BuildFile.dat"))?;
        let executable = read(paths.executable)?;
        let (process_runtime,activity)=crate::live_host::process_runtime::startup(&executable,source_options)?;
        let style = crate::block_sidewalk_labels::load_style(
            &openmt2001::pe::PeImage::parse(&executable)?, &paths.installer,
        )?;
        let directory = read(paths.scene.join("archive.DIR"))?;
        let archive = read(paths.scene.join("archive.bin"))?;
        let maps = paths.map.parent().ok_or("intro map parent")?.to_path_buf();
        let scripts = maps.parent().ok_or("script directory missing")?;
        let map = read(paths.map)?;
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
            .map_err(|e| e.to_string())?;
        let mut ui_text = camera_live::ui_text_resources::UiTextResources::load(
            &executable, &paths.installer, &paths.language, &index, &archive)?;
        let cursors = Some(crate::cursor::Images::load(&index, &archive)?);
        let mut ui_images = camera_live::process_images::Images::load(&executable, &index, &archive, [640,480])?;
        let navigation = crate::navigation::Navigation::load_shared(&index, &archive, &executable, &ui_images)?;
        let mut selection = crate::selection::Assets::load(&executable, &index, &archive)?;
        style.apply_blocks(&mut selection.blocks)?;
        let mut navigator=if deferred{
            crate::navigator::Navigator::load_deferred(&index,&archive,&selection.blocks,&executable)?
        }else{crate::navigator::Navigator::load(&index,&archive,&selection.blocks,&executable)?};
        //The current frontend selects the supplied USA board here.
        ui_text.install_usa_board(&executable, &paths.installer)?;
        let primary_products=if deferred{
            camera_live::deferred_primary::PrimaryProducts::Deferred(camera_live::deferred_primary::DeferredPrimary::allocate(
                &executable,&index,&archive,navigator.process_primitives_mut(),&ui_text,&ui_images)?)
        }else{
        let mut prefix = Some(camera_live::main_construction::Prefix::load(
            &executable, &index, &archive, [640,480], &mut ui_text,
            navigator.process_primitives_mut(), &mut ui_images)?);
        let mut child5 = Some(camera_live::main_construction::AwaitingChild5::continue_child1(
            &mut prefix, &executable, &mut ui_text, navigator.process_primitives_mut(), &mut ui_images)?);
        let mut child2 = Some(camera_live::main_construction::AwaitingChild2::continue_child5(
            &mut child5, &executable, &mut ui_text, navigator.process_primitives_mut(), &mut ui_images)?);
        let mut parent_tail = Some(camera_live::main_construction::AwaitingParentTail::continue_child2(
            &mut child2, &executable, &mut ui_text, navigator.process_primitives_mut(), &mut ui_images)?);
        let main_prefix = camera_live::main_construction::CompletedMainLayout::continue_parent_tail(
            &mut parent_tail, &executable, &mut ui_text, navigator.process_primitives_mut(), &mut ui_images)?;
        camera_live::deferred_primary::PrimaryProducts::Reordered(main_prefix)
        };
        let tooltip = crate::tooltip::Tooltip::load_shared(
            &index, &archive, &paths.installer, &paths.language, &ui_text)?;
        let result_actors = crate::auction_scene::Assets::load_result(
            &openmt2001::pe::PeImage::parse(&executable)?, &index, &archive,
        )?;
        let result_text = crate::fresh_city::result::text::Assets::load_shared(
            &index, &archive, &paths.installer, &paths.language, &ui_text,
        )?;
        let result_medallions = crate::fresh_city::result::medallions::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &index, &archive,
        )?;
        let result_audio = crate::fresh_city::result::audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &paths.installer, &paths.ffmpeg,
        )?;
        let auction_audio = crate::live_host::auction_audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &paths.installer,
        )?;
        let transport_audio = crate::transport_audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &paths.installer,
        )?;
        let era_audio = crate::era_audio::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &paths.installer, &paths.ffmpeg,
        )?;
        let gameplay_sound = crate::gameplay_sound::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &paths.installer,
        )?;
        let time_panel = crate::time_panel::Assets::load_shared(&index, &archive, &paths.installer,
            &paths.language, &executable, &ui_text,
        )?;
        let cash_arrow = crate::cash_arrow::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &index, &archive,
        )?;
        let dust = crate::dust_scene::Assets::load(&index, &archive)?;
        let game_options = crate::game_options::Assets::load(
            &index, &archive, &paths.installer, &paths.language)?;
        let mut save_dialog = crate::save_dialog::Assets::load(
            &index, &archive, &paths.installer, &paths.language)?;
        let save_catalog = crate::campaign_store::load_catalog(
            &openmt2001::pe::PeImage::parse(&executable)?,
            scripts,
        )?;
        save_dialog.set_level_catalog(&save_catalog);
        let settings_menu = crate::settings_menu::Assets::load(
            &index, &archive, &paths.installer, &paths.language,
        )?;
        let sidewalk_labels = crate::block_sidewalk_labels::Assets::load(
            &index, &archive, &paths.installer, &selection.blocks, &style,
        )?;
        let tutorial_visual = crate::tutorial_visual::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &index, &archive,
        )?;
        let city_pulse = crate::city_pulse_scene::Assets::load(&index, &archive)?;
        let result_firework_texture = crate::fresh_city::result::particles_gpu::FireworkTexture::load(&index, &archive)?;
        let strategic_scene = crate::strategic_scene::Assets::load(
            &openmt2001::pe::PeImage::parse(&executable)?, &index, &archive, &selection.blocks,
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
        let track = read(
            paths
                .installer
                .join(format!("Standard/Cut_scenes/intro_{}.tm", paths.quality)),
        )?;
        let track = Flyby::parse(&track)?;
        let samples = (1..track.frame_count())
            .map(|i| track.sample(i))
            .collect::<Result<Vec<_>, _>>()?;
        // Original FLYBY1/hub.lua associates these tracks with these hours.
        let phase = match paths.quality.as_str() {
            "low" => 12,
            "medium" => 20,
            "high" => 8,
            _ => return Err("intro quality must be low, medium or high".into()),
        };
        let scene = render_scene::assemble_with_style(Request {
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
        }, Some(&style),
        )?;
        eprintln!(
            "Intro assembled: {} buildings, {} triangle vertices, {} camera samples; {} track / hour {phase}",
            scene.building_count,
            scene.vertex_count,
            samples.len(),
            paths.quality
        );
        let inputs = Arc::new(SceneInputs {
            compatibility: paths.compatibility,
            style,
            process_runtime:Some(process_runtime.clone()),
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
            process_runtime, activity, primary_products, ui_images, ui_text,
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
    process_runtime:Option<crate::live_host::process_runtime::Handle>,
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
    if eye.into_iter().chain(target).chain(up).any(|x| !x.is_finite()) {
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
        center: [target[0] - angle.sin() * 25., target[1], target[2] + angle.cos() * 25.,
        ],
        angle,
        height: eye[1],
        radius,
    };
    Ok((CameraSample { eye, target, up, vertical_fov,
        }, controls,
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
    if scale <= 0. { return None; }
    let point = [
        (position[0] - (f64::from(size[0]) - 1280. * scale) / 2.) / scale,
        (position[1] - (f64::from(size[1]) - 960. * scale) / 2.) / scale,
    ];
    if point[0] >= 0. && point[0] < 1280. && point[1] >= 0. && point[1] < 960. {
        Some(point.map(|v| v.round() as i32))
    } else { None }
}

/// Saved non-block views may start outside the normal block pan clamp.
/// Preserve their recorded pose during manual inspection; the arithmetic
/// mirrors BlockCamera::tick before its final ±500 center clamp.
fn saved_camera_tick(controls: &mut openmt2001::camera::BlockCamera, keys: [bool; 4],
)
    -> ([f32; 3], [f32; 3]) {
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
fn mode12_input_tick(controls: &mut openmt2001::camera::BlockCamera,
    saved_camera: bool, keys: [bool; 4], drag: [i32; 2],
    sensitivity: i32, invert_vertical: bool,
)
    -> Result<([f32; 3], [f32; 3]), String> {
    mode12_input_tick_with_height(controls,saved_camera,keys,drag,sensitivity,invert_vertical,false)
}
fn mode12_input_tick_with_height(controls:&mut openmt2001::camera::BlockCamera,
    saved_camera:bool,keys:[bool;4],drag:[i32;2],sensitivity:i32,invert_vertical:bool,low_height:bool,
)->Result<([f32;3],[f32;3]),String>{
    let mut pose = if saved_camera { saved_camera_tick(controls, keys) }
        else { controls.tick(keys) };
    if drag != [0; 2] {
        controls.right_drag(&mut pose, drag, sensitivity, invert_vertical, false, low_height)?;
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
struct DetailVisibility { spans: Vec<DetailSpan>,
}
impl DetailVisibility {
    fn add(&mut self, scene: &Scene, category: usize, center: [f32; 2],
        draw: usize, vertices: std::ops::Range<usize>, pivot_y: f32,
    ) -> Result<(), String> {
        let source = scene.draws.get(draw).ok_or("detail span draw missing")?;
        let slice = source.vertices.get(vertices.clone()).ok_or("detail span leaves draw")?;
        if !slice.is_empty() {
            self.spans.push(DetailSpan {category,center,draw,start:vertices.start,
                positions:slice.iter().map(|vertex| [vertex[0],vertex[1],vertex[2]])
                    .collect(),pivot_y,presentation:None,
            });
        }
        Ok(())
    }

    /// Original 4b7fd0 pass3 scales each descriptor's Y axis through the
    /// 50-unit depth band, then skips category details past far. Ordinary
    /// pass1 building materials remain submitted at every setting.
    fn reframe(&mut self, scene: &mut Scene, camera: CameraSample, distance: u8,
    )
        -> Result<Vec<usize>, String> {
        let factor = crate::people_scene::Policy::distance_factor(u32::from(distance))?;
        let fade = crate::category_distance::Fade::from_factor(factor);
        let view = openmt2001::camera::scene_view_with_up(
            camera.eye, camera.target, camera.up);
        let frustum = crate::selection::CategoryFrustum::new(camera)?;
        let mut visibility = std::collections::BTreeMap::new();
        let mut changed = Vec::new();
        for span in &mut self.spans {
            let scale = if let Some(&scale) = visibility.get(&span.category) {
                scale
            } else {
                let depth = openmt2001::camera::translated_scene_view(view,
                    [span.center[0], 0., span.center[1]],
                )[11];
                let scale = if frustum.visible(span.category, span.center,
                    scene.category_model_heights.get(&span.category).copied(),
                )? {
                    fade.detail_scale(depth)
                } else { None };
                visibility.insert(span.category, scale);
                scale
            };
            let presentation = (scale.is_some(), scale.unwrap_or(0.).to_bits());
            if span.presentation == Some(presentation) { continue; }
            let draw = scene.draws.get_mut(span.draw).ok_or("detail span draw missing")?;
            let vertices = draw.vertices.get_mut(span.start..span.start + span.positions.len())
                .ok_or("detail span leaves draw")?;
            let hidden = span.positions[0];
            for (vertex, &position) in vertices.iter_mut().zip(&span.positions) {
                if let Some(scale) = scale {
                    vertex[..3].copy_from_slice(&[position[0],
                        span.pivot_y + (position[1] - span.pivot_y) * scale, position[2],
                    ]);
                } else {
                    vertex[..3].copy_from_slice(&hidden);
                }
            }
            span.presentation = Some(presentation);
            changed.push(span.draw);
        }
        changed.sort_unstable();changed.dedup();
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
    restored_presentation: Vec<openmt2001::category_ownership::Seam>,
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
        category: blocks.iter().position(|block| block.id == native_category)?,
        building,
    })
}
#[cfg(test)]
struct DispatchProbe {pause:[u8;9],tasks:Vec<openmt2001::timers::Task>,frequency:(f32,Option<usize>,Option<usize>),frame:openmt2001::timers::Frame,epoch:Instant}
enum FreshRequest {
    #[cfg(test)] DispatchProbe(mpsc::Sender<DispatchProbe>),
    CameraTurn { turn: camera_producer::Turn,
        reply: mpsc::Sender<Result<camera_producer::Payload,String>> },
    UiCameraReady { reply: mpsc::Sender<Result<(), String>> },
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
        reply: mpsc::Sender<Result<openmt2001::outcome_notice::Sequence, String>>,
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
    Options { open:bool, reply:mpsc::Sender<Result<(),String>> },
    ToggleSpeed,
    DismissPrompt(u32, Option<mpsc::Sender<Vec<crate::prompt::Request>>>),
    TutorialControl(u32),
    TutorialProducer { control: u32, event: u32,
    },
    TutorialPick(crate::tutorial_two_interaction::Pick),
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
        reply: mpsc::Sender<Result<[i32; 3], String>>,
    },
    Build {
        form: crate::build_panel::Request,
        submit: bool,
        reply: mpsc::Sender<Result<crate::fresh_city::BuildInspection, String>>,
    },
    Inspect {
        camera: CameraSample,
        reply: mpsc::Sender<Result<LoadedInspection, String>>,
    },
}
impl SceneInputs {
    fn styled_board(&self, image: &openmt2001::pe::PeImage<'_>,
    )
        -> Result<openmt2001::board::Board, String> {
        let mut board = openmt2001::board::Board::from_v14(image)?;
        self.style.apply_board(&mut board)?;
        Ok(board)
    }

    fn preview_worker(
        &self,
        setup: crate::singleplayer::Setup,
        camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, String>>,
    ) {
        self.session_worker(WorkerStart::Fresh(setup), camera, tx, None);
    }
    fn saved_worker(&self, path: PathBuf, save_profile_root: Option<PathBuf>, camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, String>>,
        pavement:crate::selection::pavement_resource::LocalPavementHandle,
    ) {
        self.session_worker(WorkerStart::Saved(path, save_profile_root), camera, tx, Some(pavement));
    }
    fn session_worker(&self, start: WorkerStart, camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, String>>,
        pavement:Option<crate::selection::pavement_resource::LocalPavementHandle>,
    ) {
        self.session_worker_with_focus(start, camera, tx, pavement, false);
    }
    fn session_worker_with_focus(&self, start: WorkerStart, camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, String>>,
        pavement:Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment: bool,
    ) {
        self.session_worker_with_sources(start, camera, tx, pavement, require_focus_attachment, false);
    }
    fn session_worker_with_sources(&self, start: WorkerStart, camera: CameraSample,
        tx: mpsc::Sender<Result<LoadedInspection, String>>,
        pavement: Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment: bool, produce_camera: bool,
    ) {
        self.session_worker_with_terminal(start,camera,tx,pavement,require_focus_attachment,produce_camera,
            camera_producer::ExitFailures::default());
    }
    fn session_worker_with_terminal(&self,start:WorkerStart,camera:CameraSample,
        tx:mpsc::Sender<Result<LoadedInspection,String>>,
        pavement:Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment:bool,produce_camera:bool,exit_failures:camera_producer::ExitFailures,
    ) {
        let _=self.session_worker_joined(start,camera,tx,pavement,require_focus_attachment,produce_camera,exit_failures,None);
    }
    fn session_worker_joined(&self,start:WorkerStart,camera:CameraSample,
        tx:mpsc::Sender<Result<LoadedInspection,String>>,
        pavement:Option<crate::selection::pavement_resource::LocalPavementHandle>,
        require_focus_attachment:bool,produce_camera:bool,exit_failures:camera_producer::ExitFailures,
        custody:Option<crate::process_dispatch::DispatchCustody>,
    ) -> Option<crate::process_dispatch::DispatchCustody> {
        let joined=custody.is_some();
        let fresh_completion=matches!(&start,WorkerStart::Fresh(_)|WorkerStart::Skirmish(_));
        let (mut dispatch,mut timing)=if let Some(custody)=custody {
            match custody.prepare_start(matches!(&start,WorkerStart::Fresh(_)|WorkerStart::Skirmish(_))) {
                Ok((seed,timing))=>(Some(seed),timing),
                Err(error)=>{let _=tx.send(Err(error));return None;}
            }
        } else {
            // Explicit isolated caller path. Ordinary App ALWAYS supplies custody.
            let mut frame=openmt2001::timers::Frame::default(); frame.initialize(0);
            (None,crate::process_dispatch::Timing{frame,epoch:Instant::now()})
        };

        if let Some(process)=&self.process_runtime {
            if let Err(error)=process.world_controller_reset4c2b80(){let _=tx.send(Err(error.to_string()));return None;}
        }
        let _offline_lease=camera_producer::OfflineLease{process:self.process_runtime.clone(),failures:Arc::clone(&exit_failures)};
        if matches!(&start,WorkerStart::Fresh(_)|WorkerStart::Skirmish(_)) {
            if let Some(process)=&self.process_runtime {
                if let Err(error)=process.source_fresh_world(){let _=tx.send(Err(error.to_string()));return None;}
            }
        }
        let mut cache = SnapshotCache::default();
        let selected_save_root = match &start {
            WorkerStart::Saved(_, root) => root.clone(),
            WorkerStart::Skirmish(request) => Some(request.save_root.clone()),
            WorkerStart::Fresh(setup) => {
                crate::save_menu::profile_root_for_campaign(&self.profile_root, &setup.profile_path)
            }
        };
        // Private native-review activation. It admits only imported ordinary
        // routes with separately inspected first-Level effects. Fresh/menu and
        // generic tutorial producer ordering remain outside this experiment.
        let review_camera = matches!(&start, WorkerStart::Saved(..))
            && std::env::var_os("OPENMT2001_CAMERA_OWNER_NATIVE_REVIEW").is_some();
        let result = match start {
            WorkerStart::Skirmish(request) => self.fresh_skirmish_joined(request, camera, &mut cache,dispatch.take()),
            WorkerStart::Fresh(setup) => {
                let retained = match setup.preview {
                    crate::singleplayer::Preview::Scenario(id) => {
                        crate::scenarios::live_tutorial(id)
                            || matches!(id, 200001..=200016 | 400001)
                            || setup.catalog.find(id).is_some_and(|index| {
                                crate::fresh_city::custom::valid_level(&setup.catalog.levels[index])
                            })
                    }
                    crate::singleplayer::Preview::Sandbox => true,
                };
                if retained {
                    self.fresh_preview_joined(setup, camera, &mut cache,dispatch.take())
                } else {
                    let _ = tx.send(self.preview_scene(setup, camera));
                    return None;
                }
            }
            WorkerStart::Saved(path, _) => self.saved_preview_with_selection_joined(
                &path, camera, &mut cache, Some(selected_save_root.as_deref()),dispatch.take(),
            ),
        };
        match result {
            Ok((mut loaded, mut session)) => {
                // Ordinary saves belong to the selected profile independently
                // of whether this session may persist campaign progression.
                let save_profile_root = selected_save_root;
                loaded.save_profile_root = save_profile_root.clone();
                let saved_auction = loaded.saved_auction.take();
                let restored_presentation = std::mem::take(&mut loaded.restored_presentation);
                let host_eye = loaded.saved_viewpoint.map_or(camera.eye, |(sample, _)| sample.eye);
                let (lease, released) = mpsc::channel();
                loaded.fresh_session = Some(lease);
                let (auction_renderer, auction_requests) = mpsc::channel();
                loaded.auction_requests = Some(auction_requests);
                let (auction_audio, auction_audio_requests) = mpsc::channel();
                loaded.auction_audio_requests = Some(auction_audio_requests);
                let (transport_audio, transport_audio_requests) = mpsc::channel();
                loaded.transport_audio_requests = Some(transport_audio_requests);
                let (era_audio, era_audio_requests) = mpsc::channel();
                loaded.era_audio_requests = Some(era_audio_requests);
                let (gameplay_sound, gameplay_sound_requests) = mpsc::channel();
                loaded.gameplay_sound_requests = Some(gameplay_sound_requests);
                let camera_connection = if review_camera {
                    let id = session.catalog.selected_level().map(|level| level.id());
                    if !matches!(id, Some(200003 | 400002)) {
                        let _ = tx.send(Err("Private camera review has no first-Level contract for this route".into()));
                        return None;
                    }
                    let Some(capture) = loaded.imported_cameras else {
                        let _ = tx.send(Err("Private camera review requires both imported raw records".into()));
                        return None;
                    };
                    let generation = session.world.topology.epoch().serial();
                    let (worker, inbox) = crate::live_host::camera_bridge::channel(generation);
                    loaded.camera_inbox = Some(inbox);
                    let Some(pavement)=pavement else {
                        let _=tx.send(Err("Private camera review requires originating selection pavement registration".into()));
                        return None;
                    };
                    Some((worker,capture,pavement))
                } else {None};
                let ui_camera_reader = if produce_camera && !review_camera {
                    let (publisher, reader) = crate::save_menu::camera_source::channel(session.world.topology.epoch());
                    loaded.camera_epoch = Some(session.world.topology.epoch());
                    loaded.camera_publisher = Some(publisher);
                    Some(reader)
                } else { None };
                if fresh_completion {
                    timing.frame.synchronize(timing.epoch.elapsed().as_millis() as u32);
                    for task in &mut session.hooks.timers {task[12..16].copy_from_slice(&0f32.to_le_bytes());}
                }
                if tx.send(Ok(loaded)).is_ok() {
                    let mut host = crate::live_host::Host::new(host_eye);
                    macro_rules! cancelled_session {
                        () => {{
                            if let Some(error)=&host.failure {camera_producer::record_exit(&exit_failures,error.clone());}
                            if joined {
                                match crate::process_dispatch::DispatchCustody::from_session(&mut session,timing) {
                                    Ok(custody)=>return Some(custody),
                                    Err(error)=>camera_producer::record_exit(&exit_failures,error),
                                }
                            }
                            return None;
                        }};
                    }
                    if let Some(reader) = &ui_camera_reader {
                        if let Err(error) = host.install_ui_camera_source(&session, reader.clone()) {
                            host.fail(error); return None;
                        }
                    }
                    host.autosave_profile_root=save_profile_root.clone();
                    host.process_runtime=self.process_runtime.clone();
                    // SAME process Frame/epoch; fresh completion above uses
                    //4ebaa0 only. Saved adoption never initializes the manager.
                    if let Some((worker,capture,pavement)) = camera_connection {
                        // Host already classifies the typed install result;
                        // explicit retirement is not an incomplete simulation.
                        match openmt2001::pe::PeImage::parse(&self.executable).map_err(crate::live_host::camera_bridge::RequestError::from)
                            .and_then(|image|host.install_camera_bridge(&mut session,&image,crate::live_host::CameraSupplierPolicy::CameraOnly,worker,capture,pavement)) {
                            Ok(()) => {}
                            Err(crate::live_host::camera_bridge::RequestError::Retired) => cancelled_session!(),
                            Err(crate::live_host::camera_bridge::RequestError::Failed(error)) => host.fail(error),
                        }
                        if host.camera_retired() { cancelled_session!(); }
                    }
                    host.time_runtime = Arc::clone(&self.time_runtime);
                    host.auction_renderer = Some(auction_renderer);
                    host.auction_audio = Some(auction_audio);
                    host.transport_audio = Some(transport_audio);
                    host.era_audio = Some(era_audio);
                    host.gameplay_sound = Some(gameplay_sound);
                    if let Err(error) = host.begin_presentation(&session) {
                        host.fail(error);
                    }
                    if host.camera_retired() { cancelled_session!(); }
                    //4ba167 enables gameplay after the startup pause counter
                    //has drained; retained script/modal pause reasons still
                    //gate each logic dispatch independently.
                    host.set_running(true);
                    if host.camera_retired() { cancelled_session!(); }
                    if let Err(error) = host.apply_restored_presentation(restored_presentation) {
                        host.fail(error);
                    }
                    if host.camera_retired() { cancelled_session!(); }
                    if let Err(error) = host.activate_saved_auction(&mut session) {
                        host.fail(error);
                    }
                    if host.camera_retired() { cancelled_session!(); }
                    if let Some(continuation) = saved_auction
                        && let Err(error) = continuation.apply(&mut session)
                            .and_then(|()| {
                            host.restore_auction_widgets(continuation.host.as_ref(), &session)
                        })
                    {
                        host.fail(error);
                    }
                    let image = match openmt2001::pe::PeImage::parse(&self.executable) {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("{error}");
                            return None;
                        }
                    };
                    let board = match self.styled_board(&image) {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("{error}");
                            return None;
                        }
                    };
                    let file = match openmt2001::build_file::BuildFile::parse(&self.build_file) {
                        Ok(value) => value,
                        Err(error) => {
                            eprintln!("{error}");
                            return None;
                        }
                    };
                    let assets = crate::fresh_city::CommandAssets {
                        image: &image,
                        board: &board,
                        file: &file,
                    };
                    if host.camera_retired() { cancelled_session!(); }
                    if let Some(reader) = &ui_camera_reader {
                        // The UI publishes its complete cold transaction before
                        // this barrier. No frame/save observes a partial owner.
                        match released.recv() {
                            Ok(FreshRequest::UiCameraReady { reply }) => {
                                let result = reader.read(session.world.topology.epoch()).map(|_| ());
                                let failed = result.is_err();
                                let _ = reply.send(result);
                                if failed { return None; }
                            }
                            Err(_)=>cancelled_session!(),
                            Ok(_)=>{camera_producer::record_exit(&exit_failures,"Expected real camera Ready barrier".into());return None;},
                        }
                    }
                    if require_focus_attachment {
                        // Explicit modern cold policy: all restore/startup and
                        // presentation effects above precede ONE live OS-focus
                        // snapshot. No frame or queued save may pass this barrier.
                        match released.recv() {
                            Ok(FreshRequest::Focus(event)) if event.is_attachment() => {
                                if event.apply(&mut host, &mut session.world.pause, &mut session.hooks.timers).is_err() {
                                    return None;
                                }
                            }
                            Ok(_) => {
                                host.fail("First focus-enabled request must attach current OS focus".into());
                                return None;
                            }
                            Err(_) => cancelled_session!(),
                        }
                    }
                    let mut last_tick = Instant::now();
                    loop {
                        if host.camera_retired() || ui_camera_reader.as_ref().is_some_and(|reader|
                            reader.read(session.world.topology.epoch()).is_err()) { break; }
                        // Poll original interface/logic timers from monotonic host
                        // milliseconds. Rendering and worker requests are separate.
                        let period = std::time::Duration::from_millis(16);
                        let request = match released
                            .recv_timeout(period.saturating_sub(last_tick.elapsed()))
                        {
                            Ok(request) => Some(request),
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        if host.camera_retired() || ui_camera_reader.as_ref().is_some_and(|reader|
                            reader.read(session.world.topology.epoch()).is_err()) { break; }
                        // Camera selection is an admitted FIFO prefix too. All
                        // Session borrows end before its by-value reply wait.
                        let request = match request {
                            Some(FreshRequest::CameraTurn {turn,reply}) => {
                                let admitted = camera_producer::admit(&session,turn);
                                if let Ok(posts) = camera_producer::deliver(turn,admitted,reply) {
                                    // Consumed once, on this SAME validated Session, after
                                    // camera/UI publication and before next frame/request.
                                    if camera_producer::run_posts(&mut host,&mut session,&assets,posts,&exit_failures).is_err(){return None;}
                                }
                                if ui_camera_reader.as_ref().is_some_and(|reader|
                                    reader.read(session.world.topology.epoch()).is_err()) { break; }
                                continue;
                            }
                            other => other,
                        };
                        // Received focus takes effect before any next due frame.
                        // No Options-style preflush and no completed-frame replay.
                        let request = match focus::before_frame(request, &mut host,
                            &mut session.world.pause, &mut session.hooks.timers) {
                            Ok(focus::BeforeFrame::FocusApplied) => continue,
                            Ok(focus::BeforeFrame::Request(other)) => other,
                            Err(_) => break,
                        };
                        if last_tick.elapsed() >= period {
                            let elapsed =
                                timing.frame.elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                            last_tick = Instant::now();
                            host.frame(&mut session, &assets, elapsed);
                        }
                        if host.camera_retired() { break; }
                        let Some(request) = request else {
                            continue;
                        };
                        match request {
                            FreshRequest::Focus(_) => unreachable!("focus handled before frame"),
                            FreshRequest::CameraTurn {..} => unreachable!("camera turn handled before frame"),
                            FreshRequest::UiCameraReady { reply } => {
                                let _ = reply.send(Err("Camera attachment already completed".into()));
                            }
                            FreshRequest::PropertyRights { category, panel_mode, activate, reply,
                            } => {
                                let _ = reply.send(host.property_rights(
                                    &mut session, &assets, category, panel_mode, activate,
                                ));
                            }
                            FreshRequest::PropertyOffer { request, reply } => {
                                let result = if request.submit {
                                    host.property_offer(&mut session, &assets,
                                        request.category, request.panel,
                                    )
                                } else {
                                    host.inspect_property_offer(&mut session, &assets,
                                        request.category, request.panel, request.initialize_amount,
                                    )
                                };
                                let _ = reply.send(result);
                            }
                            FreshRequest::SaveSelected { target, camera, capture, game_mode, interface_mode, reply,
                            } => {
                                let result = (|| {
                                    let root = save_profile_root.as_deref()
                                        .ok_or("Save Game requires a selected V2 profile")?;
                                    if host.has_ui_camera_source() {
                                        return host.save_selected_from_ui_snapshot(&mut session,
                                            &image, root, &target, game_mode);
                                    }
                                    if capture.is_some() && !host.options_save_allowed(&session.world.pause,&session.hooks.timers) {
                                        return Err("Save requires the active Options hold".to_owned());
                                    }
                                    //The by-value pair retains Owner's raw bytes. Do not
                                    //recompute a sample or overlay either record here.
                                    let cameras = match capture {
                                        Some(capture) => {
                                        if review_camera {eprintln!("[camera] serializing captured raw pair");}
                                        capture.into_records()
                                    },
                                        None => crate::save_menu::cameras(session.saved_cameras, camera)?,
                                    };
                                    let bytes = session.save_v2_with_options_view(
                                        &image, cameras, game_mode, &host, interface_mode,
                                    )?;
                                    crate::save_menu::write_target(root, &target, &bytes)
                                })();
                                let _ = reply.send(result);
                            }
                            FreshRequest::DeleteSave { id, reply } => {
                                let result = save_profile_root.as_deref()
                                    .ok_or("Save Game requires a selected V2 profile".to_owned())
                                    .and_then(|root| crate::save_menu::delete_slot(root, &id));
                                let _ = reply.send(result);
                            }
                            FreshRequest::ResultState { reply } => {
                                let _ = reply.send(session.result_presentation().cloned()
                                    .map(|presentation| ResultSnapshot {
                                        presentation,
                                        tick: session.world.clock.timing[0],
                                        hour: session.world.clock.hour(),
                                        counter: session.world.clock.timing[2],
                                    })
                                    .ok_or("Result request has no retained script state".into(),
                                        ),
                                );
                            }
                            FreshRequest::ResultCredits { reply } => {
                                let result = session.construct_result_credits(&image)
                                    .and_then(|_| {
                                        session.result_presentation().cloned()
                                        .ok_or("Result credits lost retained script state".into(),
                                        )
                                    });
                                let _ = reply.send(result);
                            }
                            FreshRequest::ResultEvent { name, reply } => {
                                let result = host.dispatch_result_event(&mut session, &assets, name)
                                    .and_then(|_| {
                                            session.result_presentation().cloned()
                                        .ok_or("Result event lost retained script state".into(),
                                            )
                                        });
                                let _ = reply.send(result);
                            }
                            FreshRequest::ResultComplete { reply } => {
                                let _ = reply.send(host.complete_result_sequence(&mut session));
                            }
                            FreshRequest::InspectOperations {
                                building,
                                bounds,
                                reply,
                            } => {
                                let _ = reply.send(
                                    session.inspect_business_operations(building, bounds, &image),
                                );
                            }
                            FreshRequest::BusinessOperation {
                                building,
                                action,
                                reply,
                            } => {
                                let _ = reply.send(host.business_operation(
                                    &mut session,
                                    &assets,
                                    building,
                                    action,
                                ));
                            }
                            FreshRequest::Card(action) => {
                                if let Err(error) = host.human_card(&mut session, &assets, action) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::HumanDialog(response, reply) => {
                                if let Err(error) =
                                    host.human_dialog_reply(&mut session, &assets, response)
                                {
                                    host.fail(error);
                                }
                                let _ = reply.send(host.take_prompts(&session));
                            }
                            FreshRequest::AuctionButton(button) => {
                                if let Err(error) =
                                    host.auction_button(&mut session, &assets, button)
                                {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::Residents { reply } => {
                                let result = crate::people_scene::Layer::from_resources(
                                    &session,
                                    &self.suncols,
                                    cache.people.as_ref().unwrap(),
                                );
                                let _ = reply.send(result);
                            }
                            FreshRequest::Vehicles { reply } => {
                                let result = session.road_traffic
                                    .project(&mut session.world.road_network, 0.)
                                    .and_then(|()| {
                                        crate::vehicle_scene::Layer::from_resources(
                                        &session,
                                        &self.suncols,
                                        cache.vehicles.as_ref().unwrap(),
                                    )
                                    });
                                let _ = reply.send(result);
                            }
                            FreshRequest::Transport { reply } => {
                                let mut auxiliaries = Vec::new();
                                let result = session
                                    .advance_transport_frame(|auxiliary| {
                                        auxiliaries.push(*auxiliary);
                                        Ok(())
                                    })
                                    .and_then(|()| {
                                        crate::transport_scene::Layer::from_resources(
                                        &session,
                                        &self.suncols,
                                        cache.transport.as_ref().unwrap(),
                                        auxiliaries,
                                    )
                                    });
                                let _ = reply.send(result);
                            }
                            FreshRequest::EditBusiness {
                                target,
                                action,
                                reply,
                            } => {
                                let result = if host.failure.is_some() {
                                    Err("Stopped session cannot edit businesses".into())
                                } else {
                                    crate::business_edit::edit(&mut session, &assets, target, action,
                                    )
                                };
                                let _ = reply.send(result);
                            }
                            FreshRequest::Run(running) => {
                                let elapsed = timing.frame
                                    .elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                                host.frame(&mut session, &assets, elapsed);
                                if host.camera_retired() { break; }
                                host.set_running(running);
                                last_tick = Instant::now();
                            }
                            FreshRequest::Options{open,reply} => {
                                let elapsed = timing.frame
                                    .elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                                host.frame(&mut session, &assets, elapsed);
                                if host.camera_retired() { break; }
                                let result=host.options(&mut session.world.pause,&mut session.hooks.timers,open);
                                if let Err(error)=&result {host.fail(error.clone());}
                                let _=reply.send(result);
                                last_tick = Instant::now();
                            }
                            FreshRequest::ToggleSpeed => host.toggle_speed(),
                            FreshRequest::Stats(action) => {
                                if let Err(error)=host.stats_action(&mut session,action) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::DismissPrompt(id, reply) => {
                                let elapsed = timing.frame
                                    .elapsed(Some(timing.epoch.elapsed().as_millis() as u32));
                                host.frame(&mut session, &assets, elapsed);
                                if host.camera_retired() { break; }
                                if host.failure.is_none()
                                    && let Err(error) = session.dispatch_session_event(
                                        "InterfacePromptDismiss",
                                        id as i32,
                                        &assets,
                                        true,
                                    )
                                {
                                    host.fail(error);
                                }
                                last_tick = Instant::now();
                                if let Some(reply) = reply {
                                    let _ = reply.send(host.take_prompts(&session));
                                }
                            }
                            FreshRequest::TutorialControl(id) => {
                                if let Err(error) = host.tutorial_event(&mut session, &assets, id) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::TutorialProducer { control, event } => {
                                if let Err(error) = host.tutorial_producer_event(&mut session, &assets, control, event,
                                ) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::TutorialPick(pick) => {
                                if let Err(error) = host.tutorial_pick(&mut session, &assets, pick) {
                                    host.fail(error);
                                }
                            }
                            FreshRequest::Status { camera, reply } => {
                                host.observe_presentation_camera(camera);
                                let _ = reply.send(host.status(&session));
                            }
                            #[cfg(test)] FreshRequest::DispatchProbe(reply)=>{
                                let _=reply.send(DispatchProbe{pause:session.world.pause.bytes,tasks:session.hooks.timers.clone(),
                                    frequency:(session.hooks.frequency.frequency,session.hooks.frequency.calendar_timer,session.hooks.frequency.logic_timer),
                                    frame:timing.frame.clone(),epoch:timing.epoch});
                            }
                            FreshRequest::LogicTick(reply) => {
                                let clock = &session.world.clock;
                                let _ = reply.send(crate::time_panel::ClockSample {
                                    ticks: clock.timing[0], interface_ticks: host.interface_ticks(),
                                    year: 1930 + clock.calendar[2] * 10 + clock.calendar[1],
                                    hour: clock.hour(), fraction: clock.timing[2],
                                    speed: host.policy.scheduler.iterations,
                                });
                            }
                            FreshRequest::Planning { category, reply } => {
                                let _ = reply.send(crate::build_panel::Context::from_session(
                                    &session, category,
                                ));
                            }
                            FreshRequest::BuildDrag { reply } => {
                                let result = host.complete_build_drag(&mut session, &assets);
                                if let Err(error) = &result { host.fail(error.clone()); }
                                let _ = reply.send(result);
                            }
                            FreshRequest::Build {
                                form,
                                submit,
                                reply,
                            } => {
                                let result = (|| {
                                    if submit && host.failure.is_some() {
                                        return Err("Stopped session cannot submit construction".into());
                                    }
                                    let image = openmt2001::pe::PeImage::parse(&self.executable)?;
                                    let board = self.styled_board(&image)?;
                                    let file =
                                        openmt2001::build_file::BuildFile::parse(&self.build_file)?;
                                    let assets = crate::fresh_city::CommandAssets {
                                        image: &image,
                                        board: &board,
                                        file: &file,
                                    };
                                    if submit {
                                        session.submit_build(&form,&assets,|event|
                                            Err(format!("paused worker received immediate command effect {event:?}")))
                                    } else {
                                        session.inspect_build(&form, &assets)
                                    }
                                })();
                                let _ = reply.send(result);
                            }
                            FreshRequest::Inspect { camera, reply } => {
                                let _ = reply.send(self.inspect_fresh(
                                    &mut session,
                                    camera,
                                    &mut cache,
                                ));
                            }
                        }
                    }
                    if let Some(error)=&host.failure {camera_producer::record_exit(&exit_failures,error.clone());}
                }
                if joined {
                    match crate::process_dispatch::DispatchCustody::from_session(&mut session,timing) {
                        Ok(custody)=>{drop(session);return Some(custody);}
                        Err(error)=>camera_producer::record_exit(&exit_failures,error),
                    }
                }
                drop(session);
            }
            Err(error) => {
                let _ = tx.send(Err(error));
            }
        }
        None
    }
    fn fresh_skirmish(
        &self,
        request: crate::menu::skirmish_route::Request,
        camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.fresh_skirmish_joined(request,camera,cache,None)
    }
    fn fresh_skirmish_joined(
        &self,
        request: crate::menu::skirmish_route::Request,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        dispatch:Option<crate::process_dispatch::Seed>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        let directory = request.launch.directory.clone();
        let profile_name = request.launch.participants[0].name.clone();
        let difficulty = request.launch.difficulty;
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let people = openmt2001::people::PeopleModels::load(&image, &index, &self.archive)?;
        let parameters = fs::read_to_string(self.installer.join("parameters/parameter.prm"))
            .map_err(|e| e.to_string())?;
        let timetable = fs::read_to_string(self.installer.join("parameters/timetable.prm"))
            .map_err(|e| e.to_string())?;
        let scripts = self.maps.parent().ok_or("script directory missing")?;
        // Catalog directory spelling is logical Windows data. Resolve its
        // actual supplied filesystem spelling before the Session's direct reads.
        let hub_path =
            crate::campaign_store::original_path(scripts, &format!("{directory}/hub.lua"))?
                .ok_or("selected Skirmish hub missing")?;
        let directory = hub_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .ok_or("selected Skirmish directory is not text")?
            .to_owned();
        // Sample once immediately before actual offline fresh Session construction.
        // Tests may call initialize_skirmish with an injected Policy instead.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs() as u32;
        let mut session = crate::fresh_city::Session::initialize_joined(
            crate::fresh_city::Inputs {
                tree_locators: Some(openmt2001::trees::Assets::load(&image, &index, &self.archive)?.locators,
                ),
                image: &image,
                board: &board,
                file: &file,
                people,
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                road_routes: fs::read(self.installer.join("Standard/route_smalltable.bin"))
                    .map_err(|e| format!("road route table: {e}"))?,
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image,
                    &index,
                    &self.archive,
                )?,
                scripts,
                parameters: &parameters,
                era_styles: crate::fresh_city::load_era_styles(&self.installer.join("parameters"))?,
                timetable: &timetable,
                scenario: &directory,
                difficulty,
                selected_character: request.launch.participants[0]
                    .token
                    .ok_or("human token missing")?,
                profile_name: &profile_name,
                character_names: &self.character_names,
            },
            crate::fresh_city::Policy {
                seed,
                auxiliary_prior_bytes: [0; 0x11c],
            },
            Some(request.catalog),None,false,Some(&self.style),crate::compatibility::Mode::StrictV14,
            Some(request.launch),self.process_runtime.clone(),dispatch,
        )?;
        let loaded = self.inspect_fresh(&mut session, camera, cache)?;
        Ok((loaded, session))
    }

    fn fresh_preview(&self,
        setup: crate::singleplayer::Setup, camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.fresh_preview_joined(setup,camera,cache,None)
    }
    fn fresh_preview_joined(&self,
        setup: crate::singleplayer::Setup, camera: CameraSample,
        cache: &mut SnapshotCache,
        dispatch:Option<crate::process_dispatch::Seed>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        let (scenario_id, scenario_token) = match setup.preview {
            crate::singleplayer::Preview::Scenario(id) => {
                if let Some(level) = setup.catalog.find(id).map(|i| &setup.catalog.levels[i])
                    && crate::fresh_city::custom::valid_level(level)
                {
                    (
                        id,
                        std::str::from_utf8(level.directory())
                            .map_err(|_| "custom scenario directory is not UTF-8")?
                            .to_owned(),
                    )
                } else {
                    let scenario = crate::scenarios::find(id)?;
                (id, scenario.token.to_owned())
                }
            }
            // Supplied v1.4 GAMESTART registers ID 400002 as "sandbox";
            // its hub creates the resident city and must run through Session.
            crate::singleplayer::Preview::Sandbox => (400002, "sandbox".to_owned()),
        };
        if setup.catalog.find(scenario_id).is_none() {
            return Err(format!(
                "level {scenario_id} is absent from the selected profile catalog"
            ));
        }
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let people = openmt2001::people::PeopleModels::load(&image, &index, &self.archive)?;
        let parameters = fs::read_to_string(self.installer.join("parameters/parameter.prm"))
            .map_err(|e| e.to_string())?;
        let timetable = fs::read_to_string(self.installer.join("parameters/timetable.prm"))
            .map_err(|e| e.to_string())?;
        let scripts = self.maps.parent().ok_or("script directory missing")?;
        let difficulty = match setup.difficulty {
            crate::singleplayer::Difficulty::Easy => 0,
            crate::singleplayer::Difficulty::Medium => 1,
            crate::singleplayer::Difficulty::Hard => 2,
        };
        let is_custom = setup.catalog.find(scenario_id).is_some_and(|index| {
            crate::fresh_city::custom::valid_level(&setup.catalog.levels[index])
        });
        // Reproducible preview policy. These unspecified prior bytes are not
        // original captured stack data, nor is seed0 the original host seed.
        let input = crate::fresh_city::Inputs {
            tree_locators: Some(
                openmt2001::trees::Assets::load(&image, &index, &self.archive)?.locators,
            ),
            image: &image,
            board: &board,
            file: &file,
            people,
            vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                &image,
                &index,
                &self.archive,
            )?,
            road_routes: fs::read(self.installer.join("Standard/route_smalltable.bin"))
                .map_err(|e| format!("road route table: {e}"))?,
            transport_models: openmt2001::traffic::models::Models::from_original(
                &image,
                &index,
                &self.archive,
            )?,
            scripts,
            parameters: &parameters,
            era_styles: crate::fresh_city::load_era_styles(&self.installer.join("parameters"))?,
            timetable: &timetable,
            scenario: &scenario_token,
            difficulty,
            selected_character: setup.character_index,
            profile_name: &setup.profile_name,
            character_names: &self.character_names,
        };
        let seed = if is_custom {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| "offline launch clock predates Unix epoch")?
                .as_secs() as u32
        } else {
            0
        };
        let policy = crate::fresh_city::Policy {
            seed,
            auxiliary_prior_bytes: [0; 0x11c],
        };
        let mut session = crate::fresh_city::Session::initialize_joined(input,policy,
            Some(setup.catalog.clone()),Some(setup.profile_path.clone()),false,Some(&self.style),
            self.compatibility,None,self.process_runtime.clone(),dispatch)?;
        eprintln!(
            "Fresh city preview: {}, difficulty {difficulty}, {} residents, {} buildings; host seed{seed} / auxiliary prior bytes zero; player roster initialized; live worker enabled",
            scenario_token,
            session.world.ledgers.residents.active_count,
            session.models.bindings.len()
        );
        if is_custom {
            eprintln!(
                "Authored custom offline launch used host seed{seed}; named profile progress retained"
            );
        }
        session.setup = Some(setup);
        let loaded = self.inspect_fresh(&mut session, camera, cache)?;
        Ok((loaded, session))
    }

    #[cfg(test)]
    fn saved_preview(&self, path: &std::path::Path, camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.saved_preview_with_selection(path, camera, cache, None)
    }
    /// `Some(None)` represents a menu with no active profile. Tests invoking
    /// saved_preview directly use the unrestricted read-only preview path.
    fn saved_preview_with_selection(
        &self,
        path: &std::path::Path,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        selected_root: Option<Option<&std::path::Path>>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        self.saved_preview_with_selection_joined(path,camera,cache,selected_root,None)
    }
    fn saved_preview_with_selection_joined(
        &self,
        path: &std::path::Path,
        camera: CameraSample,
        cache: &mut SnapshotCache,
        selected_root: Option<Option<&std::path::Path>>,
        dispatch:Option<crate::process_dispatch::Seed>,
    ) -> Result<(LoadedInspection, crate::fresh_city::Session<'_>), String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let save = openmt2001::save::SaveFile::parse(&bytes)?;
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        save.validate_compatibility(&image)?;
        if let Some(process)=&self.process_runtime {process.source_load_unjoined().map_err(|e|e.to_string())?;}
        let [id, _, _, difficulty] = save.level_metadata()?;
        let scripts = self.maps.parent().ok_or("script directory missing")?;
        let catalog = crate::campaign_store::load_catalog(&image, scripts)?;
        let level = catalog.levels.get(catalog.find(id).ok_or("saved level is absent from original catalog")?,
            )
            .ok_or("saved catalog level is missing")?;
        let scenario = std::str::from_utf8(level.directory())
            .map_err(|_| "saved scenario directory is not UTF-8")?;
        // Catalog names retain Windows casing; use the supplied directory's
        // spelling for the session's hub, player and AI reads on Linux.
        let scenario_path = crate::campaign_store::original_path(scripts, scenario)?
            .ok_or("saved scenario script directory is missing")?;
        let scenario = scenario_path.strip_prefix(scripts)
            .map_err(|_| "saved scenario directory is outside scripts")?
            .to_str().ok_or("saved scenario directory is not UTF-8")?;
        let difficulty = i32::try_from(difficulty).map_err(|_| "saved difficulty exceeds signed range")?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let index = openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
            .map_err(|e| e.to_string())?;
        let parameters = fs::read_to_string(self.installer.join("parameters/parameter.prm"))
            .map_err(|e| e.to_string())?;
        let timetable = fs::read_to_string(self.installer.join("parameters/timetable.prm"))
            .map_err(|e| e.to_string())?;
        // An arbitrary original save has no authenticated V2 profile selection.
        let mut restored_presentation=Vec::new();
        let mut session = crate::fresh_city::Session::load_original_joined(
            crate::fresh_city::Inputs {
                tree_locators: Some(openmt2001::trees::Assets::load(&image, &index, &self.archive)?.locators,
                    ),
                image: &image, board: &board, file: &file,
                people: openmt2001::people::PeopleModels::load(&image, &index, &self.archive)?,
                vehicle_models: openmt2001::traffic::road::models::Models::from_original(
                    &image, &index, &self.archive,
                    )?,
                transport_models: openmt2001::traffic::models::Models::from_original(
                    &image, &index, &self.archive,
                    )?,
                road_routes: fs::read(self.installer.join("Standard/route_smalltable.bin"))
                    .map_err(|e| format!("road route table: {e}"))?,
                scripts, parameters: &parameters,
                era_styles: crate::fresh_city::load_era_styles(&self.installer.join("parameters"),
                    )?,
                timetable: &timetable, scenario, difficulty,
                selected_character: 0, profile_name: b"Loaded original game",
                character_names: &self.character_names,
            },
            crate::fresh_city::Policy { seed: 0, auxiliary_prior_bytes: [0; 0x11c],
                },
            &bytes,None,None,Some(&mut restored_presentation),Some(&self.style),
            self.process_runtime.clone(),dispatch,
        )?;
        // Native restore success4bcd74/7b precedes the explicit modern extension.
        // Input/wrapper transition is published only by the later real focus
        // prefix; no reset or synthesized acquisition occurs on this worker.
        crate::process_dispatch::native_load_completed(&mut session)?;
        let metadata = session.restore_v2_extension_with_image(&save, &image)?;
        #[cfg(test)]
        crate::session_save::natural_observer::after_restore(&session);
        session.repair_imported_resident_models(self.compatibility, metadata.is_some())?;
        let saved_auction = metadata.as_ref().and_then(|metadata| metadata.auction.clone());
        let saved_interface_mode = metadata.as_ref().and_then(|metadata| metadata.interface_mode);
        let is_custom = session
            .catalog
            .selected_level()
            .is_some_and(crate::fresh_city::custom::valid_level);
        let filename = metadata.and_then(|metadata| metadata.profile_filename);
        let progress = if is_custom {
            // Custom progress belongs to the actively selected named profile.
            // An imported save's filename cannot choose another profile's writer.
            match selected_root {
                Some(Some(root)) => {
                    crate::save_menu::progress_for_profile_root(&self.profile_root, root)
                }
                Some(None) => None,
                None => filename
                    .as_ref()
                    .map(|name| self.profile_root.join("campaign").join(name)),
            }
        } else {
            filename.and_then(|filename| {
                let profile = self.profile_root.join("campaign").join(filename);
                let owner =
                    crate::save_menu::profile_root_for_campaign(&self.profile_root, &profile);
                selected_root
                    .is_none_or(|selected| selected == owner.as_deref())
                    .then_some(profile)
            })
        };
        if let Some(profile) = progress {
                crate::campaign_store::load_progress(&mut session.catalog, &profile)?;
                if !session.catalog.select(id, false) {
                    return Err("Saved level is absent after profile reload".into());
                }
                session.campaign_profile = Some(profile);
        }
        let saved_viewpoint = session.saved_cameras.as_ref()
            .map(normal_saved_city_viewpoint)
            .transpose()?;
        let saved_prior_mode = saved_interface_mode
            .or_else(|| saved_auction.as_ref().map(|auction| auction.prior_mode)).or_else(|| {
                (session.saved_game_mode == Some(2)
            && session.world.city.auction.controller.reference != 0)
            .then(|| {
                        u32::from_le_bytes(session.world.city.auction.controller.record[0x150..0x154]
                .try_into().unwrap(),
                        )
                    })
            });
        let camera = saved_viewpoint.map_or(camera, |(sample, _)| sample);
        let mut loaded = self.inspect_fresh(&mut session, camera, cache)?;
        loaded.saved_viewpoint = saved_viewpoint;
        loaded.saved_prior_mode = saved_prior_mode;
        loaded.saved_auction = saved_auction;
        loaded.restored_presentation = restored_presentation;
        Ok((loaded, session))
    }

    fn ensure_appearance_assets(&self, cache: &mut SnapshotCache) -> Result<(), String> {
        if cache.appearance_assets.is_none() {
            let tab = fs::read(self.installer.join("Standard/fonts/coppgoth23p.tab"))
                .map_err(|e| format!("saved appearance font4: {e}"))?;
            let offsets = fs::read(self.language.join("tycoon.off"))
                .map_err(|e| format!("saved appearance language offsets: {e}"))?;
            let glyphs = fs::read(self.language.join("tycoon.bin"))
                .map_err(|e| format!("saved appearance language glyphs: {e}"))?;
            cache.appearance_assets = Some(crate::save_appearance::Assets::from_original(
                &self.directory, &self.archive)?.with_language(&tab, &offsets, &glyphs)?,
            );
        }
        Ok(())
    }

    fn append_static_appearance(
        &self,
        prepared: &std::collections::BTreeMap<usize,
            (openmt2001::scene::SceneBuilding,
             openmt2001::save::appearance::ModelIdentity, u32,
            ),
        >,
        scene: &mut Scene,
        detail_visibility: &mut DetailVisibility,
        cache: &mut SnapshotCache,
        context: StaticAppearanceContext<'_, '_>,
    ) -> Result<(), String> {
        let started = Instant::now();
        let instances = prepared.values().flat_map(|(building, identity, _)| {
                identity.renderer_templates.iter().filter(|row|
                crate::static_detail::supports(row.descriptor)).map(move |row| (building, row))
            }).collect::<Vec<_>>();
        if instances.is_empty() {
            return Ok(());
        }
        if cache.static_details.is_none() {
            let index = openmt2001::archive::ArchiveIndex::parse(
                &self.directory, self.archive.len() as u64,
            ).map_err(|error| error.to_string())?;
            cache.static_details = Some(crate::static_detail::Resources::load(
                context.image, &index, &self.archive,
            )?);
        }
        let load_elapsed = started.elapsed();
        let (draws, spans) = cache.static_details.as_ref().unwrap().draws_with_spans(
            &instances, &self.suncols, context.hour,
        )?;
        if std::env::var_os("OPENMT2001_PROFILE_SNAPSHOTS").is_some() {
            eprintln!("Live static details: instances={} load={load_elapsed:?} draws={:?} total={:?}",
                instances.len(), started.elapsed() - load_elapsed, started.elapsed());
        }
        scene.vertex_count += draws.iter().map(|draw| draw.vertices.len()).sum::<usize>();
        let first = scene.draws.len();
        scene.draws.extend(draws);
        for span in spans {
            let center = context.board.blocks.iter().find(|block| block.id == span.category)
                .ok_or("static detail category missing from board")?.center;
            detail_visibility.add(scene,span.category,center,first+span.draw,
                span.vertices,span.pivot_y,
            )?;
        }
        #[cfg(test)]
        cache.static_detail_ranges.push(first..scene.draws.len());
        scene.diagnostics.push(format!(
            "Static appearance: {} original TM detail instances", instances.len(),
        ));
        Ok(())
    }

    fn appearance_payload(
        session: &crate::fresh_city::Session<'_>,
        record: &[u8; openmt2001::building_pool::RECORD_SIZE],
        assets: &crate::save_appearance::Assets,
    ) -> Result<(u32, crate::save_appearance::RefreshLabels), String> {
        //485520 reads metadata character/name, independently of roster slot.
        let owner=usize::from(record[0x2d]);
        if owner>6 {return Err("appearance owner leaves roster".into());}
        let (class,name)=if owner==0 {(10,None)} else {
            let metadata=&session.saved_player_metadata[owner-1];
            let name=&metadata[8..];
            let len=name.iter().position(|&byte|byte==0)
                .ok_or("appearance owner metadata name unterminated")?;
            (u32::from_le_bytes(metadata[..4].try_into().unwrap()),Some(&name[..len]),
            )
        };
        let business=u32::from_le_bytes(record[0x1c..0x20].try_into().unwrap());
        let kind=if business==u32::MAX {-1} else {
            let row=session.world.ledgers.businesses.get(business as usize)
                .ok_or("appearance building business index outside pool")?;
            i32::from_le_bytes(row[4..8].try_into().unwrap())
        };
        let name_id=if kind==-1 {0x35e} else {
            let info=session.world.catalog.info.rows().get(usize::try_from(kind)
                .map_err(|_|"appearance business subtype is negative")?)
                .ok_or("appearance business subtype outside catalog")?;
            u32::from_le_bytes(info[4..8].try_into().unwrap())
        };
        let mut payload = assets.refresh_labels(name,name_id)?;
        payload.banner = Some(crate::save_appearance::BannerSelection::refreshed(class,kind)?);
        Ok((class,payload))
    }

    fn append_saved_appearance(
        &self,
        session: &crate::fresh_city::Session<'_>,
        scene: &mut Scene,
        detail_visibility: &mut DetailVisibility,
        cache: &mut SnapshotCache,
    ) -> Result<(), String> {
        use openmt2001::{save::{SavedBuilding, appearance, restore}, scene::SceneBuilding,
        };
        self.ensure_appearance_assets(cache)?;
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let mut prepared = std::collections::BTreeMap::new();
        let mut labels = std::collections::BTreeMap::new();
        for inspection in &scene.inspection_buildings {
            let Some(slot) = inspection.runtime_slot else { continue; };
            let Some(identity) = session.models.appearance_candidates.get(&slot) else { continue; };
            let record = &session.world.buildings[slot];
            if record[0x3f] == 0 || !detail_model_at_unit_scale(record) { continue; }
            let source = SavedBuilding { slot, category: inspection.category,
                bytes: record[..0x40].try_into().unwrap(),
            };
            let building = SceneBuilding::from_saved(&source, &board, &file)?;
            let current = match appearance::model_identity(&file, building.selected, source.height()) {
                Ok(identity) => identity,
                Err(error) if error.contains("has no native 4846f0 class") => continue,
                Err(error) => return Err(error),
            };
            if current != *identity {
                // A later in-game remodel needs a new 485200 binding. The
                // initial saved registry must not be drawn at its stale pose.
                continue;
            }
            let (class,text)=Self::appearance_payload(session,record,
                cache.appearance_assets.as_ref().unwrap(),
            )?;
            labels.insert(slot,text);
            prepared.insert(slot, (building, identity.clone(), class));
        }
        let mut categories = session.world.categories;
        let mut buildings = session.world.buildings.clone();
        let mut callbacks = Vec::new();
        restore::post_load_appearance_walk(
            &mut categories, &mut buildings, 0x591318,
            |slot, _, _, _| {
                if prepared.contains_key(&slot) { callbacks.push(slot); }
                Ok(())
            },
        )?;
        let (details, replacements, spans) = crate::save_appearance::restored_draws_with_spans(
            &prepared, &callbacks, &image, cache.appearance_assets.as_ref().unwrap(),
            &labels,
        )?;
        scene.vertex_count += details.iter().map(|draw| draw.vertices.len()).sum::<usize>();
        let first = scene.draws.len();
        scene.draws.extend(details);
        for (slot, range, pivot_y) in spans {
            let building = &prepared.get(&slot).ok_or("appearance span slot missing")?.0;
            let center = board.blocks.iter().find(|block| block.id == building.block_id)
                .ok_or("appearance category missing from board")?.center;
            for draw in range {
                let index = first+draw;
                detail_visibility.add(scene,building.block_id,center,index,
                    0..scene.draws[index].vertices.len(),pivot_y,
                )?;
            }
        }
        self.append_static_appearance(&prepared,scene,detail_visibility,cache,
            StaticAppearanceContext { board:&board,image:&image,
                hour:session.world.clock.hour(),
            },
        )?;
        scene.diagnostics.push(format!(
            "Saved appearance: {} prepared buildings, {replacements} native-order detail replacements",
            prepared.len(),
        ));
        Ok(())
    }

    /// Fresh map and newly built models also pass through native 485200 when
    /// the building binds. Draw the already checked constructor classes from
    /// each current model identity; keep unknown classes visible in diagnostics
    /// until their original renderer path is recovered. Saved models that still
    /// match their load-time identity already used the ordered 485520 walk.
    fn append_current_appearance(
        &self,
        session: &crate::fresh_city::Session<'_>,
        scene: &mut Scene,
        detail_visibility: &mut DetailVisibility,
        cache: &mut SnapshotCache,
        after_saved_refresh: bool,
    ) -> Result<(), String> {
        use openmt2001::{save::{SavedBuilding, appearance}, scene::SceneBuilding,
        };
        if !scene.inspection_buildings.iter().any(|inspection| {
            inspection.runtime_slot.is_some_and(|slot| {
                session.world.buildings[slot][0x3f] != 0
                && session.models.bindings.contains_key(&slot)
            })
        }) {
            return Ok(());
        }
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let board = self.styled_board(&image)?;
        let file = openmt2001::build_file::BuildFile::parse(&self.build_file)?;
        let mut prepared = std::collections::BTreeMap::new();
        let mut callbacks=Vec::new();
        let mut labels=std::collections::BTreeMap::new();
        let mut unsupported = std::collections::BTreeMap::<String, usize>::new();
        for inspection in &scene.inspection_buildings {
            let Some(slot) = inspection.runtime_slot else { continue; };
            let record = &session.world.buildings[slot];
            if record[0x3f] == 0 || !session.models.bindings.contains_key(&slot) {
                continue;
            }
            // The detail pose is for the completed model. Native 444ff0
            // gives status-4 buildings a unit model matrix even when their
            // auxiliary scale bytes are zero. For pending-world growth wait
            // until the auxiliary scale reaches one, so signs do not float
            // over a small construction model.
            if !detail_model_at_unit_scale(record) {
                continue;
            }
            let source = SavedBuilding { slot, category: inspection.category,
                bytes: record[..0x40].try_into().unwrap(),
            };
            let building = SceneBuilding::from_saved(&source, &board, &file)?;
            let identity = match appearance::model_identity(&file, building.selected, source.height()) {
                Ok(identity) => identity,
                Err(error) if error.contains("has no native 4846f0 class") => {
                    *unsupported.entry(error).or_default() += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            if after_saved_refresh
                && session.models.appearance_candidates.get(&slot) == Some(&identity)
            {
                continue;
            }
            let class=if openmt2001::building_geometry::refresh_appearance_on_bind(record) {
                self.ensure_appearance_assets(cache)?;
                let (class,text)=Self::appearance_payload(session,record,
                    cache.appearance_assets.as_ref().unwrap(),
                )?;
                callbacks.push(slot);labels.insert(slot,text);class
            }else{10};
            prepared.insert(slot, (building, identity, class));
        }
        let (details, spans) = if prepared.is_empty() {
            (Vec::new(),Vec::new())
        } else {
            self.ensure_appearance_assets(cache)?;
            let (details,_,spans) = crate::save_appearance::restored_draws_with_spans(
                &prepared, &callbacks, &image, cache.appearance_assets.as_ref().unwrap(),
                &labels,
            )?;
            (details,spans)
        };
        scene.vertex_count += details.iter().map(|draw| draw.vertices.len()).sum::<usize>();
        let first = scene.draws.len();
        scene.draws.extend(details);
        for (slot, range, pivot_y) in spans {
            let building = &prepared.get(&slot).ok_or("appearance span slot missing")?.0;
            let center = board.blocks.iter().find(|block| block.id == building.block_id)
                .ok_or("appearance category missing from board")?.center;
            for draw in range {
                let index = first+draw;
                detail_visibility.add(scene,building.block_id,center,index,
                    0..scene.draws[index].vertices.len(),pivot_y,
                )?;
            }
        }
        self.append_static_appearance(&prepared,scene,detail_visibility,cache,
            StaticAppearanceContext { board:&board,image:&image,
                hour:session.world.clock.hour(),
            },
        )?;
        scene.diagnostics.push(format!(
            "Current appearance: {} checked models, {} unsupported model classes",
            prepared.len(), unsupported.values().sum::<usize>(),
        ));
        for (class, count) in unsupported {
            scene.diagnostics.push(format!("{count} current models: {class}"));
        }
        Ok(())
    }

    fn inspect_fresh(
        &self,
        session: &mut crate::fresh_city::Session<'_>,
        camera: CameraSample,
        cache: &mut SnapshotCache,
    ) -> Result<LoadedInspection, String> {
        let started = Instant::now();
        let image = openmt2001::pe::PeImage::parse(&self.executable)?;
        let index =
            openmt2001::archive::ArchiveIndex::parse(&self.directory, self.archive.len() as u64)
                .map_err(|e| e.to_string())?;
        let input_elapsed = started.elapsed();
        let hour = session.world.clock.hour();
        let profile_snapshot = std::env::var_os("OPENMT2001_PROFILE_SNAPSHOTS").is_some();
        let cache_observation = profile_snapshot.then(|| {
            (
            cache.hour,
            cache.city.is_some(),
            cache.buildings.as_slice() == session.world.buildings.as_slice(),
            cache.scaffolds_active,
        )
        });
        let mut reuse_city = false;
        let mut updated_city_draws = Vec::new();
        let mut assembly_elapsed = std::time::Duration::ZERO;
        let mut appearance_elapsed = std::time::Duration::ZERO;
        let mut retained = None;
        if cache.hour == hour && let Some(mut city) = cache.city.take() {
            if cache.buildings.as_slice() == session.world.buildings.as_slice() {
                reuse_city = true;
            } else if let Some(draws) = city.patch_runtime_poses(
                &cache.buildings, &session.world.buildings)? {
                reuse_city = true;
                updated_city_draws = draws;
                cache.buildings = session.world.buildings.to_vec();
            }
            if reuse_city {
                // Move the base out while patching: a pose/metadata error can
                // leave partial writes, and must discard that base. Successful
                // snapshots clone only once for their actor layers.
                session.refresh_scene_records(&mut city)?;
                retained = Some(city.clone());
                cache.city = Some(city);
            }
        }
        let mut detail_visibility = if retained.is_some() {
            cache.detail_visibility.clone()
        } else {
            DetailVisibility::default()
        };
        let mut scene = if let Some(scene) = retained {
            scene
        } else {
            #[cfg(test)]
            cache.static_detail_ranges.clear();
            let assembly_started = Instant::now();
            let mut scene = session.scene_with_style_cached(
                &self.build_file,
                OriginalAssets {
                    executable: &self.executable,
                    directory: &self.directory,
                    archive: &self.archive,
                },
                &self.suncols,
                Some(camera),
                true,
                Some(&self.style),
                Some(&mut cache.converted_textures),
            )?;
            assembly_elapsed = assembly_started.elapsed();
            let appearance_started = Instant::now();
            if session.saved_game_mode.is_some() {
                self.append_saved_appearance(session,&mut scene,&mut detail_visibility,cache)?;
                self.append_current_appearance(session,&mut scene,&mut detail_visibility,cache,true,
                )?;
            } else {
                self.append_current_appearance(session,&mut scene,&mut detail_visibility,cache,false,
                )?;
            }
            appearance_elapsed = appearance_started.elapsed();
            cache.hour = hour;
            cache.buildings = session.world.buildings.to_vec();
            cache.detail_visibility = detail_visibility.clone();
            cache.city = Some(scene.clone());
            scene
        };
        let scaffold_started = Instant::now();
        let has_scaffolds = construction_scaffold::has_active(&session.world.pending_world);
        if has_scaffolds != cache.scaffolds_active {
            // Adding or clearing scaffold geometry changes the retained GPU
            // page layout independently of the cached building geometry.
            reuse_city = false;
        }
        cache.scaffolds_active = has_scaffolds;
        if has_scaffolds && cache.scaffolds.is_none() {
            cache.scaffolds = Some(construction_scaffold::Resources::new(
                &image, &index, &self.archive,
            )?);
        }
        if let Some(scaffolds) = &cache.scaffolds {
            let range = scaffolds.append(
                &mut scene,
                &session.world.pending_world,
                &session.world.buildings,
                &self.suncols,
                hour,
            )?;
            if has_scaffolds {
                updated_city_draws.extend(range);
            }
        }
        let scaffold_elapsed = scaffold_started.elapsed();
        let city_elapsed = started.elapsed();
        if cache.trees.is_none() {
            cache.trees = Some(std::sync::Arc::new(crate::tree_scene::Resources::load(
                &image, &index, &self.archive, &self.suncols,
            )?));
        }
        let resources = cache.trees.as_ref().unwrap().clone();
        let mut trees = if let Some(state) = &session.models.tree_state {
            let pool = state.borrow();
            if !cache.tree_instances.as_ref().is_some_and(|(revision, instances)| {
                    *revision == pool.revision() && instances.matches(pool.pool())
                }) {
                cache.tree_instances = Some((pool.revision(), resources.instances(pool.pool().clone())?));
            }
            crate::tree_scene::Layer::with_instances(resources,
                cache.tree_instances.as_ref().unwrap().1.clone(), self.compatibility,
            )
        } else {
            crate::tree_scene::Layer::new(resources, self.compatibility)
        };
        trees.attach(&mut scene);
        if cache.people.is_none() {
            cache.people = Some(crate::people_scene::Resources::new(
                session,
                &image,
                &index,
                &self.archive,
            )?);
        }
        let mut people = crate::people_scene::Layer::from_resources(
            session,
            &self.suncols,
            cache.people.as_ref().unwrap(),
        )?;
        people.attach(&mut scene);
        if cache.vehicles.is_none() {
            cache.vehicles = Some(crate::vehicle_scene::Resources::new(
                session,
                &image,
                &index,
                &self.archive,
            )?);
        }
        session.road_traffic.project(&mut session.world.road_network, 0.)?;
        let mut vehicles = crate::vehicle_scene::Layer::from_resources(
            session,
            &self.suncols,
            cache.vehicles.as_ref().unwrap(),
        )?;
        vehicles.attach(&mut scene);
        if cache.transport.is_none() {
            cache.transport = Some(crate::transport_scene::Resources::new(
                &session.transport_models,
                &image,
                &index,
                &self.archive,
            )?);
        }
        let mut transport_auxiliaries = Vec::new();
        session.advance_transport_frame(|auxiliary| {
            transport_auxiliaries.push(*auxiliary);
            Ok(())
        })?;
        let mut transport = crate::transport_scene::Layer::from_resources(
            session,
            &self.suncols,
            cache.transport.as_ref().unwrap(),
            transport_auxiliaries,
        )?;
        transport.attach(&mut scene);
        let render_elapsed = started.elapsed();
        let city_people = Some(crate::city_people::Report::from_world(
            &session.world, &image, &session.roster.manager,
        )?);
        let player_info = Some(crate::player_info::State::from_runtime(
            &session.roster.manager,
            &session.world.ledgers.players,
            session.roster.status.current_player,
        )?);
        let comparison = Some(crate::comparison::Data::from_runtime(
            &session.world,
            &player_info.as_ref().unwrap().records,
            &image,
            0x591318,
        )?);
        let mut block_information = crate::block_information::Report::from_runtime(
            &session.world,
            player_info.as_ref().unwrap(),
            &image,
        )?;
        if let Some(report) = &mut block_information {
            report.attach_railroads(
                &session.population.properties,
                session
                    .world
                    .settings
                    .daily_freight(session.difficulty)
                    .ok_or("railroad freight difficulty")?,
            )?;
        }
        let board = self.styled_board(&image)?;
        let runtime_locator = (session.saved_game_mode.is_none())
            .then(|| crate::locator::RuntimeInputs::from_world(&session.world));
        let consumer_polls = Some(crate::consumer_polls::Poll::from_runtime(&session.world)?);
        let city_ownership = Some(crate::city_ownership::Ownership::from_runtime(
            &session.world, player_info.as_ref().unwrap(),
        )?);
        let city_rights = crate::city_rights::Rights::from_runtime(
            &session.world, player_info.as_ref().unwrap(),
        )?;
        let city_prestige = Some(crate::city_prestige::Prestige::from_runtime(
            &session.world, &board,
        )?);
        if let Some((previous_hour, had_city, buildings_equal, previous_scaffolds)) = cache_observation {
            eprintln!(
                "Live snapshot: inputs {input_elapsed:?}, assembly {assembly_elapsed:?}, appearance {appearance_elapsed:?}, scaffold {scaffold_elapsed:?}, city cumulative {:?}, actors/resources host elapsed {:?}, reports host elapsed {:?}, total host elapsed {:?}",
                city_elapsed,
                render_elapsed.saturating_sub(city_elapsed),
                started.elapsed().saturating_sub(render_elapsed),
                started.elapsed()
            );
            eprintln!(
                "Snapshot cache tick{} previous_hour{previous_hour} hour{hour} had_city{had_city} buildings_equal{buildings_equal} previous_scaffolds{previous_scaffolds} scaffolds{has_scaffolds} reuse_city{reuse_city} draws{}; host elapsed durations include waits; not thread CPU time or GPU execution",
                session.world.clock.timing[0], scene.draws.len(),
            );
        }
        Ok(LoadedInspection {
            camera_epoch: None,
            camera_publisher: None,
            camera_inbox: None,
            save_profile_root: None,
            logic_tick: Some(session.world.clock.timing[0]),
            construction_active: session.world.pending_world.records
                .iter().any(|row| matches!(row[2], 1 | 2)),
            music_era: Some(session.world.clock.calendar[2]),
            environment_time: Some((session.world.clock.hour(), session.world.clock.timing[2])),
            imported_cameras: session.saved_cameras.map(openmt2001::camera::bank::Capture::from_records),
            saved_viewpoint: None,
            saved_prior_mode: None,
            saved_auction: None,
            restored_presentation: Vec::new(),
            reuse_city,
            updated_city_draws,
            auction_requests: None,
            auction_audio_requests: None,
            transport_audio_requests: None,
            era_audio_requests: None,
            gameplay_sound_requests: None,
            player_info,
            comparison,
            city_people,
            runtime_locator,
            consumer_polls,
            city_ownership,
            city_rights,
            navigator_owners: crate::navigator::runtime_owners(&session.world.categories,
                &session.roster.manager, session.roster.status.current_player, &image,
            )?,
            block_information,
            city_prestige,
            scene,
            detail_visibility,
            status: None,
            fresh_session: None,
            trees: Some(trees),
            people: Some(people),
            vehicles: Some(vehicles),
            transport: Some(transport),
        })
    }
    fn preview_scene(
        &self,
        setup: crate::singleplayer::Setup,
        camera: CameraSample,
    ) -> Result<LoadedInspection, String> {
        eprintln!(
            "Preview setup: {:?}, character {}, profile {}",
            setup.difficulty,
            setup.character_index,
            String::from_utf8_lossy(&setup.profile_name)
        );
        let map = match setup.preview {
            crate::singleplayer::Preview::Scenario(id) => {
                let scenario = crate::scenarios::find(id)?;
                let map = scenario
                    .map
                    .ok_or("Preview unavailable: scenario initialization required")?;
                let path = self.maps.join(map);
                eprintln!(
                    "Static preview source: level {id}, {}, scripts/maps/{map}; hub not executed",
                    scenario.token
                );
                fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?
            }
            crate::singleplayer::Preview::Sandbox => String::new(),
        };
        // Diagnostic phase12 only: this is not a recovered initialization clock.
        render_scene::assemble_with_style(Request {
            build_file: &self.build_file,
            source: Source::Map(&map),
            original: Some(OriginalAssets {
                executable: &self.executable,
                directory: &self.directory,
                archive: &self.archive,
            }),
            suncols: Some(&self.suncols),
            phase: Some(12),
            camera: Some(camera),
            width: 1280,
            height: 960,
            animated: true,
        }, Some(&self.style),
        )
        .map(|scene| LoadedInspection {
            camera_epoch: None,
            camera_publisher: None,
            camera_inbox: None,
            save_profile_root: None,
            logic_tick: None,
            construction_active: false,
            music_era: None,
            environment_time: None,
            imported_cameras: None,
            saved_viewpoint: None,
            saved_prior_mode: None,
            saved_auction: None,
            restored_presentation: Vec::new(),
            reuse_city: false,
            updated_city_draws: Vec::new(),
            auction_requests: None,
            auction_audio_requests: None,
            transport_audio_requests: None,
            era_audio_requests: None,
            gameplay_sound_requests: None,
            comparison: None,
            city_people: None,
            runtime_locator: None,
            consumer_polls: None,
            city_ownership: None,
            city_rights: None,
            navigator_owners: Vec::new(),
            player_info: None,
            block_information: None,
            city_prestige: None,
            fresh_session: None,
            trees: None,
            people: None,
            vehicles: None,
            transport: None,
            scene,
            detail_visibility: DetailVisibility::default(),
            status: None,
        })
    }

}
struct Inspection {
    music_admission:Option<focus::MusicAdmission>,
    camera_epoch: Option<openmt2001::level_startup::SourceEpoch>,
    camera_turns: std::collections::VecDeque<camera_producer::Pending>,
    camera_producer: Option<camera_producer::Producer>,
    camera_ready: Option<mpsc::Receiver<Result<(), String>>>,
    ui_graph:Option<camera_live::ui_graph::UiGraphState>,
    camera_navigation:Option<camera_live::navigation::State>,
    camera_focus:camera_live::focus::Focus,
    navigation_click_latched:bool,
    camera_endpoint: Option<camera_live::endpoint::Endpoint>,
    camera_capture_pending: Option<(crate::save_menu::Target, mpsc::Receiver<Result<openmt2001::camera::bank::Capture,String>>)>,
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
    last_environment_query: Instant,
    last_status_query: Instant,
    last_snapshot: Instant,
    construction_active: bool,
    snapshot_tick: u32,
    card_input: crate::card_panel::Input,
    card_selection: Option<(i32, i32)>,
    prompts: crate::prompt::State,
    tutorial_message: crate::prompt::State,
    event_list: crate::event_list::View,
    stats_panel: crate::stats_panel::View,
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
    toolbar_bounds: [[f32;4];3],
    toolbar_regions: [Option<[f32;4]>;3],
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
enum ResultPhase { Flyby, CutScene,
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
    credit_pending: Option<mpsc::Receiver<Result<crate::fresh_city::result::ResultPresentation, String>>>,
    cutscene_pending: Option<mpsc::Receiver<Result<crate::fresh_city::result::ResultPresentation, String>>>,
    completed: Option<mpsc::Receiver<Result<openmt2001::outcome_notice::Sequence, String>>>,
    actor_light: openmt2001::lighting::SceneLight,
    actors: PreparedScene,
    actor_draws: usize,
    title: Option<(Option<u32>, Option<usize>, u8, PreparedScene)>,
}
impl Inspection {
    fn apply_status_camera_events(&mut self, status: &crate::live_host::Status,
        selection:&crate::selection::Assets, navigation:&crate::navigation::Navigation)->Result<(),String>{
        if status.tutorial_camera_events.len() < self.tutorial_camera_seen {
            return Err("tutorial camera event log regressed".into());
        }
        if self.camera_endpoint.is_some() && status.tutorial_camera_events.len()!=self.tutorial_camera_seen {
            return Err("Bridged camera received unordered Status camera effects".into());
        }
        for &action in &status.tutorial_camera_events[self.tutorial_camera_seen..] {
            self.apply_tutorial_camera(action,
                selection, navigation,
            )?;
        }
        self.tutorial_camera_seen = status.tutorial_camera_events.len();
        Ok(())
    }
    fn service_camera_turns(&mut self, selection: &crate::selection::Assets,
        adjustment_assets: Option<&crate::business_adjustment::Assets>) {
        let view=self;
        loop {
            let Some(front)=view.camera_turns.front() else{break};
            let result=match front.reply.try_recv(){
                Ok(value)=>value,
                Err(mpsc::TryRecvError::Empty)=>break,
                Err(mpsc::TryRecvError::Disconnected)=>Err("Camera selection Session closed".into()),
            };
            let mut pending=view.camera_turns.pop_front().expect("front pending turn");
            let payload=match result{Ok(payload)=>payload,Err(error)=>{view.save_dialog.message=Some(error);continue;}};
            let prepared=(||{
                if view.camera_epoch!=Some(payload.turn.epoch) {return Err("Retired camera selection origin".to_owned());}
                if let camera_producer::Event::Actor{selected,auto_rotate,origin}=pending.event {
                    let identity=pending.turn.building.ok_or("Actor turn omitted identity")?;
                    let index=view.scene.inspection_buildings.iter().position(|b|
                        b.runtime_slot.or(b.saved_slot)==Some(identity.building_slot as usize)
                        && b.category as u32==identity.category_id).ok_or("Live selected building is not in current presentation")?;
                    let category=selection.blocks.iter().position(|b|b.id as u32==identity.category_id).ok_or("Selected category no longer presented")?;
                    let _=selected; // Scene vector position is presentation only.
                    pending.event=camera_producer::Event::Actor{selected:crate::selection::Hit{category,building:Some(index)},auto_rotate,origin};
                    if pending.open_adjustment {
                        if let Some(assets)=adjustment_assets {
                            pending.adjustment=assets.open_for_owner(&view.scene,index,
                                view.live_status.as_ref().and_then(|status|status.business_owner))?;
                        }
                    }
                }
                Ok::<(),String>(())
            })();
            if let Err(error)=prepared {payload.receipt.complete(Err(error.clone()));view.save_dialog.message=Some(error);continue;}
            let Some(mut producer)=view.camera_producer.take() else{payload.receipt.complete(Err("Camera producer retired".into()));continue};
            let mut state=camera_producer::View::read(view);
            let result=camera_producer::complete_with_posts(&mut producer,&mut state,pending.turn,pending.event,payload,
                selection,std::mem::take(&mut pending.posts),|state|{
                    state.put(view);
                    if let Some(adjustment)=pending.adjustment.take(){view.adjustment=Some(adjustment);}
                    view.hovered=None;view.title_state=None;view.reframe_pending=true;
                    let (reset_keys,reset_mouse)=match pending.event {
                        camera_producer::Event::Actor{origin:camera_producer::ActorOrigin::Tutorial,..}=>(true,true),
                        camera_producer::Event::Actor{origin:camera_producer::ActorOrigin::Scene,..}=>(pending.open_adjustment,pending.open_adjustment),
                        camera_producer::Event::Actor{origin:camera_producer::ActorOrigin::Locator,..}=>(true,false),
                        camera_producer::Event::Actor{..}|camera_producer::Event::Deselect=>(false,false),
                        camera_producer::Event::ChooseView{..}=>(true,true),
                        _=>(true,false),
                    };
                    if reset_keys{view.keys=[false;4];view.taps=[false;4];view.drag=[0;2];}
                    if reset_mouse{view.mouse_buttons=[false;2];}
                    if let Some(locator)=&mut view.locator{
                        if matches!(pending.event,camera_producer::Event::Compass{..}){locator.cancel_press();}
                        locator.dirty=true;
                    }
                    if matches!(pending.event,camera_producer::Event::Actor{origin:camera_producer::ActorOrigin::Locator,..}){
                        if let Some(comparison)=&mut view.comparison{comparison.cancel_press();}
                    }
                });
            view.camera_producer=Some(producer);
            if let Err(error)=result {view.save_dialog.message=Some(error);}
        }
    }
    fn time_target(&self,assets:&crate::time_panel::Assets,point:[f32;2],auction:bool)
        ->Option<crate::time_panel::Button>{
        let target=assets.button_in_mode(point,auction)?;
        let Some(g)=&self.ui_graph else{return Some(target);};
        use camera_live::ui_graph::PanelId;
        match target {
            crate::time_panel::Button::Options=>g.input(PanelId::Clock).then_some(target),
            crate::time_panel::Button::Manager=>g.input(PanelId::E).then_some(target),
            crate::time_panel::Button::Speed=>(g.input(PanelId::E)&&g.speed_disabled==0).then_some(target),
        }
    }
    fn time_occupied(&self,assets:&crate::time_panel::Assets,point:[f32;2],auction:bool)->bool{
        let Some(g)=&self.ui_graph else{return assets.contains_in_mode(point,auction);};
        if !assets.contains_in_mode(point,auction){return false;}
        match assets.button_in_mode(point,auction){
            Some(crate::time_panel::Button::Speed|crate::time_panel::Button::Manager)=>g.input(camera_live::ui_graph::PanelId::E),
            _=>g.input(camera_live::ui_graph::PanelId::Clock),
        }
    }
    fn secondary_enabled(&self)->bool {
        self.ui_graph.as_ref().is_none_or(|g|g.group_enabled(camera_live::navigation::Group::Secondary))
    }
    fn options_visible(&self)->bool {self.secondary_enabled()&&self.game_options.open}
    fn save_visible(&self)->bool {self.secondary_enabled()&&self.save_dialog.visible}
    fn settings_visible(&self)->bool {self.secondary_enabled()&&self.settings_menu.page.is_some()}
    fn auction_visible(&self)->bool {
        self.auction.scene.is_some()&&self.ui_graph.as_ref().is_none_or(|g|g.group_enabled(camera_live::navigation::Group::Tertiary))
    }
    fn graph_input(&self,id:camera_live::ui_graph::PanelId)->bool {
        self.ui_graph.as_ref().is_some_and(|graph|graph.input(id))
    }
    fn graph_body(&self,id:camera_live::ui_graph::PanelId)->bool {
        self.ui_graph.as_ref().is_some_and(|graph|graph.body(id))
    }
    fn graph_group(&self,group:camera_live::navigation::Group)->bool {
        self.ui_graph.as_ref().is_some_and(|graph|graph.group_enabled(group))
    }
    fn normal_star_detail(&self) -> Option<u8> {
        (self.result.is_none() && self.result_pending.is_none()
            && !self.live_status.as_ref().is_some_and(|status| status.result_request.is_some()))
            .then_some(self.settings_menu.graphics.detail)
    }
    fn open_options(&mut self) -> Result<(), String> {
        self.game_options.open(self.live_status.as_ref().is_some_and(|s| s.running));
        // Before the first worker status, keep Save inert; authoritative
        // tutorial state refreshes the row as soon as Status arrives.
        self.game_options.set_save_disabled(true);
        self.options_ack.cancel();
        if let Some(sender) = &self._fresh_session {
            let (reply,result)=mpsc::channel();
            sender.send(FreshRequest::Options{open:true,reply}).map_err(|e|e.to_string())?;
            self.options_ack.begin(true,result);
        }
        // Status carries drained, exactly-once prompt events. Retain its
        // receiver across menu transitions, then request current state again.
        self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
        self.mouse_buttons = [false; 2];
        Ok(())
    }
    fn close_options(&mut self) -> Result<(), String> {
        self.game_options.close();
        self.options_ack.cancel();
        if let Some(sender) = &self._fresh_session {
            let (reply,result)=mpsc::channel();
            sender.send(FreshRequest::Options{open:false,reply}).map_err(|e|e.to_string())?;
            self.options_ack.begin(false,result);
        }
        self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        Ok(())
    }
    fn queue_camera_event(&mut self,event:camera_producer::Event,building:Option<camera_producer::ExpectedBuilding>,
        adjustment:Option<crate::business_adjustment::State>,open_adjustment:bool)->Result<&mut camera_producer::Pending,String>{
        let turn=camera_producer::Turn{epoch:self.camera_epoch.ok_or("Camera UI has no actual Session origin")?,building};
        let sender=self._fresh_session.as_ref().ok_or("Camera UI Session closed")?;
        camera_producer::enqueue(&mut self.camera_turns,sender,turn,event,adjustment,open_adjustment)?;
        Ok(self.camera_turns.back_mut().expect("enqueued camera turn"))
    }
    fn queue_actor_camera(&mut self,selected:crate::selection::Hit,assets:&crate::selection::Assets,
        origin:camera_producer::ActorOrigin,adjustment:Option<crate::business_adjustment::State>,open_adjustment:bool)->Result<&mut camera_producer::Pending,String>{
        let expected=camera_producer::identity_for_hit(&self.scene,assets,selected)?;
        self.queue_camera_event(camera_producer::Event::Actor{selected,auto_rotate:self.settings_menu.gameplay.auto_rotate,origin},Some(expected),adjustment,open_adjustment)
    }
    fn commit_camera(&mut self) {
        if let Some(producer) = &mut self.camera_producer {
            if let Err(error) = producer.commit(self.camera, if self.strategic {3} else {0}) {
                self.game_options.set_save_disabled(true);
                self.save_dialog.message = Some(format!("Save failed: {error}"));
            }
        }
    }
    fn toggle_navigation(&mut self, navigation: &crate::navigation::Navigation) {
        if self.camera_producer.is_some() {
            let post=self.camera_control_post(73);
            match self.queue_camera_event(camera_producer::Event::Compass { strategic:navigation.camera },None,None,false) {
                Ok(pending)=>pending.posts.extend(post),
                Err(error)=>self.save_dialog.message=Some(error),
            }
            return;
        }
        if let Some(locator) = &mut self.locator { locator.cancel_press(); }
        let current = camera_producer::Controls::read(self);
        if let Some(normal) = self.return_camera.take() {
            normal.put(self);
            self.strategic = false;
        } else {
            self.return_camera = Some(current);
            self.camera = navigation.camera;
            self.selected = None;
            self.strategic = true;
        }
        self.commit_camera();
        self.hovered = None;
        self.title_state = None;
        self.reframe_pending = true;
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
    }
    fn apply_tutorial_camera(
        &mut self,
        action: crate::live_host::TutorialCameraAction,
        selection: &crate::selection::Assets,
        navigation: &crate::navigation::Navigation,
    ) -> Result<(), String> {
        use crate::live_host::TutorialCameraAction;
        if self.camera_producer.is_some() {
            match action {
                TutorialCameraAction::Mode{mode:12,target}=>{
                    let category=selection.blocks.iter().position(|b|b.id==target as usize).ok_or("Tutorial category identity missing")?;
                    self.queue_camera_event(camera_producer::Event::Block{category:target,selected:crate::selection::Hit{category,building:None},origin:camera_producer::BlockOrigin::Tutorial},None,None,false)?;
                }
                TutorialCameraAction::Mode{mode:13,target}=>{
                    let index=self.scene.inspection_buildings.iter().position(|b|
                        b.business().is_some_and(|b|b.slot==target as usize)
                        || b.initialized_apartment.as_ref().is_some_and(|b|b.slot==target as usize)).ok_or("Tutorial live building identity not presented")?;
                    let b=&self.scene.inspection_buildings[index];
                    let hit=inspection_hit(&selection.blocks,b.category,Some(index)).ok_or("Tutorial category identity missing")?;
                    self.queue_actor_camera(hit,selection,camera_producer::ActorOrigin::Tutorial,None,false)?;
                }
                TutorialCameraAction::PlanView=>{self.queue_camera_event(camera_producer::Event::ChooseView{mode:3,strategic:navigation.camera},None,None,false)?;},
                TutorialCameraAction::CityView=>{self.queue_camera_event(camera_producer::Event::ChooseView{mode:0,strategic:navigation.camera},None,None,false)?;},
                TutorialCameraAction::Mode{mode,target}=>return Err(format!("Unsupported tutorial camera mode{mode} target{target}")),
            }
            return Ok(());
        }
        match action {
            TutorialCameraAction::Mode { mode: 12, target } => {
                let category = selection.blocks.iter().position(|block| block.id == target as usize)
                    .ok_or_else(|| format!("tutorial block {target} leaves original board"))?;
                self.focus_category(category, selection);
            }
            TutorialCameraAction::PlanView => {
                // SetPlanView calls choose_view(3), not the compass's4baf10
                // copy transaction. Its unjoined raw boundary cannot invent S.
                self.return_camera = Some(camera_producer::Controls::read(self));
                self.camera = navigation.camera;
                self.selected = None;
                self.strategic = true;
                self.reframe_pending = true;
            }
            TutorialCameraAction::CityView => {
                self.inertia = openmt2001::camera::BlockCameraInertia::new(
                    self.camera.eye, self.camera.target,
                );
                self.strategic = false;
                self.return_camera = None;
                self.reframe_pending = true;
            }
            TutorialCameraAction::Mode { mode: 13, target } => {
                // Native 4d9d80 interprets target as a business slot, then
                // 462f30 resolves its parent building before 4c3a20 frames it.
                let building_index = self.scene.inspection_buildings.iter().position(|building| {
                    building.initialized_business.as_ref().is_some_and(|business|
                        business.slot == target as usize)
                    || building.saved_business.as_ref().is_some_and(|business|
                        business.slot == target as usize)
                }).ok_or_else(|| {
                        format!("tutorial business {target} has no rendered building")
                    })?;
                let building = &self.scene.inspection_buildings[building_index];
                let hit = inspection_hit(&selection.blocks, building.category, Some(building_index))
                    .ok_or("tutorial building category leaves original board")?;
                let category = hit.category;
                let (controls, angle) = crate::navigation::building_entry(
                    selection.blocks[category].center,
                    [building.translation[0], building.translation[2]],
                    building.camera_entrance,
                    building.kind == 2,
                    self.controls.angle,
                    self.building_angle,
                ).ok_or("tutorial building camera lacks original entrance")?;
                self.selected = Some(hit);
                self.inertia = openmt2001::camera::BlockCameraInertia::new(
                    self.camera.eye, self.camera.target,
                );
                self.controls = controls;
                self.building_angle = angle;
                self.building_camera = true;
                self.strategic = false;
                self.return_camera = None;
                self.reframe_pending = true;
            }
            TutorialCameraAction::Mode { mode, target } => {
                return Err(format!("tutorial camera mode {mode} target {target} is unsupported"));
            }
        }
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
        self.mouse_buttons = [false; 2];
        self.commit_camera();
        Ok(())
    }
    fn camera_control_post(&self,id:u32)->Option<camera_producer::Post>{
        (self.live_status.as_ref().is_some_and(|status|status.failure.is_none()&&status.tutorial_enabled_controls.contains(&id))
            && self._fresh_session.is_some()).then_some(camera_producer::Post::TutorialControl(id))
    }
    fn send_tutorial_control(&mut self, id: u32) {
        if self.live_status.as_ref().is_some_and(|status| {
            status.failure.is_none() && status.tutorial_enabled_controls.contains(&id)
        })
            && let Some(sender) = &self._fresh_session {
            let _ = sender.send(FreshRequest::TutorialControl(id));
            self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        }
    }
    fn send_tutorial_producer(&mut self, control: u32, event: u32) {
        if self.live_status.as_ref().is_some_and(|s| s.tutorial_enabled_controls.contains(&control))
            && let Some(sender) = &self._fresh_session {
            let _ = sender.send(FreshRequest::TutorialProducer { control, event });
            self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        }
    }
    fn send_tutorial_pick(&mut self, pick: crate::tutorial_two_interaction::Pick) {
        if let Some(sender) = &self._fresh_session {
            let _ = sender.send(FreshRequest::TutorialPick(pick));
            self.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
        }
    }
    fn focus_category(&mut self, category: usize, selection: &crate::selection::Assets) {
        if self.camera_producer.is_some() {
            if let Err(error)=self.queue_camera_event(camera_producer::Event::Block {
                category:selection.blocks[category].id as u32,
                selected:crate::selection::Hit {category,building:None},
                origin:camera_producer::BlockOrigin::Selection,
            },None,None,false){self.save_dialog.message=Some(error);}
            return;
        }
        self.selected = Some(crate::selection::Hit {
            category,
            building: None,
        });
        self.inertia =
            openmt2001::camera::BlockCameraInertia::new(self.camera.eye, self.camera.target);
        self.controls = crate::navigation::block_entry(selection.center(category));
        self.building_camera = false;
        self.strategic = false;
        self.return_camera = None;
        self.reframe_pending = true;
        self.title_state = None;
        self.hovered = None;
        self.adjustment = None;
        self.keys = [false; 4];
        self.taps = [false; 4];
        self.drag = [0; 2];
        self.commit_camera();
    }
    fn reply_human_dialog(&mut self, response: crate::human_dialog::Reply) -> Result<(), String> {
        let sender = self
            ._fresh_session
            .as_ref()
            .ok_or("Human dialog has no live session")?;
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::HumanDialog(response, reply))
            .map_err(|e| e.to_string())?;
        self.prompt_reply = Some(answer);
        Ok(())
    }
    fn dismiss_prompt(&mut self, id: u32) -> Result<(), String> {
        let sender = self
            ._fresh_session
            .as_ref()
            .ok_or("prompt has no live session")?;
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::DismissPrompt(id, Some(reply)))
            .map_err(|e| e.to_string())?;
        self.prompt_reply = Some(answer);
        Ok(())
    }
    fn ordinary_toolbar_category(&self,blocks:&[openmt2001::board::Block])->Option<usize> {
        if self.strategic || self.operations.building.is_some()
            || !self.selected.is_some_and(|hit|hit.building.is_none()) { return None; }
        self.utility_category(blocks).filter(|&category|self.block_information.as_ref()
            .is_some_and(|report|report.available(category)&&report.categories[category].is_some()))
    }
    fn toolbar_policy(&self,menu:&crate::menu::Menu,blocks:&[openmt2001::board::Block],input:bool)
        ->Option<crate::locator::toolbar_collapse::Policy>
    {
        use crate::locator::toolbar_collapse::Action;
        let category=self.ordinary_toolbar_category(blocks)?;
        let mut shown=self.main_panel_visible()&&!self.game_options.open
            &&self.planning.is_none()&&self.adjustment.is_none();
        let mut allowed=Action::ALL.map(|a|self.live_status.as_ref().is_none_or(|s|
            !s.tutorial_session||s.tutorial_enabled_controls.contains(&a.tutorial())));
        if input {
            shown &= !self.toolbar_right_held&&self.settings_menu.page.is_none()
                &&!self.save_dialog.visible&&!self.stats_requested
                &&!self.live_status.as_ref().is_some_and(|s|s.stats.is_some())
                &&self.planning_pending.is_none()&&self.auction.scene.is_none();
            let pointer=self.pointer.map(|p|p.map(|v|v as f32*0.5));
            let [l,t,r,b]=menu.card_assets().entry;
            let entry_hit=pointer.is_some_and(|[x,y]|x>=l&&x<=r&&y>=t&&y<=b);
            if entry_hit||self.card_input.captured()
                ||self.live_status.as_ref().is_some_and(|s|s.card.is_some())
                    &&menu.card_assets().contains(pointer) { shown=false; }
            if self.prompts.request().is_some() {
                let occupied=self.prompts.input_captured()||self.pointer.is_some_and(|p|
                    menu.prompt_assets().input_occupied(p.map(|v|v as f32*0.5),
                        self.live_status.as_ref().map_or([0.;2],|s|s.tutorial_interaction.prompt_offset())));
                shown &= !occupied;
                for a in Action::ALL { allowed[a.index()] &= self.live_status.as_ref()
                    .is_some_and(|s|s.tutorial_enabled_controls.contains(&a.tutorial())); }
            }
            if self.live_status.as_ref().is_some_and(|s|s.event_list_visible&&self.auction.scene.is_none())
                &&self.pointer.is_some_and(|p|menu.event_list_assets().contains(p.map(|v|v as f32*0.5))) {
                shown=false;
            }
        }
        let build=self._fresh_session.is_some()&&!self.live_status.as_ref()
            .is_some_and(|s|s.auction_locked_categories.contains(&category));
        Some(menu.locator_assets().ordinary_toolbar_policy(self.block_information.as_ref()?,
            category,shown,build,allowed))
    }
    fn toolbar_target(&self,menu:&crate::menu::Menu,blocks:&[openmt2001::board::Block])
        ->Option<crate::locator::toolbar_collapse::Action>
    {
        let policy=self.toolbar_policy(menu,blocks,true)?;
        policy.target(self.toolbar_bounds,self.pointer?.map(|v|v as f32*0.5))
    }
    fn begin_toolbar_planning(&mut self,category:usize) {
        if let Some(sender)=&self._fresh_session {
            let (reply,result)=mpsc::channel();
            if sender.send(FreshRequest::Planning{category,reply}).is_ok() {
                self.planning_pending=Some(result);self.send_tutorial_control(11);
            }
        }
    }
    fn main_panel_visible(&self) -> bool {
        if self.ui_graph.is_some(){return self.graph_body(camera_live::ui_graph::PanelId::A);}
        self.live_status.as_ref().is_none_or(|s| s.player_panels[0])
    }
    fn statistics_visible(&self) -> bool {
        if self.ui_graph.is_some(){return self.graph_body(camera_live::ui_graph::PanelId::Comparison);}
        self.live_status.as_ref().is_none_or(|s| s.player_panels[1])
    }
    fn over_navigation(&self) -> bool {
        (self.ui_graph.is_none()||self.graph_input(camera_live::ui_graph::PanelId::E))
        && self.pointer.is_some_and(|[x, y]| {
            let [l, t, r, b] = self.navigation_bounds;
            let x = x as f32 * 0.5;
            let y = y as f32 * 0.5;
            x >= l && x <= r && y >= t && y <= b
        })
    }
    fn over_details(&self) -> bool {
        self.main_panel_visible()
            && self.pointer.is_some_and(|[x, y]| {
                let [l, t, r, b] = self.detail_bounds;
                let x = x as f32 * 0.5;
                let y = y as f32 * 0.5;
                x >= l && x <= r && y >= t && y <= b
            })
    }
    fn over_navigator(&self) -> bool {
        (self.ui_graph.is_none()||self.graph_input(camera_live::ui_graph::PanelId::F))
        && self.pointer.is_some_and(|[x, y]| {
            let [l, t, r, b] = self.navigator_bounds;
            let x = x as f32 * 0.5;
            let y = y as f32 * 0.5;
            x >= l && x <= r && y >= t && y <= b
        })
    }
    fn utility_category(&self, blocks: &[openmt2001::board::Block]) -> Option<usize> {
        let selected = self.selected.map(|hit| {
            if hit.building.is_some_and(|i| {
                self.scene.inspection_buildings.get(i)
                .is_some_and(|building| !building_has_management_link(building))
            }) {
                crate::selection::Hit { category: hit.category, building: None,
                }
            } else {
                hit
            }
        });
        utility_report_category(selected, blocks, self.block_information.as_ref())
    }
    fn scene_pointer(&self) -> Option<[i32; 2]> {
        self.pointer.filter(|[x, y]| {
            if self.adjustment.is_some()
                || self.planning.is_some()
                || self.planning_pending.is_some()
                || (self.statistics_visible()
                    && self
                        .comparison
                        .as_ref()
                        .is_some_and(|c| c.contains([*x, *y])))
                || (self.operations.building.is_some()
                    && self.main_panel_visible()
                    && self.details_visible
                    && !self.strategic
                    && *x >= 0
                    && *x <= 392
                    && *y >= 0
                    && *y <= 640)
                || self.locator_blocked
                || self.over_navigation()
                || self.over_details()
                || (self.main_panel_visible()&&!self.strategic&&self.operations.building.is_none()
                    &&self.toolbar_regions.iter().flatten().any(|b|
                        crate::locator::toolbar_collapse::hit(*b,[*x as f32*0.5,*y as f32*0.5])))
                || self.over_navigator()
            {
                return false;
            }
            self.information
                .iter()
                .filter(|_| self.main_panel_visible() && self.details_visible && !self.strategic)
                .all(|(_, [l, t, r, b])| {
                    let x = *x as f32 * 0.5;
                    let y = *y as f32 * 0.5;
                    x < *l || x > *r || y < *t || y > *b
                })
        })
    }
}

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
                *current = (f64::from(*current)
                    + (f64::from(desired) - f64::from(*current)) / 10.) as f32;
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
                    + (f64::from(endpoint[axis]) - f64::from(start[axis]))
                        * f64::from(phase) / 10.) as f32
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
    pub(crate) process_frontend:Option<process_frontend::Owner>,
    camera_physical: camera_live::PhysicalInput,
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
    loading: Option<mpsc::Receiver<Result<LoadedInspection, String>>>,
    loading_from_setup: bool,
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
}
impl Drop for Intro {
    fn drop(&mut self){
        // Window close/error teardown must cross the same source retirement
        // boundary before fields drop assets or the Session lease.
        self.retire_inspection();
    }
}
impl Intro {
    pub(crate) fn window_focus(&mut self, focused: bool) -> Result<(), String> {
        self.service_camera_turns();
        if let Some(process)=&self.process_frontend {process.borrow_mut().observe(focused)?;}
        self.focus.observe(focused)?;
        if let Some(process)=&self.process_frontend {process.borrow_mut().note_sent(self.focus.sent());}
        Ok(())
    }
    /// Drain before modal/loading early returns and every dependent worker poll.
    /// No call in this service can request work from the suspended Session.
    fn service_camera_turns(&mut self) {
        if let Some(view)=&mut self.inspection {
            view.service_camera_turns(&self.assets.selection,
                self.menu.as_ref().map(|menu|menu.adjustment_assets()));
        }
    }
    /// Full default activation waits for the real audio prefix provider.
    /// Install only before starting a World; installation is not cold attach.
    pub(crate) fn install_focus_effects(&mut self, effects: Box<dyn focus::Effects>) -> Result<(), String> {
        if self.inspection.is_some() || self.loading.is_some() {
            return Err("Focus effects must be installed before World loading".into());
        }
        self.focus.install(effects)
    }
    pub(crate) fn service_focus(&mut self) -> Result<(), String> {
        self.service_camera_turns();
        let result:Result<(),String>=(|| {
            self.focus.poll()?;
            if let Some(process)=&self.process_frontend {process.borrow_mut().complete_physical(self.focus.completed());}
            // A single prefix: release its typed suffix before the next prefix.
            self.focus.service_one()?;
            self.focus.poll()?;
            if let Some(process)=&self.process_frontend {process.borrow_mut().complete_physical(self.focus.completed());}
            Ok(())
        })();
        if let Err(error)=&result {if let Some(process)=&self.process_frontend {process.borrow_mut().fail(error.clone());}}
        result
    }
    pub fn request_process_exit(&self,writer:crate::live_host::process_runtime::QuitWriter)->Result<(),String>{
        self.assets.process_runtime.request_quit(writer).map_err(|e|e.to_string())
    }
    pub fn process_exit_ready(&self)->Result<bool,String>{self.assets.process_runtime.exit_ready().map_err(|e|e.to_string())}

    pub(crate) fn birth_process_frontend(&mut self,window:winit::window::WindowId)->Result<(),String>{
        self.sync_profile_settings()?;
        let receipt=self.assets.activity.take_frontend_startup()?;
        let process=process_frontend::ProcessFrontend::new(receipt,self.assets.process_runtime.clone(),window,self.audio_levels)?;
        self.install_focus_effects(Box::new(process_frontend::Provider(process.clone())))?;
        self.process_frontend=Some(process);Ok(())
    }
    pub(crate) fn physical_sample(&mut self,window:winit::window::WindowId,sample:process_frontend::PhysicalSample)->Result<(),String>{
        if let Some(process)=&self.process_frontend {process.borrow_mut().sample(window,sample,self.focus.sent(),self.focus.completed())?;}
        Ok(())
    }
    fn process_start_ready(&self)->bool {
        camera_producer::startup_ready(&self.camera_worker,&self.camera_worker_failure)
            && !self.focus_pending() && self.process_frontend.as_ref().is_none_or(|p|p.borrow().phase==process_frontend::Phase::Menu)
    }
    fn start_custody(&mut self,fresh:bool)->Result<Option<crate::process_dispatch::DispatchCustody>,String>{
        self.process_frontend.as_ref().map(|p|p.borrow_mut().take_custody(fresh)).transpose()
    }
    pub(crate) fn focus_pending(&self)->bool {self.focus.sent()!=self.focus.completed()}
    pub fn ui_effects_level(&self) -> u8 { self.audio_levels.effects }

    fn settings_snapshot(&self) -> crate::profile_settings::Snapshot {
        crate::profile_settings::Snapshot {
            audio: self.audio_levels,
            graphics: self.inspection.as_ref().map_or(self.graphics_levels,
                |view| view.settings_menu.graphics),
            gameplay: self.inspection.as_ref().map_or(self.gameplay_levels,
                |view| view.settings_menu.gameplay),
        }
    }

    fn persist_settings(&mut self) {
        let snapshot=self.settings_snapshot();
        menu_preferences::Cells{profile:&mut self.settings_profile,saved:&mut self.saved_settings,
            error:&mut self.settings_save_error,audio:&mut self.audio_levels,
            graphics:&mut self.graphics_levels,gameplay:&mut self.gameplay_levels}.persist(snapshot);
    }
    fn sync_profile_settings(&mut self)->Result<(),String>{
        menu_preferences::Cells{profile:&mut self.settings_profile,saved:&mut self.saved_settings,
            error:&mut self.settings_save_error,audio:&mut self.audio_levels,
            graphics:&mut self.graphics_levels,gameplay:&mut self.gameplay_levels}
            .sync_profile(self.menu.as_mut(),self.process_frontend.as_ref())
    }
    /// App invokes this once after EVERY synchronous window-event dispatch,
    /// including inner early returns, before any following OS focus prefix.
    pub(crate) fn commit_menu_settings(&mut self)->Result<(),String>{
        if self.inspection.is_none() && let Some(menu)=&mut self.menu {
            menu_preferences::Cells{profile:&mut self.settings_profile,saved:&mut self.saved_settings,
                error:&mut self.settings_save_error,audio:&mut self.audio_levels,
                graphics:&mut self.graphics_levels,gameplay:&mut self.gameplay_levels}
                .commit_menu(menu,self.process_frontend.as_ref())?;
        } else {self.sync_profile_settings()?;self.persist_settings();}
        Ok(())
    }

    fn retire_inspection(&mut self) {
        self.focus.detach();
        if let Some(process)=&self.process_frontend {process.borrow_mut().draining();}
        if let Some(producer) = self.inspection.as_mut().and_then(|view|view.camera_producer.take()) {
            let (owner, result) = producer.retire();
            assert!(self.camera_parked.is_none(), "camera Owner has one process custodian");
            self.camera_parked = Some(owner);
            if let Err(error) = result { eprintln!("Camera source retirement failed: {error}"); }
        }
        if let Some(view) = &mut self.inspection { camera_producer::cancel_pending(&mut view.camera_turns); }
        if let camera_live::deferred_primary::PrimaryProducts::Deferred(primary)=&mut self.assets.primary_products{
            //Invalidate the current World claim before Endpoint retires/wakes.
            if let Err(error)=primary.retire_for_camera(self.assets.navigator.process_primitives_mut()){
                eprintln!("D current-World retirement failed: {error}");
            }
        }
        if let Err(error)=refresh::retire(&mut self.refresh_retry,&mut self.refreshing_fresh,&mut self.loading,
            self.inspection.as_mut().map(|view|&mut view.camera_endpoint),&mut self.camera_parked){
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
    fn apply_audio_levels(process:&Option<process_frontend::Owner>,levels:crate::settings_menu::Levels) {
        if let Some(process)=process {process.borrow_mut().commit_levels(levels);}
    }
    fn draw_tutorial_visual(
        assets: &Assets,
        menu: &Option<crate::menu::Menu>,
        renderer: &Renderer,
        color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        view: &Inspection,
    ) -> Result<(), String> {
        let Some(target) = view.live_status.as_ref()
            .and_then(|status| status.tutorial_arrows.last()) else { return Ok(());
        };
        let origin = menu.as_ref().ok_or("tutorial prompt assets missing")?
            .prompt_assets().arrow_origin();
        // Frame 50 is the standing tutor pose in the original 90-frame mesh.
        // The native transition timing remains to be recovered.
        let offset = view.live_status.as_ref().map_or([0.; 2], |status| {
            status.tutorial_interaction.prompt_offset()
        });
        let overlay = assets.tutorial_visual.prepare_positioned(
            renderer, origin, [target[0] as f32, target[1] as f32], 50, offset,
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
            &actor_draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>(),
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
            process_frontend:None,
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
            loading_skirmish: false,
            refreshing_fresh: false,
            refresh_retry: refresh::Retry::default(),
            inspection: None,
            camera_physical: camera_live::PhysicalInput::default(),
            camera_parked: Some(camera_producer::new_process_owner()?),
            audio_levels: crate::settings_menu::Levels::default(),
            graphics_levels: crate::settings_menu::GraphicsLevels::default(),
            gameplay_levels: crate::settings_menu::GameplayLevels::default(),
            settings_profile: None,
            saved_settings: crate::profile_settings::Snapshot::default(),
            settings_save_error: None,
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
        if let Some(state) = self.inspection.as_mut().and_then(|view| view.camera_ui.as_mut()) {
            state.cursor_presented();
        }
    }
    pub fn inspection_cursor_kind(&self) -> Option<usize> {
        if self.inspection.is_none() {
            // Keep the existing city indices; appended index3 is original cursor1.
            let hover = self.loading.is_none()
                && self.menu.as_ref().is_some_and(|menu| menu.enabled_cursor_hover());
            return Some(if hover { 3 } else { 0 });
        }
        self.inspection.as_ref().map(|view| {
            if let Some(state) = &view.camera_ui { return state.cursor_index(); }
            if view.secondary_enabled() && let Some(page) = view.settings_menu.page {
                let blocked = view.toolbar_right_held || view.save_visible()
                    || view.stats_requested
                    || view.live_status.as_ref().is_some_and(|status|status.stats.is_some());
                let pointer = view.pointer.map(|p|p.map(|v|v as f32*0.5));
                return if !blocked && self.assets.settings_menu.action_cursor_hover(page,pointer) {
                    3
                } else { 0 };
            }
            if self.menu.as_ref().is_some_and(|menu|
                view.toolbar_target(menu,&self.assets.selection.blocks).is_some()) { return 3; }
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
    ) -> Result<mpsc::Receiver<Result<crate::fresh_city::BuildInspection, String>>, String> {
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
        camera_producer::request_refresh(self.inspection.as_mut(), &self.assets.selection,
            self.menu.as_ref().map(|menu|menu.adjustment_assets()), &mut self.loading,
            &mut self.refreshing_fresh, &mut self.refresh_retry)
    }
    pub fn inspecting(&self) -> bool {
        self.inspection.is_some()
    }
    /// App feeds every event before logical/menu/Inspection routing. Keep this
    /// process input across city exit, true Load and presentation refresh.
    pub fn camera_physical_event(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::{ElementState, WindowEvent};
        match event {
            WindowEvent::KeyboardInput { event, .. } =>
                self.camera_physical.key(event.physical_key, event.state == ElementState::Pressed),
            WindowEvent::MouseInput { button, state, .. } =>
                self.camera_physical.button(*button, *state == ElementState::Pressed),
            WindowEvent::CursorMoved { position, .. } =>
                self.camera_physical.pointer([position.x, position.y]),
            WindowEvent::Focused(false) => self.camera_physical.unfocus(),
            _ => (),
        }
    }
    pub(crate) fn camera_physical_sample(&mut self,sample:process_frontend::PhysicalSample){
        process_frontend::project_camera_sample(&mut self.camera_physical,self.inspection.as_mut(),sample);
    }
    pub fn inspection_key(&mut self, key: winit::keyboard::NamedKey, pressed: bool) -> Result<(), String> {
        self.service_camera_turns();
        let result = self.inspection_key_inner(key, pressed);
        if let Some(view) = &mut self.inspection { view.commit_camera(); }
        result
    }
    fn inspection_key_inner(
        &mut self,
        key: winit::keyboard::NamedKey,
        pressed: bool,
    ) -> Result<(), String> {
        use winit::keyboard::NamedKey;
        // Releases must retire held arrows even when a modal consumes the event.
        // Keep admitted taps until the camera callback consumes them.
        if !pressed && let Some(view) = &mut self.inspection {
            let index = match key {
                NamedKey::ArrowRight => Some(0),
                NamedKey::ArrowLeft => Some(1),
                NamedKey::ArrowDown => Some(2),
                NamedKey::ArrowUp => Some(3),
                _ => None,
            };
            if let Some(i) = index {
                view.keys[i] = false;
            }
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.result.is_some() || view.result_pending.is_some()
                || view.live_status.as_ref().is_some_and(|status| status.result_request.is_some())
        }) {
            if pressed && key == NamedKey::Escape
                && let Some(result) = self.inspection.as_mut().and_then(|view| view.result.as_mut()) {
                result.bypass_input = true;
            }
            return Ok(());
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.stats_requested || view.live_status.as_ref().is_some_and(|status|status.stats.is_some())
        }) {
            return Ok(());
        }
        if pressed && let Some(view) = &mut self.inspection {
            if view.save_visible() {
                match key {
                    NamedKey::Escape => {view.save_dialog.escape();}
                    NamedKey::Backspace => view.save_dialog.erase(),
                    NamedKey::Enter => {
                        if let Some(action)=view.save_dialog.enter() {
                            Self::submit_save_dialog_action(view,action);
                        }
                    }
                    _=>(),
                }
                return Ok(());
            }
            if view.prompts.request().is_some() {
                if key == NamedKey::Enter
                    && let Some(id) = view.prompts.begin_dismiss()
                {
                    view.dismiss_prompt(id)?;
                }
                return Ok(());
            }
            if view.options_visible() {
                if view.settings_visible() {
                    if key == NamedKey::Escape || key == NamedKey::Enter {
                        view.settings_menu.back();
                    }
                    return Ok(());
                }
                if key == NamedKey::Escape || key == NamedKey::Enter {
                    view.close_options()?;
                }
                return Ok(());
            }
            if key == NamedKey::Escape && view._fresh_session.is_some()
                && view.planning.is_none() && view.planning_pending.is_none()
                && view.adjustment.is_none()
            {
                view.open_options()?;
                return Ok(());
            }
            if view.live_status.as_ref().is_some_and(|status| status.tutorial_controls.all_disabled)
                && key != NamedKey::Escape
            {
                return Ok(());
            }
            if key == NamedKey::Space
                && let Some(sender) = &view._fresh_session
            {
                let running = !view.live_status.as_ref().is_some_and(|s| s.running);
                sender
                    .send(FreshRequest::Run(running))
                    .map_err(|e| e.to_string())?;
                if let Some(status) = &mut view.live_status {
                    status.running = running && status.failure.is_none();
                }
                view.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
                return Ok(());
            }
        }
        if key == NamedKey::Escape && pressed {
            if let Some(view) = &mut self.inspection
                && (view.planning.take().is_some() || view.planning_pending.take().is_some())
            {
                return Ok(());
            }
            if let Some(view) = &mut self.inspection
                && let Some(adjustment) = &mut view.adjustment
            {
                if adjustment.request_close() {
                    view.adjustment = None;
                }
                return Ok(());
            }
            self.retire_inspection();
            self.last_sample = None;
            return Ok(());
        }
        if let Some(view) = &mut self.inspection {
            if view.auction_visible()
                || view.strategic
                || view.adjustment.is_some()
                || view.planning.is_some()
                || view.live_status.as_ref().is_some_and(|status| {
                    status.tutorial_controls.camera_disabled
                        || status.tutorial_controls.all_disabled
                })
            {
                return Ok(());
            }
            let index = match key {
                NamedKey::ArrowRight => Some(0),
                NamedKey::ArrowLeft => Some(1),
                NamedKey::ArrowDown => Some(2),
                NamedKey::ArrowUp => Some(3),
                _ => None,
            };
            if let Some(i) = index && self.process_frontend.is_none() {
                view.keys[i] = pressed;
                view.taps[i] |= pressed;
            }
            // Height keys remain a diagnostic host control, separate from native mouse zoom.
            if pressed && matches!(key, NamedKey::PageUp | NamedKey::PageDown) {
                view.controls.height = page_height(view.controls.height, key == NamedKey::PageUp);
            }
        }
        Ok(())
    }
    pub fn inspection_text(&mut self,text:&str) {
        if let Some(view)=&mut self.inspection
            && view.options_visible() && view.save_visible() {
            view.save_dialog.input(text);
        }
    }
    fn submit_save_dialog_action(view:&mut Inspection,action:crate::save_dialog::Action) {
        use crate::save_dialog::Action;
        if !matches!(&action,Action::Back) && !view.options_ack.ready() {
            view.save_dialog.message=Some("Preparing Save Game...".into());
            return;
        }
        match action {
            Action::Back=>(),
            Action::NewName(name)=> {
                Self::submit_save_target(view,
                crate::save_menu::Target::NewName(name))
            }
            Action::ExistingSlot(id)=> {
                Self::submit_save_target(view,
                crate::save_menu::Target::ExistingSlot(id))
            }
            Action::DeleteSlot(id)=>{
                if view.save_delete_pending.is_some() {return;}
                let Some(sender)=&view._fresh_session else {
                    view.save_dialog.message=Some("Delete unavailable in this view".into());
                    return;
                };
                let (reply,result)=mpsc::channel();
                match sender.send(FreshRequest::DeleteSave{id,reply}) {
                    Ok(())=>{view.save_delete_pending=Some(result);view.save_dialog.busy=true;
                        view.save_dialog.message=Some("Deleting...".into());}
                    Err(_)=> {
                        view.save_dialog.message=Some("Delete failed: session closed".into())
                    }
                }
            }
        }
    }
    fn submit_save_target(view:&mut Inspection,target:crate::save_menu::Target) {
        if let Some(error) = view.camera_producer.as_ref().and_then(|p|p.error()) {
            view.save_dialog.message = Some(format!("Save failed: {error}")); return;
        }
        if view.save_pending.is_some() || view.camera_capture_pending.is_some() {return;}
        if !view.options_ack.ready() {
            view.save_dialog.message=Some("Preparing Save Game...".into());
            return;
        }
        if let Some(error)=view.live_status.as_ref().and_then(|status|status.failure.as_ref()) {
            view.save_dialog.busy=false;
            view.save_dialog.message=Some(format!("Save failed: {error}"));
            return;
        }
        if let Some(state)=&mut view.camera_ui {
            view.camera_capture_pending=Some((target,state.queue_save_capture()));
            view.save_dialog.busy=true;
            view.save_dialog.message=Some("Saving...".into());
            return;
        }
        Self::send_save_target(view,target,None);
    }
    fn poll_camera_save(view:&mut Inspection) {
        let Some((_,receiver))=&view.camera_capture_pending else{return};
        let result=match receiver.try_recv() {
            Ok(value)=>value,
            Err(mpsc::TryRecvError::Empty)=>return,
            Err(mpsc::TryRecvError::Disconnected)=>Err("Camera capture boundary closed".into()),
        };
        let (target,_)=view.camera_capture_pending.take().unwrap();
        match result {
            Ok(capture)=>Self::send_save_target(view,target,Some(capture)),
            Err(error)=>{view.save_dialog.busy=false;view.save_dialog.message=Some(format!("Save failed: {error}"));}
        }
    }
    fn send_save_target(view:&mut Inspection,target:crate::save_menu::Target,
        capture:Option<openmt2001::camera::bank::Capture>) {
        if !view.options_ack.ready() {
            view.save_dialog.busy=false;
            view.save_dialog.message=Some("Save cancelled: Options closed".into());
            return;
        }
        let Some(sender)=&view._fresh_session else {
            view.save_dialog.busy=false;
            view.save_dialog.message=Some("Save unavailable in this view".into());
            return;
        };
        let (reply,result)=mpsc::channel();
        let request=FreshRequest::SaveSelected {target,camera:view.camera,capture,
            game_mode:if view.auction.scene.is_some(){2}else{1},
            interface_mode:if view.strategic{3}else{0},reply,
        };
        match sender.send(request) {
            Ok(())=>{view.save_pending=Some(result);view.save_dialog.busy=true;
                view.save_dialog.message=Some("Saving...".into());}
            Err(_)=>{view.save_dialog.busy=false;
                view.save_dialog.message=Some("Save failed: session closed".into());},
        }
    }
    pub fn inspection_cursor(&mut self, position: [f64; 2], size: [u32; 2]) {
        if let Some(view) = &mut self.inspection {
            if view.adjustment.is_none()
                && !view.options_visible()
                && view.auction.scene.is_none()
                && view.prompts.request().is_none()
                && !view.stats_requested
                && !view.live_status.as_ref().is_some_and(|status|status.stats.is_some())
                && !view.live_status.as_ref().is_some_and(|status| {
                    status.tutorial_controls.camera_disabled
                        || status.tutorial_controls.all_disabled
                })
                && view.mouse_buttons[1] && self.process_frontend.is_none()
                && let Some(previous) = view.cursor
            {
                for i in 0..2 {
                    view.drag[i] = view.drag[i].saturating_add(
                        (position[i] - previous[i]).round().clamp(-4096., 4096.) as i32,
                    );
                }
            }
            view.cursor = Some(position);
            view.pointer = internal_pointer(position, size);
            let time_input = !view.stats_requested
                && !view.live_status.as_ref().is_some_and(|status| status.stats.is_some())
                && !view.options_visible() && view.settings_menu.page.is_none()
                && view.prompts.request().is_none()
                && view.live_status.as_ref().is_some_and(|status|status.time_display.visible);
            let time_pointer = time_input.then_some(view.pointer).flatten()
                .map(|point| point.map(|v| v as f32 * 0.5));
            let auction_time = view.live_status.as_ref().is_some_and(|status| status.auction.is_some());
            let time_hit=time_pointer.and_then(|point|view.time_target(&self.assets.time_panel,point,auction_time));
            if view.time_panel.graph_cursor(time_hit,time_input&&view.mouse_buttons[0]).is_some()
                && let Err(error) = view.gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Time held re-entry sound: {error}");
            }
            if let (Some(menu),Some(snapshot))=(&self.menu,
                view.live_status.as_ref().and_then(|status|status.stats.as_ref()),
            ) {
                let _=view.stats_panel.pointer(menu.stats_assets(),
                    view.pointer.map(|point|point.map(|v|v as f32*0.5)),
                    view.mouse_buttons[0],snapshot,
                );
            }
            if view.secondary_enabled() && matches!(view.settings_menu.page,
                Some(crate::settings_menu::Page::Sound|crate::settings_menu::Page::Graphics|
                    crate::settings_menu::Page::Gameplay))
            {
                let distance = view.settings_menu.graphics.distance;
                if view.settings_menu.drag(&self.assets.settings_menu,
                    view.pointer.map(|p| p.map(|v| v as f32*0.5)), &mut self.audio_levels,
                )
                {
                    Self::apply_audio_levels(&self.process_frontend, self.audio_levels);
                }
                if view.settings_menu.graphics.distance != distance {
                    view.reframe_pending = true;
                }
            }
            if view.settings_visible()
                && view.settings_menu.pointer_click_cue(&self.assets.settings_menu,
                    view.pointer.map(|p| p.map(|v| v as f32*0.5)),
                )
                && let Err(error)=view.gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Settings held re-entry sound: {error}");
            }
            if view.options_visible() && !view.save_visible()
                && view.settings_menu.page.is_none()
                && view.game_options.pointer_click_cue(&self.assets.game_options,
                    view.pointer.map(|p| p.map(|v| v as f32*0.5)),
                )
                && let Err(error)=view.gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Options held re-entry sound: {error}");
            }
            if view.save_visible() {
                let pointer=view.pointer.map(|p|p.map(|v|v as f32*0.5));
                let cue=if view.save_dialog.generic_prompt().is_some() {
                    self.menu.as_ref().is_some_and(|menu| {
                        let bounds=menu.prompt_assets().dialog_buttons(2);
                        bounds.len()==2 && view.save_dialog.prompt_click_cue(
                            pointer,[bounds[0],bounds[1]])
                    })
                } else {
                    view.save_dialog.pointer_click_cue(&self.assets.save_dialog,pointer)
                };
                if cue && let Err(error)=view.gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick) {
                    eprintln!("Save Game held re-entry sound: {error}");
                }
            }
            if view.auction_visible()
                && let (Some(menu),Some(auction))=(&self.menu,
                    view.live_status.as_ref().and_then(|status|status.auction.as_ref()),
                )
                && view.auction_panel.pointer_click_cue(menu.auction_panel_assets(),auction,
                    view.pointer.filter(|_|view.prompts.request().is_none()
                        && !view.stats_requested).map(|point|point.map(|v|v as f32*0.5)),
                )
                && let Err(error)=view.gameplay_sound_player
                    .apply(crate::gameplay_sound::Operation::UiLightClick)
            {
                eprintln!("Auction held re-entry sound: {error}");
            }
            if let Some(planning)=&mut view.planning {
                let occupied=view.live_status.as_ref().is_some_and(|status| {
                    status.event_list_visible && view.auction.scene.is_none()
                })
                    && self.menu.as_ref().is_some_and(|menu| {
                    view.pointer.is_some_and(|point| {
                        menu.event_list_assets().contains(point.map(|v|v as f32*0.5))
                    })
                });
                planning.pointer(view.pointer.filter(|_|!occupied));
                if planning.pointer_click_cue()
                    && let Err(error)=view.gameplay_sound_player
                        .apply(crate::gameplay_sound::Operation::UiLightClick)
                {
                    eprintln!("Planning held re-entry sound: {error}");
                }
            }
        }
    }
    pub fn inspection_scroll(&mut self, delta: i32) {
        if let Some(view)=&mut self.inspection && view.save_visible() {
            view.save_dialog.scroll(delta);
            return;
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.stats_requested
            || view.live_status.as_ref().is_some_and(|status|status.stats.is_some())
        }) {return;}
        if self
            .inspection
            .as_ref()
            .is_some_and(|v| v.auction.scene.is_some() || v.prompts.request().is_some())
        {
            return;
        }
        if self.inspection.as_ref().is_some_and(|view| {
            view.live_status.as_ref().is_some_and(|status| status.tutorial_controls.all_disabled)
        }) {
            return;
        }
        if let Some(view) = &mut self.inspection
            && view.main_panel_visible()
            && view.details_visible
            && !view.strategic
            && view.operations.building.is_some()
            && view.operations.mode == 3
            && let Some(menu) = &self.menu
            && view
                .pointer
                .is_some_and(|p| menu.business_assets().contains(p.map(|v| v as f32 * 0.5)))
        {
            let rows = view
                .operations
                .inspection
                .as_ref()
                .map_or(0, |d| d.conversions.len());
            view.operations.scroll = (view.operations.scroll as i64 - i64::from(delta))
                .clamp(0, rows.saturating_sub(1) as i64)
                as usize;
            return;
        }
        if let Some(planning) = self
            .inspection
            .as_mut()
            .and_then(|view| view.planning.as_mut())
        {
            planning.scroll(delta);
            return;
        }
        if let Some(view) = &mut self.inspection
            && !view.strategic
            && view.main_panel_visible()
            && view.details_visible
            && view.over_details()
            && view.utility_report.details
            && let Some(category) = view.utility_category(&self.assets.selection.blocks)
            && let Some(report) = &view.block_information
            && report.categories[category].is_some()
        {
            view.utility_report.scroll = (view.utility_report.scroll as i64 - i64::from(delta))
                .clamp(
                    0,
                    report.land_rent[category].len().saturating_sub(22) as i64,
                ) as usize;
            view.utility_report.dirty = true;
            return;
        }
        if let (Some(view), Some(menu)) = (&mut self.inspection, &self.menu)
            && view.strategic
            && view.locator_blocked
            && let Some(locator) = &mut view.locator
        {
            locator.scroll(menu.locator_assets(), delta);
        }
    }
    pub fn inspection_mouse(&mut self, button: winit::event::MouseButton, pressed: bool) {
        self.service_camera_turns();
        self.inspection_mouse_inner(button, pressed);
        if let Some(view) = &mut self.inspection { view.commit_camera(); }
    }
    fn inspection_mouse_inner(&mut self, button: winit::event::MouseButton, pressed: bool) {
        if let Some(view) = &mut self.inspection {
            if view.ui_graph.is_some()&&button==winit::event::MouseButton::Left{
                //Read the SAME staged GUI owner; physical/display state
                //cannot overwrite old current/prior GUI cells before poll.
                let hit=camera_live::upper_f::panel_hit(view);
                let rejected=match hit{
                    Ok(false)=>None,
                    Ok(true)=>Some("F original parent/group button transaction is unjoined; legacy Board/release-camera path rejected".to_owned()),
                    Err(error)=>Some(error),
                };
                if let Some(error)=rejected{
                    if let Some(state)=&mut view.camera_ui{state.reject_upper_input(error);}
                    return;
                }
            }
            // Capture both edges BEFORE stats/save/settings/prompt/card/auction
            // returns. Do not move the camera dispatcher's late Right assignment.
            if button == winit::event::MouseButton::Right
                && view.toolbar_right_held != pressed
            {
                view.toolbar_right_held = pressed;
                view.toolbar_input.cancel();
            }
            let side_policy=self.menu.as_ref().and_then(|menu|
                view.toolbar_policy(menu,&self.assets.selection.blocks,true));
            view.toolbar_input.update(side_policy,view.toolbar_bounds,
                view.pointer.map(|p|p.map(|v|v as f32*0.5)));
            let stats_modal=view.stats_requested
                || view.live_status.as_ref().is_some_and(|status|status.stats.is_some());
            if stats_modal { view.auction_panel.cancel_press(); }
            if stats_modal || view.settings_visible() || view.options_visible()
                || view.prompts.request().is_some() {
                view.time_panel.cancel_input();view.time_pressed=None;
            }
            if let (Some(menu),Some(snapshot))=(&self.menu,
                view.live_status.as_ref().and_then(|status|status.stats.as_ref()),
            ) {
                if button==winit::event::MouseButton::Left {
                    if let Some(action)=view.stats_panel.pointer(menu.stats_assets(),
                        view.pointer.map(|point|point.map(|v|v as f32*0.5)),pressed,snapshot,
                    )
                        && let Some(sender)=&view._fresh_session {
                        let _=sender.send(FreshRequest::Stats(action));
                        view.stats_requested=true;
                        view.last_status_query=Instant::now()-std::time::Duration::from_secs(1);
                    }
                    view.mouse_buttons[0]=pressed;
                }
                view.drag=[0;2];
                return;
            }
            if stats_modal {return;}
            if view.save_visible() {
                if button==winit::event::MouseButton::Left {
                    let pointer=view.pointer.map(|p|p.map(|v|v as f32*0.5));
                    let action = if view.save_dialog.generic_prompt().is_some() {
                        self.menu.as_ref().and_then(|menu| {
                            let bounds=menu.prompt_assets().dialog_buttons(2);
                            (bounds.len()==2).then(|| {
                                    view.save_dialog.mouse_generic(
                                pointer,pressed,[bounds[0],bounds[1]],
                                    )
                                }).flatten()
                        })
                    } else {
                        view.save_dialog.mouse(&self.assets.save_dialog,pointer,pressed)
                    };
                    if let Some(action)=action {
                        Self::submit_save_dialog_action(view,action);
                    }
                    let cue=if view.save_dialog.generic_prompt().is_some() {
                        self.menu.as_ref().is_some_and(|menu| {
                            let bounds=menu.prompt_assets().dialog_buttons(2);
                            bounds.len()==2 && view.save_dialog.prompt_click_cue(
                                pointer,[bounds[0],bounds[1]])
                        })
                    } else {
                        view.save_dialog.pointer_click_cue(&self.assets.save_dialog,pointer)
                    };
                    if cue && let Err(error)=view.gameplay_sound_player
                        .apply(crate::gameplay_sound::Operation::UiLightClick) {
                        eprintln!("Save Game button press sound: {error}");
                    }
                }
                return;
            }
            if view.settings_visible() {
                if button == winit::event::MouseButton::Left {
                    let pointer=view.pointer.map(|p| p.map(|v| v as f32*0.5));
                    let distance=view.settings_menu.graphics.distance;
                    if view.settings_menu.mouse(&self.assets.settings_menu,pointer,pressed,
                        &mut self.audio_levels,
                    ) {
                        Self::apply_audio_levels(&self.process_frontend, self.audio_levels);
                    }
                    if view.settings_menu.graphics.distance != distance {
                        view.reframe_pending=true;
                    }
                    if view.settings_menu.pointer_click_cue(&self.assets.settings_menu,pointer)
                        && let Err(error)=view.gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Settings button press sound: {error}");
                    }
                }
                return;
            }
            if view.options_visible() {
                if button == winit::event::MouseButton::Left {
                    let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                    let activation = view.game_options.mouse(&self.assets.game_options, pointer, pressed);
                    if view.game_options.pointer_click_cue(&self.assets.game_options,pointer)
                        && let Err(error)=view.gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Options button press sound: {error}");
                    }
                    if !pressed {
                        match activation {
                            Some(crate::game_options::Button::Continue) => {
                                if let Err(error) = view.close_options() {
                                    eprintln!("[Options] {error}");
                                }
                            }
                            Some(crate::game_options::Button::Quit) => {
                                self.retire_inspection();
                                self.last_sample = None;
                                return;
                            }
                            Some(crate::game_options::Button::Settings) => {
                                view.settings_menu.open();
                            }
                            Some(crate::game_options::Button::Save) if view.save_pending.is_none() && view.options_ack.ready() => {
                                if let Some(root) = view.save_profile_root.as_deref() {
                                    view.save_dialog.open(root);
                                } else {view.game_options.set_message("Save unavailable in this view".into());}
                            }
                            _ => {}
                        }
                    }
                }
                return;
            }
            let event_list_occupied = view.live_status.as_ref().is_some_and(|status| {
                    status.event_list_visible && view.auction.scene.is_none()
                })
                && self.menu.as_ref().is_some_and(|menu| {
                    view.pointer.is_some_and(|point| {
                        menu.event_list_assets().contains(point.map(|v|v as f32*0.5))
                    })
                });
            // Original tutorials enable selected controls while a prompt stays
            // open. Preserve a captured planning press through its release.
            if let Some(planning) = &mut view.planning {
                planning.pointer(view.pointer.filter(|_|!event_list_occupied));
            }
            let prompt_input_occupied = view.prompts.request().is_some()
                && (view.prompts.input_captured() || self.menu.as_ref().is_some_and(|menu| {
                        view.pointer.is_some_and(|pointer| {
                            menu.prompt_assets().input_occupied(
                        pointer.map(|value| value as f32 * 0.5),
                        view.live_status.as_ref().map_or([0.;2], |status| {
                                    status.tutorial_interaction.prompt_offset()
                                }),
                            )
                        })
                    }));
            let tutorial_planning_hit = button == winit::event::MouseButton::Left
                && !prompt_input_occupied
                && view.planning.as_ref().is_some_and(|planning| {
                    planning.tutorial_control_pressed()
                        || planning.tutorial_drag_at()
                            && view.selected.is_some_and(|hit| hit.building.is_none())
                        || planning.tutorial_control_at().is_some_and(|id| {
                            view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        })
                });
            let tutorial_auction_hit = button == winit::event::MouseButton::Left
                && view.auction.scene.is_some()
                && (view.auction_panel.tutorial_control_pressed()
                    || !prompt_input_occupied && self.menu.as_ref().is_some_and(|menu| {
                            view.live_status.as_ref().is_some_and(|status| {
                                status.auction.as_ref().is_some_and(|auction| {
                                    view.auction_panel.tutorial_control_at(menu.auction_panel_assets(),
                                    view.pointer.map(|p|p.map(|v|v as f32*0.5)),auction,
                                        )
                                    .is_some_and(|id| {
                                            status.tutorial_enabled_controls.contains(&id)
                                        })
                                })
                            })
                        }));
            let tutorial_locator_hit = button == winit::event::MouseButton::Left
                && view.strategic
                && self.menu.as_ref().zip(view.locator.as_ref()).is_some_and(|(menu, locator)| {
                    locator.tutorial_control_at(menu.locator_assets(), view.pointer)
                        .is_some_and(|id| {
                                    view.live_status.as_ref().is_some_and(|status| {
                                        status.tutorial_enabled_controls.contains(&id)
                                    })
                                })
                        || locator.tutorial_control_pressed()
                },
                    );
            let tutorial_side_hit=button==winit::event::MouseButton::Left
                && self.menu.as_ref().is_some_and(|menu|
                    view.toolbar_target(menu,&self.assets.selection.blocks).is_some()
                        || view.toolbar_input.captured());
            let tutorial_property_hit = tutorial_side_hit || button == winit::event::MouseButton::Left
                && !view.strategic && view.main_panel_visible() && view.details_visible
                && self.menu.as_ref().zip(view.block_information.as_ref())
                    .zip(view.utility_category(&self.assets.selection.blocks)).is_some_and(|((menu, report), category)| {
                        (if view.ordinary_toolbar_category(&self.assets.selection.blocks).is_some() {
                            menu.locator_assets().ordinary_body_tutorial_at(
                                &view.utility_report,view.pointer,report,category)
                        } else { menu.locator_assets().property_tutorial_control_at(
                            &view.utility_report,view.pointer,report,category) })
                            .is_some_and(|id| {
                                view.live_status.as_ref().is_some_and(|status| {
                                    status.tutorial_enabled_controls.contains(&id)
                                })
                            })
                            || view.utility_report.tutorial_control_pressed()
                    });
            let tutorial_business_hit = button == winit::event::MouseButton::Left
                && self.menu.as_ref().is_some_and(|menu| {
                    let id = if let Some(adjustment) = &mut view.adjustment {
                        adjustment.tutorial_control_at(menu.adjustment_assets(), view.pointer)
                    } else if !view.strategic && view.main_panel_visible() && view.details_visible
                        && view.operations.building.is_some() {
                        view.operations.tutorial_control_at(menu.business_assets(),
                            view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                        )
                    } else { None };
                    !prompt_input_occupied && id.is_some_and(|id| {
                            view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        })
                        || view.adjustment.as_ref().is_some_and(|a| a.tutorial_control_pressed())
                        || view.operations.tutorial_control_pressed()
                });
            if view.prompts.request().is_some()
                && !tutorial_locator_hit && !tutorial_property_hit && !tutorial_planning_hit
                && !tutorial_business_hit && !tutorial_auction_hit
            {
                if let Some(menu) = &self.menu {
                    view.prompts.cursor(
                        view.pointer
                            .map(|p| p.map(|v| v as f32 * 0.5))
                            .unwrap_or([-1.; 2]),
                    );
                    if button == winit::event::MouseButton::Left {
                        if let Some(dialog) = view.prompts.request().and_then(|r| r.dialog) {
                            view.prompts.dialog_mouse(
                                pressed,
                                &menu.prompt_assets().dialog_buttons(dialog.button_count),
                            );
                        } else if let Some(id) = view
                            .prompts
                            .request_left_button(pressed, menu.prompt_assets().button_at(
                                view.live_status.as_ref().map_or([0.; 2], |status| {
                                    status.tutorial_interaction.prompt_offset()
                                }),
                            ),
                        )
                        {
                            let _ = view.dismiss_prompt(id);
                        }
                    }
                }
                return;
            }
            let tutorial_go_to_block_hit = button == winit::event::MouseButton::Left
                && view.main_panel_visible()
                && view.details_visible
                && !view.strategic
                && view.selected.is_some_and(|hit| hit.building.is_some())
                && view.live_status.as_ref().is_some_and(|status|
                    status.tutorial_enabled_controls.contains(&14))
                && self.menu.as_ref().is_some_and(|menu| {
                    view.pointer.is_some_and(|point| {
                        menu.business_assets()
                        .go_to_block_contains(point.map(|value| value as f32 * 0.5))
                    })
                });
            let tutorial_control_hit = view.live_status.as_ref().is_some_and(|status| {
                status.tutorial_enabled_controls.contains(&103) && view.over_navigator()
                    || status.tutorial_enabled_controls.contains(&73) && view.over_navigation()
                    || status.tutorial_interaction.has_pick_gate()
                        && view.scene_pointer().is_some()
                    || status.tutorial_enabled_controls.iter().any(|id| matches!(id, 76..=78))
                        && view.statistics_visible()
                        && view.comparison.as_ref().is_some_and(|comparison| {
                            view.pointer.is_some_and(|point| comparison.contains(point))
                        })
            }) || tutorial_go_to_block_hit || view.tutorial_go_to_block_pressed
                || tutorial_locator_hit || tutorial_property_hit || tutorial_planning_hit || tutorial_business_hit || tutorial_auction_hit;
            if view.live_status.as_ref().is_some_and(|status| status.tutorial_controls.all_disabled)
                && !tutorial_control_hit
            {
                view.mouse_buttons = [false; 2];
                view.drag = [0; 2];
                return;
            }
            if button == winit::event::MouseButton::Left
                && (tutorial_go_to_block_hit || view.tutorial_go_to_block_pressed)
            {
                if pressed {
                    view.tutorial_go_to_block_pressed = tutorial_go_to_block_hit;
                } else {
                    let activate = view.tutorial_go_to_block_pressed && tutorial_go_to_block_hit;
                    view.tutorial_go_to_block_pressed = false;
                    if activate && let Some(hit) = view.selected {
                        if view.camera_producer.is_some() {
                            let post=view.camera_control_post(14);
                            let event=camera_producer::Event::Block{category:self.assets.selection.blocks[hit.category].id as u32,
                                selected:crate::selection::Hit{category:hit.category,building:None},origin:camera_producer::BlockOrigin::Selection};
                            match view.queue_camera_event(event,None,None,false){Ok(pending)=>pending.posts.extend(post),Err(error)=>view.save_dialog.message=Some(error)}
                        } else {
                            view.focus_category(hit.category, &self.assets.selection);
                            view.send_tutorial_control(14);
                        }
                    }
                }
                view.mouse_buttons[0] = false;
                view.double_click.clear();
                return;
            }
            if let (Some(menu), Some(status)) = (&self.menu, &view.live_status) {
                let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                let [l, t, r, b] = menu.card_assets().entry;
                let entry_hit = pointer.is_some_and(|[x, y]| x >= l && x <= r && y >= t && y <= b);
                if status.card.is_some() && menu.card_assets().contains(pointer)
                    || entry_hit
                    || view.card_input.captured()
                {
                    if button == winit::event::MouseButton::Left
                        && let Some(action) = view.card_input.mouse(
                            menu.card_assets(),
                            status.card,
                            status.card_entry,
                            pointer,
                            pressed,
                        )
                        && let Some(sender) = &view._fresh_session
                    {
                        let _ = sender.send(FreshRequest::Card(action));
                        view.last_status_query = Instant::now() - std::time::Duration::from_secs(1);
                    }
                    view.mouse_buttons = [false; 2];
                    view.drag = [0; 2];
                    return;
                }
            }
            if button == winit::event::MouseButton::Left
                && view._fresh_session.is_some()
                && view.live_status.as_ref().is_some_and(|status|status.time_display.visible)
                && let Some(pointer) = view.pointer.map(|p| p.map(|v| v as f32 * 0.5))
            {
                let auction=view.live_status.as_ref().is_some_and(|status|status.auction.is_some());
                let hit = view.time_target(&self.assets.time_panel,pointer,auction);
                if view.time_occupied(&self.assets.time_panel,pointer,auction) || view.time_pressed.is_some() {
                    view.time_panel.graph_cursor(hit,false);
                    if view.time_panel.mouse(pressed).is_some()
                        && let Err(error) = view.gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Time button press sound: {error}");
                    }
                    if pressed {
                        view.time_pressed = hit;
                    } else if hit.is_some() && hit == view.time_pressed {
                        match hit.unwrap() {
                            crate::time_panel::Button::Speed => {
                                if let Some(sender) = &view._fresh_session {
                                    let _ = sender.send(FreshRequest::ToggleSpeed);
                                    view.last_status_query = Instant::now()
                                        - std::time::Duration::from_secs(1);
                                }
                            }
                            crate::time_panel::Button::Options => {
                                if let Err(error) = view.open_options() {
                                    eprintln!("[Options] {error}");
                                }
                            }
                            crate::time_panel::Button::Manager => {
                                if let Some(sender)=&view._fresh_session {
                                    let _=sender.send(FreshRequest::Stats(crate::live_host::StatsAction::Open,
                                    ));
                                    view.stats_requested=true;
                                    view.keys=[false;4];view.taps=[false;4];view.drag=[0;2];
                                    view.last_status_query=Instant::now()-std::time::Duration::from_secs(1);
                                }
                            }
                        }
                    }
                    if !pressed { view.time_pressed = None; }
                    view.mouse_buttons = [false; 2];
                    view.drag = [0; 2];
                    return;
                }
            }
            if view.auction_visible() {
                if button == winit::event::MouseButton::Left
                    && let (Some(menu), Some(auction)) = (
                        &self.menu,
                        view.live_status.as_ref().and_then(|s| s.auction.as_ref()),
                    )
                {
                    let pointer=view.pointer.filter(|_|!prompt_input_occupied)
                        .map(|point|point.map(|v|v as f32*0.5));
                    let activation=view.auction_panel.mouse(menu.auction_panel_assets(),
                        auction,pointer,pressed,
                    );
                    if view.auction_panel.pointer_click_cue(menu.auction_panel_assets(),
                        auction,pointer,
                    )
                        && let Err(error)=view.gameplay_sound_player
                            .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Auction button press sound: {error}");
                    }
                    if let (Some(button),Some(sender))=(activation,&view._fresh_session) {
                        let _=sender.send(FreshRequest::AuctionButton(button));
                        view.last_status_query=Instant::now()-std::time::Duration::from_secs(1);
                    }
                }
                return;
            }
            if !view.strategic
                && button == winit::event::MouseButton::Left
                && let Some(category) = view.live_status.as_ref().and_then(|status| {
                    crate::notice_panel::View::category(
                        &status.notices,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )
                })
            {
                if pressed
                    && let Some(index) = self
                        .assets
                        .selection
                        .blocks
                        .iter()
                        .position(|b| b.id == category as usize)
                {
                    view.planning = None;
                    view.planning_pending = None;
                    view.focus_category(index, &self.assets.selection);
                }
                view.mouse_buttons = [false; 2];
                view.double_click.clear();
                return;
            }
            if let Some(planning) = &mut view.planning {
                planning.pointer(view.pointer.filter(|_|!event_list_occupied));
                if button == winit::event::MouseButton::Left {
                    planning.mouse(pressed);
                    let control = planning.take_tutorial_control();
                    if planning.take_placement_request() {
                        let (reply, receiver) = mpsc::channel();
                        if view._fresh_session.as_ref().is_some_and(|sender| {
                            sender.send(FreshRequest::BuildDrag { reply }).is_ok()
                        })
                        {
                            planning.placement_pending = Some(receiver);
                        } else {
                            planning.diagnostic = Some("Build placement session disconnected".into());
                            planning.dirty = true;
                        }
                    }
                    if planning.pointer_click_cue() && let Err(error) = view.gameplay_sound_player
                        .apply(crate::gameplay_sound::Operation::UiLightClick)
                    {
                        eprintln!("Planning button press sound: {error}");
                    }
                    if let Some(id) = control {
                        view.send_tutorial_control(id);
                    }
                }
                return;
            }
            if view.planning_pending.is_some() {
                return;
            }
            if button == winit::event::MouseButton::Left
                && let Some(menu) = &self.menu
            {
                let (consumed, category) = view.auction_notice.mouse(
                    menu.auction_notice_assets(),
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    pressed,
                );
                if consumed {
                    if let Some(category) = category
                        && let Some(category) = self
                            .assets
                            .selection
                            .blocks
                            .iter()
                            .position(|block| block.id == category as usize)
                    {
                        view.focus_category(category, &self.assets.selection);
                    }
                    view.mouse_buttons = [false; 2];
                    view.double_click.clear();
                    return;
                }
            }
            if button==winit::event::MouseButton::Left
                &&let Some(policy)=self.menu.as_ref().and_then(|menu|
                    view.toolbar_policy(menu,&self.assets.selection.blocks,true))
            {
                let (consumed,action)=view.toolbar_input.mouse(policy,view.toolbar_bounds,
                    view.pointer.map(|p|p.map(|v|v as f32*0.5)),pressed);
                if let Some(action)=action {
                    use crate::locator::toolbar_collapse::Action;
                    // Original Info/lease reveal BEFORE dispatching their modes.
                    action.reveal(&mut view.details_visible);
                    view.utility_report.select(Some(policy.category));
                    match action {
                        Action::Info=>{
                            view.utility_report.close_property_offer();
                            view.utility_report.close_property_rights();
                            view.send_tutorial_control(9);
                        }
                        Action::Lease=>{
                            if !view.utility_report.property_offer_open() {
                                view.utility_report.open_property_offer(policy.category);
                            }
                            view.send_tutorial_control(10);
                        }
                        Action::Build=>view.begin_toolbar_planning(policy.category),
                    }
                    view.title_state=None;
                    view.utility_report.dirty=true;
                }
                if consumed {
                    view.mouse_buttons[0]=false;view.double_click.clear();return;
                }
            }
            if !view.strategic
                && view.main_panel_visible()
                && view.details_visible
                && view._fresh_session.is_some()
                && view.ordinary_toolbar_category(&self.assets.selection.blocks).is_none()
                && view.selected.is_some_and(|s| s.building.is_none())
                && !view.selected.is_some_and(|s| {
                    view.live_status.as_ref().is_some_and(|status| {
                        status
                            .auction_locked_categories
                            .contains(&self.assets.selection.blocks[s.category].id)
                    })
                })
                && let Some(menu) = &self.menu
            {
                let [l, t, r, b] = menu.planning_assets().entry_bounds;
                let over = view.pointer.is_some_and(|p| {
                    let [x, y] = p.map(|v| v as f32 * 0.5);
                    x >= l && x <= r && y >= t && y <= b
                });
                if button == winit::event::MouseButton::Left && (over || view.planning_pressed) {
                    if pressed {
                        view.planning_pressed = over;
                    } else {
                        let activate = view.planning_pressed && over;
                        view.planning_pressed = false;
                        if activate {
                            let category =
                                self.assets.selection.blocks[view.selected.unwrap().category].id;
                            let (reply, result) = mpsc::channel();
                            if view
                                ._fresh_session
                                .as_ref()
                                .unwrap()
                                .send(FreshRequest::Planning { category, reply })
                                .is_ok()
                            {
                                view.planning_pending = Some(result);
                                view.send_tutorial_control(11);
                            }
                        }
                    }
                    view.mouse_buttons = [false; 2];
                    view.double_click.clear();
                    return;
                }
            }
            if view.main_panel_visible()
                && view.details_visible
                && !view.strategic
                && view.adjustment.is_none()
                && view.operations.building.is_some()
                && let Some(menu) = &self.menu
                && (view.operations.captured()
                    || view.pointer.is_some_and(|p| {
                        menu.business_assets().contains(p.map(|v| v as f32 * 0.5))
                    }))
            {
                if button == winit::event::MouseButton::Left {
                    view.operations.mouse(
                        menu.business_assets(),
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                        pressed,
                    );
                    if let Some((control, event)) = view.operations.take_tutorial_producer() {
                        view.send_tutorial_producer(control, event);
                    }
                    if let Some(id) = view.operations.take_tutorial_control()
                        && view.live_status.as_ref().is_some_and(|s| s.tutorial_enabled_controls.contains(&id)) {
                        view.send_tutorial_control(id);
                    }
                }
                view.mouse_buttons = [false; 2];
                return;
            }
            if let Some(adjustment) = &mut view.adjustment {
                if let Some(menu) = &self.menu {
                    adjustment.pointer(menu.adjustment_assets(), view.pointer);
                }
                let close = button == winit::event::MouseButton::Left && adjustment.mouse(pressed);
                let requested = adjustment.requested_building.take();
                let tutorial_control = adjustment.take_tutorial_control();
                if close {
                    view.adjustment = None;
                } else if let Some(building) = requested
                    && let Some(menu) = &self.menu
                {
                    match menu
                        .adjustment_assets()
                        .navigate(&view.scene, adjustment, building)
                    {
                        Ok(panel) => {
                            let b = &view.scene.inspection_buildings[building];
                            let Some(hit) = inspection_hit(&self.assets.selection.blocks,
                                b.category, Some(building),
                            ) else { return; };
                            if view.camera_producer.is_some() {
                                if let Err(error)=view.queue_actor_camera(hit,&self.assets.selection,camera_producer::ActorOrigin::Adjustment,Some(panel),false){view.save_dialog.message=Some(error);}
                                if let Some(id)=tutorial_control {view.send_tutorial_control(id);}
                                return;
                            }
                            view.adjustment = Some(panel);
                            view.selected = Some(hit);
                            if let Some((controls, angle)) = crate::navigation::building_entry(
                                self.assets.selection.blocks[hit.category].center,
                                [b.translation[0], b.translation[2]],
                                b.camera_entrance,
                                b.kind == 2,
                                view.controls.angle,
                                view.building_angle,
                            ) {
                                view.controls = controls;
                                view.building_angle = angle;
                                view.building_camera = true;
                            } else {
                                view.building_camera = false;
                                view.controls.center = self.assets.selection.center(hit.category);
                                view.controls.height = 120.;
                                view.controls.radius = 80.;
                            }
                        }
                        Err(error) => {
                            eprintln!("Business Adjustment navigation unavailable: {error}")
                        }
                    }
                }
                if let Some(id) = tutorial_control
                    && view.live_status.as_ref().is_some_and(|s| s.tutorial_enabled_controls.contains(&id)) {
                    view.send_tutorial_control(id);
                }
                return;
            }
            if view.statistics_visible()
                && let (Some(comparison), Some(menu)) = (&mut view.comparison, &self.menu)
            {
                comparison.pointer(menu.comparison_assets(), view.pointer);
                if view.pointer.is_some_and(|p| comparison.contains(p)) || comparison.pressed() {
                    if button == winit::event::MouseButton::Left {
                        let activated = comparison.mouse(pressed);
                        if let Some(id) = activated.and_then(|button| [76, 77, 78].get(button).copied())
                            && view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        {
                            view.send_tutorial_control(id);
                        }
                        view.mouse_buttons[0] = false;
                        view.double_click.clear();
                    } else if button == winit::event::MouseButton::Right {
                        comparison.cancel_press();
                        view.mouse_buttons[1] = false;
                        view.drag = [0; 2];
                    }
                    return;
                }
            }
            if event_list_occupied && !(view.strategic
                && view.locator.as_ref().is_some_and(|locator| {
                        locator.pressed()
                    || self.menu.as_ref().is_some_and(|menu| {
                                view.pointer.is_some_and(|point| {
                                    menu.locator_assets().contains(locator,point)
                                })
                            })
                    })) {
                view.mouse_buttons=[false;2];view.drag=[0;2];view.double_click.clear();return;
            }
            view.locator_blocked = view.strategic
                && (view.ui_graph.is_none()||view.graph_input(camera_live::ui_graph::PanelId::C))
                && view.locator.as_ref().is_some_and(|locator| {
                    self.menu.as_ref().is_some_and(|menu| {
                        view.pointer
                            .is_some_and(|p| menu.locator_assets().contains(locator, p))
                    })
                });
            match button {
                winit::event::MouseButton::Left => {
                    if view.mouse_buttons[0] != pressed && view.mouse_buttons[1] {
                        // Do not reinterpret accumulated orbit deltas as pan, or vice versa.
                        view.drag = [0; 2];
                    }
                    view.mouse_buttons[0] = pressed;
                    if view.strategic
                        && !view.mouse_buttons[1]
                        && (view.locator_blocked
                            || view.locator.as_ref().is_some_and(|l| l.pressed()))
                        && let (Some(locator), Some(menu)) = (&mut view.locator, &self.menu)
                    {
                        view.double_click.clear();
                        let selected = locator.mouse(
                            menu.locator_assets(), &view.scene, view.pointer, pressed,
                        );
                        let tutorial_control = locator.take_tutorial_control();
                        if let Some(id) = tutorial_control
                            && view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&id)
                            })
                        {
                            view.send_tutorial_control(id);
                        }
                        if let Some(i) = selected {
                            let building = &view.scene.inspection_buildings[i];
                            let Some(category) = self
                                .assets
                                .selection
                                .blocks
                                .iter()
                                .position(|b| b.id == building.category)
                            else {
                                return;
                            };
                            let hit=crate::selection::Hit{category,building:Some(i)};
                            if view.camera_producer.is_some() {
                                if let Err(error)=view.queue_actor_camera(hit,&self.assets.selection,camera_producer::ActorOrigin::Locator,None,false){view.save_dialog.message=Some(error);}
                                return;
                            }
                            view.selected = Some(hit);
                            let close_up = crate::navigation::building_entry(
                                self.assets.selection.blocks[category].center,
                                [building.translation[0], building.translation[2]],
                                building.camera_entrance,
                                building.kind == 2,
                                view.controls.angle,
                                view.building_angle,
                            );
                            if let Some((controls, angle)) = close_up {
                                view.controls = controls;
                                view.building_angle = angle;
                                view.building_camera = true;
                            } else {
                                view.controls = crate::navigation::block_entry(
                                    self.assets.selection.center(category),
                                );
                                view.building_camera = false;
                            }
                            view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                view.camera.eye,
                                view.camera.target,
                            );
                            view.strategic = false;
                            view.return_camera = None;
                            view.reframe_pending = true;
                            view.title_state = None;
                            if let Some(comparison) = &mut view.comparison {
                                comparison.cancel_press();
                            }
                            view.keys = [false; 4];
                            view.taps = [false; 4];
                            view.drag = [0; 2];
                            view.hovered = None;
                            eprintln!("Locator saved business -> building {i}, paused inspection");
                        }
                        return;
                    }
                    if view.ui_graph.is_none()&&pressed&&view.over_navigator()&&!view.mouse_buttons[1] {
                        view.navigator_pressed = true;
                        view.double_click.clear();
                        return;
                    }
                    if view.ui_graph.is_none()&&!pressed&&view.navigator_pressed {
                        view.navigator_pressed = false;
                        if view.over_navigator()
                            && let Some(point) = view.pointer
                            && let Some((category, _)) = self
                                .assets
                                .navigator
                                .pick_inspection(&self.assets.selection.blocks, point)
                        {
                            if view.camera_producer.is_some() {
                                let event=camera_producer::Event::Block{category:self.assets.selection.blocks[category].id as u32,
                                    selected:crate::selection::Hit{category,building:None},origin:camera_producer::BlockOrigin::Navigator};
                                let post=view.camera_control_post(103);
                                match view.queue_camera_event(event,None,None,false){Ok(pending)=>pending.posts.extend(post),Err(error)=>view.save_dialog.message=Some(error)}
                                return;
                            }
                            view.selected = Some(crate::selection::Hit {
                                category,
                                building: None,
                            });
                            if view.strategic {
                                view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                    view.camera.eye,
                                    view.camera.target,
                                );
                            }
                            view.hovered = None;
                            view.controls.center = self.assets.selection.center(category);
                            view.controls.height = 120.;
                            view.controls.radius = 80.;
                            view.building_camera = false;
                            view.strategic = false;
                            view.return_camera = None;
                            view.reframe_pending = true;
                            view.title_state = None;
                            view.keys = [false; 4];
                            view.taps = [false; 4];
                            view.drag = [0; 2];
                            eprintln!(
                                "Navigator category {} -> mode12",
                                self.assets.selection.blocks[category].id
                            );
                            if view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&103)
                            }) {
                                view.send_tutorial_control(103);
                            }
                        }
                        return;
                    }
                    if !view.strategic
                        && view.main_panel_visible()
                        && view.details_visible
                        && view.utility_category(&self.assets.selection.blocks).is_some()
                        && let Some(menu) = &self.menu
                    {
                        view.utility_report.select(view.utility_category(&self.assets.selection.blocks));
                        menu.locator_assets()
                            .utility_pointer(&mut view.utility_report, view.pointer);
                        let category = view.utility_category(&self.assets.selection.blocks).unwrap();
                        if view.ordinary_toolbar_category(&self.assets.selection.blocks).is_some() {
                            menu.locator_assets().ordinary_body_pointer(&mut view.utility_report,
                                view.pointer,view.block_information.as_ref().unwrap(),category);
                        } else {
                        menu.locator_assets().land_rent_pointer(
                            &mut view.utility_report,
                            view.pointer,
                            view.block_information.as_ref().unwrap(),
                            category,
                        );
                        }
                        let rights_enabled = view.live_status.as_ref().is_none_or(|status| {
                            !status.tutorial_session || status.tutorial_enabled_controls.contains(&91)
                        });
                        if view.utility_report.mouse_with_rights_admission(pressed, rights_enabled) {
                            let tutorial_after=view.utility_report.take_tutorial_control();
                            let camera_post=(tutorial_after==Some(6)).then(||view.camera_control_post(6)).flatten();
                            let mut hyperlink_committed = false;
                            if let Some(slot) = view.utility_report.requested_slot.take()
                                && let Some(i) =
                                    view.scene.inspection_buildings.iter().position(|b| {
                                        b.category == category
                                            && (b.saved_slot == Some(slot)
                                                || b.runtime_slot == Some(slot))
                                    })
                                && let Some(hit) = inspection_hit(&self.assets.selection.blocks,
                                    category, Some(i))
                            {
                                if view.camera_producer.is_some() {
                                    match view.queue_actor_camera(hit,&self.assets.selection,camera_producer::ActorOrigin::Hyperlink,None,false){
                                        Ok(pending)=>{pending.posts.extend(camera_post);hyperlink_committed=true;},
                                        Err(error)=>view.save_dialog.message=Some(error),
                                    }
                                } else {
                                let building = &view.scene.inspection_buildings[i];
                                view.selected = Some(hit);
                                if let Some((controls, angle)) = crate::navigation::building_entry(
                                    self.assets.selection.blocks[hit.category].center,
                                    [building.translation[0], building.translation[2]],
                                    building.camera_entrance,
                                    building.kind == 2,
                                    view.controls.angle,
                                    view.building_angle,
                                ) {
                                    view.controls = controls;
                                    view.building_angle = angle;
                                    view.building_camera = true;
                                } else {
                                    view.controls = crate::navigation::block_entry(
                                        self.assets.selection.center(hit.category),
                                    );
                                    view.building_camera = false;
                                }
                                view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                    view.camera.eye,
                                    view.camera.target,
                                );
                                view.reframe_pending = true;
                                view.title_state = None;
                                hyperlink_committed = true;
                                }
                            }
                            if let Some(id) = tutorial_after && (id != 6 || hyperlink_committed) {
                                // Only6 is conditional on the camera/UI hyperlink
                                // commit. Other handler actions remain unconditional.
                                if id!=6 || view.camera_producer.is_none(){view.send_tutorial_control(id);}
                            }
                            view.mouse_buttons[0] = false;
                            view.double_click.clear();
                            return;
                        }
                    }
                    if pressed && view.over_details() {
                        view.details_pressed = true;
                        view.double_click.clear();
                        return;
                    }
                    if !pressed && view.details_pressed {
                        view.details_pressed = false;
                        if view.over_details() {
                            view.details_visible = !view.details_visible;
                            view.toolbar_input.cancel();
                        }
                        return;
                    }
                    if pressed && view.over_navigation() {
                        view.navigation_pressed = true;
                        view.double_click.clear();
                        return;
                    }
                    if !pressed && view.navigation_pressed {
                        view.navigation_pressed = false;
                        if view.over_navigation() {
                            view.toggle_navigation(&self.assets.navigation);
                            if view.camera_producer.is_none() && view.live_status.as_ref().is_some_and(|status| {
                                status.tutorial_enabled_controls.contains(&73)
                            }) {
                                view.send_tutorial_control(73);
                            }
                        }
                        return;
                    }
                    if pressed
                        && !view.mouse_buttons[1]
                        && !view.live_status.as_ref().is_some_and(|status| status.tutorial_controls.picking_disabled)
                        && view.scene_pointer().is_some() {
                        let picked = view.scene_pointer().and_then(|point| {
                            self.assets.selection.pick(
                                view.camera,
                                &view.scene,
                                point,
                                view.selected,
                            )
                        });
                        let tutorial_pick = picked.and_then(|hit| {
                            let pick = match (view.strategic, hit.building) {
                                (false, Some(index)) => {
                                    let building = &view.scene.inspection_buildings[index];
                                    crate::tutorial_two_interaction::Pick::Building {
                                        slot: building.runtime_slot.or(building.saved_slot).unwrap_or(index),
                                    }
                                }
                                _ => crate::tutorial_two_interaction::Pick::Block {
                                    category_id: self.assets.selection.blocks[hit.category].id as u32,
                                },
                            };
                            view.live_status.as_ref().and_then(|status| {
                                status.tutorial_interaction.callback(pick).map(|_| pick)
                            })
                        });
                        if view.live_status.as_ref().is_some_and(|status|
                            status.tutorial_interaction.has_pick_gate())
                            && tutorial_pick.is_none() {
                            view.double_click.clear();
                            return;
                        }
                        let double = view.cursor.is_some_and(|point| {
                            view.double_click.press(view.started.elapsed(), point)
                        });
                        let selected = if view.strategic {
                            picked.map(|hit| crate::selection::Hit {
                                category: hit.category, building: None,
                            })
                        } else { picked };
                        let changed = view.selected != selected;
                        if !view.strategic && view.camera_producer.is_some() {
                            if let Some(hit)=selected {
                                let post=if changed{tutorial_pick.map(camera_producer::Post::TutorialPick)}else{None};
                                let open_adjustment=double
                                    && !view.live_status.as_ref().is_some_and(|s|s.tutorial_interaction.has_pick_gate())
                                    && !view.live_status.as_ref().is_some_and(|s|s.auction_locked_categories.contains(&self.assets.selection.blocks[hit.category].id));
                                let result=if hit.building.is_some(){
                                    view.queue_actor_camera(hit,&self.assets.selection,camera_producer::ActorOrigin::Scene,None,open_adjustment)
                                }else{
                                    view.queue_camera_event(camera_producer::Event::Block{category:self.assets.selection.blocks[hit.category].id as u32,
                                        selected:hit,origin:camera_producer::BlockOrigin::Selection},None,None,false)
                                };
                                match result{Ok(pending)=>pending.posts.extend(post),Err(error)=>view.save_dialog.message=Some(error)}
                                return;
                            } else {
                                if let Err(error)=view.queue_camera_event(camera_producer::Event::Deselect,None,None,false){view.save_dialog.message=Some(error);}
                                return;
                            }
                        }
                        view.selected = selected;
                        if view.strategic {
                            if changed && let Some(pick) = tutorial_pick {
                                view.send_tutorial_pick(pick);
                            }
                            if double && let Some(hit) = view.selected {
                                if view.camera_producer.is_some() {
                                    let event=camera_producer::Event::Block{category:self.assets.selection.blocks[hit.category].id as u32,
                                        selected:hit,origin:camera_producer::BlockOrigin::StrategicDoubleClick};
                                    let post=match tutorial_pick{
                                        Some(crate::tutorial_two_interaction::Pick::Block{category_id})=>Some(camera_producer::Post::TutorialPick(crate::tutorial_two_interaction::Pick::BlockEntry{category_id})),
                                        _=>None,
                                    };
                                    match view.queue_camera_event(event,None,None,false){Ok(pending)=>pending.posts.extend(post),Err(error)=>view.save_dialog.message=Some(error)}
                                    return;
                                }
                                view.building_camera = false;
                                view.controls = crate::navigation::block_entry(
                                    self.assets.selection.center(hit.category),
                                );
                                view.inertia = openmt2001::camera::BlockCameraInertia::new(
                                    view.camera.eye,
                                    view.camera.target,
                                );
                                view.strategic = false;
                                view.return_camera = None;
                                view.title_state = None;
                                view.reframe_pending = true;
                                eprintln!(
                                    "Strategic double-click: category {} -> mode12, height120/radius80",
                                    self.assets.selection.blocks[hit.category].id
                                );
                                if let Some(crate::tutorial_two_interaction::Pick::Block {
                                    category_id,
                                }) = tutorial_pick {
                                    view.send_tutorial_pick(
                                        crate::tutorial_two_interaction::Pick::BlockEntry {
                                            category_id,
                                        },
                                    );
                                }
                            }
                        } else if let Some(index) = view.selected {
                            if double
                                && !view.live_status.as_ref().is_some_and(|status| {
                                    status.tutorial_interaction.has_pick_gate()
                                })
                                && !view.live_status.as_ref().is_some_and(|status| {
                                    status
                                        .auction_locked_categories
                                        .contains(&self.assets.selection.blocks[index.category].id)
                                })
                                && let (Some(building), Some(menu)) = (index.building, &self.menu)
                            {
                                match menu.adjustment_assets().open_for_owner(&view.scene, building,
                                    view.live_status.as_ref().and_then(|s| s.business_owner),
                                ) {
                                    Ok(panel) => view.adjustment = panel,
                                    Err(error) => {
                                        eprintln!("Business Adjustment unavailable: {error}")
                                    }
                                }
                                view.keys = [false; 4];
                                view.taps = [false; 4];
                                view.mouse_buttons = [false; 2];
                                view.drag = [0; 2];
                            }
                            let close_up = index.building.and_then(|i| {
                                let building = &view.scene.inspection_buildings[i];
                                crate::navigation::building_entry_with_options(
                                    self.assets.selection.blocks[index.category].center,
                                    [building.translation[0], building.translation[2]],
                                    building.camera_entrance,
                                    building.kind == 2,
                                    view.controls.angle,
                                    view.building_angle,
                                    self.assets.selection.center(index.category),
                                    view.settings_menu.gameplay.auto_rotate,
                                )
                            });
                            if let Some((controls, angle)) = close_up {
                                view.controls = controls;
                                view.building_angle = angle;
                                view.building_camera = true;
                                eprintln!(
                                    "Building selection -> mode13 camera; Auto Rotate {}",
                                    view.settings_menu.gameplay.auto_rotate
                                );
                            } else {
                                if index.building.is_some() {
                                    eprintln!(
                                        "Building close-up unresolved: no original entrance; retaining parent block camera"
                                    );
                                }
                                view.building_camera = false;
                                view.controls.center = self.assets.selection.center(index.category);
                                view.controls.height = 120.;
                                view.controls.radius = 80.;
                            }
                            if changed && let Some(pick) = tutorial_pick {
                                view.send_tutorial_pick(pick);
                            }
                        }
                    }
                }
                winit::event::MouseButton::Right => {
                    view.mouse_buttons[1] = pressed;
                    if pressed {
                        if let Some(locator) = &mut view.locator {
                            locator.cancel_press();
                        }
                        view.navigator_pressed = false;
                        view.double_click.clear();
                    }
                }
                _ => (),
            }
        }
    }
    pub fn inspection_unfocus(&mut self) {
        if let Some(view) = &mut self.inspection {
            view.toolbar_input.cancel();
            view.toolbar_right_held = false;
            view.settings_menu.cancel_input();
            if let Some(comparison) = &mut view.comparison {
                comparison.cancel_press();
            }
            if let Some(adjustment) = &mut view.adjustment {
                adjustment.cancel_press();
            }
            if let Some(locator) = &mut view.locator {
                locator.cancel_press();
            }
            view.keys = [false; 4];
            view.navigation_pressed = false;
            view.details_pressed = false;
            view.navigator_pressed = false;
            view.double_click.clear();
            view.stats_panel.cancel_input();
            view.time_panel.cancel_input();view.time_pressed=None;
            view.taps = [false; 4];
            view.mouse_buttons = [false; 2];
            view.drag = [0; 2];
            view.cursor = None;
        }
    }
    fn refresh_inspection_projection(&mut self) -> Result<(), String> {
        if let Some(view) = &mut self.inspection {
            if !view.reframe_pending { return Ok(()); }
            let mut changed = view.scene.reframe(view.camera)?;
            if let Some(trees) = &mut view.trees {
                changed.extend(trees.reframe(&mut view.scene, view.camera)?);
            }
            changed.extend(view.detail_visibility.reframe(&mut view.scene, view.camera,
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
            let changed = normal_city_scene_updates(&mut view.scene, view.camera, star_detail, changed)?;
            for index in changed {
                self.renderer.update_dynamic_vertices(&mut view.prepared, index,
                    &view.scene.draws[index].borrowed(),
                )?;
            }
            let cameras = view.scene.draws.iter().map(|draw|
                draw.view_projection.unwrap_or(view.scene.camera)).collect::<Vec<_>>();
            self.renderer.update_cameras(&view.prepared, &cameras, view.scene.fog)?;
            view.reframe_pending = false;
        }
        Ok(())
    }
    fn inspection_update(&mut self) -> Result<(), String> {
        if self.inspection.as_ref().is_some_and(|view| view.camera_ui.is_some()) {
            return self.refresh_inspection_projection();
        }
        if let Some(view) = &mut self.inspection {
            if !view.camera_turns.is_empty() { return Ok(()); }
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
                if !view.camera_producer.as_ref().map_or(!view.strategic,|p|p.legacy_callback_enabled(view.strategic)) {
                    view.tick = tick + 1;
                    view.drag = [0; 2];
                    break;
                }
                // Preserve short host key taps until one original callback consumes them.
                let keys = std::array::from_fn(|i| view.keys[i] || view.taps[i]);
                // Sample the SAME process word once for this combined callback.
                let low_height=self.assets.process_runtime.performance_nonzero().map_err(|e|e.to_string())?;
                let buttons=if self.process_frontend.is_some(){self.camera_physical.snapshot()?.1}else{view.mouse_buttons};
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
                        view.settings_menu.gameplay.invert_vertical, low_height,
                    )?
                } else {
                    mode12_input_tick_with_height(&mut view.controls, view.saved_camera, keys, view.drag,
                        i32::from(view.settings_menu.gameplay.mouse_sensitivity),
                        view.settings_menu.gameplay.invert_vertical, low_height,
                    )?
                };
                view.drag = [0; 2];
                let (eye, target) = pose;
                view.taps = [false; 4];
                view.inertia.tick(eye, target);
                if self.process_frontend.is_some(){self.camera_physical.consumed();}
                view.tick += 1;
            }
            // Original4b9c94 samples4e0590 using the camera scheduler's remainder
            // fraction (4eb7f0). Host monotonic time supplies the50Hz interval.
            if view.camera_producer.as_ref().map_or(!view.strategic,|p|p.legacy_callback_enabled(view.strategic)) {
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
            changed.extend(view.detail_visibility.reframe(&mut view.scene, view.camera,
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
            let changed = normal_city_scene_updates(&mut view.scene, view.camera, star_detail, changed)?;
            for index in changed {
                self.renderer.update_dynamic_vertices(&mut view.prepared, index,
                    &view.scene.draws[index].borrowed(),
                )?;
            }
            let cameras = view.scene.draws.iter().map(|draw|
                draw.view_projection.unwrap_or(view.scene.camera)).collect::<Vec<_>>();
            self.renderer.update_cameras(&view.prepared, &cameras, view.scene.fog)?;
        }
        Ok(())
    }
    fn draw_result(&mut self, presenter: &Presenter) -> Result<bool, String> {
        let Some(view) = self.inspection.as_mut() else { return Ok(false);
        };
        if view.result.is_none() && view.result_pending.is_none()
            && view.live_status.as_ref().is_some_and(|status| status.result_request.is_some()) {
            let sender = view._fresh_session.as_ref().ok_or("Result has no retained worker")?;
            let (reply, answer) = mpsc::channel();
            sender.send(FreshRequest::ResultState { reply }).map_err(|e| e.to_string())?;
            view.result_pending = Some(answer);
        }
        if let Some(receiver) = &view.result_pending {
            match focus::poll_result(receiver,view.music_admission.as_ref()) {
                Ok(result) => {
                    view.result_pending = None;
                    let snapshot = result?;
                    let state = snapshot.presentation;
                    view.environment_pending = None;
                    view.environment_tick = snapshot.tick;
                    if view.environment_time != Some((snapshot.hour, snapshot.counter)) {
                        let mut changed = view.scene.advance_daylight(
                            snapshot.hour, snapshot.counter, view.camera,
                        )?;
                        if let Some(trees) = &mut view.trees {
                            changed.extend(trees.reframe(&mut view.scene, view.camera)?);
                        }
                        for index in changed {
                            let draw = &view.scene.draws[index];
                            self.renderer.update_dynamic_surface(
                                &mut view.prepared, index, &draw.borrowed(),
                                draw.view_projection.unwrap_or(view.scene.camera), view.scene.fog,
                            )?;
                        }
                        for (index, draw) in view.scene.draws.iter().enumerate() {
                            self.renderer.update_camera(&view.prepared, index,
                                draw.view_projection.unwrap_or(view.scene.camera), view.scene.fog,
                            )?;
                        }
                        view.environment_time = Some((snapshot.hour, snapshot.counter));
                    }
                    let flyby = state.flyby.as_ref().ok_or("Result script did not set flyby")?;
                    let name = flyby.rsplit('/').next().ok_or("Result flyby has no filename")?;
                    let path = self.assets.inputs.installer.join("Standard/Cut_scenes").join(name);
                    let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                    let track = Flyby::parse(&bytes)?;
                    let samples = (1..track.frame_count()).map(|frame| track.sample(frame))
                        .collect::<Result<Vec<_>, _>>()?;
                    // 4b3230 submits each owned block's four token quads during
                    // the same city draw as the result flyby. Keep their scene
                    // depth and camera, ahead of the actor/title passes.
                    let medallions = self.assets.result_medallions.draws(
                        &self.assets.selection.blocks, &state)?;
                    let fireworks = match (state.fireworks, state.fireworks_seed) {
                        (Some(setting), Some(seed)) => {
                            let image = openmt2001::pe::PeImage::parse(&self.assets.inputs.executable)?;
                            Some(crate::fresh_city::result::fireworks::Fireworks::new(
                                &image, setting, seed,
                            )?)
                        }
                        (None, None) => None,
                        _ => return Err("result firework setting and seed differ".into()),
                    };
                    let fireworks_draw = fireworks.as_ref().map(|_| view.scene.draws.len() + medallions.len());
                    if !medallions.is_empty() || fireworks.is_some() {
                        view.scene.draws.extend(medallions);
                        if fireworks.is_some() {
                            view.scene.draws.push(
                                self.assets.result_firework_texture.empty_draw());
                        }
                        let draws = view.scene.draws.iter().map(|draw| draw.borrowed())
                            .collect::<Vec<_>>();
                        view.prepared = self.renderer.prepare(&draws,
                            view.scene.camera, view.scene.fog)?;
                    }
                    let (hour, counter) = view.environment_time
                        .ok_or("result city lacks retained environment time")?;
                    let actor_light = openmt2001::lighting::SunColors::parse(
                        &self.assets.inputs.suncols)?.sample(hour, counter)?;
                    let actor_poses = crate::auction_scene::result_layout(state.characters)?;
                    let actor_draws = self.assets.result_actors.draws_result(
                        &actor_poses, &actor_light, view.camera.eye,
                    )?;
                    let actors = self.renderer.prepare(
                        &actor_draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>(),
                        view.scene.camera, view.scene.fog,
                    )?;
                    let mut controller = crate::result_controller::Controller::new(
                        state.bypass_enabled, state.credits, state.credit_rows,
                    );
                    controller.set_identifier(state.cut_scene_identifier);
                    let music_address = view.live_status.as_ref()
                        .and_then(|status| status.result_request)
                        .ok_or("Result request disappeared before music selection")?.music_address;
                    view.era_audio_player.apply(crate::era_audio::Operation::Stop)?;
                    let music = match self.assets.result_audio.play_with_audio(music_address, &self.process_frontend.as_ref().ok_or("Result requires process audio owner")?.borrow().audio) {
                        Ok(playing) => Some(playing),
                        Err(error) => { eprintln!("{error}"); None }
                    };
                    view.result = Some(ResultView { state, fireworks, fireworks_draw,
                        fireworks_ticks: 0, _music: music, samples, started: Instant::now(),
                        phase: ResultPhase::Flyby, controller, serviced_ticks: 0,
                        bypass_input: false, pending: None, credit_pending: None,
                        cutscene_pending: None, completed: None,
                        actor_light, actors, actor_draws: actor_draws.len(), title: None,
                    });
                    view.keys = [false; 4];
                    view.taps = [false; 4];
                    view.drag = [0; 2];
                    view.mouse_buttons = [false; 2];
                }
                Err(mpsc::TryRecvError::Empty) => return Ok(true),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result worker disconnected".into());
                }
            }
        }
        let Some(result) = view.result.as_mut() else { return Ok(false);
        };
        let service_tick = (result.started.elapsed().as_secs_f64() * 50.) as usize;
        if let Some(fireworks) = &mut result.fireworks {
            while result.fireworks_ticks < service_tick {
                result.fireworks_ticks += 1;
                fireworks.tick()?;
                for cue in fireworks.take_sounds() {
                    view.gameplay_sound_player.apply(
                        crate::gameplay_sound::Operation::ResultFirework(cue))?;
                }
            }
        }
        while result.serviced_ticks < service_tick && result.cutscene_pending.is_none()
            && result.credit_pending.is_none() {
            result.serviced_ticks += 1;
            let ended = result.controller.tick(std::mem::take(&mut result.bypass_input));
            if result.controller.take_credit_construction_edge() {
                let sender = view._fresh_session.as_ref().ok_or("Result worker missing at credits edge")?;
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::ResultCredits { reply }).map_err(|e| e.to_string())?;
                result.credit_pending = Some(answer);
            }
            if ended {
                let sender = view._fresh_session.as_ref().ok_or("Result worker missing at cutscene end")?;
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::ResultEvent { name: "CutSceneEnded", reply,
                    })
                    .map_err(|e| e.to_string())?;
                result.cutscene_pending = Some(answer);
            }
        }
        if let Some(receiver) = &result.credit_pending {
            match receiver.try_recv() {
                Ok(state) => {
                    result.state = state?;
                    result.credit_pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result credits worker disconnected".into());
                }
            }
        }
        if matches!(result.phase, ResultPhase::Flyby) {
            let tick = (result.started.elapsed().as_secs_f64() * 50.) as usize;
            let sample = result.samples[tick.min(result.samples.len() - 1)];
            if view.camera != sample {
                view.camera = sample;
                if let Some(producer) = &mut view.camera_producer {
                    let _ = producer.commit(sample, if view.strategic {3} else {0});
                }
                let mut changed = view.scene.reframe(sample)?;
                if let Some(trees) = &mut view.trees {
                    changed.extend(trees.reframe(&mut view.scene, sample)?);
                }
                let draw_distance = u32::from(view.settings_menu.graphics.distance);
                changed.extend(view.detail_visibility.reframe(&mut view.scene, sample,
                    view.settings_menu.graphics.distance,
                )?);
                let residents = if let Some(people) = &view.people {
                    people.reframe(&mut view.scene, sample, draw_distance)?
                } else { 0..0 };
                changed.extend(residents);
                if let Some(vehicles) = &view.vehicles {
                    changed.extend(vehicles.reframe(&mut view.scene, sample, draw_distance)?);
                }
                if let Some(transport) = &view.transport {
                    changed.extend(transport.reframe(&mut view.scene, sample, draw_distance)?);
                }
                for (i, draw) in view.scene.draws.iter().enumerate() {
                    if changed.contains(&i) {
                        self.renderer.update_dynamic_surface(&mut view.prepared, i,
                            &draw.borrowed(), draw.view_projection.unwrap_or(view.scene.camera), view.scene.fog,
                        )?;
                    } else {
                        self.renderer.update_camera(&view.prepared, i,
                            draw.view_projection.unwrap_or(view.scene.camera), view.scene.fog,
                        )?;
                    }
                }
                let actor_poses = crate::auction_scene::result_layout(result.state.characters)?;
                let actor_draws = self.assets.result_actors.draws_result(
                    &actor_poses, &result.actor_light, sample.eye,
                )?;
                if actor_draws.len() != result.actor_draws {
                    return Err("result actor draw count changed during flyby".into());
                }
                for (i, draw) in actor_draws.iter().enumerate() {
                    self.renderer.update_surface(&mut result.actors, i, &draw.borrowed(),
                        view.scene.camera, view.scene.fog,
                    )?;
                }
            }
            if tick >= result.samples.len() && result.pending.is_none() {
                let sender = view._fresh_session.as_ref().ok_or("Result worker missing at flyby end")?;
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::ResultEvent { name: "FlybyEnded", reply,
                    })
                    .map_err(|e| e.to_string())?;
                result.pending = Some(answer);
            }
            if let Some(receiver) = &result.pending {
                match receiver.try_recv() {
                    Ok(state) => {
                        result.state = state?;
                        result.controller.set_identifier(result.state.cut_scene_identifier);
                        result.controller.set_bypass(result.state.bypass_enabled);
                        result.pending = None;
                        result.phase = ResultPhase::CutScene;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("Result event worker disconnected".into());
                    }
                }
            }
        }
        if let Some(receiver) = &result.cutscene_pending {
            match receiver.try_recv() {
                Ok(state) => {
                    result.state = state?;
                    result.cutscene_pending = None;
                    if result.state.end_requested.is_none() {
                        return Err("Result CutSceneEnded did not request Game.End".into());
                    }
                    let sender = view._fresh_session.as_ref().ok_or("Result worker missing at completion")?;
                    let (reply, answer) = mpsc::channel();
                    sender.send(FreshRequest::ResultComplete { reply }).map_err(|e| e.to_string())?;
                    result.completed = Some(answer);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result event worker disconnected".into());
                }
            }
        }
        if let Some(receiver) = &result.completed {
            match receiver.try_recv() {
                Ok(sequence) => {
                    let _ = sequence?;
                    self.retire_inspection();
                    if let Some(menu) = &mut self.menu { menu.refresh_campaign()?; }
                    return Ok(false);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Result completion worker disconnected".into());
                }
            }
        }
        if let (Some(fireworks), Some(index)) = (&result.fireworks, result.fireworks_draw) {
            let draw = &mut view.scene.draws[index];
            crate::fresh_city::result::particles_gpu::fill(draw, fireworks.points(), view.camera)?;
            self.renderer.update_surface(
                &mut view.prepared, index, &draw.borrowed(), view.scene.camera, None,
            )?;
        }
        update_city_pulse(&self.assets.city_pulse, &self.renderer, view.pulse_draw,
            view.pulse_state.as_ref(), view.camera, &mut view.scene, &mut view.prepared,
        )?;
        self.renderer.draw_prepared(&self.color_view, &self.depth_view, &view.prepared);
        self.renderer.draw_world_overlay(&self.color_view, &self.depth_view, &result.actors);
        let title_id = result.controller.title_visible().then(|| result.state.text_id()).flatten();
        let credit_row = result.controller.credit_row();
        let text_alpha = result.controller.alpha();
        if (title_id.is_none() && credit_row.is_none()) || text_alpha == 0 {
            result.title = None;
        } else if result.title.as_ref().is_none_or(|(id, row, alpha, _)| {
            *id != title_id || *row != credit_row || *alpha != text_alpha
        }) {
            let scene = if let Some(row) = credit_row {
                self.assets.result_text.prepare_credit(
                    &self.renderer, &result.state, row, text_alpha,
                )?
            } else {
                self.assets.result_text.prepare(
                    &self.renderer, &result.state, text_alpha)?
            };
            result.title = scene.map(|scene| (title_id, credit_row, text_alpha, scene));
        }
        if let Some((_, _, _, title)) = &result.title {
            self.renderer.draw_overlay(&self.color_view, &self.depth_view, title);
        }
        presenter.present_binding(&self.binding).map_err(|e| e.to_string())?;
        Ok(true)
    }

    pub fn draw(&mut self, presenter: &Presenter) -> Result<(), String> {
        // App serviced focus and fed admitted physical samples at redraw entry.
        self.service_camera_turns();
        if let Some(worker)=&mut self.camera_worker {
            if let Some(result)=worker.poll(){
                let custody=worker.take_custody();
                self.camera_worker=None;
                let result=result.and_then(|()| {
                    if let Some(process)=&self.process_frontend {
                        process.borrow_mut().return_custody(custody.ok_or("Joined worker returned no custody")?)?;
                    }
                    Ok(())
                });
                if let Err(error)=result{
                    if let Some(process)=&self.process_frontend {process.borrow_mut().fail(error.clone());}
                    self.camera_worker_failure=Some(error.clone());
                    self.loading=None;
                    self.retire_inspection();
                    eprintln!("City worker failed; replacement startup blocked: {error}");
                    if let Some(menu)=&mut self.menu{
                        menu.load.message=Some(error.clone());menu.single.message=Some(error.clone());menu.skirmish.error=Some(error);
                        menu.load.busy=false;menu.single.busy=false;menu.invalidate();
                    }
                }
            }
        }
        let camera_start_ready=self.process_start_ready();
        if !camera_start_ready && let Some(menu)=&mut self.menu {
            if menu.load.request.is_some(){menu.load.message=Some(self.camera_worker_failure.clone().unwrap_or_else(||"Finishing previous city before loading...".into()));}
            if menu.single.request.is_some(){menu.single.message=Some(self.camera_worker_failure.clone().unwrap_or_else(||"Finishing previous city before starting...".into()));}
            // Skirmish request/busy remains retained; its ordinary start error
            // is surfaced by finish on a terminal failure, not fake completion.
            if menu.skirmish.request.is_some() && let Some(error)=&self.camera_worker_failure{menu.skirmish.finish(Some(error.clone()));}
        }
        self.commit_menu_settings()?;
        // A host worker reuses the same original byte inputs and shared assembler.
        // Native startup enables gameplay; Space controls the host pause gate.
        // The worker retains its Lua VM and world until inspection is dismissed.
        if !self.refreshing_fresh
            && self.menu.as_ref().is_some_and(|m| {
                if self.loading_skirmish {
                    !m.skirmish.busy
                } else if self.loading_from_setup {
                    !m.single.visible || !m.single.busy
                } else {
                    !m.load.visible
                }
            })
        {
            self.loading = None;
        }
        if self.loading.is_none() && self.inspection.is_some()
            && self.refresh_retry.due(Instant::now())
        {
            if let Err(error) = self.refresh_fresh_city() {
                self.refresh_retry.reject(error);
            }
        }
        // Normal level startup4b9fa0->4b9d40 writes this mode6 overview
        // at4b9e5f..91. Represent the same pose in retained city controls;
        // clicking a block then uses the original120/80 dimensions.
        let (mut inspection_camera, mut inspection_controls) = fresh_city_camera();
        if self.refreshing_fresh
            && let Some(view) = &self.inspection
        {
            inspection_controls = view.controls;
            inspection_camera = view.camera;
        }
        if self.process_start_ready() && self.loading.is_none()
            && let Some(path) = self.menu.as_mut().and_then(|m| m.load.request.take())
        {
            self.loading_skirmish = false;
            self.loading_from_setup = false;
            let save_profile_root = self.menu.as_ref().and_then(|m| m.current_save_root());
            let pavement=self.assets.selection.pavement_resource().binding()?.handle();
            let inputs = self.assets.inputs.clone();
            let focus_enabled = self.focus.enabled();
            let custody=self.start_custody(false)?;
            let (tx, rx) = mpsc::channel();
            self.loading = Some(rx);
            self.camera_worker=Some(camera_producer::spawn_session_joined("saved-city-simulation",inputs,
                WorkerStart::Saved(path,save_profile_root),inspection_camera,tx,Some(pavement),focus_enabled,custody)?);
        }
        if self.process_start_ready() && self.loading.is_none()
            && let Some(request) = self.menu.as_mut().and_then(|m| m.skirmish.request.take())
        {
            self.loading_skirmish = true;
            self.loading_from_setup = true;
            let inputs = self.assets.inputs.clone();
            let focus_enabled = self.focus.enabled();
            let custody=self.start_custody(true)?;
            let (tx, rx) = mpsc::channel();
            self.loading = Some(rx);
            match camera_producer::spawn_session_joined("skirmish-simulation",inputs,WorkerStart::Skirmish(request),
                inspection_camera,tx,None,focus_enabled,custody) {
                Ok(worker)=>self.camera_worker=Some(worker),
                Err(error)=>{
                self.camera_worker_failure=Some(error.clone());
                if let Some(process)=&self.process_frontend {process.borrow_mut().fail(error.clone());}
                self.loading = None;
                self.loading_skirmish = false;
                self.loading_from_setup = false;
                if let Some(menu) = self.menu.as_mut() {
                    menu.skirmish.finish(Some(error.to_string()));
                    menu.invalidate();
                }
                }
            }
        }
        if self.process_start_ready() && self.loading.is_none()
            && let Some(preview) = self.menu.as_mut().and_then(|m| m.single.request.take())
        {
            self.loading_skirmish = false;
            self.loading_from_setup = true;
            let inputs = self.assets.inputs.clone();
            let focus_enabled = self.focus.enabled();
            let custody=self.start_custody(true)?;
            let (tx, rx) = mpsc::channel();
            self.loading = Some(rx);
            self.camera_worker=Some(camera_producer::spawn_session_joined("city-simulation",inputs,
                WorkerStart::Fresh(preview),inspection_camera,tx,None,focus_enabled,custody)?);
        }
        if let Some(rx) = &self.loading {
            let result = match rx.try_recv() {
                Ok(r) => Some(r),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(e) => Some(Err(e.to_string())),
            };
            if let Some(result) = result {
                self.loading = None;
                let refreshing = self.refreshing_fresh;
                self.refreshing_fresh = false;
                let result = result.and_then(|loaded| {
                    let camera_initial = if loaded.camera_publisher.is_some() {
                        Some(camera_producer::Initial::from_loaded(&loaded, camera_producer::Controls {
                                camera: inspection_camera, controls: inspection_controls,
                                inertia: openmt2001::camera::BlockCameraInertia::new(inspection_camera.eye, inspection_camera.target),
                                saved: false, building: false, building_angle: 0., selected: None,
                            })?)
                    } else { None };
                    let (inspection_camera, inspection_controls) = camera_initial.as_ref()
                        .map(|i|(i.active.camera,i.active.controls))
                        .or(loaded.saved_viewpoint).unwrap_or((inspection_camera, inspection_controls));
                    let retained_session = loaded.fresh_session.or_else(|| {
                        if refreshing {
                            self.inspection
                                .as_ref()
                                .and_then(|view| view._fresh_session.clone())
                        } else {
                            None
                        }
                    });
                    let graphics_levels = self.inspection.as_ref().map_or(self.graphics_levels,
                        |view| view.settings_menu.graphics);
                    let star_detail = self.inspection.as_ref().map_or(Some(graphics_levels.detail),
                        Inspection::normal_star_detail);
                    let mut scene = loaded.scene;
                    let mut detail_visibility = loaded.detail_visibility;
                    if refreshing && let Some(previous) = &self.inspection {
                        scene.retain_environment(&previous.scene)?;
                    }
                    let mut environment_changed = if let Some(tick) = loaded.logic_tick {
                        scene.advance_environment(tick, inspection_camera.eye)?
                    } else {
                        Vec::new()
                    };
                    if let Some((hour, counter)) = loaded.environment_time {
                        environment_changed.extend(scene.advance_daylight(hour, counter, inspection_camera)?);
                    }
                    environment_changed.extend(loaded.updated_city_draws);
                    let environment_changed = normal_city_scene_updates(
                        &mut scene, inspection_camera, star_detail, environment_changed,
                    )?;
                    let mut trees = loaded.trees;
                    let people = loaded.people;
                    let vehicles = loaded.vehicles;
                    let transport = loaded.transport;
                    if let Some((source, year, hour)) = loaded.status {
                        eprintln!("[session] Loaded {source}: year {year}, hour {hour}");
                    }
                    if let Some(menu) = &self.menu {
                        menu.validate_inspection_names(&scene)?;
                    }
                    let pulse_draw = retained_session.as_ref().map(|_| {
                        let index = scene.draws.len();
                        scene.draws.push(self.assets.city_pulse.empty_draw());
                        index
                    });
                    let draws = scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>();
                    let profile_prepare = std::env::var_os("OPENMT2001_PROFILE_SNAPSHOTS")
                        .map(|_| Instant::now());
                    let mut prepared = self.renderer.prepare(&draws, scene.camera, scene.fog)?;
                    if let Some(started) = profile_prepare {
                        eprintln!(
                            "Snapshot frontend tick{:?} refreshing{refreshing} reuse_city{} full_prepare{} draws{} detached_prepare host elapsed {:?}; includes waits, excludes subsequent updates and GPU execution",
                            loaded.logic_tick, loaded.reuse_city,
                            true, draws.len(), started.elapsed(),
                        );
                    }
                    if refreshing && loaded.reuse_city {
                        for index in environment_changed {
                            let draw = &scene.draws[index];
                            self.renderer.update_dynamic_surface(
                                &mut prepared, index, &draw.borrowed(),
                                draw.view_projection.unwrap_or(scene.camera), scene.fog,
                            )?;
                        }
                    }
                    // A cached scene may have been created from another view.
                    // Reapply normal admission before this second upload seam.
                    if trees.is_some() || people.is_some() || vehicles.is_some() || transport.is_some() {
                        let changed = scene.reframe(inspection_camera)?;
                        let changed = normal_city_scene_updates(
                            &mut scene, inspection_camera, star_detail, changed,
                        )?;
                        for i in changed {
                            self.renderer.update_dynamic_surface(
                                &mut prepared,
                                i,
                                &scene.draws[i].borrowed(),
                                scene.camera,
                                scene.fog,
                            )?;
                        }
                        let cameras = scene.draws.iter().map(|draw|
                            draw.view_projection.unwrap_or(scene.camera)).collect::<Vec<_>>();
                        self.renderer.update_cameras(&prepared, &cameras, scene.fog)?;
                    }
                    let gameplay_levels = self.inspection.as_ref().map_or(self.gameplay_levels,
                        |view| view.settings_menu.gameplay);
                    let draw_distance = u32::from(graphics_levels.distance);
                    for i in detail_visibility.reframe(&mut scene, inspection_camera,
                        draw_distance as u8)? {
                        self.renderer.update_dynamic_vertices(&mut prepared, i,
                            &scene.draws[i].borrowed())?;
                    }
                    if let Some(trees) = &mut trees {
                        for i in trees.reframe(&mut scene, inspection_camera)? {
                            let draw = &scene.draws[i];
                            self.renderer.update_dynamic_surface(
                                &mut prepared, i, &draw.borrowed(),
                                draw.view_projection.unwrap_or(scene.camera), scene.fog,
                            )?;
                        }
                    }
                    if let Some(people) = &people {
                        for i in people.reframe(&mut scene, inspection_camera, draw_distance)? {
                            self.renderer.update_dynamic_surface(
                                &mut prepared,
                                i,
                                &scene.draws[i].borrowed(),
                                scene.camera,
                                scene.fog,
                            )?;
                        }
                    }
                    if let Some(vehicles) = &vehicles {
                        for i in vehicles.reframe(&mut scene, inspection_camera, draw_distance)? {
                            self.renderer.update_dynamic_surface(
                                &mut prepared, i, &scene.draws[i].borrowed(),
                                scene.camera, scene.fog,
                            )?;
                        }
                    }
                    if let Some(transport) = &transport {
                        for i in transport.reframe(&mut scene, inspection_camera, draw_distance)? {
                            self.renderer.update_dynamic_surface(
                                &mut prepared, i, &scene.draws[i].borrowed(),
                                scene.camera, scene.fog,
                            )?;
                        }
                    }
                    let capacity = self.assets.selection.marker_surface(
                        Some(crate::selection::Hit {
                            category: 0,
                            building: None,
                        }),
                        Some(crate::selection::Hit {
                            category: 1,
                            building: None,
                        }),
                        &scene,
                    )?;
                    let mut draw = capacity.draw();
                    draw.mode = openmt2001::render::SurfaceMode::StaticLit;
                    let markers = self.renderer.prepare(&[draw], scene.camera, scene.fog)?;
                    let mut locator = self.menu.as_ref().map(|menu| {
                        if let Some(inputs) = loaded.runtime_locator.as_ref() {
                            menu.locator_assets().runtime_state(inputs).map(Some)
                        } else {
                            menu.locator_assets().state(&scene)
                        }
                    }).transpose()?.flatten();
                    if locator.is_none() && loaded.city_people.is_some() {
                        locator = self
                            .menu
                            .as_ref()
                            .map(|m| m.locator_assets().people_state())
                            .transpose()?;
                    }
                    if let (Some(locator), Some(report)) = (&mut locator, loaded.city_people) {
                        locator.attach_people(report);
                    }
                    if let Some(locator) = &mut locator {
                        if let Some(players) = loaded.player_info {
                            locator.attach_players(players);
                        }
                        locator.refresh_scene(&scene);
                        locator.attach_polls(loaded.consumer_polls);
                        locator.attach_ownership(loaded.city_ownership);
                        locator.attach_rights(loaded.city_rights);
                        locator.attach_prestige(loaded.city_prestige);
                    }
                    let mut era_audio_player = crate::era_audio::Player::new(
                        self.assets.era_audio.clone());
                    if let Some(process)=&self.process_frontend {era_audio_player.bind_audio(process.borrow().audio.clone())?;}
                    else {era_audio_player.set_levels(self.audio_levels.music,self.audio_levels.effects);}
                    let mut auction_audio_player = crate::live_host::auction_audio::Player::new(
                        self.assets.auction_audio.clone());
                    if let Some(process)=&self.process_frontend {auction_audio_player.bind_audio(process.borrow().audio.clone());}
                    else {auction_audio_player.set_level(self.audio_levels.effects);}
                    let mut transport_audio_player = crate::transport_audio::Player::new(
                        self.assets.transport_audio.clone());
                    if let Some(process)=&self.process_frontend {transport_audio_player.bind_audio(process.borrow().audio.clone());}
                    else {transport_audio_player.set_level(self.audio_levels.effects);}
                    let mut gameplay_sound_player = crate::gameplay_sound::Player::new(
                        self.assets.gameplay_sound.clone());
                    if let Some(process)=&self.process_frontend {gameplay_sound_player.bind_audio(process.borrow().audio.clone());}
                    else {gameplay_sound_player.set_level(self.audio_levels.effects);}
                    // Native fresh startup 4b9f3f calls 4806b0 with the
                    // current era, selecting its music through 4ad6f0.
                    // Later decade changes use the same 4806b0 callback.
                    let mut settings_menu = crate::settings_menu::View::default();
                    settings_menu.graphics = graphics_levels;
                    settings_menu.gameplay = gameplay_levels;
                    let camera_inbox = loaded.camera_inbox;
                    let imported_cameras = loaded.imported_cameras;
                    let pending_music_era=loaded.music_era;
                    let mut view = Inspection {
                        music_admission:self.focus.enabled().then(||self.focus.music_admission()),
                        camera_epoch: loaded.camera_epoch,
                        camera_turns: Default::default(),
                        camera_producer: None,
                        camera_ready: None,
                        camera_endpoint: None,
                        camera_capture_pending: None,
                        camera_ui: None,
                        ui_graph:None,
                        camera_navigation:None,
                        camera_focus:camera_live::focus::Focus::default(),
                        navigation_click_latched:false,
                        save_profile_root: loaded.save_profile_root,
                        saved_camera: loaded.saved_viewpoint.is_some(),
                        result_pending: None,
                        result: None,
                        auction_requests: loaded.auction_requests,
                        auction_audio_requests: loaded.auction_audio_requests,
                        auction_audio_player,
                        transport_audio_requests: loaded.transport_audio_requests,
                        transport_audio_player,
                        era_audio_requests: loaded.era_audio_requests,
                        era_audio_player,
                        gameplay_sound_requests: loaded.gameplay_sound_requests,
                        gameplay_sound_player,
                        auction: crate::auction_bridge::View::default(),
                        auction_panel: crate::auction_panel::View::default(),
                        planning: None,
                        planning_pending: None,
                        planning_pressed: false,
                        live_status: None,
                        time_panel: crate::time_panel::View::default(),
                        cash_arrow: crate::cash_arrow::View::default(),
                        dust: crate::dust_scene::View::default(),
                        time_pressed: None,
                        game_options: crate::game_options::View::default(),
                        save_dialog: crate::save_dialog::View::default(),
                        save_pending: None,
                        options_ack: options_ack::Gate::default(),
                        save_delete_pending: None,
                        settings_menu,
                        detail_visibility,
                        pulse_draw,
                        pulse_state: None,
                        status_pending: None,
                        environment_pending: None,
                        environment_tick: loaded.logic_tick.unwrap_or(0),
                        environment_time: loaded.environment_time,
                        last_environment_query: Instant::now()
                            - std::time::Duration::from_millis(40),
                        last_status_query: Instant::now() - std::time::Duration::from_secs(1),
                        last_snapshot: Instant::now(),
                        construction_active: loaded.construction_active,
                        snapshot_tick: 0,
                        card_input: crate::card_panel::Input::default(),
                        card_selection: None,
                        prompts: crate::prompt::State::default(),
                        tutorial_message: crate::prompt::State::default(),
                        event_list: crate::event_list::View::default(),
                        stats_panel: crate::stats_panel::View::default(),
                        stats_requested: false,
                        tutorial_camera_seen: 0,
                        auction_notice: crate::auction_notice::State::default(),
                        notifications: crate::notice_panel::View::default(),
                        prompt_reply: None,
                        block_information: loaded.block_information,
                        utility_report: crate::utility_report::View::default(),
                        property_pending: None,
                        rights_pending: None,
                        comparison: loaded.comparison.map(crate::comparison::View::new),
                        trees,
                        people,
                        people_pending: None,
                        last_people_query: Instant::now(),
                        vehicles,
                        vehicles_pending: None,
                        last_vehicles_query: Instant::now(),
                        transport,
                        transport_pending: None,
                        last_transport_query: Instant::now(),
                        _fresh_session: retained_session,
                        adjustment: None,
                        operations: crate::business_view::View::default(),
                        locator,
                        locator_blocked: false,
                        information: None,
                        details_visible: true,
                        details_pressed: false,
                        toolbar_input: Default::default(),
                        toolbar_right_held: false,
                        toolbar_bounds: self.menu.as_ref().ok_or("ordinary toolbar assets missing")?
                            .locator_assets().ordinary_toolbar_bounds(),
                        toolbar_regions: [None;3],
                        detail_icons: if refreshing && self.inspection.is_some() {
                            Default::default()
                        } else { self.assets.navigation.prepare_details(&self.renderer)? },
                        detail_bounds: self.assets.navigation.detail_bounds,
                        navigator: self
                            .assets
                            .navigator
                            .prepare_with_ownership(&self.renderer, &loaded.navigator_owners)?,
                        navigator_owners: loaded.navigator_owners,
                        navigator_bounds: self.assets.navigator.bounds,
                        navigator_pressed: false,
                        tutorial_go_to_block_pressed: false,
                        tooltip: None,
                        tooltip_state: None,
                        icon_tooltip_dwell: crate::tooltip::IconDwell::default(),
                        navigation: if refreshing && self.inspection.is_some() {
                            Default::default()
                        } else { self.assets.navigation.prepare(&self.renderer)? },
                        navigation_bounds: self.assets.navigation.bounds,
                        navigation_pressed: false,
                        double_click: crate::navigation::DoubleClick::default(),
                        strategic: camera_initial.as_ref().map_or(loaded.saved_prior_mode == Some(3), |initial|initial.strategic()),
                        return_camera: None,
                        reframe_pending: false,
                        scene,
                        prepared,
                        markers,
                        camera: inspection_camera,
                        controls: inspection_controls,
                        building_angle: 0.,
                        building_camera: false,
                        inertia: openmt2001::camera::BlockCameraInertia::new(
                            inspection_camera.eye,
                            inspection_camera.target,
                        ),
                        keys: [false; 4],
                        taps: [false; 4],
                        cursor: None,
                        mouse_buttons: [false; 2],
                        drag: [0; 2],
                        pointer: None,
                        hovered: None,
                        selected: None,
                        title_state: None,
                        started: Instant::now(),
                        tick: 0,
                    };
                    let needs_ui = camera_inbox.is_some() || (refreshing && self.inspection.as_ref()
                        .is_some_and(|old| old.camera_endpoint.is_some() && old.camera_ui.is_none()));
                    if needs_ui {
                        view.camera_ui = Some(camera_live::State::new(
                            camera_live::Globals::after_normal_initialization(camera_live::OriginalGraphicsPolicy::NormalD3d,false),
                            camera_live::Cursor::new(0,0)?, None, None));
                    }
                    if camera_inbox.is_some() && imported_cameras.is_none() {
                        return Err("review channel requires imported pair".into());
                    }
                    if refreshing && camera_inbox.is_some() {
                        return Err("scene refresh must retain its actual endpoint".into());
                    }
                    let property_install = if camera_inbox.is_some() {
                        let image=openmt2001::pe::PeImage::parse(&self.assets.inputs.executable)?;
                        Some(camera_live::endpoint::PropertyInstall::capture(&self.assets.selection,&image)?)
                    } else {None};
                    let plan = if refreshing {
                        let previous = self.inspection.as_mut().ok_or("refresh has no retained inspection")?;
                        let menu = self.menu.as_ref().ok_or("refresh menu assets missing")?;
                        // Selection is a prospective presentation index, never
                        // the C stable key or raw current-selection authority.
                        view.selected = previous.selected.and_then(|hit| {
                            let building = match hit.building {
                                Some(index) => {
                                    let slot = previous.scene.inspection_buildings.get(index)?.runtime_slot?;
                                    Some(view.scene.inspection_buildings.iter()
                                        .position(|b| b.runtime_slot == Some(slot))?)
                                }
                                None => None,
                            };
                            Some(crate::selection::Hit { category: hit.category, building })
                        });
                        let inputs = refresh::Inputs {
                            previous_buildings: &previous.scene.inspection_buildings,
                            next_scene: (&view.scene).into(),
                            locator_extents: menu.locator_assets().refresh_extents(),
                            business_owner: previous.live_status.as_ref().and_then(|s| s.business_owner),
                        };
                        // Borrow distinct production fields so the exact
                        // GPU-independent transaction also serves the tests.
                        let mut old = refresh::Owners {
                            locator: &mut previous.locator, adjustment: &mut previous.adjustment,
                            graph: &mut previous.ui_graph, ui: &mut previous.camera_ui,
                            navigation: &mut previous.camera_navigation, focus: &mut previous.camera_focus,
                            endpoint: &mut previous.camera_endpoint,
                            navigation_click_latched: &mut previous.navigation_click_latched,
                        };
                        let mut next = refresh::Owners {
                            locator: &mut view.locator, adjustment: &mut view.adjustment,
                            graph: &mut view.ui_graph, ui: &mut view.camera_ui,
                            navigation: &mut view.camera_navigation, focus: &mut view.camera_focus,
                            endpoint: &mut view.camera_endpoint,
                            navigation_click_latched: &mut view.navigation_click_latched,
                        };
                        Some(refresh::prepare(&mut old, &mut next, inputs,
                            menu.adjustment_assets(), &mut self.refresh_retry)?)
                    } else { None };
                    if let Some(initial) = camera_initial {
                        initial.active.put(&mut view);
                        view.return_camera = initial.normal_return;
                        view.camera_producer = Some(camera_producer::Producer::attach(&mut self.camera_parked,
                            loaded.camera_publisher.expect("prepared unique publisher"), &initial)?);
                    }
                    Ok((view, plan, camera_inbox, property_install,pending_music_era))
                });
                if let Some(menu) = &mut self.menu {
                    match result {
                        Ok((mut view, plan, camera_inbox, property_install,pending_music_era)) => {
                            if let Some(plan) = plan {
                                let previous = self.inspection.as_mut().expect("prepared retained inspection");
                                refresh::commit(&mut refresh::Owners::of(previous),
                                    &mut refresh::Owners::of(&mut view), plan, &mut self.refresh_retry);
                                // From here through publication every operation is infallible.
                                // These five overlay states depend only on this Intro's
                                // immutable navigation assets and renderer, not city reports.
                                view.detail_icons = std::mem::take(&mut previous.detail_icons);
                                view.navigation = std::mem::take(&mut previous.navigation);
                                view.auction_requests = previous.auction_requests.take();
                                view.auction_audio_requests = previous.auction_audio_requests.take();
                                std::mem::swap(
                                    &mut view.auction_audio_player,
                                    &mut previous.auction_audio_player,
                                );
                                view.transport_audio_requests = previous.transport_audio_requests.take();
                                std::mem::swap(
                                    &mut view.transport_audio_player,
                                    &mut previous.transport_audio_player,
                                );
                                view.era_audio_requests = previous.era_audio_requests.take();
                                std::mem::swap(
                                    &mut view.era_audio_player,
                                    &mut previous.era_audio_player,
                                );
                                view.gameplay_sound_requests = previous.gameplay_sound_requests.take();
                                std::mem::swap(
                                    &mut view.gameplay_sound_player,
                                    &mut previous.gameplay_sound_player,
                                );
                                view.result=previous.result.take();
                                view.auction = std::mem::take(&mut previous.auction);
                                view.auction_panel = std::mem::take(&mut previous.auction_panel);
                                view.notifications = std::mem::take(&mut previous.notifications);
                                view.planning = previous.planning.take();
                                view.planning_pending = previous.planning_pending.take();
                                view.live_status = previous.live_status.take();
                                view.time_panel = std::mem::take(&mut previous.time_panel);
                                view.cash_arrow = std::mem::take(&mut previous.cash_arrow);
                                view.dust = std::mem::take(&mut previous.dust);
                                view.time_pressed = previous.time_pressed.take();
                                view.game_options = std::mem::take(&mut previous.game_options);
                                // A scene snapshot carries render state, not the selected
                                // profile identity. Keep the worker's writable save scope.
                                view.save_profile_root = previous.save_profile_root.take();
                                view.save_dialog = std::mem::take(&mut previous.save_dialog);
                                view.save_pending = previous.save_pending.take();
                                view.options_ack = std::mem::take(&mut previous.options_ack);
                                view.save_delete_pending = previous.save_delete_pending.take();
                                view.settings_menu = std::mem::take(&mut previous.settings_menu);
                                view.pulse_state = previous.pulse_state.take();
                                view.status_pending = previous.status_pending.take();
                                view.last_status_query = previous.last_status_query;
                                view.snapshot_tick =
                                    view.live_status.as_ref().map_or(0, |s| s.ticks);
                                view.operations = std::mem::take(&mut previous.operations);
                                view.operations.refresh = true;
                                view.card_input = std::mem::take(&mut previous.card_input);
                                view.card_selection = previous.card_selection;
                                view.prompts = std::mem::take(&mut previous.prompts);
                                view.tutorial_message = std::mem::take(&mut previous.tutorial_message);
                                view.event_list = std::mem::take(&mut previous.event_list);
                                view.stats_panel = std::mem::take(&mut previous.stats_panel);
                                view.stats_requested = previous.stats_requested;
                                view.auction_notice = std::mem::take(&mut previous.auction_notice);
                                view.prompt_reply = previous.prompt_reply.take();
                                view.cursor = previous.cursor;
                                view.pointer = previous.pointer;
                                view.mouse_buttons = previous.mouse_buttons;
                                view.camera_capture_pending = previous.camera_capture_pending.take();
                                view.keys = previous.keys;
                                view.taps = previous.taps;
                                view.drag = previous.drag;
                                view.started = previous.started;
                                view.tick = previous.tick;
                                view.navigation_pressed = previous.navigation_pressed;
                                view.navigator_pressed = previous.navigator_pressed;
                                view.details_pressed = previous.details_pressed;
                                view.strategic = previous.strategic;
                                view.return_camera = previous.return_camera.map(|mut state| {
                                    state.selected = state.selected.and_then(|hit| {
                                        let building = match hit.building {
                                            Some(index) => {
                                                let slot = previous.scene.inspection_buildings.get(index)?.runtime_slot?;
                                                Some(view.scene.inspection_buildings.iter().position(|b|b.runtime_slot==Some(slot))?)
                                            }
                                            None => None,
                                        };
                                        Some(crate::selection::Hit { category:hit.category,building })
                                    });
                                    state
                                });
                                camera_producer::commit_refresh(previous,&mut view);
                                view.details_visible = previous.details_visible;
                                view.toolbar_input = std::mem::take(&mut previous.toolbar_input);
                                // Read the current event-thread sample; never
                                // derive it from a snapshot's stale camera flags.
                                view.toolbar_right_held = previous.toolbar_right_held;

                                if let Some(locator) = &mut view.locator
                                    && !locator.has_strategic() {
                                    locator.select_poll_category(view.selected.and_then(|hit| {
                                        self.assets.selection.blocks.get(hit.category).map(|block| block.id)
                                    }));
                                    locator.pointer(menu.locator_assets(), if view.strategic && view.adjustment.is_none() {
                                        view.pointer
                                    } else { None });
                                }
                                if let (Some(comparison), Some(old)) = (&mut view.comparison, &previous.comparison) {
                                    comparison.retain_ui_from(old);
                                }
                                view.utility_report = std::mem::take(&mut previous.utility_report);
                                view.utility_report.select(view.utility_category(&self.assets.selection.blocks));
                                view.utility_report.refresh_property_offer();
                                view.property_pending = previous.property_pending.take();
                                view.rights_pending = previous.rights_pending.take();
                                view.reframe_pending = true;
                            }
                            if !refreshing {
                                eprintln!(
                                    "{}: {} buildings; simulation paused",
                                    if self.loading_from_setup {
                                        "Scenario preview"
                                    } else {
                                        "Saved city inspection"
                                    },
                                    view.scene.building_count
                                );
                            }
                            if let Some(inbox) = camera_inbox {
                                let owner = self.camera_parked.take().expect("one process camera Owner parked before native attach");
                                view.camera_endpoint = Some(camera_live::endpoint::Endpoint::with_property_install(
                                    owner,inbox,property_install.expect("prepared actual property install resources")));
                            }
                            self.refresh_retry.complete();
                            if !refreshing && view.camera_producer.is_some() {
                                if let Some(sender) = &view._fresh_session {
                                    let (reply, result) = mpsc::channel();
                                    if let Err(error) = sender.send(FreshRequest::UiCameraReady { reply }) {
                                        view.camera_producer.as_mut().expect("attached producer").invalidate(error.to_string());
                                    }
                                    view.camera_ready = Some(result);
                                }
                            }
                            if !refreshing {
                                if let Some(process)=&self.process_frontend {process.borrow_mut().stop_menu()?;}
                                if view._fresh_session.is_some() && let Some(era)=pending_music_era {
                                    view.era_audio_player.apply(crate::era_audio::Operation::Select{era,repeat:true})?;
                                }
                            }
                            let focus_sender = (!refreshing && self.focus.enabled())
                                .then(||view._fresh_session.clone()).flatten();
                            self.inspection = Some(view);
                            if let Some(sender) = focus_sender {
                                self.focus.attach(sender)?;
                                if let Some(process)=&self.process_frontend {process.borrow_mut().note_sent(self.focus.sent());}
                            }
                            if self.loading_skirmish {
                                menu.skirmish.finish(None);
                            } else if self.loading_from_setup {
                                menu.single.message = None;
                            } else {
                                menu.load.message = None;
                            }
                            presenter.window.set_title("OpenMT2001 - city inspection (simulation paused) / arrows pan, right-drag orbit/zoom, Esc returns");
                        }
                        Err(e) => {
                            if refreshing {
                                // Keep the SAME inspection/endpoint running. A
                                // bounded retry sends the real Inspect request;
                                // no Intro::draw error reaches AppExit here.
                                self.refresh_retry.reject(e.clone());
                                eprintln!("Scene refresh deferred: {e}");
                            }
                            if self.loading_skirmish {
                                menu.skirmish.finish(Some(e));
                            } else if self.loading_from_setup {
                                menu.single.message = Some(e)
                            } else {
                                menu.load.message = Some(e)
                            }
                        }
                    }
                    if self.loading_from_setup {
                        menu.single.busy = false;
                    } else {
                        menu.load.busy = false;
                    }
                    menu.invalidate();
                }
            }
        }
        if let Some(view) = &mut self.inspection {
            if let Some(receiver) = &view.camera_ready {
                match receiver.try_recv() {
                    Ok(Ok(())) => { view.camera_ready = None; }
                    Err(mpsc::TryRecvError::Empty) => {}
                    other => {
                        let error = match other { Ok(Err(e)) => e, _ => "Camera attachment boundary closed".into() };
                        if let Some(producer) = &mut view.camera_producer { producer.invalidate(error.clone()); }
                        view.save_dialog.message = Some(error); view.camera_ready = None;
                    }
                }
            }
            match view.options_ack.poll() {
                Ok(Some(_))=>view.game_options.set_save_disabled(!view.options_ack.ready()
                    || view.camera_producer.as_ref().is_some_and(|p|p.error().is_some())
                    || view.live_status.as_ref().is_none_or(|s|s.tutorial_session||s.failure.is_some())),
                Ok(None)=>{},
                Err(error)=>{view.game_options.set_save_disabled(true);view.game_options.set_message(error);}
            }
            camera_live::ui_cursor_pass(view, &self.assets, self.menu.as_ref())?;
            camera_live::audio::service_with_music_poll(view,self.focus.sent()==self.focus.completed())?;
            if let Some(mut endpoint)=view.camera_endpoint.take() {
                let result=(|| {
                    let joins=camera_live::UiJoins{primary_clear:None,navigation:None,tutorial:None};
                    let mut context=camera_live::UiContext::new(view,&mut self.camera_physical,&mut self.assets,
                        &joins,camera_live::SoundBoundary::Existing2d)?;
                    // A direct ack wakes the worker, which produces the next
                    // ordered request. Do not force a full software-rendered
                    // frame between every such request. Bound transport work
                    // to8ms, waiting at most1ms between completed operations.
                    // These times never enter numeric sampling or Host timers.
                    let started=Instant::now();let budget=std::time::Duration::from_millis(8);
                    let mut completed=0usize;
                    loop {
                        let remaining=budget.saturating_sub(started.elapsed());
                        if completed>0 && remaining.is_zero(){break;}
                        let service=if completed==0 {endpoint.service(&mut context)?}
                            else {endpoint.wait_service(&mut context,remaining.min(std::time::Duration::from_millis(1)))?};
                        if service==crate::live_host::camera_bridge::Service::Idle {break;}
                        completed+=1;
                    }
                    if completed>1 && std::env::var_os("OPENMT2001_CAMERA_OWNER_NATIVE_REVIEW").is_some() {
                        static REPORTED:std::sync::atomic::AtomicBool=std::sync::atomic::AtomicBool::new(false);
                        if !REPORTED.swap(true,std::sync::atomic::Ordering::Relaxed) {
                            eprintln!("[camera] serviced{completed} ordered requests within one bounded UI pass");
                        }
                    }
                    Ok::<_,String>(())
                })();
                view.camera_endpoint=Some(endpoint);
                result?;
            }
            Self::poll_camera_save(view);
            while let Some(request) = view
                .auction_requests
                .as_ref()
                .and_then(|r| r.try_recv().ok())
            {
                let result = (|| {
                    view.auction.service_actors(&self.renderer)?;
                    for cue in view.auction.take_cues() {
                        view.gameplay_sound_player.apply(
                            crate::gameplay_sound::Operation::AuctionActor(cue))?;
                    }
                    let inputs = &self.assets.inputs;
                    view.auction.present(
                        request.operation,
                        &self.renderer,
                        (&inputs.executable, &inputs.directory, &inputs.archive),
                        if view.strategic { 3 } else { 0 },
                    )
                })();
                view.keys = [false; 4];
                view.taps = [false; 4];
                view.drag = [0; 2];
                let _ = request.reply.send(result);
            }
            view.auction.service_actors(&self.renderer)?;
            for cue in view.auction.take_cues() {
                view.gameplay_sound_player.apply(
                    crate::gameplay_sound::Operation::AuctionActor(cue))?;
            }
        }
        self.refresh_inspection_projection()?;
        if !self
            .inspection
            .as_ref()
            .is_some_and(|v| {
            v.auction.scene.is_some() || v.prompts.request().is_some()
                || v.result.is_some() || v.result_pending.is_some()
                || v.live_status.as_ref().is_some_and(|s| s.result_request.is_some())
        })
        {
            self.inspection_update()?;
        }
        if let Some(view) = &mut self.inspection {
            let draw_distance = u32::from(view.settings_menu.graphics.distance);
            if let Some(receiver) = &view.people_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.people_pending = None;
                        let mut people = result?;
                        if let Some(previous) = &view.people {
                            people.replace_attachment(previous)?;
                            for i in people.reframe(&mut view.scene, view.camera, draw_distance)? {
                                self.renderer.update_dynamic_surface(
                                    &mut view.prepared,
                                    i,
                                    &view.scene.draws[i].borrowed(),
                                    view.scene.camera,
                                    view.scene.fog,
                                )?;
                            }
                            view.people = Some(people);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.people_pending = None;
                        return Err("Resident simulation session disconnected".into());
                    }
                }
            }
            if self.loading.is_none()
                && view.people_pending.is_none()
                && view.last_people_query.elapsed().as_millis() >= 100
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.running && !s.script_paused && s.failure.is_none())
                && let Some(sender) = &view._fresh_session
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::Residents { reply })
                    .map_err(|e| e.to_string())?;
                view.people_pending = Some(answer);
                view.last_people_query = Instant::now();
            }
            if let Some(receiver) = &view.vehicles_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.vehicles_pending = None;
                        let mut vehicles = result?;
                        if let Some(previous) = &view.vehicles {
                            vehicles.replace_attachment(previous)?;
                            for i in vehicles.reframe(&mut view.scene, view.camera, draw_distance)? {
                                self.renderer.update_dynamic_surface(
                                    &mut view.prepared, i,
                                    &view.scene.draws[i].borrowed(),
                                    view.scene.camera, view.scene.fog,
                                )?;
                            }
                            view.vehicles = Some(vehicles);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.vehicles_pending = None;
                        return Err("Road simulation session disconnected".into());
                    }
                }
            }
            if self.loading.is_none()
                && view.vehicles_pending.is_none()
                && view.last_vehicles_query.elapsed().as_millis() >= 40
                && view.live_status.as_ref().is_some_and(|s| s.running && !s.script_paused && s.failure.is_none())
                && let Some(sender) = &view._fresh_session
            {
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::Vehicles { reply })
                    .map_err(|e| e.to_string())?;
                view.vehicles_pending = Some(answer);
                view.last_vehicles_query = Instant::now();
            }
            if let Some(receiver) = &view.transport_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.transport_pending = None;
                        let mut transport = result?;
                        if let Some(previous) = &view.transport {
                            transport.replace_attachment(previous)?;
                            for i in transport.reframe(&mut view.scene, view.camera, draw_distance)? {
                                self.renderer.update_dynamic_surface(
                                    &mut view.prepared, i,
                                    &view.scene.draws[i].borrowed(),
                                    view.scene.camera, view.scene.fog,
                                )?;
                            }
                            view.transport = Some(transport);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.transport_pending = None;
                        return Err("Transport simulation session disconnected".into());
                    }
                }
            }
            if self.loading.is_none()
                && view.transport_pending.is_none()
                && view.last_transport_query.elapsed().as_millis() >= 40
                && view.live_status.as_ref().is_some_and(|status| {
                    status.running && !status.script_paused && status.failure.is_none()
                })
                && let Some(sender) = &view._fresh_session
            {
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::Transport { reply })
                    .map_err(|e| e.to_string())?;
                view.transport_pending = Some(answer);
                view.last_transport_query = Instant::now();
            }
        }
        if let Some(view) = &mut self.inspection
            && let Some(state) = &mut view.adjustment
        {
            if let Some(receiver) = &state.edit_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        state.edit_pending = None;
                        state.receive_edit(result);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        state.edit_pending = None;
                        state.receive_edit(Err("Business session disconnected".into()));
                    }
                }
            }
            if state.edit_pending.is_none()
                && let Some(action) = state.edit_request.take()
                && let (Some(target), Some(sender)) = (state.edit_target, &view._fresh_session)
            {
                let (reply, answer) = mpsc::channel();
                match sender.send(FreshRequest::EditBusiness {
                    target,
                    action,
                    reply,
                }) {
                    Ok(()) => {
                        state.edit_pending = Some(answer);
                        state.edit_inflight = Some(action);
                    }
                    Err(error) => state.receive_edit(Err(error.to_string())),
                }
            }
            if state.close {
                view.adjustment = None;
            }
            let producer = view.adjustment.as_mut().and_then(|s| s.take_tutorial_producer());
            if let Some((control, event)) = producer { view.send_tutorial_producer(control, event); }
            let control = view.adjustment.as_mut().and_then(|s| s.take_tutorial_control());
            if let Some(id) = control
                && view.live_status.as_ref().is_some_and(|s| s.tutorial_enabled_controls.contains(&id)) {
                view.send_tutorial_control(id);
            }
        }
        if let Some(view) = &mut self.inspection {
            let selected = view
                .selected
                .and_then(|h| h.building)
                .and_then(|i| view.scene.inspection_buildings.get(i))
                .filter(|b| building_has_management_link(b))
                .and_then(|b| b.runtime_slot);
            view.operations.select(selected);
            //Original report blink clock822d40 advances with25Hz logic ticks.
            view.operations.logic_tick = view.live_status.as_ref().map_or(0, |s| s.ticks);
            view.operations.poll();
            if let Some(menu) = &self.menu {
                view.operations.pointer(
                    menu.business_assets(),
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                );
            }
            if view.operations.refresh
                && view.operations.pending.is_none()
                && let (Some(building), Some(sender)) = (selected, &view._fresh_session)
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::InspectOperations {
                        building,
                        bounds: self
                            .menu
                            .as_ref()
                            .ok_or("building menu assets missing")?
                            .business_assets()
                            .bounds(),
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                view.operations.pending = Some(answer);
                view.operations.refresh = false;
            }
            if let Some(action) = view.operations.action.take()
                && let (Some(building), Some(sender)) = (selected, &view._fresh_session)
            {
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::BusinessOperation {
                        building,
                        action,
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                view.operations.action_pending = Some(answer);
            }
            if let Some(mode) = view.operations.navigation.take() {
                if mode == 1
                    && let (Some(i), Some(menu)) =
                        (view.selected.and_then(|h| h.building), &self.menu)
                {
                    view.adjustment = menu.adjustment_assets().open_for_owner(&view.scene, i,
                        view.live_status.as_ref().and_then(|s| s.business_owner),
                    )?;
                }
                if mode == 5
                    && let Some(hit) = view.selected
                {
                    //40889b restores the original block camera on return.
                    view.focus_category(hit.category, &self.assets.selection);
                }
            }
        }
        let mut build_request = None;
        if let Some(view) = &mut self.inspection {
            if let Some(receiver) = &view.planning_pending {
                match receiver.try_recv() {
                    Ok(result) => {
                        view.planning_pending = None;
                        match result {
                            Ok(context) => {
                                if matches!(context.entry(), crate::build_panel::Entry::Message {
                                    show_rights_action: true, ..
                                }) {
                                    view.utility_report.open_property_rights(context.category, 0);
                                    view.details_visible = true;
                                    view.title_state = None;
                                } else if let Some(menu) = &self.menu {
                                    let mut planning = menu.planning_assets().open(context)?;
                                    planning.set_tutorial_build_type(view.live_status.as_ref()
                                        .map_or(-1, |status| status.tutorial_build_type),
                                    );
                                    planning.set_tutorial_build_use(view.live_status.as_ref()
                                        .map_or(999, |status| status.tutorial_build_use),
                                    );
                                    view.planning = Some(planning);
                                }
                            }
                            Err(error) => eprintln!("Planning unavailable: {error}"),
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.planning_pending = None;
                    }
                }
            }
            if let Some(planning) = &mut view.planning {
                let occupied=view.live_status.as_ref().is_some_and(|status|status.event_list_visible)
                    && self.menu.as_ref().is_some_and(|menu| {
                        view.pointer.is_some_and(|point| {
                            menu.event_list_assets().contains(point.map(|v|v as f32*0.5))
                        })
                    });
                planning.pointer(view.pointer.filter(|_|!occupied));
                let block = self
                    .assets
                    .selection
                    .blocks
                    .iter()
                    .find(|b| b.id == planning.context.category)
                    .ok_or("Planning block missing")?;
                planning.project_grid(view.camera, block.center)?;
                if let Some(receiver) = &planning.placement_pending {
                    match receiver.try_recv() {
                        Ok(Ok(choices)) => planning.apply_placement_choices(choices),
                        Ok(Err(error)) => {
                            planning.placement_pending = None;
                            planning.diagnostic = Some(error);
                            planning.dirty = true;
                        }
                        Err(mpsc::TryRecvError::Disconnected) => {
                            planning.placement_pending = None;
                            planning.diagnostic = Some("Build placement session disconnected".into());
                            planning.dirty = true;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                    }
                }
                if let Some(receiver) = &planning.pending {
                    match receiver.try_recv() {
                        Ok(result) => {
                            planning.pending = None;
                            match result {
                                Ok(quote) if quote.submitted => {
                                    // The native pending-world entry begins at scale zero.
                                    // Show its growth before the idle one-second refresh.
                                    view.construction_active = true;
                                    planning.close = true;
                                }
                                Ok(quote) => {
                                    if planning.request().ok() == planning.quote_request {
                                        planning.diagnostic = if quote.category_locked {
                                            Some("This block is locked".into())
                                        } else if !quote.placement_allowed {
                                            Some("This lot is occupied".into())
                                        } else if !quote.affordable {
                                            Some("Insufficient funds".into())
                                        } else if quote.preview.is_none() {
                                            Some("No building model matches".into())
                                        } else {
                                            None
                                        };
                                        planning.reset_preview();
                                        planning.quote = Some(quote);
                                    } else {
                                        planning.needs_quote = true;
                                    }
                                }
                                Err(error) => planning.diagnostic = Some(error),
                            }
                            planning.dirty = true;
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => {
                            planning.pending = None;
                            planning.diagnostic = Some("Session disconnected".into());
                            planning.dirty = true;
                        }
                    }
                }
                if planning.pending.is_none() && (planning.needs_quote || planning.submit) {
                    match planning.request() {
                        Ok(form) => {
                            planning.quote_request = Some(form.clone());
                            build_request = Some((form, planning.submit));
                        }
                        Err(error) => {
                            planning.diagnostic = Some(error);
                            planning.dirty = true;
                        }
                    }
                    planning.needs_quote = false;
                    planning.submit = false;
                }
                if planning.close {
                    view.planning = None;
                }
            }
        }
        if let Some((form, submit)) = build_request {
            let result = self.request_build_form(form, submit);
            if let Some(planning) = self
                .inspection
                .as_mut()
                .and_then(|view| view.planning.as_mut())
            {
                match result {
                    Ok(receiver) => planning.pending = Some(receiver),
                    Err(error) => planning.diagnostic = Some(error),
                }
                planning.dirty = true;
            }
        }
        let mut refresh = false;
        let mut tutorial_completed = false;
        if let Some(view) = &mut self.inspection {
            if let Some(receiver) = &view.save_pending {
                let outcome = match receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err("Session closed before saving".into()))
                    }
                };
                if let Some(result) = outcome {
                    let message = match &result {
                        Ok(path) => {
                            eprintln!("Saved V2 game: {}", path.display());
                            format!("Saved: {}", path.file_name().unwrap_or_default().to_string_lossy())
                        }
                        Err(error) => {
                            eprintln!("Save failed: {error}");
                            format!("Save failed: {error}")
                        }
                    };
                    view.game_options.set_message(message);
                    if let Some(root) = view.save_profile_root.as_deref() {
                        view.save_dialog.saved(result, root);
                    }
                    view.save_pending = None;
                }
            }
            if let Some(receiver)=&view.save_delete_pending {
                let outcome=match receiver.try_recv() {
                    Ok(result)=>Some(result),
                    Err(mpsc::TryRecvError::Empty)=>None,
                    Err(mpsc::TryRecvError::Disconnected)=> {
                        Some(Err("Session closed before deletion".into()))
                    }
                };
                if let Some(result)=outcome {
                    if let Some(root) = view.save_profile_root.as_deref() {
                        view.save_dialog.deleted(result, root);
                    }
                    view.save_delete_pending=None;
                }
            }
            if let Some(hit) = view.selected {
                let selected = (
                    self.assets.selection.blocks[hit.category].id as i32,
                    hit.building
                        .and_then(|i| view.scene.inspection_buildings[i].runtime_slot)
                        .map_or(-1, |i| i as i32),
                );
                if view.card_selection != Some(selected)
                    && let Some(sender) = &view._fresh_session
                {
                    sender
                        .send(FreshRequest::Card(
                            crate::fresh_city::cards::Action::Select {
                                category: selected.0,
                                building: selected.1,
                            },
                        ))
                        .map_err(|e| e.to_string())?;
                    view.card_selection = Some(selected);
                }
            }
            if let Some(reply) = &view.prompt_reply {
                match reply.try_recv() {
                    Ok(requests) => {
                        view.prompt_reply = None;
                        view.prompts.finish_dismiss(requests);
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        return Err("Prompt session disconnected".into());
                    }
                }
            }
            for id in view
                .prompts
                .advance_to((view.started.elapsed().as_secs_f64() * 50.).floor() as u64)
            {
                view.dismiss_prompt(id)?;
            }
            for response in view.prompts.take_dialog_replies() {
                view.reply_human_dialog(response)?;
            }
            if let Some(receiver) = &view.status_pending {
                match receiver.try_recv() {
                    Ok(mut status) => {
                        crate::live_host::report_status(&status, view.live_status.as_ref());
                        view.stats_requested=status.stats.is_some();
                        tutorial_completed = status.tutorial_completed;
                        view.apply_status_camera_events(&status,
                            &self.assets.selection, &self.assets.navigation)?;
                        for request in status.prompts.drain(..) {
                            view.prompts.submit(request);
                            view.keys = [false; 4];
                            view.taps = [false; 4];
                            view.drag = [0; 2];
                            view.mouse_buttons = [false; 2];
                        }
                        if status.tutorial_message != view.tutorial_message.request() {
                            view.tutorial_message = crate::prompt::State::default();
                            if let Some(message) = status.tutorial_message.clone() {
                                view.tutorial_message.submit(message);
                            }
                        }
                        view.auction_notice.update(status.auction_notice.clone());
                        if let Some(planning) = &mut view.planning {
                            planning.set_tutorial_build_type(status.tutorial_build_type);
                            planning.set_tutorial_build_use(status.tutorial_build_use);
                        }
                        if !status.player_panels[0] {
                            view.planning = None;
                            view.planning_pending = None;
                            view.adjustment = None;
                            view.details_visible = false;
                        }
                        if view.selected.is_some_and(|selected| {
                            status
                                .auction_locked_categories
                                .contains(&self.assets.selection.blocks[selected.category].id)
                        }) {
                            //Original41ed10->4053a0/407cd0 cancels editing a
                            //category once its auction announcement is admitted.
                            view.planning = None;
                            view.planning_pending = None;
                            view.adjustment = None;
                        }
                        update_locator_financial(&mut view.locator, &status)?;
                        if let Some(locator) = &mut view.locator {
                            locator.refresh_scene(&view.scene);
                        }
                        if let (Some(locator), Some(rows)) =
                            (&mut view.locator, status.city_people_rows.take())
                        {
                            locator.apply_people_rows(rows);
                        }
                        view.pulse_state = Some(status.pulse_lights.clone());
                        view.time_panel.update(&self.assets.time_panel, &self.renderer, &status)?;
                        view.cash_arrow.update(&self.assets.cash_arrow, &self.renderer, status.cash_arrow,
                        )?;
                        view.dust.update(&self.assets.dust, &self.renderer, &status.demolition_points,
                            view.camera, view.scene.camera,
                        )?;
                        if let Some(error)=&status.failure {
                            if let Some(state)=&mut view.camera_ui {state.reject_save_captures(error);}
                            if view.camera_capture_pending.take().is_some() {
                                view.save_dialog.busy=false;
                                view.save_dialog.message=Some(format!("Save failed: {error}"));
                            }
                        }
                        view.game_options.set_save_disabled(!view.options_ack.ready() || status.tutorial_session || status.failure.is_some()
                            || view.camera_producer.as_ref().is_some_and(|p|p.error().is_some()));
                        view.live_status = Some(status);
                        view.status_pending = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        view.status_pending = None;
                    }
                }
            }
            if view.status_pending.is_none()
                && view.last_status_query.elapsed().as_millis()
                    >= if view.auction.scene.is_some() || view.cash_arrow.active || view.dust.active {
                        50
                    } else {
                        250
                    }
                && let Some(sender) = &view._fresh_session
            {
                let (reply, result) = mpsc::channel();
                sender
                    .send(FreshRequest::Status {
                        camera: view.camera.eye,
                        reply,
                    })
                    .map_err(|e| e.to_string())?;
                view.status_pending = Some(result);
                view.last_status_query = Instant::now();
            }
            if let Some(reply) = &view.environment_pending {
                match reply.try_recv() {
                    Ok(clock) => {
                        view.environment_pending = None;
                        view.time_panel.update_clock(&self.assets.time_panel, &self.renderer, clock,
                        )?;
                        let (tick, hour, counter) = (clock.ticks, clock.hour, clock.fraction);
                        if tick != view.environment_tick || view.environment_time != Some((hour, counter)) {
                            let mut changed = view.scene.advance_environment(tick, view.camera.eye)?;
                            changed.extend(view.scene.advance_daylight(hour, counter, view.camera,
                            )?);
                            if let Some(trees) = &mut view.trees {
                                changed.extend(trees.reframe(&mut view.scene, view.camera)?);
                            }
                            if let Some(people) = &view.people {
                                changed.extend(people.reframe(&mut view.scene, view.camera,
                                    u32::from(view.settings_menu.graphics.distance))?);
                            }
                            let distance = u32::from(view.settings_menu.graphics.distance);
                            if let Some(vehicles) = &view.vehicles {
                                changed.extend(vehicles.reframe(&mut view.scene, view.camera, distance)?);
                            }
                            if let Some(transport) = &view.transport {
                                changed.extend(transport.reframe(&mut view.scene, view.camera, distance)?);
                            }
                            let star_detail = view.normal_star_detail();
                            let changed = normal_city_scene_updates(
                                &mut view.scene, view.camera, star_detail, changed,
                            )?;
                            for index in changed {
                                let draw = &view.scene.draws[index];
                                self.renderer.update_dynamic_vertices(
                                    &mut view.prepared, index, &draw.borrowed(),
                                )?;
                            }
                            let cameras = view.scene.draws.iter().map(|draw|
                                draw.view_projection.unwrap_or(view.scene.camera)).collect::<Vec<_>>();
                            self.renderer.update_cameras(&view.prepared, &cameras, view.scene.fog,
                            )?;
                            view.environment_tick = tick;
                            view.environment_time = Some((hour, counter));
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => view.environment_pending = None,
                }
            }
            if view.environment_pending.is_none()
                && view.auction.scene.is_none()
                && view.result.is_none()
                && view.last_environment_query.elapsed().as_millis() >= 40
                && let Some(sender) = &view._fresh_session
            {
                let (reply, result) = mpsc::channel();
                sender.send(FreshRequest::LogicTick(reply)).map_err(|e| e.to_string())?;
                view.environment_pending = Some(result);
                view.last_environment_query = Instant::now();
            }
            refresh = self.loading.is_none()
                && self.refresh_retry.error.is_none()
                && view.auction.scene.is_none()
                && view.adjustment.is_none()
                && view.planning.is_none()
                && view.planning_pending.is_none()
                && view.last_snapshot.elapsed()
                    >= city_snapshot_interval(view.construction_active)
                && view
                    .live_status
                    .as_ref()
                    .is_some_and(|s| s.ticks != view.snapshot_tick);
        }
        if tutorial_completed {
            refresh = false;
            self.retire_inspection();
            if let Some(menu) = &mut self.menu {
                menu.refresh_campaign()?;
            }
            self.last_sample = None;
        }
        if self.draw_result(presenter)? {
            return Ok(());
        }
        if refresh {
            self.refresh_fresh_city()?;
        }
        if let Some(view) = &mut self.inspection
            && view.ui_graph.as_ref().is_none_or(|g|g.group_enabled(camera_live::navigation::Group::Tertiary))
            && let Some((scene, true)) = &view.auction.scene
        {
            self.renderer
                .draw_prepared(&self.color_view, &self.depth_view, scene);
            if let Some(status)=&view.live_status {
                if view.prompts.request().is_some() || view.stats_requested || status.stats.is_some() {
                    view.time_panel.cancel_input();view.time_pressed=None;
                }
                view.time_panel.update(&self.assets.time_panel,&self.renderer,status)?;
                if let Some(panel)=&view.time_panel.prepared {
                    self.renderer.draw_overlay(&self.color_view,&self.depth_view,panel);
                }
            }
            if let Some(menu) = &self.menu {
                view.auction_panel.prepare(
                    menu.auction_panel_assets(),
                    &self.renderer,
                    view.live_status.as_ref().and_then(|s| s.auction.as_ref()),
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                )?;
                if let Some(panel) = &view.auction_panel.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
            }
            Self::draw_tutorial_visual(
                &self.assets, &self.menu, &self.renderer,
                &self.color_view, &self.depth_view, view,
            )?;
            if let Some(menu)=&self.menu {
                view.stats_panel.update(menu.stats_assets(),&self.renderer,
                    view.live_status.as_ref().and_then(|status|status.stats.as_ref()),
                )?;
                if let Some(panel)=&view.stats_panel.prepared {
                    self.renderer.draw_overlay(&self.color_view,&self.depth_view,panel);
                }
            }
            if view.prompts.request().is_some()
                && let Some(menu) = &self.menu
            {
                view.prompts.cursor(
                    view.pointer
                        .map(|p| p.map(|v| v as f32 * 0.5))
                        .unwrap_or([-1.; 2]),
                );
                let panel = menu
                    .prompt_assets()
                    .prepare_positioned(&self.renderer, &view.prompts,
                        view.live_status.as_ref().map_or([0.; 2], |status| {
                        status.tutorial_interaction.prompt_offset()
                    }),
                )?;
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, &panel);
            }
            if view.prompts.request().is_none()
                && view.tutorial_message.request().is_some()
                && let Some(menu) = &self.menu
            {
                let panel = menu.prompt_assets()
                    .prepare_message_positioned(&self.renderer, &view.tutorial_message,
                        view.live_status.as_ref().map_or([0.; 2], |status| {
                        status.tutorial_interaction.prompt_offset()
                    }),
                )?;
                self.renderer.draw_overlay(&self.color_view, &self.depth_view, &panel);
            }
            if let Some(status) = &view.live_status {
                presenter.window.set_title(&format!(
                    "OpenMT2001 - auction{}",
                    status
                        .failure
                        .as_ref()
                        .map_or(String::new(), |e| format!(" - stopped: {e}"))
                ));
            }
            return presenter
                .present_binding(&self.binding)
                .map_err(|e| e.to_string());
        }
        if let Some(view) = &mut self.inspection {
            if view.ui_graph.is_some(){
                if let Some(prepared)=camera_live::upper_f::body(view,&self.assets,&self.renderer)?{
                    view.navigator=prepared;
                }
            }else{
            //43d0f0 reads the logic-step eye93af64/6c, not the interpolated render eye.
            let arrow_eye = if view.strategic {
                view.camera.eye
            } else {
                view.inertia.sample(0.)?.0
            };
            self.assets.navigator.update_ownership_arrow(
                &self.renderer,
                &mut view.navigator,
                [arrow_eye[0], arrow_eye[2]],
                view.controls.angle,
                view.live_status.as_ref().map_or(0, |status| status.ticks),
                &view.navigator_owners,
            )?;
            }
            if view.ui_graph.is_some() {
                camera_live::focus::present(view,&self.assets,self.menu.as_ref(),&self.renderer)?;
            }else{
            let navigator_tooltip = view
                .pointer
                .filter(|_| view.over_navigator())
                .and_then(|point| {
                    self.assets
                        .navigator
                        .pick_inspection(&self.assets.selection.blocks, point)
                })
                .map(|(category, point)|
                    (self.assets.selection.blocks[category].text_id, point));
            let icon = if navigator_tooltip.is_none()
                && view.strategic && !view.options_visible()
                && view.prompts.request().is_none() && view.adjustment.is_none()
                && view.planning.is_none()
            {
                view.pointer.and_then(|point| {
                    view.locator.as_ref().and_then(|locator| {
                        self.menu.as_ref().and_then(|menu|
                        locator.icon_tooltip_at(menu.locator_assets(), point))
                    })
                })
            } else { None };
            let side_icon=self.menu.as_ref().and_then(|menu|
                view.toolbar_target(menu,&self.assets.selection.blocks)).map(|a|a.tooltip());
            let visible_icon = view.icon_tooltip_dwell.advance(side_icon.or(icon));
            let tooltip_state = navigator_tooltip.or_else(||
                visible_icon.zip(view.pointer));
            if tooltip_state != view.tooltip_state {
                view.tooltip = tooltip_state
                    .map(|(text_id, point)| {
                        self.assets.tooltip.prepare(
                        &self.renderer, text_id, point)
                    })
                    .transpose()?;
                view.tooltip_state = tooltip_state;
            }
            }
            view.locator_blocked = view.strategic
                && (view.ui_graph.is_none()||view.graph_input(camera_live::ui_graph::PanelId::C))
                && view.locator.as_ref().is_some_and(|locator| {
                    self.menu.as_ref().is_some_and(|menu| {
                        view.pointer
                            .is_some_and(|p| menu.locator_assets().contains(locator, p))
                    })
                });
            if let (Some(locator), Some(menu)) = (&mut view.locator, &self.menu) {
                locator.refresh_scene(&view.scene);
                if let Some(category) = locator.take_category_selection() {
                    view.selected = inspection_hit(&self.assets.selection.blocks, category, None);
                }
                locator.select_poll_category(view.selected.and_then(|hit| {
                    self.assets.selection.blocks.get(hit.category).map(|block| block.id)
                }));
                locator.pointer(
                    menu.locator_assets(),
                    if view.strategic && view.adjustment.is_none()
                        && view.ui_graph.as_ref().is_none_or(|graph|graph.input(camera_live::ui_graph::PanelId::C)) {
                        view.pointer
                    } else {
                        None
                    },
                );
            }
            if view.strategic
                && let (Some(locator), Some(menu)) = (&mut view.locator, &self.menu)
                && (view.ui_graph.is_some() || locator.dirty || locator.prepared.is_none())
            {
                locator.prepared=Some(if let Some(graph)=&mut view.ui_graph {
                    menu.locator_assets().prepare_graph(&self.renderer,locator,&view.scene,
                        graph.body(camera_live::ui_graph::PanelId::C),
                        graph.body(camera_live::ui_graph::PanelId::D),
                        Some(&graph.world.colors),graph.world.mode,&mut graph.world.messages)?
                }else{menu.locator_assets().prepare(&self.renderer,locator,&view.scene)?});
                locator.dirty = false;
            }
            if view.statistics_visible()
                && let (Some(comparison), Some(menu)) = (&mut view.comparison, &self.menu)
            {
                comparison.pointer(
                    menu.comparison_assets(),
                    if view.adjustment.is_none() {
                        view.pointer
                    } else {
                        None
                    },
                );
                comparison.prepare(
                    menu.comparison_assets(),
                    &self.renderer,
                    view.locator
                        .as_ref()
                        .map(|l| l.financial_records())
                        .unwrap_or(&view.scene.saved_players),
                )?;
            }
            if let Some(menu) = &self.menu {
                view.utility_report.select(view.utility_category(&self.assets.selection.blocks));
                let side_policy=view.toolbar_policy(menu,&self.assets.selection.blocks,true);
                view.toolbar_input.update(side_policy,view.toolbar_bounds,
                    view.pointer.map(|p|p.map(|v|v as f32*0.5)));
                let visible_policy=view.toolbar_policy(menu,&self.assets.selection.blocks,false);
                view.toolbar_regions=std::array::from_fn(|i|visible_policy
                    .filter(|q|q.visible(crate::locator::toolbar_collapse::Action::ALL[i]))
                    .map(|_|view.toolbar_bounds[i]));
                let point = if !view.strategic
                    && view.main_panel_visible()
                    && view.details_visible
                    && view.adjustment.is_none()
                    && view.utility_category(&self.assets.selection.blocks).is_some()
                {
                    view.pointer
                } else {
                    None
                };
                menu.locator_assets()
                    .utility_pointer(&mut view.utility_report, point);
                if let Some(pending) = &view.property_pending {
                    let response = match pending.reply.try_recv() {
                        Ok(result) => Some(result),
                        Err(mpsc::TryRecvError::Empty) => None,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            Some(Err("Session closed before inspecting the property".into()))
                        }
                    };
                    if let Some(result) = response {
                        view.utility_report.apply_property_inspection(pending.category, result);
                        view.property_pending = None;
                    }
                }
                if view.property_pending.is_none()
                    && let Some(request) = view.utility_report.take_property_request()
                {
                    let category = request.category;
                    if let Some(sender) = &view._fresh_session {
                        let (reply, response) = mpsc::channel();
                        if sender.send(FreshRequest::PropertyOffer { request, reply }).is_ok() {
                            view.property_pending = Some(PendingPropertyInspection {
                                category, reply: response,
                            });
                        } else {
                            view.utility_report.apply_property_inspection(category,
                                Err("Property offer failed: session closed".into()),
                            );
                        }
                    } else {
                        view.utility_report.apply_property_inspection(category,
                            Err("Property offers require a running game session".into()),
                        );
                    }
                }
                if let Some(pending) = &view.rights_pending {
                    let response = match pending.reply.try_recv() {
                        Ok(result) => Some(result),
                        Err(mpsc::TryRecvError::Empty) => None,
                        Err(mpsc::TryRecvError::Disconnected) =>
                            Some(Err("Session closed before inspecting building rights".into(),
                        )),
                    };
                    if let Some(result) = response {
                        view.utility_report.apply_rights_inspection(pending.category, result);
                        view.rights_pending = None;
                    }
                }
                if view.rights_pending.is_none()
                    && let Some(request) = view.utility_report.take_rights_request()
                {
                    let category = request.category;
                    if let Some(sender) = &view._fresh_session {
                        let (reply, response) = mpsc::channel();
                        if sender.send(FreshRequest::PropertyRights {
                            category, panel_mode: request.panel_mode,
                            activate: request.activate, reply,
                        }).is_ok() {
                            view.rights_pending = Some(PendingRightsInspection {
                                category, reply: response,
                            });
                        } else {
                            view.utility_report.apply_rights_inspection(category,
                                Err("Building rights request failed: session closed".into()),
                            );
                        }
                    } else {
                        view.utility_report.apply_rights_inspection(category,
                            Err("Building rights require a running game session".into()),
                        );
                    }
                }
                if let Some(category) = view.utility_report.take_rights_open()
                    && view.planning.is_none() && view.planning_pending.is_none()
                    && let Some(sender) = &view._fresh_session
                {
                    let (reply, response) = mpsc::channel();
                    if sender.send(FreshRequest::Planning { category, reply }).is_ok() {
                        view.planning_pending = Some(response);
                    }
                }
                if view.utility_report.dirty {
                    view.title_state = None;
                    view.utility_report.dirty = false;
                }
            }
            let hovered = view.scene_pointer().and_then(|point| {
                self.assets
                    .selection
                    .pick(view.camera, &view.scene, point, view.selected)
            });
            if view.title_state != Some((view.selected, hovered)) {
                if view.title_state.map(|s| s.0) != Some(view.selected) {
                    view.information = match (view.selected, &self.menu) {
                        (Some(hit), Some(menu)) if !hit.building.is_some_and(|i| {
                            building_has_management_link(&view.scene.inspection_buildings[i])
                        }) => {
                            // Original 4c39ed..4c3a09 selects main-panel mode1
                            // for a picked building whose +1c business link is -1.
                            // The building stays picked and retains its camera.
                            let category = hit.category;
                            let block = &self.assets.selection.blocks[category];
                            if let Some(report) = &view.block_information
                                && report.available(block.id)
                            {
                                Some(if view.ordinary_toolbar_category(&self.assets.selection.blocks).is_some() {
                                    menu.locator_assets().ordinary_property_body(
                                        &self.renderer,report,block.id,&view.utility_report)?
                                } else { menu.locator_assets().property_panel(
                                    &self.renderer,report,block.id,&view.utility_report)? })
                            } else {
                                // There is no original Block Information
                                // report until the current player snapshot is
                                // available. Defer the panel for that interval.
                                None
                            }
                        }
                        _ => None,
                    };
                }
                view.title_state = Some((view.selected, hovered));
                view.hovered = hovered;
                let surface =
                    self.assets
                        .selection
                        .marker_surface(view.selected, hovered, &view.scene)?;
                let mut draw = surface.draw();
                draw.mode = openmt2001::render::SurfaceMode::StaticLit;
                self.renderer.update_surface(
                    &mut view.markers,
                    0,
                    &draw,
                    view.scene.camera,
                    view.scene.fog,
                )?;
                let name = |index: Option<crate::selection::Hit>| {
                    index
                        .map(|i| {
                            let category = &self.assets.selection.blocks[i.category].name;
                            match i.building {
                                Some(b) => match view.scene.inspection_buildings[b].saved_slot {
                                    Some(slot) => format!("{category}, building #{slot}"),
                                    None => format!("{category}, scene building {b}"),
                                },
                                None => category.clone(),
                            }
                        })
                        .unwrap_or_else(|| "none".to_owned())
                };
                presenter.window.set_title(&format!(
                    "OpenMT2001 - {} {} / selected: {} / hover: {} / {}",
                    view.live_status
                        .as_ref()
                        .map_or("inspection", |s| if s.failure.is_some() {
                            "stopped"
                        } else if s.running && !s.script_paused {
                            "running"
                        } else {
                            "paused"
                        }),
                    if view.strategic { "strategic" } else { "city" },
                    name(view.selected),
                    name(view.hovered),
                    if view.strategic {
                        "double-click block to enter, compass restores camera, Esc returns"
                    } else {
                        "right-drag orbit, compass overview, Esc returns"
                    }
                ));
            }
            if let Some(detail) = view.normal_star_detail() {
                let changed = view.scene.apply_normal_star_detail(detail, view.camera)?;
                let updates = changed.into_iter().map(|index|
                    (index, view.scene.draws[index].borrowed())).collect::<Vec<_>>();
                self.renderer.update_dynamic_vertices_batch(&mut view.prepared, &updates)?;
            }
            if view.strategic {
                let colors = view.block_information.as_ref().map(|report| &report.title_colors);
                let map = self.strategic_scene.prepare(
                    &self.assets.strategic_scene, &self.renderer,
                    view.scene.camera, colors,
                )?;
                self.renderer.draw_prepared(&self.color_view, &self.depth_view, map);
            } else {
                update_city_pulse(&self.assets.city_pulse, &self.renderer, view.pulse_draw,
                    view.pulse_state.as_ref(), view.camera, &mut view.scene, &mut view.prepared,
                )?;
                self.renderer
                    .draw_prepared(&self.color_view, &self.depth_view, &view.prepared);
                if view.dust.active && view.dust.prepared.is_some() {
                    // Recompute the billboard basis after camera movement, even
                    //between authoritative point updates from the worker.
                    if let Some(status)=&view.live_status {
                        view.dust.update(&self.assets.dust,&self.renderer,&status.demolition_points,
                            view.camera,view.scene.camera,
                        )?;
                    }
                    self.renderer.draw_world_overlay(&self.color_view,&self.depth_view,
                        view.dust.prepared.as_ref().ok_or("dust draw lost prepared resources")?,
                    );
                }
                self.assets.sidewalk_labels.update(&self.renderer, &mut self.sidewalk_labels,
                    &mut self.sidewalk_shading, &view.scene, view.camera,
                    view.settings_menu.graphics.distance,
                )?;
                self.renderer
                    .draw_world_overlay(&self.color_view, &self.depth_view, &self.sidewalk_labels,
                );
            for index in 0..self.mr_monopoly_draws {
                self.renderer.update_camera(
                    &self.mr_monopoly,
                    index,
                    view.scene.camera,
                    view.scene.fog,
                )?;
            }
            self.renderer
                .draw_world_overlay(&self.color_view, &self.depth_view, &self.mr_monopoly,
                );
            // The quoted model is world geometry. Composite it while the city
            // depth attachment is still retained, before any UI pass clears it.
            if let (Some(planning), Some(_menu)) = (&mut view.planning, &self.menu) {
                let inputs = &self.assets.inputs;
                let files = crate::build_view::PreviewFiles {
                    build_file: &inputs.build_file,
                    executable: &inputs.executable,
                    directory: &inputs.directory,
                    archive: &inputs.archive,
                    suncols: &inputs.suncols,
                    phase: view.live_status.as_ref().map_or(9, |status| status.hour),
                };
                if let Some(preview) = planning.preview(
                    &self.renderer,
                    view.scene.camera,
                    view.scene.fog,
                    &self.assets.selection,
                    files,
                )? {
                    self.renderer.draw_world_overlay(
                        &self.color_view, &self.depth_view, preview,
                    );
                    if let Some(marker) = planning.preview_marker() {
                        self.renderer.draw_world_overlay(
                            &self.color_view, &self.depth_view, marker,
                        );
                    }
                }
            }
            self.renderer
                .update_camera(&view.markers, 0, view.scene.camera, view.scene.fog)?;
            self.renderer
                .draw_overlay(&self.color_view, &self.depth_view, &view.markers);
            }
            if let (Some(menu),Some(status))=(&self.menu,&view.live_status) {
                view.event_list.update(menu.event_list_assets(),&self.renderer,
                    status.event_list_visible,status.event_list_settled,
                )?;
                if let Some(panel)=&view.event_list.prepared {
                    //43ce12 inserts model14, then13..0 move to head;43cb40 draws tail first.
                    self.renderer.draw_overlay(&self.color_view,&self.depth_view,panel);
                }
            }
            if !view.options_visible() {
            // Native 404eb0 closes the current main/Block Information panel
            // before 43be20 opens the construction planner. Keep its retained
            // state for dismissal, but do not composite glyphs behind the
            // planner's translucent body.
            let base_panel_visible = view.main_panel_visible() && view.planning.is_none();
            if base_panel_visible
                && view.details_visible
                && !view.strategic
                && view.operations.building.is_none()
                && let Some((panel, _)) = &view.information
            {
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, panel);
            }
            if base_panel_visible
                && view.details_visible
                && !view.strategic
                && view.adjustment.is_none()
                && view.operations.building.is_some()
                && let (Some(menu), Some(data)) = (&self.menu, &view.operations.inspection)
            {
                let panel = menu.business_assets().prepare(
                    &self.renderer,
                    &view.operations,
                    data.demolition_label,
                )?;
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, &panel);
            }
            if base_panel_visible
                && let Some(menu)=&self.menu
                && let Some(policy)=view.toolbar_policy(menu,&self.assets.selection.blocks,false)
                && policy.shown
            {
                let panel=menu.locator_assets().ordinary_toolbar_scene(&self.renderer,policy,
                    &view.toolbar_input,&view.utility_report,view.details_visible)?;
                self.renderer.draw_overlay(&self.color_view,&self.depth_view,&panel);
            }
            if base_panel_visible {
                self.renderer.draw_overlay(
                    &self.color_view,
                    &self.depth_view,
                    &view.detail_icons[usize::from(view.details_visible)],
                );
            }
            if view.ui_graph.is_none()||view.ui_graph.as_ref().is_some_and(|g|g.shell(camera_live::ui_graph::PanelId::F)){
                self.renderer.draw_overlay(&self.color_view,&self.depth_view,&view.navigator);
            }
            if view.strategic
                && let Some(panel) = view.locator.as_ref().and_then(|l| l.prepared.as_ref())
            {
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, panel);
            }
            if view.statistics_visible()
                && let Some(panel) = view.comparison.as_ref().and_then(|c| c.prepared.as_ref())
            {
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, panel);
            }
            let icon = if view.navigation_pressed && view.over_navigation() {
                2
            } else if view.over_navigation() {
                1
            } else {
                0
            };
            if view.ui_graph.is_none()||view.graph_body(camera_live::ui_graph::PanelId::E){
                self.renderer.draw_overlay(&self.color_view,&self.depth_view,&view.navigation[icon]);
            }
            // The bottom-right round controls share the navigation backing;
            // draw their original-texture faces after it, along with the clock.
            if let Some(status)=&view.live_status {
                if view.prompts.request().is_some() || view.stats_requested || status.stats.is_some() {
                    view.time_panel.cancel_input();view.time_pressed=None;
                }
                view.time_panel.update(&self.assets.time_panel,&self.renderer,status)?;
            }
            if let Some(graph)=&view.ui_graph {
                view.time_panel.graph_gates(graph.body(camera_live::ui_graph::PanelId::Clock),
                    graph.body(camera_live::ui_graph::PanelId::E),graph.speed_disabled!=0,graph.speed_blink);
                view.time_panel.prepare_graph(&self.assets.time_panel,&self.renderer)?;
                if let Some(bottom)=&view.time_panel.bottom_prepared {
                    self.renderer.draw_overlay(&self.color_view,&self.depth_view,bottom);
                }
            }
            if let Some(panel) = &view.time_panel.prepared {
                self.renderer.draw_overlay(&self.color_view, &self.depth_view, panel);
            }
            if view.cash_arrow.active && let Some(arrow) = &view.cash_arrow.prepared {
                self.renderer.draw_overlay(&self.color_view, &self.depth_view, arrow);
            }
            if let Some(tooltip) = &view.tooltip {
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, tooltip);
            }
            if let (Some(adjustment), Some(menu)) = (&mut view.adjustment, &self.menu) {
                adjustment.pointer(menu.adjustment_assets(), view.pointer);
                adjustment.prepare(menu.adjustment_assets(), &self.renderer)?;
                if let Some(panel) = &adjustment.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
            }
            if let Some(menu) = &self.menu {
                if let Some(planning) = &mut view.planning {
                    planning.sync_model_variants(&self.assets.inputs.build_file)?;
                    if let Some(grid) = planning.grid_overlay(&self.renderer)? {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, &grid);
                    }
                    menu.planning_assets().prepare(&self.renderer, planning)?;
                    if let Some(panel) = &planning.prepared {
                        self.renderer
                            .draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                } else if !view.strategic
                    && view.main_panel_visible()
                    && view.details_visible
                    && view.ordinary_toolbar_category(&self.assets.selection.blocks).is_none()
                    && view._fresh_session.is_some()
                    && view.selected.is_some_and(|s| s.building.is_none())
                    && !view.selected.is_some_and(|s| {
                        view.live_status.as_ref().is_some_and(|status| {
                            status
                                .auction_locked_categories
                                .contains(&self.assets.selection.blocks[s.category].id)
                        })
                    })
                {
                    let [l, t, r, b] = menu.planning_assets().entry_bounds;
                    let hovered = view.pointer.is_some_and(|p| {
                        let [x, y] = p.map(|v| v as f32 * 0.5);
                        x >= l && x <= r && y >= t && y <= b
                    });
                    let icon = menu.planning_assets().entry(&self.renderer, hovered)?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &icon);
                }
            }
            if let Some(menu) = &self.menu {
                view.auction_notice.prepare(
                    menu.auction_notice_assets(),
                    &self.renderer,
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                )?;
                if let Some(panel) = &view.auction_notice.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
                let notices = view
                    .live_status
                    .as_ref()
                    .map(|s| s.notices.as_slice())
                    .unwrap_or(&[]);
                view.notifications.prepare(
                    menu.notification_assets(),
                    &self.renderer,
                    if view.strategic { &[] } else { notices },
                    view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                )?;
                if let Some(panel) = &view.notifications.prepared {
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, panel);
                }
            }
            if let (Some(menu), Some(status)) = (&self.menu, &view.live_status) {
                let pointer = view.pointer.map(|p| p.map(|v| v as f32 * 0.5));
                let entry = menu.card_assets().entry_scene(
                    &self.renderer,
                    status.card_entry,
                    status.card_visual,
                    &view.card_input,
                    pointer,
                )?;
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, &entry);
                if let Some(card) = status.card {
                    let panel = menu.card_assets().prepare(
                        &self.renderer,
                        card,
                        &view.card_input,
                        pointer,
                    )?;
                    self.renderer
                        .draw_overlay(&self.color_view, &self.depth_view, &panel);
                }
            }
            Self::draw_tutorial_visual(
                &self.assets, &self.menu, &self.renderer,
                &self.color_view, &self.depth_view, view,
                )?;
            if let Some(menu)=&self.menu {
                view.stats_panel.update(menu.stats_assets(),&self.renderer,
                    view.live_status.as_ref().and_then(|status|status.stats.as_ref()),
                    )?;
                if let Some(panel)=&view.stats_panel.prepared {
                    self.renderer.draw_overlay(&self.color_view,&self.depth_view,panel);
                }
            }
            if view.prompts.request().is_some()
                && let Some(menu) = &self.menu
            {
                view.prompts.cursor(
                    view.pointer
                        .map(|p| p.map(|v| v as f32 * 0.5))
                        .unwrap_or([-1.; 2]),
                );
                let panel = menu
                    .prompt_assets()
                    .prepare_positioned(&self.renderer, &view.prompts,
                        view.live_status.as_ref().map_or([0.; 2], |status| {
                            status.tutorial_interaction.prompt_offset()
                        }),
                    )?;
                self.renderer
                    .draw_overlay(&self.color_view, &self.depth_view, &panel);
            }
            if view.prompts.request().is_none()
                && view.tutorial_message.request().is_some()
                && let Some(menu) = &self.menu
            {
                let panel = menu.prompt_assets()
                    .prepare_message_positioned(&self.renderer, &view.tutorial_message,
                        view.live_status.as_ref().map_or([0.; 2], |status| {
                            status.tutorial_interaction.prompt_offset()
                        }),
                    )?;
                self.renderer.draw_overlay(&self.color_view, &self.depth_view, &panel);
            }
            }
            if view.options_visible() {
                if view.save_visible() {
                    let panel=self.assets.save_dialog.prepare(&self.renderer,
                        &view.save_dialog,view.pointer.map(|p|p.map(|v|v as f32*0.5)),
                    )?;
                    self.renderer.draw_overlay(&self.color_view,&self.depth_view,&panel);
                    if let Some(modal)=self.assets.save_dialog.prepare_name_modal(&self.renderer,
                        &view.save_dialog,view.pointer.map(|p|p.map(|v|v as f32*0.5)),
                    )? {
                        self.renderer.draw_overlay(&self.color_view,&self.depth_view,&modal);
                    }
                    if let Some((dialog,pressed_button))=view.save_dialog.generic_prompt()
                        && let Some(menu)=&self.menu
                    {
                        let panel=menu.prompt_assets().prepare_dialog(&self.renderer,dialog,
                            view.pointer.map(|p|p.map(|v|v as f32*0.5)).unwrap_or([-1.;2]),
                            pressed_button,
                        )?;
                        self.renderer.draw_overlay(&self.color_view,&self.depth_view,&panel);
                    }
                } else if view.settings_visible() {
                    view.settings_menu.update_levels(&self.assets.settings_menu, &self.renderer,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)), self.audio_levels,
                    )?;
                    if let Some(panel) = &view.settings_menu.prepared {
                        self.renderer.draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                } else {
                    view.game_options.update(&self.assets.game_options, &self.renderer,
                        view.pointer.map(|p| p.map(|v| v as f32 * 0.5)),
                    )?;
                    if let Some(panel) = &view.game_options.prepared {
                        self.renderer.draw_overlay(&self.color_view, &self.depth_view, panel);
                    }
                }
            }
            return presenter
                .present_binding(&self.binding)
                .map_err(|e| e.to_string());
        }

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
                self.menu_camera.as_mut().ok_or("menu camera missing")?
                    .sample(tick, phase)
            } else {
                self.assets.samples[sample]
            };
            self.sidewalk_camera = camera;
            let mut changed = self.assets.scene.reframe(camera)?;
            if let Some(menu) = &self.menu {
                changed.extend(self.assets.scene.apply_normal_star_detail(menu.settings.graphics.detail, camera)?);
                changed.sort_unstable();
                changed.dedup();
            }
            let updates = changed.into_iter().map(|index|
                (index, self.assets.scene.draws[index].borrowed())).collect::<Vec<_>>();
            self.renderer.update_dynamic_vertices_batch(&mut self.prepared, &updates)?;
            let cameras = self.assets.scene.draws.iter().map(|draw|
                draw.view_projection.unwrap_or(self.assets.scene.camera)).collect::<Vec<_>>();
            self.renderer.update_cameras(&self.prepared, &cameras, self.assets.scene.fog)?;
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
        self.assets.sidewalk_labels.update(&self.renderer, &mut self.sidewalk_labels,
            &mut self.sidewalk_shading, &self.assets.scene, self.sidewalk_camera, 35,
        )?;
        if tick >= self.assets.samples.len() && !self.ended && self.menu.is_none() {
            self.ended = true;
            self.open_menu();
            eprintln!("FlybyEnded: main menu");
            presenter.window.set_title("OpenMT2001 - main menu");
        }
        let submit_started = presenter.profile_menu_prepare(prepare_started);
        self.renderer
            .draw_prepared(&self.color_view, &self.depth_view, &self.prepared);
        self.renderer
            .draw_world_overlay(&self.color_view, &self.depth_view, &self.sidewalk_labels);
        self.renderer
            .draw_world_overlay(&self.color_view, &self.depth_view, &self.mr_monopoly);
        if let Some(menu) = &mut self.menu {
            menu.draw(&self.renderer, &self.color_view, &self.depth_view)?;
        }
        presenter.profile_menu_submit(submit_started);
        presenter
            .present_binding(&self.binding)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    /// Original-input regression. Serialization is entirely
    /// in memory; this never launches the original executable or writes a save.
    #[test]
    #[ignore = "requires supplied original saves/scripts/assets; retirement world/clock regression"]
    fn retired_camera_host_keeps_world_clock_and_committed_timers() {
        std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
            let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
            let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
            let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let executable = read("OPENMT2001_EXE");
            let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let inputs = SceneInputs {
                process_runtime:None,
                compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable), time_runtime: Default::default(),
                // This test never creates or writes this path.
                profile_root: PathBuf::from("unused-camera-retirement-test-profiles"),
                character_names: crate::fresh_city::character_names(
                    &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                    &exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                ).unwrap(),
                installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                maps: map.parent().unwrap().to_path_buf(), build_file: read("OPENMT2001_BUILD_FILE"),
                executable, directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"), suncols: read("OPENMT2001_SUNCOLS"),
            };
            let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
                up: [0., 1., 0.], vertical_fov: 45. };
            let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
            let board = inputs.styled_board(&image).unwrap();
            let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
            let assets = crate::fresh_city::CommandAssets { image: &image, board: &board, file: &file };
            for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
                let (loaded, mut session) = inputs.saved_preview(
                    &PathBuf::from(std::env::var(key).unwrap()), camera,
                    &mut SnapshotCache::default()).unwrap();
                let capture = loaded.imported_cameras.unwrap();
                let index=openmt2001::archive::ArchiveIndex::parse(&inputs.directory,inputs.archive.len() as u64).unwrap();
                let selection=crate::selection::Assets::load(&inputs.executable,&index,&inputs.archive).unwrap();
                let pavement=selection.pavement_resource().binding().unwrap().handle();
                let mut host = crate::live_host::Host::new(camera.eye);
                let (worker, inbox) = crate::live_host::camera_bridge::channel(session.world.topology.epoch().serial());
                let install=camera_live::endpoint::PropertyInstall::capture(&selection,&image).unwrap();
                let ui = std::thread::spawn(move || {
                    let owner=camera_live::owner::Owner::new(openmt2001::camera::bank::Bank::loader_zero_storage(),1);
                    let mut endpoint=camera_live::endpoint::Endpoint::with_property_install(owner,inbox,install);
                    let mut context=camera_live::endpoint::tests::Ui::default();
                    assert_eq!(endpoint.wait_service(&mut context,std::time::Duration::from_secs(2)).unwrap(),
                        crate::live_host::camera_bridge::Service::Completed);
                    assert_eq!(endpoint.owner().property_descriptors().unwrap().count().unwrap(),52);
                    assert!(endpoint.camera_only_admission(),"CameraOnly must not capture C/F suppliers");
                    let owner=endpoint.into_owner();
                    assert!(owner.property_lifecycle_error().is_none());
                });
                host.install_camera_bridge(&mut session, &image, crate::live_host::CameraSupplierPolicy::CameraOnly, worker, capture, pavement).unwrap();
                ui.join().unwrap();
                assert!(host.camera_retired());
                let before = session.save_v2(&image, capture.into_records(), 1).unwrap();
                let timing = session.world.clock.timing;
                let calendar = session.world.clock.calendar;
                let timers = session.hooks.timers.clone();
                let interface_ticks = host.interface_ticks();
                // Even a stale running bit and catch-up-sized delta cannot
                // dispatch a retired session's interface or world callbacks.
                host.running = true;
                host.frame(&mut session, &assets, 1000);
                host.tick(&mut session, &assets);
                assert!(host.options(&mut session.world.pause,&mut session.hooks.timers,false).is_err());
                host.set_running(true);
                assert!(!host.running && host.failure.is_none());
                assert_eq!(session.world.clock.timing, timing);
                assert_eq!(session.world.clock.calendar, calendar);
                assert_eq!(session.hooks.timers, timers);
                assert_eq!(host.interface_ticks(), interface_ticks);
                assert_eq!(session.save_v2(&image, capture.into_records(), 1).unwrap(), before);
            }
        }).unwrap().join().unwrap();
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original Time assets and offscreen GPU; release performance measurement"]
    fn original_time_panel_texture_preparation_performance() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
        let parameters = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameters.parent().unwrap().parent().unwrap();
        let language = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent().unwrap().parent().unwrap().join("US/language/usa");
        let assets = crate::time_panel::Assets::load(&index, &archive, installer, &language, &executable).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let prepare = |fraction| {
            assets.prepare(&renderer, crate::time_panel::Display {
            year: 1930, hour: 9, speed: 1, name: "JUSTIN", cash: Some(15000.),
            fraction, character: Some(4), cash_icon_phase: 0,
        },
                ).unwrap()
        };
        renderer.bypass_texture_cache_for_capture(true);
        let started = Instant::now();
        for fraction in 0..100 { std::hint::black_box(prepare(fraction)); }
        let uncached = started.elapsed();
        renderer.bypass_texture_cache_for_capture(false);
        prepare(0);
        let started = Instant::now();
        for fraction in 0..100 { std::hint::black_box(prepare(fraction)); }
        let cached = started.elapsed();
        let mut retained = prepare(0);
        let started = Instant::now();
        for fraction in 0..100 {
            assets.refresh_for_test(&renderer, &mut retained, crate::time_panel::Display {
                year: 1930, hour: 9, speed: 1, name: "JUSTIN", cash: Some(15000.),
                fraction, character: Some(4), cash_icon_phase: 0,
            },
                ).unwrap();
        }
        eprintln!("Original Time panel prepare (100 changing clock fractions): uncached {uncached:?}, resident {cached:?}, retained {:?}", started.elapsed());
    }

    fn test_board_style(executable: &[u8]) -> openmt2001::board_style::Style {
        let parameters = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        crate::block_sidewalk_labels::load_style(
            &openmt2001::pe::PeImage::parse(executable).unwrap(),
            parameters.parent().unwrap().parent().unwrap(),
        ).unwrap()
    }

    #[cfg(feature = "capture")]
    mod rights_worker_test {
        include!("intro_rights_test.rs");
    }
    #[cfg(feature = "capture")]
    mod auction_save_test {
        include!("intro_auction_save_test.rs");
    }
    #[cfg(feature = "capture")]
    mod strategic_capture_test {
        include!("intro_strategic_capture_test.rs");
    }
    use super::*;

    #[cfg(target_os = "linux")]
    mod toolbar_right_dispatch_test {
        include!("toolbar_right_dispatch_test.rs");
    }

    #[cfg(feature = "capture")]
    mod menu_identity_test {
        include!("intro_menu_identity_test.rs");
    }

    #[test]
    fn normal_city_entry_keeps_native_pose_with_53_degree_projection() {
        let (sample, controls) = fresh_city_camera();
        assert_eq!(sample.eye, [26., 586., -695.]);
        assert_eq!(sample.target, [26., 463., -595.]);
        assert_eq!(sample.up, [0., 1., 0.]);
        assert_eq!(sample.vertical_fov, 53.);
        assert_eq!(controls.center, [51., 463., -595.]);
        assert_eq!(controls.height, 586.);
        assert_eq!(controls.radius, 100.);
        assert_eq!(controls.angle, -std::f32::consts::FRAC_PI_2);
    }

    #[test]
    fn normal_saved_city_selects_primary_pose_and_owns_fov() {
        let mut records = [[0u8; 196]; 2];
        let poses = [
            ([82., 120., -454.5], [82., 0.25, -374.5], [0., 1., 0.]),
            ([410., 80., -60.], [10., 2., 20.], [0., 0., 1.]),
        ];
        for (record, (eye, target, up)) in records.iter_mut().zip(poses) {
            for (offset, values) in [(0x4c, eye), (0x58, target), (0x64, up)] {
                for (axis, value) in values.into_iter().enumerate() {
                    let at = offset + axis * 4;
                    record[at..at + 4].copy_from_slice(&f32::to_le_bytes(value));
                }
            }
        }
        let (sample, controls) = normal_saved_city_viewpoint(&records).unwrap();
        assert_eq!((sample.eye, sample.target, sample.up), poses[0]);
        assert_eq!(sample.vertical_fov, 53.);
        assert_eq!(controls.radius, 80.);
        assert_eq!(controls.height, 120.);
        // Other owners retain the caller-selected FOV and record.
        let generic = saved_viewpoint(&records[1], 61.).unwrap().0;
        assert_eq!((generic.eye, generic.target, generic.up), poses[1]);
        assert_eq!(generic.vertical_fov, 61.);
        // An invalid primary must not silently select the usable backup.
        records[0][0x4c..0x50].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(normal_saved_city_viewpoint(&records).is_err());
    }

    #[test]
    #[ignore = "requires supplied v1.4 PE and both original saves; read-only camera contract"]
    fn supplied_normal_city_camera_matches_pe_fov_and_projection() {
        let executable = fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let word = |address| u32::from_le_bytes(image.at(address, 4).unwrap().try_into().unwrap());
        assert_eq!(word(0x545acc), 53f32.to_bits(), "normal reset FOV data");
        assert_eq!(word(0x52af24), 0x4f1550, "renderer slot14 projection setter");
        assert_eq!(word(0x52b0bc), 0x3c0efa1e, "original half-degree factor");
        assert_eq!(fresh_city_camera().0.vertical_fov.to_bits(), word(0x545acc));
        for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let bytes = fs::read(std::env::var(key).unwrap()).unwrap();
            let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
            let records = save.cameras().unwrap().map(|row| *row);
            let sample = normal_saved_city_viewpoint(&records).unwrap().0;
            let primary = |offset| f32::from_le_bytes(
                records[0][offset..offset + 4].try_into().unwrap());
            assert_eq!(sample.eye, [primary(0x4c), primary(0x50), primary(0x54)], "{key}");
            assert_eq!(sample.target, [primary(0x58), primary(0x5c), primary(0x60)], "{key}");
            assert_eq!(sample.up, [primary(0x64), primary(0x68), primary(0x6c)], "{key}");
            assert_eq!(sample.vertical_fov.to_bits(), word(0x545acc), "{key}");
            let [near, far] = openmt2001::camera::city_clip(sample.eye[1]).unwrap();
            let matrix = openmt2001::camera::projection(sample.vertical_fov, near, far,
                1280, 960, [0., 1., 0., 1.]).unwrap();
            // Original004f1480 full-clip focal scales at53 degrees and4:3.
            // Specified viewport only; not a claim about the recording's aspect.
            assert!((matrix[5] - 2.0056954286f32).abs() < 0.000001, "{key} vertical focal scale");
            assert!((matrix[0] - 1.5042715714f32).abs() < 0.000001, "{key} aspect scaling");
            assert_eq!((matrix[8], matrix[9], matrix[11], matrix[15]), (0., 0., 1., 0.));
        }
    }

    #[test]
    #[ignore = "requires original board and supplied saved business memberships"]
    fn original_category_hyperlinks_keep_native_identity() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let exe = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&exe).unwrap();
        let board = openmt2001::board::Board::from_v14(&image).unwrap();
        // Real board hyperlinks above both holes. Building ordinals are
        // controlled inputs here; they do not claim a natural high-ID business.
        let mut high_ids = 0;
        for block in board.blocks.iter().filter(|block| block.id >= 46) {
            let hit = inspection_hit(&board.blocks, block.id, Some(7)).unwrap();
            assert_eq!(board.blocks[hit.category].id, block.id);
            assert_eq!(hit.category + 2, block.id);
            assert_eq!(hit.building, Some(7));
            high_ids += 1;
        }
        assert_eq!(high_ids, 6);
        assert!(inspection_hit(&board.blocks, 40, None).is_none());
        assert!(inspection_hit(&board.blocks, 45, None).is_none());
        let save = read("OPENMT2001_SAVE_ONE");
        let saved = openmt2001::save::SaveFile::parse(&save).unwrap();
        let buildings = saved.buildings().unwrap();
        assert!(!buildings.is_empty());
        // These original retained memberships establish the scene ID domain.
        for (ordinal, building) in buildings.iter().enumerate() {
            let hit = inspection_hit(&board.blocks, building.category, Some(ordinal)).unwrap();
            assert_eq!(board.blocks[hit.category].id, building.category);
            assert_eq!(hit.building, Some(ordinal));
        }
    }

    #[test]
    fn construction_snapshot_cadence_is_bounded_to_active_growth() {
        assert_eq!(
            city_snapshot_interval(true),
            std::time::Duration::from_millis(250)
        );
        assert_eq!(
            city_snapshot_interval(false),
            std::time::Duration::from_secs(1)
        );
    }

    #[test]
    fn menu_camera_retains_flyby_and_uses_native_tenth_step() {
        let flyby = CameraSample {
            eye: [-40., 20., 20.], target: [-45., 10., 25.],
            up: [0., 1., 0.], vertical_fov: 45.,
        };
        let mut camera = MenuCamera::new(flyby);
        assert_eq!(camera.sample(0, 0.), flyby);
        let first = camera.sample(1, 0.);
        assert_eq!(first.eye[1], 21.);
        assert_eq!(first.eye[2], 25.4);
        assert_eq!(first.target, [-45.8, 9.5, 26.9]);
        let before = camera.eye_start;
        let presented = camera.sample(1, 1.);
        assert!((presented.eye[1] - 21.9).abs() < 0.00001);
        assert_eq!(camera.eye_start, before, "presentation must not advance the tick");
        let second = camera.sample(2, 0.);
        assert!((second.eye[1] - 21.9).abs() < 0.00001);
        assert_eq!(camera.tick, 2);
    }

    #[test]
    #[ignore = "requires original top-right panel mesh"]
    fn time_panel_hitboxes_at_default_window_size() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
            .unwrap();
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let executable = read("OPENMT2001_EXE");
        let language = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent().unwrap().parent().unwrap().join("US/language/usa");
        let panel = crate::time_panel::Assets::load(&index, &archive, installer,
            &language, &executable).unwrap();
        let hit = |screen| {
            let point = internal_pointer(screen, [960, 720]).unwrap();
            panel.button(point.map(|v| v as f32 * 0.5))
        };
        assert_eq!(hit([880., 26.]), Some(crate::time_panel::Button::Options));
        let options = internal_pointer([910., 26.], [960, 720]).unwrap();
        assert!(panel.contains(options.map(|v| v as f32 * 0.5)));
        assert_eq!(hit([930., 692.]), Some(crate::time_panel::Button::Speed));
        assert_eq!(hit([875., 692.]), Some(crate::time_panel::Button::Manager));
        let manager = internal_pointer([875., 692.], [960, 720]).unwrap();
        assert!(panel.contains(manager.map(|v| v as f32 * 0.5)));
        let compass = internal_pointer([818., 692.], [960, 720]).unwrap();
        assert!(!panel.contains(compass.map(|v| v as f32 * 0.5)),
            "compass retains its own navigation hitbox");
        let outside = internal_pointer([730., 26.], [960, 720]).unwrap();
        assert!(!panel.contains(outside.map(|v| v as f32 * 0.5)));
        let letterboxed = internal_pointer([1090., 692.], [1280, 720]).unwrap();
        assert_eq!(panel.button(letterboxed.map(|v| v as f32 * 0.5)),
            Some(crate::time_panel::Button::Speed));
        let manager_letterboxed=internal_pointer([1035.,692.],[1280,720]).unwrap();
        assert_eq!(panel.button(manager_letterboxed.map(|v|v as f32*0.5)),
            Some(crate::time_panel::Button::Manager));
        assert!(internal_pointer([50., 692.], [1280, 720]).is_none());
    }

    #[test]
    #[ignore = "requires original menu mesh and language assets"]
    fn game_options_hitboxes_at_default_and_letterboxed_window_size() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
            .unwrap();
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let executable = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = executable.parent().unwrap().parent().unwrap().join("US/language/usa");
        let assets = crate::game_options::Assets::load(&index, &archive, installer,
            &language).unwrap();
        let labels = assets.labels().iter().map(|row| row.iter().copied()
            .map(|b| b + 32).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        assert_eq!(labels, [b"CONTINUE".to_vec(), b"SAVE GAME".to_vec(),
            b"SETTINGS".to_vec(), b"QUIT".to_vec()]);
        let bounds = assets.bounds();
        assert_eq!(bounds[0], [79., 161.77893, 568., 195.77893]);
        let executable_bytes = fs::read(&executable).unwrap();
        let image = openmt2001::pe::PeImage::parse(&executable_bytes).unwrap();
        for (i, expected) in [b"MAIN_BUTT02", b"MAIN_BUTT03", b"MAIN_BUTT04", b"MAIN_BUTT05",
        ]
            .into_iter().enumerate() {
            let native = image.at(0x543a68 + i as u32 * 20, 20).unwrap();
            let name = u32::from_le_bytes(native[..4].try_into().unwrap());
            assert_eq!(image.at(name, expected.len()).unwrap(), expected);
        }
        let actions = [crate::game_options::Button::Continue,
            crate::game_options::Button::Save,
            crate::game_options::Button::Settings,
            crate::game_options::Button::Quit,
        ];
        let mut state = crate::game_options::View::default();
        state.open(true);
        assert!(state.open && state.resume_running);
        for (i, [l,t,r,b]) in bounds.into_iter().enumerate() {
            let point = [(l+r)/2., (t+b)/2.];
            for (size, screen) in [([960,720], [point[0]*1.5, point[1]*1.5]),
                ([1280,720], [160.+point[0]*1.5, point[1]*1.5]),
            ] {
                let internal = internal_pointer(screen.map(f64::from), size).unwrap();
                assert_eq!(assets.button(internal.map(|v| v as f32 * 0.5)), Some(actions[i]));
            }
            assert_eq!(assets.button([l-1., (t+b)/2.]), None);
            assert_eq!(state.mouse(&assets, Some(point), true), None);
            assert_eq!(state.mouse(&assets, Some([l-1., (t+b)/2.]), false), None,
                "release outside the pressed row must cancel");
            assert_eq!(state.mouse(&assets, Some(point), true), None);
            assert_eq!(state.mouse(&assets, Some(point), false), Some(actions[i]));
        }
        state.set_save_disabled(true);
        let [l,t,r,b]=bounds[1];
        let save=[(l+r)*0.5,(t+b)*0.5];
        assert_eq!(state.mouse(&assets,Some(save),true),None);
        assert_eq!(state.mouse(&assets,Some(save),false),None,
            "native tutorial Save row is disabled before activation");
        assert!(state.close(), "Continue restores the prior running state");
        state.open(false);
        assert!(!state.close(), "a previously paused game remains paused");
        assert!(internal_pointer([50., 200.], [1280,720]).is_none());
    }

    #[test]
    #[ignore = "requires original Settings meshes and language assets"]
    fn settings_menu_hitboxes_at_default_and_letterboxed_window_size() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
            .unwrap();
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let executable = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = executable.parent().unwrap().parent().unwrap().join("US/language/usa");
        let assets = crate::settings_menu::Assets::load(&index, &archive, installer,
            &language).unwrap();
        for (id, expected) in [(1679,b"SETTINGS".as_slice()),(1607,b"Sound"),
            (1608,b"Graphics"),(1609,b"Gameplay"),(1610,b"Board"),
            (1770,b"Credits"),(1139,b"Reset to default"),(1127,b"OK"),
        ] {
            let decoded = assets.label_for_test(id).iter().map(|&b| b+32).collect::<Vec<_>>();
            assert_eq!(decoded, expected, "original Settings text ID {id}");
        }
        let page = crate::settings_menu::Page::Main;
        assert_eq!(assets.bounds(page,crate::settings_menu::Button::Sound).unwrap(),
            [79.,111.52376,568.,145.52376]);
        let rows = [crate::settings_menu::Button::Sound,
            crate::settings_menu::Button::Graphics,
            crate::settings_menu::Button::Gameplay,
            crate::settings_menu::Button::Back,
        ];
        let mut state = crate::settings_menu::View::default();
        let mut levels = crate::settings_menu::Levels::default();
        state.open();
        for button in rows {
            let [l,t,r,b] = assets.bounds(page,button).unwrap();
            let point = [(l+r)/2.,(t+b)/2.];
            for (size, screen) in [([960,720],[point[0]*1.5,point[1]*1.5]),
                ([1280,720],[160.+point[0]*1.5,point[1]*1.5]),
            ] {
                let internal = internal_pointer(screen.map(f64::from),size).unwrap();
                assert_eq!(assets.button(page,internal.map(|v| v as f32*0.5)),Some(button));
            }
            state.mouse(&assets,Some(point),true,&mut levels);
            state.mouse(&assets,Some([l-1.,(t+b)/2.]),false,&mut levels);
            assert_eq!(state.page,Some(page),"different release target cancels");
            state.mouse(&assets,Some(point),true,&mut levels);
            state.mouse(&assets,Some(point),false,&mut levels);
            if button == crate::settings_menu::Button::Back {
                assert_eq!(state.page,None);
            } else {
                assert_ne!(state.page,Some(page));
                let subpage = state.page.unwrap();
                let back = assets.bounds(subpage,crate::settings_menu::Button::Back).unwrap();
                let center = [(back[0]+back[2])/2.,(back[1]+back[3])/2.];
                state.mouse(&assets,Some(center),true,&mut levels);
                state.mouse(&assets,Some(center),false,&mut levels);
                assert_eq!(state.page,Some(page));
            }
        }
        assert!(internal_pointer([50.,200.],[1280,720]).is_none());
    }

    #[test]
    #[ignore = "requires original Sound mesh and v1.4 executable"]
    fn sound_settings_original_slider_input_and_dispatch() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        // 49bd0f/49bd8c create slider groups 4/5, each with max 100.
        assert_eq!(image.at(0x49bd0f,2).unwrap(), &[0x6a,4]);
        assert_eq!(image.at(0x49bd88,2).unwrap(), &[0x6a,5]);
        assert_eq!(image.at(0x49bd18,5).unwrap(), &[0xbe,0,0,0xc8,0x42]);
        assert_eq!(image.at(0x4c1902,10).unwrap(),
            &[0xc7,0x05,0xd8,0x06,0x92,0x00,0x3f,0x00,0x00,0x00]);
        assert_eq!(image.at(0x4c190c,10).unwrap(),
            &[0xc7,0x05,0xdc,0x06,0x92,0x00,0x53,0x00,0x00,0x00]);
        let call = |at: u32| {
            let bytes = image.at(at,5).unwrap();
            assert_eq!(bytes[0],0xe8);
            (at+5).wrapping_add_signed(i32::from_le_bytes(bytes[1..5].try_into().unwrap()))
        };
        assert_eq!(call(0x49c211),0x4af3f0);
        assert_eq!(call(0x49c22d),0x4af410);
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let index = openmt2001::archive::ArchiveIndex::parse(&directory,archive.len() as u64)
            .unwrap();
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let language = exe_path.parent().unwrap().parent().unwrap().join("US/language/usa");
        let assets = crate::settings_menu::Assets::load(&index,&archive,installer,&language)
            .unwrap();
        use crate::settings_menu::{Button,Levels,Page,View};
        let mut state=View::default();
        let mut levels=Levels::default();
        state.open();
        state.mouse(&assets,Some([320.,128.]),true,&mut levels);
        state.mouse(&assets,Some([320.,128.]),false,&mut levels);
        assert_eq!(state.page,Some(Page::Sound));
        for button in [Button::MusicTrack,Button::EffectsTrack,Button::MusicLeft,
            Button::MusicRight,Button::EffectsLeft,Button::EffectsRight,
        ] {
            let [l,t,r,b]=assets.bounds(Page::Sound,button).unwrap();
            let center=[(l+r)/2.,(t+b)/2.];
            for (size,screen) in [([960,720],[center[0]*1.5,center[1]*1.5]),
                ([1280,720],[160.+center[0]*1.5,center[1]*1.5]),
            ] {
                let internal=internal_pointer(screen.map(f64::from),size).unwrap();
                assert_eq!(assets.button(Page::Sound,internal.map(|v| v as f32*0.5)),Some(button));
            }
        }
        let track=assets.bounds(Page::Sound,Button::MusicTrack).unwrap();
        let y=(track[1]+track[3])/2.;
        let native_track=crate::ui::slider_track(track.map(f32::trunc),false);
        assert!(state.mouse(&assets,Some([native_track[0]+1.,y]),true,&mut levels));
        assert_eq!(levels.music,0);
        assert!(state.drag(&assets,Some([native_track[0]+(native_track[2]-native_track[0])*0.37,y]),
            &mut levels));
        assert_eq!(levels.music,37);
        state.mouse(&assets,Some([track[2],y]),false,&mut levels);
        let arrow=assets.bounds(Page::Sound,Button::MusicRight).unwrap();
        let center=[(arrow[0]+arrow[2])/2.,(arrow[1]+arrow[3])/2.];
        state.mouse(&assets,Some(center),true,&mut levels);
        assert!(!state.mouse(&assets,Some([arrow[0]-1.,center[1]]),false,&mut levels));
        assert_eq!(levels.music,37);
        state.mouse(&assets,Some(center),true,&mut levels);
        assert!(state.mouse(&assets,Some(center),false,&mut levels));
        assert_eq!(levels.music,38);
        // The original Sound panel has no Reset child; only the native
        // slider and arrow controls may change these levels.
        assert_eq!(levels.effects, Levels::default().effects);
    }

    // Whole original-data city assemblies plus visible retained-resource probes.
    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires supplied originals and GPU"]
    fn original_city_refresh_matches_independent_prepare() {
        // Parent must supply a unique on-disk directory; never use shared /tmp.
        let parent = PathBuf::from(std::env::var("OPENMT2001_EQ_SCRATCH").unwrap());
        assert!(parent.is_absolute() && parent.is_dir());
        let owned = parent.join(format!("city-refresh-{}", std::process::id()));
        fs::create_dir(&owned).expect("scratch must be uniquely owned");
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); }
        }
        let _cleanup = Cleanup(owned.clone());
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let save = read("OPENMT2001_SAVE_ONE");
        let map = read("OPENMT2001_INTRO_MAP");
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        eprintln!("equivalence adapter: {}", renderer.adapter_name);
        let compare = |fresh: &openmt2001::render::PreparedScene,
                       retained: &openmt2001::render::PreparedScene, label: &str| {
            let a = owned.join("fresh.png");
            let b = owned.join("retained.png");
            crate::capture_test::capture_standalone(&renderer, fresh, a.to_str().unwrap());
            crate::capture_test::capture_standalone(&renderer, retained, b.to_str().unwrap());
            assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap(), "{label}");
        };
        for saved in [true, false] {
            let mut retained = None;
            let mut previous_materials = None;
            let mut material_transition = false;
            for hour in [9, 10, 11, 21, 9] {
                // Complete independent assembly, original order, no worker/world step.
                let scene = render_scene::assemble(Request {
                    build_file: &build, source: if saved { Source::Save(&save) } else { Source::Map(std::str::from_utf8(&map).unwrap()) },
                    original: Some(OriginalAssets { executable: &executable,
                        directory: &directory, archive: &archive,
                    }),
                    suncols: Some(&suncols), phase: Some(hour), camera: None,
                    width: 1280, height: 960, animated: true,
                }).unwrap();
                let materials = scene.draws.iter().map(|d| {
                        (d.mode, d.secondary.as_ref().map(|t| {
                    use std::hash::Hasher;
                    let mut hash = std::collections::hash_map::DefaultHasher::new();
                    hash.write(t.rgba.as_ref());
                    (t.width, t.height, t.mipmaps, hash.finish())
                }),
                        )
                    }).collect::<Vec<_>>();
                if let Some(old) = &previous_materials { material_transition |= old != &materials; }
                previous_materials = Some(materials);
                let draws = scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>();
                let fresh = renderer.prepare(&draws, scene.camera, scene.fog).unwrap();
                if let Some(old) = &mut retained {
                    renderer.refresh_prepared(old, &draws, scene.camera, scene.fog).unwrap();
                } else { retained = Some(renderer.prepare(&draws, scene.camera, scene.fog).unwrap()); }
                compare(&fresh, retained.as_ref().unwrap(), &format!("complete city hour {hour}"),
                );
            }
            assert!(material_transition, "fixture must exercise day/night material or secondary transition");
        }
        // Visible diagnostic contract probes supplement potentially occluded city data.
        // No invented city assembly/lighting rules: these are explicitly synthetic.
        use openmt2001::render::{SecondaryTexture, SurfaceMode, TexturedDraw};
        let identity = [1.,0.,0.,0., 0.,1.,0.,0., 0.,0.,1.,0., 0.,0.,0.,1.,
        ];
        let triangle = [[-0.8,-0.8,0.2,1.,1.,1.,0.,0.],
                        [0.8,-0.8,0.2,1.,1.,1.,1.,0.],
                        [0.,0.8,0.2,1.,1.,1.,0.5,1.],
        ];
        let mut retained = None;
        let mut previous_pixels = None;
        for case in 0..10 {
            let count = [3,6,3,0,3,3,3,3,3,3][case];
            let mut vertices = triangle.repeat(count / 3);
            if count == 6 { for v in &mut vertices[3..] { v[0] *= 0.5; v[1] += 0.1; } }
            let uv = vec![[0.3,0.7]; count];
            let specular = vec![[0.1,0.0,0.0]; count];
            let alpha = vec![128; count];
            let primary = [255, if case < 5 { 80 } else { 180 }, 20, 255];
            let secondary = [20, 40, if case == 7 { 100 } else { 240 }, 255];
            let blend = case >= 6;
            let draw = TexturedDraw {
                view_projection: (case >= 5).then_some(identity), vertices: &vertices,
                specular: Some(&specular), diffuse_alpha: Some(&alpha),
                mode: if blend { SurfaceMode::FacadeBlend } else { SurfaceMode::StaticLit },
                width: 1, height: 1, rgba: &primary, mipmaps: case >= 7,
                secondary: blend.then_some(SecondaryTexture { uv: &uv, diffuse_alpha: 120,
                    width: 1, height: 1, rgba: &secondary, mipmaps: case >= 7,
                }),
            };
            let mut camera = identity;
            if case >= 5 { camera[12] = 0.2; } // override must win
            let draws = if case == 8 { Vec::new() } else { vec![draw] };
            let fog = (case >= 5).then_some(openmt2001::render::EyeFog { color: 0x406080, start: 0.0, end: 2.0,
            });
            let fresh = renderer.prepare(&draws, camera, fog).unwrap();
            if let Some(old) = &mut retained {
                renderer.refresh_prepared(old, &draws, camera, fog).unwrap();
            } else { retained = Some(renderer.prepare(&draws, camera, fog).unwrap()); }
            compare(&fresh, retained.as_ref().unwrap(), &format!("probe {case}"));
            let pixels = fs::read(owned.join("fresh.png")).unwrap();
            if matches!(case, 3|4|5|7) { assert_ne!(previous_pixels.as_ref().unwrap(), &pixels,
                "geometry/texture controls must visibly change"); }
            previous_pixels = Some(pixels);
        }
        // Replace only one of two surfaces, leaving a shared camera allocation
        // beside a standalone replacement buffer. Then update both geometries,
        // global camera/fog and a per-draw override across further refreshes.
        let mut mixed = None;
        let mut previous = None;
        for case in 0..5 {
            let left = triangle.iter().map(|v| {
                let mut v = *v; v[0] = v[0] * 0.4 - 0.5; v
            }).collect::<Vec<_>>();
            let right = triangle.iter().map(|v| {
                let mut v = *v; v[0] = v[0] * 0.4 + 0.5; v
            }).collect::<Vec<_>>();
            let left_pixels = [255, if case == 0 { 20 } else { 180 }, 20, 255];
            let right_pixels = [20, 120, if case < 3 { 255 } else { 80 }, 255];
            let mut camera = identity;
            camera[13] = case as f32 * 0.07;
            let fog = Some(openmt2001::render::EyeFog {
                color: 0x406080, start: 0.0, end: 2.0,
            });
            let draws = [
                TexturedDraw { view_projection: None,
                    vertices: if case == 4 { &[] } else { &left },
                    specular: None, diffuse_alpha: None, mode: SurfaceMode::StaticLit,
                    width:1, height:1, rgba:&left_pixels, mipmaps:false, secondary:None,
                },
                TexturedDraw { view_projection: Some(identity), vertices:&right,
                    specular: None, diffuse_alpha: None, mode: SurfaceMode::StaticLit,
                    width:1, height:1, rgba:&right_pixels, mipmaps:false, secondary:None,
                },
            ];
            let fresh = renderer.prepare(&draws, camera, fog).unwrap();
            if let Some(old) = &mut mixed {
                renderer.refresh_prepared(old, &draws, camera, fog).unwrap();
            } else { mixed = Some(renderer.prepare(&draws, camera, fog).unwrap()); }
            compare(&fresh, mixed.as_ref().unwrap(), &format!("mixed buffers {case}"),
            );
            let pixels = fs::read(owned.join("fresh.png")).unwrap();
            if let Some(previous) = &previous {
                assert_ne!(previous, &pixels, "mixed controls must visibly change");
            }
            previous = Some(pixels);
        }
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original intro assets and GPU"]
    fn menu_vertex_upload_batch_matches_individual_updates() {
        for phase in [0, 12] { verify_menu_vertex_upload_phase(phase); }
    }

    #[cfg(feature = "capture")]
    fn verify_menu_vertex_upload_phase(phase: u32) {
        eprintln!("Vertex batch original-city phase{phase}");
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let camera = CameraSample { eye: [-53., 30., 74.], target: [-53., 5., 44.],
            up: [0., 1., 0.], vertical_fov: 45.,
        };
        let mut scene = render_scene::assemble(Request {
            build_file: &build_file, source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets { executable: &executable,
                directory: &directory, archive: &archive,
            }),
            suncols: Some(&suncols), phase: Some(phase), camera: Some(camera),
            width: 1280, height: 960, animated: true,
        }).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let borrowed = scene.draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>();
        let mut individual = renderer.prepare(&borrowed, scene.camera, scene.fog).unwrap();
        let mut batch = renderer.prepare(&borrowed, scene.camera, scene.fog).unwrap();
        renderer.update_dynamic_vertices_batch(&mut batch, &[]).unwrap();
        let mut menu = MenuCamera::new(camera);
        let mut old_time = std::time::Duration::ZERO;
        let mut batch_time = std::time::Duration::ZERO;
        let mut reframe_time = std::time::Duration::ZERO;
        for tick in 0..100 {
            let started = Instant::now();
            let changed = scene.reframe(menu.sample(tick, 0.25)).unwrap();
            reframe_time += started.elapsed();
            let started = Instant::now();
            for (index, draw) in scene.draws.iter().enumerate() {
                if changed.contains(&index) {
                    renderer.update_dynamic_surface(&mut individual, index, &draw.borrowed(),
                        scene.camera, scene.fog,
                        ).unwrap();
                } else {
                    renderer.update_camera(&individual, index,
                        draw.view_projection.unwrap_or(scene.camera), scene.fog,
                        ).unwrap();
                }
            }
            old_time += started.elapsed();
            let started = Instant::now();
            let updates = changed.iter().map(|&index|
                (index, scene.draws[index].borrowed())).collect::<Vec<_>>();
            renderer.update_dynamic_vertices_batch(&mut batch, &updates).unwrap();
            let cameras = scene.draws.iter().map(|draw|
                draw.view_projection.unwrap_or(scene.camera)).collect::<Vec<_>>();
            renderer.update_cameras(&batch, &cameras, scene.fog).unwrap();
            batch_time += started.elapsed();
            if [0, 50, 99].contains(&tick) {
                let reference = format!("work/frontend/vertex-individual-{tick}.png");
                let actual = format!("work/frontend/vertex-batch-{tick}.png");
                crate::capture_test::capture_standalone(&renderer, &individual, &reference);
                crate::capture_test::capture_standalone(&renderer, &batch, &actual);
                assert_eq!(fs::read(reference).unwrap(), fs::read(actual).unwrap());
                let fresh = format!("work/frontend/vertex-fresh-{tick}.png");
                crate::capture_test::capture_scene_world_overlays(&renderer, &scene, &[], &[], &fresh,
                );
                assert_eq!(fs::read(fresh).unwrap(),
                    fs::read(format!("work/frontend/vertex-batch-{tick}.png")).unwrap());
            } else {
                // Flush staging uploads equally without adding offscreen rendering
                // to the CPU preparation measurements.
                renderer.queue.submit([]);
                renderer.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
        }
        // Individual APIs must still isolate each draw's camera and fog slots.
        let alternate = scene.fog.map(|mut fog| { fog.color ^= 0x00ff0000; fog });
        renderer.update_camera(&individual, 0, scene.camera, alternate).unwrap();
        renderer.update_camera(&batch, 0, scene.camera, alternate).unwrap();
        crate::capture_test::capture_standalone(&renderer, &individual,
            "work/frontend/vertex-individual-fog.png",
        );
        crate::capture_test::capture_standalone(&renderer, &batch,
            "work/frontend/vertex-batch-fog.png",
        );
        assert_eq!(fs::read("work/frontend/vertex-individual-fog.png").unwrap(),
            fs::read("work/frontend/vertex-batch-fog.png").unwrap());
        // A material replacement introduces a standalone uniform buffer. The
        // batch API must update that live buffer as well as the retained arena.
        let mut pixels = scene.draws[0].rgba.to_vec();
        pixels[0] ^= 255;
        scene.draws[0].rgba = pixels.into();
        let extra = scene.draws[0].vertices.clone();
        scene.draws[0].vertices.extend(extra);
        if let Some(values) = &mut scene.draws[0].specular { values.extend(values.clone()); }
        if let Some(values) = &mut scene.draws[0].diffuse_alpha { values.extend(values.clone()); }
        if let Some(secondary) = &mut scene.draws[0].secondary { secondary.uv.extend(secondary.uv.clone()); }
        renderer.update_dynamic_surface(&mut individual, 0, &scene.draws[0].borrowed(),
            scene.camera, scene.fog,
            ).unwrap();
        renderer.update_dynamic_vertices_batch(&mut batch, &[(0, scene.draws[0].borrowed())]).unwrap();
        let borrowed = scene.draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>();
        renderer.refresh_prepared(&mut individual, &borrowed, scene.camera, scene.fog).unwrap();
        renderer.refresh_prepared(&mut batch, &borrowed, scene.camera, scene.fog).unwrap();
        let changed = scene.reframe(menu.sample(125, 0.5)).unwrap();
        for (index, draw) in scene.draws.iter().enumerate() {
            if changed.contains(&index) {
                renderer.update_dynamic_surface(&mut individual, index, &draw.borrowed(),
                    scene.camera, scene.fog,
                    ).unwrap();

            } else {
                renderer.update_camera(&individual, index,
                    draw.view_projection.unwrap_or(scene.camera), scene.fog,
                    ).unwrap();
            }
        }
        let updates = changed.iter().map(|&index|
            (index, scene.draws[index].borrowed())).collect::<Vec<_>>();
        renderer.update_dynamic_vertices_batch(&mut batch, &updates).unwrap();
        let cameras = scene.draws.iter().map(|draw|
            draw.view_projection.unwrap_or(scene.camera)).collect::<Vec<_>>();
        renderer.update_cameras(&batch, &cameras, scene.fog).unwrap();
        crate::capture_test::capture_standalone(&renderer, &individual,
            "work/frontend/vertex-individual-replaced.png",
        );
        crate::capture_test::capture_standalone(&renderer, &batch,
            "work/frontend/vertex-batch-replaced.png",
        );
        assert_eq!(fs::read("work/frontend/vertex-individual-replaced.png").unwrap(),
            fs::read("work/frontend/vertex-batch-replaced.png").unwrap());
        eprintln!("Original menu {} draws, 100 moving frames: reframe {reframe_time:?}, individual uploads {old_time:?}, batch uploads {batch_time:?}", scene.draws.len());
    }


    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original intro assets and GPU"]
    fn menu_batch_camera_upload_matches_individual_updates() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let camera = CameraSample { eye: [-53., 30., 74.], target: [-53., 5., 44.],
            up: [0., 1., 0.], vertical_fov: 45.,
        };
        let mut scene = render_scene::assemble(Request {
            build_file: &build_file, source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets { executable: &executable,
                directory: &directory, archive: &archive,
            }),
            suncols: Some(&suncols), phase: Some(12), camera: Some(camera),
            width: 1280, height: 960, animated: true,
        }).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let borrowed = scene.draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>();
        let mut individual = renderer.prepare(&borrowed, scene.camera, scene.fog).unwrap();
        let mut batch = renderer.prepare(&borrowed, scene.camera, scene.fog).unwrap();
        let mut menu = MenuCamera::new(camera);
        let mut old_time = std::time::Duration::ZERO;
        let mut batch_time = std::time::Duration::ZERO;
        let mut reframe_time = std::time::Duration::ZERO;
        for tick in 0..100 {
            let started = Instant::now();
            let changed = scene.reframe(menu.sample(tick, 0.25)).unwrap();
            reframe_time += started.elapsed();
            let started = Instant::now();
            for (index, draw) in scene.draws.iter().enumerate() {
                if changed.contains(&index) {
                    renderer.update_dynamic_surface(&mut individual, index, &draw.borrowed(),
                        scene.camera, scene.fog,
                        ).unwrap();
                } else {
                    renderer.update_camera(&individual, index,
                        draw.view_projection.unwrap_or(scene.camera), scene.fog,
                        ).unwrap();
                }
            }
            old_time += started.elapsed();
            let started = Instant::now();
            for &index in &changed {
                renderer.update_dynamic_vertices(&mut batch, index,
                    &scene.draws[index].borrowed()).unwrap();
            }
            let cameras = scene.draws.iter().map(|draw|
                draw.view_projection.unwrap_or(scene.camera)).collect::<Vec<_>>();
            renderer.update_cameras(&batch, &cameras, scene.fog).unwrap();
            batch_time += started.elapsed();
            if [0, 50, 99].contains(&tick) {
                let reference = format!("work/frontend/menu-individual-{tick}.png");
                let actual = format!("work/frontend/menu-batch-{tick}.png");
                crate::capture_test::capture_standalone(&renderer, &individual, &reference);
                crate::capture_test::capture_standalone(&renderer, &batch, &actual);
                assert_eq!(fs::read(reference).unwrap(), fs::read(actual).unwrap());
                let fresh = format!("work/frontend/menu-fresh-{tick}.png");
                crate::capture_test::capture_scene_world_overlays(&renderer, &scene, &[], &[], &fresh,
                );
                assert_eq!(fs::read(fresh).unwrap(),
                    fs::read(format!("work/frontend/menu-batch-{tick}.png")).unwrap());
            } else {
                // Flush staging uploads equally without adding offscreen rendering
                // to the CPU preparation measurements.
                renderer.queue.submit([]);
                renderer.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            }
        }
        // Individual APIs must still isolate each draw's camera and fog slots.
        let alternate = scene.fog.map(|mut fog| { fog.color ^= 0x00ff0000; fog });
        renderer.update_camera(&individual, 0, scene.camera, alternate).unwrap();
        renderer.update_camera(&batch, 0, scene.camera, alternate).unwrap();
        crate::capture_test::capture_standalone(&renderer, &individual,
            "work/frontend/menu-individual-fog.png",
        );
        crate::capture_test::capture_standalone(&renderer, &batch,
            "work/frontend/menu-batch-fog.png",
        );
        assert_eq!(fs::read("work/frontend/menu-individual-fog.png").unwrap(),
            fs::read("work/frontend/menu-batch-fog.png").unwrap());
        // A material replacement introduces a standalone uniform buffer. The
        // batch API must update that live buffer as well as the retained arena.
        let mut pixels = scene.draws[0].rgba.to_vec();
        pixels[0] ^= 255;
        scene.draws[0].rgba = pixels.into();
        let extra = scene.draws[0].vertices.clone();
        scene.draws[0].vertices.extend(extra);
        if let Some(values) = &mut scene.draws[0].specular { values.extend(values.clone()); }
        if let Some(values) = &mut scene.draws[0].diffuse_alpha { values.extend(values.clone()); }
        if let Some(secondary) = &mut scene.draws[0].secondary { secondary.uv.extend(secondary.uv.clone()); }
        renderer.update_dynamic_surface(&mut individual, 0, &scene.draws[0].borrowed(),
            scene.camera, scene.fog,
            ).unwrap();
        renderer.update_dynamic_vertices(&mut batch, 0, &scene.draws[0].borrowed()).unwrap();
        let borrowed = scene.draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>();
        renderer.refresh_prepared(&mut individual, &borrowed, scene.camera, scene.fog).unwrap();
        renderer.refresh_prepared(&mut batch, &borrowed, scene.camera, scene.fog).unwrap();
        let changed = scene.reframe(menu.sample(125, 0.5)).unwrap();
        for (index, draw) in scene.draws.iter().enumerate() {
            if changed.contains(&index) {
                renderer.update_dynamic_surface(&mut individual, index, &draw.borrowed(),
                    scene.camera, scene.fog,
                    ).unwrap();
                renderer.update_dynamic_vertices(&mut batch, index, &draw.borrowed()).unwrap();
            } else {
                renderer.update_camera(&individual, index,
                    draw.view_projection.unwrap_or(scene.camera), scene.fog,
                    ).unwrap();
            }
        }
        let cameras = scene.draws.iter().map(|draw|
            draw.view_projection.unwrap_or(scene.camera)).collect::<Vec<_>>();
        renderer.update_cameras(&batch, &cameras, scene.fog).unwrap();
        crate::capture_test::capture_standalone(&renderer, &individual,
            "work/frontend/menu-individual-replaced.png",
        );
        crate::capture_test::capture_standalone(&renderer, &batch,
            "work/frontend/menu-batch-replaced.png",
        );
        assert_eq!(fs::read("work/frontend/menu-individual-replaced.png").unwrap(),
            fs::read("work/frontend/menu-batch-replaced.png").unwrap());
        eprintln!("Original menu {} draws, 100 moving frames: reframe {reframe_time:?}, individual uploads {old_time:?}, batch uploads {batch_time:?}", scene.draws.len());
    }

    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original intro assets and GPU"]
    fn capture_mr_monopoly_in_intro_city() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
        let index = openmt2001::archive::ArchiveIndex::parse(&directory, archive.len() as u64)
            .unwrap();
        let actors = crate::auction_scene::Assets::load_result(&image, &index, &archive).unwrap();
        let actor_draws = actors.draws(&crate::auction_scene::result_layout((10, 10)).unwrap())
            .unwrap();
        let camera = CameraSample {
            eye: [-53., 30., 74.], target: [-53., 5., 44.], up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let mut scene = render_scene::assemble(Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets { executable: &executable,
                directory: &directory, archive: &archive,
            }),
            suncols: Some(&suncols), phase: Some(12), camera: Some(camera),
            width: 1280, height: 960, animated: true,
        }).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut city = renderer.prepare(&scene.draws.iter().map(|draw| draw.borrowed())
            .collect::<Vec<_>>(), scene.camera, scene.fog,
            ).unwrap();
        let actor = renderer.prepare(&actor_draws.iter().map(|draw| draw.borrowed())
            .collect::<Vec<_>>(), scene.camera, scene.fog,
            ).unwrap();
        let path = std::env::var("OPENMT2001_MR_MONOPOLY_CAPTURE")
            .unwrap_or_else(|_| "work/frontend/mr-monopoly-menu-capture.png".into());
        crate::capture_test::capture_prepared_world_overlays(
            &renderer, &city, &[&actor], &[], &path,
        );
        let parameter = std::path::PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap();
        let language = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent().unwrap().parent().unwrap().join("US/language/usa");
        let time_assets = crate::time_panel::Assets::load(&index, &archive, installer,
            &language, &executable).unwrap();
        let time = time_assets.prepare(&renderer, crate::time_panel::Display {
            year: 1930, hour: 12, speed: 1,
            name: "Player", cash: Some(8000.), fraction: 312,
            character: Some(4), cash_icon_phase: 0,
        },
            ).unwrap();
        let navigation = crate::navigation::Navigation::load(&index, &archive, &executable)
            .unwrap().prepare(&renderer).unwrap();
        crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor],
            &[&navigation[0], &time], "work/frontend/time-panel-city-corrected.png",
        );
        let language = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap())
            .parent().unwrap().parent().unwrap().join("US/language/usa");
        let options_assets = crate::game_options::Assets::load(&index, &archive, installer,
            &language).unwrap();
        let options = options_assets.prepare(&renderer,
            Some(crate::game_options::Button::Continue), None).unwrap();
        crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor],
            &[&options], "work/frontend/game-options-city.png",
        );
        let mut save_feedback = crate::game_options::View::default();
        save_feedback.open(false);
        save_feedback.set_message("Saved: V2-1790352000000-000.sav".into());
        save_feedback.update(&options_assets, &renderer, None).unwrap();
        crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor],
            &[save_feedback.prepared.as_ref().unwrap()], "work/frontend/game-options-save-feedback.png",
        );
        let mut tutorial_options=crate::game_options::View::default();
        tutorial_options.open(false);
        tutorial_options.set_save_disabled(true);
        tutorial_options.update(&options_assets, &renderer, None).unwrap();
        crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor],
            &[tutorial_options.prepared.as_ref().unwrap()],
            "work/frontend/game-options-tutorial-save-disabled.png",
        );
        let settings_assets=crate::settings_menu::Assets::load(&index,&archive,installer,
            &language).unwrap();
        let settings=settings_assets.prepare(&renderer,crate::settings_menu::Page::Main,
            None,None,crate::settings_menu::Levels::default(),
            ).unwrap();
        crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor],
            &[&settings], "work/frontend/settings-menu-city.png",
        );
        let sound=settings_assets.prepare(&renderer,crate::settings_menu::Page::Sound,
            None,None,crate::settings_menu::Levels {music:60,effects:25,
                },
            ).unwrap();
        crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor],
            &[&sound], "work/frontend/sound-settings-city.png",
        );
        for (name,page) in [("graphics",crate::settings_menu::Page::Graphics),
            ("gameplay",crate::settings_menu::Page::Gameplay),
        ] {
            let panel=settings_assets.prepare(&renderer,page,None,None,
                crate::settings_menu::Levels::default(),
                ).unwrap();
            crate::capture_test::capture_prepared_world_overlays(&renderer,&city,&[&actor],
                &[&panel],&format!("work/frontend/{name}-settings-city.png"),
            );
        }
        // Controlled flyby-to-menu frames. The original recording has splash
        // skips, so these are native-rule checkpoints rather than synchronized
        // movie timestamps.
        let track = fs::read(installer.join("Standard/Cut_scenes/intro_low.tm")).unwrap();
        let track = Flyby::parse(&track).unwrap();
        for frame in [1, 100] {
            scene.reframe(track.sample(frame).unwrap()).unwrap();
            for index in 0..actor_draws.len() {
                renderer.update_camera(&actor, index, scene.camera, scene.fog).unwrap();
            }
            crate::capture_test::capture_scene_world_overlays(&renderer, &scene, &[&actor], &[],
                &format!("work/frontend/mr-monopoly-intro-frame-{frame:04}.png"),
            );
        }
        city = renderer.prepare(&scene.draws.iter().map(|draw| draw.borrowed())
            .collect::<Vec<_>>(), scene.camera, scene.fog,
            ).unwrap();
        let final_flyby = track.sample(track.frame_count() - 1).unwrap();
        let mut menu_camera = MenuCamera::new(final_flyby);
        for tick in [0, 10, 50] {
            let camera = menu_camera.sample(tick, 0.);
            let changed = scene.reframe(camera).unwrap();
            for (index, draw) in scene.draws.iter().enumerate() {
                if changed.contains(&index) {
                    renderer.update_dynamic_surface(&mut city, index, &draw.borrowed(),
                        scene.camera, scene.fog,
                        ).unwrap();
                } else {
                    renderer.update_camera(&city, index,
                        draw.view_projection.unwrap_or(scene.camera), scene.fog,
                        ).unwrap();
                }
            }
            for index in 0..actor_draws.len() {
                renderer.update_camera(&actor, index, scene.camera, scene.fog).unwrap();
            }
            crate::capture_test::capture_prepared_world_overlays(&renderer, &city, &[&actor], &[],
                &format!("work/frontend/menu-camera-transition-{tick:02}.png"),
            );
        }
    }

    #[test]
    #[ignore = "requires original intro assets and a GPU adapter"]
    fn normal_city_star_uploads_apply_detail_before_borrowing() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let camera = CameraSample { eye: [0., 50., 0.], target: [0., 50., 1.],
            up: [0., 1., 0.], vertical_fov: NORMAL_CITY_VERTICAL_FOV };
        let mut scene = render_scene::assemble(Request {
            build_file: &build_file, source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets { executable: &executable, directory: &directory, archive: &archive }),
            suncols: Some(&suncols), phase: Some(20), camera: Some(camera),
            width: 1280, height: 960, animated: true,
        }).unwrap();
        let stars = scene.draws.iter().position(|d| d.mode == openmt2001::render::SurfaceMode::Stars).unwrap();
        let capacity = scene.draws[stars].vertices.len();
        assert!(capacity > 0);
        let changed = scene.advance_daylight(20, 0, camera).unwrap();
        assert_eq!(scene.draws[stars].borrowed().vertices.len(), capacity, "night premise");
        // The actual snapshot owner uses this seam before prepare/refresh/take.
        normal_city_scene_updates(&mut scene, camera, Some(0), changed).unwrap();
        assert!(scene.draws[stars].borrowed().vertices.is_empty());
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut prepared = renderer.prepare(&scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
            scene.camera, scene.fog).unwrap();
        // Observe the actual arguments supplied to the renderer, not just a
        // policy boolean. Masked updates must never expand/write full stars.
        let upload = |scene: &Scene, changed: &[usize], prepared: &mut PreparedScene| {
            let mut star_vertices = 0;
            for &index in changed {
                let draw = scene.draws[index].borrowed();
                if index == stars { star_vertices += draw.vertices.len(); }
                renderer.update_dynamic_vertices(prepared, index, &draw).unwrap();
            }
            star_vertices
        };
        for detail in [0, 1, 2, 1, 0, 2] {
            for event in 0..2 {
                let changed = if event == 0 {
                    scene.reframe(camera).unwrap()
                } else {
                    let mut changed = scene.advance_environment(37, camera.eye).unwrap();
                    changed.extend(scene.advance_daylight(20, 45, camera).unwrap());
                    changed
                };
                // Same ordering function used by camera and environment owners.
                let changed = normal_city_scene_updates(&mut scene, camera, Some(detail), changed).unwrap();
                assert_eq!(scene.draws[stars].vertices.len(), capacity);
                let expected = if detail == 2 { capacity } else { 0 };
                assert_eq!(scene.draws[stars].borrowed().vertices.len(), expected);
                let sent = upload(&scene, &changed, &mut prepared);
                assert_eq!(sent, if changed.contains(&stars) { expected } else { 0 },
                    "Detail{detail} event{event}: exact star payload to renderer");
                if detail != 2 { assert_eq!(sent, 0, "no masked full-star upload"); }
            }
        }
        // Same-hour replacement, then the separate post-preparation camera seam.
        let mut replacement = scene.clone();
        replacement.retain_environment(&scene).unwrap();
        let changed = replacement.advance_daylight(20, 45, camera).unwrap();
        normal_city_scene_updates(&mut replacement, camera, Some(1), changed).unwrap();
        assert!(replacement.draws[stars].borrowed().vertices.is_empty());
        let mut replacement_prepared = renderer.prepare(&replacement.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
            replacement.camera, replacement.fog).unwrap();
        let changed = replacement.reframe(camera).unwrap();
        let changed = normal_city_scene_updates(&mut replacement, camera, Some(1), changed).unwrap();
        assert_eq!(upload(&replacement, &changed, &mut replacement_prepared), 0);
        let changed = replacement.reframe(camera).unwrap();
        let changed = normal_city_scene_updates(&mut replacement, camera, Some(2), changed).unwrap();
        assert_eq!(upload(&replacement, &changed, &mut replacement_prepared), capacity,
            "High grows/restores the same previously masked prepared surface");
        // Explicitly unowned/result callers retain generic night admission.
        replacement.apply_normal_star_detail(0, camera).unwrap();
        let changed = replacement.reframe(camera).unwrap();
        let changed = normal_city_scene_updates(&mut replacement, camera, None, changed).unwrap();
        assert_eq!(upload(&replacement, &changed, &mut replacement_prepared), capacity);
    }

    #[test]
    #[ignore = "requires original intro assets and a GPU adapter"]
    fn right_drag_mode12_reframes_retained_gpu_city() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let executable = read("OPENMT2001_EXE");
        let directory = read("OPENMT2001_ARCHIVE_DIR");
        let archive = read("OPENMT2001_ARCHIVE_BIN");
        let build_file = read("OPENMT2001_BUILD_FILE");
        let suncols = read("OPENMT2001_SUNCOLS");
        let map = read("OPENMT2001_INTRO_MAP");
        let mut controls = openmt2001::camera::BlockCamera {
            center: [0.; 3], angle: -std::f32::consts::FRAC_PI_2,
            height: 180., radius: 220.,
        };
        let (eye, target) = mode12_input_tick(&mut controls, false, [false; 4], [0; 2],50,false).unwrap();
        let mut camera = CameraSample { eye, target, up: [0., 1., 0.], vertical_fov: 45.,
        };
        let mut scene = render_scene::assemble(Request {
            build_file: &build_file,
            source: Source::Map(std::str::from_utf8(&map).unwrap()),
            original: Some(OriginalAssets { executable: &executable,
                directory: &directory, archive: &archive,
            }),
            suncols: Some(&suncols), phase: Some(12), camera: Some(camera),
            width: 1280, height: 960, animated: true,
        }).unwrap();
        let layout = scene.draws.iter().map(|draw| draw.vertices.len()).collect::<Vec<_>>();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        let mut prepared = renderer.prepare(&scene.draws.iter().map(|d| d.borrowed())
            .collect::<Vec<_>>(), scene.camera, scene.fog,
            ).unwrap();
        let mut batch = renderer.prepare(&scene.draws.iter().map(|d| d.borrowed())
            .collect::<Vec<_>>(), scene.camera, scene.fog,
            ).unwrap();
        let original_matrix = scene.camera;
        let first = mode12_input_tick(&mut controls, false, [false; 4], [8, -12],50,false).unwrap();
        assert_eq!(first.0[0], eye[0], "horizontal orbit applies next callback");
        assert_ne!(first.0[1], eye[1], "zoom applies in this callback");
        let second = mode12_input_tick(&mut controls, false, [false; 4], [0; 2],50,false).unwrap();
        assert_ne!(second.0[0], first.0[0]);
        camera.eye = second.0;
        camera.target = second.1;
        let changed = scene.reframe(camera).unwrap();
        assert_ne!(scene.camera, original_matrix);
        for (index, draw) in scene.draws.iter().enumerate() {
            if changed.contains(&index) {
                renderer.update_dynamic_surface(&mut prepared, index, &draw.borrowed(),
                    draw.view_projection.unwrap_or(scene.camera), scene.fog,
                    ).unwrap();
            } else {
                renderer.update_camera(&prepared, index,
                    draw.view_projection.unwrap_or(scene.camera), scene.fog,
                    ).unwrap();
            }
        }
        assert_eq!(scene.draws.iter().map(|draw| draw.vertices.len()).collect::<Vec<_>>(), layout);
        for phase in 0..2 {
            let changed = if phase == 0 { changed.clone() } else {
                let mut changed = scene.advance_environment(37, camera.eye).unwrap();
                changed.extend(scene.advance_daylight(13, 45, camera).unwrap());
                changed.sort_unstable();
                changed.dedup();
                for &index in &changed {
                    renderer.update_dynamic_surface(&mut prepared, index,
                        &scene.draws[index].borrowed(), scene.camera, scene.fog,
                        ).unwrap();
                }
                for (index, draw) in scene.draws.iter().enumerate() {
                    renderer.update_camera(&prepared, index,
                        draw.view_projection.unwrap_or(scene.camera), scene.fog,
                        ).unwrap();
                }
                changed
            };
            for index in changed {
                renderer.update_dynamic_vertices(&mut batch, index,
                    &scene.draws[index].borrowed()).unwrap();
            }
            let cameras = scene.draws.iter().map(|draw|
                draw.view_projection.unwrap_or(scene.camera)).collect::<Vec<_>>();
            renderer.update_cameras(&batch, &cameras, scene.fog).unwrap();
            #[cfg(feature = "capture")]
            {
                let reference = format!("work/frontend/city-individual-phase-{phase}.png");
                let actual = format!("work/frontend/city-batch-phase-{phase}.png");
                let fresh = format!("work/frontend/city-fresh-phase-{phase}.png");
                crate::capture_test::capture_standalone(&renderer, &prepared, &reference);
                crate::capture_test::capture_standalone(&renderer, &batch, &actual);
                crate::capture_test::capture_scene_world_overlays(&renderer, &scene, &[], &[], &fresh,
                );
                assert_eq!(fs::read(reference).unwrap(), fs::read(&actual).unwrap());
                assert_eq!(fs::read(fresh).unwrap(), fs::read(actual).unwrap());
            }
        }
    }
    #[test]
    #[ignore = "requires supplied v1.4 assets"]
    fn sandbox_and_bonus_launch_use_retained_hubs() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
            let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
            let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let executable = read("OPENMT2001_EXE");
            let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
                character_names: crate::fresh_city::character_names(
                    &image, &exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                ).unwrap(),
                installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                maps: map.parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"), executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"), archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            };
            let catalog = crate::campaign_store::load_catalog(
                &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                inputs.maps.parent().unwrap(),
            ).unwrap();
            let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
                up: [0., 1., 0.], vertical_fov: 45.,
                };
            use crate::singleplayer::{Difficulty, Preview};
            for (preview, id, difficulty, expected_cash, expected_people) in [
                (Preview::Sandbox, 400002, Difficulty::Easy, Some(20000.), 500,
                    ),
                (Preview::Scenario(400001), 400001, Difficulty::Easy, Some(40000000.), 500,
                    ),
                (Preview::Scenario(400001), 400001, Difficulty::Medium, Some(30000000.), 500,
                    ),
                (Preview::Scenario(400001), 400001, Difficulty::Hard, Some(20000000.), 500,
                    ),
                (Preview::Scenario(200016), 200016, Difficulty::Easy, Some(8000.), 410,
                    ),
                (Preview::Scenario(100001), 100001, Difficulty::Easy, None, 0),
            ] {
                let setup = crate::singleplayer::Setup {
                    preview,
                    difficulty,
                    character_index: 4,
                    profile_name: b"Launch test".to_vec(),
                    catalog: catalog.clone(),
                    profile_path: PathBuf::from("work/frontend/launch-test-profile.lua"),
                };
                let mut cache = SnapshotCache::default();
                let (loaded, session) = inputs.fresh_preview(setup, camera, &mut cache).unwrap();
                assert_eq!(session.catalog.selected_level().unwrap().id(), id);
                assert_eq!(session.world.ledgers.residents.active_count, expected_people);
                if let Some(cash) = expected_cash {
                    assert_eq!(session.world.player_money, cash);
                    assert_eq!(session.world.ledgers.players[0].cash, cash);
                }
                if matches!(id, 400001 | 400002) {
                    assert_eq!(session.roster.manager.players.len(), 1);
                }
                if id == 400001 {
                    let row = session.world.catalog.info.row(42).unwrap();
                    for (at, expected) in [(0x38,1), (0x3c,30), (0x48,1), (0x44,20), (0x40,6)] {
                        assert_eq!(u32::from_le_bytes(row[at..at+4].try_into().unwrap()), expected);
                    }
                }
                println!("Launch contract id{id} cash{} people{} roster{}", session.world.ledgers.players[0].cash,
                    session.world.ledgers.residents.active_count, session.roster.manager.players.len());
                if id != 100001 {
                    assert!(session.world.ledgers.residents.active_count > 0);
                }
                assert!(loaded.player_info.is_some());
                assert!(!loaded.scene.draws.is_empty());
                if id == 100001 {
                    let plain = session.scene(&inputs.build_file,
                        OriginalAssets { executable: &inputs.executable,
                            directory: &inputs.directory, archive: &inputs.archive,
                                },
                        &inputs.suncols, Some(camera), true,
                            ).unwrap();
                    assert!(cache.city.as_ref().unwrap().draws.len() > plain.draws.len(),
                        "original Tutorial2 map signs and banners must reach the live city");
                }
            }
        }).unwrap().join().unwrap();
    }
    #[test]
    #[ignore = "requires supplied original saves, scripts and assets"]
    fn building_tree_saved_snapshot_retention() {
        std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
            let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
            let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
            let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let executable = read("OPENMT2001_EXE");
            let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
            let inputs = SceneInputs {
                process_runtime:None,
                compatibility: crate::compatibility::Mode::Modern,
                style: test_board_style(&executable), time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("tree-snapshot-profiles"),
                character_names: crate::fresh_city::character_names(
                    &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                    &exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                ).unwrap(),
                installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
                maps: map.parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"), executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"), archive: read("OPENMT2001_ARCHIVE_BIN"),
                suncols: read("OPENMT2001_SUNCOLS"),
            };
            let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
                up: [0., 1., 0.], vertical_fov: 45. };
            for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
                let mut cache = SnapshotCache::default();
                let (_, mut session) = inputs.saved_preview(
                    &PathBuf::from(std::env::var(key).unwrap()), camera, &mut cache).unwrap();
                let state = session.models.tree_state.as_ref().unwrap().clone();
                let revision = state.borrow().revision();
                let mut seen = std::collections::BTreeSet::new();
                for (&slot, _) in &session.models.bindings {
                    use openmt2001::trees::building::{OWNER_COUNT_OFFSET, OWNER_INDICES_OFFSET};
                    let record = &session.world.buildings[slot];
                    let word = |at| u32::from_le_bytes(record[at..at+4].try_into().unwrap()) as usize;
                    let pool = state.borrow();
                    let indices = pool.indices(slot).expect("bound model must own its tree list, including empty lists");
                    assert_eq!(word(OWNER_COUNT_OFFSET), indices.len());
                    for (n, &index) in indices.iter().enumerate() {
                        assert_eq!(word(OWNER_INDICES_OFFSET + n*4), index);
                        assert!(index >= 137 && seen.insert(index));
                        assert!(pool.pool().slot(index).is_some());
                    }
                }
                assert_eq!(state.borrow().pool().count(), 137 + seen.len());
                let retained = cache.tree_instances.as_ref().unwrap().1.clone();
                let _ = inputs.inspect_fresh(&mut session, camera, &mut cache).unwrap();
                assert!(std::sync::Arc::ptr_eq(&retained, &cache.tree_instances.as_ref().unwrap().1));
                assert_eq!(state.borrow().revision(), revision, "snapshot cannot allocate tree IDs");
                assert!(std::rc::Rc::ptr_eq(&state, session.models.tree_state.as_ref().unwrap()));
                eprintln!("{key}: {} building trees, stable pool/revision across repeated scene snapshots", seen.len());
                if key == "OPENMT2001_SAVE_TWO" {
                    let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
                    let board = inputs.styled_board(&image).unwrap();
                    let file = openmt2001::build_file::BuildFile::parse(&inputs.build_file).unwrap();
                    let assets = crate::fresh_city::CommandAssets { image: &image, board: &board, file: &file };
                    let mut selected = None;
                    'search: for category in 0..52 {
                        if !matches!(crate::build_panel::Context::from_session(&session,category).unwrap().entry(),
                            crate::build_panel::Entry::Open) { continue; }
                        for x in 0..=6 { for y in 0..=5 { for variant in 0..32 {
                            let request = crate::build_panel::Request { category: category as i32,
                                origin: [x,y], size: [1,2], height: 1, packed_model: variant << 16,
                                quality: 0, kind: 2, displayed_quote: 0., business: 44 };
                            if let Ok(quote) = session.inspect_build(&request,&assets)
                                && quote.allowed()
                                && let Some(preview) = &quote.preview
                                && !openmt2001::trees::building::locals(&file,preview.selected,1).unwrap().is_empty() {
                                eprintln!("Tree-bearing legal preview model{} category{category} lot{x},{y} variant{variant}",preview.selected.index);
                                selected = Some(request); break 'search;
                            }
                        }}}
                    }
                    let request = selected.expect("supplied skirmish admits a legally quoted 1x2 landscape");
                    let slot = openmt2001::building_pool::first_free(&session.world.buildings).unwrap().unwrap();
                    assert!(session.submit_build(&request,&assets,|_|Ok(())).unwrap().submitted);
                    let mut host = crate::live_host::Host::new(camera.eye);
                    host.running = true;
                    for _ in 0..350 {
                        host.tick(&mut session,&assets);
                        assert!(host.failure.is_none(),"{:?}",host.failure);
                        for prompt in host.take_prompts(&session) {
                            session.dispatch_session_event("InterfacePromptDismiss",prompt.dismissal_id as i32,&assets,true).unwrap();
                        }
                        if state.borrow().indices(slot).is_some_and(|ids| !ids.is_empty()) { break; }
                    }
                    let ids = state.borrow().indices(slot).expect("real queued Build must bind tree ownership").to_vec();
                    assert!(!ids.is_empty());
                    let record = &session.world.buildings[slot];
                    assert_eq!(&record[4..8],&[request.origin[0] as u8,request.origin[1] as u8,
                        (request.origin[0]+request.size[0]-1) as u8,(request.origin[1]+request.size[1]-1) as u8]);
                    assert_eq!(record[8],request.height as u8);
                    assert_eq!(u32::from_le_bytes(record[0x24..0x28].try_into().unwrap()),request.kind);
                    let poses: Vec<_> = ids.iter().map(|&i| *state.borrow().pool().slot(i).unwrap()).collect();
                    let _ = inputs.inspect_fresh(&mut session,camera,&mut cache).unwrap();
                    assert!(!std::sync::Arc::ptr_eq(&retained,&cache.tree_instances.as_ref().unwrap().1));
                    let bytes = session.save_v2(&image,session.saved_cameras.unwrap(),1).unwrap();
                    let path = std::env::temp_dir().join(format!("building-tree-roundtrip-{}-{}.sav",
                        std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
                    struct SavedFixture(PathBuf);
                    impl Drop for SavedFixture { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }
                    let mut output = fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
                    let fixture = SavedFixture(path.clone());
                    std::io::Write::write_all(&mut output,&bytes).unwrap();
                    drop(output);
                    let (_, mut resumed) = inputs.saved_preview(&path,camera,&mut SnapshotCache::default()).unwrap();
                    let resumed_state = resumed.models.tree_state.as_ref().unwrap().clone();
                    let resumed_ids = resumed_state.borrow().indices(slot).unwrap().to_vec();
                    let restored: Vec<_> = resumed_ids.iter().map(|&i| *resumed_state.borrow().pool().slot(i).unwrap()).collect();
                    assert_eq!(restored,poses,"cold native/model rebind must retain tree poses");
                    assert_eq!(resumed_state.borrow().pool().count(),137+seen.len()+poses.len());
                    let mut resumed_host = crate::live_host::Host::new(camera.eye);
                    resumed_host.running = true;
                    let before = session.world.clock.timing[0];
                    assert_eq!(resumed.world.clock.timing[0],before);
                    for _ in 0..30 {
                        assert!(host.running && resumed_host.running);
                        assert!(!session.world.pause.has_reason() && !resumed.world.pause.has_reason());
                        host.tick(&mut session,&assets); resumed_host.tick(&mut resumed,&assets);
                        assert!(host.failure.is_none() && resumed_host.failure.is_none());
                        assert_eq!(session.world.clock,resumed.world.clock);
                    }
                    assert_eq!(session.world.clock.timing[0],before+30,"continuation must advance30 real logic ticks");
                    assert_eq!(state.borrow().indices(slot).unwrap(),ids);
                    assert_eq!(resumed_state.borrow().indices(slot).unwrap(),resumed_ids);
                    eprintln!("Legal landscape on category{} slot{slot}: {} trees through queued construction, scene invalidation, export/cold reload and30 continuation ticks",request.category,poses.len());
                    drop(fixture);
                    assert!(!path.exists(),"owned roundtrip fixture removed");
                }
            }
        }).unwrap().join().unwrap();
    }
    #[test]
    #[ignore = "requires both supplied original saves and v1.4 assets"]
    fn load_menu_uses_retained_original_session() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let executable = read("OPENMT2001_EXE");
        let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
            character_names: crate::fresh_city::character_names(
                &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                &exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
            ).unwrap(),
            installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
            language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
            maps: map.parent().unwrap().to_path_buf(),
            build_file: read("OPENMT2001_BUILD_FILE"), executable,
            directory: read("OPENMT2001_ARCHIVE_DIR"), archive: read("OPENMT2001_ARCHIVE_BIN"),
            suncols: read("OPENMT2001_SUNCOLS"),
        };
        let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
            up: [0., 1., 0.], vertical_fov: 45.,
                };
        for key in ["OPENMT2001_SAVE_ONE", "OPENMT2001_SAVE_TWO"] {
            let mut cache = SnapshotCache::default();
            let (loaded, mut session) = inputs.saved_preview(
                std::path::Path::new(&std::env::var(key).unwrap()), camera,
                &mut cache,
                        ).unwrap();
            if key == "OPENMT2001_SAVE_ONE" {
                assert!(loaded.scene.diagnostics.iter().any(|line|
                    line.contains("45 native-order detail replacements")));
                let baseline = session.scene(&inputs.build_file,
                    OriginalAssets { executable: &inputs.executable,
                        directory: &inputs.directory, archive: &inputs.archive,
                                },
                    &inputs.suncols, Some(loaded.saved_viewpoint.unwrap().0), true,
                            ).unwrap();
                let mut current_models = baseline.clone();
                let mut current_cache = SnapshotCache::default();
                let mut current_visibility = DetailVisibility::default();
                inputs.append_current_appearance(
                    &session, &mut current_models, &mut current_visibility,
                    &mut current_cache, false,
                ).unwrap();
                assert_eq!(current_models.draws.len(), cache.city.as_ref().unwrap().draws.len(),
                    "live model appearance must submit the same original details");
                let expected_draws = session.models.appearance_candidates.values()
                    .flat_map(|model| &model.renderer_templates)
                    .filter(|row| u32::from_le_bytes(row.row[..4].try_into().unwrap()) == 1)
                    .map(|row| if row.descriptor == 0x0108_0202 { 3 } else { 2 })
                    .sum::<usize>();
                assert_eq!(expected_draws, 105, "supplied active-row dynamic detail contract");
                // Static inactive rows are batched into shared atlas pages, not
                // counted by the active-row dynamic renderer-template formula.
                let static_categories = baseline.inspection_buildings.iter().filter_map(|building| {
                    let slot = building.runtime_slot?;
                    let record = &session.world.buildings[slot];
                    (record[0x3f] != 0 && detail_model_at_unit_scale(record)
                        && session.models.appearance_candidates.get(&slot)?.renderer_templates.iter()
                            .any(|row| crate::static_detail::supports(row.descriptor)
                                && u32::from_le_bytes(row.row[..4].try_into().unwrap()) == 0))
                        .then_some(building.category)
                }).collect::<std::collections::BTreeSet<_>>();
                assert!(!static_categories.is_empty(), "supplied save must exercise static details");
                let check_details = |scene: &Scene, detail_cache: &SnapshotCache,
                                     visibility: &DetailVisibility, base_draws: usize| {
                    let mut end = base_draws;
                    let mut static_draws = 0;
                    let mut categories = std::collections::BTreeSet::new();
                    for range in &detail_cache.static_detail_ranges {
                        assert!(range.start >= end && range.end <= scene.draws.len(),
                            "static ranges must be ordered, disjoint, and appended to the base");
                        end = range.end;
                        static_draws += range.len();
                        for index in range.clone() {
                            let draw = &scene.draws[index];
                            assert!(!draw.vertices.is_empty(), "static page must contain geometry");
                            let mut spans = visibility.spans.iter().filter(|span| span.draw == index)
                                .collect::<Vec<_>>();
                            spans.sort_by_key(|span| span.start);
                            let mut vertex_end = 0;
                            for span in spans {
                                assert_eq!(span.start, vertex_end, "static spans must cover each page once");
                                vertex_end += span.positions.len();
                                assert!(static_categories.contains(&span.category),
                                    "static category must own an eligible supported inactive row");
                                categories.insert(span.category);
                            }
                            assert_eq!(vertex_end, draw.vertices.len(), "static page has untracked vertices");
                        }
                    }
                    assert!(static_draws > 0, "supplied static rows must produce atlas pages");
                    assert_eq!(categories, static_categories, "static category coverage");
                    assert_eq!(scene.draws.len() - base_draws - static_draws, expected_draws,
                        "saved dynamic details must retain the active-row count");
                    static_draws
                };
                let retained = cache.city.as_ref().unwrap();
                let static_draws = check_details(retained, &cache, &cache.detail_visibility,
                    baseline.draws.len());
                assert_eq!(check_details(&current_models, &current_cache, &current_visibility,
                    baseline.draws.len()), static_draws, "saved/current static page count");
                // Same immutable Session, pose, time and append helpers; only
                // the scene projection input changes. No host tick or replay.
                let control_camera = CameraSample { vertical_fov: 45.,
                    ..loaded.saved_viewpoint.unwrap().0 };
                let mut control = session.scene(&inputs.build_file,
                    OriginalAssets { executable: &inputs.executable,
                        directory: &inputs.directory, archive: &inputs.archive },
                    &inputs.suncols, Some(control_camera), true).unwrap();
                let control_base = control.draws.len();
                assert_eq!(control_base, baseline.draws.len(), "FOV must not change base draw admission");
                let mut control_cache = SnapshotCache::default();
                let mut control_visibility = DetailVisibility::default();
                inputs.append_saved_appearance(&session, &mut control, &mut control_visibility,
                    &mut control_cache).unwrap();
                inputs.append_current_appearance(&session, &mut control, &mut control_visibility,
                    &mut control_cache, true).unwrap();
                assert_eq!(check_details(&control, &control_cache, &control_visibility, control_base),
                    static_draws, "45/53 static page count");
                assert_eq!(control_cache.static_detail_ranges, cache.static_detail_ranges,
                    "45/53 static page placement");
                for (control_draw, saved_draw) in control.draws[control_base..].iter()
                    .zip(&retained.draws[baseline.draws.len()..]) {
                    assert_eq!(control_draw.vertices, saved_draw.vertices, "45/53 detail geometry");
                    assert_eq!(control_draw.mode, saved_draw.mode, "45/53 detail material mode");
                    assert_eq!(control_draw.specular, saved_draw.specular, "45/53 detail lighting");
                    assert_eq!((control_draw.width, control_draw.height),
                        (saved_draw.width, saved_draw.height), "45/53 detail atlas dimensions");
                    assert_eq!(control_draw.rgba, saved_draw.rgba, "45/53 detail atlas pixels");
                }
                eprintln!("SAVE_ONE appearance: {expected_draws} dynamic draws + {static_draws} static pages; \
                    static categories {static_categories:?}; identical45/53 detail submission");
                // Controlled ownership-only mutation: the same-hour snapshot must
                // rebuild appearance, then restore the original material submission.
                let original_record = session.world.buildings[12];
                assert_eq!(original_record[0x2d], 1);
                let original_materials = cache.city.as_ref().unwrap().draws.iter()
                    .map(|draw| (draw.width, draw.height, draw.rgba.clone())).collect::<Vec<_>>();
                let mut absent_business = original_record;
                absent_business[0x1c..0x20].copy_from_slice(&u32::MAX.to_le_bytes());
                let payload = |record: &[u8; openmt2001::building_pool::RECORD_SIZE]| {
                    let (class, labels) = SceneInputs::appearance_payload(&session, record,
                        cache.appearance_assets.as_ref().unwrap()).unwrap();
                    (class, crate::save_appearance::banner_material_indices(
                        &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                        labels.banner.unwrap()).unwrap())
                };
                assert_eq!(payload(&original_record), (3, (48, 3, Some(5))));
                assert_eq!(payload(&absent_business), (3, (48, 3, None)));
                session.world.buildings[12][0x2d] = 0;
                let city = inputs.inspect_fresh(&mut session, loaded.saved_viewpoint.unwrap().0,
                    &mut cache).unwrap();
                assert!(!city.reuse_city, "owner change must rebuild cached appearance");
                let (class, labels) = SceneInputs::appearance_payload(&session,
                    &session.world.buildings[12], cache.appearance_assets.as_ref().unwrap()).unwrap();
                assert_eq!(class, 10);
                assert_eq!(crate::save_appearance::banner_material_indices(
                    &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                    labels.banner.unwrap()).unwrap(), (55, 286, Some(5)));
                assert_ne!(cache.city.as_ref().unwrap().draws.iter()
                    .map(|draw| (draw.width, draw.height, draw.rgba.clone())).collect::<Vec<_>>(),
                    original_materials, "owner change must reach actual scene materials");
                session.world.buildings[12] = original_record;
                let restored = inputs.inspect_fresh(&mut session, loaded.saved_viewpoint.unwrap().0,
                    &mut cache).unwrap();
                assert!(!restored.reuse_city);
                assert_eq!(cache.city.as_ref().unwrap().draws.iter()
                    .map(|draw| (draw.width, draw.height, draw.rgba.clone())).collect::<Vec<_>>(),
                    original_materials, "restored owner must restore original scene materials");
                let mut unchanged = cache.city.as_ref().unwrap().clone();
                inputs.append_current_appearance(
                    &session, &mut unchanged, &mut DetailVisibility::default(),
                    &mut SnapshotCache::default(), true,
                ).unwrap();
                assert_eq!(unchanged.draws.len(), cache.city.as_ref().unwrap().draws.len(),
                    "load-time identities must not submit duplicate live details");
                let (&slot, original_identity) = session.models.appearance_candidates
                    .iter().next().unwrap();
                let original_identity = original_identity.clone();
                session.models.appearance_candidates.remove(&slot);
                let mut rebound = baseline.clone();
                let mut rebound_cache = SnapshotCache::default();
                inputs.append_saved_appearance(&session, &mut rebound,
                    &mut DetailVisibility::default(), &mut rebound_cache,
                            ).unwrap();
                inputs.append_current_appearance(
                    &session, &mut rebound, &mut DetailVisibility::default(),
                    &mut rebound_cache, true,
                ).unwrap();
                assert_eq!(rebound.draws.len(), cache.city.as_ref().unwrap().draws.len(),
                    "a model without its saved identity must acquire current details");
                session.models.appearance_candidates.insert(slot, original_identity);
            }
            let saved = session.saved_cameras.as_ref().unwrap();
            let (visible, controls) = loaded.saved_viewpoint.unwrap();
            assert_eq!(visible.vertical_fov, 53., "ordinary saved owner retained menu FOV");
            let at = |offset: usize| {
                        f32::from_le_bytes(saved[0][offset..offset + 4].try_into().unwrap())
                    };
            assert_eq!(visible.eye, [at(0x4c), at(0x50), at(0x54)]);
            assert_eq!(visible.target, [at(0x58), at(0x5c), at(0x60)]);
            assert_eq!(visible.up, [at(0x64), at(0x68), at(0x6c)]);
            assert_ne!(visible.eye, camera.eye, "save still uses menu camera");
            assert!(controls.radius > 0.);
            let mut pan = controls;
            let (first_eye, first_target) = saved_camera_tick(&mut pan, [true, false, false, false]);
            for (actual, saved) in first_eye.into_iter().zip(visible.eye) {
                assert!((actual - saved).abs() < 0.01, "saved camera jumps at first pan callback");
            }
            for (actual, saved) in first_target.into_iter().zip(visible.target) {
                assert!((actual - saved).abs() < 0.01, "saved target jumps at first pan callback");
            }
            let (next_eye, next_target) = saved_camera_tick(&mut pan, [false; 4]);
            assert!((next_eye[0] - first_eye[0]).hypot(next_eye[2] - first_eye[2]) <= 2.01);
            assert!((next_target[0] - first_target[0]).hypot(next_target[2] - first_target[2]) <= 2.01);
            let page_up_height = page_height(controls.height, true);
            assert!((page_up_height - visible.eye[1]).abs() <= 10.);
            assert!(loaded.fresh_session.is_none());
            assert!(session.saved_game_mode.is_some(), "{key} did not retain saved mode");
            assert!(!loaded.scene.draws.is_empty(), "{key} did not assemble a live city");
            assert!(session.campaign_profile.is_none(), "arbitrary save attached a V2 profile");
            if key == "OPENMT2001_SAVE_ONE" {
                // A renderer becomes available only after the load result is
                // delivered. Exercise the deferred mode-2 Prepare/Activate
                // handoff without requiring a GPU in this loader test.
                let category = session.world.city.categories.iter()
                    .position(|row| row[5] == 0).unwrap();
                session.saved_game_mode = Some(2);
                let controller = &mut session.world.city.auction.controller;
                controller.reference = 0x7f4758;
                controller.record[4..8].copy_from_slice(&(category as u32).to_le_bytes());
                controller.record[0x7d] = 1;
                let (sender, receiver) = mpsc::channel::<crate::auction_bridge::Request>();
                let responder = std::thread::spawn(move || {
                    let first = receiver.recv().unwrap();
                    assert!(matches!(first.operation, crate::auction_bridge::Operation::Prepare { .. }));
                    first.reply.send(Ok(0)).unwrap();
                    let second = receiver.recv().unwrap();
                    assert!(matches!(second.operation, crate::auction_bridge::Operation::Activate));
                    second.reply.send(Ok(0)).unwrap();
                });
                let mut host = crate::live_host::Host::new(camera.eye);
                host.auction_renderer = Some(sender);
                host.activate_saved_auction(&mut session).unwrap();
                responder.join().unwrap();
                assert_eq!(session.world.city.auction.controller.record[0x7c..0x7e], [1, 0]);
            }
        }
        }).unwrap().join().unwrap();
    }
    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original supplied save, original assets and GPU"]
    fn capture_saved_appearance_against_plain_city() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
        let language = installer.join("US/language/usa");
        let executable = read("OPENMT2001_EXE");
        let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
            character_names: crate::fresh_city::character_names(
                &openmt2001::pe::PeImage::parse(&executable).unwrap(), &language,
                    ).unwrap(),
            installer, language,
            maps: PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap())
                .parent().unwrap().to_path_buf(),
            build_file: read("OPENMT2001_BUILD_FILE"), executable,
            directory: read("OPENMT2001_ARCHIVE_DIR"),
            archive: read("OPENMT2001_ARCHIVE_BIN"),
            suncols: read("OPENMT2001_SUNCOLS"),
        };
        let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
            up: [0., 1., 0.], vertical_fov: 45.,
                };
        let mut cache = SnapshotCache::default();
        let (loaded, session) = inputs.saved_preview(
            std::path::Path::new(&std::env::var("OPENMT2001_SAVE_ONE").unwrap()),
            camera, &mut cache,
                    ).unwrap();
        let plain = session.scene(&inputs.build_file,
            OriginalAssets { executable: &inputs.executable,
                directory: &inputs.directory, archive: &inputs.archive,
                        },
            &inputs.suncols, Some(loaded.saved_viewpoint.unwrap().0), true,
                    ).unwrap();
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
        let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
        crate::capture_test::capture(&renderer, None, &plain,
            "work/save/plain-saved-city.png",
                );
        crate::capture_test::capture(&renderer, None, cache.city.as_ref().unwrap(),
            "work/save/appearance-saved-city.png",
                );
        }).unwrap().join().unwrap();
    }
    #[cfg(feature = "capture")]
    #[test]
    #[ignore = "requires original Scenario1 assets and an offscreen GPU"]
    fn capture_static_details_against_fresh_city() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
            let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
            let installer = parameter.parent().unwrap().parent().unwrap().to_path_buf();
            let language = installer.join("US/language/usa");
            let executable = read("OPENMT2001_EXE");
            let image = openmt2001::pe::PeImage::parse(&executable).unwrap();
            let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
                character_names: crate::fresh_city::character_names(&image, &language).unwrap(),
                installer, language,
                maps: PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap())
                    .parent().unwrap().to_path_buf(),
                build_file: read("OPENMT2001_BUILD_FILE"), executable,
                directory: read("OPENMT2001_ARCHIVE_DIR"),
                archive: read("OPENMT2001_ARCHIVE_BIN"), suncols: read("OPENMT2001_SUNCOLS"),
            };
            let catalog = crate::campaign_store::load_catalog(
                &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
                inputs.maps.parent().unwrap(),
            ).unwrap();
            let setup = crate::singleplayer::Setup {
                preview: crate::singleplayer::Preview::Scenario(200001),
                difficulty: crate::singleplayer::Difficulty::Easy,
                character_index: 4, profile_name: b"Static detail capture".to_vec(), catalog,
                profile_path: PathBuf::from("work/frontend/static-capture-profile.lua"),
            };
            let camera = CameraSample { eye: [0., 180., 220.], target: [0.; 3],
                up: [0., 1., 0.], vertical_fov: 45. };
            let mut cache = SnapshotCache::default();
            let (_loaded, _session) = inputs.fresh_preview(setup, camera, &mut cache).unwrap();
            let mut after = cache.city.as_ref().unwrap().clone();
            let first = cache.static_detail_ranges.first().unwrap();
            let vertex = after.draws[first.clone()].iter()
                .flat_map(|draw| &draw.vertices).next().unwrap();
            let focus = [vertex[0], vertex[1], vertex[2]];
            let close = CameraSample { eye: [focus[0] + 28., focus[1] + 20., focus[2] + 30.],
                target: focus, ..camera };
            let mut before = after.clone();
            let mut removed_vertices = 0;
            for range in cache.static_detail_ranges.iter().rev() {
                removed_vertices += before.draws[range.clone()].iter()
                    .map(|draw| draw.vertices.len()).sum::<usize>();
                before.draws.drain(range.clone());
            }
            assert!(removed_vertices > 0, "original Scenario1 must submit static meshes");
            before.vertex_count -= removed_vertices;
            before.reframe(close).unwrap();
            after.reframe(close).unwrap();
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let before_path = "work/frontend/static-city-before.png";
            let after_path = "work/frontend/static-city-after.png";
            crate::capture_test::capture(&renderer, None, &before, before_path);
            crate::capture_test::capture(&renderer, None, &after, after_path);
            let pixels = |path| {
                let mut reader = png::Decoder::new(std::io::BufReader::new(
                    fs::File::open(path).unwrap())).read_info().unwrap();
                let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
                let frame = reader.next_frame(&mut rgba).unwrap();
                rgba.truncate(frame.buffer_size());
                rgba
            };
            let before_pixels = pixels(before_path);
            let after_pixels = pixels(after_path);
            let changed = before_pixels.chunks_exact(4).zip(after_pixels.chunks_exact(4))
                .filter(|(before, after)| before[..3] != after[..3]).count();
            assert!(changed > 16, "static meshes changed only {changed} GPU pixels");
            eprintln!("Static capture: {removed_vertices} submitted vertices, {changed} changed pixels; focus {focus:?}");
        }).unwrap().join().unwrap();
    }
    #[test]
    #[ignore = "requires supplied Custom originals; fresh/save/selected-profile cold ownership"]
    fn custom_fresh_save_and_cold_selected_profile_ownership() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                struct Scratch(PathBuf);
                impl Drop for Scratch {
                    fn drop(&mut self) {
                        fs::remove_dir_all(&self.0).unwrap();
                    }
                }
                let root = Scratch(std::env::temp_dir().join(format!("custom-profile-{}-{}",
                std::process::id(), std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())));
                fs::create_dir_all(&root.0).unwrap();
                let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
                let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
                let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
                let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
                let executable = read("OPENMT2001_EXE");
                let language = exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa");
                let inputs = SceneInputs {
                process_runtime:None,
                    compatibility: crate::compatibility::Mode::Modern,
                    style: test_board_style(&executable),
                    time_runtime: Default::default(),
                    profile_root: root.0.clone(),
                    character_names: crate::fresh_city::character_names(
                        &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                        &language,
                    )
                    .unwrap(),
                    installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
                    language,
                    maps: map.parent().unwrap().to_path_buf(),
                    build_file: read("OPENMT2001_BUILD_FILE"),
                    executable,
                    directory: read("OPENMT2001_ARCHIVE_DIR"),
                    archive: read("OPENMT2001_ARCHIVE_BIN"),
                    suncols: read("OPENMT2001_SUNCOLS"),
                };
                let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
                let catalog =
                    crate::campaign_store::load_catalog(&image, inputs.maps.parent().unwrap())
                        .unwrap();
                let alice = crate::campaign_store::profile_path(&root.0, b"Alice");
                let bob = crate::campaign_store::profile_path(&root.0, b"Bob");
                // Distinct persistence fixtures; these are not gameplay victories.
                for (path, attempts) in [(&alice, 2), (&bob, 7)] {
                    let mut profile = catalog.clone();
                    profile.reset_progress();
                    profile.set_level_info(500000, attempts, [0; 3]);
                    crate::campaign_store::save_progress(&profile, path).unwrap();
                }
                let before = [fs::read(&alice).unwrap(), fs::read(&bob).unwrap()];
                let mut selected = catalog.clone();
                crate::campaign_store::load_progress(&mut selected, &alice).unwrap();
                let setup = crate::singleplayer::Setup {
                    preview: crate::singleplayer::Preview::Scenario(500000),
                    difficulty: crate::singleplayer::Difficulty::Medium,
                    character_index: 4,
                    profile_name: b"Alice".to_vec(),
                    catalog: selected,
                    profile_path: alice.clone(),
                };
                let camera = CameraSample {
                    eye: [0., 180., 220.],
                    target: [0.; 3],
                    up: [0., 1., 0.],
                    vertical_fov: 0.7,
                };
                let (inspection, mut session) = inputs
                    .fresh_preview(setup, camera, &mut SnapshotCache::default())
                    .unwrap();
                assert_eq!(session.campaign_profile, Some(alice.clone()));
                assert_eq!(session.catalog.selected_level().unwrap().attempts(), 2);
                let bytes = session
                    .save_v2(&image, crate::save_menu::cameras(None, camera).unwrap(), 1)
                    .unwrap();
                let save = openmt2001::save::SaveFile::parse(&bytes).unwrap();
                let extension = openmt2001::save::extension::Extension::parse(&save)
                    .unwrap()
                    .unwrap();
                assert!(extension.version >= 4);
                let path = root.0.join("custom.sav");
                fs::write(&path, &bytes).unwrap();
                drop(session);
                drop(inspection);
                let (inspection, restored) = inputs
                    .saved_preview(&path, camera, &mut SnapshotCache::default())
                    .unwrap();
                assert_eq!(
                    restored.campaign_profile,
                    Some(alice.clone()),
                    "the explicit runtime extension must retain the fresh profile identity"
                );
                drop(restored);
                drop(inspection);
                for (name, expected, attempts) in [
                    (Some(b"Alice".as_slice()), Some(alice.clone()), 2),
                    (Some(b"Bob".as_slice()), Some(bob.clone()), 7),
                    (None, None, 0),
                ] {
                    let selected_root =
                        name.map(|name| crate::save_menu::selected_profile_root(&root.0, name));
                    let (inspection, restored) = inputs
                        .saved_preview_with_selection(
                            &path,
                            camera,
                            &mut SnapshotCache::default(),
                            Some(selected_root.as_deref()),
                        )
                        .unwrap();
                    assert_eq!(restored.campaign_profile, expected);
                    assert_eq!(restored.catalog.selected_level().unwrap().id(), 500000);
                    assert_eq!(
                        restored.catalog.selected_level().unwrap().attempts(),
                        attempts
                    );
                    assert_eq!(restored.difficulty, 1);
                    drop(restored);
                    drop(inspection);
                }
                assert_eq!(
                    [fs::read(&alice).unwrap(), fs::read(&bob).unwrap()],
                    before,
                    "startup/save/reload must not count a completed result"
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }
    #[test]
    #[ignore = "requires original fresh scenario and graphics assets"]
    fn fresh_preview_worker_retains_session_until_inspection_closes() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let executable = read("OPENMT2001_EXE");
        let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
            character_names: crate::fresh_city::character_names(
                &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                &exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa"),
            )
            .unwrap(),
            installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
            language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
            maps: map.parent().unwrap().to_path_buf(),
            build_file: read("OPENMT2001_BUILD_FILE"),
            executable,
            directory: read("OPENMT2001_ARCHIVE_DIR"),
            archive: read("OPENMT2001_ARCHIVE_BIN"),
            suncols: read("OPENMT2001_SUNCOLS"),
        };
        let inputs = Arc::new(inputs);
        let scripts = inputs.maps.parent().unwrap();
        let catalog = crate::campaign_store::load_catalog(
            &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
            scripts,
        )
        .unwrap();
        let setup = crate::singleplayer::Setup {
            preview: crate::singleplayer::Preview::Scenario(200003),
            difficulty: crate::singleplayer::Difficulty::Easy,
            character_index: 2,
            profile_name: b"Preview test".to_vec(),
            catalog,
            profile_path: crate::campaign_store::profile_path(&inputs.profile_root, b"Preview test",
            ),
        };
        let mut controls = openmt2001::camera::BlockCamera {
            center: [0.; 3],
            angle: -std::f32::consts::FRAC_PI_2,
            height: 180.,
            radius: 220.,
        };
        let (eye, target) = controls.tick([false; 4]);
        let camera = CameraSample {
            eye,
            target,
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let (tx, rx) = mpsc::channel();
        let worker_inputs = inputs.clone();
        let worker = std::thread::Builder::new()
            .name("city-simulation-test".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || worker_inputs.preview_worker(setup, camera, tx))
            .unwrap();
        let mut loaded = rx.recv().unwrap().unwrap();
        assert!(loaded.fresh_session.is_some());
        assert_eq!(loaded.music_era, Some(0));
        assert!(loaded.vehicles.is_some(), "retained city must bind road model materials");
        assert!(loaded.transport.is_some(), "retained city must bind transport materials");
        assert!(loaded.status.is_none());
        assert_eq!(loaded.scene.building_count, 9);
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_SIDEWALK_CAPTURE") {
            crate::block_sidewalk_labels::capture_marvin(
                &loaded.scene, loaded.block_information.as_ref().unwrap(), &path,
            );
        }
        let sender = loaded.fresh_session.as_ref().unwrap();
        let (reply, initial_status) = mpsc::channel();
        sender.send(FreshRequest::Status { camera: camera.eye, reply,
            }).unwrap();
        let initial_status = initial_status.recv().unwrap();
        assert!(initial_status.running, "native worker startup enables gameplay");
        assert!(initial_status.failure.is_none(), "{:?}", initial_status.failure);
        // Exercise the same asynchronous Manager requests as the window.
        // Two opens must add one modal reason, and GUI4 freezes Time while
        // the registered interface timer continues to dispatch.
        sender.send(FreshRequest::Stats(crate::live_host::StatsAction::Open)).unwrap();
        sender.send(FreshRequest::Stats(crate::live_host::StatsAction::Open)).unwrap();
        let (reply,opened)=mpsc::channel();
        sender.send(FreshRequest::Status{camera:camera.eye,reply,
            }).unwrap();
        let opened=opened.recv().unwrap();
        assert!(opened.running&&opened.script_paused);
        let snapshot=opened.stats.as_ref().unwrap();
        assert_eq!(snapshot.players[0].name,b"Preview test");
        assert_eq!(snapshot.players[0].character,2);
        std::thread::sleep(std::time::Duration::from_millis(120));
        let (reply,held)=mpsc::channel();
        sender.send(FreshRequest::Status{camera:camera.eye,reply,
            }).unwrap();
        let held=held.recv().unwrap();
        assert_eq!(held.ticks,opened.ticks,"Stats modal suspends the actual logic timer group");
        assert!(held.interface_ticks>opened.interface_ticks);
        assert_eq!(held.time_display,opened.time_display,"GUI4 suppresses native Time callbacks");
        sender.send(FreshRequest::Stats(crate::live_host::StatsAction::Close)).unwrap();
        let (reply,closed)=mpsc::channel();
        sender.send(FreshRequest::Status{camera:camera.eye,reply,
            }).unwrap();
        let closed=closed.recv().unwrap();
        assert!(closed.running&&!closed.script_paused&&closed.stats.is_none(),
            "closing a duplicate-opened graph releases exactly one reason");
        sender.send(FreshRequest::Run(false)).unwrap();
        sender.send(FreshRequest::Stats(crate::live_host::StatsAction::Open)).unwrap();
        let (reply,paused_graph)=mpsc::channel();
        sender.send(FreshRequest::Status{camera:camera.eye,reply,
            }).unwrap();
        let paused_graph=paused_graph.recv().unwrap();
        assert!(!paused_graph.running&&paused_graph.script_paused&&paused_graph.stats.is_some(),
            "original session-active flag remains set during a player's pause");
        sender.send(FreshRequest::Stats(crate::live_host::StatsAction::Close)).unwrap();
        let (reply, paused_status) = mpsc::channel();
        sender.send(FreshRequest::Status { camera: camera.eye, reply,
            }).unwrap();
        let paused_tick = paused_status.recv().unwrap().ticks;
        let (reply, saved_result) = mpsc::channel();
        let save_name = format!("worker-{:012x}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64 & 0xffff_ffff_ffff);
        sender.send(FreshRequest::SaveSelected { target: crate::save_menu::Target::NewName(save_name.clone()), capture:None,
            camera, game_mode: 1, interface_mode: 0, reply,
            }).unwrap();
        let saved_path = saved_result.recv().unwrap().unwrap();
        assert_eq!(saved_path.file_stem().unwrap().to_string_lossy(), save_name);
        let scoped_root = crate::save_menu::selected_profile_root(&inputs.profile_root, b"Preview test");
        assert!(saved_path.starts_with(fs::canonicalize(scoped_root.join("saves")).unwrap()));
        let saved_bytes = fs::read(&saved_path).unwrap();
        let saved_file = openmt2001::save::SaveFile::parse(&saved_bytes).unwrap();
        let saved_players = saved_file.players().unwrap();
        assert_eq!(saved_players.len(), 1);
        let mut expected_metadata = [0u8; 48];
        expected_metadata[..4].copy_from_slice(&2_i32.to_le_bytes());
        expected_metadata[8..20].copy_from_slice(b"Preview test");
        assert_eq!(*saved_players[0].metadata, expected_metadata,
            "fresh export retains actual frontend character, kind, name and difficulty");
        let reload_inputs = inputs.clone();
        let reload_path = saved_path.clone();
        let restored = std::thread::Builder::new().stack_size(32 * 1024 * 1024)
            .spawn(move || {
                let (inspection, session) = reload_inputs.saved_preview(
                    &reload_path, camera, &mut SnapshotCache::default()).unwrap();
                assert_eq!(session.campaign_profile, Some(crate::campaign_store::profile_path(
                    &reload_inputs.profile_root, b"Preview test")));
                assert_eq!(session.world.clock.timing[0], paused_tick);
                assert_eq!(session.saved_player_metadata[0], expected_metadata);
                inspection
            }).unwrap().join().unwrap();
        assert_eq!(restored.scene.building_count, loaded.scene.building_count);
        assert_eq!(restored.saved_viewpoint.unwrap().0.eye, camera.eye);
        let (delete_reply, delete_result) = mpsc::channel();
        sender.send(FreshRequest::DeleteSave { id: saved_path.file_name().unwrap()
            .to_string_lossy().to_string(), reply: delete_reply,
            }).unwrap();
        delete_result.recv().unwrap().unwrap();
        assert!(!saved_path.exists(), "confirmed Delete must remove only the selected V2 slot");
        for expected in [4, 1] {
            sender.send(FreshRequest::ToggleSpeed).unwrap();
            let (reply, result) = mpsc::channel();
            sender.send(FreshRequest::Status { camera: camera.eye, reply,
                }).unwrap();
            let status = result.recv().unwrap();
            assert_eq!(status.speed, expected);
            assert!(!status.running, "speed control does not unpause the city");
        }
        crate::prompt::verify_card_panel(
            &loaded.scene,
            &loaded.player_info.as_ref().unwrap().records[0],
        );
        crate::auction_notice::verify_panel(
            &loaded.scene,
            &loaded.player_info.as_ref().unwrap().records[0],
        );
        crate::business_adjustment::verify_live_panels(&loaded.scene);
        assert!(!worker.is_finished());
        let (reply, answer) = mpsc::channel();
        loaded
            .fresh_session
            .as_ref()
            .unwrap()
            .send(FreshRequest::Planning {
                category: 33,
                reply,
            })
            .unwrap();
        let planning = answer.recv().unwrap().unwrap();
        assert_eq!(planning.category, 33);
        assert_eq!(planning.current_owner, 1);
        assert_eq!(planning.era, 0);
        assert_eq!(planning.players.len(), 1);
        assert_eq!(planning.catalog.rows().len(), 45);
        assert_eq!(
            planning.category_record[5],
            loaded.block_information.as_ref().unwrap().owners[33]
        );
        assert_eq!(planning.entry(), crate::build_panel::Entry::Open);
        let initial_floor = u32::from_le_bytes(
            planning.catalog.row(0).unwrap()[0x48..0x4c]
                .try_into()
                .unwrap(),
        ) as i32;
        let preview_worker = loaded.fresh_session.as_ref().unwrap().clone();
        crate::build_view::verify_live_context(planning, &mut loaded.scene, |form| {
            let (reply, answer) = mpsc::channel();
            preview_worker
                .send(FreshRequest::Build {
                    form,
                    submit: false,
                    reply,
                })
                .unwrap();
            answer.recv().unwrap().unwrap()
        }, || {
            let (reply, answer) = mpsc::channel();
            preview_worker.send(FreshRequest::BuildDrag { reply }).unwrap();
            answer.recv().unwrap().unwrap()
        },
        );
        drop(preview_worker);
        let mut form = crate::build_panel::Request {
            category: 33,
            origin: [0, 0],
            size: [2, 2],
            height: initial_floor,
            packed_model: 0,
            kind: 0,
            quality: 0,
            displayed_quote: f32::NAN,
            business: 0,
        };
        let mut accepted = None;
        let mut construction_camera = None;
        for origin in [[0, 0], [0, 5], [5, 0], [5, 5]] {
            form.origin = origin;
            let (reply, answer) = mpsc::channel();
            loaded
                .fresh_session
                .as_ref()
                .unwrap()
                .send(FreshRequest::Build {
                    form: form.clone(),
                    submit: false,
                    reply,
                })
                .unwrap();
            let quote = answer.recv().unwrap().unwrap();
            assert!(!quote.submitted);
            assert!(quote.total.is_finite());
            if quote.allowed() {
                let center = quote.preview.as_ref().unwrap().center;
                construction_camera = Some(CameraSample {
                    eye: [center[0], 120., center[1] + 100.],
                    target: [center[0], 0., center[1]],
                    up: [0., 1., 0.],
                    vertical_fov: 45.,
                });
                accepted = Some(quote.total);
                break;
            }
        }
        let total = accepted.expect("original fresh city has a valid construction preview");
        let construction_camera = construction_camera.unwrap();
        let (reply, answer) = mpsc::channel();
        loaded
            .fresh_session
            .as_ref()
            .unwrap()
            .send(FreshRequest::Build {
                form,
                submit: true,
                reply,
            })
            .unwrap();
        let submitted = answer.recv().unwrap().unwrap();
        assert!(submitted.submitted);
        assert_eq!(
            submitted.total, total,
            "UI NaN quote is never financial input"
        );
        let (reply, answer) = mpsc::channel();
        loaded
            .fresh_session
            .as_ref()
            .unwrap()
            .send(FreshRequest::Inspect { camera, reply })
            .unwrap();
        let refreshed = answer.recv().unwrap().unwrap();
        assert!(
            refreshed.reuse_city,
            "paused snapshot reuses exact city geometry"
        );
        assert_eq!(
            refreshed.scene.building_count, loaded.scene.building_count,
            "paused worker must not secretly execute queued construction"
        );
        assert_eq!(refreshed.detail_visibility.spans.len(),
            loaded.detail_visibility.spans.len(),
            "snapshot reuse must retain registry-detail ownership");
        drop(refreshed);
        assert!(!loaded.detail_visibility.spans.is_empty(),
            "original fresh city should contain registry details");
        let first_detail = loaded.detail_visibility.spans.iter()
            .find(|span| {
                span.positions.iter().any(|position| position[1] != span.pivot_y)
            })
            .expect("original registry details have no vertical geometry").clone();
        let detail_camera = CameraSample {
            eye: [first_detail.center[0], 80., first_detail.center[1] + 300.],
            target: [first_detail.center[0], 0., first_detail.center[1]],
            up: [0., 1., 0.], vertical_fov: 45.,
        };
        let depth = openmt2001::camera::translated_scene_view(
            openmt2001::camera::scene_view_with_up(detail_camera.eye,
                detail_camera.target, detail_camera.up,
            ),
            [first_detail.center[0],0.,first_detail.center[1]],
        )[11];
        assert!(depth > 200. && depth <= 500.,
            "fixture camera should cross the native detail0/100 far bounds: {depth}");
        let base_vertices = loaded.scene.draws[0].vertices.clone();
        let started = Instant::now();
        loaded.detail_visibility.reframe(&mut loaded.scene, detail_camera, 0).unwrap();
        let hidden = loaded.scene.draws[first_detail.draw].vertices
            [first_detail.start..first_detail.start + first_detail.positions.len()]
            .iter().map(|v| [v[0],v[1],v[2]]).collect::<Vec<_>>();
        assert!(hidden.iter().all(|position| *position == hidden[0]),
            "native pass3 far gate should hide category registry details");
        let band_camera = CameraSample {
            eye: [first_detail.center[0], 80., first_detail.center[1] + 235.],
            target: [first_detail.center[0], 0., first_detail.center[1]],
            up: [0., 1., 0.], vertical_fov: 45.,
        };
        let band_depth = openmt2001::camera::translated_scene_view(
            openmt2001::camera::scene_view_with_up(band_camera.eye,
                band_camera.target, band_camera.up,
            ),
            [first_detail.center[0],0.,first_detail.center[1]],
        )[11];
        assert!(band_depth > 220. && band_depth < 270.,
            "fixture camera should enter original detail fade band: {band_depth}");
        loaded.detail_visibility.reframe(&mut loaded.scene, band_camera, 35).unwrap();
        let faded = loaded.scene.draws[first_detail.draw].vertices
            [first_detail.start..first_detail.start + first_detail.positions.len()]
            .iter().map(|v| [v[0],v[1],v[2]]).collect::<Vec<_>>();
        assert!(faded.iter().zip(&first_detail.positions).any(|(now, original)|
            now[1] != original[1] && now[1] != first_detail.pivot_y),
            "original registry detail never entered intermediate vertical scale");
        assert!(faded.iter().zip(&first_detail.positions).all(|(now, original)|
            now[0] == original[0] && now[2] == original[2]),
            "native detail band scales Y only");
        loaded.detail_visibility.reframe(&mut loaded.scene, detail_camera, 100).unwrap();
        let restored = loaded.scene.draws[first_detail.draw].vertices
            [first_detail.start..first_detail.start + first_detail.positions.len()]
            .iter().map(|v| [v[0],v[1],v[2]]).collect::<Vec<_>>();
        assert_eq!(restored, first_detail.positions,
            "raising distance restores the original detail triangles");
        assert_eq!(loaded.scene.draws[0].vertices, base_vertices,
            "the mode1 city base must remain untouched");
        eprintln!("[detail distance] spans={} three reframes={:?}",
            loaded.detail_visibility.spans.len(),started.elapsed());
        assert!(loaded.people.is_some());
        let focus = loaded.people.as_ref().unwrap().first_position();
        let near_camera = CameraSample {
            eye: [focus[0], focus[1] + 4., focus[2] - 12.],
            target: focus,
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };

        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_FRESH_HOST_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(target_os = "windows") {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let mut prepared = renderer
                .prepare(
                    &loaded
                        .scene
                        .draws
                        .iter()
                        .map(|d| d.borrowed())
                        .collect::<Vec<_>>(),
                    loaded.scene.camera,
                    loaded.scene.fog,
                )
                .unwrap();
            for (name, sample) in [
                ("near", near_camera),
                (
                    "lod",
                    CameraSample {
                        eye: [focus[0], focus[1] + 15., focus[2] + 120.],
                        ..near_camera
                    },
                ),
                (
                    "empty",
                    CameraSample {
                        eye: [focus[0], 3500., focus[2] + 10.],
                        ..near_camera
                    },
                ),
                ("return", near_camera),
            ] {
                let mut changed = loaded.scene.reframe(sample).unwrap();
                let residents = loaded
                    .people
                    .as_ref()
                    .unwrap()
                    .reframe(&mut loaded.scene, sample, 35)
                    .unwrap();
                let count: usize = loaded.scene.draws[residents.clone()]
                    .iter()
                    .map(|d| d.vertices.len())
                    .sum();
                if name == "empty" {
                    assert_eq!(count, 0);
                } else {
                    assert!(count > 0);
                }
                let dynamic = residents.clone();
                changed.extend(residents);
                for (i, draw) in loaded.scene.draws.iter().enumerate() {
                    if dynamic.contains(&i) {
                        renderer
                            .update_dynamic_surface(
                                &mut prepared,
                                i,
                                &draw.borrowed(),
                                loaded.scene.camera,
                                loaded.scene.fog,
                            )
                            .unwrap_or_else(|error| {
                                panic!("reframe {name} dynamic surface {i} {:?} vertices {}: {error}", draw.mode, draw.vertices.len())
                            });
                    } else if changed.contains(&i) {
                        renderer
                            .update_dynamic_surface(
                                &mut prepared,
                                i,
                                &draw.borrowed(),
                                loaded.scene.camera,
                                loaded.scene.fog,
                            )
                            .unwrap_or_else(|error| {
                                panic!("reframe {name} retained surface {i} {:?} vertices {}: {error}", draw.mode, draw.vertices.len())
                            });
                    } else {
                        renderer
                            .update_camera(
                                &prepared,
                                i,
                                draw.view_projection.unwrap_or(loaded.scene.camera),
                                loaded.scene.fog,
                            )
                            .unwrap();
                    }
                }
                crate::capture_test::capture_prepared(
                    &renderer,
                    &loaded.scene,
                    &prepared,
                    &format!("{path}-{name}.png"),
                );
            }
            // 452470/4524d0 map the Graphics Draw Distance percentage to
            // moving-object LOD and far visibility. Hold the camera fixed so
            // these captures isolate that original control's visible effect.
            let distance_camera = CameraSample {
                eye: [focus[0], focus[1] + 15., focus[2] + 120.],
                ..near_camera
            };
            let mut distance_counts = Vec::new();
            for detail in [0, 35, 100] {
                loaded.scene.reframe(distance_camera).unwrap();
                loaded.detail_visibility.reframe(&mut loaded.scene, distance_camera,
                    detail as u8).unwrap();
                let residents = loaded.people.as_ref().unwrap()
                    .reframe(&mut loaded.scene, distance_camera, detail).unwrap();
                distance_counts.push(loaded.scene.draws[residents].iter()
                    .map(|draw| draw.vertices.len()).sum::<usize>(),
                );
                let draws = loaded.scene.draws.iter().map(|draw| draw.borrowed())
                    .collect::<Vec<_>>();
                let distance_prepared = renderer.prepare(&draws, loaded.scene.camera,
                    loaded.scene.fog).unwrap();
                crate::capture_test::capture_prepared(&renderer, &loaded.scene,
                    &distance_prepared, &format!("{path}-distance-{detail}.png"),
                );
            }
            assert!(distance_counts[0] < distance_counts[2],
                "original draw-distance range should reveal residents at a fixed camera");
            // The same native control gates category registry details in pass3.
            // This camera holds the first original detail category in frame.
            for detail in [0, 35, 100] {
                loaded.scene.reframe(detail_camera).unwrap();
                loaded.detail_visibility.reframe(&mut loaded.scene, detail_camera,
                    detail as u8).unwrap();
                loaded.people.as_ref().unwrap()
                    .reframe(&mut loaded.scene, detail_camera, detail).unwrap();
                let draws = loaded.scene.draws.iter().map(|draw| draw.borrowed())
                    .collect::<Vec<_>>();
                let captured = renderer.prepare(&draws, loaded.scene.camera,
                    loaded.scene.fog).unwrap();
                crate::capture_test::capture_prepared(&renderer, &loaded.scene,
                    &captured, &format!("{path}-category-detail-{detail}.png"),
                );
            }
        }
        let residents = loaded
            .people
            .as_ref()
            .unwrap()
            .reframe(&mut loaded.scene, near_camera, 35)
            .unwrap();
        assert!(
            loaded.scene.draws[residents]
                .iter()
                .any(|d| !d.vertices.is_empty())
        );
        let sender = loaded.fresh_session.as_ref().unwrap();
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Status {
                camera: camera.eye,
                reply,
            })
            .unwrap();
        let paused = answer.recv().unwrap();
        assert_eq!(
            paused.ticks, paused_tick,
            "inspection and build submission do not advance a paused session"
        );
        if std::env::var_os("OPENMT2001_PROFILE_PACE").is_some() {
            for sample in 0..3 {
                let started = Instant::now();
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::Residents { reply }).unwrap();
                answer.recv().unwrap().unwrap();
                let residents = started.elapsed();
                let started = Instant::now();
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::Vehicles { reply }).unwrap();
                answer.recv().unwrap().unwrap();
                let vehicles = started.elapsed();
                let started = Instant::now();
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::Transport { reply }).unwrap();
                answer.recv().unwrap().unwrap();
                eprintln!("PACE worker request sample={sample} residents={residents:?} vehicles={vehicles:?} transport={:?}", started.elapsed());
            }
        }
        sender.send(FreshRequest::Run(true)).unwrap();
        let pace_started = Instant::now();
        let profile_pace = std::env::var_os("OPENMT2001_PROFILE_PACE").is_some();
        let mut pace_bucket = 0;
        let mut paused_total = std::time::Duration::ZERO;
        let mut status_samples = 0u32;
        let mut status_total = std::time::Duration::ZERO;
        let mut status_max = std::time::Duration::ZERO;
        let deadline = Instant::now() + std::time::Duration::from_secs(45);
        let mut foundation_seen = false;
        let mut partial_seen = false;
        let mut partial_tick = None;
        let mut patched_seen = false;
        loop {
            let status_started = Instant::now();
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::Status {
                    camera: camera.eye,
                    reply,
                })
                .unwrap();
            let status = answer.recv().unwrap();
            if profile_pace {
                let latency = status_started.elapsed();
                status_samples += 1;
                status_total += latency;
                status_max = status_max.max(latency);
            }
            if profile_pace && status.ticks / 25 > pace_bucket {
                pace_bucket = status.ticks / 25;
                eprintln!("PACE 1x logic={} interface={} active={:.3}s wall={:.3}s status_mean={:.2}ms status_max={:.2}ms",
                    status.ticks, status.interface_ticks,
                    (pace_started.elapsed() - paused_total).as_secs_f64(),
                    pace_started.elapsed().as_secs_f64(),
                    status_total.as_secs_f64() * 1000. / f64::from(status_samples),
                    status_max.as_secs_f64() * 1000.);
            }
            assert!(
                status.failure.is_none(),
                "real worker tick {} stopped: {:?}",
                status.ticks,
                status.failure
            );
            if !foundation_seen && (45..75).contains(&status.ticks) {
                let pause_started = Instant::now();
                sender.send(FreshRequest::Run(false)).unwrap();
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::Inspect {
                    camera: construction_camera, reply,
                }).unwrap();
                let foundation = answer.recv().unwrap().unwrap();
                assert!(foundation.construction_active);
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_CONSTRUCTION_LIVE_CAPTURE") {
                    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                    if cfg!(windows) {
                        descriptor.backends = wgpu::Backends::DX12;
                    }
                    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                    crate::capture_test::capture(
                        &renderer,
                        None,
                        &foundation.scene,
                        &format!("{prefix}-foundation-{}.png", status.ticks),
                    );
                }
                foundation_seen = true;
                sender.send(FreshRequest::Run(true)).unwrap();
                paused_total += pause_started.elapsed();
            }
            if !partial_seen && (125..175).contains(&status.ticks) {
                let pause_started = Instant::now();
                // Hold the original 25 Hz worker while the debug GPU capture
                // runs, so a later scripted prompt is still observed below.
                sender.send(FreshRequest::Run(false)).unwrap();
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::Inspect {
                        camera: construction_camera,
                        reply,
                    })
                    .unwrap();
                let partial = answer.recv().unwrap().unwrap();
                assert!(partial.construction_active);
                assert!(!partial.reuse_city, "growing pose invalidates the cached city");
                assert_eq!(
                    partial.scene.building_count,
                    loaded.scene.building_count + 1,
                    "live snapshot must include the actively growing building"
                );
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_CONSTRUCTION_LIVE_CAPTURE") {
                    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                    if cfg!(windows) {
                        descriptor.backends = wgpu::Backends::DX12;
                    }
                    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                    crate::capture_test::capture(
                        &renderer,
                        None,
                        &partial.scene,
                        &format!("{prefix}-partial-{}.png", status.ticks),
                    );
                }
                partial_seen = true;
                partial_tick = Some(status.ticks);
                sender.send(FreshRequest::Run(true)).unwrap();
                paused_total += pause_started.elapsed();
            }
            if partial_tick.is_some_and(|tick| status.ticks >= tick + 10)
                && !patched_seen && status.ticks < 225 {
                let pause_started = Instant::now();
                sender.send(FreshRequest::Run(false)).unwrap();
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::Inspect {
                    camera: construction_camera, reply,
                }).unwrap();
                let growing = answer.recv().unwrap().unwrap();
                assert!(growing.construction_active);
                assert!(growing.reuse_city,
                    "the second active-growth snapshot must reuse the city layout");
                assert!(!growing.updated_city_draws.is_empty(),
                    "active-growth scale must patch existing material draws");
                #[cfg(feature = "capture")]
                if let Ok(prefix) = std::env::var("OPENMT2001_CONSTRUCTION_LIVE_CAPTURE") {
                    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
                    if cfg!(windows) {
                        descriptor.backends = wgpu::Backends::DX12;
                    }
                    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
                    crate::capture_test::capture(
                        &renderer,
                        None,
                        &growing.scene,
                        &format!("{prefix}-growing-{}.png", status.ticks),
                    );
                }
                patched_seen = true;
                sender.send(FreshRequest::Run(true)).unwrap();
                paused_total += pause_started.elapsed();
            }
            if status.ticks >= 275 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "live worker did not reach construction completion: {status:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        assert!(foundation_seen, "worker skipped the scaffold foundation interval");
        assert!(partial_seen, "worker skipped the visible construction interval");
        assert!(patched_seen, "worker skipped the retained growth-pose update");
        sender.send(FreshRequest::Run(false)).unwrap();
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Inspect { camera, reply })
            .unwrap();
        let built = answer.recv().unwrap().unwrap();
        assert!(!built.construction_active);
        assert!(!built.reuse_city, "construction invalidates city geometry");
        assert_eq!(
            built.scene.building_count,
            loaded.scene.building_count + 1,
            "live command creates a real scene building"
        );
        assert!(
            built
                .scene
                .inspection_buildings
                .iter()
                .any(|b| b.initialized_business.is_some()),
            "construction opens its authoritative business after its separate delayed open command"
        );
        crate::business_adjustment::verify_live_panels(&built.scene);
        let shop = built
            .scene
            .inspection_buildings
            .iter()
            .find(|b| b.initialized_business.is_some())
            .unwrap();
        let business = shop.initialized_business.as_ref().unwrap();
        let target = crate::business_edit::Target {
            building: shop.runtime_slot.unwrap(),
            business: business.slot,
        };
        let edit = |action| {
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::EditBusiness {
                    target,
                    action,
                    reply,
                })
                .unwrap();
            answer.recv().unwrap().unwrap()
        };
        let old = edit(crate::business_edit::Action::Refresh);
        let price = edit(crate::business_edit::Action::Price {
            slot: 0,
            increase: true,
        });
        assert_eq!(
            price[0x1c4..0x1c8],
            old[0x1c4..0x1c8],
            "editing only changes pending price"
        );
        assert_ne!(price[0x1ec..0x1f0], old[0x1ec..0x1f0]);
        let stock = edit(crate::business_edit::Action::Stock {
            slot: 0,
            increase: false,
        });
        assert_eq!(
            stock[0x1b8..0x1ba],
            old[0x1b8..0x1ba],
            "editing only changes pending target"
        );
        let submitted = edit(crate::business_edit::Action::Apply);
        assert_eq!(submitted[0x208], 0);
        assert_eq!(
            submitted[0x1c4..0x1c8],
            old[0x1c4..0x1c8],
            "Apply preserves the real command delay"
        );
        sender.send(FreshRequest::Run(true)).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(15);
        let mut saw_prompt = false;
        loop {
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::Status {
                    camera: camera.eye,
                    reply,
                })
                .unwrap();
            let status = answer.recv().unwrap();
            assert!(
                status.failure.is_none(),
                "post-build worker stopped: {status:?}"
            );
            if let Some(prompt) = status.prompts.first() {
                assert!(status.script_paused);
                std::thread::sleep(std::time::Duration::from_millis(80));
                let (reply, paused_answer) = mpsc::channel();
                sender
                    .send(FreshRequest::Status {
                        camera: camera.eye,
                        reply,
                    })
                    .unwrap();
                let still_paused = paused_answer.recv().unwrap();
                assert_eq!(
                    still_paused.ticks, status.ticks,
                    "script pause suspends25Hz logic"
                );
                assert!(
                    still_paused.interface_ticks > status.interface_ticks,
                    "registered50Hz interface task continues during script pause"
                );
                assert!(still_paused.prompts.is_empty());
                assert_eq!(
                    prompt.dismissal_id, 1,
                    "original scenario3 callback identity"
                );
                let mut ui = crate::prompt::State::default();
                ui.submit(prompt.clone());
                let id = ui.begin_dismiss().unwrap();
                let (reply, answer) = mpsc::channel();
                sender
                    .send(FreshRequest::DismissPrompt(id, Some(reply)))
                    .unwrap();
                assert!(
                    ui.request().is_some(),
                    "keep slot active during worker callback"
                );
                ui.finish_dismiss(answer.recv().unwrap());
                assert!(ui.request().is_none());
                saw_prompt = true;
            }
            if status.ticks >= 350 {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "scripted prompt did not release live worker: {status:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        assert!(saw_prompt, "original9:30 scenario tip reached the host");
        sender.send(FreshRequest::Run(false)).unwrap();
        let applied = edit(crate::business_edit::Action::Refresh);
        assert_eq!(applied[0x1c4..0x1c8], price[0x1ec..0x1f0]);
        assert_eq!(applied[0x1b8..0x1ba], stock[0x1e0..0x1e2]);
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::Inspect {
                camera: near_camera,
                reply,
            })
            .unwrap();
        let final_snapshot = answer.recv().unwrap().unwrap();
        assert!(
            final_snapshot.reuse_city,
            "business terms preserve city geometry"
        );
        let final_shop = final_snapshot
            .scene
            .inspection_buildings
            .iter()
            .find(|b| b.runtime_slot == Some(target.building))
            .unwrap();
        assert_eq!(
            final_shop.initialized_business.as_ref().unwrap().record,
            applied,
            "cached geometry must expose the latest authoritative business record"
        );
        let (reply, answer) = mpsc::channel();
        sender.send(FreshRequest::Residents { reply }).unwrap();
        let mut moving = answer.recv().unwrap().unwrap();
        let (reply, answer) = mpsc::channel();
        sender.send(FreshRequest::Vehicles { reply }).unwrap();
        let mut traffic = answer.recv().unwrap().unwrap();
        assert!(traffic.active_count() > 0,
            "live original admission should activate road vehicle records");
        traffic.replace_attachment(final_snapshot.vehicles.as_ref().unwrap()).unwrap();
        let focus = traffic.first_active_position().unwrap();
        let near_road = CameraSample {
            eye: [focus[0], focus[1] + 4., focus[2] - 12.],
            target: focus,
            ..camera
        };
        let mut road_scene = final_snapshot.scene.clone();
        road_scene.reframe(near_road).unwrap();
        let road_draws = traffic.reframe(&mut road_scene, near_road, 35).unwrap();
        assert!(road_scene.draws[road_draws]
            .iter().any(|draw| !draw.vertices.is_empty()),
            "projected original road vehicles must submit visible mesh geometry");
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_LIVE_VEHICLE_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let prepared = renderer.prepare(
                &road_scene.draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>(),
                road_scene.camera,
                road_scene.fog,
            ).unwrap();
            crate::capture_test::capture_prepared(&renderer, &road_scene, &prepared, &path);
        }
        let moving_transport = final_snapshot.transport.as_ref().unwrap();
        let focus = moving_transport.first_position().unwrap();
        let near_transport = CameraSample {
            eye: [focus[0], focus[1] + 8., focus[2] - 24.],
            target: focus,
            ..camera
        };
        let mut transport_scene = final_snapshot.scene.clone();
        transport_scene.reframe(near_transport).unwrap();
        let transport_draws = moving_transport
            .reframe(&mut transport_scene, near_transport, 35).unwrap();
        assert!(transport_scene.draws[transport_draws]
            .iter().any(|draw| !draw.vertices.is_empty()),
            "native transport callbacks must submit visible meshes near their pose");
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_LIVE_TRANSPORT_CAPTURE") {
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            let prepared = renderer.prepare(
                &transport_scene.draws.iter().map(|draw| draw.borrowed()).collect::<Vec<_>>(),
                transport_scene.camera, transport_scene.fog,
            ).unwrap();
            crate::capture_test::capture_prepared(&renderer, &transport_scene, &prepared, &path);
        }
        assert!(
            moving.differs_from(loaded.people.as_ref().unwrap()),
            "resident-only transport must contain advanced live poses/frames"
        );
        moving
            .replace_attachment(final_snapshot.people.as_ref().unwrap())
            .unwrap();
        #[cfg(feature = "capture")]
        if let Ok(path) = std::env::var("OPENMT2001_LIVE_RESIDENT_CAPTURE") {
            let scene = &mut final_snapshot.scene.clone();
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            if cfg!(windows) {
                descriptor.backends = wgpu::Backends::DX12;
            }
            let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
            // Upload zero resident vertices, then prove an incoming population
            // snapshot can grow those buffers without replacing its textures.
            let empty = CameraSample {
                eye: [0., 3500., 10.],
                ..camera
            };
            moving.reframe(scene, empty, 35).unwrap();
            let mut prepared = renderer
                .prepare(
                    &scene.draws.iter().map(|d| d.borrowed()).collect::<Vec<_>>(),
                    scene.camera,
                    scene.fog,
                )
                .unwrap();
            let focus = moving.first_position();
            let near = CameraSample {
                eye: [focus[0], focus[1] + 4., focus[2] - 12.],
                target: focus,
                ..camera
            };
            let mut changed = scene.reframe(near).unwrap();
            let residents = moving.reframe(scene, near, 35).unwrap();
            assert!(
                scene.draws[residents.clone()]
                    .iter()
                    .any(|d| !d.vertices.is_empty())
            );
            changed.extend(residents.clone());
            for (i, draw) in scene.draws.iter().enumerate() {
                if changed.contains(&i) {
                    renderer
                        .update_dynamic_surface(&mut prepared, i, &draw.borrowed(), scene.camera, scene.fog,
                        )
                        .unwrap();
                } else {
                    renderer
                        .update_camera(
                            &prepared,
                            i,
                            draw.view_projection.unwrap_or(scene.camera),
                            scene.fog,
                        )
                        .unwrap();
                }
            }
            crate::capture_test::capture_prepared(&renderer, scene, &prepared, &path);
        }
        let inspect_operations = |building| {
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::InspectOperations {
                    building,
                    bounds: [30., 29., 190., 300.],
                    reply,
                })
                .unwrap();
            answer.recv().unwrap().unwrap()
        };
        let shop_controls = inspect_operations(target.building);
        assert_eq!(
            shop_controls.permissions.trade,
            Some(openmt2001::business_permissions::Trade::Sell)
        );
        assert!(shop_controls.permissions.convert && shop_controls.permissions.demolish);
        assert!(!shop_controls.report.summary.is_empty());
        #[cfg(feature = "capture")]
        crate::business_view::capture_inspection(&shop_controls);
        let apartment = final_snapshot
            .scene
            .inspection_buildings
            .iter()
            .find(|b| b.initialized_apartment.is_some())
            .unwrap()
            .runtime_slot
            .unwrap();
        let apartment_controls = inspect_operations(apartment);
        assert!(!apartment_controls.permissions.convert);
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::BusinessOperation {
                building: apartment,
                action: openmt2001::business_controls::Action::Convert {
                    selected_label: shop_controls.business_label,
                },
                reply,
            })
            .unwrap();
        assert!(
            answer
                .recv()
                .unwrap()
                .unwrap_err()
                .contains("original panel rules")
        );
        let (reply, answer) = mpsc::channel();
        sender
            .send(FreshRequest::BusinessOperation {
                building: target.building,
                action: openmt2001::business_controls::Action::Trade,
                reply,
            })
            .unwrap();
        assert!(answer.recv().unwrap().unwrap());
        assert!(
            inspect_operations(target.building)
                .permissions
                .trade
                .is_none(),
            "pending sale disables repeated submission before dispatch"
        );
        sender.send(FreshRequest::Run(true)).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(6);
        loop {
            let (reply, answer) = mpsc::channel();
            sender
                .send(FreshRequest::Status {
                    camera: camera.eye,
                    reply,
                })
                .unwrap();
            let status = answer.recv().unwrap();
            assert!(status.failure.is_none(), "worker sale failed: {status:?}");
            if status.ticks >= 380 {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        sender.send(FreshRequest::Run(false)).unwrap();
        assert_ne!(
            inspect_operations(target.building).permissions.trade,
            Some(openmt2001::business_permissions::Trade::Sell),
            "actual delayed sale removes ownership"
        );
        eprintln!(
            "Real worker building operations: native report, owned retail permissions, apartment conversion rejection, pending sale gate, delayed ownership transfer"
        );
        drop(final_snapshot);
        drop(built);
        drop(loaded);
        worker.join().unwrap();
    }
    #[test]
    #[ignore = "requires original Starting Out graphics/script inputs"]
    fn starting_out_early_controls_and_options_remain_live() {
        let read = |key| fs::read(std::env::var(key).unwrap()).unwrap();
        let map = PathBuf::from(std::env::var("OPENMT2001_MAP").unwrap());
        let parameter = PathBuf::from(std::env::var("OPENMT2001_PARAMETERS").unwrap());
        let executable = read("OPENMT2001_EXE");
        let exe_path = PathBuf::from(std::env::var("OPENMT2001_EXE").unwrap());
        let inputs = SceneInputs {
                process_runtime:None,
            compatibility: crate::compatibility::Mode::StrictV14,
                style: test_board_style(&executable),
                time_runtime: Default::default(),
                profile_root: std::env::temp_dir().join("omt-v2-capture-profiles"),
            character_names: crate::fresh_city::character_names(
                &openmt2001::pe::PeImage::parse(&executable).unwrap(),
                &exe_path
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("US/language/usa"),
            )
            .unwrap(),
            installer: parameter.parent().unwrap().parent().unwrap().to_path_buf(),
            language: exe_path.parent().unwrap().parent().unwrap().join("US/language/usa"),
            maps: map.parent().unwrap().to_path_buf(),
            build_file: read("OPENMT2001_BUILD_FILE"),
            executable,
            directory: read("OPENMT2001_ARCHIVE_DIR"),
            archive: read("OPENMT2001_ARCHIVE_BIN"),
            suncols: read("OPENMT2001_SUNCOLS"),
        };
        let inputs = Arc::new(inputs);
        let scripts = inputs.maps.parent().unwrap();
        let catalog = crate::campaign_store::load_catalog(
            &openmt2001::pe::PeImage::parse(&inputs.executable).unwrap(),
            scripts,
        )
        .unwrap();
        let setup = crate::singleplayer::Setup {
            preview: crate::singleplayer::Preview::Scenario(200003),
            difficulty: crate::singleplayer::Difficulty::Easy,
            character_index: 2,
            profile_name: b"Justin".to_vec(),
            catalog,
            profile_path: crate::campaign_store::profile_path(&inputs.profile_root, b"Justin"),
        };
        let mut controls = openmt2001::camera::BlockCamera {
            center: [0.; 3],
            angle: -std::f32::consts::FRAC_PI_2,
            height: 180.,
            radius: 220.,
        };
        let (eye, target) = controls.tick([false; 4]);
        let camera = CameraSample {
            eye,
            target,
            up: [0., 1., 0.],
            vertical_fov: 45.,
        };
        let (tx, rx) = mpsc::channel();
        let worker_inputs = inputs.clone();
        let worker = std::thread::Builder::new()
            .name("city-simulation-test".into())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || worker_inputs.preview_worker(setup, camera, tx))
            .unwrap();

        let loaded = rx.recv().unwrap().unwrap();
        let sender = loaded.fresh_session.as_ref().unwrap().clone();
        let status_raw = || {
            let (reply, answer) = mpsc::channel();
            sender.send(FreshRequest::Status { camera: camera.eye, reply,
                }).unwrap();
            answer.recv_timeout(std::time::Duration::from_secs(15)).unwrap()
        };
        // Every Status consumes its actual one-shot prompt batch, even when
        // this test asks only about running/failure. Retain all such batches.
        let retained_prompts=std::cell::RefCell::new(std::collections::VecDeque::new());
        let status=|| {
            let mut observed=status_raw();
            retained_prompts.borrow_mut().extend(std::mem::take(&mut observed.prompts));
            observed
        };
        sender.send(FreshRequest::Run(false)).unwrap();
        let initial = status();
        assert!(initial.failure.is_none(), "{initial:?}");
        assert!(initial.ticks < 312, "construction test must precede9:30 tip");
        for id in [11, 12, 13, 15, 16, 17, 18, 19, 20, 88] {
            sender.send(FreshRequest::TutorialControl(id)).unwrap();
        }
        sender.send(FreshRequest::TutorialProducer { control: 40, event: 7,
            }).unwrap();
        sender.send(FreshRequest::AuctionButton(0)).unwrap();
        let (reply, answer) = mpsc::channel();
        sender.send(FreshRequest::Planning { category: 33, reply,
            }).unwrap();
        let planning = answer.recv().unwrap().unwrap();
        assert_eq!(planning.entry(), crate::build_panel::Entry::Open);
        let (reply, answer) = mpsc::channel();
        sender.send(FreshRequest::BuildDrag { reply }).unwrap();
        answer.recv().unwrap().unwrap();
        let floor = u32::from_le_bytes(planning.catalog.row(0).unwrap()[0x48..0x4c]
            .try_into().unwrap(),
        ) as i32;
        let mut form = crate::build_panel::Request {
            category: 33, origin: [0, 0], size: [2, 2], height: floor,
            packed_model: 0, kind: 0, quality: 0, displayed_quote: f32::NAN,
            business: 0,
        };
        let mut allowed = false;
        for origin in [[0, 0], [0, 5], [5, 0], [5, 5]] {
            form.origin = origin;
            let (reply, answer) = mpsc::channel();
            sender.send(FreshRequest::Build { form: form.clone(), submit: false, reply,
                }).unwrap();
            if answer.recv().unwrap().unwrap().allowed() { allowed = true; break; }
        }
        assert!(allowed);
        let (reply, answer) = mpsc::channel();
        sender.send(FreshRequest::Build { form, submit: true, reply,
            }).unwrap();
        assert!(answer.recv().unwrap().unwrap().submitted);
        assert!(status().failure.is_none(), "building before first tip must remain legal");
        let options=|open| {
            let(reply,ack)=mpsc::channel();sender.send(FreshRequest::Options{open,reply}).unwrap();
            ack.recv_timeout(std::time::Duration::from_secs(15)).expect("actual Options owner reply").unwrap();
        };
        for running in [false, true] {
            sender.send(FreshRequest::Run(running)).unwrap();
            options(true);
            options(true);
            sender.send(FreshRequest::Run(true)).unwrap();
            assert!(!status().running);
            options(false);
            assert_eq!(status().running, running);
        }
        sender.send(FreshRequest::ToggleSpeed).unwrap();
        sender.send(FreshRequest::Run(true)).unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let paused = loop {
            // Leave a Status reply unread while opening Options. The worker
            // drains prompt events into that reply exactly once; the UI must
            // retain its receiver through the transition to display the tip.
            let (reply, answer) = mpsc::channel();
            sender.send(FreshRequest::Status { camera: camera.eye, reply,
                }).unwrap();
            options(true);
            let mut barrier = status_raw();
            assert!(!barrier.running && barrier.failure.is_none(), "{barrier:?}");
            let mut snapshot = answer.recv_timeout(std::time::Duration::from_secs(15)).unwrap();
            options(false);
            assert!(snapshot.failure.is_none(), "{snapshot:?}");
            // Worker FIFO produced A before B, regardless of which receiver
            // the test read first. Neither actual returned batch may be lost.
            if !snapshot.prompts.is_empty()||!barrier.prompts.is_empty(){
                eprintln!("retained tutorial Status batches A={} B={}",snapshot.prompts.len(),barrier.prompts.len());
            }
            retained_prompts.borrow_mut().extend(std::mem::take(&mut snapshot.prompts));
            retained_prompts.borrow_mut().extend(std::mem::take(&mut barrier.prompts));
            let has_tip=!retained_prompts.borrow().is_empty();
            if has_tip { break status(); } // actual post-close pause observation
            assert!(Instant::now() < deadline, "first original tip did not arrive");
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert!(paused.script_paused, "original prompt owns its own pause reason");
        assert_eq!(retained_prompts.borrow().front().unwrap().dismissal_id, 1);
        options(true);
        options(false);
        assert!(status().script_paused, "Options must not dismiss the scenario tip");
        let (reply, answer) = mpsc::channel();
        sender.send(FreshRequest::DismissPrompt(1, Some(reply))).unwrap();
        answer.recv().unwrap();
        assert_eq!(retained_prompts.borrow_mut().pop_front().unwrap().dismissal_id,1);
        let after = status();
        assert!(after.running && !after.script_paused && after.failure.is_none(), "{after:?}");
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let next = status();
            assert!(next.failure.is_none(), "{next:?}");
            if next.ticks > after.ticks { break; }
            assert!(Instant::now() < deadline, "dismissed tip did not resume logic");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        // The user crash occurred after construction and subsequent original
        // tips, when the hourly ship reached the actual rendering adapter.
        let image = openmt2001::pe::PeImage::parse(&inputs.executable).unwrap();
        let ships = std::array::from_fn::<_, 4, _>(|i| {
            u32::from_le_bytes(
            image.at(0x539d50 + i as u32 * 4, 4).unwrap().try_into().unwrap(),
            )
        });
        let deadline = Instant::now() + std::time::Duration::from_secs(45);
        loop {
            let next = status();
            assert!(next.failure.is_none(), "{next:?}");
            let prompts:Vec<_>=retained_prompts.borrow_mut().drain(..).collect();
            for prompt in prompts {
                let (reply, answer) = mpsc::channel();
                sender.send(FreshRequest::DismissPrompt(prompt.dismissal_id, Some(reply),
                    )).unwrap();
                answer.recv().unwrap();
            }
            if next.ticks >= 2000 { break; }
            assert!(Instant::now() < deadline, "Starting Out failed to reach its first hourly ship");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        sender.send(FreshRequest::Run(false)).unwrap();
        let (reply, answer) = mpsc::channel();
        sender.send(FreshRequest::Inspect { camera, reply }).unwrap();
        let mut snapshot = answer.recv().unwrap().unwrap();
        let transport = snapshot.transport.as_ref().unwrap();
        let references = transport.submitted_references().collect::<Vec<_>>();
        assert!(references.iter().any(|reference| ships.contains(reference)),
            "actual noon frame did not submit an hourly ship: {references:x?}");
        transport.reframe(&mut snapshot.scene, camera, 35).unwrap();
        assert!(status().failure.is_none());
        eprintln!("Starting Out: early build, stale controls, Options, queued tips and noon ship rendering pass ({references:x?})");
        drop(snapshot);
        drop(sender);
        drop(loaded);
        worker.join().unwrap();
    }

}

#[cfg(test)]
#[path="intro_autosave_natural_host_test.rs"]
mod natural_host_tests;

#[cfg(test)]
#[path="intro/focus_worker_tests.rs"]
mod focus_worker_tests;

#[cfg(test)]
#[path = "intro/camera_producer_live_tests.rs"]
mod camera_producer_live_tests;

#[cfg(test)]
#[path="intro/process_worker_tests.rs"]
mod process_worker_tests;

#[cfg(test)]
#[path="intro/process_review_tests.rs"]
pub(crate) mod process_review_tests;
