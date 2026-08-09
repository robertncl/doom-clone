//! Game logic: particles, projectiles, enemy AI, barrels, lava, pickups,
//! shooting, and the per-frame update that ties them together.

use super::*;
use crate::constants::*;
use crate::entity::shot_hit_dist;
use crate::types::Enemy;

// ---- hitscan alignment ----

#[test]
fn shot_alignment_widens_for_close_targets_and_tightens_far_away() {
    // Dead ahead always hits, near or far.
    assert!(shot_hit_dist(0.0, 0.0, 0.0, 1.0, 0.0).is_some());
    assert!(shot_hit_dist(0.0, 0.0, 0.0, 20.0, 0.0).is_some());
    // A fixed lateral offset that is inside the cone up close...
    assert!(shot_hit_dist(0.0, 0.0, 0.0, 1.0, 0.15).is_some());
    // ...is outside it at range, because the tolerance shrinks with distance.
    assert!(shot_hit_dist(0.0, 0.0, 0.0, 20.0, 3.0).is_none());
    // Directly behind is never a hit.
    assert!(shot_hit_dist(0.0, 0.0, 0.0, -5.0, 0.0).is_none());
    // Angle wrapping: aiming at -PI vs +PI is the same direction.
    assert!(shot_hit_dist(0.0, 0.0, std::f64::consts::PI, -5.0, 0.0).is_some());
    assert!(shot_hit_dist(0.0, 0.0, -std::f64::consts::PI, -5.0, 0.0).is_some());
    // Returned value is the distance to the target.
    let d = shot_hit_dist(0.0, 0.0, 0.0, 3.0, 0.0).unwrap();
    assert!((d - 3.0).abs() < 1e-9);
}

// ---- particles ----

#[test]
fn particles_spawn_move_and_expire() {
    let mut g = open_room();
    g.spawn_particle(5.0, 5.0, 1.0, 0.0, 0.5, 0xFF0000);
    assert_eq!(live_particles(&g), 1);

    g.update_particles(0.1);
    let p = g.parts.iter().find(|p| p.life > 0.0).unwrap();
    assert!(p.x > 5.0, "a particle with +X velocity should drift right");
    assert!(p.vx < 1.0, "velocity should damp each step");

    // Run past its lifetime and it goes away.
    for _ in 0..20 {
        g.update_particles(0.1);
    }
    assert_eq!(live_particles(&g), 0);
}

#[test]
fn the_particle_pool_is_bounded() {
    let mut g = open_room();
    for _ in 0..MAX_PARTICLES + 40 {
        g.spawn_particle(5.0, 5.0, 0.0, 0.0, 1.0, 0xFFFFFF);
    }
    assert_eq!(live_particles(&g), MAX_PARTICLES, "spawns past the pool are dropped");
}

#[test]
fn blood_and_sparks_produce_particles() {
    let mut g = open_room();
    g.spawn_blood(4.0, 4.0, 8);
    assert_eq!(live_particles(&g), 8);
    let mut g = open_room();
    g.spawn_sparks(4.0, 4.0);
    assert_eq!(live_particles(&g), 6);
}

// ---- fireballs ----

#[test]
fn a_fireball_flies_toward_its_target() {
    let mut g = open_room();
    g.spawn_fireball(2.0, 2.0, 8.0, 2.0, 0.0, 3.0, 12);
    let fb = g.fireballs[0];
    assert!(fb.alive);
    assert_eq!(fb.dmg, 12);
    assert!((fb.vx - 3.0).abs() < 1e-9, "speed goes into the aim direction");
    assert!(fb.vy.abs() < 1e-9);
}

#[test]
fn a_fireball_aimed_at_its_own_position_is_not_launched() {
    let mut g = open_room();
    g.spawn_fireball(2.0, 2.0, 2.0, 2.0, 0.0, 3.0, 12);
    assert_eq!(live_fireballs(&g), 0, "a zero-length aim has no direction to fly");
}

#[test]
fn spread_rotates_a_volley_off_the_aim_line() {
    let mut g = open_room();
    g.spawn_fireball(2.0, 2.0, 8.0, 2.0, 0.5, 3.0, 12);
    assert!(g.fireballs[0].vy > 0.0, "positive spread should steer off-axis");
}

