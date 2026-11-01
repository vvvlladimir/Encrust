mod place;

use core_geometry::glam::EulerRot;
use core_geometry::{Mat3, Quat, Transform, Vec3};

use crate::gizmo::Axes;
use crate::i18n::tr;
use crate::import::recenter;
use crate::panels::Window;
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::ui::{
    Segment, Segmented, axis_label, compact_button, field_label, hairline, icon, icon_toggle, kept,
    number_field, section_with_action, section_with_actions, theme,
};

/// Millimetres, degrees and percent per point of drag on the transform fields.
pub(super) const POSITION_DRAG: f64 = 0.1;
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

/// The highest a model may be lifted off the plate by typing, millimetres.
pub(super) const MAX_LIFT_MM: f32 = 100.0;

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
    scale(ui, &mut window.doc.scene, &window.doc.plate, id, axes);
    place::place(ui, window, id);
    place::auto_orient_section(ui, window, id);
}

/// Whether the handles, the turn buttons and the sizes follow the plate's axes or the
/// model's own. Where the model stands and how it is turned are read on the plate's either way.
fn axes(ui: &mut egui::Ui, axes: &mut Axes) {
    egui::Frame::new()
        .inner_margin(theme::PANEL_MARGIN)
        .show(ui, |ui| {
            let segments = [
                Segment::new(Axes::Global, tr!("position-global")),
                Segment::new(Axes::Local, tr!("position-local")),
            ];
            Segmented::new(&segments)
                .width(ui.available_width())
                .show(ui, axes);
        });
    hairline(ui);
}

fn position(ui: &mut egui::Ui, scene: &mut Scene, plate: &BuildPlate, id: ObjectId) {
    let actions = [
        (icon::DROP_TO_PLATE, tr!("position-on-plate")),
        (icon::RESET, tr!("position-move-reset")),
    ];
    let actions: Vec<(&str, &str)> = actions
        .iter()
        .map(|(glyph, tip)| (*glyph, tip.as_str()))
        .collect();
    let (pressed, _) = section_with_actions(ui, &tr!("position-move"), &actions, |ui| {
        let Some(object) = scene.get_mut(id) else {
            return;
        };
        let mut pivot = object.pivot();
        let mut changed = false;
        for (axis, axis_name) in AXIS_NAMES.iter().enumerate() {
            ui.horizontal(|ui| {
                label_column(ui, axis, &tr!("unit-mm"));
                let width = ui.available_width();
                let value = &mut pivot.translation[axis];
                let name = tr!("position-move-along", axis = *axis_name);
                changed |= number_field(ui, &name, value, POSITION_DRAG, ANY, Some(2), width);
            });
        }
        // Written back only on a frame the user changed a number: settling every frame
        // would let the round trip through the centre of mass accumulate.
        if changed {
            object.settle(pivot);
        }
    });
    match pressed {
        Some(0) => {
            if let Some(object) = scene.get_mut(id) {
                object.stand_on_plate();
                object.lift_mm = None;
            }
        }
        Some(_) => recentre(scene, plate, id),
        None => {}
    }
}

/// Euler angles are written back only on frames the user changed one: round-tripping the
/// quaternion every frame accumulates rounding and drifts near gimbal lock.
fn rotation(ui: &mut egui::Ui, window: &mut Window, id: ObjectId, axes: Axes) {
    let tooltip = tr!("position-rotation-reset");
    let (reset, _) = section_with_action(
        ui,
        &tr!("position-rotation"),
        (icon::RESET, &tooltip),
        |ui| {
            if let Some(object) = window.doc.scene.get_mut(id) {
                let pivot = object.pivot();
                if let Some(turn) = rotation_rows(ui, pivot.rotation, axes) {
                    object.settle(pivot.turned(turn));
                }
            }
        },
    );
    if reset && let Some(object) = window.doc.scene.get_mut(id) {
        let pivot = object.pivot();
        object.settle(pivot.turned(pivot.rotation.inverse()));
        object.stand_on_plate();
    }
}

