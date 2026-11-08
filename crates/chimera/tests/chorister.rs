//! Chorister (invented creature): a held note is finite, in range and on pitch; its two sets of
//! folds sound two pitches at once through the shared tube, and chaos roughens the duet.
use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(params: &[(&str, f64)], sec: f64) -> Vec<f32> {
    chimera::creatures::init();
    let def = creature_by_id("chorister").expect("chorister is registered");
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
fn held_note_is_finite_in_range_and_on_pitch() {
    let out = hold(&[], 1.0);
    assert!(out.iter().all(|x| x.is_finite()));
    let peak = out.iter().fold(0f32, |a, &b| a.max(b.abs()));
    assert!((0.2..0.95).contains(&peak), "peak {peak}");
    let s = (0.6 * FS) as usize;
    let frame: Vec<f64> = out[s..s + 4096].iter().map(|&x| x as f64).collect();
    let est = yin_pitch(&frame, FS, YinOpts { min_f0: Some(60.0), max_f0: Some(1000.0), ..Default::default() });
    assert!((est.f0 / 196.0 - 1.0).abs() < 0.03, "pitch {}", est.f0);
}

#[test]
fn two_folds_sing_two_pitches_into_one_tube() {
    // a fifth: the upper folds' fundamental (1.5 f) is no harmonic of the lower note
    let clean = [("detune", 0.0), ("chaos", 0.0)];
    let fifth = hold(&[clean[0], clean[1], ("interval", 7.0)], 1.0);
    let unison = hold(&[clean[0], clean[1], ("interval", 0.0)], 1.0);
    let upper = 196.0 * 2f64.powf(7.0 / 12.0);
    let tail = |x: &[f32]| x[(0.4 * FS) as usize..].to_vec();
    let (fifth, unison) = (tail(&fifth), tail(&unison));
    let in_fifth = magnitude(&fifth, upper) / magnitude(&fifth, 196.0);
    let in_unison = magnitude(&unison, upper) / magnitude(&unison, 196.0);
    assert!(in_fifth > 0.2, "the upper voice sounds in a fifth: {in_fifth}");
    assert!(in_fifth > 10.0 * in_unison, "and not in unison: {in_fifth} vs {in_unison}");
}

#[test]
fn chaos_tips_the_song_into_noise() {
    let s = (0.6 * FS) as usize;
    let ap = |chaos: f64| {
        let out = hold(&[("chaos", chaos), ("detune", 0.0)], 1.0);
        let frame: Vec<f64> = out[s..s + 4096].iter().map(|&x| x as f64).collect();
        yin_pitch(&frame, FS, YinOpts { min_f0: Some(60.0), max_f0: Some(1000.0), threshold: Some(1.0) }).aperiodicity
    };
    let (calm, wild) = (ap(0.0), ap(1.0));
    assert!(wild > calm + 0.1, "aperiodicity {calm} → {wild}");
}
