use std::path::PathBuf;

use core_edit::ObjectId;
use core_geometry::Vec3;
use thiserror::Error;

/// Why an edit was not carried out; the document is left as it was.
#[derive(Debug, Clone, PartialEq, Error)]
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
    /// A label with no text, or only spaces, to write.
    #[error("there is no text to write")]
    BlankLabel,
    /// A label asked for where the object has no surface to write on.
    #[error("object {0:?} has no surface to write on there")]
    NoSurface(ObjectId),
    /// A font that cannot be read; `path` is the file asked for, none for the bundled one.
    #[error("the font could not be read: {reason}")]
    BadFont {
        path: Option<PathBuf>,
        reason: String,
    },
    /// A face number past the last face of the object's mesh, counted from 0.
    #[error("object {object:?} has no face {number}")]
    NoSuchFace { object: ObjectId, number: usize },
    /// A shell number past the last shell of the object's mesh, counted from 1.
    #[error("object {object:?} has no shell {number}")]
    NoSuchShell { object: ObjectId, number: usize },
    /// A relief picture that is missing or cannot be decoded.
    #[error("the picture {} could not be read: {reason}", path.display())]
    BadPicture { path: PathBuf, reason: String },
    /// A wall as thick as the model, leaving no room for a cavity.
    #[error("object {0:?} has no room for a cavity at that wall")]
    NoRoom(ObjectId),
    /// A drain point nearest an edge or corner, where its hole would sit half off the model.
    #[error("the point {at_mm} on object {object:?} is nearest an edge or corner, at {mouth_mm}")]
    OffAFace {
        object: ObjectId,
        at_mm: Vec3,
        /// Where the hole's mouth would be, in plate mm.
        mouth_mm: Vec3,
    },
    /// A drain search on an object with no cavity to drain.
    #[error("object {0:?} has no cavity to drain")]
    NoCavity(ObjectId),
    /// An object with no surface, which stands nowhere on the plate to be arranged.
    #[error("object {0:?} has no footprint on the plate")]
    NoFootprint(ObjectId),
    /// An object whose transform cannot be inverted, so no plate point maps onto it.
    #[error("object {0:?} has a transform that cannot be inverted")]
    BadTransform(ObjectId),
    /// A support profile that is not one this build ships, `auto` among them: its sizing is the
    /// window's catalogue's, not the document's.
    #[error("no shipped support profile is named {0:?}")]
    NoProfile(String),
    /// A printer the catalogue does not list, named by an orient to prove its poses on.
    #[error("no printer {0:?} in the catalogue")]
    NoPrinter(String),
    /// A resin the catalogue does not list, named by an orient to prove its poses with.
    #[error("no resin {0:?} in the catalogue")]
    NoResin(String),
    /// The shipped catalogue could not be read, so no machine is known to prove poses on.
    #[error("the printer catalogue could not be read: {0}")]
    BadCatalogue(String),
    /// An orient asked for a pose past the last the search found.
    #[error("object {object:?} has no pose {pick}: the search found {found}")]
    NoChoice {
        object: ObjectId,
        pick: usize,
        found: usize,
    },
    /// The run was stopped before it finished.
    #[error("the edit was cancelled")]
    Cancelled,
    /// A split, part-off, combination, repair, bridge or relief failed in the geometry; `reason` is the geometry's own message.
    #[error("the edit of object {object:?} failed: {reason}")]
    Failed { object: ObjectId, reason: String },
}
