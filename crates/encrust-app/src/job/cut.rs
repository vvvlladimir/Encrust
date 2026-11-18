use core_document::{ApplyError, CutAsked, CutModel, Piece, cut, pieces};
use core_edit::ObjectId;

use crate::i18n::tr;
use crate::job::{Worker, pooled};

/// One model to cut, and how.
pub struct CutRequest {
    pub id: ObjectId,
    pub model: CutModel,
    pub asked: CutAsked,
}

/// How a cut ended.
#[derive(Debug)]
pub enum CutOutcome {
    Cut {
        /// The halves kept and the loose dowels, laid out as they stand on the plate.
        pieces: Vec<Piece>,
        /// Pins, dowels or magnets put in, or pieces of the face a key runs round.
        placed: usize,
        /// Pieces of the cut face too small or thin to take a fixing.
        bare: usize,
        /// Cut loops that did not close, and were left uncapped.
        open_loops: usize,
        /// Why the halves were left plain when they were to be joined.
        unjoined: Option<String>,
    },
    /// The plane does not cross the model, which is left as it was.
    Missed,
    /// The model's holes cannot be taken out exactly, so halves cut from it would lose them.
    Undrilled,
    Failed(String),
}

/// A cut on its own thread: building pins into a real model is a boolean of a second or
/// more.
pub type CutJob = Worker<CutOutcome>;

/// Starts the cut. The thread is detached: it ends on its own.
pub fn spawn_cut(request: CutRequest) -> CutJob {
    Worker::spawn(
        // Neither the cut nor the joint can be given up on part way.
        move |_, _| pooled(|| cut_and_lay(&request), CutOutcome::Failed),
        || CutOutcome::Failed(tr!("job-thread-stopped-cut")),
    )
}

fn cut_and_lay(request: &CutRequest) -> CutOutcome {
    match cut(&request.model, &request.asked, request.id) {
        Ok(done) => CutOutcome::Cut {
            placed: done.joined.placed,
            bare: done.joined.bare,
            open_loops: done.open_loops,
            unjoined: done.unjoined.map(|error| error.to_string()),
            pieces: pieces(done.joined, &request.asked),
        },
        Err(ApplyError::Missed(_)) => CutOutcome::Missed,
        Err(ApplyError::Undrilled(_)) => CutOutcome::Undrilled,
        Err(error) => CutOutcome::Failed(error.to_string()),
    }
}
