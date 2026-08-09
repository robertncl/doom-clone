//! The raycaster: wall columns, the floor/ceiling cast (both the plain and the
//! lava-aware path), the pain-flash post-process, and whole-frame composition.

use super::*;
use crate::constants::*;

/// Index of the pixel at (x, y).
fn px(g: &Game, x: usize, y: usize) -> u32 {
    g.pixels[y * SCREEN_W + x]
}

#[test]
fn put_pixel_writes_inside_the_frame_and_ignores_everything_outside() {
    let mut g = open_room();
    g.pixels.fill(0);
    g.put_pixel(10, 20, 0x00AB_CDEF);
    assert_eq!(px(&g, 10, 20), 0x00AB_CDEF);

    // Out of bounds in every direction is a silent no-op, not a panic.
    g.put_pixel(-1, 20, 0xFFFFFF);
    g.put_pixel(10, -1, 0xFFFFFF);
    g.put_pixel(SCREEN_W as i32, 20, 0xFFFFFF);
    g.put_pixel(10, SCREEN_H as i32, 0xFFFFFF);
    assert_eq!(count_color(&g, 0x00FF_FFFF), 0);
}

#[test]
fn fill_rect_clips_to_the_frame() {
    let mut g = open_room();
    g.pixels.fill(0);
    g.fill_rect(5, 5, 4, 3, 0x0000_FF00);
    assert_eq!(count_color(&g, 0x0000_FF00), 12);

    // A rect straddling the top-left corner draws only its visible part.
    g.pixels.fill(0);
    g.fill_rect(-2, -2, 4, 4, 0x0000_00FF);
    assert_eq!(count_color(&g, 0x0000_00FF), 4);

    // Entirely off-screen draws nothing.
    g.pixels.fill(0);
    g.fill_rect(SCREEN_W as i32 + 10, 0, 5, 5, 0x00FF_0000);
    g.fill_rect(0, SCREEN_H as i32 + 10, 5, 5, 0x00FF_0000);
    g.fill_rect(-50, 0, 5, 5, 0x00FF_0000);
    assert_eq!(count_color(&g, 0x00FF_0000), 0);
}

#[test]
fn a_wall_column_records_depth_and_paints_a_band() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    g.player.angle = std::f64::consts::PI; // face the west wall, ~1.5 tiles away
    g.pixels.fill(0);

    let dir_x = g.player.angle.cos();
    let dir_y = g.player.angle.sin();
    let plane_half = (FOV / 2.0).tan();
    g.cast_column(SCREEN_W / 2, dir_x, dir_y, -dir_y * plane_half, dir_x * plane_half);

    assert!(g.depth[SCREEN_W / 2] > 0.0, "the column should record a distance");
    assert!(g.depth[SCREEN_W / 2] < 3.0, "and the wall is close");
    let mid = px(&g, SCREEN_W / 2, SCREEN_H / 2);
    assert_ne!(mid, 0, "the wall band should be painted");
}

#[test]
fn a_nearer_wall_reads_as_a_shorter_depth() {
    let mut g = open_room();
    g.player.angle = std::f64::consts::PI;
    let dir_x = g.player.angle.cos();
    let dir_y = g.player.angle.sin();
    let plane_half = (FOV / 2.0).tan();

    g.player.x = 2.0;
    g.player.y = 8.0;
    g.cast_column(SCREEN_W / 2, dir_x, dir_y, -dir_y * plane_half, dir_x * plane_half);
    let near = g.depth[SCREEN_W / 2];

    g.player.x = 10.0;
    g.cast_column(SCREEN_W / 2, dir_x, dir_y, -dir_y * plane_half, dir_x * plane_half);
    let far = g.depth[SCREEN_W / 2];

    assert!(near < far, "backing away should increase the recorded depth");
}

#[test]
fn a_ray_that_escapes_the_map_still_terminates() {
    // Every side open: the DDA has to bail on its iteration cap rather than
    // looping forever.
    let mut g = open_room();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            g.cur_map[y][x] = b'.';
        }
    }
    g.player.x = 8.0;
    g.player.y = 8.0;
    g.render_frame();
    assert!(g.depth.iter().all(|d| d.is_finite()));
}

#[test]
fn both_wall_orientations_get_shaded() {
    // North/south faces are dimmed relative to east/west ones, so a frame that
    // sees a corner should contain a spread of brightnesses.
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;
    g.player.angle = 0.9; // looking diagonally at a corner
    g.render_frame();
    let band: Vec<u32> = (0..SCREEN_W).map(|x| px(&g, x, SCREEN_H / 2)).collect();
    let distinct: std::collections::HashSet<u32> = band.into_iter().collect();
    assert!(distinct.len() > 10, "a diagonal view should not be one flat colour");
}

#[test]
fn the_floor_and_ceiling_are_both_filled() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;
    g.pixels.fill(0);
    g.render_frame();
    // Sample above and below the horizon, away from the HUD bar at the bottom.
    assert_ne!(px(&g, SCREEN_W / 2, SCREEN_H / 2 + 20), 0, "floor is unpainted");
    assert_ne!(px(&g, SCREEN_W / 2, SCREEN_H / 2 - 20), 0, "ceiling is unpainted");
}

