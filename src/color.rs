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

/// Bilinear texture fetch with wrap-around. `(u, v)` are in texel units; each
/// `TEX_SIZE` block tiles seamlessly with itself, so the wrap blends cleanly.
/// Smooths the blocky nearest-neighbour look of the procedural textures.
#[inline]
pub fn sample_tex_bilinear(tex: &[u32], u: f64, v: f64) -> u32 {
    let fu = u - 0.5;
    let fv = v - 0.5;
    let u0 = fu.floor() as i32;
    let v0 = fv.floor() as i32;
    let du = fu - u0 as f64;
    let dv = fv - v0 as f64;

    let mask = (TEX_SIZE - 1) as i32;
    let x0 = (u0 & mask) as usize;
    let x1 = ((u0 + 1) & mask) as usize;
    let y0 = (v0 & mask) as usize;
    let y1 = ((v0 + 1) & mask) as usize;

    let c00 = tex[y0 * TEX_SIZE + x0];
    let c10 = tex[y0 * TEX_SIZE + x1];
    let c01 = tex[y1 * TEX_SIZE + x0];
    let c11 = tex[y1 * TEX_SIZE + x1];

    let w00 = (1.0 - du) * (1.0 - dv);
    let w10 = du * (1.0 - dv);
    let w01 = (1.0 - du) * dv;
    let w11 = du * dv;

    let chan = |shift: u32| -> i32 {
        (((c00 >> shift) & 0xFF) as f64 * w00
            + ((c10 >> shift) & 0xFF) as f64 * w10
            + ((c01 >> shift) & 0xFF) as f64 * w01
            + ((c11 >> shift) & 0xFF) as f64 * w11) as i32
    };
    make_color(chan(16), chan(8), chan(0))
}
