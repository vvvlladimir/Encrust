use core_engine::ShownAt;
use core_engine::project::{Axis, HollowState, Keep, ObjectId};
use core_geometry::{Scalar, Vec3};
use core_joint::Joints;
use core_volume::{
    Boolean, CHANNEL_DIAMETER_MM, HoleKind, HoleSize, PLUG_GAP_MM, ReliefSettings, RepairMethod,
    Tiling,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The supports profile that sizes each support by what it holds, as `--supports auto`.
pub const AUTO: &str = "auto";

/// One action on a plate, naming the object it acts on; points and positions are plate
/// millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "kebab-case")]
pub enum Edit {
    /// Cuts `object` across `axis` at `at_mm`, keeping `keep` and joining both halves by
    /// `joints`; with `lay_flat` each half lies on its cut face.
    Cut {
        object: ObjectId,
        axis: Axis,
        at_mm: Scalar,
        #[serde(default)]
        keep: Keep,
        #[serde(default)]
        joints: Joints,
        #[serde(default)]
        lay_flat: bool,
    },
    /// Moves `object` by `by_mm`.
    Move { object: ObjectId, by_mm: Vec3 },
    /// Sets the turn of `object` to `degrees` about X, then Y, then Z of the plate, about its
    /// centre of mass, and stands it on the plate.
    Rotate { object: ObjectId, degrees: Vec3 },
    /// Sets the scale of `object` to `factors` about its centre of mass and stands it on the
    /// plate.
    Scale { object: ObjectId, factors: Vec3 },
    /// Takes `object` off the plate, and the plugs laid for it.
    Delete { object: ObjectId },
    /// Puts a copy of `object`, with its supports, cavity and settings, `offset_mm` from it,
    /// right after it.
    Duplicate { object: ObjectId, offset_mm: Vec3 },
    /// Turns `object` inside out across `axis` of its own frame, where its bounds' centre
    /// stays.
    Mirror { object: ObjectId, axis: Axis },
    /// Breaks `object` into the bodies it is made of; a body under `smallest_mm3` (the
    /// default where unset) is joined to the one it touches, or dropped.
    Split {
        object: ObjectId,
        #[serde(default)]
        smallest_mm3: Option<Scalar>,
    },
    /// Cuts the part of `object` at plate point `at_mm` off where it joins the rest.
    PartOff { object: ObjectId, at_mm: Vec3 },
    /// Replaces `objects` by the one `op` makes of them, the first being what a difference
    /// keeps; exact where every object is a solid, sampled where one is not.
    Combine {
        objects: Vec<ObjectId>,
        op: Boolean,
        /// Set, the cut is exact and takes only the side of the first the others stand on it
        /// from.
        #[serde(default)]
        toward: Option<Vec3>,
    },
    /// Writes `text` on `object` at the surface nearest plate point `at_mm`, raised out of it
    /// or sunk into it; an unset size is the window's Text tool's.
    Label {
        object: ObjectId,
        text: String,
        at_mm: Vec3,
        /// How tall the capitals stand, mm.
        #[serde(default)]
        height_mm: Option<Scalar>,
        /// How far the letters stand out of the surface or, unless `raised`, into it, mm.
        #[serde(default)]
        depth_mm: Option<Scalar>,
        #[serde(default = "on_by_default")]
        raised: bool,
        /// Each letter bends onto the surface under it.
        #[serde(default)]
        follow_surface: bool,
        /// The line's turn about the normal, anticlockwise, degrees.
        #[serde(default)]
        angle_deg: Scalar,
        /// A font file; the bundled IBM Plex Sans where unset.
        #[serde(default)]
        font: Option<PathBuf>,
    },
    /// Repairs `object`'s mesh by `method`, to the lengths the window's Repair tool starts
    /// with; what was built on its old surface goes.
    Repair {
        object: ObjectId,
        method: RepairMethod,
        /// Nearer than this, mm, a seam's two edges are one; the window's where unset.
        #[serde(default)]
        seam_tolerance_mm: Option<Scalar>,
        /// Smaller than this, mm², a shell enclosing nothing goes; the window's where unset.
        #[serde(default)]
        smallest_shell_mm2: Option<Scalar>,
    },
    /// Turns, bridges and drops what is named on `object`'s mesh: faces from 0, shells from 1,
    /// a bridge as two open edges of two vertices each; drops come last.
    Mend {
        object: ObjectId,
        #[serde(default)]
        flip_faces: Vec<usize>,
        #[serde(default)]
        drop_faces: Vec<usize>,
        #[serde(default)]
        drop_shells: Vec<usize>,
        #[serde(default)]
        bridge: Vec<[[u32; 2]; 2]>,
    },
    /// Presses the picture at `image`, tiled `tile_mm` wide and turned `angle_deg`, into
    /// `object` `depth_mm` deep, as `--relief` does; only its painted faces move where any are.
    Relief {
        object: ObjectId,
        depth_mm: Scalar,
        image: PathBuf,
        #[serde(default = "tile_mm_by_default")]
        tile_mm: Scalar,
        #[serde(default)]
        angle_deg: Scalar,
        #[serde(default)]
        invert: bool,
        #[serde(default)]
        symmetric: bool,
        /// How far the picture is blurred first, px.
        #[serde(default)]
        smoothing_px: Scalar,
        /// Faces within this of facing down stay unmoved, degrees.
        #[serde(default)]
        hold_bottom_deg: Scalar,
        /// Faces within this of facing up stay unmoved, degrees.
        #[serde(default)]
        hold_top_deg: Scalar,
        /// The lattice's precision, 0 to 1.
        #[serde(default = "precision_by_default")]
        precision: Scalar,
    },
    /// Hollows `object` by `settings`, which it keeps as its own, the wall kept solid along
    /// each of `keep_solid` as the window's Keep solid clicks lay it; its supports stay.
    Hollow {
        object: ObjectId,
        settings: HollowState,
        /// Each a run of plate points: a blocker at the surface nearest the first, rods on.
        #[serde(default)]
        keep_solid: Vec<Vec<Vec3>>,
        /// How far each blocker reaches past the wall, mm; fitted to the model where unset.
        #[serde(default)]
        keep_solid_reach_mm: Option<Scalar>,
    },
    /// Drills holes of `size` and `kind` (`AtDepth` where unset) nearest each of `at_mm`, as
    /// `--drain-at` does; `auto` adds those that drain every pocket, `vent` a vent on each.
    Drain {
        object: ObjectId,
        #[serde(default)]
        at_mm: Vec<Vec3>,
        #[serde(default)]
        size: HoleSize,
        #[serde(default = "at_depth")]
        kind: HoleKind,
        #[serde(default)]
        auto: bool,
        #[serde(default = "on_by_default")]
        vent: bool,
    },
    /// Digs a drainage channel of `diameter_mm` along `points_mm` through `object`, as
    /// `--channel` does.
    Channel {
        object: ObjectId,
        points_mm: Vec<Vec3>,
        #[serde(default = "channel_diameter_mm")]
        diameter_mm: Scalar,
    },
    /// Lays a plug for each drain hole of `object` as new objects, at the nearest free spot of
    /// the usable area `plate_mm` across from `low_mm` where set, else in a row past it.
    Plugs {
        object: ObjectId,
        /// How much narrower a side each plug is than its hole: the final gap, mm.
        #[serde(default = "plug_gap_mm")]
        gap_mm: Scalar,
        #[serde(default)]
        plate_mm: Option<[Scalar; 2]>,
        #[serde(default)]
        low_mm: [Scalar; 2],
    },
    /// Spreads every printable object of the active plate, with its supports, `clearance_mm`
    /// apart over the usable area, turning one no slide fits as the window does.
    Arrange {
        /// The usable area's corner: the plate less its inset, mm.
        #[serde(default)]
        low_mm: [Scalar; 2],
        /// The usable area's size from `low_mm`, mm.
        plate_mm: [Scalar; 2],
        clearance_mm: Scalar,
    },
    /// Stands `object` the way it prints best for `goal`, the `pick`th best from 0, on the
    /// usable area `plate_mm` across from `low_mm` as `Arrange` takes them.
    Orient {
        object: ObjectId,
        #[serde(default)]
        goal: OrientAim,
        #[serde(default)]
        max_tilt_deg: Option<Scalar>,
        /// Keep to the fewest supports where the model shows.
        #[serde(default)]
        clean_surface: bool,
        #[serde(default)]
        pick: usize,
        #[serde(default)]
        plate_mm: Option<[Scalar; 2]>,
        #[serde(default)]
        low_mm: [Scalar; 2],
        /// The catalogue id of the printer the poses are proved on; the standard one where unset.
        #[serde(default)]
        printer: Option<String>,
        /// The catalogue id of the resin the poses are proved with; the stock numbers where unset.
        #[serde(default)]
        resin: Option<String>,
        /// Places no support should touch, poses leaving them clear ranked first; plate mm as
        /// the object stands or, with `keep_clear_seen_deg`, as it stood turned so and centred.
        #[serde(default)]
        keep_clear_mm: Vec<Vec3>,
        /// How near a place a contact still touches it, mm; `KEEP_CLEAR_RADIUS_MM` where unset.
        #[serde(default)]
        keep_clear_radius_mm: Option<Scalar>,
        #[serde(default)]
        keep_clear_seen_deg: Option<Vec3>,
        /// How high a pose on supports stands off the plate, mm, as the window's Lift keeps it;
        /// the support profile's where unset.
        #[serde(default)]
        lift_mm: Option<Scalar>,
    },
    /// Stands supports under `object` by a shipped preset (the active group's where unset), with
    /// a contact nearest each of `at_mm`, those alone with `only_at`; see `docs/cli.md`.
    Supports {
        object: ObjectId,
        #[serde(default)]
        profile: Option<String>,
        #[serde(default)]
        at_mm: Vec<Vec3>,
        #[serde(default)]
        only_at: bool,
        #[serde(default)]
        cavity: bool,
        #[serde(default)]
        shown_at: Vec<ShownAt>,
        /// Reads the side painted shown in the file, as the window's Clean surface does.
        #[serde(default)]
        clean_surface: bool,
    },
    /// Joins the supports of `object` whose contacts lie nearest `near_mm`, two or more, plate
    /// mm, into one, as the window's Merge into one joins those picked (ADR 0440).
    MergeSupports {
        object: ObjectId,
        near_mm: Vec<Vec3>,
    },
}

impl Edit {
    /// The object the edit acts on, a combination's first; `None` for an arrange of the plate.
    pub fn object(&self) -> Option<ObjectId> {
        match self {
            Self::Combine { objects, .. } => objects.first().copied(),
            Self::Arrange { .. } => None,
            Self::Cut { object, .. }
            | Self::Move { object, .. }
            | Self::Rotate { object, .. }
            | Self::Scale { object, .. }
            | Self::Delete { object }
            | Self::Duplicate { object, .. }
            | Self::Mirror { object, .. }
            | Self::Split { object, .. }
            | Self::PartOff { object, .. }
            | Self::Label { object, .. }
            | Self::Repair { object, .. }
            | Self::Mend { object, .. }
            | Self::Relief { object, .. }
            | Self::Hollow { object, .. }
            | Self::Drain { object, .. }
            | Self::Channel { object, .. }
            | Self::Plugs { object, .. }
            | Self::Orient { object, .. }
            | Self::Supports { object, .. }
            | Self::MergeSupports { object, .. } => Some(*object),
        }
    }
}

fn tile_mm_by_default() -> Scalar {
    Tiling::default().tile_mm
}

fn precision_by_default() -> Scalar {
    ReliefSettings::default().precision
}

fn on_by_default() -> bool {
    true
}

fn at_depth() -> HoleKind {
    HoleKind::AtDepth
}

fn channel_diameter_mm() -> Scalar {
    CHANNEL_DIAMETER_MM
}

fn plug_gap_mm() -> Scalar {
    PLUG_GAP_MM
}

/// What an `Edit::Orient` stands a model for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrientAim {
    /// Every goal weighed against the others.
    #[default]
    Auto,
    /// The pose that needs the fewest supports.
    FewestSupports,
    /// The smallest cross-section pulled off the film at any layer.
    LeastPeel,
    /// The shortest stack, model and lift.
    ShortestPrint,
}

impl OrientAim {
    /// Every aim, with the word that names it in a file, a flag and a recipe.
    pub const WORDS: [(&'static str, Self); 4] = [
        ("auto", Self::Auto),
        ("fewest-supports", Self::FewestSupports),
        ("least-peel", Self::LeastPeel),
        ("shortest-print", Self::ShortestPrint),
    ];

    /// The words of `WORDS` alone, in the same order.
    pub const NAMES: [&'static str; Self::WORDS.len()] = [
        Self::WORDS[0].0,
        Self::WORDS[1].0,
        Self::WORDS[2].0,
        Self::WORDS[3].0,
    ];

    /// The aim `word` names; any other word is `auto`.
    pub fn named(word: &str) -> Self {
        Self::WORDS
            .iter()
            .find(|(name, _)| *name == word)
            .map_or(Self::Auto, |(_, aim)| *aim)
    }
}

#[cfg(test)]
mod tests;
