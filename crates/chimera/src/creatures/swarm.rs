//! Swarm (invented): a cloud of tiny clicking bodies that moves and speaks as one. Its "words" are
//! patterns of rhythm and density. Each click comes from a random member at a random moment (a
//! Poisson process from the engine's deterministic RNG) and rings one of three small horns, each a
//! quarter-wave tube closed at the clicking end. The swarm's pulse gathers the clicks into bursts:
//! at sync 0 they fall anywhere, at sync 1 only in a short window at the start of each beat.

use super::build::{TubeArea, TubeOpts, dx_min, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, GateTimes, ParamDef};
use crate::airway::network::{AirwayNetwork, Tube};
use crate::math::{Rng, exp, js_max, js_min, pow, sqrt};
use crate::physics::C_SOUND;
use crate::sources::valves::ClickTrain;

const PITCH: usize = 0;
const DENSITY: usize = 1;
const PULSE: usize = 2;
const SYNC: usize = 3;
const SCATTER: usize = 4;

const PITCH_MIN: f64 = 600.0;
const PITCH_MAX: f64 = 4000.0;
/// horn cross-section (cm²)
const AREA: f64 = 0.3;
/// volume velocity of a unit click (cm³/s)
const FLOW: f64 = 60.0;
/// largest and smallest horn tuning relative to the pitch (at full scatter)
const SPREAD_UP: f64 = 0.4;
const SPREAD_DOWN: f64 = 0.35;
/// clicks a burst event sends (at amount 1), spread over `BURST_S`
const BURST: f64 = 36.0;
const BURST_S: f64 = 0.06;

pub struct Swarm {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    horns: [Tube; 3],
    clicks: [ClickTrain; 3],
    rng: Rng,
    dt: f64,
    /// clicks per internal sample (expected), before the pulse window
    rate: f64,
    pulse: f64,
    phase: f64,
    window: f64,
    sync: f64,
    amp: f64,
    pitch_level: f64,
    prev_gate: f64,
    activity: f64,
    activity_decay: f64,
}

/// measured: the coarse horns ring ~9% sharp of a quarter wave
const TUNE: f64 = 1.09;

/// quarter-wave length (cm) for a horn tuned to `f`, less the open end's correction
fn horn_length(f: f64) -> f64 {
    js_max(0.5, C_SOUND / (4.0 * f / TUNE) - 0.6 * sqrt(AREA / crate::math::PI))
}

fn ratios(scatter: f64) -> [f64; 3] {
    [1.0, 1.0 + SPREAD_UP * scatter, 1.0 / (1.0 + SPREAD_DOWN * scatter)]
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let names = ["horn1", "horn2", "horn3"];
    let top = ratios(1.0);
    let horns: [Tube; 3] = std::array::from_fn(|i| {
        let shortest = horn_length(PITCH_MAX * js_max(1.0, top[i]));
        let n = js_min(64.0, js_max(3.0, (shortest / dx_min(fs)).floor()));
        tube(&mut net, names[i], fs, horn_length(PITCH_MIN), TubeArea::Uniform(AREA), TubeOpts { wall: None, min: Some(n), max: Some(n), ..Default::default() })
    });
    for h in &horns {
        net.add_radiation(h.last_node());
    }
    net.finalize();
    let tubes = names.iter().zip(horns).map(|(n, t)| (*n, t)).collect();
    let clicks = std::array::from_fn(|_| {
        let mut c = ClickTrain::new(fs);
        c.width = 0.00015;
        c
    });
    Box::new(Swarm { net, tubes, horns, clicks, rng: Rng::new(0x5a_a2a1), dt: 1.0 / fs, rate: 0.0, pulse: 6.0, phase: 0.0, window: 1.0, sync: 0.0, amp: 1.0, pitch_level: 1.0, prev_gate: 0.0, activity: 0.0, activity_decay: exp(-1.0 / (0.05 * fs)) })
}

impl Swarm {
    /// one click from a random member into a random horn, after `delay` seconds
    fn click(&mut self, delay: f64, amp: f64) {
        let h = js_min(2.0, (self.rng.next() * 3.0).floor()) as usize;
        let a = amp * (0.35 + 0.65 * self.rng.next()) * if self.rng.next() < 0.5 { 1.0 } else { -1.0 };
        self.clicks[h].click(delay, a);
    }

    fn burst(&mut self, amount: f64) {
        let n = js_min(2.0 * BURST, js_max(0.0, amount * BURST)).floor() as usize;
        for _ in 0..n {
            let d = self.rng.next() * BURST_S;
            self.click(d * d / BURST_S, 0.5 * self.pitch_level); // denser at the start
        }
    }
}

