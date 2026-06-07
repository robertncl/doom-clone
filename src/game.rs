//! The central `Game` struct — all the state that lived in C file-scope globals
//! now lives here — plus map access, movement, RNG, and high-score handling.

use crate::audio::Audio;
use crate::constants::*;
use crate::textures::Textures;
use crate::types::*;
use std::io::{BufRead, Write};

pub struct Game {
    pub keys: [bool; K_COUNT],
    pub key_edge: [bool; K_COUNT],
    pub running: bool,
    pub muzzle_flash: i32,
    pub level: i32,
    pub level_enemy_count: i32,
    pub show_intro: bool,
    pub score: i32,
    pub score_saved: bool,
    pub final_rank: i32,
    pub level_bonus_given: bool,
    pub high_scores: [i32; MAX_HIGHSCORES],
    pub level_clear_timer: f64,
    pub pain_flash: f64,
    pub global_time: f64,

    pub player: Player,
    pub enemies: [Enemy; MAX_ENEMIES],
    pub fireballs: [Fireball; MAX_FIREBALLS],
    pub parts: [Particle; MAX_PARTICLES],
    pub pickups: [Pickup; MAX_PICKUPS],

    pub pixels: Vec<u32>,
    pub depth: Vec<f64>,
    /// Current level's working map (walls + cleared floor cells), as ASCII bytes.
    pub cur_map: [[u8; MAP_W]; MAP_H],

    pub tex: Textures,
    pub audio: Audio,
    pub bot: Bot,
    rng: u32,
}

impl Game {
    pub fn new() -> Game {
        Game {
            keys: [false; K_COUNT],
            key_edge: [false; K_COUNT],
            running: true,
            muzzle_flash: 0,
            level: 0,
            level_enemy_count: 0,
            show_intro: true,
            score: 0,
            score_saved: false,
            final_rank: 0,
            level_bonus_given: false,
            high_scores: [0; MAX_HIGHSCORES],
            level_clear_timer: 0.0,
            pain_flash: 0.0,
            global_time: 0.0,
            player: Player::default(),
            enemies: [Enemy::default(); MAX_ENEMIES],
            fireballs: [Fireball::default(); MAX_FIREBALLS],
            parts: [Particle::default(); MAX_PARTICLES],
            pickups: [Pickup::default(); MAX_PICKUPS],
            pixels: vec![0; SCREEN_W * SCREEN_H],
            depth: vec![0.0; SCREEN_W],
            cur_map: [[b'.'; MAP_W]; MAP_H],
            tex: Textures::build(),
            audio: Audio::new(),
            bot: Bot::default(),
            rng: std::env::var("DOOM_SEED")
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0x1234_5678),
        }
    }

    // ---- RNG (replaces libc rand(); the exact sequence is unimportant) ----

    pub fn rand_u32(&mut self) -> u32 {
        self.rng = self.rng.wrapping_mul(1103515245).wrapping_add(12345);
        self.rng
    }

    /// Uniform f64 in [0, 1).
    pub fn rand_f64(&mut self) -> f64 {
        (self.rand_u32() >> 8) as f64 / (1u32 << 24) as f64
    }

    // ---- Map access ----

    pub fn map_wall_type(&self, mx: i32, my: i32) -> usize {
        if mx < 0 || mx >= MAP_W as i32 || my < 0 || my >= MAP_H as i32 {
            return WALL_STONE;
        }
        match self.cur_map[my as usize][mx as usize] {
            b'#' => WALL_STONE,
            b'=' => WALL_BRICK,
            b'B' => WALL_METAL,
            b'D' => WALL_WOOD,
            b'H' => WALL_HELL,
            _ => WALL_NONE,
        }
    }

    pub fn map_blocked(&self, mx: i32, my: i32) -> bool {
        self.map_wall_type(mx, my) != WALL_NONE
    }

    pub fn reset_transients(&mut self) {
        self.enemies = [Enemy::default(); MAX_ENEMIES];
        self.fireballs = [Fireball::default(); MAX_FIREBALLS];
        self.parts = [Particle::default(); MAX_PARTICLES];
        self.pickups = [Pickup::default(); MAX_PICKUPS];
    }

    pub fn reset_game(&mut self) {
        self.player.health = 100;
        self.player.armor = 0;
        self.player.ammo = 50;
        self.score = 0;
        self.score_saved = false;
        self.final_rank = 0;
        self.load_level(0);
    }

    // ---- Movement ----

    /// Attempts to move to (nx, ny), sliding along walls one axis at a time.
    /// Returns a bitmask: bit 0 set if the X move succeeded, bit 1 if Y did.
    pub fn try_move(&mut self, nx: f64, ny: f64) -> i32 {
        let pad = 0.18;
        let mut moved = 0;
        if !self.map_blocked((nx + pad) as i32, self.player.y as i32)
            && !self.map_blocked((nx - pad) as i32, self.player.y as i32)
        {
            self.player.x = nx;
            moved |= 1;
        }
        if !self.map_blocked(self.player.x as i32, (ny + pad) as i32)
            && !self.map_blocked(self.player.x as i32, (ny - pad) as i32)
        {
            self.player.y = ny;
            moved |= 2;
        }
        moved
    }

    pub fn all_enemies_dead(&self) -> bool {
        !self.enemies.iter().any(|e| e.alive)
    }

    // ---- High scores ----

    pub fn load_high_scores(&mut self) {
        self.high_scores = [0; MAX_HIGHSCORES];
        if let Ok(f) = std::fs::File::open(HIGHSCORE_FILE) {
            let reader = std::io::BufReader::new(f);
            for (i, line) in reader.lines().enumerate() {
                if i >= MAX_HIGHSCORES {
                    break;
                }
                match line {
                    Ok(s) => match s.trim().parse::<i32>() {
                        Ok(v) => self.high_scores[i] = v,
                        Err(_) => break,
                    },
                    Err(_) => break,
                }
            }
        }
    }

    pub fn save_high_scores(&self) {
        if let Ok(mut f) = std::fs::File::create(HIGHSCORE_FILE) {
            for s in &self.high_scores {
                let _ = writeln!(f, "{}", s);
            }
        }
    }

    /// Inserts score; returns 1-based rank if it made the list, else 0.
    pub fn submit_score(&mut self, s: i32) -> i32 {
        for i in 0..MAX_HIGHSCORES {
            if s > self.high_scores[i] {
                for j in (i + 1..MAX_HIGHSCORES).rev() {
                    self.high_scores[j] = self.high_scores[j - 1];
                }
                self.high_scores[i] = s;
                self.save_high_scores();
                return (i + 1) as i32;
            }
        }
        0
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}
