//! The plan before a run: what each step of the picked recipe would decide for the models a
//! run would take, measured off the drawing thread and kept until they or the recipe change.

use std::sync::Arc;

use core_geometry::{Mesh, MeshDiagnostics, Transform};
use core_recipe::{Decision, Foreseen, Metrics, Model, Plan, Reading, Value, measure_planned};

use super::{kind_name, note_text, unturned};
use crate::i18n::tr;
use crate::job::Worker;
use crate::plate::BuildPlate;
use crate::scene::{ObjectId, Scene, SceneObject};
use crate::ui::PlanMark;

/// The plan foreseen for the models in hand, and the measuring of it while it runs.
#[derive(Debug, Default)]
pub struct Foresight {
    asked: Option<Key>,
    worker: Option<Worker<Vec<Vec<Foreseen>>>>,
    seen: Option<(Key, Vec<Vec<Foreseen>>)>,
}

/// What a foresight was taken of: the plan, the plate, and each model as it stood.
#[derive(Debug, Clone, PartialEq)]
struct Key {
    plan: Plan,
    plate: BuildPlate,
    models: Vec<ModelKey>,
}

/// A model as a foresight took it, by what changes its metrics: its mesh, placing, cavity,
/// turn and the supports it stands on, whose bounds are only read when it is measured.
#[derive(Debug, Clone, PartialEq)]
struct ModelKey {
    id: ObjectId,
    mesh: usize,
    transform: Transform,
    hollow: bool,
    unturned: bool,
    supports: Vec<usize>,
}

/// One model as a measure on another thread takes it.
struct Taken {
    mesh: Arc<Mesh>,
    transform: Transform,
    diagnostics: MeshDiagnostics,
    turned: bool,
    hollow: bool,
    fits: bool,
}

impl Taken {
    fn of(object: &SceneObject, plate: &BuildPlate) -> Self {
        Self {
            mesh: Arc::clone(&object.mesh),
            transform: object.transform,
            diagnostics: object.summary.diagnostics.clone(),
            turned: !unturned(object),
            hollow: object.hollow.is_hollow(),
            fits: fits(object, plate),
        }
    }

    fn model(&self) -> Model<'_> {
        Model {
            mesh: &self.mesh,
            transform: &self.transform,
            diagnostics: &self.diagnostics,
            turned: self.turned,
            hollow: self.hollow,
            fits: self.fits,
        }
    }
}

/// Whether `object` fits the build volume with what stands under it.
pub(super) fn fits(object: &SceneObject, plate: &BuildPlate) -> bool {
    crate::plate::bounds(object).is_some_and(|bounds| plate.fit(&bounds).fits())
}

impl Foresight {
    /// What `plan` foresees for each of `ids`, a list a model, or `None` while it is being
    /// measured; a change to the plan or to a model measures again.
    pub fn look(
        &mut self,
        plan: &Plan,
        scene: &Scene,
        plate: &BuildPlate,
        ids: &[ObjectId],
    ) -> Option<&[Vec<Foreseen>]> {
        let objects: Vec<&SceneObject> = scene.named(ids).collect();
        let key = Key {
            plan: plan.clone(),
            plate: plate.clone(),
            models: objects.iter().map(|object| key_of(object)).collect(),
        };
        if self.asked.as_ref() != Some(&key) {
            let taken: Vec<Taken> = objects.iter().map(|o| Taken::of(o, plate)).collect();
            let plan = plan.clone();
            self.worker = Some(Worker::spawn(move |_, _| foresee(&plan, &taken), Vec::new));
            self.asked = Some(key.clone());
        }
        if let Some(seen) = self.worker.as_mut().and_then(Worker::poll) {
            self.worker = None;
            self.seen = Some((key.clone(), seen));
        }
        self.seen
            .as_ref()
            .filter(|(seen, _)| *seen == key)
            .map(|(_, seen)| seen.as_slice())
    }
}

fn key_of(object: &SceneObject) -> ModelKey {
    ModelKey {
        id: object.id,
        mesh: Arc::as_ptr(&object.mesh) as usize,
        transform: object.transform,
        hollow: object.hollow.is_hollow(),
        unturned: unturned(object),
        supports: object
            .supports
            .drawn()
            .iter()
            .map(|group| Arc::as_ptr(&group.mesh) as usize)
            .collect(),
    }
}

fn foresee(plan: &Plan, taken: &[Taken]) -> Vec<Vec<Foreseen>> {
    taken
        .iter()
        .map(|taken| {
            let metrics = measure_planned(plan, &taken.model(), None, &Metrics::default());
            core_recipe::preview(plan, &metrics)
        })
        .collect()
}

/// One step as the plan shows it across the models: its mark, and the readings that decided
/// it for the first model it was decided on, as `name value` pairs.
#[derive(Debug, Clone, PartialEq)]
pub struct StepView {
    pub mark: PlanMark,
    /// How many of the models run it, where some do and some do not.
    pub runs_on: Option<(usize, usize)>,
    pub reading: Option<String>,
}

