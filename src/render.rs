//! Pixel/rect helpers, the DDA raycaster (textured walls + floor/ceiling cast),
//! the pain-flash post-process, and full-frame composition.

use crate::color::{make_color, sample_tex_bilinear, shade_color};
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

    pub fn cast_column(&mut self, col: usize) {
        let camera_x = 2.0 * col as f64 / SCREEN_W as f64 - 1.0;
        let dir_x = self.player.angle.cos();
        let dir_y = self.player.angle.sin();
        let plane_x = -self.player.angle.sin() * (FOV / 2.0).tan();
        let plane_y = self.player.angle.cos() * (FOV / 2.0).tan();

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

        let step = TEX_SIZE as f64 / line_h as f64;
        let mut tex_pos = (clip_start - SCREEN_H as i32 / 2 + line_h / 2) as f64 * step;
        for y in clip_start..=clip_end {
            let c = sample_tex_bilinear(&self.tex.wall[wall_type], tex_uf, tex_pos);
            tex_pos += step;
            self.pixels[y as usize * SCREEN_W + col] = shade_color(c, shade);
        }

        // Floor/ceiling cast for rows below the wall (and mirror to above).
        let mut floor_start = draw_end + 1;
        if floor_start <= SCREEN_H as i32 / 2 {
            floor_start = SCREEN_H as i32 / 2 + 1;
        }
        if floor_start < 0 {
            floor_start = 0;
        }
        for y in floor_start..SCREEN_H as i32 {
            let p = y as f64 - SCREEN_H as f64 / 2.0;
            if p <= 0.0 {
                continue;
            }
            let row_dist = (SCREEN_H as f64 * 0.5) / p;
            let floor_x = self.player.x + row_dist * ray_dir_x;
            let floor_y = self.player.y + row_dist * ray_dir_y;
            let tex_x = floor_x * TEX_SIZE as f64;
            let tex_y = floor_y * TEX_SIZE as f64;
            let mut fb = 1.0 - row_dist / MAX_DEPTH;
            if fb < 0.1 {
                fb = 0.1;
            }
            self.pixels[y as usize * SCREEN_W + col] =
                shade_color(sample_tex_bilinear(&self.tex.floor, tex_x, tex_y), fb);
            let cy = SCREEN_H as i32 - y - 1;
            if cy >= 0 && cy < draw_start {
                self.pixels[cy as usize * SCREEN_W + col] =
                    shade_color(sample_tex_bilinear(&self.tex.ceil, tex_x, tex_y), fb * 0.85);
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
        for x in 0..SCREEN_W {
            self.cast_column(x);
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
