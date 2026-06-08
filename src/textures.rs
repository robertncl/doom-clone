//! Procedurally generated wall/floor/ceiling textures.
//!
//! Each material's macro layout (brick courses, metal panels, floor tiles, …)
//! is authored against a fixed 64-texel "design" grid and mapped onto the real
//! `TEX_SIZE` grid through a scale factor `s = TEX_SIZE / 64`. Bumping the
//! texture resolution therefore keeps the same proportions but yields crisper
//! edges and finer grain under the bilinear filter. Low-frequency colour
//! variation comes from smoothed value-noise / fbm (organic mottling) layered
//! over the fine `hash2` grain, so surfaces read as weathered rather than as
//! uniform TV-static noise.

use crate::color::{hash2, make_color};
use crate::constants::*;

pub struct Textures {
    /// One texture per wall kind. Index 0 (`WALL_NONE`) is unused but kept so
    /// `wall[wall_type]` indexes directly.
    pub wall: Vec<Vec<u32>>,
    pub floor: Vec<u32>,
    pub ceil: Vec<u32>,
}

/// Smoothed value noise with period `cell` texels: bilinear-interpolate (with a
/// smoothstep falloff) between hashed lattice points. Returns 0..=255. Raw
/// `hash2` is white noise; this varies gently, so it reads as natural mottling.
fn vnoise(x: i32, y: i32, cell: i32) -> f64 {
    let gx = x.div_euclid(cell);
    let gy = y.div_euclid(cell);
    let fx = (x - gx * cell) as f64 / cell as f64;
    let fy = (y - gy * cell) as f64 / cell as f64;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let h = |i: i32, j: i32| hash2(i, j) as f64;
    let top = h(gx, gy) + (h(gx + 1, gy) - h(gx, gy)) * sx;
    let bot = h(gx, gy + 1) + (h(gx + 1, gy + 1) - h(gx, gy + 1)) * sx;
    top + (bot - top) * sy
}

/// Two-octave fbm centred near 0 (≈ -160..=160): a broad swell plus a
/// half-amplitude finer octave. `cell` is the coarse period in texels. Used only
/// at build time, so the extra octaves cost nothing per frame.
fn fbm(x: i32, y: i32, cell: i32) -> f64 {
    (vnoise(x, y, cell) - 128.0) + (vnoise(x + 99, y + 33, (cell / 2).max(2)) - 128.0) * 0.5
}

