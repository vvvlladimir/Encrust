//! A pose judged by what it prints: stood on its supports, cut and cured, and read the
//! way a finished plate is. See `docs/design/orientation.md`.

use std::sync::Arc;

use core_analysis::{Measured, PEEL_N_PER_MM4, RiskKind};
use core_format::ExposurePlan;
use core_geometry::{
    Mesh, Quat, Scalar, Transform, Vec3, center_over_plate, drop_to_plate, transform_mesh,
};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_plate::{
    LOOKS_UP, OrientGoal, OrientSettings, Oriented, Printed, Readings, Ruling, finalists, rank,
    rotation_for, rulings,
};
use printer_profiles::{MaterialProfile, PrinterProfile, SupportProfile};
use rayon::prelude::*;

use crate::cut::Cutting;
use crate::error::EngineError;
use crate::plate::{Model, Plate};
use crate::run::Run;
use crate::stand::{left_hanging, stand_under};

/// Poses printed to choose between, on top of the way the model stands.
const FINALISTS: usize = 4;

/// What a suction cup or a tearing pull weighs against an island or a point left hanging:
/// the first two lose the whole print, the others a detail of it.
const FATAL: usize = 10;

/// What of a goal's own reading is noise once printed: a contact or two, half a newton, a
/// few layers.
fn slack(goal: OrientGoal) -> f64 {
    match goal {
        OrientGoal::FewestSupports | OrientGoal::CleanTop => 2.0,
        OrientGoal::LeastPeel => 0.5,
        OrientGoal::ShortestPrint => 5.0,
    }
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
    pub support: &'a SupportProfile,
    /// Layers rasterised at once, as `Plate::raster_window`.
    pub raster_window: usize,
}

/// What a pose costs once it is supported, cut and cured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Printout {
    pub contacts: usize,
    /// Places that need holding up and are not: contacts with no room for a column, and
    /// for a model set on the plate everything placement would have stood a support under.
    pub hanging: usize,
    /// Contacts on faces that look up in the model's own file.
    pub top_contacts: usize,
    pub layers: usize,
    /// The hardest pull of any layer on the film, newtons.
    pub pull_n: f64,
    /// The largest step in cured area from one layer to the next, square millimetres.
    pub growth_mm2: f32,
    pub islands: usize,
    pub cups: usize,
    pub peels: usize,
    pub resin_ml: f32,
}

impl Printout {
    /// What loses the whole print rather than a detail of it: suction cups and tearing
    /// pulls.
    pub fn fatal(&self) -> usize {
        self.cups + self.peels
    }

    /// What in the pose would fail the print, a cup or a tearing pull counted `FATAL`
    /// times over.
    pub fn failures(&self) -> usize {
        FATAL * self.fatal() + self.islands + self.hanging
    }

    /// The reading `goal` is meant to win, lower being better.
    pub fn reading(&self, goal: OrientGoal) -> f64 {
        match goal {
            OrientGoal::FewestSupports => self.contacts as f64,
            OrientGoal::LeastPeel => self.pull_n,
            OrientGoal::ShortestPrint => self.layers as f64,
            OrientGoal::CleanTop => self.top_contacts as f64,
        }
    }
}

/// An orientation chosen by printing the search's best few, and what it prints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proven {
    pub oriented: Oriented,
    pub printout: Printout,
}

/// One finalist printed, and what decided its place: `None` for a pose set on the plate
/// that left anything hanging, which is not ranked as one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tried {
    pub proven: Proven,
    pub ruling: Option<Ruling>,
}

/// Finds the orientation a model prints best in, by printing: the search's
/// [`finalists`], then [`prove`].
pub fn orient_proven(
    printed: &Printed,
    settings: &OrientSettings,
    trial: &Trial,
) -> Result<Proven, EngineError> {
    let found = finalists(printed, settings, usize::MAX)?;
    prove(printed, settings, &found, trial)?
        .first()
        .map(|tried| tried.proven)
        .ok_or(EngineError::EmptyPlate)
}

