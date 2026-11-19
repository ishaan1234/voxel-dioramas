use glam::Vec3;
use std::f32::consts::{PI, TAU};

use super::scene::SceneData;

/// Movement keys currently held (WASD / QE / Shift / arrows).
#[derive(Default, Clone, Copy)]
pub struct MoveKeys {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub fast: bool,
    pub turn_left: bool,
    pub turn_right: bool,
    pub look_up: bool,
    pub look_down: bool,
}

impl MoveKeys {
    fn any(&self) -> bool {
        self.forward || self.back || self.left || self.right || self.up || self.down
            || self.turn_left || self.turn_right || self.look_up || self.look_down
    }
}

pub struct OrbitCam {
    pub keys: MoveKeys,
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
    // Input moves these goals; the visible camera eases toward them each frame
    // (critically-damped exponential smoothing), so drags, scroll steps and
    // preset changes glide instead of jumping.
    goal_target: Vec3,
    goal_yaw: f32,
    goal_pitch: f32,
    goal_dist: f32,
}

impl OrbitCam {
    pub fn new(scene: &SceneData) -> Self {
        let p0 = &scene.presets[0];
        let mut c = Self {
            keys: MoveKeys::default(),
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
            goal_target: p0.target,
            goal_yaw: 0.0,
            goal_pitch: 0.0,
            goal_dist: 200.0,
        };
        c.set_preset(scene, 0, true);
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

    /// Move to a preset: `snap` jumps there (screenshots), otherwise glide.
    pub fn set_preset(&mut self, scene: &SceneData, idx: usize, snap: bool) {
        if idx >= scene.presets.len() {
            return;
        }
        let p = &scene.presets[idx];
        if self.tour {
            self.tour = false;
            self.adopt_tour_view();
        }
        self.preset_name = p.name;
        let d = p.pos - p.target;
        let dist = d.length().clamp(6.0, self.max_dist);
        self.goal_target = p.target;
        self.goal_dist = dist;
        self.goal_pitch = (d.y / dist).asin().clamp(self.min_pitch, 1.48);
        // Take the short way round.
        let yaw = d.x.atan2(d.z);
        // Gliding takes the short way round; snapping uses the exact angle.
        self.goal_yaw = if snap { yaw } else { self.yaw + (yaw - self.yaw + PI).rem_euclid(TAU) - PI };
        if snap {
            self.target = self.goal_target;
            self.dist = self.goal_dist;
            self.pitch = self.goal_pitch;
            self.yaw = self.goal_yaw;
        }
    }

    /// Snap to an explicit eye / target (used by `--view` screenshots).
    pub fn set_view(&mut self, eye: Vec3, target: Vec3) {
        self.tour = false;
        self.preset_name = "Custom";
        let d = eye - target;
        self.target = target;
        self.dist = d.length().clamp(0.5, self.max_dist);
        self.pitch = (d.y / self.dist).asin();
        self.yaw = d.x.atan2(d.z);
        self.goal_target = self.target;
        self.goal_dist = self.dist;
        self.goal_pitch = self.pitch;
        self.goal_yaw = self.yaw;
    }

    /// When leaving the tour, continue orbiting from where the tour camera was.
    fn adopt_tour_view(&mut self) {
        let d = self.tour_pos_actual - self.tour_tgt_actual;
        self.target = self.tour_tgt_actual;
        self.dist = d.length().clamp(6.0, self.max_dist);
        self.pitch = (d.y / self.dist).asin().clamp(self.min_pitch, 1.48);
        self.yaw = d.x.atan2(d.z);
        self.goal_target = self.target;
        self.goal_dist = self.dist;
        self.goal_pitch = self.pitch;
        self.goal_yaw = self.yaw;
    }

    fn leave_tour(&mut self) {
        if self.tour {
            self.tour = false;
            self.adopt_tour_view();
        }
    }

    /// Apply held keys to the goals: WASD moves the look-at point across the
    /// ground plane relative to the view, Q/E lower/raise it, arrows turn.
    fn apply_keys(&mut self, dt: f32) {
        let k = self.keys;
        if !k.any() {
            return;
        }
        self.leave_tour();
        let speed = (self.goal_dist * 0.55).max(8.0) * if k.fast { 3.0 } else { 1.0 } * dt;
        // The camera looks along -(sin yaw, cos yaw) on the ground plane.
        let fwd = -Vec3::new(self.goal_yaw.sin(), 0.0, self.goal_yaw.cos());
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        let axis = |pos: bool, neg: bool| (pos as i32 - neg as i32) as f32;
        self.goal_target += fwd * (axis(k.forward, k.back) * speed)
            + right * (axis(k.right, k.left) * speed)
            + Vec3::Y * (axis(k.up, k.down) * speed);
        let turn = 1.6 * dt * if k.fast { 2.0 } else { 1.0 };
        self.goal_yaw += axis(k.turn_left, k.turn_right) * turn;
        self.goal_pitch = (self.goal_pitch + axis(k.look_up, k.look_down) * turn).clamp(self.min_pitch, 1.48);
    }

    /// Ease the orbit camera toward its goals (frame-rate independent).
    pub fn advance(&mut self, dt: f32) {
        self.apply_keys(dt);
        let k = 1.0 - (-14.0 * dt).exp();
        self.target = self.target.lerp(self.goal_target, k);
        self.yaw += (self.goal_yaw - self.yaw) * k;
        self.pitch += (self.goal_pitch - self.pitch) * k;
        // Zoom in log space so it feels uniform at every distance. Skip when
        // settled: exp(ln(x)) is not exactly x and would jitter a still camera.
        if self.dist != self.goal_dist {
            self.dist = (self.dist.ln() + (self.goal_dist.ln() - self.dist.ln()) * k).exp();
        }
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
        self.leave_tour();
        self.goal_yaw -= dx * 0.0055;
        self.goal_pitch = (self.goal_pitch + dy * 0.0055).clamp(self.min_pitch, 1.48);
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.leave_tour();
        let right = Vec3::new(self.goal_yaw.cos(), 0.0, -self.goal_yaw.sin());
        let fwd = Vec3::new(self.goal_yaw.sin(), 0.0, self.goal_yaw.cos());
        let pan = self.goal_dist * 0.0014;
        self.goal_target -= right * (dx * pan) + fwd * (dy * pan);
    }

    pub fn zoom(&mut self, scroll: f32) {
        self.leave_tour();
        self.goal_dist = (self.goal_dist * (1.0 - scroll * 0.12)).clamp(6.0, self.max_dist);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam_looking(eye: Vec3, target: Vec3) -> OrbitCam {
        let mut c = OrbitCam {
            keys: MoveKeys::default(),
            target,
            yaw: 0.0,
            pitch: 0.0,
            dist: 10.0,
            max_dist: 500.0,
            min_pitch: -1.2,
            tour: false,
            tour_t: 0.0,
            tour_pos_actual: eye,
            tour_tgt_actual: target,
            preset_name: "test",
            goal_target: target,
            goal_yaw: 0.0,
            goal_pitch: 0.0,
            goal_dist: 10.0,
        };
        c.set_view(eye, target);
        c
    }

    #[test]
    fn wasd_moves_relative_to_view() {
        // Looking from +Z toward the origin, i.e. along -Z.
        let mut c = cam_looking(Vec3::new(0.0, 5.0, 20.0), Vec3::ZERO);
        let start_eye = c.eye();
        c.keys.forward = true;
        for _ in 0..60 {
            c.advance(1.0 / 60.0);
        }
        c.keys.forward = false;
        for _ in 0..120 {
            c.advance(1.0 / 60.0); // let the smoothing settle
        }
        assert!(c.target.z < -1.0, "W should move toward -Z, got {:?}", c.target);
        assert!(c.eye().z < start_eye.z);
        let z = c.target.z;
        c.keys.right = true;
        for _ in 0..60 {
            c.advance(1.0 / 60.0);
        }
        assert!(c.target.x > 1.0, "D should move toward +X, got {:?}", c.target);
        assert!((c.target.z - z).abs() < 0.5, "strafing should not move forward");
    }

    #[test]
    fn settled_camera_does_not_drift() {
        let mut c = cam_looking(Vec3::new(30.0, 20.0, 40.0), Vec3::new(1.0, 2.0, 3.0));
        let e = c.eye();
        for _ in 0..1000 {
            c.advance(1.0 / 60.0);
        }
        assert_eq!(c.eye(), e);
    }
}
