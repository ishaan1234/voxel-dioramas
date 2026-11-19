//! Experiences: self-contained modes on top of a scene (explore, games,
//! creative tools, ambient wallpaper...). Each lives in `src/experiences/`
//! and is listed in `experiences::REGISTRY`.
//!
//! Lifecycle: `enter` → (`scene_loaded` after every scene load) →
//! per frame: `window_event`* → `ui` → `update` → render → ... → `exit`.

use glam::{Mat4, Vec3, Vec4};
use winit::event::WindowEvent;

use super::atmosphere::Atmosphere;
use super::camera::OrbitCam;
use super::props::Props;
use super::scene::{raycast_surface, Scene, WorldParams};
use super::storage::Storage;

/// Things an experience asks the app to do (applied after the frame).
#[derive(Default)]
pub struct Requests {
    /// Load (or regenerate) a scene: (scene id, params).
    pub load_scene: Option<(String, WorldParams)>,
    /// Switch to another experience by id.
    pub switch_experience: Option<String>,
    /// Offscreen render request (postcards / clips); see `engine::capture`.
    pub capture: Option<CaptureRequest>,
    /// Short message shown briefly at the top of the screen.
    pub toast: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CaptureRequest {
    pub width: u32,
    pub height: u32,
    /// 1 = still image; >1 = animation frames.
    pub frames: u32,
    pub fps: f32,
    /// File name stem for the export.
    pub name: String,
}

/// Per-frame view of the world handed to experiences.
pub struct Ctx<'a> {
    pub scene: &'a dyn Scene,
    pub scene_id: &'a str,
    pub params: WorldParams,
    pub cam: &'a mut OrbitCam,
    pub atmo: &'a mut Atmosphere,
    pub props: &'a mut Props,
    pub storage: &'a mut Storage,
    pub requests: &'a mut Requests,
    pub paused: &'a mut bool,
    /// Simulation time (seconds) and frame delta.
    pub time: f32,
    pub dt: f32,
    /// Framebuffer size in physical pixels.
    pub screen: (u32, u32),
    /// Camera of the *previous* frame (good enough for picking).
    pub view_proj: Mat4,
    pub eye: Vec3,
}

impl Ctx<'_> {
    /// World-space ray through a pixel (physical coordinates, origin top-left).
    pub fn screen_ray(&self, px: f32, py: f32) -> (Vec3, Vec3) {
        let (w, h) = (self.screen.0.max(1) as f32, self.screen.1.max(1) as f32);
        let ndc_x = px / w * 2.0 - 1.0;
        let ndc_y = 1.0 - py / h * 2.0;
        let inv = self.view_proj.inverse();
        let far = inv * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let far = far.truncate() / far.w;
        (self.eye, (far - self.eye).normalize_or_zero())
    }

    /// Ground point under a pixel, via `Scene::surface_y`.
    pub fn pick_ground(&self, px: f32, py: f32) -> Option<Vec3> {
        let (o, d) = self.screen_ray(px, py);
        raycast_surface(self.scene, o, d, 1500.0)
    }

    /// Project a world point to pixels; None if behind the camera.
    pub fn world_to_screen(&self, p: Vec3) -> Option<(f32, f32)> {
        let c = self.view_proj * p.extend(1.0);
        if c.w <= 0.0 {
            return None;
        }
        let n = c.truncate() / c.w;
        Some(((n.x + 1.0) * 0.5 * self.screen.0 as f32, (1.0 - n.y) * 0.5 * self.screen.1 as f32))
    }

    /// Is a world point inside the view (with a margin in NDC units)?
    pub fn in_view(&self, p: Vec3, margin: f32) -> bool {
        let c = self.view_proj * p.extend(1.0);
        if c.w <= 0.0 {
            return false;
        }
        let n = c.truncate() / c.w;
        n.x.abs() <= 1.0 - margin && n.y.abs() <= 1.0 - margin && (0.0..=1.0).contains(&n.z)
    }
}

pub trait Experience {
    fn id(&self) -> &'static str;
    /// Called when the experience becomes active.
    fn enter(&mut self, _ctx: &mut Ctx) {}
    /// Called before switching away.
    fn exit(&mut self, _ctx: &mut Ctx) {}
    /// Called after every scene load (props were just cleared).
    fn scene_loaded(&mut self, _ctx: &mut Ctx) {}
    /// Per-frame logic (after input and UI).
    fn update(&mut self, _ctx: &mut Ctx) {}
    /// Draw panels / HUD with egui.
    fn ui(&mut self, _ui: &egui::Context, _ctx: &mut Ctx) {}
    /// Raw window events not consumed by egui. Return true to stop the default
    /// camera controls from also handling it.
    fn window_event(&mut self, _ev: &WindowEvent, _ctx: &mut Ctx) -> bool {
        false
    }
    /// Whether the default orbit / WASD / touch camera is active.
    fn free_camera(&self) -> bool {
        true
    }
    /// Hide the global menu bar (immersive modes); the user can still reveal it.
    fn immersive(&self) -> bool {
        false
    }
}

pub struct ExperienceEntry {
    pub id: &'static str,
    pub title: &'static str,
    /// One line for menus.
    pub blurb: &'static str,
    pub make: fn() -> Box<dyn Experience>,
}
