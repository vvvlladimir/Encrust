//! A desk beside the plate in the render view: a cutting mat with things everyone knows the
//! size of lying on it, at their real size. See docs/design/viewport.md.

use std::f32::consts::FRAC_PI_2;
use std::sync::{Arc, OnceLock};

use core_geometry::{Mesh, Quat, Scalar, Transform, Vec2, Vec3, smooth_normals};
use core_supports::Shaded;

use crate::plate::BuildPlate;
use crate::render::gpu::{MAX_PROP_SHADOWS, ModelDraw, PropShadow};
use crate::render::vertex::ModelInstance;
use crate::ui::theme;

mod shapes;

/// An A4 self-healing cutting mat, millimetres: across, along and thick.
pub const MAT_MM: [Scalar; 3] = [220.0, 300.0, 3.0];
/// Millimetres between the plate's edge and the mat.
const GAP_MM: Scalar = 15.0;
/// Degrees two faces may turn apart and still be shaded as one curved surface.
const CREASE_DEG: Scalar = 40.0;

/// What a reference is made of, which the shader paints it as: `prop_fragment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    Mat = 0,
    Can = 1,
    Banana = 2,
    Pen = 3,
    Coin = 4,
    Steel = 5,
    Primer = 6,
}

/// A prop's instance names its material in the slot a model's overhang threshold takes:
/// `PROP_BASE - material`, below anything a model or marker carries.
const PROP_BASE: f32 = -2.0;

/// One reference: its mesh in its own frame, what it is made of, and where it lies on the
/// mat, millimetres from the mat's front corner nearest the plate, turned by `turn`.
pub struct Reference {
    pub shaded: Shaded,
    pub material: Material,
    pub at: Vec3,
    pub turn: Quat,
}

fn reference(mesh: Mesh, material: Material, at: Vec3, turn: Quat) -> Reference {
    let normals = Arc::new(smooth_normals(&mesh, CREASE_DEG));
    Reference {
        shaded: Shaded {
            mesh: Arc::new(mesh),
            normals,
        },
        material,
        at,
        turn,
    }
}

/// Where the banana's arc is centred on the mat, millimetres, and how it is turned.
const BANANA_AT: Vec2 = Vec2::new(247.5, 150.0);

/// Built once: the GPU cache keys a mesh by its address, so the same `Arc` every frame.
pub fn desk() -> &'static [Reference] {
    static BUILT: OnceLock<Vec<Reference>> = OnceLock::new();
    BUILT.get_or_init(|| {
        let top = MAT_MM[2];
        let flat = Quat::IDENTITY;
        vec![
            reference(shapes::mat(), Material::Mat, Vec3::ZERO, flat),
            reference(
                shapes::ruler(),
                Material::Steel,
                Vec3::new(18.0, 65.0, top),
                flat,
            ),
            reference(
                shapes::pen(),
                Material::Pen,
                Vec3::new(38.0, 15.0, top + shapes::PEN_REST_MM),
                Quat::from_rotation_x(-FRAC_PI_2),
            ),
            reference(
                shapes::coin(),
                Material::Coin,
                Vec3::new(75.0, 28.0, top),
                flat,
            ),
            reference(
                shapes::figure(),
                Material::Primer,
                Vec3::new(75.0, 72.0, top),
                flat,
            ),
            reference(
                shapes::can(),
                Material::Can,
                Vec3::new(85.0, 140.0, top),
                flat,
            ),
            reference(
                shapes::banana(),
                Material::Banana,
                BANANA_AT.extend(top),
                Quat::from_rotation_z(FRAC_PI_2),
            ),
        ]
    })
}

/// Where the mat's front corner nearest the plate stands, and how it is turned: off the
/// plate's side to the viewer's right, its middle level with the plate's.
fn mat_frame(plate: &BuildPlate) -> (Vec3, Quat) {
    let turn = plate.front_turn();
    let (rightward, away) = (turn * Vec3::X, turn * Vec3::Y);
    let extent = |axis: Vec3| axis.x.abs() * plate.x_mm + axis.y.abs() * plate.y_mm;
    let corner =
        plate.center() + rightward * (extent(rightward) / 2.0 + GAP_MM) - away * MAT_MM[1] / 2.0;
    (corner, turn)
}

/// Each reference with where it stands on the plate.
pub fn placed(plate: &BuildPlate) -> Vec<(&'static Reference, Transform)> {
    let (corner, turn) = mat_frame(plate);
    desk()
        .iter()
        .map(|reference| {
            let at = corner + turn * reference.at;
            let transform =
                Transform::from_scale_rotation_translation(Vec3::ONE, turn * reference.turn, at);
            (reference, transform)
        })
        .collect()
}

/// The references as markers, smooth-shaded, each naming its material to the shader:
/// drawn, never picked, sliced or capped.
pub fn drawn(plate: &BuildPlate) -> impl Iterator<Item = ModelDraw> {
    placed(plate).into_iter().map(|(reference, at)| {
        let material = PROP_BASE - f32::from(reference.material as u8);
        let instance = ModelInstance::new(at, theme::scene().reference, material);
        ModelDraw::smooth(&reference.shaded, instance)
    })
}

/// The soft shadows the props cast on the mat, in the mat's own millimetres: each a
/// segment and the reach of what stands on it. The banana's curve takes three.
pub fn shadows() -> [PropShadow; MAX_PROP_SHADOWS] {
    let line = PropShadow::new;
    let dot = |at: Vec2, reach_mm: f32| PropShadow::new(at, at, reach_mm);
    let banana = |share: f32| {
        let spine = shapes::banana_spine(share);
        BANANA_AT + Vec2::new(-spine.y, spine.x)
    };
    [
        line(Vec2::new(18.0, 15.0), Vec2::new(18.0, 115.0), 6.0),
        line(
            Vec2::new(38.0, 15.0),
            Vec2::new(38.0, 15.0 + shapes::PEN_MM),
            4.0,
        ),
        dot(Vec2::new(75.0, 28.0), shapes::COIN_MM[0] / 2.0),
        dot(Vec2::new(75.0, 72.0), shapes::BASE_MM[0] / 2.0),
        dot(Vec2::new(85.0, 140.0), shapes::CAN_RADIUS_MM),
        line(banana(0.05), banana(0.35), 12.0),
        line(banana(0.35), banana(0.65), 16.0),
        line(banana(0.65), banana(0.95), 12.0),
    ]
}

#[cfg(test)]
mod tests;
