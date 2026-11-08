//! The Resonant (invented): one breath, three voices. Lungs fill an air sac; three sets of vocal
//! folds in its far wall each open into a horn of a different length, so a single breath sounds a
//! chord. Each horn's resonances colour its own voice, and the shared sac couples them: a loud
//! voice steals breath from the others.
//!
//! (A first version used buzzing lip valves, as the elephant's trumpet does, to lock each voice
//! onto its horn like a bugle. Held steady, that valve dies out within a tenth of a second here,
//! the elephant's included, so it can't carry a chord.)

use super::build::{CMH2O, TubeArea, TubeOpts, lungs, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, ParamDef};
use crate::airway::network::{AirwayNetwork, PortSpec, Tube};
use crate::math::{js_max, js_min, pow, sqrt};
use crate::physics::{WALL_SOFT, WALL_TRACHEA};
use crate::sources::glottis::ParametricGlottis;

const PITCH: usize = 0;
const THIRD: usize = 1;
const FIFTH: usize = 2;
const VOICES: usize = 3;
const PRESSURE: usize = 4;
const TENSION: usize = 5;

/// (name, length cm, mouth area cm²) of the three horns, longest first
const HORNS: [(&str, f64, f64); 3] = [("hornLow", 58.0, 12.0), ("hornMid", 46.0, 9.0), ("hornHigh", 36.0, 7.0)];
/// bore area at the folds (cm²)
const BORE: f64 = 2.2;

pub struct Resonant {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    lungs: usize,
    sac: Tube,
    horns: [Tube; 3],
    ports: [usize; 3],
    folds: [ParametricGlottis; 3],
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let trachea = tube(&mut net, "trachea", fs, 30.0, TubeArea::Uniform(6.0), TubeOpts { wall: Some(WALL_TRACHEA), coarse: Some(1.6), max: Some(40.0), ..Default::default() });
    let sac = tube(&mut net, "sac", fs, 22.0, TubeArea::Profile(&|u| 6.0 + 40.0 * (1.0 - (2.0 * u - 1.0) * (2.0 * u - 1.0))), TubeOpts { wall: Some(WALL_SOFT), coarse: Some(1.2), perimeter_factor: Some(1.3), wall_radiation: Some(0.3), ..Default::default() });
    let lung_node = lungs(&mut net, trachea.first_node(), 6.0);
    net.connect(trachea.last_node(), sac.first_node());
    let mut horns = [sac; 3];
    let mut ports = [0usize; 3];
    for (i, &(name, len, mouth)) in HORNS.iter().enumerate() {
        // a narrow bore that flares only near the mouth
        let profile = move |u: f64| BORE + (mouth - BORE) * pow(u, 4.0);
        let h = tube(&mut net, name, fs, len, TubeArea::Profile(&profile), TubeOpts { wall: Some(WALL_SOFT), perimeter_factor: Some(1.3), wall_radiation: Some(0.2), ..Default::default() });
        ports[i] = net.add_port(sac.last_node(), h.first_node(), PortSpec::new(0.0, 0.4).kinetic(0.37, 1.0).slit(sqrt(BORE) * 0.9));
        net.add_radiation(h.last_node());
        horns[i] = h;
    }
    net.finalize();
    let tubes = vec![("trachea", trachea), ("sac", sac), (HORNS[0].0, horns[0]), (HORNS[1].0, horns[1]), (HORNS[2].0, horns[2])];
    let folds = [ParametricGlottis::new(fs), ParametricGlottis::new(fs), ParametricGlottis::new(fs)];
    Box::new(Resonant { net, tubes, lungs: lung_node, sac, horns, ports, folds })
}

