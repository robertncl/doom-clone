//! Compile-time constants shared across the game.

pub const SCREEN_W: usize = 640;
pub const SCREEN_H: usize = 400;
pub const WIN_SCALE: usize = 2;
pub const MAP_W: usize = 16;
pub const MAP_H: usize = 16;
pub const FOV: f64 = std::f64::consts::PI / 3.0;
pub const MAX_DEPTH: f64 = 24.0;
pub const TEX_SIZE: usize = 128; // power of two (sampler wrap masks with TEX_SIZE-1)

// Player movement feel: velocity is smoothed toward a target each frame so
// starts/stops glide instead of snapping. Accel/friction are per-second rates
// used as exponential-smoothing factors (higher = snappier).
pub const MOVE_SPEED: f64 = 3.0; // max walk speed (world units / sec)
pub const MOVE_ACCEL: f64 = 14.0; // how fast velocity ramps toward target
pub const MOVE_FRICTION: f64 = 16.0; // how fast velocity decays when no input
pub const TURN_SPEED: f64 = 2.7; // max turn rate (radians / sec)
pub const TURN_ACCEL: f64 = 16.0; // how fast turn-rate ramps up
pub const TURN_FRICTION: f64 = 18.0; // how fast turn-rate decays when released

pub const LEVEL_COUNT: usize = 5;
pub const MAX_ENEMIES: usize = 16;
pub const MAX_PARTICLES: usize = 192;
pub const MAX_FIREBALLS: usize = 16;
pub const MAX_PICKUPS: usize = 16;
pub const MAX_HIGHSCORES: usize = 5;
pub const HIGHSCORE_FILE: &str = "doom_scores.dat";

// Input action indices (the old `K_*` enum). Used to index `keys`/`key_edge`.
pub const K_FWD: usize = 0;
pub const K_BACK: usize = 1;
pub const K_STRAFEL: usize = 2;
pub const K_STRAFER: usize = 3;
pub const K_TURNL: usize = 4;
pub const K_TURNR: usize = 5;
pub const K_SHOOT: usize = 6;
pub const K_RESTART: usize = 7;
pub const K_QUIT: usize = 8;
pub const K_WEAPON1: usize = 9; // select pistol
pub const K_WEAPON2: usize = 10; // select shotgun
pub const K_WEAPON3: usize = 11; // select rifle
pub const K_COUNT: usize = 12;

// Wall kinds (old `WALL_*` enum). Index 0 = none (empty space).
pub const WALL_NONE: usize = 0;
pub const WALL_STONE: usize = 1;
pub const WALL_BRICK: usize = 2;
pub const WALL_METAL: usize = 3;
pub const WALL_WOOD: usize = 4;
pub const WALL_HELL: usize = 5;
pub const WALL_KIND_MAX: usize = 6;

// Enemy and pickup kinds.
pub const EN_GRUNT: i32 = 0;
pub const EN_IMP: i32 = 1;
pub const PU_HEALTH: i32 = 0;
pub const PU_AMMO: i32 = 1;
pub const PU_SHOTGUN: i32 = 2;
pub const PU_RIFLE: i32 = 3;

// Weapon kinds (the player's equipped gun). The pistol is the starting weapon;
// the shotgun and rifle are found as pickups.
pub const WP_PISTOL: i32 = 0;
pub const WP_SHOTGUN: i32 = 1;
pub const WP_RIFLE: i32 = 2;
pub const WP_COUNT: usize = 3;

// Sound kinds (old `SND_*` enum).
pub const SND_SHOOT: usize = 0;
pub const SND_HIT: usize = 1;
pub const SND_DEATH: usize = 2;
pub const SND_PICKUP_HEALTH: usize = 3;
pub const SND_PICKUP_AMMO: usize = 4;
pub const SND_FIREBALL: usize = 5;
pub const SND_PLAYER_HURT: usize = 6;
pub const SND_LEVEL_CLEAR: usize = 7;
pub const SND_GAME_OVER: usize = 8;
pub const SND_PICKUP_WEAPON: usize = 9;
pub const SND_KIND_MAX: usize = 10;
