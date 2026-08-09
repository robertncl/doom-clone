//! Level layouts and `load_level`.
//!
//! Map encoding — walls: `#` stone, `=` brick, `B` metal, `D` wood,
//! `H` hell-rock, `T` tech panel. Floor: `.` plain, `~` lava (walkable, burns).
//! Contents: `p` player spawn, `g` grunt, `i` imp, `w` wraith, `k` baron,
//! `o` fuel barrel, `h` health, `a` ammo, `s` shotgun pickup, `r` rifle pickup.
//!
//! Levels 1-5 are the original approachable set. From level 6 on the maps get
//! denser and start layering the extra mechanics: barrels to shoot (6), lava
//! and the first baron (7), a lava-channelled tech foundry (8), and a hell
//! core built around a lava lake with two barons (9).

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
    // Level 6 — metal bastion: winding plated corridors, ambushes at the turns.
    // Wraiths start showing up here, and fuel barrels are parked next to the
    // tighter enemy pockets.
    [
        "BBBBBBBBBBBBBBBB",
        "Bp.....B..a...gB",
        "B.BBB..B..BBBB.B",
        "B.B.s..D..i.o..B",
        "B.B.BBBBBBB.BB.B",
        "B....g.o...B.a.B",
        "BBBB.BBBB..B...B",
        "B..h....B..BBB.B",
        "B.BBBBB.B..w...B",
        "B.B.a.i.B.BBBB.B",
        "B.D.BBBBB.g..B.B",
        "B.B..w...BB..D.B",
        "B.BBBBBB..B..B.B",
        "B..h...BoiB..B.B",
        "B.h.i..B..B.a..B",
        "BBBBBBBBBBBBBBBB",
    ],
    // Level 7 — hell gauntlet: nested rings around a rifle vault. The vault is
    // now moated with lava (the rifle costs a burn), a baron holds the east
    // corridor, and a lava shortcut cuts through the south wall.
    [
        "HHHHHHHHHHHHHHHH",
        "Hp.h...........H",
        "H.HHHHH.HHHHH..H",
        "H.H.a.i....gH..H",
        "H.H.HHHHHH..H..H",
        "H.H.H..r.H..H.iH",
        "H.H.H~HH~H..H..H",
        "H.H.H....H.kH..H",
        "H...HHH.HH.aH..H",
        "H.g..a.og...H..H",
        "HH.HHHHHHHHHH.aH",
        "H..i...h..g....H",
        "H.HHH.HHHH~HHH.H",
        "H.h.H.i..H..a..H",
        "H...H..a.H..i.gH",
        "HHHHHHHHHHHHHHHH",
    ],
    // Level 8 — the foundry: tech panelling around a sealed furnace (cross the
    // lava to loot it), a baron penned behind the metal bulkhead, and barrels
    // seeded along the service corridors.
    [
        "TTTTTTTTTTTTTTTT",
        "Tp...T.g..a..w.T",
        "T.TT.T.TT.TTTT.T",
        "T.T..T.T~~~T..iT",
        "T.T.oT.TrohT.TTT",
        "T.T..B.T~~~Tag.T",
        "T.TTTB.TTTTTTT.T",
        "T..w.B..g.h.i..T",
        "TTT.TBBBBBTTTT.T",
        "T.a.T.i..k.h.T.T",
        "T.T.TTTTT.T.aT.T",
        "T.T..g..o.T..T.T",
        "T.TTTTTT..TTTa.T",
        "T.h....~~..w.o.T",
        "T....i.~~..a..sT",
        "TTTTTTTTTTTTTTTT",
    ],
    // Level 9 — inferno core: a hell keep built around a lava cell. One baron
    // sits in the fire at the centre (you fight it standing in the burn or
    // snipe from the doorway), a second holds the southern ring.
    [
        "HHHHHHHHHHHHHHHH",
        "Hp...a..g...i..H",
        "H.HHHHHHHHHH.H.H",
        "H.H..w.i.aoH.H.H",
        "H.H.HHHHH.HH.H.H",
        "H.H.H~~~H.ra.H.H",
        "H.H.H~k~H.HHH.aH",
        "H.H.H~~~H.H.wahH",
        "H.H.HH.HH.H.HHHH",
        "H.H.ai...~..hg.H",
        "H.HHHHHHHH.HHH.H",
        "H.ho.w...H.k.a.H",
        "HHHHH.HHHHHH.HHH",
        "Ha..i.a..~.g.ohH",
        "H.g.h..w....i..H",
        "HHHHHHHHHHHHHHHH",
    ],
];

impl Game {
    pub fn load_level(&mut self, n: usize) {
        self.reset_transients();
        let mut e_idx = 0usize;
        let mut p_idx = 0usize;
        let mut b_idx = 0usize;
        self.has_hazard = false;
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
                    b'g' | b'i' | b'w' | b'k' => {
                        if e_idx < MAX_ENEMIES {
                            let kind = match c {
                                b'i' => EN_IMP,
                                b'w' => EN_WRAITH,
                                b'k' => EN_BARON,
                                _ => EN_GRUNT,
                            };
                            self.enemies[e_idx].x = x as f64 + 0.5;
                            self.enemies[e_idx].y = y as f64 + 0.5;
                            self.enemies[e_idx].kind = kind;
                            self.enemies[e_idx].alive = true;
                            self.enemies[e_idx].hp = EN_HP[kind as usize];
                            // Stagger the animation phase by tile so a pack of
                            // the same kind doesn't move in lockstep.
                            self.enemies[e_idx].anim = (x + y) as f64 * 0.7 + kind as f64 * 0.9;
                            e_idx += 1;
                        }
                        dest = b'.';
                    }
                    b'o' => {
                        if b_idx < MAX_BARRELS {
                            self.barrels[b_idx].x = x as f64 + 0.5;
                            self.barrels[b_idx].y = y as f64 + 0.5;
                            self.barrels[b_idx].alive = true;
                            b_idx += 1;
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
                    b'~' => self.has_hazard = true,
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
