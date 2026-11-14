//! The 3D view and full-frame composition.
//!
//! A frame is built in two steps. First, one DDA ray per framebuffer column
//! finds that column's wall and records everything needed to draw it (extent,
//! texture and mip level, light). That table, the sprite list and the level's
//! lighting make up an immutable [`View`]. Second, the framebuffer is cut into
//! horizontal bands and each band is rendered from the view: its slice of the
//! wall columns, then floor/ceiling/sky rows (only the pixels walls left
//! uncovered), then the sprites clipped to the band. Bands share nothing
//! mutable, so they run on as many threads as the game is given, and every
//! pixel comes out the same whatever the thread count.
//!
//! The framebuffer is `scale` times the logical 640x400 in each direction.
//! The 3D view renders at full framebuffer resolution; the HUD is drawn in
//! logical pixels and scaled up by [`Game::put_pixel`] / [`Game::fill_rect`],
//! so it stays crisp pixel art at any size.

use crate::color::*;
use crate::constants::*;
use crate::game::Game;
use crate::lighting::*;
use crate::sprites::Item;
use crate::textures::*;
use std::sync::{Mutex, OnceLock};

/// Floor rows sample the lightmap every `SPAN` logical pixels and interpolate
/// light linearly in between (the floor position is linear along a row, so
/// this is close to exact and saves a bilinear lightmap fetch per pixel).
const SPAN: usize = 16;
const SPANS: usize = SCREEN_W / SPAN;
/// Ceiling light relative to the floor below it (256 = equal).
const CEIL_LIGHT: i32 = 218;
/// North/south-facing walls get this share of the light — a fixed "sun"
/// direction so corners read as corners.
const SIDE_LIGHT: i32 = 205;
/// Logical rows per band, the unit of work threads pull.
const BAND_ROWS: usize = 16;
/// How far mip selection leans towards the more compressed axis of a surface
/// seen at a glancing angle: 0 picks by the sharper axis (crisp, shimmers),
/// 1 by the blurrier one (stable, smeared). A quarter keeps floors readable.
const ANISO: f64 = 0.25;

/// Parts of the view, for the benchmark's per-stage timing.
pub const STAGE_WALLS: u8 = 1;
pub const STAGE_FLATS: u8 = 2;
pub const STAGE_SPRITES: u8 = 4;
pub const STAGE_ALL: u8 = 7;

/// One framebuffer column's wall, resolved once per frame.
#[derive(Clone, Copy)]
pub struct WallCol {
    /// First and last framebuffer rows of the band (inclusive, clipped).
    top: i32,
    bot: i32,
    tex: &'static [u32],
    bits: u32,
    x0: usize,
    x1: usize,
    wu: u32,
    /// Texture V at row `top` and its step per row, fixed point.
    v_top: i64,
    dv: i64,
    lit: Lit,
}

impl WallCol {
    pub const EMPTY: WallCol =
        WallCol { top: 0, bot: -1, tex: &[], bits: 0, x0: 0, x1: 0, wu: 0, v_top: 0, dv: 0, lit: Lit { k: [0; 3], ke: 0, add: [0; 3] } };
}

