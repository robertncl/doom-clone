//! On-screen UI: first-person weapon, bitmap fonts, HUD bar with animated face,
//! score readout, intro panel, banners, game-over overlay, and the minimap.

use crate::color::make_color;
use crate::constants::*;
use crate::game::Game;

// Big 3x5 digit font (3x3 px blocks), low 3 bits per row. Used for HUD numbers.
const GLYPH: [[u8; 5]; 10] = [
    [0x7, 0x5, 0x5, 0x5, 0x7],
    [0x2, 0x6, 0x2, 0x2, 0x7],
    [0x7, 0x1, 0x7, 0x4, 0x7],
    [0x7, 0x1, 0x7, 0x1, 0x7],
    [0x5, 0x5, 0x7, 0x1, 0x1],
    [0x7, 0x4, 0x7, 0x1, 0x7],
    [0x7, 0x4, 0x7, 0x5, 0x7],
    [0x7, 0x1, 0x1, 0x1, 0x1],
    [0x7, 0x5, 0x7, 0x5, 0x7],
    [0x7, 0x5, 0x7, 0x1, 0x7],
];

// Small 3x5 font (2x2 px blocks), encoded MSB-first: row 0 = bits 14..12.
const ALPHA_GLYPH: [u16; 26] = [
    0x2BED, 0x6BAE, 0x3923, 0x6B6E, 0x79A7, 0x79A4, 0x396B, 0x5BED, 0x7497, 0x126A, 0x5D35, 0x4927,
    0x5FED, 0x6B6B, 0x7B67, 0x6BA4, 0x2B79, 0x6BAD, 0x388E, 0x7492, 0x5B6F, 0x5B6A, 0x5BFD, 0x5AAD,
    0x5A92, 0x72A7,
];

const DIGIT_GLYPH: [u16; 10] = [
    0x7B67, 0x2C97, 0x62A7, 0x628E, 0x5BC9, 0x798E, 0x39EF, 0x7292, 0x7BE7, 0x7BCE,
];

/// Width in pixels of a string in the small font (` ` = 4px, else 8px).
pub fn text_width(s: &str) -> i32 {
    let mut w = 0;
    for &ch in s.as_bytes() {
        w += if ch == b' ' { 4 } else { 8 };
    }
    w
}

impl Game {
    fn draw_digit(&mut self, d: i32, x: i32, y: i32, c: u32) {
        if !(0..=9).contains(&d) {
            return;
        }
        for ry in 0..5 {
            for rx in 0..3 {
                if GLYPH[d as usize][ry] & (1 << (2 - rx)) != 0 {
                    self.fill_rect(x + rx as i32 * 3, y + ry as i32 * 3, 3, 3, c);
                }
            }
        }
    }

    fn draw_number(&mut self, n: i32, x: i32, y: i32, c: u32) {
        let n = if n < 0 { 0 } else { n };
        let s = format!("{}", n);
        for (i, ch) in s.bytes().enumerate() {
            self.draw_digit((ch - b'0') as i32, x + i as i32 * 12, y, c);
        }
    }

    fn draw_glyph_bits(&mut self, g: u16, x: i32, y: i32, c: u32) {
        for ry in 0..5 {
            for rx in 0..3 {
                let bit = 14 - (ry * 3 + rx);
                if g & (1 << bit) != 0 {
                    self.fill_rect(x + rx as i32 * 2, y + ry as i32 * 2, 2, 2, c);
                }
            }
        }
    }

    fn draw_letter(&mut self, ch: u8, x: i32, y: i32, c: u32) {
        let mut ch = ch;
        if (b'a'..=b'z').contains(&ch) {
            ch -= 32;
        }
        if (b'A'..=b'Z').contains(&ch) {
            self.draw_glyph_bits(ALPHA_GLYPH[(ch - b'A') as usize], x, y, c);
        } else if (b'0'..=b'9').contains(&ch) {
            self.draw_glyph_bits(DIGIT_GLYPH[(ch - b'0') as usize], x, y, c);
        } else if ch == b'/' {
            self.fill_rect(x + 4, y, 2, 2, c);
            self.fill_rect(x + 2, y + 4, 2, 2, c);
            self.fill_rect(x + 2, y + 6, 2, 2, c);
            self.fill_rect(x, y + 8, 2, 2, c);
        } else if ch == b'-' {
            self.fill_rect(x, y + 4, 6, 2, c);
        } else if ch == b':' {
            self.fill_rect(x + 2, y + 2, 2, 2, c);
            self.fill_rect(x + 2, y + 6, 2, 2, c);
        } else if ch == b'.' {
            self.fill_rect(x + 2, y + 8, 2, 2, c);
        }
    }

