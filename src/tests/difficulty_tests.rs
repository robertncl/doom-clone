//! Difficulty: the skill slider on the intro, and the tuning gradient it sets.

use super::*;
use crate::constants::*;
use crate::difficulty::*;

// ---- the gradient ----

#[test]
fn normal_is_the_original_tuning_on_every_level() {
    let id = Tuning::from_pressure(0.0);
    for lv in 0..LEVEL_COUNT {
        assert_eq!(pressure(SKILL_NORMAL, lv), 0.0);
        assert_eq!(Tuning::at(SKILL_NORMAL, lv), id);
    }
    assert_eq!(id.enemy_dmg, 1.0);
    assert_eq!(id.enemy_hp, 1.0);
    assert_eq!(id.enemy_speed, 1.0);
    assert_eq!(id.enemy_rate, 1.0);
    assert_eq!(id.fireball_speed, 1.0);
    assert_eq!(id.supply, 1.0);
}

#[test]
fn each_notch_up_the_slider_is_harsher_on_every_level() {
    for lv in 0..LEVEL_COUNT {
        for s in 1..SKILL_COUNT {
            assert!(
                pressure(s, lv) > pressure(s - 1, lv),
                "skill {s} should be harder than {} on level {lv}",
                s - 1
            );
            let (lo, hi) = (Tuning::at(s - 1, lv), Tuning::at(s, lv));
            assert!(hi.enemy_dmg > lo.enemy_dmg);
            assert!(hi.enemy_hp > lo.enemy_hp);
            assert!(hi.enemy_speed > lo.enemy_speed);
            assert!(hi.enemy_rate > lo.enemy_rate);
            assert!(hi.fireball_speed > lo.fireball_speed);
            assert!(hi.supply < lo.supply);
        }
    }
}

#[test]
fn easy_skills_are_flat_and_hard_skills_climb_with_depth() {
    let last = LEVEL_COUNT - 1;
    for s in [SKILL_BABY, SKILL_EASY, SKILL_NORMAL] {
        assert_eq!(pressure(s, 0), pressure(s, last), "skill {s} has no ramp");
    }
    for s in [SKILL_HARD, SKILL_NIGHTMARE] {
        for lv in 1..LEVEL_COUNT {
            assert!(pressure(s, lv) > pressure(s, lv - 1), "skill {s} climbs at level {lv}");
        }
    }
    assert!(pressure(SKILL_NIGHTMARE, last) - pressure(SKILL_NIGHTMARE, 0)
        > pressure(SKILL_HARD, last) - pressure(SKILL_HARD, 0),
        "nightmare's ramp is the steepest");
}

#[test]
fn out_of_range_skill_and_level_clamp() {
    assert_eq!(pressure(99, 0), pressure(SKILL_NIGHTMARE, 0));
    assert_eq!(pressure(SKILL_HARD, 99), pressure(SKILL_HARD, LEVEL_COUNT - 1));
}

#[test]
fn scale_rounds_and_never_drops_below_one() {
    assert_eq!(scale(7, 1.0), 7);
    assert_eq!(scale(8, 1.5), 12);
    assert_eq!(scale(3, 0.7), 2);
    assert_eq!(scale(2, 0.1), 1);
    assert_eq!(scale(5, 0.0), 1);
}

#[test]
fn skills_parse_by_number_or_name() {
    assert_eq!(parse_skill("1"), Some(SKILL_BABY));
    assert_eq!(parse_skill("5"), Some(SKILL_NIGHTMARE));
    assert_eq!(parse_skill("hard"), Some(SKILL_HARD));
    assert_eq!(parse_skill(" Nightmare "), Some(SKILL_NIGHTMARE));
    for bad in ["0", "6", "insane", ""] {
        assert_eq!(parse_skill(bad), None, "input {bad:?}");
    }
}

// ---- the slider ----

fn on_intro() -> Game {
    let mut g = open_room();
    g.show_intro = true;
    g
}

