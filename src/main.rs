//! DOOM-style raycasting FPS written in Rust.
//!
//! Controls: W/Up move, S/Down back, A/D strafe, Left/Right turn, Space shoot,
//! R restart (after death), Esc quit.
//!
//! Flags: `--headless` (no window, fixed timestep), `--frames N` (stop after N),
//! `--bot` (AI plays), `--selftest` (validate levels and exit 0/1),
//! `--shot PATH` (dump one frame as a PPM) with `--shot-level N`,
//! `--shot-pos X,Y` and `--shot-angle DEG` to aim the camera.
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
#[cfg(test)]
mod tests;
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
    (K_WEAPON1, &[Key::Key1]),
    (K_WEAPON2, &[Key::Key2]),
    (K_WEAPON3, &[Key::Key3]),
];

/// Everything the command line can set. Parsed out of `main` so the flag
/// handling can be tested without spawning a process.
#[derive(Debug, PartialEq)]
struct Args {
    headless: bool,
    selftest: bool,
    bot: bool,
    /// Stop after this many frames; negative means run until quit.
    max_frames: i64,
    shot: Option<String>,
    shot_level: usize,
    shot_pos: Option<(f64, f64)>,
    shot_angle: Option<f64>,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            headless: false,
            selftest: false,
            bot: false,
            max_frames: -1,
            shot: None,
            shot_level: 0,
            shot_pos: None,
            shot_angle: None,
        }
    }
}

/// Parse `argv` (including argv[0]). Unknown flags are ignored, and a flag
/// whose value is missing or unparseable keeps the default rather than failing
/// the run — this is a game, not a build tool.
fn parse_args(argv: &[String]) -> Args {
    let mut a = Args::default();
    let mut i = 1;
    while i < argv.len() {
        // Value-taking flags consume the next argument when there is one.
        let value = |i: &mut usize| -> Option<&str> {
            if *i + 1 < argv.len() {
                *i += 1;
                Some(argv[*i].as_str())
            } else {
                None
            }
        };
        match argv[i].as_str() {
            "--headless" => a.headless = true,
            "--selftest" => a.selftest = true,
            "--bot" => a.bot = true,
            "--frames" => {
                if let Some(v) = value(&mut i) {
                    a.max_frames = v.parse().unwrap_or(-1);
                }
            }
            "--shot" => {
                if let Some(v) = value(&mut i) {
                    a.shot = Some(v.to_string());
                }
            }
            "--shot-level" => {
                if let Some(v) = value(&mut i) {
                    a.shot_level = v.parse().unwrap_or(0);
                }
            }
            "--shot-pos" => {
                if let Some(v) = value(&mut i) {
                    let mut it = v.split(',').map(|s| s.trim().parse::<f64>());
                    if let (Some(Ok(x)), Some(Ok(y))) = (it.next(), it.next()) {
                        a.shot_pos = Some((x, y));
                    }
                }
            }
            "--shot-angle" => {
                if let Some(v) = value(&mut i) {
                    a.shot_angle = v.parse::<f64>().ok().map(|d| d.to_radians());
                }
            }
            _ => {}
        }
        i += 1;
    }
    a
}

/// Build the one-frame game state a `--shot` run renders.
fn shot_game(args: &Args) -> Game {
    let mut g = Game::new();
    g.load_high_scores();
    g.reset_game();
    if args.shot_level > 0 {
        g.load_level(args.shot_level.min(LEVEL_COUNT - 1));
    }
    // Camera overrides let a shot frame anything in the level, not just
    // whatever the spawn point happens to face.
    if let Some((x, y)) = args.shot_pos {
        g.player.x = x;
        g.player.y = y;
    }
    if let Some(a) = args.shot_angle {
        g.player.angle = a;
    }
    g.show_intro = false;
    g
}

/// Do whatever the arguments ask for and return the process exit code. Split
/// from `main` so every mode except the windowed loop (which needs a real
/// display) can be driven from a test.
fn run(args: &Args) -> i32 {
    if args.selftest {
        return selftest::run_self_test();
    }

    // Render a single frame of one level to a PPM and exit (renderer check).
    if let Some(path) = &args.shot {
        let mut g = shot_game(args);
        g.render_frame();
        write_ppm(path, &g.pixels);
        return 0;
    }

    let mut g = Game::new();
    g.load_high_scores();
    // Audio only makes sense for the interactive window; headless/bot test runs
    // skip it (no point spawning a PCM player, and it keeps them deterministic).
    if !args.headless {
        g.audio.init();
    }
    g.reset_game();

    let frames = if args.headless {
        run_headless(&mut g, args.bot, args.max_frames)
    } else {
        run_windowed(&mut g, args.bot, args.max_frames)
    };

    if args.bot {
        println!("[bot] done: {} frames, level {}, final score {}", frames, g.level + 1, g.score);
    }
    g.audio.shutdown();
    0
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    std::process::exit(run(&parse_args(&argv)));
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
        "[bot] t={:5.1}s  level={}  hp={:3}  ammo={:2}  score={:6}  enemies={}/{}",
        frames as f64 / 60.0,
        g.level + 1,
        g.player.health,
        g.player.ammo,
        g.score,
        g.level_enemy_count - alive,
        g.level_enemy_count
    );
    // `DOOM_DEBUG` adds a per-enemy dump — the quickest way to see *which*
    // enemy a stalled bot is failing to reach.
    if std::env::var("DOOM_DEBUG").is_ok() {
        for (i, e) in g.enemies.iter().enumerate() {
            if e.alive {
                println!(
                    "      enemy {i} kind={} at ({:.2},{:.2}) hp={} | player ({:.2},{:.2})",
                    e.kind, e.x, e.y, e.hp, g.player.x, g.player.y
                );
            }
        }
    }
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

/// Fold this frame's held-key state into the game: `keys` is the held state,
/// and `key_edge` gets a rising edge OR'd in (update_game clears the ones it
/// consumes, matching the C event model). `prev_down` carries last frame's
/// state across the call. Separate from the window loop so the edge detection
/// can be tested without a display.
fn apply_input(g: &mut Game, down: &[bool; K_COUNT], prev_down: &mut [bool; K_COUNT]) {
    for action in 0..K_COUNT {
        if down[action] && !prev_down[action] {
            g.key_edge[action] = true;
        }
        g.keys[action] = down[action];
        prev_down[action] = down[action];
    }
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
        let mut down = [false; K_COUNT];
        for &(action, phys) in KEYMAP {
            down[action] = phys.iter().any(|&k| window.is_key_down(k));
        }
        apply_input(g, &down, &mut prev_down);

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
