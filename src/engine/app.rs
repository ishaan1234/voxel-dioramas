//! winit event loop: input routing (egui → experience → default camera),
//! the per-frame experience/UI cycle, and scene (re)generation.
//! Works on desktop and in the browser (WebGPU).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use super::experience::{Experience, ExperienceEntry, Requests};
use super::gpu::GpuState;
use super::platform;
use super::scene::{Scene, WorldParams};
use super::storage::Storage;
use super::ui::{self, EguiLayer, ShellAction, ShellView};

pub struct SceneEntry {
    pub id: &'static str,
    /// Display name for menus.
    pub title: &'static str,
    pub build: fn(&WorldParams) -> Box<dyn Scene>,
}

/// One offscreen screenshot request: `--shot preset:time:seconds:path.png`.
pub struct Shot {
    /// Explicit (eye, target) overriding the preset, from a preceding `--view`.
    pub view: Option<(glam::Vec3, glam::Vec3)>,
    pub preset: usize,
    pub time_mode: usize,
    pub t: f32,
    pub path: String,
}

pub struct RunOptions {
    pub scene_idx: usize,
    pub params: WorldParams,
    pub experience: Option<String>,
    pub shots: Vec<Shot>,
    pub shot_size: (u32, u32),
    /// `--bench`: time every camera preset offscreen and exit.
    pub bench: bool,
    /// Screenshot overrides: weather and continuous hour.
    pub weather: Option<super::atmosphere::Weather>,
    pub hour: Option<f32>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self { scene_idx: 0, params: WorldParams::default(), experience: None, shots: Vec::new(), shot_size: (1600, 900), bench: false, weather: None, hour: None }
    }
}

/// Actions that can come from the keyboard or from the web page.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    NextScene,
    PrevScene,
    Time,
    Tour,
    Pause,
    Preset(usize),
}

impl Command {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "next_scene" => Self::NextScene,
            "prev_scene" => Self::PrevScene,
            "time" => Self::Time,
            "tour" => Self::Tour,
            "pause" => Self::Pause,
            _ => Self::Preset(s.strip_prefix("preset:")?.parse::<usize>().ok()?.checked_sub(1)?),
        })
    }
}

thread_local! {
    static COMMANDS: RefCell<Vec<Command>> = const { RefCell::new(Vec::new()) };
}

/// Queue a command; it is applied on the next frame.
pub fn push_command(c: Command) {
    COMMANDS.with(|q| q.borrow_mut().push(c));
}

/// Active touch points and the baseline of a two-finger gesture.
#[derive(Default)]
struct Touches {
    points: Vec<(u64, (f64, f64))>,
    pinch: Option<(f64, (f64, f64))>,
}

impl Touches {
    fn two_finger(&self) -> Option<(f64, (f64, f64))> {
        if self.points.len() != 2 {
            return None;
        }
        let (a, b) = (self.points[0].1, self.points[1].1);
        let dist = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt().max(1.0);
        Some((dist, ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)))
    }
}

type PendingScene = Rc<RefCell<Option<(usize, WorldParams, Box<dyn Scene>)>>>;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct App {
    registry: &'static [SceneEntry],
    experiences: &'static [ExperienceEntry],
    opts: RunOptions,
    scene_idx: usize,
    params: WorldParams,
    window: Option<Arc<Window>>,
    state: Option<GpuState>,
    /// Filled asynchronously (browser GPU init / deferred scene builds).
    pending_state: Rc<RefCell<Option<GpuState>>>,
    pending_scene: PendingScene,
    loading: bool,
    touches: Touches,
    ui: Option<EguiLayer>,
    shell_expanded: bool,
    exp_idx: usize,
    exp: Box<dyn Experience>,
    exp_started: bool,
    storage: Storage,
    requests: Requests,
    toast: Option<(String, f32)>,
}