    pub fn draw_text(&mut self, s: &str, x: i32, y: i32, c: u32) {
        let mut dx = 0;
        for &ch in s.as_bytes() {
            if ch == b' ' {
                dx += 4;
            } else {
                self.draw_letter(ch, x + dx, y, c);
                dx += 8;
            }
        }
    }

    pub fn draw_weapon(&mut self) {
        // Weapon sways with movement: phase tracks distance walked (so it pauses
        // when standing still) and amplitude scales with current speed.
        let mut sp = (self.player.vx * self.player.vx + self.player.vy * self.player.vy).sqrt()
            / MOVE_SPEED;
        if sp > 1.0 {
            sp = 1.0;
        }
        let ph = self.player.bob * 6.0;
        let gx = SCREEN_W as i32 / 2 + (ph.cos() * 8.0 * sp) as i32;
        let gy = SCREEN_H as i32 - 40 + (ph.sin().abs() * 7.0 * sp) as i32;

        // stock
        self.fill_rect(gx - 55, gy - 40, 110, 50, 0x281810);
        self.fill_rect(gx - 55, gy - 40, 110, 4, 0x60381C);
        self.fill_rect(gx - 55, gy - 8, 110, 4, 0x18100A);
        // receiver
        self.fill_rect(gx - 45, gy - 60, 90, 25, 0x383838);
        self.fill_rect(gx - 45, gy - 60, 90, 4, 0x585858);
        self.fill_rect(gx - 45, gy - 39, 90, 4, 0x181818);
        // barrel
        self.fill_rect(gx - 10, gy - 110, 20, 55, 0x202020);
        self.fill_rect(gx - 10, gy - 110, 4, 55, 0x404040);
        self.fill_rect(gx + 6, gy - 110, 4, 55, 0x101010);
        // muzzle
        self.fill_rect(gx - 12, gy - 114, 24, 6, 0x101010);
        // pump
        self.fill_rect(gx - 18, gy - 50, 36, 12, 0x402010);
        self.fill_rect(gx - 18, gy - 50, 36, 3, 0x804030);
        // sight
        self.fill_rect(gx - 1, gy - 116, 2, 4, 0xC0C0C0);
        // trigger guard
        self.fill_rect(gx - 8, gy - 30, 16, 12, 0x202020);

        if self.muzzle_flash > 0 {
            let fx = gx;
            let fy = gy - 116;
            for y in -25..18 {
                for x in -32..32 {
                    let d2 = x * x + y * y;
                    if d2 > 700 {
                        continue;
                    }
                    let d = (d2 as f64).sqrt() as i32;
                    let v = 255 - d * 9;
                    if v < 0 {
                        continue;
                    }
                    let r = v;
                    let g_c = if v > 180 { v } else { v * 7 / 10 };
                    let b = v / 6;
                    self.put_pixel(fx + x, fy + y, make_color(r, g_c, b));
                }
            }
        }
    }

    fn draw_face(&mut self, x: i32, y: i32, hp: i32) {
        // 28x28 face block
        let skin = if hp > 60 {
            0xE0B080
        } else if hp > 30 {
            0xC09060
        } else {
            0x806040
        };
        let blood = 0x800000;
        self.fill_rect(x, y, 28, 28, skin);
        self.fill_rect(x, y, 28, 2, 0xA08060);
        self.fill_rect(x, y + 26, 28, 2, 0x604030);
        // hair
        self.fill_rect(x + 2, y, 24, 5, 0x402008);
        self.fill_rect(x + 2, y + 4, 4, 2, 0x402008);
        self.fill_rect(x + 22, y + 4, 4, 2, 0x402008);
        // eyes
        let eye_y = if hp < 30 { y + 11 } else { y + 9 };
        self.fill_rect(x + 7, eye_y, 4, 3, 0xFFFFFF);
        self.fill_rect(x + 17, eye_y, 4, 3, 0xFFFFFF);
        let pupil_off = ((self.global_time * 1.7).sin() * 1.0) as i32;
        self.fill_rect(x + 8 + pupil_off, eye_y, 2, 3, 0x000000);
        self.fill_rect(x + 18 + pupil_off, eye_y, 2, 3, 0x000000);
        // nose
        self.fill_rect(x + 13, y + 13, 2, 4, 0xA07050);
        // mouth depends on health
        if hp > 60 {
            self.fill_rect(x + 9, y + 20, 10, 2, 0x401010);
        } else if hp > 30 {
            self.fill_rect(x + 10, y + 21, 8, 2, 0x301010);
        } else {
            self.fill_rect(x + 9, y + 22, 10, 2, 0x200808);
            self.fill_rect(x + 9, y + 20, 2, 2, 0x301010);
            self.fill_rect(x + 17, y + 20, 2, 2, 0x301010);
        }
        // blood for low health
        if hp < 50 {
            self.fill_rect(x + 5, y + 6, 2, 4, blood);
            self.fill_rect(x + 21, y + 8, 2, 6, blood);
        }
        if hp < 25 {
            self.fill_rect(x + 12, y + 5, 4, 3, blood);
            self.fill_rect(x + 10, y + 8, 2, 3, blood);
        }
        if hp <= 0 {
            // X over eyes
            for i in 0..4 {
                self.put_pixel(x + 7 + i, eye_y + i, 0x000000);
                self.put_pixel(x + 10 - i, eye_y + i, 0x000000);
                self.put_pixel(x + 17 + i, eye_y + i, 0x000000);
                self.put_pixel(x + 20 - i, eye_y + i, 0x000000);
            }
        }
    }

