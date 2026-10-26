//! Work on one model kept while the model's mesh is the one it was of: started off the
//! drawing thread, taken in when done, dropped once the mesh is replaced.

use crate::job::{Worker, pooled};
use crate::scene::{Scene, SceneObject, Taken};

/// A piece of work on one model: running, or done with an answer or without one.
#[derive(Debug)]
pub(super) enum Pending<T> {
    Running(Worker<Option<T>>),
    Done(Option<T>),
}

/// Starts `work` on `object` unless what is kept for it is of its mesh as it stands.
pub(super) fn start<T: Send + 'static>(
    kept: &mut Vec<(Taken, Pending<T>)>,
    object: &SceneObject,
    work: impl FnOnce() -> T + Send + 'static,
) {
    if kept.iter().any(|(taken, _)| taken.is_model(object)) {
        return;
    }
    kept.retain(|(taken, _)| taken.id != object.id);
    let worker = Worker::spawn(move |_, _| pooled(|| Some(work()), |_| None), || None);
    kept.push((Taken::of(object), Pending::Running(worker)));
}

/// What is kept for `object` as it stands: `None` when nothing, `Some(None)` while it runs
/// or where it ended without an answer.
pub(super) fn done<'a, T>(
    kept: &'a [(Taken, Pending<T>)],
    object: &SceneObject,
) -> Option<Option<&'a T>> {
    let (_, pending) = kept.iter().find(|(taken, _)| taken.is_model(object))?;
    Some(match pending {
        Pending::Running(_) => None,
        Pending::Done(answer) => answer.as_ref(),
    })
}

pub(super) fn running<T>(kept: &[(Taken, Pending<T>)]) -> bool {
    kept.iter()
        .any(|(_, pending)| matches!(pending, Pending::Running(_)))
}

/// Takes in the answers that have come, and drops what was of a mesh no longer on the plate.
pub(super) fn settle<T: Send + 'static>(kept: &mut Vec<(Taken, Pending<T>)>, scene: &Scene) {
    kept.retain_mut(|(taken, pending)| {
        if let Pending::Running(worker) = pending
            && let Some(answer) = worker.poll()
        {
            *pending = Pending::Done(answer);
        }
        scene
            .get(taken.id)
            .is_some_and(|object| taken.is_model(object))
    });
}
