use core_geometry::Scalar;
use printer_profiles::SupportProfile;

/// Cell size of the grid layers are read on, millimetres.
///
/// A tenth of a millimetre is four times finer than the thinnest support head and five
/// times finer than the narrowest overhang worth holding, and coarse enough that a layer
/// of a large model is a few hundred rows; see `docs/decisions/0032`.
pub const GRID_PITCH_MM: Scalar = 0.1;

/// How far up the stack an overhang is measured over, millimetres.
///
/// An angle cannot be read off one layer: at 0.05 mm a wall leaning 45 degrees moves
/// 0.05 mm, which is under one cell of the grid. Measuring the same lean over a
/// millimetre of rise turns it into ten cells, which the grid resolves cleanly.
pub const REFERENCE_RISE_MM: Scalar = 1.0;

/// An island narrower than this is a speck of stray geometry, not a part. Supporting it
/// would cost more than losing it: 0.2 mm of bounding radius, which is this across.
pub const MIN_ISLAND_MM: Scalar = 0.4;

/// A layer whose underside is no higher than this stands on the build plate itself,
/// millimetres.
pub const PLATE_CONTACT_MM: Scalar = 1e-3;

/// The lowest density that still means anything. Below it the spacing runs away.
const MIN_DENSITY: Scalar = 0.1;

/// The closest two contacts are ever put, millimetres: under this they are one contact
/// on any panel Encrust writes for.
const MIN_SPACING_MM: Scalar = 0.5;

/// Angles this far from vertical or from a ceiling are clamped: the tangent of a right
/// angle is not a number, and a wall needs no support however it is asked for.
const MIN_OVERHANG_DEG: Scalar = 1.0;
const MAX_OVERHANG_DEG: Scalar = 89.0;

/// What one run of automatic placement measures with, all in millimetres.
///
/// Everything here is derived from the profile: the head's own area says how much island
/// one support carries, the density says how many of them to put down, and the overhang
/// angle says what counts as hanging over nothing at all.
#[derive(Debug, Clone, Copy, PartialEq)]
// Every one of these is a length, and a length without its unit in the name is what
// `AGENTS.md` forbids.
#[allow(clippy::struct_field_names)]
pub struct SampleConfig {
    /// How far apart supports are put down over an overhang. Doubling the profile's
    /// density halves the ground one support covers, not the distance to the next.
    pub spacing_mm: Scalar,
    /// How far a support already standing keeps another one away.
    pub coverage_radius_mm: Scalar,
    /// How far a surface may lean before it needs holding up: the horizontal reach that
    /// [`REFERENCE_RISE_MM`] of rise is allowed to cover.
    pub reach_mm: Scalar,
    /// A piece of overhang narrower and shorter than this is not worth a column of its
    /// own: a ledge of a few cells is a step of the staircase every sloped surface is
    /// printed as.
    pub min_overhang_mm: Scalar,
}