    pub fn draw_hud(&mut self) {
        let bar_y = SCREEN_H as i32 - 56;
        // gradient background bar
        for y in bar_y..SCREEN_H as i32 {
            let t = (y - bar_y) * 255 / 56;
            let c = make_color(32 + t / 8, 28 + t / 8, 24 + t / 10);
            self.fill_rect(0, y, SCREEN_W as i32, 1, c);
        }
        self.fill_rect(0, bar_y, SCREEN_W as i32, 2, 0xA08060);
        self.fill_rect(0, bar_y + 2, SCREEN_W as i32, 1, 0x402010);

        // dividers
        let mut dx = 110;
        while dx <= SCREEN_W as i32 - 110 {
            self.fill_rect(dx, bar_y + 6, 2, 44, 0x281810);
            self.fill_rect(dx + 2, bar_y + 6, 1, 44, 0x60381C);
            dx += SCREEN_W as i32 - 220;
        }

        // Face panel center
        let fx = SCREEN_W as i32 / 2 - 14;
        let fy = bar_y + 14;
        self.fill_rect(fx - 4, fy - 4, 36, 36, 0x100804);
        self.draw_face(fx, fy, self.player.health);

        // Health
        let hc = if self.player.health > 50 {
            0x40E040
        } else if self.player.health > 20 {
            0xE0E040
        } else {
            0xE04040
        };
        self.draw_text("HEALTH", 20, bar_y + 6, 0xC0A080);
        self.draw_number(self.player.health, 20, bar_y + 18, hc);

        // Ammo
        self.draw_text("AMMO", SCREEN_W as i32 - 80, bar_y + 6, 0xC0A080);
        self.draw_number(self.player.ammo, SCREEN_W as i32 - 80, bar_y + 18, 0xE0E060);

        // Level + kills
        let alive = self.enemies[..self.level_enemy_count.max(0) as usize]
            .iter()
            .filter(|e| e.alive)
            .count() as i32;
        let kills = self.level_enemy_count - alive;

        let buf = format!("LEVEL {}", self.level + 1);
        self.draw_text(&buf, 20, SCREEN_H as i32 - 14, 0xE0C080);

        let buf = format!("KILLS {}/{}", kills, self.level_enemy_count);
        let kw = text_width(&buf);
        self.draw_text(&buf, SCREEN_W as i32 - kw - 20, SCREEN_H as i32 - 14, 0xC0A080);
    }

    pub fn draw_score_readout(&mut self) {
        let buf = format!("SCORE {}", self.score);
        let w = text_width(&buf);
        self.fill_rect(6, 6, w + 8, 14, 0x101010);
        self.fill_rect(6, 6, w + 8, 1, 0x806020);
        self.fill_rect(6, 19, w + 8, 1, 0x402010);
        self.draw_text(&buf, 10, 8, 0xFFE060);
    }

