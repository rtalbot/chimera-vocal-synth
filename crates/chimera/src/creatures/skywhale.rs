//! Skywhale (invented): a floating gas-bag the size of a building. Gas squeezed from the bag drives
//! enormous vocal folds at the top of a four-metre windpipe; the sound travels through a vast
//! throat and a flaring horn as long as a house. Fundamentals sit in the infrasound (8–60 Hz) with
//! a stack of audible harmonics, like an elephant rumble scaled up; the groan adds fry and chaos.

use super::build::{CMH2O, TubeArea, TubeOpts, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, GateTimes, ParamDef};
use crate::airway::network::{AirwayNetwork, NoiseSpec, PortSpec, Tube};
use crate::math::{js_min, sqrt};
use crate::physics::{C_SOUND, RHO, WALL_SOFT, WALL_TRACHEA};
use crate::sources::glottis::ParametricGlottis;

const PITCH: usize = 0;
const PRESSURE: usize = 1;
const BRIGHT: usize = 2;
const ROUGH: usize = 3;
const MOUTH: usize = 4;

const PIPE_AREA: f64 = 80.0;

pub struct Skywhale {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    windpipe: Tube,
    throat: Tube,
    bag: usize,
    port: usize,
    folds: ParametricGlottis,
    lips: usize,
}

/// The horn keeps its shape; the mouth is a port at its end (re-shaping a tube this big would
/// pump tens of litres of air and thump).
fn horn_profile(u: f64) -> f64 {
    160.0 + 700.0 * u * u * u
}

fn lip_area(mouth: f64) -> f64 {
    60.0 + 900.0 * mouth
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let windpipe = tube(&mut net, "windpipe", fs, 400.0, TubeArea::Uniform(PIPE_AREA), TubeOpts { wall: Some(WALL_TRACHEA), coarse: Some(3.0), max: Some(80.0), ..Default::default() });
    let throat = tube(&mut net, "throat", fs, 260.0, TubeArea::Uniform(160.0), TubeOpts { wall: Some(WALL_SOFT), coarse: Some(3.0), max: Some(60.0), wall_radiation: Some(0.4), perimeter_factor: Some(1.3), ..Default::default() });
    let horn = tube(&mut net, "horn", fs, 600.0, TubeArea::Profile(&horn_profile), TubeOpts { wall: Some(WALL_SOFT), coarse: Some(3.0), max: Some(110.0), wall_radiation: Some(0.4), perimeter_factor: Some(1.2), ..Default::default() });
    let bag = net.add_pressure_node("gas bag");
    net.add_port(bag, windpipe.first_node(), PortSpec::new(PIPE_AREA, 1.0).kinetic(0.0, 0.0).resistance((0.5 * RHO * C_SOUND) / PIPE_AREA));
    let port = net.add_port(windpipe.last_node(), throat.first_node(), PortSpec::new(0.0, 1.2).kinetic(0.37, 1.0).slit(sqrt(PIPE_AREA) * 0.9));
    net.connect(throat.last_node(), horn.first_node());
    let lip_tube = tube(&mut net, "lip", fs, 12.0, TubeArea::Uniform(lip_area(1.0)), TubeOpts { wall: Some(WALL_SOFT), ..Default::default() });
    let lips = net.add_port(horn.last_node(), lip_tube.first_node(), PortSpec::new(lip_area(0.7), 6.0).kinetic(0.1, 0.5));
    net.add_radiation(lip_tube.last_node());
    net.finalize();
    let mut folds = ParametricGlottis::new(fs);
    folds.threshold = 1.2 * CMH2O;
    folds.params.jitter = 0.003;
    folds.params.shimmer = 0.05;
    folds.params.drift = 5.0;
    let tubes = vec![("windpipe", windpipe), ("throat", throat), ("horn", horn)];
    Box::new(Skywhale { net, tubes, windpipe, throat, bag, port, folds, lips })
}

