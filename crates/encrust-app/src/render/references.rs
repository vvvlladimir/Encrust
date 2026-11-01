//! The render view's set: a cutting mat under the models with things everyone knows the size
//! of lying around them, at their real size. See docs/design/viewport.md.

use std::f32::consts::FRAC_PI_2;
use std::sync::{Arc, OnceLock};

use core_geometry::{Aabb, Mesh, Quat, Scalar, Transform, Vec2, Vec3, cuboid, smooth_normals};
use core_supports::Shaded;

use crate::plate::BuildPlate;
use crate::render::gpu::{Desk, ModelDraw};
use crate::render::vertex::ModelInstance;
use crate::ui::theme;

mod shapes;

/// Self-healing cutting mats as sold, millimetres across and along: A3, A2, A1 and A0. A set
/// past the largest takes a mat cut to it.
const MAT_SIZES_MM: [[Scalar; 2]; 4] = [
    [450.0, 300.0],
    [600.0, 450.0],
    [900.0, 600.0],
    [1200.0, 900.0],
];
pub const MAT_THICK_MM: Scalar = 3.0;
/// Millimetres the mat's top lies under the plate's level, so the models' contact shadow,
/// laid at that level, never fights it for depth.
pub const SINK_MM: Scalar = 0.05;
/// Millimetres kept clear round the models, between props, and round the set on the mat.
const MODEL_GAP_MM: Scalar = 20.0;
const PROP_GAP_MM: Scalar = 12.0;
const MAT_MARGIN_MM: Scalar = 30.0;
/// Degrees two faces may turn apart and still be shaded as one curved surface.
const CREASE_DEG: Scalar = 40.0;

/// What a reference is made of, which the shader paints it as: `prop_colour`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    Mat = 0,
    Can = 1,
    Banana = 2,
    Pen = 3,
    Coin = 4,
    Steel = 5,
}

/// A prop's instance names its material in the slot a model's overhang threshold takes:
/// `PROP_BASE - material`, below anything a model or marker carries.
const PROP_BASE: f32 = -2.0;

/// One prop: its mesh in its own frame, what it is made of, how it is turned to lie as it
/// does, and the box it fills turned so.
pub struct Prop {
    pub shaded: Shaded,
    pub material: Material,
    pub turn: Quat,
    pub bounds: Aabb,
}

fn shaded(mesh: Mesh) -> Shaded {
    let normals = Arc::new(smooth_normals(&mesh, CREASE_DEG));
    Shaded {
        mesh: Arc::new(mesh),
        normals,
    }
}

fn prop(mesh: Mesh, material: Material, turn: Quat) -> Prop {
    let turned = core_geometry::transform_mesh(
        &mesh,
        Transform::from_scale_rotation_translation(Vec3::ONE, turn, Vec3::ZERO),
    );
    let bounds = turned.aabb().unwrap_or(Aabb::new(Vec3::ZERO, Vec3::ZERO));
    Prop {
        shaded: shaded(mesh),
        material,
        turn,
        bounds,
    }
}

/// Built once: the GPU cache keys a mesh by its address, so the same `Arc` every frame.
/// In the order `lay_out` places them: the front row, the left column, the right one.
pub fn props() -> &'static [Prop] {
    static BUILT: OnceLock<Vec<Prop>> = OnceLock::new();
    BUILT.get_or_init(|| {
        let along_u = Quat::from_rotation_z(-FRAC_PI_2);
        let flat = Quat::IDENTITY;
        vec![
            prop(
                shapes::pen(),
                Material::Pen,
                along_u * Quat::from_rotation_x(-FRAC_PI_2),
            ),
            prop(shapes::ruler(), Material::Steel, along_u),
            prop(shapes::coin(), Material::Coin, flat),
            prop(shapes::can(), Material::Can, flat),
            prop(
                shapes::banana(),
                Material::Banana,
                Quat::from_rotation_z(FRAC_PI_2),
            ),
        ]
    })
}

/// The mat as a unit box, sized by its placement's scale.
fn mat_box() -> &'static Shaded {
    static BUILT: OnceLock<Shaded> = OnceLock::new();
    BUILT.get_or_init(|| shaded(cuboid(Vec3::ZERO, Vec3::ONE)))
}

/// A rectangle in the viewer's frame: millimetres right of the plate's middle and away from
/// it, as the viewer faces the plate's front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub low: Vec2,
    pub high: Vec2,
}

impl Rect {
    fn size(self) -> Vec2 {
        self.high - self.low
    }

    fn grown(self, by: Scalar) -> Self {
        Self {
            low: self.low - Vec2::splat(by),
            high: self.high + Vec2::splat(by),
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            low: self.low.min(other.low),
            high: self.high.max(other.high),
        }
    }
}

/// The viewer's frame over the plate: its middle, and the plate's right and away.
struct Frame {
    middle: Vec3,
    turn: Quat,
}

impl Frame {
    fn of(plate: &BuildPlate) -> Self {
        Self {
            middle: plate.center(),
            turn: plate.front_turn(),
        }
    }