#[test]
fn the_fireball_pool_is_bounded() {
    let mut g = open_room();
    for _ in 0..MAX_FIREBALLS + 10 {
        g.spawn_fireball(2.0, 2.0, 8.0, 2.0, 0.0, 3.0, 12);
    }
    assert_eq!(live_fireballs(&g), MAX_FIREBALLS);
}

#[test]
fn a_fireball_bursts_on_a_wall() {
    let mut g = open_room();
    // Fired at the west wall from just inside it.
    g.spawn_fireball(1.5, 5.0, 0.0, 5.0, 0.0, 3.0, 12);
    for _ in 0..30 {
        g.update_fireballs(1.0 / 60.0);
    }
    assert_eq!(live_fireballs(&g), 0, "it should die against the wall");
    assert!(live_particles(&g) > 0, "and throw sparks where it hit");
}

#[test]
fn a_fireball_that_reaches_the_player_hurts_them() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 5.0;
    g.spawn_fireball(5.0, 5.0, 8.0, 5.0, 0.0, 3.0, 12);
    for _ in 0..90 {
        g.update_fireballs(1.0 / 60.0);
        if g.player.health < 100 {
            break;
        }
    }
    assert_eq!(g.player.health, 88);
    assert!(g.pain_flash > 0.0, "taking a hit should flash the screen");
    assert_eq!(live_fireballs(&g), 0, "the fireball is consumed by the hit");
}

#[test]
fn a_fireball_cannot_drive_health_below_zero() {
    let mut g = open_room();
    g.player.health = 5;
    g.player.x = 8.0;
    g.player.y = 5.0;
    g.spawn_fireball(5.0, 5.0, 8.0, 5.0, 0.0, 3.0, 40);
    for _ in 0..90 {
        g.update_fireballs(1.0 / 60.0);
        if g.player.health == 0 {
            break;
        }
    }
    assert_eq!(g.player.health, 0);
}

#[test]
fn a_fireball_expires_when_its_life_runs_out() {
    let mut g = open_room();
    // Aimed along a long clear corridor so nothing stops it early.
    g.player.x = 14.0;
    g.player.y = 14.0;
    g.spawn_fireball(2.0, 8.0, 3.0, 8.0, 0.0, 0.05, 12);
    for _ in 0..400 {
        g.update_fireballs(1.0 / 60.0);
    }
    assert_eq!(live_fireballs(&g), 0, "life should run out even without a collision");
}

#[test]
fn enemy_fire_detonates_a_barrel_it_flies_into() {
    let mut g = open_room();
    put_barrel(&mut g, 0, 6.0, 5.0);
    g.player.x = 12.0;
    g.player.y = 5.0;
    g.spawn_fireball(4.0, 5.0, 12.0, 5.0, 0.0, 3.0, 12);
    for _ in 0..90 {
        g.update_fireballs(1.0 / 60.0);
        if !g.barrels[0].alive {
            break;
        }
    }
    assert!(!g.barrels[0].alive, "the barrel should go up");
    assert_eq!(g.player.health, 100, "and the player, far away, is untouched");
}

// ---- enemy movement helpers ----

#[test]
fn move_enemy_slides_along_a_wall_instead_of_stopping() {
    let g = open_room();
    // Wall to the west (x < 1). Pushing north-west should still move north.
    let mut e = Enemy { x: 1.1, y: 5.0, ..Default::default() };
    g.move_enemy(&mut e, -0.5, 0.5);
    assert!((e.x - 1.1).abs() < 1e-9, "blocked axis holds");
    assert!(e.y > 5.0, "free axis still moves");
}

#[test]
fn melee_never_hits_a_player_who_is_already_down() {
    let mut g = open_room();
    g.player.health = 0;
    g.melee_player(20);
    assert_eq!(g.player.health, 0);
    assert_eq!(g.pain_flash, 0.0, "no flash for a hit that never landed");
}

// ---- enemy AI, per kind ----

