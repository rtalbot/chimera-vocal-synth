//! The Resonant (an invented creature, so no TypeScript golden): one breath sounds a chord.

use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

/// One second of the held voice with these parameters; returns the last half second.
fn hold(vals: &[(&str, f64)]) -> Vec<f64> {
    let def = creature_by_id("resonant").unwrap();
    let vals: Vec<(usize, f64)> = vals.iter().map(|(k, x)| (def.param_index(k).unwrap(), *x)).collect();
    let mut v = CreatureVoice::new(def, FS, &vals);
    v.set_gate(1.0);
    let mut out = vec![0f32; FS as usize];
    for b in out.chunks_mut(128) {
        v.render(b);
    }
    out[out.len() / 2..].iter().map(|&x| x as f64).collect()
}

/// Energy at one frequency (Goertzel), relative to the signal's total energy.
fn share_at(x: &[f64], f: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * f / FS;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &s) in x.iter().enumerate() {
        re += s * (w * i as f64).cos();
        im -= s * (w * i as f64).sin();
    }
    let total: f64 = x.iter().map(|s| s * s).sum();
    (re * re + im * im) / (total * x.len() as f64)
}

#[test]
fn one_voice_sings_the_root_in_tune() {
    let x = hold(&[("pitch", 220.0), ("voices", 0.0)]);
    let p = yin_pitch(&x[..4096], FS, YinOpts { min_f0: Some(60.0), max_f0: Some(1200.0), ..Default::default() });
    assert!((p.f0 - 220.0).abs() < 2.0, "root at {} Hz", p.f0);
    assert!(x.iter().all(|s| s.is_finite()));
    let peak = x.iter().fold(0.0f64, |m, s| m.max(s.abs()));
    assert!(peak > 0.05 && peak < 1.0, "peak {peak}");
}

#[test]
fn more_voices_add_the_chord_tones() {
    // A major: the third (C#, 277 Hz) and fifth (E, 330 Hz) appear only when their voices sound
    let tones = [("third", 220.0 * 2f64.powf(4.0 / 12.0)), ("fifth", 220.0 * 2f64.powf(7.0 / 12.0))];
    let solo = hold(&[("pitch", 220.0), ("voices", 0.0)]);
    let chord = hold(&[("pitch", 220.0), ("voices", 1.0)]);
    for (name, f) in tones {
        let (a, b) = (share_at(&solo, f), share_at(&chord, f));
        assert!(b > 20.0 * a, "{name} at {f:.0} Hz: solo {a:.2e}, chord {b:.2e}");
    }
}
