use core_geometry::glam::EulerRot;
use core_geometry::{Quat, Transform, Vec3};

use crate::import::recenter;
use crate::panels::Window;
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::state::Tools;
use crate::ui::{
    axis_label, compact_button, field_label, icon, icon_toggle, number_field, progress_bar,
    secondary_button, section_with_action, theme,
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

/// Position and angle take any value.
const ANY: std::ops::RangeInclusive<f32> = f32::MIN..=f32::MAX;

/// Where the selected model stands, in the same numbers the gizmo drags: its pivot, not
/// the origin its file was authored around. See `docs/decisions/0110`.
pub fn ui(ui: &mut egui::Ui, window: &mut Window) {
    let Some(id) = window.doc.scene.selected() else {
        return;
    };
    position(ui, &mut window.doc.scene, &window.doc.plate, id);
    rotation(ui, window, id);
    scale(ui, &mut window.doc.scene, &window.doc.plate, id);
}

fn position(ui: &mut egui::Ui, scene: &mut Scene, plate: &BuildPlate, id: ObjectId) {
    let tooltip = "Back to the middle of the plate";
    let (reset, _) = section_with_action(ui, "Move", (icon::RESET, tooltip), |ui| {
        let Some(object) = scene.get_mut(id) else {
            return;
        };
        let mut pivot = object.pivot();
        let mut changed = false;
        for axis in 0..3 {
            ui.horizontal(|ui| {
                label_column(ui, axis, "mm");
                let width = ui.available_width();
                let value = &mut pivot.translation[axis];
                changed |= number_field(ui, value, POSITION_DRAG, ANY, Some(2), width);
            });
        }
        // Written back only on a frame the user changed a number: settling every frame
        // would let the round trip through the centre of mass accumulate.
        if changed {
            object.settle(pivot);
        }
        ui.add_space(4.0);
        if secondary_button(ui, "", "Center").clicked()
            && let Some(bounds) = object.world_bounds()
        {
            let offset = plate.center() - bounds.center();
            object.transform.translation += offset * Vec3::new(1.0, 1.0, 0.0);
        }
        if secondary_button(ui, "", "On Platform").clicked() {
            object.stand_on_plate();
        }
    });
    if reset {
        place(scene, plate, id);
    }
}

/// Euler angles are written back only on the frames the user actually changed one.
/// Round-tripping the quaternion every frame would let rounding accumulate, and near
/// gimbal lock the three numbers would drift on their own.
fn rotation(ui: &mut egui::Ui, window: &mut Window, id: ObjectId) {
    let tooltip = "Back to the way the model was imported";
    let (reset, _) = section_with_action(ui, "Rotation", (icon::RESET, tooltip), |ui| {
        if let Some(object) = window.doc.scene.get_mut(id) {
            let mut pivot = object.pivot();
            if rotation_rows(ui, &mut pivot) {
                object.settle(pivot);
            }
        }
        ui.add_space(4.0);
        orient_to_face(ui, window.tools);
        auto_orient(ui, window, id);
    });
    if reset && let Some(object) = window.doc.scene.get_mut(id) {
        object.settle(Transform {
            rotation: Quat::IDENTITY,
            ..object.pivot()
        });
        object.stand_on_plate();
    }
}

fn rotation_rows(ui: &mut egui::Ui, pivot: &mut Transform) -> bool {
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
            typed |= number_field(ui, &mut degrees[axis], ANGLE_DRAG, ANY, Some(2), width);
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
        pivot.rotation = (turn * pivot.rotation).normalize();
    }
    typed || turned
}

/// Size in the model's own axes, stretched: what the model measures before it is turned,
/// so a number typed here stays put however it is rotated afterwards.
fn scale(ui: &mut egui::Ui, scene: &mut Scene, plate: &BuildPlate, id: ObjectId) {
    let linked_id = ui.id().with("scale-linked");
    let mut linked = ui.memory(|memory| memory.data.get_temp(linked_id).unwrap_or(true));
    let tooltip = "Back to the size the model was imported at";
    let (reset, _) = section_with_action(ui, "Scale", (icon::RESET, tooltip), |ui| {
        let Some(object) = scene.get_mut(id) else {
            return;
        };
        let extent = object
            .mesh
            .aabb()
            .map_or(Vec3::ONE, |bounds| bounds.maxs - bounds.mins);
        let mut pivot = object.pivot();
        let column = scale_header(ui, &mut linked);
        let mut factors = pivot.scale.abs();
        let mut changed = false;
        for axis in 0..3 {
            ui.horizontal(|ui| {
                label_column(ui, axis, "");
                changed |= scale_row(ui, &mut factors, extent, axis, linked, column);
            });
        }
        if changed {
            pivot.scale = keeping_mirror(pivot.scale, factors);
            object.settle(pivot);
        }
        ui.add_space(4.0);
        if secondary_button(ui, "", "Scale to Fit").clicked() {
            fit(object, plate);
            place(scene, plate, id);
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
    extent: Vec3,
    axis: usize,
    linked: bool,
    width: f32,
) -> bool {
    let before = factors[axis];
    let mut size_mm = extent[axis] * before;
    let mut percent = before * 100.0;
    let min_size = extent[axis] * MIN_SCALE;

    let sizes = min_size..=f32::MAX;
    let sized = number_field(ui, &mut size_mm, POSITION_DRAG, sizes, Some(2), width);
    let percents = MIN_SCALE * 100.0..=f32::MAX;
    let ratioed = number_field(ui, &mut percent, PERCENT_DRAG, percents, Some(2), width);
    let wanted = match (sized, ratioed) {
        (true, _) if extent[axis] > 0.0 => Some(size_mm / extent[axis]),
        (_, true) => Some(percent / 100.0),
        _ => None,
    };
    let Some(wanted) = wanted else {
        return false;
    };
    let wanted = wanted.max(MIN_SCALE);
    if linked {
        *factors = (*factors * (wanted / before)).max(Vec3::splat(MIN_SCALE));
    } else {
        factors[axis] = wanted;
    }
    true
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

/// Waits for a click on a model, then lays the face under it on the plate. Pressed again,
/// or Esc, stops waiting.
fn orient_to_face(ui: &mut egui::Ui, tools: &mut Tools) {
    let picking = tools.orient.picking_face;
    let label = if picking {
        "Click a face to lay down"
    } else {
        "Orient to Face"
    };
    if secondary_button(ui, icon::SELECT, label).clicked() {
        if picking {
            tools.orient.stop_picking();
        } else {
            tools.orient.picking_face = true;
        }
    }
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
fn place(scene: &mut Scene, plate: &BuildPlate, id: ObjectId) {
    if let Some(index) = scene.objects().iter().position(|object| object.id == id) {
        recenter(scene, plate, index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