#[test]
fn a_grunt_closes_the_distance_then_swings() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 8.0, 2.5, EN_GRUNT);
    let start = g.enemies[0].x;
    g.update_enemies(0.1);
    assert!(g.enemies[0].x < start, "it should walk toward the player");

    // Point blank: it attacks instead of moving.
    put_enemy(&mut g, 0, 3.0, 2.5, EN_GRUNT);
    g.update_enemies(0.1);
    assert_eq!(g.player.health, 93, "a grunt swing costs 7");
    assert!(g.enemies[0].atk_cool > 0.0, "and starts a cooldown");

    // Still on cooldown, so no second hit this frame.
    g.update_enemies(0.1);
    assert_eq!(g.player.health, 93);
}

#[test]
fn an_imp_holds_a_firing_range() {
    // Too far: it advances.
    let mut g = open_room();
    put_enemy(&mut g, 0, 10.0, 2.5, EN_IMP);
    g.update_enemies(0.1);
    assert!(g.enemies[0].x < 10.0);

    // Too close: it backs off.
    let mut g = open_room();
    put_enemy(&mut g, 0, 4.0, 2.5, EN_IMP);
    g.enemies[0].atk_cool = 5.0; // suppress the shot so we only see movement
    g.update_enemies(0.1);
    assert!(g.enemies[0].x > 4.0, "an imp backs away when crowded");
}

#[test]
fn an_imp_throws_one_fireball_per_cooldown() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 6.0, 2.5, EN_IMP);
    g.update_enemies(1.0 / 60.0);
    assert_eq!(live_fireballs(&g), 1);
    assert_eq!(g.fireballs[0].dmg, 12, "imp fire is the light kind");
    // Immediately after, it's on cooldown.
    g.update_enemies(1.0 / 60.0);
    assert_eq!(live_fireballs(&g), 1);
}

#[test]
fn an_imp_out_of_range_holds_its_fire() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 13.5, 2.5, EN_IMP);
    g.update_enemies(1.0 / 60.0);
    assert_eq!(live_fireballs(&g), 0, "nothing to shoot at from across the map");
}

#[test]
fn wraiths_spiral_in_rather_than_charging_straight() {
    let mut g = open_room();
    // Even and odd slots orbit opposite ways, so a pack fans out.
    put_enemy(&mut g, 0, 8.0, 2.5, EN_WRAITH);
    put_enemy(&mut g, 1, 8.0, 2.5, EN_WRAITH);
    g.global_time = 0.4; // put the swirl term off zero
    for _ in 0..20 {
        g.update_enemies(1.0 / 60.0);
    }
    assert!(g.enemies[0].x < 8.0, "it still closes overall");
    assert!(
        (g.enemies[0].y - g.enemies[1].y).abs() > 1e-6,
        "opposite orbit directions should separate them"
    );
}

#[test]
fn a_wraith_claws_at_point_blank() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 3.0, 2.5, EN_WRAITH);
    g.update_enemies(0.1);
    assert_eq!(g.player.health, 92, "a wraith swipe costs 8");
}

#[test]
fn a_baron_fans_three_fireballs_and_hits_harder_up_close() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 8.5, 2.5, EN_BARON);
    g.update_enemies(1.0 / 60.0);
    assert_eq!(live_fireballs(&g), 3, "a volley is three wide");
    assert!(g.fireballs[0].dmg > 12, "baron fire outweighs imp fire");
    // The fan is spread: not every shot flies on the same line.
    let vys: Vec<f64> = g.fireballs.iter().filter(|f| f.alive).map(|f| f.vy).collect();
    assert!(vys.iter().any(|v| *v > 0.0) && vys.iter().any(|v| *v < 0.0));

    // In melee range it punches instead.
    let mut g = open_room();
    put_enemy(&mut g, 0, 3.2, 2.5, EN_BARON);
    g.update_enemies(1.0 / 60.0);
    assert_eq!(g.player.health, 82, "a baron fist costs 18");
    assert_eq!(live_fireballs(&g), 0);
}

#[test]
fn a_baron_walks_in_from_across_the_room() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 12.0, 2.5, EN_BARON);
    g.enemies[0].atk_cool = 5.0;
    g.update_enemies(0.1);
    assert!(g.enemies[0].x < 12.0);
}

