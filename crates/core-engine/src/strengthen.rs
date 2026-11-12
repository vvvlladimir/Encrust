//! Automatic placement strengthened where what a tip carries is too much for it: the tip is
//! built to the next wider size, or given a neighbour at the widest (ADR 0422).

use core_analysis::{Cured, Load, Tip, TipLoads, cure};
use core_geometry::{Buckets, Bvh, Mesh, Ray, Scalar, Vec3};
use core_raster::{Grey, PixelPitch, RasterSettings, Rasterizer, ScanlineRasterizer, Shading};
use core_slicer::Sliced;
use core_supports::{Blocked, Need, Reason};
use printer_profiles::AUTO_SIZES;
use rayon::prelude::*;

use crate::error::EngineError;
use crate::stand::SupportPlan;

/// The margin a tip is wanted to keep over what pulls on it.
pub const WANTED_MARGIN: f64 = 1.5;
/// The most rounds placement is strengthened in, each a full read of the stack.
const MOST_ROUNDS: usize = 3;
/// The pitch the stack is read at, millimetres: a 0.3 mm tip and a 0.5 mm rod are six and
/// ten pixels across, and the panel's own pitch is unknown here.
const PITCH_MM: f32 = 0.05;
/// How far from a weak tip its neighbour stands, in the placement's spacing.
const NEIGHBOUR_SPACINGS: Scalar = 0.5;
/// The ways around a weak tip a neighbour is looked for.
const AROUND: [[Scalar; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
/// How near a contact already standing a neighbour may not seat, in its reach: that spot is held.
const TAKEN_SHARE: Scalar = 0.5;

/// What strengthening changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Strengthened {
    /// Rounds that changed anything.
    pub rounds: usize,
    /// Sizes tips were made heavier by, a size a round.
    pub thickened: usize,
    /// Tips added beside a weak one already at the widest.
    pub added: usize,
}

/// A model in plate coordinates and the stack automatic placement cut it into.
#[derive(Clone, Copy)]
pub struct Underside<'a> {
    pub stack: &'a Sliced,
    pub layer_mm: Scalar,
    pub surface: &'a Mesh,
    pub bvh: &'a Bvh,
    pub blocked: Option<&'a Blocked>,
}

/// Strengthens each of `needs` keeping under [`WANTED_MARGIN`] once built to `plan`, with
/// `held` already standing at the placing profile; stops when `keep_going` says so.
pub fn strengthen(
    model: Underside<'_>,
    needs: &mut Vec<Need>,
    held: &[Vec3],
    plan: &SupportPlan,
    keep_going: &mut dyn FnMut() -> bool,
) -> Result<Strengthened, EngineError> {
    let table = plan.table();
    let placing = plan.placing();
    let reach_mm = placing.spacing_mm() * NEIGHBOUR_SPACINGS;
    let mut taken = Buckets::new(reach_mm);
    let standing = needs
        .iter()
        .map(|need| need.contact)
        .chain(held.iter().copied());
    standing.for_each(|at| taken.insert(at.truncate(), at));
    let mut done = Strengthened::default();
    for _ in 0..MOST_ROUNDS {
        if !keep_going() {
            break;
        }
        let tips: Vec<Tip> = needs
            .iter()
            .map(|need| tip(need.contact, table_radius(&table, plan.group_of(need))))
            .chain(held.iter().map(|at| tip(*at, placing.surface_radius_mm())))
            .collect();
        let Some(loads) = carried(
            model.stack,
            model.surface,
            model.layer_mm,
            &tips,
            keep_going,
        )?
        else {
            break;
        };
        let before = done;
        for at in 0..needs.len() {
            let Some(worst_z_mm) = weak(&loads[at]) else {
                continue;
            };
            if heavier(plan, &needs[at]) {
                needs[at].heavier += 1;
                done.thickened += 1;
            } else if let Some(contact) =
                neighbour(model, needs[at].contact, (worst_z_mm, reach_mm), &taken)
            {
                taken.insert(contact.truncate(), contact);
                needs.push(Need {
                    contact,
                    ..needs[at]
                });
                done.added += 1;
            }
        }
        if done == before {
            break;
        }
        done.rounds += 1;
    }
    Ok(done)
}

