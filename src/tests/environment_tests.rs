//! Level looks: themes, dressing (flats, wall-face variants, decorations),
//! the lightmap and its moving lights, the generated textures and skies, and
//! the cached sprite rasters.

use super::*;
use crate::constants::*;
use crate::level::LEVELS;
use crate::lighting::*;
use crate::sprites::{anim_frame, bank, decor_frames, ENEMY_ANIM, SPR};
use crate::textures::*;
use crate::theme::*;

/// A level's map as the loader leaves it (walls and floor only).
fn level_map(n: usize) -> [[u8; MAP_W]; MAP_H] {
    let mut g = new_game();
    g.load_level(n);
    g.cur_map
}

fn open_map() -> [[u8; MAP_W]; MAP_H] {
    open_room().cur_map
}

// ---- themes ----

#[test]
fn every_level_has_a_theme_and_they_differ() {
    let looks: std::collections::HashSet<_> =
        THEMES.iter().map(|t| (t.floor, t.ceil, t.sky, t.fog)).collect();
    assert!(looks.len() >= 8, "nine levels should not share looks ({} distinct)", looks.len());
    assert!(THEMES.iter().any(|t| t.sky.is_some()), "some levels are outdoors");
    assert!(THEMES.iter().any(|t| t.sky.is_none()), "and some are indoors");
}

#[test]
fn theme_odds_and_counts_stay_in_range() {
    for t in &THEMES {
        let fp: u32 = t.floor_patches.iter().map(|p| p.1 as u32).sum();
        let cp: u32 = t.ceil_patches.iter().map(|p| p.1 as u32).sum();
        let faces = t.light_chance as u32 + t.detail_chance as u32 + t.worn_chance as u32;
        assert!(fp <= 256 && cp <= 256 && faces <= 256, "{} rolls past 256", t.name);
        assert!(t.decor_count <= MAX_DECOR, "{} asks for too many decorations", t.name);
        assert!(t.floor < FLAT_COUNT && t.ceil < FLAT_COUNT);
        assert!(t.sky.is_none_or(|s| s < SKY_COUNT));
        assert!(t.decor.iter().all(|d| d.0 < DEC_KIND_MAX));
        assert!(t.fog_density > 0.0);
    }
}

#[test]
fn an_out_of_range_level_still_gets_a_theme() {
    assert_eq!(theme_for(-3).name, THEMES[0].name);
    assert_eq!(theme_for(99).name, THEMES[LEVEL_COUNT - 1].name);
}

// ---- dressing ----

#[test]
fn building_the_same_level_twice_gives_the_same_look() {
    for n in 0..LEVEL_COUNT {
        let map = level_map(n);
        let (a, b) = (Environment::build(n as i32, &map, &[]), Environment::build(n as i32, &map, &[]));
        assert_eq!(a.floor_flat, b.floor_flat);
        assert_eq!(a.face_variant, b.face_variant);
        assert_eq!(a.lights.len(), b.lights.len());
        assert_eq!(a.decor.len(), b.decor.len());
        assert_eq!(a.lightmap, b.lightmap);
    }
}

#[test]
fn lava_cells_get_the_lava_floor_and_others_their_themes_flats() {
    for n in 0..LEVEL_COUNT {
        let map = level_map(n);
        let env = Environment::build(n as i32, &map, &[]);
        let t = &THEMES[n];
        let allowed: Vec<usize> = std::iter::once(t.floor).chain(t.floor_patches.iter().map(|p| p.0)).collect();
        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let f = env.floor_flat[y * MAP_W + x] as usize;
                if map[y][x] == b'~' {
                    assert_eq!(f, FLAT_LAVA, "level {n} lava at ({x},{y})");
                } else {
                    assert!(allowed.contains(&f), "level {n} cell ({x},{y}) has a foreign flat {f}");
                }
            }
        }
    }
}

#[test]
fn every_level_mixes_in_floor_patches() {
    for n in 0..LEVEL_COUNT {
        let env = Environment::build(n as i32, &level_map(n), &[]);
        let base = THEMES[n].floor as u8;
        let patched = env.floor_flat[..OUTSIDE].iter().filter(|&&f| f != base && f != FLAT_LAVA as u8).count();
        assert!(patched > 0, "level {} has a uniform floor", n + 1);
    }
}

