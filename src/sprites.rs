//! Sprites: enemies, pickups, barrels, decorations, fireballs and particles,
//! depth-sorted and drawn against the wall depth buffer.
//!
//! Each sprite is described by a procedural `*_pixel(u, v)` function that
//! returns `Some(color)` for a drawn texel or `None` for transparent. Those
//! functions are far too slow to run per screen pixel (a baron filling the
//! view cost ~10 ms a frame), so every sprite and animation frame is rendered
//! once into a cached, mipmapped raster (see [`bank`]), and drawing is a
//! bilinear lookup plus lighting from the level's lightmap.

use crate::color::{blend4, make_color, shade_color, Lit, FP_BITS, FP_ONE};
use crate::constants::*;
use crate::game::Game;
use crate::lighting::{Decor, MAX_DECOR};
use crate::theme::*;
use crate::types::{Barrel, Enemy, Fireball, Pickup};
use std::sync::OnceLock;

/// Grunt: a helmeted soldier with a rifle held across the body.
fn grunt_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx0 = u - 0.5;
    let cy = v - 0.5;
    let sway = anim.sin() * 0.015;
    let cx = cx0 - sway;

    // --- Foreground details first so they aren't overpainted by bigger shapes ---

    // Gun muzzle tip (most forward)
    if cy > 0.005 && cy < 0.035 && cx > 0.36 && cx < 0.39 {
        return Some(0x050505);
    }
    // Gun barrel
    if cy > 0.00 && cy < 0.04 && cx > 0.20 && cx < 0.36 {
        let t = ((cx - 0.20) * 80.0) as i32;
        return Some(make_color(40 - t / 4, 40 - t / 4, 40 - t / 4));
    }
    // Gun body
    if cy > 0.04 && cy < 0.10 && cx > 0.22 && cx < 0.32 {
        return Some(if cy < 0.06 { 0x303030 } else { 0x181818 });
    }
    // Hand on gun
    if cy > 0.04 && cy < 0.11 && cx > 0.17 && cx < 0.22 {
        return Some(0xA08060);
    }

    // Belt buckle (highlight)
    if cy > 0.20 && cy < 0.26 && cx.abs() < 0.04 {
        return Some(if cy < 0.22 { 0xE0C040 } else { 0xA08020 });
    }
    // Belt strap
    if cy > 0.20 && cy < 0.26 && cx.abs() < 0.22 {
        return Some(0x181208);
    }

    // Chest emblem (cross)
    if (cx.abs() < 0.015 && cy > 0.03 && cy < 0.13) || ((cy - 0.08).abs() < 0.015 && cx.abs() < 0.05)
    {
        return Some(0xC0A040);
    }

    // Helmet rim (band across forehead)
    if cx.abs() < 0.15 && cy > -0.32 && cy < -0.28 {
        return Some(0x141810);
    }
    // Visor reflection
    if (cx - 0.05) * (cx - 0.05) + (cy + 0.245) * (cy + 0.245) < 0.0004 {
        return Some(0x80B0E0);
    }
    if (cx + 0.06) * (cx + 0.06) + (cy + 0.245) * (cy + 0.245) < 0.0002 {
        return Some(0x4070A0);
    }
    // Visor (dark goggles band)
    if cx.abs() < 0.13 && cy > -0.28 && cy < -0.22 {
        return Some(0x080808);
    }

    // Stubble / mouth shadow
    if cx.abs() < 0.06 && cy > -0.16 && cy < -0.12 {
        return Some(0x4C2818);
    }

    // Helmet highlight strip on top
    if cy > -0.46 && cy < -0.42 && cx.abs() < 0.10 {
        return Some(0x80A058);
    }
    // Helmet dome (background of head)
    if cx * cx + (cy + 0.36) * (cy + 0.36) < 0.025 && cy < -0.27 {
        let mut t = (cy + 0.46) / 0.20;
        if t < 0.0 {
            t = 0.0;
        }
        if t > 1.0 {
            t = 1.0;
        }
        let v_ = (70.0 - 30.0 * t) as i32;
        return Some(make_color(v_ - 10, v_ + 20, v_ - 20));
    }

    // Face skin with side shading
    if cx * cx + (cy + 0.18) * (cy + 0.18) < 0.014 {
        let mut xt = (cx + 0.10) / 0.20;
        if xt > 1.0 {
            xt = 1.0;
        }
        if xt < 0.0 {
            xt = 0.0;
        }
        let rr = (210.0 - 40.0 * (1.0 - xt)) as i32;
        let gg = (170.0 - 30.0 * (1.0 - xt)) as i32;
        let bb = (130.0 - 25.0 * (1.0 - xt)) as i32;
        return Some(make_color(rr, gg, bb));
    }

    // Pauldrons (shoulders) with edge shading
    if cy > -0.10 && cy < -0.04 && cx.abs() < 0.26 {
        let xt = cx.abs() / 0.26;
        let base = (78.0 - 30.0 * xt) as i32;
        return Some(make_color(base - 10, base + 18, base - 28));
    }

    // Vest stripe
    if cy > -0.02 && cy < 0.01 && cx.abs() < 0.18 {
        return Some(0x2A3812);
    }
    // Chest armor
    if cy > -0.04 && cy < 0.20 && cx.abs() < 0.20 {
        let t = (cy + 0.04) / 0.24;
        let base = (90.0 - 35.0 * t) as i32;
        return Some(make_color(base - 8, base + 18, base - 28));
    }

    // Legs with vertical shading
    if cy > 0.26 && cy < 0.44 && ((cx > -0.18 && cx < -0.03) || (cx > 0.03 && cx < 0.18)) {
        let t = (cy - 0.26) / 0.18;
        let base = (60.0 - 25.0 * t) as i32;
        return Some(make_color(base - 8, base + 12, base - 20));
    }

    // Boot tip highlight
    if cy > 0.42 && cy < 0.44 && ((cx + 0.10).abs() < 0.085 || (cx - 0.10).abs() < 0.085) {
        return Some(0x302010);
    }
    // Boots
    if cy > 0.42 && cy < 0.50 && ((cx + 0.10).abs() < 0.085 || (cx - 0.10).abs() < 0.085) {
        return Some(0x100804);
    }
    None
}

