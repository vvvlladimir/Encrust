use core_geometry::{Buckets, Scalar, Vec2, Vec3};

/// The supports standing under one piece of a layer.
#[derive(Debug, Clone)]
pub(super) struct Held {
    pub(super) buckets: Buckets<Hold>,
    /// The lowest height any support here still holds up to, so that a set with nothing
    /// to drop is not walked at all.
    pub(super) spent_z: Scalar,
}

/// One support standing, and the radius of the ball it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Hold {
    pub(super) at: Vec3,
    pub(super) reach_mm: Scalar,
}

impl Hold {
    /// The height past which this support holds nothing.
    pub(super) fn spent_z(self) -> Scalar {
        self.at.z + self.reach_mm
    }
}

impl Held {
    /// Nothing held yet, in buckets as wide as the coverage radius `radius_mm`.
    pub(super) fn new(radius_mm: Scalar) -> Self {
        Self {
            buckets: Buckets::new(radius_mm),
            spent_z: Scalar::INFINITY,
        }
    }

    /// Whether anything here already holds `at` on the layer whose contacts sit at `z`,
    /// each by its own ball, which is no wider than the side of a bucket.
    pub(super) fn holds(&self, at: Vec2, z: Scalar, above_mm: Scalar) -> bool {
        self.buckets.around(at).any(|hold| {
            let reach = reach_at(z - hold.at.z, hold.reach_mm, above_mm);
            (hold.at.truncate() - at).length_squared() < reach * reach
        })
    }

    /// Whether anything here stands within `clear_mm` of `at` on the plate, whatever its
    /// height; `clear_mm` is no more than the side of a bucket.
    pub(super) fn near(&self, at: Vec2, clear_mm: Scalar) -> bool {
        self.buckets
            .around(at)
            .any(|hold| (hold.at.truncate() - at).length_squared() < clear_mm * clear_mm)
    }

    pub(super) fn add(&mut self, contact: Vec3, reach_mm: Scalar) {
        self.put(Hold {
            at: contact,
            reach_mm,
        });
    }

    pub(super) fn put(&mut self, hold: Hold) {
        self.spent_z = self.spent_z.min(hold.spent_z());
        self.buckets.insert(hold.at.truncate(), hold);
    }

    /// Drops everything that holds nothing at `z` or above.
    pub(super) fn prune(&mut self, z: Scalar) {
        if self.buckets.is_empty() || self.spent_z >= z {
            return;
        }
        self.buckets.retain(|hold| hold.spent_z() >= z);
        self.spent_z = self
            .buckets
            .iter()
            .map(|hold| hold.spent_z())
            .fold(Scalar::INFINITY, Scalar::min);
    }

    /// Takes on everything of `other` that still holds something at `z` or above and is not here
    /// already, so pieces that split and rejoin do not hold each support many times.
    pub(super) fn take_in(&mut self, other: &Self, z: Scalar) {
        for hold in other.buckets.iter() {
            if hold.spent_z() >= z && !self.buckets.at(hold.at.truncate()).contains(hold) {
                self.put(*hold);
            }
        }
    }
}

/// How far a standing support still holds, `rise` mm above itself: a ball of the coverage
/// radius (ADR 0079).
pub(super) fn reach_at(rise: Scalar, radius_mm: Scalar, above_mm: Scalar) -> Scalar {
    // A seed sits on the surface and a placed contact a layer under it, so a support that
    // much above the place still counts as under it.
    let rise = (rise + above_mm).max(0.0);
    (radius_mm * radius_mm - rise * rise).max(0.0).sqrt()
}
