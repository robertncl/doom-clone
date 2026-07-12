//! Color packing, value noise, shading, and bilinear texture sampling.
//!
//! Colors are packed as `0x00RRGGBB` (the byte layout minifb expects), exactly
//! matching the C `makeColor`.

use crate::constants::TEX_SIZE;
use crate::textures::Tex;

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

/// Convert a `0.0..=1.0` shade to the `0..=256` fixed-point factor the
/// samplers take. Hoisted out of the pixel loops: the shade is constant per
/// wall column / floor row, so the clamp + float multiply happens once there.
#[inline]
pub fn shade_fp(shade: f64) -> u32 {
    (shade.clamp(0.0, 1.0) * 256.0) as u32
}

/// Bilinear blend + shade of four texels with `0..=256` fixed-point weights.
/// Per channel: blend the two rows in X, blend those in Y (>>16 → 0..=255),
/// then apply shade (* sh >> 8 → 0..=255). The three channels run the same
/// instruction sequence, which LLVM auto-vectorizes — resist the temptation to
/// pack channels into wider lanes by hand; measured slower here.
#[inline]
fn bilinear_finish(c00: u32, c10: u32, c01: u32, c11: u32, wu: u32, wv: u32, sh: u32) -> u32 {
    let iwu = 256 - wu;
    let iwv = 256 - wv;
    let blend = |shift: u32| -> u32 {
        let top = ((c00 >> shift) & 0xFF) * iwu + ((c10 >> shift) & 0xFF) * wu;
        let bot = ((c01 >> shift) & 0xFF) * iwu + ((c11 >> shift) & 0xFF) * wu;
        let val = (top * iwv + bot * wv) >> 16;
        (val * sh) >> 8
    };
    (blend(16) << 16) | (blend(8) << 8) | blend(0)
}

/// Bilinear texture fetch (wrap-around) **and** distance shading in one integer
/// fixed-point pass. `(u, v)` are in texel units; each `TEX_SIZE` block tiles
/// seamlessly so the wrap blends cleanly. `sh` is a [`shade_fp`] factor.
#[inline]
pub fn sample_tex_bilinear_shaded(tex: &Tex, u: f64, v: f64, sh: u32) -> u32 {
    let fu = u - 0.5;
    let fv = v - 0.5;
    let u0 = fu.floor();
    let v0 = fv.floor();
    let wu = ((fu - u0) * 256.0) as u32; // sub-texel X, 0..=255
    let wv = ((fv - v0) * 256.0) as u32; // sub-texel Y, 0..=255

    let mask = (TEX_SIZE - 1) as i32;
    let x0 = (u0 as i32 & mask) as usize;
    let x1 = ((u0 as i32 + 1) & mask) as usize;
    let row0 = (v0 as i32 & mask) as usize * TEX_SIZE;
    let row1 = ((v0 as i32 + 1) & mask) as usize * TEX_SIZE;

    bilinear_finish(tex[row0 + x0], tex[row0 + x1], tex[row1 + x0], tex[row1 + x1], wu, wv, sh)
}

/// Wall-column variant: the horizontal texel pair and X weight are constant
/// down a whole column, so the caller hoists them and only `v` varies here.
/// `x0`/`x1` are re-masked so the compiler can prove the fetches in bounds.
#[inline]
pub fn sample_tex_bilinear_col(tex: &Tex, x0: usize, x1: usize, wu: u32, v: f64, sh: u32) -> u32 {
    let x0 = x0 & (TEX_SIZE - 1);
    let x1 = x1 & (TEX_SIZE - 1);
    let fv = v - 0.5;
    let v0 = fv.floor();
    let wv = ((fv - v0) * 256.0) as u32;
    let mask = (TEX_SIZE - 1) as i32;
    let row0 = (v0 as i32 & mask) as usize * TEX_SIZE;
    let row1 = ((v0 as i32 + 1) & mask) as usize * TEX_SIZE;
    bilinear_finish(tex[row0 + x0], tex[row0 + x1], tex[row1 + x0], tex[row1 + x1], wu, wv, sh)
}
