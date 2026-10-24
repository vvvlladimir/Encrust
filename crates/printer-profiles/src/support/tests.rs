use super::bracing::{BracePattern, BracingPreset};
use super::branching::Branching;
use super::*;
use crate::ProfileError;
use std::path::Path;

fn path() -> &'static Path {
    Path::new("inline.toml")
}

fn toml_of(profile: &SupportProfile) -> String {
    toml::to_string(profile).expect("serialises")
}

#[test]
fn every_shipped_preset_reads_as_valid() {
    let catalogue = crate::Catalogue::bundled().expect("the shipped profiles read");
    for entry in catalogue.supports() {
        let preset = SupportProfile::shipped(&entry.id).expect("a shipped preset reads");
        preset.validate().expect("a shipped preset is valid");
    }
    assert!(
        SupportProfile::shipped(STOCK_SUPPORT).is_ok(),
        "a new object starts with a preset this build ships"
    );
}

#[test]
fn a_profile_that_names_nothing_reads_at_every_fields_default() {
    let parsed = SupportProfile::from_toml_str("", path()).expect("every field has a default");
    assert_eq!(parsed, SupportProfile::unstated());
}

#[test]
fn default_survives_a_toml_round_trip() {
    let original = SupportProfile::default();
    let parsed: SupportProfile = toml::from_str(&toml_of(&original)).expect("parses back");
    assert_eq!(original, parsed);
}

#[test]
fn a_profile_without_facets_takes_the_default() {
    let profile = SupportProfile::from_toml_str(
        r#"
name = "No facets"

[tip]
contact_diameter_mm = 0.4
contact_depth_mm = 0.2

[top]
length_mm = 2.0

[middle]
diameter_mm = 2.0

[bottom]
platform_diameter_mm = 8.0
platform_thickness_mm = 1.0
"#,
        path(),
    )
    .expect("valid profile");
    assert_eq!(profile.facets, 12, "as every shipped preset has");
    assert!((profile.density - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_profile_without_branching_still_branches_by_default() {
    let profile = SupportProfile::from_toml_str(
        r#"
name = "No branching"

[tip]
contact_diameter_mm = 0.4
contact_depth_mm = 0.2

[top]
length_mm = 2.0

[middle]
diameter_mm = 2.0

[bottom]
platform_diameter_mm = 8.0
platform_thickness_mm = 1.0
"#,
        path(),
    )
    .expect("valid profile");
    assert_eq!(profile.branching, Branching::default());
    assert!(profile.branching.enabled);
}

#[test]
fn bracing_written_before_its_zones_is_crossed_all_the_way_up() {
    let profile = SupportProfile::from_toml_str(
        r#"
name = "Braced"

[tip]
contact_diameter_mm = 0.4
contact_depth_mm = 0.0

[top]
length_mm = 2.0

[middle]
diameter_mm = 1.0

[bottom]
platform_diameter_mm = 6.0
platform_thickness_mm = 0.25

[bracing]
enabled = true
diameter_mm = 0.6
max_spacing_mm = 8.0
start_height_mm = 5.0
rise_mm = 8.5
"#,
        path(),
    )
    .expect("valid profile");
    let bracing = profile.bracing;
    assert_eq!((bracing.bottom.levels, bracing.top.levels), (0, 0));
    assert_eq!(bracing.middle, BracePattern::Cross);
}

#[test]
fn a_preset_is_known_by_its_numbers_on_or_off() {
    for preset in BracingPreset::ALL {
        let mut bracing = preset.bracing();
        assert_eq!(BracingPreset::of(&bracing), Some(preset));
        bracing.enabled = false;
        assert_eq!(BracingPreset::of(&bracing), Some(preset));
        bracing.rise_mm += 0.5;
        assert_eq!(BracingPreset::of(&bracing), None);
    }
}

#[test]
fn a_zero_small_pillar_length_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.small_pillar.max_length_mm = 0.0;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "small_pillar max_length_mm",
            ..
        }
    ));
}

