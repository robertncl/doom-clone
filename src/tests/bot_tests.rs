//! The AI player: line of sight, BFS fields, goal choice, range control,
//! dodging, stuck recovery and the attract-mode restart.

use super::*;
use crate::constants::*;

/// Does the bot hold any movement key this frame?
fn moving(g: &Game) -> bool {
    g.keys[K_FWD] || g.keys[K_BACK] || g.keys[K_STRAFEL] || g.keys[K_STRAFER]
}

// ---- line of sight ----

#[test]
fn line_of_sight_is_blocked_by_walls_and_clear_otherwise() {
    let mut g = open_room();
    assert!(g.bot_los(2.5, 2.5, 12.5, 2.5), "an open room has clear sight");
    for y in 0..MAP_H {
        g.cur_map[y][6] = b'#';
    }
    assert!(!g.bot_los(2.5, 2.5, 12.5, 2.5), "a wall should break it");
    assert!(g.bot_los(2.5, 2.5, 4.5, 2.5), "but not for a target on our side");
}

#[test]
fn lava_does_not_block_line_of_sight() {
    let mut g = open_room();
    g.cur_map[2][6] = b'~';
    g.has_hazard = true;
    assert!(g.bot_los(2.5, 2.5, 12.5, 2.5), "you can see across a lava pool");
}

// ---- BFS fields ----

#[test]
fn the_distance_field_grows_with_distance_from_its_root() {
    let g = open_room();
    let f = g.bot_field(2, 2);
    assert_eq!(f[2][2], 0, "the root is zero");
    assert_eq!(f[2][3], 1);
    assert_eq!(f[2][4], 2);
    assert!(f[10][10] > f[4][4], "further cells cost more");
}

#[test]
fn walls_are_left_unreachable_in_the_field() {
    let g = open_room();
    let f = g.bot_field(2, 2);
    assert_eq!(f[0][0], -1, "a wall cell is never entered");
}

#[test]
fn a_field_rooted_in_a_wall_or_off_the_map_is_empty() {
    let g = open_room();
    for (x, y) in [(0, 0), (-1, 5), (5, -1), (MAP_W as i32, 5), (5, MAP_H as i32)] {
        let f = g.bot_field(x, y);
        assert!(f.iter().all(|row| row.iter().all(|&v| v == -1)), "root ({x},{y}) should be empty");
    }
}

#[test]
fn a_sealed_off_room_is_unreachable() {
    let mut g = open_room();
    // Box off the north-east corner completely.
    for x in 10..MAP_W {
        g.cur_map[10][x] = b'#';
    }
    for y in 0..10 {
        g.cur_map[y][10] = b'#';
    }
    let f = g.bot_field(2, 2);
    assert_eq!(f[5][13], -1, "nothing inside the sealed room is reachable");
}

#[test]
fn the_hazard_avoiding_field_refuses_to_cross_lava() {
    let mut g = open_room();
    // A lava wall straight across the room, with no way round.
    for y in 0..MAP_H {
        g.cur_map[y][6] = b'~';
    }
    g.has_hazard = true;

    let safe = g.bot_field_ex(2, 2, true);
    assert_eq!(safe[2][10], -1, "the dry field stops at the lava");
    let any = g.bot_field_ex(2, 2, false);
    assert!(any[2][10] > 0, "the permissive field walks through it");
}

#[test]
fn a_dry_detour_is_preferred_over_a_lava_shortcut() {
    let mut g = open_room();
    // Lava straight ahead, but a clear way round the top.
    for y in 3..MAP_H {
        g.cur_map[y][6] = b'~';
    }
    g.has_hazard = true;
    let safe = g.bot_field_ex(2, 8, true);
    assert!(safe[8][10] > 0, "there is still a dry route round the pool");
}

// ---- goal selection and combat ----

#[test]
fn the_bot_dismisses_the_intro() {
    let mut g = open_room();
    g.show_intro = true;
    g.bot_think(1.0 / 60.0);
    assert!(g.key_edge[K_FWD], "it should press something to get past the intro");
}

#[test]
fn the_bot_restarts_after_a_pause_when_the_run_ends() {
    let mut g = open_room();
    g.score_saved = true;
    g.bot_think(1.0);
    assert!(!g.key_edge[K_RESTART], "it waits a beat first");
    for _ in 0..4 {
        g.bot_think(1.0);
    }
    assert!(g.key_edge[K_RESTART], "then it restarts for the attract loop");
}

