//! Color packing, value noise, shading, and bilinear texture sampling.
//!
//! Colors are packed as `0x00RRGGBB` (the byte layout minifb expects), exactly
//! matching the C `makeColor`. Textures add an emission byte on top
//! (`0xEERRGGBB`, see `textures.rs`).
//!
//! The samplers work in fixed point: texture coordinates are texels with
//! [`FP_BITS`] fractional bits, so the hot loops step them with integer adds
//! and pull texel indices and blend weights out with shifts and masks — no
//! float-to-int conversions per pixel.

use crate::constants::TEX_SIZE;
use crate::textures::Tex;

/// Fractional bits in a fixed-point texture coordinate.
pub const FP_BITS: u32 = 16;
/// One texel in fixed point.
pub const FP_ONE: i64 = 1 << FP_BITS;

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

/// How to light a texel: per-channel light `k` (256 = as painted, already
/// multiplied by how much survives the fog), the strength `ke` of the texel's
/// own glow (0..=256, also fog-attenuated), and the fog colour scattered in
/// on top (`add`, 0..=255 per channel). Constant along a wall column; along a
/// floor row only `k` changes, and that is interpolated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lit {
    pub k: [u32; 3],
    pub ke: u32,
    pub add: [u32; 3],
}

impl Lit {
    /// Plain shading by one factor: no glow, no fog.
    pub fn uniform(sh: u32) -> Lit {
        Lit { k: [sh; 3], ke: 0, add: [0; 3] }
    }
}

/// Bilinear blend of four `0xEERRGGBB` texels with `0..=256` fixed-point
/// weights, returning `[emission, r, g, b]` each 0..=255. The four channels
/// run the same instruction sequence, which LLVM vectorizes — resist packing
/// channels into wider lanes by hand; measured slower here.
#[inline(always)]
pub(crate) fn blend4(c00: u32, c10: u32, c01: u32, c11: u32, wu: u32, wv: u32) -> [u32; 4] {
    let iwu = 256 - wu;
    let iwv = 256 - wv;
    let blend = |shift: u32| -> u32 {
        let top = ((c00 >> shift) & 0xFF) * iwu + ((c10 >> shift) & 0xFF) * wu;
        let bot = ((c01 >> shift) & 0xFF) * iwu + ((c11 >> shift) & 0xFF) * wu;
        (top * iwv + bot * wv) >> 16
    };
    [blend(24), blend(16), blend(8), blend(0)]
}

/// Light a blended texel: colour × (light + own glow), plus fog, clamped.
#[inline(always)]
pub fn shade4(t: [u32; 4], lit: &Lit) -> u32 {
    let e = (t[0] * lit.ke) >> 8;
    let r = (((t[1] * (lit.k[0] + e)) >> 8) + lit.add[0]).min(255);
    let g = (((t[2] * (lit.k[1] + e)) >> 8) + lit.add[1]).min(255);
    let b = (((t[3] * (lit.k[2] + e)) >> 8) + lit.add[2]).min(255);
    (r << 16) | (g << 8) | b
}

/// Bilinear fetch at fixed-point texel coordinates. `u`/`v` are already offset
/// by half a texel (so texel centres land on whole numbers); the texture wraps.
#[inline(always)]
pub fn fetch_bilinear(tex: &Tex, u: i64, v: i64) -> [u32; 4] {
    let mask = (TEX_SIZE - 1) as i64;
    let (tu, tv) = (u >> FP_BITS, v >> FP_BITS);
    let wu = ((u >> (FP_BITS - 8)) & 0xFF) as u32;
    let wv = ((v >> (FP_BITS - 8)) & 0xFF) as u32;
    let x0 = (tu & mask) as usize;
    let x1 = ((tu + 1) & mask) as usize;
    let row0 = (tv & mask) as usize * TEX_SIZE;
    let row1 = ((tv + 1) & mask) as usize * TEX_SIZE;
    blend4(tex[row0 + x0], tex[row0 + x1], tex[row1 + x0], tex[row1 + x1], wu, wv)
}

/// Wall-column variant: the horizontal texel pair and X weight are constant
/// down a whole column, so the caller hoists them and only `v` varies here.
/// `x0`/`x1` are re-masked so the compiler can prove the fetches in bounds.
#[inline(always)]
pub fn fetch_col(tex: &Tex, x0: usize, x1: usize, wu: u32, v: i64) -> [u32; 4] {
    let x0 = x0 & (TEX_SIZE - 1);
    let x1 = x1 & (TEX_SIZE - 1);
    let mask = (TEX_SIZE - 1) as i64;
    let tv = v >> FP_BITS;
    let wv = ((v >> (FP_BITS - 8)) & 0xFF) as u32;
    let row0 = (tv & mask) as usize * TEX_SIZE;
    let row1 = ((tv + 1) & mask) as usize * TEX_SIZE;
    blend4(tex[row0 + x0], tex[row0 + x1], tex[row1 + x0], tex[row1 + x1], wu, wv)
}

/// Texel-space `f64` to the samplers' fixed point, including the half-texel
/// offset that centres bilinear filtering.
#[inline]
pub fn to_fp(texels: f64) -> i64 {
    ((texels - 0.5) * FP_ONE as f64).floor() as i64
}

/// Sample and light one texel at float texel coordinates — the convenience
/// form of [`fetch_bilinear`] + [`shade4`] for code outside the hot loops.
pub fn sample_lit(tex: &Tex, u: f64, v: f64, lit: &Lit) -> u32 {
    shade4(fetch_bilinear(tex, to_fp(u), to_fp(v)), lit)
}
