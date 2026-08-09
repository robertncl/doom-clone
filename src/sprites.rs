//! Procedural enemy/pickup/fireball/particle sprites, depth-sorted and drawn
//! against the wall depth buffer. `grunt_pixel`/`imp_pixel` return `Some(color)`
//! for a drawn texel or `None` for transparent (the C `int`/`*out` pattern).

use crate::color::{make_color, shade_color};
use crate::constants::*;
use crate::game::Game;
use crate::types::{Barrel, Enemy, Fireball, Pickup};

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
    let hover = (anim * 1.6).sin() * 0.035;
    let cy = v - 0.46 - hover;
    // The whole shape drifts sideways a little, like it's swimming.
    let cx = cx0 - (anim * 1.1).sin() * 0.02;

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
        let swing = (anim * 1.6).cos() * 0.04;
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

impl Game {
    /// Composite a billboard sprite onto the framebuffer. Samples `sample(u,v)`
    /// (u,v in 0..1) with 2x2 supersampling for anti-aliased edges, darkens
    /// partial-coverage texels into a subtle outline, applies distance `shade`
    /// (or a white hit `flash`), and alpha-blends over the existing pixel by
    /// sub-pixel coverage scaled by `alpha` (1.0 = opaque; the wraith is drawn
    /// see-through). Very large/near sprites fall back to a single sample
    /// (their edges are already many pixels thick, so AA buys little and the
    /// supersample would cost the most there). Columns nearer in the wall `depth`
    /// buffer occlude the sprite.
    #[allow(clippy::too_many_arguments)]
    fn draw_sprite<F: Fn(f64, f64) -> Option<u32>>(
        &mut self,
        dsx: i32,
        dsy: i32,
        w: i32,
        h: i32,
        tx: f64,
        shade: f64,
        flash: bool,
        alpha: f64,
        sample: F,
    ) {
        let sx0 = dsx.max(0);
        let sx1 = (dsx + w).min(SCREEN_W as i32);
        let sy0 = dsy.max(0);
        let sy1 = (dsy + h).min(SCREEN_H as i32);
        let (inv_w, inv_h) = (1.0 / w as f64, 1.0 / h as f64);
        let offs: &[f64] = if h <= 240 { &[0.25, 0.75] } else { &[0.5] };
        let nn = (offs.len() * offs.len()) as i32;

        for x in sx0..sx1 {
            if tx >= self.depth[x as usize] {
                continue;
            }
            let xb = (x - dsx) as f64;
            for y in sy0..sy1 {
                let yb = (y - dsy) as f64;
                let (mut rs, mut gs, mut bs, mut cnt) = (0i32, 0i32, 0i32, 0i32);
                for &oy in offs {
                    let v = (yb + oy) * inv_h;
                    for &ox in offs {
                        let u = (xb + ox) * inv_w;
                        if let Some(c) = sample(u, v) {
                            rs += ((c >> 16) & 0xFF) as i32;
                            gs += ((c >> 8) & 0xFF) as i32;
                            bs += (c & 0xFF) as i32;
                            cnt += 1;
                        }
                    }
                }
                if cnt == 0 {
                    continue;
                }
                let cov = cnt as f64 / nn as f64 * alpha;
                let (sr, sg, sb) = if flash {
                    (255.0, 240.0, 240.0)
                } else {
                    let m = shade * (0.6 + 0.4 * cov); // rim-darken thin edges
                    ((rs / cnt) as f64 * m, (gs / cnt) as f64 * m, (bs / cnt) as f64 * m)
                };
                let idx = y as usize * SCREEN_W + x as usize;
                let dst = self.pixels[idx];
                let dr = ((dst >> 16) & 0xFF) as f64;
                let dg = ((dst >> 8) & 0xFF) as f64;
                let db = (dst & 0xFF) as f64;
                self.pixels[idx] = make_color(
                    (dr + (sr - dr) * cov) as i32,
                    (dg + (sg - dg) * cov) as i32,
                    (db + (sb - db) * cov) as i32,
                );
            }
        }
    }