/// Everything a band needs to render, fixed for the frame.
pub struct View<'a> {
    pub w: usize,
    pub h: usize,
    pub scale: usize,
    cols: &'a [WallCol],
    /// Wall extents per column, kept apart from `cols` so the flat pass's
    /// visibility test reads a compact array.
    pub top: &'a [i32],
    pub bot: &'a [i32],
    pub depth: &'a [f64],
    env: &'a Environment,
    /// Texture per map cell (plus "outside") at each mip level.
    floor_tex: Vec<[&'static [u32]; OUTSIDE + 1]>,
    ceil_tex: Vec<[&'static [u32]; OUTSIDE + 1]>,
    sky: Option<&'static SkyTex>,
    /// Sky texel pair and blend weight per column.
    sky_cols: Vec<(usize, usize, u32)>,
    px: f64,
    py: f64,
    ray_left_x: f64,
    ray_left_y: f64,
    span_x: f64,
    span_y: f64,
    pub items: &'a [Item],
}

/// Mip level whose texels are about `step` base texels wide.
fn mip_for(step: f64) -> usize {
    if step <= 1.0 {
        0
    } else {
        (step.log2() as usize).min(TEX_MIPS - 1)
    }
}

/// Blend two `0x00RRGGBB` colours by `w` (0..=256 towards `b`).
#[inline(always)]
fn lerp_rgb(a: u32, b: u32, w: u32) -> u32 {
    let iw = 256 - w;
    let ch = |s: u32| ((((a >> s) & 0xFF) * iw + ((b >> s) & 0xFF) * w) >> 8) << s;
    ch(16) | ch(8) | ch(0)
}

/// Angle of each column's ray relative to the view direction, for a
/// framebuffer `w` columns wide. Constant per width, so the sky's per-column
/// lookup needs no trig per frame.
fn column_angles(w: usize) -> &'static [f64] {
    static ANGLES: [OnceLock<Vec<f64>>; MAX_SCALE + 1] = [const { OnceLock::new() }; MAX_SCALE + 1];
    ANGLES[(w / SCREEN_W).min(MAX_SCALE)].get_or_init(|| {
        let plane_half = (FOV / 2.0).tan();
        (0..w).map(|x| (((2 * x + 1) as f64 / w as f64 - 1.0) * plane_half).atan()).collect()
    })
}