fn tap(g: &mut Game, k: usize) {
    g.key_edge[k] = true;
    g.update_game(1.0 / 60.0);
}

#[test]
fn the_slider_starts_on_normal() {
    assert_eq!(new_game().skill, SKILL_NORMAL);
}

#[test]
fn left_and_right_move_the_slider_without_starting_the_game() {
    let mut g = on_intro();
    tap(&mut g, K_TURNR);
    assert_eq!(g.skill, SKILL_HARD);
    tap(&mut g, K_STRAFER);
    assert_eq!(g.skill, SKILL_NIGHTMARE);
    tap(&mut g, K_TURNL);
    tap(&mut g, K_STRAFEL);
    assert_eq!(g.skill, SKILL_NORMAL);
    assert!(g.show_intro, "adjusting the slider doesn't dismiss the intro");
    assert!(!g.key_edge[K_TURNL] && !g.key_edge[K_STRAFEL], "the presses are consumed");

    tap(&mut g, K_SHOOT);
    assert!(!g.show_intro, "any other key starts the game");
    assert_eq!(g.skill, SKILL_NORMAL, "on the chosen skill");
}

#[test]
fn the_slider_stops_at_both_ends() {
    let mut g = on_intro();
    for _ in 0..10 {
        tap(&mut g, K_TURNL);
    }
    assert_eq!(g.skill, SKILL_BABY);
    for _ in 0..10 {
        tap(&mut g, K_TURNR);
    }
    assert_eq!(g.skill, SKILL_NIGHTMARE);
}

#[test]
fn the_slider_is_inert_once_the_game_is_running() {
    let mut g = open_room();
    tap(&mut g, K_TURNR);
    assert_eq!(g.skill, SKILL_NORMAL);
}

#[test]
fn the_skill_survives_a_restart() {
    let mut g = new_game();
    g.set_skill(SKILL_HARD);
    g.reset_game();
    assert_eq!(g.skill, SKILL_HARD);
}

// ---- what the skill changes ----

#[test]
fn changing_skill_on_the_intro_rescales_the_waiting_enemies() {
    let mut g = new_game();
    g.reset_game();
    let grunt = g.enemies.iter().position(|e| e.alive && e.kind == EN_GRUNT).unwrap();
    assert_eq!(g.enemies[grunt].hp, EN_HP[EN_GRUNT as usize]);
    g.set_skill(SKILL_BABY);
    assert_eq!(g.enemies[grunt].hp, 1);
    g.set_skill(SKILL_NIGHTMARE);
    assert_eq!(g.enemies[grunt].hp, scale(EN_HP[EN_GRUNT as usize], 1.25));
}

#[test]
fn spawned_enemy_hp_follows_skill_and_depth() {
    let baron_hp = |skill: usize, lv: usize| {
        let mut g = new_game();
        g.skill = skill;
        g.load_level(lv);
        g.enemies.iter().find(|e| e.alive && e.kind == EN_BARON).map(|e| e.hp)
    };
    let last = LEVEL_COUNT - 1;
    assert_eq!(baron_hp(SKILL_NORMAL, last), Some(EN_HP[EN_BARON as usize]));
    assert!(baron_hp(SKILL_BABY, last) < baron_hp(SKILL_NORMAL, last));
    assert_eq!(baron_hp(SKILL_NIGHTMARE, last), Some(scale(EN_HP[EN_BARON as usize], 1.5)));
}

#[test]
fn melee_hits_harder_on_harder_skills() {
    let taken = |skill: usize| {
        let mut g = open_room();
        g.skill = skill;
        put_enemy(&mut g, 0, 3.0, 2.5, EN_GRUNT);
        g.update_enemies(1.0 / 60.0);
        100 - g.player.health
    };
    assert_eq!(taken(SKILL_NORMAL), 7);
    assert!(taken(SKILL_BABY) < 7);
    assert!(taken(SKILL_NIGHTMARE) > 7);
}

