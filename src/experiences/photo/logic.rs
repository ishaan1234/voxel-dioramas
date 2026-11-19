//! Pure Photo Hunt rules: target lists, shutter scoring, and the album file.
//!
//! Scoring (0–3 stars), once a target is inside the view and its condition holds:
//!
//! ```text
//! centred = 1 at the screen centre, 0 at a corner
//! size    = 1 when the landmark's on-screen diameter matches the sweet spot
//!           for that condition, 0 when the ratio is about 3× off
//! score   = 0.62 * centred + 0.38 * size
//! stars   = 3 if score >= 0.72,  2 if >= 0.46,  1 if >= 0.20,  else 0
//! ```
//!
//! Anything off-screen, or whose time / weather / distance condition fails, is 0.
//! Sweet-spot distances sit inside the close-up and far-away gates, so a careful
//! shot of either kind can still reach 3 stars.
//!
//! Time of day uses the hour reported by `Atmosphere::hour` (preset clocks are
//! 12:00 day, 18:00 golden hour, 00:00 night):
//!
//! ```text
//! night      hour < 5.5  or hour >= 19.5
//! golden     5.5 <= hour < 8.5  or  16 <= hour < 19.5
//! daylight   8.5 <= hour < 16
//! ```
//!
//! Distance gates (metres), from the landmark radius `r`:
//!
//! ```text
//! close = clamp(max(r, 0.5) * 6.5, 14, 140)     close-up: distance <= close
//! far   = clamp(max(max(r, 0.5) * 18, close * 2.2), close * 2.2, 640)
//!                                                far away: distance >= far
//! ```

use std::f32::consts::{PI, SQRT_2};

/// Vertical field of view used by the renderer (`engine::gpu`, 44 degrees).
const VERTICAL_FOV_RAD: f32 = 44.0 * PI / 180.0;

const CENTRE_WEIGHT: f32 = 0.62;
const SIZE_WEIGHT: f32 = 0.38;
const STAR_3: f32 = 0.72;
const STAR_2: f32 = 0.46;
const STAR_1: f32 = 0.20;

/// What the photograph has to show, besides the landmark itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Condition {
    Night,
    GoldenHour,
    Rain,
    CloseUp,
    FarAway,
    Daylight,
    Storm,
    Snow,
}

const CONDITIONS: [Condition; 8] = [
    Condition::Night,
    Condition::GoldenHour,
    Condition::Rain,
    Condition::CloseUp,
    Condition::FarAway,
    Condition::Daylight,
    Condition::Storm,
    Condition::Snow,
];

impl Condition {
    pub(super) fn key(self) -> &'static str {
        match self {
            Condition::Night => "night",
            Condition::GoldenHour => "golden",
            Condition::Rain => "rain",
            Condition::CloseUp => "close",
            Condition::FarAway => "far",
            Condition::Daylight => "day",
            Condition::Storm => "storm",
            Condition::Snow => "snow",
        }
    }

    pub(super) fn phrase(self) -> &'static str {
        match self {
            Condition::Night => "at night",
            Condition::GoldenHour => "during golden hour",
            Condition::Rain => "in the rain",
            Condition::CloseUp => "close up",
            Condition::FarAway => "from far away",
            Condition::Daylight => "in daylight",
            Condition::Storm => "in a storm",
            Condition::Snow => "in the snow",
        }
    }

    pub(super) fn hint(self) -> &'static str {
        match self {
            Condition::Night => "Try visiting at night",
            Condition::GoldenHour => "Try golden hour",
            Condition::Rain => "Wait for the rain",
            Condition::CloseUp => "Get closer",
            Condition::FarAway => "Step farther back",
            Condition::Daylight => "Try this in daylight",
            Condition::Storm => "Wait for a storm",
            Condition::Snow => "Wait for snow",
        }
    }
}

/// Coarse time of day derived from an hour in `[0, 24)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Night,
    Golden,
    Day,
}

pub(super) fn day_phase(hour: f32) -> Phase {
    let h = if hour.is_finite() { hour.rem_euclid(24.0) } else { 12.0 };
    if h < 5.5 || h >= 19.5 {
        Phase::Night
    } else if h < 8.5 || h >= 16.0 {
        Phase::Golden
    } else {
        Phase::Day
    }
}

