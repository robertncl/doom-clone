//! Procedurally generated wall, floor/ceiling ("flat") and sky textures.
//!
//! Each material's macro layout (brick courses, metal panels, floor tiles, …)
//! is authored against a fixed 64-texel "design" grid and mapped onto the real
//! `TEX_SIZE` grid through a scale factor `s = TEX_SIZE / 64`. Bumping the
//! texture resolution therefore keeps the same proportions but yields crisper
//! edges and finer grain under the bilinear filter. Low-frequency colour
//! variation comes from smoothed value-noise / fbm (organic mottling) layered
//! over the fine `hash2` grain, so surfaces read as weathered rather than as
//! uniform TV-static noise.
//!
//! Texels are `0xEERRGGBB`: the top byte is *emission*, how much of the texel
//! lights itself (a torch flame, a lamp lens, lava). The renderer adds it on
//! top of the lightmap, so those texels glow even in a dark room. Plain
//! surfaces leave it at zero.
//!
//! Everything here is immutable once built, so one copy is shared by every
//! `Game` in the process ([`Textures::shared`]); skies are built on first use.

use crate::color::{hash2, make_color};
use crate::constants::*;
use crate::theme::*;
use std::sync::OnceLock;

/// One square texture. A fixed-size array (not a `Vec`) so the sampler's
/// mask-derived indices are provably in bounds and the bounds checks compile
/// away in the hot wall/floor loops.
pub type Tex = [u32; TEX_SIZE * TEX_SIZE];

/// Variants every wall material comes in. A wall face picks one per level from
/// a hash of its position (see `lighting.rs`), so long runs of the same wall
/// break up.
pub const WALL_VARIANTS: usize = 4;
pub const WV_BASE: usize = 0;
/// Decorative, unlit: a banner, a barred window, hazard paint, skulls…
pub const WV_DETAIL: usize = 1;
/// A light fixture: torch, lamp, lantern, fire fissure or screen. These faces
/// also become light sources in the level's lightmap.
pub const WV_LIGHT: usize = 2;
/// Weathered: moss, blood, rust, scorching.
pub const WV_WORN: usize = 3;

/// Sky panorama size: `SKY_W` texels around the full circle, and one row per
/// screen row above the horizon (the camera never pitches).
pub const SKY_W: usize = 1024;
pub const SKY_H: usize = SCREEN_H / 2;
pub type SkyTex = [u32; SKY_W * SKY_H];

pub struct Textures {
    /// `wall[kind][variant]`. Index 0 (`WALL_NONE`) is a copy of stone so
    /// `wall[wall_type]` indexes directly.
    pub wall: Vec<[Box<Tex>; WALL_VARIANTS]>,
    /// Floors and ceilings, indexed by the `FLAT_*` constants.
    pub flats: Vec<Box<Tex>>,
}

/// Freeze a build-time buffer into a fixed-size texture.
fn to_tex(v: Vec<u32>) -> Box<Tex> {
    v.into_boxed_slice().try_into().expect("texture buffer size mismatch")
}

// ---- noise ----

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

/// Value noise on a continuous lattice that wraps every `period` lattice
/// steps in X — what a sky panorama needs to join up behind the player.
/// Returns 0..1.
fn pnoise(x: f64, y: f64, period: i32, seed: i32) -> f64 {
    let gx = x.floor();
    let gy = y.floor();
    let fx = x - gx;
    let fy = y - gy;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let (gx, gy) = (gx as i32, gy as i32);
    let h = |i: i32, j: i32| hash2(i.rem_euclid(period) + seed * 1013, j + seed * 7919) as f64 / 255.0;
    let top = h(gx, gy) + (h(gx + 1, gy) - h(gx, gy)) * sx;
    let bot = h(gx, gy + 1) + (h(gx + 1, gy + 1) - h(gx, gy + 1)) * sx;
    top + (bot - top) * sy
}