fn imp_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx = u - 0.5;
    let cy0 = v - 0.5;
    let bob = (anim * 2.0).sin() * 0.025;
    let cy = cy0 - bob;
    let arm_swing = (anim * 2.0).sin() * 0.05;

    // --- Smallest foreground details first ---

    // Eye highlight (specular)
    if (cx - 0.075) * (cx - 0.075) + (cy + 0.275) * (cy + 0.275) < 0.00035 {
        return Some(0xFFFFB0);
    }
    if (cx + 0.065) * (cx + 0.065) + (cy + 0.275) * (cy + 0.275) < 0.00025 {
        return Some(0xFFFF80);
    }
    // Glowing yellow iris
    if (cx - 0.07) * (cx - 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0020 {
        return Some(0xFFE020);
    }
    if (cx + 0.07) * (cx + 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0020 {
        return Some(0xFFE020);
    }
    // Eye socket (dark ring around iris)
    if (cx - 0.07) * (cx - 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0042 {
        return Some(0x100404);
    }
    if (cx + 0.07) * (cx + 0.07) + (cy + 0.27) * (cy + 0.27) < 0.0042 {
        return Some(0x100404);
    }

    // Nostrils
    if (cx.abs() - 0.014).abs() < 0.005 && cy > -0.215 && cy < -0.195 {
        return Some(0x000000);
    }
    // Nose snout
    if cx.abs() < 0.022 && cy > -0.23 && cy < -0.17 {
        return Some(0x401408);
    }

    // Upper fangs
    if cy > -0.15 && cy < -0.10 && ((cx + 0.05).abs() < 0.014 || (cx - 0.05).abs() < 0.014) {
        let gray = 230 - ((cy + 0.15) * 200.0) as i32;
        return Some(make_color(gray, gray, gray * 9 / 10));
    }
    // Lower fangs
    if cy > -0.10 && cy < -0.07 && ((cx + 0.025).abs() < 0.012 || (cx - 0.025).abs() < 0.012) {
        return Some(0xD8D8C0);
    }
    // Mouth gash
    if cy > -0.17 && cy < -0.13 && cx.abs() < 0.10 {
        return Some(0x180404);
    }

    // Skull ridge brow
    if cy > -0.42 && cy < -0.38 && cx.abs() < 0.14 {
        return Some(0x401008);
    }

    // Belly skull mark
    if cx.abs() < 0.035 && cy > -0.02 && cy < 0.03 {
        return Some(0xE0C040);
    }
    if cx.abs() < 0.02 && cy > 0.03 && cy < 0.06 {
        return Some(0x100400);
    }
    // Rib hints
    if ((cy > 0.06 && cy < 0.08) || (cy > 0.11 && cy < 0.13)) && cx.abs() > 0.06 && cx.abs() < 0.13 {
        return Some(0x300808);
    }

    // Claws
    {
        let claw_y = 0.20 + arm_swing;
        if cy > claw_y && cy < claw_y + 0.07 {
            let mut side = -1;
            while side <= 1 {
                for finger in 0..3 {
                    let fx = side as f64 * (0.20 + finger as f64 * 0.05);
                    if (cx - fx).abs() < 0.013 {
                        let t = (cy - claw_y) / 0.07;
                        let mut gray = (230.0 - 130.0 * t) as i32;
                        if gray < 100 {
                            gray = 100;
                        }
                        return Some(make_color(gray, gray, gray * 4 / 5));
                    }
                }
                side += 2;
            }
        }
    }
    // Hoof claws (toes)
    if cy > 0.42 && cy < 0.48 {
        let mut side = -1;
        while side <= 1 {
            for toe in 0..2 {
                let fx = side as f64 * (0.06 + toe as f64 * 0.06);
                if (cx - fx).abs() < 0.018 {
                    return Some(0x101010);
                }
            }
            side += 2;
        }
    }

    // --- Limbs / body / head (background of sprite) ---

    // Arms (biceps with highlight)
    {
        let top_y = -0.06 + arm_swing;
        let bot_y = 0.22 + arm_swing;
        if cy > top_y && cy < bot_y {
            if cx > 0.18 && cx < 0.32 {
                let bicep = (cx - 0.25) * (cx - 0.25) + (cy - 0.05) * (cy - 0.05) * 0.4;
                return Some(if bicep < 0.005 { 0x782010 } else { 0x501008 });
            }
            if cx > -0.32 && cx < -0.18 {
                let bicep = (cx + 0.25) * (cx + 0.25) + (cy - 0.05) * (cy - 0.05) * 0.4;
                return Some(if bicep < 0.005 { 0x782010 } else { 0x501008 });
            }
        }
    }

    // Legs
    if cy > 0.24 && cy < 0.44 && cx.abs() > 0.04 && cx.abs() < 0.15 {
        let t = (cy - 0.24) / 0.20;
        let base = (85.0 - 35.0 * t) as i32;
        return Some(make_color(base, base / 5, base / 6));
    }

    // Tail (S-curve, animated)
    {
        let tail_x = ((cy + 0.5) * 7.0 + anim * 2.0).sin() * 0.05;
        if cy > 0.02 && cy < 0.34 && (cx + 0.30 + tail_x).abs() < 0.02 {
            return Some(0x401008);
        }
    }

    // Body torso with gradient shading + highlight
    {
        let body_t = cx * cx * 1.6 + (cy - 0.04) * (cy - 0.04) * 0.9;
        if body_t < 0.068 {
            let mut shade = 1.0 - body_t * 8.0;
            if shade < 0.4 {
                shade = 0.4;
            }
            let mut rr = (130.0 * shade) as i32;
            let mut gg = (40.0 * shade) as i32;
            let mut bb = (28.0 * shade) as i32;
            if cx < -0.05 && cy < 0.04 {
                rr += 30;
                gg += 14;
                bb += 8;
            }
            return Some(make_color(rr, gg, bb));
        }
    }

    // Head with subtle shading
    {
        let head_t = cx * cx + (cy + 0.30) * (cy + 0.30) * 1.2;
        if head_t < 0.034 {
            let mut shade = 1.0 - head_t * 12.0;
            if shade < 0.3 {
                shade = 0.3;
            }
            let rr = (150.0 * shade) as i32 + 18;
            let gg = (55.0 * shade) as i32 + 8;
            let bb = (40.0 * shade) as i32 + 6;
            return Some(make_color(rr, gg, bb));
        }
    }

    // Horns (tapering, gradient)
    {
        let hx = [-0.17, 0.17];
        for h in &hx {
            if cy > -0.52 && cy < -0.34 {
                let t = (cy + 0.52) / 0.18;
                let half_w = 0.05 * t;
                if cx > h - half_w && cx < h + half_w {
                    let gray = if cy < -0.46 {
                        30 + (20.0 * t) as i32
                    } else if cy < -0.42 {
                        55
                    } else {
                        95
                    };
                    return Some(make_color(gray, gray * 4 / 5, gray * 3 / 4));
                }
            }
        }
    }

    None
}

/// Wraith: a hovering spectre. No legs — the torso frays into a tattered wisp
/// that ripples with `anim` — a hooded skull face with cold blue eyes, and thin
/// claw hands held out front. Drawn translucent by the caller.
fn wraith_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx0 = u - 0.5;
    let hover = (anim * 1.5).sin() * 0.035;
    let cy = v - 0.46 - hover;
    // The whole shape drifts sideways a little, like it's swimming.
    let cx = cx0 - anim.sin() * 0.02;

    // --- Face details (foreground) ---
    // Eye glow, brighter core inside a wider halo.
    for &ex in &[-0.062, 0.062] {
        let d = (cx - ex) * (cx - ex) + (cy + 0.255) * (cy + 0.255);
        if d < 0.00035 {
            return Some(0xF0FFFF);
        }
        if d < 0.0016 {
            return Some(0x60D0FF);
        }
        if d < 0.0032 {
            return Some(0x143048);
        }
    }
    // Hollow mouth slit.
    if cy > -0.19 && cy < -0.15 && cx.abs() < 0.05 {
        return Some(0x102030);
    }
    // Skull cheekbones under the hood.
    if cy > -0.24 && cy < -0.10 && cx.abs() < 0.11 {
        let t = (cy + 0.24) / 0.14;
        let g = (190.0 - 60.0 * t) as i32;
        return Some(make_color(g - 30, g - 10, g));
    }

    // Claws: three thin talons per hand, swinging with the drift.
    {
        let swing = (anim * 1.5).cos() * 0.04;
        let hy = 0.06 + swing;
        if cy > hy && cy < hy + 0.10 {
            let mut side = -1;
            while side <= 1 {
                for finger in 0..3 {
                    let fx = side as f64 * (0.19 + finger as f64 * 0.045);
                    if (cx - fx).abs() < 0.011 {
                        let t = (cy - hy) / 0.10;
                        let g = (220.0 - 90.0 * t) as i32;
                        return Some(make_color(g - 40, g - 10, g));
                    }
                }
                side += 2;
            }
        }
    }

    // --- Body: a hood/shoulder mass tapering into ragged streamers ---
    // Streamers below the waist: vertical ribbons whose length wobbles.
    if cy >= 0.10 {
        let ripple = ((cx * 14.0) + anim * 3.0).sin() * 0.06;
        let tail_end = 0.44 + ripple;
        let width = 0.20 * (1.0 - ((cy - 0.10) / 0.40).min(1.0) * 0.55);
        if cy < tail_end && cx.abs() < width {
            // Gaps between ribbons so the edge reads as torn cloth.
            let rib = ((cx * 26.0) + anim).sin();
            if rib > -0.35 {
                let t = (cy - 0.10) / 0.36;
                let b = (150.0 - 90.0 * t) as i32;
                return Some(make_color(b / 3, b * 2 / 3, b));
            }
        }
        return None;
    }

    // Shoulders / cowl.
    let body = cx * cx * 1.5 + (cy + 0.06) * (cy + 0.06) * 0.7;
    if body < 0.055 {
        let shade = (1.0 - body * 9.0).max(0.45);
        let r = (70.0 * shade) as i32;
        let g = (120.0 * shade) as i32;
        let b = (165.0 * shade) as i32;
        return Some(make_color(r, g, b));
    }
    // Hood: a pointed cowl over the skull.
    let head = cx * cx * 1.15 + (cy + 0.27) * (cy + 0.27);
    if head < 0.030 {
        let shade = (1.0 - head * 14.0).max(0.35);
        let r = (60.0 * shade) as i32 + 10;
        let g = (105.0 * shade) as i32 + 14;
        let b = (150.0 * shade) as i32 + 20;
        return Some(make_color(r, g, b));
    }
    None
}

/// Baron: the heavy. A broad armoured brute — plated pauldrons, a scarred
/// chest, wide sweeping horns and molten eyes. Deliberately bulkier than the
/// imp so its silhouette reads as "trouble" at a distance.
fn baron_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx0 = u - 0.5;
    let cy0 = v - 0.5;
    let sway = (anim * 1.5).sin() * 0.012;
    let cx = cx0 - sway;
    let cy = cy0;
    let stride = (anim * 1.5).sin() * 0.04;

    // --- Face (foreground) ---
    // Molten eyes, small and deep-set, with a hot core.
    for &ex in &[-0.072, 0.072] {
        let d = (cx - ex) * (cx - ex) * 1.2 + (cy + 0.285) * (cy + 0.285) * 2.0;
        if d < 0.00030 {
            return Some(0xFFF0C0);
        }
        if d < 0.00130 {
            return Some(0xFF8018);
        }
        if d < 0.00260 {
            return Some(0x180800);
        }
    }
    // Scowl: a heavy brow bar over each eye, angled down toward the nose.
    for &side in &[-1.0f64, 1.0] {
        let bx = (cx * side - 0.03) / 0.11; // 0 at the inner end, 1 at the outer
        if (0.0..=1.0).contains(&bx) {
            let by = -0.345 + bx * 0.035; // rises toward the temple
            if cy > by && cy < by + 0.045 {
                return Some(0x3E2412);
            }
        }
    }
    // Snarl: an open maw with a row of tusks.
    if cy > -0.20 && cy < -0.15 && cx.abs() < 0.10 {
        let tusk = ((cx * 46.0).sin()).abs() > 0.55;
        return Some(if tusk { 0xE8E0C0 } else { 0x200606 });
    }

    // Chest plate: a banded cuirass split by a central seam.
    if cy > -0.02 && cy < 0.20 && cx.abs() < 0.21 {
        let t = (cy + 0.02) / 0.22;
        let base = (128.0 - 46.0 * t) as i32;
        if cx.abs() < 0.014 {
            return Some(make_color(base - 40, base - 48, base - 56));
        }
        // Two raised bands catch the light across the plate.
        let band = (cy - 0.045).abs() < 0.015 || (cy - 0.135).abs() < 0.015;
        let side_fall = (cx.abs() / 0.21 * 26.0) as i32; // rounds the plate off
        if band {
            return Some(make_color(base + 34 - side_fall, base + 28 - side_fall, base + 14 - side_fall));
        }
        return Some(make_color(base - side_fall, base - 14 - side_fall, base - 30 - side_fall));
    }

    // Pauldrons: domed plates capping the shoulders, sitting proud of the chest.
    for &side in &[-1.0f64, 1.0] {
        let px = cx * side;
        let d = (px - 0.26) * (px - 0.26) / 0.0130 + (cy + 0.055) * (cy + 0.055) / 0.0085;
        if d < 1.0 {
            let base = (155.0 - 60.0 * d) as i32;
            // Rivets around the plate's rim.
            if d > 0.72 && ((px * 34.0) as i32 + (cy * 34.0) as i32) % 3 == 0 {
                return Some(0xE0D8C0);
            }
            return Some(make_color(base, base - 18, base - 34));
        }
    }

    // Arms hanging outside the plates.
    if cy > 0.02 && cy < 0.26 && cx.abs() > 0.22 && cx.abs() < 0.34 {
        let t = (cy - 0.02) / 0.24;
        let base = (140.0 - 45.0 * t) as i32;
        return Some(make_color(base, base / 2, base / 3));
    }
    // Fists.
    if cy >= 0.26 && cy < 0.33 && cx.abs() > 0.20 && cx.abs() < 0.34 {
        return Some(0x8A3A20);
    }

    // Legs with a slow stride offset, hooves at the bottom.
    for (i, &lx) in [-0.13, 0.13].iter().enumerate() {
        let off = if i == 0 { stride } else { -stride };
        if cy > 0.20 + off && cy < 0.44 + off && (cx - lx).abs() < 0.085 {
            let t = (cy - 0.20 - off) / 0.24;
            let base = (120.0 - 40.0 * t) as i32;
            return Some(make_color(base, base / 2 - 4, base / 3));
        }
        if cy >= 0.44 + off && cy < 0.50 + off && (cx - lx).abs() < 0.10 {
            return Some(0x181008);
        }
    }

    // Head: a heavy cranium tapering into a squared-off jaw, so the silhouette
    // isn't a plain disc.
    let skull = cx * cx * 1.30 + (cy + 0.315) * (cy + 0.315) * 1.20;
    let jaw = cy > -0.235 && cy < -0.115 && cx.abs() < 0.155 - (cy + 0.235) * 0.42;
    if skull < 0.032 || jaw {
        let shade = (1.0 - skull * 12.0).clamp(0.35, 1.0);
        let r = (165.0 * shade) as i32 + 20;
        let g = (85.0 * shade) as i32 + 10;
        let b = (55.0 * shade) as i32 + 8;
        return Some(make_color(r, g, b));
    }

    // Horns: wide, curving up and outward from the temples.
    for &side in &[-1.0f64, 1.0] {
        if cy > -0.50 && cy < -0.28 {
            let t = (cy + 0.50) / 0.22; // 0 at tip, 1 at base
            let axis = side * (0.34 - 0.16 * t); // sweeps inward toward the skull
            let half_w = 0.018 + 0.038 * t;
            if (cx - axis).abs() < half_w {
                let g = (90.0 + 90.0 * (1.0 - t)) as i32;
                return Some(make_color(g, g * 9 / 10, g * 7 / 10));
            }
        }
    }
    None
}

