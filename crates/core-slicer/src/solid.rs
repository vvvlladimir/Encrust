use std::borrow::Cow;

use core_geometry::{Aabb, Mesh, Scalar, Vec2, Vec3};
use rayon::prelude::*;

use crate::engine::{PlaneSliceEngine, SliceEngine};
use crate::{Contour, Layer, SliceError, Sliced};

/// What a stack is cut from, a set of planes at a time: one mesh, or a plate whose copies
/// are cut once.
pub trait Source: Sync {
    /// Every contour on each of `heights`, one layer a height in the order given.
    fn slice_at(&self, heights: &[Scalar]) -> Result<Sliced, SliceError>;
}

impl Source for Mesh {
    fn slice_at(&self, heights: &[Scalar]) -> Result<Sliced, SliceError> {
        PlaneSliceEngine.slice_at(self, heights)
    }
}

/// A plate as it is cut: what stands on it once, as one mesh, and what stands on it several
/// times over, cut once and laid down at each copy (`docs/design/slicing.md`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Solid {
    pub mesh: Mesh,
    pub repeated: Vec<Repeated>,
}

/// One mesh where it stands, and again at each of `offsets_mm`: copies moved along the
/// plate and in no other way.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Repeated {
    pub mesh: Mesh,
    /// How far each further copy stands from `mesh`, plate millimetres.
    pub offsets_mm: Vec<Vec2>,
}

impl From<Mesh> for Solid {
    fn from(mesh: Mesh) -> Self {
        Self {
            mesh,
            repeated: Vec::new(),
        }
    }
}

impl Solid {
    /// Whether nothing on the plate has a face to cut.
    pub fn is_empty(&self) -> bool {
        self.mesh.is_empty()
            && self
                .repeated
                .iter()
                .all(|repeated| repeated.mesh.is_empty())
    }

    /// Every distinct body once, merged: what a stack is planned from, since a copy moved
    /// along the plate adds no height and no slope the body it copies does not have.
    pub fn distinct(&self) -> Cow<'_, Mesh> {
        if self.repeated.is_empty() {
            return Cow::Borrowed(&self.mesh);
        }
        let mut merged = self.mesh.clone();
        for repeated in &self.repeated {
            merged.append(&repeated.mesh);
        }
        Cow::Owned(merged)
    }

    /// The box around everything on the plate, every copy included.
    pub fn aabb(&self) -> Option<Aabb> {
        let mut bounds = self.mesh.aabb();
        for repeated in &self.repeated {
            let Some(body) = repeated.mesh.aabb() else {
                continue;
            };
            for offset in std::iter::once(Vec2::ZERO).chain(repeated.offsets_mm.iter().copied()) {
                let shift = Vec3::new(offset.x, offset.y, 0.0);
                let (low, high) = (body.mins + shift, body.maxs + shift);
                let grown = bounds.get_or_insert(Aabb::new(low, high));
                grown.take_point(low);
                grown.take_point(high);
            }
        }
        bounds
    }
}

impl Source for Solid {
    fn slice_at(&self, heights: &[Scalar]) -> Result<Sliced, SliceError> {
        let mut sliced = if self.mesh.is_empty() {
            Sliced {
                layers: heights.iter().map(|z| Layer::empty(*z)).collect(),
                ..Sliced::default()
            }
        } else {
            PlaneSliceEngine.slice_at(&self.mesh, heights)?
        };
        for repeated in self.repeated.iter().filter(|r| !r.mesh.is_empty()) {
            let body = PlaneSliceEngine.slice_at(&repeated.mesh, heights)?;
            stamp(&mut sliced, &body, &repeated.offsets_mm);
        }
        Ok(sliced)
    }
}

/// `body` laid into `sliced` where it was cut and again at each of `offsets_mm`. What the
/// mesh made the slicer paper over is counted once a copy, as cutting each would have.
fn stamp(sliced: &mut Sliced, body: &Sliced, offsets_mm: &[Vec2]) {
    let copies = offsets_mm.len() + 1;
    sliced.open_contours += body.open_contours * copies;
    sliced.degenerate_contours += body.degenerate_contours * copies;
    sliced.unlinked_segments += body.unlinked_segments * copies;
    sliced
        .layers
        .par_iter_mut()
        .zip(&body.layers)
        .for_each(|(layer, cut)| {
            layer.contours.reserve(cut.contours.len() * copies);
            layer.contours.extend(cut.contours.iter().cloned());
            for offset in offsets_mm {
                layer
                    .contours
                    .extend(cut.contours.iter().map(|contour| moved(contour, *offset)));
            }
        });
}

fn moved(contour: &Contour, offset: Vec2) -> Contour {
    Contour::new(
        contour.points.iter().map(|point| *point + offset).collect(),
        contour.winding,
    )
}
