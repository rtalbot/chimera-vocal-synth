//! Glass Singer (invented): silicon life with a crystal body. A sharp click at the base of three
//! rigid, almost sealed spires rings them like struck glass. Each spire is closed at the base and
//! nearly closed at the tip (a pinhole that radiates), so it rings at c/2L and its harmonics; the
//! spires' lengths stand in the ratios of a free bar's modes (1 : 2.76 : 5.40), so together they
//! sound inharmonic, bell- and glass-like. Ring time is the walls' (tiny) viscous loss and the
//! pinhole size; brightness is the click's sharpness and how hard the short spires are struck.

use super::build::{TubeArea, TubeOpts, dx_min, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, ParamDef};
use crate::airway::network::{AirwayNetwork, Tube};
use crate::math::{Rng, exp, js_max, js_min, pow};
use crate::physics::C_SOUND;
use crate::sources::valves::ClickTrain;

const PITCH: usize = 0;
const RING: usize = 1;
const BRIGHT: usize = 2;
const STRIKE: usize = 3;
const ROLL: usize = 4;

/// spire length ratios (free-bar mode ratios: the short spires ring 2.76× and 5.40× higher)
const RATIOS: [f64; 3] = [1.0, 2.756, 5.404];
const PITCH_MIN: f64 = 220.0;
const PITCH_MAX: f64 = 1760.0;
/// spire cross-section (cm²)
const AREA: f64 = 2.0;
/// volume velocity of a unit click (cm³/s)
const FLOW: f64 = 900.0;
/// measured: the pinhole and the coarse grid ring the first partial ~7% sharp of c/2L
const TUNE: f64 = 0.93;

pub struct GlassSinger {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    spires: [Tube; 3],
    clicks: ClickTrain,
    rng: Rng,
    weight: [f64; 3],
    areas: [Vec<f64>; 3],
    strike: f64,
    prev_gate: f64,
    roll_phase: f64,
    flash: f64,
    flash_decay: f64,
}

fn spire_length(pitch: f64, i: usize) -> f64 {
    C_SOUND / (2.0 * pitch * TUNE * RATIOS[i])
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let names = ["spire1", "spire2", "spire3"];
    let spires: [Tube; 3] = std::array::from_fn(|i| {
        // as many sections as the shortest length (top pitch) allows; longer lengths are coarser
        let shortest = spire_length(PITCH_MAX, i);
        let n = js_max(3.0, (shortest / dx_min(fs)).floor());
        tube(&mut net, names[i], fs, spire_length(PITCH_MIN, i), TubeArea::Uniform(AREA), TubeOpts { wall: None, min: Some(n), max: Some(n), viscous_factor: Some(0.2), ..Default::default() })
    });
    for s in &spires {
        net.add_radiation(s.last_node());
    }
    net.finalize();
    let tubes = names.iter().zip(spires).map(|(n, t)| (*n, t)).collect();
    let areas = std::array::from_fn(|i| vec![AREA; spires[i].n]);
    let mut clicks = ClickTrain::new(fs);
    clicks.width = 0.0003;
    Box::new(GlassSinger { net, tubes, spires, clicks, rng: Rng::new(0x5111_c0de), weight: [1.0, 0.6, 0.4], areas, strike: 0.8, prev_gate: 0.0, roll_phase: 0.0, flash: 0.0, flash_decay: exp(-1.0 / (0.08 * fs)) })
}

impl GlassSinger {
    fn strike_now(&mut self, delay: f64, amp: f64) {
        self.clicks.click(delay, amp);
    }
}

