//! The central `Game` struct: map queries, movement/collision, RNG, resets and
//! the high-score table.

use super::*;
use crate::constants::*;
use crate::game::Game;

// ---- map queries ----

#[test]
fn every_wall_glyph_maps_to_its_material() {
    let mut g = open_room();
    for (glyph, kind) in [
        (b'#', WALL_STONE),
        (b'=', WALL_BRICK),
        (b'B', WALL_METAL),
        (b'D', WALL_WOOD),
        (b'H', WALL_HELL),
        (b'T', WALL_TECH),
    ] {
        g.cur_map[5][5] = glyph;
        assert_eq!(g.map_wall_type(5, 5), kind, "glyph {}", glyph as char);
        assert!(g.map_blocked(5, 5));
    }
    // Floor and lava are both walk-through.
    for glyph in [b'.', b'~'] {
        g.cur_map[5][5] = glyph;
        assert_eq!(g.map_wall_type(5, 5), WALL_NONE);
        assert!(!g.map_blocked(5, 5));
    }
}

#[test]
fn outside_the_map_is_solid_stone() {
    let g = open_room();
    for (x, y) in [(-1, 5), (5, -1), (MAP_W as i32, 5), (5, MAP_H as i32)] {
        assert_eq!(g.map_wall_type(x, y), WALL_STONE);
        assert!(g.map_blocked(x, y));
    }
}

#[test]
fn only_lava_tiles_are_hazardous_and_never_out_of_bounds() {
    let mut g = open_room();
    g.cur_map[5][5] = b'~';
    assert!(g.map_hazard(5, 5));
    assert!(!g.map_hazard(6, 5));
    for (x, y) in [(-1, 5), (5, -1), (MAP_W as i32, 5), (5, MAP_H as i32)] {
        assert!(!g.map_hazard(x, y), "out of bounds is wall, not lava");
    }
}

// ---- movement ----

#[test]
fn try_move_reports_which_axes_got_through() {
    let mut g = open_room();
    g.player.x = 5.0;
    g.player.y = 5.0;
    assert_eq!(g.try_move(5.5, 5.5), 3, "open floor: both axes move");
    assert_eq!(g.player.x, 5.5);
    assert_eq!(g.player.y, 5.5);

    // Wall to the west only: X blocked, Y free.
    g.player.x = 1.5;
    g.player.y = 5.0;
    let moved = g.try_move(1.0, 5.5);
    assert_eq!(moved & 1, 0, "X into the wall is refused");
    assert_eq!(moved & 2, 2, "Y still slides");
}

#[test]
fn a_body_in_the_pad_band_can_still_escape_the_wall() {
    // Regression: checking both padded edges vetoed the very move that would
    // pull the player out of a wall they were already overlapping, so they
    // stuck to it forever (every step rejected, velocity zeroed, never a step
    // big enough to clear it).
    let mut g = open_room();
    g.cur_map[4][5] = b'#'; // wall directly north of (5, 5)
    g.player.x = 5.5;
    g.player.y = 5.11; // within pad (0.18) of the wall at y < 5.0

    let moved = g.try_move(5.5, 5.13); // a tiny step *away* from the wall
    assert_eq!(moved & 2, 2, "moving away from the wall must be allowed");
    assert!(g.player.y > 5.11);

    // ...but stepping further into it is still refused.
    g.player.y = 5.11;
    let moved = g.try_move(5.5, 5.10);
    assert_eq!(moved & 2, 0, "moving deeper into the wall is still blocked");
}

#[test]
fn the_player_cannot_walk_through_a_wall() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 2.5;
    for _ in 0..100 {
        let nx = g.player.x - 0.1;
        g.try_move(nx, g.player.y);
    }
    assert!(g.player.x > 1.0, "the west wall holds");
}

// ---- RNG ----

#[test]
fn the_rng_is_deterministic_and_in_range() {
    let mut a = open_room();
    let mut b = open_room();
    for _ in 0..50 {
        assert_eq!(a.rand_u32(), b.rand_u32(), "same seed, same stream");
    }
    let mut g = open_room();
    let mut distinct = std::collections::HashSet::new();
    for _ in 0..500 {
        let v = g.rand_f64();
        assert!((0.0..1.0).contains(&v), "rand_f64 out of range: {v}");
        distinct.insert((v * 1000.0) as u32);
    }
    assert!(distinct.len() > 100, "the stream should actually vary");
}

// ---- resets ----

#[test]
fn reset_transients_clears_the_world_but_not_the_player() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 5.0, 5.0, EN_GRUNT);
    put_barrel(&mut g, 0, 6.0, 6.0);
    put_pickup(&mut g, 0, 7.0, 7.0, PU_AMMO);
    g.spawn_particle(1.0, 1.0, 0.0, 0.0, 1.0, 0xFFFFFF);
    g.spawn_fireball(1.0, 1.0, 5.0, 5.0, 0.0, 3.0, 12);
    g.player.health = 42;
    g.hazard_burn = 0.5;

    g.reset_transients();

    assert_eq!(live_enemies(&g), 0);
    assert_eq!(live_fireballs(&g), 0);
    assert_eq!(live_particles(&g), 0);
    assert!(!g.barrels[0].alive);
    assert!(!g.pickups[0].alive);
    assert_eq!(g.hazard_burn, 0.0);
    assert_eq!(g.player.health, 42, "the player survives a transient reset");
}