pub(super) fn clock_label(hour: f32) -> &'static str {
    match day_phase(hour) {
        Phase::Night => "Night",
        Phase::Golden => "Golden hour",
        Phase::Day => "Daylight",
    }
}

pub(super) fn stars_label(stars: u8) -> &'static str {
    match stars {
        0 => "0 stars",
        1 => "1 star",
        2 => "2 stars",
        _ => "3 stars",
    }
}

/// Weather the shutter checks. Mapped from `engine::atmosphere::Weather`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WeatherKind {
    Clear,
    Cloudy,
    Rain,
    Storm,
    Snow,
    Mist,
}

/// A scene landmark, detached from the engine type so the rules stay testable.
#[derive(Clone, Debug)]
pub(super) struct LandmarkIn {
    pub name: String,
    pub pos: [f32; 3],
    pub radius: f32,
}

/// One photograph the player is trying to take.
#[derive(Clone, Debug)]
pub(super) struct Target {
    pub id: String,
    pub landmark: String,
    pub condition: Condition,
    pub pos: [f32; 3],
    pub radius: f32,
}

impl Target {
    pub(super) fn title(&self) -> String {
        format!("{} {}", self.landmark, self.condition.phrase())
    }
}

/// Where the landmark sits in this frame, plus the clock and weather.
#[derive(Clone, Copy, Debug)]
pub(super) struct ViewSample {
    /// Pixel position. `None` when the point is behind the camera or off-screen.
    pub screen_xy: Option<(f32, f32)>,
    pub screen_w: f32,
    pub screen_h: f32,
    /// Metres from the camera to the landmark.
    pub distance: f32,
    /// Projected radius in pixels.
    pub radius_px: f32,
    pub hour: f32,
    pub weather: WeatherKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Score {
    pub stars: u8,
    pub score: f32,
    pub visible: bool,
    pub centred: f32,
    pub size: f32,
    pub condition_met: bool,
}

/// Best shots for one scene. Records stay sorted by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Album {
    records: Vec<AlbumRecord>,
}

#[derive(Clone, Debug, PartialEq)]
struct AlbumRecord {
    id: String,
    stars: u8,
    score: f32,
}

pub(super) fn storage_key(scene_id: &str) -> String {
    format!("photo.album.{scene_id}")
}

/// 6–10 targets. The same landmarks and seed always produce the same list.
/// Every list includes night, golden hour, rain, close-up, and far-away shots.
pub(super) fn generate_targets(landmarks: &[LandmarkIn], seed: u32) -> Vec<Target> {
    let n = landmarks.len();
    if n == 0 {
        return Vec::new();
    }
    let shift = hash_u32(seed, n as u32) as usize;
    let mut pairs = Vec::new();
    for cond_i in 0..CONDITIONS.len() {
        pairs.push(((shift + cond_i * 3) % n, cond_i));
    }
    for lap in 1..n {
        for cond_i in 0..CONDITIONS.len() {
            pairs.push(((shift + cond_i * 3 + lap) % n, cond_i));
        }
    }

    let mut seen = vec![false; n * CONDITIONS.len()];
    let mut unique = Vec::new();
    for (lm_i, cond_i) in pairs {
        let key = lm_i * CONDITIONS.len() + cond_i;
        if seen[key] {
            continue;
        }
        seen[key] = true;
        unique.push((lm_i, cond_i));
    }
    let want = desired_count(n, seed).min(unique.len());
    unique.truncate(want);

    let mut used_ids: Vec<(String, u32)> = Vec::new();
    let mut targets = Vec::with_capacity(unique.len());
    for (lm_i, cond_i) in unique {
        let lm = &landmarks[lm_i];
        let condition = CONDITIONS[cond_i];
        let (id, display) = unique_id(&lm.name, condition, &mut used_ids);
        targets.push(Target {
            id,
            landmark: display,
            condition,
            pos: lm.pos,
            radius: lm.radius.max(0.0),
        });
    }
    targets
}

