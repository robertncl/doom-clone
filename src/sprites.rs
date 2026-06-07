//! Procedural enemy/pickup/fireball/particle sprites, depth-sorted and drawn
//! against the wall depth buffer. `grunt_pixel`/`imp_pixel` return `Some(color)`
//! for a drawn texel or `None` for transparent (the C `int`/`*out` pattern).

use crate::color::{make_color, shade_color};
use crate::constants::*;
use crate::game::Game;
use crate::types::{Enemy, Fireball, Pickup};

fn grunt_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx0 = u - 0.5;
    let cy = v - 0.5;
    let sway = anim.sin() * 0.015;
    let cx = cx0 - sway;

    // --- Foreground details first so they aren't overpainted by bigger shapes ---

    // Gun muzzle tip (most forward)
    if cy > 0.005 && cy < 0.035 && cx > 0.36 && cx < 0.39 {
        return Some(0x050505);
    }
    // Gun barrel
    if cy > 0.00 && cy < 0.04 && cx > 0.20 && cx < 0.36 {
        let t = ((cx - 0.20) * 80.0) as i32;
        return Some(make_color(40 - t / 4, 40 - t / 4, 40 - t / 4));
    }
    // Gun body
    if cy > 0.04 && cy < 0.10 && cx > 0.22 && cx < 0.32 {
        return Some(if cy < 0.06 { 0x303030 } else { 0x181818 });
    }
    // Hand on gun
    if cy > 0.04 && cy < 0.11 && cx > 0.17 && cx < 0.22 {
        return Some(0xA08060);
    }

    // Belt buckle (highlight)
    if cy > 0.20 && cy < 0.26 && cx.abs() < 0.04 {
        return Some(if cy < 0.22 { 0xE0C040 } else { 0xA08020 });
    }
    // Belt strap
    if cy > 0.20 && cy < 0.26 && cx.abs() < 0.22 {
        return Some(0x181208);
    }

    // Chest emblem (cross)
    if (cx.abs() < 0.015 && cy > 0.03 && cy < 0.13) || ((cy - 0.08).abs() < 0.015 && cx.abs() < 0.05)
    {
        return Some(0xC0A040);
    }

    // Helmet rim (band across forehead)
    if cx.abs() < 0.15 && cy > -0.32 && cy < -0.28 {
        return Some(0x141810);
    }
    // Visor reflection
    if (cx - 0.05) * (cx - 0.05) + (cy + 0.245) * (cy + 0.245) < 0.0004 {
        return Some(0x80B0E0);
    }
    if (cx + 0.06) * (cx + 0.06) + (cy + 0.245) * (cy + 0.245) < 0.0002 {
        return Some(0x4070A0);
    }
    // Visor (dark goggles band)
    if cx.abs() < 0.13 && cy > -0.28 && cy < -0.22 {
        return Some(0x080808);
    }

    // Stubble / mouth shadow
    if cx.abs() < 0.06 && cy > -0.16 && cy < -0.12 {
        return Some(0x4C2818);
    }

    // Helmet highlight strip on top
    if cy > -0.46 && cy < -0.42 && cx.abs() < 0.10 {
        return Some(0x80A058);
    }
    // Helmet dome (background of head)
    if cx * cx + (cy + 0.36) * (cy + 0.36) < 0.025 && cy < -0.27 {
        let mut t = (cy + 0.46) / 0.20;
        if t < 0.0 {
            t = 0.0;
        }
        if t > 1.0 {
            t = 1.0;
        }
        let v_ = (70.0 - 30.0 * t) as i32;
        return Some(make_color(v_ - 10, v_ + 20, v_ - 20));
    }

    // Face skin with side shading
    if cx * cx + (cy + 0.18) * (cy + 0.18) < 0.014 {
        let mut xt = (cx + 0.10) / 0.20;
        if xt > 1.0 {
            xt = 1.0;
        }
        if xt < 0.0 {
            xt = 0.0;
        }
        let rr = (210.0 - 40.0 * (1.0 - xt)) as i32;
        let gg = (170.0 - 30.0 * (1.0 - xt)) as i32;
        let bb = (130.0 - 25.0 * (1.0 - xt)) as i32;
        return Some(make_color(rr, gg, bb));
    }

    // Pauldrons (shoulders) with edge shading
    if cy > -0.10 && cy < -0.04 && cx.abs() < 0.26 {
        let xt = cx.abs() / 0.26;
        let base = (78.0 - 30.0 * xt) as i32;
        return Some(make_color(base - 10, base + 18, base - 28));
    }

    // Vest stripe
    if cy > -0.02 && cy < 0.01 && cx.abs() < 0.18 {
        return Some(0x2A3812);
    }
    // Chest armor
    if cy > -0.04 && cy < 0.20 && cx.abs() < 0.20 {
        let t = (cy + 0.04) / 0.24;
        let base = (90.0 - 35.0 * t) as i32;
        return Some(make_color(base - 8, base + 18, base - 28));
    }

    // Legs with vertical shading
    if cy > 0.26 && cy < 0.44 && ((cx > -0.18 && cx < -0.03) || (cx > 0.03 && cx < 0.18)) {
        let t = (cy - 0.26) / 0.18;
        let base = (60.0 - 25.0 * t) as i32;
        return Some(make_color(base - 8, base + 12, base - 20));
    }

    // Boot tip highlight
    if cy > 0.42 && cy < 0.44 && ((cx + 0.10).abs() < 0.085 || (cx - 0.10).abs() < 0.085) {
        return Some(0x302010);
    }
    // Boots
    if cy > 0.42 && cy < 0.50 && ((cx + 0.10).abs() < 0.085 || (cx - 0.10).abs() < 0.085) {
        return Some(0x100804);
    }
    None
}