    pub fn draw_game_over_overlay(&mut self) {
        let is_victory = self.player.health > 0;
        let title = if is_victory { "VICTORY" } else { "YOU DIED" };
        let title_c = if is_victory { 0x40E0FF } else { 0xFF4040 };

        // Dim the play area, tint by outcome
        let limit = SCREEN_W * (SCREEN_H - 56);
        for px in self.pixels[..limit].iter_mut() {
            let c = *px;
            let mut r = ((c >> 16) & 0xFF) as i32;
            let mut g = ((c >> 8) & 0xFF) as i32;
            let mut b = (c & 0xFF) as i32;
            if is_victory {
                r /= 3;
                g = g / 3 + 20;
                b = b / 3 + 30;
            } else {
                r = r / 2 + 50;
                g /= 4;
                b /= 4;
            }
            *px = make_color(r, g, b);
        }

        let mut y = 40;
        let tw = text_width(title);
        self.draw_text(title, (SCREEN_W as i32 - tw) / 2, y, title_c);

        y += 32;
        let sbuf = format!("SCORE {}", self.score);
        self.draw_text(&sbuf, (SCREEN_W as i32 - text_width(&sbuf)) / 2, y, 0xFFE060);

        y += 22;
        if self.final_rank > 0 {
            let rbuf = format!("NEW HIGH SCORE RANK {}", self.final_rank);
            self.draw_text(&rbuf, (SCREEN_W as i32 - text_width(&rbuf)) / 2, y, 0x60FF60);
            y += 22;
        }

        y += 14;
        self.draw_text(
            "HIGH SCORES",
            (SCREEN_W as i32 - text_width("HIGH SCORES")) / 2,
            y,
            0xE0E0E0,
        );

        y += 22;
        for i in 0..MAX_HIGHSCORES {
            let buf = format!("{}. {}", i + 1, self.high_scores[i]);
            let c = if self.final_rank == i as i32 + 1 { 0x60FF60 } else { 0xC0A080 };
            self.draw_text(&buf, (SCREEN_W as i32 - text_width(&buf)) / 2, y, c);
            y += 14;
        }

        y += 20;
        if ((self.global_time * 2.0) as i32) & 1 != 0 {
            self.draw_text(
                "PRESS R TO RESTART",
                (SCREEN_W as i32 - text_width("PRESS R TO RESTART")) / 2,
                y,
                0x40E040,
            );
        }
    }

    pub fn draw_crosshair(&mut self) {
        let cx = SCREEN_W as i32 / 2;
        let cy = SCREEN_H as i32 / 2 - 30;
        for i in -5..=5 {
            if (-1..=1).contains(&i) {
                continue;
            }
            self.put_pixel(cx + i, cy, 0xE0E0E0);
            self.put_pixel(cx, cy + i, 0xE0E0E0);
        }
        self.put_pixel(cx, cy, 0xFF4040);
    }

    pub fn draw_banner(&mut self, text: &str, y: i32, c: u32) {
        let tw = text_width(text);
        let tx = (SCREEN_W as i32 - tw) / 2;
        self.fill_rect(0, y - 10, SCREEN_W as i32, 3, c);
        self.fill_rect(0, y + 12, SCREEN_W as i32, 3, c);
        self.fill_rect(tx - 10, y - 5, tw + 20, 12, 0x101010);
        self.draw_text(text, tx, y - 3, c);
    }