#[test]
fn dead_and_coincident_enemies_are_skipped_safely() {
    let mut g = open_room();
    // A corpse still ticks its hit flash down, but does nothing else.
    put_enemy(&mut g, 0, 5.0, 5.0, EN_GRUNT);
    g.enemies[0].alive = false;
    g.enemies[0].hit_flash = 0.1;
    // An enemy exactly on the player has no direction to move in.
    put_enemy(&mut g, 1, 2.5, 2.5, EN_GRUNT);
    g.update_enemies(1.0 / 60.0);
    assert!(g.enemies[0].hit_flash < 0.1);
    assert!((g.enemies[1].x - 2.5).abs() < 1e-9);
}

// ---- damage and scoring ----

#[test]
fn damaging_an_enemy_scores_only_on_the_kill() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 5.0, 5.0, EN_BARON);
    g.damage_enemy(0, 1);
    assert_eq!(g.score, 0, "a wound is not a kill");
    assert!(g.enemies[0].hit_flash > 0.0);
    g.damage_enemy(0, 99);
    assert!(!g.enemies[0].alive);
    assert_eq!(g.score, EN_SCORE[EN_BARON as usize]);

    // A corpse can't be scored twice.
    g.damage_enemy(0, 99);
    assert_eq!(g.score, EN_SCORE[EN_BARON as usize]);
}

#[test]
fn each_kind_is_worth_its_own_score() {
    for kind in [EN_GRUNT, EN_IMP, EN_WRAITH, EN_BARON] {
        let mut g = open_room();
        put_enemy(&mut g, 0, 5.0, 5.0, kind);
        g.damage_enemy(0, 99);
        assert_eq!(g.score, EN_SCORE[kind as usize]);
    }
}

// ---- barrels ----

#[test]
fn shooting_a_barrel_blasts_nearby_enemies() {
    let mut g = open_room();
    put_barrel(&mut g, 0, 5.5, 2.5);
    // In the blast, and behind the barrel so the pellet stops at the barrel.
    put_enemy(&mut g, 0, 6.4, 2.5, EN_GRUNT);
    // Well outside BARREL_RADIUS.
    put_enemy(&mut g, 1, 11.5, 2.5, EN_GRUNT);

    g.shoot();

    assert!(!g.barrels[0].alive, "the barrel should have gone up");
    assert!(!g.enemies[0].alive, "the blast should have killed the near grunt");
    assert!(g.enemies[1].alive, "the far grunt is out of range");
    assert_eq!(g.enemies[1].hp, EN_HP[EN_GRUNT as usize]);
}

#[test]
fn barrel_blasts_chain_through_neighbours() {
    let mut g = open_room();
    put_barrel(&mut g, 0, 5.5, 2.5);
    put_barrel(&mut g, 1, 7.0, 2.5);
    // Only in range of the *second* barrel, so it can only die by the chain.
    put_enemy(&mut g, 0, 8.4, 2.5, EN_GRUNT);

    g.shoot();

    assert!(!g.barrels[0].alive && !g.barrels[1].alive, "both barrels should blow");
    assert!(!g.enemies[0].alive, "the chained blast should reach the far grunt");
}

#[test]
fn the_player_takes_damage_from_their_own_barrel() {
    let mut g = open_room();
    put_barrel(&mut g, 0, 3.4, 2.5);
    g.shoot();
    assert!(g.player.health < 100, "popping a barrel point-blank should hurt");
    assert!(g.pain_flash > 0.0);
}

#[test]
fn blast_damage_falls_off_with_distance() {
    let near = {
        let mut g = open_room();
        put_enemy(&mut g, 0, 5.7, 2.5, EN_BARON);
        put_barrel(&mut g, 0, 5.5, 2.5);
        g.explode_barrel(0);
        g.enemies[0].hp
    };
    let far = {
        let mut g = open_room();
        put_enemy(&mut g, 0, 7.6, 2.5, EN_BARON);
        put_barrel(&mut g, 0, 5.5, 2.5);
        g.explode_barrel(0);
        g.enemies[0].hp
    };
    assert!(near < far, "closer to the barrel means more damage ({near} vs {far})");
}

