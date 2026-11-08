//! Alien languages (`creatures::language`): every invented creature has one, its sounds exist on
//! that creature, translation is deterministic, prosody follows the punctuation, and every alien
//! speaks cleanly.

use chimera::creatures::language::{LANGUAGES, SpeakOptions, language_of, load_and_play, speak};
use chimera::creatures::types::{CallTarget, CreatureGroup, ParamRole};
use chimera::creatures::{CREATURES, CreatureVoice, creature_by_id};

#[test]
fn every_invented_creature_has_a_language_and_only_they_do() {
    for def in CREATURES.iter() {
        assert_eq!(language_of(def.id).is_some(), def.group == CreatureGroup::Fantasy, "{}", def.id);
    }
    for l in LANGUAGES {
        assert!(creature_by_id(l.creature).is_some(), "{} names a creature that doesn't exist", l.name);
    }
}

#[test]
fn every_sound_names_a_param_or_event_of_its_creature() {
    for l in LANGUAGES {
        let def = creature_by_id(l.creature).unwrap();
        let sets = l.sustains.iter().chain(l.accents.iter()).map(|p| p.set).chain(std::iter::once(l.hold));
        for set in sets {
            for (k, _) in set {
                let ok = match k.strip_prefix('!') {
                    Some(e) => def.events.contains(&e),
                    None => def.param_index(k).is_some(),
                };
                assert!(ok, "{} ({}): {k:?} is not one of its params or events", l.name, def.id);
            }
        }
        assert!(!l.sustains.is_empty(), "{} has no sustained sounds", l.name);
    }
}

#[test]
fn the_same_text_always_says_the_same_and_words_differ() {
    let def = creature_by_id("accordion").unwrap();
    let a = speak(def, "Where are you going?", &SpeakOptions::default()).unwrap();
    let b = speak(def, "where are you going?", &SpeakOptions::default()).unwrap();
    assert_eq!(a.keys, b.keys, "case doesn't change the words");
    assert_eq!(a.alien_text(), b.alien_text());
    // a vocabulary of everyday words: most map to different alien words
    let words = ["hello", "water", "friend", "danger", "home", "star", "food", "sleep", "light", "dark", "go", "stay"];
    let mut seen = std::collections::HashSet::new();
    for w in words {
        seen.insert(speak(def, w, &SpeakOptions::default()).unwrap().words[0].alien.clone());
    }
    assert!(seen.len() >= words.len() - 2, "only {} distinct alien words for {} English ones", seen.len(), words.len());
}

/// The pitch param's value in the last key that sets it.
fn last_pitch(def: &chimera::creatures::CreatureDef, text: &str) -> f64 {
    let pi = def.params.iter().position(|p| p.role == Some(ParamRole::Pitch)).unwrap();
    let s = speak(def, text, &SpeakOptions::default()).unwrap();
    s.keys.iter().rev().find_map(|k| k.v.iter().find(|(n, _)| *n == CallTarget::Param(pi)).map(|(_, x)| *x)).unwrap()
}

#[test]
fn a_question_rises_at_its_end_and_a_statement_falls() {
    for l in LANGUAGES {
        let def = creature_by_id(l.creature).unwrap();
        let q = last_pitch(def, "are you coming home?");
        let st = last_pitch(def, "are you coming home.");
        assert!(q > st * 1.1, "{}: question ends at {q:.1}, statement at {st:.1}", l.name);
    }
}

#[test]
fn commas_and_full_stops_are_pauses_and_words_are_timed_in_order() {
    let def = creature_by_id("stalker").unwrap();
    let run = speak(def, "come here now", &SpeakOptions::default()).unwrap();
    let paused = speak(def, "come, here. now", &SpeakOptions::default()).unwrap();
    assert!(paused.duration > run.duration + 0.2, "pauses: {:.2} s vs {:.2} s", paused.duration, run.duration);
    for s in [&run, &paused] {
        for w in s.words.windows(2) {
            assert!(w[0].t0 < w[0].t1 && w[0].t1 <= w[1].t0, "words overlap: {w:?}");
        }
    }
    let fast = speak(def, "come here now", &SpeakOptions { rate: Some(2.0), ..Default::default() }).unwrap();
    assert!(fast.duration < run.duration * 0.6);
    assert!(speak(def, " ... ", &SpeakOptions::default()).is_none(), "nothing to say");
}

