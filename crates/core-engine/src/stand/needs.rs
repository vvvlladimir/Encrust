use core_geometry::{Bvh, Mesh, Scalar, Vec3};
use core_slicer::Sliced;
use core_supports::{Blocked, Need, Shown, WallSide, generate_supports};

use printer_profiles::{AUTO_SIZES, AutoProfile, SupportProfile};

use super::{ByHand, SupportPlan, SupportShapes};
use crate::error::EngineError;
use crate::strengthen::{Underside, strengthen};

/// What finding where a model needs holding up is asked of: its cut stack and surface, in
/// plate coordinates, and what keeps placement off parts of it.
pub struct NeedsAsked<'a> {
    pub stack: &'a Sliced,
    pub surface: &'a Mesh,
    pub bvh: &'a Bvh,
    pub layer_height_mm: Scalar,
    pub plan: &'a SupportPlan,
    pub lows: &'a [Vec3],
    pub painted: Option<Blocked>,
    pub shown: Option<&'a Shown>,
    /// Contacts of supports already standing, plate mm: they hold up what is near them.
    pub seeds: &'a [Vec3],
    /// The side of a hollow model's wall held up, or `None` for a solid one.
    pub side: Option<WallSide>,
}

impl NeedsAsked<'_> {
    pub(super) fn blocked(&mut self) -> Option<Blocked> {
        Blocked::keeping(self.painted.take(), self.side.clone())
    }
}

/// Where the model needs holding up on `asked.side`, strengthened with `held` standing
/// beside them; `tick(layer)` says whether to go on.
pub fn needs_on(
    mut asked: NeedsAsked<'_>,
    held: &[Vec3],
    tick: &mut dyn FnMut(usize) -> bool,
) -> Result<Vec<Need>, EngineError> {
    let blocked = asked.blocked();
    let mut needs = placed_on(&asked, blocked.as_ref(), tick);
    strengthened_on(&asked, blocked.as_ref(), &mut needs, held, &mut || tick(0))?;
    Ok(needs)
}

/// Where the model needs holding up on its outside and, with `inside`, on its cavity, each side
/// as `sides` asks it and held against the other's tips.
pub fn needs_on_both<'a>(
    sides: &dyn Fn(bool) -> NeedsAsked<'a>,
    inside: bool,
    held: &[Vec3],
    tick: &mut dyn FnMut(usize) -> bool,
) -> Result<Vec<Need>, EngineError> {
    both_sides(sides, (inside, true), held, tick)
}

/// [`needs_on_both`], the strengthening optional.
pub(super) fn both_sides<'a>(
    sides: &dyn Fn(bool) -> NeedsAsked<'a>,
    (inside, strengthening): (bool, bool),
    held: &[Vec3],
    tick: &mut dyn FnMut(usize) -> bool,
) -> Result<Vec<Need>, EngineError> {
    let place = |side: bool, tick: &mut dyn FnMut(usize) -> bool| {
        let mut asked = sides(side);
        let blocked = asked.blocked();
        placed_on(&asked, blocked.as_ref(), tick)
    };
    let mut needs = place(false, tick);
    let mut cavity = if inside {
        place(true, tick)
    } else {
        Vec::new()
    };
    if strengthening {
        let contacts = |needs: &[Need]| needs.iter().map(|need| need.contact).collect::<Vec<_>>();
        let mut strengthen = |side: bool, needs: &mut Vec<Need>, others: &[Vec3]| {
            let mut asked = sides(side);
            let blocked = asked.blocked();
            let held: Vec<Vec3> = held.iter().chain(others).copied().collect();
            strengthened_on(&asked, blocked.as_ref(), needs, &held, &mut || tick(0))
        };
        strengthen(false, &mut needs, &contacts(&cavity))?;
        if !cavity.is_empty() {
            strengthen(true, &mut cavity, &contacts(&needs))?;
        }
    }
    needs.extend(cavity);
    Ok(needs)
}

/// What automatic placement touches, kept to the asked side of the wall.
pub(super) fn placed_on(
    asked: &NeedsAsked<'_>,
    blocked: Option<&Blocked>,
    tick: &mut dyn FnMut(usize) -> bool,
) -> Vec<Need> {
    let seeds: Vec<Vec3> = match &asked.side {
        Some(side) => asked
            .seeds
            .iter()
            .copied()
            .filter(|seed| side.keeps(*seed))
            .collect(),
        None => asked.seeds.to_vec(),
    };
    let mut needs = generate_supports(
        asked.stack,
        asked.layer_height_mm,
        &asked.plan.placing(),
        asked.plan.sizing(),
        &seeds,
        asked.lows,
        blocked,
        asked.shown,
        tick,
    );
    if let Some(side) = &asked.side {
        needs.retain(|need| side.keeps(need.contact));
    }
    needs
}

/// Strengthens `needs`, placed on the asked side, with `held` standing beside them.
pub(super) fn strengthened_on(
    asked: &NeedsAsked<'_>,
    blocked: Option<&Blocked>,
    needs: &mut Vec<Need>,
    held: &[Vec3],
    keep_going: &mut dyn FnMut() -> bool,
) -> Result<(), EngineError> {
    let underside = Underside {
        stack: asked.stack,
        layer_mm: asked.layer_height_mm,
        surface: asked.surface,
        bvh: asked.bvh,
        blocked,
    };
    strengthen(underside, needs, held, asked.plan, keep_going).map(|_| ())
}

/// What a model's supports are built to: the shapes `sizing` names, or `profile` alone.
pub fn plan_of(
    profile: &SupportProfile,
    sizing: Option<(&AutoProfile, &[SupportProfile; AUTO_SIZES + 1])>,
) -> SupportPlan {
    match sizing {
        Some((auto, shapes)) => SupportPlan {
            shapes: SupportShapes::Sized {
                auto: Box::new(auto.clone()),
                shapes: Box::new(shapes.clone()),
            },
            cavity: false,
            merges: Vec::new(),
            by_hand: ByHand::default(),
            shown_at: Vec::new(),
        },
        None => SupportPlan::of(profile.clone()),
    }
}