#[test]
fn a_blast_the_player_is_clear_of_leaves_them_alone() {
    let mut g = open_room();
    put_barrel(&mut g, 0, 12.0, 12.0);
    g.explode_barrel(0);
    assert_eq!(g.player.health, 100);
}

#[test]
fn a_barrel_blast_scores_its_kills() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 5.6, 2.5, EN_GRUNT);
    put_barrel(&mut g, 0, 5.5, 2.5);
    g.explode_barrel(0);
    assert!(!g.enemies[0].alive);
    assert_eq!(g.score, EN_SCORE[EN_GRUNT as usize]);
}

#[test]
fn barrel_lookup_finds_only_live_barrels_in_range() {
    let mut g = open_room();
    put_barrel(&mut g, 0, 5.0, 5.0);
    assert_eq!(g.barrel_at(5.1, 5.0, 0.45), Some(0));
    assert_eq!(g.barrel_at(9.0, 9.0, 0.45), None);
    g.barrels[0].alive = false;
    assert_eq!(g.barrel_at(5.1, 5.0, 0.45), None);
}

// ---- lava ----

#[test]
fn lava_burns_the_player_at_the_tuned_rate() {
    let mut g = open_room();
    g.cur_map[2][2] = b'~'; // the tile the player is standing on
    g.has_hazard = true;

    for _ in 0..60 {
        g.update_hazard(1.0 / 60.0);
    }
    let burned = 100 - g.player.health;
    assert!(
        (burned - HAZARD_DPS as i32).abs() <= 1,
        "one second in lava should cost about {} health, took {}",
        HAZARD_DPS,
        burned
    );

    // Stepping off stops the burn.
    g.player.x = 4.5;
    let health = g.player.health;
    for _ in 0..60 {
        g.update_hazard(1.0 / 60.0);
    }
    assert_eq!(g.player.health, health, "dry ground should not burn");
}

#[test]
fn a_lava_burn_cannot_push_health_below_zero() {
    let mut g = open_room();
    g.cur_map[2][2] = b'~';
    g.has_hazard = true;
    g.player.health = 3;
    for _ in 0..600 {
        g.update_hazard(1.0 / 60.0);
    }
    assert_eq!(g.player.health, 0);
}

#[test]
fn a_partial_frame_of_lava_does_no_whole_damage_yet() {
    let mut g = open_room();
    g.cur_map[2][2] = b'~';
    g.has_hazard = true;
    g.update_hazard(1.0 / 600.0); // far less than one point's worth
    assert_eq!(g.player.health, 100, "fractional burn accrues, it doesn't round up");
    assert!(g.hazard_burn > 0.0);
}

// ---- pickups ----

#[test]
fn health_and_ammo_pickups_top_the_player_up_and_clamp() {
    let mut g = open_room();
    g.player.health = 50;
    g.player.ammo = 10;
    put_pickup(&mut g, 0, 2.5, 2.5, PU_HEALTH);
    put_pickup(&mut g, 1, 2.5, 2.5, PU_AMMO);
    g.update_pickups();
    assert_eq!(g.player.health, 75);
    assert_eq!(g.player.ammo, 22);
    assert!(!g.pickups[0].alive && !g.pickups[1].alive);
    assert!(live_particles(&g) > 0, "pickups pop some sparkle");

    // Both cap out rather than overflowing.
    let mut g = open_room();
    g.player.health = 95;
    g.player.ammo = 95;
    put_pickup(&mut g, 0, 2.5, 2.5, PU_HEALTH);
    put_pickup(&mut g, 1, 2.5, 2.5, PU_AMMO);
    g.update_pickups();
    assert_eq!(g.player.health, 100);
    assert_eq!(g.player.ammo, 99);
}

