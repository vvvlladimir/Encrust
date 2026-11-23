//! The steps a recipe runs once on the whole plate: `check` measures it as the report does and
//! answers what it finds by edits, `arrange` packs it by one, `save` writes it there and then.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_edit::Edit;
use core_engine::{
    CupVent, DEFAULT_TEMPLATE, Hanging, SupportPlan, Unvented, cup_vents, island_floors,
    island_holds, lever_holds, neck_profile,
};
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};
use core_plate::ArrangeSettings;
use core_recipe::{Metric, StepKind, edits_of, retry, save_folder};
use core_volume::{HoleKind, HoleSize};

use super::settings::Settings;
use super::steps::{checked, hold_up};
use super::{Ctx, Did, Run, Saved};
use crate::batch::sliced_name;
use crate::commands::printer::{machine_of, send_to};
use crate::estimate::{Print, measure_plate};
use crate::pipeline::{Watch, raster_window, slice_staged};
use crate::profiles::{AUTO, plan_of};
use crate::slice_report::SliceReport;
use crate::stage::{Staged, cleared, said, staged, usable_area};
use core_recipe::resolved::{flag, number, text};

impl Run<'_> {
    /// The plate as the document stands now, staged to be cut.
    fn staged(&self) -> Staged {
        let models = self.staging.models();
        let drainage = self
            .pieces
            .iter()
            .any(|piece| self.staging.hollowed(piece.id));
        let ctx = self.ctx;
        let machine = (ctx.job, ctx.chosen, ctx.layer_height_mm);
        staged((Vec::new(), models), drainage, machine, false)
    }

    /// Answers the cups and necks `settings` ask for, then measures, notes and supports the
    /// islands of `ids` while step `index`'s retry holds; says how many times it went again.
    pub(super) fn check(&mut self, index: usize, ids: &[usize], settings: &Settings) -> Result<u8> {
        let mut unchanged = self.fix_found(ids, settings)?;
        let mut attempt = 0;
        loop {
            let (slice, print) = match unchanged.take() {
                Some(measured) => measured,
                None => self.measured()?,
            };
            let floors = island_floors(&print.measured.risks(), slice.plan(), &print.panel);
            let layer_mm = slice.plan().nominal_thickness();
            let owners = self.owners(&floors, layer_mm);
            let peel_n = print.measured.hardest_pull().map_or(0.0, |peak| peak.value);
            for &id in ids {
                let islands = owners
                    .iter()
                    .filter(|owner| owner.is_none_or(|(owner, _)| owner == id))
                    .count();
                let trapped = self.pieces[id].trapped(&self.staging, self.ctx.watch.stop)?;
                let piece = &mut self.pieces[id];
                checked(piece, &self.staging, islands, peel_n);
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
        let mut staged = self.staged();
        let window = raster_window(&self.ctx.job.raster);
        let plate = staged.take_plate(None, window).context(
            "a check cuts and measures the plate, which needs a printer: name it with \
             --printer or --profile",
        )?;
        measure_plate(plate, staged.drainage, None, &quiet(self.ctx))
    }

    /// Each piece's model as it prints, standing on the plate.
    fn meshes(&self) -> Vec<Mesh> {
        let meshes = self.pieces.iter().map(|piece| self.staging.mesh(piece.id));
        meshes.map(Option::unwrap_or_default).collect()
    }

    /// The piece each island of `floors` hangs from and the point a support there touches,
    /// or `None` for one hanging from none.
    fn owners(&self, floors: &[Vec3], layer_mm: Scalar) -> Vec<Option<(usize, Vec3)>> {
        let meshes = self.meshes();
        let bvhs: Vec<Bvh> = meshes.iter().map(Bvh::build).collect();
        island_holds(floors, layer_mm, &hanging(&meshes, &bvhs))
    }

    /// Drills the suction cups and stands supports under the weak necks of `ids` where
    /// `settings` ask for it; hands back its measure when that changed nothing.
    fn fix_found(
        &mut self,
        ids: &[usize],
        settings: &Settings,
    ) -> Result<Option<(SliceReport, Print)>> {
        let cups = flag(settings, "drill_cups").unwrap_or(false);
        let levers = flag(settings, "hold_levers").unwrap_or(false);
        if !cups && !levers {
            return Ok(None);
        }
        let (slice, print) = self.measured()?;
        let risks = print.measured.risks();
        let meshes = self.meshes();
        let bvhs: Vec<Bvh> = meshes.iter().map(Bvh::build).collect();
        let models = hanging(&meshes, &bvhs);
        let widest_hole_mm = (ids.iter())
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
        let mut changed = false;
        for vent in vents {
            match vent {
                Ok(vent) if ids.contains(&vent.model) => changed |= self.vent(vent)?,
                Ok(_) => {}
                Err(Unvented::OnThePlate) => tracing::warn!(
                    "a suction cup stands on the plate: lift the model onto supports or turn it"
                ),
                Err(Unvented::TooDeep) => {
                    tracing::warn!("a suction cup has no wall thin enough to drill a vent through");
                }
            }
        }
        if let Some(holds) = necks {
            changed |= self.hold_necks(ids, &holds)?;
        }
        Ok((!changed).then_some((slice, print)))
    }

    /// Drills `vent` into its piece by a `Drain` at its mouth, walled on a solid one; says
    /// whether it drilled one.
    fn vent(&mut self, vent: CupVent) -> Result<bool> {
        let piece = &self.pieces[vent.model];
        let object = piece.id;
        let size = HoleSize {
            depth_mm: vent.depth_mm,
            ..piece.shaping.hollow.hole_size()
        };
        let kind = match self.staging.hollowed(object) {
            true => HoleKind::AtDepth,
            false => HoleKind::Walled,
        };
        let edit = Edit::Drain {
            object,
            at_mm: vec![vent.mouth_mm],
            size,
            kind,
            auto: false,
            vent: false,
        };
        match self.staging.apply(&edit, object, self.ctx.watch.stop) {
            Ok(applied) => Ok(matches!(
                applied.report,
                core_document::Report::Drained { holes, .. } if holes > 0
            )),
            Err(core_document::ApplyError::Cancelled) => Err(crate::exit::Cancelled.into()),
            Err(error) => {
                tracing::warn!(
                    "{}: a cup was left undrilled: {error}",
                    piece.input.display()
                );
                Ok(false)
            }
        }
    }

    /// What the supports of `id` stand to: its latest supports step's profile, else `auto`.
    fn own_plan(&self, id: usize) -> Result<SupportPlan> {
        let piece = &self.pieces[id];
        if let Some(plan) = &piece.shaping.supports {
            return Ok(plan.clone());
        }
        plan_of(piece.supported_by.as_deref().unwrap_or(AUTO))
    }

    /// What the supports under a weak neck of `id` stand to: its own, made thick enough.
    fn neck_plan(&self, id: usize) -> Result<SupportPlan> {
        let plan = self.own_plan(id)?;
        Ok(SupportPlan::of(neck_profile(&plan.placing())))
    }

    /// The supports `holds` names on each of `ids`, to that piece's own plan made thick enough;
    /// says whether it stood any.
    fn hold_necks(&mut self, ids: &[usize], holds: &[(usize, Vec3)]) -> Result<bool> {
        let mut stood = false;
        for &id in ids {
            let found: Vec<Vec3> = (holds.iter())
                .filter(|(owner, _)| *owner == id)
                .map(|(_, at)| *at)
                .collect();
            if !found.is_empty() {
                let plan = self.neck_plan(id)?;
                hold_up(
                    (&mut self.pieces[id], &mut self.staging),
                    &plan,
                    &found,
                    self.ctx,
                )?;
                stood = true;
            }
        }
        Ok(stood)
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
            let found: Vec<Vec3> = (owners.iter().flatten())
                .filter(|(owner, _)| *owner == id)
                .map(|(_, at)| *at)
                .collect();
            if found.is_empty() {
                continue;
            }
            let plan = self.own_plan(id)?;
            hold_up(
                (&mut self.pieces[id], &mut self.staging),
                &plan,
                &found,
                self.ctx,
            )?;
        }
        Ok(())
    }

    /// Packs every piece over the plate as `--arrange` does, `spacing_mm` apart, by an
    /// `Arrange` edit; clearing only what is in the way, which no edit carries yet, by moves.
    pub(super) fn arrange(&mut self, settings: &Settings) -> Result<()> {
        let printer = self.ctx.chosen.printer.as_ref();
        let printer = printer.context("an arrange step needs a printer to know the plate")?;
        let first = self.pieces.first().map(|piece| piece.id);
        let Some(first) = first else {
            return Ok(());
        };
        let placed = (self.staging.mesh(first).and_then(|mesh| mesh.aabb()))
            .context("the model has no surface")?;
        let area = usable_area(printer);
        let drift_mm = self.ctx.chosen.material.compensation.fit_drift_mm();
        match edits_of(StepKind::Arrange, settings, first, placed, area, drift_mm) {
            Ok(edits) => {
                for edit in edits {
                    (self.staging.apply(&edit, first, self.ctx.watch.stop)).map_err(said)?;
                }
                Ok(())
            }
            Err(_) => self.clear_the_way(settings, printer),
        }
    }

    /// Moves only what is past the usable plate or meets another, as Check's Move to free
    /// space does.
    fn clear_the_way(
        &mut self,
        settings: &Settings,
        printer: &printer_profiles::PrinterProfile,
    ) -> Result<()> {
        let default = ArrangeSettings::default();
        let asked = ArrangeSettings {
            clearance_mm: number(settings, "spacing_mm").unwrap_or(default.clearance_mm),
            ..default
        };
        let standing: Vec<Mesh> = (self.pieces.iter())
            .map(|piece| self.staging.standing_mesh(piece.id))
            .collect();
        let footprints: Vec<(&Mesh, &Path)> = (standing.iter().zip(&self.pieces))
            .map(|(mesh, piece)| (mesh, piece.input.as_path()))
            .collect();
        let moves = cleared(&footprints, Some(printer), &asked)?;
        for (piece, moved) in self.pieces.iter().zip(moves) {
            if moved == Transform::default() {
                continue;
            }
            let edit = Edit::Move {
                object: piece.id,
                by_mm: moved.translation,
            };
            (self.staging.apply(&edit, piece.id, self.ctx.watch.stop)).map_err(said)?;
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
        let mut staged = self.staged();
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

/// Each of `meshes` with its `bvhs`, standing as it prints, as the risks read them.
fn hanging<'a>(meshes: &'a [Mesh], bvhs: &'a [Bvh]) -> Vec<Hanging<'a>> {
    (meshes.iter().zip(bvhs))
        .map(|(mesh, bvh)| (mesh, bvh, Transform::default()))
        .collect()
}

/// A run's watch that says nothing: a plate cut inside a step is not the run's report.
fn quiet<'a>(ctx: &Ctx<'a>) -> Watch<'a> {
    Watch::quiet(ctx.watch.stop)
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
