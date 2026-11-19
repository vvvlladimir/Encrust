//! `--edits` carried out on a project through `core-document`'s apply, and `--project-out`
//! writing the plate they leave (ADR 0466).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_document::{Document, ImportedModel, apply};
use core_edit::Edit;
use core_engine::project::{Manifest, Project, Summary, save};
use core_geometry::{Mesh, Orientation, diagnose};
use core_volume::Cancel;

use crate::args::JobArgs;
use crate::pipeline::{Watch, import};
use crate::profiles::Chosen;
use crate::project;
use crate::report::ImportReport;
use crate::stage::Staged;

/// The edits a run makes to a project's plate, and where the plate they leave is written.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct EditArgs {
    /// Edit the project's plate by the JSON list of edits in FILE, in order, before the run:
    /// what the window's tools and a chat client do (`docs/design/document.md`).
    #[arg(long, value_name = "FILE")]
    pub edits: Option<PathBuf>,

    /// Write the project's plate as the edits leave it to FILE, an `.encrust` project.
    #[arg(long, value_name = "FILE")]
    pub project_out: Option<PathBuf>,
}

impl EditArgs {
    /// Whether either flag was given.
    pub fn wanted(&self) -> bool {
        self.edits.is_some() || self.project_out.is_some()
    }

    /// `project` with `--edits` carried out on it, written to `--project-out` where asked.
    pub fn applied(&self, project: Project) -> Result<Project> {
        let edits = match &self.edits {
            Some(path) => read(path)?,
            None => Vec::new(),
        };
        let mut document = Document::from_project(project);
        for (index, edit) in edits.iter().enumerate() {
            apply(&mut document, edit, Cancel::never())
                .with_context(|| format!("edit {} of --edits", index + 1))?;
        }
        let project = document.into_project();
        if let Some(path) = &self.project_out {
            save(path, &project)
                .with_context(|| format!("cannot write the project {}", path.display()))?;
        }
        Ok(project)
    }

    /// Refuses the flags for a plate file, which its own keys shape.
    pub fn refuse_for_models(&self) -> Result<()> {
        if self.wanted() {
            // TODO(step-C1): models staged into a Document, so --edits and --project-out take them.
            bail!(
                "--edits and --project-out take a project (.encrust); a model is shaped by its flags"
            );
        }
        Ok(())
    }
}

/// The models as imported and placed written as a project by `--project-out`, `--edits` carried
/// out on it first, and staged as that project.
pub fn stage_models(
    paths: &[PathBuf],
    arrange: bool,
    job: &JobArgs,
    chosen: &Chosen,
    watch: &Watch,
) -> Result<Staged> {
    let Some(out) = &job.edits.project_out else {
        bail!("--edits on models needs --project-out, which writes them as a project first");
    };
    let shaped = !project::shaping_flags(job)
        .iter()
        .all(|flag| *flag == "--center");
    if arrange || shaped || job.model.wanted() {
        bail!(
            "--project-out writes models as imported; shape them by --edits on the project it writes"
        );
    }
    chosen.measured()?;
    let mut models = Vec::with_capacity(paths.len());
    for path in paths {
        let (mesh, import, _) = import(path, &job.import, None, chosen, watch)?;
        models.push(imported_model(path, mesh, &import));
        watch.stop.check()?;
    }
    let selection = &job.profile;
    let printer = selection
        .profile
        .is_none()
        .then(|| selection.printer.clone());
    let resin = selection
        .material
        .is_none()
        .then(|| selection.resin.clone());
    let manifest = Manifest::new(printer.flatten(), resin.flatten());
    let project = Document::of_models(manifest, models).into_project();
    let project = job.edits.applied(project)?;
    project::stage_checked(out, &project, None, job, watch)
}

fn imported_model(path: &Path, mesh: Mesh, import: &ImportReport) -> ImportedModel {
    let summary = Summary {
        vertices_merged: import.welded.vertices_merged,
        faces_removed: import.welded.faces_removed(),
        orientation: import.orientation.unwrap_or(Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        }),
        diagnostics: import
            .diagnostics
            .clone()
            .unwrap_or_else(|| diagnose(&mesh)),
    };
    let name = path.file_stem().map_or_else(
        || path.display().to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    ImportedModel {
        name,
        mesh,
        summary,
    }
}

fn read(path: &Path) -> Result<Vec<Edit>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the edits {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("{} is not a JSON list of edits", path.display()))
}
