//! Chorister (invented): two larynxes, each at the top of its own short throat, open into one
//! shared pharynx. The two sets of folds sing their own pitches into the same tube and fight over
//! it: near-unison beats, open intervals growl, and chaos tips the duet into rough noise.

use super::build::{CMH2O, TubeArea, TubeOpts, fill_areas, lungs, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, GateTimes, ParamDef};
use crate::airway::network::{AirwayNetwork, NoiseSpec, PortSpec, Tube};
use crate::math::{js_min, pow, sqrt};
use crate::physics::{WALL_SOFT, WALL_TRACHEA};
use crate::sources::glottis::ParametricGlottis;

const PITCH: usize = 0;
const INTERVAL: usize = 1;
const DETUNE: usize = 2;
const BALANCE: usize = 3;
const CHAOS: usize = 4;
const PRESSURE: usize = 5;
const MOUTH: usize = 6;

const THROAT_AREA: f64 = 1.6;

pub struct Chorister {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    throat_l: Tube,
    throat_r: Tube,
    pharynx: Tube,
    mouth: Tube,
    lungs: usize,
    port_l: usize,
    port_r: usize,
    folds_l: ParametricGlottis,
    folds_r: ParametricGlottis,
    mouth_area: Vec<f64>,
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let trachea = tube(&mut net, "trachea", fs, 22.0, TubeArea::Uniform(3.0), TubeOpts { wall: Some(WALL_TRACHEA), coarse: Some(1.6), ..Default::default() });
    let throat_l = tube(&mut net, "throatL", fs, 6.0, TubeArea::Uniform(THROAT_AREA), TubeOpts { wall: Some(WALL_TRACHEA), ..Default::default() });
    let throat_r = tube(&mut net, "throatR", fs, 6.0, TubeArea::Uniform(THROAT_AREA), TubeOpts { wall: Some(WALL_TRACHEA), ..Default::default() });
    let pharynx = tube(&mut net, "pharynx", fs, 13.0, TubeArea::Uniform(4.5), TubeOpts { wall: Some(WALL_SOFT), wall_radiation: Some(0.25), perimeter_factor: Some(1.3), ..Default::default() });
    let mouth = tube(&mut net, "mouth", fs, 9.0, TubeArea::Uniform(3.5), TubeOpts { wall: Some(WALL_SOFT), wall_radiation: Some(0.25), perimeter_factor: Some(1.5), ..Default::default() });
    let lung_node = lungs(&mut net, trachea.first_node(), 3.0);
    // the trachea forks into two throats, each topped by its own set of folds
    net.connect(trachea.last_node(), throat_l.first_node());
    net.connect(trachea.last_node(), throat_r.first_node());
    let slit = sqrt(THROAT_AREA) * 0.9;
    let port_l = net.add_port(throat_l.last_node(), pharynx.first_node(), PortSpec::new(0.0, 0.4).kinetic(0.37, 1.0).slit(slit));
    let port_r = net.add_port(throat_r.last_node(), pharynx.first_node(), PortSpec::new(0.0, 0.4).kinetic(0.37, 1.0).slit(slit));
    net.connect(pharynx.last_node(), mouth.first_node());
    net.add_radiation(mouth.last_node());
    net.finalize();
    let folds_l = ParametricGlottis::new(fs);
    let folds_r = ParametricGlottis::new(fs);
    let tubes = vec![("trachea", trachea), ("throatL", throat_l), ("throatR", throat_r), ("pharynx", pharynx), ("mouth", mouth)];
    Box::new(Chorister { net, tubes, throat_l, throat_r, pharynx, mouth, lungs: lung_node, port_l, port_r, folds_l, folds_r, mouth_area: vec![0.0; mouth.n] })
}

