//! Which level each model is drawn at this frame, from what the card can draw, camera
//! motion and apparent size; see `docs/design/viewport.md`.

use std::sync::Arc;

use core_geometry::{FastMap, Mesh};

use crate::camera::OrbitCamera;
use crate::panels::Window;
use crate::render::{Budget, Candidate, Level, Light, Lods, Step, allot, px_per_mm, refine};
use crate::scene::{ObjectId, SceneObject};
use crate::state::View;

/// How long after the camera last moved it still counts as moving, seconds: long enough
/// to carry a drag over a pointer held still, short enough not to be noticed.
const SETTLE_S: f64 = 0.3;
/// The most a resting frame's levels may show, pixels: half of one is not seen.
const UNSEEN_PX: f32 = 0.5;
/// The most a moving frame's levels may show, pixels: a model drawn lighter while it turns
/// still reads as itself. Past it the frame is slower instead.
const MOVING_MOST_PX: f32 = 4.0;
/// The least share of its faces a picked model is drawn with, however the camera moves:
/// the model in hand is the one being looked at.
const PICKED_LEAST_SHARE: f32 = 0.2;

/// What the frame draws in place of models' own meshes, and the models' faces it draws,
/// or `None` for a frame the card's time is not to be learnt from.
pub(super) struct Levels {
    pub light: Light,
    pub faces: Option<usize>,
}

/// One model of the frame: its levels as `allot` weighs them, and the lighter meshes
/// behind every level after the first.
struct Model {
    id: ObjectId,
    steps: Candidate,
    lighter: Vec<Arc<Mesh>>,
}

/// The levels this frame draws in a view `height_px` tall: within the card's 60 Hz budget
/// while the camera moves, detail back a level a frame at rest. A capped cut draws whole.
pub(super) fn levels(ui: &egui::Ui, window: &mut Window, capped: bool, height_px: f32) -> Levels {
    let view = &mut *window.view;
    view.budget_for(view.options.render);
    let moving = camera_moving(ui, view, capped);
    if capped {
        view.levels.clear();
        return Levels {
            light: Light::default(),
            faces: None,
        };
    }
    let scene = &window.doc.scene;
    let models: Vec<Model> = scene
        .printable(scene.active_plate())
        .map(|object| {
            let picked = scene.is_selected(object.id);
            model(
                object,
                &mut view.lods,
                &view.camera,
                height_px,
                moving,
                picked,
            )
        })
        .collect();
    let wanted = wanted(&view.budget, &models, moving);
    let (chosen, settling) = if moving {
        (wanted, true)
    } else {
        let drawn = drawn(&view.levels, &models, &wanted);
        let chosen = refine(&drawn, &wanted);
        // Detail still coming back is worth another frame; once it is all back, none.
        let settling = chosen != drawn;
        if settling {
            ui.ctx().request_repaint();
        }
        (chosen, settling)
    };

    let faces: usize = models
        .iter()
        .zip(&chosen)
        .map(|(model, &level)| model.steps[level].faces)
        .sum();
    if moving {
        view.drawn_faces = Some(faces);
    }
    view.levels = models
        .iter()
        .zip(&chosen)
        .map(|(model, &level)| (model.id, level))
        .collect();
    let meshes = models
        .into_iter()
        .zip(chosen)
        .filter(|&(_, level)| level > 0)
        .map(|(model, level)| (model.id, Arc::clone(&model.lighter[level - 1])))
        .collect();
    Levels {
        light: Light { meshes, settling },
        faces: Some(faces),
    }
}

/// Whether the camera has moved within `SETTLE_S`, taking in the frame's timings and the
/// levels built since the last one on the way.
fn camera_moving(ui: &egui::Ui, view: &mut View, capped: bool) -> bool {
    let (now, dt_s) = ui.input(|input| (input.time, input.unstable_dt));
    if view.camera != view.drawn_camera {
        view.drawn_camera = view.camera;
        view.moved_s = now;
    }
    if view.lods.poll() {
        ui.ctx().request_repaint();
    }
    view.budget.take_times();
    let still_s = now - view.moved_s;
    let moving = still_s < SETTLE_S;
    // What the last frame drew is what this frame's wait was spent on.
    if let Some(faces) = view.drawn_faces.take().filter(|_| moving && !capped) {
        view.budget.observe(dt_s, faces);
    }
    if moving {
        // One more frame once the camera has come to rest, to bring the detail back.
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(SETTLE_S - still_s));
    }
    moving
}

/// The levels the frame would draw going straight to them: what fits 60 Hz while the camera
/// moves, only the unseen given up at rest, every model whole while the card copes.
fn wanted(budget: &Budget, models: &[Model], moving: bool) -> Vec<usize> {
    let candidates: Vec<Candidate> = models.iter().map(|model| model.steps.clone()).collect();
    let allotted = if moving {
        budget
            .moving()
            .map(|budget| allot(&candidates, budget, MOVING_MOST_PX))
    } else {
        budget
            .resting()
            .map(|budget| allot(&candidates, budget, UNSEEN_PX))
    };
    allotted.unwrap_or_else(|| vec![0; models.len()])
}

