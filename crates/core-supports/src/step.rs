use core_geometry::{Buckets, Scalar, Vec3};

/// The steps a run has held that still reach the layer; faces within a step's height are held
/// as part of it, steadying supports likewise (ADR 0326); bucketed, dropped when passed.
pub(crate) struct Zones {
    buckets: Buckets<Zone>,
    /// The lowest height any zone still reaches up to.
    spent_z: Scalar,
}

/// One sample of a step, how far above it the step reaches, and what it put out.
#[derive(Debug, Clone, Copy)]
struct Zone {
    at: Vec3,
    height_mm: Scalar,
    new_mm3: Scalar,
}

impl Zone {
    fn spent_z(self) -> Scalar {
        self.at.z + self.height_mm
    }
}

impl Zones {
    /// `tallest_mm` is the furthest any step reaches.
    pub(crate) fn new(tallest_mm: Scalar) -> Self {
        Self {
            buckets: Buckets::new(tallest_mm.max(1e-3)),
            spent_z: Scalar::INFINITY,
        }
    }

    /// Takes on a sample of a step at `at` that reaches `height_mm` above itself and put
    /// out `new_mm3` in its layer.
    pub(crate) fn add(&mut self, at: Vec3, height_mm: Scalar, new_mm3: Scalar) {
        let zone = Zone {
            at,
            height_mm,
            new_mm3,
        };
        self.spent_z = self.spent_z.min(zone.spent_z());
        self.buckets.insert(at.truncate(), zone);
    }

    /// What the largest step whose reach takes in `at` put out, or `None` where no step
    /// at or below its height reaches it.
    pub(crate) fn reaching(&self, at: Vec3) -> Option<Scalar> {
        self.buckets
            .around(at.truncate())
            .filter(|zone| zone.at.z <= at.z && zone.at.distance(at) <= zone.height_mm)
            .map(|zone| zone.new_mm3)
            .max_by(Scalar::total_cmp)
    }

    /// Drops every step that reaches nothing at `z` or above.
    pub(crate) fn prune(&mut self, z: Scalar) {
        if self.spent_z >= z {
            return;
        }
        self.buckets.retain(|zone| zone.spent_z() >= z);
        self.spent_z = self
            .buckets
            .iter()
            .map(|zone| zone.spent_z())
            .fold(Scalar::INFINITY, Scalar::min);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_reaches_what_is_above_it_within_its_height_and_nothing_else() {
        let mut zones = Zones::new(3.2);
        zones.add(Vec3::new(10.0, 10.0, 5.0), 1.5, 2.0);
        let reached = |x: Scalar, z: Scalar| zones.reaching(Vec3::new(x, 10.0, z));
        assert_eq!(reached(10.0, 6.0), Some(2.0), "a millimetre straight above");
        assert_eq!(reached(11.0, 6.0), Some(2.0), "1.41 mm away, inside 1.5");
        assert_eq!(reached(11.5, 6.0), None, "1.8 mm away is past its height");
        assert_eq!(reached(10.5, 4.5), None, "below it is not above it");
    }

    #[test]
    fn the_largest_step_reaching_a_place_sizes_it_and_a_spent_one_goes() {
        let mut zones = Zones::new(3.2);
        zones.add(Vec3::new(10.0, 10.0, 5.0), 1.5, 2.0);
        zones.add(Vec3::new(10.0, 11.0, 5.0), 3.2, 9.0);
        assert_eq!(zones.reaching(Vec3::new(10.0, 10.5, 6.0)), Some(9.0));
        zones.prune(7.0);
        assert_eq!(zones.buckets.len(), 1, "1.5 mm up is spent");
        assert_eq!(zones.reaching(Vec3::new(10.0, 10.5, 7.5)), Some(9.0));
    }
}
