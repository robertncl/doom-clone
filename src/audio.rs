//! Software sound synthesis plus output by piping raw PCM to an external player
//! (`paplay`/`aplay`/`play`/`sox`). If none is found, audio is a silent no-op —
//! the game runs fine without it. (There's no native Windows audio backend, and
//! none of those players exist on Windows, so audio stays silent there.)

use crate::constants::*;
use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};

const AUDIO_RATE: f64 = 22050.0;
const MAX_SOUNDS: usize = 16;
const AUDIO_BUF_MAX: usize = 4096;

const SOUND_DUR: [f64; SND_KIND_MAX] =
    [0.20, 0.15, 0.50, 0.30, 0.25, 0.35, 0.35, 0.90, 1.60, 0.40];

#[derive(Clone, Copy, Default)]
struct ActiveSound {
    kind: usize,
    t: f64,
    active: bool,
    seed: u32,
}

pub struct Audio {
    sounds: [ActiveSound; MAX_SOUNDS],
    ok: bool,
    rng: u32,
    _child: Option<Child>, // kept alive so its stdin pipe stays open
    stdin: Option<ChildStdin>,
}

fn audio_noise(s: &mut u32) -> u32 {
    *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
    *s
}

fn sound_sample(kind: usize, t: f64, seed: &mut u32) -> f64 {
    let tau = 2.0 * std::f64::consts::PI;
    match kind {
        SND_SHOOT => {
            let env = (-t * 18.0).exp();
            let n = ((audio_noise(seed) >> 16) as i32 - 32768) as f64 / 32768.0;
            let boom = (t * 100.0 * tau).sin() * env * 0.7;
            n * env * 0.9 + boom
        }
        SND_HIT => {
            let env = (-t * 14.0).exp();
            let mut freq = 240.0 - t * 380.0;
            if freq < 40.0 {
                freq = 40.0;
            }
            (t * freq * tau).sin() * env
        }
        SND_DEATH => {
            let env = (-t * 4.0).exp();
            let n = ((audio_noise(seed) >> 16) as i32 - 32768) as f64 / 32768.0;
            let mut freq = 360.0 - t * 500.0;
            if freq < 50.0 {
                freq = 50.0;
            }
            (t * freq * tau).sin() * env * 0.5 + n * env * 0.35
        }
        SND_PICKUP_HEALTH => {
            let env = (-t * 6.0).exp();
            let freq = 700.0 + t * 1600.0;
            (t * freq * tau).sin() * env * 0.8
        }
        SND_PICKUP_AMMO => {
            let env = (-t * 11.0).exp();
            let f1 = (t * 1500.0 * tau).sin();
            let f2 = (t * 2300.0 * tau).sin();
            (f1 + f2 * 0.5) * env * 0.6
        }
        SND_FIREBALL => {
            let env = (-t * 5.0).exp() * (1.0 - (-t * 28.0).exp());
            let n = ((audio_noise(seed) >> 16) as i32 - 32768) as f64 / 32768.0;
            let lf = (t * 180.0 * tau).sin() * 0.4;
            n * env * 0.9 + lf * env
        }
        SND_PLAYER_HURT => {
            let env = (-t * 7.0).exp();
            let n = ((audio_noise(seed) >> 16) as i32 - 32768) as f64 / 32768.0;
            let freq = 380.0 + (t * 30.0).sin() * 80.0;
            (t * freq * tau).sin() * env * 0.5 + n * env * 0.35
        }
        SND_LEVEL_CLEAR => {
            let env = (-t * 1.4).exp();
            let freqs = [523.25, 659.25, 783.99];
            let mut idx = (t * 6.0) as usize;
            if idx > 2 {
                idx = 2;
            }
            (t * freqs[idx] * tau).sin() * env * 0.6
        }
        SND_GAME_OVER => {
            let env = (-t * 0.8).exp();
            let freq = 220.0 * 0.5f64.powf(t / 0.7);
            (t * freq * tau).sin() * env * 0.8
        }
        SND_PICKUP_WEAPON => {
            // A confident rising two-note chime — heavier than the ammo blip.
            let env = (-t * 5.0).exp();
            let freq = if t < 0.12 { 500.0 } else { 750.0 };
            let click = ((audio_noise(seed) >> 16) as i32 - 32768) as f64 / 32768.0;
            (t * freq * tau).sin() * env * 0.7 + click * (-t * 40.0).exp() * 0.3
        }
        _ => 0.0,
    }
}

