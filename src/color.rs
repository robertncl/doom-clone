//! Color packing, value noise, shading, and bilinear texture sampling.
//!
//! Colors are packed as `0x00RRGGBB` (the byte layout minifb expects), exactly
//! matching the C `makeColor`.

use crate::constants::TEX_SIZE;

/// Clamp the three channels to 0..=255 and pack into `0x00RRGGBB`.
#[inline]
pub fn make_color(r: i32, g: i32, b: i32) -> u32 {
    let r = r.clamp(0, 255) as u32;
    let g = g.clamp(0, 255) as u32;
    let b = b.clamp(0, 255) as u32;
    (r << 16) | (g << 8) | b
}

/// 2D integer value-noise hash, returns 0..=255. Uses wrapping unsigned math to
/// reproduce the C overflow behavior exactly.
#[inline]
pub fn hash2(x: i32, y: i32) -> i32 {
    let mut h = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263));
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFF) as i32
}

/// Multiply a packed color's channels by `mul` (clamped at 0).
#[inline]
pub fn shade_color(c: u32, mul: f64) -> u32 {
    let mul = if mul < 0.0 { 0.0 } else { mul };
    let r = (((c >> 16) & 0xFF) as f64 * mul) as i32;
    let g = (((c >> 8) & 0xFF) as f64 * mul) as i32;
    let b = ((c & 0xFF) as f64 * mul) as i32;
    make_color(r, g, b)
}

/// Bilinear texture fetch (wrap-around) **and** distance shading in one integer
/// fixed-point pass. `(u, v)` are in texel units; each `TEX_SIZE` block tiles
/// seamlessly so the wrap blends cleanly. `shade` is `0.0..=1.0`.
///
/// This replaces `shade_color(sample_tex_bilinear(..))` in the hot wall and
/// floor/ceiling loops: the four texels are unpacked once and the result packed
/// once, with no intermediate `f64` color, second clamp, or repack. Sub-texel
/// and shade fractions are carried as 0..=256 fixed-point weights, so the whole
/// blend is integer multiply/shift — the channels stay in range by construction.
#[inline]
pub fn sample_tex_bilinear_shaded(tex: &[u32], u: f64, v: f64, shade: f64) -> u32 {
    let fu = u - 0.5;
    let fv = v - 0.5;
    let u0 = fu.floor();
    let v0 = fv.floor();
    let wu = ((fu - u0) * 256.0) as u32; // sub-texel X, 0..=255
    let wv = ((fv - v0) * 256.0) as u32; // sub-texel Y, 0..=255
    let sh = (shade.clamp(0.0, 1.0) * 256.0) as u32; // 0..=256

    let mask = (TEX_SIZE - 1) as i32;
    let x0 = (u0 as i32 & mask) as usize;
    let x1 = ((u0 as i32 + 1) & mask) as usize;
    let row0 = (v0 as i32 & mask) as usize * TEX_SIZE;
    let row1 = ((v0 as i32 + 1) & mask) as usize * TEX_SIZE;

    let c00 = tex[row0 + x0];
    let c10 = tex[row0 + x1];
    let c01 = tex[row1 + x0];
    let c11 = tex[row1 + x1];

    let iwu = 256 - wu;
    let iwv = 256 - wv;
    // Per channel: blend the two rows in X, blend those in Y (>>16 → 0..=255),
    // then apply shade (* sh >> 8 → 0..=255).
    let blend = |shift: u32| -> u32 {
        let top = ((c00 >> shift) & 0xFF) * iwu + ((c10 >> shift) & 0xFF) * wu;
        let bot = ((c01 >> shift) & 0xFF) * iwu + ((c11 >> shift) & 0xFF) * wu;
        let val = (top * iwv + bot * wv) >> 16;
        (val * sh) >> 8
    };
    (blend(16) << 16) | (blend(8) << 8) | blend(0)
}
