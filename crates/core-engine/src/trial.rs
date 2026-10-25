//! A pose judged by what it prints: stood on its supports, cut and cured, and read the
//! way a finished plate is. See `docs/design/orientation.md`.

mod islands;

use std::sync::Arc;

use core_analysis::{Measured, RiskKind};
use core_format::ExposurePlan;
use core_geometry::{
    Mesh, Quat, Scalar, Transform, Vec2, Vec3, center_over_plate, drop_to_plate, fitting_turn,
    signed_volume, transform_mesh,
};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_plate::{
    LOOKS_UP, OrientGoal, OrientPreset, OrientSettings, Oriented, Printed, Readings, Ruling,
    finalists, rank, rulings,
};
use printer_profiles::{BuildVolume, MaterialProfile, PrinterProfile};
use rayon::prelude::*;

use crate::cut::Cutting;
use crate::error::EngineError;
use crate::plate::{Model, Plate};
use crate::run::Run;
use crate::stand::{SupportPlan, ToStand, stand_seated, stand_under};
use crate::trial::islands::islands_born;

/// Poses printed to choose between, on top of the way the model stands: four for a model
/// of a million faces, more for a lighter one, which prints as many in the same time, but
/// never more for one past `LIGHT_REACH_MM` across, whose layers and plate cost more than
/// its faces.
const FINALISTS: usize = 4;
const MOST_FINALISTS: usize = 16;
const FINALIST_FACES: usize = 2_000_000;
const LIGHT_REACH_MM: Scalar = 200.0;

fn finalists_for(faces: usize, reach_mm: Scalar) -> usize {
    if reach_mm > LIGHT_REACH_MM {
        return FINALISTS;
    }
    (FINALIST_FACES / faces.max(1)).clamp(FINALISTS, MOST_FINALISTS)
}

/// How coarse a trial is cut: the tallest pose into about `TRIAL_LAYERS`, no layer
/// thicker than `TRIAL_LAYER_MM`, cured at a pixel of `TRIAL_PIXEL_MM`. A trial ranks
/// poses against each other, which the shape of a stack settles long before its last
/// layer and its last pixel do.
const TRIAL_LAYERS: Scalar = 160.0;
const TRIAL_LAYER_MM: Scalar = 0.15;
const TRIAL_PIXEL_MM: Scalar = 0.2;

/// The machine and the resin of `trial` made as coarse as a trial needs for a model
/// `reach_mm` across, and never finer than they are.
fn coarse(trial: &Trial, reach_mm: Scalar) -> (PrinterProfile, MaterialProfile) {
    let mut printer = trial.printer.clone();
    let display = &mut printer.display;
    let fewer = |pixels: u32, across_mm: Scalar| {
        ((across_mm / TRIAL_PIXEL_MM).ceil() as u32).clamp(1, pixels.max(1))
    };
    display.width_px = fewer(display.width_px, display.width_mm);
    display.height_px = fewer(display.height_px, display.height_mm);
    let mut material = trial.material.clone();
    material.layer_height_mm = material
        .layer_height_mm
        .max((reach_mm / TRIAL_LAYERS).min(TRIAL_LAYER_MM));
    (printer, material)
}

/// What every pose is printed with to be judged.
#[derive(Debug, Clone, Copy)]
pub struct Trial<'a> {
    pub printer: &'a PrinterProfile,
    pub material: &'a MaterialProfile,
    /// What the pose is supported with, as the window would stand it: one profile, or an
    /// automatic one sizing each support.
    pub support: &'a SupportPlan,
    /// Layers rasterised at once, as `Plate::raster_window`.
    pub raster_window: usize,
}

/// What a pose costs once it is supported, cut and cured.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Printout {
    pub contacts: usize,
    /// The resin in the supports, millilitres, as the plate's summary counts it.
    pub supports_ml: f32,
    /// Places that need holding up and are not: contacts with no room for a column, on
    /// supports or set on the plate.
    pub hanging: usize,
    /// Contacts on faces that look up in the model's own file.
    pub top_contacts: usize,
    pub layers: usize,
    /// How tall the stack stands, model and supports, millimetres: what the time follows.
    pub stack_mm: f32,
    /// The hardest pull of any layer on the film, newtons.
    pub pull_n: f64,
    /// The largest step in cured area from one layer to the next, square millimetres.
    pub growth_mm2: f32,
    /// Islands the print cures at its own pixel and layer, however coarse the trial.
    pub islands: usize,
    pub cups: usize,
    pub peels: usize,
    pub resin_ml: f32,
    /// Whether the pose and its supports overflow the build volume however they are turned
    /// about the vertical.
    pub overflows: bool,
}

