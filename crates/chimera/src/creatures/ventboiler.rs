//! Vent Boiler (invented): lives on deep-sea hydrothermal vents and talks by boiling. Superheated
//! water flashes into cavitation bubbles (Rayleigh–Plesset) at the back of a soft head cavity:
//! each bubble swells, collapses with a crack and rings at its Minnaert frequency (≈ 3.2/r₀ Hz,
//! r₀ in m). The bubbles' volume velocity drives the head, a tube closed at the back and opening
//! through a mouth, whose resonance colours the crackle into pops and gurgles. Heat is the bubble
//! rate, the bubble size sets the pitch, and roil scatters the sizes and strengths.

use super::build::{TubeArea, TubeOpts, dx_min, tube};
use super::types::{CallDef, CallKey, CreatureDef, CreatureGroup, CreatureSim, Curve, GateTimes, ParamDef};
use crate::airway::network::{AirwayNetwork, Tube};
use crate::math::{PI, Rng, exp, js_max, js_min, pow};
use crate::physics::{C_SOUND, WALL_SOFT};
use crate::sources::valves::{Bubble, ClickTrain};

const PITCH: usize = 0;
const HEAT: usize = 1;
const ROIL: usize = 2;
const HEAD: usize = 3;
const MOUTH: usize = 4;

const HEAD_MIN: f64 = 90.0;
const HEAD_MAX: f64 = 600.0;
/// head cross-section (cm²)
const HEAD_AREA: f64 = 10.0;
const MOUTH_LEN: f64 = 5.0;
/// bubbles that can sound at once
const POOL: usize = 4;
/// cm³ per m³
const M3_TO_CM3: f64 = 1.0e6;
/// Minnaert: f·r₀ ≈ 3.2 Hz·m at one atmosphere
const MINNAERT: f64 = 3.2;
/// a bubble's ring dies away in about this many periods
const DAMP_CYCLES: f64 = 8.0;
/// share of bubbles that cavitate with a crack, at full roil
const CRACKS: f64 = 0.35;
/// volume velocity of a unit crack (cm³/s)
const CRACK_FLOW: f64 = 1500.0;

pub struct VentBoiler {
    net: AirwayNetwork,
    tubes: Vec<(&'static str, Tube)>,
    head: Tube,
    mouth: Tube,
    bubbles: [Bubble; POOL],
    /// each bubble's volume less its equilibrium volume (m³), last sample
    vol: [f64; POOL],
    /// equilibrium volume (m³)
    v0: [f64; POOL],
    next: usize,
    rng: Rng,
    fs: f64,
    dt: f64,
    /// expected bubbles per internal sample
    rate: f64,
    r0: f64,
    roil: f64,
    prev_gate: f64,
    head_area: Vec<f64>,
    mouth_area: Vec<f64>,
    lp1: f64,
    lp2: f64,
    lp_coef: f64,
    level: f64,
    cracks: ClickTrain,
    crack_flash: f64,
    heat_level: f64,
}

fn head_length(f: f64) -> f64 {
    // quarter wave, less the mouth's share
    js_max(4.0, C_SOUND / (4.0 * f) - 0.5 * MOUTH_LEN)
}

fn create(fs: f64) -> Box<dyn CreatureSim> {
    let mut net = AirwayNetwork::new(fs);
    let n = js_max(3.0, (head_length(HEAD_MAX) / dx_min(fs)).floor());
    let head = tube(&mut net, "head", fs, head_length(HEAD_MIN), TubeArea::Uniform(HEAD_AREA), TubeOpts { wall: Some(WALL_SOFT), min: Some(n), max: Some(n), wall_radiation: Some(0.2), perimeter_factor: Some(1.3), ..Default::default() });
    let mouth = tube(&mut net, "mouth", fs, MOUTH_LEN, TubeArea::Uniform(2.0), TubeOpts { wall: Some(WALL_SOFT), perimeter_factor: Some(1.6), ..Default::default() });
    net.connect(head.last_node(), mouth.first_node());
    net.add_radiation(mouth.last_node());
    net.finalize();
    let bubbles: [Bubble; POOL] = std::array::from_fn(|_| Bubble::new(fs));
    let mut cracks = ClickTrain::new(fs);
    cracks.width = 0.00012;
    let v0 = 4.0 / 3.0 * PI * bubbles[0].r0 * bubbles[0].r0 * bubbles[0].r0;
    Box::new(VentBoiler {
        net,
        tubes: vec![("head", head), ("mouth", mouth)],
        head,
        mouth,
        bubbles,
        vol: [0.0; POOL],
        v0: [v0; POOL],
        next: 0,
        rng: Rng::new(0x0b0_11e5),
        fs,
        dt: 1.0 / fs,
        rate: 0.0,
        r0: 0.004,
        roil: 0.4,
        prev_gate: 0.0,
        head_area: vec![HEAD_AREA; head.n],
        mouth_area: vec![2.0; mouth.n],
        lp1: 0.0,
        lp2: 0.0,
        lp_coef: 0.1,
        level: M3_TO_CM3,
        cracks,
        crack_flash: 0.0,
        heat_level: 1.0,
    })
}

impl VentBoiler {
    /// Release one bubble of `size` × the pitch's radius. It pinches off ringing (`strength` 0..1
    /// sets the amplitude); a `crack` also cavitates, with a sharp shock before the ring.
    fn boil(&mut self, size: f64, strength: f64, crack: bool) {
        let i = self.next;
        self.next = (self.next + 1) % POOL;
        let b = &mut self.bubbles[i];
        let r0 = js_max(0.0002, js_min(0.02, self.r0 * size));
        // damping: the ring dies in about DAMP_CYCLES periods, whatever the size
        b.viscosity = (1000.0 * r0 * MINNAERT) / (2.0 * DAMP_CYCLES);
        let strength = js_max(0.0, js_min(1.5, strength));
        b.pluck(r0, 0.06 + 0.24 * strength);
        if crack {
            // the collapse's shock: a sharp volume pulse (Bubble's own snap collapses too violently
            // to integrate at 48 kHz for vent-sized bubbles)
            self.cracks.click(0.0, strength * (0.5 + 0.5 * self.rng.next()));
        }
        self.v0[i] = 4.0 / 3.0 * PI * r0 * r0 * r0;
    }

