//! Accordion Neck: vocal folds into a neck whose length changes while it sings. The note holds its
//! pitch, stretching the neck pulls the spectrum down, and no length in the range (built at the
//! shortest, so sections only grow) goes unstable.
use chimera::analysis::fft::{Fft, hann, power_spectrum_db};
use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(fs: f64, params: &[(&str, f64)], sec: f64) -> Vec<f32> {
    let def = creature_by_id("accordion").unwrap();
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

fn peak(x: &[f32]) -> f64 {
    x.iter().fold(0.0, |m, &v| f64::max(m, (v as f64).abs()))
}

/// Magnitude-weighted mean frequency of a 4096-sample frame starting at `at` seconds.
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
fn held_note_is_in_tune() {
    let out = hold(FS, &[], 1.0);
    assert!(out.iter().all(|v| v.is_finite()));
    let p = peak(&out);
    assert!((0.2..1.0).contains(&p), "peak {p}");
    let s = (0.6 * FS) as usize;
    let frame: Vec<f64> = out[s..s + 4096].iter().map(|&v| v as f64).collect();
    let f = yin_pitch(
        &frame,
        FS,
        YinOpts {
            min_f0: Some(60.0),
            max_f0: Some(800.0),
            ..Default::default()
        },
    )
    .f0;
    assert!((f / 180.0 - 1.0).abs() < 0.02, "pitch {f} Hz for 180");
}

#[test]
fn stretching_the_neck_lowers_the_spectrum() {
    let short = centroid(&hold(FS, &[("neck", 15.0)], 0.8), 0.6);
    let long = centroid(&hold(FS, &[("neck", 85.0)], 0.8), 0.6);
    assert!(
        long < 0.8 * short,
        "centroid {short:.0} Hz short, {long:.0} Hz long"
    );
    // and mid-call, as the neck unfolds: the same note, a darker sound
    let def = creature_by_id("accordion").unwrap();
    let mut v = CreatureVoice::new(def, FS, &[]);
    v.play_call_id("stretch", None, 1.0);
    let mut out = vec![0.0f32; (2.2 * FS) as usize];
    for b in out.chunks_mut(128) {
        v.render(b);
    }
    let (early, late) = (centroid(&out, 0.3), centroid(&out, 1.9));
    assert!(
        late < 0.85 * early,
        "call centroid {early:.0} → {late:.0} Hz"
    );
}

#[test]
fn every_neck_length_stays_stable() {
    // the neck's sections are sized at its shortest length; stretching only lengthens them, so the
    // CFL bound holds everywhere, at every rate, with the most extreme shape
    for fs in [44100.0, 48000.0, 96000.0] {
        for neck in [14.0, 30.0, 60.0, 90.0] {
            for (pleats, mouth) in [(0.0, 1.0), (1.0, 0.0)] {
                let out = hold(
                    fs,
                    &[
                        ("neck", neck),
                        ("pitch", 600.0),
                        ("pressure", 30.0),
                        ("pleats", pleats),
                        ("mouth", mouth),
                        ("pouch", 1.0),
                    ],
                    0.4,
                );
                assert!(
                    out.iter().all(|v| v.is_finite()),
                    "non-finite at {fs} Hz, neck {neck} cm"
                );
            }
        }
    }
}

#[test]
fn calls_stay_under_full_scale() {
    let def = creature_by_id("accordion").unwrap();
    for c in &def.calls {
        let mut v = CreatureVoice::new(def, FS, &[]);
        v.play_call_id(c.id, None, 1.0);
        let mut out = vec![0.0f32; ((c.keys.last().unwrap().t + 0.3) * FS) as usize];
        for b in out.chunks_mut(128) {
            v.render(b);
        }
        let p = peak(&out);
        assert!(
            out.iter().all(|v| v.is_finite()) && p > 0.1 && p < 1.0,
            "{}: peak {p}",
            c.id
        );
    }
}
