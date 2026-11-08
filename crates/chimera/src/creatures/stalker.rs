//! Stalker (invented): a predator with a mouth inside its mouth. Behind wide outer jaws sits a
//! narrow inner throat-jaw that shoots forward through them. Its hiss is turbulence where air is
//! forced through the inner jaw's teeth; its shriek is pressed, high vocal folds with an
//! aerodynamic whistle at those teeth singing an octave above. Pushing the inner jaw out lengthens
//! the narrow tube and shortens the wide one, so the whole sound changes shape as it strikes.

use super::build::{CMH2O, TubeArea, TubeOpts, dx_min, fill_areas, lungs, tube};
use super::types::{
    CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, GateTimes, ParamDef,
};
use crate::airway::network::{AirwayNetwork, NoiseSpec, PortSpec, Tube};
use crate::math::{clamp, js_max, js_min, pow, sqrt};
use crate::physics::{WALL_SOFT, WALL_TRACHEA};
use crate::sources::glottis::ParametricGlottis;
use crate::sources::valves::{ClickTrain, Whistle};

const PITCH: usize = 0;
const MODE: usize = 1;
const REACH: usize = 2;
const PRESSURE: usize = 3;
const RASP: usize = 4;
const GAPE: usize = 5;

const SNAP: usize = 0;

const TRACHEA_AREA: f64 = 2.5;
/// inner jaw length (cm) retracted and fully out
const INNER_MIN: f64 = 3.0;
const INNER_MAX: f64 = 13.0;
/// outer mouth length (cm): the inner jaw pushes through it, so it shortens as the jaw comes out
const OUTER_MIN: f64 = 3.0;
const OUTER_MAX: f64 = 10.0;
/// whistle flow (cm³/s) per unit excursion at full drive
const WHISTLE_FLOW: f64 = 15.0;

pub struct Stalker {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    trachea: Tube,
    throat: Tube,
    inner: Tube,
    outer: Tube,
    lungs: usize,
    glottis: usize,
    teeth: usize,
    folds: ParametricGlottis,
    whistle: Whistle,
    whistle_drive: f64,
    whistle_flow: f64,
    /// how hard the teeth hiss now (0..1, for the glyph)
    hiss_level: f64,
    clicks: ClickTrain,
    inner_areas: Vec<f64>,
    outer_areas: Vec<f64>,
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let trachea = tube(
        &mut net,
        "trachea",
        fs,
        14.0,
        TubeArea::Uniform(TRACHEA_AREA),
        TubeOpts {
            wall: Some(WALL_TRACHEA),
            coarse: Some(1.6),
            ..Default::default()
        },
    );
    let throat = tube(
        &mut net,
        "throat",
        fs,
        7.0,
        TubeArea::Uniform(4.0),
        TubeOpts {
            wall: Some(WALL_SOFT),
            perimeter_factor: Some(1.3),
            ..Default::default()
        },
    );
    // both jaws are built at their shortest, so changing their lengths only lengthens sections
    let inner = tube(
        &mut net,
        "inner",
        fs,
        INNER_MIN,
        TubeArea::Uniform(0.8),
        TubeOpts {
            wall: Some(WALL_SOFT),
            perimeter_factor: Some(1.4),
            min: Some(4.0),
            ..Default::default()
        },
    );
    let outer = tube(
        &mut net,
        "outer",
        fs,
        OUTER_MIN,
        TubeArea::Uniform(7.0),
        TubeOpts {
            wall: Some(WALL_SOFT),
            perimeter_factor: Some(1.6),
            min: Some(3.0),
            ..Default::default()
        },
    );
    debug_assert!(
        INNER_MIN / inner.n as f64 >= dx_min(fs) && OUTER_MIN / outer.n as f64 >= dx_min(fs)
    );
    let lungs = lungs(&mut net, trachea.first_node(), TRACHEA_AREA);
    let glottis = net.add_port(
        trachea.last_node(),
        throat.first_node(),
        PortSpec::new(0.0, 0.4)
            .kinetic(0.37, 1.0)
            .slit(sqrt(TRACHEA_AREA) * 0.9),
    );
    net.connect(throat.last_node(), inner.first_node());
    let teeth = net.connect(inner.last_node(), outer.first_node());
    net.add_radiation(outer.last_node());
    net.finalize();
    let mut folds = ParametricGlottis::new(fs);
    folds.params.drift = 10.0;
    let mut clicks = ClickTrain::new(fs);
    clicks.ring = 3400.0;
    clicks.ring_decay = 0.0015;
    clicks.width = 0.0003;
    Box::new(Stalker {
        net,
        tubes: vec![
            ("trachea", trachea),
            ("throat", throat),
            ("inner", inner),
            ("outer", outer),
        ],
        trachea,
        throat,
        inner,
        outer,
        lungs,
        glottis,
        teeth,
        folds,
        whistle: Whistle::new(fs),
        whistle_drive: 0.0,
        whistle_flow: 0.0,
        hiss_level: 0.0,
        clicks,
        inner_areas: vec![0.0; inner.n],
        outer_areas: vec![0.0; outer.n],
    })
}

