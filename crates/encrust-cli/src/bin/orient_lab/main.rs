//! A bench for orientation: each goal's choice, the poses a person would try by hand and,
//! asked for, an even spread over the sphere, all judged by the supports and the stack
//! they actually produce, against rules of success every goal shares. See
//! `docs/design/orientation.md`; the rules are in `verdict.rs`.

mod bodies;
mod corpus;
mod poses;
mod report;
mod verdict;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use core_engine::Trial;
use core_geometry::{Mesh, orient_outward, weld};
use core_plate::OrientGoal;
use encrust_cli::pipeline;
use printer_profiles::{Catalogue, MaterialProfile, PrinterProfile, SupportProfile};

use crate::corpus::{Entry, Expectations};
use crate::poses::{Bench, Source};
use crate::report::{GoalRow, ModelReport, PoseRow, Report};
use crate::verdict::{Verdict, judge};

/// Benches orientation on the built-in bodies, on models named outright, or on a corpus.
#[derive(Parser)]
#[command(name = "orient-lab")]
struct Args {
    /// Models to bench. With none and no `--corpus`, the built-in bodies.
    models: Vec<PathBuf>,
    /// A corpus file listing models and what is expected of each.
    #[arg(long)]
    corpus: Option<PathBuf>,
    /// Bench only the models whose name holds this.
    #[arg(long)]
    only: Option<String>,
    #[arg(long, default_value = "elegoo-saturn-4-ultra")]
    printer: String,
    #[arg(long, default_value = "medium")]
    support: String,
    /// Also measure this many directions spread over the sphere, so the best known pose
    /// is not only one a person or the search thought of. Slow: a full print each.
    #[arg(long, default_value_t = 0)]
    oracle: usize,
    /// Write the run as JSON, to keep as a baseline.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Compare with a run kept before; a goal that passed there and fails here fails the
    /// run.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Judge the poses a run kept again, measuring nothing: what changing a rule of
    /// success does, in seconds. Expectations come from `--corpus`, if given.
    #[arg(long)]
    rejudge: Option<PathBuf>,
}

/// What every pose is printed with.
struct Setup {
    printer: PrinterProfile,
    material: MaterialProfile,
    support: SupportProfile,
}

impl Setup {
    fn bench(&self) -> Bench<'_> {
        Bench {
            trial: Trial {
                printer: &self.printer,
                material: &self.material,
                support: &self.support,
                raster_window: rayon::current_num_threads(),
            },
            support: &self.support,
            volume: &self.printer.build_volume,
        }
    }
}

/// One model to bench: where its mesh comes from and what is expected of it.
struct Subject {
    name: String,
    mesh: MeshSource,
    expect: Expectations,
}

enum MeshSource {
    Built(Mesh),
    File(PathBuf),
    Skipped(String),
}

