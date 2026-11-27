//! Text written on a model, raised or sunk (`docs/design/labels.md`).

use std::path::PathBuf;
use std::sync::Arc;

use core_edit::{Edit, ObjectId};
use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_text::{
    Font, Frame, LABEL_DEPTH_MM, LETTER_HEIGHT_MM, LabelSettings, TextError, label, outline,
};
use core_volume::{Boolean, Cancel};

use crate::apply::{Applied, Report};
use crate::combine::combined;
use crate::document::Document;
use crate::error::ApplyError;
use crate::piece::{Piece, Role};

/// The face labels are written in unless another is named, the window's own.
const BUNDLED_FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/fonts/IBMPlexSans-Regular.ttf"
));

/// What is written, and where and how.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelAsked {
    pub text: String,
    /// The plate point the label is written nearest to, mm.
    pub at_mm: Vec3,
    pub height_mm: Scalar,
    pub depth_mm: Scalar,
    pub raised: bool,
    pub follow_surface: bool,
    /// How far the line is turned about the surface's normal, degrees anticlockwise.
    pub angle_deg: Scalar,
    /// A font file; the bundled IBM Plex Sans where unset.
    pub font: Option<PathBuf>,
}

impl LabelAsked {
    /// What a label edit asks, with the Text tool's size where the edit names none; `None`
    /// for any other edit.
    pub fn of(edit: &Edit) -> Option<Self> {
        let Edit::Label {
            text,
            at_mm,
            height_mm,
            depth_mm,
            raised,
            follow_surface,
            angle_deg,
            font,
            ..
        } = edit
        else {
            return None;
        };
        Some(Self {
            text: text.clone(),
            at_mm: *at_mm,
            height_mm: height_mm.unwrap_or(LETTER_HEIGHT_MM),
            depth_mm: depth_mm.unwrap_or(LABEL_DEPTH_MM),
            raised: *raised,
            follow_surface: *follow_surface,
            angle_deg: *angle_deg,
            font: font.clone(),
        })
    }
}

/// The prisms of a label laid on a model, and the way out of the surface they stand on.
#[derive(Debug, Clone, PartialEq)]
pub struct Lettering {
    pub prisms: Vec<Mesh>,
    /// Added to the model, or taken out of it.
    pub raised: bool,
    pub normal: Vec3,
}

impl Lettering {
    /// The models to combine, `placed` first, and the operation that stamps the lettering.
    pub fn stamp(self, placed: &Mesh) -> (Vec<Mesh>, Boolean) {
        let op = if self.raised {
            Boolean::Union
        } else {
            Boolean::Difference
        };
        let mut models = vec![placed.clone()];
        models.extend(self.prisms);
        (models, op)
    }
}

/// `asked` laid on `placed`, an object where it stands on the plate, named `object` in an
/// error; the stamp is left to [`combined`].
pub fn label_prisms(
    placed: &Mesh,
    asked: &LabelAsked,
    object: ObjectId,
) -> Result<Lettering, ApplyError> {
    let font = font_of(asked.font.as_ref())?;
    let refused = |error| text_error(error, object, asked.font.as_ref());
    let shapes = outline(&font, &asked.text, asked.height_mm).map_err(refused)?;
    let bvh = Bvh::build(placed);
    let frame = frame_at(placed, &bvh, asked).ok_or(ApplyError::NoSurface(object))?;
    let settings = LabelSettings {
        depth_mm: asked.depth_mm,
        raised: asked.raised,
        follow_surface: asked.follow_surface,
    };
    let written =
        label(&shapes, frame, placed, &bvh, Transform::default(), settings).map_err(refused)?;
    Ok(Lettering {
        prisms: written.prisms,
        raised: written.raised,
        normal: frame.normal,
    })
}

/// `placed` with `asked` written on it, named `object` in an error.
pub fn labelled(
    placed: &Mesh,
    asked: &LabelAsked,
    cancel: Cancel<'_>,
    object: ObjectId,
) -> Result<Mesh, ApplyError> {
    let lettering = label_prisms(placed, asked, object)?;
    let toward = lettering.normal;
    let (models, op) = lettering.stamp(placed);
    Ok(combined(&models, op, Some(toward), cancel, object)?.mesh)
}

fn font_of(path: Option<&PathBuf>) -> Result<Font, ApplyError> {
    let bad = |reason: String| ApplyError::BadFont {
        path: path.cloned(),
        reason,
    };
    let bytes = match path {
        Some(path) => std::fs::read(path).map_err(|error| bad(error.to_string()))?,
        None => BUNDLED_FONT.to_vec(),
    };
    Font::new(Arc::new(bytes), 0).map_err(|error| match error {
        TextError::BadFont(reason) => bad(reason),
        other => bad(other.to_string()),
    })
}

/// The frame on the face of `placed` nearest the asked point, none where there is no face
/// with an area and a direction.
fn frame_at(placed: &Mesh, bvh: &Bvh, asked: &LabelAsked) -> Option<Frame> {
    let nearest = bvh.closest(asked.at_mm)?;
    let normal = placed.triangle(nearest.face)?.normal_unnormalized();
    Frame::on(nearest.point, normal, asked.angle_deg)
}

/// What `error` is to a label on `object` in the font file `font`, the bundled one where none.
fn text_error(error: TextError, object: ObjectId, font: Option<&PathBuf>) -> ApplyError {
    match error {
        TextError::Blank => ApplyError::BlankLabel,
        TextError::BadFont(reason) => ApplyError::BadFont {
            path: font.cloned(),
            reason,
        },
        TextError::OffTheModel => ApplyError::NoSurface(object),
        other => ApplyError::Failed {
            object,
            reason: other.to_string(),
        },
    }
}

/// Replaces `id` by itself with `asked` written on it, where it stands.
pub(crate) fn apply_label(
    document: &mut Document,
    id: ObjectId,
    asked: &LabelAsked,
    cancel: Cancel<'_>,
) -> Result<Applied, ApplyError> {
    let (state, meshes) = document.object(id).ok_or(ApplyError::NoObject(id))?;
    let placed = transform_mesh(&meshes.source, state.transform);
    let written = labelled(&placed, asked, cancel, id)?;
    let added = document.replace(id, vec![Piece::sound(Role::Kept, written)]);
    Ok(Applied {
        removed: vec![id],
        added,
        report: Report::Labelled,
    })
}

#[cfg(test)]
mod tests;