#[test]
fn weapon_pickups_are_owned_equipped_and_come_with_ammo() {
    for (kind, wp) in [(PU_SHOTGUN, WP_SHOTGUN), (PU_RIFLE, WP_RIFLE)] {
        let mut g = open_room();
        g.player.ammo = 0;
        put_pickup(&mut g, 0, 2.5, 2.5, kind);
        g.update_pickups();
        assert!(g.player.weapons[wp as usize], "picking a gun up should own it");
        assert_eq!(g.player.weapon, wp, "and equip it");
        assert_eq!(g.player.ammo, 8, "with enough ammo to be useful");
    }
}

#[test]
fn a_pickup_out_of_reach_is_not_collected() {
    let mut g = open_room();
    put_pickup(&mut g, 0, 9.0, 9.0, PU_HEALTH);
    g.player.health = 50;
    g.update_pickups();
    assert!(g.pickups[0].alive);
    assert_eq!(g.player.health, 50);
}

// ---- shooting ----

#[test]
fn shooting_costs_ammo_and_needs_ammo() {
    let mut g = open_room();
    g.player.ammo = 1;
    g.shoot();
    assert_eq!(g.player.ammo, 0);
    assert!(g.muzzle_flash > 0);

    g.muzzle_flash = 0;
    g.shoot();
    assert_eq!(g.player.ammo, 0, "an empty gun does nothing");
    assert_eq!(g.muzzle_flash, 0);
}

#[test]
fn each_weapon_has_its_own_damage_profile() {
    // Pistol: one point per shot.
    let mut g = open_room();
    put_enemy(&mut g, 0, 6.0, 2.5, EN_BARON);
    g.shoot();
    assert_eq!(g.enemies[0].hp, EN_HP[EN_BARON as usize] - 1);

    // Rifle: three.
    let mut g = open_room();
    g.player.weapon = WP_RIFLE;
    put_enemy(&mut g, 0, 6.0, 2.5, EN_BARON);
    g.shoot();
    assert_eq!(g.enemies[0].hp, EN_HP[EN_BARON as usize] - 3);

    // Shotgun: several pellets, so more than the pistol at point-blank where
    // the whole spread lands.
    let mut g = open_room();
    g.player.weapon = WP_SHOTGUN;
    put_enemy(&mut g, 0, 3.2, 2.5, EN_BARON);
    g.shoot();
    assert!(g.enemies[0].hp < EN_HP[EN_BARON as usize] - 1, "the spread should multi-hit");
}

#[test]
fn a_shot_that_hits_a_wall_throws_sparks() {
    let mut g = open_room();
    g.shoot();
    assert!(live_particles(&g) > 0);
}

#[test]
fn a_shot_stops_at_the_wall_and_spares_whatever_is_behind_it() {
    let mut g = open_room();
    // Wall the corridor off between the player and the target.
    for y in 0..MAP_H {
        g.cur_map[y][6] = b'#';
    }
    put_enemy(&mut g, 0, 9.0, 2.5, EN_GRUNT);
    g.shoot();
    assert_eq!(g.enemies[0].hp, EN_HP[EN_GRUNT as usize], "the wall ate the shot");
}

#[test]
fn a_shot_picks_the_nearest_aligned_target() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 9.0, 2.5, EN_GRUNT);
    put_enemy(&mut g, 1, 5.0, 2.5, EN_GRUNT);
    g.shoot();
    assert_eq!(g.enemies[0].hp, EN_HP[EN_GRUNT as usize], "the far one is shielded");
    assert!(g.enemies[1].hp < EN_HP[EN_GRUNT as usize]);
}

#[test]
fn a_dead_enemy_does_not_absorb_shots() {
    let mut g = open_room();
    put_enemy(&mut g, 0, 5.0, 2.5, EN_GRUNT);
    g.enemies[0].alive = false;
    put_enemy(&mut g, 1, 9.0, 2.5, EN_GRUNT);
    g.shoot();
    assert!(g.enemies[1].hp < EN_HP[EN_GRUNT as usize], "the shot passes through the corpse");
}

// ---- the per-frame update ----

#[test]
fn the_intro_swallows_the_first_keypress() {
    let mut g = open_room();
    g.show_intro = true;
    g.key_edge[K_FWD] = true;
    g.update_game(1.0 / 60.0);
    assert!(!g.show_intro, "any key dismisses the intro");
    assert!(!g.key_edge[K_FWD], "and that press is consumed, not passed through");
}