impl View<'_> {
    /// Render rows `y0..` of the frame into `band` (whole rows).
    fn render_band(&self, band: &mut [u32], y0: usize, stages: u8) {
        let w = self.w;
        let y1 = y0 + band.len() / w;
        if stages & STAGE_WALLS != 0 {
            for (x, c) in self.cols.iter().enumerate() {
                let c = *c; // by value: the loop below keeps it in registers
                let a = c.top.max(y0 as i32);
                let b = c.bot.min(y1 as i32 - 1);
                if a > b {
                    continue;
                }
                let mut v = c.v_top + (a - c.top) as i64 * c.dv;
                let mut idx = (a as usize - y0) * w + x;
                for _ in a..=b {
                    band[idx] = shade4(fetch_col(c.tex, c.bits, c.x0, c.x1, c.wu, v), &c.lit);
                    v += c.dv;
                    idx += w;
                }
            }
        }
        if stages & STAGE_FLATS != 0 {
            for (i, row) in band.chunks_mut(w).enumerate() {
                self.flat_row(y0 + i, row);
            }
        }
        if stages & STAGE_SPRITES != 0 {
            for item in self.items {
                item.draw(self, band, y0 as i32, y1 as i32);
            }
        }
    }

    /// One row of floor, ceiling or sky: every pixel in it that no wall
    /// covers. The camera distance is constant along a row, so the texture
    /// coordinate steps linearly and fog and mip level are one value each.
    fn flat_row(&self, y: usize, out: &mut [u32]) {
        let (w, h) = (self.w, self.h);
        let half = h / 2;
        let yi = y as i32;
        let is_floor = y > half;
        // Rows below the horizon are floor; rows above mirror them as ceiling.
        let d_rows = if is_floor {
            y - half
        } else if y + 1 < half {
            h - 1 - y - half
        } else {
            // The two rows on the horizon. A wall always spans them unless a
            // ray left the map; plug any such gap with fog.
            let fog = (self.env.fog[0] << 16) | (self.env.fog[1] << 8) | self.env.fog[2];
            for (x, px) in out.iter_mut().enumerate() {
                if yi < self.top[x] || yi > self.bot[x] {
                    *px = fog;
                }
            }
            return;
        };

        if let (false, Some(sk)) = (is_floor, self.sky) {
            // Sky: fixed to the view direction, blended between its rows.
            let fy = ((y as f64 + 0.5) * SKY_H as f64 / half as f64 - 0.5).clamp(0.0, (SKY_H - 1) as f64);
            let r0 = fy as usize;
            let r1 = (r0 + 1).min(SKY_H - 1);
            let wv = ((fy - r0 as f64) * 256.0) as u32;
            let (row0, row1) = (r0 * SKY_W, r1 * SKY_W);
            for (x, px) in out.iter_mut().enumerate() {
                if yi < self.top[x] {
                    let (u0, u1, wu) = self.sky_cols[x];
                    *px = lerp_rgb(lerp_rgb(sk[row0 + u0], sk[row0 + u1], wu), lerp_rgb(sk[row1 + u0], sk[row1 + u1], wu), wv);
                }
            }
            return;
        }

        let half_h = h as f64 / 2.0;
        let row_dist = half_h / d_rows as f64;
        let wx0 = self.px + row_dist * self.ray_left_x;
        let wy0 = self.py + row_dist * self.ray_left_y;
        let dwx = row_dist * self.span_x / w as f64;
        let dwy = row_dist * self.span_y / w as f64;

        // Mip level: texels per pixel along the row, nudged towards the much
        // denser spacing between rows at a glancing angle.
        let along = dwx.hypot(dwy) * TEX_SIZE as f64;
        let across = TEX_SIZE as f64 * half_h / (d_rows * d_rows) as f64;
        let level = mip_for(along * (across / along).max(1.0).powf(ANISO));
        let bits = MipTex::bits(level);
        let ts = (1u64 << bits) as f64 * FP_ONE as f64;
        let mut fu = ((wx0 * (1u64 << bits) as f64 - 0.5) * FP_ONE as f64) as i64;
        let mut fv = ((wy0 * (1u64 << bits) as f64 - 0.5) * FP_ONE as f64) as i64;
        let du = (dwx * ts) as i64;
        let dv = (dwy * ts) as i64;
        let texes = if is_floor { &self.floor_tex[level] } else { &self.ceil_tex[level] };

        let vis = self.env.fog_vis(row_dist);
        let glow = if is_floor { self.env.flicker[LAYER_LAVA] } else { 1.0 };
        let base = self.env.lit([0; 3], vis, glow);
        let lmul = if is_floor { 256 } else { CEIL_LIGHT };
        let extent = if is_floor { self.bot } else { self.top };

        // Light at span boundaries, fetched only for spans with something
        // visible: near the horizon most of a row is wall.
        let span = SPAN * self.scale;
        let mut edge = [[0i32; 3]; SPANS + 1];
        let mut have = [false; SPANS + 1];
        for s in 0..SPANS {
            let xs = s * span;
            let covered = |x: usize| if is_floor { yi <= extent[x] } else { yi >= extent[x] };
            if (xs..xs + span).all(covered) {
                fu += du * span as i64;
                fv += dv * span as i64;
                continue;
            }
            for e in [s, s + 1] {
                if !have[e] {
                    let xe = (e * span) as f64;
                    let l = self.env.light_at(wx0 + dwx * xe, wy0 + dwy * xe);
                    edge[e] = l.map(|c| (((c.max(0) * vis as i32) >> 8) * lmul) >> 8);
                    have[e] = true;
                }
            }
            // Light interpolation in 8 extra fractional bits.
            let mut lf = edge[s].map(|c| c << 8);
            let dl: [i32; 3] = std::array::from_fn(|c| ((edge[s + 1][c] - edge[s][c]) << 8) / span as i32);
            // `x` indexes the wall extents as well as the row.
            #[allow(clippy::needless_range_loop)]
            for x in xs..xs + span {
                if !covered(x) {
                    let cx = (fu >> FP_BITS) >> bits;
                    let cy = (fv >> FP_BITS) >> bits;
                    let ci = if (cx as u64) < MAP_W as u64 && (cy as u64) < MAP_H as u64 {
                        cy as usize * MAP_W + cx as usize
                    } else {
                        OUTSIDE
                    };
                    let lit = Lit { k: lf.map(|c| (c >> 8) as u32), ke: base.ke, add: base.add };
                    out[x] = shade4(fetch_bilinear(texes[ci], bits, fu, fv), &lit);
                }
                fu += du;
                fv += dv;
                for c in 0..3 {
                    lf[c] += dl[c];
                }
            }
        }
    }
}

impl Environment {
    /// How much of a surface `d` units away shows through the fog, 0..=256.
    #[inline]
    pub fn fog_vis(&self, d: f64) -> u32 {
        ((-self.fog_density * d.max(0.0)).exp() * 256.0) as u32
    }

