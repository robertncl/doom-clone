//! Plain-old-data entity structs, ported from the C `typedef struct`s.
//!
//! All are `Copy` so we can read one out of an array, mutate the copy, and
//! write it back — the Rust-friendly equivalent of the C `Enemy *e = &arr[i]`
//! pointer pattern, without tangling up the borrow checker.

#[derive(Clone, Copy, Default)]
pub struct Player {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub vx: f64, // world-space velocity (smoothed)
    pub vy: f64,
    pub va: f64,  // angular velocity (smoothed turn)
    pub bob: f64, // view/weapon bob phase accumulator
    pub health: i32,
    pub armor: i32,
    pub ammo: i32,
}

#[derive(Clone, Copy, Default)]
pub struct Enemy {
    pub x: f64,
    pub y: f64,
    pub kind: i32, // EN_GRUNT / EN_IMP
    pub alive: bool,
    pub hp: i32,
    pub hit_flash: f64,
    pub atk_cool: f64,
    pub anim: f64,
}

#[derive(Clone, Copy, Default)]
pub struct Fireball {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub alive: bool,
    pub life: f64,
}

#[derive(Clone, Copy, Default)]
pub struct Particle {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub life: f64,
    pub color: u32,
}

#[derive(Clone, Copy, Default)]
pub struct Pickup {
    pub x: f64,
    pub y: f64,
    pub alive: bool,
    pub kind: i32, // PU_HEALTH / PU_AMMO
}

/// Persistent bot state. In the C version these were function-local `static`
/// variables inside `botThink`; here they live on the game so they survive
/// across frames (and, like the C statics, across game restarts).
#[derive(Clone, Copy, Default)]
pub struct Bot {
    pub restart_t: f64,
    pub last_x: f64,
    pub last_y: f64,
    pub stuck_t: f64,
    pub unstuck_t: f64,
    pub flip: bool,
    pub fire_t: f64,
}
