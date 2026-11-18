//! One layer's pull, and the part's own weight, read through the tips holding the part:
//! shared as one rigid section, or by the springs each support gives (ADR 0422, 0455).

use super::Tip;
use super::bend::Bend;
use super::frame::{Frame, Moved, Reaction, Stiffness, own, series};
use super::section::{Pull, Section};

/// Directions round the peel's ellipse the film may let go from; eight keep the worst
/// within a few percent of the ellipse's own.
const PEEL_WAYS: usize = 8;

/// What one layer did to one tip of a part.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Felt {
    /// In its tip, or in the contact its tree stands on the model by where that is worse.
    pub(super) stress_mpa: f64,
    /// How far the layer moved across under its pull, mm.
    pub(super) shift_mm: f64,
    /// Where the pull acted that moved it furthest, mm.
    pub(super) shift_at_mm: [f64; 2],
    /// What the squeeze as the plate goes down presses the tip's trunk with, newtons.
    pub(super) squeeze_n: f64,
}

/// What a tip's support stands on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Hold {
    /// The plate, or something held still.
    Still,
    /// Nothing: the part it holds, which it ties to itself, or a part that rocks.
    Loose,
    /// Another part, `part`, which gives `compliance` mm per N where it stands.
    On { part: u32, compliance: Stiffness },
}

/// One layer of one part: its pull, the part's weight with the point it hangs from, what
/// supports standing on it pass down, each a force in N at a point in mm, and its height.
#[derive(Clone)]
pub(super) struct Layer {
    pub(super) pull: Pull,
    pub(super) weight: (f64, [f64; 3]),
    pub(super) passed: Vec<([f64; 3], [f64; 3])>,
    pub(super) z_mm: f64,
}

/// What a layer did to a part's tips, and what they pass down to each part they stand on.
pub(super) struct Reading {
    pub(super) felt: Vec<Felt>,
    pub(super) passed: Vec<(u32, [f64; 3], [f64; 3])>,
}

/// What `layer` does to each of `tips`, all of the one part and each held as `holds` says,
/// given how the part bends above them: by their springs where every tip has one, else as one
/// rigid section.
pub(super) fn read(tips: &[&Tip], holds: &[Hold], layer: &Layer, bend: &Bend) -> Reading {
    if tips.iter().all(|tip| tip.spring.is_some()) {
        sprung(tips, holds, layer, bend)
    } else {
        Reading {
            felt: rigid(tips, layer, bend),
            passed: Vec::new(),
        }
    }
}

fn rigid(tips: &[&Tip], layer: &Layer, bend: &Bend) -> Vec<Felt> {
    let mut pull = layer.pull;
    let (weight_n, [wx, wy, _]) = layer.weight;
    pull.hang(weight_n, [wx, wy]);
    for &([_, _, force], [x, y, _]) in &layer.passed {
        pull.hang(force, [x, y]);
    }
    let section = Section::of(tips.iter().copied());
    let (shift_mm, shift_at_mm) = worst(layer, |at| bend.shift(layer.z_mm, at, layer.pull.force_n));
    tips.iter()
        .map(|tip| Felt {
            stress_mpa: section.stress_mpa(tip, &pull),
            shift_mm,
            shift_at_mm,
            squeeze_n: 0.0,
        })
        .collect()
}

fn sprung(tips: &[&Tip], holds: &[Hold], layer: &Layer, bend: &Bend) -> Reading {
    let held: Vec<Option<Stiffness>> = tips
        .iter()
        .zip(holds)
        .map(|(tip, hold)| held_by(tip, hold))
        .collect();
    let holding: Vec<(&Tip, Stiffness)> = tips
        .iter()
        .zip(&held)
        .filter_map(|(tip, k)| Some((*tip, (*k)?)))
        .collect();
    let frame = Frame::of(&holding);
    let mut felt = vec![Felt::default(); tips.len()];
    if holding.is_empty() || frame.rocks() {
        let rocking = Felt {
            stress_mpa: f64::INFINITY,
            shift_mm: f64::INFINITY,
            shift_at_mm: centre(&layer.pull),
            squeeze_n: 0.0,
        };
        felt.fill(rocking);
        return Reading {
            felt,
            passed: Vec::new(),
        };
    }
    let force = layer.pull.force_n;
    let mut worst_shift = (0.0, centre(&layer.pull));
    for at in levers(&layer.pull) {
        let Some(moved) = frame.moved(&loads(layer, force, at)) else {
            continue;
        };
        let stress = stresses(tips, &held, &moved);
        for (one, stress) in felt.iter_mut().zip(stress) {
            one.stress_mpa = one.stress_mpa.max(stress);
        }
        if force <= 0.0 {
            continue;
        }
        let [dx, dy, _] = moved.at([at[0], at[1], layer.z_mm]);
        let [bx, by] = bend.shift(layer.z_mm, at, force);
        let shift = (dx + bx).hypot(dy + by);
        if shift > worst_shift.0 {
            worst_shift = (shift, at);
        }
    }
    let squeezed = squeeze(&frame, tips, &held, layer);
    for (one, squeeze_n) in felt.iter_mut().zip(squeezed) {
        (one.shift_mm, one.shift_at_mm) = worst_shift;
        one.squeeze_n = squeeze_n;
    }
    let passed = frame
        .moved(&loads(layer, force, centre(&layer.pull)))
        .map(|moved| passed_down(tips, holds, &held, &moved))
        .unwrap_or_default();
    Reading { felt, passed }
}

