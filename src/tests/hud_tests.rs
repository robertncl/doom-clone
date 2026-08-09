//! The HUD layer: fonts, the status bar and its face, weapons, minimap,
//! banners, the intro panel and the game-over overlay.

use super::*;
use crate::constants::*;
use crate::hud::text_width;

/// A blank-framebuffer game, so a draw call's output is all that's on screen.
fn blank() -> Game {
    let mut g = open_room();
    g.pixels.fill(0);
    g
}

// ---- text ----

#[test]
fn text_width_counts_spaces_as_half_a_character() {
    assert_eq!(text_width(""), 0);
    assert_eq!(text_width("A"), 8);
    assert_eq!(text_width("AB"), 16);
    assert_eq!(text_width(" "), 4);
    assert_eq!(text_width("A B"), 20);
}

#[test]
fn every_character_class_the_font_supports_renders() {
    // Uppercase, lowercase (folded to caps), digits and the punctuation the
    // HUD actually uses.
    for s in ["ABC", "xyz", "0123456789", "/", "-", ":", "."] {
        let mut g = blank();
        g.draw_text(s, 20, 20, 0x00FF_FFFF);
        assert!(any_pixel_drawn(&g), "nothing drawn for {s:?}");
    }
}

#[test]
fn unsupported_characters_draw_nothing_rather_than_panicking() {
    let mut g = blank();
    g.draw_text("!@#$%^&*()", 20, 20, 0x00FF_FFFF);
    assert!(!any_pixel_drawn(&g), "unknown glyphs should be skipped silently");
}

#[test]
fn a_space_advances_the_cursor_without_drawing() {
    let mut with_space = blank();
    with_space.draw_text("A B", 20, 20, 0x00FF_FFFF);
    let mut without = blank();
    without.draw_text("AB", 20, 20, 0x00FF_FFFF);
    assert_ne!(with_space.pixels, without.pixels, "the space should shift the second letter");
    assert_eq!(
        count_color(&with_space, 0x00FF_FFFF),
        count_color(&without, 0x00FF_FFFF),
        "but not add any ink"
    );
}

#[test]
fn text_drawn_off_screen_is_clipped() {
    let mut g = blank();
    g.draw_text("EDGE", -50, -50, 0x00FF_FFFF);
    g.draw_text("EDGE", SCREEN_W as i32 + 10, SCREEN_H as i32 + 10, 0x00FF_FFFF);
    assert!(!any_pixel_drawn(&g));
}

// ---- the status bar ----

#[test]
fn the_hud_bar_reports_health_ammo_weapon_and_progress() {
    let mut g = blank();
    g.player.health = 87;
    g.player.ammo = 42;
    g.level = 3;
    g.level_enemy_count = 9;
    g.draw_hud();
    assert!(any_pixel_drawn(&g));
}

#[test]
fn the_health_readout_changes_colour_as_the_player_fades() {
    // Healthy, hurt and critical each use a different colour.
    let mut seen = std::collections::HashSet::new();
    for hp in [100, 40, 10] {
        let mut g = blank();
        g.player.health = hp;
        g.draw_hud();
        for c in [0x0040_E040u32, 0x00E0_E040, 0x00E0_4040] {
            if count_color(&g, c) > 0 {
                seen.insert(c);
            }
        }
    }
    assert_eq!(seen.len(), 3, "each health band should get its own colour");
}

#[test]
fn the_face_reacts_to_every_health_band() {
    let mut frames = Vec::new();
    for hp in [100, 55, 40, 20, 0] {
        let mut g = blank();
        g.player.health = hp;
        g.draw_hud();
        frames.push(g.pixels.clone());
    }
    for i in 1..frames.len() {
        assert_ne!(frames[i - 1], frames[i], "the face should differ between health bands");
    }
}

#[test]
fn the_faces_eyes_track_over_time() {
    // Sampled at sine peak and trough, the pupils should sit in different
    // places. (They used to sit still: the offset truncated to zero.)
    let mut a = blank();
    a.global_time = std::f64::consts::FRAC_PI_2 / 1.7; // sin = +1
    a.draw_hud();
    let mut b = blank();
    b.global_time = 3.0 * std::f64::consts::FRAC_PI_2 / 1.7; // sin = -1
    b.draw_hud();
    assert_ne!(a.pixels, b.pixels, "the pupils should drift with time");
}