impl CreatureSim for Skywhale {
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
        9.0e-8
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        self.net.set_fixed_pressure(self.bag, p[PRESSURE] * CMH2O * gate);
        let b = p[BRIGHT];
        let rough = p[ROUGH];
        let g = &mut self.folds.params;
        g.f0 = p[PITCH];
        g.amplitude = 12.0 * (1.0 - 0.45 * rough);
        // short, hard closures keep the harmonics of an infrasonic fundamental audible
        g.open_quotient = 0.55 - 0.3 * b;
        g.closure_sharpness = 0.4 + 0.6 * b;
        g.effort = 0.6 + 0.35 * b;
        g.chaos = 0.7 * rough;
        g.fry = 0.6 * rough;
        g.shimmer = 0.05 + 0.2 * rough;
        self.net.set_port_target(self.lips, lip_area(p[MOUTH]));
        let list = [NoiseSpec::new(self.port, self.throat.branch(1), self.windpipe.branch(self.windpipe.n - 2), 2e-7 + 1e-6 * rough).band(1500.0, 60.0)];
        self.net.set_noise_sources(&list);
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let dp = self.net.p[self.windpipe.last_node()] - self.net.p[self.throat.first_node()];
        let area = self.folds.next(dp);
        self.net.set_port_area(self.port, area);
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("folds", js_min(1.0, self.folds.area / 14.0)));
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "skywhale",
        name: "Skywhale",
        latin: "Aerocetus nubivagus (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "A floating gas-bag the size of a building: giant vocal folds and a horn as long as a house turn its infrasonic moans into towering harmonics.",
        role: "Sub-bass drones, infrasonic moans, slow groans",
        oversample: 1,
        gate_times: Some(GateTimes { attack: 0.35, release: 0.9 }),
        params: vec![
            ParamDef::new("pitch", "Moan pitch", 8.0, 60.0, 16.0).unit("Hz").log().smooth(0.15).pitch(),
            ParamDef::new("pressure", "Bag pressure", 2.0, 20.0, 8.0).unit("cmH₂O").smooth(0.1),
            ParamDef::new("bright", "Brightness", 0.0, 1.0, 0.6).smooth(0.1),
            ParamDef::new("rough", "Groan", 0.0, 1.0, 0.1).smooth(0.1),
            ParamDef::new("mouth", "Horn mouth", 0.0, 1.0, 0.7).smooth(0.2),
        ],
        calls: vec![
            CallDef::new(
                "moan",
                "Moan",
                "Six seconds of infrasonic moan sinking slowly",
                Some(16.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 19.0), ("rough", 0.05), ("bright", 0.6)]),
                    k(0.8, &[("gate", 1.0)]),
                    k(5.4, &[("gate", 1.0), ("pitch", 13.0)]).curve(Curve::Exp),
                    k(6.2, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "groan",
                "Groan",
                "A slow, creaking groan that swells and sags",
                Some(16.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 10.0), ("rough", 0.6), ("pressure", 6.0), ("bright", 0.8), ("mouth", 0.3)]),
                    k(0.6, &[("gate", 1.0)]),
                    k(2.4, &[("pitch", 14.0), ("rough", 0.75), ("pressure", 8.0), ("mouth", 0.9)]).curve(Curve::Exp),
                    k(4.4, &[("gate", 1.0), ("pitch", 9.0), ("rough", 0.5), ("pressure", 7.0), ("mouth", 0.4)]).curve(Curve::Exp),
                    k(5.0, &[("gate", 0.0), ("rough", 0.1), ("pressure", 8.0), ("bright", 0.6), ("mouth", 0.7)]),
                ],
            ),
            CallDef::new(
                "hail",
                "Hail",
                "A rising call that climbs out of the infrasound and falls back",
                Some(16.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 14.0), ("pressure", 9.0), ("rough", 0.05)]),
                    k(0.5, &[("gate", 1.0)]),
                    k(2.0, &[("pitch", 48.0), ("pressure", 5.5)]).curve(Curve::Exp),
                    k(3.4, &[("gate", 1.0), ("pitch", 20.0), ("pressure", 9.0)]).curve(Curve::Exp),
                    k(4.0, &[("gate", 0.0), ("pressure", 8.0)]),
                ],
            ),
        ],
        events: vec![],
        create,
    }
    .resolved()
}
