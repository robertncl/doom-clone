//! Render scale and threads: the framebuffer can be any multiple of the
//! logical 640x400, and the view renders in parallel bands that must agree
//! pixel for pixel with a single-threaded render.

use super::*;
use crate::constants::*;
use crate::render::{STAGE_ALL, STAGE_FLATS, STAGE_SPRITES, STAGE_WALLS};
use crate::textures::{MipTex, Textures};

/// A busy scene on a real level: walls, floor, sky, sprites and a fireball.
fn scene(scale: usize, threads: usize) -> Game {
    let mut g = new_game();
    g.set_scale(scale);
    g.threads = threads;
    g.load_level(0);
    g.show_intro = false;
    g.global_time = 2.5;
    g.player.angle = 0.4;
    g.spawn_fireball(4.5, 1.5, 1.5, 1.5, 0.0, 3.0, 12);
    g.spawn_particle(3.0, 1.6, 0.0, 0.0, 1.0, 0x00FF_FFFF);
    g
}

#[test]
fn a_new_game_renders_at_the_logical_size_on_one_thread() {
    let g = new_game();
    assert_eq!((g.scale, g.threads), (1, 1));
    assert_eq!((g.fb_w, g.fb_h), (SCREEN_W, SCREEN_H));
    assert_eq!(g.pixels.len(), SCREEN_W * SCREEN_H);
}

#[test]
fn set_scale_resizes_every_buffer_and_clamps() {
    let mut g = new_game();
    g.set_scale(2);
    assert_eq!((g.fb_w, g.fb_h), (2 * SCREEN_W, 2 * SCREEN_H));
    assert_eq!(g.pixels.len(), 4 * SCREEN_W * SCREEN_H);
    assert_eq!(g.depth.len(), 2 * SCREEN_W);
    assert_eq!(g.cols.len(), 2 * SCREEN_W);
    assert_eq!((g.wall_top.len(), g.wall_bot.len()), (2 * SCREEN_W, 2 * SCREEN_W));
    g.set_scale(0);
    assert_eq!(g.scale, 1);
    g.set_scale(99);
    assert_eq!(g.scale, MAX_SCALE);
}

#[test]
fn threads_do_not_change_a_single_pixel() {
    for scale in [1, 2] {
        let mut one = scene(scale, 1);
        one.render_frame();
        for threads in [2, 3, 8, 64] {
            let mut many = scene(scale, threads);
            many.render_frame();
            assert!(one.pixels == many.pixels, "{threads} threads at {scale}x drew a different frame");
        }
    }
}

#[test]
fn every_pixel_of_the_view_is_painted_at_every_scale() {
    // Paint the buffer a colour the renderer never produces, then check
    // nothing of it survives: no gaps between bands, rows or columns.
    const MARK: u32 = 0x00FE_01FE;
    for scale in 1..=3 {
        let mut g = scene(scale, 4);
        g.pixels.fill(MARK);
        g.update_lighting();
        g.render_view(STAGE_ALL);
        let left = g.pixels.iter().filter(|&&p| p == MARK).count();
        assert_eq!(left, 0, "{left} pixels unpainted at {scale}x");
    }
}

#[test]
fn a_ray_that_leaves_the_map_still_leaves_no_gap() {
    const MARK: u32 = 0x00FE_01FE;
    let mut g = open_room();
    for row in g.cur_map.iter_mut() {
        row.fill(b'.');
    }
    g.player.x = 8.0;
    g.player.y = 8.0;
    g.pixels.fill(MARK);
    g.update_lighting();
    g.render_view(STAGE_ALL);
    assert_eq!(g.pixels.iter().filter(|&&p| p == MARK).count(), 0);
}

#[test]
fn the_stages_paint_disjoint_parts_that_add_up() {
    const MARK: u32 = 0x00FE_01FE;
    let painted = |stages: u8| {
        let mut g = scene(1, 1);
        g.reset_transients();
        g.pixels.fill(MARK);
        g.update_lighting();
        g.render_view(stages);
        g.pixels.iter().map(|&p| p != MARK).collect::<Vec<bool>>()
    };
    let (walls, flats) = (painted(STAGE_WALLS), painted(STAGE_FLATS));
    assert!(walls.iter().any(|&p| p) && flats.iter().any(|&p| p));
    assert!(walls.iter().zip(&flats).all(|(w, f)| w != f), "walls and flats must tile the frame exactly");
    assert!(painted(0).iter().all(|&p| !p), "no stages, no pixels");
    let _ = STAGE_SPRITES;
}

#[test]
fn a_higher_scale_is_the_same_picture_in_more_detail() {
    // Average each 2x2 block of the 2x frame: it should sit close to the 1x
    // frame (same scene, same lighting), over the 3D view above the HUD bar.
    let mut lo = scene(1, 1);
    lo.render_frame();
    let mut hi = scene(2, 4);
    hi.render_frame();
    let (mut diff, mut n) = (0u64, 0u64);
    for y in 0..SCREEN_H - 60 {
        for x in 0..SCREEN_W {
            let a = lo.pixels[y * SCREEN_W + x];
            for sh in [16, 8, 0] {
                let mut sum = 0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    sum += (hi.pixels[(2 * y + dy) * hi.fb_w + 2 * x + dx] >> sh) & 0xFF;
                }
                diff += ((a >> sh) & 0xFF).abs_diff(sum / 4) as u64;
                n += 1;
            }
        }
    }
    let mean = diff as f64 / n as f64;
    assert!(mean < 6.0, "2x frame differs from 1x by {mean:.1} levels per channel on average");
}