impl CreatureSim for Stalker {
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
        7.0e-8
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        let mode = p[MODE];
        let reach = p[REACH];
        self.net
            .set_fixed_pressure(self.lungs, p[PRESSURE] * CMH2O * gate);
        let g = &mut self.folds.params;
        g.f0 = p[PITCH];
        // shriek: pressed folds (short open phase, abrupt closure); hiss: folds pulled wide apart
        g.amplitude = 0.12 * mode * clamp(pow(800.0 / p[PITCH], 0.5), 0.6, 1.5);
        g.abduction = 0.6 * (1.0 - mode);
        g.open_quotient = 0.4;
        g.closure_sharpness = 0.9;
        g.effort = 0.95;
        g.chaos = 0.75 * p[RASP];
        g.jitter = 0.004 + 0.03 * p[RASP];
        g.shimmer = 0.03 + 0.15 * p[RASP];
        self.whistle.f = 2.0 * p[PITCH];
        self.whistle.coupling = 0.5;
        self.whistle_drive = mode * gate;
        self.whistle_flow = WHISTLE_FLOW * pow(800.0 / p[PITCH], 1.2);
        // the inner jaw: a narrow tube whose tip is a ring of teeth (the hiss's constriction)
        let inner_len = INNER_MIN + (INNER_MAX - INNER_MIN) * reach;
        let tip = 0.12 + 0.25 * mode;
        let areas = fill_areas(
            self.inner,
            |u| if u > 0.8 { tip } else { 0.9 + 0.4 * u },
            &mut self.inner_areas,
        );
        self.net
            .set_tube_targets(self.inner, areas, Some(inner_len), None);
        let outer_len = js_max(OUTER_MIN, OUTER_MAX - (OUTER_MAX - OUTER_MIN) * reach);
        let gape = p[GAPE];
        let areas = fill_areas(
            self.outer,
            |u| 7.0 + (0.4 + 14.0 * gape - 7.0) * u * u,
            &mut self.outer_areas,
        );
        self.net
            .set_tube_targets(self.outer, areas, Some(outer_len), None);
        let n = self.inner.n;
        let hiss = 1.0 - 0.7 * mode;
        self.hiss_level = (1.0 - mode) * gate;
        self.net.set_noise_sources(&[
            NoiseSpec::new(
                self.inner.branch(n - 2),
                self.teeth,
                self.inner.branch(n - 3),
                5.5e-4 * hiss,
            )
            .band(7000.0, 1500.0),
            NoiseSpec::new(
                self.glottis,
                self.throat.branch(1),
                self.trachea.branch(self.trachea.n - 2),
                1e-4 * hiss,
            )
            .band(4000.0, 400.0),
        ]);
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let dp = self.net.p[self.trachea.last_node()] - self.net.p[self.throat.first_node()];
        let area = self.folds.next(dp);
        self.net.set_port_area(self.glottis, area);
        let tip = self.inner.last_node();
        let w = self
            .whistle
            .next(self.whistle_drive, self.net.p[tip], self.whistle_flow);
        self.net.ext_flow[tip] += w;
        let c = self.clicks.next();
        self.net.ext_flow[self.outer.first_node()] += 250.0 * c;
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("glottis", js_min(1.0, self.folds.area / 0.6)));
        let whistle = self.whistle.out.abs() / js_max(1e-9, 2.0 * self.whistle_flow);
        out.push(("teeth", js_min(1.0, js_max(whistle, self.hiss_level))));
    }
    fn trigger(&mut self, event: usize, amount: f64) {
        if event == SNAP {
            // the inner teeth meet: a sharp double clack
            self.clicks.click(0.0, amount);
            self.clicks.click(0.004, -0.6 * amount);
        }
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "stalker",
        name: "Stalker",
        latin: "Endognathus insidiator (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "A mouth inside a mouth: a narrow inner jaw shoots out through the outer one. It hisses through the inner teeth and shrieks with pressed folds and a whistle at those teeth.",
        role: "Hiss textures, piercing screams, percussive strikes",
        oversample: 2,
        gate_times: Some(GateTimes { attack: 0.008, release: 0.05 }),
        params: vec![
            ParamDef::new("pitch", "Shriek pitch", 250.0, 2000.0, 800.0).unit("Hz").log().smooth(0.015).pitch(),
            ParamDef::new("mode", "Hiss ↔ shriek", 0.0, 1.0, 1.0).smooth(0.02),
            ParamDef::new("reach", "Inner jaw reach", 0.0, 1.0, 0.3).smooth(0.015),
            ParamDef::new("pressure", "Breath pressure", 5.0, 40.0, 20.0).unit("cmH₂O").smooth(0.02),
            ParamDef::new("rasp", "Rasp", 0.0, 1.0, 0.25).smooth(0.04),
            ParamDef::new("gape", "Outer jaws", 0.0, 1.0, 0.7).smooth(0.03),
        ],
        calls: vec![
            CallDef::new(
                "hiss",
                "Hiss",
                "A long breathy threat forced through the inner teeth",
                None,
                vec![
                    k(0.0, &[("gate", 0.0), ("mode", 0.0), ("reach", 0.15), ("gape", 0.4), ("pressure", 14.0)]),
                    k(0.15, &[("gate", 1.0)]),
                    k(1.1, &[("gate", 1.0), ("pressure", 30.0), ("reach", 0.45), ("gape", 0.8)]),
                    k(1.4, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "shriek",
                "Shriek",
                "A rising scream that tears as the inner jaw pushes out",
                Some(800.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("mode", 1.0), ("pitch", 650.0), ("reach", 0.1), ("rasp", 0.15), ("pressure", 22.0), ("gape", 0.9)]),
                    k(0.03, &[("gate", 1.0)]),
                    k(0.9, &[("pitch", 1300.0), ("reach", 0.9), ("rasp", 0.6), ("pressure", 22.0)]).curve(Curve::Exp),
                    k(1.15, &[("pitch", 1100.0), ("gate", 1.0)]).curve(Curve::Exp),
                    k(1.25, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "strike",
                "Inner-jaw strike",
                "A hiss, then the inner jaw shoots out with a clack and a shriek, and pulls back",
                Some(800.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("mode", 0.0), ("reach", 0.0), ("gape", 0.5), ("pressure", 18.0), ("rasp", 0.25)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(0.45, &[("reach", 0.0), ("mode", 0.0), ("gape", 0.5)]),
                    k(0.5, &[("reach", 1.0), ("gape", 1.0), ("pressure", 26.0)]),
                    k(0.5, &[("mode", 1.0), ("pitch", 1000.0), ("!snap", 1.0)]).curve(Curve::Step),
                    k(0.85, &[("pitch", 780.0), ("mode", 1.0), ("rasp", 0.5)]).curve(Curve::Exp),
                    k(0.9, &[("mode", 0.0), ("pressure", 16.0)]),
                    k(1.25, &[("reach", 0.0), ("gape", 0.5), ("gate", 1.0)]),
                    k(1.4, &[("gate", 0.0)]),
                ],
            ),
        ],
        events: vec!["snap"],
        create,
    }
    .resolved()
}
