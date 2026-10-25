use core_engine::project::{AutoSizing, FillProfiles};
use core_geometry::{Scalar, Vec2, Vec3};
use core_supports::{Part, ProjectSettings, SupportTree};
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, Raft, SupportProfile};
use serde::{Deserialize, Serialize};

mod fill;
mod groups;
mod inside;
mod run;
pub use groups::{own_raft_shown, with_own};

/// How many shapes an automatic run builds to: its sizes, and the one that steadies a part.
const SHAPES: usize = AUTO_SIZES + 1;

/// How near a support has to stand to where the last one was placed to start a line from
/// it, millimetres: both are the same point put through the same matrix.
const LINE_START_MM: Scalar = 1e-3;

use crate::i18n::tr;
use crate::job::SupportJob;
use crate::scene::{ObjectId, Scene, SceneObject};

/// What the one group a plate starts with is called until a profile is chosen for it.
const FIRST_GROUP: &str = "Default";

/// The group the Check tool stands island supports in, apart from the ones built by hand.
pub const ISLANDS_GROUP: &str = "Islands";

/// Whether a tree a hand has frozen stands inside the model's cavity: every tip of it does.
fn frozen_inside(object: &SceneObject, tree: &SupportTree) -> bool {
    let mut contacts = tree.contacts().peekable();
    contacts.peek().is_some() && contacts.all(|contact| object.holds_inside(contact))
}

/// Which side of a hollow model's wall an automatic run holds up: everything outside it,
/// or only its cavity; see `docs/decisions/0332`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Outside,
    Inside,
}

/// What a click on the model does with the Paint tool: stand a support, paint a fill, paint
/// where supports keep out, or take paint off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placing {
    #[default]
    Point,
    Paint,
    Block,
    Erase,
}

impl Placing {
    pub fn label(self) -> String {
        match self {
            Self::Point => tr!("supports-placing-place"),
            Self::Paint => tr!("supports-placing-fill"),
            Self::Block => tr!("supports-placing-block"),
            Self::Erase => tr!("supports-placing-erase"),
        }
    }

    /// Whether this works the brush over a patch rather than on a single support.
    pub fn paints(self) -> bool {
        matches!(self, Self::Paint | Self::Block | Self::Erase)
    }

    /// Whether what it paints is the patch supports keep out of.
    pub fn blocks(self) -> bool {
        self == Self::Block
    }
}

/// A named set of supports built to one profile. A support carries the number of its
/// group, so retuning a group retunes every support in it; see `docs/decisions/0094`.
#[derive(Debug, Clone)]
pub struct SupportGroup {
    pub name: String,
    pub profile: SupportProfile,
    /// Left out of the view, though not out of the print: a way to see past one kind of
    /// support to another.
    pub hidden: bool,
}

/// One piece of one frozen support: what the Edit mode selects, shows picked and carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picked {
    pub id: crate::scene::ObjectId,
    /// Which of the object's frozen trees, which is stable until it is taken away.
    pub frozen: usize,
    pub part: Part,
}

/// What the Supports tool places with, and the automatic run it has going.
#[derive(Debug)]
pub struct SupportTool {
    /// The active group's numbers, which the panel's fields edit in place. They are
    /// written back into `groups` whenever another group is chosen.
    pub profile: SupportProfile,
    /// Every group on the plate, the first of which everything falls back to.
    pub groups: Vec<SupportGroup>,
    /// Which group new supports join, and whose numbers `profile` is holding.
    pub active: u16,
    /// The parts the Edit mode is working on. Several at once: a drag carries all of
    /// them by the same step.
    pub picked: Vec<Picked>,
    /// Where the part under the cursor stood when the drag began, in plate coordinates,
    /// so that every step is measured from the same place.
    pub anchor: Option<Vec3>,
    pub placing: Placing,
    /// Radius of the brush, millimetres of plate.
    pub brush_radius_mm: Scalar,
    /// How far a surface may turn from the face that was clicked and still be painted
    /// with it, degrees.
    pub flood_angle_deg: Scalar,
    pub fill: ProjectSettings,
    /// The saved profiles a fill builds its rim and its inside to, or the group in hand.
    pub fill_profiles: FillProfiles,
    /// Which side of a hollow model's wall the run going holds up.
    side: Side,
    /// How far apart a line of supports drawn with shift stands, millimetres.
    pub inline_spacing_mm: Scalar,
    /// The support Place last put down, in its model's own space: where a line drawn with
    /// shift starts from. See `docs/decisions/0330`.
    pub last_placed: Option<(ObjectId, Vec3)>,
    /// The brush has hold of the pointer: the press that started this drag landed on a
    /// model, so the stroke paints instead of turning the camera.
    pub stroke: bool,
    /// Where the brush was last put down, in panel points, so that a fast drag is painted
    /// as the line it drew rather than as the frames it was sampled on.
    pub last_daub: Option<Vec2>,
    pub job: Option<SupportJob>,
    /// The automatic profile the panel's list has chosen in place of a support profile,
    /// or `None` while a run places to the group in hand. See `docs/decisions/0325`.
    pub auto: Option<AutoProfile>,
    /// The automatic profile set aside while supports are built to the group in hand, so
    /// that sizing turned back on finds its numbers as they were left.
    parked_auto: Option<AutoProfile>,
    /// What the run going sizes its supports by, and the shapes it sizes them into.
    sizing: Option<(AutoProfile, [SupportProfile; SHAPES])>,
    /// The raft every model stands on that has none of its own. A raft is one slab under
    /// a part, so it is the part's and the plate's, not any one group's; see ADR 0253.
    pub raft: Raft,
    /// A profile the panel has just saved, until the plate has taken it on.
    pub saved: Option<SupportProfile>,
    /// A profile the panel's list has just had chosen, until a group has taken it up.
    pub chosen: Option<SupportProfile>,
}

