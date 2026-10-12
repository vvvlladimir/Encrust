use core_geometry::glam::EulerRot;
use core_geometry::{Quat, Transform, Vec3};
use core_plate::OrientGoal;

use crate::gizmo::Axes;
use crate::import::recenter;
use crate::panels::Window;
use crate::plate::BuildPlate;
use crate::scene::{Axis, ObjectId, Scene, SceneObject};
use crate::ui::{
    Segment, Segmented, axis_label, compact_button, describe, field_label, hairline, icon,
    icon_toggle, nested, number_field, number_row, progress_bar, secondary_button, section,
    section_with_action, switch, theme,
};

/// Millimetres, degrees and percent per point of drag on the transform fields.
const POSITION_DRAG: f64 = 0.1;
const ANGLE_DRAG: f64 = 0.5;
const PERCENT_DRAG: f64 = 0.5;
/// The column the axis letters and their units stand in, in every block, so the fields
/// of all three line up.
const AXIS_W: f32 = 46.0;
/// Width of a ±45° button.
const TURN_W: f32 = 48.0;
/// What a turn button adds about its axis, degrees.
const TURN_DEG: f32 = 45.0;

/// A scale this small is indistinguishable from a flattened object, which can be neither
/// picked nor sliced.
const MIN_SCALE: f32 = 1e-3;

/// The tilt a limit starts at once it is switched on, degrees.
const DEFAULT_TILT_DEG: f32 = 45.0;

/// The highest a model may be lifted off the plate by typing, millimetres.
const MAX_LIFT_MM: f32 = 100.0;

/// The gap between a model and the mirrored copy stood beside it, millimetres.
const COPY_GAP_MM: f32 = 5.0;

/// How near a face's normal has to be to an axis to be named for it alone: within about
/// eighteen degrees.
const STRAIGHT: f32 = 0.95;

/// Position and angle take any value.
const ANY: std::ops::RangeInclusive<f32> = f32::MIN..=f32::MAX;
/// What a screen reader calls each axis's boxes.
const AXIS_NAMES: [&str; 3] = ["X", "Y", "Z"];

/// Where the selected model stands, in the same numbers the gizmo drags: its pivot, not
/// the origin its file was authored around. See `docs/decisions/0110`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some(id) = window.doc.scene.selected() else {
        super::select::nothing_picked(ui, window.doc);
        return;
    };
    axes(ui, &mut window.view.gizmo.axes);
    let axes = window.view.gizmo.axes;
    position(ui, &mut window.doc.scene, &window.doc.plate, id);
    rotation(ui, window, id, axes);
    scale(ui, &mut window.doc.scene, id, axes);
    place(ui, &mut window.doc.scene, &window.doc.plate, id);
    orient(ui, window, id);
    lay_flat(ui, window, id);
    mirror(ui, &mut window.doc.scene);
}

/// Whether the handles, the turn buttons and the sizes follow the plate's axes or the
/// model's own. Where the model stands and how it is turned are read on the plate's either way.
fn axes(ui: &mut egui::Ui, axes: &mut Axes) {
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            let segments = [
                Segment::new(Axes::Global, "Global"),
                Segment::new(Axes::Local, "Local"),
            ];
            Segmented::new(&segments)
                .width(ui.available_width())
                .show(ui, axes);
        });
    hairline(ui);
}

fn position(ui: &mut egui::Ui, scene: &mut Scene, plate: &BuildPlate, id: ObjectId) {
    let tooltip = "Back to the middle of the plate";
    let (reset, _) = section_with_action(ui, "Move", (icon::RESET, tooltip), |ui| {
        let Some(object) = scene.get_mut(id) else {
            return;
        };
        let mut pivot = object.pivot();
        let mut changed = false;
        for (axis, axis_name) in AXIS_NAMES.iter().enumerate() {
            ui.horizontal(|ui| {
                label_column(ui, axis, "mm");
                let width = ui.available_width();
                let value = &mut pivot.translation[axis];
                let name = format!("Move along {axis_name}");
                changed |= number_field(ui, &name, value, POSITION_DRAG, ANY, Some(2), width);
            });
        }
        // Written back only on a frame the user changed a number: settling every frame
        // would let the round trip through the centre of mass accumulate.
        if changed {
            object.settle(pivot);
        }
    });
    if reset {
        recentre(scene, plate, id);
    }
}

