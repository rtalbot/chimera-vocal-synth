//! Whistler: a whistle locked to a pipe that is the whole body. It sustains when held, plays the
//! keyboard's pitch, and stretching the body slides the note down and then snaps it up to the next
//! resonance.
use chimera::analysis::pitch::{YinOpts, yin_pitch};
use chimera::creatures::{CreatureVoice, creature_by_id};

const FS: f64 = 48000.0;

fn hold(fs: f64, params: &[(&str, f64)], sec: f64) -> Vec<f32> {
    let def = creature_by_id("whistler").unwrap();
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

fn f0(x: &[f32], at: f64) -> f64 {
    let s = (at * FS) as usize;
    let frame: Vec<f64> = x[s..s + 4096].iter().map(|&v| v as f64).collect();
    yin_pitch(
        &frame,
        FS,
        YinOpts {
            min_f0: Some(200.0),
            max_f0: Some(4000.0),
            ..Default::default()
        },
    )
    .f0
}

#[test]
fn held_note_sustains_in_tune() {
    let out = hold(FS, &[], 1.2);
    assert!(out.iter().all(|v| v.is_finite()));
    let p = peak(&out);
    assert!((0.3..1.0).contains(&p), "peak {p}");
    let f = f0(&out, 0.6);
    assert!((f / 880.0 - 1.0).abs() < 0.03, "pitch {f} Hz for 880");
    // a self-oscillator, not a decaying ring: the last quarter second is as loud as the second
    let early = rms(&out[(0.25 * FS) as usize..(0.5 * FS) as usize]);
    let late = rms(&out[(0.95 * FS) as usize..(1.2 * FS) as usize]);
    assert!(early > 0.1 && late > 0.9 * early, "rms {early} then {late}");
}

#[test]
fn keys_play_in_tune_across_the_range() {
    for target in [520.0, 1000.0, 1900.0] {
        let out = hold(FS, &[("pitch", target)], 0.8);
        let f = f0(&out, 0.6);
        assert!((f / target - 1.0).abs() < 0.04, "pitch {f} Hz for {target}");
    }
}

#[test]
fn stretching_slides_then_snaps() {
    let f = |stretch: f64| f0(&hold(FS, &[("stretch", stretch)], 0.8), 0.6);
    let (a, b, c, d) = (f(0.9), f(1.0), f(1.1), f(1.2));
    // the body lengthens and its resonance pulls the whistle down with it...
    assert!(a > b && b > c && c < 0.92 * a, "slide {a} {b} {c}");
    // ...until the next resonance is nearer and the whistle jumps up to it
    assert!(d > 1.1 * c, "snap {c} → {d}");
    // without the lock the pipe barely matters
    let g = |stretch: f64| {
        f0(
            &hold(FS, &[("stretch", stretch), ("coupling", 0.0)], 0.8),
            0.6,
        )
    };
    let (x, y) = (g(0.9), g(1.1));
    assert!((x / y - 1.0).abs() < 0.03, "unlocked {x} {y}");
}

#[test]
fn extremes_stay_finite_at_every_rate() {
    let def = creature_by_id("whistler").unwrap();
    for fs in [44100.0, 48000.0, 96000.0] {
        for pitch in [def.params[0].min, def.params[0].max] {
            for stretch in [0.7, 1.5] {
                for flare in [0.0, 1.0] {
                    let out = hold(
                        fs,
                        &[
                            ("pitch", pitch),
                            ("stretch", stretch),
                            ("flare", flare),
                            ("breath", 1.0),
                            ("coupling", 2.0),
                        ],
                        0.4,
                    );
                    assert!(
                        out.iter().all(|v| v.is_finite()),
                        "non-finite at {fs} Hz, {pitch} Hz, stretch {stretch}"
                    );
                }
            }
        }
    }
}