    /// Shading recipe for a surface with lightmap value `light`, fog
    /// visibility `vis` and glow multiplier `glow` (the flicker of whatever
    /// light the surface belongs to).
    #[inline]
    pub fn lit(&self, light: [i32; 3], vis: u32, glow: f64) -> Lit {
        Lit {
            k: light.map(|l| (l.max(0) as u32 * vis) >> 8),
            ke: (vis as f64 * glow) as u32,
            add: self.fog.map(|f| (f * (256 - vis)) >> 8),
        }
    }
}

impl Game {
    /// Set a logical pixel: a `scale`-sized block of the framebuffer.
    #[inline]
    pub fn put_pixel(&mut self, x: i32, y: i32, c: u32) {
        if (x as u32) < SCREEN_W as u32 && (y as u32) < SCREEN_H as u32 {
            let s = self.scale;
            let (x, y) = (x as usize * s, y as usize * s);
            for row in y..y + s {
                let i = row * self.fb_w + x;
                self.pixels[i..i + s].fill(c);
            }
        }
    }

    /// Fill a rectangle given in logical pixels, clipped to the screen.
    pub fn fill_rect(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: u32) {
        let s = self.scale;
        let x1 = (x0 + w).min(SCREEN_W as i32);
        let y1 = (y0 + h).min(SCREEN_H as i32);
        let (x0, y0) = (x0.max(0), y0.max(0));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let (xa, xb) = (x0 as usize * s, x1 as usize * s);
        for y in y0 as usize * s..y1 as usize * s {
            let row = y * self.fb_w;
            self.pixels[row + xa..row + xb].fill(c);
        }
    }

    /// How much of a surface `d` units away shows through the fog, 0..=256.
    pub fn fog_visibility(&self, d: f64) -> u32 {
        self.env.fog_vis(d)
    }

