//! Bellows (invented creature): a held note is finite, in range and on pitch, and its slow sac
//! squeezes swell and sag the level without ever letting the drone break.
use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(params: &[(&str, f64)], sec: f64) -> Vec<f32> {
    chimera::creatures::init();
    let def = creature_by_id("bellows").expect("bellows is registered");
    let p: Vec<(usize, f64)> = params.iter().map(|(k, x)| (def.param_index(k).expect("param"), *x)).collect();
    let mut v = CreatureVoice::new(def, FS, &p);
    v.set_gate(1.0);
    let mut out = vec![0f32; (sec * FS) as usize];
    for b in out.chunks_mut(128) {
        v.render(b);
    }
    out
}

/// RMS in 100 ms windows from `from` seconds on.
fn envelope(x: &[f32], from: f64) -> Vec<f64> {
    x[(from * FS) as usize..].chunks(4800).map(|w| (w.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>() / w.len() as f64).sqrt()).collect()
}

#[test]
fn held_note_is_finite_in_range_and_on_pitch() {
    let out = hold(&[], 1.5);
    assert!(out.iter().all(|x| x.is_finite()));
    let peak = out.iter().fold(0f32, |a, &b| a.max(b.abs()));
    assert!((0.2..0.95).contains(&peak), "peak {peak}");
    let s = (1.0 * FS) as usize;
    let frame: Vec<f64> = out[s..s + 4096].iter().map(|&x| x as f64).collect();
    let est = yin_pitch(&frame, FS, YinOpts { min_f0: Some(30.0), max_f0: Some(400.0), ..Default::default() });
    assert!((est.f0 - 65.0).abs() < 2.0, "pitch {}", est.f0);
}

#[test]
fn sac_squeezes_swell_the_drone_without_breaking_it() {
    // two full squeezes at a 1.5 s period
    let pulsed = envelope(&hold(&[("period", 1.5), ("depth", 0.9)], 4.0), 1.0);
    let (lo, hi) = pulsed.iter().fold((f64::INFINITY, 0f64), |(l, h), &e| (l.min(e), h.max(e)));
    assert!(hi / lo > 2.0, "squeezes should swell the level: {lo} .. {hi}");
    assert!(lo > 0.03 * hi, "the drone never breaks: {lo} .. {hi}");
    let steady = envelope(&hold(&[("depth", 0.0)], 4.0), 1.0);
    let (lo, hi) = steady.iter().fold((f64::INFINITY, 0f64), |(l, h), &e| (l.min(e), h.max(e)));
    assert!(hi / lo < 1.3, "no squeeze, no swell: {lo} .. {hi}");
}
