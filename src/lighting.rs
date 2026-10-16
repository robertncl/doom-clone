//! Level dressing and lighting: everything about a level's look that depends
//! on its map, worked out once per level and kept in [`Environment`].
//!
//! - **Flats**: which floor and ceiling texture each cell gets (the theme's
//!   base, a rolled patch, or lava).
//! - **Wall faces**: which variant each visible face shows — plain, a detail,
//!   weathered, or a light fixture.
//! - **Decorations**: non-solid props stood against the walls.
//! - **Lightmap**: light per spot on the floor, `LM_RES` samples per cell. It
//!   starts from the theme's ambient, darkened where floor meets wall (ambient
//!   occlusion), then adds every light source that can see the spot — walls
//!   cast shadows. Light sources are sorted into layers so flames can flicker
//!   and lava can pulse each frame without redoing the shadow tests.
//!
//! The environment is rebuilt whenever the level or the map changes (see
//! [`Game::ensure_environment`]), so code that edits `cur_map` — tests do —
//! never sees lighting for a map that is no longer there.

use crate::constants::*;
use crate::game::Game;
use crate::textures::*;
use crate::theme::*;

/// Lightmap samples per map cell, per axis.
pub const LM_RES: usize = 4;
pub const LM_W: usize = MAP_W * LM_RES;
pub const LM_H: usize = MAP_H * LM_RES;

/// Light layers: one steady, three flame groups that flicker out of step with
/// each other, and the slow pulse of lava.
pub const LAYER_STEADY: usize = 0;
pub const LAYER_LAVA: usize = 4;
pub const LAYERS: usize = 5;

/// Fixed-point light: 256 is the texture as painted; lights may push a spot
/// up to twice that.
pub const LIGHT_ONE: i32 = 256;
pub const LIGHT_MAX: i32 = 512;

/// Wall faces by the direction they face: -X, +X, -Y, +Y.
pub const FACE_NEG_X: usize = 0;
pub const FACE_POS_X: usize = 1;
pub const FACE_NEG_Y: usize = 2;
pub const FACE_POS_Y: usize = 3;
const FACE_DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Index used for "outside the map" in the per-cell tables.
pub const OUTSIDE: usize = MAP_W * MAP_H;

/// Burst lights (barrel explosions) alive at once.
pub const MAX_FLASHES: usize = 8;
/// Most decorations a level can hold (themes ask for at most ten).
pub const MAX_DECOR: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub x: f64,
    pub y: f64,
    /// Colour and strength together: 1.0 per channel is a bright light.
    pub color: [f64; 3],
    pub radius: f64,
    pub layer: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Decor {
    pub x: f64,
    pub y: f64,
    pub kind: usize,
    /// Animation offset, so neighbouring flames don't move in lockstep.
    pub phase: f64,
}

/// A short-lived light from an explosion.
#[derive(Clone, Copy, Default, Debug)]
pub struct Flash {
    pub x: f64,
    pub y: f64,
    pub life: f64,
}

pub struct Environment {
    /// The (level, map) this was built for; `None` until the first build.
    built_for: Option<(i32, [[u8; MAP_W]; MAP_H])>,
    pub level: i32,
    pub sky: Option<usize>,
    pub fog: [u32; 3],
    pub fog_density: f64,
    /// Flat index per cell, plus one entry for outside the map.
    pub floor_flat: [u8; OUTSIDE + 1],
    pub ceil_flat: [u8; OUTSIDE + 1],
    /// Per wall cell and face: which `WV_*` variant shows, and which light
    /// layer drives its glow.
    pub face_variant: [[u8; 4]; OUTSIDE],
    pub face_layer: [[u8; 4]; OUTSIDE],
    pub lights: Vec<Light>,
    pub decor: Vec<Decor>,
    /// Baked light per layer, `[layer][texel]`, RGB.
    layers: Vec<[f32; 3]>,
    /// This frame's light: the layers mixed at their current flicker, plus
    /// moving lights. Fixed point (`LIGHT_ONE` = 1.0).
    pub lightmap: Vec<[i32; 3]>,
    /// This frame's multiplier for each layer — glowing texels on a fixture
    /// use the same value as the light it throws.
    pub flicker: [f64; LAYERS],
}

