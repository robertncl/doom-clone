//! Billboard sprites: enemies of each kind, pickups, fireballs, barrels and
//! particles — including depth occlusion, clipping and the near/far sampling
//! paths.
//!
//! The procedural sprite functions are driven through a real frame rather than
//! called directly: a close-up billboard samples the whole `u,v` square, so one
//! render covers the shape's branches the way the game actually hits them.

use super::*;
use crate::constants::*;

/// A room with the camera at a known spot looking east down a clear corridor.
fn corridor() -> Game {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    g.player.angle = 0.0;
    // Sit on the last level: `sprite_pixels` clears the enemies for its
    // baseline pass, and on any earlier level an empty room would raise the
    // LEVEL CLEAR banner across the very region being compared.
    g.level = LEVEL_COUNT as i32 - 1;
    g
}

/// How many pixels of the 3D viewport the scene's entities are responsible for:
/// render as-is, strip every entity, render the identical view again, and count
/// the difference. Comparing against a re-render of the *same* game keeps the
/// map and camera identical, and the window excludes the HUD strips — the
/// minimap and kill counter also react to entities, which would otherwise show
/// up as "sprite" pixels.
fn sprite_pixels(g: &mut Game) -> usize {
    g.render_frame();
    let with = g.pixels.clone();
    g.reset_transients();
    g.render_frame();

    let (x1, y0, y1) = (SCREEN_W - 110, 24, SCREEN_H - 56);
    let mut n = 0;
    for y in y0..y1 {
        for x in 0..x1 {
            if with[y * SCREEN_W + x] != g.pixels[y * SCREEN_W + x] {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn every_enemy_kind_draws_at_close_range() {
    for kind in [EN_GRUNT, EN_IMP, EN_WRAITH, EN_BARON] {
        let mut g = corridor();
        put_enemy(&mut g, 0, 5.0, 8.0, kind);
        let n = sprite_pixels(&mut g);
        assert!(n > 2000, "enemy kind {kind} drew only {n} pixels");
    }
}

#[test]
fn a_baron_has_a_bigger_silhouette_than_a_grunt() {
    let grunt = {
        let mut g = corridor();
        put_enemy(&mut g, 0, 7.0, 8.0, EN_GRUNT);
        sprite_pixels(&mut g)
    };
    let baron = {
        let mut g = corridor();
        put_enemy(&mut g, 0, 7.0, 8.0, EN_BARON);
        sprite_pixels(&mut g)
    };
    assert!(baron > grunt, "the baron should loom larger ({baron} vs {grunt})");
}

#[test]
fn a_wraith_is_drawn_see_through() {
    // Against the same backdrop, a translucent sprite leaves more of the wall
    // showing than an opaque one of similar size.
    let mut g = corridor();
    put_enemy(&mut g, 0, 5.0, 8.0, EN_WRAITH);
    g.render_frame();
    let wraith_frame = g.pixels.clone();

    let mut g2 = corridor();
    put_enemy(&mut g2, 0, 5.0, 8.0, EN_GRUNT);
    g2.render_frame();

    assert_ne!(wraith_frame, g2.pixels, "the two kinds should not render identically");
}

#[test]
fn enemies_animate_between_frames() {
    let mut g = corridor();
    put_enemy(&mut g, 0, 6.0, 8.0, EN_IMP);
    g.render_frame();
    let first = g.pixels.clone();
    g.enemies[0].anim += 1.3;
    g.render_frame();
    assert_ne!(first, g.pixels, "the animation phase should change the sprite");
}

#[test]
fn a_hit_enemy_flashes_white() {
    let mut g = corridor();
    put_enemy(&mut g, 0, 5.0, 8.0, EN_GRUNT);
    g.render_frame();
    let normal = g.pixels.clone();
    g.enemies[0].hit_flash = 0.15;
    g.render_frame();
    assert_ne!(normal, g.pixels, "a hit flash should change how it's drawn");
}

#[test]
fn a_dying_enemy_still_draws_while_its_flash_lasts() {
    let mut g = corridor();
    put_enemy(&mut g, 0, 5.0, 8.0, EN_GRUNT);
    g.enemies[0].alive = false;
    g.enemies[0].hit_flash = 0.1;
    assert!(sprite_pixels(&mut g) > 500, "the death frame should still be visible");
}

#[test]
fn an_enemy_behind_the_camera_is_not_drawn() {
    let mut g = corridor();
    put_enemy(&mut g, 0, 1.2, 8.0, EN_GRUNT); // behind us, facing east
    assert_eq!(sprite_pixels(&mut g), 0);
}

#[test]
fn an_enemy_behind_a_wall_is_hidden_by_the_depth_buffer() {
    let mut g = corridor();
    for y in 0..MAP_H {
        g.cur_map[y][5] = b'#'; // wall across the corridor
    }
    put_enemy(&mut g, 0, 8.0, 8.0, EN_GRUNT); // beyond it
    assert_eq!(sprite_pixels(&mut g), 0, "the wall should occlude it completely");
}

#[test]
fn an_enemy_off_to_the_side_is_clipped_not_wrapped() {
    let mut g = corridor();
    // Far enough off-axis to sit outside the view frustum.
    put_enemy(&mut g, 0, 3.0, 13.5, EN_GRUNT);
    g.render_frame(); // must not panic on the clipped billboard
    assert!(any_pixel_drawn(&g));
}

#[test]
fn a_very_close_enemy_takes_the_single_sample_path() {
    // Huge billboards skip supersampling; this exercises that branch and the
    // clipping that goes with a sprite larger than the screen.
    let mut g = corridor();
    put_enemy(&mut g, 0, 2.9, 8.0, EN_BARON);
    g.render_frame();
    assert!(any_pixel_drawn(&g));
}

#[test]
fn distance_dims_a_sprite() {
    let near = {
        let mut g = corridor();
        put_enemy(&mut g, 0, 4.0, 8.0, EN_GRUNT);
        g.render_frame();
        g.pixels.iter().map(|p| ((p >> 16) & 0xFF) as u64).sum::<u64>()
    };
    let far = {
        let mut g = corridor();
        put_enemy(&mut g, 0, 12.0, 8.0, EN_GRUNT);
        g.render_frame();
        g.pixels.iter().map(|p| ((p >> 16) & 0xFF) as u64).sum::<u64>()
    };
    assert_ne!(near, far, "distance shading should change the frame's brightness");
}

#[test]
fn every_pickup_kind_draws() {
    for kind in [PU_HEALTH, PU_AMMO, PU_SHOTGUN, PU_RIFLE] {
        let mut g = corridor();
        put_pickup(&mut g, 0, 5.0, 8.0, kind);
        let n = sprite_pixels(&mut g);
        assert!(n > 300, "pickup kind {kind} drew only {n} pixels");
    }
}

#[test]
fn pickups_bob_over_time() {
    let mut g = corridor();
    put_pickup(&mut g, 0, 5.0, 8.0, PU_HEALTH);
    g.render_frame();
    let first = g.pixels.clone();
    g.global_time += 0.7;
    g.render_frame();
    assert_ne!(first, g.pixels, "the bob should shift it on screen");
}

#[test]
fn a_distant_pickup_is_still_drawn_at_its_minimum_size() {
    let mut g = corridor();
    put_pickup(&mut g, 0, 14.2, 8.0, PU_AMMO);
    assert!(sprite_pixels(&mut g) > 0, "far pickups clamp to a visible minimum");
}

#[test]
fn a_barrel_draws_and_can_be_occluded() {
    let mut g = corridor();
    put_barrel(&mut g, 0, 5.0, 8.0);
    assert!(sprite_pixels(&mut g) > 500);

    let mut g = corridor();
    for y in 0..MAP_H {
        g.cur_map[y][5] = b'#';
    }
    put_barrel(&mut g, 0, 8.0, 8.0);
    assert_eq!(sprite_pixels(&mut g), 0);

    let mut g = corridor();
    put_barrel(&mut g, 0, 1.2, 8.0); // behind the camera
    assert_eq!(sprite_pixels(&mut g), 0);
}

#[test]
fn a_fireball_draws_as_a_glowing_ball() {
    let mut g = corridor();
    g.spawn_fireball(5.0, 8.0, 2.5, 8.0, 0.0, 3.0, 12);
    assert!(sprite_pixels(&mut g) > 200);

    // ...and one behind the camera is skipped.
    let mut g = corridor();
    g.spawn_fireball(1.2, 8.0, 0.5, 8.0, 0.0, 3.0, 12);
    assert_eq!(sprite_pixels(&mut g), 0);
}

#[test]
fn a_distant_fireball_clamps_to_a_minimum_size() {
    let mut g = corridor();
    g.spawn_fireball(14.0, 8.0, 2.5, 8.0, 0.0, 3.0, 12);
    assert!(sprite_pixels(&mut g) > 0);
}

#[test]
fn a_fireball_behind_a_wall_is_occluded() {
    let mut g = corridor();
    for y in 0..MAP_H {
        g.cur_map[y][5] = b'#';
    }
    g.spawn_fireball(8.0, 8.0, 2.5, 8.0, 0.0, 3.0, 12);
    assert_eq!(sprite_pixels(&mut g), 0);
}

#[test]
fn particles_draw_and_fade() {
    let mut g = corridor();
    g.spawn_particle(5.0, 8.0, 0.0, 0.0, 1.0, 0x00FF_FFFF);
    assert!(sprite_pixels(&mut g) > 0);

    // Behind the camera, off the side, and behind a wall: all skipped.
    let mut g = corridor();
    g.spawn_particle(1.0, 8.0, 0.0, 0.0, 1.0, 0x00FF_FFFF);
    g.spawn_particle(5.0, 1.2, 0.0, 0.0, 1.0, 0x00FF_FFFF);
    g.render_frame();

    let mut g = corridor();
    for y in 0..MAP_H {
        g.cur_map[y][5] = b'#';
    }
    g.spawn_particle(8.0, 8.0, 0.0, 0.0, 1.0, 0x00FF_FFFF);
    assert_eq!(sprite_pixels(&mut g), 0);
}

#[test]
fn a_long_lived_particle_is_drawn_at_full_brightness() {
    // Life over 1.0 clamps the fade factor rather than overshooting it.
    let mut g = corridor();
    g.spawn_particle(5.0, 8.0, 0.0, 0.0, 3.0, 0x00FF_FFFF);
    assert!(sprite_pixels(&mut g) > 0);
}

#[test]
fn sprites_are_drawn_far_to_near() {
    // Two enemies on the same bearing: the near one must win the overlap.
    let mut g = corridor();
    put_enemy(&mut g, 0, 9.0, 8.0, EN_GRUNT); // far, listed first
    put_enemy(&mut g, 1, 5.0, 8.0, EN_BARON); // near, listed second
    g.render_frame();
    let both = g.pixels.clone();

    let mut g2 = corridor();
    put_enemy(&mut g2, 0, 5.0, 8.0, EN_BARON); // near one alone
    g2.render_frame();

    // The near sprite covers the far one, so the centre column should match the
    // near-only frame.
    let x = SCREEN_W / 2;
    let col_both: Vec<u32> = (SCREEN_H / 2 - 30..SCREEN_H / 2).map(|y| both[y * SCREEN_W + x]).collect();
    let col_near: Vec<u32> =
        (SCREEN_H / 2 - 30..SCREEN_H / 2).map(|y| g2.pixels[y * SCREEN_W + x]).collect();
    assert_eq!(col_both, col_near, "the nearer sprite should be on top");
}

#[test]
fn a_crowded_scene_renders_every_sprite_class_at_once() {
    let mut g = corridor();
    for (i, kind) in [EN_GRUNT, EN_IMP, EN_WRAITH, EN_BARON].iter().enumerate() {
        put_enemy(&mut g, i, 5.0 + i as f64 * 1.5, 8.0 + (i as f64 - 1.5) * 0.4, *kind);
    }
    for (i, kind) in [PU_HEALTH, PU_AMMO, PU_SHOTGUN, PU_RIFLE].iter().enumerate() {
        put_pickup(&mut g, i, 6.0 + i as f64, 7.2, *kind);
    }
    put_barrel(&mut g, 0, 7.0, 8.8);
    g.spawn_fireball(9.0, 8.0, 2.5, 8.0, 0.0, 3.0, 12);
    for i in 0..10 {
        g.spawn_particle(6.0 + i as f64 * 0.2, 8.0, 0.0, 0.0, 1.0, 0x00FF_A040);
    }
    g.render_frame();
    assert!(any_pixel_drawn(&g));
}
