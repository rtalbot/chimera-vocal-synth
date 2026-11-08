//! Swarm: its words are rhythm and density, so more density must mean more clicks per second, and
//! sync must gather them onto the pulse.
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(params: &[(&str, f64)], secs: f64) -> Vec<f32> {
    chimera::creatures::init();
    let def = creature_by_id("swarm").expect("swarm registered");
    let init: Vec<(usize, f64)> = params.iter().map(|(k, v)| (def.param_index(k).expect("param"), *v)).collect();
    let mut v = CreatureVoice::new(def, FS, &init);
    v.set_gate(1.0);
    let mut out = vec![0f32; (secs * FS) as usize];
    for block in out.chunks_mut(128) {
        v.render(block);
    }
    out
}

/// 1 ms frame RMS
fn frames(x: &[f32]) -> Vec<f64> {
    x.chunks(48).map(|c| (c.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / c.len() as f64).sqrt()).collect()
}

/// onsets per second after the first 100 ms. A click's attack is far steeper than the horns'
/// ringing, so count the spikes of the signal's slope (a 0.5 ms refractory merges each attack).
fn onset_rate(x: &[f32]) -> f64 {
    let x = &x[4800..];
    let d: Vec<f64> = x.windows(2).map(|w| (w[1] - w[0]).abs() as f64).collect();
    let th = 0.15 * d.iter().cloned().fold(0.0, f64::max);
    let (mut n, mut last) = (0, -100i64);
    for (i, v) in d.iter().enumerate() {
        if *v > th {
            if i as i64 - last > 24 {
                n += 1;
            }
            last = i as i64;
        }
    }
    n as f64 / (x.len() as f64 / FS)
}

#[test]
fn holds_finite_and_in_range() {
    let x = hold(&[], 1.0);
    assert!(x.iter().all(|v| v.is_finite()));
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
    assert!((0.05..0.95).contains(&peak), "peak {peak}");
}

#[test]
fn more_density_more_onsets() {
    let sparse = onset_rate(&hold(&[("density", 30.0), ("sync", 0.0)], 1.5));
    let dense = onset_rate(&hold(&[("density", 300.0), ("sync", 0.0)], 1.5));
    assert!(sparse > 10.0 && sparse < 60.0, "30 clicks/s heard as {sparse} onsets/s");
    assert!(dense > 2.5 * sparse, "density 300 gives {dense} onsets/s, density 30 gives {sparse}");
}

#[test]
fn sync_gathers_clicks_onto_the_pulse() {
    // pulse 4 Hz: with full sync, the second half of every beat is silent
    let x = hold(&[("density", 400.0), ("pulse", 4.0), ("sync", 1.0)], 1.5);
    let f = frames(&x);
    let beat = 250; // ms
    let (mut on, mut off) = (0.0, 0.0);
    for (i, v) in f.iter().enumerate().skip(beat) {
        let ph = (i % beat) as f64 / beat as f64;
        if ph < 0.1 {
            on += v * v;
        } else if ph > 0.4 {
            off += v * v;
        }
    }
    assert!(on > 20.0 * off, "energy in the pulse window {on}, between pulses {off}");
}