/// Wrapping fbm for skies: `octaves` of [`pnoise`], each twice as fine. `x` is
/// in lattice steps of the coarsest octave, which wraps every `period` steps.
/// Returns roughly 0..1.
fn pfbm(x: f64, y: f64, period: i32, octaves: u32, seed: i32) -> f64 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, 1.0);
    for o in 0..octaves {
        sum += pnoise(x * f, y * f, period * f as i32, seed + o as i32) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

/// Distances to the nearest and second-nearest jittered feature point of a
/// Voronoi grid with `cell`-unit spacing that tiles every `period` cells, plus
/// the nearest point's id. Drives cobblestones and rock cracks.
fn voronoi(x: f64, y: f64, cell: f64, period: i32, seed: i32) -> (f64, f64, i32) {
    let gx = (x / cell).floor() as i32;
    let gy = (y / cell).floor() as i32;
    let (mut d1, mut d2, mut id) = (1e9f64, 1e9f64, 0);
    for j in -1..=1 {
        for i in -1..=1 {
            let (cx, cy) = (gx + i, gy + j);
            let (wx, wy) = (cx.rem_euclid(period), cy.rem_euclid(period));
            let jx = hash2(wx * 3 + seed, wy * 5 + 11) as f64 / 255.0;
            let jy = hash2(wx * 7 + 3, wy * 2 + seed) as f64 / 255.0;
            let px = (cx as f64 + 0.15 + 0.7 * jx) * cell;
            let py = (cy as f64 + 0.15 + 0.7 * jy) * cell;
            let d = ((px - x) * (px - x) + (py - y) * (py - y)).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = wy * period + wx;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    (d1, d2, id)
}

// ---- colour helpers (build time only) ----

fn ch(c: u32, shift: u32) -> f64 {
    ((c >> shift) & 0xFF) as f64
}

fn rgb(r: f64, g: f64, b: f64) -> u32 {
    make_color(r as i32, g as i32, b as i32)
}

/// Linear blend of two texels, emission included.
fn mix(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let l = |s: u32| ch(a, s) + (ch(b, s) - ch(a, s)) * t;
    let e = l(24).clamp(0.0, 255.0) as u32;
    (e << 24) | rgb(l(16), l(8), l(0))
}

/// Scale a texel's colour (emission kept).
fn darken(c: u32, m: f64) -> u32 {
    (c & 0xFF00_0000) | rgb(ch(c, 16) * m, ch(c, 8) * m, ch(c, 0) * m)
}

/// Set emission to at least `e` (0..255).
fn glow(c: u32, e: f64) -> u32 {
    let cur = ch(c, 24);
    let e = e.max(cur).clamp(0.0, 255.0) as u32;
    (c & 0x00FF_FFFF) | (e << 24)
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 1 in the middle of a texture, falling to 0 before the edges — patches fade
/// out so a variant cell still joins seamlessly onto its neighbours.
fn patch_weight(du: f64, dv: f64) -> f64 {
    let m = (du - 32.0).abs().max((dv - 32.0).abs());
    1.0 - smoothstep(17.0, 29.0, m)
}

/// Build a texture from a per-texel function of real texel coordinates.
fn gen(f: impl Fn(i32, i32) -> u32) -> Vec<u32> {
    let mut out = vec![0u32; TEX_SIZE * TEX_SIZE];
    for v in 0..TEX_SIZE as i32 {
        for u in 0..TEX_SIZE as i32 {
            out[v as usize * TEX_SIZE + u as usize] = f(u, v);
        }
    }
    out
}

/// Repaint `base` through `f(du, dv, texel)`, where `du, dv` are the texel's
/// centre in 64-unit design space.
fn overlay(base: &[u32], f: impl Fn(f64, f64, u32) -> u32) -> Vec<u32> {
    let s = (TEX_SIZE / 64) as f64;
    let mut out = base.to_vec();
    for v in 0..TEX_SIZE {
        for u in 0..TEX_SIZE {
            let du = (u as f64 + 0.5) / s;
            let dv = (v as f64 + 0.5) / s;
            out[v * TEX_SIZE + u] = f(du, dv, base[v * TEX_SIZE + u]);
        }
    }
    out
}

fn in_rect(du: f64, dv: f64, u0: f64, u1: f64, v0: f64, v1: f64) -> bool {
    du >= u0 && du < u1 && dv >= v0 && dv < v1
}

// ---- base wall materials (the original set, unchanged) ----

fn wall_stone(u: i32, v: i32, s: i32) -> u32 {
    // Per-block tint, fbm mottling, cracks, moss.
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
    make_color(s_r + block_shadow, s_g + block_shadow, s_b + block_shadow)
}

fn wall_brick(u: i32, v: i32, s: i32) -> u32 {
    // Bevels: top/left highlight, bottom/right shadow.
    let row = v / (8 * s);
    let col_off = if row & 1 != 0 { 8 * s } else { 0 };
    let bu = (u + col_off) % (16 * s) / s; // design column 0..15
    let bv = v % (8 * s) / s; // design row 0..7
    let mortar = bv == 0 || bv == 7 || bu == 0 || bu == 15;
    let hi_edge = bv == 1 || bu == 1;
    let lo_edge = bv == 6 || bu == 14;
    let bn = hash2(u / (2 * s) + (row & 1) * 13, v / (2 * s)) - 128;
    let brick_mottle = (fbm(u + 7, v + 3, 16 * s) / 10.0) as i32;
    if mortar {
        return make_color(60, 52, 48);
    }
    let (mut r, mut g, mut b) =
        (150 + bn / 4 + brick_mottle, 65 + bn / 8 + brick_mottle / 2, 50 + bn / 8 + brick_mottle / 2);
    if hi_edge {
        r += 25;
        g += 15;
        b += 8;
    }
    if lo_edge {
        r -= 30;
        g -= 20;
        b -= 15;
    }
    make_color(r, g, b)
}

fn wall_metal(u: i32, v: i32, s: i32) -> u32 {
    // Bevels, rivets, scratches and small rust spots.
    let pu = u % (32 * s) / s; // design panel coord 0..31
    let pv = v % (32 * s) / s;
    let bevel = pu < 2 || pu > 29 || pv < 2 || pv > 29;
    let hi = pu < 1 || pv < 1;
    let rivet = (pu == 5 || pu == 26) && (pv == 5 || pv == 26);
    let rivet_hi = pu == 5 && pv == 5;
    let mn = hash2(u, v) - 128;
    let scratch = if (u * 3 + v) % (31 * s) == 0 && v % (8 * s) / s > 1 { 30 } else { 0 };
    let rust = hash2(u / (3 * s) + 5, v / (3 * s) + 11) > 235;
    if rivet_hi {
        make_color(230, 230, 235)
    } else if rivet {
        make_color(200, 200, 210)
    } else if bevel && hi {
        make_color(150, 160, 190)
    } else if bevel {
        make_color(38, 46, 66)
    } else if rust {
        make_color(130 + mn / 8, 70 + mn / 10, 35 + mn / 12)
    } else {
        make_color(80 + mn / 6 + scratch, 95 + mn / 6 + scratch, 130 + mn / 6 + scratch)
    }
}

fn wall_wood(u: i32, v: i32, s: i32) -> u32 {
    // Vertical plank divisions, sin grain, knots.
    let wn = hash2(u / (4 * s), v / s) - 128;
    let pl = u % (16 * s) / s; // design plank coord 0..15
    let plank_seam = if pl == 0 || pl == 15 { -40 } else { 0 };
    let plank_hi = if pl == 1 { 15 } else { 0 };
    let grain = (18.0 * (u as f64 * 0.42 / s as f64 + wn as f64 * 0.05).sin()) as i32;
    let band = if v % (22 * s) / s < 2 { -35 } else { 0 };
    let knot1 = if (u - 22 * s).pow(2) + (v - 30 * s).pow(2) < 10 * s * s { -45 } else { 0 };
    let knot2 = if (u - 8 * s).pow(2) + (v - 50 * s).pow(2) < 7 * s * s { -40 } else { 0 };
    let k = band + knot1 + knot2 + plank_seam;
    make_color(115 + grain + wn / 8 + k + plank_hi, 70 + grain / 2 + wn / 10 + k, 30 + wn / 12 + k)
}

fn wall_hell(u: i32, v: i32, s: i32) -> u32 {
    // Dark base with glowing red veins + lava spots.
    let hn = hash2(u, v) - 128;
    let hn2 = hash2(u + 50 * s, v + 30 * s) - 128;
    let hmottle = (fbm(u + 31, v + 17, 16 * s) / 7.0) as i32;
    let vein_u = (u + hash2(u / (6 * s), v / (6 * s)) % (6 * s)) % (22 * s);
    let vein_v = (v + hash2(v / (6 * s), u / (6 * s)) % (6 * s)) % (18 * s);
    let mut vein = vein_u < 2 * s || vein_v < 2 * s;
    if hash2(u / (5 * s) + 3, v / (5 * s) + 9) > 195 {
        vein = false;
    }
    let lava_spot = hash2(u / (4 * s), v / (4 * s)) > 248;
    if vein {
        // The veins glow faintly on their own, so hell rock never goes flat
        // black in the dark.
        let c = make_color(200 + hn / 6, 40 + hn2 / 8, 20 + hn2 / 10);
        return glow(c, 70.0);
    }
    if lava_spot {
        return glow(make_color(230, 140 + hn / 8, 30), 150.0);
    }
    let (mut r, mut g, mut b) = (60 + hn / 4 + hmottle, 22 + hn2 / 8 + hmottle / 3, 22 + hn / 10 + hmottle / 3);
    if (u * u + v * v) % (17 * s * s) == 0 {
        r -= 20;
        g -= 8;
        b -= 8;
    }
    make_color(r, g, b)
}

fn wall_tech(u: i32, v: i32, s: i32) -> u32 {
    // Dark plating cut by glowing coolant channels, with recessed vents and a
    // bolt at each panel corner.
    let qu = u % (32 * s) / s; // design panel coord 0..31
    let qv = v % (32 * s) / s;
    let tn = hash2(u + 13, v + 7) - 128;
    let tmottle = (fbm(u + 41, v + 23, 20 * s) / 9.0) as i32;
    // Channel runs down one edge of every panel and across its middle.
    let channel = qu == 15 || qu == 16 || (qv > 6 && qv < 9 && qu > 3 && qu < 28);
    let vent = qv > 18 && qv < 28 && qu > 6 && qu < 25 && (qv % 3 == 0);
    let seam = qu < 1 || qv < 1 || qu > 30 || qv > 30;
    let bolt = (qu == 3 || qu == 28) && (qv == 3 || qv == 28);
    if bolt {
        make_color(170 + tn / 8, 180 + tn / 8, 190 + tn / 8)
    } else if channel {
        // Cyan glow, brightest along the channel's centre line.
        let core = if qu == 15 || qv == 7 { 60 } else { 0 };
        glow(make_color(30 + core / 2 + tn / 12, 170 + core + tn / 8, 190 + core + tn / 8), 110.0)
    } else if vent {
        make_color(18, 22, 26)
    } else if seam {
        make_color(22 + tn / 12, 26 + tn / 12, 34 + tn / 12)
    } else {
        make_color(58 + tn / 6 + tmottle, 64 + tn / 6 + tmottle, 74 + tn / 6 + tmottle)
    }
}

// ---- wall overlays: light fixtures ----

/// A warm halo painted onto the wall around a light, so the fixture sits in a
/// pool of its own glow rather than on flat, unlit stone.
#[allow(clippy::too_many_arguments)]
fn halo(c: u32, du: f64, dv: f64, cx: f64, cy: f64, r: f64, tint: u32, e: f64) -> u32 {
    let d = ((du - cx) * (du - cx) + (dv - cy) * (dv - cy)).sqrt();
    if d >= r {
        return c;
    }
    let t = (1.0 - d / r).powi(2);
    glow(mix(c, tint, 0.35 * t), e * t)
}

/// A flame: a teardrop from `top` down to `bot` centred on `cx`, white-hot at
/// the base fading to red at the tips. Fully emissive.
fn flame(du: f64, dv: f64, cx: f64, top: f64, bot: f64, w: f64) -> Option<u32> {
    if dv < top || dv > bot {
        return None;
    }
    let t = (dv - top) / (bot - top); // 0 tip .. 1 base
    let half = w * t.powf(0.6) * (1.0 - t * t * 0.55);
    let off = (du - cx).abs();
    if off >= half {
        return None;
    }
    let r = off / half.max(0.01);
    let heat = (1.0 - r) * (0.35 + 0.65 * t);
    let c = if heat > 0.62 {
        mix(rgb(255.0, 200.0, 80.0), rgb(255.0, 250.0, 215.0), (heat - 0.62) / 0.38)
    } else if heat > 0.3 {
        mix(rgb(250.0, 110.0, 30.0), rgb(255.0, 200.0, 80.0), (heat - 0.3) / 0.32)
    } else {
        mix(rgb(170.0, 40.0, 15.0), rgb(250.0, 110.0, 30.0), heat / 0.3)
    };
    Some(glow(c, 200.0 + 55.0 * heat))
}

/// Iron torch sconce with a burning brand (stone and brick walls).
fn torch_sconce(du: f64, dv: f64, c: u32) -> u32 {
    let mut c = c;
    // Soot stain above the flame.
    let (sx, sy) = ((du - 32.0) / 8.0, (dv - 4.0) / 9.0);
    if sx * sx + sy * sy < 1.0 {
        c = darken(c, 0.45 + 0.55 * (sx * sx + sy * sy));
    }
    c = halo(c, du, dv, 32.0, 14.0, 17.0, rgb(255.0, 150.0, 60.0), 80.0);
    if let Some(f) = flame(du, dv, 32.0, 2.0, 21.0, 6.5) {
        return f;
    }
    let iron = rgb(46.0, 40.0, 38.0);
    // Cup holding the brand, with a hot rim.
    if in_rect(du, dv, 27.5, 36.5, 20.0, 24.0) {
        return if dv < 21.0 { glow(rgb(200.0, 90.0, 30.0), 140.0) } else { iron };
    }
    // Wooden brand.
    if in_rect(du, dv, 30.5, 33.5, 24.0, 40.0) {
        let shade = if du < 31.5 { 1.15 } else if du > 32.5 { 0.7 } else { 1.0 };
        return rgb(96.0 * shade, 58.0 * shade, 30.0 * shade);
    }
    // Wall plate with two rivets.
    if in_rect(du, dv, 27.0, 37.0, 36.0, 47.0) {
        let rivet = (du - 29.5).abs() < 1.0 && (dv - 44.5).abs() < 1.0
            || (du - 34.5).abs() < 1.0 && (dv - 44.5).abs() < 1.0;
        return if rivet {
            rgb(120.0, 112.0, 104.0)
        } else if dv < 37.0 || du < 28.0 {
            rgb(78.0, 70.0, 64.0)
        } else {
            iron
        };
    }
    c
}

/// A caged lamp: steel housing around a cold white lens (metal walls).
fn wall_lamp(du: f64, dv: f64, c: u32) -> u32 {
    let c = halo(c, du, dv, 32.0, 24.0, 20.0, rgb(190.0, 215.0, 255.0), 70.0);
    if in_rect(du, dv, 21.0, 43.0, 19.0, 29.0) {
        // Lens, crossed by the cage bars.
        let bar = (du - 21.0) % 5.5 < 1.0;
        return if bar { glow(rgb(130.0, 150.0, 170.0), 120.0) } else { glow(rgb(228.0, 240.0, 255.0), 255.0) };
    }
    if in_rect(du, dv, 18.0, 46.0, 16.0, 32.0) {
        return if dv < 17.0 { rgb(120.0, 128.0, 140.0) } else { rgb(34.0, 38.0, 46.0) };
    }
    c
}

/// A hanging lantern on a hook (wooden walls).
fn lantern(du: f64, dv: f64, c: u32) -> u32 {
    let c = halo(c, du, dv, 32.0, 22.0, 18.0, rgb(255.0, 190.0, 100.0), 75.0);
    let frame = rgb(40.0, 34.0, 28.0);
    if in_rect(du, dv, 31.4, 32.6, 2.0, 12.0) {
        return frame; // hook chain
    }
    if in_rect(du, dv, 26.0, 38.0, 12.0, 15.0) || in_rect(du, dv, 26.0, 38.0, 30.0, 33.0) {
        return rgb(64.0, 54.0, 42.0); // cap and base
    }
    if in_rect(du, dv, 27.0, 37.0, 15.0, 30.0) {
        let bar = du < 28.0 || du > 36.0 || (dv - 22.5).abs() < 0.6;
        if bar {
            return frame;
        }
        let t = 1.0 - ((du - 32.0).abs() / 5.0).min(1.0);
        return glow(mix(rgb(240.0, 150.0, 60.0), rgb(255.0, 235.0, 170.0), t), 220.0 + 35.0 * t);
    }
    c
}

/// A glowing fissure splitting the rock (hell walls).
fn fire_fissure(du: f64, dv: f64, c: u32) -> u32 {
    if !(3.0..61.0).contains(&dv) {
        return c;
    }
    let centre = 32.0 + 5.0 * (dv * 0.33).sin() + 2.5 * (dv * 0.91 + 1.0).sin();
    let half = 0.8 + 3.2 * (std::f64::consts::PI * (dv - 3.0) / 58.0).sin();
    let off = (du - centre).abs();
    if off < half {
        let t = 1.0 - off / half;
        return glow(mix(rgb(230.0, 70.0, 15.0), rgb(255.0, 220.0, 120.0), t), 210.0 + 45.0 * t);
    }
    if off < half + 7.0 {
        let t = (1.0 - (off - half) / 7.0).powi(2);
        return glow(mix(darken(c, 0.8), rgb(220.0, 70.0, 20.0), 0.55 * t), 120.0 * t);
    }
    c
}

/// A status screen with scrolling readouts (tech walls).
fn status_screen(du: f64, dv: f64, c: u32) -> u32 {
    let c = halo(c, du, dv, 32.0, 26.0, 22.0, rgb(90.0, 255.0, 170.0), 50.0);
    if in_rect(du, dv, 17.0, 47.0, 15.0, 37.0) {
        let line = ((dv - 17.0) / 3.0).floor();
        let on_row = (dv - 17.0) % 3.0 < 1.4 && dv < 35.5 && dv > 17.0;
        let len = 6.0 + (hash2(line as i32, 77) % 20) as f64;
        if on_row && du > 19.0 && du < 19.0 + len {
            return glow(rgb(110.0, 255.0, 150.0), 235.0);
        }
        return glow(rgb(8.0, 34.0, 22.0), 90.0);
    }
    if in_rect(du, dv, 14.0, 50.0, 12.0, 40.0) {
        let lit = in_rect(du, dv, 43.0, 46.0, 37.5, 39.0);
        return if lit { glow(rgb(255.0, 60.0, 40.0), 255.0) } else { rgb(24.0, 28.0, 32.0) };
    }
    c
}

// ---- wall overlays: details and wear ----

/// A heavy red banner with gold trim and a pointed hem (stone).
fn banner(du: f64, dv: f64, c: u32) -> u32 {
    let hem = 52.0 - (du - 32.0).abs() * 0.6;
    if !(20.0..44.0).contains(&du) || !(4.0..hem).contains(&dv) {
        // Its shadow on the wall.
        if (21.0..46.0).contains(&du) && (5.0..hem + 2.0).contains(&dv) {
            return darken(c, 0.6);
        }
        return c;
    }
    let edge = du < 22.0 || du > 42.0 || dv < 6.0 || dv > hem - 2.0;
    if edge {
        return rgb(200.0, 160.0, 60.0);
    }
    let fold = 0.8 + 0.2 * ((du - 20.0) * 0.8).sin();
    let (ex, ey) = (du - 32.0, dv - 24.0);
    if ex.abs() + ey.abs() < 6.0 {
        return rgb(220.0 * fold, 180.0 * fold, 70.0 * fold); // emblem
    }
    rgb(140.0 * fold, 18.0 * fold, 20.0 * fold)
}

/// A barred window onto darkness, with a stone sill (brick).
fn barred_window(du: f64, dv: f64, c: u32) -> u32 {
    if in_rect(du, dv, 18.0, 46.0, 40.0, 44.0) {
        return if dv < 41.0 { rgb(170.0, 165.0, 155.0) } else { rgb(120.0, 116.0, 110.0) };
    }
    if in_rect(du, dv, 18.0, 46.0, 12.0, 40.0) {
        if (du - 18.0) % 6.0 < 1.5 && du > 19.0 {
            return rgb(56.0, 54.0, 58.0); // bars
        }
        let depth = 1.0 - (dv - 12.0) / 40.0;
        return rgb(14.0 * depth + 6.0, 12.0 * depth + 6.0, 18.0 * depth + 8.0);
    }
    c
}

/// Yellow-and-black hazard band under a vent grille (metal).
fn hazard_vent(du: f64, dv: f64, c: u32) -> u32 {
    if (40.0..52.0).contains(&dv) {
        let stripe = (((du + dv) / 5.0).floor() as i32) & 1 == 0;
        return if stripe { rgb(215.0, 175.0, 30.0) } else { rgb(24.0, 22.0, 20.0) };
    }
    if in_rect(du, dv, 16.0, 48.0, 8.0, 26.0) {
        let slat = (dv - 8.0) % 4.5;
        return if slat < 1.2 {
            rgb(150.0, 158.0, 175.0)
        } else if slat < 3.2 {
            rgb(20.0, 24.0, 30.0)
        } else {
            rgb(70.0, 78.0, 96.0)
        };
    }
    c
}

/// Two cross-braced planks nailed over the boards (wood).
fn cross_brace(du: f64, dv: f64, c: u32) -> u32 {
    let d1 = (du - dv).abs() / std::f64::consts::SQRT_2;
    let d2 = (du + dv - 64.0).abs() / std::f64::consts::SQRT_2;
    let d = d1.min(d2);
    if d < 4.0 {
        let nail = ((du - 10.0).abs() < 1.0 || (du - 54.0).abs() < 1.0) && d < 1.2;
        if nail {
            return rgb(150.0, 150.0, 155.0);
        }
        let edge = if d > 3.0 { 0.7 } else { 1.0 };
        let grain = 0.92 + 0.08 * ((du + dv) * 1.3).sin();
        return rgb(150.0 * edge * grain, 98.0 * edge * grain, 52.0 * edge * grain);
    }
    if d < 5.0 {
        return darken(c, 0.55);
    }
    c
}

/// Skulls set into the rock (hell).
fn embedded_skulls(du: f64, dv: f64, c: u32) -> u32 {
    for &(cx, cy, r) in &[(16.0, 18.0, 6.5), (45.0, 30.0, 7.0), (25.0, 47.0, 6.0)] {
        let (x, y) = ((du - cx) / r, (dv - cy) / r);
        let cranium = x * x + y * y < 1.0;
        let jaw = x.abs() < 0.55 && (0.6..1.25).contains(&y);
        if !(cranium || jaw) {
            continue;
        }
        let eye = ((x - 0.38).powi(2) + (y - 0.1).powi(2) < 0.07) || ((x + 0.38).powi(2) + (y - 0.1).powi(2) < 0.07);
        let nose = x.abs() < 0.12 && (0.35..0.6).contains(&y);
        let teeth = jaw && y > 0.85 && ((x * 8.0).floor() as i32) & 1 == 0;
        if eye || nose {
            return rgb(20.0, 8.0, 6.0);
        }
        let shade = 0.75 + 0.25 * (-x - y).clamp(-1.0, 1.0);
        return if teeth {
            rgb(150.0, 140.0, 115.0)
        } else {
            rgb(205.0 * shade, 192.0 * shade, 160.0 * shade)
        };
    }
    c
}

/// Two coolant pipes clamped across the plating (tech).
fn pipes_overlay(du: f64, dv: f64, c: u32) -> u32 {
    for &(top, h) in &[(42.0, 7.0), (52.0, 5.0)] {
        if (top..top + h).contains(&dv) {
            if (du % 16.0) < 2.5 {
                return rgb(90.0, 92.0, 96.0); // clamp
            }
            let t = (dv - top) / h; // round shading across the pipe
            let m = 0.45 + 0.75 * (std::f64::consts::PI * t).sin() - 0.25 * t;
            return rgb(110.0 * m, 120.0 * m, 128.0 * m);
        }
    }
    c
}

/// Weathering, chosen per material: moss and grime on stone, blood on brick
/// and hell rock, rust on metal, charring on wood, scorching on tech.
fn worn(kind: usize, du: f64, dv: f64, c: u32, u: i32, v: i32) -> u32 {
    let n = fbm(u + 211, v + 97, 24) / 160.0; // ≈ -1..1
    match kind {
        WALL_STONE => {
            let moss = n + (dv / 64.0) * 0.7 - 0.2; // more moss low down
            if moss > 0.25 {
                let g = (hash2(u, v) % 40) as f64;
                return mix(c, rgb(45.0 + g * 0.3, 92.0 + g, 40.0), ((moss - 0.25) * 2.5).min(0.85));
            }
            darken(c, 0.9 + 0.1 * n)
        }
        WALL_BRICK | WALL_HELL => {
            // A splash with drips running down from it.
            let (sx, sy) = (du - 30.0, dv - 20.0);
            let splash = (sx * sx + sy * sy).sqrt() < 9.0 + 4.0 * n;
            let col = (du * 1.7).floor() as i32;
            let drip_len = 20.0 + (hash2(col, 5) % 26) as f64;
            let drip = (18.0..42.0).contains(&du) && hash2(col, 9) > 150 && dv > 20.0 && dv < 20.0 + drip_len;
            if splash || drip {
                return mix(c, rgb(90.0, 6.0, 8.0), 0.8);
            }
            c
        }
        WALL_METAL => {
            // Rust streaks bleeding down from the rivet rows.
            let col = (du * 1.5).floor() as i32;
            let streak = hash2(col, 31) > 120;
            let fall = ((dv - 6.0) / 52.0).clamp(0.0, 1.0);
            if streak && dv > 6.0 {
                return mix(c, rgb(120.0, 58.0, 26.0), 0.65 * (1.0 - fall) * (0.6 + 0.4 * n.abs()));
            }
            c
        }
        WALL_WOOD => {
            if n > 0.15 {
                let ember = hash2(u, v) > 250 && n > 0.45;
                return if ember {
                    glow(rgb(255.0, 120.0, 30.0), 200.0)
                } else {
                    darken(c, (1.0 - (n - 0.15) * 2.2).max(0.15))
                };
            }
            c
        }
        _ => {
            // Tech: a blast scorch with a crack through the panel.
            let (sx, sy) = (du - 36.0, dv - 30.0);
            let d = (sx * sx + sy * sy).sqrt();
            if (du - 36.0 - (dv - 30.0) * 0.4).abs() < 0.8 && d < 16.0 {
                return rgb(10.0, 10.0, 12.0);
            }
            if d < 20.0 + 6.0 * n {
                return darken(c, 0.25 + 0.75 * (d / 20.0).min(1.0));
            }
            c
        }
    }
}

// ---- flats ----

fn flat_tile(u: i32, v: i32, s: i32) -> u32 {
    // Bevels, scuffs, cracked tile patches.
    let tu = u % (16 * s) / s; // design tile coord 0..15
    let tv = v % (16 * s) / s;
    let grout = tu == 0 || tv == 0 || tu == 15 || tv == 15;
    let tile_bevel_hi = tu == 1 || tv == 1;
    let tile_bevel_lo = tu == 14 || tv == 14;
    let fn_ = hash2(u, v) - 128;
    let floor_mottle = (fbm(u + 5, v + 9, 20 * s) / 9.0) as i32;
    let tile_seed = hash2(u / (16 * s), v / (16 * s));
    let cracked_tile = tile_seed > 230;
    let (mut r, mut g, mut b);
    if grout {
        r = 25;
        g = 25;
        b = 30;
    } else if cracked_tile && (u + v * 2) % (9 * s) == 0 {
        r = 35;
        g = 35;
        b = 35;
    } else {
        r = 75 + fn_ / 6 + floor_mottle;
        g = 70 + fn_ / 6 + floor_mottle;
        b = 60 + fn_ / 8 + floor_mottle;
        if cracked_tile {
            r -= 15;
            g -= 12;
            b -= 10;
        }
        if tile_bevel_hi {
            r += 12;
            g += 12;
            b += 10;
        }
        if tile_bevel_lo {
            r -= 12;
            g -= 12;
            b -= 10;
        }
    }
    if (u + v) % (23 * s) == 0 && !grout {
        r += 8;
        g += 8;
        b += 8;
    }
    make_color(r, g, b)
}

fn flat_cobble(u: i32, v: i32, s: i32) -> u32 {
    let (du, dv) = ((u as f64 + 0.5) / s as f64, (v as f64 + 0.5) / s as f64);
    let (d1, d2, id) = voronoi(du, dv, 8.0, 8, 3);
    let gap = d2 - d1;
    let grain = (hash2(u, v) - 128) as f64 / 10.0;
    if gap < 1.1 {
        return rgb(34.0 + grain * 0.5, 32.0 + grain * 0.5, 30.0); // mortar
    }
    let tone = (hash2(id, 41) - 128) as f64 / 5.0;
    let warm = (hash2(id, 43) % 18) as f64;
    // Rounded tops: brighter towards each stone's centre, darker at the rim.
    let dome = 1.0 - (d1 / 6.0).min(1.0) * 0.35 - smoothstep(2.4, 1.1, gap) * 0.2;
    let m = fbm(u + 17, v + 3, 12 * s) / 14.0;
    rgb(
        (98.0 + tone + warm + grain + m) * dome,
        (92.0 + tone + warm * 0.6 + grain + m) * dome,
        (84.0 + tone + grain + m) * dome,
    )
}

fn flat_dirt(u: i32, v: i32, s: i32) -> u32 {
    let m = fbm(u + 7, v + 41, 22 * s) / 5.0;
    let fine = (hash2(u, v) - 128) as f64 / 7.0;
    let pebble = hash2(u / (2 * s), v / (2 * s)) > 244;
    if pebble {
        let p = 0.7 + (hash2(u + 1, v) % 60) as f64 / 100.0;
        return rgb(110.0 * p, 104.0 * p, 96.0 * p);
    }
    rgb(72.0 + m + fine, 58.0 + m * 0.8 + fine, 44.0 + m * 0.6 + fine * 0.8)
}

fn flat_grate(u: i32, v: i32, s: i32) -> u32 {
    let (du, dv) = ((u as f64 + 0.5) / s as f64, (v as f64 + 0.5) / s as f64);
    let (gu, gv) = (du % 8.0, dv % 8.0);
    let frame = !(1.5..62.5).contains(&du) || !(1.5..62.5).contains(&dv);
    let n = (hash2(u, v) - 128) as f64 / 12.0;
    if frame {
        return rgb(96.0 + n, 100.0 + n, 106.0 + n);
    }
    let bar_u = gu < 2.0;
    let bar_v = gv < 2.0;
    if bar_u || bar_v {
        let hi = (bar_u && gu < 0.7) || (bar_v && gv < 0.7);
        let m = if hi { 1.25 } else { 1.0 };
        return rgb((104.0 + n) * m, (108.0 + n) * m, (114.0 + n) * m);
    }
    // Looking down through the holes: dark, with a faint lower deck.
    let deck = fbm(u + 3, v + 5, 16 * s) / 16.0;
    rgb(14.0 + deck, 14.0 + deck, 16.0 + deck)
}

fn flat_plate(u: i32, v: i32, s: i32) -> u32 {
    let (du, dv) = ((u as f64 + 0.5) / s as f64, (v as f64 + 0.5) / s as f64);
    let (pu, pv) = (du % 32.0, dv % 32.0);
    let n = (hash2(u, v) - 128) as f64 / 10.0;
    let m = fbm(u + 31, v + 7, 24 * s) / 12.0;
    if !(1.0..31.0).contains(&pu) || !(1.0..31.0).contains(&pv) {
        return rgb(26.0, 28.0, 32.0); // seam
    }
    let bolt = ((pu - 4.0).abs() < 1.2 || (pu - 28.0).abs() < 1.2) && ((pv - 4.0).abs() < 1.2 || (pv - 28.0).abs() < 1.2);
    if bolt {
        return rgb(150.0, 152.0, 156.0);
    }
    // Diamond tread: small raised lozenges in a staggered grid.
    let row = (pv / 4.0).floor();
    let tx = (pu + if row as i32 & 1 == 1 { 2.0 } else { 0.0 }) % 4.0 - 2.0;
    let ty = pv % 4.0 - 2.0;
    let tread = tx.abs() * 0.5 + ty.abs() < 0.9;
    let base = 82.0 + n + m;
    if tread {
        rgb(base + 26.0, base + 28.0, base + 32.0)
    } else {
        rgb(base, base + 2.0, base + 8.0)
    }
}

fn flat_hellrock(u: i32, v: i32, s: i32) -> u32 {
    let (du, dv) = ((u as f64 + 0.5) / s as f64, (v as f64 + 0.5) / s as f64);
    let (d1, d2, id) = voronoi(du, dv, 16.0, 4, 9);
    let gap = d2 - d1;
    let n = (hash2(u, v) - 128) as f64 / 8.0;
    let m = fbm(u + 5, v + 77, 14 * s) / 8.0;
    if gap < 0.9 {
        // Molten seams between the plates of rock.
        let t = 1.0 - gap / 0.9;
        return glow(mix(rgb(120.0, 24.0, 10.0), rgb(255.0, 120.0, 30.0), t), 120.0 + 120.0 * t);
    }
    if gap < 2.0 {
        return rgb(30.0 + n * 0.5, 12.0, 10.0);
    }
    let tone = (hash2(id, 7) % 22) as f64;
    rgb(56.0 + tone + n + m, 26.0 + tone * 0.3 + n * 0.5 + m * 0.4, 22.0 + n * 0.4 + m * 0.3)
}

fn flat_lava(u: i32, v: i32, s: i32) -> u32 {
    // Molten cells with dark crust rafts floating on them.
    let lmottle = fbm(u + 3, v + 61, 14 * s);
    let lfine = fbm(u + 71, v + 5, 5 * s);
    let heat = lmottle + lfine * 0.5; // ≈ -240..240
    if heat < -70.0 {
        // Cooled crust: near-black rock with a dull red underglow.
        let t = ((-heat - 70.0) / 90.0).min(1.0);
        let c = rgb(110.0 - 60.0 * t, 34.0 - 22.0 * t, 20.0 - 14.0 * t);
        return glow(c, 150.0 * (1.0 - t));
    }
    if heat > 90.0 {
        // Hottest veins run white-yellow.
        let t = ((heat - 90.0) / 110.0).min(1.0);
        return glow(rgb(255.0, 200.0 + 55.0 * t, 60.0 + 120.0 * t), 255.0);
    }
    let t = (heat + 70.0) / 160.0; // 0..1 across the molten band
    glow(rgb(215.0 + 40.0 * t, 60.0 + 130.0 * t, 18.0 + 30.0 * t), 220.0)
}

fn flat_beams(u: i32, v: i32, s: i32) -> u32 {
    // Noise + occasional support-beam pattern.
    let cn = hash2(u + 17, v + 31) - 128;
    let ceil_mottle = (fbm(u + 21, v + 13, 22 * s) / 10.0) as i32;
    let beam = if v % (32 * s) / s < 3 || u % (32 * s) / s < 3 { -20 } else { 0 };
    let beam_hi = if v % (32 * s) == 0 || u % (32 * s) == 0 { -8 } else { 0 };
    let k = cn / 8 + beam + beam_hi + ceil_mottle;
    make_color(38 + k, 36 + k, 44 + k)
}

fn flat_vault(u: i32, v: i32, s: i32) -> u32 {
    // Big dark masonry blocks in offset courses.
    let row = v / (16 * s);
    let off = if row & 1 == 1 { 16 * s } else { 0 };
    let bu = (u + off) % (32 * s) / s;
    let bv = v % (16 * s) / s;
    let n = hash2(u, v) - 128;
    let m = (fbm(u + 9, v + 13, 18 * s) / 9.0) as i32;
    if bu == 0 || bv == 0 {
        return make_color(16, 15, 15);
    }
    let tint = (hash2((u + off) / (32 * s), row + 3) - 128) / 10;
    let edge = if bu == 1 || bv == 1 { 10 } else { 0 };
    let k = n / 9 + m + tint + edge;
    make_color(52 + k, 49 + k, 47 + k)
}

fn flat_ceil_panel(u: i32, v: i32, s: i32) -> u32 {
    let (du, dv) = ((u as f64 + 0.5) / s as f64, (v as f64 + 0.5) / s as f64);
    let (pu, pv) = (du % 32.0, dv % 32.0);
    let n = (hash2(u + 3, v + 1) - 128) as f64 / 12.0;
    if !(1.0..31.0).contains(&pu) || !(1.0..31.0).contains(&pv) {
        return rgb(28.0, 30.0, 36.0);
    }
    let vent = (10.0..22.0).contains(&pu) && (12.0..20.0).contains(&pv) && (pv % 2.0) < 1.0;
    if vent {
        return rgb(30.0, 32.0, 38.0);
    }
    let m = fbm(u + 1, v + 61, 20 * s) / 14.0;
    rgb(70.0 + n + m, 76.0 + n + m, 86.0 + n + m)
}

fn flat_pipes(u: i32, v: i32, s: i32) -> u32 {
    let (du, dv) = ((u as f64 + 0.5) / s as f64, (v as f64 + 0.5) / s as f64);
    let n = (hash2(u, v) - 128) as f64 / 10.0;
    // A rusty girder across the middle.
    if (28.0..36.0).contains(&du) {
        let rim = !(29.0..35.0).contains(&du);
        let m = if rim { 0.7 } else { 1.0 };
        return rgb((88.0 + n) * m, (52.0 + n * 0.6) * m, (34.0 + n * 0.4) * m);
    }
    for &(top, h) in &[(8.0, 12.0), (40.0, 10.0)] {
        if (top..top + h).contains(&dv) {
            let t = (dv - top) / h;
            let m = 0.4 + 0.8 * (std::f64::consts::PI * t).sin();
            let band = (du % 21.0) < 2.0;
            let c = if band { 70.0 } else { 104.0 };
            return rgb((c + n) * m, (c + 6.0 + n) * m, (c + 12.0 + n) * m);
        }
    }
    let m = fbm(u + 41, v + 3, 18 * s) / 12.0;
    rgb(30.0 + n * 0.5 + m, 29.0 + n * 0.5 + m, 28.0 + n * 0.5 + m)
}

// ---- flat patches (a base flat plus something in the middle of the cell) ----

fn patch_cracks(du: f64, dv: f64, c: u32, u: i32, v: i32) -> u32 {
    let w = patch_weight(du, dv);
    if w <= 0.0 {
        return c;
    }
    let crack = ((du - 32.0) * 0.9 + 4.0 * (dv * 0.3).sin() - (dv - 32.0) * 0.4).abs() < 0.8
        || ((dv - 30.0) + 3.0 * (du * 0.4).sin() + (du - 32.0) * 0.6).abs() < 0.7;
    if crack && w > 0.3 {
        return rgb(22.0, 20.0, 20.0);
    }
    let dirt = (fbm(u + 3, v + 9, 20) / 160.0 + 0.3).clamp(0.0, 1.0);
    mix(c, rgb(60.0, 50.0, 38.0), dirt * w * 0.7)
}

fn patch_blood(du: f64, dv: f64, c: u32, u: i32, v: i32) -> u32 {
    let w = patch_weight(du, dv);
    let (x, y) = (du - 32.0, dv - 32.0);
    let r = (x * x + y * y).sqrt();
    let blob = 13.0 + fbm(u + 50, v + 80, 18) / 14.0;
    if r < blob * w.max(0.3) && w > 0.05 {
        let gloss = (x + y + 10.0).abs() < 1.6 && r < blob * 0.6;
        return if gloss { rgb(170.0, 60.0, 60.0) } else { rgb(98.0 - r, 8.0, 10.0) };
    }
    // Spatter around the pool.
    if hash2(u / 2, v / 2) > 249 && w > 0.2 {
        return rgb(90.0, 10.0, 10.0);
    }
    c
}

fn patch_moss(du: f64, dv: f64, c: u32, u: i32, v: i32) -> u32 {
    let w = patch_weight(du, dv);
    let n = fbm(u + 13, v + 29, 16) / 160.0 + 0.25;
    let amt = (n * w * 1.8).clamp(0.0, 0.85);
    if amt <= 0.05 {
        return c;
    }
    let g = (hash2(u, v) % 50) as f64;
    mix(c, rgb(40.0 + g * 0.3, 88.0 + g, 34.0), amt)
}

fn patch_puddle(du: f64, dv: f64, c: u32, u: i32, v: i32) -> u32 {
    let w = patch_weight(du, dv);
    let shape = fbm(u + 90, v + 12, 20) / 160.0;
    let (x, y) = ((du - 32.0) / 15.0, (dv - 32.0) / 11.0);
    let r = (x * x + y * y).sqrt() - shape * 0.35;
    if r < 1.0 && w > 0.1 {
        // Still water: the stones show through, darkened and cooled, with a
        // pale sky glint across the surface.
        let glint = (x * 0.8 - y + 0.2).abs() < 0.08;
        let base = mix(darken(c, 0.45), rgb(40.0, 56.0, 72.0), 0.55);
        return if glint { mix(base, rgb(200.0, 210.0, 225.0), 0.6) } else { base };
    }
    if r < 1.15 {
        return darken(c, 0.75); // wet rim
    }
    c
}

fn patch_bones(du: f64, dv: f64, c: u32) -> u32 {
    let w = patch_weight(du, dv);
    if w < 0.2 {
        return c;
    }
    let bone = rgb(196.0, 186.0, 160.0);
    // A few long bones (capsules) and a skull.
    let caps: [(f64, f64, f64, f64); 3] = [(20.0, 26.0, 34.0, 20.0), (30.0, 40.0, 44.0, 44.0), (22.0, 42.0, 26.0, 30.0)];
    for (x0, y0, x1, y1) in caps {
        let (vx, vy) = (x1 - x0, y1 - y0);
        let t = (((du - x0) * vx + (dv - y0) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
        let (px, py) = (x0 + vx * t - du, y0 + vy * t - dv);
        let d = (px * px + py * py).sqrt();
        let knob = if !(0.08..=0.92).contains(&t) { 1.9 } else { 1.2 };
        if d < knob {
            return darken(bone, 0.8 + 0.2 * (1.0 - d / knob));
        }
    }
    let (x, y) = ((du - 40.0) / 5.5, (dv - 28.0) / 5.0);
    if x * x + y * y < 1.0 {
        let hole = (x - 0.35).powi(2) + (y + 0.1).powi(2) < 0.08 || (x + 0.35).powi(2) + (y + 0.1).powi(2) < 0.08;
        return if hole { rgb(20.0, 16.0, 14.0) } else { darken(bone, 0.85 + 0.15 * -y) };
    }
    c
}

fn patch_hazard_pad(du: f64, dv: f64, c: u32) -> u32 {
    let m = (du - 32.0).abs().max((dv - 32.0).abs());
    if (20.0..26.0).contains(&m) {
        let stripe = (((du + dv) / 4.0).floor() as i32) & 1 == 0;
        return if stripe { rgb(210.0, 170.0, 30.0) } else { rgb(28.0, 26.0, 22.0) };
    }
    c
}

fn patch_ceiling_light(du: f64, dv: f64, c: u32) -> u32 {
    if in_rect(du, dv, 18.0, 46.0, 18.0, 46.0) {
        if !in_rect(du, dv, 20.0, 44.0, 20.0, 44.0) {
            return rgb(150.0, 155.0, 165.0); // frame
        }
        let grid = (du - 20.0) % 6.0 < 0.7 || (dv - 20.0) % 6.0 < 0.7;
        return if grid { glow(rgb(170.0, 180.0, 190.0), 150.0) } else { glow(rgb(236.0, 242.0, 255.0), 255.0) };
    }
    c
}

// ---- skies ----

fn sky_gradient(stops: &[(f64, [f64; 3])], t: f64) -> [f64; 3] {
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t <= t1 {
            let k = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
            return [c0[0] + (c1[0] - c0[0]) * k, c0[1] + (c1[1] - c0[1]) * k, c0[2] + (c1[2] - c0[2]) * k];
        }
    }
    stops[stops.len() - 1].1
}

/// Height (in rows) of a wrapping ridge line at column `u`: the skyline of
/// distant hills, spires or ruins.
fn ridge(u: f64, base: f64, amp: f64, period: i32, seed: i32) -> f64 {
    base + amp * pfbm(u / SKY_W as f64 * period as f64, 0.5, period, 4, seed)
}

fn build_sky(id: usize) -> Box<SkyTex> {
    let mut out = vec![0u32; SKY_W * SKY_H];
    let h = SKY_H as f64;
    for r in 0..SKY_H {
        for u in 0..SKY_W {
            let uf = u as f64;
            let t = r as f64 / (h - 1.0); // 0 top .. 1 horizon
            let up = h - 1.0 - r as f64; // rows above the horizon
            let x = uf / SKY_W as f64; // 0..1 around the circle
            let c = match id {
                SKY_DUSK => {
                    let mut c = sky_gradient(
                        &[(0.0, [34.0, 22.0, 64.0]), (0.55, [128.0, 56.0, 96.0]), (0.85, [226.0, 120.0, 84.0]), (1.0, [252.0, 176.0, 96.0])],
                        t,
                    );
                    // Long, flat clouds lit from below by the setting sun.
                    let cl = pfbm(x * 6.0, r as f64 / 16.0, 6, 4, 5);
                    let band = smoothstep(0.15, 0.4, t) * (1.0 - smoothstep(0.7, 0.9, t));
                    let dens = smoothstep(0.52, 0.75, cl) * band;
                    let lit = [236.0, 120.0, 92.0];
                    let shadow = [96.0, 58.0, 98.0];
                    let k = smoothstep(0.0, 1.0, t * 1.4 - 0.2);
                    for i in 0..3 {
                        let cc = shadow[i] + (lit[i] - shadow[i]) * k;
                        c[i] += (cc - c[i]) * dens;
                    }
                    // The sun, low on the horizon, with a soft halo.
                    let (sx, sy) = (uf - 300.0, r as f64 - 176.0);
                    let sd = (sx * sx + sy * sy).sqrt();
                    let halo = (1.0 - sd / 90.0).max(0.0).powi(2) * 0.6;
                    for (i, s) in [255.0, 220.0, 150.0].iter().enumerate() {
                        c[i] += (s - c[i]) * if sd < 13.0 { 1.0 } else { halo };
                    }
                    // Two ridges of hills, the near one darker.
                    if up < ridge(uf, 6.0, 26.0, 7, 11) {
                        c = [44.0, 24.0, 52.0];
                    } else if up < ridge(uf, 14.0, 34.0, 5, 12) {
                        c = [92.0, 52.0, 86.0];
                    }
                    c
                }
                SKY_HELL => {
                    let mut c = sky_gradient(
                        &[(0.0, [26.0, 3.0, 6.0]), (0.5, [94.0, 14.0, 12.0]), (0.85, [196.0, 52.0, 18.0]), (1.0, [240.0, 110.0, 30.0])],
                        t,
                    );
                    // Churning clouds: warped fbm, dark billows over the glow.
                    let warp = pfbm(x * 4.0, r as f64 / 30.0, 4, 3, 21) * 2.0;
                    let cl = pfbm(x * 8.0 + warp, r as f64 / 20.0 + warp, 8, 4, 23);
                    let dark = smoothstep(0.45, 0.7, cl) * (1.0 - t * 0.6);
                    let hot = smoothstep(0.7, 0.85, cl) * t;
                    c = c.map(|v| v * (1.0 - 0.7 * dark));
                    c[0] += (255.0 - c[0]) * hot * 0.5;
                    c[1] += (120.0 - c[1]) * hot * 0.4;
                    // Black spires along the horizon.
                    let spire = (pnoise(uf / 8.0, 0.3, (SKY_W / 8) as i32, 31) - 0.5).max(0.0) * 80.0;
                    if up < ridge(uf, 4.0, 18.0, 9, 33) + spire {
                        c = [18.0, 4.0, 4.0];
                    }
                    c
                }
                SKY_NIGHT => {
                    let mut c = sky_gradient(&[(0.0, [3.0, 5.0, 16.0]), (0.7, [14.0, 22.0, 52.0]), (1.0, [34.0, 50.0, 88.0])], t);
                    // Stars, thinning towards the horizon haze.
                    let hs = hash2(u as i32 * 7 + 3, r as i32 * 13 + 1);
                    if hs > 252 && t < 0.8 {
                        let b = 140.0 + (hash2(u as i32, r as i32) % 115) as f64;
                        c = [b, b, b * 1.05];
                    }
                    // Wisps of cloud.
                    let cl = pfbm(x * 5.0, r as f64 / 14.0, 5, 4, 41);
                    let dens = smoothstep(0.58, 0.8, cl) * 0.5;
                    for (i, w) in [70.0, 80.0, 104.0].iter().enumerate() {
                        c[i] += (w - c[i]) * dens;
                    }
                    // The moon, with a glow and a few darker maria.
                    let (mx, my) = (uf - 700.0, r as f64 - 48.0);
                    let md = (mx * mx + my * my).sqrt();
                    if md < 13.0 {
                        let maria = pnoise(mx / 4.0 + 10.0, my / 4.0 + 10.0, 1000, 43);
                        let m = 0.85 + 0.15 * (1.0 - smoothstep(0.45, 0.65, maria));
                        c = [232.0 * m, 230.0 * m, 210.0 * m];
                    } else {
                        let halo = (1.0 - md / 70.0).max(0.0).powi(2) * 0.35;
                        for (i, w) in [180.0, 190.0, 210.0].iter().enumerate() {
                            c[i] += (w - c[i]) * halo;
                        }
                    }
                    // A treeline: a ragged ridge with spiky tops.
                    let spikes = (hash2(u as i32 / 3, 51) % 9) as f64;
                    if up < ridge(uf, 8.0, 16.0, 6, 53) + spikes {
                        c = [6.0, 8.0, 14.0];
                    }
                    c
                }
                _ => {
                    // SKY_EMBER: smoke over a burning horizon, with ash.
                    let mut c = sky_gradient(
                        &[(0.0, [20.0, 14.0, 12.0]), (0.6, [56.0, 30.0, 20.0]), (0.9, [150.0, 60.0, 22.0]), (1.0, [230.0, 120.0, 40.0])],
                        t,
                    );
                    let smoke = pfbm(x * 6.0, r as f64 / 18.0, 6, 5, 61);
                    let plume = smoothstep(0.4, 0.75, smoke);
                    c = c.map(|v| v * (1.0 - 0.6 * plume * (1.0 - t * 0.5)));
                    if hash2(u as i32 * 3 + 7, r as i32 * 5 + 9) > 253 {
                        c = [255.0, 140.0 + (hash2(u as i32, 3) % 80) as f64, 40.0];
                    }
                    if up < ridge(uf, 5.0, 30.0, 4, 63) {
                        // Volcano flanks with a glowing seam near the summit line.
                        let top = ridge(uf, 5.0, 30.0, 4, 63);
                        c = if top - up < 1.5 { [200.0, 70.0, 20.0] } else { [14.0, 8.0, 6.0] };
                    }
                    c
                }
            };
            out[r * SKY_W + u] = rgb(c[0], c[1], c[2]);
        }
    }
    out.into_boxed_slice().try_into().expect("sky buffer size mismatch")
}

static TEXTURES: OnceLock<Textures> = OnceLock::new();
static SKIES: [OnceLock<Box<SkyTex>>; SKY_COUNT] = [const { OnceLock::new() }; SKY_COUNT];

/// Sky panorama `id`, built the first time any level asks for it.
pub fn sky(id: usize) -> &'static SkyTex {
    let id = id.min(SKY_COUNT - 1);
    SKIES[id].get_or_init(|| build_sky(id))
}

impl Textures {
    /// The process-wide texture set. Built once on first use; every `Game`
    /// shares it, so a restart (or a test building many games) pays nothing.
    pub fn shared() -> &'static Textures {
        TEXTURES.get_or_init(Textures::build)
    }

    pub fn build() -> Textures {
        let s = TEX_SIZE as i32 / 64; // resolution scale: 1 at 64, 2 at 128
        let base: [fn(i32, i32, i32) -> u32; WALL_KIND_MAX] =
            [wall_stone, wall_stone, wall_brick, wall_metal, wall_wood, wall_hell, wall_tech];
        let light: [fn(f64, f64, u32) -> u32; WALL_KIND_MAX] =
            [torch_sconce, torch_sconce, torch_sconce, wall_lamp, lantern, fire_fissure, status_screen];
        let detail: [fn(f64, f64, u32) -> u32; WALL_KIND_MAX] =
            [banner, banner, barred_window, hazard_vent, cross_brace, embedded_skulls, pipes_overlay];

        let mut wall = Vec::with_capacity(WALL_KIND_MAX);
        for kind in 0..WALL_KIND_MAX {
            let b = gen(|u, v| base[kind](u, v, s));
            let d = overlay(&b, detail[kind]);
            let l = overlay(&b, light[kind]);
            let w = {
                let sc = TEX_SIZE as f64 / 64.0;
                overlay(&b, |du, dv, c| worn(kind, du, dv, c, (du * sc) as i32, (dv * sc) as i32))
            };
            wall.push([to_tex(b), to_tex(d), to_tex(l), to_tex(w)]);
        }

        let tile = gen(|u, v| flat_tile(u, v, s));
        let cobble = gen(|u, v| flat_cobble(u, v, s));
        let dirt = gen(|u, v| flat_dirt(u, v, s));
        let plate = gen(|u, v| flat_plate(u, v, s));
        let hellrock = gen(|u, v| flat_hellrock(u, v, s));
        let ceil_panel = gen(|u, v| flat_ceil_panel(u, v, s));
        let sc = TEX_SIZE as f64 / 64.0;
        let tex_uv = move |du: f64, dv: f64| ((du * sc) as i32, (dv * sc) as i32);

        let mut flats: Vec<Option<Vec<u32>>> = vec![None; FLAT_COUNT];
        flats[FLAT_TILE_CRACKED] = Some(overlay(&tile, |du, dv, c| {
            let (u, v) = tex_uv(du, dv);
            patch_cracks(du, dv, c, u, v)
        }));
        flats[FLAT_TILE_BLOOD] = Some(overlay(&tile, |du, dv, c| {
            let (u, v) = tex_uv(du, dv);
            patch_blood(du, dv, c, u, v)
        }));
        flats[FLAT_COBBLE_MOSS] = Some(overlay(&cobble, |du, dv, c| {
            let (u, v) = tex_uv(du, dv);
            patch_moss(du, dv, c, u, v)
        }));
        flats[FLAT_COBBLE_PUDDLE] = Some(overlay(&cobble, |du, dv, c| {
            let (u, v) = tex_uv(du, dv);
            patch_puddle(du, dv, c, u, v)
        }));
        flats[FLAT_DIRT_BONES] = Some(overlay(&dirt, patch_bones));
        flats[FLAT_PLATE_HAZARD] = Some(overlay(&plate, patch_hazard_pad));
        flats[FLAT_HELLROCK_BLOOD] = Some(overlay(&hellrock, |du, dv, c| {
            let (u, v) = tex_uv(du, dv);
            patch_blood(du, dv, c, u, v)
        }));
        flats[FLAT_CEIL_LIGHT] = Some(overlay(&ceil_panel, patch_ceiling_light));
        flats[FLAT_GRATE] = Some(gen(|u, v| flat_grate(u, v, s)));
        flats[FLAT_LAVA] = Some(gen(|u, v| flat_lava(u, v, s)));
        flats[FLAT_BEAMS] = Some(gen(|u, v| flat_beams(u, v, s)));
        flats[FLAT_VAULT] = Some(gen(|u, v| flat_vault(u, v, s)));
        flats[FLAT_PIPES] = Some(gen(|u, v| flat_pipes(u, v, s)));
        flats[FLAT_TILE] = Some(tile);
        flats[FLAT_COBBLE] = Some(cobble);
        flats[FLAT_DIRT] = Some(dirt);
        flats[FLAT_PLATE] = Some(plate);
        flats[FLAT_HELLROCK] = Some(hellrock);
        flats[FLAT_CEIL_PANEL] = Some(ceil_panel);

        Textures {
            wall,
            flats: flats.into_iter().map(|f| to_tex(f.expect("every flat is generated"))).collect(),
        }
    }
}
