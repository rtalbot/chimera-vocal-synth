//! Glass Singer: a held note is one strike that rings like glass at the played pitch, for longer
//! the higher the ring setting.
use chimera::analysis::fft::{Fft, hann, power_spectrum_db};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(params: &[(&str, f64)], secs: f64) -> Vec<f32> {
    chimera::creatures::init();
    let def = creature_by_id("glasssinger").expect("glasssinger registered");
    let init: Vec<(usize, f64)> = params.iter().map(|(k, v)| (def.param_index(k).expect("param"), *v)).collect();
    let mut v = CreatureVoice::new(def, FS, &init);
    v.set_gate(1.0);
    let mut out = vec![0f32; (secs * FS) as usize];
    for block in out.chunks_mut(128) {
        v.render(block);
    }
    out
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}

fn at(t: f64) -> usize {
    (t * FS) as usize
}

#[test]
fn a_strike_rings_at_the_played_pitch() {
    let pitch = 440.0;
    let x = hold(&[("pitch", pitch), ("roll", 0.0), ("bright", 0.3)], 1.0);
    assert!(x.iter().all(|v| v.is_finite()));
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!((0.05..0.95).contains(&peak), "peak {peak}");
    // the strongest partial near the pitch is the pitch (first partial of the long spire)
    let n = 16384;
    let frame: Vec<f64> = x[at(0.05)..at(0.05) + n].iter().map(|v| *v as f64).collect();
    let db = power_spectrum_db(&frame, &Fft::new(n), &hann(n));
    let bin = FS / n as f64;
    let (lo, hi) = ((0.6 * pitch / bin) as usize, (1.3 * pitch / bin) as usize);
    let k = (lo..hi).max_by(|a, b| db[*a].total_cmp(&db[*b])).unwrap();
    let f = k as f64 * bin;
    assert!((f / pitch - 1.0).abs() < 0.05, "strongest partial near the pitch at {f} Hz");
}

#[test]
fn more_ring_rings_longer() {
    let decay = |ring: f64| {
        let x = hold(&[("ring", ring), ("roll", 0.0)], 1.0);
        rms(&x[at(0.6)..at(0.9)]) / rms(&x[at(0.02)..at(0.3)])
    };
    let (short, long) = (decay(0.1), decay(0.9));
    assert!(long > 0.1, "a glassy ring is still sounding after 0.6 s ({long})");
    assert!(long > 5.0 * short, "ring 0.9 keeps {long} of its level, ring 0.1 {short}");
}