#[test]
fn reset_game_puts_the_player_back_at_the_start() {
    let mut g = super::new_game();
    g.player.health = 3;
    g.player.ammo = 0;
    g.player.weapons = [true, true, true];
    g.player.weapon = WP_RIFLE;
    g.score = 9999;
    g.score_saved = true;
    g.final_rank = 2;
    g.load_level(4);

    g.reset_game();

    assert_eq!(g.player.health, 100);
    assert_eq!(g.player.ammo, 50);
    assert_eq!(g.player.weapon, WP_PISTOL);
    assert_eq!(g.player.weapons, [true, false, false], "guns are lost on restart");
    assert_eq!(g.score, 0);
    assert!(!g.score_saved);
    assert_eq!(g.final_rank, 0);
    assert_eq!(g.level, 0);
}

#[test]
fn all_enemies_dead_tracks_the_last_kill() {
    let mut g = open_room();
    assert!(g.all_enemies_dead(), "an empty room is already clear");
    put_enemy(&mut g, 0, 5.0, 5.0, EN_GRUNT);
    put_enemy(&mut g, 1, 6.0, 6.0, EN_IMP);
    assert!(!g.all_enemies_dead());
    g.enemies[0].alive = false;
    assert!(!g.all_enemies_dead(), "one left is not clear");
    g.enemies[1].alive = false;
    assert!(g.all_enemies_dead());
}

#[test]
fn a_default_game_matches_a_new_one() {
    let a = Game::default();
    let b = Game::new();
    assert_eq!(a.player.health, b.player.health);
    assert_eq!(a.running, b.running);
    assert_eq!(a.show_intro, b.show_intro);
}

// ---- high scores ----

/// A game whose score table lives in a scratch file, so tests never touch the
/// table in the working directory.
fn game_with_scratch_scores(name: &str) -> (Game, std::path::PathBuf) {
    let path = scratch_path(name);
    let _ = std::fs::remove_file(&path);
    let mut g = Game::new();
    g.score_file = path.to_string_lossy().into_owned();
    (g, path)
}

#[test]
fn a_missing_score_file_leaves_an_empty_table() {
    let (mut g, path) = game_with_scratch_scores("missing");
    g.high_scores = [5; MAX_HIGHSCORES];
    g.load_high_scores();
    assert_eq!(g.high_scores, [0; MAX_HIGHSCORES]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn scores_survive_a_save_and_load_round_trip() {
    let (mut g, path) = game_with_scratch_scores("roundtrip");
    g.high_scores = [500, 400, 300, 200, 100];
    g.save_high_scores();

    // A second game pointed at the same file — built directly, since the
    // helper clears the file it's given.
    let mut g2 = Game::new();
    g2.score_file = path.to_string_lossy().into_owned();
    g2.load_high_scores();
    assert_eq!(g2.high_scores, [500, 400, 300, 200, 100]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_malformed_score_file_is_read_up_to_the_bad_line() {
    let (mut g, path) = game_with_scratch_scores("malformed");
    std::fs::write(&path, "900\n800\nnot-a-number\n600\n").unwrap();
    g.load_high_scores();
    assert_eq!(g.high_scores[0], 900);
    assert_eq!(g.high_scores[1], 800);
    assert_eq!(g.high_scores[2], 0, "parsing stops at the junk line");
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_score_file_with_extra_lines_is_truncated_to_the_table() {
    let (mut g, path) = game_with_scratch_scores("toolong");
    let body: String = (0..MAX_HIGHSCORES + 5).map(|i| format!("{}\n", 100 - i)).collect();
    std::fs::write(&path, body).unwrap();
    g.load_high_scores();
    assert_eq!(g.high_scores[0], 100);
    assert_eq!(g.high_scores[MAX_HIGHSCORES - 1], 100 - (MAX_HIGHSCORES as i32 - 1));
    let _ = std::fs::remove_file(path);
}

#[test]
fn submitting_a_score_inserts_it_at_the_right_rank() {
    let (mut g, path) = game_with_scratch_scores("submit");
    g.high_scores = [500, 400, 300, 200, 100];

    assert_eq!(g.submit_score(450), 2, "450 slots in second");
    assert_eq!(g.high_scores, [500, 450, 400, 300, 200]);

    assert_eq!(g.submit_score(9999), 1, "a new best takes the top");
    assert_eq!(g.high_scores[0], 9999);

    assert_eq!(g.submit_score(1), 0, "too small to make the table");
    let _ = std::fs::remove_file(path);
}

#[test]
fn submitting_writes_the_table_out() {
    let (mut g, path) = game_with_scratch_scores("submit-writes");
    g.high_scores = [0; MAX_HIGHSCORES];
    g.submit_score(777);
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.starts_with("777"), "the new score should be on disk: {body:?}");
    let _ = std::fs::remove_file(path);
}