/// Euler angles are written back only on the frames the user actually changed one.
/// Round-tripping the quaternion every frame would let rounding accumulate, and near
/// gimbal lock the three numbers would drift on their own.
fn rotation(ui: &mut egui::Ui, window: &mut Window, id: ObjectId, axes: Axes) {
    let tooltip = "Back to the way the model was imported";
    let (reset, _) = section_with_action(ui, "Rotation", (icon::RESET, tooltip), |ui| {
        if let Some(object) = window.doc.scene.get_mut(id) {
            let mut pivot = object.pivot();
            if rotation_rows(ui, &mut pivot, axes) {
                object.settle(pivot);
            }
        }
    });
    if reset && let Some(object) = window.doc.scene.get_mut(id) {
        object.settle(Transform {
            rotation: Quat::IDENTITY,
            ..object.pivot()
        });
        object.stand_on_plate();
    }
}

fn rotation_rows(ui: &mut egui::Ui, pivot: &mut Transform, axes: Axes) -> bool {
    let (x, y, z) = pivot.rotation.to_euler(EulerRot::XYZ);
    // Rounded to what the field shows, plus zero, so float dust never reads as "-0.00".
    let shown = |radians: f32| (radians.to_degrees() * 100.0).round() / 100.0 + 0.0;
    let mut degrees = Vec3::new(shown(x), shown(y), shown(z));
    let mut typed = false;
    let mut turn = Quat::IDENTITY;
    for axis in 0..3 {
        ui.horizontal(|ui| {
            label_column(ui, axis, "°");
            let spacing = ui.spacing().item_spacing.x;
            let width = ui.available_width() - 2.0 * (TURN_W + spacing);
            let degrees = &mut degrees[axis];
            let name = format!("Rotation about {}", AXIS_NAMES[axis]);
            typed |= number_field(ui, &name, degrees, ANGLE_DRAG, ANY, Some(2), width);
            for sign in [-1.0, 1.0] {
                let text = format!("{:+}°", sign * TURN_DEG);
                let clicked = compact_button(ui, &text, TURN_W).clicked();
                if clicked {
                    let about = Vec3::AXES[axis];
                    turn = Quat::from_axis_angle(about, (sign * TURN_DEG).to_radians()) * turn;
                }
            }
        });
    }
    if typed {
        pivot.rotation = Quat::from_euler(
            EulerRot::XYZ,
            degrees.x.to_radians(),
            degrees.y.to_radians(),
            degrees.z.to_radians(),
        );
    }
    let turned = turn != Quat::IDENTITY;
    if turned {
        pivot.rotation = turned_by(pivot.rotation, turn, axes);
    }
    typed || turned
}

/// `rotation` with `turn` applied about the plate's axes or about the model's own.
fn turned_by(rotation: Quat, turn: Quat, axes: Axes) -> Quat {
    match axes {
        Axes::Global => turn * rotation,
        Axes::Local => rotation * turn,
    }
    .normalize()
}

/// The model's size along the plate's axes or its own, and its stretch on each of its own
/// axes as a percentage of the imported size.
fn scale(ui: &mut egui::Ui, scene: &mut Scene, id: ObjectId, axes: Axes) {
    let linked_id = ui.id().with("scale-linked");
    let mut linked = ui.memory(|memory| memory.data.get_temp(linked_id).unwrap_or(true));
    let tooltip = "Back to the size the model was imported at";
    let (reset, _) = section_with_action(ui, "Scale", (icon::RESET, tooltip), |ui| {
        if let Some(object) = scene.get_mut(id) {
            scale_rows(ui, object, &mut linked, axes);
        }
    });
    ui.memory_mut(|memory| memory.data.insert_temp(linked_id, linked));
    if reset && let Some(object) = scene.get_mut(id) {
        let pivot = object.pivot();
        object.settle(Transform {
            scale: keeping_mirror(pivot.scale, Vec3::ONE),
            ..pivot
        });
        object.stand_on_plate();
    }
}