impl Default for SupportTool {
    fn default() -> Self {
        Self {
            profile: SupportProfile::default(),
            groups: vec![SupportGroup {
                name: FIRST_GROUP.to_owned(),
                profile: SupportProfile::default(),
                hidden: false,
            }],
            active: 0,
            picked: Vec::new(),
            anchor: None,
            placing: Placing::default(),
            brush_radius_mm: 2.0,
            flood_angle_deg: 30.0,
            fill: ProjectSettings::default(),
            fill_profiles: FillProfiles::default(),
            side: Side::Outside,
            inline_spacing_mm: core_supports::INLINE_SPACING_MM,
            last_placed: None,
            stroke: false,
            last_daub: None,
            job: None,
            auto: None,
            parked_auto: None,
            sizing: None,
            raft: SupportProfile::default().raft,
            saved: None,
            chosen: None,
        }
    }
}

impl SupportTool {
    /// Why the Generate button is greyed out, or `None` when it is not.
    pub fn blocker(&self, scene: &Scene) -> Option<String> {
        if scene.target_count() == 0 {
            return Some(tr!("supports-nothing-visible"));
        }
        None
    }

    /// Where a line of supports drawn on object `id` starts, in plate coordinates: the last
    /// support placed, if it was placed on that model and still stands there. One undone,
    /// taken away or left on a scene since replaced is no start.
    pub fn line_start(&self, scene: &Scene, id: ObjectId) -> Option<Vec3> {
        let (on, contact) = self.last_placed?;
        let object = scene.get(id).filter(|_| on == id)?;
        let supports = &object.supports;
        let mut standing = supports
            .points()
            .iter()
            .map(|point| point.contact)
            .chain(supports.frozen().iter().flat_map(SupportTree::contacts));
        standing
            .any(|at| at.distance_squared(contact) < LINE_START_MM * LINE_START_MM)
            .then(|| object.transform.to_matrix().transform_point3(contact))
    }

    /// Starts a plate nothing was ever chosen for and nothing stands on with the automatic
    /// profile; done where the undo stack starts, as that is not an edit.
    pub fn start_automatic_unless_chosen(&mut self, scene: &Scene, catalogue: &Catalogue) {
        let supported = scene
            .objects()
            .iter()
            .any(|object| !object.supports.is_empty());
        if self.sizing().is_none() && !supported {
            self.set_automatic(true, catalogue);
        }
    }

    /// Sizes supports by what they hold, or builds them to the group in hand. Turned on,
    /// it takes up the automatic profile last set aside, else the first `catalogue` ships.
    pub fn set_automatic(&mut self, on: bool, catalogue: &Catalogue) {
        if on == self.auto.is_some() {
            return;
        }
        if on {
            self.auto = self
                .parked_auto
                .take()
                .or_else(|| catalogue.autos().next().map(|entry| entry.profile.clone()));
        } else {
            self.parked_auto = self.auto.take();
        }
    }

    /// The automatic profile in hand or set aside, as a project and the undo stack keep it.
    pub fn sizing(&self) -> Option<AutoSizing> {
        let (on, profile) = match (&self.auto, &self.parked_auto) {
            (Some(profile), _) => (true, profile),
            (None, Some(profile)) => (false, profile),
            (None, None) => return None,
        };
        Some(AutoSizing {
            on,
            profile: profile.clone(),
        })
    }

    /// Takes up automatic sizing as `sizing` left it, or none chosen at all.
    pub fn set_sizing(&mut self, sizing: Option<AutoSizing>) {
        (self.auto, self.parked_auto) = match sizing {
            Some(AutoSizing { on: true, profile }) => (Some(profile), None),
            Some(AutoSizing { on: false, profile }) => (None, Some(profile)),
            None => (None, None),
        };
    }
}

#[cfg(test)]
mod tests;
