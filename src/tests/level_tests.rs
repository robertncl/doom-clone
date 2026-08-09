//! Level data and the loader, plus the self-test that guards the level set.

use super::*;
use crate::constants::*;
use crate::level::LEVELS;
use crate::selftest::run_self_test;

#[test]
fn the_bundled_levels_all_pass_the_self_test() {
    assert_eq!(run_self_test(), 0, "shipped levels must satisfy every invariant");
}

#[test]
fn every_level_loads_with_a_spawn_and_something_to_fight() {
    let mut g = super::new_game();
    for n in 0..LEVEL_COUNT {
        g.load_level(n);
        assert_eq!(g.level, n as i32);
        assert!(g.level_enemy_count > 0, "level {n} has nothing to kill");
        assert!(!g.map_blocked(g.player.x as i32, g.player.y as i32), "level {n} spawn is solid");
        assert_eq!(g.level_clear_timer, 0.0);
        assert!(!g.level_bonus_given);
        assert_eq!(g.player.angle, 0.0, "every level starts facing east");
    }
}

#[test]
fn the_loader_turns_glyphs_into_entities_and_leaves_plain_floor() {
    let mut g = super::new_game();
    g.load_level(0);
    // Spawn markers are consumed: nothing but walls, floor and lava remains.
    for row in g.cur_map.iter() {
        for &c in row.iter() {
            assert!(
                matches!(c, b'.' | b'~' | b'#' | b'=' | b'B' | b'D' | b'H' | b'T'),
                "unconsumed map glyph '{}'",
                c as char
            );
        }
    }
}

#[test]
fn each_enemy_glyph_spawns_its_kind_with_the_right_health() {
    // Levels 8 and 9 between them carry all four kinds.
    let mut g = super::new_game();
    let mut seen = [false; EN_KIND_MAX];
    for n in 0..LEVEL_COUNT {
        g.load_level(n);
        for e in g.enemies.iter().filter(|e| e.alive) {
            seen[e.kind as usize] = true;
            assert_eq!(e.hp, EN_HP[e.kind as usize], "kind {} spawned with wrong hp", e.kind);
        }
    }
    assert!(seen.iter().all(|&s| s), "every enemy kind should appear somewhere: {seen:?}");
}

#[test]
fn every_pickup_kind_appears_in_the_level_set() {
    let mut g = super::new_game();
    let mut seen = [false; 4];
    for n in 0..LEVEL_COUNT {
        g.load_level(n);
        for p in g.pickups.iter().filter(|p| p.alive) {
            seen[p.kind as usize] = true;
        }
    }
    assert!(seen.iter().all(|&s| s), "health/ammo/shotgun/rifle should all be placed: {seen:?}");
}

#[test]
fn the_hazard_flag_matches_the_map_contents() {
    let mut g = super::new_game();
    for n in 0..LEVEL_COUNT {
        g.load_level(n);
        let has_lava = LEVELS[n].iter().any(|row| row.contains('~'));
        assert_eq!(g.has_hazard, has_lava, "level {n} hazard flag disagrees with its map");
    }
}

#[test]
fn later_levels_are_the_ones_with_the_new_mechanics() {
    let mut g = super::new_game();
    // The first five levels stay approachable: no lava, no barrels, no heavies.
    for n in 0..5 {
        g.load_level(n);
        assert!(!g.has_hazard, "level {} should have no lava", n + 1);
        assert_eq!(g.barrels.iter().filter(|b| b.alive).count(), 0);
        assert!(
            !g.enemies.iter().any(|e| e.alive && (e.kind == EN_BARON || e.kind == EN_WRAITH)),
            "level {} should not have the late-game demons",
            n + 1
        );
    }
    // And the back half is where they show up.
    let mut lava = false;
    let mut barrels = 0;
    let mut barons = 0;
    for n in 5..LEVEL_COUNT {
        g.load_level(n);
        lava |= g.has_hazard;
        barrels += g.barrels.iter().filter(|b| b.alive).count();
        barons += g.enemies.iter().filter(|e| e.alive && e.kind == EN_BARON).count();
    }
    assert!(lava, "the later levels should introduce lava");
    assert!(barrels > 0, "and barrels");
    assert!(barons > 0, "and barons");
}

#[test]
fn loading_a_level_clears_whatever_the_last_one_left_behind() {
    let mut g = super::new_game();
    g.load_level(0);
    g.spawn_particle(2.0, 2.0, 0.0, 0.0, 5.0, 0xFFFFFF);
    g.spawn_fireball(2.0, 2.0, 5.0, 5.0, 0.0, 3.0, 12);
    g.load_level(1);
    assert_eq!(live_particles(&g), 0);
    assert_eq!(live_fireballs(&g), 0);
}

#[test]
fn no_level_overflows_the_entity_pools() {
    // The loader silently drops spawns past capacity, which would quietly
    // delete part of a level's design.
    for n in 0..LEVEL_COUNT {
        let (mut e, mut p, mut b) = (0, 0, 0);
        for row in LEVELS[n].iter() {
            for ch in row.chars() {
                match ch {
                    'g' | 'i' | 'w' | 'k' => e += 1,
                    'h' | 'a' | 's' | 'r' => p += 1,
                    'o' => b += 1,
                    _ => {}
                }
            }
        }
        assert!(e <= MAX_ENEMIES, "level {n} has {e} enemies");
        assert!(p <= MAX_PICKUPS, "level {n} has {p} pickups");
        assert!(b <= MAX_BARRELS, "level {n} has {b} barrels");
    }
}
