//! Colour packing, hashing, shading and the bilinear texture samplers.

use crate::color::*;
use crate::constants::TEX_SIZE;
use crate::textures::Tex;

#[test]
fn make_color_packs_rgb_and_clamps_out_of_range_channels() {
    assert_eq!(make_color(0x12, 0x34, 0x56), 0x0012_3456);
    assert_eq!(make_color(255, 255, 255), 0x00FF_FFFF);
    assert_eq!(make_color(-40, -1, -999), 0, "negatives clamp to zero");
    assert_eq!(make_color(999, 300, 256), 0x00FF_FFFF, "overbright clamps to white");
    assert_eq!(make_color(0, 0, 0), 0);
}

#[test]
fn hash2_is_a_stable_byte_valued_hash() {
    for (x, y) in [(0, 0), (1, 0), (-5, 7), (99999, -99999)] {
        let v = hash2(x, y);
        assert!((0..=255).contains(&v), "hash2({x},{y}) = {v} is out of byte range");
        assert_eq!(v, hash2(x, y), "the same input must give the same output");
    }
    // Neighbouring cells should not collide wholesale.
    let distinct: std::collections::HashSet<i32> =
        (0..64).map(|i| hash2(i, i * 7 + 1)).collect();
    assert!(distinct.len() > 20, "hash spread looks degenerate");
}

#[test]
fn shade_color_scales_channels_and_floors_at_black() {
    assert_eq!(shade_color(0x00FF_FFFF, 1.0), 0x00FF_FFFF);
    assert_eq!(shade_color(0x00FF_FFFF, 0.0), 0);
    assert_eq!(shade_color(0x00FF_FFFF, -3.0), 0, "a negative multiplier clamps to black");
    let half = shade_color(0x0080_8080, 0.5);
    assert_eq!(half, make_color(64, 64, 64));
}

#[test]
fn shade_fp_maps_a_unit_shade_onto_the_fixed_point_range() {
    assert_eq!(shade_fp(0.0), 0);
    assert_eq!(shade_fp(1.0), 256);
    assert_eq!(shade_fp(0.5), 128);
    assert_eq!(shade_fp(-1.0), 0, "clamped low");
    assert_eq!(shade_fp(5.0), 256, "clamped high");
}

/// A texture that is a flat colour, so sampling is easy to reason about.
fn flat_tex(c: u32) -> Box<Tex> {
    vec![c; TEX_SIZE * TEX_SIZE].into_boxed_slice().try_into().unwrap()
}

#[test]
fn sampling_a_flat_texture_returns_that_colour_shaded() {
    let tex = flat_tex(0x0080_8080);
    let full = sample_tex_bilinear_shaded(&tex, 10.0, 10.0, shade_fp(1.0));
    assert_eq!(full, 0x0080_8080, "unshaded sampling is the texel itself");
    let dark = sample_tex_bilinear_shaded(&tex, 10.0, 10.0, shade_fp(0.5));
    assert_eq!(dark, make_color(64, 64, 64));
    let black = sample_tex_bilinear_shaded(&tex, 10.0, 10.0, 0);
    assert_eq!(black, 0);
}

#[test]
fn sampling_wraps_around_the_texture_edges() {
    let tex = flat_tex(0x0040_5060);
    // Way outside 0..TEX_SIZE, in both directions — the sampler masks, so these
    // must all land on a real texel rather than panicking or reading garbage.
    for (u, v) in [(-5.0, -5.0), (0.0, 0.0), (1e4, -1e4), (TEX_SIZE as f64 + 0.5, 3.5)] {
        assert_eq!(sample_tex_bilinear_shaded(&tex, u, v, shade_fp(1.0)), 0x0040_5060);
    }
}

#[test]
fn bilinear_blending_lands_between_two_texel_colours() {
    // Left half black, right half white: sampling across the seam should give
    // an intermediate value rather than snapping to one side.
    let mut buf = vec![0u32; TEX_SIZE * TEX_SIZE];
    for y in 0..TEX_SIZE {
        for x in 0..TEX_SIZE {
            buf[y * TEX_SIZE + x] = if x < TEX_SIZE / 2 { 0 } else { 0x00FF_FFFF };
        }
    }
    let tex: Box<Tex> = buf.into_boxed_slice().try_into().unwrap();
    let mid = TEX_SIZE as f64 / 2.0;
    let blended = sample_tex_bilinear_shaded(&tex, mid, 10.0, shade_fp(1.0));
    let r = (blended >> 16) & 0xFF;
    assert!((1..255).contains(&r), "expected a blend across the seam, got {r}");
}

#[test]
fn the_column_sampler_agrees_with_the_general_one() {
    let tex = flat_tex(0x0011_2233);
    let sh = shade_fp(0.75);
    let general = sample_tex_bilinear_shaded(&tex, 8.5, 12.25, sh);
    // Same texel pair the wall loop would hoist for u = 8.5.
    let col = sample_tex_bilinear_col(&tex, 8, 9, 0, 12.25, sh);
    assert_eq!(general, col);
}

#[test]
fn the_column_sampler_masks_indices_into_range() {
    let tex = flat_tex(0x00AA_BBCC);
    // Deliberately out-of-range x indices: the sampler re-masks them.
    let c = sample_tex_bilinear_col(&tex, TEX_SIZE + 3, TEX_SIZE + 4, 128, 5.0, shade_fp(1.0));
    assert_eq!(c, 0x00AA_BBCC);
}
