//! Software sound synthesis and the mixer.
//!
//! Nothing here needs a working audio device: the mixer writes into a caller
//! supplied buffer, and `tick` only touches the pipe if one was opened. Tests
//! set `ok` directly to exercise the paths a real player would unlock.

use crate::audio::{sound_sample, Audio, AUDIO_BUF_MAX, AUDIO_RATE, MAX_SOUNDS, SOUND_DUR};
use crate::constants::*;

#[test]
fn every_sound_kind_synthesizes_a_bounded_waveform() {
    for kind in 0..SND_KIND_MAX {
        let mut seed = 12345u32;
        let mut nonzero = false;
        let dur = SOUND_DUR[kind];
        assert!(dur > 0.0, "sound {kind} has no duration");
        // Walk the whole envelope.
        for i in 0..200 {
            let t = dur * i as f64 / 200.0;
            let s = sound_sample(kind, t, &mut seed);
            assert!(s.is_finite(), "sound {kind} produced a non-finite sample at t={t}");
            assert!(s.abs() <= 4.0, "sound {kind} sample {s} is wildly out of range");
            if s.abs() > 1e-6 {
                nonzero = true;
            }
        }
        assert!(nonzero, "sound {kind} is silent");
    }
}

#[test]
fn an_unknown_sound_kind_is_silent() {
    let mut seed = 1;
    assert_eq!(sound_sample(SND_KIND_MAX + 5, 0.1, &mut seed), 0.0);
}

#[test]
fn sound_envelopes_decay_over_their_lifetime() {
    // Sampled early vs late, the tail should be quieter for the percussive
    // sounds (averaged, since several are noise-based).
    for kind in [SND_SHOOT, SND_HIT, SND_DEATH, SND_EXPLOSION] {
        let dur = SOUND_DUR[kind];
        let mut seed = 999u32;
        let early: f64 =
            (0..40).map(|i| sound_sample(kind, dur * 0.02 * i as f64 / 40.0, &mut seed).abs()).sum();
        let late: f64 = (0..40)
            .map(|i| sound_sample(kind, dur * (0.8 + 0.2 * i as f64 / 40.0), &mut seed).abs())
            .sum();
        assert!(early > late, "sound {kind} should decay ({early} -> {late})");
    }
}

#[test]
fn a_silent_audio_device_ignores_everything() {
    let mut a = Audio::new();
    assert!(!a.ok, "a fresh Audio has no output until init finds a player");
    a.play(SND_SHOOT); // no-op
    a.tick(1.0 / 60.0); // no-op
    let mut buf = [0i16; 64];
    a.mix_samples(&mut buf);
    assert!(buf.iter().all(|&s| s == 0), "no active sounds means silence");
}

#[test]
fn playing_a_sound_fills_a_voice_and_mixes_audibly() {
    let mut a = Audio::new();
    a.ok = true; // pretend a player is attached; tick writes nowhere
    a.play(SND_SHOOT);

    let mut buf = [0i16; 512];
    a.mix_samples(&mut buf);
    assert!(buf.iter().any(|&s| s != 0), "an active sound should produce samples");
    assert!(buf.iter().all(|&s| s.abs() <= 28000), "output must stay inside the headroom");
}

#[test]
fn an_out_of_range_sound_kind_is_never_queued() {
    let mut a = Audio::new();
    a.ok = true;
    a.play(SND_KIND_MAX);
    let mut buf = [0i16; 128];
    a.mix_samples(&mut buf);
    assert!(buf.iter().all(|&s| s == 0));
}

#[test]
fn the_voice_pool_is_bounded() {
    let mut a = Audio::new();
    a.ok = true;
    // More plays than voices: the extras are dropped, not queued.
    for _ in 0..MAX_SOUNDS + 8 {
        a.play(SND_HIT);
    }
    let mut buf = [0i16; 256];
    a.mix_samples(&mut buf); // must not panic or overflow
    assert!(buf.iter().any(|&s| s != 0));
}

