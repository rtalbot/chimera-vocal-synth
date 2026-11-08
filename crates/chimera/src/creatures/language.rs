//! Alien languages: English text → an utterance in an invented creature's own sounds.
//!
//! Each alien speaks with what its body can do. Its [`Language`] lists sustained sounds (the
//! "vowels": a snapshot of some of its params, held for a syllable) and percussive sounds (the
//! "consonants": a short burst, or a one-shot event such as a strike or a pop). A word becomes a
//! fixed string of these, chosen by hashing the English word, so the same word always comes out
//! the same and the language stays consistent. The languages are tonal: each syllable also takes a
//! low, middle or high tone from the hash (written ` and ´ in the transliteration). Prosody comes from the text: pitch drifts down over
//! a phrase, stressed (first) syllables are longer and higher, a question rises at its end, an
//! exclamation is louder and higher, and commas and full stops are pauses.
//!
//! [`speak`] plans the utterance (allocating, off the audio thread) as call keys; a
//! [`CreatureVoice`](super::CreatureVoice) loads them into its reserved spoken call and plays them
//! without allocating.

use super::types::{CallTarget, CreatureDef, Curve, ParamRole};
use crate::math::{Rng, js_max, js_min};
use serde::Serialize;

/// One sound of a language: params to set while it sounds (`"!name"` keys fire an event).
#[derive(Debug)]
pub struct Phone {
    /// how it is written in the alien transliteration
    pub glyph: &'static str,
    pub set: &'static [(&'static str, f64)],
    /// length relative to the language's syllable
    pub dur: f64,
}

#[derive(Debug)]
pub struct Language {
    /// what the language is called
    pub name: &'static str,
    /// the creature it belongs to
    pub creature: &'static str,
    /// one sentence on how it sounds
    pub about: &'static str,
    /// syllable length (s) at rate 1
    pub syllable: f64,
    /// sustained sounds: every syllable has one
    pub sustains: &'static [Phone],
    /// percussive sounds: a syllable starts with one with probability `onset`
    pub accents: &'static [Phone],
    pub onset: f64,
    /// how far prosody moves the pitch (fraction of the base pitch)
    pub span: f64,
    /// the breath between words of a phrase (gate level, 0 = silence)
    pub joint: f64,
    /// params held for the whole utterance (e.g. a steady squeeze for the Bellows)
    pub hold: &'static [(&'static str, f64)],
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct SpeakOptions {
    /// base pitch (the pitch param's value); None: the param's default
    pub pitch: Option<f64>,
    /// speaking rate: 1 normal, 2 twice as fast
    pub rate: Option<f64>,
    /// sung, not spoken: every syllable on the base pitch (no tones, stress or phrase shape),
    /// held longer, with no pauses (the keys sing a word at a time)
    pub sung: Option<bool>,
}

/// One planned key: time (s), curve, and the values it sets (resolved targets).
#[derive(Clone, Debug, PartialEq)]
pub struct SpokenKey {
    pub t: f64,
    pub curve: Option<Curve>,
    pub v: Vec<(CallTarget, f64)>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpokenWord {
    /// the English word
    pub text: String,
    /// its alien form
    pub alien: String,
    pub t0: f64,
    pub t1: f64,
}

#[derive(Clone, Debug)]
pub struct Spoken {
    pub keys: Vec<SpokenKey>,
    pub words: Vec<SpokenWord>,
    pub duration: f64,
}

impl Spoken {
    /// The whole transliteration, words and punctuation as written.
    pub fn alien_text(&self) -> String {
        self.words.iter().map(|w| w.alien.as_str()).collect::<Vec<_>>().join(" ")
    }
}

/// The language of a creature, if it has one.
pub fn language_of(id: &str) -> Option<&'static Language> {
    LANGUAGES.iter().find(|l| l.creature == id)
}

enum Token {
    Word(String),
    /// a pause: comma-like (false) or a phrase end (true), with the mark that ends it
    Pause { full: bool, mark: char },
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut Vec<Token>| {
        if !w.is_empty() {
            out.push(Token::Word(std::mem::take(w)));
        }
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' {
            word.extend(ch.to_lowercase());
        } else {
            flush(&mut word, &mut out);
            match ch {
                ',' | ';' | ':' | '—' | '-' => out.push(Token::Pause { full: false, mark: ch }),
                '.' | '!' | '?' => out.push(Token::Pause { full: true, mark: ch }),
                _ => {}
            }
        }
    }
    flush(&mut word, &mut out);
    out
}

