//! `--recipe`: each model taken through a recipe's steps, each decided by `core_recipe` on the
//! model as the steps before left it and run by its flag's own call (ADR 0403).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_engine::{Model, SupportPlan};
use core_geometry::{Mesh, Scalar};
use core_recipe::{
    BUILT_IN, Decision, Metrics, OnFail, Plan, Recipe, Resolved, Setting, StepKind, built_in,
    decide, decide_forced, validate,
};
use core_volume::HollowSettings;

mod drain;
mod piece;
mod plate;
mod prepared;
mod report;
mod settings;
mod steps;

use piece::Piece;
pub use prepared::Prepared;
pub use report::RecipeRun;
use report::StepRun;
use settings::{Settings, forced, typed_over};

use crate::args::JobArgs;
use crate::pipeline::Watch;
use crate::profiles::{AUTO, Chosen, plan_of};
use crate::raster_report::RasterReport;
use crate::slice_report::SliceReport;
use crate::stage::{Found, Part, Shaping};

/// The built-in recipe `--prepare` runs.
pub const PREPARE: &str = "prepare";

/// The recipe a run was asked for, read and checked.
pub struct Asked {
    pub plan: Plan,
    /// Whether it is the built-in `prepare`, whose report keeps `--prepare`'s `prepared`.
    pub prepare: bool,
}

impl Asked {
    /// The recipe `--recipe` or `--prepare` names, or `None` for a run without one.
    pub fn of(job: &JobArgs) -> Result<Option<Self>> {
        let Some(asked) = job.recipe_asked() else {
            return Ok(None);
        };
        Ok(Some(Self {
            plan: plan_of_recipe(asked)?,
            prepare: asked == PREPARE,
        }))
    }

    /// Whether one of its steps writes the plate.
    pub fn saves(&self) -> bool {
        self.plan
            .recipe()
            .steps
            .iter()
            .any(|step| step.enabled && step.kind == StepKind::Save)
    }
}

/// The built-in recipe `asked` names, or the recipe file at that path.
fn plan_of_recipe(asked: &str) -> Result<Plan> {
    let path = Path::new(asked);
    let is_file = path.is_file() || path.extension().is_some_and(|ext| ext == "toml");
    if !is_file {
        let ids: Vec<&str> = BUILT_IN.iter().map(|(id, _)| *id).collect();
        return built_in(asked).with_context(|| {
            format!(
                "--recipe takes a recipe file or a built-in id: {}",
                ids.join(", ")
            )
        });
    }
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the recipe {}", path.display()))?;
    Recipe::from_toml(&text)
        .and_then(validate)
        .with_context(|| format!("{} is not a recipe this build runs", path.display()))
}

/// What a recipe runs with: the flags, the machine, the layer height and how the run talks.
pub struct Ctx<'a> {
    pub job: &'a JobArgs,
    pub chosen: &'a Chosen,
    pub layer_height_mm: Scalar,
    pub watch: &'a Watch<'a>,
}

impl Ctx<'_> {
    /// The supports `piece` is judged on and its holes placed for before a supports step
    /// stands it: those it was given or stands on, else the catalogue's automatic where the
    /// recipe stands it on any.
    fn support_plan(&self, piece: &Piece) -> Result<Option<SupportPlan>> {
        match piece.shaping.supports.clone().or(piece.plan.clone()) {
            Some(plan) => Ok(Some(plan)),
            None if piece.stands => plan_of(AUTO).map(Some),
            None => Ok(None),
        }
    }
}

/// The plate as a save step wrote it.
pub struct Saved {
    pub output: PathBuf,
    pub slice: SliceReport,
    pub raster: Option<RasterReport>,
}

/// What a step did besides what it decided: why it did nothing, the holes it drilled, the
/// retries it took, where it wrote.
#[derive(Debug, Default, Clone)]
struct Did {
    note: Option<String>,
    holes: Option<usize>,
    retries: Option<u8>,
    saved: Option<String>,
}

impl Did {
    fn nothing(why: &str) -> Self {
        Self {
            note: Some(why.to_owned()),
            ..Self::default()
        }
    }
}

/// A recipe going over the models of a plate.
struct Run<'a> {
    asked: &'a Asked,
    ctx: &'a Ctx<'a>,
    pieces: Vec<Piece>,
    saved: Option<Saved>,
}

