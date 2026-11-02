use core_geometry::{Aabb, Scalar, Vec2, Vec3};

/// How far past the build volume counts as past it, millimetres. Under a micron is float dust
/// from the placement, not a model hanging off the plate.
const PAST_THE_PLATE_MM: Scalar = 1.0e-3;

/// How what stands on the plate, a model with its supports and raft, sits in the build volume.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PlateFit {
    /// How much larger than the build volume it is, per axis, mm.
    pub overflow_mm: Vec3,
    /// How far it reaches outside the build volume where it stands, per axis, mm; whatever
    /// does is clipped off the masks.
    pub outside_mm: Vec3,
    /// How far it reaches into the plate's inset along X and Y, mm; it still slices whole.
    pub inset_mm: Vec2,
}

impl PlateFit {
    /// How `bounds`, plate mm, sit in a build volume `volume_mm` whose plate is clear of its
    /// inset between the corners `usable_mm`.
    pub fn of(bounds: &Aabb, volume_mm: Vec3, usable_mm: [Vec2; 2]) -> Self {
        let [low, high] = usable_mm;
        let inset = (low - bounds.mins.truncate()).max(bounds.maxs.truncate() - high);
        Self {
            overflow_mm: past(bounds.maxs - bounds.mins - volume_mm),
            outside_mm: past((-bounds.mins).max(bounds.maxs - volume_mm)),
            inset_mm: past(inset.extend(0.0)).truncate(),
        }
    }

    /// Whether all of it prints: nothing past the build volume.
    pub fn fits(&self) -> bool {
        self.overflow_mm == Vec3::ZERO && self.outside_mm == Vec3::ZERO
    }

    /// Whether it reaches into the plate's inset, which a maker keeps clear.
    pub fn in_inset(&self) -> bool {
        self.inset_mm != Vec2::ZERO
    }
}

/// What is left of `amount` once the axes inside the volume, and the dust, are dropped.
fn past(amount: Vec3) -> Vec3 {
    Vec3::select(
        amount.cmpgt(Vec3::splat(PAST_THE_PLATE_MM)),
        amount,
        Vec3::ZERO,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VOLUME: Vec3 = Vec3::splat(20.0);
    const WHOLE_PLATE: [Vec2; 2] = [Vec2::ZERO, Vec2::splat(20.0)];

    fn fit_of(low: Vec3, high: Vec3) -> PlateFit {
        PlateFit::of(&Aabb::new(low, high), VOLUME, WHOLE_PLATE)
    }

    #[test]
    fn a_box_inside_the_volume_fits() {
        let fit = fit_of(Vec3::splat(5.0), Vec3::splat(15.0));
        assert!(fit.fits() && !fit.in_inset(), "got {fit:?}");
    }

    #[test]
    fn a_box_too_big_names_by_how_much_on_each_axis() {
        let fit = fit_of(Vec3::ZERO, Vec3::new(30.0, 10.0, 25.0));
        assert!(!fit.fits());
        assert_eq!(
            fit.overflow_mm,
            Vec3::new(10.0, 0.0, 5.0),
            "30 - 20 and 25 - 20 mm"
        );
    }

    #[test]
    fn a_box_small_enough_but_off_the_edge_does_not_fit() {
        let fit = fit_of(Vec3::new(-3.0, 5.0, 0.0), Vec3::new(7.0, 15.0, 10.0));
        assert!(
            !fit.fits(),
            "what hangs off the plate is clipped off the masks"
        );
        assert_eq!(fit.overflow_mm, Vec3::ZERO);
        assert_eq!(
            fit.outside_mm,
            Vec3::new(3.0, 0.0, 0.0),
            "3 mm short of x = 0"
        );
    }

    #[test]
    fn a_box_in_the_inset_fits_and_says_how_far_in() {
        let usable = [Vec2::new(1.5, 0.0), Vec2::splat(20.0)];
        let bounds = Aabb::new(Vec3::new(0.5, 5.0, 0.0), Vec3::new(10.5, 15.0, 10.0));
        let fit = PlateFit::of(&bounds, VOLUME, usable);
        assert!(fit.fits(), "the inset warns, it never clips");
        assert_eq!(fit.inset_mm, Vec2::new(1.0, 0.0), "1.5 - 0.5 mm along X");
    }

    #[test]
    fn float_dust_under_the_plate_is_not_a_box_off_it() {
        let fit = fit_of(Vec3::new(0.0, 0.0, -1.0e-5), Vec3::splat(10.0));
        assert!(fit.fits(), "{} mm is placement dust", 1.0e-5);
    }
}
