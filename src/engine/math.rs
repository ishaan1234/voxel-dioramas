use glam::Vec3;

pub struct Rng(pub u32);

impl Rng {
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6D2B79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f32) / 4294967296.0
    }
}

#[inline]
pub fn hash2(x: f32, z: f32) -> f32 {
    let n = (x * 127.1 + z * 311.7).sin() * 43758.5453123;
    n - n.floor()
}

#[inline]
pub fn noise2(x: f32, z: f32) -> f32 {
    let ix = x.floor();
    let iz = z.floor();
    let fx = x - ix;
    let fz = z - iz;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);
    let a = hash2(ix, iz);
    let b = hash2(ix + 1.0, iz);
    let c = hash2(ix, iz + 1.0);
    let d = hash2(ix + 1.0, iz + 1.0);
    a + (b - a) * ux + (c - a) * uz + (a - b - c + d) * ux * uz
}

pub fn fbm2(x: f32, z: f32, oct: i32) -> f32 {
    let mut v = 0.0;
    let mut a = 0.5;
    let mut f = 1.0;
    for _ in 0..oct {
        v += a * noise2(x * f, z * f);
        f *= 2.03;
        a *= 0.5;
    }
    v
}

pub fn ridged2(x: f32, z: f32, oct: i32) -> f32 {
    let mut v = 0.0;
    let mut a = 0.5;
    let mut f = 1.0;
    for _ in 0..oct {
        let n = 1.0 - (noise2(x * f, z * f) * 2.0 - 1.0).abs();
        v += a * n * n;
        f *= 2.07;
        a *= 0.5;
    }
    v
}

pub struct CatmullRomCurve3 {
    points: Vec<Vec3>,
    closed: bool,
    tension: f32,
    arc_lengths: Vec<f32>,
}

impl CatmullRomCurve3 {
    pub fn new(points: Vec<Vec3>, closed: bool, tension: f32) -> Self {
        let mut c = Self {
            points,
            closed,
            tension,
            arc_lengths: Vec::new(),
        };
        c.compute_arc_lengths(200);
        c
    }

    pub fn get_point(&self, t: f32) -> Vec3 {
        let l = self.points.len() as i32;
        let segs = if self.closed { l } else { l - 1 };
        let p = segs as f32 * t;
        let mut int_pt = p.floor() as i32;
        let mut w = p - int_pt as f32;

        if self.closed {
            int_pt = int_pt.rem_euclid(l);
        } else if w == 0.0 && int_pt >= l - 1 {
            int_pt = l - 2;
            w = 1.0;
        } else if int_pt < 0 {
            int_pt = 0;
            w = 0.0;
        } else if int_pt >= l - 1 {
            int_pt = l - 2;
            w = 1.0;
        }

        let idx = |i: i32| -> usize { i.rem_euclid(l) as usize };

        let p0 = if self.closed || int_pt > 0 {
            self.points[idx(int_pt - 1)]
        } else {
            2.0 * self.points[0] - self.points[1]
        };
        let p1 = self.points[idx(int_pt)];
        let p2 = self.points[idx(int_pt + 1)];
        let p3 = if self.closed || int_pt + 2 < l {
            self.points[idx(int_pt + 2)]
        } else {
            2.0 * self.points[(l - 1) as usize] - self.points[(l - 2) as usize]
        };

        let v0 = (p2 - p0) * self.tension;
        let v1 = (p3 - p1) * self.tension;
        let c0 = p1;
        let c1 = v0;
        let c2 = -3.0 * p1 + 3.0 * p2 - 2.0 * v0 - v1;
        let c3 = 2.0 * p1 - 2.0 * p2 + v0 + v1;
        ((c3 * w + c2) * w + c1) * w + c0
    }

    fn compute_arc_lengths(&mut self, divisions: usize) {
        self.arc_lengths.clear();
        self.arc_lengths.reserve(divisions + 1);
        self.arc_lengths.push(0.0);
        let mut last = self.get_point(0.0);
        let mut sum = 0.0;
        for p in 1..=divisions {
            let cur = self.get_point(p as f32 / divisions as f32);
            sum += cur.distance(last);
            self.arc_lengths.push(sum);
            last = cur;
        }
    }

