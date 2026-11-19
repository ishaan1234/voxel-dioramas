//! egui integration (desktop + web) and the global menu bar ("shell").

use std::sync::Arc;

use winit::window::Window;

use super::app::SceneEntry;
use super::atmosphere::{Atmosphere, DayClock, Weather};
use super::experience::ExperienceEntry;
use super::scene::{Season, WorldParams};

pub struct EguiLayer {
    pub ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

impl EguiLayer {
    pub fn new(window: &Arc<Window>, device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        ctx.set_style_of(egui::Theme::Dark, touch_friendly_style());
        ctx.set_theme(egui::Theme::Dark);
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            Some(device.limits().max_texture_dimension_2d as usize),
        );
        let renderer = egui_wgpu::Renderer::new(device, target_format, egui_wgpu::RendererOptions::default());
        Self { ctx, state, renderer }
    }

    /// Feed a window event; returns true if egui wants it exclusively.
    pub fn on_event(&mut self, window: &Window, ev: &winit::event::WindowEvent) -> bool {
        self.state.on_window_event(window, ev).consumed
    }

    /// Does egui currently want pointer / keyboard input (hovering a panel,
    /// typing in a field)?
    pub fn wants_pointer(&self) -> bool {
        self.ctx.egui_wants_pointer_input() || self.ctx.is_pointer_over_egui()
    }
    pub fn wants_keyboard(&self) -> bool {
        self.ctx.egui_wants_keyboard_input()
    }

    pub fn begin(&mut self, window: &Window) {
        let input = self.state.take_egui_input(window);
        self.ctx.begin_pass(input);
    }

    /// Finish the egui frame and paint it on top of `view`.
    pub fn end_and_paint(
        &mut self,
        window: &Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        size: (u32, u32),
    ) -> Vec<wgpu::CommandBuffer> {
        let full = self.ctx.end_pass();
        self.state.handle_platform_output(window, full.platform_output);
        let ppp = full.pixels_per_point;
        let prims = self.ctx.tessellate(full.shapes, ppp);
        for (id, deltas) in &full.textures_delta.set {
            for delta in deltas {
                self.renderer.update_texture(device, queue, *id, delta);
            }
        }
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [size.0, size.1], pixels_per_point: ppp };
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("egui") });
        let mut cmds = self.renderer.update_buffers(device, queue, &mut enc, &prims, &sd);
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            let mut pass = pass.forget_lifetime();
            self.renderer.render(&mut pass, &prims, &sd);
        }
        for id in &full.textures_delta.free {
            self.renderer.free_texture(id);
        }
        cmds.push(enc.finish());
        cmds
    }
}

/// Bigger hit targets and text so the UI works with fingers on a tablet.
fn touch_friendly_style() -> egui::Style {
    let mut s = egui::Style::default();
    s.spacing.interact_size = egui::vec2(44.0, 30.0);
    s.spacing.button_padding = egui::vec2(10.0, 6.0);
    s.spacing.item_spacing = egui::vec2(8.0, 6.0);
    s.spacing.slider_width = 160.0;
    for (_, font) in s.text_styles.iter_mut() {
        font.size *= 1.12;
    }
    s.visuals.window_corner_radius = egui::CornerRadius::same(12);
    s
}

/// What the menu bar shows and edits.
pub struct ShellView<'a> {
    pub scenes: &'static [SceneEntry],
    pub scene_idx: usize,
    pub experiences: &'static [ExperienceEntry],
    pub exp_idx: usize,
    pub params: WorldParams,
    pub atmo: &'a mut Atmosphere,
    pub presets: Vec<&'static str>,
    pub tour: bool,
    pub paused: bool,
    pub fps: u32,
    pub loading: bool,
}

pub enum ShellAction {
    LoadScene(usize, WorldParams),
    Experience(usize),
    Preset(usize),
    ToggleTour,
    TogglePause,
}