/// Syllables in an English spelling: vowel groups, a silent final e dropped (1..=4).
fn syllables(word: &str) -> usize {
    let w: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    if w.is_empty() {
        // a number: one syllable per digit, up to four
        return word.chars().filter(|c| c.is_ascii_digit()).count().clamp(1, 4);
    }
    let vowel = |c: char| "aeiouy".contains(c);
    let mut n = 0;
    let mut prev = false;
    for &c in &w {
        let v = vowel(c);
        if v && !prev {
            n += 1;
        }
        prev = v;
    }
    if n > 1 && w.len() > 2 && w[w.len() - 1] == 'e' && !vowel(w[w.len() - 2]) {
        n -= 1;
    }
    n.clamp(1, 4)
}

/// FNV-1a: a stable seed per word.
fn hash(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn pick<'a>(list: &'a [Phone], rng: &mut Rng) -> &'a Phone {
    &list[((rng.next() * list.len() as f64) as usize).min(list.len() - 1)]
}

fn target(def: &CreatureDef, name: &str) -> CallTarget {
    if name == "gate" {
        CallTarget::Gate
    } else if let Some(e) = name.strip_prefix('!') {
        CallTarget::Event(def.events.iter().position(|x| *x == e))
    } else if let Some(i) = def.param_index(name) {
        CallTarget::Param(i)
    } else {
        CallTarget::Unknown
    }
}

/// One syllable as planned, before times and pitches are laid out.
struct Syl {
    accent: Option<&'static Phone>,
    sustain: &'static Phone,
    /// −1 low, 0 middle, 1 high
    tone: i8,
    stressed: bool,
    /// index of the word it belongs to
    word: usize,
}