#[test]
fn a_dead_bot_stops_pressing_keys() {
    let mut g = open_room();
    g.player.health = 0;
    g.keys[K_FWD] = true;
    g.bot_think(1.0 / 60.0);
    assert!(!moving(&g));
}

#[test]
fn with_nothing_to_do_the_bot_stands_still() {
    let mut g = open_room();
    g.bot_think(1.0 / 60.0);
    assert!(!moving(&g), "an empty level gives it no goal");
}

#[test]
fn the_bot_closes_on_a_distant_enemy() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 12.0, 8.0, EN_GRUNT);
    for _ in 0..60 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.x > 2.5, "it should have advanced on the target");
}

#[test]
fn the_bot_shoots_once_it_has_a_bead() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 6.0, 8.0, EN_GRUNT);
    let ammo = g.player.ammo;
    for _ in 0..120 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.ammo < ammo, "it should have opened fire");
}

#[test]
fn the_bot_backs_off_an_enemy_in_its_face() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 9.0, 8.0, EN_GRUNT);
    g.bot_think(1.0 / 60.0);
    assert!(g.keys[K_BACK] || g.keys[K_STRAFEL] || g.keys[K_STRAFER], "it should give ground");
}

#[test]
fn the_bot_goes_for_health_when_it_is_hurt() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    g.player.health = 20;
    put_pickup(&mut g, 0, 8.0, 8.0, PU_HEALTH);
    // An enemy exists but survival comes first.
    put_enemy(&mut g, 0, 2.5, 12.0, EN_GRUNT);
    for _ in 0..200 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        if !g.pickups[0].alive {
            break;
        }
    }
    assert!(!g.pickups[0].alive, "it should have collected the medkit");
}

#[test]
fn the_bot_goes_for_ammo_when_it_is_dry() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    g.player.ammo = 0;
    put_pickup(&mut g, 0, 8.0, 8.0, PU_AMMO);
    for _ in 0..200 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        if !g.pickups[0].alive {
            break;
        }
    }
    assert!(!g.pickups[0].alive);
}

#[test]
fn the_bot_detours_for_a_gun_it_does_not_own() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    put_pickup(&mut g, 0, 6.0, 8.0, PU_RIFLE);
    for _ in 0..200 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        if g.player.weapons[WP_RIFLE as usize] {
            break;
        }
    }
    assert!(g.player.weapons[WP_RIFLE as usize], "the rifle is worth picking up");
}

#[test]
fn the_bot_ignores_a_gun_it_already_carries() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    g.player.weapons[WP_RIFLE as usize] = true;
    put_pickup(&mut g, 0, 6.0, 8.0, PU_RIFLE);
    for _ in 0..60 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    assert!(g.pickups[0].alive, "a spare rifle is not worth the walk");
}

#[test]
fn a_hurt_bot_does_not_detour_for_a_gun() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    g.player.health = 45; // below the detour threshold, above the panic one
    put_pickup(&mut g, 0, 6.0, 8.0, PU_RIFLE);
    for _ in 0..60 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    assert!(g.pickups[0].alive, "shopping can wait when you're hurt");
}

#[test]
fn the_bot_mops_up_leftover_pickups_once_the_level_is_quiet() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    put_pickup(&mut g, 0, 7.0, 8.0, PU_AMMO);
    for _ in 0..200 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        if !g.pickups[0].alive {
            break;
        }
    }
    assert!(!g.pickups[0].alive);
}

#[test]
fn an_unreachable_enemy_is_ignored() {
    let mut g = open_room();
    // Seal a grunt inside its own box.
    for x in 9..13 {
        g.cur_map[6][x] = b'#';
        g.cur_map[10][x] = b'#';
    }
    for y in 6..11 {
        g.cur_map[y][9] = b'#';
        g.cur_map[y][12] = b'#';
    }
    put_enemy(&mut g, 0, 10.5, 8.5, EN_GRUNT);
    g.player.x = 2.5;
    g.player.y = 2.5;
    g.bot_think(1.0 / 60.0);
    assert!(!moving(&g), "there's no way to it, so no reason to walk");
}

