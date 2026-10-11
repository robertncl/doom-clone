//! Difficulty: the skill picked on the intro screen's slider, and the gradient
//! of enemy/supply tuning it produces across the nine levels.
//!
//! Everything funnels through one number, the *pressure*: 0 is the hand-tuned
//! game (NORMAL plays exactly as it always has), negative is kinder, positive
//! is harsher. Below NORMAL the discount is flat for the whole run; above it
//! the pressure also climbs level by level, so HARD and NIGHTMARE get steeper
//! the deeper you go on top of the escalation already built into the maps.

use crate::constants::LEVEL_COUNT;

pub const SKILL_BABY: usize = 0;
pub const SKILL_EASY: usize = 1;
pub const SKILL_NORMAL: usize = 2;
pub const SKILL_HARD: usize = 3;
pub const SKILL_NIGHTMARE: usize = 4;
pub const SKILL_COUNT: usize = 5;

pub const SKILL_NAMES: [&str; SKILL_COUNT] = ["BABY", "EASY", "NORMAL", "HARD", "NIGHTMARE"];

/// Slider colour per skill — a green-to-blood-red ramp, so the bar itself
/// reads as the gradient.
pub const SKILL_COLORS: [u32; SKILL_COUNT] = [0x40E0A0, 0x80E040, 0xE0E040, 0xE08030, 0xE02020];

/// Pressure on level 1, per skill.
const SKILL_BASE: [f64; SKILL_COUNT] = [-0.6, -0.3, 0.0, 0.25, 0.5];
/// Extra pressure reached by the last level (added linearly per level).
const SKILL_RAMP: [f64; SKILL_COUNT] = [0.0, 0.0, 0.0, 0.25, 0.5];

/// Pressure for `skill` on level index `level` (0-based).
pub fn pressure(skill: usize, level: usize) -> f64 {
    let s = skill.min(SKILL_COUNT - 1);
    let depth = level.min(LEVEL_COUNT - 1) as f64 / (LEVEL_COUNT - 1) as f64;
    SKILL_BASE[s] + SKILL_RAMP[s] * depth
}

/// Multipliers applied to the base tuning. All are exactly 1.0 at pressure 0,
/// so NORMAL is bit-for-bit the original game (and the bot fingerprint holds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// Damage enemies deal to the player (melee and fireballs).
    pub enemy_dmg: f64,
    /// Enemy spawn hit points.
    pub enemy_hp: f64,
    /// Enemy walk speed.
    pub enemy_speed: f64,
    /// Attack rate: cooldowns are divided by this.
    pub enemy_rate: f64,
    /// Fireball flight speed.
    pub fireball_speed: f64,
    /// Health/ammo restored by a pickup.
    pub supply: f64,
}

impl Tuning {
    pub fn from_pressure(p: f64) -> Tuning {
        Tuning {
            enemy_dmg: 1.0 + p,
            enemy_hp: 1.0 + 0.5 * p,
            enemy_speed: 1.0 + 0.3 * p,
            enemy_rate: 1.0 + 0.5 * p,
            fireball_speed: 1.0 + 0.25 * p,
            supply: 1.0 - 0.4 * p,
        }
    }

    pub fn at(skill: usize, level: usize) -> Tuning {
        Tuning::from_pressure(pressure(skill, level))
    }
}

/// Scale an integer stat, rounding, but never below 1 — a hit always hurts and
/// an enemy always needs shooting.
pub fn scale(v: i32, m: f64) -> i32 {
    ((v as f64 * m).round() as i32).max(1)
}

/// Parse a `--skill` value: 1-based number or a name, case-insensitive.
pub fn parse_skill(s: &str) -> Option<usize> {
    let s = s.trim();
    if let Ok(n) = s.parse::<usize>() {
        return (1..=SKILL_COUNT).contains(&n).then(|| n - 1);
    }
    SKILL_NAMES.iter().position(|name| name.eq_ignore_ascii_case(s))
}

impl crate::game::Game {
    /// Tuning for the current skill and level.
    pub fn tuning(&self) -> Tuning {
        Tuning::at(self.skill, self.level.max(0) as usize)
    }

    /// Change skill. Enemies already spawned (the intro sits over a loaded
    /// level 1) are re-rolled to the new skill's full HP, so the choice applies
    /// from the first room rather than from level 2.
    pub fn set_skill(&mut self, skill: usize) {
        self.skill = skill.min(SKILL_COUNT - 1);
        let t = self.tuning();
        for e in self.enemies.iter_mut().filter(|e| e.alive) {
            e.hp = scale(crate::constants::EN_HP[e.kind as usize], t.enemy_hp);
        }
    }
}
