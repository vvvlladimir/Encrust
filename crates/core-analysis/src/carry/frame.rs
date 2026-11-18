//! A part as a rigid body on its tips' springs: a load moves it, and each tip carries what
//! its own spring is moved by (ADR 0455).

use super::{GREEN_MODULUS_MPA, Tip};

/// Six unknowns: the part's move along x, y, z, millimetres, then its turn about them, radians.
type Six = [f64; 6];

/// Below this share of its largest pivot a pivot is nothing: the tips cannot hold the part
/// still that way, and it rocks.
const ROCKS_BELOW: f64 = 1e-12;

/// What a load does to the part at one tip.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Reaction {
    /// Along z, newtons, positive where the tip is stretched.
    pub(super) axial_n: f64,
    /// Across, newtons.
    pub(super) lateral_n: f64,
    /// Bending the tip's neck as the part turns, N·mm.
    pub(super) bending_n_mm: f64,
}

impl Reaction {
    /// The highest stress in the tip's neck, megapascals: tension, bending by its turn, and
    /// the lateral force as shear, since it acts where the neck meets the part.
    pub(super) fn stress_mpa(&self, radius_mm: f64) -> f64 {
        let area = std::f64::consts::PI * radius_mm * radius_mm;
        let modulus = area * radius_mm / 4.0;
        (self.axial_n.max(0.0) + self.lateral_n) / area + self.bending_n_mm / modulus
    }
}

/// A spring as its stiffness each way, N/mm: `k[i][j]` is the force along `i` per mm moved along `j`.
pub(super) type Stiffness = [[f64; 3]; 3];

/// The stiffness `tip`'s own support gives, or `None` where it has no spring.
pub(super) fn own(tip: &Tip) -> Option<Stiffness> {
    let spring = tip.spring?;
    let (across, along) = (spring.lateral_n_per_mm, spring.axial_n_per_mm);
    Some([[across, 0.0, 0.0], [0.0, across, 0.0], [0.0, 0.0, along]])
}

/// `k` in series with something that gives `compliance` mm per N behind it.
pub(super) fn series(k: Stiffness, compliance: Stiffness) -> Option<Stiffness> {
    let soft = invert(k)?;
    invert(add(soft, compliance))
}

/// A part on the springs of its tips, solved once for that set of tips.
pub(super) struct Frame {
    origin: [f64; 3],
    /// Lower Cholesky factor of the stiffness about `origin`, or `None` where the part rocks.
    factor: Option<[Six; 6]>,
}

impl Frame {
    /// The part on `held`, each tip with the stiffness it is held by.
    pub(super) fn of(held: &[(&Tip, Stiffness)]) -> Self {
        let origin = centre(held.iter().map(|(tip, _)| *tip));
        let mut stiffness = [[0.0; 6]; 6];
        for (tip, k) in held {
            let r = off(tip, origin);
            let rows = [mover(0, r), mover(1, r), mover(2, r)];
            for a in 0..3 {
                for b in 0..3 {
                    for i in 0..6 {
                        for j in 0..6 {
                            stiffness[i][j] += rows[a][i] * k[a][b] * rows[b][j];
                        }
                    }
                }
            }
            let turning = turning(tip);
            for (axis, row) in stiffness.iter_mut().enumerate().skip(3) {
                row[axis] += turning;
            }
        }
        Self {
            origin,
            factor: cholesky(&stiffness),
        }
    }

    /// Whether the tips leave the part a way to turn or slide that nothing resists.
    pub(super) fn rocks(&self) -> bool {
        self.factor.is_none()
    }

    /// How the part moves under `loads`, each a force in newtons, positive z away from the
    /// plate, at a point in mm; `None` where it rocks.
    pub(super) fn moved(&self, loads: &[([f64; 3], [f64; 3])]) -> Option<Moved> {
        let factor = self.factor.as_ref()?;
        let mut load = [0.0; 6];
        for &(force, at) in loads {
            let r = sub(at, self.origin);
            for (axis, force) in force.into_iter().enumerate() {
                for (sum, row) in load.iter_mut().zip(mover(axis, r)) {
                    *sum += row * force;
                }
            }
        }
        Some(Moved {
            origin: self.origin,
            six: solve(factor, load),
        })
    }

    /// How far `at` moves per newton pushed on it each way, mm/N; `None` where the part rocks.
    pub(super) fn compliance_at(&self, at: [f64; 3]) -> Option<Stiffness> {
        let mut compliance = [[0.0; 3]; 3];
        for axis in 0..3 {
            let mut unit = [0.0; 3];
            unit[axis] = 1.0;
            let moved = self.moved(&[(unit, at)])?.at(at);
            for (row, value) in compliance.iter_mut().zip(moved) {
                row[axis] = value;
            }
        }
        Some(compliance)
    }
}

/// A move of the part.
pub(super) struct Moved {
    origin: [f64; 3],
    six: Six,
}

