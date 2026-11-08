//! Accordion Neck (invented): vocal folds at the base of a pleated neck that stretches and shrinks
//! while it sings. The neck is the vocal tract, so its length moves every formant at once (a
//! human tract changes length by a centimetre or two; this one by most of a metre), and a pouch
//! that slides along it and the depth of its pleats bend the vowel into shapes no mouth can make.

use super::build::{CMH2O, TubeArea, TubeOpts, dx_min, fill_areas, lungs, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, ParamDef};
use crate::airway::network::{AirwayNetwork, NoiseSpec, PortSpec, Tube};
use crate::math::{PI, clamp, exp, js_max, js_min, pow, sin, sqrt};
use crate::physics::{WALL_SOFT, WALL_TRACHEA};
use crate::sources::glottis::ParametricGlottis;

const PITCH: usize = 0;
const NECK: usize = 1;
const POUCH: usize = 2;
const PLEATS: usize = 3;
const PRESSURE: usize = 4;
const MOUTH: usize = 5;

const NECK_MIN: f64 = 14.0;
const NECK_MAX: f64 = 90.0;
const BORE: f64 = 2.4;
const TRACHEA_AREA: f64 = 2.0;
const PLEAT_COUNT: f64 = 7.0;

pub struct Accordion {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    trachea: Tube,
    neck: Tube,
    head: Tube,
    lungs: usize,
    glottis: usize,
    folds: ParametricGlottis,
    neck_areas: Vec<f64>,
    head_areas: Vec<f64>,
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let trachea = tube(
        &mut net,
        "trachea",
        fs,
        16.0,
        TubeArea::Uniform(TRACHEA_AREA),
        TubeOpts {
            wall: Some(WALL_TRACHEA),
            coarse: Some(1.6),
            ..Default::default()
        },
    );
    // built at its shortest: stretching only lengthens the sections, so the CFL limit holds
    let neck = tube(
        &mut net,
        "neck",
        fs,
        NECK_MIN,
        TubeArea::Uniform(BORE),
        TubeOpts {
            wall: Some(WALL_SOFT),
            wall_radiation: Some(0.3),
            perimeter_factor: Some(1.3),
            ..Default::default()
        },
    );
    debug_assert!(NECK_MIN / neck.n as f64 >= dx_min(fs));
    let head = tube(
        &mut net,
        "head",
        fs,
        5.0,
        TubeArea::Uniform(3.0),
        TubeOpts {
            wall: Some(WALL_SOFT),
            perimeter_factor: Some(1.4),
            ..Default::default()
        },
    );
    let lungs = lungs(&mut net, trachea.first_node(), TRACHEA_AREA);
    let glottis = net.add_port(
        trachea.last_node(),
        neck.first_node(),
        PortSpec::new(0.0, 0.4)
            .kinetic(0.37, 1.0)
            .slit(sqrt(TRACHEA_AREA) * 0.9),
    );
    net.connect(neck.last_node(), head.first_node());
    net.add_radiation(head.last_node());
    net.finalize();
    let mut folds = ParametricGlottis::new(fs);
    folds.params.vibrato_rate = 5.0;
    Box::new(Accordion {
        net,
        tubes: vec![("trachea", trachea), ("neck", neck), ("head", head)],
        trachea,
        neck,
        head,
        lungs,
        glottis,
        folds,
        neck_areas: vec![0.0; neck.n],
        head_areas: vec![0.0; head.n],
    })
}

