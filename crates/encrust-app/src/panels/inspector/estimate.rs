use core_geometry::Scalar;

use crate::panels::Window;
use crate::scene::Scene;
use crate::slicing::Slicing;
use crate::ui::stats;

/// How tall the stack will be, drawn over the Slice button so the layer height can be
/// judged without opening the tool that sets it.
pub fn row(ui: &mut egui::Ui, window: &mut Window) {
    let (height_mm, layers) = stack(&window.doc.scene, &window.machine.slicing);
    // An adaptive stack only knows its count once the model has been planned, so the
    // panel states the two ends of the range the settings allow.
    let count = match layers {
        (most, Some(fewest)) => format!("{fewest} to {most}"),
        (layers, None) => layers.to_string(),
    };
    stats(
        ui,
        &[("Height", format!("{height_mm:.1} mm")), ("Layers", count)],
    );
}

/// The height of everything visible on the plate and how many layers it comes to: one
/// count, or the most and the fewest an adaptive stack could take.
fn stack(scene: &Scene, slicing: &Slicing) -> (Scalar, (usize, Option<usize>)) {
    let Some(bounds) = scene.world_bounds() else {
        return (0.0, (0, None));
    };
    let height_mm = (bounds.maxs.z - bounds.mins.z).max(0.0);
    let count = |height: Scalar| (height_mm / height).ceil().max(0.0) as usize;
    let fewest = slicing
        .adaptive
        .map(|_| count(slicing.layer_height_mm()))
        .filter(|_| slicing.layer_height_mm() > 0.0);
    let thinnest = slicing
        .adaptive
        .map_or(slicing.layer_height_mm(), |adaptive| adaptive.min_height_mm);
    (height_mm, (count(thinnest), fewest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::ImportSummary;
    use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
    use std::sync::Arc;

    fn scene_with_a_model(height_mm: Scalar) -> Scene {
        let mesh = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, height_mm),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let mut scene = Scene::default();
        scene.insert(crate::scene::Imported::new(
            "tetra".to_owned(),
            Arc::new(mesh.clone()),
            Transform::default(),
            ImportSummary {
                vertices_merged: 0,
                faces_removed: 0,
                orientation: Orientation {
                    flipped_faces: 0,
                    inverted_shells: 0,
                    orientable: true,
                },
                diagnostics: diagnose(&mesh),
            },
        ));
        scene
    }

    #[test]
    fn the_estimate_counts_the_layers_the_model_needs() {
        let mut slicing = Slicing::default();
        slicing.set_layer_height(0.05);
        // One millimetre at 0.05 mm a layer.
        let (height_mm, layers) = stack(&scene_with_a_model(1.0), &slicing);
        assert!((height_mm - 1.0).abs() < 1e-6);
        assert_eq!(layers, (20, None));
    }

    #[test]
    fn an_adaptive_estimate_states_both_ends_of_what_the_settings_allow() {
        let mut slicing = Slicing::default();
        slicing.set_layer_height(0.1);
        slicing.adaptive = Some(core_slicer::AdaptiveSettings {
            min_height_mm: 0.02,
            ..core_slicer::AdaptiveSettings::default()
        });
        // One millimetre is ten layers at the ceiling and fifty at the floor.
        let (_, layers) = stack(&scene_with_a_model(1.0), &slicing);
        assert_eq!(layers, (50, Some(10)));
    }

    #[test]
    fn an_empty_plate_has_nothing_to_estimate() {
        assert_eq!(
            stack(&Scene::default(), &Slicing::default()),
            (0.0, (0, None))
        );
    }
}