#[test]
fn every_alien_speaks_cleanly() {
    let text = "Hello, traveller. Where are you going? We have waited for you!";
    for l in LANGUAGES {
        let def = creature_by_id(l.creature).unwrap();
        let s = speak(def, text, &SpeakOptions::default()).unwrap();
        let mut v = CreatureVoice::new(def, 48000.0, &[]);
        assert!(load_and_play(&mut v, &s, 1.0), "{}: the utterance fits", l.name);
        let n = ((s.duration + 0.5) * 48000.0) as usize;
        let mut out = vec![0f32; n];
        for b in out.chunks_mut(128) {
            v.render(b);
        }
        assert!(out.iter().all(|x| x.is_finite()), "{}: non-finite output", l.name);
        let peak = out.iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.05 && peak <= 1.0, "{}: peak {peak}", l.name);
        // it is heard while the words are said, and it ends in silence
        let rms = |a: f64, b: f64| {
            let s = &out[(a * 48000.0) as usize..(b * 48000.0) as usize];
            (s.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / s.len() as f64).sqrt()
        };
        let w = &s.words[0];
        assert!(rms(w.t0, w.t1) > 0.01, "{}: the first word is silent", l.name);
        assert!(rms(s.duration + 0.4, s.duration + 0.5) < 0.2 * rms(w.t0, w.t1), "{}: it doesn't fall silent at the end", l.name);
        assert_eq!(v.active_call(), None, "{}: the spoken call ended", l.name);
    }
}

#[test]
fn the_flat_form_plays_exactly_like_the_planned_utterance() {
    use chimera::creatures::language::{encode, play_flat};
    for id in ["glasssinger", "stalker"] {
        let def = creature_by_id(id).unwrap();
        let s = speak(def, "Strike now, then wait.", &SpeakOptions::default()).unwrap();
        let render = |v: &mut CreatureVoice| {
            let mut out = vec![0f32; ((s.duration + 0.3) * 48000.0) as usize];
            for b in out.chunks_mut(128) {
                v.render(b);
            }
            out
        };
        let mut a = CreatureVoice::new(def, 48000.0, &[]);
        load_and_play(&mut a, &s, 1.0);
        let mut b = CreatureVoice::new(def, 48000.0, &[]);
        assert!(play_flat(&mut b, &encode(&s), 1.0), "{id}: the flat form loads whole");
        assert_eq!(render(&mut a), render(&mut b), "{id}: same audio");
        // malformed input loads what it can and says so, without panicking
        let flat = encode(&s);
        let mut c = CreatureVoice::new(def, 48000.0, &[]);
        assert!(!play_flat(&mut c, &flat[..flat.len() - 1], 1.0));
        assert!(!play_flat(&mut c, &[0.0, 0.0, f64::NAN], 1.0));
    }
}

#[test]
fn a_sung_word_stays_on_its_note_and_lasts_longer() {
    let def = creature_by_id("resonant").unwrap();
    let pi = def.param_index("pitch").unwrap();
    let opts = SpeakOptions { pitch: Some(330.0), sung: Some(true), ..Default::default() };
    let sung = speak(def, "traveller?", &opts).unwrap();
    for k in &sung.keys {
        if let Some((_, x)) = k.v.iter().find(|(n, _)| *n == CallTarget::Param(pi)) {
            assert_eq!(*x, 330.0, "every syllable on the note");
        }
    }
    // (unpunctuated: a spoken question also ends in a pause, a sung word doesn't)
    let sung = speak(def, "traveller", &opts).unwrap();
    let said = speak(def, "traveller", &SpeakOptions { pitch: Some(330.0), ..Default::default() }).unwrap();
    assert!(sung.duration > said.duration * 1.2, "sung {:.2} s, said {:.2} s", sung.duration, said.duration);
    assert_eq!(sung.words[0].alien, said.words[0].alien, "the same word either way");
}