impl CreatureSim for Chorister {
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
        1.5e-7
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        self.net.set_fixed_pressure(self.lungs, p[PRESSURE] * CMH2O * gate);
        let chaos = p[CHAOS];
        // chaos also pushes the two voices apart
        let spread = p[DETUNE] + 60.0 * chaos * chaos;
        let f_l = p[PITCH] * pow(2.0, -spread / 2400.0);
        let f_r = p[PITCH] * pow(2.0, p[INTERVAL] / 12.0 + spread / 2400.0);
        let bal = p[BALANCE];
        let amp_l = js_min(1.0, 2.0 * (1.0 - bal));
        let amp_r = js_min(1.0, 2.0 * bal);
        for (g, f, amp, drift) in [(&mut self.folds_l.params, f_l, amp_l, 5.0), (&mut self.folds_r.params, f_r, amp_r, 7.0)] {
            g.f0 = f;
            g.amplitude = 0.2 * amp;
            g.open_quotient = 0.6 - 0.1 * chaos;
            g.closure_sharpness = 0.6 + 0.3 * chaos;
            g.effort = 0.65 + 0.3 * chaos;
            g.jitter = 0.004 + 0.05 * chaos;
            g.shimmer = 0.03 + 0.25 * chaos;
            g.drift = drift + 40.0 * chaos;
            g.chaos = chaos;
            g.diplophonia = 0.5 * chaos;
        }
        let m = p[MOUTH];
        let areas = fill_areas(self.mouth, |u| if u > 0.7 { 0.3 + 4.5 * m } else { 3.5 }, &mut self.mouth_area);
        self.net.set_tube_targets(self.mouth, areas, None, None);
        // breath roughness at both larynxes rises with chaos
        let gain = 4e-7 + 3e-6 * chaos;
        let ph = self.pharynx.branch(1);
        let list = [
            NoiseSpec::new(self.port_l, ph, self.throat_l.branch(self.throat_l.n - 2), gain).band(4000.0, 250.0),
            NoiseSpec::new(self.port_r, ph, self.throat_r.branch(self.throat_r.n - 2), gain).band(4000.0, 250.0),
        ];
        self.net.set_noise_sources(&list);
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let ph = self.net.p[self.pharynx.first_node()];
        let dp = self.net.p[self.throat_l.last_node()] - ph;
        let a = self.folds_l.next(dp);
        self.net.set_port_area(self.port_l, a);
        let dp = self.net.p[self.throat_r.last_node()] - ph;
        let a = self.folds_r.next(dp);
        self.net.set_port_area(self.port_r, a);
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("left", js_min(1.0, self.folds_l.area / 0.25)));
        out.push(("right", js_min(1.0, self.folds_r.area / 0.25)));
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "chorister",
        name: "Chorister",
        latin: "Bilaryngia discors (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "Two sets of vocal folds sing into one shared throat and fight over it: beating unisons, growling intervals, and chaos that tips song into roar.",
        role: "Beating pads, two-note leads, growls",
        oversample: 1,
        gate_times: Some(GateTimes { attack: 0.04, release: 0.15 }),
        params: vec![
            ParamDef::new("pitch", "Pitch", 70.0, 700.0, 196.0).unit("Hz").log().smooth(0.03).pitch(),
            ParamDef::new("interval", "Interval", -12.0, 12.0, 0.0).unit("st").smooth(0.06).digits(1),
            ParamDef::new("detune", "Detune", 0.0, 100.0, 16.0).unit("cents").smooth(0.06).digits(0),
            ParamDef::new("balance", "Left ↔ right folds", 0.0, 1.0, 0.5).smooth(0.05),
            ParamDef::new("chaos", "Chaos", 0.0, 1.0, 0.1).smooth(0.05),
            ParamDef::new("pressure", "Breath pressure", 4.0, 30.0, 12.0).unit("cmH₂O").smooth(0.02),
            ParamDef::new("mouth", "Mouth opening", 0.0, 1.0, 0.7).smooth(0.04),
        ],
        calls: vec![
            CallDef::new(
                "hymn",
                "Hymn",
                "A beating unison that opens into a fifth",
                Some(196.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 196.0), ("interval", 0.0), ("detune", 14.0), ("chaos", 0.05), ("balance", 0.5)]),
                    k(0.15, &[("gate", 1.0)]),
                    k(1.2, &[("interval", 0.0)]),
                    k(1.8, &[("interval", 7.0), ("detune", 4.0)]),
                    k(3.2, &[("gate", 1.0), ("interval", 7.0)]),
                    k(3.6, &[("gate", 0.0), ("interval", 0.0), ("detune", 16.0), ("chaos", 0.1)]),
                ],
            ),
            CallDef::new(
                "quarrel",
                "Quarrel",
                "The two voices pull apart, trade the throat and break into a growl",
                Some(196.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 180.0), ("interval", 1.0), ("detune", 10.0), ("chaos", 0.05), ("balance", 0.5), ("pressure", 14.0)]),
                    k(0.1, &[("gate", 1.0)]),
                    k(0.7, &[("balance", 0.15)]),
                    k(1.3, &[("balance", 0.85), ("interval", -2.0)]),
                    k(1.9, &[("balance", 0.5), ("chaos", 0.5), ("detune", 40.0), ("pressure", 20.0)]),
                    k(2.8, &[("gate", 1.0), ("chaos", 0.9), ("pitch", 150.0), ("interval", 0.5)]).curve(Curve::Exp),
                    k(3.1, &[("gate", 0.0), ("chaos", 0.1), ("detune", 16.0), ("interval", 0.0), ("pressure", 12.0)]),
                ],
            ),
            CallDef::new(
                "howl",
                "Howl",
                "Both voices climb an octave together, roughening as they go",
                Some(196.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 150.0), ("interval", 3.0), ("detune", 8.0), ("chaos", 0.0), ("mouth", 0.4), ("pressure", 9.0)]),
                    k(0.2, &[("gate", 1.0)]),
                    k(2.0, &[("pitch", 300.0), ("chaos", 0.6), ("mouth", 1.0)]).curve(Curve::Exp),
                    k(2.6, &[("gate", 1.0), ("pitch", 260.0)]).curve(Curve::Exp),
                    k(2.9, &[("gate", 0.0), ("chaos", 0.1), ("interval", 0.0), ("detune", 16.0), ("mouth", 0.7), ("pressure", 12.0)]),
                ],
            ),
        ],
        events: vec![],
        create,
    }
    .resolved()
}