impl CreatureSim for Resonant {
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
        GAIN
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        self.net.set_fixed_pressure(self.lungs, p[PRESSURE] * CMH2O * gate);
        let root = p[PITCH];
        let pitches = [root, root * pow(2.0, p[THIRD] / 12.0), root * pow(2.0, p[FIFTH] / 12.0)];
        // voices 0: the root alone; 0.5: root and third; 1: the full chord
        let v = p[VOICES];
        let drive = [1.0, js_min(1.0, js_max(0.0, 2.0 * v)), js_min(1.0, js_max(0.0, 2.0 * v - 1.0))];
        let t = p[TENSION];
        for i in 0..3 {
            let g = &mut self.folds[i].params;
            g.f0 = pitches[i];
            g.amplitude = 0.35 * drive[i];
            // a silent voice holds its folds shut rather than open, so it doesn't leak breath
            g.abduction = 0.0;
            g.open_quotient = 0.7 - 0.25 * t;
            g.closure_sharpness = 0.4 + 0.5 * t;
            g.effort = 0.5 + 0.45 * t;
            // the voices drift apart slightly, so the chord beats like a live ensemble
            g.drift = 4.0 + 3.0 * i as f64;
        }
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let up = self.net.p[self.sac.last_node()];
        for i in 0..3 {
            let dp = up - self.net.p[self.horns[i].first_node()];
            let area = self.folds[i].next(dp);
            self.net.set_port_area(self.ports[i], area);
        }
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("low", js_min(1.0, self.folds[0].area / 0.35)));
        out.push(("mid", js_min(1.0, self.folds[1].area / 0.35)));
        out.push(("high", js_min(1.0, self.folds[2].area / 0.35)));
    }
}

const GAIN: f64 = 2.5e-8;

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "resonant",
        name: "Resonant",
        latin: "Tricornis harmonicus (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "One breath, three voices: three sets of vocal folds, each at the base of its own horn, sound a chord from a single air sac.",
        role: "Horn-like chords and swells from a single body",
        oversample: 1,
        gate_times: None,
        params: vec![
            ParamDef::new("pitch", "Root pitch", 90.0, 600.0, 220.0).unit("Hz").log().smooth(0.03).pitch(),
            ParamDef::new("third", "Second voice", 0.0, 12.0, 4.0).unit("st").smooth(0.06).digits(1),
            ParamDef::new("fifth", "Third voice", 0.0, 19.0, 7.0).unit("st").smooth(0.06).digits(1),
            ParamDef::new("voices", "Voices", 0.0, 1.0, 1.0).smooth(0.08),
            ParamDef::new("pressure", "Breath pressure", 4.0, 45.0, 22.0).unit("cmH₂O").smooth(0.02),
            ParamDef::new("tension", "Fold tension", 0.0, 1.0, 0.5).smooth(0.05),
        ],
        calls: vec![
            CallDef::new(
                "swell",
                "Swell",
                "A major chord that blooms from one voice to three",
                Some(220.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 220.0), ("third", 4.0), ("fifth", 7.0), ("voices", 0.0), ("pressure", 14.0)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(1.6, &[("voices", 1.0), ("pressure", 28.0)]),
                    k(2.6, &[("pressure", 24.0)]),
                    k(3.0, &[("gate", 0.0), ("pressure", 22.0)]),
                ],
            ),
            CallDef::new(
                "lament",
                "Lament",
                "A minor chord whose root sinks while the upper voices hold",
                Some(220.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 247.0), ("third", 3.0), ("fifth", 7.0), ("voices", 1.0), ("pressure", 20.0)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(0.8, &[("pitch", 247.0), ("third", 3.0), ("fifth", 7.0)]),
                    k(2.2, &[("pitch", 196.0), ("third", 6.0), ("fifth", 11.0)]).curve(Curve::Exp),
                    k(2.6, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "greeting",
                "Greeting",
                "Three rising blasts, each a wider chord than the last",
                Some(220.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 196.0), ("third", 0.0), ("fifth", 0.0), ("voices", 1.0), ("pressure", 26.0)]),
                    k(0.03, &[("gate", 1.0)]),
                    k(0.35, &[("gate", 0.0)]),
                    k(0.45, &[("pitch", 220.0), ("third", 5.0), ("fifth", 5.0)]).curve(Curve::Step),
                    k(0.48, &[("gate", 1.0)]),
                    k(0.8, &[("gate", 0.0)]),
                    k(0.9, &[("pitch", 262.0), ("third", 7.0), ("fifth", 12.0)]).curve(Curve::Step),
                    k(0.93, &[("gate", 1.0), ("pressure", 32.0)]),
                    k(1.7, &[("pitch", 294.0)]).curve(Curve::Exp),
                    k(1.85, &[("gate", 0.0), ("pressure", 22.0)]),
                ],
            ),
        ],
        events: vec![],
        create,
    }
    .resolved()
}
