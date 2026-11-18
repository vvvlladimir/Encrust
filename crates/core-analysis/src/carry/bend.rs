//! A part bending over its own sections between its highest tip and the layer pulled on: a
//! beam up the stack, summed as it climbs so no section is kept (ADR 0455).

use crate::layer::Piece;

use super::GREEN_MODULUS_MPA;

/// One layer of a part as a beam's section: its centre, mm, and the inverse of its second
/// moments about it, `[xx, xy, yy]`, per mm⁴.
#[derive(Debug, Clone, Copy)]
pub(super) struct Slice {
    centre_mm: [f64; 2],
    inverse: [f64; 3],
}

impl Slice {
    /// The section `pieces` make together, or `None` where they cure nothing.
    pub(super) fn of<'p>(pieces: impl Iterator<Item = &'p Piece> + Clone) -> Option<Self> {
        let (mut area, mut sx, mut sy) = (0.0, 0.0, 0.0);
        for piece in pieces.clone() {
            let a = f64::from(piece.area_mm2);
            area += a;
            sx += a * f64::from(piece.centre_mm[0]);
            sy += a * f64::from(piece.centre_mm[1]);
        }
        if area <= 0.0 {
            return None;
        }
        let centre_mm = [sx / area, sy / area];
        let (mut xx, mut xy, mut yy) = (0.0, 0.0, 0.0);
        for piece in pieces {
            let a = f64::from(piece.area_mm2);
            let [pxx, pxy, pyy] = piece.spread_mm2.map(f64::from);
            let dx = f64::from(piece.centre_mm[0]) - centre_mm[0];
            let dy = f64::from(piece.centre_mm[1]) - centre_mm[1];
            xx += a * (pxx + dx * dx);
            xy += a * (pxy + dx * dy);
            yy += a * (pyy + dy * dy);
        }
        let det = xx * yy - xy * xy;
        (det > 0.0).then(|| Self {
            centre_mm,
            inverse: [yy / det, -xy / det, xx / det],
        })
    }
}

/// The sections a part climbed through since its highest tip, as four running sums of the
/// compliance of each, so the bend under any later pull is read in closed form.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Bend {
    /// Σ w J, Σ w J c, Σ w z J, Σ w z J c, with `w` the layer's rise over the modulus.
    compliance: [f64; 3],
    compliance_at: [f64; 2],
    moment: [f64; 3],
    moment_at: [f64; 2],
}

impl Bend {
    /// Adds a section `rise_mm` thick at height `z_mm`.
    pub(super) fn add(&mut self, z_mm: f64, rise_mm: f64, slice: &Slice) {
        let w = rise_mm / GREEN_MODULUS_MPA;
        let j = slice.inverse;
        let jc = turn(j, slice.centre_mm);
        for ((near, far), j) in self.compliance.iter_mut().zip(&mut self.moment).zip(j) {
            *near += w * j;
            *far += w * z_mm * j;
        }
        for ((near, far), jc) in self
            .compliance_at
            .iter_mut()
            .zip(&mut self.moment_at)
            .zip(jc)
        {
            *near += w * jc;
            *far += w * z_mm * jc;
        }
    }

    /// How far the layer at `z_mm` moves across, mm, under `force_n` pulling at `at_mm`.
    pub(super) fn shift(&self, z_mm: f64, at_mm: [f64; 2], force_n: f64) -> [f64; 2] {
        let near = turn(self.compliance, at_mm);
        let far = turn(self.moment, at_mm);
        [0, 1].map(|axis| {
            let upper = z_mm * (near[axis] - self.compliance_at[axis]);
            let lower = far[axis] - self.moment_at[axis];
            -force_n * (upper - lower)
        })
    }
}

fn turn([xx, xy, yy]: [f64; 3], [x, y]: [f64; 2]) -> [f64; 2] {
    [xx * x + xy * y, xy * x + yy * y]
}