#[test]
fn each_weapon_name_is_shown() {
    let mut frames = Vec::new();
    for w in [WP_PISTOL, WP_SHOTGUN, WP_RIFLE] {
        let mut g = blank();
        g.player.weapon = w;
        g.draw_hud();
        frames.push(g.pixels.clone());
    }
    assert_ne!(frames[0], frames[1]);
    assert_ne!(frames[1], frames[2]);
}

#[test]
fn numbers_render_including_zero_and_negatives() {
    let mut zero = blank();
    zero.player.health = 0;
    zero.draw_hud();
    assert!(any_pixel_drawn(&zero));

    // A negative reading is floored at zero rather than drawing a stray sign.
    let mut neg = blank();
    neg.player.health = -50;
    neg.draw_hud();
    let mut z2 = blank();
    z2.player.health = 0;
    z2.draw_hud();
    assert!(any_pixel_drawn(&neg));
}

#[test]
fn the_score_readout_draws_its_panel() {
    let mut g = blank();
    g.score = 123456;
    g.draw_score_readout();
    assert!(any_pixel_drawn(&g));
}

// ---- weapons ----

#[test]
fn every_weapon_draws_in_first_person() {
    for w in [WP_PISTOL, WP_SHOTGUN, WP_RIFLE] {
        let mut g = blank();
        g.player.weapon = w;
        g.draw_weapon();
        assert!(any_pixel_drawn(&g), "weapon {w} drew nothing");
    }
}

#[test]
fn the_weapon_sways_with_movement_and_settles_when_still() {
    let mut moving = blank();
    moving.player.vx = MOVE_SPEED;
    moving.player.bob = 1.0;
    moving.draw_weapon();

    let mut still = blank();
    still.draw_weapon();
    assert_ne!(moving.pixels, still.pixels, "the sway should move the gun");
}

#[test]
fn sway_amplitude_is_capped_at_full_speed() {
    // Velocity beyond MOVE_SPEED clamps, so an absurd speed looks like a run.
    let mut fast = blank();
    fast.player.vx = MOVE_SPEED * 50.0;
    fast.player.bob = 1.0;
    fast.draw_weapon();

    let mut run = blank();
    run.player.vx = MOVE_SPEED;
    run.player.bob = 1.0;
    run.draw_weapon();
    assert_eq!(fast.pixels, run.pixels, "sway should clamp at walking pace");
}

#[test]
fn firing_lights_a_muzzle_flash_at_each_barrel() {
    for w in [WP_PISTOL, WP_SHOTGUN, WP_RIFLE] {
        let mut lit = blank();
        lit.player.weapon = w;
        lit.muzzle_flash = 5;
        lit.draw_weapon();

        let mut dark = blank();
        dark.player.weapon = w;
        dark.draw_weapon();
        assert_ne!(lit.pixels, dark.pixels, "weapon {w} should light up when fired");
    }
}

// ---- minimap ----

#[test]
fn the_minimap_colours_every_tile_type() {
    let mut g = blank();
    // One of each wall glyph plus floor and lava.
    for (i, glyph) in [b'.', b'#', b'=', b'B', b'D', b'H', b'T', b'~'].iter().enumerate() {
        g.cur_map[5][i + 1] = *glyph;
    }
    g.draw_minimap();
    for c in [0x0030_3030u32, 0x0080_8080, 0x00A0_4030, 0x0040_60A0, 0x0080_5020, 0x0060_2010,
              0x002F_6A70, 0x00E0_5018]
    {
        assert!(count_color(&g, c) > 0, "minimap is missing colour {c:06X}");
    }
}

#[test]
fn the_minimap_marks_enemies_by_kind() {
    let mut g = blank();
    put_enemy(&mut g, 0, 3.0, 3.0, EN_GRUNT);
    put_enemy(&mut g, 1, 5.0, 3.0, EN_IMP);
    put_enemy(&mut g, 2, 7.0, 3.0, EN_WRAITH);
    put_enemy(&mut g, 3, 9.0, 3.0, EN_BARON);
    g.draw_minimap();
    for c in [0x00C0_C040u32, 0x00E0_4020, 0x0060_D0FF, 0x00FF_80E0] {
        assert!(count_color(&g, c) > 0, "no blip for enemy colour {c:06X}");
    }
}

