//! The 3D view and full-frame composition.
//!
//! Walls go first: one DDA ray per screen column finds the wall, paints its
//! textured band and records how far up and down the band reaches. The floor
//! and ceiling (or sky) are then filled a row at a time — only the pixels the
//! walls left uncovered, so no pixel is shaded twice. Everything is lit from
//! the level's lightmap and faded into its fog (see `lighting.rs`).

use crate::color::*;
use crate::constants::*;
use crate::game::Game;
use crate::lighting::*;
use crate::textures::*;
use std::sync::OnceLock;

/// Floor rows sample the lightmap every `SPAN` pixels and interpolate light
/// linearly in between (the floor position is linear along a row, so this is
/// close to exact and saves a bilinear lightmap fetch per pixel).
const SPAN: usize = 16;
const SPANS: usize = SCREEN_W / SPAN;
/// Ceiling light relative to the floor below it (256 = equal).
const CEIL_LIGHT: i32 = 218;
/// North/south-facing walls get this share of the light — a fixed "sun"
/// direction so corners read as corners.
const SIDE_LIGHT: i32 = 205;
/// log2(TEX_SIZE): texel coordinate to map cell is a shift.
const TEX_SHIFT: u32 = TEX_SIZE.trailing_zeros();

/// Angle of each screen column's ray relative to the view direction. Constant,
/// so the sky's per-column lookup needs no trig per frame.
fn column_angles() -> &'static [f64] {
    static ANGLES: OnceLock<Vec<f64>> = OnceLock::new();
    ANGLES.get_or_init(|| {
        let plane_half = (FOV / 2.0).tan();
        (0..SCREEN_W)
            .map(|x| (((2 * x + 1) as f64 / SCREEN_W as f64 - 1.0) * plane_half).atan())
            .collect()
    })
}

impl Game {
    #[inline]
    pub fn put_pixel(&mut self, x: i32, y: i32, c: u32) {
        if (x as u32) < SCREEN_W as u32 && (y as u32) < SCREEN_H as u32 {
            self.pixels[y as usize * SCREEN_W + x as usize] = c;
        }
    }

    pub fn fill_rect(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: u32) {
        let mut x0 = x0;
        let mut y0 = y0;
        let mut x1 = x0 + w;
        let mut y1 = y0 + h;
        if x0 < 0 {
            x0 = 0;
        }
        if y0 < 0 {
            y0 = 0;
        }
        if x1 > SCREEN_W as i32 {
            x1 = SCREEN_W as i32;
        }
        if y1 > SCREEN_H as i32 {
            y1 = SCREEN_H as i32;
        }
        for y in y0..y1 {
            for x in x0..x1 {
                self.pixels[y as usize * SCREEN_W + x as usize] = c;
            }
        }
    }

    /// How much of a surface `d` units away shows through the fog, 0..=256.
    #[inline]
    pub fn fog_visibility(&self, d: f64) -> u32 {
        ((-self.env.fog_density * d.max(0.0)).exp() * 256.0) as u32
    }

    /// Shading recipe for a surface with lightmap value `light`, fog
    /// visibility `vis` and glow multiplier `glow` (the flicker of whatever
    /// light the surface belongs to).
    #[inline]
    pub fn lit_for(&self, light: [i32; 3], vis: u32, glow: f64) -> Lit {
        let fog = self.env.fog;
        Lit {
            k: light.map(|l| (l.max(0) as u32 * vis) >> 8),
            ke: (vis as f64 * glow) as u32,
            add: fog.map(|f| (f * (256 - vis)) >> 8),
        }
    }

