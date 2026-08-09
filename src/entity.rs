//! Game-logic updates: particles, fireballs, enemy AI, pickups, the hitscan
//! shot, and the per-frame `update_game` that ties movement, combat, and level
//! progression together.

use crate::constants::*;
use crate::game::Game;
use crate::types::Enemy;
use std::f64::consts::PI;

/// Distance from (px,py) to (tx,ty) if that point sits inside the shot's
/// angular tolerance around `ang`, else `None`. The tolerance shrinks with
/// distance, so far targets need a tighter bead.
pub(crate) fn shot_hit_dist(px: f64, py: f64, ang: f64, tx: f64, ty: f64) -> Option<f64> {
    let dx = tx - px;
    let dy = ty - py;
    let d = (dx * dx + dy * dy).sqrt();
    let mut a = dy.atan2(dx) - ang;
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    let tol = (0.22 / d.max(1.0)).max(0.04);
    if a.abs() > tol {
        None
    } else {
        Some(d)
    }
}

impl Game {
    pub(crate) fn spawn_particle(&mut self, x: f64, y: f64, vx: f64, vy: f64, life: f64, color: u32) {
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

    pub(crate) fn spawn_blood(&mut self, x: f64, y: f64, count: i32) {
        for _ in 0..count {
            let a = self.rand_f64() * 2.0 * PI;
            let s = 0.3 + self.rand_f64() * 0.7;
            let life = 0.6 + self.rand_f64() * 0.4;
            self.spawn_particle(x, y, a.cos() * s, a.sin() * s, life, 0xC02020);
        }
    }

    pub(crate) fn spawn_sparks(&mut self, x: f64, y: f64) {
        for _ in 0..6 {
            let a = self.rand_f64() * 2.0 * PI;
            self.spawn_particle(x, y, a.cos() * 0.4, a.sin() * 0.4, 0.35, 0xFFE060);
        }
    }

    pub(crate) fn update_particles(&mut self, dt: f64) {
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

    /// Launch a projectile from (x,y) toward (tx,ty). `spread` rotates the shot
    /// off that line (radians), so a caller can fan out a volley from one aim.
    pub(crate) fn spawn_fireball(&mut self, x: f64, y: f64, tx: f64, ty: f64, spread: f64, speed: f64, dmg: i32) {
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
            let a = dy.atan2(dx) + spread;
            self.fireballs[i].x = x;
            self.fireballs[i].y = y;
            self.fireballs[i].vx = a.cos() * speed;
            self.fireballs[i].vy = a.sin() * speed;
            self.fireballs[i].alive = true;
            self.fireballs[i].life = 3.0;
            self.fireballs[i].dmg = dmg;
            self.audio.play(SND_FIREBALL);
            return;
        }
    }

    pub(crate) fn update_fireballs(&mut self, dt: f64) {
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
            // A fireball that lands on a barrel sets it off — enemy fire can
            // just as easily blow up the scenery in the player's favour.
            if let Some(b) = self.barrel_at(fb.x, fb.y, 0.45) {
                fb.alive = false;
                self.fireballs[i] = fb;
                self.explode_barrel(b);
                continue;
            }
            // hit player
            let dx = self.player.x - fb.x;
            let dy = self.player.y - fb.y;
            if dx * dx + dy * dy < 0.18 {
                self.player.health -= fb.dmg;
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

    /// Move an enemy by (dx,dy), sliding along walls one axis at a time so a
    /// corner deflects it instead of stopping it dead.
    pub(crate) fn move_enemy(&self, e: &mut Enemy, dx: f64, dy: f64) {
        if !self.map_blocked((e.x + dx) as i32, e.y as i32) {
            e.x += dx;
        }
        if !self.map_blocked(e.x as i32, (e.y + dy) as i32) {
            e.y += dy;
        }
    }

    /// Land a melee hit on the player (no-op if they're already down).
    pub(crate) fn melee_player(&mut self, dmg: i32) {
        if self.player.health <= 0 {
            return;
        }
        self.player.health = (self.player.health - dmg).max(0);
        self.pain_flash = 0.3;
        self.audio.play(SND_PLAYER_HURT);
    }

    pub(crate) fn update_enemies(&mut self, dt: f64) {
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

            match e.kind {
                EN_GRUNT => {
                    // Walks straight at you and swings.
                    let speed = 1.1 * dt;
                    if dist > 0.7 {
                        self.move_enemy(&mut e, nx * speed, ny * speed);
                    } else if e.atk_cool <= 0.0 {
                        e.atk_cool = 1.0;
                        self.melee_player(7);
                    }
                }
                EN_IMP => {
                    // Keeps medium distance and throws fireballs.
                    let speed = 0.9 * dt;
                    if dist > 4.5 {
                        self.move_enemy(&mut e, nx * speed, ny * speed);
                    } else if dist < 2.5 {
                        self.move_enemy(&mut e, -nx * speed * 0.5, -ny * speed * 0.5);
                    }
                    if e.atk_cool <= 0.0 && dist < 8.0 && self.player.health > 0 {
                        let (ex, ey) = (e.x, e.y);
                        let (tx, ty) = (self.player.x, self.player.y);
                        e.atk_cool = 2.0 + self.rand_f64();
                        self.enemies[i] = e;
                        self.spawn_fireball(ex, ey, tx, ty, 0.0, 3.0, 12);
                        continue;
                    }
                }
                EN_WRAITH => {
                    // Fast and evasive: it spirals in rather than charging on a
                    // straight line, so it's awkward to keep in the crosshair.
                    // Alternating orbit direction by index keeps a pack from
                    // stacking into a single silhouette.
                    let speed = 2.0 * dt;
                    if dist > 0.75 {
                        let side = if i & 1 == 0 { 1.0 } else { -1.0 };
                        let swirl = (self.global_time * 2.2 + e.anim).sin() * 0.8 * side;
                        let mx = nx - ny * swirl;
                        let my = ny + nx * swirl;
                        let l = (mx * mx + my * my).sqrt().max(1e-6);
                        self.move_enemy(&mut e, mx / l * speed, my / l * speed);
                    } else if e.atk_cool <= 0.0 {
                        e.atk_cool = 0.9;
                        self.melee_player(8);
                    }
                }
                _ => {
                    // BARON: a bruiser. Closes to brawling range, hits hard up
                    // close, and lobs a three-way fireball fan from further out
                    // so backing off isn't a free answer.
                    let speed = 1.3 * dt;
                    if dist > 1.4 {
                        self.move_enemy(&mut e, nx * speed, ny * speed);
                    } else if e.atk_cool <= 0.0 {
                        e.atk_cool = 1.3;
                        self.melee_player(18);
                    }
                    if e.atk_cool <= 0.0 && dist >= 1.4 && dist < 10.0 && self.player.health > 0 {
                        let (ex, ey) = (e.x, e.y);
                        let (tx, ty) = (self.player.x, self.player.y);
                        e.atk_cool = 2.4 + self.rand_f64();
                        self.enemies[i] = e;
                        for k in -1..=1 {
                            self.spawn_fireball(ex, ey, tx, ty, k as f64 * 0.22, 3.6, 13);
                        }
                        continue;
                    }
                }
            }
            self.enemies[i] = e;
        }
    }

    /// Apply `dmg` to an enemy, with the blood, sound and score-on-kill that go
    /// with it. Shared by the hitscan shot and by barrel blasts.
    pub(crate) fn damage_enemy(&mut self, i: usize, dmg: i32) {
        if !self.enemies[i].alive {
            return;
        }
        self.enemies[i].hp -= dmg;
        self.enemies[i].hit_flash = 0.15;
        let (ex, ey, kind, hp) =
            (self.enemies[i].x, self.enemies[i].y, self.enemies[i].kind, self.enemies[i].hp);
        self.spawn_blood(ex, ey, 8);
        if hp <= 0 {
            self.enemies[i].alive = false;
            self.spawn_blood(ex, ey, 14);
            self.score += EN_SCORE[kind as usize];
            self.audio.play(SND_DEATH);
        } else {
            self.audio.play(SND_HIT);
        }
    }

    /// Index of a live barrel within `r` of (x,y), if any.
    pub(crate) fn barrel_at(&self, x: f64, y: f64, r: f64) -> Option<usize> {
        (0..MAX_BARRELS).find(|&i| {
            let b = self.barrels[i];
            b.alive && (b.x - x) * (b.x - x) + (b.y - y) * (b.y - y) < r * r
        })
    }

    /// Blow up barrel `idx` and everything its blast reaches: enemies take
    /// damage falling off with distance, the player takes a smaller share (so
    /// point-blank barrel-popping is a real risk), and barrels caught in the
    /// radius go up too. Chaining runs off a worklist rather than recursion, so
    /// a whole line of barrels detonates in one pass.
    pub(crate) fn explode_barrel(&mut self, idx: usize) {
        let mut queue = [0usize; MAX_BARRELS];
        let mut tail = 0usize;
        queue[tail] = idx;
        tail += 1;
        self.barrels[idx].alive = false;

        let mut head = 0usize;
        while head < tail {
            let (bx, by) = (self.barrels[queue[head]].x, self.barrels[queue[head]].y);
            head += 1;
            self.audio.play(SND_EXPLOSION);

            // Fireball puff: a bright core of embers plus a slower smoke ring.
            for k in 0..20 {
                let a = self.rand_f64() * 2.0 * PI;
                let s = 0.8 + self.rand_f64() * 2.2;
                let life = 0.35 + self.rand_f64() * 0.5;
                let c = if k % 3 == 0 { 0xFFF0A0 } else { 0xFF7020 };
                self.spawn_particle(bx, by, a.cos() * s, a.sin() * s, life, c);
            }
            for _ in 0..6 {
                let a = self.rand_f64() * 2.0 * PI;
                self.spawn_particle(bx, by, a.cos() * 0.5, a.sin() * 0.5, 0.9, 0x404040);
            }

            // Falloff factor for a target `d` away: 1 at the centre, 0 at the edge.
            let falloff = |d: f64| (1.0 - d / BARREL_RADIUS).max(0.0);

            for i in 0..MAX_ENEMIES {
                let e = self.enemies[i];
                if !e.alive {
                    continue;
                }
                let d = ((e.x - bx) * (e.x - bx) + (e.y - by) * (e.y - by)).sqrt();
                if d >= BARREL_RADIUS {
                    continue;
                }
                let dmg = ((BARREL_DAMAGE as f64 * falloff(d)) as i32).max(1);
                self.damage_enemy(i, dmg);
            }

            let pd = ((self.player.x - bx) * (self.player.x - bx)
                + (self.player.y - by) * (self.player.y - by))
                .sqrt();
            if pd < BARREL_RADIUS && self.player.health > 0 {
                let dmg = ((BARREL_SELF_DAMAGE as f64 * falloff(pd)) as i32).max(1);
                self.player.health = (self.player.health - dmg).max(0);
                self.pain_flash = 0.45;
                self.audio.play(SND_PLAYER_HURT);
            }

            for j in 0..MAX_BARRELS {
                let b = self.barrels[j];
                if !b.alive {
                    continue;
                }
                if (b.x - bx) * (b.x - bx) + (b.y - by) * (b.y - by) < BARREL_RADIUS * BARREL_RADIUS
                {
                    self.barrels[j].alive = false;
                    queue[tail] = j;
                    tail += 1;
                }
            }
        }
    }

    /// Burn the player while they stand in lava. Damage accrues fractionally so
    /// the rate is frame-rate independent, and the hurt sound is throttled so a
    /// long crossing doesn't machine-gun it.
    pub(crate) fn update_hazard(&mut self, dt: f64) {
        if !self.map_hazard(self.player.x as i32, self.player.y as i32) {
            self.hazard_burn = 0.0;
            return;
        }
        self.hazard_burn += HAZARD_DPS * dt;
        let whole = self.hazard_burn as i32;
        if whole <= 0 {
            return;
        }
        self.hazard_burn -= whole as f64;
        self.player.health = (self.player.health - whole).max(0);
        if self.pain_flash < 0.2 {
            self.pain_flash = 0.2;
        }
        if self.hazard_snd_t <= 0.0 {
            self.hazard_snd_t = 0.7;
            self.audio.play(SND_PLAYER_HURT);
        }
    }

    pub(crate) fn update_pickups(&mut self) {
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

    pub(crate) fn shoot(&mut self) {
        if self.player.ammo <= 0 {
            return;
        }
        self.player.ammo -= 1;
        self.muzzle_flash = 5;
        self.audio.play(SND_SHOOT);

        // Per-weapon fire pattern: (pellet count, half-spread radians, damage).
        // The pistol is a balanced single shot; the shotgun sprays several
        // low-damage pellets; the rifle is a single high-damage, pinpoint shot.
        let (pellets, spread, dmg) = match self.player.weapon {
            WP_SHOTGUN => (5, 0.13, 1),
            WP_RIFLE => (1, 0.0, 3),
            _ => (1, 0.0, 1),
        };
        for k in 0..pellets {
            // First pellet is always dead-center, so the shotgun is never worse
            // than the pistol against a point target; the rest fan out evenly.
            let off = if pellets == 1 || k == 0 {
                0.0
            } else {
                ((k as f64) / (pellets as f64 - 1.0) - 0.5) * 2.0 * spread
            };
            self.fire_ray(off, dmg);
        }
    }

    /// Trace one hitscan pellet at `angle_off` from the aim direction, stopping
    /// at the nearest wall and damaging the nearest enemy along it by `dmg`.
    pub(crate) fn fire_ray(&mut self, angle_off: f64, dmg: i32) {
        let ang = self.player.angle + angle_off;
        let rx = ang.cos();
        let ry = ang.sin();

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

        // Nearest thing the pellet lines up with, out to the wall it stopped at.
        // Barrels compete with enemies for the hit, so a barrel in front of a
        // pack eats the shot — and pays it back with the blast.
        let (px, py) = (self.player.x, self.player.y);
        let mut best_dist = wall_t;
        let mut best: Option<(bool, usize)> = None; // (is_barrel, index)
        for i in 0..MAX_ENEMIES {
            let e = self.enemies[i];
            if !e.alive {
                continue;
            }
            if let Some(d) = shot_hit_dist(px, py, ang, e.x, e.y) {
                if d < best_dist {
                    best_dist = d;
                    best = Some((false, i));
                }
            }
        }
        for i in 0..MAX_BARRELS {
            let b = self.barrels[i];
            if !b.alive {
                continue;
            }
            if let Some(d) = shot_hit_dist(px, py, ang, b.x, b.y) {
                if d < best_dist {
                    best_dist = d;
                    best = Some((true, i));
                }
            }
        }
        match best {
            Some((true, i)) => self.explode_barrel(i),
            Some((false, i)) => self.damage_enemy(i, dmg),
            None => {}
        }
    }

    pub fn update_game(&mut self, dt: f64) {
        self.global_time += dt;
        if self.pain_flash > 0.0 {
            self.pain_flash -= dt;
        }
        if self.hazard_snd_t > 0.0 {
            self.hazard_snd_t -= dt;
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

            // Direct weapon select (1/2/3) — only switches to owned weapons.
            for (key, wp) in [(K_WEAPON1, WP_PISTOL), (K_WEAPON2, WP_SHOTGUN), (K_WEAPON3, WP_RIFLE)]
            {
                if self.key_edge[key] {
                    if self.player.weapons[wp as usize] {
                        self.player.weapon = wp;
                    }
                    self.key_edge[key] = false;
                }
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
            self.update_hazard(dt);
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