/// The level each model was drawn at last, or for one new to the frame the one it wants.
fn drawn(levels: &FastMap<ObjectId, usize>, models: &[Model], wanted: &[usize]) -> Vec<usize> {
    models
        .iter()
        .zip(wanted)
        .map(|(model, &wanted)| {
            let last = model.steps.len() - 1;
            levels
                .get(&model.id)
                .map_or(wanted, |&drawn| drawn.min(last))
        })
        .collect()
}

/// `object`'s levels as they look from `camera`. A hollowed model at rest has only its
/// own: the resin trapped in it is painted from faces no lighter level has.
fn model(
    object: &SceneObject,
    lods: &mut Lods,
    camera: &OrbitCamera,
    height_px: f32,
    moving: bool,
    picked: bool,
) -> Model {
    let mesh = object.printed();
    let mut steps = vec![Step {
        faces: mesh.faces.len(),
        error_px: 0.0,
    }];
    let built = lods.of(mesh);
    let levels = match object.hollow.cavity_faces() {
        Some(_) if !moving => &[],
        _ => built,
    };
    let px = px_per_model_mm(object, camera, height_px);
    let levels = &levels[..kept_levels(mesh.faces.len(), levels, picked)];
    steps.extend(levels.iter().map(|level| Step {
        faces: level.mesh.faces.len(),
        error_px: level.error_mm * px,
    }));
    Model {
        id: object.id,
        steps,
        lighter: levels.iter().map(|level| Arc::clone(&level.mesh)).collect(),
    }
}

/// How many of a model's lighter `levels` it may be drawn at: all, or for a picked model
/// those keeping at least `PICKED_LEAST_SHARE` of its `faces`.
fn kept_levels(faces: usize, levels: &[Level], picked: bool) -> usize {
    if !picked {
        return levels.len();
    }
    let least = (faces as f32 * PICKED_LEAST_SHARE) as usize;
    levels
        .iter()
        .take_while(|level| level.mesh.faces.len() >= least)
        .count()
}

/// Pixels a millimetre of `object`'s own mesh spans at its nearest to the eye. An eye
/// inside it sees any error as large as it gets.
fn px_per_model_mm(object: &SceneObject, camera: &OrbitCamera, height_px: f32) -> f32 {
    let Some(bounds) = object.world_bounds() else {
        return 0.0;
    };
    let middle = (bounds.mins + bounds.maxs) / 2.0;
    let nearest_mm = middle.distance(camera.eye()) - (bounds.maxs - middle).length();
    if nearest_mm <= 0.0 && !camera.orthographic {
        return f32::MAX;
    }
    let linear = object.transform.linear();
    let scale = linear
        .x_axis
        .length()
        .max(linear.y_axis.length())
        .max(linear.z_axis.length());
    px_per_mm(camera, nearest_mm, height_px) * scale
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plate::BuildPlate;

    /// Levels of a mesh of `faces` faces, each holding `share` of the one before.
    fn levels_of(faces: usize, share: f32, count: usize) -> Vec<Level> {
        (1..=count)
            .map(|step| {
                let kept = (faces as f32 * share.powi(step as i32)) as usize;
                let mesh = Mesh::new(vec![core_geometry::Vec3::ZERO; 3], vec![[0, 1, 2]; kept]);
                Level {
                    mesh: Arc::new(mesh),
                    error_mm: step as f32,
                }
            })
            .collect()
    }

    #[test]
    fn a_picked_model_keeps_a_fifth_of_its_faces_however_the_camera_moves() {
        let levels = levels_of(100_000, 0.5, 5);
        // 50 000 and 25 000 hold a fifth; 12 500 and finer do not.
        assert_eq!(kept_levels(100_000, &levels, true), 2);
    }

    #[test]
    fn a_model_not_picked_may_be_drawn_at_any_of_its_levels() {
        let levels = levels_of(100_000, 0.5, 5);
        assert_eq!(kept_levels(100_000, &levels, false), levels.len());
    }

    #[test]
    fn a_moving_frame_never_draws_a_model_coarser_than_its_cap() {
        let candidates = vec![vec![
            Step {
                faces: 1_000_000,
                error_px: 0.0,
            },
            Step {
                faces: 200_000,
                error_px: 1.0,
            },
            Step {
                faces: 2_000,
                error_px: 40.0,
            },
        ]];
        assert_eq!(allot(&candidates, 1_000, MOVING_MOST_PX), vec![1]);
    }

    #[test]
    fn the_budget_is_learnt_afresh_when_the_render_view_is_turned_on() {
        let mut view = View::framing(&BuildPlate::default());
        for _ in 0..3 {
            view.budget.observe(0.05, 1_000_000);
        }
        assert!(view.budget.moving().is_some(), "slow frames cut the budget");
        view.budget_for(false);
        assert!(
            view.budget.moving().is_some(),
            "the same view keeps what it learnt"
        );
        view.budget_for(true);
        assert!(
            view.budget.moving().is_none(),
            "the render view starts from nothing"
        );
    }
}
