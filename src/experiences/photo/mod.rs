//! Photo Hunt: frame a scene's landmarks under the right light, weather, and distance.
//!
//! Controls: free camera, Shutter button or F to shoot, Album button or G to open the album.

mod logic;

use glam::Vec3;
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::engine::atmosphere::Weather;
use crate::engine::experience::{Ctx, Experience};
use crate::engine::storage::Storage;

use logic::{Album, LandmarkIn, Target, ViewSample, WeatherKind};

const FLASH_SECONDS: f32 = 0.42;
const CARD_SECONDS: f32 = 8.0;

pub fn make() -> Box<dyn Experience> {
    Box::new(PhotoHunt::default())
}

#[derive(Default)]
struct PhotoHunt {
    targets: Vec<Target>,
    album: Album,
    scene_id: String,
    album_open: bool,
    flash: f32,
    result: Option<ShotResult>,
}

#[derive(Clone)]
struct ShotResult {
    captures: Vec<CaptureLine>,
    misses: Vec<String>,
    age: f32,
}

#[derive(Clone)]
struct CaptureLine {
    title: String,
    stars: u8,
    score: f32,
    new_best: bool,
}

struct AlbumRow {
    title: String,
    found: bool,
    stars: u8,
    score: f32,
    hint: &'static str,
}

impl Experience for PhotoHunt {
    fn id(&self) -> &'static str {
        "photo"
    }

    fn free_camera(&self) -> bool {
        true
    }

    fn exit(&mut self, ctx: &mut Ctx) {
        self.persist(ctx.storage);
    }

    fn scene_loaded(&mut self, ctx: &mut Ctx) {
        if !self.scene_id.is_empty() {
            self.persist(ctx.storage);
        }
        let landmarks: Vec<LandmarkIn> = ctx
            .scene
            .data()
            .landmarks
            .iter()
            .map(|lm| LandmarkIn { name: lm.name.clone(), pos: lm.pos.to_array(), radius: lm.radius })
            .collect();
        self.targets = logic::generate_targets(&landmarks, ctx.params.seed);
        self.scene_id = ctx.scene_id.to_string();
        self.album = load_album(ctx.storage, &self.scene_id);
        self.result = None;
        self.flash = 0.0;
    }

    fn update(&mut self, ctx: &mut Ctx) {
        if self.flash > 0.0 {
            self.flash = (self.flash - ctx.dt / FLASH_SECONDS).max(0.0);
        }
        if let Some(shot) = &mut self.result {
            shot.age += ctx.dt;
        }
        if self.result.as_ref().is_some_and(|shot| shot.age > CARD_SECONDS) {
            self.result = None;
        }
    }

    fn window_event(&mut self, ev: &WindowEvent, ctx: &mut Ctx) -> bool {
        let WindowEvent::KeyboardInput { event, .. } = ev else {
            return false;
        };
        if event.state != ElementState::Pressed || event.repeat {
            return false;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return false;
        };
        match code {
            KeyCode::KeyF => {
                self.take_photo(ctx);
                true
            }
            KeyCode::KeyG => {
                self.album_open = !self.album_open;
                true
            }
            _ => false,
        }
    }

    fn ui(&mut self, egui_ctx: &egui::Context, ctx: &mut Ctx) {
        self.draw_status(egui_ctx, ctx);
        self.draw_album_button(egui_ctx);
        self.draw_shutter(egui_ctx, ctx);
        self.draw_album(egui_ctx);
        self.draw_result(egui_ctx);
        self.draw_flash(egui_ctx);
    }
}

impl PhotoHunt {
    fn persist(&self, storage: &mut Storage) {
        if self.scene_id.is_empty() {
            return;
        }
        storage.set(&logic::storage_key(&self.scene_id), &logic::serialize_album(&self.album));
    }

    fn take_photo(&mut self, ctx: &mut Ctx) {
        let hour = ctx.atmo.hour();
        let weather = weather_kind(ctx.atmo.weather);
        let mut captures = Vec::new();
        let mut misses = Vec::new();
        let mut improved = false;
        for target in &self.targets {
            let score = logic::score_target(target, sample_view(ctx, target, hour, weather));
            if score.stars >= 1 {
                let new_best = self.album.record(&target.id, score.stars, score.score);
                improved |= new_best;
                captures.push(CaptureLine { title: target.title(), stars: score.stars, score: score.score, new_best });
            } else if score.visible {
                let line = if score.condition_met {
                    format!("{} is in frame. Centre it for a clearer shot.", target.title())
                } else {
                    format!("{} — {}", target.title(), target.condition.hint())
                };
                misses.push(line);
            }
        }
        captures.sort_by(|a, b| b.stars.cmp(&a.stars).then(a.title.cmp(&b.title)));
        misses.sort();
        misses.truncate(4);
        if improved {
            self.persist(ctx.storage);
        }
        self.flash = 1.0;
        self.result = Some(ShotResult { captures, misses, age: 0.0 });
    }

