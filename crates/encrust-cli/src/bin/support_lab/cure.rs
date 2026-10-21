//! The supported model cut and cured the way a print is, and read the way the Check tool
//! reads it: what hangs, what tears and what pulls.

use std::sync::Arc;

use anyhow::{Context, Result};
use core_analysis::{Measured, Risk, RiskKind};
use core_engine::{Cutting, Model, Plate, Run};
use core_format::ExposurePlan;
use core_geometry::{Mesh, Transform, Vec2};
use core_pipeline::{PanelOverrides, SlicedFormat};
use printer_profiles::{MaterialProfile, PrinterProfile};

/// What the cured stack says about one supported model.
#[derive(Debug, Clone, Default)]
pub struct Cured {
    pub layers: usize,
    pub resin_ml: f32,
    pub hardest_pull_n: f32,
    /// Every risk the stack names, with where it stands on the plate.
    pub risks: Vec<Placed>,
    pub seconds: f64,
}

/// A risk and where it stands, plate millimetres.
#[derive(Debug, Clone, Copy)]
pub struct Placed {
    pub risk: Risk,
    pub at: Vec2,
    pub z_mm: f32,
}

impl Cured {
    pub fn count(&self, wanted: fn(&RiskKind) -> bool) -> usize {
        self.risks
            .iter()
            .filter(|placed| wanted(&placed.risk.kind))
            .count()
    }

    /// The strongest neck stress any layer puts on what holds it, megapascals.
    pub fn worst_lever_mpa(&self) -> f32 {
        self.risks
            .iter()
            .filter_map(|placed| match placed.risk.kind {
                RiskKind::Lever { stress_mpa, .. } => Some(stress_mpa),
                _ => None,
            })
            .fold(0.0, f32::max)
    }
}

/// Cures `model` with `supports` on `printer` with `material`.
pub fn cure(
    model: &Mesh,
    supports: &Mesh,
    printer: &PrinterProfile,
    material: &MaterialProfile,
) -> Result<Cured> {
    let started = std::time::Instant::now();
    let plate = plate_of(model, supports, printer, material);
    let measured = Run::measuring(&plate)?
        .measure(&mut ())?
        .context("the plate cured nothing")?;
    let layer_mm = material.layer_height_mm;
    Ok(Cured {
        layers: measured.layer_count(),
        resin_ml: measured.volume_mm3() / 1000.0,
        hardest_pull_n: measured.hardest_pull().map_or(0.0, |peak| peak.value),
        risks: placed_risks(&measured, layer_mm),
        seconds: started.elapsed().as_secs_f64(),
    })
}

fn placed_risks(measured: &Measured, layer_mm: f32) -> Vec<Placed> {
    measured
        .risks()
        .into_iter()
        .map(|risk| Placed {
            risk,
            at: on_plate(risk.at_mm),
            z_mm: (risk.layer as f32 + 1.0) * layer_mm,
        })
        .collect()
}

/// Where a risk stands on the plate: the stack is measured against the plate, not the
/// mirrored panel, which the coupon's lopsided grid confirms.
fn on_plate(at_mm: [f32; 2]) -> Vec2 {
    Vec2::from(at_mm)
}

fn plate_of(
    model: &Mesh,
    supports: &Mesh,
    printer: &PrinterProfile,
    material: &MaterialProfile,
) -> Plate {
    let supports = (!supports.faces.is_empty())
        .then(|| Arc::new(supports.clone()))
        .into_iter()
        .collect();
    Plate {
        models: vec![Model {
            supports,
            ..Model::placed(Arc::new(model.clone()), Transform::default())
        }],
        printer: printer.clone(),
        material: material.clone(),
        panel: PanelOverrides::default(),
        cutting: Cutting::uniform(material.layer_height_mm),
        exposure: ExposurePlan::default(),
        remove_islands: false,
        hanging_passes: 0,
        format: SlicedFormat::Goo,
        raster_window: rayon::current_num_threads(),
        created_unix_s: 0,
    }
}
