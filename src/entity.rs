//! Game-logic updates: particles, fireballs, enemy AI, pickups, the hitscan
//! shot, and the per-frame `update_game` that ties movement, combat, and level
//! progression together.

use crate::constants::*;
use crate::game::Game;
use std::f64::consts::PI;

impl Game {
    fn spawn_particle(&mut self, x: f64, y: f64, vx: f64, vy: f64, life: f64, color: u32) {
        for p in self.parts.iter_mut() {
            if p.life <= 0.0 {
                p.x = x;
                p.y = y;
                p.vx = vx;
                p.vy = vy;
                p.life = life;
                p.color = color;
                return;
            }
        }
    }

    fn spawn_blood(&mut self, x: f64, y: f64, count: i32) {
        for _ in 0..count {
            let a = self.rand_f64() * 2.0 * PI;
            let s = 0.3 + self.rand_f64() * 0.7;
            let life = 0.6 + self.rand_f64() * 0.4;
            self.spawn_particle(x, y, a.cos() * s, a.sin() * s, life, 0xC02020);
        }
    }

    fn spawn_sparks(&mut self, x: f64, y: f64) {
        for _ in 0..6 {
            let a = self.rand_f64() * 2.0 * PI;
            self.spawn_particle(x, y, a.cos() * 0.4, a.sin() * 0.4, 0.35, 0xFFE060);
        }
    }

    fn update_particles(&mut self, dt: f64) {
        for p in self.parts.iter_mut() {
            if p.life <= 0.0 {
                continue;
            }
            p.life -= dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vx *= 0.92;
            p.vy *= 0.92;
        }
    }

    fn spawn_fireball(&mut self, x: f64, y: f64, tx: f64, ty: f64) {
        for i in 0..MAX_FIREBALLS {
            if self.fireballs[i].alive {
                continue;
            }
            let dx = tx - x;
            let dy = ty - y;
            let d = (dx * dx + dy * dy).sqrt();
            if d < 0.0001 {
                return;
            }
            self.fireballs[i].x = x;
            self.fireballs[i].y = y;
            self.fireballs[i].vx = dx / d * 3.0;
            self.fireballs[i].vy = dy / d * 3.0;
            self.fireballs[i].alive = true;
            self.fireballs[i].life = 3.0;
            self.audio.play(SND_FIREBALL);
            return;
        }
    }

    fn update_fireballs(&mut self, dt: f64) {
        for i in 0..MAX_FIREBALLS {
            let mut fb = self.fireballs[i];
            if !fb.alive {
                continue;
            }
            fb.life -= dt;
            if fb.life <= 0.0 {
                fb.alive = false;
                self.fireballs[i] = fb;
                continue;
            }
            let nx = fb.x + fb.vx * dt;
            let ny = fb.y + fb.vy * dt;
            if self.map_blocked(nx as i32, ny as i32) {
                let (sx, sy) = (fb.x, fb.y);
                fb.alive = false;
                self.fireballs[i] = fb;
                self.spawn_sparks(sx, sy);
                continue;
            }
            fb.x = nx;
            fb.y = ny;
            // hit player
            let dx = self.player.x - fb.x;
            let dy = self.player.y - fb.y;
            if dx * dx + dy * dy < 0.18 {
                self.player.health -= 12;
                if self.player.health < 0 {
                    self.player.health = 0;
                }
                self.pain_flash = 0.35;
                let (bx, by) = (fb.x, fb.y);
                fb.alive = false;
                self.fireballs[i] = fb;
                self.spawn_blood(bx, by, 6);
                self.audio.play(SND_PLAYER_HURT);
                continue;
            }
            // spawn flame trail
            if ((self.global_time * 30.0) as i32) % 2 == 0 {
                let (tx, ty) = (fb.x, fb.y);
                self.fireballs[i] = fb;
                self.spawn_particle(tx, ty, 0.0, 0.0, 0.25, 0xFFA040);
            } else {
                self.fireballs[i] = fb;
            }
        }
    }

