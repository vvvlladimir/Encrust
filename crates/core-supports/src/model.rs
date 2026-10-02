use std::sync::Arc;

use core_geometry::{Adjacency, Bvh, Mesh, Scalar, Transform, Vec3};
use printer_profiles::SupportProfile;

use crate::{
    Blocked, Placed, Profiles, Region, SupportPoint, SupportTree, columns, grow, mesh_groups,
};

/// A transform this close to singular has flattened its model, and a click on it cannot
/// be mapped back into the model's own space.
const SINGULAR: f32 = 1e-12;

/// The supports of one model: where they touch it, what is painted on it, and the trees
/// and meshes built from that; see `docs/decisions/0129`.
///
/// Points are kept in the model's own space so that they stay on the surface when the
/// model is moved, turned or scaled. Everything derived from them is rebuilt whenever the
/// points, the placement or the profile change, and the mesh is kept behind an `Arc` so
/// that a viewport caches it on the GPU by address.
#[derive(Debug, Clone, Default)]
pub struct ModelSupports {
    points: Vec<SupportPoint>,
    /// The patch the user has painted to be filled with supports.
    painted: Region,
    /// The patch the user has painted to be left alone.
    blocked: Region,
    /// The supports the hand has taken hold of, kept as the trees they are, in the
    /// model's own space. Nothing regrows them; see `docs/decisions/0095`.
    frozen: Vec<SupportTree>,
    /// The two patches as geometry, kept apart from the columns because a stroke of the
    /// brush has to show up in the frame it was painted in, and the columns take too long
    /// to be rebuilt per frame of a drag.
    patches: Patches,
    built: Option<Built>,
}

#[derive(Debug, Clone)]
struct Built {
    /// Every tree standing under the model: the ones grown from the points first, the
    /// frozen ones after them.
    trees: Vec<SupportTree>,
    /// How many of `trees` were grown rather than frozen, which is where the frozen ones
    /// start and what turns a picked tree into an index into `frozen`.
    grown: usize,
    /// One mesh per group, in group order, so that each is drawn in a colour of its own.
    meshes: Vec<Arc<Mesh>>,
    transform: Transform,
    /// The profile of every group as it stood when this was built.
    table: Vec<SupportProfile>,
    painted: Region,
    blocked: Region,
}

/// The painted patches as they were last drawn, and what they were built from.
#[derive(Debug, Clone, Default)]
struct Patches {
    painted: Option<Arc<Mesh>>,
    blocked: Option<Arc<Mesh>>,
    painted_of: Region,
    blocked_of: Region,
    transform: Transform,
}

/// Millimetres a painted patch is drawn off the surface it covers, so that it is not
/// fighting the model's own faces for the same depth.
const PATCH_LIFT_MM: Scalar = 0.05;

impl ModelSupports {
    /// The supports a project file was saved with. Everything built from them is left
    /// out of the file and grown again on the next rebuild.
    pub fn restore(
        points: Vec<SupportPoint>,
        painted: Region,
        blocked: Region,
        frozen: Vec<SupportTree>,
    ) -> Self {
        Self {
            points,
            painted,
            blocked,
            frozen,
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.frozen.is_empty()
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
            + self
                .frozen
                .iter()
                .map(SupportTree::tip_count)
                .sum::<usize>()
    }

    /// Where every support touches the model, in plate coordinates: the points and the
    /// tips of the frozen trees alike. What an automatic run is handed as seeds.
    pub fn contacts(&self, transform: Transform) -> Vec<Vec3> {
        let matrix = transform.to_matrix();
        self.points
            .iter()
            .map(|point| point.contact)
            .chain(self.frozen.iter().flat_map(SupportTree::contacts))
            .map(|contact| matrix.transform_point3(contact))
            .collect()
    }

    /// Where the supports touch the model, in the model's own space.
    pub fn points(&self) -> &[SupportPoint] {
        &self.points
    }

    /// The supports as they stand: one tree per trunk, each carrying one tip or several.
    /// The grown ones come first and the frozen ones after them.
    pub fn trees(&self) -> &[SupportTree] {
        self.built.as_ref().map_or(&[], |built| &built.trees)
    }

    /// Which frozen tree a tree of [`ModelSupports::trees`] is, or `None` when it was
    /// grown from a point and is nobody's to edit until it has been frozen.
    pub fn frozen_of(&self, tree: usize) -> Option<usize> {
        let built = self.built.as_ref()?;
        tree.checked_sub(built.grown)
            .filter(|frozen| *frozen < self.frozen.len())
    }

    /// The trees the hand has taken hold of, in the model's own space.
    pub fn frozen(&self) -> &[SupportTree] {
        &self.frozen
    }

    /// Takes a grown tree out of the automatic run and keeps it as it stands, so that the
    /// next rebuild cannot undo what is about to be done to it. Answers where it landed
    /// in [`ModelSupports::frozen`], or `None` when there was nothing to freeze.
    ///
    /// The points its tips grew from go with it: a frozen tree carries its own contacts
    /// and is regrown by nobody.
    pub fn freeze(&mut self, tree: usize, transform: Transform) -> Option<usize> {
        if let Some(frozen) = self.frozen_of(tree) {
            return Some(frozen);
        }
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return None;
        }
        let standing = self.built.as_ref()?.trees.get(tree)?.clone();

        let mut taken: Vec<usize> = standing.points().collect();
        taken.sort_unstable();
        for point in taken.into_iter().rev() {
            if point < self.points.len() {
                self.points.remove(point);
            }
        }

        self.frozen.push(standing.moved(matrix.inverse()));
        self.built = None;
        Some(self.frozen.len() - 1)
    }