#[test]
fn light_fixtures_sit_on_visible_faces_and_are_spaced_apart() {
    for n in 0..LEVEL_COUNT {
        let map = level_map(n);
        let env = Environment::build(n as i32, &map, &[]);
        let mut fixtures = 0;
        for y in 0..MAP_H as i32 {
            for x in 0..MAP_W as i32 {
                for (f, (dx, dy)) in [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().enumerate() {
                    if env.face_variant[(y as usize) * MAP_W + x as usize][f] as usize == WV_LIGHT {
                        assert!(is_wall(&map, x, y), "a fixture on open floor");
                        assert!(!is_wall(&map, x + dx, y + dy), "a fixture facing into a wall");
                        fixtures += 1;
                    }
                }
            }
        }
        assert!(fixtures > 0, "level {} has no wall lights", n + 1);
        let fixed: Vec<&Light> = env.lights.iter().filter(|l| l.layer != LAYER_LAVA).collect();
        for (i, a) in fixed.iter().enumerate() {
            for b in &fixed[i + 1..] {
                assert!((a.x - b.x).hypot(a.y - b.y) >= 2.4, "level {} has lights on top of each other", n + 1);
            }
        }
    }
}

#[test]
fn decorations_stand_on_open_floor_against_a_wall_and_out_of_the_way() {
    for n in 0..LEVEL_COUNT {
        let map = level_map(n);
        let keep = [(1, 1), (3, 3)];
        let env = Environment::build(n as i32, &map, &keep);
        assert!(!env.decor.is_empty(), "level {} is undecorated", n + 1);
        assert!(env.decor.len() <= THEMES[n].decor_count);
        for (i, d) in env.decor.iter().enumerate() {
            let (cx, cy) = (d.x as i32, d.y as i32);
            assert!(!is_wall(&map, cx, cy) && map[cy as usize][cx as usize] != b'~');
            assert!(!keep.contains(&(cx, cy)), "a prop in a cell that must stay clear");
            assert!([(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dy)| is_wall(&map, cx + dx, cy + dy)));
            assert!(THEMES[n].decor.iter().any(|t| t.0 == d.kind), "a prop the theme didn't ask for");
            for e in &env.decor[i + 1..] {
                assert!((d.x - e.x).hypot(d.y - e.y) >= 2.4);
            }
        }
    }
}

#[test]
fn lit_decorations_bring_their_own_light() {
    let mut total = 0;
    for n in 0..LEVEL_COUNT {
        let env = Environment::build(n as i32, &level_map(n), &[]);
        for d in env.decor.iter().filter(|d| decor_light(d.kind).is_some()) {
            total += 1;
            assert!(env.lights.iter().any(|l| (l.x - d.x).abs() < 1e-9 && (l.y - d.y).abs() < 1e-9));
        }
    }
    assert!(total > 0, "no torches, lamps or braziers anywhere");
}

#[test]
fn the_environment_follows_edits_to_the_map() {
    let mut g = open_room();
    g.render_frame();
    assert_ne!(g.env.floor_flat[5 * MAP_W + 5], FLAT_LAVA as u8);
    g.cur_map[5][5] = b'~';
    g.render_frame();
    assert_eq!(g.env.floor_flat[5 * MAP_W + 5], FLAT_LAVA as u8, "the lava edit should be picked up");
}

// ---- light ----

#[test]
fn line_of_sight_is_blocked_by_walls() {
    let mut map = open_map();
    assert!(line_of_sight(&map, 2.5, 2.5, 12.5, 9.5));
    for y in 0..MAP_H {
        map[y][7] = b'#';
    }
    assert!(!line_of_sight(&map, 2.5, 2.5, 12.5, 9.5), "light must not pass a wall");
    assert!(line_of_sight(&map, 2.5, 2.5, 6.5, 9.5), "nor be blocked on its own side");
    assert!(line_of_sight(&map, 6.5, 3.5, 6.5, 3.5), "a point sees itself");
}

#[test]
fn corners_are_darker_than_open_floor() {
    let map = open_map();
    let mid = ambient_occlusion(&map, 8.5, 8.5);
    let edge = ambient_occlusion(&map, 1.05, 8.5);
    let corner = ambient_occlusion(&map, 1.05, 1.05);
    assert_eq!(mid, 1.0);
    assert!(edge < mid && corner < edge, "mid {mid}, edge {edge}, corner {corner}");
}

#[test]
fn a_light_brightens_what_it_can_see_and_not_beyond_a_wall() {
    // Level 3's catacombs are dark, so a torch's pool stands out clearly.
    let mut g = new_game();
    g.load_level(2);
    g.update_lighting();
    let l = *g.env.lights.iter().find(|l| l.layer != LAYER_LAVA).expect("a light");
    let near = g.env.light_at(l.x, l.y)[0];
    let far = g.env.light_at(l.x + 7.0, l.y).min(g.env.light_at(l.x - 7.0, l.y))[0];
    assert!(near > far, "light should fall off ({near} vs {far})");
    assert!(near > (THEMES[2].ambient[0] * LIGHT_ONE as f64) as i32);
}

#[test]
fn light_lookups_off_the_map_are_clamped() {
    let mut g = open_room();
    g.update_lighting();
    for (x, y) in [(-5.0, -5.0), (99.0, 3.0), (3.0, 99.0), (f64::MAX, f64::MIN)] {
        let l = g.env.light_at(x, y);
        assert!(l.iter().all(|&c| (0..=LIGHT_MAX).contains(&c)));
    }
}

#[test]
fn flames_flicker_but_steady_light_does_not() {
    let mut g = new_game();
    g.load_level(2);
    g.global_time = 1.0;
    g.update_lighting();
    let a = g.env.flicker;
    g.global_time = 1.37;
    g.update_lighting();
    let b = g.env.flicker;
    assert_eq!(a[LAYER_STEADY], 1.0);
    assert_eq!(b[LAYER_STEADY], 1.0);
    assert_ne!(a[1], b[1], "flames should move");
    for t in 0..200 {
        let f = flame_flicker(t as f64 * 0.137, 2);
        assert!((0.6..=1.0).contains(&f), "flicker {f} out of range");
    }
}

#[test]
fn the_muzzle_flash_lights_up_the_room() {
    let mut g = open_room();
    g.player.x = 8.5;
    g.player.y = 8.5;
    g.update_lighting();
    let dark = g.env.light_at(8.5, 8.5);
    g.muzzle_flash = 5;
    g.update_lighting();
    let lit = g.env.light_at(8.5, 8.5);
    assert!(lit[0] > dark[0], "firing should light the player's surroundings");
}

#[test]
fn fireballs_and_explosions_cast_light() {
    let mut g = open_room();
    g.update_lighting();
    let before = g.env.light_at(10.5, 10.5)[0];
    g.spawn_fireball(10.5, 10.5, 2.5, 2.5, 0.0, 3.0, 12);
    g.update_lighting();
    assert!(g.env.light_at(10.5, 10.5)[0] > before, "a fireball glows");

    let mut g = open_room();
    g.update_lighting();
    let before = g.env.light_at(5.5, 9.5)[0];
    put_barrel(&mut g, 0, 5.5, 9.5);
    g.explode_barrel(0);
    g.update_lighting();
    let burst = g.env.light_at(5.5, 9.5)[0];
    assert!(burst > before, "an explosion lights the room");
    for _ in 0..60 {
        g.update_game(1.0 / 60.0);
    }
    g.update_lighting();
    assert!(g.env.light_at(5.5, 9.5)[0] < burst, "and then fades");
}

#[test]
fn a_new_flash_replaces_the_faintest_one() {
    let mut g = open_room();
    for i in 0..MAX_FLASHES {
        g.add_flash(i as f64, 1.0);
        g.flashes[i].life = 0.1 * (i + 1) as f64;
    }
    g.add_flash(9.0, 9.0);
    assert_eq!((g.flashes[0].x, g.flashes[0].life), (9.0, 0.6), "slot 0 was the faintest");
}

// ---- textures and skies ----

fn emission(t: &Tex) -> usize {
    t.iter().filter(|&&c| c >> 24 > 0).count()
}

#[test]
fn glowing_surfaces_carry_emission_and_plain_ones_do_not() {
    let tex = Textures::shared();
    assert_eq!(tex.flats.len(), FLAT_COUNT);
    assert!(emission(&tex.flats[FLAT_LAVA]) > TEX_SIZE * TEX_SIZE / 2, "lava glows");
    assert!(emission(&tex.flats[FLAT_CEIL_LIGHT]) > 0, "ceiling lights glow");
    assert_eq!(emission(&tex.flats[FLAT_TILE]), 0);
    assert_eq!(emission(&tex.flats[FLAT_COBBLE]), 0);
    for kind in 1..WALL_KIND_MAX {
        assert!(emission(&tex.wall[kind][WV_LIGHT]) > 0, "wall kind {kind}'s fixture should glow");
        assert_ne!(tex.wall[kind][WV_DETAIL][..], tex.wall[kind][WV_BASE][..], "kind {kind} detail");
        assert_ne!(tex.wall[kind][WV_WORN][..], tex.wall[kind][WV_BASE][..], "kind {kind} worn");
    }
    assert_eq!(emission(&tex.wall[WALL_STONE][WV_BASE]), 0);
}

#[test]
fn floor_patches_join_their_base_seamlessly() {
    let tex = Textures::shared();
    for (patch, base) in [(FLAT_TILE_BLOOD, FLAT_TILE), (FLAT_COBBLE_MOSS, FLAT_COBBLE), (FLAT_DIRT_BONES, FLAT_DIRT)] {
        let (p, b) = (&tex.flats[patch], &tex.flats[base]);
        for i in 0..TEX_SIZE {
            for (x, y) in [(i, 0), (0, i), (i, TEX_SIZE - 1), (TEX_SIZE - 1, i)] {
                assert_eq!(p[y * TEX_SIZE + x], b[y * TEX_SIZE + x], "flat {patch} differs at its edge ({x},{y})");
            }
        }
    }
}

#[test]
fn every_sky_is_built_and_distinct() {
    let skies: Vec<&SkyTex> = (0..SKY_COUNT).map(sky).collect();
    for (i, a) in skies.iter().enumerate() {
        assert!(a.iter().any(|&c| c != a[0]), "sky {i} is one flat colour");
        for b in &skies[i + 1..] {
            assert_ne!(a[..], b[..]);
        }
    }
    assert!(std::ptr::eq(sky(SKY_DUSK), sky(SKY_DUSK)), "built once and shared");
    assert!(std::ptr::eq(sky(99), sky(SKY_COUNT - 1)), "an unknown id clamps");
}

// ---- rendering with the environment ----

#[test]
fn the_sky_stays_put_as_you_walk() {
    // Outdoor level 1: stepping forward moves the floor but not the sky.
    let mut g = new_game();
    g.load_level(0);
    g.show_intro = false;
    g.player.angle = 0.3;
    g.render_frame();
    let (x, y) = (SCREEN_W / 3, 30);
    let sky_before = g.pixels[y * SCREEN_W + x];
    let floor_before = g.pixels[(SCREEN_H - 70) * SCREEN_W + x];
    g.player.x += 0.15;
    g.render_frame();
    assert_eq!(g.pixels[y * SCREEN_W + x], sky_before, "the sky is at infinity");
    assert_ne!(g.pixels[(SCREEN_H - 70) * SCREEN_W + x], floor_before);
}

#[test]
fn fog_thins_visibility_with_distance() {
    let g = open_room();
    let (near, mid, far) = (g.fog_visibility(0.0), g.fog_visibility(5.0), g.fog_visibility(20.0));
    assert_eq!(near, 256);
    assert!(near > mid && mid > far);
}

#[test]
fn the_gun_takes_the_light_of_the_room() {
    let gun = |light: i32| {
        let mut g = open_room();
        g.pixels.fill(0);
        g.gun_light = [light; 3];
        g.draw_weapon();
        g.pixels.iter().map(|&p| (p >> 16) & 0xFF).sum::<u32>()
    };
    assert!(gun(100) < gun(300), "a dark room should darken the gun");
}

// ---- sprite cache ----

#[test]
fn animation_frames_wrap_around_the_cycle() {
    assert_eq!(anim_frame(1.0, 8, 0.0), 0);
    assert_eq!(anim_frame(1.0, 8, 0.99), 7);
    assert_eq!(anim_frame(1.0, 8, 1.0), 0, "a full period comes back to the start");
    assert_eq!(anim_frame(1.0, 8, -0.01), 7, "negative phases wrap too");
}

#[test]
fn every_cached_sprite_has_pixels_and_full_mip_chains() {
    let b = bank();
    let check = |r: &crate::sprites::Raster, what: &str| {
        assert_eq!(r.levels.len(), 4, "{what} mips");
        for (l, lv) in r.levels.iter().enumerate() {
            assert_eq!(lv.len(), (SPR >> l) * (SPR >> l), "{what} level {l} size");
        }
        assert!(r.levels[0].iter().any(|&p| p >> 24 > 0), "{what} is empty");
    };
    for (k, frames) in b.enemies.iter().enumerate() {
        assert_eq!(frames.len(), ENEMY_ANIM[k].1);
        frames.iter().for_each(|f| check(f, "enemy"));
    }
    b.pickups.iter().for_each(|p| check(p, "pickup"));
    check(&b.barrel, "barrel");
    for (k, frames) in b.decor.iter().enumerate() {
        assert_eq!(frames.len(), decor_frames(k));
        frames.iter().for_each(|f| check(f, "decoration"));
    }
    assert_ne!(b.decor[DEC_TORCH][0].levels[0], b.decor[DEC_TORCH][4].levels[0], "torch flames animate");
}

#[test]
fn decorations_appear_in_the_view() {
    // Level 1 from its spawn, turned round until a prop is in view: the frame
    // with props differs from one where they are removed.
    let mut g = new_game();
    g.load_level(0);
    g.show_intro = false;
    let mut seen = false;
    for k in 0..16 {
        g.player.angle = k as f64 * std::f64::consts::TAU / 16.0;
        g.render_frame();
        let with = g.pixels.clone();
        let saved = std::mem::take(&mut g.env.decor);
        g.render_frame();
        g.env.decor = saved;
        if with != g.pixels {
            seen = true;
            break;
        }
    }
    assert!(seen, "no decoration was ever drawn");
}

#[test]
fn every_level_renders_its_sky_or_ceiling_and_fixtures() {
    for n in 0..LEVEL_COUNT {
        let mut g = new_game();
        g.load_level(n);
        g.show_intro = false;
        g.render_frame();
        assert!(any_pixel_drawn(&g));
        assert_eq!(g.env.sky, THEMES[n].sky);
        let _ = LEVELS[n]; // every level index is real
    }
}