impl App {
    /// Generate scene `idx` with `params`. In the browser the build is
    /// deferred until the loading message has painted.
    fn request_scene(&mut self, idx: usize, params: WorldParams) {
        if self.loading {
            return;
        }
        let entry = &self.registry[idx];
        platform::status(&format!("Generating {}...", entry.title));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let t0 = web_time::Instant::now();
            let scene = (entry.build)(&params);
            println!("Generated {} (seed {}, {}) in {:.2}s", entry.id, params.seed, params.season.name(), t0.elapsed().as_secs_f32());
            *self.pending_scene.borrow_mut() = Some((idx, params, scene));
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.loading = true;
            let slot = self.pending_scene.clone();
            let build = entry.build;
            let window = self.window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                platform::web::yield_to_browser().await;
                *slot.borrow_mut() = Some((idx, params, build(&params)));
                if let Some(w) = window {
                    w.request_redraw();
                }
            });
        }
    }

    fn switch_scene(&mut self, delta: isize) {
        let n = self.registry.len() as isize;
        let idx = (self.scene_idx as isize + delta).rem_euclid(n) as usize;
        self.request_scene(idx, self.params);
    }

    fn scene_index(&self, id: &str) -> Option<usize> {
        self.registry.iter().position(|e| e.id == id)
    }

    fn apply(&mut self, c: Command) {
        match c {
            Command::NextScene => self.switch_scene(1),
            Command::PrevScene => self.switch_scene(-1),
            _ => {
                let Some(s) = self.state.as_mut() else { return };
                match c {
                    Command::Time => s.cycle_time(),
                    Command::Tour => s.cam.toggle_tour(),
                    Command::Pause => s.paused = !s.paused,
                    Command::Preset(i) => s.fly_to_preset(i),
                    _ => {}
                }
            }
        }
    }

    /// Run `f` with an experience context for the current frame.
    fn with_ctx<R>(&mut self, dt: f32, f: impl FnOnce(&mut dyn Experience, &mut super::experience::Ctx) -> R) -> Option<R> {
        let s = self.state.as_mut()?;
        let id = self.registry[self.scene_idx].id;
        let mut ctx = s.ctx(id, self.params, &mut self.storage, &mut self.requests, dt);
        Some(f(self.exp.as_mut(), &mut ctx))
    }

    fn switch_experience(&mut self, idx: usize) {
        if idx == self.exp_idx && self.exp_started {
            return;
        }
        self.with_ctx(0.0, |e, c| e.exit(c));
        self.exp_idx = idx;
        self.exp = (self.experiences[idx].make)();
        self.with_ctx(0.0, |e, c| {
            e.enter(c);
            e.scene_loaded(c);
        });
        self.exp_started = true;
    }

    /// Adopt anything that finished asynchronously, then run queued commands.
    fn pump(&mut self) {
        if self.state.is_none() {
            if let Some(st) = self.pending_state.borrow_mut().take() {
                self.ui = Some(st.make_ui());
                self.state = Some(st);
                self.loading = false;
                platform::status("");
            }
        }
        if self.state.is_some() && !self.exp_started {
            self.exp_started = true;
            self.with_ctx(0.0, |e, c| {
                e.enter(c);
                e.scene_loaded(c);
            });
        }
        let loaded = self.pending_scene.borrow_mut().take();
        if let (Some((idx, params, scene)), Some(s)) = (loaded, self.state.as_mut()) {
            s.load_scene(scene);
            self.scene_idx = idx;
            self.params = params;
            self.loading = false;
            platform::status("");
            self.with_ctx(0.0, |e, c| e.scene_loaded(c));
        }
        let cmds: Vec<Command> = COMMANDS.with(|q| q.borrow_mut().drain(..).collect());
        for c in cmds {
            self.apply(c);
        }
    }

    /// One frame: UI + experience logic + render.
    fn frame(&mut self) {
        self.pump();
        let (Some(s), Some(ui)) = (self.state.as_mut(), self.ui.as_mut()) else { return };
        let dt = s.begin_frame();
        ui.begin(&s.window);
        let egui_ctx = ui.ctx.clone();

        // Global menu bar.
        let mut actions = Vec::new();
        if !self.exp.immersive() || self.shell_expanded {
            let presets = s.scene().data().presets.iter().map(|p| p.name).collect();
            let mut view = ShellView {
                scenes: self.registry,
                scene_idx: self.scene_idx,
                experiences: self.experiences,
                exp_idx: self.exp_idx,
                params: self.params,
                atmo: &mut s.atmo,
                presets,
                tour: s.cam.tour,
                paused: s.paused,
                fps: s.fps,
                loading: self.loading,
            };
            actions = ui::shell_ui(&egui_ctx, &mut view, &mut self.shell_expanded);
        } else {
            egui::Area::new(egui::Id::unique("menu_reveal")).anchor(egui::Align2::LEFT_TOP, [8.0, 8.0]).show(&egui_ctx, |ui| {
                if ui.button("Menu").clicked() {
                    self.shell_expanded = true;
                }
            });
        }

        // Experience UI and logic.
        self.with_ctx(dt, |e, c| {
            e.ui(&egui_ctx, c);
            e.update(c);
        });

        // Toasts.
        if let Some(msg) = self.requests.toast.take() {
            self.toast = Some((msg, 3.0));
        }
        if let Some((msg, left)) = &mut self.toast {
            *left -= dt;
            egui::Area::new(egui::Id::unique("toast")).anchor(egui::Align2::CENTER_TOP, [0.0, 70.0]).show(&egui_ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| ui.label(msg.as_str()));
            });
            if *left <= 0.0 {
                self.toast = None;
            }
        }

        let (Some(s), Some(ui)) = (self.state.as_mut(), self.ui.as_mut()) else { return };
        s.render(dt, Some(ui));

        // Apply menu actions and experience requests.
        for a in actions {
            match a {
                ShellAction::LoadScene(i, p) => self.request_scene(i, p),
                ShellAction::Experience(i) => self.switch_experience(i),
                ShellAction::Preset(i) => {
                    if let Some(s) = self.state.as_mut() {
                        s.fly_to_preset(i)
                    }
                }
                ShellAction::ToggleTour => {
                    if let Some(s) = self.state.as_mut() {
                        s.cam.toggle_tour()
                    }
                }
                ShellAction::TogglePause => {
                    if let Some(s) = self.state.as_mut() {
                        s.paused = !s.paused
                    }
                }
            }
        }
        if let Some((id, params)) = self.requests.load_scene.take() {
            if let Some(i) = self.scene_index(&id) {
                self.request_scene(i, params);
            }
        }
        if let Some(id) = self.requests.switch_experience.take() {
            if let Some(i) = self.experiences.iter().position(|e| e.id == id) {
                self.switch_experience(i);
            }
        }
    }

    fn on_touch(&mut self, t: Touch) {
        let Some(s) = self.state.as_mut() else { return };
        let pos = (t.location.x, t.location.y);
        let tc = &mut self.touches;
        match t.phase {
            TouchPhase::Started => {
                tc.points.retain(|(id, _)| *id != t.id);
                tc.points.push((t.id, pos));
                tc.pinch = tc.two_finger();
            }
            TouchPhase::Moved => {
                let Some(i) = tc.points.iter().position(|(id, _)| *id == t.id) else { return };
                let prev = tc.points[i].1;
                tc.points[i].1 = pos;
                match tc.points.len() {
                    // One finger orbits.
                    1 => s.cam.orbit(((pos.0 - prev.0) * 0.6) as f32, ((pos.1 - prev.1) * 0.6) as f32),
                    // Two fingers: pinch zooms, moving the midpoint pans.
                    2 => {
                        if let (Some((d0, m0)), Some((d1, m1))) = (tc.pinch, tc.two_finger()) {
                            let ratio = (d1 / d0) as f32;
                            s.cam.zoom((1.0 - 1.0 / ratio) / 0.12);
                            s.cam.pan((m1.0 - m0.0) as f32, (m1.1 - m0.1) as f32);
                        }
                        tc.pinch = tc.two_finger();
                    }
                    _ => {}
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                tc.points.retain(|(id, _)| *id != t.id);
                tc.pinch = tc.two_finger();
            }
        }
    }

    /// Default camera / global keys (when the experience allows it).
    fn default_controls(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        if let WindowEvent::Touch(t) = event {
            self.on_touch(t);
            return;
        }
        if let WindowEvent::KeyboardInput {
            event: KeyEvent { physical_key: PhysicalKey::Code(code), state: ElementState::Pressed, repeat: false, .. },
            ..
        } = event
        {
            match code {
                KeyCode::Tab => return self.switch_scene(1),
                KeyCode::Backquote => return self.switch_scene(-1),
                _ => {}
            }
        }
        let Some(s) = self.state.as_mut() else { return };
        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => s.left_drag = pressed,
                    MouseButton::Right => s.right_drag = pressed,
                    _ => {}
                }
                if !pressed {
                    s.last_cursor = None;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((lx, ly)) = s.last_cursor {
                    let (dx, dy) = ((position.x - lx) as f32, (position.y - ly) as f32);
                    if s.left_drag {
                        s.cam.orbit(dx, dy);
                    } else if s.right_drag {
                        s.cam.pan(dx, dy);
                    }
                }
                if s.left_drag || s.right_drag {
                    s.last_cursor = Some((position.x, position.y));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let sc = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.02,
                };
                s.cam.zoom(sc);
            }
            // Held movement keys (press and release).
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key: PhysicalKey::Code(code), state: ks, .. },
                ..
            } if is_move_key(code) => {
                let down = ks == ElementState::Pressed;
                let k = &mut s.cam.keys;
                match code {
                    KeyCode::KeyW => k.forward = down,
                    KeyCode::KeyS => k.back = down,
                    KeyCode::KeyA => k.left = down,
                    KeyCode::KeyD => k.right = down,
                    KeyCode::KeyE => k.up = down,
                    KeyCode::KeyQ => k.down = down,
                    KeyCode::ShiftLeft | KeyCode::ShiftRight => k.fast = down,
                    KeyCode::ArrowLeft => k.turn_left = down,
                    KeyCode::ArrowRight => k.turn_right = down,
                    KeyCode::ArrowUp => k.look_up = down,
                    KeyCode::ArrowDown => k.look_down = down,
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key: PhysicalKey::Code(code), state: ElementState::Pressed, .. },
                ..
            } => match code {
                KeyCode::KeyT | KeyCode::Space => s.cam.toggle_tour(),
                KeyCode::KeyN | KeyCode::KeyL => s.cycle_time(),
                KeyCode::KeyP => s.paused = !s.paused,
                KeyCode::KeyR | KeyCode::Digit1 => s.fly_to_preset(0),
                KeyCode::Digit2 => s.fly_to_preset(1),
                KeyCode::Digit3 => s.fly_to_preset(2),
                KeyCode::Digit4 => s.fly_to_preset(3),
                KeyCode::Digit5 => s.fly_to_preset(4),
                KeyCode::Digit6 => s.fly_to_preset(5),
                KeyCode::Digit7 => s.fly_to_preset(6),
                KeyCode::Digit8 => s.fly_to_preset(7),
                KeyCode::Digit9 => s.fly_to_preset(8),
                #[cfg(not(target_arch = "wasm32"))]
                KeyCode::Escape => event_loop.exit(),
                _ => {}
            },
            _ => {}
        }
        let _ = event_loop;
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn run_offscreen_jobs(&mut self, state: &mut GpuState, event_loop: &ActiveEventLoop) -> bool {
        if self.opts.bench {
            let (w, h) = self.opts.shot_size;
            let n = state.preset_count();
            let mut total = 0.0;
            println!("Benchmark {w}x{h}, 200 frames per view (no vsync):");
            for p in 0..n {
                state.set_preset(p);
                let (mean, p95) = state.bench(200, w, h);
                total += mean;
                println!("  {:<20} mean {mean:6.2} ms  p95 {p95:6.2} ms  ({:.0} fps)", state.cam.preset_name, 1000.0 / mean);
            }
            println!("  average over views: {:.2} ms ({:.0} fps)", total / n as f32, 1000.0 * n as f32 / total);
            event_loop.exit();
            return true;
        }
        if !self.opts.shots.is_empty() {
            let (w, h) = self.opts.shot_size;
            for shot in &self.opts.shots {
                state.set_preset(shot.preset);
                if let Some((eye, target)) = shot.view {
                    state.cam.set_view(eye, target);
                }
                state.atmo.clock = match self.opts.hour {
                    Some(hour) => super::atmosphere::DayClock::Hours { hour, speed: 0.0, real_time: false },
                    None => super::atmosphere::DayClock::Preset(shot.time_mode % 3),
                };
                if let Some(w) = self.opts.weather {
                    state.atmo.weather = w;
                    state.atmo.weather_mix = 1.0;
                }
                state.capture(shot.t, w, h, &shot.path);
            }
            event_loop.exit();
            return true;
        }
        false
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let entry = &self.registry[self.scene_idx];
        let attrs = Window::default_attributes().with_title(format!("Voxel Dioramas — {}", entry.title));
        #[cfg(not(target_arch = "wasm32"))]
        let attrs = attrs.with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs.with_canvas(Some(platform::web::canvas())).with_prevent_default(true)
        };
        let win = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(win.clone());

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.request_scene(self.scene_idx, self.params);
            let (_, _, scene) = self.pending_scene.borrow_mut().take().unwrap();
            let mut state = pollster::block_on(GpuState::new(win, scene));
            if self.run_offscreen_jobs(&mut state, event_loop) {
                return;
            }
            *self.pending_state.borrow_mut() = Some(state);
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.loading = true;
            let slot = self.pending_state.clone();
            let build = entry.build;
            let title = entry.title;
            let params = self.params;
            wasm_bindgen_futures::spawn_local(async move {
                platform::status(&format!("Generating {title}..."));
                platform::web::yield_to_browser().await;
                let scene = build(&params);
                platform::status("Starting the GPU...");
                platform::web::yield_to_browser().await;
                let state = GpuState::new(win.clone(), scene).await;
                *slot.borrow_mut() = Some(state);
                win.request_redraw();
            });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::RedrawRequested => return self.frame(),
            WindowEvent::CloseRequested => return event_loop.exit(),
            WindowEvent::Resized(sz) => {
                if let Some(s) = self.state.as_mut() {
                    s.resize(sz.width, sz.height);
                }
            }
            WindowEvent::Focused(false) => {
                if let Some(s) = self.state.as_mut() {
                    s.cam.keys = Default::default();
                }
            }
            _ => {}
        }
        // 1. The UI gets first pick.
        if let (Some(ui), Some(w)) = (self.ui.as_mut(), self.window.as_ref()) {
            let consumed = ui.on_event(w, &event);
            let pointer_on_ui = ui.wants_pointer()
                && matches!(event, WindowEvent::MouseInput { state: ElementState::Pressed, .. } | WindowEvent::MouseWheel { .. })
                || matches!(&event, WindowEvent::Touch(t) if t.phase == TouchPhase::Started && ui.wants_pointer());
            let typing = ui.wants_keyboard() && matches!(event, WindowEvent::KeyboardInput { .. });
            if consumed || pointer_on_ui || typing {
                return;
            }
        }
        // 2. Then the experience.
        if self.state.is_some() && self.with_ctx(0.0, |e, c| e.window_event(&event, c)).unwrap_or(false) {
            return;
        }
        // 3. Then the default camera controls.
        if self.exp.free_camera() {
            self.default_controls(event_loop, event);
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn is_move_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::KeyW
            | KeyCode::KeyA
            | KeyCode::KeyS
            | KeyCode::KeyD
            | KeyCode::KeyQ
            | KeyCode::KeyE
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ArrowLeft
            | KeyCode::ArrowRight
            | KeyCode::ArrowUp
            | KeyCode::ArrowDown
    )
}