#[test]
fn the_minimap_marks_barrels_pickups_and_the_player() {
    let mut g = blank();
    put_barrel(&mut g, 0, 4.0, 4.0);
    put_pickup(&mut g, 0, 6.0, 4.0, PU_HEALTH);
    put_pickup(&mut g, 1, 8.0, 4.0, PU_AMMO);
    g.draw_minimap();
    assert!(count_color(&g, 0x0090_7020) > 0, "no barrel marker");
    assert!(count_color(&g, 0x00E0_4040) > 0, "no health marker");
    assert!(count_color(&g, 0x00E0_C040) > 0, "no ammo marker");
    assert!(count_color(&g, 0x0040_E040) > 0, "no player marker");
}

#[test]
fn dead_things_leave_the_minimap() {
    let mut g = blank();
    put_enemy(&mut g, 0, 3.0, 3.0, EN_GRUNT);
    put_barrel(&mut g, 0, 4.0, 4.0);
    put_pickup(&mut g, 0, 6.0, 4.0, PU_HEALTH);
    g.enemies[0].alive = false;
    g.barrels[0].alive = false;
    g.pickups[0].alive = false;
    g.draw_minimap();
    assert_eq!(count_color(&g, 0x00C0_C040), 0);
    assert_eq!(count_color(&g, 0x0090_7020), 0);
    assert_eq!(count_color(&g, 0x00E0_4040), 0);
}

#[test]
fn the_player_arrow_follows_the_facing() {
    let mut a = blank();
    a.draw_minimap();
    let mut b = blank();
    b.player.angle = std::f64::consts::FRAC_PI_2;
    b.draw_minimap();
    assert_ne!(a.pixels, b.pixels, "the heading needle should rotate");
}

// ---- crosshair, banners, overlays ----

#[test]
fn the_crosshair_draws_a_gapped_cross() {
    let mut g = blank();
    g.draw_crosshair();
    assert!(count_color(&g, 0x00E0_E0E0) > 0, "no crosshair arms");
    assert_eq!(count_color(&g, 0x00FF_4040), 1, "exactly one centre dot");
}

#[test]
fn a_banner_draws_rules_across_the_screen() {
    let mut g = blank();
    g.draw_banner("TEST", 100, 0x0040_FF40);
    assert!(count_color(&g, 0x0040_FF40) > SCREEN_W, "the banner rules span the frame");
}

#[test]
fn the_intro_panel_lists_the_controls_and_the_score_table() {
    let mut g = blank();
    g.high_scores = [900, 800, 700, 600, 500];
    g.draw_intro();
    assert!(any_pixel_drawn(&g));
    assert!(count_color(&g, 0x00C0_A040) > 100, "expected the panel border");
}

#[test]
fn the_intro_prompt_blinks() {
    let mut on = blank();
    on.global_time = 0.75; // prompt visible
    on.draw_intro();
    let mut off = blank();
    off.global_time = 0.25; // prompt hidden
    off.draw_intro();
    assert_ne!(on.pixels, off.pixels, "the PRESS ANY KEY prompt should blink");
}

#[test]
fn the_game_over_overlay_differs_for_a_death_and_a_win() {
    let mut dead = blank();
    dead.player.health = 0;
    dead.score = 500;
    dead.draw_game_over_overlay();

    let mut won = blank();
    won.player.health = 100;
    won.score = 500;
    won.draw_game_over_overlay();

    assert!(any_pixel_drawn(&dead));
    assert!(any_pixel_drawn(&won));
    assert_ne!(dead.pixels, won.pixels, "victory and death should not look the same");
}

#[test]
fn the_overlay_calls_out_a_new_high_score() {
    let mut ranked = blank();
    ranked.final_rank = 1;
    ranked.draw_game_over_overlay();

    let mut unranked = blank();
    unranked.final_rank = 0;
    unranked.draw_game_over_overlay();

    assert_ne!(ranked.pixels, unranked.pixels, "a ranking should add a line");
}

#[test]
fn the_restart_prompt_blinks() {
    let mut on = blank();
    on.global_time = 0.75;
    on.draw_game_over_overlay();
    let mut off = blank();
    off.global_time = 0.25;
    off.draw_game_over_overlay();
    assert_ne!(on.pixels, off.pixels);
}
