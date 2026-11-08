//! Stalker: a hiss through the inner jaw's teeth (turbulence, aperiodic), a shriek from pressed
//! folds and a whistle (periodic, high), and an inner jaw that strikes with a clack.
use chimera::analysis::fft::{Fft, hann, power_spectrum_db};
use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(fs: f64, params: &[(&str, f64)], sec: f64) -> Vec<f32> {
    let def = creature_by_id("stalker").unwrap();
    let init: Vec<(usize, f64)> = params
        .iter()
        .map(|(k, x)| (def.param_index(k).unwrap(), *x))
        .collect();
    let mut v = CreatureVoice::new(def, fs, &init);
    v.set_gate(1.0);
    let mut out = vec![0.0f32; (sec * fs) as usize];
    for b in out.chunks_mut(128) {
        v.render(b);
    }
    out
}

fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / x.len() as f64).sqrt()
}

fn peak(x: &[f32]) -> f64 {
    x.iter().fold(0.0, |m, &v| f64::max(m, (v as f64).abs()))
}

fn yin(x: &[f32], at: f64) -> (f64, f64) {
    let s = (at * FS) as usize;
    let frame: Vec<f64> = x[s..s + 4096].iter().map(|&v| v as f64).collect();
    let p = yin_pitch(
        &frame,
        FS,
        YinOpts {
            min_f0: Some(150.0),
            max_f0: Some(4000.0),
            ..Default::default()
        },
    );
    (p.f0, p.aperiodicity)
}

fn centroid(x: &[f32], at: f64) -> f64 {
    let n = 4096;
    let s = (at * FS) as usize;
    let frame: Vec<f64> = x[s..s + n].iter().map(|&v| v as f64).collect();
    let db = power_spectrum_db(&frame, &Fft::new(n), &hann(n));
    let (mut num, mut den) = (0.0, 0.0);
    for (k, d) in db.iter().enumerate() {
        let p = 10f64.powf(d / 20.0);
        num += p * k as f64 * FS / n as f64;
        den += p;
    }
    num / den
}

#[test]
fn held_shriek_is_high_and_in_tune() {
    let out = hold(FS, &[], 1.0);
    assert!(out.iter().all(|v| v.is_finite()));
    let p = peak(&out);
    assert!((0.2..1.0).contains(&p), "peak {p}");
    let (f, ap) = yin(&out, 0.6);
    assert!(
        (f / 800.0 - 1.0).abs() < 0.03 && ap < 0.2,
        "pitch {f} Hz (aperiodicity {ap}) for 800"
    );
    let c = centroid(&out, 0.6);
    assert!(c > 2000.0, "shriek centroid {c:.0} Hz");
}

#[test]
fn hiss_is_noise() {
    let out = hold(FS, &[("mode", 0.0)], 1.0);
    assert!(out.iter().all(|v| v.is_finite()));
    let level = rms(&out[(0.4 * FS) as usize..]);
    let (_, ap) = yin(&out, 0.6);
    assert!(level > 0.05, "hiss rms {level}");
    assert!(ap > 0.4, "hiss aperiodicity {ap}");
    assert!(centroid(&out, 0.6) > 2000.0);
}

#[test]
fn strike_clacks_and_shrieks() {
    let def = creature_by_id("stalker").unwrap();
    let mut v = CreatureVoice::new(def, FS, &[]);
    v.play_call_id("strike", None, 1.0);
    let mut out = vec![0.0f32; (1.7 * FS) as usize];
    for b in out.chunks_mut(128) {
        v.render(b);
    }
    assert!(out.iter().all(|v| v.is_finite()) && peak(&out) < 1.0);
    // hissing, retracted ...
    let (_, ap_before) = yin(&out, 0.2);
    // ... the jaw shoots out and it shrieks (rough at first, while the whistle at the teeth fights
    // the moving jaw, then settling on a pitch) ...
    let (f, ap_strike) = yin(&out, 0.72);
    assert!(
        ap_before > 0.3 && ap_strike < 0.2 && f > 600.0,
        "before ap {ap_before}, strike {f} Hz ap {ap_strike}"
    );
    // ... and the clack is a transient on top of the hiss
    let s = (0.5 * FS) as usize;
    let clack = peak(&out[s..s + 480]);
    let hiss = rms(&out[(0.2 * FS) as usize..(0.4 * FS) as usize]);
    assert!(clack > 3.0 * hiss, "clack {clack} over hiss rms {hiss}");
}

#[test]
fn extremes_stay_finite_at_every_rate() {
    for fs in [44100.0, 48000.0, 96000.0] {
        for reach in [0.0, 1.0] {
            for (pitch, mode) in [(250.0, 1.0), (2000.0, 1.0), (800.0, 0.0), (800.0, 0.5)] {
                let out = hold(
                    fs,
                    &[
                        ("reach", reach),
                        ("pitch", pitch),
                        ("mode", mode),
                        ("pressure", 40.0),
                        ("rasp", 1.0),
                        ("gape", 1.0),
                    ],
                    0.4,
                );
                assert!(
                    out.iter().all(|v| v.is_finite()),
                    "non-finite at {fs} Hz, reach {reach}, {pitch} Hz"
                );
            }
        }
    }
}