impl Audio {
    pub fn new() -> Audio {
        Audio {
            sounds: [ActiveSound::default(); MAX_SOUNDS],
            ok: false,
            rng: 0x2545_F491,
            _child: None,
            stdin: None,
        }
    }

    pub fn play(&mut self, kind: usize) {
        if !self.ok || kind >= SND_KIND_MAX {
            return;
        }
        for s in self.sounds.iter_mut() {
            if !s.active {
                self.rng = self.rng.wrapping_mul(1103515245).wrapping_add(12345);
                s.kind = kind;
                s.t = 0.0;
                s.active = true;
                s.seed = ((self.rng >> 8) & 0xFFFF) | 1;
                return;
            }
        }
    }

    pub fn init(&mut self) {
        // First external player that launches wins; each reads raw s16le mono.
        let candidates: &[(&str, &[&str])] = &[
            (
                "paplay",
                &["--raw", "--format=s16le", "--rate=22050", "--channels=1", "--latency-msec=80"],
            ),
            (
                "aplay",
                &["-q", "-t", "raw", "-f", "S16_LE", "-r", "22050", "-c", "1", "--buffer-time=80000"],
            ),
            ("play", &["-q", "-t", "raw", "-r", "22050", "-c", "1", "-e", "signed", "-b", "16", "-"]),
            (
                "sox",
                &["-q", "-t", "raw", "-r", "22050", "-c", "1", "-e", "signed", "-b", "16", "-", "-d"],
            ),
        ];
        for (bin, args) in candidates {
            if let Ok(mut child) = Command::new(bin)
                .args(*args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                self.stdin = child.stdin.take();
                self._child = Some(child);
                self.ok = true;
                return;
            }
        }
    }

    pub fn shutdown(&mut self) {
        self.stdin = None;
        self._child = None;
        self.ok = false;
    }

    fn mix_samples(&mut self, buf: &mut [i16]) {
        let count = buf.len();
        for (i, slot) in buf.iter_mut().enumerate() {
            let sample_time = i as f64 / AUDIO_RATE;
            let mut mix = 0.0;
            for s in self.sounds.iter_mut() {
                if !s.active {
                    continue;
                }
                let st = s.t + sample_time;
                if st >= SOUND_DUR[s.kind] {
                    continue;
                }
                mix += sound_sample(s.kind, st, &mut s.seed);
            }
            if mix > 1.0 {
                mix = 1.0;
            }
            if mix < -1.0 {
                mix = -1.0;
            }
            *slot = (mix * 28000.0) as i16;
        }
        let advance = count as f64 / AUDIO_RATE;
        for s in self.sounds.iter_mut() {
            if !s.active {
                continue;
            }
            s.t += advance;
            if s.t >= SOUND_DUR[s.kind] {
                s.active = false;
            }
        }
    }

    pub fn tick(&mut self, dt: f64) {
        if !self.ok || dt <= 0.0 {
            return;
        }
        let mut samples = (dt * AUDIO_RATE + 0.5) as usize;
        if samples == 0 {
            return;
        }
        if samples > AUDIO_BUF_MAX {
            samples = AUDIO_BUF_MAX;
        }
        let mut buf = [0i16; AUDIO_BUF_MAX];
        self.mix_samples(&mut buf[..samples]);

        let mut bytes = Vec::with_capacity(samples * 2);
        for &s in &buf[..samples] {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        let mut broken = false;
        if let Some(stdin) = &mut self.stdin {
            if stdin.write_all(&bytes).is_err() {
                broken = true; // pipe broken — quietly stop
            }
        }
        if broken {
            self.ok = false;
        }
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}