    /// Cast one screen column: DDA to the first wall, then draw the textured
    /// wall band and record its depth and extent. The ray basis (`dir_*`,
    /// `plane_*`) is constant per frame, so it's computed once in
    /// `render_frame` and passed in rather than recomputing trig for all 640
    /// columns.
    pub fn cast_column(&mut self, col: usize, dir_x: f64, dir_y: f64, plane_x: f64, plane_y: f64) {
        let camera_x = 2.0 * col as f64 / SCREEN_W as f64 - 1.0;
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

        let line_h = (SCREEN_H as f64 / perp_dist) as i32;
        let draw_start = -line_h / 2 + SCREEN_H as i32 / 2;
        let draw_end = line_h / 2 + SCREEN_H as i32 / 2;
        let clip_start = draw_start.max(0);
        let clip_end = draw_end.min(SCREEN_H as i32 - 1);
        self.wall_top[col] = clip_start;
        self.wall_bot[col] = clip_end;

        // Where along the face the ray hit (texture U), and a point just in
        // front of the face for the light lookup.
        let hit_x = self.player.x + perp_dist * ray_dir_x;
        let hit_y = self.player.y + perp_dist * ray_dir_y;
        let (face, wall_u, lx, ly) = if side == 0 {
            let face = if step_x > 0 { FACE_NEG_X } else { FACE_POS_X };
            let lx = map_x as f64 + if step_x > 0 { -0.02 } else { 1.02 };
            (face, hit_y, lx, hit_y)
        } else {
            let face = if step_y > 0 { FACE_NEG_Y } else { FACE_POS_Y };
            let ly = map_y as f64 + if step_y > 0 { -0.02 } else { 1.02 };
            (face, hit_x, hit_x, ly)
        };
        let mut tex_uf = (wall_u - wall_u.floor()) * TEX_SIZE as f64;
        if (side == 0 && ray_dir_x > 0.0) || (side == 1 && ray_dir_y < 0.0) {
            tex_uf = TEX_SIZE as f64 - tex_uf;
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
        let lit = self.lit_for(light, self.fog_visibility(perp_dist), self.env.flicker[layer.min(LAYERS - 1)]);

        // The horizontal texel pair and X weight are constant down the column;
        // resolve them once here instead of per pixel.
        let fu = tex_uf - 0.5;
        let u0 = fu.floor();
        let wu = ((fu - u0) * 256.0) as u32;
        let mask = (TEX_SIZE - 1) as i32;
        let x0 = (u0 as i32 & mask) as usize;
        let x1 = ((u0 as i32 + 1) & mask) as usize;

        let tex: &'static Tex = &self.tex.wall[wall_type][variant];
        let step = TEX_SIZE as f64 / line_h.max(1) as f64;
        let start_v = (clip_start - SCREEN_H as i32 / 2 + line_h / 2) as f64 * step - 0.5;
        let mut v = (start_v * FP_ONE as f64) as i64;
        let dv = (step * FP_ONE as f64) as i64;
        let mut idx = clip_start as usize * SCREEN_W + col;
        for _ in clip_start..=clip_end {
            self.pixels[idx] = shade4(fetch_col(tex, x0, x1, wu, v), &lit);
            v += dv;
            idx += SCREEN_W;
        }
    }

    /// Fill the floor (lower half) and the ceiling or sky (upper half), a row
    /// pair at a time, skipping every pixel a wall already covers. For a given
    /// screen row the camera distance is constant, so the texture coordinate
    /// steps linearly across the row and the fog is one value per row.
    pub fn render_floor_ceiling(&mut self, dir_x: f64, dir_y: f64, plane_x: f64, plane_y: f64) {
        let env = &self.env;
        let tex: &'static Textures = self.tex;
        // Texture per map cell (plus "outside"), resolved once per frame.
        let floor_tex: [&'static Tex; OUTSIDE + 1] = std::array::from_fn(|i| &*tex.flats[env.floor_flat[i] as usize]);
        let ceil_tex: [&'static Tex; OUTSIDE + 1] = std::array::from_fn(|i| &*tex.flats[env.ceil_flat[i] as usize]);
        let sky_tex = env.sky.map(sky);

        // Sky column lookup: texel pair and blend weight for each column.
        let mut sky_cols = [(0usize, 0usize, 0u32); SCREEN_W];
        if sky_tex.is_some() {
            let angles = column_angles();
            let turn = std::f64::consts::TAU;
            for (x, sc) in sky_cols.iter_mut().enumerate() {
                let u = ((self.player.angle + angles[x]) / turn).rem_euclid(1.0) * SKY_W as f64;
                let u0 = u.floor();
                *sc = (u0 as usize % SKY_W, (u0 as usize + 1) % SKY_W, ((u - u0) * 256.0) as u32);
            }
        }

        let (px, py) = (self.player.x, self.player.y);
        let ray_left_x = dir_x - plane_x;
        let ray_left_y = dir_y - plane_y;
        let span_x = 2.0 * plane_x; // ray_right - ray_left, in X
        let span_y = 2.0 * plane_y;
        let half_h = SCREEN_H as f64 / 2.0;
        let inv_w = 1.0 / SCREEN_W as f64;
        let ts = TEX_SIZE as f64 * FP_ONE as f64;
        let floor_glow = env.flicker[LAYER_LAVA];

        for y in (SCREEN_H / 2 + 1)..SCREEN_H {
            let yc = SCREEN_H - 1 - y; // the mirrored ceiling row
            let row_dist = half_h / (y as f64 - half_h);
            let wx0 = px + row_dist * ray_left_x;
            let wy0 = py + row_dist * ray_left_y;
            let dwx = row_dist * span_x * inv_w;
            let dwy = row_dist * span_y * inv_w;

            let vis = self.fog_visibility(row_dist);
            let base = self.lit_for([0; 3], vis, floor_glow);
            let ke_ceil = vis;

            // Light at each span boundary, already through the fog.
            let mut kf = [[0i32; 3]; SPANS + 1];
            let mut kc = [[0i32; 3]; SPANS + 1];
            for s in 0..=SPANS {
                let xs = (s * SPAN) as f64;
                let l = env.light_at(wx0 + dwx * xs, wy0 + dwy * xs);
                kf[s] = l.map(|c| (c.max(0) * vis as i32) >> 8);
                kc[s] = kf[s].map(|c| (c * CEIL_LIGHT) >> 8);
            }

            let mut fu = ((wx0 * TEX_SIZE as f64 - 0.5) * FP_ONE as f64) as i64;
            let mut fv = ((wy0 * TEX_SIZE as f64 - 0.5) * FP_ONE as f64) as i64;
            let du = (dwx * ts) as i64;
            let dv = (dwy * ts) as i64;

            let (above, below) = self.pixels.split_at_mut(y * SCREEN_W);
            let ceil_out = &mut above[yc * SCREEN_W..yc * SCREEN_W + SCREEN_W];
            let floor_out = &mut below[..SCREEN_W];
            let (y_i, yc_i) = (y as i32, yc as i32);

            for s in 0..SPANS {
                // Light interpolation in 8 extra fractional bits.
                let mut lf = kf[s].map(|c| c << 8);
                let mut lc = kc[s].map(|c| c << 8);
                let dlf: [i32; 3] = std::array::from_fn(|c| ((kf[s + 1][c] - kf[s][c]) << 8) / SPAN as i32);
                let dlc: [i32; 3] = std::array::from_fn(|c| ((kc[s + 1][c] - kc[s][c]) << 8) / SPAN as i32);
                for x in s * SPAN..(s + 1) * SPAN {
                    let show_floor = y_i > self.wall_bot[x];
                    let show_ceil = yc_i < self.wall_top[x];
                    if show_floor || show_ceil {
                        let cx = (fu >> FP_BITS) >> TEX_SHIFT;
                        let cy = (fv >> FP_BITS) >> TEX_SHIFT;
                        let ci = if (cx as u64) < MAP_W as u64 && (cy as u64) < MAP_H as u64 {
                            cy as usize * MAP_W + cx as usize
                        } else {
                            OUTSIDE
                        };
                        if show_floor {
                            let lit = Lit { k: lf.map(|c| (c >> 8) as u32), ke: base.ke, add: base.add };
                            floor_out[x] = shade4(fetch_bilinear(floor_tex[ci], fu, fv), &lit);
                        }
                        if show_ceil {
                            ceil_out[x] = match sky_tex {
                                Some(sk) => {
                                    let (u0, u1, w) = sky_cols[x];
                                    let row = yc * SKY_W;
                                    lerp_rgb(sk[row + u0], sk[row + u1], w)
                                }
                                None => {
                                    let lit = Lit { k: lc.map(|c| (c >> 8) as u32), ke: ke_ceil, add: base.add };
                                    shade4(fetch_bilinear(ceil_tex[ci], fu, fv), &lit)
                                }
                            };
                        }
                    }
                    fu += du;
                    fv += dv;
                    for c in 0..3 {
                        lf[c] += dlf[c];
                        lc[c] += dlc[c];
                    }
                }
            }
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

        // Ray basis is constant across the frame — compute the trig once here
        // instead of per column / per floor row.
        let dir_x = self.player.angle.cos();
        let dir_y = self.player.angle.sin();
        let plane_half = (FOV / 2.0).tan();
        let plane_x = -dir_y * plane_half;
        let plane_y = dir_x * plane_half;

        for x in 0..SCREEN_W {
            self.cast_column(x, dir_x, dir_y, plane_x, plane_y);
        }
        self.render_floor_ceiling(dir_x, dir_y, plane_x, plane_y);
        self.render_sprites();
        self.draw_crosshair();
        self.gun_light = self.env.light_at(self.player.x, self.player.y);
        self.draw_weapon();
        self.draw_hud();
        self.draw_minimap();
        self.draw_score_readout();
        self.post_process();

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
}

/// Blend two `0x00RRGGBB` colours by `w` (0..=256 towards `b`).
#[inline(always)]
fn lerp_rgb(a: u32, b: u32, w: u32) -> u32 {
    let iw = 256 - w;
    let ch = |s: u32| ((((a >> s) & 0xFF) * iw + ((b >> s) & 0xFF) * w) >> 8) << s;
    ch(16) | ch(8) | ch(0)
}
