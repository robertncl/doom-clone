//! Command-line handling, the PPM dump, the bot status line and the headless
//! loop. (`run_windowed` and `main` itself need a display and a process, so
//! they're the one part of the binary left to manual smoke testing.)

use super::*;
use crate::constants::*;
use crate::{apply_input, bot_status, parse_args, run, run_headless, shot_game, write_ppm, Args};

fn argv(items: &[&str]) -> Vec<String> {
    std::iter::once("doom".to_string()).chain(items.iter().map(|s| s.to_string())).collect()
}

#[test]
fn no_flags_gives_the_default_windowed_run() {
    let a = parse_args(&argv(&[]));
    assert_eq!(a, Args::default());
    assert!(!a.headless && !a.bot && !a.selftest);
    assert_eq!(a.max_frames, -1, "runs until quit by default");
}

#[test]
fn the_boolean_flags_are_picked_up_in_any_order() {
    let a = parse_args(&argv(&["--bot", "--headless", "--selftest"]));
    assert!(a.headless && a.bot && a.selftest);
    let b = parse_args(&argv(&["--selftest", "--headless", "--bot"]));
    assert_eq!(a, b);
}

#[test]
fn frames_takes_a_count() {
    assert_eq!(parse_args(&argv(&["--frames", "120"])).max_frames, 120);
    // Junk falls back to the default rather than aborting the run.
    assert_eq!(parse_args(&argv(&["--frames", "banana"])).max_frames, -1);
    // A missing value is ignored.
    assert_eq!(parse_args(&argv(&["--frames"])).max_frames, -1);
}

#[test]
fn shot_takes_a_path_and_optional_camera() {
    let a = parse_args(&argv(&["--shot", "/tmp/x.ppm", "--shot-level", "3"]));
    assert_eq!(a.shot.as_deref(), Some("/tmp/x.ppm"));
    assert_eq!(a.shot_level, 3);
    assert_eq!(a.shot_pos, None);
    assert_eq!(a.shot_angle, None);

    let a = parse_args(&argv(&["--shot-pos", "4.5,6.25", "--shot-angle", "180"]));
    assert_eq!(a.shot_pos, Some((4.5, 6.25)));
    let angle = a.shot_angle.unwrap();
    assert!((angle - std::f64::consts::PI).abs() < 1e-9, "degrees convert to radians");

    // Whitespace around the pair is tolerated.
    assert_eq!(parse_args(&argv(&["--shot-pos", " 1.0 , 2.0 "])).shot_pos, Some((1.0, 2.0)));
}

#[test]
fn a_malformed_camera_argument_is_ignored() {
    for bad in ["nope", "1.0", "1.0,", ",2.0", "a,b"] {
        assert_eq!(parse_args(&argv(&["--shot-pos", bad])).shot_pos, None, "input {bad:?}");
    }
    assert_eq!(parse_args(&argv(&["--shot-angle", "sideways"])).shot_angle, None);
    assert_eq!(parse_args(&argv(&["--shot-level", "many"])).shot_level, 0);
}

#[test]
fn value_flags_with_no_value_are_ignored() {
    let a = parse_args(&argv(&["--shot"]));
    assert_eq!(a.shot, None);
    let a = parse_args(&argv(&["--shot-level"]));
    assert_eq!(a.shot_level, 0);
    let a = parse_args(&argv(&["--shot-pos"]));
    assert_eq!(a.shot_pos, None);
    let a = parse_args(&argv(&["--shot-angle"]));
    assert_eq!(a.shot_angle, None);
}

#[test]
fn unknown_flags_are_skipped() {
    let a = parse_args(&argv(&["--wat", "-x", "positional", "--bot"]));
    assert!(a.bot, "a real flag after junk is still honoured");
    assert!(!a.headless);
}