    pub fn draw_intro(&mut self) {
        let x0 = 70;
        let y0 = 20;
        let w = SCREEN_W as i32 - 140;
        let h = 360;

        // dim and tint background panel
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let idx = y as usize * SCREEN_W + x as usize;
                let c = self.pixels[idx];
                let r = ((c >> 16) & 0xFF) as i32;
                let g = ((c >> 8) & 0xFF) as i32;
                let b = (c & 0xFF) as i32;
                self.pixels[idx] = make_color(r / 4 + 10, g / 5, b / 5);
            }
        }

        // double-line border
        self.fill_rect(x0, y0, w, 3, 0xC0A040);
        self.fill_rect(x0, y0 + h - 3, w, 3, 0xC0A040);
        self.fill_rect(x0, y0, 3, h, 0xC0A040);
        self.fill_rect(x0 + w - 3, y0, 3, h, 0xC0A040);
        self.fill_rect(x0 + 6, y0 + 6, w - 12, 1, 0x603018);
        self.fill_rect(x0 + 6, y0 + h - 7, w - 12, 1, 0x603018);
        self.fill_rect(x0 + 6, y0 + 6, 1, h - 12, 0x603018);
        self.fill_rect(x0 + w - 7, y0 + 6, 1, h - 12, 0x603018);

        // title
        self.draw_text(
            "DOOM CLONE",
            x0 + (w - text_width("DOOM CLONE")) / 2,
            y0 + 20,
            0xFFC040,
        );
        self.draw_text(
            "CONTROLS",
            x0 + (w - text_width("CONTROLS")) / 2,
            y0 + 56,
            0xE0E0E0,
        );

        let lx = x0 + 60;
        let mut ty = y0 + 90;
        self.draw_text("W S", lx, ty, 0x60C0FF);
        self.draw_text("MOVE", lx + 160, ty, 0xC0C0C0);
        ty += 22;
        self.draw_text("A D", lx, ty, 0x60C0FF);
        self.draw_text("STRAFE", lx + 160, ty, 0xC0C0C0);
        ty += 22;
        self.draw_text("ARROWS", lx, ty, 0x60C0FF);
        self.draw_text("TURN", lx + 160, ty, 0xC0C0C0);
        ty += 22;
        self.draw_text("SPACE", lx, ty, 0x60C0FF);
        self.draw_text("SHOOT", lx + 160, ty, 0xC0C0C0);
        ty += 22;
        self.draw_text("R", lx, ty, 0x60C0FF);
        self.draw_text("RESTART", lx + 160, ty, 0xC0C0C0);
        ty += 22;
        self.draw_text("ESC", lx, ty, 0x60C0FF);
        self.draw_text("QUIT", lx + 160, ty, 0xC0C0C0);
        ty += 24;

        // divider
        self.fill_rect(x0 + 30, ty, w - 60, 1, 0x60381C);
        ty += 12;

        self.draw_text(
            "HIGH SCORES",
            x0 + (w - text_width("HIGH SCORES")) / 2,
            ty,
            0xFFC040,
        );
        ty += 18;

        for i in 0..MAX_HIGHSCORES {
            let buf = format!("{}. {}", i + 1, self.high_scores[i]);
            self.draw_text(&buf, x0 + (w - text_width(&buf)) / 2, ty, 0xC0A080);
            ty += 14;
        }
        ty += 10;

        // blinking prompt
        if ((self.global_time * 2.0) as i32) & 1 != 0 {
            self.draw_text(
                "PRESS ANY KEY",
                x0 + (w - text_width("PRESS ANY KEY")) / 2,
                ty,
                0x40E040,
            );
        }
    }

    pub fn draw_minimap(&mut self) {
        let mx0 = SCREEN_W as i32 - 100;
        let my0 = 8;
        let cell = 5;
        self.fill_rect(
            mx0 - 2,
            my0 - 2,
            MAP_W as i32 * cell + 4,
            MAP_H as i32 * cell + 4,
            0x101010,
        );
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let c = match self.cur_map[y][x] {
                    b'.' => 0x303030,
                    b'#' => 0x808080,
                    b'=' => 0xA04030,
                    b'B' => 0x4060A0,
                    b'D' => 0x805020,
                    b'H' => 0x602010,
                    _ => 0x404040,
                };
                self.fill_rect(mx0 + x as i32 * cell, my0 + y as i32 * cell, cell - 1, cell - 1, c);
            }
        }
        // enemies
        for i in 0..MAX_ENEMIES {
            if !self.enemies[i].alive {
                continue;
            }
            let c = if self.enemies[i].kind == EN_IMP { 0xE04020 } else { 0xC0C040 };
            let px = mx0 + (self.enemies[i].x * cell as f64) as i32;
            let py = my0 + (self.enemies[i].y * cell as f64) as i32;
            self.fill_rect(px - 1, py - 1, 3, 3, c);
        }
        // pickups
        for i in 0..MAX_PICKUPS {
            if !self.pickups[i].alive {
                continue;
            }
            let c = if self.pickups[i].kind == PU_HEALTH { 0xE04040 } else { 0xE0C040 };
            let px = mx0 + (self.pickups[i].x * cell as f64) as i32;
            let py = my0 + (self.pickups[i].y * cell as f64) as i32;
            self.put_pixel(px, py, c);
            self.put_pixel(px + 1, py, c);
            self.put_pixel(px, py + 1, c);
        }
        // player
        let ppx = mx0 + (self.player.x * cell as f64) as i32;
        let ppy = my0 + (self.player.y * cell as f64) as i32;
        self.fill_rect(ppx - 1, ppy - 1, 3, 3, 0x40E040);
        let dx = (self.player.angle.cos() * 4.0) as i32;
        let dy = (self.player.angle.sin() * 4.0) as i32;
        for s in 0..4 {
            self.put_pixel(ppx + dx * s / 4, ppy + dy * s / 4, 0x80FF80);
        }
    }
}
