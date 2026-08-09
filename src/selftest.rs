//! `--selftest`: validates level data (geometry, spawns, reachability) and
//! exercises a few frames of each level headlessly. Prints PASS/FAIL, returns
//! a process exit code.
//!
//! The checking is [`validate_levels`], which takes the level table as an
//! argument and *returns* its complaints rather than printing them. That keeps
//! the validator itself testable — it is the guard for every level edit, so a
//! silently broken check would be worse than no check at all.

use crate::constants::*;
use crate::game::Game;
use crate::level::LEVELS;

/// Collects the reasons a level set was rejected.
#[derive(Default)]
pub struct Failures(pub Vec<String>);

impl Failures {
    fn check(&mut self, cond: bool, what: impl FnOnce() -> String) {
        if !cond {
            self.0.push(what());
        }
    }

    pub fn ok(&self) -> bool {
        self.0.is_empty()
    }
}

pub fn run_self_test() -> i32 {
    let failures = validate_levels(&LEVELS);
    for f in &failures.0 {
        eprintln!("SELFTEST FAIL: {}", f);
    }
    if failures.ok() {
        println!("SELFTEST PASS ({} levels)", LEVEL_COUNT);
        0
    } else {
        println!("SELFTEST FAILED");
        1
    }
}

/// Validate every level in `levels`: geometry, spawn markers, entity capacity,
/// reachability from the spawn, and a short headless run of each.
pub fn validate_levels(levels: &[[&str; MAP_H]]) -> Failures {
    let mut f = Failures::default();
    let valid = "#=BDHT.~pgiwkhasro";
    let mut g = Game::new();

    for n in 0..levels.len() {
        let mut player_count = 0;
        let (mut enemy_count, mut pickup_count, mut barrel_count) = (0, 0, 0);
        let mut geometry_ok = true;
        for y in 0..MAP_H {
            let row = levels[n][y];
            geometry_ok &= row.len() == MAP_W;
            f.check(row.len() == MAP_W, || format!("level {} row {} length", n, y));
            for (x, ch) in row.chars().enumerate() {
                match ch {
                    'p' => player_count += 1,
                    'g' | 'i' | 'w' | 'k' => enemy_count += 1,
                    'h' | 'a' | 's' | 'r' => pickup_count += 1,
                    'o' => barrel_count += 1,
                    _ => {}
                }
                f.check(valid.contains(ch),
                    || format!("level {} ({},{}) char '{}' is valid", n, x, y, ch));
            }
        }
        f.check(player_count == 1, || format!("level {} has exactly one player spawn", n));

        // Over-capacity spawns are silently dropped by the loader, which would
        // quietly delete part of a level's design — catch it here instead.
        f.check(enemy_count <= MAX_ENEMIES,
            || format!("level {} enemy count {} fits MAX_ENEMIES", n, enemy_count));
        f.check(pickup_count <= MAX_PICKUPS,
            || format!("level {} pickup count {} fits MAX_PICKUPS", n, pickup_count));
        f.check(barrel_count <= MAX_BARRELS,
            || format!("level {} barrel count {} fits MAX_BARRELS", n, barrel_count));

        // The loader indexes a fixed row width, so a map that failed the
        // geometry check can't be run — report what we have and move on.
        if !geometry_ok {
            continue;
        }

        g.reset_game();
        g.load_level_map(&levels[n], n);
        g.show_intro = false;

        f.check(g.player.x >= 0.0
                && g.player.x < MAP_W as f64
                && g.player.y >= 0.0
                && g.player.y < MAP_H as f64,
            || format!("level {} player spawn is in-bounds", n));
        f.check(!g.map_blocked(g.player.x as i32, g.player.y as i32),
            || format!("level {} player does not spawn inside a wall", n));
        f.check(!g.map_hazard(g.player.x as i32, g.player.y as i32),
            || format!("level {} player does not spawn in lava", n));
        f.check(g.level_enemy_count > 0, || format!("level {} has at least one enemy", n));
        f.check(g.level_enemy_count == enemy_count as i32,
            || format!("level {} spawned every enemy in the map", n));

        // Flood-fill walkable cells from spawn, then assert every enemy and
        // pickup is reachable. A sealed-off enemy can never be killed, so the
        // level would never clear (all_enemies_dead stays false forever).
        {
            let mut seen = [[false; MAP_W]; MAP_H];
            let mut qx = [0i32; MAP_W * MAP_H];
            let mut qy = [0i32; MAP_W * MAP_H];
            let (mut head, mut tail) = (0usize, 0usize);
            let sx = g.player.x as i32;
            let sy = g.player.y as i32;
            seen[sy as usize][sx as usize] = true;
            qx[tail] = sx;
            qy[tail] = sy;
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
                    if seen[ny as usize][nx as usize] || g.map_blocked(nx, ny) {
                        continue;
                    }
                    seen[ny as usize][nx as usize] = true;
                    qx[tail] = nx;
                    qy[tail] = ny;
                    tail += 1;
                }
            }
            for i in 0..MAX_ENEMIES {
                if !g.enemies[i].alive {
                    continue;
                }
                f.check(seen[g.enemies[i].y as usize][g.enemies[i].x as usize],
                    || format!("level {} enemy {} is reachable from spawn", n, i));
            }
            for i in 0..MAX_PICKUPS {
                if !g.pickups[i].alive {
                    continue;
                }
                f.check(seen[g.pickups[i].y as usize][g.pickups[i].x as usize],
                    || format!("level {} pickup {} is reachable from spawn", n, i));
            }
            for i in 0..MAX_BARRELS {
                if !g.barrels[i].alive {
                    continue;
                }
                f.check(seen[g.barrels[i].y as usize][g.barrels[i].x as usize],
                    || format!("level {} barrel {} is reachable from spawn", n, i));
            }
        }

        for _ in 0..60 {
            g.update_game(1.0 / 60.0);
            g.render_frame();
        }

        f.check(g.player.health >= 0 && g.player.health <= 100,
            || format!("level {} player health stays in range after running", n));
        f.check(g.running, || format!("level {} game keeps running after a few frames", n));
    }

    f
}
