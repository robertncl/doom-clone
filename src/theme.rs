//! Per-level look: what each level's floor, ceiling (or sky), fog, ambient
//! light and decoration are made of.
//!
//! A theme only changes how a level *looks*. Geometry, spawns and every
//! gameplay rule come from the map, so swapping a theme can never make a level
//! harder or unwinnable. All the variation a theme asks for (which floor cells
//! get a patch, which wall faces carry a torch, where the braziers stand) is
//! rolled from a hash of the level and cell, so a level looks the same on every
//! visit and in every test.

use crate::constants::LEVEL_COUNT;

// ---- Flat textures (floors and ceilings), indices into `Textures::flats` ----
pub const FLAT_TILE: usize = 0;
pub const FLAT_TILE_CRACKED: usize = 1;
pub const FLAT_TILE_BLOOD: usize = 2;
pub const FLAT_COBBLE: usize = 3;
pub const FLAT_COBBLE_MOSS: usize = 4;
pub const FLAT_COBBLE_PUDDLE: usize = 5;
pub const FLAT_DIRT: usize = 6;
pub const FLAT_DIRT_BONES: usize = 7;
pub const FLAT_GRATE: usize = 8;
pub const FLAT_PLATE: usize = 9;
pub const FLAT_PLATE_HAZARD: usize = 10;
pub const FLAT_HELLROCK: usize = 11;
pub const FLAT_HELLROCK_BLOOD: usize = 12;
pub const FLAT_LAVA: usize = 13;
pub const FLAT_BEAMS: usize = 14;
pub const FLAT_VAULT: usize = 15;
pub const FLAT_CEIL_PANEL: usize = 16;
pub const FLAT_CEIL_LIGHT: usize = 17;
pub const FLAT_PIPES: usize = 18;
pub const FLAT_COUNT: usize = 19;

// ---- Skies (panoramas seen above the walls on outdoor levels) ----
pub const SKY_DUSK: usize = 0;
pub const SKY_HELL: usize = 1;
pub const SKY_NIGHT: usize = 2;
pub const SKY_EMBER: usize = 3;
pub const SKY_COUNT: usize = 4;

// ---- Decorations: non-solid props that stand against the walls ----
pub const DEC_TORCH: usize = 0;
pub const DEC_LAMP: usize = 1;
pub const DEC_SKULLS: usize = 2;
pub const DEC_TREE: usize = 3;
pub const DEC_CHAINS: usize = 4;
pub const DEC_BRAZIER: usize = 5;
pub const DEC_PILLAR: usize = 6;
pub const DEC_KIND_MAX: usize = 7;

pub struct Theme {
    pub name: &'static str,
    /// Base floor, plus patches: `(flat, chance in 256)` rolled per cell.
    pub floor: usize,
    pub floor_patches: &'static [(usize, u8)],
    /// Base ceiling and its patches. Ignored when the level has a sky.
    pub ceil: usize,
    pub ceil_patches: &'static [(usize, u8)],
    /// Open sky instead of a ceiling.
    pub sky: Option<usize>,
    /// Colour far geometry fades into; outdoor themes match the sky's horizon.
    pub fog: [u8; 3],
    /// Fog thickness: visibility falls off as `exp(-density * distance)`.
    pub fog_density: f64,
    /// Light everywhere before any lamp is counted (1.0 = texture as painted).
    pub ambient: [f64; 3],
    /// Per visible wall face, chance in 256 of a light fixture (spaced apart),
    /// a decorative variant, or a weathered one.
    pub light_chance: u8,
    pub detail_chance: u8,
    pub worn_chance: u8,
    /// Decorations to scatter: `(DEC_* kind, weight)`, and how many.
    pub decor: &'static [(usize, u8)],
    pub decor_count: usize,
}

