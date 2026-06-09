//! The AI player. It drives the same `keys`/`key_edge` a human would, reading
//! full world state: BFS pathfinding around walls, line-of-sight gated firing,
//! range control, fireball dodging, pickup seeking, stuck recovery, and
//! auto-restart for an endless attract-mode demo.

use crate::constants::*;
use crate::game::Game;
use std::f64::consts::PI;

impl Game {
    /// True if the straight segment a->b is unobstructed by walls.
    pub fn bot_los(&self, ax: f64, ay: f64, bx: f64, by: f64) -> bool {
        let dx = bx - ax;
        let dy = by - ay;
        let d = (dx * dx + dy * dy).sqrt();
        let steps = (d / 0.05) as i32 + 1;
        for i in 1..steps {
            let t = i as f64 / steps as f64;
            if self.map_blocked((ax + dx * t) as i32, (ay + dy * t) as i32) {
                return false;
            }
        }
        true
    }

    /// BFS distance field (in tiles) from (tx,ty) over all walkable cells;
    /// unreachable cells stay -1.
    pub fn bot_field(&self, tx: i32, ty: i32) -> [[i32; MAP_W]; MAP_H] {
        let mut field = [[-1i32; MAP_W]; MAP_H];
        if tx < 0 || tx >= MAP_W as i32 || ty < 0 || ty >= MAP_H as i32 || self.map_blocked(tx, ty) {
            return field;
        }
        let mut qx = [0i32; MAP_W * MAP_H];
        let mut qy = [0i32; MAP_W * MAP_H];
        let (mut head, mut tail) = (0usize, 0usize);
        field[ty as usize][tx as usize] = 0;
        qx[tail] = tx;
        qy[tail] = ty;
        tail += 1;
        let ox = [1, -1, 0, 0];
        let oy = [0, 0, 1, -1];
        while head < tail {
            let cx = qx[head];
            let cy = qy[head];
            head += 1;
            for k in 0..4 {
                let nx = cx + ox[k];
                let ny = cy + oy[k];
                if nx < 0 || nx >= MAP_W as i32 || ny < 0 || ny >= MAP_H as i32 {
                    continue;
                }
                if field[ny as usize][nx as usize] != -1 || self.map_blocked(nx, ny) {
                    continue;
                }
                field[ny as usize][nx as usize] = field[cy as usize][cx as usize] + 1;
                qx[tail] = nx;
                qy[tail] = ny;
                tail += 1;
            }
        }
        field
    }

