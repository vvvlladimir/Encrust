use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use core_geometry::{MendMethod, MendSettings, Mended, Mesh, mend};

use crate::i18n::tr;
use crate::scene::{ObjectId, Shaped};

/// One model to close the holes in, owned so that it can cross to the worker thread.
pub struct RepairRequest {
    pub id: ObjectId,
    pub name: String,
    pub mesh: Arc<Mesh>,
}

/// One model as repair left it, in the space it was in.
#[derive(Debug, Clone, PartialEq)]
pub struct Repaired {
    pub id: ObjectId,
    pub name: String,
    pub shaped: Shaped,
    pub mended: Mended,
}

/// How one repair ended.
#[derive(Debug, Clone, PartialEq)]
pub enum RepairOutcome {
    /// Boxed because it carries a whole model, like an import's own outcome.
    Done(Box<Repaired>),
    /// The whole error chain flattened into one line, as the window has no terminal.
    Failed(String),
}

/// One model being repaired on its own thread.
///
/// Closing the holes walks the mesh's edges and rebuilds its hierarchy afterwards, which
/// is far too much for a frame on a real part; see `docs/decisions/0038`.
#[derive(Debug)]
pub struct RepairJob {
    outcome: Receiver<RepairOutcome>,
    id: ObjectId,
    name: String,
}

impl RepairJob {
    /// Starts the repair. The thread is detached: it ends on its own.
    pub fn spawn(request: RepairRequest) -> Self {
        let (sender, outcome) = mpsc::channel();
        let (id, name) = (request.id, request.name.clone());
        crate::job::spawn(move || {
            let _ = sender.send(run(request));
        });
        Self { outcome, id, name }
    }

    /// Returns the outcome on the frame the repair ends, and `None` while it is going.
    pub fn poll(&mut self) -> Option<RepairOutcome> {
        match self.outcome.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(RepairOutcome::Failed(tr!(
                "job-thread-stopped-repair",
                name = self.name.clone()
            ))),
        }
    }

    pub fn label(&self) -> String {
        tr!("job-repairing", name = self.name.clone())
    }

    /// The model being repaired.
    pub fn id(&self) -> ObjectId {
        self.id
    }
}

fn run(request: RepairRequest) -> RepairOutcome {
    let mut mesh = (*request.mesh).clone();
    let mended = mend(&mut mesh, MendMethod::Repair, MendSettings::default());
    RepairOutcome::Done(Box::new(Repaired {
        id: request.id,
        name: request.name,
        shaped: Shaped::of(Arc::new(mesh)),
        mended,
    }))
}

#[cfg(test)]
mod tests;