/// Step `index` as `seen` foresees it for every model.
pub fn step_view(seen: &[Vec<Foreseen>], index: usize) -> StepView {
    let each: Vec<&Foreseen> = seen.iter().filter_map(|model| model.get(index)).collect();
    let runs = each
        .iter()
        .filter(|f| matches!(f, Foreseen::Decided(Decision::Run { .. })))
        .count();
    let first = |wanted: fn(&Foreseen) -> bool| each.iter().copied().find(|f| wanted(f));
    let picked = first(|f| matches!(f, Foreseen::Decided(Decision::Stop { .. })))
        .or_else(|| first(|f| matches!(f, Foreseen::Decided(Decision::Pause { .. }))))
        .or_else(|| first(|f| matches!(f, Foreseen::Decided(Decision::Run { .. }))))
        .or_else(|| first(|f| matches!(f, Foreseen::WhenReached)))
        .or_else(|| each.first().copied());
    let (mark, read): (PlanMark, &[Reading]) = match picked {
        Some(Foreseen::Decided(Decision::Run { read, .. })) => (PlanMark::Runs, read),
        Some(Foreseen::Decided(Decision::Skip { read })) => (PlanMark::Skipped, read),
        Some(Foreseen::Decided(Decision::Stop { read, .. })) => (PlanMark::Stops, read),
        Some(Foreseen::Decided(Decision::Pause { .. })) => (PlanMark::Pauses, &[]),
        Some(Foreseen::Decided(Decision::Disabled)) => (PlanMark::Disabled, &[]),
        Some(Foreseen::WhenReached) | None => (PlanMark::WhenReached, &[]),
    };
    let partly = mark == PlanMark::Runs && runs < each.len();
    StepView {
        mark,
        runs_on: partly.then_some((runs, each.len())),
        reading: (!read.is_empty() && mark != PlanMark::Runs).then(|| readings(read)),
    }
}

/// The readings as the recipe names them: `saves_ml 4.2, big_ml 30`.
pub fn readings(read: &[Reading]) -> String {
    let pairs: Vec<String> = read
        .iter()
        .map(|reading| format!("{} {}", reading.name, value(reading.value)))
        .collect();
    pairs.join(", ")
}

fn value(value: Value) -> String {
    match value {
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => {
            let text = format!("{number:.2}");
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    }
}

/// One line of the plan list: the step's name, its mark and verdict, the readings that
/// decided it, and the note where it undoes an earlier step.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanLine {
    pub name: String,
    pub mark: PlanMark,
    pub verdict: String,
    pub reading: Option<String>,
    pub note: Option<String>,
}

/// The plan list before a run, as `seen` foresees each step.
pub fn foreseen_lines(plan: &Plan, seen: &[Vec<Foreseen>]) -> Vec<PlanLine> {
    lines(plan, |index| step_view(seen, index))
}

/// The plan list while a run goes, each step where `marks` says it stands.
pub fn running_lines(plan: &Plan, marks: &[PlanMark]) -> Vec<PlanLine> {
    lines(plan, |index| StepView {
        mark: marks.get(index).copied().unwrap_or(PlanMark::ToCome),
        runs_on: None,
        reading: None,
    })
}

fn lines(plan: &Plan, view: impl Fn(usize) -> StepView) -> Vec<PlanLine> {
    let steps = &plan.recipe().steps;
    (0..steps.len())
        .map(|index| {
            let view = view(index);
            let notes: Vec<String> = plan
                .notes()
                .iter()
                .filter(|note| note.step == index)
                .map(note_text)
                .collect();
            PlanLine {
                name: tr!(
                    "recipe-step-numbered",
                    number = index + 1,
                    step = kind_name(steps[index].kind)
                ),
                mark: view.mark,
                verdict: verdict(view.mark, view.runs_on),
                reading: view.reading,
                note: (!notes.is_empty()).then(|| notes.join("\n")),
            }
        })
        .collect()
}

fn verdict(mark: PlanMark, runs_on: Option<(usize, usize)>) -> String {
    match (mark, runs_on) {
        (PlanMark::Runs, Some((count, total))) => {
            tr!("recipe-plan-runs-on", count = count, total = total)
        }
        (PlanMark::Runs, None) => tr!("recipe-plan-runs"),
        (PlanMark::Skipped, _) => tr!("recipe-plan-skipped"),
        (PlanMark::WhenReached, _) => tr!("recipe-plan-when-reached"),
        (PlanMark::Disabled, _) => tr!("recipe-plan-off"),
        (PlanMark::Stops, _) => tr!("recipe-plan-stops"),
        (PlanMark::Pauses, _) => tr!("recipe-plan-pauses"),
        (PlanMark::Done, _) => tr!("recipe-plan-done"),
        (PlanMark::Running, _) => tr!("recipe-plan-running"),
        (PlanMark::ToCome, _) => tr!("recipe-plan-to-come"),
    }
}