fn imp_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx = u - 0.5;
    let cy0 = v - 0.5;
    let bob = (anim * 2.0).sin() * 0.025;
    let cy = cy0 - bob;
    let arm_swing = (anim * 2.0).sin() * 0.05;

    // --- Smallest foreground details first ---

    // Eye highlight (specular)
    if (cx - 0.075) * (cx - 0.075) + (cy + 0.275) * (cy + 0.275) < 0.00035 {
        return Some(0xFFFFB0);
    }
    if (cx + 0.065) * (cx + 0.065) + (cy + 0.275) * (cy + 0.275) < 0.00025 {
        return Some(0xFFFF80);
    }
    // Glowing yellow iris
    if (cx - 0.07) * (cx - 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0020 {
        return Some(0xFFE020);
    }
    if (cx + 0.07) * (cx + 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0020 {
        return Some(0xFFE020);
    }
    // Eye socket (dark ring around iris)
    if (cx - 0.07) * (cx - 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0042 {
        return Some(0x100404);
    }
    if (cx + 0.07) * (cx + 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0042 {
        return Some(0x100404);
    }

    // Nostrils
    if (cx.abs() - 0.014).abs() < 0.005 && cy > -0.215 && cy < -0.195 {
        return Some(0x000000);
    }
    // Nose snout
    if cx.abs() < 0.022 && cy > -0.23 && cy < -0.17 {
        return Some(0x401408);
    }

    // Upper fangs
    if cy > -0.15 && cy < -0.10 && ((cx + 0.05).abs() < 0.014 || (cx - 0.05).abs() < 0.014) {
        let gray = 230 - ((cy + 0.15) * 200.0) as i32;
        return Some(make_color(gray, gray, gray * 9 / 10));
    }
    // Lower fangs
    if cy > -0.10 && cy < -0.07 && ((cx + 0.025).abs() < 0.012 || (cx - 0.025).abs() < 0.012) {
        return Some(0xD8D8C0);
    }
    // Mouth gash
    if cy > -0.17 && cy < -0.13 && cx.abs() < 0.10 {
        return Some(0x180404);
    }

    // Skull ridge brow
    if cy > -0.42 && cy < -0.38 && cx.abs() < 0.14 {
        return Some(0x401008);
    }

    // Belly skull mark
    if cx.abs() < 0.035 && cy > -0.02 && cy < 0.03 {
        return Some(0xE0C040);
    }
    if cx.abs() < 0.02 && cy > 0.03 && cy < 0.06 {
        return Some(0x100400);
    }
    // Rib hints
    if ((cy > 0.06 && cy < 0.08) || (cy > 0.11 && cy < 0.13)) && cx.abs() > 0.06 && cx.abs() < 0.13 {
        return Some(0x300808);
    }

    // Claws
    {
        let claw_y = 0.20 + arm_swing;
        if cy > claw_y && cy < claw_y + 0.07 {
            let mut side = -1;
            while side <= 1 {
                for finger in 0..3 {
                    let fx = side as f64 * (0.20 + finger as f64 * 0.05);
                    if (cx - fx).abs() < 0.013 {
                        let t = (cy - claw_y) / 0.07;
                        let mut gray = (230.0 - 130.0 * t) as i32;
                        if gray < 100 {
                            gray = 100;
                        }
                        return Some(make_color(gray, gray, gray * 4 / 5));
                    }
                }
                side += 2;
            }
        }
    }
    // Hoof claws (toes)
    if cy > 0.42 && cy < 0.48 {
        let mut side = -1;
        while side <= 1 {
            for toe in 0..2 {
                let fx = side as f64 * (0.06 + toe as f64 * 0.06);
                if (cx - fx).abs() < 0.018 {
                    return Some(0x101010);
                }
            }
            side += 2;
        }
    }

    // --- Limbs / body / head (background of sprite) ---

    // Arms (biceps with highlight)
    {
        let top_y = -0.06 + arm_swing;
        let bot_y = 0.22 + arm_swing;
        if cy > top_y && cy < bot_y {
            if cx > 0.18 && cx < 0.32 {
                let bicep = (cx - 0.25) * (cx - 0.25) + (cy - 0.05) * (cy - 0.05) * 0.4;
                return Some(if bicep < 0.005 { 0x782010 } else { 0x501008 });
            }
            if cx > -0.32 && cx < -0.18 {
                let bicep = (cx + 0.25) * (cx + 0.25) + (cy - 0.05) * (cy - 0.05) * 0.4;
                return Some(if bicep < 0.005 { 0x782010 } else { 0x501008 });
            }
        }
    }

    // Legs
    if cy > 0.24 && cy < 0.44 && cx.abs() > 0.04 && cx.abs() < 0.15 {
        let t = (cy - 0.24) / 0.20;
        let base = (85.0 - 35.0 * t) as i32;
        return Some(make_color(base, base / 5, base / 6));
    }

    // Tail (S-curve, animated)
    {
        let tail_x = ((cy + 0.5) * 7.0 + anim * 2.0).sin() * 0.05;
        if cy > 0.02 && cy < 0.34 && (cx + 0.30 + tail_x).abs() < 0.02 {
            return Some(0x401008);
        }
    }

    // Body torso with gradient shading + highlight
    {
        let body_t = cx * cx * 1.6 + (cy - 0.04) * (cy - 0.04) * 0.9;
        if body_t < 0.068 {
            let mut shade = 1.0 - body_t * 8.0;
            if shade < 0.4 {
                shade = 0.4;
            }
            let mut rr = (130.0 * shade) as i32;
            let mut gg = (40.0 * shade) as i32;
            let mut bb = (28.0 * shade) as i32;
            if cx < -0.05 && cy < 0.04 {
                rr += 30;
                gg += 14;
                bb += 8;
            }
            return Some(make_color(rr, gg, bb));
        }
    }

    // Head with subtle shading
    {
        let head_t = cx * cx + (cy + 0.30) * (cy + 0.30) * 1.2;
        if head_t < 0.034 {
            let mut shade = 1.0 - head_t * 12.0;
            if shade < 0.3 {
                shade = 0.3;
            }
            let rr = (150.0 * shade) as i32 + 18;
            let gg = (55.0 * shade) as i32 + 8;
            let bb = (40.0 * shade) as i32 + 6;
            return Some(make_color(rr, gg, bb));
        }
    }

    // Horns (tapering, gradient)
    {
        let hx = [-0.17, 0.17];
        for h in &hx {
            if cy > -0.52 && cy < -0.34 {
                let t = (cy + 0.52) / 0.18;
                let half_w = 0.05 * t;
                if cx > h - half_w && cx < h + half_w {
                    let gray = if cy < -0.46 {
                        30 + (20.0 * t) as i32
                    } else if cy < -0.42 {
                        55
                    } else {
                        95
                    };
                    return Some(make_color(gray, gray * 4 / 5, gray * 3 / 4));
                }
            }
        }
    }

    None
}

impl Game {
    fn draw_enemy(&mut self, e: Enemy) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let dx = e.x - px;
        let dy = e.y - py;
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return;
        }

        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        let sprite_h = (SCREEN_H as f64 / tx) as i32;
        let sprite_w = sprite_h;
        let dsx = (screen_x - sprite_w as f64 / 2.0) as i32;
        let dsy = -sprite_h / 2 + SCREEN_H as i32 / 2;
        let sx0 = dsx.max(0);
        let sx1 = (dsx + sprite_w).min(SCREEN_W as i32);
        let sy0 = dsy.max(0);
        let sy1 = (dsy + sprite_h).min(SCREEN_H as i32);

        let mut shade = 1.0 - tx / MAX_DEPTH;
        if shade < 0.25 {
            shade = 0.25;
        }
        let flash = e.hit_flash > 0.0;

        for x in sx0..sx1 {
            if tx >= self.depth[x as usize] {
                continue;
            }
            let u = (x - dsx) as f64 / sprite_w as f64;
            for y in sy0..sy1 {
                let v = (y - dsy) as f64 / sprite_h as f64;
                let col = if e.kind == EN_IMP {
                    imp_pixel(u, v, e.anim)
                } else {
                    grunt_pixel(u, v, e.anim)
                };
                if let Some(col) = col {
                    let shaded = if flash { 0xFFF0F0 } else { shade_color(col, shade) };
                    self.pixels[y as usize * SCREEN_W + x as usize] = shaded;
                }
            }
        }
    }

    fn draw_fireball(&mut self, fb: Fireball) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let dx = fb.x - px;
        let dy = fb.y - py;
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return;
        }

        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        let mut sz = ((SCREEN_H as f64 / tx) * 0.35) as i32;
        if sz < 2 {
            sz = 2;
        }
        let dsx = (screen_x - sz as f64 / 2.0) as i32;
        let dsy = -sz / 2 + SCREEN_H as i32 / 2;
        let sx0 = dsx.max(0);
        let sx1 = (dsx + sz).min(SCREEN_W as i32);
        let sy0 = dsy.max(0);
        let sy1 = (dsy + sz).min(SCREEN_H as i32);
        let r2 = (sz as f64 * 0.5) * (sz as f64 * 0.5);

        for x in sx0..sx1 {
            if tx >= self.depth[x as usize] {
                continue;
            }
            for y in sy0..sy1 {
                let pxd = x as f64 - (dsx as f64 + sz as f64 * 0.5);
                let pyd = y as f64 - (dsy as f64 + sz as f64 * 0.5);
                let d2 = pxd * pxd + pyd * pyd;
                if d2 > r2 {
                    continue;
                }
                let t = d2 / r2;
                let r = (255.0 * (1.0 - t * 0.4)) as i32;
                let g = (180.0 * (1.0 - t)) as i32;
                let b = (40.0 * (1.0 - t)) as i32;
                self.pixels[y as usize * SCREEN_W + x as usize] = make_color(r, g, b);
            }
        }
    }

    fn draw_pickup(&mut self, p: Pickup) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let dx = p.x - px;
        let dy = p.y - py;
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return;
        }

        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        let mut sz = ((SCREEN_H as f64 / tx) * 0.45) as i32;
        if sz < 4 {
            sz = 4;
        }
        let dsx = (screen_x - sz as f64 / 2.0) as i32;
        let bob = (self.global_time * 3.0 + p.x + p.y).sin() * (sz as f64 * 0.08);
        let dsy = (SCREEN_H as f64 / 2.0 + sz as f64 * 0.15 + bob) as i32;
        let sx0 = dsx.max(0);
        let sx1 = (dsx + sz).min(SCREEN_W as i32);
        let sy0 = dsy.max(0);
        let sy1 = (dsy + sz).min(SCREEN_H as i32);

        let mut shade = 1.0 - tx / MAX_DEPTH;
        if shade < 0.3 {
            shade = 0.3;
        }

        for x in sx0..sx1 {
            if tx >= self.depth[x as usize] {
                continue;
            }
            let u = (x - dsx) as f64 / sz as f64;
            for y in sy0..sy1 {
                let v = (y - dsy) as f64 / sz as f64;
                let cx = u - 0.5;
                let cy = v - 0.5;
                let mut col = 0u32;
                let mut draw = false;
                if p.kind == PU_HEALTH {
                    // white kit with red cross
                    if cx.abs() < 0.45 && cy.abs() < 0.45 {
                        col = 0xE8E8E8;
                        draw = true;
                        if (cx.abs() < 0.10 && cy.abs() < 0.35) || (cy.abs() < 0.10 && cx.abs() < 0.35)
                        {
                            col = 0xD03020;
                        }
                        if cx.abs() > 0.42 || cy.abs() > 0.42 {
                            col = 0x808080;
                        }
                    }
                } else {
                    // ammo box: dark green with yellow strap
                    if cx.abs() < 0.45 && cy.abs() < 0.30 {
                        col = 0x305020;
                        draw = true;
                        if cy.abs() < 0.06 {
                            col = 0xC0A030;
                        }
                        if cx.abs() > 0.42 || cy.abs() > 0.27 {
                            col = 0x102008;
                        }
                    }
                }
                if draw {
                    self.pixels[y as usize * SCREEN_W + x as usize] = shade_color(col, shade);
                }
            }
        }
    }

    fn draw_particles(&mut self) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        for i in 0..MAX_PARTICLES {
            let p = self.parts[i];
            if p.life <= 0.0 {
                continue;
            }
            let dx = p.x - px;
            let dy = p.y - py;
            let cs = (-ang).cos();
            let sn = (-ang).sin();
            let tx = dx * cs - dy * sn;
            let ty = dx * sn + dy * cs;
            if tx <= 0.1 {
                continue;
            }
            let plane_half = (FOV / 2.0).tan();
            let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
            let mut sz = ((SCREEN_H as f64 / tx) * 0.08) as i32;
            if sz < 1 {
                sz = 1;
            }
            let sx = screen_x as i32;
            let sy = SCREEN_H as i32 / 2;
            if sx < 0 || sx >= SCREEN_W as i32 {
                continue;
            }
            if tx >= self.depth[sx as usize] {
                continue;
            }
            let mut fade = p.life;
            if fade > 1.0 {
                fade = 1.0;
            }
            let c = shade_color(p.color, fade);
            for yy in -sz..=sz {
                for xx in -sz..=sz {
                    if xx * xx + yy * yy > sz * sz {
                        continue;
                    }
                    self.put_pixel(sx + xx, sy + yy, c);
                }
            }
        }
    }

    pub fn render_sprites(&mut self) {
        // (distance², kind, index); kind 0=enemy 1=pickup 2=fireball.
        let mut refs: Vec<(f64, u8, usize)> = Vec::new();
        let (px, py) = (self.player.x, self.player.y);
        for i in 0..MAX_ENEMIES {
            let e = self.enemies[i];
            if !(e.alive || e.hit_flash > 0.0) {
                continue;
            }
            let dx = e.x - px;
            let dy = e.y - py;
            refs.push((dx * dx + dy * dy, 0, i));
        }
        for i in 0..MAX_PICKUPS {
            if !self.pickups[i].alive {
                continue;
            }
            let dx = self.pickups[i].x - px;
            let dy = self.pickups[i].y - py;
            refs.push((dx * dx + dy * dy, 1, i));
        }
        for i in 0..MAX_FIREBALLS {
            if !self.fireballs[i].alive {
                continue;
            }
            let dx = self.fireballs[i].x - px;
            let dy = self.fireballs[i].y - py;
            refs.push((dx * dx + dy * dy, 2, i));
        }
        // Far-to-near so nearer sprites overwrite farther ones.
        refs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, kind, idx) in refs {
            match kind {
                0 => self.draw_enemy(self.enemies[idx]),
                1 => self.draw_pickup(self.pickups[idx]),
                _ => self.draw_fireball(self.fireballs[idx]),
            }
        }
        self.draw_particles();
    }
}
