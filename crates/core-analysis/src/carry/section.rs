//! The tips under a part read as one section of separate discs, and one layer's pull on it.

use crate::layer::Piece;

use super::Tip;

/// One layer's pull on one part: its force, and that force's moments.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Pull {
    pub(super) force_n: f64,
    pub(super) moment_x: f64,
    pub(super) moment_y: f64,
    /// Force times the ellipse its peel lever may point to, `[xx, xy, yy]`, N·mm: the film's
    /// last grip off each piece's centre, far along its length and near across its width.
    pub(super) peel: [f64; 3],
}

impl Pull {
    pub(super) fn add(&mut self, piece: &Piece) {
        let force = f64::from(piece.pull_n);
        self.force_n += force;
        self.moment_x += force * f64::from(piece.centre_mm[0]);
        self.moment_y += force * f64::from(piece.centre_mm[1]);
        // Half the spread keeps a disc's lever at half its polar gyration, as calibrated.
        let lever = root(piece.spread_mm2.map(|moment| f64::from(moment) / 2.0));
        for (peel, lever) in self.peel.iter_mut().zip(lever) {
            *peel += force * lever;
        }
    }
}

/// The square root of a symmetric `[xx, xy, yy]` with no negative eigenvalue, in closed form.
fn root([xx, xy, yy]: [f64; 3]) -> [f64; 3] {
    let det = (xx * yy - xy * xy).max(0.0).sqrt();
    let scale = (xx + yy + 2.0 * det).sqrt();
    if scale <= 0.0 {
        return [0.0; 3];
    }
    [(xx + det) / scale, xy / scale, (yy + det) / scale]
}

/// The tips under a part read as one section of separate discs.
#[derive(Debug, Clone, Copy)]
pub(super) struct Section {
    area_mm2: f64,
    centre_mm: [f64; 2],
    /// The inverse of its second moments, `[xx, xy, yy]`, per mm^4.
    inverse: [f64; 3],
}

impl Section {
    pub(super) fn of<'t>(tips: impl Iterator<Item = &'t Tip> + Clone) -> Self {
        let area = |tip: &Tip| std::f64::consts::PI * tip.radius_mm * tip.radius_mm;
        let (mut area_mm2, mut sum_x, mut sum_y) = (0.0, 0.0, 0.0);
        for tip in tips.clone() {
            let a = area(tip);
            area_mm2 += a;
            sum_x += a * f64::from(tip.at_mm[0]);
            sum_y += a * f64::from(tip.at_mm[1]);
        }
        let centre_mm = [sum_x / area_mm2, sum_y / area_mm2];
        let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
        for tip in tips {
            let (dx, dy) = off(tip, centre_mm);
            let a = area(tip);
            let own = a * tip.radius_mm * tip.radius_mm / 4.0;
            xx += a * dx * dx + own;
            xy += a * dx * dy;
            yy += a * dy * dy + own;
        }
        let det = xx * yy - xy * xy;
        Self {
            area_mm2,
            centre_mm,
            inverse: [yy / det, -xy / det, xx / det],
        }
    }

    /// `[x, y]` through the inverse: the stress gradient a moment makes, or the reverse.
    fn turned(&self, x: f64, y: f64) -> (f64, f64) {
        let [xx, xy, yy] = self.inverse;
        (xx * x + xy * y, xy * x + yy * y)
    }

    /// The highest stress in `tip`, whichever way the film lets go: the pull's own moment
    /// about the section, then its peel lever taken the way that is worst for this tip.
    pub(super) fn stress_mpa(&self, tip: &Tip, pull: &Pull) -> f64 {
        if pull.force_n <= 0.0 {
            return 0.0;
        }
        let (mx, my) = (
            pull.moment_x - pull.force_n * self.centre_mm[0],
            pull.moment_y - pull.force_n * self.centre_mm[1],
        );
        let (gx, gy) = self.turned(mx, my);
        let (dx, dy) = off(tip, self.centre_mm);
        let (tx, ty) = self.turned(dx, dy);
        let [pxx, pxy, pyy] = pull.peel;
        let toward = (pxx * tx + pxy * ty).hypot(pxy * tx + pyy * ty);
        let [ixx, ixy, iyy] = self.inverse;
        let bent = [
            [ixx * pxx + ixy * pxy, ixx * pxy + ixy * pyy],
            [ixy * pxx + iyy * pxy, ixy * pxy + iyy * pyy],
        ];
        let peel = toward + tip.radius_mm * largest_stretch(bent);
        (pull.force_n / self.area_mm2 + gx * dx + gy * dy + gx.hypot(gy) * tip.radius_mm + peel)
            .max(0.0)
    }
}

/// The most `matrix` lengthens a unit vector: its largest singular value, in closed form.
fn largest_stretch([[a, b], [c, d]]: [[f64; 2]; 2]) -> f64 {
    let sum = a * a + b * b + c * c + d * d;
    let det = a * d - b * c;
    f64::midpoint(sum, (sum * sum - 4.0 * det * det).max(0.0).sqrt()).sqrt()
}

fn off(tip: &Tip, centre_mm: [f64; 2]) -> (f64, f64) {
    (
        f64::from(tip.at_mm[0]) - centre_mm[0],
        f64::from(tip.at_mm[1]) - centre_mm[1],
    )
}