fn scale_rows(ui: &mut egui::Ui, object: &mut SceneObject, linked: &mut bool, axes: Axes) {
    let mut pivot = object.pivot();
    let mut factors = pivot.scale.abs();
    let sizes = match (axes, object.world_bounds()) {
        (Axes::Global, Some(bounds)) => Sizes::OnThePlate(bounds.maxs - bounds.mins),
        _ => Sizes::OwnAxes(
            object
                .mesh
                .aabb()
                .map_or(Vec3::ONE, |bounds| bounds.maxs - bounds.mins),
        ),
    };
    let column = scale_header(ui, linked);
    let mut changed = false;
    for axis in 0..3 {
        ui.horizontal(|ui| {
            label_column(ui, axis, "");
            changed |= scale_row(ui, &mut factors, sizes, axis, *linked, column);
        });
    }
    if changed {
        pivot.scale = keeping_mirror(pivot.scale, factors);
        object.settle(pivot);
    }
}

/// What the Size column reads.
#[derive(Clone, Copy)]
enum Sizes {
    /// Along the model's own axes, from this unscaled extent, millimetres: what it measures
    /// before it is turned, so a size typed here stays put however it is turned later.
    OwnAxes(Vec3),
    /// Along the plate's axes, as it stands now, millimetres. Stretching a turned model
    /// along one of them would shear it, so a new one grows all three in proportion.
    OnThePlate(Vec3),
}

/// The two column titles and the link that keeps the three axes in proportion. Returns
/// the width each column's field gets.
fn scale_header(ui: &mut egui::Ui, linked: &mut bool) -> f32 {
    let spacing = ui.spacing().item_spacing.x;
    let column = (ui.available_width() - AXIS_W - 2.0 * spacing) / 2.0;
    ui.horizontal(|ui| {
        ui.add_space(AXIS_W + spacing);
        caption(ui, "Size", "mm", column);
        caption(ui, "Ratio", "%", column - theme::ICON_SIZE - spacing);
        let tooltip = "Keep the proportions";
        if icon_toggle(ui, icon::LINKED, tooltip, *linked).clicked() {
            *linked = !*linked;
        }
    });
    column
}

fn caption(ui: &mut egui::Ui, text: &str, unit: &str, width: f32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width.max(0.0), theme::ICON_SIZE),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width.max(0.0));
            field_label(ui, text, theme::colors().text_mid, unit);
        },
    );
}

/// One axis as a size and as a percentage of the imported size. With the link on, a
/// change to either stretches the other two axes by the same factor.
fn scale_row(
    ui: &mut egui::Ui,
    factors: &mut Vec3,
    sizes: Sizes,
    axis: usize,
    linked: bool,
    width: f32,
) -> bool {
    let before = factors[axis];
    let (shown_mm, on_the_plate) = match sizes {
        Sizes::OwnAxes(extent) => (extent[axis] * before, false),
        Sizes::OnThePlate(size) => (size[axis], true),
    };
    let mut size_mm = shown_mm;
    let mut percent = before * 100.0;

    let size_name = format!("{} size", AXIS_NAMES[axis]);
    let least_mm = shown_mm / before * MIN_SCALE;
    let sized = number_field(
        ui,
        &size_name,
        &mut size_mm,
        POSITION_DRAG,
        least_mm..=f32::MAX,
        Some(2),
        width,
    );
    let ratio_name = format!("{} ratio", AXIS_NAMES[axis]);
    let percents = MIN_SCALE * 100.0..=f32::MAX;
    let ratioed = number_field(
        ui,
        &ratio_name,
        &mut percent,
        PERCENT_DRAG,
        percents,
        Some(2),
        width,
    );
    let grow = match (sized, ratioed) {
        (true, _) if shown_mm > 0.0 => size_mm / shown_mm,
        (_, true) => percent / 100.0 / before,
        _ => return false,
    };
    let together = linked || (sized && on_the_plate);
    *factors = grown(*factors, axis, grow, together);
    true
}

