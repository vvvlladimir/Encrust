//! The steps a recipe runs once on the whole plate: `check` measures it as the report does,
//! `arrange` packs it as `--arrange` does, `save` writes it there and then.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::{
    CupVent, DEFAULT_TEMPLATE, Hanging, SupportPlan, Unvented, cup_vents, island_floors,
    island_holds, lever_holds, neck_profile,
};
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};
use core_plate::ArrangeSettings;
use core_recipe::{Metric, retry, save_folder};
use core_volume::{DrainHole, HoleKind, HoleSize, ModelHollow};

use super::piece::Piece;
use super::settings::Settings;
use super::steps::{checked, hold_up};
use super::{Ctx, Did, Run, Saved};
use crate::batch::sliced_name;
use crate::commands::printer::{machine_of, send_to};
use crate::estimate::{Print, measure_plate};
use crate::pipeline::{Watch, raster_window, slice_staged};
use crate::profiles::{AUTO, plan_of};
use crate::slice_report::SliceReport;
use crate::stage::{Staged, arranged, cleared, staged};
use core_recipe::resolved::{flag, number, text};

impl Run<'_> {
    /// The plate as the pieces stand now, staged to be cut.
    fn staged(&self) -> Result<Staged> {
        let mut models = self
            .pieces
            .iter()
            .map(Piece::model)
            .collect::<Result<Vec<_>>>()?;
        models.extend(self.pieces.iter().flat_map(Piece::plug_models));
        let drainage = self
            .pieces
            .iter()
            .any(|piece| piece.cavity.is_some() || !piece.holes.is_empty());
        let ctx = self.ctx;
        let machine = (ctx.job, ctx.chosen, ctx.layer_height_mm);
        Ok(staged((Vec::new(), models), drainage, machine, false))
    }

    /// Answers the cups and necks `settings` ask for, then measures, notes and supports the
    /// islands of `ids` while step `index`'s retry holds; says how many times it went again.
    pub(super) fn check(&mut self, index: usize, ids: &[usize], settings: &Settings) -> Result<u8> {
        self.fix_found(ids, settings)?;
        let mut attempt = 0;
        loop {
            let (slice, print) = self.measured()?;
            let floors = island_floors(&print.measured.risks(), slice.plan(), &print.panel);
            let layer_mm = slice.plan().nominal_thickness();
            let owners = self.owners(&floors, layer_mm);
            let peel_n = print.measured.hardest_pull().map_or(0.0, |peak| peak.value);
            for &id in ids {
                let islands = owners
                    .iter()
                    .filter(|owner| owner.is_none_or(|(owner, _)| owner == id))
                    .count();
                let trapped = self.pieces[id].trapped(layer_mm, self.ctx.watch.stop)?;
                let piece = &mut self.pieces[id];
                checked(piece, islands, peel_n);
                piece.note(Metric::Trapped, trapped as Scalar);
            }
            if !self.retries(index, ids, attempt)? {
                return Ok(attempt);
            }
            self.hold_islands(ids, &owners)?;
            attempt += 1;
        }
    }

    fn measured(&self) -> Result<(SliceReport, Print)> {
        let mut staged = self.staged()?;
        let window = raster_window(&self.ctx.job.raster);
        let plate = staged.take_plate(None, window).context(
            "a check cuts and measures the plate, which needs a printer: name it with \
             --printer or --profile",
        )?;
        measure_plate(plate, staged.drainage, &quiet(self.ctx))
    }

    /// The piece each island of `floors` hangs from and the point a support there touches,
    /// or `None` for one hanging from none.
    fn owners(&self, floors: &[Vec3], layer_mm: Scalar) -> Vec<Option<(usize, Vec3)>> {
        let bvhs = self.bvhs();
        island_holds(floors, layer_mm, &self.hanging(&bvhs))
    }

    fn bvhs(&self) -> Vec<Bvh> {
        self.pieces
            .iter()
            .map(|piece| Bvh::build(&piece.mesh))
            .collect()
    }

    fn hanging<'a>(&'a self, bvhs: &'a [Bvh]) -> Vec<Hanging<'a>> {
        self.pieces
            .iter()
            .zip(bvhs)
            .map(|(piece, bvh)| (&piece.mesh, bvh, Transform::default()))
            .collect()
    }

    /// Drills the suction cups and stands supports under the weak necks of `ids` where
    /// `settings` ask for it; a cup or neck with nothing to answer it is left as it is.
    fn fix_found(&mut self, ids: &[usize], settings: &Settings) -> Result<()> {
        let cups = flag(settings, "drill_cups").unwrap_or(false);
        let levers = flag(settings, "hold_levers").unwrap_or(false);
        if !cups && !levers {
            return Ok(());
        }
        let (slice, print) = self.measured()?;
        let risks = print.measured.risks();
        let bvhs = self.bvhs();
        let models = self.hanging(&bvhs);
        let widest_hole_mm = ids
            .iter()
            .map(|&id| self.pieces[id].shaping.hollow.hole_size().diameter_mm)
            .reduce(Scalar::max);
        let vents = match (cups, widest_hole_mm) {
            (true, Some(hole_mm)) => {
                cup_vents(&risks, slice.plan(), &print.panel, &models, hole_mm)
            }
            _ => Vec::new(),
        };
        let necks = match (levers, ids.first()) {
            (true, Some(&first)) => {
                let sizing = self.neck_plan(first)?.placing();
                let holds = lever_holds(&risks, slice.plan(), &print.panel, &models, &sizing);
                Some(holds.into_iter().flatten().collect::<Vec<_>>())
            }
            _ => None,
        };
        for vent in vents {
            match vent {
                Ok(vent) if ids.contains(&vent.model) => self.vent(&bvhs, vent),
                Ok(_) => {}
                Err(Unvented::OnThePlate) => tracing::warn!(
                    "a suction cup stands on the plate: lift the model onto supports or turn it"
                ),
                Err(Unvented::TooDeep) => {
                    tracing::warn!("a suction cup has no wall thin enough to drill a vent through");
                }
            }
        }
        match necks {
            Some(holds) => self.hold_necks(ids, &holds),
            None => Ok(()),
        }
    }

    /// Drills `vent` into its piece along its own axis, walled on a solid one.
    fn vent(&mut self, bvhs: &[Bvh], vent: CupVent) {
        let piece = &mut self.pieces[vent.model];
        let size = HoleSize {
            depth_mm: vent.depth_mm,
            ..piece.shaping.hollow.hole_size()
        };
        let bvh = &bvhs[vent.model];
        let at = (vent.mouth_mm, vent.outward);
        let hole = ModelHollow::drain_at(&piece.mesh, bvh, at.0, at.1, size, Transform::default());
        match hole.filter(|hole| !hole.lies_within(&piece.holes)) {
            Some(hole) => piece.holes.push(DrainHole {
                kind: match piece.cavity {
                    Some(_) => HoleKind::AtDepth,
                    None => HoleKind::Walled,
                },
                ..hole
            }),
            None => tracing::warn!("{}: a cup was left undrilled", piece.input.display()),
        }
    }

    /// What the supports under a weak neck of `id` stand to: its own, made thick enough.
    fn neck_plan(&self, id: usize) -> Result<SupportPlan> {
        let piece = &self.pieces[id];
        let plan = match piece.plan.clone().or(piece.shaping.supports.clone()) {
            Some(plan) => plan,
            None => plan_of(AUTO)?,
        };
        Ok(SupportPlan::of(neck_profile(&plan.placing())))
    }

    /// The supports `holds` names on each of `ids`, to that piece's own plan made thick enough.
    fn hold_necks(&mut self, ids: &[usize], holds: &[(usize, Vec3)]) -> Result<()> {
        for &id in ids {
            let found: Vec<Vec3> = holds
                .iter()
                .filter(|(owner, _)| *owner == id)
                .map(|(_, at)| *at)
                .collect();
            if !found.is_empty() {
                let plan = self.neck_plan(id)?;
                hold_up(&mut self.pieces[id], &plan, &found)?;
            }
        }
        Ok(())
    }

    fn retries(&self, index: usize, ids: &[usize], attempt: u8) -> Result<bool> {
        for &id in ids {
            let metrics = self.metrics_of(&self.pieces[id]);
            if retry(&self.asked.plan, index, &metrics, attempt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A support under every island `owners` names on one of `ids`, added to what each
    /// stands on, as Check's Support every island stands one.
    fn hold_islands(&mut self, ids: &[usize], owners: &[Option<(usize, Vec3)>]) -> Result<()> {
        for &id in ids {
            let found: Vec<Vec3> = owners
                .iter()
                .flatten()
                .filter(|(owner, _)| *owner == id)
                .map(|(_, at)| *at)
                .collect();
            if found.is_empty() {
                continue;
            }
            let piece = &mut self.pieces[id];
            let plan = match piece.plan.clone().or(piece.shaping.supports.clone()) {
                Some(plan) => plan,
                None => plan_of(AUTO)?,
            };
            hold_up(piece, &plan, &found)?;
        }
        Ok(())
    }

    /// Packs every piece over the plate as `--arrange` does, `spacing_mm` apart.
    pub(super) fn arrange(&mut self, settings: &Settings) -> Result<()> {
        let default = ArrangeSettings::default();
        let asked = ArrangeSettings {
            clearance_mm: number(settings, "spacing_mm").unwrap_or(default.clearance_mm),
            ..default
        };
        let standing: Vec<Mesh> = self.pieces.iter().map(standing_mesh).collect();
        let footprints: Vec<(&Mesh, &Path)> = standing
            .iter()
            .zip(&self.pieces)
            .map(|(mesh, piece)| (mesh, piece.input.as_path()))
            .collect();
        let printer = self.ctx.chosen.printer.as_ref();
        let moves = match flag(settings, "only_in_the_way").unwrap_or(false) {
            true => cleared(&footprints, printer, &asked)?,
            false => arranged(&footprints, printer, &asked)?,
        };
        for (piece, moved) in self.pieces.iter_mut().zip(moves) {
            piece.moved(moved);
        }
        Ok(())
    }

    /// Writes the plate into the step's folder under its name template, and sends it on to
    /// the printer where it asks; `estimate` writes nothing.
    pub(super) fn save(&mut self, settings: &Settings) -> Result<Did> {
        let ctx = self.ctx;
        if !ctx.watch.writes {
            return Ok(Did::nothing("estimate writes nothing"));
        }
        let folder = folder_of(text(settings, "folder"))?;
        let template = text(settings, "name").unwrap_or(DEFAULT_TEMPLATE);
        let named = self.pieces.first().map(|piece| piece.input.clone());
        let when = jiff::Zoned::now().datetime();
        let machine = (ctx.chosen, ctx.layer_height_mm);
        let (_, file) = sliced_name(template, &named.unwrap_or_default(), machine, when);
        std::fs::create_dir_all(&folder)
            .with_context(|| format!("cannot make the folder {}", folder.display()))?;
        let output = folder.join(file);
        let mut staged = self.staged()?;
        let (slice, raster) = slice_staged(&mut staged, &output, ctx.job, &quiet(ctx))?;
        if ctx.watch.talk {
            println!("Saved into {}", output.display());
        }
        if flag(settings, "send") == Some(true) {
            let machine = machine_of(ctx.chosen.printer.as_ref())?;
            let name = send_to(&machine, &output, ctx.watch.progress, ctx.watch.stop)?;
            if ctx.watch.talk {
                println!("{name} is on {}", machine.address);
            }
        }
        let note = output.display().to_string();
        self.saved = Some(Saved {
            output,
            slice,
            raster,
        });
        Ok(Did {
            saved: Some(note),
            ..Did::default()
        })
    }
}

/// A run's watch that says nothing: a plate cut inside a step is not the run's report.
fn quiet<'a>(ctx: &Ctx<'a>) -> Watch<'a> {
    Watch::quiet(ctx.watch.stop)
}

/// What `piece` covers of the plate with its supports, as one mesh.
fn standing_mesh(piece: &Piece) -> Mesh {
    let mut mesh = piece.mesh.clone();
    if let Some((_, supports)) = &piece.supports {
        mesh.append(supports);
    }
    mesh
}

/// The folder a save step names, `~` its user's home; the home without one.
fn folder_of(asked: Option<&str>) -> Result<PathBuf> {
    let home = std::env::home_dir();
    match (asked, home) {
        (Some(folder), home) => Ok(save_folder(folder, home.as_deref())),
        (None, Some(home)) => Ok(home),
        (None, None) => bail!("a save step with no folder saves into the home, and there is none"),
    }
}