    pub fn bot_think(&mut self, dt: f64) {
        // Intro screen: any key edge dismisses it.
        if self.show_intro {
            self.key_edge[K_FWD] = true;
            return;
        }

        // Game over: pause, then restart to keep the demo running forever.
        if self.score_saved {
            self.bot.restart_t += dt;
            if self.bot.restart_t > 3.0 {
                self.key_edge[K_RESTART] = true;
                self.bot.restart_t = 0.0;
            }
            return;
        }
        self.bot.restart_t = 0.0;

        // We fully own the movement keys each frame (no key-release events feed
        // the bot). Leave QUIT/RESTART edges from real input untouched.
        self.keys[K_FWD] = false;
        self.keys[K_BACK] = false;
        self.keys[K_STRAFEL] = false;
        self.keys[K_STRAFER] = false;
        self.keys[K_TURNL] = false;
        self.keys[K_TURNR] = false;
        if self.player.health <= 0 {
            return;
        }

        let px = self.player.x;
        let py = self.player.y;

        // Reachability field rooted at the player: pf[cell] >= 0 means we can
        // walk there. Lets the bot ignore targets sealed behind walls.
        let pf = self.bot_field(px as i32, py as i32);
        let reach = |wx: f64, wy: f64| pf[wy as usize][wx as usize] >= 0;

        // ---- Pick a goal: survival pickups first, else nearest enemy, else
        //      any leftover pickup. A pickup must be reachable; an enemy must be
        //      reachable or in direct sight. ----
        let mut goal_kind = 0; // 0 none, 1 enemy, 2 pickup
        let mut gx = 0.0;
        let mut gy = 0.0;
        let mut best = 1e9;
        let low_hp = self.player.health < 40;
        let low_ammo = self.player.ammo <= 1;

        if low_hp || low_ammo {
            for i in 0..MAX_PICKUPS {
                let p = self.pickups[i];
                if !p.alive {
                    continue;
                }
                if !((low_hp && p.kind == PU_HEALTH) || (low_ammo && p.kind == PU_AMMO)) {
                    continue;
                }
                if !reach(p.x, p.y) {
                    continue;
                }
                let dx = p.x - px;
                let dy = p.y - py;
                let d = dx * dx + dy * dy;
                if d < best {
                    best = d;
                    gx = p.x;
                    gy = p.y;
                    goal_kind = 2;
                }
            }
        }
        if goal_kind == 0 {
            for i in 0..MAX_ENEMIES {
                let e = self.enemies[i];
                if !e.alive {
                    continue;
                }
                if !reach(e.x, e.y) && !self.bot_los(px, py, e.x, e.y) {
                    continue;
                }
                let dx = e.x - px;
                let dy = e.y - py;
                let d = dx * dx + dy * dy;
                if d < best {
                    best = d;
                    gx = e.x;
                    gy = e.y;
                    goal_kind = 1;
                }
            }
        }
        if goal_kind == 0 {
            for i in 0..MAX_PICKUPS {
                let p = self.pickups[i];
                if !p.alive {
                    continue;
                }
                // The bot only values consumables (health/ammo); weapon pickups
                // are a player convenience it never detours for.
                if p.kind != PU_HEALTH && p.kind != PU_AMMO {
                    continue;
                }
                if !reach(p.x, p.y) {
                    continue;
                }
                let dx = p.x - px;
                let dy = p.y - py;
                let d = dx * dx + dy * dy;
                if d < best {
                    best = d;
                    gx = p.x;
                    gy = p.y;
                    goal_kind = 2;
                }
            }
        }
        if goal_kind == 0 {
            return; // nothing reachable; level auto-clears
        }

        let gdist = ((gx - px) * (gx - px) + (gy - py) * (gy - py)).sqrt();
        let have_los = self.bot_los(px, py, gx, gy);
        // "See" an enemy if there's line-of-sight, or it's point-blank (where
        // the LOS ray can clip a wall corner and give a false negative).
        let see_enemy = goal_kind == 1 && (have_los || gdist < 1.5) && gdist < 9.0;

        // Steer straight at the goal when visible, else toward the next BFS
        // waypoint (the adjacent walkable cell closest to the goal).
        let aim_x;
        let aim_y;
        if have_los {
            aim_x = gx;
            aim_y = gy;
        } else {
            let field = self.bot_field(gx as i32, gy as i32);
            let cx = px as i32;
            let cy = py as i32;
            let mut bestv = field[cy as usize][cx as usize];
            let mut wx = cx;
            let mut wy = cy;
            let ox = [1, -1, 0, 0];
            let oy = [0, 0, 1, -1];
            for k in 0..4 {
                let nx = cx + ox[k];
                let ny = cy + oy[k];
                if nx < 0 || nx >= MAP_W as i32 || ny < 0 || ny >= MAP_H as i32 {
                    continue;
                }
                let f = field[ny as usize][nx as usize];
                if f < 0 {
                    continue;
                }
                if bestv < 0 || f < bestv {
                    bestv = f;
                    wx = nx;
                    wy = ny;
                }
            }
            aim_x = wx as f64 + 0.5;
            aim_y = wy as f64 + 0.5;
        }

        // Incoming fireball? Note the perpendicular so we can sidestep it.
        let mut dodge = false;
        let mut dodge_x = 0.0;
        let mut dodge_y = 0.0;
        for i in 0..MAX_FIREBALLS {
            let fb = self.fireballs[i];
            if !fb.alive {
                continue;
            }
            let rx = px - fb.x;
            let ry = py - fb.y;
            if rx * rx + ry * ry > 9.0 {
                continue; // too far to matter
            }
            if fb.vx * rx + fb.vy * ry <= 0.0 {
                continue; // heading away
            }
            dodge = true;
            dodge_x = -fb.vy;
            dodge_y = fb.vx;
            break;
        }

        // ---- Decide where to face and a world-space move vector. Keeping the
        // move vector independent of facing lets the bot back straight away from
        // a grunt while still spinning to bring it into the crosshair. ----
        let face_x;
        let face_y;
        let mut mvx = 0.0;
        let mut mvy = 0.0;
        if see_enemy {
            face_x = gx;
            face_y = gy; // aim at the enemy
            let ux = (gx - px) / gdist;
            let uy = (gy - py) / gdist;
            if gdist > 4.0 {
                mvx = ux;
                mvy = uy; // close in
            } else if gdist < 2.2 {
                mvx = -ux;
                mvy = -uy; // open up
            }
            // else hold position in the 2.2..4.0 sweet spot and shoot
        } else {
            face_x = aim_x;
            face_y = aim_y; // head to the waypoint/goal
            let ax = aim_x - px;
            let ay = aim_y - py;
            let al = (ax * ax + ay * ay).sqrt();
            if al > 1e-6 {
                mvx = ax / al;
                mvy = ay / al;
            }
        }
        if dodge {
            // fold a strong perpendicular sidestep into the move
            let dl = (dodge_x * dodge_x + dodge_y * dodge_y).sqrt();
            if dl > 1e-6 {
                mvx += 1.5 * dodge_x / dl;
                mvy += 1.5 * dodge_y / dl;
            }
        }

        // ---- Turn toward the face target (bang-bang with a small deadzone). ----
        let desired = (face_y - py).atan2(face_x - px);
        let mut err = desired - self.player.angle;
        while err > PI {
            err -= 2.0 * PI;
        }
        while err < -PI {
            err += 2.0 * PI;
        }
        if err > 0.05 {
            self.keys[K_TURNR] = true;
        } else if err < -0.05 {
            self.keys[K_TURNL] = true;
        }

        // ---- Translate the world move vector into held keys via the player's
        // forward and strafe axes. ----
        let fx = self.player.angle.cos();
        let fy = self.player.angle.sin();
        let srx = -self.player.angle.sin();
        let sry = self.player.angle.cos();
        let fwd = mvx * fx + mvy * fy;
        let strafe = mvx * srx + mvy * sry;
        if fwd > 0.25 {
            self.keys[K_FWD] = true;
        } else if fwd < -0.25 {
            self.keys[K_BACK] = true;
        }
        if strafe > 0.25 {
            self.keys[K_STRAFER] = true;
        } else if strafe < -0.25 {
            self.keys[K_STRAFEL] = true;
        }

        // ---- Stuck recovery: if we want to move but aren't, juke for a moment. ----
        let moved = (px - self.bot.last_x) * (px - self.bot.last_x)
            + (py - self.bot.last_y) * (py - self.bot.last_y);
        self.bot.last_x = px;
        self.bot.last_y = py;
        let want_move = (mvx * mvx + mvy * mvy) > 1e-6;
        if want_move && moved < 1e-4 {
            self.bot.stuck_t += dt;
        } else {
            self.bot.stuck_t = 0.0;
        }
        if self.bot.stuck_t > 0.35 {
            self.bot.unstuck_t = 0.5;
            self.bot.flip = !self.bot.flip;
            self.bot.stuck_t = 0.0;
        }
        if self.bot.unstuck_t > 0.0 {
            self.bot.unstuck_t -= dt;
            self.keys[K_FWD] = true;
            if self.bot.flip {
                self.keys[K_STRAFER] = true;
            } else {
                self.keys[K_STRAFEL] = true;
            }
        }

        // ---- Fire when locked on, with a cadence so ammo isn't dumped. ----
        if self.bot.fire_t > 0.0 {
            self.bot.fire_t -= dt;
        }
        if see_enemy && self.player.ammo > 0 && self.bot.fire_t <= 0.0 {
            let mut tol = 0.22 / (if gdist < 1.0 { 1.0 } else { gdist });
            if tol < 0.04 {
                tol = 0.04;
            }
            if err.abs() <= tol {
                self.key_edge[K_SHOOT] = true;
                self.bot.fire_t = 0.16;
            }
        }
    }
}
