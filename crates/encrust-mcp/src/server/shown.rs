//! `show_in_window`: a revision on the open window's plate because the client asked, the
//! window started where none answers (ADR 0465).

use anyhow::{Result, bail};
use serde_json::json;

use super::Server;
use super::reply::Reply;
use crate::params::JobRef;

impl Server {
    /// Shows the revision `at` names in the window, and says whether the window was started.
    pub(super) async fn shown(&self, at: JobRef) -> Result<Reply> {
        let revision = self.jobs.open(&at.job, at.revision)?;
        if revision.recipe.is_project() {
            bail!(
                "{} is a project: it opens in the window from File, Open project",
                revision.recipe.model.display()
            );
        }
        let (window, started) = self.window.show_now(&revision).await?;
        let shown = window["shown"] == true;
        Ok(Reply {
            body: json!({
                "job": revision.job,
                "revision": revision.number,
                "window": window,
                "started": started,
                "unshown": revision.recipe.unshown(),
            }),
            failed: !shown,
        })
    }
}
