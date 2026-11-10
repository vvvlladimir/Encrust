//! Weak tips made stronger in a few rounds: a tip under the wanted margin is built to the
//! next heavier shape, or given a neighbour once it is the heaviest. See docs/design/analysis.md.

use anyhow::Result;
use core_engine::SupportPlan;
use core_geometry::{Bvh, Mesh, Ray, Scalar, Vec3};
use printer_profiles::{AUTO_SIZES, MaterialProfile, PrinterProfile};
use serde::{Deserialize, Serialize};

use crate::shape;
use crate::stand::Supported;
use crate::tips::{self, WANTED_MARGIN};

/// The most rounds a model is strengthened in, each a full read of every tip.
const MOST_ROUNDS: usize = 3;
/// How far from a weak tip its neighbour stands, in the placement's spacing.
const NEIGHBOUR_SPACINGS: Scalar = 0.5;
/// The ways around a weak tip a neighbour is looked for.
const AROUND: [[Scalar; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];

/// What strengthening changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Strengthened {
    /// Rounds that changed anything.
    pub rounds: usize,
    /// Tips built to a heavier shape, once a round each.
    pub thickened: usize,
    /// Tips added beside a weak one already at the heaviest.
    pub added: usize,
}

/// Strengthens the tips of `supported` under [`WANTED_MARGIN`], built to `plan`, and grows
/// its trees again after each round.
pub fn strengthen(
    supported: &mut Supported,
    plan: &SupportPlan,
    printer: &PrinterProfile,
    material: &MaterialProfile,
    neck_mpa: f32,
) -> Result<Strengthened> {
    let reach_mm = shape::spacing_mm(&plan.placing()) * NEIGHBOUR_SPACINGS;
    let mut done = Strengthened::default();
    for _ in 0..MOST_ROUNDS {
        let weak = weak(supported, printer, material, neck_mpa)?;
        let bvh = Bvh::build(&supported.model);
        let before = done;
        for (at, worst_z_mm) in weak {
            if heavier(plan, supported.groups[at]) {
                supported.groups[at] += 1;
                done.thickened += 1;
            } else if let Some(contact) = neighbour(
                &supported.model,
                &bvh,
                supported.contacts[at],
                worst_z_mm,
                reach_mm,
            ) {
                supported.contacts.push(contact);
                supported.groups.push(supported.groups[at]);
                if let Some(&reason) = supported.reasons.get(at) {
                    supported.reasons.push(reason);
                }
                done.added += 1;
            }
        }
        if done == before {
            break;
        }
        done.rounds += 1;
        supported.regrow()?;
    }
    Ok(done)
}

/// Each standing tip under the wanted margin, and the height it is worst at.
fn weak(
    supported: &Supported,
    printer: &PrinterProfile,
    material: &MaterialProfile,
    neck_mpa: f32,
) -> Result<Vec<(usize, f32)>> {
    let read = tips::read(supported, printer, material, neck_mpa)?;
    Ok(shape::standing(supported)
        .into_iter()
        .zip(&read.tips)
        .filter(|(_, tip)| tip.margin.is_some_and(|margin| margin < WANTED_MARGIN))
        .filter_map(|(at, tip)| Some((at, tip.worst_z_mm?)))
        .collect())
}

/// Whether `plan` has a size after `group` with a wider tip, its steadying shape not counted.
fn heavier(plan: &SupportPlan, group: u16) -> bool {
    let next = usize::from(group) + 1;
    let table = plan.table();
    let tip_mm = |at: usize| table.get(at).map(|profile| profile.tip.contact_diameter_mm);
    plan.sizing().is_some() && next < AUTO_SIZES && tip_mm(next) > tip_mm(next - 1)
}

/// The lowest underside `reach_mm` around `tip` that seats no higher than `worst_z_mm`: a
/// neighbour seated over the layer the tip is worst at comes too late to share it.
fn neighbour(
    model: &Mesh,
    bvh: &Bvh,
    tip: Vec3,
    worst_z_mm: f32,
    reach_mm: Scalar,
) -> Option<Vec3> {
    AROUND
        .iter()
        .filter_map(|[x, y]| {
            // Starting a reach under the tip keeps a slope up to 45 degrees in front of the ray.
            let from = tip + Vec3::new(x * reach_mm, y * reach_mm, -reach_mm);
            let hit = bvh.raycast(&Ray::new(from, Vec3::Z))?;
            let faces_down = model.triangle(hit.face)?.normal_unnormalized().z < 0.0;
            let point = from + Vec3::Z * hit.t;
            (faces_down && point.z <= worst_z_mm).then_some(point)
        })
        .min_by(|a, b| a.z.total_cmp(&b.z))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::cuboid;
    use printer_profiles::Catalogue;

    /// A 10 mm block whose underside lies flat 5 mm over the plate.
    fn block() -> (Mesh, Bvh) {
        let block = cuboid(Vec3::new(0.0, 0.0, 5.0), Vec3::new(10.0, 10.0, 15.0));
        let bvh = Bvh::build(&block);
        (block, bvh)
    }

    #[test]
    fn a_neighbour_seats_on_the_same_flat_underside_a_reach_away() {
        let (block, bvh) = block();
        let tip = Vec3::new(5.0, 5.0, 5.0);
        let found = neighbour(&block, &bvh, tip, 6.0, 1.5).expect("the underside is all round");
        assert!(
            (found.z - 5.0).abs() < 1e-4
                && (found.truncate().distance(tip.truncate()) - 1.5).abs() < 1e-4,
            "expected a point of z = 5 mm, 1.5 mm off the tip, got {found}"
        );
    }

    #[test]
    fn no_neighbour_seats_after_the_layer_the_tip_is_worst_at() {
        let (block, bvh) = block();
        let found = neighbour(&block, &bvh, Vec3::new(5.0, 5.0, 5.0), 4.9, 1.5);
        assert_eq!(
            found, None,
            "the underside at 5 mm is over the worst layer at 4.9 mm"
        );
    }

    #[test]
    fn sizes_thicken_up_to_the_heaviest_and_never_into_the_steadying_shape() {
        let catalogue = Catalogue::bundled().expect("the bundled profiles load");
        let auto = catalogue
            .autos()
            .next()
            .expect("an automatic profile ships")
            .profile
            .clone();
        let sized = SupportPlan::sized(auto, &catalogue).expect("its shapes ship");
        let heaviest = (AUTO_SIZES - 1) as u16;
        assert!(heavier(&sized, heaviest - 1), "a lighter size thickens");
        assert!(
            !heavier(&sized, heaviest),
            "the heaviest size gets a neighbour"
        );
        assert!(
            !heavier(&sized, heaviest + 1),
            "the steadying shape gets a neighbour"
        );
        let light = catalogue
            .support("light")
            .expect("light ships")
            .profile
            .clone();
        assert!(
            !heavier(&SupportPlan::of(light), 0),
            "one profile has nothing heavier"
        );
    }

    #[test]
    fn a_size_no_wider_than_the_last_is_not_heavier() {
        let catalogue = Catalogue::bundled().expect("the bundled profiles load");
        let auto = catalogue
            .autos()
            .next()
            .expect("an automatic profile ships")
            .profile
            .clone();
        let mut sized = SupportPlan::sized(auto, &catalogue).expect("its shapes ship");
        for profile in sized.profiles_mut() {
            profile.tip.contact_diameter_mm = 0.4;
        }
        assert!(!heavier(&sized, 0), "every size has the same 0.4 mm tip");
    }
}
