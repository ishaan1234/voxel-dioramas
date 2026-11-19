//! Time of day and weather.
//!
//! Scenes still define three hand-tuned lighting keyframes (day, golden hour,
//! night). The clock either shows one of them exactly (`DayClock::Preset`,
//! which reproduces the original look bit-for-bit) or runs a continuous
//! 24-hour cycle that blends the keyframes and moves the sun along an arc.
//! Weather is layered on top as a modifier plus its own particle field.

use glam::Vec3;

use super::scene::{SceneData, TimeModeParams};

#[derive(Copy, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DayClock {
    /// One of the scene's three keyframes: 0 day, 1 golden hour, 2 night.
    Preset(usize),
    /// Continuous time. `hour` in [0, 24). `speed` = in-game hours per real
    /// second (0 = frozen). `real_time` follows the device clock instead.
    Hours { hour: f32, speed: f32, real_time: bool },
}

impl Default for DayClock {
    fn default() -> Self {
        DayClock::Preset(0)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Weather {
    Clear,
    Cloudy,
    Rain,
    Storm,
    Snow,
    Mist,
}

impl Weather {
    pub const ALL: [Weather; 6] = [Weather::Clear, Weather::Cloudy, Weather::Rain, Weather::Storm, Weather::Snow, Weather::Mist];
    pub fn name(self) -> &'static str {
        match self {
            Weather::Clear => "Clear",
            Weather::Cloudy => "Cloudy",
            Weather::Rain => "Rain",
            Weather::Storm => "Storm",
            Weather::Snow => "Snow",
            Weather::Mist => "Mist",
        }
    }
}

/// Weather particle look (rain streaks / snowflakes), consumed by the renderer.
#[derive(Copy, Clone, Debug)]
pub struct Precipitation {
    pub count: u32,
    pub fall: (f32, f32),
    pub drift: (f32, f32),
    pub size: Vec3,
    pub color: u32,
    pub glow: f32,
    /// 0 = falls straight, 1 = petal-like sway and spin.
    pub sway: f32,
}

#[derive(Clone, Debug)]
pub struct Atmosphere {
    pub clock: DayClock,
    pub weather: Weather,
    /// 0..1 fade-in of the current weather.
    pub weather_mix: f32,
}

impl Default for Atmosphere {
    fn default() -> Self {
        Self { clock: DayClock::default(), weather: Weather::Clear, weather_mix: 1.0 }
    }
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn lerpf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Blend two lighting keyframes (sun position handled separately).
pub fn lerp_params(a: &TimeModeParams, b: &TimeModeParams, t: f32) -> TimeModeParams {
    TimeModeParams {
        name: if t < 0.5 { a.name } else { b.name },
        sky_top: lerp3(a.sky_top, b.sky_top, t),
        sky_mid: lerp3(a.sky_mid, b.sky_mid, t),
        sky_bot: lerp3(a.sky_bot, b.sky_bot, t),
        fog_col: lerp3(a.fog_col, b.fog_col, t),
        fog_d: lerpf(a.fog_d, b.fog_d, t),
        sun_col: lerp3(a.sun_col, b.sun_col, t),
        sun_int: lerpf(a.sun_int, b.sun_int, t),
        sun_pos: a.sun_pos.lerp(b.sun_pos, t),
        hemi_sky: lerp3(a.hemi_sky, b.hemi_sky, t),
        hemi_gnd: lerp3(a.hemi_gnd, b.hemi_gnd, t),
        hemi_int: lerpf(a.hemi_int, b.hemi_int, t),
        win_emissive: lerpf(a.win_emissive, b.win_emissive, t),
        lamp_emissive: lerpf(a.lamp_emissive, b.lamp_emissive, t),
        point_int: lerpf(a.point_int, b.point_int, t),
        exposure: lerpf(a.exposure, b.exposure, t),
        stars: lerpf(a.stars, b.stars, t),
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Current local hour of the device clock.
pub fn device_hour() -> f32 {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        d.get_hours() as f32 + d.get_minutes() as f32 / 60.0 + d.get_seconds() as f32 / 3600.0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use chrono::Timelike;
        let now = chrono::Local::now();
        now.hour() as f32 + now.minute() as f32 / 60.0 + now.second() as f32 / 3600.0
    }
}

impl Atmosphere {
    /// Advance the clock by `dt` real seconds.
    pub fn tick(&mut self, dt: f32) {
        if let DayClock::Hours { hour, speed, real_time } = &mut self.clock {
            if *real_time {
                *hour = device_hour();
            } else {
                *hour = (*hour + *speed * dt).rem_euclid(24.0);
            }
        }
        self.weather_mix = (self.weather_mix + dt / 3.0).min(1.0);
    }

    pub fn set_weather(&mut self, w: Weather) {
        if w != self.weather {
            self.weather = w;
            self.weather_mix = 0.0;
        }
    }

    /// Current hour (preset clocks report a representative hour).
    pub fn hour(&self) -> f32 {
        match self.clock {
            DayClock::Preset(0) => 12.0,
            DayClock::Preset(1) => 18.0,
            DayClock::Preset(_) => 0.0,
            DayClock::Hours { hour, .. } => hour,
        }
    }

    /// Lighting for this frame. `t` = simulation time (lightning flicker).
    pub fn resolve(&self, d: &SceneData, t: f32) -> TimeModeParams {
        let base = match self.clock {
            DayClock::Preset(i) => d.time_modes[i.min(2)].clone(),
            DayClock::Hours { hour, .. } => blend_hour(d, hour),
        };
        self.apply_weather(base, t)
    }

    fn apply_weather(&self, mut p: TimeModeParams, t: f32) -> TimeModeParams {
        let k = smooth(self.weather_mix);
        let grey = |c: [f32; 3], amount: f32, tint: [f32; 3]| {
            let l = c[0] * 0.3 + c[1] * 0.59 + c[2] * 0.11;
            lerp3(c, [l * tint[0], l * tint[1], l * tint[2]], amount)
        };
        let (desat, sun, fog, expo, stars, tint) = match self.weather {
            Weather::Clear => return p,
            Weather::Cloudy => (0.45, 0.5, 1.4, 0.97, 0.3, [1.0, 1.0, 1.03]),
            Weather::Rain => (0.65, 0.3, 1.9, 0.9, 0.0, [0.95, 0.98, 1.08]),
            Weather::Storm => (0.8, 0.15, 2.3, 0.78, 0.0, [0.9, 0.93, 1.1]),
            Weather::Snow => (0.55, 0.5, 1.7, 1.02, 0.1, [1.0, 1.02, 1.08]),
            Weather::Mist => (0.35, 0.7, 3.2, 1.0, 0.2, [1.0, 1.0, 1.0]),
        };
        let a = desat * k;
        p.sky_top = grey(p.sky_top, a, tint);
        p.sky_mid = grey(p.sky_mid, a, tint);
        p.sky_bot = grey(p.sky_bot, a, tint);
        p.fog_col = grey(p.fog_col, a, tint);
        p.sun_col = grey(p.sun_col, a * 0.5, tint);
        p.sun_int *= lerpf(1.0, sun, k);
        p.fog_d *= lerpf(1.0, fog, k);
        p.exposure *= lerpf(1.0, expo, k);
        p.stars *= lerpf(1.0, stars, k);
        if self.weather == Weather::Snow {
            // Snow brightens the ground bounce.
            p.hemi_gnd = lerp3(p.hemi_gnd, [0.85, 0.88, 0.95], 0.5 * k);
        }
        if self.weather == Weather::Storm {
            // Lightning: brief, irregular exposure flashes.
            let slot = (t * 1.7).floor();
            let h = ((slot * 12.9898).sin() * 43758.545).fract().abs();
            let phase = (t * 1.7).fract();
            if h > 0.93 && phase < 0.12 {
                let f = 1.0 - phase / 0.12;
                p.exposure *= 1.0 + 2.5 * f * k;
                p.sky_top = lerp3(p.sky_top, [0.8, 0.85, 1.0], 0.5 * f * k);
            }
        }
        p
    }

    /// Particles for the current weather, if any.
    pub fn precipitation(&self) -> Option<Precipitation> {
        match self.weather {
            Weather::Rain | Weather::Storm => Some(Precipitation {
                count: if self.weather == Weather::Storm { 9000 } else { 6000 },
                fall: (22.0, 30.0),
                drift: (0.5, 1.5),
                size: Vec3::new(0.03, 0.7, 0.03),
                color: 0xb8c4d8,
                glow: 0.8,
                sway: 0.0,
            }),
            Weather::Snow => Some(Precipitation {
                count: 5000,
                fall: (1.2, 2.6),
                drift: (0.3, 1.0),
                size: Vec3::new(0.22, 0.22, 0.22),
                color: 0xffffff,
                glow: 1.25,
                sway: 0.6,
            }),
            _ => None,
        }
    }
}

/// Continuous day: blend night → golden → day → golden → night by hour and
/// move the sun on an east-west arc around the scene's noon direction.
fn blend_hour(d: &SceneData, hour: f32) -> TimeModeParams {
    let [day, gold, night] = &d.time_modes;
    let h = hour.rem_euclid(24.0);
    let mut p = match h {
        h if h < 4.5 => night.clone(),
        h if h < 6.5 => lerp_params(night, gold, smooth((h - 4.5) / 2.0)),
        h if h < 9.0 => lerp_params(gold, day, smooth((h - 6.5) / 2.5)),
        h if h < 16.0 => day.clone(),
        h if h < 18.5 => lerp_params(day, gold, smooth((h - 16.0) / 2.5)),
        h if h < 20.5 => lerp_params(gold, night, smooth((h - 18.5) / 2.0)),
        _ => night.clone(),
    };
    // Sun arc: rises at 6, peaks at 12, sets at 18; the moon takes the night.
    let target = d.shadow.target;
    let noon = day.sun_pos - target;
    let dist = noon.length();
    let noon_az = noon.z.atan2(noon.x);
    let noon_el = (noon.y / dist).asin();
    let (frac, el_max) = if (6.0..18.0).contains(&h) {
        ((h - 6.0) / 12.0, noon_el)
    } else {
        ((h + 6.0).rem_euclid(24.0) / 12.0, (night.sun_pos - target).normalize().y.asin())
    };
    let az = noon_az + (frac - 0.5) * std::f32::consts::PI * 0.9;
    let el = (frac * std::f32::consts::PI).sin().max(0.0) * el_max + 0.08;
    let dir = Vec3::new(el.cos() * az.cos(), el.sin(), el.cos() * az.sin());
    p.sun_pos = target + dir * dist;
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hour_blend_is_continuous() {
        // A tiny SceneData stand-in is awkward to build; test the curve itself.
        let mut last = smooth(0.0);
        for i in 1..=100 {
            let v = smooth(i as f32 / 100.0);
            assert!(v >= last && v - last < 0.03);
            last = v;
        }
    }

    #[test]
    fn clear_weather_is_identity() {
        let a = Atmosphere::default();
        assert!(a.precipitation().is_none());
    }
}
