use super::{ModelSupports, next_revision};
use crate::SupportPoint;
use core_geometry::{Transform, Vec3};

impl ModelSupports {
    /// Puts a support where the model was clicked. `contact` is in plate coordinates.
    pub fn add(&mut self, contact: Vec3, transform: Transform, group: u16) {
        let Some(inverse) = transform.inverse_matrix() else {
            return;
        };
        self.points
            .push(SupportPoint::new(inverse.transform_point3(contact)).in_group(group));
        self.points_changed();
    }

    /// Takes every support built to `group` away with it, shifts the groups above down, and returns
    /// how many went; they go rather than regrow in an unasked shape, and history takes it back.
    pub fn drop_group(&mut self, group: u16) -> usize {
        let before = self.point_count();
        self.points.retain(|support| support.group != group);
        self.drop_frozen_of(group);
        let mut moved = false;
        for support in &mut self.points {
            if support.group > group {
                support.group -= 1;
                moved = true;
            }
        }
        for tree in &mut self.frozen {
            moved |= tree.close_up_over(group);
        }
        let gone = before - self.point_count();
        if gone > 0 || moved {
            self.points_changed();
        }
        gone
    }

    /// Takes away the point a support grew from. Out of range does nothing, which is what
    /// a click on a support that has since been rebuilt away amounts to.
    pub fn remove(&mut self, point: usize) {
        if point < self.points.len() {
            self.points.remove(point);
            self.points_changed();
        }
    }

    /// Keeps the points `keep` answers true for, by their index, and answers how many went.
    pub fn retain_points(&mut self, mut keep: impl FnMut(usize) -> bool) -> usize {
        let before = self.points.len();
        let mut index = 0;
        self.points.retain(|_| {
            index += 1;
            keep(index - 1)
        });
        let gone = before - self.points.len();
        if gone > 0 {
            self.points_changed();
        }
        gone
    }

    /// Keeps the frozen trees `keep` answers true for, by their index, and answers how many
    /// tips went with the rest.
    pub fn retain_frozen(&mut self, mut keep: impl FnMut(usize) -> bool) -> usize {
        let mut index = 0;
        let mut gone = 0;
        self.frozen.retain(|tree| {
            index += 1;
            let kept = keep(index - 1);
            if !kept {
                gone += tree.tip_count();
            }
            kept
        });
        if gone > 0 {
            self.built = None;
        }
        gone
    }

    /// Takes every support of `group` away and leaves the group itself, and every other
    /// group's number, as it was. Returns how many supports went.
    pub fn clear_group(&mut self, group: u16) -> usize {
        let before = self.point_count();
        self.points.retain(|support| support.group != group);
        self.drop_frozen_of(group);
        let gone = before - self.point_count();
        if gone > 0 {
            self.points_changed();
        }
        gone
    }

    /// Takes the frozen trees built to `group` away, and the tips of `group` off the trees
    /// of other groups they were merged into.
    pub(super) fn drop_frozen_of(&mut self, group: u16) {
        self.frozen = self
            .frozen
            .iter()
            .filter_map(|tree| tree.without_group(group))
            .collect();
    }

    /// Takes every support away: the points and the trees a hand has frozen alike.
    pub fn clear(&mut self) {
        self.points.clear();
        self.frozen.clear();
        self.points_changed();
    }

    /// Drops the trees grown from the points with everything built of them.
    pub(super) fn points_changed(&mut self) {
        self.forest = None;
        self.built = None;
        self.revision = next_revision();
    }
}
