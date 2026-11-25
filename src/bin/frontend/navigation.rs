//! Original compass widget and strategic-view camera; native host presentation.
use crate::ui::{self, Surface};
use openmt2001::{
    archive::ArchiveIndex,
    flyby::CameraSample,
    pe::PeImage,
    render::{PreparedScene, Renderer},
    texture::TextureSet,
};
pub struct Navigation {
    pub bounds: [f32; 4],
    pub camera: CameraSample,
    pub detail_bounds: [f32; 4],
    detail_icons: [Surface; 2],
    detail_background: Surface,
    surfaces: Vec<Surface>,
    icons: [Surface; 3],
}
impl Navigation {
    pub fn load(index: &ArchiveIndex, archive: &[u8], executable: &[u8]) -> Result<Self, String> {
        let images = crate::intro::camera_live::process_images::Images::load(executable,index,archive,[640,480])?;
        Self::load_shared(index,archive,executable,&images)
    }
    pub(crate) fn load_shared(index:&ArchiveIndex,archive:&[u8],executable:&[u8],
        images:&crate::intro::camera_live::process_images::Images)->Result<Self,String>{
        let (mut surfaces, regions) = ui::panel_base(
            index,
            archive,
            "gamedata2\\brand_new_panels\\new_nav_bottom.tm",
            &[],
        )?;
        let image = PeImage::parse(executable)?;
        // Original425120 resolves NAV_BACKING_MAIN (original region0);
        //425440 fills it at depth0.5 before the three textured controls.
        let mut backing = Surface::solid();
        let color = u32::from_le_bytes(image.at(0x5362c0, 4)?.try_into().unwrap());
        backing.rect(*regions.first().ok_or("missing navigation backing")?, [color; 4]);
        surfaces.push(backing);
        let bounds = *regions.get(3).ok_or("missing compass region")?;
        let textures = images.textures()?;
        // Original425172..4251a5 assigns normal117, hover116, pressed115;
        // original4c0f9f registers the GTD at68ae28 in source order.
        if textures.textures.len() <= 117 {
            return Err("incomplete compass textures".into());
        }
        let icons = [117, 116, 115].map(|i| Surface::new(&textures.textures[i]));
        let entry = index
            .find("gamedata2/brand_new_panels/main_panel_v2.tm")
            .ok_or("missing main panel")?;
        let mesh = openmt2001::mesh::CachedMesh::parse(
            &archive[entry.offset as usize..(entry.offset + entry.length) as usize],
        )?;
        let detail_bounds = openmt2001::ui_mesh::regions(&mesh, 0)?
            .into_iter()
            .find(|r| r.name == "P_MAIN_OPENCLOSE_ICON")
            .ok_or("missing main panel toggle")?
            .bounds;
        // Original41fae0 uses68af44(open) /68af48(closed); registered from68ae28.
        let detail_icons =
            [false, true].map(|open| Surface::new(&textures.textures[detail_icon(open)]));
        Ok(Self {
            detail_bounds,
            detail_icons,
            detail_background: Surface::new(&textures.textures[88]),
            bounds,
            camera: strategic_camera(&image)?,
            surfaces,
            icons,
        })
    }
    pub fn prepare(&self, renderer: &Renderer) -> Result<[PreparedScene; 3], String> {
        prepare_icons(renderer, self.bounds, &self.surfaces, &self.icons)?
            .try_into()
            .map_err(|_| "compass states".into())
    }
    pub fn prepare_details(&self, renderer: &Renderer) -> Result<[PreparedScene; 2], String> {
        // Original41ddf7..41de12 assigns normal background68af88 (index88).
        let mut background = self.detail_background.clone();
        let [l, t, r, b] = self.detail_bounds;
        background.quad(
            [[l, t], [l, b], [r, b], [r, t]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
            [0xffffffff; 4],
        );
        prepare_icons(
            renderer,
            self.detail_bounds,
            &[background],
            &self.detail_icons,
        )?
        .try_into()
        .map_err(|_| "detail states".into())
    }
}
fn detail_icon(open: bool) -> usize {
    if open { 71 } else { 72 }
}
#[cfg(test)]
#[test]
#[ignore = "requires original bottom HUD backing oracle"]
fn original_navigation_backing() {
    let data = std::fs::read("work/frontend/native-bottom-hud.bin").unwrap();
    assert_eq!(&data[..8], b"MTBHUD01");
    let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
    let at = 20 + word(12) as usize;
    let directory = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
    let archive = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let executable = std::fs::read(std::env::var("OPENMT2001_EXE").unwrap()).unwrap();
    let navigation = Navigation::load(&index, &archive, &executable).unwrap();
    assert_eq!(navigation.surfaces.len(), 1);
    let actual = &navigation.surfaces[0];
    let [l, t, r, b] = std::array::from_fn(|i| f32::from_bits(word(at + i * 4)));
    let points = [[l, t], [l, b], [r, b], [r, t]];
    for (vertex, i) in [0, 1, 2, 0, 2, 3].into_iter().enumerate() {
        let color = word(at + 20 + i * 4);
        assert_eq!(actual.vertices[vertex][..3], [points[i][0], points[i][1],
            f32::from_bits(word(at + 36))]);
        assert_eq!(actual.alpha[vertex], (color >> 24) as u8);
        for (axis, shift) in [16, 8, 0].into_iter().enumerate() {
            assert_eq!(actual.vertices[vertex][axis + 3], ((color >> shift) & 255) as f32 / 255.);
        }
    }
}
#[cfg(all(test, feature = "capture"))]
#[test]
#[ignore = "requires original GenericTextures640 and GPU"]
fn capture_native_nav_control_textures() {
    let directory = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_DIR").unwrap()).unwrap();
    let archive = std::fs::read(std::env::var("OPENMT2001_ARCHIVE_BIN").unwrap()).unwrap();
    let index = ArchiveIndex::parse(&directory, archive.len() as u64).unwrap();
    let entry = index.find("gamedata2/GenericTextures640/textures.gtd").unwrap();
    let set = TextureSet::parse(&archive[entry.offset as usize
        ..(entry.offset + entry.length) as usize]).unwrap();
    let mut surfaces = vec![];
    for (cell, id) in [82, 83, 84, 17, 65, 191, 192, 193, 194, 115, 116, 117]
        .into_iter().enumerate() {
        let mut surface = Surface::new(&set.textures[id]);
        let x = (cell % 4) as f32 * 160.;
        let y = (cell / 4) as f32 * 160.;
        surface.quad([[x, y], [x, y + 120.], [x + 150., y + 120.], [x + 150., y]],
            [[0., 0.], [0., 1.], [1., 1.], [1., 0.]], [0xffffffff; 4]);
        surfaces.push(surface);
    }
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if cfg!(windows) { descriptor.backends = wgpu::Backends::DX12; }
    let renderer = pollster::block_on(Renderer::new(descriptor)).unwrap();
    let prepared = renderer.prepare(&surfaces.iter().map(Surface::draw).collect::<Vec<_>>(),
        [2./640.,0.,0.,0.,0.,-2./480.,0.,0.,0.,0.,1.,0.,-1.,1.,0.,1.], None).unwrap();
    crate::capture_test::capture_standalone(&renderer, &prepared,
        "work/frontend/nav-control-atlas.png");
}
fn prepare_icons(
    renderer: &Renderer,
    bounds: [f32; 4],
    base: &[Surface],
    icons: &[Surface],
) -> Result<Vec<PreparedScene>, String> {
    icons
        .iter()
        .map(|icon| {
            let mut surfaces = base.to_vec();
            let mut icon = icon.clone();
            let [l, t, r, b] = bounds;
            icon.quad(
                [[l, t], [l, b], [r, b], [r, t]],
                [[0., 0.], [0., 1.], [1., 1.], [1., 0.]],
                [0xffffffff; 4],
            );
            surfaces.push(icon);
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
        })
        .collect()
}
fn strategic_camera(image: &PeImage<'_>) -> Result<CameraSample, String> {
    // Original4b5509 supplies these coordinates to4e0260; mode4 callback is no-op.
    Ok(CameraSample {
        eye: [
            -220.,
            1050.,
            (-30f64 - f64::from(image.f32_at(0x52508c)?)) as f32,
        ],
        target: [-220., 0., -30.],
        up: [0., 1., 0.],
        vertical_fov: 45.,
    })
}
/// Original4c3c13..4c3c35 strategic category entry; center is the retained
/// category translation plus the original pavement model center.
pub fn block_entry(center: [f32; 3]) -> openmt2001::camera::BlockCamera {
    openmt2001::camera::BlockCamera {
        center,
        angle: f32::from_bits(0xbfc90fdb),
        height: 120.,
        radius: 80.,
    }
}

/// Original481cb0, default smart-camera branch (startup4c192a).
/// Inputs use category origin, building center, and the first category-relative entrance.
pub fn building_entry(
    category: [f32; 2],
    building: [f32; 2],
    entrance: Option<[f32; 3]>,
    landscape: bool,
    angle: f32,
    previous_target: f32,
) -> Option<(openmt2001::camera::BlockCamera, f32)> {
    building_entry_with_options(category, building, entrance, landscape, angle,
        previous_target, [category[0], 0., category[1]], true)
}

/// 481d01 branches on Gameplay AutoRotate at 9206f0. Disabled mode does not
/// inspect the selected building entrance: it focuses the parent category
/// model center, retains the current angle, and uses the 90/75 camera pose.
#[allow(clippy::too_many_arguments)]
pub fn building_entry_with_options(
    category: [f32; 2],
    building: [f32; 2],
    entrance: Option<[f32; 3]>,
    landscape: bool,
    angle: f32,
    previous_target: f32,
    category_model_center: [f32; 3],
    auto_rotate: bool,
) -> Option<(openmt2001::camera::BlockCamera, f32)> {
    if !auto_rotate {
        return Some((openmt2001::camera::BlockCamera {
            center: category_model_center,
            angle,
            height: 90.,
            radius: 75.,
        }, angle));
    }
    let vector = if landscape {
        std::array::from_fn(|i| (f64::from(category[i]) - f64::from(building[i])) as f32)
    } else {
        let p = entrance?;
        [-p[0], -p[2]]
    };
    let [x, z] = vector;
    // Original compares absolute FTOL results, not absolute float coordinates.
    let ax = (f64::from(x).trunc() as i32).unsigned_abs();
    let az = (f64::from(z).trunc() as i32).unsigned_abs();
    let mut target = previous_target;
    if az > ax {
        if z < 0. {
            target = f32::from_bits(0x3fc90fdb);
        } else if z > 0. {
            target = f32::from_bits(0xbfc90fdb);
        }
    } else if ax == az {
        target = match (x < 0., x > 0., z < 0., z > 0.) {
            (true, _, _, true) => f32::from_bits(0xbf490fdb),
            (true, _, true, _) => f32::from_bits(0x3f490fdb),
            (_, true, _, true) => f32::from_bits(0xc016cbe4),
            (_, true, true, _) => f32::from_bits(0x4016cbe4),
            _ => target,
        };
    } else if x < 0. {
        target = 0.;
    } else if x > 0. {
        target = f32::from_bits(0x40490fdb);
    }
    let tau = f32::from_bits(0x40c90fdb);
    let pi = f32::from_bits(0x40490fdb);
    let mut angle = angle;
    if angle > tau {
        angle = (f64::from(angle) - f64::from(tau)) as f32;
    }
    if angle < -tau {
        angle = (f64::from(angle) + f64::from(tau)) as f32;
    }
    let difference = f64::from(angle) - f64::from(target);
    if difference > f64::from(pi) {
        target = (f64::from(target) + f64::from(tau)) as f32;
    } else if difference < -f64::from(pi) {
        target = (f64::from(target) - f64::from(tau)) as f32;
    }
    Some((
        openmt2001::camera::BlockCamera {
            center: [
                (f64::from(category[0]) - f64::from(x)) as f32,
                20.,
                (f64::from(category[1]) - f64::from(z)) as f32,
            ],
            angle,
            height: 70.,
            radius: 45.,
        },
        target,
    ))
}
/// Mode13 callback481fe0 and angle easing4e0358. Existing block keys and
/// default right-drag numeric rules are identical; its target has no25-unit offset.
pub fn building_tick(
    controls: &mut openmt2001::camera::BlockCamera,
    target_angle: &mut f32,
    keys: [bool; 4],
    drag: [i32; 2],
) -> Result<([f32; 3], [f32; 3]), String> {
    building_tick_with_options(controls,target_angle,keys,drag,50,false)
}
/// Mode13 camera callback using the live Gameplay Options globals 9206f4/f1.
pub fn building_tick_with_options(
    controls: &mut openmt2001::camera::BlockCamera,
    target_angle: &mut f32,
    keys: [bool; 4],
    drag: [i32; 2],
    sensitivity: i32,
    invert_vertical: bool,
) -> Result<([f32; 3], [f32; 3]), String> {
    // Historical standalone API explicitly uses the zero predicate. Production
    // calls the height-taking entry below with its live process projection.
    building_tick_with_height(controls,target_angle,keys,drag,sensitivity,invert_vertical,false)
}
pub fn building_tick_with_height(
    controls:&mut openmt2001::camera::BlockCamera,target_angle:&mut f32,
    keys:[bool;4],drag:[i32;2],sensitivity:i32,invert_vertical:bool,low_height:bool,
)->Result<([f32;3],[f32;3]),String>{
    let target = controls.center;
    controls.tick(keys);
    let mut pose = ([0.; 3], target);
    if drag != [0; 2] {
        controls.right_drag(&mut pose, drag, sensitivity, invert_vertical, false, low_height)?;
        if drag[0] != 0 {
            *target_angle = controls.angle;
        }
    }
    let angle =
        (f64::from(*target_angle) - f64::from(controls.angle)) / 20. + f64::from(controls.angle);
    controls.angle = angle as f32;
    // Native FST stores angle but retains its extended value for FSIN; FCOS reloads it.
    let sin = f64::from(angle.sin() as f32);
    let cos = f64::from(f64::from(controls.angle).cos() as f32);
    pose.0 = [
        (f64::from(controls.radius) * cos + f64::from(controls.center[0])) as f32,
        controls.height,
        (f64::from(controls.radius) * sin + f64::from(controls.center[2])) as f32,
    ];
    Ok(pose)
}

/// Both-button translation shared by original mode12/13 callbacks. Pose is
/// emitted before input; mode13's subsequent angle easing recomputes eye only.
pub fn pan_tick(
    controls: &mut openmt2001::camera::BlockCamera,
    building_angle: Option<&mut f32>,
    keys: [bool; 4],
    delta: [i32; 2],
    sensitivity: i32,
) -> Result<([f32; 3], [f32; 3]), String> {
    if !(0..=100).contains(&sensitivity) || delta.iter().any(|v| v.unsigned_abs() > 4096) {
        return Err("unsupported camera pan input".into());
    }
    let mut previous = *controls;
    let old_pose = previous.tick([false; 4]);
    let old_target = controls.center;
    let sin = f64::from(f64::from(controls.angle).sin() as f32);
    let cos = f64::from(f64::from(controls.angle).cos() as f32);
    let speed = f64::from((f64::from(sensitivity) * f64::from(0.01f32) * 2. + 1.) as f32);
    let [dx, dy] = delta.map(f64::from);
    controls.center[0] = (f64::from(controls.center[0]) - dx * sin * speed) as f32;
    controls.center[2] = (f64::from(controls.center[2]) + dx * cos * speed) as f32;
    controls.center[0] = (f64::from(controls.center[0]) + dy * cos * speed) as f32;
    controls.center[2] = (f64::from(controls.center[2]) + dy * sin * speed) as f32;
    if let Some(angle) = building_angle {
        let (eye, _) = building_tick(controls, angle, keys, [0; 2])?;
        Ok((eye, old_target))
    } else {
        controls.tick(keys);
        Ok(old_pose)
    }
}

/// Host event classifier: original4c6cd2 consumes Windows message0x203.
/// Winit exposes button edges, so classify using Windows system preferences.
/// Non-Windows hosts currently use500ms/4px, not recovered platform preferences.
#[derive(Default)]
pub struct DoubleClick {
    previous: Option<(std::time::Duration, [f64; 2])>,
}
impl DoubleClick {
    pub fn clear(&mut self) {
        self.previous = None;
    }
    pub fn press(&mut self, now: std::time::Duration, point: [f64; 2]) -> bool {
        let (milliseconds, rectangle) = double_click_settings();
        self.classify(
            now,
            point,
            std::time::Duration::from_millis(milliseconds.into()),
            rectangle,
        )
    }
    fn classify(
        &mut self,
        now: std::time::Duration,
        point: [f64; 2],
        limit: std::time::Duration,
        rectangle: [i32; 2],
    ) -> bool {
        let double = self.previous.is_some_and(|(time, first)| {
            now.checked_sub(time)
                .is_some_and(|elapsed| elapsed <= limit)
                && (0..2).all(|i| {
                    let half = f64::from(rectangle[i] / 2);
                    point[i] >= first[i] - half && point[i] < first[i] + half
                })
        });
        self.previous = if double { None } else { Some((now, point)) };
        double
    }
}
#[cfg(target_os = "windows")]
fn double_click_settings() -> (u32, [i32; 2]) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDoubleClickTime() -> u32;
        fn GetSystemMetrics(index: i32) -> i32;
    }
    // Scalar read-only Win32 APIs;36/37 are SM_CXDOUBLECLK/SM_CYDOUBLECLK.
    unsafe {
        (
            GetDoubleClickTime(),
            [GetSystemMetrics(36), GetSystemMetrics(37)],
        )
    }
}
#[cfg(not(target_os = "windows"))]
fn double_click_settings() -> (u32, [i32; 2]) {
    (500, [4, 4])
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires original both-button camera oracle"]
    fn matches_native_camera_pan() {
        let b =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_CAMERA_PAN").expect("fixture")).unwrap();
        assert_eq!(&b[..8], b"MTCPAN01");
        let w = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let f = |at| f32::from_bits(w(at));
        let mut at = 12;
        for case in 0..w(8) {
            let mode = w(at);
            let mut c = openmt2001::camera::BlockCamera {
                center: std::array::from_fn(|i| f(at + 4 + i * 4)),
                angle: f(at + 16),
                height: 120.,
                radius: 80.,
            };
            let mut goal = (f64::from(c.angle) + 0.5) as f32;
            let delta = [w(at + 20) as i32, w(at + 24) as i32];
            let speed = w(at + 28) as i32;
            let keys = std::array::from_fn(|i| w(at + 32) & (1 << i) != 0);
            let (eye, target) = super::pan_tick(
                &mut c,
                (mode == 13).then_some(&mut goal),
                keys,
                delta,
                speed,
            )
            .unwrap();
            let actual: Vec<_> = eye
                .into_iter()
                .chain(target)
                .chain(c.center)
                .chain([c.angle])
                .flat_map(f32::to_le_bytes)
                .collect();
            assert_eq!(actual, b[at + 36..at + 76], "case{case} mode{mode}");
            at += 76;
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "requires original main-panel toggle oracle"]
    fn matches_native_panel_toggle() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_PANEL_TOGGLE").expect("fixture"))
            .unwrap();
        assert_eq!(&b[..8], b"MTTOGL01");
        let w = |at| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        let length = w(8) as usize;
        let frames = w(12) as usize;
        let mesh = openmt2001::mesh::CachedMesh::parse(&b[16..16 + length]).unwrap();
        let mut at = 16 + length;
        for frame in 0..frames {
            let region = openmt2001::ui_mesh::regions(&mesh, frame)
                .unwrap()
                .into_iter()
                .find(|r| r.name == "P_MAIN_OPENCLOSE_ICON")
                .unwrap();
            assert_eq!(region.part, 12);
            assert_eq!(
                region.bounds.map(f32::to_bits),
                std::array::from_fn(|i| w(at + i * 4))
            );
            at += 16;
        }
        for _ in 0..3 {
            let open = w(at) == 0;
            assert_eq!(u32::from(open), w(at + 4));
            assert_eq!(super::detail_icon(open), w(at + 8) as usize);
            at += 12;
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "requires original v1.4 AutoRotate-off building camera oracle"]
    fn matches_native_auto_rotate_off_camera() {
        let bytes = std::fs::read("work/frontend/native-auto-rotate-camera.bin").unwrap();
        assert_eq!(&bytes[..8], b"MTAUTO01");
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let count = word(8) as usize;
        assert_eq!(bytes.len(), 12 + count * 56);
        for case in 0..count {
            let at = 12 + case * 56;
            let input = std::array::from_fn::<_, 7, _>(|i| f32::from_bits(word(at + i * 4)));
            let category_model_center = std::array::from_fn(|i| {
                (f64::from(input[i]) + f64::from(input[3 + i])) as f32
            });
            let (controls, goal) = super::building_entry_with_options(
                [input[0], input[2]], [1000., -1000.], None, false, input[6],
                123., category_model_center, false).unwrap();
            let actual = controls.center.into_iter()
                .chain([controls.angle, goal, controls.height, controls.radius])
                .flat_map(f32::to_le_bytes).collect::<Vec<_>>();
            assert_eq!(actual, bytes[at + 28..at + 56], "AutoRotate off case {case}");
        }
    }
    #[test]
    #[ignore = "requires original building-camera oracle"]
    fn matches_native_building_camera() {
        let bytes =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_BUILDING_CAMERA").expect("fixture"))
                .unwrap();
        assert_eq!(&bytes[..8], b"MTBLDC01");
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let cases = word(8) as usize;
        let ticks = word(12) as usize;
        let mut at = 16;
        for case in 0..cases {
            let f = |p| f32::from_bits(word(p));
            let x = f(at);
            let z = f(at + 4);
            let angle = f(at + 8);
            let kind = word(at + 12);
            at += 16;
            let (mut controls, mut goal) = super::building_entry(
                [120., -180.],
                [120. - x, -180. - z],
                Some([-x, 6., -z]),
                kind == 2,
                angle,
                0.,
            )
            .unwrap();
            let state = |c: &openmt2001::camera::BlockCamera, g: f32| {
                c.center
                    .into_iter()
                    .chain([c.angle, g, c.height, c.radius])
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                state(&controls, goal),
                bytes[at..at + 28],
                "setup case{case}"
            );
            assert_eq!(&bytes[at + 28..at + 35], &[1, 1, 1, 20, 0, 0, 0]);
            at += 35;
            for tick in 0..ticks {
                let keys = std::array::from_fn(|i| tick % 11 == i);
                let drag = if tick >= 40 {
                    [[-20, 15], [14, -10], [0, 0]][tick % 3]
                } else {
                    [0, 0]
                };
                let (eye, target) =
                    super::building_tick(&mut controls, &mut goal, keys, drag).unwrap();
                let actual = state(&controls, goal)
                    .into_iter()
                    .chain(eye.into_iter().chain(target).flat_map(f32::to_le_bytes))
                    .collect::<Vec<_>>();
                assert_eq!(actual, bytes[at..at + 52], "case{case} tick{tick}");
                at += 52;
            }
        }
        assert_eq!(at, bytes.len());
        assert!(super::building_entry([0.; 2], [0.; 2], None, false, 0., 0.).is_none());
    }
    #[test]
    fn host_click_pair_limits_and_reset() {
        use std::time::Duration;
        let mut clicks = super::DoubleClick::default();
        let mut press = |ms, point| {
            clicks.classify(
                Duration::from_millis(ms),
                point,
                Duration::from_millis(500),
                [4, 4],
            )
        };
        assert!(!press(0, [20., 20.]));
        assert!(press(500, [18., 21.]));
        assert!(!press(510, [20., 20.])); // consumed pair does not form a triple
        assert!(!press(1011, [20., 20.])); // time outside preference
        assert!(!press(1020, [22., 20.])); // right edge is outside rectangle
        assert!(press(1030, [23., 20.]));
        clicks.clear();
        assert!(!clicks.classify(
            Duration::from_millis(1040),
            [23., 20.],
            Duration::from_millis(500),
            [4, 4]
        ));
    }
    #[test]
    #[ignore = "requires original strategic-entry oracle fixture"]
    fn matches_native_strategic_block_entry() {
        let b = std::fs::read(std::env::var("OPENMT2001_NATIVE_STRATEGIC_ENTRY").expect("fixture"))
            .unwrap();
        assert_eq!(&b[..8], b"MTDBL001");
        assert_eq!(u32::from_le_bytes(b[8..12].try_into().unwrap()), 3);
        let mut at = 12;
        for _ in 0..3 {
            let f = |i| f32::from_le_bytes(b[at + i * 4..at + i * 4 + 4].try_into().unwrap());
            let center = std::array::from_fn(|i| (f64::from(f(i)) + f64::from(f(i + 3))) as f32);
            let state = super::block_entry(center);
            let actual: Vec<_> = state
                .center
                .into_iter()
                .chain([state.angle, state.height, state.radius])
                .flat_map(f32::to_le_bytes)
                .collect();
            assert_eq!(actual, b[at + 24..at + 48]);
            assert_eq!(&b[at + 48..at + 59], &[12, 0, 0, 0, 1, 1, 0, 20, 0, 0, 0]);
            at += 59;
        }
        for message in 0x201u32..=0x205 {
            assert_eq!(
                u32::from_le_bytes(b[at..at + 4].try_into().unwrap()),
                message
            );
            assert_eq!(b[at + 4], u8::from(message == 0x203));
            at += 5;
        }
        assert_eq!(at, b.len());
    }
    #[test]
    #[ignore = "requires original executable and strategic camera oracle"]
    fn matches_native_strategic_camera() {
        let b =
            std::fs::read(std::env::var("OPENMT2001_NATIVE_STRATEGIC_CAMERA").expect("fixture"))
                .unwrap();
        let exe = std::fs::read(std::env::var("OPENMT2001_EXE").expect("executable")).unwrap();
        assert_eq!(&b[..8], b"MTSTRAT1");
        let pose = super::strategic_camera(&openmt2001::pe::PeImage::parse(&exe).unwrap()).unwrap();
        let actual: Vec<_> = pose
            .eye
            .into_iter()
            .chain(pose.target)
            .chain(pose.up)
            .flat_map(f32::to_le_bytes)
            .collect();
        assert_eq!(actual, b[8..44]);
        assert_eq!(&b[44..], &[4, 0, 0, 0, 0, 0, 0]);
    }
}
