//! `--selftest`: validates level data (geometry, spawns, reachability) and
//! exercises a few frames of each level headlessly. Prints PASS/FAIL, returns
//! a process exit code.

use crate::constants::*;
use crate::game::Game;
use crate::level::LEVELS;

fn check(ok: &mut bool, cond: bool, what: &str) {
    if !cond {
        eprintln!("SELFTEST FAIL: {}", what);
    }
    *ok &= cond;
}

pub fn run_self_test() -> i32 {
    let mut ok = true;
    let valid = "#=BDH.pgihasr";
    let mut g = Game::new();

    for n in 0..LEVEL_COUNT {
        let mut player_count = 0;
        for y in 0..MAP_H {
            let row = LEVELS[n][y];
            check(&mut ok, row.len() == MAP_W, &format!("level {} row {} length", n, y));
            for (x, ch) in row.chars().enumerate() {
                if ch == 'p' {
                    player_count += 1;
                }
                check(
                    &mut ok,
                    valid.contains(ch),
                    &format!("level {} ({},{}) char '{}' is valid", n, x, y, ch),
                );
            }
        }
        check(&mut ok, player_count == 1, &format!("level {} has exactly one player spawn", n));

        g.reset_game();
        g.load_level(n);
        g.show_intro = false;

        check(
            &mut ok,
            g.player.x >= 0.0
                && g.player.x < MAP_W as f64
                && g.player.y >= 0.0
                && g.player.y < MAP_H as f64,
            &format!("level {} player spawn is in-bounds", n),
        );
        check(
            &mut ok,
            !g.map_blocked(g.player.x as i32, g.player.y as i32),
            &format!("level {} player does not spawn inside a wall", n),
        );
        check(&mut ok, g.level_enemy_count > 0, &format!("level {} has at least one enemy", n));

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
                check(
                    &mut ok,
                    seen[g.enemies[i].y as usize][g.enemies[i].x as usize],
                    &format!("level {} enemy {} is reachable from spawn", n, i),
                );
            }
            for i in 0..MAX_PICKUPS {
                if !g.pickups[i].alive {
                    continue;
                }
                check(
                    &mut ok,
                    seen[g.pickups[i].y as usize][g.pickups[i].x as usize],
                    &format!("level {} pickup {} is reachable from spawn", n, i),
                );
            }
        }

        for _ in 0..60 {
            g.update_game(1.0 / 60.0);
            g.render_frame();
        }

        check(
            &mut ok,
            g.player.health >= 0 && g.player.health <= 100,
            &format!("level {} player health stays in range after running", n),
        );
        check(&mut ok, g.running, &format!("level {} game keeps running after a few frames", n));
    }

    if ok {
        println!("SELFTEST PASS ({} levels)", LEVEL_COUNT);
        0
    } else {
        println!("SELFTEST FAILED");
        1
    }
}