#[test]
fn quitting_from_the_intro_exits() {
    let mut g = open_room();
    g.show_intro = true;
    g.key_edge[K_QUIT] = true;
    g.update_game(1.0 / 60.0);
    assert!(!g.running);
    assert!(g.show_intro, "quit does not count as dismissing it");
}

#[test]
fn the_intro_waits_when_no_key_is_pressed() {
    let mut g = open_room();
    g.show_intro = true;
    g.update_game(1.0 / 60.0);
    assert!(g.show_intro);
}

#[test]
fn movement_keys_drive_the_player_and_ease_to_a_stop() {
    let mut g = open_room();
    let x0 = g.player.x;
    g.keys[K_FWD] = true;
    for _ in 0..20 {
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.x > x0, "forward should move along the facing");
    assert!(g.player.bob > 0.0, "and advance the walk bob");

    // Release and the velocity decays rather than snapping to zero.
    g.keys[K_FWD] = false;
    let v = g.player.vx;
    g.update_game(1.0 / 60.0);
    assert!(g.player.vx < v && g.player.vx > 0.0, "friction eases it down");
}

#[test]
fn every_movement_and_turn_key_does_something() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;

    for (key, check) in [
        (K_BACK, "back"),
        (K_STRAFEL, "strafe left"),
        (K_STRAFER, "strafe right"),
    ] {
        let mut g2 = open_room();
        g2.player.x = 8.0;
        g2.player.y = 8.0;
        let (x0, y0) = (g2.player.x, g2.player.y);
        g2.keys[key] = true;
        for _ in 0..20 {
            g2.update_game(1.0 / 60.0);
        }
        assert!(
            (g2.player.x - x0).abs() > 1e-3 || (g2.player.y - y0).abs() > 1e-3,
            "{check} should move the player"
        );
    }

    g.keys[K_TURNL] = true;
    for _ in 0..10 {
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.angle < 0.0, "turn left decreases the angle");

    g.keys[K_TURNL] = false;
    g.keys[K_TURNR] = true;
    for _ in 0..30 {
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.angle > 0.0, "turn right brings it back");
}

#[test]
fn walking_into_a_wall_kills_that_axis_of_velocity() {
    let mut g = open_room();
    g.player.x = 1.5;
    g.player.y = 2.5;
    g.player.angle = std::f64::consts::PI; // face the west wall
    g.keys[K_FWD] = true;
    for _ in 0..30 {
        g.update_game(1.0 / 60.0);
    }
    assert!(g.player.x > 1.0, "the wall stops us");
    assert!(g.player.vx.abs() < 1e-6, "and the blocked axis stops accumulating");
}

#[test]
fn the_shoot_key_fires_once_per_press() {
    let mut g = open_room();
    g.key_edge[K_SHOOT] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.player.ammo, 49);
    assert!(!g.key_edge[K_SHOOT], "the edge is consumed");
    g.update_game(1.0 / 60.0);
    assert_eq!(g.player.ammo, 49, "holding it down does not auto-fire");
}

#[test]
fn weapon_keys_only_select_guns_the_player_owns() {
    let mut g = open_room();
    g.key_edge[K_WEAPON2] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.player.weapon, WP_PISTOL, "we don't own the shotgun yet");

    g.player.weapons[WP_SHOTGUN as usize] = true;
    g.key_edge[K_WEAPON2] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.player.weapon, WP_SHOTGUN);

    g.player.weapons[WP_RIFLE as usize] = true;
    g.key_edge[K_WEAPON3] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.player.weapon, WP_RIFLE);

    g.key_edge[K_WEAPON1] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.player.weapon, WP_PISTOL);
}

#[test]
fn a_dead_player_coasts_to_a_halt_and_stops_acting() {
    let mut g = open_room();
    g.player.health = 0;
    g.player.vx = 2.0;
    g.player.va = 1.0;
    g.key_edge[K_SHOOT] = true;
    g.update_game(1.0 / 60.0);
    assert!(g.player.vx < 2.0, "velocity bleeds off");
    assert!(g.player.va < 1.0);
    assert_eq!(g.player.ammo, 50, "a corpse can't shoot");
}