#[test]
fn an_enemy_visible_but_unwalkable_is_still_engaged() {
    let mut g = open_room();
    // A lava pit the bot can see across but would rather not cross.
    for y in 6..10 {
        for x in 5..8 {
            g.cur_map[y][x] = b'~';
        }
    }
    g.has_hazard = true;
    g.player.x = 3.0;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 9.5, 8.0, EN_GRUNT);
    let ammo = g.player.ammo;
    for _ in 0..120 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.ammo < ammo, "it should shoot across the pit");
}

#[test]
fn the_bot_dodges_incoming_fire() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 13.0, 8.0, EN_IMP);
    // A fireball already on its way in.
    g.spawn_fireball(11.0, 8.0, 8.0, 8.0, 0.0, 3.0, 12);
    let y0 = g.player.y;
    for _ in 0..30 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    assert!((g.player.y - y0).abs() > 1e-3, "it should sidestep the shot");
}

#[test]
fn a_fireball_heading_away_is_not_dodged() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 13.5, 8.0, EN_GRUNT); // far enough that it wants to close in
    g.spawn_fireball(9.0, 8.0, 14.0, 8.0, 0.0, 3.0, 12); // flying away from us
    g.bot_think(1.0 / 60.0);
    assert!(moving(&g), "it should still be getting on with the fight");
}

#[test]
fn the_bot_will_not_loiter_in_lava() {
    let mut g = open_room();
    // Standing in fire, at the range where it would otherwise hold position.
    g.cur_map[8][8] = b'~';
    g.has_hazard = true;
    g.player.x = 8.5;
    g.player.y = 8.5;
    put_enemy(&mut g, 0, 11.5, 8.5, EN_GRUNT);
    g.bot_think(1.0 / 60.0);
    assert!(moving(&g), "burning is never worth standing still for");
}

#[test]
fn the_bot_crosses_lava_when_that_is_the_only_way_through() {
    let mut g = open_room();
    // A lava band spanning the room: the target is only reachable through it.
    for y in 0..MAP_H {
        g.cur_map[y][6] = b'~';
    }
    g.has_hazard = true;
    g.player.x = 2.5;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 12.0, 8.0, EN_GRUNT);
    for _ in 0..400 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        if g.player.x > 7.0 {
            break;
        }
    }
    assert!(g.player.x > 7.0, "it should take the burn rather than stall on the bank");
}

#[test]
fn the_bot_shakes_itself_loose_when_stuck() {
    let mut g = open_room();
    // Wedged into a dead-end alcove with the goal on the far side of a wall.
    for y in 0..MAP_H {
        g.cur_map[y][4] = b'#';
    }
    g.cur_map[8][4] = b'.'; // one narrow doorway
    g.player.x = 2.0;
    g.player.y = 2.5;
    put_enemy(&mut g, 0, 12.0, 2.5, EN_GRUNT);
    for _ in 0..240 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    // Either it found the doorway or the unstick juke fired; both mean it did
    // not sit still forever.
    assert!(g.bot.stuck_t < 0.4, "it should not be permanently jammed");
}

#[test]
fn goals_are_ranked_by_path_distance_not_straight_line() {
    // Regression: a target just past a wall (but a long walk away) used to beat
    // one genuinely down the corridor, and the bot yo-yoed between them.
    let mut g = open_room();
    for x in 0..MAP_W {
        g.cur_map[4][x] = b'#'; // full-width wall, no way through
    }
    g.player.x = 8.5;
    g.player.y = 5.5;
    // Two tiles away as the crow flies, but on the far side of the wall.
    put_enemy(&mut g, 0, 8.5, 3.0, EN_GRUNT);
    // Further in a straight line, but walkable.
    put_enemy(&mut g, 1, 8.5, 10.0, EN_GRUNT);

    for _ in 0..90 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.y > 5.5, "it should commit to the reachable enemy, not the near-but-walled one");
}

#[test]
fn the_bot_keeps_its_target_rather_than_flip_flopping() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 12.0, 8.0, EN_GRUNT);
    put_enemy(&mut g, 1, 12.2, 8.2, EN_GRUNT); // nearly identical distance
    g.bot_think(1.0 / 60.0);
    let first = g.bot.target;
    assert!(first >= 0);
    for _ in 0..10 {
        g.bot_think(1.0 / 60.0);
    }
    assert_eq!(g.bot.target, first, "hysteresis should hold the choice steady");
}