    fn update_enemies(&mut self, dt: f64) {
        for i in 0..MAX_ENEMIES {
            let mut e = self.enemies[i];
            if e.hit_flash > 0.0 {
                e.hit_flash -= dt;
            }
            if !e.alive {
                self.enemies[i] = e;
                continue;
            }
            e.anim += dt * 4.0;
            if e.atk_cool > 0.0 {
                e.atk_cool -= dt;
            }

            let dx = self.player.x - e.x;
            let dy = self.player.y - e.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < 0.001 {
                self.enemies[i] = e;
                continue;
            }
            let nx = dx / dist;
            let ny = dy / dist;

            if e.kind == EN_GRUNT {
                let speed = 1.1 * dt;
                if dist > 0.7 {
                    let mx = e.x + nx * speed;
                    let my = e.y + ny * speed;
                    if !self.map_blocked(mx as i32, e.y as i32) {
                        e.x = mx;
                    }
                    if !self.map_blocked(e.x as i32, my as i32) {
                        e.y = my;
                    }
                } else if e.atk_cool <= 0.0 && self.player.health > 0 {
                    self.player.health -= 7;
                    if self.player.health < 0 {
                        self.player.health = 0;
                    }
                    e.atk_cool = 1.0;
                    self.pain_flash = 0.3;
                    self.audio.play(SND_PLAYER_HURT);
                }
            } else {
                // IMP: keep medium distance, throw fireballs
                let speed = 0.9 * dt;
                if dist > 4.5 {
                    let mx = e.x + nx * speed;
                    let my = e.y + ny * speed;
                    if !self.map_blocked(mx as i32, e.y as i32) {
                        e.x = mx;
                    }
                    if !self.map_blocked(e.x as i32, my as i32) {
                        e.y = my;
                    }
                } else if dist < 2.5 {
                    let mx = e.x - nx * speed * 0.5;
                    let my = e.y - ny * speed * 0.5;
                    if !self.map_blocked(mx as i32, e.y as i32) {
                        e.x = mx;
                    }
                    if !self.map_blocked(e.x as i32, my as i32) {
                        e.y = my;
                    }
                }
                if e.atk_cool <= 0.0 && dist < 8.0 && self.player.health > 0 {
                    let (ex, ey) = (e.x, e.y);
                    let (tx, ty) = (self.player.x, self.player.y);
                    e.atk_cool = 2.0 + self.rand_f64();
                    self.enemies[i] = e;
                    self.spawn_fireball(ex, ey, tx, ty);
                    continue;
                }
            }
            self.enemies[i] = e;
        }
    }

    fn update_pickups(&mut self) {
        for i in 0..MAX_PICKUPS {
            if !self.pickups[i].alive {
                continue;
            }
            let dx = self.pickups[i].x - self.player.x;
            let dy = self.pickups[i].y - self.player.y;
            if dx * dx + dy * dy < 0.25 {
                let (px, py, kind) = (self.pickups[i].x, self.pickups[i].y, self.pickups[i].kind);
                let color = match kind {
                    PU_HEALTH => {
                        self.player.health = (self.player.health + 25).min(100);
                        self.audio.play(SND_PICKUP_HEALTH);
                        0xFFC0C0
                    }
                    PU_AMMO => {
                        self.player.ammo = (self.player.ammo + 12).min(99);
                        self.audio.play(SND_PICKUP_AMMO);
                        0xFFE060
                    }
                    _ => {
                        // Weapon pickup: own it, auto-equip it, and throw in a
                        // little ammo so it's immediately useful.
                        let wp = if kind == PU_RIFLE { WP_RIFLE } else { WP_SHOTGUN };
                        self.player.weapons[wp as usize] = true;
                        self.player.weapon = wp;
                        self.player.ammo = (self.player.ammo + 8).min(99);
                        self.audio.play(SND_PICKUP_WEAPON);
                        if kind == PU_RIFLE { 0x80FF80 } else { 0xFFB060 }
                    }
                };
                self.pickups[i].alive = false;
                for _ in 0..8 {
                    let a = self.rand_f64() * 2.0 * PI;
                    self.spawn_particle(px, py, a.cos() * 0.3, a.sin() * 0.3, 0.4, color);
                }
            }
        }
    }

    fn shoot(&mut self) {
        if self.player.ammo <= 0 {
            return;
        }
        self.player.ammo -= 1;
        self.muzzle_flash = 5;
        self.audio.play(SND_SHOOT);

        // Find nearest enemy along the aim ray within angular tolerance.
        let rx = self.player.angle.cos();
        let ry = self.player.angle.sin();

        // Wall stop distance
        let mut wall_t = 0.0;
        while wall_t < MAX_DEPTH {
            wall_t += 0.05;
            if self.map_blocked(
                (self.player.x + rx * wall_t) as i32,
                (self.player.y + ry * wall_t) as i32,
            ) {
                let (sx, sy) = (self.player.x + rx * wall_t, self.player.y + ry * wall_t);
                self.spawn_sparks(sx, sy);
                break;
            }
        }

        let mut best_idx: i32 = -1;
        let mut best_dist = wall_t;
        for i in 0..MAX_ENEMIES {
            let e = self.enemies[i];
            if !e.alive {
                continue;
            }
            let dx = e.x - self.player.x;
            let dy = e.y - self.player.y;
            let d = (dx * dx + dy * dy).sqrt();
            let mut ang = dy.atan2(dx) - self.player.angle;
            while ang > PI {
                ang -= 2.0 * PI;
            }
            while ang < -PI {
                ang += 2.0 * PI;
            }
            // angular tolerance shrinks with distance
            let mut tol = 0.22 / (if d < 1.0 { 1.0 } else { d });
            if tol < 0.04 {
                tol = 0.04;
            }
            if ang.abs() > tol {
                continue;
            }
            if d < best_dist {
                best_dist = d;
                best_idx = i as i32;
            }
        }
        if best_idx >= 0 {
            let i = best_idx as usize;
            self.enemies[i].hp -= 1;
            self.enemies[i].hit_flash = 0.15;
            let (ex, ey, kind, hp) =
                (self.enemies[i].x, self.enemies[i].y, self.enemies[i].kind, self.enemies[i].hp);
            self.spawn_blood(ex, ey, 8);
            if hp <= 0 {
                self.enemies[i].alive = false;
                self.spawn_blood(ex, ey, 14);
                self.score += if kind == EN_IMP { 200 } else { 100 };
                self.audio.play(SND_DEATH);
            } else {
                self.audio.play(SND_HIT);
            }
        }
    }