impl Default for Environment {
    fn default() -> Self {
        Environment {
            built_for: None,
            level: 0,
            sky: None,
            fog: [0; 3],
            fog_density: 0.06,
            floor_flat: [FLAT_TILE as u8; OUTSIDE + 1],
            ceil_flat: [FLAT_BEAMS as u8; OUTSIDE + 1],
            face_variant: [[WV_BASE as u8; 4]; OUTSIDE],
            face_layer: [[LAYER_STEADY as u8; 4]; OUTSIDE],
            lights: Vec::new(),
            decor: Vec::new(),
            layers: vec![[0.0; 3]; LAYERS * LM_W * LM_H],
            lightmap: vec![[LIGHT_ONE; 3]; LM_W * LM_H],
            flicker: [1.0; LAYERS],
        }
    }
}

/// Mix level, position and a salt into one well-spread hash.
fn roll(level: i32, x: i32, y: i32, salt: u32) -> u32 {
    let mut h = (level as u32).wrapping_mul(0x9E37_79B1)
        ^ (x as u32).wrapping_mul(0x85EB_CA77)
        ^ (y as u32).wrapping_mul(0xC2B2_AE3D)
        ^ salt.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// The same wall test as `Game::map_wall_type`, over a bare map.
pub fn is_wall(map: &[[u8; MAP_W]; MAP_H], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= MAP_W as i32 || y >= MAP_H as i32 {
        return true;
    }
    matches!(map[y as usize][x as usize], b'#' | b'=' | b'B' | b'D' | b'H' | b'T')
}

fn wall_kind(c: u8) -> usize {
    match c {
        b'=' => WALL_BRICK,
        b'B' => WALL_METAL,
        b'D' => WALL_WOOD,
        b'H' => WALL_HELL,
        b'T' => WALL_TECH,
        _ => WALL_STONE,
    }
}

/// True if nothing solid lies on the straight line between two points: a grid
/// walk (Amanatides–Woo) through every cell the segment crosses, so light
/// can't slip through a wall corner.
pub fn line_of_sight(map: &[[u8; MAP_W]; MAP_H], x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let (mut cx, mut cy) = (x0.floor() as i32, y0.floor() as i32);
    let (tx, ty) = (x1.floor() as i32, y1.floor() as i32);
    let (dx, dy) = (x1 - x0, y1 - y0);
    let step_x = if dx > 0.0 { 1 } else { -1 };
    let step_y = if dy > 0.0 { 1 } else { -1 };
    let t_dx = if dx != 0.0 { (1.0 / dx).abs() } else { f64::INFINITY };
    let t_dy = if dy != 0.0 { (1.0 / dy).abs() } else { f64::INFINITY };
    let mut t_x = if dx > 0.0 { (cx as f64 + 1.0 - x0) * t_dx } else { (x0 - cx as f64) * t_dx };
    let mut t_y = if dy > 0.0 { (cy as f64 + 1.0 - y0) * t_dy } else { (y0 - cy as f64) * t_dy };
    for _ in 0..(MAP_W + MAP_H) * 2 {
        if cx == tx && cy == ty {
            return true;
        }
        if t_x < t_y {
            t_x += t_dx;
            cx += step_x;
        } else {
            t_y += t_dy;
            cy += step_y;
        }
        if is_wall(map, cx, cy) {
            return cx == tx && cy == ty; // the end point itself may sit in a wall
        }
    }
    true
}

/// Darkening where floor meets wall: 1.0 in the open, down to ~0.6 tucked in
/// a corner. Applied to ambient light only, so a torch still lights its own
/// corner.
pub(crate) fn ambient_occlusion(map: &[[u8; MAP_W]; MAP_H], wx: f64, wy: f64) -> f64 {
    let (cx, cy) = (wx.floor() as i32, wy.floor() as i32);
    let (fx, fy) = (wx - cx as f64, wy - cy as f64);
    let near = |d: f64| {
        let t = (1.0 - d / 0.55).max(0.0);
        1.0 - 0.36 * t * t
    };
    let w = |x: i32, y: i32| is_wall(map, x, y);
    let mut occ = 1.0;
    if w(cx - 1, cy) {
        occ *= near(fx);
    }
    if w(cx + 1, cy) {
        occ *= near(1.0 - fx);
    }
    if w(cx, cy - 1) {
        occ *= near(fy);
    }
    if w(cx, cy + 1) {
        occ *= near(1.0 - fy);
    }
    // Outer corners: a diagonal wall whose two neighbours are open.
    for (dx, dy, ddx, ddy) in [(-1, -1, fx, fy), (1, -1, 1.0 - fx, fy), (-1, 1, fx, 1.0 - fy), (1, 1, 1.0 - fx, 1.0 - fy)] {
        if w(cx + dx, cy + dy) && !w(cx + dx, cy) && !w(cx, cy + dy) {
            occ *= near((ddx * ddx + ddy * ddy).sqrt());
        }
    }
    occ
}

/// Light thrown by a light-fixture face of each wall material.
fn fixture_light(kind: usize) -> ([f64; 3], f64, bool) {
    // (colour, radius, flickers)
    match kind {
        WALL_METAL => ([0.66, 0.78, 0.95], 4.6, false),
        WALL_WOOD => ([0.95, 0.66, 0.34], 4.2, true),
        WALL_HELL => ([1.0, 0.36, 0.12], 4.4, true),
        WALL_TECH => ([0.32, 0.86, 0.6], 4.0, false),
        _ => ([1.0, 0.6, 0.27], 4.6, true), // torch
    }
}

/// Light thrown by a decoration, if it gives any.
pub fn decor_light(kind: usize) -> Option<([f64; 3], f64, bool)> {
    match kind {
        DEC_TORCH => Some(([1.0, 0.6, 0.28], 4.4, true)),
        DEC_LAMP => Some(([0.7, 0.82, 1.0], 4.4, false)),
        DEC_BRAZIER => Some(([1.0, 0.44, 0.14], 4.8, true)),
        _ => None,
    }
}

impl Environment {
    /// Work out a level's dressing and bake its lightmap. `keep_clear` lists
    /// cells a decoration must not stand in (the spawn and the pickups).
    pub fn build(level: i32, map: &[[u8; MAP_W]; MAP_H], keep_clear: &[(i32, i32)]) -> Environment {
        let theme = theme_for(level);
        let mut env = Environment {
            built_for: Some((level, *map)),
            level,
            sky: theme.sky,
            fog: [theme.fog[0] as u32, theme.fog[1] as u32, theme.fog[2] as u32],
            fog_density: theme.fog_density,
            ..Environment::default()
        };

        // ---- flats ----
        let pick = |base: usize, patches: &[(usize, u8)], h: u32| -> u8 {
            let mut acc = 0u32;
            let r = h % 256;
            for &(flat, chance) in patches {
                acc += chance as u32;
                if r < acc {
                    return flat as u8;
                }
            }
            base as u8
        };
        for (cy, row) in map.iter().enumerate() {
            for (cx, &cell) in row.iter().enumerate() {
                let i = cy * MAP_W + cx;
                let (x, y) = (cx as i32, cy as i32);
                env.floor_flat[i] = if cell == b'~' {
                    FLAT_LAVA as u8
                } else {
                    pick(theme.floor, theme.floor_patches, roll(level, x, y, 1))
                };
                env.ceil_flat[i] = pick(theme.ceil, theme.ceil_patches, roll(level, x, y, 2));
            }
        }
        env.floor_flat[OUTSIDE] = theme.floor as u8;
        env.ceil_flat[OUTSIDE] = theme.ceil as u8;

        // ---- wall faces ----
        // Visit every visible face in a hashed order (so fixtures don't pile up
        // in one corner of the map), spacing lights at least `GAP` apart.
        const GAP: f64 = 3.2;
        let mut faces = Vec::new();
        for cy in 0..MAP_H as i32 {
            for cx in 0..MAP_W as i32 {
                if !is_wall(map, cx, cy) {
                    continue;
                }
                for (f, (dx, dy)) in FACE_DIRS.iter().enumerate() {
                    if !is_wall(map, cx + dx, cy + dy) {
                        faces.push((roll(level, cx, cy, 10 + f as u32), cx, cy, f));
                    }
                }
            }
        }
        faces.sort_unstable();
        let lc = theme.light_chance as u32;
        let dc = lc + theme.detail_chance as u32;
        let wc = dc + theme.worn_chance as u32;
        for &(h, cx, cy, f) in &faces {
            let r = (h >> 8) % 256;
            let (dx, dy) = FACE_DIRS[f];
            let i = cy as usize * MAP_W + cx as usize;
            let variant = if r < lc {
                // Light position: just off the face, into the open cell.
                let lx = cx as f64 + 0.5 + dx as f64 * 0.78;
                let ly = cy as f64 + 0.5 + dy as f64 * 0.78;
                let spaced = env.lights.iter().all(|l| (l.x - lx).hypot(l.y - ly) >= GAP);
                if spaced {
                    let kind = wall_kind(map[cy as usize][cx as usize]);
                    let (color, radius, flickers) = fixture_light(kind);
                    let layer = if flickers { 1 + (h % 3) as usize } else { LAYER_STEADY };
                    env.lights.push(Light { x: lx, y: ly, color, radius, layer });
                    env.face_layer[i][f] = layer as u8;
                    WV_LIGHT
                } else {
                    WV_DETAIL
                }
            } else if r < dc {
                WV_DETAIL
            } else if r < wc {
                WV_WORN
            } else {
                WV_BASE
            };
            env.face_variant[i][f] = variant as u8;
        }

        // ---- lava glow ----
        for (cy, row) in map.iter().enumerate() {
            for (cx, &cell) in row.iter().enumerate() {
                if cell == b'~' {
                    env.lights.push(Light {
                        x: cx as f64 + 0.5,
                        y: cy as f64 + 0.5,
                        color: [0.62, 0.26, 0.07],
                        radius: 2.6,
                        layer: LAYER_LAVA,
                    });
                }
            }
        }

        // ---- decorations ----
        let total_weight: u32 = theme.decor.iter().map(|d| d.1 as u32).sum();
        if total_weight > 0 && theme.decor_count > 0 {
            let mut spots = Vec::new();
            for cy in 0..MAP_H as i32 {
                for cx in 0..MAP_W as i32 {
                    if is_wall(map, cx, cy) || map[cy as usize][cx as usize] == b'~' || keep_clear.contains(&(cx, cy)) {
                        continue;
                    }
                    let walls: Vec<(i32, i32)> =
                        FACE_DIRS.iter().copied().filter(|&(dx, dy)| is_wall(map, cx + dx, cy + dy)).collect();
                    // Against a wall, but not boxed into a dead-end nook.
                    if walls.is_empty() || walls.len() > 2 {
                        continue;
                    }
                    spots.push((roll(level, cx, cy, 40), cx, cy, walls));
                }
            }
            spots.sort_unstable_by_key(|s| s.0);
            for (h, cx, cy, walls) in spots {
                if env.decor.len() >= theme.decor_count.min(MAX_DECOR) {
                    break;
                }
                // Hug the wall(s): shift from the cell centre towards each.
                let (mut x, mut y) = (cx as f64 + 0.5, cy as f64 + 0.5);
                for (dx, dy) in &walls {
                    x += *dx as f64 * 0.24;
                    y += *dy as f64 * 0.24;
                }
                if env.decor.iter().any(|d| (d.x - x).hypot(d.y - y) < 2.4) {
                    continue;
                }
                let mut pick = (h >> 8) % total_weight;
                let mut kind = theme.decor[0].0;
                for &(k, w) in theme.decor {
                    if pick < w as u32 {
                        kind = k;
                        break;
                    }
                    pick -= w as u32;
                }
                if let Some((color, radius, flickers)) = decor_light(kind) {
                    // A lit prop too close to another light becomes the
                    // theme's first unlit prop instead, if it has one.
                    if env.lights.iter().any(|l| l.layer != LAYER_LAVA && (l.x - x).hypot(l.y - y) < 2.6) {
                        match theme.decor.iter().find(|d| decor_light(d.0).is_none()) {
                            Some(&(k, _)) => kind = k,
                            None => continue,
                        }
                    } else {
                        let layer = if flickers { 1 + (h % 3) as usize } else { LAYER_STEADY };
                        env.lights.push(Light { x, y, color, radius, layer });
                    }
                }
                let phase = (h % 1000) as f64 * 0.0063;
                env.decor.push(Decor { x, y, kind, phase });
            }
        }

        env.bake(map, theme.ambient);
        env
    }

    /// Fill the light layers: ambient × occlusion into the steady layer, then
    /// every light into its own layer wherever it has line of sight.
    fn bake(&mut self, map: &[[u8; MAP_W]; MAP_H], ambient: [f64; 3]) {
        let n = LM_W * LM_H;
        let mut open = vec![false; n];
        for ty in 0..LM_H {
            for tx in 0..LM_W {
                let (wx, wy) = ((tx as f64 + 0.5) / LM_RES as f64, (ty as f64 + 0.5) / LM_RES as f64);
                let i = ty * LM_W + tx;
                if is_wall(map, wx as i32, wy as i32) {
                    continue;
                }
                open[i] = true;
                let ao = ambient_occlusion(map, wx, wy);
                self.layers[LAYER_STEADY * n + i] = ambient.map(|a| (a * ao) as f32);
            }
        }
        for l in &self.lights {
            let r = l.radius;
            let (tx0, tx1) = (((l.x - r) * LM_RES as f64).floor().max(0.0) as usize, (((l.x + r) * LM_RES as f64).ceil() as usize).min(LM_W));
            let (ty0, ty1) = (((l.y - r) * LM_RES as f64).floor().max(0.0) as usize, (((l.y + r) * LM_RES as f64).ceil() as usize).min(LM_H));
            for ty in ty0..ty1 {
                for tx in tx0..tx1 {
                    let i = ty * LM_W + tx;
                    if !open[i] {
                        continue;
                    }
                    let (wx, wy) = ((tx as f64 + 0.5) / LM_RES as f64, (ty as f64 + 0.5) / LM_RES as f64);
                    let d = (wx - l.x).hypot(wy - l.y);
                    if d >= r || !line_of_sight(map, l.x, l.y, wx, wy) {
                        continue;
                    }
                    let t = d / r;
                    let fall = (1.0 - t * t).powi(2);
                    for c in 0..3 {
                        self.layers[l.layer * n + i][c] += (l.color[c] * fall) as f32;
                    }
                }
            }
        }
        // Texels inside walls take the average of their open neighbours, so
        // bilinear sampling right at a wall face blends in light from the
        // room rather than the black inside of the wall.
        for layer in 0..LAYERS {
            let base = layer * n;
            for ty in 0..LM_H {
                for tx in 0..LM_W {
                    let i = ty * LM_W + tx;
                    if open[i] {
                        continue;
                    }
                    let (mut sum, mut cnt) = ([0.0f32; 3], 0.0f32);
                    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
                        let (nx, ny) = (tx as i32 + dx, ty as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= LM_W as i32 || ny >= LM_H as i32 {
                            continue;
                        }
                        let j = ny as usize * LM_W + nx as usize;
                        if open[j] {
                            for (s, v) in sum.iter_mut().zip(self.layers[base + j]) {
                                *s += v;
                            }
                            cnt += 1.0;
                        }
                    }
                    if cnt > 0.0 {
                        self.layers[base + i] = sum.map(|s| s / cnt);
                    }
                }
            }
        }
    }

    /// Bilinear light at world position `(wx, wy)`, fixed point per channel.
    #[inline]
    pub fn light_at(&self, wx: f64, wy: f64) -> [i32; 3] {
        let fx = (wx * LM_RES as f64 - 0.5).clamp(0.0, (LM_W - 1) as f64);
        let fy = (wy * LM_RES as f64 - 0.5).clamp(0.0, (LM_H - 1) as f64);
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(LM_W - 1), (y0 + 1).min(LM_H - 1));
        let wx = ((fx - x0 as f64) * 256.0) as i32;
        let wy = ((fy - y0 as f64) * 256.0) as i32;
        let lm = &self.lightmap;
        let (a, b, c, d) = (lm[y0 * LM_W + x0], lm[y0 * LM_W + x1], lm[y1 * LM_W + x0], lm[y1 * LM_W + x1]);
        let mut out = [0i32; 3];
        for k in 0..3 {
            let top = a[k] * (256 - wx) + b[k] * wx;
            let bot = c[k] * (256 - wx) + d[k] * wx;
            out[k] = (top * (256 - wy) + bot * wy) >> 16;
        }
        out
    }
}