fn desired_count(n_landmarks: usize, seed: u32) -> usize {
    let h = hash_u32(seed ^ 0x51ed_1234, n_landmarks as u32);
    6 + (h % 5) as usize
}

fn hash_u32(a: u32, b: u32) -> u32 {
    let mut h = 0x811c_9dc5u32;
    for v in [a, b] {
        for byte in v.to_le_bytes() {
            h ^= byte as u32;
            h = h.wrapping_mul(0x0100_0193);
        }
    }
    h
}

fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c == '\n' || c == '\r' || c == '\t' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

fn unique_id(landmark: &str, condition: Condition, seen: &mut Vec<(String, u32)>) -> (String, String) {
    let base = format!("{}:{}", condition.key(), sanitize(landmark));
    let count = if let Some((_, n)) = seen.iter_mut().find(|(s, _)| s == &base) {
        *n += 1;
        *n
    } else {
        seen.push((base.clone(), 1));
        1
    };
    if count == 1 {
        (base, landmark.to_string())
    } else {
        (format!("{base}#{count}"), format!("{landmark} ({count})"))
    }
}

pub(super) fn close_distance(radius: f32) -> f32 {
    (radius.max(0.5) * 6.5).clamp(14.0, 140.0)
}

pub(super) fn far_distance(radius: f32) -> f32 {
    let close = close_distance(radius);
    let raw = (radius.max(0.5) * 18.0).max(close * 2.2);
    raw.clamp(close * 2.2, 640.0)
}

pub(super) fn sweet_distance(cond: Condition, radius: f32) -> f32 {
    let close = close_distance(radius);
    let far = far_distance(radius);
    match cond {
        Condition::CloseUp => close * 0.62,
        Condition::FarAway => far * 1.25,
        _ => (close + far) * 0.5,
    }
}

/// Projected radius in pixels for the renderer's vertical field of view.
pub(super) fn projected_radius_px(radius: f32, distance: f32, screen_h: f32) -> f32 {
    let dist = distance.max(0.05);
    let half = (VERTICAL_FOV_RAD * 0.5).tan();
    (radius.max(0.0) / dist) / half * (screen_h.max(1.0) * 0.5)
}

pub(super) fn score_target(target: &Target, view: ViewSample) -> Score {
    let visible = match view.screen_xy {
        Some((x, y)) => {
            view.screen_w > 1.0
                && view.screen_h > 1.0
                && x >= 0.0
                && y >= 0.0
                && x <= view.screen_w
                && y <= view.screen_h
        }
        None => false,
    };
    let centred = if visible {
        let (x, y) = view.screen_xy.unwrap();
        centredness(x, y, view.screen_w, view.screen_h)
    } else {
        0.0
    };
    let min_side = view.screen_w.min(view.screen_h).max(1.0);
    let actual = (view.radius_px.max(0.0) * 2.0) / min_side;
    let ideal = ideal_fraction(target, view.screen_w, view.screen_h);
    let size = if visible { size_match(actual, ideal) } else { 0.0 };
    let condition_met = condition_met(target.condition, target.radius, &view);
    if !visible || !condition_met {
        return Score { stars: 0, score: 0.0, visible, centred, size, condition_met };
    }
    let score = CENTRE_WEIGHT * centred + SIZE_WEIGHT * size;
    Score { stars: stars_from_score(score), score, visible, centred, size, condition_met }
}

fn centredness(x: f32, y: f32, w: f32, h: f32) -> f32 {
    let nx = (x / w - 0.5).abs() * 2.0;
    let ny = (y / h - 0.5).abs() * 2.0;
    let corner = nx.hypot(ny) / SQRT_2;
    (1.0 - corner).clamp(0.0, 1.0)
}

fn ideal_fraction(target: &Target, screen_w: f32, screen_h: f32) -> f32 {
    let dist = sweet_distance(target.condition, target.radius).max(0.05);
    let radius_px = projected_radius_px(target.radius, dist, screen_h);
    let min_side = screen_w.min(screen_h).max(1.0);
    (radius_px * 2.0) / min_side
}

fn size_match(actual: f32, ideal: f32) -> f32 {
    if ideal < 1.0e-4 {
        return 1.0;
    }
    let ratio = (actual.max(0.0) / ideal).max(1.0e-3);
    let log = ratio.ln().abs();
    (1.0 - log / 1.1).clamp(0.0, 1.0)
}

