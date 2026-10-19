use core_csg::Solid;
use core_geometry::{Cut, Mesh, Plane, Scalar};

use crate::error::JointError;
use crate::key::key;
use crate::pegs::peg;
use crate::settings::{JointKind, Joints};

/// A model cut in two, with what `join` needs to read its material on either side.
#[derive(Debug)]
pub struct Halves<'a> {
    /// The halves of what prints, closed over the cut.
    pub cut: Cut,
    pub plane: Plane,
    /// What prints, uncut, in the same space as the halves.
    pub printed: &'a Mesh,
    /// The model's outside alone, uncut, where what prints is a hollowed shell of it: a
    /// socket over the cavity then stands on a pillar of it.
    pub solid: Option<&'a Mesh>,
}

/// The halves joined: the lower carrying the pins or tongue, both their sockets.
#[derive(Debug, Clone, Default)]
pub struct Joined {
    pub below: Mesh,
    pub above: Mesh,
    /// Dowels to print beside the halves.
    pub loose: Vec<Dowel>,
    /// Pins, dowels or magnets put in, or pieces of the face a key runs round.
    pub placed: usize,
    /// Separate pieces of the cut face too small or too thin through to take any.
    pub bare: usize,
}

/// A loose dowel standing on the plate at the origin, round the z axis.
#[derive(Debug, Clone)]
pub struct Dowel {
    pub mesh: Mesh,
    pub radius_mm: Scalar,
}

/// What one half gains and loses, in the order it is done: pillars cut to the model's
/// outside, then pegs, then sockets.
#[derive(Debug, Default)]
pub(crate) struct Work {
    pub pillars: Mesh,
    pub pegs: Mesh,
    pub sockets: Mesh,
}

/// The work on both halves, the dowels to print beside them, and how many fixings went in
/// and how many pieces of the face took none.
#[derive(Debug, Default)]
pub(crate) struct Planned {
    pub below: Work,
    pub above: Work,
    pub loose: Vec<Dowel>,
    pub placed: usize,
    pub bare: usize,
}

/// Joins the two halves of a cut as `joints` asks, so they go back together true. A plain
/// cut, or a face with no room for any fixing, hands the halves back as they were; a size
/// that cannot be built is [`JointError::BadSize`].
///
/// See `docs/design/joints.md` for where fixings go and how big they are.
pub fn join(halves: Halves<'_>, joints: &Joints) -> Result<Joined, JointError> {
    joints.check()?;
    let planned = match joints.kind {
        None => Planned::default(),
        Some(JointKind::Key) => key(&halves, joints.key),
        Some(kind) => peg(&halves, kind, joints.pegs, joints.magnets),
    };
    let needs_solid =
        !planned.below.pillars.faces.is_empty() || !planned.above.pillars.faces.is_empty();
    let outside = match (needs_solid, halves.solid) {
        (true, Some(solid)) => Some(Solid::new(solid)?),
        _ => None,
    };
    Ok(Joined {
        below: worked(halves.cut.below, &planned.below, outside.as_ref())?,
        above: worked(halves.cut.above, &planned.above, outside.as_ref())?,
        loose: planned.loose,
        placed: planned.placed,
        bare: planned.bare,
    })
}

/// A model cut in two and its halves joined, as [`cut_and_join`] leaves them.
#[derive(Debug, Clone)]
pub struct CutJoined {
    pub joined: Joined,
    /// Cut loops that did not close, and were left uncapped.
    pub open_loops: usize,
    /// Why the halves were left plain when they were to be joined.
    pub unjoined: Option<JointError>,
}

/// `printed` cut by `plane` and its halves joined as `joints` asks, or `None` where the
/// plane misses it. `solid` is the model's outside where `printed` is a hollowed shell of it.
/// A joint that cannot be built leaves the halves plain, with the reason.
pub fn cut_and_join(
    printed: &Mesh,
    solid: Option<&Mesh>,
    plane: Plane,
    joints: Option<&Joints>,
) -> Option<CutJoined> {
    let halves = core_geometry::cut(printed, plane);
    if halves.below.is_empty() || halves.above.is_empty() {
        return None;
    }
    let open_loops = halves.open_loops;
    let plain = |halves: Cut| Joined {
        below: halves.below,
        above: halves.above,
        ..Joined::default()
    };
    let Some(joints) = joints.filter(|joints| joints.kind.is_some()) else {
        return Some(CutJoined {
            joined: plain(halves),
            open_loops,
            unjoined: None,
        });
    };
    let joining = Halves {
        cut: halves.clone(),
        plane,
        printed,
        solid,
    };
    let (joined, unjoined) = match join(joining, joints) {
        Ok(joined) => (joined, None),
        Err(error) => (plain(halves), Some(error)),
    };
    Some(CutJoined {
        joined,
        open_loops,
        unjoined,
    })
}

/// `half` with `work` done to it, left as it is where there is none.
fn worked(half: Mesh, work: &Work, outside: Option<&Solid>) -> Result<Mesh, JointError> {
    let idle = [&work.pillars, &work.pegs, &work.sockets]
        .iter()
        .all(|mesh| mesh.faces.is_empty());
    if idle {
        return Ok(half);
    }
    let mut solid = Solid::new(&half)?;
    if let Some(outside) = outside.filter(|_| !work.pillars.faces.is_empty()) {
        solid = solid.union(&Solid::new(&work.pillars)?.intersection(outside)?)?;
    }
    if !work.pegs.faces.is_empty() {
        solid = solid.union(&Solid::new(&work.pegs)?)?;
    }
    if !work.sockets.faces.is_empty() {
        solid = solid.difference(&Solid::new(&work.sockets)?)?;
    }
    Ok(solid.mesh())
}
