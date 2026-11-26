use crate::present::Presenter;
use rodio::Source;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{Window, WindowId},
};
type Error = Box<dyn std::error::Error>;
const FRAME_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// Physical history belongs to App, ahead of every inspection/modal/pending
/// return. The result is event-local, never queued across redraw/key events.
#[derive(Default)]
pub(crate) struct ParticipantLeft { held: bool }
impl ParticipantLeft {
    pub(crate) fn sample(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseInput { button: MouseButton::Left, state, .. } => {
                let down = *state == ElementState::Pressed;
                let fresh = down && !self.held;
                self.held = down;
                fresh
            }
            WindowEvent::Focused(false) => { self.held = false; false }
            _ => false,
        }
    }
}
/// Same upstream pending gate used by actual App dispatch and regression.
/// Sampling is deliberately separate and must occur even if this returns early.
pub(crate) fn route_menu_left(menu: &mut crate::menu::Menu, pressed: bool, fresh: bool) -> bool {
    if menu.pending.is_none() || menu.desktop_quit_active() {
        menu.mouse_button_with_participant_press(pressed, fresh);
        menu.light_click_on_held_entry()
    } else { false }
}
struct Pixels {
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}
struct Assets {
    movie: Vec<u8>,
    audio: Vec<f32>,
    ui_light_click: Vec<f32>,
    deepred: Pixels,
    warning: Pixels,
    loading: Pixels,
}
fn command(ffmpeg: &Path, movie: &Path, args: &[&str]) -> Result<Vec<u8>, Error> {
    let mut command = Command::new(ffmpeg);
    command
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(movie)
        .args(args)
        .arg("pipe:1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // helper has no visible console
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "Media decode failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output.stdout)
}
fn load_intro_music(ffmpeg: PathBuf, path: PathBuf)
    -> std::sync::mpsc::Receiver<Result<Vec<f32>, String>>
{
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let decoded = command(
            &ffmpeg,
            &path,
            &["-vn", "-ac", "2", "-ar", "44100", "-f", "f32le"],
        )
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            if bytes.is_empty() || !bytes.len().is_multiple_of(8) {
                return Err("Incomplete original intro music samples".to_owned());
            }
            Ok(bytes
                .chunks_exact(4)
                .map(|sample| f32::from_le_bytes(sample.try_into().unwrap()))
                .collect())
        });
        let _ = sender.send(decoded);
    });
    receiver
}
fn image(path: PathBuf) -> Result<Pixels, Error> {
    let image = image::open(&path)?.into_rgba8();
    Ok(Pixels {
        width: image.width(),
        height: image.height(),
        bytes: image.into_raw(),
    })
}
impl Assets {
    fn load(root: &Path, ffmpeg: &Path, skip_splash: bool) -> Result<Self, Error> {
        let (movie, audio) = if skip_splash {
            (Vec::new(), Vec::new())
        } else {
            let movie_path = root.join("Standard/infogrames.mpg");
            let movie = command(
                ffmpeg,
                &movie_path,
                &[
                    "-an",
                    "-vf",
                    "scale=320:240,fps=25",
                    "-pix_fmt",
                    "rgba",
                    "-f",
                    "rawvideo",
                ],
            )?;
            if movie.is_empty() || !movie.len().is_multiple_of(320 * 240 * 4) {
                return Err("Incomplete original movie frames".into());
            }
            let audio = command(
                ffmpeg,
                &movie_path,
                &["-vn", "-ac", "2", "-ar", "44100", "-f", "f32le"],
            )?;
            let audio = audio
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            (movie, audio)
        };
        Ok(Self {
            movie,
            audio,
            ui_light_click: crate::gameplay_sound::pcm16(
                &root.join("Standard/sound/high/spot_any_ui_lightclick.wav"),
            )?,
            deepred: image(root.join("Standard/Screens/DeepRed.tga"))?,
            warning: image(root.join("US/language/usa/Warning.tga"))?,
            loading: image(root.join("US/language/usa/loading.tga"))?,
        })
    }
}
struct App {
    intro_loading: std::sync::mpsc::Receiver<Result<crate::intro::Assets, String>>,
    intro: Option<crate::intro::Intro>,
    cursors: Option<[winit::window::CustomCursor; 4]>,
    cursor_kind: Option<usize>,
    menu_onload: bool,
    modifiers: ModifiersState,
    assets: Assets,
    presenter: Option<Presenter>,
    stage: usize,
    since: Instant,
    audio_bus: crate::audio_bus::Bus,
    sink: Option<rodio::Sink>,
    ui_click_sinks: Vec<rodio::Sink>,
    intro_music_loading: Option<std::sync::mpsc::Receiver<Result<Vec<f32>, String>>>,
    intro_music_playing: bool,
    intro_music_attempted: bool,
    intro_music_path: PathBuf,
    intro_music_decoder: PathBuf,
    failure: Option<String>,
    exit_after: Option<Duration>,
    launched: Instant,
    frame_profile: Option<FrameProfile>,
    next_redraw: Instant,
    desktop_keys: crate::menu::desktop_quit::Keys,
    participant_left: ParticipantLeft,
    window_focus: Option<bool>,
}
struct FrameProfile {
    started: Instant,
    frames: u32,
    draw_time: Duration,
    slowest: Duration,
}
impl FrameProfile {
    fn sample(&mut self, elapsed: Duration, inspecting: bool, presenter: &Presenter) {
        if self.frames == 0 {
            self.started = Instant::now();
        }
        self.frames += 1;
        self.draw_time += elapsed;
        self.slowest = self.slowest.max(elapsed);
        let window = self.started.elapsed();
        if window >= Duration::from_secs(2) {
            eprintln!(
                "FRAME {} CPU_attempts={:.1}/s draw_mean={:.2}ms draw_max={:.2}ms",
                if inspecting { "city" } else { "intro/menu" },
                f64::from(self.frames) / window.as_secs_f64(),
                self.draw_time.as_secs_f64() * 1000. / f64::from(self.frames),
                self.slowest.as_secs_f64() * 1000.,
            );
            presenter.report_presentation();
            self.started = Instant::now();
            self.frames = 0;
            self.draw_time = Duration::ZERO;
            self.slowest = Duration::ZERO;
        }
    }
}
impl App {
    fn play_menu_light_click(&mut self) {
        if let Some(mixer) = self.audio_bus.mixer() {
            self.ui_click_sinks.retain(|sink| !sink.empty());
            let sink = rodio::Sink::connect_new(mixer);
            sink.set_volume(self.intro.as_ref().map_or(1., |intro| {
                f32::from(intro.ui_effects_level()) / 100.
            }));
            sink.append(rodio::buffer::SamplesBuffer::new(
                1,
                22_050,
                self.assets.ui_light_click.clone(),
            ));
            self.ui_click_sinks.push(sink);
        }
    }
    fn update_intro_music(&mut self) {
        if self.intro.as_ref().is_some_and(|intro| intro.inspecting()) {
            if self.intro_music_playing {
                if let Some(sink) = &self.sink {
                    sink.stop();
                }
                self.intro_music_playing = false;
            }
            self.intro_music_loading = None;
            self.intro_music_attempted = false;
            return;
        }
        if !self.intro_music_attempted {
            self.intro_music_loading = Some(load_intro_music(
                self.intro_music_decoder.clone(), self.intro_music_path.clone()));
            self.intro_music_attempted = true;
        }
        let Some(loading) = &self.intro_music_loading else {
            return;
        };
        match loading.try_recv() {
            Ok(Ok(samples)) => {
                self.intro_music_loading = None;
                if let Some(mixer) = self.audio_bus.mixer() {
                    let sink = rodio::Sink::connect_new(mixer);
                    sink.append(rodio::buffer::SamplesBuffer::new(2, 44_100, samples).repeat_infinite());
                    self.sink = Some(sink);
                    self.intro_music_playing = true;
                    eprintln!("Audio: original intro music started");
                } else {
                    eprintln!("Audio device unavailable; intro music is silent");
                }
            }
            Ok(Err(error)) => {
                self.intro_music_loading = None;
                eprintln!("Intro music unavailable: {error}");
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.intro_music_loading = None;
                eprintln!("Intro music decoder disconnected");
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => (),
        }
    }
    fn advance(&mut self) {
        if self.stage == 0
            && let Some(sink) = &self.sink
        {
            sink.stop();
        }
        self.stage = (self.stage + 1).min(3);
        self.since = Instant::now();
        eprintln!(
            "Boot stage: {}",
            [
                "Infogrames movie",
                "DeepRed",
                "USA legal notice",
                "Loading original 3D intro"
            ][self.stage]
        );
        if self.stage == 3
            && let Some(p) = &self.presenter
        {
            p.window.set_title("OpenMT2001 - loading original intro");
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.presenter.is_some() {
            return;
        }
        let result = (|| -> Result<Presenter, Error> {
            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("OpenMT2001")
                        .with_inner_size(winit::dpi::LogicalSize::new(960., 720.)),
                )?,
            );
            pollster::block_on(Presenter::new(window))
        })();
        match result {
            Ok(p) => {
                self.window_focus = Some(p.window.has_focus());
                self.presenter = Some(p);
                self.since = Instant::now();
                self.launched = self.since;
                if self.stage == 0 {
                    match self.audio_bus.mixer() {
                        Some(mixer) => {
                            let sink = rodio::Sink::connect_new(mixer);
                            sink.append(rodio::buffer::SamplesBuffer::new(
                                2,
                                44100,
                                std::mem::take(&mut self.assets.audio),
                            ));
                            self.sink = Some(sink);
                        }
                        None => {
                            eprintln!("Audio device unavailable; playing movie silently")
                        }
                    }
                    eprintln!("Boot stage: Infogrames movie");
                }
            }
            Err(e) => {
                self.failure = Some(e.to_string());
                event_loop.exit();
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(intro) = &mut self.intro { intro.camera_physical_event(&event); }
        // Process focus is observed even during loading and before every
        // inspection/modal early return. Sending never waits for worker effects.
        if let WindowEvent::Focused(focused) = &event {
            self.window_focus = Some(*focused);
            if let Some(intro) = &mut self.intro {
                if let Err(error) = intro.window_focus(*focused) {
                    self.failure = Some(error);
                    event_loop.exit();
                    return;
                }
            }
        }
        if matches!(&event, WindowEvent::RedrawRequested) {
            self.next_redraw = Instant::now() + FRAME_INTERVAL;
        }
        let participant_down = self.participant_left.sample(&event);
        // Track physical samples BEFORE logical routing, including releases,
        // repeats and events consumed by inspection. No city behavior changes.
        if self.stage == 3 {
            match &event {
                WindowEvent::KeyboardInput { event, .. } => self.desktop_keys.physical(
                    event.physical_key, event.state == ElementState::Pressed, event.repeat),
                WindowEvent::MouseInput { button: MouseButton::Right, state, .. } =>
                    self.desktop_keys.mouse_right(*state == ElementState::Pressed),
                WindowEvent::Focused(false) => self.desktop_keys.release_on_unfocus(),
                _ => (),
            }
            if let Some(menu) = self.intro.as_mut().and_then(|intro| intro.menu.as_mut()) {
                menu.desktop_right_button(self.desktop_keys.right());
            }
        }
        if let Some(intro) = &mut self.intro
            && intro.inspecting()
        {
            match &event {
                WindowEvent::KeyboardInput { event, .. } => {
                    if let Key::Named(key) = event.logical_key
                        && let Err(e) =
                            intro.inspection_key(key, event.state == ElementState::Pressed)
                    {
                        self.failure = Some(e);
                        event_loop.exit();
                    }
                    if event.state == ElementState::Pressed
                        && let Key::Character(text) = &event.logical_key
                        && !self.modifiers.control_key() && !self.modifiers.super_key()
                    {
                        intro.inspection_text(text);
                    }
                    return;
                }
                WindowEvent::Focused(false) => intro.inspection_unfocus(),
                WindowEvent::CursorMoved { position, .. } => {
                    if let Some(presenter) = &self.presenter {
                        let size = presenter.window.inner_size();
                        intro
                            .inspection_cursor([position.x, position.y], [size.width, size.height]);
                    }
                    return;
                }
                WindowEvent::MouseInput { button, state, .. } => {
                    intro.inspection_mouse(*button, *state == ElementState::Pressed);
                    return;
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let y = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y as f64,
                        winit::event::MouseScrollDelta::PixelDelta(p) => p.y,
                    };
                    if y != 0. {
                        intro.inspection_scroll(if y > 0. { -1 } else { 1 });
                    }
                    return;
                }
                _ => (),
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(intro)=&self.intro {
                    if let Err(error)=intro.request_process_exit(crate::live_host::process_runtime::QuitWriter::Window){self.failure=Some(error);}
                    match intro.process_exit_ready(){Ok(true)=>event_loop.exit(),Ok(false)=>{},Err(error)=>{self.failure=Some(error);event_loop.exit();}}
                }else{event_loop.exit();}
            },
            WindowEvent::Focused(false) => {
                if let Some(menu) = self.intro.as_mut().and_then(|intro| intro.menu.as_mut()) {
                    menu.cancel_input();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                // An authentic quit ignores logical menu navigation/text.
                // Its physical edges are consumed on the next UI event1 tick.
                if self.stage == 3 && self.intro.as_ref()
                    .and_then(|intro| intro.menu.as_ref())
                    .is_some_and(|menu| menu.desktop_input_blocked()) { return; }
                match event.logical_key {
                    Key::Named(key) if self.stage == 3 => {
                        if let Some(intro) = &mut self.intro {
                            if let Some(menu) = &mut intro.menu {
                                match key {
                                    NamedKey::Escape => menu.back(),
                                    NamedKey::ArrowDown | NamedKey::Tab => menu.navigate(1),
                                    NamedKey::ArrowUp => menu.navigate(-1),
                                    NamedKey::Backspace => menu.erase(),
                                    NamedKey::Enter | NamedKey::Space => {
                                        if key == NamedKey::Space && menu.is_editing() {
                                            menu.input(" ");
                                        } else if menu.pending.is_none() {
                                            menu.activate();
                                        }
                                    }
                                    _ => (),
                                }
                            } else if matches!(
                                key,
                                NamedKey::Escape | NamedKey::Space | NamedKey::Enter
                            ) {
                                intro.open_menu();
                            }
                        } else if key == NamedKey::Escape {
                            event_loop.exit();
                        }
                    }
                    Key::Named(NamedKey::Escape | NamedKey::Space | NamedKey::Enter)
                        if self.stage == 0 =>
                    {
                        self.advance()
                    }
                    Key::Character(text) if self.stage == 3 => {
                        if let Some(intro) = &mut self.intro
                            && let Some(menu) = &mut intro.menu
                        {
                            if self.modifiers.control_key() || self.modifiers.super_key() {
                                if text.eq_ignore_ascii_case("v") && menu.is_editing() {
                                    if let Ok(mut clipboard) = arboard::Clipboard::new()
                                        && let Ok(text) = clipboard.get_text()
                                    {
                                        menu.input(&text);
                                    }
                                } else if text.eq_ignore_ascii_case("a") {
                                    menu.select_all();
                                }
                            } else {
                                menu.input(&text);
                            }
                        }
                    }
                    _ => (),
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let mut light_click = false;
                if let (Some(intro), Some(p)) = (&mut self.intro, &self.presenter)
                    && let Some(menu) = &mut intro.menu
                    && (menu.pending.is_none() || menu.desktop_quit_active())
                {
                    let size = p.window.inner_size();
                    let scale = (size.width as f32 / 640.).min(size.height as f32 / 480.);
                    if scale > 0. {
                        let point = [
                            (position.x as f32 - (size.width as f32 - scale * 640.) / 2.) / scale,
                            (position.y as f32 - (size.height as f32 - scale * 480.) / 2.) / scale,
                        ];
                        menu.pointer(point);
                        light_click = menu.light_click_on_held_entry();
                    }
                }
                if light_click {
                    self.play_menu_light_click();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let mut light_click = false;
                if let Some(intro) = &mut self.intro {
                    if let Some(menu) = &mut intro.menu {
                        light_click = route_menu_left(menu,
                            state == ElementState::Pressed, participant_down);
                    } else if state == ElementState::Pressed {
                        intro.open_menu();
                    }
                }
                if light_click {
                    self.play_menu_light_click();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(intro) = &mut self.intro
                    && let Some(menu) = &mut intro.menu
                {
                    let y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 20.,
                    };
                    menu.scroll(-y.signum() as i32);
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(p) = &mut self.presenter {
                    p.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.stage == 3 {
                    if self.intro.is_none() {
                        match self.intro_loading.try_recv() {
                            Ok(Ok(mut assets)) => {
                                match assets.cursors.take().unwrap().create(event_loop) {
                                    Ok(cursors) => self.cursors = Some(cursors),
                                    Err(e) => {
                                        self.failure = Some(e);
                                        event_loop.exit();
                                        return;
                                    }
                                }
                                let result = crate::intro::Intro::new(
                                    self.presenter.as_ref().unwrap(),
                                    assets,
                                );
                                match result {
                                    Ok(mut intro) => {
                                        if let Some(focused) = self.window_focus {
                                            // Initial observation only; the
                                            // Session attachment occurs later.
                                            if let Err(error) = intro.window_focus(focused) {
                                                self.failure = Some(error);
                                                event_loop.exit();
                                                return;
                                            }
                                        }
                                        if self.menu_onload {
                                            intro.open_menu();
                                        }
                                        self.intro = Some(intro);
                                        self.presenter.as_ref().unwrap().reset_presentation_profile();
                                        self.presenter.as_ref().unwrap().window.set_title(
                                            "OpenMT2001 - original 3D intro / main menu",
                                        );
                                        eprintln!("Boot stage: original 3D intro");
                                    }
                                    Err(e) => {
                                        self.failure = Some(e);
                                        event_loop.exit();
                                        return;
                                    }
                                }
                            }
                            Ok(Err(e)) => {
                                self.failure = Some(e);
                                event_loop.exit();
                                return;
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => (),
                            Err(e) => {
                                self.failure = Some(e.to_string());
                                event_loop.exit();
                                return;
                            }
                        }
                    }
                    if self.intro.is_some() {
                        self.update_intro_music();
                    }
                    // One sampled UI event1 per menu redraw; current/previous
                    // survive opening and closing. Camera50Hz is a separate clock.
                    let sample = self.desktop_keys.sample();
                    if let Some(menu) = self.intro.as_mut().and_then(|intro| intro.menu.as_mut()) {
                        menu.desktop_ui_event1(sample);
                        if menu.take_desktop_exit() {
                            //Writer only; an in-progress central quit still waits
                            //for its actual tail. Normal menu exit has no tail.
                            if let Some(intro)=&self.intro {
                                if let Err(error)=intro.request_process_exit(crate::live_host::process_runtime::QuitWriter::Menu){self.failure=Some(error);event_loop.exit();return;}
                            }
                        }
                    }
                    if let Some(intro)=&self.intro {
                        match intro.process_exit_ready(){Ok(true)=>{event_loop.exit();return;},Ok(false)=>{},Err(error)=>{self.failure=Some(error);event_loop.exit();return;}}
                    }
                    if let Some(intro) = &mut self.intro {
                        let draw_started = self.frame_profile.as_ref().map(|_| Instant::now());
                        if let Err(e) = intro.draw(self.presenter.as_ref().unwrap()) {
                            self.failure = Some(e);
                            event_loop.exit();
                        }
                        if let (Some(profile), Some(started)) =
                            (&mut self.frame_profile, draw_started)
                        {
                            profile.sample(started.elapsed(), intro.inspecting(), self.presenter.as_ref().unwrap());
                        }
                        let kind = intro.inspection_cursor_kind();
                        if self.cursor_kind != kind {
                            let window = &self.presenter.as_ref().unwrap().window;
                            if let Some(kind) = kind {
                                window.set_cursor(self.cursors.as_ref().unwrap()[kind].clone());
                            } else {
                                window.set_cursor(winit::window::CursorIcon::Default);
                            }
                            self.cursor_kind = kind;
                        }
                        intro.inspection_cursor_presented();
                        return;
                    }
                }
                let elapsed = self.since.elapsed().as_secs_f64();
                let movie_frames = self.assets.movie.len() / (320 * 240 * 4);
                let duration = [movie_frames as f64 / 25., 3., 6., f64::INFINITY][self.stage];
                if elapsed >= duration {
                    self.advance();
                }
                let (width, height, pixels) = if self.stage == 0 {
                    let frame =
                        ((self.since.elapsed().as_secs_f64() * 25.) as usize).min(movie_frames - 1);
                    (
                        320,
                        240,
                        &self.assets.movie[frame * 320 * 240 * 4..(frame + 1) * 320 * 240 * 4],
                    )
                } else {
                    let image = [
                        &self.assets.deepred,
                        &self.assets.warning,
                        &self.assets.loading,
                    ][self.stage - 1];
                    (image.width, image.height, image.bytes.as_slice())
                };
                if let Some(p) = &mut self.presenter
                    && let Err(e) = p.draw(width, height, pixels)
                {
                    self.failure = Some(e.to_string());
                    event_loop.exit();
                }
            }
            _ => (),
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Service pending UI-only prefixes even if no surface redraw is
        // available (for example, while minimized). Never wait on a worker.
        if let Some(intro) = &mut self.intro {
            if let Err(error) = intro.service_focus() {
                self.failure = Some(error);
                event_loop.exit();
                return;
            }
        }
        if self
            .exit_after
            .is_some_and(|limit| self.launched.elapsed() >= limit)
        {
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        if self.intro.is_some() {
            // Fifo surface acquisition supplies display pacing. A second
            // fixed 60Hz wait can miss refresh intervals on other monitors.
            if let Some(p) = &self.presenter {
                p.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::Poll);
            return;
        }
        if now >= self.next_redraw {
            if let Some(p) = &self.presenter {
                p.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(now + FRAME_INTERVAL));
        } else {
            // Input events wake WaitUntil. They must not postpone the existing
            // frame deadline or request an unbounded stream of extra frames.
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_redraw));
        }
    }
}
pub fn run() -> Result<(), Error> {
    let mut root = PathBuf::from(
        "Original Distribution/Monopoly Tycoon ISO 2001 Archive.org/Extracted Installer",
    );
    let local_ffmpeg = PathBuf::from(
        "work/python/Lib/site-packages/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe",
    );
    let mut ffmpeg = std::env::var_os("OPENMT2001_FFMPEG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) && local_ffmpeg.exists() {
                local_ffmpeg
            } else {
                PathBuf::from("ffmpeg")
            }
        });
    let mut scene_root = PathBuf::from(
        "Original Distribution/Monopoly Tycoon ISO 2001 Archive.org/Extracted Disc/Monopoly Tycoon/max",
    );
    let mut executable = PathBuf::from("work/patch14/payload/Essential/mc.exe");
    let mut intro_map = PathBuf::from("work/patch14/payload/Essential/scripts/MAPS/intromap.lua");
    let mut quality = String::from("low");
    let mut save_root = None;
    let mut profile_root = crate::profile::default_directory();
    let mut language = PathBuf::from("work/patch14/payload/US/language/usa");
    let mut compatibility = crate::compatibility::Mode::default();
    let mut skip_splash = false;
    let mut menu_onload = false;
    let mut exit_after = None;
    let mut source_options=String::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scene-root" => {
                scene_root = args
                    .next()
                    .ok_or("--scene-root requires original max directory")?
                    .into()
            }
            "--executable" => {
                executable = args
                    .next()
                    .ok_or("--executable requires original v1.4 mc.exe")?
                    .into()
            }
            "--intro-map" => {
                intro_map = args
                    .next()
                    .ok_or("--intro-map requires original intromap.lua")?
                    .into()
            }
            "--intro-quality" => {
                quality = args
                    .next()
                    .ok_or("--intro-quality requires low, medium or high")?
            }
            "--save-dir" => {
                save_root = Some(PathBuf::from(
                    args.next().ok_or("--save-dir requires a directory")?,
                ));
            }
            "--profile-dir" => {
                profile_root = args
                    .next()
                    .ok_or("--profile-dir requires V2 user data directory")?
                    .into()
            }
            "--language" => {
                language = args
                    .next()
                    .ok_or("--language requires patched language directory")?
                    .into()
            }
            "--strict-v14" => compatibility = crate::compatibility::Mode::StrictV14,
            "--skip-splash" => skip_splash = true,
            "--menu" => {
                skip_splash = true;
                menu_onload = true;
            }
            "--source-options"=>{source_options=args.next().ok_or("--source-options requires the native option string")?;}
            "--assets" => {
                root = args
                    .next()
                    .ok_or("--assets requires extracted installer directory")?
                    .into()
            }
            "--ffmpeg" => {
                ffmpeg = args
                    .next()
                    .ok_or("--ffmpeg requires executable path")?
                    .into()
            }
            "--exit-after" => {
                exit_after = Some(Duration::try_from_secs_f64(
                    args.next()
                        .ok_or("--exit-after requires seconds")?
                        .parse()?,
                )?)
            }
            "--help" => {
                println!(
                    "OpenMT2001 native frontend milestone\n--strict-v14 (disable documented blocker fixes; default uses modern compatibility)\n--source-options <native options> (DEBUGSTUFF / TESTMODE source parser)\n--assets <extracted-installer-directory>\n--ffmpeg <decoder-executable> (or OPENMT2001_FFMPEG)\n--exit-after <seconds> (smoke check)\nEscape/Space/Enter skips the publisher movie. Escape closes during loading; Escape/Space/Enter opens the menu during the intro.\n--scene-root <original-max-directory>\n--executable <v1.4-mc.exe>\n--intro-map <intromap.lua>\n--intro-quality <low|medium|high> (default low; original performance selection is not connected)\n--skip-splash (intro smoke check)\n--menu (direct menu smoke check)\n--language <patched-language-directory>\n--save-dir <directory-containing-original-saves> (default V2-profile-directory/saves)\n--profile-dir <V2-profile-directory> (default per-user application data)\nNAME: creates/selects V2 player profiles; names currently support 1-31 ASCII characters. Backspace edits, Ctrl/Cmd+A selects all, Ctrl/Cmd+V pastes.\nMenu: mouse or arrows/Tab select; Enter activates; Escape returns from pending screens. Quit: physical main Return confirms Yes; Backspace selects No. LOAD GAME restores original saves into a live retained city and its saved view; arrows pan, PgUp/PgDn change height, Escape returns. Scroll over scenario information to read more. SINGLE PLAYER launches supported unlocked campaign scenarios, Bonus, and Sandbox with difficulty/character setup; campaign results update V2 progression. Win/Lose/Complete result hubs use the retained city and original end callback; credits text uses original records. Winner/loser music uses original source assets. Water and clouds advance with the live logic clock."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument {arg}").into()),
        }
    }
    if !matches!(quality.as_str(), "low" | "medium" | "high") {
        return Err("intro quality must be low, medium or high".into());
    }
    let assets = Assets::load(&root, &ffmpeg, skip_splash)?;
    let music_path = root.join("Standard/sound/music/music_intro.wma");
    let music_decoder = ffmpeg.clone();
    let intro_music_loading = load_intro_music(music_decoder.clone(), music_path.clone());
    let (intro_sender, intro_loading) = std::sync::mpsc::channel();
    let paths = crate::intro::Paths {
        compatibility,
        installer: root,
        ffmpeg,
        scene: scene_root,
        executable,
        map: intro_map,
        quality,
        language,
        save_root: save_root.unwrap_or_else(|| profile_root.join("saves")),
        profile_root,
    };
    std::thread::spawn(move || {
        let _ = intro_sender.send(crate::intro::Assets::load_with_source_options(paths,&source_options));
    });
    let now = Instant::now();
    let mut app = App {
        intro_loading,
        intro: None,
        cursors: None,
        cursor_kind: None,
        menu_onload,
        modifiers: ModifiersState::default(),
        assets,
        presenter: None,
        stage: if skip_splash { 3 } else { 0 },
        since: now,
        audio_bus: crate::audio_bus::Bus::new(),
        sink: None,
        ui_click_sinks: Vec::new(),
        intro_music_loading: Some(intro_music_loading),
        intro_music_playing: false,
        intro_music_attempted: true,
        intro_music_path: music_path,
        intro_music_decoder: music_decoder,
        failure: None,
        exit_after,
        launched: now,
        next_redraw: now,
        desktop_keys: Default::default(),
        participant_left: Default::default(),
        window_focus: None,
        frame_profile: std::env::var_os("OPENMT2001_PROFILE_PACE").map(|_| FrameProfile {
            started: now,
            frames: 0,
            draw_time: Duration::ZERO,
            slowest: Duration::ZERO,
        }),
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}


#[cfg(test)]
mod participant_physical_input_tests {
    use super::*;
    fn left(down: bool) -> WindowEvent {
        WindowEvent::MouseInput { device_id: winit::event::DeviceId::dummy(),
            button: MouseButton::Left,
            state: if down { ElementState::Pressed } else { ElementState::Released } }
    }
    #[test]
    fn event_local_left_edges_survive_filtered_delivery_and_focus_loss() {
        let mut sample = ParticipantLeft::default();
        assert!(sample.sample(&left(true))); // event could be swallowed by inspection/pending
        assert!(!sample.sample(&WindowEvent::RedrawRequested));
        assert!(!sample.sample(&WindowEvent::Focused(true)));
        assert!(!sample.sample(&left(true)), "no repeat edge after nonmouse events");
        assert!(!sample.sample(&left(false))); // release need not reach Menu
        assert!(sample.sample(&left(true)), "physical release restores next edge");
        assert!(!sample.sample(&WindowEvent::Focused(false)));
        assert!(sample.sample(&left(true)), "missing OS release after focus loss is harmless");
        assert!(!sample.sample(&left(false)));
        assert!(!sample.sample(&left(false)));
    }
}