impl Printout {
    /// What loses the whole print rather than a detail of it: suction cups and tearing
    /// pulls.
    pub fn fatal(&self) -> usize {
        self.cups + self.peels
    }

    /// The reading `goal` is meant to win, lower being better.
    pub fn reading(&self, goal: OrientGoal) -> f64 {
        match goal {
            OrientGoal::FewestSupports => f64::from(self.supports_ml),
            OrientGoal::LeastPeel => self.pull_n,
            OrientGoal::ShortestPrint => f64::from(self.stack_mm),
        }
    }

    /// What of `goal`'s [`Printout::reading`] is noise: a strut's resin, millilitres, half
    /// a newton, and two of a trial's thickest layers.
    pub fn noise(goal: OrientGoal) -> f64 {
        match goal {
            OrientGoal::FewestSupports => 0.05,
            OrientGoal::LeastPeel => 0.5,
            OrientGoal::ShortestPrint => 2.0 * f64::from(TRIAL_LAYER_MM),
        }
    }
}

/// An orientation chosen by printing the search's best few, and what it prints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proven {
    pub oriented: Oriented,
    pub printout: Printout,
}

/// One finalist printed, and what decided its place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tried {
    pub proven: Proven,
    pub ruling: Ruling,
}

/// One way to stand a model, and what it printed where it was printed to be judged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Choice {
    pub oriented: Oriented,
    pub printout: Option<Printout>,
    /// What decided its place: whether it is worth offering beside the first.
    pub ruling: Ruling,
}

/// The ways to stand a model for print, best first: the search's [`finalists`] printed
/// with `trial` and ranked by [`prove`] where there is a machine, and the search's own
/// answer where there is none. Every front end orients through it (ADR 0290).
pub fn orient_for_print(
    printed: &Printed,
    settings: &OrientSettings,
    trial: Option<&Trial>,
) -> Result<Vec<Choice>, EngineError> {
    let found = finalists(printed, settings, usize::MAX)?;
    let Some(trial) = trial else {
        return Ok(found
            .into_iter()
            .map(|oriented| Choice {
                oriented,
                printout: None,
                ruling: oriented.ruling,
            })
            .collect());
    };
    let tried = prove(printed, settings, &found, trial)?;
    Ok(tried
        .into_iter()
        .map(|tried| Choice {
            oriented: tried.proven.oriented,
            printout: Some(tried.proven.printout),
            ruling: tried.ruling,
        })
        .collect())
}