impl CreatureSim for Swarm {
    fn net(&self) -> Option<&AirwayNetwork> {
        Some(&self.net)
    }
    fn net_mut(&mut self) -> Option<&mut AirwayNetwork> {
        Some(&mut self.net)
    }
    fn tubes(&self) -> &[(&'static str, Tube)] {
        &self.tubes
    }
    fn gain(&self) -> f64 {
        1.9e-7
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        let r = ratios(p[SCATTER]);
        for (h, r) in self.horns.iter().zip(r) {
            self.net.set_tube_targets(*h, &[AREA; 64][..h.n], Some(horn_length(p[PITCH] * r)), None);
        }
        let density = p[DENSITY];
        self.rate = density * gate * self.dt;
        self.pulse = p[PULSE];
        self.sync = p[SYNC];
        self.window = 1.0 - 0.9 * p[SYNC];
        // a denser swarm is louder, but less than in proportion
        // a denser swarm is louder, but less than in proportion; low horns radiate less
        self.pitch_level = pow(1800.0 / p[PITCH], 0.8);
        // (density within the pulse's window, where the clicks crowd)
        self.amp = pow((200.0 * self.window) / density, 0.3) * self.pitch_level;
        if gate > 0.05 && self.prev_gate <= 0.05 {
            self.phase = 0.0;
            self.burst(0.4);
        }
        self.prev_gate = gate;
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        // the pulse: clicks gather into the first `window` of each beat
        self.phase += self.pulse * self.dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let env = (1.0 - self.sync) + self.sync * if self.phase < self.window { 1.0 / self.window } else { 0.0 };
        let pr = self.rate * env;
        if pr > 0.0 && self.rng.next() < pr {
            self.click(0.0, self.amp);
        }
        let mut any = 0.0;
        for i in 0..3 {
            let y = self.clicks[i].next();
            if y != 0.0 {
                self.net.ext_flow[self.horns[i].first_node()] += y * FLOW;
                any = js_max(any, y.abs());
            }
        }
        self.activity = js_max(self.activity * self.activity_decay, js_min(1.0, any));
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("swarm", self.activity));
    }
    fn trigger(&mut self, _event: usize, amount: f64) {
        self.burst(amount);
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "swarm",
        name: "Swarm",
        latin: "Multivox concordans (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "Thousands of tiny clicking bodies moving as one; it speaks in patterns of rhythm and density through three small horns.",
        role: "Rattles, shakers, granular textures and rhythmic chatter",
        oversample: 2,
        gate_times: Some(GateTimes { attack: 0.01, release: 0.08 }),
        params: vec![
            ParamDef::new("pitch", "Horn pitch", PITCH_MIN, PITCH_MAX, 1800.0).unit("Hz").log().smooth(0.02).pitch(),
            ParamDef::new("density", "Density", 10.0, 3000.0, 250.0).unit("/s").log().smooth(0.05),
            ParamDef::new("pulse", "Pulse", 0.5, 24.0, 6.0).unit("Hz").log().smooth(0.05).digits(1),
            ParamDef::new("sync", "Sync", 0.0, 1.0, 0.6).smooth(0.05).digits(2),
            ParamDef::new("scatter", "Scatter", 0.0, 1.0, 0.4).smooth(0.05).digits(2),
        ],
        calls: vec![
            CallDef::new(
                "chatter",
                "Chatter",
                "A tight rhythmic phrase that speeds up, then slows",
                Some(1800.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("density", 500.0), ("pulse", 7.0), ("sync", 0.85), ("pitch", 1800.0)]),
                    k(0.02, &[("gate", 1.0)]),
                    k(0.8, &[("pulse", 13.0), ("pitch", 2200.0)]).curve(Curve::Exp),
                    k(1.6, &[("pulse", 4.0), ("density", 300.0), ("pitch", 1600.0)]).curve(Curve::Exp),
                    k(1.9, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "surge",
                "Surge",
                "The cloud thickens from a few ticks to a roar and thins again",
                Some(1800.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("density", 20.0), ("sync", 0.1), ("pulse", 3.0), ("pitch", 1500.0), ("scatter", 0.6)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(1.1, &[("density", 2500.0), ("pitch", 2400.0), ("scatter", 0.9)]).curve(Curve::Exp),
                    k(2.2, &[("density", 20.0), ("pitch", 1400.0), ("scatter", 0.4)]).curve(Curve::Exp),
                    k(2.3, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "rally",
                "Rally",
                "Three sharp bursts, then the whole swarm in step",
                Some(1800.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 2000.0), ("density", 1200.0), ("pulse", 10.0), ("sync", 1.0), ("!burst", 1.0)]),
                    k(0.25, &[("!burst", 0.8)]),
                    k(0.5, &[("!burst", 1.2), ("pitch", 1800.0)]),
                    k(0.7, &[("gate", 0.0)]),
                    k(0.75, &[("gate", 1.0)]),
                    k(1.6, &[("gate", 1.0), ("pitch", 1500.0)]).curve(Curve::Exp),
                    k(1.7, &[("gate", 0.0)]),
                ],
            ),
        ],
        events: vec!["burst"],
        create,
    }
    .resolved()
}