#[test]
fn a_sound_retires_once_its_duration_has_elapsed() {
    let mut a = Audio::new();
    a.ok = true;
    a.play(SND_SHOOT);

    // Mix well past the sound's duration in chunks.
    let chunk = (AUDIO_RATE * 0.05) as usize;
    let mut buf = vec![0i16; chunk];
    let rounds = (SOUND_DUR[SND_SHOOT] / 0.05).ceil() as usize + 2;
    for _ in 0..rounds {
        a.mix_samples(&mut buf);
    }
    buf.iter_mut().for_each(|s| *s = 0);
    a.mix_samples(&mut buf);
    assert!(buf.iter().all(|&s| s == 0), "the voice should have been freed");
}

#[test]
fn simultaneous_sounds_stay_clamped() {
    let mut a = Audio::new();
    a.ok = true;
    for _ in 0..MAX_SOUNDS {
        a.play(SND_EXPLOSION);
    }
    let mut buf = [0i16; 512];
    a.mix_samples(&mut buf);
    assert!(
        buf.iter().all(|&s| s.abs() <= 28000),
        "a full voice pool must not clip past the limiter"
    );
}

#[test]
fn tick_ignores_degenerate_timesteps_and_caps_its_buffer() {
    let mut a = Audio::new();
    a.ok = true;
    a.play(SND_SHOOT);

    a.tick(0.0); // no time passed
    a.tick(-1.0); // time going backwards
    a.tick(1e-9); // rounds to zero samples
    a.tick(10.0); // far more than one buffer's worth: must clamp, not overrun
    assert!(a.ok, "none of that should have torn down the device");
}

#[test]
fn tick_with_no_pipe_leaves_the_device_up() {
    let mut a = Audio::new();
    a.ok = true;
    a.play(SND_HIT);
    for _ in 0..10 {
        a.tick(1.0 / 60.0);
    }
    assert!(a.ok);
}

#[test]
fn a_full_buffer_request_is_serialized_without_overrunning() {
    let mut a = Audio::new();
    a.ok = true;
    a.play(SND_LEVEL_CLEAR);
    // Exactly the buffer cap.
    a.tick(AUDIO_BUF_MAX as f64 / AUDIO_RATE);
    assert!(a.ok);
}

#[test]
fn init_without_a_player_installed_stays_silent_and_shutdown_is_safe() {
    let mut a = Audio::new();
    a.init(); // may or may not find a player on this machine
    a.shutdown();
    assert!(!a.ok, "shutdown always leaves the device closed");
    a.play(SND_SHOOT);
    a.tick(1.0 / 60.0);
}

#[test]
fn a_default_audio_is_a_new_one() {
    let a = Audio::default();
    assert!(!a.ok);
}

// ---- the external player pipe ----
//
// `cat` stands in for a real audio player: it exists everywhere, accepts raw
// bytes on stdin, and lets the write path run for real.

#[test]
fn a_launchable_player_opens_the_device_and_takes_samples() {
    let mut a = Audio::new();
    a.init_with(&[("definitely-not-a-real-binary-xyz", &[]), ("cat", &[])]);
    assert!(a.ok, "it should fall through to the candidate that launches");

    a.play(SND_SHOOT);
    for _ in 0..20 {
        a.tick(1.0 / 60.0);
    }
    assert!(a.ok, "writing to a live pipe should keep the device up");

    a.shutdown();
    assert!(!a.ok);
}

#[test]
fn a_player_that_exits_takes_the_device_down_quietly() {
    let mut a = Audio::new();
    // `true` exits immediately, so the pipe breaks on the first sizeable write.
    a.init_with(&[("true", &[])]);
    if !a.ok {
        return; // no `true` binary on this platform; nothing to assert
    }
    a.play(SND_EXPLOSION);
    for _ in 0..200 {
        a.tick(1.0 / 30.0);
        if !a.ok {
            break;
        }
    }
    assert!(!a.ok, "a broken pipe should quietly disable audio");
    a.tick(1.0 / 60.0); // and stay disabled without erroring
}

#[test]
fn no_launchable_player_leaves_the_device_closed() {
    let mut a = Audio::new();
    a.init_with(&[("definitely-not-a-real-binary-xyz", &[])]);
    assert!(!a.ok);
}
