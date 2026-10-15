//! A pose judged by what it prints: stood on its supports, cut and cured, and read the
//! way a finished plate is. See `docs/design/orientation.md`.

use std::sync::Arc;

use core_analysis::{Measured, RiskKind};
use core_format::ExposurePlan;
use core_geometry::{
    Mesh, Quat, Scalar, Transform, Vec3, center_over_plate, drop_to_plate, signed_volume,
    transform_mesh,
};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_plate::{
    LOOKS_UP, OrientGoal, OrientSettings, Oriented, Printed, Readings, Ruling, finalists, rank,
    rotation_for, rulings,
};
use printer_profiles::{BuildVolume, MaterialProfile, PrinterProfile, SupportProfile};
use rayon::prelude::*;

use crate::cut::Cutting;
use crate::error::EngineError;
use crate::plate::{Model, Plate};
use crate::run::Run;
use crate::stand::{stand_seated, stand_under};

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

/// What a suction cup or a tearing pull weighs against an island or a point left hanging:
/// the first two lose the whole print, the others a detail of it.
const FATAL: usize = 10;

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
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Printout {
    pub contacts: usize,
    /// The resin in the supports, millilitres, as the plate's summary counts it.
    pub supports_ml: f32,
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

    /// What in the pose would fail the print, a cup or a tearing pull counted `FATAL`
    /// times over.
    pub fn failures(&self) -> usize {
        FATAL * self.fatal() + self.islands + self.hanging
    }

    /// The reading `goal` is meant to win, lower being better.
    pub fn reading(&self, goal: OrientGoal) -> f64 {
        match goal {
            OrientGoal::FewestSupports => f64::from(self.supports_ml),
            OrientGoal::LeastPeel => self.pull_n,
            OrientGoal::ShortestPrint => self.layers as f64,
            OrientGoal::CleanTop => self.top_contacts as f64,
        }
    }

    /// What of `goal`'s [`Printout::reading`] is noise: a strut's resin, millilitres, two
    /// contacts, half a newton, five layers.
    pub fn noise(goal: OrientGoal) -> f64 {
        match goal {
            OrientGoal::FewestSupports => 0.05,
            OrientGoal::CleanTop => 2.0,
            OrientGoal::LeastPeel => 0.5,
            OrientGoal::ShortestPrint => 5.0,
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
    let support_layer_mm = trial.material.layer_height_mm;
    let (printer, material) = coarse(trial, reach_mm);
    let trial = &Trial {
        printer: &printer,
        material: &material,
        ..*trial
    };
    let standing = rotation_for(settings.down_now);
    let is_standing = |pose: &Oriented| pose.rotation.abs_diff_eq(standing, 1e-5);
    let poses = to_print(
        found,
        finalists_for(printed.outer.faces.len(), reach_mm),
        is_standing,
    );
    let tried = poses
        .into_par_iter()
        .map(|pose| {
            print_held(
                printed,
                pose.rotation,
                pose.on_plate,
                trial,
                support_layer_mm,
            )
            .map(|printout| Proven {
                oriented: *pose,
                printout,
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;
    Ok(judged(&tried, settings.goal, is_standing))
}

/// The first `finalists` of `found`, the way the model stands and the best pose set on the
/// plate, each once: the search cannot weigh a seat's supports as surely as a lift's.
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
    poses
}

/// `tried` ranked for `goal` on what each printed, best first, with what decided its place.
fn judged(
    tried: &[Proven],
    goal: OrientGoal,
    is_standing: impl Fn(&Oriented) -> bool,
) -> Vec<Tried> {
    let noise = OrientGoal::ALL.map(Printout::noise);
    let slack = noise[goal.index()];
    let readings: Vec<Readings> = tried
        .iter()
        .enumerate()
        .map(|(at, pose)| Readings {
            fatal: f64::from(u8::from(pose.printout.overflows)),
            suction: pose.printout.fatal() as f64,
            unheld: Some(pose.printout.islands as f64),
            roomless: pose.printout.hanging as f64,
            goals: OrientGoal::ALL.map(|each| pose.printout.reading(each)),
            // The goal's reading to the noise of its unit, then the search's own order.
            tiebreak: [(pose.printout.reading(goal) / slack).round(), at as f64],
            standing: is_standing(&pose.oriented),
        })
        .collect();
    let rulings = rulings(&readings, goal, &noise);
    rank(&readings, goal, &noise)
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
    print_held(
        printed,
        rotation,
        on_plate,
        trial,
        trial.material.layer_height_mm,
    )
}

/// The same with supports placed on `support_layer_mm`, the print's own layer even when
/// `trial` cures a coarse stack.
fn print_held(
    printed: &Printed,
    rotation: Quat,
    on_plate: bool,
    trial: &Trial,
    support_layer_mm: Scalar,
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

    let stand = if on_plate { stand_seated } else { stand_under };
    let stood = stand(&mut placed, &mut cuts, trial.support, support_layer_mm)?;
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
    let plate = plate_of(placed, cuts, supports, trial);
    let measured = Run::measuring(&plate)?
        .measure(&mut ())?
        .ok_or(EngineError::EmptyPlate)?;
    Ok(Printout {
        overflows,
        ..printout(&held, &measured)
    })
}

/// Whether `placed` and its `supports` reach past `volume`, their two widths matched with
/// the plate's sorted: the plate is free to turn them about the vertical.
fn overflows(placed: &Mesh, supports: &[Arc<Mesh>], volume: &BuildVolume) -> bool {
    let Some(mut bounds) = placed.aabb() else {
        return false;
    };
    for support in supports.iter().filter_map(|support| support.aabb()) {
        bounds.mins = bounds.mins.min(support.mins);
        bounds.maxs = bounds.maxs.max(support.maxs);
    }
    let size = bounds.maxs - bounds.mins;
    let (narrow, wide) = (size.x.min(size.y), size.x.max(size.y));
    narrow > volume.x.min(volume.y) || wide > volume.x.max(volume.y) || bounds.maxs.z > volume.z
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

fn printout(held: &Held, measured: &Measured) -> Printout {
    let risks = measured.risks();
    let count = |wanted: fn(&RiskKind) -> bool| risks.iter().filter(|r| wanted(&r.kind)).count();
    Printout {
        contacts: held.contacts,
        supports_ml: held.supports_mm3 / 1000.0,
        hanging: held.hanging,
        top_contacts: held.top_contacts,
        layers: measured.layer_count(),
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