    /// Cast one framebuffer column: DDA to the first wall, then record its
    /// depth, extent, texture and light for the band renderer. The ray basis
    /// (`dir_*`, `plane_*`) is constant per frame, so it's computed once and
    /// passed in rather than recomputing trig for every column.
    pub fn cast_column(&mut self, col: usize, dir_x: f64, dir_y: f64, plane_x: f64, plane_y: f64) {
        let (w, h) = (self.fb_w, self.fb_h as i32);
        let camera_x = 2.0 * col as f64 / w as f64 - 1.0;
        let ray_dir_x = dir_x + plane_x * camera_x;
        let ray_dir_y = dir_y + plane_y * camera_x;

        let mut map_x = self.player.x as i32;
        let mut map_y = self.player.y as i32;

        let delta_x = if ray_dir_x == 0.0 { 1e30 } else { (1.0 / ray_dir_x).abs() };
        let delta_y = if ray_dir_y == 0.0 { 1e30 } else { (1.0 / ray_dir_y).abs() };

        let (step_x, mut side_x) = if ray_dir_x < 0.0 {
            (-1, (self.player.x - map_x as f64) * delta_x)
        } else {
            (1, (map_x as f64 + 1.0 - self.player.x) * delta_x)
        };
        let (step_y, mut side_y) = if ray_dir_y < 0.0 {
            (-1, (self.player.y - map_y as f64) * delta_y)
        } else {
            (1, (map_y as f64 + 1.0 - self.player.y) * delta_y)
        };

        let mut side = 0;
        let mut wall_type = WALL_STONE;
        for _ in 0..128 {
            if side_x < side_y {
                side_x += delta_x;
                map_x += step_x;
                side = 0;
            } else {
                side_y += delta_y;
                map_y += step_y;
                side = 1;
            }
            let t = self.map_wall_type(map_x, map_y);
            if t != WALL_NONE {
                wall_type = t;
                break;
            }
        }

        let perp_dist = if side == 0 { side_x - delta_x } else { side_y - delta_y }.max(0.0001);
        self.depth[col] = perp_dist;

        let line_h = (h as f64 / perp_dist) as i32;
        let draw_start = -line_h / 2 + h / 2;
        let draw_end = line_h / 2 + h / 2;
        let clip_start = draw_start.max(0);
        let clip_end = draw_end.min(h - 1);
        self.wall_top[col] = clip_start;
        self.wall_bot[col] = clip_end;

        // Where along the face the ray hit (texture U), a point just in front
        // of the face for the light lookup, and how fast U moves per column.
        let hit_x = self.player.x + perp_dist * ray_dir_x;
        let hit_y = self.player.y + perp_dist * ray_dir_y;
        let cross = plane_y * ray_dir_x - plane_x * ray_dir_y;
        let (face, wall_u, lx, ly, u_per_col) = if side == 0 {
            let face = if step_x > 0 { FACE_NEG_X } else { FACE_POS_X };
            let lx = map_x as f64 + if step_x > 0 { -0.02 } else { 1.02 };
            (face, hit_y, lx, hit_y, perp_dist * cross / ray_dir_x)
        } else {
            let face = if step_y > 0 { FACE_NEG_Y } else { FACE_POS_Y };
            let ly = map_y as f64 + if step_y > 0 { -0.02 } else { 1.02 };
            (face, hit_x, hit_x, ly, perp_dist * cross / ray_dir_y)
        };

        // Mip level: texels per pixel down the column, nudged towards the
        // horizontal spacing when the wall is seen at a glancing angle.
        let v_step = TEX_SIZE as f64 / line_h.max(1) as f64;
        let h_step = (u_per_col * 2.0 / w as f64).abs() * TEX_SIZE as f64;
        let level = mip_for(v_step * (h_step / v_step).max(1.0).powf(ANISO));
        let bits = MipTex::bits(level);
        let ts = (1u32 << bits) as f64;

        let mut tex_uf = (wall_u - wall_u.floor()) * ts;
        if (side == 0 && ray_dir_x > 0.0) || (side == 1 && ray_dir_y < 0.0) {
            tex_uf = ts - tex_uf;
        }

        let in_map = (0..MAP_W as i32).contains(&map_x) && (0..MAP_H as i32).contains(&map_y);
        let cell = if in_map { map_y as usize * MAP_W + map_x as usize } else { 0 };
        let (variant, layer) = if in_map {
            (self.env.face_variant[cell][face] as usize, self.env.face_layer[cell][face] as usize)
        } else {
            (WV_BASE, LAYER_STEADY)
        };

        let mut light = self.env.light_at(lx, ly);
        if side == 1 {
            light = light.map(|l| (l * SIDE_LIGHT) >> 8);
        }
        let lit = self.env.lit(light, self.env.fog_vis(perp_dist), self.env.flicker[layer.min(LAYERS - 1)]);

        // The horizontal texel pair and X weight are constant down the column.
        let fu = tex_uf - 0.5;
        let u0 = fu.floor();
        let wu = ((fu - u0) * 256.0) as u32;
        let mask = (1i32 << bits) - 1;
        let step = ts / line_h.max(1) as f64;
        let start_v = (clip_start - h / 2 + line_h / 2) as f64 * step - 0.5;
        self.cols[col] = WallCol {
            top: clip_start,
            bot: clip_end,
            tex: &self.tex.wall[wall_type][variant].levels[level],
            bits,
            x0: (u0 as i32 & mask) as usize,
            x1: ((u0 as i32 + 1) & mask) as usize,
            wu,
            v_top: (start_v * FP_ONE as f64) as i64,
            dv: (step * FP_ONE as f64) as i64,
            lit,
        };
    }