/// The angles the model is turned to, and the buttons that turn it further, about the
/// plate's axes, so a stretch the model carries turns with it.
fn rotation_rows(ui: &mut egui::Ui, rotation: Quat, axes: Axes) -> Option<Quat> {
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
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
            let name = tr!("position-rotation-about", axis = AXIS_NAMES[axis]);
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
        let wanted = Quat::from_euler(
            EulerRot::XYZ,
            degrees.x.to_radians(),
            degrees.y.to_radians(),
            degrees.z.to_radians(),
        );
        return Some(wanted * rotation.inverse());
    }
    (turn != Quat::IDENTITY).then(|| about_plate(rotation, turn, axes))
}

/// `turn`, asked about the plate's axes or about the model's own, as a turn about the
/// plate's.
fn about_plate(rotation: Quat, turn: Quat, axes: Axes) -> Quat {
    match axes {
        Axes::Global => turn,
        Axes::Local => rotation * turn * rotation.inverse(),
    }
}

/// The model's size and stretch along the plate's axes or along its own.
fn scale(ui: &mut egui::Ui, scene: &mut Scene, plate: &BuildPlate, id: ObjectId, axes: Axes) {
    let linked_id = ui.id().with("scale-linked");
    let mut linked = kept::get(ui.ctx(), linked_id).unwrap_or(true);
    let actions = [
        (icon::FIT, tr!("position-scale-to-fit")),
        (icon::RESET, tr!("position-scale-reset")),
    ];
    let actions: Vec<(&str, &str)> = actions
        .iter()
        .map(|(glyph, tip)| (*glyph, tip.as_str()))
        .collect();
    let (pressed, _) = section_with_actions(ui, &tr!("position-scale"), &actions, |ui| {
        if let Some(object) = scene.get_mut(id) {
            scale_rows(ui, object, &mut linked, axes);
        }
    });
    kept::set(ui.ctx(), linked_id, &linked);
    if pressed == Some(0)
        && let Some(object) = scene.get_mut(id)
    {
        fit(object, plate);
        recentre(scene, plate, id);
        return;
    }
    if pressed == Some(1)
        && let Some(object) = scene.get_mut(id)
    {
        let pivot = object.pivot();
        object.settle(Transform {
            scale: keeping_mirror(pivot.scale, Vec3::ONE),
            stretch: Mat3::IDENTITY,
            ..pivot
        });
        object.stand_on_plate();
    }
}

fn scale_rows(ui: &mut egui::Ui, object: &mut SceneObject, linked: &mut bool, axes: Axes) {
    let pivot = object.pivot();
    let shown = Shown::of(object, axes);
    let column = scale_header(ui, linked);
    let mut asked = None;
    for axis in 0..3 {
        ui.horizontal(|ui| {
            label_column(ui, axis, "");
            if let Some(grow) = scale_row(ui, &shown, axis, column) {
                asked = Some((axis, grow));
            }
        });
    }
    if let Some((axis, grow)) = asked {
        let together = *linked;
        object.settle(grown(pivot, shown.ratio, axis, grow, together, axes));
    }
}

/// What the two columns read, each along the axes the switch above them picked.
struct Shown {
    /// What the model measures along each axis, millimetres.
    size_mm: Vec3,
    /// How many times each axis is drawn out of the imported model: the length of a model
    /// axis through the placement, or of a plate axis read back through it.
    ratio: Vec3,
}

impl Shown {
    fn of(object: &SceneObject, axes: Axes) -> Self {
        let linear = object.transform.linear();
        match (axes, object.world_bounds()) {
            (Axes::Global, Some(bounds)) => Self {
                size_mm: bounds.maxs - bounds.mins,
                ratio: object.transform.plate_factors(),
            },
            _ => {
                let ratio = Vec3::new(
                    linear.x_axis.length(),
                    linear.y_axis.length(),
                    linear.z_axis.length(),
                );
                let extent = object
                    .mesh
                    .aabb()
                    .map_or(Vec3::ONE, |bounds| bounds.maxs - bounds.mins);
                Self {
                    size_mm: extent * ratio,
                    ratio,
                }
            }
        }
    }
}

