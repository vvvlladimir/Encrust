//! Check's answers on a saved plate: every object of it measured, those `--only` names
//! answered, as the window's Fix all on a selection (ADR 0465).

use std::path::Path;

use anyhow::{Context, Result};
use core_recipe::{Recipe, validate};

use super::piece::Piece;
use super::{Asked, Ctx, Run};
use crate::pipeline::saved_model_report;
use crate::stage::{Shaping, Staging};

/// `staging` with Check's answers the flags ask for given to the objects `--only` names, or
/// every one; the others weigh in on what hangs from what, untouched.
pub(crate) fn answered(staging: Staging, path: &Path, ctx: &Ctx) -> Result<Staging> {
    let Some(step) = ctx.job.fixes.step()? else {
        return Ok(staging);
    };
    let recipe = Recipe {
        name: "Check's answers".to_owned(),
        vars: Default::default(),
        steps: vec![step],
    };
    let asked = Asked {
        plan: validate(recipe).context("Check's answers cannot run")?,
        prepare: false,
    };
    let shaping = Shaping::of(ctx.job)?;
    let printer = ctx.chosen.printer.as_ref();
    let named = staging.every_target();
    let mut pieces = Vec::new();
    let mut targets = Vec::new();
    for id in staging.on_plate() {
        let mesh = staging
            .mesh(id)
            .context("an object of the plate has no surface")?;
        let import = saved_model_report(path, &mesh, printer)?;
        if named.contains(&id) {
            targets.push(pieces.len());
        }
        let entry = (path.to_path_buf(), shaping.clone(), true);
        pieces.push(Piece::new(entry, id, (import, None)));
    }
    let mut run = Run {
        asked: &asked,
        ctx,
        staging,
        pieces,
        targets,
        saved: None,
    };
    run.step(0)?;
    Ok(run.staging)
}
