use core_edit::ObjectId;
use thiserror::Error;

/// Why an edit was not carried out; the document is left as it was.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplyError {
    #[error("no object {0:?} on the plate")]
    NoObject(ObjectId),
    #[error("the cut plane misses object {0:?}")]
    Missed(ObjectId),
    /// The object's drain holes cannot be taken out exactly, so its halves would lose them.
    #[error("the holes of object {0:?} cannot be taken out before it is cut")]
    Undrilled(ObjectId),
    /// A scale factor that is not finite and above zero.
    #[error("a scale for object {0:?} is not finite and above zero")]
    BadScale(ObjectId),
    /// A part-off at a point that is off the model, or on a body with no neck to cut at.
    #[error("object {0:?} has no neck to cut a part off at there")]
    NoNeck(ObjectId),
    /// A combination of fewer than two objects.
    #[error("a combination takes two objects or more")]
    TooFew,
    /// A combination that leaves no volume, as an intersection of objects that share none.
    #[error("the combination leaves nothing")]
    NothingLeft,
    /// An object the exact combination cannot read as a closed solid.
    #[error("object {0:?} is not a closed solid")]
    NotSolid(ObjectId),
    /// The run was stopped before it finished.
    #[error("the edit was cancelled")]
    Cancelled,
    /// A split, part-off or combination failed in the geometry; `reason` is the geometry's own message.
    #[error("the edit of object {object:?} failed: {reason}")]
    Failed { object: ObjectId, reason: String },
}