/// Takes every imported model through `asked`'s steps and answers the parts and models of
/// the plate it leaves, and the plate as its last save step wrote it.
pub fn lay(
    asked: &Asked,
    (pieces, found): (Vec<Vec<Mesh>>, Vec<Found>),
    entries: &[(PathBuf, Shaping)],
    ctx: &Ctx,
) -> Result<(Vec<Part>, Vec<Model>, Option<Saved>)> {
    for (_, shaping) in entries {
        refuse_pointing(shaping)?;
        refuse_unstepped(&asked.plan, shaping)?;
    }
    check_sends(&asked.plan, ctx)?;
    let mut steps = asked.plan.recipe().steps.iter();
    let stands = steps.any(|step| step.enabled && step.kind == StepKind::Supports);
    let pieces = pieces
        .into_iter()
        .zip(found)
        .zip(entries)
        .flat_map(|((meshes, found), (path, shaping))| {
            let mesh = meshes.into_iter().next();
            let shaped = (path.clone(), shaping.clone(), stands);
            mesh.map(|mesh| Piece::new(shaped, mesh, found))
        })
        .collect();
    if ctx.watch.talk {
        println!("Recipe {}", asked.plan.recipe().name);
    }
    let mut run = Run {
        asked,
        ctx,
        pieces,
        saved: None,
    };
    for index in 0..asked.plan.recipe().steps.len() {
        run.step(index)?;
    }
    run.finish()
}

/// Refuses what points at the model as imported, which a recipe's steps may move first.
fn refuse_pointing(shaping: &Shaping) -> Result<()> {
    let hollow = &shaping.hollow;
    if hollow.pointing() || !hollow.drain_support_at.is_empty() {
        bail!(
            "--drain-at, --channel and --drain-support-at point at the model as imported; a \
             recipe's drain step takes `points`, fractions of the box the model stands in"
        );
    }
    Ok(())
}

/// Refuses a flag that forces a step of a kind `plan` has none of to run.
fn refuse_unstepped(plan: &Plan, shaping: &Shaping) -> Result<()> {
    let recipe = plan.recipe();
    for (kind, flag) in forced(shaping) {
        let mut steps = recipe.steps.iter();
        if !steps.any(|step| step.enabled && step.kind == kind) {
            bail!(
                "{flag} runs the recipe's {} steps, and {} has no {} step",
                kind.name(),
                recipe.name,
                kind.name()
            );
        }
    }
    Ok(())
}

/// Refuses a recipe that sends the plate on before anything is cut, where there is no printer
/// to send it to.
fn check_sends(plan: &Plan, ctx: &Ctx) -> Result<()> {
    if !ctx.watch.writes {
        return Ok(());
    }
    let sends = |set: &core_recipe::Settings| {
        matches!(
            set.get("send"),
            Some(Setting::Bool(true) | Setting::Expr(_))
        )
    };
    for (index, step) in plan.recipe().steps.iter().enumerate() {
        let save = step.enabled && step.kind == StepKind::Save;
        if save && (sends(&step.set) || step.choose.iter().any(|choice| sends(&choice.set))) {
            crate::commands::printer::machine_of(ctx.chosen.printer.as_ref())
                .with_context(|| format!("step {index} (save) sends the plate to the printer"))?;
        }
    }
    Ok(())
}