#[test]
fn a_small_pillar_without_a_length_takes_the_default() {
    let mut text = toml_of(&SupportProfile::shipped("medium").expect("the medium preset ships"));
    text = text.replace("max_length_mm = 5.0\n", "");
    assert!(!text.contains("max_length_mm"), "the field is left out");
    let parsed = SupportProfile::from_toml_str(&text, path()).expect("still a profile");
    assert!((parsed.small_pillar.max_length_mm - 10.0).abs() < f32::EPSILON);
}

#[test]
fn islands_spaced_out_of_order_or_at_nothing_are_rejected() {
    let mut falling = SupportProfile::shipped("medium").expect("the medium preset ships");
    falling.islands.area_mm2 = [3.2, 2.4];
    let err = SupportProfile::from_toml_str(&toml_of(&falling), path()).unwrap_err();
    assert!(matches!(err, ProfileError::OutOfOrder { .. }));
    let mut nothing = SupportProfile::shipped("medium").expect("the medium preset ships");
    nothing.islands.share[0] = 0.0;
    let err = SupportProfile::from_toml_str(&toml_of(&nothing), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "islands share",
            ..
        }
    ));
}

#[test]
fn a_zero_contact_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.tip.contact_diameter_mm = 0.0;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "contact_diameter_mm",
            ..
        }
    ));
}

#[test]
fn a_contact_may_meet_the_surface_but_not_stand_off_it() {
    let mut flush = SupportProfile::shipped("medium").expect("the medium preset ships");
    flush.tip.contact_depth_mm = 0.0;
    SupportProfile::from_toml_str(&toml_of(&flush), path()).expect("a flush contact loads");

    flush.tip.contact_depth_mm = -0.1;
    let err = SupportProfile::from_toml_str(&toml_of(&flush), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "contact_depth_mm",
            ..
        }
    ));
}

#[test]
fn a_profile_without_a_steep_spacing_takes_the_default() {
    let text = toml_of(&SupportProfile::shipped("medium").expect("the medium preset ships"))
        .lines()
        .filter(|line| !line.starts_with("steep_contact_spacing_mm"))
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = SupportProfile::from_toml_str(&text, path()).expect("still a profile");
    assert!((parsed.steep_contact_spacing_mm - 3.3).abs() < f32::EPSILON);
}

#[test]
fn a_steep_spacing_of_zero_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.steep_contact_spacing_mm = 0.0;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "steep_contact_spacing_mm",
            ..
        }
    ));
}

#[test]
fn a_density_of_zero_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.density = 0.0;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::NonPositive {
            field: "density",
            ..
        }
    ));
}

#[test]
fn an_overhang_angle_past_a_right_angle_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.max_overhang_deg = 95.0;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::OutOfOrder {
            smaller: "max_overhang_deg",
            ..
        }
    ));
}

#[test]
fn a_two_sided_pillar_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.facets = 2;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::TooFew {
            field: "facets",
            ..
        }
    ));
}

#[test]
fn a_contact_wider_than_the_pillar_under_it_is_a_nail_head_not_an_error() {
    let mut nailed = SupportProfile::shipped("medium").expect("the medium preset ships");
    nailed.tip.contact_diameter_mm = nailed.middle.diameter_mm + 1.0;
    SupportProfile::from_toml_str(&toml_of(&nailed), path())
        .expect("a head wider than its pillar is a cone that narrows");
}

#[test]
fn a_profile_written_with_the_widths_a_segment_no_longer_has_still_loads() {
    let old = toml_of(&SupportProfile::shipped("medium").expect("the medium preset ships"))
        .replace(
            "[top]",
            "[top]\nupper_diameter_mm = 0.55\nlower_diameter_mm = 1.2",
        )
        .replace("[bottom]", "[bottom]\nupper_diameter_mm = 1.2");
    let profile = SupportProfile::from_toml_str(&old, path())
        .expect("a profile saved before ADR 0238 names widths that are now derived");
    assert_eq!(
        profile,
        SupportProfile::shipped("medium").expect("the medium preset ships")
    );
}

#[test]
fn a_raft_lifts_the_part_by_its_own_thickness_on_top_of_the_lift() {
    let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    assert!((profile.lift_mm() - profile.z_lift_mm).abs() < f32::EPSILON);
    profile.raft.enabled = true;
    assert!(
        (profile.lift_mm() - profile.z_lift_mm - profile.raft.thickness_mm).abs() < 1e-6,
        "a slab under the feet must not be printed into the part"
    );
}