    pub fn update_game(&mut self, dt: f64) {
        self.global_time += dt;
        if self.pain_flash > 0.0 {
            self.pain_flash -= dt;
        }

        if self.show_intro {
            if self.key_edge[K_QUIT] {
                self.running = false;
                self.key_edge[K_QUIT] = false;
                return;
            }
            for i in 0..K_COUNT {
                if i == K_QUIT {
                    continue;
                }
                if self.key_edge[i] {
                    self.show_intro = false;
                    self.key_edge[i] = false;
                    break;
                }
            }
            return;
        }

        let fx = self.player.angle.cos();
        let fy = self.player.angle.sin();
        let sxv = -self.player.angle.sin();
        let syv = self.player.angle.cos();

        if self.player.health > 0 {
            // Build the desired ("wish") move direction from held keys.
            let mut wish_x = 0.0;
            let mut wish_y = 0.0;
            if self.keys[K_FWD] {
                wish_x += fx;
                wish_y += fy;
            }
            if self.keys[K_BACK] {
                wish_x -= fx;
                wish_y -= fy;
            }
            if self.keys[K_STRAFEL] {
                wish_x -= sxv;
                wish_y -= syv;
            }
            if self.keys[K_STRAFER] {
                wish_x += sxv;
                wish_y += syv;
            }
            let wl = (wish_x * wish_x + wish_y * wish_y).sqrt();

            // Target velocity, then smooth current velocity toward it.
            let mut tvx = 0.0;
            let mut tvy = 0.0;
            let mut rate = MOVE_FRICTION;
            if wl > 1e-6 {
                tvx = wish_x / wl * MOVE_SPEED;
                tvy = wish_y / wl * MOVE_SPEED;
                rate = MOVE_ACCEL;
            }
            let mut mk = rate * dt;
            if mk > 1.0 {
                mk = 1.0;
            }
            self.player.vx += (tvx - self.player.vx) * mk;
            self.player.vy += (tvy - self.player.vy) * mk;

            let moved = self.try_move(
                self.player.x + self.player.vx * dt,
                self.player.y + self.player.vy * dt,
            );
            if moved & 1 == 0 {
                self.player.vx = 0.0;
            }
            if moved & 2 == 0 {
                self.player.vy = 0.0;
            }

            // Advance the bob phase by distance actually travelled.
            self.player.bob +=
                (self.player.vx * self.player.vx + self.player.vy * self.player.vy).sqrt() * dt;

            // Smoothed turning (keyboard) with the same accel/friction model.
            let mut turn_wish = 0.0;
            if self.keys[K_TURNL] {
                turn_wish -= 1.0;
            }
            if self.keys[K_TURNR] {
                turn_wish += 1.0;
            }
            let tva = turn_wish * TURN_SPEED;
            let trate = if turn_wish != 0.0 { TURN_ACCEL } else { TURN_FRICTION };
            let mut tk = trate * dt;
            if tk > 1.0 {
                tk = 1.0;
            }
            self.player.va += (tva - self.player.va) * tk;
            self.player.angle += self.player.va * dt;

            if self.key_edge[K_SHOOT] {
                self.shoot();
                self.key_edge[K_SHOOT] = false;
            }
        } else {
            // Dead: coast velocity to zero so the view settles smoothly.
            self.player.vx *= 0.9;
            self.player.vy *= 0.9;
            self.player.va *= 0.9;
        }

        if self.key_edge[K_RESTART] && self.score_saved {
            self.reset_game();
            self.key_edge[K_RESTART] = false;
        }
        if self.key_edge[K_QUIT] {
            self.running = false;
        }

        if self.muzzle_flash > 0 {
            self.muzzle_flash -= 1;
        }

        if self.player.health > 0 {
            self.update_enemies(dt);
            self.update_fireballs(dt);
            self.update_pickups();
        } else if !self.score_saved {
            self.final_rank = self.submit_score(self.score);
            self.score_saved = true;
            self.audio.play(SND_GAME_OVER);
        }
        self.update_particles(dt);

        if self.player.health > 0 && self.all_enemies_dead() {
            if !self.level_bonus_given {
                self.score += 500 + (self.level + 1) * 100;
                self.level_bonus_given = true;
                self.audio.play(SND_LEVEL_CLEAR);
            }
            self.level_clear_timer += dt;
            if self.level_clear_timer > 2.5 {
                if (self.level as usize + 1) < LEVEL_COUNT {
                    self.load_level(self.level as usize + 1);
                } else if !self.score_saved {
                    self.final_rank = self.submit_score(self.score);
                    self.score_saved = true;
                    self.level_clear_timer = 0.0;
                }
            }
        }
    }
}