/// What each of `tips` carries as `surface`, cut into `stack` of `layer_mm`, prints; `None`
/// once `keep_going`, asked between batches of layers, says stop.
pub fn carried(
    stack: &Sliced,
    surface: &Mesh,
    layer_mm: Scalar,
    tips: &[Tip],
    keep_going: &mut dyn FnMut() -> bool,
) -> Result<Option<Vec<Load>>, EngineError> {
    let Some(bounds) = surface.aabb() else {
        return Ok(Some(vec![Load::default(); tips.len()]));
    };
    let panel = panel_over(bounds.maxs.x, bounds.maxs.y);
    let mut fold = TipLoads::new(tips, layer_mm);
    for layers in stack.layers.chunks(rayon::current_num_threads().max(1)) {
        if !keep_going() {
            return Ok(None);
        }
        let cured: Vec<Cured> = layers
            .par_iter()
            .map(|layer| {
                let rastered = ScanlineRasterizer
                    .rasterize(layer, &panel)
                    .map_err(EngineError::Panel)?;
                Ok(cure(&rastered.runs, panel.pitch))
            })
            .collect::<Result<_, EngineError>>()?;
        for (layer, cured) in layers.iter().zip(cured) {
            fold.push(layer.z, cured);
        }
    }
    Ok(Some(fold.loads()))
}

fn tip(at: Vec3, radius_mm: f32) -> Tip {
    Tip {
        at_mm: [at.x, at.y],
        z_mm: at.z,
        radius_mm: f64::from(radius_mm),
    }
}

fn table_radius(table: &[printer_profiles::SupportProfile], group: u16) -> f32 {
    let at = usize::from(group).min(table.len().saturating_sub(1));
    table
        .get(at)
        .map_or(0.0, printer_profiles::SupportProfile::surface_radius_mm)
}

/// The height a tip is worst at, where it keeps under the wanted margin.
fn weak(load: &Load) -> Option<f32> {
    let margin = load.margin()?;
    (margin < WANTED_MARGIN).then_some(load.z_mm?)
}

/// Whether `plan` sizes `need` and has a size after its own with a wider tip; a
/// steadying support has none.
fn heavier(plan: &SupportPlan, need: &Need) -> bool {
    let Some(auto) = plan.sizing() else {
        return false;
    };
    if need.reason == Reason::Steady || need.shown {
        return false;
    }
    let size = need.size_in(auto);
    let table = plan.table();
    let tip_mm = |at: usize| table.get(at).map(|profile| profile.tip.contact_diameter_mm);
    size + 1 < AUTO_SIZES && tip_mm(size + 1) > tip_mm(size)
}

/// The lowest underside `reach_mm` round `tip`, free and held by none of `taken`, seated no
/// higher than `worst_z_mm`: one over the layer the tip is worst at comes too late to share it.
fn neighbour(
    model: Underside<'_>,
    tip: Vec3,
    (worst_z_mm, reach_mm): (f32, Scalar),
    taken: &Buckets<Vec3>,
) -> Option<Vec3> {
    AROUND
        .iter()
        .filter_map(|[x, y]| {
            // Starting a reach under the tip keeps a slope up to 45 degrees in front of the ray.
            let from = tip + Vec3::new(x * reach_mm, y * reach_mm, -reach_mm);
            let hit = model.bvh.raycast(&Ray::new(from, Vec3::Z))?;
            let faces_down = model.surface.triangle(hit.face)?.normal_unnormalized().z < 0.0;
            let point = from + Vec3::Z * hit.t;
            let free = model
                .blocked
                .is_none_or(|blocked| !blocked.covers(point, 0.0));
            let held = taken
                .around(point.truncate())
                .any(|other| other.distance(point) < reach_mm * TAKEN_SHARE);
            (faces_down && free && !held && point.z <= worst_z_mm).then_some(point)
        })
        .min_by(|a, b| a.z.total_cmp(&b.z))
}

/// An unmirrored panel from the plate's corner out past `x_mm`, `y_mm`, so its millimetres
/// are the plate's.
fn panel_over(x_mm: f32, y_mm: f32) -> RasterSettings {
    let pixels = |mm: f32| (mm.max(0.0) / PITCH_MM).ceil() as u32 + 2;
    RasterSettings {
        width_px: pixels(x_mm),
        height_px: pixels(y_mm),
        pitch: PixelPitch {
            x: PITCH_MM,
            y: PITCH_MM,
        },
        mirror_x: false,
        mirror_y: false,
        shading: Shading::Binary,
        grey: Grey::default(),
        blur_px: 0,
    }
}

#[cfg(test)]
mod tests;