/// Plan `text` in `def`'s language. None when the creature has none (or nothing is said).
pub fn speak(def: &CreatureDef, text: &str, opts: &SpeakOptions) -> Option<Spoken> {
    let lang = language_of(def.id)?;
    let rate = js_max(0.25, js_min(4.0, opts.rate.unwrap_or(1.0)));
    let sung = opts.sung.unwrap_or(false);
    let syl_len = lang.syllable / rate * if sung { 1.5 } else { 1.0 };
    let pitch_i = def.params.iter().position(|p| p.role == Some(ParamRole::Pitch));
    let base = match (opts.pitch, pitch_i) {
        (Some(p), Some(_)) if p.is_finite() && p > 0.0 => p,
        (_, Some(i)) => def.params[i].default,
        _ => 0.0,
    };

    // phrases of syllables, with the mark that ends each and the pauses between
    struct Phrase {
        syls: Vec<Syl>,
        mark: char,
        /// pause after it (s)
        pause: f64,
    }
    let mut phrases: Vec<Phrase> = vec![Phrase { syls: Vec::new(), mark: '.', pause: 0.0 }];
    let mut words: Vec<SpokenWord> = Vec::new();
    for tok in tokenize(text) {
        match tok {
            Token::Word(w) => {
                let mut rng = Rng::new(hash(&w));
                let n = syllables(&w);
                let wi = words.len();
                let mut alien = String::new();
                for k in 0..n {
                    let accent = if !lang.accents.is_empty() && rng.next() < lang.onset { Some(pick(lang.accents, &mut rng)) } else { None };
                    let sustain = pick(lang.sustains, &mut rng);
                    let tone = ((rng.next() * 3.0) as i8).min(2) - 1;
                    if let Some(a) = accent {
                        alien.push_str(a.glyph);
                    }
                    // the tone mark sits on the sound's first vowel
                    let mark = match tone {
                        1 => Some('\u{301}'),
                        -1 => Some('\u{300}'),
                        _ => None,
                    };
                    let mut marked = false;
                    for c in sustain.glyph.chars() {
                        alien.push(c);
                        if !marked && "aeiouAEIOU".contains(c) {
                            if let Some(m) = mark {
                                alien.push(m);
                            }
                            marked = true;
                        }
                    }
                    if !marked && let Some(m) = mark {
                        alien.push(m);
                    }
                    phrases.last_mut().unwrap().syls.push(Syl { accent, sustain, tone, stressed: k == 0, word: wi });
                }
                words.push(SpokenWord { text: w, alien, t0: 0.0, t1: 0.0 });
            }
            Token::Pause { full, mark } => {
                let cur = phrases.last_mut().unwrap();
                if cur.syls.is_empty() {
                    continue;
                }
                cur.mark = mark;
                cur.pause = if sung { 0.0 } else if full { 2.2 * syl_len } else { 1.0 * syl_len };
                if let Some(w) = words.last_mut() {
                    w.alien.push(mark);
                }
                phrases.push(Phrase { syls: Vec::new(), mark: '.', pause: 0.0 });
            }
        }
    }
    phrases.retain(|p| !p.syls.is_empty());
    if phrases.is_empty() {
        return None;
    }

    let mut keys: Vec<SpokenKey> = Vec::new();
    let mut push = |t: f64, curve: Option<Curve>, v: Vec<(CallTarget, f64)>| keys.push(SpokenKey { t, curve, v });
    let held: Vec<(CallTarget, f64)> = lang.hold.iter().map(|(k, x)| (target(def, k), *x)).filter(|(n, _)| *n != CallTarget::Unknown).collect();
    let values = |set: &[(&'static str, f64)], pitch: f64, gate: f64, events: bool| -> Vec<(CallTarget, f64)> {
        let mut v: Vec<(CallTarget, f64)> = vec![(CallTarget::Gate, gate)];
        if let Some(i) = pitch_i {
            v.push((CallTarget::Param(i), pitch));
        }
        v.extend(held.iter().copied());
        for &(k, x) in set {
            let n = target(def, k);
            if matches!(n, CallTarget::Event(_)) && !events {
                continue;
            }
            if n != CallTarget::Unknown && !v.iter().any(|(m, _)| *m == n) {
                v.push((n, x));
            }
        }
        v
    };

    let first = &phrases[0].syls[0];
    let lead = 0.06;
    // start silent, already shaped like the first sound, so the breath opens onto it
    push(0.0, None, values(first.sustain.set, base, 0.0, false));
    let mut t = lead;
    for ph in &phrases {
        let n = ph.syls.len();
        let (lift, loud) = match ph.mark {
            '!' => (1.0 + 0.5 * lang.span, 1.25),
            _ => (1.0, 1.0),
        };
        for (i, s) in ph.syls.iter().enumerate() {
            let u = if n > 1 { i as f64 / (n - 1) as f64 } else { 0.0 };
            let last = i == n - 1;
            // declination, stress, and the phrase's end
            let mut f = 1.0 + lang.span * (0.25 - 0.5 * u + 0.6 * s.tone as f64) + if s.stressed { 0.25 * lang.span } else { 0.0 };
            if last {
                f *= match ph.mark {
                    '?' => 1.0 + 1.2 * lang.span,
                    ',' | ';' | ':' => 1.0 + 0.2 * lang.span,
                    _ => 1.0 - 0.4 * lang.span,
                };
            }
            let pitch = if sung { base } else { base * f * lift };
            let gate = (if s.stressed { 1.0 } else { 0.8 }) * loud;
            let new_word = i == 0 || ph.syls[i - 1].word != s.word;
            if new_word {
                words[s.word].t0 = t;
            }
            if let Some(a) = s.accent {
                let d = js_max(0.03, a.dur * 0.4 * syl_len);
                push(t, Some(Curve::Step), values(a.set, pitch, gate, true));
                t += d;
            }
            let d = s.sustain.dur * syl_len * if s.stressed { 1.3 } else { 1.0 } * if last { 1.4 } else { 1.0 };
            // reach the sound quickly, hold it, glide out over the syllable's last third
            push(t, None, values(s.sustain.set, pitch, gate, true));
            push(t + 0.66 * d, None, values(s.sustain.set, pitch, gate, false));
            t += d;
            let word_ends = last || ph.syls[i + 1].word != s.word;
            if word_ends {
                words[s.word].t1 = t;
                if !last && lang.joint < 1.0 {
                    // a breath between words
                    let gap = 0.35 * syl_len;
                    push(t, None, vec![(CallTarget::Gate, lang.joint * gate)]);
                    t += gap;
                }
            }
        }
        if ph.pause > 0.0 {
            push(t, None, vec![(CallTarget::Gate, 0.0)]);
            t += ph.pause;
        }
    }
    push(t, None, vec![(CallTarget::Gate, 0.0)]);
    let duration = t + 0.05;
    Some(Spoken { keys, words, duration })
}

const fn ph(glyph: &'static str, set: &'static [(&'static str, f64)], dur: f64) -> Phone {
    Phone { glyph, set, dur }
}

/// The languages, one per invented creature.
pub static LANGUAGES: &[Language] = &[
    Language {
        name: "Chordal",
        creature: "resonant",
        about: "Every syllable is a chord: the shape of the chord is the vowel; tense blasts are the consonants.",
        syllable: 0.3,
        sustains: &[
            ph("om", &[("third", 4.0), ("fifth", 7.0), ("voices", 1.0), ("tension", 0.4)], 1.0),
            ph("um", &[("third", 3.0), ("fifth", 7.0), ("voices", 1.0), ("tension", 0.3)], 1.0),
            ph("eth", &[("third", 5.0), ("fifth", 10.0), ("voices", 1.0), ("tension", 0.6)], 1.0),
            ph("ai", &[("third", 7.0), ("fifth", 12.0), ("voices", 1.0), ("tension", 0.8)], 0.9),
            ph("ul", &[("third", 0.0), ("fifth", 0.0), ("voices", 0.3), ("tension", 0.2)], 1.1),
            ph("ree", &[("third", 2.0), ("fifth", 9.0), ("voices", 0.8), ("tension", 0.9)], 0.8),
        ],
        accents: &[
            ph("t", &[("voices", 0.0), ("tension", 1.0), ("pressure", 34.0)], 0.6),
            ph("v", &[("voices", 0.5), ("tension", 0.0), ("pressure", 14.0)], 0.8),
        ],
        onset: 0.5,
        span: 0.18,
        joint: 0.2,
        hold: &[("pressure", 22.0)],
    },
    Language {
        name: "Swell-speech",
        creature: "bellows",
        about: "Slow, unbroken drones: brightness and the horn's flare carry the meaning, surges of the sac mark the syllables.",
        syllable: 0.45,
        sustains: &[
            ph("oo", &[("bright", 0.1), ("flare", 0.2)], 1.0),
            ph("aa", &[("bright", 0.9), ("flare", 0.8)], 1.0),
            ph("oh", &[("bright", 0.4), ("flare", 0.6)], 1.0),
            ph("eh", &[("bright", 0.7), ("flare", 0.3)], 0.9),
        ],
        accents: &[ph("hm", &[("pressure", 24.0), ("bright", 0.95)], 1.0)],
        onset: 0.35,
        span: 0.12,
        joint: 0.6,
        hold: &[("depth", 0.1), ("pressure", 14.0)],
    },
    Language {
        name: "Twofold",
        creature: "chorister",
        about: "Two voices in one throat: the interval between them and the mouth's opening are the vowels; growls and swaps are the consonants.",
        syllable: 0.24,
        sustains: &[
            ph("ae", &[("interval", 7.0), ("mouth", 0.9), ("chaos", 0.05)], 1.0),
            ph("oo", &[("interval", 0.0), ("mouth", 0.3), ("chaos", 0.05)], 1.0),
            ph("ih", &[("interval", 5.0), ("mouth", 0.6), ("chaos", 0.1)], 0.9),
            ph("aw", &[("interval", -5.0), ("mouth", 0.8), ("chaos", 0.1)], 1.0),
            ph("ue", &[("interval", 4.0), ("mouth", 0.45), ("chaos", 0.05)], 1.0),
        ],
        accents: &[
            ph("gr", &[("chaos", 0.8), ("mouth", 0.5)], 0.8),
            ph("k", &[("balance", 1.0), ("mouth", 0.1)], 0.5),
            ph("b", &[("balance", 0.0), ("mouth", 0.05)], 0.5),
        ],
        onset: 0.6,
        span: 0.22,
        joint: 0.15,
        hold: &[("detune", 12.0), ("pressure", 12.0)],
    },
    Language {
        name: "Deepsong",
        creature: "skywhale",
        about: "Moans you feel more than hear: the horn's mouth and brightness shape each long syllable, groans break them up.",
        syllable: 0.7,
        sustains: &[
            ph("OOM", &[("mouth", 0.3), ("bright", 0.3), ("rough", 0.05)], 1.0),
            ph("AAH", &[("mouth", 0.95), ("bright", 0.8), ("rough", 0.05)], 1.0),
            ph("ORR", &[("mouth", 0.6), ("bright", 0.5), ("rough", 0.6)], 1.0),
            ph("EEN", &[("mouth", 0.8), ("bright", 1.0), ("rough", 0.0)], 0.9),
        ],
        accents: &[ph("KH", &[("rough", 1.0), ("pressure", 14.0)], 1.0)],
        onset: 0.3,
        span: 0.25,
        joint: 0.5,
        hold: &[("pressure", 8.0)],
    },
    Language {
        name: "Chime",
        creature: "glasssinger",
        about: "Every syllable is a struck spire: brightness and ring time are the vowels, a trill is a long one.",
        syllable: 0.26,
        sustains: &[
            ph("ting", &[("bright", 0.9), ("ring", 0.5), ("roll", 0.0), ("!strike", 1.0)], 1.0),
            ph("tong", &[("bright", 0.2), ("ring", 0.9), ("roll", 0.0), ("!strike", 1.0)], 1.2),
            ph("tel", &[("bright", 0.5), ("ring", 0.7), ("roll", 0.0), ("!strike", 0.7)], 1.0),
            ph("tirri", &[("bright", 0.7), ("ring", 0.6), ("roll", 14.0), ("!strike", 0.8)], 1.3),
        ],
        accents: &[ph("k", &[("bright", 1.0), ("ring", 0.1), ("!strike", 1.0)], 0.6)],
        onset: 0.25,
        span: 0.3,
        joint: 0.0,
        hold: &[("strike", 0.8)],
    },
    Language {
        name: "Hum-of-many",
        creature: "swarm",
        about: "Meaning is in rhythm and density: a buzz, a ticking, a rattle; bursts are the consonants.",
        syllable: 0.22,
        sustains: &[
            ph("zz", &[("density", 1500.0), ("sync", 0.0), ("pulse", 6.0), ("scatter", 0.3)], 1.0),
            ph("tk", &[("density", 80.0), ("sync", 1.0), ("pulse", 14.0), ("scatter", 0.5)], 1.0),
            ph("rr", &[("density", 400.0), ("sync", 0.8), ("pulse", 22.0), ("scatter", 0.4)], 1.0),
            ph("sh", &[("density", 3000.0), ("sync", 0.0), ("pulse", 6.0), ("scatter", 1.0)], 1.0),
        ],
        accents: &[ph("tz", &[("!burst", 1.0)], 0.5)],
        onset: 0.4,
        span: 0.25,
        joint: 0.0,
        hold: &[],
    },
    Language {
        name: "Bubbletongue",
        creature: "ventboiler",
        about: "It talks by boiling: how hard it boils and the shape of its head are the vowels, big single bubbles are the consonants.",
        syllable: 0.26,
        sustains: &[
            ph("blub", &[("heat", 30.0), ("head", 150.0), ("mouth", 0.3)], 1.0),
            ph("glo", &[("heat", 60.0), ("head", 300.0), ("mouth", 0.6)], 1.0),
            ph("bah", &[("heat", 120.0), ("head", 240.0), ("mouth", 0.9)], 1.0),
            ph("plip", &[("heat", 10.0), ("head", 500.0), ("mouth", 0.5)], 0.8),
        ],
        accents: &[ph("p", &[("!pop", 1.0)], 0.5)],
        onset: 0.45,
        span: 0.25,
        joint: 0.0,
        hold: &[("roil", 0.4)],
    },
    Language {
        name: "Pipe-song",
        creature: "whistler",
        about: "Whistled syllables: stretching its body snaps the whistle between notes; breath dips are the consonants.",
        syllable: 0.2,
        sustains: &[
            ph("wee", &[("stretch", 0.85), ("flare", 0.2)], 1.0),
            ph("woo", &[("stretch", 1.3), ("flare", 0.7)], 1.0),
            ph("wi", &[("stretch", 1.0), ("flare", 0.4)], 0.8),
            ph("weo", &[("stretch", 1.15), ("flare", 0.5)], 1.1),
        ],
        accents: &[ph("h", &[("breath", 0.25)], 0.6)],
        onset: 0.35,
        span: 0.2,
        joint: 0.0,
        hold: &[("breath", 0.75), ("coupling", 1.4)],
    },
    Language {
        name: "Neckvowel",
        creature: "accordion",
        about: "The closest to speech: neck length and the pouch along it make real vowel-like colours; a closing mouth is the consonant.",
        syllable: 0.22,
        sustains: &[
            ph("a", &[("neck", 20.0), ("pouch", 0.7), ("mouth", 0.9)], 1.0),
            ph("o", &[("neck", 60.0), ("pouch", 0.3), ("mouth", 0.4)], 1.0),
            ph("u", &[("neck", 85.0), ("pouch", 0.2), ("mouth", 0.2)], 1.0),
            ph("i", &[("neck", 15.0), ("pouch", 0.85), ("mouth", 0.5)], 0.9),
            ph("e", &[("neck", 30.0), ("pouch", 0.6), ("mouth", 0.7)], 1.0),
        ],
        accents: &[
            ph("m", &[("mouth", 0.03)], 0.7),
            ph("l", &[("mouth", 0.25), ("pleats", 0.8)], 0.6),
        ],
        onset: 0.55,
        span: 0.2,
        joint: 0.1,
        hold: &[("pressure", 12.0)],
    },
    Language {
        name: "Hunt-tongue",
        creature: "stalker",
        about: "Hisses and shrieks: whether it hisses or shrieks and how far the inner jaw reaches are the vowels; the jaw's clack is the consonant.",
        syllable: 0.18,
        sustains: &[
            ph("ss", &[("mode", 0.0), ("gape", 0.4), ("reach", 0.2)], 1.0),
            ph("ka", &[("mode", 1.0), ("reach", 0.8), ("gape", 0.9)], 1.0),
            ph("rr", &[("mode", 0.6), ("rasp", 0.8), ("gape", 0.6)], 1.0),
            ph("ee", &[("mode", 1.0), ("reach", 0.2), ("gape", 0.5), ("rasp", 0.2)], 0.9),
        ],
        accents: &[ph("x", &[("!snap", 1.0), ("mode", 0.0)], 0.5)],
        onset: 0.45,
        span: 0.22,
        joint: 0.0,
        hold: &[("pressure", 20.0)],
    },
];

/// Load a planned utterance into a voice's spoken call (no allocation) and play it. False when it
/// didn't fit (the rest is cut).
pub fn load_and_play(voice: &mut super::CreatureVoice, s: &Spoken, amp: f64) -> bool {
    voice.spoken_clear();
    let mut fit = true;
    for k in &s.keys {
        if !voice.spoken_push_key(k.t, k.curve) {
            fit = false;
            break;
        }
        for &(n, x) in &k.v {
            voice.spoken_push_value(n, x);
        }
    }
    voice.play_spoken(amp);
    fit
}

// -------------------------------------------------------------------- the browser's flat form
// An utterance crosses to the audio thread as f64s: per key `t, curve, n`, then `n` pairs
// `target, value`. Curve: 0 none, 1 linear, 2 exponential, 3 step. Target: a param's index (≥ 0),
// −1 the gate, −2 − e event `e`.

fn curve_code(c: Option<Curve>) -> f64 {
    match c {
        None => 0.0,
        Some(Curve::Lin) => 1.0,
        Some(Curve::Exp) => 2.0,
        Some(Curve::Step) => 3.0,
    }
}

/// The flat form of an utterance's keys.
pub fn encode(s: &Spoken) -> Vec<f64> {
    let mut out = Vec::new();
    for k in &s.keys {
        let v: Vec<(f64, f64)> = k
            .v
            .iter()
            .filter_map(|&(n, x)| match n {
                CallTarget::Param(i) => Some((i as f64, x)),
                CallTarget::Gate => Some((-1.0, x)),
                CallTarget::Event(Some(e)) => Some((-2.0 - e as f64, x)),
                _ => None,
            })
            .collect();
        out.extend_from_slice(&[k.t, curve_code(k.curve), v.len() as f64]);
        for (n, x) in v {
            out.extend_from_slice(&[n, x]);
        }
    }
    out
}

/// Load a flat utterance into a voice's spoken call and play it (no allocation; malformed input
/// stops the load where it goes wrong). False when it was cut short.
pub fn play_flat(voice: &mut super::CreatureVoice, flat: &[f64], amp: f64) -> bool {
    voice.spoken_clear();
    let mut i = 0;
    let mut whole = true;
    while i + 3 <= flat.len() {
        let (t, c, n) = (flat[i], flat[i + 1], flat[i + 2]);
        if !(n >= 0.0) || i + 3 + 2 * n as usize > flat.len() {
            whole = false;
            break;
        }
        let curve = match c as i64 {
            1 => Some(Curve::Lin),
            2 => Some(Curve::Exp),
            3 => Some(Curve::Step),
            _ => None,
        };
        if !voice.spoken_push_key(t, curve) {
            whole = false;
            break;
        }
        for j in 0..n as usize {
            let (code, x) = (flat[i + 3 + 2 * j], flat[i + 4 + 2 * j]);
            let target = if code >= 0.0 {
                CallTarget::Param(code as usize)
            } else if code == -1.0 {
                CallTarget::Gate
            } else {
                CallTarget::Event(Some((-code - 2.0) as usize))
            };
            voice.spoken_push_value(target, x);
        }
        i += 3 + 2 * n as usize;
    }
    voice.play_spoken(amp);
    whole && i == flat.len()
}