impl CreatureSim for Accordion {
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
        4.5e-7
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        self.net
            .set_fixed_pressure(self.lungs, p[PRESSURE] * CMH2O * gate);
        let g = &mut self.folds.params;
        g.f0 = p[PITCH];
        // radiation rises with pitch: smaller fold excursions up high keep the level even
        g.amplitude = 0.22 * clamp(pow(180.0 / p[PITCH], 0.6), 0.5, 1.6);
        g.open_quotient = 0.6;
        g.effort = 0.7;
        g.vibrato_depth = 18.0;
        let length = js_min(NECK_MAX, js_max(NECK_MIN, p[NECK]));
        let (pouch, pleats) = (p[POUCH], p[PLEATS]);
        let areas = fill_areas(
            self.neck,
            |u| {
                let d = (u - pouch) / 0.13;
                BORE * (1.0 + 0.85 * pleats * sin(2.0 * PI * PLEAT_COUNT * u))
                    * (1.0 + 3.5 * exp(-d * d))
            },
            &mut self.neck_areas,
        );
        self.net
            .set_tube_targets(self.neck, areas, Some(length), None);
        let mouth = p[MOUTH];
        let areas = fill_areas(
            self.head,
            |u| 3.0 - (3.0 - (0.3 + 5.0 * mouth)) * u,
            &mut self.head_areas,
        );
        self.net.set_tube_targets(self.head, areas, None, None);
        self.net.set_noise_sources(&[NoiseSpec::new(
            self.glottis,
            self.neck.branch(1),
            self.trachea.branch(self.trachea.n - 2),
            3e-7,
        )
        .band(4000.0, 300.0)]);
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        let dp = self.net.p[self.trachea.last_node()] - self.net.p[self.neck.first_node()];
        let area = self.folds.next(dp);
        self.net.set_port_area(self.glottis, area);
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        out.push(("glottis", js_min(1.0, self.folds.area / 0.3)));
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    let mut squeeze = vec![
        k(
            0.0,
            &[
                ("gate", 0.0),
                ("pitch", 196.0),
                ("neck", 24.0),
                ("pleats", 0.6),
                ("pouch", 0.4),
            ],
        ),
        k(0.05, &[("gate", 1.0)]),
    ];
    // a little tune on the squeezebox: each note pumps the neck out and back
    for (i, f) in [196.0, 247.0, 294.0, 247.0, 330.0, 294.0]
        .iter()
        .enumerate()
    {
        let t = 0.05 + i as f64 * 0.32;
        squeeze.push(k(t, &[("pitch", *f), ("neck", 24.0)]).curve(Curve::Step));
        squeeze.push(k(t + 0.18, &[("neck", 62.0)]));
    }
    squeeze.push(k(2.0, &[("neck", 30.0), ("gate", 1.0)]));
    squeeze.push(k(2.15, &[("gate", 0.0)]));
    CreatureDef {
        id: "accordion",
        name: "Accordion Neck",
        latin: "Plicicollis cantans (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "Vocal folds at the foot of a pleated neck that stretches to most of a metre while it sings, sliding every formant at once through vowels no mouth can make.",
        role: "Morphing pads, talking-bass glides, wah leads",
        oversample: 2,
        gate_times: None,
        params: vec![
            ParamDef::new("pitch", "Pitch", 70.0, 600.0, 180.0).unit("Hz").log().smooth(0.02).pitch(),
            ParamDef::new("neck", "Neck length", NECK_MIN, NECK_MAX, 32.0).unit("cm").smooth(0.06).digits(0),
            ParamDef::new("pouch", "Pouch position", 0.0, 1.0, 0.5).smooth(0.06),
            ParamDef::new("pleats", "Pleat depth", 0.0, 1.0, 0.3).smooth(0.06),
            ParamDef::new("pressure", "Breath pressure", 4.0, 30.0, 12.0).unit("cmH₂O").smooth(0.02),
            ParamDef::new("mouth", "Mouth opening", 0.0, 1.0, 0.6).smooth(0.03),
        ],
        calls: vec![
            CallDef::new(
                "stretch",
                "Long stretch",
                "One held note while the neck unfolds to full length and snaps back",
                Some(180.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 180.0), ("neck", 15.0), ("pouch", 0.2), ("pleats", 0.2)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(2.0, &[("neck", 88.0), ("pouch", 0.8), ("pleats", 0.7)]),
                    k(2.4, &[("neck", 20.0), ("gate", 1.0)]),
                    k(2.6, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new("squeeze", "Squeezebox", "A little tune; every note pumps the neck out and back", Some(196.0), squeeze),
            CallDef::new(
                "sigh",
                "Concertina sigh",
                "A falling moan while the pouch rolls down the neck",
                Some(260.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("pitch", 260.0), ("neck", 50.0), ("pouch", 0.95), ("pleats", 0.0), ("mouth", 1.0), ("pressure", 14.0)]),
                    k(0.1, &[("gate", 1.0)]),
                    k(1.6, &[("pitch", 110.0), ("pouch", 0.05), ("pleats", 0.9), ("neck", 70.0), ("mouth", 0.35), ("pressure", 20.0)]).curve(Curve::Exp),
                    k(1.8, &[("gate", 0.0)]),
                ],
            ),
        ],
        events: vec![],
        create,
    }
    .resolved()
}