pub fn parse_args(registry: &[SceneEntry]) -> Result<RunOptions, String> {
    let mut opts = RunOptions::default();
    let mut args = std::env::args().skip(1);
    let mut view: Option<(glam::Vec3, glam::Vec3)> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--shot" => {
                let spec = args.next().ok_or("--shot needs preset:time:seconds:path")?;
                let mut parts = spec.splitn(4, ':');
                let mut num = |name: &str| -> Result<f32, String> {
                    parts
                        .next()
                        .and_then(|p| p.parse::<f32>().ok())
                        .ok_or(format!("bad {name} in --shot {spec}"))
                };
                let preset = num("preset")? as usize;
                let time_mode = num("time")? as usize;
                let t = num("seconds")?;
                let path = parts.next().ok_or("missing path in --shot")?.to_string();
                opts.shots.push(Shot { view, preset: preset.saturating_sub(1), time_mode, t, path });
            }
            "--view" => {
                // --view ex,ey,ez,tx,ty,tz : camera for the following --shot(s)
                let v = args.next().ok_or("--view needs ex,ey,ez,tx,ty,tz")?;
                let n: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if n.len() != 6 {
                    return Err(format!("bad --view {v}"));
                }
                view = Some((glam::Vec3::new(n[0], n[1], n[2]), glam::Vec3::new(n[3], n[4], n[5])));
            }
            "--size" => {
                let v = args.next().ok_or("--size needs WxH")?;
                let (w, h) = v.split_once('x').ok_or("--size needs WxH")?;
                opts.shot_size = (w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?);
            }
            "--seed" => {
                opts.params.seed = args.next().ok_or("--seed needs a number")?.parse().map_err(|_| "bad seed")?;
            }
            "--season" => {
                let v = args.next().ok_or("--season needs spring|summer|autumn|winter")?;
                opts.params.season = super::scene::Season::ALL
                    .into_iter()
                    .find(|s| s.name().eq_ignore_ascii_case(&v))
                    .ok_or(format!("unknown season {v}"))?;
            }
            "--weather" => {
                let v = args.next().ok_or("--weather needs a name")?;
                opts.weather = Some(
                    super::atmosphere::Weather::ALL.into_iter().find(|w| w.name().eq_ignore_ascii_case(&v)).ok_or(format!("unknown weather {v}"))?,
                );
            }
            "--hour" => opts.hour = Some(args.next().ok_or("--hour needs 0-24")?.parse().map_err(|_| "bad hour")?),
            "--mode" => opts.experience = Some(args.next().ok_or("--mode needs an experience id")?),
            "--bench" => opts.bench = true,
            "--list" => {
                for e in registry {
                    println!("{}", e.id);
                }
                std::process::exit(0);
            }
            name => {
                opts.scene_idx = registry
                    .iter()
                    .position(|e| e.id == name)
                    .ok_or_else(|| {
                        let ids: Vec<_> = registry.iter().map(|e| e.id).collect();
                        format!("unknown scene '{name}'. Available: {}", ids.join(", "))
                    })?;
            }
        }
    }
    Ok(opts)
}

pub fn run(registry: &'static [SceneEntry], experiences: &'static [ExperienceEntry], opts: RunOptions) {
    let event_loop = EventLoop::new().unwrap();
    let exp_idx = opts.experience.as_deref().and_then(|id| experiences.iter().position(|e| e.id == id)).unwrap_or(0);
    let app = App {
        registry,
        experiences,
        scene_idx: opts.scene_idx,
        params: opts.params,
        opts,
        window: None,
        state: None,
        pending_state: Rc::new(RefCell::new(None)),
        pending_scene: Rc::new(RefCell::new(None)),
        loading: false,
        touches: Touches::default(),
        ui: None,
        shell_expanded: false,
        exp_idx,
        exp: (experiences[exp_idx].make)(),
        exp_started: false,
        storage: Storage::new(),
        requests: Requests::default(),
        toast: None,
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).unwrap();
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
}