    fn to_world(&self, at: Vec3) -> Vec3 {
        self.middle + self.turn * at
    }

    /// The rectangle `bounds` covers, seen in this frame.
    fn rect_of(&self, bounds: &Aabb) -> Rect {
        let back = self.turn.inverse();
        let corners = [bounds.mins, bounds.maxs].map(|at| (back * (at - self.middle)).truncate());
        Rect {
            low: corners[0].min(corners[1]),
            high: corners[0].max(corners[1]),
        }
    }
}

/// Where each prop and the mat lie: the props round `models`, the plate's middle if there
/// are none, and the smallest mat sold that takes them all with a margin.
pub struct Layout {
    pub placed: Vec<(&'static Prop, Rect)>,
    pub mat: Rect,
}

/// The front row under the models (the pen, then the ruler before it), the coin and the can
/// left of them, the banana right: each row clear of the others by its side.
pub fn lay_out(models: Rect) -> Layout {
    let clear = models.grown(MODEL_GAP_MM);
    let [pen, ruler, coin, can, banana] = [0, 1, 2, 3, 4].map(|at| &props()[at]);
    let size = |prop: &Prop| (prop.bounds.maxs - prop.bounds.mins).truncate();
    let at = |prop: &'static Prop, low: Vec2| {
        let rect = Rect {
            low,
            high: low + size(prop),
        };
        (prop, rect)
    };
    let pen_low = Vec2::new(clear.low.x, clear.low.y - size(pen).y);
    let ruler_low = pen_low - Vec2::new(0.0, PROP_GAP_MM + size(ruler).y);
    let coin_low = Vec2::new(clear.low.x - size(coin).x, clear.low.y);
    let can_y = (clear.high.y - size(can).y).max(coin_low.y + size(coin).y + PROP_GAP_MM);
    let can_low = Vec2::new(clear.low.x - size(can).x, can_y);
    let banana_y = (clear.low.y + clear.high.y - size(banana).y) / 2.0;
    let banana_low = Vec2::new(clear.high.x, banana_y.max(clear.low.y));
    let placed = vec![
        at(pen, pen_low),
        at(ruler, ruler_low),
        at(coin, coin_low),
        at(can, can_low),
        at(banana, banana_low),
    ];
    let set = placed
        .iter()
        .fold(models, |set, (_, rect)| set.union(*rect))
        .grown(MAT_MARGIN_MM);
    Layout {
        placed,
        mat: mat_round(set),
    }
}

/// The smallest mat sold that covers `set`, either way round, centred on it.
fn mat_round(set: Rect) -> Rect {
    let need = set.size();
    let size = MAT_SIZES_MM
        .iter()
        .flat_map(|&[long, short]| [Vec2::new(long, short), Vec2::new(short, long)])
        .find(|size| size.x >= need.x && size.y >= need.y)
        .unwrap_or(need);
    let middle = (set.low + set.high) / 2.0;
    Rect {
        low: middle - size / 2.0,
        high: middle + size / 2.0,
    }
}

/// The set laid round the models standing in `models`, or round the plate's middle, as
/// draws and as what the shader needs to paint the mat.
pub fn set(plate: &BuildPlate, models: Option<Aabb>) -> (Vec<ModelDraw>, Desk) {
    let frame = Frame::of(plate);
    let around = models.map_or(
        Rect {
            low: Vec2::ZERO,
            high: Vec2::ZERO,
        },
        |bounds| frame.rect_of(&bounds),
    );
    let layout = lay_out(around);
    let floor = -SINK_MM;
    let mut draws = Vec::with_capacity(layout.placed.len() + 1);
    for (prop, rect) in &layout.placed {
        let at = rect.low.extend(floor) - prop.bounds.mins;
        let world = frame.to_world(at);
        let transform =
            Transform::from_scale_rotation_translation(Vec3::ONE, frame.turn * prop.turn, world);
        draws.push(draw(&prop.shaded, prop.material, transform));
    }
    let mat = layout.mat;
    let mat_low = frame.to_world(mat.low.extend(floor - MAT_THICK_MM));
    let mat_high = frame.to_world(mat.high.extend(floor));
    let scale = mat.size().extend(MAT_THICK_MM);
    let placement = Transform::from_scale_rotation_translation(scale, frame.turn, mat_low);
    draws.push(draw(mat_box(), Material::Mat, placement));
    let desk = Desk {
        rect: [
            mat_low.x.min(mat_high.x),
            mat_low.y.min(mat_high.y),
            mat_low.x.max(mat_high.x),
            mat_low.y.max(mat_high.y),
        ],
        mat_mm: mat.size().to_array(),
    };
    (draws, desk)
}

fn draw(shaded: &Shaded, material: Material, at: Transform) -> ModelDraw {
    let slot = PROP_BASE - f32::from(material as u8);
    ModelDraw::smooth(
        shaded,
        ModelInstance::new(at, theme::scene().reference, slot),
    )
}

#[cfg(test)]
mod tests;