/// `factors` with `axis` grown `grow` times, or all three when `together`, none below the
/// smallest scale.
fn grown(factors: Vec3, axis: usize, grow: f32, together: bool) -> Vec3 {
    let mut grown = factors;
    if together {
        grown *= grow;
    } else {
        grown[axis] *= grow;
    }
    grown.max(Vec3::splat(MIN_SCALE))
}

/// The axis letter, and its unit, in a column of one width so the fields line up.
fn label_column(ui: &mut egui::Ui, axis: usize, unit: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(AXIS_W, theme::FIELD_H),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(AXIS_W);
            axis_label(ui, axis, unit);
        },
    );
}

/// Standing the selection on the plate, over its middle, or as large as the volume takes.
fn place(ui: &mut egui::Ui, scene: &mut Scene, plate: &BuildPlate, id: ObjectId) {
    section(ui, "Place", None, |ui| {
        let mut fitted = false;
        ui.columns(3, |columns| {
            let Some(object) = scene.get_mut(id) else {
                return;
            };
            if secondary_button(&mut columns[0], "", "On the plate").clicked() {
                object.stand_on_plate();
            }
            if secondary_button(&mut columns[1], "", "Centre").clicked()
                && let Some(bounds) = object.world_bounds()
            {
                let offset = plate.center() - bounds.center();
                object.transform.translation += offset * Vec3::new(1.0, 1.0, 0.0);
            }
            if secondary_button(&mut columns[2], "", "Scale to fit").clicked() {
                fit(object, plate);
                fitted = true;
            }
        });
        if fitted {
            recentre(scene, plate, id);
        }
        lift(ui, scene, id);
    });
}

/// How high the model's lowest point stands over the plate. Typing a height moves the
/// model there; it is the model's place, so nothing else remembers it.
fn lift(ui: &mut egui::Ui, scene: &mut Scene, id: ObjectId) {
    let Some(object) = scene.get_mut(id) else {
        return;
    };
    let Some(bottom_mm) = object.world_bounds().map(|bounds| bounds.mins.z) else {
        return;
    };
    let mut lift_mm = bottom_mm;
    let typed = number_row(
        ui,
        "Lift off the plate",
        &mut lift_mm,
        "mm",
        POSITION_DRAG,
        0.0..=MAX_LIFT_MM,
        2,
    );
    if typed {
        object.transform.translation.z += lift_mm - bottom_mm;
    }
}

/// Turning the selection the way it prints best, and the goals that search will weigh.
fn orient(ui: &mut egui::Ui, window: &mut Window, id: ObjectId) {
    section(ui, "Orient", None, |ui| {
        describe(ui, "Turn the selection the way it prints best.");
        let settings = &mut window.tools.orient.settings;
        ui.horizontal(|ui| {
            field_label(ui, "Optimise for", theme::colors().text_mid, "");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("orient-goal")
                    .width(theme::FIELD_W)
                    .selected_text(settings.goal.label())
                    .show_ui(ui, |ui| {
                        for goal in OrientGoal::ALL {
                            ui.selectable_value(&mut settings.goal, goal, goal.label());
                        }
                    });
            });
        });
        let mut limited = settings.max_tilt_deg.is_some();
        if switch(ui, &mut limited, "Limit the tilt").changed() {
            settings.max_tilt_deg = limited.then_some(DEFAULT_TILT_DEG);
        }
        if let Some(tilt_deg) = settings.max_tilt_deg.as_mut() {
            nested(ui, |ui| {
                number_row(
                    ui,
                    "Max tilt",
                    tilt_deg,
                    "\u{b0}",
                    ANGLE_DRAG,
                    0.0..=180.0,
                    0,
                );
            });
            describe(ui, "Measured from the way the model stands now.");
        }
        auto_orient(ui, window, id);
    });
}

