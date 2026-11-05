use core_geometry::{Vec3, cuboid, diagnose};

use super::*;

fn number(metrics: &Metrics, metric: Metric) -> Scalar {
    match metrics.get(metric) {
        Some(Value::Number(number)) => number,
        other => panic!("{} should be a number, got {other:?}", metric.name()),
    }
}

fn flag(metrics: &Metrics, metric: Metric) -> bool {
    match metrics.get(metric) {
        Some(Value::Bool(flag)) => flag,
        other => panic!("{} should be a flag, got {other:?}", metric.name()),
    }
}

fn measured(mesh: &Mesh, transform: &Transform, hollow: bool) -> Metrics {
    let diagnostics = diagnose(mesh);
    let model = Model {
        mesh,
        transform,
        diagnostics: &diagnostics,
        turned: false,
        hollow,
        fits: true,
    };
    measure(&model, 2.0)
}

fn near(actual: Scalar, expected: Scalar, what: &str) {
    assert!(
        (actual - expected).abs() < 0.01 * expected.abs().max(1.0),
        "{what}: expected {expected}, got {actual}"
    );
}

#[test]
fn a_forty_millimetre_cube_measures_to_its_closed_form() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(40.0));
    let metrics = measured(&cube, &Transform::default(), false);
    for dimension in [Metric::HeightMm, Metric::WidthMm, Metric::DepthMm] {
        near(number(&metrics, dimension), 40.0, dimension.name());
    }
    near(
        number(&metrics, Metric::VolumeMl),
        64.0,
        "volume_ml, 40³ mm³",
    );
    near(
        number(&metrics, Metric::SurfaceCm2),
        96.0,
        "surface_cm2, 6 x 40² mm²",
    );
    near(number(&metrics, Metric::Faces), 12.0, "faces");
    near(number(&metrics, Metric::Defects), 0.0, "defects");
    near(
        number(&metrics, Metric::SavesMl),
        (64_000.0 - 9_600.0 * 2.0) / 1000.0,
        "saves_ml, 64000 - 9600 x 2 mm³",
    );
    near(
        number(&metrics, Metric::SavesShare),
        44_800.0 / 64_000.0,
        "saves_share",
    );
    assert!(flag(&metrics, Metric::Sound), "a closed cube is sound");
    assert!(
        flag(&metrics, Metric::WorthHollowing),
        "44.8 ml is 70 % of the cube"
    );
    assert!(flag(&metrics, Metric::Fits) && !flag(&metrics, Metric::Turned));
    assert!(!flag(&metrics, Metric::Hollow));
}

#[test]
fn a_thin_plate_is_not_worth_hollowing() {
    let plate = cuboid(Vec3::ZERO, Vec3::new(60.0, 60.0, 3.0));
    let metrics = measured(&plate, &Transform::default(), false);
    assert!(
        !flag(&metrics, Metric::WorthHollowing),
        "a 3 mm plate is all wall"
    );
    near(
        number(&metrics, Metric::SavesMl),
        0.0,
        "saves_ml floored at 0",
    );
}

#[test]
fn a_placement_scaled_fourfold_measures_the_larger_body() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(10.0));
    let placed = Transform {
        scale: Vec3::splat(4.0),
        ..Transform::default()
    };
    let metrics = measured(&cube, &placed, false);
    near(number(&metrics, Metric::WidthMm), 40.0, "width_mm");
    near(number(&metrics, Metric::VolumeMl), 64.0, "volume_ml");
    near(number(&metrics, Metric::SurfaceCm2), 96.0, "surface_cm2");
    assert!(
        flag(&metrics, Metric::WorthHollowing),
        "placed, it is the 40 mm cube"
    );
}

#[test]
fn an_open_cube_is_unsound_with_defects_and_never_worth_hollowing() {
    let mut open = cuboid(Vec3::ZERO, Vec3::splat(40.0));
    open.faces.pop();
    let metrics = measured(&open, &Transform::default(), false);
    assert!(
        !flag(&metrics, Metric::Sound),
        "a missing face leaves a hole"
    );
    assert!(
        number(&metrics, Metric::Defects) > 0.0,
        "the hole is a defect"
    );
    assert!(!flag(&metrics, Metric::WorthHollowing));
}

#[test]
fn a_hollow_model_is_not_worth_hollowing_again() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(40.0));
    let metrics = measured(&cube, &Transform::default(), true);
    assert!(flag(&metrics, Metric::Hollow));
    assert!(!flag(&metrics, Metric::WorthHollowing));
}

#[test]
fn an_empty_mesh_measures_to_zero_without_a_share() {
    let metrics = measured(&Mesh::default(), &Transform::default(), false);
    assert_eq!(number(&metrics, Metric::SavesShare), 0.0);
    assert_eq!(number(&metrics, Metric::HeightMm), 0.0);
}

#[test]
fn every_name_reads_back_to_its_metric() {
    for metric in Metric::ALL {
        assert_eq!(Metric::from_name(metric.name()), Some(metric));
    }
    assert_eq!(Metric::from_name("nonsense"), None);
}

#[test]
fn what_a_step_measures_later_is_left_unset_at_the_start() {
    let cube = cuboid(Vec3::ZERO, Vec3::splat(40.0));
    let metrics = measured(&cube, &Transform::default(), false);
    for metric in Metric::ALL {
        assert_eq!(
            metrics.get(metric).is_none(),
            !metric.known_after().is_empty(),
            "{} is set exactly when known from the start",
            metric.name()
        );
        if let Some(value) = metrics.get(metric) {
            assert_eq!(
                matches!(value, Value::Bool(_)),
                metric.kind() == Kind::Bool,
                "{} holds its kind",
                metric.name()
            );
        }
    }
}