    /// Render the 3D view (the stages named in `stages`) into the framebuffer,
    /// on `self.threads` threads. Lighting must be current (`update_lighting`).
    pub fn render_view(&mut self, stages: u8) {
        // Ray basis is constant across the frame — compute the trig once.
        let dir_x = self.player.angle.cos();
        let dir_y = self.player.angle.sin();
        let plane_half = (FOV / 2.0).tan();
        let plane_x = -dir_y * plane_half;
        let plane_y = dir_x * plane_half;

        for x in 0..self.fb_w {
            self.cast_column(x, dir_x, dir_y, plane_x, plane_y);
        }
        let items = if stages & STAGE_SPRITES != 0 { self.sprite_items() } else { Vec::new() };

        let (w, h) = (self.fb_w, self.fb_h);
        let env = &self.env;
        let tex = self.tex;
        let per_level = |flats: &[u8; OUTSIDE + 1]| -> Vec<[&'static [u32]; OUTSIDE + 1]> {
            (0..TEX_MIPS).map(|l| std::array::from_fn(|i| &*tex.flats[flats[i] as usize].levels[l])).collect()
        };
        let sky_tex = env.sky.map(sky);
        let mut sky_cols = Vec::new();
        if sky_tex.is_some() {
            let turn = std::f64::consts::TAU;
            sky_cols = column_angles(w)
                .iter()
                .map(|a| {
                    let u = ((self.player.angle + a) / turn).rem_euclid(1.0) * SKY_W as f64;
                    let u0 = u.floor();
                    (u0 as usize % SKY_W, (u0 as usize + 1) % SKY_W, ((u - u0) * 256.0) as u32)
                })
                .collect();
        }
        let view = View {
            w,
            h,
            scale: self.scale,
            cols: &self.cols,
            top: &self.wall_top,
            bot: &self.wall_bot,
            depth: &self.depth,
            env,
            floor_tex: per_level(&env.floor_flat),
            ceil_tex: per_level(&env.ceil_flat),
            sky: sky_tex,
            sky_cols,
            px: self.player.x,
            py: self.player.y,
            ray_left_x: dir_x - plane_x,
            ray_left_y: dir_y - plane_y,
            span_x: 2.0 * plane_x,
            span_y: 2.0 * plane_y,
            items: &items,
        };

        let rows = BAND_ROWS * self.scale;
        let bands = self.pixels.chunks_mut(rows * w).enumerate();
        if self.threads <= 1 {
            for (i, band) in bands {
                view.render_band(band, i * rows, stages);
            }
        } else {
            // Threads pull bands from a shared queue, so a cheap band of sky
            // doesn't leave a thread idle while another grinds through walls.
            let queue = Mutex::new(bands);
            let work = || loop {
                let next = queue.lock().unwrap_or_else(|e| e.into_inner()).next();
                match next {
                    Some((i, band)) => view.render_band(band, i * rows, stages),
                    None => break,
                }
            };
            std::thread::scope(|s| {
                for _ in 1..self.threads {
                    s.spawn(work);
                }
                work();
            });
        }
    }

    pub fn post_process(&mut self) {
        if self.pain_flash > 0.0 {
            let a = self.pain_flash.min(0.4);
            // 0..=256 fixed-point factors, hoisted so the full-screen loop is
            // pure integer (it runs on every combat frame; the old per-pixel
            // f64 version was a ~1 ms spike exactly when the player is hurt).
            let af = (a * 256.0) as u32; // red lift, ≤ 102
            let gf = ((1.0 - a * 0.4) * 256.0) as u32; // green/blue scale
            for px in self.pixels.iter_mut() {
                let c = *px;
                let r = (c >> 16) & 0xFF;
                let g = (c >> 8) & 0xFF;
                let b = c & 0xFF;
                let r = r + (((255 - r) * af) >> 8); // ≤ 255 by construction
                let g = (g * gf) >> 8;
                let b = (b * gf) >> 8;
                *px = (r << 16) | (g << 8) | b;
            }
        }
    }

    pub fn render_frame(&mut self) {
        self.update_lighting();
        self.render_view(STAGE_ALL);
        self.draw_overlay();

        if self.score_saved {
            self.draw_game_over_overlay();
        } else if self.player.health > 0
            && self.all_enemies_dead()
            && (self.level as usize + 1) < LEVEL_COUNT
        {
            self.draw_banner(crate::i18n::tr(self.lang, crate::i18n::Msg::LevelClear), 60, 0x40FF40);
        }

        if self.show_intro {
            self.draw_intro();
        }
    }

    /// Everything drawn over the view every frame: crosshair, gun, HUD bar,
    /// minimap, score and the pain flash.
    pub fn draw_overlay(&mut self) {
        self.draw_crosshair();
        self.gun_light = self.env.light_at(self.player.x, self.player.y);
        self.draw_weapon();
        self.draw_hud();
        self.draw_minimap();
        self.draw_score_readout();
        self.post_process();
    }
}
