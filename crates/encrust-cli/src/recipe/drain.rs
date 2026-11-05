//! The drain step: `--drain-at auto`'s proven search over the cavity, or a hole at each
//! point of the placed box snapped onto the surface as `--drain-at` snaps one.

use anyhow::{Context, Result, bail};
use core_engine::planned_lift;
use core_geometry::{Bvh, Scalar, Vec3};
use core_recipe::Metric;
use core_volume::{HoleKind, HoleSize};

use super::piece::Piece;
use super::settings::{Settings, flag, number, points, word};
use super::{Ctx, Did};
use crate::hollowing::{AutoReport, Ground, HollowArgs, auto_drains, hole_near};

/// Drills the holes the step asks for and notes what is left trapped.
pub(super) fn drain(piece: &mut Piece, settings: &Settings, ctx: &Ctx) -> Result<Did> {
    // TODO(step-6): plugs, as `--drain-plugs` lays them beside the model.
    if flag(settings, "plugs") == Some(true) {
        bail!("plugs = true is not supported yet");
    }
    let typed = piece.shaping.hollow.hole_size();
    let size = HoleSize {
        diameter_mm: number(settings, "diameter_mm").unwrap_or(typed.diameter_mm),
        depth_mm: number(settings, "depth_mm").unwrap_or(typed.depth_mm),
        ..typed
    };
    let drilled = piece.holes.len();
    let did = match word(settings, "at") {
        Some("points") => at_points(piece, points(settings, "points"), size, ctx)?,
        _ => {
            let vent = flag(settings, "vent").unwrap_or(true);
            proven(piece, (size, vent), ctx)?
        }
    };
    Ok(Did {
        holes: Some(piece.holes.len() - drilled),
        ..did
    })
}

/// A hole on the surface nearest each of `points`, fractions of the box the model stands in:
/// a recess with the wall kept round it on a model not hollowed yet, a drain on a shell.
fn at_points(piece: &mut Piece, points: &[[Scalar; 3]], size: HoleSize, ctx: &Ctx) -> Result<Did> {
    let bounds = piece.mesh.aabb().context("mesh has no vertices")?;
    let bvh = Bvh::build(&piece.mesh);
    let kind = match piece.cavity {
        Some(_) => HoleKind::Drain,
        None => HoleKind::Walled,
    };
    for point in points {
        let near = bounds.mins + Vec3::from_array(*point) * bounds.extents();
        let hole = hole_near(&piece.mesh, &bvh, (near, Vec3::ZERO), size, kind)?;
        piece.holes.push(hole);
    }
    let trapped = piece.trapped(ctx.layer_height_mm, ctx.watch.stop)?;
    piece.note(Metric::Trapped, trapped as Scalar);
    Ok(Did::default())
}

/// The holes the engine proves drain every pocket of the cavity, a vent on each where `vent`.
fn proven(piece: &mut Piece, (size, vent): (HoleSize, bool), ctx: &Ctx) -> Result<Did> {
    if piece.cavity.is_none() {
        piece.note(Metric::Trapped, 0.0);
        return Ok(Did::nothing("no cavity to drain"));
    }
    let args = HollowArgs {
        drain_mm: size.diameter_mm,
        drain_depth_mm: size.depth_mm,
        no_vent: !vent,
        ..piece.shaping.hollow.clone()
    };
    let ground = ground_of(piece, ctx)?;
    let placed = (piece.holes.as_slice(), &[][..]);
    let drained = auto_drains(&piece.mesh, placed, &args, piece.wall, ground)?;
    if ctx.watch.talk {
        print!("{}", AutoReport(&drained, ground.lifted_mm));
    }
    piece.holes.extend(drained.drain_holes());
    piece.note(Metric::Trapped, drained.trapped.len() as Scalar);
    piece.drains = Some(drained);
    piece.prepared.drained = true;
    Ok(Did::default())
}

/// Where the model prints as its holes are looked for: lifted by the supports it stands on,
/// or by those a later step of the recipe stands it on, else resting on the plate.
fn ground_of(piece: &Piece, ctx: &Ctx) -> Result<Ground> {
    let layer_height_mm = ctx.layer_height_mm;
    if piece.supports.is_some() {
        return Ok(Ground {
            layer_height_mm,
            on_plate: false,
            lifted_mm: 0.0,
        });
    }
    let Some(plan) = ctx.support_plan(piece)? else {
        return Ok(Ground {
            layer_height_mm,
            on_plate: true,
            lifted_mm: 0.0,
        });
    };
    let lift = planned_lift(&piece.mesh, &plan, piece.seated, layer_height_mm)
        .context("cannot work out how far the supports stand the model up")?;
    Ok(Ground {
        layer_height_mm,
        on_plate: lift.resting,
        lifted_mm: lift.lifted_mm,
    })
}
