//! Vent Boiler: talks by boiling, so more heat must mean more pops, and the bubbles ring at the
//! played pitch.
use chimera::analysis::fft::{Fft, hann, power_spectrum_db};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(params: &[(&str, f64)], secs: f64) -> Vec<f32> {
    chimera::creatures::init();
    let def = creature_by_id("ventboiler").expect("ventboiler registered");
    let init: Vec<(usize, f64)> = params.iter().map(|(k, v)| (def.param_index(k).expect("param"), *v)).collect();
    let mut v = CreatureVoice::new(def, FS, &init);
    v.set_gate(1.0);
    let mut out = vec![0f32; (secs * FS) as usize];
    for block in out.chunks_mut(128) {
        v.render(block);
    }
    out
}

/// pops per second after the first 100 ms: a 1 ms frame that jumps well above the one before
fn pop_rate(x: &[f32]) -> f64 {
    let f: Vec<f64> = x[4800..].chunks(48).map(|c| (c.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / c.len() as f64).sqrt()).collect();
    let n = f.windows(2).filter(|w| w[1] > 0.005 && w[1] > 2.5 * w[0]).count();
    n as f64 / (f.len() as f64 / 1000.0)
}

#[test]
fn holds_finite_and_in_range() {
    for heat in [14.0, 200.0] {
        let x = hold(&[("heat", heat), ("roil", 1.0)], 1.0);
        assert!(x.iter().all(|v| v.is_finite()));
        let peak = x.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!((0.05..0.95).contains(&peak), "heat {heat}: peak {peak}");
    }
}

#[test]
fn more_heat_more_pops() {
    let low = pop_rate(&hold(&[("heat", 4.0), ("roil", 0.0)], 2.0));
    let high = pop_rate(&hold(&[("heat", 40.0), ("roil", 0.0)], 2.0));
    assert!(high > 3.0 * low.max(1.0), "heat 40 gives {high} pops/s, heat 4 gives {low}");
}

#[test]
fn bubbles_ring_at_the_played_pitch() {
    let pitch = 1200.0;
    let x = hold(&[("pitch", pitch), ("heat", 60.0), ("roil", 0.0), ("head", 120.0)], 1.5);
    let n = 8192;
    let fft = Fft::new(n);
    let mut acc = vec![0.0; n / 2 + 1];
    for s in (4800..x.len() - n).step_by(n / 2) {
        let frame: Vec<f64> = x[s..s + n].iter().map(|v| *v as f64).collect();
        for (a, d) in acc.iter_mut().zip(power_spectrum_db(&frame, &fft, &hann(n))) {
            *a += 10f64.powf(d / 10.0);
        }
    }
    let bin = FS / n as f64;
    let (lo, hi) = ((0.5 * pitch / bin) as usize, (2.0 * pitch / bin) as usize);
    let k = (lo..hi).max_by(|a, b| acc[*a].total_cmp(&acc[*b])).unwrap();
    let f = k as f64 * bin;
    assert!((f / pitch - 1.0).abs() < 0.08, "strongest component between an octave down and up at {f} Hz");
}
