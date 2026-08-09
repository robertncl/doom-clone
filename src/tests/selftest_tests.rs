//! The level validator itself.
//!
//! This is the guard every level edit is checked against, so it needs its own
//! tests: a validator that silently passes bad data is worse than none. Each
//! case feeds it a level table broken in exactly one way.

use crate::constants::*;
use crate::level::LEVELS;
use crate::selftest::{run_self_test, validate_levels};

/// A minimal valid level: solid border, one spawn, one grunt, one of each
/// pickup, reachable.
const GOOD: [&str; MAP_H] = [
    "################",
    "#p.............#",
    "#..............#",
    "#....g.........#",
    "#..............#",
    "#....h..a......#",
    "#..............#",
    "#....s..r......#",
    "#..............#",
    "#..............#",
    "#..............#",
    "#..............#",
    "#..............#",
    "#..............#",
    "#..............#",
    "################",
];

/// `GOOD` with one row swapped for `row`.
fn with_row(y: usize, row: &'static str) -> [&'static str; MAP_H] {
    let mut m = GOOD;
    m[y] = row;
    m
}

fn failures(map: [&'static str; MAP_H]) -> Vec<String> {
    validate_levels(&[map]).0
}

#[test]
fn a_good_level_passes_cleanly() {
    let f = validate_levels(&[GOOD]);
    assert!(f.ok(), "the reference level should pass: {:?}", f.0);
    assert!(f.0.is_empty());
}

#[test]
fn the_shipped_levels_pass_and_the_runner_reports_success() {
    assert!(validate_levels(&LEVELS).ok());
    assert_eq!(run_self_test(), 0);
}

#[test]
fn a_short_row_is_caught() {
    let f = failures(with_row(5, "#..#"));
    assert!(f.iter().any(|m| m.contains("row 5 length")), "{f:?}");
}

#[test]
fn an_unknown_glyph_is_caught() {
    let f = failures(with_row(5, "#....X.........#"));
    assert!(f.iter().any(|m| m.contains("char 'X'")), "{f:?}");
}

#[test]
fn a_missing_or_duplicated_spawn_is_caught() {
    let none = failures(with_row(1, "#..............#"));
    assert!(none.iter().any(|m| m.contains("exactly one player spawn")), "{none:?}");

    let two = failures(with_row(5, "#....p.........#"));
    assert!(two.iter().any(|m| m.contains("exactly one player spawn")), "{two:?}");
}

#[test]
fn a_level_with_nothing_to_kill_is_caught() {
    let f = failures(with_row(3, "#..............#"));
    assert!(f.iter().any(|m| m.contains("at least one enemy")), "{f:?}");
}

// Note: there is deliberately no test for "spawn inside a wall" or "spawn in
// lava". The loader rewrites the `p` cell to plain floor, so neither can be
// expressed in a level table — those two checks are belt-and-braces against a
// future loader change, and can only ever pass from this entry point.

#[test]
fn an_unreachable_enemy_is_caught() {
    // Seal the grunt into a one-tile box.
    let mut m = GOOD;
    m[2] = "#...###........#";
    m[3] = "#...#g#........#";
    m[4] = "#...###........#";
    let f = validate_levels(&[m]).0;
    assert!(f.iter().any(|s| s.contains("enemy 0 is reachable")), "{f:?}");
}

#[test]
fn an_unreachable_pickup_is_caught() {
    let mut m = GOOD;
    m[4] = "#...###........#";
    m[5] = "#...#h#.a......#";
    m[6] = "#...###........#";
    let f = validate_levels(&[m]).0;
    assert!(f.iter().any(|s| s.contains("pickup") && s.contains("reachable")), "{f:?}");
}

#[test]
fn an_unreachable_barrel_is_caught() {
    let mut m = GOOD;
    m[9] = "#...###........#";
    m[10] = "#...#o#........#";
    m[11] = "#...###........#";
    let f = validate_levels(&[m]).0;
    assert!(f.iter().any(|s| s.contains("barrel") && s.contains("reachable")), "{f:?}");
}

#[test]
fn too_many_enemies_for_the_pool_is_caught() {
    let mut m = GOOD;
    // Fill several rows with grunts, well past MAX_ENEMIES.
    for y in 2..7 {
        m[y] = "#gggggggggggggg#";
    }
    let f = validate_levels(&[m]).0;
    assert!(f.iter().any(|s| s.contains("fits MAX_ENEMIES")), "{f:?}");
    assert!(f.iter().any(|s| s.contains("spawned every enemy")), "over-capacity spawns are dropped");
}

#[test]
fn too_many_pickups_for_the_pool_is_caught() {
    let mut m = GOOD;
    for y in 9..12 {
        m[y] = "#aaaaaaaaaaaaaa#";
    }
    let f = validate_levels(&[m]).0;
    assert!(f.iter().any(|s| s.contains("fits MAX_PICKUPS")), "{f:?}");
}

#[test]
fn too_many_barrels_for_the_pool_is_caught() {
    let mut m = GOOD;
    for y in 9..12 {
        m[y] = "#oooooooooooooo#";
    }
    let f = validate_levels(&[m]).0;
    assert!(f.iter().any(|s| s.contains("fits MAX_BARRELS")), "{f:?}");
}

#[test]
fn several_broken_levels_are_all_reported() {
    let bad_a = with_row(5, "#....X.........#");
    let bad_b = with_row(1, "#..............#");
    let f = validate_levels(&[GOOD, bad_a, bad_b]).0;
    assert!(f.iter().any(|m| m.contains("level 1")), "{f:?}");
    assert!(f.iter().any(|m| m.contains("level 2")), "{f:?}");
    assert!(!f.iter().any(|m| m.contains("level 0")), "the good level should be quiet: {f:?}");
}