#[test]
fn dying_saves_the_score_once() {
    let mut g = open_room();
    g.score = 1234;
    g.high_scores = [0; MAX_HIGHSCORES];
    g.player.health = 0;
    g.update_game(1.0 / 60.0);
    assert!(g.score_saved);
    assert_eq!(g.final_rank, 1, "a first score takes the top slot");
    // Running further frames doesn't re-submit.
    let rank = g.final_rank;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.final_rank, rank);
}

#[test]
fn restart_is_only_offered_once_the_score_is_saved() {
    let mut g = open_room();
    // Something still alive, so the level doesn't clear and pay a bonus midway
    // through the test.
    put_enemy(&mut g, 0, 12.0, 12.0, EN_GRUNT);
    g.score = 500;
    g.key_edge[K_RESTART] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.score, 500, "no restart mid-run");

    g.player.health = 0;
    g.update_game(1.0 / 60.0); // saves the score
    g.key_edge[K_RESTART] = true;
    g.update_game(1.0 / 60.0);
    assert_eq!(g.score, 0, "now it restarts");
    assert_eq!(g.player.health, 100);
    assert_eq!(g.level, 0);
}

#[test]
fn the_quit_key_stops_the_game() {
    let mut g = open_room();
    g.key_edge[K_QUIT] = true;
    g.update_game(1.0 / 60.0);
    assert!(!g.running);
}

#[test]
fn the_muzzle_flash_fades_on_its_own() {
    let mut g = open_room();
    g.muzzle_flash = 3;
    for _ in 0..5 {
        g.update_game(1.0 / 60.0);
    }
    assert_eq!(g.muzzle_flash, 0);
}

#[test]
fn the_pain_flash_fades_on_its_own() {
    let mut g = open_room();
    g.pain_flash = 0.1;
    for _ in 0..20 {
        g.update_game(1.0 / 60.0);
    }
    assert!(g.pain_flash <= 0.0);
}

#[test]
fn clearing_a_level_pays_a_bonus_then_moves_on() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    // Wipe the level out.
    for e in g.enemies.iter_mut() {
        e.alive = false;
    }
    g.update_game(1.0 / 60.0);
    assert!(g.level_bonus_given, "clearing pays out immediately");
    let score = g.score;
    assert!(score > 0);

    // The bonus is paid once, not every frame.
    g.update_game(1.0 / 60.0);
    assert_eq!(g.score, score);

    // After the celebration delay, the next level loads.
    for _ in 0..200 {
        g.update_game(1.0 / 60.0);
        if g.level == 1 {
            break;
        }
    }
    assert_eq!(g.level, 1);
    assert!(!g.level_bonus_given, "and the new level starts fresh");
}

#[test]
fn clearing_the_last_level_wins_the_game() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.load_level(LEVEL_COUNT - 1);
    for e in g.enemies.iter_mut() {
        e.alive = false;
    }
    for _ in 0..300 {
        g.update_game(1.0 / 60.0);
        if g.score_saved {
            break;
        }
    }
    assert!(g.score_saved, "the run is banked");
    assert!(g.player.health > 0, "and it's a win, not a death");
    assert_eq!(g.level as usize, LEVEL_COUNT - 1, "there is nowhere further to go");
}

#[test]
fn a_full_frame_runs_every_subsystem_together() {
    let mut g = open_room();
    g.has_hazard = true;
    g.cur_map[2][2] = b'~';
    put_enemy(&mut g, 0, 5.0, 2.5, EN_IMP);
    put_barrel(&mut g, 0, 8.0, 8.0);
    put_pickup(&mut g, 0, 6.0, 6.0, PU_AMMO);
    g.spawn_particle(3.0, 3.0, 0.1, 0.1, 1.0, 0xFFFFFF);
    g.keys[K_FWD] = true;
    for _ in 0..120 {
        g.update_game(1.0 / 60.0);
    }
    assert!(g.global_time > 0.0);
    assert!(g.player.health < 100, "the lava tile should have bitten");
}