/// The global menu bar. Returns actions for the app to apply.
pub fn shell_ui(ctx: &egui::Context, v: &mut ShellView, expanded: &mut bool) -> Vec<ShellAction> {
    let mut out = Vec::new();
    ctx.root_ui(|root| {
    egui::Panel::top("shell")
        .frame(egui::Frame::new().fill(egui::Color32::from_rgba_unmultiplied(14, 18, 30, 228)).inner_margin(egui::Margin::symmetric(10, 6)))
        .show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(if *expanded { "Close" } else { "Menu" }).clicked() {
                    *expanded = !*expanded;
                }
                ui.label(egui::RichText::new(v.experiences[v.exp_idx].title).strong());
                ui.label("·");
                ui.label(v.scenes[v.scene_idx].title);
                if v.loading {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("{} fps", v.fps)).weak());
                });
            });
            if !*expanded {
                return;
            }
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                // Mode
                let mut exp = v.exp_idx;
                egui::ComboBox::from_label("Mode")
                    .selected_text(v.experiences[exp].title)
                    .show_ui(ui, |ui| {
                        for (i, e) in v.experiences.iter().enumerate() {
                            ui.selectable_value(&mut exp, i, e.title).on_hover_text(e.blurb);
                        }
                    });
                if exp != v.exp_idx {
                    out.push(ShellAction::Experience(exp));
                }

                // Scene
                let mut sc = v.scene_idx;
                egui::ComboBox::from_label("World")
                    .selected_text(v.scenes[sc].title)
                    .show_ui(ui, |ui| {
                        for (i, s) in v.scenes.iter().enumerate() {
                            ui.selectable_value(&mut sc, i, s.title);
                        }
                    });
                if sc != v.scene_idx && !v.loading {
                    out.push(ShellAction::LoadScene(sc, v.params));
                }

                // Season (regenerates the world)
                let mut season = v.params.season;
                egui::ComboBox::from_label("Season")
                    .selected_text(season.name())
                    .show_ui(ui, |ui| {
                        for s in Season::ALL {
                            ui.selectable_value(&mut season, s, s.name());
                        }
                    });
                if season != v.params.season && !v.loading {
                    out.push(ShellAction::LoadScene(v.scene_idx, WorldParams { season, ..v.params }));
                }

                // Seed
                ui.label("Seed");
                let mut seed = v.params.seed;
                let resp = ui.add(egui::DragValue::new(&mut seed).range(0..=999_999));
                if resp.lost_focus() && seed != v.params.seed && !v.loading {
                    out.push(ShellAction::LoadScene(v.scene_idx, WorldParams { seed, ..v.params }));
                }
                if ui.button("🎲").on_hover_text("New random world").clicked() && !v.loading {
                    let s = (web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(7) % 999_999).max(1);
                    out.push(ShellAction::LoadScene(v.scene_idx, WorldParams { seed: s, ..v.params }));
                }
                if v.params.seed != 0 && ui.button("Original").clicked() && !v.loading {
                    out.push(ShellAction::LoadScene(v.scene_idx, WorldParams { seed: 0, ..v.params }));
                }
            });
            ui.horizontal_wrapped(|ui| {
                // Time of day
                let label = match v.atmo.clock {
                    DayClock::Preset(0) => "Day",
                    DayClock::Preset(1) => "Golden hour",
                    DayClock::Preset(_) => "Night",
                    DayClock::Hours { real_time: true, .. } => "Live clock",
                    DayClock::Hours { .. } => "Day cycle",
                };
                egui::ComboBox::from_label("Time")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for (i, n) in ["Day", "Golden hour", "Night"].iter().enumerate() {
                            if ui.selectable_label(v.atmo.clock == DayClock::Preset(i), *n).clicked() {
                                v.atmo.clock = DayClock::Preset(i);
                            }
                        }
                        if ui.selectable_label(matches!(v.atmo.clock, DayClock::Hours { real_time: false, .. }), "Day cycle").clicked() {
                            v.atmo.clock = DayClock::Hours { hour: v.atmo.hour(), speed: 0.2, real_time: false };
                        }
                        if ui.selectable_label(matches!(v.atmo.clock, DayClock::Hours { real_time: true, .. }), "Live clock").clicked() {
                            v.atmo.clock = DayClock::Hours { hour: super::atmosphere::device_hour(), speed: 0.0, real_time: true };
                        }
                    });
                if let DayClock::Hours { hour, speed, real_time: false } = &mut v.atmo.clock {
                    ui.add(egui::Slider::new(hour, 0.0..=23.99).text("h").fixed_decimals(1));
                    ui.add(egui::Slider::new(speed, 0.0..=2.0).text("speed"));
                }

                // Weather
                let mut w = v.atmo.weather;
                egui::ComboBox::from_label("Weather")
                    .selected_text(w.name())
                    .show_ui(ui, |ui| {
                        for x in Weather::ALL {
                            ui.selectable_value(&mut w, x, x.name());
                        }
                    });
                v.atmo.set_weather(w);

                ui.menu_button("Views", |ui| {
                    for (i, n) in v.presets.iter().enumerate() {
                        if ui.button(format!("{}  {n}", i + 1)).clicked() {
                            out.push(ShellAction::Preset(i));
                            ui.close();
                        }
                    }
                });
                if ui.selectable_label(v.tour, "Tour").clicked() {
                    out.push(ShellAction::ToggleTour);
                }
                if ui.selectable_label(v.paused, "Pause").clicked() {
                    out.push(ShellAction::TogglePause);
                }
            });
        });
    });
    out
}
