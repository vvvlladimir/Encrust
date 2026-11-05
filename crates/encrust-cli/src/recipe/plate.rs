//! The steps a recipe runs once on the whole plate: `check` measures it as the report does,
//! `arrange` packs it as `--arrange` does, `save` writes it there and then.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::{DEFAULT_TEMPLATE, island_floors, island_underside};
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3};
use core_plate::ArrangeSettings;
use core_recipe::{Metric, retry};

use super::piece::Piece;
use super::settings::{Settings, flag, number, word};
use super::steps::{checked, stand};
use super::{Ctx, Did, Run, Saved};
use crate::batch::sliced_name;
use crate::commands::printer::{machine_of, send_to};
use crate::estimate::{Print, measure_plate};
use crate::pipeline::{Watch, raster_window, slice_staged};
use crate::profiles::{AUTO, plan_of};
use crate::slice_report::SliceReport;
use crate::stage::{Staged, arranged, staged};

impl Run<'_> {
    /// The plate as the pieces stand now, staged to be cut.
    fn staged(&self) -> Result<Staged> {
        let models = self
            .pieces
            .iter()
            .map(Piece::model)
            .collect::<Result<Vec<_>>>()?;
        let drainage = self
            .pieces
            .iter()
            .any(|piece| piece.cavity.is_some() || !piece.holes.is_empty());
        let ctx = self.ctx;
        let machine = (ctx.job, ctx.chosen, ctx.layer_height_mm);
        Ok(staged((Vec::new(), models), drainage, machine, false))
    }

    /// Measures the plate and notes what it found on each of `ids`, then supports their
    /// islands and measures again while step `index`'s retry holds for any; answers how many
    /// times it went again.
    pub(super) fn check(&mut self, index: usize, ids: &[usize]) -> Result<u8> {
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
        let bvhs: Vec<Bvh> = self
            .pieces
            .iter()
            .map(|piece| Bvh::build(&piece.mesh))
            .collect();
        floors
            .iter()
            .map(|floor| {
                bvhs.iter()
                    .enumerate()
                    .filter_map(|(id, bvh)| Some((id, island_underside(bvh, *floor, layer_mm)?)))
                    .min_by(|(_, (one, _)), (_, (other, _))| one.total_cmp(other))
                    .map(|(id, (_, at))| (id, at))
            })
            .collect()
    }

    fn retries(&self, index: usize, ids: &[usize], attempt: u8) -> Result<bool> {
        for &id in ids {
            let metrics = self.metrics_of(&self.pieces[id])?;
            if retry(&self.asked.plan, index, &metrics, attempt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A support under every island `owners` names on one of `ids`, as Check's Support every
    /// island stands one; each such piece stands on its supports again with them.
    fn hold_islands(&mut self, ids: &[usize], owners: &[Option<(usize, Vec3)>]) -> Result<()> {
        for &id in ids {
            let piece = &mut self.pieces[id];
            let down = Vec3::Z * piece.lifted_mm();
            let found: Vec<Vec3> = owners
                .iter()
                .flatten()
                .filter(|(owner, _)| *owner == id)
                .map(|(_, at)| *at - down)
                .collect();
            if found.is_empty() {
                continue;
            }
            piece.held.extend(found);
            let plan = match piece.plan.clone().or(piece.shaping.supports.clone()) {
                Some(plan) => plan,
                None => plan_of(AUTO)?,
            };
            piece.take_off_supports();
            stand(piece, plan, self.ctx)?;
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
        let offsets = arranged(&footprints, printer, &asked)?;
        for (piece, offset) in self.pieces.iter_mut().zip(offsets) {
            piece.moved(Transform::from_translation(offset));
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
        let folder = folder_of(word(settings, "folder"))?;
        let template = word(settings, "name").unwrap_or(DEFAULT_TEMPLATE);
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
    let folder = match (asked, home) {
        (Some("~"), Some(home)) => home,
        (Some(folder), Some(home)) if folder.starts_with("~/") => home.join(&folder[2..]),
        (Some(folder), _) => PathBuf::from(folder),
        (None, Some(home)) => home,
        (None, None) => bail!("a save step with no folder saves into the home, and there is none"),
    };
    Ok(folder)
}