#[test]
fn the_hud_is_drawn_in_logical_pixels_scaled_up() {
    let mut g = new_game();
    g.set_scale(3);
    g.pixels.fill(0);
    g.put_pixel(10, 20, 0x00AB_CDEF);
    assert_eq!(count_color(&g, 0x00AB_CDEF), 9, "one logical pixel is a 3x3 block");
    assert_eq!(g.pixels[(20 * 3) * g.fb_w + 10 * 3], 0x00AB_CDEF);
    assert_eq!(g.pixels[(20 * 3 + 2) * g.fb_w + 10 * 3 + 2], 0x00AB_CDEF);

    g.pixels.fill(0);
    g.fill_rect(SCREEN_W as i32 - 2, SCREEN_H as i32 - 2, 10, 10, 0x0012_3456);
    assert_eq!(count_color(&g, 0x0012_3456), 2 * 2 * 9, "rectangles clip in logical pixels");
    g.put_pixel(-1, 0, 0x00FF_FFFF);
    g.put_pixel(SCREEN_W as i32, 0, 0x00FF_FFFF);
    assert_eq!(count_color(&g, 0x00FF_FFFF), 0);
}

#[test]
fn hud_text_is_the_same_shape_at_every_scale() {
    let ink = |scale: usize| {
        let mut g = new_game();
        g.set_scale(scale);
        g.pixels.fill(0);
        g.draw_text("SCORE 1234", 40, 40, 0x00FF_FFFF);
        count_color(&g, 0x00FF_FFFF)
    };
    let base = ink(1);
    assert!(base > 0);
    assert_eq!(ink(2), 4 * base);
    assert_eq!(ink(3), 9 * base);
}

#[test]
fn the_intro_and_game_over_screens_render_at_2x() {
    let mut g = scene(2, 4);
    g.show_intro = true;
    g.render_frame();
    assert!(count_color(&g, 0x00C0_A040) > 400, "intro border, scaled");

    let mut g = scene(2, 4);
    g.player.health = 0;
    g.score_saved = true;
    g.render_frame();
    // The HUD bar's top rule survives the red tint over the play area.
    let bar_row = (SCREEN_H - 56) * 2;
    assert_eq!(g.pixels[bar_row * g.fb_w + 5], 0x00A0_8060);
}

#[test]
fn sprites_scale_with_the_framebuffer() {
    let covered = |scale: usize| {
        let mut g = open_room();
        g.set_scale(scale);
        g.level = LEVEL_COUNT as i32 - 1;
        g.player.x = 2.5;
        g.player.y = 8.0;
        put_enemy(&mut g, 0, 5.0, 8.0, EN_GRUNT);
        g.render_frame();
        let with = g.pixels.clone();
        g.reset_transients();
        g.render_frame();
        with.iter().zip(&g.pixels).filter(|(a, b)| a != b).count()
    };
    let (one, two) = (covered(1), covered(2));
    assert!(one > 1000);
    let ratio = two as f64 / one as f64;
    assert!((3.4..4.6).contains(&ratio), "a sprite should cover ~4x the pixels at 2x (got {ratio:.2})");
}

#[test]
fn projection_is_in_framebuffer_pixels() {
    let mut g = open_room();
    g.player.x = 2.5;
    g.player.y = 8.0;
    let (x1, d1, h1) = g.project(6.5, 8.0).unwrap();
    g.set_scale(2);
    let (x2, d2, h2) = g.project(6.5, 8.0).unwrap();
    assert_eq!(d1, d2, "depth is in world units");
    assert!((x2 - 2.0 * x1).abs() < 1e-9);
    assert_eq!(h2, 2 * h1);
}

// ---- mipmapped textures ----

#[test]
fn every_texture_has_a_full_mip_chain() {
    let tex = Textures::shared();
    let check = |t: &MipTex| {
        assert_eq!(t.levels.len(), TEX_MIPS);
        for (l, lv) in t.levels.iter().enumerate() {
            let side = TEX_SIZE >> l;
            assert_eq!(lv.len(), side * side);
            assert_eq!(1usize << MipTex::bits(l), side);
        }
    };
    tex.flats.iter().for_each(check);
    tex.wall.iter().flatten().for_each(check);
}

#[test]
fn a_mip_level_is_the_average_of_the_one_above() {
    let mut base = vec![0u32; TEX_SIZE * TEX_SIZE];
    // Left half black, right half white with full emission.
    for y in 0..TEX_SIZE {
        for x in TEX_SIZE / 2..TEX_SIZE {
            base[y * TEX_SIZE + x] = 0xFFFF_FFFF;
        }
    }
    let t = MipTex::from_base(base);
    let last = &t.levels[TEX_MIPS - 1];
    let side = TEX_SIZE >> (TEX_MIPS - 1);
    assert_eq!(last[0], 0, "left stays black");
    assert_eq!(last[side - 1], 0xFFFF_FFFF, "right stays white and glowing");

    // A checkerboard of 0 and 200 averages to 100 one level down.
    let checker: Vec<u32> = (0..TEX_SIZE * TEX_SIZE).map(|i| if (i % TEX_SIZE + i / TEX_SIZE) % 2 == 0 { 0 } else { 0x00C8_C8C8 }).collect();
    let t = MipTex::from_base(checker);
    assert!(t.levels[1].iter().all(|&c| c == 0x0064_6464));
}

#[test]
fn distant_floor_uses_smaller_mips_and_still_renders() {
    // Looking down a long open room: far rows pick coarse mips. The frame
    // must be fully painted and the far floor must not be a flat colour.
    let mut g = open_room();
    g.player.x = 1.5;
    g.player.y = 8.0;
    g.render_frame();
    let row = SCREEN_H / 2 + 12;
    let far: std::collections::HashSet<u32> = (200..440).map(|x| g.pixels[row * SCREEN_W + x]).collect();
    assert!(far.len() > 8, "far floor row has only {} colours", far.len());
}