/// Prints the search's best few of `found` and the way the model stands, each supported,
/// cut and cured, with every pose that may rest on the plate tried there bare, and answers
/// them ranked by [`core_plate::rank`] on what they printed, the search's own order
/// breaking ties: the first is the choice, the rest are why. The trials are cut coarse, so
/// a printout is a ranking's and not the print's: [`print_out`] gives that.
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
    let (printer, material) = coarse(trial, reach_mm);
    let trial = &Trial {
        printer: &printer,
        material: &material,
        ..*trial
    };
    let standing = rotation_for(settings.down_now);
    let is_standing = |pose: &Oriented| pose.rotation.abs_diff_eq(standing, 1e-5);
    let poses: Vec<&Oriented> = found
        .iter()
        .enumerate()
        .filter(|(at, pose)| *at < FINALISTS || pose.on_plate || is_standing(pose))
        .map(|(_, pose)| pose)
        .collect();
    let printed_out = poses
        .into_par_iter()
        .map(|pose| {
            print_out(printed, pose.rotation, pose.on_plate, trial).map(|printout| Proven {
                oriented: *pose,
                printout,
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;
    let seated = |pose: &Proven| !pose.oriented.on_plate || pose.printout.hanging == 0;
    let (tried, unseated): (Vec<Proven>, Vec<Proven>) = printed_out.into_iter().partition(seated);

    let readings: Vec<Readings> = tried
        .iter()
        .enumerate()
        .map(|(at, pose)| Readings {
            fatal: pose.printout.fatal() as f64,
            unheld: Some((pose.printout.islands + pose.printout.hanging) as f64),
            primary: pose.printout.reading(settings.goal),
            // The search's own order breaks a tie.
            tiebreak: at as f64,
            standing: is_standing(&pose.oriented),
        })
        .collect();
    let slack = slack(settings.goal);
    let rulings = rulings(&readings, slack);
    let ranked = rank(&readings, slack).into_iter().map(|at| Tried {
        proven: tried[at],
        ruling: Some(rulings[at]),
    });
    let unranked = unseated.into_iter().map(|proven| Tried {
        proven,
        ruling: None,
    });
    Ok(ranked.chain(unranked).collect())
}

/// Stands the model the way `rotation` turns it in the middle of the plate, on supports
/// unless it is `on_plate`, and reads the stack it and its supports cure.
pub fn print_out(
    printed: &Printed,
    rotation: Quat,
    on_plate: bool,
    trial: &Trial,
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

    let layer_mm = trial.material.layer_height_mm;
    let held = if on_plate {
        Held {
            hanging: left_hanging(&placed, cuts.as_ref(), trial.support, layer_mm)?,
            ..Held::default()
        }
    } else {
        let stood = stand_under(&mut placed, &mut cuts, trial.support, layer_mm)?;
        Held {
            contacts: stood.contacts.len(),
            hanging: stood.contacts.len().saturating_sub(stood.standing),
            top_contacts: looking_up(&stood.facing, rotation),
            supports: vec![Arc::new(stood.supports)],
        }
    };

    let plate = plate_of(placed, cuts, held.supports, trial);
    let measured = Run::of(&plate)?
        .measure(&mut ())?
        .ok_or(EngineError::EmptyPlate)?;
    Ok(printout(
        [held.contacts, held.hanging, held.top_contacts],
        &measured,
    ))
}

/// What stands under a pose, and what was found needing it.
#[derive(Default)]
struct Held {
    contacts: usize,
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
fn plate_of(placed: Mesh, cuts: Option<Mesh>, supports: Vec<Arc<Mesh>>, trial: &Trial) -> Plate {
    Plate {
        models: vec![Model {
            mesh: Arc::new(placed),
            transform: Transform::default(),
            cuts: cuts.map(Arc::new),
            supports,
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

fn printout([contacts, hanging, top_contacts]: [usize; 3], measured: &Measured) -> Printout {
    let risks = measured.risks();
    let count = |wanted: fn(&RiskKind) -> bool| risks.iter().filter(|r| wanted(&r.kind)).count();
    Printout {
        contacts,
        hanging,
        top_contacts,
        layers: measured.layer_count(),
        pull_n: measured
            .hardest_pull()
            .map_or(0.0, |peak| f64::from(peak.value) * PEEL_N_PER_MM4),
        growth_mm2: measured.largest_growth().map_or(0.0, |peak| peak.value),
        islands: count(|kind| matches!(kind, RiskKind::Island { .. })),
        cups: count(|kind| matches!(kind, RiskKind::SuctionCup { .. })),
        peels: count(|kind| matches!(kind, RiskKind::Peel { .. })),
        resin_ml: measured.volume_mm3() / 1000.0,
    }
}