impl SampleConfig {
    /// The measurements a profile asks for.
    ///
    /// The spacing is the profile's own, not the head's: a slicer that widens a contact
    /// to hold better must not thin the lattice out by doing it (ADR 0131). The two
    /// constants, 2.9 and 1.3, come from measurements of how much a single head of a
    /// given area can hold: a 0.4 mm head carries about 1.65 mm of island, a 0.5 mm head
    /// about 1.85 mm. That carry is what says when a ledge is too
    /// small to bother with.
    pub fn for_profile(profile: &SupportProfile) -> Self {
        let head_radius_mm = profile.contact_radius_mm();
        let head_area = std::f32::consts::PI * head_radius_mm * head_radius_mm;
        let carry_mm = head_area.mul_add(2.9, 1.3);

        // Density counts supports over an area, not along a line, so it divides the
        // spacing by its own root. See `docs/decisions/0081`.
        let density = profile.density.max(MIN_DENSITY);
        let spacing_mm = profile.contact_spacing_mm.max(MIN_SPACING_MM) / density.sqrt();
        let lean = profile
            .max_overhang_deg
            .clamp(MIN_OVERHANG_DEG, MAX_OVERHANG_DEG)
            .to_radians();

        Self {
            spacing_mm,
            // Three quarters of the spacing: a support put down on this layer suppresses
            // the same lattice point on the next one, and leaves the next point along
            // free. It is a radius in three dimensions, so it is also how far up a
            // support holds; see `docs/decisions/0079`.
            coverage_radius_mm: spacing_mm * 0.75,
            reach_mm: REFERENCE_RISE_MM * lean.tan(),
            // Half of what one head carries: a ledge that small is inside the head that
            // would be put under it.
            min_overhang_mm: carry_mm / 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lattice_is_spaced_the_way_the_profile_asks_for() {
        let profile = crate::tests::profile();
        let config = SampleConfig::for_profile(&profile);
        assert!(
            (config.spacing_mm - profile.contact_spacing_mm).abs() < 1e-4,
            "at the default density the spacing is the profile's own, got {}",
            config.spacing_mm
        );
    }

    #[test]
    fn a_wider_head_does_not_thin_the_lattice_out() {
        let profile = crate::tests::profile();
        let fat = SampleConfig::for_profile(&SupportProfile {
            tip: printer_profiles::TipSegment {
                contact_diameter_mm: profile.tip.contact_diameter_mm * 2.0,
                ..profile.tip
            },
            ..profile.clone()
        });
        let thin = SampleConfig::for_profile(&profile);
        assert!(
            (fat.spacing_mm - thin.spacing_mm).abs() < 1e-4,
            "the head says what a ledge is too small for, not how far apart the lattice \
             stands; see `docs/decisions/0131`"
        );
        assert!(
            fat.min_overhang_mm > thin.min_overhang_mm,
            "a wider head swallows a wider ledge"
        );
    }

    #[test]
    fn twice_the_density_puts_twice_as_many_supports_on_the_same_area() {
        let normal = SampleConfig::for_profile(&crate::tests::profile());
        let dense = SampleConfig::for_profile(&SupportProfile {
            density: 2.0,
            ..crate::tests::profile()
        });

        // Twice as many over an area is the spacing over the root of two, which is what
        // makes the density a count per square millimetre.
        let over_an_area = (normal.spacing_mm / dense.spacing_mm).powi(2);
        assert!(
            (over_an_area - 2.0).abs() < 1e-3,
            "expected twice the supports per area, got {over_an_area} times as many"
        );
        assert!(dense.coverage_radius_mm < normal.coverage_radius_mm);
    }

    #[test]
    fn a_density_of_zero_is_clamped_instead_of_dividing_by_it() {
        let config = SampleConfig::for_profile(&SupportProfile {
            density: 0.0,
            ..crate::tests::profile()
        });
        assert!(config.spacing_mm.is_finite() && config.spacing_mm > 0.0);
    }

    #[test]
    fn forty_five_degrees_lets_a_surface_move_its_own_rise() {
        let config = SampleConfig::for_profile(&SupportProfile {
            max_overhang_deg: 45.0,
            ..crate::tests::profile()
        });
        assert!(
            (config.reach_mm - REFERENCE_RISE_MM).abs() < 1e-4,
            "a 45 degree lean covers as much ground as it climbs, got {}",
            config.reach_mm
        );
    }

    #[test]
    fn a_shallower_angle_is_stricter_about_what_hangs_over_nothing() {
        let strict = SampleConfig::for_profile(&SupportProfile {
            max_overhang_deg: 30.0,
            ..crate::tests::profile()
        });
        let loose = SampleConfig::for_profile(&SupportProfile {
            max_overhang_deg: 60.0,
            ..crate::tests::profile()
        });
        assert!(
            strict.reach_mm < loose.reach_mm,
            "a surface allowed to lean further may cover more ground before it is held"
        );
    }

    #[test]
    fn an_angle_at_a_right_angle_stays_a_number() {
        let flat = SampleConfig::for_profile(&SupportProfile {
            max_overhang_deg: 90.0,
            ..crate::tests::profile()
        });
        assert!(flat.reach_mm.is_finite(), "got {}", flat.reach_mm);
    }

    #[test]
    fn the_shipped_presets_grow_stricter_as_they_grow_heavier() {
        let light = SampleConfig::for_profile(&SupportProfile::light());
        let heavy = SampleConfig::for_profile(&SupportProfile::heavy());

        assert!(heavy.spacing_mm < light.spacing_mm, "heavier is denser");
        assert!(heavy.reach_mm < light.reach_mm, "heavier is stricter");
    }
}