/// Fuel barrel: a banded drum with a hazard stripe, cylinder-shaded so it reads
/// as round. `flash` lifts it toward white for the instant before it blows.
fn barrel_pixel(u: f64, v: f64) -> Option<u32> {
    let cx = u - 0.5;
    let cy = v - 0.5;
    let hw = 0.30;
    if cx.abs() > hw || cy < -0.42 || cy > 0.44 {
        return None;
    }
    // Round the lid and base corners slightly.
    let edge = cx.abs() / hw;
    if (cy < -0.38 || cy > 0.40) && edge > 0.86 {
        return None;
    }
    // Cylinder shading: bright a third of the way in from the left.
    let round = 1.0 - ((cx + 0.09) / hw).abs().min(1.0).powi(2) * 0.62;

    // Lid: darker ellipse with a filler cap.
    if cy < -0.34 {
        if cx.abs() < 0.06 {
            return Some(0x6A6A56);
        }
        let g = (110.0 * round) as i32;
        return Some(make_color(g, g, g * 3 / 4));
    }
    // Rolling hoops.
    if (cy + 0.16).abs() < 0.028 || (cy - 0.16).abs() < 0.028 {
        let g = (150.0 * round) as i32;
        return Some(make_color(g, g - 10, g - 26));
    }
    // Hazard stripe across the middle.
    if cy.abs() < 0.09 {
        let diag = ((cx * 22.0 + cy * 22.0).sin()) > 0.0;
        return Some(if diag {
            make_color((235.0 * round) as i32, (190.0 * round) as i32, (40.0 * round) as i32)
        } else {
            make_color((40.0 * round) as i32, (34.0 * round) as i32, (26.0 * round) as i32)
        });
    }
    // Body: olive drum with a little rust mottling.
    let rust = ((cx * 31.0).sin() * (cy * 27.0).cos()) > 0.72;
    if rust {
        return Some(make_color((130.0 * round) as i32, (66.0 * round) as i32, (30.0 * round) as i32));
    }
    let g = (128.0 * round) as i32;
    Some(make_color(g * 3 / 4, g, g / 2))
}

