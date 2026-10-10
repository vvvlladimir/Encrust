//! What one frame hands the card: the models, the cut surfaces, the textured models and
//! the per-fragment records — drain cuts, exposure bands and trapped pockets.

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use core_geometry::{Aabb, Mat4, Mesh, Scalar, Vec2, Vec3};

use crate::render::vertex::{LabelVertex, LineVertex, ModelInstance};
use crate::scene::Mapped;
use crate::ui::theme;

/// The box one pocket of trapped resin stands in, plate millimetres: what the cavity is
/// painted red inside of, and nowhere else. See ADR 0200.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct TrapBox {
    /// `xyz` is the near corner; `w` is unused padding.
    pub low: [f32; 4],
    /// `xyz` is the far corner; `w` is unused padding.
    pub high: [f32; 4],
}

/// One band of print height that takes an exposure of its own, for the fragment shader to
/// tint the models with.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct ExposureBand {
    /// `x` is the bottom of the band and `y` its top, plate millimetres.
    span: [f32; 4],
    /// `xyz` is what the band washes the surface with.
    tint: [f32; 4],
}

impl ExposureBand {
    /// Takes a token rather than floats so that no colour can be spelled in the renderer.
    pub fn new(from_mm: f32, to_mm: f32, tint: egui::Color32) -> Self {
        Self {
            span: [from_mm, to_mm, 0.0, 0.0],
            tint: theme::gamma(tint),
        }
    }
}

/// One drain hole or channel segment, in plate millimetres, for the fragment shader to
/// subtract from whatever is drawn.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct DrainCut {
    /// `xyz` is the mouth, `w` the radius there.
    pub mouth: [f32; 4],
    /// `xyz` is the tip, `w` the radius there.
    pub tip: [f32; 4],
}

/// One frame's worth of what to draw, handed over in one piece rather than as a row of
/// parallel arguments.
pub struct FrameInput<'a> {
    pub view_projection: Mat4,
    /// Where the camera stands, plate millimetres, which is what the x-ray measures a
    /// surface's lean against.
    pub eye: Vec3,
    /// Height the models are cut at, plate millimetres, or `None` to draw them whole.
    pub section_mm: Option<Scalar>,
    pub lines: &'a [LineVertex],
    pub models: &'a [ModelDraw],
    /// The solids the cut is capped against, counted into the stencil plane.
    pub solids: &'a [ModelDraw],
    /// The bodies cut out of each object, and the object they are cut into: what the
    /// inside of a hole is drawn from. See ADR 0201.
    pub cut_surfaces: &'a [CutDraw],
    /// The quad lying in the cutting plane the cap is painted with.
    pub cap: &'a [LineVertex],
    /// The word lying in front of the plate, and the font atlas its triangles sample. The atlas
    /// goes to the card the first time it is seen and is kept by its address after that.
    pub label: &'a [LabelVertex],
    pub atlas: Option<Arc<egui::ColorImage>>,
    /// The holes and channels already cut into the models being drawn, subtracted per
    /// fragment because the meshes themselves are never cut; see ADR 0071, 0073.
    pub cuts: &'a [DrainCut],
    /// The pockets of resin the last drainage check found, which is where the cavity is
    /// painted red and nowhere else; see ADR 0200.
    pub pockets: &'a [TrapBox],
    /// The models drawn with their own texture on them rather than flat, which is what
    /// the Relief tool shows; see ADR 0116.
    pub reliefs: &'a [ReliefDraw],
    /// The exposure bands washed over the models, and the height below which one has no
    /// effect because the bottom block keeps the resin's ramp; see ADR 0090.
    pub bands: &'a [ExposureBand],
    pub band_floor_mm: f32,
    /// The build volume in plate millimetres, or `None` to mark nothing standing past it.
    pub volume_mm: Option<Vec3>,
    /// Where the Cut tool's plane meets the model it is set on, traced over its surface.
    pub cut_line: Option<CutLine>,
    /// Whether the models are drawn seen through, which is what shows a cavity that holds
    /// resin; see `docs/design/viewport.md`.
    pub xray: bool,
    /// The plate the models stand and cast their contact shadow on, or `None` for none.
    pub floor: Option<Floor>,
    /// Whether the plate's grid is drawn into it.
    pub grid: bool,
}

/// The rectangle the contact shadow is laid on, plate millimetres, at the plate's height.
#[derive(Debug, Clone, Copy)]
pub struct Floor {
    pub near_corner_mm: Vec2,
    pub size_mm: Vec2,
}

impl Floor {
    /// The corner and the size in one `vec4`, as the shader reads them.
    pub(super) fn area(self) -> [f32; 4] {
        [
            self.near_corner_mm.x,
            self.near_corner_mm.y,
            self.size_mm.x,
            self.size_mm.y,
        ]
    }
}

/// The Cut tool's plane, traced as a line wherever it crosses the surface inside `bounds`.
#[derive(Debug, Clone, Copy)]
pub struct CutLine {
    pub normal: Vec3,
    /// How far along `normal` the plane stands from the origin, millimetres.
    pub offset_mm: Scalar,
    /// The model being cut, plate millimetres, so no other model is traced.
    pub bounds: Aabb,
}

