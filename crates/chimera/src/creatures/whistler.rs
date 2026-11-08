//! Whistler (invented): its whole body is one resizable pipe, closed at the tail and open at the
//! mouth, sounded by an aerodynamic whistle at the closed end. The whistle is a van der Pol
//! oscillator coupled to the pipe's own pressure, so it locks onto the pipe's resonances: stretch or
//! squeeze the pipe and the note slides with it, then snaps to the next resonance, like a bugle
//! played by changing the bugle.

use super::build::{CMH2O, TubeArea, TubeOpts, dx_min, fill_areas, tube};
use super::types::{
    CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, GateTimes, ParamDef,
};
use crate::airway::network::{AirwayNetwork, NoiseSpec, PortSpec, Tube};
use crate::math::{PI, js_max, js_min, pow};
use crate::physics::C_SOUND;
use crate::sources::valves::Whistle;

const PITCH: usize = 0;
const STRETCH: usize = 1;
const FLARE: usize = 2;
const BREATH: usize = 3;
const COUPLING: usize = 4;

/// At stretch 1 the pipe's fifth odd harmonic (5·c/4L) sits on the played pitch, so its
/// neighbours (3/5 and 7/5 of the note) are close enough to snap to.
const MODE: f64 = 5.0;
const PITCH_MIN: f64 = 500.0;
const PITCH_MAX: f64 = 2000.0;
const STRETCH_MIN: f64 = 0.7;
const STRETCH_MAX: f64 = 1.5;
/// cross-section of the pipe (cm²) before the flare
const BORE: f64 = 1.6;
/// whistle flow (cm³/s) per unit oscillator excursion
const FLOW: f64 = 60.0;
/// local pressure (dyn/cm²) that reads as full scale in the whistle's coupling
const P_SCALE: f64 = 1.0;

/// length scale measured so that stretch 1 plays the note in tune (within ~2 % over the range)
const END_CORRECTION: f64 = 1.033;

fn tuned_length(pitch: f64, stretch: f64) -> f64 {
    END_CORRECTION * stretch * MODE * C_SOUND / (4.0 * pitch)
}

pub struct Whistler {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    body: Tube,
    lungs: usize,
    jet: usize,
    whistle: Whistle,
    drive: f64,
    min_length: f64,
    /// previous oscillator output, for its velocity
    prev_x: f64,
    /// whistle flow scale (cm³/s) for the current note
    flow: f64,
    /// fs / ω: turns the per-sample change of x into a unit-amplitude velocity
    vel_scale: f64,
    fs: f64,
    areas: Vec<f64>,
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    // built at its shortest, so stretching only ever lengthens the sections (CFL stays safe)
    let min_length = tuned_length(PITCH_MAX, STRETCH_MIN);
    let body = tube(
        &mut net,
        "body",
        fs,
        min_length,
        TubeArea::Uniform(BORE),
        TubeOpts {
            wall: None,
            max: Some(200.0),
            ..Default::default()
        },
    );
    debug_assert!(min_length / body.n as f64 >= dx_min(fs));
    let lungs = net.add_pressure_node("lungs");
    // the breath enters through a narrow jet slit beside the whistle (its turbulence is the hiss)
    let jet = net.add_port(
        lungs,
        body.first_node(),
        PortSpec::new(0.04, 0.3).kinetic(0.5, 1.0),
    );
    net.add_radiation(body.last_node());
    net.finalize();
    Box::new(Whistler {
        net,
        tubes: vec![("body", body)],
        body,
        lungs,
        jet,
        whistle: Whistle::new(fs),
        drive: 0.0,
        min_length,
        prev_x: 0.0,
        flow: FLOW,
        vel_scale: 0.0,
        fs,
        areas: vec![0.0; body.n],
    })
}

