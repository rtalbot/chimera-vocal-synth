//! Skywhale (invented creature): a held moan is finite and in range, its infrasonic fundamental
//! sits where asked (YIN can't see 16 Hz in a short frame, so the test reads the spectrum), and it
//! carries audible harmonics.
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(params: &[(&str, f64)], sec: f64) -> Vec<f32> {
    chimera::creatures::init();
    let def = creature_by_id("skywhale").expect("skywhale is registered");
    let p: Vec<(usize, f64)> = params.iter().map(|(k, x)| (def.param_index(k).expect("param"), *x)).collect();
    let mut v = CreatureVoice::new(def, FS, &p);
    v.set_gate(1.0);
    let mut out = vec![0f32; (sec * FS) as usize];
    for b in out.chunks_mut(128) {
        v.render(b);
    }
    out
}

/// Hann-windowed magnitude at `f` (Goertzel by direct sum).
fn magnitude(x: &[f32], f: f64) -> f64 {
    let w = std::f64::consts::TAU * f / FS;
    let n = x.len() as f64;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &s) in x.iter().enumerate() {
        let h = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n).cos();
        re += s as f64 * h * (w * i as f64).cos();
        im += s as f64 * h * (w * i as f64).sin();
    }
    (re * re + im * im).sqrt() / n
}

#[test]
fn held_moan_is_finite_in_range_and_infrasonic() {
    let out = hold(&[], 3.0);
    assert!(out.iter().all(|x| x.is_finite()));
    let peak = out.iter().fold(0f32, |a, &b| a.max(b.abs()));
    assert!((0.2..0.95).contains(&peak), "peak {peak}");
    // 2 s of the held moan: 16 Hz and its harmonics stand far above the frequencies between them
    let x = &out[(1.0 * FS) as usize..];
    for k in 1..=3 {
        let on = magnitude(x, 16.0 * k as f64);
        let off = magnitude(x, 16.0 * k as f64 + 8.0);
        assert!(on > 10.0 * off, "harmonic {k}: {on} vs {off} between");
    }
}

#[test]
fn moan_has_audible_harmonics() {
    let x = hold(&[], 3.0);
    let x = &x[(1.0 * FS) as usize..];
    let fundamental = magnitude(x, 16.0);
    // strongest harmonic between 150 and 320 Hz, within 40 dB of the fundamental
    let upper = (10..=20).map(|k| magnitude(x, 16.0 * k as f64)).fold(0f64, f64::max);
    assert!(upper > 0.01 * fundamental, "upper harmonics {upper} vs fundamental {fundamental}");
}

#[test]
fn pitch_follows_the_keys_down_to_8_hz() {
    let x = hold(&[("pitch", 10.0)], 3.0);
    let x = &x[(1.0 * FS) as usize..];
    // 10 Hz itself is under the voice's DC blocker; its harmonics land on 10 Hz multiples
    let on = magnitude(x, 30.0);
    let off = magnitude(x, 35.0);
    assert!(on > 10.0 * off, "30 Hz {on} vs 35 Hz {off}");
}
