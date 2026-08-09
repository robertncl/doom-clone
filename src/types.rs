//! Plain-old-data entity structs.
//!
//! All are `Copy` so we can read one out of an array, mutate the copy, and
//! write it back without tangling up the borrow checker.

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
    pub weapon: i32,         // currently equipped WP_*
    pub weapons: [bool; 3],  // which WP_* the player owns (index by WP_*)
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
    /// Health taken on a hit. Baron volleys hit harder than imp spit.
    pub dmg: i32,
}

/// A fuel barrel: one hit pops it, and the blast hurts everything nearby —
/// including the player — and detonates neighbouring barrels.
#[derive(Clone, Copy, Default)]
pub struct Barrel {
    pub x: f64,
    pub y: f64,
    pub alive: bool,
    /// Counts down after a hit so the sprite flashes before it goes.
    pub hit_flash: f64,
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
    pub kind: i32, // PU_HEALTH / PU_AMMO / PU_SHOTGUN / PU_RIFLE
}

/// Persistent bot state — kept on the game so it survives across frames and
/// across game restarts, rather than as locals inside the bot's think step.
#[derive(Clone, Copy)]
pub struct Bot {
    pub restart_t: f64,
    pub last_x: f64,
    pub last_y: f64,
    pub stuck_t: f64,
    pub unstuck_t: f64,
    pub flip: bool,
    pub fire_t: f64,
    /// Index of the enemy currently being hunted (-1 = none). Gives goal
    /// selection hysteresis so two equidistant enemies behind opposite walls
    /// don't flip the pathfinding waypoint every frame.
    pub target: i32,
}

impl Default for Bot {
    fn default() -> Self {
        Bot {
            restart_t: 0.0,
            last_x: 0.0,
            last_y: 0.0,
            stuck_t: 0.0,
            unstuck_t: 0.0,
            flip: false,
            fire_t: 0.0,
            target: -1,
        }
    }
}
