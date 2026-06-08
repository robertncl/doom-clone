//! Pixel/rect helpers, the DDA raycaster (textured walls + floor/ceiling cast),
//! the pain-flash post-process, and full-frame composition.

use crate::color::{make_color, sample_tex_bilinear_shaded};
use crate::constants::*;
use crate::game::Game;

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

    /// Cast one screen column: DDA to the first wall, then draw the textured
    /// wall band and record its depth. Floor/ceiling are filled separately by
    /// [`Game::render_floor_ceiling`]. The ray basis (`dir_*`, `plane_*`) is
    /// constant per frame, so it's computed once in `render_frame` and passed in
    /// rather than recomputing trig for all 640 columns.
    pub fn cast_column(&mut self, col: usize, dir_x: f64, dir_y: f64, plane_x: f64, plane_y: f64) {
        let camera_x = 2.0 * col as f64 / SCREEN_W as f64 - 1.0;
        let ray_dir_x = dir_x + plane_x * camera_x;
        let ray_dir_y = dir_y + plane_y * camera_x;

        let mut map_x = self.player.x as i32;
        let mut map_y = self.player.y as i32;

        let delta_x = if ray_dir_x == 0.0 { 1e30 } else { (1.0 / ray_dir_x).abs() };
        let delta_y = if ray_dir_y == 0.0 { 1e30 } else { (1.0 / ray_dir_y).abs() };

        let step_x;
        let step_y;
        let mut side_x;
        let mut side_y;

        if ray_dir_x < 0.0 {
            step_x = -1;
            side_x = (self.player.x - map_x as f64) * delta_x;
        } else {
            step_x = 1;
            side_x = (map_x as f64 + 1.0 - self.player.x) * delta_x;
        }
        if ray_dir_y < 0.0 {
            step_y = -1;
            side_y = (self.player.y - map_y as f64) * delta_y;
        } else {
            step_y = 1;
            side_y = (map_y as f64 + 1.0 - self.player.y) * delta_y;
        }

        let mut hit = false;
        let mut side = 0;
        let mut iter = 0;
        let mut wall_type = WALL_STONE;
        while !hit && iter < 128 {
            iter += 1;
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
            if t != 0 {
                hit = true;
                wall_type = t;
            }
        }

        let mut perp_dist = if side == 0 { side_x - delta_x } else { side_y - delta_y };
        if perp_dist < 0.0001 {
            perp_dist = 0.0001;
        }
        self.depth[col] = perp_dist;

        let line_h = (SCREEN_H as f64 / perp_dist) as i32;
        let draw_start = -line_h / 2 + SCREEN_H as i32 / 2;
        let draw_end = line_h / 2 + SCREEN_H as i32 / 2;
        let clip_start = if draw_start < 0 { 0 } else { draw_start };
        let clip_end = if draw_end >= SCREEN_H as i32 { SCREEN_H as i32 - 1 } else { draw_end };

        // Wall U coord at hit
        let mut wall_hit_x = if side == 0 {
            self.player.y + perp_dist * ray_dir_y
        } else {
            self.player.x + perp_dist * ray_dir_x
        };
        wall_hit_x -= wall_hit_x.floor();

        let mut tex_uf = wall_hit_x * TEX_SIZE as f64;
        if side == 0 && ray_dir_x > 0.0 {
            tex_uf = TEX_SIZE as f64 - tex_uf;
        }
        if side == 1 && ray_dir_y < 0.0 {
            tex_uf = TEX_SIZE as f64 - tex_uf;
        }

        let mut shade = 1.0 - perp_dist / MAX_DEPTH;
        if shade < 0.18 {
            shade = 0.18;
        }
        if side == 1 {
            shade *= 0.72;
        }

        let tex = &self.tex.wall[wall_type];
        let step = TEX_SIZE as f64 / line_h as f64;
        let mut tex_pos = (clip_start - SCREEN_H as i32 / 2 + line_h / 2) as f64 * step;
        for y in clip_start..=clip_end {
            self.pixels[y as usize * SCREEN_W + col] =
                sample_tex_bilinear_shaded(tex, tex_uf, tex_pos, shade);
            tex_pos += step;
        }
    }

    /// Fill the floor (lower half) and ceiling (upper half) with a per-row cast.
    /// For a given screen row the camera distance is constant, so the texture
    /// coordinate steps linearly across the row — one divide per row instead of
    /// per pixel, and the framebuffer writes run sequentially in memory (vs. the
    /// old per-column cast that strided by a full row each step). Walls are drawn
    /// on top afterwards, so over-drawing the wall band here is harmless.
    pub fn render_floor_ceiling(&mut self, dir_x: f64, dir_y: f64, plane_x: f64, plane_y: f64) {
        // Disjoint borrows: read textures + player, write the framebuffer.
        let Game { pixels, tex, player, .. } = self;
        let (px, py) = (player.x, player.y);
        let floor = &tex.floor;
        let ceil = &tex.ceil;

        // Ray directions at the screen edges (camera_x = -1 .. +1).
        let ray_left_x = dir_x - plane_x;
        let ray_left_y = dir_y - plane_y;
        let span_x = 2.0 * plane_x; // ray_right - ray_left, in X
        let span_y = 2.0 * plane_y;
        let half_h = SCREEN_H as f64 / 2.0;
        let inv_w = 1.0 / SCREEN_W as f64;

        for y in (SCREEN_H / 2 + 1)..SCREEN_H {
            let row_dist = half_h / (y as f64 - half_h);
            let mut fx = (px + row_dist * ray_left_x) * TEX_SIZE as f64;
            let mut fy = (py + row_dist * ray_left_y) * TEX_SIZE as f64;
            let dfx = row_dist * span_x * inv_w * TEX_SIZE as f64;
            let dfy = row_dist * span_y * inv_w * TEX_SIZE as f64;

            let fb = (1.0 - row_dist / MAX_DEPTH).max(0.1);
            let fb_ceil = fb * 0.85;
            let floor_row = y * SCREEN_W;
            let ceil_row = (SCREEN_H - 1 - y) * SCREEN_W; // mirror above the horizon

            for col in 0..SCREEN_W {
                pixels[floor_row + col] = sample_tex_bilinear_shaded(floor, fx, fy, fb);
                pixels[ceil_row + col] = sample_tex_bilinear_shaded(ceil, fx, fy, fb_ceil);
                fx += dfx;
                fy += dfy;
            }
        }
    }

    pub fn post_process(&mut self) {
        if self.pain_flash > 0.0 {
            let mut a = self.pain_flash;
            if a > 0.4 {
                a = 0.4;
            }
            for px in self.pixels.iter_mut() {
                let c = *px;
                let r = ((c >> 16) & 0xFF) as f64;
                let g = ((c >> 8) & 0xFF) as f64;
                let b = (c & 0xFF) as f64;
                let r = (r + (255.0 - r) * a) as i32;
                let g = (g * (1.0 - a * 0.4)) as i32;
                let b = (b * (1.0 - a * 0.4)) as i32;
                *px = make_color(r, g, b);
            }
        }
    }

    pub fn render_frame(&mut self) {
        // Ray basis is constant across the frame — compute the trig once here
        // instead of per column / per floor row.
        let dir_x = self.player.angle.cos();
        let dir_y = self.player.angle.sin();
        let plane_half = (FOV / 2.0).tan();
        let plane_x = -dir_y * plane_half;
        let plane_y = dir_x * plane_half;

        self.render_floor_ceiling(dir_x, dir_y, plane_x, plane_y);
        for x in 0..SCREEN_W {
            self.cast_column(x, dir_x, dir_y, plane_x, plane_y);
        }
        self.render_sprites();
        self.draw_crosshair();
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
            self.draw_banner("LEVEL CLEAR", 60, 0x40FF40);
        }

        if self.show_intro {
            self.draw_intro();
        }
    }
}