#[test]
fn lava_tiles_paint_a_different_floor_than_plain_ground() {
    // Same viewpoint, same geometry — only the floor material differs.
    let plain = {
        let mut g = open_room();
        g.player.x = 8.0;
        g.player.y = 8.0;
        g.render_frame();
        px(&g, SCREEN_W / 2, SCREEN_H / 2 + 40)
    };
    let molten = {
        let mut g = open_room();
        g.player.x = 8.0;
        g.player.y = 8.0;
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                if g.cur_map[y][x] == b'.' {
                    g.cur_map[y][x] = b'~';
                }
            }
        }
        g.has_hazard = true;
        g.render_frame();
        px(&g, SCREEN_W / 2, SCREEN_H / 2 + 40)
    };
    assert_ne!(plain, molten, "the hazard path should sample the lava texture");
    let r = (molten >> 16) & 0xFF;
    let b = molten & 0xFF;
    assert!(r > b, "lava should read hot (r={r}, b={b})");
}

#[test]
fn the_hazard_floor_path_handles_a_view_off_the_edge_of_the_map() {
    // Floor rays run past the map bounds; the cell lookup has to reject those
    // rather than folding them onto tile zero.
    let mut g = open_room();
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            g.cur_map[y][x] = if x == 0 || y == 0 { b'~' } else { b'.' };
        }
    }
    g.has_hazard = true;
    g.player.x = 1.5;
    g.player.y = 1.5;
    for angle in [0.0, 1.5, 3.0, 4.5] {
        g.player.angle = angle;
        g.render_frame(); // must not panic or read out of bounds
    }
    assert!(any_pixel_drawn(&g));
}

#[test]
fn the_pain_flash_reddens_the_frame_and_then_lets_go() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;

    g.pain_flash = 0.0;
    g.render_frame();
    let calm = px(&g, SCREEN_W / 2, SCREEN_H / 2 + 20);

    g.pain_flash = 0.4;
    g.render_frame();
    let hurt = px(&g, SCREEN_W / 2, SCREEN_H / 2 + 20);

    let calm_r = (calm >> 16) & 0xFF;
    let hurt_r = (hurt >> 16) & 0xFF;
    let calm_b = calm & 0xFF;
    let hurt_b = hurt & 0xFF;
    assert!(hurt_r > calm_r, "red should lift under the flash");
    assert!(hurt_b < calm_b, "and the other channels should drop");
}

#[test]
fn a_full_frame_draws_every_layer_of_the_scene() {
    let mut g = open_room();
    g.player.x = 8.0;
    g.player.y = 8.0;
    put_enemy(&mut g, 0, 11.0, 8.0, EN_GRUNT);
    put_pickup(&mut g, 0, 10.0, 8.4, PU_HEALTH);
    put_barrel(&mut g, 0, 9.5, 7.6);
    g.spawn_fireball(10.5, 8.0, 8.0, 8.0, 0.0, 3.0, 12);
    g.spawn_particle(9.0, 8.0, 0.0, 0.0, 1.0, 0x00FF_00FF);
    g.muzzle_flash = 3;

    g.render_frame();
    assert!(any_pixel_drawn(&g));
}

#[test]
fn the_level_clear_banner_shows_between_levels() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    for e in g.enemies.iter_mut() {
        e.alive = false;
    }
    g.render_frame();
    // The banner paints full-width rules in its accent colour.
    assert!(count_color(&g, 0x0040_FF40) > SCREEN_W, "expected the LEVEL CLEAR banner");
}

#[test]
fn the_last_level_shows_no_next_level_banner() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    g.load_level(LEVEL_COUNT - 1);
    for e in g.enemies.iter_mut() {
        e.alive = false;
    }
    g.render_frame();
    assert!(count_color(&g, 0x0040_FF40) < SCREEN_W, "there is no level after the last one");
}

#[test]
fn the_intro_panel_covers_the_scene_on_the_first_frame() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = true;
    g.render_frame();
    // The intro frame is drawn in its border colour.
    assert!(count_color(&g, 0x00C0_A040) > 100, "expected the intro panel border");
}

#[test]
fn the_game_over_overlay_replaces_the_scene() {
    let mut g = open_room();
    g.player.health = 0;
    g.score_saved = true;
    g.final_rank = 1;
    g.render_frame();
    assert!(any_pixel_drawn(&g));
}

#[test]
fn rendering_works_from_inside_every_level() {
    let mut g = super::new_game();
    g.reset_game();
    g.show_intro = false;
    for n in 0..LEVEL_COUNT {
        g.load_level(n);
        for step in 0..4 {
            g.player.angle = step as f64 * std::f64::consts::FRAC_PI_2;
            g.render_frame();
            assert!(any_pixel_drawn(&g), "level {n} rendered an empty frame");
        }
    }
}