impl Textures {
    pub fn build() -> Textures {
        let n = TEX_SIZE * TEX_SIZE;
        let mut wall = vec![vec![0u32; n]; WALL_KIND_MAX];
        let mut floor = vec![0u32; n];
        let mut ceil = vec![0u32; n];
        let s = TEX_SIZE as i32 / 64; // resolution scale: 1 at 64, 2 at 128

        for v in 0..TEX_SIZE as i32 {
            for u in 0..TEX_SIZE as i32 {
                let idx = (v as usize) * TEX_SIZE + u as usize;

                // ---- Stone: per-block tint, fbm mottling, cracks, moss ----
                let n_ = hash2(u, v) - 128;
                let block_tint = (hash2(u / (16 * s) + 1, v / (16 * s) + 7) - 128) / 6;
                let mottle = (fbm(u, v, 18 * s) / 8.0) as i32;
                let crack1 = if (u + v * 2) % (19 * s) == 0 { -50 } else { 0 };
                let crack2 = if (u * 2 - v + 64 * s) % (23 * s) == 0 { -40 } else { 0 };
                let crack3 = if (u - 28 * s).abs() + (v * 2 - 30 * s).abs() < 4 * s { -35 } else { 0 };
                let moss_seed = hash2(u / (4 * s), v / (4 * s));
                let moss_local = hash2(u, v);
                let moss = moss_seed > 230 && moss_local > 160;
                let (s_r, s_g, s_b);
                if moss {
                    let mn2 = hash2(u, v) - 128;
                    s_r = 50 + mn2 / 6;
                    s_g = 110 + mn2 / 4;
                    s_b = 50 + mn2 / 6;
                } else {
                    let delta = crack1 + crack2 + crack3 + block_tint + n_ / 5 + mottle;
                    s_r = 130 + delta;
                    s_g = 125 + delta - 2;
                    s_b = 115 + delta - 10;
                }
                let block_shadow = if (u % (16 * s) < s || v % (16 * s) < s) && !moss { -15 } else { 0 };
                wall[WALL_STONE][idx] =
                    make_color(s_r + block_shadow, s_g + block_shadow, s_b + block_shadow);

                // ---- Brick: bevels (top/left highlight, bottom/right shadow) ----
                let row = v / (8 * s);
                let col_off = if row & 1 != 0 { 8 * s } else { 0 };
                let bu = (u + col_off) % (16 * s) / s; // design column 0..15
                let bv = v % (8 * s) / s; // design row 0..7
                let mortar = bv == 0 || bv == 7 || bu == 0 || bu == 15;
                let hi_edge = bv == 1 || bu == 1;
                let lo_edge = bv == 6 || bu == 14;
                let bn = hash2(u / (2 * s) + (row & 1) * 13, v / (2 * s)) - 128;
                let brick_mottle = (fbm(u + 7, v + 3, 16 * s) / 10.0) as i32;
                let (mut br_r, mut br_g, mut br_b);
                if mortar {
                    br_r = 60;
                    br_g = 52;
                    br_b = 48;
                } else {
                    br_r = 150 + bn / 4 + brick_mottle;
                    br_g = 65 + bn / 8 + brick_mottle / 2;
                    br_b = 50 + bn / 8 + brick_mottle / 2;
                    if hi_edge {
                        br_r += 25;
                        br_g += 15;
                        br_b += 8;
                    }
                    if lo_edge {
                        br_r -= 30;
                        br_g -= 20;
                        br_b -= 15;
                    }
                }
                wall[WALL_BRICK][idx] = make_color(br_r, br_g, br_b);

                // ---- Metal: bevels, rivets, scratches and small rust spots ----
                let pu = u % (32 * s) / s; // design panel coord 0..31
                let pv = v % (32 * s) / s;
                let bevel = pu < 2 || pu > 29 || pv < 2 || pv > 29;
                let hi = pu < 1 || pv < 1;
                let rivet = (pu == 5 || pu == 26) && (pv == 5 || pv == 26);
                let rivet_hi = pu == 5 && pv == 5;
                let mn = hash2(u, v) - 128;
                let scratch = if (u * 3 + v) % (31 * s) == 0 && v % (8 * s) / s > 1 { 30 } else { 0 };
                let rust = hash2(u / (3 * s) + 5, v / (3 * s) + 11) > 235;
                let (m_r, m_g, m_b);
                if rivet_hi {
                    m_r = 230;
                    m_g = 230;
                    m_b = 235;
                } else if rivet {
                    m_r = 200;
                    m_g = 200;
                    m_b = 210;
                } else if bevel && hi {
                    m_r = 150;
                    m_g = 160;
                    m_b = 190;
                } else if bevel {
                    m_r = 38;
                    m_g = 46;
                    m_b = 66;
                } else if rust {
                    m_r = 130 + mn / 8;
                    m_g = 70 + mn / 10;
                    m_b = 35 + mn / 12;
                } else {
                    m_r = 80 + mn / 6 + scratch;
                    m_g = 95 + mn / 6 + scratch;
                    m_b = 130 + mn / 6 + scratch;
                }
                wall[WALL_METAL][idx] = make_color(m_r, m_g, m_b);

                // ---- Wood: vertical plank divisions, sin grain, knots ----
                let wn = hash2(u / (4 * s), v / s) - 128;
                let pl = u % (16 * s) / s; // design plank coord 0..15
                let plank_seam = if pl == 0 || pl == 15 { -40 } else { 0 };
                let plank_hi = if pl == 1 { 15 } else { 0 };
                let grain = (18.0 * (u as f64 * 0.42 / s as f64 + wn as f64 * 0.05).sin()) as i32;
                let band = if v % (22 * s) / s < 2 { -35 } else { 0 };
                let knot1 = if (u - 22 * s).pow(2) + (v - 30 * s).pow(2) < 10 * s * s { -45 } else { 0 };
                let knot2 = if (u - 8 * s).pow(2) + (v - 50 * s).pow(2) < 7 * s * s { -40 } else { 0 };
                let w_r = 115 + grain + wn / 8 + band + knot1 + knot2 + plank_seam + plank_hi;
                let w_g = 70 + grain / 2 + wn / 10 + band + knot1 + knot2 + plank_seam;
                let w_b = 30 + wn / 12 + band + knot1 + knot2 + plank_seam;
                wall[WALL_WOOD][idx] = make_color(w_r, w_g, w_b);

                // ---- Hell rock: dark base with glowing red veins + lava spots ----
                let hn = hash2(u, v) - 128;
                let hn2 = hash2(u + 50 * s, v + 30 * s) - 128;
                let hmottle = (fbm(u + 31, v + 17, 16 * s) / 7.0) as i32;
                let vein_u = (u + hash2(u / (6 * s), v / (6 * s)) % (6 * s)) % (22 * s);
                let vein_v = (v + hash2(v / (6 * s), u / (6 * s)) % (6 * s)) % (18 * s);
                let mut vein = vein_u < 2 * s || vein_v < 2 * s;
                if hash2(u / (5 * s) + 3, v / (5 * s) + 9) > 195 {
                    vein = false;
                }
                let lava = hash2(u / (4 * s), v / (4 * s)) > 248;
                let (mut h_r, mut h_g, mut h_b);
                if vein {
                    let glow = 200 + hn / 6;
                    h_r = glow;
                    h_g = 40 + hn2 / 8;
                    h_b = 20 + hn2 / 10;
                } else if lava {
                    h_r = 230;
                    h_g = 140 + hn / 8;
                    h_b = 30;
                } else {
                    h_r = 60 + hn / 4 + hmottle;
                    h_g = 22 + hn2 / 8 + hmottle / 3;
                    h_b = 22 + hn / 10 + hmottle / 3;
                    if (u * u + v * v) % (17 * s * s) == 0 {
                        h_r -= 20;
                        h_g -= 8;
                        h_b -= 8;
                    }
                }
                wall[WALL_HELL][idx] = make_color(h_r, h_g, h_b);

                // ---- Floor tile: bevels, scuffs, cracked tile patches ----
                let tu = u % (16 * s) / s; // design tile coord 0..15
                let tv = v % (16 * s) / s;
                let grout = tu == 0 || tv == 0 || tu == 15 || tv == 15;
                let tile_bevel_hi = tu == 1 || tv == 1;
                let tile_bevel_lo = tu == 14 || tv == 14;
                let fn_ = hash2(u, v) - 128;
                let floor_mottle = (fbm(u + 5, v + 9, 20 * s) / 9.0) as i32;
                let tile_seed = hash2(u / (16 * s), v / (16 * s));
                let cracked_tile = tile_seed > 230;
                let (mut f_r, mut f_g, mut f_b);
                if grout {
                    f_r = 25;
                    f_g = 25;
                    f_b = 30;
                } else if cracked_tile && (u + v * 2) % (9 * s) == 0 {
                    f_r = 35;
                    f_g = 35;
                    f_b = 35;
                } else {
                    f_r = 75 + fn_ / 6 + floor_mottle;
                    f_g = 70 + fn_ / 6 + floor_mottle;
                    f_b = 60 + fn_ / 8 + floor_mottle;
                    if cracked_tile {
                        f_r -= 15;
                        f_g -= 12;
                        f_b -= 10;
                    }
                    if tile_bevel_hi {
                        f_r += 12;
                        f_g += 12;
                        f_b += 10;
                    }
                    if tile_bevel_lo {
                        f_r -= 12;
                        f_g -= 12;
                        f_b -= 10;
                    }
                }
                if (u + v) % (23 * s) == 0 && !grout {
                    f_r += 8;
                    f_g += 8;
                    f_b += 8;
                }
                floor[idx] = make_color(f_r, f_g, f_b);

                // ---- Ceiling: noise + occasional support-beam pattern ----
                let cn = hash2(u + 17, v + 31) - 128;
                let ceil_mottle = (fbm(u + 21, v + 13, 22 * s) / 10.0) as i32;
                let beam = if v % (32 * s) / s < 3 || u % (32 * s) / s < 3 { -20 } else { 0 };
                let beam_hi = if v % (32 * s) == 0 || u % (32 * s) == 0 { -8 } else { 0 };
                ceil[idx] = make_color(
                    38 + cn / 8 + beam + beam_hi + ceil_mottle,
                    36 + cn / 8 + beam + beam_hi + ceil_mottle,
                    44 + cn / 8 + beam + beam_hi + ceil_mottle,
                );
            }
        }

        Textures { wall, floor, ceil }
    }
}
