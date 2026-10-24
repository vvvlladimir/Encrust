use super::*;

#[test]
fn a_flatter_band_of_overhang_is_held_closer() {
    let mut profile = crate::fixtures::profile();
    profile.steep_contact_spacing_mm = 8.0;
    let config = SampleConfig::for_profile(&profile);
    let spacings: Vec<Scalar> = (0..LEAN_BANDS)
        .map(|band| config.band_spacing_mm(band))
        .collect();
    assert!(spacings.windows(2).all(|pair| pair[1] < pair[0]));
    assert!(
        (spacings[LEAN_BANDS - 1] - config.spacing_mm).abs() < 1e-5,
        "the flattest band is a face looking down, held at the spacing itself"
    );
    assert!(
        (config.band_lean_rad(1).to_degrees() - 56.25).abs() < 1e-3,
        "a quarter of the way from the 45 degree angle to level"
    );
}

#[test]
fn a_steep_spacing_under_the_spacing_is_the_spacing() {
    let mut profile = crate::fixtures::profile();
    profile.steep_contact_spacing_mm = 1.0;
    let config = SampleConfig::for_profile(&profile);
    assert!(!config.spacing_leans());
    assert!((config.band_spacing_mm(0) - config.spacing_mm).abs() < 1e-5);
}

#[test]
fn an_island_is_spaced_as_its_profile_says() {
    let mut profile = crate::fixtures::profile();
    profile.islands = IslandSpacing {
        area_mm2: [10.0, 20.0],
        share: [0.5, 0.75, 1.0],
    };
    let config = SampleConfig::for_profile(&profile);
    assert!(
        (config.island_spacing_mm(5.0) - config.spacing_mm * 0.5).abs() < 1e-5,
        "a 5 mm² island is under the profile's first 10 mm², so held at half the spacing"
    );
    assert!(
        (config.island_spacing_mm(15.0) - config.spacing_mm * 0.75).abs() < 1e-5,
        "a 15 mm² island reaches the first threshold alone"
    );
}

#[test]
fn a_small_island_is_held_closer_than_a_large_one() {
    let config = SampleConfig::for_profile(&crate::fixtures::profile());
    let (small, middling, large) = (
        config.island_spacing_mm(1.0),
        config.island_spacing_mm(2.8),
        config.island_spacing_mm(100.0),
    );
    assert!(small < middling && middling < large);
    assert!(
        (large - config.spacing_mm).abs() < 1e-5,
        "a large island is held at the spacing itself"
    );
}

#[test]
fn the_lattice_is_spaced_the_way_the_profile_asks_for() {
    let profile = crate::fixtures::profile();
    let config = SampleConfig::for_profile(&profile);
    assert!(
        (config.spacing_mm - profile.contact_spacing_mm).abs() < 1e-4,
        "at the default density the spacing is the profile's own, got {}",
        config.spacing_mm
    );
}

#[test]
fn a_wider_head_does_not_thin_the_lattice_out() {
    let profile = crate::fixtures::profile();
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
    let normal = SampleConfig::for_profile(&crate::fixtures::profile());
    let dense = SampleConfig::for_profile(&SupportProfile {
        density: 2.0,
        ..crate::fixtures::profile()
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
        ..crate::fixtures::profile()
    });
    assert!(config.spacing_mm.is_finite() && config.spacing_mm > 0.0);
}

#[test]
fn forty_five_degrees_lets_a_surface_move_its_own_rise() {
    let config = SampleConfig::for_profile(&SupportProfile {
        max_overhang_deg: 45.0,
        ..crate::fixtures::profile()
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
        ..crate::fixtures::profile()
    });
    let loose = SampleConfig::for_profile(&SupportProfile {
        max_overhang_deg: 60.0,
        ..crate::fixtures::profile()
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
        ..crate::fixtures::profile()
    });
    assert!(flat.reach_mm.is_finite(), "got {}", flat.reach_mm);
}

#[test]
fn every_shipped_preset_lifts_a_part_far_enough_for_its_own_support() {
    for profile in [
        SupportProfile::shipped("light").expect("the light preset ships"),
        SupportProfile::shipped("medium").expect("the medium preset ships"),
        SupportProfile::shipped("heavy").expect("the heavy preset ships"),
    ] {
        let flare_mm = profile.flare_lower_radius_mm() - profile.pillar_radius_mm();
        let needed_mm = profile.bottom.platform_thickness_mm
            + flare_mm
            + profile.top.length_mm
            + crate::column::MIN_PILLAR_HEIGHT_MM;
        assert!(
            needed_mm <= profile.z_lift_mm + 1e-4,
            "{}: a pad, a flare, a top segment and the shortest pillar take {needed_mm} mm \
                 and the lift leaves {} mm",
            profile.name,
            profile.z_lift_mm
        );
    }
}