/// One theme per level, in level order.
pub static THEMES: [Theme; LEVEL_COUNT] = [
    // 1 — an outpost at dusk: cobbled yard under a burning sunset.
    Theme {
        name: "OUTPOST",
        floor: FLAT_COBBLE,
        floor_patches: &[(FLAT_COBBLE_MOSS, 40), (FLAT_COBBLE_PUDDLE, 22)],
        ceil: FLAT_BEAMS,
        ceil_patches: &[],
        sky: Some(SKY_DUSK),
        fog: [92, 58, 70],
        fog_density: 0.045,
        ambient: [0.86, 0.76, 0.70],
        light_chance: 34,
        detail_chance: 26,
        worn_chance: 40,
        decor: &[(DEC_TORCH, 3), (DEC_TREE, 2), (DEC_PILLAR, 1)],
        decor_count: 7,
    },
    // 2 — a metal base: grated floors and strip-lit ceilings.
    Theme {
        name: "BASE",
        floor: FLAT_GRATE,
        floor_patches: &[(FLAT_PLATE, 60), (FLAT_PLATE_HAZARD, 18)],
        ceil: FLAT_CEIL_PANEL,
        ceil_patches: &[(FLAT_CEIL_LIGHT, 34)],
        sky: None,
        fog: [14, 20, 30],
        fog_density: 0.075,
        ambient: [0.58, 0.64, 0.74],
        light_chance: 40,
        detail_chance: 40,
        worn_chance: 30,
        decor: &[(DEC_LAMP, 3), (DEC_PILLAR, 1)],
        decor_count: 6,
    },
    // 3 — catacombs: dark vaults, bones underfoot, torches the only light.
    Theme {
        name: "CATACOMBS",
        floor: FLAT_DIRT,
        floor_patches: &[(FLAT_DIRT_BONES, 34), (FLAT_COBBLE, 40)],
        ceil: FLAT_VAULT,
        ceil_patches: &[],
        sky: None,
        fog: [10, 7, 6],
        fog_density: 0.11,
        ambient: [0.36, 0.33, 0.32],
        light_chance: 56,
        detail_chance: 22,
        worn_chance: 60,
        decor: &[(DEC_TORCH, 3), (DEC_SKULLS, 3), (DEC_CHAINS, 2)],
        decor_count: 10,
    },
    // 4 — a hell courtyard under a blood-red sky.
    Theme {
        name: "HELL COURT",
        floor: FLAT_HELLROCK,
        floor_patches: &[(FLAT_HELLROCK_BLOOD, 30), (FLAT_DIRT_BONES, 20)],
        ceil: FLAT_VAULT,
        ceil_patches: &[],
        sky: Some(SKY_HELL),
        fog: [96, 26, 18],
        fog_density: 0.05,
        ambient: [0.80, 0.56, 0.50],
        light_chance: 30,
        detail_chance: 34,
        worn_chance: 44,
        decor: &[(DEC_BRAZIER, 3), (DEC_SKULLS, 2), (DEC_TREE, 1)],
        decor_count: 8,
    },
    // 5 — ruins at night: moonlit stone, warm torches in the cold.
    Theme {
        name: "NIGHT RUINS",
        floor: FLAT_COBBLE,
        floor_patches: &[(FLAT_COBBLE_MOSS, 60), (FLAT_DIRT, 34), (FLAT_COBBLE_PUDDLE, 20)],
        ceil: FLAT_VAULT,
        ceil_patches: &[],
        sky: Some(SKY_NIGHT),
        fog: [18, 26, 46],
        fog_density: 0.06,
        ambient: [0.44, 0.50, 0.66],
        light_chance: 44,
        detail_chance: 24,
        worn_chance: 54,
        decor: &[(DEC_TORCH, 3), (DEC_TREE, 2), (DEC_PILLAR, 2)],
        decor_count: 9,
    },
    // 6 — the bastion: industrial plating, cold lamps, hazard markings.
    Theme {
        name: "BASTION",
        floor: FLAT_PLATE,
        floor_patches: &[(FLAT_GRATE, 50), (FLAT_PLATE_HAZARD, 26)],
        ceil: FLAT_PIPES,
        ceil_patches: &[(FLAT_CEIL_LIGHT, 24)],
        sky: None,
        fog: [16, 22, 30],
        fog_density: 0.08,
        ambient: [0.52, 0.56, 0.64],
        light_chance: 36,
        detail_chance: 44,
        worn_chance: 40,
        decor: &[(DEC_LAMP, 3), (DEC_CHAINS, 1), (DEC_PILLAR, 1)],
        decor_count: 6,
    },
    // 7 — the hell gauntlet: lava light under an ember sky.
    Theme {
        name: "GAUNTLET",
        floor: FLAT_HELLROCK,
        floor_patches: &[(FLAT_HELLROCK_BLOOD, 34), (FLAT_DIRT, 24)],
        ceil: FLAT_VAULT,
        ceil_patches: &[],
        sky: Some(SKY_EMBER),
        fog: [70, 30, 16],
        fog_density: 0.06,
        ambient: [0.62, 0.48, 0.42],
        light_chance: 30,
        detail_chance: 36,
        worn_chance: 50,
        decor: &[(DEC_BRAZIER, 3), (DEC_SKULLS, 2), (DEC_CHAINS, 1)],
        decor_count: 8,
    },
    // 8 — the foundry: tech plating, coolant light and furnace glow.
    Theme {
        name: "FOUNDRY",
        floor: FLAT_PLATE,
        floor_patches: &[(FLAT_PLATE_HAZARD, 36), (FLAT_GRATE, 44)],
        ceil: FLAT_PIPES,
        ceil_patches: &[(FLAT_CEIL_LIGHT, 30)],
        sky: None,
        fog: [20, 24, 24],
        fog_density: 0.08,
        ambient: [0.46, 0.50, 0.52],
        light_chance: 44,
        detail_chance: 40,
        worn_chance: 40,
        decor: &[(DEC_LAMP, 2), (DEC_PILLAR, 1), (DEC_CHAINS, 1)],
        decor_count: 6,
    },
    // 9 — the inferno core: a burning sky over rock and blood.
    Theme {
        name: "INFERNO",
        floor: FLAT_HELLROCK,
        floor_patches: &[(FLAT_HELLROCK_BLOOD, 48), (FLAT_DIRT_BONES, 24)],
        ceil: FLAT_VAULT,
        ceil_patches: &[],
        sky: Some(SKY_HELL),
        fog: [84, 18, 12],
        fog_density: 0.07,
        ambient: [0.66, 0.44, 0.40],
        light_chance: 26,
        detail_chance: 40,
        worn_chance: 60,
        decor: &[(DEC_BRAZIER, 3), (DEC_SKULLS, 3), (DEC_PILLAR, 1)],
        decor_count: 9,
    },
];

/// The theme for level index `level` (clamped, so a test's out-of-range level
/// still gets one).
pub fn theme_for(level: i32) -> &'static Theme {
    &THEMES[(level.max(0) as usize).min(LEVEL_COUNT - 1)]
}
