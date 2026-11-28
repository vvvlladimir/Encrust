//! `--edits` carried out through `core-document`'s apply, on a project or on models after their
//! flags, and `--project-out` writing the plate they leave (ADR 0456, 0458).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use core_document::{Document, ImportedModel, apply};
use core_edit::Edit;
use core_engine::project::{Project, Summary, save};
use core_geometry::{Mesh, Orientation, diagnose};
use core_volume::Cancel;

use crate::report::ImportReport;
use crate::stage::Seed;
use crate::steps::{Edited, edit_bytes};

/// The edits a run makes to a project's plate, and where the plate they leave is written.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct EditArgs {
    /// Edit the plate, a project's or the models' after their flags, by the JSON list of edits
    /// in FILE, in order: what the window's tools and a chat client do (`docs/design/document.md`).
    #[arg(long, value_name = "FILE")]
    pub edits: Option<PathBuf>,

    /// Write the plate as the run's edits leave it to FILE, an `.encrust` project.
    #[arg(long, value_name = "FILE")]
    pub project_out: Option<PathBuf>,
}

impl EditArgs {
    /// Whether either flag was given.
    pub fn wanted(&self) -> bool {
        self.edits.is_some() || self.project_out.is_some()
    }

    /// The edits `--edits` names, none without it.
    pub fn listed(&self) -> Result<Vec<Edit>> {
        match &self.edits {
            Some(path) => Ok(read(path)?.into_iter().map(proved_as_shipped).collect()),
            None => Ok(Vec::new()),
        }
    }

    /// `project` with `--edits` carried out on it, written to `--project-out` where asked.
    pub(crate) fn applied(&self, (project, mut seed): (Project, Seed)) -> Result<(Project, Seed)> {
        let mut document = Document::from_project(project);
        document.keep_poses_in(crate::kept_poses::folder());
        each_edit(&self.listed()?, |edit| {
            let next = (seed.as_ref())
                .map(|(steps, key)| (Arc::clone(steps), key.then(&edit_bytes(edit))));
            if let Some((steps, key)) = &next
                && let Some(edited) = steps.plate(*key)
            {
                document = edited.document;
            } else {
                let stage = crate::spent::edit_stage(edit);
                let applied =
                    crate::spent::timed(&stage, || apply(&mut document, edit, Cancel::never()))?;
                if let Some((steps, key)) = &next {
                    let edited = Edited {
                        document: document.clone(),
                        owners: Vec::new(),
                        applied,
                    };
                    steps.keep_plate(*key, edited);
                }
            }
            seed = next;
            Ok(())
        })?;
        let written = self.project_out.as_ref().zip(seed.as_ref());
        let kept = written.map(|_| document.clone());
        let project = document.into_project();
        if let Some(path) = &self.project_out {
            write_project(path, &project)?;
        }
        if let (Some((path, (steps, key))), Some(document)) = (written, kept) {
            steps.wrote(path, *key, document);
        }
        Ok((project, seed))
    }

    /// Refuses the flags for a plate file, which its own keys shape.
    pub fn refuse_for_plate_file(&self) -> Result<()> {
        if self.wanted() {
            bail!(
                "--edits and --project-out take a project or models; a plate file is shaped by its keys"
            );
        }
        Ok(())
    }
}

pub(crate) fn imported_model(path: &Path, mesh: Mesh, import: &ImportReport) -> ImportedModel {
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

/// Carries out each of `edits` in order by `apply_one`, an error naming its place in `--edits`.
pub(crate) fn each_edit(
    edits: &[Edit],
    mut apply_one: impl FnMut(&Edit) -> Result<()>,
) -> Result<()> {
    for (index, edit) in edits.iter().enumerate() {
        apply_one(edit).with_context(|| format!("edit {} of --edits", index + 1))?;
    }
    Ok(())
}

/// Writes `project` to `path`, as `--project-out` asks.
pub(crate) fn write_project(path: &Path, project: &Project) -> Result<()> {
    crate::spent::timed("project:write", || save(path, project))
        .with_context(|| format!("cannot write the project {}", path.display()))
}

/// An orient proved on the machine `--orient` proves on: a printer the shipped catalogue lists,
/// the standard one for the user's own, and the stock resin numbers, as no resin ships.
fn proved_as_shipped(mut edit: Edit) -> Edit {
    if let Edit::Orient { printer, resin, .. } = &mut edit {
        *printer = crate::stage::shipped_printer(printer.as_deref(), false);
        *resin = None;
    }
    edit
}
