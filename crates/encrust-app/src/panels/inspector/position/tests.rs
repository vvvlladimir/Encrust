use super::*;

#[test]
fn a_turn_is_about_the_plates_axes_or_the_models_own() {
    let lying = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let turn = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    // Laid on its back, the model's own Z points along the plate's -Y.
    let own = about_plate(lying, turn, Axes::Local) * lying * Vec3::Z;
    assert!(
        own.abs_diff_eq(lying * Vec3::Z, 1e-5),
        "a turn about its own Z spins it in place"
    );
    let plate = about_plate(lying, turn, Axes::Global) * lying * Vec3::Z;
    assert!(
        plate.abs_diff_eq(Vec3::X, 1e-5),
        "a turn about the plate's Z swings -Y round to X, got {plate:?}"
    );
}

#[test]
fn a_local_move_steps_along_the_models_own_axes() {
    let pivot = Transform::from_scale_rotation_translation(
        Vec3::ONE,
        Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
        Vec3::new(10.0, 20.0, 5.0),
    );
    let typed = Vec3::new(10.0, 0.0, 0.0);
    assert_eq!(moved_to(pivot, typed, Axes::Global), typed);
    let local = moved_to(pivot, typed, Axes::Local);
    assert!(
        local.abs_diff_eq(Vec3::new(10.0, 30.0, 5.0), 1e-4),
        "turned a quarter about Z, its own X is the plate's Y: 10 mm on from (10, 20, 5), got {local:?}"
    );
}

#[test]
fn a_local_rotation_turns_by_the_angle_about_the_models_own_axis() {
    let lying = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let turn = typed_turn(lying, Vec3::new(0.0, 0.0, 90.0), Axes::Local);
    let own_z = turn * lying * Vec3::Z;
    assert!(
        own_z.abs_diff_eq(lying * Vec3::Z, 1e-5),
        "a turn about its own Z leaves that axis where it was, got {own_z:?}"
    );
    let own_x = turn * lying * Vec3::X;
    assert!(
        own_x.abs_diff_eq(lying * Vec3::Y, 1e-5),
        "90 degrees about its own Z swings its X onto its Y, got {own_x:?}"
    );
    let global = typed_turn(lying, Vec3::new(0.0, 0.0, 90.0), Axes::Global) * lying;
    assert!(
        global.abs_diff_eq(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), 1e-5),
        "on the plate the fields set the angles outright, got {global:?}"
    );
}

fn frame_with_button(ctx: &egui::Context, pressed: bool, run: impl FnMut(&mut egui::Ui)) {
    let events = vec![egui::Event::PointerButton {
        pos: egui::pos2(10.0, 10.0),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }];
    let input = egui::RawInput {
        events,
        ..Default::default()
    };
    let mut run = run;
    ctx.run_ui(input, |ui| run(ui)).textures_delta.clear();
}

#[test]
fn a_local_step_shows_while_held_and_falls_back_to_zero_on_release() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("step");
    let step = Vec3::new(2.5, 0.0, 0.0);
    frame_with_button(&ctx, true, |ui| keep_step(ui, id, step));
    let mut held = Vec3::ZERO;
    frame_with_button(&ctx, true, |ui| held = held_step(ui, id));
    assert_eq!(
        held, step,
        "the dragged figure stays in the field while the button is down"
    );
    frame_with_button(&ctx, false, |ui| held = held_step(ui, id));
    assert_eq!(held, Vec3::ZERO, "let go, the field reads zero again");
    frame_with_button(&ctx, true, |ui| held = held_step(ui, id));
    assert_eq!(
        held,
        Vec3::ZERO,
        "the next press starts from zero, not the last step"
    );
}

fn slanted() -> Transform {
    Transform::from_scale_rotation_translation(
        Vec3::new(1.0, 2.0, -1.0),
        Quat::from_rotation_x(0.7),
        Vec3::ONE,
    )
}

#[test]
fn a_global_ratio_stretches_the_model_along_the_plate() {
    let pivot = slanted();
    let ratio = pivot.plate_factors();
    let stretched = grown(pivot, ratio, 2, 1.5, false, Axes::Global);
    assert!(
        stretched
            .plate_factors()
            .abs_diff_eq(ratio * Vec3::new(1.0, 1.0, 1.5), 1e-5),
        "the plate's Z reads half as long again and X and Y as before, got {:?}",
        stretched.plate_factors()
    );
    assert!(stretched.is_mirrored(), "a stretch keeps a mirror");
}

#[test]
fn a_local_ratio_stretches_the_models_own_axis() {
    let pivot = slanted();
    let stretched = grown(pivot, pivot.scale.abs(), 1, 2.0, false, Axes::Local);
    assert_eq!(stretched.scale, Vec3::new(1.0, 4.0, -1.0));
    assert_eq!(stretched.stretch, Mat3::IDENTITY);
}

#[test]
fn linked_sizes_grow_all_three_alike_and_never_to_nothing() {
    let pivot = slanted();
    let grown_up = grown(pivot, pivot.scale.abs(), 0, 3.0, true, Axes::Global);
    assert_eq!(grown_up.scale, Vec3::new(3.0, 6.0, -3.0));
    let crushed = grown(pivot, pivot.scale.abs(), 0, 0.0, true, Axes::Local);
    assert!(
        (crushed.scale.abs().min_element() - MIN_SCALE).abs() < 1e-7,
        "the shortest axis stops at the smallest scale, got {:?}",
        crushed.scale
    );
}

#[test]
fn a_new_size_keeps_the_axes_a_mirror_flipped() {
    let mirrored = Vec3::new(-1.0, 1.0, -2.0);
    assert_eq!(
        keeping_mirror(mirrored, Vec3::new(1.5, 1.5, 3.0)),
        Vec3::new(-1.5, 1.5, -3.0)
    );
    assert_eq!(
        keeping_mirror(mirrored, Vec3::ZERO),
        Vec3::new(-MIN_SCALE, MIN_SCALE, -MIN_SCALE),
        "a size of nothing is the smallest one, still mirrored"
    );
}
