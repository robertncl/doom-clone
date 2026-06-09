//! Level layouts and `load_level`.
//!
//! Map encoding: `#` stone, `=` brick, `B` metal, `D` wood, `H` hell-rock,
//! `.` floor, `p` player spawn, `g` grunt, `i` imp, `h` health, `a` ammo,
//! `s` shotgun pickup, `r` rifle pickup.

use crate::constants::*;
use crate::game::Game;

pub static LEVELS: [[&str; MAP_H]; LEVEL_COUNT] = [
    [
        "################",
        "#p.............#",
        "#..==s.====r...#",
        "#..=...g...=.a.#",
        "#..=.......=...#",
        "#..====....=...#",
        "#...h..........#",
        "#......=====...#",
        "#......=.g.....#",
        "#......=...==..#",
        "#......=.......#",
        "#......========#",
        "#..i...........#",
        "#..===......h..#",
        "#.........g....#",
        "################",
    ],
    [
        "BBBBBBBBBBBBBBBB",
        "Bp.......B..s..B",
        "B...B....B.g...B",
        "B...D.h..D.....B",
        "B...B....BBBB..B",
        "B...B....i.....B",
        "B...BBBBBB.....B",
        "B.g......B...a.B",
        "B........B..BBBB",
        "BBBB.h...B.....B",
        "B........B.g...B",
        "B...BBBBBB.....B",
        "B...B......i...B",
        "B...B....BBBBBBB",
        "B.a.B......g...B",
        "BBBBBBBBBBBBBBBB",
    ],
    [
        "################",
        "#p.#.....g.....#",
        "#..#.########..#",
        "#....#r.h..=.a.#",
        "####.#.====.=..#",
        "#.a..#.=i=..=..#",
        "#.####.=.=..=..#",
        "#....g.=.=..=..#",
        "#.######.=..=..#",
        "#.h......=..=..#",
        "#.########..=..#",
        "#......i....=..#",
        "#.##########=..#",
        "#......a....=..#",
        "#.============g#",
        "################",
    ],
    [
        "HHHHHHHHHHHHHHHH",
        "Hp..s.g........H",
        "H..============H",
        "H......i......aH",
        "H..=..HHHHHH...H",
        "H..=....h..H.g.H",
        "H..====Hgg.H...H",
        "H......H...H...H",
        "H..a...HHHHH...H",
        "H..============H",
        "H..............H",
        "H...HHHHHHH..i.H",
        "H.....gg..H....H",
        "H.h.H.....H..a.H",
        "H...HHHHHHHHHHHH",
        "HHHHHHHHHHHHHHHH",
    ],
    [
        "HHHHHHHHHHHHHHHH",
        "Hp...g....i....H",
        "H.####.##.####.H",
        "H.#r........#..H",
        "H.#....g....#..H",
        "H.#.........#h.H",
        "H.####.##.####.H",
        "H..............H",
        "H.####.##.####.H",
        "H.#....i....#..H",
        "H.#.........#a.H",
        "H.#....g....#..H",
        "H.####.##.####.H",
        "H..g........i..H",
        "H.....a....h...H",
        "HHHHHHHHHHHHHHHH",
    ],
];

impl Game {
    pub fn load_level(&mut self, n: usize) {
        self.reset_transients();
        let mut e_idx = 0usize;
        let mut p_idx = 0usize;
        for y in 0..MAP_H {
            let row = LEVELS[n][y].as_bytes();
            for x in 0..MAP_W {
                let c = row[x];
                let mut dest = c;
                match c {
                    b'p' => {
                        self.player.x = x as f64 + 0.5;
                        self.player.y = y as f64 + 0.5;
                        self.player.angle = 0.0;
                        self.player.vx = 0.0;
                        self.player.vy = 0.0;
                        self.player.va = 0.0;
                        self.player.bob = 0.0;
                        dest = b'.';
                    }
                    b'g' => {
                        if e_idx < MAX_ENEMIES {
                            self.enemies[e_idx].x = x as f64 + 0.5;
                            self.enemies[e_idx].y = y as f64 + 0.5;
                            self.enemies[e_idx].kind = EN_GRUNT;
                            self.enemies[e_idx].alive = true;
                            self.enemies[e_idx].hp = 2;
                            self.enemies[e_idx].anim = (x + y) as f64 * 0.7;
                            e_idx += 1;
                        }
                        dest = b'.';
                    }
                    b'i' => {
                        if e_idx < MAX_ENEMIES {
                            self.enemies[e_idx].x = x as f64 + 0.5;
                            self.enemies[e_idx].y = y as f64 + 0.5;
                            self.enemies[e_idx].kind = EN_IMP;
                            self.enemies[e_idx].alive = true;
                            self.enemies[e_idx].hp = 3;
                            self.enemies[e_idx].anim = (x + y) as f64 * 0.5;
                            e_idx += 1;
                        }
                        dest = b'.';
                    }
                    b'h' => {
                        if p_idx < MAX_PICKUPS {
                            self.pickups[p_idx].x = x as f64 + 0.5;
                            self.pickups[p_idx].y = y as f64 + 0.5;
                            self.pickups[p_idx].kind = PU_HEALTH;
                            self.pickups[p_idx].alive = true;
                            p_idx += 1;
                        }
                        dest = b'.';
                    }
                    b'a' => {
                        if p_idx < MAX_PICKUPS {
                            self.pickups[p_idx].x = x as f64 + 0.5;
                            self.pickups[p_idx].y = y as f64 + 0.5;
                            self.pickups[p_idx].kind = PU_AMMO;
                            self.pickups[p_idx].alive = true;
                            p_idx += 1;
                        }
                        dest = b'.';
                    }
                    b's' | b'r' => {
                        if p_idx < MAX_PICKUPS {
                            self.pickups[p_idx].x = x as f64 + 0.5;
                            self.pickups[p_idx].y = y as f64 + 0.5;
                            self.pickups[p_idx].kind =
                                if c == b'r' { PU_RIFLE } else { PU_SHOTGUN };
                            self.pickups[p_idx].alive = true;
                            p_idx += 1;
                        }
                        dest = b'.';
                    }
                    _ => {}
                }
                self.cur_map[y][x] = dest;
            }
        }
        self.level = n as i32;
        self.level_enemy_count = e_idx as i32;
        self.level_clear_timer = 0.0;
        self.level_bonus_given = false;
    }
}