/// The two column titles and the link that keeps the three axes in proportion. Returns
/// the width each column's field gets.
fn scale_header(ui: &mut egui::Ui, linked: &mut bool) -> f32 {
    let spacing = ui.spacing().item_spacing.x;
    let column = (ui.available_width() - AXIS_W - 2.0 * spacing) / 2.0;
    ui.horizontal(|ui| {
        ui.add_space(AXIS_W + spacing);
        caption(ui, &tr!("position-size"), &tr!("unit-mm"), column);
        caption(
            ui,
            &tr!("position-ratio"),
            "%",
            column - theme::ICON_SIZE - spacing,
        );
        let tooltip = tr!("position-keep-proportions");
        if icon_toggle(ui, icon::LINKED, &tooltip, *linked).clicked() {
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

/// One axis as a size and as a percentage of the imported size. Answers how many times
/// over the user asked for that axis to grow, on the frame they did.
fn scale_row(ui: &mut egui::Ui, shown: &Shown, axis: usize, width: f32) -> Option<f32> {
    let (was_mm, was_ratio) = (shown.size_mm[axis], shown.ratio[axis]);
    let (mut size_mm, mut percent) = (was_mm, was_ratio * 100.0);

    let size_name = tr!("position-axis-size", axis = AXIS_NAMES[axis]);
    let least_mm = was_mm / was_ratio.max(MIN_SCALE) * MIN_SCALE;
    let sized = number_field(
        ui,
        &size_name,
        &mut size_mm,
        POSITION_DRAG,
        least_mm..=f32::MAX,
        Some(2),
        width,
    );
    let ratio_name = tr!("position-axis-ratio", axis = AXIS_NAMES[axis]);
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
    match (sized, ratioed) {
        (true, _) if was_mm > 0.0 => Some(size_mm / was_mm),
        (_, true) if was_ratio > 0.0 => Some(percent / 100.0 / was_ratio),
        _ => None,
    }
}

/// `pivot` with `axis` grown `grow` times (model or plate axes per `axes`), or all three when
/// `together`. Never drawn out below the smallest scale, from `ratio`.
fn grown(
    pivot: Transform,
    ratio: Vec3,
    axis: usize,
    grow: f32,
    together: bool,
    axes: Axes,
) -> Transform {
    if together {
        let grow = grow.max(MIN_SCALE / ratio.min_element().max(MIN_SCALE));
        return Transform {
            scale: pivot.scale * grow,
            ..pivot
        };
    }
    let grow = grow.max(MIN_SCALE / ratio[axis].max(MIN_SCALE));
    let mut factors = Vec3::ONE;
    factors[axis] = grow;
    match axes {
        Axes::Global => pivot.stretched(factors),
        Axes::Local => Transform {
            scale: pivot.scale * factors,
            ..pivot
        },
    }
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

/// Starts Auto-orient on `only`, or on the whole plate, and says so on the stage if it cannot.
pub(crate) fn auto_orient(window: &mut Window, only: Option<ObjectId>) {
    let machine = window
        .machine
        .slicing
        .printer
        .as_ref()
        .map(|printer| (printer, &window.machine.slicing.material));
    let started = window
        .tools
        .supports
        .orient_plans(&window.machine.slicing.catalogue)
        .and_then(|plans| {
            window.tools.orient.start(
                &mut window.doc.scene,
                &window.doc.plate,
                only,
                plans,
                machine,
                &mut window.machine.status,
            )
        });
    if let Err(error) = started {
        window.machine.status = crate::status::Status::failed(&error);
    }
}

#[cfg(test)]
mod tests {
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
}
