//! `--edits` carried out on a project through `core-document`'s apply, and `--project-out`
//! writing the plate they leave (ADR 0466).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use core_document::{Document, apply};
use core_edit::Edit;
use core_engine::project::{Project, save};
use core_volume::Cancel;

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

    /// Refuses the flags for models, which their own flags shape.
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

fn read(path: &Path) -> Result<Vec<Edit>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the edits {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("{} is not a JSON list of edits", path.display()))
}
