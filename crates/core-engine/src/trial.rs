//! A pose judged by what it prints: stood on its supports, cut and cured, and read the
//! way a finished plate is. See `docs/design/orientation.md`.

use std::sync::Arc;

use core_analysis::{Measured, PEEL_N_PER_MM4, RiskKind};
use core_format::ExposurePlan;
use core_geometry::{
    Bvh, Mesh, Quat, Scalar, Transform, Vec3, center_over_plate, drop_to_plate, transform_mesh,
};
use core_pipeline::{PanelOverrides, SlicedFormat};
use core_plate::{OrientGoal, OrientSettings, Oriented, Printed, finalists, rotation_for};
use printer_profiles::{MaterialProfile, PrinterProfile, SupportProfile};
use rayon::prelude::*;

use crate::cut::Cutting;
use crate::error::EngineError;
use crate::plate::{Model, Plate};
use crate::run::Run;
use crate::stand::{append, contacts_under, facing, stand_under, touched};

/// A face looks up in its file once its normal is within sixty degrees of +Z, the reading
/// `core-plate` takes of the side meant to be seen.
const LOOKS_UP: Scalar = 0.5;

/// Poses printed to choose between, on top of the way the model stands.
const FINALISTS: usize = 4;

/// What a suction cup or a tearing pull weighs against an island or a point left hanging:
/// the first two lose the whole print, the others a detail of it.
const FATAL: usize = 10;

/// The most places a pose set bare on the plate may leave hanging and still be a print
/// with no supports: a test piece's own overhangs, not a model's underside.
const BARE_HANGING: usize = 16;

/// The share of its own reading a turn has to win to be worth taking over standing still.
const GAIN: f64 = 0.05;

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

