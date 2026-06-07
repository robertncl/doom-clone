//! Procedurally generated wall/floor/ceiling textures (port of `buildTextures`).

use crate::color::{hash2, make_color};
use crate::constants::*;

pub struct Textures {
    /// One texture per wall kind. Index 0 (`WALL_NONE`) is unused but kept so
    /// `wall[wall_type]` indexes directly, matching the C array.
    pub wall: Vec<Vec<u32>>,
    pub floor: Vec<u32>,
    pub ceil: Vec<u32>,
}

impl Textures {
    pub fn build() -> Textures {
        let n = TEX_SIZE * TEX_SIZE;
        let mut wall = vec![vec![0u32; n]; WALL_KIND_MAX];
        let mut floor = vec![0u32; n];
        let mut ceil = vec![0u32; n];

        for v in 0..TEX_SIZE as i32 {
            for u in 0..TEX_SIZE as i32 {
                let idx = (v as usize) * TEX_SIZE + u as usize;

                // ---- Stone: per-block tint, cracks, occasional moss patches ----
                let n_ = hash2(u, v) - 128;
                let block_u = u / 16;
                let block_v = v / 16;
                let block_tint = (hash2(block_u + 1, block_v + 7) - 128) / 6;
                let crack1 = if (u + v * 2) % 19 == 0 { -50 } else { 0 };
                let crack2 = if (u * 2 - v + 64) % 23 == 0 { -40 } else { 0 };
                let crack3 = if (u - 28).abs() + (v * 2 - 30).abs() < 4 { -35 } else { 0 };
                let moss_seed = hash2(u / 4, v / 4);
                let moss_local = hash2(u, v);
                let moss = moss_seed > 230 && moss_local > 160;
                let (s_r, s_g, s_b);
                if moss {
                    let mn2 = hash2(u, v) - 128;
                    s_r = 50 + mn2 / 6;
                    s_g = 110 + mn2 / 4;
                    s_b = 50 + mn2 / 6;
                } else {
                    let delta = crack1 + crack2 + crack3 + block_tint + n_ / 4;
                    s_r = 130 + delta;
                    s_g = 125 + delta - 2;
                    s_b = 115 + delta - 10;
                }
                let block_shadow = if (u % 16 == 0 || v % 16 == 0) && !moss { -15 } else { 0 };
                wall[WALL_STONE][idx] =
                    make_color(s_r + block_shadow, s_g + block_shadow, s_b + block_shadow);

                // ---- Brick: bevels (top/left highlight, bottom/right shadow) ----
                let row = v / 8;
                let col_off = if row & 1 != 0 { 8 } else { 0 };
                let brick_u = (u + col_off) % 16;
                let brick_v = v % 8;
                let mortar = brick_v == 0 || brick_v == 7 || brick_u == 0 || brick_u == 15;
                let hi_edge = brick_v == 1 || brick_u == 1;
                let lo_edge = brick_v == 6 || brick_u == 14;
                let bn = hash2(u / 2 + (row & 1) * 13, v / 2) - 128;
                let (mut br_r, mut br_g, mut br_b);
                if mortar {
                    br_r = 60;
                    br_g = 52;
                    br_b = 48;
                } else {
                    br_r = 150 + bn / 4;
                    br_g = 65 + bn / 8;
                    br_b = 50 + bn / 8;
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
                let p_u = u % 32;
                let p_v = v % 32;
                let bevel = p_u < 2 || p_u > 29 || p_v < 2 || p_v > 29;
                let hi = p_u < 1 || p_v < 1;
                let rivet = (p_u == 5 || p_u == 26) && (p_v == 5 || p_v == 26);
                let rivet_hi = p_u == 5 && p_v == 5;
                let mn = hash2(u, v) - 128;
                let scratch = if (u * 3 + v) % 31 == 0 && (v % 8) > 1 { 30 } else { 0 };
                let rust = hash2(u / 3 + 5, v / 3 + 11) > 235;
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
                let wn = hash2(u / 4, v) - 128;
                let plank = u % 16;
                let plank_seam = if plank == 0 || plank == 15 { -40 } else { 0 };
                let plank_hi = if plank == 1 { 15 } else { 0 };
                let grain = (18.0 * (u as f64 * 0.42 + wn as f64 * 0.05).sin()) as i32;
                let band = if v % 22 < 2 { -35 } else { 0 };
                let knot1 = if (u - 22) * (u - 22) + (v - 30) * (v - 30) < 10 { -45 } else { 0 };
                let knot2 = if (u - 8) * (u - 8) + (v - 50) * (v - 50) < 7 { -40 } else { 0 };
                let w_r = 115 + grain + wn / 8 + band + knot1 + knot2 + plank_seam + plank_hi;
                let w_g = 70 + grain / 2 + wn / 10 + band + knot1 + knot2 + plank_seam;
                let w_b = 30 + wn / 12 + band + knot1 + knot2 + plank_seam;
                wall[WALL_WOOD][idx] = make_color(w_r, w_g, w_b);

                // ---- Hell rock: dark base with glowing red veins + lava spots ----
                let hn = hash2(u, v) - 128;
                let hn2 = hash2(u + 50, v + 30) - 128;
                let vein_u = (u + (hash2(u / 6, v / 6) % 6)) % 22;
                let vein_v = (v + (hash2(v / 6, u / 6) % 6)) % 18;
                let mut vein = vein_u < 2 || vein_v < 2;
                if hash2(u / 5 + 3, v / 5 + 9) > 195 {
                    vein = false;
                }
                let lava = hash2(u / 4, v / 4) > 248;
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
                    h_r = 60 + hn / 4;
                    h_g = 22 + hn2 / 8;
                    h_b = 22 + hn / 10;
                    if (u * u + v * v) % 17 == 0 {
                        h_r -= 20;
                        h_g -= 8;
                        h_b -= 8;
                    }
                }
                wall[WALL_HELL][idx] = make_color(h_r, h_g, h_b);

                // ---- Floor tile: bevels, scuffs, cracked tile patches ----
                let t_u = u % 16;
                let t_v = v % 16;
                let grout = t_u == 0 || t_v == 0 || t_u == 15 || t_v == 15;
                let tile_bevel_hi = t_u == 1 || t_v == 1;
                let tile_bevel_lo = t_u == 14 || t_v == 14;
                let fn_ = hash2(u, v) - 128;
                let tile_seed = hash2(u / 16, v / 16);
                let cracked_tile = tile_seed > 230;
                let (mut f_r, mut f_g, mut f_b);
                if grout {
                    f_r = 25;
                    f_g = 25;
                    f_b = 30;
                } else if cracked_tile && (u + v * 2) % 9 == 0 {
                    f_r = 35;
                    f_g = 35;
                    f_b = 35;
                } else {
                    f_r = 75 + fn_ / 6;
                    f_g = 70 + fn_ / 6;
                    f_b = 60 + fn_ / 8;
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
                if (u + v) % 23 == 0 && !grout {
                    f_r += 8;
                    f_g += 8;
                    f_b += 8;
                }
                floor[idx] = make_color(f_r, f_g, f_b);

                // ---- Ceiling: noise + occasional support-beam pattern ----
                let cn = hash2(u + 17, v + 31) - 128;
                let beam = if v % 32 < 3 || u % 32 < 3 { -20 } else { 0 };
                let beam_hi = if v % 32 == 0 || u % 32 == 0 { -8 } else { 0 };
                ceil[idx] = make_color(
                    38 + cn / 8 + beam + beam_hi,
                    36 + cn / 8 + beam + beam_hi,
                    44 + cn / 8 + beam + beam_hi,
                );
            }
        }

        Textures { wall, floor, ceil }
    }
}
