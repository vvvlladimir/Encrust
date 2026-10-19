use core_geometry::{Bvh, Mesh, Quat, Ray, Scalar, Transform, Vec3, raycast_placed};

use crate::drape::draped;
use crate::error::TextError;
use crate::outline::Shape;
use crate::prism::prism;

/// How far a label reaches past the surface it stands on, in millimetres, so that no face
/// of it lies on the model's and the cut has something to bite.
pub(crate) const OVERLAP_MM: Scalar = 0.2;

/// Past this share of the normal along the plate's z, a face counts as level and the
/// label reads along y instead.
const LEVEL: Scalar = 0.99;

/// Where a label stands, on the plate: the point it is centred on, the way out of the
/// surface there, and the way its letters read up, square to that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
    pub normal: Vec3,
    pub up: Vec3,
}

impl Frame {
    /// A label centred on `origin` facing out along `normal`, reading up the plate's z
    /// turned `angle_deg` anticlockwise about the normal; up its y on a level face.
    pub fn on(origin: Vec3, normal: Vec3, angle_deg: Scalar) -> Option<Self> {
        let normal = normal.try_normalize()?;
        let toward = if normal.z.abs() > LEVEL {
            Vec3::Y
        } else {
            Vec3::Z
        };
        let up = (toward - normal * toward.dot(normal)).try_normalize()?;
        let up = Quat::from_axis_angle(normal, angle_deg.to_radians()) * up;
        Some(Self { origin, normal, up })
    }

    fn right(&self) -> Vec3 {
        self.up.cross(self.normal)
    }

    /// A point of the label's own space — x right, y up, z out — on the plate.
    pub fn place(&self, local: Vec3) -> Vec3 {
        self.origin + self.right() * local.x + self.up * local.y + self.normal * local.z
    }
}

/// How a label is cut: how tall its capitals are and how far it stands out of the surface
/// or into it, in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelSettings {
    pub depth_mm: Scalar,
    /// Raised out of the surface, or sunk into it.
    pub raised: bool,
    /// Bent onto the surface letter by letter, or stood flat on the plane at the click.
    pub follow_surface: bool,
}

/// A label's prisms on the plate, one a filled shape, and whether they are to be added to
/// the model or taken out of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub prisms: Vec<Mesh>,
    pub raised: bool,
}

/// `shapes` laid at `frame` on the model `mesh` stands as under `transform`, `depth_mm` out
/// of its surface or into it.
///
/// Flat, the surface is followed only within the depth of the label's plane, so a label
/// never reaches further than twice its depth from the point it was centred on, whatever
/// stands beside it. Following the surface, every point of every letter stands the depth
/// off the surface under it. See `docs/design/labels.md`.
pub fn label(
    shapes: &[Shape],
    frame: Frame,
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    settings: LabelSettings,
) -> Result<Label, TextError> {
    let depth = settings.depth_mm;
    if !(depth.is_finite() && depth > 0.0) {
        return Err(TextError::BadSize {
            what: "depth",
            value_mm: depth,
        });
    }
    if settings.follow_surface {
        let prisms = draped(shapes, frame, mesh, bvh, transform, settings)?;
        return Ok(Label {
            prisms,
            raised: settings.raised,
        });
    }
    let (low, high) = surface_under(shapes, frame, mesh, bvh, transform, depth)?;
    let (bottom, top) = if settings.raised {
        (low - OVERLAP_MM, high.max(0.0) + depth)
    } else {
        (low.min(0.0) - depth, high + OVERLAP_MM)
    };
    let prisms = shapes
        .iter()
        .map(|shape| {
            let mut prism = prism(shape, bottom, top);
            for vertex in &mut prism.vertices {
                *vertex = frame.place(*vertex);
            }
            prism
        })
        .collect();
    Ok(Label {
        prisms,
        raised: settings.raised,
    })
}

/// How far below and above the label's plane the surface runs under its corners, along
/// the normal, in millimetres, looked for no further than `window` either side of it.
///
/// Whatever stands further off — a pillar beside the label, the far side of a thin wall —
/// is not the face being written on, and reaching for it would cut the label that deep.
fn surface_under(
    shapes: &[Shape],
    frame: Frame,
    mesh: &Mesh,
    bvh: &Bvh,
    transform: Transform,
    window: Scalar,
) -> Result<(Scalar, Scalar), TextError> {
    let corners = shapes
        .iter()
        .flat_map(|shape| shape.outer.iter().chain(shape.holes.iter().flatten()));
    let (mut low, mut high) = (Scalar::MAX, Scalar::MIN);
    for corner in corners {
        let start = frame.place(corner.extend(window));
        let Some(hit) = raycast_placed(mesh, bvh, transform, &Ray::new(start, -frame.normal))
        else {
            continue;
        };
        let height = window - hit.distance;
        if height < -window {
            continue;
        }
        low = low.min(height);
        high = high.max(height);
    }
    if low > high {
        return Err(TextError::OffTheModel);
    }
    Ok((low, high))
}