/// Finds the orientation a model prints best in, by printing: the search's best few and
/// the way the model stands are each supported, cut and cured, with every pose that may
/// rest on the plate tried there bare. Those with the fewest cups and tearing pulls are
/// kept, then those failing little more than the soundest, and of them the one that reads
/// best on the goal is taken. The model stays as it stands unless a turn
/// wins `GAIN` of that reading. The trials are cut coarse, so the printout answered is a
/// ranking's and not the print's: [`print_out`] gives that.
pub fn orient_proven(
    printed: &Printed,
    settings: &OrientSettings,
    trial: &Trial,
) -> Result<Proven, EngineError> {
    let found = finalists(printed, settings, usize::MAX)?;
    let sketch = Depth::Sketch {
        top: settings.goal == OrientGoal::CleanTop,
        layer_mm: trial.material.layer_height_mm,
    };
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
    let tried = poses
        .into_par_iter()
        .map(|pose| {
            tried(printed, pose.rotation, pose.on_plate, trial, sketch).map(|printout| Proven {
                oriented: *pose,
                printout,
            })
        })
        .filter(|tried| {
            tried.as_ref().map_or(true, |pose| {
                !pose.oriented.on_plate || pose.printout.hanging <= BARE_HANGING
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;

    // What loses the whole print comes before anything a goal asks for.
    let least_fatal = tried.iter().map(|pose| pose.printout.fatal()).min();
    let printable = |pose: &&Proven| Some(pose.printout.fatal()) == least_fatal;
    let reading = |pose: &Proven| pose.printout.reading(settings.goal);
    // The first of equals: the search's own order breaks a tie.
    let best = tried
        .iter()
        .filter(printable)
        .reduce(|best, pose| {
            if reading(pose) < reading(best) {
                pose
            } else {
                best
            }
        })
        .ok_or(EngineError::EmptyPlate)?;
    let stays = tried
        .iter()
        .filter(printable)
        .find(|pose| is_standing(&pose.oriented))
        .filter(|now| reading(now) - reading(best) <= GAIN * reading(now).abs());
    Ok(*stays.unwrap_or(best))
}

/// Stands the model the way `rotation` turns it in the middle of the plate, on supports
/// unless it is `on_plate`, and reads the stack it and its supports cure.
pub fn print_out(
    printed: &Printed,
    rotation: Quat,
    on_plate: bool,
    trial: &Trial,
) -> Result<Printout, EngineError> {
    tried(printed, rotation, on_plate, trial, Depth::Print)
}

/// How far a pose is taken before it is read.
#[derive(Clone, Copy)]
enum Depth {
    /// Supports grown and meshed under it, and cured with it.
    Print,
    /// Only where supports would touch, with the model cured alone: enough to rank a
    /// pose on, at a fraction of the cost. Its islands are not read, since nothing
    /// holds them. `top` says whether the faces the contacts land on are looked up, and
    /// `layer_mm` is the print's own layer, which what hangs off a bare pose is counted
    /// at: sixteen places is a line too fine for a coarse stack to draw.
    Sketch { top: bool, layer_mm: Scalar },
}

fn tried(
    printed: &Printed,
    rotation: Quat,
    on_plate: bool,
    trial: &Trial,
    depth: Depth,
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
    let mut held = if on_plate {
        Held {
            hanging: hanging_on_plate(
                &placed,
                cuts.as_ref(),
                trial.support,
                match depth {
                    Depth::Print => layer_mm,
                    Depth::Sketch { layer_mm, .. } => layer_mm,
                },
            )?,
            ..Held::default()
        }
    } else {
        match depth {
            Depth::Print => {
                let stood = stand_under(&mut placed, &mut cuts, trial.support, layer_mm)?;
                Held {
                    contacts: stood.contacts.len(),
                    hanging: stood.contacts.len().saturating_sub(stood.standing),
                    top_contacts: looking_up(&stood.facing, rotation),
                    under_layers: 0,
                    supports: vec![Arc::new(stood.supports)],
                }
            }
            Depth::Sketch { top, .. } => {
                let (lifted_mm, drilled, contacts) =
                    touched(&mut placed, &mut cuts, trial.support, layer_mm)?;
                let facing = if top {
                    facing(&drilled, &Bvh::build(&drilled), &contacts)
                } else {
                    Vec::new()
                };
                Held {
                    contacts: contacts.len(),
                    top_contacts: looking_up(&facing, rotation),
                    // Cured alone the stack starts at the model; its supports would
                    // start it at the plate.
                    under_layers: (lifted_mm / layer_mm).round() as usize,
                    ..Held::default()
                }
            }
        }
    };

    let supports = std::mem::take(&mut held.supports);
    let plate = plate_of(placed, cuts, supports, trial);
    let measured = Run::of(&plate)?
        .measure(&mut ())?
        .ok_or(EngineError::EmptyPlate)?;
    let mut read = printout([held.contacts, held.hanging, held.top_contacts], &measured);
    if matches!(depth, Depth::Sketch { .. }) && !on_plate {
        read.islands = 0;
        read.layers += held.under_layers;
    }
    Ok(read)
}

/// What stands under a pose, and what was found needing it.
#[derive(Default)]
struct Held {
    contacts: usize,
    hanging: usize,
    top_contacts: usize,
    /// Layers of support under the model that a sketch does not cure.
    under_layers: usize,
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

/// A face this near the lowest point of a model set on the plate is held by the plate.
const SEATED_MM: Scalar = 0.5;

/// How many supports a model set on the plate goes without: what placement would stand
/// under it lifted clear, less what would only have held its seat.
fn hanging_on_plate(
    placed: &Mesh,
    cuts: Option<&Mesh>,
    support: &SupportProfile,
    layer_mm: Scalar,
) -> Result<usize, EngineError> {
    let lift_mm = support.lift_mm();
    let lifted = Transform::from_translation(Vec3::Z * lift_mm);
    let mut drilled = transform_mesh(placed, lifted);
    if let Some(cuts) = cuts {
        append(&mut drilled, &transform_mesh(cuts, lifted));
    }
    let contacts = contacts_under(&drilled, support, layer_mm)?;
    Ok(contacts
        .iter()
        .filter(|contact| contact.z > lift_mm + SEATED_MM)
        .count())
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