/// Health pickup: a 3D-shaded cream medkit with a beveled rim and a shaded red
/// cross. `u,v` in 0..1; returns `None` outside the rounded-rect body.
fn health_pixel(u: f64, v: f64, _anim: f64) -> Option<u32> {
    let cx = u - 0.5;
    let cy = v - 0.5;
    let ax = cx.abs();
    let ay = cy.abs();
    let (hw, hh, r) = (0.42, 0.42, 0.12);
    if ax > hw || ay > hh {
        return None;
    }
    if ax > hw - r && ay > hh - r {
        let (dx, dy) = (ax - (hw - r), ay - (hh - r));
        if dx * dx + dy * dy > r * r {
            return None;
        }
    }
    // Red cross, shaded lighter toward the top, with a crisp upper-left edge.
    if (ax < 0.11 && ay < 0.31) || (ay < 0.11 && ax < 0.31) {
        let t = (cy + 0.31) / 0.62; // 0 top .. 1 bottom
        let hi = (ay < 0.31 && (cx + 0.095).abs() < 0.02) || (ax < 0.31 && (cy + 0.095).abs() < 0.02);
        let rr = (238.0 - 46.0 * t) as i32 + if hi { 16 } else { 0 };
        let gg = (54.0 - 22.0 * t) as i32 + if hi { 12 } else { 0 };
        let bb = (46.0 - 20.0 * t) as i32;
        return Some(make_color(rr.min(255), gg, bb));
    }
    // Cream body with a beveled frame (top/left highlight, bottom/right shadow).
    let hi = cy < -(hh - 0.06) || cx < -(hw - 0.06);
    let lo = cy > (hh - 0.06) || cx > (hw - 0.06);
    let grad = (-cx - cy) * 14.0;
    let base = 224.0 + grad + if hi { 18.0 } else { 0.0 } - if lo { 34.0 } else { 0.0 };
    Some(make_color(base as i32, (base - 6.0) as i32, (base - 24.0) as i32))
}

/// Ammo pickup: a beveled olive box with brass-capped shotgun shells poking out
/// of the top and a hazard stripe. `u,v` in 0..1.
fn ammo_pixel(u: f64, v: f64, _anim: f64) -> Option<u32> {
    let cx = u - 0.5;
    let cy = v - 0.5;
    let ax = cx.abs();
    let ay = cy.abs();
    let (hw, hh, r) = (0.44, 0.34, 0.06);
    if ax > hw || ay > hh {
        return None;
    }
    if ax > hw - r && ay > hh - r {
        let (dx, dy) = (ax - (hw - r), ay - (hh - r));
        if dx * dx + dy * dy > r * r {
            return None;
        }
    }
    // Shells poking out of the top: brass cap over a red body, cylinder-shaded.
    if cy > -0.30 && cy < 0.02 && ax < 0.40 {
        let pitch = 0.158;
        let sx = (cx + 0.4).rem_euclid(pitch) - pitch * 0.5;
        if sx.abs() < 0.06 {
            let round = 1.0 - (sx / 0.06).powi(2) * 0.55; // bright center, dark sides
            return Some(if cy < -0.15 {
                make_color((235.0 * round) as i32, (188.0 * round) as i32, (60.0 * round) as i32)
            } else {
                make_color((196.0 * round) as i32, (44.0 * round) as i32 + 8, (34.0 * round) as i32 + 6)
            });
        }
        return Some(0x1E2412); // dark gap between shells
    }
    // Olive metal box: bevel + a yellow hazard stripe near the bottom.
    let hi = cy < -(hh - 0.05) || cx < -(hw - 0.05);
    let lo = cy > (hh - 0.05) || cx > (hw - 0.05);
    if cy > 0.12 && cy < 0.20 {
        return Some(make_color(198, 170, 44));
    }
    let base = 66.0 + (-cx - cy) * 9.0 + if hi { 16.0 } else { 0.0 } - if lo { 26.0 } else { 0.0 };
    Some(make_color((base * 0.82) as i32, base as i32, (base * 0.44) as i32))
}

/// Weapon pickup: a side-on gun silhouette resting on the floor — a metal
/// barrel over a tinted body and grip. The body tint distinguishes the kind
/// (brown for the shotgun, olive-green for the rifle). `u,v` in 0..1.
fn weapon_pixel(u: f64, v: f64, kind: i32) -> Option<u32> {
    let cx = u - 0.5;
    let cy = v - 0.5;
    // Body tint per weapon: (mid, highlight).
    let (body, hilite) = if kind == PU_RIFLE {
        (0x2E3A24, 0x46583A) // green
    } else {
        (0x4A2E18, 0x6A4524) // wood brown (shotgun)
    };
    // Metal barrel: a horizontal bar across the upper half, cylinder-shaded.
    if cy > -0.20 && cy < -0.02 && cx > -0.44 && cx < 0.40 {
        let round = 1.0 - ((cy + 0.11) / 0.09).abs() * 0.45;
        let g = (150.0 * round) as i32;
        return Some(make_color(g, g, g));
    }
    // Receiver / body block.
    if cy >= -0.02 && cy < 0.14 && cx > -0.34 && cx < 0.34 {
        return Some(if cy < 0.05 { hilite } else { body });
    }
    // Trigger guard.
    if cx > -0.06 && cx < 0.12 && cy >= 0.14 && cy < 0.24 {
        return Some(0x1A1A1A);
    }
    // Grip, angled down to the right.
    if cy >= 0.10 && cy < 0.42 {
        let gx = 0.18 + (cy - 0.10) * 0.35;
        if cx > gx - 0.11 && cx < gx + 0.07 {
            return Some(if cx < gx - 0.04 { hilite } else { body });
        }
    }
    None
}

// ---- decorations ----

/// Flame colour by heat: 0 is the red fringe, 1 the white-hot core.
fn flame_rgb(heat: f64) -> u32 {
    let h = heat.clamp(0.0, 1.0);
    let (r, g, b) = if h > 0.62 {
        let t = (h - 0.62) / 0.38;
        (255.0, 200.0 + 50.0 * t, 80.0 + 135.0 * t)
    } else if h > 0.3 {
        let t = (h - 0.3) / 0.32;
        (250.0 + 5.0 * t, 110.0 + 90.0 * t, 30.0 + 50.0 * t)
    } else {
        let t = h / 0.3;
        (170.0 + 80.0 * t, 40.0 + 70.0 * t, 15.0 + 15.0 * t)
    };
    make_color(r as i32, g as i32, b as i32)
}

/// One tongue of flame: a teardrop from `top` (its tip) down to `bot` around
/// `cx`, at most `w` half-width, swaying with `anim`.
fn flame_tongue(u: f64, v: f64, cx: f64, top: f64, bot: f64, w: f64, anim: f64) -> Option<u32> {
    if v < top || v > bot {
        return None;
    }
    let t = (v - top) / (bot - top); // 0 tip .. 1 base
    let sway = (anim + v * 11.0).sin() * 0.018 * (1.0 - t);
    let half = w * t.powf(0.6) * (1.0 - t * t * 0.5);
    let off = (u - cx - sway).abs();
    if off >= half {
        return None;
    }
    Some(flame_rgb((1.0 - off / half) * (0.35 + 0.65 * t)))
}

/// Iron, cylinder-shaded across `t` in -1..1 (lit from the left).
fn iron(t: f64, r: f64, g: f64, b: f64) -> u32 {
    let m = (1.0 - (t + 0.3).abs().min(1.0) * 0.5).max(0.3);
    make_color((r * m) as i32, (g * m) as i32, (b * m) as i32)
}

