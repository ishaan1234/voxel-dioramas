//! winit event loop, input (mouse, keyboard, touch), commands from the web UI,
//! and scene switching. Works on desktop and in the browser (WebGPU).

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

use super::gpu::GpuState;
use super::platform;
use super::scene::Scene;

pub struct SceneEntry {
    pub id: &'static str,
    pub build: fn() -> Box<dyn Scene>,
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
    pub shots: Vec<Shot>,
    pub shot_size: (u32, u32),
    /// `--bench`: time every camera preset offscreen and exit.
    pub bench: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self { scene_idx: 0, shots: Vec::new(), shot_size: (1600, 900), bench: false }
    }
}

/// Actions that can come from the keyboard or from the web page's buttons.
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

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct App {
    registry: &'static [SceneEntry],
    opts: RunOptions,
    scene_idx: usize,
    window: Option<Arc<Window>>,
    state: Option<GpuState>,
    /// Filled asynchronously (browser GPU init / deferred scene builds).
    pending_state: Rc<RefCell<Option<GpuState>>>,
    pending_scene: Rc<RefCell<Option<Box<dyn Scene>>>>,
    loading: bool,
    touches: Touches,
}

impl App {
    /// Build scene `idx` and hand it to the renderer. In the browser the build
    /// is deferred until the loading message has painted.
    fn request_scene(&mut self, idx: usize) {
        if self.loading {
            return;
        }
        self.scene_idx = idx;
        let entry = &self.registry[idx];
        platform::status(&format!("Generating {}...", entry.id));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let t0 = web_time::Instant::now();
            let scene = (entry.build)();
            println!("Generated {} in {:.2}s", entry.id, t0.elapsed().as_secs_f32());
            *self.pending_scene.borrow_mut() = Some(scene);
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.loading = true;
            let slot = self.pending_scene.clone();
            let build = entry.build;
            let window = self.window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                platform::web::yield_to_browser().await;
                *slot.borrow_mut() = Some(build());
                if let Some(w) = window {
                    w.request_redraw();
                }
            });
        }
    }

    fn switch_scene(&mut self, delta: isize) {
        let n = self.registry.len() as isize;
        let idx = (self.scene_idx as isize + delta).rem_euclid(n) as usize;
        self.request_scene(idx);
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

    /// Adopt anything that finished asynchronously, then run queued commands.
    fn pump(&mut self) {
        if self.state.is_none() {
            if let Some(st) = self.pending_state.borrow_mut().take() {
                self.state = Some(st);
                self.loading = false;
                platform::status("");
            }
        }
        if let Some(s) = self.state.as_mut() {
            if let Some(scene) = self.pending_scene.borrow_mut().take() {
                s.load_scene(scene);
                self.loading = false;
                platform::status("");
            }
        }
        let cmds: Vec<Command> = COMMANDS.with(|q| q.borrow_mut().drain(..).collect());
        for c in cmds {
            self.apply(c);
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
                state.time_mode = shot.time_mode % 3;
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
        let attrs = Window::default_attributes().with_title(format!("Voxel Dioramas — {}", entry.id));
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
            self.request_scene(self.scene_idx);
            let scene = self.pending_scene.borrow_mut().take().unwrap();
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
            let id = entry.id;
            wasm_bindgen_futures::spawn_local(async move {
                platform::status(&format!("Generating {id}..."));
                platform::web::yield_to_browser().await;
                let scene = build();
                platform::status("Starting the GPU...");
                platform::web::yield_to_browser().await;
                let state = GpuState::new(win.clone(), scene).await;
                *slot.borrow_mut() = Some(state);
                win.request_redraw();
            });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let WindowEvent::RedrawRequested = event {
            self.pump();
        }
        if let WindowEvent::Touch(t) = event {
            self.on_touch(t);
            return;
        }
        // Keys that need &mut self (scene switching) are handled first.
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
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(sz) => s.resize(sz.width, sz.height),
            WindowEvent::RedrawRequested => s.render(),
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
            WindowEvent::Focused(false) => s.cam.keys = Default::default(),
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

pub fn run(registry: &'static [SceneEntry], opts: RunOptions) {
    let event_loop = EventLoop::new().unwrap();
    let app = App {
        registry,
        scene_idx: opts.scene_idx,
        opts,
        window: None,
        state: None,
        pending_state: Rc::new(RefCell::new(None)),
        pending_scene: Rc::new(RefCell::new(None)),
        loading: false,
        touches: Touches::default(),
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
