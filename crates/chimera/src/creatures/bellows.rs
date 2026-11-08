//! Bellows (invented): no lungs. A muscular air sac squeezes in slow pulses and pushes air through a
//! reed-like fold at its neck into a long, wide, flaring horn. The sac never lets go between
//! squeezes, so the tone swells and sags but never breaks into syllables.

use super::build::{CMH2O, TubeArea, TubeOpts, fill_areas, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, GateTimes, ParamDef};
use crate::airway::network::{AirwayNetwork, PortSpec, Tube};
use crate::math::{TAU, cos, js_min, sqrt};
use crate::physics::{C_SOUND, RHO, WALL_SOFT};
use crate::sources::glottis::ParametricGlottis;

const PITCH: usize = 0;
const PERIOD: usize = 1;
const DEPTH: usize = 2;
const PRESSURE: usize = 3;
const BRIGHT: usize = 4;
const FLARE: usize = 5;

const NECK_AREA: f64 = 8.0;

pub struct Bellows {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    neck: Tube,
    horn: Tube,
    sac: usize,
    reed_port: usize,
    reed: ParametricGlottis,
    horn_area: Vec<f64>,
    phase: f64,
    squeeze: f64,
}

fn horn_profile(u: f64, flare: f64) -> f64 {
    6.0 + 10.0 * u + (8.0 + 70.0 * flare) * u * u * u * u
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let neck = tube(&mut net, "neck", fs, 14.0, TubeArea::Uniform(NECK_AREA), TubeOpts { wall: Some(WALL_SOFT), ..Default::default() });
    let horn = tube(&mut net, "horn", fs, 170.0, TubeArea::Profile(&|u| horn_profile(u, 0.6)), TubeOpts { wall: Some(WALL_SOFT), coarse: Some(1.6), max: Some(110.0), wall_radiation: Some(0.3), perimeter_factor: Some(1.2), ..Default::default() });
    // the sac: a prescribed-pressure node feeding the neck through a mildly resistive opening
    let sac = net.add_pressure_node("sac");
    net.add_port(sac, neck.first_node(), PortSpec::new(NECK_AREA, 1.0).kinetic(0.0, 0.0).resistance((0.5 * RHO * C_SOUND) / NECK_AREA));
    let reed_port = net.add_port(neck.last_node(), horn.first_node(), PortSpec::new(0.0, 0.5).kinetic(0.37, 1.0).slit(sqrt(NECK_AREA) * 0.9));
    net.add_radiation(horn.last_node());
    net.finalize();
    let mut reed = ParametricGlottis::new(fs);
    reed.params.jitter = 0.002;
    reed.params.shimmer = 0.015;
    reed.params.drift = 4.0;
    reed.params.skew = 0.7;
    reed.threshold = 2.0 * CMH2O;
    let tubes = vec![("neck", neck), ("horn", horn)];
    Box::new(Bellows { net, tubes, neck, horn, sac, reed_port, reed, horn_area: vec![0.0; horn.n], phase: 0.0, squeeze: 0.0 })
}

impl CreatureSim for Bellows {
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
        3.6e-8
    }
    fn control(&mut self, p: &[f64], gate: f64, dt: f64) {
        // slow squeeze: a raised cosine per period; the sac never fully relaxes
        self.phase += dt / p[PERIOD];
        self.phase -= self.phase.floor();
        let swell = 0.5 - 0.5 * cos(TAU * self.phase);
        let depth = p[DEPTH];
        self.squeeze = 1.0 - depth + depth * swell;
        let sq = self.squeeze;
        self.net.set_fixed_pressure(self.sac, p[PRESSURE] * CMH2O * gate * (0.3 + 0.7 * sq));
        let b = p[BRIGHT];
        let r = &mut self.reed.params;
        r.f0 = p[PITCH];
        r.amplitude = 1.1 * (0.55 + 0.45 * sq);
        // the reed slams shut harder as the squeeze peaks: the swell brightens
        r.open_quotient = 0.7 - 0.3 * b - 0.1 * sq;
        r.closure_sharpness = 0.3 + 0.6 * b * (0.5 + 0.5 * sq);
        r.effort = 0.5 + 0.4 * sq;
        let flare = p[FLARE];
        let areas = fill_areas(self.horn, |u| horn_profile(u, flare), &mut self.horn_area);
        self.net.set_tube_targets(self.horn, areas, None, None);
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let dp = self.net.p[self.neck.last_node()] - self.net.p[self.horn.first_node()];
        let area = self.reed.next(dp);
        self.net.set_port_area(self.reed_port, area);
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("reed", js_min(1.0, self.reed.area / 1.4)));
        out.push(("sac", self.squeeze));
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "bellows",
        name: "Bellows",
        latin: "Follis perpetuus (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "No lungs: a muscular air sac squeezes in slow pulses, forcing air through a reed at its neck into a long, wide horn.",
        role: "Swelling drones, pads, slow pulsing bass",
        oversample: 1,
        gate_times: Some(GateTimes { attack: 0.15, release: 0.5 }),
        params: vec![
            ParamDef::new("pitch", "Reed pitch", 35.0, 240.0, 65.0).unit("Hz").log().smooth(0.08).pitch(),
            ParamDef::new("period", "Squeeze period", 0.5, 3.0, 1.8).unit("s").smooth(0.2).digits(2),
            ParamDef::new("depth", "Squeeze depth", 0.0, 1.0, 0.65).smooth(0.1),
            ParamDef::new("pressure", "Sac pressure", 4.0, 30.0, 14.0).unit("cmH₂O").smooth(0.05),
            ParamDef::new("bright", "Brightness", 0.0, 1.0, 0.5).smooth(0.05),
            ParamDef::new("flare", "Horn flare", 0.0, 1.0, 0.6).smooth(0.1),
        ],
        calls: vec![
            CallDef::new(
                "drone",
                "Drone",
                "Seven seconds of steady, swelling drone",
                Some(65.0),
                vec![k(0.0, &[("gate", 0.0), ("pitch", 65.0), ("period", 1.8), ("depth", 0.7)]), k(0.4, &[("gate", 1.0)]), k(6.6, &[("gate", 1.0)]), k(7.2, &[("gate", 0.0)])],
            ),
            CallDef::new(
                "swell",
                "Great swell",
                "One slow squeeze that blooms bright and sags in pitch",
                Some(65.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 62.0), ("period", 3.0), ("depth", 0.2), ("pressure", 8.0), ("bright", 0.2)]),
                    k(0.3, &[("gate", 1.0)]),
                    k(2.0, &[("pitch", 70.0), ("pressure", 26.0), ("bright", 0.9)]).curve(Curve::Exp),
                    k(4.0, &[("gate", 1.0), ("pitch", 58.0), ("pressure", 9.0), ("bright", 0.3)]).curve(Curve::Exp),
                    k(4.6, &[("gate", 0.0), ("pressure", 14.0), ("bright", 0.5)]),
                ],
            ),
            CallDef::new(
                "throb",
                "Throb",
                "Fast, deep squeezes on a falling note",
                Some(65.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 90.0), ("period", 0.5), ("depth", 1.0), ("pressure", 20.0)]),
                    k(0.15, &[("gate", 1.0)]),
                    k(3.0, &[("gate", 1.0), ("pitch", 55.0)]).curve(Curve::Exp),
                    k(3.4, &[("gate", 0.0), ("period", 1.8), ("depth", 0.65), ("pressure", 14.0)]),
                ],
            ),
        ],
        events: vec![],
        create,
    }
    .resolved()
}