fn stars_from_score(score: f32) -> u8 {
    if score >= STAR_3 {
        3
    } else if score >= STAR_2 {
        2
    } else if score >= STAR_1 {
        1
    } else {
        0
    }
}

fn condition_met(cond: Condition, radius: f32, view: &ViewSample) -> bool {
    match cond {
        Condition::Night => day_phase(view.hour) == Phase::Night,
        Condition::GoldenHour => day_phase(view.hour) == Phase::Golden,
        Condition::Daylight => day_phase(view.hour) == Phase::Day,
        Condition::Rain => matches!(view.weather, WeatherKind::Rain | WeatherKind::Storm),
        Condition::Storm => view.weather == WeatherKind::Storm,
        Condition::Snow => view.weather == WeatherKind::Snow,
        Condition::CloseUp => view.distance <= close_distance(radius),
        Condition::FarAway => view.distance >= far_distance(radius),
    }
}

impl Album {
    pub(super) fn is_found(&self, id: &str) -> bool {
        self.stars_of(id) >= 1
    }

    pub(super) fn stars_of(&self, id: &str) -> u8 {
        self.records.iter().find(|r| r.id == id).map(|r| r.stars).unwrap_or(0)
    }

    pub(super) fn score_of(&self, id: &str) -> f32 {
        self.records.iter().find(|r| r.id == id).map(|r| r.score).unwrap_or(0.0)
    }

    pub(super) fn found_count(&self, targets: &[Target]) -> usize {
        targets.iter().filter(|t| self.is_found(&t.id)).count()
    }

    /// Keep this shot when it beats the stored one. Returns true on a new best.
    pub(super) fn record(&mut self, id: &str, stars: u8, score: f32) -> bool {
        let stars = stars.min(3);
        if stars == 0 || id.is_empty() {
            return false;
        }
        let score = millis_to_score(quantize_millis(score));
        if let Some(rec) = self.records.iter_mut().find(|r| r.id == id) {
            let better = stars > rec.stars || (stars == rec.stars && score > rec.score);
            if better {
                rec.stars = stars;
                rec.score = score;
                true
            } else {
                false
            }
        } else {
            self.records.push(AlbumRecord { id: id.to_string(), stars, score });
            self.records.sort_by(|a, b| a.id.cmp(&b.id));
            true
        }
    }
}

fn quantize_millis(score: f32) -> u16 {
    if !score.is_finite() {
        return 0;
    }
    (score.clamp(0.0, 1.0) * 1000.0).round().min(1000.0) as u16
}

fn millis_to_score(millis: u16) -> f32 {
    (millis.min(1000) as f32) / 1000.0
}

/// Line format: `v1` then `id<TAB>stars<TAB>millis` (score × 1000). Sorted by id.
pub(super) fn serialize_album(album: &Album) -> String {
    let mut lines = vec!["v1".to_string()];
    let mut records = album.records.clone();
    records.sort_by(|a, b| a.id.cmp(&b.id));
    for rec in records {
        let id = sanitize(&rec.id);
        if id.is_empty() {
            continue;
        }
        lines.push(format!("{id}\t{}\t{}", rec.stars.min(3), quantize_millis(rec.score)));
    }
    lines.join("\n")
}