    /// The frozen tree `frozen`, to be edited in the model's own space.
    pub fn frozen_mut(&mut self, frozen: usize) -> Option<&mut SupportTree> {
        self.built = None;
        self.frozen.get_mut(frozen)
    }

    /// Hands a frozen tree back to the automatic run: its tips become points again and
    /// the next rebuild grows them afresh.
    pub fn thaw(&mut self, frozen: usize, transform: Transform) {
        if frozen >= self.frozen.len() {
            return;
        }
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return;
        }
        let tree = self.frozen.remove(frozen);
        for contact in tree.contacts() {
            self.points
                .push(SupportPoint::new(contact).in_group(tree.group()));
        }
        self.built = None;
    }

    /// Takes a frozen support away for good.
    pub fn remove_frozen(&mut self, frozen: usize) {
        if frozen < self.frozen.len() {
            self.frozen.remove(frozen);
            self.built = None;
        }
    }

    /// How many tips are being carried. Fewer than the points when one of them has no
    /// room under it.
    pub fn standing(&self) -> usize {
        self.trees().iter().map(SupportTree::tip_count).sum()
    }

    /// The column meshes, one per group and in plate coordinates, or `None` until they
    /// have been built.
    pub fn meshes(&self) -> Option<&[Arc<Mesh>]> {
        self.built.as_ref().map(|built| built.meshes.as_slice())
    }

    /// The patch painted to be supported, and the patch painted to be left alone, both in
    /// plate coordinates and both `None` until they have been built.
    pub fn patches(&self) -> (Option<&Arc<Mesh>>, Option<&Arc<Mesh>>) {
        (self.patches.painted.as_ref(), self.patches.blocked.as_ref())
    }

    /// Rebuilds the geometry of the two patches if the paint or the placement has moved
    /// under them. Cheap next to [`ModelSupports::refresh`]: it copies the painted
    /// triangles and nothing else, which is what lets a brush stroke show as it is drawn.
    pub fn refresh_patches(&mut self, model: &Mesh, transform: Transform) {
        let fresh = self.patches.transform == transform
            && self.patches.painted_of == self.painted
            && self.patches.blocked_of == self.blocked;
        if fresh {
            return;
        }

        self.patches = Patches {
            painted: self
                .painted
                .submesh(model, transform, PATCH_LIFT_MM)
                .map(Arc::new),
            blocked: self
                .blocked
                .submesh(model, transform, PATCH_LIFT_MM)
                .map(Arc::new),
            painted_of: self.painted.clone(),
            blocked_of: self.blocked.clone(),
            transform,
        };
    }

    /// The faces painted to be supported.
    pub fn painted(&self) -> &Region {
        &self.painted
    }

    /// The faces painted to be left alone.
    pub fn blocked(&self) -> &Region {
        &self.blocked
    }

    /// Paints or unpaints the faces within `radius_mm` of `at`, in plate coordinates.
    /// `blocking` paints the patch supports keep out of rather than the one they fill.
    pub fn paint(
        &mut self,
        placed: &Placed,
        at: Vec3,
        radius_mm: Scalar,
        blocking: bool,
        marked: bool,
    ) -> usize {
        let region = if blocking {
            &mut self.blocked
        } else {
            &mut self.painted
        };
        let changed = region.brush(placed, at, radius_mm, marked);
        if changed > 0 {
            self.built = None;
        }
        changed
    }

    /// Paints the surface the face `seed` belongs to, out to `max_angle_deg` of turn.
    pub fn paint_face(
        &mut self,
        model: &Mesh,
        adjacency: &Adjacency,
        seed: usize,
        max_angle_deg: Scalar,
        blocking: bool,
        marked: bool,
    ) -> usize {
        let region = if blocking {
            &mut self.blocked
        } else {
            &mut self.painted
        };
        let changed = region.flood(model, adjacency, seed, max_angle_deg, marked);
        if changed > 0 {
            self.built = None;
        }
        changed
    }

    /// Takes the paint off one of the two patches.
    pub fn clear_paint(&mut self, blocking: bool) {
        let region = if blocking {
            &mut self.blocked
        } else {
            &mut self.painted
        };
        if !region.is_empty() {
            region.clear();
            // Dropped rather than rebuilt: an object with nothing left on it is not asked
            // to refresh, so a stale patch would stay on screen.
            self.patches = Patches::default();
            self.built = None;
        }
    }

    /// What placement has to keep out of, or `None` when nothing is painted out of
    /// bounds.
    pub fn keep_out(&self, model: &Mesh, transform: Transform) -> Option<Blocked> {
        Blocked::new(model, &self.blocked, transform)
    }

    /// Puts a support where the model was clicked. `contact` is in plate coordinates.
    pub fn add(&mut self, contact: Vec3, transform: Transform, group: u16) {
        let matrix = transform.to_matrix();
        if matrix.determinant().abs() < SINGULAR {
            return;
        }
        self.points
            .push(SupportPoint::new(matrix.inverse().transform_point3(contact)).in_group(group));
        self.built = None;
    }

    /// Hands every support of `group` back to the default one, and shifts the groups
    /// above it down, for a group that has been taken away.
    pub fn regroup(&mut self, group: u16) {
        for support in &mut self.points {
            support.group = match support.group {
                at if at == group => 0,
                at if at > group => at - 1,
                at => at,
            };
        }
        self.built = None;
    }

    /// Takes away the point a support grew from. Out of range does nothing, which is what
    /// a click on a support that has since been rebuilt away amounts to.
    pub fn remove(&mut self, point: usize) {
        if point < self.points.len() {
            self.points.remove(point);
            self.built = None;
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.built = None;
    }

    /// Rebuilds the supports if anything they depend on has changed.
    ///
    /// Every point is dropped down the model again, which is one ray each through `bvh`,
    /// the hierarchy of `model`; see `docs/decisions/0036`. The tips that came through
    /// are then merged into shared trunks; see `docs/decisions/0040`.
    pub fn refresh(
        &mut self,
        model: &Mesh,
        bvh: &Bvh,
        transform: Transform,
        table: &[SupportProfile],
    ) {
        self.refresh_patches(model, transform);
        let Some(profiles) = Profiles::new(table) else {
            return;
        };
        let fresh = self.built.as_ref().is_some_and(|built| {
            built.transform == transform
                && built.table == table
                && built.painted == self.painted
                && built.blocked == self.blocked
        });
        if fresh {
            return;
        }

        let keep_out = self.keep_out(model, transform);
        let placed = Placed::new(model, bvh, transform).blocking(keep_out.as_ref());
        let columns = columns(&self.points, &placed, profiles);
        let mut trees = grow(&columns, &placed, profiles);
        let grown = trees.len();

        // A frozen tree is kept in the model's own space, so it rides the placement the
        // same way a point does; nothing else is asked of it.
        let matrix = transform.to_matrix();
        trees.extend(self.frozen.iter().map(|tree| tree.moved(matrix)));

        self.built = Some(Built {
            meshes: mesh_groups(&trees, &placed, profiles)
                .into_iter()
                .map(Arc::new)
                .collect(),
            trees,
            grown,
            transform,
            table: table.to_vec(),
            painted: self.painted.clone(),
            blocked: self.blocked.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Quat;
    use std::f32::consts::FRAC_PI_2;

    fn profile() -> SupportProfile {
        crate::tests::profile()
    }

    /// An empty model, so every column runs all the way to the plate.
    fn nothing() -> Mesh {
        Mesh::default()
    }

    /// Rebuilds against a model, building its hierarchy the way the scene does.
    fn refresh(supports: &mut ModelSupports, model: &Mesh, transform: Transform) {
        supports.refresh(
            model,
            &Bvh::build(model),
            transform,
            std::slice::from_ref(&profile()),
        );
    }

    /// Where the one support of `supports` meets the model, in plate coordinates.
    fn contact_of(supports: &ModelSupports) -> Vec3 {
        supports.trees()[0]
            .nodes()
            .iter()
            .find(|node| node.is_leaf())
            .expect("a support carries a tip")
            .position
    }

    fn with_one_support(transform: Transform) -> ModelSupports {
        let mut supports = ModelSupports::default();
        supports.add(Vec3::new(10.0, 10.0, 20.0), transform, 0);
        refresh(&mut supports, &nothing(), transform);
        supports
    }

    #[test]
    fn a_new_object_has_no_supports_and_no_mesh() {
        let supports = ModelSupports::default();
        assert!(supports.is_empty());
        assert!(supports.meshes().is_none());
        assert!(supports.trees().is_empty());
    }

    #[test]
    fn a_placed_support_becomes_a_column_with_geometry() {
        let supports = with_one_support(Transform::default());
        assert_eq!(supports.point_count(), 1);
        assert_eq!(supports.standing(), 1);
        assert!(!supports.meshes().expect("a column was built")[0].is_empty());
    }

    #[test]
    fn a_contact_is_stored_in_the_models_own_space() {
        let transform = Transform::from_translation(Vec3::new(50.0, 0.0, 0.0));
        let supports = with_one_support(transform);

        // Clicked at x = 10 on a model standing at x = 50, so the anchor is 40 mm back
        // along the model's own x, and comes out at x = 10 again when it is rebuilt.
        assert!(contact_of(&supports).abs_diff_eq(Vec3::new(10.0, 10.0, 20.0), 1e-4));
    }

    #[test]
    fn a_support_follows_the_model_that_is_moved_under_it() {
        let mut supports = with_one_support(Transform::default());
        let moved = Transform::from_translation(Vec3::new(5.0, 0.0, 0.0));
        refresh(&mut supports, &nothing(), moved);

        assert!(
            (contact_of(&supports).x - 15.0).abs() < 1e-4,
            "the anchor travelled with the model"
        );
    }

    #[test]
    fn a_support_follows_the_model_that_is_turned_under_it() {
        let mut supports = with_one_support(Transform::default());
        let turned = Transform {
            rotation: Quat::from_rotation_z(FRAC_PI_2),
            ..Transform::default()
        };
        refresh(&mut supports, &nothing(), turned);

        // A quarter turn about z maps (10, 10) onto (-10, 10).
        let contact = contact_of(&supports);
        assert!(
            contact.abs_diff_eq(Vec3::new(-10.0, 10.0, 20.0), 1e-4),
            "expected the anchor to turn with the model, got {contact}"
        );
    }

    #[test]
    fn a_thicker_profile_rebuilds_the_mesh() {
        let mut supports = with_one_support(Transform::default());
        let thin = supports.meshes().expect("built")[0].clone();

        supports.refresh(
            &nothing(),
            &Bvh::default(),
            Transform::default(),
            std::slice::from_ref(&SupportProfile::heavy()),
        );
        let thick = &supports.meshes().expect("rebuilt")[0];
        assert!(
            !Arc::ptr_eq(&thin, thick),
            "a new profile must produce a new mesh"
        );
    }

    #[test]
    fn nothing_changing_keeps_the_mesh_the_viewport_has_cached() {
        let mut supports = with_one_support(Transform::default());
        let first = supports.meshes().expect("built")[0].clone();

        refresh(&mut supports, &nothing(), Transform::default());
        assert!(
            Arc::ptr_eq(&first, &supports.meshes().expect("still built")[0]),
            "an unchanged object must not churn the GPU cache every frame"
        );
    }

    #[test]
    fn removing_a_point_drops_its_support() {
        let mut supports = with_one_support(Transform::default());
        supports.remove(0);
        refresh(&mut supports, &nothing(), Transform::default());

        assert!(supports.is_empty());
        assert!(supports.trees().is_empty());
        assert!(
            supports
                .meshes()
                .expect("an empty mesh is still built")
                .iter()
                .all(|mesh| mesh.is_empty())
        );
    }

    #[test]
    fn removing_a_point_that_is_not_there_changes_nothing() {
        let mut supports = with_one_support(Transform::default());
        supports.remove(7);
        assert_eq!(supports.point_count(), 1);
    }

    #[test]
    fn clearing_takes_every_support_away() {
        let mut supports = with_one_support(Transform::default());
        supports.add(Vec3::new(20.0, 20.0, 20.0), Transform::default(), 0);
        supports.clear();
        assert!(supports.is_empty());
    }

    #[test]
    fn a_click_on_a_flattened_model_places_nothing() {
        let flat = Transform {
            scale: Vec3::new(1.0, 1.0, 0.0),
            ..Transform::default()
        };
        let mut supports = ModelSupports::default();
        supports.add(Vec3::new(10.0, 10.0, 20.0), flat, 0);
        assert!(supports.is_empty());
    }
}