#[test]
fn a_flare_wider_than_the_pad_it_meets_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.bottom.lower_diameter_mm = broken.bottom.platform_diameter_mm + 1.0;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::OutOfOrder {
            smaller: "bottom lower_diameter_mm",
            ..
        }
    ));
}

#[test]
fn a_foot_narrower_than_the_thickest_trunk_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.bottom.platform_diameter_mm = broken.branching.max_trunk_diameter_mm - 0.5;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::OutOfOrder {
            larger: "platform_diameter_mm",
            ..
        }
    ));
}

#[test]
fn a_trunk_thinner_than_the_pillar_it_carries_is_rejected() {
    let mut broken = SupportProfile::shipped("medium").expect("the medium preset ships");
    broken.branching.max_trunk_diameter_mm = broken.middle.diameter_mm - 0.1;
    let err = SupportProfile::from_toml_str(&toml_of(&broken), path()).unwrap_err();
    assert!(matches!(
        err,
        ProfileError::OutOfOrder {
            smaller: "diameter_mm",
            ..
        }
    ));
}

#[test]
fn malformed_toml_reports_its_path() {
    let err = SupportProfile::from_toml_str("name = ", path()).unwrap_err();
    assert!(matches!(err, ProfileError::Parse { .. }));
}

#[test]
fn the_radii_are_half_the_diameters() {
    let profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    assert!((profile.contact_radius_mm() - 0.3).abs() < f32::EPSILON);
    assert!((profile.pillar_radius_mm() - 0.65).abs() < f32::EPSILON);
    assert!((profile.base_radius_mm() - 4.5).abs() < f32::EPSILON);
}

#[test]
fn a_trunk_is_thicker_than_either_branch_but_thinner_than_their_sum() {
    let branching = Branching::default();
    let merged = branching.merged_radius_mm(0.6, 0.6);
    assert!(
        merged > 0.6 && merged < 1.2,
        "a trunk carrying two 0.6 mm branches is between one of them and both, got \
             {merged}"
    );
}

#[test]
fn a_trunk_never_grows_past_its_cap() {
    let branching = Branching::default();
    let mut radius = 0.6;
    for _ in 0..20 {
        radius = branching.merged_radius_mm(radius, 0.6);
    }
    assert!(
        (radius - branching.max_trunk_diameter_mm / 2.0).abs() < 1e-5,
        "twenty merges must stop at the cap, got {radius}"
    );
}

#[test]
fn a_trunk_base_widens_with_height_up_to_the_cap() {
    let profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    let short = profile.trunk_base_radius_mm(5.0, 1);
    let tall = profile.trunk_base_radius_mm(24.0, 1);
    let towering = profile.trunk_base_radius_mm(200.0, 1);
    assert!(
        (short - profile.pillar_radius_mm()).abs() < 1e-6,
        "5 mm over a slenderness of 15 is under the 1.2 mm pillar, got {short}"
    );
    assert!(
        (tall - 0.8).abs() < 1e-5,
        "24 mm over a slenderness of 15 is 1.6 mm across, got {tall}"
    );
    assert!(
        (towering - profile.branching.max_trunk_diameter_mm / 2.0).abs() < 1e-6,
        "a 200 mm trunk stops at the cap, got {towering}"
    );
}

#[test]
fn a_trunk_base_widens_with_the_tips_it_carries() {
    let profile = SupportProfile::shipped("medium").expect("the medium preset ships");
    let one = profile.trunk_base_radius_mm(1.0, 1);
    let four = profile.trunk_base_radius_mm(1.0, 4);
    assert!(
        four > one,
        "four tips need a wider base than one even on a short trunk: {four} vs {one}"
    );
}

#[test]
fn a_forty_five_degree_branch_covers_as_much_ground_as_it_drops() {
    let branching = Branching {
        max_angle_deg: 45.0,
        ..Branching::default()
    };
    assert!((branching.reach_per_drop() - 1.0).abs() < 1e-5);
}

#[test]
fn an_id_nothing_ships_under_is_unknown() {
    assert!(matches!(
        SupportProfile::shipped("no-such-preset"),
        Err(ProfileError::Unknown { .. })
    ));
}