/// Prints the search's best few of `found` and the way the model stands, and answers them
/// ranked by [`core_plate::rank`] on what they printed. See `docs/design/orientation.md`.
pub fn prove(
    printed: &Printed,
    settings: &OrientSettings,
    found: &[Oriented],
    trial: &Trial,
) -> Result<Vec<Tried>, EngineError> {
    let reach_mm = printed
        .outer
        .aabb()
        .map_or(0.0, |bounds| (bounds.maxs - bounds.mins).length());
    let print = trial;
    let (printer, material) = coarse(trial, reach_mm);
    let trial = &Trial {
        printer: &printer,
        material: &material,
        ..*trial
    };
    let standing = settings.down_now.try_normalize().unwrap_or(Vec3::NEG_Z);
    let is_standing =
        |pose: &Oriented| (pose.rotation.inverse() * Vec3::NEG_Z).abs_diff_eq(standing, 1e-5);
    let poses = to_print(
        found,
        finalists_for(printed.outer.faces.len(), reach_mm),
        is_standing,
    );
    let tried = poses
        .into_par_iter()
        .map(|pose| {
            print_held(printed, pose.rotation, pose.on_plate, trial, Some(print)).map(|printout| {
                Proven {
                    oriented: *pose,
                    printout,
                }
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;
    Ok(judged(&tried, settings.preset, is_standing))
}

/// The first `finalists` of `found`, the way the model stands and the best pose set on the
/// plate, each once: the search cannot weigh a seat's supports as surely as a lift's. The
/// best that fits the build volume is among them whenever one does.
fn to_print(
    found: &[Oriented],
    finalists: usize,
    is_standing: impl Fn(&Oriented) -> bool,
) -> Vec<&Oriented> {
    let best_seat = found.iter().position(|pose| pose.on_plate);
    let mut poses: Vec<&Oriented> = Vec::new();
    for (at, pose) in found.iter().enumerate() {
        let wanted = at < finalists || is_standing(pose) || Some(at) == best_seat;
        let again = |kept: &&Oriented| {
            kept.on_plate == pose.on_plate && kept.rotation.abs_diff_eq(pose.rotation, 1e-5)
        };
        if wanted && !poses.iter().any(again) {
            poses.push(pose);
        }
    }
    if poses.iter().all(|pose| pose.overflows)
        && let Some(fitting) = found.iter().find(|pose| !pose.overflows)
    {
        poses.push(fitting);
    }
    poses
}

/// `tried` ranked for `preset` on what each printed, best first, with what decided its place.
fn judged(
    tried: &[Proven],
    preset: OrientPreset,
    is_standing: impl Fn(&Oriented) -> bool,
) -> Vec<Tried> {
    let noise = OrientGoal::ALL.map(Printout::noise);
    let to_noise = |pose: &Proven| match preset {
        OrientPreset::Goal(goal) => (pose.printout.reading(goal) / noise[goal.index()]).round(),
        OrientPreset::Auto => 0.0,
    };
    let readings: Vec<Readings> = tried
        .iter()
        .enumerate()
        .map(|(at, pose)| Readings {
            fatal: f64::from(u8::from(pose.printout.overflows)),
            // A model set on the plate holds itself up, so what hangs off it fails it as a cup.
            suction: (pose.printout.cups
                + usize::from(pose.oriented.on_plate && pose.printout.hanging > 0))
                as f64,
            tearing: pose.printout.peels as f64,
            unheld: Some(pose.printout.islands as f64),
            roomless: pose.printout.hanging as f64,
            goals: OrientGoal::ALL.map(|each| pose.printout.reading(each)),
            // The goal's reading to the noise of its unit, then the search's own order.
            tiebreak: [to_noise(pose), at as f64],
            standing: is_standing(&pose.oriented),
        })
        .collect();
    let rulings = rulings(&readings, preset, &noise);
    rank(&readings, preset, &noise)
        .into_iter()
        .map(|at| Tried {
            proven: tried[at],
            ruling: rulings[at],
        })
        .collect()
}

/// Stands the model the way `rotation` turns it in the middle of the plate, lifted onto
/// supports, or `on_plate` with supports only under what hangs over it, and reads the stack
/// it and its supports cure.
pub fn print_out(
    printed: &Printed,
    rotation: Quat,
    on_plate: bool,
    trial: &Trial,
) -> Result<Printout, EngineError> {
    print_held(printed, rotation, on_plate, trial, None)
}

/// The same cured as coarse as `trial`, for a `print` finer than it: supports are placed on
/// the print's own layer, and its islands counted at its own pixel and layer.
fn print_held(
    printed: &Printed,
    rotation: Quat,
    on_plate: bool,
    trial: &Trial,
    print: Option<&Trial>,
) -> Result<Printout, EngineError> {
    let turn = Transform {
        rotation,
        ..Transform::default()
    };
    let turned = transform_mesh(printed.shell.unwrap_or(printed.outer), turn);
    let bounds = turned.aabb().ok_or(EngineError::EmptyPlate)?;
    let volume = &trial.printer.build_volume;
    let offset = drop_to_plate(&bounds) + center_over_plate(&bounds, volume.x, volume.y);
    let moved = Transform {
        translation: offset,
        ..turn
    };
    let mut placed = transform_mesh(&turned, Transform::from_translation(offset));
    let mut cuts = printed.cuts.map(|cuts| transform_mesh(cuts, moved));

    let layer_mm = print.unwrap_or(trial).material.layer_height_mm;
    let stand = if on_plate { stand_seated } else { stand_under };
    let model = ToStand {
        mesh: &mut placed,
        cavity: None,
        cuts: &mut cuts,
        drill_exactly: false,
    };
    let stood = stand(model, trial.support, layer_mm)?;
    let mut held = Held {
        contacts: stood.contacts.len(),
        supports_mm3: signed_volume(&stood.supports).abs(),
        hanging: stood.contacts.len().saturating_sub(stood.standing),
        top_contacts: looking_up(&stood.facing, rotation),
        supports: (!stood.supports.faces.is_empty())
            .then(|| Arc::new(stood.supports))
            .into_iter()
            .collect(),
    };

    let overflows = overflows(&placed, &held.supports, &trial.printer.build_volume);
    let supports = std::mem::take(&mut held.supports);
    let (placed, cuts) = (Arc::new(placed), cuts.map(Arc::new));
    let plate = plate_of(&placed, cuts.as_ref(), &supports, trial);
    let measured = Run::measuring(&plate)?
        .measure(&mut ())?
        .ok_or(EngineError::EmptyPlate)?;
    let mut printout = printout(&held, &measured, trial.material.layer_height_mm);
    if let Some(print) = print {
        printout.islands = islands_printed(&placed, cuts.as_ref(), &supports, print)?;
    }
    Ok(Printout {
        overflows,
        ..printout
    })
}

/// The islands `placed` cures on its supports at the pixel and layer `print` prints at.
fn islands_printed(
    placed: &Arc<Mesh>,
    cuts: Option<&Arc<Mesh>>,
    supports: &[Arc<Mesh>],
    print: &Trial,
) -> Result<usize, EngineError> {
    let full = Run::measuring(&plate_of(placed, cuts, supports, print))?;
    islands_born(&full, placed)
}

/// Whether `placed` and its `supports` reach past `volume` however they are turned about
/// the vertical, which the plate is free to do.
fn overflows(placed: &Mesh, supports: &[Arc<Mesh>], volume: &BuildVolume) -> bool {
    let meshes = || std::iter::once(placed).chain(supports.iter().map(|support| &**support));
    let top_mm = meshes()
        .filter_map(Mesh::aabb)
        .map(|bounds| bounds.maxs.z)
        .fold(Scalar::NEG_INFINITY, Scalar::max);
    let plan: Vec<Vec2> = meshes()
        .flat_map(|mesh| mesh.vertices.iter().map(|vertex| vertex.truncate()))
        .collect();
    top_mm > volume.z || fitting_turn(&plan, Vec2::new(volume.x, volume.y)).is_none()
}

/// What stands under a pose, and what was found needing it.
struct Held {
    contacts: usize,
    supports_mm3: Scalar,
    hanging: usize,
    top_contacts: usize,
    supports: Vec<Arc<Mesh>>,
}

/// Contacts on a face that looks up in the model's own file, once `rotation` is taken
/// back out of the way each one faces.
fn looking_up(facing: &[Vec3], rotation: Quat) -> usize {
    let back = rotation.inverse();
    facing
        .iter()
        .filter(|facing| (back * **facing).z > LOOKS_UP)
        .count()
}

/// One model and its supports on the plate, cut and cured the way `trial` prints.
fn plate_of(
    placed: &Arc<Mesh>,
    cuts: Option<&Arc<Mesh>>,
    supports: &[Arc<Mesh>],
    trial: &Trial,
) -> Plate {
    Plate {
        models: vec![Model {
            mesh: Arc::clone(placed),
            transform: Transform::default(),
            cuts: cuts.cloned(),
            drilled: None,
            supports: supports.to_vec(),
        }],
        printer: trial.printer.clone(),
        material: trial.material.clone(),
        panel: PanelOverrides::default(),
        cutting: Cutting::uniform(trial.material.layer_height_mm),
        exposure: ExposurePlan::default(),
        remove_islands: false,
        hanging_passes: 0,
        format: SlicedFormat::Goo,
        raster_window: trial.raster_window,
        created_unix_s: 0,
    }
}

/// What `measured` cured, cut uniformly at `layer_mm`, and what `held` stood under it.
fn printout(held: &Held, measured: &Measured, layer_mm: Scalar) -> Printout {
    let risks = measured.risks();
    let count = |wanted: fn(&RiskKind) -> bool| risks.iter().filter(|r| wanted(&r.kind)).count();
    Printout {
        contacts: held.contacts,
        supports_ml: held.supports_mm3 / 1000.0,
        hanging: held.hanging,
        top_contacts: held.top_contacts,
        layers: measured.layer_count(),
        stack_mm: measured.layer_count() as Scalar * layer_mm,
        pull_n: measured
            .hardest_pull()
            .map_or(0.0, |peak| f64::from(peak.value)),
        growth_mm2: measured.largest_growth().map_or(0.0, |peak| peak.value),
        islands: count(|kind| matches!(kind, RiskKind::Island { .. })),
        cups: count(|kind| matches!(kind, RiskKind::SuctionCup { .. })),
        peels: count(|kind| matches!(kind, RiskKind::Peel { .. })),
        resin_ml: measured.volume_mm3() / 1000.0,
        overflows: false,
    }
}

#[cfg(test)]
mod tests {
    use core_plate::Score;

    use super::*;

    /// A pose that prints `supports_ml`, on the plate or lifted, with `hanging` contacts.
    fn tried(on_plate: bool, supports_ml: f32, hanging: usize) -> Proven {
        Proven {
            oriented: Oriented {
                rotation: Quat::from_rotation_x(if on_plate { 0.0 } else { 1.0 }),
                score: Score {
                    overhang_mm2: 0.0,
                    low_points: 0,
                    support_mm3: 0.0,
                    under_mm2: 0.0,
                    under_mm3: 0.0,
                    bottom_mm2: 0.0,
                    height_mm: 0.0,
                    shadow_mm2: 0.0,
                    peak_mm2: None,
                    peak_pull_n: None,
                    cup_mm2: None,
                    lift_mm: 0.0,
                    total: 0.0,
                },
                on_plate,
                overflows: false,
                considered: 0,
                ruling: Ruling::Chosen,
            },
            printout: Printout {
                supports_ml,
                hanging,
                pull_n: 5.0,
                stack_mm: 20.0,
                ..Printout::default()
            },
        }
    }

    #[test]
    fn a_seat_with_something_hanging_fails_like_a_cup_but_not_the_fewest_supports() {
        let poses = [tried(true, 0.5, 1), tried(false, 2.0, 0)];
        let first = |preset| {
            judged(&poses, preset, |_| false)[0]
                .proven
                .oriented
                .on_plate
        };
        assert!(!first(OrientPreset::Goal(OrientGoal::LeastPeel)));
        assert!(first(OrientPreset::Goal(OrientGoal::FewestSupports)));
    }

    #[test]
    fn the_best_pose_that_fits_is_printed_when_no_finalist_does() {
        let mut found: Vec<Oriented> = (0..6)
            .map(|at| Oriented {
                rotation: Quat::from_rotation_x(0.3 * at as Scalar),
                ..tried(false, 1.0, 0).oriented
            })
            .collect();
        for pose in &mut found[..5] {
            pose.overflows = true;
        }
        let poses = to_print(&found, 4, |_| false);
        assert!(
            poses.iter().any(|pose| !pose.overflows),
            "the sixth pose, the only one that fits, is printed too"
        );
        assert_eq!(poses.len(), 5);
    }

    #[test]
    fn a_bar_longer_than_the_plate_fits_across_its_diagonal() {
        // At 45 degrees a 100 by 4 mm bar spans (100 + 4) / sqrt 2 = 74 mm each way.
        let corners = (0..8)
            .map(|at| {
                let unit = Vec3::new(
                    (at & 1) as Scalar,
                    ((at >> 1) & 1) as Scalar,
                    (at >> 2) as Scalar,
                );
                unit * Vec3::new(100.0, 4.0, 4.0)
            })
            .collect();
        let bar = Mesh::new(corners, vec![[0, 1, 2]]);
        let plate = |z: Scalar| BuildVolume {
            x: 90.0,
            y: 90.0,
            z,
        };
        assert!(!overflows(&bar, &[], &plate(10.0)));
        assert!(
            overflows(&bar, &[], &plate(3.0)),
            "4 mm tall in a 3 mm volume"
        );
    }
}
