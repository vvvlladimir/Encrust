use core_geometry::{Bvh, Mesh, Ray, Scalar, Transform, Vec2, raycast_placed};
use rayon::prelude::*;

use crate::error::TextError;
use crate::label::{Frame, LabelSettings, OVERLAP_MM};
use crate::outline::Shape;
use crate::prism::stand;
use crate::sheet::Sheet;

/// The fill's spacing as a share of the label's longest side, so a long label is not cut
/// into more triangles than a short one.
const SPACING_SHARE: Scalar = 1.0 / 160.0;

/// The finest the fill is cut, in millimetres: the sag of a chord this long on a curve of
/// 5 mm radius is under 2 µm.
const MIN_SPACING_MM: Scalar = 0.25;

/// `shapes` laid at `frame` and bent onto the model's surface point by point, each letter
/// standing `depth_mm` out of it or into it everywhere, as prisms on the plate.
///
/// The surface is looked for no further from the label's plane than half its longest side,
/// or twice its depth, whichever is more. See `docs/design/labels.md`.
pub(crate) fn draped(
    shapes: &[Shape],
    frame: Frame,
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    settings: LabelSettings,
) -> Result<Vec<Mesh>, TextError> {
    let longest = longest_side(shapes);
    let spacing = (longest * SPACING_SHARE).max(MIN_SPACING_MM);
    let reach = (longest / 2.0).max(2.0 * settings.depth_mm);
    let sheets: Vec<Sheet> = shapes
        .par_iter()
        .map(|shape| Sheet::fine(shape, spacing))
        .collect();
    let surface = Surface {
        frame,
        mesh,
        bvh,
        transform,
        reach,
    };
    let heights: Vec<Vec<Option<Scalar>>> = sheets
        .iter()
        .map(|sheet| sheet.flat.par_iter().map(|p| surface.height(*p)).collect())
        .collect();
    let found: Vec<Scalar> = heights.iter().flatten().flatten().copied().collect();
    if found.is_empty() {
        return Err(TextError::OffTheModel);
    }
    let mean = found.iter().sum::<Scalar>() / found.len() as Scalar;
    let (below, above) = if settings.raised {
        (OVERLAP_MM, settings.depth_mm)
    } else {
        (settings.depth_mm, OVERLAP_MM)
    };
    Ok(sheets
        .iter()
        .zip(&heights)
        .map(|(sheet, heights)| {
            let surface: Vec<Scalar> = heights.iter().map(|h| h.unwrap_or(mean)).collect();
            let bottom: Vec<Scalar> = surface.iter().map(|h| h - below).collect();
            let top: Vec<Scalar> = surface.iter().map(|h| h + above).collect();
            let mut prism = stand(sheet, &bottom, &top);
            for vertex in &mut prism.vertices {
                *vertex = frame.place(*vertex);
            }
            prism
        })
        .collect())
}

fn longest_side(shapes: &[Shape]) -> Scalar {
    let points = shapes.iter().flat_map(|shape| shape.outer.iter());
    let (low, high) = points.fold((Vec2::MAX, Vec2::MIN), |(low, high), point| {
        (low.min(*point), high.max(*point))
    });
    (high - low).max_element().max(0.0)
}

/// The model's surface under a label, as heights off the label's plane.
struct Surface<'a> {
    frame: Frame,
    mesh: &'a Mesh,
    bvh: &'a Bvh,
    transform: Transform,
    /// The furthest off the plane the surface is looked for, millimetres.
    reach: Scalar,
}

impl Surface<'_> {
    /// How far above the label's plane the surface stands at `point` of it, along the
    /// normal, in millimetres; `None` where there is none within reach.
    ///
    /// The ray goes down from the plane first. Where the face it meets looks down too, the
    /// point is inside the model and the surface is the one above it.
    fn height(&self, point: Vec2) -> Option<Scalar> {
        let start = self.frame.place(point.extend(0.0));
        let normal = self.frame.normal;
        let down = raycast_placed(
            self.mesh,
            self.bvh,
            self.transform,
            &Ray::new(start, -normal),
        );
        let height = match down {
            Some(hit) if hit.normal.dot(normal) >= 0.0 => -hit.distance,
            _ => {
                let up = Ray::new(start, normal);
                let hit = raycast_placed(self.mesh, self.bvh, self.transform, &up)?;
                // An overhang above a point beside the model is not the face written on.
                (hit.normal.dot(normal) > 0.0).then_some(hit.distance)?
            }
        };
        (height.abs() <= self.reach).then_some(height)
    }
}