/// A standing torch: a flaming cup on an iron pole with tripod feet.
fn torch_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx = u - 0.5;
    let top = 0.08 + 0.025 * (anim * 2.0).sin();
    if let Some(c) = flame_tongue(u, v, 0.5, top, 0.34, 0.085, anim) {
        return Some(c);
    }
    if (0.33..0.40).contains(&v) {
        let half = 0.11 - (v - 0.33) * 0.7;
        if cx.abs() < half {
            return Some(if v < 0.345 { make_color(232, 112, 40) } else { iron(cx / half, 80.0, 70.0, 62.0) });
        }
    }
    if (0.40..0.92).contains(&v) {
        let collar = (v - 0.52).abs() < 0.012 || (v - 0.76).abs() < 0.012;
        let half = if collar { 0.034 } else { 0.022 };
        if cx.abs() < half {
            return Some(iron(cx / half, 70.0, 63.0, 58.0));
        }
    }
    if (0.88..0.985).contains(&v) {
        let spread = (v - 0.88) * 1.6;
        for leg in [-1.0, 0.0, 1.0] {
            if (cx - leg * spread).abs() < 0.016 {
                return Some(make_color(52, 46, 42));
            }
        }
    }
    None
}

/// A tech lamp post: caged white lamp on a striped post.
fn lamp_pixel(u: f64, v: f64) -> Option<u32> {
    let cx = u - 0.5;
    if (0.08..0.25).contains(&v) && cx.abs() < 0.10 {
        if cx.abs() < 0.075 && (0.105..0.225).contains(&v) {
            let bar = ((cx + 0.075) / 0.03).fract() < 0.22;
            return Some(if bar { make_color(120, 140, 160) } else { make_color(236, 244, 255) });
        }
        return Some(if v < 0.095 { make_color(150, 156, 166) } else { make_color(40, 44, 52) });
    }
    if (0.25..0.92).contains(&v) {
        let half = if v < 0.30 { 0.028 } else { 0.036 };
        if cx.abs() < half {
            if (0.80..0.86).contains(&v) {
                let stripe = (((v - cx) / 0.02).floor() as i32) & 1 == 0;
                return Some(if stripe { make_color(214, 176, 36) } else { make_color(26, 24, 22) });
            }
            return Some(iron(cx / half, 104.0, 110.0, 120.0));
        }
    }
    if (0.92..0.98).contains(&v) && cx.abs() < 0.12 {
        return Some(if v < 0.93 { make_color(124, 128, 136) } else { make_color(58, 62, 70) });
    }
    None
}

