//! `--bench`: render a fixed set of camera views and report frame times.
//!
//! Two workloads. "levels" stands at every level's spawn and turns through
//! eight headings, which is what most of play looks like. "close combat" puts
//! each enemy kind straight in front of the camera at a few distances, the
//! worst case for the sprite path. Each view is drawn once to warm up, then
//! `reps` times under the clock. Per view the report takes the *median* frame,
//! so a busy machine (another build, a browser) skews it far less than a mean
//! would; compare builds by running them back to back.
//!
//! Only public game API is used (load a level, place the camera, render), so
//! the same file drops into an older checkout for an A/B comparison.

use crate::constants::*;
use crate::game::Game;
use std::time::Instant;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Time `reps` renders of the game's current view: median ms per frame.
fn time_view(g: &mut Game, reps: usize) -> f64 {
    g.render_frame(); // warm-up: first touch of textures and buffers
    let mut times = Vec::with_capacity(reps);
    for _ in 0..reps {
        let t0 = Instant::now();
        g.render_frame();
        times.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    median(times)
}

/// Per-stage split of one frame, mirroring `render_frame`'s order: lighting
/// update, wall columns, floor/ceiling/sky, sprites, then everything drawn
/// over the view (gun, HUD, minimap). Median milliseconds per frame.
fn stage_split(g: &mut Game, reps: usize) -> [f64; 5] {
    let mut t: [Vec<f64>; 5] = Default::default();
    for _ in 0..reps {
        let dir_x = g.player.angle.cos();
        let dir_y = g.player.angle.sin();
        let plane_half = (FOV / 2.0).tan();
        let (plane_x, plane_y) = (-dir_y * plane_half, dir_x * plane_half);
        let t0 = Instant::now();
        g.update_lighting();
        let t1 = Instant::now();
        for x in 0..SCREEN_W {
            g.cast_column(x, dir_x, dir_y, plane_x, plane_y);
        }
        let t2 = Instant::now();
        g.render_floor_ceiling(dir_x, dir_y, plane_x, plane_y);
        let t3 = Instant::now();
        g.render_sprites();
        let t4 = Instant::now();
        g.draw_crosshair();
        g.draw_weapon();
        g.draw_hud();
        g.draw_minimap();
        g.draw_score_readout();
        g.post_process();
        let t5 = Instant::now();
        for (i, (a, b)) in [(t0, t1), (t1, t2), (t2, t3), (t3, t4), (t4, t5)].iter().enumerate() {
            t[i].push((*b - *a).as_secs_f64() * 1000.0);
        }
    }
    t.map(median)
}

fn level_views(reps: usize) -> Vec<f64> {
    let mut per_level = Vec::new();
    for lv in 0..LEVEL_COUNT {
        let mut g = Game::new();
        g.load_level(lv);
        g.show_intro = false;
        g.global_time = 1.0;
        let (sx, sy) = (g.player.x, g.player.y);
        let mut total = 0.0;
        for k in 0..8 {
            g.player.x = sx;
            g.player.y = sy;
            g.player.angle = k as f64 * std::f64::consts::FRAC_PI_4;
            total += time_view(&mut g, reps);
        }
        per_level.push(total / 8.0);
    }
    per_level
}

fn close_combat(reps: usize) -> Vec<(i32, f64, f64)> {
    let mut out = Vec::new();
    for kind in [EN_GRUNT, EN_IMP, EN_WRAITH, EN_BARON] {
        for dist in [1.2, 1.7, 2.5] {
            let mut g = Game::new();
            g.load_level(0);
            g.show_intro = false;
            g.global_time = 1.0;
            for e in g.enemies.iter_mut() {
                e.alive = false;
            }
            // Level 1 has a long open run east of the spawn at (1.5, 1.5).
            g.player.x = 1.5;
            g.player.y = 1.5;
            g.player.angle = 0.0;
            g.enemies[0].x = 1.5 + dist;
            g.enemies[0].y = 1.5;
            g.enemies[0].kind = kind;
            g.enemies[0].alive = true;
            g.enemies[0].hp = EN_HP[kind as usize];
            out.push((kind, dist, time_view(&mut g, reps)));
        }
    }
    out
}

/// Run both workloads and print the report. Returns the process exit code.
pub fn run_bench(reps: usize) -> i32 {
    let reps = reps.max(1);
    println!("bench: {reps} frames per view, {SCREEN_W}x{SCREEN_H}");
    let levels = level_views(reps);
    for (i, ms) in levels.iter().enumerate() {
        println!("  level {}            {:7.3} ms/frame", i + 1, ms);
    }
    let lv_mean = levels.iter().sum::<f64>() / levels.len() as f64;
    println!("  levels mean        {lv_mean:7.3} ms/frame");

    // Where the time goes, averaged over the same level views.
    let mut split = [0.0f64; 5];
    for lv in 0..LEVEL_COUNT {
        let mut g = Game::new();
        g.load_level(lv);
        g.show_intro = false;
        g.global_time = 1.0;
        g.render_frame();
        for k in 0..8 {
            g.player.angle = k as f64 * std::f64::consts::FRAC_PI_4;
            for (s, v) in split.iter_mut().zip(stage_split(&mut g, reps)) {
                *s += v / (8 * LEVEL_COUNT) as f64;
            }
        }
    }
    let names = ["lighting", "walls", "floor+ceiling", "sprites", "hud+overlay"];
    let parts: Vec<String> = names.iter().zip(split).map(|(n, v)| format!("{n} {v:.3}")).collect();
    println!("  stages (ms)        {}", parts.join(", "));

    let combat = close_combat(reps);
    let names = ["grunt", "imp", "wraith", "baron"];
    for (kind, dist, ms) in &combat {
        println!("  {:<6} at {:.1}       {:7.3} ms/frame", names[*kind as usize], dist, ms);
    }
    let cc_mean = combat.iter().map(|c| c.2).sum::<f64>() / combat.len() as f64;
    let cc_max = combat.iter().map(|c| c.2).fold(0.0, f64::max);
    println!("  close combat mean  {cc_mean:7.3} ms/frame (worst {cc_max:.3})");
    0
}