/// The model's largest flat faces, each laid on the plate by a press, and a wait for a
/// click on any other face. Pressed again, or Esc, stops waiting.
fn lay_flat(ui: &mut egui::Ui, window: &mut Window, id: ObjectId) {
    section(ui, "Lay flat", None, |ui| {
        describe(ui, "Turn the model so a face rests on the plate.");
        let Some(object) = window.doc.scene.get(id) else {
            return;
        };
        let flats = window.tools.orient.flat_faces(object);
        let mut laid = None;
        for flat in flats.iter() {
            let normal = object.world_normal(flat.normal);
            let area_mm2 = world_area_mm2(flat.area_mm2, flat.normal, object.transform.scale);
            if face_row(ui, &facing(normal), area_mm2).clicked() {
                laid = Some(normal);
            }
        }
        if let Some(normal) = laid
            && let Some(object) = window.doc.scene.get_mut(id)
        {
            object.lay_face_down(normal);
        }

        let tools = &mut window.tools;
        let picking = tools.orient.picking_face;
        let label = if picking {
            "Click a face to lay down"
        } else {
            "Pick another face on the model"
        };
        if secondary_button(ui, icon::SELECT, label).clicked() {
            if picking {
                tools.orient.stop_picking();
            } else {
                tools.orient.picking_face = true;
            }
        }
    });
}

/// One flat face: which way it looks now, and how much of it there is.
fn face_row(ui: &mut egui::Ui, name: &str, area_mm2: f32) -> egui::Response {
    let colors = theme::colors();
    let button = egui::Button::new(
        egui::RichText::new(name)
            .font(theme::label())
            .color(colors.text_high),
    )
    .right_text(
        egui::RichText::new(format!("{area_mm2:.0} mm\u{b2}"))
            .font(theme::figures(11.5))
            .color(colors.text_low),
    )
    .min_size(egui::vec2(ui.available_width(), theme::FIELD_H));
    ui.add(button)
}

/// The side of the plate a face looks towards, in plate coordinates.
fn facing(normal: Vec3) -> String {
    let along = normal.abs();
    let tilted = along.max_element() < STRAIGHT;
    let side = if along.z >= along.x && along.z >= along.y {
        if normal.z < 0.0 { "Underside" } else { "Top" }
    } else if along.y >= along.x {
        if normal.y < 0.0 { "Front" } else { "Back" }
    } else if normal.x < 0.0 {
        "Left side"
    } else {
        "Right side"
    };
    if tilted {
        format!("{side}, tilted")
    } else {
        side.to_owned()
    }
}

/// An area measured in the model's own space, as it stands stretched by `scale`: a face
/// with unit normal `n` grows by `|sx sy sz| * |n / s|`.
fn world_area_mm2(area_mm2: f32, normal: Vec3, scale: Vec3) -> f32 {
    let scale = scale.abs().max(Vec3::splat(MIN_SCALE));
    area_mm2 * scale.x * scale.y * scale.z * (normal / scale).length()
}

/// Every flip works on the whole selection, so a plate of four does not need the same
/// button pressed four times. Keeping the original flips a copy stood beside it instead.
fn mirror(ui: &mut egui::Ui, scene: &mut Scene) {
    let keep_id = ui.id().with("mirror-keep");
    let mut keep = ui.data(|data| data.get_temp(keep_id).unwrap_or(false));
    section(ui, "Mirror", None, |ui| {
        let picked = scene.selection().to_vec();
        let mut flipped = None;
        ui.columns(Axis::ALL.len(), |columns| {
            for (column, axis) in columns.iter_mut().zip(Axis::ALL) {
                let label = format!("Across {}", axis.label());
                if secondary_button(column, "", &label).clicked() {
                    flipped = Some(axis);
                }
            }
        });
        if let Some(axis) = flipped {
            let flipped = if keep { copies(scene, &picked) } else { picked };
            for id in &flipped {
                scene.mirror(*id, axis);
            }
        }
        switch(ui, &mut keep, "Keep the original");
        describe(ui, "Mirror a copy beside it instead.");
    });
    ui.data_mut(|data| data.insert_temp(keep_id, keep));
}

