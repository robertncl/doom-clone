//! Test suite.
//!
//! All tests live under this module rather than in `#[cfg(test)] mod tests`
//! blocks next to the code, for one reason: coverage. Inline test modules are
//! instrumented along with everything else, and since test bodies run by
//! definition they score ~100% and quietly inflate the headline number. Keeping
//! them in one subtree lets the coverage run exclude them
//! (`--ignore-filename-regex 'src/tests/'`, wrapped up in `./coverage.sh`), so
//! the figure reported is coverage of the *game*.
//!
//! The cost is that tests can't reach private items, so module-internal helpers
//! the tests drive directly are `pub(crate)`. In a binary crate that's a
//! formality — nothing here is public API.

mod audio_tests;
mod bot_tests;
mod color_tests;
mod entity_tests;
mod game_tests;
mod hud_tests;
mod level_tests;
mod main_tests;
mod playthrough_tests;
mod render_tests;
mod selftest_tests;
mod sprites_tests;

use crate::constants::*;
use crate::game::Game;
use crate::types::Barrel;

/// A fresh game with its high-score table redirected to a scratch file. Tests
/// end runs all the time, and a finished run writes the table out — with the
/// default path that would rewrite the file in the working directory.
pub fn new_game() -> Game {
    let mut g = Game::new();
    g.score_file = scratch_path("scores.dat").to_string_lossy().into_owned();
    g
}

/// A game sitting in an empty walled room, player at (2.5, 2.5) facing +X with
/// a full clip. The map is overwritten wholesale so a test's geometry is
/// exactly what it asked for, with no level data bleeding in.
pub fn open_room() -> Game {
    let mut g = new_game();
    g.reset_game();
    g.reset_transients();
    g.show_intro = false;
    for y in 0..MAP_H {
        for x in 0..MAP_W {
            let edge = x == 0 || y == 0 || x == MAP_W - 1 || y == MAP_H - 1;
            g.cur_map[y][x] = if edge { b'#' } else { b'.' };
        }
    }
    g.has_hazard = false;
    g.player.x = 2.5;
    g.player.y = 2.5;
    g.player.angle = 0.0;
    g.player.health = 100;
    g.player.ammo = 50;
    g.player.weapon = WP_PISTOL;
    g
}

/// Place a live enemy of `kind` at (x, y) in slot `i`, with that kind's HP.
pub fn put_enemy(g: &mut Game, i: usize, x: f64, y: f64, kind: i32) {
    g.enemies[i].x = x;
    g.enemies[i].y = y;
    g.enemies[i].kind = kind;
    g.enemies[i].hp = EN_HP[kind as usize];
    g.enemies[i].alive = true;
    g.enemies[i].atk_cool = 0.0;
    g.enemies[i].hit_flash = 0.0;
}

pub fn put_barrel(g: &mut Game, i: usize, x: f64, y: f64) {
    g.barrels[i] = Barrel { x, y, alive: true, hit_flash: 0.0 };
}

pub fn put_pickup(g: &mut Game, i: usize, x: f64, y: f64, kind: i32) {
    g.pickups[i].x = x;
    g.pickups[i].y = y;
    g.pickups[i].kind = kind;
    g.pickups[i].alive = true;
}

pub fn live_enemies(g: &Game) -> usize {
    g.enemies.iter().filter(|e| e.alive).count()
}

pub fn live_fireballs(g: &Game) -> usize {
    g.fireballs.iter().filter(|f| f.alive).count()
}

pub fn live_particles(g: &Game) -> usize {
    g.parts.iter().filter(|p| p.life > 0.0).count()
}

/// True if any pixel in the framebuffer is non-black — the cheapest useful
/// assertion that a draw call actually put something on screen.
pub fn any_pixel_drawn(g: &Game) -> bool {
    g.pixels.iter().any(|&p| p != 0)
}

/// Count of pixels matching `c`, for asserting a specific element was drawn.
pub fn count_color(g: &Game, c: u32) -> usize {
    g.pixels.iter().filter(|&&p| p == c).count()
}

/// A scratch path unique to `name`, for tests that touch the filesystem.
pub fn scratch_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("doom-clone-test-{}-{}", std::process::id(), name));
    p
}