#[test]
fn melee_cooldown_shrinks_on_harder_skills() {
    let cool = |skill: usize| {
        let mut g = open_room();
        g.skill = skill;
        put_enemy(&mut g, 0, 3.0, 2.5, EN_GRUNT);
        g.update_enemies(1.0 / 60.0);
        g.enemies[0].atk_cool
    };
    assert_eq!(cool(SKILL_NORMAL), 1.0);
    assert!(cool(SKILL_NIGHTMARE) < cool(SKILL_NORMAL));
    assert!(cool(SKILL_BABY) > cool(SKILL_NORMAL));
}

#[test]
fn imp_fireballs_fly_faster_and_burn_harder_on_harder_skills() {
    let shot = |skill: usize| {
        let mut g = open_room();
        g.skill = skill;
        put_enemy(&mut g, 0, 6.5, 2.5, EN_IMP);
        g.update_enemies(1.0 / 60.0);
        let fb = g.fireballs.iter().find(|f| f.alive).copied().unwrap();
        ((fb.vx * fb.vx + fb.vy * fb.vy).sqrt(), fb.dmg)
    };
    let (n_spd, n_dmg) = shot(SKILL_NORMAL);
    assert!((n_spd - 3.0).abs() < 1e-9);
    assert_eq!(n_dmg, 12);
    let (h_spd, h_dmg) = shot(SKILL_NIGHTMARE);
    assert!(h_spd > n_spd && h_dmg > n_dmg);
}

#[test]
fn enemies_close_in_faster_on_harder_skills() {
    let step = |skill: usize| {
        let mut g = open_room();
        g.skill = skill;
        put_enemy(&mut g, 0, 8.5, 2.5, EN_GRUNT);
        g.update_enemies(0.1);
        8.5 - g.enemies[0].x
    };
    assert!(step(SKILL_NIGHTMARE) > step(SKILL_NORMAL));
    assert!(step(SKILL_BABY) < step(SKILL_NORMAL));
}

#[test]
fn pickups_are_more_generous_on_easier_skills() {
    let gained = |skill: usize, kind: i32| {
        let mut g = open_room();
        g.skill = skill;
        g.player.health = 10;
        g.player.ammo = 0;
        put_pickup(&mut g, 0, 2.5, 2.5, kind);
        g.update_pickups();
        if kind == PU_HEALTH { g.player.health - 10 } else { g.player.ammo }
    };
    assert_eq!(gained(SKILL_NORMAL, PU_HEALTH), 25);
    assert_eq!(gained(SKILL_NORMAL, PU_AMMO), 12);
    assert!(gained(SKILL_BABY, PU_HEALTH) > 25);
    assert!(gained(SKILL_NIGHTMARE, PU_AMMO) < 12);
}

// ---- drawing ----

#[test]
fn the_intro_draws_the_selected_skill_in_its_colour() {
    for s in [SKILL_BABY, SKILL_NIGHTMARE] {
        let mut g = open_room();
        g.skill = s;
        g.pixels.fill(0);
        g.draw_intro();
        assert!(count_color(&g, SKILL_COLORS[s]) > 100, "skill {s} lit in its colour");
    }
}

#[test]
fn the_slider_lights_up_to_the_selection() {
    let lit = |skill: usize| {
        let mut g = open_room();
        g.skill = skill;
        g.pixels.fill(0);
        g.draw_intro();
        SKILL_COLORS.iter().filter(|&&c| count_color(&g, c) > 100).count()
    };
    assert_eq!(lit(SKILL_BABY), 1);
    assert_eq!(lit(SKILL_NORMAL), 3);
    assert_eq!(lit(SKILL_NIGHTMARE), SKILL_COUNT);
}

#[test]
fn the_game_over_screen_names_the_skill() {
    let mut g = open_room();
    g.skill = SKILL_HARD;
    g.player.health = 0;
    g.pixels.fill(0);
    g.draw_game_over_overlay();
    assert!(count_color(&g, SKILL_COLORS[SKILL_HARD]) > 0);
}
