//! `--bench`: render a fixed set of camera views and report frame times.
//!
//! Two workloads. "levels" stands at every level's spawn and turns through
//! eight headings, which is what most of play looks like. "close combat" puts
//! each enemy kind straight in front of the camera at a few distances, the
//! worst case for the sprite path. Each view is drawn once to warm up, then
//! `reps` times under the clock. Per view the report takes the *fastest* frame,
//! the one least disturbed by a busy machine (another build, a browser);
//! compare builds by running them back to back.
//!
//! Both workloads run at three settings: the logical 640x400 on one thread
//! (comparable with older builds), the window's default 2x on one thread
//! (what the extra pixels cost), and 2x on `threads` threads (what the window
//! actually runs).

use crate::constants::*;
use crate::game::Game;
use crate::render::{STAGE_FLATS, STAGE_SPRITES, STAGE_WALLS};
use std::time::Instant;

/// Fastest of `reps` runs of `f` in milliseconds, after one warm-up run. The
/// minimum is the run least disturbed by whatever else the machine is doing.
fn time_ms(reps: usize, mut f: impl FnMut()) -> f64 {
    f(); // warm-up: first touch of textures and buffers
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let t0 = Instant::now();
        f();
        best = best.min(t0.elapsed().as_secs_f64() * 1000.0);
    }
    best
}

fn new_game(scale: usize, threads: usize, level: usize) -> Game {
    let mut g = Game::new();
    g.set_scale(scale);
    g.threads = threads;
    g.load_level(level);
    g.show_intro = false;
    g.global_time = 1.0;
    g
}

/// Mean over all levels and eight headings of the median frame time.
fn level_views(reps: usize, scale: usize, threads: usize) -> f64 {
    let mut total = 0.0;
    for lv in 0..LEVEL_COUNT {
        let mut g = new_game(scale, threads, lv);
        for k in 0..8 {
            g.player.angle = k as f64 * std::f64::consts::FRAC_PI_4;
            total += time_ms(reps, || g.render_frame());
        }
    }
    total / (8 * LEVEL_COUNT) as f64
}

/// Per-stage split of a frame on one thread, averaged over the level views:
/// lighting update, walls, floor/ceiling/sky, sprites, HUD and overlays.
fn stage_split(reps: usize, scale: usize) -> [f64; 5] {
    let mut split = [0.0f64; 5];
    for lv in 0..LEVEL_COUNT {
        let mut g = new_game(scale, 1, lv);
        g.render_frame();
        for k in 0..8 {
            g.player.angle = k as f64 * std::f64::consts::FRAC_PI_4;
            let t = [
                time_ms(reps, || g.update_lighting()),
                time_ms(reps, || g.render_view(STAGE_WALLS)),
                time_ms(reps, || g.render_view(STAGE_FLATS)),
                time_ms(reps, || g.render_view(STAGE_SPRITES)),
                time_ms(reps, || g.draw_overlay()),
            ];
            // Each view pass also casts the wall columns; charge that to walls.
            let cast = time_ms(reps, || g.render_view(0));
            for (i, v) in t.iter().enumerate() {
                let v = if i == 2 || i == 3 { (v - cast).max(0.0) } else { *v };
                split[i] += v / (8 * LEVEL_COUNT) as f64;
            }
        }
    }
    split
}

/// Median frame time with each enemy kind close in front of the camera:
/// `(mean, worst)` over kinds and distances.
fn close_combat(reps: usize, scale: usize, threads: usize) -> (f64, f64) {
    let mut times = Vec::new();
    for kind in [EN_GRUNT, EN_IMP, EN_WRAITH, EN_BARON] {
        for dist in [1.2, 1.7, 2.5] {
            let mut g = new_game(scale, threads, 0);
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
            times.push(time_ms(reps, || g.render_frame()));
        }
    }
    (times.iter().sum::<f64>() / times.len() as f64, times.iter().fold(0.0, |a, &b| a.max(b)))
}

/// Run both workloads and print the report. Returns the process exit code.
pub fn run_bench(reps: usize, threads: usize) -> i32 {
    let reps = reps.max(1);
    let threads = threads.max(1);
    println!("bench: best of {reps} frames per view, ms/frame");
    println!("  {:<26} {:>8} {:>14} {:>14}", "setting", "levels", "combat mean", "combat worst");
    for (scale, t) in [(1, 1), (2, 1), (2, threads)] {
        let lv = level_views(reps, scale, t);
        let (mean, worst) = close_combat(reps, scale, t);
        let name = format!("{}x{} on {} thread{}", SCREEN_W * scale, SCREEN_H * scale, t, if t == 1 { "" } else { "s" });
        println!("  {name:<26} {lv:>8.3} {mean:>14.3} {worst:>14.3}");
    }
    let names = ["lighting", "walls", "floor+ceiling", "sprites", "hud+overlay"];
    for scale in [1, 2] {
        let parts: Vec<String> =
            names.iter().zip(stage_split(reps, scale)).map(|(n, v)| format!("{n} {v:.3}")).collect();
        println!("  stages at {}x, 1 thread: {}", scale, parts.join(", "));
    }
    0
}