/// A copy of each of `ids` stood clear of it to the right, all of them selected.
fn copies(scene: &mut Scene, ids: &[ObjectId]) -> Vec<ObjectId> {
    let copied: Vec<ObjectId> = ids
        .iter()
        .filter_map(|id| {
            let bounds = scene.get(*id)?.world_bounds()?;
            let width = bounds.maxs.x - bounds.mins.x;
            scene.duplicate(*id, Vec3::new(width + COPY_GAP_MM, 0.0, 0.0))
        })
        .collect();
    scene.select_many(&copied);
    copied
}

/// Turns the model the way it prints best, on a worker thread: the search measures every
/// candidate over the whole mesh and slices the best few.
fn auto_orient(ui: &mut egui::Ui, window: &mut Window, id: ObjectId) {
    let running = window.tools.orient.is_running();
    let label = if running {
        "Orienting..."
    } else {
        "Auto-orient"
    };
    let clicked = ui
        .add_enabled_ui(!running, |ui| secondary_button(ui, icon::ORIENT, label))
        .inner
        .clicked();

    if running {
        // The search measures every candidate over the whole mesh, which is no share to
        // count: the bar runs rather than standing at nought.
        progress_bar(ui, None);
    }
    if clicked && let Err(error) = window.tools.orient.start(&window.doc.scene, Some(id)) {
        window.machine.status = crate::status::Status::failed(&error);
    }
}

/// Stretches the model evenly until its bounds meet the build volume on the tightest side.
fn fit(object: &mut SceneObject, plate: &BuildPlate) {
    let Some(bounds) = object.world_bounds() else {
        return;
    };
    let size = bounds.maxs - bounds.mins;
    let room = Vec3::new(plate.x_mm, plate.y_mm, plate.z_mm);
    let factor = (room / size.max(Vec3::splat(f32::EPSILON))).min_element();
    let pivot = object.pivot();
    object.settle(Transform {
        scale: keeping_mirror(pivot.scale, pivot.scale.abs() * factor),
        ..pivot
    });
}

/// `sizes` as factors on each axis, flipped wherever `scale` was: the fields show and
/// take a size, and a mirror is not one.
fn keeping_mirror(scale: Vec3, sizes: Vec3) -> Vec3 {
    sizes.max(Vec3::splat(MIN_SCALE)) * scale.signum()
}

/// Centres the model over the plate and stands it on it, in one move.
fn recentre(scene: &mut Scene, plate: &BuildPlate, id: ObjectId) {
    if let Some(index) = scene.objects().iter().position(|object| object.id == id) {
        recenter(scene, plate, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stretched_face_grows_by_the_two_axes_it_spans() {
        let scale = Vec3::new(2.0, -3.0, 4.0);
        // A unit square facing up spans X and Y: 2 by 3 once stretched, mirror or not.
        assert!((world_area_mm2(1.0, Vec3::Z, scale) - 6.0).abs() < 1e-5);
        assert!((world_area_mm2(1.0, Vec3::X, scale) - 12.0).abs() < 1e-5);
    }

    #[test]
    fn a_turn_is_about_the_plates_axes_or_the_models_own() {
        let lying = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let turn = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        // Laid on its back, the model's own Z points along the plate's -Y.
        let model_z = turned_by(lying, turn, Axes::Local) * Vec3::Z;
        assert!(
            model_z.abs_diff_eq(lying * Vec3::Z, 1e-5),
            "a turn about its own Z spins it in place"
        );
        let plate_z = turned_by(lying, turn, Axes::Global) * Vec3::Z;
        assert!(
            plate_z.abs_diff_eq(Vec3::X, 1e-5),
            "a turn about the plate's Z swings -Y round to X, got {plate_z:?}"
        );
    }

    #[test]
    fn a_size_grows_its_own_axis_or_all_three_together() {
        let factors = Vec3::new(1.0, 2.0, 0.5);
        assert_eq!(grown(factors, 1, 1.5, false), Vec3::new(1.0, 3.0, 0.5));
        assert_eq!(grown(factors, 1, 1.5, true), Vec3::new(1.5, 3.0, 0.75));
        assert_eq!(
            grown(factors, 2, 0.0, false),
            Vec3::new(1.0, 2.0, MIN_SCALE),
            "nothing shrinks below the smallest scale"
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
}