    fn draw_status(&self, egui_ctx: &egui::Context, ctx: &Ctx) {
        let found = self.album.found_count(&self.targets);
        let total = self.targets.len();
        let clock = logic::clock_label(ctx.atmo.hour());
        let weather = ctx.atmo.weather.name();
        egui::Area::new(egui::Id::unique("photo_status"))
            .anchor(egui::Align2::LEFT_TOP, [12.0, 56.0])
            .show(egui_ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(format!("Photo Hunt  {found}/{total}  ·  {clock}  ·  {weather}"));
                });
            });
    }

    fn draw_album_button(&mut self, egui_ctx: &egui::Context) {
        let mut toggle = false;
        egui::Area::new(egui::Id::unique("photo_album_btn"))
            .anchor(egui::Align2::RIGHT_TOP, [-12.0, 56.0])
            .show(egui_ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let label = if self.album_open { "Hide album" } else { "Album" };
                    if ui
                        .add_sized(
                            [120.0, 52.0],
                            egui::Button::new(egui::RichText::new(label).size(16.0)).min_size(egui::vec2(120.0, 52.0)),
                        )
                        .clicked()
                    {
                        toggle = true;
                    }
                    ui.label(egui::RichText::new("G").weak());
                });
            });
        if toggle {
            self.album_open = !self.album_open;
        }
    }

    fn draw_shutter(&mut self, egui_ctx: &egui::Context, ctx: &mut Ctx) {
        let mut shoot = false;
        egui::Area::new(egui::Id::unique("photo_shutter"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -16.0])
            .show(egui_ctx, |ui| {
                ui.vertical_centered(|ui| {
                    if ui
                        .add_sized(
                            [140.0, 64.0],
                            egui::Button::new(egui::RichText::new("Shutter").size(20.0).strong())
                                .min_size(egui::vec2(140.0, 64.0))
                                .corner_radius(egui::CornerRadius::same(32)),
                        )
                        .clicked()
                    {
                        shoot = true;
                    }
                    ui.label(egui::RichText::new("F").weak());
                });
            });
        if shoot {
            self.take_photo(ctx);
        }
    }

    fn draw_album(&mut self, egui_ctx: &egui::Context) {
        let mut open = self.album_open;
        if open {
            let rows: Vec<AlbumRow> = self
                .targets
                .iter()
                .map(|t| AlbumRow {
                    title: t.title(),
                    found: self.album.is_found(&t.id),
                    stars: self.album.stars_of(&t.id),
                    score: self.album.score_of(&t.id),
                    hint: t.condition.hint(),
                })
                .collect();
            let found = self.album.found_count(&self.targets);
            let total = rows.len();
            egui::Window::new("Photo Album")
                .id(egui::Id::unique("photo_album"))
                .open(&mut open)
                .anchor(egui::Align2::RIGHT_TOP, [-12.0, 140.0])
                .collapsible(false)
                .resizable(false)
                .vscroll(true)
                .default_width(340.0)
                .max_height(480.0)
                .show(egui_ctx, |ui| {
                    if total == 0 {
                        ui.label("This world has no landmarks to photograph.");
                        return;
                    }
                    ui.label(egui::RichText::new(format!("Found {found}/{total}")).strong());
                    ui.label("Match the hint, then frame the landmark and press Shutter.");
                    for row in &rows {
                        ui.separator();
                        ui.label(egui::RichText::new(&row.title).strong());
                        if row.found {
                            ui.label(format!("{} · best score {:.3}", logic::stars_label(row.stars), row.score));
                        } else {
                            ui.weak(row.hint);
                        }
                    }
                });
        }
        self.album_open = open;
    }

    fn draw_result(&mut self, egui_ctx: &egui::Context) {
        let Some(shot) = self.result.clone() else { return };
        let mut dismiss = false;
        egui::Window::new("This shot")
            .id(egui::Id::unique("photo_shot"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, -20.0])
            .collapsible(false)
            .resizable(false)
            .vscroll(true)
            .default_width(400.0)
            .max_height(460.0)
            .show(egui_ctx, |ui| {
                if shot.captures.is_empty() {
                    ui.label(egui::RichText::new("Nothing captured").strong());
                    ui.label("Frame a landmark near the centre and match its condition.");
                } else {
                    ui.label(egui::RichText::new(format!("Captured {}", shot.captures.len())).strong());
                    for line in &shot.captures {
                        let best = if line.new_best { " · new best" } else { "" };
                        ui.label(format!("{} — {} ({:.3}){best}", line.title, logic::stars_label(line.stars), line.score));
                    }
                }
                if !shot.misses.is_empty() {
                    ui.separator();
                    ui.label("Still looking");
                    for miss in &shot.misses {
                        ui.weak(miss);
                    }
                }
                ui.add_space(8.0);
                if ui.add_sized([120.0, 44.0], egui::Button::new("Close")).clicked() {
                    dismiss = true;
                }
            });
        if dismiss {
            self.result = None;
        }
    }

    fn draw_flash(&self, egui_ctx: &egui::Context) {
        if self.flash <= 0.01 {
            return;
        }
        let alpha = (self.flash.clamp(0.0, 1.0) * 210.0) as u8;
        let painter = egui_ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("photo_flash")));
        painter.rect_filled(egui_ctx.viewport_rect(), 0.0, egui::Color32::from_white_alpha(alpha));
    }
}

fn load_album(storage: &Storage, scene_id: &str) -> Album {
    storage.get(&logic::storage_key(scene_id)).map(|text| logic::deserialize_album(&text)).unwrap_or_default()
}

fn sample_view(ctx: &Ctx, target: &Target, hour: f32, weather: WeatherKind) -> ViewSample {
    let pos = Vec3::from_array(target.pos);
    let screen_xy = if ctx.in_view(pos, 0.0) { ctx.world_to_screen(pos) } else { None };
    let distance = ctx.eye.distance(pos);
    ViewSample {
        screen_xy,
        screen_w: ctx.screen.0.max(1) as f32,
        screen_h: ctx.screen.1.max(1) as f32,
        distance,
        radius_px: logic::projected_radius_px(target.radius, distance, ctx.screen.1 as f32),
        hour,
        weather,
    }
}

fn weather_kind(weather: Weather) -> WeatherKind {
    match weather {
        Weather::Clear => WeatherKind::Clear,
        Weather::Cloudy => WeatherKind::Cloudy,
        Weather::Rain => WeatherKind::Rain,
        Weather::Storm => WeatherKind::Storm,
        Weather::Snow => WeatherKind::Snow,
        Weather::Mist => WeatherKind::Mist,
    }
}