/// A heap of skulls and bones at the foot of the wall.
fn skulls_pixel(u: f64, v: f64) -> Option<u32> {
    // Front-most first: the first shape that covers a texel wins.
    for &(sx, sy, r) in &[(0.63, 0.885, 0.078), (0.36, 0.875, 0.08), (0.5, 0.80, 0.075)] {
        let (x, y) = ((u - sx) / r, (v - sy) / r);
        let cranium = x * x + y * y < 1.0 && y < 0.5;
        let jaw = x.abs() < 0.55 && (0.45..1.15).contains(&y);
        if !(cranium || jaw) {
            continue;
        }
        let eye = (x - 0.36).powi(2) + (y - 0.12).powi(2) < 0.075 || (x + 0.36).powi(2) + (y - 0.12).powi(2) < 0.075;
        let nose = x.abs() < 0.1 && (0.35..0.6).contains(&y);
        if eye || nose {
            return Some(make_color(24, 14, 10));
        }
        if jaw && y > 0.8 && ((x * 7.0).floor() as i32) & 1 == 0 {
            return Some(make_color(150, 138, 112)); // teeth
        }
        let m = (0.85 - x * 0.18 - y * 0.25).clamp(0.45, 1.05);
        return Some(make_color((206.0 * m) as i32, (194.0 * m) as i32, (162.0 * m) as i32));
    }
    for &(x0, y0, x1, y1) in &[(0.22, 0.955, 0.52, 0.935), (0.46, 0.975, 0.80, 0.945)] {
        let (vx, vy) = (x1 - x0, y1 - y0);
        let t = (((u - x0) * vx + (v - y0) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
        let d = (x0 + vx * t - u).hypot(y0 + vy * t - v);
        if d < if !(0.06..=0.94).contains(&t) { 0.02 } else { 0.012 } {
            return Some(make_color(196, 184, 152));
        }
    }
    None
}

/// Distance from `(u, v)` to a segment, and how far along it (0..1).
fn seg(u: f64, v: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> (f64, f64, f64) {
    let (vx, vy) = (x1 - x0, y1 - y0);
    let t = (((u - x0) * vx + (v - y0) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    let (dx, dy) = (u - (x0 + vx * t), v - (y0 + vy * t));
    // Signed side, so the left edge of each limb can catch the light.
    let side = vx * dy - vy * dx;
    ((dx * dx + dy * dy).sqrt(), t, side)
}

/// A dead tree: a gnarled trunk with bare, forking branches.
fn tree_pixel(u: f64, v: f64) -> Option<u32> {
    // (x0, y0, x1, y1, radius at start, radius at end)
    const LIMBS: [(f64, f64, f64, f64, f64, f64); 10] = [
        (0.50, 1.00, 0.47, 0.62, 0.050, 0.032),
        (0.47, 0.62, 0.53, 0.28, 0.032, 0.016),
        (0.53, 0.28, 0.50, 0.08, 0.016, 0.005),
        (0.47, 0.64, 0.27, 0.42, 0.020, 0.007),
        (0.27, 0.42, 0.17, 0.27, 0.007, 0.003),
        (0.27, 0.42, 0.33, 0.30, 0.006, 0.003),
        (0.50, 0.48, 0.73, 0.32, 0.017, 0.006),
        (0.73, 0.32, 0.82, 0.16, 0.006, 0.003),
        (0.52, 0.34, 0.36, 0.17, 0.010, 0.004),
        (0.53, 0.30, 0.66, 0.14, 0.009, 0.003),
    ];
    // Root flare.
    if v > 0.94 {
        let half = 0.05 + (v - 0.94) * 1.6;
        if (u - 0.5).abs() < half {
            return Some(make_color(56, 46, 38));
        }
    }
    for &(x0, y0, x1, y1, r0, r1) in &LIMBS {
        let (d, t, side) = seg(u, v, x0, y0, x1, y1);
        let r = r0 + (r1 - r0) * t;
        if d < r {
            let lit = side < 0.0 && d > r * 0.35;
            return Some(if lit { make_color(96, 80, 66) } else { make_color(58, 47, 40) });
        }
    }
    None
}

/// Two chains hanging from the ceiling, one ending in a meat hook.
fn chains_pixel(u: f64, v: f64) -> Option<u32> {
    for &(cx, len) in &[(0.42, 0.52), (0.58, 0.66)] {
        if v > len {
            continue;
        }
        let dx = u - cx;
        let link = (v / 0.04) as i32;
        let lv = (v % 0.04) / 0.04;
        if link & 1 == 0 {
            let (ex, ey) = (dx / 0.016, (lv - 0.5) / 0.5);
            let d = ex * ex + ey * ey;
            if d < 1.0 && d > 0.35 {
                return Some(iron(ex, 120.0, 116.0, 112.0));
            }
        } else if dx.abs() < 0.006 {
            return Some(make_color(70, 66, 62));
        }
    }
    // The hook: a shank, then a J curling back up.
    if (0.66..0.71).contains(&v) && (u - 0.58).abs() < 0.007 {
        return Some(make_color(130, 124, 118));
    }
    let (hx, hy) = (u - 0.556, v - 0.71);
    let r = hx.hypot(hy);
    if hy >= 0.0 && (r - 0.024).abs() < 0.007 {
        return Some(make_color(130, 124, 118));
    }
    None
}

/// A fire bowl on a pedestal, burning with three tongues of flame.
fn brazier_pixel(u: f64, v: f64, anim: f64) -> Option<u32> {
    let cx = u - 0.5;
    let tongues = [
        (0.43, 0.30 + 0.04 * (anim * 1.7).sin(), 0.06),
        (0.50, 0.18 + 0.04 * (anim * 1.3 + 1.0).sin(), 0.085),
        (0.57, 0.28 + 0.04 * (anim * 2.1 + 2.0).sin(), 0.06),
    ];
    for &(x, top, w) in &tongues {
        if let Some(c) = flame_tongue(u, v, x, top, 0.6, w, anim + x * 10.0) {
            return Some(c);
        }
    }
    if (0.57..0.61).contains(&v) && cx.abs() < 0.17 {
        let hot = ((u * 61.0).sin() * (v * 97.0).cos()) > 0.2;
        return Some(if hot { make_color(255, 150, 50) } else { make_color(150, 40, 16) });
    }
    if (0.60..0.71).contains(&v) {
        let half = 0.2 - (v - 0.6) * 0.9;
        if cx.abs() < half {
            let rim = v < 0.62;
            return Some(if rim { make_color(150, 120, 100) } else { iron(cx / half, 92.0, 80.0, 72.0) });
        }
    }
    if (0.71..0.93).contains(&v) && cx.abs() < 0.055 {
        return Some(iron(cx / 0.055, 110.0, 104.0, 96.0));
    }
    if (0.93..0.99).contains(&v) && cx.abs() < 0.13 {
        return Some(iron(cx / 0.13, 120.0, 114.0, 106.0));
    }
    None
}

/// A fluted stone column with a capital and a base.
fn pillar_pixel(u: f64, v: f64) -> Option<u32> {
    let cx = u - 0.5;
    let stone = |m: f64| make_color((150.0 * m) as i32, (144.0 * m) as i32, (132.0 * m) as i32);
    if v < 0.07 && cx.abs() < 0.19 {
        return Some(stone(if v < 0.02 { 1.1 } else { 0.85 }));
    }
    if (0.07..0.11).contains(&v) && cx.abs() < 0.16 {
        return Some(stone(0.95));
    }
    if (0.93..1.0).contains(&v) && cx.abs() < 0.2 {
        return Some(stone(if v < 0.945 { 1.05 } else { 0.8 }));
    }
    if (0.89..0.93).contains(&v) && cx.abs() < 0.16 {
        return Some(stone(0.95));
    }
    if (0.11..0.89).contains(&v) && cx.abs() < 0.13 {
        let t = cx / 0.13;
        let round = 1.0 - (t + 0.35).abs().powi(2) * 0.5;
        let flute = ((t * 4.0 * std::f64::consts::PI).cos() * 0.5 + 0.5) * 0.18;
        let chip = ((u * 53.0).sin() * (v * 41.0).sin()) > 0.93;
        return Some(stone((round - flute - if chip { 0.25 } else { 0.0 }).max(0.3)));
    }
    None
}

// ---- the sprite bank: every sprite pre-rendered once ----

/// Raster size of a cached sprite.
pub const SPR: usize = 128;
/// Mip levels: 128, 64, 32 and 16 texels.
const MIPS: usize = 4;
/// Supersamples per texel axis when rasterising (3x3 per texel).
const SS: usize = 3;
/// Animation frames for flame decorations, over one 2π cycle.
const FLAME_FRAMES: usize = 8;

/// `(period, frames)` of each enemy kind's animation, indexed by `EN_*`. The
/// period is the shortest span after which every sine in the kind's sprite
/// function repeats, so the frames loop without a jump.
pub(crate) const ENEMY_ANIM: [(f64, usize); EN_KIND_MAX] = [
    (std::f64::consts::TAU, 24),
    (std::f64::consts::PI, 12),
    (2.0 * std::f64::consts::TAU, 32),
    (2.0 * std::f64::consts::TAU / 3.0, 16),
];

/// A pre-rendered sprite: premultiplied `0xAARRGGBB` (alpha = coverage), plus
/// box-filtered mips so distant sprites stay stable instead of shimmering.
pub struct Raster {
    pub(crate) levels: Vec<Vec<u32>>,
}

/// Which cached frame shows at animation phase `anim`.
pub(crate) fn anim_frame(period: f64, frames: usize, anim: f64) -> usize {
    (((anim / period).rem_euclid(1.0) * frames as f64) as usize).min(frames - 1)
}

pub(crate) fn decor_frames(kind: usize) -> usize {
    if kind == DEC_TORCH || kind == DEC_BRAZIER { FLAME_FRAMES } else { 1 }
}

#[derive(Clone, Copy)]
enum Job {
    Enemy(i32, usize),
    Pickup(i32),
    Barrel,
    Decor(usize, usize),
}

fn sample_job(job: Job, u: f64, v: f64) -> Option<u32> {
    match job {
        Job::Enemy(kind, frame) => {
            let (period, frames) = ENEMY_ANIM[kind as usize];
            let anim = (frame as f64 + 0.5) / frames as f64 * period;
            match kind {
                EN_IMP => imp_pixel(u, v, anim),
                EN_WRAITH => wraith_pixel(u, v, anim),
                EN_BARON => baron_pixel(u, v, anim),
                _ => grunt_pixel(u, v, anim),
            }
        }
        Job::Pickup(kind) => match kind {
            PU_HEALTH => health_pixel(u, v, 0.0),
            PU_AMMO => ammo_pixel(u, v, 0.0),
            _ => weapon_pixel(u, v, kind),
        },
        Job::Barrel => barrel_pixel(u, v),
        Job::Decor(kind, frame) => {
            let anim = (frame as f64 + 0.5) / decor_frames(kind) as f64 * std::f64::consts::TAU;
            match kind {
                DEC_TORCH => torch_pixel(u, v, anim),
                DEC_LAMP => lamp_pixel(u, v),
                DEC_SKULLS => skulls_pixel(u, v),
                DEC_TREE => tree_pixel(u, v),
                DEC_CHAINS => chains_pixel(u, v),
                DEC_BRAZIER => brazier_pixel(u, v, anim),
                _ => pillar_pixel(u, v),
            }
        }
    }
}

/// Render a sprite function into a raster: supersample each texel, darken
/// thinly-covered edge texels into a subtle outline (as the old per-pixel
/// path did on screen), premultiply by coverage, then build the mips.
fn rasterize(job: Job) -> Raster {
    let mut base = vec![0u32; SPR * SPR];
    let n = (SS * SS) as f64;
    for ty in 0..SPR {
        for tx in 0..SPR {
            let (mut r, mut g, mut b, mut cnt) = (0.0, 0.0, 0.0, 0.0);
            for sy in 0..SS {
                let v = (ty as f64 + (sy as f64 + 0.5) / SS as f64) / SPR as f64;
                for sx in 0..SS {
                    let u = (tx as f64 + (sx as f64 + 0.5) / SS as f64) / SPR as f64;
                    if let Some(c) = sample_job(job, u, v) {
                        r += ((c >> 16) & 0xFF) as f64;
                        g += ((c >> 8) & 0xFF) as f64;
                        b += (c & 0xFF) as f64;
                        cnt += 1.0;
                    }
                }
            }
            if cnt == 0.0 {
                continue;
            }
            let cov = cnt / n;
            let k = (0.6 + 0.4 * cov) * cov / cnt;
            let a = (cov * 255.0).round() as u32;
            base[ty * SPR + tx] = (a << 24) | make_color((r * k) as i32, (g * k) as i32, (b * k) as i32);
        }
    }
    let mut levels = vec![base];
    for l in 1..MIPS {
        let (src, ss) = (&levels[l - 1], SPR >> (l - 1));
        let ds = ss / 2;
        let mut out = vec![0u32; ds * ds];
        for y in 0..ds {
            for x in 0..ds {
                let px = [src[2 * y * ss + 2 * x], src[2 * y * ss + 2 * x + 1], src[(2 * y + 1) * ss + 2 * x], src[(2 * y + 1) * ss + 2 * x + 1]];
                let avg = |s: u32| (px.iter().map(|p| (p >> s) & 0xFF).sum::<u32>() + 2) / 4;
                out[y * ds + x] = (avg(24) << 24) | (avg(16) << 16) | (avg(8) << 8) | avg(0);
            }
        }
        levels.push(out);
    }
    Raster { levels }
}

pub(crate) struct SpriteBank {
    pub(crate) enemies: Vec<Vec<Raster>>,
    pub(crate) pickups: Vec<Raster>,
    pub(crate) barrel: Raster,
    pub(crate) decor: Vec<Vec<Raster>>,
}

/// The process-wide sprite cache, rasterised on first use across all cores
/// (~100 rasters; a fraction of a second, once).
pub(crate) fn bank() -> &'static SpriteBank {
    static BANK: OnceLock<SpriteBank> = OnceLock::new();
    BANK.get_or_init(|| {
        let mut jobs = Vec::new();
        for kind in 0..EN_KIND_MAX as i32 {
            for f in 0..ENEMY_ANIM[kind as usize].1 {
                jobs.push(Job::Enemy(kind, f));
            }
        }
        for kind in [PU_HEALTH, PU_AMMO, PU_SHOTGUN, PU_RIFLE] {
            jobs.push(Job::Pickup(kind));
        }
        jobs.push(Job::Barrel);
        for kind in 0..DEC_KIND_MAX {
            for f in 0..decor_frames(kind) {
                jobs.push(Job::Decor(kind, f));
            }
        }

        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
        let jobs_ref = &jobs;
        let done: Vec<Option<Raster>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    s.spawn(move || {
                        jobs_ref
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| i % threads == t)
                            .map(|(i, &j)| (i, rasterize(j)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            let mut out: Vec<Option<Raster>> = (0..jobs_ref.len()).map(|_| None).collect();
            for h in handles {
                for (i, r) in h.join().expect("sprite rasteriser thread panicked") {
                    out[i] = Some(r);
                }
            }
            out
        });
        // Results come back in job order, which is the bank's layout.
        let mut it = done.into_iter().map(|r| r.expect("every sprite job ran"));
        let mut next = || it.next().expect("job count matches the bank layout");
        let enemies = (0..EN_KIND_MAX).map(|k| (0..ENEMY_ANIM[k].1).map(|_| next()).collect()).collect();
        let pickups = (0..4).map(|_| next()).collect();
        let barrel = next();
        let decor = (0..DEC_KIND_MAX).map(|k| (0..decor_frames(k)).map(|_| next()).collect()).collect();
        SpriteBank { enemies, pickups, barrel, decor }
    })
}

/// Sprites never get darker than this, so an enemy in a black corner is still
/// a silhouette you can shoot.
const MIN_SPRITE_LIGHT: i32 = 96;

impl Game {
    /// Camera-space projection of a world point: `(screen_x, depth, base_h)`,
    /// where `base_h` is the height of a full wall at that depth. `None` when
    /// the point is behind (or practically at) the camera.
    pub(crate) fn project(&self, x: f64, y: f64) -> Option<(f64, f64, i32)> {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let (dx, dy) = (x - px, y - py);
        let (cs, sn) = ((-ang).cos(), (-ang).sin());
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return None;
        }
        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        Some((screen_x, tx, (SCREEN_H as f64 / tx) as i32))
    }

    /// Lighting for a sprite standing at `(x, y)`, `depth` from the camera.
    fn sprite_lit(&self, x: f64, y: f64, depth: f64) -> Lit {
        let light = self.env.light_at(x, y).map(|c| c.max(MIN_SPRITE_LIGHT));
        self.lit_for(light, self.fog_visibility(depth), 1.0)
    }

    /// Composite a cached sprite into the `w`×`h` screen box at `(dsx, dsy)`:
    /// pick the mip closest to (but not below) the on-screen size, sample it
    /// bilinearly, light and fog it, and blend by coverage scaled by `alpha`
    /// (0..=256; the wraith is see-through). `flash` draws it white instead
    /// (the hit flash). Columns nearer in the wall depth buffer occlude it.
    #[allow(clippy::too_many_arguments)]
    fn draw_raster(&mut self, r: &Raster, dsx: i32, dsy: i32, w: i32, h: i32, depth: f64, lit: &Lit, flash: bool, alpha: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let (sx0, sx1) = (dsx.max(0), (dsx + w).min(SCREEN_W as i32));
        let (sy0, sy1) = (dsy.max(0), (dsy + h).min(SCREEN_H as i32));
        if sx0 >= sx1 || sy0 >= sy1 {
            return;
        }
        let mut level = 0;
        while level + 1 < MIPS && (SPR >> (level + 1)) as i32 >= w.max(h) {
            level += 1;
        }
        let size = SPR >> level;
        let tex = &r.levels[level];
        let last = size as i64 - 1;
        let (su, sv) = (size as f64 / w as f64, size as f64 / h as f64);
        let dv = (sv * FP_ONE as f64) as i64;
        let v_start = ((((sy0 - dsy) as f64 + 0.5) * sv - 0.5) * FP_ONE as f64).floor() as i64;
        for x in sx0..sx1 {
            if depth >= self.depth[x as usize] {
                continue;
            }
            let u = ((((x - dsx) as f64 + 0.5) * su - 0.5) * FP_ONE as f64).floor() as i64;
            let tu = u >> FP_BITS;
            let wu = ((u >> (FP_BITS - 8)) & 0xFF) as u32;
            let (x0, x1) = (tu.clamp(0, last) as usize, (tu + 1).clamp(0, last) as usize);
            let mut v = v_start;
            let mut idx = sy0 as usize * SCREEN_W + x as usize;
            for _ in sy0..sy1 {
                let tv = v >> FP_BITS;
                let wv = ((v >> (FP_BITS - 8)) & 0xFF) as u32;
                v += dv;
                let (r0, r1) = (tv.clamp(0, last) as usize * size, (tv + 1).clamp(0, last) as usize * size);
                let t = blend4(tex[r0 + x0], tex[r0 + x1], tex[r1 + x0], tex[r1 + x1], wu, wv);
                if t[0] == 0 {
                    idx += SCREEN_W;
                    continue;
                }
                let a = ((t[0] * alpha) >> 8).min(255);
                let src = if flash {
                    [a, a * 240 / 255, a * 240 / 255]
                } else {
                    std::array::from_fn(|c| ((((t[c + 1] * lit.k[c]) >> 8) * alpha) >> 8) + ((a * lit.add[c]) >> 8))
                };
                let keep = 255 - a;
                let keep = keep + (keep >> 7); // 0..=256
                let dst = self.pixels[idx];
                let mix = |s: u32, sh: u32| (s + ((((dst >> sh) & 0xFF) * keep) >> 8)).min(255);
                self.pixels[idx] = (mix(src[0], 16) << 16) | (mix(src[1], 8) << 8) | mix(src[2], 0);
                idx += SCREEN_W;
            }
        }
    }

    /// A soft dark ellipse on the floor under a sprite, `half_w` wide each
    /// side of `sx`, centred on the floor line `feet`. Only floor pixels in
    /// front of the walls take it.
    fn draw_shadow(&mut self, sx: f64, feet: i32, half_w: f64, depth: f64) {
        let rx = half_w.max(1.0);
        let ry = (half_w * 0.22).max(1.5);
        let x0 = ((sx - rx).floor() as i32).max(0);
        let x1 = ((sx + rx).ceil() as i32).min(SCREEN_W as i32);
        let y0 = ((feet as f64 - ry).floor() as i32).max(SCREEN_H as i32 / 2 + 1);
        let y1 = ((feet as f64 + ry).ceil() as i32).min(SCREEN_H as i32);
        for x in x0..x1 {
            if depth >= self.depth[x as usize] {
                continue;
            }
            let nx = (x as f64 + 0.5 - sx) / rx;
            for y in y0.max(self.wall_bot[x as usize] + 1)..y1 {
                let ny = (y as f64 + 0.5 - feet as f64) / ry;
                let d = nx * nx + ny * ny;
                if d >= 1.0 {
                    continue;
                }
                let keep = (256.0 * (1.0 - 0.5 * (1.0 - d) * (1.0 - d))) as u32;
                let idx = y as usize * SCREEN_W + x as usize;
                let c = self.pixels[idx];
                let ch = |s: u32| ((((c >> s) & 0xFF) * keep) >> 8) << s;
                self.pixels[idx] = ch(16) | ch(8) | ch(0);
            }
        }
    }

    fn draw_enemy(&mut self, e: Enemy) {
        let Some((screen_x, tx, base_h)) = self.project(e.x, e.y) else {
            return;
        };
        // Kinds differ in bulk, but they all stand on the same floor: scale the
        // billboard about its feet so a baron towers instead of sinking.
        let sprite_h = (base_h as f64 * EN_SCALE[e.kind as usize]) as i32;
        let sprite_w = sprite_h;
        let feet = SCREEN_H as i32 / 2 + base_h / 2;
        let dsx = (screen_x - sprite_w as f64 / 2.0) as i32;
        let dsy = feet - sprite_h;

        self.draw_shadow(screen_x, feet, sprite_w as f64 * 0.26, tx);
        let lit = self.sprite_lit(e.x, e.y, tx);
        let alpha = if e.kind == EN_WRAITH { 184 } else { 256 };
        let (period, frames) = ENEMY_ANIM[e.kind as usize];
        let raster = &bank().enemies[e.kind as usize][anim_frame(period, frames, e.anim)];
        self.draw_raster(raster, dsx, dsy, sprite_w, sprite_h, tx, &lit, e.hit_flash > 0.0, alpha);
    }

    fn draw_barrel(&mut self, b: Barrel) {
        let Some((screen_x, tx, base_h)) = self.project(b.x, b.y) else {
            return;
        };
        let sz = ((base_h as f64 * 0.72) as i32).max(4);
        let feet = SCREEN_H as i32 / 2 + base_h / 2;
        let dsx = (screen_x - sz as f64 / 2.0) as i32;
        self.draw_shadow(screen_x, feet, sz as f64 * 0.3, tx);
        let lit = self.sprite_lit(b.x, b.y, tx);
        self.draw_raster(&bank().barrel, dsx, feet - sz, sz, sz, tx, &lit, b.hit_flash > 0.0, 256);
    }

    fn draw_fireball(&mut self, fb: Fireball) {
        let Some((screen_x, tx, _)) = self.project(fb.x, fb.y) else {
            return;
        };
        let sz = (((SCREEN_H as f64 / tx) * 0.35) as i32).max(2);
        let (cx, cy) = (screen_x, SCREEN_H as f64 / 2.0);
        let core = sz as f64 * 0.5;
        let halo = core * 2.2;
        let sx0 = ((cx - halo) as i32).max(0);
        let sx1 = ((cx + halo) as i32 + 1).min(SCREEN_W as i32);
        let sy0 = ((cy - halo) as i32).max(0);
        let sy1 = ((cy + halo) as i32 + 1).min(SCREEN_H as i32);
        for x in sx0..sx1 {
            if tx >= self.depth[x as usize] {
                continue;
            }
            for y in sy0..sy1 {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt();
                let idx = y as usize * SCREEN_W + x as usize;
                if d < core {
                    // Hot core, white-yellow in the middle.
                    let t = (d / core).powi(2);
                    self.pixels[idx] = make_color((255.0 * (1.0 - t * 0.3)) as i32, (220.0 * (1.0 - t * 0.75)) as i32, (120.0 * (1.0 - t)) as i32);
                } else if d < halo {
                    // Additive glow around it.
                    let g = (1.0 - (d - core) / (halo - core)).powi(2);
                    let c = self.pixels[idx];
                    let add = |s: u32, a: f64| ((((c >> s) & 0xFF) as f64 + a * g) as u32).min(255) << s;
                    self.pixels[idx] = add(16, 170.0) | add(8, 70.0) | add(0, 14.0);
                }
            }
        }
    }

    fn draw_pickup(&mut self, p: Pickup) {
        let Some((screen_x, tx, base_h)) = self.project(p.x, p.y) else {
            return;
        };
        let sz = (((SCREEN_H as f64 / tx) * 0.45) as i32).max(4);
        let dsx = (screen_x - sz as f64 / 2.0) as i32;
        let bob = (self.global_time * 3.0 + p.x + p.y).sin() * (sz as f64 * 0.08);
        let dsy = (SCREEN_H as f64 / 2.0 + sz as f64 * 0.15 + bob) as i32;
        let feet = SCREEN_H as i32 / 2 + base_h / 2;
        self.draw_shadow(screen_x, feet, sz as f64 * 0.36, tx);
        let lit = self.sprite_lit(p.x, p.y, tx);
        let kind = (p.kind as usize).min(3);
        self.draw_raster(&bank().pickups[kind], dsx, dsy, sz, sz, tx, &lit, false, 256);
    }

    fn draw_decor(&mut self, d: Decor) {
        let Some((screen_x, tx, base_h)) = self.project(d.x, d.y) else {
            return;
        };
        let scale = if d.kind == DEC_TREE { 1.3 } else { 1.0 };
        let h = (base_h as f64 * scale) as i32;
        let feet = SCREEN_H as i32 / 2 + base_h / 2;
        let dsx = (screen_x - h as f64 / 2.0) as i32;
        let shadow = match d.kind {
            DEC_CHAINS => 0.0,
            DEC_TORCH | DEC_LAMP => 0.12,
            DEC_PILLAR => 0.22,
            _ => 0.18,
        };
        if shadow > 0.0 {
            self.draw_shadow(screen_x, feet, h as f64 * shadow, tx);
        }
        let lit = self.sprite_lit(d.x, d.y, tx);
        let frame = anim_frame(std::f64::consts::TAU, decor_frames(d.kind), self.global_time * 7.0 + d.phase);
        self.draw_raster(&bank().decor[d.kind][frame], dsx, feet - h, h, h, tx, &lit, false, 256);
    }

    fn draw_particles(&mut self) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        // Camera basis is the same for every particle — compute the trig once.
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let plane_half = (FOV / 2.0).tan();
        for i in 0..MAX_PARTICLES {
            let p = self.parts[i];
            if p.life <= 0.0 {
                continue;
            }
            let dx = p.x - px;
            let dy = p.y - py;
            let tx = dx * cs - dy * sn;
            let ty = dx * sn + dy * cs;
            if tx <= 0.1 {
                continue;
            }
            let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
            let mut sz = ((SCREEN_H as f64 / tx) * 0.08) as i32;
            if sz < 1 {
                sz = 1;
            }
            let sx = screen_x as i32;
            let sy = SCREEN_H as i32 / 2;
            if sx < 0 || sx >= SCREEN_W as i32 {
                continue;
            }
            if tx >= self.depth[sx as usize] {
                continue;
            }
            let mut fade = p.life;
            if fade > 1.0 {
                fade = 1.0;
            }
            let c = shade_color(p.color, fade);
            for yy in -sz..=sz {
                for xx in -sz..=sz {
                    if xx * xx + yy * yy > sz * sz {
                        continue;
                    }
                    self.put_pixel(sx + xx, sy + yy, c);
                }
            }
        }
    }

    pub fn render_sprites(&mut self) {
        // (distance², kind, index); kind 0=enemy 1=pickup 2=fireball 3=barrel
        // 4=decoration. Fixed-capacity scratch on the stack — no per-frame
        // allocation.
        const MAX_SPRITES: usize = MAX_ENEMIES + MAX_PICKUPS + MAX_FIREBALLS + MAX_BARRELS + MAX_DECOR;
        let mut refs = [(0.0f64, 0u8, 0usize); MAX_SPRITES];
        let mut n = 0;
        let (px, py) = (self.player.x, self.player.y);
        let mut push = |x: f64, y: f64, kind: u8, i: usize| {
            refs[n] = ((x - px) * (x - px) + (y - py) * (y - py), kind, i);
            n += 1;
        };
        for (i, e) in self.enemies.iter().enumerate() {
            if e.alive || e.hit_flash > 0.0 {
                push(e.x, e.y, 0, i);
            }
        }
        for (i, p) in self.pickups.iter().enumerate() {
            if p.alive {
                push(p.x, p.y, 1, i);
            }
        }
        for (i, f) in self.fireballs.iter().enumerate() {
            if f.alive {
                push(f.x, f.y, 2, i);
            }
        }
        for (i, b) in self.barrels.iter().enumerate() {
            if b.alive {
                push(b.x, b.y, 3, i);
            }
        }
        for (i, d) in self.env.decor.iter().enumerate().take(MAX_DECOR) {
            push(d.x, d.y, 4, i);
        }
        // Far-to-near so nearer sprites overwrite farther ones (stable, so
        // equal distances keep the same draw order as before).
        let refs = &mut refs[..n];
        refs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        for &(_, kind, idx) in refs.iter() {
            match kind {
                0 => self.draw_enemy(self.enemies[idx]),
                1 => self.draw_pickup(self.pickups[idx]),
                2 => self.draw_fireball(self.fireballs[idx]),
                3 => self.draw_barrel(self.barrels[idx]),
                _ => self.draw_decor(self.env.decor[idx]),
            }
        }
        self.draw_particles();
    }
}
