//! DOOM-style raycasting FPS — a Rust port of the original single-file `doom.c`.
//!
//! Controls: W/Up move, S/Down back, A/D strafe, Left/Right turn, Space shoot,
//! R restart (after death), Esc quit.
//!
//! Flags: `--headless` (no window, fixed timestep), `--frames N` (stop after N),
//! `--bot` (AI plays), `--selftest` (validate levels and exit 0/1).
#![allow(dead_code)]

mod audio;
mod bot;
mod color;
mod constants;
mod entity;
mod game;
mod hud;
mod level;
mod render;
mod selftest;
mod sprites;
mod textures;
mod types;

use constants::*;
use game::Game;
use minifb::{Key, Scale, Window, WindowOptions};
use std::time::Instant;

/// Physical keys mapped onto each game action (mirrors the X11/Win32 mapping).
const KEYMAP: &[(usize, &[Key])] = &[
    (K_FWD, &[Key::W, Key::Up]),
    (K_BACK, &[Key::S, Key::Down]),
    (K_STRAFEL, &[Key::A]),
    (K_STRAFER, &[Key::D]),
    (K_TURNL, &[Key::Left]),
    (K_TURNR, &[Key::Right]),
    (K_SHOOT, &[Key::Space]),
    (K_RESTART, &[Key::R]),
    (K_QUIT, &[Key::Escape]),
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut headless = false;
    let mut selftest = false;
    let mut bot = false;
    let mut max_frames: i64 = -1;
    let mut shot: Option<String> = None;
    let mut shot_level = 0usize;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--headless" => headless = true,
            "--selftest" => selftest = true,
            "--bot" => bot = true,
            "--frames" => {
                if i + 1 < args.len() {
                    max_frames = args[i + 1].parse().unwrap_or(-1);
                    i += 1;
                }
            }
            "--shot" => {
                if i + 1 < args.len() {
                    shot = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--shot-level" => {
                if i + 1 < args.len() {
                    shot_level = args[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if selftest {
        std::process::exit(selftest::run_self_test());
    }

    // Render a single frame of one level to a PPM and exit (renderer check).
    if let Some(path) = shot {
        let mut g = Game::new();
        g.load_high_scores();
        g.reset_game();
        if shot_level > 0 {
            g.load_level(shot_level.min(LEVEL_COUNT - 1));
        }
        g.show_intro = false;
        g.render_frame();
        write_ppm(&path, &g.pixels);
        return;
    }

    let mut g = Game::new();
    g.load_high_scores();
    // Audio only makes sense for the interactive window; headless/bot test runs
    // skip it (no point spawning a PCM player, and it keeps them deterministic).
    if !headless {
        g.audio.init();
    }
    g.reset_game();

    let frames = if headless {
        run_headless(&mut g, bot, max_frames)
    } else {
        run_windowed(&mut g, bot, max_frames)
    };

    if bot {
        println!(
            "[bot] done: {} frames, level {}, final score {}",
            frames,
            g.level + 1,
            g.score
        );
    }
    g.audio.shutdown();
}

/// Dump the 0RGB framebuffer as a binary PPM (P6) for offline inspection.
fn write_ppm(path: &str, pixels: &[u32]) {
    let mut buf = Vec::with_capacity(SCREEN_W * SCREEN_H * 3 + 32);
    buf.extend_from_slice(format!("P6\n{} {}\n255\n", SCREEN_W, SCREEN_H).as_bytes());
    for &px in pixels {
        buf.push(((px >> 16) & 0xFF) as u8);
        buf.push(((px >> 8) & 0xFF) as u8);
        buf.push((px & 0xFF) as u8);
    }
    if let Err(e) = std::fs::write(path, &buf) {
        eprintln!("failed to write {path}: {e}");
    }
}

fn bot_status(g: &Game, frames: u64) {
    let alive = g.enemies[..g.level_enemy_count.max(0) as usize]
        .iter()
        .filter(|e| e.alive)
        .count() as i32;
    println!(
        "[bot] t={:5.1}s  level={}  hp={:3}  ammo={:2}  score={:6}  enemies={}/{}  pos=({:.2},{:.2}) ang={:.2}",
        frames as f64 / 60.0,
        g.level + 1,
        g.player.health,
        g.player.ammo,
        g.score,
        g.level_enemy_count - alive,
        g.level_enemy_count,
        g.player.x,
        g.player.y,
        g.player.angle
    );
}

/// Headless loop: fixed 60 Hz timestep, no window, no frame pacing — a bounded
/// `--frames` run covers a predictable span of game time and finishes fast.
fn run_headless(g: &mut Game, bot: bool, max_frames: i64) -> u64 {
    let mut frames = 0u64;
    while g.running {
        let dt = 1.0 / 60.0;
        if bot {
            g.bot_think(dt);
        }
        g.update_game(dt);
        g.render_frame();
        g.audio.tick(dt);

        if bot && frames % 60 == 0 {
            bot_status(g, frames);
        }
        frames += 1;
        if max_frames > 0 && frames >= max_frames as u64 {
            g.running = false;
        }
    }
    frames
}

/// Windowed loop via minifb (handles the 2x upscale + present).
fn run_windowed(g: &mut Game, bot: bool, max_frames: i64) -> u64 {
    let mut window = Window::new(
        "Doom Clone",
        SCREEN_W,
        SCREEN_H,
        WindowOptions {
            scale: Scale::X2,
            ..WindowOptions::default()
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("Failed to open window ({e}). Try --headless.");
        std::process::exit(1);
    });
    window.set_target_fps(60);

    let mut prev = Instant::now();
    let mut frames = 0u64;
    let mut prev_down = [false; K_COUNT];

    while g.running && window.is_open() {
        // Read held keys and derive rising edges (OR-in edges; update_game
        // clears the ones it consumes, matching the C event model).
        for &(action, phys) in KEYMAP {
            let down = phys.iter().any(|&k| window.is_key_down(k));
            if down && !prev_down[action] {
                g.key_edge[action] = true;
            }
            g.keys[action] = down;
            prev_down[action] = down;
        }

        let now = Instant::now();
        let dt = (now - prev).as_secs_f64().min(0.05);
        prev = now;

        if bot {
            g.bot_think(dt);
        }
        g.update_game(dt);
        g.render_frame();
        g.audio.tick(dt);

        window
            .update_with_buffer(&g.pixels, SCREEN_W, SCREEN_H)
            .unwrap();

        frames += 1;
        if max_frames > 0 && frames >= max_frames as u64 {
            g.running = false;
        }
    }
    frames
}
