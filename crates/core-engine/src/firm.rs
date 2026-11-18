//! Automatic placement checked on the trees it grows into, each support the spring its tree
//! makes, and mended where a part sways, a trunk buckles or a tip tears (ADR 0455).

use core_analysis::{Footing, GREEN_MODULUS_MPA, Load, Spring, Tip};
use core_geometry::{Buckets, Ray, Scalar, Transform, Vec3};
use core_supports::{
    Give, Need, Placed, Profiles, SupportPoint, SupportTree, columns, gives, grow,
};

use crate::error::EngineError;
use crate::stand::SupportPlan;
use crate::strengthen::{
    AROUND, NEIGHBOUR_SPACINGS, TAKEN_SHARE, Underside, WANTED_MARGIN, carried, heavier, neighbour,
    table_radius,
};

/// The most rounds the grown supports are checked and mended in, each a growth and a read.
const MOST_ROUNDS: usize = 3;
/// How far under a swaying layer a contact is looked for, in the placement's spacing.
const SWAY_REACH_SPACINGS: Scalar = 1.0;
/// How long a trunk buckles over, in its free length: fixed at its foot, its top free to sway.
const BUCKLING_LENGTHS: f64 = 2.0;

/// What checking the grown supports changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Firmed {
    /// Rounds that changed anything.
    pub rounds: usize,
    /// Contacts added under a layer that swayed.
    pub steadied: usize,
    /// Tips made heavier, or given a neighbour, where a trunk buckled or a tip tore.
    pub strengthened: usize,
}

/// Grows `needs`, with `held` standing beside them at the placing profile, into the trees
/// `plan` builds, reads each tip on the spring its tree makes, and mends what sways, buckles
/// or tears; stops when `keep_going` says so.
pub fn firm_up(
    model: Underside<'_>,
    needs: &mut Vec<Need>,
    held: &[Vec3],
    plan: &SupportPlan,
    keep_going: &mut dyn FnMut() -> bool,
) -> Result<Firmed, EngineError> {
    let table = plan.table();
    let Some(profiles) = Profiles::new(&table) else {
        return Ok(Firmed::default());
    };
    let placed =
        Placed::new(model.surface, model.bvh, Transform::default()).blocking(model.blocked);
    let reach_mm = plan.placing().spacing_mm();
    let mut taken = Buckets::new(reach_mm * NEIGHBOUR_SPACINGS);
    for at in needs
        .iter()
        .map(|need| need.contact)
        .chain(held.iter().copied())
    {
        taken.insert(at.truncate(), at);
    }
    let mut done = Firmed::default();
    for _ in 0..MOST_ROUNDS {
        if !keep_going() {
            break;
        }
        let tips = sprung_tips(needs, held, plan, (&placed, profiles));
        let Some(loads) = carried(
            model.stack,
            model.surface,
            model.layer_mm,
            &tips,
            keep_going,
        )?
        else {
            break;
        };
        let before = done;
        mend(
            model,
            needs,
            &loads,
            plan,
            (&mut taken, reach_mm),
            &mut done,
        );
        if done == before {
            break;
        }
        done.rounds += 1;
    }
    Ok(done)
}

/// Each of `needs` then `held` as a tip on the spring its tree makes, grown as `plan` builds
/// it; a contact no tree carries stands nothing, so it reads as holding nothing.
fn sprung_tips(
    needs: &[Need],
    held: &[Vec3],
    plan: &SupportPlan,
    (placed, profiles): (&Placed, Profiles),
) -> Vec<Tip> {
    let table = plan.table();
    let points: Vec<SupportPoint> = needs
        .iter()
        .map(|need| SupportPoint {
            group: plan.group_of(need),
            ..SupportPoint::new(need.contact)
        })
        .chain(held.iter().map(|at| SupportPoint::new(*at)))
        .collect();
    let trees = grow(&columns(&points, placed, profiles), placed, profiles);
    let held = held_by(&trees, &points, (placed, profiles));
    points
        .iter()
        .zip(held)
        .map(|(point, held)| Tip {
            at_mm: [point.contact.x, point.contact.y],
            // A contact no tree carries is put out of reach of every layer.
            z_mm: if held.is_some() {
                point.contact.z
            } else {
                Scalar::MAX
            },
            radius_mm: f64::from(table_radius(&table, point.group)),
            spring: held.map(|(spring, _)| spring),
            stands_on: held.and_then(|(_, footing)| footing),
        })
        .collect()
}

/// The spring a tree makes under one of its tips, and where that tree stands on the model.
pub type Held = (Spring, Option<Footing>);