pub(super) fn deserialize_album(text: &str) -> Album {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let Some(version) = lines.next() else {
        return Album::default();
    };
    if version != "v1" {
        return Album::default();
    }
    let mut album = Album::default();
    for line in lines {
        let mut parts = line.split('\t');
        let Some(id) = parts.next() else { continue };
        let Some(stars) = parts.next().and_then(|s| s.parse::<u8>().ok()) else { continue };
        let Some(millis) = parts.next().and_then(|s| s.parse::<u16>().ok()) else { continue };
        if id.is_empty() || id.contains('\n') {
            continue;
        }
        let stars = stars.min(3);
        if stars == 0 {
            continue;
        }
        let score = millis_to_score(millis);
        // Last (or better) line wins if a file repeats an id.
        if album.records.iter().any(|r| r.id == id) {
            album.record(id, stars, score);
        } else {
            album.records.push(AlbumRecord { id: id.to_string(), stars, score });
        }
    }
    album.records.sort_by(|a, b| a.id.cmp(&b.id));
    album
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::atmosphere::{Atmosphere, DayClock};

    fn place(name: &str, radius: f32) -> LandmarkIn {
        LandmarkIn { name: name.to_string(), pos: [3.0, 4.0, 5.0], radius }
    }

    fn places(n: usize) -> Vec<LandmarkIn> {
        (0..n)
            .map(|i| LandmarkIn {
                name: format!("Place {i}"),
                pos: [i as f32, 2.0, -1.0],
                radius: 8.0 + i as f32,
            })
            .collect()
    }

    fn has(targets: &[Target], cond: Condition) -> bool {
        targets.iter().any(|t| t.condition == cond)
    }

    #[test]
    fn empty_scene_has_no_targets() {
        assert!(generate_targets(&[], 0).is_empty());
    }

    #[test]
    fn targets_are_deterministic_and_in_range() {
        for n in [1_usize, 2, 5, 12] {
            for seed in [0_u32, 1, 7, 99] {
                let a = generate_targets(&places(n), seed);
                let b = generate_targets(&places(n), seed);
                assert_eq!(a.len(), b.len());
                assert!(a.iter().map(|t| t.id.as_str()).eq(b.iter().map(|t| t.id.as_str())));
                assert!((6..=10).contains(&a.len()), "n={n} seed={seed} count={}", a.len());
                let mut ids: Vec<_> = a.iter().map(|t| t.id.clone()).collect();
                let before = ids.clone();
                ids.sort();
                ids.dedup();
                assert_eq!(ids.len(), before.len(), "duplicate ids");
                assert!(has(&a, Condition::Night));
                assert!(has(&a, Condition::GoldenHour));
                assert!(has(&a, Condition::Rain));
                assert!(has(&a, Condition::CloseUp));
                assert!(has(&a, Condition::FarAway));
            }
        }
    }

    #[test]
    fn seed_changes_the_list_when_there_are_several_landmarks() {
        let lms = places(4);
        let base: Vec<_> = generate_targets(&lms, 0).into_iter().map(|t| t.id).collect();
        let differed = (1..40_u32).any(|seed| {
            let ids: Vec<_> = generate_targets(&lms, seed).into_iter().map(|t| t.id).collect();
            ids != base
        });
        assert!(differed);
    }

    #[test]
    fn titles_name_the_landmark_and_the_condition() {
        let targets = generate_targets(&[place("Spirit Tree", 14.0)], 0);
        let night = targets.iter().find(|t| t.condition == Condition::Night).unwrap();
        assert_eq!(night.title(), "Spirit Tree at night");
        assert_eq!(night.condition.hint(), "Try visiting at night");
        assert_eq!(night.pos, [3.0, 4.0, 5.0]);
        let close = targets.iter().find(|t| t.condition == Condition::CloseUp).unwrap();
        assert_eq!(close.title(), "Spirit Tree close up");
        assert_eq!(close.condition.hint(), "Get closer");
    }

    #[test]
    fn duplicate_landmark_names_get_distinct_ids() {
        let lms = vec![place("Bridge", 10.0), place("Bridge", 12.0)];
        let targets = generate_targets(&lms, 3);
        let mut ids: Vec<_> = targets.iter().map(|t| t.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), targets.len());
    }

    fn framed(target: &Target, hour: f32, weather: WeatherKind, distance: f32, xy: Option<(f32, f32)>) -> ViewSample {
        let (w, h) = (1280.0, 720.0);
        let radius_px = projected_radius_px(target.radius, distance, h);
        ViewSample { screen_xy: xy, screen_w: w, screen_h: h, distance, radius_px, hour, weather }
    }

    fn perfect(target: &Target, hour: f32, weather: WeatherKind) -> ViewSample {
        let dist = sweet_distance(target.condition, target.radius);
        framed(target, hour, weather, dist, Some((640.0, 360.0)))
    }

    #[test]
    fn perfect_shots_score_three_stars() {
        let targets = generate_targets(&[place("Observatory", 10.0)], 1);
        let night = targets.iter().find(|t| t.condition == Condition::Night).unwrap();
        let score = score_target(night, perfect(night, 0.0, WeatherKind::Clear));
        assert!(score.visible && score.condition_met);
        assert_eq!(score.stars, 3);
        assert!(score.score >= STAR_3);

        let close = targets.iter().find(|t| t.condition == Condition::CloseUp).unwrap();
        let score = score_target(close, perfect(close, 12.0, WeatherKind::Clear));
        assert!(score.condition_met);
        assert_eq!(score.stars, 3);
        assert!(close_distance(close.radius) >= sweet_distance(Condition::CloseUp, close.radius));

        let far = targets.iter().find(|t| t.condition == Condition::FarAway).unwrap();
        let score = score_target(far, perfect(far, 12.0, WeatherKind::Clear));
        assert!(score.condition_met);
        assert_eq!(score.stars, 3);

        let rain = targets.iter().find(|t| t.condition == Condition::Rain).unwrap();
        assert_eq!(score_target(rain, perfect(rain, 12.0, WeatherKind::Rain)).stars, 3);
        assert_eq!(score_target(rain, perfect(rain, 23.0, WeatherKind::Storm)).stars, 3);
        assert_eq!(score_target(rain, perfect(rain, 12.0, WeatherKind::Clear)).stars, 0);
    }

    #[test]
    fn framing_and_conditions_change_the_stars() {
        let targets = generate_targets(&[place("Pagoda", 10.0)], 2);
        let night = targets.iter().find(|t| t.condition == Condition::Night).unwrap();
        let dist = sweet_distance(night.condition, night.radius);

        let corner = score_target(night, framed(night, 0.0, WeatherKind::Clear, dist, Some((0.0, 0.0))));
        assert_eq!(corner.stars, 1);
        assert!(corner.centred < 0.05);

        let edge = score_target(night, framed(night, 0.0, WeatherKind::Clear, dist, Some((0.0, 360.0))));
        assert_eq!(edge.stars, 2);

        let wrong_time = score_target(night, perfect(night, 12.0, WeatherKind::Clear));
        assert!(wrong_time.visible);
        assert!(!wrong_time.condition_met);
        assert_eq!(wrong_time.stars, 0);
        assert_eq!(wrong_time.score, 0.0);

        let hidden = score_target(night, framed(night, 0.0, WeatherKind::Clear, dist, None));
        assert!(!hidden.visible);
        assert_eq!(hidden.stars, 0);

        let off = score_target(night, framed(night, 0.0, WeatherKind::Clear, dist, Some((-4.0, 10.0))));
        assert!(!off.visible);
        assert_eq!(off.stars, 0);
    }

    #[test]
    fn close_and_far_use_distance_gates() {
        let radius = 10.0;
        let close = close_distance(radius);
        let far = far_distance(radius);
        assert!(far > close * 2.0);
        let targets = generate_targets(&[place("Gate", radius)], 4);
        let close_t = targets.iter().find(|t| t.condition == Condition::CloseUp).unwrap();
        let far_t = targets.iter().find(|t| t.condition == Condition::FarAway).unwrap();
        let at = |t: &Target, d: f32| framed(t, 12.0, WeatherKind::Clear, d, Some((640.0, 360.0)));
        assert!(score_target(close_t, at(close_t, close)).condition_met);
        assert!(!score_target(close_t, at(close_t, close + 0.05)).condition_met);
        assert!(score_target(far_t, at(far_t, far)).condition_met);
        assert!(!score_target(far_t, at(far_t, far - 0.05)).condition_met);
        // A point cannot satisfy both gates.
        assert!(!(close >= far));
    }

    #[test]
    fn day_phases_match_preset_hours_and_boundaries() {
        assert_eq!(day_phase(0.0), Phase::Night);
        assert_eq!(day_phase(5.499), Phase::Night);
        assert_eq!(day_phase(5.5), Phase::Golden);
        assert_eq!(day_phase(8.499), Phase::Golden);
        assert_eq!(day_phase(8.5), Phase::Day);
        assert_eq!(day_phase(12.0), Phase::Day);
        assert_eq!(day_phase(15.999), Phase::Day);
        assert_eq!(day_phase(16.0), Phase::Golden);
        assert_eq!(day_phase(18.0), Phase::Golden);
        assert_eq!(day_phase(19.499), Phase::Golden);
        assert_eq!(day_phase(19.5), Phase::Night);
        assert_eq!(day_phase(24.0), Phase::Night);
        assert_eq!(day_phase(-1.0), Phase::Night);

        let mut atmo = Atmosphere::default();
        atmo.clock = DayClock::Preset(0);
        assert_eq!(day_phase(atmo.hour()), Phase::Day);
        atmo.clock = DayClock::Preset(1);
        assert_eq!(day_phase(atmo.hour()), Phase::Golden);
        atmo.clock = DayClock::Preset(2);
        assert_eq!(day_phase(atmo.hour()), Phase::Night);
    }

    fn manual(cond: Condition) -> Target {
        Target { id: format!("t-{}", cond.key()), landmark: "Harbor".into(), condition: cond, pos: [0.0, 1.0, 0.0], radius: 12.0 }
    }

    #[test]
    fn storm_and_snow_are_specific() {
        let storm = manual(Condition::Storm);
        let snow = manual(Condition::Snow);
        assert_eq!(score_target(&storm, perfect(&storm, 12.0, WeatherKind::Storm)).stars, 3);
        assert_eq!(score_target(&storm, perfect(&storm, 12.0, WeatherKind::Rain)).stars, 0);
        assert_eq!(score_target(&snow, perfect(&snow, 12.0, WeatherKind::Snow)).stars, 3);
        assert_eq!(score_target(&snow, perfect(&snow, 12.0, WeatherKind::Rain)).stars, 0);
    }

    #[test]
    fn album_round_trip_and_best_score() {
        let mut album = Album::default();
        assert!(!album.record("night:Gate", 0, 0.9));
        assert!(album.record("night:Gate", 1, 0.4));
        assert!(!album.record("night:Gate", 1, 0.2));
        assert_eq!(album.stars_of("night:Gate"), 1);
        assert!(album.record("night:Gate", 2, 0.61));
        assert!(!album.record("night:Gate", 2, 0.5));
        assert!(album.record("day:Gate", 3, 0.9));
        assert!(album.record("close:Gate", 1, 0.25));
        assert_eq!(album.records.len(), 3);

        let text = serialize_album(&album);
        assert!(text.starts_with("v1\n"));
        let loaded = deserialize_album(&text);
        assert_eq!(loaded, album);
        assert_eq!(loaded.stars_of("night:Gate"), 2);
        assert!((loaded.score_of("night:Gate") - 0.61).abs() < 1.0e-6);
        assert!(loaded.is_found("day:Gate"));
        assert!(!loaded.is_found("missing"));

        let again = deserialize_album(&serialize_album(&loaded));
        assert_eq!(again, loaded);
    }

    #[test]
    fn album_rejects_garbage_and_keeps_the_better_duplicate() {
        assert!(deserialize_album("").records.is_empty());
        assert!(deserialize_album("nope").records.is_empty());
        assert!(deserialize_album("v2\nnight:Gate\t3\t900").records.is_empty());
        let messy = "v1\n\nbad line\nnight:Gate\t2\tnot-a-number\nnight:Gate\t1\t400\nnight:Gate\t3\t800\n\t2\t100\n";
        let album = deserialize_album(messy);
        assert_eq!(album.records.len(), 1);
        assert_eq!(album.stars_of("night:Gate"), 3);
        assert!((album.score_of("night:Gate") - 0.8).abs() < 1.0e-6);
    }

    #[test]
    fn storage_key_is_per_scene() {
        assert_eq!(storage_key("sky-islands"), "photo.album.sky-islands");
    }

    #[test]
    fn found_count_matches_recorded_targets() {
        let targets = generate_targets(&places(3), 0);
        let mut album = Album::default();
        assert_eq!(album.found_count(&targets), 0);
        album.record(&targets[0].id, 2, 0.55);
        album.record(&targets[1].id, 3, 0.9);
        assert_eq!(album.found_count(&targets), 2);
    }
}