#[test]
fn a_shot_game_starts_on_the_requested_level_and_camera() {
    let args = parse_args(&argv(&["--shot", "x.ppm", "--shot-level", "2"]));
    let g = shot_game(&args);
    assert_eq!(g.level, 2);
    assert!(!g.show_intro, "the shot shows the scene, not the intro");

    let args = parse_args(&argv(&["--shot", "x.ppm", "--shot-pos", "5.5,6.5", "--shot-angle", "90"]));
    let g = shot_game(&args);
    assert_eq!((g.player.x, g.player.y), (5.5, 6.5));
    assert!((g.player.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
}

#[test]
fn a_shot_level_beyond_the_last_one_is_clamped() {
    let args = parse_args(&argv(&["--shot", "x.ppm", "--shot-level", "999"]));
    let g = shot_game(&args);
    assert_eq!(g.level as usize, LEVEL_COUNT - 1);
}

#[test]
fn write_ppm_emits_a_readable_p6_image() {
    let path = scratch_path("shot.ppm");
    let _ = std::fs::remove_file(&path);

    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.render_frame();
    write_ppm(path.to_str().unwrap(), &g.pixels);

    let bytes = std::fs::read(&path).unwrap();
    let header = format!("P6\n{} {}\n255\n", SCREEN_W, SCREEN_H);
    assert!(bytes.starts_with(header.as_bytes()), "wrong PPM header");
    assert_eq!(
        bytes.len(),
        header.len() + SCREEN_W * SCREEN_H * 3,
        "one RGB triple per pixel"
    );

    // The first pixel's bytes should match the framebuffer's top-left texel.
    let px = g.pixels[0];
    let off = header.len();
    assert_eq!(bytes[off], ((px >> 16) & 0xFF) as u8);
    assert_eq!(bytes[off + 1], ((px >> 8) & 0xFF) as u8);
    assert_eq!(bytes[off + 2], (px & 0xFF) as u8);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn write_ppm_to_an_unwritable_path_fails_quietly() {
    let g = super::new_game();
    // A directory that doesn't exist: the write errors, and the game carries on.
    write_ppm("/nonexistent-dir-for-doom-tests/shot.ppm", &g.pixels);
}

#[test]
fn the_bot_status_line_reports_without_panicking() {
    let mut g = super::new_game();
    g.reset_game();
    bot_status(&g, 0);
    bot_status(&g, 600);

    // Degenerate counts (an unloaded level) must not index out of bounds.
    g.level_enemy_count = 0;
    bot_status(&g, 1);
    g.level_enemy_count = -5;
    bot_status(&g, 2);
}

#[test]
fn the_headless_loop_stops_at_the_frame_cap() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    let frames = run_headless(&mut g, false, 30);
    assert_eq!(frames, 30);
    assert!(!g.running, "hitting the cap ends the loop");
    assert!(g.global_time > 0.0, "and time advanced while it ran");
}

#[test]
fn the_headless_loop_drives_the_bot_and_prints_status() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    // Past 60 frames so the once-a-second status line is exercised too.
    let frames = run_headless(&mut g, true, 90);
    assert_eq!(frames, 90);
    assert!(g.score >= 0);
}

#[test]
fn the_headless_loop_also_stops_when_the_game_quits() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.running = false;
    assert_eq!(run_headless(&mut g, false, -1), 0, "an already-stopped game runs no frames");
}

// ---- input edge detection ----

#[test]
fn a_key_press_produces_one_edge_and_a_held_state() {
    let mut g = super::new_game();
    let mut prev = [false; K_COUNT];
    let mut down = [false; K_COUNT];

    down[K_SHOOT] = true;
    apply_input(&mut g, &down, &mut prev);
    assert!(g.keys[K_SHOOT], "the key reads as held");
    assert!(g.key_edge[K_SHOOT], "and the press is an edge");

    // Held down across frames: still held, but no new edge once consumed.
    g.key_edge[K_SHOOT] = false;
    apply_input(&mut g, &down, &mut prev);
    assert!(g.keys[K_SHOOT]);
    assert!(!g.key_edge[K_SHOOT], "holding a key is not a fresh press");

    // Release, then press again: a new edge.
    down[K_SHOOT] = false;
    apply_input(&mut g, &down, &mut prev);
    assert!(!g.keys[K_SHOOT]);
    down[K_SHOOT] = true;
    apply_input(&mut g, &down, &mut prev);
    assert!(g.key_edge[K_SHOOT], "re-pressing should register");
}

#[test]
fn an_unconsumed_edge_is_not_cleared_by_input() {
    let mut g = super::new_game();
    let mut prev = [false; K_COUNT];
    let down = [false; K_COUNT];
    g.key_edge[K_QUIT] = true;
    apply_input(&mut g, &down, &mut prev);
    assert!(g.key_edge[K_QUIT], "edges are cleared by the consumer, not the reader");
}

#[test]
fn every_action_can_be_pressed() {
    let mut g = super::new_game();
    let mut prev = [false; K_COUNT];
    let down = [true; K_COUNT];
    apply_input(&mut g, &down, &mut prev);
    assert!(g.keys.iter().all(|&k| k));
    assert!(g.key_edge.iter().all(|&e| e));
}

// ---- the top-level dispatch ----

#[test]
fn selftest_mode_validates_the_levels_and_reports_success() {
    assert_eq!(run(&parse_args(&argv(&["--selftest"]))), 0);
}

#[test]
fn shot_mode_writes_the_image_and_exits() {
    let path = scratch_path("run-shot.ppm");
    let _ = std::fs::remove_file(&path);

    let args = parse_args(&argv(&[
        "--shot",
        path.to_str().unwrap(),
        "--shot-level",
        "6",
        "--shot-pos",
        "2.5,2.5",
        "--shot-angle",
        "45",
    ]));
    assert_eq!(run(&args), 0);

    let meta = std::fs::metadata(&path).expect("the shot should have been written");
    assert_eq!(meta.len() as usize, SCREEN_W * SCREEN_H * 3 + format!("P6\n{} {}\n255\n", SCREEN_W, SCREEN_H).len());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn headless_mode_runs_the_requested_frames() {
    // Short enough that the player can't die, so nothing writes a score file.
    let args = parse_args(&argv(&["--headless", "--frames", "30"]));
    assert_eq!(run(&args), 0);
}