    fn draw_enemy(&mut self, e: Enemy) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let dx = e.x - px;
        let dy = e.y - py;
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return;
        }

        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        // Kinds differ in bulk, but they all stand on the same floor: scale the
        // billboard about its feet so a baron towers instead of sinking.
        let base_h = (SCREEN_H as f64 / tx) as i32;
        let sprite_h = (base_h as f64 * EN_SCALE[e.kind as usize]) as i32;
        let sprite_w = sprite_h;
        let feet = SCREEN_H as i32 / 2 + base_h / 2;
        let dsx = (screen_x - sprite_w as f64 / 2.0) as i32;
        let dsy = feet - sprite_h;

        let shade = (1.0 - tx / MAX_DEPTH).max(0.25);
        let flash = e.hit_flash > 0.0;
        let alpha = if e.kind == EN_WRAITH { 0.72 } else { 1.0 };
        let (anim, kind) = (e.anim, e.kind);
        self.draw_sprite(dsx, dsy, sprite_w, sprite_h, tx, shade, flash, alpha, move |u, v| {
            match kind {
                EN_IMP => imp_pixel(u, v, anim),
                EN_WRAITH => wraith_pixel(u, v, anim),
                EN_BARON => baron_pixel(u, v, anim),
                _ => grunt_pixel(u, v, anim),
            }
        });
    }

    fn draw_barrel(&mut self, b: Barrel) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let dx = b.x - px;
        let dy = b.y - py;
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return;
        }

        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        let base_h = (SCREEN_H as f64 / tx) as i32;
        let sz = ((base_h as f64 * 0.72) as i32).max(4);
        let feet = SCREEN_H as i32 / 2 + base_h / 2;
        let dsx = (screen_x - sz as f64 / 2.0) as i32;
        let dsy = feet - sz;

        let shade = (1.0 - tx / MAX_DEPTH).max(0.3);
        self.draw_sprite(dsx, dsy, sz, sz, tx, shade, b.hit_flash > 0.0, 1.0, barrel_pixel);
    }

    fn draw_fireball(&mut self, fb: Fireball) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let dx = fb.x - px;
        let dy = fb.y - py;
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return;
        }

        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        let mut sz = ((SCREEN_H as f64 / tx) * 0.35) as i32;
        if sz < 2 {
            sz = 2;
        }
        let dsx = (screen_x - sz as f64 / 2.0) as i32;
        let dsy = -sz / 2 + SCREEN_H as i32 / 2;
        let sx0 = dsx.max(0);
        let sx1 = (dsx + sz).min(SCREEN_W as i32);
        let sy0 = dsy.max(0);
        let sy1 = (dsy + sz).min(SCREEN_H as i32);
        let r2 = (sz as f64 * 0.5) * (sz as f64 * 0.5);

        for x in sx0..sx1 {
            if tx >= self.depth[x as usize] {
                continue;
            }
            for y in sy0..sy1 {
                let pxd = x as f64 - (dsx as f64 + sz as f64 * 0.5);
                let pyd = y as f64 - (dsy as f64 + sz as f64 * 0.5);
                let d2 = pxd * pxd + pyd * pyd;
                if d2 > r2 {
                    continue;
                }
                let t = d2 / r2;
                let r = (255.0 * (1.0 - t * 0.4)) as i32;
                let g = (180.0 * (1.0 - t)) as i32;
                let b = (40.0 * (1.0 - t)) as i32;
                self.pixels[y as usize * SCREEN_W + x as usize] = make_color(r, g, b);
            }
        }
    }

    fn draw_pickup(&mut self, p: Pickup) {
        let (px, py, ang) = (self.player.x, self.player.y, self.player.angle);
        let dx = p.x - px;
        let dy = p.y - py;
        let cs = (-ang).cos();
        let sn = (-ang).sin();
        let tx = dx * cs - dy * sn;
        let ty = dx * sn + dy * cs;
        if tx <= 0.1 {
            return;
        }

        let plane_half = (FOV / 2.0).tan();
        let screen_x = (SCREEN_W as f64 / 2.0) * (1.0 + ty / (tx * plane_half));
        let mut sz = ((SCREEN_H as f64 / tx) * 0.45) as i32;
        if sz < 4 {
            sz = 4;
        }
        let dsx = (screen_x - sz as f64 / 2.0) as i32;
        let bob = (self.global_time * 3.0 + p.x + p.y).sin() * (sz as f64 * 0.08);
        let dsy = (SCREEN_H as f64 / 2.0 + sz as f64 * 0.15 + bob) as i32;

        let shade = (1.0 - tx / MAX_DEPTH).max(0.3);
        let kind = p.kind;
        self.draw_sprite(dsx, dsy, sz, sz, tx, shade, false, 1.0, move |u, v| match kind {
            PU_HEALTH => health_pixel(u, v, 0.0),
            PU_AMMO => ammo_pixel(u, v, 0.0),
            _ => weapon_pixel(u, v, kind),
        });
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
        // (distance², kind, index); kind 0=enemy 1=pickup 2=fireball 3=barrel.
        // Fixed-capacity scratch on the stack — no per-frame allocation.
        const MAX_SPRITES: usize = MAX_ENEMIES + MAX_PICKUPS + MAX_FIREBALLS + MAX_BARRELS;
        let mut refs = [(0.0f64, 0u8, 0usize); MAX_SPRITES];
        let mut n = 0;
        let (px, py) = (self.player.x, self.player.y);
        for i in 0..MAX_ENEMIES {
            let e = self.enemies[i];
            if !(e.alive || e.hit_flash > 0.0) {
                continue;
            }
            let dx = e.x - px;
            let dy = e.y - py;
            refs[n] = (dx * dx + dy * dy, 0, i);
            n += 1;
        }
        for i in 0..MAX_PICKUPS {
            if !self.pickups[i].alive {
                continue;
            }
            let dx = self.pickups[i].x - px;
            let dy = self.pickups[i].y - py;
            refs[n] = (dx * dx + dy * dy, 1, i);
            n += 1;
        }
        for i in 0..MAX_FIREBALLS {
            if !self.fireballs[i].alive {
                continue;
            }
            let dx = self.fireballs[i].x - px;
            let dy = self.fireballs[i].y - py;
            refs[n] = (dx * dx + dy * dy, 2, i);
            n += 1;
        }
        for i in 0..MAX_BARRELS {
            if !self.barrels[i].alive {
                continue;
            }
            let dx = self.barrels[i].x - px;
            let dy = self.barrels[i].y - py;
            refs[n] = (dx * dx + dy * dy, 3, i);
            n += 1;
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
                _ => self.draw_barrel(self.barrels[idx]),
            }
        }
        self.draw_particles();
    }
}