/// For each of `points`, the spring the tree of `trees` carrying it makes and where that
/// tree stands on the model; `None` for a point no tree carries.
pub fn held_by(
    trees: &[SupportTree],
    points: &[SupportPoint],
    (placed, profiles): (&Placed, Profiles),
) -> Vec<Option<Held>> {
    let mut held = vec![None; points.len()];
    for give in gives(trees, points, placed, profiles) {
        if let Some(slot) = held.get_mut(give.point) {
            let footing = give.stands_on.map(|(at, radius)| Footing {
                at_mm: at.to_array(),
                radius_mm: f64::from(radius),
            });
            *slot = Some((spring_of(&give), footing));
        }
    }
    held
}

/// The spring `give` makes in green resin.
pub fn spring_of(give: &Give) -> Spring {
    let stiff = |compliance: f64| GREEN_MODULUS_MPA / compliance.max(f64::MIN_POSITIVE);
    let length = BUCKLING_LENGTHS * give.free_mm;
    Spring {
        axial_n_per_mm: stiff(give.axial_per_mm),
        lateral_n_per_mm: stiff(give.lateral_per_mm),
        turning_n_mm_per_rad: stiff(give.turning_per_mm3),
        trunk: u32::try_from(give.tree).unwrap_or(u32::MAX),
        buckles_at_n: std::f64::consts::PI.powi(2) * GREEN_MODULUS_MPA * give.trunk_mm4
            / (length * length).max(f64::MIN_POSITIVE),
    }
}

/// Adds or thickens what `loads` show weak among `needs`, once per place `taken` leaves free.
fn mend(
    model: Underside<'_>,
    needs: &mut Vec<Need>,
    loads: &[Load],
    plan: &SupportPlan,
    (taken, reach_mm): (&mut Buckets<Vec3>, Scalar),
    done: &mut Firmed,
) {
    for at in 0..needs.len() {
        let load = loads[at];
        let added = if load.sways() {
            let contact = under_sway(model, &load, reach_mm, taken);
            done.steadied += usize::from(contact.is_some());
            contact
        } else if weak(&load) {
            let tears = load.margin().is_some_and(|margin| margin < WANTED_MARGIN);
            if tears && heavier(plan, &needs[at]) {
                needs[at].heavier += 1;
                done.strengthened += 1;
                continue;
            }
            let reach = reach_mm * NEIGHBOUR_SPACINGS;
            let contact = load.z_mm.and_then(|worst_z_mm| {
                neighbour(model, needs[at].contact, (worst_z_mm, reach), taken)
            });
            done.strengthened += usize::from(contact.is_some());
            contact
        } else {
            None
        };
        if let Some(contact) = added {
            taken.insert(contact.truncate(), contact);
            needs.push(Need {
                contact,
                ..needs[at]
            });
        }
    }
}

/// Whether a tip tears or its trunk buckles.
fn weak(load: &Load) -> bool {
    let tears = load.margin().is_some_and(|margin| margin < WANTED_MARGIN);
    let buckles = load
        .buckling_margin
        .is_some_and(|margin| margin < WANTED_MARGIN);
    tears || buckles
}

/// The highest underside within a spacing round where the layer that swayed was pulled,
/// free and held by none of `taken`, no higher than that layer: a tip there holds the part
/// near where it moved.
fn under_sway(
    model: Underside<'_>,
    load: &Load,
    reach_mm: Scalar,
    taken: &Buckets<Vec3>,
) -> Option<Vec3> {
    let [x, y, z] = load.shift_at_mm?;
    let reach = reach_mm * SWAY_REACH_SPACINGS;
    let from_z = z - reach;
    std::iter::once([0.0, 0.0])
        .chain(
            AROUND
                .iter()
                .map(|[ax, ay]| [ax * reach / 2.0, ay * reach / 2.0]),
        )
        .filter_map(|[ax, ay]| {
            let from = Vec3::new(x + ax, y + ay, from_z);
            let hit = model.bvh.raycast(&Ray::new(from, Vec3::Z))?;
            let faces_down = model.surface.triangle(hit.face)?.normal_unnormalized().z < 0.0;
            let point = from + Vec3::Z * hit.t;
            let free = model
                .blocked
                .is_none_or(|blocked| !blocked.covers(point, 0.0));
            let held = taken
                .around(point.truncate())
                .any(|other| other.distance(point) < reach_mm * NEIGHBOUR_SPACINGS * TAKEN_SHARE);
            (faces_down && free && !held && point.z <= z).then_some(point)
        })
        .max_by(|a, b| a.z.total_cmp(&b.z))
}

#[cfg(test)]
mod tests;