/// One object to draw: which cached mesh, which of its faces, and which instance slot
/// holds its placement.
pub struct ModelDraw {
    pub mesh: Arc<Mesh>,
    pub instance: ModelInstance,
    pub(super) faces: Range<usize>,
    /// A range some later frame may ask for on its own. Declared here so the pieces break
    /// at its ends from the first upload: a shell reaches the card before the drainage
    /// check says whether its cavity is to be painted, and re-cutting a mesh of millions
    /// of triangles for that answer is a stall the user would see.
    breaks: Option<Range<usize>>,
}

impl ModelDraw {
    /// The whole mesh.
    pub fn whole(mesh: Arc<Mesh>, instance: ModelInstance) -> Self {
        let faces = 0..mesh.faces.len();
        Self {
            mesh,
            instance,
            faces,
            breaks: None,
        }
    }

    /// The whole mesh, with `breaks` kept as a range a later frame may draw on its own.
    pub fn whole_around(mesh: Arc<Mesh>, breaks: Range<usize>, instance: ModelInstance) -> Self {
        Self {
            breaks: Some(breaks),
            ..Self::whole(mesh, instance)
        }
    }

    /// Which faces of the mesh this draw covers.
    #[cfg(test)]
    pub fn faces(&self) -> Range<usize> {
        self.faces.clone()
    }

    /// Only `faces` of the mesh, which is how the cavity inside a shell is painted on its
    /// own without a second copy of it on the card.
    pub fn part(mesh: Arc<Mesh>, faces: Range<usize>, instance: ModelInstance) -> Self {
        Self {
            mesh,
            instance,
            faces,
            breaks: None,
        }
    }
}

/// One object's cuts and the object itself: the bodies are drawn where this solid says
/// there is material in front of them.
pub struct CutDraw {
    pub body: ModelDraw,
    pub solid: ModelDraw,
}

/// One object drawn with the texture a relief would be pressed from, instead of flat.
pub struct ReliefDraw {
    pub mesh: Arc<Mesh>,
    /// The map and its images, which are also what the cache is keyed by: pressing a
    /// relief replaces both the mesh and the map.
    pub mapped: Arc<Mapped>,
    pub instance: ModelInstance,
}

/// Where every mesh of a frame has to break, by the address its `Arc` holds.
pub(super) type Splits = HashMap<usize, Vec<usize>>;

/// Every cut surface of a frame as a draw of its own: the body cut out of an object, then
/// the object it was cut into.
pub(super) fn cut_draws<'a>(frame: &'a FrameInput<'a>) -> impl Iterator<Item = &'a ModelDraw> {
    frame
        .cut_surfaces
        .iter()
        .flat_map(|cut| [&cut.body, &cut.solid])
}

/// Where the pieces of each mesh have to break, by cache key: the ends of every draw that
/// asks for part of it, and of every range a draw of the whole mesh declares for later.
pub(super) fn splits_of<'a>(draws: impl Iterator<Item = &'a ModelDraw>) -> Splits {
    let mut splits: Splits = HashMap::new();
    for draw in draws {
        let at = splits.entry(Arc::as_ptr(&draw.mesh) as usize).or_default();
        at.push(draw.faces.start);
        at.push(draw.faces.end);
        at.extend(draw.breaks.iter().flat_map(|at| [at.start, at.end]));
    }
    // Sorted and deduplicated, so that two frames asking for the same breaks in a
    // different order agree and the mesh is not uploaded again for nothing.
    for at in splits.values_mut() {
        at.sort_unstable();
        at.dedup();
    }
    splits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::vertex::NOT_MARKED;
    use crate::ui::theme;
    use core_geometry::Transform;

    fn instance() -> ModelInstance {
        ModelInstance::new(Transform::default(), theme::scene().object, NOT_MARKED)
    }

    /// A hollowed shell reaches the card long before the drainage check says whether the
    /// cavity in it is to be painted. It declares that range from the start, so the frame
    /// which does paint it asks for no break the frames before it did not, and a mesh of
    /// millions of triangles is never cut and uploaded a second time.
    #[test]
    fn a_shell_declares_the_cavity_in_it_before_any_frame_paints_it() {
        let mesh = Arc::new(Mesh::new(vec![Vec3::ZERO; 3], vec![[0, 1, 2]; 8]));
        let key = Arc::as_ptr(&mesh) as usize;
        let shell = ModelDraw::whole_around(Arc::clone(&mesh), 3..6, instance());
        let cavity = ModelDraw::part(Arc::clone(&mesh), 3..6, instance());

        let before = splits_of([&shell].into_iter());
        let after = splits_of([&shell, &cavity].into_iter());
        assert_eq!(
            before[&key],
            vec![0, 3, 6, 8],
            "the ends of the cavity and of the mesh"
        );
        assert_eq!(
            before[&key], after[&key],
            "painting the cavity asks for no break the frame before it did not"
        );
    }

    /// Without that, the breaks change under a cache keyed by the mesh alone, which is why
    /// `cache` compares them rather than taking whatever it already holds.
    #[test]
    fn a_shell_that_never_declared_its_cavity_asks_for_a_new_break() {
        let mesh = Arc::new(Mesh::new(vec![Vec3::ZERO; 3], vec![[0, 1, 2]; 8]));
        let key = Arc::as_ptr(&mesh) as usize;
        let shell = ModelDraw::whole(Arc::clone(&mesh), instance());
        let cavity = ModelDraw::part(Arc::clone(&mesh), 3..6, instance());

        assert_ne!(
            splits_of([&shell].into_iter())[&key],
            splits_of([&shell, &cavity].into_iter())[&key]
        );
    }
}
