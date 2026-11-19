use glam::Vec3;

use super::scene::SceneData;

pub struct OrbitCam {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub max_dist: f32,
    pub min_pitch: f32,
    pub tour: bool,
    pub tour_t: f32,
    pub tour_pos_actual: Vec3,
    pub tour_tgt_actual: Vec3,
    pub preset_name: &'static str,
}

impl OrbitCam {
    pub fn new(scene: &SceneData) -> Self {
        let p0 = &scene.presets[0];
        let mut c = Self {
            target: p0.target,
            yaw: 0.0,
            pitch: 0.0,
            dist: 200.0,
            max_dist: scene.max_cam_dist,
            min_pitch: scene.min_pitch,
            tour: false,
            tour_t: 0.0,
            tour_pos_actual: p0.pos,
            tour_tgt_actual: p0.target,
            preset_name: p0.name,
        };
        c.set_preset(scene, 0);
        c
    }

    pub fn eye(&self) -> Vec3 {
        if self.tour {
            return self.tour_pos_actual;
        }
        let cp = self.pitch.cos();
        self.target + Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), self.yaw.cos() * cp) * self.dist
    }

    pub fn look_target(&self) -> Vec3 {
        if self.tour { self.tour_tgt_actual } else { self.target }
    }

    pub fn set_preset(&mut self, scene: &SceneData, idx: usize) {
        if idx >= scene.presets.len() {
            return;
        }
        let p = &scene.presets[idx];
        self.tour = false;
        self.preset_name = p.name;
        self.target = p.target;
        let d = p.pos - p.target;
        self.dist = d.length().clamp(6.0, self.max_dist);
        self.pitch = (d.y / self.dist).asin().clamp(self.min_pitch, 1.48);
        self.yaw = d.x.atan2(d.z);
    }

    pub fn toggle_tour(&mut self) {
        self.tour = !self.tour;
        if self.tour {
            self.tour_pos_actual = self.eye_orbit();
            self.tour_tgt_actual = self.target;
        }
    }

    fn eye_orbit(&self) -> Vec3 {
        let cp = self.pitch.cos();
        self.target + Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), self.yaw.cos() * cp) * self.dist
    }

    pub fn advance_tour(&mut self, scene: &SceneData, dt: f32) {
        if !self.tour {
            return;
        }
        self.tour_t = (self.tour_t + dt / scene.tour_secs).fract();
        let gp = scene.tour_pos.get_point_at(self.tour_t);
        let gl = scene.tour_look.get_point_at(self.tour_t);
        let alpha = 1.0 - (-4.5 * dt).exp();
        self.tour_pos_actual = self.tour_pos_actual.lerp(gp, alpha);
        self.tour_tgt_actual = self.tour_tgt_actual.lerp(gl, alpha);
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.tour = false;
        self.yaw -= dx * 0.0055;
        self.pitch = (self.pitch + dy * 0.0055).clamp(self.min_pitch, 1.48);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.tour = false;
        let right = Vec3::new(self.yaw.cos(), 0.0, -self.yaw.sin());
        let fwd = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let pan = self.dist * 0.0014;
        self.target -= right * (dx * pan) + fwd * (dy * pan);
    }

    pub fn zoom(&mut self, scroll: f32) {
        self.tour = false;
        self.dist = (self.dist * (1.0 - scroll * 0.1)).clamp(6.0, self.max_dist);
    }
}