/// Flame flicker for layer group `g` at time `t`: a few incommensurate sines,
/// so it never visibly repeats. Stays within about 0.66..1.0.
pub(crate) fn flame_flicker(t: f64, g: usize) -> f64 {
    let p = g as f64 * 1.9;
    let w = (t * 7.3 + p).sin() * 0.55 + (t * 17.9 + p * 2.0).sin() * 0.3 + (t * 31.1 + p * 3.0).sin() * 0.15;
    0.83 + 0.17 * w
}

impl Game {
    /// Rebuild the environment if the level or the map has changed since it
    /// was last built. Cheap when nothing changed (one 256-byte compare).
    pub fn ensure_environment(&mut self) {
        if self.env.built_for == Some((self.level, self.cur_map)) {
            return;
        }
        let mut clear: Vec<(i32, i32)> = vec![(self.player.x as i32, self.player.y as i32)];
        clear.extend(self.pickups.iter().filter(|p| p.alive).map(|p| (p.x as i32, p.y as i32)));
        clear.extend(self.barrels.iter().filter(|b| b.alive).map(|b| (b.x as i32, b.y as i32)));
        self.env = Environment::build(self.level, &self.cur_map, &clear);
    }

    /// This frame's lightmap: the baked layers at their current flicker, plus
    /// light from the muzzle flash, fireballs in flight and explosions.
    pub fn update_lighting(&mut self) {
        self.ensure_environment();
        let t = self.global_time;
        let env = &mut self.env;
        env.flicker[LAYER_STEADY] = 1.0;
        for g in 1..LAYER_LAVA {
            env.flicker[g] = flame_flicker(t, g);
        }
        env.flicker[LAYER_LAVA] = 0.9 + 0.1 * (t * 2.3).sin();

        let n = LM_W * LM_H;
        let f: [f32; LAYERS] = env.flicker.map(|v| v as f32);
        for i in 0..n {
            let mut acc = [0.0f32; 3];
            for (l, fl) in f.iter().enumerate() {
                let s = env.layers[l * n + i];
                acc[0] += s[0] * fl;
                acc[1] += s[1] * fl;
                acc[2] += s[2] * fl;
            }
            env.lightmap[i] = acc.map(|v| ((v * LIGHT_ONE as f32) as i32).clamp(0, LIGHT_MAX));
        }

        // Moving lights: (x, y, colour, radius).
        let mut dynamic: Vec<(f64, f64, [f64; 3], f64)> = Vec::new();
        if self.muzzle_flash > 0 {
            // At the player, not out at the muzzle: pressed against a wall the
            // muzzle is inside it, and light from there would leak through.
            let k = self.muzzle_flash as f64 / 5.0;
            dynamic.push((self.player.x, self.player.y, [0.95 * k, 0.78 * k, 0.5 * k], 4.2));
        }
        for fb in self.fireballs.iter().filter(|f| f.alive) {
            dynamic.push((fb.x, fb.y, [0.9, 0.45, 0.14], 2.8));
        }
        for fl in self.flashes.iter().filter(|f| f.life > 0.0) {
            let k = (fl.life / 0.6).min(1.0);
            dynamic.push((fl.x, fl.y, [1.6 * k, 1.0 * k, 0.45 * k], 5.0));
        }
        let map = &self.cur_map;
        for (lx, ly, color, r) in dynamic {
            let tx0 = ((lx - r) * LM_RES as f64).floor().max(0.0) as usize;
            let tx1 = (((lx + r) * LM_RES as f64).ceil().max(0.0) as usize).min(LM_W);
            let ty0 = ((ly - r) * LM_RES as f64).floor().max(0.0) as usize;
            let ty1 = (((ly + r) * LM_RES as f64).ceil().max(0.0) as usize).min(LM_H);
            for ty in ty0..ty1 {
                for tx in tx0..tx1 {
                    let (wx, wy) = ((tx as f64 + 0.5) / LM_RES as f64, (ty as f64 + 0.5) / LM_RES as f64);
                    let d = (wx - lx).hypot(wy - ly);
                    if d >= r || is_wall(map, wx as i32, wy as i32) || !line_of_sight(map, lx, ly, wx, wy) {
                        continue;
                    }
                    let tt = d / r;
                    let fall = (1.0 - tt * tt).powi(2);
                    let px = &mut env.lightmap[ty * LM_W + tx];
                    for c in 0..3 {
                        px[c] = (px[c] + (color[c] * fall * LIGHT_ONE as f64) as i32).min(LIGHT_MAX);
                    }
                }
            }
        }
    }

    /// Record an explosion's light burst (fades over ~0.6 s).
    pub fn add_flash(&mut self, x: f64, y: f64) {
        if let Some(slot) = self.flashes.iter_mut().min_by(|a, b| a.life.total_cmp(&b.life)) {
            *slot = Flash { x, y, life: 0.6 };
        }
    }
}
