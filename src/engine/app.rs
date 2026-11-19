//! winit event loop, input handling and scene switching.

use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use super::gpu::GpuState;
use super::scene::Scene;

pub struct SceneEntry {
    pub id: &'static str,
    pub build: fn() -> Box<dyn Scene>,
}

/// One offscreen screenshot request: `--shot preset:time:seconds:path.png`.
pub struct Shot {
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

struct App {
    registry: &'static [SceneEntry],
    opts: RunOptions,
    scene_idx: usize,
    state: Option<GpuState>,
}

impl App {
    fn switch_scene(&mut self, delta: isize) {
        let n = self.registry.len() as isize;
        self.scene_idx = (self.scene_idx as isize + delta).rem_euclid(n) as usize;
        let entry = &self.registry[self.scene_idx];
        if let Some(s) = self.state.as_mut() {
            s.window.set_title(&format!("Generating {}...", entry.id));
            s.load_scene((entry.build)());
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let entry = &self.registry[self.scene_idx];
        let attrs = Window::default_attributes()
            .with_title(format!("Voxel Dioramas — Generating {}...", entry.id))
            .with_inner_size(winit::dpi::LogicalSize::new(1440.0, 900.0));
        let win = Arc::new(event_loop.create_window(attrs).unwrap());
        let t0 = std::time::Instant::now();
        let scene = (entry.build)();
        println!("Generated {} in {:.2}s", entry.id, t0.elapsed().as_secs_f32());
        let mut state = pollster::block_on(GpuState::new(win, scene));

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
            return;
        }
        if !self.opts.shots.is_empty() {
            let (w, h) = self.opts.shot_size;
            for shot in &self.opts.shots {
                state.set_preset(shot.preset);
                state.time_mode = shot.time_mode % 3;
                state.capture(shot.t, w, h, &shot.path);
            }
            event_loop.exit();
            return;
        }
        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key: PhysicalKey::Code(code), state: ElementState::Pressed, .. },
                ..
            } => match code {
                KeyCode::Tab => self.switch_scene(1),
                KeyCode::Backquote => self.switch_scene(-1),
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
                KeyCode::Escape => event_loop.exit(),
                _ => {}
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(s) = &self.state {
            s.window.request_redraw();
        }
    }
}

pub fn parse_args(registry: &[SceneEntry]) -> Result<RunOptions, String> {
    let mut opts = RunOptions { scene_idx: 0, shots: Vec::new(), shot_size: (1600, 900), bench: false };
    let mut args = std::env::args().skip(1);
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
                opts.shots.push(Shot { preset: preset.saturating_sub(1), time_mode, t, path });
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
    let mut app = App { registry, scene_idx: opts.scene_idx, opts, state: None };
    event_loop.run_app(&mut app).unwrap();
}