/// The stiffness `tip` holds its part by: its own, in series with the part it stands on;
/// none where it stands on its own part.
fn held_by(tip: &Tip, hold: &Hold) -> Option<Stiffness> {
    let k = own(tip)?;
    match hold {
        Hold::Still => Some(k),
        Hold::Loose => None,
        Hold::On { compliance, .. } => series(k, *compliance),
    }
}

/// The pull at `at`, the weight and what is passed down, as the frame's loads.
fn loads(layer: &Layer, force: f64, at: [f64; 2]) -> Vec<([f64; 3], [f64; 3])> {
    let (weight_n, weight_at) = layer.weight;
    let mut loads = vec![
        ([0.0, 0.0, force], [at[0], at[1], layer.z_mm]),
        ([0.0, 0.0, weight_n], weight_at),
    ];
    loads.extend_from_slice(&layer.passed);
    loads
}

/// The stress each tip takes from `moved`: in its own neck, and in the one contact its tree
/// stands on the model by, which every tip of that tree loads.
fn stresses(tips: &[&Tip], held: &[Option<Stiffness>], moved: &Moved) -> Vec<f64> {
    let forces: Vec<[f64; 3]> = tips
        .iter()
        .zip(held)
        .map(|(tip, k)| k.map_or([0.0; 3], |k| moved.force(tip, &k)))
        .collect();
    let mut stress: Vec<f64> = tips
        .iter()
        .zip(held)
        .map(|(tip, k)| k.map_or(0.0, |k| moved.reaction(tip, &k).stress_mpa(tip.radius_mm)))
        .collect();
    for run in by_trunk(tips) {
        let Some(footing) = tips[run[0]].stands_on else {
            continue;
        };
        let total = run.iter().fold([0.0; 3], |sum, at| {
            std::array::from_fn(|axis| sum[axis] + forces[*at][axis])
        });
        let foot = Reaction {
            axial_n: total[2],
            lateral_n: total[0].hypot(total[1]),
            bending_n_mm: 0.0,
        }
        .stress_mpa(footing.radius_mm);
        for at in run {
            stress[at] = stress[at].max(foot);
        }
    }
    stress
}

/// What each tip standing on another part passes down to it: the force it carries, at the
/// point its support stands on.
fn passed_down(
    tips: &[&Tip],
    holds: &[Hold],
    held: &[Option<Stiffness>],
    moved: &Moved,
) -> Vec<(u32, [f64; 3], [f64; 3])> {
    tips.iter()
        .zip(holds)
        .zip(held)
        .filter_map(|((tip, hold), k)| {
            let Hold::On { part, .. } = hold else {
                return None;
            };
            let footing = tip.stands_on?;
            let at = footing.at_mm.map(f64::from);
            Some((*part, moved.force(tip, &(*k)?), at))
        })
        .collect()
}

/// What each tip's trunk is pressed with as the plate goes down into the resin and the
/// film pushes the layer back as hard as it pulled, the part's weight against it.
fn squeeze(frame: &Frame, tips: &[&Tip], held: &[Option<Stiffness>], layer: &Layer) -> Vec<f64> {
    let Some(moved) = frame.moved(&loads(layer, -layer.pull.force_n, centre(&layer.pull))) else {
        return vec![0.0; tips.len()];
    };
    let pressed: Vec<f64> = tips
        .iter()
        .zip(held)
        .map(|(tip, k)| k.map_or(0.0, |k| -moved.force(tip, &k)[2]))
        .collect();
    let mut on_trunk = vec![0.0; tips.len()];
    for run in by_trunk(tips) {
        let total: f64 = run.iter().map(|at| pressed[*at]).sum();
        for at in run {
            on_trunk[at] = total.max(0.0);
        }
    }
    on_trunk
}

/// The tips, by index, gathered by the trunk they stand on.
fn by_trunk(tips: &[&Tip]) -> Vec<Vec<usize>> {
    let trunk = |at: usize| tips[at].spring.map_or(u32::MAX, |spring| spring.trunk);
    let mut order: Vec<usize> = (0..tips.len()).collect();
    order.sort_unstable_by_key(|at| trunk(*at));
    order
        .chunk_by(|a, b| trunk(*a) == trunk(*b))
        .map(<[usize]>::to_vec)
        .collect()
}

/// The largest of `shift` over the points round the peel's ellipse, and where.
fn worst(layer: &Layer, shift: impl Fn([f64; 2]) -> [f64; 2]) -> (f64, [f64; 2]) {
    levers(&layer.pull)
        .map(|at| {
            let [x, y] = shift(at);
            (x.hypot(y), at)
        })
        .fold(
            (0.0, centre(&layer.pull)),
            |a, b| if b.0 > a.0 { b } else { a },
        )
}

/// Where the pull acts, mm: the middle of what pulls.
fn centre(pull: &Pull) -> [f64; 2] {
    if pull.force_n <= 0.0 {
        return [0.0, 0.0];
    }
    [pull.moment_x / pull.force_n, pull.moment_y / pull.force_n]
}

/// The points the film's last grip may lie at, round the ellipse of its peel lever.
fn levers(pull: &Pull) -> impl Iterator<Item = [f64; 2]> + '_ {
    let c = centre(pull);
    let [xx, xy, yy] = pull
        .peel
        .map(|value| value / pull.force_n.max(f64::MIN_POSITIVE));
    (0..PEEL_WAYS).map(move |way| {
        let angle = std::f64::consts::TAU * way as f64 / PEEL_WAYS as f64;
        let (y, x) = angle.sin_cos();
        [c[0] + xx * x + xy * y, c[1] + xy * x + yy * y]
    })
}