fn main() -> Result<ExitCode> {
    let args = Args::parse();
    let catalogue = Catalogue::bundled().context("the bundled profiles do not load")?;
    let setup = Setup {
        printer: catalogue.printer(&args.printer)?.profile.clone(),
        material: MaterialProfile::default(),
        support: catalogue.support(&args.support)?.profile.clone(),
    };
    println!(
        "printer {}, supports {}, layer {} mm, oracle {}\n",
        args.printer, args.support, setup.material.layer_height_mm, args.oracle
    );

    let report = match &args.rejudge {
        Some(path) => rejudged(&Report::read(path)?, &args)?,
        None => {
            let mut report = Report::new(&args.printer, &args.support, args.oracle);
            for subject in subjects(&args)? {
                report.models.push(bench(subject, &setup, args.oracle));
            }
            report
        }
    };
    let (passed, judged) = report.score();
    println!("{passed} of {judged} goals pass");

    if let Some(path) = &args.json {
        report.write(path)?;
    }
    if let Some(path) = &args.baseline {
        let changes = report::compare(&Report::read(path)?, &report);
        report::print_changes(&changes);
        if changes.iter().any(report::Change::regressed) {
            return Ok(ExitCode::FAILURE);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// The models the arguments name, filtered by `--only`.
fn subjects(args: &Args) -> Result<Vec<Subject>> {
    let mut subjects: Vec<Subject> = if let Some(path) = &args.corpus {
        corpus::read(path)?.into_iter().map(from_entry).collect()
    } else if !args.models.is_empty() {
        args.models
            .iter()
            .map(|path| Subject {
                name: name_of(path),
                mesh: MeshSource::File(path.clone()),
                expect: Expectations::default(),
            })
            .collect()
    } else {
        bodies::all()
            .into_iter()
            .map(|(name, mesh)| Subject {
                name: name.to_owned(),
                mesh: MeshSource::Built(mesh),
                expect: Expectations::default(),
            })
            .collect()
    };
    if let Some(only) = &args.only {
        subjects.retain(|subject| subject.name.contains(only.as_str()));
    }
    Ok(subjects)
}

fn from_entry(entry: Entry) -> Subject {
    Subject {
        name: name_of(&entry.path),
        mesh: entry
            .skip
            .map_or(MeshSource::File(entry.path), MeshSource::Skipped),
        expect: entry.expect,
    }
}

fn name_of(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Benches one model. One that cannot be loaded or measured is reported skipped, so a run
/// over a corpus still finishes.
fn bench(subject: Subject, setup: &Setup, oracle: usize) -> ModelReport {
    let skipped = |name: String, reason: String| {
        println!("## {name} — skipped: {reason}\n");
        ModelReport {
            name,
            faces: 0,
            skipped: Some(reason),
            poses: Vec::new(),
            goals: Vec::new(),
        }
    };
    let mesh = match subject.mesh {
        MeshSource::Built(mesh) => mesh,
        MeshSource::Skipped(reason) => return skipped(subject.name, reason),
        MeshSource::File(path) => match load(&path) {
            Ok(mesh) => mesh,
            Err(error) => return skipped(subject.name, format!("{error:#}")),
        },
    };
    match setup.bench().poses(&mesh, oracle) {
        Ok(poses) => judged(subject.name, mesh.faces.len(), &poses, &subject.expect),
        Err(error) => skipped(subject.name, format!("{error:#}")),
    }
}

/// Judges every goal of one measured model and prints it.
fn judged(name: String, faces: usize, poses: &[poses::Pose], expect: &Expectations) -> ModelReport {
    let verdicts: Vec<Verdict> = OrientGoal::ALL
        .into_iter()
        .filter_map(|goal| judge(goal, poses, expect.for_goal(goal), faces))
        .collect();
    let seconds_of = |goal| {
        poses
            .iter()
            .find(|pose| pose.source == Source::Goal(goal))
            .map_or(0.0, |pose| pose.seconds)
    };
    let model = ModelReport {
        name,
        faces,
        skipped: None,
        poses: poses.iter().map(PoseRow::of).collect(),
        goals: verdicts
            .iter()
            .map(|verdict| GoalRow::of(verdict, seconds_of(verdict.goal)))
            .collect(),
    };
    report::print_model(&model, &verdicts);
    model
}

/// `kept` judged again by the rules as they are now, with the corpus's expectations.
fn rejudged(kept: &Report, args: &Args) -> Result<Report> {
    let expectations: Vec<Subject> = match &args.corpus {
        Some(path) => corpus::read(path)?.into_iter().map(from_entry).collect(),
        None => Vec::new(),
    };
    let mut report = Report::new(&kept.printer, &kept.support, kept.oracle);
    for model in &kept.models {
        if args
            .only
            .as_ref()
            .is_some_and(|only| !model.name.contains(only.as_str()))
        {
            continue;
        }
        if model.skipped.is_some() {
            report.models.push(model.clone());
            continue;
        }
        let expect = expectations
            .iter()
            .find(|subject| subject.name == model.name)
            .map(|subject| subject.expect.clone())
            .unwrap_or_default();
        let poses = model
            .poses
            .iter()
            .map(PoseRow::pose)
            .collect::<Result<Vec<_>>>()?;
        report
            .models
            .push(judged(model.name.clone(), model.faces, &poses, &expect));
    }
    Ok(report)
}

fn load(path: &Path) -> Result<Mesh> {
    let loaded = pipeline::load(path)?;
    let mut mesh = weld(&loaded.mesh, core_geometry::DEFAULT_WELD_TOLERANCE).mesh;
    orient_outward(&mut mesh);
    Ok(mesh)
}