impl CreatureSim for Whistler {
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
        1.0e-7
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        let breath = p[BREATH];
        self.drive = js_min(1.0, breath * gate);
        self.whistle.f = p[PITCH];
        self.vel_scale = self.fs / (2.0 * PI * p[PITCH]);
        // radiation rises with frequency: less whistle flow for higher notes keeps the level even
        self.flow = FLOW * pow(880.0 / p[PITCH], 1.3);
        self.whistle.coupling = p[COUPLING];
        self.net
            .set_fixed_pressure(self.lungs, 4.0 * CMH2O * breath * gate);
        self.net.set_port_target(self.jet, 0.01 + 0.05 * breath);
        let length = js_max(self.min_length, tuned_length(p[PITCH], p[STRETCH]));
        let flare = p[FLARE];
        let areas = fill_areas(
            self.body,
            |u| BORE * (1.0 + flare * 6.0 * u * u * u * u),
            &mut self.areas,
        );
        self.net
            .set_tube_targets(self.body, areas, Some(length), None);
        self.net.set_noise_sources(&[NoiseSpec::new(
            self.jet,
            self.body.branch(0),
            self.jet,
            2e-6,
        )
        .band(6000.0, 800.0)]);
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let first = self.body.first_node();
        // the whistle feels the pipe's pressure at the closed end and blows flow in step with its
        // own velocity, which pulls it toward the nearest pipe resonance (below one, the pressure
        // it meets stiffens it; above one, it softens it)
        let x = self
            .whistle
            .next(self.drive, self.net.p[first] * P_SCALE, 1.0);
        let vel = (x - self.prev_x) * self.vel_scale;
        self.prev_x = x;
        self.net.ext_flow[first] += self.flow * vel;
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("whistle", js_min(1.0, self.prev_x.abs() / 2.0)));
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "whistler",
        name: "Whistler",
        latin: "Aulosoma tubicen (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "Its whole body is one pipe sounded by a whistle at the tail; stretch the pipe and the note slides with it, then snaps to the next resonance.",
        role: "Glassy leads, bugle-like breaks and glissandi",
        oversample: 2,
        gate_times: Some(GateTimes { attack: 0.02, release: 0.08 }),
        params: vec![
            ParamDef::new("pitch", "Whistle pitch", PITCH_MIN, PITCH_MAX, 880.0).unit("Hz").log().smooth(0.01).pitch(),
            ParamDef::new("stretch", "Body stretch", STRETCH_MIN, STRETCH_MAX, 1.0).smooth(0.03).digits(2),
            ParamDef::new("flare", "Mouth flare", 0.0, 1.0, 0.3).smooth(0.03),
            ParamDef::new("breath", "Breath", 0.0, 1.0, 0.7).smooth(0.02),
            ParamDef::new("coupling", "Lock to body", 0.0, 2.0, 1.4).smooth(0.03),
        ],
        calls: vec![
            CallDef::new(
                "glide",
                "Stretch glide",
                "Holds a note while the body stretches: the pitch slides, then snaps up a resonance",
                Some(880.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 880.0), ("stretch", 0.85), ("breath", 0.7)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(1.6, &[("stretch", 1.45)]),
                    k(2.2, &[("stretch", 0.85)]),
                    k(2.3, &[("gate", 1.0)]),
                    k(2.45, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "warble",
                "Warble",
                "The body pulses short and long, flipping between two resonances",
                Some(1000.0),
                {
                    let mut keys = vec![k(0.0, &[("gate", 0.0), ("pitch", 1000.0), ("stretch", 1.0), ("breath", 0.8)]), k(0.04, &[("gate", 1.0)])];
                    for i in 0..6 {
                        let t = 0.1 + i as f64 * 0.22;
                        keys.push(k(t, &[("stretch", 1.0)]));
                        keys.push(k(t + 0.11, &[("stretch", 1.3)]));
                    }
                    keys.push(k(1.45, &[("gate", 1.0), ("stretch", 1.0)]));
                    keys.push(k(1.6, &[("gate", 0.0)]));
                    keys
                },
            ),
            CallDef::new(
                "hoot",
                "Falling hoot",
                "A breathy hoot that glides down an octave, the body lengthening to follow",
                Some(1400.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 1400.0), ("stretch", 1.0), ("breath", 0.5), ("flare", 0.8)]),
                    k(0.08, &[("gate", 1.0), ("breath", 0.9)]),
                    k(1.0, &[("pitch", 700.0), ("breath", 0.6)]).curve(Curve::Exp),
                    k(1.2, &[("gate", 0.0)]),
                ],
            ),
        ],
        events: vec![],
        create,
    }
    .resolved()
}