impl Moved {
    /// How far the point `at` moves, mm.
    pub(super) fn at(&self, at: [f64; 3]) -> [f64; 3] {
        let r = sub(at, self.origin);
        let [ux, uy, uz, tx, ty, tz] = self.six;
        [
            ux + ty * r[2] - tz * r[1],
            uy + tz * r[0] - tx * r[2],
            uz + tx * r[1] - ty * r[0],
        ]
    }

    /// The force `tip`, held by `k`, carries of the move, N, positive z where it is stretched.
    pub(super) fn force(&self, tip: &Tip, k: &Stiffness) -> [f64; 3] {
        let d = self.at(position(tip));
        k.map(|row| row[0] * d[0] + row[1] * d[1] + row[2] * d[2])
    }

    /// What `tip`, held by `k`, carries of the move.
    pub(super) fn reaction(&self, tip: &Tip, k: &Stiffness) -> Reaction {
        let [fx, fy, fz] = self.force(tip, k);
        Reaction {
            axial_n: fz,
            lateral_n: fx.hypot(fy),
            bending_n_mm: turning(tip) * self.six[3].hypot(self.six[4]),
        }
    }
}

/// How hard `tip` resists the part turning on it, N·mm per radian: its neck, as long as it
/// is wide, in series with the support under it.
fn turning(tip: &Tip) -> f64 {
    let r = tip.radius_mm;
    let neck = GREEN_MODULUS_MPA * std::f64::consts::PI * r * r * r / 8.0;
    let column = tip.spring.map_or(f64::INFINITY, |s| s.turning_n_mm_per_rad);
    1.0 / (1.0 / neck + 1.0 / column)
}

fn add(a: Stiffness, b: Stiffness) -> Stiffness {
    std::array::from_fn(|i| std::array::from_fn(|j| a[i][j] + b[i][j]))
}

/// The inverse of a 3×3 matrix, or `None` where it has none.
fn invert(m: Stiffness) -> Option<Stiffness> {
    let cofactor = |i: usize, j: usize| {
        let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
        let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
        m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
    };
    let det: f64 = (0..3).map(|j| m[0][j] * cofactor(0, j)).sum();
    (det.abs() > f64::MIN_POSITIVE)
        .then(|| std::array::from_fn(|i| std::array::from_fn(|j| cofactor(j, i) / det)))
}

/// The row taking the part's six unknowns to the move of a point `r` off the origin along `axis`.
fn mover(axis: usize, [rx, ry, rz]: [f64; 3]) -> Six {
    match axis {
        0 => [1.0, 0.0, 0.0, 0.0, rz, -ry],
        1 => [0.0, 1.0, 0.0, -rz, 0.0, rx],
        _ => [0.0, 0.0, 1.0, ry, -rx, 0.0],
    }
}

fn centre<'t>(tips: impl Iterator<Item = &'t Tip>) -> [f64; 3] {
    let (mut sum, mut count) = ([0.0; 3], 0.0);
    for tip in tips {
        let at = position(tip);
        for axis in 0..3 {
            sum[axis] += at[axis];
        }
        count += 1.0;
    }
    sum.map(|value| value / f64::max(count, 1.0))
}

fn position(tip: &Tip) -> [f64; 3] {
    [
        f64::from(tip.at_mm[0]),
        f64::from(tip.at_mm[1]),
        f64::from(tip.z_mm),
    ]
}

fn off(tip: &Tip, origin: [f64; 3]) -> [f64; 3] {
    sub(position(tip), origin)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The lower factor of a symmetric matrix, or `None` where a pivot is next to nothing.
fn cholesky(matrix: &[Six; 6]) -> Option<[Six; 6]> {
    let largest = (0..6).map(|i| matrix[i][i]).fold(0.0, f64::max);
    let mut lower = [[0.0; 6]; 6];
    for i in 0..6 {
        for j in 0..=i {
            let sum: f64 = (0..j).map(|k| lower[i][k] * lower[j][k]).sum();
            if i == j {
                let pivot = matrix[i][i] - sum;
                if pivot <= ROCKS_BELOW * largest {
                    return None;
                }
                lower[i][i] = pivot.sqrt();
            } else {
                lower[i][j] = (matrix[i][j] - sum) / lower[j][j];
            }
        }
    }
    Some(lower)
}

fn solve(lower: &[Six; 6], load: Six) -> Six {
    let mut y = [0.0; 6];
    for i in 0..6 {
        let sum: f64 = (0..i).map(|k| lower[i][k] * y[k]).sum();
        y[i] = (load[i] - sum) / lower[i][i];
    }
    let mut x = [0.0; 6];
    for i in (0..6).rev() {
        let sum: f64 = (i + 1..6).map(|k| lower[k][i] * x[k]).sum();
        x[i] = (y[i] - sum) / lower[i][i];
    }
    x
}