    pub fn u_to_t(&self, u: f32) -> f32 {
        let u = u.clamp(0.0, 1.0);
        let total = *self.arc_lengths.last().unwrap_or(&1.0);
        let target = u * total;
        let il = self.arc_lengths.len() - 1;
        let mut low = 0usize;
        let mut high = il;
        while low <= high {
            let mid = (low + high) / 2;
            let diff = self.arc_lengths[mid] - target;
            if diff < 0.0 {
                low = mid + 1;
            } else if diff > 0.0 {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            } else {
                high = mid;
                break;
            }
        }
        let i = high.min(il - 1);
        let len_before = self.arc_lengths[i];
        let len_after = self.arc_lengths[i + 1];
        let seg_len = (len_after - len_before).max(1e-6);
        let frac = (target - len_before) / seg_len;
        (i as f32 + frac) / il as f32
    }

    pub fn get_point_at(&self, u: f32) -> Vec3 {
        self.get_point(self.u_to_t(u))
    }

    pub fn get_tangent_at(&self, u: f32) -> Vec3 {
        let t = self.u_to_t(u);
        let delta = 0.0001;
        let t1 = (t - delta).max(0.0);
        let t2 = (t + delta).min(1.0);
        (self.get_point(t2) - self.get_point(t1)).normalize_or_zero()
    }
}

// ---------------------------------------------------------------------------
// 3D value noise (used by SDF scenes for volumetric displacement / carving)
// ---------------------------------------------------------------------------

#[inline]
pub fn hash3(x: f32, y: f32, z: f32) -> f32 {
    let n = (x * 127.1 + y * 269.5 + z * 311.7).sin() * 43758.5453123;
    n - n.floor()
}

pub fn noise3(p: Vec3) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec3::splat(3.0) - 2.0 * f);
    let h = |dx: f32, dy: f32, dz: f32| hash3(i.x + dx, i.y + dy, i.z + dz);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(h(0.0, 0.0, 0.0), h(1.0, 0.0, 0.0), u.x);
    let x10 = lerp(h(0.0, 1.0, 0.0), h(1.0, 1.0, 0.0), u.x);
    let x01 = lerp(h(0.0, 0.0, 1.0), h(1.0, 0.0, 1.0), u.x);
    let x11 = lerp(h(0.0, 1.0, 1.0), h(1.0, 1.0, 1.0), u.x);
    lerp(lerp(x00, x10, u.y), lerp(x01, x11, u.y), u.z)
}

pub fn fbm3(p: Vec3, oct: i32) -> f32 {
    let mut v = 0.0;
    let mut a = 0.5;
    let mut q = p;
    for _ in 0..oct {
        v += a * noise3(q);
        q = q * 2.03 + Vec3::new(17.1, 3.7, 9.2);
        a *= 0.5;
    }
    v
}

/// Golden-angle (Vogel) spiral: evenly spread points on a disc without a grid.
pub fn golden_spiral(i: usize, n: usize, radius: f32) -> (f32, f32) {
    let golden = std::f32::consts::PI * (3.0 - 5.0f32.sqrt());
    let r = radius * ((i as f32 + 0.5) / n as f32).sqrt();
    let a = i as f32 * golden;
    (r * a.cos(), r * a.sin())
}

/// Dart-throwing Poisson-disc sampling inside a disc of `radius`.
/// `accept` lets the caller reject points (e.g. off-island, on a building pad).
pub fn poisson_disc<F: FnMut(f32, f32) -> bool>(
    rng: &mut Rng,
    radius: f32,
    min_dist: f32,
    attempts: usize,
    mut accept: F,
) -> Vec<(f32, f32)> {
    let mut pts: Vec<(f32, f32)> = Vec::new();
    let md2 = min_dist * min_dist;
    for _ in 0..attempts {
        let a = rng.next_f32() * std::f32::consts::TAU;
        let r = radius * rng.next_f32().sqrt();
        let (x, z) = (a.cos() * r, a.sin() * r);
        if pts.iter().any(|&(px, pz)| (px - x).powi(2) + (pz - z).powi(2) < md2) {
            continue;
        }
        if accept(x, z) {
            pts.push((x, z));
        }
    }
    pts
}