    fn random_boil(&mut self, gain: f64) {
        let size = 1.0 + self.roil * (1.3 * self.rng.next() - 0.5);
        let strength = gain * (1.0 - 0.6 * self.roil * self.rng.next());
        let crack = self.rng.next() < CRACKS * self.roil;
        self.boil(size, strength, crack);
    }
}

impl CreatureSim for VentBoiler {
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
        0.6e-7
    }
    fn control(&mut self, p: &[f64], gate: f64, _dt: f64) {
        self.r0 = MINNAERT / p[PITCH];
        // big bubbles move far more water: even out the level across the keyboard
        // and a hard boil is louder, but less than in proportion
        self.heat_level = js_min(1.0, pow(14.0 / p[HEAT], 0.3));
        self.level = M3_TO_CM3 * pow(p[PITCH] / 800.0, 1.2) * self.heat_level;
        self.lp_coef = 1.0 - exp((-2.0 * PI * js_min(0.4 * self.fs, 1.6 * p[PITCH])) / self.fs);
        self.roil = p[ROIL];
        self.rate = p[HEAT] * gate * self.dt;
        self.head_area.fill(HEAD_AREA);
        self.net.set_tube_targets(self.head, &self.head_area, Some(head_length(p[HEAD])), None);
        let m = p[MOUTH];
        let n = self.mouth.n;
        for (i, a) in self.mouth_area.iter_mut().enumerate() {
            // the lips at the end narrow most
            let u = (i as f64 + 0.5) / n as f64;
            *a = 0.15 + m * (6.0 - 3.0 * u);
        }
        self.net.set_tube_targets(self.mouth, &self.mouth_area, None, None);
        if gate > 0.05 && self.prev_gate <= 0.05 {
            self.random_boil(0.9);
        }
        self.prev_gate = gate;
    }
    #[inline]
    fn sample(&mut self) -> f64 {
        if self.rate > 0.0 && self.rng.next() < self.rate {
            self.random_boil(1.0);
        }
        // the bubbles' volume velocity (their swelling about equilibrium) drives the head
        let mut u = 0.0;
        for i in 0..POOL {
            let b = &mut self.bubbles[i];
            b.next();
            let r = b.radius();
            let v = 4.0 / 3.0 * PI * r * r * r - self.v0[i];
            u += (v - self.vol[i]) * self.fs;
            self.vol[i] = v;
        }
        // the bubbles sit in a mesh of vapour and water that smooths their onsets: a 2-pole low-pass
        // above the ring keeps the pluck's flow step from ringing every mode of the head
        self.lp1 += self.lp_coef * (u - self.lp1);
        self.lp2 += self.lp_coef * (self.lp1 - self.lp2);
        let c = self.cracks.next();
        self.crack_flash = js_max(self.crack_flash * 0.999, c.abs());
        self.net.ext_flow[self.head.first_node()] += self.lp2 * self.level + c * CRACK_FLOW * self.heat_level;
        self.net.step()
    }
    fn source_state(&self, out: &mut Vec<(&'static str, f64)>) {
        out.clear();
        let mut swell: f64 = 0.0;
        for b in &self.bubbles {
            swell = js_max(swell, (b.radius() / b.r0 - 1.0).abs() * 3.0);
        }
        out.push(("bubbles", js_max(0.0, js_min(1.0, swell))));
        out.push(("crack", js_min(1.0, self.crack_flash)));
    }
    fn trigger(&mut self, _event: usize, amount: f64) {
        // a belch: one big, hard bubble
        let a = js_min(1.5, js_max(0.0, amount));
        self.boil(1.0 + 0.4 * a, 0.4 + 0.4 * a, false);
    }
}

pub fn def() -> CreatureDef {
    let k = CallKey::new;
    CreatureDef {
        id: "ventboiler",
        name: "Vent Boiler",
        latin: "Fervor abyssalis (invented)",
        group: CreatureGroup::Fantasy,
        blurb: "Lives on deep-sea vents and talks by boiling: cavitation bubbles crack and ring inside a soft head cavity.",
        role: "Crackles, pops, gurgles and bubbling percussion",
        oversample: 1,
        gate_times: Some(GateTimes { attack: 0.01, release: 0.1 }),
        params: vec![
            ParamDef::new("pitch", "Bubble pitch", 250.0, 3000.0, 800.0).unit("Hz").log().smooth(0.02).pitch(),
            ParamDef::new("heat", "Heat", 0.5, 200.0, 14.0).unit("/s").log().smooth(0.08),
            ParamDef::new("roil", "Roil", 0.0, 1.0, 0.4).smooth(0.05).digits(2),
            ParamDef::new("head", "Head resonance", HEAD_MIN, HEAD_MAX, 240.0).unit("Hz").log().smooth(0.06),
            ParamDef::new("mouth", "Mouth", 0.0, 1.0, 0.5).smooth(0.04).digits(2),
        ],
        calls: vec![
            CallDef::new(
                "simmer",
                "Simmer",
                "Slow pops gathering into a steady simmer",
                Some(800.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("heat", 3.0), ("roil", 0.3), ("mouth", 0.4)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(2.0, &[("heat", 25.0), ("mouth", 0.7)]).curve(Curve::Exp),
                    k(2.4, &[("gate", 1.0)]),
                    k(2.5, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "boil",
                "Rolling boil",
                "Heat surges to a seething roar while the head opens and closes",
                Some(800.0),
                vec![
                    k(0.0, &[("gate", 0.0), ("heat", 20.0), ("roil", 0.7), ("mouth", 0.2), ("head", 180.0)]),
                    k(0.05, &[("gate", 1.0)]),
                    k(0.9, &[("heat", 180.0), ("mouth", 0.9), ("head", 320.0)]).curve(Curve::Exp),
                    k(1.5, &[("mouth", 0.3), ("head", 200.0)]),
                    k(2.2, &[("heat", 30.0), ("mouth", 0.8)]).curve(Curve::Exp),
                    k(2.3, &[("gate", 0.0)]),
                ],
            ),
            CallDef::new(
                "belch",
                "Belch",
                "Three big bubbles burst up through a widening throat",
                Some(800.0),
                vec![
                    k(0.0, &[("pitch", 500.0), ("head", 140.0), ("mouth", 0.3), ("!pop", 0.8)]),
                    k(0.22, &[("pitch", 420.0), ("head", 170.0), ("mouth", 0.6), ("!pop", 1.0)]).curve(Curve::Exp),
                    k(0.5, &[("pitch", 330.0), ("head", 210.0), ("mouth", 1.0), ("!pop", 1.4)]).curve(Curve::Exp),
                    k(1.2, &[("mouth", 1.0)]),
                ],
            ),
        ],
        events: vec!["pop"],
        create,
    }
    .resolved()
}