impl Run<'_> {
    /// Decides step `index` for every model, then runs it on those it runs for.
    fn step(&mut self, index: usize) -> Result<()> {
        let recipe = self.asked.plan.recipe();
        let step = &recipe.steps[index];
        let kind = step.kind;
        let runs = self
            .decided(index)
            .with_context(|| format!("step {index} ({}) of {}", kind.name(), recipe.name))?;
        if kind == StepKind::Pause {
            let said = self.pieces.first().and_then(|piece| piece.steps.last());
            if let Some(said) = said.filter(|_| self.ctx.watch.talk) {
                println!("Pause: {}\n", said.note.as_deref().unwrap_or("paused"));
            }
            return Ok(());
        }
        let done = match kind {
            StepKind::Check | StepKind::Arrange | StepKind::Save => self.on_plate(index, &runs),
            _ => self.on_models(index, runs),
        };
        match done {
            Err(error) if step.on_fail == OnFail::Skip => {
                tracing::warn!(
                    "step {index} ({}) failed and is skipped: {error:#}",
                    kind.name()
                );
                Ok(())
            }
            done => {
                done.with_context(|| format!("step {index} ({}) of {}", kind.name(), recipe.name))
            }
        }
    }

    /// Each model's decision for step `index`, noted on it; the models it runs for with their
    /// settings, or the gate's message once one stops.
    fn decided(&mut self, index: usize) -> Result<Vec<(usize, Settings)>> {
        let kind = self.asked.plan.recipe().steps[index].kind;
        let mut runs = Vec::new();
        let mut stopped = None;
        for id in 0..self.pieces.len() {
            let metrics = self.metrics_of(&self.pieces[id])?;
            let typed = forced(&self.pieces[id].shaping).any(|(typed, _)| typed == kind);
            let decide = if typed { decide_forced } else { decide };
            let decision = decide(&self.asked.plan, index, &metrics)?;
            let settings = match &decision {
                Decision::Run { settings, .. } => {
                    typed_over(kind, settings.clone(), &self.pieces[id].shaping)
                }
                _ => Settings::new(),
            };
            let noted = StepRun::of(index, kind, &decision, &settings);
            if self.ctx.watch.talk {
                match self.pieces.len() {
                    1 => println!("{noted}"),
                    _ => println!("{noted}  {}", self.pieces[id].input.display()),
                }
            }
            self.pieces[id].steps.push(noted);
            match decision {
                Decision::Run { .. } => runs.push((id, settings)),
                Decision::Stop { message, .. } => stopped = stopped.or(Some(message)),
                _ => {}
            }
        }
        if let Some(message) = stopped {
            match message.is_empty() {
                true => bail!("the recipe's gate stopped the run"),
                false => bail!("the recipe's gate stopped the run: {message}"),
            }
        }
        Ok(runs)
    }

    /// Runs a step that works on one model on each it runs for, a model it fails on skipped
    /// where the step says so.
    fn on_models(&mut self, index: usize, runs: Vec<(usize, Settings)>) -> Result<()> {
        let step = &self.asked.plan.recipe().steps[index];
        for (id, settings) in runs {
            let piece = &mut self.pieces[id];
            let did = match steps::run(step.kind, piece, &settings, self.ctx) {
                Err(error) if step.on_fail == OnFail::Skip => {
                    let why = format!("failed and skipped: {error:#}");
                    tracing::warn!("{}: step {index} {why}", piece.input.display());
                    Did::nothing(&why)
                }
                did => did?,
            };
            noted(piece, did);
        }
        Ok(())
    }

    /// Runs a step that works on the plate as a whole once, with the settings of the first
    /// model it runs for.
    fn on_plate(&mut self, index: usize, runs: &[(usize, Settings)]) -> Result<()> {
        let Some((_, settings)) = runs.first() else {
            return Ok(());
        };
        let ids: Vec<usize> = runs.iter().map(|(id, _)| *id).collect();
        let did = match self.asked.plan.recipe().steps[index].kind {
            StepKind::Check => Did {
                retries: Some(self.check(index, &ids)?),
                ..Did::default()
            },
            StepKind::Arrange => {
                self.arrange(settings)?;
                Did::default()
            }
            _ => self.save(settings)?,
        };
        for id in ids {
            noted(&mut self.pieces[id], did.clone());
        }
        Ok(())
    }

    /// Every variable of `piece` as it stands; `saves_ml` at the wall its hollow step would
    /// hollow it at, the wall a flag gave, or Hollow's default.
    fn metrics_of(&self, piece: &Piece) -> Result<Metrics> {
        let printer = self.ctx.chosen.printer.as_ref();
        let (wall_mm, worked_out) = match piece.shaping.hollow.thickness_mm {
            Some(wall_mm) => (wall_mm, None),
            None => wall_of(&self.asked.plan),
        };
        let metrics = piece.metrics(wall_mm, printer);
        let Some(index) = worked_out else {
            return Ok(metrics);
        };
        Ok(match decide(&self.asked.plan, index, &metrics)? {
            Decision::Run { settings, .. } => match settings.get("wall_mm") {
                Some(Resolved::Number(wall_mm)) => piece.metrics(*wall_mm, printer),
                _ => metrics,
            },
            _ => metrics,
        })
    }

    /// The parts and models the pieces make as the run left them, with what `--prepare`
    /// says of each where it was asked.
    fn finish(self) -> Result<(Vec<Part>, Vec<Model>, Option<Saved>)> {
        let ctx = self.ctx;
        let name = self.asked.plan.recipe().name.clone();
        let printer = ctx.chosen.printer.as_ref();
        let mut parts = Vec::with_capacity(self.pieces.len());
        let mut models = Vec::with_capacity(self.pieces.len());
        for piece in self.pieces {
            if ctx.watch.talk && self.asked.prepare {
                print!("{}", piece.prepared);
            }
            let (part, model) = piece.laid((&name, self.asked.prepare), printer, ctx.watch.talk)?;
            parts.push(part);
            models.push(model);
        }
        Ok((parts, models, self.saved))
    }
}

/// Notes what a step did on the record of it `piece` holds last.
fn noted(piece: &mut Piece, did: Did) {
    if let Some(step) = piece.steps.last_mut() {
        step.note = did.note.or(step.note.take());
        step.holes = did.holes;
        step.retries = did.retries;
        step.saved = did.saved;
    }
}

/// The wall `saves_ml` is reckoned at before a model is reached: the first hollow step's when
/// it names one outright, else Hollow's default; and that step, when it is worked out.
fn wall_of(plan: &Plan) -> (Scalar, Option<usize>) {
    let default = HollowSettings::default().thickness_mm;
    let mut steps = plan.recipe().steps.iter().enumerate();
    let first = steps.find(|(_, step)| step.enabled && step.kind == StepKind::Hollow);
    match first.map(|(index, step)| (index, step.set.get("wall_mm"))) {
        Some((_, Some(Setting::Number(wall_mm)))) => (*wall_mm, None),
        Some((index, Some(Setting::Expr(_)))) => (default, Some(index)),
        _ => (default, None),
    }
}
