//! `--edits` carried out through `core-document`'s apply, on a project or on models after their
//! flags, and `--project-out` writing the plate they leave (ADR 0466, 0472).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_document::{Document, ImportedModel, apply};
use core_edit::Edit;
use core_engine::project::{Project, Summary, save};
use core_geometry::{Mesh, Orientation, diagnose};
use core_volume::Cancel;

use crate::report::ImportReport;

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

    /// The edits `--edits` names, none without it.
    pub fn listed(&self) -> Result<Vec<Edit>> {
        match &self.edits {
            Some(path) => read(path),
            None => Ok(Vec::new()),
        }
    }

    /// `project` with `--edits` carried out on it, written to `--project-out` where asked.
    pub fn applied(&self, project: Project) -> Result<Project> {
        let edits = self.listed()?;
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