impl CreatureSim for GlassSinger {
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
        6.5e-7
    }
    fn control(&mut self, p: &[f64], gate: f64, dt: f64) {
        let pitch = js_max(PITCH_MIN * 0.5, p[PITCH]);
        let ring = p[RING];
        let bright = p[BRIGHT];
        self.strike = p[STRIKE];
        // ring: the walls' viscous loss and the pinhole at each tip
        self.net.viscous_scale = 0.02 + 1.5 * (1.0 - ring) * (1.0 - ring);
        let tip = 0.004 + 0.04 * (1.0 - ring);
        for i in 0..3 {
            let t = self.spires[i];
            let a = &mut self.areas[i];
            a.fill(AREA);
            a[t.n - 1] = tip;
            self.net.set_tube_targets(t, a, Some(spire_length(pitch, i)), None);
        }
        self.clicks.width = 0.0009 * (1.0 - bright) + 0.00005 * bright;
        // long spires and small pinholes radiate less: strike them harder to keep the level even
        // and a sharper click carries less volume: partly make that up
        let level = pow(660.0 / pitch, 0.9) * pow(0.014 / tip, 0.7) * pow(0.000475 / self.clicks.width, 0.4);
        self.weight = [level, level * (0.25 + 0.6 * bright), level * (0.1 + 0.7 * bright)];
        // a struck note: one strike as the gate opens, then a roll of softer strikes while held
        if gate > 0.05 && self.prev_gate <= 0.05 {
            self.strike_now(0.0, self.strike);
            self.roll_phase = 0.0;
        } else if gate > 0.05 && p[ROLL] > 0.05 {
            let rate = p[ROLL];
            let next = self.roll_phase + rate * dt;
            if next >= 1.0 {
                let delay = js_max(0.0, (1.0 - self.roll_phase) / rate);
                let amp = self.strike * gate * (0.45 + 0.3 * self.rng.next());
                self.strike_now(js_min(dt, delay), amp);
                self.roll_phase = next - 1.0;
            } else {
                self.roll_phase = next;
            }
        }
        self.prev_gate = gate;
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let y = self.clicks.next();
        if y != 0.0 {
            for i in 0..3 {
                self.net.ext_flow[self.spires[i].first_node()] += y * FLOW * self.weight[i];
            }
            self.flash = js_max(self.flash, js_min(1.0, y.abs()));
        }
        self.flash *= self.flash_decay;
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("strike", self.flash));
    }
    fn trigger(&mut self, _event: usize, amount: f64) {
        self.strike_now(0.0, js_max(0.0, js_min(1.5, amount)));
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "glasssinger",
        name: "Glass Singer",
        latin: "Vitricantor silicis (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "Silicon life with a crystal body: sharp clicks strike three sealed, rigid spires that ring like glass.",
        role: "Chimes, bells, glassy plucks and shimmering rolls",
        oversample: 2,
        gate_times: Some(super::types::GateTimes { attack: 0.004, release: 0.06 }),
        params: vec![
            ParamDef::new("pitch", "Pitch", PITCH_MIN, PITCH_MAX, 660.0).unit("Hz").log().smooth(0.01).pitch(),
            ParamDef::new("ring", "Ring time", 0.0, 1.0, 0.75).smooth(0.05).digits(2),
            ParamDef::new("bright", "Brightness", 0.0, 1.0, 0.5).smooth(0.03).digits(2),
            ParamDef::new("strike", "Strike", 0.1, 1.0, 0.8).smooth(0.01).digits(2),
            ParamDef::new("roll", "Roll", 0.0, 20.0, 3.0).unit("Hz").smooth(0.05).digits(1),
        ],
        calls: vec![
            CallDef::new("chime", "Chime", "One hard strike, left to ring", Some(660.0), vec![k(0.0, &[("pitch", 660.0), ("ring", 0.95), ("bright", 0.7), ("!strike", 1.0)]), k(2.5, &[("ring", 0.95)])]),
            CallDef::new(
                "peal",
                "Peal",
                "A falling phrase of four strikes",
                Some(880.0),
                vec![
                    k(0.0, &[("pitch", 1320.0), ("ring", 0.8), ("bright", 0.6), ("!strike", 0.9)]),
                    k(0.3, &[("pitch", 1100.0), ("!strike", 0.8)]).curve(Curve::Step),
                    k(0.6, &[("pitch", 990.0), ("!strike", 0.85)]).curve(Curve::Step),
                    k(0.9, &[("pitch", 660.0), ("!strike", 1.0)]).curve(Curve::Step),
                    k(2.2, &[("ring", 0.8)]),
                ],
            ),
            CallDef::new(
                "shiver",
                "Shiver",
                "A fast roll that swells, brightens and fades",
                Some(660.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("roll", 16.0), ("bright", 0.2), ("ring", 0.4), ("pitch", 700.0)]),
                    k(0.05, &[("gate", 0.5)]),
                    k(0.8, &[("gate", 1.0), ("bright", 0.9), ("pitch", 740.0)]).curve(Curve::Exp),
                    k(1.6, &[("gate", 0.2), ("bright", 0.3), ("pitch", 660.0)]).curve(Curve::Exp),
                    k(1.7, &[("gate", 0.0)]),
                ],
            ),
        ],
        events: vec!["strike"],
        create,
    }
    .resolved()
}
