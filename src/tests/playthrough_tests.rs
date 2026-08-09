//! End-to-end runs: the bot playing the real levels, frame by frame, through
//! the same update-and-render loop the binary uses.
//!
//! These are the integration net under the unit tests — they catch the kind of
//! breakage that only shows up when the AI, the simulation and the renderer are
//! all pointed at real level data (a bot that livelocks, a level that can't be
//! cleared, a sprite that panics at some particular distance).

use super::*;
use crate::constants::*;

/// Run `frames` of bot-driven play, rendering every frame like the real loop.
fn play(g: &mut Game, frames: usize) {
    for _ in 0..frames {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        g.render_frame();
    }
}

#[test]
fn the_bot_makes_progress_on_every_level() {
    for n in 0..LEVEL_COUNT {
        let mut g = super::new_game();
        g.reset_game();
        g.show_intro = false;
        g.load_level(n);
        // Give it the tools a player would have by this point in the run.
        g.player.weapons = [true, true, true];
        g.player.weapon = WP_RIFLE;
        g.player.ammo = 99;

        let start = live_enemies(&g);
        play(&mut g, 1800); // 30 seconds
        let left = live_enemies(&g);

        // "Progress" means thinning the level out, finishing it outright (at
        // which point the count refers to the *next* level's spawns), or dying
        // trying. Standing around with the same enemies alive is the failure.
        let advanced = g.level as usize != n;
        assert!(
            advanced || left < start || g.player.health == 0,
            "level {} made no progress in 30s ({start} enemies, still {left})",
            n + 1
        );
    }
}

#[test]
fn a_level_can_be_cleared_and_hands_over_to_the_next() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.player.weapons = [true, true, true];
    g.player.weapon = WP_RIFLE;
    g.player.ammo = 99;

    let mut advanced = false;
    for _ in 0..7200 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        g.render_frame();
        if g.level > 0 {
            advanced = true;
            break;
        }
    }
    assert!(advanced, "the bot should be able to finish the first level");
}

#[test]
fn a_full_frame_loop_stays_healthy_on_the_late_levels() {
    // The back half is where the extra mechanics live: lava under foot,
    // barrels going off, barons throwing volleys. Run each with rendering on
    // and assert the invariants that should hold no matter what happens.
    for n in 5..LEVEL_COUNT {
        let mut g = super::new_game();
        g.reset_game();
        g.show_intro = false;
        g.load_level(n);
        play(&mut g, 1200);

        assert!(g.running, "level {} stopped the game", n + 1);
        assert!(
            (0..=100).contains(&g.player.health),
            "level {} put health out of range: {}",
            n + 1,
            g.player.health
        );
        assert!((0..=99).contains(&g.player.ammo), "level {} broke the ammo count", n + 1);
        assert!(g.score >= 0);
        for e in g.enemies.iter().filter(|e| e.alive) {
            assert!(
                e.x > 0.0 && e.x < MAP_W as f64 && e.y > 0.0 && e.y < MAP_H as f64,
                "level {} let an enemy escape the map at ({}, {})",
                n + 1,
                e.x,
                e.y
            );
            assert!(e.hp > 0, "a live enemy should have hit points left");
        }
        assert!(
            g.player.x > 0.0 && g.player.x < MAP_W as f64,
            "level {} let the player leave the map",
            n + 1
        );
    }
}

#[test]
fn a_barrel_in_the_line_of_fire_goes_off_during_real_play() {
    // Driven by the bot rather than by calling shoot() directly: it lines up on
    // the grunt, the barrel is in the way, and the blast does the rest.
    let mut g = super::open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    super::put_barrel(&mut g, 0, 7.0, 8.0);
    super::put_enemy(&mut g, 0, 9.0, 8.0, EN_GRUNT);

    for _ in 0..600 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        g.render_frame();
        if !g.barrels[0].alive {
            break;
        }
    }
    assert!(!g.barrels[0].alive, "the bot should have shot the barrel between it and the grunt");
    assert!(!g.enemies[0].alive, "and the blast should have taken the grunt with it");
}

#[test]
fn the_player_takes_lava_damage_during_real_play() {
    // Level 9's centre is a lava cell: something in there ends up burning.
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.load_level(LEVEL_COUNT - 1);
    assert!(g.has_hazard);
    let mut burned = false;
    for _ in 0..3600 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        if g.map_hazard(g.player.x as i32, g.player.y as i32) && g.hazard_burn > 0.0 {
            burned = true;
            break;
        }
    }
    assert!(burned, "the bot should have had to cross the fire");
}

#[test]
fn dying_ends_the_run_cleanly() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.load_level(LEVEL_COUNT - 1);
    g.player.health = 1;
    g.player.ammo = 0; // no way to fight back

    let mut died = false;
    for _ in 0..3600 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        g.render_frame();
        if g.score_saved {
            died = true;
            break;
        }
    }
    assert!(died, "unarmed and on 1hp in the final level, it should go down");
    assert_eq!(g.player.health, 0);
    assert!(g.running, "death ends the run, not the process");
}

#[test]
fn the_attract_loop_restarts_after_a_death() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.player.health = 0;
    // Death is registered, score banked, then the bot restarts.
    for _ in 0..600 {
        g.bot_think(1.0 / 60.0);
        g.update_game(1.0 / 60.0);
        if g.player.health == 100 {
            break;
        }
    }
    assert_eq!(g.player.health, 100, "the demo should have started a fresh run");
    assert_eq!(g.level, 0);
}

#[test]
fn play_is_deterministic_for_a_given_seed() {
    let run = || {
        let mut g = super::new_game();
        g.reset_game();
        g.show_intro = false;
        play(&mut g, 600);
        (g.player.x, g.player.y, g.score, g.player.health, live_enemies(&g))
    };
    assert_eq!(run(), run(), "same seed, same run");
}
